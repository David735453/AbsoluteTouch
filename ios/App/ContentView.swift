import AbsoluteTouchCore
import SwiftUI
import UIKit

/// M0 placeholder shell.
///
/// The drawing surface, pairing flow, and streaming status land in M3. This
/// screen only reports the protocol the app and desktop share.
struct ContentView: View {
	var body: some View {
		NavigationStack {
			ScrollView {
				VStack(alignment: .leading, spacing: 20) {
					header
					connectionCard
					protocolCard
					footer
				}
				.padding(24)
				.frame(maxWidth: 680, alignment: .leading)
				.frame(maxWidth: .infinity)
			}
			.background(Color(uiColor: .systemGroupedBackground))
			.navigationTitle("AbsoluteTouch")
			.navigationBarTitleDisplayMode(.inline)
		}
	}

	private var header: some View {
		VStack(alignment: .leading, spacing: 8) {
			Label("iPad input bridge", systemImage: "pencil.tip")
				.font(.subheadline.weight(.semibold))
				.foregroundStyle(.tint)

			Text("Turn your iPad and Apple Pencil into an absolute graphics tablet.")
				.font(.title2.weight(.semibold))
				.fixedSize(horizontal: false, vertical: true)

			Text("Streaming, pairing, and the drawing surface arrive in later milestones.")
				.font(.subheadline)
				.foregroundStyle(.secondary)
				.fixedSize(horizontal: false, vertical: true)
		}
	}

	private var connectionCard: some View {
		Card {
			HStack(alignment: .top, spacing: 14) {
				Image(systemName: "wifi.slash")
					.font(.title2)
					.foregroundStyle(.secondary)
					.frame(width: 30, height: 30)
					.accessibilityHidden(true)

				VStack(alignment: .leading, spacing: 4) {
					Text("Not connected")
						.font(.headline)
					Text("This build has no network or injection pipeline yet.")
						.font(.subheadline)
						.foregroundStyle(.secondary)
						.fixedSize(horizontal: false, vertical: true)
				}
			}
			.accessibilityElement(children: .combine)
		}
	}

	private var protocolCard: some View {
		Card {
			Text("Wire protocol")
				.font(.headline)

			Grid(alignment: .leading, horizontalSpacing: 24, verticalSpacing: 10) {
				protocolRow("Version", "\(ProtocolSpec.version)")
				protocolRow("Packet size", "\(ProtocolSpec.packetSize) bytes")
				protocolRow("Magic", "0x5441 (AT)")
				protocolRow("Transports", "Wi-Fi UDP / USB")
				protocolRow("Pencil input", "Tilt and hover, no pressure")
			}
			.font(.callout)
		}
	}

	private func protocolRow(_ label: String, _ value: String) -> some View {
		GridRow {
			Text(label)
				.foregroundStyle(.secondary)
			Text(value)
				.gridColumnAlignment(.trailing)
				.multilineTextAlignment(.trailing)
		}
	}

	private var footer: some View {
		Text("M0 shell · Version \(marketingVersion) · Protocol v\(ProtocolSpec.version)")
			.font(.footnote)
			.foregroundStyle(.secondary)
			.frame(maxWidth: .infinity, alignment: .center)
			.padding(.top, 8)
	}

	private var marketingVersion: String {
		Bundle.main.object(
			forInfoDictionaryKey: "CFBundleShortVersionString"
		) as? String ?? "0.1.0"
	}
}

private struct Card<Content: View>: View {
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

#Preview {
	ContentView()
}
