# 0008: Pointer device is the Linux default

- **Status:** Accepted
- **Date:** 2026-10-27

## Context

[ADR 0006](0006-linux-uinput-tablet.md) registered one stylus-class `uinput`
device: `BTN_TOOL_PEN`, `BTN_TOOL_RUBBER`, `BTN_TOOL_FINGER`, `BTN_TOUCH`, and
`BTN_STYLUS`, plus absolute position, pressure, and tilt. The kernel-level test
proved that events reached the device node, so M2 was declared complete.

The first real-hardware session (M3, KDE Plasma 6 on Wayland) showed packets
arriving and `injected 2407 samples, 0 inject errors` with no cursor movement.
Probe devices built with the same axes isolated the cause:

| Keys                                      | Cursor on Plasma Wayland               |
| ----------------------------------------- | -------------------------------------- |
| `BTN_TOOL_PEN`, `BTN_TOUCH`, `BTN_STYLUS` | does not move                          |
| `BTN_TOOL_PEN` plus `BTN_LEFT`            | does not move                          |
| `BTN_LEFT` plus `BTN_TOUCH`               | moves                                  |
| `BTN_LEFT` plus `BTN_STYLUS`              | does not move                          |
| `BTN_LEFT` only                           | moves, to the exact requested position |

`udev` tags the probe `ID_INPUT=1` and `ID_INPUT_TABLET=1`, and the compositor
opens the node, so the device is not being ignored: libinput classifies it as a
tablet tool, and the compositor forwards tablet events to applications that
bind them instead of emulating the pointer. Any stylus key (`BTN_TOOL_PEN`,
`BTN_STYLUS`) selects that path. The same trace replayed through a
pointer-class device moved the real cursor.

## Decision

- Add a `device` setting with two values: `pointer` (default) and `tablet`.
  It is configurable per run through `device` in the TOML file,
  `ABSOLUTETOUCH_DEVICE`, or `--device pointer|tablet`.
- `pointer` registers `AbsoluteTouch Pointer`: `BTN_LEFT` and `BTN_RIGHT` with
  the same absolute axes. Contact becomes `BTN_LEFT`, the barrel becomes
  `BTN_RIGHT`, and no stylus key is declared. Hover moves the pointer without
  pressing.
- `tablet` keeps the device from ADR 0006 unchanged, now named
  `AbsoluteTouch Tablet`, with product ID `0x0002`.
- Keep pressure and tilt axes on both devices. A pointer-class device reports
  them for completeness; only tablet-aware applications consume them, and the
  wire format does not change.
- Default to `pointer`, because a device that does not move the cursor fails the
  primary use case (osu!, general desktop use) on the default session type of
  every current Linux desktop.

## Consequences

- osu! stable and lazer, browsers, and window managers work out of the box.
  Drawing applications that want pressure and tilt need `device = "tablet"`,
  which works on X11 and delivers tablet events on Wayland even though the
  cursor does not follow.
- The named device differs by mode, so documentation and checklists now say
  **AbsoluteTouch Pointer** or **AbsoluteTouch Tablet** depending on `device`.
- A pointer device has no eraser, no tool switching, and no hover distinction
  beyond "button up". The iPad still sends that information, and tablet mode
  still exposes it.
- Radio firmware and firmware quirks are unaffected: the change is confined to
  `desktop/src/inject/linux.rs`, the configuration schema, and the CLI.
- The kernel device test now builds both shapes and verifies the name and axes
  of each.
