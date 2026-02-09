// g_main.rs — Game main entry point and frame logic
// Converted from: myq2-original/game/g_main.c

/*
Copyright (C) 1997-2001 Id Software, Inc.

This program is free software; you can redistribute it and/or
modify it under the terms of the GNU General Public License
as published by the Free Software Foundation; either version 2
of the License, or (at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.

See the GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program; if not, write to the Free Software
Foundation, Inc., 59 Temple Place - Suite 330, Boston, MA  02111-1307, USA.
*/

use crate::g_local::*;
use crate::game::*;
use crate::game_import::*;
use crate::g_utils::g_spawn;
use myq2_common::q_shared::DmFlags;

// PRINT_HIGH, DF_SAME_LEVEL imported from g_local::* (via q_shared)

// ============================================================
// ShutdownGame
// ============================================================

pub fn shutdown_game(_ctx: &mut GameContext) {
    gi_dprintf("==== ShutdownGame ====\n");

    gi_free_tags(TAG_LEVEL);
    gi_free_tags(TAG_GAME);
}

// ============================================================
// GetGameAPI
//
// Returns a GameExport struct with all entry points populated.
// In C this returned a pointer to a static; here we return by value.
// Function pointers are represented as Option<usize> indices.
// ============================================================

pub struct GameExport {
    pub apiversion: i32,
    pub edict_size: usize,
    // In C, function pointers were stored here. In Rust, we use
    // the GameContext methods directly. The fields below are kept
    // for structural fidelity as callback indices.
    pub init_fn: Option<usize>,
    pub shutdown_fn: Option<usize>,
    pub spawn_entities_fn: Option<usize>,
    pub write_game_fn: Option<usize>,
    pub read_game_fn: Option<usize>,
    pub write_level_fn: Option<usize>,
    pub read_level_fn: Option<usize>,
    pub client_think_fn: Option<usize>,
    pub client_connect_fn: Option<usize>,
    pub client_userinfo_changed_fn: Option<usize>,
    pub client_disconnect_fn: Option<usize>,
    pub client_begin_fn: Option<usize>,
    pub client_command_fn: Option<usize>,
    pub run_frame_fn: Option<usize>,
    pub server_command_fn: Option<usize>,
}

pub fn get_game_api() -> GameExport {
    GameExport {
        apiversion: GAME_API_VERSION,
        edict_size: std::mem::size_of::<Edict>(),
        init_fn: Some(0),          // InitGame
        shutdown_fn: Some(1),      // ShutdownGame
        spawn_entities_fn: Some(2),// SpawnEntities
        write_game_fn: Some(3),    // WriteGame
        read_game_fn: Some(4),     // ReadGame
        write_level_fn: Some(5),   // WriteLevel
        read_level_fn: Some(6),    // ReadLevel
        client_think_fn: Some(7),  // ClientThink
        client_connect_fn: Some(8),// ClientConnect
        client_userinfo_changed_fn: Some(9), // ClientUserinfoChanged
        client_disconnect_fn: Some(10), // ClientDisconnect
        client_begin_fn: Some(11), // ClientBegin
        client_command_fn: Some(12),// ClientCommand
        run_frame_fn: Some(13),    // G_RunFrame
        server_command_fn: Some(14),// ServerCommand
    }
}

// ============================================================
// Sys_Error — only needed when not hard-linked.
// Routes through gi.error for proper engine error handling.
// ============================================================

pub fn sys_error(_ctx: &GameContext, error: &str) {
    // gi.error (ERR_FATAL, "%s", text);
    gi_error(error);
}

// ============================================================
// ClientEndServerFrames
// ============================================================

pub fn client_end_server_frames(ctx: &mut GameContext) {
    // calc the player views now that all pushing
    // and damage has been added
    let max = ctx.maxclients as i32;
    for i in 0..max {
        let ent_idx = (1 + i) as usize;
        if ent_idx >= ctx.edicts.len() {
            continue;
        }
        if !ctx.edicts[ent_idx].inuse || ctx.edicts[ent_idx].client.is_none() {
            continue;
        }
        client_end_server_frame(ctx, ent_idx);
    }
}

// ============================================================
// CreateTargetChangeLevel
//
// Returns the entity index of the newly created target_changelevel.
// ============================================================

pub fn create_target_change_level(ctx: &mut GameContext, map: &str) -> usize {
    // ent = G_Spawn();
    let ent_idx = g_spawn(ctx);
    ctx.edicts[ent_idx].classname = "target_changelevel".to_string();
    // Com_sprintf(level.nextmap, sizeof(level.nextmap), "%s", map);
    ctx.level.nextmap = map.to_string();
    ctx.edicts[ent_idx].map = ctx.level.nextmap.clone();
    ent_idx
}

// ============================================================
// EndDMLevel
//
// The timelimit or fraglimit has been exceeded.
// ============================================================

pub fn end_dm_level(ctx: &mut GameContext) {
    // stay on same level flag
    if DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_SAME_LEVEL) {
        let map = ctx.level.mapname.clone();
        let ent_idx = create_target_change_level(ctx, &map);
        begin_intermission(ctx, ent_idx);
        return;
    }

    // see if it's in the map list
    if !ctx.sv_maplist.is_empty() {
        let seps: &[char] = &[' ', ',', '\n', '\r'];
        let maps: Vec<String> = ctx.sv_maplist
            .split(|c: char| seps.contains(&c))
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        let mut first: Option<String> = None;
        let mut found = false;

        for (idx, t) in maps.iter().enumerate() {
            if t.eq_ignore_ascii_case(&ctx.level.mapname) {
                // it's in the list, go to the next one
                if idx + 1 < maps.len() {
                    let next = maps[idx + 1].clone();
                    let ent_idx = create_target_change_level(ctx, &next);
                    begin_intermission(ctx, ent_idx);
                } else {
                    // end of list, go to first one
                    match &first {
                        Some(f) => {
                            let f = f.clone();
                            let ent_idx = create_target_change_level(ctx, &f);
                            begin_intermission(ctx, ent_idx);
                        }
                        None => {
                            // there isn't a first one, same level
                            let map = ctx.level.mapname.clone();
                            let ent_idx = create_target_change_level(ctx, &map);
                            begin_intermission(ctx, ent_idx);
                        }
                    }
                }
                found = true;
                break;
            }
            if first.is_none() {
                first = Some(t.clone());
            }
        }

        if found {
            return;
        }
    }

    if !ctx.level.nextmap.is_empty() {
        // go to a specific map
        let nextmap = ctx.level.nextmap.clone();
        let ent_idx = create_target_change_level(ctx, &nextmap);
        begin_intermission(ctx, ent_idx);
    } else {
        // search for a changelevel
        let found = crate::g_utils::g_find(ctx, 0, "classname", "target_changelevel");
        match found {
            Some(ent_idx) => {
                begin_intermission(ctx, ent_idx);
            }
            None => {
                // the map designer didn't include a changelevel,
                // so create a fake ent that goes back to the same level
                let map = ctx.level.mapname.clone();
                let ent_idx = create_target_change_level(ctx, &map);
                begin_intermission(ctx, ent_idx);
            }
        }
    }
}


/// BeginIntermission — starts intermission sequence.
/// Delegates to p_hud::begin_intermission. Both types are GameCtx.
fn begin_intermission(ctx: &mut GameContext, ent_idx: usize) {
    crate::p_hud::begin_intermission(ctx, ent_idx);
}

// ============================================================
// CheckNeedPass
// ============================================================

pub fn check_need_pass(ctx: &mut GameContext) {
    // Always recompute needpass from password/spectator_password.
    // Called once per frame; the comparison is trivial.
    let mut need: i32 = 0;

    if !ctx.password.is_empty()
        && !ctx.password.eq_ignore_ascii_case("none")
    {
        need |= 1;
    }
    if !ctx.spectator_password.is_empty()
        && !ctx.spectator_password.eq_ignore_ascii_case("none")
    {
        need |= 2;
    }

    gi_cvar_set("needpass", &format!("{}", need));
    ctx.needpass = need as f32;
}

// ============================================================
// CheckDMRules
// ============================================================

pub fn check_dm_rules(ctx: &mut GameContext) {
    if ctx.level.intermissiontime != 0.0 {
        return;
    }

    if ctx.deathmatch == 0.0 {
        return;
    }

    if ctx.timelimit != 0.0
        && ctx.level.time >= ctx.timelimit * 60.0 {
            gi_bprintf(PRINT_HIGH, "Timelimit hit.\n");
            end_dm_level(ctx);
            return;
        }

    if ctx.fraglimit != 0.0 {
        let max = ctx.maxclients as i32;
        for i in 0..max {
            let ent_idx = (i + 1) as usize;
            if ent_idx >= ctx.edicts.len() || !ctx.edicts[ent_idx].inuse {
                continue;
            }
            let client_idx = i as usize;
            if client_idx >= ctx.clients.len() {
                continue;
            }
            if ctx.clients[client_idx].resp.score >= ctx.fraglimit as i32 {
                gi_bprintf(PRINT_HIGH, "Fraglimit hit.\n");
                end_dm_level(ctx);
                return;
            }
        }
    }
}

// ============================================================
// ExitLevel
// ============================================================

pub fn exit_level(ctx: &mut GameContext) {
    let command = format!("gamemap \"{}\"\n", ctx.level.changemap);
    gi_add_command_string(&command);

    ctx.level.changemap = String::new();
    ctx.level.exitintermission = 0;
    ctx.level.intermissiontime = 0.0;
    client_end_server_frames(ctx);

    // clear some things before going to next level
    let max = ctx.maxclients as i32;
    for i in 0..max {
        let ent_idx = (1 + i) as usize;
        if ent_idx >= ctx.edicts.len() || !ctx.edicts[ent_idx].inuse {
            continue;
        }
        if let Some(client_idx) = ctx.edicts[ent_idx].client {
            if client_idx < ctx.clients.len() {
                let max_health = ctx.clients[client_idx].pers.max_health;
                if ctx.edicts[ent_idx].health > max_health {
                    ctx.edicts[ent_idx].health = max_health;
                }
            }
        }
    }
}

// ============================================================
// G_RunFrame
//
// Advances the world by 0.1 seconds.
// ============================================================

pub fn g_run_frame(ctx: &mut GameContext) {
    ctx.level.framenum += 1;
    ctx.level.time = ctx.level.framenum as f32 * FRAMETIME;

    // choose a client for monsters to target this frame
    ai_set_sight_client(ctx);

    // exit intermissions
    if ctx.level.exitintermission != 0 {
        exit_level(ctx);
        return;
    }

    //
    // treat each object in turn
    // even the world gets a chance to think
    //
    let num_edicts = ctx.num_edicts as usize;
    let maxclients = ctx.maxclients as i32;

    for i in 0..num_edicts {
        if i >= ctx.edicts.len() {
            break;
        }
        if !ctx.edicts[i].inuse {
            continue;
        }

        ctx.level.current_entity = i as i32;

        // VectorCopy (ent->s.origin, ent->s.old_origin);
        ctx.edicts[i].s.old_origin = ctx.edicts[i].s.origin;

        // if the ground entity moved, make sure we are still on it
        let ground_idx = ctx.edicts[i].groundentity;
        if ground_idx >= 0 {
            let ground = ground_idx as usize;
            if ground < ctx.edicts.len()
                && ctx.edicts[ground].linkcount != ctx.edicts[i].groundentity_linkcount {
                    ctx.edicts[i].groundentity = -1;
                    let flags = ctx.edicts[i].flags;
                    let svflags = ctx.edicts[i].svflags;
                    if !flags.intersects(FL_SWIM | FL_FLY) && (svflags & SVF_MONSTER) != 0 {
                        m_check_ground(ctx, i);
                    }
                }
        }

        if i > 0 && (i as i32) <= maxclients {
            client_begin_server_frame(ctx, i);
            continue;
        }

        g_run_entity(ctx, i);
    }

    // see if it is time to end a deathmatch
    check_dm_rules(ctx);

    // see if needpass needs updated
    check_need_pass(ctx);

    // build the playerstate_t structures for all players
    client_end_server_frames(ctx);
}

// ============================================================
// Cross-module bridge functions
// ============================================================

/// AI_SetSightClient — finds a client for monster AI sight checks.
/// Bridges to g_ai::ai_set_sight_client via AiContext.
fn ai_set_sight_client(ctx: &mut GameContext) {
    let mut ai_ctx = crate::g_ai::AiContext {
        edicts: std::mem::take(&mut ctx.edicts),
        clients: std::mem::take(&mut ctx.clients),
        level: std::mem::take(&mut ctx.level),
        game: std::mem::take(&mut ctx.game),
        coop: ctx.coop,
        skill: ctx.skill,
        enemy_vis: false,
        enemy_infront: false,
        enemy_range: 0,
        enemy_yaw: 0.0,
    };
    crate::g_ai::ai_set_sight_client(&mut ai_ctx);
    ctx.edicts = ai_ctx.edicts;
    ctx.clients = ai_ctx.clients;
    ctx.level = ai_ctx.level;
    ctx.game = ai_ctx.game;
}

/// M_CheckGround — checks if a monster entity is on ground.
/// Bridges to g_monster::m_check_ground_raw via direct edicts/level access.
fn m_check_ground(ctx: &mut GameContext, ent_idx: usize) {
    crate::g_monster::m_check_ground_raw(ent_idx as i32, &mut ctx.edicts, &mut ctx.level);
}

/// G_RunEntity — dispatches entity think/physics based on movetype.
/// Bridges to g_phys::g_run_entity via direct edicts/level access.
fn g_run_entity(ctx: &mut GameContext, ent_idx: usize) {
    crate::g_phys::g_run_entity(ent_idx, &mut ctx.edicts, &mut ctx.level);
}

/// ClientBeginServerFrame — initializes frame for a client entity.
/// Delegates to p_client::client_begin_server_frame. Both types are GameCtx.
fn client_begin_server_frame(ctx: &mut GameContext, ent_idx: usize) {
    crate::p_client::client_begin_server_frame(ctx, ent_idx);
}

/// ClientEndServerFrame — updates client entity state at end of frame.
/// Bridges to p_view::client_end_server_frame.
fn client_end_server_frame(ctx: &mut GameContext, ent_idx: usize) {
    let client_idx = match ctx.edicts[ent_idx].client {
        Some(ci) => ci,
        None => return,
    };
    if client_idx >= ctx.clients.len() {
        return;
    }

    let mut vctx = crate::p_view::ViewContext::default();
    let cvars = crate::p_view::ViewCvars {
        sv_rollangle: crate::p_view::CvarRef { value: ctx.sv_rollangle },
        sv_rollspeed: crate::p_view::CvarRef { value: ctx.sv_rollspeed },
        run_pitch: crate::p_view::CvarRef { value: ctx.run_pitch },
        run_roll: crate::p_view::CvarRef { value: ctx.run_roll },
        bob_up: crate::p_view::CvarRef { value: ctx.bob_up },
        bob_pitch: crate::p_view::CvarRef { value: ctx.bob_pitch },
        bob_roll: crate::p_view::CvarRef { value: ctx.bob_roll },
        gun_x: crate::p_view::CvarRef { value: ctx.gun_x },
        gun_y: crate::p_view::CvarRef { value: ctx.gun_y },
        gun_z: crate::p_view::CvarRef { value: ctx.gun_z },
        deathmatch: crate::p_view::CvarRef { value: ctx.deathmatch },
        dmflags: crate::p_view::CvarRef { value: ctx.dmflags },
    };

    crate::p_view::client_end_server_frame(
        &mut ctx.edicts[ent_idx],
        &mut ctx.clients[client_idx],
        &ctx.level,
        &ctx.game,
        &ctx.items,
        &mut vctx,
        &cvars,
        ctx.snd_fry,
    );
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn init_test_gi() {
        // OnceLock silently ignores subsequent calls, safe for parallel tests
        crate::game_import::set_gi(Box::new(crate::game_import::StubGameImport));
    }

    fn make_ctx(num_clients: i32) -> GameContext {
        init_test_gi();
        let mut ctx = GameContext::default();
        ctx.maxclients = num_clients as f32;
        ctx.game.maxclients = num_clients;
        // Create world entity + client entities
        for _ in 0..=(num_clients as usize) {
            ctx.edicts.push(Edict::default());
        }
        ctx.num_edicts = ctx.edicts.len() as i32;
        for _ in 0..num_clients {
            ctx.clients.push(GClient::default());
        }
        ctx
    }

    #[test]
    fn test_game_context_creation() {
        let ctx = GameContext::default();
        assert_eq!(ctx.num_edicts, 0);
        assert_eq!(ctx.means_of_death, 0);
        assert_eq!(ctx.level.framenum, 0);
        assert_eq!(ctx.level.time, 0.0);
    }

    #[test]
    fn test_g_run_frame_increments_time() {
        let mut ctx = make_ctx(1);
        ctx.edicts[0].inuse = true; // world entity
        g_run_frame(&mut ctx);
        assert_eq!(ctx.level.framenum, 1);
        assert!((ctx.level.time - FRAMETIME).abs() < f32::EPSILON);

        g_run_frame(&mut ctx);
        assert_eq!(ctx.level.framenum, 2);
        assert!((ctx.level.time - 2.0 * FRAMETIME).abs() < f32::EPSILON);
    }

    #[test]
    fn test_g_run_frame_exit_intermission() {
        let mut ctx = make_ctx(1);
        ctx.edicts[0].inuse = true;
        ctx.level.exitintermission = 1;
        ctx.level.changemap = "test".to_string();
        g_run_frame(&mut ctx);
        // After ExitLevel, changemap is cleared
        assert!(ctx.level.changemap.is_empty());
        assert_eq!(ctx.level.exitintermission, 0);
        assert_eq!(ctx.level.intermissiontime, 0.0);
    }

    #[test]
    fn test_check_need_pass_no_passwords() {
        init_test_gi();
        let mut ctx = GameContext::default();
        check_need_pass(&mut ctx);
        // No passwords set, needpass should be 0
        assert_eq!(ctx.needpass, 0.0);
    }

    #[test]
    fn test_check_need_pass_password_set() {
        init_test_gi();
        let mut ctx = GameContext::default();
        ctx.password = "secret".to_string();
        check_need_pass(&mut ctx);
        assert_eq!(ctx.needpass, 1.0);
    }

    #[test]
    fn test_check_need_pass_both() {
        init_test_gi();
        let mut ctx = GameContext::default();
        ctx.password = "secret".to_string();
        ctx.spectator_password = "spec".to_string();
        check_need_pass(&mut ctx);
        assert_eq!(ctx.needpass, 3.0);
    }

    #[test]
    fn test_check_need_pass_none_string() {
        init_test_gi();
        let mut ctx = GameContext::default();
        ctx.password = "none".to_string();
        ctx.spectator_password = "NONE".to_string();
        check_need_pass(&mut ctx);
        assert_eq!(ctx.needpass, 0.0);
    }

    #[test]
    fn test_check_dm_rules_no_deathmatch() {
        let mut ctx = make_ctx(4);
        ctx.deathmatch = 0.0;
        ctx.timelimit = 10.0;
        ctx.level.time = 999.0;
        // Should return early without calling EndDMLevel
        check_dm_rules(&mut ctx);
    }

    #[test]
    fn test_check_dm_rules_intermission_active() {
        let mut ctx = make_ctx(4);
        ctx.deathmatch = 1.0;
        ctx.level.intermissiontime = 5.0;
        // Should return early
        check_dm_rules(&mut ctx);
    }

    #[test]
    fn test_create_target_change_level() {
        let mut ctx = make_ctx(1);
        let idx = create_target_change_level(&mut ctx, "base2");
        assert_eq!(ctx.edicts[idx].classname, "target_changelevel");
        assert_eq!(ctx.edicts[idx].map, "base2");
        assert_eq!(ctx.level.nextmap, "base2");
    }

    #[test]
    fn test_get_game_api() {
        let export = get_game_api();
        assert_eq!(export.apiversion, GAME_API_VERSION);
        assert!(export.init_fn.is_some());
        assert!(export.shutdown_fn.is_some());
        assert!(export.run_frame_fn.is_some());
    }

    #[test]
    fn test_g_find_by_classname() {
        let mut ctx = make_ctx(0);
        let mut ent = Edict::default();
        ent.inuse = true;
        ent.classname = "target_changelevel".to_string();
        ctx.edicts.push(ent);
        ctx.num_edicts = ctx.edicts.len() as i32;
        ctx.build_entity_indices();
        let result = crate::g_utils::g_find(&ctx, 0, "classname", "target_changelevel");
        assert!(result.is_some());

        let result = crate::g_utils::g_find(&ctx, 0, "classname", "nonexistent");
        assert!(result.is_none());
    }

    #[test]
    fn test_exit_level_clamps_health() {
        let mut ctx = make_ctx(1);
        ctx.edicts[0].inuse = true;
        ctx.edicts[1].inuse = true;
        ctx.edicts[1].client = Some(0);
        ctx.edicts[1].health = 200;
        ctx.clients[0].pers.max_health = 100;
        ctx.level.changemap = "base1".to_string();
        exit_level(&mut ctx);
        assert_eq!(ctx.edicts[1].health, 100);
    }

    #[test]
    fn test_ground_entity_check_in_run_frame() {
        let mut ctx = make_ctx(0);
        // World entity
        ctx.edicts[0].inuse = true;
        ctx.edicts[0].linkcount = 5;
        // A monster entity
        let mut monster = Edict::default();
        monster.inuse = true;
        monster.groundentity = 0; // grounded on world
        monster.groundentity_linkcount = 3; // stale linkcount
        monster.svflags = SVF_MONSTER;
        ctx.edicts.push(monster);
        ctx.num_edicts = ctx.edicts.len() as i32;

        g_run_frame(&mut ctx);

        // Ground entity should have been cleared because linkcount mismatched
        assert_eq!(ctx.edicts[1].groundentity, -1);
    }
}
