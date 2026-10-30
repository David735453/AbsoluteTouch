import AbsoluteTouchCore
import SwiftUI
import UIKit

/// Receives pointer samples from a ``CanvasView``.
@MainActor
protocol CanvasViewDelegate: AnyObject {
	func canvasView(_ view: CanvasView, didSample sample: PointerSample)
}

/// UIKit canvas that turns Pencil and finger input into wire samples.
///
/// UIKit is required here because SwiftUI gestures do not expose coalesced
/// touches, stylus altitude and azimuth, or hover poses. The view normalizes
/// positions against its active area, so a small area means the whole desktop
/// is reachable without crossing the screen, and rotations or multitasking
/// resizes do not change the chosen shape.
final class CanvasView: UIView {
	weak var delegate: CanvasViewDelegate?

	/// Part of the bounds that accepts input, as fractions of the bounds.
	///
	/// ``StreamingView`` resolves the user's settings against the canvas
	/// aspect before assigning it, so this rectangle already has the requested
	/// ratio.
	var activeArea = ActiveArea.full {
		didSet {
			guard activeArea != oldValue else {
				return
			}
			setNeedsDisplay()
		}
	}

	private var activeTouch: UITouch?
	private var lastLocation: CGPoint?
	private var lastContact = false
	private var hoverActive = false
	private var displayLink: CADisplayLink?
	private var lastInputTime: CFTimeInterval = 0

	override init(frame: CGRect) {
		super.init(frame: frame)
		isMultipleTouchEnabled = true
		isOpaque = true
		backgroundColor = UIColor(white: 0.05, alpha: 1)
		contentMode = .redraw

		let hover = UIHoverGestureRecognizer(
			target: self,
			action: #selector(handleHover(_:))
		)
		hover.allowedTouchTypes = [UITouch.TouchType.pencil.rawValue as NSNumber]
		hover.cancelsTouchesInView = false
		hover.delaysTouchesBegan = false
		addGestureRecognizer(hover)
	}

	@available(*, unavailable)
	required init?(coder: NSCoder) {
		fatalError("init(coder:) is not supported")
	}

	override func didMoveToWindow() {
		super.didMoveToWindow()
		if window == nil {
			stopDisplayLink()
		}
	}

	override func draw(_ rect: CGRect) {
		CanvasAreaOverlay(bounds: bounds, area: areaRect).draw()
		CanvasIndicator(
			location: lastLocation,
			isContact: lastContact,
			isHover: hoverActive
		).draw()
	}
}

// MARK: - Touch handling

extension CanvasView {
	override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
		for touch in touches where touch.type == .pencil {
			if adopt(touch) {
				return
			}
		}

		guard activeTouch == nil else {
			return
		}
		guard let finger = touches.first(where: { $0.type == .direct }) else {
			return
		}
		_ = adopt(finger)
	}

	override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
		guard let touch = touches.first(where: { $0 === activeTouch }) else {
			return
		}

		var samples = event?.coalescedTouches(for: touch) ?? []
		if samples.isEmpty {
			samples = [touch]
		}
		for sample in samples {
			emitTouch(sample)
		}
		lastInputTime = CACurrentMediaTime()
	}

	override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
		guard let touch = touches.first(where: { $0 === activeTouch }) else {
			return
		}
		emitOutOfRange(
			at: touch.preciseLocation(in: self),
			isFinger: touch.type == .direct
		)
		finishTouch()
	}

	override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
		guard let touch = touches.first(where: { $0 === activeTouch }) else {
			return
		}
		emitOutOfRange(
			at: touch.preciseLocation(in: self),
			isFinger: touch.type == .direct
		)
		finishTouch()
	}
}

// MARK: - Hover and display link

extension CanvasView {
	@objc private func handleHover(_ recognizer: UIHoverGestureRecognizer) {
		switch recognizer.state {
		case .began, .changed:
			guard activeTouch == nil else {
				return
			}
			let location = recognizer.location(in: self)
			guard isInside(location) else {
				leaveHover(at: location)
				return
			}
			guard
				let sample = makeSample(
					location: location,
					phase: .hover,
					altitudeRadians: Double(recognizer.altitudeAngle),
					azimuthRadians: Double(recognizer.azimuthAngle(in: self)),
					isFinger: false
				)
			else {
				return
			}
			hoverActive = true
			lastContact = false
			lastLocation = location
			setNeedsDisplay()
			delegate?.canvasView(self, didSample: sample)

		case .ended, .cancelled:
			leaveHover(at: lastLocation ?? recognizer.location(in: self))

		default:
			break
		}
	}

	@objc private func displayLinkFired(_ link: CADisplayLink) {
		guard let touch = activeTouch else {
			stopDisplayLink()
			return
		}
		guard CACurrentMediaTime() - lastInputTime >= 0.018 else {
			return
		}
		emitTouch(touch)
		lastInputTime = CACurrentMediaTime()
	}

	private func startDisplayLink() {
		guard displayLink == nil else {
			return
		}
		let link = CADisplayLink(
			target: self,
			selector: #selector(displayLinkFired(_:))
		)
		link.add(to: .main, forMode: .common)
		displayLink = link
	}

	private func stopDisplayLink() {
		displayLink?.invalidate()
		displayLink = nil
	}
}

// MARK: - Sampling

extension CanvasView {
	/// Adopt a touch that starts inside the active area.
	///
	/// A touch that starts outside is ignored entirely: the desktop never
	/// sees a contact that was aimed at a dimmed part of the screen.
	///
	/// - Parameter touch: The new touch.
	/// - Returns: `true` when the touch became the active one.
	@discardableResult
	private func adopt(_ touch: UITouch) -> Bool {
		guard isInside(touch.preciseLocation(in: self)) else {
			return false
		}
		if let activeTouch, activeTouch !== touch {
			emitOutOfRange(
				at: activeTouch.preciseLocation(in: self),
				isFinger: activeTouch.type == .direct
			)
		}
		activeTouch = touch
		hoverActive = false
		lastInputTime = CACurrentMediaTime()
		emitTouch(touch)
		startDisplayLink()
		return true
	}

	private func finishTouch() {
		activeTouch = nil
		lastContact = false
		hoverActive = false
		stopDisplayLink()
	}

	/// End hover, sending one out-of-range sample if the Pencil was inside.
	///
	/// - Parameter location: Where the Pencil left, in view points.
	private func leaveHover(at location: CGPoint) {
		guard hoverActive else {
			return
		}
		hoverActive = false
		emitOutOfRange(at: location, isFinger: false)
	}

	private func emitTouch(_ touch: UITouch) {
		let isPencil = touch.type == .pencil
		let location = touch.preciseLocation(in: self)
		guard isInside(location) else {
			// Leaving the area lifts the contact but keeps the touch, so
			// coming back in starts a new stroke instead of being ignored
			// until the Pencil is lifted.
			if lastContact {
				emitOutOfRange(at: location, isFinger: touch.type == .direct)
			}
			return
		}
		guard
			let sample = makeSample(
				location: location,
				phase: .contact,
				altitudeRadians: isPencil ? Double(touch.altitudeAngle) : nil,
				azimuthRadians: isPencil
					? Double(touch.azimuthAngle(in: self))
					: nil,
				isFinger: touch.type == .direct
			)
		else {
			return
		}
		lastContact = true
		hoverActive = false
		lastLocation = location
		setNeedsDisplay()
		delegate?.canvasView(self, didSample: sample)
	}

	private func emitOutOfRange(at location: CGPoint, isFinger: Bool) {
		guard
			let sample = makeSample(
				location: location,
				phase: .outOfRange,
				altitudeRadians: nil,
				azimuthRadians: nil,
				isFinger: isFinger
			)
		else {
			return
		}
		lastContact = false
		hoverActive = false
		lastLocation = location
		setNeedsDisplay()
		delegate?.canvasView(self, didSample: sample)
	}

	private func makeSample(
		location: CGPoint,
		phase: PointerPhase,
		altitudeRadians: Double?,
		azimuthRadians: Double?,
		isFinger: Bool
	) -> PointerSample? {
		let area = areaRect
		guard area.width > 0, area.height > 0 else {
			return nil
		}

		return PointerSample(
			x: PointerNormalization.axis(
				Double(location.x - area.minX),
				extent: Double(area.width)
			),
			y: PointerNormalization.axis(
				Double(location.y - area.minY),
				extent: Double(area.height)
			),
			phase: phase,
			pressure: 0,
			altitude: altitudeRadians.map {
				PointerNormalization.altitude(radians: $0)
			} ?? 0,
			azimuth: azimuthRadians.map {
				PointerNormalization.azimuth(radians: $0)
			} ?? 0,
			isFinger: isFinger
		)
	}

	/// The active area in view points.
	private var areaRect: CGRect {
		CGRect(
			x: bounds.width * activeArea.x,
			y: bounds.height * activeArea.y,
			width: bounds.width * activeArea.width,
			height: bounds.height * activeArea.height
		)
	}

	/// Whether a view-space point is inside the active area.
	///
	/// - Parameter location: A point in view coordinates.
	/// - Returns: `true` when the point accepts input.
	private func isInside(_ location: CGPoint) -> Bool {
		areaRect.contains(location)
	}
}
