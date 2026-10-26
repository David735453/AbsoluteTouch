import AbsoluteTouchCore
import Foundation
import Testing

private let vectorsURL = URL(fileURLWithPath: #filePath)
	.deletingLastPathComponent()
	.deletingLastPathComponent()
	.deletingLastPathComponent()
	.deletingLastPathComponent()
	.deletingLastPathComponent()
	.appendingPathComponent("protocol/vectors.json")

@Suite("Golden vectors")
struct GoldenVectorTests {
	@Test("Every shared vector decodes and re-encodes")
	func goldenVectorsRoundTrip() throws {
		let file = try loadVectors()

		#expect(file.version == ProtocolSpec.version)
		#expect(file.packetSize == ProtocolSpec.packetSize)
		#expect(!file.packets.isEmpty)

		for vector in file.packets {
			let bytes = try decodeHex(vector.hex)
			#expect(bytes.count == ProtocolSpec.packetSize, "\(vector.name): size")

			let packet = try Packet(bytes: bytes)
			let context = "vector \(vector.name)"

			#expect(packet.version == vector.version, "\(context): version")
			#expect(packet.type.rawValue == vector.type, "\(context): type")
			#expect(packet.flags.rawValue == vector.flags, "\(context): flags")
			#expect(packet.sequence == vector.seq, "\(context): sequence")
			#expect(packet.sessionID == vector.sessionID, "\(context): session")
			#expect(
				packet.deviceTimeMicroseconds == vector.deviceTime,
				"\(context): device time"
			)
			#expect(
				packet.authTag.bytes == (try decodeHex(vector.authTag)),
				"\(context): auth tag"
			)

			if let token = vector.token {
				#expect(
					packet.token == (try decodeHex(token)),
					"\(context): token"
				)
			} else {
				#expect(packet.x == (vector.x ?? 0), "\(context): x")
				#expect(packet.y == (vector.y ?? 0), "\(context): y")
				#expect(
					packet.pressure == (vector.pressure ?? 0),
					"\(context): pressure"
				)
				#expect(
					packet.altitude == (vector.altitude ?? 0),
					"\(context): altitude"
				)
				#expect(
					packet.azimuth == (vector.azimuth ?? 0),
					"\(context): azimuth"
				)
				#expect(packet.twist == (vector.twist ?? 0), "\(context): twist")
				#expect(
					packet.status == (vector.status ?? 0),
					"\(context): status"
				)
				#expect(
					packet.reserved == (vector.reserved ?? 0),
					"\(context): reserved"
				)
			}

			#expect(packet.encode() == bytes, "\(context): re-encode")
		}
	}

	@Test("Vectors cover every packet type")
	func vectorsCoverEveryType() throws {
		let file = try loadVectors()
		let types = Set(file.packets.map(\.type))

		for type in PacketType.allCases {
			#expect(types.contains(type.rawValue), "missing vector for \(type)")
		}
	}
}

private func loadVectors() throws -> VectorFile {
	let data = try Data(contentsOf: vectorsURL)
	return try JSONDecoder().decode(VectorFile.self, from: data)
}

private struct VectorFile: Decodable {
	let version: UInt8
	let packetSize: Int
	let packets: [Vector]

	enum CodingKeys: String, CodingKey {
		case version
		case packetSize = "packet_size"
		case packets
	}
}

private struct Vector: Decodable {
	let name: String
	let hex: String
	let version: UInt8
	let type: UInt8
	let flags: UInt16
	let seq: UInt16
	let sessionID: UInt32
	let deviceTime: UInt32
	let x: UInt16?
	let y: UInt16?
	let pressure: UInt16?
	let altitude: UInt16?
	let azimuth: UInt16?
	let twist: UInt16?
	let status: UInt16?
	let reserved: UInt16?
	let token: String?
	let authTag: String

	enum CodingKeys: String, CodingKey {
		case name
		case hex
		case version
		case type
		case flags
		case seq
		case x
		case y
		case pressure
		case altitude
		case azimuth
		case twist
		case status
		case reserved
		case token
		case sessionID = "session_id"
		case deviceTime = "t_device_us"
		case authTag = "auth_tag"
	}
}

private struct HexError: Error {
	let value: String
}

private func decodeHex(_ hex: String) throws -> [UInt8] {
	guard hex.count.isMultiple(of: 2) else {
		throw HexError(value: hex)
	}

	var bytes: [UInt8] = []
	bytes.reserveCapacity(hex.count / 2)

	var index = hex.startIndex
	while index < hex.endIndex {
		let end = hex.index(index, offsetBy: 2)
		guard let byte = UInt8(hex[index..<end], radix: 16) else {
			throw HexError(value: hex)
		}
		bytes.append(byte)
		index = end
	}
	return bytes
}
