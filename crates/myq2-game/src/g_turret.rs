// g_turret.rs — Turret entities
// Converted from: myq2-original/game/g_turret.c
//
// Copyright (C) 1997-2001 Id Software, Inc.
// Licensed under the GNU General Public License v2.

use crate::g_local::*;
use crate::game::*;
use crate::game_import::*;
use myq2_common::q_shared::{Vec3, PITCH, YAW, MASK_MONSTERSOLID, RF_FRAMELERP, vector_length, angle_vectors_tuple, vectoangles_tuple};

use std::f32::consts::PI;

// ============================================================
// Helper functions
// ============================================================

/// Normalize angles to [0, 360) range for indices 0 and 1.
pub fn angles_normalize(vec: &mut Vec3) {
    while vec[0] > 360.0 {
        vec[0] -= 360.0;
    }
    while vec[0] < 0.0 {
        vec[0] += 360.0;
    }
    while vec[1] > 360.0 {
        vec[1] -= 360.0;
    }
    while vec[1] < 0.0 {
        vec[1] += 360.0;
    }
}

/// Snap a float to nearest 1/8 value.
pub fn snap_to_eights(x: f32) -> f32 {
    let x = x * 8.0;
    let x = if x > 0.0 { x + 0.5 } else { x - 0.5 };
    0.125 * (x as i32 as f32)
}

// ============================================================
// Turret blocked callback
// ============================================================

/// Called when a turret entity is blocked by another entity.
/// `self_idx` and `other_idx` are entity indices into `edicts`.
pub fn turret_blocked(edicts: &mut Vec<Edict>, level: &mut LevelLocals, self_idx: usize, other_idx: usize) {
    let other_takedamage = edicts[other_idx].takedamage;
    if other_takedamage != 0 {
        let teammaster_idx = edicts[self_idx].teammaster as usize;
        let attacker_idx = {
            let tm_owner = edicts[teammaster_idx].owner;
            if tm_owner >= 0 {
                tm_owner as usize
            } else {
                teammaster_idx
            }
        };
        let dmg = edicts[teammaster_idx].dmg;
        let other_origin = edicts[other_idx].s.origin;
        crate::g_combat::t_damage(
            other_idx, self_idx, attacker_idx,
            [0.0; 3], other_origin, [0.0; 3],
            dmg, 10, DamageFlags::empty(), MOD_CRUSH,
            edicts, level,
        );
    }
}

// ============================================================
// turret_breach (pitch + yaw turret)
// ============================================================

/// Fire the turret breach weapon (rocket).
pub fn turret_breach_fire(edicts: &mut Vec<Edict>, level: &mut LevelLocals, skill_value: f32, self_idx: usize) {
    let angles = edicts[self_idx].s.angles;
    let origin = edicts[self_idx].s.origin;
    let move_origin = edicts[self_idx].move_origin;

    // AngleVectors
    let (f, r, u) = angle_vectors_tuple(&angles);

    // VectorMA chain to compute start
    let mut start = [0.0f32; 3];
    for i in 0..3 {
        start[i] = origin[i] + move_origin[0] * f[i];
    }
    for i in 0..3 {
        start[i] += move_origin[1] * r[i];
    }
    for i in 0..3 {
        start[i] += move_origin[2] * u[i];
    }

    let damage = 100 + (rand_float() * 50.0) as i32;
    let speed = 550 + (50.0 * skill_value) as i32;

    let teammaster_idx = edicts[self_idx].teammaster as usize;
    let owner_idx = edicts[teammaster_idx].owner;

    crate::g_weapon::fire_rocket(
        owner_idx as usize, edicts, level, &start, &f, damage, speed, 150.0, damage,
    );
    let snd = gi_soundindex("weapons/rocklf1a.wav");
    gi_positioned_sound(&start, self_idx as i32, CHAN_WEAPON, snd, 1.0, ATTN_NORM as f32, 0.0);
}

/// Main think function for turret_breach entities.
pub fn turret_breach_think(edicts: &mut Vec<Edict>, level: &mut LevelLocals, skill_value: f32, self_idx: usize) {
    let mut current_angles = edicts[self_idx].s.angles;
    angles_normalize(&mut current_angles);

    angles_normalize(&mut edicts[self_idx].move_angles);
    if edicts[self_idx].move_angles[PITCH] > 180.0 {
        edicts[self_idx].move_angles[PITCH] -= 360.0;
    }

    // Clamp pitch to mins & maxs
    if edicts[self_idx].move_angles[PITCH] > edicts[self_idx].pos1[PITCH] {
        edicts[self_idx].move_angles[PITCH] = edicts[self_idx].pos1[PITCH];
    } else if edicts[self_idx].move_angles[PITCH] < edicts[self_idx].pos2[PITCH] {
        edicts[self_idx].move_angles[PITCH] = edicts[self_idx].pos2[PITCH];
    }

    // Clamp yaw to mins & maxs
    let move_yaw = edicts[self_idx].move_angles[YAW];
    let pos1_yaw = edicts[self_idx].pos1[YAW];
    let pos2_yaw = edicts[self_idx].pos2[YAW];
    if move_yaw < pos1_yaw || move_yaw > pos2_yaw {
        let mut dmin = (pos1_yaw - move_yaw).abs();
        if dmin < -180.0 {
            dmin += 360.0;
        } else if dmin > 180.0 {
            dmin -= 360.0;
        }
        let mut dmax = (pos2_yaw - move_yaw).abs();
        if dmax < -180.0 {
            dmax += 360.0;
        } else if dmax > 180.0 {
            dmax -= 360.0;
        }
        if dmin.abs() < dmax.abs() {
            edicts[self_idx].move_angles[YAW] = pos1_yaw;
        } else {
            edicts[self_idx].move_angles[YAW] = pos2_yaw;
        }
    }

    // delta = move_angles - current_angles
    let mut delta = [0.0f32; 3];
    for i in 0..3 {
        delta[i] = edicts[self_idx].move_angles[i] - current_angles[i];
    }
    if delta[0] < -180.0 {
        delta[0] += 360.0;
    } else if delta[0] > 180.0 {
        delta[0] -= 360.0;
    }
    if delta[1] < -180.0 {
        delta[1] += 360.0;
    } else if delta[1] > 180.0 {
        delta[1] -= 360.0;
    }
    delta[2] = 0.0;

    let speed = edicts[self_idx].speed;
    if delta[0] > speed * FRAMETIME {
        delta[0] = speed * FRAMETIME;
    }
    if delta[0] < -speed * FRAMETIME {
        delta[0] = -speed * FRAMETIME;
    }
    if delta[1] > speed * FRAMETIME {
        delta[1] = speed * FRAMETIME;
    }
    if delta[1] < -speed * FRAMETIME {
        delta[1] = -speed * FRAMETIME;
    }

    // VectorScale(delta, 1.0/FRAMETIME, self->avelocity)
    for i in 0..3 {
        edicts[self_idx].avelocity[i] = delta[i] * (1.0 / FRAMETIME);
    }

    edicts[self_idx].nextthink = level.time + FRAMETIME;

    // Propagate yaw avelocity to all team members
    let avelocity_yaw = edicts[self_idx].avelocity[1];
    let mut ent_idx = edicts[self_idx].teammaster;
    while ent_idx >= 0 {
        edicts[ent_idx as usize].avelocity[1] = avelocity_yaw;
        ent_idx = edicts[ent_idx as usize].teamchain;
    }

    // If we have a driver, adjust their velocities
    let owner_idx = edicts[self_idx].owner;
    if owner_idx >= 0 {
        let oi = owner_idx as usize;

        // Angular: copy ours
        edicts[oi].avelocity[0] = edicts[self_idx].avelocity[0];
        edicts[oi].avelocity[1] = edicts[self_idx].avelocity[1];

        // x & y
        let angle_deg = edicts[self_idx].s.angles[1] + edicts[oi].move_origin[1];
        let angle_rad = angle_deg * (PI * 2.0 / 360.0);
        let self_origin = edicts[self_idx].s.origin;
        let owner_move_origin = edicts[oi].move_origin;

        let target_x = snap_to_eights(self_origin[0] + angle_rad.cos() * owner_move_origin[0]);
        let target_y = snap_to_eights(self_origin[1] + angle_rad.sin() * owner_move_origin[0]);
        let _target_z = edicts[oi].s.origin[2];

        let dir_x = target_x - edicts[oi].s.origin[0];
        let dir_y = target_y - edicts[oi].s.origin[1];
        edicts[oi].velocity[0] = dir_x * 1.0 / FRAMETIME;
        edicts[oi].velocity[1] = dir_y * 1.0 / FRAMETIME;

        // z
        let pitch_rad = edicts[self_idx].s.angles[PITCH] * (PI * 2.0 / 360.0);
        let target_z2 = snap_to_eights(
            self_origin[2] + owner_move_origin[0] * pitch_rad.tan() + owner_move_origin[2],
        );

        let diff = target_z2 - edicts[oi].s.origin[2];
        edicts[oi].velocity[2] = diff * 1.0 / FRAMETIME;

        if edicts[self_idx].spawnflags & 65536 != 0 {
            turret_breach_fire(edicts, level, skill_value, self_idx);
            edicts[self_idx].spawnflags &= !65536;
        }
    }
}

/// Finish initialization of turret_breach entity (called one frame after spawn).
pub fn turret_breach_finish_init(edicts: &mut Vec<Edict>, _level: &LevelLocals, self_idx: usize) {
    let target = edicts[self_idx].target.clone();
    if target.is_empty() {
        let classname = edicts[self_idx].classname.clone();
        let origin = edicts[self_idx].s.origin;
        gi_dprintf(&format!("{} at {:?} needs a target\n", classname, origin));
    } else {
        // G_PickTarget deferred: requires GameContext not available here
        // Caller must set target_ent before calling this function
        let target_ent_idx = edicts[self_idx].target_ent as usize;
        // VectorSubtract(self->target_ent->s.origin, self->s.origin, self->move_origin)
        let te_origin = edicts[target_ent_idx].s.origin;
        let self_origin = edicts[self_idx].s.origin;
        edicts[self_idx].move_origin = [
            te_origin[0] - self_origin[0],
            te_origin[1] - self_origin[1],
            te_origin[2] - self_origin[2],
        ];
        // G_FreeEdict(target_ent) deferred: requires GameContext
        // Caller should free the target_ent after this returns
    }

    let dmg = edicts[self_idx].dmg;
    let teammaster_idx = edicts[self_idx].teammaster as usize;
    edicts[teammaster_idx].dmg = dmg;

    // self->think = turret_breach_think; self->think(self);
    // The caller should set the think callback and invoke turret_breach_think.
}

/// Spawn function for turret_breach entity.
pub fn sp_turret_breach(edicts: &mut Vec<Edict>, level: &LevelLocals, st: &mut SpawnTemp, self_idx: usize) {
    edicts[self_idx].solid = Solid::Bsp;
    edicts[self_idx].movetype = MoveType::Push;
    gi_setmodel(self_idx as i32, &edicts[self_idx].model.clone());

    if edicts[self_idx].speed == 0.0 {
        edicts[self_idx].speed = 50.0;
    }
    if edicts[self_idx].dmg == 0 {
        edicts[self_idx].dmg = 10;
    }

    if st.minpitch == 0.0 {
        st.minpitch = -30.0;
    }
    if st.maxpitch == 0.0 {
        st.maxpitch = 30.0;
    }
    if st.maxyaw == 0.0 {
        st.maxyaw = 360.0;
    }

    edicts[self_idx].pos1[PITCH] = -st.minpitch;
    edicts[self_idx].pos1[YAW] = st.minyaw;
    edicts[self_idx].pos2[PITCH] = -st.maxpitch;
    edicts[self_idx].pos2[YAW] = st.maxyaw;

    edicts[self_idx].ideal_yaw = edicts[self_idx].s.angles[YAW];
    edicts[self_idx].move_angles[YAW] = edicts[self_idx].s.angles[YAW];

    // self->blocked = turret_blocked (set via callback index by caller)
    // self->think = turret_breach_finish_init (set via callback index by caller)
    edicts[self_idx].nextthink = level.time + FRAMETIME;
    gi_linkentity(self_idx as i32);
}

// ============================================================
// turret_base (yaw only)
// ============================================================

/// Spawn function for turret_base entity.
pub fn sp_turret_base(edicts: &mut Vec<Edict>, _level: &LevelLocals, self_idx: usize) {
    edicts[self_idx].solid = Solid::Bsp;
    edicts[self_idx].movetype = MoveType::Push;
    gi_setmodel(self_idx as i32, &edicts[self_idx].model.clone());
    // self->blocked = turret_blocked (set via callback index by caller)
    gi_linkentity(self_idx as i32);
}

// ============================================================
// turret_driver
// ============================================================

/// Death function for turret driver.
pub fn turret_driver_die(
    edicts: &mut Vec<Edict>,
    level: &mut LevelLocals,
    self_idx: usize,
    inflictor_idx: usize,
    attacker_idx: usize,
    damage: i32,
    point: Vec3,
) {
    let target_ent_idx = edicts[self_idx].target_ent as usize;

    // Level the gun
    edicts[target_ent_idx].move_angles[0] = 0.0;

    // Remove the driver from the end of the team chain
    let teammaster_idx = edicts[target_ent_idx].teammaster as usize;
    let mut ent_idx = teammaster_idx;
    loop {
        let next = edicts[ent_idx].teamchain;
        if next == self_idx as i32 {
            break;
        }
        ent_idx = next as usize;
    }
    edicts[ent_idx].teamchain = -1;
    edicts[self_idx].teammaster = -1;
    edicts[self_idx].flags.remove(FL_TEAMSLAVE);

    edicts[target_ent_idx].owner = -1;
    let te_teammaster = edicts[target_ent_idx].teammaster as usize;
    edicts[te_teammaster].owner = -1;

    // Call infantry_die via dispatch (die callback on the entity)
    if let Some(die_fn) = edicts[self_idx].die_fn {
        crate::dispatch::dispatch_die(die_fn, self_idx, inflictor_idx, attacker_idx, edicts, level, damage, point);
    }
}

/// Think function for turret driver — find targets and aim the turret.
pub fn turret_driver_think(
    edicts: &mut Vec<Edict>,
    level: &LevelLocals,
    skill_value: f32,
    self_idx: usize,
) {
    edicts[self_idx].nextthink = level.time + FRAMETIME;

    // Check if enemy is still valid
    let enemy_idx = edicts[self_idx].enemy;
    if enemy_idx >= 0 {
        let ei = enemy_idx as usize;
        if !edicts[ei].inuse || edicts[ei].health <= 0 {
            edicts[self_idx].enemy = -1;
        }
    }

    if edicts[self_idx].enemy < 0 {
        // FindTarget deferred: requires AiContext not available here
        // Caller should invoke find_target before this function
        edicts[self_idx].monsterinfo.trail_time = level.time;
        edicts[self_idx].monsterinfo.aiflags.remove(AI_LOST_SIGHT);
    } else {
        let enemy_idx = edicts[self_idx].enemy as usize;
        let is_visible = crate::g_ai::visible(&edicts[self_idx], &edicts[enemy_idx]);
        if is_visible {
            if edicts[self_idx].monsterinfo.aiflags.intersects(AI_LOST_SIGHT) {
                edicts[self_idx].monsterinfo.trail_time = level.time;
                edicts[self_idx].monsterinfo.aiflags.remove(AI_LOST_SIGHT);
            }
        } else {
            edicts[self_idx].monsterinfo.aiflags.insert(AI_LOST_SIGHT);
            return;
        }
    }

    // Let the turret know where we want it to aim
    let enemy_idx = edicts[self_idx].enemy;
    if enemy_idx >= 0 {
        let ei = enemy_idx as usize;
        let target_ent_idx = edicts[self_idx].target_ent as usize;
        let mut target = edicts[ei].s.origin;
        target[2] += edicts[ei].viewheight as f32;
        let te_origin = edicts[target_ent_idx].s.origin;
        let dir = [
            target[0] - te_origin[0],
            target[1] - te_origin[1],
            target[2] - te_origin[2],
        ];
        // vectoangles(dir, self->target_ent->move_angles)
        edicts[target_ent_idx].move_angles = vectoangles_tuple(&dir);
    }

    // Decide if we should shoot
    if level.time < edicts[self_idx].monsterinfo.attack_finished {
        return;
    }

    let reaction_time = (3.0 - skill_value) * 1.0;
    if (level.time - edicts[self_idx].monsterinfo.trail_time) < reaction_time {
        return;
    }

    edicts[self_idx].monsterinfo.attack_finished = level.time + reaction_time + 1.0;
    // Signal target entity via spawnflags (original Q2 behavior)
    let target_ent_idx = edicts[self_idx].target_ent as usize;
    edicts[target_ent_idx].spawnflags |= 65536;
}

/// Link function for turret driver (called one frame after spawn).
pub fn turret_driver_link(edicts: &mut Vec<Edict>, level: &LevelLocals, self_idx: usize) {
    // self->think = turret_driver_think (set by caller)
    edicts[self_idx].nextthink = level.time + FRAMETIME;

    // G_PickTarget deferred: requires GameContext
    // Caller must set target_ent before calling this function
    let target_ent_idx = edicts[self_idx].target_ent as usize;

    edicts[target_ent_idx].owner = self_idx as i32;
    let te_teammaster = edicts[target_ent_idx].teammaster as usize;
    edicts[te_teammaster].owner = self_idx as i32;

    // Copy target_ent angles to self
    edicts[self_idx].s.angles = edicts[target_ent_idx].s.angles;

    // Compute distance and angle offset for driver positioning
    let te_origin = edicts[target_ent_idx].s.origin;
    let self_origin = edicts[self_idx].s.origin;
    let mut vec = [
        te_origin[0] - self_origin[0],
        te_origin[1] - self_origin[1],
        0.0,
    ];
    edicts[self_idx].move_origin[0] = vector_length(&vec);

    vec = [
        self_origin[0] - te_origin[0],
        self_origin[1] - te_origin[1],
        self_origin[2] - te_origin[2],
    ];
    let mut angles = vectoangles_tuple(&vec);
    angles_normalize(&mut angles);
    edicts[self_idx].move_origin[1] = angles[1];

    edicts[self_idx].move_origin[2] = self_origin[2] - te_origin[2];

    // Add the driver to the end of the team chain
    let mut ent_idx = te_teammaster;
    loop {
        let next = edicts[ent_idx].teamchain;
        if next < 0 {
            break;
        }
        ent_idx = next as usize;
    }
    edicts[ent_idx].teamchain = self_idx as i32;
    edicts[self_idx].teammaster = te_teammaster as i32;
    edicts[self_idx].flags.insert(FL_TEAMSLAVE);
}

/// Spawn function for turret_driver entity.
pub fn sp_turret_driver(
    edicts: &mut Vec<Edict>,
    level: &mut LevelLocals,
    st: &SpawnTemp,
    deathmatch_value: f32,
    self_idx: usize,
) {
    if deathmatch_value != 0.0 {
        // G_FreeEdict deferred: requires GameContext. Caller should free entity.
        return;
    }

    edicts[self_idx].movetype = MoveType::Push;
    edicts[self_idx].solid = Solid::Bbox;
    edicts[self_idx].s.modelindex = gi_modelindex("models/monsters/infantry/tris.md2");

    edicts[self_idx].mins = [-16.0, -16.0, -24.0];
    edicts[self_idx].maxs = [16.0, 16.0, 32.0];

    edicts[self_idx].health = 100;
    edicts[self_idx].gib_health = 0;
    edicts[self_idx].mass = 200;
    edicts[self_idx].viewheight = 24;

    // self->die = turret_driver_die (set via callback index by caller)
    // self->monsterinfo.stand = infantry_stand (set via callback index by caller)

    edicts[self_idx].flags.insert(FL_NO_KNOCKBACK);

    level.total_monsters += 1;

    edicts[self_idx].svflags |= SVF_MONSTER;
    edicts[self_idx].s.renderfx |= RF_FRAMELERP;
    edicts[self_idx].takedamage = Damage::Aim as i32;
    // self->use = monster_use (set via callback index by caller)
    edicts[self_idx].clipmask = MASK_MONSTERSOLID;
    edicts[self_idx].s.old_origin = edicts[self_idx].s.origin;
    edicts[self_idx].monsterinfo.aiflags.insert(AI_STAND_GROUND | AI_DUCKED);

    if !st.item.is_empty() {
        // FindItemByClassname deferred: requires GameContext
        // If not found:
        let classname = edicts[self_idx].classname.clone();
        let origin = edicts[self_idx].s.origin;
        gi_dprintf(&format!(
            "{} at {:?} has bad item: {}\n",
            classname, origin, st.item
        ));
    }

    // self->think = turret_driver_link (set via callback index by caller)
    edicts[self_idx].nextthink = level.time + FRAMETIME;

    gi_linkentity(self_idx as i32);
}

// ============================================================
// Utility helpers (placeholders for cross-module functions)
// ============================================================

/// Random float in [0, 1).
fn rand_float() -> f32 {
    (rand::random::<u16>() & 0x7fff) as f32 / 0x7fff as f32
}
