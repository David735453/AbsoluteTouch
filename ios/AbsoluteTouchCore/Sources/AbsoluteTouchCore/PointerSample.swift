/// Where the pointer is relative to the drawing surface.
public enum PointerPhase: Sendable, Hashable {
	/// The tip is touching the screen.
	case contact

	/// The Pencil is in range but not touching the screen.
	case hover

	/// The pointer left the active area.
	case outOfRange
}

/// A pointer state ready to be placed in a `SAMPLE` packet.
///
/// The phase is an enum rather than a flag set so that an invalid
/// `CONTACT | HOVER` sample cannot be constructed. Predicted samples are not
/// sent: the desktop never injects them, and the app has no use for them.
public struct PointerSample: Sendable, Hashable {
	/// Normalized X position, `0...65535`.
	public var x: UInt16

	/// Normalized Y position, `0...65535`.
	public var y: UInt16

	/// Pointer phase.
	public var phase: PointerPhase

	/// Normalized pressure. Unused on the reference Pencil (USB-C).
	public var pressure: UInt16

	/// Normalized altitude angle, `0` parallel to the screen, `65535` perpendicular.
	public var altitude: UInt16

	/// Normalized azimuth angle, a full turn across `0...65535`.
	public var azimuth: UInt16

	/// The sample came from a finger rather than the Pencil.
	public var isFinger: Bool

	/// The eraser tool is selected.
	public var isEraser: Bool

	/// The barrel button is pressed.
	public var isBarrel: Bool

	/// Create a pointer sample.
	public init(
		x: UInt16,
		y: UInt16,
		phase: PointerPhase,
		pressure: UInt16 = 0,
		altitude: UInt16 = 0,
		azimuth: UInt16 = 0,
		isFinger: Bool = false,
		isEraser: Bool = false,
		isBarrel: Bool = false
	) {
		self.x = x
		self.y = y
		self.phase = phase
		self.pressure = pressure
		self.altitude = altitude
		self.azimuth = azimuth
		self.isFinger = isFinger
		self.isEraser = isEraser
		self.isBarrel = isBarrel
	}

	/// The wire flag bits for this sample.
	public var flags: PacketFlags {
		var flags: PacketFlags = []
		switch phase {
		case .contact:
			flags.insert(.contact)
		case .hover:
			flags.insert(.hover)
		case .outOfRange:
			flags.insert(.outOfRange)
		}
		if isFinger {
			flags.insert(.finger)
		}
		if isEraser {
			flags.insert(.eraser)
		}
		if isBarrel {
			flags.insert(.barrel)
		}
		return flags
	}
}

/// Conversions from UIKit's point and radian values to wire units.
///
/// These are pure functions so the app's coordinate behavior can be tested on
/// Linux, where UIKit is unavailable.
public enum PointerNormalization {
	/// Convert a point coordinate to `0...65535` over `extent`.
	///
	/// Values outside the active area are clamped. A zero or non-finite extent
	/// yields zero.
	///
	/// - Parameters:
	///   - value: The coordinate in points.
	///   - extent: The width or height of the active area in points.
	/// - Returns: The normalized coordinate.
	public static func axis(_ value: Double, extent: Double) -> UInt16 {
		guard extent.isFinite, extent > 0, value.isFinite else {
			return 0
		}
		return normalizedUnit(value / extent)
	}

	/// Convert UIKit's altitude angle to the wire scale.
	///
	/// - Parameter radians: `0` is parallel to the screen and `π/2` is
	///   perpendicular.
	/// - Returns: `0...65535`, clamped.
	public static func altitude(radians: Double) -> UInt16 {
		normalizedUnit(radians / (.pi / 2))
	}

	/// Convert UIKit's azimuth angle to the wire scale.
	///
	/// - Parameter radians: A full turn in radians, zero along `+x`.
	/// - Returns: `0...65535`, with a full turn wrapping to zero.
	public static func azimuth(radians: Double) -> UInt16 {
		guard radians.isFinite else {
			return 0
		}
		var angle = radians.truncatingRemainder(dividingBy: 2 * .pi)
		if angle < 0 {
			angle += 2 * .pi
		}
		return normalizedUnit(angle / (2 * .pi))
	}

	private static func normalizedUnit(_ fraction: Double) -> UInt16 {
		guard fraction.isFinite else {
			return 0
		}
		let clamped = min(max(fraction, 0), 1)
		return UInt16((clamped * 65_535).rounded())
	}
}
