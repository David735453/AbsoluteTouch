# Setup

This guide covers building the desktop CLI, building the iPad app, and
preparing Linux for tablet injection. The desktop Wi-Fi session, trace replay,
the Linux `uinput` backend, the iPad sender, and the USB transport through
`usbmuxd` exist today (M1–M4). The Windows/macOS backends (M5, M6) are planned,
and the USB path still needs validation on real hardware.

## Requirements

| Component        | Requirement                                                                    |
| ---------------- | ------------------------------------------------------------------------------ |
| Desktop          | Rust 1.99 or newer (edition 2024)                                              |
| iPad app build   | macOS, Xcode 27, XcodeGen                                                      |
| iOS              | iPadOS 17.0 or newer                                                           |
| Linux backend    | Write access to `/dev/uinput`; read access to `/dev/input/event*` for `evtest` |
| Swift core tests | Swift 6.4 or newer (Linux or macOS)                                            |
| Markdown docs    | Node 24 or newer for `make fmt-md` and `make lint-md`                          |

The core package declares `swift-tools-version: 6.4`; CI installs Swift 6.4.1
with `SwiftyLab/setup-swift`. Markdown is formatted with the Prettier pinned in
`package-lock.json`, so run `npm install` once before `make fmt-md`; the checks
are skipped when `node_modules` is absent. The iOS app's `SWIFT_VERSION: "6.0"` is the Swift
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

`doctor` reports the OS, architecture, `/dev/uinput` state, and probes
`usbmuxd` by connecting to it and asking for the device list. It exits
successfully even when a backend is missing; use the report to fix the
environment. A stale `/var/run/usbmuxd` socket from a stopped daemon is reported
as unreachable, not as present.

Repository helpers:

```sh
make setup      # enable the tracked pre-commit hook
make toolchain  # update stable Rust and its rustfmt/clippy components
make test       # Rust + Swift core tests
make check      # formatting, lints, and vector check
make lint       # cargo clippy with warnings as errors
make fmt        # cargo fmt
make fmt-check  # formatting check only
make hooks      # enable the tracked pre-commit hook (idempotent)
make doctor     # CLI environment report
```

`make setup`, `make check`, `make test`, and `make lint` enable the hook
automatically, so a fresh clone gets the staged-file checks the first time it
runs any of them. The hook runs rustfmt, clippy, Rust tests, swift-format, Swift
tests, workflow YAML parsing, golden vectors, and Ruby syntax for the staged
areas; `make hooks-off` disables it.

### Configuration and environment

`absolutetouch run` reads `~/.config/absolutetouch/config.toml` if it exists;
`--config PATH` selects another file. The format is described under
[Configuration file](#configuration-file). Environment variables override the
file:

| Variable                           | Meaning                                      |
| ---------------------------------- | -------------------------------------------- |
| `ABSOLUTETOUCH_BIND`               | Listener address                             |
| `ABSOLUTETOUCH_UDP_PORT`           | Wi-Fi UDP port (default 37689)               |
| `ABSOLUTETOUCH_TCP_PORT`           | USB TCP port (default 37690)                 |
| `ABSOLUTETOUCH_TRANSPORT`          | `wifi` or `usb`                              |
| `ABSOLUTETOUCH_TOKEN`              | Pairing code or 32 hex characters            |
| `ABSOLUTETOUCH_BACKEND`            | `auto`, `mock`, `uinput`, `windows`, `macos` |
| `ABSOLUTETOUCH_DEVICE`             | `pointer` (default) or `tablet`              |
| `ABSOLUTETOUCH_DRY_RUN`            | `true` or `false`                            |
| `ABSOLUTETOUCH_WATCHDOG_MS`        | Release timeout in milliseconds              |
| `ABSOLUTETOUCH_SESSION_TIMEOUT_MS` | Session timeout in milliseconds              |
| `ABSOLUTETOUCH_PRESSURE_DEFAULT`   | Fallback pressure level                      |

### Run and replay

```sh
# Create a pairing code and write it to the config file
absolutetouch token --reset

# Wi-Fi session without OS injection
absolutetouch run --dry-run --record session.atrace

# USB session: list the cable-connected iPad, then receive through usbmuxd
absolutetouch devices
absolutetouch run --transport usb

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

Start a session and check that the device is registered. The name depends on
the `device` setting: `pointer` (the default) registers **AbsoluteTouch
Pointer**, and `device = "tablet"` registers **AbsoluteTouch Tablet**.

```sh
grep -A12 "AbsoluteTouch" /proc/bus/input/devices
```

Both devices show `EV=b` (key, absolute axes, and synchronization) and an
`ABS=` mask covering `ABS_X`, `ABS_Y`, `ABS_PRESSURE`, `ABS_TILT_X`, and
`ABS_TILT_Y`. The pointer device declares `BTN_LEFT` and `BTN_RIGHT`; the
tablet declares `BTN_TOOL_PEN`, `BTN_TOOL_RUBBER`, `BTN_TOOL_FINGER`,
`BTN_TOUCH`, and `BTN_STYLUS`. While the process runs, the device appears under
its name in desktop input settings and in `libinput list-devices`.

Use `pointer` for osu! and general desktop control. Use `tablet` for drawing
applications that consume tablet events: on Wayland compositors a stylus-class
device does not move the cursor, so pointer mode is the only shape that works
everywhere. See [ADR 0008](decisions/0008-linux-pointer-device.md).

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
target is iPadOS 17.0. The app is iPad-only; it does not run on iPhone.

The first screen is the pairing form. It remembers the transport, address, port,
and pairing code in `UserDefaults`, so pairing is entered once per iPad. Tapping
**Start streaming** replaces the form with the full-screen canvas and a session
HUD. Backgrounding the app ends the session with `BYE`; starting again returns
to the canvas with the saved values.

Choose **Wi-Fi** or **USB** at the top of the form. The pairing field takes the
six-character code from `absolutetouch token`; a pasted 32-character token from
an older configuration still works and is accepted with spaces, `-`, or `:`
separators. The code is case-insensitive and folds `I`/`L` to `1` and `O` to
`0`.

### Local network permission

Wi-Fi streaming triggers the local network prompt. The usage description is in
`ios/App/Info.plist`. If permission was denied, enable it under
**Settings → Privacy & Security → Local Network**.

USB streaming does not use the local network. It still requires the iPad to be
trusted by the desktop and `usbmuxd` to see the device.

## Streaming

The desktop Wi-Fi receiver (M1), the Linux `uinput` backend (M2), the iPad
sender (M3), and the USB transport (M4) are implemented. The Wi-Fi path has been
exercised end to end with a real iPad Air and Apple Pencil (USB-C) on Linux. USB
uses the same sessions and packets over a cable; it is implemented on both sides
but has not been validated on hardware yet.

```sh
# Normal Linux path: receive over Wi-Fi and inject through uinput
absolutetouch run

# Same, but over USB through usbmuxd
absolutetouch run --transport usb

# Two iPads attached: pick one by serial or a fragment of it
absolutetouch devices
absolutetouch run --transport usb --serial 00008110

# Same, with the backend named explicitly
absolutetouch run --backend uinput

# Register a stylus tablet instead of the default pointer device
absolutetouch run --device tablet

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

# Show or reset the pairing code
absolutetouch token
absolutetouch token --reset

# Print a full 32-character token instead of a code
absolutetouch token --reset --hex
```

`auto` selects `uinput` on Linux. `--dry-run` always selects the mock backend
and prints each sample, even when another backend is configured. Replay without
`--dry-run` injects the recorded samples; use `--dry-run` when you only want to
inspect a trace.

### Wi-Fi end-to-end

1. Find the desktop's LAN address (`ip addr` on Linux) and allow inbound UDP
   37689 in its firewall.
2. Start the receiver:

   ```sh
   cd desktop
   cargo run -- run
   ```

   It prints `listening on udp 0.0.0.0:37689`. Use
   `cargo run -- run --dry-run` to watch samples on the terminal instead of
   injecting them.

3. Read the pairing code with `cargo run -- token`, or create one with
   `cargo run -- token --reset`. The reset form rewrites the config file and
   prints a six-character code.
4. Build and run the iPad app, choose **Wi-Fi**, then enter the address, port
   37689, and the code. Tap **Start streaming** and allow local network access
   on first use.
5. The desktop prints `session N accepted from PEER` and the app HUD changes to
   **Streaming**. Touch and drag on the canvas to move the cursor; hover the
   Pencil to send hover samples; lift the Pencil to send an out-of-range
   sample.
6. Tapping **Stop** or backgrounding the app sends `BYE`, and the desktop
   releases input immediately.

The app repeats `HELLO` every 500 ms until the desktop accepts, sends a
heartbeat once per second while idle, and sends samples while the Pencil moves
or is held. If the desktop disappears for five seconds the app starts a new
handshake with the same token, so restarting the desktop does not require
touching the iPad. The most common failure is a token mismatch: both sides must
have the same pairing code. The app accepts either a six-character code or a
full 32-character token, with spaces, `-`, or `:` separators.

### USB end-to-end

USB needs `usbmuxd`, the daemon that owns the cable connection, plus the usual
trust step. On Linux, install it with your distribution's `usbmuxd` package
(for example `sudo apt install usbmuxd`) and check that it is running:

```sh
systemctl status usbmuxd
idevice_id -l          # from libimobiledevice-utils
cargo run -- doctor
```

Then:

1. Connect the iPad by cable and tap **Trust** when the iPad asks. Start
   `cargo run -- devices`; it prints one line per attached device:
   `00008110-… device 3 product 0x12ab USB`. No devices means the cable, trust,
   or daemon is wrong, not the app.
2. In the iPad app, choose **USB** and keep the default TCP port 37690. Enter
   the pairing code. Tap **Start streaming**; the app listens for the desktop.
3. On the desktop, run:

   ```sh
   cargo run -- run --transport usb
   ```

   With two iPads attached, add `--serial` with the serial or a unique fragment
   from `devices`.

4. The desktop opens a connection to port 37690 inside the app through
   `usbmuxd`, prints `connecting to SERIAL over usb (port 37690)`, then
   `session N accepted from usb connection M`. From here the session behaves
   exactly like Wi-Fi: same handshake, heartbeat, samples, watchdog, and
   `input latency` summary.
5. Stopping the app or unplugging the cable releases input and closes the
   stream; the desktop keeps retrying, so restarting the app or reconnecting
   the cable resumes the session without touching the command. Press `Ctrl+C`
   to stop it.

The app and the desktop can be started in either order. While nothing is
listening, `usbmuxd` reports
`the iPad refused the connection on TCP port 37690`; the desktop prints that
once, retries twice a second, and connects as soon as the app starts listening.
The same retry covers a pulled cable, a restarted app, and a restarted
`usbmuxd`, and the counters in the final summary add up across reconnects.

Both ends also recover in the other direction. The USB listener stays open when
the desktop goes away, so stopping the desktop command leaves the app on
"Connecting" and the next `run` is accepted as soon as it connects. Over Wi-Fi
the app notices a restarted desktop through the five-second session timeout and
starts a new handshake. Neither case needs the app's **Stop** button.
Firmware and Mac/Windows setups also work when `usbmuxd` is reachable, but Linux
is the reference platform.

The app binds its USB listener to `127.0.0.1` inside the iPad, because that is
the only address `usbmuxd` connects to. Binding the wildcard address instead
would also expose the port to Wi-Fi and, on iOS 14 and later, park the listener
in the local network permission prompt: the port is then not bound at all and
`usbmuxd` reports it as refused while the app still looks like it is waiting.
Keep the app in the foreground; it holds the screen awake for the duration of a
session, but a locked iPad suspends it and closes the listener.

### Latency

The desktop prints an `input latency` line in the session summary: how much
later each sample was injected than the fastest sample of the session, measured
on the desktop clock after the heartbeat offset is known. It covers the sender
queue, the network, and the receive loop, but not the display pipeline or the
game's own frame time.

The absolute age is not reported because the heartbeat offset is only accurate
to the round-trip asymmetry, which on a buffering link is tens of milliseconds.
The fastest sample of the run is the floor, and every other sample is measured
above it, which is the widest gap between two sample delays in the recorded
trace.

The offset is measured once, at the first heartbeat, and then frozen for the
session. Only a constant offset is needed, and re-measuring it every second made
the reported age wander by the round-trip asymmetry — 64 ms peak to peak on an
iPhone hotspot, which turned a recorded 76 ms stall into a reported 132 ms.

To check a recording, replay it at real speed and compare the two summaries:

```sh
absolutetouch run --record session.atrace
absolutetouch replay session.atrace --speed 1
```

`--speed 1` keeps the heartbeat timing of the original run, so the replayed
`input latency` line has to match the recorded one exactly. Any other speed
suppresses the line, because a scaled replay cannot reproduce clock alignment.
A `worst` that is larger on replay than on the live run means the offset is
moving again.

```text
input latency: 3800 us last, 41900 us worst above floor
```

If `worst` is regularly in the tens of milliseconds while `last` is small, the
link is buffering bursts rather than losing packets. Wi-Fi power saving is the
usual cause on Linux:

```sh
iw dev wlan0 get power_save        # "Power save: on" adds up to a beacon of delay
sudo iw dev wlan0 set power_save off
```

Make it persistent with `wifi.powersave = 2` in
`/etc/NetworkManager/conf.d/wifi-powersave-off.conf` and restart
NetworkManager, or disable power saving for the adapter in the desktop
environment's settings. A phone hotspot is the other common cause: it
advertises a long beacon interval and buffers frames for sleeping clients.
Using the same router as both devices, or switching to USB, avoids it.

The app does not keep a backlog: a burst of samples is coalesced to the newest
position before it is sent, so a short stall costs one frame rather than
leaving the cursor trailing behind the Pencil for the rest of the stroke. The
HUD counts the samples it replaced as `coalesced`.

### Ports

| Transport | Protocol |  Port | Listener           |
| --------- | -------- | ----: | ------------------ |
| Wi-Fi     | UDP      | 37689 | Desktop            |
| USB       | TCP      | 37690 | iPad via `usbmuxd` |

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

| Condition                | Path                                         |
| ------------------------ | -------------------------------------------- |
| `XDG_CONFIG_HOME` is set | `$XDG_CONFIG_HOME/absolutetouch/config.toml` |
| Otherwise, `HOME` is set | `$HOME/.config/absolutetouch/config.toml`    |

`--config PATH` overrides the default. Platform-native locations (macOS
Application Support, Windows `%APPDATA%`) arrive with those backends; until
then use `--config` there.

Example shape:

```toml
transport = "wifi"
bind = "0.0.0.0"
udp_port = 37689
tcp_port = 37690
# A six-character pairing code, or 32 hexadecimal characters.
token = "K7M2QX"
watchdog_ms = 750
session_timeout_ms = 5000
pressure_default = 32768
dry_run = false
backend = "auto"
# "pointer" (default) moves the desktop cursor; "tablet" exposes tool,
# pressure, and tilt events for tablet-aware applications.
device = "pointer"

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

`absolutetouch token --reset` generates a six-character pairing code and writes
it to the config file. Six Crockford base32 characters carry 30 bits, which is
short enough to type on an iPad and long enough that accidental collisions are
unlikely; the desktop expands the code to the 16-byte wire token with a
deterministic FNV-1a expansion. The expansion is not a password hash and adds no
entropy: the code is the secret, and the token travels in clear text.
`absolutetouch token --reset --hex` writes a full 32-character token instead,
and the `token` field accepts either form.

The iPad must present the same token in `HELLO`; a mismatch is rejected and
logged, without a reply. Five wrong tokens are answered per 30-second window and
further guesses in that window are dropped silently and counted as rate
limited, which is what makes a 30-bit code too slow to brute force. A correct
code clears the budget. While a session is active the desktop pins it to the
peer that completed the handshake, so another peer cannot take over.
Regenerating the code invalidates the old pairing.

The token is sent in clear text and is not encryption. Use it on a trusted
network, or use USB. Enter the same code in the iPad app; it is kept in the
app's `UserDefaults`, which is convenient but not secure storage. When the
desktop has no token configured it accepts any token and echoes it back, so
configuring one is recommended even on a home network.

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
conflicting ways. On Linux the virtual device is named **AbsoluteTouch
Pointer** with the default `device = "pointer"`, or **AbsoluteTouch Tablet**
with `device = "tablet"`; select it where the game or desktop asks for a
tablet.

## Troubleshooting

| Symptom                                              | Check                                                                                                                                                                   |
| ---------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| No cursor movement                                   | `doctor`, firewall, correct desktop IP, token, iPad foreground; then check that `run` was not started with `--dry-run` or `--backend mock`, which never create a device |
| App stays on **Connecting**                          | Same network, port 37689, matching code, desktop logs; check the app's code field                                                                                       |
| **Connecting** for a few seconds after a crash       | The desktop still holds the old peer; it clears after the 5000 ms session timeout                                                                                       |
| App says **Disconnected**                            | The transport closed: relaunch the app, check Wi-Fi or the cable, then start again                                                                                      |
| Hover works but contact does not                     | Pencil tip, app foreground, canvas not covered by the HUD                                                                                                               |
| Cursor moves but buttons stick                       | Stop the desktop; the watchdog should release. Report a bug with a trace                                                                                                |
| Pointer trails behind the Pencil, or lags in bursts  | Check `input latency` in the summary, then Wi-Fi power saving (`iw dev wlan0 get power_save`) and the access point; see [Latency](#latency)                             |
| `doctor` says `/dev/uinput` missing                  | Load the module, fix the udev rule, log out and in                                                                                                                      |
| `uinput` is present but not writable                 | Recheck the udev rule and group membership; `newgrp input` or log in again                                                                                              |
| No **AbsoluteTouch Pointer** (or **Tablet**) device  | The session process must be running; check `/proc/bus/input/devices` and the `device` setting                                                                           |
| Game ignores the tablet                              | Select the device explicitly, or check Raw Input/tablet settings                                                                                                        |
| Local network prompt never appears                   | Reinstall or reset the app; check Settings → Privacy → Local Network                                                                                                    |
| `devices` says no devices                            | Cable, iPad trust prompt, `systemctl status usbmuxd`, `idevice_id -l`                                                                                                   |
| `run --transport usb` says the daemon is unreachable | Start `usbmuxd` and check `ABSOLUTETOUCH_USBMUX_SOCKET`; `doctor` prints the path it tried                                                                              |
| USB says the iPad refused the port                   | The app is not listening: choose **USB**, tap **Start streaming**, keep the app in the foreground, then run the desktop command                                         |
| iOS build fails on signing                           | Set the development team; CI builds use `CODE_SIGNING_ALLOWED=NO`                                                                                                       |
| TestFlight upload fails                              | Check the `testflight` environment secrets and API key permissions; run the TestFlight toggle from the Release workflow                                                 |

When reporting a bug, include the OS, backend, transport, and a recorded trace.
Never include tokens or personal network details.
