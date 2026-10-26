# 0003: Linux-first injection backends

- **Status:** Accepted
- **Date:** 2026-10-26

## Context

osu! players mostly use Windows, but Linux is the easiest platform on which to
build and validate an absolute tablet: `uinput` is documented, scriptable, and
observable with `evtest` and `libinput`. Windows synthetic pen APIs and macOS
event injection are less transparent and need manual validation on real
hardware.

The project also needs a code path that runs in CI and in replay tests without
any OS input device.

## Decision

- Define an `Injector` trait above the OS boundary.
- Provide `MockInjector` for tests and replay.
- Make Linux `uinput` the reference backend (M2): absolute position, pressure,
  tilt, hover, eraser, and buttons.
- Ship Windows synthetic pen plus `SendInput` fallback (M5) and macOS `CGEvent`
  mouse injection (M6) as beta until manual validation passes.

OS-specific code stays in `desktop/src/inject/`.

## Consequences

- Mapping, session, and protocol logic are testable everywhere.
- Linux can be validated automatically and with real hardware; it defines the
  expected behavior.
- Windows and macOS are not called "supported" because they compile. Each needs
  a manual checklist and an honest beta label.
- Windows cannot guarantee full tablet semantics on every game; osu! stable may
  need Raw Input disabled.
- macOS `CGEvent` injection cannot express everything a real tablet can, and it
  requires Accessibility permission.
- A new backend must implement `release_all` correctly; the watchdog depends on
  it.
