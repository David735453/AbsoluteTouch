//! Session state machine: handshake, sequence window, heartbeats, watchdog.
//!
//! The session is transport-agnostic and has no clock of its own: callers pass
//! `Instant` values, which keeps every rule testable without sleeping.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use crate::mapping::{MappedSample, Mapping, TabletSample, Tilt};
use crate::protocol::packet::Packet;
use crate::protocol::types::{PacketType, flags};
use crate::stats::Stats;

/// Capability bits the reference host can consume.
pub const HOST_CAPS: u16 = flags::CAP_PRESSURE | flags::CAP_TILT | flags::CAP_HOVER;

/// Mask of the capability bits defined by version 1.
pub const CAP_MASK: u16 =
    flags::CAP_PRESSURE | flags::CAP_TILT | flags::CAP_HOVER | flags::CAP_TWIST;

/// Session identifiers are never zero.
pub const NO_SESSION: u32 = 0;

/// Policy for one session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionConfig {
    /// Required pairing token; `None` accepts any token.
    pub token: Option<[u8; 16]>,
    /// Capabilities advertised in `HELLO_ACK`.
    pub host_caps: u16,
    /// Silence after which input is released but the session stays alive.
    pub watchdog: Duration,
    /// Silence after which the session ends.
    pub session_timeout: Duration,
    /// Pressure substituted when the iPad lacks `CAP_PRESSURE`.
    pub pressure_default: u16,
    /// Only the peer that completed the handshake may send data.
    pub pin_peer: bool,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            token: None,
            host_caps: HOST_CAPS,
            watchdog: Duration::from_millis(750),
            session_timeout: Duration::from_millis(5_000),
            pressure_default: 32_768,
            pin_peer: true,
        }
    }
}

/// Why a packet did not join or continue a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    /// The token did not match the configured one.
    BadToken,
    /// Another peer already owns the session.
    Busy,
    /// Data arrived before a handshake.
    NotStreaming,
    /// The session identifier or peer did not match.
    WrongSession,
}

/// An effect the caller must perform after handling a packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Accept the session.
    SendHelloAck {
        /// Destination.
        peer: SocketAddr,
        /// Assigned session identifier.
        session_id: u32,
        /// Echoed token.
        token: [u8; 16],
        /// Advertised host capabilities.
        caps: u16,
    },
    /// Answer a heartbeat.
    SendHeartbeatAck {
        /// Destination.
        peer: SocketAddr,
        /// Fully populated acknowledgement.
        packet: Packet,
    },
    /// Reject or drop an inbound packet.
    Rejected {
        /// Source of the packet.
        peer: SocketAddr,
        /// Reason for the rejection.
        reason: RejectReason,
    },
    /// Inject a mapped sample.
    Inject(MappedSample),
    /// Release every pressed button.
    ReleaseAll,
}

/// Source of session identifiers.
pub trait SessionIds: std::fmt::Debug {
    /// Return a non-zero identifier.
    fn next_id(&mut self) -> u32;
}

/// Random session identifiers.
#[derive(Debug, Default, Clone, Copy)]
pub struct RandomIds;

impl SessionIds for RandomIds {
    fn next_id(&mut self) -> u32 {
        loop {
            let mut bytes = [0_u8; 4];
            if getrandom::fill(&mut bytes).is_ok() {
                let id = u32::from_le_bytes(bytes);
                if id != NO_SESSION {
                    return id;
                }
            } else {
                let nanos = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(1, |elapsed| elapsed.as_nanos() as u64);
                return (nanos as u32).max(1);
            }
        }
    }
}

/// Deterministic session identifiers for tests and replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedIds(pub u32);

impl SessionIds for FixedIds {
    fn next_id(&mut self) -> u32 {
        let id = self.0.max(1);
        self.0 = id.wrapping_add(1).max(1);
        id
    }
}

/// Sequence tracking with a 64-packet reorder window.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SeqWindow {
    last: u16,
    seen: u64,
    initialized: bool,
}

/// Result of offering a sequence number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeqResult {
    /// A new packet.
    Fresh,
    /// The sequence number was already seen.
    Duplicate,
    /// The sequence number is too old to reason about.
    Stale,
}

impl SeqWindow {
    /// Forget all history.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Offer a sequence number.
    pub fn accept(&mut self, seq: u16) -> SeqResult {
        if !self.initialized {
            self.initialized = true;
            self.last = seq;
            self.seen = 1;
            return SeqResult::Fresh;
        }

        let difference = seq.wrapping_sub(self.last) as i16;
        match difference.cmp(&0) {
            std::cmp::Ordering::Greater => {
                let shift = difference.unsigned_abs();
                self.seen = if shift >= 64 {
                    1
                } else {
                    (self.seen << shift) | 1
                };
                self.last = seq;
                SeqResult::Fresh
            }
            std::cmp::Ordering::Equal => SeqResult::Duplicate,
            std::cmp::Ordering::Less => {
                let behind = difference.unsigned_abs();
                if behind > 63 {
                    return SeqResult::Stale;
                }
                let bit = 1_u64 << behind;
                if self.seen & bit != 0 {
                    SeqResult::Duplicate
                } else {
                    self.seen |= bit;
                    SeqResult::Fresh
                }
            }
        }
    }
}

#[derive(Debug)]
struct Stream {
    session_id: u32,
    peer: SocketAddr,
    token: [u8; 16],
    caps: u16,
    last_packet: Instant,
    released: bool,
    seq: SeqWindow,
}

/// Handshake and data state machine.
#[derive(Debug)]
pub struct Session<I: SessionIds = RandomIds> {
    config: SessionConfig,
    mapping: Mapping,
    ids: I,
    stream: Option<Stream>,
    actions: Vec<Action>,
    stats: Stats,
    started: Instant,
}

impl Session<RandomIds> {
    /// Create a session with random identifiers.
    #[must_use]
    pub fn new(config: SessionConfig, mapping: Mapping, started: Instant) -> Self {
        Self::with_ids(config, mapping, started, RandomIds)
    }
}

impl<I: SessionIds> Session<I> {
    /// Create a session with an explicit identifier source.
    #[must_use]
    pub fn with_ids(config: SessionConfig, mapping: Mapping, started: Instant, ids: I) -> Self {
        Self {
            config,
            mapping,
            ids,
            stream: None,
            actions: Vec::with_capacity(4),
            stats: Stats::default(),
            started,
        }
    }

    /// Counters collected so far.
    #[must_use]
    pub const fn stats(&self) -> &Stats {
        &self.stats
    }

    /// Mutable counters, for transport-side drops.
    pub const fn stats_mut(&mut self) -> &mut Stats {
        &mut self.stats
    }

    /// The active session identifier, if any.
    #[must_use]
    pub fn session_id(&self) -> Option<u32> {
        self.stream.as_ref().map(|stream| stream.session_id)
    }

    /// Whether a session is currently established.
    #[must_use]
    pub const fn is_streaming(&self) -> bool {
        self.stream.is_some()
    }

    /// Whether the watchdog released input and no new sample has arrived.
    #[must_use]
    pub fn is_released(&self) -> bool {
        self.stream.as_ref().is_none_or(|stream| stream.released)
    }

    /// Drain the effects of the packets handled so far.
    pub fn take_actions(&mut self) -> Vec<Action> {
        std::mem::take(&mut self.actions)
    }

    /// Handle one decoded packet.
    pub fn handle(&mut self, peer: SocketAddr, packet: &Packet, now: Instant) {
        self.stats.record_packet();

        if let Some(stream) = &self.stream {
            if now.saturating_duration_since(stream.last_packet) >= self.config.session_timeout {
                self.expire();
            }
        }

        match packet.packet_type {
            PacketType::Hello => self.on_hello(peer, packet, now),
            PacketType::Sample => self.on_sample(peer, packet, now),
            PacketType::Heartbeat => self.on_heartbeat(peer, packet, now),
            PacketType::Bye => self.on_bye(peer, packet),
            PacketType::HelloAck | PacketType::HeartbeatAck => {
                self.stats.record_ignored();
            }
        }
    }

    /// Advance timers without a packet.
    pub fn tick(&mut self, now: Instant) {
        let Some(stream) = &self.stream else {
            return;
        };
        let silence = now.saturating_duration_since(stream.last_packet);

        if silence >= self.config.session_timeout {
            self.expire();
        } else if silence >= self.config.watchdog && !stream.released {
            self.release(true);
        }
    }

    fn expire(&mut self) {
        self.stats.record_timeout();
        self.release(false);
        self.stream = None;
    }

    fn release(&mut self, watchdog: bool) {
        if let Some(stream) = &mut self.stream {
            if stream.released && watchdog {
                return;
            }
            stream.released = true;
        }
        if watchdog {
            self.stats.record_watchdog_release();
        }
        self.stats.record_release();
        self.actions.push(Action::ReleaseAll);
    }

    fn on_hello(&mut self, peer: SocketAddr, packet: &Packet, now: Instant) {
        let token = packet.token();

        if let Some(expected) = self.config.token {
            if token != expected {
                self.reject(peer, RejectReason::BadToken);
                return;
            }
        }

        if let Some((existing_peer, existing_token, existing_id)) = self
            .stream
            .as_ref()
            .map(|stream| (stream.peer, stream.token, stream.session_id))
        {
            if existing_peer == peer && existing_token == token {
                // The iPad repeats HELLO until it sees the acknowledgement.
                self.actions.push(Action::SendHelloAck {
                    peer,
                    session_id: existing_id,
                    token,
                    caps: self.config.host_caps,
                });
                self.touch(now);
                return;
            }
            if self.config.pin_peer {
                self.reject(peer, RejectReason::Busy);
                return;
            }
            self.release(false);
            self.stream = None;
        }

        let session_id = self.ids.next_id();
        let caps = packet.flags & CAP_MASK;
        self.stream = Some(Stream {
            session_id,
            peer,
            token,
            caps,
            last_packet: now,
            released: false,
            seq: SeqWindow::default(),
        });
        self.stats.record_session();
        self.actions.push(Action::SendHelloAck {
            peer,
            session_id,
            token,
            caps: self.config.host_caps,
        });
    }

    fn on_sample(&mut self, peer: SocketAddr, packet: &Packet, now: Instant) {
        let Some((_session_id, caps)) = self.active(peer, packet) else {
            return;
        };

        match self.accept_seq(packet.seq) {
            SeqResult::Fresh => {}
            SeqResult::Duplicate => {
                self.stats.record_duplicate();
                return;
            }
            SeqResult::Stale => {
                self.stats.record_stale();
                return;
            }
        }
        self.touch(now);

        if packet.flags & flags::PREDICTED != 0 {
            self.stats.record_predicted();
            return;
        }

        if packet.flags & flags::OUT_OF_RANGE != 0 {
            self.stats.record_out_of_range();
            self.release(false);
            return;
        }

        if packet.flags & flags::CONTACT != 0 && packet.flags & flags::HOVER != 0 {
            self.stats.record_ignored();
            return;
        }

        let sample = self.sample_from(packet, caps);
        let mapped = self.mapping.map(&sample);
        self.stats.record_sample();
        self.actions.push(Action::Inject(mapped));
    }

    fn on_heartbeat(&mut self, peer: SocketAddr, packet: &Packet, now: Instant) {
        let Some((session_id, _caps)) = self.active(peer, packet) else {
            return;
        };
        self.keepalive(now);

        let host_now = self.host_us(now);
        let echoed_host = u32::from(packet.pressure) | (u32::from(packet.altitude) << 16);
        if echoed_host != 0 {
            let rtt = host_now.wrapping_sub(echoed_host);
            let midpoint = echoed_host.wrapping_add(rtt / 2);
            let offset = i64::from(packet.device_time_us) - i64::from(midpoint);
            self.stats.record_timing(rtt, offset);
        }

        let mut ack = Packet::new(PacketType::HeartbeatAck);
        ack.seq = packet.seq;
        ack.session_id = session_id;
        ack.device_time_us = host_now;
        ack.x = (packet.device_time_us & 0xFFFF) as u16;
        ack.y = (packet.device_time_us >> 16) as u16;
        ack.pressure = (host_now & 0xFFFF) as u16;
        ack.altitude = (host_now >> 16) as u16;

        self.stats.record_heartbeat();
        self.actions
            .push(Action::SendHeartbeatAck { peer, packet: ack });
    }

    fn on_bye(&mut self, peer: SocketAddr, packet: &Packet) {
        let Some(session_id) = self.session_id() else {
            self.stats.record_ignored();
            return;
        };
        let peer_matches = self
            .stream
            .as_ref()
            .is_some_and(|stream| stream.peer == peer);

        if !peer_matches || packet.session_id != session_id {
            self.stats.record_wrong_session();
            self.actions.push(Action::Rejected {
                peer,
                reason: RejectReason::WrongSession,
            });
            return;
        }
        self.release(false);
        self.stream = None;
    }

    fn active(&mut self, peer: SocketAddr, packet: &Packet) -> Option<(u32, u16)> {
        let Some(stream) = &self.stream else {
            self.reject(peer, RejectReason::NotStreaming);
            return None;
        };
        let peer_matches = !self.config.pin_peer || stream.peer == peer;
        if !peer_matches || packet.session_id != stream.session_id {
            self.stats.record_wrong_session();
            self.actions.push(Action::Rejected {
                peer,
                reason: RejectReason::WrongSession,
            });
            return None;
        }
        Some((stream.session_id, stream.caps))
    }

    fn accept_seq(&mut self, seq: u16) -> SeqResult {
        self.stream
            .as_mut()
            .map_or(SeqResult::Stale, |stream| stream.seq.accept(seq))
    }

    fn touch(&mut self, now: Instant) {
        if let Some(stream) = &mut self.stream {
            stream.last_packet = now;
            stream.released = false;
        }
    }

    /// Keep the session alive without clearing the watchdog release.
    ///
    /// Only a pointer sample means the input is active again; an idle
    /// heartbeat must not re-arm the watchdog and release input once per
    /// heartbeat period.
    fn keepalive(&mut self, now: Instant) {
        if let Some(stream) = &mut self.stream {
            stream.last_packet = now;
        }
    }

    fn sample_from(&self, packet: &Packet, caps: u16) -> TabletSample {
        let pressure = if caps & flags::CAP_PRESSURE != 0 {
            packet.pressure
        } else {
            self.config.pressure_default
        };

        let tilt = if caps & flags::CAP_TILT != 0 && packet.flags & flags::FINGER == 0 {
            Some(Tilt::from_angles(packet.altitude, packet.azimuth))
        } else {
            None
        };

        TabletSample {
            x: packet.x,
            y: packet.y,
            pressure,
            tilt,
            twist: packet.twist,
            contact: packet.flags & flags::CONTACT != 0,
            hover: packet.flags & flags::HOVER != 0,
            eraser: packet.flags & flags::ERASER != 0,
            barrel: packet.flags & flags::BARREL != 0,
            finger: packet.flags & flags::FINGER != 0,
            out_of_range: false,
        }
    }

    fn reject(&mut self, peer: SocketAddr, reason: RejectReason) {
        self.stats.record_ignored();
        self.actions.push(Action::Rejected { peer, reason });
    }

    fn host_us(&self, now: Instant) -> u32 {
        now.saturating_duration_since(self.started).as_micros() as u32
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::time::{Duration, Instant};

    use super::{
        Action, FixedIds, HOST_CAPS, RejectReason, SeqResult, SeqWindow, Session, SessionConfig,
    };
    use crate::mapping::{Mapping, Rect};
    use crate::protocol::packet::Packet;
    use crate::protocol::types::{PacketType, flags};

    const TOKEN: [u8; 16] = [0x5A; 16];

    fn peer() -> SocketAddr {
        "127.0.0.1:41000".parse().expect("peer address")
    }

    fn other_peer() -> SocketAddr {
        "127.0.0.1:41001".parse().expect("peer address")
    }

    fn session(started: Instant) -> Session<FixedIds> {
        Session::with_ids(
            SessionConfig::default(),
            Mapping::full(),
            started,
            FixedIds(7),
        )
    }

    fn session_with_token(started: Instant) -> Session<FixedIds> {
        let config = SessionConfig {
            token: Some(TOKEN),
            ..SessionConfig::default()
        };
        Session::with_ids(config, Mapping::full(), started, FixedIds(7))
    }

    fn hello(token: [u8; 16]) -> Packet {
        let mut packet = Packet::new(PacketType::Hello);
        packet.set_token(token);
        packet.flags = flags::CAP_TILT | flags::CAP_HOVER;
        packet
    }

    fn sample(seq: u16, x: u16, y: u16) -> Packet {
        let mut packet = Packet::new(PacketType::Sample);
        packet.seq = seq;
        packet.session_id = 7;
        packet.x = x;
        packet.y = y;
        packet.pressure = 12_345;
        packet.flags = flags::CONTACT;
        packet
    }

    fn heartbeat(seq: u16) -> Packet {
        let mut packet = Packet::new(PacketType::Heartbeat);
        packet.seq = seq;
        packet.session_id = 7;
        packet
    }

    fn handshake(session: &mut Session<FixedIds>, started: Instant) {
        session.handle(peer(), &hello(TOKEN), started);
        let actions = session.take_actions();
        assert!(matches!(actions.as_slice(), [Action::SendHelloAck { .. }]));
    }

    #[test]
    fn hello_is_acknowledged_with_an_assigned_session() {
        let started = Instant::now();
        let mut session = session(started);

        session.handle(peer(), &hello(TOKEN), started);

        assert!(session.is_streaming());
        assert_eq!(session.session_id(), Some(7));
        assert_eq!(
            session.take_actions(),
            vec![Action::SendHelloAck {
                peer: peer(),
                session_id: 7,
                token: TOKEN,
                caps: HOST_CAPS,
            }]
        );
        assert_eq!(session.stats().sessions, 1);
    }

    #[test]
    fn repeated_hello_reuses_the_session() {
        let started = Instant::now();
        let mut session = session(started);
        handshake(&mut session, started);
        session.take_actions();

        session.handle(peer(), &hello(TOKEN), started + Duration::from_millis(100));

        assert_eq!(
            session.take_actions(),
            vec![Action::SendHelloAck {
                peer: peer(),
                session_id: 7,
                token: TOKEN,
                caps: HOST_CAPS,
            }]
        );
        assert_eq!(session.stats().sessions, 1);
    }

    #[test]
    fn bad_token_is_rejected() {
        let started = Instant::now();
        let mut session = session_with_token(started);

        session.handle(peer(), &hello([0x11; 16]), started);

        assert!(!session.is_streaming());
        assert_eq!(
            session.take_actions(),
            vec![Action::Rejected {
                peer: peer(),
                reason: RejectReason::BadToken,
            }]
        );
    }

    #[test]
    fn a_second_peer_cannot_take_over() {
        let started = Instant::now();
        let mut session = session(started);
        handshake(&mut session, started);
        session.take_actions();

        session.handle(other_peer(), &hello(TOKEN), started);

        assert_eq!(session.session_id(), Some(7));
        assert_eq!(
            session.take_actions(),
            vec![Action::Rejected {
                peer: other_peer(),
                reason: RejectReason::Busy,
            }]
        );
    }

    #[test]
    fn sample_before_hello_is_rejected() {
        let started = Instant::now();
        let mut session = session(started);

        session.handle(peer(), &sample(1, 10, 20), started);

        assert_eq!(
            session.take_actions(),
            vec![Action::Rejected {
                peer: peer(),
                reason: RejectReason::NotStreaming,
            }]
        );
    }

    #[test]
    fn sample_is_mapped_and_injected() {
        let started = Instant::now();
        let mut session = session(started);
        handshake(&mut session, started);

        session.handle(peer(), &sample(1, u16::MAX, 0), started);

        let actions = session.take_actions();
        match actions.as_slice() {
            [Action::Inject(mapped)] => {
                assert_eq!(mapped.x, u16::MAX);
                assert_eq!(mapped.y, 0);
                assert!(mapped.contact);
            }
            other => panic!("unexpected actions: {other:?}"),
        }
        assert_eq!(session.stats().samples, 1);
    }

    #[test]
    fn wrong_session_id_is_dropped() {
        let started = Instant::now();
        let mut session = session(started);
        handshake(&mut session, started);
        session.take_actions();

        let mut packet = sample(1, 1, 1);
        packet.session_id = 99;
        session.handle(peer(), &packet, started);

        assert_eq!(
            session.take_actions(),
            vec![Action::Rejected {
                peer: peer(),
                reason: RejectReason::WrongSession,
            }]
        );
        assert_eq!(session.stats().wrong_session, 1);
    }

    #[test]
    fn packets_from_another_peer_are_dropped() {
        let started = Instant::now();
        let mut session = session(started);
        handshake(&mut session, started);
        session.take_actions();

        session.handle(other_peer(), &sample(1, 1, 1), started);

        assert_eq!(session.stats().wrong_session, 1);
        assert_eq!(
            session.take_actions(),
            vec![Action::Rejected {
                peer: other_peer(),
                reason: RejectReason::WrongSession,
            }]
        );
    }

    #[test]
    fn duplicate_sequences_are_not_injected() {
        let started = Instant::now();
        let mut session = session(started);
        handshake(&mut session, started);

        session.handle(peer(), &sample(1, 1, 1), started);
        session.handle(peer(), &sample(1, 2, 2), started);

        assert_eq!(session.take_actions().len(), 1);
        assert_eq!(session.stats().duplicates, 1);
    }

    #[test]
    fn predicted_samples_are_skipped() {
        let started = Instant::now();
        let mut session = session(started);
        handshake(&mut session, started);

        let mut packet = sample(1, 5, 5);
        packet.flags = flags::CONTACT | flags::PREDICTED;
        session.handle(peer(), &packet, started);

        assert!(session.take_actions().is_empty());
        assert_eq!(session.stats().predicted_skipped, 1);
    }

    #[test]
    fn pressure_falls_back_when_the_ipad_lacks_the_capability() {
        let started = Instant::now();
        let mut session = session(started);
        handshake(&mut session, started);

        session.handle(peer(), &sample(1, 1, 1), started);

        match session.take_actions().as_slice() {
            [Action::Inject(mapped)] => assert_eq!(mapped.pressure, 32_768),
            other => panic!("unexpected actions: {other:?}"),
        }
    }

    #[test]
    fn reported_pressure_is_kept() {
        let started = Instant::now();
        let mut session = session(started);
        let mut hello = hello(TOKEN);
        hello.flags = flags::CAP_PRESSURE;
        session.handle(peer(), &hello, started);
        session.take_actions();

        session.handle(peer(), &sample(1, 1, 1), started);

        match session.take_actions().as_slice() {
            [Action::Inject(mapped)] => assert_eq!(mapped.pressure, 12_345),
            other => panic!("unexpected actions: {other:?}"),
        }
    }

    #[test]
    fn host_advertises_pressure_tilt_and_hover() {
        assert_eq!(
            HOST_CAPS,
            flags::CAP_PRESSURE | flags::CAP_TILT | flags::CAP_HOVER
        );
    }

    #[test]
    fn tilt_is_projected_when_the_ipad_supports_it() {
        let started = Instant::now();
        let mut session = session(started);
        handshake(&mut session, started);

        let mut packet = sample(1, 1, 1);
        packet.altitude = 0;
        packet.azimuth = 0;
        session.handle(peer(), &packet, started);

        match session.take_actions().as_slice() {
            [Action::Inject(mapped)] => {
                assert_eq!(mapped.tilt, Some(crate::mapping::Tilt { x: 90, y: 0 }));
            }
            other => panic!("unexpected actions: {other:?}"),
        }
    }

    #[test]
    fn tilt_is_absent_without_the_capability_or_for_a_finger() {
        let started = Instant::now();
        let mut session = session(started);
        let mut hello = hello(TOKEN);
        hello.flags = flags::CAP_HOVER;
        session.handle(peer(), &hello, started);
        session.take_actions();

        session.handle(peer(), &sample(1, 1, 1), started);
        match session.take_actions().as_slice() {
            [Action::Inject(mapped)] => assert_eq!(mapped.tilt, None),
            other => panic!("unexpected actions: {other:?}"),
        }

        let mut finger = sample(2, 2, 2);
        finger.flags = flags::CONTACT | flags::FINGER;
        session.handle(peer(), &finger, started);
        match session.take_actions().as_slice() {
            [Action::Inject(mapped)] => assert_eq!(mapped.tilt, None),
            other => panic!("unexpected actions: {other:?}"),
        }
    }

    #[test]
    fn out_of_range_releases_input() {
        let started = Instant::now();
        let mut session = session(started);
        handshake(&mut session, started);

        let mut packet = sample(1, 1, 1);
        packet.flags = flags::OUT_OF_RANGE;
        session.handle(peer(), &packet, started);

        assert_eq!(session.take_actions(), vec![Action::ReleaseAll]);
        assert!(session.is_released());
    }

    #[test]
    fn hover_and_contact_together_are_dropped() {
        let started = Instant::now();
        let mut session = session(started);
        handshake(&mut session, started);

        let mut packet = sample(1, 1, 1);
        packet.flags = flags::CONTACT | flags::HOVER;
        session.handle(peer(), &packet, started);

        assert!(session.take_actions().is_empty());
        assert_eq!(session.stats().ignored, 1);
    }

    #[test]
    fn watchdog_releases_once_but_keeps_the_session() {
        let started = Instant::now();
        let mut session = session(started);
        handshake(&mut session, started);
        session.handle(peer(), &sample(1, 1, 1), started);
        session.take_actions();

        session.tick(started + Duration::from_millis(700));
        assert!(session.take_actions().is_empty());

        session.tick(started + Duration::from_millis(800));
        assert_eq!(session.take_actions(), vec![Action::ReleaseAll]);
        assert!(session.is_streaming());
        assert!(session.is_released());
        assert_eq!(session.stats().watchdog_releases, 1);

        session.tick(started + Duration::from_millis(900));
        assert!(session.take_actions().is_empty());
    }

    #[test]
    fn heartbeat_keeps_the_session_alive_without_repeating_the_release() {
        let started = Instant::now();
        let mut session = session(started);
        handshake(&mut session, started);
        session.handle(peer(), &sample(1, 1, 1), started);
        session.take_actions();
        session.tick(started + Duration::from_millis(800));
        assert_eq!(session.take_actions(), vec![Action::ReleaseAll]);

        session.handle(
            peer(),
            &heartbeat(2),
            started + Duration::from_millis(1_000),
        );
        assert!(matches!(
            session.take_actions().as_slice(),
            [Action::SendHeartbeatAck { .. }]
        ));

        // The heartbeat kept the session alive, but only a sample may clear
        // the watchdog release.
        session.tick(started + Duration::from_millis(1_800));
        assert!(session.take_actions().is_empty());
        assert!(session.is_streaming());
        assert!(session.is_released());
        assert_eq!(session.stats().watchdog_releases, 1);

        session.handle(
            peer(),
            &heartbeat(3),
            started + Duration::from_millis(3_000),
        );
        session.handle(
            peer(),
            &sample(2, 5, 5),
            started + Duration::from_millis(3_100),
        );
        assert!(!session.is_released());
        assert!(
            session
                .take_actions()
                .iter()
                .any(|action| matches!(action, Action::Inject(_)))
        );
    }

    #[test]
    fn a_new_sample_after_the_watchdog_is_injected() {
        let started = Instant::now();
        let mut session = session(started);
        handshake(&mut session, started);
        session.tick(started + Duration::from_millis(800));
        session.take_actions();

        session.handle(
            peer(),
            &sample(1, 1, 1),
            started + Duration::from_millis(900),
        );

        assert_eq!(session.take_actions().len(), 1);
        assert!(!session.is_released());
    }

    #[test]
    fn session_timeout_ends_the_session() {
        let started = Instant::now();
        let mut session = session(started);
        handshake(&mut session, started);

        session.tick(started + Duration::from_millis(5_000));

        assert!(!session.is_streaming());
        assert_eq!(session.take_actions(), vec![Action::ReleaseAll]);
        assert_eq!(session.stats().timeouts, 1);
    }

    #[test]
    fn bye_releases_and_ends_the_session() {
        let started = Instant::now();
        let mut session = session(started);
        handshake(&mut session, started);

        let mut packet = Packet::new(PacketType::Bye);
        packet.session_id = 7;
        session.handle(peer(), &packet, started);

        assert!(!session.is_streaming());
        assert_eq!(session.take_actions(), vec![Action::ReleaseAll]);
    }

    #[test]
    fn heartbeat_is_acknowledged_and_timed() {
        let started = Instant::now();
        let mut session = session(started);
        handshake(&mut session, started);
        session.take_actions();

        let mut packet = Packet::new(PacketType::Heartbeat);
        packet.seq = 3;
        packet.session_id = 7;
        packet.device_time_us = 5_000_000;
        let echoed_host: u32 = 90_000;
        packet.pressure = (echoed_host & 0xFFFF) as u16;
        packet.altitude = (echoed_host >> 16) as u16;

        session.handle(peer(), &packet, started + Duration::from_millis(100));

        match session.take_actions().as_slice() {
            [Action::SendHeartbeatAck { packet: ack, .. }] => {
                assert_eq!(ack.packet_type, PacketType::HeartbeatAck);
                assert_eq!(ack.session_id, 7);
                assert_eq!(ack.seq, 3);
                assert_eq!(ack.x, (0x004C_4B40_u32 & 0xFFFF) as u16);
                assert_eq!(ack.y, (0x004C_4B40_u32 >> 16) as u16);
                assert_eq!(ack.device_time_us, 100_000);
            }
            other => panic!("unexpected actions: {other:?}"),
        }
        assert_eq!(session.stats().last_rtt_us, Some(10_000));
        assert_eq!(session.stats().last_offset_us, Some(4_905_000));
    }

    #[test]
    fn heartbeat_before_hello_is_rejected() {
        let started = Instant::now();
        let mut session = session(started);
        let mut packet = Packet::new(PacketType::Heartbeat);
        packet.session_id = 7;

        session.handle(peer(), &packet, started);

        assert_eq!(
            session.take_actions(),
            vec![Action::Rejected {
                peer: peer(),
                reason: RejectReason::NotStreaming,
            }]
        );
    }

    #[test]
    fn sequence_window_accepts_reordering_inside_the_window() {
        let mut window = SeqWindow::default();

        assert_eq!(window.accept(10), SeqResult::Fresh);
        assert_eq!(window.accept(12), SeqResult::Fresh);
        assert_eq!(window.accept(11), SeqResult::Fresh);
        assert_eq!(window.accept(11), SeqResult::Duplicate);
        assert_eq!(window.accept(10), SeqResult::Duplicate);
    }

    #[test]
    fn sequence_window_rejects_old_packets() {
        let mut window = SeqWindow::default();

        assert_eq!(window.accept(1_000), SeqResult::Fresh);
        assert_eq!(window.accept(1_065), SeqResult::Fresh);
        assert_eq!(window.accept(999), SeqResult::Stale);
    }

    #[test]
    fn sequence_window_handles_wraparound() {
        let mut window = SeqWindow::default();

        assert_eq!(window.accept(u16::MAX), SeqResult::Fresh);
        assert_eq!(window.accept(0), SeqResult::Fresh);
        assert_eq!(window.accept(1), SeqResult::Fresh);
        assert_eq!(window.accept(0), SeqResult::Duplicate);
    }

    #[test]
    fn mapping_target_is_applied_to_injected_samples() {
        let started = Instant::now();
        let mapping = Mapping::new(
            Rect {
                x: 0,
                y: 0,
                width: 1_000,
                height: 1_000,
            },
            crate::mapping::AspectMode::Stretch,
            None,
            None,
        );
        let mut session =
            Session::with_ids(SessionConfig::default(), mapping, started, FixedIds(7));
        handshake(&mut session, started);

        session.handle(peer(), &sample(1, u16::MAX, u16::MAX), started);

        match session.take_actions().as_slice() {
            [Action::Inject(mapped)] => {
                assert_eq!(mapped.x, 1_000);
                assert_eq!(mapped.y, 1_000);
            }
            other => panic!("unexpected actions: {other:?}"),
        }
    }
}
