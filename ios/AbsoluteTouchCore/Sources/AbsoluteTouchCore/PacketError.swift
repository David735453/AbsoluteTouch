import Foundation

/// Errors produced while decoding or encoding a packet.
public enum PacketError: Error, Equatable, Sendable {
    /// The buffer was not exactly ``ProtocolSpec/packetSize`` bytes.
    case invalidLength(Int)

    /// The magic value did not match.
    case badMagic(UInt16)

    /// The protocol version is not supported.
    case unsupportedVersion(UInt8)

    /// The packet type is unknown.
    case unknownType(UInt8)

    /// A session token was not exactly ``ProtocolSpec/tokenSize`` bytes.
    case invalidTokenLength(Int)
}

extension PacketError: LocalizedError {
    /// A human-readable description of the error.
    public var errorDescription: String? {
        switch self {
        case .invalidLength(let length):
            "Packet must be exactly \(ProtocolSpec.packetSize) bytes, got \(length)."
        case .badMagic(let magic):
            "Bad magic 0x\(String(format: "%04X", magic))."
        case .unsupportedVersion(let version):
            "Unsupported protocol version \(version)."
        case .unknownType(let type):
            "Unknown packet type \(type)."
        case .invalidTokenLength(let length):
            "Token must be exactly \(ProtocolSpec.tokenSize) bytes, got \(length)."
        }
    }
}
