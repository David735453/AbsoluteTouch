/// Drives a ``SessionMachine`` from a transport and a monotonic clock.
///
/// The controller is one-shot: ``start()`` begins a session and ``stop()``
/// finishes it. Create a new controller for the next session. Input is handed
/// to ``submit(_:)`` synchronously and consumed by an actor task, so the UI
/// never spawns one task per touch sample.
public actor SessionController {
	/// Session states, in order. Finishes when the controller stops.
	public nonisolated let states: AsyncStream<SessionState>

	private let transport: any DatagramTransport
	private let clock: ContinuousClock
	private let origin: ContinuousClock.Instant
	private let stateContinuation: AsyncStream<SessionState>.Continuation
	private let sampleStream: AsyncStream<PointerSample>
	private nonisolated let sampleContinuation: AsyncStream<PointerSample>.Continuation

	private var machine: SessionMachine
	private var started = false
	private var finished = false
	private var flushing = false
	private var lastPublishedState: SessionState?
	private var receiveTask: Task<Void, Never>?
	private var tickTask: Task<Void, Never>?
	private var sampleTask: Task<Void, Never>?

	/// Create a controller.
	///
	/// - Parameters:
	///   - configuration: The session policy.
	///   - transport: The transport to open on ``start()``.
	public init(configuration: SessionConfiguration, transport: any DatagramTransport) {
		self.transport = transport
		self.machine = SessionMachine(configuration: configuration)

		let clock = ContinuousClock()
		self.clock = clock
		self.origin = clock.now

		let (states, stateContinuation) = AsyncStream<SessionState>.makeStream()
		self.states = states
		self.stateContinuation = stateContinuation

		let (samples, sampleContinuation) = AsyncStream<PointerSample>.makeStream()
		self.sampleStream = samples
		self.sampleContinuation = sampleContinuation
	}

	/// Current state, available without subscribing to ``states``.
	public var currentState: SessionState {
		machine.state
	}

	/// Current counters.
	public var statistics: SessionStatistics {
		machine.statistics
	}

	/// Hand a pointer sample to the session.
	///
	/// This is safe to call from the main actor before the session is streaming;
	/// the machine drops samples that cannot be sent.
	///
	/// - Parameter sample: The pointer state.
	public nonisolated func submit(_ sample: PointerSample) {
		sampleContinuation.yield(sample)
	}

	/// Open the transport and begin handshaking.
	public func start() async {
		guard !started, !finished else {
			return
		}
		started = true

		do {
			try await transport.open()
		} catch {
			machine.fail(.transport(error.localizedDescription))
			publish()
			await shutdown()
			return
		}

		guard !finished else {
			await transport.close()
			return
		}

		machine.start(now: elapsed())
		publish()
		await flush()
		guard !finished else {
			return
		}
		startTasks()
	}

	/// Send `BYE` when streaming, then close the transport.
	public func stop() async {
		guard !finished else {
			return
		}
		guard started else {
			await shutdown()
			return
		}

		machine.stop(now: elapsed())
		publish()
		await flush()
		await shutdown()
	}

	private func startTasks() {
		let transport = self.transport
		receiveTask = Task { [weak self] in
			do {
				for try await datagram in transport.datagrams {
					guard let self else {
						return
					}
					await self.receive(datagram)
				}
				await self?.connectionClosed()
			} catch {
				await self?.connectionFailed(error)
			}
		}

		tickTask = Task { [weak self] in
			while !Task.isCancelled {
				do {
					try await Task.sleep(for: .milliseconds(20))
				} catch {
					return
				}
				guard let self else {
					return
				}
				await self.tick()
			}
		}

		let samples = sampleStream
		sampleTask = Task { [weak self] in
			for await sample in samples {
				guard let self else {
					return
				}
				await self.send(sample)
			}
		}
	}

	private func receive(_ datagram: [UInt8]) async {
		guard !finished else {
			return
		}
		guard let packet = try? Packet(bytes: datagram) else {
			return
		}
		machine.receive(packet, now: elapsed())
		publish()
		await flush()
	}

	private func tick() async {
		guard !finished else {
			return
		}
		machine.tick(now: elapsed())
		publish()
		await flush()
	}

	private func send(_ sample: PointerSample) async {
		guard !finished else {
			return
		}
		machine.submit(sample, now: elapsed())
		await flush()
	}

	private func flush() async {
		if flushing {
			return
		}
		flushing = true
		defer { flushing = false }

		while !finished {
			let packets = machine.takeOutbound()
			guard !packets.isEmpty else {
				return
			}
			for packet in packets {
				do {
					try await transport.send(packet.encode())
				} catch {
					await fail(.transport(error.localizedDescription))
					return
				}
			}
		}
	}

	private func connectionClosed() async {
		guard !finished else {
			return
		}
		await fail(.transport("The network connection closed."))
	}

	private func connectionFailed(_ error: any Error) async {
		guard !finished else {
			return
		}
		await fail(.transport(error.localizedDescription))
	}

	private func fail(_ failure: SessionFailure) async {
		guard !finished else {
			return
		}
		machine.fail(failure)
		publish()
		await shutdown()
	}

	private func shutdown() async {
		guard !finished else {
			return
		}
		finished = true
		receiveTask?.cancel()
		tickTask?.cancel()
		sampleTask?.cancel()
		receiveTask = nil
		tickTask = nil
		sampleTask = nil
		sampleContinuation.finish()
		stateContinuation.finish()
		await transport.close()
	}

	private func publish() {
		let state = machine.state
		guard state != lastPublishedState else {
			return
		}
		lastPublishedState = state
		stateContinuation.yield(state)
	}

	private func elapsed() -> Duration {
		origin.duration(to: clock.now)
	}
}
