// client.rs — primary header for client
// Converted from: myq2-original/client/client.h

use myq2_common::q_shared::{
    EntityState, PlayerState, UserCmd, Vec3, MAX_CLIENTS, MAX_CONFIGSTRINGS,
    MAX_IMAGES, MAX_ITEMS, MAX_MODELS, MAX_SOUNDS, MAX_EDICTS,
};
use myq2_common::qcommon::{NetChan, UPDATE_BACKUP};
use crate::cl_smooth::SmoothingState;

// ============================================================
// Limits from ref.h
// ============================================================

pub const MAX_DLIGHTS: usize = 32;
pub const MAX_ENTITIES: usize = 128;
pub const MAX_PARTICLES: usize = 4096;
// MAX_LIGHTSTYLES defined below with other client constants

pub const POWERSUIT_SCALE: f32 = 4.0;

pub const SHELL_RED_COLOR: i32 = 0xF2;
pub const SHELL_GREEN_COLOR: i32 = 0xD0;
pub const SHELL_BLUE_COLOR: i32 = 0xF3;
pub const SHELL_RG_COLOR: i32 = 0xDC;
pub const SHELL_RB_COLOR: i32 = 0x68;
pub const SHELL_BG_COLOR: i32 = 0x78;
pub const SHELL_DOUBLE_COLOR: i32 = 0xDF;
pub const SHELL_HALF_DAM_COLOR: i32 = 0x90;
pub const SHELL_CYAN_COLOR: i32 = 0x72;
pub const SHELL_WHITE_COLOR: i32 = 0xD7;

pub const ENTITY_FLAGS: i32 = 68;
pub const API_VERSION: i32 = 3;

// ============================================================
// Ref types (from ref.h)
// ============================================================

/// entity_t — renderer entity
#[derive(Debug, Clone)]
pub struct Entity {
    pub model: i32,             // model index (opaque type outside refresh)
    pub angles: Vec3,
    pub origin: Vec3,           // also used as RF_BEAM's "from"
    pub frame: i32,             // also used as RF_BEAM's diameter
    pub oldorigin: Vec3,        // also used as RF_BEAM's "to"
    pub oldframe: i32,
    pub backlerp: f32,          // 0.0 = current, 1.0 = old
    pub skinnum: i32,           // also used as RF_BEAM's palette index
    pub lightstyle: i32,        // for flashing entities
    pub alpha: f32,             // ignore if RF_TRANSLUCENT isn't set
    pub skin: i32,              // image index, 0 for inline skin
    pub flags: i32,
}

impl Default for Entity {
    fn default() -> Self {
        Self {
            model: 0,
            angles: [0.0; 3],
            origin: [0.0; 3],
            frame: 0,
            oldorigin: [0.0; 3],
            oldframe: 0,
            backlerp: 0.0,
            skinnum: 0,
            lightstyle: 0,
            alpha: 0.0,
            skin: 0,
            flags: 0,
        }
    }
}

pub use myq2_common::q_shared::{DLight, StainType, DStain};

pub use myq2_common::q_shared::{Particle, LightStyle};

/// refdef_t — renderer scene definition
#[derive(Debug, Clone)]
pub struct RefDef {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub fov_x: f32,
    pub fov_y: f32,
    pub vieworg: Vec3,
    pub viewangles: Vec3,
    pub blend: [f32; 4],       // rgba 0-1 full screen blend
    pub time: f32,             // time is used to auto animate
    pub rdflags: i32,          // RDF_UNDERWATER, etc
    pub areabits: Vec<u8>,
    pub lightstyles: Vec<LightStyle>,
    pub num_entities: i32,
    pub entities: Vec<Entity>,
    pub num_dlights: i32,
    pub dlights: Vec<DLight>,
    pub num_particles: i32,
    pub particles: Vec<Particle>,
}

impl Default for RefDef {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            fov_x: 0.0,
            fov_y: 0.0,
            vieworg: [0.0; 3],
            viewangles: [0.0; 3],
            blend: [0.0; 4],
            time: 0.0,
            rdflags: 0,
            areabits: Vec::new(),
            lightstyles: Vec::new(),
            num_entities: 0,
            entities: Vec::new(),
            num_dlights: 0,
            dlights: Vec::new(),
            num_particles: 0,
            particles: Vec::new(),
        }
    }
}

// ============================================================
// Client constants from client.h
// ============================================================

// MAX_MAP_AREAS imported from myq2_common::qfiles
// MAX_LIGHTSTYLES imported from myq2_common::q_shared
// Re-export for other client modules
pub use myq2_common::qfiles::MAX_MAP_AREAS;
pub use myq2_common::q_shared::MAX_LIGHTSTYLES;

pub const MAX_CLIENTWEAPONMODELS: usize = 20; // PGM -- upped from 16 to fit the chainfist vwep
pub const CMD_BACKUP: usize = 64;             // allow a lot of command backups for very fast systems
pub const MAX_PARSE_ENTITIES: usize = 1024;
pub const MAX_SUSTAINS: usize = 32;

// Sustain effect types (for think callbacks)
pub const SUSTAIN_STEAM: i32 = 1;   // Steam particles
pub const SUSTAIN_WIDOW: i32 = 2;   // Widow splash/beam effect
pub const SUSTAIN_NUKE: i32 = 3;    // Nuclear blast expanding

// Particle constants
pub const PARTICLE_GRAVITY: f32 = 40.0;
pub const BLASTER_PARTICLE_COLOR: i32 = 0xE0;
pub const INSTANT_PARTICLE: f32 = -10000.0;
pub const BEAMLENGTH: f32 = 16.0;

// Image type — canonical definition in myq2_common::q_shared::ImageType
pub use myq2_common::q_shared::ImageType;

// NOTE: The renderer owns the canonical Image struct (vk_model_types::Image).
// The client refers to images via opaque handles (RefImage pointers).

// ============================================================
// frame_t
// ============================================================

#[derive(Debug, Clone)]
pub struct Frame {
    pub valid: bool,               // cleared if delta parsing was invalid
    pub serverframe: i32,
    pub servertime: i32,           // server time the message is valid for (in msec)
    pub deltaframe: i32,
    pub areabits: [u8; MAX_MAP_AREAS / 8], // portalarea visibility bits
    pub playerstate: PlayerState,
    pub num_entities: i32,
    pub parse_entities: i32,       // non-masked index into cl_parse_entities array
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            valid: false,
            serverframe: 0,
            servertime: 0,
            deltaframe: 0,
            areabits: [0u8; MAX_MAP_AREAS / 8],
            playerstate: PlayerState::default(),
            num_entities: 0,
            parse_entities: 0,
        }
    }
}

// ============================================================
// centity_t — client entity
// ============================================================

/// Frame sample for animation history (used in spline interpolation)
#[derive(Debug, Clone, Copy, Default)]
pub struct AnimFrameSample {
    /// Animation frame number
    pub frame: i32,
    /// Time this frame was recorded (server time ms)
    pub time: i32,
}

/// Client-side animation state for smooth animation continuation during packet loss.
#[derive(Debug, Clone)]
pub struct EntityAnimState {
    /// Current animation frame (client-predicted)
    pub frame: i32,
    /// Previous animation frame for lerping
    pub oldframe: i32,
    /// Time accumulated in current frame (ms)
    pub frame_time: f32,
    /// How long this animation frame should last (ms) - estimated from frame delta
    pub frame_duration: f32,
    /// Whether this entity is currently animating
    pub animating: bool,
    /// Last server frame we received animation data
    pub last_server_frame: i32,
    /// Animation sequence type (for predicting next frame)
    pub anim_type: AnimationType,

    // === Spline interpolation for smooth animation ===
    /// History of recent frames for spline interpolation (circular buffer)
    pub frame_history: [AnimFrameSample; 4],
    /// Number of valid samples in history
    pub history_count: usize,
    /// Index of next sample to write
    pub history_index: usize,
    /// Whether spline interpolation is enabled
    pub spline_enabled: bool,
}

impl Default for EntityAnimState {
    fn default() -> Self {
        Self {
            frame: 0,
            oldframe: 0,
            frame_time: 0.0,
            frame_duration: 100.0,
            animating: false,
            last_server_frame: 0,
            anim_type: AnimationType::Unknown,
            frame_history: [AnimFrameSample::default(); 4],
            history_count: 0,
            history_index: 0,
            spline_enabled: true,
        }
    }
}

impl EntityAnimState {
    /// Add a frame sample to the history for spline interpolation
    pub fn add_frame_sample(&mut self, frame: i32, time: i32) {
        // Don't add duplicate frames
        if self.history_count > 0 {
            let prev_idx = if self.history_index == 0 { 3 } else { self.history_index - 1 };
            if self.frame_history[prev_idx].frame == frame {
                return;
            }
        }

        self.frame_history[self.history_index] = AnimFrameSample { frame, time };
        self.history_index = (self.history_index + 1) % 4;
        if self.history_count < 4 {
            self.history_count += 1;
        }
    }

    /// Get smoothed frame and backlerp using Catmull-Rom interpolation
    /// Returns (frame, oldframe, backlerp) for rendering
    pub fn get_spline_frame(&self, current_time: i32) -> Option<(i32, i32, f32)> {
        if !self.spline_enabled || self.history_count < 3 {
            return None;
        }

        // Get the 4 most recent samples in order
        let mut samples: Vec<AnimFrameSample> = Vec::with_capacity(4);
        for i in 0..self.history_count.min(4) {
            let idx = (self.history_index + 4 - self.history_count + i) % 4;
            samples.push(self.frame_history[idx]);
        }

        // Need at least 3 samples for interpolation
        if samples.len() < 3 {
            return None;
        }

        // Find where current_time falls in the sequence
        let n = samples.len();
        for i in 0..(n - 1) {
            let t0 = samples[i].time;
            let t1 = samples[i + 1].time;

            if current_time >= t0 && current_time <= t1 + 100 {
                // Interpolate between these two frames
                let duration = (t1 - t0) as f32;
                if duration <= 0.0 {
                    continue;
                }

                let t = ((current_time - t0) as f32 / duration).clamp(0.0, 1.0);

                // Get frames for Catmull-Rom
                let f0 = if i > 0 { samples[i - 1].frame } else { samples[i].frame };
                let f1 = samples[i].frame;
                let f2 = samples[i + 1].frame;
                let f3 = if i + 2 < n { samples[i + 2].frame } else { samples[i + 1].frame };

                // Catmull-Rom interpolation for frame (treat as float, then round)
                let frame_f = catmull_rom_frame(f0 as f32, f1 as f32, f2 as f32, f3 as f32, t);
                let frame = frame_f.round() as i32;
                let oldframe = if frame > 0 { frame - 1 } else { 0 };

                // Backlerp for vertex interpolation
                let backlerp = (1.0 - (frame_f - frame_f.floor())).clamp(0.0, 1.0);

                return Some((frame.max(0), oldframe.max(0), backlerp));
            }
        }

        None
    }

    /// Clear animation history
    pub fn clear_history(&mut self) {
        self.frame_history = [AnimFrameSample::default(); 4];
        self.history_count = 0;
        self.history_index = 0;
    }
}

/// Catmull-Rom interpolation for animation frames
fn catmull_rom_frame(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;

    0.5 * ((2.0 * p1) +
           (-p0 + p2) * t +
           (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2 +
           (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
}

/// Animation type hints for prediction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimationType {
    #[default]
    Unknown,
    Idle,
    Walk,
    Run,
    Attack,
    Pain,
    Death,
    /// Special rotating item animation
    Rotate,
}

/// Velocity tracking for entity extrapolation
#[derive(Debug, Clone, Default)]
pub struct EntityVelocity {
    /// Estimated velocity (units per second)
    pub velocity: Vec3,
    /// Last time velocity was updated
    pub last_update_time: i32,
    /// Whether velocity is valid for extrapolation
    pub valid: bool,
    /// Previous origin for velocity calculation
    pub prev_origin: Vec3,
    /// Time of previous origin sample
    pub prev_time: i32,

    // === Angular velocity for rotation extrapolation ===
    /// Angular velocity (degrees per second) for each axis
    pub angular_velocity: Vec3,
    /// Previous angles for angular velocity calculation
    pub prev_angles: Vec3,
    /// Whether angular velocity is valid for extrapolation
    pub angular_valid: bool,
}

#[derive(Debug, Clone)]
pub struct CEntity {
    pub baseline: EntityState,     // delta from this if not from a previous frame
    pub current: EntityState,
    pub prev: EntityState,         // will always be valid, but might just be a copy of current
    pub serverframe: i32,          // if not current, this ent isn't in the frame
    pub trailcount: i32,           // for diminishing grenade trails
    pub lerp_origin: Vec3,         // for trails (variable hz)
    pub fly_stoptime: i32,

    // === Smoothness improvements ===
    /// Velocity tracking for extrapolation
    pub velocity: EntityVelocity,
    /// Animation state for client-side continuation
    pub anim_state: EntityAnimState,
    /// Last time this entity was updated from server
    pub last_update_time: i32,
    /// Number of consecutive frames this entity was missing from server updates
    pub missed_frames: i32,
    /// Time when this entity first spawned (for fade-in effect)
    pub spawn_time: i32,
    /// Last known shell/powerup effects (for continuation during packet loss)
    pub last_effects: i32,
    /// Last known render flags (for continuation during packet loss)
    pub last_renderfx: i32,
}

impl Default for CEntity {
    fn default() -> Self {
        Self {
            baseline: EntityState::default(),
            current: EntityState::default(),
            prev: EntityState::default(),
            serverframe: 0,
            trailcount: 0,
            lerp_origin: [0.0; 3],
            fly_stoptime: 0,
            velocity: EntityVelocity::default(),
            anim_state: EntityAnimState::default(),
            last_update_time: 0,
            missed_frames: 0,
            spawn_time: 0,
            last_effects: 0,
            last_renderfx: 0,
        }
    }
}

// ============================================================
// clientinfo_t
// ============================================================

#[derive(Debug, Clone)]
pub struct ClientInfo {
    pub name: String,
    pub cinfo: String,
    pub skin: i32,                 // image index
    pub icon: i32,                 // image index
    pub iconname: String,
    pub model: i32,                // model index
    pub weaponmodel: [i32; MAX_CLIENTWEAPONMODELS], // model indices
}

impl Default for ClientInfo {
    fn default() -> Self {
        Self {
            name: String::new(),
            cinfo: String::new(),
            skin: 0,
            icon: 0,
            iconname: String::new(),
            model: 0,
            weaponmodel: [0; MAX_CLIENTWEAPONMODELS],
        }
    }
}

// ============================================================
// client_state_t — wiped completely at every server map change
// ============================================================

#[derive(Debug)]
pub struct ClientState {
    pub timeoutcount: i32,

    pub timedemo_frames: i32,
    pub timedemo_start: i32,

    pub refresh_prepped: bool,     // false if on new level or new ref dll
    pub sound_prepped: bool,       // ambient sounds can start
    pub force_refdef: bool,        // vid has changed, so we can't use a paused refdef

    pub parse_entities: i32,       // index (not anded off) into cl_parse_entities[]

    pub cmd: UserCmd,
    pub cmds: [UserCmd; CMD_BACKUP],           // each message will send several old cmds
    pub cmd_time: [i32; CMD_BACKUP],           // time sent, for calculating pings
    pub predicted_origins: [[i16; 3]; CMD_BACKUP], // for debug comparing against server

    pub predicted_step: f32,                   // for stair up smoothing
    pub predicted_step_time: u32,

    pub predicted_origin: Vec3,    // generated by CL_PredictMovement
    pub predicted_angles: Vec3,
    pub prediction_error: Vec3,

    pub frame: Frame,              // received from server
    pub surpresscount: i32,        // number of messages rate suppressed
    pub frames: Vec<Frame>,        // [UPDATE_BACKUP]

    // the client maintains its own idea of view angles, which are
    // sent to the server each frame. It is cleared to 0 upon entering each level.
    pub viewangles: Vec3,

    pub time: i32,                 // this is the time value that the client
                                   // is rendering at. always <= cls.realtime
    pub lerpfrac: f32,             // between oldframe and frame

    pub refdef: RefDef,

    pub v_forward: Vec3,
    pub v_right: Vec3,
    pub v_up: Vec3,                // set when refdef.angles is set

    //
    // transient data from server
    //
    pub layout: String,            // general 2D overlay (max 1024 chars)
    pub inventory: [i32; MAX_ITEMS],

    //
    // non-gameserver information
    //
    pub cinematictime: i32,        // cls.realtime for first cinematic frame
    pub cinematicframe: i32,
    pub cinematicpalette: [u8; 768],
    pub cinematicpalette_active: bool,
    pub cinematic_file: Option<std::fs::File>,

    //
    // server state information
    //
    pub attractloop: bool,         // running the attract loop, any key will menu
    pub servercount: i32,          // server identification for prespawns
    pub gamedir: String,
    pub playernum: i32,

    pub configstrings: Vec<String>, // [MAX_CONFIGSTRINGS]

    //
    // locally derived information from server state
    //
    pub model_draw: [i32; MAX_MODELS],     // model indices
    pub model_clip: [i32; MAX_MODELS],     // cmodel indices

    pub sound_precache: [i32; MAX_SOUNDS], // sfx indices
    pub image_precache: [i32; MAX_IMAGES], // image indices

    pub clientinfo: Vec<ClientInfo>,       // [MAX_CLIENTS]
    pub baseclientinfo: ClientInfo,

    // === Network smoothness settings ===
    /// Time nudge for interpolation (ms). Negative = more responsive but jittery.
    /// Positive = smoother but more latency. Range: -100 to 100.
    pub cl_timenudge: i32,
    /// Enable velocity-based extrapolation for remote entities
    pub cl_extrapolate: bool,
    /// Maximum extrapolation time (ms) before clamping
    pub cl_extrapolate_max: i32,
    /// Enable client-side animation continuation during packet loss
    pub cl_anim_continue: bool,
    /// Enable projectile prediction/extrapolation
    pub cl_projectile_predict: bool,
    /// Last valid frame time for packet loss detection
    pub last_valid_frame_time: i32,
    /// Number of consecutive frames with packet loss
    pub packet_loss_frames: i32,

    // === Advanced smoothing state ===
    /// Combined smoothing state (adaptive interp, dead reckoning, view smoothing, etc.)
    pub smoothing: SmoothingState,
}

impl Default for ClientState {
    fn default() -> Self {
        Self {
            timeoutcount: 0,
            timedemo_frames: 0,
            timedemo_start: 0,
            refresh_prepped: false,
            sound_prepped: false,
            force_refdef: false,
            parse_entities: 0,
            cmd: UserCmd::default(),
            cmds: [UserCmd::default(); CMD_BACKUP],
            cmd_time: [0; CMD_BACKUP],
            predicted_origins: [[0i16; 3]; CMD_BACKUP],
            predicted_step: 0.0,
            predicted_step_time: 0,
            predicted_origin: [0.0; 3],
            predicted_angles: [0.0; 3],
            prediction_error: [0.0; 3],
            frame: Frame::default(),
            surpresscount: 0,
            frames: {
                let mut v = Vec::with_capacity(UPDATE_BACKUP as usize);
                for _ in 0..UPDATE_BACKUP {
                    v.push(Frame::default());
                }
                v
            },
            viewangles: [0.0; 3],
            time: 0,
            lerpfrac: 0.0,
            refdef: RefDef::default(),
            v_forward: [0.0; 3],
            v_right: [0.0; 3],
            v_up: [0.0; 3],
            layout: String::new(),
            inventory: [0; MAX_ITEMS],
            cinematictime: 0,
            cinematicframe: 0,
            cinematicpalette: [0u8; 768],
            cinematicpalette_active: false,
            cinematic_file: None,
            attractloop: false,
            servercount: 0,
            gamedir: String::new(),
            playernum: 0,
            configstrings: {
                let mut v = Vec::with_capacity(MAX_CONFIGSTRINGS);
                for _ in 0..MAX_CONFIGSTRINGS {
                    v.push(String::new());
                }
                v
            },
            model_draw: [0; MAX_MODELS],
            model_clip: [0; MAX_MODELS],
            sound_precache: [0; MAX_SOUNDS],
            image_precache: [0; MAX_IMAGES],
            clientinfo: {
                let mut v = Vec::with_capacity(MAX_CLIENTS);
                for _ in 0..MAX_CLIENTS {
                    v.push(ClientInfo::default());
                }
                v
            },
            baseclientinfo: ClientInfo::default(),

            // Network smoothness defaults
            cl_timenudge: 0,
            cl_extrapolate: true,
            cl_extrapolate_max: 50,  // max 50ms extrapolation
            cl_anim_continue: true,
            cl_projectile_predict: true,
            last_valid_frame_time: 0,
            packet_loss_frames: 0,

            // Advanced smoothing state
            smoothing: SmoothingState::new(MAX_EDICTS),
        }
    }
}

// ============================================================
// connstate_t — connection state
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i32)]
pub enum ConnState {
    Uninitialized = 0,
    Disconnected = 1,  // not talking to a server
    Connecting = 2,    // sending request packets to the server
    Connected = 3,     // netchan_t established, waiting for svc_serverdata
    Active = 4,        // game views should be displayed
}

// ============================================================
// dltype_t — download type
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum DlType {
    #[default]
    None = 0,
    Model = 1,
    Sound = 2,
    Skin = 3,
    Single = 4,
}

// ============================================================
// keydest_t — key destination
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum KeyDest {
    #[default]
    Game = 0,
    Console = 1,
    Message = 2,
    Menu = 3,
}

// ============================================================
// client_static_t — persistent through server connections
// ============================================================

pub struct ClientStatic {
    pub state: ConnState,
    pub key_dest: KeyDest,

    pub framecount: i32,
    pub realtime: i32,             // always increasing, no clamping, etc
    pub frametime: f32,            // seconds since last frame

    // screen rendering information
    pub disable_screen: f32,       // showing loading plaque between levels
                                   // or changing rendering dlls
                                   // if time gets > 30 seconds ahead, break it
    pub disable_servercount: i32,  // when we receive a frame and cl.servercount
                                   // > cls.disable_servercount, clear disable_screen

    // connection information
    pub servername: String,        // name of server from original connect
    pub connect_time: f32,         // for connection retransmits

    pub quake_port: i32,           // a 16 bit value that allows quake servers
                                   // to work around address translating routers
    pub netchan: NetChan,
    pub server_protocol: i32,      // in case we are doing some kind of version hack

    pub challenge: i32,            // from the server to use for connecting

    // file transfer from server
    pub download_tempname: String,
    pub download_name: String,
    pub download_number: i32,
    pub download_type: DlType,
    pub download_percent: i32,

    // demo recording info must be here, so it isn't cleared on level change
    pub demo_recording: bool,
    pub demo_waiting: bool,        // don't record until a non-delta message is received

    // demo playback info (R1Q2/Q2Pro enhanced demo system)
    pub demo_playing: bool,        // true if playing back a demo
    pub demo_file_path: String,    // path to currently playing demo

    // Auto-reconnect state (R1Q2/Q2Pro feature)
    /// True if auto-reconnect is pending
    pub auto_reconnect_pending: bool,
    /// Number of reconnect attempts so far
    pub auto_reconnect_attempts: i32,
    /// Time of next reconnect attempt (realtime)
    pub auto_reconnect_time: i32,
    /// Last server we were connected to (for reconnect)
    pub last_server: String,
}

impl Default for ClientStatic {
    fn default() -> Self {
        Self {
            state: ConnState::Uninitialized,
            key_dest: KeyDest::Game,
            framecount: 0,
            realtime: 0,
            frametime: 0.0,
            disable_screen: 0.0,
            disable_servercount: 0,
            servername: String::new(),
            connect_time: 0.0,
            quake_port: 0,
            netchan: NetChan::default(),
            server_protocol: 0,
            challenge: 0,
            download_tempname: String::new(),
            download_name: String::new(),
            download_number: 0,
            download_type: DlType::None,
            download_percent: 0,
            demo_recording: false,
            demo_waiting: false,
            demo_playing: false,
            demo_file_path: String::new(),
            auto_reconnect_pending: false,
            auto_reconnect_attempts: 0,
            auto_reconnect_time: 0,
            last_server: String::new(),
        }
    }
}

// ============================================================
// cl_sustain_t — ROGUE sustained effects
// ============================================================

#[derive(Debug, Clone)]
pub struct ClSustain {
    pub id: i32,
    pub sustain_type: i32,
    pub endtime: i32,
    pub nextthink: i32,
    pub thinkinterval: i32,
    pub org: Vec3,
    pub dir: Vec3,
    pub color: i32,
    pub count: i32,
    pub magnitude: i32,
    // think callback is handled via function pointers / closures at call site
    /// Original endtime before packet loss extension
    pub original_endtime: i32,
    /// Whether this effect has been extended during packet loss
    pub extended: bool,
}

impl Default for ClSustain {
    fn default() -> Self {
        Self {
            id: 0,
            sustain_type: 0,
            endtime: 0,
            nextthink: 0,
            thinkinterval: 0,
            org: [0.0; 3],
            dir: [0.0; 3],
            color: 0,
            count: 0,
            magnitude: 0,
            original_endtime: 0,
            extended: false,
        }
    }
}

// ============================================================
// kbutton_t — input button state
// ============================================================

#[derive(Debug, Clone, Copy)]
#[derive(Default)]
pub struct KButton {
    pub down: [i32; 2],       // key nums holding it down
    pub downtime: u32,        // msec timestamp
    pub msec: u32,            // msec down this frame
    pub state: i32,
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use myq2_common::q_shared::{MAX_CLIENTS, MAX_CONFIGSTRINGS, MAX_ITEMS};
    use myq2_common::qcommon::UPDATE_BACKUP;
    use myq2_common::qfiles::MAX_MAP_AREAS;

    // ============================================================
    // Constant validation tests
    // ============================================================

    #[test]
    fn test_max_dlights_constant() {
        assert_eq!(MAX_DLIGHTS, 32);
    }

    #[test]
    fn test_max_entities_constant() {
        assert_eq!(MAX_ENTITIES, 128);
    }

    #[test]
    fn test_max_particles_constant() {
        assert_eq!(MAX_PARTICLES, 4096);
    }

    #[test]
    fn test_max_clientweaponmodels_constant() {
        assert_eq!(MAX_CLIENTWEAPONMODELS, 20);
    }

    #[test]
    fn test_cmd_backup_constant() {
        assert_eq!(CMD_BACKUP, 64);
    }

    #[test]
    fn test_max_parse_entities_constant() {
        assert_eq!(MAX_PARSE_ENTITIES, 1024);
    }

    #[test]
    fn test_max_sustains_constant() {
        assert_eq!(MAX_SUSTAINS, 32);
    }

    #[test]
    fn test_particle_gravity_constant() {
        assert_eq!(PARTICLE_GRAVITY, 40.0);
    }

    #[test]
    fn test_blaster_particle_color_constant() {
        assert_eq!(BLASTER_PARTICLE_COLOR, 0xE0);
    }

    #[test]
    fn test_instant_particle_constant() {
        assert_eq!(INSTANT_PARTICLE, -10000.0);
    }

    #[test]
    fn test_beamlength_constant() {
        assert_eq!(BEAMLENGTH, 16.0);
    }

    #[test]
    fn test_powersuit_scale_constant() {
        assert_eq!(POWERSUIT_SCALE, 4.0);
    }

    #[test]
    fn test_api_version_constant() {
        assert_eq!(API_VERSION, 3);
    }

    #[test]
    fn test_entity_flags_constant() {
        assert_eq!(ENTITY_FLAGS, 68);
    }

    // ============================================================
    // Shell color constant tests
    // ============================================================

    #[test]
    fn test_shell_color_constants() {
        assert_eq!(SHELL_RED_COLOR, 0xF2);
        assert_eq!(SHELL_GREEN_COLOR, 0xD0);
        assert_eq!(SHELL_BLUE_COLOR, 0xF3);
        assert_eq!(SHELL_RG_COLOR, 0xDC);
        assert_eq!(SHELL_RB_COLOR, 0x68);
        assert_eq!(SHELL_BG_COLOR, 0x78);
        assert_eq!(SHELL_DOUBLE_COLOR, 0xDF);
        assert_eq!(SHELL_HALF_DAM_COLOR, 0x90);
        assert_eq!(SHELL_CYAN_COLOR, 0x72);
        assert_eq!(SHELL_WHITE_COLOR, 0xD7);
    }

    // ============================================================
    // Sustain effect type constants
    // ============================================================

    #[test]
    fn test_sustain_type_constants() {
        assert_eq!(SUSTAIN_STEAM, 1);
        assert_eq!(SUSTAIN_WIDOW, 2);
        assert_eq!(SUSTAIN_NUKE, 3);
    }

    // ============================================================
    // ConnState enum tests
    // ============================================================

    #[test]
    fn test_connstate_repr_values() {
        assert_eq!(ConnState::Uninitialized as i32, 0);
        assert_eq!(ConnState::Disconnected as i32, 1);
        assert_eq!(ConnState::Connecting as i32, 2);
        assert_eq!(ConnState::Connected as i32, 3);
        assert_eq!(ConnState::Active as i32, 4);
    }

    #[test]
    fn test_connstate_ordering() {
        assert!(ConnState::Uninitialized < ConnState::Disconnected);
        assert!(ConnState::Disconnected < ConnState::Connecting);
        assert!(ConnState::Connecting < ConnState::Connected);
        assert!(ConnState::Connected < ConnState::Active);
    }

    #[test]
    fn test_connstate_equality() {
        assert_eq!(ConnState::Active, ConnState::Active);
        assert_ne!(ConnState::Active, ConnState::Disconnected);
    }

    // ============================================================
    // DlType enum tests
    // ============================================================

    #[test]
    fn test_dltype_repr_values() {
        assert_eq!(DlType::None as i32, 0);
        assert_eq!(DlType::Model as i32, 1);
        assert_eq!(DlType::Sound as i32, 2);
        assert_eq!(DlType::Skin as i32, 3);
        assert_eq!(DlType::Single as i32, 4);
    }

    #[test]
    fn test_dltype_default() {
        let dl: DlType = Default::default();
        assert_eq!(dl, DlType::None);
    }

    // ============================================================
    // KeyDest enum tests
    // ============================================================

    #[test]
    fn test_keydest_repr_values() {
        assert_eq!(KeyDest::Game as i32, 0);
        assert_eq!(KeyDest::Console as i32, 1);
        assert_eq!(KeyDest::Message as i32, 2);
        assert_eq!(KeyDest::Menu as i32, 3);
    }

    #[test]
    fn test_keydest_default() {
        let kd: KeyDest = Default::default();
        assert_eq!(kd, KeyDest::Game);
    }

    // ============================================================
    // AnimationType enum tests
    // ============================================================

    #[test]
    fn test_animation_type_default() {
        let at: AnimationType = Default::default();
        assert_eq!(at, AnimationType::Unknown);
    }

    #[test]
    fn test_animation_type_variants() {
        // Verify all variants exist and are distinct
        let variants = [
            AnimationType::Unknown,
            AnimationType::Idle,
            AnimationType::Walk,
            AnimationType::Run,
            AnimationType::Attack,
            AnimationType::Pain,
            AnimationType::Death,
            AnimationType::Rotate,
        ];
        for i in 0..variants.len() {
            for j in (i + 1)..variants.len() {
                assert_ne!(variants[i], variants[j]);
            }
        }
    }

    // ============================================================
    // Entity struct default tests
    // ============================================================

    #[test]
    fn test_entity_default() {
        let e = Entity::default();
        assert_eq!(e.model, 0);
        assert_eq!(e.angles, [0.0; 3]);
        assert_eq!(e.origin, [0.0; 3]);
        assert_eq!(e.frame, 0);
        assert_eq!(e.oldorigin, [0.0; 3]);
        assert_eq!(e.oldframe, 0);
        assert_eq!(e.backlerp, 0.0);
        assert_eq!(e.skinnum, 0);
        assert_eq!(e.lightstyle, 0);
        assert_eq!(e.alpha, 0.0);
        assert_eq!(e.skin, 0);
        assert_eq!(e.flags, 0);
    }

    // ============================================================
    // Frame struct default tests
    // ============================================================

    #[test]
    fn test_frame_default() {
        let f = Frame::default();
        assert!(!f.valid);
        assert_eq!(f.serverframe, 0);
        assert_eq!(f.servertime, 0);
        assert_eq!(f.deltaframe, 0);
        assert_eq!(f.areabits, [0u8; MAX_MAP_AREAS / 8]);
        assert_eq!(f.num_entities, 0);
        assert_eq!(f.parse_entities, 0);
    }

    #[test]
    fn test_frame_areabits_size() {
        let f = Frame::default();
        // MAX_MAP_AREAS = 256, so areabits should be 32 bytes
        assert_eq!(f.areabits.len(), MAX_MAP_AREAS / 8);
        assert_eq!(f.areabits.len(), 32);
    }

    // ============================================================
    // RefDef struct default tests
    // ============================================================

    #[test]
    fn test_refdef_default() {
        let rd = RefDef::default();
        assert_eq!(rd.x, 0);
        assert_eq!(rd.y, 0);
        assert_eq!(rd.width, 0);
        assert_eq!(rd.height, 0);
        assert_eq!(rd.fov_x, 0.0);
        assert_eq!(rd.fov_y, 0.0);
        assert_eq!(rd.vieworg, [0.0; 3]);
        assert_eq!(rd.viewangles, [0.0; 3]);
        assert_eq!(rd.blend, [0.0; 4]);
        assert_eq!(rd.time, 0.0);
        assert_eq!(rd.rdflags, 0);
        assert!(rd.areabits.is_empty());
        assert!(rd.lightstyles.is_empty());
        assert_eq!(rd.num_entities, 0);
        assert!(rd.entities.is_empty());
        assert_eq!(rd.num_dlights, 0);
        assert!(rd.dlights.is_empty());
        assert_eq!(rd.num_particles, 0);
        assert!(rd.particles.is_empty());
    }

    // ============================================================
    // CEntity struct default tests
    // ============================================================

    #[test]
    fn test_centity_default() {
        let ce = CEntity::default();
        assert_eq!(ce.serverframe, 0);
        assert_eq!(ce.trailcount, 0);
        assert_eq!(ce.lerp_origin, [0.0; 3]);
        assert_eq!(ce.fly_stoptime, 0);
        assert_eq!(ce.last_update_time, 0);
        assert_eq!(ce.missed_frames, 0);
        assert_eq!(ce.spawn_time, 0);
        assert_eq!(ce.last_effects, 0);
        assert_eq!(ce.last_renderfx, 0);
    }

    // ============================================================
    // EntityVelocity default tests
    // ============================================================

    #[test]
    fn test_entity_velocity_default() {
        let ev = EntityVelocity::default();
        assert_eq!(ev.velocity, [0.0; 3]);
        assert_eq!(ev.last_update_time, 0);
        assert!(!ev.valid);
        assert_eq!(ev.prev_origin, [0.0; 3]);
        assert_eq!(ev.prev_time, 0);
        assert_eq!(ev.angular_velocity, [0.0; 3]);
        assert_eq!(ev.prev_angles, [0.0; 3]);
        assert!(!ev.angular_valid);
    }

    // ============================================================
    // EntityAnimState tests
    // ============================================================

    #[test]
    fn test_entity_anim_state_default() {
        let eas = EntityAnimState::default();
        assert_eq!(eas.frame, 0);
        assert_eq!(eas.oldframe, 0);
        assert_eq!(eas.frame_time, 0.0);
        assert_eq!(eas.frame_duration, 100.0);
        assert!(!eas.animating);
        assert_eq!(eas.last_server_frame, 0);
        assert_eq!(eas.anim_type, AnimationType::Unknown);
        assert_eq!(eas.history_count, 0);
        assert_eq!(eas.history_index, 0);
        assert!(eas.spline_enabled);
    }

    #[test]
    fn test_add_frame_sample_basic() {
        let mut eas = EntityAnimState::default();
        eas.add_frame_sample(10, 100);
        assert_eq!(eas.history_count, 1);
        assert_eq!(eas.history_index, 1);
        assert_eq!(eas.frame_history[0].frame, 10);
        assert_eq!(eas.frame_history[0].time, 100);
    }

    #[test]
    fn test_add_frame_sample_multiple() {
        let mut eas = EntityAnimState::default();
        eas.add_frame_sample(10, 100);
        eas.add_frame_sample(11, 200);
        eas.add_frame_sample(12, 300);
        assert_eq!(eas.history_count, 3);
        assert_eq!(eas.history_index, 3);
        assert_eq!(eas.frame_history[0].frame, 10);
        assert_eq!(eas.frame_history[1].frame, 11);
        assert_eq!(eas.frame_history[2].frame, 12);
    }

    #[test]
    fn test_add_frame_sample_wraps_around() {
        let mut eas = EntityAnimState::default();
        eas.add_frame_sample(10, 100);
        eas.add_frame_sample(11, 200);
        eas.add_frame_sample(12, 300);
        eas.add_frame_sample(13, 400);
        // Buffer full (4 slots)
        assert_eq!(eas.history_count, 4);
        assert_eq!(eas.history_index, 0); // wrapped

        // Add one more, overwrites slot 0
        eas.add_frame_sample(14, 500);
        assert_eq!(eas.history_count, 4);
        assert_eq!(eas.history_index, 1);
        assert_eq!(eas.frame_history[0].frame, 14);
    }

    #[test]
    fn test_add_frame_sample_deduplicates() {
        let mut eas = EntityAnimState::default();
        eas.add_frame_sample(10, 100);
        eas.add_frame_sample(10, 200); // same frame, should be skipped
        assert_eq!(eas.history_count, 1);
        assert_eq!(eas.history_index, 1);
    }

    #[test]
    fn test_clear_history() {
        let mut eas = EntityAnimState::default();
        eas.add_frame_sample(10, 100);
        eas.add_frame_sample(11, 200);
        eas.add_frame_sample(12, 300);
        assert_eq!(eas.history_count, 3);

        eas.clear_history();
        assert_eq!(eas.history_count, 0);
        assert_eq!(eas.history_index, 0);
        for s in &eas.frame_history {
            assert_eq!(s.frame, 0);
            assert_eq!(s.time, 0);
        }
    }

    #[test]
    fn test_get_spline_frame_needs_3_samples() {
        let mut eas = EntityAnimState::default();
        eas.add_frame_sample(10, 100);
        eas.add_frame_sample(11, 200);
        // Only 2 samples, should return None
        assert!(eas.get_spline_frame(150).is_none());
    }

    #[test]
    fn test_get_spline_frame_disabled() {
        let mut eas = EntityAnimState::default();
        eas.spline_enabled = false;
        eas.add_frame_sample(10, 100);
        eas.add_frame_sample(11, 200);
        eas.add_frame_sample(12, 300);
        assert!(eas.get_spline_frame(250).is_none());
    }

    #[test]
    fn test_get_spline_frame_at_sample_time() {
        let mut eas = EntityAnimState::default();
        eas.add_frame_sample(10, 100);
        eas.add_frame_sample(11, 200);
        eas.add_frame_sample(12, 300);

        // At exactly time=200 (second sample), should interpolate within [100,200]
        let result = eas.get_spline_frame(200);
        assert!(result.is_some());
        let (frame, oldframe, _backlerp) = result.unwrap();
        assert!(frame >= 0);
        assert!(oldframe >= 0);
    }

    #[test]
    fn test_get_spline_frame_midpoint() {
        let mut eas = EntityAnimState::default();
        eas.add_frame_sample(0, 0);
        eas.add_frame_sample(10, 100);
        eas.add_frame_sample(20, 200);
        eas.add_frame_sample(30, 300);

        // At midpoint t=150, between frames 10 and 20
        let result = eas.get_spline_frame(150);
        assert!(result.is_some());
        let (frame, oldframe, backlerp) = result.unwrap();
        assert!(frame >= 0);
        assert!(oldframe >= 0);
        assert!(backlerp >= 0.0 && backlerp <= 1.0);
    }

    // ============================================================
    // Catmull-Rom interpolation tests
    // ============================================================

    #[test]
    fn test_catmull_rom_at_t0_returns_p1() {
        let result = catmull_rom_frame(0.0, 10.0, 20.0, 30.0, 0.0);
        assert!((result - 10.0).abs() < 0.001,
            "catmull_rom at t=0 should return p1=10.0, got {}", result);
    }

    #[test]
    fn test_catmull_rom_at_t1_returns_p2() {
        let result = catmull_rom_frame(0.0, 10.0, 20.0, 30.0, 1.0);
        assert!((result - 20.0).abs() < 0.001,
            "catmull_rom at t=1 should return p2=20.0, got {}", result);
    }

    #[test]
    fn test_catmull_rom_midpoint_linear() {
        // For uniformly spaced control points, midpoint should be ~average of p1 and p2
        let result = catmull_rom_frame(0.0, 10.0, 20.0, 30.0, 0.5);
        assert!((result - 15.0).abs() < 0.001,
            "catmull_rom at t=0.5 with linear points should be ~15.0, got {}", result);
    }

    #[test]
    fn test_catmull_rom_identical_points() {
        let result = catmull_rom_frame(5.0, 5.0, 5.0, 5.0, 0.5);
        assert!((result - 5.0).abs() < 0.001);
    }

    // ============================================================
    // AnimFrameSample default tests
    // ============================================================

    #[test]
    fn test_anim_frame_sample_default() {
        let afs = AnimFrameSample::default();
        assert_eq!(afs.frame, 0);
        assert_eq!(afs.time, 0);
    }

    // ============================================================
    // ClientInfo default tests
    // ============================================================

    #[test]
    fn test_clientinfo_default() {
        let ci = ClientInfo::default();
        assert!(ci.name.is_empty());
        assert!(ci.cinfo.is_empty());
        assert_eq!(ci.skin, 0);
        assert_eq!(ci.icon, 0);
        assert!(ci.iconname.is_empty());
        assert_eq!(ci.model, 0);
        assert_eq!(ci.weaponmodel, [0; MAX_CLIENTWEAPONMODELS]);
    }

    // ============================================================
    // ClientState default tests
    // ============================================================

    #[test]
    fn test_clientstate_default_basic() {
        let cs = ClientState::default();
        assert_eq!(cs.timeoutcount, 0);
        assert_eq!(cs.timedemo_frames, 0);
        assert_eq!(cs.timedemo_start, 0);
        assert!(!cs.refresh_prepped);
        assert!(!cs.sound_prepped);
        assert!(!cs.force_refdef);
        assert_eq!(cs.parse_entities, 0);
    }

    #[test]
    fn test_clientstate_default_prediction() {
        let cs = ClientState::default();
        assert_eq!(cs.predicted_step, 0.0);
        assert_eq!(cs.predicted_step_time, 0);
        assert_eq!(cs.predicted_origin, [0.0; 3]);
        assert_eq!(cs.predicted_angles, [0.0; 3]);
        assert_eq!(cs.prediction_error, [0.0; 3]);
    }

    #[test]
    fn test_clientstate_default_view() {
        let cs = ClientState::default();
        assert_eq!(cs.viewangles, [0.0; 3]);
        assert_eq!(cs.time, 0);
        assert_eq!(cs.lerpfrac, 0.0);
        assert_eq!(cs.v_forward, [0.0; 3]);
        assert_eq!(cs.v_right, [0.0; 3]);
        assert_eq!(cs.v_up, [0.0; 3]);
    }

    #[test]
    fn test_clientstate_default_frames_vec() {
        let cs = ClientState::default();
        assert_eq!(cs.frames.len(), UPDATE_BACKUP as usize);
        for frame in &cs.frames {
            assert!(!frame.valid);
            assert_eq!(frame.serverframe, 0);
        }
    }

    #[test]
    fn test_clientstate_default_configstrings() {
        let cs = ClientState::default();
        assert_eq!(cs.configstrings.len(), MAX_CONFIGSTRINGS);
        for s in &cs.configstrings {
            assert!(s.is_empty());
        }
    }

    #[test]
    fn test_clientstate_default_clientinfo() {
        let cs = ClientState::default();
        assert_eq!(cs.clientinfo.len(), MAX_CLIENTS);
    }

    #[test]
    fn test_clientstate_default_inventory() {
        let cs = ClientState::default();
        assert_eq!(cs.inventory, [0; MAX_ITEMS]);
    }

    #[test]
    fn test_clientstate_default_cinematic() {
        let cs = ClientState::default();
        assert_eq!(cs.cinematictime, 0);
        assert_eq!(cs.cinematicframe, 0);
        assert_eq!(cs.cinematicpalette, [0u8; 768]);
        assert!(!cs.cinematicpalette_active);
        assert!(cs.cinematic_file.is_none());
    }

    #[test]
    fn test_clientstate_default_smoothing() {
        let cs = ClientState::default();
        assert_eq!(cs.cl_timenudge, 0);
        assert!(cs.cl_extrapolate);
        assert_eq!(cs.cl_extrapolate_max, 50);
        assert!(cs.cl_anim_continue);
        assert!(cs.cl_projectile_predict);
        assert_eq!(cs.last_valid_frame_time, 0);
        assert_eq!(cs.packet_loss_frames, 0);
    }

    // ============================================================
    // ClientStatic default tests
    // ============================================================

    #[test]
    fn test_clientstatic_default() {
        let cls = ClientStatic::default();
        assert_eq!(cls.state, ConnState::Uninitialized);
        assert_eq!(cls.key_dest, KeyDest::Game);
        assert_eq!(cls.framecount, 0);
        assert_eq!(cls.realtime, 0);
        assert_eq!(cls.frametime, 0.0);
        assert_eq!(cls.disable_screen, 0.0);
        assert_eq!(cls.disable_servercount, 0);
        assert!(cls.servername.is_empty());
        assert_eq!(cls.connect_time, 0.0);
        assert_eq!(cls.quake_port, 0);
        assert_eq!(cls.server_protocol, 0);
        assert_eq!(cls.challenge, 0);
    }

    #[test]
    fn test_clientstatic_default_download() {
        let cls = ClientStatic::default();
        assert!(cls.download_tempname.is_empty());
        assert!(cls.download_name.is_empty());
        assert_eq!(cls.download_number, 0);
        assert_eq!(cls.download_type, DlType::None);
        assert_eq!(cls.download_percent, 0);
    }

    #[test]
    fn test_clientstatic_default_demo() {
        let cls = ClientStatic::default();
        assert!(!cls.demo_recording);
        assert!(!cls.demo_waiting);
        assert!(!cls.demo_playing);
        assert!(cls.demo_file_path.is_empty());
    }

    #[test]
    fn test_clientstatic_default_reconnect() {
        let cls = ClientStatic::default();
        assert!(!cls.auto_reconnect_pending);
        assert_eq!(cls.auto_reconnect_attempts, 0);
        assert_eq!(cls.auto_reconnect_time, 0);
        assert!(cls.last_server.is_empty());
    }

    // ============================================================
    // ClSustain default tests
    // ============================================================

    #[test]
    fn test_clsustain_default() {
        let s = ClSustain::default();
        assert_eq!(s.id, 0);
        assert_eq!(s.sustain_type, 0);
        assert_eq!(s.endtime, 0);
        assert_eq!(s.nextthink, 0);
        assert_eq!(s.thinkinterval, 0);
        assert_eq!(s.org, [0.0; 3]);
        assert_eq!(s.dir, [0.0; 3]);
        assert_eq!(s.color, 0);
        assert_eq!(s.count, 0);
        assert_eq!(s.magnitude, 0);
        assert_eq!(s.original_endtime, 0);
        assert!(!s.extended);
    }

    // ============================================================
    // KButton default tests
    // ============================================================

    #[test]
    fn test_kbutton_default() {
        let kb = KButton::default();
        assert_eq!(kb.down, [0; 2]);
        assert_eq!(kb.downtime, 0);
        assert_eq!(kb.msec, 0);
        assert_eq!(kb.state, 0);
    }

    // ============================================================
    // CMD_BACKUP array sizing tests
    // ============================================================

    #[test]
    fn test_cmd_arrays_sizing() {
        let cs = ClientState::default();
        assert_eq!(cs.cmds.len(), CMD_BACKUP);
        assert_eq!(cs.cmd_time.len(), CMD_BACKUP);
        assert_eq!(cs.predicted_origins.len(), CMD_BACKUP);
    }
}

