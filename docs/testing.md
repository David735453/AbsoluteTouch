# Testing

Every behavior gets a test. Protocol changes get a golden vector. Platform
claims require manual validation on real hardware, not just a successful
compile.

## Test layers

| Layer | What it covers | Where |
| --- | --- | --- |
| Protocol unit tests | Length, magic, version, type, token, round-trip | `desktop/src/protocol/` |
| Golden vectors | Cross-language byte agreement | `protocol/vectors.json` |
| CLI tests | Argument parsing and defaults | `desktop/src/cli.rs` |
| Config tests | Defaults, precedence, validation, example file | `desktop/src/config.rs` |
| Mapping tests | Stretch, offset, letterbox, flag passthrough | `desktop/src/mapping.rs` |
| Session tests | Handshake, token, peer pin, sequence window, heartbeat, watchdog | `desktop/src/session.rs` |
| Injector tests (mock) | Exact event sequences through `MockInjector` | `desktop/src/inject/mock.rs` |
| Injector tests (Linux) | Frames, tilt, tool switching, release; real device when permitted | `desktop/src/inject/linux.rs` |
| Trace tests | Round-trip, offsets, truncation, corrupt packets | `desktop/src/transport/trace.rs` |
| UDP tests | Local socket receive, decode errors, replies, shutdown | `desktop/src/transport/udp.rs` |
| End-to-end | HELLO/SAMPLE/BYE over loopback, record then replay | `desktop/src/run.rs` |
| Swift core tests | Same protocol and vectors as Rust | `ios/AbsoluteTouchCore/Tests/` |
| Swift session tests | Handshake, heartbeat RTT, timeout re-handshake, sequence wrap | `ios/AbsoluteTouchCore/Tests/AbsoluteTouchCoreTests/SessionMachineTests.swift` |
| Swift controller tests | Fake datagram transport, state stream, failure, `BYE` | `ios/AbsoluteTouchCore/Tests/AbsoluteTouchCoreTests/SessionControllerTests.swift` |
| Swift pointer tests | Coordinate/angle normalization, phase flags, token parsing | `PointerSampleTests.swift`, `PairingTokenTests.swift` |
| Manual hardware tests | Real Pencil, real backend, real OS | Checklist below |

## Commands

```sh
# Everything that runs on Linux
make check        # vectors, rustfmt, clippy, swift-format, ruby -c
make test
make doctor

# Rust only
cd desktop
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check

# Swift core only, on Linux or macOS
swift test --package-path ios/AbsoluteTouchCore
swift format lint --strict --recursive ios/AbsoluteTouchCore ios/App

# The app target needs Xcode; CI builds it unsigned for the simulator
xcodegen generate --spec ios/project.yml

# Optional SwiftLint (repository config in .swiftlint.yml)
swiftlint lint --quiet --strict

# Ruby files used by the iOS release lane
ruby -c ios/Gemfile
ruby -c ios/fastlane/Fastfile

# Golden vectors are current
python3 tools/gen_vectors.py --check
```

A change is not ready when any of these fails locally.

## Local pre-commit hook

`make hooks` points `core.hooksPath` at `.githooks/`, so the tracked
`.githooks/pre-commit` runs on every commit. `make hooks-off` restores the
default. The hook only looks at staged files:

| Staged files | Check |
| --- | --- |
| `desktop/**/*.rs`, `desktop/Cargo.toml` | `cargo fmt --check` |
| `ios/**/*.swift` | `swift format lint --strict`, plus `swiftlint` when it can start |
| `protocol/`, `tools/gen_vectors.py` | `gen_vectors.py --check` |
| `ios/Gemfile`, `ios/fastlane/Fastfile` | `ruby -c` |

The hook reports problems and the make target that fixes them; it never rewrites
files. Missing tools (`swift`, `swiftlint`, `ruby`) are skipped with a note, so a
partial toolchain does not block commits. CI always runs the checks, so a skip
locally cannot slip through.

## Formatting rules

- Indentation: Swift uses tabs displayed two columns wide; Rust uses rustfmt's
  four spaces. `.editorconfig` and `.swift-format` agree with this.
- Rust: `rustfmt.toml`, enforced by `cargo fmt --check`.
- Swift: `.swift-format`, enforced by `swift format lint --strict` (bundled
  with the Swift toolchain). Without `--strict`, swift-format reports findings
  but still exits 0, so CI would not fail.
- SwiftLint: `.swiftlint.yml`, layered on top of swift-format for rules the
  formatter does not cover. Run in CI on macOS, optional locally.
- Rubocop is not used; the Ruby surface is a Gemfile and a Fastfile that only
  need syntax checks.

## Golden vectors

`protocol/vectors.json` is the contract between the Rust desktop and the Swift
iPad core. Both suites decode each vector, compare every field, and re-encode
it byte for byte. A stale vector file or a one-byte disagreement fails both
builds.

Adding or changing a vector:

1. Update `tools/gen_vectors.py`.
2. Run `python3 tools/gen_vectors.py`.
3. Run both test suites.
4. Run `python3 tools/gen_vectors.py --check`.

When the packet layout changes, bump the protocol version and update the
vectors, `docs/protocol.md`, and both implementations in the same change. Never
reorder or repurpose a field silently.

## CI

All workflows are manual (`workflow_dispatch`). There are no push,
pull-request, or schedule triggers, and `pull_request_target` is forbidden.

### `ci.yml`

| Input | Runner | What runs |
| --- | --- | --- |
| `lint` | `ubuntu-26.04` | Golden vectors, `cargo fmt --check`, `swift format lint --strict` (in `swift:6.4.1`), `ruby -c`, YAML/JSON/plist parse checks |
| `linux` | `ubuntu-26.04` | Vector check, Rust fmt/clippy/tests, doctor, Swift format check and Swift core tests in the `swift:6.4.1` container |
| `linux_arm` | `ubuntu-26.04-arm` | Vector check, Rust fmt/clippy/tests |
| `windows` | `windows-latest` | Rust fmt/clippy/tests (beta) |
| `windows_arm` | `windows-11-arm` | Rust fmt/clippy/tests (beta) |
| `macos` | `macos-latest` | Xcode 27, Rust tests, Swift tests, `swift format lint --strict`, SwiftLint, XcodeGen, unsigned simulator build |

Trigger from the Actions UI or with the CLI:

```sh
gh workflow run ci.yml -f linux=true -f macos=true -f windows=false
```

### `release.yml`

Manual, versioned build. Inputs select platforms and whether the GitHub release
is a draft. Linux x86_64/ARM64, Windows x64/ARM64, and macOS ARM64 are built
natively on their runners. Only the publish job has `contents: write`; every
other job is read-only.

### `testflight.yml`

Manual archive and optional upload. It runs in the `testflight` environment and
requires `ASC_KEY_ID`, `ASC_ISSUER_ID`, `ASC_KEY_P8_BASE64`, and
`APPLE_TEAM_ID`. Upload is off by default; set the `upload` input to build
first and upload only when intended. The workflow never runs for pull requests.

## Recording and replay

Manual repetition is unreliable for latency and injection regressions. The
desktop CLI records received packets and replays them through the same session
and mapping code:

```sh
absolutetouch run --record session.atrace
absolutetouch replay session.atrace --dry-run
absolutetouch replay session.atrace --speed 2
```

Replay tests use `MockInjector` and assert the exact injected sequence. The
trace format is internal and versioned (`ATTR`, version 1, 8-byte header,
48-byte records); it is not the wire protocol. `--record` keeps rejected
packets too, so a trace can reproduce misbehaving clients.

## Manual hardware checklist

Run this before calling a platform or milestone supported. Attach the OS,
backend, transport, iPad model, and Pencil model to the result.

### Connection

- [ ] `doctor` reports the backend and permissions correctly.
- [ ] On Linux, `run` registers **AbsoluteTouch Tablet** with the expected axes
      and keys in `/proc/bus/input/devices` and `libinput list-devices`.
- [ ] `HELLO` / `HELLO_ACK` completes within one second.
- [ ] A wrong token is rejected and logged.
- [ ] A second iPad with the wrong token or source IP cannot take over.
- [ ] `BYE` ends the session and releases input.

### iPad app (M3)

- [ ] The pairing screen keeps the address, port, and token across launches.
- [ ] The first session shows the local network prompt.
- [ ] The HUD shows **Connecting** and then **Streaming** within one second of
      `HELLO_ACK`.
- [ ] A wrong token stays on **Connecting** and is rejected in the desktop log.
- [ ] Rotating the iPad, or switching to Split View, keeps the active area
      aligned with the canvas bounds.
- [ ] Holding the Pencil still for longer than the watchdog window keeps the
      contact alive.
- [ ] Lifting the Pencil sends an out-of-range sample and clears contact.
- [ ] Stopping and backgrounding send `BYE`; the desktop releases immediately.
- [ ] Restarting the desktop mid-session re-handshakes without the user
      restarting the app, and the desktop accepts samples afterwards.

### Position and contact

- [ ] Cursor reaches all four corners of the configured target.
- [ ] Aspect ratio and letterboxing match the configuration.
- [ ] Press, drag, and release produce no stuck contact.
- [ ] Lifting the Pencil releases the button immediately.

### Pencil features

- [ ] Hover moves the cursor without pressing.
- [ ] Leaving range sends `OUT_OF_RANGE` and clears hover.
- [ ] Tilt changes both axes in a tablet tester, for contact and for hover.
- [ ] Pressure is present only on hardware that supports it; the reference
      Pencil (USB-C) uses the default level.
- [ ] Eraser and finger samples behave as configured.
- [ ] The display runs at its real rate; do not claim 120/240 Hz for the
      reference iPad Air (M4).

### Failure and safety

- [ ] Wi-Fi loss mid-stroke releases input within the watchdog window.
- [ ] Unplugging USB mid-stroke releases input.
- [ ] Backgrounding the app mid-stroke releases input.
- [ ] Killing the desktop process mid-stroke releases input.
- [ ] Reconnecting starts a new session with a new `session_id`.
- [ ] A flood of malformed packets is dropped without a panic or leak.

### osu!

- [ ] osu! stable works with the selected backend.
- [ ] osu! lazer works with the selected backend.
- [ ] Absolute positioning matches the chosen play area.
- [ ] Raw Input settings are documented for the tested OS.

## Support policy

- **Linux** is the reference platform. It is supported after the M2 manual
  checklist passes on real hardware.
- **The iPad app** is beta until the M3 manual checklist passes on a real iPad
  and Pencil. A green core test run or a simulator build does not make it
  supported.
- **Windows** and **macOS** are beta until their manual checklists pass on real
  hardware. A green compile job is not support.
- **TestFlight** builds are beta by definition; they do not imply App Store
  readiness.
- The reference device is an iPad Air (M4) with Apple Pencil (USB-C). Other
  iPads and Pencils may work, but they need their own validation.

## Adding tests

- Put pure logic behind the injector trait so it runs without `/dev/uinput`,
  Windows, or macOS.
- Prefer a recorded trace over a hand-written sequence of hundreds of samples.
- Keep protocol tests deterministic; do not depend on wall-clock time.
- Do not add simulator UI automation unless the workflow explicitly selects a
  simulator and the test adds real value.
- Never weaken a test to make a platform job pass. Fix the behavior or mark the
  platform unsupported.
