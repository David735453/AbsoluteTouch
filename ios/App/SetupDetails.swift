import AbsoluteTouchCore
import SwiftUI

/// Transport-specific connection instructions shown below the pairing form.
struct HowToCard: View {
	/// Transport the user selected.
	let transport: TransportChoice

	var body: some View {
		Card {
			Text("How to connect")
				.font(.headline)

			VStack(alignment: .leading, spacing: 10) {
				switch transport {
				case .wifi:
					step(
						1,
						"On the desktop, run `cargo run -- run`. Enter that machine's local IP address and UDP port 37689 here."
					)
					step(
						2,
						"Copy the pairing code with `cargo run -- token`, or create one with `cargo run -- token --reset`."
					)
					step(
						3,
						"Use the same code here and start streaming. Both devices must be on the same Wi-Fi network."
					)
				case .usb:
					step(
						1,
						"Connect the iPad by cable, trust this computer, and start streaming. The app listens for the desktop."
					)
					step(
						2,
						"On the desktop, run `cargo run -- run --transport usb` and copy the pairing code from `cargo run -- token`."
					)
					step(
						3,
						"Use the same code here. `usbmuxd` must be running; `cargo run -- devices` shows attached iPads."
					)
				}
			}
			.font(.callout)
			.foregroundStyle(.secondary)
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
}

/// Static protocol facts shown at the bottom of the pairing form.
struct ProtocolCard: View {
	var body: some View {
		Card {
			Text("Wire protocol")
				.font(.headline)

			Grid(alignment: .leading, horizontalSpacing: 24, verticalSpacing: 10) {
				row("Version", "\(ProtocolSpec.version)")
				row("Packet size", "\(ProtocolSpec.packetSize) bytes")
				row("Magic", "0x5441 (AT)")
				row("Transport", "Wi-Fi UDP or USB")
				row("Pairing code", "\(PairingCode.length) characters")
				row("Device capabilities", "Tilt and hover")
			}
			.font(.callout)
		}
	}

	private func row(_ label: String, _ value: String) -> some View {
		GridRow {
			Text(label)
				.foregroundStyle(.secondary)
			Text(value)
				.gridColumnAlignment(.trailing)
				.multilineTextAlignment(.trailing)
		}
	}
}

/// Milestone and version line.
struct SetupFooter: View {
	var body: some View {
		Text("M4 · Version \(marketingVersion) · Protocol v\(ProtocolSpec.version)")
			.font(.footnote)
			.foregroundStyle(.secondary)
			.frame(maxWidth: .infinity, alignment: .center)
			.padding(.top, 8)
	}

	private var marketingVersion: String {
		Bundle.main.object(
			forInfoDictionaryKey: "CFBundleShortVersionString"
		) as? String ?? "0.0.1"
	}
}
