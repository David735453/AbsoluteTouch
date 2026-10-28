import Foundation

/// A six-character pairing code that expands to the 16-byte wire token.
///
/// Thirty-two hexadecimal characters are unusable on a touch keyboard, so the
/// desktop prints a code instead and both sides expand it. The code *is* the
/// secret: the expansion is deterministic and adds no entropy. Six Crockford
/// base32 characters carry 30 bits, which stops accidental cross-session
/// injection when the desktop rate limits wrong guesses and is nowhere near
/// enough to resist a hostile network.
///
/// The expansion must match `desktop/src/pairing.rs` byte for byte; the shared
/// vectors in `protocol/vectors.json` are the contract.
public enum PairingCode {
	/// Number of characters in a pairing code.
	public static let length = 6

	/// Crockford base32 alphabet: no `I`, `L`, `O`, or `U`.
	public static let alphabet = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"

	/// Domain separator, so this expansion cannot collide with another use.
	private static let domain = Array("absolutetouch.pairing.v1:".utf8)

	/// FNV-1a offset basis, as published by the FNV specification.
	private static let fnvOffsetBasis: UInt64 = 0xcbf2_9ce4_8422_2325

	/// FNV-1a prime, as published by the FNV specification.
	private static let fnvPrime: UInt64 = 0x0000_0100_0000_01b3

	/// Normalize a typed code to its canonical upper-case form.
	///
	/// Whitespace, `-`, and `:` separators are ignored. Lower case is folded to
	/// upper case, and the letters Crockford excludes are folded to the digits
	/// they resemble: `I` and `L` to `1`, `O` to `0`.
	///
	/// - Parameter text: What the user typed.
	/// - Returns: Exactly ``length`` characters from ``alphabet``.
	/// - Throws: ``PairingCodeError`` when the text is not a valid code.
	public static func normalize(_ text: String) throws -> String {
		var code = ""
		code.reserveCapacity(length)
		for character in text {
			if character.isWhitespace || character == "-" || character == ":" {
				continue
			}
			guard character.isASCII else {
				throw PairingCodeError.character(character)
			}
			var folded = Character(character.uppercased())
			switch folded {
			case "I", "L": folded = "1"
			case "O": folded = "0"
			default: break
			}
			guard alphabet.contains(folded) else {
				throw PairingCodeError.character(character)
			}
			code.append(folded)
		}

		guard code.count == length else {
			throw PairingCodeError.length(code.count)
		}
		return code
	}

	/// Expand a pairing code into the token sent in `HELLO`.
	///
	/// - Parameter text: What the user typed.
	/// - Returns: The 16-byte token the desktop expects.
	/// - Throws: ``PairingCodeError`` when the text is not a valid code.
	public static func token(from text: String) throws -> PairingToken {
		let code = try normalize(text)
		return PairingToken(validatedBytes: expand(code))
	}

	/// A new random code from the system random number generator.
	///
	/// Each character consumes the low five bits of one random byte. The
	/// alphabet has 32 symbols, so every value is equally likely.
	public static func random() -> String {
		var generator = SystemRandomNumberGenerator()
		let symbols = Array(alphabet)
		var code = ""
		code.reserveCapacity(length)
		for _ in 0..<length {
			code.append(symbols[Int(UInt8.random(in: .min ... .max, using: &generator) & 0x1F)])
		}
		return code
	}

	/// Expand a normalized code with two FNV-1a passes.
	static func expand(_ code: String) -> [UInt8] {
		let first = domain + Array(code.utf8)
		let high = fnv1a64(first)
		let low = fnv1a64(first + withUnsafeBytes(of: high.littleEndian, Array.init))

		var token = withUnsafeBytes(of: high.littleEndian, Array.init)
		token.append(contentsOf: withUnsafeBytes(of: low.littleEndian, Array.init))
		return token
	}

	/// FNV-1a over the 64-bit variant.
	private static func fnv1a64(_ bytes: [UInt8]) -> UInt64 {
		var hash = fnvOffsetBasis
		for byte in bytes {
			hash ^= UInt64(byte)
			hash = hash &* fnvPrime
		}
		return hash
	}
}

/// Why a pairing code was rejected.
public enum PairingCodeError: Error, Sendable, Equatable {
	/// The code did not have exactly ``PairingCode/length`` characters.
	case length(Int)
	/// The code contained a character outside ``PairingCode/alphabet``.
	case character(Character)
}

extension PairingCodeError: LocalizedError {
	/// A human-readable description in the device's language.
	public var errorDescription: String? {
		switch self {
		case .length(let count):
			Localization.current.format(.pairingCodeLength, PairingCode.length, count)
		case .character(let character):
			Localization.current.format(.pairingCodeCharacter, String(character))
		}
	}
}
