//! End-to-end USB command-line tests against a fake `usbmuxd`.
//!
//! The unit tests in `src/transport/usbmux.rs` cover the framing and error
//! mapping. These tests run the real binary so the `devices` command, the
//! `run --transport usb` wiring, and the `ABSOLUTETOUCH_USBMUX_SOCKET`
//! override are exercised together.
//!
//! USB is Unix-only in this milestone; the daemon socket does not exist on
//! Windows, where `usbmuxd` is reached over TCP instead.

#![cfg(unix)]

use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use plist::{Dictionary, Value};

/// Environment variable the client reads for the daemon path.
const SOCKET_ENV: &str = "ABSOLUTETOUCH_USBMUX_SOCKET";

/// Read one request and return its tag and plist.
fn read_request(stream: &mut impl Read) -> (u32, Dictionary) {
    let mut header = [0_u8; 16];
    stream.read_exact(&mut header).expect("request header");
    let length = u32::from_le_bytes(header[0..4].try_into().expect("length"));
    let tag = u32::from_le_bytes(header[12..16].try_into().expect("tag"));

    let mut payload = vec![0_u8; length as usize - 16];
    stream.read_exact(&mut payload).expect("request payload");
    let plist = Value::from_reader_xml(payload.as_slice()).expect("request plist");
    (
        tag,
        plist.as_dictionary().expect("request dictionary").clone(),
    )
}

/// Write one framed message.
fn write_frame(stream: &mut impl Write, message: u32, tag: u32, payload: &[u8]) {
    let length = u32::try_from(16 + payload.len()).expect("length");
    let mut header = [0_u8; 16];
    header[0..4].copy_from_slice(&length.to_le_bytes());
    header[4..8].copy_from_slice(&1_u32.to_le_bytes());
    header[8..12].copy_from_slice(&message.to_le_bytes());
    header[12..16].copy_from_slice(&tag.to_le_bytes());
    stream.write_all(&header).expect("reply header");
    stream.write_all(payload).expect("reply payload");
}

/// Write a plist reply.
fn write_plist(stream: &mut impl Write, tag: u32, plist: &Value) {
    let mut payload = Vec::new();
    plist.to_writer_xml(&mut payload).expect("encode reply");
    write_frame(stream, 8, tag, &payload);
}

/// Write a four-byte result reply.
fn write_result(stream: &mut impl Write, tag: u32, result: u32) {
    write_frame(stream, 1, tag, &result.to_le_bytes());
}

/// Build a `ListDevices` reply for the given `(device id, serial)` pairs.
fn device_list(devices: &[(u64, &str)]) -> Value {
    let entries: Vec<Value> = devices
        .iter()
        .map(|&(id, serial)| {
            let mut properties = Dictionary::new();
            properties.insert("DeviceID".to_owned(), Value::from(id));
            properties.insert("SerialNumber".to_owned(), Value::from(serial));
            properties.insert("ProductID".to_owned(), Value::from(0x12_AB_u64));
            properties.insert("ConnectionType".to_owned(), Value::from("USB"));

            let mut entry = Dictionary::new();
            entry.insert("MessageType".to_owned(), Value::from("Attached"));
            entry.insert("Properties".to_owned(), Value::Dictionary(properties));
            Value::Dictionary(entry)
        })
        .collect();

    let mut dict = Dictionary::new();
    dict.insert("MessageType".to_owned(), Value::from("Result"));
    dict.insert("Number".to_owned(), Value::from(0_u64));
    dict.insert("DeviceList".to_owned(), Value::Array(entries));
    Value::Dictionary(dict)
}

/// Bind a fake daemon socket in a temporary directory.
fn fake_daemon() -> (tempfile::TempDir, PathBuf, UnixListener) {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = directory.path().join("usbmuxd");
    let listener = UnixListener::bind(&path).expect("bind fake daemon");
    (directory, path, listener)
}

/// The binary under test, pointed at `socket`.
fn binary(socket: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_absolutetouch"));
    command.env(SOCKET_ENV, socket);
    command
}

/// A configuration file that keeps the test off the real user config.
fn config(directory: &Path) -> PathBuf {
    let path = directory.join("config.toml");
    std::fs::write(&path, "transport = \"usb\"\nbackend = \"mock\"\n").expect("write config");
    path
}

/// Run the CLI with the fake daemon and return its output.
fn run(command: &mut Command) -> Output {
    command.output().expect("run binary")
}

#[test]
fn devices_lists_the_attached_ipad() {
    let (_directory, path, listener) = fake_daemon();
    let thread = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let (tag, request) = read_request(&mut stream);
        assert_eq!(
            request.get("MessageType").and_then(Value::as_string),
            Some("ListDevices")
        );
        write_plist(
            &mut stream,
            tag,
            &device_list(&[(3, "00008110-001A2C3D4E5F")]),
        );
    });

    let output = run(binary(&path).arg("devices"));
    thread.join().expect("daemon thread");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("00008110-001A2C3D4E5F"), "{stdout}");
    assert!(stdout.contains("device 3"), "{stdout}");
    assert!(stdout.contains("USB") || stdout.contains("usb"), "{stdout}");
}

#[test]
fn run_without_a_device_says_how_to_list_one() {
    let (_directory, path, listener) = fake_daemon();
    let thread = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let (tag, _) = read_request(&mut stream);
        write_plist(&mut stream, tag, &device_list(&[]));
    });

    let directory = tempfile::tempdir().expect("tempdir");
    let output = run(binary(&path)
        .arg("--config")
        .arg(config(directory.path()))
        .arg("run"));
    thread.join().expect("daemon thread");

    assert!(!output.status.success(), "run should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no USB device matches"), "{stderr}");
    assert!(stderr.contains("absolutetouch devices"), "{stderr}");
}

#[test]
fn run_reports_a_refused_port_with_the_app_hint() {
    let (_directory, path, listener) = fake_daemon();
    let thread = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");

        let (tag, request) = read_request(&mut stream);
        assert_eq!(
            request.get("MessageType").and_then(Value::as_string),
            Some("ListDevices")
        );
        write_plist(
            &mut stream,
            tag,
            &device_list(&[(3, "00008110-001A2C3D4E5F")]),
        );

        let (tag, request) = read_request(&mut stream);
        assert_eq!(
            request.get("MessageType").and_then(Value::as_string),
            Some("Connect")
        );
        assert_eq!(
            request
                .get("PortNumber")
                .and_then(Value::as_unsigned_integer),
            Some(u64::from(37_690_u16.to_be()))
        );
        write_result(&mut stream, tag, 3);
    });

    let directory = tempfile::tempdir().expect("tempdir");
    let output = run(binary(&path)
        .arg("--config")
        .arg(config(directory.path()))
        .arg("run"));
    thread.join().expect("daemon thread");

    assert!(!output.status.success(), "run should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("37690"), "{stderr}");
    assert!(stderr.contains("open AbsoluteTouch"), "{stderr}");
}

#[test]
fn a_missing_daemon_points_at_the_socket_path() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = directory.path().join("missing-usbmuxd");

    let output = run(binary(&path).arg("devices"));

    assert!(!output.status.success(), "devices should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(path.to_str().expect("path")),
        "stderr should name the socket: {stderr}"
    );
}
