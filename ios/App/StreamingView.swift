import AbsoluteTouchCore
import Foundation
import SwiftUI

/// Full-screen drawing surface plus a compact session heads-up display.
///
/// The canvas draws a small ring under the Pencil so the user can confirm the
/// active area lines up with the desktop before touching down. The HUD stays in
/// the safe area and never intercepts touches outside its own controls.
struct StreamingView: View {
	let viewModel: StreamingViewModel
	let settings: ConnectionSettings
	let onStop: @MainActor () -> Void

	var body: some View {
		ZStack {
			CanvasViewRepresentable { sample in
				viewModel.submit(sample)
			}
			.ignoresSafeArea()

			VStack(spacing: 0) {
				hud
					.safeAreaPadding(.horizontal, 20)
					.safeAreaPadding(.top, 8)
				Spacer(minLength: 0)
			}

			if case .failed(let failure) = viewModel.state {
				failureCard(failure)
					.padding(32)
			}
		}
	}

	private var hud: some View {
		HStack(spacing: 14) {
			Circle()
				.fill(statusColor)
				.frame(width: 10, height: 10)
				.overlay(Circle().stroke(.white.opacity(0.25), lineWidth: 1))

			VStack(alignment: .leading, spacing: 3) {
				Text(statusTitle)
					.font(.headline)
				Text(statusDetail)
					.font(.caption)
					.foregroundStyle(.secondary)
					.lineLimit(1)
			}
			.accessibilityElement(children: .combine)

			Spacer(minLength: 8)

			if viewModel.state.isStreaming {
				Text(statisticsText)
					.font(.caption.monospacedDigit())
					.foregroundStyle(.secondary)
					.lineLimit(1)
			}

			Button {
				onStop()
			} label: {
				Label("Stop", systemImage: "stop.fill")
			}
			.buttonStyle(.borderedProminent)
			.tint(.red)
			.controlSize(.large)
		}
		.padding(.horizontal, 18)
		.padding(.vertical, 12)
		.background(.regularMaterial, in: Capsule())
	}

	private func failureCard(_ failure: SessionFailure) -> some View {
		VStack(spacing: 16) {
			Image(systemName: settings.transport == .usb ? "cable.connector" : "wifi.exclamationmark")
				.font(.system(size: 38, weight: .semibold))
				.foregroundStyle(.red)
				.accessibilityHidden(true)

			Text("Connection failed")
				.font(.title3.weight(.semibold))

			Text(failure.localizedDescription)
				.font(.callout)
				.multilineTextAlignment(.center)
				.foregroundStyle(.secondary)
				.fixedSize(horizontal: false, vertical: true)

			Text(
				settings.transport == .usb
					? "Check the cable and trust prompt, keep this app in the foreground, and confirm `usbmuxd` is running."
					: "Check that the desktop is still running and that both devices are on the same network."
			)
			.font(.footnote)
			.multilineTextAlignment(.center)
			.foregroundStyle(.secondary)
			.fixedSize(horizontal: false, vertical: true)

			Button {
				onStop()
			} label: {
				Label("Back to setup", systemImage: "chevron.backward")
			}
			.buttonStyle(.borderedProminent)
			.controlSize(.large)
		}
		.padding(26)
		.frame(maxWidth: 420)
		.background(.regularMaterial, in: RoundedRectangle(cornerRadius: 24, style: .continuous))
	}

	private var statusColor: Color {
		switch viewModel.state {
		case .idle:
			.gray
		case .connecting:
			.orange
		case .streaming:
			.green
		case .failed:
			.red
		}
	}

	private var statusTitle: String {
		switch viewModel.state {
		case .idle:
			"Not streaming"
		case .connecting:
			"Connecting"
		case .streaming:
			"Streaming"
		case .failed:
			"Disconnected"
		}
	}

	private var statusDetail: String {
		let endpoint = endpointDescription
		switch viewModel.state {
		case .idle:
			return "No session"
		case .connecting(let attempt):
			return attempt <= 1
				? "Waiting for \(endpoint)"
				: "Waiting for \(endpoint) · attempt \(attempt)"
		case .streaming(let sessionID, _):
			return "Session \(String(format: "%08X", sessionID)) · \(endpoint)"
		case .failed:
			return "The desktop did not accept the session"
		}
	}

	private var endpointDescription: String {
		switch settings.transport {
		case .wifi:
			"\(settings.host):\(settings.port)"
		case .usb:
			"USB port \(settings.port)"
		}
	}

	private var statisticsText: String {
		var parts = ["\(viewModel.statistics.samplesSent) samples"]
		if viewModel.statistics.samplesCoalesced > 0 {
			parts.append("\(viewModel.statistics.samplesCoalesced) coalesced")
		}
		if let roundTrip = viewModel.statistics.lastRoundTripMicroseconds {
			parts.append(String(format: "%.1f ms RTT", Double(roundTrip) / 1_000))
		}
		if viewModel.statistics.timeouts > 0 {
			parts.append("\(viewModel.statistics.timeouts) reconnects")
		}
		return parts.joined(separator: " · ")
	}
}

#Preview {
	StreamingView(
		viewModel: StreamingViewModel(),
		settings: ConnectionSettings(
			host: "192.0.2.10",
			port: ConnectionSettings.defaultPort,
			token: PairingToken.random()
		),
		onStop: {}
	)
}
