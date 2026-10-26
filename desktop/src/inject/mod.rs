//! OS input injection.
//!
//! The injector trait and mock backend arrive in M1. Linux `uinput` is M2;
//! Windows and macOS backends are M5 and M6.

pub mod linux;
pub mod macos;
pub mod mock;
pub mod windows;
