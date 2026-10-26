/// Flag bits stored in the packet header.
///
/// Pointer flags occupy bits 0 through 6 and are valid in `SAMPLE` packets.
/// Capability flags occupy bits 8 through 11 and are only valid in `HELLO` and
/// `HELLO_ACK` packets. All other bits are reserved and must be zero.
public struct PacketFlags: OptionSet, Sendable, Hashable {
	/// The raw 16-bit flag value.
	public let rawValue: UInt16

	/// Create a flag set from a raw value.
	///
	/// - Parameter rawValue: The 16-bit flag value.
	public init(rawValue: UInt16) {
		self.rawValue = rawValue
	}

	/// The tip is in contact with the screen.
	public static let contact = PacketFlags(rawValue: 1 << 0)

	/// The Pencil is in range but not touching the screen.
	public static let hover = PacketFlags(rawValue: 1 << 1)

	/// The eraser tool is selected.
	public static let eraser = PacketFlags(rawValue: 1 << 2)

	/// The barrel button is pressed.
	public static let barrel = PacketFlags(rawValue: 1 << 3)

	/// The sample is a prediction of a future position.
	public static let predicted = PacketFlags(rawValue: 1 << 4)

	/// The pointer left the active area.
	public static let outOfRange = PacketFlags(rawValue: 1 << 5)

	/// The sample originated from a finger.
	public static let finger = PacketFlags(rawValue: 1 << 6)

	/// The sender supports pressure.
	public static let capabilityPressure = PacketFlags(rawValue: 1 << 8)

	/// The sender supports tilt.
	public static let capabilityTilt = PacketFlags(rawValue: 1 << 9)

	/// The sender supports hover.
	public static let capabilityHover = PacketFlags(rawValue: 1 << 10)

	/// The sender supports twist.
	public static let capabilityTwist = PacketFlags(rawValue: 1 << 11)

	/// Every pointer flag defined by version 1.
	public static let pointerMask: PacketFlags = [
		.contact, .hover, .eraser, .barrel, .predicted, .outOfRange, .finger,
	]

	/// Every capability flag defined by version 1.
	public static let capabilityMask: PacketFlags = [
		.capabilityPressure, .capabilityTilt, .capabilityHover, .capabilityTwist,
	]
}
