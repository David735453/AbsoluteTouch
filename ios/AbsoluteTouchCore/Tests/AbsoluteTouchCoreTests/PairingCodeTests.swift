import AbsoluteTouchCore
import Foundation
import Testing

@Suite("Pairing codes")
struct PairingCodeTests {
	@Test("Every shared code expands to the shared token")
	func sharedVectors() throws {
		let file = try loadPairingVectors()

		#expect(file.codeLength == PairingCode.length)
		#expect(file.alphabet == PairingCode.alphabet)
		#expect(!file.vectors.isEmpty)

		for vector in file.vectors {
			#expect(
				try PairingCode.normalize(vector.input) == vector.code,
				"\(vector.input): normalize"
			)

			let token = try PairingCode.token(from: vector.input)
			let expected = try decodeHex(vector.token)
			#expect(token.bytes == expected, "\(vector.input): token")
			#expect(
				try PairingToken(code: vector.input).bytes == expected,
				"\(vector.input): token initializer"
			)
		}
	}

	@Test("Confusable letters fold to digits")
	func confusableLettersFold() throws {
		#expect(try PairingCode.normalize("0O1IL2") == "001112")
		#expect(try PairingCode.normalize("k7m-2qx") == "K7M2QX")
		#expect(try PairingCode.normalize(" k7m : 2qx ") == "K7M2QX")
	}

	@Test("Wrong length and unknown characters are rejected")
	func rejectsInvalidCodes() {
		#expect(throws: PairingCodeError.length(3)) {
			try PairingCode.normalize("ABC")
		}
		#expect(throws: PairingCodeError.length(7)) {
			try PairingCode.normalize("ABCDEFG")
		}
		#expect(throws: PairingCodeError.character("$")) {
			try PairingCode.normalize("ABC$EF")
		}
		#expect(throws: PairingCodeError.character("$")) {
			try PairingToken(code: "ABC$EF")
		}
	}

	@Test("A generated code round-trips")
	func generatedCodesRoundTrip() throws {
		for _ in 0..<64 {
			let code = PairingCode.random()
			#expect(code.count == PairingCode.length)
			#expect(try PairingCode.normalize(code) == code)
			#expect(code.allSatisfy { PairingCode.alphabet.contains($0) })
			#expect(try PairingToken(code: code).bytes.count == PairingToken.byteCount)
		}
	}

	@Test("Different codes give different tokens")
	func codesAreDistinct() throws {
		let first = try PairingCode.token(from: "K7M2QX")
		let second = try PairingCode.token(from: "K7M2QY")

		#expect(first != second)
		#expect(first.bytes[8...].contains { $0 != 0 })
	}
}

private let vectorsURL = URL(fileURLWithPath: #filePath)
	.deletingLastPathComponent()
	.deletingLastPathComponent()
	.deletingLastPathComponent()
	.deletingLastPathComponent()
	.deletingLastPathComponent()
	.appendingPathComponent("protocol/vectors.json")

private func loadPairingVectors() throws -> PairingSection {
	let data = try Data(contentsOf: vectorsURL)
	return try JSONDecoder().decode(PairingVectorFile.self, from: data).pairing
}

private struct PairingVectorFile: Decodable {
	let pairing: PairingSection
}

private struct PairingSection: Decodable {
	let codeLength: Int
	let alphabet: String
	let domain: String
	let vectors: [PairingVector]

	enum CodingKeys: String, CodingKey {
		case codeLength = "code_length"
		case alphabet
		case domain
		case vectors
	}
}

private struct PairingVector: Decodable {
	let input: String
	let code: String
	let token: String
}

private struct HexError: Error {
	let value: String
}

private func decodeHex(_ hex: String) throws -> [UInt8] {
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
