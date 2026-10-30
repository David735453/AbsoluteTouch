# 0011: iPad drawing area with an aspect lock

- **Status:** Accepted
- **Date:** 2026-10-28

## Context

The whole iPad was the drawing surface, so reaching opposite corners of the
desktop meant crossing the whole screen. For osu!-style play a smaller surface
is more comfortable and more sensitive, but it has to keep a predictable shape
and must not be reconfigured mid-session.

The wire format carries normalized coordinates and the desktop maps them to its
own target rectangle. The desktop target is already configurable there, and
changing it from the iPad would need a new packet type and a matching desktop
change.

## Decision

- The app owns an _active area_: the part of the iPad canvas that accepts input.
  It is stored as fractions of the canvas, not points, so rotation, Split View,
  and other iPad sizes keep its place and shape.
- No protocol change. The canvas normalizes positions against the active
  rectangle, so a small area still spans the whole desktop target.
- The aspect lock is a setting, on by default at 16:9 to match the desktop
  screen. While it is on, the stored height and center are kept and the width is
  derived from the ratio and the current canvas shape. If the result would not
  fit, both sides scale down by the same factor so the ratio survives.
- A fresh install keeps the full screen stored and resolves it to a full-width
  16:9 strip on a 4:3 iPad. **Reset to full screen** stores the whole screen and
  releases the lock, because a locked full screen cannot exist unless the canvas
  has that ratio.
- The canvas ignores contacts that begin outside the area. A stroke that leaves
  the area sends `OUT_OF_RANGE` and lifts the desktop pointer; coming back in
  starts a new stroke while the Pencil is still down.
- The area is configured on a separate page from the pairing screen, which is
  unreachable during a session. The page has a preview with drag, plus size,
  width, ratio, and position controls so every adjustment is also available to
  VoiceOver.
- The settings live in `@AppStorage` keys under `absoluteTouch.area.*`.

## Consequences

- The desktop cannot tell whether a sample came from a small area; mapping,
  letterboxing, and target configuration stay exactly as they were.
- The area is app-local state. A second iPad keeps its own rectangle, which is
  the useful behavior for a per-device input surface.
- The rectangle is visible while streaming as a dimmed surround, so the user can
  find it without leaving the canvas.
- A locked area is always a subset of the canvas; the editor clamps rather than
  allowing values that would break the ratio.
