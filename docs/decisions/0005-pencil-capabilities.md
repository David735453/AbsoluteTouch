# 0005: Pencil capabilities

- **Status:** Accepted
- **Date:** 2026-10-26

## Context

The reference hardware is an iPad Air (M4) with Apple Pencil (USB-C). That
Pencil reports tilt and hover, but **not pressure**, and it has none of the
Apple Pencil Pro features (squeeze, barrel roll, haptics). Other Pencils and
iPads do report pressure.

Hover matters for a tablet because it moves the cursor before contact, matching
how graphics tablets behave. Dropping it would make the reference device feel
worse than it is.

## Decision

- Tilt and hover are in scope; hover ships with the M3 app.
- Pressure is a capability, not a requirement. The protocol always carries a
  pressure field, and `CAP_PRESSURE` says whether it is meaningful.
- When pressure is unsupported, the desktop substitutes a configurable default
  level for backends that need one.
- Twist is reserved and zero on the reference Pencil.
- Pencil Pro features are out of scope.
- The reference display is 60 Hz. Documentation must not claim 120 or 240 Hz.

## Consequences

- The app and host must not present pressure as available on the reference
  hardware.
- Golden vectors cover pressure values, and the capability bits are tested,
  even though the reference device cannot produce pressure.
- Hover requires extra UIKit handling and a host hover state, including a
  release on `OUT_OF_RANGE`.
- Testing pressure requires different hardware; the default-level path is tested
  on the reference device instead.
