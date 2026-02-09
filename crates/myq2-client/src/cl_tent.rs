// cl_tent.rs -- client side temporary entities
// Converted from: myq2-original/client/cl_tent.c
//
// ============================================================
// ARCHITECTURE NOTE: Event Queueing / Batching
// ============================================================
// The temporary entity system already implements effective event
// queueing through its array-based storage (cl_explosions, cl_beams,
// cl_lasers, cl_sustains). This provides:
//
// 1. **Batched storage** - Temp entities stored in fixed arrays
// 2. **Deferred rendering** - cl_add_tent_entities adds to ViewState
// 3. **Packet loss handling** - Beam velocity extrapolation, effect continuation
// 4. **Temporal decoupling** - Creation time vs render time separated
//
// Additional queueing would add complexity without benefit since
// the architecture already decouples parsing from rendering.
// ============================================================

use myq2_common::q_shared::*;
use myq2_common::qcommon::{SizeBuf, UPDATE_MASK};
use myq2_common::common::{com_printf, com_error};

use crate::client::*;
use crate::cl_fx::ClFxState;
use crate::cl_view::ViewState;
use crate::cl_parse::{msg_read_byte, msg_read_short, msg_read_long, msg_read_float,
                       msg_read_pos, msg_read_dir};

// ============================================================
// Types
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ExpType {
    Free = 0,
    Explosion,
    Misc,
    Flash,
    MFlash,
    Poly,
    Poly2,
}

#[derive(Debug, Clone)]
pub struct Explosion {
    pub exp_type: ExpType,
    pub ent: Entity,
    pub frames: i32,
    pub light: f32,
    pub lightcolor: Vec3,
    pub start: f32,
    pub baseframe: i32,
}

impl Default for Explosion {
    fn default() -> Self {
        Self {
            exp_type: ExpType::Free,
            ent: Entity::default(),
            frames: 0,
            light: 0.0,
            lightcolor: [0.0; 3],
            start: 0.0,
            baseframe: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Beam {
    pub entity: i32,
    pub dest_entity: i32,
    pub model: i32, // model index, 0 = none
    pub endtime: i32,
    pub offset: Vec3,
    pub start: Vec3,
    pub end: Vec3,

    // === Endpoint velocity tracking for smooth beam movement ===
    /// Previous end position for velocity calculation
    pub prev_end: Vec3,
    /// Calculated endpoint velocity (units per second)
    pub end_velocity: Vec3,
    /// Last time endpoint was updated
    pub last_update_time: i32,
    /// Whether endpoint velocity is valid
    pub velocity_valid: bool,
}

impl Default for Beam {
    fn default() -> Self {
        Self {
            entity: 0,
            dest_entity: 0,
            model: 0,
            endtime: 0,
            offset: [0.0; 3],
            start: [0.0; 3],
            end: [0.0; 3],
            prev_end: [0.0; 3],
            end_velocity: [0.0; 3],
            last_update_time: 0,
            velocity_valid: false,
        }
    }
}

impl Beam {
    /// Update endpoint with velocity calculation
    pub fn update_endpoint(&mut self, new_end: &Vec3, current_time: i32) {
        // Calculate velocity from position change
        if self.last_update_time > 0 {
            let dt = (current_time - self.last_update_time) as f32 / 1000.0;
            if dt > 0.001 && dt < 1.0 {
                for i in 0..3 {
                    self.end_velocity[i] = (new_end[i] - self.end[i]) / dt;
                }
                self.velocity_valid = true;
            } else if dt >= 1.0 {
                // Too much time, reset velocity
                self.velocity_valid = false;
            }
        }

        self.prev_end = self.end;
        self.end = *new_end;
        self.last_update_time = current_time;
    }

    /// Get extrapolated endpoint position during packet loss
    pub fn get_extrapolated_end(&self, current_time: i32, max_extrap_ms: i32) -> Vec3 {
        if !self.velocity_valid {
            return self.end;
        }

        let dt = (current_time - self.last_update_time) as f32 / 1000.0;
        if dt <= 0.0 || dt > (max_extrap_ms as f32 / 1000.0) {
            return self.end;
        }

        let mut result = self.end;
        for i in 0..3 {
            result[i] += self.end_velocity[i] * dt;
        }
        result
    }

    /// Clear velocity state (for beam reset)
    pub fn clear_velocity(&mut self) {
        self.prev_end = [0.0; 3];
        self.end_velocity = [0.0; 3];
        self.last_update_time = 0;
        self.velocity_valid = false;
    }
}

#[derive(Debug, Clone)]
pub struct Laser {
    pub ent: Entity,
    pub endtime: i32,

    // === Endpoint velocity tracking for smooth laser movement ===
    /// Previous origin position for velocity calculation
    pub prev_origin: Vec3,
    /// Calculated origin velocity (units per second)
    pub origin_velocity: Vec3,
    /// Previous oldorigin (end point) for velocity calculation
    pub prev_oldorigin: Vec3,
    /// Calculated endpoint velocity (units per second)
    pub end_velocity: Vec3,
    /// Last time endpoints were updated
    pub last_update_time: i32,
    /// Whether velocity is valid
    pub velocity_valid: bool,
}

impl Default for Laser {
    fn default() -> Self {
        Self {
            ent: Entity::default(),
            endtime: 0,
            prev_origin: [0.0; 3],
            origin_velocity: [0.0; 3],
            prev_oldorigin: [0.0; 3],
            end_velocity: [0.0; 3],
            last_update_time: 0,
            velocity_valid: false,
        }
    }
}

impl Laser {
    /// Update laser endpoints with velocity calculation for smooth extrapolation
    pub fn update_endpoints(&mut self, origin: &Vec3, oldorigin: &Vec3, current_time: i32) {
        // Calculate velocity from position change
        if self.last_update_time > 0 {
            let dt = (current_time - self.last_update_time) as f32 / 1000.0;
            if dt > 0.001 && dt < 1.0 {
                for i in 0..3 {
                    self.origin_velocity[i] = (origin[i] - self.ent.origin[i]) / dt;
                    self.end_velocity[i] = (oldorigin[i] - self.ent.oldorigin[i]) / dt;
                }
                self.velocity_valid = true;
            } else if dt >= 1.0 {
                // Too much time, reset velocity
                self.velocity_valid = false;
            }
        }

        self.prev_origin = self.ent.origin;
        self.prev_oldorigin = self.ent.oldorigin;
        self.ent.origin = *origin;
        self.ent.oldorigin = *oldorigin;
        self.last_update_time = current_time;
    }

    /// Get extrapolated laser origin during packet loss
    pub fn get_extrapolated_origin(&self, current_time: i32, max_extrap_ms: i32) -> Vec3 {
        if !self.velocity_valid {
            return self.ent.origin;
        }

        let dt = (current_time - self.last_update_time) as f32 / 1000.0;
        if dt <= 0.0 || dt > (max_extrap_ms as f32 / 1000.0) {
            return self.ent.origin;
        }

        let mut result = self.ent.origin;
        for i in 0..3 {
            result[i] += self.origin_velocity[i] * dt;
        }
        result
    }

    /// Get extrapolated laser endpoint during packet loss
    pub fn get_extrapolated_end(&self, current_time: i32, max_extrap_ms: i32) -> Vec3 {
        if !self.velocity_valid {
            return self.ent.oldorigin;
        }

        let dt = (current_time - self.last_update_time) as f32 / 1000.0;
        if dt <= 0.0 || dt > (max_extrap_ms as f32 / 1000.0) {
            return self.ent.oldorigin;
        }

        let mut result = self.ent.oldorigin;
        for i in 0..3 {
            result[i] += self.end_velocity[i] * dt;
        }
        result
    }

    /// Clear velocity state (for laser reset)
    pub fn clear_velocity(&mut self) {
        self.prev_origin = [0.0; 3];
        self.origin_velocity = [0.0; 3];
        self.prev_oldorigin = [0.0; 3];
        self.end_velocity = [0.0; 3];
        self.last_update_time = 0;
        self.velocity_valid = false;
    }
}


/// Sustain think function type.
pub type SustainThinkFn = fn(&mut ClSustain);

// ============================================================
// Constants
// ============================================================

pub const MAX_EXPLOSIONS: usize = 32;
pub const MAX_BEAMS: usize = 32;
pub const MAX_LASERS: usize = 32;

// ============================================================
// Module-level state (equivalent to C globals)
// ============================================================

pub struct TEntState {
    pub cl_explosions: Vec<Explosion>,
    pub cl_beams: Vec<Beam>,
    pub cl_playerbeams: Vec<Beam>,
    pub cl_lasers: Vec<Laser>,
    pub cl_sustains: Vec<ClSustain>,

    // Sound handles (i32, 0 = none)
    pub cl_sfx_ric1: i32,
    pub cl_sfx_ric2: i32,
    pub cl_sfx_ric3: i32,
    pub cl_sfx_lashit: i32,
    pub cl_sfx_spark5: i32,
    pub cl_sfx_spark6: i32,
    pub cl_sfx_spark7: i32,
    pub cl_sfx_railg: i32,
    pub cl_sfx_rockexp: i32,
    pub cl_sfx_grenexp: i32,
    pub cl_sfx_watrexp: i32,
    pub cl_sfx_plasexp: i32,
    pub cl_sfx_footsteps: [i32; 4],
    pub cl_sfx_lightning: i32,
    pub cl_sfx_disrexp: i32,

    // Model handles (i32, 0 = none)
    pub cl_mod_explode: i32,
    pub cl_mod_smoke: i32,
    pub cl_mod_flash: i32,
    pub cl_mod_parasite_segment: i32,
    pub cl_mod_grapple_cable: i32,
    pub cl_mod_parasite_tip: i32,
    pub cl_mod_explo4: i32,
    pub cl_mod_bfg_explo: i32,
    pub cl_mod_powerscreen: i32,
    pub cl_mod_plasmaexplo: i32,
    pub cl_mod_lightning: i32,
    pub cl_mod_heatbeam: i32,
    pub cl_mod_monster_heatbeam: i32,
    pub cl_mod_explo4_big: i32,
}

impl Default for TEntState {
    fn default() -> Self {
        Self {
            cl_explosions: vec![Explosion::default(); MAX_EXPLOSIONS],
            cl_beams: vec![Beam::default(); MAX_BEAMS],
            cl_playerbeams: vec![Beam::default(); MAX_BEAMS],
            cl_lasers: vec![Laser::default(); MAX_LASERS],
            cl_sustains: vec![ClSustain::default(); MAX_SUSTAINS],
            cl_sfx_ric1: 0, cl_sfx_ric2: 0, cl_sfx_ric3: 0,
            cl_sfx_lashit: 0, cl_sfx_spark5: 0, cl_sfx_spark6: 0, cl_sfx_spark7: 0,
            cl_sfx_railg: 0, cl_sfx_rockexp: 0, cl_sfx_grenexp: 0, cl_sfx_watrexp: 0,
            cl_sfx_plasexp: 0, cl_sfx_footsteps: [0; 4],
            cl_sfx_lightning: 0, cl_sfx_disrexp: 0,
            cl_mod_explode: 0, cl_mod_smoke: 0, cl_mod_flash: 0,
            cl_mod_parasite_segment: 0, cl_mod_grapple_cable: 0, cl_mod_parasite_tip: 0,
            cl_mod_explo4: 0, cl_mod_bfg_explo: 0, cl_mod_powerscreen: 0,
            cl_mod_plasmaexplo: 0, cl_mod_lightning: 0, cl_mod_heatbeam: 0,
            cl_mod_monster_heatbeam: 0, cl_mod_explo4_big: 0,
        }
    }
}

impl TEntState {
    /// Extend sustain effect lifetimes during packet loss to prevent abrupt expiration.
    /// Call this when packet loss is detected to keep effects visible longer.
    pub fn cl_extend_sustains_for_packet_loss(&mut self, cl_time: i32, extension_ms: i32) {
        for s in self.cl_sustains.iter_mut() {
            if s.id == 0 {
                continue;
            }

            // Only extend if the effect is about to expire (within 200ms) and not already extended
            if s.endtime < cl_time + 200 && !s.extended {
                // Store original endtime if not already set
                if s.original_endtime == 0 {
                    s.original_endtime = s.endtime;
                }

                // Extend the endtime
                s.endtime = cl_time + extension_ms;
                s.extended = true;
            }
        }
    }

    /// Reset extended sustain effects when packets are received again.
    /// This restores normal expiration behavior.
    pub fn cl_reset_extended_sustains(&mut self) {
        for s in self.cl_sustains.iter_mut() {
            if s.extended {
                s.extended = false;
                // Don't restore original endtime - let the effect run its course
            }
        }
    }
}

// ============================================================
// Placeholder external functions (stubs)
// ============================================================

// Sound/renderer wrappers — delegate to the real implementations
fn s_register_sound(name: &str) -> i32 {
    crate::cl_main::cl_s_register_sound(name)
}
fn r_register_model(name: &str) -> i32 { crate::console::r_register_model(name) }
fn draw_find_pic(name: &str) -> i32 { crate::console::draw_find_pic(name) }

fn s_start_sound(origin: Option<&Vec3>, entnum: i32, channel: i32, sfx: i32, volume: f32, attenuation: f32, timeofs: f32) {
    crate::cl_main::cl_s_start_sound(origin, entnum, channel, sfx, volume, attenuation, timeofs);
}
fn add_stain(pos: &Vec3, intensity: i32, r: i32, g: i32, b: i32, alpha: i32, mode: i32) {
    // SAFETY: single-threaded engine, RENDERER_FNS set at startup
    unsafe {
        (crate::console::RENDERER_FNS.r_add_stain)(
            pos,
            intensity as f32,
            r as f32,
            g as f32,
            b as f32,
            alpha as f32,
            mode,
        );
    }
}

const STAIN_MODULATE: i32 = 0;
const STAIN_SUBTRACT: i32 = 1;

use crate::cl_fx::{qrand as rand_val, frand};

// ============================================================
// Public functions
// ============================================================

/// Register all temp entity sounds.
pub fn cl_register_tent_sounds() {
    crate::cl_main::with_tent_state(|tent| {
        cl_register_tent_sounds_on(tent);
    });
}

/// Register all temp entity sounds on the given state.
pub fn cl_register_tent_sounds_on(ts: &mut TEntState) {
    ts.cl_sfx_ric1 = s_register_sound("world/ric1.wav");
    ts.cl_sfx_ric2 = s_register_sound("world/ric2.wav");
    ts.cl_sfx_ric3 = s_register_sound("world/ric3.wav");
    ts.cl_sfx_lashit = s_register_sound("weapons/lashit.wav");
    ts.cl_sfx_spark5 = s_register_sound("world/spark5.wav");
    ts.cl_sfx_spark6 = s_register_sound("world/spark6.wav");
    ts.cl_sfx_spark7 = s_register_sound("world/spark7.wav");
    ts.cl_sfx_railg = s_register_sound("weapons/railgf1a.wav");
    ts.cl_sfx_rockexp = s_register_sound("weapons/rocklx1a.wav");
    ts.cl_sfx_grenexp = s_register_sound("weapons/grenlx1a.wav");
    ts.cl_sfx_watrexp = s_register_sound("weapons/xpld_wat.wav");

    s_register_sound("player/land1.wav");
    s_register_sound("player/fall2.wav");
    s_register_sound("player/fall1.wav");

    for i in 0..4 {
        let name = format!("player/step{}.wav", i + 1);
        ts.cl_sfx_footsteps[i] = s_register_sound(&name);
    }

    ts.cl_sfx_lightning = s_register_sound("weapons/tesla.wav");
    ts.cl_sfx_disrexp = s_register_sound("weapons/disrupthit.wav");
}

/// Register all temp entity models.
pub fn cl_register_tent_models(ts: &mut TEntState) {
    ts.cl_mod_explode = r_register_model("models/objects/explode/tris.md2");
    ts.cl_mod_smoke = r_register_model("models/objects/smoke/tris.md2");
    ts.cl_mod_flash = r_register_model("models/objects/flash/tris.md2");
    ts.cl_mod_parasite_segment = r_register_model("models/monsters/parasite/segment/tris.md2");
    ts.cl_mod_grapple_cable = r_register_model("models/ctf/segment/tris.md2");
    ts.cl_mod_parasite_tip = r_register_model("models/monsters/parasite/tip/tris.md2");
    ts.cl_mod_explo4 = r_register_model("models/objects/r_explode/tris.md2");
    ts.cl_mod_bfg_explo = r_register_model("sprites/s_bfg2.sp2");
    ts.cl_mod_powerscreen = r_register_model("models/items/armor/effect/tris.md2");

    // Precache additional models
    r_register_model("models/objects/grenade2/tris.md2");
    r_register_model("models/weapons/v_machn/tris.md2");
    r_register_model("models/weapons/v_handgr/tris.md2");
    r_register_model("models/weapons/v_shotg2/tris.md2");
    r_register_model("models/objects/gibs/bone/tris.md2");
    r_register_model("models/objects/gibs/sm_meat/tris.md2");
    r_register_model("models/objects/gibs/bone2/tris.md2");

    draw_find_pic("w_machinegun");
    draw_find_pic("a_bullets");
    draw_find_pic("i_health");
    draw_find_pic("a_grenades");

    // ROGUE
    ts.cl_mod_explo4_big = r_register_model("models/objects/r_explode2/tris.md2");
    ts.cl_mod_lightning = r_register_model("models/proj/lightning/tris.md2");
    ts.cl_mod_heatbeam = r_register_model("models/proj/beam/tris.md2");
    ts.cl_mod_monster_heatbeam = r_register_model("models/proj/widowbeam/tris.md2");
}

/// Clear all temp entities.
pub fn cl_clear_tents(ts: &mut TEntState) {
    for b in ts.cl_beams.iter_mut() { *b = Beam::default(); }
    for e in ts.cl_explosions.iter_mut() { *e = Explosion::default(); }
    for l in ts.cl_lasers.iter_mut() { *l = Laser::default(); }
    for b in ts.cl_playerbeams.iter_mut() { *b = Beam::default(); }
    for s in ts.cl_sustains.iter_mut() { *s = ClSustain::default(); }
}

/// Allocate an explosion slot, reusing the oldest if all are in use.
pub fn cl_alloc_explosion(ts: &mut TEntState, cl_time: i32) -> usize {
    for i in 0..MAX_EXPLOSIONS {
        if ts.cl_explosions[i].exp_type == ExpType::Free {
            ts.cl_explosions[i] = Explosion::default();
            return i;
        }
    }

    let mut oldest_time = cl_time;
    let mut index = 0;
    for i in 0..MAX_EXPLOSIONS {
        if (ts.cl_explosions[i].start as i32) < oldest_time {
            oldest_time = ts.cl_explosions[i].start as i32;
            index = i;
        }
    }
    ts.cl_explosions[index] = Explosion::default();
    index
}

/// Create smoke and flash effects at origin.
pub fn cl_smoke_and_flash(ts: &mut TEntState, cl: &ClientState, origin: &Vec3) {
    let idx = cl_alloc_explosion(ts, cl.time);
    {
        let ex = &mut ts.cl_explosions[idx];
        ex.ent.origin = *origin;
        ex.exp_type = ExpType::Misc;
        ex.frames = 4;
        ex.ent.flags = RF_TRANSLUCENT;
        ex.start = (cl.frame.servertime - 100) as f32;
        ex.ent.model = ts.cl_mod_smoke;
    }

    let idx = cl_alloc_explosion(ts, cl.time);
    {
        let ex = &mut ts.cl_explosions[idx];
        ex.ent.origin = *origin;
        ex.exp_type = ExpType::Flash;
        ex.ent.flags = RF_FULLBRIGHT;
        ex.frames = 2;
        ex.start = (cl.frame.servertime - 100) as f32;
        ex.ent.model = ts.cl_mod_flash;
    }
}

/// Parse particles from network message.
pub fn cl_parse_particles(fx: &mut ClFxState, cl: &ClientState, net_message: &mut SizeBuf) {
    let mut pos: Vec3 = [0.0; 3];
    let mut dir: Vec3 = [0.0; 3];
    msg_read_pos(net_message, &mut pos);
    msg_read_dir(net_message, &mut dir);
    let color = msg_read_byte(net_message);
    let count = msg_read_byte(net_message);
    fx.cl_particle_effect(&pos, &dir, color, count, cl.time as f32);
}

/// Parse a beam from network message.
pub fn cl_parse_beam(ts: &mut TEntState, cl: &ClientState, model: i32, net_message: &mut SizeBuf) -> i32 {
    let ent = msg_read_short(net_message);
    let mut start: Vec3 = [0.0; 3];
    let mut end: Vec3 = [0.0; 3];
    msg_read_pos(net_message, &mut start);
    msg_read_pos(net_message, &mut end);

    // Look for existing beam to update (with velocity tracking)
    for b in ts.cl_beams.iter_mut() {
        if b.entity == ent {
            b.entity = ent; b.model = model; b.endtime = cl.time + 200;
            b.start = start;
            // Update endpoint with velocity tracking for smooth movement
            b.update_endpoint(&end, cl.time);
            vector_clear(&mut b.offset);
            return ent;
        }
    }
    // Create new beam
    for b in ts.cl_beams.iter_mut() {
        if b.model == 0 || b.endtime < cl.time {
            b.entity = ent; b.model = model; b.endtime = cl.time + 200;
            b.start = start; b.end = end; vector_clear(&mut b.offset);
            b.clear_velocity(); // New beam, no velocity yet
            return ent;
        }
    }
    com_printf("beam list overflow!\n");
    ent
}

/// Parse a beam with offset from network message.
pub fn cl_parse_beam2(ts: &mut TEntState, cl: &ClientState, model: i32, net_message: &mut SizeBuf) -> i32 {
    let ent = msg_read_short(net_message);
    let mut start: Vec3 = [0.0; 3];
    let mut end: Vec3 = [0.0; 3];
    let mut offset: Vec3 = [0.0; 3];
    msg_read_pos(net_message, &mut start);
    msg_read_pos(net_message, &mut end);
    msg_read_pos(net_message, &mut offset);

    // Look for existing beam to update (with velocity tracking)
    for b in ts.cl_beams.iter_mut() {
        if b.entity == ent {
            b.entity = ent; b.model = model; b.endtime = cl.time + 200;
            b.start = start;
            // Update endpoint with velocity tracking for smooth movement
            b.update_endpoint(&end, cl.time);
            b.offset = offset;
            return ent;
        }
    }
    // Create new beam
    for b in ts.cl_beams.iter_mut() {
        if b.model == 0 || b.endtime < cl.time {
            b.entity = ent; b.model = model; b.endtime = cl.time + 200;
            b.start = start; b.end = end; b.offset = offset;
            b.clear_velocity(); // New beam, no velocity yet
            return ent;
        }
    }
    com_printf("beam list overflow!\n");
    ent
}

/// Parse a player beam (ROGUE).
pub fn cl_parse_player_beam(ts: &mut TEntState, cl: &ClientState, mut model: i32, net_message: &mut SizeBuf) -> i32 {
    let ent = msg_read_short(net_message);
    let mut start: Vec3 = [0.0; 3];
    let mut end: Vec3 = [0.0; 3];
    let mut offset: Vec3 = [0.0; 3];
    msg_read_pos(net_message, &mut start);
    msg_read_pos(net_message, &mut end);

    if model == ts.cl_mod_heatbeam {
        vector_set(&mut offset, 2.0, 7.0, -3.0);
    } else if model == ts.cl_mod_monster_heatbeam {
        model = ts.cl_mod_heatbeam;
        vector_set(&mut offset, 0.0, 0.0, 0.0);
    } else {
        msg_read_pos(net_message, &mut offset);
    }

    // Look for existing beam to update (with velocity tracking)
    for b in ts.cl_playerbeams.iter_mut() {
        if b.entity == ent {
            b.entity = ent; b.model = model; b.endtime = cl.time + 200;
            b.start = start;
            // Update endpoint with velocity tracking for smooth movement
            b.update_endpoint(&end, cl.time);
            b.offset = offset;
            return ent;
        }
    }
    // Create new beam
    for b in ts.cl_playerbeams.iter_mut() {
        if b.model == 0 || b.endtime < cl.time {
            b.entity = ent; b.model = model; b.endtime = cl.time + 100;
            b.start = start; b.end = end; b.offset = offset;
            b.clear_velocity(); // New beam, no velocity yet
            return ent;
        }
    }
    com_printf("beam list overflow!\n");
    ent
}

/// Parse lightning beam.
pub fn cl_parse_lightning(ts: &mut TEntState, cl: &ClientState, model: i32, net_message: &mut SizeBuf) -> i32 {
    let src_ent = msg_read_short(net_message);
    let dest_ent = msg_read_short(net_message);
    let mut start: Vec3 = [0.0; 3];
    let mut end: Vec3 = [0.0; 3];
    msg_read_pos(net_message, &mut start);
    msg_read_pos(net_message, &mut end);

    // Look for existing beam to update (with velocity tracking)
    for b in ts.cl_beams.iter_mut() {
        if b.entity == src_ent && b.dest_entity == dest_ent {
            b.entity = src_ent; b.dest_entity = dest_ent; b.model = model;
            b.endtime = cl.time + 200; b.start = start;
            // Update endpoint with velocity tracking
            b.update_endpoint(&end, cl.time);
            vector_clear(&mut b.offset);
            return src_ent;
        }
    }
    // Create new beam
    for b in ts.cl_beams.iter_mut() {
        if b.model == 0 || b.endtime < cl.time {
            b.entity = src_ent; b.dest_entity = dest_ent; b.model = model;
            b.endtime = cl.time + 200; b.start = start; b.end = end;
            vector_clear(&mut b.offset);
            b.clear_velocity(); // New beam, no velocity yet
            return src_ent;
        }
    }
    com_printf("beam list overflow!\n");
    src_ent
}

/// Parse laser effect.
pub fn cl_parse_laser(ts: &mut TEntState, cl: &ClientState, colors: u32, net_message: &mut SizeBuf) {
    let mut start: Vec3 = [0.0; 3];
    let mut end: Vec3 = [0.0; 3];
    msg_read_pos(net_message, &mut start);
    msg_read_pos(net_message, &mut end);

    for l in ts.cl_lasers.iter_mut() {
        if l.endtime < cl.time {
            l.ent.flags = RF_TRANSLUCENT | RF_BEAM;
            l.ent.alpha = 0.30;
            l.ent.skinnum = (colors >> ((rand_val() % 4) as u32 * 8)) as i32 & 0xff;
            l.ent.model = 0;
            l.ent.frame = 4;
            l.endtime = cl.time + 100;
            // Update endpoints with velocity tracking for smooth extrapolation during packet loss
            l.update_endpoints(&start, &end, cl.time);
            return;
        }
    }
}

// ============================================================
// ROGUE sustain effects
// ============================================================

pub fn cl_parse_steam(ts: &mut TEntState, fx: &mut ClFxState, cl: &ClientState, net_message: &mut SizeBuf) {
    let mut pos: Vec3 = [0.0; 3];
    let mut dir: Vec3 = [0.0; 3];

    let id = msg_read_short(net_message);
    if id != -1 {
        let mut free_idx: Option<usize> = None;
        for i in 0..MAX_SUSTAINS {
            if ts.cl_sustains[i].id == 0 { free_idx = Some(i); break; }
        }
        if let Some(idx) = free_idx {
            let s = &mut ts.cl_sustains[idx];
            s.id = id;
            s.sustain_type = SUSTAIN_STEAM; // Steam effect type
            s.count = msg_read_byte(net_message);
            msg_read_pos(net_message, &mut s.org);
            msg_read_dir(net_message, &mut s.dir);
            let r = msg_read_byte(net_message);
            s.color = r & 0xff;
            s.magnitude = msg_read_short(net_message);
            let duration = msg_read_long(net_message);
            s.endtime = cl.time + duration;
            s.original_endtime = s.endtime;
            s.extended = false;
            s.thinkinterval = 100;
            s.nextthink = cl.time;
        } else {
            let _cnt = msg_read_byte(net_message);
            msg_read_pos(net_message, &mut pos);
            msg_read_dir(net_message, &mut dir);
            let _r = msg_read_byte(net_message);
            let _magnitude = msg_read_short(net_message);
            let _interval = msg_read_long(net_message);
        }
    } else {
        let cnt = msg_read_byte(net_message);
        msg_read_pos(net_message, &mut pos);
        msg_read_dir(net_message, &mut dir);
        let r = msg_read_byte(net_message);
        let magnitude = msg_read_short(net_message);
        let color = r & 0xff;
        fx.cl_particle_steam_effect(&pos, &dir, color, cnt, magnitude, cl.time as f32);
    }
}

pub fn cl_parse_widow(ts: &mut TEntState, cl: &ClientState, net_message: &mut SizeBuf) {
    let mut pos: Vec3 = [0.0; 3];
    let id = msg_read_short(net_message);

    let mut free_idx: Option<usize> = None;
    for i in 0..MAX_SUSTAINS {
        if ts.cl_sustains[i].id == 0 { free_idx = Some(i); break; }
    }
    if let Some(idx) = free_idx {
        let s = &mut ts.cl_sustains[idx];
        s.id = id;
        s.sustain_type = SUSTAIN_WIDOW; // Widow splash effect type
        msg_read_pos(net_message, &mut s.org);
        s.endtime = cl.time + 2100;
        s.original_endtime = s.endtime;
        s.extended = false;
        s.thinkinterval = 1;
        s.nextthink = cl.time;
    } else {
        msg_read_pos(net_message, &mut pos);
    }
}

pub fn cl_parse_nuke(ts: &mut TEntState, cl: &ClientState, net_message: &mut SizeBuf) {
    let mut pos: Vec3 = [0.0; 3];

    let mut free_idx: Option<usize> = None;
    for i in 0..MAX_SUSTAINS {
        if ts.cl_sustains[i].id == 0 { free_idx = Some(i); break; }
    }
    if let Some(idx) = free_idx {
        let s = &mut ts.cl_sustains[idx];
        s.id = 21000;
        s.sustain_type = SUSTAIN_NUKE; // Nuke blast effect type
        msg_read_pos(net_message, &mut s.org);
        s.endtime = cl.time + 1000;
        s.original_endtime = s.endtime;
        s.extended = false;
        s.thinkinterval = 1;
        s.nextthink = cl.time;
    } else {
        msg_read_pos(net_message, &mut pos);
    }
}

// ============================================================
// CL_ParseTEnt
// ============================================================

static SPLASH_COLOR: [u8; 7] = [0x00, 0xe0, 0xb0, 0x50, 0xd0, 0xe0, 0xe8];

/// Parse a temp entity message from the server.
pub fn cl_parse_tent(ts: &mut TEntState, fx: &mut ClFxState, cl: &ClientState, net_message: &mut SizeBuf) {
    let te_type = msg_read_byte(net_message);
    let mut pos: Vec3 = [0.0; 3];
    let mut pos2: Vec3 = [0.0; 3];
    let mut dir: Vec3 = [0.0; 3];

    match te_type {
        x if x == TempEvent::Blood as i32 => {
            msg_read_pos(net_message, &mut pos);
            msg_read_dir(net_message, &mut dir);
            fx.cl_blood_effect(&pos, &dir, 0xe8, 60, cl.time as f32);
            let r = 25 + (rand_val() % 75);
            add_stain(&pos, 20 + (r / 10), 205 + (r / 2), 0, 0, 205 + (r / 2), STAIN_MODULATE);
        }

        x if x == TempEvent::Gunshot as i32
            || x == TempEvent::Sparks as i32
            || x == TempEvent::BulletSparks as i32 =>
        {
            msg_read_pos(net_message, &mut pos);
            msg_read_dir(net_message, &mut dir);
            if te_type == TempEvent::Gunshot as i32 {
                fx.cl_particle_effect(&pos, &dir, 0, 40, cl.time as f32);
            } else {
                fx.cl_particle_effect(&pos, &dir, 0xe0, 6, cl.time as f32);
            }
            if te_type != TempEvent::Sparks as i32 {
                cl_smoke_and_flash(ts, cl, &pos);
                let cnt = rand_val() & 15;
                if cnt == 1 { s_start_sound(Some(&pos), 0, 0, ts.cl_sfx_ric1, 1.0, ATTN_NORM, 0.0); }
                else if cnt == 2 { s_start_sound(Some(&pos), 0, 0, ts.cl_sfx_ric2, 1.0, ATTN_NORM, 0.0); }
                else if cnt == 3 { s_start_sound(Some(&pos), 0, 0, ts.cl_sfx_ric3, 1.0, ATTN_NORM, 0.0); }
            }
        }

        x if x == TempEvent::ScreenSparks as i32 || x == TempEvent::ShieldSparks as i32 => {
            msg_read_pos(net_message, &mut pos);
            msg_read_dir(net_message, &mut dir);
            if te_type == TempEvent::ScreenSparks as i32 { fx.cl_particle_effect(&pos, &dir, 0xd0, 40, cl.time as f32); }
            else { fx.cl_particle_effect(&pos, &dir, 0xb0, 40, cl.time as f32); }
            s_start_sound(Some(&pos), 0, 0, ts.cl_sfx_lashit, 1.0, ATTN_NORM, 0.0);
        }

        x if x == TempEvent::Shotgun as i32 => {
            msg_read_pos(net_message, &mut pos);
            msg_read_dir(net_message, &mut dir);
            fx.cl_particle_effect(&pos, &dir, 0, 20, cl.time as f32);
            cl_smoke_and_flash(ts, cl, &pos);
            let r = 79 + (rand_val() % 15) + (rand_val() % 15);
            add_stain(&pos, 5, r, r, r, 175 + (rand_val() % 100), STAIN_MODULATE);
        }

        x if x == TempEvent::Splash as i32 => {
            let cnt = msg_read_byte(net_message);
            msg_read_pos(net_message, &mut pos);
            msg_read_dir(net_message, &mut dir);
            let r = msg_read_byte(net_message);
            let color = if r > 6 { 0x00 } else { SPLASH_COLOR[r as usize] as i32 };
            fx.cl_particle_effect(&pos, &dir, color, cnt, cl.time as f32);
            if r == SPLASH_SPARKS {
                let r2 = rand_val() & 3;
                if r2 == 0 { s_start_sound(Some(&pos), 0, 0, ts.cl_sfx_spark5, 1.0, ATTN_STATIC, 0.0); }
                else if r2 == 1 { s_start_sound(Some(&pos), 0, 0, ts.cl_sfx_spark6, 1.0, ATTN_STATIC, 0.0); }
                else { s_start_sound(Some(&pos), 0, 0, ts.cl_sfx_spark7, 1.0, ATTN_STATIC, 0.0); }
            }
        }

        x if x == TempEvent::LaserSparks as i32 => {
            let cnt = msg_read_byte(net_message);
            msg_read_pos(net_message, &mut pos);
            msg_read_dir(net_message, &mut dir);
            let color = msg_read_byte(net_message);
            fx.cl_particle_effect2(&pos, &dir, color, cnt, cl.time as f32);
        }

        x if x == TempEvent::Bluehyperblaster as i32 => {
            msg_read_pos(net_message, &mut pos);
            msg_read_pos(net_message, &mut dir);
            fx.cl_blaster_particles(&pos, &dir, cl.time as f32);
        }

        x if x == TempEvent::Blaster as i32 => {
            msg_read_pos(net_message, &mut pos);
            msg_read_dir(net_message, &mut dir);
            add_stain(&pos, 5, 50, 50, 0, 500, STAIN_SUBTRACT);
            fx.cl_blaster_particles(&pos, &dir, cl.time as f32);

            let idx = cl_alloc_explosion(ts, cl.time);
            let ex = &mut ts.cl_explosions[idx];
            ex.ent.origin = pos;
            ex.ent.angles[0] = dir[2].acos() * RAD_TO_DEG;
            if dir[0] != 0.0 { ex.ent.angles[1] = dir[1].atan2(dir[0]) * RAD_TO_DEG; }
            else if dir[1] > 0.0 { ex.ent.angles[1] = 90.0; }
            else if dir[1] < 0.0 { ex.ent.angles[1] = 270.0; }
            else { ex.ent.angles[1] = 0.0; }
            ex.exp_type = ExpType::Misc;
            ex.ent.flags = RF_FULLBRIGHT | RF_TRANSLUCENT;
            ex.start = (cl.frame.servertime - 100) as f32;
            ex.light = 150.0;
            ex.lightcolor[0] = 1.0; ex.lightcolor[1] = 1.0;
            ex.ent.model = ts.cl_mod_explode;
            ex.frames = 4;
            s_start_sound(Some(&pos), 0, 0, ts.cl_sfx_lashit, 1.0, ATTN_NORM, 0.0);
        }

        x if x == TempEvent::Railtrail as i32 => {
            msg_read_pos(net_message, &mut pos);
            msg_read_pos(net_message, &mut pos2);
            fx.cl_rail_trail(&pos, &pos2, cl.time as f32, false);
            s_start_sound(Some(&pos2), 0, 0, ts.cl_sfx_railg, 1.0, ATTN_NORM, 0.0);
        }

        x if x == TempEvent::Explosion2 as i32 || x == TempEvent::GrenadeExplosion as i32 || x == TempEvent::GrenadeExplosionWater as i32 => {
            msg_read_pos(net_message, &mut pos);
            let r = (rand_val() % 30) + (rand_val() % 30);
            add_stain(&pos, 45, r, r, r, 175 + (rand_val() % 100), STAIN_MODULATE);
            let idx = cl_alloc_explosion(ts, cl.time);
            let ex = &mut ts.cl_explosions[idx];
            ex.ent.origin = pos; ex.exp_type = ExpType::Poly; ex.ent.flags = RF_FULLBRIGHT;
            ex.start = (cl.frame.servertime - 100) as f32; ex.light = 350.0;
            ex.lightcolor = [1.0, 0.5, 0.5];
            ex.ent.model = ts.cl_mod_explo4; ex.frames = 19; ex.baseframe = 30;
            ex.ent.angles[1] = (rand_val() % 360) as f32;
            fx.cl_explosion_particles(&pos, cl.time as f32);
            if te_type == TempEvent::GrenadeExplosionWater as i32 { s_start_sound(Some(&pos), 0, 0, ts.cl_sfx_watrexp, 1.0, ATTN_NORM, 0.0); }
            else { s_start_sound(Some(&pos), 0, 0, ts.cl_sfx_grenexp, 1.0, ATTN_NORM, 0.0); }
        }

        x if x == TempEvent::PlasmaExplosion as i32 => {
            msg_read_pos(net_message, &mut pos);
            let idx = cl_alloc_explosion(ts, cl.time);
            let ex = &mut ts.cl_explosions[idx];
            ex.ent.origin = pos; ex.exp_type = ExpType::Poly; ex.ent.flags = RF_FULLBRIGHT;
            ex.start = (cl.frame.servertime - 100) as f32; ex.light = 350.0;
            ex.lightcolor = [1.0, 0.5, 0.5];
            ex.ent.angles[1] = (rand_val() % 360) as f32;
            ex.ent.model = ts.cl_mod_explo4;
            if frand() < 0.5 { ex.baseframe = 15; }
            ex.frames = 15;
            fx.cl_explosion_particles(&pos, cl.time as f32);
            s_start_sound(Some(&pos), 0, 0, ts.cl_sfx_rockexp, 1.0, ATTN_NORM, 0.0);
        }

        x if x == TempEvent::Explosion1 as i32 || x == TempEvent::Explosion1Big as i32
            || x == TempEvent::RocketExplosion as i32 || x == TempEvent::RocketExplosionWater as i32
            || x == TempEvent::Explosion1Np as i32 =>
        {
            msg_read_pos(net_message, &mut pos);
            let r = (rand_val() % 30) + (rand_val() % 30);
            add_stain(&pos, 45, r, r, r, 175 + (rand_val() % 100), STAIN_MODULATE);
            let idx = cl_alloc_explosion(ts, cl.time);
            let ex = &mut ts.cl_explosions[idx];
            ex.ent.origin = pos; ex.exp_type = ExpType::Poly; ex.ent.flags = RF_FULLBRIGHT;
            ex.start = (cl.frame.servertime - 100) as f32; ex.light = 350.0;
            ex.lightcolor = [1.0, 0.5, 0.5];
            ex.ent.angles[1] = (rand_val() % 360) as f32;
            if te_type != TempEvent::Explosion1Big as i32 { ex.ent.model = ts.cl_mod_explo4; }
            else { ex.ent.model = ts.cl_mod_explo4_big; }
            if frand() < 0.5 { ex.baseframe = 15; }
            ex.frames = 15;
            if te_type != TempEvent::Explosion1Big as i32 && te_type != TempEvent::Explosion1Np as i32 { fx.cl_explosion_particles(&pos, cl.time as f32); }
            if te_type == TempEvent::RocketExplosionWater as i32 { s_start_sound(Some(&pos), 0, 0, ts.cl_sfx_watrexp, 1.0, ATTN_NORM, 0.0); }
            else { s_start_sound(Some(&pos), 0, 0, ts.cl_sfx_rockexp, 1.0, ATTN_NORM, 0.0); }
        }

        x if x == TempEvent::BfgExplosion as i32 => {
            msg_read_pos(net_message, &mut pos);
            let idx = cl_alloc_explosion(ts, cl.time);
            let ex = &mut ts.cl_explosions[idx];
            ex.ent.origin = pos; ex.exp_type = ExpType::Poly; ex.ent.flags = RF_FULLBRIGHT;
            ex.start = (cl.frame.servertime - 100) as f32; ex.light = 350.0;
            ex.lightcolor = [0.0, 1.0, 0.0];
            ex.ent.model = ts.cl_mod_bfg_explo;
            ex.ent.flags |= RF_TRANSLUCENT; ex.ent.alpha = 0.30; ex.frames = 4;
        }

        x if x == TempEvent::BfgBigexplosion as i32 => {
            msg_read_pos(net_message, &mut pos);
            fx.cl_bfg_explosion_particles(&pos, cl.time as f32);
        }

        x if x == TempEvent::BfgLaser as i32 => {
            cl_parse_laser(ts, cl, 0xd0d1d2d3, net_message);
        }

        x if x == TempEvent::Bubbletrail as i32 => {
            msg_read_pos(net_message, &mut pos);
            msg_read_pos(net_message, &mut pos2);
            fx.cl_bubble_trail(&pos, &pos2, cl.time as f32);
        }

        x if x == TempEvent::ParasiteAttack as i32 || x == TempEvent::MedicCableAttack as i32 => {
            cl_parse_beam(ts, cl, ts.cl_mod_parasite_segment, net_message);
        }

        x if x == TempEvent::Bosstport as i32 => {
            msg_read_pos(net_message, &mut pos);
            fx.cl_big_teleport_particles(&pos, cl.time as f32);
            s_start_sound(Some(&pos), 0, 0, s_register_sound("misc/bigtele.wav"), 1.0, ATTN_NONE, 0.0);
        }

        x if x == TempEvent::GrappleCable as i32 => {
            cl_parse_beam2(ts, cl, ts.cl_mod_grapple_cable, net_message);
        }

        x if x == TempEvent::WeldingSparks as i32 => {
            let cnt = msg_read_byte(net_message);
            msg_read_pos(net_message, &mut pos);
            msg_read_dir(net_message, &mut dir);
            let color = msg_read_byte(net_message);
            fx.cl_particle_effect2(&pos, &dir, color, cnt, cl.time as f32);
            let idx = cl_alloc_explosion(ts, cl.time);
            let ex = &mut ts.cl_explosions[idx];
            ex.ent.origin = pos; ex.exp_type = ExpType::Flash; ex.ent.flags = RF_BEAM;
            ex.start = cl.frame.servertime as f32 - 0.1;
            ex.light = (100 + (rand_val() % 75)) as f32;
            ex.lightcolor = [1.0, 1.0, 0.3];
            ex.ent.model = ts.cl_mod_flash; ex.frames = 2;
        }

        x if x == TempEvent::Greenblood as i32 => {
            msg_read_pos(net_message, &mut pos);
            msg_read_dir(net_message, &mut dir);
            fx.cl_particle_effect2(&pos, &dir, 0xdf, 30, cl.time as f32);
        }

        x if x == TempEvent::TunnelSparks as i32 => {
            let cnt = msg_read_byte(net_message);
            msg_read_pos(net_message, &mut pos);
            msg_read_dir(net_message, &mut dir);
            let color = msg_read_byte(net_message);
            fx.cl_particle_effect3(&pos, &dir, color, cnt, cl.time as f32);
        }

        x if x == TempEvent::Blaster2 as i32 || x == TempEvent::Flechette as i32 => {
            msg_read_pos(net_message, &mut pos);
            msg_read_dir(net_message, &mut dir);
            if te_type == TempEvent::Blaster2 as i32 { fx.cl_blaster_particles2(&pos, &dir, 0xd0, cl.time as f32); }
            else { fx.cl_blaster_particles2(&pos, &dir, 0x6f, cl.time as f32); }

            let idx = cl_alloc_explosion(ts, cl.time);
            let ex = &mut ts.cl_explosions[idx];
            ex.ent.origin = pos;
            ex.ent.angles[0] = dir[2].acos() * RAD_TO_DEG;
            if dir[0] != 0.0 { ex.ent.angles[1] = dir[1].atan2(dir[0]) * RAD_TO_DEG; }
            else if dir[1] > 0.0 { ex.ent.angles[1] = 90.0; }
            else if dir[1] < 0.0 { ex.ent.angles[1] = 270.0; }
            else { ex.ent.angles[1] = 0.0; }
            ex.exp_type = ExpType::Misc; ex.ent.flags = RF_FULLBRIGHT | RF_TRANSLUCENT;
            if te_type == TempEvent::Blaster2 as i32 { ex.ent.skinnum = 1; }
            else { ex.ent.skinnum = 2; }
            ex.start = (cl.frame.servertime - 100) as f32; ex.light = 150.0;
            if te_type == TempEvent::Blaster2 as i32 { ex.lightcolor[1] = 1.0; }
            else { ex.lightcolor = [0.19, 0.41, 0.75]; }
            ex.ent.model = ts.cl_mod_explode; ex.frames = 4;
            s_start_sound(Some(&pos), 0, 0, ts.cl_sfx_lashit, 1.0, ATTN_NORM, 0.0);
        }

        x if x == TempEvent::Lightning as i32 => {
            let ent = cl_parse_lightning(ts, cl, ts.cl_mod_lightning, net_message);
            s_start_sound(None, ent, CHAN_WEAPON, ts.cl_sfx_lightning, 1.0, ATTN_NORM, 0.0);
        }

        x if x == TempEvent::Debugtrail as i32 => {
            msg_read_pos(net_message, &mut pos);
            msg_read_pos(net_message, &mut pos2);
            fx.cl_debug_trail(&pos, &pos2, cl.time as f32);
        }

        x if x == TempEvent::PlainExplosion as i32 => {
            msg_read_pos(net_message, &mut pos);
            let r = (rand_val() % 30) + (rand_val() % 30);
            add_stain(&pos, 45, r, r, r, 175 + (rand_val() % 100), STAIN_MODULATE);
            let idx = cl_alloc_explosion(ts, cl.time);
            let ex = &mut ts.cl_explosions[idx];
            ex.ent.origin = pos; ex.exp_type = ExpType::Poly; ex.ent.flags = RF_FULLBRIGHT;
            ex.start = (cl.frame.servertime - 100) as f32; ex.light = 350.0;
            ex.lightcolor = [1.0, 0.5, 0.5];
            ex.ent.angles[1] = (rand_val() % 360) as f32;
            ex.ent.model = ts.cl_mod_explo4;
            if frand() < 0.5 { ex.baseframe = 15; }
            ex.frames = 15;
            // Note: original checks TE_ROCKET_EXPLOSION_WATER here (bug in original)
            if te_type == TempEvent::RocketExplosionWater as i32 { s_start_sound(Some(&pos), 0, 0, ts.cl_sfx_watrexp, 1.0, ATTN_NORM, 0.0); }
            else { s_start_sound(Some(&pos), 0, 0, ts.cl_sfx_rockexp, 1.0, ATTN_NORM, 0.0); }
        }

        x if x == TempEvent::Flashlight as i32 => {
            msg_read_pos(net_message, &mut pos);
            let ent = msg_read_short(net_message);
            fx.cl_flashlight(ent, &pos, cl.time as f32);
        }

        x if x == TempEvent::Forcewall as i32 => {
            msg_read_pos(net_message, &mut pos);
            msg_read_pos(net_message, &mut pos2);
            let color = msg_read_byte(net_message);
            fx.cl_force_wall(&pos, &pos2, color, cl.time as f32);
        }

        x if x == TempEvent::Heatbeam as i32 => { cl_parse_player_beam(ts, cl, ts.cl_mod_heatbeam, net_message); }
        x if x == TempEvent::MonsterHeatbeam as i32 => { cl_parse_player_beam(ts, cl, ts.cl_mod_monster_heatbeam, net_message); }

        x if x == TempEvent::HeatbeamSparks as i32 => {
            msg_read_pos(net_message, &mut pos);
            msg_read_dir(net_message, &mut dir);
            fx.cl_particle_steam_effect(&pos, &dir, 8, 50, 60, cl.time as f32);
            s_start_sound(Some(&pos), 0, 0, ts.cl_sfx_lashit, 1.0, ATTN_NORM, 0.0);
        }

        x if x == TempEvent::HeatbeamSteam as i32 => {
            msg_read_pos(net_message, &mut pos);
            msg_read_dir(net_message, &mut dir);
            fx.cl_particle_steam_effect(&pos, &dir, 0xe0, 20, 60, cl.time as f32);
            s_start_sound(Some(&pos), 0, 0, ts.cl_sfx_lashit, 1.0, ATTN_NORM, 0.0);
        }

        x if x == TempEvent::Steam as i32 => { cl_parse_steam(ts, fx, cl, net_message); }

        x if x == TempEvent::Bubbletrail2 as i32 => {
            msg_read_pos(net_message, &mut pos);
            msg_read_pos(net_message, &mut pos2);
            fx.cl_bubble_trail2(&pos, &pos2, 8, cl.time as f32);
            s_start_sound(Some(&pos), 0, 0, ts.cl_sfx_lashit, 1.0, ATTN_NORM, 0.0);
        }

        x if x == TempEvent::Moreblood as i32 => {
            msg_read_pos(net_message, &mut pos);
            msg_read_dir(net_message, &mut dir);
            fx.cl_blood_effect(&pos, &dir, 0xe8, 250, cl.time as f32);
            let r = 25 + (rand_val() % 75);
            add_stain(&pos, 20 + (r / 10), 205 + (r / 2), 0, 0, 205 + (r / 2), STAIN_MODULATE);
        }

        x if x == TempEvent::ChainfistSmoke as i32 => {
            dir = [0.0, 0.0, 1.0];
            msg_read_pos(net_message, &mut pos);
            fx.cl_particle_smoke_effect(&pos, &dir, 0, 20, 20, cl.time as f32);
        }

        x if x == TempEvent::ElectricSparks as i32 => {
            msg_read_pos(net_message, &mut pos);
            msg_read_dir(net_message, &mut dir);
            fx.cl_particle_effect(&pos, &dir, 0x75, 40, cl.time as f32);
            s_start_sound(Some(&pos), 0, 0, ts.cl_sfx_lashit, 1.0, ATTN_NORM, 0.0);
        }

        x if x == TempEvent::TrackerExplosion as i32 => {
            msg_read_pos(net_message, &mut pos);
            fx.cl_color_flash(&pos, 0, 150.0, -1.0, -1.0, -1.0, cl.time as f32);
            fx.cl_color_explosion_particles(&pos, 0, 1, cl.time as f32);
            s_start_sound(Some(&pos), 0, 0, ts.cl_sfx_disrexp, 1.0, ATTN_NORM, 0.0);
        }

        x if x == TempEvent::TeleportEffect as i32 || x == TempEvent::DballGoal as i32 => {
            msg_read_pos(net_message, &mut pos);
            fx.cl_teleport_particles(&pos, cl.time as f32);
        }

        x if x == TempEvent::Widowbeamout as i32 => { cl_parse_widow(ts, cl, net_message); }
        x if x == TempEvent::Nukeblast as i32 => { cl_parse_nuke(ts, cl, net_message); }

        x if x == TempEvent::Widowsplash as i32 => {
            msg_read_pos(net_message, &mut pos);
            fx.cl_widow_splash(&pos, cl.time as f32);
        }

        x if x == TempEvent::Stain as i32 => {
            msg_read_pos(net_message, &mut pos);
            let intens = msg_read_float(net_message);
            let mut color = [0i32; 3];
            for c in color.iter_mut() { *c = -(msg_read_byte(net_message)); }
            add_stain(&pos, intens as i32, color[0], color[1], color[2], 256, STAIN_SUBTRACT);
        }

        _ => {
            com_error(ERR_DROP, "CL_ParseTEnt: bad type");
        }
    }
}

// ============================================================
// CL_AddBeams
// ============================================================

/// Lookup function type for getting entity interpolated positions.
/// Returns the lerp_origin for the given entity number, or None if not found/invalid.
pub type EntityLookupFn<'a> = &'a dyn Fn(i32) -> Option<Vec3>;

pub fn cl_add_beams(
    ts: &TEntState,
    cl: &ClientState,
    view: &mut ViewState,
    entity_lookup: EntityLookupFn,
) {
    // === Adaptive beam extrapolation limit based on network jitter ===
    // Use 2x jitter + 100ms base, capped at 500ms, minimum 100ms
    // This allows beams to continue smoothly on high-jitter connections
    // while limiting extrapolation on low-jitter connections
    let jitter = cl.smoothing.network_stats.jitter;
    let adaptive_extrap_ms = ((jitter * 2 + 100) as i32).clamp(100, 500);

    for b in ts.cl_beams.iter() {
        if b.model == 0 || b.endtime < cl.time { continue; }

        // Determine beam start position
        let mut start = b.start;
        if b.entity == cl.playernum + 1 {
            // Local player - use current view position
            start = cl.refdef.vieworg;
            start[2] -= 22.0;
        } else if b.entity > 0 {
            // Other entity - use interpolated position if available
            if let Some(ent_pos) = entity_lookup(b.entity) {
                start = ent_pos;
            }
        }

        // Determine beam end position - use dest_entity's interpolated position if valid
        // During packet loss, use velocity-extrapolated endpoint for smooth movement
        let end = if b.dest_entity > 0 {
            // Try to get entity position from lookup first
            if let Some(ent_pos) = entity_lookup(b.dest_entity) {
                ent_pos
            } else if b.velocity_valid && cl.packet_loss_frames > 0 {
                // Entity not available (packet loss) - use velocity extrapolation
                b.get_extrapolated_end(cl.time, adaptive_extrap_ms)
            } else {
                b.end
            }
        } else if b.velocity_valid && cl.packet_loss_frames > 0 {
            // No dest_entity but we have velocity - extrapolate during packet loss
            b.get_extrapolated_end(cl.time, adaptive_extrap_ms)
        } else {
            b.end
        };

        let org = vector_add(&start, &b.offset);
        let mut dist = vector_subtract(&end, &org);

        let (yaw, pitch) = calc_beam_angles(&dist);

        let mut d = vector_normalize(&mut dist);
        let mut ent = Entity::default();
        let model_length = if b.model == ts.cl_mod_lightning { d -= 20.0; 35.0 } else { 30.0 };
        let steps = (d / model_length).ceil();
        let len = if steps > 1.0 { (d - model_length) / (steps - 1.0) } else { 0.0 };

        if b.model == ts.cl_mod_lightning && d <= model_length {
            ent.origin = end; ent.model = b.model; ent.flags = RF_FULLBRIGHT;
            ent.angles[0] = pitch; ent.angles[1] = yaw; ent.angles[2] = (rand_val() % 360) as f32;
            crate::cl_view::v_add_entity(view, &ent);
            continue;
        }
        let mut org = org;
        while d > 0.0 {
            ent.origin = org; ent.model = b.model;
            if b.model == ts.cl_mod_lightning {
                ent.flags = RF_FULLBRIGHT; ent.angles[0] = -pitch; ent.angles[1] = yaw + 180.0;
            } else {
                ent.angles[0] = pitch; ent.angles[1] = yaw;
            }
            ent.angles[2] = (rand_val() % 360) as f32;
            crate::cl_view::v_add_entity(view, &ent);
            for j in 0..3 { org[j] += dist[j] * len; }
            d -= model_length;
        }
    }
}

/// Calculate pitch and yaw for a beam direction vector.
fn calc_beam_angles(dist: &Vec3) -> (f32, f32) {
    if dist[1] == 0.0 && dist[0] == 0.0 {
        (0.0, if dist[2] > 0.0 { 90.0 } else { 270.0 })
    } else {
        let mut y = if dist[0] != 0.0 { dist[1].atan2(dist[0]) * RAD_TO_DEG } else if dist[1] > 0.0 { 90.0 } else { 270.0 };
        if y < 0.0 { y += 360.0; }
        let forward = (dist[0] * dist[0] + dist[1] * dist[1]).sqrt();
        let mut p = dist[2].atan2(forward) * -RAD_TO_DEG;
        if p < 0.0 { p += 360.0; }
        (y, p)
    }
}

/// ROGUE - draw player locked beams.
pub fn cl_add_player_beams(
    ts: &TEntState,
    fx: &mut ClFxState,
    cl: &ClientState,
    view: &mut ViewState,
    hand_value: Option<f32>,
    entity_lookup: EntityLookupFn,
) {
    // === Adaptive beam extrapolation limit based on network jitter ===
    // Use 2x jitter + 100ms base, capped at 500ms, minimum 100ms
    let jitter = cl.smoothing.network_stats.jitter;
    let adaptive_extrap_ms = ((jitter * 2 + 100) as i32).clamp(100, 500);

    let hand_multiplier = match hand_value {
        Some(v) if v == 2.0 => 0.0,
        Some(v) if v == 1.0 => -1.0,
        _ => 1.0,
    };

    for b in ts.cl_playerbeams.iter() {
        if b.model == 0 || b.endtime < cl.time { continue; }

        let org;
        let mut f: Vec3 = [0.0; 3];
        let mut r: Vec3 = [0.0; 3];
        let mut u: Vec3 = [0.0; 3];

        if ts.cl_mod_heatbeam != 0 && b.model == ts.cl_mod_heatbeam {
            if b.entity == cl.playernum + 1 {
                let ps = &cl.frame.playerstate;
                let j = ((cl.frame.serverframe - 1) & UPDATE_MASK) as usize;
                let oldframe = if j < cl.frames.len() && cl.frames[j].serverframe == cl.frame.serverframe - 1 && cl.frames[j].valid { &cl.frames[j] } else { &cl.frame };
                let ops = &oldframe.playerstate;
                let mut start = [0.0f32; 3];
                for k in 0..3 { start[k] = cl.refdef.vieworg[k] + ops.gunoffset[k] + cl.lerpfrac * (ps.gunoffset[k] - ops.gunoffset[k]); }
                let mut o = vector_ma(&start, hand_multiplier * b.offset[0], &cl.v_right);
                o = vector_ma(&o, b.offset[1], &cl.v_forward);
                o = vector_ma(&o, b.offset[2], &cl.v_up);
                if hand_value == Some(2.0) { o = vector_ma(&o, -1.0, &cl.v_up); }
                r = cl.v_right; f = cl.v_forward; u = cl.v_up;
                org = o;
            } else {
                org = b.start;
            }
        } else {
            let mut start = b.start;
            if b.entity == cl.playernum + 1 { start = cl.refdef.vieworg; start[2] -= 22.0; }
            org = vector_add(&start, &b.offset);
        }

        // Use interpolated endpoint if dest_entity is valid
        // During packet loss, use velocity-extrapolated endpoint for smooth movement
        let end = if b.dest_entity > 0 {
            // Try to get entity position from lookup first
            if let Some(ent_pos) = entity_lookup(b.dest_entity) {
                ent_pos
            } else if b.velocity_valid && cl.packet_loss_frames > 0 {
                // Entity not available (packet loss) - use velocity extrapolation
                b.get_extrapolated_end(cl.time, adaptive_extrap_ms)
            } else {
                b.end
            }
        } else if b.velocity_valid && cl.packet_loss_frames > 0 {
            // No dest_entity but we have velocity - extrapolate during packet loss
            b.get_extrapolated_end(cl.time, adaptive_extrap_ms)
        } else {
            b.end
        };

        let mut dist = vector_subtract(&end, &org);

        if ts.cl_mod_heatbeam != 0 && b.model == ts.cl_mod_heatbeam && b.entity == cl.playernum + 1 {
            let len = vector_length(&dist);
            dist = vector_scale(&f, len);
            dist = vector_ma(&dist, hand_multiplier * b.offset[0], &r);
            dist = vector_ma(&dist, b.offset[1], &f);
            dist = vector_ma(&dist, b.offset[2], &u);
        }

        let (yaw, pitch) = calc_beam_angles(&dist);

        let mut framenum = 0;
        if ts.cl_mod_heatbeam != 0 && b.model == ts.cl_mod_heatbeam {
            if b.entity != cl.playernum + 1 {
                framenum = 2;
                let mut ent = Entity::default();
                ent.angles = [-pitch, yaw + 180.0, 0.0];
                let mut af = [0.0f32; 3]; let mut ar = [0.0f32; 3]; let mut au = [0.0f32; 3];
                angle_vectors(&ent.angles, Some(&mut af), Some(&mut ar), Some(&mut au));
                if !vector_compare(&b.offset, &vec3_origin) {
                    // offset adjustment for player models
                } else {
                    fx.cl_monster_plasma_shell(&b.start, cl.time as f32);
                }
            } else { framenum = 1; }
        }

        if ts.cl_mod_heatbeam != 0 && b.model == ts.cl_mod_heatbeam && b.entity == cl.playernum + 1 {
            fx.cl_heatbeam(&org, &dist, &r, &u, cl.time as f32);
        }

        let mut d = vector_normalize(&mut dist);
        let mut ent = Entity::default();
        let model_length = if b.model == ts.cl_mod_heatbeam { 32.0 }
            else if b.model == ts.cl_mod_lightning { d -= 20.0; 35.0 }
            else { 30.0 };
        let steps = (d / model_length).ceil();
        let len = if steps > 1.0 { (d - model_length) / (steps - 1.0) } else { 0.0 };

        if b.model == ts.cl_mod_lightning && d <= model_length {
            ent.origin = end; ent.model = b.model; ent.flags = RF_FULLBRIGHT;
            ent.angles = [pitch, yaw, (rand_val() % 360) as f32];
            crate::cl_view::v_add_entity(view, &ent); continue;
        }
        let mut org = org;
        while d > 0.0 {
            ent.origin = org; ent.model = b.model;
            if ts.cl_mod_heatbeam != 0 && b.model == ts.cl_mod_heatbeam {
                ent.flags = RF_FULLBRIGHT; ent.angles = [-pitch, yaw + 180.0, (cl.time % 360) as f32]; ent.frame = framenum;
            } else if b.model == ts.cl_mod_lightning {
                ent.flags = RF_FULLBRIGHT; ent.angles = [-pitch, yaw + 180.0, (rand_val() % 360) as f32];
            } else {
                ent.angles = [pitch, yaw, (rand_val() % 360) as f32];
            }
            crate::cl_view::v_add_entity(view, &ent);
            for j in 0..3 { org[j] += dist[j] * len; }
            d -= model_length;
        }
    }
}

// ============================================================
// CL_AddExplosions
// ============================================================

pub fn cl_add_explosions(ts: &mut TEntState, cl: &ClientState, view: &mut ViewState) {
    for ex in ts.cl_explosions.iter_mut() {
        if ex.exp_type == ExpType::Free { continue; }
        let frac = (cl.time as f32 - ex.start) / 100.0;
        let f = frac.floor() as i32;
        let ent = &mut ex.ent;

        match ex.exp_type {
            ExpType::MFlash => { if f >= ex.frames - 1 { ex.exp_type = ExpType::Free; } }
            ExpType::Misc => {
                if f >= ex.frames - 1 { ex.exp_type = ExpType::Free; }
                else { ent.alpha = 1.0 - frac / (ex.frames - 1) as f32; }
            }
            ExpType::Flash => {
                if f >= 1 { ex.exp_type = ExpType::Free; }
                else { ent.alpha = 1.0; }
            }
            ExpType::Poly => {
                if f >= ex.frames - 1 { ex.exp_type = ExpType::Free; }
                else {
                    ent.alpha = (16.0 - f as f32) / 16.0;
                    if f < 10 { ent.skinnum = f >> 1; if ent.skinnum < 0 { ent.skinnum = 0; } }
                    else { ent.flags |= RF_TRANSLUCENT; if f < 13 { ent.skinnum = 5; } else { ent.skinnum = 6; } }
                }
            }
            ExpType::Poly2 => {
                if f >= ex.frames - 1 { ex.exp_type = ExpType::Free; }
                else { ent.alpha = (5.0 - f as f32) / 5.0; ent.skinnum = 0; ent.flags |= RF_TRANSLUCENT; }
            }
            _ => {}
        }

        if ex.exp_type == ExpType::Free { continue; }
        if ex.light != 0.0 { crate::cl_view::v_add_light(view, &ent.origin, ex.light * ent.alpha, ex.lightcolor[0], ex.lightcolor[1], ex.lightcolor[2]); }

        ent.oldorigin = ent.origin;
        let f_clamped = if f < 0 { 0 } else { f };
        ent.frame = ex.baseframe + f_clamped + 1;
        ent.oldframe = ex.baseframe + f_clamped;
        ent.backlerp = 1.0 - cl.lerpfrac;
        crate::cl_view::v_add_entity(view, ent);
    }
}

pub fn cl_add_lasers(ts: &TEntState, cl: &ClientState, view: &mut ViewState) {
    // Calculate adaptive extrapolation time based on network jitter
    let adaptive_extrap_ms = if cl.packet_loss_frames > 0 {
        // During packet loss, use longer extrapolation (up to 300ms)
        (100 + cl.smoothing.adaptive_interp.get_jitter() * 2).min(300)
    } else {
        // Normal operation, use jitter-based extrapolation
        (50 + cl.smoothing.adaptive_interp.get_jitter()).min(150)
    };

    for l in ts.cl_lasers.iter() {
        if l.endtime >= cl.time {
            // During packet loss or when velocity is valid, use extrapolated positions
            if l.velocity_valid && cl.packet_loss_frames > 0 {
                let mut ent = l.ent.clone();
                ent.origin = l.get_extrapolated_origin(cl.time, adaptive_extrap_ms);
                ent.oldorigin = l.get_extrapolated_end(cl.time, adaptive_extrap_ms);
                crate::cl_view::v_add_entity(view, &ent);
            } else {
                crate::cl_view::v_add_entity(view, &l.ent);
            }
        }
    }
}

pub fn cl_process_sustain(ts: &mut TEntState, fx: &mut ClFxState, cl: &ClientState) {
    for s in ts.cl_sustains.iter_mut() {
        if s.id != 0 {
            if s.endtime >= cl.time && cl.time >= s.nextthink {
                // Execute think callback based on sustain type
                match s.sustain_type {
                    SUSTAIN_STEAM => {
                        // Steam effect - spawn steam particles each think
                        fx.cl_particle_steam_effect(
                            &s.org,
                            &s.dir,
                            s.color,
                            s.count,
                            s.magnitude,
                            cl.time as f32,
                        );
                    }
                    SUSTAIN_WIDOW => {
                        // Widow beam/splash effect - expanding beam effect
                        // Use widow splash with calculated ratio for expansion
                        let ratio = 1.0 - ((s.endtime as f32 - cl.time as f32) / 2100.0);
                        fx.cl_widowbeamout(&s.org, ratio, cl.time as f32);
                    }
                    SUSTAIN_NUKE => {
                        // Nuke blast effect - expanding particle sphere
                        let ratio = 1.0 - ((s.endtime as f32 - cl.time as f32) / 1000.0);
                        fx.cl_nukeblast(&s.org, ratio, cl.time as f32);
                    }
                    _ => {
                        // Unknown type - use generic steam effect
                        fx.cl_particle_steam_effect(
                            &s.org,
                            &s.dir,
                            s.color,
                            s.count,
                            s.magnitude,
                            cl.time as f32,
                        );
                    }
                }
                // Schedule next think
                s.nextthink = cl.time + s.thinkinterval;
            } else if s.endtime < cl.time {
                // Effect expired - clear the slot
                s.id = 0;
            }
        }
    }
}

/// Main entry point to add all temp entities.
/// Main entry point to add all temp entities.
/// entity_lookup: Optional closure to get interpolated entity positions for beam smoothing.
pub fn cl_add_tents(
    ts: &mut TEntState,
    fx: &mut ClFxState,
    cl: &ClientState,
    view: &mut ViewState,
    hand_value: Option<f32>,
    entity_lookup: EntityLookupFn,
) {
    cl_add_beams(ts, cl, view, entity_lookup);
    cl_add_player_beams(ts, fx, cl, view, hand_value, entity_lookup);
    cl_add_explosions(ts, cl, view);
    cl_add_lasers(ts, cl, view);
    cl_process_sustain(ts, fx, cl);
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== Beam update_endpoint velocity tracking ==========

    #[test]
    fn beam_default() {
        let b = Beam::default();
        assert_eq!(b.entity, 0);
        assert_eq!(b.dest_entity, 0);
        assert_eq!(b.model, 0);
        assert_eq!(b.endtime, 0);
        assert_eq!(b.end, [0.0; 3]);
        assert!(!b.velocity_valid);
    }

    #[test]
    fn beam_update_endpoint_first_call_no_velocity() {
        let mut b = Beam::default();
        b.update_endpoint(&[100.0, 200.0, 300.0], 1000);
        assert_eq!(b.end, [100.0, 200.0, 300.0]);
        assert_eq!(b.last_update_time, 1000);
        // First call: last_update_time was 0, so no velocity calc
        assert!(!b.velocity_valid);
    }

    #[test]
    fn beam_update_endpoint_second_call_calculates_velocity() {
        let mut b = Beam::default();
        b.end = [0.0, 0.0, 0.0];
        b.last_update_time = 1000;

        b.update_endpoint(&[100.0, 0.0, 0.0], 1100);
        assert!(b.velocity_valid);
        // Velocity = (100 - 0) / 0.1 = 1000 u/s
        assert!((b.end_velocity[0] - 1000.0).abs() < 1.0);
        assert_eq!(b.prev_end, [0.0, 0.0, 0.0]);
        assert_eq!(b.end, [100.0, 0.0, 0.0]);
    }

    #[test]
    fn beam_update_endpoint_large_dt_resets_velocity() {
        let mut b = Beam::default();
        b.end = [0.0; 3];
        b.last_update_time = 1000;
        b.velocity_valid = true;
        b.end_velocity = [500.0, 0.0, 0.0];

        b.update_endpoint(&[100.0, 0.0, 0.0], 2500); // dt = 1.5s >= 1.0
        assert!(!b.velocity_valid);
    }

    #[test]
    fn beam_update_endpoint_tiny_dt_preserves_velocity() {
        let mut b = Beam::default();
        b.end = [0.0; 3];
        b.last_update_time = 1000;
        b.velocity_valid = true;
        b.end_velocity = [500.0, 0.0, 0.0];

        b.update_endpoint(&[0.001, 0.0, 0.0], 1000); // dt = 0
        // dt <= 0.001, so velocity should remain unchanged (valid stays true, no update)
        assert!(b.velocity_valid);
        assert_eq!(b.end_velocity[0], 500.0);
    }

    // ========== Beam get_extrapolated_end ==========

    #[test]
    fn beam_get_extrapolated_end_no_velocity() {
        let b = Beam {
            end: [100.0, 200.0, 300.0],
            velocity_valid: false,
            ..Beam::default()
        };
        let result = b.get_extrapolated_end(5000, 300);
        assert_eq!(result, [100.0, 200.0, 300.0]);
    }

    #[test]
    fn beam_get_extrapolated_end_with_velocity() {
        let b = Beam {
            end: [100.0, 0.0, 0.0],
            end_velocity: [1000.0, 500.0, 0.0],
            velocity_valid: true,
            last_update_time: 1000,
            ..Beam::default()
        };
        // 100ms after last update
        let result = b.get_extrapolated_end(1100, 300);
        assert!((result[0] - 200.0).abs() < 0.1); // 100 + 1000 * 0.1
        assert!((result[1] - 50.0).abs() < 0.1);  // 0 + 500 * 0.1
    }

    #[test]
    fn beam_get_extrapolated_end_exceeds_max() {
        let b = Beam {
            end: [100.0, 0.0, 0.0],
            end_velocity: [1000.0, 0.0, 0.0],
            velocity_valid: true,
            last_update_time: 1000,
            ..Beam::default()
        };
        // 400ms > max 300ms
        let result = b.get_extrapolated_end(1400, 300);
        assert_eq!(result, [100.0, 0.0, 0.0]);
    }

    #[test]
    fn beam_get_extrapolated_end_negative_dt() {
        let b = Beam {
            end: [100.0, 0.0, 0.0],
            end_velocity: [1000.0, 0.0, 0.0],
            velocity_valid: true,
            last_update_time: 2000,
            ..Beam::default()
        };
        let result = b.get_extrapolated_end(1000, 300);
        assert_eq!(result, [100.0, 0.0, 0.0]);
    }

    // ========== Beam clear_velocity ==========

    #[test]
    fn beam_clear_velocity() {
        let mut b = Beam {
            prev_end: [10.0; 3],
            end_velocity: [100.0; 3],
            last_update_time: 5000,
            velocity_valid: true,
            ..Beam::default()
        };
        b.clear_velocity();
        assert_eq!(b.prev_end, [0.0; 3]);
        assert_eq!(b.end_velocity, [0.0; 3]);
        assert_eq!(b.last_update_time, 0);
        assert!(!b.velocity_valid);
    }

    // ========== Laser update_endpoints ==========

    #[test]
    fn laser_default() {
        let l = Laser::default();
        assert_eq!(l.endtime, 0);
        assert!(!l.velocity_valid);
        assert_eq!(l.origin_velocity, [0.0; 3]);
        assert_eq!(l.end_velocity, [0.0; 3]);
    }

    #[test]
    fn laser_update_endpoints_first_call() {
        let mut l = Laser::default();
        let origin = [100.0, 200.0, 300.0];
        let end = [400.0, 500.0, 600.0];
        l.update_endpoints(&origin, &end, 1000);
        assert_eq!(l.ent.origin, origin);
        assert_eq!(l.ent.oldorigin, end);
        assert_eq!(l.last_update_time, 1000);
        assert!(!l.velocity_valid); // first call, no prev time
    }

    #[test]
    fn laser_update_endpoints_second_call_calculates_velocity() {
        let mut l = Laser::default();
        l.ent.origin = [0.0, 0.0, 0.0];
        l.ent.oldorigin = [0.0, 0.0, 0.0];
        l.last_update_time = 1000;

        l.update_endpoints(&[100.0, 0.0, 0.0], &[200.0, 0.0, 0.0], 1100);
        assert!(l.velocity_valid);
        // origin velocity = (100 - 0) / 0.1 = 1000
        assert!((l.origin_velocity[0] - 1000.0).abs() < 1.0);
        // end velocity = (200 - 0) / 0.1 = 2000
        assert!((l.end_velocity[0] - 2000.0).abs() < 1.0);
    }

    #[test]
    fn laser_update_endpoints_large_dt_resets() {
        let mut l = Laser::default();
        l.ent.origin = [0.0; 3];
        l.ent.oldorigin = [0.0; 3];
        l.last_update_time = 1000;
        l.velocity_valid = true;

        l.update_endpoints(&[100.0; 3], &[200.0; 3], 2500); // dt >= 1.0
        assert!(!l.velocity_valid);
    }

    // ========== Laser get_extrapolated_origin ==========

    #[test]
    fn laser_get_extrapolated_origin_no_velocity() {
        let mut l = Laser::default();
        l.ent.origin = [50.0, 60.0, 70.0];
        l.velocity_valid = false;
        let result = l.get_extrapolated_origin(5000, 300);
        assert_eq!(result, [50.0, 60.0, 70.0]);
    }

    #[test]
    fn laser_get_extrapolated_origin_with_velocity() {
        let mut l = Laser::default();
        l.ent.origin = [100.0, 0.0, 0.0];
        l.origin_velocity = [500.0, 0.0, 0.0];
        l.velocity_valid = true;
        l.last_update_time = 1000;

        let result = l.get_extrapolated_origin(1200, 300); // 200ms
        assert!((result[0] - 200.0).abs() < 0.1); // 100 + 500*0.2
    }

    #[test]
    fn laser_get_extrapolated_origin_exceeds_max() {
        let mut l = Laser::default();
        l.ent.origin = [100.0, 0.0, 0.0];
        l.origin_velocity = [500.0, 0.0, 0.0];
        l.velocity_valid = true;
        l.last_update_time = 1000;

        let result = l.get_extrapolated_origin(1400, 300); // 400ms > 300ms
        assert_eq!(result, [100.0, 0.0, 0.0]);
    }

    // ========== Laser get_extrapolated_end ==========

    #[test]
    fn laser_get_extrapolated_end_with_velocity() {
        let mut l = Laser::default();
        l.ent.oldorigin = [200.0, 0.0, 0.0];
        l.end_velocity = [1000.0, 0.0, 0.0];
        l.velocity_valid = true;
        l.last_update_time = 1000;

        let result = l.get_extrapolated_end(1150, 300); // 150ms
        assert!((result[0] - 350.0).abs() < 0.1); // 200 + 1000*0.15
    }

    #[test]
    fn laser_get_extrapolated_end_no_velocity() {
        let mut l = Laser::default();
        l.ent.oldorigin = [200.0, 300.0, 400.0];
        l.velocity_valid = false;
        let result = l.get_extrapolated_end(5000, 300);
        assert_eq!(result, [200.0, 300.0, 400.0]);
    }

    // ========== Laser clear_velocity ==========

    #[test]
    fn laser_clear_velocity() {
        let mut l = Laser::default();
        l.origin_velocity = [100.0; 3];
        l.end_velocity = [200.0; 3];
        l.prev_origin = [50.0; 3];
        l.prev_oldorigin = [60.0; 3];
        l.velocity_valid = true;
        l.last_update_time = 5000;

        l.clear_velocity();
        assert_eq!(l.origin_velocity, [0.0; 3]);
        assert_eq!(l.end_velocity, [0.0; 3]);
        assert_eq!(l.prev_origin, [0.0; 3]);
        assert_eq!(l.prev_oldorigin, [0.0; 3]);
        assert!(!l.velocity_valid);
        assert_eq!(l.last_update_time, 0);
    }

    // ========== calc_beam_angles ==========

    #[test]
    fn calc_beam_angles_straight_up() {
        let dist = [0.0, 0.0, 100.0];
        let (yaw, pitch) = calc_beam_angles(&dist);
        assert_eq!(yaw, 0.0);
        assert_eq!(pitch, 90.0);
    }

    #[test]
    fn calc_beam_angles_straight_down() {
        let dist = [0.0, 0.0, -100.0];
        let (yaw, pitch) = calc_beam_angles(&dist);
        assert_eq!(yaw, 0.0);
        assert_eq!(pitch, 270.0);
    }

    #[test]
    fn calc_beam_angles_positive_x() {
        let dist = [100.0, 0.0, 0.0];
        let (yaw, pitch) = calc_beam_angles(&dist);
        // atan2(0, 100) = 0 degrees
        assert!((yaw - 0.0).abs() < 0.1);
        // atan2(0, 100) * -RAD_TO_DEG = 0, but formula uses -atan2
        assert!(pitch.abs() < 0.1 || (pitch - 360.0).abs() < 0.1);
    }

    #[test]
    fn calc_beam_angles_positive_y() {
        let dist = [0.0, 100.0, 0.0];
        let (yaw, pitch) = calc_beam_angles(&dist);
        // x == 0, y > 0 -> yaw = 90
        assert!((yaw - 90.0).abs() < 0.1);
    }

    #[test]
    fn calc_beam_angles_negative_y() {
        let dist = [0.0, -100.0, 0.0];
        let (yaw, pitch) = calc_beam_angles(&dist);
        // x == 0, y < 0 -> yaw = 270
        assert!((yaw - 270.0).abs() < 0.1);
    }

    #[test]
    fn calc_beam_angles_diagonal_xy() {
        let dist = [100.0, 100.0, 0.0];
        let (yaw, _pitch) = calc_beam_angles(&dist);
        // atan2(100, 100) = 45 degrees
        assert!((yaw - 45.0).abs() < 0.1);
    }

    // ========== cl_alloc_explosion ==========

    #[test]
    fn cl_alloc_explosion_uses_free_slot() {
        let mut ts = TEntState::default();
        let idx = cl_alloc_explosion(&mut ts, 1000);
        assert!(idx < MAX_EXPLOSIONS);
        assert_eq!(ts.cl_explosions[idx].exp_type, ExpType::Free);
    }

    #[test]
    fn cl_alloc_explosion_returns_different_slots() {
        let mut ts = TEntState::default();
        let idx1 = cl_alloc_explosion(&mut ts, 1000);
        ts.cl_explosions[idx1].exp_type = ExpType::Explosion;
        let idx2 = cl_alloc_explosion(&mut ts, 1000);
        assert_ne!(idx1, idx2);
    }

    #[test]
    fn cl_alloc_explosion_reuses_oldest_when_full() {
        let mut ts = TEntState::default();
        // Fill all slots
        for i in 0..MAX_EXPLOSIONS {
            ts.cl_explosions[i].exp_type = ExpType::Explosion;
            ts.cl_explosions[i].start = (1000 + i * 100) as f32;
        }
        // Oldest is index 0 (start = 1000)
        let idx = cl_alloc_explosion(&mut ts, 50000);
        assert_eq!(idx, 0);
        // The slot should be reset
        assert_eq!(ts.cl_explosions[idx].exp_type, ExpType::Free);
    }

    #[test]
    fn cl_alloc_explosion_reuses_oldest_specific() {
        let mut ts = TEntState::default();
        for i in 0..MAX_EXPLOSIONS {
            ts.cl_explosions[i].exp_type = ExpType::Explosion;
            ts.cl_explosions[i].start = (2000 + i * 100) as f32;
        }
        // Make slot 5 have the earliest start
        ts.cl_explosions[5].start = 500.0;
        let idx = cl_alloc_explosion(&mut ts, 50000);
        assert_eq!(idx, 5);
    }

    // ========== TEntState sustain extension ==========

    #[test]
    fn tent_extend_sustains_for_packet_loss() {
        let mut ts = TEntState::default();
        ts.cl_sustains[0].id = 1;
        ts.cl_sustains[0].endtime = 1100; // about to expire at cl_time 1000
        ts.cl_sustains[0].extended = false;
        ts.cl_sustains[0].original_endtime = 0;

        ts.cl_extend_sustains_for_packet_loss(1000, 500);

        assert!(ts.cl_sustains[0].extended);
        assert_eq!(ts.cl_sustains[0].endtime, 1500); // cl_time + extension
        assert_eq!(ts.cl_sustains[0].original_endtime, 1100); // saved original
    }

    #[test]
    fn tent_extend_sustains_skips_already_extended() {
        let mut ts = TEntState::default();
        ts.cl_sustains[0].id = 1;
        ts.cl_sustains[0].endtime = 1100;
        ts.cl_sustains[0].extended = true; // already extended
        ts.cl_sustains[0].original_endtime = 900;

        ts.cl_extend_sustains_for_packet_loss(1000, 500);

        // Should not be re-extended
        assert_eq!(ts.cl_sustains[0].endtime, 1100); // unchanged
    }

    #[test]
    fn tent_extend_sustains_skips_far_future() {
        let mut ts = TEntState::default();
        ts.cl_sustains[0].id = 1;
        ts.cl_sustains[0].endtime = 5000; // far in the future, not about to expire

        ts.cl_extend_sustains_for_packet_loss(1000, 500);

        // Not about to expire (5000 >= 1000 + 200), should not be extended
        assert!(!ts.cl_sustains[0].extended);
        assert_eq!(ts.cl_sustains[0].endtime, 5000);
    }

    #[test]
    fn tent_extend_sustains_skips_inactive() {
        let mut ts = TEntState::default();
        ts.cl_sustains[0].id = 0; // inactive
        ts.cl_sustains[0].endtime = 1100;

        ts.cl_extend_sustains_for_packet_loss(1000, 500);

        assert!(!ts.cl_sustains[0].extended);
    }

    #[test]
    fn tent_reset_extended_sustains() {
        let mut ts = TEntState::default();
        ts.cl_sustains[0].id = 1;
        ts.cl_sustains[0].extended = true;
        ts.cl_sustains[0].endtime = 1500;
        ts.cl_sustains[0].original_endtime = 1100;

        ts.cl_sustains[1].id = 2;
        ts.cl_sustains[1].extended = true;

        ts.cl_reset_extended_sustains();

        assert!(!ts.cl_sustains[0].extended);
        assert!(!ts.cl_sustains[1].extended);
        // endtime is NOT restored (comment in code says let it run)
        assert_eq!(ts.cl_sustains[0].endtime, 1500);
    }

    // ========== cl_clear_tents ==========

    #[test]
    fn cl_clear_tents_clears_all() {
        let mut ts = TEntState::default();
        ts.cl_beams[0].entity = 5;
        ts.cl_beams[0].model = 1;
        ts.cl_explosions[0].exp_type = ExpType::Explosion;
        ts.cl_lasers[0].endtime = 9999;
        ts.cl_playerbeams[0].entity = 3;
        ts.cl_sustains[0].id = 42;

        cl_clear_tents(&mut ts);

        assert_eq!(ts.cl_beams[0].entity, 0);
        assert_eq!(ts.cl_beams[0].model, 0);
        assert_eq!(ts.cl_explosions[0].exp_type, ExpType::Free);
        assert_eq!(ts.cl_lasers[0].endtime, 0);
        assert_eq!(ts.cl_playerbeams[0].entity, 0);
        assert_eq!(ts.cl_sustains[0].id, 0);
    }

    // ========== ExpType enum ==========

    #[test]
    fn exp_type_values() {
        assert_eq!(ExpType::Free as i32, 0);
        assert_eq!(ExpType::Explosion as i32, 1);
        assert_eq!(ExpType::Misc as i32, 2);
        assert_eq!(ExpType::Flash as i32, 3);
        assert_eq!(ExpType::MFlash as i32, 4);
        assert_eq!(ExpType::Poly as i32, 5);
        assert_eq!(ExpType::Poly2 as i32, 6);
    }

    // ========== Explosion default ==========

    #[test]
    fn explosion_default() {
        let ex = Explosion::default();
        assert_eq!(ex.exp_type, ExpType::Free);
        assert_eq!(ex.frames, 0);
        assert_eq!(ex.light, 0.0);
        assert_eq!(ex.lightcolor, [0.0; 3]);
        assert_eq!(ex.start, 0.0);
        assert_eq!(ex.baseframe, 0);
    }

    // ========== TEntState default ==========

    #[test]
    fn tent_state_default_sizes() {
        let ts = TEntState::default();
        assert_eq!(ts.cl_explosions.len(), MAX_EXPLOSIONS);
        assert_eq!(ts.cl_beams.len(), MAX_BEAMS);
        assert_eq!(ts.cl_playerbeams.len(), MAX_BEAMS);
        assert_eq!(ts.cl_lasers.len(), MAX_LASERS);
        assert_eq!(ts.cl_sustains.len(), MAX_SUSTAINS);
    }

    #[test]
    fn tent_state_default_sound_handles_zero() {
        let ts = TEntState::default();
        assert_eq!(ts.cl_sfx_ric1, 0);
        assert_eq!(ts.cl_sfx_rockexp, 0);
        assert_eq!(ts.cl_sfx_footsteps, [0; 4]);
    }

    #[test]
    fn tent_state_default_model_handles_zero() {
        let ts = TEntState::default();
        assert_eq!(ts.cl_mod_explode, 0);
        assert_eq!(ts.cl_mod_explo4, 0);
        assert_eq!(ts.cl_mod_lightning, 0);
        assert_eq!(ts.cl_mod_heatbeam, 0);
    }

    // ========== Constants ==========

    #[test]
    fn tent_constants() {
        assert_eq!(MAX_EXPLOSIONS, 32);
        assert_eq!(MAX_BEAMS, 32);
        assert_eq!(MAX_LASERS, 32);
    }

    // ========== Beam 3D velocity ==========

    #[test]
    fn beam_update_endpoint_3d_velocity() {
        let mut b = Beam::default();
        b.end = [100.0, 200.0, 300.0];
        b.last_update_time = 1000;

        b.update_endpoint(&[200.0, 400.0, 600.0], 1100);
        assert!(b.velocity_valid);
        // x: (200-100)/0.1 = 1000, y: (400-200)/0.1 = 2000, z: (600-300)/0.1 = 3000
        assert!((b.end_velocity[0] - 1000.0).abs() < 1.0);
        assert!((b.end_velocity[1] - 2000.0).abs() < 1.0);
        assert!((b.end_velocity[2] - 3000.0).abs() < 1.0);
    }

    #[test]
    fn beam_get_extrapolated_end_3d() {
        let b = Beam {
            end: [100.0, 200.0, 300.0],
            end_velocity: [1000.0, 2000.0, 3000.0],
            velocity_valid: true,
            last_update_time: 1000,
            ..Beam::default()
        };
        let result = b.get_extrapolated_end(1050, 300); // 50ms
        assert!((result[0] - 150.0).abs() < 0.1);
        assert!((result[1] - 300.0).abs() < 0.1);
        assert!((result[2] - 450.0).abs() < 0.1);
    }

    // ========== Laser 3D velocity ==========

    #[test]
    fn laser_update_endpoints_3d() {
        let mut l = Laser::default();
        l.ent.origin = [100.0, 200.0, 300.0];
        l.ent.oldorigin = [400.0, 500.0, 600.0];
        l.last_update_time = 1000;

        l.update_endpoints(
            &[200.0, 400.0, 600.0], // new origin
            &[500.0, 700.0, 900.0], // new end
            1100,
        );
        assert!(l.velocity_valid);
        // origin vel: (200-100)/0.1=1000, (400-200)/0.1=2000, (600-300)/0.1=3000
        assert!((l.origin_velocity[0] - 1000.0).abs() < 1.0);
        assert!((l.origin_velocity[1] - 2000.0).abs() < 1.0);
        // end vel: (500-400)/0.1=1000, (700-500)/0.1=2000, (900-600)/0.1=3000
        assert!((l.end_velocity[0] - 1000.0).abs() < 1.0);
        assert!((l.end_velocity[1] - 2000.0).abs() < 1.0);
    }

    // ========== SPLASH_COLOR table ==========

    #[test]
    fn splash_color_table() {
        assert_eq!(SPLASH_COLOR.len(), 7);
        assert_eq!(SPLASH_COLOR[0], 0x00);
        assert_eq!(SPLASH_COLOR[1], 0xe0);
        assert_eq!(SPLASH_COLOR[2], 0xb0);
        assert_eq!(SPLASH_COLOR[3], 0x50);
        assert_eq!(SPLASH_COLOR[4], 0xd0);
        assert_eq!(SPLASH_COLOR[5], 0xe0);
        assert_eq!(SPLASH_COLOR[6], 0xe8);
    }
}
