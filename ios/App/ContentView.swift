import AbsoluteTouchCore
import SwiftUI
import UIKit

/// Pairing form for the iPad sender.
///
/// The form persists its fields in `UserDefaults`, so the pairing token only
/// needs to be entered once. Starting a session swaps the whole screen for
/// ``StreamingView``; stopping returns here.
struct ContentView: View {
	@State private var viewModel = StreamingViewModel()
	@State private var session: ConnectionSettings?
	@Environment(\.scenePhase) private var scenePhase

	@AppStorage("absoluteTouch.host") private var host = ""
	@AppStorage("absoluteTouch.port") private var portText = String(
		ConnectionSettings.defaultPort
	)
	@AppStorage("absoluteTouch.token") private var tokenText = ""

	var body: some View {
		Group {
			if viewModel.isActive, let session {
				StreamingView(
					viewModel: viewModel,
					settings: session,
					onStop: stopSession
				)
			} else {
				setup
			}
		}
		.onAppear {
			if tokenText.isEmpty {
				tokenText = PairingToken.random().hex
			}
		}
		.onChange(of: scenePhase) { _, phase in
			// A backgrounded app cannot keep sampling. Saying goodbye lets the
			// desktop release the tablet immediately instead of waiting for the
			// watchdog.
			if phase == .background, viewModel.isActive {
				stopSession()
			}
		}
	}

	private var setup: some View {
		NavigationStack {
			ScrollView {
				VStack(alignment: .leading, spacing: 20) {
					header
					pairingCard
					howToCard
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

			Text("Enter the desktop's address and pairing token, then start streaming.")
				.font(.subheadline)
				.foregroundStyle(.secondary)
				.fixedSize(horizontal: false, vertical: true)
		}
	}

	private var pairingCard: some View {
		Card {
			Text("Desktop")
				.font(.headline)

			field("Address") {
				TextField("192.0.2.10", text: $host)
					.textFieldStyle(.roundedBorder)
					.keyboardType(.URL)
					.textInputAutocapitalization(.never)
					.autocorrectionDisabled()
					.submitLabel(.next)
			}

			field("UDP port") {
				TextField("37689", text: $portText)
					.textFieldStyle(.roundedBorder)
					.keyboardType(.numberPad)
			}

			field("Pairing token") {
				HStack(spacing: 8) {
					TextField("32 hexadecimal characters", text: $tokenText)
						.textFieldStyle(.roundedBorder)
						.font(.system(.callout, design: .monospaced))
						.textInputAutocapitalization(.never)
						.autocorrectionDisabled()

					Button {
						tokenText = PairingToken.random().hex
					} label: {
						Image(systemName: "dice")
					}
					.buttonStyle(.bordered)
					.accessibilityLabel("Generate a new token")

					Button {
						if let pasted = UIPasteboard.general.string {
							tokenText = pasted
						}
					} label: {
						Image(systemName: "doc.on.clipboard")
					}
					.buttonStyle(.bordered)
					.accessibilityLabel("Paste a token")

					Button {
						UIPasteboard.general.string = tokenText
					} label: {
						Image(systemName: "doc.on.doc")
					}
					.buttonStyle(.bordered)
					.accessibilityLabel("Copy the token")
				}
			}

			if let validationMessage {
				Label(validationMessage, systemImage: "exclamationmark.triangle.fill")
					.font(.footnote)
					.foregroundStyle(.orange)
					.fixedSize(horizontal: false, vertical: true)
			}

			Button {
				startSession()
			} label: {
				Label("Start streaming", systemImage: "play.fill")
					.frame(maxWidth: .infinity)
			}
			.buttonStyle(.borderedProminent)
			.controlSize(.large)
			.disabled(connectionSettings == nil)
		}
	}

	private var howToCard: some View {
		Card {
			Text("How to connect")
				.font(.headline)

			VStack(alignment: .leading, spacing: 10) {
				step(
					1,
					"On the desktop, run `cargo run -- run`. Enter that machine's local IP address and port 37689 here."
				)
				step(
					2,
					"Copy the token with `cargo run -- token`, or create one with `cargo run -- token --reset`."
				)
				step(
					3,
					"Use the same token here. Both devices must be on the same Wi-Fi network."
				)
			}
			.font(.callout)
			.foregroundStyle(.secondary)
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
				protocolRow("Transport", "Wi-Fi UDP")
				protocolRow("Device capabilities", "Tilt and hover")
			}
			.font(.callout)
		}
	}

	private var footer: some View {
		Text("M3 · Version \(marketingVersion) · Protocol v\(ProtocolSpec.version)")
			.font(.footnote)
			.foregroundStyle(.secondary)
			.frame(maxWidth: .infinity, alignment: .center)
			.padding(.top, 8)
	}

	private func field<Content: View>(
		_ label: String,
		@ViewBuilder content: () -> Content
	) -> some View {
		VStack(alignment: .leading, spacing: 6) {
			Text(label)
				.font(.caption)
				.foregroundStyle(.secondary)
			content()
		}
	}

	private func step(_ number: Int, _ text: String) -> some View {
		HStack(alignment: .firstTextBaseline, spacing: 10) {
			Text("\(number).")
				.font(.callout.monospacedDigit().weight(.semibold))
				.frame(width: 18, alignment: .trailing)
			Text(text)
				.fixedSize(horizontal: false, vertical: true)
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

	private var marketingVersion: String {
		Bundle.main.object(
			forInfoDictionaryKey: "CFBundleShortVersionString"
		) as? String ?? "0.1.0"
	}

	private var connectionSettings: ConnectionSettings? {
		let address = host.trimmingCharacters(in: .whitespacesAndNewlines)
		guard !address.isEmpty else {
			return nil
		}
		let port = portText.trimmingCharacters(in: .whitespacesAndNewlines)
		guard let portNumber = UInt16(port), portNumber != 0 else {
			return nil
		}
		guard let token = PairingToken(hex: tokenText) else {
			return nil
		}
		return ConnectionSettings(host: address, port: portNumber, token: token)
	}

	private var validationMessage: String? {
		if host.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
			return "Enter the desktop address printed by the desktop CLI."
		}
		let port = portText.trimmingCharacters(in: .whitespacesAndNewlines)
		if UInt16(port) == nil || UInt16(port) == 0 {
			return "Enter a UDP port between 1 and 65535."
		}
		if PairingToken(hex: tokenText) == nil {
			return "The token must be exactly 32 hexadecimal characters."
		}
		return nil
	}

	private func startSession() {
		guard let settings = connectionSettings else {
			return
		}
		session = settings
		viewModel.start(settings: settings)
	}

	private func stopSession() {
		viewModel.stop()
		session = nil
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
