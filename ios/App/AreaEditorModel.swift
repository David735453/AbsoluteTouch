import AbsoluteTouchCore
import SwiftUI

/// Bindings and derived values behind the area editor.
///
/// The view stays declarative and every mutation goes through the tested core
/// model, so the editor cannot produce a rectangle outside the canvas.
struct AreaEditorModel {
	/// Saved area.
	let settings: Binding<ActiveAreaSettings>

	/// Shape of the screen canvas.
	let canvasAspect: Double

	/// The rectangle as it will be drawn, already clamped and ratio-fitted.
	var resolved: ActiveArea {
		settings.wrappedValue.resolved(forCanvasAspect: canvasAspect)
	}

	/// Localized one-line description of the rectangle.
	var summary: String {
		settings.wrappedValue.summary(forCanvasAspect: canvasAspect)
	}

	/// Whether the ratio lock is on.
	var isLocked: Bool {
		settings.wrappedValue.isAspectLocked
	}

	/// Selected shape while the lock is on.
	var ratio: AspectRatio {
		settings.wrappedValue.aspectRatio
	}

	/// Turn the ratio lock on or off.
	///
	/// - Parameter locked: The new lock state.
	func setLocked(_ locked: Bool) {
		settings.wrappedValue.setAspectLocked(locked, canvasAspect: canvasAspect)
	}

	/// Choose a preset or custom shape.
	///
	/// - Parameter ratio: The new shape.
	func setRatio(_ ratio: AspectRatio) {
		settings.wrappedValue.setAspectRatio(ratio, canvasAspect: canvasAspect)
	}

	/// Restore the whole screen and release the lock.
	func reset() {
		settings.wrappedValue.resetToFullScreen()
	}

	/// Move the rectangle so its center lands on a canvas-relative point.
	///
	/// - Parameters:
	///   - x: Center X, `0...1`.
	///   - y: Center Y, `0...1`.
	func moveCenter(toX x: Double, y: Double) {
		settings.wrappedValue.moveCenter(
			toX: x,
			y: y,
			canvasAspect: canvasAspect
		)
	}

	/// Height of the resolved rectangle, as a fraction of the canvas.
	var height: Binding<Double> {
		Binding(
			get: { resolved.height },
			set: { settings.wrappedValue.setHeight($0, canvasAspect: canvasAspect) }
		)
	}

	/// Width of the resolved rectangle, as a fraction of the canvas.
	var width: Binding<Double> {
		Binding(
			get: { resolved.width },
			set: { settings.wrappedValue.setWidth($0, canvasAspect: canvasAspect) }
		)
	}

	/// Horizontal center of the rectangle.
	var centerX: Binding<Double> {
		Binding(
			get: { resolved.x + resolved.width / 2 },
			set: {
				moveCenter(
					toX: $0,
					y: resolved.y + resolved.height / 2
				)
			}
		)
	}

	/// Vertical center of the rectangle.
	var centerY: Binding<Double> {
		Binding(
			get: { resolved.y + resolved.height / 2 },
			set: {
				moveCenter(
					toX: resolved.x + resolved.width / 2,
					y: $0
				)
			}
		)
	}

	/// Width part of the selected ratio.
	var ratioWidth: Binding<Int> {
		Binding(
			get: { ratio.width },
			set: { setRatio(AspectRatio(width: $0, height: ratio.height)) }
		)
	}

	/// Height part of the selected ratio.
	var ratioHeight: Binding<Int> {
		Binding(
			get: { ratio.height },
			set: { setRatio(AspectRatio(width: ratio.width, height: $0)) }
		)
	}

	/// A percentage for labels, rounded to a whole number.
	///
	/// - Parameter fraction: A `0...1` fraction.
	/// - Returns: The percentage, `0...100`.
	func percent(_ fraction: Double) -> Int {
		Int((fraction * 100).rounded())
	}

	/// The rectangle in a view's coordinate space.
	///
	/// - Parameters:
	///   - area: The resolved rectangle.
	///   - size: Size of the preview.
	/// - Returns: The rectangle in points.
	func areaRect(_ area: ActiveArea, in size: CGSize) -> CGRect {
		CGRect(
			x: area.x * size.width,
			y: area.y * size.height,
			width: area.width * size.width,
			height: area.height * size.height
		)
	}
}
