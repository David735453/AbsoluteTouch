/// A width-to-height ratio such as 16:9.
///
/// The ratio describes the rectangle on the glass, not a fraction of the
/// iPad's canvas, so it survives rotation: a 16:9 area stays wide and short
/// in both orientations.
public struct AspectRatio: Sendable, Hashable {
	/// Horizontal part of the ratio, at least `1`.
	public let width: Int

	/// Vertical part of the ratio, at least `1`.
	public let height: Int

	/// Create a ratio, clamping both parts to at least one.
	///
	/// - Parameters:
	///   - width: Horizontal part.
	///   - height: Vertical part.
	public init(width: Int, height: Int) {
		self.width = max(width, 1)
		self.height = max(height, 1)
	}

	/// The desktop screen's shape, and the default for a small drawing area.
	public static let sixteenByNine = AspectRatio(width: 16, height: 9)

	/// A tall alternative for Pencil-only setups.
	public static let nineBySixteen = AspectRatio(width: 9, height: 16)

	/// Presets offered in the area editor, widest first.
	public static let presets: [AspectRatio] = [
		.sixteenByNine,
		AspectRatio(width: 4, height: 3),
		AspectRatio(width: 1, height: 1),
		AspectRatio(width: 3, height: 4),
		AspectRatio(width: 2, height: 3),
		.nineBySixteen,
	]

	/// Width divided by height.
	public var value: Double {
		Double(width) / Double(height)
	}

	/// The ratio as `16:9`, for chips and summaries.
	public var label: String {
		"\(width):\(height)"
	}

	/// The width fraction that gives this ratio at `heightFraction`.
	///
	/// - Parameters:
	///   - heightFraction: Height as a fraction of the canvas, `0...1`.
	///   - canvasAspect: Canvas width divided by canvas height.
	/// - Returns: Width as a fraction of the canvas.
	public func widthFraction(forHeight heightFraction: Double, canvasAspect: Double) -> Double {
		guard canvasAspect.isFinite, canvasAspect > 0 else {
			return heightFraction
		}
		return heightFraction * value / canvasAspect
	}

	/// The height fraction that gives this ratio at `widthFraction`.
	///
	/// - Parameters:
	///   - widthFraction: Width as a fraction of the canvas, `0...1`.
	///   - canvasAspect: Canvas width divided by canvas height.
	/// - Returns: Height as a fraction of the canvas.
	public func heightFraction(forWidth widthFraction: Double, canvasAspect: Double) -> Double {
		guard canvasAspect.isFinite, canvasAspect > 0 else {
			return widthFraction
		}
		return widthFraction * canvasAspect / value
	}
}

/// The rectangle of the iPad screen that accepts Pencil input.
///
/// Fractions of the canvas are stored instead of points so the area keeps its
/// place when the iPad rotates or enters Split View. The desktop still
/// receives `0...65535` across this rectangle, so a small area means less hand
/// travel and a more sensitive cursor.
public struct ActiveArea: Sendable, Hashable {
	/// Smallest side, as a fraction of the canvas, so the area cannot vanish.
	public static let minimumDimension = 0.05

	/// The whole screen.
	public static let full = ActiveArea(x: 0, y: 0, width: 1, height: 1)

	/// Left edge as a fraction of the canvas.
	public var x: Double

	/// Top edge as a fraction of the canvas.
	public var y: Double

	/// Width as a fraction of the canvas.
	public var width: Double

	/// Height as a fraction of the canvas.
	public var height: Double

	/// Create an area from canvas fractions.
	///
	/// The values are clamped by ``clamped``; this initializer does not
	/// validate them.
	///
	/// - Parameters:
	///   - x: Left edge.
	///   - y: Top edge.
	///   - width: Width.
	///   - height: Height.
	public init(x: Double, y: Double, width: Double, height: Double) {
		self.x = x
		self.y = y
		self.width = width
		self.height = height
	}

	/// The same rectangle, forced inside the canvas.
	///
	/// Non-finite values fall back to the full screen and sides never shrink
	/// below ``minimumDimension``.
	public var clamped: ActiveArea {
		let width = clampDimension(self.width)
		let height = clampDimension(self.height)
		return ActiveArea(
			x: clampOrigin(x, extent: width),
			y: clampOrigin(y, extent: height),
			width: width,
			height: height
		)
	}

	/// Area as a fraction of the canvas, `0.05...1`.
	public var screenFraction: Double {
		let area = clamped
		return area.width * area.height
	}

	/// Whether a canvas-relative point is inside the area.
	///
	/// - Parameters:
	///   - x: Horizontal position, `0...1`.
	///   - y: Vertical position, `0...1`.
	/// - Returns: `true` when the point is inside, edges included.
	public func contains(x: Double, y: Double) -> Bool {
		let area = clamped
		return x >= area.x
			&& y >= area.y
			&& x <= area.x + area.width
			&& y <= area.y + area.height
	}

	private func clampDimension(_ value: Double) -> Double {
		guard value.isFinite else {
			return 1
		}
		return min(max(value, ActiveArea.minimumDimension), 1)
	}

	private func clampOrigin(_ value: Double, extent: Double) -> Double {
		guard value.isFinite else {
			return 0
		}
		return min(max(value, 0), 1 - extent)
	}
}

/// The saved shape of the drawing area plus its aspect lock.
///
/// The stored rectangle is only a starting point: when the lock is on,
/// ``resolved(forCanvasAspect:)`` derives the width from the height and the
/// current canvas, which is what keeps the rectangle at the chosen ratio after
/// a rotation.
public struct ActiveAreaSettings: Sendable, Hashable {
	/// Stored rectangle, in canvas fractions.
	public var area: ActiveArea

	/// Whether the width follows ``aspectRatio``.
	public var isAspectLocked: Bool

	/// Shape the rectangle keeps while the lock is on.
	public var aspectRatio: AspectRatio

	/// Full screen, locked to 16:9.
	///
	/// A locked full screen resolves to a full-width 16:9 strip, which matches
	/// the desktop screen and can be shrunk from the editor.
	public static let `default` = ActiveAreaSettings(
		area: .full,
		isAspectLocked: true,
		aspectRatio: .sixteenByNine
	)

	/// Create settings from a stored rectangle.
	///
	/// - Parameters:
	///   - area: Stored rectangle; clamped when resolved.
	///   - isAspectLocked: Whether the width follows the ratio.
	///   - aspectRatio: Shape used while locked.
	public init(
		area: ActiveArea = .full,
		isAspectLocked: Bool = true,
		aspectRatio: AspectRatio = .sixteenByNine
	) {
		self.area = area
		self.isAspectLocked = isAspectLocked
		self.aspectRatio = aspectRatio
	}

	/// The rectangle to draw and hit-test on a canvas of a given shape.
	///
	/// Unlocked settings return the clamped stored rectangle. Locked settings
	/// keep the stored height and center, derive the width from
	/// ``aspectRatio``, and scale down if the result would not fit.
	///
	/// - Parameter canvasAspect: Canvas width divided by canvas height.
	/// - Returns: A rectangle inside the canvas with the requested ratio.
	public func resolved(forCanvasAspect canvasAspect: Double) -> ActiveArea {
		let base = area.clamped
		guard isAspectLocked, canvasAspect.isFinite, canvasAspect > 0 else {
			return base
		}

		var height = base.height
		var width = aspectRatio.widthFraction(forHeight: height, canvasAspect: canvasAspect)
		if width > 1 {
			height *= 1 / width
			width = 1
		}

		let centerX = base.x + base.width / 2
		let centerY = base.y + base.height / 2
		return ActiveArea(
			x: centerX - width / 2,
			y: centerY - height / 2,
			width: width,
			height: height
		).clamped
	}

	/// Turn the aspect lock on or off without moving the rectangle.
	///
	/// Turning the lock on immediately fits the rectangle to the ratio.
	///
	/// - Parameters:
	///   - locked: The new lock state.
	///   - canvasAspect: Canvas width divided by canvas height.
	public mutating func setAspectLocked(_ locked: Bool, canvasAspect: Double) {
		area = resolved(forCanvasAspect: canvasAspect)
		isAspectLocked = locked
		area = resolved(forCanvasAspect: canvasAspect)
	}

	/// Choose a new ratio, keeping the height and center.
	///
	/// - Parameters:
	///   - ratio: The new shape.
	///   - canvasAspect: Canvas width divided by canvas height.
	public mutating func setAspectRatio(_ ratio: AspectRatio, canvasAspect: Double) {
		area = resolved(forCanvasAspect: canvasAspect)
		aspectRatio = ratio
		area = resolved(forCanvasAspect: canvasAspect)
	}

	/// Set the height fraction, growing or shrinking around the center.
	///
	/// While the lock is on the width follows the ratio; otherwise it is
	/// unchanged.
	///
	/// - Parameters:
	///   - fraction: Height as a fraction of the canvas.
	///   - canvasAspect: Canvas width divided by canvas height.
	public mutating func setHeight(_ fraction: Double, canvasAspect: Double) {
		let current = resolved(forCanvasAspect: canvasAspect)
		let centerY = current.y + current.height / 2
		var next = current
		next.height = clampFraction(fraction)
		next.y = centerY - next.height / 2
		area = next.clamped
	}

	/// Set the width fraction, growing or shrinking around the center.
	///
	/// - Parameters:
	///   - fraction: Width as a fraction of the canvas.
	///   - canvasAspect: Canvas width divided by canvas height.
	public mutating func setWidth(_ fraction: Double, canvasAspect: Double) {
		let current = resolved(forCanvasAspect: canvasAspect)
		let centerX = current.x + current.width / 2
		var next = current
		next.width = clampFraction(fraction)
		next.x = centerX - next.width / 2
		area = next.clamped
	}

	/// Move the rectangle so its center lands on a canvas-relative point.
	///
	/// - Parameters:
	///   - x: Center X, `0...1`.
	///   - y: Center Y, `0...1`.
	///   - canvasAspect: Canvas width divided by canvas height.
	public mutating func moveCenter(toX x: Double, y: Double, canvasAspect: Double) {
		let current = resolved(forCanvasAspect: canvasAspect)
		area =
			ActiveArea(
				x: x - current.width / 2,
				y: y - current.height / 2,
				width: current.width,
				height: current.height
			).clamped
	}

	/// Restore the whole screen and switch the lock off.
	public mutating func resetToFullScreen() {
		area = .full
		isAspectLocked = false
	}

	private func clampFraction(_ value: Double) -> Double {
		guard value.isFinite else {
			return ActiveArea.minimumDimension
		}
		return min(max(value, ActiveArea.minimumDimension), 1)
	}
}

extension ActiveAreaSettings {
	/// A short description of the resolved area for the home card and editor.
	///
	/// A locked area reports its ratio and height; an unlocked one reports both
	/// sides, or "Full screen" when it covers everything.
	///
	/// - Parameters:
	///   - canvasAspect: Canvas width divided by canvas height.
	///   - localization: Language for the description.
	/// - Returns: Localized text such as `16:9 · 60% of the height`.
	public func summary(
		forCanvasAspect canvasAspect: Double,
		localization: Localization = .current
	) -> String {
		let area = resolved(forCanvasAspect: canvasAspect)
		guard isAspectLocked else {
			guard area != .full else {
				return localization.text(.drawingAreaFullScreen)
			}
			return localization.format(
				.drawingAreaSizeSummary,
				percent(area.width),
				percent(area.height)
			)
		}
		return localization.format(
			.drawingAreaSummary,
			aspectRatio.label,
			percent(area.height)
		)
	}

	private func percent(_ fraction: Double) -> Int {
		Int((fraction * 100).rounded())
	}
}
