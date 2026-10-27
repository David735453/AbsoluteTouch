//! Session and transport counters.

use std::fmt;

/// Counters collected by the transport and session layers.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    /// Valid packets received from any peer.
    pub packets_received: u64,
    /// Datagrams that failed protocol decoding.
    pub protocol_errors: u64,
    /// Packets dropped because the session identifier did not match.
    pub wrong_session: u64,
    /// Duplicate sequence numbers.
    pub duplicates: u64,
    /// Sequence numbers outside the reorder window.
    pub stale: u64,
    /// Packets that were valid but irrelevant in the current state.
    pub ignored: u64,
    /// Samples injected.
    pub samples: u64,
    /// Predicted samples skipped by policy.
    pub predicted_skipped: u64,
    /// Out-of-range samples that released input.
    pub out_of_range: u64,
    /// Heartbeats answered.
    pub heartbeats: u64,
    /// Release calls issued to the injector.
    pub releases: u64,
    /// Watchdog releases.
    pub watchdog_releases: u64,
    /// Sessions that ended because of the session timeout.
    pub timeouts: u64,
    /// Sessions accepted.
    pub sessions: u64,
    /// Received packets dropped because the queue was full.
    pub channel_drops: u64,
    /// Injector errors.
    pub inject_errors: u64,
    /// Most recent heartbeat round-trip time in microseconds.
    pub last_rtt_us: Option<u32>,
    /// Most recent clock-offset estimate in microseconds.
    pub last_offset_us: Option<i64>,
    /// Most recent injected-sample age above the session floor, in microseconds.
    pub last_sample_age_us: Option<u32>,
    /// Largest injected-sample age above the session floor, in microseconds.
    pub worst_sample_age_us: u32,
    /// Fastest injected-sample age seen this session, in microseconds.
    ///
    /// The heartbeat offset is only accurate to the round-trip asymmetry, so an
    /// absolute age carries a constant unknown bias. Keeping the fastest age
    /// and reporting the excess cancels that bias.
    floor_sample_age_us: Option<i32>,
}

impl fmt::Display for Stats {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            formatter,
            "packets: {} received, {} protocol errors, {} wrong session, {} duplicates, {} stale, {} ignored",
            self.packets_received,
            self.protocol_errors,
            self.wrong_session,
            self.duplicates,
            self.stale,
            self.ignored,
        )?;
        writeln!(
            formatter,
            "samples: {} injected, {} predicted skipped, {} out of range",
            self.samples, self.predicted_skipped, self.out_of_range,
        )?;
        writeln!(
            formatter,
            "sessions: {} accepted, {} timeouts, {} releases ({} watchdog), {} inject errors",
            self.sessions, self.timeouts, self.releases, self.watchdog_releases, self.inject_errors,
        )?;
        writeln!(
            formatter,
            "queue drops: {}, heartbeats: {}",
            self.channel_drops, self.heartbeats,
        )?;
        if let Some(rtt) = self.last_rtt_us {
            writeln!(formatter, "last RTT: {rtt} us")?;
        }
        if let Some(offset) = self.last_offset_us {
            writeln!(formatter, "last clock offset: {offset} us")?;
        }
        if let Some(age) = self.last_sample_age_us {
            writeln!(
                formatter,
                "input latency: {age} us last, {} us worst above floor",
                self.worst_sample_age_us,
            )?;
        }
        Ok(())
    }
}

impl Stats {
    /// Record one valid packet.
    pub const fn record_packet(&mut self) {
        self.packets_received += 1;
    }

    /// Record a rejected packet.
    pub const fn record_protocol_error(&mut self) {
        self.protocol_errors += 1;
    }

    /// Record a packet for a different session.
    pub const fn record_wrong_session(&mut self) {
        self.wrong_session += 1;
    }

    /// Record a duplicate sequence number.
    pub const fn record_duplicate(&mut self) {
        self.duplicates += 1;
    }

    /// Record a stale sequence number.
    pub const fn record_stale(&mut self) {
        self.stale += 1;
    }

    /// Record an ignored packet.
    pub const fn record_ignored(&mut self) {
        self.ignored += 1;
    }

    /// Record an injected sample.
    pub const fn record_sample(&mut self) {
        self.samples += 1;
    }

    /// Record a skipped predicted sample.
    pub const fn record_predicted(&mut self) {
        self.predicted_skipped += 1;
    }

    /// Record an out-of-range sample.
    pub const fn record_out_of_range(&mut self) {
        self.out_of_range += 1;
    }

    /// Record an answered heartbeat.
    pub const fn record_heartbeat(&mut self) {
        self.heartbeats += 1;
    }

    /// Record a release sent to the injector.
    pub const fn record_release(&mut self) {
        self.releases += 1;
    }

    /// Record a watchdog release.
    pub const fn record_watchdog_release(&mut self) {
        self.watchdog_releases += 1;
    }

    /// Record a session timeout.
    pub const fn record_timeout(&mut self) {
        self.timeouts += 1;
    }

    /// Record an accepted session.
    pub const fn record_session(&mut self) {
        self.sessions += 1;
    }

    /// Record a transport queue drop.
    pub const fn record_channel_drop(&mut self) {
        self.channel_drops += 1;
    }

    /// Add a batch of transport queue drops.
    pub const fn add_channel_drops(&mut self, count: u64) {
        self.channel_drops += count;
    }

    /// Record an injector error.
    pub const fn record_inject_error(&mut self) {
        self.inject_errors += 1;
    }

    /// Store the latest heartbeat estimate.
    pub const fn record_timing(&mut self, rtt_us: u32, offset_us: i64) {
        self.last_rtt_us = Some(rtt_us);
        self.last_offset_us = Some(offset_us);
    }

    /// Largest sample age accepted as a measurement, in microseconds.
    ///
    /// Anything further from the session floor is a clock jump or a sample from
    /// another basestation, not input the user is waiting for.
    pub const MAX_SAMPLE_AGE_US: i32 = 1_000_000;

    /// Record the age of one injected sample, relative to the session floor.
    ///
    /// The heartbeat offset is only as accurate as the round-trip asymmetry, so
    /// an absolute age carries a bias that is constant for the whole session.
    /// Comparing each sample with the fastest sample seen so far cancels that
    /// bias and leaves the delay the user actually feels, which is also what
    /// `analyze.py` reports for a recorded trace.
    ///
    /// # Panics
    ///
    /// Never panics; ages more than [`Self::MAX_SAMPLE_AGE_US`] away from the
    /// floor are ignored.
    pub const fn record_sample_age(&mut self, age_us: i32) {
        if age_us > Self::MAX_SAMPLE_AGE_US || age_us < -Self::MAX_SAMPLE_AGE_US {
            return;
        }
        let floor = match self.floor_sample_age_us {
            Some(floor) if floor <= age_us => floor,
            _ => age_us,
        };
        self.floor_sample_age_us = Some(floor);
        let excess = (age_us - floor) as u32;
        self.last_sample_age_us = Some(excess);
        if excess > self.worst_sample_age_us {
            self.worst_sample_age_us = excess;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Stats;

    #[test]
    fn display_includes_the_important_counters() {
        let mut stats = Stats::default();
        stats.record_packet();
        stats.record_sample();
        stats.record_timing(1_500, -250);
        stats.record_sample_age(2_000);
        stats.record_sample_age(4_000);

        let text = stats.to_string();

        assert!(text.contains("1 received"));
        assert!(text.contains("1 injected"));
        assert!(text.contains("1500 us"));
        assert!(text.contains("-250 us"));
        assert!(text.contains("input latency: 2000 us last, 2000 us worst above floor"));
    }

    #[test]
    fn sample_age_is_reported_above_the_fastest_sample() {
        let mut stats = Stats::default();
        stats.record_sample_age(3_000);
        assert_eq!(stats.last_sample_age_us, Some(0));

        stats.record_sample_age(1_000);
        assert_eq!(stats.last_sample_age_us, Some(0));

        stats.record_sample_age(9_000);
        assert_eq!(stats.last_sample_age_us, Some(8_000));
        assert_eq!(stats.worst_sample_age_us, 8_000);

        // Clock noise can put a sample below the floor; it lowers the floor
        // instead of wrapping around.
        stats.record_sample_age(-2_000);
        assert_eq!(stats.last_sample_age_us, Some(0));

        stats.record_sample_age(3_000);
        assert_eq!(stats.last_sample_age_us, Some(5_000));
        assert_eq!(stats.worst_sample_age_us, 8_000);
    }

    #[test]
    fn nonsense_sample_ages_are_ignored() {
        let mut stats = Stats::default();
        stats.record_sample_age(1_500);
        stats.record_sample_age(Stats::MAX_SAMPLE_AGE_US + 1);
        stats.record_sample_age(-Stats::MAX_SAMPLE_AGE_US - 1);

        assert_eq!(stats.last_sample_age_us, Some(0));
        assert_eq!(stats.worst_sample_age_us, 0);
    }

    #[test]
    fn counters_start_at_zero() {
        assert_eq!(Stats::default(), Stats::default());
        assert_eq!(Stats::default().packets_received, 0);
    }
}
