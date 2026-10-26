/// Kinds of packets carried by the AbsoluteTouch protocol.
public enum PacketType: UInt8, CaseIterable, Sendable {
	/// Session initiation sent by the iPad.
	case hello = 1

	/// Session acceptance sent by the desktop.
	case helloAck = 2

	/// Pointer sample.
	case sample = 3

	/// Liveness check sent by the iPad.
	case heartbeat = 4

	/// Liveness response sent by the desktop.
	case heartbeatAck = 5

	/// Clean session shutdown.
	case bye = 6
}
