# 0001: Fixed 40-byte packet

- **Status:** Accepted
- **Date:** 2026-10-26

## Context

The iPad must send pointer samples to the desktop with low, predictable latency.
The format has to be implemented twice, in Swift and Rust, and tested across
languages. Early samples need position, contact, tilt, hover, and pressure
fields; future Pencils and backends will need more.

The alternatives were a minimal 12-byte packet, a variable-length TLV format, or
a schema-driven encoding such as Protobuf or CBOR.

## Decision

Use a single fixed-size 40-byte little-endian packet for every message:

- 16-byte header: magic, version, type, flags, sequence, session, timestamp.
- 16-byte payload: position, pressure, tilt, twist, status, reserved.
- 8-byte reserved authentication tag.

Magic is `0x5441`, version is `1`, and the layout is frozen for version 1.

## Consequences

- Encoding and decoding are simple, allocation-free, and easy to audit.
- The golden vectors are compact and readable.
- At 60 Hz a packet costs 2.4 KB/s; even a much higher sample rate is
  negligible for Wi-Fi or USB.
- Reserved fields avoid a breaking change for pressure, twist, or future
  telemetry on current hardware.
- There is no self-describing schema; a layout change requires a version bump
  and parallel updates to both implementations and the vectors.
- The reserved auth tag is not authentication yet. Version 1 security relies on
  a trusted network, the pairing token, and an optional source-IP pin.

See [protocol.md](../protocol.md) for the field table.
