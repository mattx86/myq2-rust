// g_misc.rs — Miscellaneous entity functions
// Converted from: myq2-original/game/g_misc.c

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
use crate::game::Multicast;
use crate::game_import as gi;
use myq2_common::common::{frand as random, crand as crandom, rand_i32};
use myq2_common::q_shared::{
    vector_clear, vector_set, vector_normalize, VEC3_ORIGIN, PMF_TIME_TELEPORT,
    MASK_MONSTERSOLID,
    vector_copy_to as vector_copy, vector_add_to as vector_add,
    vector_subtract_to as vector_subtract, vector_scale_to as vector_scale,
    vector_ma_to as vector_ma, vectoyaw, vectoangles, angle_vectors,
};

// ============================================================
// Local constants
// ============================================================

const START_OFF: i32 = 1;
const CLOCK_MESSAGE_SIZE: usize = 16;

// EF_*, RF_*, CHAN_*, ATTN_* come from g_local::* re-export (myq2_common::q_shared)

// Events — imported from q_shared via g_local
// EV_OTHER_TELEPORT and EV_PLAYER_TELEPORT come from g_local::* re-export
// TE_EXPLOSION1, TE_EXPLOSION2 come from g_local::* re-export
// SVC_TEMP_ENTITY comes from g_local::* re-export
// PMF_TIME_TELEPORT comes from myq2_common::q_shared (value = 32)
// DAMAGE_YES / DAMAGE_NO come from g_local::*

// CS_LIGHTS comes from g_local (re-exported from q_shared)


// ============================================================
// Callback indices for think/touch/use/die dispatch tables
// ============================================================

pub use crate::dispatch::THINK_GIB;
pub use crate::dispatch::THINK_FREE_EDICT as THINK_G_FREE_EDICT;
pub use crate::dispatch::THINK_TH_VIEWTHING;
pub use crate::dispatch::THINK_MISC_BLACKHOLE;
pub use crate::dispatch::THINK_MISC_EASTERTANK;
pub use crate::dispatch::THINK_MISC_EASTERCHICK;
pub use crate::dispatch::THINK_MISC_EASTERCHICK2;
pub use crate::dispatch::THINK_COMMANDER_BODY;
pub use crate::dispatch::THINK_COMMANDER_BODY_DROP;
pub use crate::dispatch::THINK_MISC_BANNER;
pub use crate::dispatch::THINK_MISC_SATELLITE_DISH;
pub use crate::dispatch::THINK_BARREL_EXPLODE;
pub use crate::dispatch::THINK_FUNC_OBJECT_RELEASE;
pub use crate::dispatch::THINK_FUNC_CLOCK;
pub use crate::dispatch::THINK_FUNC_TRAIN_FIND;
pub use crate::dispatch::THINK_MISC_VIPER_BOMB_PRETHINK;
pub use crate::dispatch::THINK_M_DROPTOFLOOR;

pub use crate::dispatch::TOUCH_GIB;
pub use crate::dispatch::TOUCH_PATH_CORNER;
pub use crate::dispatch::TOUCH_POINT_COMBAT;
pub use crate::dispatch::TOUCH_FUNC_OBJECT;
pub use crate::dispatch::TOUCH_BARREL;
pub use crate::dispatch::TOUCH_MISC_VIPER_BOMB;
pub use crate::dispatch::TOUCH_TELEPORTER;

pub use crate::dispatch::USE_AREAPORTAL;
pub use crate::dispatch::USE_LIGHT;
pub use crate::dispatch::USE_FUNC_WALL;
pub use crate::dispatch::USE_FUNC_OBJECT;
pub use crate::dispatch::USE_FUNC_EXPLOSIVE;
pub use crate::dispatch::USE_FUNC_EXPLOSIVE_SPAWN;
pub use crate::dispatch::USE_MISC_BLACKHOLE;
pub use crate::dispatch::USE_COMMANDER_BODY;
pub use crate::dispatch::USE_MISC_SATELLITE_DISH;
pub use crate::dispatch::USE_MISC_VIPER;
pub use crate::dispatch::USE_MISC_VIPER_BOMB;
pub use crate::dispatch::USE_MISC_STROGG_SHIP;
pub use crate::dispatch::USE_TARGET_STRING;
pub use crate::dispatch::USE_FUNC_CLOCK;
pub use crate::dispatch::USE_TRAIN;

pub use crate::dispatch::DIE_GIB;
pub use crate::dispatch::DIE_DEBRIS;
pub use crate::dispatch::DIE_FUNC_EXPLOSIVE as DIE_FUNC_EXPLOSIVE_EXPLODE;
pub use crate::dispatch::DIE_BARREL as DIE_BARREL_DELAY;
pub use crate::dispatch::DIE_MISC_DEADSOLDIER;

// ============================================================
// Helper functions
// ============================================================

// rand_i32 imported from myq2_common::common


fn angle2short(a: f32) -> i16 {
    myq2_common::q_shared::angle2short(a) as i16
}

use crate::g_utils::{vtos, g_free_edict, g_spawn};


use crate::game_import::{gi_dprintf, gi_write_byte, gi_write_position, gi_multicast};

// ============================================================
// func_group
// Used to group brushes together just for editor convenience.
// ============================================================

// (No code — func_group is editor-only)

// ============================================================
// func_areaportal
// ============================================================

/// Use_Areaportal — toggles area portal state
pub fn use_areaportal(ctx: &mut GameContext, ent_idx: usize, _other_idx: usize, _activator_idx: usize) {
    let ent = &mut ctx.edicts[ent_idx];
    ent.count ^= 1; // toggle state
    let style = ent.style;
    let count = ent.count;
    gi::gi_set_area_portal_state(style, count != 0);
}

/// SP_func_areaportal — spawn function for func_areaportal
pub fn sp_func_areaportal(ctx: &mut GameContext, ent_idx: usize) {
    ctx.edicts[ent_idx].use_fn = Some(USE_AREAPORTAL);
    ctx.edicts[ent_idx].count = 0; // always start closed
}

// ============================================================
// Misc functions
// ============================================================

/// VelocityForDamage — compute velocity vector for damage-based gib toss
pub fn velocity_for_damage(damage: i32, v: &mut [f32; 3]) {
    v[0] = 100.0 * crandom();
    v[1] = 100.0 * crandom();
    v[2] = 200.0 + 100.0 * random();

    if damage < 50 {
        vector_scale(&v.clone(), 0.7, v);
    } else {
        vector_scale(&v.clone(), 1.2, v);
    }
}

/// ClipGibVelocity — clamp gib velocity to reasonable bounds
pub fn clip_gib_velocity(ent: &mut Edict) {
    if ent.velocity[0] < -300.0 {
        ent.velocity[0] = -300.0;
    } else if ent.velocity[0] > 300.0 {
        ent.velocity[0] = 300.0;
    }
    if ent.velocity[1] < -300.0 {
        ent.velocity[1] = -300.0;
    } else if ent.velocity[1] > 300.0 {
        ent.velocity[1] = 300.0;
    }
    if ent.velocity[2] < 200.0 {
        ent.velocity[2] = 200.0; // always some upwards
    } else if ent.velocity[2] > 500.0 {
        ent.velocity[2] = 500.0;
    }
}

// ============================================================
// Gibs
// ============================================================

/// gib_think — animate gib frames, then schedule free
pub fn gib_think(ctx: &mut GameContext, self_idx: usize) {
    ctx.edicts[self_idx].s.frame += 1;
    ctx.edicts[self_idx].nextthink = ctx.level.time + FRAMETIME;

    if ctx.edicts[self_idx].s.frame == 10 {
        ctx.edicts[self_idx].think_fn = Some(THINK_G_FREE_EDICT);
        ctx.edicts[self_idx].nextthink = ctx.level.time + 8.0 + random() * 10.0;
    }
}

/// gib_touch — gib hits ground, play sound and orient
pub fn gib_touch(
    ctx: &mut GameContext,
    self_idx: usize,
    _other_idx: usize,
    plane: Option<&[f32; 3]>, // plane normal, None if no plane
    _surf: Option<usize>,
) {
    if ctx.edicts[self_idx].groundentity < 0 {
        return;
    }

    ctx.edicts[self_idx].touch_fn = None;

    if let Some(normal) = plane {
        let snd = gi::gi_soundindex("misc/fhit3.wav");
        gi::gi_sound(self_idx as i32, CHAN_VOICE, snd, 1.0, ATTN_NORM, 0.0);

        let mut normal_angles = [0.0_f32; 3];
        let mut right = [0.0_f32; 3];
        vectoangles(normal, &mut normal_angles);
        angle_vectors(&normal_angles, None, Some(&mut right), None);
        vectoangles(&right, &mut ctx.edicts[self_idx].s.angles);

        if ctx.edicts[self_idx].s.modelindex == ctx.sm_meat_index {
            ctx.edicts[self_idx].s.frame += 1;
            ctx.edicts[self_idx].think_fn = Some(THINK_GIB);
            ctx.edicts[self_idx].nextthink = ctx.level.time + FRAMETIME;
        }
    }
}

/// gib_die — free the gib entity
pub fn gib_die(ctx: &mut GameContext, self_idx: usize, _inflictor: usize, _attacker: usize, _damage: i32, _point: &[f32; 3]) {
    g_free_edict(ctx, self_idx);
}

/// ThrowGib — spawn a gib chunk
pub fn throw_gib(ctx: &mut GameContext, self_idx: usize, gibname: &str, damage: i32, gib_type: i32) {
    ctx.maxclients = ctx.game.maxclients as f32;
    let gib_idx = g_spawn(ctx);

    let mut size = [0.0_f32; 3];
    let mut origin = [0.0_f32; 3];
    vector_scale(&ctx.edicts[self_idx].size, 0.5, &mut size);
    vector_add(&ctx.edicts[self_idx].absmin, &size, &mut origin);
    ctx.edicts[gib_idx].s.origin[0] = origin[0] + crandom() * size[0];
    ctx.edicts[gib_idx].s.origin[1] = origin[1] + crandom() * size[1];
    ctx.edicts[gib_idx].s.origin[2] = origin[2] + crandom() * size[2];

    gi::gi_setmodel(gib_idx as i32, gibname);
    ctx.edicts[gib_idx].solid = Solid::Not;
    ctx.edicts[gib_idx].s.effects |= EF_GIB;
    ctx.edicts[gib_idx].flags |= FL_NO_KNOCKBACK;
    ctx.edicts[gib_idx].takedamage = DAMAGE_YES;
    ctx.edicts[gib_idx].die_fn = Some(DIE_GIB);

    let vscale;
    if gib_type == GIB_ORGANIC {
        ctx.edicts[gib_idx].movetype = MoveType::Toss;
        ctx.edicts[gib_idx].touch_fn = Some(TOUCH_GIB);
        vscale = 0.5;
    } else {
        ctx.edicts[gib_idx].movetype = MoveType::Bounce;
        vscale = 1.0;
    }

    let mut vd = [0.0_f32; 3];
    velocity_for_damage(damage, &mut vd);
    let self_vel = ctx.edicts[self_idx].velocity;
    vector_ma(&self_vel, vscale, &vd, &mut ctx.edicts[gib_idx].velocity);
    clip_gib_velocity(&mut ctx.edicts[gib_idx]);
    ctx.edicts[gib_idx].avelocity[0] = random() * 600.0;
    ctx.edicts[gib_idx].avelocity[1] = random() * 600.0;
    ctx.edicts[gib_idx].avelocity[2] = random() * 600.0;

    ctx.edicts[gib_idx].think_fn = Some(THINK_G_FREE_EDICT);
    ctx.edicts[gib_idx].nextthink = ctx.level.time + 10.0 + random() * 10.0;

    gi::gi_linkentity(gib_idx as i32);
}

/// ThrowHead — replace entity model with a gib head
pub fn throw_head(ctx: &mut GameContext, self_idx: usize, gibname: &str, damage: i32, gib_type: i32) {
    ctx.edicts[self_idx].s.skinnum = 0;
    ctx.edicts[self_idx].s.frame = 0;
    vector_clear(&mut ctx.edicts[self_idx].mins);
    vector_clear(&mut ctx.edicts[self_idx].maxs);

    ctx.edicts[self_idx].s.modelindex2 = 0;
    gi::gi_setmodel(self_idx as i32, gibname);
    ctx.edicts[self_idx].solid = Solid::Not;
    ctx.edicts[self_idx].s.effects |= EF_GIB;
    ctx.edicts[self_idx].s.effects &= !EF_FLIES;
    ctx.edicts[self_idx].s.sound = 0;
    ctx.edicts[self_idx].flags |= FL_NO_KNOCKBACK;
    ctx.edicts[self_idx].svflags &= !SVF_MONSTER;
    ctx.edicts[self_idx].takedamage = DAMAGE_YES;
    ctx.edicts[self_idx].die_fn = Some(DIE_GIB);

    let vscale;
    if gib_type == GIB_ORGANIC {
        ctx.edicts[self_idx].movetype = MoveType::Toss;
        ctx.edicts[self_idx].touch_fn = Some(TOUCH_GIB);
        vscale = 0.5;
    } else {
        ctx.edicts[self_idx].movetype = MoveType::Bounce;
        vscale = 1.0;
    }

    let mut vd = [0.0_f32; 3];
    velocity_for_damage(damage, &mut vd);
    let self_vel = ctx.edicts[self_idx].velocity;
    vector_ma(&self_vel, vscale, &vd, &mut ctx.edicts[self_idx].velocity);
    clip_gib_velocity(&mut ctx.edicts[self_idx]);

    // YAW = index 1
    ctx.edicts[self_idx].avelocity[1] = crandom() * 600.0;

    ctx.edicts[self_idx].think_fn = Some(THINK_G_FREE_EDICT);
    ctx.edicts[self_idx].nextthink = ctx.level.time + 10.0 + random() * 10.0;

    gi::gi_linkentity(self_idx as i32);
}

/// ThrowClientHead — replace player model with a random gib head
pub fn throw_client_head(ctx: &mut GameContext, self_idx: usize, damage: i32) {
    let mut vd = [0.0_f32; 3];
    let gibname;

    if rand_i32() & 1 != 0 {
        gibname = "models/objects/gibs/head2/tris.md2";
        ctx.edicts[self_idx].s.skinnum = 1; // second skin is player
    } else {
        gibname = "models/objects/gibs/skull/tris.md2";
        ctx.edicts[self_idx].s.skinnum = 0;
    }

    ctx.edicts[self_idx].s.origin[2] += 32.0;
    ctx.edicts[self_idx].s.frame = 0;
    gi::gi_setmodel(self_idx as i32, gibname);
    vector_set(&mut ctx.edicts[self_idx].mins, -16.0, -16.0, 0.0);
    vector_set(&mut ctx.edicts[self_idx].maxs, 16.0, 16.0, 16.0);

    ctx.edicts[self_idx].takedamage = DAMAGE_NO;
    ctx.edicts[self_idx].solid = Solid::Not;
    ctx.edicts[self_idx].s.effects = EF_GIB;
    ctx.edicts[self_idx].s.sound = 0;
    ctx.edicts[self_idx].flags |= FL_NO_KNOCKBACK;

    ctx.edicts[self_idx].movetype = MoveType::Bounce;
    velocity_for_damage(damage, &mut vd);
    let self_vel = ctx.edicts[self_idx].velocity;
    vector_add(&self_vel, &vd, &mut ctx.edicts[self_idx].velocity);

    if let Some(client_idx) = ctx.edicts[self_idx].client {
        // bodies in the queue don't have a client anymore
        ctx.clients[client_idx].anim_priority = ANIM_DEATH;
        ctx.clients[client_idx].anim_end = ctx.edicts[self_idx].s.frame;
    } else {
        ctx.edicts[self_idx].think_fn = None;
        ctx.edicts[self_idx].nextthink = 0.0;
    }

    gi::gi_linkentity(self_idx as i32);
}

// ============================================================
// Debris
// ============================================================

/// debris_die — free debris entity
pub fn debris_die(ctx: &mut GameContext, self_idx: usize, _inflictor: usize, _attacker: usize, _damage: i32, _point: &[f32; 3]) {
    g_free_edict(ctx, self_idx);
}

/// ThrowDebris — spawn a debris chunk
pub fn throw_debris(ctx: &mut GameContext, self_idx: usize, modelname: &str, speed: f32, origin: &[f32; 3]) {
    ctx.maxclients = ctx.game.maxclients as f32;
    let chunk_idx = g_spawn(ctx);
    vector_copy(origin, &mut ctx.edicts[chunk_idx].s.origin);
    gi::gi_setmodel(chunk_idx as i32, modelname);

    let mut v = [0.0_f32; 3];
    v[0] = 100.0 * crandom();
    v[1] = 100.0 * crandom();
    v[2] = 100.0 + 100.0 * crandom();
    let self_vel = ctx.edicts[self_idx].velocity;
    vector_ma(&self_vel, speed, &v, &mut ctx.edicts[chunk_idx].velocity);

    ctx.edicts[chunk_idx].movetype = MoveType::Bounce;
    ctx.edicts[chunk_idx].solid = Solid::Not;
    ctx.edicts[chunk_idx].avelocity[0] = random() * 600.0;
    ctx.edicts[chunk_idx].avelocity[1] = random() * 600.0;
    ctx.edicts[chunk_idx].avelocity[2] = random() * 600.0;
    ctx.edicts[chunk_idx].think_fn = Some(THINK_G_FREE_EDICT);
    ctx.edicts[chunk_idx].nextthink = ctx.level.time + 5.0 + random() * 5.0;
    ctx.edicts[chunk_idx].s.frame = 0;
    ctx.edicts[chunk_idx].flags = EntityFlags::empty();
    ctx.edicts[chunk_idx].classname = "debris".to_string();
    ctx.edicts[chunk_idx].takedamage = DAMAGE_YES;
    ctx.edicts[chunk_idx].die_fn = Some(DIE_DEBRIS);
    gi::gi_linkentity(chunk_idx as i32);
}

// ============================================================
// Explosions
// ============================================================

/// BecomeExplosion1 — turn entity into explosion1 temp entity and free it
pub fn become_explosion1(ctx: &mut GameContext, self_idx: usize) {
    gi_write_byte(SVC_TEMP_ENTITY);
    gi_write_byte(TE_EXPLOSION1);
    gi_write_position(&ctx.edicts[self_idx].s.origin);
    gi_multicast(&ctx.edicts[self_idx].s.origin, Multicast::Pvs as i32);

    g_free_edict(ctx, self_idx);
}

/// BecomeExplosion2 — turn entity into explosion2 temp entity and free it
pub fn become_explosion2(ctx: &mut GameContext, self_idx: usize) {
    gi_write_byte(SVC_TEMP_ENTITY);
    gi_write_byte(TE_EXPLOSION2);
    gi_write_position(&ctx.edicts[self_idx].s.origin);
    gi_multicast(&ctx.edicts[self_idx].s.origin, Multicast::Pvs as i32);

    g_free_edict(ctx, self_idx);
}

// ============================================================
// path_corner
// ============================================================

/// path_corner_touch — monster reaches path corner, navigate to next
pub fn path_corner_touch(
    ctx: &mut GameContext,
    self_idx: usize,
    other_idx: usize,
    _plane: Option<&[f32; 3]>,
    _surf: Option<usize>,
) {
    if ctx.edicts[other_idx].movetarget != self_idx as i32 {
        return;
    }

    if ctx.edicts[other_idx].enemy >= 0 {
        return;
    }

    if !ctx.edicts[self_idx].pathtarget.is_empty() {
        let savetarget = ctx.edicts[self_idx].target.clone();
        ctx.edicts[self_idx].target = ctx.edicts[self_idx].pathtarget.clone();
        ctx.maxclients = ctx.game.maxclients as f32;
        ctx.num_edicts = ctx.edicts.len() as i32;
        ctx.max_edicts = ctx.edicts.capacity() as i32;
        crate::g_utils::g_use_targets(ctx, self_idx, other_idx);
        ctx.edicts[self_idx].target = savetarget;
    }

    let next = if !ctx.edicts[self_idx].target.is_empty() {
        let target = ctx.edicts[self_idx].target.clone();
        crate::g_utils::g_pick_target(ctx, &target)
    } else {
        None
    };

    let mut next = next;

    // Handle teleport spawnflag
    if let Some(next_idx) = next {
        if (ctx.edicts[next_idx].spawnflags & 1) != 0 {
            let mut v = [0.0_f32; 3];
            vector_copy(&ctx.edicts[next_idx].s.origin, &mut v);
            v[2] += ctx.edicts[next_idx].mins[2];
            v[2] -= ctx.edicts[other_idx].mins[2];
            vector_copy(&v, &mut ctx.edicts[other_idx].s.origin);
            let next_target = ctx.edicts[next_idx].target.clone();
            next = crate::g_utils::g_pick_target(ctx, &next_target);
            ctx.edicts[other_idx].s.event = EV_OTHER_TELEPORT;
        }
    }

    let next_i32 = next.map(|n| n as i32).unwrap_or(-1);
    ctx.edicts[other_idx].goalentity = next_i32;
    ctx.edicts[other_idx].movetarget = next_i32;

    if ctx.edicts[self_idx].wait != 0.0 {
        ctx.edicts[other_idx].monsterinfo.pausetime = ctx.level.time + ctx.edicts[self_idx].wait;
        // other->monsterinfo.stand(other)
        crate::dispatch::call_stand(other_idx, &mut ctx.edicts, &mut ctx.level);
        return;
    }

    if ctx.edicts[other_idx].movetarget < 0 {
        ctx.edicts[other_idx].monsterinfo.pausetime = ctx.level.time + 100000000.0;
        crate::dispatch::call_stand(other_idx, &mut ctx.edicts, &mut ctx.level);
    } else {
        let goal_idx = ctx.edicts[other_idx].goalentity as usize;
        let mut v = [0.0_f32; 3];
        vector_subtract(
            &ctx.edicts[goal_idx].s.origin,
            &ctx.edicts[other_idx].s.origin,
            &mut v,
        );
        ctx.edicts[other_idx].ideal_yaw = vectoyaw(&v);
    }
}

/// SP_path_corner — spawn function for path_corner
pub fn sp_path_corner(ctx: &mut GameContext, self_idx: usize) {
    if ctx.edicts[self_idx].targetname.is_empty() {
        gi_dprintf(&format!(
            "path_corner with no targetname at {}\n",
            vtos(&ctx.edicts[self_idx].s.origin)
        ));
        g_free_edict(ctx, self_idx);
        return;
    }

    ctx.edicts[self_idx].solid = Solid::Trigger;
    ctx.edicts[self_idx].touch_fn = Some(TOUCH_PATH_CORNER);
    vector_set(&mut ctx.edicts[self_idx].mins, -8.0, -8.0, -8.0);
    vector_set(&mut ctx.edicts[self_idx].maxs, 8.0, 8.0, 8.0);
    ctx.edicts[self_idx].svflags |= SVF_NOCLIENT;
    gi::gi_linkentity(self_idx as i32);
}

// ============================================================
// point_combat
// ============================================================

/// point_combat_touch
pub fn point_combat_touch(
    ctx: &mut GameContext,
    self_idx: usize,
    other_idx: usize,
    _plane: Option<&[f32; 3]>,
    _surf: Option<usize>,
) {
    if ctx.edicts[other_idx].movetarget != self_idx as i32 {
        return;
    }

    if !ctx.edicts[self_idx].target.is_empty() {
        ctx.edicts[other_idx].target = ctx.edicts[self_idx].target.clone();
        let other_target = ctx.edicts[other_idx].target.clone();
        let picked = crate::g_utils::g_pick_target(ctx, &other_target);
        let picked_i32 = picked.map(|p| p as i32).unwrap_or(-1);
        ctx.edicts[other_idx].goalentity = picked_i32;
        ctx.edicts[other_idx].movetarget = picked_i32;

        if picked.is_none() {
            gi_dprintf(&format!(
                "{} at {} target {} does not exist\n",
                ctx.edicts[self_idx].classname,
                vtos(&ctx.edicts[self_idx].s.origin),
                ctx.edicts[self_idx].target
            ));
            ctx.edicts[other_idx].movetarget = self_idx as i32;
        }
        ctx.edicts[self_idx].target = String::new();
    } else if (ctx.edicts[self_idx].spawnflags & 1) != 0
        && !ctx.edicts[other_idx].flags.intersects(FL_SWIM | FL_FLY)
    {
        ctx.edicts[other_idx].monsterinfo.pausetime = ctx.level.time + 100000000.0;
        ctx.edicts[other_idx].monsterinfo.aiflags |= AI_STAND_GROUND;
        crate::dispatch::call_stand(other_idx, &mut ctx.edicts, &mut ctx.level);
    }

    if ctx.edicts[other_idx].movetarget == self_idx as i32 {
        ctx.edicts[other_idx].target = String::new();
        ctx.edicts[other_idx].movetarget = -1;
        ctx.edicts[other_idx].goalentity = ctx.edicts[other_idx].enemy;
        ctx.edicts[other_idx].monsterinfo.aiflags &= !AI_COMBAT_POINT;
    }

    if !ctx.edicts[self_idx].pathtarget.is_empty() {
        let savetarget = ctx.edicts[self_idx].target.clone();
        ctx.edicts[self_idx].target = ctx.edicts[self_idx].pathtarget.clone();

        let enemy_idx = ctx.edicts[other_idx].enemy;
        let oldenemy_idx = ctx.edicts[other_idx].oldenemy;
        let activator_from_idx = ctx.edicts[other_idx].activator;

        let activator;
        if enemy_idx >= 0 && ctx.edicts[enemy_idx as usize].client.is_some() {
            activator = enemy_idx as usize;
        } else if oldenemy_idx >= 0 && ctx.edicts[oldenemy_idx as usize].client.is_some() {
            activator = oldenemy_idx as usize;
        } else if activator_from_idx >= 0 && ctx.edicts[activator_from_idx as usize].client.is_some() {
            activator = activator_from_idx as usize;
        } else {
            activator = other_idx;
        }

        ctx.maxclients = ctx.game.maxclients as f32;
        ctx.num_edicts = ctx.edicts.len() as i32;
        ctx.max_edicts = ctx.edicts.capacity() as i32;
        crate::g_utils::g_use_targets(ctx, self_idx, activator);
        ctx.edicts[self_idx].target = savetarget;
    }
}

/// SP_point_combat — spawn function for point_combat
pub fn sp_point_combat(ctx: &mut GameContext, self_idx: usize) {
    if ctx.deathmatch != 0.0 {
        g_free_edict(ctx, self_idx);
        return;
    }
    ctx.edicts[self_idx].solid = Solid::Trigger;
    ctx.edicts[self_idx].touch_fn = Some(TOUCH_POINT_COMBAT);
    vector_set(&mut ctx.edicts[self_idx].mins, -8.0, -8.0, -16.0);
    vector_set(&mut ctx.edicts[self_idx].maxs, 8.0, 8.0, 16.0);
    ctx.edicts[self_idx].svflags = SVF_NOCLIENT;
    gi::gi_linkentity(self_idx as i32);
}

// ============================================================
// viewthing (debug)
// ============================================================

/// TH_viewthing — debug viewthing frame advance
pub fn th_viewthing(ctx: &mut GameContext, ent_idx: usize) {
    ctx.edicts[ent_idx].s.frame = (ctx.edicts[ent_idx].s.frame + 1) % 7;
    ctx.edicts[ent_idx].nextthink = ctx.level.time + FRAMETIME;
}

/// SP_viewthing — spawn debug viewthing
pub fn sp_viewthing(ctx: &mut GameContext, ent_idx: usize) {
    gi_dprintf("viewthing spawned\n");

    ctx.edicts[ent_idx].movetype = MoveType::None;
    ctx.edicts[ent_idx].solid = Solid::Bbox;
    ctx.edicts[ent_idx].s.renderfx = RF_FRAMELERP;
    vector_set(&mut ctx.edicts[ent_idx].mins, -16.0, -16.0, -24.0);
    vector_set(&mut ctx.edicts[ent_idx].maxs, 16.0, 16.0, 32.0);
    ctx.edicts[ent_idx].s.modelindex = gi::gi_modelindex("models/objects/banner/tris.md2");
    gi::gi_linkentity(ent_idx as i32);
    ctx.edicts[ent_idx].nextthink = ctx.level.time + 0.5;
    ctx.edicts[ent_idx].think_fn = Some(THINK_TH_VIEWTHING);
}

// ============================================================
// info_null / info_notnull
// ============================================================

/// SP_info_null — free immediately
pub fn sp_info_null(ctx: &mut GameContext, self_idx: usize) {
    g_free_edict(ctx, self_idx);
}

/// SP_info_notnull — set absmin/absmax to origin
pub fn sp_info_notnull(ctx: &mut GameContext, self_idx: usize) {
    let origin = ctx.edicts[self_idx].s.origin;
    vector_copy(&origin, &mut ctx.edicts[self_idx].absmin);
    vector_copy(&origin, &mut ctx.edicts[self_idx].absmax);
}

// ============================================================
// light
// ============================================================

/// light_use — toggle light on/off
pub fn light_use(ctx: &mut GameContext, self_idx: usize, _other_idx: usize, _activator_idx: usize) {
    if (ctx.edicts[self_idx].spawnflags & START_OFF) != 0 {
        gi::gi_configstring(CS_LIGHTS as i32 + ctx.edicts[self_idx].style, "m");
        ctx.edicts[self_idx].spawnflags &= !START_OFF;
    } else {
        gi::gi_configstring(CS_LIGHTS as i32 + ctx.edicts[self_idx].style, "a");
        ctx.edicts[self_idx].spawnflags |= START_OFF;
    }
}

/// SP_light — spawn a toggleable light
pub fn sp_light(ctx: &mut GameContext, self_idx: usize) {
    // no targeted lights in deathmatch, because they cause global messages
    if ctx.edicts[self_idx].targetname.is_empty() || ctx.deathmatch != 0.0 {
        g_free_edict(ctx, self_idx);
        return;
    }

    if ctx.edicts[self_idx].style >= 32 {
        ctx.edicts[self_idx].use_fn = Some(USE_LIGHT);
        if (ctx.edicts[self_idx].spawnflags & START_OFF) != 0 {
            gi::gi_configstring(CS_LIGHTS as i32 + ctx.edicts[self_idx].style, "a");
        } else {
            gi::gi_configstring(CS_LIGHTS as i32 + ctx.edicts[self_idx].style, "m");
        }
    }
}

// ============================================================
// func_wall
// ============================================================

/// func_wall_use — toggle wall visibility/solidity
pub fn func_wall_use(ctx: &mut GameContext, self_idx: usize, _other_idx: usize, _activator_idx: usize) {
    if ctx.edicts[self_idx].solid == Solid::Not {
        ctx.edicts[self_idx].solid = Solid::Bsp;
        ctx.edicts[self_idx].svflags &= !SVF_NOCLIENT;
        ctx.maxclients = ctx.game.maxclients as f32;
        crate::g_utils::killbox(ctx, self_idx);
    } else {
        ctx.edicts[self_idx].solid = Solid::Not;
        ctx.edicts[self_idx].svflags |= SVF_NOCLIENT;
    }
    gi::gi_linkentity(self_idx as i32);

    if (ctx.edicts[self_idx].spawnflags & 2) == 0 {
        ctx.edicts[self_idx].use_fn = None;
    }
}

/// SP_func_wall — spawn a func_wall
pub fn sp_func_wall(ctx: &mut GameContext, self_idx: usize) {
    ctx.edicts[self_idx].movetype = MoveType::Push;
    let model = ctx.edicts[self_idx].model.clone();
    gi::gi_setmodel(self_idx as i32, &model);

    if (ctx.edicts[self_idx].spawnflags & 8) != 0 {
        ctx.edicts[self_idx].s.effects |= EF_ANIM_ALL;
    }
    if (ctx.edicts[self_idx].spawnflags & 16) != 0 {
        ctx.edicts[self_idx].s.effects |= EF_ANIM_ALLFAST;
    }

    // just a wall
    if (ctx.edicts[self_idx].spawnflags & 7) == 0 {
        ctx.edicts[self_idx].solid = Solid::Bsp;
        gi::gi_linkentity(self_idx as i32);
        return;
    }

    // it must be TRIGGER_SPAWN
    if (ctx.edicts[self_idx].spawnflags & 1) == 0 {
        ctx.edicts[self_idx].spawnflags |= 1;
    }

    // yell if the spawnflags are odd
    if (ctx.edicts[self_idx].spawnflags & 4) != 0
        && (ctx.edicts[self_idx].spawnflags & 2) == 0 {
            gi_dprintf("func_wall START_ON without TOGGLE\n");
            ctx.edicts[self_idx].spawnflags |= 2;
        }

    ctx.edicts[self_idx].use_fn = Some(USE_FUNC_WALL);
    if (ctx.edicts[self_idx].spawnflags & 4) != 0 {
        ctx.edicts[self_idx].solid = Solid::Bsp;
    } else {
        ctx.edicts[self_idx].solid = Solid::Not;
        ctx.edicts[self_idx].svflags |= SVF_NOCLIENT;
    }
    gi::gi_linkentity(self_idx as i32);
}

// ============================================================
// func_object
// ============================================================

/// func_object_touch — squash things we land on
pub fn func_object_touch(
    ctx: &mut GameContext,
    self_idx: usize,
    other_idx: usize,
    plane: Option<&[f32; 3]>,
    _surf: Option<usize>,
) {
    // only squash things we fall on top of
    let Some(normal) = plane else { return };
    if normal[2] < 1.0 {
        return;
    }
    if ctx.edicts[other_idx].takedamage == DAMAGE_NO {
        return;
    }
    let origin = ctx.edicts[self_idx].s.origin;
    let dmg = ctx.edicts[self_idx].dmg;
    ctx.maxclients = ctx.game.maxclients as f32;
    crate::g_combat::ctx_t_damage(ctx, other_idx, self_idx, self_idx, &VEC3_ORIGIN, &origin, &VEC3_ORIGIN, dmg, 1, DamageFlags::empty(), MOD_CRUSH);
}

/// func_object_release — make object tossable
pub fn func_object_release(ctx: &mut GameContext, self_idx: usize) {
    ctx.edicts[self_idx].movetype = MoveType::Toss;
    ctx.edicts[self_idx].touch_fn = Some(TOUCH_FUNC_OBJECT);
}

/// func_object_use — spawn the func_object into the world
pub fn func_object_use(ctx: &mut GameContext, self_idx: usize, _other_idx: usize, _activator_idx: usize) {
    ctx.edicts[self_idx].solid = Solid::Bsp;
    ctx.edicts[self_idx].svflags &= !SVF_NOCLIENT;
    ctx.edicts[self_idx].use_fn = None;
    ctx.maxclients = ctx.game.maxclients as f32;
    crate::g_utils::killbox(ctx, self_idx);
    func_object_release(ctx, self_idx);
}

/// SP_func_object — spawn a func_object
pub fn sp_func_object(ctx: &mut GameContext, self_idx: usize) {
    let model = ctx.edicts[self_idx].model.clone();
    gi::gi_setmodel(self_idx as i32, &model);

    ctx.edicts[self_idx].mins[0] += 1.0;
    ctx.edicts[self_idx].mins[1] += 1.0;
    ctx.edicts[self_idx].mins[2] += 1.0;
    ctx.edicts[self_idx].maxs[0] -= 1.0;
    ctx.edicts[self_idx].maxs[1] -= 1.0;
    ctx.edicts[self_idx].maxs[2] -= 1.0;

    if ctx.edicts[self_idx].dmg == 0 {
        ctx.edicts[self_idx].dmg = 100;
    }

    if ctx.edicts[self_idx].spawnflags == 0 {
        ctx.edicts[self_idx].solid = Solid::Bsp;
        ctx.edicts[self_idx].movetype = MoveType::Push;
        ctx.edicts[self_idx].think_fn = Some(THINK_FUNC_OBJECT_RELEASE);
        ctx.edicts[self_idx].nextthink = ctx.level.time + 2.0 * FRAMETIME;
    } else {
        ctx.edicts[self_idx].solid = Solid::Not;
        ctx.edicts[self_idx].movetype = MoveType::Push;
        ctx.edicts[self_idx].use_fn = Some(USE_FUNC_OBJECT);
        ctx.edicts[self_idx].svflags |= SVF_NOCLIENT;
    }

    if (ctx.edicts[self_idx].spawnflags & 2) != 0 {
        ctx.edicts[self_idx].s.effects |= EF_ANIM_ALL;
    }
    if (ctx.edicts[self_idx].spawnflags & 4) != 0 {
        ctx.edicts[self_idx].s.effects |= EF_ANIM_ALLFAST;
    }

    ctx.edicts[self_idx].clipmask = MASK_MONSTERSOLID;

    gi::gi_linkentity(self_idx as i32);
}

// ============================================================
// func_explosive
// ============================================================

/// func_explosive_explode — blow up the explosive brush
pub fn func_explosive_explode(
    ctx: &mut GameContext,
    self_idx: usize,
    inflictor_idx: usize,
    attacker_idx: usize,
    _damage: i32,
    _point: &[f32; 3],
) {
    let mut origin = [0.0_f32; 3];
    let mut size = [0.0_f32; 3];

    // bmodel origins are (0 0 0), we need to adjust
    vector_scale(&ctx.edicts[self_idx].size, 0.5, &mut size);
    vector_add(&ctx.edicts[self_idx].absmin, &size, &mut origin);
    vector_copy(&origin, &mut ctx.edicts[self_idx].s.origin);

    ctx.edicts[self_idx].takedamage = DAMAGE_NO;

    let dmg = ctx.edicts[self_idx].dmg;
    if dmg != 0 {
        ctx.maxclients = ctx.game.maxclients as f32;
        crate::g_combat::ctx_t_radius_damage(ctx, self_idx, attacker_idx, dmg as f32, 0, (dmg + 40) as f32, MOD_EXPLOSIVE);
    }

    let inflictor_origin = ctx.edicts[inflictor_idx].s.origin;
    let self_origin = ctx.edicts[self_idx].s.origin;
    vector_subtract(&self_origin, &inflictor_origin, &mut ctx.edicts[self_idx].velocity);
    vector_normalize(&mut ctx.edicts[self_idx].velocity);
    let vel = ctx.edicts[self_idx].velocity;
    vector_scale(&vel, 150.0, &mut ctx.edicts[self_idx].velocity);

    // start chunks towards the center
    vector_scale(&size.clone(), 0.5, &mut size);

    let mut mass = ctx.edicts[self_idx].mass;
    if mass == 0 {
        mass = 75;
    }

    // big chunks
    if mass >= 100 {
        let mut count = mass / 100;
        if count > 8 {
            count = 8;
        }
        for _ in 0..count {
            let mut chunkorigin = [0.0_f32; 3];
            chunkorigin[0] = origin[0] + crandom() * size[0];
            chunkorigin[1] = origin[1] + crandom() * size[1];
            chunkorigin[2] = origin[2] + crandom() * size[2];
            throw_debris(ctx, self_idx, "models/objects/debris1/tris.md2", 1.0, &chunkorigin);
        }
    }

    // small chunks
    let mut count = mass / 25;
    if count > 16 {
        count = 16;
    }
    for _ in 0..count {
        let mut chunkorigin = [0.0_f32; 3];
        chunkorigin[0] = origin[0] + crandom() * size[0];
        chunkorigin[1] = origin[1] + crandom() * size[1];
        chunkorigin[2] = origin[2] + crandom() * size[2];
        throw_debris(ctx, self_idx, "models/objects/debris2/tris.md2", 2.0, &chunkorigin);
    }

    ctx.maxclients = ctx.game.maxclients as f32;
    ctx.num_edicts = ctx.edicts.len() as i32;
    ctx.max_edicts = ctx.edicts.capacity() as i32;
    crate::g_utils::g_use_targets(ctx, self_idx, attacker_idx);

    if ctx.edicts[self_idx].dmg != 0 {
        become_explosion1(ctx, self_idx);
    } else {
        g_free_edict(ctx, self_idx);
    }
}

/// func_explosive_use — trigger the explosion via use
pub fn func_explosive_use(ctx: &mut GameContext, self_idx: usize, other_idx: usize, _activator_idx: usize) {
    let health = ctx.edicts[self_idx].health;
    func_explosive_explode(ctx, self_idx, self_idx, other_idx, health, &VEC3_ORIGIN);
}

/// func_explosive_spawn — spawn trigger for explosive
pub fn func_explosive_spawn(ctx: &mut GameContext, self_idx: usize, _other_idx: usize, _activator_idx: usize) {
    ctx.edicts[self_idx].solid = Solid::Bsp;
    ctx.edicts[self_idx].svflags &= !SVF_NOCLIENT;
    ctx.edicts[self_idx].use_fn = None;
    ctx.maxclients = ctx.game.maxclients as f32;
    crate::g_utils::killbox(ctx, self_idx);
    gi::gi_linkentity(self_idx as i32);
}

/// SP_func_explosive — spawn function for func_explosive
pub fn sp_func_explosive(ctx: &mut GameContext, self_idx: usize) {
    if ctx.deathmatch != 0.0 {
        // auto-remove for deathmatch
        g_free_edict(ctx, self_idx);
        return;
    }

    ctx.edicts[self_idx].movetype = MoveType::Push;

    gi::gi_modelindex("models/objects/debris1/tris.md2");
    gi::gi_modelindex("models/objects/debris2/tris.md2");

    let model = ctx.edicts[self_idx].model.clone();
    gi::gi_setmodel(self_idx as i32, &model);

    if (ctx.edicts[self_idx].spawnflags & 1) != 0 {
        ctx.edicts[self_idx].svflags |= SVF_NOCLIENT;
        ctx.edicts[self_idx].solid = Solid::Not;
        ctx.edicts[self_idx].use_fn = Some(USE_FUNC_EXPLOSIVE_SPAWN);
    } else {
        ctx.edicts[self_idx].solid = Solid::Bsp;
        if !ctx.edicts[self_idx].targetname.is_empty() {
            ctx.edicts[self_idx].use_fn = Some(USE_FUNC_EXPLOSIVE);
        }
    }

    if (ctx.edicts[self_idx].spawnflags & 2) != 0 {
        ctx.edicts[self_idx].s.effects |= EF_ANIM_ALL;
    }
    if (ctx.edicts[self_idx].spawnflags & 4) != 0 {
        ctx.edicts[self_idx].s.effects |= EF_ANIM_ALLFAST;
    }

    // if use != func_explosive_use, set up for shootable
    if ctx.edicts[self_idx].use_fn != Some(USE_FUNC_EXPLOSIVE) {
        if ctx.edicts[self_idx].health == 0 {
            ctx.edicts[self_idx].health = 100;
        }
        ctx.edicts[self_idx].die_fn = Some(DIE_FUNC_EXPLOSIVE_EXPLODE);
        ctx.edicts[self_idx].takedamage = DAMAGE_YES;
    }

    gi::gi_linkentity(self_idx as i32);
}

// ============================================================
// misc_explobox (barrel)
// ============================================================

/// barrel_touch — barrel pushed by other entities
pub fn barrel_touch(
    ctx: &mut GameContext,
    self_idx: usize,
    other_idx: usize,
    _plane: Option<&[f32; 3]>,
    _surf: Option<usize>,
) {
    if ctx.edicts[other_idx].groundentity < 0
        || ctx.edicts[other_idx].groundentity == self_idx as i32
    {
        return;
    }

    let ratio = ctx.edicts[other_idx].mass as f32 / ctx.edicts[self_idx].mass as f32;
    let mut v = [0.0_f32; 3];
    vector_subtract(&ctx.edicts[self_idx].s.origin, &ctx.edicts[other_idx].s.origin, &mut v);
    crate::entity_adapters::m_walkmove(&mut ctx.edicts, &mut ctx.clients, self_idx as i32, vectoyaw(&v), 20.0 * ratio * FRAMETIME);
}

/// barrel_explode — barrel blows up
pub fn barrel_explode(ctx: &mut GameContext, self_idx: usize) {
    let dmg = ctx.edicts[self_idx].dmg;
    let activator = ctx.edicts[self_idx].activator;
    ctx.maxclients = ctx.game.maxclients as f32;
    crate::g_combat::ctx_t_radius_damage(ctx, self_idx, activator as usize, dmg as f32, 0, (dmg + 40) as f32, MOD_BARREL);

    let save = ctx.edicts[self_idx].s.origin;
    let size = ctx.edicts[self_idx].size;
    let absmin = ctx.edicts[self_idx].absmin;
    vector_ma(&absmin, 0.5, &size, &mut ctx.edicts[self_idx].s.origin);

    // a few big chunks
    let spd = 1.5 * dmg as f32 / 200.0;
    let origin = ctx.edicts[self_idx].s.origin;
    let ent_size = ctx.edicts[self_idx].size;

    let mut org = [0.0_f32; 3];
    org[0] = origin[0] + crandom() * ent_size[0];
    org[1] = origin[1] + crandom() * ent_size[1];
    org[2] = origin[2] + crandom() * ent_size[2];
    throw_debris(ctx, self_idx, "models/objects/debris1/tris.md2", spd, &org);
    let origin = ctx.edicts[self_idx].s.origin;
    let ent_size = ctx.edicts[self_idx].size;
    org[0] = origin[0] + crandom() * ent_size[0];
    org[1] = origin[1] + crandom() * ent_size[1];
    org[2] = origin[2] + crandom() * ent_size[2];
    throw_debris(ctx, self_idx, "models/objects/debris1/tris.md2", spd, &org);

    // bottom corners
    let spd = 1.75 * dmg as f32 / 200.0;
    let absmin = ctx.edicts[self_idx].absmin;
    vector_copy(&absmin, &mut org);
    throw_debris(ctx, self_idx, "models/objects/debris3/tris.md2", spd, &org);
    vector_copy(&absmin, &mut org);
    org[0] += ent_size[0];
    throw_debris(ctx, self_idx, "models/objects/debris3/tris.md2", spd, &org);
    vector_copy(&absmin, &mut org);
    org[1] += ent_size[1];
    throw_debris(ctx, self_idx, "models/objects/debris3/tris.md2", spd, &org);
    vector_copy(&absmin, &mut org);
    org[0] += ent_size[0];
    org[1] += ent_size[1];
    throw_debris(ctx, self_idx, "models/objects/debris3/tris.md2", spd, &org);

    // a bunch of little chunks
    let spd = 2.0 * dmg as f32 / 200.0;
    let origin = ctx.edicts[self_idx].s.origin;
    let ent_size = ctx.edicts[self_idx].size;
    for _ in 0..8 {
        org[0] = origin[0] + crandom() * ent_size[0];
        org[1] = origin[1] + crandom() * ent_size[1];
        org[2] = origin[2] + crandom() * ent_size[2];
        throw_debris(ctx, self_idx, "models/objects/debris2/tris.md2", spd, &org);
    }

    vector_copy(&save, &mut ctx.edicts[self_idx].s.origin);
    if ctx.edicts[self_idx].groundentity >= 0 {
        become_explosion2(ctx, self_idx);
    } else {
        become_explosion1(ctx, self_idx);
    }
}

/// barrel_delay — die callback, schedule explosion
pub fn barrel_delay(ctx: &mut GameContext, self_idx: usize, _inflictor: usize, attacker_idx: usize, _damage: i32, _point: &[f32; 3]) {
    ctx.edicts[self_idx].takedamage = DAMAGE_NO;
    ctx.edicts[self_idx].nextthink = ctx.level.time + 2.0 * FRAMETIME;
    ctx.edicts[self_idx].think_fn = Some(THINK_BARREL_EXPLODE);
    ctx.edicts[self_idx].activator = attacker_idx as i32;
}

/// SP_misc_explobox — spawn an exploding barrel
pub fn sp_misc_explobox(ctx: &mut GameContext, self_idx: usize) {
    if ctx.deathmatch != 0.0 {
        g_free_edict(ctx, self_idx);
        return;
    }

    gi::gi_modelindex("models/objects/debris1/tris.md2");
    gi::gi_modelindex("models/objects/debris2/tris.md2");
    gi::gi_modelindex("models/objects/debris3/tris.md2");

    ctx.edicts[self_idx].solid = Solid::Bbox;
    ctx.edicts[self_idx].movetype = MoveType::Step;

    ctx.edicts[self_idx].model = "models/objects/barrels/tris.md2".to_string();
    let model = ctx.edicts[self_idx].model.clone();
    ctx.edicts[self_idx].s.modelindex = gi::gi_modelindex(&model);
    vector_set(&mut ctx.edicts[self_idx].mins, -16.0, -16.0, 0.0);
    vector_set(&mut ctx.edicts[self_idx].maxs, 16.0, 16.0, 40.0);

    if ctx.edicts[self_idx].mass == 0 {
        ctx.edicts[self_idx].mass = 400;
    }
    if ctx.edicts[self_idx].health == 0 {
        ctx.edicts[self_idx].health = 10;
    }
    if ctx.edicts[self_idx].dmg == 0 {
        ctx.edicts[self_idx].dmg = 150;
    }

    ctx.edicts[self_idx].die_fn = Some(DIE_BARREL_DELAY);
    ctx.edicts[self_idx].takedamage = DAMAGE_YES;
    ctx.edicts[self_idx].monsterinfo.aiflags = AI_NOSTEP;

    ctx.edicts[self_idx].touch_fn = Some(TOUCH_BARREL);

    ctx.edicts[self_idx].think_fn = Some(THINK_M_DROPTOFLOOR);
    ctx.edicts[self_idx].nextthink = ctx.level.time + 2.0 * FRAMETIME;

    gi::gi_linkentity(self_idx as i32);
}

// ============================================================
// misc_blackhole
// ============================================================

/// misc_blackhole_use — free the blackhole
pub fn misc_blackhole_use(ctx: &mut GameContext, ent_idx: usize, _other_idx: usize, _activator_idx: usize) {
    g_free_edict(ctx, ent_idx);
}

/// misc_blackhole_think — animate blackhole frames
pub fn misc_blackhole_think(ctx: &mut GameContext, self_idx: usize) {
    ctx.edicts[self_idx].s.frame += 1;
    if ctx.edicts[self_idx].s.frame < 19 {
        ctx.edicts[self_idx].nextthink = ctx.level.time + FRAMETIME;
    } else {
        ctx.edicts[self_idx].s.frame = 0;
        ctx.edicts[self_idx].nextthink = ctx.level.time + FRAMETIME;
    }
}

/// SP_misc_blackhole — spawn a blackhole
pub fn sp_misc_blackhole(ctx: &mut GameContext, ent_idx: usize) {
    ctx.edicts[ent_idx].movetype = MoveType::None;
    ctx.edicts[ent_idx].solid = Solid::Not;
    vector_set(&mut ctx.edicts[ent_idx].mins, -64.0, -64.0, 0.0);
    vector_set(&mut ctx.edicts[ent_idx].maxs, 64.0, 64.0, 8.0);
    ctx.edicts[ent_idx].s.modelindex = gi::gi_modelindex("models/objects/black/tris.md2");
    ctx.edicts[ent_idx].s.renderfx = RF_TRANSLUCENT;
    ctx.edicts[ent_idx].use_fn = Some(USE_MISC_BLACKHOLE);
    ctx.edicts[ent_idx].think_fn = Some(THINK_MISC_BLACKHOLE);
    ctx.edicts[ent_idx].nextthink = ctx.level.time + 2.0 * FRAMETIME;
    gi::gi_linkentity(ent_idx as i32);
}

// ============================================================
// misc_eastertank
// ============================================================

/// misc_eastertank_think — animate easter tank frames
pub fn misc_eastertank_think(ctx: &mut GameContext, self_idx: usize) {
    ctx.edicts[self_idx].s.frame += 1;
    if ctx.edicts[self_idx].s.frame < 293 {
        ctx.edicts[self_idx].nextthink = ctx.level.time + FRAMETIME;
    } else {
        ctx.edicts[self_idx].s.frame = 254;
        ctx.edicts[self_idx].nextthink = ctx.level.time + FRAMETIME;
    }
}

/// SP_misc_eastertank — spawn easter tank
pub fn sp_misc_eastertank(ctx: &mut GameContext, ent_idx: usize) {
    ctx.edicts[ent_idx].movetype = MoveType::None;
    ctx.edicts[ent_idx].solid = Solid::Bbox;
    vector_set(&mut ctx.edicts[ent_idx].mins, -32.0, -32.0, -16.0);
    vector_set(&mut ctx.edicts[ent_idx].maxs, 32.0, 32.0, 32.0);
    ctx.edicts[ent_idx].s.modelindex = gi::gi_modelindex("models/monsters/tank/tris.md2");
    ctx.edicts[ent_idx].s.frame = 254;
    ctx.edicts[ent_idx].think_fn = Some(THINK_MISC_EASTERTANK);
    ctx.edicts[ent_idx].nextthink = ctx.level.time + 2.0 * FRAMETIME;
    gi::gi_linkentity(ent_idx as i32);
}

// ============================================================
// misc_easterchick
// ============================================================

/// misc_easterchick_think — animate easter chick frames
pub fn misc_easterchick_think(ctx: &mut GameContext, self_idx: usize) {
    ctx.edicts[self_idx].s.frame += 1;
    if ctx.edicts[self_idx].s.frame < 247 {
        ctx.edicts[self_idx].nextthink = ctx.level.time + FRAMETIME;
    } else {
        ctx.edicts[self_idx].s.frame = 208;
        ctx.edicts[self_idx].nextthink = ctx.level.time + FRAMETIME;
    }
}

/// SP_misc_easterchick — spawn easter chick
pub fn sp_misc_easterchick(ctx: &mut GameContext, ent_idx: usize) {
    ctx.edicts[ent_idx].movetype = MoveType::None;
    ctx.edicts[ent_idx].solid = Solid::Bbox;
    vector_set(&mut ctx.edicts[ent_idx].mins, -32.0, -32.0, 0.0);
    vector_set(&mut ctx.edicts[ent_idx].maxs, 32.0, 32.0, 32.0);
    ctx.edicts[ent_idx].s.modelindex = gi::gi_modelindex("models/monsters/bitch/tris.md2");
    ctx.edicts[ent_idx].s.frame = 208;
    ctx.edicts[ent_idx].think_fn = Some(THINK_MISC_EASTERCHICK);
    ctx.edicts[ent_idx].nextthink = ctx.level.time + 2.0 * FRAMETIME;
    gi::gi_linkentity(ent_idx as i32);
}

// ============================================================
// misc_easterchick2
// ============================================================

/// misc_easterchick2_think — animate easter chick2 frames
pub fn misc_easterchick2_think(ctx: &mut GameContext, self_idx: usize) {
    ctx.edicts[self_idx].s.frame += 1;
    if ctx.edicts[self_idx].s.frame < 287 {
        ctx.edicts[self_idx].nextthink = ctx.level.time + FRAMETIME;
    } else {
        ctx.edicts[self_idx].s.frame = 248;
        ctx.edicts[self_idx].nextthink = ctx.level.time + FRAMETIME;
    }
}

/// SP_misc_easterchick2 — spawn easter chick2
pub fn sp_misc_easterchick2(ctx: &mut GameContext, ent_idx: usize) {
    ctx.edicts[ent_idx].movetype = MoveType::None;
    ctx.edicts[ent_idx].solid = Solid::Bbox;
    vector_set(&mut ctx.edicts[ent_idx].mins, -32.0, -32.0, 0.0);
    vector_set(&mut ctx.edicts[ent_idx].maxs, 32.0, 32.0, 32.0);
    ctx.edicts[ent_idx].s.modelindex = gi::gi_modelindex("models/monsters/bitch/tris.md2");
    ctx.edicts[ent_idx].s.frame = 248;
    ctx.edicts[ent_idx].think_fn = Some(THINK_MISC_EASTERCHICK2);
    ctx.edicts[ent_idx].nextthink = ctx.level.time + 2.0 * FRAMETIME;
    gi::gi_linkentity(ent_idx as i32);
}

// ============================================================
// monster_commander_body
// ============================================================

/// commander_body_think — animate commander body falling
pub fn commander_body_think(ctx: &mut GameContext, self_idx: usize) {
    ctx.edicts[self_idx].s.frame += 1;
    if ctx.edicts[self_idx].s.frame < 24 {
        ctx.edicts[self_idx].nextthink = ctx.level.time + FRAMETIME;
    } else {
        ctx.edicts[self_idx].nextthink = 0.0;
    }

    if ctx.edicts[self_idx].s.frame == 22 {
        let snd = gi::gi_soundindex("tank/thud.wav");
        gi::gi_sound(self_idx as i32, CHAN_BODY, snd, 1.0, ATTN_NORM, 0.0);
    }
}

/// commander_body_use — trigger commander body animation
pub fn commander_body_use(ctx: &mut GameContext, self_idx: usize, _other_idx: usize, _activator_idx: usize) {
    ctx.edicts[self_idx].think_fn = Some(THINK_COMMANDER_BODY);
    ctx.edicts[self_idx].nextthink = ctx.level.time + FRAMETIME;
    let snd = gi::gi_soundindex("tank/pain.wav");
    gi::gi_sound(self_idx as i32, CHAN_BODY, snd, 1.0, ATTN_NORM, 0.0);
}

/// commander_body_drop — make body tossable
pub fn commander_body_drop(ctx: &mut GameContext, self_idx: usize) {
    ctx.edicts[self_idx].movetype = MoveType::Toss;
    ctx.edicts[self_idx].s.origin[2] += 2.0;
}

/// SP_monster_commander_body — spawn commander body
pub fn sp_monster_commander_body(ctx: &mut GameContext, self_idx: usize) {
    ctx.edicts[self_idx].movetype = MoveType::None;
    ctx.edicts[self_idx].solid = Solid::Bbox;
    ctx.edicts[self_idx].model = "models/monsters/commandr/tris.md2".to_string();
    let model = ctx.edicts[self_idx].model.clone();
    ctx.edicts[self_idx].s.modelindex = gi::gi_modelindex(&model);
    vector_set(&mut ctx.edicts[self_idx].mins, -32.0, -32.0, 0.0);
    vector_set(&mut ctx.edicts[self_idx].maxs, 32.0, 32.0, 48.0);
    ctx.edicts[self_idx].use_fn = Some(USE_COMMANDER_BODY);
    ctx.edicts[self_idx].takedamage = DAMAGE_YES;
    ctx.edicts[self_idx].flags = FL_GODMODE;
    ctx.edicts[self_idx].s.renderfx |= RF_FRAMELERP;
    gi::gi_linkentity(self_idx as i32);

    gi::gi_soundindex("tank/thud.wav");
    gi::gi_soundindex("tank/pain.wav");

    ctx.edicts[self_idx].think_fn = Some(THINK_COMMANDER_BODY_DROP);
    ctx.edicts[self_idx].nextthink = ctx.level.time + 5.0 * FRAMETIME;
}

// ============================================================
// misc_banner
// ============================================================

/// misc_banner_think — animate banner frames
pub fn misc_banner_think(ctx: &mut GameContext, ent_idx: usize) {
    ctx.edicts[ent_idx].s.frame = (ctx.edicts[ent_idx].s.frame + 1) % 16;
    ctx.edicts[ent_idx].nextthink = ctx.level.time + FRAMETIME;
}

/// SP_misc_banner — spawn a banner
pub fn sp_misc_banner(ctx: &mut GameContext, ent_idx: usize) {
    ctx.edicts[ent_idx].movetype = MoveType::None;
    ctx.edicts[ent_idx].solid = Solid::Not;
    ctx.edicts[ent_idx].s.modelindex = gi::gi_modelindex("models/objects/banner/tris.md2");
    ctx.edicts[ent_idx].s.frame = rand_i32() % 16;
    gi::gi_linkentity(ent_idx as i32);

    ctx.edicts[ent_idx].think_fn = Some(THINK_MISC_BANNER);
    ctx.edicts[ent_idx].nextthink = ctx.level.time + FRAMETIME;
}

// ============================================================
// misc_deadsoldier
// ============================================================

/// misc_deadsoldier_die — gib the dead soldier if enough damage
pub fn misc_deadsoldier_die(ctx: &mut GameContext, self_idx: usize, _inflictor: usize, _attacker: usize, damage: i32, _point: &[f32; 3]) {
    if ctx.edicts[self_idx].health > -80 {
        return;
    }

    let snd = gi::gi_soundindex("misc/udeath.wav");
    gi::gi_sound(self_idx as i32, CHAN_BODY, snd, 1.0, ATTN_NORM, 0.0);
    for _ in 0..4 {
        throw_gib(ctx, self_idx, "models/objects/gibs/sm_meat/tris.md2", damage, GIB_ORGANIC);
    }
    throw_head(ctx, self_idx, "models/objects/gibs/head2/tris.md2", damage, GIB_ORGANIC);
}

/// SP_misc_deadsoldier — spawn a dead soldier prop
pub fn sp_misc_deadsoldier(ctx: &mut GameContext, ent_idx: usize) {
    if ctx.deathmatch != 0.0 {
        g_free_edict(ctx, ent_idx);
        return;
    }

    ctx.edicts[ent_idx].movetype = MoveType::None;
    ctx.edicts[ent_idx].solid = Solid::Bbox;
    ctx.edicts[ent_idx].s.modelindex = gi::gi_modelindex("models/deadbods/dude/tris.md2");

    // Defaults to frame 0
    if (ctx.edicts[ent_idx].spawnflags & 2) != 0 {
        ctx.edicts[ent_idx].s.frame = 1;
    } else if (ctx.edicts[ent_idx].spawnflags & 4) != 0 {
        ctx.edicts[ent_idx].s.frame = 2;
    } else if (ctx.edicts[ent_idx].spawnflags & 8) != 0 {
        ctx.edicts[ent_idx].s.frame = 3;
    } else if (ctx.edicts[ent_idx].spawnflags & 16) != 0 {
        ctx.edicts[ent_idx].s.frame = 4;
    } else if (ctx.edicts[ent_idx].spawnflags & 32) != 0 {
        ctx.edicts[ent_idx].s.frame = 5;
    } else {
        ctx.edicts[ent_idx].s.frame = 0;
    }

    vector_set(&mut ctx.edicts[ent_idx].mins, -16.0, -16.0, 0.0);
    vector_set(&mut ctx.edicts[ent_idx].maxs, 16.0, 16.0, 16.0);
    ctx.edicts[ent_idx].deadflag = DEAD_DEAD;
    ctx.edicts[ent_idx].takedamage = DAMAGE_YES;
    ctx.edicts[ent_idx].svflags |= SVF_MONSTER | SVF_DEADMONSTER;
    ctx.edicts[ent_idx].die_fn = Some(DIE_MISC_DEADSOLDIER);
    ctx.edicts[ent_idx].monsterinfo.aiflags |= AI_GOOD_GUY;

    gi::gi_linkentity(ent_idx as i32);
}

// ============================================================
// misc_viper
// ============================================================

/// misc_viper_use — make viper visible and start train use
pub fn misc_viper_use(ctx: &mut GameContext, self_idx: usize, other_idx: usize, activator_idx: usize) {
    ctx.edicts[self_idx].svflags &= !SVF_NOCLIENT;
    ctx.edicts[self_idx].use_fn = Some(USE_TRAIN);
    // train_use(self, other, activator)
    crate::dispatch::call_use(self_idx, other_idx, activator_idx, &mut ctx.edicts, &mut ctx.level);
}

/// SP_misc_viper — spawn a viper flyby entity
pub fn sp_misc_viper(ctx: &mut GameContext, ent_idx: usize) {
    if ctx.edicts[ent_idx].target.is_empty() {
        gi_dprintf(&format!(
            "misc_viper without a target at {}\n",
            vtos(&ctx.edicts[ent_idx].absmin)
        ));
        g_free_edict(ctx, ent_idx);
        return;
    }

    if ctx.edicts[ent_idx].speed == 0.0 {
        ctx.edicts[ent_idx].speed = 300.0;
    }

    ctx.edicts[ent_idx].movetype = MoveType::Push;
    ctx.edicts[ent_idx].solid = Solid::Not;
    ctx.edicts[ent_idx].s.modelindex = gi::gi_modelindex("models/ships/viper/tris.md2");
    vector_set(&mut ctx.edicts[ent_idx].mins, -16.0, -16.0, 0.0);
    vector_set(&mut ctx.edicts[ent_idx].maxs, 16.0, 16.0, 32.0);

    ctx.edicts[ent_idx].think_fn = Some(THINK_FUNC_TRAIN_FIND);
    ctx.edicts[ent_idx].nextthink = ctx.level.time + FRAMETIME;
    ctx.edicts[ent_idx].use_fn = Some(USE_MISC_VIPER);
    ctx.edicts[ent_idx].svflags |= SVF_NOCLIENT;
    let speed = ctx.edicts[ent_idx].speed;
    ctx.edicts[ent_idx].moveinfo.accel = speed;
    ctx.edicts[ent_idx].moveinfo.decel = speed;
    ctx.edicts[ent_idx].moveinfo.speed = speed;

    gi::gi_linkentity(ent_idx as i32);
}

// ============================================================
// misc_bigviper
// ============================================================

/// SP_misc_bigviper — spawn a large stationary viper
pub fn sp_misc_bigviper(ctx: &mut GameContext, ent_idx: usize) {
    ctx.edicts[ent_idx].movetype = MoveType::None;
    ctx.edicts[ent_idx].solid = Solid::Bbox;
    vector_set(&mut ctx.edicts[ent_idx].mins, -176.0, -120.0, -24.0);
    vector_set(&mut ctx.edicts[ent_idx].maxs, 176.0, 120.0, 72.0);
    ctx.edicts[ent_idx].s.modelindex = gi::gi_modelindex("models/ships/bigviper/tris.md2");
    gi::gi_linkentity(ent_idx as i32);
}

// ============================================================
// misc_viper_bomb
// ============================================================

/// misc_viper_bomb_touch — bomb hits something, explode
pub fn misc_viper_bomb_touch(
    ctx: &mut GameContext,
    self_idx: usize,
    _other_idx: usize,
    _plane: Option<&[f32; 3]>,
    _surf: Option<usize>,
) {
    let activator = ctx.edicts[self_idx].activator as usize;
    ctx.maxclients = ctx.game.maxclients as f32;
    ctx.num_edicts = ctx.edicts.len() as i32;
    ctx.max_edicts = ctx.edicts.capacity() as i32;
    crate::g_utils::g_use_targets(ctx, self_idx, activator);

    ctx.edicts[self_idx].s.origin[2] = ctx.edicts[self_idx].absmin[2] + 1.0;
    let dmg = ctx.edicts[self_idx].dmg;
    ctx.maxclients = ctx.game.maxclients as f32;
    crate::g_combat::ctx_t_radius_damage(ctx, self_idx, self_idx, dmg as f32, 0, (dmg + 40) as f32, MOD_BOMB);
    become_explosion2(ctx, self_idx);
}

/// misc_viper_bomb_prethink — adjust bomb trajectory
pub fn misc_viper_bomb_prethink(ctx: &mut GameContext, self_idx: usize) {
    ctx.edicts[self_idx].groundentity = -1;

    let mut diff = ctx.edicts[self_idx].timestamp - ctx.level.time;
    if diff < -1.0 {
        diff = -1.0;
    }

    let mut v = [0.0_f32; 3];
    let dir = ctx.edicts[self_idx].moveinfo.dir;
    vector_scale(&dir, 1.0 + diff, &mut v);
    v[2] = diff;

    let roll = ctx.edicts[self_idx].s.angles[2];
    vectoangles(&v, &mut ctx.edicts[self_idx].s.angles);
    ctx.edicts[self_idx].s.angles[2] = roll + 10.0;
}

/// misc_viper_bomb_use — activate the bomb
pub fn misc_viper_bomb_use(ctx: &mut GameContext, self_idx: usize, _other_idx: usize, activator_idx: usize) {
    ctx.edicts[self_idx].solid = Solid::Bbox;
    ctx.edicts[self_idx].svflags &= !SVF_NOCLIENT;
    ctx.edicts[self_idx].s.effects |= EF_ROCKET;
    ctx.edicts[self_idx].use_fn = None;
    ctx.edicts[self_idx].movetype = MoveType::Toss;
    ctx.edicts[self_idx].prethink_fn = Some(THINK_MISC_VIPER_BOMB_PRETHINK);
    ctx.edicts[self_idx].touch_fn = Some(TOUCH_MISC_VIPER_BOMB);
    ctx.edicts[self_idx].activator = activator_idx as i32;

    let viper_idx = crate::g_utils::g_find(ctx, 0, "classname", "misc_viper");
    if let Some(viper) = viper_idx {
        let dir = ctx.edicts[viper].moveinfo.dir;
        let speed = ctx.edicts[viper].moveinfo.speed;
        vector_scale(&dir, speed, &mut ctx.edicts[self_idx].velocity);

        ctx.edicts[self_idx].timestamp = ctx.level.time;
        vector_copy(&dir, &mut ctx.edicts[self_idx].moveinfo.dir);
    }
}

/// SP_misc_viper_bomb — spawn a viper bomb
pub fn sp_misc_viper_bomb(ctx: &mut GameContext, self_idx: usize) {
    ctx.edicts[self_idx].movetype = MoveType::None;
    ctx.edicts[self_idx].solid = Solid::Not;
    vector_set(&mut ctx.edicts[self_idx].mins, -8.0, -8.0, -8.0);
    vector_set(&mut ctx.edicts[self_idx].maxs, 8.0, 8.0, 8.0);

    ctx.edicts[self_idx].s.modelindex = gi::gi_modelindex("models/objects/bomb/tris.md2");

    if ctx.edicts[self_idx].dmg == 0 {
        ctx.edicts[self_idx].dmg = 1000;
    }

    ctx.edicts[self_idx].use_fn = Some(USE_MISC_VIPER_BOMB);
    ctx.edicts[self_idx].svflags |= SVF_NOCLIENT;

    gi::gi_linkentity(self_idx as i32);
}

// ============================================================
// misc_strogg_ship
// ============================================================

/// misc_strogg_ship_use — make ship visible and start train
pub fn misc_strogg_ship_use(ctx: &mut GameContext, self_idx: usize, other_idx: usize, activator_idx: usize) {
    ctx.edicts[self_idx].svflags &= !SVF_NOCLIENT;
    ctx.edicts[self_idx].use_fn = Some(USE_TRAIN);
    // train_use(self, other, activator)
    crate::dispatch::call_use(self_idx, other_idx, activator_idx, &mut ctx.edicts, &mut ctx.level);
}

/// SP_misc_strogg_ship — spawn a strogg ship flyby
pub fn sp_misc_strogg_ship(ctx: &mut GameContext, ent_idx: usize) {
    if ctx.edicts[ent_idx].target.is_empty() {
        gi_dprintf(&format!(
            "{} without a target at {}\n",
            ctx.edicts[ent_idx].classname,
            vtos(&ctx.edicts[ent_idx].absmin)
        ));
        g_free_edict(ctx, ent_idx);
        return;
    }

    if ctx.edicts[ent_idx].speed == 0.0 {
        ctx.edicts[ent_idx].speed = 300.0;
    }

    ctx.edicts[ent_idx].movetype = MoveType::Push;
    ctx.edicts[ent_idx].solid = Solid::Not;
    ctx.edicts[ent_idx].s.modelindex = gi::gi_modelindex("models/ships/strogg1/tris.md2");
    vector_set(&mut ctx.edicts[ent_idx].mins, -16.0, -16.0, 0.0);
    vector_set(&mut ctx.edicts[ent_idx].maxs, 16.0, 16.0, 32.0);

    ctx.edicts[ent_idx].think_fn = Some(THINK_FUNC_TRAIN_FIND);
    ctx.edicts[ent_idx].nextthink = ctx.level.time + FRAMETIME;
    ctx.edicts[ent_idx].use_fn = Some(USE_MISC_STROGG_SHIP);
    ctx.edicts[ent_idx].svflags |= SVF_NOCLIENT;
    let speed = ctx.edicts[ent_idx].speed;
    ctx.edicts[ent_idx].moveinfo.accel = speed;
    ctx.edicts[ent_idx].moveinfo.decel = speed;
    ctx.edicts[ent_idx].moveinfo.speed = speed;

    gi::gi_linkentity(ent_idx as i32);
}

// ============================================================
// misc_satellite_dish
// ============================================================

/// misc_satellite_dish_think — animate satellite dish
pub fn misc_satellite_dish_think(ctx: &mut GameContext, self_idx: usize) {
    ctx.edicts[self_idx].s.frame += 1;
    if ctx.edicts[self_idx].s.frame < 38 {
        ctx.edicts[self_idx].nextthink = ctx.level.time + FRAMETIME;
    }
}

/// misc_satellite_dish_use — start dish animation
pub fn misc_satellite_dish_use(ctx: &mut GameContext, self_idx: usize, _other_idx: usize, _activator_idx: usize) {
    ctx.edicts[self_idx].s.frame = 0;
    ctx.edicts[self_idx].think_fn = Some(THINK_MISC_SATELLITE_DISH);
    ctx.edicts[self_idx].nextthink = ctx.level.time + FRAMETIME;
}

/// SP_misc_satellite_dish — spawn a satellite dish
pub fn sp_misc_satellite_dish(ctx: &mut GameContext, ent_idx: usize) {
    ctx.edicts[ent_idx].movetype = MoveType::None;
    ctx.edicts[ent_idx].solid = Solid::Bbox;
    vector_set(&mut ctx.edicts[ent_idx].mins, -64.0, -64.0, 0.0);
    vector_set(&mut ctx.edicts[ent_idx].maxs, 64.0, 64.0, 128.0);
    ctx.edicts[ent_idx].s.modelindex = gi::gi_modelindex("models/objects/satellite/tris.md2");
    ctx.edicts[ent_idx].use_fn = Some(USE_MISC_SATELLITE_DISH);
    gi::gi_linkentity(ent_idx as i32);
}

// ============================================================
// light_mine1 / light_mine2
// ============================================================

/// SP_light_mine1 — spawn mine light 1
pub fn sp_light_mine1(ctx: &mut GameContext, ent_idx: usize) {
    ctx.edicts[ent_idx].movetype = MoveType::None;
    ctx.edicts[ent_idx].solid = Solid::Bbox;
    ctx.edicts[ent_idx].s.modelindex = gi::gi_modelindex("models/objects/minelite/light1/tris.md2");
    gi::gi_linkentity(ent_idx as i32);
}

/// SP_light_mine2 — spawn mine light 2
pub fn sp_light_mine2(ctx: &mut GameContext, ent_idx: usize) {
    ctx.edicts[ent_idx].movetype = MoveType::None;
    ctx.edicts[ent_idx].solid = Solid::Bbox;
    ctx.edicts[ent_idx].s.modelindex = gi::gi_modelindex("models/objects/minelite/light2/tris.md2");
    gi::gi_linkentity(ent_idx as i32);
}

// ============================================================
// misc_gib_arm / misc_gib_leg / misc_gib_head
// ============================================================

/// SP_misc_gib_arm — spawn a gib arm (for target_spawner)
pub fn sp_misc_gib_arm(ctx: &mut GameContext, ent_idx: usize) {
    gi::gi_setmodel(ent_idx as i32, "models/objects/gibs/arm/tris.md2");
    ctx.edicts[ent_idx].solid = Solid::Not;
    ctx.edicts[ent_idx].s.effects |= EF_GIB;
    ctx.edicts[ent_idx].takedamage = DAMAGE_YES;
    ctx.edicts[ent_idx].die_fn = Some(DIE_GIB);
    ctx.edicts[ent_idx].movetype = MoveType::Toss;
    ctx.edicts[ent_idx].svflags |= SVF_MONSTER;
    ctx.edicts[ent_idx].deadflag = DEAD_DEAD;
    ctx.edicts[ent_idx].avelocity[0] = random() * 200.0;
    ctx.edicts[ent_idx].avelocity[1] = random() * 200.0;
    ctx.edicts[ent_idx].avelocity[2] = random() * 200.0;
    ctx.edicts[ent_idx].think_fn = Some(THINK_G_FREE_EDICT);
    ctx.edicts[ent_idx].nextthink = ctx.level.time + 30.0;
    gi::gi_linkentity(ent_idx as i32);
}

/// SP_misc_gib_leg — spawn a gib leg (for target_spawner)
pub fn sp_misc_gib_leg(ctx: &mut GameContext, ent_idx: usize) {
    gi::gi_setmodel(ent_idx as i32, "models/objects/gibs/leg/tris.md2");
    ctx.edicts[ent_idx].solid = Solid::Not;
    ctx.edicts[ent_idx].s.effects |= EF_GIB;
    ctx.edicts[ent_idx].takedamage = DAMAGE_YES;
    ctx.edicts[ent_idx].die_fn = Some(DIE_GIB);
    ctx.edicts[ent_idx].movetype = MoveType::Toss;
    ctx.edicts[ent_idx].svflags |= SVF_MONSTER;
    ctx.edicts[ent_idx].deadflag = DEAD_DEAD;
    ctx.edicts[ent_idx].avelocity[0] = random() * 200.0;
    ctx.edicts[ent_idx].avelocity[1] = random() * 200.0;
    ctx.edicts[ent_idx].avelocity[2] = random() * 200.0;
    ctx.edicts[ent_idx].think_fn = Some(THINK_G_FREE_EDICT);
    ctx.edicts[ent_idx].nextthink = ctx.level.time + 30.0;
    gi::gi_linkentity(ent_idx as i32);
}

/// SP_misc_gib_head — spawn a gib head (for target_spawner)
pub fn sp_misc_gib_head(ctx: &mut GameContext, ent_idx: usize) {
    gi::gi_setmodel(ent_idx as i32, "models/objects/gibs/head/tris.md2");
    ctx.edicts[ent_idx].solid = Solid::Not;
    ctx.edicts[ent_idx].s.effects |= EF_GIB;
    ctx.edicts[ent_idx].takedamage = DAMAGE_YES;
    ctx.edicts[ent_idx].die_fn = Some(DIE_GIB);
    ctx.edicts[ent_idx].movetype = MoveType::Toss;
    ctx.edicts[ent_idx].svflags |= SVF_MONSTER;
    ctx.edicts[ent_idx].deadflag = DEAD_DEAD;
    ctx.edicts[ent_idx].avelocity[0] = random() * 200.0;
    ctx.edicts[ent_idx].avelocity[1] = random() * 200.0;
    ctx.edicts[ent_idx].avelocity[2] = random() * 200.0;
    ctx.edicts[ent_idx].think_fn = Some(THINK_G_FREE_EDICT);
    ctx.edicts[ent_idx].nextthink = ctx.level.time + 30.0;
    gi::gi_linkentity(ent_idx as i32);
}

// ============================================================
// target_character / target_string
// ============================================================

/// SP_target_character — spawn a target_character (used with target_string)
pub fn sp_target_character(ctx: &mut GameContext, self_idx: usize) {
    ctx.edicts[self_idx].movetype = MoveType::Push;
    let model = ctx.edicts[self_idx].model.clone();
    gi::gi_setmodel(self_idx as i32, &model);
    ctx.edicts[self_idx].solid = Solid::Bsp;
    ctx.edicts[self_idx].s.frame = 12;
    gi::gi_linkentity(self_idx as i32);
}

/// target_string_use — update the string display
pub fn target_string_use(ctx: &mut GameContext, self_idx: usize, _other_idx: usize, _activator_idx: usize) {
    let message = ctx.edicts[self_idx].message.clone();
    let l = message.len();

    let mut e_idx_opt = {
        let tm = ctx.edicts[self_idx].teammaster;
        if tm >= 0 { Some(tm as usize) } else { None }
    };

    while let Some(e_idx) = e_idx_opt {
        if ctx.edicts[e_idx].count == 0 {
            let tc = ctx.edicts[e_idx].teamchain;
            e_idx_opt = if tc >= 0 { Some(tc as usize) } else { None };
            continue;
        }

        let n = (ctx.edicts[e_idx].count - 1) as usize;
        if n > l {
            ctx.edicts[e_idx].s.frame = 12;
        } else if let Some(c) = message.as_bytes().get(n) {
            let c = *c as char;
            if ('0'..='9').contains(&c) {
                ctx.edicts[e_idx].s.frame = (c as i32) - ('0' as i32);
            } else if c == '-' {
                ctx.edicts[e_idx].s.frame = 10;
            } else if c == ':' {
                ctx.edicts[e_idx].s.frame = 11;
            } else {
                ctx.edicts[e_idx].s.frame = 12;
            }
        } else {
            ctx.edicts[e_idx].s.frame = 12;
        }

        let tc = ctx.edicts[e_idx].teamchain;
        e_idx_opt = if tc >= 0 { Some(tc as usize) } else { None };
    }
}

/// SP_target_string — spawn a target_string
pub fn sp_target_string(ctx: &mut GameContext, self_idx: usize) {
    if ctx.edicts[self_idx].message.is_empty() {
        ctx.edicts[self_idx].message = String::new();
    }
    ctx.edicts[self_idx].use_fn = Some(USE_TARGET_STRING);
}

// ============================================================
// func_clock
// ============================================================

/// func_clock_reset — reset clock state
fn func_clock_reset(ctx: &mut GameContext, self_idx: usize) {
    ctx.edicts[self_idx].activator = -1;
    if (ctx.edicts[self_idx].spawnflags & 1) != 0 {
        ctx.edicts[self_idx].health = 0;
        ctx.edicts[self_idx].wait = ctx.edicts[self_idx].count as f32;
    } else if (ctx.edicts[self_idx].spawnflags & 2) != 0 {
        ctx.edicts[self_idx].health = ctx.edicts[self_idx].count;
        ctx.edicts[self_idx].wait = 0.0;
    }
}

/// func_clock_format_countdown — format the countdown message
fn func_clock_format_countdown(ctx: &mut GameContext, self_idx: usize) {
    let health = ctx.edicts[self_idx].health;
    let style = ctx.edicts[self_idx].style;

    if style == 0 {
        ctx.edicts[self_idx].message = format!("{:2}", health);
        return;
    }

    if style == 1 {
        let mut msg = format!("{:2}:{:2}", health / 60, health % 60);
        // Replace space at position 3 with '0'
        let bytes = unsafe { msg.as_bytes_mut() };
        if bytes.len() > 3 && bytes[3] == b' ' {
            bytes[3] = b'0';
        }
        ctx.edicts[self_idx].message = msg;
        return;
    }

    if style == 2 {
        let hours = health / 3600;
        let minutes = (health - (health / 3600) * 3600) / 60;
        let seconds = health % 60;
        let mut msg = format!("{:2}:{:2}:{:2}", hours, minutes, seconds);
        let bytes = unsafe { msg.as_bytes_mut() };
        if bytes.len() > 3 && bytes[3] == b' ' {
            bytes[3] = b'0';
        }
        if bytes.len() > 6 && bytes[6] == b' ' {
            bytes[6] = b'0';
        }
        ctx.edicts[self_idx].message = msg;
    }
}

/// func_clock_think — advance clock and update display
pub fn func_clock_think(ctx: &mut GameContext, self_idx: usize) {
    if ctx.edicts[self_idx].enemy < 0 {
        let target = ctx.edicts[self_idx].target.clone();
        let found = crate::g_utils::g_find(ctx, 0, "targetname", &target);
        if let Some(idx) = found {
            ctx.edicts[self_idx].enemy = idx as i32;
        } else {
            return;
        }
    }

    if (ctx.edicts[self_idx].spawnflags & 1) != 0 {
        func_clock_format_countdown(ctx, self_idx);
        ctx.edicts[self_idx].health += 1;
    } else if (ctx.edicts[self_idx].spawnflags & 2) != 0 {
        func_clock_format_countdown(ctx, self_idx);
        ctx.edicts[self_idx].health -= 1;
    } else {
        // Time of day clock - uses std::time for local time
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        // Convert to hours/minutes/seconds of day (UTC)
        let secs_of_day = now % 86400;
        let hours = secs_of_day / 3600;
        let minutes = (secs_of_day % 3600) / 60;
        let seconds = secs_of_day % 60;
        let mut msg = format!("{:2}:{:2}:{:2}", hours, minutes, seconds);
        let bytes = unsafe { msg.as_bytes_mut() };
        if bytes.len() > 3 && bytes[3] == b' ' {
            bytes[3] = b'0';
        }
        if bytes.len() > 6 && bytes[6] == b' ' {
            bytes[6] = b'0';
        }
        ctx.edicts[self_idx].message = msg;
    }

    let enemy_idx = ctx.edicts[self_idx].enemy as usize;
    ctx.edicts[enemy_idx].message = ctx.edicts[self_idx].message.clone();
    // enemy->use(enemy, self, self)
    crate::dispatch::call_use(enemy_idx, self_idx, self_idx, &mut ctx.edicts, &mut ctx.level);

    let health = ctx.edicts[self_idx].health;
    let wait = ctx.edicts[self_idx].wait as i32;
    if ((ctx.edicts[self_idx].spawnflags & 1) != 0 && health > wait)
        || ((ctx.edicts[self_idx].spawnflags & 2) != 0 && health < wait)
    {
        if !ctx.edicts[self_idx].pathtarget.is_empty() {
            let savetarget = ctx.edicts[self_idx].target.clone();
            let savemessage = ctx.edicts[self_idx].message.clone();
            ctx.edicts[self_idx].target = ctx.edicts[self_idx].pathtarget.clone();
            ctx.edicts[self_idx].message = String::new();
            let activator = ctx.edicts[self_idx].activator as usize;
            ctx.maxclients = ctx.game.maxclients as f32;
            ctx.num_edicts = ctx.edicts.len() as i32;
            ctx.max_edicts = ctx.edicts.capacity() as i32;
            crate::g_utils::g_use_targets(ctx, self_idx, activator);
            ctx.edicts[self_idx].target = savetarget;
            ctx.edicts[self_idx].message = savemessage;
        }

        if (ctx.edicts[self_idx].spawnflags & 8) == 0 {
            return;
        }

        func_clock_reset(ctx, self_idx);

        if (ctx.edicts[self_idx].spawnflags & 4) != 0 {
            return;
        }
    }

    ctx.edicts[self_idx].nextthink = ctx.level.time + 1.0;
}

/// func_clock_use — start the clock via use trigger
pub fn func_clock_use(ctx: &mut GameContext, self_idx: usize, _other_idx: usize, activator_idx: usize) {
    if (ctx.edicts[self_idx].spawnflags & 8) == 0 {
        ctx.edicts[self_idx].use_fn = None;
    }
    if ctx.edicts[self_idx].activator >= 0 {
        return;
    }
    ctx.edicts[self_idx].activator = activator_idx as i32;
    // self->think(self)
    func_clock_think(ctx, self_idx);
}

/// SP_func_clock — spawn a func_clock
pub fn sp_func_clock(ctx: &mut GameContext, self_idx: usize) {
    if ctx.edicts[self_idx].target.is_empty() {
        gi_dprintf(&format!(
            "{} with no target at {}\n",
            ctx.edicts[self_idx].classname,
            vtos(&ctx.edicts[self_idx].s.origin)
        ));
        g_free_edict(ctx, self_idx);
        return;
    }

    if (ctx.edicts[self_idx].spawnflags & 2) != 0 && ctx.edicts[self_idx].count == 0 {
        gi_dprintf(&format!(
            "{} with no count at {}\n",
            ctx.edicts[self_idx].classname,
            vtos(&ctx.edicts[self_idx].s.origin)
        ));
        g_free_edict(ctx, self_idx);
        return;
    }

    if (ctx.edicts[self_idx].spawnflags & 1) != 0 && ctx.edicts[self_idx].count == 0 {
        ctx.edicts[self_idx].count = 60 * 60;
    }

    func_clock_reset(ctx, self_idx);

    ctx.edicts[self_idx].message = String::with_capacity(CLOCK_MESSAGE_SIZE);

    ctx.edicts[self_idx].think_fn = Some(THINK_FUNC_CLOCK);

    if (ctx.edicts[self_idx].spawnflags & 4) != 0 {
        ctx.edicts[self_idx].use_fn = Some(USE_FUNC_CLOCK);
    } else {
        ctx.edicts[self_idx].nextthink = ctx.level.time + 1.0;
    }
}

// ============================================================
// Teleporter
// ============================================================

/// teleporter_touch — teleport player to destination
pub fn teleporter_touch(
    ctx: &mut GameContext,
    self_idx: usize,
    other_idx: usize,
    _plane: Option<&[f32; 3]>,
    _surf: Option<usize>,
) {
    if ctx.edicts[other_idx].client.is_none() {
        return;
    }

    let target = ctx.edicts[self_idx].target.clone();
    let dest_opt = crate::g_utils::g_find(ctx, 0, "targetname", &target);
    let Some(dest_idx) = dest_opt else {
        gi_dprintf("Couldn't find destination\n");
        return;
    };

    // unlink to make sure it can't possibly interfere with KillBox
    gi::gi_unlinkentity(other_idx as i32);

    let dest_origin = ctx.edicts[dest_idx].s.origin;
    vector_copy(&dest_origin, &mut ctx.edicts[other_idx].s.origin);
    vector_copy(&dest_origin, &mut ctx.edicts[other_idx].s.old_origin);
    ctx.edicts[other_idx].s.origin[2] += 10.0;

    // clear the velocity and hold them in place briefly
    vector_clear(&mut ctx.edicts[other_idx].velocity);
    if let Some(client_idx) = ctx.edicts[other_idx].client {
        ctx.clients[client_idx].ps.pmove.pm_time = (160 >> 3) as u8;
        ctx.clients[client_idx].ps.pmove.pm_flags |= PMF_TIME_TELEPORT as u8;
    }

    // draw the teleport splash at source and on the player
    let owner_idx = ctx.edicts[self_idx].owner as usize;
    ctx.edicts[owner_idx].s.event = EV_PLAYER_TELEPORT;
    ctx.edicts[other_idx].s.event = EV_PLAYER_TELEPORT;

    // set angles
    if let Some(client_idx) = ctx.edicts[other_idx].client {
        let dest_angles = ctx.edicts[dest_idx].s.angles;
        for i in 0..3 {
            ctx.clients[client_idx].ps.pmove.delta_angles[i] =
                angle2short(dest_angles[i] - ctx.clients[client_idx].resp.cmd_angles[i]);
        }

        vector_clear(&mut ctx.edicts[other_idx].s.angles);
        vector_clear(&mut ctx.clients[client_idx].ps.viewangles);
        vector_clear(&mut ctx.clients[client_idx].v_angle);
    }

    // kill anything at the destination
    ctx.maxclients = ctx.game.maxclients as f32;
    crate::g_utils::killbox(ctx, other_idx);

    gi::gi_linkentity(other_idx as i32);
}

/// SP_misc_teleporter — spawn a teleporter pad
pub fn sp_misc_teleporter(ctx: &mut GameContext, ent_idx: usize) {
    if ctx.edicts[ent_idx].target.is_empty() {
        gi_dprintf("teleporter without a target.\n");
        g_free_edict(ctx, ent_idx);
        return;
    }

    gi::gi_setmodel(ent_idx as i32, "models/objects/dmspot/tris.md2");
    ctx.edicts[ent_idx].s.skinnum = 1;
    ctx.edicts[ent_idx].s.effects = EF_TELEPORTER;
    ctx.edicts[ent_idx].s.sound = gi::gi_soundindex("world/amb10.wav");
    ctx.edicts[ent_idx].solid = Solid::Bbox;

    vector_set(&mut ctx.edicts[ent_idx].mins, -32.0, -32.0, -24.0);
    vector_set(&mut ctx.edicts[ent_idx].maxs, 32.0, 32.0, -16.0);
    gi::gi_linkentity(ent_idx as i32);

    ctx.maxclients = ctx.game.maxclients as f32;
    let trig_idx = g_spawn(ctx);
    ctx.edicts[trig_idx].touch_fn = Some(TOUCH_TELEPORTER);
    ctx.edicts[trig_idx].solid = Solid::Trigger;
    ctx.edicts[trig_idx].target = ctx.edicts[ent_idx].target.clone();
    ctx.edicts[trig_idx].owner = ent_idx as i32;
    let ent_origin = ctx.edicts[ent_idx].s.origin;
    vector_copy(&ent_origin, &mut ctx.edicts[trig_idx].s.origin);
    vector_set(&mut ctx.edicts[trig_idx].mins, -8.0, -8.0, 8.0);
    vector_set(&mut ctx.edicts[trig_idx].maxs, 8.0, 8.0, 24.0);
    gi::gi_linkentity(trig_idx as i32);
}

/// SP_misc_teleporter_dest — spawn a teleporter destination
pub fn sp_misc_teleporter_dest(ctx: &mut GameContext, ent_idx: usize) {
    gi::gi_setmodel(ent_idx as i32, "models/objects/dmspot/tris.md2");
    ctx.edicts[ent_idx].s.skinnum = 0;
    ctx.edicts[ent_idx].solid = Solid::Bbox;
    vector_set(&mut ctx.edicts[ent_idx].mins, -32.0, -32.0, -24.0);
    vector_set(&mut ctx.edicts[ent_idx].maxs, 32.0, 32.0, -16.0);
    gi::gi_linkentity(ent_idx as i32);
}

// ============================================================
// Unit tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::g_local::{Edict, GameCtx, LevelLocals, GameLocals, SpawnTemp, GClient};

    fn init_test_gi() {
        crate::game_import::set_gi(Box::new(crate::game_import::StubGameImport));
    }

    /// Helper: create a minimal GameContext with `n` default edicts.
    fn make_ctx(n: usize) -> GameContext {
        init_test_gi();
        let mut ctx = GameCtx::default();
        ctx.edicts = vec![Edict::default(); n];
        for (i, e) in ctx.edicts.iter_mut().enumerate() {
            e.inuse = i > 0; // edict 0 = world, not "in use" for searches
        }
        ctx.game = GameLocals::default();
        ctx.level = LevelLocals::default();
        ctx
    }

    // ============================================================
    // velocity_for_damage tests
    // ============================================================

    #[test]
    fn test_velocity_for_damage_low_damage_scales_down() {
        // damage < 50 should scale velocity by 0.7
        let mut v = [0.0_f32; 3];
        // Call multiple times and verify the z-component is always positive
        // and the scaling factor is 0.7 for low damage
        for _ in 0..10 {
            velocity_for_damage(30, &mut v);
            // After scaling by 0.7, z base range is [200..300]*0.7 = [140..210]
            assert!(v[2] > 0.0, "z velocity should be positive, got {}", v[2]);
            // x and y are in [-100..100]*0.7 = [-70..70]
            assert!(v[0].abs() <= 70.01, "x out of range: {}", v[0]);
            assert!(v[1].abs() <= 70.01, "y out of range: {}", v[1]);
            assert!(v[2] <= 210.01, "z out of range: {}", v[2]);
        }
    }

    #[test]
    fn test_velocity_for_damage_high_damage_scales_up() {
        // damage >= 50 should scale velocity by 1.2
        let mut v = [0.0_f32; 3];
        for _ in 0..10 {
            velocity_for_damage(100, &mut v);
            // After scaling by 1.2, z base range is [200..300]*1.2 = [240..360]
            assert!(v[2] > 0.0, "z velocity should be positive, got {}", v[2]);
            assert!(v[0].abs() <= 120.01, "x out of range: {}", v[0]);
            assert!(v[1].abs() <= 120.01, "y out of range: {}", v[1]);
            assert!(v[2] <= 360.01, "z out of range: {}", v[2]);
        }
    }

    #[test]
    fn test_velocity_for_damage_boundary() {
        // damage == 50 is the boundary -- should use 1.2 scale (not <50)
        let mut v = [0.0_f32; 3];
        velocity_for_damage(50, &mut v);
        // With 1.2 scale, z should be at least 200*1.2 = 240
        assert!(v[2] >= 240.0 * 0.99, "z velocity should be >= ~240, got {}", v[2]);
    }

    // ============================================================
    // clip_gib_velocity tests
    // ============================================================

    #[test]
    fn test_clip_gib_velocity_clamps_x_low() {
        let mut ent = Edict::default();
        ent.velocity = [-500.0, 0.0, 300.0];
        clip_gib_velocity(&mut ent);
        assert_eq!(ent.velocity[0], -300.0);
    }

    #[test]
    fn test_clip_gib_velocity_clamps_x_high() {
        let mut ent = Edict::default();
        ent.velocity = [500.0, 0.0, 300.0];
        clip_gib_velocity(&mut ent);
        assert_eq!(ent.velocity[0], 300.0);
    }

    #[test]
    fn test_clip_gib_velocity_clamps_y_low() {
        let mut ent = Edict::default();
        ent.velocity = [0.0, -500.0, 300.0];
        clip_gib_velocity(&mut ent);
        assert_eq!(ent.velocity[1], -300.0);
    }

    #[test]
    fn test_clip_gib_velocity_clamps_y_high() {
        let mut ent = Edict::default();
        ent.velocity = [0.0, 500.0, 300.0];
        clip_gib_velocity(&mut ent);
        assert_eq!(ent.velocity[1], 300.0);
    }

    #[test]
    fn test_clip_gib_velocity_clamps_z_low() {
        // z < 200 should be clamped to 200
        let mut ent = Edict::default();
        ent.velocity = [0.0, 0.0, 50.0];
        clip_gib_velocity(&mut ent);
        assert_eq!(ent.velocity[2], 200.0);
    }

    #[test]
    fn test_clip_gib_velocity_clamps_z_high() {
        // z > 500 should be clamped to 500
        let mut ent = Edict::default();
        ent.velocity = [0.0, 0.0, 999.0];
        clip_gib_velocity(&mut ent);
        assert_eq!(ent.velocity[2], 500.0);
    }

    #[test]
    fn test_clip_gib_velocity_within_bounds_unchanged() {
        let mut ent = Edict::default();
        ent.velocity = [100.0, -200.0, 350.0];
        clip_gib_velocity(&mut ent);
        assert_eq!(ent.velocity, [100.0, -200.0, 350.0]);
    }

    #[test]
    fn test_clip_gib_velocity_z_at_boundaries() {
        // z == 200 should stay 200 (not modified)
        let mut ent = Edict::default();
        ent.velocity = [0.0, 0.0, 200.0];
        clip_gib_velocity(&mut ent);
        assert_eq!(ent.velocity[2], 200.0);

        // z == 500 should stay 500
        ent.velocity = [0.0, 0.0, 500.0];
        clip_gib_velocity(&mut ent);
        assert_eq!(ent.velocity[2], 500.0);
    }

    // ============================================================
    // use_areaportal tests
    // ============================================================

    #[test]
    fn test_use_areaportal_toggles_count() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].count = 0;
        ctx.edicts[1].style = 5;

        // First toggle: 0 ^ 1 = 1
        use_areaportal(&mut ctx, 1, 0, 0);
        assert_eq!(ctx.edicts[1].count, 1);

        // Second toggle: 1 ^ 1 = 0
        use_areaportal(&mut ctx, 1, 0, 0);
        assert_eq!(ctx.edicts[1].count, 0);
    }

    #[test]
    fn test_sp_func_areaportal_sets_use_fn_and_count() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].count = 42; // should be reset to 0

        sp_func_areaportal(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].count, 0);
        assert_eq!(ctx.edicts[1].use_fn, Some(USE_AREAPORTAL));
    }

    // ============================================================
    // gib_think tests
    // ============================================================

    #[test]
    fn test_gib_think_increments_frame() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].s.frame = 5;
        ctx.level.time = 10.0;

        gib_think(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].s.frame, 6);
        assert!((ctx.edicts[1].nextthink - 10.1).abs() < 0.001);
    }

    #[test]
    fn test_gib_think_at_frame_10_schedules_free() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].s.frame = 9;
        ctx.level.time = 5.0;

        gib_think(&mut ctx, 1);
        // After incrementing, frame == 10, so think_fn should be set to free
        assert_eq!(ctx.edicts[1].s.frame, 10);
        assert_eq!(ctx.edicts[1].think_fn, Some(THINK_G_FREE_EDICT));
        // nextthink should be time + 8..18 (random component)
        assert!(ctx.edicts[1].nextthink >= 13.0); // 5.0 + 8.0
    }

    // ============================================================
    // sp_misc_explobox (barrel) initialization tests
    // ============================================================

    #[test]
    fn test_sp_misc_explobox_defaults() {
        let mut ctx = make_ctx(3);
        ctx.deathmatch = 0.0;
        ctx.edicts[1].mass = 0;
        ctx.edicts[1].health = 0;
        ctx.edicts[1].dmg = 0;

        sp_misc_explobox(&mut ctx, 1);

        assert_eq!(ctx.edicts[1].solid, Solid::Bbox);
        assert_eq!(ctx.edicts[1].movetype, MoveType::Step);
        assert_eq!(ctx.edicts[1].mass, 400);
        assert_eq!(ctx.edicts[1].health, 10);
        assert_eq!(ctx.edicts[1].dmg, 150);
        assert_eq!(ctx.edicts[1].die_fn, Some(DIE_BARREL_DELAY));
        assert_eq!(ctx.edicts[1].takedamage, DAMAGE_YES);
        assert!(ctx.edicts[1].monsterinfo.aiflags.intersects(AI_NOSTEP));
        assert_eq!(ctx.edicts[1].touch_fn, Some(TOUCH_BARREL));
        assert_eq!(ctx.edicts[1].mins, [-16.0, -16.0, 0.0]);
        assert_eq!(ctx.edicts[1].maxs, [16.0, 16.0, 40.0]);
    }

    #[test]
    fn test_sp_misc_explobox_custom_values_preserved() {
        let mut ctx = make_ctx(3);
        ctx.deathmatch = 0.0;
        ctx.edicts[1].mass = 200;
        ctx.edicts[1].health = 50;
        ctx.edicts[1].dmg = 300;

        sp_misc_explobox(&mut ctx, 1);

        assert_eq!(ctx.edicts[1].mass, 200);
        assert_eq!(ctx.edicts[1].health, 50);
        assert_eq!(ctx.edicts[1].dmg, 300);
    }

    #[test]
    fn test_sp_misc_explobox_deathmatch_frees() {
        // Entity index must be > maxclients + BODY_QUEUE_SIZE for g_free_edict to work
        let mut ctx = make_ctx(12);
        ctx.deathmatch = 1.0;

        sp_misc_explobox(&mut ctx, 10);
        // In deathmatch, entity should be freed (inuse = false)
        assert!(!ctx.edicts[10].inuse);
    }

    // ============================================================
    // barrel_delay (die callback) tests
    // ============================================================

    #[test]
    fn test_barrel_delay_sets_up_explosion() {
        let mut ctx = make_ctx(3);
        ctx.level.time = 10.0;
        ctx.edicts[1].takedamage = DAMAGE_YES;

        barrel_delay(&mut ctx, 1, 0, 2, 50, &[0.0; 3]);

        assert_eq!(ctx.edicts[1].takedamage, DAMAGE_NO);
        assert!((ctx.edicts[1].nextthink - (10.0 + 2.0 * FRAMETIME)).abs() < 0.001);
        assert_eq!(ctx.edicts[1].think_fn, Some(THINK_BARREL_EXPLODE));
        assert_eq!(ctx.edicts[1].activator, 2);
    }

    // ============================================================
    // misc_blackhole_think tests
    // ============================================================

    #[test]
    fn test_misc_blackhole_think_increments_frame() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].s.frame = 10;
        ctx.level.time = 5.0;

        misc_blackhole_think(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].s.frame, 11);
        assert!((ctx.edicts[1].nextthink - 5.1).abs() < 0.001);
    }

    #[test]
    fn test_misc_blackhole_think_wraps_at_19() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].s.frame = 18;
        ctx.level.time = 5.0;

        misc_blackhole_think(&mut ctx, 1);
        // frame goes to 19, which triggers reset to 0
        assert_eq!(ctx.edicts[1].s.frame, 0);
    }

    // ============================================================
    // misc_eastertank_think tests
    // ============================================================

    #[test]
    fn test_misc_eastertank_think_normal_advance() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].s.frame = 260;
        ctx.level.time = 1.0;

        misc_eastertank_think(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].s.frame, 261);
    }

    #[test]
    fn test_misc_eastertank_think_wraps_at_293() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].s.frame = 292;
        ctx.level.time = 1.0;

        misc_eastertank_think(&mut ctx, 1);
        // frame goes to 293, wraps to 254
        assert_eq!(ctx.edicts[1].s.frame, 254);
    }

    // ============================================================
    // misc_banner_think tests
    // ============================================================

    #[test]
    fn test_misc_banner_think_wraps_modulo_16() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].s.frame = 15;
        ctx.level.time = 1.0;

        misc_banner_think(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].s.frame, 0);
    }

    #[test]
    fn test_misc_banner_think_increments() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].s.frame = 7;
        ctx.level.time = 1.0;

        misc_banner_think(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].s.frame, 8);
    }

    // ============================================================
    // th_viewthing tests
    // ============================================================

    #[test]
    fn test_th_viewthing_wraps_modulo_7() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].s.frame = 6;
        ctx.level.time = 1.0;

        th_viewthing(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].s.frame, 0);
    }

    // ============================================================
    // sp_info_notnull tests
    // ============================================================

    #[test]
    fn test_sp_info_notnull_copies_origin() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].s.origin = [100.0, 200.0, 300.0];

        sp_info_notnull(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].absmin, [100.0, 200.0, 300.0]);
        assert_eq!(ctx.edicts[1].absmax, [100.0, 200.0, 300.0]);
    }

    // ============================================================
    // light_use tests
    // ============================================================

    #[test]
    fn test_light_use_toggle_off() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].spawnflags = 0; // light is ON (START_OFF is not set)
        ctx.edicts[1].style = 32;

        light_use(&mut ctx, 1, 0, 0);
        // Should set START_OFF
        assert_ne!(ctx.edicts[1].spawnflags & START_OFF, 0);
    }

    #[test]
    fn test_light_use_toggle_on() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].spawnflags = START_OFF; // light is OFF
        ctx.edicts[1].style = 32;

        light_use(&mut ctx, 1, 0, 0);
        // Should clear START_OFF
        assert_eq!(ctx.edicts[1].spawnflags & START_OFF, 0);
    }

    // ============================================================
    // func_wall_use tests
    // ============================================================

    #[test]
    fn test_func_wall_use_makes_solid() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].solid = Solid::Not;
        ctx.edicts[1].svflags = SVF_NOCLIENT;

        func_wall_use(&mut ctx, 1, 0, 0);
        assert_eq!(ctx.edicts[1].solid, Solid::Bsp);
        assert_eq!(ctx.edicts[1].svflags & SVF_NOCLIENT, 0);
    }

    #[test]
    fn test_func_wall_use_makes_invisible() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].solid = Solid::Bsp;
        ctx.edicts[1].svflags = 0;

        func_wall_use(&mut ctx, 1, 0, 0);
        assert_eq!(ctx.edicts[1].solid, Solid::Not);
        assert_ne!(ctx.edicts[1].svflags & SVF_NOCLIENT, 0);
    }

    #[test]
    fn test_func_wall_use_clears_use_fn_without_toggle_flag() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].solid = Solid::Bsp;
        ctx.edicts[1].spawnflags = 0; // no TOGGLE flag (bit 2)
        ctx.edicts[1].use_fn = Some(USE_FUNC_WALL);

        func_wall_use(&mut ctx, 1, 0, 0);
        assert!(ctx.edicts[1].use_fn.is_none());
    }

    #[test]
    fn test_func_wall_use_keeps_use_fn_with_toggle_flag() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].solid = Solid::Bsp;
        ctx.edicts[1].spawnflags = 2; // TOGGLE flag set
        ctx.edicts[1].use_fn = Some(USE_FUNC_WALL);

        func_wall_use(&mut ctx, 1, 0, 0);
        assert_eq!(ctx.edicts[1].use_fn, Some(USE_FUNC_WALL));
    }

    // ============================================================
    // sp_func_object tests
    // ============================================================

    #[test]
    fn test_sp_func_object_default_dmg() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].dmg = 0;
        ctx.edicts[1].spawnflags = 0;

        sp_func_object(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].dmg, 100);
        assert_eq!(ctx.edicts[1].solid, Solid::Bsp);
        assert_eq!(ctx.edicts[1].movetype, MoveType::Push);
        assert_eq!(ctx.edicts[1].clipmask, MASK_MONSTERSOLID);
    }

    #[test]
    fn test_sp_func_object_trigger_spawn() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].dmg = 50;
        ctx.edicts[1].spawnflags = 1; // trigger spawn

        sp_func_object(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].dmg, 50);
        assert_eq!(ctx.edicts[1].solid, Solid::Not);
        assert_ne!(ctx.edicts[1].svflags & SVF_NOCLIENT, 0);
        assert_eq!(ctx.edicts[1].use_fn, Some(USE_FUNC_OBJECT));
    }

    // ============================================================
    // sp_misc_deadsoldier tests
    // ============================================================

    #[test]
    fn test_sp_misc_deadsoldier_frame_selection() {
        // Test various spawnflags map to correct frame
        let cases = [
            (0, 0),   // no flags -> frame 0
            (2, 1),   // flag 2 -> frame 1
            (4, 2),   // flag 4 -> frame 2
            (8, 3),   // flag 8 -> frame 3
            (16, 4),  // flag 16 -> frame 4
            (32, 5),  // flag 32 -> frame 5
        ];

        for (spawnflags, expected_frame) in &cases {
            let mut ctx = make_ctx(2);
            ctx.deathmatch = 0.0;
            ctx.edicts[1].spawnflags = *spawnflags;

            sp_misc_deadsoldier(&mut ctx, 1);
            assert_eq!(
                ctx.edicts[1].s.frame, *expected_frame,
                "spawnflags {} should give frame {}", spawnflags, expected_frame
            );
            assert_eq!(ctx.edicts[1].deadflag, DEAD_DEAD);
            assert_eq!(ctx.edicts[1].takedamage, DAMAGE_YES);
            assert_ne!(ctx.edicts[1].svflags & SVF_MONSTER, 0);
            assert_ne!(ctx.edicts[1].svflags & SVF_DEADMONSTER, 0);
        }
    }

    #[test]
    fn test_sp_misc_deadsoldier_deathmatch_frees() {
        let mut ctx = make_ctx(12);
        ctx.deathmatch = 1.0;

        sp_misc_deadsoldier(&mut ctx, 10);
        assert!(!ctx.edicts[10].inuse);
    }

    // ============================================================
    // commander_body_drop tests
    // ============================================================

    #[test]
    fn test_commander_body_drop_sets_toss_and_raises() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].movetype = MoveType::None;
        ctx.edicts[1].s.origin = [0.0, 0.0, 100.0];

        commander_body_drop(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].movetype, MoveType::Toss);
        assert_eq!(ctx.edicts[1].s.origin[2], 102.0);
    }

    // ============================================================
    // commander_body_think tests
    // ============================================================

    #[test]
    fn test_commander_body_think_advances_frame() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].s.frame = 10;
        ctx.level.time = 1.0;

        commander_body_think(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].s.frame, 11);
        assert!((ctx.edicts[1].nextthink - 1.1).abs() < 0.001);
    }

    #[test]
    fn test_commander_body_think_stops_at_24() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].s.frame = 23;
        ctx.level.time = 1.0;

        commander_body_think(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].s.frame, 24);
        assert_eq!(ctx.edicts[1].nextthink, 0.0);
    }

    // ============================================================
    // func_clock_format_countdown tests
    // ============================================================

    #[test]
    fn test_func_clock_format_style0_plain_seconds() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].health = 42;
        ctx.edicts[1].style = 0;

        func_clock_format_countdown(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].message, "42");
    }

    #[test]
    fn test_func_clock_format_style1_minutes_seconds() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].health = 125; // 2 min 5 sec
        ctx.edicts[1].style = 1;

        func_clock_format_countdown(&mut ctx, 1);
        // Expected: " 2:05"
        assert!(ctx.edicts[1].message.contains("2"));
        assert!(ctx.edicts[1].message.contains("05") || ctx.edicts[1].message.contains(": 5"));
    }

    #[test]
    fn test_func_clock_format_style2_hours_minutes_seconds() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].health = 3661; // 1h 1m 1s
        ctx.edicts[1].style = 2;

        func_clock_format_countdown(&mut ctx, 1);
        assert!(ctx.edicts[1].message.contains("1"));
    }

    // ============================================================
    // func_clock_reset tests
    // ============================================================

    #[test]
    fn test_func_clock_reset_timer_up() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].spawnflags = 1; // TIMER_UP
        ctx.edicts[1].count = 60;

        func_clock_reset(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].activator, -1);
        assert_eq!(ctx.edicts[1].health, 0);
        assert_eq!(ctx.edicts[1].wait, 60.0);
    }

    #[test]
    fn test_func_clock_reset_timer_down() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].spawnflags = 2; // TIMER_DOWN
        ctx.edicts[1].count = 120;

        func_clock_reset(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].activator, -1);
        assert_eq!(ctx.edicts[1].health, 120);
        assert_eq!(ctx.edicts[1].wait, 0.0);
    }

    // ============================================================
    // sp_func_explosive tests
    // ============================================================

    #[test]
    fn test_sp_func_explosive_default_health() {
        let mut ctx = make_ctx(3);
        ctx.deathmatch = 0.0;
        ctx.edicts[1].health = 0;
        ctx.edicts[1].spawnflags = 0; // not trigger_spawn, no targetname

        sp_func_explosive(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].health, 100);
        assert_eq!(ctx.edicts[1].takedamage, DAMAGE_YES);
        assert_eq!(ctx.edicts[1].die_fn, Some(DIE_FUNC_EXPLOSIVE_EXPLODE));
    }

    #[test]
    fn test_sp_func_explosive_deathmatch_frees() {
        let mut ctx = make_ctx(12);
        ctx.deathmatch = 1.0;

        sp_func_explosive(&mut ctx, 10);
        assert!(!ctx.edicts[10].inuse);
    }

    #[test]
    fn test_sp_func_explosive_trigger_spawn() {
        let mut ctx = make_ctx(3);
        ctx.deathmatch = 0.0;
        ctx.edicts[1].spawnflags = 1; // TRIGGER_SPAWN

        sp_func_explosive(&mut ctx, 1);
        assert_ne!(ctx.edicts[1].svflags & SVF_NOCLIENT, 0);
        assert_eq!(ctx.edicts[1].solid, Solid::Not);
        assert_eq!(ctx.edicts[1].use_fn, Some(USE_FUNC_EXPLOSIVE_SPAWN));
    }

    // ============================================================
    // misc_satellite_dish_think tests
    // ============================================================

    #[test]
    fn test_misc_satellite_dish_think_normal() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].s.frame = 20;
        ctx.level.time = 1.0;

        misc_satellite_dish_think(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].s.frame, 21);
        assert!((ctx.edicts[1].nextthink - 1.1).abs() < 0.001);
    }

    #[test]
    fn test_misc_satellite_dish_think_stops_at_38() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].s.frame = 37;
        ctx.level.time = 1.0;

        misc_satellite_dish_think(&mut ctx, 1);
        // frame goes to 38, no nextthink set (stops animating)
        assert_eq!(ctx.edicts[1].s.frame, 38);
        // nextthink should remain at default (0.0) since we don't set it
        assert_eq!(ctx.edicts[1].nextthink, 0.0);
    }

    // ============================================================
    // misc_satellite_dish_use tests
    // ============================================================

    #[test]
    fn test_misc_satellite_dish_use_resets_frame() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].s.frame = 30;
        ctx.level.time = 5.0;

        misc_satellite_dish_use(&mut ctx, 1, 0, 0);
        assert_eq!(ctx.edicts[1].s.frame, 0);
        assert_eq!(ctx.edicts[1].think_fn, Some(THINK_MISC_SATELLITE_DISH));
        assert!((ctx.edicts[1].nextthink - 5.1).abs() < 0.001);
    }

    // ============================================================
    // sp_misc_viper_bomb tests
    // ============================================================

    #[test]
    fn test_sp_misc_viper_bomb_default_dmg() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].dmg = 0;

        sp_misc_viper_bomb(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].dmg, 1000);
        assert_eq!(ctx.edicts[1].movetype, MoveType::None);
        assert_eq!(ctx.edicts[1].solid, Solid::Not);
        assert_ne!(ctx.edicts[1].svflags & SVF_NOCLIENT, 0);
        assert_eq!(ctx.edicts[1].use_fn, Some(USE_MISC_VIPER_BOMB));
    }

    #[test]
    fn test_sp_misc_viper_bomb_custom_dmg() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].dmg = 500;

        sp_misc_viper_bomb(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].dmg, 500);
    }

    // ============================================================
    // misc_viper_bomb_prethink tests
    // ============================================================

    #[test]
    fn test_misc_viper_bomb_prethink_adjusts_angles() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].timestamp = 10.0;
        ctx.edicts[1].moveinfo.dir = [1.0, 0.0, 0.0];
        ctx.edicts[1].s.angles = [0.0, 0.0, 30.0]; // roll = 30
        ctx.level.time = 9.5;

        misc_viper_bomb_prethink(&mut ctx, 1);
        // roll should increase by 10
        assert!((ctx.edicts[1].s.angles[2] - 40.0).abs() < 0.01);
        // groundentity should be cleared
        assert_eq!(ctx.edicts[1].groundentity, -1);
    }

    #[test]
    fn test_misc_viper_bomb_prethink_clamps_diff() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].timestamp = 5.0;
        ctx.edicts[1].moveinfo.dir = [0.0, 1.0, 0.0];
        ctx.edicts[1].s.angles = [0.0, 0.0, 0.0];
        ctx.level.time = 100.0; // diff would be -95, clamped to -1

        misc_viper_bomb_prethink(&mut ctx, 1);
        // Should not crash and roll should be 10.0
        assert!((ctx.edicts[1].s.angles[2] - 10.0).abs() < 0.01);
    }
}

