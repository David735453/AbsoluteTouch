# Architecture

AbsoluteTouch turns an iPad and Apple Pencil into an absolute graphics tablet.
The iPad captures pointer input and sends fixed-size packets; a Rust desktop
CLI validates them, maps coordinates, and injects them through an OS backend.

This document describes the intended system. M0 implements the protocol, the
CLI skeleton, the Swift core package, and manual CI. M1 adds the Wi-Fi UDP
session on the desktop, trace recording and replay, coordinate mapping, and the
mock injector. M2 adds the Linux `uinput` absolute tablet. M3 adds the iPad
sender: the UIKit canvas, the Swift session machine and controller, and a Wi-Fi
UDP transport. The M3 app is covered by core tests on Linux but has not yet been
validated on real hardware. The other platform backends are still planned.

## Goals

- Low, predictable latency on a trusted local network or over USB.
- Exact cross-language agreement on the wire format.
- One code path for osu! stable, osu! lazer, and general desktop use.
- A testable core without `/dev/uinput`, Windows, or macOS.
- Safe failure: never leave a mouse button or contact stuck.

## Non-goals for v1

- Audio, video, or screen mirroring.
- Ethernet transport.
- A desktop GUI.
- Apple Pencil Pro features (squeeze, barrel roll, haptics).
- Pressure on hardware that does not report it.
- Internet-facing operation or remote play over an untrusted network.

## System overview

```text
┌──────────────────────── iPad ────────────────────────┐
│ AbsoluteTouch app                                    │
│  ├─ SwiftUI shell (pairing, status)                  │
│  ├─ UIKit drawing surface (M3)                       │
│  └─ AbsoluteTouchCore                                 │
│      ├─ Protocol codec (M0)                          │
│      ├─ Session machine + controller (M3)            │
│      └─ Transports: UDP (M3) / TCP listener (M4)     │
└───────────────┬──────────────────────────────────────┘
                │ Wi-Fi UDP 37689
                │ or USB via usbmuxd → TCP 37690
┌───────────────▼──────────────────────────────────────┐
│ Desktop CLI                                           │
│  ├─ transport: UDP, usbmuxd, trace replay             │
│  ├─ session: handshake, sequence, watchdog            │
│  ├─ mapping: normalized → target rectangle            │
│  ├─ inject: mock, uinput, Windows, macOS              │
│  └─ config, stats, doctor                             │
└──────────────────────────────────────────────────────┘
```

The iPad is the input device and the protocol initiator. For USB, the iPad is
also the TCP server; the desktop connects through the local `usbmuxd` daemon.
See [ADR 0002](decisions/0002-wifi-and-usb-transports.md).

## Desktop

The desktop is a single Rust crate with a library and a thin binary.

| Module | Responsibility | Status |
| --- | --- | --- |
| `protocol` | 40-byte encode/decode, flags, errors | M0 |
| `config` | Defaults, TOML, environment, and CLI precedence | M1 |
| `cli` | Argument parsing and command dispatch | M1 |
| `doctor` | Environment and permission report | M0 |
| `transport::udp` | Wi-Fi UDP receive path and bounded queue | M1 |
| `transport::usbmux` | USB path through `usbmuxd` | M4 |
| `transport::trace` | Record and replay packet traces | M1 |
| `session` | Handshake, sequence window, heartbeats, watchdog | M1 |
| `mapping` | Normalized device → target rectangle | M1 |
| `inject` | Injector trait, mock recorder, platform backends | M1 mock, M2 uinput; M5/M6 planned |
| `stats` | Counters for drops, latency, errors | M1 |

### Session pipeline

Implemented M1 flow:

1. The UDP receiver thread blocks on the socket with a short poll timeout.
2. It decodes length, magic, version, and packet type with the protocol
   decoder, then forwards valid packets and decode failures over a bounded
   channel. A full channel is a visible, counted drop; the receiver never
   blocks the OS socket indefinitely.
3. The session loop checks the session identifier and peer, tracks the
   sequence window, applies mapping, and injects.
4. Two timers guard the session: after 750 ms of silence the watchdog releases
   all input but keeps the session alive, so hover or idle heartbeats resume
   without a new handshake. After 5000 ms of silence the session ends and a new
   `HELLO` is required. Both values are configurable.
5. `HELLO_ACK` and `HEARTBEAT_ACK` are sent from the same loop that observes
   them, so replies share the session's view of time and state.
6. `BYE`, the stop flag (Ctrl+C), transport failure, and a normal exit all run
   the same release path. The runner calls `release_all` once more before it
   returns.

The design deliberately avoids an async runtime through M2. At 60 Hz a single
receiver thread plus a tight session loop is simpler to reason about than a
task graph, and the polled socket doubles as the idle wait. Later backends can
change this if measurements justify it.

### Recording and replay

`--record FILE` writes every inbound packet, including packets the session
later rejects, with a host-relative arrival offset. `replay FILE` feeds the
same session, mapping, and injector code from the trace without opening a
socket, so injection decisions can be compared and regression-tested. The trace
format is internal and versioned; it is not the wire protocol.

### Injection abstraction

The session layer talks to an `Injector` trait (M1):

```rust
trait Injector {
    fn inject(&mut self, sample: &MappedSample) -> Result<(), InjectError>;
    fn release_all(&mut self) -> Result<(), InjectError>;
}
```

- `MockInjector` records events for tests and supports assertions on exact
  sequences.
- The Linux backend (M2) registers an `AbsoluteTouch Tablet` `uinput` device
  with `ABS_X`/`ABS_Y` (`0..=65535`), `ABS_PRESSURE`, `ABS_TILT_X`/`ABS_TILT_Y`
  (`-90..=90` degrees), and the pen, rubber, finger, touch, and stylus keys.
  Hover is the tool in range with `BTN_TOUCH` clear. Releasing clears every
  button, pressure, and tilt without moving the cursor. See
  [ADR 0006](decisions/0006-linux-uinput-tablet.md).
- Windows uses a synthetic pen device where possible and `SendInput` as a
  fallback (M5, beta).
- macOS uses `CGEvent` mouse events (M6, beta). It requires Accessibility
  permission and cannot provide full tablet semantics.

OS-specific code stays inside `desktop/src/inject/`. Everything above the trait
must be testable on any platform.

### Mapping

The wire format carries normalized coordinates. The host maps them to a target
rectangle:

- Default target: the full desktop, as a normalized rectangle.
- Default aspect behavior: `preserve`, which letterboxes the active area inside
  the target and centers it. Until the desktop aspect ratio is configured,
  preserve falls back to stretching.
- No smoothing or prediction on the host by default. Predicted samples from the
  iPad are flagged and are not injected as separate contacts.
- Pressure defaults to 32,768 when `CAP_PRESSURE` is absent.
- Tilt is projected onto the backend's axes; the Linux backend uses
  `ABS_TILT_X` / `ABS_TILT_Y`.
- Hover updates position with the pen tool in range but no contact.
- Finger samples are tagged so a policy or backend can treat them differently
  later.

Mapping is pure and unit-tested without any OS backend.

## iPad app

| Layer | Responsibility | Status |
| --- | --- | --- |
| `ios/App` | SwiftUI pairing shell, UIKit canvas, view model | M3 |
| `AbsoluteTouchCore` | Protocol, session machine, controller, UDP transport | M3 |
| UI tests | Not used for protocol logic | - |

### Why a Swift package

The core package has no UIKit or SwiftUI dependency. It builds and tests on
Linux, which is what CI uses, and it keeps protocol changes honest: the same
golden vectors run against the Rust and Swift codecs.

### Why SwiftUI plus UIKit

SwiftUI is a good fit for status, pairing, and settings screens. The drawing
surface needs precise touch timestamps, coalesced touches, and stylus
altitude/azimuth. A `UIView` receives those through `UIEvent` and `UITouch`,
which SwiftUI gestures do not expose directly. The app therefore uses a SwiftUI
shell around a UIKit canvas. See
[ADR 0004](decisions/0004-ios-shell-and-core-package.md) and
[ADR 0007](decisions/0007-ios-streaming-session.md).

### Pointer pipeline

- The canvas adopts one pointer at a time. A Pencil that begins contact takes
  priority over fingers; other touches are ignored until it lifts.
- Contact samples come from coalesced touches. A `CADisplayLink` re-sends the
  last position while the Pencil is held, so a stationary contact keeps the
  desktop watchdog armed without inventing motion.
- Hover comes from a `UIHoverGestureRecognizer` restricted to Pencil touches.
  Its location, altitude, and azimuth are normalized with the same functions as
  contact samples.
- Lifting the Pencil or leaving hover sends one `outOfRange` sample and then
  stops.
- The sample phase is an enum, so an invalid `CONTACT | HOVER` packet cannot be
  constructed. No predicted samples are sent, and pressure is zero because the
  reference Pencil (USB-C) does not report it.
- `PointerNormalization` converts points and radians to wire units. It is pure
  and tested on Linux, where UIKit is unavailable.

### Concurrency

The app uses Swift 6 language mode with strict concurrency:

- The canvas and view model are `@MainActor`.
- One `SessionController` actor per session owns the transport, state machine,
  and sequence counter. It is one-shot: starting a new session creates a new
  actor.
- The canvas calls a `nonisolated` `submit`, which yields into an `AsyncStream`.
  One actor task drains it, so the UI never spawns a task per touch sample.
- Separate actor tasks receive datagrams and run a 20 ms timer. Outgoing packets
  are flushed in order from whichever task produced them.
- The UI consumes `AsyncStream<SessionState>` and samples statistics at 4 Hz. It
  never blocks on socket writes.

Protocol types are value types and `Sendable`. The codec has no shared mutable
state.

### Streaming lifecycle

```text
idle ──start──▶ connecting(attempt) ──HELLO_ACK──▶ streaming(session)
                    │  ▲                              │
                    │  └──── 5000 ms silence ─────────┤
                    └──── transport error ────────────┴──▶ failed / idle
```

- `HELLO` is repeated every 500 ms until the desktop sends a matching
  `HELLO_ACK` with a non-zero session identifier and the same token.
- While streaming, the app sends heartbeats once per second. A heartbeat echoes
  the last acknowledged device and host timestamps, which gives the app a
  round-trip time and clock offset.
- The sequence counter is not reset when a handshake starts again. The desktop
  keeps its sequence window across repeated `HELLO`s, so resetting would look
  like a stale or replayed stream.
- Five seconds without an accepted packet starts a new handshake with the same
  token. The transport stays open, so a desktop restart is recovered without
  returning to the pairing screen.
- Stopping the session or backgrounding the app sends `BYE` and closes the
  transport.
- Streaming only runs while the app is in the foreground; iOS suspends the app
  otherwise, and the desktop watchdog then releases input. No background audio
  or background location tricks are used.

## Transport relationships

### Wi-Fi

- UDP, desktop port 37689.
- The iPad sends to a configured desktop address, so the desktop does not need
  to discover the iPad.
- The desktop replies to the source address of `HELLO`.
- Discovery is not required for v1. A Bonjour service type
  (`_absolutetouch._udp`) is declared in `Info.plist` for a future convenience
  flow.

### USB

- The iPad listens on TCP port 37690.
- The desktop asks `usbmuxd` to connect to that port on the selected device.
- The protocol, handshake, and packet stream are identical to Wi-Fi.
- USB avoids Wi-Fi jitter and local-network permission prompts, but depends on
  the platform's `usbmuxd` support.

## Safety

- The desktop watchdog releases every button and contact after 750 ms without
  a valid packet; a session ends after 5000 ms of silence, on `BYE`, on
  transport close, and on process shutdown.
- The iPad stops sampling when the app is backgrounded, sends `BYE` on stop or
  backgrounding, and stops on transport error.
- `--dry-run` decodes and logs samples without injecting.
- `doctor` reports missing permissions and device nodes before a session.
- The token and source-IP pin reduce accidental cross-session injection. They
  are not a substitute for network security.

## Milestones

| Milestone | Scope |
| --- | --- |
| M0 | Skeleton, protocol, golden vectors, Rust + Swift tests, manual CI |
| M1 | Desktop session, UDP, config, trace replay, mock injector |
| M2 | Linux `uinput` backend |
| M3 | iPad canvas, Wi-Fi end-to-end, hover |
| M4 | USB via `usbmuxd` |
| M5 | Windows backends (beta) |
| M6 | macOS backend, TestFlight, releases |

Platforms are not marked supported because they compile. Linux is the
reference; Windows and macOS stay beta until manual validation on real
hardware. See [testing.md](testing.md).
