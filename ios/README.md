# iOS app

The iPad app is a SwiftUI pairing shell around a UIKit canvas. It links the
UI-free `AbsoluteTouchCore` package, which owns the protocol codec, the session
machine, and the Wi-Fi UDP transport. M3 implements the first end-to-end sender;
it still needs validation on a real iPad and Pencil.

## Structure

| Path | Contents |
| --- | --- |
| `App/ContentView.swift` | Pairing form: address, port, token, start button |
| `App/StreamingView.swift` | Full-screen canvas, session HUD, failure overlay |
| `App/CanvasView.swift` | UIKit touch, hover, and coalesced-sample capture |
| `App/StreamingViewModel.swift` | Observable session owner and statistics polling |
| `AbsoluteTouchCore/Sources/` | Protocol, `SessionMachine`, `SessionController`, UDP transport |
| `AbsoluteTouchCore/Tests/` | Core tests, including the shared golden vectors |

The canvas sends contact samples from coalesced touches plus a display-link
keepalive, hover samples from a Pencil-only `UIHoverGestureRecognizer`, and one
out-of-range sample when the Pencil lifts. The session repeats `HELLO` every
500 ms, sends a heartbeat once per second, and re-handshakes after five seconds
of silence. Stopping or backgrounding the app sends `BYE`.

## Requirements

- macOS with Xcode 27
- [XcodeGen](https://github.com/yonaskolb/XcodeGen)
- An Apple developer team for device builds

## Generate and run

```sh
brew install xcodegen
cd ios
xcodegen generate --spec project.yml
open AbsoluteTouch.xcodeproj
```

The generated `AbsoluteTouch.xcodeproj` is ignored by git. Edit `project.yml`
instead, then regenerate.

Set your development team in the Xcode target's **Signing & Capabilities**
tab. The bundle identifier is `org.dstudios.absolutetouch`.

## Toolchains

Three version numbers are deliberately different:

| Setting | Value | Meaning |
| --- | --- | --- |
| `swift-tools-version` in `AbsoluteTouchCore/Package.swift` | 6.4 | Minimum Swift compiler for the package; CI uses the `swift:6.4.1` container |
| `SWIFT_VERSION` in `project.yml` | 6.0 | Swift **language mode** (strict concurrency), not the compiler version |
| `deploymentTarget` in `project.yml` | 17.0 | Minimum iPadOS; the app is built against the iOS 27 SDK from Xcode 27 |

Xcode 27 supplies the compiler and SDK; `SWIFT_VERSION` only selects the
language rules.

## Tests

The UI-free logic lives in `AbsoluteTouchCore` and runs on Linux and macOS:

```sh
swift test --package-path ios/AbsoluteTouchCore
```

The suite covers the protocol and shared golden vectors, pairing-token parsing,
coordinate and angle normalization, the session state machine (handshake,
heartbeats, timeouts, sequence wrap), and the controller against a fake
datagram transport. The app target itself needs Xcode; CI builds it unsigned
for the simulator, and real behavior is checked with the manual M3 checklist in
[`docs/testing.md`](../docs/testing.md).

## TestFlight

`fastlane/Fastfile` archives the app and optionally uploads it to TestFlight.
The `testflight` GitHub environment supplies `ASC_KEY_ID`, `ASC_ISSUER_ID`,
`ASC_KEY_P8_BASE64`, and `APPLE_TEAM_ID`. Uploads only run from the manual
`TestFlight` workflow.
