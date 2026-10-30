import UIKit

/// Dims the part of the canvas that does not accept input.
///
/// A full-screen area draws nothing, so the default look stays a plain black
/// canvas.
struct CanvasAreaOverlay {
	let bounds: CGRect
	let area: CGRect

	func draw() {
		guard
			area.minX > bounds.minX
				|| area.minY > bounds.minY
				|| area.maxX < bounds.maxX
				|| area.maxY < bounds.maxY
		else {
			return
		}

		let dim = UIBezierPath(rect: bounds)
		dim.append(UIBezierPath(rect: area))
		dim.usesEvenOddFillRule = true
		UIColor.black.withAlphaComponent(0.55).setFill()
		dim.fill()

		UIColor.white.withAlphaComponent(0.3).setStroke()
		let outline = UIBezierPath(rect: area)
		outline.lineWidth = 1
		outline.stroke()
	}
}

/// Draws the small ring that shows where the Pencil is.
///
/// Drawing lives outside the canvas class so touch handling and presentation
/// stay separately readable.
struct CanvasIndicator {
	let location: CGPoint?
	let isContact: Bool
	let isHover: Bool

	func draw() {
		guard let location else {
			return
		}

		if isContact {
			UIColor.white.withAlphaComponent(0.7).setStroke()
			let ring = UIBezierPath(
				ovalIn: CGRect(
					x: location.x - 9,
					y: location.y - 9,
					width: 18,
					height: 18
				)
			)
			ring.lineWidth = 1.5
			ring.stroke()

			UIColor.white.setFill()
			let dot = UIBezierPath(
				ovalIn: CGRect(
					x: location.x - 3,
					y: location.y - 3,
					width: 6,
					height: 6
				)
			)
			dot.fill()
		} else if isHover {
			UIColor.white.withAlphaComponent(0.5).setStroke()
			let ring = UIBezierPath(
				ovalIn: CGRect(
					x: location.x - 10,
					y: location.y - 10,
					width: 20,
					height: 20
				)
			)
			ring.lineWidth = 1.5
			ring.stroke()
		}
	}
}
