// net.rs -- Global network dispatch functions
//
// In the C original, NET_GetPacket() and NET_SendPacket() are global functions
// that any module can call. In Rust, we use function pointers registered at
// startup by the platform layer (myq2-sys) to avoid circular dependencies.

use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::{Mutex, OnceLock};

use crate::qcommon::{NetAdr, NetAdrType, NetSrc, SizeBuf};

// =============================================================================
// Address utility functions (pure logic, no sockets)
// =============================================================================

/// Compare two net addresses including port.
pub fn net_compare_adr(a: &NetAdr, b: &NetAdr) -> bool {
    if a.adr_type != b.adr_type {
        return false;
    }

    match a.adr_type {
        NetAdrType::Loopback => true,
        NetAdrType::Ip | NetAdrType::Broadcast => a.ip == b.ip && a.port == b.port,
        NetAdrType::Ip6 | NetAdrType::Broadcast6 => {
            a.ip6 == b.ip6 && a.port == b.port && a.scope_id == b.scope_id
        }
    }
}

/// Compare two net addresses ignoring port.
pub fn net_compare_base_adr(a: &NetAdr, b: &NetAdr) -> bool {
    if a.adr_type != b.adr_type {
        return false;
    }

    match a.adr_type {
        NetAdrType::Loopback => true,
        NetAdrType::Ip | NetAdrType::Broadcast => a.ip == b.ip,
        NetAdrType::Ip6 | NetAdrType::Broadcast6 => a.ip6 == b.ip6 && a.scope_id == b.scope_id,
    }
}

/// Convert a NetAdr to a human-readable string.
pub fn net_adr_to_string(a: &NetAdr) -> String {
    match a.adr_type {
        NetAdrType::Loopback => "loopback".to_string(),
        NetAdrType::Ip | NetAdrType::Broadcast => {
            format!(
                "{}.{}.{}.{}:{}",
                a.ip[0],
                a.ip[1],
                a.ip[2],
                a.ip[3],
                u16::from_be(a.port)
            )
        }
        NetAdrType::Ip6 | NetAdrType::Broadcast6 => {
            // Format IPv6 address with brackets (RFC 2732 style)
            let port = u16::from_be(a.port);
            let ip6_str = format_ipv6(&a.ip6);
            if a.scope_id != 0 {
                format!("[{}%{}]:{}", ip6_str, a.scope_id, port)
            } else {
                format!("[{}]:{}", ip6_str, port)
            }
        }
    }
}

/// Format an IPv6 address as a string (simplified, not compressed).
fn format_ipv6(ip6: &[u8; 16]) -> String {
    // Convert to 8 u16 groups
    let groups: Vec<u16> = (0..8)
        .map(|i| u16::from_be_bytes([ip6[i * 2], ip6[i * 2 + 1]]))
        .collect();

    // Simple formatting without zero compression for now
    groups
        .iter()
        .map(|g| format!("{:x}", g))
        .collect::<Vec<_>>()
        .join(":")
}

/// mattx86: NET_IsLocalAdr -- checks if address is a private/local IP.
pub fn net_is_local_adr(a: &NetAdr) -> bool {
    match a.adr_type {
        NetAdrType::Loopback => true,
        NetAdrType::Ip | NetAdrType::Broadcast => {
            // IPv4 private/local ranges
            a.ip[0] == 127
                || (a.ip[0] == 192 && a.ip[1] == 168)
                || (a.ip[0] == 172 && (16..=31).contains(&a.ip[1]))
                || a.ip[0] == 10
        }
        NetAdrType::Ip6 | NetAdrType::Broadcast6 => {
            // IPv6 loopback (::1)
            if a.ip6 == [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1] {
                return true;
            }
            // IPv6 link-local (fe80::/10)
            if a.ip6[0] == 0xfe && (a.ip6[1] & 0xc0) == 0x80 {
                return true;
            }
            // IPv6 unique local (fc00::/7)
            if (a.ip6[0] & 0xfe) == 0xfc {
                return true;
            }
            false
        }
    }
}

/// Returns true if the address is a loopback address.
pub fn net_is_local_address(adr: &NetAdr) -> bool {
    adr.adr_type == NetAdrType::Loopback
}

/// Parse a string into a NetAdr.
///
/// Supports:
/// - "localhost" -> Loopback
/// - "1.2.3.4" or "1.2.3.4:27910" -> IPv4
/// - "[::1]" or "[::1]:27910" -> IPv6
/// - "[fe80::1%eth0]:27910" -> IPv6 with scope ID
/// - "hostname" or "hostname:27910" -> DNS resolution (IPv4 or IPv6)
pub fn net_string_to_adr(s: &str) -> Option<NetAdr> {
    if s == "localhost" {
        return Some(NetAdr {
            adr_type: NetAdrType::Loopback,
            ip: [0; 4],
            ip6: [0; 16],
            scope_id: 0,
            port: 0,
        });
    }

    // Check for IPv6 bracket notation: [address]:port or [address]
    if s.starts_with('[') {
        return parse_ipv6_bracketed(s);
    }

    // IPv4 address or hostname: "host" or "host:port"
    let (host, port) = if let Some(colon_pos) = s.rfind(':') {
        // Make sure this isn't an IPv6 address without brackets
        if s.matches(':').count() > 1 {
            // Multiple colons but no brackets - try as bare IPv6
            return parse_bare_ipv6(s);
        }
        let port_str = &s[colon_pos + 1..];
        let port: u16 = port_str.parse().ok()?;
        (&s[..colon_pos], port.to_be())
    } else {
        (s, 0u16)
    };

    // Try to resolve hostname (supports both IPv4 and IPv6)
    let addr_str = format!("{}:0", host);
    if let Ok(mut addrs) = addr_str.to_socket_addrs() {
        if let Some(addr) = addrs.next() {
            match addr {
                SocketAddr::V4(v4) => {
                    return Some(NetAdr {
                        adr_type: NetAdrType::Ip,
                        ip: v4.ip().octets(),
                        ip6: [0; 16],
                        scope_id: 0,
                        port,
                    });
                }
                SocketAddr::V6(v6) => {
                    return Some(NetAdr {
                        adr_type: NetAdrType::Ip6,
                        ip: [0; 4],
                        ip6: v6.ip().octets(),
                        scope_id: v6.scope_id(),
                        port,
                    });
                }
            }
        }
    }

    None
}

/// Parse an IPv6 address in bracket notation: [address]:port or [address%scope]:port
fn parse_ipv6_bracketed(s: &str) -> Option<NetAdr> {
    // Find the closing bracket
    let close_bracket = s.find(']')?;
    let addr_part = &s[1..close_bracket]; // Skip opening bracket

    // Check for scope ID (e.g., fe80::1%eth0)
    let (addr_str, scope_id) = if let Some(percent_pos) = addr_part.find('%') {
        let scope_str = &addr_part[percent_pos + 1..];
        // Scope ID can be numeric or interface name
        let scope: u32 = scope_str.parse().unwrap_or(0);
        (&addr_part[..percent_pos], scope)
    } else {
        (addr_part, 0u32)
    };

    // Parse the port if present
    let port = if close_bracket + 1 < s.len() && s.as_bytes()[close_bracket + 1] == b':' {
        let port_str = &s[close_bracket + 2..];
        port_str.parse::<u16>().ok()?.to_be()
    } else {
        0u16
    };

    // Parse the IPv6 address
    let ip6 = parse_ipv6_octets(addr_str)?;

    Some(NetAdr {
        adr_type: NetAdrType::Ip6,
        ip: [0; 4],
        ip6,
        scope_id,
        port,
    })
}

/// Parse a bare IPv6 address without brackets (no port support)
fn parse_bare_ipv6(s: &str) -> Option<NetAdr> {
    // Check for scope ID
    let (addr_str, scope_id) = if let Some(percent_pos) = s.find('%') {
        let scope_str = &s[percent_pos + 1..];
        let scope: u32 = scope_str.parse().unwrap_or(0);
        (&s[..percent_pos], scope)
    } else {
        (s, 0u32)
    };

    let ip6 = parse_ipv6_octets(addr_str)?;

    Some(NetAdr {
        adr_type: NetAdrType::Ip6,
        ip: [0; 4],
        ip6,
        scope_id,
        port: 0,
    })
}

/// Parse an IPv6 address string into 16 bytes
fn parse_ipv6_octets(s: &str) -> Option<[u8; 16]> {
    // Use std's IPv6 parsing
    use std::net::Ipv6Addr;
    let addr: Ipv6Addr = s.parse().ok()?;
    Some(addr.octets())
}

/// Function signature for NET_GetPacket.
pub type NetGetPacketFn = fn(NetSrc, &mut NetAdr, &mut SizeBuf) -> bool;

/// Function signature for NET_SendPacket.
pub type NetSendPacketFn = fn(NetSrc, &[u8], &NetAdr);

struct NetDispatch {
    get_packet: Option<NetGetPacketFn>,
    send_packet: Option<NetSendPacketFn>,
}

static NET_DISPATCH: OnceLock<Mutex<NetDispatch>> = OnceLock::new();

fn dispatch() -> &'static Mutex<NetDispatch> {
    NET_DISPATCH.get_or_init(|| {
        Mutex::new(NetDispatch {
            get_packet: None,
            send_packet: None,
        })
    })
}

/// Register the platform's NET_GetPacket implementation.
pub fn net_register_get_packet(f: NetGetPacketFn) {
    dispatch().lock().unwrap().get_packet = Some(f);
}

/// Register the platform's NET_SendPacket implementation.
pub fn net_register_send_packet(f: NetSendPacketFn) {
    dispatch().lock().unwrap().send_packet = Some(f);
}

/// NET_GetPacket -- receive the next packet from the network.
///
/// Calls the platform-registered implementation. Returns false if no
/// implementation is registered or no packet is available.
pub fn net_get_packet(sock: NetSrc, from: &mut NetAdr, message: &mut SizeBuf) -> bool {
    let guard = dispatch().lock().unwrap();
    if let Some(f) = guard.get_packet {
        f(sock, from, message)
    } else {
        false
    }
}

/// NET_SendPacket -- send a packet to the given address.
///
/// Calls the platform-registered implementation. Does nothing if no
/// implementation is registered.
pub fn net_send_packet(sock: NetSrc, data: &[u8], to: &NetAdr) {
    let guard = dispatch().lock().unwrap();
    if let Some(f) = guard.send_packet {
        f(sock, data, to);
    }
}

/// NET_Config — Open or close network sockets based on multiplayer mode.
///
/// When `multiplayer` is true, opens server and client sockets if not already open.
/// When false, closes all sockets. In the C original this was in win32/net_udp.c.
/// The actual socket management is handled by the platform layer; this is the
/// common dispatch point.
pub fn net_config(_multiplayer: bool) {
    // The platform layer (myq2-sys) manages socket lifetime.
    // When fully integrated, this would call a registered callback similar to
    // net_register_get_packet/net_register_send_packet. For now, socket
    // management happens at startup/shutdown in the platform layer.
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qcommon::{NetAdr, NetAdrType};

    /// Helper: create an IPv4 NetAdr.
    fn make_ipv4(ip: [u8; 4], port: u16) -> NetAdr {
        NetAdr {
            adr_type: NetAdrType::Ip,
            ip,
            ip6: [0; 16],
            scope_id: 0,
            port: port.to_be(),
        }
    }

    /// Helper: create a loopback NetAdr.
    fn make_loopback() -> NetAdr {
        NetAdr {
            adr_type: NetAdrType::Loopback,
            ip: [0; 4],
            ip6: [0; 16],
            scope_id: 0,
            port: 0,
        }
    }

    /// Helper: create an IPv6 NetAdr.
    fn make_ipv6(ip6: [u8; 16], port: u16, scope_id: u32) -> NetAdr {
        NetAdr {
            adr_type: NetAdrType::Ip6,
            ip: [0; 4],
            ip6,
            scope_id,
            port: port.to_be(),
        }
    }

    // =========================================================================
    // net_compare_adr
    // =========================================================================

    #[test]
    fn compare_adr_same_ipv4() {
        let a = make_ipv4([192, 168, 1, 1], 27910);
        let b = make_ipv4([192, 168, 1, 1], 27910);
        assert!(net_compare_adr(&a, &b));
    }

    #[test]
    fn compare_adr_different_ip() {
        let a = make_ipv4([192, 168, 1, 1], 27910);
        let b = make_ipv4([192, 168, 1, 2], 27910);
        assert!(!net_compare_adr(&a, &b));
    }

    #[test]
    fn compare_adr_different_port() {
        let a = make_ipv4([10, 0, 0, 1], 27910);
        let b = make_ipv4([10, 0, 0, 1], 27911);
        assert!(!net_compare_adr(&a, &b));
    }

    #[test]
    fn compare_adr_different_types() {
        let a = make_loopback();
        let b = make_ipv4([127, 0, 0, 1], 0);
        assert!(!net_compare_adr(&a, &b));
    }

    #[test]
    fn compare_adr_loopback_always_equal() {
        let a = make_loopback();
        let b = make_loopback();
        assert!(net_compare_adr(&a, &b));
    }

    #[test]
    fn compare_adr_ipv6_same() {
        let ip6 = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]; // ::1
        let a = make_ipv6(ip6, 27910, 0);
        let b = make_ipv6(ip6, 27910, 0);
        assert!(net_compare_adr(&a, &b));
    }

    #[test]
    fn compare_adr_ipv6_different_scope() {
        let ip6 = [0xfe, 0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
        let a = make_ipv6(ip6, 27910, 1);
        let b = make_ipv6(ip6, 27910, 2);
        assert!(!net_compare_adr(&a, &b));
    }

    // =========================================================================
    // net_compare_base_adr
    // =========================================================================

    #[test]
    fn compare_base_adr_ignores_port() {
        let a = make_ipv4([192, 168, 1, 1], 27910);
        let b = make_ipv4([192, 168, 1, 1], 12345);
        assert!(net_compare_base_adr(&a, &b));
    }

    #[test]
    fn compare_base_adr_different_ip() {
        let a = make_ipv4([192, 168, 1, 1], 27910);
        let b = make_ipv4([192, 168, 1, 2], 27910);
        assert!(!net_compare_base_adr(&a, &b));
    }

    #[test]
    fn compare_base_adr_different_types() {
        let a = make_loopback();
        let b = make_ipv4([127, 0, 0, 1], 0);
        assert!(!net_compare_base_adr(&a, &b));
    }

    #[test]
    fn compare_base_adr_loopback() {
        let a = make_loopback();
        let b = make_loopback();
        assert!(net_compare_base_adr(&a, &b));
    }

    #[test]
    fn compare_base_adr_ipv6_ignores_port() {
        let ip6 = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
        let a = make_ipv6(ip6, 27910, 0);
        let b = make_ipv6(ip6, 0, 0);
        assert!(net_compare_base_adr(&a, &b));
    }

    #[test]
    fn compare_base_adr_ipv6_different_scope() {
        let ip6 = [0xfe, 0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
        let a = make_ipv6(ip6, 27910, 1);
        let b = make_ipv6(ip6, 27910, 2);
        assert!(!net_compare_base_adr(&a, &b));
    }

    // =========================================================================
    // net_adr_to_string
    // =========================================================================

    #[test]
    fn adr_to_string_loopback() {
        let a = make_loopback();
        assert_eq!(net_adr_to_string(&a), "loopback");
    }

    #[test]
    fn adr_to_string_ipv4() {
        let a = make_ipv4([192, 168, 1, 100], 27910);
        assert_eq!(net_adr_to_string(&a), "192.168.1.100:27910");
    }

    #[test]
    fn adr_to_string_ipv4_zero_port() {
        let a = make_ipv4([10, 0, 0, 1], 0);
        assert_eq!(net_adr_to_string(&a), "10.0.0.1:0");
    }

    #[test]
    fn adr_to_string_ipv6() {
        let ip6 = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]; // ::1
        let a = make_ipv6(ip6, 27910, 0);
        assert_eq!(net_adr_to_string(&a), "[0:0:0:0:0:0:0:1]:27910");
    }

    #[test]
    fn adr_to_string_ipv6_with_scope() {
        let ip6 = [0xfe, 0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
        let a = make_ipv6(ip6, 27910, 5);
        assert_eq!(net_adr_to_string(&a), "[fe80:0:0:0:0:0:0:1%5]:27910");
    }

    // =========================================================================
    // net_is_local_adr
    // =========================================================================

    #[test]
    fn is_local_adr_loopback() {
        let a = make_loopback();
        assert!(net_is_local_adr(&a));
    }

    #[test]
    fn is_local_adr_127_x() {
        let a = make_ipv4([127, 0, 0, 1], 0);
        assert!(net_is_local_adr(&a));
    }

    #[test]
    fn is_local_adr_127_other() {
        let a = make_ipv4([127, 1, 2, 3], 0);
        assert!(net_is_local_adr(&a));
    }

    #[test]
    fn is_local_adr_192_168() {
        let a = make_ipv4([192, 168, 0, 1], 0);
        assert!(net_is_local_adr(&a));
    }

    #[test]
    fn is_local_adr_172_16() {
        let a = make_ipv4([172, 16, 0, 1], 0);
        assert!(net_is_local_adr(&a));
    }

    #[test]
    fn is_local_adr_172_31() {
        let a = make_ipv4([172, 31, 255, 255], 0);
        assert!(net_is_local_adr(&a));
    }

    #[test]
    fn is_local_adr_172_32_not_local() {
        let a = make_ipv4([172, 32, 0, 1], 0);
        assert!(!net_is_local_adr(&a));
    }

    #[test]
    fn is_local_adr_10_x() {
        let a = make_ipv4([10, 0, 0, 1], 0);
        assert!(net_is_local_adr(&a));
    }

    #[test]
    fn is_local_adr_public_ip() {
        let a = make_ipv4([8, 8, 8, 8], 0);
        assert!(!net_is_local_adr(&a));
    }

    #[test]
    fn is_local_adr_another_public() {
        let a = make_ipv4([203, 0, 113, 1], 0);
        assert!(!net_is_local_adr(&a));
    }

    #[test]
    fn is_local_adr_ipv6_loopback() {
        let ip6 = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]; // ::1
        let a = make_ipv6(ip6, 0, 0);
        assert!(net_is_local_adr(&a));
    }

    #[test]
    fn is_local_adr_ipv6_link_local() {
        let ip6 = [0xfe, 0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
        let a = make_ipv6(ip6, 0, 0);
        assert!(net_is_local_adr(&a));
    }

    #[test]
    fn is_local_adr_ipv6_unique_local_fc() {
        let ip6 = [0xfc, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
        let a = make_ipv6(ip6, 0, 0);
        assert!(net_is_local_adr(&a));
    }

    #[test]
    fn is_local_adr_ipv6_unique_local_fd() {
        let ip6 = [0xfd, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
        let a = make_ipv6(ip6, 0, 0);
        assert!(net_is_local_adr(&a));
    }

    #[test]
    fn is_local_adr_ipv6_public() {
        // 2001:db8::1 (documentation range, but not local)
        let ip6 = [0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
        let a = make_ipv6(ip6, 0, 0);
        assert!(!net_is_local_adr(&a));
    }

    // =========================================================================
    // net_is_local_address
    // =========================================================================

    #[test]
    fn is_local_address_loopback_true() {
        let a = make_loopback();
        assert!(net_is_local_address(&a));
    }

    #[test]
    fn is_local_address_ipv4_false() {
        let a = make_ipv4([127, 0, 0, 1], 0);
        assert!(!net_is_local_address(&a));
    }

    #[test]
    fn is_local_address_ipv6_false() {
        let ip6 = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
        let a = make_ipv6(ip6, 0, 0);
        assert!(!net_is_local_address(&a));
    }

    // =========================================================================
    // net_string_to_adr
    // =========================================================================

    #[test]
    fn string_to_adr_localhost() {
        let adr = net_string_to_adr("localhost").unwrap();
        assert_eq!(adr.adr_type, NetAdrType::Loopback);
        assert_eq!(adr.port, 0);
    }

    #[test]
    fn string_to_adr_ipv4_no_port() {
        let adr = net_string_to_adr("127.0.0.1").unwrap();
        assert_eq!(adr.adr_type, NetAdrType::Ip);
        assert_eq!(adr.ip, [127, 0, 0, 1]);
        assert_eq!(adr.port, 0);
    }

    #[test]
    fn string_to_adr_ipv4_with_port() {
        let adr = net_string_to_adr("127.0.0.1:27910").unwrap();
        assert_eq!(adr.adr_type, NetAdrType::Ip);
        assert_eq!(adr.ip, [127, 0, 0, 1]);
        assert_eq!(u16::from_be(adr.port), 27910);
    }

    #[test]
    fn string_to_adr_ipv6_bracketed() {
        let adr = net_string_to_adr("[::1]").unwrap();
        assert_eq!(adr.adr_type, NetAdrType::Ip6);
        assert_eq!(
            adr.ip6,
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]
        );
        assert_eq!(adr.port, 0);
    }

    #[test]
    fn string_to_adr_ipv6_bracketed_with_port() {
        let adr = net_string_to_adr("[::1]:27910").unwrap();
        assert_eq!(adr.adr_type, NetAdrType::Ip6);
        assert_eq!(
            adr.ip6,
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]
        );
        assert_eq!(u16::from_be(adr.port), 27910);
    }

    #[test]
    fn string_to_adr_invalid_garbage() {
        assert!(net_string_to_adr("not_an_address_at_all!!!").is_none());
    }

    #[test]
    fn string_to_adr_invalid_bad_port() {
        // Port is not a valid number
        assert!(net_string_to_adr("127.0.0.1:notaport").is_none());
    }

    #[test]
    fn string_to_adr_invalid_brackets_no_addr() {
        // Empty brackets with no valid IPv6 inside
        assert!(net_string_to_adr("[]").is_none());
    }

    #[test]
    fn string_to_adr_ipv4_specific() {
        let adr = net_string_to_adr("10.20.30.40:8080").unwrap();
        assert_eq!(adr.ip, [10, 20, 30, 40]);
        assert_eq!(u16::from_be(adr.port), 8080);
    }

    #[test]
    fn string_to_adr_ipv6_full() {
        let adr = net_string_to_adr("[fe80::1]:27910").unwrap();
        assert_eq!(adr.adr_type, NetAdrType::Ip6);
        // fe80::1 = fe80:0:0:0:0:0:0:1
        assert_eq!(adr.ip6[0], 0xfe);
        assert_eq!(adr.ip6[1], 0x80);
        assert_eq!(adr.ip6[15], 1);
        assert_eq!(u16::from_be(adr.port), 27910);
    }

    #[test]
    fn string_to_adr_ipv6_scope() {
        let adr = net_string_to_adr("[fe80::1%3]:27910").unwrap();
        assert_eq!(adr.adr_type, NetAdrType::Ip6);
        assert_eq!(adr.scope_id, 3);
        assert_eq!(u16::from_be(adr.port), 27910);
    }
}
