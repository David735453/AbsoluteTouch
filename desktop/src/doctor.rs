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

    let usbmuxd = Path::new("/var/run/usbmuxd");
    println!(
        "usbmuxd: {}",
        if usbmuxd.exists() {
            "socket present"
        } else {
            "socket missing"
        }
    );
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
