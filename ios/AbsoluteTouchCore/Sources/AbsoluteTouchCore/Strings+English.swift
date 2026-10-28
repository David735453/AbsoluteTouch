/// English copy, the source language for every other table.
///
/// Keep one entry per ``TextKey``; `LocalizationTests` fails if a key is
/// missing, empty, or has surrounding whitespace.
enum EnglishStrings {
	static let table: [TextKey: String] = [
		// Pairing and transport errors
		.pairingCodeLength: "A pairing code has %1$d characters, not %2$d.",
		.pairingCodeCharacter: "%1$@ is not a pairing code character.",
		.udpInvalidAddress: "Enter a desktop address and a port between 1 and 65535.",
		.tcpInvalidPort: "Enter a TCP port between 1 and 65535.",
		.networkClosed: "The network connection closed.",

		// Pairing screen
		.headerBadge: "iPad input bridge",
		.headerTitle: "Turn your iPad and Apple Pencil into an absolute graphics tablet.",
		.headerSubtitle:
			"Enter the desktop's address or connect by cable, then enter the pairing code.",
		.desktopSection: "Desktop",
		.transportLabel: "Transport",
		.transportWiFi: "Wi-Fi",
		.transportUSB: "USB",
		.addressLabel: "Address",
		.udpPortLabel: "UDP port",
		.tcpPortLabel: "TCP port",
		.pairingCodeLabel: "Pairing code",
		.generateCodeAccessibility: "Generate a new pairing code",
		.pasteCodeAccessibility: "Paste a pairing code",
		.copyCodeAccessibility: "Copy the pairing code",
		.startStreaming: "Start streaming",
		.validationAddress: "Enter the desktop address printed by the desktop CLI.",
		.validationPort: "Enter a port between 1 and 65535.",
		.validationCode: "Enter the %1$d-character pairing code from `absolutetouch token`.",

		// Connection help
		.howToConnect: "How to connect",
		.wifiStep1:
			"On the desktop, run `cargo run -- run`. Enter that machine's local IP "
			+ "address and UDP port 37689 here.",
		.wifiStep2:
			"Copy the pairing code with `cargo run -- token`, or create one with "
			+ "`cargo run -- token --reset`.",
		.wifiStep3:
			"Use the same code here and start streaming. Both devices must be on "
			+ "the same Wi-Fi network.",
		.usbStep1:
			"Connect the iPad by cable, trust this computer, and start streaming. "
			+ "The app listens for the desktop.",
		.usbStep2:
			"On the desktop, run `cargo run -- run --transport usb` and copy the "
			+ "pairing code from `cargo run -- token`.",
		.usbStep3:
			"Use the same code here. `usbmuxd` must be running; "
			+ "`cargo run -- devices` shows attached iPads.",

		// Protocol card
		.wireProtocol: "Wire protocol",
		.protocolVersionLabel: "Version",
		.protocolPacketSizeLabel: "Packet size",
		.protocolMagicLabel: "Magic",
		.protocolTransportLabel: "Transport",
		.protocolPairingCodeLabel: "Pairing code",
		.protocolCapabilitiesLabel: "Device capabilities",
		.packetSizeValue: "%1$d bytes",
		.magicValue: "0x5441 (AT)",
		.transportValue: "Wi-Fi UDP or USB",
		.pairingCodeLengthValue: "%1$d characters",
		.capabilitiesValue: "Tilt and hover",
		.stepNumber: "%1$d.",
		.setupFooter: "M4 · Version %1$@ · Protocol v%2$d",

		// Drawing area
		.drawingArea: "Drawing area",
		.drawingAreaSummary: "%1$@ · %2$d%% of the height",
		.drawingAreaSizeSummary: "%1$d%% wide · %2$d%% tall",
		.drawingAreaFullScreen: "Full screen",
		.areaExplanation:
			"Choose the part of the screen that accepts the Pencil. Touches "
			+ "outside it are ignored, and the rectangle still maps to the whole "
			+ "desktop.",
		.areaDragHint: "Drag inside the rectangle to move it.",
		.areaSizeLabel: "Size",
		.areaSizeValue: "%1$d%%",
		.areaWidthLabel: "Width",
		.areaHeightLabel: "Height",
		.areaWidthValue: "%1$d%%",
		.areaHeightValue: "%1$d%%",
		.areaHorizontalLabel: "Horizontal position",
		.areaVerticalLabel: "Vertical position",
		.areaPositionLabel: "Position",
		.areaSideValue: "%1$@: %2$d",
		.areaLockLabel: "Lock aspect ratio",
		.areaLockHint: "The rectangle keeps this shape when you resize it.",
		.areaRatioLabel: "Aspect ratio",
		.areaCustomRatio: "Custom ratio",
		.areaRatioWidthLabel: "Width",
		.areaRatioHeightLabel: "Height",
		.areaReset: "Reset to full screen",

		// Streaming screen
		.stop: "Stop",
		.connectionFailed: "Connection failed",
		.failureHintUSB:
			"Check the cable and trust prompt, keep this app in the foreground, "
			+ "and confirm `usbmuxd` is running.",
		.failureHintWiFi:
			"Check that the desktop is still running and that both devices are on the same network.",
		.backToSetup: "Back to setup",
		.statusIdle: "Not streaming",
		.statusConnecting: "Connecting",
		.statusStreaming: "Streaming",
		.statusDisconnected: "Disconnected",
		.statusNoSession: "No session",
		.statusWaiting: "Waiting for %1$@",
		.statusWaitingAttempt: "Waiting for %1$@ · attempt %2$d",
		.statusSession: "Session %1$08X · %2$@",
		.statusRejected: "The desktop did not accept the session",
		.endpointUSB: "USB port %1$d",
		.statisticsSamples: "%1$d samples",
		.statisticsCoalesced: "%1$d coalesced",
		.statisticsRoundTrip: "%1$.1f ms RTT",
		.statisticsReconnects: "%1$d reconnects",
	]
}
