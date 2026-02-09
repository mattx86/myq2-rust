// g_chase.rs — Chase camera logic
// Converted from: myq2-original/game/g_chase.c

use crate::g_local::*;
use crate::game_import::*;
use myq2_common::q_shared::{
    angle2short, angle_vectors, vector_copy, vector_ma, vector_normalize, Vec3,
    VEC3_ORIGIN, MASK_SOLID, PITCH, YAW, ROLL, PMF_NO_PREDICTION, PmType,
};

/// UpdateChaseCam — update the chase camera position for a spectating player.
///
/// Equivalent of C `UpdateChaseCam(edict_t *ent)`.
pub fn update_chase_cam(ctx: &mut GameCtx, ent_idx: usize) {
    let chase_target_idx = ctx.client_of(ent_idx).chase_target;

    // Is our chase target gone?
    if chase_target_idx < 0 {
        return;
    }
    let chase_target = chase_target_idx as usize;

    if !ctx.edicts[chase_target].inuse
        || ctx.client_of(chase_target).resp.spectator
    {
        let old = chase_target_idx;
        chase_next(ctx, ent_idx);
        if ctx.client_of(ent_idx).chase_target == old {
            ctx.client_of_mut(ent_idx).chase_target = -1;
            ctx.client_of_mut(ent_idx).ps.pmove.pm_flags &= !PMF_NO_PREDICTION;
            return;
        }
    }

    let targ_idx = ctx.client_of(ent_idx).chase_target as usize;

    let mut ownerv = vector_copy(&ctx.edicts[targ_idx].s.origin);
    let _oldgoal = vector_copy(&ctx.edicts[ent_idx].s.origin);

    ownerv[2] += ctx.edicts[targ_idx].viewheight as f32;

    let mut angles = vector_copy(&ctx.client_of(targ_idx).v_angle);
    if angles[PITCH] > 56.0 {
        angles[PITCH] = 56.0;
    }

    let mut forward: Vec3 = [0.0; 3];
    let mut right: Vec3 = [0.0; 3];
    angle_vectors(&angles, Some(&mut forward), Some(&mut right), None);
    vector_normalize(&mut forward);

    let mut o = vector_ma(&ownerv, -30.0, &forward);

    if o[2] < ctx.edicts[targ_idx].s.origin[2] + 20.0 {
        o[2] = ctx.edicts[targ_idx].s.origin[2] + 20.0;
    }

    // jump animation lifts
    if ctx.edicts[targ_idx].groundentity < 0 {
        o[2] += 16.0;
    }

    let trace = gi_trace(&ownerv, &VEC3_ORIGIN, &VEC3_ORIGIN, &o, targ_idx as i32, MASK_SOLID);

    let mut goal = vector_copy(&trace.endpos);

    // VectorMA(goal, 2, forward, goal)
    goal[0] += 2.0 * forward[0];
    goal[1] += 2.0 * forward[1];
    goal[2] += 2.0 * forward[2];

    // pad for floors and ceilings
    let mut o = vector_copy(&goal);
    o[2] += 6.0;
    let trace = gi_trace(&goal, &VEC3_ORIGIN, &VEC3_ORIGIN, &o, targ_idx as i32, MASK_SOLID);
    if trace.fraction < 1.0 {
        goal = vector_copy(&trace.endpos);
        goal[2] -= 6.0;
    }

    let mut o = vector_copy(&goal);
    o[2] -= 6.0;
    let trace = gi_trace(&goal, &VEC3_ORIGIN, &VEC3_ORIGIN, &o, targ_idx as i32, MASK_SOLID);
    if trace.fraction < 1.0 {
        goal = vector_copy(&trace.endpos);
        goal[2] += 6.0;
    }

    if ctx.edicts[targ_idx].deadflag != 0 {
        ctx.client_of_mut(ent_idx).ps.pmove.pm_type = PmType::Dead;
    } else {
        ctx.client_of_mut(ent_idx).ps.pmove.pm_type = PmType::Freeze;
    }

    ctx.edicts[ent_idx].s.origin = goal;

    for i in 0..3 {
        let targ_v_angle_i = ctx.client_of(targ_idx).v_angle[i];
        let cmd_angles_i = ctx.client_of(ent_idx).resp.cmd_angles[i];
        ctx.client_of_mut(ent_idx).ps.pmove.delta_angles[i] =
            angle2short(targ_v_angle_i - cmd_angles_i) as i16;
    }

    if ctx.edicts[targ_idx].deadflag != 0 {
        let killer_yaw = ctx.client_of(targ_idx).killer_yaw;
        let cl = ctx.client_of_mut(ent_idx);
        cl.ps.viewangles[ROLL] = 40.0;
        cl.ps.viewangles[PITCH] = -15.0;
        cl.ps.viewangles[YAW] = killer_yaw;
    } else {
        let targ_v_angle = vector_copy(&ctx.client_of(targ_idx).v_angle);
        let cl = ctx.client_of_mut(ent_idx);
        cl.ps.viewangles = targ_v_angle;
        cl.v_angle = targ_v_angle;
    }

    ctx.edicts[ent_idx].viewheight = 0;
    ctx.client_of_mut(ent_idx).ps.pmove.pm_flags |= PMF_NO_PREDICTION;
    gi_linkentity(ent_idx as i32);
}

/// ChaseNext — cycle to the next valid chase target.
///
/// Equivalent of C `ChaseNext(edict_t *ent)`.
pub fn chase_next(ctx: &mut GameCtx, ent_idx: usize) {
    let chase_target = ctx.client_of(ent_idx).chase_target;
    if chase_target < 0 {
        return;
    }

    let mut i = chase_target;
    let maxclients = ctx.maxclients as i32;

    loop {
        i += 1;
        if i > maxclients {
            i = 1;
        }
        let e = i as usize;
        if !ctx.edicts[e].inuse {
            if i == chase_target {
                break;
            }
            continue;
        }
        if !ctx.client_of(e).resp.spectator {
            break;
        }
        if i == chase_target {
            break;
        }
    }

    ctx.client_of_mut(ent_idx).chase_target = i;
    ctx.client_of_mut(ent_idx).update_chase = true;
}

/// ChasePrev — cycle to the previous valid chase target.
///
/// Equivalent of C `ChasePrev(edict_t *ent)`.
pub fn chase_prev(ctx: &mut GameCtx, ent_idx: usize) {
    let chase_target = ctx.client_of(ent_idx).chase_target;
    if chase_target < 0 {
        return;
    }

    let mut i = chase_target;
    let maxclients = ctx.maxclients as i32;

    loop {
        i -= 1;
        if i < 1 {
            i = maxclients;
        }
        let e = i as usize;
        if !ctx.edicts[e].inuse {
            if i == chase_target {
                break;
            }
            continue;
        }
        if !ctx.client_of(e).resp.spectator {
            break;
        }
        if i == chase_target {
            break;
        }
    }

    ctx.client_of_mut(ent_idx).chase_target = i;
    ctx.client_of_mut(ent_idx).update_chase = true;
}

/// GetChaseTarget — find the first valid chase target for a spectator.
///
/// Equivalent of C `GetChaseTarget(edict_t *ent)`.
pub fn get_chase_target(ctx: &mut GameCtx, ent_idx: usize) {
    let maxclients = ctx.maxclients as i32;

    for i in 1..=maxclients {
        let other = i as usize;
        if ctx.edicts[other].inuse && !ctx.client_of(other).resp.spectator {
            ctx.client_of_mut(ent_idx).chase_target = i;
            ctx.client_of_mut(ent_idx).update_chase = true;
            update_chase_cam(ctx, ent_idx);
            return;
        }
    }

    gi_centerprintf(ent_idx as i32, "No other players to chase.");
}
