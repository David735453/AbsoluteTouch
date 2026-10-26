# iOS app

The iPad app is a SwiftUI shell that links the UI-free `AbsoluteTouchCore`
package. M0 contains only a status screen; the UIKit drawing surface and the
streaming pipeline arrive in M3.

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

Golden vectors are read from `protocol/vectors.json` in the repository root, so
the Rust and Swift suites always check the same bytes.

## TestFlight

`fastlane/Fastfile` archives the app and optionally uploads it to TestFlight.
The `testflight` GitHub environment supplies `ASC_KEY_ID`, `ASC_ISSUER_ID`,
`ASC_KEY_P8_BASE64`, and `APPLE_TEAM_ID`. Uploads only run from the manual
`TestFlight` workflow.
