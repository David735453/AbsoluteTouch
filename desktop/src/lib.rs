//! AbsoluteTouch desktop library.
//!
//! The library is split into a protocol layer, a session/transport layer,
//! coordinate mapping, and OS-specific injectors. M1 implements the Wi-Fi UDP
//! session, trace recording and replay, mapping, and the mock injector; the
//! platform backends are added in M2, M5, and M6.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod cli;
pub mod config;
pub mod doctor;
pub mod error;
pub mod inject;
pub mod mapping;
pub mod pairing;
pub mod protocol;
pub mod run;
pub mod session;
pub mod stats;
pub mod transport;
