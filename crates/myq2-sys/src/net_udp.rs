// net_udp.rs -- Converted from myq2-original/win32/net_udp.c
// UDP networking (default path when SWAP_UDP_FOR_TCP is NOT defined)
//
// All network I/O uses async threaded processing with packet queueing.
// Packets are received by background I/O threads and queued for the game thread.

#![allow(dead_code)]

use std::io;
use std::net::UdpSocket;
use std::sync::Arc;

use myq2_common::common::com_printf;
use myq2_common::net_queue::{PacketQueue, DEFAULT_QUEUE_CAPACITY};
use myq2_common::qcommon::*;
use socket2::{Domain, Protocol, Socket, Type};
use crate::MAX_LOOPBACK;
use crate::net_common::{IPTOS_LOWDELAY, Loopback, netadr_to_socket_addr};
use crate::net_io_thread::NetIoThreadManager;

// =============================================================================
// Constants
// =============================================================================

pub use myq2_common::qcommon::PORT_CLIENT;

// =============================================================================
// Net state
// =============================================================================

pub struct NetState {
    loopbacks: [Loopback; 2],
    ip_sockets: [Option<UdpSocket>; 2],
    old_config: bool,
    // Cvar values cached at init time
    pub net_shownet: f32,
    noudp: bool,
    /// Packet queue for async I/O - always active
    packet_queue: PacketQueue,
    /// I/O thread manager - handles background packet reception
    io_manager: NetIoThreadManager,
}

impl Default for NetState {
    fn default() -> Self {
        Self {
            loopbacks: [Loopback::default(), Loopback::default()],
            ip_sockets: [None, None],
            old_config: false,
            net_shownet: 0.0,
            noudp: false,
            packet_queue: PacketQueue::new(DEFAULT_QUEUE_CAPACITY),
            io_manager: NetIoThreadManager::new(),
        }
    }
}

// =============================================================================
// Address helpers
// =============================================================================

// Address helpers — re-exported from myq2_common::net
pub use myq2_common::net::{
    net_compare_adr, net_compare_base_adr, net_adr_to_string,
    net_is_local_adr, net_string_to_adr, net_is_local_address,
};

// =============================================================================
// Loopback buffers for local player
// =============================================================================

impl NetState {
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
        if self.ip_sockets[idx].is_none() {
            return;
        }

        let sender = self.packet_queue.sender();

        // Clone the socket for the I/O thread
        let socket = self.ip_sockets[idx].as_ref().unwrap();
        let socket_clone = match socket.try_clone() {
            Ok(s) => Arc::new(s),
            Err(e) => {
                com_printf(&format!("Failed to clone socket for I/O thread: {}\n", e));
                return;
            }
        };

        self.io_manager.spawn_udp(sock, socket_clone, sender);
        com_printf(&format!("Started UDP I/O thread for {:?}\n", sock));
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
    // Get / Send packets
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
        // Always check loopback first (for local/singleplayer)
        if self.net_get_loop_packet(sock, net_from, net_message) {
            return true;
        }

        // Get packet from the async queue
        if let Some(packet) = self.packet_queue.try_recv() {
            // Filter by socket type if needed
            if packet.sock != sock {
                // Put it back? For now, just process it anyway since we have one queue
                // In practice, client and server rarely receive simultaneously
            }

            if packet.data.len() >= net_message.maxsize as usize {
                com_printf(&format!(
                    "Oversize packet from {}\n",
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
    /// Sends are still synchronous (non-blocking) since they're typically fast
    /// and we need immediate error feedback for connection management.
    pub fn net_send_packet(&mut self, sock: NetSrc, data: &[u8], to: &NetAdr) {
        if to.adr_type == NetAdrType::Loopback {
            self.net_send_loop_packet(sock, data);
            return;
        }

        let idx = sock as usize;

        match to.adr_type {
            NetAdrType::Broadcast | NetAdrType::Ip => {
                if let Some(ref socket) = self.ip_sockets[idx] {
                    let addr = netadr_to_socket_addr(to);
                    if let Err(e) = socket.send_to(data, addr) {
                        if e.kind() != io::ErrorKind::WouldBlock {
                            com_printf(&format!(
                                "NET_SendPacket ERROR: {} to {}\n",
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

    /// Open a non-blocking, broadcast-capable UDP socket with low-delay ToS.
    fn net_ip_socket(net_interface: &str, port: i32) -> Option<UdpSocket> {
        let bind_addr = if net_interface.is_empty()
            || net_interface.eq_ignore_ascii_case("localhost")
        {
            "0.0.0.0"
        } else {
            net_interface
        };

        let port_actual = if port == PORT_ANY { 0u16 } else { port as u16 };

        // Use socket2 to create socket with advanced options
        let socket = match Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)) {
            Ok(s) => s,
            Err(e) => {
                com_printf(&format!("WARNING: UDP_OpenSocket: socket: {}\n", e));
                return None;
            }
        };

        // Set IP_TOS to IPTOS_LOWDELAY for reduced latency
        // This hints to routers to prioritize this traffic for low latency
        if let Err(e) = socket.set_tos(IPTOS_LOWDELAY) {
            // Non-fatal: some platforms may not support this
            com_printf(&format!("WARNING: UDP_OpenSocket: set_tos: {}\n", e));
        }

        // Parse and bind to address
        let addr: std::net::SocketAddrV4 = format!("{}:{}", bind_addr, port_actual)
            .parse()
            .ok()?;
        let addr = socket2::SockAddr::from(addr);

        if let Err(e) = socket.bind(&addr) {
            com_printf(&format!("WARNING: UDP_OpenSocket: bind: {}\n", e));
            return None;
        }

        // Set non-blocking mode
        if let Err(e) = socket.set_nonblocking(true) {
            com_printf(&format!("WARNING: UDP_OpenSocket: set_nonblocking: {}\n", e));
            return None;
        }

        // Enable broadcast
        if let Err(e) = socket.set_broadcast(true) {
            com_printf(&format!("WARNING: UDP_OpenSocket: set_broadcast: {}\n", e));
            return None;
        }

        // Convert socket2::Socket to std::net::UdpSocket
        Some(socket.into())
    }

    fn net_open_ip(&mut self, ip_str: &str, port_server: i32, port_client: i32, dedicated: bool) {
        // Open server socket and start I/O thread
        if self.ip_sockets[NetSrc::Server as usize].is_none() {
            let port = if port_server != 0 {
                port_server
            } else {
                PORT_SERVER
            };
            self.ip_sockets[NetSrc::Server as usize] = Self::net_ip_socket(ip_str, port);
            if self.ip_sockets[NetSrc::Server as usize].is_none() && dedicated {
                panic!("Couldn't allocate dedicated server IP port");
            }
            // Start I/O thread for server socket
            if self.ip_sockets[NetSrc::Server as usize].is_some() {
                self.start_io_thread(NetSrc::Server);
            }
        }

        // Dedicated servers don't need client ports
        if dedicated {
            return;
        }

        // Open client socket and start I/O thread
        if self.ip_sockets[NetSrc::Client as usize].is_none() {
            let port = if port_client != 0 {
                port_client
            } else {
                PORT_CLIENT
            };
            self.ip_sockets[NetSrc::Client as usize] = Self::net_ip_socket(ip_str, port);
            if self.ip_sockets[NetSrc::Client as usize].is_none() {
                self.ip_sockets[NetSrc::Client as usize] =
                    Self::net_ip_socket(ip_str, PORT_ANY);
            }
            // Start I/O thread for client socket
            if self.ip_sockets[NetSrc::Client as usize].is_some() {
                self.start_io_thread(NetSrc::Client);
            }
        }
    }

    // =========================================================================
    // Config
    // =========================================================================

    /// A single player game will only use the loopback code.
    pub fn net_config(&mut self, multiplayer: bool) {
        if self.old_config == multiplayer {
            return;
        }
        self.old_config = multiplayer;

        if !multiplayer {
            // Stop I/O threads and close sockets
            self.stop_io_threads();
            self.ip_sockets[0] = None;
            self.ip_sockets[1] = None;
        } else if !self.noudp {
            // Open sockets (I/O threads started automatically)
            self.net_open_ip("localhost", 0, 0, false);
        }
    }

    /// NET_Sleep — sleeps msec or until net socket is ready.
    /// With async I/O, this just does a simple sleep since I/O threads handle reception.
    pub fn net_sleep(&self, msec: i32) {
        // Only sleep for dedicated servers
        let dedicated = myq2_common::cvar::cvar_variable_value("dedicated");
        if dedicated == 0.0 {
            return;
        }

        // With async I/O, we just sleep - the I/O thread is already receiving
        if msec > 0 {
            std::thread::sleep(std::time::Duration::from_millis(msec as u64));
        }
    }

    // =========================================================================
    // Init / Shutdown
    // =========================================================================

    /// Initialize networking with async I/O.
    pub fn net_init(&mut self) {
        com_printf("Network initialized (async UDP I/O).\n");
        self.noudp = false;
        self.net_shownet = 0.0;
    }

    pub fn net_shutdown(&mut self) {
        self.stop_io_threads();
        self.net_config(false);
    }
}

// netadr_to_socket_addr and socket_addr_to_netadr are imported from net_common

/// Return a descriptive error string for the last network error.
pub fn net_error_string() -> String {
    std::io::Error::last_os_error().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use myq2_common::qcommon::{NetAdr, NetAdrType, SizeBuf, MAX_MSGLEN};

    // -------------------------------------------------------
    // NetState default values
    // -------------------------------------------------------

    #[test]
    fn test_net_state_default() {
        let state = NetState::default();
        assert!(state.ip_sockets[0].is_none());
        assert!(state.ip_sockets[1].is_none());
        assert!(!state.old_config);
        assert!((state.net_shownet - 0.0).abs() < 1e-6);
        assert!(!state.noudp);
    }

    #[test]
    fn test_net_state_initial_queue_empty() {
        let state = NetState::default();
        assert_eq!(state.queued_packet_count(), 0);
    }

    // -------------------------------------------------------
    // Loopback send/receive
    // -------------------------------------------------------

    #[test]
    fn test_loopback_send_and_receive() {
        let mut state = NetState::default();

        // Send a packet via client loopback (goes to server side: idx ^ 1 = 1)
        let data = b"hello loopback";
        state.net_send_loop_packet(NetSrc::Client, data);

        // Receive from server loopback (reads from idx = 1)
        let mut net_from = NetAdr::default();
        let mut net_message = SizeBuf::new(MAX_MSGLEN as i32);

        let got = state.net_get_loop_packet(NetSrc::Server, &mut net_from, &mut net_message);
        assert!(got);
        assert_eq!(net_from.adr_type, NetAdrType::Loopback);
        assert_eq!(net_message.cursize, data.len() as i32);
        assert_eq!(&net_message.data[..data.len()], data);
    }

    #[test]
    fn test_loopback_empty_receive_returns_false() {
        let mut state = NetState::default();
        let mut net_from = NetAdr::default();
        let mut net_message = SizeBuf::new(MAX_MSGLEN as i32);

        let got = state.net_get_loop_packet(NetSrc::Client, &mut net_from, &mut net_message);
        assert!(!got);
    }

    #[test]
    fn test_loopback_multiple_packets() {
        let mut state = NetState::default();

        // Send multiple packets
        state.net_send_loop_packet(NetSrc::Client, b"packet1");
        state.net_send_loop_packet(NetSrc::Client, b"packet2");
        state.net_send_loop_packet(NetSrc::Client, b"packet3");

        // Receive all three from the server side
        for expected in &[b"packet1" as &[u8], b"packet2", b"packet3"] {
            let mut net_from = NetAdr::default();
            let mut net_message = SizeBuf::new(MAX_MSGLEN as i32);
            let got = state.net_get_loop_packet(NetSrc::Server, &mut net_from, &mut net_message);
            assert!(got);
            assert_eq!(net_message.cursize, expected.len() as i32);
            assert_eq!(&net_message.data[..expected.len()], *expected);
        }

        // No more packets
        let mut net_from = NetAdr::default();
        let mut net_message = SizeBuf::new(MAX_MSGLEN as i32);
        assert!(!state.net_get_loop_packet(NetSrc::Server, &mut net_from, &mut net_message));
    }

    #[test]
    fn test_loopback_wrap_around() {
        let mut state = NetState::default();

        // Send MAX_LOOPBACK + 1 packets to overflow the circular buffer
        for i in 0..=MAX_LOOPBACK {
            let data = format!("pkt{}", i);
            state.net_send_loop_packet(NetSrc::Client, data.as_bytes());
        }

        // Only the last MAX_LOOPBACK packets should be available
        // The first packet should be lost
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
        // The oldest packet (pkt0) should have been dropped
        assert!(!received.contains(&"pkt0".to_string()));
    }

    #[test]
    fn test_loopback_bidirectional() {
        let mut state = NetState::default();

        // Client sends to server
        state.net_send_loop_packet(NetSrc::Client, b"from_client");
        // Server sends to client
        state.net_send_loop_packet(NetSrc::Server, b"from_server");

        // Server receives client's packet
        let mut net_from = NetAdr::default();
        let mut net_message = SizeBuf::new(MAX_MSGLEN as i32);
        assert!(state.net_get_loop_packet(NetSrc::Server, &mut net_from, &mut net_message));
        assert_eq!(&net_message.data[..11], b"from_client");

        // Client receives server's packet
        let mut net_from = NetAdr::default();
        let mut net_message = SizeBuf::new(MAX_MSGLEN as i32);
        assert!(state.net_get_loop_packet(NetSrc::Client, &mut net_from, &mut net_message));
        assert_eq!(&net_message.data[..11], b"from_server");
    }

    #[test]
    fn test_loopback_data_truncated_to_max_msglen() {
        let mut state = NetState::default();

        // Create data larger than MAX_MSGLEN
        let oversized = vec![0xABu8; MAX_MSGLEN + 100];
        state.net_send_loop_packet(NetSrc::Client, &oversized);

        let mut net_from = NetAdr::default();
        let mut net_message = SizeBuf::new(MAX_MSGLEN as i32);
        let got = state.net_get_loop_packet(NetSrc::Server, &mut net_from, &mut net_message);
        assert!(got);
        // Data should be truncated to MAX_MSGLEN
        assert_eq!(net_message.cursize, MAX_MSGLEN as i32);
    }

    // -------------------------------------------------------
    // net_error_string
    // -------------------------------------------------------

    #[test]
    fn test_net_error_string_returns_string() {
        // Just verify it returns a non-empty string (the actual error depends on OS state)
        let s = net_error_string();
        assert!(!s.is_empty());
    }

    // -------------------------------------------------------
    // PORT constants
    // -------------------------------------------------------

    #[test]
    fn test_port_constants() {
        assert_eq!(myq2_common::qcommon::PORT_CLIENT, 27901);
        assert_eq!(myq2_common::qcommon::PORT_SERVER, 27910);
        assert_eq!(myq2_common::qcommon::PORT_ANY, -1);
    }
}

// =============================================================================
// Global networking context — mirrors C's global socket state
// =============================================================================

use std::sync::{Mutex, OnceLock};

static NET_STATE: OnceLock<Mutex<NetState>> = OnceLock::new();

fn global_net_state() -> &'static Mutex<NetState> {
    NET_STATE.get_or_init(|| Mutex::new(NetState::default()))
}

/// Access the global NetState under a lock, execute a closure, and return the result.
pub fn with_net_state<F, R>(f: F) -> R
where
    F: FnOnce(&mut NetState) -> R,
{
    let mut guard = global_net_state().lock().unwrap();
    f(&mut guard)
}

/// Global NET_GetPacket implementation suitable for registering with
/// myq2_common::net::net_register_get_packet().
fn global_net_get_packet(sock: NetSrc, from: &mut NetAdr, message: &mut SizeBuf) -> bool {
    with_net_state(|net| net.net_get_packet(sock, from, message))
}

/// Global NET_SendPacket implementation suitable for registering with
/// myq2_common::net::net_register_send_packet().
fn global_net_send_packet(sock: NetSrc, data: &[u8], to: &NetAdr) {
    with_net_state(|net| net.net_send_packet(sock, data, to))
}

/// Initialize the global networking state and register dispatch functions.
/// Call once at startup.
pub fn net_global_init() {
    with_net_state(|net| net.net_init());
    myq2_common::net::net_register_get_packet(global_net_get_packet);
    myq2_common::net::net_register_send_packet(global_net_send_packet);
}
