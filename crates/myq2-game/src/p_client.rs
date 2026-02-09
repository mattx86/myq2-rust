// p_client.rs — Player client logic
// Converted from: myq2-original/game/p_client.c

use crate::g_local::*;
use crate::game::*;
use crate::game_import::*;
use crate::g_utils::{g_free_edict, g_spawn};
use myq2_common::q_shared::{
    Vec3, PmType, UserCmd,
    PITCH, YAW, ROLL,
    MASK_PLAYERSOLID, MASK_DEADSOLID,
    PMF_DUCKED, PMF_JUMP_HELD, PMF_TIME_TELEPORT, PMF_NO_PREDICTION,
    BUTTON_ATTACK, BUTTON_ANY,
    DmFlags, DF_SPAWN_FARTHEST, DF_FIXED_FOV, DF_QUAD_DROP, DF_FORCE_RESPAWN,
    PRINT_MEDIUM, PRINT_HIGH,
    MZ_LOGIN, MZ_LOGOUT,
    CHAN_BODY, CHAN_VOICE,
    ATTN_NORM,
    CS_PLAYERSKINS,
    q_streq_nocase,
    vector_subtract, vector_length, vector_copy, vec3_origin,
};

use std::f32::consts::PI;
use crate::m_player_frames::*;

// ============================================================
// Event constants (placeholders if not in q_shared)
// ============================================================

// EV_PLAYER_TELEPORT comes from g_local::* re-export (= 6, from myq2_common::q_shared)

// angle2short / short2angle come from g_local::* (q_shared)


// ============================================================
// Gross, ugly, disgusting hack section
// ============================================================

/// Fix coop spawn spot target names to match nearest single-player spot.
/// Converted from: SP_FixCoopSpots
pub fn sp_fix_coop_spots(ctx: &mut GameContext, self_idx: usize) {
    let mut spot_idx: Option<usize> = None;

    loop {
        spot_idx = crate::g_utils::g_find(ctx, spot_idx.map_or(0, |i| i + 1), "classname", "info_player_start");
        let si = match spot_idx {
            Some(i) => i,
            None => return,
        };

        if ctx.edicts[si].targetname.is_empty() {
            continue;
        }

        let d = vector_subtract(&ctx.edicts[self_idx].s.origin, &ctx.edicts[si].s.origin);
        if vector_length(&d) < 384.0 {
            if ctx.edicts[self_idx].targetname.is_empty()
                || !q_streq_nocase(&ctx.edicts[self_idx].targetname, &ctx.edicts[si].targetname)
            {
                let new_name = ctx.edicts[si].targetname.clone();
                ctx.edicts[self_idx].targetname = new_name;
            }
            return;
        }
    }
}

/// Create coop spawn spots on maps that don't have them.
/// Converted from: SP_CreateCoopSpots
pub fn sp_create_coop_spots(ctx: &mut GameContext, _self_idx: usize) {
    if q_streq_nocase(&ctx.level.mapname, "security") {
        let spot_idx = g_spawn(ctx);
        ctx.edicts[spot_idx].classname = "info_player_coop".to_string();
        ctx.edicts[spot_idx].s.origin[0] = 188.0 - 64.0;
        ctx.edicts[spot_idx].s.origin[1] = -164.0;
        ctx.edicts[spot_idx].s.origin[2] = 80.0;
        ctx.edicts[spot_idx].targetname = "jail3".to_string();
        ctx.edicts[spot_idx].s.angles[1] = 90.0;

        let spot_idx = g_spawn(ctx);
        ctx.edicts[spot_idx].classname = "info_player_coop".to_string();
        ctx.edicts[spot_idx].s.origin[0] = 188.0 + 64.0;
        ctx.edicts[spot_idx].s.origin[1] = -164.0;
        ctx.edicts[spot_idx].s.origin[2] = 80.0;
        ctx.edicts[spot_idx].targetname = "jail3".to_string();
        ctx.edicts[spot_idx].s.angles[1] = 90.0;

        let spot_idx = g_spawn(ctx);
        ctx.edicts[spot_idx].classname = "info_player_coop".to_string();
        ctx.edicts[spot_idx].s.origin[0] = 188.0 + 128.0;
        ctx.edicts[spot_idx].s.origin[1] = -164.0;
        ctx.edicts[spot_idx].s.origin[2] = 80.0;
        ctx.edicts[spot_idx].targetname = "jail3".to_string();
        ctx.edicts[spot_idx].s.angles[1] = 90.0;
    }
}

/// Converted from: SP_info_player_start
pub fn sp_info_player_start(ctx: &mut GameContext, self_idx: usize) {
    if ctx.coop == 0.0 {
        return;
    }
    if q_streq_nocase(&ctx.level.mapname, "security") {
        // invoke one of our gross, ugly, disgusting hacks
        // In C: self->think = SP_CreateCoopSpots;
        // Here we set think_fn and nextthink; dispatch resolves the callback.
        ctx.edicts[self_idx].think_fn = Some(THINK_SP_CREATE_COOP_SPOTS);
        ctx.edicts[self_idx].nextthink = ctx.level.time + FRAMETIME;
    }
}

/// Converted from: SP_info_player_deathmatch
pub fn sp_info_player_deathmatch(ctx: &mut GameContext, self_idx: usize) {
    if ctx.deathmatch == 0.0 {
        g_free_edict(ctx, self_idx);
        return;
    }
    sp_misc_teleporter_dest(ctx, self_idx);
}

/// Converted from: SP_info_player_coop
pub fn sp_info_player_coop(ctx: &mut GameContext, self_idx: usize) {
    if ctx.coop == 0.0 {
        g_free_edict(ctx, self_idx);
        return;
    }

    let maps_needing_fix = [
        "jail2", "jail4", "mine1", "mine2", "mine3", "mine4",
        "lab", "boss1", "fact3", "biggun", "space", "command",
        "power2", "strike",
    ];

    for map in &maps_needing_fix {
        if q_streq_nocase(&ctx.level.mapname, map) {
            ctx.edicts[self_idx].think_fn = Some(THINK_SP_FIX_COOP_SPOTS);
            ctx.edicts[self_idx].nextthink = ctx.level.time + FRAMETIME;
            return;
        }
    }
}

/// Converted from: SP_info_player_intermission
pub fn sp_info_player_intermission() {
    // empty — intentional
}

// ============================================================
// Player pain / gender / obituary
// ============================================================

/// Converted from: player_pain
pub fn player_pain(
    _ctx: &mut GameContext,
    _self_idx: usize,
    _other_idx: usize,
    _kick: f32,
    _damage: i32,
) {
    // player pain is handled at the end of the frame in P_DamageFeedback
}

/// Converted from: IsFemale
pub fn is_female(ctx: &GameContext, ent_idx: usize) -> bool {
    let client_idx = match ctx.edicts[ent_idx].client {
        Some(c) => c,
        None => return false,
    };
    let info = info_value_for_key(&ctx.clients[client_idx].pers.userinfo, "gender");
    info.starts_with('f') || info.starts_with('F')
}

/// Converted from: IsNeutral
pub fn is_neutral(ctx: &GameContext, ent_idx: usize) -> bool {
    let client_idx = match ctx.edicts[ent_idx].client {
        Some(c) => c,
        None => return false,
    };
    let info = info_value_for_key(&ctx.clients[client_idx].pers.userinfo, "gender");
    if info.is_empty() {
        return true;
    }
    let c = info.chars().next().unwrap();
    c != 'f' && c != 'F' && c != 'm' && c != 'M'
}

/// Converted from: ClientObituary
pub fn client_obituary(ctx: &mut GameContext, self_idx: usize, _inflictor_idx: usize, attacker_idx: usize) {
    if ctx.coop != 0.0 && ctx.edicts[attacker_idx].client.is_some() {
        ctx.means_of_death |= MOD_FRIENDLY_FIRE;
    }

    if ctx.deathmatch != 0.0 || ctx.coop != 0.0 {
        let ff = (ctx.means_of_death & MOD_FRIENDLY_FIRE) != 0;
        let mod_val = ctx.means_of_death & !MOD_FRIENDLY_FIRE;
        let mut message: Option<&str> = None;
        let mut message2: &str = "";

        match mod_val {
            MOD_SUICIDE => message = Some("suicides"),
            MOD_FALLING => message = Some("cratered"),
            MOD_CRUSH => message = Some("was squished"),
            MOD_WATER => message = Some("sank like a rock"),
            MOD_SLIME => message = Some("melted"),
            MOD_LAVA => message = Some("does a back flip into the lava"),
            MOD_EXPLOSIVE | MOD_BARREL => message = Some("blew up"),
            MOD_EXIT => message = Some("found a way out"),
            MOD_TARGET_LASER => message = Some("saw the light"),
            MOD_TARGET_BLASTER => message = Some("got blasted"),
            MOD_BOMB | MOD_SPLASH | MOD_TRIGGER_HURT => message = Some("was in the wrong place"),
            _ => {}
        }

        if attacker_idx == self_idx {
            match mod_val {
                MOD_HELD_GRENADE => message = Some("tried to put the pin back in"),
                MOD_HG_SPLASH | MOD_G_SPLASH => {
                    if is_neutral(ctx, self_idx) {
                        message = Some("tripped on its own grenade");
                    } else if is_female(ctx, self_idx) {
                        message = Some("tripped on her own grenade");
                    } else {
                        message = Some("tripped on his own grenade");
                    }
                }
                MOD_R_SPLASH => {
                    if is_neutral(ctx, self_idx) {
                        message = Some("blew itself up");
                    } else if is_female(ctx, self_idx) {
                        message = Some("blew herself up");
                    } else {
                        message = Some("blew himself up");
                    }
                }
                MOD_BFG_BLAST => message = Some("should have used a smaller gun"),
                _ => {
                    if is_neutral(ctx, self_idx) {
                        message = Some("killed itself");
                    } else if is_female(ctx, self_idx) {
                        message = Some("killed herself");
                    } else {
                        message = Some("killed himself");
                    }
                }
            }
        }

        if let Some(msg) = message {
            let self_client = ctx.edicts[self_idx].client.unwrap();
            gi_bprintf(PRINT_MEDIUM, &format!("{} {}.\n", ctx.clients[self_client].pers.netname, msg));
            if ctx.deathmatch != 0.0 {
                ctx.clients[self_client].resp.score -= 1;
            }
            ctx.edicts[self_idx].enemy = -1;
            return;
        }

        ctx.edicts[self_idx].enemy = attacker_idx as i32;

        if attacker_idx != 0 && ctx.edicts[attacker_idx].client.is_some() {
            match mod_val {
                MOD_BLASTER => message = Some("was blasted by"),
                MOD_SHOTGUN => message = Some("was gunned down by"),
                MOD_SSHOTGUN => {
                    message = Some("was blown away by");
                    message2 = "'s super shotgun";
                }
                MOD_MACHINEGUN => message = Some("was machinegunned by"),
                MOD_CHAINGUN => {
                    message = Some("was cut in half by");
                    message2 = "'s chaingun";
                }
                MOD_GRENADE => {
                    message = Some("was popped by");
                    message2 = "'s grenade";
                }
                MOD_G_SPLASH => {
                    message = Some("was shredded by");
                    message2 = "'s shrapnel";
                }
                MOD_ROCKET => {
                    message = Some("ate");
                    message2 = "'s rocket";
                }
                MOD_R_SPLASH => {
                    message = Some("almost dodged");
                    message2 = "'s rocket";
                }
                MOD_HYPERBLASTER => {
                    message = Some("was melted by");
                    message2 = "'s hyperblaster";
                }
                MOD_RAILGUN => message = Some("was railed by"),
                MOD_BFG_LASER => {
                    message = Some("saw the pretty lights from");
                    message2 = "'s BFG";
                }
                MOD_BFG_BLAST => {
                    message = Some("was disintegrated by");
                    message2 = "'s BFG blast";
                }
                MOD_BFG_EFFECT => {
                    message = Some("couldn't hide from");
                    message2 = "'s BFG";
                }
                MOD_HANDGRENADE => {
                    message = Some("caught");
                    message2 = "'s handgrenade";
                }
                MOD_HG_SPLASH => {
                    message = Some("didn't see");
                    message2 = "'s handgrenade";
                }
                MOD_HELD_GRENADE => {
                    message = Some("feels");
                    message2 = "'s pain";
                }
                MOD_TELEFRAG => {
                    message = Some("tried to invade");
                    message2 = "'s personal space";
                }
                _ => {}
            }

            if let Some(msg) = message {
                let self_client = ctx.edicts[self_idx].client.unwrap();
                let attacker_client = ctx.edicts[attacker_idx].client.unwrap();
                gi_bprintf(PRINT_MEDIUM, &format!("{} {} {}{}\n",
                    ctx.clients[self_client].pers.netname,
                    msg,
                    ctx.clients[attacker_client].pers.netname,
                    message2
                ));
                if ctx.deathmatch != 0.0 {
                    if ff {
                        ctx.clients[attacker_client].resp.score -= 1;
                    } else {
                        ctx.clients[attacker_client].resp.score += 1;
                    }
                }
                return;
            }
        }
    }

    let self_client = ctx.edicts[self_idx].client.unwrap();
    gi_bprintf(PRINT_MEDIUM, &format!("{} died.\n", ctx.clients[self_client].pers.netname));
    if ctx.deathmatch != 0.0 {
        ctx.clients[self_client].resp.score -= 1;
    }
}

// ============================================================
// Toss client weapon on death
// ============================================================

/// Converted from: TossClientWeapon
pub fn toss_client_weapon(ctx: &mut GameContext, self_idx: usize) {
    if ctx.deathmatch == 0.0 {
        return;
    }

    let client_idx = ctx.edicts[self_idx].client.unwrap();
    let mut item: Option<usize> = ctx.clients[client_idx].pers.weapon;

    if let Some(item_idx) = item {
        let ammo_index = ctx.clients[client_idx].ammo_index;
        if ammo_index >= 0 && ctx.clients[client_idx].pers.inventory[ammo_index as usize] == 0 {
            item = None;
        }
        if ctx.items[item_idx].pickup_name == "Blaster" {
            item = None;
        }
    }

    let quad = if !DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_QUAD_DROP) {
        false
    } else {
        ctx.clients[client_idx].quad_framenum > (ctx.level.framenum as f32 + 10.0)
    };

    let spread = if item.is_some() && quad { 22.5 } else { 0.0 };

    if let Some(item_idx) = item {
        ctx.clients[client_idx].v_angle[YAW] -= spread;
        let drop_idx = drop_item(ctx, self_idx, item_idx);
        ctx.clients[client_idx].v_angle[YAW] += spread;
        ctx.edicts[drop_idx].spawnflags = DROPPED_PLAYER_ITEM;
    }

    if quad {
        ctx.clients[client_idx].v_angle[YAW] += spread;
        let quad_item = find_item_by_classname(ctx, "item_quad");
        let drop_idx = drop_item(ctx, self_idx, quad_item.unwrap());
        ctx.clients[client_idx].v_angle[YAW] -= spread;
        ctx.edicts[drop_idx].spawnflags |= DROPPED_PLAYER_ITEM;

        ctx.edicts[drop_idx].touch_fn = Some(TOUCH_ITEM);
        ctx.edicts[drop_idx].nextthink = ctx.level.time
            + (ctx.clients[client_idx].quad_framenum - ctx.level.framenum as f32) * FRAMETIME;
        ctx.edicts[drop_idx].think_fn = Some(THINK_G_FREE_EDICT);
    }
}

// ============================================================
// LookAtKiller
// ============================================================

/// Converted from: LookAtKiller
pub fn look_at_killer(ctx: &mut GameContext, self_idx: usize, inflictor_idx: usize, attacker_idx: usize) {
    let world_idx: usize = 0;

    let dir: Vec3;

    if attacker_idx != 0 && attacker_idx != world_idx && attacker_idx != self_idx {
        dir = vector_subtract(&ctx.edicts[attacker_idx].s.origin, &ctx.edicts[self_idx].s.origin);
    } else if inflictor_idx != 0 && inflictor_idx != world_idx && inflictor_idx != self_idx {
        dir = vector_subtract(&ctx.edicts[inflictor_idx].s.origin, &ctx.edicts[self_idx].s.origin);
    } else {
        let client_idx = ctx.edicts[self_idx].client.unwrap();
        ctx.clients[client_idx].killer_yaw = ctx.edicts[self_idx].s.angles[YAW];
        return;
    }

    let client_idx = ctx.edicts[self_idx].client.unwrap();

    if dir[0] != 0.0 {
        ctx.clients[client_idx].killer_yaw = (180.0 / PI) * dir[1].atan2(dir[0]);
    } else {
        ctx.clients[client_idx].killer_yaw = 0.0;
        if dir[1] > 0.0 {
            ctx.clients[client_idx].killer_yaw = 90.0;
        } else if dir[1] < 0.0 {
            ctx.clients[client_idx].killer_yaw = -90.0;
        }
    }
    if ctx.clients[client_idx].killer_yaw < 0.0 {
        ctx.clients[client_idx].killer_yaw += 360.0;
    }
}

// ============================================================
// player_die
// ============================================================


/// Converted from: player_die
pub fn player_die(
    ctx: &mut GameContext,
    self_idx: usize,
    inflictor_idx: usize,
    attacker_idx: usize,
    damage: i32,
    _point: Vec3,
) {
    ctx.edicts[self_idx].avelocity = vec3_origin;

    ctx.edicts[self_idx].takedamage = Damage::Yes as i32;
    ctx.edicts[self_idx].movetype = MoveType::Toss;

    ctx.edicts[self_idx].s.modelindex2 = 0; // remove linked weapon model

    ctx.edicts[self_idx].s.angles[0] = 0.0;
    ctx.edicts[self_idx].s.angles[2] = 0.0;

    ctx.edicts[self_idx].s.sound = 0;
    let client_idx = ctx.edicts[self_idx].client.unwrap();
    ctx.clients[client_idx].weapon_sound = 0;

    ctx.edicts[self_idx].maxs[2] = -8.0;

    ctx.edicts[self_idx].svflags |= SVF_DEADMONSTER;

    if ctx.edicts[self_idx].deadflag == DEAD_NO {
        ctx.clients[client_idx].respawn_time = ctx.level.time + 1.0;
        look_at_killer(ctx, self_idx, inflictor_idx, attacker_idx);
        ctx.clients[client_idx].ps.pmove.pm_type = PmType::Dead;
        client_obituary(ctx, self_idx, inflictor_idx, attacker_idx);
        toss_client_weapon(ctx, self_idx);
        if ctx.deathmatch != 0.0 {
            crate::p_hud::cmd_help_f(ctx,self_idx);
        }

        // clear inventory
        let num_items = ctx.game.num_items as usize;
        for n in 0..num_items {
            if ctx.coop != 0.0 && ctx.items[n].flags.intersects(IT_KEY) {
                let inv_val = ctx.clients[client_idx].pers.inventory[n];
                ctx.clients[client_idx].resp.coop_respawn.inventory[n] = inv_val;
            }
            ctx.clients[client_idx].pers.inventory[n] = 0;
        }
    }

    // remove powerups
    ctx.clients[client_idx].quad_framenum = 0.0;
    ctx.clients[client_idx].invincible_framenum = 0.0;
    ctx.clients[client_idx].breather_framenum = 0.0;
    ctx.clients[client_idx].enviro_framenum = 0.0;
    ctx.edicts[self_idx].flags &= !FL_POWER_ARMOR;

    if ctx.edicts[self_idx].health < -40 {
        // gib
        gi_sound(self_idx as i32, CHAN_BODY, gi_soundindex("misc/udeath.wav"), 1.0, ATTN_NORM as f32, 0.0);
        for _n in 0..4 {
            crate::g_misc::throw_gib(ctx, self_idx, "models/objects/gibs/sm_meat/tris.md2", damage, GIB_ORGANIC);
        }
        crate::g_misc::throw_client_head(ctx, self_idx, damage);

        ctx.edicts[self_idx].takedamage = Damage::No as i32;
    } else {
        // normal death
        if ctx.edicts[self_idx].deadflag == DEAD_NO {
            ctx.death_anim_index = (ctx.death_anim_index + 1) % 3;
            let i = ctx.death_anim_index;

            ctx.clients[client_idx].anim_priority = ANIM_DEATH;
            if (ctx.clients[client_idx].ps.pmove.pm_flags & PMF_DUCKED) != 0 {
                ctx.edicts[self_idx].s.frame = FRAME_CRDEATH1 - 1;
                ctx.clients[client_idx].anim_end = FRAME_CRDEATH5;
            } else {
                match i {
                    0 => {
                        ctx.edicts[self_idx].s.frame = FRAME_DEATH101 - 1;
                        ctx.clients[client_idx].anim_end = FRAME_DEATH106;
                    }
                    1 => {
                        ctx.edicts[self_idx].s.frame = FRAME_DEATH201 - 1;
                        ctx.clients[client_idx].anim_end = FRAME_DEATH206;
                    }
                    2 | _ => {
                        ctx.edicts[self_idx].s.frame = FRAME_DEATH301 - 1;
                        ctx.clients[client_idx].anim_end = FRAME_DEATH308;
                    }
                }
            }
            let death_snd = (rand_val() % 4) + 1;
            gi_sound(self_idx as i32, CHAN_VOICE, gi_soundindex(&format!("*death{}.wav", death_snd)), 1.0, ATTN_NORM as f32, 0.0);
        }
    }

    ctx.edicts[self_idx].deadflag = DEAD_DEAD;

    gi_linkentity(self_idx as i32);
}

// ============================================================
// InitClientPersistant
// ============================================================

/// Converted from: InitClientPersistant
pub fn init_client_persistant(ctx: &mut GameContext, client_idx: usize) {
    ctx.clients[client_idx].pers = ClientPersistant::default();

    let blaster_idx = find_item(ctx, "Blaster");
    if let Some(item_idx) = blaster_idx {
        ctx.clients[client_idx].pers.selected_item = item_idx as i32;
        ctx.clients[client_idx].pers.inventory[item_idx] = 1;
        ctx.clients[client_idx].pers.weapon = Some(item_idx);
    }

    ctx.clients[client_idx].pers.health = 100;
    ctx.clients[client_idx].pers.max_health = 100;

    ctx.clients[client_idx].pers.max_bullets = 200;
    ctx.clients[client_idx].pers.max_shells = 100;
    ctx.clients[client_idx].pers.max_rockets = 50;
    ctx.clients[client_idx].pers.max_grenades = 50;
    ctx.clients[client_idx].pers.max_cells = 200;
    ctx.clients[client_idx].pers.max_slugs = 50;

    ctx.clients[client_idx].pers.connected = true;
}

/// Converted from: InitClientResp
pub fn init_client_resp(ctx: &mut GameContext, client_idx: usize) {
    ctx.clients[client_idx].resp = ClientRespawn::default();
    ctx.clients[client_idx].resp.enterframe = ctx.level.framenum;
    ctx.clients[client_idx].resp.coop_respawn = ctx.clients[client_idx].pers.clone();
}

/// Converted from: SaveClientData
pub fn save_client_data(ctx: &mut GameContext) {
    for i in 0..ctx.game.maxclients as usize {
        let ent_idx = 1 + i;
        if !ctx.edicts[ent_idx].inuse {
            continue;
        }
        ctx.clients[i].pers.health = ctx.edicts[ent_idx].health;
        ctx.clients[i].pers.max_health = ctx.edicts[ent_idx].max_health;
        ctx.clients[i].pers.saved_flags =
            (ctx.edicts[ent_idx].flags & (FL_GODMODE | FL_NOTARGET | FL_POWER_ARMOR)).bits();
        if ctx.coop != 0.0 {
            let ci = ctx.edicts[ent_idx].client.unwrap();
            ctx.clients[i].pers.score = ctx.clients[ci].resp.score;
        }
    }
}

/// Converted from: FetchClientEntData
pub fn fetch_client_ent_data(ctx: &mut GameContext, ent_idx: usize) {
    let client_idx = ctx.edicts[ent_idx].client.unwrap();
    ctx.edicts[ent_idx].health = ctx.clients[client_idx].pers.health;
    ctx.edicts[ent_idx].max_health = ctx.clients[client_idx].pers.max_health;
    ctx.edicts[ent_idx].flags |= EntityFlags::from_bits_truncate(ctx.clients[client_idx].pers.saved_flags);
    if ctx.coop != 0.0 {
        ctx.clients[client_idx].resp.score = ctx.clients[client_idx].pers.score;
    }
}

// ============================================================
// SelectSpawnPoint
// ============================================================

/// Converted from: PlayersRangeFromSpot
pub fn players_range_from_spot(ctx: &GameContext, spot_idx: usize) -> f32 {
    let mut best_distance: f32 = 9999999.0;

    for n in 1..=(ctx.maxclients as usize) {
        if !ctx.edicts[n].inuse {
            continue;
        }
        if ctx.edicts[n].health <= 0 {
            continue;
        }
        let v = vector_subtract(&ctx.edicts[spot_idx].s.origin, &ctx.edicts[n].s.origin);
        let dist = vector_length(&v);
        if dist < best_distance {
            best_distance = dist;
        }
    }

    best_distance
}

/// Converted from: SelectRandomDeathmatchSpawnPoint
pub fn select_random_deathmatch_spawn_point(ctx: &GameContext) -> Option<usize> {
    let mut count: i32 = 0;
    let mut range1: f32 = 99999.0;
    let mut range2: f32 = 99999.0;
    let mut spot1: Option<usize> = None;
    let mut spot2: Option<usize> = None;

    let mut spot: Option<usize> = None;
    loop {
        spot = crate::g_utils::g_find(ctx, spot.map_or(0, |i| i + 1), "classname", "info_player_deathmatch");
        match spot {
            None => break,
            Some(si) => {
                count += 1;
                let range = players_range_from_spot(ctx, si);
                if range < range1 {
                    range1 = range;
                    spot1 = Some(si);
                } else if range < range2 {
                    range2 = range;
                    spot2 = Some(si);
                }
            }
        }
    }

    if count == 0 {
        return None;
    }

    if count <= 2 {
        spot1 = None;
        spot2 = None;
    } else {
        count -= 2;
    }

    let mut selection = rand_val() % count;

    spot = None;
    loop {
        spot = crate::g_utils::g_find(ctx, spot.map_or(0, |i| i + 1), "classname", "info_player_deathmatch");
        if spot == spot1 || spot == spot2 {
            selection += 1;
        }
        if selection == 0 {
            break;
        }
        selection -= 1;
    }

    spot
}

/// Converted from: SelectFarthestDeathmatchSpawnPoint
pub fn select_farthest_deathmatch_spawn_point(ctx: &GameContext) -> Option<usize> {
    let mut bestspot: Option<usize> = None;
    let mut bestdistance: f32 = 0.0;

    let mut spot: Option<usize> = None;
    loop {
        spot = crate::g_utils::g_find(ctx, spot.map_or(0, |i| i + 1), "classname", "info_player_deathmatch");
        match spot {
            None => break,
            Some(si) => {
                let d = players_range_from_spot(ctx, si);
                if d > bestdistance {
                    bestspot = Some(si);
                    bestdistance = d;
                }
            }
        }
    }

    if bestspot.is_some() {
        return bestspot;
    }

    // if there is a player just spawned on each and every start spot
    // we have no choice to turn one into a telefrag meltdown
    crate::g_utils::g_find(ctx, 0, "classname", "info_player_deathmatch")
}

/// Converted from: SelectDeathmatchSpawnPoint
pub fn select_deathmatch_spawn_point(ctx: &GameContext) -> Option<usize> {
    if DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_SPAWN_FARTHEST) {
        select_farthest_deathmatch_spawn_point(ctx)
    } else {
        select_random_deathmatch_spawn_point(ctx)
    }
}

/// Converted from: SelectCoopSpawnPoint
pub fn select_coop_spawn_point(ctx: &GameContext, ent_idx: usize) -> Option<usize> {
    let client_idx = ctx.edicts[ent_idx].client.unwrap();
    // index = ent->client - game.clients
    let mut index = client_idx as i32;

    // player 0 starts in normal player spawn point
    if index == 0 {
        return None;
    }

    let mut spot: Option<usize> = None;
    loop {
        spot = crate::g_utils::g_find(ctx, spot.map_or(0, |i| i + 1), "classname", "info_player_coop");
        match spot {
            None => return None,
            Some(si) => {
                let target = if ctx.edicts[si].targetname.is_empty() {
                    ""
                } else {
                    &ctx.edicts[si].targetname
                };
                if q_streq_nocase(&ctx.game.spawnpoint, target) {
                    index -= 1;
                    if index == 0 {
                        return Some(si);
                    }
                }
            }
        }
    }
}

/// Converted from: SelectSpawnPoint
pub fn select_spawn_point(ctx: &GameContext, ent_idx: usize) -> (Vec3, Vec3) {
    let mut spot: Option<usize> = None;

    if ctx.deathmatch != 0.0 {
        spot = select_deathmatch_spawn_point(ctx);
    } else if ctx.coop != 0.0 {
        spot = select_coop_spawn_point(ctx, ent_idx);
    }

    // find a single player start spot
    if spot.is_none() {
        let mut search: Option<usize> = None;
        loop {
            search = crate::g_utils::g_find(ctx, search.map_or(0, |i| i + 1), "classname", "info_player_start");
            match search {
                None => break,
                Some(si) => {
                    if ctx.game.spawnpoint.is_empty() && ctx.edicts[si].targetname.is_empty() {
                        spot = Some(si);
                        break;
                    }
                    if ctx.game.spawnpoint.is_empty() || ctx.edicts[si].targetname.is_empty() {
                        continue;
                    }
                    if q_streq_nocase(&ctx.game.spawnpoint, &ctx.edicts[si].targetname) {
                        spot = Some(si);
                        break;
                    }
                }
            }
        }

        if spot.is_none() {
            if ctx.game.spawnpoint.is_empty() {
                spot = crate::g_utils::g_find(ctx, 0, "classname", "info_player_start");
            }
            if spot.is_none() {
                panic!("Couldn't find spawn point {}", ctx.game.spawnpoint);
            }
        }
    }

    let si = spot.unwrap();
    let mut origin = vector_copy(&ctx.edicts[si].s.origin);
    origin[2] += 9.0;
    let angles = vector_copy(&ctx.edicts[si].s.angles);

    (origin, angles)
}

// ============================================================
// Body queue
// ============================================================

/// Converted from: InitBodyQue
pub fn init_body_que(ctx: &mut GameContext) {
    ctx.level.body_que = 0;
    for _i in 0..BODY_QUEUE_SIZE {
        let ent_idx = g_spawn(ctx);
        ctx.edicts[ent_idx].classname = "bodyque".to_string();
    }
}

/// Converted from: body_die
pub fn body_die(ctx: &mut GameContext, self_idx: usize, _inflictor_idx: usize, _attacker_idx: usize, damage: i32, _point: Vec3) {
    if ctx.edicts[self_idx].health < -40 {
        gi_sound(self_idx as i32, CHAN_BODY, gi_soundindex("misc/udeath.wav"), 1.0, ATTN_NORM as f32, 0.0);
        for _n in 0..4 {
            crate::g_misc::throw_gib(ctx, self_idx, "models/objects/gibs/sm_meat/tris.md2", damage, GIB_ORGANIC);
        }
        ctx.edicts[self_idx].s.origin[2] -= 48.0;
        crate::g_misc::throw_client_head(ctx, self_idx, damage);
        ctx.edicts[self_idx].takedamage = Damage::No as i32;
    }
}

/// Converted from: CopyToBodyQue
pub fn copy_to_body_que(ctx: &mut GameContext, ent_idx: usize) {
    let body_idx = (ctx.maxclients as usize) + (ctx.level.body_que as usize) + 1;
    ctx.level.body_que = (ctx.level.body_que + 1) % BODY_QUEUE_SIZE as i32;

    gi_unlinkentity(ent_idx as i32);
    gi_unlinkentity(body_idx as i32);

    ctx.edicts[body_idx].s = ctx.edicts[ent_idx].s.clone();
    ctx.edicts[body_idx].s.number = body_idx as i32;

    ctx.edicts[body_idx].svflags = ctx.edicts[ent_idx].svflags;
    ctx.edicts[body_idx].mins = vector_copy(&ctx.edicts[ent_idx].mins);
    ctx.edicts[body_idx].maxs = vector_copy(&ctx.edicts[ent_idx].maxs);
    ctx.edicts[body_idx].absmin = vector_copy(&ctx.edicts[ent_idx].absmin);
    ctx.edicts[body_idx].absmax = vector_copy(&ctx.edicts[ent_idx].absmax);
    ctx.edicts[body_idx].size = vector_copy(&ctx.edicts[ent_idx].size);
    ctx.edicts[body_idx].solid = ctx.edicts[ent_idx].solid;
    ctx.edicts[body_idx].clipmask = ctx.edicts[ent_idx].clipmask;
    ctx.edicts[body_idx].owner = ctx.edicts[ent_idx].owner;
    ctx.edicts[body_idx].movetype = ctx.edicts[ent_idx].movetype;

    ctx.edicts[body_idx].die_fn = Some(DIE_BODY_DIE);
    ctx.edicts[body_idx].takedamage = Damage::Yes as i32;

    gi_linkentity(body_idx as i32);
}

// ============================================================
// Respawn
// ============================================================

/// Converted from: respawn
pub fn respawn(ctx: &mut GameContext, self_idx: usize) {
    if ctx.deathmatch != 0.0 || ctx.coop != 0.0 {
        // spectators don't leave bodies
        if ctx.edicts[self_idx].movetype != MoveType::Noclip {
            copy_to_body_que(ctx, self_idx);
        }
        ctx.edicts[self_idx].svflags &= !SVF_NOCLIENT;
        put_client_in_server(ctx, self_idx);

        // add a teleportation effect
        ctx.edicts[self_idx].s.event = EV_PLAYER_TELEPORT;

        // hold in place briefly
        let ci = ctx.edicts[self_idx].client.unwrap();
        ctx.clients[ci].ps.pmove.pm_flags = PMF_TIME_TELEPORT;
        ctx.clients[ci].ps.pmove.pm_time = 14;
        ctx.clients[ci].respawn_time = ctx.level.time;

        return;
    }

    // restart the entire server
    gi_add_command_string("menu_loadgame\n");
}

/// Converted from: spectator_respawn
pub fn spectator_respawn(ctx: &mut GameContext, ent_idx: usize) {
    let ci = ctx.edicts[ent_idx].client.unwrap();

    if ctx.clients[ci].pers.spectator {
        let value = info_value_for_key(&ctx.clients[ci].pers.userinfo, "spectator");
        if !ctx.spectator_password.is_empty()
            && ctx.spectator_password != "none"
            && ctx.spectator_password != value
        {
            gi_cprintf(ent_idx as i32, PRINT_HIGH, "Spectator password incorrect.\n");
            ctx.clients[ci].pers.spectator = false;
            gi_write_byte(SVC_STUFFTEXT);
            gi_write_string("spectator 0\n");
            gi_unicast(ent_idx as i32, true);
            return;
        }

        // count spectators
        let mut numspec = 0;
        for i in 1..=(ctx.maxclients as usize) {
            if ctx.edicts[i].inuse {
                if let Some(c) = ctx.edicts[i].client {
                    if ctx.clients[c].pers.spectator {
                        numspec += 1;
                    }
                }
            }
        }

        if numspec >= ctx.maxspectators as i32 {
            gi_cprintf(ent_idx as i32, PRINT_HIGH, "Server spectator limit is full.");
            ctx.clients[ci].pers.spectator = false;
            gi_write_byte(SVC_STUFFTEXT);
            gi_write_string("spectator 0\n");
            gi_unicast(ent_idx as i32, true);
            return;
        }
    } else {
        let value = info_value_for_key(&ctx.clients[ci].pers.userinfo, "password");
        if !ctx.password.is_empty()
            && ctx.password != "none"
            && ctx.password != value
        {
            gi_cprintf(ent_idx as i32, PRINT_HIGH, "Password incorrect.\n");
            ctx.clients[ci].pers.spectator = true;
            gi_write_byte(SVC_STUFFTEXT);
            gi_write_string("spectator 1\n");
            gi_unicast(ent_idx as i32, true);
            return;
        }
    }

    // clear client on respawn
    ctx.clients[ci].resp.score = 0;
    ctx.clients[ci].pers.score = 0;

    ctx.edicts[ent_idx].svflags &= !SVF_NOCLIENT;
    put_client_in_server(ctx, ent_idx);

    // add a teleportation effect
    if !ctx.clients[ci].pers.spectator {
        gi_write_byte(SVC_MUZZLEFLASH);
        gi_write_short(ent_idx as i32);
        gi_write_byte(MZ_LOGIN);
        gi_multicast(&ctx.edicts[ent_idx].s.origin, MULTICAST_PVS);

        ctx.clients[ci].ps.pmove.pm_flags = PMF_TIME_TELEPORT;
        ctx.clients[ci].ps.pmove.pm_time = 14;
    }

    ctx.clients[ci].respawn_time = ctx.level.time;

    if ctx.clients[ci].pers.spectator {
        gi_bprintf(PRINT_HIGH, &format!("{} has moved to the sidelines\n", ctx.clients[ci].pers.netname));
    } else {
        gi_bprintf(PRINT_HIGH, &format!("{} joined the game\n", ctx.clients[ci].pers.netname));
    }
}

// ============================================================
// PutClientInServer
// ============================================================

/// Converted from: PutClientInServer
pub fn put_client_in_server(ctx: &mut GameContext, ent_idx: usize) {
    let mins: Vec3 = [-16.0, -16.0, -24.0];
    let maxs: Vec3 = [16.0, 16.0, 32.0];

    // find a spawn point
    let (spawn_origin, spawn_angles) = select_spawn_point(ctx, ent_idx);

    let index = ent_idx - 1;
    let client_idx = index; // clients[index]

    // deathmatch wipes most client data every spawn
    let resp: ClientRespawn;
    if ctx.deathmatch != 0.0 {
        resp = ctx.clients[client_idx].resp.clone();
        let userinfo = ctx.clients[client_idx].pers.userinfo.clone();
        init_client_persistant(ctx, client_idx);
        client_userinfo_changed(ctx, ent_idx, &userinfo);
    } else if ctx.coop != 0.0 {
        let mut r = ctx.clients[client_idx].resp.clone();
        let userinfo = ctx.clients[client_idx].pers.userinfo.clone();
        r.coop_respawn.game_helpchanged = ctx.clients[client_idx].pers.game_helpchanged;
        r.coop_respawn.helpchanged = ctx.clients[client_idx].pers.helpchanged;
        ctx.clients[client_idx].pers = r.coop_respawn.clone();
        client_userinfo_changed(ctx, ent_idx, &userinfo);
        if r.score > ctx.clients[client_idx].pers.score {
            ctx.clients[client_idx].pers.score = r.score;
        }
        resp = r;
    } else {
        resp = ClientRespawn::default();
    }

    // clear everything but the persistant data
    let saved = ctx.clients[client_idx].pers.clone();
    ctx.clients[client_idx] = GClient::default();
    ctx.clients[client_idx].pers = saved;
    if ctx.clients[client_idx].pers.health <= 0 {
        init_client_persistant(ctx, client_idx);
    }
    ctx.clients[client_idx].resp = resp;

    // copy some data from the client to the entity
    fetch_client_ent_data(ctx, ent_idx);

    // clear entity values
    ctx.edicts[ent_idx].groundentity = -1;
    ctx.edicts[ent_idx].client = Some(client_idx);
    ctx.edicts[ent_idx].takedamage = Damage::Aim as i32;
    ctx.edicts[ent_idx].movetype = MoveType::Walk;
    ctx.edicts[ent_idx].viewheight = 22;
    ctx.edicts[ent_idx].inuse = true;
    ctx.edicts[ent_idx].classname = "player".to_string();
    ctx.edicts[ent_idx].mass = 200;
    ctx.edicts[ent_idx].solid = Solid::Bbox;
    ctx.edicts[ent_idx].deadflag = DEAD_NO;
    ctx.edicts[ent_idx].air_finished = ctx.level.time + 12.0;
    ctx.edicts[ent_idx].clipmask = MASK_PLAYERSOLID;
    ctx.edicts[ent_idx].model = "players/male/tris.md2".to_string();
    ctx.edicts[ent_idx].pain_fn = Some(PAIN_PLAYER);
    ctx.edicts[ent_idx].die_fn = Some(DIE_PLAYER);
    ctx.edicts[ent_idx].waterlevel = 0;
    ctx.edicts[ent_idx].watertype = 0;
    ctx.edicts[ent_idx].flags &= !FL_NO_KNOCKBACK;
    ctx.edicts[ent_idx].svflags &= !SVF_DEADMONSTER;

    ctx.edicts[ent_idx].mins = mins;
    ctx.edicts[ent_idx].maxs = maxs;
    ctx.edicts[ent_idx].velocity = vec3_origin;

    // clear playerstate values
    ctx.clients[client_idx].ps = PlayerState::default();

    ctx.clients[client_idx].ps.pmove.origin[0] = (spawn_origin[0] * 8.0) as i16;
    ctx.clients[client_idx].ps.pmove.origin[1] = (spawn_origin[1] * 8.0) as i16;
    ctx.clients[client_idx].ps.pmove.origin[2] = (spawn_origin[2] * 8.0) as i16;

    if ctx.deathmatch != 0.0 && DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_FIXED_FOV) {
        ctx.clients[client_idx].ps.fov = 90.0;
    } else {
        let fov_str = info_value_for_key(&ctx.clients[client_idx].pers.userinfo, "fov");
        let fov: f32 = fov_str.parse().unwrap_or(90.0);
        if fov < 1.0 {
            ctx.clients[client_idx].ps.fov = 90.0;
        } else if fov > 160.0 {
            ctx.clients[client_idx].ps.fov = 160.0;
        } else {
            ctx.clients[client_idx].ps.fov = fov;
        }
    }

    if let Some(weapon_idx) = ctx.clients[client_idx].pers.weapon {
        ctx.clients[client_idx].ps.gunindex = gi_modelindex(&ctx.items[weapon_idx].view_model);
    }

    // clear entity state values
    ctx.edicts[ent_idx].s.effects = 0;
    ctx.edicts[ent_idx].s.modelindex = 255;  // will use the skin specified model
    ctx.edicts[ent_idx].s.modelindex2 = 255; // custom gun model
    ctx.edicts[ent_idx].s.skinnum = (ent_idx - 1) as i32;

    ctx.edicts[ent_idx].s.frame = 0;
    ctx.edicts[ent_idx].s.origin = vector_copy(&spawn_origin);
    ctx.edicts[ent_idx].s.origin[2] += 1.0; // make sure off ground
    ctx.edicts[ent_idx].s.old_origin = vector_copy(&ctx.edicts[ent_idx].s.origin);

    // set the delta angle
    for i in 0..3 {
        ctx.clients[client_idx].ps.pmove.delta_angles[i] =
            angle2short(spawn_angles[i] - ctx.clients[client_idx].resp.cmd_angles[i]) as i16;
    }

    ctx.edicts[ent_idx].s.angles[PITCH] = 0.0;
    ctx.edicts[ent_idx].s.angles[YAW] = spawn_angles[YAW];
    ctx.edicts[ent_idx].s.angles[ROLL] = 0.0;
    ctx.clients[client_idx].ps.viewangles = vector_copy(&ctx.edicts[ent_idx].s.angles);
    ctx.clients[client_idx].v_angle = vector_copy(&ctx.edicts[ent_idx].s.angles);

    // spawn a spectator
    if ctx.clients[client_idx].pers.spectator {
        ctx.clients[client_idx].chase_target = -1;
        ctx.clients[client_idx].resp.spectator = true;

        ctx.edicts[ent_idx].movetype = MoveType::Noclip;
        ctx.edicts[ent_idx].solid = Solid::Not;
        ctx.edicts[ent_idx].svflags |= SVF_NOCLIENT;
        ctx.clients[client_idx].ps.gunindex = 0;
        gi_linkentity(ent_idx as i32);
        return;
    } else {
        ctx.clients[client_idx].resp.spectator = false;
    }

    crate::g_utils::killbox(ctx, ent_idx);

    gi_linkentity(ent_idx as i32);

    // force the current weapon up
    ctx.clients[client_idx].newweapon = ctx.clients[client_idx].pers.weapon;
    crate::p_weapon::change_weapon(ctx,ent_idx);
}

// ============================================================
// ClientBeginDeathmatch
// ============================================================

/// Converted from: ClientBeginDeathmatch
pub fn client_begin_deathmatch(ctx: &mut GameContext, ent_idx: usize) {
    g_init_edict(ctx, ent_idx);

    let ci = ctx.edicts[ent_idx].client.unwrap();
    init_client_resp(ctx, ci);

    put_client_in_server(ctx, ent_idx);

    if ctx.level.intermissiontime != 0.0 {
        crate::p_hud::move_client_to_intermission(ctx,ent_idx);
    } else {
        // send effect
        gi_write_byte(SVC_MUZZLEFLASH);
        gi_write_short(ent_idx as i32);
        gi_write_byte(MZ_LOGIN);
        gi_multicast(&ctx.edicts[ent_idx].s.origin, MULTICAST_PVS);
    }

    let ci = ctx.edicts[ent_idx].client.unwrap();
    gi_bprintf(PRINT_HIGH, &format!("{} entered the game\n", ctx.clients[ci].pers.netname));

    // NOTE: ClientEndServerFrame is called from the game dispatch layer after
    // ClientBegin/ClientBeginDeathmatch, as it requires p_view's ViewContext state.
}

// ============================================================
// ClientBegin
// ============================================================

/// Converted from: ClientBegin
pub fn client_begin(ctx: &mut GameContext, ent_idx: usize) {
    ctx.edicts[ent_idx].client = Some(ent_idx - 1);

    if ctx.deathmatch != 0.0 {
        client_begin_deathmatch(ctx, ent_idx);
        return;
    }

    // if there is already a body waiting for us (a loadgame), just take it
    if ctx.edicts[ent_idx].inuse {
        let ci = ctx.edicts[ent_idx].client.unwrap();
        for i in 0..3 {
            ctx.clients[ci].ps.pmove.delta_angles[i] =
                angle2short(ctx.clients[ci].ps.viewangles[i]) as i16;
        }
    } else {
        g_init_edict(ctx, ent_idx);
        ctx.edicts[ent_idx].classname = "player".to_string();
        let ci = ent_idx - 1;
        init_client_resp(ctx, ci);
        put_client_in_server(ctx, ent_idx);
    }

    if ctx.level.intermissiontime != 0.0 {
        crate::p_hud::move_client_to_intermission(ctx,ent_idx);
    } else if ctx.game.maxclients > 1 {
        gi_write_byte(SVC_MUZZLEFLASH);
        gi_write_short(ent_idx as i32);
        gi_write_byte(MZ_LOGIN);
        gi_multicast(&ctx.edicts[ent_idx].s.origin, MULTICAST_PVS);

        let ci = ctx.edicts[ent_idx].client.unwrap();
        gi_bprintf(PRINT_HIGH, &format!("{} entered the game\n", ctx.clients[ci].pers.netname));
    }

    // NOTE: ClientEndServerFrame is called from the game dispatch layer after
    // ClientBegin/ClientBeginDeathmatch, as it requires p_view's ViewContext state.
}

// ============================================================
// ClientUserinfoChanged
// ============================================================

/// Converted from: ClientUserinfoChanged
pub fn client_userinfo_changed(ctx: &mut GameContext, ent_idx: usize, userinfo: &str) {
    let mut userinfo = userinfo.to_string();

    // check for malformed or illegal info strings
    if !info_validate(&userinfo) {
        userinfo = "\\name\\badinfo\\skin\\male/grunt".to_string();
    }

    let ci = ctx.edicts[ent_idx].client.unwrap();

    // set name
    let s = info_value_for_key(&userinfo, "name");
    ctx.clients[ci].pers.netname = s;

    // set spectator
    let s = info_value_for_key(&userinfo, "spectator");
    ctx.clients[ci].pers.spectator = ctx.deathmatch != 0.0 && !s.is_empty() && s != "0";

    // set skin
    let skin = info_value_for_key(&userinfo, "skin");
    let playernum = ent_idx - 1;

    gi_configstring(
        (CS_PLAYERSKINS + playernum) as i32,
        &format!("{}\\{}", ctx.clients[ci].pers.netname, skin),
    );

    // fov
    if ctx.deathmatch != 0.0 && DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_FIXED_FOV) {
        ctx.clients[ci].ps.fov = 90.0;
    } else {
        let fov_str = info_value_for_key(&userinfo, "fov");
        let fov: f32 = fov_str.parse().unwrap_or(90.0);
        if fov < 1.0 {
            ctx.clients[ci].ps.fov = 90.0;
        } else if fov > 160.0 {
            ctx.clients[ci].ps.fov = 160.0;
        } else {
            ctx.clients[ci].ps.fov = fov;
        }
    }

    // handedness
    let s = info_value_for_key(&userinfo, "hand");
    if !s.is_empty() {
        ctx.clients[ci].pers.hand = s.parse().unwrap_or(0);
    }

    // save off the userinfo
    ctx.clients[ci].pers.userinfo = userinfo;
}

// ============================================================
// ClientConnect
// ============================================================

/// Converted from: ClientConnect
/// Returns (allowed: bool, userinfo: String) — modifies userinfo to add rejmsg on rejection.
pub fn client_connect(ctx: &mut GameContext, ent_idx: usize, userinfo: &str) -> (bool, String) {
    let mut userinfo = userinfo.to_string();

    // check banned IP
    let value = info_value_for_key(&userinfo, "ip");
    if sv_filter_packet(&value) {
        info_set_value_for_key(&mut userinfo, "rejmsg", "Banned.");
        return (false, userinfo);
    }

    // check for a spectator
    let value = info_value_for_key(&userinfo, "spectator");
    if ctx.deathmatch != 0.0 && !value.is_empty() && value != "0" {
        if !ctx.spectator_password.is_empty()
            && ctx.spectator_password != "none"
            && ctx.spectator_password != value
        {
            info_set_value_for_key(&mut userinfo, "rejmsg", "Spectator password required or incorrect.");
            return (false, userinfo);
        }

        // count spectators
        let mut numspec = 0;
        for i in 0..ctx.maxclients as usize {
            if ctx.edicts[i + 1].inuse {
                if let Some(c) = ctx.edicts[i + 1].client {
                    if ctx.clients[c].pers.spectator {
                        numspec += 1;
                    }
                }
            }
        }
        if numspec >= ctx.maxspectators as i32 {
            info_set_value_for_key(&mut userinfo, "rejmsg", "Server spectator limit is full.");
            return (false, userinfo);
        }
    } else {
        // check for a password
        let value = info_value_for_key(&userinfo, "password");
        if !ctx.password.is_empty()
            && ctx.password != "none"
            && ctx.password != value
        {
            info_set_value_for_key(&mut userinfo, "rejmsg", "Password required or incorrect.");
            return (false, userinfo);
        }
    }

    // they can connect
    let client_idx = ent_idx - 1;
    ctx.edicts[ent_idx].client = Some(client_idx);

    if !ctx.edicts[ent_idx].inuse {
        init_client_resp(ctx, client_idx);
        if !ctx.game.autosaved || ctx.clients[client_idx].pers.weapon.is_none() {
            init_client_persistant(ctx, client_idx);
        }
    }

    client_userinfo_changed(ctx, ent_idx, &userinfo);

    if ctx.game.maxclients > 1 {
        gi_dprintf(&format!("{} connected\n", ctx.clients[client_idx].pers.netname));
    }

    ctx.edicts[ent_idx].svflags = 0;
    ctx.clients[client_idx].pers.connected = true;

    (true, userinfo)
}

// ============================================================
// ClientDisconnect
// ============================================================

/// Converted from: ClientDisconnect
pub fn client_disconnect(ctx: &mut GameContext, ent_idx: usize) {
    if ctx.edicts[ent_idx].client.is_none() {
        return;
    }

    let ci = ctx.edicts[ent_idx].client.unwrap();

    gi_bprintf(PRINT_HIGH, &format!("{} disconnected\n", ctx.clients[ci].pers.netname));

    // send effect
    gi_write_byte(SVC_MUZZLEFLASH);
    gi_write_short(ent_idx as i32);
    gi_write_byte(MZ_LOGOUT);
    gi_multicast(&ctx.edicts[ent_idx].s.origin, MULTICAST_PVS);

    gi_unlinkentity(ent_idx as i32);

    ctx.edicts[ent_idx].s.modelindex = 0;
    ctx.edicts[ent_idx].solid = Solid::Not;
    ctx.edicts[ent_idx].inuse = false;
    ctx.edicts[ent_idx].classname = "disconnected".to_string();
    ctx.clients[ci].pers.connected = false;

    let playernum = ent_idx - 1;
    gi_configstring((CS_PLAYERSKINS + playernum) as i32, "");
}

// ============================================================
// PM_trace / CheckBlock / PrintPmove
// ============================================================

/// Converted from: PM_trace
/// pmove doesn't need to know about passent and contentmask
pub fn pm_trace(ctx: &GameContext, start: &Vec3, mins: &Vec3, maxs: &Vec3, end: &Vec3) {
    let mask = if ctx.edicts[ctx.pm_passent].health > 0 {
        MASK_PLAYERSOLID
    } else {
        MASK_DEADSOLID
    };
    let _trace = gi_trace(start, mins, maxs, end, ctx.pm_passent as i32, mask);
}

/// Converted from: CheckBlock
pub fn check_block(b: &[u8]) -> u32 {
    let mut v: u32 = 0;
    for &byte in b {
        v = v.wrapping_add(byte as u32);
    }
    v
}

/// Converted from: PrintPmove
pub fn print_pmove(pm_cmd_impulse: u8, c1: u32, c2: u32) {
    gi_dprintf(&format!("sv {:3}:{} {}\n", pm_cmd_impulse, c1, c2));
}

// ============================================================
// ClientThink
// ============================================================

/// Converted from: ClientThink
pub fn client_think(ctx: &mut GameContext, ent_idx: usize, ucmd: &UserCmd) {
    ctx.level.current_entity = ent_idx as i32;
    let ci = ctx.edicts[ent_idx].client.unwrap();

    if ctx.level.intermissiontime != 0.0 {
        ctx.clients[ci].ps.pmove.pm_type = PmType::Freeze;
        // can exit intermission after five seconds
        if ctx.level.time > ctx.level.intermissiontime + 5.0
            && (ucmd.buttons & BUTTON_ANY) != 0
        {
            ctx.level.exitintermission = 1;
        }
        return;
    }

    ctx.pm_passent = ent_idx;

    if ctx.clients[ci].chase_target >= 0 {
        ctx.clients[ci].resp.cmd_angles[0] = short2angle(ucmd.angles[0]);
        ctx.clients[ci].resp.cmd_angles[1] = short2angle(ucmd.angles[1]);
        ctx.clients[ci].resp.cmd_angles[2] = short2angle(ucmd.angles[2]);
    } else {
        // set up for pmove
        if ctx.edicts[ent_idx].movetype == MoveType::Noclip {
            ctx.clients[ci].ps.pmove.pm_type = PmType::Spectator;
        } else if ctx.edicts[ent_idx].s.modelindex != 255 {
            ctx.clients[ci].ps.pmove.pm_type = PmType::Gib;
        } else if ctx.edicts[ent_idx].deadflag != DEAD_NO {
            ctx.clients[ci].ps.pmove.pm_type = PmType::Dead;
        } else {
            ctx.clients[ci].ps.pmove.pm_type = PmType::Normal;
        }

        ctx.clients[ci].ps.pmove.gravity = ctx.sv_gravity as i16;

        // Pmove integration: set up pmove_t and call gi.Pmove(&pm).
        // The pmove module (qcommon/pmove.c) processes player movement physics.
        // Results (origin, velocity, groundentity, waterlevel, etc.) are copied back.
        // NOTE: Full pmove requires gi_pmove() which calls into myq2_common::pmove.
        // For now, we set up the inputs and update cmd_angles.
        let mut pm_s_origin = [0i16; 3];
        let mut pm_s_velocity = [0i16; 3];
        for i in 0..3 {
            pm_s_origin[i] = (ctx.edicts[ent_idx].s.origin[i] * 8.0) as i16;
            pm_s_velocity[i] = (ctx.edicts[ent_idx].velocity[i] * 8.0) as i16;
        }

        // gi.Pmove(&pm) would be called here. The pmove module handles:
        // - clip movement against world and entities
        // - stair stepping, swimming, flying
        // - setting groundentity, waterlevel, watertype
        // After pmove, results would be: pm.s.origin, pm.s.velocity, pm.viewangles,
        // pm.groundentity, pm.watertype, pm.waterlevel, pm.cmd, pm.touchents[]

        // Update cmd_angles from user command
        ctx.clients[ci].resp.cmd_angles[0] = short2angle(ucmd.angles[0]);
        ctx.clients[ci].resp.cmd_angles[1] = short2angle(ucmd.angles[1]);
        ctx.clients[ci].resp.cmd_angles[2] = short2angle(ucmd.angles[2]);

        // Jump sound: played when pmove detects landing (pm.groundentity set, was NULL)
        // Requires pmove results, deferred until pmove is integrated.

        if ctx.edicts[ent_idx].deadflag != DEAD_NO {
            ctx.clients[ci].ps.viewangles[ROLL] = 40.0;
            ctx.clients[ci].ps.viewangles[PITCH] = -15.0;
            ctx.clients[ci].ps.viewangles[YAW] = ctx.clients[ci].killer_yaw;
        }

        gi_linkentity(ent_idx as i32);

        if ctx.edicts[ent_idx].movetype != MoveType::Noclip {
            crate::g_utils::g_touch_triggers(ctx, ent_idx);
        }

        // Touch other objects: In C, iterates pm.touchents[] and calls other->touch().
        // Requires pmove results (pm.numtouch, pm.touchents[]), deferred until pmove is integrated.
    }

    ctx.clients[ci].oldbuttons = ctx.clients[ci].buttons;
    ctx.clients[ci].buttons = ucmd.buttons as i32;
    ctx.clients[ci].latched_buttons |= ctx.clients[ci].buttons & !ctx.clients[ci].oldbuttons;

    // save light level
    ctx.edicts[ent_idx].light_level = ucmd.lightlevel as i32;

    // fire weapon from final position if needed
    if (ctx.clients[ci].latched_buttons & (BUTTON_ATTACK as i32)) != 0 {
        if ctx.clients[ci].resp.spectator {
            ctx.clients[ci].latched_buttons = 0;

            if ctx.clients[ci].chase_target >= 0 {
                ctx.clients[ci].chase_target = -1;
                ctx.clients[ci].ps.pmove.pm_flags &= !PMF_NO_PREDICTION;
            } else {
                crate::g_chase::get_chase_target(ctx,ent_idx);
            }
        } else if !ctx.clients[ci].weapon_thunk {
            ctx.clients[ci].weapon_thunk = true;
            crate::p_weapon::think_weapon(ctx,ent_idx);
        }
    }

    if ctx.clients[ci].resp.spectator {
        if ucmd.upmove >= 10 {
            if (ctx.clients[ci].ps.pmove.pm_flags & PMF_JUMP_HELD) == 0 {
                ctx.clients[ci].ps.pmove.pm_flags |= PMF_JUMP_HELD;
                if ctx.clients[ci].chase_target >= 0 {
                    crate::g_chase::chase_next(ctx,ent_idx);
                } else {
                    crate::g_chase::get_chase_target(ctx,ent_idx);
                }
            }
        } else {
            ctx.clients[ci].ps.pmove.pm_flags &= !PMF_JUMP_HELD;
        }
    }

    // update chase cam if being followed
    for i in 1..=(ctx.maxclients as usize) {
        if ctx.edicts[i].inuse {
            if let Some(c) = ctx.edicts[i].client {
                if ctx.clients[c].chase_target == ent_idx as i32 {
                    crate::g_chase::update_chase_cam(ctx,i);
                }
            }
        }
    }
}

// ============================================================
// ClientBeginServerFrame
// ============================================================

/// Converted from: ClientBeginServerFrame
pub fn client_begin_server_frame(ctx: &mut GameContext, ent_idx: usize) {
    if ctx.level.intermissiontime != 0.0 {
        return;
    }

    let ci = ctx.edicts[ent_idx].client.unwrap();

    if ctx.deathmatch != 0.0
        && ctx.clients[ci].pers.spectator != ctx.clients[ci].resp.spectator
        && (ctx.level.time - ctx.clients[ci].respawn_time) >= 5.0
    {
        spectator_respawn(ctx, ent_idx);
        return;
    }

    // run weapon animations if it hasn't been done by a ucmd_t
    if !ctx.clients[ci].weapon_thunk && !ctx.clients[ci].resp.spectator {
        crate::p_weapon::think_weapon(ctx,ent_idx);
    } else {
        ctx.clients[ci].weapon_thunk = false;
    }

    if ctx.edicts[ent_idx].deadflag != DEAD_NO {
        // wait for any button just going down
        if ctx.level.time > ctx.clients[ci].respawn_time {
            let button_mask: i32 = if ctx.deathmatch != 0.0 {
                BUTTON_ATTACK as i32
            } else {
                -1 // all buttons
            };

            if (ctx.clients[ci].latched_buttons & button_mask) != 0
                || (ctx.deathmatch != 0.0 && DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_FORCE_RESPAWN))
            {
                respawn(ctx, ent_idx);
                ctx.clients[ci].latched_buttons = 0;
            }
        }
        return;
    }

    // add player trail so monsters can follow
    if ctx.deathmatch == 0.0 {
        // Player trail for monster AI pursuit: add breadcrumbs when not visible from last spot.
        // In C: if (!visible(ent, PlayerTrail_LastSpot())) PlayerTrail_Add(ent->s.old_origin)
        // This uses p_trail module which is functional but visible() needs trace support.
        // For now, always add trail points for AI tracking.
        // crate::p_trail would be called here if we had a shared context.
    }

    ctx.clients[ci].latched_buttons = 0;
}

// ============================================================
// Callback dispatch constants
// These are used as Option<usize> values in think_fn, die_fn, etc.
// ============================================================

pub use crate::dispatch::THINK_SP_CREATE_COOP_SPOTS;
pub use crate::dispatch::THINK_SP_FIX_COOP_SPOTS;
pub use crate::dispatch::THINK_FREE_EDICT as THINK_G_FREE_EDICT;
pub use crate::dispatch::TOUCH_ITEM;
pub use crate::dispatch::PAIN_PLAYER;
pub use crate::dispatch::DIE_PLAYER;
pub use crate::dispatch::DIE_BODY_DIE;

// ============================================================
// Stub functions for cross-module calls
// These will be implemented in their respective modules.
// ============================================================


/// Placeholder: initialize an edict.
fn g_init_edict(ctx: &mut GameContext, ent_idx: usize) {
    let client = ctx.edicts[ent_idx].client;
    ctx.edicts[ent_idx] = Edict::default();
    ctx.edicts[ent_idx].client = client;
    ctx.edicts[ent_idx].inuse = true;
}

/// SP_misc_teleporter_dest — sets up teleporter destination spot entity.
fn sp_misc_teleporter_dest(ctx: &mut GameContext, ent_idx: usize) {
    gi_setmodel(ent_idx as i32, "models/objects/dmspot/tris.md2");
    ctx.edicts[ent_idx].s.skinnum = 0;
    ctx.edicts[ent_idx].solid = Solid::Bbox;
    ctx.edicts[ent_idx].mins = [-32.0, -32.0, -24.0];
    ctx.edicts[ent_idx].maxs = [32.0, 32.0, -16.0];
    gi_linkentity(ent_idx as i32);
}

/// O(1) item lookup by pickup name via global HashMap index.
fn find_item(_ctx: &GameContext, name: &str) -> Option<usize> {
    crate::g_items::find_item(name)
}

fn find_item_by_classname(_ctx: &GameContext, classname: &str) -> Option<usize> {
    crate::g_items::find_item_by_classname(classname)
}

/// Drop_Item — drops an item entity into the world. Returns entity index of the dropped item.
fn drop_item(ctx: &mut GameContext, ent_idx: usize, item_idx: usize) -> usize {
    let dropped_idx = g_spawn(ctx);

    ctx.edicts[dropped_idx].classname = ctx.items[item_idx].classname.clone();
    ctx.edicts[dropped_idx].item = Some(item_idx);
    ctx.edicts[dropped_idx].spawnflags = DROPPED_ITEM;
    ctx.edicts[dropped_idx].s.effects = EF_ROTATE;
    ctx.edicts[dropped_idx].s.renderfx = RF_GLOW;
    ctx.edicts[dropped_idx].mins = [-15.0, -15.0, -15.0];
    ctx.edicts[dropped_idx].maxs = [15.0, 15.0, 15.0];

    // Set model
    let model = ctx.items[item_idx].world_model.clone();
    gi_setmodel(dropped_idx as i32, &model);
    ctx.edicts[dropped_idx].solid = Solid::Trigger;
    ctx.edicts[dropped_idx].movetype = MoveType::Toss;
    ctx.edicts[dropped_idx].owner = ent_idx as i32;

    // Toss forward from owner
    let ci = ctx.edicts[ent_idx].client.unwrap_or(0);
    let angle = ctx.clients[ci].v_angle[1] * PI / 180.0;
    let forward = [angle.cos(), angle.sin(), 0.0];
    ctx.edicts[dropped_idx].s.origin = ctx.edicts[ent_idx].s.origin;
    ctx.edicts[dropped_idx].s.origin[2] += 24.0;
    for i in 0..3 {
        ctx.edicts[dropped_idx].velocity[i] = forward[i] * 100.0;
    }
    ctx.edicts[dropped_idx].velocity[2] = 300.0;

    ctx.edicts[dropped_idx].nextthink = ctx.level.time + 30.0;
    // think_fn will be set to g_free_edict via dispatch

    gi_linkentity(dropped_idx as i32);

    dropped_idx
}

use myq2_common::q_shared::info_value_for_key;

/// Placeholder: Info_Validate
fn info_validate(userinfo: &str) -> bool {
    !userinfo.is_empty() && userinfo.contains('\\')
}

/// Info_SetValueForKey — removes existing key then appends new key/value.
fn info_set_value_for_key(userinfo: &mut String, key: &str, value: &str) {
    // Remove existing key/value pair if present
    let parts: Vec<&str> = userinfo.split('\\').collect();
    let mut new_info = String::new();
    let mut i = 1; // skip leading empty
    while i + 1 < parts.len() {
        if !parts[i].eq_ignore_ascii_case(key) {
            new_info.push('\\');
            new_info.push_str(parts[i]);
            new_info.push('\\');
            new_info.push_str(parts[i + 1]);
        }
        i += 2;
    }
    // Append new key/value
    new_info.push('\\');
    new_info.push_str(key);
    new_info.push('\\');
    new_info.push_str(value);
    *userinfo = new_info;
}

/// Placeholder: SV_FilterPacket (IP ban check)
fn sv_filter_packet(_ip: &str) -> bool {
    false
}

/// Random number matching C rand() & 0x7fff.
fn rand_val() -> i32 {
    (rand::random::<u32>() & 0x7fff) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_test_gi() {
        crate::game_import::set_gi(Box::new(crate::game_import::StubGameImport));
    }

    /// Create a minimal GameContext with the given number of clients.
    /// Allocates world entity (index 0) + one entity per client + extra entities.
    fn make_ctx(num_clients: i32) -> GameContext {
        init_test_gi();
        let mut ctx = GameContext::default();
        ctx.maxclients = num_clients as f32;
        ctx.game.maxclients = num_clients;
        ctx.deathmatch = 1.0;
        // world entity + client entities
        let total_edicts = 1 + num_clients as usize + BODY_QUEUE_SIZE + 16;
        for _ in 0..total_edicts {
            ctx.edicts.push(Edict::default());
        }
        ctx.num_edicts = ctx.edicts.len() as i32;
        for _ in 0..num_clients {
            ctx.clients.push(GClient::default());
        }
        ctx
    }

    /// Create a context with a single player wired up at ent_idx=1, client_idx=0.
    fn make_single_player_ctx() -> GameContext {
        let mut ctx = make_ctx(1);
        ctx.edicts[1].inuse = true;
        ctx.edicts[1].client = Some(0);
        ctx.edicts[1].health = 100;
        ctx.edicts[1].max_health = 100;
        ctx.clients[0].pers.connected = true;
        ctx.clients[0].pers.netname = "TestPlayer".to_string();
        ctx.clients[0].pers.userinfo = "\\name\\TestPlayer\\skin\\male/grunt\\gender\\male".to_string();
        ctx
    }

    // ============================================================
    // info_validate tests
    // ============================================================

    #[test]
    fn test_info_validate_valid() {
        assert!(info_validate("\\name\\test\\skin\\male/grunt"));
    }

    #[test]
    fn test_info_validate_empty() {
        assert!(!info_validate(""));
    }

    #[test]
    fn test_info_validate_no_backslash() {
        assert!(!info_validate("name=test"));
    }

    // ============================================================
    // info_set_value_for_key tests
    // ============================================================

    #[test]
    fn test_info_set_value_for_key_add_new() {
        let mut info = "\\name\\player".to_string();
        info_set_value_for_key(&mut info, "skin", "male/grunt");
        let result = info_value_for_key(&info, "skin");
        assert_eq!(result, "male/grunt");
        // original key preserved
        let name = info_value_for_key(&info, "name");
        assert_eq!(name, "player");
    }

    #[test]
    fn test_info_set_value_for_key_replace_existing() {
        let mut info = "\\name\\player\\skin\\female/athena".to_string();
        info_set_value_for_key(&mut info, "skin", "male/grunt");
        let result = info_value_for_key(&info, "skin");
        assert_eq!(result, "male/grunt");
    }

    #[test]
    fn test_info_set_value_for_key_case_insensitive() {
        let mut info = "\\Name\\player\\Skin\\female/athena".to_string();
        info_set_value_for_key(&mut info, "skin", "male/grunt");
        // The old "Skin" key should be removed (case insensitive)
        // and new "skin" added
        let result = info_value_for_key(&info, "skin");
        assert_eq!(result, "male/grunt");
    }

    // ============================================================
    // check_block tests
    // ============================================================

    #[test]
    fn test_check_block_empty() {
        assert_eq!(check_block(&[]), 0);
    }

    #[test]
    fn test_check_block_simple() {
        let data = [1u8, 2, 3, 4, 5];
        assert_eq!(check_block(&data), 15);
    }

    #[test]
    fn test_check_block_wrapping() {
        let data = vec![255u8; 100];
        let expected: u32 = 255 * 100;
        assert_eq!(check_block(&data), expected);
    }

    // ============================================================
    // is_female / is_neutral tests
    // ============================================================

    #[test]
    fn test_is_female_true() {
        let ctx = make_ctx_with_gender("female");
        assert!(is_female(&ctx, 1));
    }

    #[test]
    fn test_is_female_false_male() {
        let ctx = make_ctx_with_gender("male");
        assert!(!is_female(&ctx, 1));
    }

    #[test]
    fn test_is_female_capital_f() {
        let ctx = make_ctx_with_gender("Female");
        assert!(is_female(&ctx, 1));
    }

    #[test]
    fn test_is_neutral_empty_gender() {
        let ctx = make_ctx_with_gender("");
        // Empty gender => neutral
        assert!(is_neutral(&ctx, 1));
    }

    #[test]
    fn test_is_neutral_other_gender() {
        let ctx = make_ctx_with_gender("it");
        assert!(is_neutral(&ctx, 1));
    }

    #[test]
    fn test_is_neutral_male_not_neutral() {
        let ctx = make_ctx_with_gender("male");
        assert!(!is_neutral(&ctx, 1));
    }

    #[test]
    fn test_is_neutral_female_not_neutral() {
        let ctx = make_ctx_with_gender("female");
        assert!(!is_neutral(&ctx, 1));
    }

    /// Helper: make a context with a single player with the given gender in userinfo.
    fn make_ctx_with_gender(gender: &str) -> GameContext {
        let mut ctx = make_single_player_ctx();
        if gender.is_empty() {
            ctx.clients[0].pers.userinfo = "\\name\\TestPlayer\\skin\\male/grunt".to_string();
        } else {
            ctx.clients[0].pers.userinfo = format!("\\name\\TestPlayer\\skin\\male/grunt\\gender\\{}", gender);
        }
        ctx
    }

    // ============================================================
    // init_client_persistant tests
    // ============================================================

    #[test]
    fn test_init_client_persistant_health() {
        init_test_gi();
        let mut ctx = make_ctx(1);
        // Set up the item list so "Blaster" can be found
        crate::g_items::init_items(&mut ctx);
        ctx.build_item_indices();
        ctx.edicts[1].client = Some(0);

        init_client_persistant(&mut ctx, 0);

        assert_eq!(ctx.clients[0].pers.health, 100);
        assert_eq!(ctx.clients[0].pers.max_health, 100);
        assert!(ctx.clients[0].pers.connected);
    }

    #[test]
    fn test_init_client_persistant_ammo_limits() {
        init_test_gi();
        let mut ctx = make_ctx(1);
        crate::g_items::init_items(&mut ctx);
        ctx.build_item_indices();
        ctx.edicts[1].client = Some(0);

        init_client_persistant(&mut ctx, 0);

        assert_eq!(ctx.clients[0].pers.max_bullets, 200);
        assert_eq!(ctx.clients[0].pers.max_shells, 100);
        assert_eq!(ctx.clients[0].pers.max_rockets, 50);
        assert_eq!(ctx.clients[0].pers.max_grenades, 50);
        assert_eq!(ctx.clients[0].pers.max_cells, 200);
        assert_eq!(ctx.clients[0].pers.max_slugs, 50);
    }

    #[test]
    fn test_init_client_persistant_blaster_equipped() {
        init_test_gi();
        let mut ctx = make_ctx(1);
        crate::g_items::init_items(&mut ctx);
        ctx.build_item_indices();
        ctx.edicts[1].client = Some(0);

        init_client_persistant(&mut ctx, 0);

        // Should have blaster weapon set
        assert!(ctx.clients[0].pers.weapon.is_some());
        let blaster_idx = ctx.clients[0].pers.weapon.unwrap();
        assert_eq!(ctx.clients[0].pers.inventory[blaster_idx], 1);
    }

    // ============================================================
    // init_client_resp tests
    // ============================================================

    #[test]
    fn test_init_client_resp() {
        let mut ctx = make_single_player_ctx();
        ctx.level.framenum = 42;
        ctx.clients[0].pers.health = 75;

        init_client_resp(&mut ctx, 0);

        assert_eq!(ctx.clients[0].resp.enterframe, 42);
        // coop_respawn should be a copy of pers
        assert_eq!(ctx.clients[0].resp.coop_respawn.health, 75);
    }

    // ============================================================
    // look_at_killer tests
    // ============================================================

    #[test]
    fn test_look_at_killer_self_kill() {
        let mut ctx = make_single_player_ctx();
        ctx.edicts[1].s.angles[YAW] = 45.0;

        look_at_killer(&mut ctx, 1, 1, 1);

        // When attacker == self and inflictor == self, should use entity yaw
        assert_eq!(ctx.clients[0].killer_yaw, 45.0);
    }

    #[test]
    fn test_look_at_killer_from_attacker() {
        let mut ctx = make_single_player_ctx();
        // Need attacker at index 2
        if ctx.edicts.len() <= 2 {
            ctx.edicts.push(Edict::default());
        }
        ctx.edicts[1].s.origin = [0.0, 0.0, 0.0];
        ctx.edicts[2].s.origin = [100.0, 0.0, 0.0]; // attacker to the east

        look_at_killer(&mut ctx, 1, 0, 2);

        // Attacker at [100,0,0] from self at [0,0,0] should give yaw ~0 degrees
        assert!((ctx.clients[0].killer_yaw - 0.0).abs() < 1.0);
    }

    #[test]
    fn test_look_at_killer_north_direction() {
        let mut ctx = make_single_player_ctx();
        if ctx.edicts.len() <= 2 {
            ctx.edicts.push(Edict::default());
        }
        ctx.edicts[1].s.origin = [0.0, 0.0, 0.0];
        ctx.edicts[2].s.origin = [0.0, 100.0, 0.0]; // attacker to the north

        look_at_killer(&mut ctx, 1, 0, 2);

        // dir.x == 0, dir.y > 0 => yaw = 90
        assert!((ctx.clients[0].killer_yaw - 90.0).abs() < 1.0);
    }

    #[test]
    fn test_look_at_killer_south_direction() {
        let mut ctx = make_single_player_ctx();
        if ctx.edicts.len() <= 2 {
            ctx.edicts.push(Edict::default());
        }
        ctx.edicts[1].s.origin = [0.0, 0.0, 0.0];
        ctx.edicts[2].s.origin = [0.0, -100.0, 0.0]; // attacker to the south

        look_at_killer(&mut ctx, 1, 0, 2);

        // dir.x == 0, dir.y < 0 => yaw = -90 + 360 = 270
        assert!((ctx.clients[0].killer_yaw - 270.0).abs() < 1.0);
    }

    // ============================================================
    // client_obituary tests
    // ============================================================

    #[test]
    fn test_obituary_suicide() {
        let mut ctx = make_single_player_ctx();
        ctx.means_of_death = MOD_SUICIDE;

        client_obituary(&mut ctx, 1, 1, 1);

        // Score should be decremented by 1 for suicide in deathmatch
        assert_eq!(ctx.clients[0].resp.score, -1);
    }

    #[test]
    fn test_obituary_falling() {
        let mut ctx = make_single_player_ctx();
        ctx.means_of_death = MOD_FALLING;

        client_obituary(&mut ctx, 1, 1, 1);

        assert_eq!(ctx.clients[0].resp.score, -1);
    }

    #[test]
    fn test_obituary_lava() {
        let mut ctx = make_single_player_ctx();
        ctx.means_of_death = MOD_LAVA;

        client_obituary(&mut ctx, 1, 1, 1);

        assert_eq!(ctx.clients[0].resp.score, -1);
    }

    #[test]
    fn test_obituary_player_kill_blaster() {
        let mut ctx = make_ctx(2);
        // Set up two players
        ctx.edicts[1].inuse = true;
        ctx.edicts[1].client = Some(0);
        ctx.clients[0].pers.netname = "Victim".to_string();
        ctx.clients[0].pers.connected = true;

        ctx.edicts[2].inuse = true;
        ctx.edicts[2].client = Some(1);
        ctx.clients[1].pers.netname = "Killer".to_string();
        ctx.clients[1].pers.connected = true;

        ctx.means_of_death = MOD_BLASTER;

        client_obituary(&mut ctx, 1, 2, 2);

        // Killer gets +1 score
        assert_eq!(ctx.clients[1].resp.score, 1);
    }

    #[test]
    fn test_obituary_player_kill_railgun() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].inuse = true;
        ctx.edicts[1].client = Some(0);
        ctx.clients[0].pers.netname = "Victim".to_string();
        ctx.clients[0].pers.connected = true;

        ctx.edicts[2].inuse = true;
        ctx.edicts[2].client = Some(1);
        ctx.clients[1].pers.netname = "Killer".to_string();
        ctx.clients[1].pers.connected = true;

        ctx.means_of_death = MOD_RAILGUN;

        client_obituary(&mut ctx, 1, 2, 2);

        assert_eq!(ctx.clients[1].resp.score, 1);
    }

    #[test]
    fn test_obituary_friendly_fire() {
        let mut ctx = make_ctx(2);
        ctx.coop = 1.0;
        ctx.edicts[1].inuse = true;
        ctx.edicts[1].client = Some(0);
        ctx.clients[0].pers.netname = "Player1".to_string();
        ctx.clients[0].pers.connected = true;

        ctx.edicts[2].inuse = true;
        ctx.edicts[2].client = Some(1);
        ctx.clients[1].pers.netname = "Player2".to_string();
        ctx.clients[1].pers.connected = true;

        ctx.means_of_death = MOD_BLASTER;

        client_obituary(&mut ctx, 1, 2, 2);

        // Friendly fire in coop: attacker should lose a point
        assert_eq!(ctx.clients[1].resp.score, -1);
    }

    #[test]
    fn test_obituary_self_rocket_splash_male() {
        let mut ctx = make_single_player_ctx();
        ctx.clients[0].pers.userinfo = "\\name\\TestPlayer\\gender\\male".to_string();
        ctx.means_of_death = MOD_R_SPLASH;

        client_obituary(&mut ctx, 1, 1, 1);

        assert_eq!(ctx.clients[0].resp.score, -1);
    }

    #[test]
    fn test_obituary_self_grenade_splash_female() {
        let mut ctx = make_single_player_ctx();
        ctx.clients[0].pers.userinfo = "\\name\\TestPlayer\\gender\\female".to_string();
        ctx.means_of_death = MOD_G_SPLASH;

        client_obituary(&mut ctx, 1, 1, 1);

        assert_eq!(ctx.clients[0].resp.score, -1);
    }

    #[test]
    fn test_obituary_bfg_blast_self() {
        let mut ctx = make_single_player_ctx();
        ctx.means_of_death = MOD_BFG_BLAST;

        client_obituary(&mut ctx, 1, 1, 1);

        assert_eq!(ctx.clients[0].resp.score, -1);
    }

    #[test]
    fn test_obituary_telefrag() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].inuse = true;
        ctx.edicts[1].client = Some(0);
        ctx.clients[0].pers.netname = "Victim".to_string();
        ctx.clients[0].pers.connected = true;

        ctx.edicts[2].inuse = true;
        ctx.edicts[2].client = Some(1);
        ctx.clients[1].pers.netname = "Killer".to_string();
        ctx.clients[1].pers.connected = true;

        ctx.means_of_death = MOD_TELEFRAG;

        client_obituary(&mut ctx, 1, 2, 2);

        assert_eq!(ctx.clients[1].resp.score, 1);
    }

    #[test]
    fn test_obituary_unknown_death_no_attacker() {
        let mut ctx = make_single_player_ctx();
        ctx.means_of_death = MOD_UNKNOWN;

        // attacker is world (index 0)
        client_obituary(&mut ctx, 1, 0, 0);

        // "died" message, score -1
        assert_eq!(ctx.clients[0].resp.score, -1);
    }

    // ============================================================
    // players_range_from_spot tests
    // ============================================================

    #[test]
    fn test_players_range_from_spot_no_players() {
        let mut ctx = make_ctx(2);
        // No players in use
        ctx.edicts.push(Edict::default());
        let spot_idx = ctx.edicts.len() - 1;
        ctx.edicts[spot_idx].s.origin = [0.0, 0.0, 0.0];

        let range = players_range_from_spot(&ctx, spot_idx);
        assert_eq!(range, 9999999.0);
    }

    #[test]
    fn test_players_range_from_spot_one_player() {
        let mut ctx = make_ctx(1);
        ctx.edicts[1].inuse = true;
        ctx.edicts[1].health = 100;
        ctx.edicts[1].s.origin = [100.0, 0.0, 0.0];

        // Spawn spot at origin
        let spot_idx = ctx.edicts.len() - 1;
        ctx.edicts[spot_idx].s.origin = [0.0, 0.0, 0.0];

        let range = players_range_from_spot(&ctx, spot_idx);
        assert!((range - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_players_range_from_spot_closest_wins() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].inuse = true;
        ctx.edicts[1].health = 100;
        ctx.edicts[1].s.origin = [100.0, 0.0, 0.0];

        ctx.edicts[2].inuse = true;
        ctx.edicts[2].health = 100;
        ctx.edicts[2].s.origin = [50.0, 0.0, 0.0];

        let spot_idx = ctx.edicts.len() - 1;
        ctx.edicts[spot_idx].s.origin = [0.0, 0.0, 0.0];

        let range = players_range_from_spot(&ctx, spot_idx);
        assert!((range - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_players_range_from_spot_dead_skipped() {
        let mut ctx = make_ctx(1);
        ctx.edicts[1].inuse = true;
        ctx.edicts[1].health = 0; // dead
        ctx.edicts[1].s.origin = [10.0, 0.0, 0.0];

        let spot_idx = ctx.edicts.len() - 1;
        ctx.edicts[spot_idx].s.origin = [0.0, 0.0, 0.0];

        let range = players_range_from_spot(&ctx, spot_idx);
        // Dead player should be skipped => default huge distance
        assert_eq!(range, 9999999.0);
    }

    // ============================================================
    // save_client_data / fetch_client_ent_data tests
    // ============================================================

    #[test]
    fn test_save_client_data() {
        let mut ctx = make_single_player_ctx();
        ctx.edicts[1].health = 75;
        ctx.edicts[1].max_health = 100;
        ctx.edicts[1].flags = FL_GODMODE | FL_NOTARGET;

        save_client_data(&mut ctx);

        assert_eq!(ctx.clients[0].pers.health, 75);
        assert_eq!(ctx.clients[0].pers.max_health, 100);
        assert_eq!(ctx.clients[0].pers.saved_flags, (FL_GODMODE | FL_NOTARGET).bits());
    }

    #[test]
    fn test_fetch_client_ent_data() {
        let mut ctx = make_single_player_ctx();
        ctx.clients[0].pers.health = 50;
        ctx.clients[0].pers.max_health = 100;
        ctx.clients[0].pers.saved_flags = FL_GODMODE.bits();

        fetch_client_ent_data(&mut ctx, 1);

        assert_eq!(ctx.edicts[1].health, 50);
        assert_eq!(ctx.edicts[1].max_health, 100);
        assert!(ctx.edicts[1].flags.intersects(FL_GODMODE));
    }

    // ============================================================
    // client_userinfo_changed tests
    // ============================================================

    #[test]
    fn test_client_userinfo_changed_name() {
        let mut ctx = make_single_player_ctx();
        let userinfo = "\\name\\NewName\\skin\\male/grunt";

        client_userinfo_changed(&mut ctx, 1, userinfo);

        assert_eq!(ctx.clients[0].pers.netname, "NewName");
    }

    #[test]
    fn test_client_userinfo_changed_fov_clamped() {
        let mut ctx = make_single_player_ctx();
        let userinfo = "\\name\\Test\\skin\\male/grunt\\fov\\200";

        client_userinfo_changed(&mut ctx, 1, userinfo);

        assert_eq!(ctx.clients[0].ps.fov, 160.0); // clamped to max
    }

    #[test]
    fn test_client_userinfo_changed_fov_below_min() {
        let mut ctx = make_single_player_ctx();
        let userinfo = "\\name\\Test\\skin\\male/grunt\\fov\\0";

        client_userinfo_changed(&mut ctx, 1, userinfo);

        assert_eq!(ctx.clients[0].ps.fov, 90.0); // reset to default
    }

    #[test]
    fn test_client_userinfo_changed_fov_normal() {
        let mut ctx = make_single_player_ctx();
        let userinfo = "\\name\\Test\\skin\\male/grunt\\fov\\110";

        client_userinfo_changed(&mut ctx, 1, userinfo);

        assert_eq!(ctx.clients[0].ps.fov, 110.0);
    }

    #[test]
    fn test_client_userinfo_changed_handedness() {
        let mut ctx = make_single_player_ctx();
        let userinfo = "\\name\\Test\\skin\\male/grunt\\hand\\2";

        client_userinfo_changed(&mut ctx, 1, userinfo);

        assert_eq!(ctx.clients[0].pers.hand, CENTER_HANDED);
    }

    #[test]
    fn test_client_userinfo_changed_invalid_info() {
        let mut ctx = make_single_player_ctx();
        // Invalid userinfo (no backslash) should be replaced with default
        let userinfo = "garbage_without_backslash";

        client_userinfo_changed(&mut ctx, 1, userinfo);

        assert_eq!(ctx.clients[0].pers.netname, "badinfo");
    }

    #[test]
    fn test_client_userinfo_changed_spectator() {
        let mut ctx = make_single_player_ctx();
        let userinfo = "\\name\\Test\\skin\\male/grunt\\spectator\\1";

        client_userinfo_changed(&mut ctx, 1, userinfo);

        assert!(ctx.clients[0].pers.spectator);
    }

    #[test]
    fn test_client_userinfo_changed_spectator_no_dm() {
        let mut ctx = make_single_player_ctx();
        ctx.deathmatch = 0.0;
        let userinfo = "\\name\\Test\\skin\\male/grunt\\spectator\\1";

        client_userinfo_changed(&mut ctx, 1, userinfo);

        // spectator only in deathmatch
        assert!(!ctx.clients[0].pers.spectator);
    }

    // ============================================================
    // client_connect tests
    // ============================================================

    #[test]
    fn test_client_connect_success() {
        let mut ctx = make_single_player_ctx();
        ctx.edicts[1].inuse = false;
        ctx.edicts[1].client = None;
        crate::g_items::init_items(&mut ctx);
        ctx.build_item_indices();

        let userinfo = "\\name\\NewPlayer\\skin\\male/grunt\\ip\\192.168.1.1";
        let (allowed, _info) = client_connect(&mut ctx, 1, userinfo);

        assert!(allowed);
        assert!(ctx.clients[0].pers.connected);
    }

    #[test]
    fn test_client_connect_wrong_password() {
        let mut ctx = make_single_player_ctx();
        ctx.password = "secret".to_string();

        let userinfo = "\\name\\NewPlayer\\skin\\male/grunt\\password\\wrong\\ip\\192.168.1.1";
        let (allowed, info) = client_connect(&mut ctx, 1, userinfo);

        assert!(!allowed);
        let rejmsg = info_value_for_key(&info, "rejmsg");
        assert!(!rejmsg.is_empty());
    }

    #[test]
    fn test_client_connect_correct_password() {
        let mut ctx = make_single_player_ctx();
        ctx.password = "secret".to_string();
        ctx.edicts[1].inuse = false;
        ctx.edicts[1].client = None;
        crate::g_items::init_items(&mut ctx);
        ctx.build_item_indices();

        let userinfo = "\\name\\NewPlayer\\skin\\male/grunt\\password\\secret\\ip\\192.168.1.1";
        let (allowed, _info) = client_connect(&mut ctx, 1, userinfo);

        assert!(allowed);
    }

    // ============================================================
    // sp_info_player_start tests
    // ============================================================

    #[test]
    fn test_sp_info_player_start_no_coop() {
        let mut ctx = make_single_player_ctx();
        ctx.coop = 0.0;

        sp_info_player_start(&mut ctx, 1);

        // Should return immediately without setting think_fn
        assert!(ctx.edicts[1].think_fn.is_none());
    }

    #[test]
    fn test_sp_info_player_start_security_map() {
        let mut ctx = make_single_player_ctx();
        ctx.coop = 1.0;
        ctx.level.mapname = "security".to_string();

        sp_info_player_start(&mut ctx, 1);

        assert_eq!(ctx.edicts[1].think_fn, Some(THINK_SP_CREATE_COOP_SPOTS));
    }

    // ============================================================
    // sp_info_player_deathmatch tests
    // ============================================================

    #[test]
    fn test_sp_info_player_deathmatch_no_dm() {
        let mut ctx = make_single_player_ctx();
        ctx.deathmatch = 0.0;
        // Use an entity index beyond the protected range (maxclients + BODY_QUEUE_SIZE)
        // so that g_free_edict can actually free it
        let spot_idx = ctx.maxclients as usize + BODY_QUEUE_SIZE + 2;
        ctx.edicts[spot_idx].inuse = true;
        ctx.edicts[spot_idx].classname = "info_player_deathmatch".to_string();

        sp_info_player_deathmatch(&mut ctx, spot_idx);

        // Should free the edict
        assert!(!ctx.edicts[spot_idx].inuse);
    }

    // ============================================================
    // sp_info_player_coop tests
    // ============================================================

    #[test]
    fn test_sp_info_player_coop_no_coop() {
        let mut ctx = make_single_player_ctx();
        ctx.coop = 0.0;
        // Use an entity index beyond the protected range
        let spot_idx = ctx.maxclients as usize + BODY_QUEUE_SIZE + 2;
        ctx.edicts[spot_idx].inuse = true;
        ctx.edicts[spot_idx].classname = "info_player_coop".to_string();

        sp_info_player_coop(&mut ctx, spot_idx);

        assert!(!ctx.edicts[spot_idx].inuse);
    }

    #[test]
    fn test_sp_info_player_coop_jail2_map() {
        let mut ctx = make_single_player_ctx();
        ctx.coop = 1.0;
        ctx.level.mapname = "jail2".to_string();
        let spot_idx = ctx.maxclients as usize + BODY_QUEUE_SIZE + 2;
        ctx.edicts[spot_idx].inuse = true;

        sp_info_player_coop(&mut ctx, spot_idx);

        assert_eq!(ctx.edicts[spot_idx].think_fn, Some(THINK_SP_FIX_COOP_SPOTS));
    }

    #[test]
    fn test_sp_info_player_coop_unknown_map() {
        let mut ctx = make_single_player_ctx();
        ctx.coop = 1.0;
        ctx.level.mapname = "someothermap".to_string();
        let spot_idx = ctx.maxclients as usize + BODY_QUEUE_SIZE + 2;
        ctx.edicts[spot_idx].inuse = true;

        sp_info_player_coop(&mut ctx, spot_idx);

        // Unknown map doesn't need fix
        assert!(ctx.edicts[spot_idx].think_fn.is_none());
    }

    // ============================================================
    // client_disconnect tests
    // ============================================================

    #[test]
    fn test_client_disconnect() {
        let mut ctx = make_single_player_ctx();

        client_disconnect(&mut ctx, 1);

        assert!(!ctx.edicts[1].inuse);
        assert_eq!(ctx.edicts[1].solid, Solid::Not);
        assert!(!ctx.clients[0].pers.connected);
        assert_eq!(ctx.edicts[1].classname, "disconnected");
    }

    #[test]
    fn test_client_disconnect_no_client() {
        let mut ctx = make_single_player_ctx();
        ctx.edicts[1].client = None;

        // Should not panic
        client_disconnect(&mut ctx, 1);
    }

    // ============================================================
    // body_que tests
    // ============================================================

    #[test]
    fn test_init_body_que() {
        let mut ctx = make_single_player_ctx();

        // Ensure we have enough edicts
        while ctx.edicts.len() < ctx.maxclients as usize + 1 + BODY_QUEUE_SIZE + 4 {
            ctx.edicts.push(Edict::default());
        }
        ctx.num_edicts = ctx.edicts.len() as i32;

        init_body_que(&mut ctx);

        assert_eq!(ctx.level.body_que, 0);
    }

    // ============================================================
    // copy_to_body_que tests
    // ============================================================

    #[test]
    fn test_copy_to_body_que_basic() {
        let mut ctx = make_single_player_ctx();
        while ctx.edicts.len() < ctx.maxclients as usize + 1 + BODY_QUEUE_SIZE + 4 {
            ctx.edicts.push(Edict::default());
        }
        ctx.num_edicts = ctx.edicts.len() as i32;
        ctx.level.body_que = 0;

        ctx.edicts[1].s.origin = [100.0, 200.0, 300.0];
        ctx.edicts[1].s.modelindex = 255;

        copy_to_body_que(&mut ctx, 1);

        // Body should be at expected index
        let body_idx = ctx.maxclients as usize + 1;
        assert_eq!(ctx.edicts[body_idx].s.origin, [100.0, 200.0, 300.0]);
        assert_eq!(ctx.edicts[body_idx].s.number, body_idx as i32);
        assert_eq!(ctx.level.body_que, 1);
    }

    #[test]
    fn test_copy_to_body_que_wraps() {
        let mut ctx = make_single_player_ctx();
        while ctx.edicts.len() < ctx.maxclients as usize + 1 + BODY_QUEUE_SIZE + 4 {
            ctx.edicts.push(Edict::default());
        }
        ctx.num_edicts = ctx.edicts.len() as i32;
        ctx.level.body_que = (BODY_QUEUE_SIZE - 1) as i32;

        copy_to_body_que(&mut ctx, 1);

        // Should wrap around to 0
        assert_eq!(ctx.level.body_que, 0);
    }
}

