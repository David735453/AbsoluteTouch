import AbsoluteTouchCore
import Testing

@Suite("Pointer normalization")
struct PointerNormalizationTests {
	@Test("Coordinates span the full wire range")
	func coordinatesSpanTheRange() {
		#expect(PointerNormalization.axis(0, extent: 1_000) == 0)
		#expect(PointerNormalization.axis(500, extent: 1_000) == 32_768)
		#expect(PointerNormalization.axis(1_000, extent: 1_000) == 65_535)
	}

	@Test("Coordinates outside the active area are clamped")
	func coordinatesAreClamped() {
		#expect(PointerNormalization.axis(-100, extent: 1_000) == 0)
		#expect(PointerNormalization.axis(1_100, extent: 1_000) == 65_535)
	}

	@Test("A degenerate active area does not divide by zero")
	func degenerateExtentIsZero() {
		#expect(PointerNormalization.axis(100, extent: 0) == 0)
		#expect(PointerNormalization.axis(100, extent: -10) == 0)
		#expect(PointerNormalization.axis(.nan, extent: 100) == 0)
		#expect(PointerNormalization.axis(100, extent: .infinity) == 0)
	}

	@Test("Altitude maps parallel and perpendicular")
	func altitudeSpansTheRange() {
		#expect(PointerNormalization.altitude(radians: 0) == 0)
		#expect(PointerNormalization.altitude(radians: .pi / 4) == 32_768)
		#expect(PointerNormalization.altitude(radians: .pi / 2) == 65_535)
		#expect(PointerNormalization.altitude(radians: -.pi / 2) == 0)
		#expect(PointerNormalization.altitude(radians: .pi) == 65_535)
	}

	@Test("Azimuth maps a full clockwise turn")
	func azimuthWrapsAtAFullTurn() {
		#expect(PointerNormalization.azimuth(radians: 0) == 0)
		#expect(PointerNormalization.azimuth(radians: .pi / 2) == 16_384)
		#expect(PointerNormalization.azimuth(radians: .pi) == 32_768)
		#expect(PointerNormalization.azimuth(radians: 3 * .pi / 2) == 49_151)
		#expect(PointerNormalization.azimuth(radians: 2 * .pi) == 0)
		#expect(PointerNormalization.azimuth(radians: -(.pi / 2)) == 49_151)
	}
}

@Suite("Pointer sample")
struct PointerSampleTests {
	@Test("The phase chooses exactly one position flag")
	func phaseMapsToFlags() {
		#expect(
			PointerSample(x: 0, y: 0, phase: .contact).flags == [.contact]
		)
		#expect(PointerSample(x: 0, y: 0, phase: .hover).flags == [.hover])
		#expect(
			PointerSample(x: 0, y: 0, phase: .outOfRange).flags == [.outOfRange]
		)
	}

	@Test("Tool flags are combined with the phase")
	func toolFlagsAreCombined() {
		let sample = PointerSample(
			x: 10,
			y: 20,
			phase: .contact,
			isFinger: true,
			isEraser: true,
			isBarrel: true
		)

		#expect(sample.flags == [.contact, .finger, .eraser, .barrel])
	}
}
