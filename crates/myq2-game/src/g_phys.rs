// g_phys.rs — Entity physics
// Converted from: myq2-original/game/g_phys.c

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

/*
pushmove objects do not obey gravity, and do not interact with each other or trigger fields,
but block normal movement and push normal objects when they move.

onground is set for toss objects when they come to a complete rest. it is set for stepping
or walking objects

doors, plats, etc are SOLID_BSP, and MOVETYPE_PUSH
bonus items are SOLID_TRIGGER touch, and MOVETYPE_TOSS
corpses are SOLID_NOT and MOVETYPE_TOSS
crates are SOLID_BBOX and MOVETYPE_TOSS
walking monsters are SOLID_SLIDEBOX and MOVETYPE_STEP
flying/floating monsters are SOLID_SLIDEBOX and MOVETYPE_FLY

solid_edge items only clip against bsp models.
*/

use myq2_common::q_shared::{
    angle_vectors, cross_product, dot_product, vector_add, vector_compare, vector_copy,
    vector_ma_to, vector_scale, vector_subtract, Trace, Vec3, VEC3_ORIGIN,
    MASK_MONSTERSOLID, MASK_SOLID, MASK_WATER, MAX_EDICTS, MAX_CLIP_PLANES,
};

use crate::g_local::{Edict, LevelLocals, MoveType, FRAMETIME, FL_FLY, FL_SWIM, FL_TEAMSLAVE};
use crate::game::{Solid, SVF_MONSTER};
// ============================================================
// Physics constants
// ============================================================

const STOP_EPSILON: f32 = 0.1;

// Physics constants (values established in original Q2 release)
const SV_STOPSPEED: f32 = 100.0;
const SV_FRICTION: f32 = 6.0;
const SV_WATERFRICTION: f32 = 1.0;

// ============================================================
// Pushed entity tracking
// ============================================================

#[derive(Debug, Clone, Copy, Default)]
struct Pushed {
    ent: i32,           // entity index
    origin: Vec3,
    angles: Vec3,
    deltayaw: f32,
}

/// Tracks entity positions during a push operation so they can be
/// rolled back if the push is blocked.
struct PushState {
    pushed: [Pushed; MAX_EDICTS],
    pushed_p: usize,
    obstacle: i32,
}

impl PushState {
    fn new() -> Self {
        Self {
            pushed: [Pushed {
                ent: -1,
                origin: [0.0; 3],
                angles: [0.0; 3],
                deltayaw: 0.0,
            }; MAX_EDICTS],
            pushed_p: 0,
            obstacle: -1,
        }
    }
}

// ============================================================
// Forward declarations (placeholders)
// ============================================================

// Cross-module wrappers — these delegate to the real implementations
// using the standalone _raw functions that accept (edicts, level) directly.

fn phys_m_check_ground(ent_idx: usize, edicts: &mut Vec<Edict>, level: &mut LevelLocals) {
    crate::g_monster::m_check_ground_raw(ent_idx as i32, edicts, level);
}

fn phys_m_check_bottom(ent_idx: usize, edicts: &mut Vec<Edict>) -> bool {
    crate::m_move::m_check_bottom_raw(ent_idx as i32, edicts)
}

fn phys_touch_triggers(ent_idx: usize, edicts: &mut Vec<Edict>, level: &mut LevelLocals) {
    use crate::g_local::GameCtx;
    let mut ctx = GameCtx {
        edicts: std::mem::take(edicts),
        level: std::mem::take(level),
        ..GameCtx::default()
    };
    crate::g_utils::g_touch_triggers(&mut ctx, ent_idx);
    *edicts = ctx.edicts;
    *level = ctx.level;
}

use crate::game_import::{
    gi_trace, gi_linkentity, gi_error, gi_positioned_sound,
    gi_sound, gi_soundindex, gi_pointcontents,
};

// Global state accessors
// These read cvar values at runtime via the myq2_common cvar system.
// Level time is read from a thread-local set by the physics entry points.

use std::cell::Cell;
thread_local! {
    static PHYS_LEVEL_TIME: Cell<f32> = const { Cell::new(0.0) };
    static PHYS_NUM_EDICTS: Cell<i32> = const { Cell::new(0) };
}

fn get_level_time() -> f32 {
    PHYS_LEVEL_TIME.with(|c| c.get())
}

fn set_level_time(t: f32) {
    PHYS_LEVEL_TIME.with(|c| c.set(t));
}

fn get_sv_gravity() -> f32 {
    // Read sv_gravity cvar; default 800
    myq2_common::cvar::cvar_variable_value("sv_gravity").max(1.0)
}

fn get_sv_maxvelocity() -> f32 {
    // Read sv_maxvelocity cvar; default 2000
    let v = myq2_common::cvar::cvar_variable_value("sv_maxvelocity");
    if v > 0.0 { v } else { 2000.0 }
}

// ============================================================
// SV_TestEntityPosition
// ============================================================

/// Returns the entity index of the first entity the given entity is stuck in,
/// or -1 if not stuck.
pub fn sv_test_entity_position(ent: &Edict) -> i32 {
    let mask = if ent.clipmask != 0 {
        ent.clipmask
    } else {
        MASK_SOLID
    };

    let trace = gi_trace(&ent.s.origin, &ent.mins, &ent.maxs, &ent.s.origin, -1, mask);

    if trace.startsolid {
        return 0; // g_edicts (world entity)
    }

    -1 // NULL
}

// ============================================================
// SV_CheckVelocity
// ============================================================

/// Bounds velocity to sv_maxvelocity.
pub fn sv_check_velocity(ent: &mut Edict) {
    let sv_maxvelocity = get_sv_maxvelocity();

    // Bound velocity
    for i in 0..3 {
        if ent.velocity[i] > sv_maxvelocity {
            ent.velocity[i] = sv_maxvelocity;
        } else if ent.velocity[i] < -sv_maxvelocity {
            ent.velocity[i] = -sv_maxvelocity;
        }
    }
}

// ============================================================
// SV_RunThink
// ============================================================

/// Runs thinking code for this frame if necessary.
/// Returns false if the entity's think function ran and removed it.
pub fn sv_run_think(ent_idx: usize, edicts: &mut Vec<Edict>, level: &mut LevelLocals) -> bool {
    let thinktime = edicts[ent_idx].nextthink;
    if thinktime <= 0.0 {
        return true;
    }
    if thinktime > level.time + 0.001 {
        return true;
    }

    edicts[ent_idx].nextthink = 0.0;
    if edicts[ent_idx].think_fn.is_none() {
        gi_error("NULL ent->think");
    }
    crate::dispatch::call_think(ent_idx, edicts, level);

    false
}

// ============================================================
// SV_Impact
// ============================================================

/// Two entities have touched, so run their touch functions.
pub fn sv_impact(e1_idx: usize, trace: &Trace, edicts: &mut Vec<Edict>, level: &mut LevelLocals) {
    let e2_idx = trace.ent_index as usize;

    if edicts[e1_idx].touch_fn.is_some() && edicts[e1_idx].solid != Solid::Not {
        crate::dispatch::call_touch(
            e1_idx,
            e2_idx,
            edicts,
            level,
            Some(&trace.plane),
            trace.surface.as_ref(),
        );
    }

    if edicts[e2_idx].touch_fn.is_some() && edicts[e2_idx].solid != Solid::Not {
        crate::dispatch::call_touch(e2_idx, e1_idx, edicts, level, None, None);
    }
}

// ============================================================
// ClipVelocity
// ============================================================

/// Slide off of the impacting object.
/// Returns the blocked flags:
/// - 1 = floor
/// - 2 = step / wall
pub fn clip_velocity(in_vel: &Vec3, normal: &Vec3, out: &mut Vec3, overbounce: f32) -> i32 {
    let mut blocked = 0;

    if normal[2] > 0.0 {
        blocked |= 1; // floor
    }
    if normal[2] == 0.0 {
        blocked |= 2; // step
    }

    let backoff = dot_product(in_vel, normal) * overbounce;

    for i in 0..3 {
        let change = normal[i] * backoff;
        out[i] = in_vel[i] - change;
        if out[i] > -STOP_EPSILON && out[i] < STOP_EPSILON {
            out[i] = 0.0;
        }
    }

    blocked
}

// ============================================================
// SV_FlyMove
// ============================================================

/// The basic solid body movement clip that slides along multiple planes.
/// Returns the clipflags if the velocity was modified (hit something solid):
/// - 1 = floor
/// - 2 = wall / step
/// - 4 = dead stop
pub fn sv_fly_move(ent_idx: usize, edicts: &mut Vec<Edict>, level: &mut LevelLocals, time: f32, mask: i32) -> i32 {
    let numbumps = 4;
    let mut blocked = 0;
    let original_velocity = vector_copy(&edicts[ent_idx].velocity);
    let primal_velocity = vector_copy(&edicts[ent_idx].velocity);
    let mut numplanes = 0;
    let mut planes: [Vec3; MAX_CLIP_PLANES] = [[0.0; 3]; MAX_CLIP_PLANES];
    let mut time_left = time;

    edicts[ent_idx].groundentity = -1;

    for _bumpcount in 0..numbumps {
        let mut end = [0.0; 3];
        for i in 0..3 {
            end[i] = edicts[ent_idx].s.origin[i] + time_left * edicts[ent_idx].velocity[i];
        }

        let trace = gi_trace(&edicts[ent_idx].s.origin, &edicts[ent_idx].mins, &edicts[ent_idx].maxs, &end, -1, mask);

        if trace.allsolid {
            edicts[ent_idx].velocity = VEC3_ORIGIN;
            return 3;
        }

        if trace.fraction > 0.0 {
            edicts[ent_idx].s.origin = vector_copy(&trace.endpos);
            edicts[ent_idx].velocity = vector_copy(&original_velocity);
            numplanes = 0;
        }

        if trace.fraction == 1.0 {
            break;
        }

        let hit = trace.ent_index;

        if trace.plane.normal[2] > 0.7 {
            blocked |= 1; // floor
            if hit >= 0 && (hit as usize) < edicts.len() && edicts[hit as usize].solid == Solid::Bsp {
                edicts[ent_idx].groundentity = hit;
                // edicts[ent_idx].groundentity_linkcount = edicts[hit as usize].linkcount;
            }
        }
        if trace.plane.normal[2] == 0.0 {
            blocked |= 2; // step
        }

        // run the impact function
        sv_impact(ent_idx, &trace, edicts, level);
        if !edicts[ent_idx].inuse {
            break;
        }

        time_left -= time_left * trace.fraction;

        if numplanes >= MAX_CLIP_PLANES {
            edicts[ent_idx].velocity = VEC3_ORIGIN;
            return 3;
        }

        planes[numplanes] = vector_copy(&trace.plane.normal);
        numplanes += 1;

        let mut new_velocity = [0.0; 3];
        let mut i = 0;
        while i < numplanes {
            clip_velocity(&original_velocity, &planes[i], &mut new_velocity, 1.0);

            let mut j = 0;
            while j < numplanes {
                if j != i && !vector_compare(&planes[i], &planes[j])
                    && dot_product(&new_velocity, &planes[j]) < 0.0 {
                        break;
                    }
                j += 1;
            }
            if j == numplanes {
                break;
            }
            i += 1;
        }

        if i != numplanes {
            edicts[ent_idx].velocity = vector_copy(&new_velocity);
        } else {
            if numplanes != 2 {
                edicts[ent_idx].velocity = VEC3_ORIGIN;
                return 7;
            }
            let dir = cross_product(&planes[0], &planes[1]);
            let d = dot_product(&dir, &edicts[ent_idx].velocity);
            edicts[ent_idx].velocity = vector_scale(&dir, d);
        }

        if dot_product(&edicts[ent_idx].velocity, &primal_velocity) <= 0.0 {
            edicts[ent_idx].velocity = VEC3_ORIGIN;
            return blocked;
        }
    }

    blocked
}


// ============================================================
// SV_AddGravity
// ============================================================

pub fn sv_add_gravity(ent: &mut Edict) {
    ent.velocity[2] -= ent.gravity * get_sv_gravity() * FRAMETIME;
}

// ============================================================
// PUSHMOVE
// ============================================================

/// Does not change the entity's velocity at all.
pub fn sv_push_entity(ent_idx: usize, push: &Vec3, edicts: &mut Vec<Edict>, level: &mut LevelLocals) -> Trace {
    let start = vector_copy(&edicts[ent_idx].s.origin);
    let end = vector_add(&start, push);

    let mask = if edicts[ent_idx].clipmask != 0 {
        edicts[ent_idx].clipmask
    } else {
        MASK_SOLID
    };

    let trace = gi_trace(&start, &edicts[ent_idx].mins, &edicts[ent_idx].maxs, &end, -1, mask);

    edicts[ent_idx].s.origin = vector_copy(&trace.endpos);
    gi_linkentity(-1);

    if trace.fraction != 1.0 {
        sv_impact(ent_idx, &trace, edicts, level);

        // if the pushed entity went away and the pusher is still there
        let trace_ent = trace.ent_index;
        if trace_ent >= 0 && (trace_ent as usize) < edicts.len()
            && !edicts[trace_ent as usize].inuse
            && edicts[ent_idx].inuse
        {
            // move the pusher back and try again
            edicts[ent_idx].s.origin = vector_copy(&start);
            gi_linkentity(ent_idx as i32);
            // Retry: re-trace
            let mask2 = if edicts[ent_idx].clipmask != 0 { edicts[ent_idx].clipmask } else { MASK_SOLID };
            let trace2 = gi_trace(&start, &edicts[ent_idx].mins, &edicts[ent_idx].maxs, &end, -1, mask2);
            edicts[ent_idx].s.origin = vector_copy(&trace2.endpos);
            gi_linkentity(ent_idx as i32);
            if trace2.fraction != 1.0 {
                sv_impact(ent_idx, &trace2, edicts, level);
            }
        }
    }

    if edicts[ent_idx].inuse {
        phys_touch_triggers(ent_idx, edicts, level);
    }

    trace
}

/// Objects need to be moved back on a failed push,
/// otherwise riders would continue to slide.
fn sv_push(pusher_idx: usize, move_vec: &Vec3, amove: &Vec3, edicts: &mut Vec<Edict>, level: &mut LevelLocals, ps: &mut PushState) -> bool {
    let mut move_clamped = [0.0; 3];

    // clamp the move to 1/8 units, so the position will
    // be accurate for client side prediction
    for i in 0..3 {
        let mut temp = move_vec[i] * 8.0;
        if temp > 0.0 {
            temp += 0.5;
        } else {
            temp -= 0.5;
        }
        move_clamped[i] = 0.125 * (temp as i32) as f32;
    }

    // find the bounding box
    let mut mins = [0.0; 3];
    let mut maxs = [0.0; 3];
    for i in 0..3 {
        mins[i] = edicts[pusher_idx].absmin[i] + move_clamped[i];
        maxs[i] = edicts[pusher_idx].absmax[i] + move_clamped[i];
    }

    // we need this for pushing things later
    let org = vector_subtract(&VEC3_ORIGIN, amove);
    let mut forward = [0.0; 3];
    let mut right = [0.0; 3];
    let mut up = [0.0; 3];
    angle_vectors(&org, Some(&mut forward), Some(&mut right), Some(&mut up));

    // save the pusher's original position
    ps.pushed[ps.pushed_p].ent = pusher_idx as i32;
    ps.pushed[ps.pushed_p].origin = vector_copy(&edicts[pusher_idx].s.origin);
    ps.pushed[ps.pushed_p].angles = vector_copy(&edicts[pusher_idx].s.angles);
    // if pusher has a client, save delta yaw
    // (client access would require the clients array which is not passed here)
    ps.pushed[ps.pushed_p].deltayaw = 0.0;
    ps.pushed_p += 1;

    // move the pusher to its final position
    edicts[pusher_idx].s.origin = vector_add(&edicts[pusher_idx].s.origin, &move_clamped);
    edicts[pusher_idx].s.angles = vector_add(&edicts[pusher_idx].s.angles, amove);
    gi_linkentity(-1);

    // see if any solid entities are inside the final position
    let num_edicts = edicts.len();

    for e in 1..num_edicts {
        if e == pusher_idx {
            continue;
        }
        let check = &edicts[e];

        if !check.inuse {
            continue;
        }
        if check.movetype == MoveType::Push
            || check.movetype == MoveType::Stop
            || check.movetype == MoveType::None
            || check.movetype == MoveType::Noclip
        {
            continue;
        }

        if check.area.prev == -1 {
            continue; // not linked in anywhere
        }

        // if the entity is standing on the pusher, it will definitely be moved
        if check.groundentity != pusher_idx as i32 {
            // see if the ent needs to be tested
            if check.absmin[0] >= maxs[0]
                || check.absmin[1] >= maxs[1]
                || check.absmin[2] >= maxs[2]
                || check.absmax[0] <= mins[0]
                || check.absmax[1] <= mins[1]
                || check.absmax[2] <= mins[2]
            {
                continue;
            }

            // see if the ent's bbox is inside the pusher's final position
            if sv_test_entity_position(check) == -1 {
                continue;
            }
        }

        if edicts[pusher_idx].movetype == MoveType::Push || edicts[e].groundentity == pusher_idx as i32 {
            // Save the entity's position before moving
            if ps.pushed_p < MAX_EDICTS {
                ps.pushed[ps.pushed_p].ent = e as i32;
                ps.pushed[ps.pushed_p].origin = vector_copy(&edicts[e].s.origin);
                ps.pushed[ps.pushed_p].angles = vector_copy(&edicts[e].s.angles);
                ps.pushed_p += 1;
            }

            // Try moving the contacted entity
            edicts[e].s.origin = vector_add(&edicts[e].s.origin, &move_clamped);

            // Figure movement due to the pusher's amove
            let org_diff = vector_subtract(&edicts[e].s.origin, &edicts[pusher_idx].s.origin);
            let org2 = [
                dot_product(&org_diff, &forward),
                -dot_product(&org_diff, &right),
                dot_product(&org_diff, &up),
            ];
            let move2 = vector_subtract(&org2, &org_diff);
            edicts[e].s.origin = vector_add(&edicts[e].s.origin, &move2);

            // May have pushed them off an edge
            if edicts[e].groundentity != pusher_idx as i32 {
                edicts[e].groundentity = -1;
            }

            let block = sv_test_entity_position(&edicts[e]);
            if block == -1 {
                // pushed ok
                gi_linkentity(e as i32);
                continue;
            }

            // if it is ok to leave in the old position, do it
            edicts[e].s.origin = vector_subtract(&edicts[e].s.origin, &move_clamped);
            edicts[e].s.origin = vector_subtract(&edicts[e].s.origin, &move2);
            let block2 = sv_test_entity_position(&edicts[e]);
            if block2 == -1 {
                ps.pushed_p = ps.pushed_p.saturating_sub(1);
                continue;
            }
        }

        // save off the obstacle so we can call the block function
        ps.obstacle = e as i32;

        // move back any entities we already moved
        // go backwards, so if the same entity was pushed twice, it goes back to the original position
        if ps.pushed_p > 0 {
            for p_idx in (0..ps.pushed_p).rev() {
                let p = ps.pushed[p_idx];
                if p.ent >= 0 && (p.ent as usize) < edicts.len() {
                    edicts[p.ent as usize].s.origin = p.origin;
                    edicts[p.ent as usize].s.angles = p.angles;
                    gi_linkentity(p.ent);
                }
            }
        }
        return false;
    }

    // see if anything we moved has touched a trigger
    if ps.pushed_p > 0 {
        for p_idx in (0..ps.pushed_p).rev() {
            let p = ps.pushed[p_idx];
            if p.ent >= 0 && (p.ent as usize) < edicts.len() {
                phys_touch_triggers(p.ent as usize, edicts, level);
            }
        }
    }

    true
}

// ============================================================
// SV_Physics_Pusher
// ============================================================

/// Bmodel objects don't interact with each other, but push all box objects.
pub fn sv_physics_pusher(ent_idx: usize, edicts: &mut Vec<Edict>, level: &mut LevelLocals) {
    // if not a team captain, so movement will be handled elsewhere
    if edicts[ent_idx].flags.intersects(FL_TEAMSLAVE) {
        return;
    }

    // make sure all team slaves can move before committing
    // any moves or calling any think functions
    // if the move is blocked, all moved objects will be backed out
    let mut ps = PushState::new();

    // Iterate through team chain
    let mut blocked_part: Option<usize> = None;
    let mut part = ent_idx as i32;
    while part >= 0 && (part as usize) < edicts.len() {
        let part_idx = part as usize;

        let has_velocity = edicts[part_idx].velocity[0] != 0.0
            || edicts[part_idx].velocity[1] != 0.0
            || edicts[part_idx].velocity[2] != 0.0
            || edicts[part_idx].avelocity[0] != 0.0
            || edicts[part_idx].avelocity[1] != 0.0
            || edicts[part_idx].avelocity[2] != 0.0;

        if has_velocity {
            let move_vec = vector_scale(&edicts[part_idx].velocity, FRAMETIME);
            let amove = vector_scale(&edicts[part_idx].avelocity, FRAMETIME);

            if !sv_push(part_idx, &move_vec, &amove, edicts, level, &mut ps) {
                blocked_part = Some(part_idx);
                break;
            }
        }

        part = edicts[part_idx].teamchain;
    }

    if ps.pushed_p > MAX_EDICTS {
        gi_error("pushed_p > &pushed[MAX_EDICTS], memory corrupted");
    }

    if let Some(blocked_idx) = blocked_part {
        // The move failed: bump all nextthink times and back out moves
        let mut mv = ent_idx as i32;
        while mv >= 0 && (mv as usize) < edicts.len() {
            let mv_idx = mv as usize;
            if edicts[mv_idx].nextthink > 0.0 {
                edicts[mv_idx].nextthink += FRAMETIME;
            }
            mv = edicts[mv_idx].teamchain;
        }

        // If the pusher has a "blocked" function, call it
        let obstacle_idx = ps.obstacle;
        if edicts[blocked_idx].blocked_fn.is_some() && obstacle_idx >= 0 {
            crate::dispatch::call_blocked(
                blocked_idx, obstacle_idx as usize, edicts, level,
            );
        }
    } else {
        // The move succeeded: call all think functions
        let mut part = ent_idx as i32;
        while part >= 0 && (part as usize) < edicts.len() {
            let part_idx = part as usize;
            sv_run_think(part_idx, edicts, level);
            part = edicts[part_idx].teamchain;
        }
    }
}

// ============================================================
// SV_Physics_None
// ============================================================

/// Non moving objects can only think.
pub fn sv_physics_none(ent_idx: usize, edicts: &mut Vec<Edict>, level: &mut LevelLocals) {
    sv_run_think(ent_idx, edicts, level);
}

// ============================================================
// SV_Physics_Noclip
// ============================================================

/// A moving object that doesn't obey physics.
pub fn sv_physics_noclip(ent_idx: usize, edicts: &mut Vec<Edict>, level: &mut LevelLocals) {
    if !sv_run_think(ent_idx, edicts, level) {
        return;
    }

    let ent = &mut edicts[ent_idx];
    let angles = ent.s.angles;
    let avelocity = ent.avelocity;
    vector_ma_to(&angles, FRAMETIME, &avelocity, &mut ent.s.angles);
    let origin = ent.s.origin;
    let velocity = ent.velocity;
    vector_ma_to(&origin, FRAMETIME, &velocity, &mut ent.s.origin);

    gi_linkentity(-1);
}

// ============================================================
// SV_Physics_Toss
// ============================================================

/// Toss, bounce, and fly movement. When onground, do nothing.
pub fn sv_physics_toss(ent_idx: usize, edicts: &mut Vec<Edict>, level: &mut LevelLocals) {
    sv_run_think(ent_idx, edicts, level);

    if edicts[ent_idx].flags.intersects(FL_TEAMSLAVE) {
        return;
    }

    if edicts[ent_idx].velocity[2] > 0.0 {
        edicts[ent_idx].groundentity = -1;
    }

    // check for the groundentity going away
    if edicts[ent_idx].groundentity != -1 {
        let ge = edicts[ent_idx].groundentity as usize;
        if ge < edicts.len() && !edicts[ge].inuse {
            edicts[ent_idx].groundentity = -1;
        }
    }

    if edicts[ent_idx].groundentity != -1 {
        return;
    }

    let old_origin = vector_copy(&edicts[ent_idx].s.origin);

    sv_check_velocity(&mut edicts[ent_idx]);

    // add gravity
    if edicts[ent_idx].movetype != MoveType::Fly && edicts[ent_idx].movetype != MoveType::FlyMissile {
        sv_add_gravity(&mut edicts[ent_idx]);
    }

    // move angles
    let angles = edicts[ent_idx].s.angles;
    let avelocity = edicts[ent_idx].avelocity;
    vector_ma_to(&angles, FRAMETIME, &avelocity, &mut edicts[ent_idx].s.angles);

    // move origin
    let move_vec = vector_scale(&edicts[ent_idx].velocity, FRAMETIME);
    let trace = sv_push_entity(ent_idx, &move_vec, edicts, level);
    if !edicts[ent_idx].inuse {
        return;
    }

    if trace.fraction < 1.0 {
        let backoff = if edicts[ent_idx].movetype == MoveType::Bounce {
            1.5
        } else {
            1.0
        };

        let mut new_velocity = [0.0; 3];
        clip_velocity(&edicts[ent_idx].velocity, &trace.plane.normal, &mut new_velocity, backoff);
        edicts[ent_idx].velocity = new_velocity;

        // stop if on ground
        if trace.plane.normal[2] > 0.7
            && (edicts[ent_idx].velocity[2] < 60.0 || edicts[ent_idx].movetype != MoveType::Bounce) {
                edicts[ent_idx].groundentity = trace.ent_index;
                edicts[ent_idx].velocity = VEC3_ORIGIN;
                edicts[ent_idx].avelocity = VEC3_ORIGIN;
            }
    }

    // check for water transition
    let wasinwater = (edicts[ent_idx].watertype & MASK_WATER) != 0;
    edicts[ent_idx].watertype = gi_pointcontents(&edicts[ent_idx].s.origin);
    let isinwater = (edicts[ent_idx].watertype & MASK_WATER) != 0;

    if isinwater {
        edicts[ent_idx].waterlevel = 1;
    } else {
        edicts[ent_idx].waterlevel = 0;
    }

    if !wasinwater && isinwater {
        gi_positioned_sound(
            &old_origin, 0, 0,
            gi_soundindex("misc/h2ohit1.wav"), 1.0, 1.0, 0.0,
        );
    } else if wasinwater && !isinwater {
        gi_positioned_sound(
            &edicts[ent_idx].s.origin, 0, 0,
            gi_soundindex("misc/h2ohit1.wav"), 1.0, 1.0, 0.0,
        );
    }

    // move teamslaves
    let mut slave = edicts[ent_idx].teamchain;
    while slave >= 0 && (slave as usize) < edicts.len() {
        let slave_idx = slave as usize;
        edicts[slave_idx].s.origin = vector_copy(&edicts[ent_idx].s.origin);
        gi_linkentity(slave as i32);
        slave = edicts[slave_idx].teamchain;
    }
}

// ============================================================
// STEPPING MOVEMENT
// ============================================================

pub fn sv_add_rotational_friction(ent: &mut Edict) {
    let angles = ent.s.angles;
    let avelocity = ent.avelocity;
    vector_ma_to(&angles, FRAMETIME, &avelocity, &mut ent.s.angles);
    let adjustment = FRAMETIME * SV_STOPSPEED * SV_FRICTION;

    for n in 0..3 {
        if ent.avelocity[n] > 0.0 {
            ent.avelocity[n] -= adjustment;
            if ent.avelocity[n] < 0.0 {
                ent.avelocity[n] = 0.0;
            }
        } else {
            ent.avelocity[n] += adjustment;
            if ent.avelocity[n] > 0.0 {
                ent.avelocity[n] = 0.0;
            }
        }
    }
}

/// Monsters freefall when they don't have a ground entity, otherwise
/// all movement is done with discrete steps.
///
/// This is also used for objects that have become still on the ground, but
/// will fall if the floor is pulled out from under them.
pub fn sv_physics_step(ent_idx: usize, edicts: &mut Vec<Edict>, level: &mut LevelLocals) {
    // airborne monsters should always check for ground
    if edicts[ent_idx].groundentity == -1 {
        phys_m_check_ground(ent_idx, edicts, level);
    }

    let groundentity = edicts[ent_idx].groundentity;

    sv_check_velocity(&mut edicts[ent_idx]);

    let wasonground = groundentity != -1;

    if edicts[ent_idx].avelocity[0] != 0.0 || edicts[ent_idx].avelocity[1] != 0.0 || edicts[ent_idx].avelocity[2] != 0.0 {
        sv_add_rotational_friction(&mut edicts[ent_idx]);
    }

    let mut hitsound = false;
    if !wasonground
        && !edicts[ent_idx].flags.intersects(FL_FLY)
            && !(edicts[ent_idx].flags.intersects(FL_SWIM) && edicts[ent_idx].waterlevel > 2) {
                if edicts[ent_idx].velocity[2] < get_sv_gravity() * -0.1 {
                    hitsound = true;
                }
                if edicts[ent_idx].waterlevel == 0 {
                    sv_add_gravity(&mut edicts[ent_idx]);
                }
            }

    if edicts[ent_idx].flags.intersects(FL_FLY) && edicts[ent_idx].velocity[2] != 0.0 {
        let speed = edicts[ent_idx].velocity[2].abs();
        let control = if speed < SV_STOPSPEED { SV_STOPSPEED } else { speed };
        let friction = SV_FRICTION / 3.0;
        let mut newspeed = speed - (FRAMETIME * control * friction);
        if newspeed < 0.0 { newspeed = 0.0; }
        newspeed /= speed;
        edicts[ent_idx].velocity[2] *= newspeed;
    }

    if edicts[ent_idx].flags.intersects(FL_SWIM) && edicts[ent_idx].velocity[2] != 0.0 {
        let speed = edicts[ent_idx].velocity[2].abs();
        let control = if speed < SV_STOPSPEED { SV_STOPSPEED } else { speed };
        let newspeed = speed - (FRAMETIME * control * SV_WATERFRICTION * edicts[ent_idx].waterlevel as f32);
        let mut newspeed = if newspeed < 0.0 { 0.0 } else { newspeed };
        newspeed /= speed;
        edicts[ent_idx].velocity[2] *= newspeed;
    }

    if edicts[ent_idx].velocity[2] != 0.0 || edicts[ent_idx].velocity[1] != 0.0 || edicts[ent_idx].velocity[0] != 0.0 {
        if (wasonground || edicts[ent_idx].flags.intersects(FL_SWIM | FL_FLY))
            && !(edicts[ent_idx].health as f32 <= 0.0 && !phys_m_check_bottom(ent_idx, edicts)) {
                let speed = (edicts[ent_idx].velocity[0] * edicts[ent_idx].velocity[0]
                    + edicts[ent_idx].velocity[1] * edicts[ent_idx].velocity[1]).sqrt();
                if speed != 0.0 {
                    let friction = SV_FRICTION;
                    let control = if speed < SV_STOPSPEED { SV_STOPSPEED } else { speed };
                    let mut newspeed = speed - FRAMETIME * control * friction;
                    if newspeed < 0.0 { newspeed = 0.0; }
                    newspeed /= speed;
                    edicts[ent_idx].velocity[0] *= newspeed;
                    edicts[ent_idx].velocity[1] *= newspeed;
                }
            }

        let mask = if (edicts[ent_idx].svflags & SVF_MONSTER) != 0 {
            MASK_MONSTERSOLID
        } else {
            MASK_SOLID
        };
        sv_fly_move(ent_idx, edicts, level, FRAMETIME, mask);

        gi_linkentity(-1);
        phys_touch_triggers(ent_idx, edicts, level);
        if !edicts[ent_idx].inuse {
            return;
        }

        if edicts[ent_idx].groundentity != -1 && !wasonground && hitsound {
            gi_sound(-1, 0, gi_soundindex("world/land.wav"), 1.0, 1.0, 0.0);
        }
    }

    // regular thinking
    sv_run_think(ent_idx, edicts, level);
}

// ============================================================
// G_RunEntity
// ============================================================

pub fn g_run_entity(ent_idx: usize, edicts: &mut Vec<Edict>, level: &mut LevelLocals) {
    set_level_time(level.time);
    if let Some(idx) = edicts[ent_idx].prethink_fn {
        crate::dispatch::dispatch_think(idx, ent_idx, edicts, level);
    }

    match edicts[ent_idx].movetype {
        MoveType::Push | MoveType::Stop => {
            sv_physics_pusher(ent_idx, edicts, level);
        }
        MoveType::None => {
            sv_physics_none(ent_idx, edicts, level);
        }
        MoveType::Noclip => {
            sv_physics_noclip(ent_idx, edicts, level);
        }
        MoveType::Step => {
            sv_physics_step(ent_idx, edicts, level);
        }
        MoveType::Toss | MoveType::Bounce | MoveType::Fly | MoveType::FlyMissile => {
            sv_physics_toss(ent_idx, edicts, level);
        }
        _ => {
            gi_error(&format!("SV_Physics: bad movetype {:?}", edicts[ent_idx].movetype));
        }
    }
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use myq2_common::q_shared::Vec3;
    use crate::g_local::{Edict, FRAMETIME};

    // ---- Helper to create a default Edict with minimal state ----
    fn make_edict() -> Edict {
        let mut e = Edict::default();
        e.inuse = true;
        e.gravity = 1.0; // default gravity multiplier (like a normal entity)
        e
    }

    // ================================================================
    // clip_velocity tests
    // ================================================================

    #[test]
    fn clip_velocity_floor_hit() {
        // Velocity going downward, hitting a flat floor (normal pointing up)
        let in_vel: Vec3 = [100.0, 0.0, -200.0];
        let normal: Vec3 = [0.0, 0.0, 1.0]; // floor
        let mut out: Vec3 = [0.0; 3];

        let blocked = clip_velocity(&in_vel, &normal, &mut out, 1.0);

        // Should flag floor (bit 0)
        assert_eq!(blocked & 1, 1, "Should be blocked by floor");
        assert_eq!(blocked & 2, 0, "Should not be blocked by wall");

        // The Z component of velocity should be removed (projected out)
        // backoff = dot(in_vel, normal) * overbounce = -200.0 * 1.0 = -200.0
        // out[2] = in_vel[2] - normal[2] * backoff = -200.0 - 1.0 * (-200.0) = 0.0
        assert!((out[0] - 100.0).abs() < 0.001);
        assert!((out[1]).abs() < 0.001);
        assert!((out[2]).abs() < 0.001, "Z velocity should be zeroed out on floor hit");
    }

    #[test]
    fn clip_velocity_wall_hit() {
        // Velocity going forward, hitting a wall (normal pointing back along X)
        let in_vel: Vec3 = [300.0, 50.0, 0.0];
        let normal: Vec3 = [-1.0, 0.0, 0.0]; // wall facing back
        let mut out: Vec3 = [0.0; 3];

        let blocked = clip_velocity(&in_vel, &normal, &mut out, 1.0);

        // normal[2] == 0.0 => blocked by step/wall (bit 1)
        assert_eq!(blocked & 2, 2, "Should be blocked by wall");
        assert_eq!(blocked & 1, 0, "Should not be blocked by floor");

        // backoff = dot([300,50,0], [-1,0,0]) * 1.0 = -300.0
        // out[0] = 300 - (-1)(-300) = 300 - 300 = 0.0
        // out[1] = 50 - 0 = 50
        // out[2] = 0 - 0 = 0
        assert!((out[0]).abs() < 0.001);
        assert!((out[1] - 50.0).abs() < 0.001);
        assert!((out[2]).abs() < 0.001);
    }

    #[test]
    fn clip_velocity_angled_surface() {
        // Hitting a 45-degree angled surface
        let normal_z = (2.0_f32).sqrt() / 2.0; // ~0.707
        let normal_x = -(2.0_f32).sqrt() / 2.0;
        let in_vel: Vec3 = [100.0, 0.0, -100.0];
        let normal: Vec3 = [normal_x, 0.0, normal_z];
        let mut out: Vec3 = [0.0; 3];

        let blocked = clip_velocity(&in_vel, &normal, &mut out, 1.0);

        // normal[2] > 0 => floor bit set
        assert_eq!(blocked & 1, 1, "Should be flagged as floor (normal z > 0)");

        // backoff = dot([100, 0, -100], [nx, 0, nz]) = 100*nx + (-100)*nz
        // nx = -0.707, nz = 0.707
        // backoff = 100*(-0.707) + (-100)*(0.707) = -70.7 - 70.7 = -141.4
        // out[0] = 100 - nx*(-141.4) = 100 - (-0.707)(-141.4) = 100 - 100 = 0
        // out[2] = -100 - nz*(-141.4) = -100 + 100 = 0
        assert!(out[0].abs() < 0.2, "X should be near 0: got {}", out[0]);
        assert!(out[2].abs() < 0.2, "Z should be near 0: got {}", out[2]);
    }

    #[test]
    fn clip_velocity_overbounce() {
        // With overbounce > 1.0 (bounce factor), velocity should reverse partially
        let in_vel: Vec3 = [0.0, 0.0, -100.0];
        let normal: Vec3 = [0.0, 0.0, 1.0]; // floor
        let mut out: Vec3 = [0.0; 3];

        let blocked = clip_velocity(&in_vel, &normal, &mut out, 1.5);

        // backoff = dot([0,0,-100], [0,0,1]) * 1.5 = -100 * 1.5 = -150
        // out[2] = -100 - 1.0 * (-150) = -100 + 150 = 50
        assert_eq!(blocked & 1, 1);
        assert!((out[2] - 50.0).abs() < 0.001, "Bounce should reflect: got {}", out[2]);
    }

    #[test]
    fn clip_velocity_stop_epsilon_clamp() {
        // Very small velocity components should be clamped to zero
        let in_vel: Vec3 = [0.05, -0.05, 0.0];
        let normal: Vec3 = [0.0, 0.0, 1.0]; // floor
        let mut out: Vec3 = [0.0; 3];

        clip_velocity(&in_vel, &normal, &mut out, 1.0);

        // STOP_EPSILON = 0.1, so values in (-0.1, 0.1) are clamped to 0
        assert_eq!(out[0], 0.0, "Should be clamped to 0");
        assert_eq!(out[1], 0.0, "Should be clamped to 0");
    }

    #[test]
    fn clip_velocity_no_hit() {
        // Velocity parallel to the surface should pass through unchanged
        let in_vel: Vec3 = [0.0, 100.0, 0.0];
        let normal: Vec3 = [1.0, 0.0, 0.0]; // wall facing along +X
        let mut out: Vec3 = [0.0; 3];

        let blocked = clip_velocity(&in_vel, &normal, &mut out, 1.0);

        // dot([0,100,0], [1,0,0]) = 0 => backoff = 0 => out = in_vel
        assert_eq!(blocked & 2, 2, "Normal z==0 always sets wall bit");
        assert!((out[0]).abs() < 0.001);
        assert!((out[1] - 100.0).abs() < 0.001);
        assert!((out[2]).abs() < 0.001);
    }

    #[test]
    fn clip_velocity_zero_velocity() {
        let in_vel: Vec3 = [0.0, 0.0, 0.0];
        let normal: Vec3 = [0.0, 0.0, 1.0];
        let mut out: Vec3 = [0.0; 3];

        let blocked = clip_velocity(&in_vel, &normal, &mut out, 1.0);

        assert_eq!(blocked & 1, 1);
        assert_eq!(out, [0.0, 0.0, 0.0]);
    }

    // ================================================================
    // sv_add_rotational_friction tests
    // ================================================================

    #[test]
    fn add_rotational_friction_positive_avelocity() {
        let mut ent = make_edict();
        ent.s.angles = [0.0, 0.0, 0.0];
        ent.avelocity = [100.0, 200.0, 300.0];

        sv_add_rotational_friction(&mut ent);

        // adjustment = FRAMETIME * SV_STOPSPEED * SV_FRICTION = 0.1 * 100.0 * 6.0 = 60.0
        // Each positive avelocity should be reduced by 60.0
        assert!((ent.avelocity[0] - 40.0).abs() < 0.001);
        assert!((ent.avelocity[1] - 140.0).abs() < 0.001);
        assert!((ent.avelocity[2] - 240.0).abs() < 0.001);

        // angles should be updated: angles += FRAMETIME * avelocity (using original avelocity)
        // angles[0] = 0 + 0.1 * 100 = 10
        // angles[1] = 0 + 0.1 * 200 = 20
        // angles[2] = 0 + 0.1 * 300 = 30
        assert!((ent.s.angles[0] - 10.0).abs() < 0.001);
        assert!((ent.s.angles[1] - 20.0).abs() < 0.001);
        assert!((ent.s.angles[2] - 30.0).abs() < 0.001);
    }

    #[test]
    fn add_rotational_friction_negative_avelocity() {
        let mut ent = make_edict();
        ent.s.angles = [0.0, 0.0, 0.0];
        ent.avelocity = [-100.0, -200.0, -300.0];

        sv_add_rotational_friction(&mut ent);

        // adjustment = 60.0
        // For negative values, avelocity += adjustment, then clamp at 0
        // -100 + 60 = -40
        // -200 + 60 = -140
        // -300 + 60 = -240
        assert!((ent.avelocity[0] - (-40.0)).abs() < 0.001);
        assert!((ent.avelocity[1] - (-140.0)).abs() < 0.001);
        assert!((ent.avelocity[2] - (-240.0)).abs() < 0.001);
    }

    #[test]
    fn add_rotational_friction_clamps_to_zero() {
        let mut ent = make_edict();
        ent.s.angles = [0.0, 0.0, 0.0];
        // Small values that should clamp to zero after friction
        // adjustment = 60.0, so anything with |avelocity| < 60 should clamp to 0
        ent.avelocity = [30.0, -30.0, 0.0];

        sv_add_rotational_friction(&mut ent);

        // 30 - 60 = -30, but since it crossed zero, clamped to 0
        assert_eq!(ent.avelocity[0], 0.0);
        // -30 + 60 = 30, but since it crossed zero, clamped to 0
        assert_eq!(ent.avelocity[1], 0.0);
        // Already 0, should stay 0
        assert_eq!(ent.avelocity[2], 0.0);
    }

    #[test]
    fn add_rotational_friction_zero_avelocity() {
        let mut ent = make_edict();
        ent.s.angles = [10.0, 20.0, 30.0];
        ent.avelocity = [0.0, 0.0, 0.0];

        let original_angles = ent.s.angles;
        sv_add_rotational_friction(&mut ent);

        // With zero avelocity, angles should not change (FRAMETIME * 0 = 0)
        // and avelocity stays 0
        assert_eq!(ent.s.angles, original_angles);
        assert_eq!(ent.avelocity, [0.0, 0.0, 0.0]);
    }

    // ================================================================
    // PushState tests
    // ================================================================

    #[test]
    fn push_state_new_initializes_correctly() {
        let ps = PushState::new();
        assert_eq!(ps.pushed_p, 0);
        assert_eq!(ps.obstacle, -1);
        // All entities should be initialized to -1
        assert_eq!(ps.pushed[0].ent, -1);
        assert_eq!(ps.pushed[MAX_EDICTS - 1].ent, -1);
    }

    // ================================================================
    // Physics constants tests
    // ================================================================

    #[test]
    fn physics_constants_match_quake2() {
        // Verify the physics constants match the original Quake 2 values
        assert_eq!(STOP_EPSILON, 0.1);
        assert_eq!(SV_STOPSPEED, 100.0);
        assert_eq!(SV_FRICTION, 6.0);
        assert_eq!(SV_WATERFRICTION, 1.0);
        assert_eq!(FRAMETIME, 0.1);
    }

    // ================================================================
    // Pushed struct tests
    // ================================================================

    #[test]
    fn pushed_default_values() {
        let p = Pushed::default();
        assert_eq!(p.ent, 0); // Default for i32
        assert_eq!(p.origin, [0.0; 3]);
        assert_eq!(p.angles, [0.0; 3]);
        assert_eq!(p.deltayaw, 0.0);
    }

    // ================================================================
    // clip_velocity edge cases
    // ================================================================

    #[test]
    fn clip_velocity_diagonal_wall() {
        // 45-degree wall (normal in XY plane)
        let n = (2.0_f32).sqrt() / 2.0;
        let in_vel: Vec3 = [200.0, 0.0, 0.0];
        let normal: Vec3 = [-n, n, 0.0]; // 45-deg wall
        let mut out: Vec3 = [0.0; 3];

        let blocked = clip_velocity(&in_vel, &normal, &mut out, 1.0);

        // normal[2] == 0 => wall bit
        assert_eq!(blocked & 2, 2);

        // backoff = dot([200,0,0], [-n,n,0]) = -200n = -200*0.707 = -141.4
        // out[0] = 200 - (-n)(-141.4) = 200 - 100 = 100
        // out[1] = 0 - n*(-141.4) = 0 + 100 = 100
        assert!((out[0] - 100.0).abs() < 1.0, "X: {}", out[0]);
        assert!((out[1] - 100.0).abs() < 1.0, "Y: {}", out[1]);
    }

    #[test]
    fn clip_velocity_ceiling() {
        // Hitting a ceiling (normal pointing down)
        let in_vel: Vec3 = [50.0, 0.0, 300.0];
        let normal: Vec3 = [0.0, 0.0, -1.0]; // ceiling
        let mut out: Vec3 = [0.0; 3];

        let blocked = clip_velocity(&in_vel, &normal, &mut out, 1.0);

        // normal[2] < 0, not > 0 => no floor bit
        // normal[2] != 0 => no step/wall bit
        assert_eq!(blocked, 0, "Ceiling should not set floor or wall bits");

        // backoff = dot([50,0,300], [0,0,-1]) = -300
        // out[2] = 300 - (-1)(-300) = 300 - 300 = 0
        assert!((out[0] - 50.0).abs() < 0.001);
        assert!((out[2]).abs() < 0.001);
    }

    #[test]
    fn clip_velocity_large_overbounce() {
        // With a large bounce factor
        let in_vel: Vec3 = [0.0, 0.0, -500.0];
        let normal: Vec3 = [0.0, 0.0, 1.0];
        let mut out: Vec3 = [0.0; 3];

        clip_velocity(&in_vel, &normal, &mut out, 2.0);

        // backoff = -500 * 2.0 = -1000
        // out[2] = -500 - 1.0*(-1000) = -500 + 1000 = 500
        assert!((out[2] - 500.0).abs() < 0.001, "Should fully reverse: got {}", out[2]);
    }

    // ================================================================
    // Integration-style tests for rotational friction over multiple frames
    // ================================================================

    #[test]
    fn rotational_friction_converges_to_zero() {
        let mut ent = make_edict();
        ent.avelocity = [500.0, -500.0, 1000.0];
        ent.s.angles = [0.0, 0.0, 0.0];

        // Run friction for many frames; avelocity should converge to zero
        for _ in 0..1000 {
            sv_add_rotational_friction(&mut ent);
        }

        assert_eq!(ent.avelocity[0], 0.0, "Should converge to 0");
        assert_eq!(ent.avelocity[1], 0.0, "Should converge to 0");
        assert_eq!(ent.avelocity[2], 0.0, "Should converge to 0");
    }

    // ================================================================
    // clip_velocity blocked flags comprehensive test
    // ================================================================

    #[test]
    fn clip_velocity_blocked_flags_comprehensive() {
        let vel: Vec3 = [100.0, 100.0, -100.0];
        let mut out: Vec3 = [0.0; 3];

        // Floor normal (z > 0): should set bit 0
        let blocked = clip_velocity(&vel, &[0.0, 0.0, 1.0], &mut out, 1.0);
        assert_eq!(blocked & 1, 1, "Floor should set bit 0");
        assert_eq!(blocked & 2, 0, "Floor should not set bit 1");

        // Wall normal (z == 0): should set bit 1
        let blocked = clip_velocity(&vel, &[1.0, 0.0, 0.0], &mut out, 1.0);
        assert_eq!(blocked & 1, 0, "Wall should not set bit 0");
        assert_eq!(blocked & 2, 2, "Wall should set bit 1");

        // Slope (z > 0 but not 0): should set bit 0 only
        let blocked = clip_velocity(&vel, &[0.0, 0.0, 0.5], &mut out, 1.0);
        assert_eq!(blocked & 1, 1, "Slope with z>0 should set floor bit");
        assert_eq!(blocked & 2, 0, "Slope with z>0 should not set wall bit");

        // Ceiling (z < 0): should set neither
        let blocked = clip_velocity(&vel, &[0.0, 0.0, -1.0], &mut out, 1.0);
        assert_eq!(blocked & 1, 0);
        assert_eq!(blocked & 2, 0);
    }

    // ================================================================
    // sv_add_gravity tests (depends on cvar system, but we can test the math)
    // ================================================================

    #[test]
    fn add_gravity_formula() {
        // sv_add_gravity uses: ent.velocity[2] -= ent.gravity * get_sv_gravity() * FRAMETIME
        // With default cvar state, get_sv_gravity() returns 1.0 (minimum clamped).
        // The exact value depends on the cvar system state, but we can verify
        // the direction of gravity application.
        let mut ent = make_edict();
        ent.velocity = [100.0, 200.0, 300.0];
        ent.gravity = 1.0;

        let orig_z = ent.velocity[2];
        sv_add_gravity(&mut ent);

        // Gravity should reduce velocity[2]
        assert!(ent.velocity[2] < orig_z, "Gravity should decrease Z velocity");
        // X and Y should be unaffected
        assert_eq!(ent.velocity[0], 100.0);
        assert_eq!(ent.velocity[1], 200.0);
    }

    #[test]
    fn add_gravity_zero_gravity_entity() {
        // An entity with gravity = 0 should not be affected
        let mut ent = make_edict();
        ent.velocity = [0.0, 0.0, 100.0];
        ent.gravity = 0.0;

        sv_add_gravity(&mut ent);

        assert_eq!(ent.velocity[2], 100.0, "Zero gravity multiplier should not change velocity");
    }

    #[test]
    fn add_gravity_high_gravity_entity() {
        // An entity with higher-than-normal gravity (e.g., 2.0)
        let mut ent_normal = make_edict();
        ent_normal.velocity = [0.0, 0.0, 0.0];
        ent_normal.gravity = 1.0;

        let mut ent_heavy = make_edict();
        ent_heavy.velocity = [0.0, 0.0, 0.0];
        ent_heavy.gravity = 2.0;

        sv_add_gravity(&mut ent_normal);
        sv_add_gravity(&mut ent_heavy);

        // Heavy entity should fall faster (velocity[2] more negative)
        assert!(ent_heavy.velocity[2] < ent_normal.velocity[2],
            "Heavy entity should have more negative Z velocity");
        // The ratio should be exactly 2:1
        assert!((ent_heavy.velocity[2] / ent_normal.velocity[2] - 2.0).abs() < 0.001,
            "Gravity ratio should be 2:1");
    }

    // ================================================================
    // sv_check_velocity tests
    // ================================================================

    #[test]
    fn check_velocity_clamps_large_values() {
        // sv_check_velocity reads sv_maxvelocity cvar.
        // In test context, the cvar defaults to a fallback value.
        // We can test that the function correctly clamps extreme values.
        let mut ent = make_edict();
        ent.velocity = [999999.0, -999999.0, 0.0];

        sv_check_velocity(&mut ent);

        // After clamping, values should be within [-maxvelocity, maxvelocity]
        let max = get_sv_maxvelocity();
        assert!(ent.velocity[0] <= max, "X should be clamped to max");
        assert!(ent.velocity[1] >= -max, "Y should be clamped to -max");
        assert_eq!(ent.velocity[2], 0.0, "Z should be unchanged (was within bounds)");
    }

    #[test]
    fn check_velocity_does_not_clamp_small_values() {
        let mut ent = make_edict();
        ent.velocity = [10.0, -20.0, 30.0];

        sv_check_velocity(&mut ent);

        // Small values should pass through untouched
        assert_eq!(ent.velocity[0], 10.0);
        assert_eq!(ent.velocity[1], -20.0);
        assert_eq!(ent.velocity[2], 30.0);
    }

    #[test]
    fn check_velocity_symmetric_clamping() {
        let mut ent = make_edict();
        let max = get_sv_maxvelocity();
        ent.velocity = [max + 100.0, -(max + 100.0), max - 1.0];

        sv_check_velocity(&mut ent);

        assert_eq!(ent.velocity[0], max);
        assert_eq!(ent.velocity[1], -max);
        assert_eq!(ent.velocity[2], max - 1.0);
    }

    // ================================================================
    // Thread-local level time tests
    // ================================================================

    #[test]
    fn level_time_set_and_get() {
        set_level_time(42.5);
        assert_eq!(get_level_time(), 42.5);

        set_level_time(0.0);
        assert_eq!(get_level_time(), 0.0);

        set_level_time(-1.0);
        assert_eq!(get_level_time(), -1.0);
    }
}
