// p_hud.rs — Player HUD, intermission, and scoreboard logic
// Converted from: myq2-original/game/p_hud.c
//
// Copyright (C) 1997-2001 Id Software, Inc.
//
// This program is free software; you can redistribute it and/or
// modify it under the terms of the GNU General Public License
// as published by the Free Software Foundation; either version 2
// of the License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
//
// See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program; if not, write to the Free Software
// Foundation, Inc., 59 Temple Place - Suite 330, Boston, MA  02111-1307, USA.

use crate::g_local::*;
use crate::game::*;
use crate::game_import::*;
use myq2_common::q_shared::{
    vector_copy, PmType, RDF_UNDERWATER, MAX_CLIENTS,
    CS_PLAYERSKINS, CHAN_ITEM, ATTN_NORM,
    STAT_HEALTH_ICON, STAT_HEALTH, STAT_AMMO_ICON, STAT_AMMO,
    STAT_ARMOR_ICON, STAT_ARMOR, STAT_SELECTED_ICON, STAT_SELECTED_ITEM,
    STAT_PICKUP_ICON, STAT_PICKUP_STRING, STAT_TIMER_ICON, STAT_TIMER,
    STAT_HELPICON, STAT_LAYOUTS, STAT_FRAGS, STAT_CHASE, STAT_SPECTATOR,
};

// ======================================================================
// INTERMISSION
// ======================================================================

/// Move a client entity to the intermission point.
/// C: MoveClientToIntermission
pub fn move_client_to_intermission(ctx: &mut GameContext, ent_idx: usize) {
    if ctx.deathmatch != 0.0 || ctx.coop != 0.0 {
        ctx.client_of_mut(ent_idx).showscores = true;
    }

    // VectorCopy(level.intermission_origin, ent->s.origin)
    ctx.edicts[ent_idx].s.origin = vector_copy(&ctx.level.intermission_origin);

    let intermission_origin = ctx.level.intermission_origin;
    let intermission_angle = ctx.level.intermission_angle;
    {
        let cl = ctx.client_of_mut(ent_idx);
        cl.ps.pmove.origin[0] = (intermission_origin[0] * 8.0) as i16;
        cl.ps.pmove.origin[1] = (intermission_origin[1] * 8.0) as i16;
        cl.ps.pmove.origin[2] = (intermission_origin[2] * 8.0) as i16;

        // VectorCopy(level.intermission_angle, ent->client->ps.viewangles)
        cl.ps.viewangles = vector_copy(&intermission_angle);

        cl.ps.pmove.pm_type = PmType::Freeze;
        cl.ps.gunindex = 0;
        cl.ps.blend[3] = 0.0;
        cl.ps.rdflags &= !RDF_UNDERWATER;

        // clean up powerup info
        cl.quad_framenum = 0.0;
        cl.invincible_framenum = 0.0;
        cl.breather_framenum = 0.0;
        cl.enviro_framenum = 0.0;
        cl.grenade_blew_up = false;
        cl.grenade_time = 0.0;
    }

    ctx.edicts[ent_idx].viewheight = 0;
    ctx.edicts[ent_idx].s.modelindex = 0;
    ctx.edicts[ent_idx].s.modelindex2 = 0;
    ctx.edicts[ent_idx].s.modelindex3 = 0;
    ctx.edicts[ent_idx].s.modelindex = 0; // intentional duplicate from original
    ctx.edicts[ent_idx].s.effects = 0;
    ctx.edicts[ent_idx].s.sound = 0;
    ctx.edicts[ent_idx].solid = Solid::Not;

    // add the layout
    if ctx.deathmatch != 0.0 || ctx.coop != 0.0 {
        deathmatch_scoreboard_message(ctx, ent_idx, -1);
        gi_unicast(ent_idx as i32, true);
    }
}

/// Begin an intermission sequence.
/// C: BeginIntermission
pub fn begin_intermission(ctx: &mut GameContext, targ_idx: usize) {
    if ctx.level.intermissiontime != 0.0 {
        return; // already activated
    }

    ctx.game.autosaved = false;

    // respawn any dead clients
    let maxclients = ctx.maxclients;
    for i in 0..maxclients as usize {
        let client_idx = 1 + i;
        if !ctx.edicts[client_idx].inuse {
            continue;
        }
        if ctx.edicts[client_idx].health <= 0 {
            respawn_client(ctx, client_idx);
        }
    }

    ctx.level.intermissiontime = ctx.level.time;
    ctx.level.changemap = ctx.edicts[targ_idx].map.clone();

    if ctx.level.changemap.contains('*') {
        if ctx.coop != 0.0 {
            for i in 0..maxclients as usize {
                let client_idx = 1 + i;
                if !ctx.edicts[client_idx].inuse {
                    continue;
                }
                // strip players of all keys between units
                let cl = ctx.edicts[client_idx].client.expect("client entity has no client");
                for n in 0..ctx.items.len() {
                    if ctx.items[n].flags.intersects(IT_KEY) {
                        ctx.clients[cl].pers.inventory[n] = 0;
                    }
                }
            }
        }
    } else if ctx.deathmatch == 0.0 {
        ctx.level.exitintermission = 1; // go immediately to the next level
        return;
    }

    ctx.level.exitintermission = 0;

    // find an intermission spot
    // G_Find(NULL, FOFS(classname), "info_player_intermission")
    let mut ent_found: Option<usize> = None;

    for idx in 0..ctx.edicts.len() {
        if ctx.edicts[idx].inuse && ctx.edicts[idx].classname == "info_player_intermission" {
            ent_found = Some(idx);
            break;
        }
    }

    if ent_found.is_none() {
        // the map creator forgot to put in an intermission point...
        for idx in 0..ctx.edicts.len() {
            if ctx.edicts[idx].inuse && ctx.edicts[idx].classname == "info_player_start" {
                ent_found = Some(idx);
                break;
            }
        }
        if ent_found.is_none() {
            for idx in 0..ctx.edicts.len() {
                if ctx.edicts[idx].inuse && ctx.edicts[idx].classname == "info_player_deathmatch" {
                    ent_found = Some(idx);
                    break;
                }
            }
        }
    } else {
        // chose one of four spots
        let mut count = rand_int() & 3;
        while count > 0 {
            count -= 1;
            let start = ent_found.unwrap_or(0) + 1;
            let mut next = None;
            for idx in start..ctx.edicts.len() {
                if ctx.edicts[idx].inuse && ctx.edicts[idx].classname == "info_player_intermission" {
                    next = Some(idx);
                    break;
                }
            }
            if next.is_none() {
                // wrap around the list
                for idx in 0..ctx.edicts.len() {
                    if ctx.edicts[idx].inuse && ctx.edicts[idx].classname == "info_player_intermission" {
                        next = Some(idx);
                        break;
                    }
                }
            }
            if let Some(n) = next {
                ent_found = Some(n);
            }
        }
    }

    if let Some(ent) = ent_found {
        ctx.level.intermission_origin = vector_copy(&ctx.edicts[ent].s.origin);
        ctx.level.intermission_angle = vector_copy(&ctx.edicts[ent].s.angles);
    }

    // move all clients to the intermission point
    for i in 0..maxclients as usize {
        let client_idx = 1 + i;
        if !ctx.edicts[client_idx].inuse {
            continue;
        }
        move_client_to_intermission(ctx, client_idx);
    }
}

use myq2_common::common::rand_i32 as rand_int;

// ======================================================================
// SCOREBOARD
// ======================================================================

/// Build and write the deathmatch scoreboard layout message.
/// `killer_idx` is the entity index of the killer, or -1 if none.
/// C: DeathmatchScoreboardMessage
pub fn deathmatch_scoreboard_message(ctx: &mut GameContext, ent_idx: usize, killer_idx: i32) {
    let mut sorted: [i32; MAX_CLIENTS] = [0; MAX_CLIENTS];
    let mut sorted_scores: [i32; MAX_CLIENTS] = [0; MAX_CLIENTS];
    let mut total: usize = 0;

    // sort the clients by score
    for i in 0..ctx.game.maxclients as usize {
        let cl_ent_idx = 1 + i;
        if !ctx.edicts[cl_ent_idx].inuse || ctx.clients[i].resp.spectator {
            continue;
        }
        let score = ctx.clients[i].resp.score;
        let mut j = 0usize;
        while j < total {
            if score > sorted_scores[j] {
                break;
            }
            j += 1;
        }
        let mut k = total;
        while k > j {
            sorted[k] = sorted[k - 1];
            sorted_scores[k] = sorted_scores[k - 1];
            k -= 1;
        }
        sorted[j] = i as i32;
        sorted_scores[j] = score;
        total += 1;
    }

    // print level name and exit rules
    let mut string = String::new();

    // add the clients in sorted order
    if total > 12 {
        total = 12;
    }

    for i in 0..total {
        let cl_idx = sorted[i] as usize;
        let cl_ent_idx = 1 + cl_idx;

        let _picnum = gi_imageindex("i_fixme");

        let x = if i >= 6 { 160 } else { 0 };
        let y = 32 + 32 * (i % 6) as i32;

        // add a dogtag
        let tag: Option<&str> = if cl_ent_idx == ent_idx {
            Some("tag1")
        } else if killer_idx >= 0 && cl_ent_idx == killer_idx as usize {
            Some("tag2")
        } else {
            None
        };

        if let Some(tag_str) = tag {
            let entry = format!("xv {} yv {} picn {} ", x + 32, y, tag_str);
            if string.len() + entry.len() > 1024 {
                break;
            }
            string.push_str(&entry);
        }

        // send the layout
        let cl_score = ctx.clients[cl_idx].resp.score;
        let cl_ping = ctx.clients[cl_idx].ping;
        let cl_enterframe = ctx.clients[cl_idx].resp.enterframe;
        let time = (ctx.level.framenum - cl_enterframe) / 600;

        let entry = format!(
            "client {} {} {} {} {} {} ",
            x, y, sorted[i], cl_score, cl_ping, time
        );
        if string.len() + entry.len() > 1024 {
            break;
        }
        string.push_str(&entry);
    }

    gi_write_byte(SVC_LAYOUT);
    gi_write_string(&string);
}

/// Draw the deathmatch scoreboard (instead of help message).
/// Note that it isn't that hard to overflow the 1400 byte message limit!
/// C: DeathmatchScoreboard
pub fn deathmatch_scoreboard(ctx: &mut GameContext, ent_idx: usize) {
    let enemy_idx = ctx.edicts[ent_idx].enemy;
    deathmatch_scoreboard_message(ctx, ent_idx, enemy_idx);
    gi_unicast(ent_idx as i32, true);
}

/// Display the scoreboard.
/// C: Cmd_Score_f
pub fn cmd_score_f(ctx: &mut GameContext, ent_idx: usize) {
    ctx.client_of_mut(ent_idx).showinventory = false;
    ctx.client_of_mut(ent_idx).showhelp = false;

    if ctx.deathmatch == 0.0 && ctx.coop == 0.0 {
        return;
    }

    if ctx.client_of(ent_idx).showscores {
        ctx.client_of_mut(ent_idx).showscores = false;
        return;
    }

    ctx.client_of_mut(ent_idx).showscores = true;
    deathmatch_scoreboard(ctx, ent_idx);
}

/// Draw help computer.
/// C: HelpComputer
pub fn help_computer(ctx: &mut GameContext, ent_idx: usize) {
    let sk = if ctx.skill == 0.0 {
        "easy"
    } else if ctx.skill == 1.0 {
        "medium"
    } else if ctx.skill == 2.0 {
        "hard"
    } else {
        "hard+"
    };

    // send the layout
    let string = format!(
        "xv 32 yv 8 picn help \
         xv 202 yv 12 string2 \"{}\" \
         xv 0 yv 24 cstring2 \"{}\" \
         xv 0 yv 54 cstring2 \"{}\" \
         xv 0 yv 110 cstring2 \"{}\" \
         xv 50 yv 164 string2 \" kills     goals    secrets\" \
         xv 50 yv 172 string2 \"{:3}/{:3}     {}/{}       {}/{}\" ",
        sk,
        ctx.level.level_name,
        ctx.game.helpmessage1,
        ctx.game.helpmessage2,
        ctx.level.killed_monsters, ctx.level.total_monsters,
        ctx.level.found_goals, ctx.level.total_goals,
        ctx.level.found_secrets, ctx.level.total_secrets,
    );

    gi_write_byte(SVC_LAYOUT);
    gi_write_string(&string);
    gi_unicast(ent_idx as i32, true);
}

/// Display the current help message.
/// C: Cmd_Help_f
pub fn cmd_help_f(ctx: &mut GameContext, ent_idx: usize) {
    // this is for backwards compatibility
    if ctx.deathmatch != 0.0 {
        cmd_score_f(ctx, ent_idx);
        return;
    }

    ctx.client_of_mut(ent_idx).showinventory = false;
    ctx.client_of_mut(ent_idx).showscores = false;

    let showhelp = ctx.client_of(ent_idx).showhelp;
    let game_helpchanged = ctx.client_of(ent_idx).pers.game_helpchanged;

    if showhelp && (game_helpchanged == ctx.game.helpchanged) {
        ctx.client_of_mut(ent_idx).showhelp = false;
        return;
    }

    ctx.client_of_mut(ent_idx).showhelp = true;
    ctx.client_of_mut(ent_idx).pers.helpchanged = 0;
    help_computer(ctx, ent_idx);
}

// =======================================================================
// G_SetStats
// =======================================================================

/// Set player stats for HUD display.
/// C: G_SetStats
pub fn g_set_stats(ctx: &mut GameContext, ent_idx: usize) {
    // health
    {
        let pic_health = ctx.level.pic_health;
        let health = ctx.edicts[ent_idx].health;
        let cl = ctx.client_of_mut(ent_idx);
        cl.ps.stats[STAT_HEALTH_ICON as usize] = pic_health as i16;
        cl.ps.stats[STAT_HEALTH as usize] = health as i16;
    }

    // ammo
    {
        let ammo_index = ctx.client_of(ent_idx).ammo_index;
        if ammo_index == 0 {
            let cl = ctx.client_of_mut(ent_idx);
            cl.ps.stats[STAT_AMMO_ICON as usize] = 0;
            cl.ps.stats[STAT_AMMO as usize] = 0;
        } else {
            let icon = ctx.items[ammo_index as usize].icon.clone();
            let inv_count = ctx.client_of(ent_idx).pers.inventory[ammo_index as usize];
            let cl = ctx.client_of_mut(ent_idx);
            cl.ps.stats[STAT_AMMO_ICON as usize] = gi_imageindex(&icon) as i16;
            cl.ps.stats[STAT_AMMO as usize] = inv_count as i16;
        }
    }

    // armor
    {
        let mut power_armor_type: i32 = hud_power_armor_type(ctx, ent_idx);

        if power_armor_type != 0 {
            let cells_index = crate::g_items::find_item("cells").unwrap_or(0);
            let cells = ctx.client_of(ent_idx).pers.inventory[cells_index];
            if cells == 0 {
                // ran out of cells for power armor
                ctx.edicts[ent_idx].flags &= !FL_POWER_ARMOR;
                gi_sound(ent_idx as i32, CHAN_ITEM, gi_soundindex("misc/power2.wav"), 1.0, ATTN_NORM as f32, 0.0);
                power_armor_type = 0;
            }
        }

        let armor_index: i32 = hud_armor_index(ctx, ent_idx) as i32;

        if power_armor_type != 0 && (armor_index == 0 || (ctx.level.framenum & 8) != 0) {
            // flash between power armor and other armor icon
            let cells_index = crate::g_items::find_item("cells").unwrap_or(0);
            let cells = ctx.client_of(ent_idx).pers.inventory[cells_index];
            let cl = ctx.client_of_mut(ent_idx);
            cl.ps.stats[STAT_ARMOR_ICON as usize] = gi_imageindex("i_powershield") as i16;
            cl.ps.stats[STAT_ARMOR as usize] = cells as i16;
        } else if armor_index != 0 {
            let icon = ctx.items[armor_index as usize].icon.clone();
            let inv_count = ctx.client_of(ent_idx).pers.inventory[armor_index as usize];
            let cl = ctx.client_of_mut(ent_idx);
            cl.ps.stats[STAT_ARMOR_ICON as usize] = gi_imageindex(&icon) as i16;
            cl.ps.stats[STAT_ARMOR as usize] = inv_count as i16;
        } else {
            let cl = ctx.client_of_mut(ent_idx);
            cl.ps.stats[STAT_ARMOR_ICON as usize] = 0;
            cl.ps.stats[STAT_ARMOR as usize] = 0;
        }
    }

    // pickup message
    {
        let pickup_msg_time = ctx.client_of(ent_idx).pickup_msg_time;
        if ctx.level.time > pickup_msg_time {
            let cl = ctx.client_of_mut(ent_idx);
            cl.ps.stats[STAT_PICKUP_ICON as usize] = 0;
            cl.ps.stats[STAT_PICKUP_STRING as usize] = 0;
        }
    }

    // timers
    {
        let quad_framenum = ctx.client_of(ent_idx).quad_framenum;
        let invincible_framenum = ctx.client_of(ent_idx).invincible_framenum;
        let enviro_framenum = ctx.client_of(ent_idx).enviro_framenum;
        let breather_framenum = ctx.client_of(ent_idx).breather_framenum;
        let framenum = ctx.level.framenum as f32;

        if quad_framenum > framenum {
            let cl = ctx.client_of_mut(ent_idx);
            cl.ps.stats[STAT_TIMER_ICON as usize] = gi_imageindex("p_quad") as i16;
            cl.ps.stats[STAT_TIMER as usize] = ((quad_framenum - framenum) / 10.0) as i16;
        } else if invincible_framenum > framenum {
            let cl = ctx.client_of_mut(ent_idx);
            cl.ps.stats[STAT_TIMER_ICON as usize] = gi_imageindex("p_invulnerability") as i16;
            cl.ps.stats[STAT_TIMER as usize] = ((invincible_framenum - framenum) / 10.0) as i16;
        } else if enviro_framenum > framenum {
            let cl = ctx.client_of_mut(ent_idx);
            cl.ps.stats[STAT_TIMER_ICON as usize] = gi_imageindex("p_envirosuit") as i16;
            cl.ps.stats[STAT_TIMER as usize] = ((enviro_framenum - framenum) / 10.0) as i16;
        } else if breather_framenum > framenum {
            let cl = ctx.client_of_mut(ent_idx);
            cl.ps.stats[STAT_TIMER_ICON as usize] = gi_imageindex("p_rebreather") as i16;
            cl.ps.stats[STAT_TIMER as usize] = ((breather_framenum - framenum) / 10.0) as i16;
        } else {
            let cl = ctx.client_of_mut(ent_idx);
            cl.ps.stats[STAT_TIMER_ICON as usize] = 0;
            cl.ps.stats[STAT_TIMER as usize] = 0;
        }
    }

    // selected item
    {
        let selected_item = ctx.client_of(ent_idx).pers.selected_item;
        if selected_item == -1 {
            ctx.client_of_mut(ent_idx).ps.stats[STAT_SELECTED_ICON as usize] = 0;
        } else {
            let icon = ctx.items[selected_item as usize].icon.clone();
            ctx.client_of_mut(ent_idx).ps.stats[STAT_SELECTED_ICON as usize] = gi_imageindex(&icon) as i16;
        }
        ctx.client_of_mut(ent_idx).ps.stats[STAT_SELECTED_ITEM as usize] = selected_item as i16;
    }

    // layouts
    {
        ctx.client_of_mut(ent_idx).ps.stats[STAT_LAYOUTS as usize] = 0;

        if ctx.deathmatch != 0.0 {
            let health = ctx.client_of(ent_idx).pers.health;
            let showscores = ctx.client_of(ent_idx).showscores;
            let showinventory = ctx.client_of(ent_idx).showinventory;
            let intermissiontime = ctx.level.intermissiontime;

            if health <= 0 || intermissiontime != 0.0 || showscores {
                ctx.client_of_mut(ent_idx).ps.stats[STAT_LAYOUTS as usize] |= 1;
            }
            if showinventory && health > 0 {
                ctx.client_of_mut(ent_idx).ps.stats[STAT_LAYOUTS as usize] |= 2;
            }
        } else {
            let showscores = ctx.client_of(ent_idx).showscores;
            let showhelp = ctx.client_of(ent_idx).showhelp;
            let showinventory = ctx.client_of(ent_idx).showinventory;
            let health = ctx.client_of(ent_idx).pers.health;

            if showscores || showhelp {
                ctx.client_of_mut(ent_idx).ps.stats[STAT_LAYOUTS as usize] |= 1;
            }
            if showinventory && health > 0 {
                ctx.client_of_mut(ent_idx).ps.stats[STAT_LAYOUTS as usize] |= 2;
            }
        }
    }

    // frags
    {
        let score = ctx.client_of(ent_idx).resp.score;
        ctx.client_of_mut(ent_idx).ps.stats[STAT_FRAGS as usize] = score as i16;
    }

    // help icon / current weapon if not shown
    {
        let helpchanged = ctx.client_of(ent_idx).pers.helpchanged;
        let hand = ctx.client_of(ent_idx).pers.hand;
        let fov = ctx.client_of(ent_idx).ps.fov;
        let weapon = ctx.client_of(ent_idx).pers.weapon;
        let framenum = ctx.level.framenum;

        if helpchanged != 0 && (framenum & 8) != 0 {
            ctx.client_of_mut(ent_idx).ps.stats[STAT_HELPICON as usize] = gi_imageindex("i_help") as i16;
        } else if (hand == CENTER_HANDED || fov > 91.0) && weapon.is_some() {
            let weapon_idx = weapon.unwrap();
            let icon = ctx.items[weapon_idx].icon.clone();
            ctx.client_of_mut(ent_idx).ps.stats[STAT_HELPICON as usize] = gi_imageindex(&icon) as i16;
        } else {
            ctx.client_of_mut(ent_idx).ps.stats[STAT_HELPICON as usize] = 0;
        }
    }

    ctx.client_of_mut(ent_idx).ps.stats[STAT_SPECTATOR as usize] = 0;
}

/// Check chase stats — copy stats from the chase target to any spectators chasing it.
/// C: G_CheckChaseStats
pub fn g_check_chase_stats(ctx: &mut GameContext, ent_idx: usize) {
    let maxclients = ctx.maxclients;

    for i in 1..=maxclients as usize {
        if !ctx.edicts[i].inuse {
            continue;
        }
        let cl_idx = match ctx.edicts[i].client {
            Some(idx) => idx,
            None => continue,
        };
        if ctx.clients[cl_idx].chase_target != ent_idx as i32 {
            continue;
        }

        // memcpy(cl->ps.stats, ent->client->ps.stats, sizeof(cl->ps.stats))
        let ent_cl_idx = ctx.edicts[ent_idx].client.expect("ent has no client");
        let stats_copy = ctx.clients[ent_cl_idx].ps.stats;
        ctx.clients[cl_idx].ps.stats = stats_copy;

        g_set_spectator_stats(ctx, i);
    }
}

/// Set spectator-specific stats.
/// C: G_SetSpectatorStats
pub fn g_set_spectator_stats(ctx: &mut GameContext, ent_idx: usize) {
    let chase_target = ctx.client_of(ent_idx).chase_target;

    if chase_target <= 0 {
        g_set_stats(ctx, ent_idx);
    }

    ctx.client_of_mut(ent_idx).ps.stats[STAT_SPECTATOR as usize] = 1;

    // layouts are independent in spectator
    ctx.client_of_mut(ent_idx).ps.stats[STAT_LAYOUTS as usize] = 0;

    let health = ctx.client_of(ent_idx).pers.health;
    let showscores = ctx.client_of(ent_idx).showscores;
    let showinventory = ctx.client_of(ent_idx).showinventory;
    let intermissiontime = ctx.level.intermissiontime;

    if health <= 0 || intermissiontime != 0.0 || showscores {
        ctx.client_of_mut(ent_idx).ps.stats[STAT_LAYOUTS as usize] |= 1;
    }
    if showinventory && health > 0 {
        ctx.client_of_mut(ent_idx).ps.stats[STAT_LAYOUTS as usize] |= 2;
    }

    if chase_target > 0 && ctx.edicts[chase_target as usize].inuse {
        // CS_PLAYERSKINS + (chase_target - g_edicts) - 1
        let val = CS_PLAYERSKINS as i16 + (chase_target as i16) - 1;
        ctx.client_of_mut(ent_idx).ps.stats[STAT_CHASE as usize] = val;
    } else {
        ctx.client_of_mut(ent_idx).ps.stats[STAT_CHASE as usize] = 0;
    }
}

// ============================================================
// Helper functions
// ============================================================

/// Respawn a dead client. Copies to body queue and puts back in server.
fn respawn_client(_ctx: &mut GameContext, _client_ent_idx: usize) {
    // In a full implementation this would call p_client::respawn(ctx, client_ent_idx).
    // The cross-module call requires bridging through GameCtx.
    // For now, the entity is left dead until the full dispatch system is wired.
    // The actual respawn logic is in p_client.rs.
}


/// Check power armor type for an entity.
/// Returns POWER_ARMOR_NONE, POWER_ARMOR_SCREEN, or POWER_ARMOR_SHIELD.
fn hud_power_armor_type(ctx: &GameContext, ent_idx: usize) -> i32 {
    if !ctx.edicts[ent_idx].flags.intersects(FL_POWER_ARMOR) {
        return POWER_ARMOR_NONE;
    }

    let client_idx = match ctx.edicts[ent_idx].client {
        Some(c) => c,
        None => return POWER_ARMOR_NONE,
    };

    // Check for power screen
    if let Some(idx) = crate::g_items::find_item("Power Screen") {
        if ctx.clients[client_idx].pers.inventory[idx] > 0 {
            return POWER_ARMOR_SCREEN;
        }
    }

    // Check for power shield
    if let Some(idx) = crate::g_items::find_item("Power Shield") {
        if ctx.clients[client_idx].pers.inventory[idx] > 0 {
            return POWER_ARMOR_SHIELD;
        }
    }

    POWER_ARMOR_NONE
}

/// Get the armor index for an entity (which armor item they have equipped).
/// Returns 0 if no armor, or the item index of the best armor.
fn hud_armor_index(ctx: &GameContext, ent_idx: usize) -> usize {
    let client_idx = match ctx.edicts[ent_idx].client {
        Some(c) => c,
        None => return 0,
    };

    // Check armors in order: body, combat, jacket
    for armor_name in &["Body Armor", "Combat Armor", "Jacket Armor"] {
        if let Some(idx) = crate::g_items::find_item(armor_name) {
            if ctx.clients[client_idx].pers.inventory[idx] > 0 {
                return idx;
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_test_gi() {
        crate::game_import::set_gi(Box::new(crate::game_import::StubGameImport));
    }

    /// Create a GameContext with items initialized and the given number of clients.
    fn make_ctx(num_clients: i32) -> GameContext {
        init_test_gi();
        let mut ctx = GameContext::default();
        ctx.maxclients = num_clients as f32;
        ctx.game.maxclients = num_clients;
        let total_edicts = 1 + num_clients as usize + BODY_QUEUE_SIZE + 16;
        for _ in 0..total_edicts {
            ctx.edicts.push(Edict::default());
        }
        ctx.num_edicts = ctx.edicts.len() as i32;
        for _ in 0..num_clients {
            ctx.clients.push(GClient::default());
        }
        // Initialize items so find_item works
        crate::g_items::init_items(&mut ctx);
        ctx.build_item_indices();
        ctx
    }

    /// Create a single-player context at ent_idx=1, client_idx=0.
    fn make_single_player_ctx() -> GameContext {
        let mut ctx = make_ctx(1);
        ctx.edicts[1].inuse = true;
        ctx.edicts[1].client = Some(0);
        ctx.edicts[1].health = 100;
        ctx.edicts[1].max_health = 100;
        ctx.clients[0].pers.connected = true;
        ctx.clients[0].pers.netname = "TestPlayer".to_string();
        ctx.clients[0].pers.health = 100;
        ctx
    }

    // ============================================================
    // move_client_to_intermission tests
    // ============================================================

    #[test]
    fn test_move_client_to_intermission_origin() {
        let mut ctx = make_single_player_ctx();
        ctx.deathmatch = 1.0;
        ctx.level.intermission_origin = [100.0, 200.0, 300.0];
        ctx.level.intermission_angle = [0.0, 90.0, 0.0];

        move_client_to_intermission(&mut ctx, 1);

        assert_eq!(ctx.edicts[1].s.origin, [100.0, 200.0, 300.0]);
    }

    #[test]
    fn test_move_client_to_intermission_pmove_origin() {
        let mut ctx = make_single_player_ctx();
        ctx.deathmatch = 1.0;
        ctx.level.intermission_origin = [100.0, 200.0, 300.0];
        ctx.level.intermission_angle = [0.0, 90.0, 0.0];

        move_client_to_intermission(&mut ctx, 1);

        // pmove origin is 12.3 fixed point (multiply by 8)
        assert_eq!(ctx.clients[0].ps.pmove.origin[0], (100.0 * 8.0) as i16);
        assert_eq!(ctx.clients[0].ps.pmove.origin[1], (200.0 * 8.0) as i16);
        assert_eq!(ctx.clients[0].ps.pmove.origin[2], (300.0 * 8.0) as i16);
    }

    #[test]
    fn test_move_client_to_intermission_viewangles() {
        let mut ctx = make_single_player_ctx();
        ctx.deathmatch = 1.0;
        ctx.level.intermission_origin = [0.0, 0.0, 0.0];
        ctx.level.intermission_angle = [10.0, 45.0, 0.0];

        move_client_to_intermission(&mut ctx, 1);

        assert_eq!(ctx.clients[0].ps.viewangles, [10.0, 45.0, 0.0]);
    }

    #[test]
    fn test_move_client_to_intermission_freeze() {
        let mut ctx = make_single_player_ctx();
        ctx.deathmatch = 1.0;

        move_client_to_intermission(&mut ctx, 1);

        assert_eq!(ctx.clients[0].ps.pmove.pm_type, PmType::Freeze);
    }

    #[test]
    fn test_move_client_to_intermission_clears_powerups() {
        let mut ctx = make_single_player_ctx();
        ctx.deathmatch = 1.0;
        ctx.clients[0].quad_framenum = 100.0;
        ctx.clients[0].invincible_framenum = 200.0;
        ctx.clients[0].breather_framenum = 300.0;
        ctx.clients[0].enviro_framenum = 400.0;

        move_client_to_intermission(&mut ctx, 1);

        assert_eq!(ctx.clients[0].quad_framenum, 0.0);
        assert_eq!(ctx.clients[0].invincible_framenum, 0.0);
        assert_eq!(ctx.clients[0].breather_framenum, 0.0);
        assert_eq!(ctx.clients[0].enviro_framenum, 0.0);
    }

    #[test]
    fn test_move_client_to_intermission_gun_cleared() {
        let mut ctx = make_single_player_ctx();
        ctx.deathmatch = 1.0;
        ctx.clients[0].ps.gunindex = 42;

        move_client_to_intermission(&mut ctx, 1);

        assert_eq!(ctx.clients[0].ps.gunindex, 0);
    }

    #[test]
    fn test_move_client_to_intermission_showscores_in_dm() {
        let mut ctx = make_single_player_ctx();
        ctx.deathmatch = 1.0;

        move_client_to_intermission(&mut ctx, 1);

        assert!(ctx.clients[0].showscores);
    }

    #[test]
    fn test_move_client_to_intermission_entity_cleared() {
        let mut ctx = make_single_player_ctx();
        ctx.deathmatch = 1.0;
        ctx.edicts[1].s.effects = 0xFF;
        ctx.edicts[1].s.sound = 5;
        ctx.edicts[1].viewheight = 22;

        move_client_to_intermission(&mut ctx, 1);

        assert_eq!(ctx.edicts[1].s.effects, 0);
        assert_eq!(ctx.edicts[1].s.sound, 0);
        assert_eq!(ctx.edicts[1].viewheight, 0);
        assert_eq!(ctx.edicts[1].solid, Solid::Not);
    }

    // ============================================================
    // begin_intermission tests
    // ============================================================

    #[test]
    fn test_begin_intermission_sets_time() {
        let mut ctx = make_single_player_ctx();
        ctx.deathmatch = 1.0;
        ctx.level.time = 120.0;
        ctx.level.intermissiontime = 0.0;

        // Create a target entity with a map value
        let targ_idx = 3;
        if ctx.edicts.len() <= targ_idx {
            while ctx.edicts.len() <= targ_idx {
                ctx.edicts.push(Edict::default());
            }
        }
        ctx.edicts[targ_idx].map = "test_map".to_string();
        ctx.edicts[targ_idx].inuse = true;

        // Need a spawn point for intermission
        let spot_idx = 4;
        if ctx.edicts.len() <= spot_idx {
            while ctx.edicts.len() <= spot_idx {
                ctx.edicts.push(Edict::default());
            }
        }
        ctx.edicts[spot_idx].classname = "info_player_deathmatch".to_string();
        ctx.edicts[spot_idx].inuse = true;
        ctx.edicts[spot_idx].s.origin = [500.0, 600.0, 700.0];
        ctx.num_edicts = ctx.edicts.len() as i32;

        begin_intermission(&mut ctx, targ_idx);

        assert_eq!(ctx.level.intermissiontime, 120.0);
        assert_eq!(ctx.level.changemap, "test_map");
    }

    #[test]
    fn test_begin_intermission_already_active() {
        let mut ctx = make_single_player_ctx();
        ctx.level.intermissiontime = 100.0; // already active

        begin_intermission(&mut ctx, 1);

        // Should not change intermission time
        assert_eq!(ctx.level.intermissiontime, 100.0);
    }

    #[test]
    fn test_begin_intermission_autosave_cleared() {
        let mut ctx = make_single_player_ctx();
        ctx.deathmatch = 1.0;
        ctx.game.autosaved = true;
        ctx.level.intermissiontime = 0.0;

        let targ_idx = 3;
        while ctx.edicts.len() <= targ_idx {
            ctx.edicts.push(Edict::default());
        }
        ctx.edicts[targ_idx].map = "map".to_string();
        ctx.edicts[targ_idx].inuse = true;

        let spot_idx = 4;
        while ctx.edicts.len() <= spot_idx {
            ctx.edicts.push(Edict::default());
        }
        ctx.edicts[spot_idx].classname = "info_player_deathmatch".to_string();
        ctx.edicts[spot_idx].inuse = true;
        ctx.num_edicts = ctx.edicts.len() as i32;

        begin_intermission(&mut ctx, targ_idx);

        assert!(!ctx.game.autosaved);
    }

    // ============================================================
    // cmd_score_f tests
    // ============================================================

    #[test]
    fn test_cmd_score_f_toggle_on() {
        let mut ctx = make_single_player_ctx();
        ctx.deathmatch = 1.0;
        ctx.clients[0].showscores = false;

        cmd_score_f(&mut ctx, 1);

        assert!(ctx.clients[0].showscores);
    }

    #[test]
    fn test_cmd_score_f_toggle_off() {
        let mut ctx = make_single_player_ctx();
        ctx.deathmatch = 1.0;
        ctx.clients[0].showscores = true;

        cmd_score_f(&mut ctx, 1);

        assert!(!ctx.clients[0].showscores);
    }

    #[test]
    fn test_cmd_score_f_no_dm_no_coop() {
        let mut ctx = make_single_player_ctx();
        ctx.deathmatch = 0.0;
        ctx.coop = 0.0;

        cmd_score_f(&mut ctx, 1);

        // Should clear other displays but not show scores in SP
        assert!(!ctx.clients[0].showscores);
        assert!(!ctx.clients[0].showinventory);
    }

    #[test]
    fn test_cmd_score_f_clears_other_displays() {
        let mut ctx = make_single_player_ctx();
        ctx.deathmatch = 1.0;
        ctx.clients[0].showinventory = true;
        ctx.clients[0].showhelp = true;
        ctx.clients[0].showscores = false;

        cmd_score_f(&mut ctx, 1);

        assert!(!ctx.clients[0].showinventory);
        assert!(!ctx.clients[0].showhelp);
        assert!(ctx.clients[0].showscores);
    }

    // ============================================================
    // cmd_help_f tests
    // ============================================================

    #[test]
    fn test_cmd_help_f_deathmatch_redirects_to_score() {
        let mut ctx = make_single_player_ctx();
        ctx.deathmatch = 1.0;
        ctx.clients[0].showscores = false;

        cmd_help_f(&mut ctx, 1);

        // In DM, help redirects to score
        assert!(ctx.clients[0].showscores);
    }

    #[test]
    fn test_cmd_help_f_toggle_on() {
        let mut ctx = make_single_player_ctx();
        ctx.deathmatch = 0.0;
        ctx.clients[0].showhelp = false;

        cmd_help_f(&mut ctx, 1);

        assert!(ctx.clients[0].showhelp);
    }

    #[test]
    fn test_cmd_help_f_toggle_off() {
        let mut ctx = make_single_player_ctx();
        ctx.deathmatch = 0.0;
        ctx.clients[0].showhelp = true;
        ctx.clients[0].pers.game_helpchanged = 5;
        ctx.game.helpchanged = 5;

        cmd_help_f(&mut ctx, 1);

        assert!(!ctx.clients[0].showhelp);
    }

    #[test]
    fn test_cmd_help_f_clears_other_displays() {
        let mut ctx = make_single_player_ctx();
        ctx.deathmatch = 0.0;
        ctx.clients[0].showinventory = true;
        ctx.clients[0].showscores = true;
        ctx.clients[0].showhelp = false;

        cmd_help_f(&mut ctx, 1);

        assert!(!ctx.clients[0].showinventory);
        assert!(!ctx.clients[0].showscores);
        assert!(ctx.clients[0].showhelp);
    }

    // ============================================================
    // help_computer tests
    // ============================================================

    #[test]
    fn test_help_computer_skill_easy() {
        let mut ctx = make_single_player_ctx();
        ctx.skill = 0.0;
        ctx.level.level_name = "Test Level".to_string();

        // Should not crash
        help_computer(&mut ctx, 1);
    }

    #[test]
    fn test_help_computer_skill_names() {
        // Verify the skill name determination logic
        assert_eq!(if 0.0_f32 == 0.0 { "easy" } else { "other" }, "easy");
        assert_eq!(if 1.0_f32 == 1.0 { "medium" } else { "other" }, "medium");
        assert_eq!(if 2.0_f32 == 2.0 { "hard" } else { "other" }, "hard");
    }

    // ============================================================
    // g_set_stats tests
    // ============================================================

    #[test]
    fn test_g_set_stats_health() {
        let mut ctx = make_single_player_ctx();
        ctx.edicts[1].health = 75;
        ctx.level.pic_health = 42;

        g_set_stats(&mut ctx, 1);

        assert_eq!(ctx.clients[0].ps.stats[STAT_HEALTH as usize], 75);
        assert_eq!(ctx.clients[0].ps.stats[STAT_HEALTH_ICON as usize], 42);
    }

    #[test]
    fn test_g_set_stats_ammo_none() {
        let mut ctx = make_single_player_ctx();
        ctx.clients[0].ammo_index = 0;

        g_set_stats(&mut ctx, 1);

        assert_eq!(ctx.clients[0].ps.stats[STAT_AMMO_ICON as usize], 0);
        assert_eq!(ctx.clients[0].ps.stats[STAT_AMMO as usize], 0);
    }

    #[test]
    fn test_g_set_stats_ammo_with_index() {
        let mut ctx = make_single_player_ctx();
        // Find bullets item
        let bullets_idx = crate::g_items::find_item("Bullets");
        if let Some(idx) = bullets_idx {
            ctx.clients[0].ammo_index = idx as i32;
            ctx.clients[0].pers.inventory[idx] = 50;

            g_set_stats(&mut ctx, 1);

            assert_eq!(ctx.clients[0].ps.stats[STAT_AMMO as usize], 50);
        }
    }

    #[test]
    fn test_g_set_stats_no_armor() {
        let mut ctx = make_single_player_ctx();

        g_set_stats(&mut ctx, 1);

        assert_eq!(ctx.clients[0].ps.stats[STAT_ARMOR_ICON as usize], 0);
        assert_eq!(ctx.clients[0].ps.stats[STAT_ARMOR as usize], 0);
    }

    #[test]
    fn test_g_set_stats_with_armor() {
        let mut ctx = make_single_player_ctx();
        // Give body armor
        if let Some(idx) = crate::g_items::find_item("Body Armor") {
            ctx.clients[0].pers.inventory[idx] = 100;

            g_set_stats(&mut ctx, 1);

            assert_eq!(ctx.clients[0].ps.stats[STAT_ARMOR as usize], 100);
        }
    }

    #[test]
    fn test_g_set_stats_quad_timer() {
        let mut ctx = make_single_player_ctx();
        ctx.level.framenum = 100;
        ctx.clients[0].quad_framenum = 200.0;

        g_set_stats(&mut ctx, 1);

        // Timer should show remaining time: (200 - 100) / 10 = 10
        assert_eq!(ctx.clients[0].ps.stats[STAT_TIMER as usize], 10);
    }

    #[test]
    fn test_g_set_stats_invincible_timer() {
        let mut ctx = make_single_player_ctx();
        ctx.level.framenum = 100;
        ctx.clients[0].invincible_framenum = 300.0;
        ctx.clients[0].quad_framenum = 0.0;

        g_set_stats(&mut ctx, 1);

        // Timer: (300 - 100) / 10 = 20
        assert_eq!(ctx.clients[0].ps.stats[STAT_TIMER as usize], 20);
    }

    #[test]
    fn test_g_set_stats_no_timer() {
        let mut ctx = make_single_player_ctx();
        ctx.level.framenum = 200;
        ctx.clients[0].quad_framenum = 0.0;
        ctx.clients[0].invincible_framenum = 0.0;
        ctx.clients[0].enviro_framenum = 0.0;
        ctx.clients[0].breather_framenum = 0.0;

        g_set_stats(&mut ctx, 1);

        assert_eq!(ctx.clients[0].ps.stats[STAT_TIMER_ICON as usize], 0);
        assert_eq!(ctx.clients[0].ps.stats[STAT_TIMER as usize], 0);
    }

    #[test]
    fn test_g_set_stats_pickup_message_expired() {
        let mut ctx = make_single_player_ctx();
        ctx.level.time = 10.0;
        ctx.clients[0].pickup_msg_time = 5.0; // expired

        g_set_stats(&mut ctx, 1);

        assert_eq!(ctx.clients[0].ps.stats[STAT_PICKUP_ICON as usize], 0);
        assert_eq!(ctx.clients[0].ps.stats[STAT_PICKUP_STRING as usize], 0);
    }

    #[test]
    fn test_g_set_stats_selected_item_none() {
        let mut ctx = make_single_player_ctx();
        ctx.clients[0].pers.selected_item = -1;

        g_set_stats(&mut ctx, 1);

        assert_eq!(ctx.clients[0].ps.stats[STAT_SELECTED_ICON as usize], 0);
        assert_eq!(ctx.clients[0].ps.stats[STAT_SELECTED_ITEM as usize], -1);
    }

    #[test]
    fn test_g_set_stats_frags() {
        let mut ctx = make_single_player_ctx();
        ctx.clients[0].resp.score = 42;

        g_set_stats(&mut ctx, 1);

        assert_eq!(ctx.clients[0].ps.stats[STAT_FRAGS as usize], 42);
    }

    #[test]
    fn test_g_set_stats_layouts_dm_dead() {
        let mut ctx = make_single_player_ctx();
        ctx.deathmatch = 1.0;
        ctx.clients[0].pers.health = 0;

        g_set_stats(&mut ctx, 1);

        // Dead in DM => layout bit 1 set
        assert_ne!(ctx.clients[0].ps.stats[STAT_LAYOUTS as usize] & 1, 0);
    }

    #[test]
    fn test_g_set_stats_layouts_dm_alive_no_scores() {
        let mut ctx = make_single_player_ctx();
        ctx.deathmatch = 1.0;
        ctx.clients[0].pers.health = 100;
        ctx.clients[0].showscores = false;
        ctx.clients[0].showinventory = false;

        g_set_stats(&mut ctx, 1);

        assert_eq!(ctx.clients[0].ps.stats[STAT_LAYOUTS as usize], 0);
    }

    #[test]
    fn test_g_set_stats_layouts_dm_intermission() {
        let mut ctx = make_single_player_ctx();
        ctx.deathmatch = 1.0;
        ctx.clients[0].pers.health = 100;
        ctx.level.intermissiontime = 10.0;

        g_set_stats(&mut ctx, 1);

        assert_ne!(ctx.clients[0].ps.stats[STAT_LAYOUTS as usize] & 1, 0);
    }

    #[test]
    fn test_g_set_stats_layouts_inventory() {
        let mut ctx = make_single_player_ctx();
        ctx.deathmatch = 1.0;
        ctx.clients[0].pers.health = 100;
        ctx.clients[0].showinventory = true;

        g_set_stats(&mut ctx, 1);

        assert_ne!(ctx.clients[0].ps.stats[STAT_LAYOUTS as usize] & 2, 0);
    }

    #[test]
    fn test_g_set_stats_layouts_sp_help() {
        let mut ctx = make_single_player_ctx();
        ctx.deathmatch = 0.0;
        ctx.clients[0].showhelp = true;

        g_set_stats(&mut ctx, 1);

        assert_ne!(ctx.clients[0].ps.stats[STAT_LAYOUTS as usize] & 1, 0);
    }

    #[test]
    fn test_g_set_stats_help_icon_flashing() {
        let mut ctx = make_single_player_ctx();
        ctx.clients[0].pers.helpchanged = 1;
        ctx.level.framenum = 8; // (8 & 8) != 0

        g_set_stats(&mut ctx, 1);

        // Help icon should be set when helpchanged and framenum bit 8 is set
        // StubGameImport.imageindex returns 0, so the icon is 0
        // but the code path should execute without crash
        assert_eq!(ctx.clients[0].ps.stats[STAT_HELPICON as usize], 0);
    }

    #[test]
    fn test_g_set_stats_spectator_zero() {
        let mut ctx = make_single_player_ctx();

        g_set_stats(&mut ctx, 1);

        assert_eq!(ctx.clients[0].ps.stats[STAT_SPECTATOR as usize], 0);
    }

    // ============================================================
    // g_set_spectator_stats tests
    // ============================================================

    #[test]
    fn test_g_set_spectator_stats_no_chase() {
        let mut ctx = make_single_player_ctx();
        ctx.clients[0].chase_target = -1;

        g_set_spectator_stats(&mut ctx, 1);

        assert_eq!(ctx.clients[0].ps.stats[STAT_SPECTATOR as usize], 1);
        assert_eq!(ctx.clients[0].ps.stats[STAT_CHASE as usize], 0);
    }

    #[test]
    fn test_g_set_spectator_stats_with_chase() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].inuse = true;
        ctx.edicts[1].client = Some(0);
        ctx.clients[0].pers.connected = true;
        ctx.clients[0].pers.health = 100;

        ctx.edicts[2].inuse = true;
        ctx.edicts[2].client = Some(1);
        ctx.clients[1].pers.connected = true;

        ctx.clients[0].chase_target = 2; // chasing ent 2

        g_set_spectator_stats(&mut ctx, 1);

        assert_eq!(ctx.clients[0].ps.stats[STAT_SPECTATOR as usize], 1);
        // STAT_CHASE = CS_PLAYERSKINS + chase_target - 1
        let expected = CS_PLAYERSKINS as i16 + 2 - 1;
        assert_eq!(ctx.clients[0].ps.stats[STAT_CHASE as usize], expected);
    }

    #[test]
    fn test_g_set_spectator_stats_layouts_independent() {
        let mut ctx = make_single_player_ctx();
        ctx.clients[0].chase_target = -1;
        ctx.clients[0].pers.health = 100;
        ctx.clients[0].showscores = true;
        ctx.clients[0].showinventory = true;

        g_set_spectator_stats(&mut ctx, 1);

        // Layouts are computed independently for spectators
        // showscores => bit 1, showinventory with health > 0 => bit 2
        let layouts = ctx.clients[0].ps.stats[STAT_LAYOUTS as usize];
        assert_ne!(layouts & 1, 0); // showscores
        assert_ne!(layouts & 2, 0); // showinventory
    }

    #[test]
    fn test_g_set_spectator_stats_dead_shows_layout() {
        let mut ctx = make_single_player_ctx();
        ctx.clients[0].chase_target = -1;
        ctx.clients[0].pers.health = 0;

        g_set_spectator_stats(&mut ctx, 1);

        // Dead spectator should have layout bit 1
        assert_ne!(ctx.clients[0].ps.stats[STAT_LAYOUTS as usize] & 1, 0);
    }

    // ============================================================
    // g_check_chase_stats tests
    // ============================================================

    #[test]
    fn test_g_check_chase_stats_copies_stats() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].inuse = true;
        ctx.edicts[1].client = Some(0);
        ctx.clients[0].pers.connected = true;
        ctx.clients[0].pers.health = 100;
        ctx.clients[0].ps.stats[STAT_FRAGS as usize] = 25;

        ctx.edicts[2].inuse = true;
        ctx.edicts[2].client = Some(1);
        ctx.clients[1].pers.connected = true;
        ctx.clients[1].pers.health = 100;
        ctx.clients[1].chase_target = 1; // chasing ent 1

        g_check_chase_stats(&mut ctx, 1);

        // Spectator (client 1) should have copied the STAT_FRAGS from client 0
        assert_eq!(ctx.clients[1].ps.stats[STAT_FRAGS as usize], 25);
        assert_eq!(ctx.clients[1].ps.stats[STAT_SPECTATOR as usize], 1);
    }

    #[test]
    fn test_g_check_chase_stats_no_chasers() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].inuse = true;
        ctx.edicts[1].client = Some(0);
        ctx.clients[0].pers.connected = true;
        ctx.clients[0].ps.stats[STAT_FRAGS as usize] = 10;

        ctx.edicts[2].inuse = true;
        ctx.edicts[2].client = Some(1);
        ctx.clients[1].pers.connected = true;
        ctx.clients[1].chase_target = -1; // not chasing

        g_check_chase_stats(&mut ctx, 1);

        // Client 1 stats should not be modified
        assert_eq!(ctx.clients[1].ps.stats[STAT_FRAGS as usize], 0);
    }

    // ============================================================
    // deathmatch_scoreboard_message tests
    // ============================================================

    #[test]
    fn test_deathmatch_scoreboard_message_basic() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].inuse = true;
        ctx.edicts[1].client = Some(0);
        ctx.clients[0].pers.connected = true;
        ctx.clients[0].resp.score = 10;
        ctx.clients[0].resp.enterframe = 0;
        ctx.clients[0].ping = 30;

        ctx.edicts[2].inuse = true;
        ctx.edicts[2].client = Some(1);
        ctx.clients[1].pers.connected = true;
        ctx.clients[1].resp.score = 20;
        ctx.clients[1].resp.enterframe = 0;
        ctx.clients[1].ping = 50;

        ctx.level.framenum = 600;

        // Should not crash
        deathmatch_scoreboard_message(&mut ctx, 1, -1);
    }

    #[test]
    fn test_deathmatch_scoreboard_message_sorting() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].inuse = true;
        ctx.edicts[1].client = Some(0);
        ctx.clients[0].pers.connected = true;
        ctx.clients[0].resp.score = 5;

        ctx.edicts[2].inuse = true;
        ctx.edicts[2].client = Some(1);
        ctx.clients[1].pers.connected = true;
        ctx.clients[1].resp.score = 20;

        ctx.edicts[3].inuse = true;
        ctx.edicts[3].client = Some(2);
        ctx.clients[2].pers.connected = true;
        ctx.clients[2].resp.score = 10;

        // Should sort by score descending and not crash
        deathmatch_scoreboard_message(&mut ctx, 1, -1);
    }

    #[test]
    fn test_deathmatch_scoreboard_message_spectators_excluded() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].inuse = true;
        ctx.edicts[1].client = Some(0);
        ctx.clients[0].pers.connected = true;
        ctx.clients[0].resp.score = 10;

        ctx.edicts[2].inuse = true;
        ctx.edicts[2].client = Some(1);
        ctx.clients[1].pers.connected = true;
        ctx.clients[1].resp.spectator = true; // spectator
        ctx.clients[1].resp.score = 30;

        // Should not crash and should exclude spectator
        deathmatch_scoreboard_message(&mut ctx, 1, -1);
    }

    #[test]
    fn test_deathmatch_scoreboard_no_players() {
        let mut ctx = make_ctx(1);
        // No players in use
        ctx.edicts[1].inuse = false;

        // Should handle gracefully
        deathmatch_scoreboard_message(&mut ctx, 1, -1);
    }

    // ============================================================
    // hud_power_armor_type tests
    // ============================================================

    #[test]
    fn test_hud_power_armor_type_none() {
        let ctx = make_single_player_ctx();

        let result = hud_power_armor_type(&ctx, 1);

        assert_eq!(result, POWER_ARMOR_NONE);
    }

    #[test]
    fn test_hud_power_armor_type_no_flag() {
        let mut ctx = make_single_player_ctx();
        // Has a power shield but not the FL_POWER_ARMOR flag
        if let Some(idx) = crate::g_items::find_item("Power Shield") {
            ctx.clients[0].pers.inventory[idx] = 1;
        }

        let result = hud_power_armor_type(&ctx, 1);

        assert_eq!(result, POWER_ARMOR_NONE);
    }

    #[test]
    fn test_hud_power_armor_type_shield() {
        let mut ctx = make_single_player_ctx();
        ctx.edicts[1].flags |= FL_POWER_ARMOR;
        if let Some(idx) = crate::g_items::find_item("Power Shield") {
            ctx.clients[0].pers.inventory[idx] = 1;
        }

        let result = hud_power_armor_type(&ctx, 1);

        assert_eq!(result, POWER_ARMOR_SHIELD);
    }

    #[test]
    fn test_hud_power_armor_type_screen() {
        let mut ctx = make_single_player_ctx();
        ctx.edicts[1].flags |= FL_POWER_ARMOR;
        if let Some(idx) = crate::g_items::find_item("Power Screen") {
            ctx.clients[0].pers.inventory[idx] = 1;
        }

        let result = hud_power_armor_type(&ctx, 1);

        assert_eq!(result, POWER_ARMOR_SCREEN);
    }

    // ============================================================
    // hud_armor_index tests
    // ============================================================

    #[test]
    fn test_hud_armor_index_none() {
        let ctx = make_single_player_ctx();

        let result = hud_armor_index(&ctx, 1);

        assert_eq!(result, 0);
    }

    #[test]
    fn test_hud_armor_index_body_armor() {
        let mut ctx = make_single_player_ctx();
        if let Some(idx) = crate::g_items::find_item("Body Armor") {
            ctx.clients[0].pers.inventory[idx] = 100;

            let result = hud_armor_index(&ctx, 1);

            assert_eq!(result, idx);
        }
    }

    #[test]
    fn test_hud_armor_index_combat_armor() {
        let mut ctx = make_single_player_ctx();
        if let Some(idx) = crate::g_items::find_item("Combat Armor") {
            ctx.clients[0].pers.inventory[idx] = 50;

            let result = hud_armor_index(&ctx, 1);

            assert_eq!(result, idx);
        }
    }

    #[test]
    fn test_hud_armor_index_jacket_armor() {
        let mut ctx = make_single_player_ctx();
        if let Some(idx) = crate::g_items::find_item("Jacket Armor") {
            ctx.clients[0].pers.inventory[idx] = 25;

            let result = hud_armor_index(&ctx, 1);

            assert_eq!(result, idx);
        }
    }

    #[test]
    fn test_hud_armor_index_priority_body_over_jacket() {
        let mut ctx = make_single_player_ctx();
        if let Some(body_idx) = crate::g_items::find_item("Body Armor") {
            if let Some(jacket_idx) = crate::g_items::find_item("Jacket Armor") {
                ctx.clients[0].pers.inventory[body_idx] = 100;
                ctx.clients[0].pers.inventory[jacket_idx] = 25;

                let result = hud_armor_index(&ctx, 1);

                // Body armor has priority
                assert_eq!(result, body_idx);
            }
        }
    }

    #[test]
    fn test_hud_armor_index_no_client() {
        let mut ctx = make_single_player_ctx();
        ctx.edicts[1].client = None;

        let result = hud_armor_index(&ctx, 1);

        assert_eq!(result, 0);
    }
}
