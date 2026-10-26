//! Trace recording and replay.
//!
//! A trace is an internal, versioned file of inbound packets with their host
//! arrival offsets. It is not the wire protocol: the 40-byte packets are
//! recorded verbatim, wrapped in a header and timing metadata.
//!
//! Layout (little-endian):
//!
//! ```text
//! header:  4 bytes magic "ATTR", 1 byte version, 3 reserved bytes
//! record:  8-byte offset in microseconds since the first record, 40-byte packet
//! ```

use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use thiserror::Error;

use crate::protocol::packet::Packet;
use crate::protocol::types::{PACKET_SIZE, ProtocolError};

/// Trace file magic.
pub const MAGIC: [u8; 4] = *b"ATTR";
/// Trace format version.
pub const VERSION: u8 = 1;
/// Header size in bytes.
pub const HEADER_LEN: u64 = 8;
/// Record size in bytes.
pub const RECORD_LEN: u64 = 8 + PACKET_SIZE as u64;

/// Trace I/O and format errors.
#[derive(Debug, Error)]
pub enum TraceError {
    /// The file could not be opened for reading.
    #[error("cannot open trace {path}: {source}")]
    Open {
        /// Path that failed.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// The file could not be created.
    #[error("cannot create trace {path}: {source}")]
    Create {
        /// Path that failed.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// Reading or writing failed.
    #[error("trace I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// The magic bytes do not match.
    #[error("not an AbsoluteTouch trace: bad magic")]
    BadMagic,
    /// The format version is not supported.
    #[error("unsupported trace version {0}; this build writes version {VERSION}")]
    UnsupportedVersion(u8),
    /// The file does not divide into whole records.
    #[error("trace {path} is truncated: {length} bytes is not a whole number of records")]
    Truncated {
        /// Path that failed.
        path: PathBuf,
        /// Observed file length.
        length: u64,
    },
    /// A recorded packet does not decode.
    #[error("trace packet {index} is invalid: {source}")]
    Packet {
        /// Zero-based record index.
        index: u64,
        /// Protocol decoding error.
        #[source]
        source: ProtocolError,
    },
}

/// Writes a trace file.
#[derive(Debug)]
pub struct TraceWriter {
    file: BufWriter<std::fs::File>,
    origin: Option<Instant>,
    records: u64,
}

impl TraceWriter {
    /// Create a trace file, replacing any existing file.
    pub fn create(path: &Path) -> Result<Self, TraceError> {
        let file = std::fs::File::create(path).map_err(|source| TraceError::Create {
            path: path.to_path_buf(),
            source,
        })?;
        let mut file = BufWriter::new(file);
        let mut header = [0_u8; HEADER_LEN as usize];
        header[..4].copy_from_slice(&MAGIC);
        header[4] = VERSION;
        file.write_all(&header)?;

        Ok(Self {
            file,
            origin: None,
            records: 0,
        })
    }

    /// Append one inbound packet observed at `at`.
    pub fn record(&mut self, packet: &Packet, at: Instant) -> Result<(), TraceError> {
        let origin = *self.origin.get_or_insert(at);
        let offset = at.saturating_duration_since(origin).as_micros() as u64;

        self.file.write_all(&offset.to_le_bytes())?;
        self.file.write_all(&packet.encode())?;
        self.records += 1;
        Ok(())
    }

    /// Number of recorded packets.
    #[must_use]
    pub const fn records(&self) -> u64 {
        self.records
    }

    /// Flush the file and report the final record count.
    pub fn finish(mut self) -> Result<u64, TraceError> {
        self.file.flush()?;
        self.file.get_ref().sync_all()?;
        Ok(self.records)
    }
}

/// One decoded trace record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraceRecord {
    /// Host time since the first record.
    pub offset: Duration,
    /// Recorded packet.
    pub packet: Packet,
}

/// Reads a trace file.
#[derive(Debug)]
pub struct TraceReader {
    path: PathBuf,
    file: BufReader<std::fs::File>,
    index: u64,
}

impl TraceReader {
    /// Open and validate a trace file.
    pub fn open(path: &Path) -> Result<Self, TraceError> {
        let file = std::fs::File::open(path).map_err(|source| TraceError::Open {
            path: path.to_path_buf(),
            source,
        })?;
        let length = file.metadata()?.len();
        if length < HEADER_LEN {
            return Err(TraceError::Truncated {
                path: path.to_path_buf(),
                length,
            });
        }
        let mut file = BufReader::new(file);

        let mut header = [0_u8; HEADER_LEN as usize];
        file.read_exact(&mut header)?;
        if header[..4] != MAGIC {
            return Err(TraceError::BadMagic);
        }
        if header[4] != VERSION {
            return Err(TraceError::UnsupportedVersion(header[4]));
        }
        if (length - HEADER_LEN) % RECORD_LEN != 0 {
            return Err(TraceError::Truncated {
                path: path.to_path_buf(),
                length,
            });
        }

        Ok(Self {
            path: path.to_path_buf(),
            file,
            index: 0,
        })
    }

    /// Read the next record, or `None` at the end of the file.
    pub fn next_record(&mut self) -> Result<Option<TraceRecord>, TraceError> {
        let mut raw = [0_u8; RECORD_LEN as usize];
        match self.file.read_exact(&mut raw) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(error) => return Err(TraceError::Io(error)),
        }

        let mut offset_bytes = [0_u8; 8];
        offset_bytes.copy_from_slice(&raw[..8]);
        let offset = Duration::from_micros(u64::from_le_bytes(offset_bytes));

        let packet = Packet::decode(&raw[8..]).map_err(|source| TraceError::Packet {
            index: self.index,
            source,
        })?;
        self.index += 1;

        Ok(Some(TraceRecord { offset, packet }))
    }

    /// Path this reader opened.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{MAGIC, RECORD_LEN, TraceError, TraceReader, TraceWriter, VERSION};
    use crate::protocol::packet::Packet;
    use crate::protocol::types::PacketType;

    fn packet(seq: u16) -> Packet {
        let mut packet = Packet::new(PacketType::Sample);
        packet.seq = seq;
        packet.session_id = 42;
        packet
    }

    #[test]
    fn roundtrip_preserves_packets_and_offsets() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("session.atrace");
        let start = Instant::now();

        let mut writer = TraceWriter::create(&path).expect("create");
        writer.record(&packet(1), start).expect("record");
        writer
            .record(&packet(2), start + Duration::from_millis(1_500))
            .expect("record");
        assert_eq!(writer.records(), 2);
        assert_eq!(writer.finish().expect("finish"), 2);

        let mut reader = TraceReader::open(&path).expect("open");
        let first = reader.next_record().expect("read").expect("record");
        let second = reader.next_record().expect("read").expect("record");

        assert_eq!(first.offset, Duration::ZERO);
        assert_eq!(first.packet, packet(1));
        assert_eq!(second.offset, Duration::from_millis(1_500));
        assert_eq!(second.packet, packet(2));
        assert!(reader.next_record().expect("read").is_none());
    }

    #[test]
    fn empty_trace_is_valid() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("empty.atrace");

        let writer = TraceWriter::create(&path).expect("create");
        assert_eq!(writer.finish().expect("finish"), 0);

        let mut reader = TraceReader::open(&path).expect("open");
        assert!(reader.next_record().expect("read").is_none());
    }

    #[test]
    fn bad_magic_is_rejected() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("bad.atrace");
        std::fs::write(&path, b"NOPE\x01\x00\x00\x00").expect("write");

        assert!(matches!(
            TraceReader::open(&path),
            Err(TraceError::BadMagic)
        ));
    }

    #[test]
    fn unsupported_version_is_rejected() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("old.atrace");
        let mut header = [0_u8; 8];
        header[..4].copy_from_slice(&MAGIC);
        header[4] = VERSION + 1;
        std::fs::write(&path, header).expect("write");

        assert!(matches!(
            TraceReader::open(&path),
            Err(TraceError::UnsupportedVersion(_))
        ));
    }

    #[test]
    fn file_shorter_than_the_header_is_truncated() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("short.atrace");
        std::fs::write(&path, b"AT").expect("write");

        assert!(matches!(
            TraceReader::open(&path),
            Err(TraceError::Truncated { length: 2, .. })
        ));
    }

    #[test]
    fn partial_record_is_rejected() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("truncated.atrace");
        let mut bytes = vec![0_u8; 8];
        bytes[..4].copy_from_slice(&MAGIC);
        bytes[4] = VERSION;
        bytes.extend_from_slice(&[0_u8; (RECORD_LEN - 1) as usize]);
        std::fs::write(&path, bytes).expect("write");

        assert!(matches!(
            TraceReader::open(&path),
            Err(TraceError::Truncated { .. })
        ));
    }

    #[test]
    fn invalid_packet_is_reported_with_its_index() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("corrupt.atrace");
        let mut bytes = vec![0_u8; 8];
        bytes[..4].copy_from_slice(&MAGIC);
        bytes[4] = VERSION;
        bytes.extend_from_slice(&[0_u8; RECORD_LEN as usize]);
        std::fs::write(&path, bytes).expect("write");

        let mut reader = TraceReader::open(&path).expect("open");

        match reader.next_record() {
            Err(TraceError::Packet { index, .. }) => assert_eq!(index, 0),
            other => panic!("unexpected result: {other:?}"),
        }
    }
}
