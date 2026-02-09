//! Game import interface — functions provided by the engine to the game module.
//! This mirrors the C `game_import_t` function pointer table from game.h.
//!
//! In C, `gi` is a global `game_import_t` struct. We mirror this with a global
//! static that is set once at game init time via `set_gi()`.

use std::sync::OnceLock;
use myq2_common::q_shared::{Vec3, Trace};

/// Global game import interface, mirrors C `game_import_t gi;` in g_main.c.
static GI: OnceLock<Box<dyn GameImport + Send + Sync>> = OnceLock::new();

/// Set the global game import interface. Called once during game init.
pub fn set_gi(gi: Box<dyn GameImport + Send + Sync>) {
    let _ = GI.set(gi);
}

/// Get a reference to the global game import interface.
fn gi() -> &'static dyn GameImport {
    GI.get().expect("GameImport not initialized").as_ref()
}

// ---- Free functions mirroring C `gi.xxx(...)` calls ----

pub fn gi_bprintf(printlevel: i32, msg: &str) { gi().bprintf(printlevel, msg); }
pub fn gi_dprintf(msg: &str) { gi().dprintf(msg); }
pub fn gi_cprintf(ent_idx: i32, printlevel: i32, msg: &str) { gi().cprintf(ent_idx, printlevel, msg); }
pub fn gi_centerprintf(ent_idx: i32, msg: &str) { gi().centerprintf(ent_idx, msg); }
pub fn gi_sound(ent_idx: i32, channel: i32, soundindex: i32, volume: f32, attenuation: f32, timeofs: f32) {
    gi().sound(ent_idx, channel, soundindex, volume, attenuation, timeofs);
}
pub fn gi_positioned_sound(origin: &Vec3, ent_idx: i32, channel: i32, soundindex: i32, volume: f32, attenuation: f32, timeofs: f32) {
    gi().positioned_sound(origin, ent_idx, channel, soundindex, volume, attenuation, timeofs);
}
pub fn gi_configstring(num: i32, string: &str) { gi().configstring(num, string); }
pub fn gi_error(msg: &str) { gi().error(msg); }
pub fn gi_modelindex(name: &str) -> i32 { gi().modelindex(name) }
pub fn gi_soundindex(name: &str) -> i32 { gi().soundindex(name) }
pub fn gi_imageindex(name: &str) -> i32 { gi().imageindex(name) }
pub fn gi_setmodel(ent_idx: i32, name: &str) { gi().setmodel(ent_idx, name); }
pub fn gi_trace(start: &Vec3, mins: &Vec3, maxs: &Vec3, end: &Vec3, passent: i32, contentmask: i32) -> Trace {
    gi().trace(start, mins, maxs, end, passent, contentmask)
}

/// Lag-compensated trace for fair hit detection on high-ping clients.
/// Rewinds entity positions based on client ping before performing the trace.
///
/// # Arguments
/// * `start` - Start position of trace
/// * `mins` - Bounding box mins (use [0;3] for point trace)
/// * `maxs` - Bounding box maxs (use [0;3] for point trace)
/// * `end` - End position of trace
/// * `passent` - Entity index to ignore (typically the shooter)
/// * `contentmask` - Content flags to test against
/// * `attacker_idx` - Entity index of the attacking client (for ping lookup)
///
/// # Returns
/// Trace result with hit information
pub fn gi_lag_compensated_trace(
    start: &Vec3,
    mins: &Vec3,
    maxs: &Vec3,
    end: &Vec3,
    passent: i32,
    contentmask: i32,
    attacker_idx: i32,
) -> Trace {
    gi().lag_compensated_trace(start, mins, maxs, end, passent, contentmask, attacker_idx)
}
pub fn gi_pointcontents(point: &Vec3) -> i32 { gi().pointcontents(point) }
pub fn gi_in_pvs(p1: &Vec3, p2: &Vec3) -> bool { gi().in_pvs(p1, p2) }
pub fn gi_in_phs(p1: &Vec3, p2: &Vec3) -> bool { gi().in_phs(p1, p2) }
pub fn gi_set_area_portal_state(portalnum: i32, open: bool) { gi().set_area_portal_state(portalnum, open); }
pub fn gi_areas_connected(area1: i32, area2: i32) -> bool { gi().areas_connected(area1, area2) }
pub fn gi_linkentity(ent_idx: i32) { gi().linkentity(ent_idx); }
pub fn gi_unlinkentity(ent_idx: i32) { gi().unlinkentity(ent_idx); }
pub fn gi_box_edicts(mins: &Vec3, maxs: &Vec3, maxcount: i32, areatype: i32) -> Vec<i32> {
    gi().box_edicts(mins, maxs, maxcount, areatype)
}
pub fn gi_multicast(origin: &Vec3, to: i32) { gi().multicast(origin, to); }
pub fn gi_unicast(ent_idx: i32, reliable: bool) { gi().unicast(ent_idx, reliable); }
pub fn gi_write_char(c: i32) { gi().write_char(c); }
pub fn gi_write_byte(c: i32) { gi().write_byte(c); }
pub fn gi_write_short(c: i32) { gi().write_short(c); }
pub fn gi_write_long(c: i32) { gi().write_long(c); }
pub fn gi_write_float(f: f32) { gi().write_float(f); }
pub fn gi_write_string(s: &str) { gi().write_string(s); }
pub fn gi_write_position(pos: &Vec3) { gi().write_position(pos); }
pub fn gi_write_dir(dir: &Vec3) { gi().write_dir(dir); }
pub fn gi_write_angle(f: f32) { gi().write_angle(f); }
pub fn gi_tag_malloc(size: i32, tag: i32) -> Vec<u8> { gi().tag_malloc(size, tag) }
pub fn gi_tag_free(tag: i32) { gi().tag_free(tag); }
pub fn gi_free_tags(tag: i32) { gi().free_tags(tag); }
pub fn gi_cvar(var_name: &str, value: &str, flags: i32) -> f32 { gi().cvar(var_name, value, flags) }
pub fn gi_cvar_set(var_name: &str, value: &str) { gi().cvar_set(var_name, value); }
/// Returns the current skill level (0-3). Shared helper to avoid per-monster duplication.
pub fn skill_value() -> f32 { gi_cvar("skill", "1", 0) }
/// Returns the current deathmatch value. Shared helper to avoid per-monster duplication.
pub fn deathmatch_value() -> f32 { gi_cvar("deathmatch", "0", 0) }
pub fn gi_cvar_forceset(var_name: &str, value: &str) { gi().cvar_forceset(var_name, value); }
pub fn gi_argc() -> i32 { gi().argc() }
pub fn gi_argv(n: i32) -> String { gi().argv(n) }
pub fn gi_args() -> String { gi().args() }
pub fn gi_add_command_string(text: &str) { gi().add_command_string(text); }
pub fn gi_debug_graph(value: f32, color: i32) { gi().debug_graph(value, color); }

/// Game import interface — functions provided by the engine to the game module.
pub trait GameImport {
    // Printing
    fn bprintf(&self, printlevel: i32, msg: &str);
    fn dprintf(&self, msg: &str);
    fn cprintf(&self, ent_idx: i32, printlevel: i32, msg: &str);
    fn centerprintf(&self, ent_idx: i32, msg: &str);

    // Sound
    fn sound(&self, ent_idx: i32, channel: i32, soundindex: i32, volume: f32, attenuation: f32, timeofs: f32);
    fn positioned_sound(&self, origin: &Vec3, ent_idx: i32, channel: i32, soundindex: i32, volume: f32, attenuation: f32, timeofs: f32);

    // Config
    fn configstring(&self, num: i32, string: &str);
    fn error(&self, msg: &str);

    // Indexing
    fn modelindex(&self, name: &str) -> i32;
    fn soundindex(&self, name: &str) -> i32;
    fn imageindex(&self, name: &str) -> i32;
    fn setmodel(&self, ent_idx: i32, name: &str);

    // Collision
    fn trace(&self, start: &Vec3, mins: &Vec3, maxs: &Vec3, end: &Vec3, passent: i32, contentmask: i32) -> Trace;
    /// Lag-compensated trace that rewinds entity positions based on attacker's ping.
    /// Falls back to regular trace if lag compensation is disabled.
    fn lag_compensated_trace(&self, start: &Vec3, mins: &Vec3, maxs: &Vec3, end: &Vec3, passent: i32, contentmask: i32, attacker_idx: i32) -> Trace {
        // Default implementation: fall back to regular trace
        self.trace(start, mins, maxs, end, passent, contentmask)
    }
    fn pointcontents(&self, point: &Vec3) -> i32;
    fn in_pvs(&self, p1: &Vec3, p2: &Vec3) -> bool;
    fn in_phs(&self, p1: &Vec3, p2: &Vec3) -> bool;
    fn set_area_portal_state(&self, portalnum: i32, open: bool);
    fn areas_connected(&self, area1: i32, area2: i32) -> bool;

    // Entity linking
    fn linkentity(&self, ent_idx: i32);
    fn unlinkentity(&self, ent_idx: i32);
    fn box_edicts(&self, mins: &Vec3, maxs: &Vec3, maxcount: i32, areatype: i32) -> Vec<i32>;

    // Network messaging
    fn multicast(&self, origin: &Vec3, to: i32);
    fn unicast(&self, ent_idx: i32, reliable: bool);
    fn write_char(&self, c: i32);
    fn write_byte(&self, c: i32);
    fn write_short(&self, c: i32);
    fn write_long(&self, c: i32);
    fn write_float(&self, f: f32);
    fn write_string(&self, s: &str);
    fn write_position(&self, pos: &Vec3);
    fn write_dir(&self, dir: &Vec3);
    fn write_angle(&self, f: f32);

    // Memory (simplified for Rust)
    fn tag_malloc(&self, size: i32, tag: i32) -> Vec<u8>;
    fn tag_free(&self, tag: i32);
    fn free_tags(&self, tag: i32);

    // Cvars
    fn cvar(&self, var_name: &str, value: &str, flags: i32) -> f32;
    fn cvar_set(&self, var_name: &str, value: &str);
    fn cvar_forceset(&self, var_name: &str, value: &str);

    // Command args
    fn argc(&self) -> i32;
    fn argv(&self, n: i32) -> String;
    fn args(&self) -> String;

    // Misc
    fn add_command_string(&self, text: &str);
    fn debug_graph(&self, value: f32, color: i32);
}

/// Stub implementation of `GameImport` that wires available methods to the
/// myq2_common engine singletons. Methods that require server state (configstring,
/// modelindex, soundindex, imageindex, setmodel, linkentity, unlinkentity,
/// box_edicts, multicast, unicast, in_pvs, in_phs) remain as stubs.
pub struct StubGameImport;

impl GameImport for StubGameImport {
    // ---- Printing: route to com_printf ----
    fn bprintf(&self, _printlevel: i32, msg: &str) {
        myq2_common::common::com_printf(msg);
    }
    fn dprintf(&self, msg: &str) {
        myq2_common::common::com_printf(msg);
    }
    fn cprintf(&self, _ent_idx: i32, _printlevel: i32, msg: &str) {
        myq2_common::common::com_printf(msg);
    }
    fn centerprintf(&self, _ent_idx: i32, msg: &str) {
        myq2_common::common::com_printf(msg);
    }

    // ---- Sound: stub (needs server state) ----
    fn sound(&self, _ent_idx: i32, _channel: i32, _soundindex: i32, _volume: f32, _attenuation: f32, _timeofs: f32) {
        // stub: needs server net
    }
    fn positioned_sound(&self, _origin: &Vec3, _ent_idx: i32, _channel: i32, _soundindex: i32, _volume: f32, _attenuation: f32, _timeofs: f32) {
        // stub: needs server net
    }

    // ---- Config: stub (needs server state) ----
    fn configstring(&self, _num: i32, _string: &str) {
        // stub: needs server state
    }

    // ---- Error: route to com_error ----
    fn error(&self, msg: &str) {
        myq2_common::common::com_error(myq2_common::qcommon::ERR_DROP, msg);
    }

    // ---- Indexing: stub (needs server state) ----
    fn modelindex(&self, _name: &str) -> i32 { 0 }
    fn soundindex(&self, _name: &str) -> i32 { 0 }
    fn imageindex(&self, _name: &str) -> i32 { 0 }
    fn setmodel(&self, _ent_idx: i32, _name: &str) {}

    // ---- Collision ----
    fn trace(&self, start: &Vec3, mins: &Vec3, maxs: &Vec3, end: &Vec3, _passent: i32, contentmask: i32) -> Trace {
        myq2_common::cmodel::with_cmodel_ctx(|ctx| {
            let headnode = if ctx.numcmodels > 0 {
                ctx.map_cmodels[0].headnode
            } else {
                0
            };
            ctx.box_trace(start, end, mins, maxs, headnode, contentmask)
        }).unwrap_or_default()
    }
    fn pointcontents(&self, point: &Vec3) -> i32 {
        myq2_common::cmodel::cm_point_contents(point, 0)
    }
    fn in_pvs(&self, _p1: &Vec3, _p2: &Vec3) -> bool {
        // stub: needs server PVS data
        false
    }
    fn in_phs(&self, _p1: &Vec3, _p2: &Vec3) -> bool {
        // stub: needs server PHS data
        false
    }
    fn set_area_portal_state(&self, portalnum: i32, open: bool) {
        myq2_common::cmodel::with_cmodel_ctx(|ctx| {
            ctx.set_area_portal_state(portalnum as usize, open);
        });
    }
    fn areas_connected(&self, area1: i32, area2: i32) -> bool {
        myq2_common::cmodel::with_cmodel_ctx(|ctx| {
            ctx.areas_connected(area1 as usize, area2 as usize)
        }).unwrap_or(false)
    }

    // ---- Entity linking: stub (needs server world) ----
    fn linkentity(&self, _ent_idx: i32) {}
    fn unlinkentity(&self, _ent_idx: i32) {}
    fn box_edicts(&self, _mins: &Vec3, _maxs: &Vec3, _maxcount: i32, _areatype: i32) -> Vec<i32> {
        Vec::new()
    }

    // ---- Network messaging: stub (needs server net) ----
    fn multicast(&self, _origin: &Vec3, _to: i32) {}
    fn unicast(&self, _ent_idx: i32, _reliable: bool) {}
    fn write_char(&self, _c: i32) {
        // stub: needs server message buffer
    }
    fn write_byte(&self, _c: i32) {
        // stub: needs server message buffer
    }
    fn write_short(&self, _c: i32) {
        // stub: needs server message buffer
    }
    fn write_long(&self, _c: i32) {
        // stub: needs server message buffer
    }
    fn write_float(&self, _f: f32) {
        // stub: needs server message buffer
    }
    fn write_string(&self, _s: &str) {
        // stub: needs server message buffer
    }
    fn write_position(&self, _pos: &Vec3) {
        // stub: needs server message buffer
    }
    fn write_dir(&self, _dir: &Vec3) {
        // stub: needs server message buffer
    }
    fn write_angle(&self, _f: f32) {
        // stub: needs server message buffer
    }

    // ---- Memory: Rust handles this ----
    fn tag_malloc(&self, size: i32, _tag: i32) -> Vec<u8> {
        vec![0u8; size as usize]
    }
    fn tag_free(&self, _tag: i32) {
        // no-op: Rust handles memory
    }
    fn free_tags(&self, _tag: i32) {
        // no-op: Rust handles memory
    }

    // ---- Cvars ----
    fn cvar(&self, var_name: &str, value: &str, flags: i32) -> f32 {
        myq2_common::cvar::cvar_get(var_name, value, flags);
        myq2_common::cvar::cvar_variable_value(var_name)
    }
    fn cvar_set(&self, var_name: &str, value: &str) {
        myq2_common::cvar::cvar_set(var_name, value);
    }
    fn cvar_forceset(&self, var_name: &str, value: &str) {
        myq2_common::cvar::cvar_force_set(var_name, value);
    }

    // ---- Command args ----
    fn argc(&self) -> i32 {
        myq2_common::cmd::cmd_argc() as i32
    }
    fn argv(&self, n: i32) -> String {
        myq2_common::cmd::cmd_argv(n as usize)
    }
    fn args(&self) -> String {
        myq2_common::cmd::cmd_args()
    }

    // ---- Misc ----
    fn add_command_string(&self, text: &str) {
        myq2_common::cmd::cbuf_add_text(text);
    }
    fn debug_graph(&self, _value: f32, _color: i32) {
        // no-op
    }
}
