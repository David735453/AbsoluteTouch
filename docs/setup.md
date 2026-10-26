# Setup

This guide covers building the desktop CLI, building the iPad app, and
preparing Linux for tablet injection. Some steps describe features that arrive
in M1 through M4; they are marked as planned.

## Requirements

| Component | Requirement |
| --- | --- |
| Desktop | Rust 1.85 or newer (edition 2024) |
| iPad app build | macOS, Xcode 27, XcodeGen |
| iOS | iPadOS 17.0 or newer |
| Linux backend | `/dev/uinput` access |
| Swift core tests | Swift 6.4 or newer (Linux or macOS) |

The core package declares `swift-tools-version: 6.4`; CI runs it in the
`swift:6.4.1` container. The iOS app's `SWIFT_VERSION: "6.0"` is the Swift
language mode, not the compiler version — Xcode 27 provides the compiler and the
iOS 27 SDK while the deployment target stays at 17.0.

## Desktop

```sh
git clone https://github.com/David735453/AbsoluteTouch.git
cd AbsoluteTouch

cd desktop
cargo test --all-targets
cargo run -- doctor
```

`doctor` reports the OS, architecture, `/dev/uinput` state, and whether the
`usbmuxd` socket is present. It exits successfully even when a backend is
missing; use the report to fix the environment.

Repository helpers:

```sh
make test       # Rust + Swift core tests
make check      # formatting, lints, and vector check
make lint       # cargo clippy with warnings as errors
make fmt        # cargo fmt
make fmt-check  # formatting check only
make hooks      # enable the tracked pre-commit hook
make doctor     # CLI environment report
```

### Linux: `/dev/uinput`

The M2 backend creates a virtual absolute tablet through `uinput`. Load the
module and grant access before running a session:

```sh
sudo modprobe uinput
```

Persist the module:

```sh
echo uinput | sudo tee /etc/modules-load.d/uinput.conf
```

Create a udev rule, for example `/etc/udev/rules.d/70-absolutetouch.rules`:

```udev
KERNEL=="uinput", SUBSYSTEM=="misc", TAG+="uaccess", OPTIONS+="static_node=uinput"
```

Reload the rules and replug or reboot:

```sh
sudo udevadm control --reload-rules
sudo udevadm trigger
```

`TAG+="uaccess"` grants the active desktop session access. On a headless or
unusual setup, add your user to the `input` group instead and log in again:

```sh
sudo usermod -aG input "$USER"
```

Verify with `cargo run -- doctor`; it should print `uinput: present and
writable`.

## iPad app

The generated Xcode project is not committed. Generate it from
`ios/project.yml`:

```sh
brew install xcodegen
cd ios
xcodegen generate --spec project.yml
open AbsoluteTouch.xcodeproj
```

Then:

1. Select the `AbsoluteTouch` target.
2. Set your development team under **Signing & Capabilities**.
3. Connect an iPad and run the app, or archive it for TestFlight.

The bundle identifier is `org.dstudios.absolutetouch`. The minimum deployment
target is iPadOS 17.0.

### Local network permission

Wi-Fi streaming triggers the local network prompt. The usage description is in
`ios/App/Info.plist`. If permission was denied, enable it under
**Settings → Privacy & Security → Local Network**.

USB streaming does not use the local network. It still requires the iPad to be
trusted by the desktop and `usbmuxd` to see the device.

## Streaming (planned)

Streaming is implemented in M1 (desktop UDP), M3 (iPad app), and M4 (USB). The
intended commands are:

```sh
# Wi-Fi
absolutetouch run --transport wifi

# USB
absolutetouch run --transport usb

# Decode and print without injecting
absolutetouch run --transport wifi --dry-run

# Record and replay
absolutetouch run --transport wifi --record session.atrace
absolutetouch replay session.atrace

# Inspect the environment
absolutetouch doctor

# Show or reset the pairing token
absolutetouch token
absolutetouch token --reset
```

### Ports

| Transport | Protocol | Port | Listener |
| --- | --- | ---: | --- |
| Wi-Fi | UDP | 37689 | Desktop |
| USB | TCP | 37690 | iPad via `usbmuxd` |

Allow inbound UDP 37689 in the desktop firewall for Wi-Fi sessions. Do not
expose it to the internet.

For `ufw`:

```sh
sudo ufw allow 37689/udp
```

For `firewalld`:

```sh
sudo firewall-cmd --add-port=37689/udp --permanent
sudo firewall-cmd --reload
```

### Configuration (planned M1)

The desktop reads a TOML file and applies CLI overrides last:

1. Command-line flags
2. Environment variables
3. Configuration file
4. Built-in defaults

Default paths:

| OS | Path |
| --- | --- |
| Linux | `$XDG_CONFIG_HOME/absolutetouch/config.toml` or `~/.config/absolutetouch/config.toml` |
| macOS | `~/Library/Application Support/absolutetouch/config.toml` |
| Windows | `%APPDATA%\absolutetouch\config.toml` |

Example shape:

```toml
[transport]
mode = "wifi"
udp_port = 37689
tcp_port = 37690

[inject]
backend = "auto"
default_pressure = 32768

[mapping]
target = "primary"
preserve_aspect = true

[pairing]
token = "replace-with-a-generated-token"
```

Exact keys may change before M1 ships; the CLI is the source of truth.

### Pairing (planned M3)

The desktop generates a 16-byte token. The iPad must present the same token in
`HELLO`, and the desktop may additionally pin the iPad's source IP. A mismatch
is rejected and logged. Regenerating the token invalidates the old pairing.

The token is sent in clear text and is not encryption. Use it on a trusted
network, or use USB.

## osu!

The target is both osu! stable and osu! lazer.

- On Linux, `uinput` is the reference backend. lazer runs natively; stable runs
  through Wine, where tablet support depends on the Wine version. Validate
  before relying on it.
- On Windows, the M5 backend uses a synthetic pen device with a `SendInput`
  fallback. In osu! stable, disable **Raw Input** if the game ignores the
  absolute device. Test both stable and lazer.
- On macOS, the M6 backend uses `CGEvent` and is beta. Grant Accessibility and
  Input Monitoring permission.

The iPad active area maps to a configurable desktop rectangle. Use the osu!
tablet area settings or the AbsoluteTouch mapping options, not both in
conflicting ways.

## Troubleshooting

| Symptom | Check |
| --- | --- |
| No cursor movement | `doctor`, firewall, correct desktop IP, token, iPad foreground |
| Cursor moves but buttons stick | Stop the desktop; the watchdog should release. Report a bug with a trace |
| `doctor` says `/dev/uinput` missing | Load the module, fix the udev rule, log out and in |
| Local network prompt never appears | Reinstall or reset the app; check Settings → Privacy → Local Network |
| USB device not listed | Trust the iPad, check `usbmuxd`, try `idevice_id -l` |
| iOS build fails on signing | Set the development team; CI builds use `CODE_SIGNING_ALLOWED=NO` |
| TestFlight upload fails | Check the `testflight` environment secrets and API key permissions |

When reporting a bug, include the OS, backend, transport, and a recorded trace
when M1 lands. Never include tokens or personal network details.
