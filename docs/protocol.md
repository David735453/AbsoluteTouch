# Protocol

- **Version:** 1
- **Packet size:** 40 bytes
- **Byte order:** little-endian
- **Magic:** `0x5441` (`41 54`, ASCII `AT`)

AbsoluteTouch uses one fixed-size packet for every message. The packet itself is
the only framing: over UDP one datagram is one packet, and over USB the TCP
stream is a sequence of back-to-back 40-byte packets with no length prefix. A
reader that receives a partial packet buffers it until the next read completes
the 40 bytes; both implementations do this, and records of either transport can
be replayed through the same code.

The Rust and Swift implementations must agree byte for byte. Shared golden
vectors live in [`protocol/vectors.json`](../protocol/vectors.json) and are
checked by both test suites.

## Packet layout

| Offset | Size | Type  | Field         | Notes                               |
| -----: | ---: | ----- | ------------- | ----------------------------------- |
|      0 |    2 | `u16` | `magic`       | Always `0x5441`                     |
|      2 |    1 | `u8`  | `version`     | Always `1`                          |
|      3 |    1 | `u8`  | `type`        | See packet types                    |
|      4 |    2 | `u16` | `flags`       | Pointer or capability bits          |
|      6 |    2 | `u16` | `seq`         | Wraps at 65,535                     |
|      8 |    4 | `u32` | `session_id`  | Zero until `HELLO_ACK`              |
|     12 |    4 | `u32` | `t_device_us` | Device monotonic time, microseconds |
|     16 |    2 | `u16` | `x`           | Position or payload word            |
|     18 |    2 | `u16` | `y`           | Position or payload word            |
|     20 |    2 | `u16` | `pressure`    | Pressure or payload word            |
|     22 |    2 | `u16` | `altitude`    | Tilt altitude or payload word       |
|     24 |    2 | `u16` | `azimuth`     | Tilt azimuth                        |
|     26 |    2 | `u16` | `twist`       | Twist; reserved on current hardware |
|     28 |    2 | `u16` | `status`      | Reserved telemetry; send `0`        |
|     30 |    2 | `u16` | `reserved`    | Send `0`                            |
|     32 |    8 | bytes | `auth_tag`    | Reserved; all zeros in version 1    |

The header is bytes 0 through 15. Bytes 16 through 31 are a type-specific
payload. Bytes 32 through 39 are the reserved authentication tag.

## Packet types

| Value | Name            | Direction      | Payload               |
| ----: | --------------- | -------------- | --------------------- |
|     1 | `HELLO`         | iPad → desktop | 16-byte session token |
|     2 | `HELLO_ACK`     | desktop → iPad | Echoed token          |
|     3 | `SAMPLE`        | iPad → desktop | Pointer state         |
|     4 | `HEARTBEAT`     | iPad → desktop | Timestamps            |
|     5 | `HEARTBEAT_ACK` | desktop → iPad | Timestamps            |
|     6 | `BYE`           | either         | None                  |

Any other value is a protocol error and must be dropped.

## Flags

Pointer flags are valid in `SAMPLE` packets. Capability flags are valid only in
`HELLO` and `HELLO_ACK` packets. Reserved bits must be zero when sending and
must be ignored when receiving.

| Bit |     Mask | Name           | Valid in             | Meaning                      |
| --: | -------: | -------------- | -------------------- | ---------------------------- |
|   0 | `0x0001` | `CONTACT`      | `SAMPLE`             | Tip is touching the screen   |
|   1 | `0x0002` | `HOVER`        | `SAMPLE`             | In range, not touching       |
|   2 | `0x0004` | `ERASER`       | `SAMPLE`             | Eraser tool selected         |
|   3 | `0x0008` | `BARREL`       | `SAMPLE`             | Barrel button pressed        |
|   4 | `0x0010` | `PREDICTED`    | `SAMPLE`             | Predicted future sample      |
|   5 | `0x0020` | `OUT_OF_RANGE` | `SAMPLE`             | Pointer left the active area |
|   6 | `0x0040` | `FINGER`       | `SAMPLE`             | Sample came from a finger    |
|   8 | `0x0100` | `CAP_PRESSURE` | `HELLO`, `HELLO_ACK` | Pressure is supported        |
|   9 | `0x0200` | `CAP_TILT`     | `HELLO`, `HELLO_ACK` | Tilt is supported            |
|  10 | `0x0400` | `CAP_HOVER`    | `HELLO`, `HELLO_ACK` | Hover is supported           |
|  11 | `0x0800` | `CAP_TWIST`    | `HELLO`, `HELLO_ACK` | Twist is supported           |

Bits 7 and 12 through 15 are reserved.

The reference hardware, an iPad Air (M4) with Apple Pencil (USB-C), advertises
`CAP_TILT | CAP_HOVER` but not `CAP_PRESSURE` and not `CAP_TWIST`. The desktop
uses a configurable default pressure when `CAP_PRESSURE` is absent.

`HELLO_ACK` advertises what the host backend can consume. The Linux `uinput`
backend reports `CAP_PRESSURE | CAP_TILT | CAP_HOVER`; `CAP_TWIST` stays off
until twist is implemented.

## Sample semantics

### Position

`x` and `y` are normalized over the active drawing area:

- `(0, 0)` is the top-left corner.
- `(65535, 65535)` is the bottom-right corner.
- `x` grows to the right and `y` grows downward.

The desktop maps the active area into its configured target rectangle. Aspect
ratio handling is a host-side mapping decision; the wire format carries only
normalized device coordinates.

### Pressure

`pressure` spans `0..=65535`. Zero is a valid pressure reading, so use
`CAP_PRESSURE` to decide whether the value is meaningful. When the capability
is absent, the host substitutes its configured default level for backends that
need one.

### Tilt and twist

- `altitude` is the angle between the stylus and the screen, mapped linearly
  from `0` (parallel to the screen) to `65535` (perpendicular).
- `azimuth` is a full turn mapped to `0..=65535`. Zero points along `+x`
  (right) and values increase toward `+y` (down). A quarter turn is `16384`.
- `twist` uses the same full-turn scale. It is reserved and must be zero on
  hardware without `CAP_TWIST`.

Backends that expose two-axis tilt project the altitude/azimuth direction onto
their own axes. The Linux `uinput` backend maps it to `ABS_TILT_X` and
`ABS_TILT_Y`.

### Hover and range

A hover sample sets `HOVER` and leaves `CONTACT` clear. The position is valid
while the Pencil is in range. When the pointer leaves the active area, the iPad
sends a final sample with `OUT_OF_RANGE`; the host releases the pointer and
clears hover state.

`CONTACT` and `HOVER` must not both be set. `PREDICTED` samples are advisory and
must never be injected as independent contacts.

### Eraser and finger

`ERASER` tells the host to use eraser semantics. `FINGER` marks samples that
came from a finger rather than the Pencil; hosts may inject these as touch
contacts or ignore them according to configuration.

## Handshake packets

### `HELLO`

The iPad initiates the session:

- `session_id` is zero.
- `flags` carries the device capability bits.
- The 16-byte payload is the pairing token, stored as eight little-endian
  `u16` words (`x` is the first word). The token is normally derived from a
  six-character pairing code, described below.
- Pointer flags must be zero.

### `HELLO_ACK`

The desktop accepts the session:

- `session_id` is a non-zero identifier chosen by the desktop.
- `flags` carries the capabilities the host backend can consume.
- The payload echoes the token from `HELLO`.
- The packet is sent to the source address of the `HELLO`.

The iPad must use the assigned `session_id` in every later packet. The desktop
drops packets whose session identifier does not match the active session.

### Heartbeats

Heartbeat packets carry two 32-bit timestamps in the payload, low word first:

| Field pair             | `HEARTBEAT` (iPad → desktop)                               | `HEARTBEAT_ACK` (desktop → iPad)      |
| ---------------------- | ---------------------------------------------------------- | ------------------------------------- |
| `x`, `y`               | Device time when the last `HEARTBEAT_ACK` arrived, or zero | Echoed device time from the heartbeat |
| `pressure`, `altitude` | Host time carried by that `HEARTBEAT_ACK`, or zero         | Host time at send, microseconds       |

The header `t_device_us` carries the sender's current monotonic time. The
payload pair lets each side echo the other's timestamp and estimate round-trip
time and clock offset. Heartbeats are sent about once per second; exact timing
is a session policy, not part of the wire format.

The echoed host time is up to one heartbeat period old, so the raw difference
between it and the arrival of the next `HEARTBEAT` is mostly waiting, not
network time. The desktop therefore subtracts the sender's own wait, measured
with `x`/`y`, before recording a round trip. A trace that shows about one
second of "RTT" is a measurement bug, not a slow link; the true round trip on
a local Wi-Fi link is a few milliseconds.

### `BYE`

Ends the session cleanly. It carries no payload. The iPad sends it when the app
stops streaming; the desktop releases every pressed button and closes the
session. In M1 the desktop does not send `BYE` when it shuts down, but it runs
the same release path.

## Pairing codes

Thirty-two hexadecimal characters are unusable on a touch keyboard, so the
desktop prints a six-character code and both sides expand it into the 16-byte
token carried by `HELLO`. The code alphabet is Crockford base32,
`0123456789ABCDEFGHJKMNPQRSTVWXYZ`: no `I`, `L`, `O`, or `U`. Input folds the
excluded letters to the digits they resemble (`I` and `L` to `1`, `O` to `0`)
and ignores whitespace, `-`, and `:`.

Six characters carry 30 bits. The expansion is a deterministic,
domain-separated FNV-1a-64, not a password hash:

| Step           | Value                                                            |
| -------------- | ---------------------------------------------------------------- |
| `message`      | ASCII `absolutetouch.pairing.v1:` followed by the six characters |
| `token[0..8]`  | little-endian FNV-1a-64 of `message`                             |
| `token[8..16]` | little-endian FNV-1a-64 of `message` followed by `token[0..8]`   |

The code is the secret and the token adds no entropy; the expansion only makes
the secret typeable. Brute-force resistance comes from the session rate limit,
not from the derivation. A fixed 30-bit space would be guessable in seconds
without it. `protocol/vectors.json` contains `pairing.vectors`, and both
implementations read them so the derivation cannot drift.

## Session policy

These rules are implemented by the session layer, not the packet codec.

1. The iPad binds a local port and sends `HELLO` to the configured desktop
   address on UDP port **37689**, repeating about twice per second until it
   receives `HELLO_ACK`.
2. The desktop validates the token when one is configured, accepts the session
   from that source address, and assigns a non-zero `session_id`. It then sends
   `HELLO_ACK`.
3. Samples start after `HELLO_ACK`. `seq` increases once per data packet and
   wraps at 65,535. It is not an absolute ordering key.
4. The iPad sends `BYE` on a clean shutdown. A watchdog releases all input if
   no valid packet arrives for 750 ms (configurable). The session itself ends
   after 5000 ms of silence, so idle heartbeats can keep a session alive
   without holding a contact. Both timeouts are configurable.
5. A session that exceeds the session timeout requires a new `HELLO`;
   `session_id` values are not reused across sessions.
6. When a pairing token is configured, `HELLO` must carry it or the desktop
   drops the packet without replying. Five wrong tokens are answered with a
   reject action per 30-second window; later wrong `HELLO`s in that window are
   dropped silently and counted as rate limited. A correct token clears the
   budget. The desktop pins the session to the peer that completed the
   handshake; another peer cannot take over while the session is active.

USB uses the same handshake and packet stream. The iPad listens on TCP port
**37690**; the desktop connects through the local `usbmuxd` service, which opens
a connection to that port inside the app. The iPad is the server.

On the daemon socket the desktop speaks `usbmuxd`'s framed XML-plist protocol:
a 16-byte little-endian header of length, version (`1`), message, and tag,
followed by an XML plist. `ListDevices` asks which iPads are attached, and
`Connect` carries `DeviceID` and a network-byte-order `PortNumber`. After a
successful `Connect` reply the same socket stops speaking that framing and
carries the raw packet stream.

## Security

Version 1 provides **no confidentiality, integrity, or replay protection**.
The token is sent in clear text. A six-character pairing code carries 30 bits,
which stops accidental cross-session mixing and casual injection on a trusted
local network only because the desktop refuses repeated wrong handshakes; it is
not a defence against an attacker who can watch traffic. Use
`token --reset --hex` with 32 hexadecimal characters when the token must be
unguessable, and enable a firewall. The `auth_tag` is reserved for a future MAC.

Do not expose the UDP port to the public internet. Use Wi-Fi that you trust, or
use USB. Pairing codes, source pins, and any future authentication work belong
in the session layer; they do not change the 40-byte layout.

## Golden vectors

[`vectors.json`](../protocol/vectors.json) contains:

| Name                | Type            | Purpose                            |
| ------------------- | --------------- | ---------------------------------- |
| `sample_contact`    | `SAMPLE`        | Typical contact with tilt          |
| `sample_boundaries` | `SAMPLE`        | Maximum field values and flags     |
| `sample_zero`       | `SAMPLE`        | All-zero payload                   |
| `hello`             | `HELLO`         | Token and capability flags         |
| `hello_ack`         | `HELLO_ACK`     | Echoed token and host capabilities |
| `heartbeat`         | `HEARTBEAT`     | Timestamp payload                  |
| `heartbeat_ack`     | `HEARTBEAT_ACK` | Timestamp payload                  |
| `bye`               | `BYE`           | Clean shutdown                     |

Both implementations decode each vector, compare every field, and re-encode it
byte for byte. Add a vector whenever a new behavior or field value needs
coverage.
