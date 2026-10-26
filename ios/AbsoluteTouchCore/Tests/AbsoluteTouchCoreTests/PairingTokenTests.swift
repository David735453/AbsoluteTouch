import AbsoluteTouchCore
import Testing

@Suite("Pairing token")
struct PairingTokenTests {
	@Test("Hex round-trips as lowercase")
	func hexRoundTrips() throws {
		let token = try #require(
			PairingToken(hex: "000102030405060708090A0B0C0D0E0F")
		)

		#expect(token.hex == "000102030405060708090a0b0c0d0e0f")
		#expect(token.bytes == (0..<16).map { UInt8($0) })
	}

	@Test("Separators and mixed case are accepted")
	func separatorsAreAccepted() throws {
		let token = try #require(
			PairingToken(hex: "0001-0203 0405:0607 0809-0a0b:0c0d 0e0f")
		)

		#expect(token.hex == "000102030405060708090a0b0c0d0e0f")
	}

	@Test("Wrong length and non-hex text are rejected")
	func invalidTextIsRejected() {
		#expect(PairingToken(hex: "") == nil)
		#expect(PairingToken(hex: "00") == nil)
		#expect(PairingToken(hex: String(repeating: "0", count: 31)) == nil)
		#expect(PairingToken(hex: String(repeating: "0", count: 33)) == nil)
		#expect(PairingToken(hex: String(repeating: "z", count: 32)) == nil)
	}

	@Test("A byte array must be exactly 16 bytes")
	func byteLengthIsChecked() {
		#expect(PairingToken(bytes: []) == nil)
		#expect(PairingToken(bytes: [UInt8](repeating: 1, count: 15)) == nil)
		#expect(PairingToken(bytes: [UInt8](repeating: 1, count: 17)) == nil)
		#expect(PairingToken(bytes: [UInt8](repeating: 1, count: 16)) != nil)
	}

	@Test("Random tokens have the wire size")
	func randomTokensAreWellFormed() {
		let token = PairingToken.random()

		#expect(token.bytes.count == PairingToken.byteCount)
		#expect(token.hex.count == PairingToken.byteCount * 2)
		#expect(PairingToken(hex: token.hex) == token)
	}
}
