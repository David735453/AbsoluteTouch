import AbsoluteTouchCore
import Testing

@Suite("Aspect ratio")
struct AspectRatioTests {
	@Test("Both parts are at least one")
	func partsAreClamped() {
		let ratio = AspectRatio(width: 0, height: -3)
		#expect(ratio.width == 1)
		#expect(ratio.height == 1)
		#expect(ratio.label == "1:1")
	}

	@Test("Width follows the canvas shape")
	func widthUsesTheCanvasAspect() {
		// On a 4:3 canvas a 16:9 area at 75% of the height spans the full width.
		let width = AspectRatio.sixteenByNine.widthFraction(
			forHeight: 0.75,
			canvasAspect: 4.0 / 3.0
		)
		#expect(abs(width - 1) < 1e-9)
	}

	@Test("Height is the inverse of width")
	func heightInvertsWidth() {
		let ratio = AspectRatio(width: 3, height: 4)
		let canvasAspect = 1.5
		let width = ratio.widthFraction(forHeight: 0.5, canvasAspect: canvasAspect)
		let height = ratio.heightFraction(forWidth: width, canvasAspect: canvasAspect)
		#expect(abs(height - 0.5) < 1e-9)
	}

	@Test("A degenerate canvas falls back to the fraction")
	func degenerateCanvasIsNeutral() {
		#expect(
			AspectRatio.nineBySixteen.widthFraction(forHeight: 0.25, canvasAspect: 0) == 0.25
		)
		#expect(
			AspectRatio.nineBySixteen.heightFraction(forWidth: 0.25, canvasAspect: .nan) == 0.25
		)
	}

	@Test("The presets include 16:9 first")
	func presetsStartWithTheDefault() {
		#expect(AspectRatio.presets.first == .sixteenByNine)
		#expect(AspectRatio.presets.count == 6)
		#expect(AspectRatio.presets.map(\.label).contains("9:16"))
	}
}

@Suite("Active area")
struct ActiveAreaTests {
	@Test("The full area covers every point")
	func fullAreaContainsTheCanvas() {
		let area = ActiveArea.full
		#expect(area.contains(x: 0, y: 0))
		#expect(area.contains(x: 0.5, y: 0.5))
		#expect(area.contains(x: 1, y: 1))
		#expect(abs(area.screenFraction - 1) < 1e-9)
	}

	@Test("A rectangle outside the canvas is pulled back in")
	func clampedPullsInside() {
		let area = ActiveArea(x: 0.8, y: 1.2, width: 0.5, height: 0.3).clamped
		#expect(abs(area.x - 0.5) < 1e-9)
		#expect(abs(area.y - 0.7) < 1e-9)
		#expect(abs(area.width - 0.5) < 1e-9)
		#expect(abs(area.height - 0.3) < 1e-9)
	}

	@Test("Oversized and vanishing sides stay usable")
	func sidesAreBounded() {
		let oversized = ActiveArea(x: 0, y: 0, width: 4, height: 2).clamped
		#expect(oversized.width == 1)
		#expect(oversized.height == 1)

		let tiny = ActiveArea(
			x: 0,
			y: 0,
			width: 0.001,
			height: -1
		).clamped
		#expect(tiny.width == ActiveArea.minimumDimension)
		#expect(tiny.height == ActiveArea.minimumDimension)
	}

	@Test("Non-finite values do not escape the canvas")
	func nonFiniteValuesFallBack() {
		let area = ActiveArea(x: .nan, y: .infinity, width: .nan, height: .nan).clamped
		#expect(area.x == 0)
		#expect(area.y == 0)
		#expect(area.width == 1)
		#expect(area.height == 1)
	}

	@Test("Hit testing uses canvas fractions")
	func hitTestingUsesFractions() {
		let area = ActiveArea(x: 0.25, y: 0.25, width: 0.5, height: 0.5)
		#expect(area.contains(x: 0.25, y: 0.25))
		#expect(area.contains(x: 0.75, y: 0.75))
		#expect(!area.contains(x: 0.24, y: 0.5))
		#expect(!area.contains(x: 0.5, y: 0.76))
	}

	@Test("The screen fraction is the rectangle's area")
	func screenFractionMultipliesTheSides() {
		let area = ActiveArea(x: 0, y: 0, width: 0.5, height: 0.25)
		#expect(abs(area.screenFraction - 0.125) < 1e-9)
	}
}

@Suite("Active area settings")
struct ActiveAreaSettingsTests {
	private let landscape = 4.0 / 3.0

	@Test("The default is a full-width 16:9 strip")
	func defaultResolvesToSixteenByNine() {
		let resolved = ActiveAreaSettings.default.resolved(forCanvasAspect: landscape)
		#expect(abs(resolved.height - 0.75) < 1e-9)
		#expect(abs(resolved.width - 1) < 1e-9)
		#expect(abs(resolved.x) < 1e-9)
		#expect(abs(resolved.y - 0.125) < 1e-9)
	}

	@Test("An unlocked area keeps the stored rectangle")
	func unlockedUsesTheStoredRectangle() {
		let settings = ActiveAreaSettings(
			area: ActiveArea(x: 0.1, y: 0.2, width: 0.3, height: 0.4),
			isAspectLocked: false
		)
		let resolved = settings.resolved(forCanvasAspect: landscape)
		#expect(resolved == ActiveArea(x: 0.1, y: 0.2, width: 0.3, height: 0.4))
	}

	@Test("A locked width never leaves the canvas")
	func lockedWidthFitsANarrowCanvas() {
		// A canvas narrower than 16:9 cannot hold a full-height 16:9
		// rectangle, so the height shrinks by the same factor instead of the
		// ratio breaking.
		let resolved = ActiveAreaSettings.default.resolved(forCanvasAspect: 0.5)
		#expect(abs(resolved.width - 1) < 1e-9)
		#expect(abs(resolved.height - 0.281_25) < 1e-9)
	}

	@Test("A canvas with the locked ratio is fully covered")
	func matchingCanvasIsFull() {
		let resolved = ActiveAreaSettings.default.resolved(forCanvasAspect: 16.0 / 9.0)
		#expect(abs(resolved.width - 1) < 1e-9)
		#expect(abs(resolved.height - 1) < 1e-9)
	}

	@Test("Resizing keeps the center")
	func resizingKeepsTheCenter() {
		var settings = ActiveAreaSettings.default
		settings.setHeight(0.5, canvasAspect: landscape)
		let resolved = settings.resolved(forCanvasAspect: landscape)
		#expect(abs(resolved.height - 0.5) < 1e-9)
		#expect(abs(resolved.width - 0.666_666_666) < 1e-9)
		#expect(abs(resolved.y - 0.25) < 1e-9)
		#expect(abs(resolved.x + resolved.width / 2 - 0.5) < 1e-9)
	}

	@Test("The width slider only moves an unlocked area")
	func widthChangesOnlyWhenUnlocked() {
		var settings = ActiveAreaSettings.default
		settings.setWidth(0.2, canvasAspect: landscape)
		#expect(abs(settings.resolved(forCanvasAspect: landscape).width - 1) < 1e-9)

		settings.isAspectLocked = false
		settings.setWidth(0.2, canvasAspect: landscape)
		#expect(abs(settings.resolved(forCanvasAspect: landscape).width - 0.2) < 1e-9)
	}

	@Test("Moving past the edge clamps the rectangle")
	func movingClampsToTheCanvas() {
		var settings = ActiveAreaSettings.default
		settings.setHeight(0.5, canvasAspect: landscape)
		settings.moveCenter(toX: 2, y: -1, canvasAspect: landscape)
		let resolved = settings.resolved(forCanvasAspect: landscape)
		#expect(abs(resolved.x + resolved.width - 1) < 1e-9)
		#expect(abs(resolved.y) < 1e-9)
	}

	@Test("Turning the lock on fits the rectangle")
	func enablingTheLockFitsTheRatio() {
		var settings = ActiveAreaSettings(
			area: .full,
			isAspectLocked: false
		)
		#expect(settings.resolved(forCanvasAspect: landscape).width == 1)

		settings.setAspectLocked(true, canvasAspect: landscape)
		let resolved = settings.resolved(forCanvasAspect: landscape)
		#expect(abs(resolved.width - 1) < 1e-9)
		#expect(abs(resolved.height - 0.75) < 1e-9)
	}

	@Test("Changing the ratio keeps the height and center")
	func changingRatioKeepsHeight() {
		var settings = ActiveAreaSettings.default
		settings.setHeight(0.5, canvasAspect: landscape)
		settings.setAspectRatio(AspectRatio(width: 9, height: 16), canvasAspect: landscape)
		let resolved = settings.resolved(forCanvasAspect: landscape)
		#expect(abs(resolved.height - 0.5) < 1e-9)
		#expect(abs(resolved.width - 0.210_937_5) < 1e-9)
		#expect(abs(resolved.x + resolved.width / 2 - 0.5) < 1e-9)
	}

	@Test("Reset returns to the whole screen without the lock")
	func resetRestoresTheFullScreen() {
		var settings = ActiveAreaSettings.default
		settings.setHeight(0.2, canvasAspect: landscape)
		settings.resetToFullScreen()
		#expect(!settings.isAspectLocked)
		#expect(settings.resolved(forCanvasAspect: landscape) == .full)
	}
}

@Suite("Active area summary")
struct ActiveAreaSummaryTests {
	private let english = Localization(language: .english)
	private let landscape = 4.0 / 3.0

	@Test("A locked area reports its ratio and height")
	func lockedSummaryUsesTheRatio() {
		var settings = ActiveAreaSettings.default
		settings.setHeight(0.5, canvasAspect: landscape)
		#expect(
			settings.summary(forCanvasAspect: landscape, localization: english)
				== "16:9 · 50% of the height"
		)
	}

	@Test("The default summary is a full-width strip")
	func defaultSummary() {
		#expect(
			ActiveAreaSettings.default.summary(
				forCanvasAspect: landscape,
				localization: english
			) == "16:9 · 75% of the height"
		)
	}

	@Test("An unlocked full screen says so")
	func fullScreenSummary() {
		let settings = ActiveAreaSettings(area: .full, isAspectLocked: false)
		#expect(
			settings.summary(forCanvasAspect: landscape, localization: english)
				== "Full screen"
		)
	}

	@Test("An unlocked rectangle reports both sides")
	func unlockedSummaryUsesBothSides() {
		let settings = ActiveAreaSettings(
			area: ActiveArea(x: 0.1, y: 0.2, width: 0.3, height: 0.4),
			isAspectLocked: false
		)
		#expect(
			settings.summary(forCanvasAspect: landscape, localization: english)
				== "30% wide · 40% tall"
		)
	}
}
