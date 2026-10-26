# 0007: iPad streaming session

- **Status:** Accepted
- **Date:** 2026-10-26

## Context

M1 and M2 gave the desktop a working Wi-Fi receiver and a Linux absolute
tablet. M3 must produce the sender side: pairing, Pencil and finger capture,
and a session that recovers from desktop restarts without user intervention. The
app targets iPadOS 17 and Swift 6 language mode, and the logic must be testable
on Linux, where UIKit and `Network.framework` are unavailable.

The reference Pencil (USB-C) reports tilt and hover but no pressure. Sending
predicted samples would add traffic that the desktop ignores. The desktop keeps
its sequence window across repeated `HELLO`s, so a sender that resets its
sequence on every handshake would look like a stale or replayed stream.

## Decision

- Split the sender into three layers:
  - `SessionMachine`, a value type with no clock or transport that owns the
    handshake, timers, sequence counter, and sample encoding.
  - `SessionController`, a one-shot actor that supplies a monotonic clock,
    drains outgoing packets in order, and runs the receive, tick, and sample
    tasks. A new session means a new controller.
  - `DatagramTransport`, a protocol with one datagram per element. The Wi-Fi
    implementation uses `NWConnection` and is gated by `#if canImport(Network)`
    so the package still builds on Linux. Tests use a fake transport.
- `SessionMachine` is driven by `Duration` values supplied by the caller and
  queues packets for `takeOutbound()`. This keeps every rule deterministic
  without sleeping.
- Send `HELLO` every 500 ms until a matching `HELLO_ACK` arrives. Send a
  heartbeat once per second while streaming. Five seconds without an accepted
  packet starts a new handshake over the same open transport.
- Do not reset the sequence counter between handshakes. The desktop keeps its
  window, and continuing the counter keeps reconnects indistinguishable from a
  long stream.
- Send no predicted samples and no pressure. Advertise only
  `CAP_TILT | CAP_HOVER`; the desktop substitutes its default pressure level.
- Model the pointer phase as an enum (`contact`, `hover`, `outOfRange`) instead
  of free flag bits, so `CONTACT | HOVER` cannot be encoded by accident.
- Use a UIKit canvas inside the SwiftUI shell:
  - Contact samples come from coalesced touches, and a `CADisplayLink`
    re-sends the held position at display rate so the desktop watchdog stays
    armed during a stationary hold.
  - Hover comes from a `UIHoverGestureRecognizer` restricted to Pencil touches
    (`allowedTouchTypes`). Location, altitude, and azimuth are normalized with
    the same pure functions as contact samples.
  - Lifting the Pencil or leaving hover sends one `outOfRange` sample.
  - The canvas adopts one pointer at a time and gives a starting Pencil priority
    over fingers.
- Normalize points and radians in `PointerNormalization` in the core package so
  the mapping is covered by Linux tests.
- Stop or backgrounding sends `BYE` and closes the transport. A suspended app
  cannot sample; the desktop watchdog covers the gap if `BYE` is lost.
- Keep the pairing token in `UserDefaults`. It is a pairing convenience, not a
  secret store; the token is clear text on the wire.

## Consequences

- Every session rule and encoding path is tested on Linux against a fake
  transport. Only the canvas, `NWConnection`, and SwiftUI need a Mac or device.
- A desktop restart is recovered without touching the iPad, at the cost of
  repeating `HELLO` every 500 ms while the desktop is away.
- Stationary contact keeps the desktop watchdog armed, but stationary hover does
  not: hover samples stop when the Pencil stops moving and the desktop releases
  the tool after the watchdog window. The cursor does not move, and the next
  hover movement resumes hover. This can be revisited if an application needs a
  persistent hover state.
- Because samples are sent in one task, a slow socket write delays the next
  sample instead of building an unbounded task list. Dropped samples are
  acceptable at 60 Hz; stale ones are not.
- The app is iPad-only and foreground-only. There is no background mode, and no
  attempt is made to keep streaming while the screen is locked.
- Real-device behavior (local network prompt, hover pose availability, corner
  accuracy, display rate) is not proven by the core tests. M3 remains beta until
  the manual checklist in [`../testing.md`](../testing.md) passes.
