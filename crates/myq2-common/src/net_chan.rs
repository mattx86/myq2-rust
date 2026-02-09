// net_chan.rs — Network channel implementation
// Converted from: myq2-original/qcommon/net_chan.c
//
// Handles reliable and unreliable message delivery over UDP.
// See the original source for the full protocol description.

use crate::common::{
    msg_begin_reading, msg_read_byte, msg_read_long, msg_read_short,
    msg_write_byte, msg_write_long, msg_write_short,
};
use crate::qcommon::{NetAdr, NetChan, NetSrc, SizeBuf, MAX_MSGLEN, PROTOCOL_R1Q2, PROTOCOL_Q2PRO};

// Q2Pro (protocol 36) fragmentation constants
/// Bit 30 of the sequence number indicates a fragmented packet
pub const FRAGMENT_BIT: u32 = 1 << 30;
/// Maximum fragment size (conservative to fit within UDP MTU)
pub const MAX_FRAGMENT_SIZE: usize = 1280;

/// Check if the last reliable message has been acknowledged.
pub fn netchan_can_reliable(chan: &NetChan) -> bool {
    chan.reliable_length == 0
}

/// Determine if we need to send a reliable message.
pub fn netchan_need_reliable(chan: &NetChan) -> bool {
    // If the remote side dropped the last reliable message, resend it
    if chan.incoming_acknowledged > chan.last_reliable_sequence
        && chan.incoming_reliable_acknowledged != chan.reliable_sequence
    {
        return true;
    }

    // If the reliable transmit buffer is empty, copy the current message out
    if chan.reliable_length == 0 && chan.message.cursize > 0 {
        return true;
    }

    false
}

/// Set up a new network channel.
/// The protocol parameter determines features like 1-byte qport (protocol 35+).
pub fn netchan_setup(sock: NetSrc, chan: &mut NetChan, adr: NetAdr, qport: i32, curtime: i32) {
    *chan = NetChan::new();
    chan.sock = sock;
    chan.remote_address = adr;
    chan.qport = qport;
    chan.last_received = curtime;
    chan.incoming_sequence = 0;
    chan.outgoing_sequence = 1;
    chan.message = SizeBuf::new((MAX_MSGLEN - 16) as i32);
    chan.message.allow_overflow = true;
    // Protocol is set separately after negotiation via netchan_set_protocol
}

/// Set the negotiated protocol version for the channel.
/// This affects features like 1-byte qport (protocol 35+).
pub fn netchan_set_protocol(chan: &mut NetChan, protocol: i32) {
    chan.protocol = protocol;
}

/// Build a packet for transmission and send it via NET_SendPacket.
///
/// Handles reliable message retransmission and copies unreliable data
/// if there's room in the packet. Matches the C original which calls
/// NET_SendPacket at the end of Netchan_Transmit.
pub fn netchan_transmit(
    chan: &mut NetChan,
    data: &[u8],
    curtime: i32,
    qport_value: i32,
) {
    netchan_transmit_with_dup(chan, data, curtime, qport_value, 0);
}

/// Build a packet for transmission and send it with optional duplication.
///
/// This is an R1Q2/Q2Pro extension that sends duplicate packets to compensate
/// for packet loss on lossy connections (WiFi, satellite, etc.).
///
/// # Arguments
/// * `chan` - The network channel
/// * `data` - Unreliable data to send
/// * `curtime` - Current time in milliseconds
/// * `qport_value` - Client's qport value
/// * `dup_count` - Number of duplicate packets to send (0-2). The original packet
///                 is always sent; dup_count specifies additional copies.
pub fn netchan_transmit_with_dup(
    chan: &mut NetChan,
    data: &[u8],
    curtime: i32,
    qport_value: i32,
    dup_count: i32,
) {
    // Clamp dup_count to reasonable range
    let dup_count = dup_count.clamp(0, 2);

    // Check for message overflow
    if chan.message.overflowed && !chan.message.allow_overflow {
        panic!("Outgoing message overflow");
    }

    let send_reliable = netchan_need_reliable(chan);

    // If the reliable transmit buffer is empty and we have pending reliable data,
    // move it to the reliable buffer
    if chan.reliable_length == 0 && chan.message.cursize > 0 {
        let cursize = chan.message.cursize as usize;
        chan.reliable_buf[..cursize]
            .copy_from_slice(&chan.message.data[..cursize]);
        chan.reliable_length = chan.message.cursize;
        chan.message.cursize = 0;
        chan.reliable_sequence ^= 1;
    }

    // Build the packet header
    let mut send = SizeBuf::new(MAX_MSGLEN as i32);

    let w1 = ((chan.outgoing_sequence as u32) & !(1u32 << 31))
        | ((send_reliable as u32) << 31);
    let w2 = ((chan.incoming_sequence as u32) & !(1u32 << 31))
        | ((chan.incoming_reliable_sequence as u32) << 31);

    chan.outgoing_sequence += 1;
    chan.last_sent = curtime;

    msg_write_long(&mut send, w1 as i32);
    msg_write_long(&mut send, w2 as i32);

    // Send the qport if we are a client
    // Protocol 35+ uses 1-byte qport for bandwidth savings
    if matches!(chan.sock, NetSrc::Client) {
        if chan.protocol >= PROTOCOL_R1Q2 {
            // R1Q2/Q2Pro: 1-byte qport
            msg_write_byte(&mut send, qport_value & 0xFF);
        } else {
            // Original protocol: 2-byte qport
            msg_write_short(&mut send, qport_value);
        }
    }

    // Copy the reliable message to the packet first
    if send_reliable {
        let reliable_len = chan.reliable_length as usize;
        send.write(&chan.reliable_buf[..reliable_len]);
        chan.last_reliable_sequence = chan.outgoing_sequence;
    }

    // Add the unreliable part if space is available
    let remaining = (send.maxsize - send.cursize) as usize;
    if remaining >= data.len() {
        send.write(data);
    } else {
        crate::common::com_printf("Netchan_Transmit: dumped unreliable\n");
    }

    // Send the datagram
    let cursize = send.cursize as usize;
    let packet_data = &send.data[..cursize];

    // Send original packet
    crate::net::net_send_packet(chan.sock, packet_data, &chan.remote_address);

    // Send duplicate packets with small delays to avoid burst loss
    for _i in 0..dup_count {
        // Small delay between duplicates (50-100 microseconds) to spread across time
        // This helps when packet loss occurs in bursts
        std::thread::sleep(std::time::Duration::from_micros(50));
        crate::net::net_send_packet(chan.sock, packet_data, &chan.remote_address);
    }
}

/// Process an incoming packet. Returns true if the packet is valid and should
/// be processed.
///
/// Modifies the message buffer to point past the header so the caller
/// can read the payload directly.
///
/// For Q2Pro protocol 36, handles fragmented packets by accumulating fragments
/// until the complete message is received.
pub fn netchan_process(chan: &mut NetChan, msg: &mut SizeBuf, curtime: i32) -> bool {
    // Read sequence numbers
    msg_begin_reading(msg);
    let mut sequence = msg_read_long(msg) as u32;
    let mut sequence_ack = msg_read_long(msg) as u32;

    // Read the qport if we are a server
    // Protocol 35+ uses 1-byte qport
    if matches!(chan.sock, NetSrc::Server) {
        if chan.protocol >= PROTOCOL_R1Q2 {
            // R1Q2/Q2Pro: 1-byte qport
            let _qport = msg_read_byte(msg);
        } else {
            // Original protocol: 2-byte qport
            let _qport = msg_read_short(msg);
        }
    }

    let reliable_message = sequence >> 31;
    let reliable_ack = sequence_ack >> 31;

    // Q2Pro (protocol 36) fragmentation check
    let fragmented = if chan.protocol >= PROTOCOL_Q2PRO {
        (sequence & FRAGMENT_BIT) != 0
    } else {
        false
    };

    // Mask off special bits
    sequence &= !(1u32 << 31);
    if chan.protocol >= PROTOCOL_Q2PRO {
        sequence &= !FRAGMENT_BIT;
    }
    sequence_ack &= !(1u32 << 31);

    // Discard stale or duplicated packets
    if (sequence as i32) <= chan.incoming_sequence {
        return false;
    }

    // Calculate dropped packets
    chan.dropped = (sequence as i32) - (chan.incoming_sequence + 1);

    // Handle Q2Pro fragmentation
    if fragmented {
        // Read fragment header: offset (short), length (short)
        let fragment_offset = msg_read_short(msg) as usize;
        let fragment_length = msg_read_short(msg) as usize;

        // Sanity checks
        if fragment_length == 0 || fragment_length > MAX_FRAGMENT_SIZE {
            crate::common::com_dprintf(&format!(
                "Netchan_Process: bad fragment length {}\n",
                fragment_length
            ));
            return false;
        }

        // Check if this is a new fragmented sequence
        if !chan.fragment_in.in_progress || chan.fragment_in.sequence != sequence as i32 {
            // Start a new fragmented message
            chan.fragment_in.reset();
            chan.fragment_in.in_progress = true;
            chan.fragment_in.sequence = sequence as i32;
        }

        // Validate fragment offset
        if fragment_offset != chan.fragment_in.current_offset as usize {
            crate::common::com_dprintf(&format!(
                "Netchan_Process: fragment out of order (expected {}, got {})\n",
                chan.fragment_in.current_offset, fragment_offset
            ));
            chan.fragment_in.reset();
            return false;
        }

        // Read the fragment data
        let data_start = msg.readcount as usize;
        let data_end = data_start + fragment_length;
        if data_end > msg.data.len() {
            crate::common::com_dprintf("Netchan_Process: fragment overflows packet\n");
            chan.fragment_in.reset();
            return false;
        }

        // Append to fragment buffer
        chan.fragment_in.buffer.extend_from_slice(&msg.data[data_start..data_end]);
        chan.fragment_in.current_offset += fragment_length as i32;

        // Check if this is the last fragment (data less than max fragment size)
        if fragment_length < MAX_FRAGMENT_SIZE {
            // Fragment complete - copy to message buffer
            chan.fragment_in.in_progress = false;
            let complete_data = std::mem::take(&mut chan.fragment_in.buffer);

            // Replace msg content with the complete defragmented message
            msg.data.clear();
            msg.data.extend_from_slice(&complete_data);
            msg.cursize = complete_data.len() as i32;
            msg.readcount = 0;

            chan.fragment_in.reset();
        } else {
            // More fragments expected - don't process yet
            return false;
        }
    }

    // If the current outgoing reliable message has been acknowledged,
    // clear the buffer
    if reliable_ack == chan.reliable_sequence as u32 {
        chan.reliable_length = 0;
    }

    // Update sequence tracking
    chan.incoming_sequence = sequence as i32;
    chan.incoming_acknowledged = sequence_ack as i32;
    chan.incoming_reliable_acknowledged = reliable_ack as i32;

    if reliable_message != 0 {
        chan.incoming_reliable_sequence ^= 1;
    }

    chan.last_received = curtime;

    true
}

/// Build an out-of-band packet (sequence = -1) and return its bytes.
pub fn netchan_out_of_band_data(data: &[u8]) -> Vec<u8> {
    let mut send = SizeBuf::new(MAX_MSGLEN as i32);
    msg_write_long(&mut send, -1); // -1 sequence means out of band
    send.write(data);
    let cursize = send.cursize as usize;
    send.data[..cursize].to_vec()
}

/// Build and send an out-of-band datagram.
/// Matches C: Netchan_OutOfBand(net_socket, adr, length, data)
pub fn netchan_out_of_band(sock: NetSrc, adr: &NetAdr, data: &[u8]) {
    let packet = netchan_out_of_band_data(data);
    crate::net::net_send_packet(sock, &packet, adr);
}

/// Build and send an out-of-band text message packet.
/// Matches C: Netchan_OutOfBandPrint(net_socket, adr, format, ...)
pub fn netchan_out_of_band_print(sock: NetSrc, adr: &NetAdr, message: &str) {
    netchan_out_of_band(sock, adr, message.as_bytes());
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qcommon::NetAdrType;

    fn make_test_chan() -> NetChan {
        let mut chan = NetChan::new();
        let adr = NetAdr {
            adr_type: NetAdrType::Ip,
            ip: [127, 0, 0, 1],
            ip6: [0; 16],
            scope_id: 0,
            port: 27910,
        };
        netchan_setup(NetSrc::Client, &mut chan, adr, 12345, 0);
        chan
    }

    #[test]
    fn test_can_reliable_empty() {
        let chan = make_test_chan();
        assert!(netchan_can_reliable(&chan));
    }

    #[test]
    fn test_need_reliable_empty() {
        let chan = make_test_chan();
        assert!(!netchan_need_reliable(&chan));
    }

    #[test]
    fn test_out_of_band() {
        let packet = netchan_out_of_band_data(b"hello");
        // First 4 bytes should be -1 (0xFFFFFFFF) in little-endian
        assert_eq!(packet[0], 0xFF);
        assert_eq!(packet[1], 0xFF);
        assert_eq!(packet[2], 0xFF);
        assert_eq!(packet[3], 0xFF);
        assert_eq!(&packet[4..], b"hello");
    }

    #[test]
    fn test_transmit_basic() {
        let mut chan = make_test_chan();
        // netchan_transmit now sends via NET_SendPacket internally;
        // without a registered send function it silently drops.
        // We just verify sequencing advances correctly.
        netchan_transmit(&mut chan, b"test", 100, 12345);
        assert_eq!(chan.outgoing_sequence, 2); // incremented
    }
}
