import AbsoluteTouchCore
import Foundation

/// User-facing copy for the app.
///
/// Views ask for a ``TextKey`` instead of containing prose, so every string
/// already exists in the translation tables and adding a language never
/// requires touching a view.
enum Copy {
	/// Localization resolved once for the device's language.
	private static let localization = Localization.current

	/// The text for `key`.
	///
	/// - Parameter key: The text to look up.
	/// - Returns: The localized string.
	static func text(_ key: TextKey) -> String {
		localization.text(key)
	}

	/// The text for `key` with its placeholders filled in.
	///
	/// - Parameters:
	///   - key: The text to look up.
	///   - arguments: Values for the positional placeholders.
	/// - Returns: The localized, formatted string.
	static func format(_ key: TextKey, _ arguments: CVarArg...) -> String {
		localization.format(key, locale: .current, arguments: arguments)
	}
}
