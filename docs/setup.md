# Setup

This guide covers building the desktop CLI, building the iPad app, and
preparing Linux for tablet injection. The desktop Wi-Fi session and trace replay
exist today (M1); the iPad sender (M3), USB (M4), and OS backends (M2, M5, M6)
are marked as planned.

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

### Configuration and environment

`absolutetouch run` reads `~/.config/absolutetouch/config.toml` if it exists;
`--config PATH` selects another file. The format is described under
[Configuration file](#configuration-file). Environment variables override the
file:

| Variable | Meaning |
| --- | --- |
| `ABSOLUTETOUCH_BIND` | Listener address |
| `ABSOLUTETOUCH_UDP_PORT` | Wi-Fi UDP port (default 37689) |
| `ABSOLUTETOUCH_TCP_PORT` | USB TCP port (default 37690) |
| `ABSOLUTETOUCH_TRANSPORT` | `wifi` or `usb` |
| `ABSOLUTETOUCH_TOKEN` | 32 hex characters |
| `ABSOLUTETOUCH_BACKEND` | `auto`, `mock`, `uinput`, `windows`, `macos` |
| `ABSOLUTETOUCH_DRY_RUN` | `true` or `false` |
| `ABSOLUTETOUCH_WATCHDOG_MS` | Release timeout in milliseconds |
| `ABSOLUTETOUCH_SESSION_TIMEOUT_MS` | Session timeout in milliseconds |
| `ABSOLUTETOUCH_PRESSURE_DEFAULT` | Fallback pressure level |

### Run and replay

```sh
# Create a pairing token and write it to the config file
absolutetouch token --reset

# Wi-Fi session without OS injection
absolutetouch run --dry-run --record session.atrace

# Mock backend: records injector decisions without touching the OS
absolutetouch run --backend mock --record session.atrace

# Replay through the same session and mapping code, no socket
absolutetouch replay session.atrace --dry-run
absolutetouch replay session.atrace --speed 2
```

M1 has no OS backend yet, so `--backend mock` and `--dry-run` are the supported
session modes. Linux `uinput` arrives in M2.

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

## Streaming

The desktop Wi-Fi receiver is implemented (M1). The iPad sender arrives in M3
and USB in M4, so `--transport usb` and `devices` still report "not
implemented".

```sh
# Wi-Fi, recording injector decisions without touching the OS (M1)
absolutetouch run --backend mock --record session.atrace

# Decode and print without injecting
absolutetouch run --dry-run

# Replay a trace through the same session and mapping code
absolutetouch replay session.atrace --dry-run
absolutetouch replay session.atrace --speed 2

# Inspect the environment
absolutetouch doctor

# Show or reset the pairing token
absolutetouch token
absolutetouch token --reset
```

Once the Linux `uinput` backend lands in M2, `absolutetouch run` without
`--backend mock` or `--dry-run` becomes the normal path.

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

### Configuration file

The desktop reads a TOML file and applies CLI overrides last:

1. Command-line flags
2. Environment variables
3. Configuration file
4. Built-in defaults

Default paths:

| Condition | Path |
| --- | --- |
| `XDG_CONFIG_HOME` is set | `$XDG_CONFIG_HOME/absolutetouch/config.toml` |
| Otherwise, `HOME` is set | `$HOME/.config/absolutetouch/config.toml` |

`--config PATH` overrides the default. Platform-native locations (macOS
Application Support, Windows `%APPDATA%`) arrive with those backends; until
then use `--config` there.

Example shape:

```toml
transport = "wifi"
bind = "0.0.0.0"
udp_port = 37689
tcp_port = 37690
token = "000102030405060708090a0b0c0d0e0f"
watchdog_ms = 750
session_timeout_ms = 5000
pressure_default = 32768
dry_run = false
backend = "auto"

[mapping]
aspect = "preserve"
source_aspect = 1.3333333333333333
# Filled from the display in M2; without it the mapping stretches.
# target_aspect = 1.7777777777777777

[mapping.target]
x = 0
y = 0
width = 65535
height = 65535
```

[`desktop/absolutetouch.example.toml`](../desktop/absolutetouch.example.toml)
documents every key, and a test keeps it parsing and validating. Unknown keys
are rejected rather than silently ignored.

### Pairing

The desktop generates a 16-byte token with `absolutetouch token --reset`. The
iPad must present the same token in `HELLO`; a mismatch is rejected and logged,
without a reply. While a session is active the desktop pins it to the source
address that completed the handshake, so another peer cannot take over.
Regenerating the token invalidates the old pairing.

The token is sent in clear text and is not encryption. Use it on a trusted
network, or use USB. The iPad-side token entry arrives with the app in M3.

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
