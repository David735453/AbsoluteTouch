# Architecture

AbsoluteTouch turns an iPad and Apple Pencil into an absolute graphics tablet.
The iPad captures pointer input and sends fixed-size packets; a Rust desktop
CLI validates them, maps coordinates, and injects them through an OS backend.

This document describes the intended system. M0 implements the protocol, the
CLI skeleton, the Swift core package, and manual CI. Streaming and injection
layers are marked as planned.

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
│  ├─ SwiftUI shell (status, pairing)                  │
│  ├─ UIKit drawing surface (M3)                       │
│  └─ AbsoluteTouchCore                                 │
│      ├─ Protocol codec (M0)                          │
│      ├─ Session state machine (M3)                   │
│      └─ Transports: UDP / TCP listener (M3)          │
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
| `config` | Defaults, TOML and CLI precedence | M0 defaults, M1 loading |
| `cli` | Argument parsing and command dispatch | M0 skeleton |
| `doctor` | Environment and permission report | M0 |
| `transport::udp` | Wi-Fi UDP receive path | M1 |
| `transport::usbmux` | USB path through `usbmuxd` | M4 |
| `transport::trace` | Record and replay packet traces | M1 |
| `session` | Handshake, sequence window, heartbeats, watchdog | M1 |
| `mapping` | Normalized device → target rectangle | M1 |
| `inject` | Injector trait and platform backends | M1 trait, M2/M5/M6 backends |
| `stats` | Counters for drops, latency, errors | M1 |

### Session pipeline

Planned M1 flow:

1. The UDP receiver thread blocks on the socket with a short timeout.
2. It validates length, magic, version, and packet type with the protocol
   decoder, then checks the session identifier.
3. It forwards pointer samples over a bounded channel. A full channel is a
   visible, counted drop; the receiver never blocks the OS socket indefinitely.
4. The session loop consumes samples in order, applies mapping, and injects.
5. A watchdog checks the time since the last valid packet. After 750 ms of
   silence it releases all input. The same release runs on `BYE`, transport
   errors, Ctrl+C, and panic unwinding.
6. Heartbeats and `HELLO_ACK` are sent from the same loop so they share the
   session's view of time and state.

The design deliberately avoids an async runtime in M1. At 60 Hz a single
receiver thread plus a tight session loop is simpler to reason about than a
task graph, and the blocked socket read doubles as the idle wait. Later
backends can change this if measurements justify it.

### Injection abstraction

The session layer talks to an `Injector` trait (planned M1):

```rust
trait Injector {
    fn inject(&mut self, sample: &MappedSample) -> Result<(), InjectError>;
    fn release_all(&mut self) -> Result<(), InjectError>;
}
```

- `MockInjector` records events for tests and supports assertions on exact
  sequences.
- The Linux backend uses `uinput` (M2). It is the reference implementation and
  supports pressure, tilt, hover, and eraser.
- Windows uses a synthetic pen device where possible and `SendInput` as a
  fallback (M5, beta).
- macOS uses `CGEvent` mouse events (M6, beta). It requires Accessibility
  permission and cannot provide full tablet semantics.

OS-specific code stays inside `desktop/src/inject/`. Everything above the trait
must be testable on any platform.

### Mapping

The wire format carries normalized coordinates. The host maps them to a target
rectangle:

- Default target: the primary display's full desktop.
- Default aspect behavior: preserve the active area's aspect ratio and center
  it inside the target, leaving unused margins. A configuration option can
  stretch instead.
- No smoothing or prediction on the host by default. Predicted samples from the
  iPad are flagged and are not injected as separate contacts.
- Pressure defaults to 32,768 when `CAP_PRESSURE` is absent.
- Tilt is projected onto the backend's axes; the Linux backend uses
  `ABS_TILT_X` / `ABS_TILT_Y`.
- Hover updates position with the pen tool in range but no contact.
- Finger samples are tagged and can be ignored or injected as touch depending
  on configuration.

Mapping is pure and unit-tested without any OS backend.

## iPad app

| Layer | Responsibility | Status |
| --- | --- | --- |
| `ios/App` | SwiftUI shell, later the drawing surface | M0 shell |
| `AbsoluteTouchCore` | Protocol, session, transports | M0 protocol |
| UI tests | Not used for protocol logic | - |

### Why a Swift package

The core package has no UIKit or SwiftUI dependency. It builds and tests on
Linux, which is what CI uses, and it keeps protocol changes honest: the same
golden vectors run against the Rust and Swift codecs.

### Why SwiftUI plus UIKit

SwiftUI is a good fit for status, pairing, and settings screens. The drawing
surface needs precise touch timestamps, coalesced touches, predicted touches,
and stylus altitude/azimuth. A `UIView` receives those through `UIEvent` and
`UITouch`, which SwiftUI gestures do not expose directly. The app therefore
uses a SwiftUI shell around a UIKit canvas (M3). See
[ADR 0004](decisions/0004-ios-shell-and-core-package.md).

### Concurrency

The app uses Swift 6 language mode with strict concurrency:

- The canvas and view models are `@MainActor`.
- A session actor (M3) owns the transport, sequence counter, and session state.
- Input is batched into the actor instead of spawning one task per touch sample.
- The UI never blocks on socket writes.

Protocol types are value types and `Sendable`. The codec has no shared mutable
state.

### Streaming lifecycle

Planned M3 state machine:

```text
idle → listening/advertising → handshaking → streaming → stopping
             ↑                       │            │
             └──── error/timeout ────┴────────────┘
```

Streaming only runs while the app is in the foreground; iOS suspends the app
when the canvas is not visible, and the desktop watchdog then releases input.
No background audio or background location tricks are used.

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
  a valid packet, on `BYE`, on transport close, and on process shutdown.
- The iPad stops sending and releases on app backgrounding or transport error.
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
