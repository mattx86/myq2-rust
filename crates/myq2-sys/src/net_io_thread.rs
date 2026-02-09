// net_io_thread.rs — Dedicated I/O thread for async network packet processing
//
// This module provides background threads that handle network I/O for both UDP
// and TCP, decoupling network operations from the main game loop. Packets are
// received in the I/O thread and enqueued for processing by the game thread.

use std::io::{self, Read};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use myq2_common::common::{com_printf, sys_milliseconds};
use myq2_common::net_queue::{PacketQueueSender, QueuedPacket};
use myq2_common::qcommon::{MAX_MSGLEN, NetAdr, NetSrc};
use crate::net_common::socket_addr_to_netadr;

/// Poll timeout for non-blocking socket operations (milliseconds).
/// Shorter values give faster shutdown response but higher CPU usage.
const IO_POLL_TIMEOUT_MS: u64 = 10;

/// Maximum number of packets to process per I/O loop iteration.
/// Prevents starvation of other sockets when one is very busy.
const MAX_PACKETS_PER_ITERATION: usize = 32;

// =============================================================================
// UDP I/O Thread
// =============================================================================

/// Configuration for a UDP I/O thread.
pub struct UdpIoConfig {
    /// Which socket type this handles (Client or Server)
    pub sock: NetSrc,
    /// The UDP socket to receive from
    pub socket: Arc<UdpSocket>,
    /// Channel to send received packets to the game thread
    pub sender: PacketQueueSender,
    /// Shutdown signal
    pub shutdown: Arc<AtomicBool>,
}

/// Spawn a UDP I/O thread that receives packets and enqueues them.
///
/// Returns a JoinHandle that can be used to wait for thread completion.
pub fn spawn_udp_io_thread(config: UdpIoConfig) -> JoinHandle<()> {
    thread::Builder::new()
        .name(format!("udp-io-{:?}", config.sock))
        .spawn(move || {
            udp_io_loop(config);
        })
        .expect("Failed to spawn UDP I/O thread")
}

/// Main UDP I/O loop - runs until shutdown is signaled.
fn udp_io_loop(config: UdpIoConfig) {
    let socket = &config.socket;
    let sender = &config.sender;
    let shutdown = &config.shutdown;
    let sock = config.sock;

    // Set socket to non-blocking for polling
    if let Err(e) = socket.set_read_timeout(Some(Duration::from_millis(IO_POLL_TIMEOUT_MS))) {
        com_printf(&format!("UDP I/O thread: failed to set timeout: {}\n", e));
        return;
    }

    let mut buf = [0u8; MAX_MSGLEN];

    while !shutdown.load(Ordering::Relaxed) {
        // Check if channel is disconnected (receiver dropped)
        if sender.is_disconnected() {
            break;
        }

        let mut packets_this_iteration = 0;

        // Receive packets until would-block or limit reached
        loop {
            match socket.recv_from(&mut buf) {
                Ok((size, from_addr)) => {
                    if size > 0 && size < MAX_MSGLEN {
                        let from = socket_addr_to_netadr(&from_addr);
                        let packet = QueuedPacket::new(
                            sock,
                            from,
                            buf[..size].to_vec(),
                            sys_milliseconds(),
                        );

                        // Try to enqueue - if queue is full, drop the packet
                        let _ = sender.try_send(packet);

                        packets_this_iteration += 1;
                        if packets_this_iteration >= MAX_PACKETS_PER_ITERATION {
                            break;
                        }
                    }
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    break;
                }
                Err(ref e) if e.kind() == io::ErrorKind::TimedOut => {
                    break;
                }
                Err(e) => {
                    if !shutdown.load(Ordering::Relaxed) {
                        com_printf(&format!("UDP I/O error: {}\n", e));
                    }
                    break;
                }
            }
        }
    }
}

// =============================================================================
// TCP I/O Thread
// =============================================================================

/// Configuration for a TCP I/O thread.
pub struct TcpIoConfig {
    /// Which socket type this handles (Client or Server)
    pub sock: NetSrc,
    /// The TCP listener for incoming connections (server)
    pub listener: Option<Arc<TcpListener>>,
    /// Existing TCP stream for client connections
    pub stream: Option<Arc<parking_lot::Mutex<TcpStream>>>,
    /// Channel to send received packets to the game thread
    pub sender: PacketQueueSender,
    /// Shutdown signal
    pub shutdown: Arc<AtomicBool>,
}

/// State for managing TCP connections in the I/O thread.
struct TcpConnectionState {
    /// Active TCP stream (accepted connection or outgoing)
    stream: Option<TcpStream>,
    /// Address of the connected peer
    peer_addr: NetAdr,
}

/// Spawn a TCP I/O thread that receives packets and enqueues them.
///
/// Returns a JoinHandle that can be used to wait for thread completion.
pub fn spawn_tcp_io_thread(config: TcpIoConfig) -> JoinHandle<()> {
    thread::Builder::new()
        .name(format!("tcp-io-{:?}", config.sock))
        .spawn(move || {
            tcp_io_loop(config);
        })
        .expect("Failed to spawn TCP I/O thread")
}

/// Main TCP I/O loop - runs until shutdown is signaled.
fn tcp_io_loop(config: TcpIoConfig) {
    let sender = &config.sender;
    let shutdown = &config.shutdown;
    let sock = config.sock;

    // Set up initial connection state
    let mut conn_state = TcpConnectionState {
        stream: None,
        peer_addr: NetAdr::default(),
    };

    // If we have an existing stream, use it
    if let Some(ref stream_mutex) = config.stream {
        if let Ok(stream) = stream_mutex.lock().try_clone() {
            if let Ok(peer) = stream.peer_addr() {
                conn_state.peer_addr = socket_addr_to_netadr(&peer);
            }
            let _ = stream.set_read_timeout(Some(Duration::from_millis(IO_POLL_TIMEOUT_MS)));
            conn_state.stream = Some(stream);
        }
    }

    // Set listener to non-blocking if present
    if let Some(ref listener) = config.listener {
        let _ = listener.set_nonblocking(true);
    }

    let mut buf = [0u8; MAX_MSGLEN];

    while !shutdown.load(Ordering::Relaxed) {
        // Check if channel is disconnected
        if sender.is_disconnected() {
            break;
        }

        // Try to accept new connections if we have a listener and no active stream
        if conn_state.stream.is_none() {
            if let Some(ref listener) = config.listener {
                match listener.accept() {
                    Ok((stream, addr)) => {
                        let _ = stream.set_nonblocking(true);
                        let _ = stream.set_read_timeout(Some(Duration::from_millis(IO_POLL_TIMEOUT_MS)));
                        conn_state.peer_addr = socket_addr_to_netadr(&addr);
                        conn_state.stream = Some(stream);
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        // No pending connections
                    }
                    Err(_) => {
                        // Accept error - continue
                    }
                }
            }
        }

        // Read from active stream
        if let Some(ref mut stream) = conn_state.stream {
            let mut packets_this_iteration = 0;

            loop {
                match stream.read(&mut buf) {
                    Ok(0) => {
                        // Connection closed
                        conn_state.stream = None;
                        conn_state.peer_addr = NetAdr::default();
                        break;
                    }
                    Ok(size) => {
                        let packet = QueuedPacket::new(
                            sock,
                            conn_state.peer_addr,
                            buf[..size].to_vec(),
                            sys_milliseconds(),
                        );

                        let _ = sender.try_send(packet);

                        packets_this_iteration += 1;
                        if packets_this_iteration >= MAX_PACKETS_PER_ITERATION {
                            break;
                        }
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        break;
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::TimedOut => {
                        break;
                    }
                    Err(_e) => {
                        // Connection error - close stream
                        conn_state.stream = None;
                        conn_state.peer_addr = NetAdr::default();
                        break;
                    }
                }
            }
        }

        // Small sleep if no work was done to prevent busy-waiting
        if conn_state.stream.is_none() && config.listener.is_none() {
            thread::sleep(Duration::from_millis(IO_POLL_TIMEOUT_MS));
        }
    }
}

// =============================================================================
// I/O Thread Manager
// =============================================================================

/// Manages the lifecycle of network I/O threads.
pub struct NetIoThreadManager {
    /// Shutdown signal shared with all threads
    shutdown: Arc<AtomicBool>,
    /// Thread handles for cleanup
    threads: Vec<JoinHandle<()>>,
}

impl NetIoThreadManager {
    /// Create a new I/O thread manager.
    pub fn new() -> Self {
        Self {
            shutdown: Arc::new(AtomicBool::new(false)),
            threads: Vec::new(),
        }
    }

    /// Check if any threads are running.
    pub fn is_enabled(&self) -> bool {
        !self.threads.is_empty()
    }

    /// Get the shutdown signal for creating new threads.
    pub fn shutdown_signal(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.shutdown)
    }

    /// Spawn a UDP I/O thread and track it.
    pub fn spawn_udp(
        &mut self,
        sock: NetSrc,
        socket: Arc<UdpSocket>,
        sender: PacketQueueSender,
    ) {
        let config = UdpIoConfig {
            sock,
            socket,
            sender,
            shutdown: Arc::clone(&self.shutdown),
        };
        let handle = spawn_udp_io_thread(config);
        self.threads.push(handle);
    }

    /// Spawn a TCP I/O thread and track it.
    pub fn spawn_tcp(
        &mut self,
        sock: NetSrc,
        listener: Option<Arc<TcpListener>>,
        stream: Option<Arc<parking_lot::Mutex<TcpStream>>>,
        sender: PacketQueueSender,
    ) {
        let config = TcpIoConfig {
            sock,
            listener,
            stream,
            sender,
            shutdown: Arc::clone(&self.shutdown),
        };
        let handle = spawn_tcp_io_thread(config);
        self.threads.push(handle);
    }

    /// Signal all threads to shut down.
    pub fn signal_shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }

    /// Shut down all I/O threads and wait for them to finish.
    pub fn shutdown(&mut self) {
        self.signal_shutdown();

        // Wait for all threads with a timeout
        for handle in self.threads.drain(..) {
            let _ = handle.join();
        }
    }
}

impl Default for NetIoThreadManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for NetIoThreadManager {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_io_manager_lifecycle() {
        let mut manager = NetIoThreadManager::new();
        assert!(!manager.is_enabled());

        // Shutdown should be safe even with no threads
        manager.shutdown();
        assert!(!manager.is_enabled());
    }
}
