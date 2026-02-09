// g_spawn.rs -- Entity spawning and world initialization
// Converted from: myq2-original/game/g_spawn.c
//
// Copyright (C) 1997-2001 Id Software, Inc.
// Licensed under GNU General Public License v2

use crate::g_local::*;
use crate::g_utils::{g_spawn, g_free_edict};
use crate::game::*;
use crate::game_import::*;
use myq2_common::q_shared::{CS_NAME, CS_SKY, CS_SKYROTATE, CS_SKYAXIS, CS_CDTRACK, CS_MAXCLIENTS, CS_STATUSBAR, CS_LIGHTS};
use rayon::prelude::*;
use std::collections::HashMap;
use std::sync::OnceLock;

// ============================================================
// O(1) LOOKUP TABLES - HashMap indices for FIELDS and SPAWNS
// ============================================================
// These provide O(1) lookup by name instead of O(n) linear search.
// Built lazily on first access.

/// HashMap for O(1) field lookup by name (lowercase) -> index into FIELDS
static FIELDS_INDEX: OnceLock<HashMap<&'static str, usize>> = OnceLock::new();

/// HashMap for O(1) spawn lookup by classname -> index into SPAWNS
static SPAWNS_INDEX: OnceLock<HashMap<&'static str, usize>> = OnceLock::new();

fn get_fields_index() -> &'static HashMap<&'static str, usize> {
    FIELDS_INDEX.get_or_init(|| {
        FIELDS.iter().enumerate()
            .map(|(i, f)| (f.name, i))
            .collect()
    })
}

fn get_spawns_index() -> &'static HashMap<&'static str, usize> {
    SPAWNS_INDEX.get_or_init(|| {
        SPAWNS.iter().enumerate()
            .map(|(i, s)| (s.name, i))
            .collect()
    })
}

// ============================================================
// Spawn function type and dispatch table
// ============================================================

/// Spawn function signature: takes a mutable game context and entity index.
pub type SpawnFn = fn(ctx: &mut GameContext, ent_idx: usize);

/// A spawn table entry mapping a classname to a spawn function.
pub struct SpawnEntry {
    pub name: &'static str,
    pub spawn: SpawnFn,
}

// ============================================================
// Placeholder spawn functions
// Each of these corresponds to a forward declaration in the C source.
// They will be replaced with real implementations as modules are converted.
// ============================================================

// ============================================================
// Helper macro for monster spawn functions that take
// (&mut Edict, &mut GameCtx).
// Swaps the edict out of the vec to avoid double-borrow.
// ============================================================
macro_rules! spawn_monster {
    ($ctx:expr, $ent:expr, $func:path) => {{
        let mut self_ent = std::mem::take(&mut $ctx.edicts[$ent]);
        $func(&mut self_ent, $ctx);
        $ctx.edicts[$ent] = self_ent;
    }};
}

// ============================================================
// Helper: build a GameContext for g_target spawn functions
// that take extra params (&mut level, &mut st) causing borrow conflicts
// ============================================================
fn make_target_ctx(ctx: &mut GameContext) -> GameContext {
    GameContext {
        edicts: std::mem::take(&mut ctx.edicts),
        num_edicts: ctx.num_edicts,
        maxclients: ctx.maxclients,
        max_edicts: ctx.max_edicts,
        level: ctx.level.clone(),
        ..GameContext::default()
    }
}

fn restore_from_target_ctx(ctx: &mut GameContext, mut tctx: GameContext) {
    ctx.edicts = std::mem::take(&mut tctx.edicts);
    ctx.num_edicts = tctx.num_edicts;
}

// ============================================================
// g_items spawn functions
// ============================================================
fn sp_item_health(ctx: &mut GameContext, ent: usize) {
    crate::g_items::sp_item_health(ctx, ent);
}
fn sp_item_health_small(ctx: &mut GameContext, ent: usize) {
    crate::g_items::sp_item_health_small(ctx, ent);
}
fn sp_item_health_large(ctx: &mut GameContext, ent: usize) {
    crate::g_items::sp_item_health_large(ctx, ent);
}
fn sp_item_health_mega(ctx: &mut GameContext, ent: usize) {
    crate::g_items::sp_item_health_mega(ctx, ent);
}

// ============================================================
// p_client spawn functions
// ============================================================
fn sp_info_player_start(ctx: &mut GameContext, ent: usize) {
    crate::p_client::sp_info_player_start(ctx, ent);
}
fn sp_info_player_deathmatch(ctx: &mut GameContext, ent: usize) {
    crate::p_client::sp_info_player_deathmatch(ctx, ent);
}
fn sp_info_player_coop(ctx: &mut GameContext, ent: usize) {
    crate::p_client::sp_info_player_coop(ctx, ent);
}
fn sp_info_player_intermission(_ctx: &mut GameContext, _ent: usize) {
    crate::p_client::sp_info_player_intermission();
}

// ============================================================
// g_func spawn functions (methods on GameContext = GameCtx)
// ============================================================
fn sp_func_plat(ctx: &mut GameContext, ent: usize) {
    ctx.sp_func_plat(ent);
}
fn sp_func_rotating(ctx: &mut GameContext, ent: usize) {
    ctx.sp_func_rotating(ent);
}
fn sp_func_button(ctx: &mut GameContext, ent: usize) {
    ctx.sp_func_button(ent);
}
fn sp_func_door(ctx: &mut GameContext, ent: usize) {
    ctx.sp_func_door(ent);
}
fn sp_func_door_secret(ctx: &mut GameContext, ent: usize) {
    ctx.sp_func_door_secret(ent);
}
fn sp_func_door_rotating(ctx: &mut GameContext, ent: usize) {
    ctx.sp_func_door_rotating(ent);
}
fn sp_func_water(ctx: &mut GameContext, ent: usize) {
    ctx.sp_func_water(ent);
}
fn sp_func_train(ctx: &mut GameContext, ent: usize) {
    ctx.sp_func_train(ent);
}
fn sp_func_conveyor(ctx: &mut GameContext, ent: usize) {
    ctx.sp_func_conveyor(ent);
}
fn sp_func_killbox(ctx: &mut GameContext, ent: usize) {
    ctx.sp_func_killbox(ent);
}
fn sp_func_timer(ctx: &mut GameContext, ent: usize) {
    ctx.sp_func_timer(ent);
}

// g_misc spawn functions for func_wall, func_object, func_explosive, func_areaportal, func_clock
fn sp_func_wall(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_func_wall(ctx, ent);
}
fn sp_func_object(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_func_object(ctx, ent);
}
fn sp_func_explosive(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_func_explosive(ctx, ent);
}
fn sp_func_areaportal(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_func_areaportal(ctx, ent);
}
fn sp_func_clock(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_func_clock(ctx, ent);
}

// ============================================================
// g_trigger spawn functions
// ============================================================
fn sp_trigger_always(ctx: &mut GameContext, ent: usize) {
    crate::g_trigger::sp_trigger_always(ctx, ent);
}
fn sp_trigger_once(ctx: &mut GameContext, ent: usize) {
    crate::g_trigger::sp_trigger_once(ctx, ent);
}
fn sp_trigger_multiple(ctx: &mut GameContext, ent: usize) {
    crate::g_trigger::sp_trigger_multiple(ctx, ent);
}
fn sp_trigger_relay(ctx: &mut GameContext, ent: usize) {
    crate::g_trigger::sp_trigger_relay(ctx, ent);
}
fn sp_trigger_push(ctx: &mut GameContext, ent: usize) {
    crate::g_trigger::sp_trigger_push(ctx, ent);
}
fn sp_trigger_hurt(ctx: &mut GameContext, ent: usize) {
    crate::g_trigger::sp_trigger_hurt(ctx, ent);
}
fn sp_trigger_key(ctx: &mut GameContext, ent: usize) {
    crate::g_trigger::sp_trigger_key(ctx, ent);
}
fn sp_trigger_counter(ctx: &mut GameContext, ent: usize) {
    crate::g_trigger::sp_trigger_counter(ctx, ent);
}
fn sp_trigger_elevator(ctx: &mut GameContext, ent: usize) {
    ctx.sp_trigger_elevator(ent);
}
fn sp_trigger_gravity(ctx: &mut GameContext, ent: usize) {
    crate::g_trigger::sp_trigger_gravity(ctx, ent);
}
fn sp_trigger_monsterjump(ctx: &mut GameContext, ent: usize) {
    crate::g_trigger::sp_trigger_monsterjump(ctx, ent);
}

// ============================================================
// g_target spawn functions (use GameContext)
// ============================================================
fn sp_target_temp_entity(ctx: &mut GameContext, ent: usize) {
    let mut tctx = make_target_ctx(ctx);
    crate::g_target::sp_target_temp_entity(&mut tctx, ent as i32);
    restore_from_target_ctx(ctx, tctx);
}
fn sp_target_speaker(ctx: &mut GameContext, ent: usize) {
    let st_copy = ctx.st.clone();
    let mut tctx = make_target_ctx(ctx);
    crate::g_target::sp_target_speaker(&mut tctx, ent as i32, &st_copy);
    restore_from_target_ctx(ctx, tctx);
}
fn sp_target_explosion(ctx: &mut GameContext, ent: usize) {
    let mut tctx = make_target_ctx(ctx);
    crate::g_target::sp_target_explosion(&mut tctx, ent as i32);
    restore_from_target_ctx(ctx, tctx);
}
fn sp_target_changelevel(ctx: &mut GameContext, ent: usize) {
    let level_copy = ctx.level.clone();
    let mut tctx = make_target_ctx(ctx);
    crate::g_target::sp_target_changelevel(&mut tctx, &level_copy, ent as i32);
    restore_from_target_ctx(ctx, tctx);
}
fn sp_target_secret(ctx: &mut GameContext, ent: usize) {
    let mut tctx = make_target_ctx(ctx);
    crate::g_target::sp_target_secret(&mut tctx, &mut ctx.level, ent as i32, &mut ctx.st, ctx.deathmatch);
    restore_from_target_ctx(ctx, tctx);
}
fn sp_target_goal(ctx: &mut GameContext, ent: usize) {
    let mut tctx = make_target_ctx(ctx);
    crate::g_target::sp_target_goal(&mut tctx, &mut ctx.level, ent as i32, &mut ctx.st, ctx.deathmatch);
    restore_from_target_ctx(ctx, tctx);
}
fn sp_target_splash(ctx: &mut GameContext, ent: usize) {
    let mut tctx = make_target_ctx(ctx);
    crate::g_target::sp_target_splash(&mut tctx, ent as i32);
    restore_from_target_ctx(ctx, tctx);
}
fn sp_target_spawner(ctx: &mut GameContext, ent: usize) {
    let mut tctx = make_target_ctx(ctx);
    crate::g_target::sp_target_spawner(&mut tctx, ent as i32);
    restore_from_target_ctx(ctx, tctx);
}
fn sp_target_blaster(ctx: &mut GameContext, ent: usize) {
    let mut tctx = make_target_ctx(ctx);
    crate::g_target::sp_target_blaster(&mut tctx, ent as i32);
    restore_from_target_ctx(ctx, tctx);
}
fn sp_target_crosslevel_trigger(ctx: &mut GameContext, ent: usize) {
    let mut tctx = make_target_ctx(ctx);
    crate::g_target::sp_target_crosslevel_trigger(&mut tctx, ent as i32);
    restore_from_target_ctx(ctx, tctx);
}
fn sp_target_crosslevel_target(ctx: &mut GameContext, ent: usize) {
    let level_copy = ctx.level.clone();
    let mut tctx = make_target_ctx(ctx);
    crate::g_target::sp_target_crosslevel_target(&mut tctx, &level_copy, ent as i32);
    restore_from_target_ctx(ctx, tctx);
}
fn sp_target_laser(ctx: &mut GameContext, ent: usize) {
    let level_copy = ctx.level.clone();
    let mut tctx = make_target_ctx(ctx);
    crate::g_target::sp_target_laser(&mut tctx, &level_copy, ent as i32);
    restore_from_target_ctx(ctx, tctx);
}
fn sp_target_help(ctx: &mut GameContext, ent: usize) {
    let mut tctx = make_target_ctx(ctx);
    crate::g_target::sp_target_help(&mut tctx, ent as i32, ctx.deathmatch);
    restore_from_target_ctx(ctx, tctx);
}
fn sp_target_lightramp(ctx: &mut GameContext, ent: usize) {
    let mut tctx = make_target_ctx(ctx);
    crate::g_target::sp_target_lightramp(&mut tctx, ent as i32, ctx.deathmatch);
    restore_from_target_ctx(ctx, tctx);
}
fn sp_target_earthquake(ctx: &mut GameContext, ent: usize) {
    let mut tctx = make_target_ctx(ctx);
    crate::g_target::sp_target_earthquake(&mut tctx, ent as i32);
    restore_from_target_ctx(ctx, tctx);
}

// m_actor target_actor and misc_actor
fn sp_target_actor(ctx: &mut GameContext, ent: usize) {
    crate::m_actor::sp_target_actor(&mut ctx.edicts, ent, &ctx.st);
}
fn sp_target_character(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_target_character(ctx, ent);
}
fn sp_target_string(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_target_string(ctx, ent);
}

// ============================================================
// g_misc spawn functions
// ============================================================
fn sp_viewthing(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_viewthing(ctx, ent);
}

fn sp_light(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_light(ctx, ent);
}
fn sp_light_mine1(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_light_mine1(ctx, ent);
}
fn sp_light_mine2(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_light_mine2(ctx, ent);
}
fn sp_info_null(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_info_null(ctx, ent);
}
fn sp_info_notnull(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_info_notnull(ctx, ent);
}
fn sp_path_corner(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_path_corner(ctx, ent);
}
fn sp_point_combat(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_point_combat(ctx, ent);
}

fn sp_misc_explobox(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_misc_explobox(ctx, ent);
}
fn sp_misc_banner(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_misc_banner(ctx, ent);
}
fn sp_misc_satellite_dish(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_misc_satellite_dish(ctx, ent);
}
fn sp_misc_actor(ctx: &mut GameContext, ent: usize) {
    crate::m_actor::sp_misc_actor(&mut ctx.edicts, ent, &ctx.level, ctx.deathmatch != 0.0);
}
fn sp_misc_gib_arm(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_misc_gib_arm(ctx, ent);
}
fn sp_misc_gib_leg(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_misc_gib_leg(ctx, ent);
}
fn sp_misc_gib_head(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_misc_gib_head(ctx, ent);
}
fn sp_misc_insane(ctx: &mut GameContext, ent: usize) {
    spawn_monster!(ctx, ent, crate::m_insane::sp_misc_insane);
}
fn sp_misc_deadsoldier(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_misc_deadsoldier(ctx, ent);
}
fn sp_misc_viper(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_misc_viper(ctx, ent);
}
fn sp_misc_viper_bomb(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_misc_viper_bomb(ctx, ent);
}
fn sp_misc_bigviper(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_misc_bigviper(ctx, ent);
}
fn sp_misc_strogg_ship(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_misc_strogg_ship(ctx, ent);
}
fn sp_misc_teleporter(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_misc_teleporter(ctx, ent);
}
fn sp_misc_teleporter_dest(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_misc_teleporter_dest(ctx, ent);
}
fn sp_misc_blackhole(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_misc_blackhole(ctx, ent);
}
fn sp_misc_eastertank(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_misc_eastertank(ctx, ent);
}
fn sp_misc_easterchick(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_misc_easterchick(ctx, ent);
}
fn sp_misc_easterchick2(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_misc_easterchick2(ctx, ent);
}

// ============================================================
// Monster spawn functions
// For monsters that take (&mut Edict, &mut GameContext),
// we swap the edict out of the vec, call the function, then swap it back.
// ============================================================

fn sp_monster_berserk(ctx: &mut GameContext, ent: usize) {
    spawn_monster!(ctx, ent, crate::m_berserk::sp_monster_berserk);
}
fn sp_monster_gladiator(ctx: &mut GameContext, ent: usize) {
    spawn_monster!(ctx, ent, crate::m_gladiator::sp_monster_gladiator);
}
fn sp_monster_gunner(ctx: &mut GameContext, ent: usize) {
    crate::m_gunner::sp_monster_gunner(&mut ctx.edicts[ent]);
}
fn sp_monster_infantry(ctx: &mut GameContext, ent: usize) {
    spawn_monster!(ctx, ent, crate::m_infantry::sp_monster_infantry);
}
fn sp_monster_soldier_light(ctx: &mut GameContext, ent: usize) {
    spawn_monster!(ctx, ent, crate::m_soldier::sp_monster_soldier_light);
}
fn sp_monster_soldier(ctx: &mut GameContext, ent: usize) {
    spawn_monster!(ctx, ent, crate::m_soldier::sp_monster_soldier);
}
fn sp_monster_soldier_ss(ctx: &mut GameContext, ent: usize) {
    spawn_monster!(ctx, ent, crate::m_soldier::sp_monster_soldier_ss);
}
fn sp_monster_tank(ctx: &mut GameContext, ent: usize) {
    spawn_monster!(ctx, ent, crate::m_tank::sp_monster_tank);
}
fn sp_monster_medic(ctx: &mut GameContext, ent: usize) {
    spawn_monster!(ctx, ent, crate::m_medic::sp_monster_medic);
}
fn sp_monster_flipper(ctx: &mut GameContext, ent: usize) {
    crate::m_flipper::sp_monster_flipper(&mut ctx.edicts[ent], ctx.deathmatch);
}
fn sp_monster_chick(ctx: &mut GameContext, ent: usize) {
    spawn_monster!(ctx, ent, crate::m_chick::sp_monster_chick);
}
fn sp_monster_parasite(ctx: &mut GameContext, ent: usize) {
    spawn_monster!(ctx, ent, crate::m_parasite::sp_monster_parasite);
}
fn sp_monster_flyer(ctx: &mut GameContext, ent: usize) {
    crate::m_flyer::sp_monster_flyer(&mut ctx.edicts[ent]);
}
fn sp_monster_brain(ctx: &mut GameContext, ent: usize) {
    spawn_monster!(ctx, ent, crate::m_brain::sp_monster_brain);
}
fn sp_monster_floater(ctx: &mut GameContext, ent: usize) {
    crate::m_float::sp_monster_floater(ctx, ent as i32);
}
fn sp_monster_hover(ctx: &mut GameContext, ent: usize) {
    spawn_monster!(ctx, ent, crate::m_hover::sp_monster_hover);
}
fn sp_monster_mutant(ctx: &mut GameContext, ent: usize) {
    spawn_monster!(ctx, ent, crate::m_mutant::sp_monster_mutant);
}
fn sp_monster_supertank(ctx: &mut GameContext, ent: usize) {
    spawn_monster!(ctx, ent, crate::m_supertank::sp_monster_supertank);
}
fn sp_monster_boss2(ctx: &mut GameContext, ent: usize) {
    spawn_monster!(ctx, ent, crate::m_boss2::sp_monster_boss2);
}
fn sp_monster_jorg(ctx: &mut GameContext, ent: usize) {
    spawn_monster!(ctx, ent, crate::m_boss31::sp_monster_jorg);
}
fn sp_monster_boss3_stand(ctx: &mut GameContext, ent: usize) {
    crate::m_boss3::sp_monster_boss3_stand(ctx, ent);
}
fn sp_monster_commander_body(ctx: &mut GameContext, ent: usize) {
    crate::g_misc::sp_monster_commander_body(ctx, ent);
}

// ============================================================
// g_turret spawn functions
// ============================================================
fn sp_turret_breach(ctx: &mut GameContext, ent: usize) {
    crate::g_turret::sp_turret_breach(&mut ctx.edicts, &ctx.level, &mut ctx.st, ent);
}
fn sp_turret_base(ctx: &mut GameContext, ent: usize) {
    crate::g_turret::sp_turret_base(&mut ctx.edicts, &ctx.level, ent);
}
fn sp_turret_driver(ctx: &mut GameContext, ent: usize) {
    crate::g_turret::sp_turret_driver(&mut ctx.edicts, &mut ctx.level, &ctx.st, ctx.deathmatch, ent);
}

// ============================================================
// Spawn dispatch table
// Exactly mirrors the C spawns[] array, same order.
// ============================================================

pub static SPAWNS: &[SpawnEntry] = &[
    SpawnEntry { name: "item_health", spawn: sp_item_health },
    SpawnEntry { name: "item_health_small", spawn: sp_item_health_small },
    SpawnEntry { name: "item_health_large", spawn: sp_item_health_large },
    SpawnEntry { name: "item_health_mega", spawn: sp_item_health_mega },

    SpawnEntry { name: "info_player_start", spawn: sp_info_player_start },
    SpawnEntry { name: "info_player_deathmatch", spawn: sp_info_player_deathmatch },
    SpawnEntry { name: "info_player_coop", spawn: sp_info_player_coop },
    SpawnEntry { name: "info_player_intermission", spawn: sp_info_player_intermission },

    SpawnEntry { name: "func_plat", spawn: sp_func_plat },
    SpawnEntry { name: "func_button", spawn: sp_func_button },
    SpawnEntry { name: "func_door", spawn: sp_func_door },
    SpawnEntry { name: "func_door_secret", spawn: sp_func_door_secret },
    SpawnEntry { name: "func_door_rotating", spawn: sp_func_door_rotating },
    SpawnEntry { name: "func_rotating", spawn: sp_func_rotating },
    SpawnEntry { name: "func_train", spawn: sp_func_train },
    SpawnEntry { name: "func_water", spawn: sp_func_water },
    SpawnEntry { name: "func_conveyor", spawn: sp_func_conveyor },
    SpawnEntry { name: "func_areaportal", spawn: sp_func_areaportal },
    SpawnEntry { name: "func_clock", spawn: sp_func_clock },
    SpawnEntry { name: "func_wall", spawn: sp_func_wall },
    SpawnEntry { name: "func_object", spawn: sp_func_object },
    SpawnEntry { name: "func_timer", spawn: sp_func_timer },
    SpawnEntry { name: "func_explosive", spawn: sp_func_explosive },
    SpawnEntry { name: "func_killbox", spawn: sp_func_killbox },

    SpawnEntry { name: "trigger_always", spawn: sp_trigger_always },
    SpawnEntry { name: "trigger_once", spawn: sp_trigger_once },
    SpawnEntry { name: "trigger_multiple", spawn: sp_trigger_multiple },
    SpawnEntry { name: "trigger_relay", spawn: sp_trigger_relay },
    SpawnEntry { name: "trigger_push", spawn: sp_trigger_push },
    SpawnEntry { name: "trigger_hurt", spawn: sp_trigger_hurt },
    SpawnEntry { name: "trigger_key", spawn: sp_trigger_key },
    SpawnEntry { name: "trigger_counter", spawn: sp_trigger_counter },
    SpawnEntry { name: "trigger_elevator", spawn: sp_trigger_elevator },
    SpawnEntry { name: "trigger_gravity", spawn: sp_trigger_gravity },
    SpawnEntry { name: "trigger_monsterjump", spawn: sp_trigger_monsterjump },

    SpawnEntry { name: "target_temp_entity", spawn: sp_target_temp_entity },
    SpawnEntry { name: "target_speaker", spawn: sp_target_speaker },
    SpawnEntry { name: "target_explosion", spawn: sp_target_explosion },
    SpawnEntry { name: "target_changelevel", spawn: sp_target_changelevel },
    SpawnEntry { name: "target_secret", spawn: sp_target_secret },
    SpawnEntry { name: "target_goal", spawn: sp_target_goal },
    SpawnEntry { name: "target_splash", spawn: sp_target_splash },
    SpawnEntry { name: "target_spawner", spawn: sp_target_spawner },
    SpawnEntry { name: "target_blaster", spawn: sp_target_blaster },
    SpawnEntry { name: "target_crosslevel_trigger", spawn: sp_target_crosslevel_trigger },
    SpawnEntry { name: "target_crosslevel_target", spawn: sp_target_crosslevel_target },
    SpawnEntry { name: "target_laser", spawn: sp_target_laser },
    SpawnEntry { name: "target_help", spawn: sp_target_help },
    SpawnEntry { name: "target_actor", spawn: sp_target_actor },
    SpawnEntry { name: "target_lightramp", spawn: sp_target_lightramp },
    SpawnEntry { name: "target_earthquake", spawn: sp_target_earthquake },
    SpawnEntry { name: "target_character", spawn: sp_target_character },
    SpawnEntry { name: "target_string", spawn: sp_target_string },

    SpawnEntry { name: "worldspawn", spawn: sp_worldspawn },
    SpawnEntry { name: "viewthing", spawn: sp_viewthing },

    SpawnEntry { name: "light", spawn: sp_light },
    SpawnEntry { name: "light_mine1", spawn: sp_light_mine1 },
    SpawnEntry { name: "light_mine2", spawn: sp_light_mine2 },
    SpawnEntry { name: "info_null", spawn: sp_info_null },
    SpawnEntry { name: "func_group", spawn: sp_info_null },
    SpawnEntry { name: "info_notnull", spawn: sp_info_notnull },
    SpawnEntry { name: "path_corner", spawn: sp_path_corner },
    SpawnEntry { name: "point_combat", spawn: sp_point_combat },

    SpawnEntry { name: "misc_explobox", spawn: sp_misc_explobox },
    SpawnEntry { name: "misc_banner", spawn: sp_misc_banner },
    SpawnEntry { name: "misc_satellite_dish", spawn: sp_misc_satellite_dish },
    SpawnEntry { name: "misc_actor", spawn: sp_misc_actor },
    SpawnEntry { name: "misc_gib_arm", spawn: sp_misc_gib_arm },
    SpawnEntry { name: "misc_gib_leg", spawn: sp_misc_gib_leg },
    SpawnEntry { name: "misc_gib_head", spawn: sp_misc_gib_head },
    SpawnEntry { name: "misc_insane", spawn: sp_misc_insane },
    SpawnEntry { name: "misc_deadsoldier", spawn: sp_misc_deadsoldier },
    SpawnEntry { name: "misc_viper", spawn: sp_misc_viper },
    SpawnEntry { name: "misc_viper_bomb", spawn: sp_misc_viper_bomb },
    SpawnEntry { name: "misc_bigviper", spawn: sp_misc_bigviper },
    SpawnEntry { name: "misc_strogg_ship", spawn: sp_misc_strogg_ship },
    SpawnEntry { name: "misc_teleporter", spawn: sp_misc_teleporter },
    SpawnEntry { name: "misc_teleporter_dest", spawn: sp_misc_teleporter_dest },
    SpawnEntry { name: "misc_blackhole", spawn: sp_misc_blackhole },
    SpawnEntry { name: "misc_eastertank", spawn: sp_misc_eastertank },
    SpawnEntry { name: "misc_easterchick", spawn: sp_misc_easterchick },
    SpawnEntry { name: "misc_easterchick2", spawn: sp_misc_easterchick2 },

    SpawnEntry { name: "monster_berserk", spawn: sp_monster_berserk },
    SpawnEntry { name: "monster_gladiator", spawn: sp_monster_gladiator },
    SpawnEntry { name: "monster_gunner", spawn: sp_monster_gunner },
    SpawnEntry { name: "monster_infantry", spawn: sp_monster_infantry },
    SpawnEntry { name: "monster_soldier_light", spawn: sp_monster_soldier_light },
    SpawnEntry { name: "monster_soldier", spawn: sp_monster_soldier },
    SpawnEntry { name: "monster_soldier_ss", spawn: sp_monster_soldier_ss },
    SpawnEntry { name: "monster_tank", spawn: sp_monster_tank },
    SpawnEntry { name: "monster_tank_commander", spawn: sp_monster_tank },
    SpawnEntry { name: "monster_medic", spawn: sp_monster_medic },
    SpawnEntry { name: "monster_flipper", spawn: sp_monster_flipper },
    SpawnEntry { name: "monster_chick", spawn: sp_monster_chick },
    SpawnEntry { name: "monster_parasite", spawn: sp_monster_parasite },
    SpawnEntry { name: "monster_flyer", spawn: sp_monster_flyer },
    SpawnEntry { name: "monster_brain", spawn: sp_monster_brain },
    SpawnEntry { name: "monster_floater", spawn: sp_monster_floater },
    SpawnEntry { name: "monster_hover", spawn: sp_monster_hover },
    SpawnEntry { name: "monster_mutant", spawn: sp_monster_mutant },
    SpawnEntry { name: "monster_supertank", spawn: sp_monster_supertank },
    SpawnEntry { name: "monster_boss2", spawn: sp_monster_boss2 },
    SpawnEntry { name: "monster_boss3_stand", spawn: sp_monster_boss3_stand },
    SpawnEntry { name: "monster_jorg", spawn: sp_monster_jorg },

    SpawnEntry { name: "monster_commander_body", spawn: sp_monster_commander_body },

    SpawnEntry { name: "turret_breach", spawn: sp_turret_breach },
    SpawnEntry { name: "turret_base", spawn: sp_turret_base },
    SpawnEntry { name: "turret_driver", spawn: sp_turret_driver },
];

// ============================================================
// Field type enum for ED_ParseField
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    LString,
    Vector,
    Int,
    Float,
    AngleHack,
    Ignore,
}

/// Field definition for entity key/value parsing.
pub struct FieldDef {
    pub name: &'static str,
    pub field_type: FieldType,
    pub flags: i32,
    /// Identifies which field on the edict/spawntemp this maps to.
    pub target: FieldTarget,
}

/// Identifies the destination field for ED_ParseField.
/// In C this was a byte offset; in Rust we use an enum to dispatch safely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldTarget {
    // Edict fields
    EdictClassname,
    EdictModel,
    EdictSpawnflags,
    EdictSpeed,
    EdictAccel,
    EdictDecel,
    EdictTarget,
    EdictTargetname,
    EdictPathtarget,
    EdictDeathtarget,
    EdictKilltarget,
    EdictCombattarget,
    EdictMessage,
    EdictTeam,
    EdictWait,
    EdictDelay,
    EdictRandom,
    EdictStyle,
    EdictCount,
    EdictHealth,
    EdictSounds,
    EdictLight,    // (ignored)
    EdictDmg,
    EdictMass,
    EdictVolume,
    EdictAttenuation,
    EdictMap,
    EdictOrigin,
    EdictAngles,
    EdictAngle,
    EdictItem,
    EdictGravity,
    EdictGoalentity,
    EdictMovetarget,
    EdictYawSpeed,
    EdictIdealYaw,
    EdictViewheight,
    EdictTakedamage,
    EdictDmgRadius,
    EdictRadiusDmg,
    EdictMins,
    EdictMaxs,
    EdictSolidType,
    EdictMoveType,
    EdictNoiseIndex,
    EdictNoiseIndex2,

    // SpawnTemp fields
    StLip,
    StDistance,
    StHeight,
    StNoise,
    StPausetime,
    StItem,
    StGravity,
    StSky,
    StSkyrotate,
    StSkyaxis,
    StMinyaw,
    StMaxyaw,
    StMinpitch,
    StMaxpitch,
    StNextmap,
}

/// The master field definition table, mirrors the C `fields[]` array.
/// FFL_SPAWNTEMP = 1, FFL_NOSPAWN = 2
pub static FIELDS: &[FieldDef] = &[
    FieldDef { name: "classname",    field_type: FieldType::LString, flags: 0, target: FieldTarget::EdictClassname },
    FieldDef { name: "model",        field_type: FieldType::LString, flags: 0, target: FieldTarget::EdictModel },
    FieldDef { name: "spawnflags",   field_type: FieldType::Int,     flags: 0, target: FieldTarget::EdictSpawnflags },
    FieldDef { name: "speed",        field_type: FieldType::Float,   flags: 0, target: FieldTarget::EdictSpeed },
    FieldDef { name: "accel",        field_type: FieldType::Float,   flags: 0, target: FieldTarget::EdictAccel },
    FieldDef { name: "decel",        field_type: FieldType::Float,   flags: 0, target: FieldTarget::EdictDecel },
    FieldDef { name: "target",       field_type: FieldType::LString, flags: 0, target: FieldTarget::EdictTarget },
    FieldDef { name: "targetname",   field_type: FieldType::LString, flags: 0, target: FieldTarget::EdictTargetname },
    FieldDef { name: "pathtarget",   field_type: FieldType::LString, flags: 0, target: FieldTarget::EdictPathtarget },
    FieldDef { name: "deathtarget",  field_type: FieldType::LString, flags: 0, target: FieldTarget::EdictDeathtarget },
    FieldDef { name: "killtarget",   field_type: FieldType::LString, flags: 0, target: FieldTarget::EdictKilltarget },
    FieldDef { name: "combattarget", field_type: FieldType::LString, flags: 0, target: FieldTarget::EdictCombattarget },
    FieldDef { name: "message",      field_type: FieldType::LString, flags: 0, target: FieldTarget::EdictMessage },
    FieldDef { name: "team",         field_type: FieldType::LString, flags: 0, target: FieldTarget::EdictTeam },
    FieldDef { name: "wait",         field_type: FieldType::Float,   flags: 0, target: FieldTarget::EdictWait },
    FieldDef { name: "delay",        field_type: FieldType::Float,   flags: 0, target: FieldTarget::EdictDelay },
    FieldDef { name: "random",       field_type: FieldType::Float,   flags: 0, target: FieldTarget::EdictRandom },
    FieldDef { name: "style",        field_type: FieldType::Int,     flags: 0, target: FieldTarget::EdictStyle },
    FieldDef { name: "count",        field_type: FieldType::Int,     flags: 0, target: FieldTarget::EdictCount },
    FieldDef { name: "health",       field_type: FieldType::Int,     flags: 0, target: FieldTarget::EdictHealth },
    FieldDef { name: "sounds",       field_type: FieldType::Int,     flags: 0, target: FieldTarget::EdictSounds },
    FieldDef { name: "light",        field_type: FieldType::Ignore,  flags: 0, target: FieldTarget::EdictLight },
    FieldDef { name: "dmg",          field_type: FieldType::Int,     flags: 0, target: FieldTarget::EdictDmg },
    FieldDef { name: "mass",         field_type: FieldType::Int,     flags: 0, target: FieldTarget::EdictMass },
    FieldDef { name: "volume",       field_type: FieldType::Float,   flags: 0, target: FieldTarget::EdictVolume },
    FieldDef { name: "attenuation",  field_type: FieldType::Float,   flags: 0, target: FieldTarget::EdictAttenuation },
    FieldDef { name: "map",          field_type: FieldType::LString, flags: 0, target: FieldTarget::EdictMap },
    FieldDef { name: "origin",       field_type: FieldType::Vector,  flags: 0, target: FieldTarget::EdictOrigin },
    FieldDef { name: "angles",       field_type: FieldType::Vector,  flags: 0, target: FieldTarget::EdictAngles },
    FieldDef { name: "angle",        field_type: FieldType::AngleHack, flags: 0, target: FieldTarget::EdictAngle },

    // SpawnTemp fields (FFL_SPAWNTEMP = 1)
    FieldDef { name: "lip",          field_type: FieldType::Int,     flags: FFL_SPAWNTEMP, target: FieldTarget::StLip },
    FieldDef { name: "distance",     field_type: FieldType::Int,     flags: FFL_SPAWNTEMP, target: FieldTarget::StDistance },
    FieldDef { name: "height",       field_type: FieldType::Int,     flags: FFL_SPAWNTEMP, target: FieldTarget::StHeight },
    FieldDef { name: "noise",        field_type: FieldType::LString, flags: FFL_SPAWNTEMP, target: FieldTarget::StNoise },
    FieldDef { name: "pausetime",    field_type: FieldType::Float,   flags: FFL_SPAWNTEMP, target: FieldTarget::StPausetime },
    FieldDef { name: "item",         field_type: FieldType::LString, flags: FFL_SPAWNTEMP, target: FieldTarget::StItem },
    FieldDef { name: "gravity",      field_type: FieldType::LString, flags: FFL_SPAWNTEMP, target: FieldTarget::StGravity },
    FieldDef { name: "sky",          field_type: FieldType::LString, flags: FFL_SPAWNTEMP, target: FieldTarget::StSky },
    FieldDef { name: "skyrotate",    field_type: FieldType::Float,   flags: FFL_SPAWNTEMP, target: FieldTarget::StSkyrotate },
    FieldDef { name: "skyaxis",      field_type: FieldType::Vector,  flags: FFL_SPAWNTEMP, target: FieldTarget::StSkyaxis },
    FieldDef { name: "minyaw",       field_type: FieldType::Float,   flags: FFL_SPAWNTEMP, target: FieldTarget::StMinyaw },
    FieldDef { name: "maxyaw",       field_type: FieldType::Float,   flags: FFL_SPAWNTEMP, target: FieldTarget::StMaxyaw },
    FieldDef { name: "minpitch",     field_type: FieldType::Float,   flags: FFL_SPAWNTEMP, target: FieldTarget::StMinpitch },
    FieldDef { name: "maxpitch",     field_type: FieldType::Float,   flags: FFL_SPAWNTEMP, target: FieldTarget::StMaxpitch },
    FieldDef { name: "nextmap",      field_type: FieldType::LString, flags: FFL_SPAWNTEMP, target: FieldTarget::StNextmap },
];

// ============================================================
// Status bar strings — exact replicas of the C originals
// ============================================================

pub const SINGLE_STATUSBAR: &str = concat!(
    "yb\t-24 ",

    // health
    "xv\t0 ",
    "hnum ",
    "xv\t50 ",
    "pic 0 ",

    // ammo
    "if 2 ",
    "\txv\t100 ",
    "\tanum ",
    "\txv\t150 ",
    "\tpic 2 ",
    "endif ",

    // armor
    "if 4 ",
    "\txv\t200 ",
    "\trnum ",
    "\txv\t250 ",
    "\tpic 4 ",
    "endif ",

    // selected item
    "if 6 ",
    "\txv\t296 ",
    "\tpic 6 ",
    "endif ",

    "yb\t-50 ",

    // picked up item
    "if 7 ",
    "\txv\t0 ",
    "\tpic 7 ",
    "\txv\t26 ",
    "\tyb\t-42 ",
    "\tstat_string 8 ",
    "\tyb\t-50 ",
    "endif ",

    // timer
    "if 9 ",
    "\txv\t262 ",
    "\tnum\t2\t10 ",
    "\txv\t296 ",
    "\tpic\t9 ",
    "endif ",

    // help / weapon icon
    "if 11 ",
    "\txv\t148 ",
    "\tpic\t11 ",
    "endif ",
);

pub const DM_STATUSBAR: &str = concat!(
    "yb\t-24 ",

    // health
    "xv\t0 ",
    "hnum ",
    "xv\t50 ",
    "pic 0 ",

    // ammo
    "if 2 ",
    "\txv\t100 ",
    "\tanum ",
    "\txv\t150 ",
    "\tpic 2 ",
    "endif ",

    // armor
    "if 4 ",
    "\txv\t200 ",
    "\trnum ",
    "\txv\t250 ",
    "\tpic 4 ",
    "endif ",

    // selected item
    "if 6 ",
    "\txv\t296 ",
    "\tpic 6 ",
    "endif ",

    "yb\t-50 ",

    // picked up item
    "if 7 ",
    "\txv\t0 ",
    "\tpic 7 ",
    "\txv\t26 ",
    "\tyb\t-42 ",
    "\tstat_string 8 ",
    "\tyb\t-50 ",
    "endif ",

    // timer
    "if 9 ",
    "\txv\t246 ",
    "\tnum\t2\t10 ",
    "\txv\t296 ",
    "\tpic\t9 ",
    "endif ",

    // help / weapon icon
    "if 11 ",
    "\txv\t148 ",
    "\tpic\t11 ",
    "endif ",

    // frags
    "xr\t-50 ",
    "yt 2 ",
    "num 3 14 ",

    // spectator
    "if 17 ",
    "xv 0 ",
    "yb -58 ",
    "string2 \"SPECTATOR MODE\" ",
    "endif ",

    // chase camera
    "if 16 ",
    "xv 0 ",
    "yb -68 ",
    "string \"Chasing\" ",
    "xv 64 ",
    "stat_string 16 ",
    "endif ",
);

// ============================================================
// ED_NewString
// ============================================================

/// Allocates a new string, converting '\\n' escape sequences to actual newlines.
/// Mirrors the C ED_NewString exactly.
pub fn ed_new_string(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut result = String::with_capacity(bytes.len());
    let mut i = 0;
    let l = bytes.len();

    while i < l {
        if bytes[i] == b'\\' && i < l - 1 {
            i += 1;
            if bytes[i] == b'n' {
                result.push('\n');
            } else {
                result.push('\\');
            }
        } else {
            result.push(bytes[i] as char);
        }
        i += 1;
    }

    result
}

// ============================================================
// ED_ParseField
// ============================================================

/// Sets a field on an edict (or spawn temp) from a key/value pair.
/// Mirrors the C ED_ParseField.
/// Uses O(1) HashMap lookup instead of O(n) linear search.
pub fn ed_parse_field(ctx: &mut GameContext, key: &str, value: &str, ent_idx: usize) {
    // O(1) lookup via HashMap (field names are lowercase in FIELDS)
    let key_lower = key.to_ascii_lowercase();
    let fields_index = get_fields_index();

    let f = match fields_index.get(key_lower.as_str()) {
        Some(&idx) => &FIELDS[idx],
        None => {
            gi_dprintf(&format!("{} is not a field\n", key));
            return;
        }
    };

    if (f.flags & FFL_NOSPAWN) != 0 {
        return;
    }

    let is_spawntemp = (f.flags & FFL_SPAWNTEMP) != 0;

    match f.field_type {
        FieldType::LString => {
            let s = ed_new_string(value);
            set_field_string(ctx, f.target, is_spawntemp, ent_idx, s);
        }
        FieldType::Vector => {
            let vec = parse_vec3(value);
            set_field_vec3(ctx, f.target, is_spawntemp, ent_idx, vec);
        }
        FieldType::Int => {
            let v: i32 = value.parse().unwrap_or(0);
            set_field_int(ctx, f.target, is_spawntemp, ent_idx, v);
        }
        FieldType::Float => {
            let v: f32 = value.parse().unwrap_or(0.0);
            set_field_float(ctx, f.target, is_spawntemp, ent_idx, v);
        }
        FieldType::AngleHack => {
            let v: f32 = value.parse().unwrap_or(0.0);
            // AngleHack sets angles to [0, v, 0]
            set_field_vec3(ctx, f.target, is_spawntemp, ent_idx, [0.0, v, 0.0]);
        }
        FieldType::Ignore => {}
    }
}

/// Parse a space-separated "x y z" string into [f32; 3].
fn parse_vec3(s: &str) -> [f32; 3] {
    let mut vec = [0.0f32; 3];
    let mut parts = s.split_whitespace();
    if let Some(x) = parts.next() { vec[0] = x.parse().unwrap_or(0.0); }
    if let Some(y) = parts.next() { vec[1] = y.parse().unwrap_or(0.0); }
    if let Some(z) = parts.next() { vec[2] = z.parse().unwrap_or(0.0); }
    vec
}

/// Dispatch a string value to the correct field on edict or spawntemp.
fn set_field_string(ctx: &mut GameContext, target: FieldTarget, is_spawntemp: bool, ent_idx: usize, val: String) {
    if is_spawntemp {
        match target {
            FieldTarget::StNoise    => ctx.st.noise = val,
            FieldTarget::StItem     => ctx.st.item = val,
            FieldTarget::StGravity  => ctx.st.gravity = val,
            FieldTarget::StSky      => ctx.st.sky = val,
            FieldTarget::StNextmap  => ctx.st.nextmap = val,
            _ => {}
        }
    } else {
        let ent = &mut ctx.edicts[ent_idx];
        match target {
            FieldTarget::EdictClassname    => ent.classname = val,
            FieldTarget::EdictModel        => ent.model = val,
            FieldTarget::EdictTarget       => ent.target = val,
            FieldTarget::EdictTargetname   => ent.targetname = val,
            FieldTarget::EdictPathtarget   => ent.pathtarget = val,
            FieldTarget::EdictDeathtarget  => ent.deathtarget = val,
            FieldTarget::EdictKilltarget   => ent.killtarget = val,
            FieldTarget::EdictCombattarget => ent.combattarget = val,
            FieldTarget::EdictMessage      => ent.message = val,
            FieldTarget::EdictTeam         => ent.team = val,
            FieldTarget::EdictMap          => ent.map = val,
            _ => {}
        }
    }
}

/// Dispatch an int value to the correct field.
fn set_field_int(ctx: &mut GameContext, target: FieldTarget, is_spawntemp: bool, ent_idx: usize, val: i32) {
    if is_spawntemp {
        match target {
            FieldTarget::StLip      => ctx.st.lip = val,
            FieldTarget::StDistance  => ctx.st.distance = val,
            FieldTarget::StHeight   => ctx.st.height = val,
            _ => {}
        }
    } else {
        let ent = &mut ctx.edicts[ent_idx];
        match target {
            FieldTarget::EdictSpawnflags => ent.spawnflags = val,
            FieldTarget::EdictStyle      => ent.style = val,
            FieldTarget::EdictCount      => ent.count = val,
            FieldTarget::EdictHealth     => ent.health = val,
            FieldTarget::EdictSounds     => ent.sounds = val,
            FieldTarget::EdictDmg        => ent.dmg = val,
            FieldTarget::EdictMass       => ent.mass = val,
            _ => {}
        }
    }
}

/// Dispatch a float value to the correct field.
fn set_field_float(ctx: &mut GameContext, target: FieldTarget, is_spawntemp: bool, ent_idx: usize, val: f32) {
    if is_spawntemp {
        match target {
            FieldTarget::StPausetime => ctx.st.pausetime = val,
            FieldTarget::StSkyrotate => ctx.st.skyrotate = val,
            FieldTarget::StMinyaw    => ctx.st.minyaw = val,
            FieldTarget::StMaxyaw    => ctx.st.maxyaw = val,
            FieldTarget::StMinpitch  => ctx.st.minpitch = val,
            FieldTarget::StMaxpitch  => ctx.st.maxpitch = val,
            _ => {}
        }
    } else {
        let ent = &mut ctx.edicts[ent_idx];
        match target {
            FieldTarget::EdictSpeed       => ent.speed = val,
            FieldTarget::EdictAccel       => ent.accel = val,
            FieldTarget::EdictDecel       => ent.decel = val,
            FieldTarget::EdictWait        => ent.wait = val,
            FieldTarget::EdictDelay       => ent.delay = val,
            FieldTarget::EdictRandom      => ent.random = val,
            FieldTarget::EdictVolume      => ent.volume = val,
            FieldTarget::EdictAttenuation => ent.attenuation = val,
            _ => {}
        }
    }
}

/// Dispatch a vec3 value to the correct field.
fn set_field_vec3(ctx: &mut GameContext, target: FieldTarget, is_spawntemp: bool, ent_idx: usize, val: [f32; 3]) {
    if is_spawntemp {
        if target == FieldTarget::StSkyaxis { ctx.st.skyaxis = val }
    } else {
        let ent = &mut ctx.edicts[ent_idx];
        match target {
            FieldTarget::EdictOrigin => ent.s.origin = val,
            FieldTarget::EdictAngles | FieldTarget::EdictAngle => ent.s.angles = val,
            _ => {}
        }
    }
}

// ============================================================
// ED_ParseEdict
// ============================================================

/// Parses an edict out of the given entity string, returning remaining data.
/// `ent_idx` should be a properly initialized empty edict.
/// Mirrors the C ED_ParseEdict.
pub fn ed_parse_edict(ctx: &mut GameContext, data: &str, ent_idx: usize) -> Option<String> {
    let mut init = false;
    ctx.st = SpawnTemp::default();

    let mut remaining = data;

    // go through all the dictionary pairs
    loop {
        // parse key
        let (com_token, rest) = com_parse(remaining);
        if com_token == "}" {
            break;
        }
        let rest = rest.unwrap_or_else(|| {
            panic!("ED_ParseEntity: EOF without closing brace");
        });

        let keyname = com_token;

        // parse value
        let (com_token, rest) = com_parse(rest);
        let rest = match rest {
            Some(r) => r,
            None => {
                if com_token == "}" {
                    panic!("ED_ParseEntity: closing brace without data");
                }
                // EOF without closing brace
                panic!("ED_ParseEntity: EOF without closing brace");
            }
        };

        if com_token == "}" {
            panic!("ED_ParseEntity: closing brace without data");
        }

        init = true;

        // keynames with a leading underscore are utility comments, discard
        if keyname.starts_with('_') {
            remaining = rest;
            continue;
        }

        ed_parse_field(ctx, &keyname, &com_token, ent_idx);
        remaining = rest;
    }

    if !init {
        ctx.edicts[ent_idx] = Edict::default();
    }

    Some(remaining.to_string())
}

// ============================================================
// ED_CallSpawn
// ============================================================

/// Finds the spawn function for the entity and calls it.
/// Mirrors the C ED_CallSpawn.
pub fn ed_call_spawn(ctx: &mut GameContext, ent_idx: usize) {
    let classname = ctx.edicts[ent_idx].classname.clone();

    if classname.is_empty() {
        gi_dprintf("ED_CallSpawn: NULL classname\n");
        return;
    }

    // check item spawn functions
    for i in 0..ctx.game.num_items as usize {
        if i >= ctx.items.len() {
            break;
        }
        if ctx.items[i].classname.is_empty() {
            continue;
        }
        if ctx.items[i].classname == classname {
            // found it — call SpawnItem
            // SpawnItem deferred: requires full item spawn integration
            gi_dprintf(&format!("SpawnItem(ent={}, item={}) deferred\n", ent_idx, i));
            return;
        }
    }

    // check normal spawn functions - O(1) HashMap lookup
    let spawns_index = get_spawns_index();
    if let Some(&idx) = spawns_index.get(classname.as_str()) {
        (SPAWNS[idx].spawn)(ctx, ent_idx);
        return;
    }

    gi_dprintf(&format!("{} doesn't have a spawn function\n", classname));
}


// ============================================================
// G_FindTeams
// ============================================================

/// Chain together all entities with a matching team field.
/// All but the first will have the FL_TEAMSLAVE flag set.
/// All but the last will have the teamchain field set to the next one.
/// Mirrors the C G_FindTeams.
///
/// OPTIMIZATION: Uses O(n) HashMap grouping instead of O(n²) nested loops.
/// First pass groups entities by team name, second pass chains each group.
pub fn g_find_teams(ctx: &mut GameContext) {
    let num_edicts = ctx.num_edicts as usize;

    // Phase 1: Group entity indices by team name - O(n)
    let mut team_groups: HashMap<String, Vec<usize>> = HashMap::new();

    for i in 1..num_edicts {
        if !ctx.edicts[i].inuse {
            continue;
        }
        if ctx.edicts[i].team.is_empty() {
            continue;
        }
        // Skip already-processed slaves (shouldn't happen on first pass, but defensive)
        if ctx.edicts[i].flags.intersects(FL_TEAMSLAVE) {
            continue;
        }

        team_groups
            .entry(ctx.edicts[i].team.clone())
            .or_default()
            .push(i);
    }

    // Phase 2: Chain each team group - O(n) total across all groups
    let mut num_teams = 0;
    let mut num_entities = 0;

    for (_team_name, members) in team_groups.iter() {
        if members.is_empty() {
            continue;
        }

        num_teams += 1;
        num_entities += members.len() as i32;

        // First entity is the team master
        let master_idx = members[0];
        ctx.edicts[master_idx].teammaster = master_idx as i32;

        // Chain the rest as slaves
        for (chain_pos, &member_idx) in members.iter().enumerate() {
            if chain_pos == 0 {
                // Master already set above
                continue;
            }

            // Link previous entity to this one
            let prev_idx = members[chain_pos - 1];
            ctx.edicts[prev_idx].teamchain = member_idx as i32;

            // Mark this entity as slave
            ctx.edicts[member_idx].teammaster = master_idx as i32;
            ctx.edicts[member_idx].flags |= FL_TEAMSLAVE;
        }
    }

    gi_dprintf(&format!("{} teams with {} entities\n", num_teams, num_entities));
}

// ============================================================
// SpawnEntities
// ============================================================

/// Creates a server's entity / program execution context by
/// parsing textual entity definitions out of an ent file.
/// Mirrors the C SpawnEntities.
pub fn spawn_entities(ctx: &mut GameContext, mapname: &str, entities: &str, spawnpoint: &str) {
    // Clamp skill
    let mut skill_level = ctx.skill.floor();
    if skill_level < 0.0 {
        skill_level = 0.0;
    }
    if skill_level > 3.0 {
        skill_level = 3.0;
    }
    if ctx.skill != skill_level {
        gi_cvar_forceset("skill", &format!("{}", skill_level));
        ctx.skill = skill_level;
    }

    // SaveClientData deferred: p_client::GameContext differs from g_spawn::GameCtx

    gi_free_tags(TAG_LEVEL);

    // Clear level and edicts
    // Parallel edict initialization - always beneficial for 1024 edicts
    ctx.level = LevelLocals::default();
    ctx.edicts.par_iter_mut().for_each(|e| {
        *e = Edict::default();
    });

    // Copy mapname and spawnpoint
    ctx.level.mapname = mapname.to_string();
    ctx.game.spawnpoint = spawnpoint.to_string();

    // Set client fields on player ents
    for i in 0..ctx.game.maxclients as usize {
        ctx.edicts[i + 1].client = Some(i);
    }

    let mut first_ent = true;
    let mut inhibit: i32 = 0;
    let mut remaining = entities.to_string();

    // parse ents
    loop {
        // parse the opening brace
        let (com_token, rest) = com_parse(&remaining);
        let rest = match rest {
            Some(r) => r,
            None => {
                if com_token.is_empty() {
                    break;
                }
                // Single token left, check if it's a brace
                if com_token != "{" {
                    panic!("ED_LoadFromFile: found {} when expecting {{", com_token);
                }
                ""
            }
        };
        if com_token != "{" {
            panic!("ED_LoadFromFile: found {} when expecting {{", com_token);
        }

        let ent_idx;
        if first_ent {
            ent_idx = 0; // world entity = g_edicts[0]
            ctx.edicts[0].inuse = true;
            ctx.num_edicts = 1;
            first_ent = false;
        } else {
            ent_idx = g_spawn(ctx);
        }

        let rest_str = match ed_parse_edict(ctx, rest, ent_idx) {
            Some(s) => s,
            None => break,
        };

        // map hack: command map, trigger_once, model *27
        if ctx.level.mapname.eq_ignore_ascii_case("command")
            && ctx.edicts[ent_idx].classname.eq_ignore_ascii_case("trigger_once")
            && ctx.edicts[ent_idx].model.eq_ignore_ascii_case("*27")
        {
            ctx.edicts[ent_idx].spawnflags &= !SPAWNFLAG_NOT_HARD;
        }

        // remove things (except the world) from different skill levels or deathmatch
        if ent_idx != 0 {
            if ctx.deathmatch != 0.0 {
                if (ctx.edicts[ent_idx].spawnflags & SPAWNFLAG_NOT_DEATHMATCH) != 0 {
                    g_free_edict(ctx, ent_idx);
                    inhibit += 1;
                    remaining = rest_str;
                    continue;
                }
            } else {
                // Note: coop check commented out in original C, preserved here
                if ((ctx.skill == 0.0) && (ctx.edicts[ent_idx].spawnflags & SPAWNFLAG_NOT_EASY) != 0)
                    || ((ctx.skill == 1.0) && (ctx.edicts[ent_idx].spawnflags & SPAWNFLAG_NOT_MEDIUM) != 0)
                    || (((ctx.skill == 2.0) || (ctx.skill == 3.0))
                        && (ctx.edicts[ent_idx].spawnflags & SPAWNFLAG_NOT_HARD) != 0)
                {
                    g_free_edict(ctx, ent_idx);
                    inhibit += 1;
                    remaining = rest_str;
                    continue;
                }
            }

            ctx.edicts[ent_idx].spawnflags &= !(SPAWNFLAG_NOT_EASY
                | SPAWNFLAG_NOT_MEDIUM
                | SPAWNFLAG_NOT_HARD
                | SPAWNFLAG_NOT_COOP
                | SPAWNFLAG_NOT_DEATHMATCH);
        }

        ed_call_spawn(ctx, ent_idx);
        remaining = rest_str;
    }

    gi_dprintf(&format!("{} entities inhibited\n", inhibit));

    g_find_teams(ctx);

    // Build O(1) entity lookup indices now that all entities are spawned
    ctx.build_entity_indices();

    ctx.num_edicts = ctx.edicts.len() as i32;
    ctx.max_edicts = ctx.edicts.capacity() as i32;
    crate::p_trail::player_trail_init(ctx);

}

// ============================================================
// SP_worldspawn
// ============================================================

/// Only used for the world entity.
/// Sets up the world, precaches common resources, configures light animation tables.
/// Mirrors the C SP_worldspawn.
pub fn sp_worldspawn(ctx: &mut GameContext, ent_idx: usize) {
    ctx.edicts[ent_idx].movetype = MoveType::Push;
    ctx.edicts[ent_idx].solid = Solid::Bsp;
    ctx.edicts[ent_idx].inuse = true;           // since the world doesn't use G_Spawn()
    ctx.edicts[ent_idx].s.modelindex = 1;       // world model is always index 1

    //---------------

    // reserve some spots for dead player bodies for coop / deathmatch
    // InitBodyQue deferred: p_client::GameContext differs from g_spawn::GameContext

    // set configstrings for items
    // SetItemNames deferred: g_items::GameContext differs from g_spawn::GameContext

    if !ctx.st.nextmap.is_empty() {
        ctx.level.nextmap = ctx.st.nextmap.clone();
    }

    // make some data visible to the server
    let message = ctx.edicts[ent_idx].message.clone();
    if !message.is_empty() {
        gi_configstring(CS_NAME as i32, &message);
        ctx.level.level_name = message;
    } else {
        ctx.level.level_name = ctx.level.mapname.clone();
    }

    let sky = ctx.st.sky.clone();
    if !sky.is_empty() {
        gi_configstring(CS_SKY as i32, &sky);
    } else {
        gi_configstring(CS_SKY as i32, "unit1_");
    }

    gi_configstring(CS_SKYROTATE as i32, &format!("{}", ctx.st.skyrotate));
    gi_configstring(CS_SKYAXIS as i32, &format!("{} {} {}",
        ctx.st.skyaxis[0], ctx.st.skyaxis[1], ctx.st.skyaxis[2]));
    gi_configstring(CS_CDTRACK as i32, &format!("{}", ctx.edicts[ent_idx].sounds));
    gi_configstring(CS_MAXCLIENTS as i32, &format!("{}", ctx.maxclients as i32));

    // status bar program
    if ctx.deathmatch != 0.0 {
        gi_configstring(CS_STATUSBAR as i32, DM_STATUSBAR);
    } else {
        gi_configstring(CS_STATUSBAR as i32, SINGLE_STATUSBAR);
    }

    //---------------

    // help icon for statusbar
    gi_imageindex("i_help");
    ctx.level.pic_health = gi_imageindex("i_health");
    gi_imageindex("help");
    gi_imageindex("field_3");

    let gravity = ctx.st.gravity.clone();
    if gravity.is_empty() {
        gi_cvar_set("sv_gravity", "800");
    } else {
        gi_cvar_set("sv_gravity", &gravity);
    }

    ctx.snd_fry = gi_soundindex("player/fry.wav");

    // PrecacheItem deferred: g_items::GameContext differs from g_spawn::GameContext

    // All the precached sounds
    let sounds = [
        "player/lava1.wav",
        "player/lava2.wav",
        "misc/pc_up.wav",
        "misc/talk1.wav",
        "misc/udeath.wav",
        "items/respawn1.wav",
        "*death1.wav",
        "*death2.wav",
        "*death3.wav",
        "*death4.wav",
        "*fall1.wav",
        "*fall2.wav",
        "*gurp1.wav",
        "*gurp2.wav",
        "*jump1.wav",
        "*pain25_1.wav",
        "*pain25_2.wav",
        "*pain50_1.wav",
        "*pain50_2.wav",
        "*pain75_1.wav",
        "*pain75_2.wav",
        "*pain100_1.wav",
        "*pain100_2.wav",
        "player/gasp1.wav",
        "player/gasp2.wav",
        "player/watr_in.wav",
        "player/watr_out.wav",
        "player/watr_un.wav",
        "player/u_breath1.wav",
        "player/u_breath2.wav",
        "items/pkup.wav",
        "world/land.wav",
        "misc/h2ohit1.wav",
        "items/damage.wav",
        "items/protect.wav",
        "items/protect4.wav",
        "weapons/noammo.wav",
        "infantry/inflies1.wav",
    ];
    for s in &sounds {
        gi_soundindex(s);
    }

    // Precached models
    ctx.sm_meat_index = gi_modelindex("models/objects/gibs/sm_meat/tris.md2");

    let models = [
        // sexed weapon models — order must match defines in g_local.h
        "#w_blaster.md2",
        "#w_shotgun.md2",
        "#w_sshotgun.md2",
        "#w_machinegun.md2",
        "#w_chaingun.md2",
        "#a_grenades.md2",
        "#w_glauncher.md2",
        "#w_rlauncher.md2",
        "#w_hyperblaster.md2",
        "#w_railgun.md2",
        "#w_bfg.md2",
        // gib models
        "models/objects/gibs/arm/tris.md2",
        "models/objects/gibs/bone/tris.md2",
        "models/objects/gibs/bone2/tris.md2",
        "models/objects/gibs/chest/tris.md2",
        "models/objects/gibs/skull/tris.md2",
        "models/objects/gibs/head2/tris.md2",
    ];
    for m in &models {
        gi_modelindex(m);
    }

    //
    // Setup light animation tables. 'a' is total darkness, 'z' is doublebright.
    //
    let light_styles: &[(usize, &str)] = &[
        (0,  "m"),
        (1,  "mmnmmommommnonmmonqnmmo"),
        (2,  "abcdefghijklmnopqrstuvwxyzyxwvutsrqponmlkjihgfedcba"),
        (3,  "mmmmmaaaaammmmmaaaaaabcdefgabcdefg"),
        (4,  "mamamamamama"),
        (5,  "jklmnopqrstuvwxyzyxwvutsrqponmlkj"),
        (6,  "nmonqnmomnmomomno"),
        (7,  "mmmaaaabcdefgmmmmaaaammmaamm"),
        (8,  "mmmaaammmaaammmabcdefaaaammmmabcdefmmmaaaa"),
        (9,  "aaaaaaaazzzzzzzz"),
        (10, "mmamammmmammamamaaamammma"),
        (11, "abcdefghijklmnopqrrqponmlkjihgfedcba"),
        // styles 32-62 are assigned by the light program for switchable lights
        (63, "a"),
    ];

    for &(idx, pattern) in light_styles {
        gi_configstring(CS_LIGHTS as i32 + idx as i32, pattern);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // parse_vec3 tests
    // ============================================================

    #[test]
    fn test_parse_vec3_normal() {
        let result = parse_vec3("1 2 3");
        assert_eq!(result, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_parse_vec3_floats() {
        let result = parse_vec3("1.5 -2.5 3.75");
        assert_eq!(result, [1.5, -2.5, 3.75]);
    }

    #[test]
    fn test_parse_vec3_empty() {
        let result = parse_vec3("");
        assert_eq!(result, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_parse_vec3_partial_two() {
        let result = parse_vec3("1 2");
        assert_eq!(result, [1.0, 2.0, 0.0]);
    }

    #[test]
    fn test_parse_vec3_partial_one() {
        let result = parse_vec3("42");
        assert_eq!(result, [42.0, 0.0, 0.0]);
    }

    #[test]
    fn test_parse_vec3_malformed() {
        let result = parse_vec3("abc");
        assert_eq!(result, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_parse_vec3_malformed_mixed() {
        let result = parse_vec3("1 abc 3");
        assert_eq!(result, [1.0, 0.0, 3.0]);
    }

    #[test]
    fn test_parse_vec3_extra_whitespace() {
        let result = parse_vec3("  10   20   30  ");
        assert_eq!(result, [10.0, 20.0, 30.0]);
    }

    #[test]
    fn test_parse_vec3_extra_values_ignored() {
        let result = parse_vec3("1 2 3 4 5");
        assert_eq!(result, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_parse_vec3_negative() {
        let result = parse_vec3("-100 -200 -300");
        assert_eq!(result, [-100.0, -200.0, -300.0]);
    }

    // ============================================================
    // ed_new_string tests
    // ============================================================

    #[test]
    fn test_ed_new_string_backslash_n() {
        let result = ed_new_string("hello\\nworld");
        assert_eq!(result, "hello\nworld");
    }

    #[test]
    fn test_ed_new_string_no_escapes() {
        let result = ed_new_string("hello world");
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_ed_new_string_empty() {
        let result = ed_new_string("");
        assert_eq!(result, "");
    }

    #[test]
    fn test_ed_new_string_multiple_escapes() {
        let result = ed_new_string("line1\\nline2\\nline3");
        assert_eq!(result, "line1\nline2\nline3");
    }

    #[test]
    fn test_ed_new_string_backslash_other() {
        // Non-'n' escape keeps the backslash
        let result = ed_new_string("path\\tvalue");
        assert_eq!(result, "path\\value");
    }

    #[test]
    fn test_ed_new_string_trailing_backslash() {
        // A trailing backslash with no following character is just kept as-is
        let result = ed_new_string("hello\\");
        assert_eq!(result, "hello\\");
    }

    #[test]
    fn test_ed_new_string_consecutive_newlines() {
        let result = ed_new_string("a\\n\\nb");
        assert_eq!(result, "a\n\nb");
    }

    // ============================================================
    // get_fields_index tests
    // ============================================================

    #[test]
    fn test_get_fields_index_not_empty() {
        let index = get_fields_index();
        assert!(!index.is_empty());
    }

    #[test]
    fn test_get_fields_index_has_classname() {
        let index = get_fields_index();
        assert!(index.contains_key("classname"));
    }

    #[test]
    fn test_get_fields_index_has_target() {
        let index = get_fields_index();
        assert!(index.contains_key("target"));
    }

    #[test]
    fn test_get_fields_index_has_origin() {
        let index = get_fields_index();
        assert!(index.contains_key("origin"));
    }

    #[test]
    fn test_get_fields_index_has_health() {
        let index = get_fields_index();
        assert!(index.contains_key("health"));
    }

    #[test]
    fn test_get_fields_index_has_spawntemp_fields() {
        let index = get_fields_index();
        assert!(index.contains_key("lip"));
        assert!(index.contains_key("noise"));
        assert!(index.contains_key("nextmap"));
    }

    #[test]
    fn test_get_fields_index_returns_valid_indices() {
        let index = get_fields_index();
        let &idx = index.get("classname").unwrap();
        assert!(idx < FIELDS.len());
        assert_eq!(FIELDS[idx].name, "classname");
    }

    #[test]
    fn test_get_fields_index_unknown_field() {
        let index = get_fields_index();
        assert!(!index.contains_key("nonexistent_field_xyz"));
    }

    // ============================================================
    // get_spawns_index tests
    // ============================================================

    #[test]
    fn test_get_spawns_index_not_empty() {
        let index = get_spawns_index();
        assert!(!index.is_empty());
    }

    #[test]
    fn test_get_spawns_index_has_info_player_start() {
        let index = get_spawns_index();
        assert!(index.contains_key("info_player_start"));
    }

    #[test]
    fn test_get_spawns_index_has_weapon_entities() {
        let index = get_spawns_index();
        // Check several known spawn classnames
        assert!(index.contains_key("worldspawn"));
        assert!(index.contains_key("func_door"));
        assert!(index.contains_key("trigger_once"));
    }

    #[test]
    fn test_get_spawns_index_has_monsters() {
        let index = get_spawns_index();
        assert!(index.contains_key("monster_berserk"));
        assert!(index.contains_key("monster_tank"));
    }

    #[test]
    fn test_get_spawns_index_returns_valid_indices() {
        let index = get_spawns_index();
        let &idx = index.get("info_player_start").unwrap();
        assert!(idx < SPAWNS.len());
        assert_eq!(SPAWNS[idx].name, "info_player_start");
    }

    #[test]
    fn test_get_spawns_index_unknown_spawn() {
        let index = get_spawns_index();
        assert!(!index.contains_key("weapon_shotgun")); // weapon_shotgun is an item, not in SPAWNS
    }

    #[test]
    fn test_get_spawns_index_covers_all_entries() {
        let index = get_spawns_index();
        // Verify every SPAWNS entry is in the index
        for entry in SPAWNS.iter() {
            assert!(
                index.contains_key(entry.name),
                "Missing spawn entry: {}",
                entry.name
            );
        }
    }
}
