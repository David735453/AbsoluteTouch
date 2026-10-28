# Testing

Every behavior gets a test. Protocol changes get a golden vector. Platform
claims require manual validation on real hardware, not just a successful
compile.

## Test layers

| Layer                  | What it covers                                                                                | Where                                                                             |
| ---------------------- | --------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------- |
| Protocol unit tests    | Length, magic, version, type, token, round-trip                                               | `desktop/src/protocol/`                                                           |
| Golden vectors         | Cross-language byte agreement                                                                 | `protocol/vectors.json`                                                           |
| CLI tests              | Argument parsing and defaults                                                                 | `desktop/src/cli.rs`                                                              |
| Config tests           | Defaults, precedence, validation, example file                                                | `desktop/src/config.rs`                                                           |
| Mapping tests          | Stretch, offset, letterbox, flag passthrough                                                  | `desktop/src/mapping.rs`                                                          |
| Session tests          | Handshake, token, peer pin, sequence window, heartbeat, watchdog, clock offset and sample age | `desktop/src/session.rs`                                                          |
| Pairing code tests     | Normalization, FNV expansion, shared vectors, generated codes                                 | `desktop/src/pairing.rs`, `PairingCodeTests.swift`                                |
| Injector tests (mock)  | Exact event sequences through `MockInjector`                                                  | `desktop/src/inject/mock.rs`                                                      |
| Injector tests (Linux) | Frames, tilt, tool switching, release; real device when permitted                             | `desktop/src/inject/linux.rs`                                                     |
| Trace tests            | Round-trip, offsets, truncation, corrupt packets                                              | `desktop/src/transport/trace.rs`                                                  |
| UDP tests              | Local socket receive, decode errors, replies, shutdown                                        | `desktop/src/transport/udp.rs`                                                    |
| USB framing tests      | One-byte reads, short reads, multi-packet reads, buffered tail                                | `desktop/src/transport/framing.rs`                                                |
| usbmuxd client tests   | Request framing, device parsing, byte-swapped port, result mapping, selection                 | `desktop/src/transport/usbmux.rs` against a fake daemon                           |
| USB CLI tests          | `devices`, `run --transport usb`, refused port, missing daemon                                | `desktop/tests/usb.rs` against a fake daemon                                      |
| End-to-end             | HELLO/SAMPLE/BYE over loopback, record then replay                                            | `desktop/src/run.rs`                                                              |
| Swift core tests       | Same protocol and vectors as Rust                                                             | `ios/AbsoluteTouchCore/Tests/`                                                    |
| Swift session tests    | Handshake, heartbeat RTT, timeout re-handshake, sequence wrap                                 | `ios/AbsoluteTouchCore/Tests/AbsoluteTouchCoreTests/SessionMachineTests.swift`    |
| Swift controller tests | Fake datagram transport, state stream, failure, `BYE`                                         | `ios/AbsoluteTouchCore/Tests/AbsoluteTouchCoreTests/SessionControllerTests.swift` |
| Swift mailbox tests    | Bounded buffer, drop-oldest, one wakeup per burst, coalescing keeps state changes             | `ios/AbsoluteTouchCore/Tests/AbsoluteTouchCoreTests/SampleMailboxTests.swift`     |
| Swift pointer tests    | Coordinate/angle normalization, phase flags, token parsing                                    | `PointerSampleTests.swift`, `PairingTokenTests.swift`                             |
| Build number tests     | Higher of the run number and the newest TestFlight build; fallback when unreadable            | `ios/fastlane/build_number_test.rb`                                               |
| Manual hardware tests  | Real Pencil, real backend, real OS                                                            | Checklist below                                                                   |

## Commands

```sh
# Everything that runs on Linux
make setup        # enable the tracked pre-commit hook
make toolchain    # update stable Rust and its clippy/rustfmt components
make check        # vectors, rustfmt, clippy, swift-format, prettier, ruby -c
make test
make doctor

# Rust only
cd desktop
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check

# Swift core only, on Linux or macOS
swift test --package-path ios/AbsoluteTouchCore
swift format lint --strict --recursive ios/AbsoluteTouchCore ios/App ios/Package.swift

# The app target needs Xcode; CI builds it unsigned for the simulator
xcodegen generate --spec ios/project.yml

# Or build/install the app without Xcode, using xtool
cd ios
xtool dev build --ipa
xtool dev run --usb

# Optional SwiftLint (repository config in .swiftlint.yml)
swiftlint lint --quiet --strict

# Ruby files used by the iOS release lane
ruby -c ios/Gemfile
ruby -c ios/fastlane/Fastfile

# Golden vectors are current
python3 tools/gen_vectors.py --check

# Markdown, using the Prettier pinned in package-lock.json
npm install
make lint-md      # check; make fmt-md rewrites in place
```

A change is not ready when any of these fails locally.

## Local pre-commit hook

`make setup` points `core.hooksPath` at `.githooks/`; `make check`, `make test`,
and `make lint` do the same, so a fresh clone gets the hook the first time it
runs one of the project commands. `make hooks` is the explicit, idempotent
target, and `make hooks-off` restores the default. The hook only looks at staged
files:

| Staged files                                                           | Checks                                                                                       |
| ---------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `desktop/**/*.rs`, `desktop/Cargo.toml`, `desktop/Cargo.lock`          | `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets` |
| `ios/**/*.swift`, `ios/AbsoluteTouchCore/Package.swift`                | `swift format lint --strict`, `swift test`, and `swiftlint` (via `tools/swiftlint.sh`)       |
| `.github/workflows/*.yml`, `.github/dependabot.yml`, `ios/project.yml` | Ruby YAML parse, plus `actionlint` (local binary or the pinned Docker image)                 |
| `ios/App/**/*.plist`, `ios/App/PrivacyInfo.xcprivacy`                  | `plistlib` parse                                                                             |
| `protocol/`, `tools/gen_vectors.py`                                    | `gen_vectors.py --check`                                                                     |
| `ios/Gemfile`, `ios/fastlane/Fastfile`                                 | `ruby -c`                                                                                    |

The hook reports problems and the make target that fixes them; it never rewrites
files. Missing tools (`swift`, `swiftlint`, `ruby`) are skipped with a note, so
a partial toolchain does not block commits. `actionlint` runs from a local
binary or, when Docker is available, the pinned `rhysd/actionlint:1.7.12` image;
it is skipped only when neither exists.

Ruby is looked up on `PATH` first and then in the mise, asdf, and rbenv shim
directories, because those shims are only exported by interactive shell setup
and hooks run non-interactively. `make lint-ruby` resolves it the same way, so a
Ruby installed by a version manager is used rather than silently skipped.

SwiftLint is a Swift binary, and Linux toolchains installed by `swiftly` keep
`libswiftCore.so` outside the default loader path, so a bare `swiftlint` exits
with `error while loading shared libraries` before it lints anything.
`tools/swiftlint.sh` derives `LD_LIBRARY_PATH` and `LINUX_SOURCEKIT_LIB_PATH`
from `swiftc -print-target-info` and execs `swiftlint`, so `make lint-swiftlint`
and the hook work on Linux and macOS. If `swiftlint` is installed but still
cannot start, the hook fails instead of skipping, because a silent skip is how a
`closure_parameter_position` violation reached CI.

Two limits matter:

- Files behind `#if canImport(Network)`, such as `UDPTransport.swift`, do not
  compile on Linux, so only the macOS CI job type-checks them. A green Linux
  hook does not prove the Apple-only sources compile.
- Rust lints change between releases. The hook uses the local stable toolchain,
  which must be as new as CI's; `make toolchain` runs `rustup update stable` and
  installs `rustfmt`/`clippy`, and `rust-toolchain.toml` selects `stable`.

## Toolchains and dependency updates

The repository follows the newest supported releases rather than pinning old
major versions:

| Tool           | Version source                                                                                                      |
| -------------- | ------------------------------------------------------------------------------------------------------------------- |
| Rust           | `rust-toolchain.toml` (`stable`); CI uses `dtolnay/rust-toolchain@stable`                                           |
| Swift          | `swift-tools-version: 6.4`; CI installs 6.4.1 with `SwiftyLab/setup-swift@latest`                                   |
| Xcode          | `xcode-27` runner plus `maxim-lobanov/setup-xcode@v1` with `xcode-version: latest`                                  |
| Ruby           | `ruby/setup-ruby@v1` with `ruby-version: "4.0"`                                                                     |
| GitHub Actions | newest major tags (`checkout@v7`, `cache@v6`, `upload-artifact@v7`, `download-artifact@v8`, `action-gh-release@v3`) |

`.github/dependabot.yml` opens weekly, grouped PRs on Monday for two ecosystems:

- Cargo at `/desktop` (patch and minor in one PR, majors separate).
- GitHub Actions at `/`, one PR per week.

SwiftPM is deliberately absent: the core has no remote dependencies yet, and the
Dependabot Swift image still ships Swift 6.3.1, so it cannot read the 6.4
tools-version manifest and every run failed with `using Swift tools version
6.4.0 but the installed version is 6.3.1`. Re-add the `swift` entry at
`/ios/AbsoluteTouchCore` when the core gains a dependency and the image is at
least 6.4; the commented block in `dependabot.yml` is ready to uncomment.

Dependabot does not update the Rust channel, the Swift container image, the
Xcode selection, or the Ruby version. Change those deliberately and keep the
table and the caches in the workflows in sync. `Cargo.lock` is committed and
Dependabot refreshes it; SwiftPM has no `Package.resolved` because the package
has no remote dependencies.

## Formatting rules

- Indentation: Swift uses tabs displayed two columns wide; Rust uses rustfmt's
  four spaces. `.editorconfig` and `.swift-format` agree with this.
- Rust: `rustfmt.toml`, enforced by `cargo fmt --check`.
- Swift: `.swift-format`, enforced by `swift format lint --strict` (bundled
  with the Swift toolchain). Without `--strict`, swift-format reports findings
  but still exits 0, so CI would not fail.
- SwiftLint: `.swiftlint.yml`, layered on top of swift-format for rules the
  formatter does not cover. Run in CI on macOS, optional locally.
- Rubocop is not used; the Ruby surface is a Gemfile and a Fastfile that only
  need syntax checks.

## Golden vectors

`protocol/vectors.json` is the contract between the Rust desktop and the Swift
iPad core. Both suites decode each vector, compare every field, and re-encode
it byte for byte. A stale vector file or a one-byte disagreement fails both
builds.

Adding or changing a vector:

1. Update `tools/gen_vectors.py`.
2. Run `python3 tools/gen_vectors.py`.
3. Run both test suites.
4. Run `python3 tools/gen_vectors.py --check`.

When the packet layout changes, bump the protocol version and update the
vectors, `docs/protocol.md`, and both implementations in the same change. Never
reorder or repurpose a field silently.

## CI

### `ci.yml`

Runs automatically on every push to `main` and nightly at 04:17 UTC, and can be
started with `workflow_dispatch`. Pull requests do not trigger it.

| Job           | Runner             | Push | Nightly | Manual input  |
| ------------- | ------------------ | ---- | ------- | ------------- |
| `lint`        | `ubuntu-26.04`     | yes  | yes     | `lint`        |
| `linux`       | `ubuntu-26.04`     | yes  | yes     | `linux`       |
| `macos`       | `xcode-27`         | yes  | yes     | `macos`       |
| `linux-arm`   | `ubuntu-26.04-arm` | no   | yes     | `linux_arm`   |
| `windows`     | `windows-latest`   | no   | yes     | `windows`     |
| `windows-arm` | `windows-11-arm`   | no   | yes     | `windows_arm` |

The push run is the fast gate: lint, Linux x86_64, and macOS/iOS. The nightly
adds the ARM64 and Windows beta targets so they do not rot. A manual dispatch
uses the per-platform booleans and defaults to `linux`, `macos`, and `lint`.
GitHub disables scheduled workflows after 60 days of repository inactivity;
re-enable the schedule from the Actions tab if the nightly stops running.

| Job                      | What runs                                                                                                                              |
| ------------------------ | -------------------------------------------------------------------------------------------------------------------------------------- |
| `lint`                   | Golden vectors, `cargo fmt --check`, `swift format lint --strict` (Swift 6.4.1), `ruby -c`, YAML/JSON/plist parse checks, `actionlint` |
| `linux`                  | Vector check, Rust fmt/clippy/tests, doctor, Swift format check and Swift core tests (Swift 6.4.1)                                     |
| `linux-arm`              | Vector check, Rust fmt/clippy/tests                                                                                                    |
| `windows`, `windows-arm` | Rust fmt/clippy/tests (beta)                                                                                                           |
| `macos`                  | Rust tests, Swift core tests, `swift format lint --strict`, SwiftLint, XcodeGen, unsigned simulator build                              |

iOS jobs run on the `xcode-27` runner image and select Xcode with
`maxim-lobanov/setup-xcode@v1` using `xcode-version: latest`, so a new
27.x point release on the image is picked up without editing the workflow.
`latest` may select a release candidate; the "Show toolchain" step prints the
exact `xcodebuild` and `swift` versions. Pushes that touch only docs still run
the same jobs; the matrix is small enough that this is not worth path filters.

```sh
gh workflow run ci.yml
gh workflow run ci.yml -f windows=true -f macos=false
```

### `codeql.yml`

CodeQL uses advanced setup for every language: `actions`, `python`, `ruby`,
`rust`, and `swift`. Advanced and default setup are mutually exclusive — an
upload from an advanced workflow is rejected with "CodeQL analyses from advanced
configurations cannot be processed when the default setup is enabled" — so the
default configuration is disabled while this workflow exists.

Swift forces the choice. The core declares `swift-tools-version: 6.4`, so the
extractor must trace a build made by a matching toolchain; default setup's
autobuild called `swift build` with an older Swift and produced no results.

| Language                    | Runner         | Build                                                                                                 |
| --------------------------- | -------------- | ----------------------------------------------------------------------------------------------------- |
| `actions`, `python`, `ruby` | `ubuntu-26.04` | none                                                                                                  |
| `rust`                      | `ubuntu-26.04` | none (CodeQL v4 has no Rust autobuild)                                                                |
| `swift`                     | `xcode-27`     | manual: `swift build --package-path ios/AbsoluteTouchCore --disable-sandbox` with Xcode's Swift 6.4.2 |

Only the UI-free core is analyzed; the UIKit app shell needs the generated Xcode
project and is covered by the `macos` job in `ci.yml`. The Swift job deliberately
uses Xcode's toolchain instead of the `SwiftyLab/setup-swift` toolchain that the
`ci.yml` jobs install: the open-source macOS build exposes no `macosx` platform,
so SwiftPM cannot choose a target platform and the build fails. `--disable-sandbox`
works around SwiftPM's sandbox helper failing under the CodeQL tracer on the
arm64 runner; the package has no build plugins, so the sandbox does not matter
here. The workflow runs on push to `main`, weekly, and on demand — never on pull
requests — and every job is read-only apart from `security-events: write`.

Reverting to default setup is one API call plus deleting the workflow:

```sh
gh api --method PATCH repos/dStudios-735453/AbsoluteTouch/code-scanning/default-setup \
  -f state=configured -f query_suite=default \
  -f 'languages[]=actions' -f 'languages[]=python' \
  -f 'languages[]=ruby' -f 'languages[]=rust'
```

```sh
gh workflow run codeql.yml
gh api repos/dStudios-735453/AbsoluteTouch/code-scanning/alerts --jq '.[].rule.id'
```

### `release.yml`

Manual and versioned. There is no separate TestFlight workflow: the TestFlight
job is one of the toggles.

| Input                                          | Default  | Effect                                                                  |
| ---------------------------------------------- | -------- | ----------------------------------------------------------------------- |
| `version`                                      | required | Tag and file names, and the iOS `MARKETING_VERSION`                     |
| `testflight`                                   | `false`  | Signed archive, upload to TestFlight, keep the IPA artifact for 14 days |
| `gh_release`                                   | `true`   | Publish a GitHub release with the selected desktop binaries             |
| `draft`                                        | `true`   | Create the GitHub release as a draft                                    |
| `linux`                                        | `true`   | Linux x86_64 tarball                                                    |
| `linux_arm`, `windows`, `windows_arm`, `macos` | `false`  | Beta platform builds                                                    |

Validation rejects a run that selects neither TestFlight nor any platform.
The TestFlight job runs in the `testflight` environment and needs the four
secrets below. The GitHub release job runs with `contents: write`; every other
job is read-only. The signed iOS IPA is deliberately removed before publishing
the GitHub release, because it is a TestFlight artifact, not a desktop
download. If any selected job fails, the release is not published.

```sh
# Desktop release only
gh workflow run release.yml -f version=0.0.1

# TestFlight only, no GitHub release and no desktop binaries
gh workflow run release.yml \
  -f version=0.0.1 -f testflight=true -f gh_release=false -f linux=false

# Both
gh workflow run release.yml -f version=0.0.1 -f testflight=true
```

### First-time setup

The workflow files must be on the default branch before push, schedule, or
`workflow_dispatch` can use them. Actions are enabled by default on a public
repository; `ci.yml` needs no repository-level secret.

The `ci.yml` macOS job and the `release.yml` TestFlight job are the only places
the iOS app target is compiled. Both run on the `xcode-27` image; `setup-xcode`
fails the step when no matching Xcode is available instead of silently building
with an older toolchain. Everything else in `ci.yml` runs on standard
GitHub-hosted runners.

The `testflight` job needs a paid Apple Developer Program membership and
one-time App Store Connect setup. Both are in place here: a `version=0.0.1`
dispatch archived, signed, uploaded, and produced a build that installs from
TestFlight on an iPad, so the steps below are known good and only need
repeating for a new team or bundle identifier.

1. Create an app record with bundle ID `org.dstudios.absolutetouch`.
2. Create a **team** App Store Connect API key with the **Admin** role
   (Users and Access → Integrations → App Store Connect API). The Account
   Holder may need to request API access first. Download the `.p8` file once;
   Apple does not let you download it again. Admin is required because the
   archive creates cloud-managed distribution certificates; an App Manager key
   can upload but may fail with `Cloud signing permission error`.
3. Create the `testflight` environment and store the four secrets in it:

```sh
gh api -X PUT "repos/{owner}/{repo}/environments/testflight"
gh secret set ASC_KEY_ID --env testflight
gh secret set ASC_ISSUER_ID --env testflight
gh secret set APPLE_TEAM_ID --env testflight
base64 < AuthKey_XXXXXXXXXX.p8 | tr -d '\n' \
  | gh secret set ASC_KEY_P8_BASE64 --env testflight
```

`APPLE_TEAM_ID` is the ten-character team identifier. Never commit the `.p8`
file; it is already covered by `.gitignore`. Signing uses automatic signing
with `-allowProvisioningUpdates`, so the API key needs access to the team's
certificates and provisioning profiles. The marketing version is the `version`
input; the build number is the higher of the workflow run number and one past
the newest TestFlight build for that version, so repeated or recreated runs
cannot reuse a number. When the fastlane lane cannot read TestFlight it warns
and uses the run number alone, and a duplicate upload is then rejected by App
Store Connect.

`testflight` on the job is the **environment name**, not an expression context;
there is no `testflight.*` context. GitHub's Actions language server (used by
VS Code and the web editor) validates the name against the environments that
already exist in the repository. When the environment is missing, the editor
reports `Value 'testflight' is not valid` and lists the expression contexts
allowed in that field. The workflow itself would create the environment on
first use, but it is cleaner to create it first with the `gh api -X PUT`
command above (or **Settings → Environments**) and reload the editor. Secrets
are still read with `${{ secrets.NAME }}` after the job declares
`environment:`.

### Caches

The repository has a 10 GB cache budget on the free plan, with
least-recently-used eviction and 7-day expiry for entries that are not
restored. The budget is shared by every branch, but entries are branch-scoped:
caches saved on `main` are available to `main` runs and to branches created
from it, not the other way round. Every cache here is bounded and keyed on
inputs that change rarely.

| Cache                                 | Path                                  | Key inputs                                                   | Typical size            | Refresh               |
| ------------------------------------- | ------------------------------------- | ------------------------------------------------------------ | ----------------------- | --------------------- |
| Rust build (`Swatinem/rust-cache@v2`) | `desktop/target`, Cargo registry      | toolchain, `Cargo.lock`, runner                              | 300–800 MB per platform | managed by the action |
| Homebrew downloads                    | `~/Library/Caches/Homebrew/downloads` | OS, architecture, hash of `ci.yml` and `release.yml`         | 20–60 MB                | edit either workflow  |
| SwiftPM build                         | `ios/AbsoluteTouchCore/.build`        | OS, architecture, `swift-6.4`, `v1`, hash of `Package.swift` | 20–50 MB                | bump `v1` in `ci.yml` |

`actions/cache` only writes a cache when the exact key is missing, so the
SwiftPM entry is a snapshot: later runs restore it, recompile whatever changed,
and keep the old key. Bump the `v1` segment in `ci.yml` when it needs to be
rebuilt from scratch, or delete the entry.

Deliberately not cached:

- Xcode `DerivedData`: hundreds of MB to several GB, stale after any project or
  SDK change, and the app is small enough that a clean build is cheaper than a
  wrong one. `ONLY_ACTIVE_ARCH=YES` and `COMPILER_INDEX_STORE_ENABLE=NO` keep
  the clean simulator build short, and the TestFlight archive is Release-only.
- The Swift toolchain: `SwiftyLab/setup-swift` caches the download itself, so a
  second cache layer would only add a stale copy.
- Bundler gems: `ios/Gemfile` has no lockfile, so the resolved set changes
  without a cache-key change. Fastlane is installed per TestFlight run, which is
  infrequent.

Inspect or prune caches after a few runs:

```sh
gh cache list
gh cache delete <key>
gh cache delete --all
```

If the total approaches 10 GB, the oldest entries are evicted automatically,
but prefer deleting stale platform caches explicitly: most of the budget is the
per-platform Rust target directories, and a platform that is not being tested
does not need its cache kept warm.

## Recording and replay

Manual repetition is unreliable for latency and injection regressions. The
desktop CLI records received packets and replays them through the same session
and mapping code:

```sh
absolutetouch run --record session.atrace
absolutetouch replay session.atrace --dry-run
absolutetouch replay session.atrace --speed 2
```

Replay tests use `MockInjector` and assert the exact injected sequence. The
trace format is internal and versioned (`ATTR`, version 1, 8-byte header,
48-byte records); it is not the wire protocol. `--record` keeps rejected
packets too, so a trace can reproduce misbehaving clients.

## Measured latency baseline

These numbers were taken on the reference iPad Air (M4) with an Apple Pencil
(USB-C), streaming over an iPhone hotspot to a Linux desktop on the same
`uinput` backend. Use them as the Wi-Fi number to beat, not as a target: they
are what the link does today.

| Run                       | Contact samples | Contact time | Contact gaps > 33 ms | Worst excess |
| ------------------------- | --------------: | -----------: | -------------------: | -----------: |
| 8 short strokes, 1 s each |        ~1000 ea |      ~1 s ea |  4 of 8 runs, one ea |     72–79 ms |
| 60 s mixed-speed stroke   |           11266 |       46.8 s |                   46 |      75.8 ms |
| USB, same stroke mix      |            4017 |       60.3 s |                    0 |       3.8 ms |

The USB row is the same iPad Air (M4) and Apple Pencil (USB-C) over `usbmuxd`
on the same Linux desktop and `uinput` backend, recorded with
`absolutetouch run --transport usb --record usb-timing.atrace` (76.9 s,
4309 packets, 0 dropped, `last RTT: 1029 us`). Contact gaps above 33 ms are
counted within a stroke, so the pauses while lifting and repositioning do not
count as stalls. `replay --speed 1` reproduced `input latency: 671 us last,
3836 us worst above floor` exactly, and its p99 arrival gap was 17.2 ms against
a 16.6 ms median — the 60 Hz sample period plus under a millisecond of link
jitter. USB delivers the same stream as Wi-Fi with none of the radio stalls.

The long run is one stall per second of contact (46 in 46.8 s). Its contact
excess percentiles were median 3.7 ms, p90 5.3 ms, p99 62.1 ms, max 75.8 ms,
with 630 of 11266 samples (5.6 %) more than 20 ms above the floor. On a 60 Hz
display a 16.7 ms frame is the budget, so the p99 and max cases are four to five
frames behind — enough to drop an osu! hit.

Three explanations were ruled out by measurement:

- **Sender uptime.** Bad and clean runs interleave across 19.6–127.4 s of
  `t_device_us` with no trend, so the app does not degrade as it runs.
- **Pencil speed.** A deliberately mixed-speed run shows the same stall rate.
  Short strokes looked cleaner only because they are less likely to contain a
  once-per-second stall; path length was near-constant (0.63–0.99 tablet
  diagonals), so the earlier correlation was stroke duration, i.e. exposure.
- **The host.** A loopback sender on the same machine, same protocol, same
  recorder, produced 2872 contact samples over 12 s with a maximum arrival gap
  of 17.4 ms and a worst excess of 873 µs. The receive loop, session, mapping,
  injector, and recorder add nothing measurable.

That leaves the radio path: the iPad's Wi-Fi power saving, the hotspot's
buffering, or both. Power saving is worth checking first, because it changes the
shape of the problem — with it enabled, 33.5 % of contact samples arrived 20–319
ms late, while disabling it left mostly clean delivery with the recurring ~70 ms
stall above. Neither is reachable from `iw` on the hotspot side, which is the
argument for M4 (USB) rather than more Wi-Fi tuning.

When adding a row here, record the transport, access point, iPad, Pencil, and
the `run --record` command that produced it. Validate the summary against
`replay --speed 1` first: the two `input latency` lines have to agree, or the
clock offset is moving and the number means nothing.

## Manual hardware checklist

Run this before calling a platform or milestone supported. Attach the OS,
backend, transport, iPad model, and Pencil model to the result.

### Connection

- [ ] `doctor` reports the backend and permissions correctly.
- [ ] On Linux, `run` registers **AbsoluteTouch Pointer** (default) or
      **AbsoluteTouch Tablet** (`--device tablet`) with the expected axes and
      keys in `/proc/bus/input/devices` and `libinput list-devices`.
- [ ] Moving the Pencil moves the desktop cursor in pointer mode on the session
      type under test (Wayland or X11).
- [ ] `input latency` in the summary stays under one frame (about 17 ms) above
      the floor for a whole stroke, and the `worst` value is recorded with the
      machine, OS, and access point in the result. Wi-Fi does not pass this
      today: see [Measured latency baseline](#measured-latency-baseline).
- [ ] `HELLO` / `HELLO_ACK` completes within one second.
- [ ] A wrong token is rejected and logged.
- [ ] A second iPad with the wrong token or source IP cannot take over.
- [ ] `BYE` ends the session and releases input.

### iPad app (M3)

- [ ] The pairing screen keeps the transport, address, port, and code across
      launches.
- [ ] The first Wi-Fi session shows the local network prompt; a USB session does
      not need it.
- [ ] A six-character pairing code is accepted, with lower case and with `I`,
      `L`, and `O` typed in place of `1` and `0`.
- [ ] The HUD shows **Connecting** and then **Streaming** within one second of
      `HELLO_ACK`.
- [ ] A wrong code stays on **Connecting** and is rejected in the desktop log.
- [ ] Rotating the iPad, or switching to Split View, keeps the active area
      aligned with the canvas bounds.
- [ ] Holding the Pencil still for longer than the watchdog window keeps the
      contact alive.
- [ ] Lifting the Pencil sends an out-of-range sample and clears contact.
- [ ] Stopping and backgrounding send `BYE`; the desktop releases immediately.
- [ ] Restarting the desktop mid-session re-handshakes without the user
      restarting the app, and the desktop accepts samples afterwards.

### USB (M4)

- [ ] `doctor` reports the daemon path and the attached device count, and not
      "socket present" when `usbmuxd` is stopped.
- [ ] `devices` lists the iPad serial, device id, product id, and `USB`.
- [ ] Two attached iPads require `--serial`; a serial fragment selects one and
      an ambiguous fragment is rejected.
- [ ] `run --transport usb` without the app running reports the refused port,
      names the port, and keeps retrying instead of exiting.
- [ ] The app reports a listener that cannot bind (for example a port already in
      use) instead of sitting on "Waiting for USB" forever.
- [ ] The iPad screen stays awake while a session is open, and auto-lock returns
      to normal after stopping or after a failure.
- [ ] With the app streaming and the code matching, the desktop prints
      `connecting to SERIAL over usb` and accepts the session.
- [x] The 40-byte stream survives Pencil bursts and pause/resume; `replay` of a
      recorded USB trace matches the live summary with `--speed 1`.
- [x] `input latency` stays under one frame above the floor and is lower than
      the Wi-Fi baseline on the same machine.
- [ ] Unplugging the cable mid-stroke releases input and the same run reconnects
      when the cable is restored.
- [ ] Restarting the desktop `run` command while the app stays on the canvas
      reconnects in the same app session: the HUD returns to Streaming and the
      new desktop run prints one accepted session.
- [x] A trace recorded over USB replays identically through the mock backend.

### Position and contact

- [ ] Cursor reaches all four corners of the configured target.
- [ ] Aspect ratio and letterboxing match the configuration.
- [ ] Press, drag, and release produce no stuck contact.
- [ ] Lifting the Pencil releases the button immediately.

### Pencil features

- [ ] Hover moves the cursor without pressing.
- [ ] Leaving range sends `OUT_OF_RANGE` and clears hover.
- [ ] Tilt changes both axes in a tablet tester, for contact and for hover.
- [ ] Pressure is present only on hardware that supports it; the reference
      Pencil (USB-C) uses the default level.
- [ ] Eraser and finger samples behave as configured.
- [ ] The display runs at its real rate; do not claim 120/240 Hz for the
      reference iPad Air (M4).

### Failure and safety

- [ ] Wi-Fi loss mid-stroke releases input within the watchdog window.
- [ ] Unplugging USB mid-stroke releases input.
- [ ] Backgrounding the app mid-stroke releases input.
- [ ] Killing the desktop process mid-stroke releases input.
- [ ] Reconnecting starts a new session with a new `session_id`.
- [ ] A flood of malformed packets is dropped without a panic or leak.

### osu!

- [ ] osu! stable works with the selected backend.
- [ ] osu! lazer works with the selected backend.
- [ ] Absolute positioning matches the chosen play area.
- [ ] Raw Input settings are documented for the tested OS.

## Support policy

- **Linux** is the reference platform. It is supported after the M2 manual
  checklist passes on real hardware.
- **The iPad app** passed the M3 Wi-Fi checklist on the reference iPad Air (M4)
  and Pencil (USB-C); it is still beta overall because USB has no hardware run
  yet and rotation, multitasking, and other iPad sizes have not been exercised.
- **USB** is beta until the M4 checklist above passes on real hardware. Unit
  tests against a fake daemon prove the framing, not the cable.
- **Windows** and **macOS** are beta until their manual checklists pass on real
  hardware. A green compile job is not support.
- **TestFlight** builds are beta by definition; they do not imply App Store
  readiness.
- The reference device is an iPad Air (M4) with Apple Pencil (USB-C). Other
  iPads and Pencils may work, but they need their own validation.

## Adding tests

- Put pure logic behind the injector trait so it runs without `/dev/uinput`,
  Windows, or macOS.
- Prefer a recorded trace over a hand-written sequence of hundreds of samples.
- Keep protocol tests deterministic; do not depend on wall-clock time.
- Do not add simulator UI automation unless the workflow explicitly selects a
  simulator and the test adds real value.
- Never weaken a test to make a platform job pass. Fix the behavior or mark the
  platform unsupported.
