# AbsoluteTouch

AbsoluteTouch turns an iPad and Apple Pencil into an absolute graphics tablet for
a desktop computer. It streams Pencil position, contact, tilt, hover, and eraser
state over Wi-Fi UDP or USB, then injects it as an absolute device.

The project is in early development. **M3** adds the iPad sender: a pairing
screen, a UIKit canvas that captures Pencil and finger input, and a Wi-Fi UDP
session that streams hover and tilt to the desktop. The app has not yet been
validated on real hardware, so treat M3 as beta; the Linux desktop path (M2) is
the validated reference.

## Status

| Area                      | State                          |
| ------------------------- | ------------------------------ |
| Protocol + golden vectors | Implemented and tested         |
| Desktop Wi-Fi session     | Implemented and tested (M1)    |
| Swift core package        | Implemented and tested         |
| Linux `uinput` tablet     | Implemented and tested (M2)    |
| iPad app + Wi-Fi sender   | Implemented, needs hardware validation (M3) |
| USB streaming             | Planned (M4)                   |
| Windows / macOS injection | Planned (M5/M6)                |
| TestFlight / releases     | Workflows ready (manual)       |

## Hardware note

The reference device is an iPad Air (M4) with Apple Pencil (USB-C). That Pencil
supports tilt and hover but **not pressure** and no Pencil Pro gestures. The
protocol reserves fields for pressure and twist so other Pencils can use them
later; the host substitutes a configurable level when pressure is unavailable.

## Requirements

- Rust 1.85+ (edition 2024)
- Swift 6.4+ for the core package
- Ruby 3.4+ for the Fastfile syntax check and the iOS release lane
- macOS with Xcode 27 and XcodeGen for iOS builds (CI only in this repository)
- Linux: write access to `/dev/uinput` for the tablet backend

## Quick start

```sh
git clone https://github.com/David735453/AbsoluteTouch.git
cd AbsoluteTouch

make test          # Rust + Swift core tests
make check         # formatting, lints, vectors, Ruby syntax
make hooks         # enable the tracked pre-commit hook
make doctor        # CLI environment report
```

Run a session:

```sh
cd desktop
cargo run -- token --reset                       # create a pairing token
cargo run -- run                                 # Linux: inject through uinput
cargo run -- run --dry-run                       # decode without injecting
cargo run -- run --backend mock --record demo.atrace
cargo run -- replay demo.atrace --dry-run
```

On the iPad, generate the Xcode project from `ios/project.yml`, run the app,
and enter the desktop's LAN address, port `37689`, and the token. See
[setup](docs/setup.md) for the full pairing walkthrough.

```sh
brew install xcodegen
cd ios
xcodegen generate --spec project.yml
open AbsoluteTouch.xcodeproj
```

Build and run the desktop CLI:

```sh
cd desktop
cargo run -- doctor
```

## Repository layout

```text
protocol/               Wire format and shared golden vectors
desktop/                Rust CLI and desktop library
ios/AbsoluteTouchCore/  UI-free Swift logic with tests
ios/App/                SwiftUI pairing shell and UIKit canvas
docs/                   Architecture, protocol, setup, testing, ADRs
.github/workflows/      Manual, per-platform CI
```

## Documentation

- [Architecture](docs/architecture.md)
- [Protocol](docs/protocol.md)
- [Setup](docs/setup.md)
- [Testing](docs/testing.md)
- [Architecture decisions](docs/decisions/)

## CI

All workflows are manual and expose per-platform toggles:

- `.github/workflows/ci.yml` — a format/lint job (vectors, rustfmt, swift-format,
  Ruby and config syntax) plus Linux x86_64/ARM64, Windows x64/ARM64, macOS
  iOS build, Swift core tests.
- `.github/workflows/release.yml` — build selected platform binaries and
  publish a GitHub release.
- `.github/workflows/testflight.yml` — archive and optionally upload to
  internal TestFlight.

## Safety

AbsoluteTouch injects input into your desktop, so it is designed to fail safe: a
watchdog releases all buttons when packets stop, on disconnect, or on shutdown.
Wi-Fi pairing uses a token plus a source-IP pin and is intended for trusted home
networks. The token is not encryption.

## License

MIT. See `LICENSE`.
