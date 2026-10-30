import AbsoluteTouchCore
import SwiftUI

/// Pairing form for the iPad sender.
///
/// Starting a session swaps the whole screen for ``StreamingView``; stopping
/// returns here. The drawing area is configured on its own page from this
/// screen, never while a session is running.
struct ContentView: View {
	@State private var viewModel = StreamingViewModel()
	@State private var session: ConnectionSettings?
	@Environment(\.scenePhase) private var scenePhase

	@AppStorage("absoluteTouch.transport") private var transportRaw = TransportChoice.wifi.rawValue

	@AppStorage(ActiveAreaStorage.x) private var areaX = 0.0
	@AppStorage(ActiveAreaStorage.y) private var areaY = 0.0
	@AppStorage(ActiveAreaStorage.width) private var areaWidth = 1.0
	@AppStorage(ActiveAreaStorage.height) private var areaHeight = 1.0
	@AppStorage(ActiveAreaStorage.isLocked) private var areaLocked = true
	@AppStorage(ActiveAreaStorage.aspectWidth) private var aspectWidth = 16
	@AppStorage(ActiveAreaStorage.aspectHeight) private var aspectHeight = 9

	private var transport: TransportChoice {
		TransportChoice(rawValue: transportRaw) ?? .wifi
	}

	var body: some View {
		Group {
			if viewModel.isActive, let session {
				StreamingView(
					viewModel: viewModel,
					settings: session,
					areaSettings: areaSettings,
					onStop: stopSession
				)
			} else {
				setup
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
			GeometryReader { proxy in
				ScrollView {
					VStack(alignment: .leading, spacing: 20) {
						header
						PairingCard(transport: transportBinding, onStart: startSession)
						DrawingAreaCard(
							settings: areaSettings,
							binding: areaBinding,
							canvasAspect: aspect(of: proxy.size)
						)
						HowToCard(transport: transport)
						ProtocolCard()
						SetupFooter()
					}
					.padding(24)
					.frame(maxWidth: 680, alignment: .leading)
					.frame(maxWidth: .infinity)
				}
			}
			.background(Color(uiColor: .systemGroupedBackground))
			.navigationTitle("AbsoluteTouch")
			.navigationBarTitleDisplayMode(.inline)
		}
	}

	private var header: some View {
		VStack(alignment: .leading, spacing: 8) {
			Label(Copy.text(.headerBadge), systemImage: "pencil.tip")
				.font(.subheadline.weight(.semibold))
				.foregroundStyle(.tint)

			Text(Copy.text(.headerTitle))
				.font(.title2.weight(.semibold))
				.fixedSize(horizontal: false, vertical: true)

			Text(Copy.text(.headerSubtitle))
				.font(.subheadline)
				.foregroundStyle(.secondary)
				.fixedSize(horizontal: false, vertical: true)
		}
	}

	private var transportBinding: Binding<TransportChoice> {
		Binding(
			get: { transport },
			set: { transportRaw = $0.rawValue }
		)
	}

	/// The saved drawing area.
	private var areaSettings: ActiveAreaSettings {
		ActiveAreaSettings(
			area: ActiveArea(x: areaX, y: areaY, width: areaWidth, height: areaHeight),
			isAspectLocked: areaLocked,
			aspectRatio: AspectRatio(width: aspectWidth, height: aspectHeight)
		)
	}

	/// Two-way projection of the stored area, used by the editor.
	private var areaBinding: Binding<ActiveAreaSettings> {
		Binding(
			get: { areaSettings },
			set: { settings in
				areaX = settings.area.x
				areaY = settings.area.y
				areaWidth = settings.area.width
				areaHeight = settings.area.height
				areaLocked = settings.isAspectLocked
				aspectWidth = settings.aspectRatio.width
				aspectHeight = settings.aspectRatio.height
			}
		)
	}

	private func aspect(of size: CGSize) -> Double {
		guard size.width > 0, size.height > 0 else {
			return 4.0 / 3.0
		}
		return Double(size.width / size.height)
	}

	private func startSession(_ settings: ConnectionSettings) {
		session = settings
		viewModel.start(settings: settings)
	}

	private func stopSession() {
		viewModel.stop()
		session = nil
	}
}

#Preview {
	ContentView()
}
