import AbsoluteTouchCore
import SwiftUI
import UIKit

/// Connection form: transport, address, port, and pairing code.
///
/// The fields persist in `UserDefaults`, so the pairing code only needs to be
/// entered once. Validation stays here; the parent only learns about a
/// complete ``ConnectionSettings``.
struct PairingCard: View {
	/// Selected transport, owned by the parent because the help card shows it.
	@Binding var transport: TransportChoice

	/// Called with a valid configuration when the user starts a session.
	let onStart: (ConnectionSettings) -> Void

	@AppStorage("absoluteTouch.host") private var host = ""
	@AppStorage("absoluteTouch.port") private var portText = String(
		ConnectionSettings.defaultPort
	)
	@AppStorage("absoluteTouch.token") private var tokenText = ""

	var body: some View {
		Card {
			Text(Copy.text(.desktopSection))
				.font(.headline)

			Picker(
				Copy.text(.transportLabel),
				selection: Binding(
					get: { transport },
					set: { choice in
						transport = choice
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
				field(Copy.text(.addressLabel)) {
					TextField("192.0.2.10", text: $host)
						.textFieldStyle(.roundedBorder)
						.keyboardType(.URL)
						.textInputAutocapitalization(.never)
						.autocorrectionDisabled()
						.submitLabel(.next)
				}
			}

			field(Copy.text(transport == .wifi ? .udpPortLabel : .tcpPortLabel)) {
				TextField(String(transport.defaultPort), text: $portText)
					.textFieldStyle(.roundedBorder)
					.keyboardType(.numberPad)
			}

			pairingCodeField

			if let validationMessage {
				Label(validationMessage, systemImage: "exclamationmark.triangle.fill")
					.font(.footnote)
					.foregroundStyle(.orange)
					.fixedSize(horizontal: false, vertical: true)
			}

			Button {
				guard let settings = connectionSettings else {
					return
				}
				onStart(settings)
			} label: {
				Label(Copy.text(.startStreaming), systemImage: "play.fill")
					.frame(maxWidth: .infinity)
			}
			.buttonStyle(.borderedProminent)
			.controlSize(.large)
			.disabled(connectionSettings == nil)
		}
		.onAppear {
			if tokenText.isEmpty {
				tokenText = PairingCode.random()
			}
		}
	}

	private var pairingCodeField: some View {
		field(Copy.text(.pairingCodeLabel)) {
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
				.accessibilityLabel(Copy.text(.generateCodeAccessibility))

				Button {
					if let pasted = UIPasteboard.general.string {
						tokenText = pasted
					}
				} label: {
					Image(systemName: "doc.on.clipboard")
				}
				.buttonStyle(.bordered)
				.accessibilityLabel(Copy.text(.pasteCodeAccessibility))

				Button {
					UIPasteboard.general.string = tokenText
				} label: {
					Image(systemName: "doc.on.doc")
				}
				.buttonStyle(.bordered)
				.accessibilityLabel(Copy.text(.copyCodeAccessibility))
			}
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
				return Copy.text(.validationAddress)
			}
		}
		let port = portText.trimmingCharacters(in: .whitespacesAndNewlines)
		if UInt16(port) == nil || UInt16(port) == 0 {
			return Copy.text(.validationPort)
		}
		if parsedToken == nil {
			return Copy.format(.validationCode, PairingCode.length)
		}
		return nil
	}
}
