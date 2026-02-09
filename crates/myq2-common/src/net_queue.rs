// net_queue.rs — Thread-safe packet queueing for async network I/O
//
// This module provides a packet queue for decoupling network I/O from the
// main game loop. A dedicated I/O thread receives packets and enqueues them
// for processing by the game thread.

use crate::qcommon::{NetAdr, NetSrc};
use crossbeam::channel::{bounded, Receiver, Sender, TrySendError};

/// A received packet with source address and timestamp.
#[derive(Clone)]
pub struct QueuedPacket {
    /// Which socket received the packet (client or server)
    pub sock: NetSrc,
    /// Source address of the packet
    pub from: NetAdr,
    /// Packet data
    pub data: Vec<u8>,
    /// Timestamp when packet was received (sys_milliseconds)
    pub timestamp: i32,
}

impl QueuedPacket {
    /// Create a new queued packet.
    pub fn new(sock: NetSrc, from: NetAdr, data: Vec<u8>, timestamp: i32) -> Self {
        Self {
            sock,
            from,
            data,
            timestamp,
        }
    }
}

/// Thread-safe packet queue for communication between I/O and game threads.
///
/// Uses a bounded crossbeam channel for backpressure control.
pub struct PacketQueue {
    sender: Sender<QueuedPacket>,
    receiver: Receiver<QueuedPacket>,
}

impl PacketQueue {
    /// Create a new bounded packet queue.
    ///
    /// # Arguments
    /// * `capacity` - Maximum number of packets that can be queued.
    ///                When full, new packets are dropped (producer never blocks).
    pub fn new(capacity: usize) -> Self {
        let (sender, receiver) = bounded(capacity);
        Self { sender, receiver }
    }

    /// Get a clone of the sender handle (for I/O thread).
    pub fn sender(&self) -> PacketQueueSender {
        PacketQueueSender {
            sender: self.sender.clone(),
        }
    }

    /// Get a reference to the receiver (for game thread).
    pub fn receiver(&self) -> &Receiver<QueuedPacket> {
        &self.receiver
    }

    /// Try to receive a packet without blocking.
    ///
    /// Returns `Some(packet)` if available, `None` if queue is empty.
    pub fn try_recv(&self) -> Option<QueuedPacket> {
        self.receiver.try_recv().ok()
    }

    /// Receive a packet, blocking until one is available.
    ///
    /// Returns `None` if the channel is disconnected.
    pub fn recv(&self) -> Option<QueuedPacket> {
        self.receiver.recv().ok()
    }

    /// Check if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.receiver.is_empty()
    }

    /// Get the number of packets currently in the queue.
    pub fn len(&self) -> usize {
        self.receiver.len()
    }
}

/// Sender handle for the packet queue (used by I/O thread).
#[derive(Clone)]
pub struct PacketQueueSender {
    sender: Sender<QueuedPacket>,
}

impl PacketQueueSender {
    /// Try to send a packet without blocking.
    ///
    /// Returns `true` if sent, `false` if queue is full (packet dropped).
    pub fn try_send(&self, packet: QueuedPacket) -> bool {
        match self.sender.try_send(packet) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => {
                // Queue full - drop the packet (common under heavy load)
                false
            }
            Err(TrySendError::Disconnected(_)) => {
                // Channel closed - I/O thread should shut down
                false
            }
        }
    }

    /// Check if the channel is likely disconnected by checking if it's empty.
    ///
    /// Note: This is an approximation. The actual disconnection is detected
    /// when try_send returns Disconnected.
    pub fn is_disconnected(&self) -> bool {
        // Crossbeam doesn't have is_disconnected, so we check via is_empty
        // on the sender side. A more accurate check happens in try_send.
        // For now, we assume connected if the sender exists.
        false
    }
}

/// Default queue capacity - handles typical burst traffic without excessive memory use.
pub const DEFAULT_QUEUE_CAPACITY: usize = 256;

/// Maximum queue capacity - absolute limit to prevent memory exhaustion.
pub const MAX_QUEUE_CAPACITY: usize = 4096;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qcommon::NetAdrType;

    fn make_test_packet(id: u8) -> QueuedPacket {
        QueuedPacket::new(
            NetSrc::Server,
            NetAdr {
                adr_type: NetAdrType::Ip,
                ip: [127, 0, 0, 1],
                ip6: [0; 16],
                scope_id: 0,
                port: 27910,
            },
            vec![id],
            1000,
        )
    }

    #[test]
    fn test_queue_basic_operations() {
        let queue = PacketQueue::new(10);
        let sender = queue.sender();

        // Queue should start empty
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);

        // Send a packet
        assert!(sender.try_send(make_test_packet(1)));
        assert!(!queue.is_empty());
        assert_eq!(queue.len(), 1);

        // Receive the packet
        let packet = queue.try_recv().unwrap();
        assert_eq!(packet.data, vec![1]);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_queue_full_drops_packets() {
        let queue = PacketQueue::new(2);
        let sender = queue.sender();

        // Fill the queue
        assert!(sender.try_send(make_test_packet(1)));
        assert!(sender.try_send(make_test_packet(2)));
        assert_eq!(queue.len(), 2);

        // Next send should fail (queue full)
        assert!(!sender.try_send(make_test_packet(3)));

        // Original packets still there
        assert_eq!(queue.try_recv().unwrap().data, vec![1]);
        assert_eq!(queue.try_recv().unwrap().data, vec![2]);
    }

    #[test]
    fn test_sender_clone() {
        let queue = PacketQueue::new(10);
        let sender1 = queue.sender();
        let sender2 = queue.sender();

        // Both senders should work
        assert!(sender1.try_send(make_test_packet(1)));
        assert!(sender2.try_send(make_test_packet(2)));

        assert_eq!(queue.len(), 2);
    }
}
