//! OS input injection.
//!
//! The session layer talks to [`Injector`] only, so transports and session
//! logic stay testable without `/dev/uinput`, Windows, or macOS. Linux `uinput`
//! is the reference backend (M2); Windows and macOS are M5 and M6.

use thiserror::Error;

use crate::mapping::MappedSample;

#[cfg(target_os = "linux")]
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
/// Names are validated before `dry_run` is considered, so a typo is reported
/// even when nothing would be injected. With `dry_run`, every valid name maps
/// to the mock backend. `auto` selects the native backend: `uinput` on Linux.
///
/// # Errors
///
/// Returns [`InjectError::Unavailable`] for unknown, unsupported, or
/// not-yet-implemented backends, and when the native device cannot be opened.
pub fn backend(name: &str, dry_run: bool) -> Result<Box<dyn Injector>, InjectError> {
    if !matches!(
        name,
        "auto" | "mock" | "uinput" | "linux" | "windows" | "macos"
    ) {
        return Err(InjectError::Unavailable(format!(
            "unknown backend {name:?}; expected auto, mock, uinput, windows, or macos"
        )));
    }

    if dry_run {
        return Ok(Box::new(mock::MockInjector::default()));
    }

    match name {
        "mock" => Ok(Box::new(mock::MockInjector::default())),
        "auto" => native_backend(),
        "uinput" | "linux" => uinput_backend(),
        "windows" => Err(InjectError::Unavailable(
            "the Windows backend arrives in M5; use --dry-run on this platform".to_owned(),
        )),
        "macos" => Err(InjectError::Unavailable(
            "the macOS backend arrives in M6; use --dry-run on this platform".to_owned(),
        )),
        _ => Err(InjectError::Unavailable(format!(
            "unknown backend {name:?}; expected auto, mock, uinput, windows, or macos"
        ))),
    }
}

#[cfg(target_os = "linux")]
fn native_backend() -> Result<Box<dyn Injector>, InjectError> {
    linux::UinputInjector::new().map(|injector| Box::new(injector) as Box<dyn Injector>)
}

#[cfg(not(target_os = "linux"))]
fn native_backend() -> Result<Box<dyn Injector>, InjectError> {
    Err(InjectError::Unavailable(
        "no injection backend for this platform in this build; use --dry-run".to_owned(),
    ))
}

#[cfg(target_os = "linux")]
fn uinput_backend() -> Result<Box<dyn Injector>, InjectError> {
    linux::UinputInjector::new().map(|injector| Box::new(injector) as Box<dyn Injector>)
}

#[cfg(not(target_os = "linux"))]
fn uinput_backend() -> Result<Box<dyn Injector>, InjectError> {
    Err(InjectError::Unavailable(
        "the uinput backend is only available on Linux".to_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::{InjectError, backend};

    #[test]
    fn mock_backend_is_always_available() {
        assert!(backend("mock", false).is_ok());
    }

    #[test]
    fn dry_run_mocks_every_valid_backend() {
        for name in ["auto", "mock", "uinput", "linux", "windows", "macos"] {
            assert!(backend(name, true).is_ok(), "{name}");
        }
    }

    #[test]
    fn unknown_backend_is_reported_even_in_a_dry_run() {
        assert!(matches!(
            backend("nonsense", true),
            Err(InjectError::Unavailable(_))
        ));
        assert!(matches!(
            backend("", true),
            Err(InjectError::Unavailable(_))
        ));
    }
}
