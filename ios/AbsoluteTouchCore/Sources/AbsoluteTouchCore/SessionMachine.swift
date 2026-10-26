/// Handshake and streaming state machine for the iPad sender.
///
/// The machine has no clock of its own: every event takes a monotonic `Duration`
/// from the caller, which keeps every rule deterministic in tests. It also has
/// no transport; outgoing packets are queued and drained with
/// ``takeOutbound()``. ``SessionController`` owns both the clock and the
/// transport.
public struct SessionMachine: Sendable {
	/// Current state.
	public private(set) var state: SessionState

	/// Counters collected so far.
	public private(set) var statistics: SessionStatistics

	private let configuration: SessionConfiguration
	private var origin: Duration?
	private var sequence: UInt16 = 0
	private var lastHello: Duration?
	private var lastHeartbeat: Duration?
	private var lastInbound: Duration?
	private var lastAcknowledgedDeviceTime: UInt32 = 0
	private var lastAcknowledgedHostTime: UInt32 = 0
	private var outbound: [Packet] = []

	/// Create an idle machine.
	///
	/// - Parameter configuration: The session policy.
	public init(configuration: SessionConfiguration) {
		self.configuration = configuration
		self.state = .idle
		self.statistics = SessionStatistics()
	}

	/// Begin a session and queue the first `HELLO`.
	///
	/// Starting an active session is ignored.
	///
	/// - Parameter now: Monotonic time supplied by the caller.
	public mutating func start(now: Duration) {
		switch state {
		case .idle, .failed:
			break
		case .connecting, .streaming:
			return
		}

		origin = now
		lastHello = nil
		lastHeartbeat = nil
		lastInbound = nil
		lastAcknowledgedDeviceTime = 0
		lastAcknowledgedHostTime = 0
		state = .connecting(attempt: 1)
		enqueueHello(now: now)
	}

	/// Advance timers without an inbound packet.
	///
	/// - Parameter now: Monotonic time supplied by the caller.
	public mutating func tick(now: Duration) {
		guard origin != nil else {
			return
		}

		switch state {
		case .connecting(let attempt):
			tickConnecting(attempt: attempt, now: now)
		case .streaming(let sessionID, _):
			tickStreaming(sessionID: sessionID, now: now)
		case .idle, .failed:
			return
		}
	}

	/// Handle one decoded packet from the desktop.
	///
	/// - Parameters:
	///   - packet: The decoded packet.
	///   - now: Monotonic time supplied by the caller.
	public mutating func receive(_ packet: Packet, now: Duration) {
		guard origin != nil else {
			return
		}

		switch state {
		case .connecting:
			receiveConnecting(packet, now: now)
		case .streaming(let sessionID, _):
			receiveStreaming(packet, sessionID: sessionID, now: now)
		case .idle, .failed:
			return
		}
	}

	/// Queue a `SAMPLE` for the active session.
	///
	/// Samples submitted while connecting or stopped are dropped.
	///
	/// - Parameters:
	///   - sample: The pointer state.
	///   - now: Monotonic time supplied by the caller.
	public mutating func submit(_ sample: PointerSample, now: Duration) {
		guard case .streaming(let sessionID, _) = state else {
			return
		}

		var packet = Packet(type: .sample)
		packet.flags = sample.flags
		packet.sequence = nextSequence()
		packet.sessionID = sessionID
		packet.deviceTimeMicroseconds = microseconds(now)
		packet.x = sample.x
		packet.y = sample.y
		packet.pressure = sample.pressure
		packet.altitude = sample.altitude
		packet.azimuth = sample.azimuth
		outbound.append(packet)
		statistics.samplesSent += 1
	}

	/// Send `BYE` and return to idle.
	///
	/// - Parameter now: Monotonic time supplied by the caller.
	public mutating func stop(now: Duration) {
		guard case .streaming(let sessionID, _) = state else {
			state = .idle
			return
		}

		var packet = Packet(type: .bye)
		packet.sequence = sequence
		packet.sessionID = sessionID
		packet.deviceTimeMicroseconds = microseconds(now)
		outbound.append(packet)
		state = .idle
		lastInbound = nil
		lastHeartbeat = nil
	}

	/// Record a terminal transport failure.
	///
	/// - Parameter failure: The failure to report.
	public mutating func fail(_ failure: SessionFailure) {
		state = .failed(failure)
		lastInbound = nil
		lastHeartbeat = nil
	}

	/// Remove and return every queued outgoing packet.
	///
	/// - Returns: Packets in the order they were produced.
	@discardableResult
	public mutating func takeOutbound() -> [Packet] {
		let packets = outbound
		outbound.removeAll(keepingCapacity: true)
		return packets
	}

	private mutating func receiveConnecting(_ packet: Packet, now: Duration) {
		guard packet.type == .helloAck,
			packet.sessionID != 0,
			packet.token == configuration.token.bytes
		else {
			return
		}
		accept(packet, now: now)
	}

	private mutating func receiveStreaming(
		_ packet: Packet,
		sessionID: UInt32,
		now: Duration
	) {
		guard packet.sessionID == sessionID else {
			return
		}
		switch packet.type {
		case .helloAck:
			// A repeated HELLO can be acknowledged more than once. Treat it as
			// liveness but do not reset the stream.
			guard packet.token == configuration.token.bytes else {
				return
			}
			lastInbound = now
		case .heartbeatAck:
			lastInbound = now
			statistics.heartbeatsReceived += 1
			recordTiming(packet, now: now)
		case .bye:
			endStream()
		case .hello, .sample, .heartbeat:
			break
		}
	}

	private mutating func tickConnecting(attempt: Int, now: Duration) {
		guard let lastHello, now - lastHello >= configuration.helloInterval else {
			return
		}
		state = .connecting(attempt: attempt + 1)
		enqueueHello(now: now)
	}

	private mutating func tickStreaming(sessionID: UInt32, now: Duration) {
		if let lastInbound, now - lastInbound >= configuration.sessionTimeout {
			statistics.timeouts += 1
			beginHandshake()
			enqueueHello(now: now)
			return
		}
		guard let lastHeartbeat, now - lastHeartbeat >= configuration.heartbeatInterval
		else {
			return
		}
		enqueueHeartbeat(now: now, sessionID: sessionID)
	}

	private mutating func accept(_ packet: Packet, now: Duration) {
		state = .streaming(
			sessionID: packet.sessionID,
			hostCapabilities: packet.flags.intersection(.capabilityMask)
		)
		lastInbound = now
		lastHeartbeat = now
		statistics.sessions += 1
	}

	private mutating func beginHandshake() {
		state = .connecting(attempt: 1)
		lastInbound = nil
		lastHeartbeat = nil
		lastAcknowledgedDeviceTime = 0
		lastAcknowledgedHostTime = 0
	}

	private mutating func endStream() {
		state = .idle
		lastInbound = nil
		lastHeartbeat = nil
	}

	private mutating func enqueueHello(now: Duration) {
		var packet = Packet(type: .hello)
		packet.flags = configuration.deviceCapabilities
		packet.deviceTimeMicroseconds = microseconds(now)
		// PairingToken always carries exactly 16 bytes.
		try? packet.setToken(configuration.token.bytes)
		outbound.append(packet)
		lastHello = now
		statistics.helloSent += 1
	}

	private mutating func enqueueHeartbeat(now: Duration, sessionID: UInt32) {
		var packet = Packet(type: .heartbeat)
		packet.sequence = nextSequence()
		packet.sessionID = sessionID
		packet.deviceTimeMicroseconds = microseconds(now)
		packet.x = Self.lowWord(lastAcknowledgedDeviceTime)
		packet.y = Self.highWord(lastAcknowledgedDeviceTime)
		packet.pressure = Self.lowWord(lastAcknowledgedHostTime)
		packet.altitude = Self.highWord(lastAcknowledgedHostTime)
		outbound.append(packet)
		lastHeartbeat = now
		statistics.heartbeatsSent += 1
	}

	private mutating func recordTiming(_ packet: Packet, now: Duration) {
		let echoedDeviceTime = Self.word(low: packet.x, high: packet.y)
		let hostTime = Self.word(low: packet.pressure, high: packet.altitude)
		guard echoedDeviceTime != 0 else {
			return
		}

		let deviceNow = microseconds(now)
		let roundTrip = deviceNow &- echoedDeviceTime
		let midpoint = echoedDeviceTime &+ roundTrip / 2
		lastAcknowledgedDeviceTime = deviceNow
		lastAcknowledgedHostTime = hostTime
		statistics.lastRoundTripMicroseconds = roundTrip
		statistics.lastClockOffsetMicroseconds = Int64(hostTime) - Int64(midpoint)
	}

	private mutating func nextSequence() -> UInt16 {
		let current = sequence
		sequence &+= 1
		return current
	}

	private func microseconds(_ duration: Duration) -> UInt32 {
		let components = duration.components
		let value =
			components.seconds * 1_000_000
			+ components.attoseconds / 1_000_000_000_000
		return UInt32(truncatingIfNeeded: value)
	}

	private static func word(low: UInt16, high: UInt16) -> UInt32 {
		UInt32(low) | (UInt32(high) << 16)
	}

	private static func lowWord(_ value: UInt32) -> UInt16 {
		UInt16(truncatingIfNeeded: value)
	}

	private static func highWord(_ value: UInt32) -> UInt16 {
		UInt16(truncatingIfNeeded: value >> 16)
	}
}
