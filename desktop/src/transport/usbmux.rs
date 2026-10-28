//! Client for the local `usbmuxd` daemon.
//!
//! `usbmuxd` is the service that owns the USB connection to an iPad. It exposes
//! a Unix socket (`/var/run/usbmuxd` on Linux and macOS) and multiplexes
//! connections to TCP ports inside apps on the device. The desktop connects to
//! the socket, asks for the device list, then asks the daemon to open a
//! connection to the port the iPad app listens on.
//!
//! The wire format is a 16-byte little-endian header followed by an XML plist:
//!
//! | Offset | Size | Field   |
//! | ------ | ---- | ------- |
//! | 0      | 4    | length  |
//! | 4      | 4    | version |
//! | 8      | 4    | message |
//! | 12     | 4    | tag     |
//!
//! Requests are `MESSAGE_PLIST` (8) with `MessageType` set to `ListDevices` or
//! `Connect`. Replies are either `MESSAGE_RESULT` (1) with a four-byte result
//! code, or `MESSAGE_PLIST` (8) whose plist may carry a `DeviceList`. After a
//! successful `Connect` the socket stops speaking this framing and becomes a
//! raw pipe to the iPad: everything after the reply is app traffic, which the
//! runner frames itself (see [`crate::transport::framing`]).
//!
//! The client speaks the XML plist protocol only. Daemons older than 1.0 reply
//! `RESULT_BADVERSION` and are reported with an update hint rather than
//! silently falling back to the binary protocol.

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use plist::{Dictionary, Value};
use thiserror::Error;

/// Environment variable that overrides the daemon socket path.
///
/// This exists for tests and for non-standard installs; the default is the
/// documented location used by `usbmuxd` itself.
pub const SOCKET_ENV: &str = "ABSOLUTETOUCH_USBMUX_SOCKET";

/// Default daemon socket on Linux and macOS.
pub const DEFAULT_SOCKET: &str = "/var/run/usbmuxd";

/// Protocol version this client speaks.
const PROTOCOL_VERSION: u32 = 1;

/// Bytes in every daemon header.
const HEADER_SIZE: usize = 16;

/// A framed reply carrying a four-byte result code.
const MESSAGE_RESULT: u32 = 1;

/// A framed reply or request carrying an XML plist.
const MESSAGE_PLIST: u32 = 8;

/// Result code for success.
const RESULT_OK: u32 = 0;

/// Result code for a device the daemon does not know.
const RESULT_BADDEV: u32 = 2;

/// Result code for a device that is not listening on the requested port.
const RESULT_CONNREFUSED: u32 = 3;

/// Result code for a daemon that predates XML plists.
const RESULT_BADVERSION: u32 = 6;

/// Largest reply accepted from the daemon.
const MAX_PAYLOAD: usize = 1 << 20;

/// How long a request may take before it is treated as failed.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// A connection to a daemon.
#[cfg(unix)]
pub type Stream = std::os::unix::net::UnixStream;

/// A connection to a daemon.
#[cfg(not(unix))]
pub type Stream = std::net::TcpStream;

/// One USB device the daemon knows about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    /// Daemon-assigned device identifier, used when connecting.
    pub id: u32,
    /// Hardware serial number, which survives reconnects and reboots.
    pub serial: String,
    /// USB product identifier.
    pub product_id: u32,
    /// `USB` or `Network`.
    pub connection_type: String,
}

impl Device {
    /// Whether the serial contains `needle`, ignoring case.
    ///
    /// Full serials are long, so a leading fragment such as `00008110` is
    /// enough to select a device when only one is attached.
    #[must_use]
    pub fn matches(&self, needle: &str) -> bool {
        self.serial
            .to_ascii_lowercase()
            .contains(&needle.to_ascii_lowercase())
    }
}

/// Why talking to `usbmuxd` failed.
#[derive(Debug, Error)]
pub enum UsbMuxError {
    /// The daemon socket could not be reached.
    #[error("cannot reach usbmuxd at {}: {source}", path.display())]
    Unavailable {
        /// Socket path that was tried.
        path: PathBuf,
        /// Connect failure.
        source: io::Error,
    },
    /// The connection failed while a request was in flight.
    #[error("usbmuxd I/O failed: {0}")]
    Io(#[from] io::Error),
    /// The daemon sent something this client does not understand.
    #[error("usbmuxd protocol error: {0}")]
    Protocol(String),
    /// The daemon returned an unexpected result code.
    #[error("usbmuxd returned result {0}; run `absolutetouch devices`")]
    Result(u32),
    /// No attached device matched the selection.
    #[error("no USB device matches {0:?}; run `absolutetouch devices`")]
    NoDevice(String),
    /// More than one device matched the selection.
    #[error("several USB devices match: {0}; pass --serial with more characters")]
    Ambiguous(String),
    /// The iPad is not listening on the requested port.
    #[error(
        "the iPad refused the connection on TCP port {port}; \
         open AbsoluteTouch on the iPad and keep it in the foreground"
    )]
    Refused {
        /// Port the iPad was asked for.
        port: u16,
    },
}

/// Socket path the daemon is expected to use.
#[must_use]
pub fn socket_path() -> PathBuf {
    path_from_override(std::env::var_os(SOCKET_ENV))
}

/// Resolve the override, falling back to the documented default.
fn path_from_override(value: Option<std::ffi::OsString>) -> PathBuf {
    value.map_or_else(|| PathBuf::from(DEFAULT_SOCKET), PathBuf::from)
}

/// A connected daemon, ready for one or more requests.
#[derive(Debug)]
pub struct Daemon {
    stream: Stream,
    path: PathBuf,
    tag: u32,
}

impl Daemon {
    /// Connect to the daemon at [`socket_path`].
    ///
    /// # Errors
    ///
    /// Returns [`UsbMuxError::Unavailable`] when the socket cannot be reached,
    /// which is the usual symptom of `usbmuxd` not running.
    pub fn connect() -> Result<Self, UsbMuxError> {
        Self::at(socket_path())
    }

    /// Connect to a daemon listening on `path`.
    ///
    /// # Errors
    ///
    /// Returns [`UsbMuxError::Unavailable`] when the socket cannot be reached,
    /// or [`UsbMuxError::Io`] when timeouts cannot be configured.
    pub fn at(path: impl Into<PathBuf>) -> Result<Self, UsbMuxError> {
        let path = path.into();
        let stream = connect_socket(&path).map_err(|source| UsbMuxError::Unavailable {
            path: path.clone(),
            source,
        })?;
        stream.set_read_timeout(Some(REQUEST_TIMEOUT))?;
        stream.set_write_timeout(Some(REQUEST_TIMEOUT))?;

        Ok(Self {
            stream,
            path,
            tag: 0,
        })
    }

    /// Socket path this daemon connection uses.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Ask the daemon which devices are attached.
    ///
    /// # Errors
    ///
    /// Returns [`UsbMuxError`] when the request or the reply is malformed, or
    /// when the daemon reports a failure.
    pub fn list_devices(&mut self) -> Result<Vec<Device>, UsbMuxError> {
        let reply = self.request("ListDevices", &[])?;
        check(reply.result)?;

        let Some(plist) = reply.plist else {
            return Err(UsbMuxError::Protocol(
                "ListDevices reply carried no plist".to_owned(),
            ));
        };
        let Some(list) = dict_item(&plist, "DeviceList").and_then(Value::as_array) else {
            // A daemon with no devices still answers, just without the key.
            return Ok(Vec::new());
        };

        list.iter().map(device_from_plist).collect()
    }

    /// Ask the daemon to open a connection to `port` inside `device_id`.
    ///
    /// On success the returned stream is a raw pipe to the iPad. This consumes
    /// the daemon connection because the socket stops speaking frames. The
    /// stream keeps the handshake timeouts, so a caller that blocks on it must
    /// set its own read timeout (the USB runner uses its tick timeout).
    ///
    /// # Errors
    ///
    /// Returns [`UsbMuxError::Refused`] when the iPad app is not listening,
    /// [`UsbMuxError::NoDevice`] when the daemon has lost the device, or a
    /// protocol error for a malformed reply.
    pub fn connect_device(mut self, device_id: u32, port: u16) -> Result<Stream, UsbMuxError> {
        // `usbmuxd` expects the port in network byte order inside the plist,
        // exactly as libusbmuxd's `htons(port)` call would produce it.
        let port_number = u64::from(port.to_be());
        let reply = self.request(
            "Connect",
            &[
                ("DeviceID", Value::from(device_id)),
                ("PortNumber", Value::from(port_number)),
            ],
        )?;

        if reply.result == RESULT_CONNREFUSED {
            return Err(UsbMuxError::Refused { port });
        }
        check(reply.result)?;

        // The daemon is done framing; the iPad app's bytes follow directly.
        Ok(self.stream)
    }

    /// Send one plist request and read its reply.
    fn request(&mut self, message: &str, extra: &[(&str, Value)]) -> Result<Reply, UsbMuxError> {
        let tag = self.next_tag();
        let payload = request_payload(message, extra)?;
        write_frame(&mut self.stream, MESSAGE_PLIST, tag, &payload)?;
        read_reply(&mut self.stream, tag)
    }

    /// Next request tag; tags are arbitrary and only need to match the reply.
    const fn next_tag(&mut self) -> u32 {
        self.tag = self.tag.wrapping_add(1);
        if self.tag == 0 {
            self.tag = 1;
        }
        self.tag
    }
}

/// A reply from the daemon: a result code and, for plist replies, the plist.
#[derive(Debug)]
struct Reply {
    result: u32,
    plist: Option<Value>,
}

/// Build the XML plist payload for a request.
fn request_payload(message: &str, extra: &[(&str, Value)]) -> Result<Vec<u8>, UsbMuxError> {
    let mut dict = Dictionary::new();
    dict.insert(
        "ClientVersionString".to_owned(),
        Value::from(format!("absolutetouch {}", env!("CARGO_PKG_VERSION"))),
    );
    dict.insert("MessageType".to_owned(), Value::from(message));
    dict.insert("ProgName".to_owned(), Value::from("absolutetouch"));
    dict.insert("kLibUSBMuxVersion".to_owned(), Value::from(3_u64));
    for (key, value) in extra {
        dict.insert((*key).to_owned(), value.clone());
    }

    let mut payload = Vec::new();
    Value::Dictionary(dict)
        .to_writer_xml(&mut payload)
        .map_err(|error| UsbMuxError::Protocol(format!("cannot encode request: {error}")))?;
    Ok(payload)
}

/// Write one framed message.
fn write_frame(stream: &mut impl Write, message: u32, tag: u32, payload: &[u8]) -> io::Result<()> {
    let length = u32::try_from(HEADER_SIZE + payload.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "request is too large"))?;

    let mut header = [0_u8; HEADER_SIZE];
    header[0..4].copy_from_slice(&length.to_le_bytes());
    header[4..8].copy_from_slice(&PROTOCOL_VERSION.to_le_bytes());
    header[8..12].copy_from_slice(&message.to_le_bytes());
    header[12..16].copy_from_slice(&tag.to_le_bytes());

    stream.write_all(&header)?;
    stream.write_all(payload)?;
    stream.flush()
}

/// Read one framed reply and verify its tag.
fn read_reply(stream: &mut impl Read, expected_tag: u32) -> Result<Reply, UsbMuxError> {
    let mut header = [0_u8; HEADER_SIZE];
    stream.read_exact(&mut header).map_err(|error| {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            UsbMuxError::Protocol("usbmuxd closed the connection".to_owned())
        } else {
            UsbMuxError::Io(error)
        }
    })?;

    let length = u32::from_le_bytes(header[0..4].try_into().expect("four bytes"));
    let message = u32::from_le_bytes(header[8..12].try_into().expect("four bytes"));
    let tag = u32::from_le_bytes(header[12..16].try_into().expect("four bytes"));

    let header_size = u32::try_from(HEADER_SIZE).expect("header fits in u32");
    if length < header_size || length as usize > HEADER_SIZE + MAX_PAYLOAD {
        return Err(UsbMuxError::Protocol(format!(
            "implausible reply length {length}"
        )));
    }
    if tag != expected_tag {
        return Err(UsbMuxError::Protocol(format!(
            "reply tag {tag} does not match request tag {expected_tag}"
        )));
    }

    let mut payload = vec![0_u8; length as usize - HEADER_SIZE];
    stream.read_exact(&mut payload)?;

    match message {
        MESSAGE_RESULT => {
            let bytes: [u8; 4] = payload.as_slice().try_into().map_err(|_| {
                UsbMuxError::Protocol(format!(
                    "result reply carried {} bytes instead of 4",
                    payload.len()
                ))
            })?;
            Ok(Reply {
                result: u32::from_le_bytes(bytes),
                plist: None,
            })
        }
        MESSAGE_PLIST => {
            let plist = Value::from_reader_xml(payload.as_slice()).map_err(|error| {
                UsbMuxError::Protocol(format!("invalid XML plist reply: {error}"))
            })?;
            let result = plist_result(&plist)?;
            Ok(Reply {
                result,
                plist: Some(plist),
            })
        }
        other => Err(UsbMuxError::Protocol(format!(
            "unexpected reply message type {other}"
        ))),
    }
}

/// Read one plist dictionary entry.
fn dict_item<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value.as_dictionary().and_then(|dict| dict.get(key))
}

/// Extract the result code from a plist, defaulting to success.
fn plist_result(plist: &Value) -> Result<u32, UsbMuxError> {
    let Some(message) = dict_item(plist, "MessageType").and_then(Value::as_string) else {
        return Ok(RESULT_OK);
    };
    if message != "Result" {
        return Ok(RESULT_OK);
    }

    let number = dict_item(plist, "Number")
        .and_then(Value::as_unsigned_integer)
        .ok_or_else(|| UsbMuxError::Protocol("Result plist has no Number".to_owned()))?;
    u32::try_from(number)
        .map_err(|_| UsbMuxError::Protocol(format!("result {number} does not fit in u32")))
}

/// Turn one `DeviceList` entry into a [`Device`].
fn device_from_plist(entry: &Value) -> Result<Device, UsbMuxError> {
    // The properties normally live under `Properties`; accept a flat entry too,
    // which is what some daemon versions produce.
    let entry_dict = entry
        .as_dictionary()
        .ok_or_else(|| UsbMuxError::Protocol("device entry is not a dictionary".to_owned()))?;
    let properties = entry_dict
        .get("Properties")
        .and_then(Value::as_dictionary)
        .unwrap_or(entry_dict);

    let id = unsigned(properties, "DeviceID")?;
    let id = u32::try_from(id)
        .map_err(|_| UsbMuxError::Protocol(format!("DeviceID {id} does not fit in u32")))?;
    let serial = properties
        .get("SerialNumber")
        .and_then(Value::as_string)
        .unwrap_or_default()
        .to_owned();
    let product_id = properties
        .get("ProductID")
        .and_then(Value::as_unsigned_integer)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(0);
    let connection_type = properties
        .get("ConnectionType")
        .and_then(Value::as_string)
        .unwrap_or("USB")
        .to_owned();

    Ok(Device {
        id,
        serial,
        product_id,
        connection_type,
    })
}

/// Read a required unsigned integer from a dictionary.
fn unsigned(dictionary: &Dictionary, key: &str) -> Result<u64, UsbMuxError> {
    dictionary
        .get(key)
        .and_then(Value::as_unsigned_integer)
        .ok_or_else(|| UsbMuxError::Protocol(format!("device entry has no {key}")))
}

/// Map a non-zero daemon result code to an error.
fn check(result: u32) -> Result<(), UsbMuxError> {
    match result {
        RESULT_OK => Ok(()),
        RESULT_BADDEV => Err(UsbMuxError::NoDevice(
            "the daemon no longer sees the device".to_owned(),
        )),
        RESULT_BADVERSION => Err(UsbMuxError::Protocol(
            "usbmuxd is too old for XML plists; update libusbmuxd".to_owned(),
        )),
        other => Err(UsbMuxError::Result(other)),
    }
}

/// Pick the one device a selector names, or the only device attached.
///
/// # Errors
///
/// Returns [`UsbMuxError::NoDevice`] when nothing matches and
/// [`UsbMuxError::Ambiguous`] when a selector matches more than one device or
/// several devices are attached and none was named.
pub fn select<'a>(
    devices: &'a [Device],
    selector: Option<&str>,
) -> Result<&'a Device, UsbMuxError> {
    match selector {
        Some(needle) => {
            let matches: Vec<&Device> = devices.iter().filter(|d| d.matches(needle)).collect();
            match matches.as_slice() {
                [device] => Ok(*device),
                [] => Err(UsbMuxError::NoDevice(needle.to_owned())),
                many => Err(UsbMuxError::Ambiguous(join_serials(many.iter().copied()))),
            }
        }
        None => match devices {
            [device] => Ok(device),
            [] => Err(UsbMuxError::NoDevice("any".to_owned())),
            many => Err(UsbMuxError::Ambiguous(join_serials(many))),
        },
    }
}

/// Join serials for an error message.
fn join_serials<'a>(devices: impl IntoIterator<Item = &'a Device>) -> String {
    devices
        .into_iter()
        .map(|device| device.serial.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Connect to the daemon socket.
#[cfg(unix)]
fn connect_socket(path: &Path) -> io::Result<Stream> {
    Stream::connect(path)
}

/// Connect to the daemon socket.
#[cfg(not(unix))]
fn connect_socket(_path: &Path) -> io::Result<Stream> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "the usbmuxd client is implemented for Unix only; this platform \
         needs the TCP transport to 127.0.0.1:27015",
    ))
}

#[cfg(all(test, unix))]
mod tests {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixListener;

    use plist::{Dictionary, Value};

    use super::{Daemon, Device, MESSAGE_PLIST, MESSAGE_RESULT, UsbMuxError, select};
    use crate::protocol::packet::Packet;
    use crate::protocol::types::PacketType;

    /// Read one framed request and return its tag and plist.
    fn read_request(stream: &mut impl Read) -> (u32, Dictionary) {
        let mut header = [0_u8; 16];
        stream.read_exact(&mut header).expect("request header");
        let length = u32::from_le_bytes(header[0..4].try_into().expect("length"));
        let message = u32::from_le_bytes(header[8..12].try_into().expect("message"));
        let tag = u32::from_le_bytes(header[12..16].try_into().expect("tag"));
        assert_eq!(message, MESSAGE_PLIST);
        assert_eq!(header[4..8], 1_u32.to_le_bytes(), "protocol version");

        let mut payload = vec![0_u8; length as usize - 16];
        stream.read_exact(&mut payload).expect("request payload");
        let plist = Value::from_reader_xml(payload.as_slice()).expect("request plist");
        (
            tag,
            plist.as_dictionary().expect("request dictionary").clone(),
        )
    }

    /// Write a plist reply.
    fn write_plist(stream: &mut impl Write, tag: u32, plist: &Value) {
        let mut payload = Vec::new();
        plist.to_writer_xml(&mut payload).expect("encode reply");
        write_header(stream, MESSAGE_PLIST, tag, &payload);
    }

    /// Write a result reply.
    fn write_result(stream: &mut impl Write, tag: u32, result: u32) {
        write_header(stream, MESSAGE_RESULT, tag, &result.to_le_bytes());
    }

    fn write_header(stream: &mut impl Write, message: u32, tag: u32, payload: &[u8]) {
        let length = u32::try_from(16 + payload.len()).expect("length");
        let mut header = [0_u8; 16];
        header[0..4].copy_from_slice(&length.to_le_bytes());
        header[4..8].copy_from_slice(&1_u32.to_le_bytes());
        header[8..12].copy_from_slice(&message.to_le_bytes());
        header[12..16].copy_from_slice(&tag.to_le_bytes());
        stream.write_all(&header).expect("reply header");
        stream.write_all(payload).expect("reply payload");
    }

    /// A `ListDevices` reply with two devices, one of them over USB.
    fn device_list() -> Value {
        let properties = |id: u64, serial: &str, product: u64, connection: &str| {
            let mut dict = Dictionary::new();
            dict.insert("DeviceID".to_owned(), Value::from(id));
            dict.insert("SerialNumber".to_owned(), Value::from(serial));
            dict.insert("ProductID".to_owned(), Value::from(product));
            dict.insert("ConnectionType".to_owned(), Value::from(connection));
            let mut entry = Dictionary::new();
            entry.insert("Properties".to_owned(), Value::Dictionary(dict));
            entry.insert("MessageType".to_owned(), Value::from("Attached"));
            Value::Dictionary(entry)
        };

        let mut dict = Dictionary::new();
        dict.insert("MessageType".to_owned(), Value::from("Result"));
        dict.insert("Number".to_owned(), Value::from(0_u64));
        dict.insert(
            "DeviceList".to_owned(),
            Value::Array(vec![
                properties(3, "00008110-001A2C3D4E5F", 0x12_AB, "USB"),
                properties(4, "00008120-009A8B7C6D5E", 0x12_AB, "Network"),
            ]),
        );
        Value::Dictionary(dict)
    }

    /// Bind a fake daemon socket in a temporary directory.
    fn fake_daemon() -> (tempfile::TempDir, std::path::PathBuf, UnixListener) {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("usbmuxd");
        let listener = UnixListener::bind(&path).expect("bind fake daemon");
        (directory, path, listener)
    }

    #[test]
    fn lists_devices_from_the_daemon_reply() {
        let (_directory, path, listener) = fake_daemon();
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let (tag, request) = read_request(&mut stream);
            assert_eq!(
                request.get("MessageType").and_then(Value::as_string),
                Some("ListDevices")
            );
            assert_eq!(
                request.get("ProgName").and_then(Value::as_string),
                Some("absolutetouch")
            );
            write_plist(&mut stream, tag, &device_list());
        });

        let mut daemon = Daemon::at(&path).expect("connect daemon");
        let devices = daemon.list_devices().expect("list devices");
        thread.join().expect("daemon thread");

        assert_eq!(
            devices,
            vec![
                Device {
                    id: 3,
                    serial: "00008110-001A2C3D4E5F".to_owned(),
                    product_id: 0x12_AB,
                    connection_type: "USB".to_owned(),
                },
                Device {
                    id: 4,
                    serial: "00008120-009A8B7C6D5E".to_owned(),
                    product_id: 0x12_AB,
                    connection_type: "Network".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn an_empty_device_list_is_not_an_error() {
        let (_directory, path, listener) = fake_daemon();
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let (tag, _) = read_request(&mut stream);
            let mut dict = Dictionary::new();
            dict.insert("MessageType".to_owned(), Value::from("Result"));
            dict.insert("Number".to_owned(), Value::from(0_u64));
            write_plist(&mut stream, tag, &Value::Dictionary(dict));
        });

        let mut daemon = Daemon::at(&path).expect("connect daemon");
        assert_eq!(daemon.list_devices().expect("list devices"), Vec::new());
        thread.join().expect("daemon thread");
    }

    #[test]
    fn connect_sends_a_byte_swapped_port_and_returns_the_raw_stream() {
        let (_directory, path, listener) = fake_daemon();
        let port = 37_690_u16;
        let mut hello = Packet::new(PacketType::Hello);
        hello.set_token([9; 16]);
        let expected = hello.encode();

        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let (tag, request) = read_request(&mut stream);
            assert_eq!(
                request.get("MessageType").and_then(Value::as_string),
                Some("Connect")
            );
            assert_eq!(
                request.get("DeviceID").and_then(Value::as_unsigned_integer),
                Some(3)
            );
            // The daemon wants network byte order; a little-endian host value
            // here would ask for the wrong port.
            assert_eq!(
                request
                    .get("PortNumber")
                    .and_then(Value::as_unsigned_integer),
                Some(u64::from(port.to_be()))
            );

            write_result(&mut stream, tag, 0);
            stream.write_all(&expected).expect("raw packet");
        });

        let daemon = Daemon::at(&path).expect("connect daemon");
        let mut stream = daemon.connect_device(3, port).expect("connect device");

        let mut received = [0_u8; 40];
        stream.read_exact(&mut received).expect("read raw packet");
        assert_eq!(received, expected);
        thread.join().expect("daemon thread");
    }

    #[test]
    fn a_refused_port_names_the_port_and_the_app() {
        let (_directory, path, listener) = fake_daemon();
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let (tag, _) = read_request(&mut stream);
            write_result(&mut stream, tag, 3);
        });

        let daemon = Daemon::at(&path).expect("connect daemon");
        let error = daemon.connect_device(3, 37_690).expect_err("refused");
        thread.join().expect("daemon thread");

        assert!(matches!(error, UsbMuxError::Refused { port: 37_690 }));
        let message = error.to_string();
        assert!(message.contains("37690"), "{message}");
        assert!(message.contains("AbsoluteTouch"), "{message}");
    }

    #[test]
    fn a_lost_device_is_reported_as_missing() {
        let (_directory, path, listener) = fake_daemon();
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let (tag, _) = read_request(&mut stream);
            write_result(&mut stream, tag, 2);
        });

        let daemon = Daemon::at(&path).expect("connect daemon");
        let error = daemon.connect_device(3, 37_690).expect_err("lost device");
        thread.join().expect("daemon thread");

        assert!(matches!(error, UsbMuxError::NoDevice(_)));
    }

    #[test]
    fn an_old_daemon_gets_an_update_hint() {
        let (_directory, path, listener) = fake_daemon();
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let (tag, _) = read_request(&mut stream);
            write_result(&mut stream, tag, 6);
        });

        let mut daemon = Daemon::at(&path).expect("connect daemon");
        let error = daemon.list_devices().expect_err("old daemon");
        thread.join().expect("daemon thread");

        match error {
            UsbMuxError::Protocol(message) => assert!(message.contains("update")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn a_closed_daemon_is_a_protocol_error() {
        let (_directory, path, listener) = fake_daemon();
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let _ = read_request(&mut stream);
            // Drop the socket without replying.
        });

        let mut daemon = Daemon::at(&path).expect("connect daemon");
        let error = daemon.list_devices().expect_err("closed");
        thread.join().expect("daemon thread");
        assert!(matches!(error, UsbMuxError::Protocol(_)), "{error:?}");
    }

    #[test]
    fn a_missing_socket_names_the_path_it_tried() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("missing");
        match Daemon::at(&path) {
            Err(UsbMuxError::Unavailable { path: tried, .. }) => assert_eq!(tried, path),
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn selection_accepts_a_serial_fragment_and_rejects_ambiguity() {
        let devices = two_devices();
        assert_eq!(select(&devices, Some("001a2c")).expect("fragment").id, 3);

        let ambiguous = select(&devices, None).expect_err("two devices");
        assert!(ambiguous.to_string().contains("00008110"), "{ambiguous}");
        assert!(matches!(
            select(&devices, Some("nope")),
            Err(UsbMuxError::NoDevice(_))
        ));
        assert_eq!(select(&devices[..1], None).expect("only device").id, 3);
        assert!(matches!(select(&[], None), Err(UsbMuxError::NoDevice(_))));
    }

    /// Two devices whose serials share a prefix.
    fn two_devices() -> Vec<Device> {
        vec![
            Device {
                id: 3,
                serial: "00008110-001A2C3D4E5F".to_owned(),
                product_id: 0x12_AB,
                connection_type: "USB".to_owned(),
            },
            Device {
                id: 4,
                serial: "00008110-009A8B7C6D5E".to_owned(),
                product_id: 0x12_AB,
                connection_type: "USB".to_owned(),
            },
        ]
    }

    #[test]
    fn the_socket_path_can_be_overridden() {
        use std::ffi::OsString;

        assert_eq!(
            super::path_from_override(Some(OsString::from("/tmp/test-usbmuxd"))),
            std::path::PathBuf::from("/tmp/test-usbmuxd")
        );
        assert_eq!(
            super::path_from_override(None),
            std::path::PathBuf::from(super::DEFAULT_SOCKET)
        );
    }
}
