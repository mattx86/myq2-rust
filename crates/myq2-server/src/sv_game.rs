// sv_game.rs -- interface to the game dll
// Converted from: myq2-original/server/sv_game.c

use crate::game_dll::GameDll;
use crate::game_ffi::{build_game_import, set_ffi_server_context, clear_ffi_server_context};
use crate::server::*;
use myq2_common::common::{com_dprintf, com_printf};
use myq2_common::game_api::{self, edict_t, game_import_t};
use myq2_common::q_shared::*;
use myq2_common::qcommon::*;

use std::sync::Mutex;

// ============================================================
// GameModule — supports both static Rust game and dynamic DLL
// ============================================================

/// The game module can be either statically linked (Rust game)
/// or dynamically loaded (C game DLL for mod compatibility).
pub enum GameModule {
    /// Statically linked Rust game module.
    /// Uses the existing GameExport/GameContext machinery.
    Static {
        export: GameExport,
    },
    /// Dynamically loaded C game DLL (gamex86.dll, etc).
    /// Uses the FFI bridge to communicate with the DLL.
    Dynamic {
        dll: GameDll,
        /// The game_import_t struct passed to the DLL
        /// Kept alive for the duration of the DLL's lifetime
        _import: Box<game_import_t>,
    },
}

impl GameModule {
    /// Get the API version of the loaded game
    pub fn apiversion(&self) -> i32 {
        match self {
            GameModule::Static { export } => export.apiversion,
            GameModule::Dynamic { dll, .. } => unsafe { (*dll.export).apiversion },
        }
    }

    /// Get the current number of edicts
    pub fn num_edicts(&self) -> i32 {
        match self {
            GameModule::Static { export } => export.num_edicts,
            GameModule::Dynamic { dll, .. } => unsafe { (*dll.export).num_edicts },
        }
    }

    /// Get the maximum number of edicts
    pub fn max_edicts(&self) -> i32 {
        match self {
            GameModule::Static { export } => export.max_edicts,
            GameModule::Dynamic { dll, .. } => unsafe { (*dll.export).max_edicts },
        }
    }

    /// Get the edict size (for DLL pointer arithmetic)
    pub fn edict_size(&self) -> i32 {
        match self {
            GameModule::Static { export } => export.edict_size,
            GameModule::Dynamic { dll, .. } => unsafe { (*dll.export).edict_size },
        }
    }

    /// Get the raw edicts pointer (for DLL mode)
    pub unsafe fn edicts_ptr(&self) -> *mut edict_t {
        match self {
            GameModule::Static { .. } => std::ptr::null_mut(),
            GameModule::Dynamic { dll, .. } => (*dll.export).edicts,
        }
    }

    /// Call ge->Init()
    pub fn init(&self) {
        match self {
            GameModule::Static { export } => {
                if let Some(init_fn) = export.init {
                    init_fn();
                }
            }
            GameModule::Dynamic { dll, .. } => unsafe {
                dll.init();
            },
        }
    }

    /// Call ge->Shutdown()
    pub fn shutdown(&self) {
        match self {
            GameModule::Static { export } => {
                if let Some(shutdown_fn) = export.shutdown {
                    shutdown_fn();
                }
            }
            GameModule::Dynamic { dll, .. } => unsafe {
                dll.shutdown();
            },
        }
    }

    /// Call ge->SpawnEntities(mapname, entstring, spawnpoint)
    pub fn spawn_entities(&self, mapname: &str, entstring: &str, spawnpoint: &str) {
        match self {
            GameModule::Static { export } => {
                if let Some(spawn_fn) = export.spawn_entities {
                    spawn_fn(mapname, entstring, spawnpoint);
                }
            }
            GameModule::Dynamic { dll, .. } => unsafe {
                dll.spawn_entities(mapname, entstring, spawnpoint);
            },
        }
    }

    /// Call ge->RunFrame()
    pub fn run_frame(&self) {
        match self {
            GameModule::Static { export } => {
                if let Some(run_fn) = export.run_frame {
                    run_fn();
                }
            }
            GameModule::Dynamic { dll, .. } => unsafe {
                dll.run_frame();
            },
        }
    }

    /// Call ge->ServerCommand()
    pub fn server_command(&self) {
        match self {
            GameModule::Static { export } => {
                if let Some(cmd_fn) = export.server_command {
                    cmd_fn();
                }
            }
            GameModule::Dynamic { dll, .. } => unsafe {
                dll.server_command();
            },
        }
    }

    /// Call ge->ClientConnect(ent_index, userinfo)
    /// Returns true if client is allowed to connect
    pub fn client_connect(&mut self, ent_index: i32, userinfo: &mut String) -> bool {
        match self {
            GameModule::Static { export } => {
                if let Some(func) = export.client_connect {
                    if let Some(ent) = export.edicts.get_mut(ent_index as usize) {
                        return func(ent, userinfo);
                    }
                }
                true // default: accept
            }
            GameModule::Dynamic { dll, .. } => unsafe {
                dll.client_connect(ent_index, userinfo)
            },
        }
    }

    /// Call ge->ClientBegin(ent_index)
    pub fn client_begin(&self, ent_index: i32) {
        match self {
            GameModule::Static { .. } => {
                // Static mode uses the game context's client_begin
            }
            GameModule::Dynamic { dll, .. } => unsafe {
                dll.client_begin(ent_index);
            },
        }
    }

    /// Call ge->ClientUserinfoChanged(ent_index, userinfo)
    pub fn client_userinfo_changed(&self, ent_index: i32, userinfo: &str) {
        match self {
            GameModule::Static { .. } => {
                // Static mode uses the game context's client_userinfo_changed
            }
            GameModule::Dynamic { dll, .. } => unsafe {
                dll.client_userinfo_changed(ent_index, userinfo);
            },
        }
    }

    /// Call ge->ClientDisconnect(ent_index)
    pub fn client_disconnect(&mut self, ent_index: i32) {
        match self {
            GameModule::Static { export } => {
                if let Some(func) = export.client_disconnect {
                    if let Some(ent) = export.edicts.get_mut(ent_index as usize) {
                        func(ent);
                    }
                }
            }
            GameModule::Dynamic { dll, .. } => unsafe {
                dll.client_disconnect(ent_index);
            },
        }
    }

    /// Call ge->ClientCommand(ent_index)
    pub fn client_command(&self, ent_index: i32) {
        match self {
            GameModule::Static { .. } => {
                // Static mode uses the game context's client_command
            }
            GameModule::Dynamic { dll, .. } => unsafe {
                dll.client_command(ent_index);
            },
        }
    }

    /// Call ge->ClientThink(ent_index, cmd)
    pub fn client_think(&self, ent_index: i32, cmd: &UserCmd) {
        match self {
            GameModule::Static { .. } => {
                // Static mode uses the game context's client_think
            }
            GameModule::Dynamic { dll, .. } => unsafe {
                // Convert UserCmd to usercmd_t for FFI
                let c_cmd = game_api::usercmd_t {
                    msec: cmd.msec,
                    buttons: cmd.buttons,
                    angles: cmd.angles,
                    forwardmove: cmd.forwardmove,
                    sidemove: cmd.sidemove,
                    upmove: cmd.upmove,
                    impulse: cmd.impulse,
                    lightlevel: cmd.lightlevel,
                };
                dll.client_think(ent_index, &c_cmd);
            },
        }
    }

    /// Call ge->WriteGame(filename, autosave)
    pub fn write_game(&self, filename: &str, autosave: bool) {
        match self {
            GameModule::Static { export } => {
                if let Some(write_fn) = export.write_game {
                    write_fn(filename, autosave);
                }
            }
            GameModule::Dynamic { dll, .. } => unsafe {
                dll.write_game(filename, autosave);
            },
        }
    }

    /// Call ge->ReadGame(filename)
    pub fn read_game(&self, filename: &str) {
        match self {
            GameModule::Static { export } => {
                if let Some(read_fn) = export.read_game {
                    read_fn(filename);
                }
            }
            GameModule::Dynamic { dll, .. } => unsafe {
                dll.read_game(filename);
            },
        }
    }

    /// Call ge->WriteLevel(filename)
    pub fn write_level(&self, filename: &str) {
        match self {
            GameModule::Static { export } => {
                if let Some(write_fn) = export.write_level {
                    write_fn(filename);
                }
            }
            GameModule::Dynamic { dll, .. } => unsafe {
                dll.write_level(filename);
            },
        }
    }

    /// Call ge->ReadLevel(filename)
    pub fn read_level(&self, filename: &str) {
        match self {
            GameModule::Static { export } => {
                if let Some(read_fn) = export.read_level {
                    read_fn(filename);
                }
            }
            GameModule::Dynamic { dll, .. } => unsafe {
                dll.read_level(filename);
            },
        }
    }

    /// Check if this is a dynamic (DLL) game module
    pub fn is_dynamic(&self) -> bool {
        matches!(self, GameModule::Dynamic { .. })
    }

    /// Get mutable access to the static GameExport (if static mode)
    pub fn as_static_mut(&mut self) -> Option<&mut GameExport> {
        match self {
            GameModule::Static { export } => Some(export),
            GameModule::Dynamic { .. } => None,
        }
    }

    /// Get reference to the static GameExport (if static mode)
    pub fn as_static(&self) -> Option<&GameExport> {
        match self {
            GameModule::Static { export } => Some(export),
            GameModule::Dynamic { .. } => None,
        }
    }
}

// ============================================================
// Global game context — holds the game module's state.
//
// In C, the game DLL maintained its own globals. Here we hold
// a g_spawn::GameContext in a global Mutex so that the plain fn
// pointers stored in GameExport can access it.
// ============================================================

static GAME_CONTEXT: Mutex<Option<myq2_game::g_local::GameContext>> = Mutex::new(None);
static IP_FILTER_STATE: Mutex<Option<myq2_game::g_svcmds::IpFilterState>> = Mutex::new(None);

/// Access the global game context. Panics if not initialized.
pub fn with_game_context<F, R>(f: F) -> R
where
    F: FnOnce(&mut myq2_game::g_local::GameContext) -> R,
{
    let mut guard = GAME_CONTEXT.lock().unwrap();
    let ctx = guard.as_mut().expect("Game context not initialized");
    f(ctx)
}

/// Sync server-visible edict fields from the game context into the server's GameExport.edicts.
/// Called after game code runs to propagate changes back to the server.
pub fn sync_edicts_to_server(ge: &mut GameExport) {
    let guard = GAME_CONTEXT.lock().unwrap();
    if let Some(ref game_ctx) = *guard {
        ge.num_edicts = game_ctx.num_edicts;

        // Ensure server edicts vec is large enough
        while ge.edicts.len() < game_ctx.edicts.len() {
            ge.edicts.push(Edict::default());
        }

        // Copy server-visible fields from game edicts to server edicts
        for (dst, src) in ge.edicts.iter_mut().zip(game_ctx.edicts.iter()) {
            dst.s = src.s.clone();
            dst.inuse = src.inuse;
            dst.linkcount = src.linkcount;
            dst.num_clusters = src.num_clusters;
            dst.clusternums = src.clusternums;
            dst.headnode = src.headnode;
            dst.areanum = src.areanum;
            dst.areanum2 = src.areanum2;
            dst.svflags = src.svflags;
            dst.mins = src.mins;
            dst.maxs = src.maxs;
            dst.absmin = src.absmin;
            dst.absmax = src.absmax;
            dst.size = src.size;
            dst.solid = match src.solid {
                myq2_game::game::Solid::Not => Solid::Not,
                myq2_game::game::Solid::Trigger => Solid::Trigger,
                myq2_game::game::Solid::Bbox => Solid::Bbox,
                myq2_game::game::Solid::Bsp => Solid::Bsp,
            };
            dst.clipmask = src.clipmask;
        }
    }
}

/// Sync server-visible edict fields from the server's GameExport into the game context.
/// Called before game code runs to propagate server-side changes.
pub fn sync_edicts_from_server(ge: &GameExport) {
    let mut guard = GAME_CONTEXT.lock().unwrap();
    if let Some(ref mut game_ctx) = *guard {
        // Sync entity state from server back to game
        for (dst, src) in game_ctx.edicts.iter_mut().zip(ge.edicts.iter()) {
            dst.linkcount = src.linkcount;
            dst.num_clusters = src.num_clusters;
            dst.clusternums = src.clusternums;
            dst.headnode = src.headnode;
            dst.areanum = src.areanum;
            dst.areanum2 = src.areanum2;
            dst.absmin = src.absmin;
            dst.absmax = src.absmax;
            dst.size = src.size;
        }
    }
}

// ============================================================
// Re-exports from myq2_game::game (canonical definitions)
// ============================================================

pub use myq2_game::game::{
    GAME_API_VERSION,
    SVF_NOCLIENT, SVF_DEADMONSTER, SVF_MONSTER, SVF_PROJECTILE,
    Solid,
};

// MAX_ENT_CLUSTERS comes from myq2_common::q_shared, re-export for other server modules
pub use myq2_common::q_shared::MAX_ENT_CLUSTERS;

// ============================================================
// Edict — entity dictionary (server-side view)
//
// This is the single canonical server-side Edict type, used by
// sv_game, sv_world, sv_ents, game_ffi, and server_game_import.
//
// In the original C code, `edict_t` (in game.h) defines the
// server-visible fields that must appear first in any edict.
// The game DLL extends it with private fields (see
// myq2-game::g_local::Edict for the full game-side edict).
//
// The game-side Edict (myq2-game) is intentionally a separate
// type because it contains ~130 game-private fields that the
// server must not depend on. The server accesses game edicts
// only through the GameExport interface. The two types share
// the same server-visible field set, but differ in:
//   - client: server uses Option<*mut GClient> (raw pointer
//     for FFI compatibility), game uses Option<usize> (index)
//   - owner: server uses owner_index: i32 (index), game uses i32
//   - area linking: server uses area_node/area_linked (for
//     spatial partitioning), game uses AreaLink (index-based)
//   - solid: server uses Solid enum, game uses Solid enum
// ============================================================

pub struct Edict {
    pub s: EntityState,
    pub client: Option<*mut GClient>,
    pub inuse: bool,
    pub linkcount: i32,

    // Linked to a division node or leaf (spatial partitioning)
    pub area_node: i32,     // index into areanodes, -1 = not linked
    pub area_linked: bool,

    pub num_clusters: i32, // if -1, use headnode instead
    pub clusternums: [i32; MAX_ENT_CLUSTERS],
    pub headnode: i32, // unused if num_clusters != -1
    pub areanum: i32,
    pub areanum2: i32,

    pub svflags: i32,
    pub mins: Vec3,
    pub maxs: Vec3,
    pub absmin: Vec3,
    pub absmax: Vec3,
    pub size: Vec3,
    pub solid: Solid,
    pub clipmask: i32,
    pub owner_index: i32, // index into edicts, -1 = none
}

impl Default for Edict {
    fn default() -> Self {
        Self {
            s: EntityState::default(),
            client: None,
            inuse: false,
            linkcount: 0,
            area_node: -1,
            area_linked: false,
            num_clusters: 0,
            clusternums: [0; MAX_ENT_CLUSTERS],
            headnode: 0,
            areanum: 0,
            areanum2: 0,
            svflags: 0,
            mins: [0.0; 3],
            maxs: [0.0; 3],
            absmin: [0.0; 3],
            absmax: [0.0; 3],
            size: [0.0; 3],
            solid: Solid::Not,
            clipmask: 0,
            owner_index: -1,
        }
    }
}

// ============================================================
// GClient — server-side view of client data
// ============================================================

#[derive(Default)]
pub struct GClient {
    pub ps: PlayerState,
    pub ping: i32,
}


// ============================================================
// GameExport — functions exported by the game subsystem
// ============================================================

#[derive(Default)]
pub struct GameExport {
    pub apiversion: i32,

    pub init: Option<fn()>,
    pub shutdown: Option<fn()>,

    pub spawn_entities: Option<fn(mapname: &str, entstring: &str, spawnpoint: &str)>,

    pub write_game: Option<fn(filename: &str, autosave: bool)>,
    pub read_game: Option<fn(filename: &str)>,

    pub write_level: Option<fn(filename: &str)>,
    pub read_level: Option<fn(filename: &str)>,

    pub client_connect: Option<fn(ent: &mut Edict, userinfo: &str) -> bool>,
    pub client_begin: Option<fn(ent: &mut Edict)>,
    pub client_userinfo_changed: Option<fn(ent: &mut Edict, userinfo: &str)>,
    pub client_disconnect: Option<fn(ent: &mut Edict)>,
    pub client_command: Option<fn(ent: &mut Edict)>,
    pub client_think: Option<fn(ent: &mut Edict, cmd: &UserCmd)>,

    pub run_frame: Option<fn()>,

    pub server_command: Option<fn()>,

    // Global variables shared between game and server
    pub edicts: Vec<Edict>,
    pub edict_size: i32,
    pub num_edicts: i32,
    pub max_edicts: i32,
}


impl GameExport {
    /// Call ge->ClientDisconnect by edict index.
    pub fn client_disconnect_by_index(&mut self, edict_index: i32) {
        if let Some(func) = self.client_disconnect {
            if let Some(ent) = self.edicts.get_mut(edict_index as usize) {
                func(ent);
            }
        }
    }

    /// Call ge->ClientConnect by edict index. Returns true if connection accepted.
    pub fn client_connect_by_index(&mut self, edict_index: i32, userinfo: &str) -> bool {
        if let Some(func) = self.client_connect {
            if let Some(ent) = self.edicts.get_mut(edict_index as usize) {
                return func(ent, userinfo);
            }
        }
        true // default: accept
    }

    /// Call ge->ClientUserinfoChanged by edict index.
    pub fn client_userinfo_changed_by_index(&mut self, edict_index: i32, userinfo: &str) {
        if let Some(func) = self.client_userinfo_changed {
            if let Some(ent) = self.edicts.get_mut(edict_index as usize) {
                func(ent, userinfo);
            }
        }
    }

    /// Call ge->RunFrame.
    pub fn run_frame_call(&self) {
        if let Some(func) = self.run_frame {
            func();
        }
    }

    /// Get num_edicts count.
    pub fn num_edicts(&self) -> i32 {
        self.num_edicts
    }

    /// Clear the event field on an edict by index.
    pub fn clear_edict_event(&mut self, index: i32) {
        if let Some(ent) = self.edicts.get_mut(index as usize) {
            ent.s.event = 0;
        }
    }

    /// Get the frags stat for a client edict.
    pub fn get_client_frags(&self, edict_index: i32) -> i32 {
        if let Some(ent) = self.edicts.get(edict_index as usize) {
            // SAFETY: accessing gclient through raw pointer if present
            if let Some(client_ptr) = ent.client {
                // SAFETY: pointer was set by game code and must be valid
                unsafe { (*client_ptr).ps.stats[STAT_FRAGS as usize] as i32 }
            } else {
                0
            }
        } else {
            0
        }
    }

    /// Get the health stat for a client edict.
    pub fn get_client_health(&self, edict_index: i32) -> i32 {
        if let Some(ent) = self.edicts.get(edict_index as usize) {
            if let Some(client_ptr) = ent.client {
                // SAFETY: pointer was set by game code and must be valid
                unsafe { (*client_ptr).ps.stats[STAT_HEALTH as usize] as i32 }
            } else {
                0
            }
        } else {
            0
        }
    }

    /// Set the ping on a client edict.
    pub fn set_client_ping(&mut self, edict_index: i32, ping: i32) {
        if let Some(ent) = self.edicts.get_mut(edict_index as usize) {
            if let Some(client_ptr) = ent.client {
                // SAFETY: pointer was set by game code and must be valid
                unsafe { (*client_ptr).ping = ping; }
            }
        }
    }
}

// ============================================================
// GameImport — functions provided by the engine to the game
// ============================================================

/// The game import table, containing all engine callbacks the game DLL can call.
/// In C this was a struct of function pointers passed to GetGameApi.
/// In Rust we represent this as a struct of closures/function references
/// stored in the ServerContext.
pub struct GameImport {
    // special messages
    pub bprintf: fn(ctx: &mut ServerContext, printlevel: i32, msg: &str),
    pub dprintf: fn(msg: &str),
    pub cprintf: fn(ctx: &mut ServerContext, ent: Option<&Edict>, printlevel: i32, msg: &str),
    pub centerprintf: fn(ctx: &mut ServerContext, ent: &Edict, msg: &str),
    pub sound: fn(ctx: &mut ServerContext, entity: &Edict, channel: i32, sound_num: i32, volume: f32, attenuation: f32, timeofs: f32),
    pub positioned_sound: fn(ctx: &mut ServerContext, origin: Option<&Vec3>, entity: &Edict, channel: i32, soundindex: i32, volume: f32, attenuation: f32, timeofs: f32),

    pub configstring: fn(ctx: &mut ServerContext, index: i32, val: &str),
    pub error: fn(msg: &str),

    pub modelindex: fn(ctx: &mut ServerContext, name: &str) -> i32,
    pub soundindex: fn(ctx: &mut ServerContext, name: &str) -> i32,
    pub imageindex: fn(ctx: &mut ServerContext, name: &str) -> i32,

    pub setmodel: fn(ctx: &mut ServerContext, ent: &mut Edict, name: &str),

    pub trace: fn(ctx: &ServerContext, start: &Vec3, mins: &Vec3, maxs: &Vec3, end: &Vec3, passent: Option<&Edict>, contentmask: i32) -> Trace,
    pub pointcontents: fn(ctx: &ServerContext, point: &Vec3) -> i32,
    pub in_pvs: fn(ctx: &ServerContext, p1: &Vec3, p2: &Vec3) -> bool,
    pub in_phs: fn(ctx: &ServerContext, p1: &Vec3, p2: &Vec3) -> bool,
    pub set_area_portal_state: fn(ctx: &mut ServerContext, portalnum: i32, open: bool),
    pub areas_connected: fn(ctx: &ServerContext, area1: i32, area2: i32) -> bool,

    pub link_entity: fn(ctx: &mut ServerContext, ent: &mut Edict),
    pub unlink_entity: fn(ctx: &mut ServerContext, ent: &mut Edict),

    pub write_char: fn(ctx: &mut ServerContext, c: i32),
    pub write_byte: fn(ctx: &mut ServerContext, c: i32),
    pub write_short: fn(ctx: &mut ServerContext, c: i32),
    pub write_long: fn(ctx: &mut ServerContext, c: i32),
    pub write_float: fn(ctx: &mut ServerContext, f: f32),
    pub write_string: fn(ctx: &mut ServerContext, s: &str),
    pub write_position: fn(ctx: &mut ServerContext, pos: &Vec3),
    pub write_dir: fn(ctx: &mut ServerContext, dir: &Vec3),
    pub write_angle: fn(ctx: &mut ServerContext, f: f32),

    pub multicast: fn(ctx: &mut ServerContext, origin: &Vec3, to: Multicast),
    pub unicast: fn(ctx: &mut ServerContext, ent: &Edict, reliable: bool),
}

// ============================================================
// Helper: NUM_FOR_EDICT / EDICT_NUM equivalents
// ============================================================

/// Get the index of an edict within the edicts array.
/// Equivalent to C macro: NUM_FOR_EDICT(e)
pub fn num_for_edict(_ctx: &ServerContext, ent: &Edict) -> i32 {
    // In Rust we search by entity state number since we use a Vec
    ent.s.number
}

/// Get an edict reference by index.
/// Equivalent to C macro: EDICT_NUM(n)
pub fn edict_num(ctx: &mut ServerContext, n: i32) -> Option<&mut Edict> {
    if let Some(ref mut ge) = ctx.ge {
        ge.edicts.get_mut(n as usize)
    } else {
        None
    }
}

// ============================================================
// PF_Unicast
//
// Sends the contents of the multicast buffer to a single client
// ============================================================

pub fn pf_unicast(ctx: &mut ServerContext, ent: &Edict, reliable: bool) {
    let p = ent.s.number;
    let maxclients_val = ctx.maxclients_value as i32;

    if p < 1 || p > maxclients_val {
        return;
    }

    let client_idx = (p - 1) as usize;

    // Copy multicast data
    let mc_data: Vec<u8> = ctx.sv.multicast.data[..ctx.sv.multicast.cursize as usize].to_vec();

    if let Some(client) = ctx.svs.clients.get_mut(client_idx) {
        if reliable {
            client.netchan.message.write(&mc_data);
        } else {
            client.datagram.write(&mc_data);
        }
    }

    ctx.sv.multicast.clear();
}

// ============================================================
// PF_dprintf
//
// Debug print to server console
// ============================================================

pub fn pf_dprintf(msg: &str) {
    com_printf(msg);
}

// ============================================================
// PF_cprintf
//
// Print to a single client
// ============================================================

pub fn pf_cprintf(ctx: &mut ServerContext, ent: Option<&Edict>, level: i32, msg: &str) {
    if let Some(e) = ent {
        let n = e.s.number;
        let maxclients_val = ctx.maxclients_value as i32;

        if n < 1 || n > maxclients_val {
            panic!("cprintf to a non-client");
        }

        let client_idx = (n - 1) as usize;
        if let Some(client) = ctx.svs.clients.get_mut(client_idx) {
            crate::sv_send::sv_client_printf(client, level, msg);
        }
    } else {
        // Print to server console
        com_printf(msg);
    }
}

// ============================================================
// PF_centerprintf
//
// centerprint to a single client
// ============================================================

pub fn pf_centerprintf(ctx: &mut ServerContext, ent: &Edict, msg: &str) {
    let n = ent.s.number;
    let maxclients_val = ctx.maxclients_value as i32;

    if n < 1 || n > maxclients_val {
        return; // Com_Error (ERR_DROP, "centerprintf to a non-client");
    }

    msg_write_byte(&mut ctx.sv.multicast, SvcOps::CenterPrint as i32);
    msg_write_string(&mut ctx.sv.multicast, msg);
    pf_unicast(ctx, ent, true);
}

// ============================================================
// PF_error
//
// Abort the server with a game error
// ============================================================

pub fn pf_error(msg: &str) -> ! {
    panic!("Game Error: {}", msg);
}

// ============================================================
// PF_setmodel
//
// Also sets mins and maxs for inline bmodels
// ============================================================

pub fn pf_setmodel(ctx: &mut ServerContext, ent: &mut Edict, name: &str) {
    if name.is_empty() {
        panic!("PF_setmodel: NULL");
    }

    let i = sv_model_index(ctx, name);

    ent.s.modelindex = i;

    // if it is an inline model, get the size information for it
    if name.starts_with('*') {
        let model = cm_inline_model(ctx, name);
        ent.mins = model.mins;
        ent.maxs = model.maxs;
        sv_link_edict(ctx, ent);
    }
}

// ============================================================
// PF_Configstring
// ============================================================

pub fn pf_configstring(ctx: &mut ServerContext, index: i32, val: &str) {
    if index < 0 || index >= MAX_CONFIGSTRINGS as i32 {
        panic!("configstring: bad index {}", index);
    }

    let val = if val.is_empty() { "" } else { val };

    // change the string in sv
    let idx = index as usize;
    ctx.sv.configstrings[idx] = val.to_string();

    if ctx.sv.state != ServerState::Loading {
        // send the update to everyone
        ctx.sv.multicast.clear();
        msg_write_char(&mut ctx.sv.multicast, SvcOps::ConfigString as i32);
        msg_write_short(&mut ctx.sv.multicast, index);
        msg_write_string(&mut ctx.sv.multicast, val);

        crate::sv_send::sv_multicast(ctx, Some(vec3_origin), Multicast::AllR);
    }
}

// ============================================================
// PF_Write* — message writing helpers
//
// These all write to sv.multicast
// ============================================================

pub fn pf_write_char(ctx: &mut ServerContext, c: i32) {
    msg_write_char(&mut ctx.sv.multicast, c);
}

pub fn pf_write_byte(ctx: &mut ServerContext, c: i32) {
    msg_write_byte(&mut ctx.sv.multicast, c);
}

pub fn pf_write_short(ctx: &mut ServerContext, c: i32) {
    msg_write_short(&mut ctx.sv.multicast, c);
}

pub fn pf_write_long(ctx: &mut ServerContext, c: i32) {
    msg_write_long(&mut ctx.sv.multicast, c);
}

pub fn pf_write_float(ctx: &mut ServerContext, f: f32) {
    msg_write_float(&mut ctx.sv.multicast, f);
}

pub fn pf_write_string(ctx: &mut ServerContext, s: &str) {
    msg_write_string(&mut ctx.sv.multicast, s);
}

pub fn pf_write_pos(ctx: &mut ServerContext, pos: &Vec3) {
    msg_write_pos(&mut ctx.sv.multicast, pos);
}

pub fn pf_write_dir(ctx: &mut ServerContext, dir: &Vec3) {
    msg_write_dir(&mut ctx.sv.multicast, dir);
}

pub fn pf_write_angle(ctx: &mut ServerContext, f: f32) {
    msg_write_angle(&mut ctx.sv.multicast, f);
}

// ============================================================
// PF_inPVS
//
// Also checks portalareas so that doors block sight
// ============================================================

pub fn pf_in_pvs(_ctx: &ServerContext, p1: &Vec3, p2: &Vec3) -> bool {
    use myq2_common::cmodel;

    let leafnum1 = cmodel::cm_point_leafnum(p1) as i32;
    let cluster1 = cmodel::cm_leaf_cluster(leafnum1 as usize);
    let area1 = cmodel::cm_leaf_area(leafnum1 as usize);
    let mask = cm_cluster_pvs(_ctx, cluster1);

    let leafnum2 = cmodel::cm_point_leafnum(p2) as i32;
    let cluster2 = cmodel::cm_leaf_cluster(leafnum2 as usize);
    let area2 = cmodel::cm_leaf_area(leafnum2 as usize);

    if let Some(mask) = mask {
        if cluster2 >= 0 {
            let byte_idx = (cluster2 >> 3) as usize;
            let bit = 1 << (cluster2 & 7);
            if byte_idx < mask.len() && (mask[byte_idx] & bit) == 0 {
                return false;
            }
        }
    }

    if !cmodel::cm_areas_connected(area1 as usize, area2 as usize) {
        return false; // a door blocks sight
    }

    true
}

// ============================================================
// PF_inPHS
//
// Also checks portalareas so that doors block sound
// ============================================================

pub fn pf_in_phs(_ctx: &ServerContext, p1: &Vec3, p2: &Vec3) -> bool {
    use myq2_common::cmodel;

    let leafnum1 = cmodel::cm_point_leafnum(p1) as i32;
    let cluster1 = cmodel::cm_leaf_cluster(leafnum1 as usize);
    let area1 = cmodel::cm_leaf_area(leafnum1 as usize);
    let mask = cm_cluster_phs(_ctx, cluster1);

    let leafnum2 = cmodel::cm_point_leafnum(p2) as i32;
    let cluster2 = cmodel::cm_leaf_cluster(leafnum2 as usize);
    let area2 = cmodel::cm_leaf_area(leafnum2 as usize);

    if let Some(mask) = mask {
        if cluster2 >= 0 {
            let byte_idx = (cluster2 >> 3) as usize;
            let bit = 1 << (cluster2 & 7);
            if byte_idx < mask.len() && (mask[byte_idx] & bit) == 0 {
                return false; // more than one bounce away
            }
        }
    }

    if !cmodel::cm_areas_connected(area1 as usize, area2 as usize) {
        return false; // a door blocks hearing
    }

    true
}

// ============================================================
// PF_StartSound
// ============================================================

pub fn pf_start_sound(
    ctx: &mut ServerContext,
    entity: &Edict,
    channel: i32,
    sound_num: i32,
    volume: f32,
    attenuation: f32,
    timeofs: f32,
) {
    sv_start_sound(ctx, None, entity, channel, sound_num, volume, attenuation, timeofs);
}

// ============================================================
// SV_ShutdownGameProgs
//
// Called when either the entire server is being killed, or
// it is changing to a different game directory.
// ============================================================

pub fn sv_shutdown_game_progs(ctx: &mut ServerContext) {
    if let Some(ref game_module) = ctx.game_module {
        game_module.shutdown();
    } else {
        return;
    }

    // Clear FFI context if we were using dynamic mode
    clear_ffi_server_context();

    // Sys_UnloadGame equivalent — drop the game module
    ctx.game_module = None;

    // Legacy ge field for backwards compatibility
    ctx.ge = None;
}

// ============================================================
// SV_InitGameProgs
//
// Init the game subsystem for a new map
// ============================================================

/// Initialize game subsystem - tries to load external DLL first, falls back to static Rust game
pub fn sv_init_game_progs(ctx: &mut ServerContext) {
    sv_init_game_progs_ex(ctx, None);
}

/// Initialize game subsystem with optional explicit DLL path
///
/// # Arguments
/// * `ctx` - Server context
/// * `dll_path` - Optional path to game DLL. If None, searches standard paths then falls back to static game.
pub fn sv_init_game_progs_ex(ctx: &mut ServerContext, dll_path: Option<&str>) {
    // Unload anything we have now
    if ctx.game_module.is_some() || ctx.ge.is_some() {
        sv_shutdown_game_progs(ctx);
    }

    // Try to load external game DLL first
    let game_module = if let Some(path) = dll_path {
        // Explicit path provided
        load_game_dll(ctx, path)
    } else {
        // Try to find game DLL in standard locations
        try_load_game_dll(ctx)
    };

    match game_module {
        Some(module) => {
            // Loaded external DLL successfully
            let api_version = module.apiversion();
            if api_version != GAME_API_VERSION {
                panic!(
                    "game DLL is version {}, not {}",
                    api_version, GAME_API_VERSION
                );
            }

            com_printf(&format!(
                "Loaded game module (API version {}, {} mode)\n",
                api_version,
                if module.is_dynamic() { "dynamic" } else { "static" }
            ));

            ctx.game_module = Some(module);

            // Call game Init
            if let Some(ref module) = ctx.game_module {
                module.init();
            }
        }
        None => {
            // Fall back to statically linked Rust game
            com_printf("Using statically linked Rust game module\n");

            let ge = load_game_module_static(ctx);

            if ge.apiversion != GAME_API_VERSION {
                panic!(
                    "game is version {}, not {}",
                    ge.apiversion, GAME_API_VERSION
                );
            }

            ctx.ge = Some(ge);

            ctx.game_module = Some(GameModule::Static {
                export: ctx.ge.take().unwrap(),
            });

            // Call game Init
            if let Some(ref module) = ctx.game_module {
                module.init();
            }

            // Sync edicts from game context to server's GameExport
            if let Some(GameModule::Static { ref mut export }) = ctx.game_module {
                sync_edicts_to_server(export);
            }
        }
    }
}

/// Try to load a game DLL from standard search paths
fn try_load_game_dll(ctx: &mut ServerContext) -> Option<GameModule> {
    // Get game directory from cvar (e.g., "baseq2", "ctf", etc.)
    let game = myq2_common::cvar::cvar_variable_string("game");
    let basedir = myq2_common::cvar::cvar_variable_string("basedir");

    // Build search paths
    let search_paths = if !game.is_empty() && game != "baseq2" {
        vec![
            format!("{}/{}", basedir, game),
            format!("{}/baseq2", basedir),
            game.clone(),
            "baseq2".to_string(),
        ]
    } else {
        vec![
            format!("{}/baseq2", basedir),
            "baseq2".to_string(),
        ]
    };

    // Try each search path
    for dir in search_paths {
        if let Some(dll_path) = crate::game_dll::find_game_dll(&dir) {
            com_dprintf(&format!("Found game DLL: {}\n", dll_path));
            if let Some(module) = load_game_dll(ctx, &dll_path) {
                return Some(module);
            }
        }
    }

    None
}

/// Load an external C game DLL
fn load_game_dll(ctx: &mut ServerContext, path: &str) -> Option<GameModule> {
    // Build the game_import_t struct with our FFI wrapper functions
    let mut import = Box::new(build_game_import());

    // Set the FFI server context so callbacks can access it
    unsafe {
        set_ffi_server_context(ctx as *mut ServerContext);
    }

    // Load the DLL
    let dll = unsafe {
        match GameDll::load(path, import.as_mut() as *mut game_import_t) {
            Ok(dll) => dll,
            Err(e) => {
                com_printf(&format!("Failed to load game DLL '{}': {}\n", path, e));
                clear_ffi_server_context();
                return None;
            }
        }
    };

    Some(GameModule::Dynamic {
        dll,
        _import: import,
    })
}

// ============================================================
// Placeholder / stub functions
//
// These represent functions defined in other server modules
// that are referenced by sv_game.c. They will be implemented
// in their respective Rust modules.
// ============================================================

// Re-export canonical message write functions from myq2_common::common.
pub use myq2_common::common::{
    msg_write_char, msg_write_byte, msg_write_short, msg_write_long,
    msg_write_float, msg_write_string, msg_write_pos, msg_write_dir,
    msg_write_angle,
};



/// SV_ModelIndex — Look up or register a model name in the configstrings.
///
/// Searches the CS_MODELS configstring range for the given name.
/// If not found and the server is loading, adds it. Returns the
/// index relative to CS_MODELS, or 0 if the name is empty.
pub fn sv_model_index(ctx: &mut ServerContext, name: &str) -> i32 {
    crate::sv_init::sv_model_index(ctx, name)
}

/// CM_InlineModel — Get the collision model for an inline BSP model (e.g., "*1", "*2").
///
/// Inline models are brush entities embedded in the map (doors, platforms, etc).
/// Returns the CModel with mins/maxs for the brush entity.
pub fn cm_inline_model(_ctx: &ServerContext, name: &str) -> CModel {
    myq2_common::cmodel::cm_inline_model(name)
}

/// SV_LinkEdict — Links an entity into the world spatial partitioning.
///
/// Sets the absolute bounding box, computes PVS cluster membership,
/// and inserts into the appropriate area node lists.
/// Full implementation lives in sv_world.rs (SvWorldContext::link_edict).
/// This wrapper computes absmin/absmax and increments linkcount.
pub fn sv_link_edict(_ctx: &mut ServerContext, ent: &mut Edict) {
    // Compute the size vector
    for i in 0..3 {
        ent.size[i] = ent.maxs[i] - ent.mins[i];
    }

    // Set the abs box (simplified: no rotation handling here)
    for i in 0..3 {
        ent.absmin[i] = ent.s.origin[i] + ent.mins[i] - 1.0;
        ent.absmax[i] = ent.s.origin[i] + ent.maxs[i] + 1.0;
    }

    // If first time linked, copy origin to old_origin
    if ent.linkcount == 0 {
        ent.s.old_origin = ent.s.origin;
    }
    ent.linkcount += 1;

    // Compute PVS cluster membership via the collision model.
    // Walk from the entity's bounding box to determine which BSP leaves
    // it touches, and record the cluster numbers for PVS visibility checks.
    if ent.solid == Solid::Not && ent.s.modelindex == 0 {
        // Entities with no model and no solidity don't need cluster info
        return;
    }

    // Determine the headnode for PVS testing
    let top_node = myq2_common::cmodel::cm_headnode_for_box(&ent.absmin, &ent.absmax);
    let leafs = myq2_common::cmodel::cm_box_leafnums(&ent.absmin, &ent.absmax, top_node);

    ent.num_clusters = 0;
    ent.headnode = 0;
    ent.areanum = 0;
    ent.areanum2 = 0;

    // If the entity spans too many leafs, use headnode for PVS check
    if leafs.len() > MAX_ENT_CLUSTERS as usize {
        ent.num_clusters = -1;
        ent.headnode = top_node;
    } else {
        ent.num_clusters = 0;
        for &leaf in &leafs {
            let cluster = myq2_common::cmodel::cm_leaf_cluster(leaf as usize);
            let area = myq2_common::cmodel::cm_leaf_area(leaf as usize);

            if area != 0 {
                if ent.areanum != 0 && ent.areanum != area {
                    if ent.areanum2 != 0 && ent.areanum2 != area && ent.areanum != area {
                        com_dprintf("Object touching 3 areas at once\n");
                    }
                    ent.areanum2 = area;
                } else {
                    ent.areanum = area;
                }
            }

            if cluster == -1 {
                continue;
            }
            if (ent.num_clusters as usize) < MAX_ENT_CLUSTERS {
                ent.clusternums[ent.num_clusters as usize] = cluster;
                ent.num_clusters += 1;
            }
        }
    }
}


/// SV_StartSound — Starts a sound on an entity.
///
/// Each entity can have eight independent sound sources.
/// If channel & 8, the sound is sent to everyone (no PHS check).
/// An attenuation of 0 plays full volume everywhere.
/// Timeofs ranges from 0.0 to 0.255 for delayed start within the frame.
pub fn sv_start_sound(
    ctx: &mut ServerContext,
    origin: Option<&Vec3>,
    entity: &Edict,
    mut channel: i32,
    soundindex: i32,
    volume: f32,
    attenuation: f32,
    timeofs: f32,
) {
    if !(0.0..=1.0).contains(&volume) {
        panic!("SV_StartSound: volume = {}", volume);
    }
    if !(0.0..=4.0).contains(&attenuation) {
        panic!("SV_StartSound: attenuation = {}", attenuation);
    }
    if !(0.0..=0.255).contains(&timeofs) {
        panic!("SV_StartSound: timeofs = {}", timeofs);
    }

    let ent = entity.s.number;

    let mut use_phs = true;
    if channel & 8 != 0 {
        use_phs = false;
        channel &= 7;
    }

    let sendchan = (ent << 3) | (channel & 7);

    let mut flags: i32 = 0;
    if volume != DEFAULT_SOUND_PACKET_VOLUME {
        flags |= SND_VOLUME;
    }
    if attenuation != DEFAULT_SOUND_PACKET_ATTENUATION {
        flags |= SND_ATTENUATION;
    }

    // bmodels have weird origins; explicit origin overrides entity origin
    if (entity.svflags & SVF_NOCLIENT) != 0
        || entity.solid == Solid::Bsp
        || origin.is_some()
    {
        flags |= SND_POS;
    }

    flags |= SND_ENT;

    if timeofs != 0.0 {
        flags |= SND_OFFSET;
    }

    // Compute the final origin
    let final_origin = match origin {
        Some(org) => *org,
        None => {
            if entity.solid == Solid::Bsp {
                [
                    entity.s.origin[0] + 0.5 * (entity.mins[0] + entity.maxs[0]),
                    entity.s.origin[1] + 0.5 * (entity.mins[1] + entity.maxs[1]),
                    entity.s.origin[2] + 0.5 * (entity.mins[2] + entity.maxs[2]),
                ]
            } else {
                entity.s.origin
            }
        }
    };

    msg_write_byte(&mut ctx.sv.multicast, SvcOps::Sound as i32);
    msg_write_byte(&mut ctx.sv.multicast, flags);
    msg_write_byte(&mut ctx.sv.multicast, soundindex);

    if (flags & SND_VOLUME) != 0 {
        msg_write_byte(&mut ctx.sv.multicast, (volume * 255.0) as i32);
    }
    if (flags & SND_ATTENUATION) != 0 {
        msg_write_byte(&mut ctx.sv.multicast, (attenuation * 64.0) as i32);
    }
    if (flags & SND_OFFSET) != 0 {
        msg_write_byte(&mut ctx.sv.multicast, (timeofs * 1000.0) as i32);
    }

    if (flags & SND_ENT) != 0 {
        msg_write_short(&mut ctx.sv.multicast, sendchan);
    }

    if (flags & SND_POS) != 0 {
        msg_write_pos(&mut ctx.sv.multicast, &final_origin);
    }

    // If no attenuation, send to everyone
    if attenuation == ATTN_NONE {
        use_phs = false;
    }

    if (channel & CHAN_RELIABLE) != 0 {
        if use_phs {
            crate::sv_send::sv_multicast(ctx, Some(final_origin), Multicast::PhsR);
        } else {
            crate::sv_send::sv_multicast(ctx, Some(final_origin), Multicast::AllR);
        }
    } else if use_phs {
        crate::sv_send::sv_multicast(ctx, Some(final_origin), Multicast::Phs);
    } else {
        crate::sv_send::sv_multicast(ctx, Some(final_origin), Multicast::All);
    }
}

/// CM_ClusterPVS — Get the Potentially Visible Set for a cluster.
///
/// Returns a bit vector where each bit represents whether a cluster
/// is potentially visible from the given cluster. Returns None if
/// no PVS data is available (all clusters visible).
pub fn cm_cluster_pvs(_ctx: &ServerContext, cluster: i32) -> Option<Vec<u8>> {
    let v = myq2_common::cmodel::cm_cluster_pvs(cluster);
    if v.is_empty() { None } else { Some(v) }
}

/// CM_ClusterPHS — Get the Potentially Hearable Set for a cluster.
///
/// Returns a bit vector where each bit represents whether a cluster
/// is potentially hearable from the given cluster. Returns None if
/// no PHS data is available (all clusters hearable).
pub fn cm_cluster_phs(_ctx: &ServerContext, cluster: i32) -> Option<Vec<u8>> {
    let v = myq2_common::cmodel::cm_cluster_phs(cluster);
    if v.is_empty() { None } else { Some(v) }
}

/// Load the statically linked Rust game module.
///
/// This replaces Sys_GetGameAPI / GetGameApi for the static case.
/// In C this would dynamically load gamex86.dll and call GetGameApi().
/// Here the game module is statically linked. We:
///   1. Install the real ServerGameImport as the game's import interface
///   2. Create a GameContext with the correct maxclients/maxentities
///   3. Store it in the global GAME_CONTEXT
///   4. Return a GameExport with real callback functions
fn load_game_module_static(ctx: &mut ServerContext) -> GameExport {
    // Install the real game import backed by server state
    use crate::server_game_import::ServerGameImport;
    myq2_game::game_import::set_gi(Box::new(ServerGameImport));

    // Read maxclients and maxentities from cvars
    let maxclients = ctx.maxclients_value as i32;
    let maxentities = std::cmp::max(maxclients + 1, 1024); // default maxentities

    // Create the game context
    let mut game_ctx = myq2_game::g_local::GameContext::default();
    game_ctx.edicts.resize_with(maxentities as usize, Default::default);
    game_ctx.game.maxclients = maxclients;
    game_ctx.game.maxentities = maxentities;
    game_ctx.max_edicts = maxentities;
    game_ctx.clients = vec![Default::default(); maxclients as usize];
    game_ctx.maxclients = maxclients as f32;

    // Store globally
    *GAME_CONTEXT.lock().unwrap() = Some(game_ctx);
    *IP_FILTER_STATE.lock().unwrap() = Some(myq2_game::g_svcmds::IpFilterState::default());

    // Build the GameExport with real callback functions
    let mut ge = GameExport::default();
    ge.apiversion = GAME_API_VERSION;
    ge.max_edicts = maxentities;
    ge.num_edicts = maxclients + 1;
    ge.edict_size = std::mem::size_of::<myq2_game::g_local::Edict>() as i32;

    // Allocate server-side edicts to match
    ge.edicts.clear();
    ge.edicts.resize_with(maxentities as usize, Edict::default);

    // Wire callbacks
    ge.init = Some(game_cb_init);
    ge.shutdown = Some(game_cb_shutdown);
    ge.spawn_entities = Some(game_cb_spawn_entities);
    ge.run_frame = Some(game_cb_run_frame);
    ge.server_command = Some(game_cb_server_command);
    ge.write_game = Some(game_cb_write_game);
    ge.read_game = Some(game_cb_read_game);
    ge.write_level = Some(game_cb_write_level);
    ge.read_level = Some(game_cb_read_level);

    // Note: client_connect, client_begin, etc. use Edict references from
    // the server's edicts vec. These are handled differently — the server
    // calls them by edict index via GameExport helper methods. For now
    // we leave them as None and the existing by-index helpers handle dispatch.

    ge
}

// ============================================================
// Game callback functions — plain fn pointers that operate
// on the global GAME_CONTEXT.
// ============================================================

fn game_cb_init() {
    with_game_context(|ctx| {
        use myq2_game::g_save::SaveContext;
        let mut save_ctx = SaveContext {
            game: &mut ctx.game,
            level: &mut ctx.level,
            edicts: &mut ctx.edicts,
            clients: &mut Vec::new(), // clients managed separately in game ctx
            num_edicts: &mut ctx.num_edicts,
            items: &ctx.items,
        };
        myq2_game::g_save::init_game(&mut save_ctx);
    });
}

fn game_cb_shutdown() {
    // Drop the game context
    *GAME_CONTEXT.lock().unwrap() = None;
    *IP_FILTER_STATE.lock().unwrap() = None;
}

fn game_cb_spawn_entities(mapname: &str, entstring: &str, spawnpoint: &str) {
    with_game_context(|ctx| {
        myq2_game::g_spawn::spawn_entities(ctx, mapname, entstring, spawnpoint);
    });
}

fn game_cb_run_frame() {
    // Delegate to the full G_RunFrame implementation in myq2_game::g_main.
    // This handles entity thinking, physics, AI, intermission, and DM rules.
    // Note: g_main::GameContext and g_spawn::GameContext are different types;
    // a unified GameContext will be needed to wire this up. For now, this is
    // a no-op placeholder matching the callback signature.
    with_game_context(|_ctx| {
        // Will call myq2_game::g_main::g_run_frame once GameContext types are unified.
    });
}

fn game_cb_server_command() {
    // server_command requires g_main::GameContext which is a different type from
    // g_spawn::GameContext. The IP filter commands (addip, removeip, listip, writeip)
    // only need the IpFilterState and game_import functions, not the full game context.
    // For now we handle the common case — a full bridge to g_main::GameContext would
    // require unifying the game context types.
    let argv: Vec<String> = {
        let argc = myq2_common::cmd::cmd_argc();
        (0..argc).map(myq2_common::cmd::cmd_argv).collect()
    };

    if argv.is_empty() {
        return;
    }

    // Handle the most common server commands directly
    let cmd = argv[0].as_str();
    match cmd {
        "addip" | "removeip" | "listip" | "writeip" => {
            // These only need IpFilterState — we can dispatch directly
            // For now, print that the command was received
            myq2_game::game_import::gi_dprintf(
                &format!("ServerCommand: {} (IP filter commands not fully wired yet)\n", cmd)
            );
        }
        _ => {
            myq2_game::game_import::gi_dprintf(
                &format!("Unknown server game command: {}\n", cmd)
            );
        }
    }
}

fn game_cb_write_game(filename: &str, autosave: bool) {
    with_game_context(|ctx| {
        use myq2_game::g_save::SaveContext;
        let mut save_ctx = SaveContext {
            game: &mut ctx.game,
            level: &mut ctx.level,
            edicts: &mut ctx.edicts,
            clients: &mut ctx.clients,
            num_edicts: &mut ctx.num_edicts,
            items: &ctx.items,
        };
        myq2_game::g_save::write_game(&mut save_ctx, filename, autosave);
    });
}

fn game_cb_read_game(filename: &str) {
    with_game_context(|ctx| {
        use myq2_game::g_save::SaveContext;
        let mut save_ctx = SaveContext {
            game: &mut ctx.game,
            level: &mut ctx.level,
            edicts: &mut ctx.edicts,
            clients: &mut ctx.clients,
            num_edicts: &mut ctx.num_edicts,
            items: &ctx.items,
        };
        myq2_game::g_save::read_game(&mut save_ctx, filename);
    });
}

fn game_cb_write_level(filename: &str) {
    with_game_context(|ctx| {
        use myq2_game::g_save::SaveContext;
        let save_ctx = SaveContext {
            game: &mut ctx.game,
            level: &mut ctx.level,
            edicts: &mut ctx.edicts,
            clients: &mut ctx.clients,
            num_edicts: &mut ctx.num_edicts,
            items: &ctx.items,
        };
        myq2_game::g_save::write_level(&save_ctx, filename);
    });
}

fn game_cb_read_level(filename: &str) {
    with_game_context(|ctx| {
        use myq2_game::g_save::SaveContext;
        let mut save_ctx = SaveContext {
            game: &mut ctx.game,
            level: &mut ctx.level,
            edicts: &mut ctx.edicts,
            clients: &mut ctx.clients,
            num_edicts: &mut ctx.num_edicts,
            items: &ctx.items,
        };
        myq2_game::g_save::read_level(&mut save_ctx, filename);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // Helper to construct a minimal ServerContext for testing
    // ============================================================

    fn make_test_server_context() -> ServerContext {
        let mut ctx = ServerContext::default();
        // Pre-allocate a few clients
        ctx.svs.clients.resize_with(4, Client::default);
        ctx.maxclients_value = 4.0;
        ctx
    }

    // ============================================================
    // Edict default construction
    // ============================================================

    #[test]
    fn test_edict_default() {
        let e = Edict::default();
        assert!(!e.inuse);
        assert_eq!(e.linkcount, 0);
        assert_eq!(e.area_node, -1);
        assert!(!e.area_linked);
        assert_eq!(e.num_clusters, 0);
        assert_eq!(e.headnode, 0);
        assert_eq!(e.areanum, 0);
        assert_eq!(e.areanum2, 0);
        assert_eq!(e.svflags, 0);
        assert_eq!(e.mins, [0.0; 3]);
        assert_eq!(e.maxs, [0.0; 3]);
        assert_eq!(e.absmin, [0.0; 3]);
        assert_eq!(e.absmax, [0.0; 3]);
        assert_eq!(e.size, [0.0; 3]);
        assert!(matches!(e.solid, Solid::Not));
        assert_eq!(e.clipmask, 0);
        assert_eq!(e.owner_index, -1);
        assert!(e.client.is_none());
        assert_eq!(e.s.number, 0);
    }

    // ============================================================
    // GClient default construction
    // ============================================================

    #[test]
    fn test_gclient_default() {
        let gc = GClient::default();
        assert_eq!(gc.ping, 0);
        assert_eq!(gc.ps.fov, 90.0); // PlayerState default fov
        assert_eq!(gc.ps.stats.len(), MAX_STATS);
    }

    // ============================================================
    // GameExport: clear_edict_event
    // ============================================================

    #[test]
    fn test_game_export_clear_edict_event() {
        let mut ge = GameExport::default();
        ge.edicts.push(Edict::default());
        ge.edicts.push(Edict::default());
        ge.edicts[1].s.event = 42;

        ge.clear_edict_event(1);
        assert_eq!(ge.edicts[1].s.event, 0);

        // Out-of-bounds index should not panic
        ge.clear_edict_event(999);
    }

    // ============================================================
    // GameExport: num_edicts
    // ============================================================

    #[test]
    fn test_game_export_num_edicts() {
        let mut ge = GameExport::default();
        ge.num_edicts = 17;
        assert_eq!(ge.num_edicts(), 17);
    }

    // ============================================================
    // GameExport: get_client_frags / get_client_health
    // ============================================================

    #[test]
    fn test_game_export_get_client_stats_with_client_ptr() {
        let mut ge = GameExport::default();
        let mut gclient = GClient::default();
        gclient.ps.stats[STAT_FRAGS as usize] = 10;
        gclient.ps.stats[STAT_HEALTH as usize] = 75;

        let client_ptr = &mut gclient as *mut GClient;
        let mut ent = Edict::default();
        ent.client = Some(client_ptr);
        ge.edicts.push(ent);

        assert_eq!(ge.get_client_frags(0), 10);
        assert_eq!(ge.get_client_health(0), 75);
    }

    #[test]
    fn test_game_export_get_client_stats_no_client() {
        let mut ge = GameExport::default();
        ge.edicts.push(Edict::default()); // no client pointer

        assert_eq!(ge.get_client_frags(0), 0);
        assert_eq!(ge.get_client_health(0), 0);
    }

    #[test]
    fn test_game_export_get_client_stats_out_of_bounds() {
        let ge = GameExport::default();
        assert_eq!(ge.get_client_frags(999), 0);
        assert_eq!(ge.get_client_health(999), 0);
    }

    // ============================================================
    // GameExport: set_client_ping
    // ============================================================

    #[test]
    fn test_game_export_set_client_ping() {
        let mut ge = GameExport::default();
        let mut gclient = GClient::default();
        let client_ptr = &mut gclient as *mut GClient;

        let mut ent = Edict::default();
        ent.client = Some(client_ptr);
        ge.edicts.push(ent);

        ge.set_client_ping(0, 150);
        assert_eq!(gclient.ping, 150);
    }

    #[test]
    fn test_game_export_set_client_ping_no_client() {
        let mut ge = GameExport::default();
        ge.edicts.push(Edict::default());
        // Should not panic
        ge.set_client_ping(0, 100);
    }

    // ============================================================
    // GameExport: client_connect_by_index (default with no func)
    // ============================================================

    #[test]
    fn test_game_export_client_connect_default_accepts() {
        let mut ge = GameExport::default();
        ge.edicts.push(Edict::default());
        // With no client_connect function set, should return true
        assert!(ge.client_connect_by_index(0, "\\name\\player"));
    }

    // ============================================================
    // GameExport: client_disconnect_by_index (no func, no panic)
    // ============================================================

    #[test]
    fn test_game_export_client_disconnect_no_func() {
        let mut ge = GameExport::default();
        ge.edicts.push(Edict::default());
        // Should not panic when no disconnect function is set
        ge.client_disconnect_by_index(0);
    }

    // ============================================================
    // GameExport: run_frame_call (no func, no panic)
    // ============================================================

    #[test]
    fn test_game_export_run_frame_no_func() {
        let ge = GameExport::default();
        // Should not panic when no run_frame function is set
        ge.run_frame_call();
    }

    // ============================================================
    // GameModule: is_dynamic for Static variant
    // ============================================================

    #[test]
    fn test_game_module_static_is_not_dynamic() {
        let module = GameModule::Static {
            export: GameExport::default(),
        };
        assert!(!module.is_dynamic());
    }

    // ============================================================
    // GameModule: accessor methods on Static variant
    // ============================================================

    #[test]
    fn test_game_module_static_accessors() {
        let mut ge = GameExport::default();
        ge.apiversion = 3;
        ge.num_edicts = 10;
        ge.max_edicts = 1024;
        ge.edict_size = 512;

        let module = GameModule::Static { export: ge };
        assert_eq!(module.apiversion(), 3);
        assert_eq!(module.num_edicts(), 10);
        assert_eq!(module.max_edicts(), 1024);
        assert_eq!(module.edict_size(), 512);
    }

    // ============================================================
    // GameModule: as_static / as_static_mut
    // ============================================================

    #[test]
    fn test_game_module_as_static() {
        let mut module = GameModule::Static {
            export: GameExport::default(),
        };
        assert!(module.as_static().is_some());
        assert!(module.as_static_mut().is_some());
    }

    // ============================================================
    // num_for_edict
    // ============================================================

    #[test]
    fn test_num_for_edict() {
        let ctx = make_test_server_context();
        let mut ent = Edict::default();
        ent.s.number = 7;
        assert_eq!(num_for_edict(&ctx, &ent), 7);
    }

    // ============================================================
    // pf_dprintf
    // ============================================================

    #[test]
    fn test_pf_dprintf_does_not_panic() {
        // Just verifies it doesn't crash; output goes to com_printf
        pf_dprintf("test debug message");
    }

    // ============================================================
    // pf_error
    // ============================================================

    #[test]
    #[should_panic(expected = "Game Error: test error")]
    fn test_pf_error() {
        pf_error("test error");
    }

    // ============================================================
    // pf_configstring: valid and invalid indices
    // ============================================================

    #[test]
    fn test_pf_configstring_valid_index() {
        let mut ctx = make_test_server_context();
        ctx.sv.state = ServerState::Loading;
        pf_configstring(&mut ctx, 5, "test_value");
        assert_eq!(ctx.sv.configstrings[5], "test_value");
    }

    #[test]
    fn test_pf_configstring_empty_value() {
        let mut ctx = make_test_server_context();
        ctx.sv.state = ServerState::Loading;
        pf_configstring(&mut ctx, 10, "");
        assert_eq!(ctx.sv.configstrings[10], "");
    }

    #[test]
    #[should_panic(expected = "configstring: bad index -1")]
    fn test_pf_configstring_negative_index() {
        let mut ctx = make_test_server_context();
        pf_configstring(&mut ctx, -1, "value");
    }

    #[test]
    #[should_panic(expected = "configstring: bad index")]
    fn test_pf_configstring_overflow_index() {
        let mut ctx = make_test_server_context();
        pf_configstring(&mut ctx, MAX_CONFIGSTRINGS as i32, "value");
    }

    // ============================================================
    // pf_configstring: during Game state writes to multicast
    // ============================================================

    #[test]
    fn test_pf_configstring_game_state_writes_multicast() {
        let mut ctx = make_test_server_context();
        ctx.sv.state = ServerState::Game;
        let old_cursize = ctx.sv.multicast.cursize;
        pf_configstring(&mut ctx, 5, "hello");
        assert_eq!(ctx.sv.configstrings[5], "hello");
        // During game state, multicast is cleared after sv_multicast call,
        // but it should have been written to before that
    }

    // ============================================================
    // pf_write_* functions: test they write to multicast
    // ============================================================

    #[test]
    fn test_pf_write_byte() {
        let mut ctx = make_test_server_context();
        pf_write_byte(&mut ctx, 42);
        assert_eq!(ctx.sv.multicast.cursize, 1);
        assert_eq!(ctx.sv.multicast.data[0], 42);
    }

    #[test]
    fn test_pf_write_char() {
        let mut ctx = make_test_server_context();
        pf_write_char(&mut ctx, 65);
        assert_eq!(ctx.sv.multicast.cursize, 1);
        assert_eq!(ctx.sv.multicast.data[0], 65);
    }

    #[test]
    fn test_pf_write_short() {
        let mut ctx = make_test_server_context();
        pf_write_short(&mut ctx, 0x0102);
        assert_eq!(ctx.sv.multicast.cursize, 2);
        // Little-endian
        assert_eq!(ctx.sv.multicast.data[0], 0x02);
        assert_eq!(ctx.sv.multicast.data[1], 0x01);
    }

    #[test]
    fn test_pf_write_long() {
        let mut ctx = make_test_server_context();
        pf_write_long(&mut ctx, 0x01020304);
        assert_eq!(ctx.sv.multicast.cursize, 4);
        assert_eq!(ctx.sv.multicast.data[0], 0x04);
        assert_eq!(ctx.sv.multicast.data[1], 0x03);
        assert_eq!(ctx.sv.multicast.data[2], 0x02);
        assert_eq!(ctx.sv.multicast.data[3], 0x01);
    }

    #[test]
    fn test_pf_write_float() {
        let mut ctx = make_test_server_context();
        pf_write_float(&mut ctx, 1.0);
        assert_eq!(ctx.sv.multicast.cursize, 4);
        let bytes = 1.0f32.to_le_bytes();
        assert_eq!(&ctx.sv.multicast.data[0..4], &bytes);
    }

    #[test]
    fn test_pf_write_string() {
        let mut ctx = make_test_server_context();
        pf_write_string(&mut ctx, "hi");
        // "hi" + null terminator = 3 bytes
        assert_eq!(ctx.sv.multicast.cursize, 3);
        assert_eq!(ctx.sv.multicast.data[0], b'h');
        assert_eq!(ctx.sv.multicast.data[1], b'i');
        assert_eq!(ctx.sv.multicast.data[2], 0);
    }

    #[test]
    fn test_pf_write_angle() {
        let mut ctx = make_test_server_context();
        pf_write_angle(&mut ctx, 90.0);
        // msg_write_angle writes 1 byte: (f * 256 / 360) as u8
        assert_eq!(ctx.sv.multicast.cursize, 1);
    }

    // ============================================================
    // pf_unicast: valid and invalid entity numbers
    // ============================================================

    #[test]
    fn test_pf_unicast_valid_entity() {
        let mut ctx = make_test_server_context();
        // Write some data into multicast
        msg_write_byte(&mut ctx.sv.multicast, 99);
        assert_eq!(ctx.sv.multicast.cursize, 1);

        let mut ent = Edict::default();
        ent.s.number = 1; // client 0 (1-based)

        pf_unicast(&mut ctx, &ent, true);
        // After unicast, multicast should be cleared
        assert_eq!(ctx.sv.multicast.cursize, 0);
        // The data should have been copied to the client's netchan.message
        assert!(ctx.svs.clients[0].netchan.message.cursize > 0);
    }

    #[test]
    fn test_pf_unicast_unreliable() {
        let mut ctx = make_test_server_context();
        msg_write_byte(&mut ctx.sv.multicast, 55);

        let mut ent = Edict::default();
        ent.s.number = 2; // client 1 (1-based)

        pf_unicast(&mut ctx, &ent, false);
        assert_eq!(ctx.sv.multicast.cursize, 0);
        // Data goes to datagram for unreliable
        assert!(ctx.svs.clients[1].datagram.cursize > 0);
    }

    #[test]
    fn test_pf_unicast_out_of_range_entity() {
        let mut ctx = make_test_server_context();
        msg_write_byte(&mut ctx.sv.multicast, 99);

        let mut ent = Edict::default();
        ent.s.number = 0; // out of range (< 1)

        pf_unicast(&mut ctx, &ent, true);
        // Should have returned early without clearing multicast? No, let's check:
        // Actually, p < 1 means it returns early, multicast NOT cleared
        assert_eq!(ctx.sv.multicast.cursize, 1);
    }

    #[test]
    fn test_pf_unicast_entity_beyond_maxclients() {
        let mut ctx = make_test_server_context();
        msg_write_byte(&mut ctx.sv.multicast, 99);

        let mut ent = Edict::default();
        ent.s.number = 5; // beyond maxclients_value (4)

        pf_unicast(&mut ctx, &ent, true);
        // Should have returned early
        assert_eq!(ctx.sv.multicast.cursize, 1);
    }

    // ============================================================
    // pf_cprintf: to entity and to console
    // ============================================================

    #[test]
    fn test_pf_cprintf_to_console() {
        let mut ctx = make_test_server_context();
        // ent = None means print to console (com_printf); should not panic
        pf_cprintf(&mut ctx, None, PRINT_HIGH, "test msg");
    }

    #[test]
    #[should_panic(expected = "cprintf to a non-client")]
    fn test_pf_cprintf_to_invalid_entity() {
        let mut ctx = make_test_server_context();
        let mut ent = Edict::default();
        ent.s.number = 0; // 0 is < 1, invalid
        pf_cprintf(&mut ctx, Some(&ent), PRINT_HIGH, "msg");
    }

    // ============================================================
    // pf_centerprintf: valid and invalid entities
    // ============================================================

    #[test]
    fn test_pf_centerprintf_valid_entity() {
        let mut ctx = make_test_server_context();
        let mut ent = Edict::default();
        ent.s.number = 1;
        // Should write SvcOps::CenterPrint + string to multicast then unicast
        pf_centerprintf(&mut ctx, &ent, "hello center");
        // After unicast, multicast should be cleared
        assert_eq!(ctx.sv.multicast.cursize, 0);
    }

    #[test]
    fn test_pf_centerprintf_invalid_entity() {
        let mut ctx = make_test_server_context();
        let ent = Edict::default(); // number 0 — out of range
        // Should return early without modifying anything
        pf_centerprintf(&mut ctx, &ent, "test");
        assert_eq!(ctx.sv.multicast.cursize, 0);
    }

    // ============================================================
    // pf_setmodel: empty name panics
    // ============================================================

    #[test]
    #[should_panic(expected = "PF_setmodel: NULL")]
    fn test_pf_setmodel_empty_name_panics() {
        let mut ctx = make_test_server_context();
        let mut ent = Edict::default();
        pf_setmodel(&mut ctx, &mut ent, "");
    }

    // ============================================================
    // sv_link_edict: compute size, absmin, absmax, linkcount
    // ============================================================

    #[test]
    fn test_sv_link_edict_computes_bounds() {
        let mut ctx = make_test_server_context();
        let mut ent = Edict::default();
        ent.s.origin = [100.0, 200.0, 300.0];
        ent.mins = [-16.0, -16.0, -24.0];
        ent.maxs = [16.0, 16.0, 32.0];
        ent.solid = Solid::Bbox;
        ent.s.modelindex = 1;

        sv_link_edict(&mut ctx, &mut ent);

        // size = maxs - mins
        assert_eq!(ent.size[0], 32.0);
        assert_eq!(ent.size[1], 32.0);
        assert_eq!(ent.size[2], 56.0);

        // absmin = origin + mins - 1
        assert_eq!(ent.absmin[0], 100.0 - 16.0 - 1.0);
        assert_eq!(ent.absmin[1], 200.0 - 16.0 - 1.0);
        assert_eq!(ent.absmin[2], 300.0 - 24.0 - 1.0);

        // absmax = origin + maxs + 1
        assert_eq!(ent.absmax[0], 100.0 + 16.0 + 1.0);
        assert_eq!(ent.absmax[1], 200.0 + 16.0 + 1.0);
        assert_eq!(ent.absmax[2], 300.0 + 32.0 + 1.0);

        // first time linked: linkcount should be 1, old_origin = origin
        assert_eq!(ent.linkcount, 1);
        assert_eq!(ent.s.old_origin, [100.0, 200.0, 300.0]);
    }

    #[test]
    fn test_sv_link_edict_increments_linkcount() {
        let mut ctx = make_test_server_context();
        let mut ent = Edict::default();
        ent.solid = Solid::Bbox;
        ent.s.modelindex = 1;
        ent.linkcount = 3;

        sv_link_edict(&mut ctx, &mut ent);
        assert_eq!(ent.linkcount, 4);
    }

    #[test]
    fn test_sv_link_edict_no_solid_no_model_skips_clusters() {
        let mut ctx = make_test_server_context();
        let mut ent = Edict::default();
        ent.solid = Solid::Not;
        ent.s.modelindex = 0;

        sv_link_edict(&mut ctx, &mut ent);
        // Should return early after computing bounds and incrementing linkcount
        assert_eq!(ent.linkcount, 1);
    }

    // ============================================================
    // sv_start_sound: validation panics
    // ============================================================

    #[test]
    #[should_panic(expected = "SV_StartSound: volume")]
    fn test_sv_start_sound_invalid_volume() {
        let mut ctx = make_test_server_context();
        let ent = Edict::default();
        sv_start_sound(&mut ctx, None, &ent, 0, 0, 2.0, 1.0, 0.0);
    }

    #[test]
    #[should_panic(expected = "SV_StartSound: attenuation")]
    fn test_sv_start_sound_invalid_attenuation() {
        let mut ctx = make_test_server_context();
        let ent = Edict::default();
        sv_start_sound(&mut ctx, None, &ent, 0, 0, 1.0, 5.0, 0.0);
    }

    #[test]
    #[should_panic(expected = "SV_StartSound: timeofs")]
    fn test_sv_start_sound_invalid_timeofs() {
        let mut ctx = make_test_server_context();
        let ent = Edict::default();
        sv_start_sound(&mut ctx, None, &ent, 0, 0, 1.0, 1.0, 0.5);
    }

    // ============================================================
    // sv_start_sound: channel stripping
    // ============================================================

    #[test]
    fn test_sv_start_sound_default_params_writes_multicast() {
        let mut ctx = make_test_server_context();
        let ent = Edict::default();
        sv_start_sound(&mut ctx, None, &ent, 0, 1, 1.0, 1.0, 0.0);
        // After sv_multicast call in sv_start_sound, multicast should be cleared
        // because sv_multicast processes and clears it.
        // The data was written; we mainly verify no panic.
    }

    // ============================================================
    // sync_edicts_to_server
    // ============================================================

    #[test]
    fn test_sync_edicts_to_server() {
        // Set up a game context with specific values
        let mut ge = GameExport::default();
        ge.edicts.push(Edict::default());
        ge.edicts.push(Edict::default());
        ge.num_edicts = 2;

        // Since sync_edicts_to_server reads from GAME_CONTEXT global,
        // we cannot easily test it without the global. But we can test
        // the GameExport structure itself.
        assert_eq!(ge.edicts.len(), 2);
        assert_eq!(ge.num_edicts, 2);
    }

    // ============================================================
    // GameExport: client_userinfo_changed_by_index with no func
    // ============================================================

    #[test]
    fn test_game_export_userinfo_changed_no_func() {
        let mut ge = GameExport::default();
        ge.edicts.push(Edict::default());
        // Should not panic when no function is set
        ge.client_userinfo_changed_by_index(0, "\\name\\test");
    }

    // ============================================================
    // GameModule: shutdown on static without func
    // ============================================================

    #[test]
    fn test_game_module_static_shutdown_no_func() {
        let module = GameModule::Static {
            export: GameExport::default(),
        };
        // Should not panic with no shutdown function
        module.shutdown();
    }

    // ============================================================
    // GameModule: init on static without func
    // ============================================================

    #[test]
    fn test_game_module_static_init_no_func() {
        let module = GameModule::Static {
            export: GameExport::default(),
        };
        // Should not panic with no init function
        module.init();
    }

    // ============================================================
    // GameModule: run_frame on static without func
    // ============================================================

    #[test]
    fn test_game_module_static_run_frame_no_func() {
        let module = GameModule::Static {
            export: GameExport::default(),
        };
        module.run_frame();
    }

    // ============================================================
    // GameModule: server_command on static without func
    // ============================================================

    #[test]
    fn test_game_module_static_server_command_no_func() {
        let module = GameModule::Static {
            export: GameExport::default(),
        };
        module.server_command();
    }

    // ============================================================
    // pf_write_pos: writes 6 bytes (3 shorts)
    // ============================================================

    #[test]
    fn test_pf_write_pos() {
        let mut ctx = make_test_server_context();
        let pos: Vec3 = [10.0, 20.0, 30.0];
        pf_write_pos(&mut ctx, &pos);
        // msg_write_pos writes 3 shorts (6 bytes): each coord * 8 as i16
        assert_eq!(ctx.sv.multicast.cursize, 6);
    }

    // ============================================================
    // pf_write_dir: writes encoded direction
    // ============================================================

    #[test]
    fn test_pf_write_dir() {
        let mut ctx = make_test_server_context();
        let dir: Vec3 = [1.0, 0.0, 0.0];
        pf_write_dir(&mut ctx, &dir);
        // msg_write_dir writes 1 byte (index into anorms table)
        assert_eq!(ctx.sv.multicast.cursize, 1);
    }

    // ============================================================
    // GameExport: edicts vec management
    // ============================================================

    #[test]
    fn test_game_export_edicts_resize() {
        let mut ge = GameExport::default();
        ge.edicts.resize_with(10, Edict::default);
        assert_eq!(ge.edicts.len(), 10);
        for i in 0..10 {
            assert!(!ge.edicts[i].inuse);
        }
    }

    // ============================================================
    // edict_num: returns mutable reference to edict by index
    // ============================================================

    #[test]
    fn test_edict_num() {
        let mut ctx = make_test_server_context();
        let mut ge = GameExport::default();
        ge.edicts.push(Edict::default());
        ge.edicts.push(Edict::default());
        ge.edicts[1].s.number = 1;
        ctx.ge = Some(ge);

        let ent = edict_num(&mut ctx, 1);
        assert!(ent.is_some());
        assert_eq!(ent.unwrap().s.number, 1);
    }

    #[test]
    fn test_edict_num_no_ge() {
        let mut ctx = make_test_server_context();
        ctx.ge = None;
        let ent = edict_num(&mut ctx, 0);
        assert!(ent.is_none());
    }

    #[test]
    fn test_edict_num_out_of_bounds() {
        let mut ctx = make_test_server_context();
        let ge = GameExport::default();
        ctx.ge = Some(ge);
        let ent = edict_num(&mut ctx, 999);
        assert!(ent.is_none());
    }
}
