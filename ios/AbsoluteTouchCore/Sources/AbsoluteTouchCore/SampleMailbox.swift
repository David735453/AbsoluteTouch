import Foundation

/// Samples waiting for the network, bounded so a slow link cannot build a backlog.
///
/// ``SessionController/submit(_:)`` is called from the main actor and must not
/// wait for a datagram, so the queue is protected by a lock instead of by the
/// controller's actor isolation. ``signals`` carries a wakeup for a new sample,
/// coalesced to one pending element: the sender drains the whole queue when it
/// wakes, and a burst of touches is one wakeup.
final class SampleMailbox: @unchecked Sendable {
	/// Wakeups for the drain loop, at most one pending at a time.
	let signals: AsyncStream<Void>

	/// Most samples kept between drains.
	///
	/// A full mailbox means the link is behind by more than this many samples.
	/// The oldest positions are the ones worth losing: every sample carries the
	/// complete pointer state, so the newest one alone describes the pointer.
	static let capacity = 32

	private let signalContinuation: AsyncStream<Void>.Continuation
	private let lock = NSLock()
	private var samples: [PointerSample] = []
	private var finished = false

	init() {
		let (signals, continuation) = AsyncStream<Void>.makeStream(
			bufferingPolicy: .bufferingNewest(1)
		)
		self.signals = signals
		self.signalContinuation = continuation
	}

	/// Queue a sample for the next drain.
	///
	/// Samples pushed after ``finish()`` are dropped; the controller has already
	/// stopped draining by then.
	func push(_ sample: PointerSample) {
		lock.lock()
		if finished {
			lock.unlock()
			return
		}
		samples.append(sample)
		if samples.count > Self.capacity {
			samples.removeFirst(samples.count - Self.capacity)
		}
		lock.unlock()
		signalContinuation.yield(())
	}

	/// Remove and return every queued sample, oldest first.
	func drain() -> [PointerSample] {
		lock.lock()
		defer { lock.unlock() }
		let drained = samples
		samples.removeAll(keepingCapacity: true)
		return drained
	}

	/// Drop queued samples and end ``signals``.
	func finish() {
		lock.lock()
		finished = true
		samples.removeAll()
		lock.unlock()
		signalContinuation.finish()
	}
}
