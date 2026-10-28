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
	@AppStorage("absoluteTouch.transport") private var transportRaw = TransportChoice.wifi.rawValue
	@AppStorage("absoluteTouch.port") private var portText = String(
		ConnectionSettings.defaultPort
	)
	@AppStorage("absoluteTouch.token") private var tokenText = ""

	private var transport: TransportChoice {
		TransportChoice(rawValue: transportRaw) ?? .wifi
	}

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
				tokenText = PairingCode.random()
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
					HowToCard(transport: transport)
					ProtocolCard()
					SetupFooter()
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

			Text("Enter the desktop's address or connect by cable, then enter the pairing code.")
				.font(.subheadline)
				.foregroundStyle(.secondary)
				.fixedSize(horizontal: false, vertical: true)
		}
	}

	private var pairingCard: some View {
		Card {
			Text("Desktop")
				.font(.headline)

			Picker(
				"Transport",
				selection: Binding(
					get: { transport },
					set: { choice in
						transportRaw = choice.rawValue
						// Switching transports also switches ports; keeping the
						// UDP port on USB would ask usbmuxd for the wrong one.
						portText = String(choice.defaultPort)
					}
				)
			) {
				ForEach(TransportChoice.allCases, id: \.rawValue) { choice in
					Text(choice.title).tag(choice)
				}
			}
			.pickerStyle(.segmented)

			if transport == .wifi {
				field("Address") {
					TextField("192.0.2.10", text: $host)
						.textFieldStyle(.roundedBorder)
						.keyboardType(.URL)
						.textInputAutocapitalization(.never)
						.autocorrectionDisabled()
						.submitLabel(.next)
				}
			}

			field(transport == .wifi ? "UDP port" : "TCP port") {
				TextField(String(transport.defaultPort), text: $portText)
					.textFieldStyle(.roundedBorder)
					.keyboardType(.numberPad)
			}

			field("Pairing code") {
				HStack(spacing: 8) {
					TextField("K7M2QX", text: $tokenText)
						.textFieldStyle(.roundedBorder)
						.font(.system(.title3, design: .monospaced))
						.textInputAutocapitalization(.characters)
						.autocorrectionDisabled()

					Button {
						tokenText = PairingCode.random()
					} label: {
						Image(systemName: "dice")
					}
					.buttonStyle(.bordered)
					.accessibilityLabel("Generate a new pairing code")

					Button {
						if let pasted = UIPasteboard.general.string {
							tokenText = pasted
						}
					} label: {
						Image(systemName: "doc.on.clipboard")
					}
					.buttonStyle(.bordered)
					.accessibilityLabel("Paste a pairing code")

					Button {
						UIPasteboard.general.string = tokenText
					} label: {
						Image(systemName: "doc.on.doc")
					}
					.buttonStyle(.bordered)
					.accessibilityLabel("Copy the pairing code")
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

	private var connectionSettings: ConnectionSettings? {
		let port = portText.trimmingCharacters(in: .whitespacesAndNewlines)
		guard let portNumber = UInt16(port), portNumber != 0 else {
			return nil
		}
		guard let token = parsedToken else {
			return nil
		}

		if transport == .wifi {
			let address = host.trimmingCharacters(in: .whitespacesAndNewlines)
			guard !address.isEmpty else {
				return nil
			}
			return ConnectionSettings(
				transport: .wifi,
				host: address,
				port: portNumber,
				token: token
			)
		}
		return ConnectionSettings(transport: .usb, host: "", port: portNumber, token: token)
	}

	/// The typed secret: a 6-character code, or 32 hex characters from an older
	/// configuration.
	private var parsedToken: PairingToken? {
		let trimmed = tokenText.trimmingCharacters(in: .whitespacesAndNewlines)
		if let hex = PairingToken(hex: trimmed) {
			return hex
		}
		return try? PairingToken(code: trimmed)
	}

	private var validationMessage: String? {
		if transport == .wifi {
			let address = host.trimmingCharacters(in: .whitespacesAndNewlines)
			if address.isEmpty {
				return "Enter the desktop address printed by the desktop CLI."
			}
		}
		let port = portText.trimmingCharacters(in: .whitespacesAndNewlines)
		if UInt16(port) == nil || UInt16(port) == 0 {
			return "Enter a port between 1 and 65535."
		}
		if parsedToken == nil {
			return "Enter the \(PairingCode.length)-character pairing code from `absolutetouch token`."
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

#Preview {
	ContentView()
}
