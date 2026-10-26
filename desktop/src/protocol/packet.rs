//! Fixed-size packet encoding and decoding.

use crate::protocol::types::{MAGIC, PACKET_SIZE, PacketType, ProtocolError, VERSION};

/// A decoded 40-byte protocol packet.
///
/// Fields are little-endian on the wire. Some payload fields are
/// type-specific: [`Packet::x`] through [`Packet::reserved`] carry the 16-byte
/// token in `HELLO` and `HELLO_ACK` packets, and two 32-bit timestamps in
/// `HEARTBEAT` packets. Use [`Packet::token`] when working with a handshake
/// packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Packet {
    /// Protocol version.
    pub version: u8,
    /// Packet kind.
    pub packet_type: PacketType,
    /// Flag bits; see [`crate::protocol::types::flags`].
    pub flags: u16,
    /// Sequence number. Wraps at 65,535.
    pub seq: u16,
    /// Session identifier assigned by the responder.
    pub session_id: u32,
    /// Device monotonic timestamp in microseconds.
    pub device_time_us: u32,
    /// Normalized X position.
    pub x: u16,
    /// Normalized Y position.
    pub y: u16,
    /// Normalized pressure.
    pub pressure: u16,
    /// Pencil altitude angle, normalized.
    pub altitude: u16,
    /// Pencil azimuth angle, normalized.
    pub azimuth: u16,
    /// Pencil twist, normalized. Reserved on current hardware.
    pub twist: u16,
    /// Reserved telemetry field. Senders set zero in version 1.
    pub status: u16,
    /// Reserved for future use.
    pub reserved: u16,
    /// Reserved authentication tag.
    pub auth_tag: [u8; 8],
}

impl Packet {
    /// Create a zeroed packet of the given kind with the current version.
    pub const fn new(packet_type: PacketType) -> Self {
        Self {
            version: VERSION,
            packet_type,
            flags: 0,
            seq: 0,
            session_id: 0,
            device_time_us: 0,
            x: 0,
            y: 0,
            pressure: 0,
            altitude: 0,
            azimuth: 0,
            twist: 0,
            status: 0,
            reserved: 0,
            auth_tag: [0; 8],
        }
    }

    /// Decode a packet from exactly [`PACKET_SIZE`] bytes.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] for a wrong length, bad magic, unsupported
    /// version, or unknown packet type.
    pub fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        if bytes.len() != PACKET_SIZE {
            return Err(ProtocolError::InvalidLength(bytes.len()));
        }

        let magic = read_u16(bytes, 0);
        if magic != MAGIC {
            return Err(ProtocolError::BadMagic(magic));
        }

        let version = bytes[2];
        if version != VERSION {
            return Err(ProtocolError::UnsupportedVersion(version));
        }

        let packet_type = PacketType::try_from(bytes[3])?;
        let mut auth_tag = [0_u8; 8];
        auth_tag.copy_from_slice(&bytes[32..40]);

        Ok(Self {
            version,
            packet_type,
            flags: read_u16(bytes, 4),
            seq: read_u16(bytes, 6),
            session_id: read_u32(bytes, 8),
            device_time_us: read_u32(bytes, 12),
            x: read_u16(bytes, 16),
            y: read_u16(bytes, 18),
            pressure: read_u16(bytes, 20),
            altitude: read_u16(bytes, 22),
            azimuth: read_u16(bytes, 24),
            twist: read_u16(bytes, 26),
            status: read_u16(bytes, 28),
            reserved: read_u16(bytes, 30),
            auth_tag,
        })
    }

    /// Encode the packet into exactly [`PACKET_SIZE`] bytes.
    pub fn encode(&self) -> [u8; PACKET_SIZE] {
        let mut bytes = [0_u8; PACKET_SIZE];
        bytes[0..2].copy_from_slice(&MAGIC.to_le_bytes());
        bytes[2] = self.version;
        bytes[3] = self.packet_type as u8;
        bytes[4..6].copy_from_slice(&self.flags.to_le_bytes());
        bytes[6..8].copy_from_slice(&self.seq.to_le_bytes());
        bytes[8..12].copy_from_slice(&self.session_id.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.device_time_us.to_le_bytes());
        bytes[16..18].copy_from_slice(&self.x.to_le_bytes());
        bytes[18..20].copy_from_slice(&self.y.to_le_bytes());
        bytes[20..22].copy_from_slice(&self.pressure.to_le_bytes());
        bytes[22..24].copy_from_slice(&self.altitude.to_le_bytes());
        bytes[24..26].copy_from_slice(&self.azimuth.to_le_bytes());
        bytes[26..28].copy_from_slice(&self.twist.to_le_bytes());
        bytes[28..30].copy_from_slice(&self.status.to_le_bytes());
        bytes[30..32].copy_from_slice(&self.reserved.to_le_bytes());
        bytes[32..40].copy_from_slice(&self.auth_tag);
        bytes
    }

    /// Return the 16-byte token stored in a `HELLO` or `HELLO_ACK` packet.
    pub fn token(&self) -> [u8; 16] {
        let mut token = [0_u8; 16];
        token[0..2].copy_from_slice(&self.x.to_le_bytes());
        token[2..4].copy_from_slice(&self.y.to_le_bytes());
        token[4..6].copy_from_slice(&self.pressure.to_le_bytes());
        token[6..8].copy_from_slice(&self.altitude.to_le_bytes());
        token[8..10].copy_from_slice(&self.azimuth.to_le_bytes());
        token[10..12].copy_from_slice(&self.twist.to_le_bytes());
        token[12..14].copy_from_slice(&self.status.to_le_bytes());
        token[14..16].copy_from_slice(&self.reserved.to_le_bytes());
        token
    }

    /// Store a 16-byte token in `HELLO` or `HELLO_ACK` payload fields.
    pub fn set_token(&mut self, token: [u8; 16]) {
        self.x = u16::from_le_bytes([token[0], token[1]]);
        self.y = u16::from_le_bytes([token[2], token[3]]);
        self.pressure = u16::from_le_bytes([token[4], token[5]]);
        self.altitude = u16::from_le_bytes([token[6], token[7]]);
        self.azimuth = u16::from_le_bytes([token[8], token[9]]);
        self.twist = u16::from_le_bytes([token[10], token[11]]);
        self.status = u16::from_le_bytes([token[12], token[13]]);
        self.reserved = u16::from_le_bytes([token[14], token[15]]);
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

#[cfg(test)]
mod tests {
    use super::Packet;
    use crate::protocol::types::{PACKET_SIZE, PacketType, ProtocolError, VERSION};
    use serde::Deserialize;

    const VECTORS: &str = include_str!("../../../protocol/vectors.json");

    #[derive(Debug, Deserialize)]
    struct VectorFile {
        version: u8,
        packet_size: usize,
        packets: Vec<Vector>,
    }

    #[derive(Debug, Deserialize)]
    struct Vector {
        name: String,
        hex: String,
        version: u8,
        #[serde(rename = "type")]
        packet_type: u8,
        flags: u16,
        seq: u16,
        session_id: u32,
        t_device_us: u32,
        #[serde(default)]
        x: u16,
        #[serde(default)]
        y: u16,
        #[serde(default)]
        pressure: u16,
        #[serde(default)]
        altitude: u16,
        #[serde(default)]
        azimuth: u16,
        #[serde(default)]
        twist: u16,
        #[serde(default)]
        status: u16,
        #[serde(default)]
        reserved: u16,
        token: Option<String>,
        auth_tag: String,
    }

    #[test]
    fn golden_vectors_decode_and_encode() {
        let file: VectorFile =
            serde_json::from_str(VECTORS).expect("protocol vectors are valid JSON");

        assert_eq!(file.version, VERSION);
        assert_eq!(file.packet_size, PACKET_SIZE);

        for vector in file.packets {
            let name = &vector.name;
            let bytes = hex::decode(&vector.hex).expect("vector hex is valid");
            let packet = Packet::decode(&bytes)
                .unwrap_or_else(|error| panic!("{name}: decode failed: {error}"));

            assert_eq!(packet.version, vector.version, "{name}: version");
            assert_eq!(
                packet.packet_type as u8, vector.packet_type,
                "{name}: packet type"
            );
            assert_eq!(packet.flags, vector.flags, "{name}: flags");
            assert_eq!(packet.seq, vector.seq, "{name}: sequence");
            assert_eq!(packet.session_id, vector.session_id, "{name}: session");
            assert_eq!(
                packet.device_time_us, vector.t_device_us,
                "{name}: device time"
            );
            assert_eq!(
                packet.auth_tag,
                fixed::<8>(&vector.auth_tag),
                "{name}: auth tag"
            );

            if let Some(token) = &vector.token {
                assert_eq!(packet.token(), fixed::<16>(token), "{name}: token");
            } else {
                assert_eq!(packet.x, vector.x, "{name}: x");
                assert_eq!(packet.y, vector.y, "{name}: y");
                assert_eq!(packet.pressure, vector.pressure, "{name}: pressure");
                assert_eq!(packet.altitude, vector.altitude, "{name}: altitude");
                assert_eq!(packet.azimuth, vector.azimuth, "{name}: azimuth");
                assert_eq!(packet.twist, vector.twist, "{name}: twist");
                assert_eq!(packet.status, vector.status, "{name}: status");
                assert_eq!(packet.reserved, vector.reserved, "{name}: reserved");
            }

            assert_eq!(
                packet.encode().as_slice(),
                bytes.as_slice(),
                "{name}: re-encode"
            );
        }
    }

    #[test]
    fn rejects_wrong_length() {
        assert_eq!(
            Packet::decode(&[0_u8; PACKET_SIZE - 1]),
            Err(ProtocolError::InvalidLength(PACKET_SIZE - 1))
        );
        assert_eq!(
            Packet::decode(&[0_u8; PACKET_SIZE + 1]),
            Err(ProtocolError::InvalidLength(PACKET_SIZE + 1))
        );
    }

    #[test]
    fn rejects_bad_magic() {
        let mut bytes = Packet::new(PacketType::Sample).encode();
        bytes[0] = 0;
        bytes[1] = 0;

        assert_eq!(Packet::decode(&bytes), Err(ProtocolError::BadMagic(0)));
    }

    #[test]
    fn rejects_unsupported_version() {
        let mut bytes = Packet::new(PacketType::Sample).encode();
        bytes[2] = VERSION + 1;

        assert_eq!(
            Packet::decode(&bytes),
            Err(ProtocolError::UnsupportedVersion(VERSION + 1))
        );
    }

    #[test]
    fn rejects_unknown_packet_type() {
        let mut bytes = Packet::new(PacketType::Sample).encode();
        bytes[3] = 99;

        assert_eq!(Packet::decode(&bytes), Err(ProtocolError::UnknownType(99)));
    }

    #[test]
    fn token_roundtrips() {
        let mut packet = Packet::new(PacketType::Hello);
        let token = [0xAB; 16];

        packet.set_token(token);

        assert_eq!(packet.token(), token);
    }

    fn fixed<const N: usize>(value: &str) -> [u8; N] {
        let bytes = hex::decode(value).expect("valid hex");
        assert_eq!(bytes.len(), N, "expected {N} bytes");
        let mut output = [0_u8; N];
        output.copy_from_slice(&bytes);
        output
    }
}
