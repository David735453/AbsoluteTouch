/// A 16-byte pairing token shared with the desktop.
///
/// The token is sent in clear text in `HELLO` and `HELLO_ACK`. It prevents
/// accidental cross-session mixing and casual unauthorized injection; it is not
/// encryption. See `docs/protocol.md`.
public struct PairingToken: Sendable, Hashable {
	/// Number of bytes in a token.
	public static let byteCount = ProtocolSpec.tokenSize

	/// The token bytes, always exactly ``byteCount`` bytes.
	public let bytes: [UInt8]

	/// Create a token from exactly 16 bytes.
	///
	/// - Parameter bytes: The token bytes.
	/// - Returns: `nil` when `bytes` does not contain exactly 16 bytes.
	public init?(bytes: [UInt8]) {
		guard bytes.count == Self.byteCount else {
			return nil
		}
		self.bytes = bytes
	}

	/// Create a token from 32 hexadecimal characters.
	///
	/// Whitespace, `-`, and `:` separators are ignored, so a token copied from
	/// the desktop or formatted in groups is accepted. Hex digits may be upper
	/// or lower case.
	///
	/// - Parameter hex: The textual token.
	/// - Returns: `nil` when the text does not contain exactly 32 hex digits.
	public init?(hex: String) {
		let cleaned = hex.filter {
			!$0.isWhitespace && $0 != "-" && $0 != ":"
		}
		let digits = Array(cleaned.utf8)
		guard digits.count == Self.byteCount * 2 else {
			return nil
		}

		var bytes: [UInt8] = []
		bytes.reserveCapacity(Self.byteCount)
		for index in stride(from: 0, to: digits.count, by: 2) {
			guard let high = Self.nibble(digits[index]),
				let low = Self.nibble(digits[index + 1])
			else {
				return nil
			}
			bytes.append((high << 4) | low)
		}
		self.bytes = bytes
	}

	/// Create a token from a pairing code typed by the user.
	///
	/// - Parameter code: A code such as `K7M2QX`; separators and the letters
	///   Crockford excludes are accepted.
	/// - Throws: ``PairingCodeError`` when the text is not a valid code.
	public init(code: String) throws {
		self = try PairingCode.token(from: code)
	}

	/// A new random token from the system random number generator.
	public static func random() -> PairingToken {
		var generator = SystemRandomNumberGenerator()
		let bytes = (0..<byteCount).map { _ in
			UInt8.random(in: .min ... .max, using: &generator)
		}
		// The memberwise initializer is unavailable because the failable
		// initializer is public; the count is fixed here.
		return PairingToken(validatedBytes: bytes)
	}

	/// The token as 32 lowercase hexadecimal characters.
	public var hex: String {
		let digits = Array("0123456789abcdef")
		var text = ""
		text.reserveCapacity(Self.byteCount * 2)
		for byte in bytes {
			text.append(digits[Int(byte >> 4)])
			text.append(digits[Int(byte & 0x0F)])
		}
		return text
	}

	init(validatedBytes bytes: [UInt8]) {
		precondition(bytes.count == Self.byteCount)
		self.bytes = bytes
	}

	private static func nibble(_ byte: UInt8) -> UInt8? {
		let zero = UInt8(ascii: "0")
		let nine = UInt8(ascii: "9")
		let lowerA = UInt8(ascii: "a")
		let lowerF = UInt8(ascii: "f")
		let upperA = UInt8(ascii: "A")
		let upperF = UInt8(ascii: "F")

		if byte >= zero && byte <= nine {
			return byte - zero
		}
		if byte >= lowerA && byte <= lowerF {
			return byte - lowerA + 10
		}
		if byte >= upperA && byte <= upperF {
			return byte - upperA + 10
		}
		return nil
	}
}
