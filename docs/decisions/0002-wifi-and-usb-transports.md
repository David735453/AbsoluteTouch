# 0002: Wi-Fi and USB transports

- **Status:** Accepted
- **Date:** 2026-10-26

## Context

The iPad has Wi-Fi and a USB-C port. Ethernet is not available on the reference
hardware and is out of scope. Wi-Fi is convenient but lossy and jittery; USB is
more predictable but requires the platform's device-multiplexing daemon.

For USB, either side could listen. `usbmuxd` exposes device services to the
desktop, and tools such as `iproxy` connect to a port that the iOS device
listens on. The iPad acting as the TCP server matches that model and keeps the
desktop from owning a device-side listener.

## Decision

Support two transports:

1. **Wi-Fi UDP.** The desktop listens on UDP 37689. The iPad initiates with
   `HELLO`.
2. **USB through `usbmuxd`.** The iPad listens on TCP 37690. The desktop
   connects through the local daemon.

The packet format, handshake, and session policy are identical on both. There
is no Ethernet transport.

## Consequences

- One session implementation serves both transports.
- Wi-Fi needs firewall rules, a trusted network, and handling for loss,
  duplication, and reordering.
- USB depends on `usbmuxd` being installed and the iPad being trusted. It avoids
  local-network permission and Wi-Fi jitter.
- The iPad needs a TCP listener in addition to its UDP socket, and both are
  active only while the app is in the foreground.
- Discovery is not required for v1. The iPad sends to a configured desktop
  address, and the desktop replies to the source address. Bonjour is a future
  convenience, not a dependency.
