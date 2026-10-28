//! Environment and permission checks.

/// Print a human-readable environment report.
pub fn report() {
    println!("AbsoluteTouch {}", env!("CARGO_PKG_VERSION"));
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    println!("os: {os} {arch}");

    #[cfg(target_os = "linux")]
    report_linux();

    #[cfg(not(target_os = "linux"))]
    println!("backend: not implemented for this platform in this build");
}

#[cfg(target_os = "linux")]
fn report_linux() {
    use std::path::Path;

    let uinput = Path::new("/dev/uinput");
    println!("uinput: {}", describe_device(uinput));
    report_usbmuxd();
}

/// Probe the daemon instead of trusting that the socket file exists.
///
/// A stale socket from a stopped daemon looks identical to a working one on
/// disk, so the check connects and asks for the device list.
#[cfg(target_os = "linux")]
fn report_usbmuxd() {
    use crate::transport::usbmux::Daemon;

    match Daemon::connect() {
        Ok(mut daemon) => match daemon.list_devices() {
            Ok(devices) => println!(
                "usbmuxd: {} ({} device(s) attached)",
                daemon.path().display(),
                devices.len()
            ),
            Err(error) => println!("usbmuxd: connected but unusable: {error}"),
        },
        Err(error) => println!("usbmuxd: {error}"),
    }
}

#[cfg(target_os = "linux")]
fn describe_device(path: &std::path::Path) -> &'static str {
    if !path.exists() {
        return "missing";
    }

    match std::fs::OpenOptions::new().write(true).open(path) {
        Ok(_) => "present and writable",
        Err(_) => "present but not writable",
    }
}
