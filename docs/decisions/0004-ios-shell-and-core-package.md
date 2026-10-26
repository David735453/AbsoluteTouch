# 0004: SwiftUI shell, UIKit canvas, core package

- **Status:** Accepted
- **Date:** 2026-10-26

## Context

The app needs a polished shell for status and pairing, plus a drawing surface
that captures high-frequency Pencil data. SwiftUI is productive for the shell,
but its gesture system does not expose coalesced touches, predicted touches, or
the full `UITouch` stylus data that a tablet needs.

The protocol and session logic must be testable on Linux CI, where no iOS
simulator is available.

## Decision

- Put UI-free logic in the Swift package `ios/AbsoluteTouchCore`.
- Build the shell in SwiftUI.
- Implement the drawing surface as a UIKit `UIView` wrapped for SwiftUI (M3).
- Use Swift 6 language mode with strict concurrency.
- Keep the core package free of UIKit and SwiftUI imports.

## Consequences

- Protocol and session tests run on Linux and macOS with `swift test`.
- The app has two UI layers and a small representable bridge, which is worth it
  for input fidelity.
- The canvas owns touch capture on the main actor; a session actor owns the
  transport. No network work happens on the main actor.
- UIKit-specific behavior (hover, coalesced touches, Apple Pencil data) is
  isolated in the app target and cannot leak into the core package.
- The generated Xcode project is not committed; `ios/project.yml` is the source
  of truth.
