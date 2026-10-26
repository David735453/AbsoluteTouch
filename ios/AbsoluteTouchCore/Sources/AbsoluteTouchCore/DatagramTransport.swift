/// A datagram transport used by ``SessionController``.
///
/// One element of ``datagrams`` is one wire packet. The stream may be consumed
/// once; the transport finishes it when the connection closes or fails.
public protocol DatagramTransport: Sendable {
	/// Inbound datagrams, in arrival order.
	var datagrams: AsyncThrowingStream<[UInt8], Error> { get }

	/// Open the transport and wait until it can send.
	func open() async throws

	/// Send one datagram.
	func send(_ datagram: [UInt8]) async throws

	/// Close the transport and finish ``datagrams``.
	func close() async
}
