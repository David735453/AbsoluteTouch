#if canImport(Network)
	import Foundation
	import Network

	/// Errors creating a UDP transport.
	public enum UDPTransportError: Error, Sendable, Equatable {
		/// The host was empty or the port was zero.
		case invalidAddress
	}

	extension UDPTransportError: LocalizedError {
		/// A human-readable description of the error.
		public var errorDescription: String? {
			switch self {
			case .invalidAddress:
				"Enter a desktop address and a port between 1 and 65535."
			}
		}
	}

	/// Wi-Fi UDP transport backed by `Network.framework`.
	///
	/// The transport connects to one desktop endpoint. The desktop replies to
	/// the source port of `HELLO`, which `NWConnection` keeps open for the
	/// lifetime of the object.
	public final class UDPTransport: DatagramTransport, @unchecked Sendable {
		/// Inbound datagrams, in arrival order.
		public let datagrams: AsyncThrowingStream<[UInt8], Error>

		private let connection: NWConnection
		private let queue: DispatchQueue
		private let continuation: AsyncThrowingStream<[UInt8], Error>.Continuation

		/// Create a transport to one desktop endpoint.
		///
		/// - Parameters:
		///   - host: An IP address or host name.
		///   - port: A non-zero UDP port.
		/// - Throws: ``UDPTransportError/invalidAddress`` when either value is
		///   unusable.
		public init(host: String, port: UInt16) throws {
			let trimmed = host.trimmingCharacters(in: .whitespacesAndNewlines)
			guard !trimmed.isEmpty, let endpointPort = NWEndpoint.Port(rawValue: port) else {
				throw UDPTransportError.invalidAddress
			}

			self.connection = NWConnection(
				host: NWEndpoint.Host(trimmed),
				port: endpointPort,
				using: .udp
			)
			self.queue = DispatchQueue(label: "org.dstudios.absolutetouch.udp")
			let (datagrams, continuation) = AsyncThrowingStream<[UInt8], Error>.makeStream()
			self.datagrams = datagrams
			self.continuation = continuation
		}

		/// Start the connection and wait until it is ready.
		public func open() async throws {
			let gate = ResumeGate()
			try await withCheckedThrowingContinuation { continuation in
				connection.stateUpdateHandler = { [weak self] state in
					guard let self else {
						return
					}
					switch state {
					case .ready:
						gate.run { continuation.resume() }
					case .failed(let error):
						self.continuation.finish(throwing: error)
						gate.run { continuation.resume(throwing: error) }
					case .cancelled:
						self.continuation.finish()
						gate.run { continuation.resume(throwing: CancellationError()) }
					default:
						break
					}
				}
				connection.start(queue: queue)
				receiveNext()
			}
		}

		/// Send one datagram.
		public func send(_ datagram: [UInt8]) async throws {
			let data = Data(datagram)
			// Spell out the continuation type: the result is discarded, so leaving
			// it to inference lets Swift 6.4 crash with "failed to produce
			// diagnostic for expression" instead of choosing `Void`.
			try await withCheckedThrowingContinuation {
				(continuation: CheckedContinuation<Void, any Error>) in
				connection.send(
					content: data,
					completion: .contentProcessed { error in
						if let error {
							continuation.resume(throwing: error)
						} else {
							continuation.resume()
						}
					}
				)
			}
		}

		/// Cancel the connection and finish ``datagrams``.
		public func close() async {
			connection.cancel()
			continuation.finish()
		}

		private func receiveNext() {
			connection.receiveMessage { [weak self] data, _, _, error in
				guard let self else {
					return
				}
				if let data, !data.isEmpty {
					self.continuation.yield(Array(data))
				}
				if let error {
					self.continuation.finish(throwing: error)
					return
				}
				self.receiveNext()
			}
		}
	}

	/// Resume a checked continuation at most once.
	///
	/// `NWConnection` reports `.ready` and later `.failed` or `.cancelled`; only
	/// the first terminal transition may resume the `open()` continuation.
	private final class ResumeGate: @unchecked Sendable {
		private let lock = NSLock()
		private var resumed = false

		func run(_ body: () -> Void) {
			lock.lock()
			if resumed {
				lock.unlock()
				return
			}
			resumed = true
			lock.unlock()
			body()
		}
	}
#endif
