//! OS input injection.
//!
//! The session layer talks to [`Injector`] only, so transports and session
//! logic stay testable without `/dev/uinput`, Windows, or macOS. Linux `uinput`
//! is the reference backend (M2); Windows and macOS are M5 and M6.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::mapping::MappedSample;

#[cfg(target_os = "linux")]
pub mod linux;
pub mod macos;
pub mod mock;
pub mod windows;

/// Kind of virtual device a backend registers.
///
/// Wayland compositors do not move the desktop cursor for a stylus-class
/// device: libinput reports it as a tablet tool, and the compositor forwards
/// those events to applications that ask for them instead of emulating a
/// pointer. A pointer-class device moves the cursor on every compositor and on
/// X11, so it is the default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceMode {
    /// Absolute pointer that drives the desktop cursor.
    #[default]
    Pointer,
    /// Stylus tablet with tool, pressure, and tilt events.
    Tablet,
}

impl DeviceMode {
    /// Parse the configuration spelling of a device mode.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim() {
            "pointer" => Some(Self::Pointer),
            "tablet" => Some(Self::Tablet),
            _ => None,
        }
    }

    /// Configuration spelling of this mode.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Pointer => "pointer",
            Self::Tablet => "tablet",
        }
    }
}

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
/// `device` only affects backends that register a kernel device; the mock
/// backend records the samples unchanged.
///
/// # Errors
///
/// Returns [`InjectError::Unavailable`] for unknown, unsupported, or
/// not-yet-implemented backends, and when the native device cannot be opened.
pub fn backend(
    name: &str,
    dry_run: bool,
    device: DeviceMode,
) -> Result<Box<dyn Injector>, InjectError> {
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
        "auto" => native_backend(device),
        "uinput" | "linux" => uinput_backend(device),
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
fn native_backend(device: DeviceMode) -> Result<Box<dyn Injector>, InjectError> {
    linux::UinputInjector::new(device).map(|injector| Box::new(injector) as Box<dyn Injector>)
}

#[cfg(not(target_os = "linux"))]
fn native_backend(_device: DeviceMode) -> Result<Box<dyn Injector>, InjectError> {
    Err(InjectError::Unavailable(
        "no injection backend for this platform in this build; use --dry-run".to_owned(),
    ))
}

#[cfg(target_os = "linux")]
fn uinput_backend(device: DeviceMode) -> Result<Box<dyn Injector>, InjectError> {
    linux::UinputInjector::new(device).map(|injector| Box::new(injector) as Box<dyn Injector>)
}

#[cfg(not(target_os = "linux"))]
fn uinput_backend(_device: DeviceMode) -> Result<Box<dyn Injector>, InjectError> {
    Err(InjectError::Unavailable(
        "the uinput backend is only available on Linux".to_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::{DeviceMode, InjectError, backend};

    #[test]
    fn mock_backend_is_always_available() {
        assert!(backend("mock", false, DeviceMode::Pointer).is_ok());
    }

    #[test]
    fn dry_run_mocks_every_valid_backend() {
        for name in ["auto", "mock", "uinput", "linux", "windows", "macos"] {
            assert!(backend(name, true, DeviceMode::Pointer).is_ok(), "{name}");
        }
    }

    #[test]
    fn unknown_backend_is_reported_even_in_a_dry_run() {
        assert!(matches!(
            backend("nonsense", true, DeviceMode::Pointer),
            Err(InjectError::Unavailable(_))
        ));
        assert!(matches!(
            backend("", true, DeviceMode::Pointer),
            Err(InjectError::Unavailable(_))
        ));
    }

    #[test]
    fn device_mode_names_roundtrip() {
        for mode in [DeviceMode::Pointer, DeviceMode::Tablet] {
            assert_eq!(DeviceMode::from_name(mode.name()), Some(mode));
        }
        assert_eq!(
            DeviceMode::from_name(" pointer "),
            Some(DeviceMode::Pointer)
        );
        assert_eq!(DeviceMode::from_name("pen"), None);
        assert_eq!(DeviceMode::default(), DeviceMode::Pointer);
    }
}
