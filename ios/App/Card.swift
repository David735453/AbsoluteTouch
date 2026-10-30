import SwiftUI

/// Grouped card used by the pairing screen and its sub-pages.
struct Card<Content: View>: View {
	private let content: Content

	init(@ViewBuilder content: () -> Content) {
		self.content = content()
	}

	var body: some View {
		VStack(alignment: .leading, spacing: 14) {
			content
		}
		.padding(20)
		.frame(maxWidth: .infinity, alignment: .leading)
		.background(
			Color(uiColor: .secondarySystemGroupedBackground),
			in: RoundedRectangle(cornerRadius: 18, style: .continuous)
		)
	}
}
