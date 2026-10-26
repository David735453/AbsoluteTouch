# Setup

This guide covers building the desktop CLI, building the iPad app, and
preparing Linux for tablet injection. The desktop Wi-Fi session, trace replay,
and Linux `uinput` backend exist today (M1 and M2); the iPad sender (M3), USB
(M4), and the Windows/macOS backends (M5, M6) are marked as planned.

## Requirements

| Component | Requirement |
| --- | --- |
| Desktop | Rust 1.85 or newer (edition 2024) |
| iPad app build | macOS, Xcode 27, XcodeGen |
| iOS | iPadOS 17.0 or newer |
| Linux backend | Write access to `/dev/uinput`; read access to `/dev/input/event*` for `evtest` |
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

# Linux: inject through uinput while recording
absolutetouch run --record session.atrace

# Replay through the same session and mapping code, no socket
absolutetouch replay session.atrace --dry-run
absolutetouch replay session.atrace --speed 2
```

`--backend mock` and `--dry-run` never touch the OS. Replay without `--dry-run`
injects the recorded samples; use `--dry-run` when you only want to inspect a
trace.

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
writable`. Reading `/dev/input/event*` is only needed for verification with
`evtest`; the desktop writes samples and does not read them back.

Start a session and check that the device is registered:

```sh
grep -A12 "AbsoluteTouch Tablet" /proc/bus/input/devices
```

The entry should show `EV=b` (key, absolute axes, and synchronization) and an
`ABS=` mask covering `ABS_X`, `ABS_Y`, `ABS_PRESSURE`, `ABS_TILT_X`, and
`ABS_TILT_Y`. The `KEY=` mask covers `BTN_TOOL_PEN`, `BTN_TOOL_RUBBER`,
`BTN_TOOL_FINGER`, `BTN_TOUCH`, and `BTN_STYLUS`. While the process runs, the
device appears as **AbsoluteTouch Tablet** in desktop input settings and in
`libinput list-devices`.

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

The desktop Wi-Fi receiver (M1) and the Linux `uinput` backend (M2) are
implemented. The iPad sender arrives in M3 and USB in M4, so `--transport usb`
and `devices` still report "not implemented".

```sh
# Normal Linux path: receive over Wi-Fi and inject through uinput
absolutetouch run

# Same, with the backend named explicitly
absolutetouch run --backend uinput

# Record the session while injecting
absolutetouch run --record session.atrace

# Decode and print without creating a device
absolutetouch run --dry-run
absolutetouch run --backend mock

# Replay a trace; without --dry-run this injects into the desktop
absolutetouch replay session.atrace --speed 2
absolutetouch replay session.atrace --dry-run

# Inspect the environment
absolutetouch doctor

# Show or reset the pairing token
absolutetouch token
absolutetouch token --reset
```

`auto` selects `uinput` on Linux. `--dry-run` always selects the mock backend
and prints each sample, even when another backend is configured. Replay without
`--dry-run` injects the recorded samples; use `--dry-run` when you only want to
inspect a trace.

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
# Set this to the desktop aspect ratio; without it the mapping stretches.
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
conflicting ways. On Linux the virtual device is named **AbsoluteTouch Tablet**;
select it where the game or desktop asks for a tablet.

## Troubleshooting

| Symptom | Check |
| --- | --- |
| No cursor movement | `doctor`, firewall, correct desktop IP, token, iPad foreground |
| Cursor moves but buttons stick | Stop the desktop; the watchdog should release. Report a bug with a trace |
| `doctor` says `/dev/uinput` missing | Load the module, fix the udev rule, log out and in |
| `uinput` is present but not writable | Recheck the udev rule and group membership; `newgrp input` or log in again |
| No **AbsoluteTouch Tablet** device | The session process must be running; check `/proc/bus/input/devices` |
| Game ignores the tablet | Select the device explicitly, or check Raw Input/tablet settings |
| Local network prompt never appears | Reinstall or reset the app; check Settings → Privacy → Local Network |
| USB device not listed | Trust the iPad, check `usbmuxd`, try `idevice_id -l` |
| iOS build fails on signing | Set the development team; CI builds use `CODE_SIGNING_ALLOWED=NO` |
| TestFlight upload fails | Check the `testflight` environment secrets and API key permissions |

When reporting a bug, include the OS, backend, transport, and a recorded trace.
Never include tokens or personal network details.
