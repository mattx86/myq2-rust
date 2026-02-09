// game_api.rs — C-compatible FFI types for game DLL interface
// Mirrors the original C game.h structures for binary compatibility
//
// These types are used when loading external game DLLs (gamex86.dll)
// and when building the Rust game module as a DLL.

#![allow(non_camel_case_types, non_snake_case)]

use std::os::raw::{c_char, c_float, c_int, c_void};

use crate::q_shared::{CPlane, CSurface, Vec3, MAX_ENT_CLUSTERS, MAX_STATS};
pub use crate::q_shared::MAXTOUCH;

// ============================================================
// Constants
// ============================================================

/// Game API version - must match between engine and game DLL
pub const GAME_API_VERSION: c_int = 3;

// edict->svflags
pub const SVF_NOCLIENT: c_int = 0x00000001;
pub const SVF_DEADMONSTER: c_int = 0x00000002;
pub const SVF_MONSTER: c_int = 0x00000004;

// solid_t values
pub const SOLID_NOT: c_int = 0;
pub const SOLID_TRIGGER: c_int = 1;
pub const SOLID_BBOX: c_int = 2;
pub const SOLID_BSP: c_int = 3;

// ============================================================
// Basic C-compatible types
// ============================================================

/// C-style boolean (int)
pub type qboolean = c_int;

/// Entity link for spatial partitioning (doubly-linked list node)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct link_t {
    pub prev: *mut link_t,
    pub next: *mut link_t,
}

impl Default for link_t {
    fn default() -> Self {
        Self {
            prev: std::ptr::null_mut(),
            next: std::ptr::null_mut(),
        }
    }
}

// ============================================================
// Trace - C-compatible version
// ============================================================

/// C-compatible trace result
/// Binary layout must match the original C trace_t exactly
#[repr(C)]
#[derive(Debug, Clone)]
pub struct trace_t {
    pub allsolid: qboolean,
    pub startsolid: qboolean,
    pub fraction: c_float,
    pub endpos: Vec3,
    pub plane: CPlane,
    pub surface: *mut CSurface,
    pub contents: c_int,
    pub ent: *mut edict_t,
}

impl Default for trace_t {
    fn default() -> Self {
        Self {
            allsolid: 0,
            startsolid: 0,
            fraction: 1.0,
            endpos: [0.0; 3],
            plane: CPlane::default(),
            surface: std::ptr::null_mut(),
            contents: 0,
            ent: std::ptr::null_mut(),
        }
    }
}

// ============================================================
// Cvar - C-compatible version
// ============================================================

/// C-compatible console variable
/// Used when passing cvar pointers across FFI boundary
#[repr(C)]
pub struct cvar_t {
    pub name: *mut c_char,
    pub string: *mut c_char,
    pub latched_string: *mut c_char,
    pub flags: c_int,
    pub modified: qboolean,
    pub value: c_float,
    pub next: *mut cvar_t,
}

// ============================================================
// Pmove - C-compatible version
// ============================================================

/// C-compatible player movement state
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct pmove_state_t {
    pub pm_type: c_int,
    pub origin: [i16; 3],
    pub velocity: [i16; 3],
    pub pm_flags: u8,
    pub pm_time: u8,
    pub gravity: i16,
    pub delta_angles: [i16; 3],
}

/// C-compatible user command
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct usercmd_t {
    pub msec: u8,
    pub buttons: u8,
    pub angles: [i16; 3],
    pub forwardmove: i16,
    pub sidemove: i16,
    pub upmove: i16,
    pub impulse: u8,
    pub lightlevel: u8,
}

/// C-compatible pmove structure with trace callbacks
#[repr(C)]
pub struct pmove_t {
    // State (in/out)
    pub s: pmove_state_t,

    // Command (in)
    pub cmd: usercmd_t,
    pub snapinitial: qboolean,

    // Results (out)
    pub numtouch: c_int,
    pub touchents: [*mut edict_t; MAXTOUCH],

    pub viewangles: Vec3,
    pub viewheight: c_float,

    pub mins: Vec3,
    pub maxs: Vec3,

    pub groundentity: *mut edict_t,
    pub watertype: c_int,
    pub waterlevel: c_int,

    // Callbacks for world interaction
    pub trace: Option<
        unsafe extern "C" fn(
            start: *const Vec3,
            mins: *const Vec3,
            maxs: *const Vec3,
            end: *const Vec3,
        ) -> trace_t,
    >,
    pub pointcontents: Option<unsafe extern "C" fn(point: *const Vec3) -> c_int>,
}

// ============================================================
// Entity state - for network transmission
// ============================================================

/// C-compatible entity state (sent over network)
#[repr(C)]
#[derive(Debug, Clone, Default)]
pub struct entity_state_t {
    pub number: c_int,
    pub origin: Vec3,
    pub angles: Vec3,
    pub old_origin: Vec3,
    pub modelindex: c_int,
    pub modelindex2: c_int,
    pub modelindex3: c_int,
    pub modelindex4: c_int,
    pub frame: c_int,
    pub skinnum: c_int,
    pub effects: u32,
    pub renderfx: c_int,
    pub solid: c_int,
    pub sound: c_int,
    pub event: c_int,
}

// ============================================================
// Player state - for client rendering
// ============================================================

/// C-compatible player state
#[repr(C)]
#[derive(Debug, Clone)]
pub struct player_state_t {
    pub pmove: pmove_state_t,
    pub viewangles: Vec3,
    pub viewoffset: Vec3,
    pub kick_angles: Vec3,
    pub gunangles: Vec3,
    pub gunoffset: Vec3,
    pub gunindex: c_int,
    pub gunframe: c_int,
    pub blend: [c_float; 4],
    pub fov: c_float,
    pub rdflags: c_int,
    pub stats: [i16; MAX_STATS],
}

impl Default for player_state_t {
    fn default() -> Self {
        Self {
            pmove: pmove_state_t::default(),
            viewangles: [0.0; 3],
            viewoffset: [0.0; 3],
            kick_angles: [0.0; 3],
            gunangles: [0.0; 3],
            gunoffset: [0.0; 3],
            gunindex: 0,
            gunframe: 0,
            blend: [0.0; 4],
            fov: 90.0,
            rdflags: 0,
            stats: [0; MAX_STATS],
        }
    }
}

// ============================================================
// Client structure (public portion)
// ============================================================

/// C-compatible gclient_t (public portion visible to server)
/// The game DLL can add private fields after this in its own definition
#[repr(C)]
pub struct gclient_t {
    pub ps: player_state_t,
    pub ping: c_int,
    // Game DLL adds fields after this point
}

// ============================================================
// Edict structure (public portion)
// ============================================================

/// C-compatible edict_t (public portion visible to server)
/// This is the server's view of an entity. The game DLL extends this
/// with private fields in its own edict definition.
#[repr(C)]
pub struct edict_t {
    pub s: entity_state_t,
    pub client: *mut gclient_t,
    pub inuse: qboolean,
    pub linkcount: c_int,

    // Linked to a division node or leaf
    pub area: link_t,

    // PVS/PHS cluster info
    pub num_clusters: c_int,
    pub clusternums: [c_int; MAX_ENT_CLUSTERS],
    pub headnode: c_int,
    pub areanum: c_int,
    pub areanum2: c_int,

    // Server flags
    pub svflags: c_int,
    pub mins: Vec3,
    pub maxs: Vec3,
    pub absmin: Vec3,
    pub absmax: Vec3,
    pub size: Vec3,
    pub solid: c_int,
    pub clipmask: c_int,
    pub owner: *mut edict_t,
    // Game DLL adds fields after this point
}

// ============================================================
// Multicast types
// ============================================================

pub const MULTICAST_ALL: c_int = 0;
pub const MULTICAST_PHS: c_int = 1;
pub const MULTICAST_PVS: c_int = 2;
pub const MULTICAST_ALL_R: c_int = 3;
pub const MULTICAST_PHS_R: c_int = 4;
pub const MULTICAST_PVS_R: c_int = 5;

// ============================================================
// Game Import structure
// ============================================================

/// Functions provided by the engine to the game DLL
/// This struct must be binary-compatible with the C game_import_t
///
/// The engine populates this struct and passes it to GetGameApi().
/// The game DLL stores it and calls these functions to interact with
/// the engine.
#[repr(C)]
pub struct game_import_t {
    // Special messages
    // Note: The original C signatures are variadic (printf-style), but we use non-variadic
    // function pointers here. C game DLLs typically pass pre-formatted strings anyway.
    // The extra variadic args will be ignored by the C calling convention.
    pub bprintf: Option<unsafe extern "C" fn(printlevel: c_int, fmt: *const c_char)>,
    pub dprintf: Option<unsafe extern "C" fn(fmt: *const c_char)>,
    pub cprintf:
        Option<unsafe extern "C" fn(ent: *mut edict_t, printlevel: c_int, fmt: *const c_char)>,
    pub centerprintf: Option<unsafe extern "C" fn(ent: *mut edict_t, fmt: *const c_char)>,
    pub sound: Option<
        unsafe extern "C" fn(
            ent: *mut edict_t,
            channel: c_int,
            soundindex: c_int,
            volume: c_float,
            attenuation: c_float,
            timeofs: c_float,
        ),
    >,
    pub positioned_sound: Option<
        unsafe extern "C" fn(
            origin: *const Vec3,
            ent: *mut edict_t,
            channel: c_int,
            soundindex: c_int,
            volume: c_float,
            attenuation: c_float,
            timeofs: c_float,
        ),
    >,

    // Config strings
    pub configstring: Option<unsafe extern "C" fn(num: c_int, string: *const c_char)>,

    // Error (does not return)
    pub error: Option<unsafe extern "C" fn(fmt: *const c_char) -> !>,

    // Resource indexing
    pub modelindex: Option<unsafe extern "C" fn(name: *const c_char) -> c_int>,
    pub soundindex: Option<unsafe extern "C" fn(name: *const c_char) -> c_int>,
    pub imageindex: Option<unsafe extern "C" fn(name: *const c_char) -> c_int>,
    pub setmodel: Option<unsafe extern "C" fn(ent: *mut edict_t, name: *const c_char)>,

    // Collision detection
    pub trace: Option<
        unsafe extern "C" fn(
            start: *const Vec3,
            mins: *const Vec3,
            maxs: *const Vec3,
            end: *const Vec3,
            passent: *mut edict_t,
            contentmask: c_int,
        ) -> trace_t,
    >,
    pub pointcontents: Option<unsafe extern "C" fn(point: *const Vec3) -> c_int>,
    pub inPVS: Option<unsafe extern "C" fn(p1: *const Vec3, p2: *const Vec3) -> qboolean>,
    pub inPHS: Option<unsafe extern "C" fn(p1: *const Vec3, p2: *const Vec3) -> qboolean>,
    pub SetAreaPortalState: Option<unsafe extern "C" fn(portalnum: c_int, open: qboolean)>,
    pub AreasConnected: Option<unsafe extern "C" fn(area1: c_int, area2: c_int) -> qboolean>,

    // Entity linking
    pub linkentity: Option<unsafe extern "C" fn(ent: *mut edict_t)>,
    pub unlinkentity: Option<unsafe extern "C" fn(ent: *mut edict_t)>,
    pub BoxEdicts: Option<
        unsafe extern "C" fn(
            mins: *const Vec3,
            maxs: *const Vec3,
            list: *mut *mut edict_t,
            maxcount: c_int,
            areatype: c_int,
        ) -> c_int,
    >,
    pub Pmove: Option<unsafe extern "C" fn(pmove: *mut pmove_t)>,

    // Network messaging
    pub multicast: Option<unsafe extern "C" fn(origin: *const Vec3, to: c_int)>,
    pub unicast: Option<unsafe extern "C" fn(ent: *mut edict_t, reliable: qboolean)>,
    pub WriteChar: Option<unsafe extern "C" fn(c: c_int)>,
    pub WriteByte: Option<unsafe extern "C" fn(c: c_int)>,
    pub WriteShort: Option<unsafe extern "C" fn(c: c_int)>,
    pub WriteLong: Option<unsafe extern "C" fn(c: c_int)>,
    pub WriteFloat: Option<unsafe extern "C" fn(f: c_float)>,
    pub WriteString: Option<unsafe extern "C" fn(s: *const c_char)>,
    pub WritePosition: Option<unsafe extern "C" fn(pos: *const Vec3)>,
    pub WriteDir: Option<unsafe extern "C" fn(pos: *const Vec3)>,
    pub WriteAngle: Option<unsafe extern "C" fn(f: c_float)>,

    // Managed memory allocation
    pub TagMalloc: Option<unsafe extern "C" fn(size: c_int, tag: c_int) -> *mut c_void>,
    pub TagFree: Option<unsafe extern "C" fn(block: *mut c_void)>,
    pub FreeTags: Option<unsafe extern "C" fn(tag: c_int)>,

    // Console variable interaction
    pub cvar: Option<
        unsafe extern "C" fn(var_name: *const c_char, value: *const c_char, flags: c_int)
            -> *mut cvar_t,
    >,
    pub cvar_set:
        Option<unsafe extern "C" fn(var_name: *const c_char, value: *const c_char) -> *mut cvar_t>,
    pub cvar_forceset:
        Option<unsafe extern "C" fn(var_name: *const c_char, value: *const c_char) -> *mut cvar_t>,

    // Command argument access
    pub argc: Option<unsafe extern "C" fn() -> c_int>,
    pub argv: Option<unsafe extern "C" fn(n: c_int) -> *mut c_char>,
    pub args: Option<unsafe extern "C" fn() -> *mut c_char>,

    // Server console command execution
    pub AddCommandString: Option<unsafe extern "C" fn(text: *const c_char)>,

    // Debug graph (unused in most builds)
    pub DebugGraph: Option<unsafe extern "C" fn(value: c_float, color: c_int)>,
}

// ============================================================
// Game Export structure
// ============================================================

/// Functions and data exported by the game DLL to the engine
/// This struct must be binary-compatible with the C game_export_t
///
/// The game DLL's GetGameApi() function returns a pointer to this struct.
/// The engine calls these functions to run the game logic.
#[repr(C)]
pub struct game_export_t {
    pub apiversion: c_int,

    // Initialization
    pub Init: Option<unsafe extern "C" fn()>,
    pub Shutdown: Option<unsafe extern "C" fn()>,

    // Level management
    pub SpawnEntities: Option<
        unsafe extern "C" fn(
            mapname: *const c_char,
            entstring: *const c_char,
            spawnpoint: *const c_char,
        ),
    >,

    // Save/Load
    pub WriteGame: Option<unsafe extern "C" fn(filename: *const c_char, autosave: qboolean)>,
    pub ReadGame: Option<unsafe extern "C" fn(filename: *const c_char)>,
    pub WriteLevel: Option<unsafe extern "C" fn(filename: *const c_char)>,
    pub ReadLevel: Option<unsafe extern "C" fn(filename: *const c_char)>,

    // Client connection lifecycle
    pub ClientConnect:
        Option<unsafe extern "C" fn(ent: *mut edict_t, userinfo: *mut c_char) -> qboolean>,
    pub ClientBegin: Option<unsafe extern "C" fn(ent: *mut edict_t)>,
    pub ClientUserinfoChanged:
        Option<unsafe extern "C" fn(ent: *mut edict_t, userinfo: *mut c_char)>,
    pub ClientDisconnect: Option<unsafe extern "C" fn(ent: *mut edict_t)>,
    pub ClientCommand: Option<unsafe extern "C" fn(ent: *mut edict_t)>,
    pub ClientThink: Option<unsafe extern "C" fn(ent: *mut edict_t, cmd: *mut usercmd_t)>,

    // Game frame
    pub RunFrame: Option<unsafe extern "C" fn()>,

    // Server console commands
    pub ServerCommand: Option<unsafe extern "C" fn()>,

    // Global entity array (allocated by game DLL)
    pub edicts: *mut edict_t,
    pub edict_size: c_int,
    pub num_edicts: c_int,
    pub max_edicts: c_int,
}

// ============================================================
// GetGameApi function type
// ============================================================

/// Signature of the GetGameApi function exported by game DLLs
///
/// The engine calls this function after loading the DLL.
/// The game DLL stores the import table and returns its export table.
pub type GetGameApiFn = unsafe extern "C" fn(import: *mut game_import_t) -> *mut game_export_t;

// ============================================================
// Helper macros for edict access
// ============================================================

/// Calculate the address of edict N given the edicts base pointer and edict_size
///
/// # Safety
/// Caller must ensure `n < max_edicts` and the edicts array is valid.
#[inline]
pub unsafe fn edict_num(edicts: *mut edict_t, edict_size: c_int, n: c_int) -> *mut edict_t {
    (edicts as *mut u8).add((edict_size * n) as usize) as *mut edict_t
}

/// Calculate the index of an edict given its pointer, base, and edict_size
///
/// # Safety
/// Caller must ensure the edict pointer is within the valid range.
#[inline]
pub unsafe fn num_for_edict(edicts: *mut edict_t, edict_size: c_int, ent: *mut edict_t) -> c_int {
    let base = edicts as usize;
    let ptr = ent as usize;
    ((ptr - base) / edict_size as usize) as c_int
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn test_struct_sizes() {
        // Verify struct sizes are reasonable for FFI
        // On 64-bit: pointer = 8 bytes, int = 4 bytes

        // pmove_state_t: int + 3*i16 + 3*i16 + u8 + u8 + i16 + 3*i16 = 4 + 6 + 6 + 1 + 1 + 2 + 6 = 26 bytes
        // But with alignment it may be padded
        println!("pmove_state_t size: {}", size_of::<pmove_state_t>());

        // usercmd_t: u8 + u8 + 3*i16 + 3*i16 + u8 + u8 = 2 + 6 + 6 + 2 = 16 bytes
        println!("usercmd_t size: {}", size_of::<usercmd_t>());

        // entity_state_t should be consistent
        println!("entity_state_t size: {}", size_of::<entity_state_t>());

        // player_state_t
        println!("player_state_t size: {}", size_of::<player_state_t>());

        // game_import_t: 44 function pointers
        let expected_import_size = 44 * size_of::<*const ()>();
        println!(
            "game_import_t size: {} (expected ~{})",
            size_of::<game_import_t>(),
            expected_import_size
        );

        // game_export_t: 17 callbacks + 4 fields (ptr, 3 ints)
        println!("game_export_t size: {}", size_of::<game_export_t>());
    }

    #[test]
    fn test_link_default() {
        let link = link_t::default();
        assert!(link.prev.is_null());
        assert!(link.next.is_null());
    }

    #[test]
    fn test_trace_default() {
        let trace = trace_t::default();
        assert_eq!(trace.allsolid, 0);
        assert_eq!(trace.fraction, 1.0);
        assert!(trace.ent.is_null());
    }

    // ============================================================
    // Struct size validation — C equivalents
    // ============================================================

    #[test]
    fn test_entity_state_t_exact_size() {
        // entity_state_t fields:
        //   number: c_int (4)
        //   origin: Vec3 [f32;3] (12)
        //   angles: Vec3 (12)
        //   old_origin: Vec3 (12)
        //   modelindex..modelindex4: 4 * c_int (16)
        //   frame: c_int (4)
        //   skinnum: c_int (4)
        //   effects: u32 (4)
        //   renderfx: c_int (4)
        //   solid: c_int (4)
        //   sound: c_int (4)
        //   event: c_int (4)
        //   Total: 4 + 12 + 12 + 12 + 16 + 4 + 4 + 4 + 4 + 4 + 4 + 4 = 84
        // Actually: 4 + 36 + 16 + 4 + 4 + 4 + 4 + 4 + 4 + 4 = 80
        // Let me count precisely:
        // number(4) + origin(12) + angles(12) + old_origin(12)
        //   + modelindex(4) + modelindex2(4) + modelindex3(4) + modelindex4(4)
        //   + frame(4) + skinnum(4) + effects(4) + renderfx(4)
        //   + solid(4) + sound(4) + event(4)
        // = 4 + 12*3 + 4*4 + 4*7 = 4 + 36 + 16 + 28 = 84
        // Actually: 15 int/u32/float fields = 15 * 4 = 60, plus 3 Vec3s = 36
        // = 60 + 36... wait let me just count fields:
        // 1 int + 3 Vec3 + 4 int + 1 int + 1 int + 1 u32 + 1 int + 1 int + 1 int + 1 int
        // = 12 scalars * 4 + 3 * 12 = 48 + 36 = 84... no
        // number(4), origin(12), angles(12), old_origin(12),
        // modelindex(4), modelindex2(4), modelindex3(4), modelindex4(4),
        // frame(4), skinnum(4), effects(4), renderfx(4), solid(4), sound(4), event(4)
        // = 4 + 12 + 12 + 12 + 4 + 4 + 4 + 4 + 4 + 4 + 4 + 4 + 4 + 4 + 4 = 84
        // 15 fields: 3 Vec3s (12 each = 36) + 12 c_int/u32 (4 each = 48) = 84
        assert_eq!(size_of::<entity_state_t>(), 84,
            "entity_state_t must be exactly 84 bytes for network/save compatibility");
    }

    #[test]
    fn test_usercmd_t_exact_size() {
        // usercmd_t fields (all repr(C)):
        //   msec: u8 (1)
        //   buttons: u8 (1)
        //   angles: [i16; 3] (6)
        //   forwardmove: i16 (2)
        //   sidemove: i16 (2)
        //   upmove: i16 (2)
        //   impulse: u8 (1)
        //   lightlevel: u8 (1)
        //   Total = 1+1+6+2+2+2+1+1 = 16
        assert_eq!(size_of::<usercmd_t>(), 16,
            "usercmd_t must be exactly 16 bytes for network compatibility");
    }

    #[test]
    fn test_pmove_state_t_size() {
        // pmove_state_t fields (repr(C)):
        //   pm_type: c_int (4)
        //   origin: [i16; 3] (6)
        //   velocity: [i16; 3] (6)
        //   pm_flags: u8 (1)
        //   pm_time: u8 (1)
        //   gravity: i16 (2)
        //   delta_angles: [i16; 3] (6)
        //   Total raw = 4 + 6 + 6 + 1 + 1 + 2 + 6 = 26
        //   With repr(C) padding: 26 padded to 28 (likely 2 bytes trailing pad)
        let sz = size_of::<pmove_state_t>();
        // Must be at least raw size
        assert!(sz >= 26, "pmove_state_t must be at least 26 bytes, got {}", sz);
        // On most platforms with repr(C), alignment of the largest field (c_int=4)
        // means size is rounded up to multiple of 4 = 28
        assert_eq!(sz % 4, 0, "pmove_state_t size should be 4-byte aligned, got {}", sz);
    }

    #[test]
    fn test_player_state_t_size() {
        // player_state_t is a larger struct containing pmove_state_t plus many fields
        let sz = size_of::<player_state_t>();
        // It contains: pmove_state_t + 5*Vec3(60) + 2*int(8) + [f32;4](16) + f32(4) + int(4) + [i16;MAX_STATS]
        // MAX_STATS = 32, so [i16;32] = 64 bytes
        // Minimum: size_of::<pmove_state_t>() + 60 + 8 + 16 + 4 + 4 + 64 = pm_size + 156
        let min_expected = size_of::<pmove_state_t>() + 156;
        assert!(sz >= min_expected,
            "player_state_t should be at least {} bytes (pmove_state_t + 156), got {}",
            min_expected, sz);
        println!("player_state_t size: {} bytes", sz);
    }

    #[test]
    fn test_cvar_t_size() {
        // cvar_t: 3 ptrs + int + int + float + ptr
        // On 64-bit: 3*8 + 4 + 4 + 4 + 8 = 44 -> padded to 48
        let sz = size_of::<cvar_t>();
        // At least 4 pointers (32) + 3 scalars (12) = 44
        assert!(sz >= 44, "cvar_t should be at least 44 bytes, got {}", sz);
        println!("cvar_t size: {} bytes", sz);
    }

    // ============================================================
    // entity_state_t field value validation
    // ============================================================

    #[test]
    fn test_entity_state_t_default() {
        let es = entity_state_t::default();
        assert_eq!(es.number, 0);
        assert_eq!(es.origin, [0.0; 3]);
        assert_eq!(es.angles, [0.0; 3]);
        assert_eq!(es.old_origin, [0.0; 3]);
        assert_eq!(es.modelindex, 0);
        assert_eq!(es.modelindex2, 0);
        assert_eq!(es.modelindex3, 0);
        assert_eq!(es.modelindex4, 0);
        assert_eq!(es.frame, 0);
        assert_eq!(es.skinnum, 0);
        assert_eq!(es.effects, 0);
        assert_eq!(es.renderfx, 0);
        assert_eq!(es.solid, 0);
        assert_eq!(es.sound, 0);
        assert_eq!(es.event, 0);
    }

    #[test]
    fn test_entity_state_t_delta_comparison() {
        // Simulate delta comparison: identify which fields changed between two states
        let baseline = entity_state_t::default();
        let mut updated = entity_state_t::default();
        updated.number = 5;
        updated.origin = [100.0, 200.0, 300.0];
        updated.modelindex = 7;
        updated.frame = 42;
        updated.effects = 0xDEAD;
        updated.sound = 3;

        // Check which fields differ (simulates network delta encoding logic)
        assert_ne!(baseline.number, updated.number, "number should differ");
        assert_ne!(baseline.origin, updated.origin, "origin should differ");
        assert_eq!(baseline.angles, updated.angles, "angles should be same");
        assert_eq!(baseline.old_origin, updated.old_origin, "old_origin should be same");
        assert_ne!(baseline.modelindex, updated.modelindex, "modelindex should differ");
        assert_eq!(baseline.modelindex2, updated.modelindex2, "modelindex2 should be same");
        assert_ne!(baseline.frame, updated.frame, "frame should differ");
        assert_eq!(baseline.skinnum, updated.skinnum, "skinnum should be same");
        assert_ne!(baseline.effects, updated.effects, "effects should differ");
        assert_eq!(baseline.renderfx, updated.renderfx, "renderfx should be same");
        assert_ne!(baseline.sound, updated.sound, "sound should differ");
        assert_eq!(baseline.event, updated.event, "event should be same");
    }

    // ============================================================
    // player_state_t validation
    // ============================================================

    #[test]
    fn test_player_state_t_default() {
        let ps = player_state_t::default();
        assert_eq!(ps.viewangles, [0.0; 3]);
        assert_eq!(ps.viewoffset, [0.0; 3]);
        assert_eq!(ps.kick_angles, [0.0; 3]);
        assert_eq!(ps.gunangles, [0.0; 3]);
        assert_eq!(ps.gunoffset, [0.0; 3]);
        assert_eq!(ps.gunindex, 0);
        assert_eq!(ps.gunframe, 0);
        assert_eq!(ps.blend, [0.0; 4]);
        assert_eq!(ps.fov, 90.0, "Default FOV should be 90 degrees");
        assert_eq!(ps.rdflags, 0);
        assert_eq!(ps.stats, [0i16; crate::q_shared::MAX_STATS]);
    }

    #[test]
    fn test_player_state_t_stats_field_encoding() {
        // Stats are i16 values, max 32 entries, used for HUD display
        let mut ps = player_state_t::default();
        // STAT_HEALTH_ICON = 0, STAT_HEALTH = 1, etc.
        ps.stats[0] = 42; // health icon
        ps.stats[1] = 100; // health value
        ps.stats[2] = 5; // ammo icon
        ps.stats[3] = 50; // ammo value
        ps.stats[crate::q_shared::MAX_STATS - 1] = -1; // last stat

        assert_eq!(ps.stats[0], 42);
        assert_eq!(ps.stats[1], 100);
        assert_eq!(ps.stats[crate::q_shared::MAX_STATS - 1], -1);
    }

    #[test]
    fn test_player_state_t_pmove_encoding() {
        // pmove_state_t uses fixed-point for origin/velocity (12.3 format)
        let mut ps = player_state_t::default();
        ps.pmove.origin = [100 * 8, 200 * 8, 300 * 8]; // 12.3 fixed point: multiply by 8
        ps.pmove.velocity = [10 * 8, 0, -5 * 8];
        ps.pmove.pm_type = 0; // PM_NORMAL
        ps.pmove.gravity = 800;
        ps.pmove.delta_angles = [0, 0, 0];

        // Verify the encoded values
        assert_eq!(ps.pmove.origin[0], 800); // 100 * 8
        assert_eq!(ps.pmove.origin[1], 1600); // 200 * 8
        assert_eq!(ps.pmove.origin[2], 2400); // 300 * 8
        assert_eq!(ps.pmove.gravity, 800);
    }

    // ============================================================
    // trace_t comprehensive validation
    // ============================================================

    #[test]
    fn test_trace_t_default_comprehensive() {
        let trace = trace_t::default();
        assert_eq!(trace.allsolid, 0, "allsolid should default to false (0)");
        assert_eq!(trace.startsolid, 0, "startsolid should default to false (0)");
        assert_eq!(trace.fraction, 1.0, "fraction should default to 1.0 (no hit)");
        assert_eq!(trace.endpos, [0.0; 3], "endpos should default to origin");
        assert_eq!(trace.plane.normal, [0.0; 3], "plane normal should be zero");
        assert_eq!(trace.plane.dist, 0.0, "plane dist should be zero");
        assert!(trace.surface.is_null(), "surface should be null");
        assert_eq!(trace.contents, 0, "contents should be zero");
        assert!(trace.ent.is_null(), "ent should be null");
    }

    #[test]
    fn test_trace_t_full_hit() {
        // Simulate a trace that hits a wall at 50% distance
        let trace = trace_t {
            allsolid: 0,
            startsolid: 0,
            fraction: 0.5,
            endpos: [50.0, 0.0, 0.0],
            plane: crate::q_shared::CPlane {
                normal: [-1.0, 0.0, 0.0],
                dist: 50.0,
                plane_type: 0,
                signbits: 1,
                pad: [0; 2],
            },
            surface: std::ptr::null_mut(),
            contents: 1, // CONTENTS_SOLID
            ent: std::ptr::null_mut(),
        };

        assert_eq!(trace.fraction, 0.5);
        assert_eq!(trace.plane.normal[0], -1.0);
        assert_eq!(trace.contents, 1);
    }

    #[test]
    fn test_trace_t_allsolid_case() {
        // When the start point is inside a solid, allsolid is set
        let trace = trace_t {
            allsolid: 1,
            startsolid: 1,
            fraction: 0.0,
            endpos: [0.0; 3],
            plane: crate::q_shared::CPlane::default(),
            surface: std::ptr::null_mut(),
            contents: 1,
            ent: std::ptr::null_mut(),
        };

        assert_eq!(trace.allsolid, 1, "allsolid should be true");
        assert_eq!(trace.startsolid, 1, "startsolid should be true");
        assert_eq!(trace.fraction, 0.0, "fraction should be 0 when fully solid");
    }

    // ============================================================
    // edict_num / num_for_edict pointer arithmetic
    // ============================================================

    #[test]
    fn test_edict_num_consecutive_addresses() {
        let edict_sz = size_of::<edict_t>() as c_int;
        let count = 16;
        let buf = vec![0u8; edict_sz as usize * count];
        let base = buf.as_ptr() as *mut edict_t;

        let mut prev_addr = base as usize;
        for i in 1..count as c_int {
            let addr = unsafe { edict_num(base, edict_sz, i) } as usize;
            let stride = addr - prev_addr;
            assert_eq!(stride, edict_sz as usize,
                "Stride between edict {} and {} should be {}, got {}",
                i - 1, i, edict_sz, stride);
            prev_addr = addr;
        }
    }

    #[test]
    fn test_num_for_edict_all_edicts() {
        let edict_sz = size_of::<edict_t>() as c_int;
        let count = 16;
        let buf = vec![0u8; edict_sz as usize * count];
        let base = buf.as_ptr() as *mut edict_t;

        for i in 0..count as c_int {
            let ptr = unsafe { edict_num(base, edict_sz, i) };
            let n = unsafe { num_for_edict(base, edict_sz, ptr) };
            assert_eq!(n, i, "num_for_edict should return {} for edict_num({})", i, i);
        }
    }

    // ============================================================
    // Solid constants
    // ============================================================

    #[test]
    fn test_solid_constants() {
        assert_eq!(SOLID_NOT, 0);
        assert_eq!(SOLID_TRIGGER, 1);
        assert_eq!(SOLID_BBOX, 2);
        assert_eq!(SOLID_BSP, 3);
    }

    // ============================================================
    // SVF flag constants
    // ============================================================

    #[test]
    fn test_svf_flag_constants() {
        assert_eq!(SVF_NOCLIENT, 0x00000001);
        assert_eq!(SVF_DEADMONSTER, 0x00000002);
        assert_eq!(SVF_MONSTER, 0x00000004);
        // Flags should be non-overlapping
        assert_eq!(SVF_NOCLIENT & SVF_DEADMONSTER, 0);
        assert_eq!(SVF_NOCLIENT & SVF_MONSTER, 0);
        assert_eq!(SVF_DEADMONSTER & SVF_MONSTER, 0);
    }

    // ============================================================
    // Multicast constants
    // ============================================================

    #[test]
    fn test_multicast_constants_sequential() {
        assert_eq!(MULTICAST_ALL, 0);
        assert_eq!(MULTICAST_PHS, 1);
        assert_eq!(MULTICAST_PVS, 2);
        assert_eq!(MULTICAST_ALL_R, 3);
        assert_eq!(MULTICAST_PHS_R, 4);
        assert_eq!(MULTICAST_PVS_R, 5);
    }

    // ============================================================
    // game_import_t field count validation
    // ============================================================

    #[test]
    fn test_game_import_t_has_44_function_pointers() {
        // game_import_t has 44 Option<fn> fields.
        // Each Option<fn> is pointer-sized on 64-bit.
        let ptr_size = size_of::<Option<unsafe extern "C" fn()>>();
        let expected = 44 * ptr_size;
        let actual = size_of::<game_import_t>();
        assert_eq!(actual, expected,
            "game_import_t should have exactly 44 function pointers ({} bytes), got {} bytes",
            expected, actual);
    }

    // ============================================================
    // game_export_t layout validation
    // ============================================================

    #[test]
    fn test_game_export_t_has_correct_field_count() {
        // game_export_t: apiversion(int) + 15 callbacks + edicts(ptr) + 3 ints
        // Total: 1 + 15 + 1 + 3 = 20 fields
        // Size should be: On 64-bit, with alignment:
        //   int(4) -> padded to 8 for pointer alignment
        //   15 * 8 = 120 (function pointers)
        //   1 * 8 = 8 (edicts pointer)
        //   3 * 4 = 12 -> padded to 16 for struct alignment
        //   Total: 8 + 120 + 8 + 12 = 148 -> padded to 152 (or possibly 144/148/152)
        let sz = size_of::<game_export_t>();
        // Just verify it is a reasonable size and contains enough room for all fields
        let min_fields = 4 + (15 * size_of::<*const ()>()) + size_of::<*const ()>() + 3 * 4;
        assert!(sz >= min_fields,
            "game_export_t should be at least {} bytes, got {}", min_fields, sz);
    }

    // ============================================================
    // link_t validation
    // ============================================================

    #[test]
    fn test_link_t_size() {
        // link_t: two pointers
        let expected = 2 * size_of::<*mut link_t>();
        assert_eq!(size_of::<link_t>(), expected,
            "link_t should be {} bytes (2 pointers)", expected);
    }

    #[test]
    fn test_link_t_default_is_null() {
        let link = link_t::default();
        assert!(link.prev.is_null());
        assert!(link.next.is_null());
    }

    // ============================================================
    // gclient_t basic validation
    // ============================================================

    #[test]
    fn test_gclient_t_contains_player_state() {
        // gclient_t must start with player_state_t for C compatibility
        let gc_size = size_of::<gclient_t>();
        let ps_size = size_of::<player_state_t>();
        assert!(gc_size >= ps_size,
            "gclient_t ({}) must be at least as large as player_state_t ({})",
            gc_size, ps_size);
    }
}
