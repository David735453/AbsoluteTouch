# 0010: Six-character pairing codes and USB stream framing

- **Status:** Accepted
- **Date:** 2026-10-28

## Context

The wire token is 16 bytes, printed as 32 hexadecimal characters. That is
unusable on an iPad keyboard, and the reference session needed the user to copy
the value by hand. A four-digit code was considered and rejected: 10^4 guesses
is about an hour of retries at one attempt per round trip and can be walked by a
script before the user finishes typing it.

USB also needed a design. `usbmuxd` gives the desktop a byte stream, while the
session layer expects whole 40-byte packets and identifies peers by address.

## Decision

### Pairing codes

- The desktop prints six Crockford base32 characters
  (`0123456789ABCDEFGHJKMNPQRSTVWXYZ`) and expands them to the 16-byte token.
  Input ignores whitespace, `-`, and `:` and folds `I`/`L` to `1` and `O` to `0`.
- Six characters carry 30 bits. That is 1000 times a four-digit PIN for the same
  typing effort and enough that accidental collisions between nearby sessions
  are unlikely.
- The expansion is two domain-separated FNV-1a-64 passes
  (`absolutetouch.pairing.v1:`), not a password hash. It adds no entropy; the
  code is the secret and the token travels in clear text.
- The derivation is part of the cross-language contract. `tools/gen_vectors.py`
  writes `pairing.vectors` into `protocol/vectors.json`, and the Rust and Swift
  suites both read it.
- Because 30 bits is small, the desktop budgets wrong handshakes: five answers
  per 30-second window, then silent drops counted as `rate_limited`. The window
  starts at the first failure and is not extended by later guesses, and a
  correct code clears the budget. This is the entire brute-force defence and is
  documented as such.
- Configuration keeps the field name `token` and accepts either a code or 32
  hexadecimal characters. `token --reset` writes a code; `token --reset --hex`
  writes a full token for users who want the larger secret.

### USB

- Add `Peer` to the session layer: `Network(SocketAddr)` or
  `Connection(u64)`. The session logic, pinning, and reject actions are
  unchanged; only the label differs.
- The USB runner lists devices, selects one (`--serial` matches a serial
  fragment), and asks `usbmuxd` to connect to TCP port 37690 inside the app.
  After a successful `Connect` reply the daemon socket is a raw pipe.
- Read the pipe inline in the session loop with a 20 ms timeout. There is one
  stream, so a reader thread buys nothing, and the timeout keeps Ctrl+C and the
  watchdog responsive.
- Reassemble packets with `FrameDecoder`. The stream has no length prefix, so
  the 40-byte packet remains the only framing and a recorded trace replays
  unchanged over either transport.
- The `usbmuxd` client speaks XML plists over the 16-byte daemon header. An old
  daemon (`RESULT_BADVERSION`) is reported with an update hint rather than
  silently falling back to the binary protocol.
- Non-default daemon paths use `ABSOLUTETOUCH_USBMUX_SOCKET`; tests point a fake
  daemon at a temporary path instead of touching `/var/run/usbmuxd`.

## Consequences

- Pairing is six characters typed once per iPad. The code appears in the config
  file and on the iPad, so it is a convenience secret, not a strong credential.
  A hostile network can still observe the token in clear text and replay it;
  version 1 has no MAC.
- Anyone who can send five wrong `HELLO`s every 30 seconds can keep a legitimate
  user out until they stop. That is a denial of service, not a compromise, and
  USB is unaffected because the desktop initiates it.
- The code space is fixed at 30 bits. A future version can lengthen the code or
  replace the derivation without changing the 40-byte packet, but both sides and
  the vectors must change together.
- USB removes Wi-Fi jitter from the path and needs no local-network permission.
  It depends on `usbmuxd`, a trusted cable pairing, and the app listening before
  the desktop connects. Startup order is documented in setup.
- `doctor` now connects to the daemon instead of checking for a socket file, so
  a stopped `usbmuxd` is reported instead of a false positive from a stale
  socket.
