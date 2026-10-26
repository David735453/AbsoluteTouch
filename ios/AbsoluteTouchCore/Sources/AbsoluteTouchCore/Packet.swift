/// A decoded AbsoluteTouch packet.
///
/// Fields are little-endian on the wire. Some payload fields are
/// type-specific: `x` through `reserved` carry the 16-byte token in `HELLO`
/// and `HELLO_ACK` packets and two 32-bit timestamps in `HEARTBEAT` packets.
/// Use ``token`` when working with a handshake packet.
public struct Packet: Sendable, Hashable {
	/// Protocol version.
	public var version: UInt8

	/// Packet kind.
	public var type: PacketType

	/// Flag bits. See ``PacketFlags``.
	public var flags: PacketFlags

	/// Sequence number. Wraps at 65,535.
	public var sequence: UInt16

	/// Session identifier assigned by the desktop.
	public var sessionID: UInt32

	/// Device monotonic timestamp in microseconds.
	public var deviceTimeMicroseconds: UInt32

	/// Normalized X position, or the low word of the first timestamp.
	public var x: UInt16

	/// Normalized Y position, or the high word of the first timestamp.
	public var y: UInt16

	/// Normalized pressure, or the low word of the second timestamp.
	public var pressure: UInt16

	/// Normalized altitude angle, or the high word of the second timestamp.
	public var altitude: UInt16

	/// Normalized azimuth angle.
	public var azimuth: UInt16

	/// Normalized twist. Reserved on current hardware.
	public var twist: UInt16

	/// Reserved telemetry field. Senders set zero in version 1.
	public var status: UInt16

	/// Reserved for future use.
	public var reserved: UInt16

	/// Reserved authentication tag; all zeros in version 1.
	public var authTag: AuthTag

	/// Create a zeroed packet of the given kind with the current version.
	///
	/// - Parameter type: The packet kind.
	public init(type: PacketType) {
		self.version = ProtocolSpec.version
		self.type = type
		self.flags = []
		self.sequence = 0
		self.sessionID = 0
		self.deviceTimeMicroseconds = 0
		self.x = 0
		self.y = 0
		self.pressure = 0
		self.altitude = 0
		self.azimuth = 0
		self.twist = 0
		self.status = 0
		self.reserved = 0
		self.authTag = .zero
	}

	/// Decode a packet from exactly ``ProtocolSpec/packetSize`` bytes.
	///
	/// - Parameter bytes: The wire bytes.
	/// - Throws: ``PacketError`` for a wrong length, bad magic, unsupported
	///   version, or unknown packet type.
	public init(bytes: [UInt8]) throws {
		guard bytes.count == ProtocolSpec.packetSize else {
			throw PacketError.invalidLength(bytes.count)
		}

		let magic = readUInt16(bytes, at: ProtocolSpec.Offset.magic)
		guard magic == ProtocolSpec.magic else {
			throw PacketError.badMagic(magic)
		}

		let version = bytes[ProtocolSpec.Offset.version]
		guard version == ProtocolSpec.version else {
			throw PacketError.unsupportedVersion(version)
		}

		let rawType = bytes[ProtocolSpec.Offset.type]
		guard let type = PacketType(rawValue: rawType) else {
			throw PacketError.unknownType(rawType)
		}

		self.init(type: type)
		self.version = version
		self.flags = PacketFlags(
			rawValue: readUInt16(bytes, at: ProtocolSpec.Offset.flags)
		)
		self.sequence = readUInt16(bytes, at: ProtocolSpec.Offset.sequence)
		self.sessionID = readUInt32(bytes, at: ProtocolSpec.Offset.sessionID)
		self.deviceTimeMicroseconds = readUInt32(
			bytes,
			at: ProtocolSpec.Offset.deviceTime
		)
		self.x = readUInt16(bytes, at: ProtocolSpec.Offset.x)
		self.y = readUInt16(bytes, at: ProtocolSpec.Offset.y)
		self.pressure = readUInt16(bytes, at: ProtocolSpec.Offset.pressure)
		self.altitude = readUInt16(bytes, at: ProtocolSpec.Offset.altitude)
		self.azimuth = readUInt16(bytes, at: ProtocolSpec.Offset.azimuth)
		self.twist = readUInt16(bytes, at: ProtocolSpec.Offset.twist)
		self.status = readUInt16(bytes, at: ProtocolSpec.Offset.status)
		self.reserved = readUInt16(bytes, at: ProtocolSpec.Offset.reserved)
		self.authTag = AuthTag(
			validatedBytes: Array(bytes[ProtocolSpec.authTagRange])
		)
	}

	/// Encode the packet into exactly ``ProtocolSpec/packetSize`` bytes.
	///
	/// - Returns: The wire bytes.
	public func encode() -> [UInt8] {
		var bytes = [UInt8](repeating: 0, count: ProtocolSpec.packetSize)
		writeUInt16(ProtocolSpec.magic, into: &bytes, at: ProtocolSpec.Offset.magic)
		bytes[ProtocolSpec.Offset.version] = version
		bytes[ProtocolSpec.Offset.type] = type.rawValue
		writeUInt16(flags.rawValue, into: &bytes, at: ProtocolSpec.Offset.flags)
		writeUInt16(sequence, into: &bytes, at: ProtocolSpec.Offset.sequence)
		writeUInt32(sessionID, into: &bytes, at: ProtocolSpec.Offset.sessionID)
		writeUInt32(
			deviceTimeMicroseconds,
			into: &bytes,
			at: ProtocolSpec.Offset.deviceTime
		)
		writeUInt16(x, into: &bytes, at: ProtocolSpec.Offset.x)
		writeUInt16(y, into: &bytes, at: ProtocolSpec.Offset.y)
		writeUInt16(pressure, into: &bytes, at: ProtocolSpec.Offset.pressure)
		writeUInt16(altitude, into: &bytes, at: ProtocolSpec.Offset.altitude)
		writeUInt16(azimuth, into: &bytes, at: ProtocolSpec.Offset.azimuth)
		writeUInt16(twist, into: &bytes, at: ProtocolSpec.Offset.twist)
		writeUInt16(status, into: &bytes, at: ProtocolSpec.Offset.status)
		writeUInt16(reserved, into: &bytes, at: ProtocolSpec.Offset.reserved)
		bytes.replaceSubrange(ProtocolSpec.authTagRange, with: authTag.bytes)
		return bytes
	}

	/// The 16-byte session token stored in a `HELLO` or `HELLO_ACK` payload.
	public var token: [UInt8] {
		var bytes = [UInt8](repeating: 0, count: ProtocolSpec.tokenSize)
		writeUInt16(x, into: &bytes, at: 0)
		writeUInt16(y, into: &bytes, at: 2)
		writeUInt16(pressure, into: &bytes, at: 4)
		writeUInt16(altitude, into: &bytes, at: 6)
		writeUInt16(azimuth, into: &bytes, at: 8)
		writeUInt16(twist, into: &bytes, at: 10)
		writeUInt16(status, into: &bytes, at: 12)
		writeUInt16(reserved, into: &bytes, at: 14)
		return bytes
	}

	/// Store a 16-byte session token in the `HELLO` or `HELLO_ACK` payload.
	///
	/// - Parameter token: The token bytes.
	/// - Throws: ``PacketError/invalidTokenLength(_:)`` when the token is not
	///   exactly ``ProtocolSpec/tokenSize`` bytes.
	public mutating func setToken(_ token: [UInt8]) throws {
		guard token.count == ProtocolSpec.tokenSize else {
			throw PacketError.invalidTokenLength(token.count)
		}
		x = readUInt16(token, at: 0)
		y = readUInt16(token, at: 2)
		pressure = readUInt16(token, at: 4)
		altitude = readUInt16(token, at: 6)
		azimuth = readUInt16(token, at: 8)
		twist = readUInt16(token, at: 10)
		status = readUInt16(token, at: 12)
		reserved = readUInt16(token, at: 14)
	}
}

private func readUInt16(_ bytes: [UInt8], at offset: Int) -> UInt16 {
	UInt16(bytes[offset]) | (UInt16(bytes[offset + 1]) << 8)
}

private func readUInt32(_ bytes: [UInt8], at offset: Int) -> UInt32 {
	UInt32(bytes[offset])
		| (UInt32(bytes[offset + 1]) << 8)
		| (UInt32(bytes[offset + 2]) << 16)
		| (UInt32(bytes[offset + 3]) << 24)
}

private func writeUInt16(_ value: UInt16, into bytes: inout [UInt8], at offset: Int) {
	bytes[offset] = UInt8(truncatingIfNeeded: value)
	bytes[offset + 1] = UInt8(truncatingIfNeeded: value >> 8)
}

private func writeUInt32(_ value: UInt32, into bytes: inout [UInt8], at offset: Int) {
	bytes[offset] = UInt8(truncatingIfNeeded: value)
	bytes[offset + 1] = UInt8(truncatingIfNeeded: value >> 8)
	bytes[offset + 2] = UInt8(truncatingIfNeeded: value >> 16)
	bytes[offset + 3] = UInt8(truncatingIfNeeded: value >> 24)
}
