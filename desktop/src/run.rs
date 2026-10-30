//! Session runner: transport, session state machine, injector, statistics.

use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::error::Error;
use crate::inject::Injector;
use crate::mapping::{MappedSample, Mapping};
use crate::protocol::packet::Packet;
use crate::protocol::types::PacketType;
use crate::session::{Action, Peer, Session, SessionConfig};
use crate::stats::Stats;
use crate::transport::framing::FrameDecoder;
use crate::transport::trace::{TraceReader, TraceWriter};
use crate::transport::udp::{Inbound, UdpTransport};
use crate::transport::usbmux::{self, Daemon};

/// How often the session loop wakes without traffic.
const TICK: Duration = Duration::from_millis(20);

/// Address used as the packet source during replay.
const REPLAY_PEER: &str = "127.0.0.1:1";

/// Outcome of a live session.
#[derive(Debug, Clone)]
pub struct Report {
    /// Session and transport counters.
    pub stats: Stats,
    /// Samples produced by the session: injected, or printed in dry-run mode.
    pub injected: u64,
    /// Packets written to the trace file, when recording.
    pub recorded: u64,
}

/// Outcome of a trace replay.
#[derive(Debug, Clone)]
pub struct ReplayReport {
    /// Session counters.
    pub stats: Stats,
    /// Records read from the trace.
    pub records: u64,
}

/// Run the Wi-Fi UDP session until `stop` is set.
///
/// # Errors
///
/// Returns an error when the socket cannot be bound, the trace or injector
/// fails, or the configuration is invalid.
pub fn wifi(
    config: &Config,
    injector: &mut dyn Injector,
    record: Option<&Path>,
    stop: &AtomicBool,
) -> Result<Report, Error> {
    let mut transport = UdpTransport::bind(&config.bind, config.udp_port)?;
    let local = transport.local_addr()?;
    eprintln!(
        "listening on udp {local} (backend {:?}, dry run: {})",
        config.backend, config.dry_run
    );

    let started = Instant::now();
    let mut session = Session::new(session_config(config)?, mapping(config), started);
    let mut recorder = record
        .map(|path| TraceWriter::create(path, started))
        .transpose()?;
    let mut injected = 0;

    while !stop.load(Ordering::Relaxed) {
        match transport.recv_timeout(TICK) {
            Ok(Some(Inbound::Packet { peer, packet })) => {
                // One timestamp for the trace and the session, so a replay
                // reconstructs exactly the clock alignment the live run saw.
                let now = Instant::now();
                if let Some(recorder) = &mut recorder {
                    recorder.record(&packet, now)?;
                }
                session.handle(peer.into(), &packet, now);
            }
            Ok(Some(Inbound::Invalid { peer, error })) => {
                session.stats_mut().record_protocol_error();
                eprintln!("dropped invalid datagram from {peer}: {error}");
            }
            Ok(None) => {}
            Err(error) => {
                eprintln!("udp receiver stopped: {error}");
                break;
            }
        }

        session.tick(Instant::now());
        apply_live_actions(
            &mut session,
            injector,
            &mut injected,
            config.dry_run,
            |bytes, peer| match peer.address() {
                Some(address) => transport.send_to(bytes, address),
                None => Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "usb peer on the udp transport",
                )),
            },
        )?;
    }

    injector.release_all()?;
    session.stats_mut().add_channel_drops(transport.dropped());
    transport.shutdown();

    let trace_records = match recorder {
        Some(recorder) => recorder.finish()?,
        None => 0,
    };

    Ok(Report {
        stats: *session.stats(),
        injected,
        recorded: trace_records,
    })
}

/// How long to wait between attempts to reach the iPad over USB.
const USB_RETRY: Duration = Duration::from_millis(500);

/// Longest single sleep while waiting for the iPad, so `stop` is honored soon.
const USB_RETRY_SLICE: Duration = Duration::from_millis(50);

/// Messages already printed while waiting for the iPad.
#[derive(Debug, Default)]
struct UsbStatus {
    /// Serial and port in the "connecting to" line.
    target: Option<String>,
    /// Last connection failure, so a retry does not repeat it.
    error: Option<String>,
}

/// Run the USB session until `stop` is set.
///
/// `usbmuxd` is asked for the attached devices, one is selected (the only one,
/// or the one `serial` names), and the daemon opens a TCP connection to the
/// port the iPad app listens on. The resulting stream is a raw byte pipe, so
/// [`FrameDecoder`] rebuilds the 40-byte packets the session expects.
///
/// Losing the stream does not end the run. A stopped app, a pulled cable, or a
/// restarted `usbmuxd` is retried every [`USB_RETRY`] until the iPad listens
/// again or `stop` is set, and input is released the moment the stream drops.
/// The next `HELLO` starts a fresh session, so the counters add up across
/// reconnects. A trace covers the first connection only: replay cannot model
/// the gap, and truncating at the disconnect is what keeps its summary exact.
///
/// # Errors
///
/// Returns an error when the trace or injector fails. Connection problems are
/// retried and reported on stderr instead.
pub fn usb(
    config: &Config,
    serial: Option<&str>,
    injector: &mut dyn Injector,
    record: Option<&Path>,
    stop: &AtomicBool,
) -> Result<Report, Error> {
    let mut session: Option<Session> = None;
    let mut recorder: Option<TraceWriter> = None;
    let mut trace_records = 0;
    let mut injected = 0;
    let mut progress = UsbStatus::default();

    while let Some((device, mut stream)) = usb_connect(config, serial, stop, &mut progress)? {
        if session.is_none() {
            // The first connection fixes the trace origin and the session's
            // clock, so the offsets stay comparable with a single-session
            // replay.
            let started = Instant::now();
            session = Some(Session::new(
                session_config(config)?,
                mapping(config),
                started,
            ));
            recorder = record
                .map(|path| TraceWriter::create(path, started))
                .transpose()?;
        }
        let session = session.as_mut().expect("session exists after connecting");

        let mut decoder = FrameDecoder::new();
        let mut buffer = [0_u8; 1_024];

        while !stop.load(Ordering::Relaxed) {
            // On unix the tick comes from `poll`. Windows uses the socket read
            // timeout instead, so it always calls `read` and treats
            // `WouldBlock` as an empty tick.
            #[cfg(unix)]
            let readable = wait_readable(&stream, TICK)?;
            #[cfg(not(unix))]
            let readable = true;

            if readable {
                match stream.read(&mut buffer) {
                    Ok(0) => {
                        eprintln!("usb connection closed by the iPad");
                        break;
                    }
                    Ok(count) => decoder.push(&buffer[..count]),
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                        ) => {}
                    Err(error) => {
                        eprintln!("usb read failed: {error}");
                        break;
                    }
                }
            }

            while let Some(frame) = decoder.next_frame() {
                let now = Instant::now();
                match Packet::decode(&frame) {
                    Ok(packet) => {
                        if let Some(recorder) = &mut recorder {
                            recorder.record(&packet, now)?;
                        }
                        session.handle(Peer::Connection(u64::from(device.id)), &packet, now);
                    }
                    Err(error) => {
                        session.stats_mut().record_protocol_error();
                        eprintln!("dropped invalid usb frame: {error}");
                    }
                }
            }

            session.tick(Instant::now());
            match apply_live_actions(
                session,
                injector,
                &mut injected,
                config.dry_run,
                |bytes, _peer| stream.write_all(bytes).map(|()| bytes.len()),
            ) {
                Ok(()) => {}
                Err(Error::Io(error)) if is_disconnect(&error) => {
                    eprintln!("usb write failed: {error}");
                    break;
                }
                Err(error) => return Err(error),
            }
        }

        session.disconnect();
        injector.release_all()?;
        progress.target = None;

        if let Some(writer) = recorder.take() {
            trace_records = writer.finish()?;
            if !stop.load(Ordering::Relaxed) {
                eprintln!("trace ended with the connection; recording a reconnect needs a new run");
            }
        }
    }

    let stats = session.as_ref().map_or_else(Stats::default, |s| *s.stats());
    Ok(Report {
        stats,
        injected,
        recorded: trace_records,
    })
}

/// Connect to the iPad, retrying until it works or `stop` is set.
///
/// Prints each distinct failure once and waits [`USB_RETRY`] before the next
/// attempt, so a stopped app or a pulled cable can be fixed without
/// restarting the desktop.
fn usb_connect(
    config: &Config,
    serial: Option<&str>,
    stop: &AtomicBool,
    progress: &mut UsbStatus,
) -> Result<Option<(usbmux::Device, usbmux::Stream)>, Error> {
    loop {
        if stop.load(Ordering::Relaxed) {
            return Ok(None);
        }

        match connect_once(config, serial, progress) {
            Ok(connection) => {
                progress.error = None;
                return Ok(Some(connection));
            }
            Err(error) => {
                let message = error.to_string();
                if progress.error.as_deref() != Some(message.as_str()) {
                    eprintln!(
                        "{message} (retrying every {} ms; press Ctrl+C to stop)",
                        USB_RETRY.as_millis()
                    );
                    progress.error = Some(message);
                }
            }
        }

        sleep_unless_stopped(USB_RETRY, stop);
    }
}

/// One connection attempt, including the announcement line.
fn connect_once(
    config: &Config,
    serial: Option<&str>,
    progress: &mut UsbStatus,
) -> Result<(usbmux::Device, usbmux::Stream), Error> {
    let mut daemon = Daemon::connect()?;
    let devices = daemon.list_devices()?;
    let device = usbmux::select(&devices, serial)?.clone();

    let target = format!(
        "connecting to {} over usb (port {})",
        device.serial, config.tcp_port
    );
    if progress.target.as_deref() != Some(target.as_str()) {
        eprintln!("{target}");
        progress.target = Some(target);
    }

    let stream = daemon.connect_device(device.id, config.tcp_port)?;
    // Windows has no `poll` here, so the read timeout provides the tick. On
    // unix the runner polls instead: Darwin rejects `SO_RCVTIMEO` on a socket
    // whose peer has already closed, which killed the first attempt after a
    // dropped connection.
    #[cfg(not(unix))]
    stream.set_read_timeout(Some(TICK))?;
    Ok((device, stream))
}

/// Wait until `stream` has bytes to read, or `timeout` expires.
///
/// Returns `false` when the timeout expired first. A socket timeout would be
/// the obvious alternative, but `usbmuxd` hands over a stream whose peer may
/// already be gone, and macOS answers `EINVAL` to `setsockopt(SO_RCVTIMEO)`
/// on such a socket. Polling re-checks the descriptor each tick instead.
#[cfg(unix)]
fn wait_readable(stream: &usbmux::Stream, timeout: Duration) -> Result<bool, Error> {
    use rustix::event::{PollFd, PollFlags, poll};
    use rustix::time::Timespec;

    let seconds = i64::try_from(timeout.as_secs()).unwrap_or(i64::MAX);
    let deadline = Timespec {
        tv_sec: seconds,
        tv_nsec: i64::from(timeout.subsec_nanos()),
    };
    let mut fds = [PollFd::new(stream, PollFlags::IN)];
    match poll(&mut fds, Some(&deadline)) {
        Ok(0) => Ok(false),
        Ok(_) => Ok(true),
        Err(error) => Err(io::Error::from(error).into()),
    }
}

/// Sleep for `total`, returning early when `stop` is set.
fn sleep_unless_stopped(total: Duration, stop: &AtomicBool) {
    let deadline = Instant::now() + total;
    while !stop.load(Ordering::Relaxed) {
        let now = Instant::now();
        if now >= deadline {
            return;
        }
        std::thread::sleep(USB_RETRY_SLICE.min(deadline.saturating_duration_since(now)));
    }
}

/// Whether an I/O error means the USB stream is gone and worth reconnecting.
fn is_disconnect(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::BrokenPipe
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::NotConnected
            | io::ErrorKind::UnexpectedEof
    )
}

/// Replay a recorded trace through the same session and mapping code.
///
/// Replay never opens a socket. Acknowledgements are counted but discarded.
///
/// # Errors
///
/// Returns an error for an unreadable trace, an invalid speed, or an injector
/// failure.
pub fn replay(
    path: &Path,
    speed: f32,
    dry_run: bool,
    config: &Config,
    injector: &mut dyn Injector,
) -> Result<ReplayReport, Error> {
    if !speed.is_finite() || !(0.001..=1_000.0).contains(&speed) {
        return Err(Error::Usage(
            "--speed must be between 0.001 and 1000".to_owned(),
        ));
    }

    let session_id = peek_session_id(path)?.unwrap_or(1);
    let peer = Peer::Network(
        REPLAY_PEER
            .parse()
            .map_err(|_| Error::Usage("invalid replay peer".to_owned()))?,
    );

    let mut reader = TraceReader::open(path)?;
    let started = Instant::now();
    let mut session = Session::with_ids(
        session_config(config)?,
        mapping(config),
        started,
        crate::session::FixedIds(session_id),
    );

    let mut records = 0;

    while let Some(record) = reader.next_record()? {
        let target = scale(record.offset, speed);
        let elapsed = started.elapsed();
        if target > elapsed {
            std::thread::sleep(target.saturating_sub(elapsed));
        }
        let now = started + target;

        session.handle(peer, &record.packet, now);
        session.tick(now);
        records += 1;

        for action in session.take_actions() {
            match action {
                Action::Inject(mapped) if dry_run => print_sample(&mapped),
                Action::Inject(mapped) => injector.inject(&mapped)?,
                Action::ReleaseAll => injector.release_all()?,
                Action::SendHelloAck { .. }
                | Action::SendHeartbeatAck { .. }
                | Action::Rejected { .. } => {}
            }
        }
    }

    injector.release_all()?;

    Ok(ReplayReport {
        stats: *session.stats(),
        records,
    })
}

/// Apply the actions the session produced, replying through `send`.
///
/// The transport is a closure rather than a socket so the same session loop
/// serves UDP (which addresses a peer) and USB (which owns one stream). `send`
/// receives the peer the session answered, so UDP can address it and USB can
/// ignore it.
fn apply_live_actions(
    session: &mut Session,
    injector: &mut dyn Injector,
    injected: &mut u64,
    dry_run: bool,
    mut send: impl FnMut(&[u8], Peer) -> io::Result<usize>,
) -> Result<(), Error> {
    for action in session.take_actions() {
        match action {
            Action::SendHelloAck {
                peer,
                session_id,
                token,
                caps,
            } => {
                let mut packet = Packet::new(PacketType::HelloAck);
                packet.session_id = session_id;
                packet.set_token(token);
                packet.flags = caps;
                send(&packet.encode(), peer)?;
                eprintln!("session {session_id} accepted from {peer}");
            }
            Action::SendHeartbeatAck { peer, packet } => {
                send(&packet.encode(), peer)?;
            }
            Action::Rejected { peer, reason } => {
                eprintln!("rejected packet from {peer}: {reason:?}");
            }
            Action::Inject(mapped) if dry_run => {
                *injected += 1;
                print_sample(&mapped);
            }
            Action::Inject(mapped) => match injector.inject(&mapped) {
                Ok(()) => *injected += 1,
                Err(error) => {
                    session.stats_mut().record_inject_error();
                    eprintln!("inject failed: {error}; releasing input");
                    injector.release_all()?;
                }
            },
            Action::ReleaseAll => injector.release_all()?,
        }
    }
    Ok(())
}

fn print_sample(sample: &MappedSample) {
    let tilt = sample.tilt.map_or_else(
        || "none".to_owned(),
        |tilt| format!("{:+},{:+}", tilt.x, tilt.y),
    );
    println!(
        "{:5} {:5} pressure={:5} tilt={} contact={} hover={}",
        sample.x, sample.y, sample.pressure, tilt, sample.contact, sample.hover
    );
}

fn session_config(config: &Config) -> Result<SessionConfig, Error> {
    Ok(SessionConfig {
        token: config.token_bytes()?,
        watchdog: Duration::from_millis(config.watchdog_ms),
        session_timeout: Duration::from_millis(config.session_timeout_ms),
        pressure_default: config.pressure_default,
        ..SessionConfig::default()
    })
}

fn mapping(config: &Config) -> Mapping {
    Mapping::new(
        config.mapping.target,
        config.mapping.aspect,
        config.mapping.source_aspect,
        config.mapping.target_aspect,
    )
}

fn scale(offset: Duration, speed: f32) -> Duration {
    offset.div_f32(speed)
}

fn peek_session_id(path: &Path) -> Result<Option<u32>, Error> {
    let mut reader = TraceReader::open(path)?;
    while let Some(record) = reader.next_record()? {
        if !matches!(
            record.packet.packet_type,
            PacketType::Hello | PacketType::HelloAck
        ) && record.packet.session_id != crate::session::NO_SESSION
        {
            return Ok(Some(record.packet.session_id));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use std::net::UdpSocket;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    use super::{replay, wifi};
    use crate::config::Config;
    use crate::inject::Injector;
    use crate::inject::mock::{Event, MockInjector};
    use crate::protocol::packet::Packet;
    use crate::protocol::types::{PacketType, flags};
    use crate::transport::trace::TraceWriter;

    const TOKEN: [u8; 16] = [0x22; 16];

    fn config(port: u16) -> Config {
        Config {
            bind: "127.0.0.1".to_owned(),
            udp_port: port,
            token: Some(crate::config::encode_hex16(&TOKEN)),
            backend: "mock".to_owned(),
            ..Config::default()
        }
    }

    fn free_port() -> u16 {
        let socket = UdpSocket::bind("127.0.0.1:0").expect("bind");
        socket.local_addr().expect("addr").port()
    }

    fn hello() -> Packet {
        let mut packet = Packet::new(PacketType::Hello);
        packet.set_token(TOKEN);
        packet.flags = flags::CAP_TILT | flags::CAP_HOVER;
        packet
    }

    fn sample(session_id: u32, seq: u16) -> Packet {
        let mut packet = Packet::new(PacketType::Sample);
        packet.session_id = session_id;
        packet.seq = seq;
        packet.x = u16::MAX;
        packet.y = u16::MAX;
        packet.flags = flags::CONTACT;
        packet
    }

    fn handshake(client: &UdpSocket, config: &Config) -> u32 {
        use std::io::ErrorKind;

        let peer = format!("{}:{}", config.bind, config.udp_port);
        client
            .set_read_timeout(Some(Duration::from_millis(100)))
            .expect("timeout");
        let mut buffer = [0_u8; 64];

        // UDP has no connection setup, so retry HELLO as the iPad does until
        // the server thread has bound its socket and answered.
        for _ in 0..40 {
            client.send_to(&hello().encode(), &peer).expect("hello");
            match client.recv_from(&mut buffer) {
                Ok((length, _)) => {
                    let ack = Packet::decode(&buffer[..length]).expect("decode ack");
                    assert_eq!(ack.packet_type, PacketType::HelloAck);
                    return ack.session_id;
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => {}
                Err(error) => panic!("hello ack: {error}"),
            }
        }
        panic!("no HELLO_ACK after retries");
    }

    #[test]
    fn wifi_session_accepts_and_injects() {
        let config = config(free_port());
        let port = config.udp_port;
        let stop = Arc::new(AtomicBool::new(false));
        let runner = {
            let stop = Arc::clone(&stop);
            let config = config.clone();
            std::thread::spawn(move || {
                let mut injector = RecordingInjector::default();
                let report = wifi(&config, &mut injector, None, &stop).expect("wifi");
                (report, injector.take())
            })
        };

        let client = UdpSocket::bind("127.0.0.1:0").expect("client");
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("timeout");
        let session_id = handshake(&client, &config);
        client
            .send_to(&sample(session_id, 1).encode(), format!("127.0.0.1:{port}"))
            .expect("sample");
        std::thread::sleep(Duration::from_millis(100));
        stop.store(true, Ordering::Relaxed);

        let (report, events) = runner.join().expect("join");

        assert_eq!(report.stats.sessions, 1);
        assert!(events.iter().any(|event| matches!(event, Event::Sample(_))));
        assert!(
            events
                .iter()
                .any(|event| matches!(event, Event::ReleaseAll))
        );
    }

    #[test]
    fn recording_a_session_produces_a_replayable_trace() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("session.atrace");
        let config = config(free_port());
        let port = config.udp_port;
        let stop = Arc::new(AtomicBool::new(false));
        let runner = {
            let stop = Arc::clone(&stop);
            let config = config.clone();
            let path = path.clone();
            std::thread::spawn(move || {
                let mut injector = RecordingInjector::default();
                wifi(&config, &mut injector, Some(&path), &stop).expect("wifi")
            })
        };

        let client = UdpSocket::bind("127.0.0.1:0").expect("client");
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("timeout");
        let session_id = handshake(&client, &config);
        client
            .send_to(&sample(session_id, 1).encode(), format!("127.0.0.1:{port}"))
            .expect("sample");
        std::thread::sleep(Duration::from_millis(100));
        stop.store(true, Ordering::Relaxed);

        let report = runner.join().expect("join");
        assert!(report.recorded >= 2);

        let mut injector = MockInjector::default();
        let replayed = replay(&path, 1000.0, false, &config, &mut injector).expect("replay");

        assert!(replayed.records >= 2);
        assert!(replayed.stats.samples >= 1);
        assert!(
            injector
                .events()
                .iter()
                .any(|event| matches!(event, Event::Sample(_)))
        );
    }

    #[test]
    fn replay_reproduces_the_heartbeat_clock_alignment() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("timed.atrace");
        let config = config(free_port());
        let port = config.udp_port;
        let peer = format!("{}:{}", config.bind, port);
        let stop = Arc::new(AtomicBool::new(false));
        let runner = {
            let stop = Arc::clone(&stop);
            let config = config.clone();
            let path = path.clone();
            std::thread::spawn(move || {
                let mut injector = RecordingInjector::default();
                wifi(&config, &mut injector, Some(&path), &stop).expect("wifi")
            })
        };

        // Wait before connecting so the first record lands well after the
        // session starts. A trace timestamped from the first record instead
        // would shift every offset by this delay.
        std::thread::sleep(Duration::from_millis(150));

        let client = UdpSocket::bind("127.0.0.1:0").expect("client");
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("timeout");
        let session_id = handshake(&client, &config);

        // The first heartbeat has no acknowledgement to echo, so it teaches
        // the host nothing about either clock.
        let mut first = Packet::new(PacketType::Heartbeat);
        first.session_id = session_id;
        first.seq = 1;
        first.device_time_us = 10_000_000;
        client.send_to(&first.encode(), &peer).expect("heartbeat");

        let mut buffer = [0_u8; 64];
        let (length, _) = client.recv_from(&mut buffer).expect("ack");
        let ack = Packet::decode(&buffer[..length]).expect("decode ack");
        assert_eq!(ack.packet_type, PacketType::HeartbeatAck);

        // The second heartbeat echoes that acknowledgement. The sender reports
        // no wait of its own, so the turnaround is the network round trip.
        let echoed_device: u32 = 10_020_000;
        let mut second = Packet::new(PacketType::Heartbeat);
        second.session_id = session_id;
        second.seq = 2;
        second.device_time_us = 10_020_000;
        second.x = (echoed_device & 0xFFFF) as u16;
        second.y = (echoed_device >> 16) as u16;
        second.pressure = ack.pressure;
        second.altitude = ack.altitude;
        client.send_to(&second.encode(), &peer).expect("heartbeat");
        let (length, _) = client.recv_from(&mut buffer).expect("ack");
        assert_eq!(
            Packet::decode(&buffer[..length])
                .expect("decode ack")
                .packet_type,
            PacketType::HeartbeatAck
        );

        let mut stroke = sample(session_id, 3);
        stroke.x = 100;
        stroke.y = 200;
        stroke.device_time_us = 10_030_000;
        client.send_to(&stroke.encode(), &peer).expect("sample");
        std::thread::sleep(Duration::from_millis(100));
        stop.store(true, Ordering::Relaxed);

        let report = runner.join().expect("join");
        assert!(report.stats.last_rtt_us.is_some());
        assert!(report.stats.last_sample_excess_us().is_some());

        let mut injector = MockInjector::default();
        // Replay at real speed: compressing time would invalidate the clock
        // alignment the run is about to check.
        let replayed = replay(&path, 1.0, false, &config, &mut injector).expect("replay");

        assert_eq!(replayed.stats.last_rtt_us, report.stats.last_rtt_us);
        assert!(replayed.stats.last_sample_excess_us().is_some());
    }

    #[test]
    fn replay_rejects_bad_speed() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("empty.atrace");
        let writer = TraceWriter::create(&path, Instant::now()).expect("create");
        writer.finish().expect("finish");
        let mut injector = MockInjector::default();

        assert!(replay(&path, 0.0, true, &Config::default(), &mut injector).is_err());
        assert!(replay(&path, 1_001.0, true, &Config::default(), &mut injector).is_err());
        assert!(replay(&path, f32::NAN, true, &Config::default(), &mut injector).is_err());
    }

    #[derive(Debug, Default)]
    struct RecordingInjector {
        events: Vec<Event>,
    }

    impl RecordingInjector {
        fn take(&mut self) -> Vec<Event> {
            std::mem::take(&mut self.events)
        }
    }

    impl Injector for RecordingInjector {
        fn inject(
            &mut self,
            sample: &crate::mapping::MappedSample,
        ) -> Result<(), crate::inject::InjectError> {
            self.events.push(Event::Sample(*sample));
            Ok(())
        }

        fn release_all(&mut self) -> Result<(), crate::inject::InjectError> {
            self.events.push(Event::ReleaseAll);
            Ok(())
        }
    }

    #[test]
    fn session_config_matches_the_file() {
        let config = config(37_689);
        let session = super::session_config(&config).expect("session config");

        assert_eq!(session.token, Some(TOKEN));
        assert_eq!(session.watchdog, Duration::from_millis(750));
        assert_eq!(session.pressure_default, 32_768);
    }

    #[test]
    fn scaled_offsets_are_finite() {
        assert_eq!(
            super::scale(Duration::from_millis(100), 2.0),
            Duration::from_millis(50)
        );
        assert!(Instant::now().elapsed() < Duration::from_secs(1));
    }
}
