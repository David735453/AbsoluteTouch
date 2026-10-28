import AbsoluteTouchCore
import Foundation
import Observation
import UIKit

/// How the iPad reaches the desktop.
enum TransportChoice: String, CaseIterable {
	/// Wi-Fi UDP; the desktop listens and the iPad sends.
	case wifi

	/// USB through `usbmuxd`; the iPad listens and the desktop connects.
	case usb

	/// Name shown in the transport picker.
	var title: String {
		switch self {
		case .wifi: Copy.text(.transportWiFi)
		case .usb: Copy.text(.transportUSB)
		}
	}

	/// Default port for this transport.
	var defaultPort: UInt16 {
		switch self {
		case .wifi: ConnectionSettings.defaultPort
		case .usb: ConnectionSettings.defaultUSBPort
		}
	}
}

/// Everything needed to reach one desktop.
struct ConnectionSettings: Equatable {
	/// Transport that carries the session.
	var transport: TransportChoice = .wifi

	/// Desktop IP address or host name; unused over USB.
	var host: String

	/// Desktop port: UDP over Wi-Fi, TCP over USB.
	var port: UInt16

	/// Pairing secret shared with the desktop.
	var token: PairingToken

	/// Default Wi-Fi UDP port from the protocol.
	static let defaultPort: UInt16 = 37_689

	/// Default USB TCP port from the protocol.
	static let defaultUSBPort: UInt16 = 37_690
}

/// Owns the streaming session and publishes state to SwiftUI.
///
/// One view model can start, stop, and start again; each run creates a fresh
/// one-shot `SessionController` and transport.
@MainActor
@Observable
final class StreamingViewModel {
	private(set) var state: SessionState = .idle
	private(set) var statistics = SessionStatistics()
	private(set) var isActive = false

	@ObservationIgnored private var controller: SessionController?
	@ObservationIgnored private var stateTask: Task<Void, Never>?
	@ObservationIgnored private var statisticsTask: Task<Void, Never>?

	/// Whether the desktop has accepted the session.
	var isStreaming: Bool {
		state.isStreaming
	}

	/// Start a new session.
	///
	/// A transport that cannot be created is reported through ``state`` so the
	/// streaming screen can show the failure and offer a way back.
	func start(settings: ConnectionSettings) {
		guard controller == nil else {
			return
		}

		let transport: any DatagramTransport
		switch settings.transport {
		case .wifi:
			do {
				transport = try UDPTransport(host: settings.host, port: settings.port)
			} catch {
				state = .failed(.transport(error.localizedDescription))
				isActive = true
				return
			}
		case .usb:
			do {
				transport = try TCPTransport(port: settings.port)
			} catch {
				state = .failed(.transport(error.localizedDescription))
				isActive = true
				return
			}
		}

		let controller = SessionController(
			configuration: SessionConfiguration(token: settings.token),
			transport: transport
		)
		self.controller = controller
		state = .connecting(attempt: 0)
		statistics = SessionStatistics()
		isActive = true
		updateIdleTimer()
		observe(controller)
		Task { await controller.start() }
	}

	/// Hand a sample to the session without blocking the UI.
	func submit(_ sample: PointerSample) {
		controller?.submit(sample)
	}

	/// Stop the session and return to setup.
	func stop() {
		let controller = self.controller
		self.controller = nil
		stateTask?.cancel()
		statisticsTask?.cancel()
		stateTask = nil
		statisticsTask = nil
		isActive = false
		state = .idle
		statistics = SessionStatistics()
		updateIdleTimer()

		if let controller {
			Task { await controller.stop() }
		}
	}

	private func observe(_ controller: SessionController) {
		stateTask = Task { [weak self] in
			for await state in controller.states {
				guard let self else {
					return
				}
				self.state = state
				self.updateIdleTimer()
			}
		}

		statisticsTask = Task { [weak self] in
			while !Task.isCancelled {
				do {
					try await Task.sleep(for: .milliseconds(250))
				} catch {
					return
				}
				guard let self, let controller = self.controller else {
					return
				}
				self.statistics = await controller.statistics
			}
		}
	}

	/// Keep the screen awake for as long as a session can be used.
	///
	/// The iPad sits untouched while the desktop connects, so the default
	/// auto-lock would suspend the app mid-session and close the USB
	/// listener. A failed session releases the timer again so auto-lock still
	/// applies once there is nothing to use.
	private func updateIdleTimer() {
		let keepAwake: Bool
		if case .failed = state {
			keepAwake = false
		} else {
			keepAwake = isActive
		}
		UIApplication.shared.isIdleTimerDisabled = keepAwake
	}
}
