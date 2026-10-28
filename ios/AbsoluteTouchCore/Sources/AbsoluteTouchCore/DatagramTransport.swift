/// Something the transport noticed that is not a packet.
///
/// A USB listener outlives the desktop connection, so the app can resume the
/// session when the desktop comes back. Wi-Fi has no equivalent: the
/// session timeout covers a desktop that stopped replying.
public enum TransportEvent: Sendable, Equatable {
	/// The connected desktop went away while the transport stayed usable.
	case peerDisconnected
}

/// A datagram transport used by ``SessionController``.
///
/// One element of ``datagrams`` is one wire packet. Both streams may be
/// consumed once; the transport finishes them when it closes.
public protocol DatagramTransport: Sendable {
	/// Inbound datagrams, in arrival order.
	var datagrams: AsyncThrowingStream<[UInt8], Error> { get }

	/// Transport events, in order.
	var events: AsyncStream<TransportEvent> { get }

	/// Open the transport and wait until it can send.
	func open() async throws

	/// Send one datagram.
	func send(_ datagram: [UInt8]) async throws

	/// Close the transport and finish ``datagrams`` and ``events``.
	func close() async
}
