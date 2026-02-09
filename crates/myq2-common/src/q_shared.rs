// q_shared.rs — foundational types and functions shared by all modules
// Converted from: myq2-original/game/q_shared.h + q_shared.c

// ============================================================
// Basic types
// ============================================================

pub type Vec3 = [f32; 3];
pub type Vec5 = [f32; 5];

// ============================================================
// Renderer interface types (from ref.h)
// These are shared between client, renderer, and sys crates.
// ============================================================

/// viddef_t — global video definition (screen width/height).
/// Matches C `viddef_t` from `vid.h` / `gl_local.h`.
/// Fields use `i32` because the values are pervasively used in signed
/// screen-coordinate arithmetic throughout the client and menu code.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct VidDef {
    pub width: i32,
    pub height: i32,
}

/// staintype_t
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(C)]
pub enum StainType {
    #[default]
    Add = 0,
    Modulate = 1,
    Subtract = 2,
}

/// dlight_t — dynamic light
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct DLight {
    pub origin: Vec3,
    pub color: Vec3,
    pub intensity: f32,
}

/// particle_t — renderer particle
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct Particle {
    pub origin: Vec3,
    pub length: Vec3,
    pub particle_type: i32,
    pub color: i32,
    pub alpha: f32,
}

// Particle types (from particles.h)
pub const PT_DEFAULT: i32 = 0;
pub const PT_FIRE: i32 = 1;
pub const PT_SMOKE: i32 = 2;
pub const PT_BUBBLE: i32 = 3;
pub const PT_BLOOD: i32 = 4;
pub const PT_MAX: i32 = PT_BLOOD + 1;

/// lightstyle_t
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct LightStyle {
    pub rgb: [f32; 3],     // 0.0 - 2.0
    pub white: f32,        // highest of rgb
}

/// dstain_t — dynamic stain
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct DStain {
    pub origin: Vec3,
    pub color: Vec3,
    pub alpha: f32,
    pub intensity: f32,
    pub stain_type: StainType,
}

/// Opaque model handle (`struct model_s *` in C).
/// The renderer defines the actual contents; this is an opaque marker type
/// used in the shared `RefEntity` and `RefRefDef` structs.
#[repr(C)]
pub struct RefModel {
    _opaque: [u8; 0],
}

/// Opaque image handle (`struct image_s *` in C).
/// The renderer defines the actual contents; this is an opaque marker type
/// used in the shared `RefEntity` struct.
#[repr(C)]
pub struct RefImage {
    _opaque: [u8; 0],
}

/// imagetype_t — image classification for the renderer (from client.h).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ImageType {
    Skin = 0,
    Sprite = 1,
    Wall = 2,
    Pic = 3,
    Sky = 4,
}

/// entity_t — passed to the renderer for drawing.
/// This is the canonical `#[repr(C)]` definition matching the C struct layout
/// from `ref.h`. Both the client and renderer share this definition.
#[repr(C)]
pub struct RefEntity {
    pub model: *mut RefModel,    // opaque type outside refresh
    pub angles: Vec3,

    // most recent data
    pub origin: Vec3,            // also used as RF_BEAM's "from"
    pub frame: i32,              // also used as RF_BEAM's diameter

    // previous data for lerping
    pub oldorigin: Vec3,         // also used as RF_BEAM's "to"
    pub oldframe: i32,

    // misc
    pub backlerp: f32,           // 0.0 = current, 1.0 = old
    pub skinnum: i32,            // also used as RF_BEAM's palette index

    pub lightstyle: i32,         // for flashing entities
    pub alpha: f32,              // ignore if RF_TRANSLUCENT isn't set

    pub skin: *mut RefImage,     // NULL for inline skin
    pub flags: i32,
}

impl Default for RefEntity {
    fn default() -> Self {
        // SAFETY: All-zero is valid for this repr(C) struct (null pointers, zero scalars).
        unsafe { std::mem::zeroed() }
    }
}

// SAFETY: Raw pointers in RefEntity are not Send/Sync by default.
// These are only used on the main thread in the engine's render path,
// matching the original single-threaded C design.
unsafe impl Send for RefEntity {}
unsafe impl Sync for RefEntity {}

/// refdef_t — rendering parameters for a frame.
/// This is the canonical `#[repr(C)]` definition matching the C struct layout
/// from `ref.h`. Both the client and renderer share this definition.
#[repr(C)]
pub struct RefRefDef {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub fov_x: f32,
    pub fov_y: f32,
    pub vieworg: Vec3,
    pub viewangles: Vec3,
    pub blend: [f32; 4],             // rgba 0-1 full screen blend
    pub time: f32,                   // time is used to auto animate
    pub rdflags: i32,                // RDF_UNDERWATER, etc

    pub areabits: *mut u8,           // if not NULL, only areas with set bits will be drawn

    pub lightstyles: *mut LightStyle, // [MAX_LIGHTSTYLES]

    pub num_entities: i32,
    pub entities: *mut RefEntity,

    pub num_dlights: i32,
    pub dlights: *mut DLight,

    pub num_particles: i32,
    pub particles: *mut Particle,
}

impl Default for RefRefDef {
    fn default() -> Self {
        // SAFETY: All-zero is valid for this repr(C) struct (null pointers, zero scalars).
        unsafe { std::mem::zeroed() }
    }
}

// SAFETY: Raw pointers in RefRefDef are not Send/Sync by default.
// These are only used on the main thread in the engine's render path,
// matching the original single-threaded C design.
unsafe impl Send for RefRefDef {}
unsafe impl Sync for RefRefDef {}

impl RefRefDef {
    /// Get a reference to the lightstyle at the given index.
    ///
    /// # Safety
    /// Caller must ensure `lightstyles` is valid and `idx` is in bounds.
    pub unsafe fn lightstyle(&self, idx: usize) -> &LightStyle {
        &*self.lightstyles.add(idx)
    }

    /// Get a reference to the dynamic light at the given index.
    ///
    /// # Safety
    /// Caller must ensure `dlights` is valid and `idx` is in bounds.
    pub unsafe fn dlight(&self, idx: usize) -> &DLight {
        &*self.dlights.add(idx)
    }
}

pub const VEC3_ORIGIN: Vec3 = [0.0, 0.0, 0.0];

/// Lowercase alias for VEC3_ORIGIN, matching C naming convention used by game code.
#[allow(non_upper_case_globals)]
pub const vec3_origin: Vec3 = [0.0, 0.0, 0.0];

// Angle indexes
pub const PITCH: usize = 0; // up / down
pub const YAW: usize = 1; // left / right
pub const ROLL: usize = 2; // fall over

// ============================================================
// Limits
// ============================================================

pub const MAX_STRING_CHARS: usize = 1024;
pub const MAX_STRING_TOKENS: usize = 80;
pub const MAX_TOKEN_CHARS: usize = 128;

pub const MAX_QPATH: usize = 64;
pub const MAX_OSPATH: usize = 128;

pub const MAX_CLIENTS: usize = 256;
pub const MAX_EDICTS: usize = 1024;
pub const MAX_LIGHTSTYLES: usize = 256;
pub const MAX_MODELS: usize = 256;
pub const MAX_SOUNDS: usize = 256;
pub const MAX_IMAGES: usize = 256;
pub const MAX_ITEMS: usize = 256;
pub const MAX_GENERAL: usize = MAX_CLIENTS * 2;
pub const MAX_ENT_CLUSTERS: usize = 16;
pub const MAX_CLIP_PLANES: usize = 5;

// ============================================================
// Server Timing Constants
// ============================================================
// Quake 2 servers run at a fixed 10Hz tick rate. This is fundamental
// to the network protocol and game physics. These constants document
// this fixed rate and should be used instead of hardcoded values.

/// Server frame time in milliseconds (100ms = 10Hz tick rate).
/// This is a protocol constant - the server always runs at exactly 10Hz.
/// `sv.time = sv.framenum * SERVER_FRAMETIME_MS`
pub const SERVER_FRAMETIME_MS: i32 = 100;

/// Server frame time in seconds (0.1s = 10Hz tick rate).
/// Use this for velocity calculations and physics.
pub const SERVER_FRAMETIME_SEC: f32 = 0.1;

/// Server tick rate in Hz.
pub const SERVER_FRAMERATE_HZ: i32 = 10;

// ============================================================
// Print / error levels
// ============================================================

pub const PRINT_LOW: i32 = 0;
pub const PRINT_MEDIUM: i32 = 1;
pub const PRINT_HIGH: i32 = 2;
pub const PRINT_CHAT: i32 = 3;

pub const ERR_FATAL: i32 = 4;
pub const ERR_DROP: i32 = 8;
pub const ERR_DISCONNECT: i32 = 16;

pub const PRINT_ALL: i32 = 0;
pub const PRINT_DEVELOPER: i32 = 1;
pub const PRINT_ALERT: i32 = 2;
pub const PRINT_INFO: i32 = 3;

// ============================================================
// Multicast
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum Multicast {
    All = 0,
    Phs = 1,
    Pvs = 2,
    AllR = 3,
    PhsR = 4,
    PvsR = 5,
}

// ============================================================
// Content flags
// ============================================================

pub const CONTENTS_SOLID: i32 = 1;
pub const CONTENTS_WINDOW: i32 = 2;
pub const CONTENTS_AUX: i32 = 4;
pub const CONTENTS_LAVA: i32 = 8;
pub const CONTENTS_SLIME: i32 = 16;
pub const CONTENTS_WATER: i32 = 32;
pub const CONTENTS_MIST: i32 = 64;
pub const LAST_VISIBLE_CONTENTS: i32 = 64;

pub const CONTENTS_AREAPORTAL: i32 = 0x8000;
pub const CONTENTS_PLAYERCLIP: i32 = 0x10000;
pub const CONTENTS_MONSTERCLIP: i32 = 0x20000;

pub const CONTENTS_CURRENT_0: i32 = 0x40000;
pub const CONTENTS_CURRENT_90: i32 = 0x80000;
pub const CONTENTS_CURRENT_180: i32 = 0x100000;
pub const CONTENTS_CURRENT_270: i32 = 0x200000;
pub const CONTENTS_CURRENT_UP: i32 = 0x400000;
pub const CONTENTS_CURRENT_DOWN: i32 = 0x800000;

pub const CONTENTS_ORIGIN: i32 = 0x1000000;
pub const CONTENTS_MONSTER: i32 = 0x2000000;
pub const CONTENTS_DEADMONSTER: i32 = 0x4000000;
pub const CONTENTS_DETAIL: i32 = 0x8000000;
pub const CONTENTS_TRANSLUCENT: i32 = 0x10000000;
pub const CONTENTS_LADDER: i32 = 0x20000000;

// ============================================================
// Surface flags
// ============================================================

pub const SURF_LIGHT: i32 = 0x1;
pub const SURF_SLICK: i32 = 0x2;
pub const SURF_SKY: i32 = 0x4;
pub const SURF_WARP: i32 = 0x8;
pub const SURF_TRANS33: i32 = 0x10;
pub const SURF_TRANS66: i32 = 0x20;
pub const SURF_FLOWING: i32 = 0x40;
pub const SURF_NODRAW: i32 = 0x80;

// ============================================================
// Content masks
// ============================================================

pub const MASK_ALL: i32 = -1;
pub const MASK_SOLID: i32 = CONTENTS_SOLID | CONTENTS_WINDOW;
pub const MASK_PLAYERSOLID: i32 =
    CONTENTS_SOLID | CONTENTS_PLAYERCLIP | CONTENTS_WINDOW | CONTENTS_MONSTER;
pub const MASK_DEADSOLID: i32 = CONTENTS_SOLID | CONTENTS_PLAYERCLIP | CONTENTS_WINDOW;
pub const MASK_MONSTERSOLID: i32 =
    CONTENTS_SOLID | CONTENTS_MONSTERCLIP | CONTENTS_WINDOW | CONTENTS_MONSTER;
pub const MASK_WATER: i32 = CONTENTS_WATER | CONTENTS_LAVA | CONTENTS_SLIME;
pub const MASK_OPAQUE: i32 = CONTENTS_SOLID | CONTENTS_SLIME | CONTENTS_LAVA;
pub const MASK_SHOT: i32 =
    CONTENTS_SOLID | CONTENTS_MONSTER | CONTENTS_WINDOW | CONTENTS_DEADMONSTER;
pub const MASK_CURRENT: i32 = CONTENTS_CURRENT_0
    | CONTENTS_CURRENT_90
    | CONTENTS_CURRENT_180
    | CONTENTS_CURRENT_270
    | CONTENTS_CURRENT_UP
    | CONTENTS_CURRENT_DOWN;

pub const AREA_SOLID: i32 = 1;
pub const AREA_TRIGGERS: i32 = 2;

// ============================================================
// Plane
// ============================================================

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct CPlane {
    pub normal: Vec3,
    pub dist: f32,
    pub plane_type: u8,
    pub signbits: u8,
    pub pad: [u8; 2],
}

impl Default for CPlane {
    fn default() -> Self {
        Self {
            normal: [0.0; 3],
            dist: 0.0,
            plane_type: 0,
            signbits: 0,
            pad: [0; 2],
        }
    }
}

// ============================================================
// Collision model / surface
// ============================================================

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct CModel {
    pub mins: Vec3,
    pub maxs: Vec3,
    pub origin: Vec3,
    pub headnode: i32,
}

impl Default for CModel {
    fn default() -> Self {
        Self {
            mins: [0.0; 3],
            maxs: [0.0; 3],
            origin: [0.0; 3],
            headnode: 0,
        }
    }
}

#[derive(Debug, Clone)]
#[repr(C)]
#[derive(Default)]
pub struct CSurface {
    pub name: [u8; 16],
    pub flags: i32,
    pub value: i32,
}


#[derive(Debug, Clone)]
#[repr(C)]
#[derive(Default)]
pub struct MapSurface {
    pub c: CSurface,
    pub rname: [u8; 32],
}


// ============================================================
// Trace
// ============================================================

#[derive(Debug, Clone)]
pub struct Trace {
    pub allsolid: bool,
    pub startsolid: bool,
    pub fraction: f32,
    pub endpos: Vec3,
    pub plane: CPlane,
    pub surface: Option<CSurface>,
    pub contents: i32,
    // ent field is module-specific and handled via generic/index
    pub ent_index: i32,
}

impl Default for Trace {
    fn default() -> Self {
        Self {
            allsolid: false,
            startsolid: false,
            fraction: 1.0,
            endpos: [0.0; 3],
            plane: CPlane::default(),
            surface: None,
            contents: 0,
            ent_index: -1,
        }
    }
}

// ============================================================
// Player movement types
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum PmType {
    Normal = 0,
    Spectator = 1,
    Dead = 2,
    Gib = 3,
    Freeze = 4,
}

pub const PMF_DUCKED: u8 = 1;
pub const PMF_JUMP_HELD: u8 = 2;
pub const PMF_ON_GROUND: u8 = 4;
pub const PMF_TIME_WATERJUMP: u8 = 8;
pub const PMF_TIME_LAND: u8 = 16;
pub const PMF_TIME_TELEPORT: u8 = 32;
pub const PMF_NO_PREDICTION: u8 = 64;

/// Communicated bit-accurate between server and client for prediction sync.
/// No floats — only integers/shorts/bytes.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct PmoveState {
    pub pm_type: PmType,
    pub origin: [i16; 3],    // 12.3 fixed point
    pub velocity: [i16; 3],  // 12.3 fixed point
    pub pm_flags: u8,
    pub pm_time: u8,
    pub gravity: i16,
    pub delta_angles: [i16; 3],
}

impl Default for PmoveState {
    fn default() -> Self {
        Self {
            pm_type: PmType::Normal,
            origin: [0; 3],
            velocity: [0; 3],
            pm_flags: 0,
            pm_time: 0,
            gravity: 0,
            delta_angles: [0; 3],
        }
    }
}

// ============================================================
// Button bits
// ============================================================

pub const BUTTON_ATTACK: u8 = 1;
pub const BUTTON_USE: u8 = 2;
pub const BUTTON_ANY: u8 = 128;

// ============================================================
// Usercmd
// ============================================================

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct UserCmd {
    pub msec: u8,
    pub buttons: u8,
    pub angles: [i16; 3],
    pub forwardmove: i16,
    pub sidemove: i16,
    pub upmove: i16,
    pub impulse: u8,
    pub lightlevel: u8,
}

// ============================================================
// Pmove (full structure for movement prediction)
// ============================================================

pub const MAXTOUCH: usize = 32;

// pmove_t contains callback function pointers in C.
// In Rust we represent them as trait objects or closures supplied at call time.
// The struct here holds just the data fields.
#[derive(Debug, Clone)]
pub struct PmoveData {
    pub s: PmoveState,
    pub cmd: UserCmd,
    pub snapinitial: bool,
    pub numtouch: i32,
    pub touchents: [i32; MAXTOUCH], // entity indices
    pub viewangles: Vec3,
    pub viewheight: f32,
    pub mins: Vec3,
    pub maxs: Vec3,
    pub groundentity: i32, // entity index, -1 = none
    pub watertype: i32,
    pub waterlevel: i32,
}

impl Default for PmoveData {
    fn default() -> Self {
        Self {
            s: PmoveState::default(),
            cmd: UserCmd::default(),
            snapinitial: false,
            numtouch: 0,
            touchents: [-1; MAXTOUCH],
            viewangles: [0.0; 3],
            viewheight: 0.0,
            mins: [0.0; 3],
            maxs: [0.0; 3],
            groundentity: -1,
            watertype: 0,
            waterlevel: 0,
        }
    }
}

// ============================================================
// Entity effects (EF_*)
// ============================================================

pub const EF_ROTATE: u32 = 0x00000001;
pub const EF_GIB: u32 = 0x00000002;
pub const EF_BLASTER: u32 = 0x00000008;
pub const EF_ROCKET: u32 = 0x00000010;
pub const EF_GRENADE: u32 = 0x00000020;
pub const EF_HYPERBLASTER: u32 = 0x00000040;
pub const EF_BFG: u32 = 0x00000080;
pub const EF_COLOR_SHELL: u32 = 0x00000100;
pub const EF_POWERSCREEN: u32 = 0x00000200;
pub const EF_ANIM01: u32 = 0x00000400;
pub const EF_ANIM23: u32 = 0x00000800;
pub const EF_ANIM_ALL: u32 = 0x00001000;
pub const EF_ANIM_ALLFAST: u32 = 0x00002000;
pub const EF_FLIES: u32 = 0x00004000;
pub const EF_QUAD: u32 = 0x00008000;
pub const EF_PENT: u32 = 0x00010000;
pub const EF_TELEPORTER: u32 = 0x00020000;
pub const EF_FLAG1: u32 = 0x00040000;
pub const EF_FLAG2: u32 = 0x00080000;
pub const EF_IONRIPPER: u32 = 0x00100000;
pub const EF_GREENGIB: u32 = 0x00200000;
pub const EF_BLUEHYPERBLASTER: u32 = 0x00400000;
pub const EF_SPINNINGLIGHTS: u32 = 0x00800000;
pub const EF_PLASMA: u32 = 0x01000000;
pub const EF_TRAP: u32 = 0x02000000;
pub const EF_TRACKER: u32 = 0x04000000;
pub const EF_DOUBLE: u32 = 0x08000000;
pub const EF_SPHERETRANS: u32 = 0x10000000;
pub const EF_TAGTRAIL: u32 = 0x20000000;
pub const EF_HALF_DAMAGE: u32 = 0x40000000;
pub const EF_TRACKERTRAIL: u32 = 0x80000000;

// ============================================================
// Render effects (RF_*)
// ============================================================

pub const RF_MINLIGHT: i32 = 1;
pub const RF_VIEWERMODEL: i32 = 2;
pub const RF_WEAPONMODEL: i32 = 4;
pub const RF_FULLBRIGHT: i32 = 8;
pub const RF_DEPTHHACK: i32 = 16;
pub const RF_TRANSLUCENT: i32 = 32;
pub const RF_FRAMELERP: i32 = 64;
pub const RF_BEAM: i32 = 128;
pub const RF_CUSTOMSKIN: i32 = 256;
pub const RF_GLOW: i32 = 512;
pub const RF_SHELL_RED: i32 = 1024;
pub const RF_SHELL_GREEN: i32 = 2048;
pub const RF_SHELL_BLUE: i32 = 4096;
pub const RF_IR_VISIBLE: i32 = 0x00008000;
pub const RF_SHELL_DOUBLE: i32 = 0x00010000;
pub const RF_SHELL_HALF_DAM: i32 = 0x00020000;
pub const RF_USE_DISGUISE: i32 = 0x00040000;

// ============================================================
// Refdef flags (RDF_*)
// ============================================================

pub const RDF_UNDERWATER: i32 = 1;
pub const RDF_NOWORLDMODEL: i32 = 2;
pub const RDF_IRGOGGLES: i32 = 4;
pub const RDF_UVGOGGLES: i32 = 8;

// ============================================================
// Muzzle flashes (MZ_*)
// ============================================================

pub const MZ_BLASTER: i32 = 0;
pub const MZ_MACHINEGUN: i32 = 1;
pub const MZ_SHOTGUN: i32 = 2;
pub const MZ_CHAINGUN1: i32 = 3;
pub const MZ_CHAINGUN2: i32 = 4;
pub const MZ_CHAINGUN3: i32 = 5;
pub const MZ_RAILGUN: i32 = 6;
pub const MZ_ROCKET: i32 = 7;
pub const MZ_GRENADE: i32 = 8;
pub const MZ_LOGIN: i32 = 9;
pub const MZ_LOGOUT: i32 = 10;
pub const MZ_RESPAWN: i32 = 11;
pub const MZ_BFG: i32 = 12;
pub const MZ_SSHOTGUN: i32 = 13;
pub const MZ_HYPERBLASTER: i32 = 14;
pub const MZ_ITEMRESPAWN: i32 = 15;
pub const MZ_IONRIPPER: i32 = 16;
pub const MZ_BLUEHYPERBLASTER: i32 = 17;
pub const MZ_PHALANX: i32 = 18;
pub const MZ_SILENCED: i32 = 128;
pub const MZ_ETF_RIFLE: i32 = 30;
pub const MZ_UNUSED: i32 = 31;
pub const MZ_SHOTGUN2: i32 = 32;
pub const MZ_HEATBEAM: i32 = 33;
pub const MZ_BLASTER2: i32 = 34;
pub const MZ_TRACKER: i32 = 35;
pub const MZ_NUKE1: i32 = 36;
pub const MZ_NUKE2: i32 = 37;
pub const MZ_NUKE4: i32 = 38;
pub const MZ_NUKE8: i32 = 39;

// Monster muzzle flashes (MZ2_*) — large table, representative subset
pub const MZ2_TANK_BLASTER_1: i32 = 1;
pub const MZ2_TANK_BLASTER_2: i32 = 2;
pub const MZ2_TANK_BLASTER_3: i32 = 3;
pub const MZ2_TANK_MACHINEGUN_1: i32 = 4;
pub const MZ2_TANK_MACHINEGUN_19: i32 = 22;
pub const MZ2_TANK_ROCKET_1: i32 = 23;
pub const MZ2_TANK_ROCKET_2: i32 = 24;
pub const MZ2_TANK_ROCKET_3: i32 = 25;
pub const MZ2_INFANTRY_MACHINEGUN_1: i32 = 26;
pub const MZ2_INFANTRY_MACHINEGUN_13: i32 = 38;
pub const MZ2_SOLDIER_BLASTER_1: i32 = 39;
pub const MZ2_SOLDIER_SHOTGUN_1: i32 = 41;
pub const MZ2_SOLDIER_MACHINEGUN_1: i32 = 43;
pub const MZ2_GUNNER_MACHINEGUN_1: i32 = 45;
pub const MZ2_GUNNER_GRENADE_1: i32 = 53;
pub const MZ2_CHICK_ROCKET_1: i32 = 57;
pub const MZ2_FLYER_BLASTER_1: i32 = 58;
pub const MZ2_MEDIC_BLASTER_1: i32 = 60;
pub const MZ2_GLADIATOR_RAILGUN_1: i32 = 61;
pub const MZ2_HOVER_BLASTER_1: i32 = 62;
pub const MZ2_ACTOR_MACHINEGUN_1: i32 = 63;
pub const MZ2_SUPERTANK_MACHINEGUN_1: i32 = 64;
pub const MZ2_SUPERTANK_ROCKET_1: i32 = 70;
pub const MZ2_BOSS2_MACHINEGUN_L1: i32 = 73;
pub const MZ2_BOSS2_ROCKET_1: i32 = 78;
pub const MZ2_FLOAT_BLASTER_1: i32 = 82;
pub const MZ2_MAKRON_BFG: i32 = 101;
pub const MZ2_MAKRON_BLASTER_1: i32 = 102;
pub const MZ2_MAKRON_RAILGUN_1: i32 = 119;
pub const MZ2_JORG_MACHINEGUN_L1: i32 = 120;
pub const MZ2_JORG_MACHINEGUN_R1: i32 = 126;
pub const MZ2_JORG_BFG_1: i32 = 132;
pub const MZ2_BOSS2_MACHINEGUN_R1: i32 = 133;
pub const MZ2_CARRIER_MACHINEGUN_L1: i32 = 138;
pub const MZ2_CARRIER_MACHINEGUN_R1: i32 = 139;
pub const MZ2_CARRIER_GRENADE: i32 = 140;
pub const MZ2_TURRET_MACHINEGUN: i32 = 141;
pub const MZ2_TURRET_ROCKET: i32 = 142;
pub const MZ2_TURRET_BLASTER: i32 = 143;
pub const MZ2_STALKER_BLASTER: i32 = 144;
pub const MZ2_DAEDALUS_BLASTER: i32 = 145;
pub const MZ2_MEDIC_BLASTER_2: i32 = 146;
pub const MZ2_CARRIER_RAILGUN: i32 = 147;
pub const MZ2_WIDOW_DISRUPTOR: i32 = 148;
pub const MZ2_WIDOW_BLASTER: i32 = 149;
pub const MZ2_WIDOW_RAIL: i32 = 150;
pub const MZ2_WIDOW_PLASMABEAM: i32 = 151;
pub const MZ2_WIDOW2_BEAMER_1: i32 = 195;
pub const MZ2_WIDOW2_BEAM_SWEEP_1: i32 = 200;
pub const MZ2_WIDOW2_BEAM_SWEEP_11: i32 = 210;

// ============================================================
// Temp entity events
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum TempEvent {
    Gunshot = 0,
    Blood = 1,
    Blaster = 2,
    Railtrail = 3,
    Shotgun = 4,
    Explosion1 = 5,
    Explosion2 = 6,
    RocketExplosion = 7,
    GrenadeExplosion = 8,
    Sparks = 9,
    Splash = 10,
    Bubbletrail = 11,
    ScreenSparks = 12,
    ShieldSparks = 13,
    BulletSparks = 14,
    LaserSparks = 15,
    ParasiteAttack = 16,
    RocketExplosionWater = 17,
    GrenadeExplosionWater = 18,
    MedicCableAttack = 19,
    BfgExplosion = 20,
    BfgBigexplosion = 21,
    Bosstport = 22,
    BfgLaser = 23,
    GrappleCable = 24,
    WeldingSparks = 25,
    Greenblood = 26,
    Bluehyperblaster = 27,
    PlasmaExplosion = 28,
    TunnelSparks = 29,
    // ROGUE
    Blaster2 = 30,
    Railtrail2 = 31,
    Flame = 32,
    Lightning = 33,
    Debugtrail = 34,
    PlainExplosion = 35,
    Flashlight = 36,
    Forcewall = 37,
    Heatbeam = 38,
    MonsterHeatbeam = 39,
    Steam = 40,
    Bubbletrail2 = 41,
    Moreblood = 42,
    HeatbeamSparks = 43,
    HeatbeamSteam = 44,
    ChainfistSmoke = 45,
    ElectricSparks = 46,
    TrackerExplosion = 47,
    TeleportEffect = 48,
    DballGoal = 49,
    Widowbeamout = 50,
    Nukeblast = 51,
    Widowsplash = 52,
    Explosion1Big = 53,
    Explosion1Np = 54,
    Flechette = 55,
    Stain = 56,
}

pub const SPLASH_UNKNOWN: i32 = 0;
pub const SPLASH_SPARKS: i32 = 1;
pub const SPLASH_BLUE_WATER: i32 = 2;
pub const SPLASH_BROWN_WATER: i32 = 3;
pub const SPLASH_SLIME: i32 = 4;
pub const SPLASH_LAVA: i32 = 5;
pub const SPLASH_BLOOD: i32 = 6;

// ============================================================
// Sound channels / attenuation
// ============================================================

pub const CHAN_AUTO: i32 = 0;
pub const CHAN_WEAPON: i32 = 1;
pub const CHAN_VOICE: i32 = 2;
pub const CHAN_ITEM: i32 = 3;
pub const CHAN_BODY: i32 = 4;
pub const CHAN_NO_PHS_ADD: i32 = 8;
pub const CHAN_RELIABLE: i32 = 16;

pub const ATTN_NONE: f32 = 0.0;
pub const ATTN_NORM: f32 = 1.0;
pub const ATTN_IDLE: f32 = 2.0;
pub const ATTN_STATIC: f32 = 3.0;

// ============================================================
// Stats
// ============================================================

pub const STAT_HEALTH_ICON: i32 = 0;
pub const STAT_HEALTH: i32 = 1;
pub const STAT_AMMO_ICON: i32 = 2;
pub const STAT_AMMO: i32 = 3;
pub const STAT_ARMOR_ICON: i32 = 4;
pub const STAT_ARMOR: i32 = 5;
pub const STAT_SELECTED_ICON: i32 = 6;
pub const STAT_PICKUP_ICON: i32 = 7;
pub const STAT_PICKUP_STRING: i32 = 8;
pub const STAT_TIMER_ICON: i32 = 9;
pub const STAT_TIMER: i32 = 10;
pub const STAT_HELPICON: i32 = 11;
pub const STAT_SELECTED_ITEM: i32 = 12;
pub const STAT_LAYOUTS: i32 = 13;
pub const STAT_FRAGS: i32 = 14;
pub const STAT_FLASHES: i32 = 15;
pub const STAT_CHASE: i32 = 16;
pub const STAT_SPECTATOR: i32 = 17;
pub const MAX_STATS: usize = 32;

// ============================================================
// Deathmatch flags (DF_*)
// ============================================================

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct DmFlags: i32 {
        const NO_HEALTH       = 0x00000001;
        const NO_ITEMS        = 0x00000002;
        const WEAPONS_STAY    = 0x00000004;
        const NO_FALLING      = 0x00000008;
        const INSTANT_ITEMS   = 0x00000010;
        const SAME_LEVEL      = 0x00000020;
        const SKINTEAMS       = 0x00000040;
        const MODELTEAMS      = 0x00000080;
        const NO_FRIENDLY_FIRE = 0x00000100;
        const SPAWN_FARTHEST  = 0x00000200;
        const FORCE_RESPAWN   = 0x00000400;
        const NO_ARMOR        = 0x00000800;
        const ALLOW_EXIT      = 0x00001000;
        const INFINITE_AMMO   = 0x00002000;
        const QUAD_DROP       = 0x00004000;
        const FIXED_FOV       = 0x00008000;
        const QUADFIRE_DROP   = 0x00010000;
        const NO_MINES        = 0x00020000;
        const NO_STACK_DOUBLE = 0x00040000;
        const NO_NUKES        = 0x00080000;
        const NO_SPHERES      = 0x00100000;
    }
}
pub const DF_NO_HEALTH: DmFlags = DmFlags::NO_HEALTH;
pub const DF_NO_ITEMS: DmFlags = DmFlags::NO_ITEMS;
pub const DF_WEAPONS_STAY: DmFlags = DmFlags::WEAPONS_STAY;
pub const DF_NO_FALLING: DmFlags = DmFlags::NO_FALLING;
pub const DF_INSTANT_ITEMS: DmFlags = DmFlags::INSTANT_ITEMS;
pub const DF_SAME_LEVEL: DmFlags = DmFlags::SAME_LEVEL;
pub const DF_SKINTEAMS: DmFlags = DmFlags::SKINTEAMS;
pub const DF_MODELTEAMS: DmFlags = DmFlags::MODELTEAMS;
pub const DF_NO_FRIENDLY_FIRE: DmFlags = DmFlags::NO_FRIENDLY_FIRE;
pub const DF_SPAWN_FARTHEST: DmFlags = DmFlags::SPAWN_FARTHEST;
pub const DF_FORCE_RESPAWN: DmFlags = DmFlags::FORCE_RESPAWN;
pub const DF_NO_ARMOR: DmFlags = DmFlags::NO_ARMOR;
pub const DF_ALLOW_EXIT: DmFlags = DmFlags::ALLOW_EXIT;
pub const DF_INFINITE_AMMO: DmFlags = DmFlags::INFINITE_AMMO;
pub const DF_QUAD_DROP: DmFlags = DmFlags::QUAD_DROP;
pub const DF_FIXED_FOV: DmFlags = DmFlags::FIXED_FOV;
pub const DF_QUADFIRE_DROP: DmFlags = DmFlags::QUADFIRE_DROP;
pub const DF_NO_MINES: DmFlags = DmFlags::NO_MINES;
pub const DF_NO_STACK_DOUBLE: DmFlags = DmFlags::NO_STACK_DOUBLE;
pub const DF_NO_NUKES: DmFlags = DmFlags::NO_NUKES;
pub const DF_NO_SPHERES: DmFlags = DmFlags::NO_SPHERES;

// ============================================================
// Degree / radian conversion
// ============================================================

pub const DEG_TO_RAD: f32 = std::f32::consts::PI / 180.0;
pub const RAD_TO_DEG: f32 = 180.0 / std::f32::consts::PI;

// ============================================================
// Angle/short conversion
// ============================================================

#[inline]
pub fn angle2short(x: f32) -> i32 {
    ((x * 65536.0 / 360.0) as i32) & 65535
}

#[inline]
pub fn short2angle(x: i16) -> f32 {
    (x as f32) * (360.0 / 65536.0)
}

// ============================================================
// Config strings
// ============================================================

pub const CS_NAME: usize = 0;
pub const CS_CDTRACK: usize = 1;
pub const CS_SKY: usize = 2;
pub const CS_SKYAXIS: usize = 3;
pub const CS_SKYROTATE: usize = 4;
pub const CS_STATUSBAR: usize = 5;
pub const CS_AIRACCEL: usize = 29;
pub const CS_MAXCLIENTS: usize = 30;
pub const CS_MAPCHECKSUM: usize = 31;
pub const CS_MODELS: usize = 32;
pub const CS_SOUNDS: usize = CS_MODELS + MAX_MODELS;
pub const CS_IMAGES: usize = CS_SOUNDS + MAX_SOUNDS;
pub const CS_LIGHTS: usize = CS_IMAGES + MAX_IMAGES;
pub const CS_ITEMS: usize = CS_LIGHTS + MAX_LIGHTSTYLES;
pub const CS_PLAYERSKINS: usize = CS_ITEMS + MAX_ITEMS;
pub const CS_GENERAL: usize = CS_PLAYERSKINS + MAX_CLIENTS;
pub const MAX_CONFIGSTRINGS: usize = CS_GENERAL + MAX_GENERAL;

// ============================================================
// Entity events
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum EntityEvent {
    None = 0,
    ItemRespawn = 1,
    Footstep = 2,
    FallShort = 3,
    Fall = 4,
    FallFar = 5,
    PlayerTeleport = 6,
    OtherTeleport = 7,
}

// EV_* integer constants matching the EntityEvent enum values
pub const EV_NONE: i32 = 0;
pub const EV_ITEM_RESPAWN: i32 = 1;
pub const EV_FOOTSTEP: i32 = 2;
pub const EV_FALLSHORT: i32 = 3;
pub const EV_FALL: i32 = 4;
pub const EV_FALLFAR: i32 = 5;
pub const EV_PLAYER_TELEPORT: i32 = 6;
pub const EV_OTHER_TELEPORT: i32 = 7;

// ============================================================
// Entity state
// ============================================================

#[derive(Debug, Clone)]
#[repr(C)]
pub struct EntityState {
    pub number: i32,
    pub origin: Vec3,
    pub angles: Vec3,
    pub old_origin: Vec3,
    pub modelindex: i32,
    pub modelindex2: i32,
    pub modelindex3: i32,
    pub modelindex4: i32,
    pub frame: i32,
    pub skinnum: i32,
    pub effects: u32,
    pub renderfx: i32,
    pub solid: i32,
    pub sound: i32,
    pub event: i32,
}

impl Default for EntityState {
    fn default() -> Self {
        Self {
            number: 0,
            origin: [0.0; 3],
            angles: [0.0; 3],
            old_origin: [0.0; 3],
            modelindex: 0,
            modelindex2: 0,
            modelindex3: 0,
            modelindex4: 0,
            frame: 0,
            skinnum: 0,
            effects: 0,
            renderfx: 0,
            solid: 0,
            sound: 0,
            event: 0,
        }
    }
}

// ============================================================
// Player state
// ============================================================

#[derive(Debug, Clone)]
#[repr(C)]
pub struct PlayerState {
    pub pmove: PmoveState,
    pub viewangles: Vec3,
    pub viewoffset: Vec3,
    pub kick_angles: Vec3,
    pub gunangles: Vec3,
    pub gunoffset: Vec3,
    pub gunindex: i32,
    pub gunframe: i32,
    pub blend: [f32; 4],
    pub fov: f32,
    pub rdflags: i32,
    pub stats: [i16; MAX_STATS],
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            pmove: PmoveState::default(),
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
// Cvar flags
// ============================================================

pub const CVAR_ZERO: i32 = 0;
pub const CVAR_ARCHIVE: i32 = 1;
pub const CVAR_USERINFO: i32 = 2;
pub const CVAR_SERVERINFO: i32 = 4;
pub const CVAR_NOSET: i32 = 8;
pub const CVAR_LATCH: i32 = 16;

// ============================================================
// System search flags
// ============================================================

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct SysFileFlags: u32 {
        const ARCH   = 0x01;
        const HIDDEN = 0x02;
        const RDONLY = 0x04;
        const SUBDIR = 0x08;
        const SYSTEM = 0x10;
    }
}
pub const SFF_ARCH: SysFileFlags = SysFileFlags::ARCH;
pub const SFF_HIDDEN: SysFileFlags = SysFileFlags::HIDDEN;
pub const SFF_RDONLY: SysFileFlags = SysFileFlags::RDONLY;
pub const SFF_SUBDIR: SysFileFlags = SysFileFlags::SUBDIR;
pub const SFF_SYSTEM: SysFileFlags = SysFileFlags::SYSTEM;

// ============================================================
// Info string limits
// ============================================================

pub const MAX_INFO_KEY: usize = 64;
pub const MAX_INFO_VALUE: usize = 64;
pub const MAX_INFO_STRING: usize = 512;

// ============================================================
// MATHLIB — Vector operations
// ============================================================

#[inline]
pub fn dot_product(a: &Vec3, b: &Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
pub fn vector_subtract(a: &Vec3, b: &Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[inline]
pub fn vector_subtract_to(a: &Vec3, b: &Vec3, out: &mut Vec3) {
    out[0] = a[0] - b[0];
    out[1] = a[1] - b[1];
    out[2] = a[2] - b[2];
}

#[inline]
pub fn vector_add(a: &Vec3, b: &Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

#[inline]
pub fn vector_add_to(a: &Vec3, b: &Vec3, out: &mut Vec3) {
    out[0] = a[0] + b[0];
    out[1] = a[1] + b[1];
    out[2] = a[2] + b[2];
}

#[inline]
pub fn vector_copy(src: &Vec3) -> Vec3 {
    *src
}

#[inline]
pub fn vector_copy_to(src: &Vec3, dst: &mut Vec3) {
    *dst = *src;
}

#[inline]
pub fn vector_clear(v: &mut Vec3) {
    v[0] = 0.0;
    v[1] = 0.0;
    v[2] = 0.0;
}

#[inline]
pub fn vector_negate_to(src: &Vec3, dst: &mut Vec3) {
    dst[0] = -src[0];
    dst[1] = -src[1];
    dst[2] = -src[2];
}

#[inline]
pub fn vector_set(v: &mut Vec3, x: f32, y: f32, z: f32) {
    v[0] = x;
    v[1] = y;
    v[2] = z;
}

/// veca + scale * vecb
pub fn vector_ma(veca: &Vec3, scale: f32, vecb: &Vec3) -> Vec3 {
    [
        veca[0] + scale * vecb[0],
        veca[1] + scale * vecb[1],
        veca[2] + scale * vecb[2],
    ]
}

/// Write result into `out`: veca + scale * vecb
pub fn vector_ma_to(veca: &Vec3, scale: f32, vecb: &Vec3, out: &mut Vec3) {
    out[0] = veca[0] + scale * vecb[0];
    out[1] = veca[1] + scale * vecb[1];
    out[2] = veca[2] + scale * vecb[2];
}

pub fn add_point_to_bounds(v: &Vec3, mins: &mut Vec3, maxs: &mut Vec3) {
    for i in 0..3 {
        if v[i] < mins[i] {
            mins[i] = v[i];
        }
        if v[i] > maxs[i] {
            maxs[i] = v[i];
        }
    }
}

pub fn vector_compare(v1: &Vec3, v2: &Vec3) -> bool {
    v1[0] == v2[0] && v1[1] == v2[1] && v1[2] == v2[2]
}

/// Normalize in place, returns original length.
pub fn vector_normalize(v: &mut Vec3) -> f32 {
    let length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if length != 0.0 {
        let ilength = 1.0 / length;
        v[0] *= ilength;
        v[1] *= ilength;
        v[2] *= ilength;
    }
    length
}

pub fn vector_length(v: &Vec3) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

pub fn vector_scale(v: &Vec3, scale: f32) -> Vec3 {
    [v[0] * scale, v[1] * scale, v[2] * scale]
}

pub fn vector_scale_to(v: &Vec3, scale: f32, out: &mut Vec3) {
    out[0] = v[0] * scale;
    out[1] = v[1] * scale;
    out[2] = v[2] * scale;
}

pub fn cross_product(v1: &Vec3, v2: &Vec3) -> Vec3 {
    [
        v1[1] * v2[2] - v1[2] * v2[1],
        v1[2] * v2[0] - v1[0] * v2[2],
        v1[0] * v2[1] - v1[1] * v2[0],
    ]
}

// ============================================================
// Matrix operations
// ============================================================

pub fn r_concat_rotations(in1: &[[f32; 3]; 3], in2: &[[f32; 3]; 3], out: &mut [[f32; 3]; 3]) {
    for i in 0..3 {
        for j in 0..3 {
            out[i][j] = in1[i][0] * in2[0][j] + in1[i][1] * in2[1][j] + in1[i][2] * in2[2][j];
        }
    }
}

// ============================================================
// Angle functions
// ============================================================

pub fn angle_vectors(
    angles: &Vec3,
    forward: Option<&mut Vec3>,
    right: Option<&mut Vec3>,
    up: Option<&mut Vec3>,
) {
    let angle_yaw = angles[YAW].to_radians();
    let sy = angle_yaw.sin();
    let cy = angle_yaw.cos();

    let angle_pitch = angles[PITCH].to_radians();
    let sp = angle_pitch.sin();
    let cp = angle_pitch.cos();

    let angle_roll = angles[ROLL].to_radians();
    let sr = angle_roll.sin();
    let cr = angle_roll.cos();

    if let Some(fwd) = forward {
        fwd[0] = cp * cy;
        fwd[1] = cp * sy;
        fwd[2] = -sp;
    }
    if let Some(r) = right {
        r[0] = -sr * sp * cy + -cr * -sy;
        r[1] = -sr * sp * sy + -cr * cy;
        r[2] = -sr * cp;
    }
    if let Some(u) = up {
        u[0] = cr * sp * cy + -sr * -sy;
        u[1] = cr * sp * sy + -sr * cy;
        u[2] = cr * cp;
    }
}

/// Convenience version of angle_vectors that returns a tuple (forward, right, up).
pub fn angle_vectors_tuple(angles: &Vec3) -> (Vec3, Vec3, Vec3) {
    let mut forward = [0.0f32; 3];
    let mut right = [0.0f32; 3];
    let mut up = [0.0f32; 3];
    angle_vectors(angles, Some(&mut forward), Some(&mut right), Some(&mut up));
    (forward, right, up)
}

/// vectoyaw — Convert a direction vector to a yaw angle.
/// Uses integer truncation matching the original C `(int)` cast.
pub fn vectoyaw(vec: &Vec3) -> f32 {
    if vec[PITCH] == 0.0 {
        if vec[YAW] > 0.0 {
            90.0
        } else if vec[YAW] < 0.0 {
            -90.0
        } else {
            0.0
        }
    } else {
        let mut yaw = (vec[YAW].atan2(vec[PITCH]) * RAD_TO_DEG) as i32 as f32;
        if yaw < 0.0 {
            yaw += 360.0;
        }
        yaw
    }
}

/// vectoangles — game DLL version. Converts a direction vector to Euler angles.
/// Uses integer truncation on yaw, matching the original C `(int)` cast.
pub fn vectoangles(value1: &Vec3, angles: &mut Vec3) {
    let yaw;
    let mut pitch;

    if value1[1] == 0.0 && value1[0] == 0.0 {
        yaw = 0.0;
        pitch = if value1[2] > 0.0 { 90.0 } else { 270.0 };
    } else {
        yaw = if value1[0] != 0.0 {
            (value1[1].atan2(value1[0]) * RAD_TO_DEG) as i32 as f32
        } else if value1[1] > 0.0 {
            90.0
        } else {
            270.0
        };

        let forward = (value1[0] * value1[0] + value1[1] * value1[1]).sqrt();
        pitch = (value1[2].atan2(forward) * RAD_TO_DEG) as i32 as f32;
        if pitch < 0.0 {
            pitch += 360.0;
        }
    }

    angles[PITCH] = -pitch;
    angles[YAW] = if yaw < 0.0 { yaw + 360.0 } else { yaw };
    angles[ROLL] = 0.0;
}

/// Convenience version of vectoangles that returns the result directly.
pub fn vectoangles_tuple(value: &Vec3) -> Vec3 {
    let mut angles = [0.0f32; 3];
    vectoangles(value, &mut angles);
    angles
}

/// vectoangles2 — renderer/client version. Same as vectoangles but without
/// integer truncation on yaw/pitch (used by effects code and renderer).
pub fn vectoangles2(value1: &Vec3, angles: &mut Vec3) {
    if value1[1] == 0.0 && value1[0] == 0.0 {
        angles[YAW] = 0.0;
        angles[PITCH] = if value1[2] > 0.0 { -90.0 } else { -270.0 };
        angles[ROLL] = 0.0;
    } else {
        angles[YAW] = if value1[0] != 0.0 {
            value1[1].atan2(value1[0]) * RAD_TO_DEG
        } else if value1[1] > 0.0 {
            90.0
        } else {
            270.0
        };
        if angles[YAW] < 0.0 {
            angles[YAW] += 360.0;
        }

        let forward = (value1[0] * value1[0] + value1[1] * value1[1]).sqrt();
        angles[PITCH] = -(value1[2].atan2(forward) * RAD_TO_DEG);
        angles[ROLL] = 0.0;
    }
}

pub fn lerp_angle(a2: f32, a1_in: f32, frac: f32) -> f32 {
    let mut a1 = a1_in;
    if a1 - a2 > 180.0 {
        a1 -= 360.0;
    }
    if a1 - a2 < -180.0 {
        a1 += 360.0;
    }
    a2 + frac * (a1 - a2)
}

pub fn anglemod(a: f32) -> f32 {
    (360.0 / 65536.0) * (((a * (65536.0 / 360.0)) as i32) & 65535) as f32
}

/// Returns 1 (front), 2 (back), or 3 (crossing) for a box vs. plane test.
pub fn box_on_plane_side(emins: &Vec3, emaxs: &Vec3, p: &CPlane) -> i32 {
    // fast axial cases
    if (p.plane_type as usize) < 3 {
        let t = p.plane_type as usize;
        if p.dist <= emins[t] {
            return 1;
        }
        if p.dist >= emaxs[t] {
            return 2;
        }
        return 3;
    }

    // general case
    let (dist1, dist2) = match p.signbits {
        0 => (
            p.normal[0] * emaxs[0] + p.normal[1] * emaxs[1] + p.normal[2] * emaxs[2],
            p.normal[0] * emins[0] + p.normal[1] * emins[1] + p.normal[2] * emins[2],
        ),
        1 => (
            p.normal[0] * emins[0] + p.normal[1] * emaxs[1] + p.normal[2] * emaxs[2],
            p.normal[0] * emaxs[0] + p.normal[1] * emins[1] + p.normal[2] * emins[2],
        ),
        2 => (
            p.normal[0] * emaxs[0] + p.normal[1] * emins[1] + p.normal[2] * emaxs[2],
            p.normal[0] * emins[0] + p.normal[1] * emaxs[1] + p.normal[2] * emins[2],
        ),
        3 => (
            p.normal[0] * emins[0] + p.normal[1] * emins[1] + p.normal[2] * emaxs[2],
            p.normal[0] * emaxs[0] + p.normal[1] * emaxs[1] + p.normal[2] * emins[2],
        ),
        4 => (
            p.normal[0] * emaxs[0] + p.normal[1] * emaxs[1] + p.normal[2] * emins[2],
            p.normal[0] * emins[0] + p.normal[1] * emins[1] + p.normal[2] * emaxs[2],
        ),
        5 => (
            p.normal[0] * emins[0] + p.normal[1] * emaxs[1] + p.normal[2] * emins[2],
            p.normal[0] * emaxs[0] + p.normal[1] * emins[1] + p.normal[2] * emaxs[2],
        ),
        6 => (
            p.normal[0] * emaxs[0] + p.normal[1] * emins[1] + p.normal[2] * emins[2],
            p.normal[0] * emins[0] + p.normal[1] * emaxs[1] + p.normal[2] * emaxs[2],
        ),
        7 => (
            p.normal[0] * emins[0] + p.normal[1] * emins[1] + p.normal[2] * emins[2],
            p.normal[0] * emaxs[0] + p.normal[1] * emaxs[1] + p.normal[2] * emaxs[2],
        ),
        _ => (0.0, 0.0),
    };

    let mut sides = 0;
    if dist1 >= p.dist {
        sides = 1;
    }
    if dist2 < p.dist {
        sides |= 2;
    }
    sides
}

pub fn project_point_on_plane(dst: &mut Vec3, p: &Vec3, normal: &Vec3) {
    let inv_denom = 1.0 / dot_product(normal, normal);
    let d = dot_product(normal, p) * inv_denom;
    let n = [
        normal[0] * inv_denom,
        normal[1] * inv_denom,
        normal[2] * inv_denom,
    ];
    dst[0] = p[0] - d * n[0];
    dst[1] = p[1] - d * n[1];
    dst[2] = p[2] - d * n[2];
}

/// Find a vector perpendicular to `src` (assumed normalized).
pub fn perpendicular_vector(dst: &mut Vec3, src: &Vec3) {
    let mut min_elem: f32 = 1.0;
    let mut pos = 0;
    for i in 0..3 {
        if src[i].abs() < min_elem {
            pos = i;
            min_elem = src[i].abs();
        }
    }
    let mut tempvec = [0.0f32; 3];
    tempvec[pos] = 1.0;

    project_point_on_plane(dst, &tempvec, src);
    vector_normalize(dst);
}

pub fn rotate_point_around_vector(dst: &mut Vec3, dir: &Vec3, point: &Vec3, degrees: f32) {
    let vf = *dir;
    let mut vr = [0.0f32; 3];
    perpendicular_vector(&mut vr, dir);
    let vup = cross_product(&vr, &vf);

    let mut m = [[0.0f32; 3]; 3];
    m[0][0] = vr[0];
    m[1][0] = vr[1];
    m[2][0] = vr[2];
    m[0][1] = vup[0];
    m[1][1] = vup[1];
    m[2][1] = vup[2];
    m[0][2] = vf[0];
    m[1][2] = vf[1];
    m[2][2] = vf[2];

    let mut im = m;
    im[0][1] = m[1][0];
    im[0][2] = m[2][0];
    im[1][0] = m[0][1];
    im[1][2] = m[2][1];
    im[2][0] = m[0][2];
    im[2][1] = m[1][2];

    let rad = degrees.to_radians();
    let mut zrot = [[0.0f32; 3]; 3];
    zrot[2][2] = 1.0;
    zrot[0][0] = rad.cos();
    zrot[0][1] = rad.sin();
    zrot[1][0] = -rad.sin();
    zrot[1][1] = rad.cos();

    let mut tmpmat = [[0.0f32; 3]; 3];
    r_concat_rotations(&m, &zrot, &mut tmpmat);
    let mut rot = [[0.0f32; 3]; 3];
    r_concat_rotations(&tmpmat, &im, &mut rot);

    for i in 0..3 {
        dst[i] = rot[i][0] * point[0] + rot[i][1] * point[1] + rot[i][2] * point[2];
    }
}

// ============================================================
// Path / string utilities
// ============================================================

/// Return the filename portion after the last '/'.
pub fn com_skip_path(pathname: &str) -> &str {
    match pathname.rfind('/') {
        Some(pos) => &pathname[pos + 1..],
        None => pathname,
    }
}

/// Strip file extension (everything from the last '.').
pub fn com_strip_extension(input: &str) -> String {
    match input.rfind('.') {
        Some(pos) => input[..pos].to_string(),
        None => input.to_string(),
    }
}

/// Return file extension without the dot.
pub fn com_file_extension(input: &str) -> &str {
    match input.rfind('.') {
        Some(pos) => &input[pos + 1..],
        None => "",
    }
}

/// Append `extension` if the path has no existing extension.
pub fn com_default_extension(path: &mut String, extension: &str) {
    // scan backwards for '.' before '/'
    for ch in path.chars().rev() {
        if ch == '/' {
            break;
        }
        if ch == '.' {
            return; // already has extension
        }
    }
    path.push_str(extension);
}

// ============================================================
// Byte order functions
// ============================================================

// On modern hardware we target little-endian. These are identity on LE,
// byte-swap on BE. Rust's native endian conversion handles this.

#[inline]
pub fn little_short(l: i16) -> i16 {
    i16::from_le(l)
}

#[inline]
pub fn big_short(l: i16) -> i16 {
    i16::from_be(l)
}

#[inline]
pub fn little_long(l: i32) -> i32 {
    i32::from_le(l)
}

#[inline]
pub fn little_float(l: f32) -> f32 {
    f32::from_bits(u32::from_le(l.to_bits()))
}

// ============================================================
// String comparison (case-insensitive)
// ============================================================

pub fn q_stricmp(s1: &str, s2: &str) -> std::cmp::Ordering {
    s1.to_ascii_lowercase().cmp(&s2.to_ascii_lowercase())
}

/// Case-insensitive string equality check.
/// Returns true if strings are equal (ignoring ASCII case).
pub fn q_streq_nocase(s1: &str, s2: &str) -> bool {
    s1.eq_ignore_ascii_case(s2)
}

pub fn q_strncasecmp(s1: &str, s2: &str, n: usize) -> std::cmp::Ordering {
    let a: String = s1.chars().take(n).collect::<String>().to_ascii_lowercase();
    let b: String = s2.chars().take(n).collect::<String>().to_ascii_lowercase();
    a.cmp(&b)
}

// ============================================================
// Token parser (COM_Parse equivalent)
// ============================================================

/// Parse one whitespace-delimited token from `data`, handling // comments
/// and "quoted strings". Returns `(token, remaining)` or `(token, None)`
/// if end of data.
pub fn com_parse(data: &str) -> (String, Option<&str>) {
    let mut chars = data.as_bytes();
    let mut token = String::new();

    // skip whitespace
    loop {
        while !chars.is_empty() && chars[0] <= b' ' {
            if chars[0] == 0 {
                return (String::new(), None);
            }
            chars = &chars[1..];
        }
        if chars.is_empty() {
            return (String::new(), None);
        }

        // skip // comments
        if chars.len() >= 2 && chars[0] == b'/' && chars[1] == b'/' {
            while !chars.is_empty() && chars[0] != b'\n' {
                chars = &chars[1..];
            }
            continue;
        }
        break;
    }

    // handle quoted strings
    if chars[0] == b'"' {
        chars = &chars[1..];
        while !chars.is_empty() && chars[0] != b'"' {
            if token.len() < MAX_TOKEN_CHARS {
                token.push(chars[0] as char);
            }
            chars = &chars[1..];
        }
        if !chars.is_empty() {
            chars = &chars[1..]; // skip closing quote
        }
        let offset = data.len() - chars.len();
        let remaining = if chars.is_empty() {
            None
        } else {
            Some(&data[offset..])
        };
        return (token, remaining);
    }

    // parse regular word
    while !chars.is_empty() && chars[0] > b' ' {
        if token.len() < MAX_TOKEN_CHARS {
            token.push(chars[0] as char);
        }
        chars = &chars[1..];
    }
    if token.len() >= MAX_TOKEN_CHARS {
        token.clear();
    }

    let offset = data.len() - chars.len();
    let remaining = if chars.is_empty() {
        None
    } else {
        Some(&data[offset..])
    };
    (token, remaining)
}

// ============================================================
// Info string functions
// ============================================================

/// Search info string `s` for `key`, return value or empty string.
pub fn info_value_for_key(s: &str, key: &str) -> String {
    let mut chars = s;
    if chars.starts_with('\\') {
        chars = &chars[1..];
    }

    loop {
        // parse key
        let sep = chars.find('\\');
        let pkey = match sep {
            Some(pos) => {
                let k = &chars[..pos];
                chars = &chars[pos + 1..];
                k
            }
            None => return String::new(),
        };

        // parse value
        let sep = chars.find('\\');
        let value = match sep {
            Some(pos) => {
                let v = &chars[..pos];
                chars = &chars[pos + 1..];
                v
            }
            None => {
                // value runs to end of string
                let v = chars;
                if pkey == key {
                    return v.to_string();
                }
                return String::new();
            }
        };

        if pkey == key {
            return value.to_string();
        }
    }
}

/// Remove a key (and its value) from an info string.
pub fn info_remove_key(s: &mut String, key: &str) {
    if key.contains('\\') {
        return;
    }

    let mut result = String::new();
    let mut chars = s.as_str();
    if chars.starts_with('\\') {
        chars = &chars[1..];
    }

    loop {
        if chars.is_empty() {
            break;
        }

        let sep = chars.find('\\');
        let pkey = match sep {
            Some(pos) => {
                let k = &chars[..pos];
                chars = &chars[pos + 1..];
                k
            }
            None => break,
        };

        let sep = chars.find('\\');
        let value = match sep {
            Some(pos) => {
                let v = &chars[..pos];
                chars = &chars[pos + 1..];
                v
            }
            None => {
                let v = chars;
                chars = "";
                v
            }
        };

        if pkey != key {
            result.push('\\');
            result.push_str(pkey);
            result.push('\\');
            result.push_str(value);
        }
    }

    *s = result;
}

/// Check that an info string contains no illegal characters.
pub fn info_validate(s: &str) -> bool {
    !s.contains('"') && !s.contains(';')
}

/// Set a key/value pair in an info string.
pub fn info_set_value_for_key(s: &mut String, key: &str, value: &str) {
    if key.contains('\\') || value.contains('\\') {
        return;
    }
    if key.contains(';') {
        return;
    }
    if key.contains('"') || value.contains('"') {
        return;
    }
    if key.len() >= MAX_INFO_KEY || value.len() >= MAX_INFO_KEY {
        return;
    }

    info_remove_key(s, key);

    if value.is_empty() {
        return;
    }

    let newi = format!("\\{}\\{}", key, value);
    if newi.len() + s.len() > MAX_INFO_STRING {
        return;
    }

    // only append printable ASCII (32..127)
    for c in newi.bytes() {
        let c = c & 127;
        if (32..127).contains(&c) {
            s.push(c as char);
        }
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dot_product() {
        let a = [1.0, 2.0, 3.0];
        let b = [4.0, 5.0, 6.0];
        assert_eq!(dot_product(&a, &b), 32.0);
    }

    #[test]
    fn test_vector_normalize() {
        let mut v = [3.0, 0.0, 4.0];
        let len = vector_normalize(&mut v);
        assert!((len - 5.0).abs() < 1e-6);
        assert!((v[0] - 0.6).abs() < 1e-6);
        assert!((v[2] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_cross_product() {
        let a = [1.0, 0.0, 0.0];
        let b = [0.0, 1.0, 0.0];
        let c = cross_product(&a, &b);
        assert_eq!(c, [0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_anglemod() {
        let a = anglemod(370.0);
        assert!((a - 10.0).abs() < 0.1);
    }

    #[test]
    fn test_lerp_angle() {
        let result = lerp_angle(0.0, 90.0, 0.5);
        assert!((result - 45.0).abs() < 1e-6);
    }

    #[test]
    fn test_box_on_plane_side_axial() {
        let mins = [-1.0, -1.0, -1.0];
        let maxs = [1.0, 1.0, 1.0];
        let plane = CPlane {
            normal: [1.0, 0.0, 0.0],
            dist: 5.0,
            plane_type: 0,
            signbits: 0,
            pad: [0; 2],
        };
        assert_eq!(box_on_plane_side(&mins, &maxs, &plane), 2);
    }

    #[test]
    fn test_info_strings() {
        let mut s = String::from("\\name\\player\\skill\\3");
        assert_eq!(info_value_for_key(&s, "name"), "player");
        assert_eq!(info_value_for_key(&s, "skill"), "3");
        assert_eq!(info_value_for_key(&s, "missing"), "");

        info_remove_key(&mut s, "skill");
        assert_eq!(info_value_for_key(&s, "skill"), "");
        assert_eq!(info_value_for_key(&s, "name"), "player");

        info_set_value_for_key(&mut s, "team", "red");
        assert_eq!(info_value_for_key(&s, "team"), "red");
    }

    #[test]
    fn test_com_parse() {
        let (token, rest) = com_parse("hello world");
        assert_eq!(token, "hello");
        assert_eq!(rest, Some(" world"));

        let (token, rest) = com_parse("\"quoted string\" next");
        assert_eq!(token, "quoted string");
        assert!(rest.is_some());

        let (token, _) = com_parse("// comment\nvalue");
        assert_eq!(token, "value");
    }

    #[test]
    fn test_com_skip_path() {
        assert_eq!(com_skip_path("models/items/weapon.md2"), "weapon.md2");
        assert_eq!(com_skip_path("nopath"), "nopath");
    }

    #[test]
    fn test_com_file_extension() {
        assert_eq!(com_file_extension("model.md2"), "md2");
        assert_eq!(com_file_extension("noext"), "");
    }

    #[test]
    fn test_byte_order() {
        // On little-endian systems these should be identity
        assert_eq!(little_long(42), 42);
        assert_eq!(little_short(1000), 1000);
    }

    // =========================================================================
    // com_strip_extension — additional tests
    // =========================================================================

    #[test]
    fn test_strip_extension_md2() {
        assert_eq!(com_strip_extension("model.md2"), "model");
    }

    #[test]
    fn test_strip_extension_no_ext() {
        assert_eq!(com_strip_extension("file"), "file");
    }

    #[test]
    fn test_strip_extension_path_with_ext() {
        assert_eq!(com_strip_extension("path/file.ext"), "path/file");
    }

    #[test]
    fn test_strip_extension_multiple_dots() {
        // rfind('.') strips from the last dot
        assert_eq!(com_strip_extension("archive.tar.gz"), "archive.tar");
    }

    #[test]
    fn test_strip_extension_dot_only() {
        assert_eq!(com_strip_extension(".hidden"), "");
    }

    #[test]
    fn test_strip_extension_empty() {
        assert_eq!(com_strip_extension(""), "");
    }

    // =========================================================================
    // com_skip_path — additional tests
    // =========================================================================

    #[test]
    fn test_skip_path_slash_file() {
        assert_eq!(com_skip_path("/file"), "file");
    }

    #[test]
    fn test_skip_path_just_file() {
        assert_eq!(com_skip_path("file"), "file");
    }

    #[test]
    fn test_skip_path_deep() {
        assert_eq!(com_skip_path("a/b/c/d.txt"), "d.txt");
    }

    #[test]
    fn test_skip_path_trailing_slash() {
        assert_eq!(com_skip_path("dir/"), "");
    }

    #[test]
    fn test_skip_path_empty() {
        assert_eq!(com_skip_path(""), "");
    }

    // =========================================================================
    // com_file_extension — additional tests
    // =========================================================================

    #[test]
    fn test_file_extension_pcx() {
        assert_eq!(com_file_extension("pic.pcx"), "pcx");
    }

    #[test]
    fn test_file_extension_with_path() {
        assert_eq!(com_file_extension("models/player.md2"), "md2");
    }

    #[test]
    fn test_file_extension_empty_string() {
        assert_eq!(com_file_extension(""), "");
    }

    // =========================================================================
    // com_default_extension
    // =========================================================================

    #[test]
    fn test_default_extension_adds() {
        let mut path = String::from("model");
        com_default_extension(&mut path, ".md2");
        assert_eq!(path, "model.md2");
    }

    #[test]
    fn test_default_extension_already_has() {
        let mut path = String::from("model.md2");
        com_default_extension(&mut path, ".pcx");
        assert_eq!(path, "model.md2"); // unchanged
    }

    #[test]
    fn test_default_extension_path_with_dot_dir() {
        // Dot in directory part but not in file part
        let mut path = String::from("some.dir/model");
        com_default_extension(&mut path, ".md2");
        assert_eq!(path, "some.dir/model.md2");
    }

    // =========================================================================
    // vectoyaw (vec3_to_yaw equivalent)
    // =========================================================================

    #[test]
    fn test_vectoyaw_zero_vector() {
        assert_eq!(vectoyaw(&[0.0, 0.0, 0.0]), 0.0);
    }

    #[test]
    fn test_vectoyaw_positive_y_only() {
        // x=0, y>0 => 90 degrees
        assert_eq!(vectoyaw(&[0.0, 1.0, 0.0]), 90.0);
    }

    #[test]
    fn test_vectoyaw_negative_y_only() {
        // x=0, y<0 => -90 degrees
        assert_eq!(vectoyaw(&[0.0, -1.0, 0.0]), -90.0);
    }

    #[test]
    fn test_vectoyaw_positive_x() {
        // x=1, y=0 => 0 degrees
        let yaw = vectoyaw(&[1.0, 0.0, 0.0]);
        assert!((yaw - 0.0).abs() < 1.0);
    }

    #[test]
    fn test_vectoyaw_negative_x() {
        // x=-1, y=0 => 180 degrees
        let yaw = vectoyaw(&[-1.0, 0.0, 0.0]);
        assert!((yaw - 180.0).abs() < 1.0);
    }

    #[test]
    fn test_vectoyaw_diagonal() {
        // x=1, y=1 => ~45 degrees
        let yaw = vectoyaw(&[1.0, 1.0, 0.0]);
        assert!((yaw - 45.0).abs() < 1.0);
    }

    // =========================================================================
    // vectoangles (vec3_to_angles equivalent)
    // =========================================================================

    #[test]
    fn test_vectoangles_forward_x() {
        let mut angles = [0.0; 3];
        vectoangles(&[1.0, 0.0, 0.0], &mut angles);
        // Pitch should be ~0, yaw should be ~0
        assert!(angles[PITCH].abs() < 1.0);
        assert!(angles[YAW].abs() < 1.0);
        assert_eq!(angles[ROLL], 0.0);
    }

    #[test]
    fn test_vectoangles_forward_y() {
        let mut angles = [0.0; 3];
        vectoangles(&[0.0, 1.0, 0.0], &mut angles);
        // yaw should be 90 degrees
        assert!((angles[YAW] - 90.0).abs() < 1.0);
    }

    #[test]
    fn test_vectoangles_up() {
        let mut angles = [0.0; 3];
        vectoangles(&[0.0, 0.0, 1.0], &mut angles);
        // Pure up: yaw=0, pitch=-90
        assert_eq!(angles[YAW], 0.0);
        assert!((angles[PITCH] - (-90.0)).abs() < 1.0);
    }

    #[test]
    fn test_vectoangles_down() {
        let mut angles = [0.0; 3];
        vectoangles(&[0.0, 0.0, -1.0], &mut angles);
        // Pure down: yaw=0, pitch=-270
        assert_eq!(angles[YAW], 0.0);
        assert!((angles[PITCH] - (-270.0)).abs() < 1.0);
    }

    #[test]
    fn test_vectoangles_diagonal() {
        let mut angles = [0.0; 3];
        vectoangles(&[1.0, 1.0, 0.0], &mut angles);
        // yaw should be ~45 degrees
        assert!((angles[YAW] - 45.0).abs() < 1.0);
    }

    // =========================================================================
    // angle_vectors (angles_vectors equivalent)
    // =========================================================================

    #[test]
    fn test_angle_vectors_zero_angles() {
        let angles = [0.0, 0.0, 0.0];
        let mut forward = [0.0f32; 3];
        let mut right = [0.0f32; 3];
        let mut up = [0.0f32; 3];
        angle_vectors(&angles, Some(&mut forward), Some(&mut right), Some(&mut up));

        // Forward should be along +X
        assert!((forward[0] - 1.0).abs() < 1e-5);
        assert!(forward[1].abs() < 1e-5);
        assert!(forward[2].abs() < 1e-5);

        // Right should be along +Y (right in Q2 coordinate system)
        assert!(right[0].abs() < 1e-5);
        assert!((right[1] - (-1.0)).abs() < 1e-5);
        assert!(right[2].abs() < 1e-5);

        // Up should be along +Z
        assert!(up[0].abs() < 1e-5);
        assert!(up[1].abs() < 1e-5);
        assert!((up[2] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_angle_vectors_yaw_90() {
        let angles = [0.0, 90.0, 0.0]; // pitch=0, yaw=90, roll=0
        let mut forward = [0.0f32; 3];
        angle_vectors(&angles, Some(&mut forward), None, None);

        // Forward should be along +Y
        assert!(forward[0].abs() < 1e-5);
        assert!((forward[1] - 1.0).abs() < 1e-5);
        assert!(forward[2].abs() < 1e-5);
    }

    #[test]
    fn test_angle_vectors_pitch_90() {
        let angles = [90.0, 0.0, 0.0]; // pitch=90, yaw=0, roll=0
        let mut forward = [0.0f32; 3];
        angle_vectors(&angles, Some(&mut forward), None, None);

        // Forward should be along -Z (pitch 90 = looking down in Q2)
        assert!(forward[0].abs() < 1e-5);
        assert!(forward[1].abs() < 1e-5);
        assert!((forward[2] - (-1.0)).abs() < 1e-5);
    }

    #[test]
    fn test_angle_vectors_none_params() {
        // Should not crash when passing None for any output
        let angles = [45.0, 90.0, 0.0];
        angle_vectors(&angles, None, None, None);
    }

    // =========================================================================
    // perpendicular_vector (perp_vector equivalent)
    // =========================================================================

    #[test]
    fn test_perpendicular_vector_x_axis() {
        let src = [1.0, 0.0, 0.0];
        let mut dst = [0.0f32; 3];
        perpendicular_vector(&mut dst, &src);

        // Result should be perpendicular to src
        let d = dot_product(&dst, &src);
        assert!(d.abs() < 1e-5, "dot product should be ~0, got {}", d);

        // Result should be unit length
        let len = vector_length(&dst);
        assert!((len - 1.0).abs() < 1e-5, "length should be ~1, got {}", len);
    }

    #[test]
    fn test_perpendicular_vector_y_axis() {
        let src = [0.0, 1.0, 0.0];
        let mut dst = [0.0f32; 3];
        perpendicular_vector(&mut dst, &src);

        let d = dot_product(&dst, &src);
        assert!(d.abs() < 1e-5);
        let len = vector_length(&dst);
        assert!((len - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_perpendicular_vector_z_axis() {
        let src = [0.0, 0.0, 1.0];
        let mut dst = [0.0f32; 3];
        perpendicular_vector(&mut dst, &src);

        let d = dot_product(&dst, &src);
        assert!(d.abs() < 1e-5);
        let len = vector_length(&dst);
        assert!((len - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_perpendicular_vector_diagonal() {
        let mut src = [1.0, 1.0, 1.0];
        vector_normalize(&mut src);
        let mut dst = [0.0f32; 3];
        perpendicular_vector(&mut dst, &src);

        let d = dot_product(&dst, &src);
        assert!(d.abs() < 1e-4);
    }

    // =========================================================================
    // rotate_point_around_vector
    // =========================================================================

    #[test]
    fn test_rotate_360_returns_original() {
        let dir = [0.0, 0.0, 1.0]; // rotate around Z
        let point = [1.0, 0.0, 0.0];
        let mut dst = [0.0f32; 3];
        rotate_point_around_vector(&mut dst, &dir, &point, 360.0);

        assert!((dst[0] - point[0]).abs() < 1e-4, "x: {} vs {}", dst[0], point[0]);
        assert!((dst[1] - point[1]).abs() < 1e-4, "y: {} vs {}", dst[1], point[1]);
        assert!((dst[2] - point[2]).abs() < 1e-4, "z: {} vs {}", dst[2], point[2]);
    }

    #[test]
    fn test_rotate_90_around_z() {
        let dir = [0.0, 0.0, 1.0]; // Z axis
        let point = [1.0, 0.0, 0.0];
        let mut dst = [0.0f32; 3];
        rotate_point_around_vector(&mut dst, &dir, &point, 90.0);

        // (1,0,0) rotated 90 degrees around Z => (0,1,0)
        assert!((dst[0] - 0.0).abs() < 1e-4, "x: {}", dst[0]);
        assert!((dst[1] - 1.0).abs() < 1e-4, "y: {}", dst[1]);
        assert!((dst[2] - 0.0).abs() < 1e-4, "z: {}", dst[2]);
    }

    #[test]
    fn test_rotate_180_around_z() {
        let dir = [0.0, 0.0, 1.0];
        let point = [1.0, 0.0, 0.0];
        let mut dst = [0.0f32; 3];
        rotate_point_around_vector(&mut dst, &dir, &point, 180.0);

        // (1,0,0) rotated 180 degrees around Z => (-1,0,0)
        assert!((dst[0] - (-1.0)).abs() < 1e-4, "x: {}", dst[0]);
        assert!(dst[1].abs() < 1e-4, "y: {}", dst[1]);
        assert!(dst[2].abs() < 1e-4, "z: {}", dst[2]);
    }

    #[test]
    fn test_rotate_90_around_x() {
        let dir = [1.0, 0.0, 0.0]; // X axis
        let point = [0.0, 1.0, 0.0];
        let mut dst = [0.0f32; 3];
        rotate_point_around_vector(&mut dst, &dir, &point, 90.0);

        // (0,1,0) rotated 90 degrees around X => (0,0,1)
        assert!(dst[0].abs() < 1e-4, "x: {}", dst[0]);
        assert!(dst[1].abs() < 1e-4, "y: {}", dst[1]);
        assert!((dst[2] - 1.0).abs() < 1e-4, "z: {}", dst[2]);
    }

    // =========================================================================
    // Endian functions — additional tests
    // =========================================================================

    #[test]
    fn test_little_short_identity() {
        assert_eq!(little_short(0), 0);
        assert_eq!(little_short(i16::MAX), i16::MAX);
        assert_eq!(little_short(i16::MIN), i16::MIN);
        assert_eq!(little_short(-1), -1);
    }

    #[test]
    fn test_little_long_identity() {
        assert_eq!(little_long(0), 0);
        assert_eq!(little_long(i32::MAX), i32::MAX);
        assert_eq!(little_long(i32::MIN), i32::MIN);
        assert_eq!(little_long(-1), -1);
        assert_eq!(little_long(0x12345678), 0x12345678);
    }

    #[test]
    fn test_little_float_identity() {
        assert_eq!(little_float(0.0), 0.0);
        assert_eq!(little_float(1.0), 1.0);
        assert_eq!(little_float(-1.0), -1.0);
        assert_eq!(little_float(3.14), 3.14);
    }

    #[test]
    fn test_big_short() {
        // big_short should byte-swap on little-endian platform
        let val: i16 = 0x0102;
        let result = big_short(val);
        // On LE, from_be(0x0102) interprets the bytes as big-endian,
        // so the actual value should be 0x0201
        assert_eq!(result, 0x0201_i16);
    }

    // =========================================================================
    // vector_ma
    // =========================================================================

    #[test]
    fn test_vector_ma_basic() {
        let a = [1.0, 2.0, 3.0];
        let b = [4.0, 5.0, 6.0];
        let result = vector_ma(&a, 2.0, &b);
        assert_eq!(result, [9.0, 12.0, 15.0]);
    }

    #[test]
    fn test_vector_ma_zero_scale() {
        let a = [1.0, 2.0, 3.0];
        let b = [100.0, 200.0, 300.0];
        let result = vector_ma(&a, 0.0, &b);
        assert_eq!(result, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_vector_ma_negative_scale() {
        let a = [10.0, 10.0, 10.0];
        let b = [1.0, 2.0, 3.0];
        let result = vector_ma(&a, -1.0, &b);
        assert_eq!(result, [9.0, 8.0, 7.0]);
    }

    // =========================================================================
    // vector_scale
    // =========================================================================

    #[test]
    fn test_vector_scale_basic() {
        let v = [1.0, 2.0, 3.0];
        assert_eq!(vector_scale(&v, 3.0), [3.0, 6.0, 9.0]);
    }

    #[test]
    fn test_vector_scale_zero() {
        let v = [5.0, 10.0, 15.0];
        assert_eq!(vector_scale(&v, 0.0), [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_vector_scale_negative() {
        let v = [1.0, -2.0, 3.0];
        assert_eq!(vector_scale(&v, -2.0), [-2.0, 4.0, -6.0]);
    }

    // =========================================================================
    // add_point_to_bounds / clear_bounds equivalent
    // =========================================================================

    #[test]
    fn test_add_point_to_bounds_expands_mins() {
        let mut mins = [0.0, 0.0, 0.0];
        let mut maxs = [10.0, 10.0, 10.0];
        add_point_to_bounds(&[-5.0, -3.0, -1.0], &mut mins, &mut maxs);
        assert_eq!(mins, [-5.0, -3.0, -1.0]);
        assert_eq!(maxs, [10.0, 10.0, 10.0]);
    }

    #[test]
    fn test_add_point_to_bounds_expands_maxs() {
        let mut mins = [0.0, 0.0, 0.0];
        let mut maxs = [10.0, 10.0, 10.0];
        add_point_to_bounds(&[15.0, 20.0, 25.0], &mut mins, &mut maxs);
        assert_eq!(mins, [0.0, 0.0, 0.0]);
        assert_eq!(maxs, [15.0, 20.0, 25.0]);
    }

    #[test]
    fn test_add_point_to_bounds_inside() {
        let mut mins = [-10.0, -10.0, -10.0];
        let mut maxs = [10.0, 10.0, 10.0];
        add_point_to_bounds(&[0.0, 0.0, 0.0], &mut mins, &mut maxs);
        assert_eq!(mins, [-10.0, -10.0, -10.0]);
        assert_eq!(maxs, [10.0, 10.0, 10.0]);
    }

    #[test]
    fn test_add_point_to_bounds_mixed() {
        let mut mins = [0.0, 0.0, 0.0];
        let mut maxs = [0.0, 0.0, 0.0];
        add_point_to_bounds(&[-1.0, 5.0, -3.0], &mut mins, &mut maxs);
        assert_eq!(mins, [-1.0, 0.0, -3.0]);
        assert_eq!(maxs, [0.0, 5.0, 0.0]);
    }

    // =========================================================================
    // vector operations — additional
    // =========================================================================

    #[test]
    fn test_vector_length() {
        assert_eq!(vector_length(&[0.0, 0.0, 0.0]), 0.0);
        assert!((vector_length(&[3.0, 4.0, 0.0]) - 5.0).abs() < 1e-6);
        assert!((vector_length(&[1.0, 1.0, 1.0]) - 3.0f32.sqrt()).abs() < 1e-6);
    }

    #[test]
    fn test_vector_compare_equal() {
        assert!(vector_compare(&[1.0, 2.0, 3.0], &[1.0, 2.0, 3.0]));
    }

    #[test]
    fn test_vector_compare_different() {
        assert!(!vector_compare(&[1.0, 2.0, 3.0], &[1.0, 2.0, 3.1]));
    }

    #[test]
    fn test_vector_subtract() {
        let result = vector_subtract(&[5.0, 10.0, 15.0], &[1.0, 2.0, 3.0]);
        assert_eq!(result, [4.0, 8.0, 12.0]);
    }

    #[test]
    fn test_vector_add() {
        let result = vector_add(&[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0]);
        assert_eq!(result, [5.0, 7.0, 9.0]);
    }

    #[test]
    fn test_vector_negate() {
        let src = [1.0, -2.0, 3.0];
        let mut dst = [0.0f32; 3];
        vector_negate_to(&src, &mut dst);
        assert_eq!(dst, [-1.0, 2.0, -3.0]);
    }

    #[test]
    fn test_vector_clear() {
        let mut v = [1.0, 2.0, 3.0];
        vector_clear(&mut v);
        assert_eq!(v, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_vector_set() {
        let mut v = [0.0f32; 3];
        vector_set(&mut v, 1.5, 2.5, 3.5);
        assert_eq!(v, [1.5, 2.5, 3.5]);
    }

    #[test]
    fn test_vector_copy() {
        let src = [7.0, 8.0, 9.0];
        let dst = vector_copy(&src);
        assert_eq!(dst, [7.0, 8.0, 9.0]);
    }

    // =========================================================================
    // r_concat_rotations
    // =========================================================================

    #[test]
    fn test_r_concat_rotations_identity() {
        let identity = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let mut result = [[0.0f32; 3]; 3];
        r_concat_rotations(&identity, &identity, &mut result);
        for i in 0..3 {
            for j in 0..3 {
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!(
                    (result[i][j] - expected).abs() < 1e-6,
                    "result[{}][{}] = {}, expected {}",
                    i, j, result[i][j], expected
                );
            }
        }
    }

    // =========================================================================
    // angle2short / short2angle
    // =========================================================================

    #[test]
    fn test_angle2short_zero() {
        assert_eq!(angle2short(0.0), 0);
    }

    #[test]
    fn test_angle2short_90() {
        let s = angle2short(90.0);
        // 90 * 65536 / 360 = 16384
        assert_eq!(s, 16384);
    }

    #[test]
    fn test_angle2short_360() {
        let s = angle2short(360.0);
        // 360 * 65536 / 360 = 65536, masked with & 65535 => 0
        assert_eq!(s, 0);
    }

    #[test]
    fn test_short2angle_zero() {
        assert_eq!(short2angle(0), 0.0);
    }

    #[test]
    fn test_short2angle_16384() {
        let a = short2angle(16384);
        assert!((a - 90.0).abs() < 0.01);
    }

    #[test]
    fn test_angle2short_short2angle_roundtrip() {
        let original = 45.0;
        let s = angle2short(original) as i16;
        let back = short2angle(s);
        assert!((back - original).abs() < 0.01);
    }

    // =========================================================================
    // q_stricmp / q_streq_nocase / q_strncasecmp
    // =========================================================================

    #[test]
    fn test_q_stricmp_equal() {
        assert_eq!(q_stricmp("Hello", "hello"), std::cmp::Ordering::Equal);
    }

    #[test]
    fn test_q_stricmp_less() {
        assert_eq!(q_stricmp("abc", "def"), std::cmp::Ordering::Less);
    }

    #[test]
    fn test_q_streq_nocase_true() {
        assert!(q_streq_nocase("QUAKE", "quake"));
    }

    #[test]
    fn test_q_streq_nocase_false() {
        assert!(!q_streq_nocase("quake", "doom"));
    }

    #[test]
    fn test_q_strncasecmp_prefix() {
        assert_eq!(q_strncasecmp("Hello World", "HELLO", 5), std::cmp::Ordering::Equal);
    }

    // =========================================================================
    // info_validate
    // =========================================================================

    #[test]
    fn test_info_validate_valid() {
        assert!(info_validate("\\name\\player"));
    }

    #[test]
    fn test_info_validate_has_quote() {
        assert!(!info_validate("\\name\\play\"er"));
    }

    #[test]
    fn test_info_validate_has_semicolon() {
        assert!(!info_validate("\\name\\player;drop"));
    }

    // =========================================================================
    // lerp_angle — additional tests
    // =========================================================================

    #[test]
    fn test_lerp_angle_wrap_positive() {
        // From 350 to 10 should interpolate through 0 (shortest path)
        let result = lerp_angle(350.0, 10.0, 0.5);
        assert!((result - 0.0).abs() < 1.0 || (result - 360.0).abs() < 1.0);
    }

    #[test]
    fn test_lerp_angle_same() {
        assert_eq!(lerp_angle(90.0, 90.0, 0.5), 90.0);
    }

    // =========================================================================
    // anglemod — additional tests
    // =========================================================================

    #[test]
    fn test_anglemod_negative() {
        let a = anglemod(-90.0);
        assert!((a - 270.0).abs() < 0.1);
    }

    #[test]
    fn test_anglemod_zero() {
        assert!((anglemod(0.0)).abs() < 0.1);
    }

    #[test]
    fn test_anglemod_720() {
        let a = anglemod(720.0);
        assert!(a.abs() < 0.1);
    }

    // =========================================================================
    // project_point_on_plane
    // =========================================================================

    #[test]
    fn test_project_point_on_plane_xy() {
        // Project (1,1,5) onto the XY plane (normal = Z axis)
        let mut dst = [0.0f32; 3];
        project_point_on_plane(&mut dst, &[1.0, 1.0, 5.0], &[0.0, 0.0, 1.0]);
        assert!((dst[0] - 1.0).abs() < 1e-5);
        assert!((dst[1] - 1.0).abs() < 1e-5);
        assert!(dst[2].abs() < 1e-5);
    }

    // =========================================================================
    // box_on_plane_side — additional tests
    // =========================================================================

    #[test]
    fn test_box_on_plane_side_front() {
        let mins = [10.0, 10.0, 10.0];
        let maxs = [20.0, 20.0, 20.0];
        let plane = CPlane {
            normal: [1.0, 0.0, 0.0],
            dist: 5.0,
            plane_type: 0,
            signbits: 0,
            pad: [0; 2],
        };
        assert_eq!(box_on_plane_side(&mins, &maxs, &plane), 1);
    }

    #[test]
    fn test_box_on_plane_side_crossing() {
        let mins = [-5.0, -5.0, -5.0];
        let maxs = [5.0, 5.0, 5.0];
        let plane = CPlane {
            normal: [1.0, 0.0, 0.0],
            dist: 0.0,
            plane_type: 0,
            signbits: 0,
            pad: [0; 2],
        };
        assert_eq!(box_on_plane_side(&mins, &maxs, &plane), 3);
    }

    // =========================================================================
    // vectoangles2
    // =========================================================================

    #[test]
    fn test_vectoangles2_forward_x() {
        let mut angles = [0.0; 3];
        vectoangles2(&[1.0, 0.0, 0.0], &mut angles);
        assert!(angles[PITCH].abs() < 1e-3);
        assert!(angles[YAW].abs() < 1e-3);
    }

    #[test]
    fn test_vectoangles2_up() {
        let mut angles = [0.0; 3];
        vectoangles2(&[0.0, 0.0, 1.0], &mut angles);
        assert!((angles[PITCH] - (-90.0)).abs() < 1e-3);
    }

    // =========================================================================
    // C-to-Rust cross-validation: anglemod
    // C: ((360.0/65536) * ((int)(a*(65536/360.0)) & 65535))
    // =========================================================================

    #[test]
    fn test_anglemod_matches_c_exact() {
        let test_values: &[f32] = &[
            0.0, 1.0, 45.0, 90.0, 180.0, 270.0, 359.0, 360.0,
            370.0, 720.0, -90.0, -180.0, -270.0, -360.0, -1.0,
            0.0001, 359.9999, 1080.5, -1080.5,
        ];
        for &a in test_values {
            let result = anglemod(a);
            // Replicate C behavior exactly
            let c_result = (360.0_f32 / 65536.0)
                * (((a * (65536.0_f32 / 360.0)) as i32) & 65535) as f32;
            assert!(
                (result - c_result).abs() < f32::EPSILON,
                "anglemod({}) = {}, C expects {}",
                a, result, c_result
            );
        }
    }

    #[test]
    fn test_anglemod_large_positive() {
        // Very large angle: should still reduce to [0, 360)
        let a = 100000.0;
        let result = anglemod(a);
        assert!(result >= 0.0 && result < 360.0,
            "anglemod({}) = {} should be in [0, 360)", a, result);
    }

    #[test]
    fn test_anglemod_large_negative() {
        let a = -100000.0;
        let result = anglemod(a);
        // C's (int) truncation on negative values means the & 65535 mask
        // maps negative to large positive. Result should be in [0, 360).
        assert!(result >= 0.0 && result < 360.0,
            "anglemod({}) = {} should be in [0, 360)", a, result);
    }

    // =========================================================================
    // C-to-Rust cross-validation: lerp_angle wrapping at 180 boundary
    // =========================================================================

    #[test]
    fn test_lerp_angle_wrapping_at_180_boundary_exact() {
        // From 0 to 181: difference > 180, so a1 should wrap
        // C: a1=181, a2=0, (181-0)>180 => a1 = 181 - 360 = -179
        // result = 0 + 0.5*(-179 - 0) = -89.5
        let result = lerp_angle(0.0, 181.0, 0.5);
        assert!(
            (result - (-89.5)).abs() < 0.001,
            "lerp_angle(0, 181, 0.5) = {}, expected -89.5",
            result
        );
    }

    #[test]
    fn test_lerp_angle_wrapping_at_minus_180_boundary() {
        // From 350 to 10: difference = 10-350 = -340, < -180, so a1 += 360
        // C: a1=10, a2=350, (10-350)=-340 < -180 => a1 = 10+360 = 370
        // result = 350 + 0.5*(370 - 350) = 350 + 10 = 360
        let result = lerp_angle(350.0, 10.0, 0.5);
        assert!(
            (result - 360.0).abs() < 0.001,
            "lerp_angle(350, 10, 0.5) = {}, expected 360.0",
            result
        );
    }

    #[test]
    fn test_lerp_angle_no_wrapping() {
        // From 10 to 170: difference = 160, no wrapping
        let result = lerp_angle(10.0, 170.0, 0.5);
        assert!(
            (result - 90.0).abs() < 0.001,
            "lerp_angle(10, 170, 0.5) = {}, expected 90.0",
            result
        );
    }

    #[test]
    fn test_lerp_angle_exactly_180_difference() {
        // Exactly 180: (180-0) is not > 180, so no wrap
        // result = 0 + 0.5*(180-0) = 90
        let result = lerp_angle(0.0, 180.0, 0.5);
        assert!(
            (result - 90.0).abs() < 0.001,
            "lerp_angle(0, 180, 0.5) = {}, expected 90.0",
            result
        );
    }

    #[test]
    fn test_lerp_angle_frac_0_returns_a2() {
        let result = lerp_angle(45.0, 270.0, 0.0);
        assert!(
            (result - 45.0).abs() < 0.001,
            "lerp_angle(45, 270, 0) = {}, expected 45.0",
            result
        );
    }

    #[test]
    fn test_lerp_angle_frac_1_returns_a1() {
        // From 45 to 90, frac=1: result = 45 + 1*(90-45) = 90
        let result = lerp_angle(45.0, 90.0, 1.0);
        assert!(
            (result - 90.0).abs() < 0.001,
            "lerp_angle(45, 90, 1.0) = {}, expected 90.0",
            result
        );
    }

    // =========================================================================
    // C-to-Rust cross-validation: box_on_plane_side
    // All 6 axial plane cases + general case
    // =========================================================================

    #[test]
    fn test_box_on_plane_side_axial_x_front() {
        // X-axis plane, box entirely in front
        let mins = [10.0, -5.0, -5.0];
        let maxs = [20.0, 5.0, 5.0];
        let plane = CPlane {
            normal: [1.0, 0.0, 0.0],
            dist: 5.0,      // plane at x=5
            plane_type: 0,   // PLANE_X
            signbits: 0,
            pad: [0; 2],
        };
        // dist(5) <= emins[0](10) => front
        assert_eq!(box_on_plane_side(&mins, &maxs, &plane), 1);
    }

    #[test]
    fn test_box_on_plane_side_axial_x_back() {
        // X-axis plane, box entirely behind
        let mins = [-20.0, -5.0, -5.0];
        let maxs = [-10.0, 5.0, 5.0];
        let plane = CPlane {
            normal: [1.0, 0.0, 0.0],
            dist: -5.0,      // plane at x=-5
            plane_type: 0,
            signbits: 0,
            pad: [0; 2],
        };
        // dist(-5) >= emaxs[0](-10) => back
        assert_eq!(box_on_plane_side(&mins, &maxs, &plane), 2);
    }

    #[test]
    fn test_box_on_plane_side_axial_y_crossing() {
        // Y-axis plane, box crosses
        let mins = [-5.0, -10.0, -5.0];
        let maxs = [5.0, 10.0, 5.0];
        let plane = CPlane {
            normal: [0.0, 1.0, 0.0],
            dist: 0.0,
            plane_type: 1,   // PLANE_Y
            signbits: 0,
            pad: [0; 2],
        };
        // dist(0) is between mins[1](-10) and maxs[1](10) => crossing
        assert_eq!(box_on_plane_side(&mins, &maxs, &plane), 3);
    }

    #[test]
    fn test_box_on_plane_side_axial_z_front() {
        let mins = [-5.0, -5.0, 50.0];
        let maxs = [5.0, 5.0, 100.0];
        let plane = CPlane {
            normal: [0.0, 0.0, 1.0],
            dist: 10.0,
            plane_type: 2,   // PLANE_Z
            signbits: 0,
            pad: [0; 2],
        };
        // dist(10) <= emins[2](50) => front
        assert_eq!(box_on_plane_side(&mins, &maxs, &plane), 1);
    }

    #[test]
    fn test_box_on_plane_side_axial_z_back() {
        let mins = [-5.0, -5.0, -100.0];
        let maxs = [5.0, 5.0, -50.0];
        let plane = CPlane {
            normal: [0.0, 0.0, 1.0],
            dist: -10.0,
            plane_type: 2,
            signbits: 0,
            pad: [0; 2],
        };
        // dist(-10) >= emaxs[2](-50) => back
        assert_eq!(box_on_plane_side(&mins, &maxs, &plane), 2);
    }

    #[test]
    fn test_box_on_plane_side_axial_y_front() {
        let mins = [-5.0, 100.0, -5.0];
        let maxs = [5.0, 200.0, 5.0];
        let plane = CPlane {
            normal: [0.0, 1.0, 0.0],
            dist: 50.0,
            plane_type: 1,
            signbits: 0,
            pad: [0; 2],
        };
        assert_eq!(box_on_plane_side(&mins, &maxs, &plane), 1);
    }

    #[test]
    fn test_box_on_plane_side_general_signbits_0() {
        // General case: normal = (0.577, 0.577, 0.577) ≈ normalized (1,1,1)
        // signbits = 0 (all positive)
        let n = 1.0 / 3.0f32.sqrt();
        let mins = [-10.0, -10.0, -10.0];
        let maxs = [10.0, 10.0, 10.0];
        let plane = CPlane {
            normal: [n, n, n],
            dist: 0.0,
            plane_type: 3, // non-axial
            signbits: 0,   // all positive normals
            pad: [0; 2],
        };
        // dist1 = n*10 + n*10 + n*10 = 30n ≈ 17.32 >= 0 => sides |= 1
        // dist2 = n*(-10) + n*(-10) + n*(-10) = -30n ≈ -17.32 < 0 => sides |= 2
        assert_eq!(box_on_plane_side(&mins, &maxs, &plane), 3);
    }

    #[test]
    fn test_box_on_plane_side_general_signbits_7() {
        // signbits = 7 (all negative normal components)
        let n = -1.0 / 3.0f32.sqrt();
        let mins = [-10.0, -10.0, -10.0];
        let maxs = [10.0, 10.0, 10.0];
        let plane = CPlane {
            normal: [n, n, n],
            dist: 0.0,
            plane_type: 3,
            signbits: 7,   // all negative normals
            pad: [0; 2],
        };
        // signbits=7: dist1 = n*mins + n*mins + n*mins
        // = (-0.577)*(-10)*3 = 17.32 >= 0 => sides |= 1
        // dist2 = n*maxs + n*maxs + n*maxs = (-0.577)*10*3 = -17.32 < 0 => sides |= 2
        assert_eq!(box_on_plane_side(&mins, &maxs, &plane), 3);
    }

    #[test]
    fn test_box_on_plane_side_general_all_front() {
        // All-positive normal, box fully in front
        let n = 1.0 / 3.0f32.sqrt();
        let mins = [50.0, 50.0, 50.0];
        let maxs = [100.0, 100.0, 100.0];
        let plane = CPlane {
            normal: [n, n, n],
            dist: 10.0,
            plane_type: 3,
            signbits: 0,
            pad: [0; 2],
        };
        // dist1 = n*100*3 >> 10, dist2 = n*50*3 >> 10
        // Both >= dist(10) => sides = 1 (front only)
        assert_eq!(box_on_plane_side(&mins, &maxs, &plane), 1);
    }

    // =========================================================================
    // C-to-Rust cross-validation: angle_vectors
    // Verify forward/right/up for key angles against C reference values
    // =========================================================================

    #[test]
    fn test_angle_vectors_zero_angles_c_compat() {
        // angles = [0,0,0] => forward=[1,0,0], right=[0,-1,0], up=[0,0,1]
        let angles = [0.0, 0.0, 0.0];
        let (fwd, right, up) = angle_vectors_tuple(&angles);

        assert!((fwd[0] - 1.0).abs() < 1e-6, "fwd[0]={}", fwd[0]);
        assert!(fwd[1].abs() < 1e-6, "fwd[1]={}", fwd[1]);
        assert!(fwd[2].abs() < 1e-6, "fwd[2]={}", fwd[2]);

        assert!(right[0].abs() < 1e-6, "right[0]={}", right[0]);
        assert!((right[1] - (-1.0)).abs() < 1e-6, "right[1]={}", right[1]);
        assert!(right[2].abs() < 1e-6, "right[2]={}", right[2]);

        assert!(up[0].abs() < 1e-6, "up[0]={}", up[0]);
        assert!(up[1].abs() < 1e-6, "up[1]={}", up[1]);
        assert!((up[2] - 1.0).abs() < 1e-6, "up[2]={}", up[2]);
    }

    #[test]
    fn test_angle_vectors_yaw_90_c_compat() {
        // angles = [0,90,0] => forward=[0,1,0], right=[1,0,0], up=[0,0,1]
        let angles = [0.0, 90.0, 0.0];
        let (fwd, right, up) = angle_vectors_tuple(&angles);

        assert!(fwd[0].abs() < 1e-5, "fwd[0]={}", fwd[0]);
        assert!((fwd[1] - 1.0).abs() < 1e-5, "fwd[1]={}", fwd[1]);
        assert!(fwd[2].abs() < 1e-5, "fwd[2]={}", fwd[2]);

        // right = -sr*sp*cy + -cr*(-sy) = 0 + cos(0)*sin(90) = 1
        // right[1] = -sr*sp*sy + -cr*cy = 0 + (-1)*cos(90) = 0
        assert!((right[0] - 1.0).abs() < 1e-5, "right[0]={}", right[0]);
        assert!(right[1].abs() < 1e-5, "right[1]={}", right[1]);
        assert!(right[2].abs() < 1e-5, "right[2]={}", right[2]);

        assert!(up[0].abs() < 1e-5, "up[0]={}", up[0]);
        assert!(up[1].abs() < 1e-5, "up[1]={}", up[1]);
        assert!((up[2] - 1.0).abs() < 1e-5, "up[2]={}", up[2]);
    }

    #[test]
    fn test_angle_vectors_pitch_90_c_compat() {
        // angles = [90,0,0] => forward=[0,0,-1]
        let angles = [90.0, 0.0, 0.0];
        let (fwd, right, up) = angle_vectors_tuple(&angles);

        assert!(fwd[0].abs() < 1e-5, "fwd[0]={}", fwd[0]);
        assert!(fwd[1].abs() < 1e-5, "fwd[1]={}", fwd[1]);
        assert!((fwd[2] - (-1.0)).abs() < 1e-5, "fwd[2]={}", fwd[2]);

        // right with pitch=90: -sr*sp*cy + -cr*(-sy) = 0 + (-1)*0 = 0
        // right[1] = 0 + (-1)*1 = -1
        assert!(right[0].abs() < 1e-5, "right[0]={}", right[0]);
        assert!((right[1] - (-1.0)).abs() < 1e-5, "right[1]={}", right[1]);
        assert!(right[2].abs() < 1e-5, "right[2]={}", right[2]);

        // up with pitch=90: cr*sp*cy = 1*1*1 = 1
        // up[1] = cr*sp*sy = 0
        // up[2] = cr*cp = cos(90) ≈ 0
        assert!((up[0] - 1.0).abs() < 1e-5, "up[0]={}", up[0]);
        assert!(up[1].abs() < 1e-5, "up[1]={}", up[1]);
        assert!(up[2].abs() < 1e-5, "up[2]={}", up[2]);
    }

    #[test]
    fn test_angle_vectors_pitch_neg90_c_compat() {
        // angles = [-90,0,0] => forward=[0,0,1] (looking up)
        let angles = [-90.0, 0.0, 0.0];
        let (fwd, _right, _up) = angle_vectors_tuple(&angles);

        assert!(fwd[0].abs() < 1e-5, "fwd[0]={}", fwd[0]);
        assert!(fwd[1].abs() < 1e-5, "fwd[1]={}", fwd[1]);
        assert!((fwd[2] - 1.0).abs() < 1e-5, "fwd[2]={}", fwd[2]);
    }

    #[test]
    fn test_angle_vectors_orthogonality() {
        // For any angle set, forward/right/up should be mutually orthogonal
        let test_angles: &[Vec3] = &[
            [0.0, 0.0, 0.0],
            [30.0, 45.0, 0.0],
            [0.0, 90.0, 0.0],
            [90.0, 0.0, 0.0],
            [-90.0, 0.0, 0.0],
            [45.0, 135.0, 30.0],
        ];
        for angles in test_angles {
            let (fwd, right, up) = angle_vectors_tuple(angles);
            let fr = dot_product(&fwd, &right);
            let fu = dot_product(&fwd, &up);
            let ru = dot_product(&right, &up);
            assert!(fr.abs() < 1e-4,
                "fwd.right should be ~0 for {:?}, got {}", angles, fr);
            assert!(fu.abs() < 1e-4,
                "fwd.up should be ~0 for {:?}, got {}", angles, fu);
            assert!(ru.abs() < 1e-4,
                "right.up should be ~0 for {:?}, got {}", angles, ru);
        }
    }

    // =========================================================================
    // C-to-Rust: integer clamp / cast behavior
    // C's implicit float-to-int uses truncation (towards zero)
    // Rust's `as i32` also truncates -- verify this matches
    // =========================================================================

    #[test]
    fn test_float_to_int_truncation_matches_c() {
        // C: (int)(3.7) = 3, (int)(-3.7) = -3
        assert_eq!(3.7f32 as i32, 3);
        assert_eq!(-3.7f32 as i32, -3);
        assert_eq!(0.9f32 as i32, 0);
        assert_eq!(-0.9f32 as i32, 0);
        // Large values
        assert_eq!(1000.999f32 as i32, 1000);
        assert_eq!(-1000.999f32 as i32, -1000);
    }

    // =========================================================================
    // C-to-Rust: com_parse edge cases
    // =========================================================================

    #[test]
    fn test_com_parse_embedded_quotes() {
        // Quoted string
        let (token, rest) = com_parse("\"hello world\" next");
        assert_eq!(token, "hello world");
        assert!(rest.is_some());
        let (token2, _) = com_parse(rest.unwrap());
        assert_eq!(token2, "next");
    }

    #[test]
    fn test_com_parse_comment_block() {
        // // comment followed by newline and then value
        let (token, _) = com_parse("// this is a comment\nactual_token");
        assert_eq!(token, "actual_token");
    }

    #[test]
    fn test_com_parse_multiple_comments() {
        let (token, _) = com_parse("// comment 1\n// comment 2\nvalue");
        assert_eq!(token, "value");
    }

    #[test]
    fn test_com_parse_only_whitespace() {
        let (token, rest) = com_parse("   \t  \n  ");
        assert!(token.is_empty());
        assert!(rest.is_none());
    }

    #[test]
    fn test_com_parse_empty_string() {
        let (token, rest) = com_parse("");
        assert!(token.is_empty());
        assert!(rest.is_none());
    }

    #[test]
    fn test_com_parse_empty_quoted_string() {
        let (token, rest) = com_parse("\"\" next");
        assert_eq!(token, "");
        assert!(rest.is_some());
    }

    #[test]
    fn test_com_parse_newline_in_whitespace() {
        // Newlines count as whitespace (byte value < ' ')
        let (token, _) = com_parse("\n\n\nhello");
        assert_eq!(token, "hello");
    }

    #[test]
    fn test_com_parse_tab_separated() {
        let (token, rest) = com_parse("first\tsecond");
        assert_eq!(token, "first");
        let (token2, _) = com_parse(rest.unwrap());
        assert_eq!(token2, "second");
    }

    // =========================================================================
    // angle2short / short2angle precision cross-validation
    // =========================================================================

    #[test]
    fn test_angle2short_negative_angle() {
        // C: ANGLE2SHORT(-90) = (int)(-90*65536/360) & 65535
        // = (int)(-16384.0) & 65535 = -16384 & 65535 = 49152
        let result = angle2short(-90.0);
        let c_result = ((-90.0f32 * 65536.0 / 360.0) as i32) & 65535;
        assert_eq!(result, c_result, "angle2short(-90) mismatch");
    }

    #[test]
    fn test_angle2short_large_angle() {
        // 720 degrees: should wrap to 0
        let result = angle2short(720.0);
        let c_result = ((720.0f32 * 65536.0 / 360.0) as i32) & 65535;
        assert_eq!(result, c_result);
    }

    #[test]
    fn test_short2angle_negative_short() {
        // Negative shorts represent angles > 180
        let result = short2angle(-16384);
        // C: SHORT2ANGLE(-16384) = (-16384) * (360.0/65536.0) = -90.0
        assert!((result - (-90.0)).abs() < 0.01,
            "short2angle(-16384) = {}, expected -90.0", result);
    }
}
