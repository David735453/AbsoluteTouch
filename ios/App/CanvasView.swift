import AbsoluteTouchCore
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
/// positions against its current bounds, so rotations and multitasking resizes
/// do not change the active area.
final class CanvasView: UIView {
	weak var delegate: CanvasViewDelegate?

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

	override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
		if let pencil = touches.first(where: { $0.type == .pencil }) {
			adopt(pencil)
			return
		}

		guard activeTouch == nil else {
			return
		}
		guard let finger = touches.first(where: { $0.type == .direct }) else {
			return
		}
		adopt(finger)
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

	override func draw(_ rect: CGRect) {
		CanvasIndicator(
			location: lastLocation,
			isContact: lastContact,
			isHover: hoverActive
		).draw()
	}

	@objc private func handleHover(_ recognizer: UIHoverGestureRecognizer) {
		switch recognizer.state {
		case .began, .changed:
			guard activeTouch == nil else {
				return
			}
			let location = recognizer.location(in: self)
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
			guard hoverActive else {
				return
			}
			let location = lastLocation ?? recognizer.location(in: self)
			hoverActive = false
			emitOutOfRange(at: location, isFinger: false)

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

	private func adopt(_ touch: UITouch) {
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
	}

	private func finishTouch() {
		activeTouch = nil
		lastContact = false
		hoverActive = false
		stopDisplayLink()
	}

	private func emitTouch(_ touch: UITouch) {
		let isPencil = touch.type == .pencil
		guard
			let sample = makeSample(
				location: touch.preciseLocation(in: self),
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
		lastLocation = touch.preciseLocation(in: self)
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
		guard bounds.width > 0, bounds.height > 0 else {
			return nil
		}

		return PointerSample(
			x: PointerNormalization.axis(
				Double(location.x),
				extent: Double(bounds.width)
			),
			y: PointerNormalization.axis(
				Double(location.y),
				extent: Double(bounds.height)
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

/// Draws the small ring that shows where the Pencil is.
///
/// Drawing lives outside the canvas class so touch handling and presentation
/// stay separately readable.
private struct CanvasIndicator {
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

/// SwiftUI wrapper around ``CanvasView``.
struct CanvasViewRepresentable: UIViewRepresentable {
	let onSample: @MainActor (PointerSample) -> Void

	func makeUIView(context: Context) -> CanvasView {
		let view = CanvasView(frame: .zero)
		view.delegate = context.coordinator
		return view
	}

	func updateUIView(_ uiView: CanvasView, context: Context) {}

	func makeCoordinator() -> Coordinator {
		Coordinator(onSample: onSample)
	}

	@MainActor
	final class Coordinator: NSObject, CanvasViewDelegate {
		private let onSample: @MainActor (PointerSample) -> Void

		init(onSample: @escaping @MainActor (PointerSample) -> Void) {
			self.onSample = onSample
		}

		func canvasView(_ view: CanvasView, didSample sample: PointerSample) {
			onSample(sample)
		}
	}
}
