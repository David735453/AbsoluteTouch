import Testing

@testable import AbsoluteTouchCore

@Suite("Stream framing")
struct PacketFramerTests {
	private func packet(sequence: UInt16) -> [UInt8] {
		var packet = Packet(type: .sample)
		packet.sequence = sequence
		return packet.encode()
	}

	private func sequences(from bytes: [[UInt8]]) -> [UInt16] {
		bytes.compactMap { try? Packet(bytes: $0).sequence }
	}

	@Test("One byte at a time reassembles every packet")
	func byteAtATime() {
		let wire = (0..<4).flatMap { packet(sequence: UInt16($0)) }
		var framer = PacketFramer()
		var packets: [[UInt8]] = []

		for byte in wire {
			packets.append(contentsOf: framer.append([byte]))
		}

		#expect(framer.buffered == 0)
		#expect(sequences(from: packets) == [0, 1, 2, 3])
	}

	@Test("Short and whole reads mix")
	func mixedReads() {
		let wire = (0..<3).flatMap { packet(sequence: UInt16($0)) }
		var framer = PacketFramer()

		// 39 bytes leaves one byte short, 40 completes that packet and leaves
		// 39 bytes of the next, and the final byte completes the last one.
		let first = framer.append(Array(wire[0..<39]))
		let second = framer.append(Array(wire[39..<79]))
		let third = framer.append(Array(wire[79..<119]))
		let fourth = framer.append(Array(wire[119..<120]))

		#expect(sequences(from: first) == [])
		#expect(sequences(from: second) == [0])
		#expect(sequences(from: third) == [1])
		#expect(sequences(from: fourth) == [2])
		#expect(framer.buffered == 0)
	}

	@Test("A large read yields every complete packet and keeps the tail")
	func largeRead() {
		var framer = PacketFramer()
		let wire = (0..<100).flatMap { packet(sequence: UInt16($0)) }

		let packets = framer.append(wire + [0xFF])

		#expect(packets.count == 100)
		#expect(sequences(from: packets) == Array(0..<100).map { UInt16($0) })
		#expect(framer.buffered == 1)
	}

	@Test("An empty read changes nothing")
	func emptyRead() {
		var framer = PacketFramer()
		let wire = packet(sequence: 9)

		#expect(framer.append(wire).count == 1)
		#expect(framer.append([]).isEmpty)
		#expect(framer.buffered == 0)
	}
}
