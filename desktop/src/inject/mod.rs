//! OS input injection.
//!
//! The session layer talks to [`Injector`] only, so transports and session
//! logic stay testable without `/dev/uinput`, Windows, or macOS. Linux `uinput`
//! is the reference backend (M2); Windows and macOS are M5 and M6.

use thiserror::Error;

use crate::mapping::MappedSample;

pub mod linux;
pub mod macos;
pub mod mock;
pub mod windows;

/// Errors returned by an injection backend.
#[derive(Debug, Error)]
pub enum InjectError {
    /// The requested backend is not available in this build or platform.
    #[error("{0}")]
    Unavailable(String),
    /// The backend rejected or failed an event.
    #[error("injection failed: {0}")]
    Failed(String),
}

/// A backend that turns mapped samples into OS input events.
pub trait Injector: std::fmt::Debug {
    /// Inject one sample.
    ///
    /// Implementations must treat a sample that is not in contact as a move
    /// only, and must not leave a pressed button behind.
    ///
    /// # Errors
    ///
    /// Returns [`InjectError`] when the backend cannot produce the event.
    fn inject(&mut self, sample: &MappedSample) -> Result<(), InjectError>;

    /// Release every button and end the current stroke.
    ///
    /// # Errors
    ///
    /// Returns [`InjectError`] when the backend cannot release input.
    fn release_all(&mut self) -> Result<(), InjectError>;
}

/// Select an injection backend by name.
///
/// `auto` picks the native backend when one exists. Until the platform
/// backends land, `auto` only succeeds with `dry_run`, where the mock backend
/// is a safe stand-in.
///
/// # Errors
///
/// Returns [`InjectError::Unavailable`] for unknown or unimplemented backends.
pub fn backend(name: &str, dry_run: bool) -> Result<Box<dyn Injector>, InjectError> {
    match name {
        "mock" => Ok(Box::new(mock::MockInjector::default())),
        "auto" if dry_run => Ok(Box::new(mock::MockInjector::default())),
        "auto" => Err(InjectError::Unavailable(
            "no injection backend in this build; run with --dry-run or --backend mock \
             (Linux uinput arrives in M2)"
                .to_owned(),
        )),
        "uinput" | "linux" => Err(InjectError::Unavailable(
            "the Linux uinput backend arrives in M2".to_owned(),
        )),
        "windows" => Err(InjectError::Unavailable(
            "the Windows backend arrives in M5".to_owned(),
        )),
        "macos" => Err(InjectError::Unavailable(
            "the macOS backend arrives in M6".to_owned(),
        )),
        other => Err(InjectError::Unavailable(format!(
            "unknown backend {other:?}; expected auto, mock, uinput, windows, or macos"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::{InjectError, backend};

    #[test]
    fn mock_backend_is_always_available() {
        assert!(backend("mock", false).is_ok());
    }

    #[test]
    fn auto_backend_needs_dry_run_until_m2() {
        assert!(matches!(
            backend("auto", false),
            Err(InjectError::Unavailable(_))
        ));
        assert!(backend("auto", true).is_ok());
    }

    #[test]
    fn unknown_backend_is_reported() {
        assert!(matches!(
            backend("nonsense", true),
            Err(InjectError::Unavailable(_))
        ));
    }
}
