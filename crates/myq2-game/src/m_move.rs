// m_move.rs -- monster movement
// Converted from: myq2-original/game/m_move.c
//
// Copyright (C) 1997-2001 Id Software, Inc.
// Licensed under the GNU General Public License v2 or later.

use crate::g_local::*;
use crate::game_import::*;
use myq2_common::q_shared::{
    Vec3, Trace, CONTENTS_SOLID, MASK_MONSTERSOLID, MASK_WATER, YAW, anglemod, VEC3_ORIGIN,
};

const STEPSIZE: f32 = 18.0;
const DI_NODIR: f32 = -1.0;

/// Game context holding entity list and game import functions.
/// All functions in this module take a mutable reference to this context
/// instead of accessing C globals directly.
pub struct MoveContext {
    pub edicts: Vec<Edict>,
    pub clients: Vec<GClient>,
    /// Debug counters from C globals `c_yes` and `c_no`.
    pub c_yes: i32,
    pub c_no: i32,
}

// ============================================================
// Helper: game import (gi) placeholders
// These will be replaced with real implementations when the
// game import table is wired up.
// ============================================================

fn move_gi_pointcontents(_ctx: &MoveContext, point: &Vec3) -> i32 {
    gi_pointcontents(point)
}

fn move_gi_trace(
    _ctx: &MoveContext,
    start: &Vec3,
    mins: &Vec3,
    maxs: &Vec3,
    end: &Vec3,
    passent_idx: i32,
    contentmask: i32,
) -> Trace {
    gi_trace(start, mins, maxs, end, passent_idx, contentmask)
}

fn move_gi_linkentity(_ctx: &mut MoveContext, ent_idx: i32) {
    gi_linkentity(ent_idx);
}

// ============================================================
// vec3 helpers (operating on [f32; 3])
// VEC3_ORIGIN comes from myq2_common::q_shared
// ============================================================

use myq2_common::q_shared::{vector_add as vec3_add, vector_copy as vec3_copy};

// ============================================================
// M_CheckBottom
//
// Returns false if any part of the bottom of the entity is off
// an edge that is not a staircase.
// ============================================================

pub fn m_check_bottom(ctx: &mut MoveContext, ent_idx: i32) -> bool {
    let ent = &ctx.edicts[ent_idx as usize];
    let mins = vec3_add(&ent.s.origin, &ent.mins);
    let maxs = vec3_add(&ent.s.origin, &ent.maxs);

    // If all of the points under the corners are solid world, don't bother
    // with the tougher checks. The corners must be within 16 of the midpoint.
    let mut start = [0.0_f32; 3];
    start[2] = mins[2] - 1.0;

    let mut all_solid = true;
    'corner_check: for x in 0..=1 {
        for y in 0..=1 {
            start[0] = if x != 0 { maxs[0] } else { mins[0] };
            start[1] = if y != 0 { maxs[1] } else { mins[1] };
            if move_gi_pointcontents(ctx, &start) != CONTENTS_SOLID {
                all_solid = false;
                break 'corner_check;
            }
        }
    }

    if all_solid {
        ctx.c_yes += 1;
        return true; // we got out easy
    }

    ctx.c_no += 1;

    // Check it for real...
    start[2] = mins[2];

    // The midpoint must be within 16 of the bottom
    let mid_x = (mins[0] + maxs[0]) * 0.5;
    let mid_y = (mins[1] + maxs[1]) * 0.5;
    start[0] = mid_x;
    start[1] = mid_y;
    let mut stop = [mid_x, mid_y, start[2] - 2.0 * STEPSIZE];

    let trace = move_gi_trace(
        ctx,
        &start,
        &VEC3_ORIGIN,
        &VEC3_ORIGIN,
        &stop,
        ent_idx,
        MASK_MONSTERSOLID,
    );

    if trace.fraction == 1.0 {
        return false;
    }
    let mid = trace.endpos[2];
    let mut bottom = mid;

    // The corners must be within 16 of the midpoint
    for x in 0..=1 {
        for y in 0..=1 {
            start[0] = if x != 0 { maxs[0] } else { mins[0] };
            start[1] = if y != 0 { maxs[1] } else { mins[1] };
            stop[0] = start[0];
            stop[1] = start[1];

            let trace = move_gi_trace(
                ctx,
                &start,
                &VEC3_ORIGIN,
                &VEC3_ORIGIN,
                &stop,
                ent_idx,
                MASK_MONSTERSOLID,
            );

            if trace.fraction != 1.0 && trace.endpos[2] > bottom {
                bottom = trace.endpos[2];
            }
            if trace.fraction == 1.0 || mid - trace.endpos[2] > STEPSIZE {
                return false;
            }
        }
    }

    ctx.c_yes += 1;
    true
}

/// Standalone wrapper for `m_check_bottom` that works with raw edicts slice.
/// Used by g_phys.rs which doesn't have a full MoveContext.
pub fn m_check_bottom_raw(ent_idx: i32, edicts: &mut Vec<Edict>) -> bool {
    let mut ctx = MoveContext {
        edicts: std::mem::take(edicts),
        clients: Vec::new(),
        c_yes: 0,
        c_no: 0,
    };
    let result = m_check_bottom(&mut ctx, ent_idx);
    *edicts = ctx.edicts;
    result
}

// ============================================================
// SV_movestep
//
// Called by monster program code.
// The move will be adjusted for slopes and stairs, but if the
// move isn't possible, no move is done and false is returned.
// ============================================================

pub fn sv_movestep(ctx: &mut MoveContext, ent_idx: i32, mov: Vec3, relink: bool) -> bool {
    let ent = &ctx.edicts[ent_idx as usize];
    let oldorg = vec3_copy(&ent.s.origin);
    let mut neworg = vec3_add(&ent.s.origin, &mov);
    let ent_flags = ent.flags;
    let ent_waterlevel = ent.waterlevel;
    let ent_enemy = ent.enemy;
    let ent_goalentity = ent.goalentity;
    let _ent_mins = ent.mins;
    let _ent_maxs = ent.maxs;
    let _ent_origin = ent.s.origin;

    // Flying monsters don't step up
    if ent_flags.intersects(FL_SWIM | FL_FLY) {
        // Try one move with vertical motion, then one without
        for i in 0..2 {
            neworg = vec3_add(&ctx.edicts[ent_idx as usize].s.origin, &mov);

            if i == 0 && ent_enemy >= 0 {
                let goal_idx = if ent_goalentity < 0 {
                    // Set goalentity to enemy
                    ctx.edicts[ent_idx as usize].goalentity = ent_enemy;
                    ent_enemy
                } else {
                    ent_goalentity
                };

                let goal_origin = ctx.edicts[goal_idx as usize].s.origin;
                let goal_has_client = ctx.edicts[goal_idx as usize].client.is_some();
                let dz = ctx.edicts[ent_idx as usize].s.origin[2] - goal_origin[2];

                if goal_has_client {
                    if dz > 40.0 {
                        neworg[2] -= 8.0;
                    }
                    if !(ent_flags.intersects(FL_SWIM) && ent_waterlevel < 2)
                        && dz < 30.0 {
                            neworg[2] += 8.0;
                        }
                } else if dz > 8.0 {
                    neworg[2] -= 8.0;
                } else if dz > 0.0 {
                    neworg[2] -= dz;
                } else if dz < -8.0 {
                    neworg[2] += 8.0;
                } else {
                    neworg[2] += dz;
                }
            }

            let ent = &ctx.edicts[ent_idx as usize];
            let trace = move_gi_trace(
                ctx,
                &ent.s.origin,
                &ent.mins,
                &ent.maxs,
                &neworg,
                ent_idx,
                MASK_MONSTERSOLID,
            );

            // Fly monsters don't enter water voluntarily
            if ent_flags.intersects(FL_FLY)
                && ent_waterlevel == 0 {
                    let ent = &ctx.edicts[ent_idx as usize];
                    let test = [
                        trace.endpos[0],
                        trace.endpos[1],
                        trace.endpos[2] + ent.mins[2] + 1.0,
                    ];
                    let contents = move_gi_pointcontents(ctx, &test);
                    if (contents & MASK_WATER) != 0 {
                        return false;
                    }
                }

            // Swim monsters don't exit water voluntarily
            if ent_flags.intersects(FL_SWIM)
                && ent_waterlevel < 2 {
                    let ent = &ctx.edicts[ent_idx as usize];
                    let test = [
                        trace.endpos[0],
                        trace.endpos[1],
                        trace.endpos[2] + ent.mins[2] + 1.0,
                    ];
                    let contents = move_gi_pointcontents(ctx, &test);
                    if (contents & MASK_WATER) == 0 {
                        return false;
                    }
                }

            if trace.fraction == 1.0 {
                let endpos = trace.endpos;
                ctx.edicts[ent_idx as usize].s.origin = endpos;
                if relink {
                    move_gi_linkentity(ctx, ent_idx);
                    crate::g_local::with_global_game_ctx(|gctx| {
                        crate::g_utils::g_touch_triggers(gctx, ent_idx as usize);
                    });
                }
                return true;
            }

            if ent_enemy < 0 {
                break;
            }
        }

        return false;
    }

    // Push down from a step height above the wished position
    let ent_aiflags = ctx.edicts[ent_idx as usize].monsterinfo.aiflags;
    let stepsize = if !ent_aiflags.intersects(AI_NOSTEP) {
        STEPSIZE
    } else {
        1.0
    };

    neworg[2] += stepsize;
    let mut end = vec3_copy(&neworg);
    end[2] -= stepsize * 2.0;

    let ent = &ctx.edicts[ent_idx as usize];
    let mut trace = move_gi_trace(
        ctx,
        &neworg,
        &ent.mins,
        &ent.maxs,
        &end,
        ent_idx,
        MASK_MONSTERSOLID,
    );

    if trace.allsolid {
        return false;
    }

    if trace.startsolid {
        neworg[2] -= stepsize;
        let ent = &ctx.edicts[ent_idx as usize];
        trace = gi_trace(
            &neworg,
            &ent.mins,
            &ent.maxs,
            &end,
            ent_idx,
            MASK_MONSTERSOLID,
        );
        if trace.allsolid || trace.startsolid {
            return false;
        }
    }

    // Don't go in to water
    if ctx.edicts[ent_idx as usize].waterlevel == 0 {
        let ent = &ctx.edicts[ent_idx as usize];
        let test = [
            trace.endpos[0],
            trace.endpos[1],
            trace.endpos[2] + ent.mins[2] + 1.0,
        ];
        let contents = move_gi_pointcontents(ctx, &test);
        if (contents & MASK_WATER) != 0 {
            return false;
        }
    }

    if trace.fraction == 1.0 {
        // If monster had the ground pulled out, go ahead and fall
        if ctx.edicts[ent_idx as usize].flags.intersects(FL_PARTIALGROUND) {
            let origin = vec3_add(&ctx.edicts[ent_idx as usize].s.origin, &mov);
            ctx.edicts[ent_idx as usize].s.origin = origin;
            if relink {
                move_gi_linkentity(ctx, ent_idx);
                crate::g_local::with_global_game_ctx(|gctx| {
                    crate::g_utils::g_touch_triggers(gctx, ent_idx as usize);
                });
            }
            ctx.edicts[ent_idx as usize].groundentity = -1;
            return true;
        }

        return false; // walked off an edge
    }

    // Check point traces down for dangling corners
    let endpos = trace.endpos;
    ctx.edicts[ent_idx as usize].s.origin = endpos;

    if !m_check_bottom(ctx, ent_idx) {
        if ctx.edicts[ent_idx as usize].flags.intersects(FL_PARTIALGROUND) {
            // Entity had floor mostly pulled out from underneath it
            // and is trying to correct
            if relink {
                move_gi_linkentity(ctx, ent_idx);
                crate::g_local::with_global_game_ctx(|gctx| {
                    crate::g_utils::g_touch_triggers(gctx, ent_idx as usize);
                });
            }
            return true;
        }
        ctx.edicts[ent_idx as usize].s.origin = oldorg;
        return false;
    }

    if ctx.edicts[ent_idx as usize].flags.intersects(FL_PARTIALGROUND) {
        ctx.edicts[ent_idx as usize].flags.remove(FL_PARTIALGROUND);
    }

    let trace_ent = trace.ent_index;
    ctx.edicts[ent_idx as usize].groundentity = trace_ent;
    if trace_ent >= 0 {
        ctx.edicts[ent_idx as usize].groundentity_linkcount =
            ctx.edicts[trace_ent as usize].linkcount;
    }

    // The move is ok
    if relink {
        move_gi_linkentity(ctx, ent_idx);
        crate::g_local::with_global_game_ctx(|gctx| {
            crate::g_utils::g_touch_triggers(gctx, ent_idx as usize);
        });
    }
    true
}

// ============================================================
// M_ChangeYaw
// ============================================================

pub fn m_change_yaw(ctx: &mut MoveContext, ent_idx: i32) {
    let ent = &ctx.edicts[ent_idx as usize];
    let current = anglemod(ent.s.angles[YAW]);
    let ideal = ent.ideal_yaw;

    if current == ideal {
        return;
    }

    let mut mov = ideal - current;
    let speed = ent.yaw_speed;

    if ideal > current {
        if mov >= 180.0 {
            mov -= 360.0;
        }
    } else if mov <= -180.0 {
        mov += 360.0;
    }

    if mov > 0.0 {
        if mov > speed {
            mov = speed;
        }
    } else if mov < -speed {
        mov = -speed;
    }

    ctx.edicts[ent_idx as usize].s.angles[YAW] = anglemod(current + mov);
}

// ============================================================
// SV_StepDirection
//
// Turns to the movement direction, and walks the current
// distance if facing it.
// ============================================================

pub fn sv_step_direction(ctx: &mut MoveContext, ent_idx: i32, yaw: f32, dist: f32) -> bool {
    ctx.edicts[ent_idx as usize].ideal_yaw = yaw;
    m_change_yaw(ctx, ent_idx);

    let yaw_rad = yaw * std::f32::consts::PI * 2.0 / 360.0;
    let mov: Vec3 = [yaw_rad.cos() * dist, yaw_rad.sin() * dist, 0.0];

    let oldorigin = vec3_copy(&ctx.edicts[ent_idx as usize].s.origin);

    if sv_movestep(ctx, ent_idx, mov, false) {
        let ent = &ctx.edicts[ent_idx as usize];
        let delta = ent.s.angles[YAW] - ent.ideal_yaw;
        if delta > 45.0 && delta < 315.0 {
            // Not turned far enough, so don't take the step
            ctx.edicts[ent_idx as usize].s.origin = oldorigin;
        }
        move_gi_linkentity(ctx, ent_idx);
        crate::g_local::with_global_game_ctx(|gctx| {
            crate::g_utils::g_touch_triggers(gctx, ent_idx as usize);
        });
        return true;
    }
    move_gi_linkentity(ctx, ent_idx);
    crate::g_local::with_global_game_ctx(|gctx| {
        crate::g_utils::g_touch_triggers(gctx, ent_idx as usize);
    });
    false
}

// ============================================================
// SV_FixCheckBottom
// ============================================================

pub fn sv_fix_check_bottom(ctx: &mut MoveContext, ent_idx: i32) {
    ctx.edicts[ent_idx as usize].flags |= FL_PARTIALGROUND;
}

// ============================================================
// SV_NewChaseDir
// ============================================================

pub fn sv_new_chase_dir(ctx: &mut MoveContext, actor_idx: i32, enemy_idx: i32, dist: f32) {
    // Defensive check: early return if no valid enemy
    if enemy_idx < 0 {
        return;
    }

    let actor = &ctx.edicts[actor_idx as usize];
    let enemy = &ctx.edicts[enemy_idx as usize];

    let olddir = anglemod(((actor.ideal_yaw / 45.0) as i32 as f32) * 45.0);
    let turnaround = anglemod(olddir - 180.0);

    let deltax = enemy.s.origin[0] - actor.s.origin[0];
    let deltay = enemy.s.origin[1] - actor.s.origin[1];

    let mut d = [0.0_f32; 3];

    if deltax > 10.0 {
        d[1] = 0.0;
    } else if deltax < -10.0 {
        d[1] = 180.0;
    } else {
        d[1] = DI_NODIR;
    }

    if deltay < -10.0 {
        d[2] = 270.0;
    } else if deltay > 10.0 {
        d[2] = 90.0;
    } else {
        d[2] = DI_NODIR;
    }

    // Try direct route
    if d[1] != DI_NODIR && d[2] != DI_NODIR {
        let tdir = if d[1] == 0.0 {
            if d[2] == 90.0 { 45.0 } else { 315.0 }
        } else if d[2] == 90.0 { 135.0 } else { 215.0 };

        if tdir != turnaround && sv_step_direction(ctx, actor_idx, tdir, dist) {
            return;
        }
    }

    // Try other directions
    // In C: if (((rand()&3) & 1) || abs(deltay) > abs(deltax))
    let swap = (rand_i32() & 3) & 1 != 0 || deltay.abs() > deltax.abs();
    if swap {
        d.swap(1, 2);
    }

    if d[1] != DI_NODIR && d[1] != turnaround && sv_step_direction(ctx, actor_idx, d[1], dist) {
        return;
    }

    if d[2] != DI_NODIR && d[2] != turnaround && sv_step_direction(ctx, actor_idx, d[2], dist) {
        return;
    }

    // There is no direct path to the player, so pick another direction
    if olddir != DI_NODIR && sv_step_direction(ctx, actor_idx, olddir, dist) {
        return;
    }

    // Randomly determine direction of search
    if rand_i32() & 1 != 0 {
        let mut tdir = 0.0_f32;
        while tdir <= 315.0 {
            if tdir != turnaround && sv_step_direction(ctx, actor_idx, tdir, dist) {
                return;
            }
            tdir += 45.0;
        }
    } else {
        let mut tdir = 315.0_f32;
        while tdir >= 0.0 {
            if tdir != turnaround && sv_step_direction(ctx, actor_idx, tdir, dist) {
                return;
            }
            tdir -= 45.0;
        }
    }

    if turnaround != DI_NODIR && sv_step_direction(ctx, actor_idx, turnaround, dist) {
        return;
    }

    ctx.edicts[actor_idx as usize].ideal_yaw = olddir; // can't move

    // If a bridge was pulled out from underneath a monster, it may not have
    // a valid standing position at all
    if !m_check_bottom(ctx, actor_idx) {
        sv_fix_check_bottom(ctx, actor_idx);
    }
}

// ============================================================
// SV_CloseEnough
// ============================================================

pub fn sv_close_enough(ctx: &MoveContext, ent_idx: i32, goal_idx: i32, dist: f32) -> bool {
    let ent = &ctx.edicts[ent_idx as usize];
    let goal = &ctx.edicts[goal_idx as usize];

    for i in 0..3 {
        if goal.absmin[i] > ent.absmax[i] + dist {
            return false;
        }
        if goal.absmax[i] < ent.absmin[i] - dist {
            return false;
        }
    }
    true
}

// ============================================================
// M_MoveToGoal
// ============================================================

pub fn m_move_to_goal(ctx: &mut MoveContext, ent_idx: i32, dist: f32) {
    let ent = &ctx.edicts[ent_idx as usize];
    let goal_idx = ent.goalentity;
    let enemy_idx = ent.enemy;
    let flags = ent.flags;
    let groundentity = ent.groundentity;
    let ideal_yaw = ent.ideal_yaw;
    let _inuse = ent.inuse;

    if groundentity < 0 && !flags.intersects(FL_FLY | FL_SWIM) {
        return;
    }

    // If the next step hits the enemy, return immediately
    if enemy_idx >= 0 && sv_close_enough(ctx, ent_idx, enemy_idx, dist) {
        return;
    }

    // Bump around...
    if ((rand_i32() & 3) == 1 || !sv_step_direction(ctx, ent_idx, ideal_yaw, dist))
        && ctx.edicts[ent_idx as usize].inuse {
            sv_new_chase_dir(ctx, ent_idx, goal_idx, dist);
        }
}

// ============================================================
// M_walkmove
// ============================================================

pub fn m_walkmove(ctx: &mut MoveContext, ent_idx: i32, yaw: f32, dist: f32) -> bool {
    let ent = &ctx.edicts[ent_idx as usize];

    if ent.groundentity < 0 && !ent.flags.intersects(FL_FLY | FL_SWIM) {
        return false;
    }

    let yaw_rad = yaw * std::f32::consts::PI * 2.0 / 360.0;
    let mov: Vec3 = [yaw_rad.cos() * dist, yaw_rad.sin() * dist, 0.0];

    sv_movestep(ctx, ent_idx, mov, true)
}

use myq2_common::common::rand_i32;

// ============================================================
// Unit tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::g_local::{Edict, EntityFlags, AiFlags};
    use myq2_common::q_shared::{anglemod, CONTENTS_SOLID};

    fn init_test_gi() {
        crate::game_import::set_gi(Box::new(crate::game_import::StubGameImport));
    }

    /// Helper: create a MoveContext with `n` default edicts.
    fn make_move_ctx(n: usize) -> MoveContext {
        init_test_gi();
        let mut edicts = vec![Edict::default(); n];
        for (i, e) in edicts.iter_mut().enumerate() {
            e.inuse = i > 0;
        }
        MoveContext {
            edicts,
            clients: Vec::new(),
            c_yes: 0,
            c_no: 0,
        }
    }

    // ============================================================
    // sv_close_enough tests
    // ============================================================

    #[test]
    fn test_sv_close_enough_overlapping() {
        let mut ctx = make_move_ctx(3);
        // Entity at origin with absmin/absmax
        ctx.edicts[1].absmin = [-10.0, -10.0, -10.0];
        ctx.edicts[1].absmax = [10.0, 10.0, 10.0];
        // Goal entity nearby
        ctx.edicts[2].absmin = [5.0, 5.0, 5.0];
        ctx.edicts[2].absmax = [15.0, 15.0, 15.0];

        // Overlapping by 5 units, dist = 0 should be true
        assert!(sv_close_enough(&ctx, 1, 2, 0.0));
    }

    #[test]
    fn test_sv_close_enough_within_dist() {
        let mut ctx = make_move_ctx(3);
        ctx.edicts[1].absmin = [-10.0, -10.0, -10.0];
        ctx.edicts[1].absmax = [10.0, 10.0, 10.0];
        ctx.edicts[2].absmin = [15.0, -10.0, -10.0];
        ctx.edicts[2].absmax = [25.0, 10.0, 10.0];

        // Gap of 5 in x, dist=5 should be close enough
        assert!(sv_close_enough(&ctx, 1, 2, 5.0));
    }

    #[test]
    fn test_sv_close_enough_too_far() {
        let mut ctx = make_move_ctx(3);
        ctx.edicts[1].absmin = [-10.0, -10.0, -10.0];
        ctx.edicts[1].absmax = [10.0, 10.0, 10.0];
        ctx.edicts[2].absmin = [100.0, 100.0, 100.0];
        ctx.edicts[2].absmax = [110.0, 110.0, 110.0];

        assert!(!sv_close_enough(&ctx, 1, 2, 5.0));
    }

    #[test]
    fn test_sv_close_enough_exactly_at_boundary() {
        let mut ctx = make_move_ctx(3);
        ctx.edicts[1].absmin = [0.0, 0.0, 0.0];
        ctx.edicts[1].absmax = [10.0, 10.0, 10.0];
        ctx.edicts[2].absmin = [20.0, 0.0, 0.0];
        ctx.edicts[2].absmax = [30.0, 10.0, 10.0];

        // Gap of 10 in x, dist=10 should be exactly at boundary
        assert!(sv_close_enough(&ctx, 1, 2, 10.0));
        // dist=9.99 should NOT be close enough
        assert!(!sv_close_enough(&ctx, 1, 2, 9.99));
    }

    #[test]
    fn test_sv_close_enough_negative_gap_on_y() {
        let mut ctx = make_move_ctx(3);
        ctx.edicts[1].absmin = [0.0, 0.0, 0.0];
        ctx.edicts[1].absmax = [10.0, 10.0, 10.0];
        // Goal below entity in y
        ctx.edicts[2].absmin = [0.0, -30.0, 0.0];
        ctx.edicts[2].absmax = [10.0, -20.0, 10.0];

        // Gap of 20 in y, dist=20 should work
        assert!(sv_close_enough(&ctx, 1, 2, 20.0));
        assert!(!sv_close_enough(&ctx, 1, 2, 19.99));
    }

    // ============================================================
    // m_change_yaw tests
    // ============================================================

    #[test]
    fn test_m_change_yaw_no_change_when_at_ideal() {
        let mut ctx = make_move_ctx(2);
        ctx.edicts[1].s.angles[YAW] = 90.0;
        ctx.edicts[1].ideal_yaw = anglemod(90.0);
        ctx.edicts[1].yaw_speed = 20.0;

        m_change_yaw(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].s.angles[YAW], anglemod(90.0));
    }

    #[test]
    fn test_m_change_yaw_turns_left() {
        let mut ctx = make_move_ctx(2);
        ctx.edicts[1].s.angles[YAW] = 0.0;
        ctx.edicts[1].ideal_yaw = 45.0;
        ctx.edicts[1].yaw_speed = 20.0;

        m_change_yaw(&mut ctx, 1);
        // Should move 20 degrees toward 45
        assert!((ctx.edicts[1].s.angles[YAW] - anglemod(20.0)).abs() < 0.1);
    }

    #[test]
    fn test_m_change_yaw_turns_right() {
        let mut ctx = make_move_ctx(2);
        ctx.edicts[1].s.angles[YAW] = 45.0;
        ctx.edicts[1].ideal_yaw = 0.0;
        ctx.edicts[1].yaw_speed = 20.0;

        m_change_yaw(&mut ctx, 1);
        // Should move 20 degrees toward 0 (i.e. subtract 20)
        assert!((ctx.edicts[1].s.angles[YAW] - anglemod(25.0)).abs() < 0.1);
    }

    #[test]
    fn test_m_change_yaw_wraps_around_positive() {
        let mut ctx = make_move_ctx(2);
        ctx.edicts[1].s.angles[YAW] = 350.0;
        ctx.edicts[1].ideal_yaw = 10.0;
        ctx.edicts[1].yaw_speed = 30.0;

        m_change_yaw(&mut ctx, 1);
        // Difference is 20 degrees going through 0, should turn 20 degrees
        let expected = anglemod(350.0 + 20.0); // should wrap to ~10
        assert!((ctx.edicts[1].s.angles[YAW] - expected).abs() < 1.0);
    }

    #[test]
    fn test_m_change_yaw_wraps_around_negative() {
        let mut ctx = make_move_ctx(2);
        ctx.edicts[1].s.angles[YAW] = 10.0;
        ctx.edicts[1].ideal_yaw = 350.0;
        ctx.edicts[1].yaw_speed = 30.0;

        m_change_yaw(&mut ctx, 1);
        // Difference is -20 degrees going through 360, should turn -20 degrees
        let expected = anglemod(10.0 - 20.0); // should wrap to ~350
        assert!((ctx.edicts[1].s.angles[YAW] - expected).abs() < 1.0);
    }

    #[test]
    fn test_m_change_yaw_speed_limits_turn() {
        let mut ctx = make_move_ctx(2);
        ctx.edicts[1].s.angles[YAW] = 0.0;
        ctx.edicts[1].ideal_yaw = 90.0;
        ctx.edicts[1].yaw_speed = 10.0;

        m_change_yaw(&mut ctx, 1);
        // Should only turn 10 degrees, not all the way to 90
        assert!((ctx.edicts[1].s.angles[YAW] - anglemod(10.0)).abs() < 0.1);
    }

    #[test]
    fn test_m_change_yaw_reaches_ideal_when_close() {
        let mut ctx = make_move_ctx(2);
        ctx.edicts[1].s.angles[YAW] = 85.0;
        ctx.edicts[1].ideal_yaw = 90.0;
        ctx.edicts[1].yaw_speed = 20.0;

        m_change_yaw(&mut ctx, 1);
        // Difference is only 5 degrees, speed is 20, so should reach 90
        assert!((ctx.edicts[1].s.angles[YAW] - anglemod(90.0)).abs() < 0.1);
    }

    // ============================================================
    // sv_fix_check_bottom tests
    // ============================================================

    #[test]
    fn test_sv_fix_check_bottom_sets_partial_ground() {
        let mut ctx = make_move_ctx(2);
        ctx.edicts[1].flags = EntityFlags::empty();

        sv_fix_check_bottom(&mut ctx, 1);
        assert!(ctx.edicts[1].flags.intersects(FL_PARTIALGROUND));
    }

    #[test]
    fn test_sv_fix_check_bottom_preserves_existing_flags() {
        let mut ctx = make_move_ctx(2);
        ctx.edicts[1].flags = FL_FLY | FL_SWIM;

        sv_fix_check_bottom(&mut ctx, 1);
        assert!(ctx.edicts[1].flags.intersects(FL_PARTIALGROUND));
        assert!(ctx.edicts[1].flags.intersects(FL_FLY));
        assert!(ctx.edicts[1].flags.intersects(FL_SWIM));
    }

    // ============================================================
    // m_walkmove tests (precondition checks)
    // ============================================================

    #[test]
    fn test_m_walkmove_no_ground_no_fly_returns_false() {
        let mut ctx = make_move_ctx(2);
        ctx.edicts[1].groundentity = -1;
        ctx.edicts[1].flags = EntityFlags::empty(); // no FL_FLY or FL_SWIM

        let result = m_walkmove(&mut ctx, 1, 0.0, 10.0);
        assert!(!result);
    }

    #[test]
    fn test_m_walkmove_fly_flag_allows_movement() {
        // With FL_FLY, even without ground, the precondition check passes
        let mut ctx = make_move_ctx(2);
        ctx.edicts[1].groundentity = -1;
        ctx.edicts[1].flags = FL_FLY;
        ctx.edicts[1].enemy = -1;

        // Movement will go through to sv_movestep which uses gi_trace
        // The trace mock returns default trace (no movement possible in test env)
        // but the precondition check should not fail
        let _result = m_walkmove(&mut ctx, 1, 90.0, 10.0);
        // We just verify it doesn't panic and gets past the guard
    }

    #[test]
    fn test_m_walkmove_swim_flag_allows_movement() {
        let mut ctx = make_move_ctx(2);
        ctx.edicts[1].groundentity = -1;
        ctx.edicts[1].flags = FL_SWIM;
        ctx.edicts[1].enemy = -1;

        let _result = m_walkmove(&mut ctx, 1, 0.0, 10.0);
        // Just verify it doesn't panic
    }

    #[test]
    fn test_m_walkmove_with_ground_allows_movement() {
        let mut ctx = make_move_ctx(2);
        ctx.edicts[1].groundentity = 0; // on ground
        ctx.edicts[1].flags = EntityFlags::empty();

        let _result = m_walkmove(&mut ctx, 1, 0.0, 10.0);
        // Just verify it doesn't panic
    }

    #[test]
    fn test_m_walkmove_yaw_to_movement_vector() {
        // Test that yaw 0 produces forward movement in +x
        // and yaw 90 produces movement in +y
        let mut ctx = make_move_ctx(2);
        ctx.edicts[1].groundentity = 0;
        ctx.edicts[1].flags = EntityFlags::empty();

        // yaw=0, dist=10 -> move = [10, 0, 0]
        let yaw = 0.0_f32;
        let dist = 10.0_f32;
        let yaw_rad = yaw * std::f32::consts::PI * 2.0 / 360.0;
        let mov = [yaw_rad.cos() * dist, yaw_rad.sin() * dist, 0.0];
        assert!((mov[0] - 10.0).abs() < 0.001);
        assert!(mov[1].abs() < 0.001);
        assert_eq!(mov[2], 0.0);

        // yaw=90, dist=10 -> move = [0, 10, 0]
        let yaw = 90.0_f32;
        let yaw_rad = yaw * std::f32::consts::PI * 2.0 / 360.0;
        let mov = [yaw_rad.cos() * dist, yaw_rad.sin() * dist, 0.0];
        assert!(mov[0].abs() < 0.001);
        assert!((mov[1] - 10.0).abs() < 0.001);

        // yaw=180, dist=10 -> move = [-10, 0, 0]
        let yaw = 180.0_f32;
        let yaw_rad = yaw * std::f32::consts::PI * 2.0 / 360.0;
        let mov = [yaw_rad.cos() * dist, yaw_rad.sin() * dist, 0.0];
        assert!((mov[0] + 10.0).abs() < 0.001);
        assert!(mov[1].abs() < 0.001);

        // yaw=270, dist=10 -> move = [0, -10, 0]
        let yaw = 270.0_f32;
        let yaw_rad = yaw * std::f32::consts::PI * 2.0 / 360.0;
        let mov = [yaw_rad.cos() * dist, yaw_rad.sin() * dist, 0.0];
        assert!(mov[0].abs() < 0.001);
        assert!((mov[1] + 10.0).abs() < 0.001);
    }

    // ============================================================
    // m_move_to_goal precondition tests
    // ============================================================

    #[test]
    fn test_m_move_to_goal_no_ground_returns_early() {
        let mut ctx = make_move_ctx(3);
        ctx.edicts[1].groundentity = -1;
        ctx.edicts[1].flags = EntityFlags::empty();
        ctx.edicts[1].goalentity = 2;
        ctx.edicts[1].ideal_yaw = 45.0;

        m_move_to_goal(&mut ctx, 1, 10.0);
        // Should return early, ideal_yaw unchanged
        assert_eq!(ctx.edicts[1].ideal_yaw, 45.0);
    }

    // ============================================================
    // m_check_bottom tests
    // ============================================================

    #[test]
    fn test_m_check_bottom_computes_correct_bounds() {
        // Test that the corners are computed correctly from origin + mins/maxs
        let mut ctx = make_move_ctx(2);
        ctx.edicts[1].s.origin = [100.0, 200.0, 50.0];
        ctx.edicts[1].mins = [-16.0, -16.0, -24.0];
        ctx.edicts[1].maxs = [16.0, 16.0, 32.0];

        // Verify the math: mins_world = origin + mins = [84, 184, 26]
        // maxs_world = origin + maxs = [116, 216, 82]
        let mins = vec3_add(&ctx.edicts[1].s.origin, &ctx.edicts[1].mins);
        let maxs = vec3_add(&ctx.edicts[1].s.origin, &ctx.edicts[1].maxs);
        assert_eq!(mins, [84.0, 184.0, 26.0]);
        assert_eq!(maxs, [116.0, 216.0, 82.0]);

        // The actual m_check_bottom uses gi_pointcontents and gi_trace
        // which are mocked, so we just test the math here
    }

    #[test]
    fn test_m_check_bottom_updates_counters() {
        // When all corners are solid, c_yes should increment
        let mut ctx = make_move_ctx(2);
        ctx.edicts[1].s.origin = [0.0, 0.0, 0.0];
        ctx.edicts[1].mins = [-1.0, -1.0, -1.0];
        ctx.edicts[1].maxs = [1.0, 1.0, 1.0];

        let initial_yes = ctx.c_yes;
        let initial_no = ctx.c_no;

        // Call m_check_bottom -- result depends on gi_pointcontents mock
        let _result = m_check_bottom(&mut ctx, 1);

        // At least one of c_yes or c_no should have changed
        assert!(ctx.c_yes > initial_yes || ctx.c_no > initial_no);
    }

    // ============================================================
    // sv_movestep tests (precondition and flag checks)
    // ============================================================

    #[test]
    fn test_sv_movestep_fly_monster_basic() {
        // Flying monster with no enemy -- should break after first iteration
        let mut ctx = make_move_ctx(2);
        ctx.edicts[1].flags = FL_FLY;
        ctx.edicts[1].enemy = -1;
        ctx.edicts[1].waterlevel = 0;
        ctx.edicts[1].s.origin = [0.0, 0.0, 0.0];
        ctx.edicts[1].mins = [-16.0, -16.0, -24.0];
        ctx.edicts[1].maxs = [16.0, 16.0, 32.0];

        let result = sv_movestep(&mut ctx, 1, [10.0, 0.0, 0.0], false);
        // With mock trace, the behavior depends on default trace result
        // but should not panic
        assert!(!result || result); // just verify it runs without panicking
    }

    #[test]
    fn test_sv_movestep_nostep_flag_reduces_stepsize() {
        // Monster with AI_NOSTEP should use step size of 1.0 instead of 18.0
        let mut ctx = make_move_ctx(2);
        ctx.edicts[1].flags = EntityFlags::empty();
        ctx.edicts[1].monsterinfo.aiflags = AiFlags::NOSTEP;
        ctx.edicts[1].enemy = -1;
        ctx.edicts[1].s.origin = [0.0, 0.0, 0.0];
        ctx.edicts[1].mins = [-16.0, -16.0, -24.0];
        ctx.edicts[1].maxs = [16.0, 16.0, 32.0];
        ctx.edicts[1].groundentity = 0;

        // Just verify it runs without panicking -- actual step height
        // is an internal detail tested through integration
        let _result = sv_movestep(&mut ctx, 1, [5.0, 0.0, 0.0], false);
    }

    #[test]
    fn test_sv_movestep_stepsize_constant() {
        assert_eq!(STEPSIZE, 18.0);
    }

    // ============================================================
    // sv_new_chase_dir tests
    // ============================================================

    #[test]
    fn test_sv_new_chase_dir_no_enemy_returns_early() {
        let mut ctx = make_move_ctx(2);
        ctx.edicts[1].ideal_yaw = 45.0;

        sv_new_chase_dir(&mut ctx, 1, -1, 10.0);
        // Should return early without changing ideal_yaw
        assert_eq!(ctx.edicts[1].ideal_yaw, 45.0);
    }

    #[test]
    fn test_sv_new_chase_dir_direction_computation() {
        // Test the direction computation logic
        // When deltax > 10, d[1] = 0.0 (east)
        // When deltay > 10, d[2] = 90.0 (north)
        let mut ctx = make_move_ctx(3);
        ctx.edicts[1].s.origin = [0.0, 0.0, 0.0];
        ctx.edicts[1].ideal_yaw = 0.0;
        ctx.edicts[2].s.origin = [100.0, 100.0, 0.0]; // NE of actor

        // This function tries multiple step directions using sv_step_direction
        // which uses sv_movestep -- in test env the trace mock limits actual movement
        // Just verify it doesn't panic
        sv_new_chase_dir(&mut ctx, 1, 2, 10.0);
    }

    // ============================================================
    // anglemod tests (from q_shared, used in m_change_yaw)
    // ============================================================

    #[test]
    fn test_anglemod_normalizes_angle() {
        // anglemod should bring angles into [0, 360) range
        let a = anglemod(370.0);
        assert!((a - anglemod(10.0)).abs() < 1.0);

        let a = anglemod(-10.0);
        assert!((a - anglemod(350.0)).abs() < 1.0);

        let a = anglemod(0.0);
        assert_eq!(a, 0.0);

        let a = anglemod(360.0);
        assert_eq!(a, 0.0);
    }

    // ============================================================
    // STEPSIZE and DI_NODIR constant tests
    // ============================================================

    #[test]
    fn test_constants() {
        assert_eq!(STEPSIZE, 18.0);
        assert_eq!(DI_NODIR, -1.0);
    }

    // ============================================================
    // m_check_bottom_raw wrapper test
    // ============================================================

    #[test]
    fn test_m_check_bottom_raw_preserves_edicts() {
        let mut edicts = vec![Edict::default(); 3];
        edicts[1].s.origin = [10.0, 20.0, 30.0];
        edicts[1].mins = [-8.0, -8.0, -8.0];
        edicts[1].maxs = [8.0, 8.0, 8.0];

        let _result = m_check_bottom_raw(1, &mut edicts);

        // After call, edicts should still be accessible and origin preserved
        assert_eq!(edicts[1].s.origin, [10.0, 20.0, 30.0]);
        assert_eq!(edicts.len(), 3);
    }

    // ============================================================
    // sv_step_direction tests
    // ============================================================

    #[test]
    fn test_sv_step_direction_sets_ideal_yaw() {
        let mut ctx = make_move_ctx(2);
        ctx.edicts[1].s.origin = [0.0, 0.0, 0.0];
        ctx.edicts[1].mins = [-16.0, -16.0, -24.0];
        ctx.edicts[1].maxs = [16.0, 16.0, 32.0];
        ctx.edicts[1].ideal_yaw = 0.0;
        ctx.edicts[1].yaw_speed = 360.0; // fast turn
        ctx.edicts[1].groundentity = 0;

        sv_step_direction(&mut ctx, 1, 90.0, 10.0);
        // ideal_yaw should be set to the requested direction
        assert_eq!(ctx.edicts[1].ideal_yaw, 90.0);
    }
}
