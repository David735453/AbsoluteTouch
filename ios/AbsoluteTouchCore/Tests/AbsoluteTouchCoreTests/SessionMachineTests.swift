import Testing

@testable import AbsoluteTouchCore

private let testToken = PairingToken(
	validatedBytes: (0..<16).map { UInt8($0) }
)

private func configuration(
	helloInterval: Duration = .milliseconds(500),
	heartbeatInterval: Duration = .seconds(1),
	sessionTimeout: Duration = .seconds(5)
) -> SessionConfiguration {
	SessionConfiguration(
		token: testToken,
		helloInterval: helloInterval,
		heartbeatInterval: heartbeatInterval,
		sessionTimeout: sessionTimeout
	)
}

private func helloAck(
	sessionID: UInt32 = 42,
	token: PairingToken = testToken,
	flags: PacketFlags = [.capabilityPressure, .capabilityTilt, .capabilityHover, .capabilityTwist]
) throws -> Packet {
	var packet = Packet(type: .helloAck)
	packet.sessionID = sessionID
	packet.flags = flags
	try packet.setToken(token.bytes)
	return packet
}

private func contactSample(
	x: UInt16 = 1_000,
	y: UInt16 = 2_000,
	phase: PointerPhase = .contact
) -> PointerSample {
	PointerSample(
		x: x,
		y: y,
		phase: phase,
		pressure: 12_345,
		altitude: 32_768,
		azimuth: 16_384
	)
}

private func streamingMachine(
	configuration policy: SessionConfiguration = configuration(),
	now: Duration = .zero
) throws -> SessionMachine {
	var machine = SessionMachine(configuration: policy)
	machine.start(now: now)
	_ = machine.takeOutbound()
	machine.receive(try helloAck(), now: now)
	return machine
}

/// Split a 32-bit value into the two wire words used by heartbeat payloads.
func lowWord(_ value: UInt32) -> UInt16 {
	UInt16(truncatingIfNeeded: value)
}

func highWord(_ value: UInt32) -> UInt16 {
	UInt16(truncatingIfNeeded: value >> 16)
}

func word(low: UInt16, high: UInt16) -> UInt32 {
	UInt32(low) | (UInt32(high) << 16)
}

@Suite("Session machine")
struct SessionMachineTests {
	@Test("Start queues a HELLO with the token and device capabilities")
	func startQueuesHello() {
		var machine = SessionMachine(configuration: configuration())

		machine.start(now: .zero)

		#expect(machine.state == .connecting(attempt: 1))
		let packets = machine.takeOutbound()
		#expect(packets.count == 1)
		#expect(packets[0].type == .hello)
		#expect(packets[0].flags == [.capabilityTilt, .capabilityHover])
		#expect(packets[0].token == testToken.bytes)
		#expect(packets[0].sessionID == 0)
		#expect(machine.statistics.helloSent == 1)
	}

	@Test("HELLO repeats at the configured interval")
	func helloRepeatsAtTheInterval() {
		var machine = SessionMachine(configuration: configuration())

		machine.start(now: .zero)
		_ = machine.takeOutbound()

		machine.tick(now: .milliseconds(499))
		#expect(machine.takeOutbound().isEmpty)
		#expect(machine.state == .connecting(attempt: 1))

		machine.tick(now: .milliseconds(500))
		let packets = machine.takeOutbound()
		#expect(packets.count == 1)
		#expect(packets[0].type == .hello)
		#expect(machine.state == .connecting(attempt: 2))
	}

	@Test("A matching HELLO_ACK starts the stream")
	func helloAckStartsTheStream() throws {
		var machine = SessionMachine(configuration: configuration())
		machine.start(now: .zero)
		_ = machine.takeOutbound()

		machine.receive(try helloAck(sessionID: 99), now: .zero)

		#expect(
			machine.state
				== .streaming(
					sessionID: 99,
					hostCapabilities: [
						.capabilityPressure, .capabilityTilt, .capabilityHover,
						.capabilityTwist,
					]
				)
		)
		#expect(machine.statistics.sessions == 1)
	}

	@Test("A HELLO_ACK with a wrong token is ignored")
	func wrongTokenIsIgnored() throws {
		var machine = SessionMachine(configuration: configuration())
		machine.start(now: .zero)
		_ = machine.takeOutbound()

		let wrongToken = PairingToken(validatedBytes: [UInt8](repeating: 0xAA, count: 16))
		machine.receive(try helloAck(token: wrongToken), now: .zero)

		#expect(machine.state == .connecting(attempt: 1))
		#expect(machine.takeOutbound().isEmpty)
	}

	@Test("A HELLO_ACK with a zero session is ignored")
	func zeroSessionIsIgnored() throws {
		var machine = SessionMachine(configuration: configuration())
		machine.start(now: .zero)
		_ = machine.takeOutbound()

		machine.receive(try helloAck(sessionID: 0), now: .zero)

		#expect(machine.state == .connecting(attempt: 1))
	}

	@Test("Only version 1 capability bits are kept from HELLO_ACK")
	func hostCapabilitiesAreMasked() throws {
		var machine = SessionMachine(configuration: configuration())
		machine.start(now: .zero)
		_ = machine.takeOutbound()

		let flags: PacketFlags = [.capabilityTilt, PacketFlags(rawValue: 1 << 15)]
		machine.receive(try helloAck(flags: flags), now: .zero)

		#expect(
			machine.state
				== .streaming(sessionID: 42, hostCapabilities: [.capabilityTilt])
		)
	}

	@Test("Samples are ignored until the handshake completes")
	func samplesNeedAStream() {
		var machine = SessionMachine(configuration: configuration())
		machine.start(now: .zero)
		_ = machine.takeOutbound()

		machine.submit(contactSample(), now: .zero)

		#expect(machine.takeOutbound().isEmpty)
		#expect(machine.statistics.samplesSent == 0)
	}

	@Test("Samples carry position, tilt, sequence, and session")
	func samplesAreEncoded() throws {
		var machine = try streamingMachine()
		machine.takeOutbound()

		machine.submit(contactSample(), now: .zero)
		var packets = machine.takeOutbound()
		#expect(packets.count == 1)
		let first = packets[0]
		#expect(first.type == .sample)
		#expect(first.flags == [.contact])
		#expect(first.sequence == 0)
		#expect(first.sessionID == 42)
		#expect(first.deviceTimeMicroseconds == 0)
		#expect(first.x == 1_000)
		#expect(first.y == 2_000)
		#expect(first.pressure == 12_345)
		#expect(first.altitude == 32_768)
		#expect(first.azimuth == 16_384)

		machine.submit(contactSample(), now: .milliseconds(1))
		packets = machine.takeOutbound()
		#expect(packets[0].sequence == 1)
		#expect(packets[0].deviceTimeMicroseconds == 1_000)
		#expect(machine.statistics.samplesSent == 2)
	}

	@Test("Coalesced samples are counted without changing the sequence")
	func coalescedSamplesAreCounted() throws {
		var machine = try streamingMachine()
		machine.takeOutbound()

		machine.noteCoalesced(3)
		machine.noteCoalesced(0)

		#expect(machine.statistics.samplesCoalesced == 3)
		#expect(machine.statistics.samplesSent == 0)
		#expect(machine.takeOutbound().isEmpty)
	}

	@Test("Out-of-range and finger flags are preserved")
	func outOfRangeFlagsArePreserved() throws {
		var machine = try streamingMachine()
		machine.takeOutbound()

		machine.submit(
			PointerSample(x: 1, y: 2, phase: .outOfRange, isFinger: true),
			now: .zero
		)

		#expect(machine.takeOutbound()[0].flags == [.outOfRange, .finger])
	}

	@Test("Heartbeats start after one interval and carry the last echo")
	func heartbeatsCarryTiming() throws {
		var machine = try streamingMachine()
		machine.takeOutbound()

		machine.tick(now: .milliseconds(999))
		#expect(machine.takeOutbound().isEmpty)

		machine.tick(now: .seconds(1))
		var packets = machine.takeOutbound()
		#expect(packets.count == 1)
		let heartbeat = packets[0]
		#expect(heartbeat.type == .heartbeat)
		#expect(heartbeat.sessionID == 42)
		#expect(heartbeat.sequence == 0)
		#expect(heartbeat.x == 0)
		#expect(heartbeat.y == 0)
		#expect(heartbeat.pressure == 0)
		#expect(heartbeat.altitude == 0)

		var ack = Packet(type: .heartbeatAck)
		ack.sessionID = 42
		ack.x = lowWord(heartbeat.deviceTimeMicroseconds)
		ack.y = highWord(heartbeat.deviceTimeMicroseconds)
		ack.pressure = lowWord(5_000_000)
		ack.altitude = highWord(5_000_000)
		machine.receive(ack, now: .milliseconds(1_100))

		#expect(machine.statistics.heartbeatsReceived == 1)
		#expect(machine.statistics.lastRoundTripMicroseconds == 100_000)
		#expect(machine.statistics.lastClockOffsetMicroseconds == 3_950_000)

		machine.tick(now: .seconds(2))
		packets = machine.takeOutbound()
		#expect(packets.count == 1)
		let next = packets[0]
		#expect(next.sequence == 1)
		#expect(word(low: next.x, high: next.y) == 1_100_000)
		#expect(word(low: next.pressure, high: next.altitude) == 5_000_000)
	}

	@Test("Silence for the session timeout starts a new handshake")
	func timeoutStartsANewHandshake() throws {
		var machine = try streamingMachine()
		machine.takeOutbound()

		machine.tick(now: .milliseconds(4_999))
		#expect(machine.state.isStreaming)
		_ = machine.takeOutbound()

		machine.tick(now: .seconds(5))
		#expect(machine.state == .connecting(attempt: 1))
		#expect(machine.statistics.timeouts == 1)
		let packets = machine.takeOutbound()
		#expect(packets.count == 1)
		#expect(packets[0].type == .hello)
	}

	@Test("A repeated acknowledgement refreshes liveness")
	func repeatedAckRefreshesLiveness() throws {
		var machine = try streamingMachine()
		machine.takeOutbound()

		machine.receive(try helloAck(), now: .seconds(4))
		machine.tick(now: .milliseconds(8_999))
		#expect(machine.state.isStreaming)

		machine.tick(now: .seconds(9))
		#expect(machine.state == .connecting(attempt: 1))
	}

	@Test("Packets from another session are ignored")
	func otherSessionPacketsAreIgnored() throws {
		var machine = try streamingMachine()
		machine.takeOutbound()

		var ack = Packet(type: .heartbeatAck)
		ack.sessionID = 7
		machine.receive(ack, now: .seconds(4))
		machine.tick(now: .seconds(5))

		#expect(machine.state == .connecting(attempt: 1))
	}

	@Test("A BYE from the desktop ends the stream")
	func byeEndsTheStream() throws {
		var machine = try streamingMachine()
		machine.takeOutbound()

		var bye = Packet(type: .bye)
		bye.sessionID = 42
		machine.receive(bye, now: .zero)

		#expect(machine.state == .idle)
		#expect(machine.takeOutbound().isEmpty)
	}

	@Test("Stop sends BYE while streaming")
	func stopSendsBye() throws {
		var machine = try streamingMachine()
		machine.submit(contactSample(), now: .zero)
		machine.takeOutbound()

		machine.stop(now: .milliseconds(10))

		#expect(machine.state == .idle)
		let packets = machine.takeOutbound()
		#expect(packets.count == 1)
		#expect(packets[0].type == .bye)
		#expect(packets[0].sessionID == 42)
		#expect(packets[0].deviceTimeMicroseconds == 10_000)
	}

	@Test("Stop before the handshake completes sends nothing")
	func stopWhileConnectingSendsNothing() {
		var machine = SessionMachine(configuration: configuration())
		machine.start(now: .zero)
		_ = machine.takeOutbound()

		machine.stop(now: .milliseconds(100))

		#expect(machine.state == .idle)
		#expect(machine.takeOutbound().isEmpty)
	}

	@Test("A failure is reported and sends nothing")
	func failureIsReported() {
		var machine = SessionMachine(configuration: configuration())
		machine.start(now: .zero)
		_ = machine.takeOutbound()

		machine.fail(.transport("offline"))

		#expect(machine.state == .failed(.transport("offline")))
		#expect(machine.takeOutbound().isEmpty)
	}

	@Test("The sequence wraps at 65,535")
	func sequenceWraps() throws {
		var machine = try streamingMachine()
		machine.takeOutbound()

		for _ in 0..<65_536 {
			machine.submit(contactSample(), now: .zero)
			_ = machine.takeOutbound()
		}

		machine.submit(contactSample(), now: .zero)
		#expect(machine.takeOutbound()[0].sequence == 0)
	}
}
