# Architecture decisions

Short records of the decisions that shape AbsoluteTouch. Each record states the
context, the decision, and the consequences.

| ADR | Decision |
| --- | --- |
| [0001](0001-fixed-40-byte-packet.md) | Fixed 40-byte little-endian packet |
| [0002](0002-wifi-and-usb-transports.md) | Wi-Fi UDP and USB through `usbmuxd`; no Ethernet |
| [0003](0003-linux-first-backends.md) | Linux `uinput` is the reference; Windows and macOS are beta |
| [0004](0004-ios-shell-and-core-package.md) | SwiftUI shell, UIKit canvas, UI-free core package |
| [0005](0005-pencil-capabilities.md) | Tilt and hover in scope; pressure is capability-gated |

Decisions are changed by adding a new ADR that supersedes the old one, not by
rewriting history.
