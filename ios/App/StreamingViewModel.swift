import AbsoluteTouchCore
import Foundation
import Observation

/// Everything needed to reach one desktop.
struct ConnectionSettings: Equatable {
	/// Desktop IP address or host name.
	var host: String

	/// Desktop UDP port.
	var port: UInt16

	/// Pairing token shared with the desktop.
	var token: PairingToken

	/// Default Wi-Fi UDP port from the protocol.
	static let defaultPort: UInt16 = 37_689
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

		let transport: UDPTransport
		do {
			transport = try UDPTransport(host: settings.host, port: settings.port)
		} catch {
			state = .failed(.transport(error.localizedDescription))
			isActive = true
			return
		}

		let controller = SessionController(
			configuration: SessionConfiguration(token: settings.token),
			transport: transport
		)
		self.controller = controller
		state = .connecting(attempt: 0)
		statistics = SessionStatistics()
		isActive = true
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
}
