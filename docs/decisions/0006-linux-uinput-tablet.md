# 0006: Linux `uinput` absolute tablet

- **Status:** Accepted
- **Date:** 2026-10-26

## Context

M1 could decode samples and record traces, but nothing reached the desktop. M2
adds the reference injection backend so the iPad can act as a real absolute
tablet for osu! and for general pointer use.

The crate forbids `unsafe` code. A backend that opens `/dev/uinput` and issues
`ioctl`s directly would need an exception, and hand-written `ioctl` layouts are
easy to get wrong. The backend also needs to expose pen, eraser, finger,
pressure, tilt, hover, and barrel state, and it must be able to release
everything deterministically.

## Decision

- Use the `evdev` crate (0.13) as a Linux-only dependency. It provides safe
  `VirtualDevice`, `UinputAbsSetup`, and event types for `uinput`.
- Register one virtual device named `AbsoluteTouch Tablet` with bus
  `BUS_VIRTUAL`, vendor `0x1209`, product `0x0001`, and version `1`.
- Expose these axes:

  | Axis | Range | Meaning |
  | --- | --- | --- |
  | `ABS_X`, `ABS_Y` | `0..=65535` | Mapped absolute position |
  | `ABS_PRESSURE` | `0..=65535` | Pressure, zero without contact |
  | `ABS_TILT_X`, `ABS_TILT_Y` | `-90..=90` | Tilt in degrees |

- Expose `BTN_TOOL_PEN`, `BTN_TOOL_RUBBER`, `BTN_TOOL_FINGER`, `BTN_TOUCH`, and
  `BTN_STYLUS`.
- Convert wire altitude/azimuth into two-axis tilt in userspace. Altitude is the
  angle from the screen; azimuth is a full turn. The sample types carry
  `tilt: Option<Tilt>`, which is `None` when the iPad lacks `CAP_TILT` or the
  sample comes from a finger.
- Treat hover as the pen tool in range with `BTN_TOUCH` clear. Do not add
  `ABS_DISTANCE`; contact is carried by `BTN_TOUCH`.
- Emit position, pressure, and tilt on every sample. Emit tool, contact, and
  barrel keys only when they change. `evdev` appends `SYN_REPORT`.
- `release_all` unconditionally clears every tool key, `BTN_TOUCH`,
  `BTN_STYLUS`, pressure, and tilt. It does not move `ABS_X` / `ABS_Y`, so a
  watchdog release does not jump the cursor.
- Validate the backend name before considering `dry_run`. Any valid name plus
  `--dry-run` selects the mock backend. `auto` selects `uinput` on Linux.
- Add `CAP_PRESSURE` to the capabilities advertised in `HELLO_ACK`, because the
  backend can consume pressure even though the reference Pencil cannot produce
  it.

## Consequences

- The user needs write access to `/dev/uinput`. A udev rule or membership in the
  `input` group is required; `doctor` checks the node.
- Linux desktops see a normal tablet through libinput. osu! stable may need the
  tablet device selected explicitly; lazer uses libinput.
- Touch, pressure, and tilt semantics are covered by pure event-sequence tests.
  The kernel device test creates a real device and skips read-back when
  `/dev/input/event*` is not readable.
- Because only `BTN_TOUCH` marks contact, applications that expect
  `ABS_DISTANCE` for hover may report the tool as always in contact; this is a
  deliberate simplification and can be revisited if a target application needs
  distance.
- Windows and macOS keep their separate ADRs; their backends must match the
  `Injector` contract, including the release guarantees.
