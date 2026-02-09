// net_tcp.rs -- Converted from myq2-original/win32/net_tcp.c
// TCP networking (active when SWAP_UDP_FOR_TCP is defined)
//
// All network I/O uses async threaded processing with packet queueing.
// Packets are received by background I/O threads and queued for the game thread.

#![allow(dead_code)]

use std::io::{self, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use myq2_common::common::com_printf;
use myq2_common::net_queue::{PacketQueue, DEFAULT_QUEUE_CAPACITY};
use myq2_common::qcommon::*;
use parking_lot::Mutex;
use socket2::{Domain, Protocol, Socket, Type, TcpKeepalive};

use crate::net_udp::{net_adr_to_string, PORT_CLIENT};
use crate::MAX_LOOPBACK;
use crate::net_common::{IPTOS_LOWDELAY, Loopback, netadr_to_socket_addr};
use crate::net_io_thread::NetIoThreadManager;

// =============================================================================
// Constants
// =============================================================================

/// TCP keepalive interval - how often to send keepalive probes
const TCP_KEEPALIVE_SECS: u64 = 60;

// =============================================================================
// TCP Net state
// =============================================================================

/// TCP-based networking state, mirroring the SWAP_UDP_FOR_TCP path in net_tcp.c.
pub struct NetTcpState {
    loopbacks: [Loopback; 2],
    /// Listening sockets for server/client
    ip_listeners: [Option<TcpListener>; 2],
    /// Connected streams (placeholder -- real implementation would manage
    /// multiple client connections)
    ip_streams: [Option<TcpStream>; 2],
    old_config: bool,
    pub net_shownet: f32,
    noudp: bool,
    /// Packet queue for async I/O - always active
    packet_queue: PacketQueue,
    /// I/O thread manager - handles background packet reception
    io_manager: NetIoThreadManager,
}

impl Default for NetTcpState {
    fn default() -> Self {
        Self {
            loopbacks: [Loopback::default(), Loopback::default()],
            ip_listeners: [None, None],
            ip_streams: [None, None],
            old_config: false,
            net_shownet: 0.0,
            noudp: false,
            packet_queue: PacketQueue::new(DEFAULT_QUEUE_CAPACITY),
            io_manager: NetIoThreadManager::new(),
        }
    }
}

impl NetTcpState {
    // =========================================================================
    // Loopback
    // =========================================================================

    fn net_get_loop_packet(
        &mut self,
        sock: NetSrc,
        net_from: &mut NetAdr,
        net_message: &mut SizeBuf,
    ) -> bool {
        let idx = sock as usize;
        let loop_buf = &mut self.loopbacks[idx];

        if loop_buf.send - loop_buf.get > MAX_LOOPBACK as i32 {
            loop_buf.get = loop_buf.send - MAX_LOOPBACK as i32;
        }

        if loop_buf.get >= loop_buf.send {
            return false;
        }

        let i = (loop_buf.get & (MAX_LOOPBACK as i32 - 1)) as usize;
        loop_buf.get += 1;

        let datalen = loop_buf.msgs[i].datalen as usize;
        net_message.data[..datalen].copy_from_slice(&loop_buf.msgs[i].data[..datalen]);
        net_message.cursize = loop_buf.msgs[i].datalen;
        *net_from = NetAdr::default();
        net_from.adr_type = NetAdrType::Loopback;
        true
    }

    fn net_send_loop_packet(&mut self, sock: NetSrc, data: &[u8]) {
        let idx = (sock as usize) ^ 1;
        let loop_buf = &mut self.loopbacks[idx];

        let i = (loop_buf.send & (MAX_LOOPBACK as i32 - 1)) as usize;
        loop_buf.send += 1;

        let len = data.len().min(MAX_MSGLEN);
        loop_buf.msgs[i].data[..len].copy_from_slice(&data[..len]);
        loop_buf.msgs[i].datalen = len as i32;
    }

    // =========================================================================
    // I/O Thread Management
    // =========================================================================

    /// Start the I/O thread for a socket. Called automatically when socket opens.
    fn start_io_thread(&mut self, sock: NetSrc) {
        let idx = sock as usize;

        let sender = self.packet_queue.sender();

        // Wrap listener in Arc if present
        let listener = self.ip_listeners[idx].as_ref().and_then(|l| {
            l.try_clone().ok().map(Arc::new)
        });

        // Wrap stream in Arc<Mutex> if present
        let stream = self.ip_streams[idx].as_ref().and_then(|s| {
            s.try_clone().ok().map(|s| Arc::new(Mutex::new(s)))
        });

        if listener.is_none() && stream.is_none() {
            return;
        }

        self.io_manager.spawn_tcp(sock, listener, stream, sender);
        com_printf(&format!("Started TCP I/O thread for {:?}\n", sock));
    }

    /// Stop all I/O threads.
    fn stop_io_threads(&mut self) {
        self.io_manager.shutdown();
    }

    /// Get the number of packets currently queued.
    pub fn queued_packet_count(&self) -> usize {
        self.packet_queue.len()
    }

    // =========================================================================
    // Get / Send packets  (TCP variant)
    // =========================================================================

    /// Receive the next available packet from the queue.
    ///
    /// Packets are received asynchronously by the I/O thread and queued.
    /// This method dequeues and returns the next available packet.
    pub fn net_get_packet(
        &mut self,
        sock: NetSrc,
        net_from: &mut NetAdr,
        net_message: &mut SizeBuf,
    ) -> bool {
        // Always check loopback first
        if self.net_get_loop_packet(sock, net_from, net_message) {
            return true;
        }

        // Get packet from the async queue
        if let Some(packet) = self.packet_queue.try_recv() {
            if packet.data.len() >= net_message.maxsize as usize {
                com_printf(&format!(
                    "Oversize TCP packet from {}\n",
                    net_adr_to_string(&packet.from)
                ));
                return false;
            }

            *net_from = packet.from;
            net_message.data[..packet.data.len()].copy_from_slice(&packet.data);
            net_message.cursize = packet.data.len() as i32;
            return true;
        }

        false
    }

    /// Send a packet to the given address.
    ///
    /// Sends are still synchronous since TCP requires ordering and we need
    /// immediate error feedback for connection management.
    pub fn net_send_packet(&mut self, sock: NetSrc, data: &[u8], to: &NetAdr) {
        if to.adr_type == NetAdrType::Loopback {
            self.net_send_loop_packet(sock, data);
            return;
        }

        let idx = sock as usize;

        match to.adr_type {
            NetAdrType::Broadcast | NetAdrType::Ip => {
                // If we don't have a stream, try to connect
                if self.ip_streams[idx].is_none() {
                    match Self::connect_tcp_stream(to) {
                        Ok(stream) => {
                            self.ip_streams[idx] = Some(stream);
                        }
                        Err(e) => {
                            com_printf(&format!(
                                "NET_SendPacket (TCP) connect ERROR: {} to {}\n",
                                e,
                                net_adr_to_string(to)
                            ));
                            return;
                        }
                    }
                }

                if let Some(ref mut stream) = self.ip_streams[idx] {
                    if let Err(e) = stream.write_all(data) {
                        if e.kind() != io::ErrorKind::WouldBlock {
                            com_printf(&format!(
                                "NET_SendPacket (TCP) ERROR: {} to {}\n",
                                e,
                                net_adr_to_string(to)
                            ));
                        }
                    }
                }
            }
            _ => {
                com_printf("NET_SendPacket: bad address type\n");
            }
        }
    }

    // =========================================================================
    // Socket creation
    // =========================================================================

    /// Open a non-blocking TCP listener socket with low-delay ToS.
    fn net_ip_socket(net_interface: &str, port: i32) -> Option<TcpListener> {
        let bind_addr = if net_interface.is_empty()
            || net_interface.eq_ignore_ascii_case("localhost")
        {
            "0.0.0.0"
        } else {
            net_interface
        };

        let port_actual = if port == PORT_ANY { 0u16 } else { port as u16 };

        // Use socket2 to create socket with advanced options
        let socket = match Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP)) {
            Ok(s) => s,
            Err(e) => {
                com_printf(&format!("WARNING: TCP_OpenSocket: socket: {}\n", e));
                return None;
            }
        };

        // Set IP_TOS to IPTOS_LOWDELAY for reduced latency
        if let Err(e) = socket.set_tos(IPTOS_LOWDELAY) {
            com_printf(&format!("WARNING: TCP_OpenSocket: set_tos: {}\n", e));
        }

        // Allow address reuse for faster restarts
        if let Err(e) = socket.set_reuse_address(true) {
            com_printf(&format!("WARNING: TCP_OpenSocket: set_reuse_address: {}\n", e));
        }

        // Parse and bind to address
        let addr: std::net::SocketAddrV4 = format!("{}:{}", bind_addr, port_actual)
            .parse()
            .ok()?;
        let addr = socket2::SockAddr::from(addr);

        if let Err(e) = socket.bind(&addr) {
            com_printf(&format!("WARNING: TCP_OpenSocket: bind: {}\n", e));
            return None;
        }

        // Listen for connections
        if let Err(e) = socket.listen(8) {
            com_printf(&format!("WARNING: TCP_OpenSocket: listen: {}\n", e));
            return None;
        }

        // Set non-blocking mode
        if let Err(e) = socket.set_nonblocking(true) {
            com_printf(&format!("WARNING: TCP_OpenSocket: set_nonblocking: {}\n", e));
            return None;
        }

        // Convert socket2::Socket to std::net::TcpListener
        Some(socket.into())
    }

    /// Configure a TCP stream with optimal options for game networking:
    /// - TCP_NODELAY: Disable Nagle's algorithm for lower latency
    /// - TCP Keepalive: Detect dead connections
    /// - ToS Low Delay: Hint to routers for priority
    fn configure_tcp_stream(stream: &TcpStream) {
        // Convert to socket2::Socket for advanced configuration
        // We need to clone the underlying socket to avoid consuming the TcpStream
        let socket = Socket::from(stream.try_clone().unwrap());

        // TCP_NODELAY: Disable Nagle's algorithm
        // This sends packets immediately instead of buffering small writes
        if let Err(e) = socket.set_nodelay(true) {
            com_printf(&format!("WARNING: TCP set_nodelay: {}\n", e));
        }

        // IP_TOS: Set low-delay type of service
        if let Err(e) = socket.set_tos(IPTOS_LOWDELAY) {
            com_printf(&format!("WARNING: TCP set_tos: {}\n", e));
        }

        // TCP Keepalive: Detect dead connections
        let keepalive = TcpKeepalive::new()
            .with_time(Duration::from_secs(TCP_KEEPALIVE_SECS));

        // On platforms that support it, also set the interval
        #[cfg(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "ios",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "windows",
        ))]
        let keepalive = keepalive.with_interval(Duration::from_secs(TCP_KEEPALIVE_SECS));

        if let Err(e) = socket.set_tcp_keepalive(&keepalive) {
            com_printf(&format!("WARNING: TCP set_keepalive: {}\n", e));
        }

        // Note: socket is dropped here but the underlying fd is NOT closed
        // because Socket::from() for references doesn't take ownership
        std::mem::forget(socket);
    }

    /// Create an outgoing TCP connection with optimal options configured.
    fn connect_tcp_stream(to: &NetAdr) -> io::Result<TcpStream> {
        // Create socket with socket2 for advanced configuration
        let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?;

        // Set IP_TOS to IPTOS_LOWDELAY for reduced latency
        if let Err(e) = socket.set_tos(IPTOS_LOWDELAY) {
            com_printf(&format!("WARNING: TCP connect set_tos: {}\n", e));
        }

        // TCP_NODELAY: Disable Nagle's algorithm before connect
        if let Err(e) = socket.set_nodelay(true) {
            com_printf(&format!("WARNING: TCP connect set_nodelay: {}\n", e));
        }

        // TCP Keepalive
        let keepalive = TcpKeepalive::new()
            .with_time(Duration::from_secs(TCP_KEEPALIVE_SECS));

        #[cfg(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "ios",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "windows",
        ))]
        let keepalive = keepalive.with_interval(Duration::from_secs(TCP_KEEPALIVE_SECS));

        if let Err(e) = socket.set_tcp_keepalive(&keepalive) {
            com_printf(&format!("WARNING: TCP connect set_keepalive: {}\n", e));
        }

        // Connect to remote address
        let addr = netadr_to_socket_addr(to);
        let sock_addr = socket2::SockAddr::from(addr);
        socket.connect(&sock_addr)?;

        // Set non-blocking after connect
        socket.set_nonblocking(true)?;

        // Convert to std::net::TcpStream
        Ok(socket.into())
    }

    fn net_open_ip(&mut self, ip_str: &str, port_server: i32, port_client: i32, dedicated: bool) {
        // Open server listener and start I/O thread
        if self.ip_listeners[NetSrc::Server as usize].is_none() {
            let port = if port_server != 0 {
                port_server
            } else {
                PORT_SERVER
            };
            self.ip_listeners[NetSrc::Server as usize] = Self::net_ip_socket(ip_str, port);
            if self.ip_listeners[NetSrc::Server as usize].is_none() && dedicated {
                panic!("Couldn't allocate dedicated server TCP port");
            }
            // Start I/O thread for server listener
            if self.ip_listeners[NetSrc::Server as usize].is_some() {
                self.start_io_thread(NetSrc::Server);
            }
        }

        if dedicated {
            return;
        }

        // Open client listener and start I/O thread
        if self.ip_listeners[NetSrc::Client as usize].is_none() {
            let port = if port_client != 0 {
                port_client
            } else {
                PORT_CLIENT
            };
            self.ip_listeners[NetSrc::Client as usize] = Self::net_ip_socket(ip_str, port);
            if self.ip_listeners[NetSrc::Client as usize].is_none() {
                self.ip_listeners[NetSrc::Client as usize] =
                    Self::net_ip_socket(ip_str, PORT_ANY);
            }
            // Start I/O thread for client listener
            if self.ip_listeners[NetSrc::Client as usize].is_some() {
                self.start_io_thread(NetSrc::Client);
            }
        }
    }

    // =========================================================================
    // Config
    // =========================================================================

    pub fn net_config(&mut self, multiplayer: bool) {
        if self.old_config == multiplayer {
            return;
        }
        self.old_config = multiplayer;

        if !multiplayer {
            self.stop_io_threads();
            self.ip_listeners[0] = None;
            self.ip_listeners[1] = None;
            self.ip_streams[0] = None;
            self.ip_streams[1] = None;
        } else if !self.noudp {
            self.net_open_ip("localhost", 0, 0, false);
        }
    }

    /// NET_Sleep — sleeps msec or until net socket is ready (dedicated servers).
    /// With async I/O, this just does a simple sleep.
    pub fn net_sleep(&self, msec: i32) {
        let dedicated = myq2_common::cvar::cvar_variable_value("dedicated");
        if dedicated == 0.0 {
            return;
        }

        if msec > 0 {
            std::thread::sleep(std::time::Duration::from_millis(msec as u64));
        }
    }

    pub fn net_init(&mut self) {
        com_printf("Network initialized (async TCP I/O).\n");
        self.noudp = false;
        self.net_shownet = 0.0;
    }

    pub fn net_shutdown(&mut self) {
        self.stop_io_threads();
        self.net_config(false);
    }
}

// netadr_to_socket_addr and socket_addr_to_netadr are imported from net_common

#[cfg(test)]
mod tests {
    use super::*;
    use myq2_common::qcommon::{NetAdr, NetAdrType, SizeBuf, MAX_MSGLEN};

    // -------------------------------------------------------
    // NetTcpState default values
    // -------------------------------------------------------

    #[test]
    fn test_net_tcp_state_default() {
        let state = NetTcpState::default();
        assert!(state.ip_listeners[0].is_none());
        assert!(state.ip_listeners[1].is_none());
        assert!(state.ip_streams[0].is_none());
        assert!(state.ip_streams[1].is_none());
        assert!(!state.old_config);
        assert!((state.net_shownet - 0.0).abs() < 1e-6);
        assert!(!state.noudp);
    }

    #[test]
    fn test_net_tcp_state_initial_queue_empty() {
        let state = NetTcpState::default();
        assert_eq!(state.queued_packet_count(), 0);
    }

    // -------------------------------------------------------
    // TCP Loopback send/receive
    // -------------------------------------------------------

    #[test]
    fn test_tcp_loopback_send_and_receive() {
        let mut state = NetTcpState::default();

        let data = b"tcp loopback test";
        state.net_send_loop_packet(NetSrc::Client, data);

        let mut net_from = NetAdr::default();
        let mut net_message = SizeBuf::new(MAX_MSGLEN as i32);

        let got = state.net_get_loop_packet(NetSrc::Server, &mut net_from, &mut net_message);
        assert!(got);
        assert_eq!(net_from.adr_type, NetAdrType::Loopback);
        assert_eq!(net_message.cursize, data.len() as i32);
        assert_eq!(&net_message.data[..data.len()], data);
    }

    #[test]
    fn test_tcp_loopback_empty_receive_returns_false() {
        let mut state = NetTcpState::default();
        let mut net_from = NetAdr::default();
        let mut net_message = SizeBuf::new(MAX_MSGLEN as i32);

        let got = state.net_get_loop_packet(NetSrc::Client, &mut net_from, &mut net_message);
        assert!(!got);
    }

    #[test]
    fn test_tcp_loopback_multiple_packets() {
        let mut state = NetTcpState::default();

        state.net_send_loop_packet(NetSrc::Server, b"alpha");
        state.net_send_loop_packet(NetSrc::Server, b"beta");

        // Client receives (server sends to idx^1 = 0 = Client)
        let mut net_from = NetAdr::default();
        let mut net_message = SizeBuf::new(MAX_MSGLEN as i32);
        assert!(state.net_get_loop_packet(NetSrc::Client, &mut net_from, &mut net_message));
        assert_eq!(&net_message.data[..5], b"alpha");

        let mut net_from = NetAdr::default();
        let mut net_message = SizeBuf::new(MAX_MSGLEN as i32);
        assert!(state.net_get_loop_packet(NetSrc::Client, &mut net_from, &mut net_message));
        assert_eq!(&net_message.data[..4], b"beta");

        // No more
        let mut net_from = NetAdr::default();
        let mut net_message = SizeBuf::new(MAX_MSGLEN as i32);
        assert!(!state.net_get_loop_packet(NetSrc::Client, &mut net_from, &mut net_message));
    }

    #[test]
    fn test_tcp_loopback_wrap_around() {
        let mut state = NetTcpState::default();

        // Overflow the circular buffer
        for i in 0..=MAX_LOOPBACK {
            let data = format!("msg{}", i);
            state.net_send_loop_packet(NetSrc::Client, data.as_bytes());
        }

        let mut received = Vec::new();
        loop {
            let mut net_from = NetAdr::default();
            let mut net_message = SizeBuf::new(MAX_MSGLEN as i32);
            if !state.net_get_loop_packet(NetSrc::Server, &mut net_from, &mut net_message) {
                break;
            }
            let s = std::str::from_utf8(&net_message.data[..net_message.cursize as usize])
                .unwrap()
                .to_string();
            received.push(s);
        }

        assert_eq!(received.len(), MAX_LOOPBACK);
        // Oldest packet should be dropped
        assert!(!received.contains(&"msg0".to_string()));
    }

    #[test]
    fn test_tcp_loopback_data_truncated_to_max_msglen() {
        let mut state = NetTcpState::default();

        let oversized = vec![0xFFu8; MAX_MSGLEN + 200];
        state.net_send_loop_packet(NetSrc::Client, &oversized);

        let mut net_from = NetAdr::default();
        let mut net_message = SizeBuf::new(MAX_MSGLEN as i32);
        let got = state.net_get_loop_packet(NetSrc::Server, &mut net_from, &mut net_message);
        assert!(got);
        assert_eq!(net_message.cursize, MAX_MSGLEN as i32);
    }

    #[test]
    fn test_tcp_loopback_bidirectional() {
        let mut state = NetTcpState::default();

        state.net_send_loop_packet(NetSrc::Client, b"c2s");
        state.net_send_loop_packet(NetSrc::Server, b"s2c");

        // Server receives from client
        let mut net_from = NetAdr::default();
        let mut net_message = SizeBuf::new(MAX_MSGLEN as i32);
        assert!(state.net_get_loop_packet(NetSrc::Server, &mut net_from, &mut net_message));
        assert_eq!(&net_message.data[..3], b"c2s");

        // Client receives from server
        let mut net_from = NetAdr::default();
        let mut net_message = SizeBuf::new(MAX_MSGLEN as i32);
        assert!(state.net_get_loop_packet(NetSrc::Client, &mut net_from, &mut net_message));
        assert_eq!(&net_message.data[..3], b"s2c");
    }

    // -------------------------------------------------------
    // TCP constants
    // -------------------------------------------------------

    #[test]
    fn test_tcp_keepalive_secs() {
        assert_eq!(TCP_KEEPALIVE_SECS, 60);
    }
}
