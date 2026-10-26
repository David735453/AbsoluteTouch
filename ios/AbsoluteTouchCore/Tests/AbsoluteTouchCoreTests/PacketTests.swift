import AbsoluteTouchCore
import Testing

@Suite("Packet")
struct PacketTests {
	@Test("A new packet starts zeroed at the current version")
	func newPacketDefaults() {
		let packet = Packet(type: .sample)

		#expect(packet.version == ProtocolSpec.version)
		#expect(packet.type == .sample)
		#expect(packet.flags.isEmpty)
		#expect(packet.sequence == 0)
		#expect(packet.sessionID == 0)
		#expect(packet.authTag == .zero)
		#expect(packet.encode().count == ProtocolSpec.packetSize)
	}

	@Test("A packet round-trips every field")
	func roundTripsEveryField() throws {
		var packet = Packet(type: .sample)
		packet.flags = [.contact, .barrel, .finger]
		packet.sequence = .max
		packet.sessionID = 0xDEAD_BEEF
		packet.deviceTimeMicroseconds = .max
		packet.x = 0
		packet.y = .max
		packet.pressure = 12_345
		packet.altitude = 22_222
		packet.azimuth = 33_333
		packet.twist = 44_444
		packet.status = 0
		packet.reserved = 0
		packet.authTag = try #require(
			AuthTag(bytes: [1, 2, 3, 4, 5, 6, 7, 8])
		)

		let decoded = try Packet(bytes: packet.encode())

		#expect(decoded == packet)
	}

	@Test("Decoding rejects a wrong buffer length")
	func rejectsWrongLength() {
		#expect(throws: PacketError.invalidLength(39)) {
			try Packet(bytes: [UInt8](repeating: 0, count: 39))
		}
		#expect(throws: PacketError.invalidLength(41)) {
			try Packet(bytes: [UInt8](repeating: 0, count: 41))
		}
	}

	@Test("Decoding rejects a bad magic value")
	func rejectsBadMagic() {
		var bytes = Packet(type: .sample).encode()
		bytes[0] = 0
		bytes[1] = 0

		#expect(throws: PacketError.badMagic(0)) {
			try Packet(bytes: bytes)
		}
	}

	@Test("Decoding rejects an unsupported version")
	func rejectsUnsupportedVersion() {
		var bytes = Packet(type: .sample).encode()
		bytes[ProtocolSpec.Offset.version] = ProtocolSpec.version + 1

		#expect(throws: PacketError.unsupportedVersion(ProtocolSpec.version + 1)) {
			try Packet(bytes: bytes)
		}
	}

	@Test("Decoding rejects an unknown packet type")
	func rejectsUnknownType() {
		var bytes = Packet(type: .sample).encode()
		bytes[ProtocolSpec.Offset.type] = 99

		#expect(throws: PacketError.unknownType(99)) {
			try Packet(bytes: bytes)
		}
	}

	@Test("The session token round-trips")
	func tokenRoundTrips() throws {
		let token = (0..<ProtocolSpec.tokenSize).map { UInt8($0) }
		var packet = Packet(type: .hello)

		try packet.setToken(token)

		#expect(packet.token == token)
		#expect(try Packet(bytes: packet.encode()).token == token)
	}

	@Test("The session token must be exactly 16 bytes")
	func rejectsWrongTokenLength() {
		var packet = Packet(type: .hello)

		#expect(throws: PacketError.invalidTokenLength(15)) {
			try packet.setToken([UInt8](repeating: 0, count: 15))
		}
	}
}
