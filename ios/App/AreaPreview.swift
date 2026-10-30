import AbsoluteTouchCore
import SwiftUI

/// Canvas-shaped preview of the drawing area.
///
/// The dimmed part is ignored while streaming; dragging moves the rectangle.
/// The page also offers position sliders, so the gesture is only a shortcut.
struct AreaPreview: View {
	/// Editor state and the geometry helpers.
	let model: AreaEditorModel

	@State private var dragStart: CGPoint?

	var body: some View {
		GeometryReader { proxy in
			let size = proxy.size
			let area = model.resolved

			Canvas { context, canvasSize in
				let bounds = CGRect(origin: .zero, size: canvasSize)
				context.fill(
					Path(bounds),
					with: .color(Color(uiColor: .secondarySystemBackground))
				)

				let drawn = model.areaRect(area, in: canvasSize)
				guard drawn != bounds else {
					return
				}

				var outside = Path(bounds)
				outside.addRect(drawn)
				context.fill(
					outside,
					with: .color(.black.opacity(0.35)),
					style: FillStyle(eoFill: true)
				)
				context.stroke(
					Path(drawn.insetBy(dx: 1, dy: 1)),
					with: .color(Color.accentColor),
					lineWidth: 2
				)
			}
			.contentShape(Rectangle())
			.gesture(dragGesture(in: size, area: area))
			.accessibilityElement(children: .ignore)
			.accessibilityLabel(Copy.text(.drawingArea))
			.accessibilityValue(model.summary)
			.overlay {
				RoundedRectangle(cornerRadius: 16, style: .continuous)
					.strokeBorder(.quaternary, lineWidth: 1)
			}
			.clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
		}
		.aspectRatio(model.canvasAspect, contentMode: .fit)
		.frame(maxWidth: .infinity, maxHeight: 420)
	}

	private func dragGesture(in size: CGSize, area: ActiveArea) -> some Gesture {
		DragGesture(minimumDistance: 1)
			.onChanged { value in
				guard size.width > 0, size.height > 0 else {
					return
				}
				if dragStart == nil {
					let rect = model.areaRect(area, in: size)
					dragStart = CGPoint(x: rect.midX, y: rect.midY)
				}
				guard let start = dragStart else {
					return
				}
				model.moveCenter(
					toX: (start.x + value.translation.width) / size.width,
					y: (start.y + value.translation.height) / size.height
				)
			}
			.onEnded { _ in
				dragStart = nil
			}
	}
}
