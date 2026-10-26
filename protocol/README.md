# Protocol

The AbsoluteTouch wire format is a fixed **40-byte little-endian packet**. The
authoritative field table lives in [`docs/protocol.md`](../docs/protocol.md).

## Files

| File | Purpose |
| --- | --- |
| `vectors.json` | Shared golden vectors read by both test suites |
| `../tools/gen_vectors.py` | Regenerates and checks `vectors.json` |
| `../desktop/src/protocol/` | Rust encoder and decoder |
| `../ios/AbsoluteTouchCore/Sources/AbsoluteTouchCore/` | Swift encoder and decoder |

## Golden vectors

`vectors.json` is the cross-language contract. The Rust and Swift tests decode
every vector, compare all fields, and re-encode it byte for byte.

Regenerate after changing the layout or the vector set:

```sh
python3 tools/gen_vectors.py
python3 tools/gen_vectors.py --check
```

CI runs the `--check` form so a stale vector file fails the build.

## Invariants

- Packet size is exactly 40 bytes.
- Magic is `0x5441`; the wire bytes are `41 54` (`AT`).
- Version is `1`. Any layout change requires a version bump.
- Offsets are fixed. Never reorder or repurpose a field.
- `seq` wraps at 65,535 and `t_device_us` wraps roughly every 71 minutes.
  Neither is used for absolute ordering.
- Capability flags are only valid in `HELLO` and `HELLO_ACK`.
- Every layout change updates the vectors, both implementations, and
  `docs/protocol.md` in the same change.
