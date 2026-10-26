//! AbsoluteTouch desktop library.
//!
//! The library is split into a protocol layer, a future session/transport
//! layer, coordinate mapping, and OS-specific injectors. M0 implements the
//! protocol and the CLI skeleton; the streaming layers are added in later
//! milestones.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod cli;
pub mod config;
pub mod doctor;
pub mod error;
pub mod inject;
pub mod mapping;
pub mod protocol;
pub mod session;
pub mod stats;
pub mod transport;
