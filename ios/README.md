# iOS app

The iPad app is a SwiftUI pairing shell around a UIKit canvas. It links the
UI-free `AbsoluteTouchCore` package, which owns the protocol codec, the session
machine, and the Wi-Fi UDP transport. M3 implements the first end-to-end sender;
it still needs validation on a real iPad and Pencil.

## Structure

| Path                           | Contents                                                       |
| ------------------------------ | -------------------------------------------------------------- |
| `App/ContentView.swift`        | Pairing form: address, port, token, start button               |
| `App/StreamingView.swift`      | Full-screen canvas, session HUD, failure overlay               |
| `App/CanvasView.swift`         | UIKit touch, hover, and coalesced-sample capture               |
| `App/StreamingViewModel.swift` | Observable session owner and statistics polling                |
| `AbsoluteTouchCore/Sources/`   | Protocol, `SessionMachine`, `SessionController`, UDP transport |
| `AbsoluteTouchCore/Tests/`     | Core tests, including the shared golden vectors                |

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

| Setting                                                    | Value | Meaning                                                                                |
| ---------------------------------------------------------- | ----- | -------------------------------------------------------------------------------------- |
| `swift-tools-version` in `AbsoluteTouchCore/Package.swift` | 6.4   | Minimum Swift compiler for the package; CI installs 6.4.1 with `SwiftyLab/setup-swift` |
| `SWIFT_VERSION` in `project.yml`                           | 6.0   | Swift **language mode** (strict concurrency), not the compiler version                 |
| `deploymentTarget` in `project.yml`                        | 17.0  | Minimum iPadOS; the app is built against the iOS 27 SDK from Xcode 27                  |

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

`fastlane/Fastfile` archives the app and uploads it to TestFlight when the
`testflight` toggle is enabled in the `Release` workflow. The `testflight`
GitHub environment supplies `ASC_KEY_ID`, `ASC_ISSUER_ID`,
`ASC_KEY_P8_BASE64`, and `APPLE_TEAM_ID`. The same workflow can publish a
GitHub release of the desktop binaries; the signed IPA stays a TestFlight
artifact and is not attached to that release.

### First TestFlight build

Do these once, in order:

1. **Apple Developer Program** membership is active. Note the ten-character
   team ID on the Membership details page.
2. **Register the App ID** `org.dstudios.absolutetouch` in Certificates,
   Identifiers & Profiles → Identifiers → App IDs. No extra capabilities are
   needed for M3.
3. **Accept the agreements** in App Store Connect → Business → Agreements, Tax,
   and Banking. Uploads fail while the free-apps agreement is inactive.
4. **Create the app record** in App Store Connect → Apps → + → New App:
   platform iOS, a unique App Store name (for example `AbsoluteTouch`), the
   bundle ID above, and a unique internal SKU (for example
   `absolutetouch-ios`). Internal TestFlight does not need screenshots or App
   Store review.
5. **Create a team API key** in Users and Access → Integrations → App Store
   Connect API → Team Keys. Use the **Admin** role: the archive creates
   cloud-managed distribution certificates, and an App Manager key can fail
   with `Cloud signing permission error`. Record the Key ID and Issuer ID and
   download the `.p8` file once; it cannot be downloaded again.
6. **Add the GitHub secrets** to the `testflight` environment:

   ```sh
   gh secret set ASC_KEY_ID --env testflight
   gh secret set ASC_ISSUER_ID --env testflight
   gh secret set APPLE_TEAM_ID --env testflight
   base64 < AuthKey_XXXXXXXXXX.p8 | tr -d '\n' \
     | gh secret set ASC_KEY_P8_BASE64 --env testflight
   ```

7. **Run the workflow** and watch it:

   ```sh
   gh workflow run release.yml \
     -f version=0.0.1 -f testflight=true -f gh_release=false -f linux=false
   gh run watch
   ```

The archive sets `MARKETING_VERSION` from `version` and
`CURRENT_PROJECT_VERSION` from `github.run_number`. After the upload, the build
appears under TestFlight once Apple finishes processing (usually 5–30 minutes).
The app is iPad-only, so use an iPad in the TestFlight app. `Info.plist`
already declares `ITSAppUsesNonExemptEncryption = false`, so no export
compliance prompt is expected.

Common first-run failures: `Cloud signing permission error` means the API key
is not Admin; `No profiles for 'org.dstudios.absolutetouch'` means the App ID,
team ID, or key role is wrong; a duplicate name or SKU is reported when the app
record already exists. A build number that was already uploaded is rejected, so
use a new workflow run rather than re-running an old one.
