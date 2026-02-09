// entity_adapters.rs — Centralized Edict adapter wrappers
//
// Many monster and game files need the same set of adapter functions that
// bridge between the entity-reference API used by game logic and the
// entity-index API used by game_import and context-based subsystems.
//
// Instead of duplicating these ~10 functions in every monster file, they
// are defined once here. Monster files can import them with:
//   use crate::entity_adapters::*;

#![allow(dead_code)]

use crate::g_local::{Edict, GClient};

// ============================================================
// Re-exports: functions whose canonical versions already have
// the right signature and can be used directly.
// ============================================================

// gi_soundindex and gi_modelindex are pure pass-throughs to game_import.
pub use crate::game_import::gi_soundindex;
pub use crate::game_import::gi_modelindex;

// visible, infront, range already take &Edict in g_ai.
pub use crate::g_ai::{visible, infront, range};

// ============================================================
// Edict → entity-index adapters for game_import functions
// ============================================================

/// Play a sound on an entity.  Adapts &Edict → entity number.
pub fn gi_sound(ent: &Edict, channel: i32, sound_index: i32, volume: f32, attenuation: f32, time_ofs: f32) {
    crate::game_import::gi_sound(ent.s.number, channel, sound_index, volume, attenuation, time_ofs);
}

/// Link an entity into the world.  Adapts &Edict → entity number.
pub fn gi_linkentity(ent: &Edict) {
    crate::game_import::gi_linkentity(ent.s.number);
}

// ============================================================
// Context-wrapped adapters
// ============================================================

/// Free an entity via the full game context.
pub fn g_free_edict(self_ent: &mut Edict) {
    let idx = self_ent.s.number as usize;
    crate::g_local::with_global_game_ctx(|ctx| {
        crate::g_utils::g_free_edict(ctx, idx);
    });
}

/// Start a walk-type monster via the game context.
pub fn walkmonster_start(self_ent: &mut Edict) {
    let self_idx = self_ent.s.number as i32;
    crate::g_local::with_global_game_ctx(|ctx| {
        crate::g_monster::walkmonster_start(ctx, self_idx);
    });
}

/// Start a fly-type monster via the game context.
pub fn flymonster_start(self_ent: &mut Edict) {
    let self_idx = self_ent.s.number as i32;
    crate::g_local::with_global_game_ctx(|ctx| {
        crate::g_monster::flymonster_start(ctx, self_idx);
    });
}

/// Start a swim-type monster via the game context.
pub fn swimmonster_start(self_ent: &mut Edict) {
    let self_idx = self_ent.s.number as i32;
    crate::g_local::with_global_game_ctx(|ctx| {
        crate::g_monster::swimmonster_start(ctx, self_idx);
    });
}

/// Throw a gib model from an entity.
pub fn throw_gib(self_ent: &mut Edict, model: &str, damage: i32, gib_type: i32) {
    let self_idx = self_ent.s.number as usize;
    let model = model.to_string();
    crate::g_local::with_global_game_ctx(|ctx| {
        crate::g_misc::throw_gib(ctx, self_idx, &model, damage, gib_type);
    });
}

/// Replace entity model with a head gib.
pub fn throw_head(self_ent: &mut Edict, model: &str, damage: i32, gib_type: i32) {
    let self_idx = self_ent.s.number as usize;
    let model = model.to_string();
    crate::g_local::with_global_game_ctx(|ctx| {
        crate::g_misc::throw_head(ctx, self_idx, &model, damage, gib_type);
    });
}

/// Call the spawn function for an entity (re-initialize it via classname lookup).
pub fn ed_call_spawn(ent: &mut Edict) {
    let ent_idx = ent.s.number as usize;
    crate::g_local::with_global_game_ctx(|ctx| {
        crate::g_spawn::ed_call_spawn(ctx, ent_idx);
    });
}

// ============================================================
// Utility adapters
// ============================================================

/// Look up the muzzle flash offset for a given flash index.
pub fn monster_flash_offset(index: i32) -> [f32; 3] {
    let idx = index as usize;
    if idx < crate::m_flash::MONSTER_FLASH_OFFSET.len() {
        crate::m_flash::MONSTER_FLASH_OFFSET[idx]
    } else {
        [0.0, 0.0, 0.0]
    }
}

/// Project a weapon source point from entity origin + offset, writing result to `out`.
pub fn g_project_source(origin: &[f32; 3], offset: &[f32; 3], forward: &[f32; 3], right: &[f32; 3], out: &mut [f32; 3]) {
    *out = crate::g_utils::g_project_source(origin, offset, forward, right);
}

// ============================================================
// Monster fire adapters
// ============================================================

/// Fire a melee hit.  Adapts &mut Edict → entity index + game context.
pub fn fire_hit(self_ent: &mut Edict, aim: &[f32; 3], damage: i32, kick: i32) -> bool {
    let self_idx = self_ent.s.number as usize;
    let mut result = false;
    crate::g_local::with_global_game_ctx(|ctx| {
        result = crate::g_weapon::fire_hit(self_idx, &mut ctx.edicts, &mut ctx.level, aim, damage, kick);
    });
    result
}

/// Fire bullets.  Adapts &Edict → entity number.
pub fn monster_fire_bullet(self_ent: &Edict, start: [f32; 3], dir: [f32; 3], damage: i32, kick: i32, hspread: i32, vspread: i32, flash_number: i32) {
    crate::g_monster::monster_fire_bullet_raw(self_ent.s.number, start, dir, damage, kick, hspread, vspread, flash_number);
}

/// Fire a shotgun blast.  Adapts &Edict → entity number.
pub fn monster_fire_shotgun(self_ent: &Edict, start: [f32; 3], dir: [f32; 3], damage: i32, kick: i32, hspread: i32, vspread: i32, count: i32, flash_number: i32) {
    crate::g_monster::monster_fire_shotgun_raw(self_ent.s.number, start, dir, damage, kick, hspread, vspread, count, flash_number);
}

/// Fire a blaster bolt.  Adapts &Edict → entity number.
pub fn monster_fire_blaster(self_ent: &Edict, start: [f32; 3], dir: [f32; 3], damage: i32, speed: i32, flash_number: i32, effect: i32) {
    crate::g_monster::monster_fire_blaster_raw(self_ent.s.number, start, dir, damage, speed, flash_number, effect);
}

/// Fire a rocket.  Adapts &Edict → entity number.
pub fn monster_fire_rocket(self_ent: &Edict, start: [f32; 3], dir: [f32; 3], damage: i32, speed: i32, flash_number: i32) {
    crate::g_monster::monster_fire_rocket_raw(self_ent.s.number, start, dir, damage, speed, flash_number);
}

/// Fire a grenade.  Adapts &Edict → entity number.
pub fn monster_fire_grenade(self_ent: &Edict, start: [f32; 3], dir: [f32; 3], damage: i32, speed: i32, flash_number: i32) {
    crate::g_monster::monster_fire_grenade_raw(self_ent.s.number, start, dir, damage, speed, flash_number);
}

/// Fire a BFG blast.  Adapts &Edict → entity number.
pub fn monster_fire_bfg(self_ent: &Edict, start: [f32; 3], dir: [f32; 3], damage: i32, speed: i32, kick: i32, damage_radius: f32, flashtype: i32) {
    crate::g_monster::monster_fire_bfg_raw(self_ent.s.number, start, dir, damage, speed, kick, damage_radius, flashtype);
}

/// Fire a railgun slug.  Adapts &Edict → entity number.
pub fn monster_fire_railgun(self_ent: &Edict, start: [f32; 3], dir: [f32; 3], damage: i32, kick: i32, flashtype: i32) {
    crate::g_monster::monster_fire_railgun_raw(self_ent.s.number, start, dir, damage, kick, flashtype);
}

// ============================================================
// Movement adapters
// ============================================================

/// M_walkmove — bridge to `m_move::m_walkmove` by temporarily moving
/// `edicts` and `clients` into a `MoveContext`.
///
/// Both `GameContext` and `AiContext` hold `edicts`/`clients` directly,
/// so callers pass those fields by mutable reference.
pub fn m_walkmove(
    edicts: &mut Vec<Edict>,
    clients: &mut Vec<GClient>,
    ent_idx: i32,
    yaw: f32,
    dist: f32,
) -> bool {
    let mut move_ctx = crate::m_move::MoveContext {
        edicts: std::mem::take(edicts),
        clients: std::mem::take(clients),
        c_yes: 0,
        c_no: 0,
    };
    let result = crate::m_move::m_walkmove(&mut move_ctx, ent_idx, yaw, dist);
    *edicts = move_ctx.edicts;
    *clients = move_ctx.clients;
    result
}
