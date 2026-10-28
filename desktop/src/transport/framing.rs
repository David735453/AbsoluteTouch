//! Fixed-size framing for the USB byte stream.
//!
//! USB is a reliable byte stream, not a datagram service: one read may return
//! half a packet or three packets at once. [`FrameDecoder`] buffers the bytes it
//! is given and hands back whole [`PACKET_SIZE`] frames in order.
//!
//! The stream carries no length prefix, because every packet already has a
//! fixed size. That keeps the USB wire format identical to the UDP datagram
//! format and means a recorded trace can be replayed over either transport. The
//! cost is that a peer which writes a partial frame desynchronizes the stream
//! until it closes; the desktop treats that as a protocol error rather than
//! trying to resynchronize on the magic bytes, which could split a valid packet
//! in two.

use crate::protocol::PACKET_SIZE;

/// Reassembles fixed-size packets from an arbitrary sequence of reads.
#[derive(Debug, Default, Clone)]
pub struct FrameDecoder {
    /// Bytes received but not yet returned as a frame.
    buffer: Vec<u8>,
}

impl FrameDecoder {
    /// Create an empty decoder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(PACKET_SIZE * 2),
        }
    }

    /// Append bytes read from the stream.
    ///
    /// Any complete frames become available through `next_frame`.
    pub fn push(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    /// Take the next complete frame, if one is buffered.
    pub fn next_frame(&mut self) -> Option<[u8; PACKET_SIZE]> {
        if self.buffer.len() < PACKET_SIZE {
            return None;
        }

        let mut frame = [0_u8; PACKET_SIZE];
        frame.copy_from_slice(&self.buffer[..PACKET_SIZE]);
        self.buffer.drain(..PACKET_SIZE);
        Some(frame)
    }

    /// Bytes buffered towards the next frame.
    #[must_use]
    pub fn buffered(&self) -> usize {
        self.buffer.len()
    }

    /// Number of complete frames waiting to be taken.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.buffer.len() / PACKET_SIZE
    }
}

#[cfg(test)]
mod tests {
    use super::{FrameDecoder, PACKET_SIZE};
    use crate::protocol::packet::Packet;
    use crate::protocol::types::PacketType;

    fn frame(seq: u16) -> [u8; PACKET_SIZE] {
        let mut packet = Packet::new(PacketType::Sample);
        packet.seq = seq;
        packet.encode()
    }

    fn decode_frames(sizes: &[usize]) -> Vec<u16> {
        let total: usize = sizes.iter().sum();
        assert_eq!(total % PACKET_SIZE, 0, "chunks must cover whole frames");
        let frames: Vec<_> = (0..u16::try_from(total / PACKET_SIZE).expect("frame count"))
            .map(frame)
            .collect();
        let wire: Vec<u8> = frames.iter().flatten().copied().collect();

        let mut decoder = FrameDecoder::new();
        let mut seqs = Vec::new();
        let mut offset = 0;
        for &size in sizes {
            decoder.push(&wire[offset..offset + size]);
            offset += size;
            while let Some(bytes) = decoder.next_frame() {
                let packet = Packet::decode(&bytes).expect("decode");
                seqs.push(packet.seq);
            }
        }

        assert_eq!(decoder.buffered(), 0);
        assert_eq!(decoder.pending(), 0);
        assert_eq!(decoder.next_frame(), None);
        seqs
    }

    #[test]
    fn one_byte_at_a_time_reassembles_every_frame() {
        let sizes = vec![1; PACKET_SIZE * 4];
        assert_eq!(decode_frames(&sizes), vec![0, 1, 2, 3]);
    }

    #[test]
    fn short_and_whole_reads_mix() {
        // 39 bytes leaves one byte short, 41 completes that frame and starts
        // the next, which then needs exactly one more read.
        assert_eq!(decode_frames(&[39, 41, 40]), vec![0, 1, 2]);
        assert_eq!(decode_frames(&[120]), vec![0, 1, 2]);
    }

    #[test]
    fn a_large_read_yields_every_complete_frame() {
        let mut decoder = FrameDecoder::new();
        let wire: Vec<u8> = (0..100_u16).flat_map(frame).collect();
        decoder.push(&wire);
        // One extra byte proves that a partial tail stays buffered rather than
        // being mistaken for a frame.
        decoder.push(&[0xFF]);

        assert_eq!(decoder.pending(), 100);
        assert_eq!(decoder.buffered(), PACKET_SIZE * 100 + 1);
        let seqs: Vec<u16> = std::iter::from_fn(|| decoder.next_frame())
            .map(|bytes| Packet::decode(&bytes).expect("decode").seq)
            .collect();
        assert_eq!(seqs, (0..100).collect::<Vec<_>>());
        assert_eq!(decoder.buffered(), 1);
    }

    #[test]
    fn an_empty_read_changes_nothing() {
        let mut decoder = FrameDecoder::new();
        decoder.push(&frame(9));
        decoder.push(&[]);
        assert_eq!(decoder.buffered(), PACKET_SIZE);
        assert_eq!(decoder.pending(), 1);

        let bytes = decoder.next_frame().expect("frame");
        assert_eq!(Packet::decode(&bytes).expect("decode").seq, 9);
        assert_eq!(decoder.buffered(), 0);
    }
}
