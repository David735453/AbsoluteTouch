import Testing

@testable import AbsoluteTouchCore

private let reconnectToken = PairingToken(
	validatedBytes: (0..<16).map { UInt8($0) }
)

private func reconnectConfiguration() -> SessionConfiguration {
	SessionConfiguration(
		token: reconnectToken,
		helloInterval: .milliseconds(500),
		heartbeatInterval: .seconds(1),
		sessionTimeout: .seconds(5)
	)
}

/// A machine that has completed the handshake and sent nothing since.
private func streamingReconnectMachine() throws -> SessionMachine {
	var machine = SessionMachine(configuration: reconnectConfiguration())
	machine.start(now: .zero)
	_ = machine.takeOutbound()

	var acknowledgement = Packet(type: .helloAck)
	acknowledgement.sessionID = 42
	try acknowledgement.setToken(reconnectToken.bytes)
	machine.receive(acknowledgement, now: .zero)
	return machine
}

@Suite("Session reconnect")
struct SessionReconnectTests {
	@Test("A lost peer starts a new handshake without counting a timeout")
	func peerLossStartsANewHandshake() throws {
		var machine = try streamingReconnectMachine()
		let sample = PointerSample(x: 1_000, y: 2_000, phase: .contact)
		machine.submit(sample, now: .milliseconds(100))

		machine.peerLost(now: .milliseconds(200))

		#expect(machine.state == .connecting(attempt: 1))
		#expect(machine.statistics.timeouts == 0)
		#expect(machine.statistics.helloSent == 2)
		// The queued sample belonged to the dead session, so only the new
		// HELLO is left.
		let packets = machine.takeOutbound()
		#expect(packets.count == 1)
		#expect(packets[0].type == .hello)
	}

	@Test("A lost peer does not disturb a handshake in progress")
	func peerLossWhileConnectingIsIgnored() throws {
		var machine = SessionMachine(configuration: reconnectConfiguration())
		machine.start(now: .zero)
		_ = machine.takeOutbound()

		machine.peerLost(now: .milliseconds(100))

		#expect(machine.state == .connecting(attempt: 1))
		#expect(machine.statistics.helloSent == 1)
		#expect(machine.takeOutbound().isEmpty)
	}

	@Test("A new desktop acknowledgement resumes after the peer is lost")
	func handshakeAfterPeerLossStreamsAgain() throws {
		var machine = try streamingReconnectMachine()
		machine.peerLost(now: .milliseconds(200))
		_ = machine.takeOutbound()

		var acknowledgement = Packet(type: .helloAck)
		acknowledgement.sessionID = 43
		try acknowledgement.setToken(reconnectToken.bytes)
		machine.receive(acknowledgement, now: .milliseconds(300))

		#expect(
			machine.state
				== .streaming(sessionID: 43, hostCapabilities: [])
		)
		#expect(machine.statistics.sessions == 2)
	}
}
