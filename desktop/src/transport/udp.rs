//! Wi-Fi UDP transport.
//!
//! One datagram is one packet. A receiver thread decodes datagrams and hands
//! them to the session loop over a bounded channel; a full channel is a counted
//! drop, never a blocked socket.

use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::protocol::packet::Packet;
use crate::protocol::types::ProtocolError;

/// Maximum datagrams buffered between the receiver and the session loop.
pub const QUEUE_CAPACITY: usize = 256;
/// Socket poll interval, which bounds how quickly the receiver stops.
pub const POLL_INTERVAL: Duration = Duration::from_millis(50);
/// Largest datagram the receiver reads; anything longer is invalid anyway.
const BUFFER_SIZE: usize = 2_048;

/// A datagram received from the network.
#[derive(Debug)]
pub enum Inbound {
    /// A valid packet.
    Packet {
        /// Source address.
        peer: SocketAddr,
        /// Decoded packet.
        packet: Packet,
    },
    /// A datagram that failed protocol decoding.
    Invalid {
        /// Source address.
        peer: SocketAddr,
        /// Decoding failure.
        error: ProtocolError,
    },
}

/// A bound UDP socket plus its receiver thread.
#[derive(Debug)]
pub struct UdpTransport {
    socket: UdpSocket,
    queue: Receiver<Inbound>,
    stop: Arc<AtomicBool>,
    dropped: Arc<AtomicU64>,
    thread: Option<JoinHandle<()>>,
}

impl UdpTransport {
    /// Bind the listener and start the receiver thread.
    ///
    /// # Errors
    ///
    /// Returns the underlying I/O error when the address cannot be bound.
    pub fn bind(bind: &str, port: u16) -> io::Result<Self> {
        let address = format!("{bind}:{port}");
        let socket = UdpSocket::bind(&address)?;
        socket.set_read_timeout(Some(POLL_INTERVAL))?;
        Self::from_socket(socket)
    }

    /// Use an existing, already bound socket.
    ///
    /// # Errors
    ///
    /// Returns the underlying I/O error when the socket cannot be configured.
    pub fn from_socket(socket: UdpSocket) -> io::Result<Self> {
        socket.set_read_timeout(Some(POLL_INTERVAL))?;
        let reader = socket.try_clone()?;
        let stop = Arc::new(AtomicBool::new(false));
        let dropped = Arc::new(AtomicU64::new(0));
        let (sender, queue) = sync_channel::<Inbound>(QUEUE_CAPACITY);

        let thread = {
            let stop = Arc::clone(&stop);
            let dropped = Arc::clone(&dropped);
            thread::Builder::new()
                .name("absolutetouch-udp".to_owned())
                .spawn(move || receive_loop(&reader, &sender, &stop, &dropped))?
        };

        Ok(Self {
            socket,
            queue,
            stop,
            dropped,
            thread: Some(thread),
        })
    }

    /// The bound address, including the port chosen by the OS when binding 0.
    ///
    /// # Errors
    ///
    /// Returns the underlying I/O error when the socket has no local address.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// Wait for the next datagram until `timeout`.
    ///
    /// Returns `Ok(None)` when the timeout expires.
    ///
    /// # Errors
    ///
    /// Returns the channel receive error when the receiver thread has stopped.
    pub fn recv_timeout(
        &self,
        timeout: Duration,
    ) -> Result<Option<Inbound>, std::sync::mpsc::RecvTimeoutError> {
        match self.queue.recv_timeout(timeout) {
            Ok(inbound) => Ok(Some(inbound)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Send one encoded packet.
    ///
    /// # Errors
    ///
    /// Returns the underlying I/O error when the datagram cannot be sent.
    pub fn send_to(&self, bytes: &[u8], peer: SocketAddr) -> io::Result<usize> {
        self.socket.send_to(bytes, peer)
    }

    /// Datagrams dropped because the queue was full or the reader stopped.
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// Ask the receiver thread to stop and wait for it.
    pub fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for UdpTransport {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn receive_loop(
    socket: &UdpSocket,
    sender: &SyncSender<Inbound>,
    stop: &AtomicBool,
    dropped: &AtomicU64,
) {
    let mut buffer = [0_u8; BUFFER_SIZE];

    while !stop.load(Ordering::Relaxed) {
        match socket.recv_from(&mut buffer) {
            Ok((length, peer)) => {
                let inbound = match Packet::decode(&buffer[..length]) {
                    Ok(packet) => Inbound::Packet { peer, packet },
                    Err(error) => Inbound::Invalid { peer, error },
                };
                match sender.try_send(inbound) {
                    Ok(()) => {}
                    Err(TrySendError::Full(_)) => {
                        dropped.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(TrySendError::Disconnected(_)) => break,
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(_) => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::UdpSocket;
    use std::time::Duration;

    use super::{Inbound, UdpTransport};
    use crate::protocol::packet::Packet;
    use crate::protocol::types::PacketType;

    fn loopback() -> (UdpTransport, UdpSocket) {
        let transport = UdpTransport::bind("127.0.0.1", 0).expect("bind");
        let target = transport.local_addr().expect("local addr");
        let client = UdpSocket::bind("127.0.0.1:0").expect("client bind");
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("timeout");
        client.connect(target).expect("connect");
        (transport, client)
    }

    #[test]
    fn receives_a_decoded_packet() {
        let (transport, client) = loopback();
        let mut packet = Packet::new(PacketType::Hello);
        packet.set_token([7; 16]);

        client.send(&packet.encode()).expect("send");

        match transport
            .recv_timeout(Duration::from_secs(2))
            .expect("recv")
        {
            Some(Inbound::Packet {
                packet: received, ..
            }) => assert_eq!(received, packet),
            other => panic!("unexpected inbound: {other:?}"),
        }
    }

    #[test]
    fn reports_invalid_datagrams() {
        let (transport, client) = loopback();

        client.send(b"not a packet").expect("send");

        match transport
            .recv_timeout(Duration::from_secs(2))
            .expect("recv")
        {
            Some(Inbound::Invalid { .. }) => {}
            other => panic!("unexpected inbound: {other:?}"),
        }
    }

    #[test]
    fn sends_replies_to_the_source() {
        let (transport, client) = loopback();
        let mut packet = Packet::new(PacketType::HelloAck);
        packet.session_id = 9;
        let peer = client.local_addr().expect("client addr");

        transport
            .send_to(&packet.encode(), peer)
            .expect("send reply");

        let mut buffer = [0_u8; 64];
        let length = client.recv(&mut buffer).expect("recv reply");
        assert_eq!(Packet::decode(&buffer[..length]).expect("decode"), packet);
    }

    #[test]
    fn times_out_without_traffic() {
        let transport = UdpTransport::bind("127.0.0.1", 0).expect("bind");

        assert!(
            transport
                .recv_timeout(Duration::from_millis(20))
                .expect("recv")
                .is_none()
        );
    }

    #[test]
    fn shutdown_stops_the_reader_thread() {
        let mut transport = UdpTransport::bind("127.0.0.1", 0).expect("bind");

        transport.shutdown();

        assert!(transport.recv_timeout(Duration::from_millis(20)).is_err());
    }
}
