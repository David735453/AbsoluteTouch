import AbsoluteTouchCore
import Foundation
import Testing

@Suite("Localization")
struct LocalizationTests {
	@Test("Every language translates every key")
	func tablesAreComplete() {
		for language in AppLanguage.allCases {
			let table = Localization.table(for: language)
			for key in TextKey.allCases {
				let text = table[key]
				#expect(text != nil, "\(language.rawValue) is missing \(key.rawValue)")
				#expect(
					text?.isEmpty == false,
					"\(language.rawValue) has an empty \(key.rawValue)"
				)
			}
		}
	}

	@Test("Tables contain nothing that is not a key")
	func tablesHaveNoStrayKeys() {
		for language in AppLanguage.allCases {
			let keys = Set(Localization.table(for: language).keys)
			let expected = Set(TextKey.allCases)
			#expect(keys.subtracting(expected).isEmpty, "\(language.rawValue) has stray keys")
		}
	}

	@Test("No entry has accidental whitespace")
	func entriesAreTrimmed() {
		for language in AppLanguage.allCases {
			for (key, text) in Localization.table(for: language) {
				#expect(
					text == text.trimmingCharacters(in: .whitespacesAndNewlines),
					"\(language.rawValue) \(key.rawValue) has surrounding whitespace"
				)
			}
		}
	}

	@Test("Language tags resolve to the primary subtag")
	func tagsResolve() {
		#expect(AppLanguage.resolved(for: "en") == .english)
		#expect(AppLanguage.resolved(for: "en-GB") == .english)
		#expect(AppLanguage.resolved(for: "EN") == .english)
		#expect(AppLanguage.resolved(for: "de-DE") == .english)
		#expect(AppLanguage.resolved(for: "fr") == .english)
		#expect(AppLanguage.resolved(for: nil) == .english)
		#expect(AppLanguage.resolved(for: "") == .english)
	}

	@Test("English text is looked up by key")
	func englishTextIsExposed() {
		let english = Localization(language: .english)
		#expect(english.text(.stop) == "Stop")
		#expect(english.text(.drawingArea) == "Drawing area")
		#expect(english.language == .english)
	}

	@Test("Positional placeholders are filled in")
	func placeholdersAreFilled() {
		let english = Localization(language: .english)
		let locale = Locale(identifier: "en_US_POSIX")
		#expect(
			english.format(.statusWaitingAttempt, locale: locale, arguments: ["1.2.3.4:37689", 3])
				== "Waiting for 1.2.3.4:37689 · attempt 3"
		)
		#expect(
			english.format(
				.statusSession,
				locale: locale,
				arguments: [UInt32(0xDEAD_BEEF), "USB port 1"]
			) == "Session DEADBEEF · USB port 1"
		)
		#expect(
			english.format(.statisticsRoundTrip, locale: locale, arguments: [12.34])
				== "12.3 ms RTT"
		)
	}

	@Test("Formatting follows the requested locale")
	func formattingUsesTheLocale() {
		let english = Localization(language: .english)
		#expect(
			english.format(
				.statisticsRoundTrip,
				locale: Locale(identifier: "de_DE"),
				arguments: [12.34]
			) == "12,3 ms RTT"
		)
	}

	@Test("The current localization uses the device language")
	func currentResolvesTheDeviceLanguage() {
		#expect(
			Localization.current.language == AppLanguage.resolved(for: Locale.preferredLanguages.first))
	}

	@Test("Formatter output is never empty")
	func formattedTextIsUsable() {
		let english = Localization(language: .english)
		#expect(!english.text(.pairingCodeLength).isEmpty)
		#expect(!english.format(.validationCode, 6).isEmpty)
	}
}
