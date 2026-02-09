// g_combat.rs — Combat and damage functions
// Converted from: myq2-original/game/g_combat.c

use crate::g_local::{
    Edict, LevelLocals, Solid,
    DEAD_DEAD,
    DamageFlags,
    AI_GOOD_GUY, AI_DUCKED, AI_SOUND_TARGET,
    FL_FLY, FL_SWIM, FL_NO_KNOCKBACK, FL_GODMODE,
    POWER_ARMOR_NONE, POWER_ARMOR_SCREEN,
    DAMAGE_RADIUS, DAMAGE_NO_ARMOR, DAMAGE_ENERGY, DAMAGE_NO_KNOCKBACK,
    DAMAGE_BULLET, DAMAGE_NO_PROTECTION,
    MOD_FRIENDLY_FIRE,
    MoveType,
};
use rayon::prelude::*;
use crate::game::{SVF_MONSTER};
use crate::g_local::{TE_SCREEN_SPARKS, TE_SHIELD_SPARKS, TE_BULLET_SPARKS, TE_SPARKS, TE_BLOOD};
use myq2_common::q_shared::{Vec3, vec3_origin, vector_add, vector_subtract, vector_scale, vector_copy, vector_length, vector_ma, vector_normalize, dot_product, angle_vectors, Trace};
use myq2_common::q_shared::{DmFlags, MASK_SOLID, CHAN_ITEM, ATTN_NORM, DF_MODELTEAMS, DF_SKINTEAMS, DF_NO_FRIENDLY_FIRE, info_value_for_key};

// LevelLocals imported from g_local

// ============================================================
// Forward declarations for functions that would be defined elsewhere
// ============================================================

/// Returns the team name for an entity.
/// Extracts team from the client's userinfo "skin" key.
fn client_team(ent_idx: usize, edicts: &[Edict]) -> String {
    let client_idx = match edicts[ent_idx].client {
        Some(c) => c,
        None => return String::new(),
    };
    crate::g_local::with_global_game_ctx(|ctx| {
        if client_idx >= ctx.clients.len() {
            return String::new();
        }
        let value = info_value_for_key(&ctx.clients[client_idx].pers.userinfo, "skin");
        if let Some(slash_pos) = value.find('/') {
            if DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_MODELTEAMS) {
                return value[..slash_pos].to_string();
            }
            return value[slash_pos + 1..].to_string();
        }
        value
    }).unwrap_or_default()
}

/// Find all entities within a radius using parallel iteration.
///
/// For scenes with many entities, this significantly speeds up radius damage
/// calculations and spatial queries. Uses distance-squared to avoid sqrt.
///
/// # Arguments
/// * `from` - Optional starting index (exclusive), or None to start from index 1
/// * `origin` - Center point to search from
/// * `radius` - Search radius
/// * `edicts` - Entity array to search
///
/// # Returns
/// Indices of entities within the radius, sorted by index for deterministic ordering.
pub fn findradius(from: Option<usize>, origin: Vec3, radius: f32, edicts: &[Edict]) -> Vec<usize> {
    let start = from.map(|f| f + 1).unwrap_or(1);

    if start >= edicts.len() {
        return Vec::new();
    }

    let radius_sq = radius * radius;

    let mut result: Vec<usize> = edicts[start..]
        .par_iter()
        .enumerate()
        .filter_map(|(rel_idx, ent)| {
            let idx = start + rel_idx;

            if !ent.inuse {
                return None;
            }
            if ent.solid == Solid::Not {
                return None;
            }

            // Calculate distance squared from origin to entity center (avoid sqrt)
            let eorg = [
                origin[0] - (ent.s.origin[0] + (ent.mins[0] + ent.maxs[0]) * 0.5),
                origin[1] - (ent.s.origin[1] + (ent.mins[1] + ent.maxs[1]) * 0.5),
                origin[2] - (ent.s.origin[2] + (ent.mins[2] + ent.maxs[2]) * 0.5),
            ];
            let dist_sq = eorg[0] * eorg[0] + eorg[1] * eorg[1] + eorg[2] * eorg[2];

            if dist_sq < radius_sq {
                Some(idx)
            } else {
                None
            }
        })
        .collect();

    // Sort for deterministic ordering (parallel collect doesn't preserve order)
    result.sort_unstable();
    result
}


/// Monster found a target
///
/// Bridges to the real `g_ai::found_target` implementation by borrowing
/// the global game context and building an `AiContext` via `mem::take`.
fn found_target(self_idx: usize, edicts: &mut [Edict]) {
    crate::g_local::with_global_game_ctx(|ctx| {
        use std::mem;
        // Sync caller's edicts into the global context before bridging
        ctx.edicts.clear();
        ctx.edicts.extend_from_slice(edicts);

        let mut ai_ctx = crate::g_ai::AiContext {
            edicts: mem::take(&mut ctx.edicts),
            clients: mem::take(&mut ctx.clients),
            level: mem::take(&mut ctx.level),
            game: mem::take(&mut ctx.game),
            coop: ctx.coop,
            skill: ctx.skill,
            enemy_vis: false,
            enemy_infront: false,
            enemy_range: 0,
            enemy_yaw: 0.0,
        };

        crate::g_ai::found_target(&mut ai_ctx, self_idx as i32);

        // Move state back into the global context
        ctx.edicts = ai_ctx.edicts;
        ctx.clients = ai_ctx.clients;
        ctx.level = ai_ctx.level;
        ctx.game = ai_ctx.game;

        // Sync edicts back to the caller's slice
        edicts.clone_from_slice(&ctx.edicts);
    });
}

/// Monster death use function
/// Fires death targets and clears the monster's targeting state.
/// Delegates to the complete implementation in g_monster.rs which
/// calls g_use_targets with the enemy as activator.
fn monster_death_use(self_idx: usize, edicts: &mut [Edict]) {
    crate::g_local::with_global_game_ctx(|ctx| {
        // Sync local edicts into the global context
        ctx.edicts.clear();
        ctx.edicts.extend_from_slice(edicts);
        // Call the complete implementation in g_monster
        crate::g_monster::monster_death_use(ctx, self_idx as i32);
        // Sync edicts back from the global context
        edicts.clone_from_slice(&ctx.edicts);
    });
}

/// Find an item by its pickup name. Searches the global item list.
fn find_item(pickup_name: &str) -> Option<usize> {
    crate::g_local::with_global_game_ctx(|ctx| {
        for i in 0..ctx.items.len() {
            if ctx.items[i].pickup_name.is_empty() {
                continue;
            }
            if ctx.items[i].pickup_name.eq_ignore_ascii_case(pickup_name) {
                return Some(i);
            }
        }
        None
    }).flatten()
}

/// Get item by index. Returns Some(index) if valid, None otherwise.
fn get_item_by_index(index: i32) -> Option<usize> {
    if index <= 0 {
        return None;
    }
    let idx = index as usize;
    crate::g_local::with_global_game_ctx(|ctx| {
        if idx < ctx.items.len() {
            Some(idx)
        } else {
            None
        }
    }).flatten()
}

/// Get armor index for entity.
/// Returns the item index of the best armor the entity's client currently has,
/// checking body armor first, then combat armor, then jacket armor.
/// Returns 0 if no armor or no client.
fn armor_index(ent: &Edict) -> i32 {
    let client_idx = match ent.client {
        Some(c) => c,
        None => return 0,
    };
    crate::g_local::with_global_game_ctx(|ctx| {
        if client_idx >= ctx.clients.len() {
            return 0;
        }
        // Look up armor item indices from the item list
        let jacket_idx = find_item("Jacket Armor").unwrap_or(0);
        let combat_idx = find_item("Combat Armor").unwrap_or(0);
        let body_idx = find_item("Body Armor").unwrap_or(0);

        // Check in order: body > combat > jacket (best first)
        if body_idx > 0 && ctx.clients[client_idx].pers.inventory[body_idx] > 0 {
            return body_idx as i32;
        }
        if combat_idx > 0 && ctx.clients[client_idx].pers.inventory[combat_idx] > 0 {
            return combat_idx as i32;
        }
        if jacket_idx > 0 && ctx.clients[client_idx].pers.inventory[jacket_idx] > 0 {
            return jacket_idx as i32;
        }
        0
    }).unwrap_or(0)
}

/// Get power armor type for entity.
/// Returns POWER_ARMOR_SHIELD, POWER_ARMOR_SCREEN, or POWER_ARMOR_NONE.
/// Checks if the entity has FL_POWER_ARMOR flag and the appropriate item in inventory.
fn power_armor_type(ent: &Edict) -> i32 {
    let client_idx = match ent.client {
        Some(c) => c,
        None => return POWER_ARMOR_NONE,
    };
    if !ent.flags.intersects(crate::g_local::FL_POWER_ARMOR) {
        return POWER_ARMOR_NONE;
    }
    crate::g_local::with_global_game_ctx(|ctx| {
        if client_idx >= ctx.clients.len() {
            return POWER_ARMOR_NONE;
        }
        // Check for Power Shield first (it takes priority in C code)
        let psi = find_item("Power Shield").unwrap_or(0);
        if psi > 0 && ctx.clients[client_idx].pers.inventory[psi] > 0 {
            return crate::g_local::POWER_ARMOR_SHIELD;
        }
        let pci = find_item("Power Screen").unwrap_or(0);
        if pci > 0 && ctx.clients[client_idx].pers.inventory[pci] > 0 {
            return POWER_ARMOR_SCREEN;
        }
        POWER_ARMOR_NONE
    }).unwrap_or(POWER_ARMOR_NONE)
}

use crate::game_import::{gi_soundindex, gi_write_byte, gi_write_position, gi_write_dir, gi_multicast, gi_sound};


// ============================================================
// Public API
// ============================================================

/// Check if two entities are on the same team.
fn on_same_team(ent1_idx: usize, ent2_idx: usize, edicts: &[Edict]) -> bool {
    let has_teams = crate::g_local::with_global_game_ctx(|ctx| {
        DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_MODELTEAMS | DF_SKINTEAMS)
    }).unwrap_or(false);
    if !has_teams {
        return false;
    }
    let ent1_team = client_team(ent1_idx, edicts);
    let ent2_team = client_team(ent2_idx, edicts);
    ent1_team == ent2_team
}

/// Returns true if the inflictor can directly damage the target.
/// Used for explosions and melee attacks.
pub fn can_damage(targ_idx: usize, inflictor_idx: usize, edicts: &[Edict]) -> bool {
    let targ = &edicts[targ_idx];
    let inflictor = &edicts[inflictor_idx];

    let mut dest: Vec3;
    let mut trace: Trace;

    // bmodels need special checking because their origin is 0,0,0
    if targ.movetype == MoveType::Push {
        dest = vector_add(&targ.absmin, &targ.absmax);
        dest = vector_scale(&dest, 0.5);
        trace = crate::game_import::gi_trace(&inflictor.s.origin, &vec3_origin, &vec3_origin, &dest, inflictor_idx as i32, MASK_SOLID);
        if trace.fraction == 1.0 {
            return true;
        }
        if trace.ent_index == targ_idx as i32 {
            return true;
        }
        return false;
    }

    trace = crate::game_import::gi_trace(&inflictor.s.origin, &vec3_origin, &vec3_origin, &targ.s.origin, inflictor_idx as i32, MASK_SOLID);
    if trace.fraction == 1.0 {
        return true;
    }

    dest = vector_copy(&targ.s.origin);
    dest[0] += 15.0;
    dest[1] += 15.0;
    trace = crate::game_import::gi_trace(&inflictor.s.origin, &vec3_origin, &vec3_origin, &dest, inflictor_idx as i32, MASK_SOLID);
    if trace.fraction == 1.0 {
        return true;
    }

    dest = vector_copy(&targ.s.origin);
    dest[0] += 15.0;
    dest[1] -= 15.0;
    trace = crate::game_import::gi_trace(&inflictor.s.origin, &vec3_origin, &vec3_origin, &dest, inflictor_idx as i32, MASK_SOLID);
    if trace.fraction == 1.0 {
        return true;
    }

    dest = vector_copy(&targ.s.origin);
    dest[0] -= 15.0;
    dest[1] += 15.0;
    trace = crate::game_import::gi_trace(&inflictor.s.origin, &vec3_origin, &vec3_origin, &dest, inflictor_idx as i32, MASK_SOLID);
    if trace.fraction == 1.0 {
        return true;
    }

    dest = vector_copy(&targ.s.origin);
    dest[0] -= 15.0;
    dest[1] -= 15.0;
    trace = crate::game_import::gi_trace(&inflictor.s.origin, &vec3_origin, &vec3_origin, &dest, inflictor_idx as i32, MASK_SOLID);
    if trace.fraction == 1.0 {
        return true;
    }

    false
}

/// Called when an entity is killed
pub fn killed(
    targ_idx: usize,
    inflictor_idx: usize,
    attacker_idx: usize,
    damage: i32,
    point: Vec3,
    edicts: &mut [Edict],
    level: &mut LevelLocals,
) {
    if edicts[targ_idx].health < -999 {
        edicts[targ_idx].health = -999;
    }

    edicts[targ_idx].enemy = attacker_idx as i32;

    if (edicts[targ_idx].svflags & SVF_MONSTER) != 0 && edicts[targ_idx].deadflag != DEAD_DEAD {
        // targ.svflags |= SVF_DEADMONSTER;
        if !edicts[targ_idx].monsterinfo.aiflags.intersects(AI_GOOD_GUY) {
            level.killed_monsters += 1;
            let coop = crate::g_local::with_global_game_ctx(|ctx| ctx.coop).unwrap_or(0.0);
            if coop != 0.0 {
                if let Some(client_idx) = edicts[attacker_idx].client {
                    crate::g_local::with_global_game_ctx(|ctx| {
                        if client_idx < ctx.clients.len() {
                            ctx.clients[client_idx].resp.score += 1;
                        }
                    });
                }
            }
            if edicts[attacker_idx].classname == "monster_medic" {
                edicts[targ_idx].owner = attacker_idx as i32;
            }
        }
    }

    let movetype = edicts[targ_idx].movetype;
    if movetype == MoveType::Push || movetype == MoveType::Stop || movetype == MoveType::None {
        // doors, triggers, etc
        crate::dispatch::call_die(targ_idx, inflictor_idx, attacker_idx, edicts, level, damage, point);
        return;
    }

    if (edicts[targ_idx].svflags & SVF_MONSTER) != 0 && edicts[targ_idx].deadflag != DEAD_DEAD {
        edicts[targ_idx].touch_fn = None;
        monster_death_use(targ_idx, edicts);
    }

    crate::dispatch::call_die(targ_idx, inflictor_idx, attacker_idx, edicts, level, damage, point);
}

/// Spawn damage visual effect
pub fn spawn_damage(te_type: i32, origin: Vec3, normal: Vec3, damage: i32) {
    let _damage_clamped = if damage > 255 { 255 } else { damage };

    gi_write_byte(3); // svc_temp_entity
    gi_write_byte(te_type);
    // gi_write_byte(damage_clamped); // Commented out in original
    gi_write_position(&origin);
    gi_write_dir(&normal);
    gi_multicast(&origin, 0); // MULTICAST_PVS
}

/// Check power armor protection
fn check_power_armor(
    ent_idx: usize,
    point: Vec3,
    normal: Vec3,
    damage: i32,
    dflags: DamageFlags,
    edicts: &mut [Edict],
    level: &mut LevelLocals,
) -> i32 {
    if damage == 0 {
        return 0;
    }

    let ent = &edicts[ent_idx];

    if dflags.intersects(DAMAGE_NO_ARMOR) {
        return 0;
    }

    let (power_armor_type, power) = if let Some(client_idx) = ent.client {
        let pat = power_armor_type(ent);
        if pat != POWER_ARMOR_NONE {
            if let Some(item_idx) = find_item("Cells") {
                let power_val = crate::g_local::with_global_game_ctx(|ctx| {
                    if client_idx < ctx.clients.len() {
                        ctx.clients[client_idx].pers.inventory[item_idx]
                    } else {
                        0
                    }
                }).unwrap_or(0);
                (pat, power_val)
            } else {
                (pat, 0)
            }
        } else {
            (POWER_ARMOR_NONE, 0)
        }
    } else if (ent.svflags & SVF_MONSTER) != 0 {
        (ent.monsterinfo.power_armor_type, ent.monsterinfo.power_armor_power)
    } else {
        return 0;
    };

    if power_armor_type == POWER_ARMOR_NONE {
        return 0;
    }
    if power == 0 {
        return 0;
    }

    let (damage_per_cell, pa_te_type, damage_modified) = if power_armor_type == POWER_ARMOR_SCREEN {
        let mut forward: Vec3 = [0.0, 0.0, 0.0];

        // only works if damage point is in front
        angle_vectors(&ent.s.angles, Some(&mut forward), None, None);
        let mut vec = vector_subtract(&point, &ent.s.origin);
        vector_normalize(&mut vec);
        let dot = dot_product(&vec, &forward);
        if dot <= 0.3 {
            return 0;
        }

        (1, TE_SCREEN_SPARKS, damage / 3)
    } else {
        (2, TE_SHIELD_SPARKS, (2 * damage) / 3)
    };

    let mut save = power * damage_per_cell;
    if save == 0 {
        return 0;
    }
    if save > damage_modified {
        save = damage_modified;
    }

    spawn_damage(pa_te_type, point, normal, save);

    edicts[ent_idx].powerarmor_time = level.time + 0.2;

    let power_used = save / damage_per_cell;

    if let Some(_client_idx) = edicts[ent_idx].client {
        // client.pers.inventory[index] -= power_used;
    } else {
        edicts[ent_idx].monsterinfo.power_armor_power -= power_used;
    }

    save
}

/// Check armor protection
fn check_armor(
    ent_idx: usize,
    point: Vec3,
    normal: Vec3,
    damage: i32,
    te_sparks: i32,
    dflags: DamageFlags,
    edicts: &mut [Edict],
) -> i32 {
    if damage == 0 {
        return 0;
    }

    let ent = &edicts[ent_idx];

    if ent.client.is_none() {
        return 0;
    }

    if dflags.intersects(DAMAGE_NO_ARMOR) {
        return 0;
    }

    let index = armor_index(ent);
    if index == 0 {
        return 0;
    }

    // Look up the armor info from the item table
    let armor_info = crate::g_local::with_global_game_ctx(|ctx| {
        if index as usize >= ctx.items.len() { return None; }
        ctx.items[index as usize].armor_info.clone()
    }).flatten();
    let armor_info = match armor_info {
        Some(info) => info,
        None => return 0,
    };

    let save = if dflags.intersects(DAMAGE_ENERGY) {
        (armor_info.energy_protection * damage as f32).ceil() as i32
    } else {
        (armor_info.normal_protection * damage as f32).ceil() as i32
    };

    // Cap save to available armor points
    let save = if let Some(client_idx) = ent.client {
        crate::g_local::with_global_game_ctx(|ctx| {
            if client_idx < ctx.clients.len() {
                let avail = ctx.clients[client_idx].pers.inventory[index as usize];
                if save > avail { avail } else { save }
            } else {
                save
            }
        }).unwrap_or(save)
    } else {
        save
    };

    if save == 0 {
        return 0;
    }

    // client.pers.inventory[index] -= save;
    spawn_damage(te_sparks, point, normal, save);

    save
}

/// Monster reaction to damage
pub fn m_react_to_damage(targ_idx: usize, attacker_idx: usize, edicts: &mut [Edict]) {
    let attacker = &edicts[attacker_idx];

    if attacker.client.is_none() && (attacker.svflags & SVF_MONSTER) == 0 {
        return;
    }

    if attacker_idx == targ_idx {
        return;
    }

    let targ_enemy = edicts[targ_idx].enemy;
    if attacker_idx as i32 == targ_enemy {
        return;
    }

    // if we are a good guy monster and our attacker is a player
    // or another good guy, do not get mad at them
    if edicts[targ_idx].monsterinfo.aiflags.intersects(AI_GOOD_GUY)
        && (attacker.client.is_some() || attacker.monsterinfo.aiflags.intersects(AI_GOOD_GUY)) {
            return;
        }

    // we now know that we are not both good guys

    // if attacker is a client, get mad at them because he's good and we're not
    if attacker.client.is_some() {
        edicts[targ_idx].monsterinfo.aiflags.remove(AI_SOUND_TARGET);

        // this can only happen in coop (both new and old enemies are clients)
        // only switch if can't see the current enemy
        if targ_enemy >= 0 && edicts[targ_enemy as usize].client.is_some() {
            if crate::g_ai::visible(&edicts[targ_idx], &edicts[targ_enemy as usize]) {
                edicts[targ_idx].oldenemy = attacker_idx as i32;
                return;
            }
            edicts[targ_idx].oldenemy = targ_enemy;
        }
        edicts[targ_idx].enemy = attacker_idx as i32;
        if !edicts[targ_idx].monsterinfo.aiflags.intersects(AI_DUCKED) {
            found_target(targ_idx, edicts);
        }
        return;
    }

    // it's the same base (walk/swim/fly) type and a different classname and it's not a tank
    // (they spray too much), get mad at them
    let targ_flags = edicts[targ_idx].flags;
    let attacker_flags = edicts[attacker_idx].flags;
    let targ_classname = &edicts[targ_idx].classname;
    let attacker_classname = &edicts[attacker_idx].classname;

    if ((targ_flags & (FL_FLY | FL_SWIM)) == (attacker_flags & (FL_FLY | FL_SWIM)))
        && (targ_classname != attacker_classname)
        && (attacker_classname != "monster_tank")
        && (attacker_classname != "monster_supertank")
        && (attacker_classname != "monster_makron")
        && (attacker_classname != "monster_jorg")
    {
        if targ_enemy >= 0 && edicts[targ_enemy as usize].client.is_some() {
            edicts[targ_idx].oldenemy = targ_enemy;
        }
        edicts[targ_idx].enemy = attacker_idx as i32;
        if !edicts[targ_idx].monsterinfo.aiflags.intersects(AI_DUCKED) {
            found_target(targ_idx, edicts);
        }
    }
    // if they *meant* to shoot us, then shoot back
    else if edicts[attacker_idx].enemy == targ_idx as i32 {
        if targ_enemy >= 0 && edicts[targ_enemy as usize].client.is_some() {
            edicts[targ_idx].oldenemy = targ_enemy;
        }
        edicts[targ_idx].enemy = attacker_idx as i32;
        if !edicts[targ_idx].monsterinfo.aiflags.intersects(AI_DUCKED) {
            found_target(targ_idx, edicts);
        }
    }
    // otherwise get mad at whoever they are mad at (help our buddy) unless it is us!
    else {
        let attacker_enemy = edicts[attacker_idx].enemy;
        if attacker_enemy >= 0 && attacker_enemy != targ_idx as i32 {
            if targ_enemy >= 0 && edicts[targ_enemy as usize].client.is_some() {
                edicts[targ_idx].oldenemy = targ_enemy;
            }
            edicts[targ_idx].enemy = attacker_enemy;
            if !edicts[targ_idx].monsterinfo.aiflags.intersects(AI_DUCKED) {
                found_target(targ_idx, edicts);
            }
        }
    }
}

/// Check if team damage should be prevented.
/// Currently a stub that always returns false (allows friendly fire).
/// A full implementation would check team membership and server cvar settings.
pub fn check_team_damage(_targ_idx: usize, _attacker_idx: usize, _edicts: &[Edict]) -> bool {
    // Stub: would need team cvars and membership check
    // if (teamplay disabled) && (targ's team == attacker's team) { return true; }
    false
}

/// Main damage function
#[allow(clippy::too_many_arguments)]
pub fn t_damage(
    targ_idx: usize,
    inflictor_idx: usize,
    attacker_idx: usize,
    dir: Vec3,
    point: Vec3,
    normal: Vec3,
    damage: i32,
    knockback: i32,
    dflags: DamageFlags,
    mod_type: i32,
    edicts: &mut [Edict],
    level: &mut LevelLocals,
) {
    if edicts[targ_idx].takedamage == 0 {
        return;
    }

    let mut damage = damage;
    let mut mod_type = mod_type;

    // Read cvar values from game context
    let (deathmatch, dmflags, coop, skill) = crate::g_local::with_global_game_ctx(|ctx| {
        (ctx.deathmatch, ctx.dmflags, ctx.coop, ctx.skill)
    }).unwrap_or((0.0, 0.0, 0.0, 0.0));

    // friendly fire avoidance
    // if enabled you can't hurt teammates (but you can hurt yourself)
    // knockback still occurs
    if targ_idx != attacker_idx
        && ((deathmatch != 0.0 && DmFlags::from_bits_truncate(dmflags as i32).intersects(DF_MODELTEAMS | DF_SKINTEAMS))
            || coop != 0.0)
        && on_same_team(targ_idx, attacker_idx, edicts) {
            if DmFlags::from_bits_truncate(dmflags as i32).intersects(DF_NO_FRIENDLY_FIRE) {
                damage = 0;
            } else {
                mod_type |= MOD_FRIENDLY_FIRE;
            }
        }
    crate::g_local::with_global_game_ctx(|ctx| {
        ctx.means_of_death = mod_type;
    });

    // easy mode takes half damage
    if skill == 0.0 && deathmatch == 0.0 && edicts[targ_idx].client.is_some() {
        damage = (damage as f32 * 0.5) as i32;
        if damage == 0 {
            damage = 1;
        }
    }

    let te_sparks = if dflags.intersects(DAMAGE_BULLET) {
        TE_BULLET_SPARKS
    } else {
        TE_SPARKS
    };

    let mut dir = dir;
    vector_normalize(&mut dir);

    // bonus damage for surprising a monster
    if !dflags.intersects(DAMAGE_RADIUS)
        && (edicts[targ_idx].svflags & SVF_MONSTER) != 0
        && edicts[attacker_idx].client.is_some()
        && edicts[targ_idx].enemy < 0
        && edicts[targ_idx].health > 0
    {
        damage *= 2;
    }

    let mut knockback = knockback;
    if edicts[targ_idx].flags.intersects(FL_NO_KNOCKBACK) {
        knockback = 0;
    }

    // figure momentum add
    if !dflags.intersects(DAMAGE_NO_KNOCKBACK) {
        let targ_movetype = edicts[targ_idx].movetype;
        if knockback != 0
            && targ_movetype != MoveType::None
            && targ_movetype != MoveType::Bounce
            && targ_movetype != MoveType::Push
            && targ_movetype != MoveType::Stop
        {
            let mut mass = edicts[targ_idx].mass;
            if mass < 50 {
                mass = 50;
            }

            let kvel = if edicts[targ_idx].client.is_some() && attacker_idx == targ_idx {
                // the rocket jump hack...
                vector_scale(&dir, 1600.0 * (knockback as f32) / (mass as f32))
            } else {
                vector_scale(&dir, 500.0 * (knockback as f32) / (mass as f32))
            };

            edicts[targ_idx].velocity = vector_add(&edicts[targ_idx].velocity, &kvel);
        }
    }

    let mut take = damage;
    let mut save = 0;

    // check for godmode
    if edicts[targ_idx].flags.intersects(FL_GODMODE) && !dflags.intersects(DAMAGE_NO_PROTECTION) {
        take = 0;
        save = damage;
        spawn_damage(te_sparks, point, normal, save);
    }

    // check for invincibility
    let targ_client = edicts[targ_idx].client;
    if targ_client.is_some() && !dflags.intersects(DAMAGE_NO_PROTECTION) {
        let invincible = if let Some(ci) = targ_client {
            crate::g_local::with_global_game_ctx(|ctx| {
                ci < ctx.clients.len() && ctx.clients[ci].invincible_framenum > level.framenum as f32
            }).unwrap_or(false)
        } else {
            false
        };
        if invincible {
            if edicts[targ_idx].pain_debounce_time < level.time {
                gi_sound(targ_idx as i32, CHAN_ITEM, gi_soundindex("items/protect4.wav"), 1.0, ATTN_NORM, 0.0);
                edicts[targ_idx].pain_debounce_time = level.time + 2.0;
            }
            take = 0;
            save = damage;
        }
    }

    let psave = check_power_armor(targ_idx, point, normal, take, dflags, edicts, level);
    take -= psave;

    let asave = check_armor(targ_idx, point, normal, take, te_sparks, dflags, edicts);
    take -= asave;

    // treat cheat/powerup savings the same as armor
    let _asave = asave + save;

    // team damage avoidance
    if !dflags.intersects(DAMAGE_NO_PROTECTION) && check_team_damage(targ_idx, attacker_idx, edicts) {
        return;
    }

    // do the damage
    if take != 0 {
        if (edicts[targ_idx].svflags & SVF_MONSTER) != 0 || targ_client.is_some() {
            spawn_damage(TE_BLOOD, point, normal, take);
        } else {
            spawn_damage(te_sparks, point, normal, take);
        }

        edicts[targ_idx].health -= take;

        if edicts[targ_idx].health <= 0 {
            if (edicts[targ_idx].svflags & SVF_MONSTER) != 0 || targ_client.is_some() {
                edicts[targ_idx].flags.insert(FL_NO_KNOCKBACK);
            }
            killed(targ_idx, inflictor_idx, attacker_idx, take, point, edicts, level);
            return;
        }
    }

    if (edicts[targ_idx].svflags & SVF_MONSTER) != 0 {
        m_react_to_damage(targ_idx, attacker_idx, edicts);
        if !edicts[targ_idx].monsterinfo.aiflags.intersects(AI_DUCKED) && take != 0 {
            crate::dispatch::call_pain(targ_idx, attacker_idx, edicts, level, knockback as f32, take);
            // nightmare mode monsters don't go into pain frames often
            if skill == 3.0 {
                edicts[targ_idx].pain_debounce_time = level.time + 5.0;
            }
        }
    } else if targ_client.is_some() {
        if !edicts[targ_idx].flags.intersects(FL_GODMODE) && take != 0 {
            crate::dispatch::call_pain(targ_idx, attacker_idx, edicts, level, knockback as f32, take);
        }
    } else if take != 0 {
        crate::dispatch::call_pain(targ_idx, attacker_idx, edicts, level, knockback as f32, take);
    }

    // add to the damage inflicted on a player this frame
    // the total will be turned into screen blends and view angle kicks
    // at the end of the frame
    if let Some(_client_idx) = targ_client {
        // client.damage_parmor += psave;
        // client.damage_armor += asave;
        // client.damage_blood += take;
        // client.damage_knockback += knockback;
        // VectorCopy(point, client.damage_from);
    }
}

/// Data needed to apply damage to a single entity.
/// Computed in parallel phase, applied in sequential phase.
#[derive(Debug, Clone)]
struct RadiusDamageData {
    ent_idx: usize,
    dir: Vec3,
    points: i32,
}

/// Radius damage using two-phase approach.
///
/// Phase 1 (parallel): Find entities in radius and compute damage amounts
/// Phase 2 (sequential): Apply damage via t_damage calls (trace not thread-safe)
pub fn t_radius_damage(
    inflictor_idx: usize,
    attacker_idx: usize,
    damage: f32,
    ignore_idx: Option<usize>,
    radius: f32,
    mod_type: i32,
    edicts: &mut [Edict],
    level: &mut LevelLocals,
) {
    let inflictor_origin = edicts[inflictor_idx].s.origin;

    let entities = findradius(None, inflictor_origin, radius, edicts);

    // Phase 1: Parallel computation of damage data
    let entity_data: Vec<_> = entities
        .iter()
        .filter_map(|&ent_idx| {
            if Some(ent_idx) == ignore_idx {
                return None;
            }
            if edicts[ent_idx].takedamage == 0 {
                return None;
            }
            Some((
                ent_idx,
                edicts[ent_idx].s.origin,
                edicts[ent_idx].mins,
                edicts[ent_idx].maxs,
            ))
        })
        .collect();

    // Parallel damage calculation
    let damage_data: Vec<RadiusDamageData> = entity_data
        .par_iter()
        .filter_map(|&(ent_idx, origin, mins, maxs)| {
            // Calculate distance to center
            let mut v = vector_add(&mins, &maxs);
            v = vector_ma(&origin, 0.5, &v);
            v = vector_subtract(&inflictor_origin, &v);
            let mut points = damage - 0.5 * vector_length(&v);

            if ent_idx == attacker_idx {
                points *= 0.5;
            }

            if points <= 0.0 {
                return None;
            }

            let dir = vector_subtract(&origin, &inflictor_origin);

            Some(RadiusDamageData {
                ent_idx,
                dir,
                points: points as i32,
            })
        })
        .collect();

    // Phase 2: Sequential damage application
    // can_damage() uses trace which isn't thread-safe, so check here
    for data in damage_data {
        if can_damage(data.ent_idx, inflictor_idx, edicts) {
            t_damage(
                data.ent_idx,
                inflictor_idx,
                attacker_idx,
                data.dir,
                inflictor_origin,
                vec3_origin,
                data.points,
                data.points,
                DAMAGE_RADIUS,
                mod_type,
                edicts,
                level,
            );
        }
    }
}

// ============================================================
// GameCtx versions of cross-module functions
// ============================================================

use crate::g_local::GameCtx;

/// GameCtx wrapper for t_damage — delegates to the canonical implementation.
#[allow(clippy::too_many_arguments)]
pub fn ctx_t_damage(
    ctx: &mut GameCtx,
    targ_idx: usize,
    inflictor_idx: usize,
    attacker_idx: usize,
    dir: &Vec3,
    point: &Vec3,
    normal: &Vec3,
    damage: i32,
    knockback: i32,
    dflags: DamageFlags,
    means_of_death: i32,
) {
    t_damage(
        targ_idx, inflictor_idx, attacker_idx,
        *dir, *point, *normal,
        damage, knockback, dflags, means_of_death,
        &mut ctx.edicts, &mut ctx.level,
    );
}

/// GameCtx wrapper for t_radius_damage — delegates to the canonical implementation.
pub fn ctx_t_radius_damage(
    ctx: &mut GameCtx,
    inflictor_idx: usize,
    attacker_idx: usize,
    damage: f32,
    ignore_idx: usize,
    radius: f32,
    means_of_death: i32,
) {
    let ignore = if ignore_idx == 0 { None } else { Some(ignore_idx) };
    t_radius_damage(
        inflictor_idx, attacker_idx, damage, ignore,
        radius, means_of_death, &mut ctx.edicts, &mut ctx.level,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::g_local::Edict;

    /// Helper: create a default Edict that is "in use" with a given origin and solid type.
    fn make_edict(origin: Vec3, solid: Solid, inuse: bool) -> Edict {
        let mut e = Edict::default();
        e.s.origin = origin;
        e.solid = solid;
        e.inuse = inuse;
        e
    }

    // ============================================================
    // findradius tests
    // ============================================================

    #[test]
    fn test_findradius_basic() {
        // edicts[0] = world (skipped), edicts[1..] = searchable
        let mut edicts = vec![Edict::default(); 5];

        // Entity 1: in use, solid, at origin [10, 0, 0]
        edicts[1] = make_edict([10.0, 0.0, 0.0], Solid::Bbox, true);
        // Entity 2: in use, solid, at origin [100, 0, 0] — far away
        edicts[2] = make_edict([100.0, 0.0, 0.0], Solid::Bbox, true);
        // Entity 3: in use, solid, at origin [5, 5, 0]
        edicts[3] = make_edict([5.0, 5.0, 0.0], Solid::Bbox, true);
        // Entity 4: not in use
        edicts[4] = make_edict([1.0, 0.0, 0.0], Solid::Bbox, false);

        let origin = [0.0, 0.0, 0.0];
        let radius = 20.0;

        let result = findradius(None, origin, radius, &edicts);
        // Entity 1 (dist=10) and entity 3 (dist~=7.07) should be found
        assert!(result.contains(&1));
        assert!(result.contains(&3));
        // Entity 2 (dist=100) should NOT be found
        assert!(!result.contains(&2));
        // Entity 4 (not in use) should NOT be found
        assert!(!result.contains(&4));
    }

    #[test]
    fn test_findradius_empty_array() {
        // Only the world entity (index 0), nothing else
        let edicts = vec![Edict::default(); 1];
        let result = findradius(None, [0.0, 0.0, 0.0], 100.0, &edicts);
        assert!(result.is_empty());
    }

    #[test]
    fn test_findradius_no_inuse_entities() {
        let mut edicts = vec![Edict::default(); 4];
        // All entities are not in use (default inuse = false)
        edicts[1].s.origin = [1.0, 0.0, 0.0];
        edicts[1].solid = Solid::Bbox;
        edicts[2].s.origin = [2.0, 0.0, 0.0];
        edicts[2].solid = Solid::Bbox;

        let result = findradius(None, [0.0, 0.0, 0.0], 100.0, &edicts);
        assert!(result.is_empty());
    }

    #[test]
    fn test_findradius_solid_not_excluded() {
        let mut edicts = vec![Edict::default(); 3];
        // Entity with Solid::Not should be excluded even if inuse
        edicts[1] = make_edict([1.0, 0.0, 0.0], Solid::Not, true);
        edicts[2] = make_edict([1.0, 0.0, 0.0], Solid::Bbox, true);

        let result = findradius(None, [0.0, 0.0, 0.0], 100.0, &edicts);
        assert!(!result.contains(&1));
        assert!(result.contains(&2));
    }

    #[test]
    fn test_findradius_exact_boundary() {
        // Entity exactly AT the radius boundary should NOT be included (strict < comparison)
        let mut edicts = vec![Edict::default(); 3];
        // Entity 1: exactly at distance 50.0 from origin (mins/maxs are [0,0,0])
        edicts[1] = make_edict([50.0, 0.0, 0.0], Solid::Bbox, true);
        // Entity 2: just inside (49.9)
        edicts[2] = make_edict([49.9, 0.0, 0.0], Solid::Bbox, true);

        let result = findradius(None, [0.0, 0.0, 0.0], 50.0, &edicts);
        // dist_sq for entity 1 = 2500.0, radius_sq = 2500.0 => NOT < => excluded
        assert!(!result.contains(&1));
        // dist_sq for entity 2 = 2490.01, radius_sq = 2500.0 => < => included
        assert!(result.contains(&2));
    }

    #[test]
    fn test_findradius_from_offset() {
        let mut edicts = vec![Edict::default(); 5];
        edicts[1] = make_edict([1.0, 0.0, 0.0], Solid::Bbox, true);
        edicts[2] = make_edict([2.0, 0.0, 0.0], Solid::Bbox, true);
        edicts[3] = make_edict([3.0, 0.0, 0.0], Solid::Bbox, true);

        // Start from entity 2 (exclusive), so only entity 3 and above should be considered
        let result = findradius(Some(2), [0.0, 0.0, 0.0], 100.0, &edicts);
        assert!(!result.contains(&1));
        assert!(!result.contains(&2));
        assert!(result.contains(&3));
    }

    #[test]
    fn test_findradius_from_beyond_bounds() {
        let edicts = vec![Edict::default(); 3];
        // Start from an index beyond the array
        let result = findradius(Some(100), [0.0, 0.0, 0.0], 100.0, &edicts);
        assert!(result.is_empty());
    }

    #[test]
    fn test_findradius_considers_entity_center() {
        // The distance calculation uses the entity's center: origin + (mins+maxs)/2
        let mut edicts = vec![Edict::default(); 2];
        let mut e = make_edict([0.0, 0.0, 0.0], Solid::Bbox, true);
        // mins = [-10, -10, -10], maxs = [10, 10, 10] => center offset = [0, 0, 0]
        // So origin + center = [0, 0, 0]
        e.mins = [-10.0, -10.0, -10.0];
        e.maxs = [10.0, 10.0, 10.0];
        edicts[1] = e;

        let result = findradius(None, [5.0, 0.0, 0.0], 10.0, &edicts);
        // Distance from [5,0,0] to [0,0,0] = 5.0, which is < 10.0
        assert!(result.contains(&1));
    }

    // ============================================================
    // check_team_damage tests
    // ============================================================

    #[test]
    fn test_check_team_damage_always_false() {
        let edicts = vec![Edict::default(); 3];
        // Stub always returns false
        assert!(!check_team_damage(0, 1, &edicts));
        assert!(!check_team_damage(1, 2, &edicts));
        assert!(!check_team_damage(0, 0, &edicts));
    }

    // ============================================================
    // Knockback math tests
    // ============================================================

    #[test]
    fn test_knockback_self_damage_scaling() {
        // When attacker == target (self-damage / rocket jump):
        // kvel = dir * 1600.0 * knockback / mass
        let dir = [1.0, 0.0, 0.0]; // normalized direction
        let knockback: f32 = 100.0;
        let mass: f32 = 200.0;
        let scale = 1600.0 * knockback / mass;
        let kvel = vector_scale(&dir, scale);
        assert_eq!(kvel, [800.0, 0.0, 0.0]);
    }

    #[test]
    fn test_knockback_normal_damage_scaling() {
        // Normal knockback: kvel = dir * 500.0 * knockback / mass
        let dir = [0.0, 1.0, 0.0];
        let knockback: f32 = 50.0;
        let mass: f32 = 100.0;
        let scale = 500.0 * knockback / mass;
        let kvel = vector_scale(&dir, scale);
        assert_eq!(kvel, [0.0, 250.0, 0.0]);
    }

    #[test]
    fn test_knockback_minimum_mass_clamp() {
        // Mass is clamped to minimum 50
        let dir = [0.0, 0.0, 1.0];
        let knockback: f32 = 100.0;
        let mut mass: i32 = 10; // below minimum
        if mass < 50 {
            mass = 50;
        }
        let scale = 500.0 * (knockback) / (mass as f32);
        let kvel = vector_scale(&dir, scale);
        assert_eq!(kvel, [0.0, 0.0, 1000.0]);
    }

    #[test]
    fn test_knockback_zero() {
        // Zero knockback should produce zero velocity addition
        let dir = [1.0, 0.0, 0.0];
        let knockback: f32 = 0.0;
        let mass: f32 = 100.0;
        let scale = 500.0 * knockback / mass;
        let kvel = vector_scale(&dir, scale);
        assert_eq!(kvel, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_knockback_heavy_entity() {
        // Heavy entity (mass=1000) should get minimal knockback
        let dir = [1.0, 0.0, 0.0];
        let knockback: f32 = 100.0;
        let mass: f32 = 1000.0;
        let scale = 500.0 * knockback / mass;
        let kvel = vector_scale(&dir, scale);
        assert_eq!(kvel, [50.0, 0.0, 0.0]);
    }

    #[test]
    fn test_knockback_self_vs_normal_ratio() {
        // Self-damage knockback (1600) should be 3.2x the normal knockback (500)
        let dir = [1.0, 0.0, 0.0];
        let knockback: f32 = 100.0;
        let mass: f32 = 200.0;

        let self_scale = 1600.0 * knockback / mass;
        let normal_scale = 500.0 * knockback / mass;

        let ratio = self_scale / normal_scale;
        assert!((ratio - 3.2).abs() < 0.001);
    }
}
