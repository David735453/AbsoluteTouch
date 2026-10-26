import Testing

@testable import AbsoluteTouchCore

private let controllerToken = PairingToken(
	validatedBytes: (0..<16).map { UInt8($0) }
)

private func controllerConfiguration() -> SessionConfiguration {
	SessionConfiguration(
		token: controllerToken,
		helloInterval: .seconds(10),
		heartbeatInterval: .seconds(10),
		sessionTimeout: .seconds(30)
	)
}

private actor FakeTransport: DatagramTransport {
	nonisolated let datagrams: AsyncThrowingStream<[UInt8], Error>

	private nonisolated let continuation: AsyncThrowingStream<[UInt8], Error>.Continuation
	private var sentBytes: [[UInt8]] = []
	private var isClosed = false

	init() {
		let (datagrams, continuation) = AsyncThrowingStream<[UInt8], Error>.makeStream()
		self.datagrams = datagrams
		self.continuation = continuation
	}

	func open() async throws {}

	func send(_ datagram: [UInt8]) async throws {
		sentBytes.append(datagram)
	}

	func close() async {
		isClosed = true
		continuation.finish()
	}

	nonisolated func push(_ packet: Packet) {
		continuation.yield(packet.encode())
	}

	nonisolated func fail(_ error: any Error) {
		continuation.finish(throwing: error)
	}

	func packets() -> [Packet] {
		sentBytes.compactMap { try? Packet(bytes: $0) }
	}

	func closed() -> Bool {
		isClosed
	}
}

private enum FakeError: Error {
	case offline
}

private func waitUntil(
	_ condition: @escaping @Sendable () async -> Bool
) async -> Bool {
	for _ in 0..<200 {
		if await condition() {
			return true
		}
		try? await Task.sleep(for: .milliseconds(5))
	}
	return false
}

@Suite("Session controller")
struct SessionControllerTests {
	@Test("The controller handshakes, sends a sample, and says goodbye")
	func controllerSendsTraffic() async throws {
		let transport = FakeTransport()
		let controller = SessionController(
			configuration: controllerConfiguration(),
			transport: transport
		)

		await controller.start()

		let helloArrived = await waitUntil {
			await transport.packets().contains { $0.type == .hello }
		}
		#expect(helloArrived)

		let hello = try #require(
			await transport.packets().first { $0.type == .hello }
		)
		#expect(hello.token == controllerToken.bytes)
		#expect(hello.flags == [.capabilityTilt, .capabilityHover])

		var acknowledgement = Packet(type: .helloAck)
		acknowledgement.sessionID = 77
		acknowledgement.flags = [.capabilityTilt, .capabilityHover]
		try acknowledgement.setToken(controllerToken.bytes)
		transport.push(acknowledgement)

		let streaming = await waitUntil {
			await controller.currentState
				== .streaming(
					sessionID: 77,
					hostCapabilities: [.capabilityTilt, .capabilityHover]
				)
		}
		#expect(streaming)

		controller.submit(PointerSample(x: 10, y: 20, phase: .contact))

		let sampleArrived = await waitUntil {
			await transport.packets().contains {
				$0.type == .sample && $0.sessionID == 77
			}
		}
		#expect(sampleArrived)

		await controller.stop()

		let bye = await transport.packets().last
		#expect(bye?.type == .bye)
		#expect(bye?.sessionID == 77)
		#expect(await transport.closed())
	}

	@Test("The state stream reports connecting and streaming")
	func stateStreamReportsTransitions() async throws {
		let transport = FakeTransport()
		let controller = SessionController(
			configuration: controllerConfiguration(),
			transport: transport
		)

		let states = Task { () -> [SessionState] in
			var collected: [SessionState] = []
			for await state in controller.states {
				collected.append(state)
			}
			return collected
		}

		await controller.start()

		var acknowledgement = Packet(type: .helloAck)
		acknowledgement.sessionID = 88
		acknowledgement.flags = [.capabilityTilt, .capabilityHover]
		try acknowledgement.setToken(controllerToken.bytes)
		transport.push(acknowledgement)

		let streamed = await waitUntil {
			await controller.currentState.isStreaming
		}
		#expect(streamed)

		await controller.stop()
		let collected = await states.value

		#expect(collected.contains(.connecting(attempt: 1)))
		#expect(
			collected.contains(
				.streaming(
					sessionID: 88,
					hostCapabilities: [.capabilityTilt, .capabilityHover]
				)
			)
		)
		#expect(collected.last == .idle)
	}

	@Test("A transport failure becomes a failed state")
	func transportFailureIsReported() async {
		let transport = FakeTransport()
		let controller = SessionController(
			configuration: controllerConfiguration(),
			transport: transport
		)

		await controller.start()
		transport.fail(FakeError.offline)

		let failed = await waitUntil {
			let state = await controller.currentState
			if case .failed = state {
				return true
			}
			return false
		}

		#expect(failed)
		#expect(await transport.closed())
	}
}
