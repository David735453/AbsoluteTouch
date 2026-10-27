import Testing

@testable import AbsoluteTouchCore

private func queuedSample(_ index: Int) -> PointerSample {
	PointerSample(x: UInt16(index), y: 0, phase: .contact)
}

@Suite("Sample mailbox")
struct SampleMailboxTests {
	@Test("Draining returns the queued samples in order")
	func drainReturnsQueuedSamples() {
		let mailbox = SampleMailbox()
		mailbox.push(queuedSample(1))
		mailbox.push(queuedSample(2))

		let drained = mailbox.drain()

		#expect(drained.map(\.x) == [1, 2])
		#expect(mailbox.drain().isEmpty)
	}

	@Test("A full mailbox keeps the newest samples")
	func fullMailboxDropsTheOldest() {
		let mailbox = SampleMailbox()
		let count = SampleMailbox.capacity + 8
		for index in 0..<count {
			mailbox.push(queuedSample(index))
		}

		let drained = mailbox.drain()

		#expect(drained.count == SampleMailbox.capacity)
		#expect(drained.first?.x == UInt16(count - SampleMailbox.capacity))
		#expect(drained.last?.x == UInt16(count - 1))
	}

	@Test("A burst of pushes is a single wakeup")
	func aBurstIsOneWakeup() async {
		let mailbox = SampleMailbox()
		for index in 0..<5 {
			mailbox.push(queuedSample(index))
		}
		mailbox.finish()

		var wakeups = 0
		for await _ in mailbox.signals {
			wakeups += 1
		}

		#expect(wakeups == 1)
	}

	@Test("A finished mailbox ignores pushes and ends its signals")
	func aFinishedMailboxIsInert() async {
		let mailbox = SampleMailbox()
		mailbox.push(queuedSample(1))
		mailbox.finish()
		mailbox.push(queuedSample(2))

		var wakeups = 0
		for await _ in mailbox.signals {
			wakeups += 1
		}

		#expect(mailbox.drain().isEmpty)
		#expect(wakeups == 1)
	}
}

@Suite("Sample coalescing")
struct SampleCoalescingTests {
	@Test("A batch of moves collapses to the newest position")
	func movesCollapseToTheNewest() {
		let batch = (0..<10).map { queuedSample($0) }

		let kept = SessionController.coalesce(batch)

		#expect(kept == [batch[9]])
	}

	@Test("Every pointer state change survives coalescing")
	func stateChangesSurvive() {
		let batch = [
			PointerSample(x: 1, y: 0, phase: .contact),
			PointerSample(x: 2, y: 0, phase: .contact),
			PointerSample(x: 3, y: 0, phase: .hover),
			PointerSample(x: 4, y: 0, phase: .hover),
			PointerSample(x: 5, y: 0, phase: .hover, isBarrel: true),
		]

		let kept = SessionController.coalesce(batch)

		#expect(kept.map(\.flags) == [.contact, .hover, [.hover, .barrel]])
		#expect(kept.last?.x == 5)
	}

	@Test("A contact and the lift-off after it are both kept")
	func pressAndReleaseSurvive() {
		let batch = [
			PointerSample(x: 1, y: 0, phase: .hover),
			PointerSample(x: 2, y: 0, phase: .contact),
			PointerSample(x: 3, y: 0, phase: .contact),
			PointerSample(x: 4, y: 0, phase: .outOfRange),
		]

		let kept = SessionController.coalesce(batch)

		#expect(kept.map(\.flags) == [.hover, .contact, .outOfRange])
	}

	@Test("An empty batch stays empty")
	func emptyBatchStaysEmpty() {
		#expect(SessionController.coalesce([]).isEmpty)
	}
}
