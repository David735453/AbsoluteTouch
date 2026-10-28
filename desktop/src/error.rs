//! Crate-level error type.

use thiserror::Error;

use crate::config::ConfigError;
use crate::inject::InjectError;
use crate::transport::trace::TraceError;
use crate::transport::usbmux::UsbMuxError;

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
    /// Configuration loading or validation failed.
    #[error(transparent)]
    Config(#[from] ConfigError),
    /// Trace recording or replay failed.
    #[error(transparent)]
    Trace(#[from] TraceError),
    /// USB transport through `usbmuxd` failed.
    #[error(transparent)]
    UsbMux(#[from] UsbMuxError),
    /// An injection backend failed or is unavailable.
    #[error(transparent)]
    Inject(#[from] InjectError),
    /// The command line asked for something impossible.
    #[error("invalid usage: {0}")]
    Usage(String),
    /// A signal handler could not be installed.
    #[error("signal handler: {0}")]
    Signal(String),
}
