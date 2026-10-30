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

| Module               | Responsibility                                   | Status                            |
| -------------------- | ------------------------------------------------ | --------------------------------- |
| `protocol`           | 40-byte encode/decode, flags, errors             | M0                                |
| `config`             | Defaults, TOML, environment, and CLI precedence  | M1                                |
| `cli`                | Argument parsing and command dispatch            | M1                                |
| `doctor`             | Environment and permission report                | M0                                |
| `pairing`            | Six-character code → 16-byte token               | M4                                |
| `transport::udp`     | Wi-Fi UDP receive path and bounded queue         | M1                                |
| `transport::usbmux`  | USB path through `usbmuxd`                       | M4                                |
| `transport::framing` | Fixed 40-byte frames over a byte stream          | M4                                |
| `transport::trace`   | Record and replay packet traces                  | M1                                |
| `session`            | Handshake, sequence window, heartbeats, watchdog | M1                                |
| `mapping`            | Normalized device → target rectangle             | M1                                |
| `inject`             | Injector trait, mock recorder, platform backends | M1 mock, M2 uinput; M5/M6 planned |
| `stats`              | Counters for drops, latency, errors              | M1                                |

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

### USB pipeline

The USB runner shares the session and injector code with Wi-Fi; only the byte
source differs:

1. `usbmux::Daemon` connects to `/var/run/usbmuxd` (or the
   `ABSOLUTETOUCH_USBMUX_SOCKET` override), lists devices, and asks the daemon
   to connect to TCP port 37690 on the selected one.
2. A successful `Connect` turns the daemon socket into a raw pipe. The runner
   reads it directly in the session loop with a 20 ms timeout, so Ctrl+C and the
   watchdog stay responsive without a reader thread.
3. `framing::FrameDecoder` reassembles 40-byte packets from arbitrary reads.
   Decode failures are counted and logged, exactly as invalid datagrams are.
4. Replies are written back on the same stream. The session names a USB peer
   `Peer::Connection(id)` instead of `Peer::Network(address)`, so the session
   state machine is unchanged; the runner maps actions to writes with a closure.
5. Losing the stream is not the end of the run. The desktop releases input,
   forgets the session, and asks `usbmuxd` for a new connection every 500 ms
   until the iPad listens again; the next `HELLO` starts a fresh session, and
   the counters add up across reconnects. On the iPad the listener stays open
   when the desktop disconnects and the transport yields a
   `peerDisconnected` event, so the app re-handshakes immediately instead of
   waiting for the five-second session timeout. Both ends can therefore be
   restarted in either order without pressing **Stop** or restarting `run`.

USB needs no reader thread because there is exactly one stream. Wi-Fi keeps its
thread because a blocking `recv_from` cannot double as the idle wait without a
socket timeout, and a timeout there would drop datagrams that arrive between
polls.

### Pairing

The desktop pins the session to the first peer that completes the handshake, and
the wire token is the expansion of a six-character code so it can be typed on an
iPad. The derivation lives in `pairing` and is mirrored in Swift; shared vectors
keep them identical. A 30-bit code is only safe to expose because wrong `HELLO`s
are budgeted: five per 30-second window, after which the desktop stops replying
and counts `rate_limited`. A correct code clears the budget.

Heartbeats carry the host and device timestamps needed to align the two clocks,
so the session also reports `input latency`: the age of each injected sample in
host time, measured above the fastest sample of the session. The absolute offset
is only accurate to the round-trip asymmetry, which is tens of milliseconds on a
buffering link, so a raw age would be dominated by clock error; the excess over
the floor is the delay the user actually feels. The offset is therefore taken
once, at the first heartbeat, and kept for the session: the report needs it to be
constant rather than accurate, and a per-heartbeat estimate wandered by the
asymmetry and invented latency that was never on the link. The counter is what
turns "it sometimes lags" into a number, and it is the measurement to quote in a
bug report.

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
- The Linux backend (M2) registers one `uinput` device with `ABS_X`/`ABS_Y`
  (`0..=65535`), `ABS_PRESSURE`, and `ABS_TILT_X`/`ABS_TILT_Y` (`-90..=90`
  degrees). `device = "pointer"` (the default) adds `BTN_LEFT`/`BTN_RIGHT` and
  moves the desktop cursor; `device = "tablet"` adds the pen, rubber, finger,
  touch, and stylus keys instead, for tablet-aware applications. Hover is the
  tool in range with no button pressed. Releasing clears every button,
  pressure, and tilt without moving the cursor. See
  [ADR 0006](decisions/0006-linux-uinput-tablet.md) and
  [ADR 0008](decisions/0008-linux-pointer-device.md).
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

| Layer               | Responsibility                                  | Status |
| ------------------- | ----------------------------------------------- | ------ |
| `ios/App`           | SwiftUI pairing shell, UIKit canvas, view model | M3, M4 |
| `AbsoluteTouchCore` | Protocol, session machine, controller, UDP/TCP  | M3, M4 |
| UI tests            | Not used for protocol logic                     | -      |

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
- A contact that begins outside the configured drawing area is ignored. Leaving
  the area mid-stroke sends `outOfRange` and lifts the desktop pointer; coming
  back in starts a new stroke while the Pencil is still down.

### Drawing area

The iPad has a configurable active rectangle, set on its own page from the
pairing screen and never during a session:

- `ActiveArea` stores the rectangle as fractions of the canvas, so it keeps its
  place through rotation, Split View, and other iPad sizes.
- `ActiveAreaSettings` adds the aspect lock and ratio. With the lock on, the
  stored height and center are kept and the width is derived from the ratio and
  the current canvas shape; a rectangle that would not fit is scaled down by
  the same factor on both sides, so the ratio never breaks.
- The lock defaults to on with 16:9, matching the desktop screen, which
  resolves a full-screen setting to a full-width 16:9 strip on a 4:3 iPad.
- The canvas normalizes positions against the area, not the screen, so the
  whole desktop is reachable inside a small rectangle. The desktop target is
  unchanged; no protocol field is involved.
- The area is dimmed outside and outlined while streaming, and the editor draws
  the same rectangle in a preview with drag, size, ratio, and position controls.

### Localization

User-facing text lives in `TextKey` and per-language tables in
`AbsoluteTouchCore`, not in view bodies:

- `Localization.current` resolves the device language by primary subtag and
  falls back to English. `Copy` is the app's accessor.
- Strings use positional placeholders (`%1$@`) so translations may reorder
  them, and formatting uses the requested locale for decimals.
- The tables are Swift source so the XcodeGen and xtool builds package no
  `.lproj` resources, and `swift test` checks completeness on Linux.
- Adding a language means one `AppLanguage` case, one table, and a
  `Localization.table(for:)` branch; the completeness test fails until every
  key exists.

### Concurrency

The app uses Swift 6 language mode with strict concurrency:

- The canvas and view model are `@MainActor`.
- One `SessionController` actor per session owns the transport, state machine,
  and sequence counter. It is one-shot: starting a new session creates a new
  actor.
- The canvas calls a `nonisolated` `submit`, which pushes into a bounded
  `SampleMailbox`. One actor task drains it, so the UI never spawns a task per
  touch sample and a slow link cannot accumulate latency.
- A drained batch is coalesced to the samples that change the pointer state
  plus the newest position. Every sample carries the complete state, so the
  desktop can skip intermediate coordinates without losing a press, a release,
  or a hover transition. This keeps each drain to one or two datagrams instead
  of one per coalesced touch.
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

- The iPad listens on TCP port 37690, bound to `127.0.0.1`: `usbmuxd` opens the
  connection from inside the iPad, so loopback is the only address that matters
  and the only one that needs no local network permission. The desktop connects
  through the local `usbmuxd` daemon.
- The desktop speaks the daemon's framed XML-plist protocol just long enough to
  run `ListDevices` and `Connect`, then uses the same socket as a raw byte pipe.
- The protocol, handshake, session rules, and packet stream are identical to
  Wi-Fi. The stream has no length prefix, so both sides reassemble 40-byte
  packets with a frame decoder.
- `absolutetouch devices` lists attached iPads; `--serial` selects between them.
  A refused connection means the iPad app is not listening yet, because
  `usbmuxd` only forwards to an open port inside the app. A listener that is
  waiting for a network path is not bound either, so the app treats `.waiting`
  as a failure instead of retrying `HELLO` forever.
- USB avoids Wi-Fi jitter and local-network permission prompts, but depends on
  the platform's `usbmuxd` support and on the iPad keeping the app in the
  foreground. The app disables the idle timer while a session is open.

## Safety

- The desktop watchdog releases every button and contact after 750 ms without
  a valid packet; a session ends after 5000 ms of silence, on `BYE`, on
  transport close, and on process shutdown.
- The iPad stops sampling when the app is backgrounded, sends `BYE` on stop or
  backgrounding, and stops on transport error.
- `--dry-run` decodes and logs samples without injecting.
- `doctor` reports missing permissions and device nodes, and probes `usbmuxd`
  instead of trusting that a socket file exists.
- The pairing token and peer pin reduce accidental cross-session injection. Six
  characters carry 30 bits, so the desktop also rate limits wrong handshakes;
  neither is a substitute for network security.

## Milestones

| Milestone | Scope                                                             |
| --------- | ----------------------------------------------------------------- |
| M0        | Skeleton, protocol, golden vectors, Rust + Swift tests, manual CI |
| M1        | Desktop session, UDP, config, trace replay, mock injector         |
| M2        | Linux `uinput` backend                                            |
| M3        | iPad canvas, Wi-Fi end-to-end, hover (validated on hardware)      |
| M4        | USB via `usbmuxd`, reconnect (validated on hardware)              |
| M5        | Windows backends (beta)                                           |
| M6        | macOS backend, TestFlight, releases                               |

Platforms are not marked supported because they compile. Linux is the
reference; Windows and macOS stay beta until manual validation on real
hardware. See [testing.md](testing.md).
