// qcommon.rs — definitions common between client and server, but not game.dll
// Converted from: myq2-original/qcommon/qcommon.h

// ============================================================
// Version / build info
// ============================================================

pub const VERSION: f32 = 3.21;
pub const BASEDIRNAME: &str = "baseq2";

#[cfg(all(target_os = "windows", not(debug_assertions)))]
pub const BUILDSTRING: &str = "Win32 RELEASE";
#[cfg(all(target_os = "windows", debug_assertions))]
pub const BUILDSTRING: &str = "Win32 DEBUG";
#[cfg(target_os = "linux")]
pub const BUILDSTRING: &str = "Linux";
#[cfg(not(any(target_os = "windows", target_os = "linux")))]
pub const BUILDSTRING: &str = "Unknown";

#[cfg(target_arch = "x86")]
pub const CPUSTRING: &str = "x86";
#[cfg(target_arch = "x86_64")]
pub const CPUSTRING: &str = "x86_64";
#[cfg(target_arch = "aarch64")]
pub const CPUSTRING: &str = "aarch64";
#[cfg(not(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64")))]
pub const CPUSTRING: &str = "Unknown";

// ============================================================
// SizeBuf — growable byte buffer
// ============================================================

#[derive(Debug, Clone, Default)]
pub struct SizeBuf {
    pub allow_overflow: bool,
    pub overflowed: bool,
    pub data: Vec<u8>,
    pub maxsize: i32,
    pub cursize: i32,
    pub readcount: i32,
}

impl SizeBuf {
    pub fn new(maxsize: i32) -> Self {
        Self {
            allow_overflow: false,
            overflowed: false,
            data: vec![0u8; maxsize as usize],
            maxsize,
            cursize: 0,
            readcount: 0,
        }
    }

    pub fn clear(&mut self) {
        self.cursize = 0;
        self.overflowed = false;
    }
}

// ============================================================
// Protocol
// ============================================================

/// Original Quake 2 protocol version
pub const PROTOCOL_VERSION: i32 = 34;

/// R1Q2 enhanced protocol version
/// Features: zlib compression, 1-byte qport, 4096 byte packets
pub const PROTOCOL_R1Q2: i32 = 35;

/// Q2Pro enhanced protocol version (extends R1Q2)
/// Features: multi-command packing, datagram fragmentation
pub const PROTOCOL_Q2PRO: i32 = 36;

/// Minimum supported protocol version
pub const PROTOCOL_VERSION_MIN: i32 = PROTOCOL_VERSION;

/// Maximum supported protocol version
pub const PROTOCOL_VERSION_MAX: i32 = PROTOCOL_Q2PRO;

// Client-to-server ops (clc_ops_e in original)
pub const CLC_BAD: u8 = 0;
pub const CLC_NOP: u8 = 1;
pub const CLC_MOVE: u8 = 2;
pub const CLC_USERINFO: u8 = 3;
pub const CLC_STRINGCMD: u8 = 4;

pub const PORT_MASTER: i32 = 27900;
pub const PORT_CLIENT: i32 = 27901;
pub const PORT_SERVER: i32 = 27910;
pub const PORT_ANY: i32 = -1;

pub const UPDATE_BACKUP: i32 = 16;
pub const UPDATE_MASK: i32 = UPDATE_BACKUP - 1;

// ============================================================
// Server-to-client ops
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum SvcOps {
    Bad = 0,
    MuzzleFlash,
    MuzzleFlash2,
    TempEntity,
    Layout,
    Inventory,
    Nop,
    Disconnect,
    Reconnect,
    Sound,
    Print,
    StuffText,
    ServerData,
    ConfigString,
    SpawnBaseline,
    CenterPrint,
    Download,
    PlayerInfo,
    PacketEntities,
    DeltaPacketEntities,
    Frame,
}

// ============================================================
// Client-to-server ops
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ClcOps {
    Bad = 0,
    Nop,
    Move,
    UserInfo,
    StringCmd,
}

// ============================================================
// Player state communication flags
// ============================================================

pub const PS_M_TYPE: i32 = 1 << 0;
pub const PS_M_ORIGIN: i32 = 1 << 1;
pub const PS_M_VELOCITY: i32 = 1 << 2;
pub const PS_M_TIME: i32 = 1 << 3;
pub const PS_M_FLAGS: i32 = 1 << 4;
pub const PS_M_GRAVITY: i32 = 1 << 5;
pub const PS_M_DELTA_ANGLES: i32 = 1 << 6;
pub const PS_VIEWOFFSET: i32 = 1 << 7;
pub const PS_VIEWANGLES: i32 = 1 << 8;
pub const PS_KICKANGLES: i32 = 1 << 9;
pub const PS_BLEND: i32 = 1 << 10;
pub const PS_FOV: i32 = 1 << 11;
pub const PS_WEAPONINDEX: i32 = 1 << 12;
pub const PS_WEAPONFRAME: i32 = 1 << 13;
pub const PS_RDFLAGS: i32 = 1 << 14;

// ============================================================
// User command communication flags
// ============================================================

pub const CM_ANGLE1: i32 = 1 << 0;
pub const CM_ANGLE2: i32 = 1 << 1;
pub const CM_ANGLE3: i32 = 1 << 2;
pub const CM_FORWARD: i32 = 1 << 3;
pub const CM_SIDE: i32 = 1 << 4;
pub const CM_UP: i32 = 1 << 5;
pub const CM_BUTTONS: i32 = 1 << 6;
pub const CM_IMPULSE: i32 = 1 << 7;

// ============================================================
// Sound flags
// ============================================================

pub const SND_VOLUME: i32 = 1 << 0;
pub const SND_ATTENUATION: i32 = 1 << 1;
pub const SND_POS: i32 = 1 << 2;
pub const SND_ENT: i32 = 1 << 3;
pub const SND_OFFSET: i32 = 1 << 4;

pub const DEFAULT_SOUND_PACKET_VOLUME: f32 = 1.0;
pub const DEFAULT_SOUND_PACKET_ATTENUATION: f32 = 1.0;

// ============================================================
// Entity state communication flags
// ============================================================

// First byte
pub const U_ORIGIN1: i32 = 1 << 0;
pub const U_ORIGIN2: i32 = 1 << 1;
pub const U_ANGLE2: i32 = 1 << 2;
pub const U_ANGLE3: i32 = 1 << 3;
pub const U_FRAME8: i32 = 1 << 4;
pub const U_EVENT: i32 = 1 << 5;
pub const U_REMOVE: i32 = 1 << 6;
pub const U_MOREBITS1: i32 = 1 << 7;

// Second byte
pub const U_NUMBER16: i32 = 1 << 8;
pub const U_ORIGIN3: i32 = 1 << 9;
pub const U_ANGLE1: i32 = 1 << 10;
pub const U_MODEL: i32 = 1 << 11;
pub const U_RENDERFX8: i32 = 1 << 12;
pub const U_EFFECTS8: i32 = 1 << 14;
pub const U_MOREBITS2: i32 = 1 << 15;

// Third byte
pub const U_SKIN8: i32 = 1 << 16;
pub const U_FRAME16: i32 = 1 << 17;
pub const U_RENDERFX16: i32 = 1 << 18;
pub const U_EFFECTS16: i32 = 1 << 19;
pub const U_MODEL2: i32 = 1 << 20;
pub const U_MODEL3: i32 = 1 << 21;
pub const U_MODEL4: i32 = 1 << 22;
pub const U_MOREBITS3: i32 = 1 << 23;

// Fourth byte
pub const U_OLDORIGIN: i32 = 1 << 24;
pub const U_SKIN16: i32 = 1 << 25;
pub const U_SOUND: i32 = 1 << 26;
pub const U_SOLID: i32 = 1 << 27;

// ============================================================
// Command execution — canonical definitions in cmd.rs
// ============================================================

pub use crate::cmd::{EXEC_NOW, EXEC_INSERT, EXEC_APPEND};

// ============================================================
// Error levels — canonical definitions in q_shared.rs
// ============================================================

pub use crate::q_shared::{ERR_FATAL, ERR_DROP};

/// ERR_QUIT is an alias for ERR_DISCONNECT (same value, engine-level semantics)
pub const ERR_QUIT: i32 = crate::q_shared::ERR_DISCONNECT;

// ============================================================
// Print levels — canonical definitions in q_shared.rs
// ============================================================

pub use crate::q_shared::{PRINT_ALL, PRINT_DEVELOPER};

// ============================================================
// Network types
// ============================================================

/// Maximum message length for protocol 34 (original Q2)
pub const MAX_MSGLEN: usize = 1400;

/// Maximum message length for protocol 35+ (R1Q2/Q2Pro)
pub const MAX_MSGLEN_R1Q2: usize = 4096;

pub const PACKET_HEADER: usize = 10;
pub const MAX_PROJECTILES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum NetAdrType {
    Loopback = 0,
    Broadcast,
    Ip,
    /// IPv6 address
    Ip6,
    /// IPv6 broadcast/multicast
    Broadcast6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum NetSrc {
    Client = 0,
    Server,
}

#[derive(Debug, Clone, Copy)]
pub struct NetAdr {
    pub adr_type: NetAdrType,
    /// IPv4 address (4 bytes)
    pub ip: [u8; 4],
    /// IPv6 address (16 bytes)
    pub ip6: [u8; 16],
    /// IPv6 scope ID for link-local addresses
    pub scope_id: u32,
    pub port: u16,
}

impl Default for NetAdr {
    fn default() -> Self {
        Self {
            adr_type: NetAdrType::Loopback,
            ip: [0; 4],
            ip6: [0; 16],
            scope_id: 0,
            port: 0,
        }
    }
}

// ============================================================
// NetChan — network channel
// ============================================================

pub const OLD_AVG: f32 = 0.99;
pub const MAX_LATENT: usize = 32;

/// Q2Pro (protocol 36) fragmentation state.
/// Used for sending/receiving large datagrams that exceed the MTU.
#[derive(Debug, Clone, Default)]
pub struct FragmentState {
    /// True if a fragmented packet is being received/sent
    pub in_progress: bool,
    /// Sequence number of the fragmented packet
    pub sequence: i32,
    /// Current offset into the fragmented data
    pub current_offset: i32,
    /// Total size of the complete message
    pub total_size: i32,
    /// Buffer for accumulating fragmented data
    pub buffer: Vec<u8>,
}

impl FragmentState {
    pub fn new() -> Self {
        Self {
            in_progress: false,
            sequence: 0,
            current_offset: 0,
            total_size: 0,
            buffer: Vec::with_capacity(MAX_MSGLEN_R1Q2),
        }
    }

    /// Reset the fragment state
    pub fn reset(&mut self) {
        self.in_progress = false;
        self.sequence = 0;
        self.current_offset = 0;
        self.total_size = 0;
        self.buffer.clear();
    }
}

pub struct NetChan {
    pub sock: NetSrc,
    pub dropped: i32,
    pub last_received: i32,
    pub last_sent: i32,
    pub remote_address: NetAdr,
    pub qport: i32,

    /// Negotiated protocol version (34, 35, or 36)
    pub protocol: i32,

    // Sequencing variables
    pub incoming_sequence: i32,
    pub incoming_acknowledged: i32,
    pub incoming_reliable_acknowledged: i32,
    pub incoming_reliable_sequence: i32,
    pub outgoing_sequence: i32,
    pub reliable_sequence: i32,
    pub last_reliable_sequence: i32,

    // Reliable staging and holding areas
    pub message: SizeBuf,
    pub message_buf: [u8; MAX_MSGLEN - 16],
    pub reliable_length: i32,
    pub reliable_buf: [u8; MAX_MSGLEN - 16],

    // Q2Pro (protocol 36) fragmentation support
    /// Incoming fragment state for receiving large packets
    pub fragment_in: FragmentState,
    /// Outgoing fragment state for sending large packets
    pub fragment_out: FragmentState,
}

impl NetChan {
    pub fn new() -> Self {
        Self {
            sock: NetSrc::Client,
            dropped: 0,
            last_received: 0,
            last_sent: 0,
            remote_address: NetAdr::default(),
            qport: 0,
            protocol: PROTOCOL_VERSION, // Default to original protocol
            incoming_sequence: 0,
            incoming_acknowledged: 0,
            incoming_reliable_acknowledged: 0,
            incoming_reliable_sequence: 0,
            outgoing_sequence: 0,
            reliable_sequence: 0,
            last_reliable_sequence: 0,
            message: SizeBuf::new((MAX_MSGLEN - 16) as i32),
            message_buf: [0u8; MAX_MSGLEN - 16],
            reliable_length: 0,
            reliable_buf: [0u8; MAX_MSGLEN - 16],
            fragment_in: FragmentState::new(),
            fragment_out: FragmentState::new(),
        }
    }
}

impl Default for NetChan {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// SVC_* integer constants matching the SvcOps enum values
// ============================================================

pub const SVC_BAD: i32 = 0;
pub const SVC_MUZZLEFLASH: i32 = 1;
pub const SVC_MUZZLEFLASH2: i32 = 2;
pub const SVC_TEMP_ENTITY: i32 = 3;
pub const SVC_LAYOUT: i32 = 4;
pub const SVC_INVENTORY: i32 = 5;
pub const SVC_NOP: i32 = 6;
pub const SVC_DISCONNECT: i32 = 7;
pub const SVC_RECONNECT: i32 = 8;
pub const SVC_SOUND: i32 = 9;
pub const SVC_PRINT: i32 = 10;
pub const SVC_STUFFTEXT: i32 = 11;
pub const SVC_SERVERDATA: i32 = 12;
pub const SVC_CONFIGSTRING: i32 = 13;
pub const SVC_SPAWNBASELINE: i32 = 14;
pub const SVC_CENTERPRINT: i32 = 15;
pub const SVC_DOWNLOAD: i32 = 16;
pub const SVC_PLAYERINFO: i32 = 17;
pub const SVC_PACKETENTITIES: i32 = 18;
pub const SVC_DELTAPACKETENTITIES: i32 = 19;
pub const SVC_FRAME: i32 = 20;

// R1Q2/Q2Pro protocol extensions (protocol 35+)
/// Compressed packet - contains zlib-compressed data
pub const SVC_ZPACKET: i32 = 21;
/// Compressed download chunk - includes uncompressed size
pub const SVC_ZDOWNLOAD: i32 = 22;

// ============================================================
// Memory tags (for Z_TagMalloc)
// ============================================================

pub const NUMVERTEXNORMALS: usize = 162;
