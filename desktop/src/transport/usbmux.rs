//! USB transport through the local `usbmuxd` service.
//!
//! Implemented in M4. The iPad is the TCP server; the desktop connects through
//! the local daemon, matching the `iproxy` / WebDriverAgent direction.
