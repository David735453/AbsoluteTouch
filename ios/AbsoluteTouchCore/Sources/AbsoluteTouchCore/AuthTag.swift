/// An eight-byte authentication tag.
///
/// The tag is reserved in protocol version 1 and is all zeros. Future versions
/// may carry a truncated MAC over the packet.
public struct AuthTag: Sendable, Hashable {
    /// Number of bytes in a tag.
    public static let byteCount = ProtocolSpec.authTagSize

    /// The tag bytes; always exactly ``byteCount`` bytes.
    public let bytes: [UInt8]

    /// The all-zero tag used by protocol version 1.
    public static let zero = AuthTag()

    /// Create an all-zero tag.
    public init() {
        self.bytes = [UInt8](repeating: 0, count: Self.byteCount)
    }

    /// Create a tag from exactly eight bytes.
    ///
    /// - Parameter bytes: The tag bytes.
    /// - Returns: `nil` when `bytes` does not contain exactly eight bytes.
    public init?(bytes: [UInt8]) {
        guard bytes.count == Self.byteCount else {
            return nil
        }
        self.bytes = bytes
    }

    init(validatedBytes bytes: [UInt8]) {
        self.bytes = bytes
    }
}
