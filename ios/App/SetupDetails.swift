import AbsoluteTouchCore
import SwiftUI

/// Transport-specific connection instructions shown below the pairing form.
struct HowToCard: View {
	/// Transport the user selected.
	let transport: TransportChoice

	var body: some View {
		Card {
			Text(Copy.text(.howToConnect))
				.font(.headline)

			VStack(alignment: .leading, spacing: 10) {
				switch transport {
				case .wifi:
					step(1, Copy.text(.wifiStep1))
					step(2, Copy.text(.wifiStep2))
					step(3, Copy.text(.wifiStep3))
				case .usb:
					step(1, Copy.text(.usbStep1))
					step(2, Copy.text(.usbStep2))
					step(3, Copy.text(.usbStep3))
				}
			}
			.font(.callout)
			.foregroundStyle(.secondary)
		}
	}

	private func step(_ number: Int, _ text: String) -> some View {
		HStack(alignment: .firstTextBaseline, spacing: 10) {
			Text(Copy.format(.stepNumber, number))
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
			Text(Copy.text(.wireProtocol))
				.font(.headline)

			Grid(alignment: .leading, horizontalSpacing: 24, verticalSpacing: 10) {
				row(Copy.text(.protocolVersionLabel), "\(ProtocolSpec.version)")
				row(
					Copy.text(.protocolPacketSizeLabel),
					Copy.format(.packetSizeValue, ProtocolSpec.packetSize)
				)
				row(Copy.text(.protocolMagicLabel), Copy.text(.magicValue))
				row(Copy.text(.protocolTransportLabel), Copy.text(.transportValue))
				row(
					Copy.text(.protocolPairingCodeLabel),
					Copy.format(.pairingCodeLengthValue, PairingCode.length)
				)
				row(
					Copy.text(.protocolCapabilitiesLabel),
					Copy.text(.capabilitiesValue)
				)
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
		Text(
			Copy.format(
				.setupFooter,
				marketingVersion,
				ProtocolSpec.version
			)
		)
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
