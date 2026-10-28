#!/usr/bin/env python3
"""Generate the shared protocol golden vectors.

The vectors are the cross-language contract between the Rust desktop and the
Swift iPad core. Run this script after changing the packet layout or the set of
vectors, then run both test suites.

Usage:
    python3 tools/gen_vectors.py
    python3 tools/gen_vectors.py --check
"""

from __future__ import annotations

import argparse
import json
import struct
import sys
from pathlib import Path

MAGIC = 0x5441
VERSION = 1
PACKET_SIZE = 40

TYPE_HELLO = 1
TYPE_HELLO_ACK = 2
TYPE_SAMPLE = 3
TYPE_HEARTBEAT = 4
TYPE_HEARTBEAT_ACK = 5
TYPE_BYE = 6

CONTACT = 1 << 0
HOVER = 1 << 1
ERASER = 1 << 2
BARREL = 1 << 3
PREDICTED = 1 << 4
OUT_OF_RANGE = 1 << 5
FINGER = 1 << 6

CAP_PRESSURE = 1 << 8
CAP_TILT = 1 << 9
CAP_HOVER = 1 << 10
CAP_TWIST = 1 << 11


def token_words(token: bytes) -> dict[str, int]:
    """Split a 16-byte token into the eight little-endian u16 fields."""
    if len(token) != 16:
        raise ValueError("token must be exactly 16 bytes")
    words = struct.unpack("<8H", token)
    return {
        "x": words[0],
        "y": words[1],
        "pressure": words[2],
        "altitude": words[3],
        "azimuth": words[4],
        "twist": words[5],
        "status": words[6],
        "reserved": words[7],
    }


CODE_LENGTH = 6
CODE_ALPHABET = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"
CODE_DOMAIN = b"absolutetouch.pairing.v1:"
FNV_OFFSET_BASIS = 0xCBF29CE484222325
FNV_PRIME = 0x00000100000001B3


def fnv1a64(data: bytes) -> int:
    """FNV-1a over the 64-bit variant, as published by the FNV specification."""
    value = FNV_OFFSET_BASIS
    for byte in data:
        value ^= byte
        value = (value * FNV_PRIME) & 0xFFFFFFFFFFFFFFFF
    return value


def normalize_code(text: str) -> str:
    """Fold a typed pairing code to its canonical upper-case form."""
    folded = []
    for character in text:
        if character.isspace() or character in "-:":
            continue
        upper = character.upper()
        upper = {"I": "1", "L": "1", "O": "0"}.get(upper, upper)
        if upper not in CODE_ALPHABET:
            raise ValueError(f"{character!r} is not a pairing code character")
        folded.append(upper)
    code = "".join(folded)
    if len(code) != CODE_LENGTH:
        raise ValueError(f"pairing code must have {CODE_LENGTH} characters, got {len(code)}")
    return code


def code_token(code: str) -> bytes:
    """Expand a normalized code into the 16-byte wire token."""
    first = CODE_DOMAIN + code.encode("ascii")
    high = fnv1a64(first)
    low = fnv1a64(first + struct.pack("<Q", high))
    return struct.pack("<QQ", high, low)


def pairing() -> dict:
    """Build the pairing-code vectors."""
    inputs = [
        ("K7M2QX", "Canonical code with letters and digits."),
        ("k7m-2qx", "Lower case with a separator, as a user might type it."),
        ("0O1IL2", "Letters Crockford excludes fold to the digits they resemble."),
        ("000000", "Smallest code; leading zeros are significant."),
        ("ZZZZZZ", "Largest code."),
    ]
    vectors = []
    for text, description in inputs:
        code = normalize_code(text)
        vectors.append(
            {
                "input": text,
                "description": description,
                "code": code,
                "token": code_token(code).hex(),
            }
        )
    return {
        "code_length": CODE_LENGTH,
        "alphabet": CODE_ALPHABET,
        "domain": CODE_DOMAIN.decode("ascii"),
        "vectors": vectors,
    }


def packet(
    *,
    packet_type: int,
    flags: int = 0,
    seq: int = 0,
    session_id: int = 0,
    device_time_us: int = 0,
    x: int = 0,
    y: int = 0,
    pressure: int = 0,
    altitude: int = 0,
    azimuth: int = 0,
    twist: int = 0,
    status: int = 0,
    reserved: int = 0,
    auth_tag: bytes = b"\x00" * 8,
) -> bytes:
    """Encode one wire packet."""
    if len(auth_tag) != 8:
        raise ValueError("auth_tag must be exactly 8 bytes")
    return struct.pack(
        "<HBBHHIIHHHHHHHH8s",
        MAGIC,
        VERSION,
        packet_type,
        flags,
        seq,
        session_id,
        device_time_us,
        x,
        y,
        pressure,
        altitude,
        azimuth,
        twist,
        status,
        reserved,
        auth_tag,
    )


def vector(
    name: str,
    description: str,
    payload: bytes,
    auth_tag: bytes = b"\x00" * 8,
    **fields: object,
) -> dict:
    """Build one JSON vector entry."""
    if len(payload) != PACKET_SIZE:
        raise ValueError(f"{name}: expected {PACKET_SIZE} bytes, got {len(payload)}")
    if len(auth_tag) != 8:
        raise ValueError(f"{name}: auth_tag must be exactly 8 bytes")
    return {
        "name": name,
        "description": description,
        "hex": payload.hex(),
        "version": VERSION,
        **fields,
        "auth_tag": auth_tag.hex(),
    }


def build() -> dict:
    """Build the complete vector document."""
    hello_token = bytes(range(16))
    words = token_words(hello_token)
    ack_token = bytes(range(15, -1, -1))
    ack_words = token_words(ack_token)

    packets = [
        vector(
            "sample_contact",
            "Pencil touching inside the active area; tilt at 45 degrees and 90 degrees azimuth.",
            packet(
                packet_type=TYPE_SAMPLE,
                flags=CONTACT,
                seq=1,
                session_id=0x11223344,
                device_time_us=123456,
                x=32768,
                y=16384,
                pressure=0,
                altitude=32768,
                azimuth=16384,
            ),
            type=TYPE_SAMPLE,
            flags=CONTACT,
            seq=1,
            session_id=0x11223344,
            t_device_us=123456,
            x=32768,
            y=16384,
            pressure=0,
            altitude=32768,
            azimuth=16384,
            twist=0,
            status=0,
            reserved=0,
        ),
        vector(
            "sample_boundaries",
            "All sample fields at their boundary values.",
            packet(
                packet_type=TYPE_SAMPLE,
                flags=CONTACT | FINGER,
                seq=0xFFFF,
                session_id=0xDEADBEEF,
                device_time_us=0xFFFFFFFF,
                x=0,
                y=0xFFFF,
                pressure=0xFFFF,
                altitude=0,
                azimuth=0xFFFF,
                twist=0,
                status=0,
                reserved=0,
            ),
            type=TYPE_SAMPLE,
            flags=CONTACT | FINGER,
            seq=0xFFFF,
            session_id=0xDEADBEEF,
            t_device_us=0xFFFFFFFF,
            x=0,
            y=0xFFFF,
            pressure=0xFFFF,
            altitude=0,
            azimuth=0xFFFF,
            twist=0,
            status=0,
            reserved=0,
        ),
        vector(
            "sample_zero",
            "A valid all-zero sample; useful for padding and parser tests.",
            packet(packet_type=TYPE_SAMPLE),
            type=TYPE_SAMPLE,
            flags=0,
            seq=0,
            session_id=0,
            t_device_us=0,
            x=0,
            y=0,
            pressure=0,
            altitude=0,
            azimuth=0,
            twist=0,
            status=0,
            reserved=0,
        ),
        vector(
            "hello",
            "Initiator HELLO with a 16-byte token and tilt/hover capabilities.",
            packet(
                packet_type=TYPE_HELLO,
                flags=CAP_TILT | CAP_HOVER,
                seq=7,
                session_id=0xAABBCCDD,
                device_time_us=1000,
                **words,
            ),
            type=TYPE_HELLO,
            flags=CAP_TILT | CAP_HOVER,
            seq=7,
            session_id=0xAABBCCDD,
            t_device_us=1000,
            token=hello_token.hex(),
        ),
        vector(
            "hello_ack",
            "Session acceptance with the echoed token and tilt/hover capabilities.",
            packet(
                packet_type=TYPE_HELLO_ACK,
                flags=CAP_TILT | CAP_HOVER,
                seq=8,
                session_id=0xAABBCCDD,
                device_time_us=1500,
                **ack_words,
            ),
            type=TYPE_HELLO_ACK,
            flags=CAP_TILT | CAP_HOVER,
            seq=8,
            session_id=0xAABBCCDD,
            t_device_us=1500,
            token=ack_token.hex(),
        ),
        vector(
            "heartbeat",
            "Heartbeat echoing the last host timestamp and the device receive time.",
            packet(
                packet_type=TYPE_HEARTBEAT,
                flags=0,
                seq=9,
                session_id=0x01020304,
                device_time_us=5000,
                x=0x1234,
                y=0x5678,
                pressure=0x9ABC,
                altitude=0xDEF0,
            ),
            type=TYPE_HEARTBEAT,
            flags=0,
            seq=9,
            session_id=0x01020304,
            t_device_us=5000,
            x=0x1234,
            y=0x5678,
            pressure=0x9ABC,
            altitude=0xDEF0,
            twist=0,
            status=0,
            reserved=0,
        ),
        vector(
            "heartbeat_ack",
            "Heartbeat acknowledgement echoing the device receive time and carrying the host timestamp.",
            packet(
                packet_type=TYPE_HEARTBEAT_ACK,
                flags=0,
                seq=9,
                session_id=0x01020304,
                device_time_us=5500,
                x=0x1111,
                y=0x2222,
                pressure=0x3333,
                altitude=0x4444,
            ),
            type=TYPE_HEARTBEAT_ACK,
            flags=0,
            seq=9,
            session_id=0x01020304,
            t_device_us=5500,
            x=0x1111,
            y=0x2222,
            pressure=0x3333,
            altitude=0x4444,
            twist=0,
            status=0,
            reserved=0,
        ),
        vector(
            "bye",
            "Clean session shutdown.",
            packet(
                packet_type=TYPE_BYE,
                flags=0,
                seq=10,
                session_id=0x01020304,
                device_time_us=6000,
            ),
            type=TYPE_BYE,
            flags=0,
            seq=10,
            session_id=0x01020304,
            t_device_us=6000,
            x=0,
            y=0,
            pressure=0,
            altitude=0,
            azimuth=0,
            twist=0,
            status=0,
            reserved=0,
        ),
    ]

    return {
        "version": VERSION,
        "packet_size": PACKET_SIZE,
        "packets": packets,
        "pairing": pairing(),
    }


def render() -> str:
    """Render the vector document as stable JSON."""
    return json.dumps(build(), indent=2) + "\n"


def main() -> int:
    """Write or check the vector file."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail if protocol/vectors.json is out of date",
    )
    args = parser.parse_args()

    root = Path(__file__).resolve().parent.parent
    target = root / "protocol" / "vectors.json"
    expected = render()

    if args.check:
        try:
            actual = target.read_text(encoding="utf-8")
        except FileNotFoundError:
            print(f"missing {target}", file=sys.stderr)
            return 1
        if actual != expected:
            print(f"{target} is out of date; run tools/gen_vectors.py", file=sys.stderr)
            return 1
        print(f"{target} is up to date")
        return 0

    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(expected, encoding="utf-8")
    print(f"wrote {target} ({len(build()['packets'])} vectors)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
