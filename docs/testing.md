# Testing

Every behavior gets a test. Protocol changes get a golden vector. Platform
claims require manual validation on real hardware, not just a successful
compile.

## Test layers

| Layer | What it covers | Where |
| --- | --- | --- |
| Protocol unit tests | Length, magic, version, type, token, round-trip | `desktop/src/protocol/` |
| Golden vectors | Cross-language byte agreement | `protocol/vectors.json` |
| CLI tests | Argument parsing and defaults | `desktop/src/cli.rs` |
| Config tests | Defaults, precedence, validation, example file | `desktop/src/config.rs` |
| Mapping tests | Stretch, offset, letterbox, flag passthrough | `desktop/src/mapping.rs` |
| Session tests | Handshake, token, peer pin, sequence window, heartbeat, watchdog | `desktop/src/session.rs` |
| Injector tests (mock) | Exact event sequences through `MockInjector` | `desktop/src/inject/mock.rs` |
| Injector tests (Linux) | Frames, tilt, tool switching, release; real device when permitted | `desktop/src/inject/linux.rs` |
| Trace tests | Round-trip, offsets, truncation, corrupt packets | `desktop/src/transport/trace.rs` |
| UDP tests | Local socket receive, decode errors, replies, shutdown | `desktop/src/transport/udp.rs` |
| End-to-end | HELLO/SAMPLE/BYE over loopback, record then replay | `desktop/src/run.rs` |
| Swift core tests | Same protocol and vectors as Rust | `ios/AbsoluteTouchCore/Tests/` |
| Swift session tests | Handshake, heartbeat RTT, timeout re-handshake, sequence wrap | `ios/AbsoluteTouchCore/Tests/AbsoluteTouchCoreTests/SessionMachineTests.swift` |
| Swift controller tests | Fake datagram transport, state stream, failure, `BYE` | `ios/AbsoluteTouchCore/Tests/AbsoluteTouchCoreTests/SessionControllerTests.swift` |
| Swift pointer tests | Coordinate/angle normalization, phase flags, token parsing | `PointerSampleTests.swift`, `PairingTokenTests.swift` |
| Manual hardware tests | Real Pencil, real backend, real OS | Checklist below |

## Commands

```sh
# Everything that runs on Linux
make setup        # enable the tracked pre-commit hook
make toolchain    # update stable Rust and its clippy/rustfmt components
make check        # vectors, rustfmt, clippy, swift-format, ruby -c
make test
make doctor

# Rust only
cd desktop
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check

# Swift core only, on Linux or macOS
swift test --package-path ios/AbsoluteTouchCore
swift format lint --strict --recursive ios/AbsoluteTouchCore ios/App

# The app target needs Xcode; CI builds it unsigned for the simulator
xcodegen generate --spec ios/project.yml

# Optional SwiftLint (repository config in .swiftlint.yml)
swiftlint lint --quiet --strict

# Ruby files used by the iOS release lane
ruby -c ios/Gemfile
ruby -c ios/fastlane/Fastfile

# Golden vectors are current
python3 tools/gen_vectors.py --check
```

A change is not ready when any of these fails locally.

## Local pre-commit hook

`make setup` points `core.hooksPath` at `.githooks/`; `make check`, `make test`,
and `make lint` do the same, so a fresh clone gets the hook the first time it
runs one of the project commands. `make hooks` is the explicit, idempotent
target, and `make hooks-off` restores the default. The hook only looks at staged
files:

| Staged files | Checks |
| --- | --- |
| `desktop/**/*.rs`, `desktop/Cargo.toml`, `desktop/Cargo.lock` | `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets` |
| `ios/**/*.swift`, `ios/AbsoluteTouchCore/Package.swift` | `swift format lint --strict`, `swift test`, and `swiftlint` (via `tools/swiftlint.sh`) |
| `.github/workflows/*.yml`, `.github/dependabot.yml`, `ios/project.yml` | Ruby YAML parse, plus `actionlint` (local binary or the pinned Docker image) |
| `ios/App/**/*.plist`, `ios/App/PrivacyInfo.xcprivacy` | `plistlib` parse |
| `protocol/`, `tools/gen_vectors.py` | `gen_vectors.py --check` |
| `ios/Gemfile`, `ios/fastlane/Fastfile` | `ruby -c` |

The hook reports problems and the make target that fixes them; it never rewrites
files. Missing tools (`swift`, `swiftlint`, `ruby`) are skipped with a note, so
a partial toolchain does not block commits. `actionlint` runs from a local
binary or, when Docker is available, the pinned `rhysd/actionlint:1.7.12` image;
it is skipped only when neither exists.

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

| Tool | Version source |
| --- | --- |
| Rust | `rust-toolchain.toml` (`stable`); CI uses `dtolnay/rust-toolchain@stable` |
| Swift | `swift-tools-version: 6.4`; CI containers use `swift:6.4.1` |
| Xcode | `xcode-27` runner plus `maxim-lobanov/setup-xcode@v1` with `xcode-version: latest` |
| Ruby | `ruby/setup-ruby@v1` with `ruby-version: "4.0"` |
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

| Job | Runner | Push | Nightly | Manual input |
| --- | --- | --- | --- | --- |
| `lint` | `ubuntu-26.04` | yes | yes | `lint` |
| `linux` | `ubuntu-26.04` | yes | yes | `linux` |
| `macos` | `xcode-27` | yes | yes | `macos` |
| `linux-arm` | `ubuntu-26.04-arm` | no | yes | `linux_arm` |
| `windows` | `windows-latest` | no | yes | `windows` |
| `windows-arm` | `windows-11-arm` | no | yes | `windows_arm` |

The push run is the fast gate: lint, Linux x86_64, and macOS/iOS. The nightly
adds the ARM64 and Windows beta targets so they do not rot. A manual dispatch
uses the per-platform booleans and defaults to `linux`, `macos`, and `lint`.
GitHub disables scheduled workflows after 60 days of repository inactivity;
re-enable the schedule from the Actions tab if the nightly stops running.

| Job | What runs |
| --- | --- |
| `lint` | Golden vectors, `cargo fmt --check`, `swift format lint --strict` (in `swift:6.4.1`), `ruby -c`, YAML/JSON/plist parse checks, `actionlint` |
| `linux` | Vector check, Rust fmt/clippy/tests, doctor, Swift format check and Swift core tests in the `swift:6.4.1` container |
| `linux-arm` | Vector check, Rust fmt/clippy/tests |
| `windows`, `windows-arm` | Rust fmt/clippy/tests (beta) |
| `macos` | Rust tests, Swift core tests, `swift format lint --strict`, SwiftLint, XcodeGen, unsigned simulator build |

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

| Language | Runner | Build |
| --- | --- | --- |
| `actions`, `python`, `ruby` | `ubuntu-26.04` | none |
| `rust` | `ubuntu-26.04` | autobuild (`cargo build` on `desktop/`) |
| `swift` | `xcode-27` | manual: Swift 6.4.1 via `SwiftyLab/setup-swift@latest`, then `swift build --package-path ios/AbsoluteTouchCore --disable-sandbox` |

Only the UI-free core is analyzed; the UIKit app shell needs the generated Xcode
project and is covered by the `macos` job in `ci.yml`. `--disable-sandbox` works
around SwiftPM's sandbox helper failing under the CodeQL tracer on the arm64
runner; the package has no build plugins, so the sandbox does not matter here.
The workflow runs on push to `main`, weekly, and on demand — never on pull
requests — and every job is read-only apart from `security-events: write`.

Reverting to default setup is one API call plus deleting the workflow:

```sh
gh api --method PATCH repos/David735453/AbsoluteTouch/code-scanning/default-setup \
  -f state=configured -f query_suite=default \
  -f 'languages[]=actions' -f 'languages[]=python' \
  -f 'languages[]=ruby' -f 'languages[]=rust'
```

```sh
gh workflow run codeql.yml
gh api repos/David735453/AbsoluteTouch/code-scanning/alerts --jq '.[].rule.id'
```

### `release.yml`

Manual and versioned. There is no separate TestFlight workflow: the TestFlight
job is one of the toggles.

| Input | Default | Effect |
| --- | --- | --- |
| `version` | required | Tag and file names, and the iOS `MARKETING_VERSION` |
| `testflight` | `false` | Signed archive, upload to TestFlight, keep the IPA artifact for 14 days |
| `gh_release` | `true` | Publish a GitHub release with the selected desktop binaries |
| `draft` | `true` | Create the GitHub release as a draft |
| `linux` | `true` | Linux x86_64 tarball |
| `linux_arm`, `windows`, `windows_arm`, `macos` | `false` | Beta platform builds |

Validation rejects a run that selects neither TestFlight nor any platform.
The TestFlight job runs in the `testflight` environment and needs the four
secrets below. The GitHub release job runs with `contents: write`; every other
job is read-only. The signed iOS IPA is deliberately removed before publishing
the GitHub release, because it is a TestFlight artifact, not a desktop
download. If any selected job fails, the release is not published.

```sh
# Desktop release only
gh workflow run release.yml -f version=0.1.0

# TestFlight only, no GitHub release and no desktop binaries
gh workflow run release.yml \
  -f version=0.1.0 -f testflight=true -f gh_release=false -f linux=false

# Both
gh workflow run release.yml -f version=0.1.0 -f testflight=true
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
one-time App Store Connect setup:

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
certificates and provisioning profiles. The build number is the workflow run
number and the marketing version is the `version` input.

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

| Cache | Path | Key inputs | Typical size | Refresh |
| --- | --- | --- | --- | --- |
| Rust build (`Swatinem/rust-cache@v2`) | `desktop/target`, Cargo registry | toolchain, `Cargo.lock`, runner | 300–800 MB per platform | managed by the action |
| Homebrew downloads | `~/Library/Caches/Homebrew/downloads` | OS, architecture, hash of `ci.yml` and `release.yml` | 20–60 MB | edit either workflow |
| SwiftPM build | `ios/AbsoluteTouchCore/.build` | OS, architecture, `swift-6.4`, `v1`, hash of `Package.swift` | 20–50 MB | bump `v1` in `ci.yml` |

`actions/cache` only writes a cache when the exact key is missing, so the
SwiftPM entry is a snapshot: later runs restore it, recompile whatever changed,
and keep the old key. Bump the `v1` segment in `ci.yml` when it needs to be
rebuilt from scratch, or delete the entry.

Deliberately not cached:

- Xcode `DerivedData`: hundreds of MB to several GB, stale after any project or
  SDK change, and the app is small enough that a clean build is cheaper than a
  wrong one. `ONLY_ACTIVE_ARCH=YES` and `COMPILER_INDEX_STORE_ENABLE=NO` keep
  the clean simulator build short, and the TestFlight archive is Release-only.
- The `swift:6.4.1` Docker image: a GitHub cache entry is the wrong mechanism,
  and the runner pulls it in well under a minute.
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

## Manual hardware checklist

Run this before calling a platform or milestone supported. Attach the OS,
backend, transport, iPad model, and Pencil model to the result.

### Connection

- [ ] `doctor` reports the backend and permissions correctly.
- [ ] On Linux, `run` registers **AbsoluteTouch Tablet** with the expected axes
      and keys in `/proc/bus/input/devices` and `libinput list-devices`.
- [ ] `HELLO` / `HELLO_ACK` completes within one second.
- [ ] A wrong token is rejected and logged.
- [ ] A second iPad with the wrong token or source IP cannot take over.
- [ ] `BYE` ends the session and releases input.

### iPad app (M3)

- [ ] The pairing screen keeps the address, port, and token across launches.
- [ ] The first session shows the local network prompt.
- [ ] The HUD shows **Connecting** and then **Streaming** within one second of
      `HELLO_ACK`.
- [ ] A wrong token stays on **Connecting** and is rejected in the desktop log.
- [ ] Rotating the iPad, or switching to Split View, keeps the active area
      aligned with the canvas bounds.
- [ ] Holding the Pencil still for longer than the watchdog window keeps the
      contact alive.
- [ ] Lifting the Pencil sends an out-of-range sample and clears contact.
- [ ] Stopping and backgrounding send `BYE`; the desktop releases immediately.
- [ ] Restarting the desktop mid-session re-handshakes without the user
      restarting the app, and the desktop accepts samples afterwards.

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
- **The iPad app** is beta until the M3 manual checklist passes on a real iPad
  and Pencil. A green core test run or a simulator build does not make it
  supported.
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
