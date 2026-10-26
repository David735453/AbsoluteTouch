//! Transport implementations.
//!
//! Wi-Fi UDP is added in M1; USB through the local `usbmuxd` service is added
//! in M4. Trace recording and replay live here because they share the packet
//! framing.

pub mod trace;
pub mod udp;
pub mod usbmux;
