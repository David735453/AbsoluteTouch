//! Coordinate mapping types.
//!
//! Mapping from normalized device coordinates to the target desktop rectangle
//! is implemented in M1. The types live here so transports and injectors can
//! share them without depending on OS-specific code.

/// A normalized sample received from the iPad.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabletSample {
    /// X position in `0..=65535`.
    pub x: u16,
    /// Y position in `0..=65535`.
    pub y: u16,
    /// Pressure in `0..=65535`.
    pub pressure: u16,
    /// Whether the tip is in contact.
    pub contact: bool,
}

/// A sample mapped into the target desktop rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MappedSample {
    /// X position in `0..=65535` within the target rectangle.
    pub x: u16,
    /// Y position in `0..=65535` within the target rectangle.
    pub y: u16,
    /// Pressure in `0..=65535`.
    pub pressure: u16,
    /// Whether the tip is in contact.
    pub contact: bool,
}
