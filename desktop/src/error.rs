//! Crate-level error type.

use thiserror::Error;

/// Errors returned by the command-line layer.
#[derive(Debug, Error)]
pub enum Error {
    /// A feature is not implemented in the current milestone.
    #[error("not implemented yet: {feature}")]
    NotImplemented {
        /// Human-readable feature name.
        feature: &'static str,
    },
    /// An I/O operation failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
