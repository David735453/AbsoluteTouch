import AbsoluteTouchCore
import SwiftUI

/// SwiftUI wrapper around ``CanvasView``.
struct CanvasViewRepresentable: UIViewRepresentable {
	/// Part of the canvas that accepts input, already resolved for its aspect.
	let activeArea: ActiveArea
	let onSample: @MainActor (PointerSample) -> Void

	func makeUIView(context: Context) -> CanvasView {
		let view = CanvasView(frame: .zero)
		view.delegate = context.coordinator
		view.activeArea = activeArea
		return view
	}

	func updateUIView(_ uiView: CanvasView, context: Context) {
		uiView.activeArea = activeArea
	}

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
