// server.rs — core server types and constants
// Converted from: myq2-original/server/server.h
//
// Copyright (C) 1997-2001 Id Software, Inc.
// Licensed under the GNU General Public License v2 or later.

use myq2_common::cvar::CvarContext;
use myq2_common::q_shared::*;
use myq2_common::qcommon::*;
use myq2_common::qfiles::MAX_MAP_AREAS;

use crate::sv_game::{GameExport, GameModule};
use crate::sv_lag_compensation::LagCompensation;

use std::fs::File;

// ============================================================
// Constants
// ============================================================

pub const MAX_MASTERS: usize = 8; // max recipients for heartbeat packets

pub const LATENCY_COUNTS: usize = 16;
pub const RATE_MESSAGES: usize = 10;

pub const MAX_CHALLENGES: usize = 1024;

/// Maximum entities per packet (standard Quake 2 value).
pub const MAX_PACKET_ENTITIES: usize = 128;

pub const SV_OUTPUTBUF_LENGTH: usize = MAX_MSGLEN - 16;

// ============================================================
// server_state_t
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
#[derive(Default)]
pub enum ServerState {
    #[default]
    Dead = 0,       // no map loaded
    Loading = 1,    // spawning level edicts
    Game = 2,       // actively running
    Cinematic = 3,
    Demo = 4,
    Pic = 5,
}


// ============================================================
// client_state_t (connection state of a client on the server)
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
#[derive(Default)]
pub enum ClientState {
    #[default]
    Free = 0,       // can be reused for a new connection
    Zombie = 1,     // client has been disconnected, but don't reuse for a couple seconds
    Connected = 2,  // has been assigned to a client_t, but not in game yet
    Spawned = 3,    // client is fully in game
}


// ============================================================
// redirect_t
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum Redirect {
    None = 0,
    Client = 1,
    Packet = 2,
}

// ============================================================
// Server (per-level state) — corresponds to C `server_t`
// ============================================================

#[repr(C)]
pub struct Server {
    pub state: ServerState, // precache commands are only valid during load

    pub attractloop: bool,  // running cinematics and demos for the local system only
    pub loadgame: bool,     // client begins should reuse existing entity

    pub time: u32,          // always sv.framenum * 100 msec
    pub framenum: i32,

    pub name: String,       // map name, or cinematic name (MAX_QPATH)

    /// Model handles — stored as i32 indices (originally cmodel_s pointers).
    pub models: [i32; MAX_MODELS],

    pub configstrings: Vec<String>,   // [MAX_CONFIGSTRINGS], each up to MAX_QPATH
    pub baselines: Vec<EntityState>,  // [MAX_EDICTS]

    // The multicast buffer is used to send a message to a set of clients.
    // It is only used to marshall data until SV_Multicast is called.
    pub multicast: SizeBuf,
    pub multicast_buf: Vec<u8>, // [MAX_MSGLEN]

    // Demo server information
    pub demofile: Option<File>,
    pub timedemo: bool, // don't time sync
}

impl Default for Server {
    fn default() -> Self {
        let mut configstrings = Vec::with_capacity(MAX_CONFIGSTRINGS);
        for _ in 0..MAX_CONFIGSTRINGS {
            configstrings.push(String::new());
        }
        let mut baselines = Vec::with_capacity(MAX_EDICTS);
        for _ in 0..MAX_EDICTS {
            baselines.push(EntityState::default());
        }

        Self {
            state: ServerState::Dead,
            attractloop: false,
            loadgame: false,
            time: 0,
            framenum: 0,
            name: String::new(),
            models: [0i32; MAX_MODELS],
            configstrings,
            baselines,
            multicast: SizeBuf::new(MAX_MSGLEN as i32),
            multicast_buf: vec![0u8; MAX_MSGLEN],
            demofile: None,
            timedemo: false,
        }
    }
}

// ============================================================
// EDICT_NUM / NUM_FOR_EDICT — index-based in Rust
// ============================================================

// ============================================================
// ClientFrame — per-frame client snapshot (client_frame_t)
// ============================================================

#[repr(C)]
pub struct ClientFrame {
    pub areabytes: i32,
    pub areabits: [u8; MAX_MAP_AREAS / 8], // portalarea visibility bits
    pub ps: PlayerState,
    pub num_entities: i32,
    pub first_entity: i32, // into the circular sv_packet_entities[]
    pub senttime: i32,     // for ping calculations
}

impl Default for ClientFrame {
    fn default() -> Self {
        Self {
            areabytes: 0,
            areabits: [0; MAX_MAP_AREAS / 8],
            ps: PlayerState::default(),
            num_entities: 0,
            first_entity: 0,
            senttime: 0,
        }
    }
}

// ============================================================
// Client — per-client server data (client_t)
// ============================================================

pub struct Client {
    pub state: ClientState,

    pub userinfo: String, // name, etc (MAX_INFO_STRING)

    pub lastframe: i32,   // for delta compression
    pub lastcmd: UserCmd,  // for filling in big drops

    pub command_msec: i32, // every seconds this is reset; if user
                           // commands exhaust it, assume time cheating

    pub frame_latency: [i32; LATENCY_COUNTS],
    pub ping: i32,

    pub message_size: [i32; RATE_MESSAGES], // used to rate drop packets
    pub rate: i32,
    pub surpress_count: i32, // number of messages rate suppressed

    pub edict_index: i32, // index into edicts array (replaces edict_t *edict pointer)
    pub name: String,      // extracted from userinfo, high bits masked (32 chars)
    pub messagelevel: i32, // for filtering printed messages

    // The datagram is written to by sound calls, prints, temp ents, etc.
    // It can be harmlessly overflowed.
    pub datagram: SizeBuf,
    pub datagram_buf: Vec<u8>, // [MAX_MSGLEN]

    pub frames: Vec<ClientFrame>, // [UPDATE_BACKUP] — updates can be delta'd from here

    pub download: Option<Vec<u8>>, // file being downloaded
    pub downloadsize: i32,         // total bytes (can't use EOF because of paks)
    pub downloadcount: i32,        // bytes sent

    pub lastmessage: i32, // sv.framenum when packet was last received
    pub lastconnect: i32,

    pub challenge: i32, // challenge of this user, randomly generated

    pub netchan: NetChan,
}

// A client can leave the server in one of four ways:
// - dropping properly by quitting or disconnecting
// - timing out if no valid messages are received for timeout.value seconds
// - getting kicked off by the server operator
// - a program error, like an overflowed reliable buffer

impl Default for Client {
    fn default() -> Self {
        Self {
            state: ClientState::Free,
            userinfo: String::new(),
            lastframe: 0,
            lastcmd: UserCmd::default(),
            command_msec: 0,
            frame_latency: [0; LATENCY_COUNTS],
            ping: 0,
            message_size: [0; RATE_MESSAGES],
            rate: 0,
            surpress_count: 0,
            edict_index: 0,
            name: String::new(),
            messagelevel: 0,
            datagram: SizeBuf::new(MAX_MSGLEN as i32),
            datagram_buf: vec![0u8; MAX_MSGLEN],
            frames: {
                let mut v = Vec::with_capacity(UPDATE_BACKUP as usize);
                v.resize_with(UPDATE_BACKUP as usize, ClientFrame::default);
                v
            },
            download: None,
            downloadsize: 0,
            downloadcount: 0,
            lastmessage: 0,
            lastconnect: 0,
            challenge: 0,
            netchan: NetChan::new(),
        }
    }
}

// ============================================================
// Challenge (challenge_t)
// ============================================================

#[repr(C)]
#[derive(Clone)]
#[derive(Default)]
pub struct Challenge {
    pub adr: NetAdr,
    pub challenge: i32,
    pub time: i32,
}


// ============================================================
// ServerStatic — persistent across level changes (server_static_t)
// ============================================================

pub struct ServerStatic {
    pub initialized: bool, // sv_init has completed
    pub realtime: i32,     // always increasing, no clamping, etc

    pub mapcmd: String, // ie: *intro.cin+base (MAX_TOKEN_CHARS)

    pub spawncount: i32, // incremented each server start; used to check late spawns

    pub clients: Vec<Client>,       // [maxclients->value]
    pub num_client_entities: i32,   // maxclients->value * UPDATE_BACKUP * MAX_PACKET_ENTITIES
    pub next_client_entities: i32,  // next client_entity to use
    pub client_entities: Vec<EntityState>, // [num_client_entities]

    pub last_heartbeat: i32,

    pub challenges: Vec<Challenge>, // [MAX_CHALLENGES] — to prevent invalid IPs from connecting

    // Server-record values
    pub demofile: Option<File>,
    pub demo_multicast: SizeBuf,
    pub demo_multicast_buf: Vec<u8>, // [MAX_MSGLEN]
}

impl Default for ServerStatic {
    fn default() -> Self {
        let mut challenges = Vec::with_capacity(MAX_CHALLENGES);
        for _ in 0..MAX_CHALLENGES {
            challenges.push(Challenge::default());
        }

        Self {
            initialized: false,
            realtime: 0,
            mapcmd: String::new(),
            spawncount: 0,
            clients: Vec::new(),
            num_client_entities: 0,
            next_client_entities: 0,
            client_entities: Vec::new(),
            last_heartbeat: 0,
            challenges,
            demofile: None,
            demo_multicast: SizeBuf::new(MAX_MSGLEN as i32),
            demo_multicast_buf: vec![0u8; MAX_MSGLEN],
        }
    }
}

// ============================================================
// ServerContext — replaces C globals (sv, svs, ge, cvar pointers,
// master_adr, sv_client, sv_player, sv_outputbuf, etc.)
// ============================================================

pub struct ServerContext {
    pub sv: Server,
    pub svs: ServerStatic,

    /// Legacy game export - kept for backwards compatibility during transition
    pub ge: Option<GameExport>,

    /// New unified game module - supports both static Rust game and dynamic C DLLs
    pub game_module: Option<GameModule>,

    // Extern globals from the C header
    pub master_adr: [NetAdr; MAX_MASTERS],
    pub sv_client_index: Option<usize>,  // index into svs.clients (replaces client_t *sv_client)
    pub sv_player_index: Option<i32>,    // edict index (replaces edict_t *sv_player)

    pub sv_outputbuf: [u8; SV_OUTPUTBUF_LENGTH],

    // Cvar values (in C these were cvar_t* globals)
    pub maxclients_value: f32,
    pub sv_paused: bool,
    pub sv_noreload: bool,
    pub sv_airaccelerate: f32,
    pub sv_enforcetime: bool,

    // Download permission cvars
    pub allow_download: bool,
    pub allow_download_players: bool,
    pub allow_download_models: bool,
    pub allow_download_sounds: bool,
    pub allow_download_maps: bool,

    // Cvar system context
    pub cvars: CvarContext,

    // Network globals (replaces C extern netadr_t net_from, sizebuf_t net_message)
    pub net_from: NetAdr,
    pub net_message: SizeBuf,

    // Timing globals
    pub time_before_game: i32,
    pub time_after_game: i32,

    /// Lag compensation system for fair hit detection on high-ping clients
    pub lag_compensation: LagCompensation,
}

impl Default for ServerContext {
    fn default() -> Self {
        Self {
            sv: Server::default(),
            svs: ServerStatic::default(),
            ge: None,
            game_module: None,
            master_adr: Default::default(),
            sv_client_index: None,
            sv_player_index: None,
            sv_outputbuf: [0u8; SV_OUTPUTBUF_LENGTH],
            maxclients_value: 1.0,
            sv_paused: false,
            sv_noreload: false,
            sv_airaccelerate: 0.0,
            sv_enforcetime: false,
            allow_download: true,
            allow_download_players: true,
            allow_download_models: true,
            allow_download_sounds: true,
            allow_download_maps: true,
            cvars: CvarContext::new(),
            net_from: NetAdr::default(),
            net_message: SizeBuf::new(MAX_MSGLEN as i32),
            time_before_game: 0,
            time_after_game: 0,
            lag_compensation: LagCompensation::new(),
        }
    }
}
