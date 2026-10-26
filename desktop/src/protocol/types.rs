//! Protocol constants, packet kinds, and errors.

use thiserror::Error;

/// Wire packet size in bytes.
pub const PACKET_SIZE: usize = 40;

/// Magic value. The wire bytes are `41 54` (`AT`) in little-endian order.
pub const MAGIC: u16 = 0x5441;

/// Current protocol version.
pub const VERSION: u8 = 1;

/// Packet kinds carried by the protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PacketType {
    /// Session initiation.
    Hello = 1,
    /// Session acceptance.
    HelloAck = 2,
    /// Pointer sample.
    Sample = 3,
    /// Liveness check.
    Heartbeat = 4,
    /// Liveness response.
    HeartbeatAck = 5,
    /// Clean shutdown.
    Bye = 6,
}

impl TryFrom<u8> for PacketType {
    type Error = ProtocolError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Hello),
            2 => Ok(Self::HelloAck),
            3 => Ok(Self::Sample),
            4 => Ok(Self::Heartbeat),
            5 => Ok(Self::HeartbeatAck),
            6 => Ok(Self::Bye),
            other => Err(ProtocolError::UnknownType(other)),
        }
    }
}

/// Flag bits stored in the packet header.
pub mod flags {
    /// The tip is in contact with the screen.
    pub const CONTACT: u16 = 1 << 0;
    /// The Pencil is in range but not touching.
    pub const HOVER: u16 = 1 << 1;
    /// The eraser tool is selected.
    pub const ERASER: u16 = 1 << 2;
    /// Barrel button is pressed.
    pub const BARREL: u16 = 1 << 3;
    /// Predicted sample.
    pub const PREDICTED: u16 = 1 << 4;
    /// The pointer left the active area.
    pub const OUT_OF_RANGE: u16 = 1 << 5;
    /// The sample originated from a finger.
    pub const FINGER: u16 = 1 << 6;

    /// The sender supports pressure.
    pub const CAP_PRESSURE: u16 = 1 << 8;
    /// The sender supports tilt.
    pub const CAP_TILT: u16 = 1 << 9;
    /// The sender supports hover.
    pub const CAP_HOVER: u16 = 1 << 10;
    /// The sender supports twist.
    pub const CAP_TWIST: u16 = 1 << 11;
}

/// Protocol decoding errors.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProtocolError {
    /// The buffer was not exactly [`PACKET_SIZE`] bytes.
    #[error("packet must be exactly {PACKET_SIZE} bytes, got {0}")]
    InvalidLength(usize),
    /// The magic value did not match.
    #[error("bad magic 0x{0:04X}")]
    BadMagic(u16),
    /// The protocol version is not supported.
    #[error("unsupported protocol version {0}")]
    UnsupportedVersion(u8),
    /// The packet type is unknown.
    #[error("unknown packet type {0}")]
    UnknownType(u8),
}

#[cfg(test)]
mod tests {
    use super::{PacketType, ProtocolError};

    #[test]
    fn packet_types_roundtrip() {
        for value in 1..=6 {
            let packet_type = PacketType::try_from(value).expect("known packet type");
            assert_eq!(packet_type as u8, value);
        }
    }

    #[test]
    fn unknown_packet_type_is_rejected() {
        assert_eq!(
            PacketType::try_from(99),
            Err(ProtocolError::UnknownType(99))
        );
    }
}
