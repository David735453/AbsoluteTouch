/// Constants that describe the AbsoluteTouch wire format.
///
/// The packet layout is fixed at 40 bytes and is shared with the Rust
/// desktop implementation. See `docs/protocol.md` for the field table and
/// `protocol/vectors.json` for the golden vectors.
public enum ProtocolSpec {
	/// Size of every wire packet in bytes.
	public static let packetSize = 40

	/// Magic value. The wire bytes are `41 54` (`AT`) in little-endian order.
	public static let magic: UInt16 = 0x5441

	/// Version of the packet layout.
	public static let version: UInt8 = 1

	/// Size of the session token carried by `HELLO` and `HELLO_ACK`.
	public static let tokenSize = 16

	/// Size of the reserved authentication tag.
	public static let authTagSize = 8

	/// Byte offsets of the fixed packet fields.
	public enum Offset {
		/// Start of the magic value.
		public static let magic = 0
		/// Protocol version byte.
		public static let version = 2
		/// Packet type byte.
		public static let type = 3
		/// Start of the flag bits.
		public static let flags = 4
		/// Start of the sequence number.
		public static let sequence = 6
		/// Start of the session identifier.
		public static let sessionID = 8
		/// Start of the device timestamp.
		public static let deviceTime = 12
		/// Start of the type-specific payload.
		public static let payload = 16
		/// Start of the X value or the first token word.
		public static let x = 16
		/// Start of the Y value or the second token word.
		public static let y = 18
		/// Start of the pressure value or the third token word.
		public static let pressure = 20
		/// Start of the altitude value or the fourth token word.
		public static let altitude = 22
		/// Start of the azimuth value or the fifth token word.
		public static let azimuth = 24
		/// Start of the twist value or the sixth token word.
		public static let twist = 26
		/// Start of the status value or the seventh token word.
		public static let status = 28
		/// Start of the reserved value or the eighth token word.
		public static let reserved = 30
		/// Start of the authentication tag.
		public static let authTag = 32
	}

	/// Byte range of the authentication tag in a wire packet.
	public static let authTagRange = Offset.authTag..<(Offset.authTag + authTagSize)
}
