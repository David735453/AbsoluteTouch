//! Session runner: transport, session state machine, injector, statistics.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::error::Error;
use crate::inject::Injector;
use crate::mapping::{MappedSample, Mapping};
use crate::protocol::packet::Packet;
use crate::protocol::types::PacketType;
use crate::session::{Action, Session, SessionConfig};
use crate::stats::Stats;
use crate::transport::trace::{TraceReader, TraceWriter};
use crate::transport::udp::{Inbound, UdpTransport};

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

    let mut session = Session::new(session_config(config)?, mapping(config), Instant::now());
    let mut recorder = record.map(TraceWriter::create).transpose()?;
    let mut injected = 0;

    while !stop.load(Ordering::Relaxed) {
        match transport.recv_timeout(TICK) {
            Ok(Some(Inbound::Packet { peer, packet })) => {
                if let Some(recorder) = &mut recorder {
                    recorder.record(&packet, Instant::now())?;
                }
                session.handle(peer, &packet, Instant::now());
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
            &transport,
            injector,
            &mut injected,
            config.dry_run,
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
    let peer = REPLAY_PEER
        .parse()
        .map_err(|_| Error::Usage("invalid replay peer".to_owned()))?;

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

fn apply_live_actions(
    session: &mut Session,
    transport: &UdpTransport,
    injector: &mut dyn Injector,
    injected: &mut u64,
    dry_run: bool,
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
                transport.send_to(&packet.encode(), peer)?;
                eprintln!("session {session_id} accepted from {peer}");
            }
            Action::SendHeartbeatAck { peer, packet } => {
                transport.send_to(&packet.encode(), peer)?;
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
    fn replay_rejects_bad_speed() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("empty.atrace");
        let writer = TraceWriter::create(&path).expect("create");
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
