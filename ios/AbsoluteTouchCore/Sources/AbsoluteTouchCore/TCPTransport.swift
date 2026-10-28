import Foundation

/// Reassembles fixed-size packets from a byte stream.
///
/// USB is a TCP stream, so a read can end in the middle of a packet or hold
/// several packets. The wire format has no length prefix because every packet
/// is ``ProtocolSpec/packetSize`` bytes; this type buffers the remainder and
/// returns every complete packet in order.
struct PacketFramer: Sendable {
	private var buffer: [UInt8] = []

	/// Append bytes read from the stream.
	///
	/// - Parameter bytes: Newly received bytes, in order.
	/// - Returns: Every packet the new bytes completed, oldest first.
	mutating func append(_ bytes: [UInt8]) -> [[UInt8]] {
		buffer.append(contentsOf: bytes)

		let size = ProtocolSpec.packetSize
		var packets: [[UInt8]] = []
		while buffer.count >= size {
			packets.append(Array(buffer.prefix(size)))
			buffer.removeFirst(size)
		}
		return packets
	}

	/// Bytes buffered towards the next packet.
	var buffered: Int {
		buffer.count
	}
}

#if canImport(Network)
	import Network

	/// Errors creating a USB transport.
	public enum TCPTransportError: Error, Sendable, Equatable {
		/// The port was zero or otherwise unusable.
		case invalidPort
	}

	extension TCPTransportError: LocalizedError {
		/// A human-readable description of the error.
		public var errorDescription: String? {
			switch self {
			case .invalidPort:
				"Enter a TCP port between 1 and 65535."
			}
		}
	}

	/// Continuation for a `Void`-returning checked continuation.
	///
	/// The alias keeps the parameter on the brace line, which both `swift-format`
	/// and SwiftLint's `closure_parameter_position` accept.
	private typealias SendContinuation = CheckedContinuation<Void, any Error>

	/// USB transport: the iPad listens and the desktop connects through `usbmuxd`.
	///
	/// Over USB the desktop is the TCP client, because it is the side that can
	/// ask `usbmuxd` to open a connection to a port inside the app. The app
	/// therefore listens on `port` and accepts one desktop at a time.
	///
	/// `NWConnection` delivers arbitrary chunks; ``PacketFramer`` rebuilds the
	/// 40-byte packets that ``SessionController`` expects. Until a desktop
	/// connects, ``send(_:)`` drops packets: the handshake is idempotent and the
	/// next `HELLO`, at most one interval later, starts the session.
	///
	/// Losing the desktop does not end the transport. The listener stays up, the
	/// connection is forgotten, and ``events`` reports `peerDisconnected` so the
	/// session can handshake again. Restarting the desktop command therefore
	/// reconnects without a trip through the app's Stop button.
	public final class TCPTransport: DatagramTransport, @unchecked Sendable {
		/// Inbound packets, in arrival order.
		public let datagrams: AsyncThrowingStream<[UInt8], Error>

		/// Transport events, in order.
		public let events: AsyncStream<TransportEvent>

		private let listener: NWListener
		private let queue: DispatchQueue
		private let continuation: AsyncThrowingStream<[UInt8], Error>.Continuation
		private let eventContinuation: AsyncStream<TransportEvent>.Continuation
		private let lock = NSLock()
		private var connection: NWConnection?
		private var framer = PacketFramer()

		/// Listen for one desktop connection on `port`.
		///
		/// - Parameter port: The TCP port the desktop asks `usbmuxd` for.
		/// - Throws: ``TCPTransportError/invalidPort`` when the port is zero,
		///   or an `NWError` when the listener cannot be created.
		public init(port: UInt16) throws {
			guard let endpointPort = NWEndpoint.Port(rawValue: port) else {
				throw TCPTransportError.invalidPort
			}

			let parameters = NWParameters.tcp
			let protocolStack = parameters.defaultProtocolStack.transportProtocol
			if let options = protocolStack as? NWProtocolTCP.Options {
				// Each packet is sent as its own write; letting TCP coalesce
				// would add a frame of latency for no gain.
				options.noDelay = true
			}
			// Restarting a session must not collide with the previous
			// listener while its socket is still in `TIME_WAIT`.
			parameters.allowLocalEndpointReuse = true

			// `usbmuxd` opens the connection from inside the app, so bind
			// loopback only. A wildcard listener is also reachable from Wi-Fi
			// and, on iOS 14+, waits on the local network permission prompt;
			// while it waits the port is not bound at all, which `usbmuxd`
			// reports as "Connection refused". Loopback needs no permission
			// and no network path, so the listener reaches `.ready` immediately.
			parameters.requiredLocalEndpoint = .hostPort(
				host: .ipv4(.loopback),
				port: endpointPort
			)

			self.listener = try NWListener(using: parameters)
			self.queue = DispatchQueue(label: "org.dstudios.absolutetouch.tcp")
			let (datagrams, continuation) = AsyncThrowingStream<[UInt8], Error>.makeStream()
			self.datagrams = datagrams
			self.continuation = continuation
			let (events, eventContinuation) = AsyncStream<TransportEvent>.makeStream()
			self.events = events
			self.eventContinuation = eventContinuation
		}

		/// Start listening.
		///
		/// Returns once the listener is started; binding failures arrive through
		/// ``datagrams`` because `NWListener` reports them asynchronously.
		public func open() async throws {
			listener.stateUpdateHandler = { [weak self] state in
				guard let self else {
					return
				}
				switch state {
				case .failed(let error), .waiting(let error):
					self.continuation.finish(throwing: error)
				case .setup, .ready, .cancelled:
					break
				@unknown default:
					break
				}
			}
			listener.newConnectionHandler = { [weak self] connection in
				self?.accept(connection)
			}
			listener.start(queue: queue)
		}

		/// Send one packet.
		///
		/// Packets sent before a desktop connects are dropped. A write that
		/// races with the desktop closing the link is reported through
		/// ``events`` instead of failing the session, because the listener
		/// accepts the next desktop connection.
		public func send(_ datagram: [UInt8]) async throws {
			guard let connection = currentConnection() else {
				return
			}

			let data = Data(datagram)
			do {
				// Spell out the continuation type: the result is discarded, so
				// leaving it to inference can make Swift choose the wrong
				// `resume`.
				try await withCheckedThrowingContinuation { (continuation: SendContinuation) in
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
			} catch {
				drop(connection)
			}
		}

		/// Stop listening and close the desktop connection.
		public func close() async {
			listener.cancel()
			takeConnection()?.cancel()
			continuation.finish()
			eventContinuation.finish()
		}

		private func accept(_ connection: NWConnection) {
			guard takeConnection() == nil else {
				// One desktop at a time; a second connection would fight over
				// the tablet.
				connection.cancel()
				return
			}
			// Bytes buffered for the previous desktop are not the start of this
			// connection's packet stream.
			framer = PacketFramer()
			store(connection)

			connection.stateUpdateHandler = { [weak self] state in
				guard let self else {
					return
				}
				switch state {
				case .ready:
					self.receiveNext(on: connection)
				case .failed:
					self.drop(connection)
				case .cancelled:
					self.forget(connection)
				case .setup, .preparing, .waiting:
					break
				@unknown default:
					break
				}
			}
			connection.start(queue: queue)
		}

		/// Forget a connection the desktop closed and let the session know.
		///
		/// Safe to call from the transport queue and from ``send(_:)``: the
		/// active connection is cleared under the lock, so only the first caller
		/// reports the disconnect.
		private func drop(_ connection: NWConnection) {
			guard take(connection) else {
				return
			}
			connection.cancel()
			eventContinuation.yield(.peerDisconnected)
		}

		/// Drop `connection` if it is still the active one.
		private func forget(_ connection: NWConnection) {
			lock.lock()
			defer { lock.unlock() }
			if self.connection === connection {
				self.connection = nil
			}
		}

		private func receiveNext(on connection: NWConnection) {
			connection.receive(
				minimumIncompleteLength: 1,
				maximumLength: 65_536
			) { [weak self] data, _, isComplete, error in
				guard let self else {
					return
				}

				if let data, !data.isEmpty {
					for packet in self.framer.append(Array(data)) {
						self.continuation.yield(packet)
					}
				}

				if isComplete || error != nil {
					self.drop(connection)
					return
				}
				self.receiveNext(on: connection)
			}
		}

		private func currentConnection() -> NWConnection? {
			lock.lock()
			defer { lock.unlock() }
			return connection
		}

		/// Clear `connection` if it is the active one.
		private func take(_ connection: NWConnection) -> Bool {
			lock.lock()
			defer { lock.unlock() }
			guard self.connection === connection else {
				return false
			}
			self.connection = nil
			return true
		}

		private func takeConnection() -> NWConnection? {
			lock.lock()
			defer { lock.unlock() }
			let existing = connection
			connection = nil
			return existing
		}

		private func store(_ connection: NWConnection) {
			lock.lock()
			defer { lock.unlock() }
			self.connection = connection
		}
	}
#endif
