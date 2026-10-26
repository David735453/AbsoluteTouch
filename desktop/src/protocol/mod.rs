//! Wire protocol implementation.
//!
//! The protocol is a fixed 40-byte little-endian packet. See
//! `docs/protocol.md` for the field table and `protocol/vectors.json` for the
//! cross-language golden vectors.

pub mod packet;
pub mod types;

pub use packet::Packet;
pub use types::{MAGIC, PACKET_SIZE, VERSION};
