# AGENTS.md

## What this is

AbsoluteTouch streams Apple Pencil and finger input from an iPad to a desktop
machine and injects it as an absolute tablet. The iPad is the input device; the
Rust desktop CLI is the receiver and OS-level injector.

**Current status:** M3 in progress. The desktop is at M2 (Wi-Fi session,
config, mapping, trace replay, mock injector, Linux `uinput` absolute tablet).
The iPad sender — pairing form, UIKit canvas, Swift session machine/controller,
and Wi-Fi UDP transport — is implemented and unit-tested on Linux, but it has
not been built for a device or validated on real hardware yet. Windows/macOS
injection is not implemented.

**Read first:**

1. `docs/architecture.md`
2. `docs/protocol.md`
3. `docs/testing.md`
4. `docs/decisions/`

## Git policy (non-negotiable)

- **Never push.** Not to `origin`, not to a fork, not to a tag.
- **Never commit without explicit approval.** Wait for the user to say
  "commit" or "make a commit" for the specific change.
- Never amend, rebase, force-push, create a tag, or open a pull request without
  explicit approval.
- Before asking for commit approval, show `git status --short` and
  `git diff --stat`.
- The repository is public. Never add secrets, API keys, `.p8` files,
  provisioning profiles, pairing tokens, private IP addresses, or personal
  information. Use placeholders in docs and CI examples.

## Locked decisions

| Area | Decision |
| --- | --- |
| Bundle ID | `org.dstudios.absolutetouch` |
| Desktop | Rust CLI plus TOML config. No GUI in v1. |
| Transports | Wi-Fi UDP and USB via `usbmuxd`. No Ethernet. |
| USB direction | iPad is the TCP server; desktop is the client. |
| Pencil | Apple Pencil (USB-C): tilt and hover, **no pressure**, no Pencil Pro features. |
| Pressure | Protocol field exists for future Pencils; host uses a default level when unsupported. |
| Hover | In scope for M3 because the iPad Air M4 supports it. |
| osu! | Stable and lazer; Linux `uinput` is the reference backend. |
| License | MIT |
| Minimum iPadOS | 17.0 |
| Display | iPad Air M4 is 60 Hz; do not claim 120/240 Hz. |

## Skills: load before writing code

Skills live in `.agents/skills/` and are also available through the skill tool.

- Rust: load `albo-rust`, `coding-guidelines`, and `domain-cli` before writing
  or changing any `.rs` file.
- Swift concurrency: load `swift-concurrency` before changing concurrency,
  actors, `Sendable`, or `@MainActor` code.
- SwiftUI: load `swiftui-pro` before changing SwiftUI views.
- Use `rust-router` / `m01`-`m15` skills when debugging specific Rust issues.

Do not edit `.agents/`, `skills-lock.json`, or `opencode.jsonc` unless the user
explicitly asks.

## Commands

```sh
# Rust desktop
cd desktop
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
cargo run -- doctor
cargo run -- run
cargo run -- run --backend mock
cargo run -- replay demo.atrace --dry-run

# Swift core (runs on Linux and macOS)
swift test --package-path ios/AbsoluteTouchCore
swift format lint --strict --recursive ios/AbsoluteTouchCore ios/App

# SwiftLint (optional; CI runs it on macOS)
swiftlint lint --quiet --strict

# iOS project generation (macOS only)
xcodegen generate --spec ios/project.yml

# Repository helpers
make setup
make toolchain
make check
make test
make lint
make fmt
make fmt-swift
make lint-ruby
make hooks
make doctor
```

`make setup` enables the tracked pre-commit hook and prints the next commands;
`make check`, `make test`, and `make lint` also enable it, so a fresh clone gets
the hook automatically. The hook runs rustfmt, clippy, `cargo test`, swift-format,
`swift test`, workflow YAML and plist parsing, golden vectors, and Ruby syntax for
the staged areas. `make toolchain` updates stable Rust and its `rustfmt`/`clippy`
components; `rust-toolchain.toml` selects the `stable` channel.

`.github/dependabot.yml` opens weekly grouped PRs for Cargo, SwiftPM, and GitHub
Actions. Merge them only after CI is green, and move to a new action major or
toolchain deliberately rather than pinning an old one.

`ci.yml` runs on every push to `main`, nightly (`17 4 * * *`), and on manual
dispatch. Pushes run Linux x86_64, macOS/iOS, and lint; the nightly also covers
Linux ARM64 and both Windows targets. `release.yml` stays manual.

## Protocol invariants

- Packet size is exactly **40 bytes**, little-endian.
- Magic is `0x5441`; wire bytes are `41 54` (`AT`).
- Version is `1`. A layout change requires a version bump.
- Offsets are fixed. Do not reorder or repurpose fields without updating
  `protocol/vectors.json`, `docs/protocol.md`, the Rust implementation, and the
  Swift implementation in the same change.
- `seq` wraps at 65,535; `t_device_us` wraps at about 71 minutes. Neither is
  used for absolute ordering.
- Capability bits are only valid in `HELLO` / `HELLO_ACK`.
- Golden vectors are the cross-language contract. Both test suites must read the
  same `protocol/vectors.json`.

## Testing rules

- Every behavior gets a test. Protocol changes get a golden vector.
- Run `make check` before asking for review. CI repeats every check with
  `--strict`, so locally green must mean strict-green.
- Keep OS-specific code inside `desktop/src/inject/`; the rest must be testable
  without `/dev/uinput`, Windows, or macOS.
- Use `MockInjector` for end-to-end tests.
- Prefer recorded traces over manual repetition: `--record` and `--replay`.
- Never mark a platform "supported" based only on a compile. Linux is the
  reference; Windows and macOS are beta until manually validated.
- Do not add UI automation that depends on a simulator being available unless
  the workflow explicitly selects one.

## CI / secrets

- Workflows: `.github/workflows/ci.yml`, `codeql.yml`, `release.yml`.
- `release.yml` carries the TestFlight toggle; there is no separate workflow.
- TestFlight secrets live in the `testflight` GitHub environment:
  `ASC_KEY_ID`, `ASC_ISSUER_ID`, `ASC_KEY_P8_BASE64`, `APPLE_TEAM_ID`.
- Never run TestFlight or release jobs on pull requests.
- Never use `pull_request_target`.
- Release jobs get `contents: write`; all other jobs are read-only.
- GitHub runner labels: `ubuntu-26.04`, `ubuntu-26.04-arm`, `windows-latest`,
  `windows-11-arm`, `macos-latest`, and `xcode-27` for iOS jobs. Xcode is
  selected by `maxim-lobanov/setup-xcode@v1` with `xcode-version: latest`.

## Repository map

```text
AGENTS.md              this file
README.md              public overview and quick start
protocol/              40-byte wire format and golden vectors
desktop/               Rust CLI, protocol, mapping, injectors
ios/AbsoluteTouchCore/ Swift package with UI-free logic and tests
ios/App/               SwiftUI pairing shell and UIKit canvas
ios/project.yml        XcodeGen project definition
docs/                  architecture, protocol, setup, testing, ADRs
.github/workflows/     push/nightly CI, Swift CodeQL, and manual release/TestFlight
.githooks/             tracked pre-commit hook (enabled by make setup/check/test/lint)
```

Do not edit the generated `ios/AbsoluteTouch.xcodeproj`; edit `ios/project.yml`
and regenerate on macOS.

## Current milestone

- [x] Repository skeleton
- [x] Protocol specification and golden vectors
- [x] Rust protocol implementation and tests
- [x] Swift protocol implementation and tests
- [x] Manual CI workflows for all platforms (defined; first runs pending)
- [x] M1: desktop session, UDP, config, replay, mock injector
- [x] M2: Linux `uinput` backend
- [ ] M3: iPad app, Wi-Fi end-to-end, hover (sender implemented; hardware
      validation pending)
- [ ] M4: USB via `usbmuxd`
- [ ] M5: Windows backends (beta)
- [ ] M6: macOS backend, TestFlight, releases

Update this checklist when a milestone is actually complete and tested.
