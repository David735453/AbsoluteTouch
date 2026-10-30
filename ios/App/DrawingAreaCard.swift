import AbsoluteTouchCore
import SwiftUI

/// Entry to the drawing area editor from the pairing screen.
///
/// The link is only reachable while no session is running: ``ContentView``
/// replaces the whole screen with ``StreamingView`` during a session.
struct DrawingAreaCard: View {
	/// Saved area, used for the summary line.
	let settings: ActiveAreaSettings

	/// Two-way access to the saved area.
	let binding: Binding<ActiveAreaSettings>

	/// Shape of the screen canvas, used for the summary and the editor.
	let canvasAspect: Double

	var body: some View {
		NavigationLink {
			AreaEditorView(settings: binding, canvasAspect: canvasAspect)
		} label: {
			Card {
				HStack(spacing: 12) {
					Label(Copy.text(.drawingArea), systemImage: "rectangle.dashed")
						.font(.headline)
					Spacer(minLength: 8)
					Image(systemName: "chevron.right")
						.font(.footnote.weight(.semibold))
						.foregroundStyle(.tertiary)
						.accessibilityHidden(true)
				}

				Text(settings.summary(forCanvasAspect: canvasAspect))
					.font(.callout)
					.foregroundStyle(.secondary)
			}
		}
		.buttonStyle(.plain)
	}
}
