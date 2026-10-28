import Foundation

/// A language the app can be shown in.
///
/// Translations are plain Swift tables in `Strings+English.swift` rather than
/// `.lproj` bundles, so the same strings work in the Xcode project and in the
/// xtool build, and the completeness test can check every table on Linux.
///
/// To add a language: add a case here, add a table in a new
/// `Strings+<Language>.swift`, and return it from ``Localization/table(for:)``.
/// `LocalizationTests` then fails until every ``TextKey`` is translated.
public enum AppLanguage: String, CaseIterable, Sendable, Hashable {
	/// English, the source language and the fallback for every other language.
	case english = "en"

	/// Language used when the device language has no table.
	public static let fallback = AppLanguage.english

	/// The language's name in that language, for a future language picker.
	public var nativeName: String {
		switch self {
		case .english: "English"
		}
	}

	/// Pick the table for a BCP 47 language tag such as `de-DE`.
	///
	/// Only the primary subtag is considered: a German table also serves
	/// `de-AT`. Unknown and missing tags fall back to ``fallback``.
	///
	/// - Parameter languageTag: A tag from `Locale.preferredLanguages`.
	/// - Returns: The language whose table should be used.
	public static func resolved(for languageTag: String?) -> AppLanguage {
		guard let languageTag else {
			return fallback
		}
		let primary =
			languageTag
			.lowercased()
			.split(separator: "-", maxSplits: 1)
			.first
			.map(String.init)
		guard let primary, !primary.isEmpty else {
			return fallback
		}
		return AppLanguage(rawValue: primary) ?? fallback
	}
}

/// A piece of user-facing text.
///
/// Views never contain prose. Every string the user can see is a case here,
/// which makes the set of translatable text finite and lets the tests prove
/// that no language is missing a key.
///
/// Placeholders are documented on the case and use positional specifiers
/// (`%1$@`) so a translation may reorder them.
public enum TextKey: String, CaseIterable, Sendable, Hashable {
	// MARK: Pairing and transport errors

	/// `%1$d` expected length, `%2$d` typed length.
	case pairingCodeLength
	/// `%1$@` the rejected character.
	case pairingCodeCharacter
	/// The Wi-Fi address or port is unusable.
	case udpInvalidAddress
	/// The USB TCP port is unusable.
	case tcpInvalidPort
	/// The transport closed while a session was running.
	case networkClosed

	// MARK: Pairing screen

	/// Small label above the headline.
	case headerBadge
	/// Pairing screen headline.
	case headerTitle
	/// Pairing screen explanation.
	case headerSubtitle
	/// Heading of the connection card.
	case desktopSection
	/// Label of the transport picker.
	case transportLabel
	/// Wi-Fi and USB names for the transport picker.
	case transportWiFi
	case transportUSB
	/// Wi-Fi host field.
	case addressLabel
	/// Port field labels.
	case udpPortLabel
	case tcpPortLabel
	/// Pairing code field.
	case pairingCodeLabel
	/// VoiceOver labels for the code buttons.
	case generateCodeAccessibility
	case pasteCodeAccessibility
	case copyCodeAccessibility
	/// Primary action.
	case startStreaming
	/// Shown when the Wi-Fi address is empty.
	case validationAddress
	/// Shown when the port is not `1...65535`.
	case validationPort
	/// `%1$d` number of characters in a pairing code.
	case validationCode

	// MARK: Connection help

	/// Heading above the transport steps.
	case howToConnect
	/// Wi-Fi setup steps.
	case wifiStep1
	case wifiStep2
	case wifiStep3
	/// USB setup steps.
	case usbStep1
	case usbStep2
	case usbStep3

	// MARK: Protocol card

	/// Heading of the protocol facts card.
	case wireProtocol
	/// Protocol fact labels.
	case protocolVersionLabel
	case protocolPacketSizeLabel
	case protocolMagicLabel
	case protocolTransportLabel
	case protocolPairingCodeLabel
	case protocolCapabilitiesLabel
	/// `%1$d` packet size in bytes.
	case packetSizeValue
	/// The fixed magic value, `0x5441` (`AT`).
	case magicValue
	/// The supported transports.
	case transportValue
	/// `%1$d` number of characters in a pairing code.
	case pairingCodeLengthValue
	/// The pointer features the reference Pencil provides.
	case capabilitiesValue
	/// `%1$d` step number in the connection help.
	case stepNumber
	/// `%1$@` marketing version, `%2$d` protocol version.
	case setupFooter

	// MARK: Drawing area

	/// Home card and editor title.
	case drawingArea
	/// `%1$@` aspect ratio, `%2$d` percentage of the screen height.
	case drawingAreaSummary
	/// `%1$d` width percentage, `%2$d` height percentage, when unlocked.
	case drawingAreaSizeSummary
	/// Summary when the whole screen accepts input.
	case drawingAreaFullScreen
	/// Why the area exists.
	case areaExplanation
	/// How to move the area in the preview.
	case areaDragHint
	/// Size slider label and its `%1$d` percentage value.
	case areaSizeLabel
	case areaSizeValue
	/// Width and height slider labels, with `%1$d` percentage values.
	case areaWidthLabel
	case areaHeightLabel
	case areaWidthValue
	case areaHeightValue
	/// Horizontal and vertical position sliders.
	case areaHorizontalLabel
	case areaVerticalLabel
	/// Heading above the position sliders.
	case areaPositionLabel
	/// `%1$@` a side name, `%2$d` its value in the custom ratio.
	case areaSideValue
	/// Aspect lock toggle and its explanation.
	case areaLockLabel
	case areaLockHint
	/// Heading above the ratio presets.
	case areaRatioLabel
	/// Disclosure label for a user-defined ratio.
	case areaCustomRatio
	/// Stepper labels for the custom ratio.
	case areaRatioWidthLabel
	case areaRatioHeightLabel
	/// Button that restores the whole screen.
	case areaReset

	// MARK: Streaming screen

	/// Ends the session.
	case stop
	/// Failure card heading.
	case connectionFailed
	/// Failure hints, one per transport.
	case failureHintUSB
	case failureHintWiFi
	/// Returns to the pairing screen.
	case backToSetup
	/// Session state titles.
	case statusIdle
	case statusConnecting
	case statusStreaming
	case statusDisconnected
	/// Session state details.
	case statusNoSession
	/// `%1$@` endpoint.
	case statusWaiting
	/// `%1$@` endpoint, `%2$d` attempt number.
	case statusWaitingAttempt
	/// `%1$08X` session id, `%2$@` endpoint.
	case statusSession
	/// The desktop never acknowledged the session.
	case statusRejected
	/// `%1$d` USB port.
	case endpointUSB
	/// Running counters.
	case statisticsSamples
	case statisticsCoalesced
	case statisticsRoundTrip
	case statisticsReconnects
}

/// Resolves ``TextKey`` values for one language.
///
/// The type is a value so previews and tests can ask for a language different
/// from the device's, and so nothing reads a global mutable table.
public struct Localization: Sendable, Hashable {
	/// The device's preferred language, resolved once per process.
	public static let current = Localization(
		language: AppLanguage.resolved(for: Locale.preferredLanguages.first)
	)

	/// Language this instance formats for.
	public let language: AppLanguage

	/// Create a localization for `language`.
	///
	/// - Parameter language: The language to use; defaults to English.
	public init(language: AppLanguage = AppLanguage.fallback) {
		self.language = language
	}

	/// The text for `key` in this language, or the English fallback.
	///
	/// The returned string may contain positional format specifiers; use
	/// ``format(_:_:)`` to fill them in.
	///
	/// - Parameter key: The text to look up.
	/// - Returns: The translated text, never empty.
	public func text(_ key: TextKey) -> String {
		Localization.table(for: language)[key]
			?? Localization.table(for: AppLanguage.fallback)[key]
			?? key.rawValue
	}

	/// The text for `key` with its placeholders filled in.
	///
	/// - Parameters:
	///   - key: The text to look up.
	///   - arguments: Values for the positional placeholders.
	/// - Returns: The formatted text.
	public func format(_ key: TextKey, _ arguments: CVarArg...) -> String {
		format(key, locale: .current, arguments: arguments)
	}

	/// The text for `key` formatted for an explicit locale.
	///
	/// Tests and previews use this to keep number formatting deterministic.
	///
	/// - Parameters:
	///   - key: The text to look up.
	///   - locale: Locale used for decimal separators and digit grouping.
	///   - arguments: Values for the positional placeholders.
	/// - Returns: The formatted text.
	public func format(_ key: TextKey, locale: Locale, arguments: [CVarArg]) -> String {
		String(format: text(key), locale: locale, arguments: arguments)
	}

	/// The complete table for `language`.
	///
	/// Exposed so tests and translation tooling can compare tables.
	///
	/// - Parameter language: The language to load.
	/// - Returns: Every translated key for that language.
	public static func table(for language: AppLanguage) -> [TextKey: String] {
		switch language {
		case .english: EnglishStrings.table
		}
	}
}
