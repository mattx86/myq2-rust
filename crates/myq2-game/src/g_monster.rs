// g_monster.rs -- Monster utility functions and weapons
// Converted from: myq2-original/game/g_monster.c

use crate::g_local::*;
use crate::game::*;
use crate::game_import::*;
use myq2_common::q_shared::{Vec3, MASK_MONSTERSOLID, MASK_WATER, CONTENTS_LAVA, CONTENTS_SLIME, CONTENTS_WATER};


// EF_*, RF_*, CHAN_*, ATTN_*, YAW come from g_local::* re-export (myq2_common::q_shared)

// ============================================================
// Monster weapons
// ============================================================

// Monster weapon fire functions. Spread parameters control accuracy.
// A skill-based accuracy system would adjust spread based on difficulty.

pub fn monster_fire_bullet(
    ctx: &mut GameContext,
    self_idx: i32,
    start: Vec3,
    dir: Vec3,
    damage: i32,
    kick: i32,
    hspread: i32,
    vspread: i32,
    flashtype: i32,
) {
    crate::g_weapon::fire_bullet(self_idx as usize, &mut ctx.edicts, &mut ctx.level,
        &start, &dir, damage, kick, hspread, vspread, MOD_UNKNOWN);

    gi_write_byte(SVC_MUZZLEFLASH2);
    gi_write_short(self_idx);
    gi_write_byte(flashtype);
    gi_multicast(&start, MULTICAST_PVS);
}

pub fn monster_fire_shotgun(
    ctx: &mut GameContext,
    self_idx: i32,
    start: Vec3,
    aimdir: Vec3,
    damage: i32,
    kick: i32,
    hspread: i32,
    vspread: i32,
    count: i32,
    flashtype: i32,
) {
    crate::g_weapon::fire_shotgun(self_idx as usize, &mut ctx.edicts, &mut ctx.level,
        &start, &aimdir, damage, kick, hspread, vspread, count, MOD_UNKNOWN);

    gi_write_byte(SVC_MUZZLEFLASH2);
    gi_write_short(self_idx);
    gi_write_byte(flashtype);
    gi_multicast(&start, MULTICAST_PVS);
}

pub fn monster_fire_blaster(
    ctx: &mut GameContext,
    self_idx: i32,
    start: Vec3,
    dir: Vec3,
    damage: i32,
    speed: i32,
    flashtype: i32,
    effect: i32,
) {
    crate::g_weapon::fire_blaster(self_idx as usize, &mut ctx.edicts, &mut ctx.level,
        &start, &dir, damage, speed, effect, false);

    gi_write_byte(SVC_MUZZLEFLASH2);
    gi_write_short(self_idx);
    gi_write_byte(flashtype);
    gi_multicast(&start, MULTICAST_PVS);
}

pub fn monster_fire_grenade(
    ctx: &mut GameContext,
    self_idx: i32,
    start: Vec3,
    aimdir: Vec3,
    damage: i32,
    speed: i32,
    flashtype: i32,
) {
    crate::g_weapon::fire_grenade(self_idx as usize, &mut ctx.edicts, &mut ctx.level,
        &start, &aimdir, damage, speed, 2.5, (damage + 40) as f32);

    gi_write_byte(SVC_MUZZLEFLASH2);
    gi_write_short(self_idx);
    gi_write_byte(flashtype);
    gi_multicast(&start, MULTICAST_PVS);
}

pub fn monster_fire_rocket(
    ctx: &mut GameContext,
    self_idx: i32,
    start: Vec3,
    dir: Vec3,
    damage: i32,
    speed: i32,
    flashtype: i32,
) {
    crate::g_weapon::fire_rocket(self_idx as usize, &mut ctx.edicts, &mut ctx.level,
        &start, &dir, damage, speed, (damage + 20) as f32, damage);

    gi_write_byte(SVC_MUZZLEFLASH2);
    gi_write_short(self_idx);
    gi_write_byte(flashtype);
    gi_multicast(&start, MULTICAST_PVS);
}

pub fn monster_fire_railgun(
    ctx: &mut GameContext,
    self_idx: i32,
    start: Vec3,
    aimdir: Vec3,
    damage: i32,
    kick: i32,
    flashtype: i32,
) {
    crate::g_weapon::fire_rail(self_idx as usize, &mut ctx.edicts, &mut ctx.level,
        &start, &aimdir, damage, kick);

    gi_write_byte(SVC_MUZZLEFLASH2);
    gi_write_short(self_idx);
    gi_write_byte(flashtype);
    gi_multicast(&start, MULTICAST_PVS);
}

pub fn monster_fire_bfg(
    ctx: &mut GameContext,
    self_idx: i32,
    start: Vec3,
    aimdir: Vec3,
    damage: i32,
    speed: i32,
    _kick: i32,
    damage_radius: f32,
    flashtype: i32,
) {
    crate::g_weapon::fire_bfg(self_idx as usize, &mut ctx.edicts, &mut ctx.level,
        &start, &aimdir, damage, speed, damage_radius);

    gi_write_byte(SVC_MUZZLEFLASH2);
    gi_write_short(self_idx);
    gi_write_byte(flashtype);
    gi_multicast(&start, MULTICAST_PVS);
}

// ============================================================
// Context-free monster weapon wrappers
// Used by monster modules that don't have a full GameContext.
// These delegate to the same muzzle-flash logic without needing ctx.
// ============================================================

pub fn monster_fire_bullet_raw(
    self_idx: i32, start: Vec3, dir: Vec3,
    damage: i32, kick: i32, hspread: i32, vspread: i32, flashtype: i32,
) {
    // fire_bullet deferred: requires edicts/level not available in raw variant
    gi_write_byte(SVC_MUZZLEFLASH2);
    gi_write_short(self_idx);
    gi_write_byte(flashtype);
    gi_multicast(&start, MULTICAST_PVS);
}

pub fn monster_fire_shotgun_raw(
    self_idx: i32, start: Vec3, aimdir: Vec3,
    damage: i32, kick: i32, hspread: i32, vspread: i32, count: i32, flashtype: i32,
) {
    // fire_shotgun deferred: requires edicts/level not available in raw variant
    gi_write_byte(SVC_MUZZLEFLASH2);
    gi_write_short(self_idx);
    gi_write_byte(flashtype);
    gi_multicast(&start, MULTICAST_PVS);
}

pub fn monster_fire_blaster_raw(
    self_idx: i32, start: Vec3, dir: Vec3,
    damage: i32, speed: i32, flashtype: i32, effect: i32,
) {
    // fire_blaster deferred: requires edicts/level not available in raw variant
    gi_write_byte(SVC_MUZZLEFLASH2);
    gi_write_short(self_idx);
    gi_write_byte(flashtype);
    gi_multicast(&start, MULTICAST_PVS);
}

pub fn monster_fire_grenade_raw(
    self_idx: i32, start: Vec3, aimdir: Vec3,
    damage: i32, speed: i32, flashtype: i32,
) {
    // fire_grenade deferred: requires edicts/level not available in raw variant
    gi_write_byte(SVC_MUZZLEFLASH2);
    gi_write_short(self_idx);
    gi_write_byte(flashtype);
    gi_multicast(&start, MULTICAST_PVS);
}

pub fn monster_fire_rocket_raw(
    self_idx: i32, start: Vec3, dir: Vec3,
    damage: i32, speed: i32, flashtype: i32,
) {
    // fire_rocket deferred: requires edicts/level not available in raw variant
    gi_write_byte(SVC_MUZZLEFLASH2);
    gi_write_short(self_idx);
    gi_write_byte(flashtype);
    gi_multicast(&start, MULTICAST_PVS);
}

pub fn monster_fire_railgun_raw(
    self_idx: i32, start: Vec3, aimdir: Vec3,
    damage: i32, kick: i32, flashtype: i32,
) {
    // fire_rail deferred: requires edicts/level not available in raw variant
    gi_write_byte(SVC_MUZZLEFLASH2);
    gi_write_short(self_idx);
    gi_write_byte(flashtype);
    gi_multicast(&start, MULTICAST_PVS);
}

pub fn monster_fire_bfg_raw(
    self_idx: i32, start: Vec3, aimdir: Vec3,
    damage: i32, speed: i32, kick: i32, damage_radius: f32, flashtype: i32,
) {
    // fire_bfg deferred: requires edicts/level not available in raw variant
    gi_write_byte(SVC_MUZZLEFLASH2);
    gi_write_short(self_idx);
    gi_write_byte(flashtype);
    gi_multicast(&start, MULTICAST_PVS);
}

// ============================================================
// Monster utility functions
// ============================================================

pub fn m_flies_off(ctx: &mut GameContext, self_idx: i32) {
    let ent = &mut ctx.edicts[self_idx as usize];
    ent.s.effects &= !EF_FLIES;
    ent.s.sound = 0;
}

pub fn m_flies_on(ctx: &mut GameContext, self_idx: i32) {
    {
        let ent = &ctx.edicts[self_idx as usize];
        if ent.waterlevel != 0 {
            return;
        }
    }

    let ent = &mut ctx.edicts[self_idx as usize];
    ent.s.effects |= EF_FLIES;
    ent.s.sound = gi_soundindex("infantry/inflies1.wav");
    ent.think_fn = Some(crate::dispatch::THINK_M_FLIES_OFF);
    ent.nextthink = ctx.level.time + 60.0;
}

pub fn m_fly_check(ctx: &mut GameContext, self_idx: i32) {
    let ent = &ctx.edicts[self_idx as usize];
    if ent.waterlevel != 0 {
        return;
    }

    let r: f32 = rand::random();
    if r > 0.5 {
        return;
    }

    let ent = &mut ctx.edicts[self_idx as usize];
    ent.think_fn = Some(crate::dispatch::THINK_M_FLIES_ON);
    let rand_val: f32 = rand::random();
    ent.nextthink = ctx.level.time + 5.0 + 10.0 * rand_val;
}


pub fn attack_finished(ctx: &mut GameContext, self_idx: i32, time: f32) {
    let ent = &mut ctx.edicts[self_idx as usize];
    ent.monsterinfo.attack_finished = ctx.level.time + time;
}

pub fn m_check_ground(ctx: &mut GameContext, ent_idx: i32) {
    let ent = &ctx.edicts[ent_idx as usize];

    if ent.flags.intersects(FL_SWIM | FL_FLY) {
        return;
    }

    if ent.velocity[2] > 100.0 {
        let ent = &mut ctx.edicts[ent_idx as usize];
        ent.groundentity = -1;
        return;
    }

    // if the hull point one-quarter unit down is solid the entity is on ground
    let _point = [
        ent.s.origin[0],
        ent.s.origin[1],
        ent.s.origin[2] - 0.25,
    ];

    let ent = &ctx.edicts[ent_idx as usize];
    let _tr = gi_trace(&ent.s.origin, &ent.mins, &ent.maxs, &_point, ent_idx, MASK_MONSTERSOLID);

    // Placeholder: in real implementation, check trace results
    // For now, simulate the trace logic structure:
    //
    // if trace.plane.normal[2] < 0.7 && !trace.startsolid {
    //     ent.groundentity = -1;
    //     return;
    // }
    //
    // if !trace.startsolid && !trace.allsolid {
    //     VectorCopy(trace.endpos, ent.s.origin);
    //     ent.groundentity = trace.ent;
    //     ent.groundentity_linkcount = edicts[trace.ent].linkcount;
    //     ent.velocity[2] = 0.0;
    // }
}

/// Standalone wrapper for `m_check_ground` that works with raw edicts/level.
/// Used by g_phys.rs which doesn't have a full GameContext.
pub fn m_check_ground_raw(ent_idx: i32, edicts: &mut Vec<Edict>, level: &mut LevelLocals) {
    let mut ctx = GameCtx {
        level: std::mem::take(level),
        edicts: std::mem::take(edicts),
        ..GameCtx::default()
    };
    m_check_ground(&mut ctx, ent_idx);
    *edicts = ctx.edicts;
    *level = ctx.level;
}

pub fn m_categorize_position(ctx: &mut GameContext, ent_idx: i32) {
    let ent = &ctx.edicts[ent_idx as usize];

    // get waterlevel
    let mut point = [
        ent.s.origin[0],
        ent.s.origin[1],
        ent.s.origin[2] + ent.mins[2] + 1.0,
    ];

    let cont = gi_pointcontents(&point);

    if cont & MASK_WATER == 0 {
        let ent = &mut ctx.edicts[ent_idx as usize];
        ent.waterlevel = 0;
        ent.watertype = 0;
        return;
    }

    let ent = &mut ctx.edicts[ent_idx as usize];
    ent.watertype = cont;
    ent.waterlevel = 1;
    point[2] += 26.0;

    let cont = gi_pointcontents(&point);

    if cont & MASK_WATER == 0 {
        return;
    }

    ent.waterlevel = 2;
    point[2] += 22.0;

    let cont = gi_pointcontents(&point);

    if cont & MASK_WATER != 0 {
        ent.waterlevel = 3;
    }
}

pub fn m_world_effects(ctx: &mut GameContext, ent_idx: i32) {
    let time = ctx.level.time;
    let ei = ent_idx as usize;
    let zero_vec = [0.0f32; 3];

    if ctx.edicts[ei].health > 0 {
        if !ctx.edicts[ei].flags.intersects(FL_SWIM) {
            if ctx.edicts[ei].waterlevel < 3 {
                ctx.edicts[ei].air_finished = time + 12.0;
            } else if ctx.edicts[ei].air_finished < time {
                // drown!
                if ctx.edicts[ei].pain_debounce_time < time {
                    let mut dmg = 2 + (2.0 * (time - ctx.edicts[ei].air_finished).floor()) as i32;
                    if dmg > 15 {
                        dmg = 15;
                    }
                    let origin = ctx.edicts[ei].s.origin;
                    ctx.maxclients = ctx.game.maxclients as f32;
                    crate::g_combat::ctx_t_damage(
                        ctx, ei, 0, 0,
                        &zero_vec, &origin, &zero_vec,
                        dmg, 0, DAMAGE_NO_ARMOR, MOD_WATER,
                    );
                    ctx.edicts[ei].pain_debounce_time = time + 1.0;
                }
            }
        } else if ctx.edicts[ei].waterlevel > 0 {
            ctx.edicts[ei].air_finished = time + 9.0;
        } else if ctx.edicts[ei].air_finished < time {
            // suffocate!
            if ctx.edicts[ei].pain_debounce_time < time {
                let mut dmg = 2 + (2.0 * (time - ctx.edicts[ei].air_finished).floor()) as i32;
                if dmg > 15 {
                    dmg = 15;
                }
                let origin = ctx.edicts[ei].s.origin;
                ctx.maxclients = ctx.game.maxclients as f32;
                crate::g_combat::ctx_t_damage(
                    ctx, ei, 0, 0,
                    &zero_vec, &origin, &zero_vec,
                    dmg, 0, DAMAGE_NO_ARMOR, MOD_WATER,
                );
                ctx.edicts[ei].pain_debounce_time = time + 1.0;
            }
        }
    }

    if ctx.edicts[ei].waterlevel == 0 {
        if ctx.edicts[ei].flags.intersects(FL_INWATER) {
            gi_sound(ent_idx, CHAN_BODY, gi_soundindex("player/watr_out.wav"), 1.0, ATTN_NORM, 0.0);
            ctx.edicts[ei].flags.remove(FL_INWATER);
        }
        return;
    }

    if (ctx.edicts[ei].watertype & CONTENTS_LAVA != 0) && (!ctx.edicts[ei].flags.intersects(FL_IMMUNE_LAVA))
        && ctx.edicts[ei].damage_debounce_time < time {
            ctx.edicts[ei].damage_debounce_time = time + 0.2;
            let dmg = 10 * ctx.edicts[ei].waterlevel;
            let origin = ctx.edicts[ei].s.origin;
            ctx.maxclients = ctx.game.maxclients as f32;
            crate::g_combat::ctx_t_damage(
                ctx, ei, 0, 0,
                &zero_vec, &origin, &zero_vec,
                dmg, 0, DamageFlags::empty(), MOD_LAVA,
            );
        }

    if (ctx.edicts[ei].watertype & CONTENTS_SLIME != 0) && (!ctx.edicts[ei].flags.intersects(FL_IMMUNE_SLIME))
        && ctx.edicts[ei].damage_debounce_time < time {
            ctx.edicts[ei].damage_debounce_time = time + 1.0;
            let dmg = 4 * ctx.edicts[ei].waterlevel;
            let origin = ctx.edicts[ei].s.origin;
            ctx.maxclients = ctx.game.maxclients as f32;
            crate::g_combat::ctx_t_damage(
                ctx, ei, 0, 0,
                &zero_vec, &origin, &zero_vec,
                dmg, 0, DamageFlags::empty(), MOD_SLIME,
            );
        }

    if !ctx.edicts[ei].flags.intersects(FL_INWATER) {
        if ctx.edicts[ei].svflags & SVF_DEADMONSTER == 0 {
            if ctx.edicts[ei].watertype & CONTENTS_LAVA != 0 {
                let r: f32 = rand::random();
                if r <= 0.5 {
                    gi_sound(ent_idx, CHAN_BODY, gi_soundindex("player/lava1.wav"), 1.0, ATTN_NORM, 0.0);
                } else {
                    gi_sound(ent_idx, CHAN_BODY, gi_soundindex("player/lava2.wav"), 1.0, ATTN_NORM, 0.0);
                }
            } else if ctx.edicts[ei].watertype & CONTENTS_SLIME != 0 {
                gi_sound(ent_idx, CHAN_BODY, gi_soundindex("player/watr_in.wav"), 1.0, ATTN_NORM, 0.0);
            } else if ctx.edicts[ei].watertype & CONTENTS_WATER != 0 {
                gi_sound(ent_idx, CHAN_BODY, gi_soundindex("player/watr_in.wav"), 1.0, ATTN_NORM, 0.0);
            }
        }

        ctx.edicts[ei].flags.insert(FL_INWATER);
        ctx.edicts[ei].damage_debounce_time = 0.0;
    }
}

pub fn m_drop_to_floor(ctx: &mut GameContext, ent_idx: i32) {
    let ent = &mut ctx.edicts[ent_idx as usize];
    ent.s.origin[2] += 1.0;

    let mut end = ent.s.origin;
    end[2] -= 256.0;

    let _tr = gi_trace(&ent.s.origin, &ent.mins, &ent.maxs, &end, ent_idx, MASK_MONSTERSOLID);

    // Placeholder: in real implementation check trace.fraction == 1 || trace.allsolid
    // if trace.fraction == 1.0 || trace.allsolid { return; }
    // VectorCopy(trace.endpos, ent->s.origin);

    gi_linkentity(ent_idx);

    m_check_ground(ctx, ent_idx);
    m_categorize_position(ctx, ent_idx);
}

pub fn m_set_effects(ctx: &mut GameContext, ent_idx: i32) {
    let time = ctx.level.time;
    let ent = &mut ctx.edicts[ent_idx as usize];

    ent.s.effects &= !(EF_COLOR_SHELL | EF_POWERSCREEN);
    ent.s.renderfx &= !(RF_SHELL_RED | RF_SHELL_GREEN | RF_SHELL_BLUE);

    if ent.monsterinfo.aiflags.intersects(AI_RESURRECTING) {
        ent.s.effects |= EF_COLOR_SHELL;
        ent.s.renderfx |= RF_SHELL_RED;
    }

    if ent.health <= 0 {
        return;
    }

    if ent.powerarmor_time > time {
        if ent.monsterinfo.power_armor_type == POWER_ARMOR_SCREEN {
            ent.s.effects |= EF_POWERSCREEN;
        } else if ent.monsterinfo.power_armor_type == POWER_ARMOR_SHIELD {
            ent.s.effects |= EF_COLOR_SHELL;
            ent.s.renderfx |= RF_SHELL_GREEN;
        }
    }
}

pub fn m_move_frame(ctx: &mut GameContext, self_idx: i32) {
    let time = ctx.level.time;
    let ent = &mut ctx.edicts[self_idx as usize];

    let move_idx = match ent.monsterinfo.currentmove {
        Some(idx) => idx,
        None => return,
    };

    ent.nextthink = time + FRAMETIME;

    // In the real implementation, move_idx indexes into a table of mmove_t.
    // For now we document the algorithm with placeholder logic.
    //
    // The C code does:
    //   move = self->monsterinfo.currentmove;
    //   if nextframe is set and in range, jump to it
    //   else advance frame, calling endfunc at last frame
    //   then call aifunc and thinkfunc for the current frame
    //
    // This requires access to the mmove_t table which is not yet defined.
    // M_MoveFrame: full frame advancement deferred until mmove_t table is defined.
    // The C code advances the frame counter within the current mmove_t,
    // calls aifunc and thinkfunc for each frame, and endfunc at the last frame.
}

pub fn monster_think(ctx: &mut GameContext, self_idx: i32) {
    m_move_frame(ctx, self_idx);

    let ent = &ctx.edicts[self_idx as usize];
    if ent.linkcount != ent.monsterinfo.linkcount {
        let ent = &mut ctx.edicts[self_idx as usize];
        ent.monsterinfo.linkcount = ent.linkcount;
        m_check_ground(ctx, self_idx);
    }

    m_categorize_position(ctx, self_idx);
    m_world_effects(ctx, self_idx);
    m_set_effects(ctx, self_idx);
}

/// Using a monster makes it angry at the current activator.
pub fn monster_use(ctx: &mut GameContext, self_idx: i32, _other_idx: i32, activator_idx: i32) {
    let ent = &ctx.edicts[self_idx as usize];

    if ent.enemy >= 0 {
        return;
    }
    if ent.health <= 0 {
        return;
    }

    let activator = &ctx.edicts[activator_idx as usize];
    if activator.flags.intersects(FL_NOTARGET) {
        return;
    }
    if activator.client.is_none() && !activator.monsterinfo.aiflags.intersects(AI_GOOD_GUY) {
        return;
    }

    // delay reaction so if the monster is teleported, its sound is still heard
    let activator_origin = ctx.edicts[activator_idx as usize].s.origin;
    let ent = &mut ctx.edicts[self_idx as usize];
    ent.enemy = activator_idx;

    // FoundTarget: set last_sighting and call run
    ent.monsterinfo.last_sighting = activator_origin;
    ent.monsterinfo.trail_time = ctx.level.time;
    crate::dispatch::call_run(self_idx as usize, &mut ctx.edicts, &mut ctx.level);
}

pub fn monster_triggered_spawn(ctx: &mut GameContext, self_idx: i32) {
    let time = ctx.level.time;
    let ent = &mut ctx.edicts[self_idx as usize];

    ent.s.origin[2] += 1.0;

    let si = self_idx as usize;
    ctx.maxclients = ctx.game.maxclients as f32;
    crate::g_utils::killbox(ctx, si);
    let ent = &mut ctx.edicts[self_idx as usize];

    ent.solid = Solid::Bbox;
    ent.movetype = MoveType::Step;
    ent.svflags &= !SVF_NOCLIENT;
    ent.air_finished = time + 12.0;

    gi_linkentity(self_idx);

    monster_start_go(ctx, self_idx);

    let ent = &ctx.edicts[self_idx as usize];
    let enemy_idx = ent.enemy;
    let spawnflags = ent.spawnflags;

    if enemy_idx >= 0 && (spawnflags & 1 == 0) {
        let enemy = &ctx.edicts[enemy_idx as usize];
        if !enemy.flags.intersects(FL_NOTARGET) {
            let enemy_origin = ctx.edicts[enemy_idx as usize].s.origin;
            ctx.edicts[self_idx as usize].monsterinfo.last_sighting = enemy_origin;
            ctx.edicts[self_idx as usize].monsterinfo.trail_time = ctx.level.time;
            crate::dispatch::call_run(self_idx as usize, &mut ctx.edicts, &mut ctx.level);
        } else {
            let ent = &mut ctx.edicts[self_idx as usize];
            ent.enemy = -1;
        }
    } else {
        let ent = &mut ctx.edicts[self_idx as usize];
        ent.enemy = -1;
    }
}

pub fn monster_triggered_spawn_use(ctx: &mut GameContext, self_idx: i32, _other_idx: i32, activator_idx: i32) {
    let time = ctx.level.time;

    // we have a one frame delay here so we don't telefrag the guy who activated us
    let ent = &mut ctx.edicts[self_idx as usize];
    ent.think_fn = Some(crate::dispatch::THINK_MONSTER_TRIGGERED_SPAWN);
    ent.nextthink = time + FRAMETIME;

    let activator = &ctx.edicts[activator_idx as usize];
    if activator.client.is_some() {
        let ent = &mut ctx.edicts[self_idx as usize];
        ent.enemy = activator_idx;
    }

    let ent = &mut ctx.edicts[self_idx as usize];
    ent.use_fn = Some(crate::dispatch::USE_MONSTER_USE);
}

pub fn monster_triggered_start(ctx: &mut GameContext, self_idx: i32) {
    let ent = &mut ctx.edicts[self_idx as usize];
    ent.solid = Solid::Not;
    ent.movetype = MoveType::None;
    ent.svflags |= SVF_NOCLIENT;
    ent.nextthink = 0.0;
    ent.use_fn = Some(crate::dispatch::USE_MONSTER_TRIGGERED_SPAWN_USE);
}

/// When a monster dies, it fires all of its targets with the current
/// enemy as activator.
pub fn monster_death_use(ctx: &mut GameContext, self_idx: i32) {
    let ent = &mut ctx.edicts[self_idx as usize];

    ent.flags.remove(FL_FLY | FL_SWIM);
    ent.monsterinfo.aiflags &= AI_GOOD_GUY;

    if ent.item.is_some() {
        // Drop_Item deferred: requires full item spawn context
        ent.item = None;
    }

    if !ent.deathtarget.is_empty() {
        ent.target = ent.deathtarget.clone();
    }

    if ent.target.is_empty() {
        return;
    }

    let enemy_idx = ctx.edicts[self_idx as usize].enemy;
    let si = self_idx as usize;
    let ai = if enemy_idx >= 0 { enemy_idx as usize } else { 0 };
    ctx.maxclients = ctx.game.maxclients as f32;
    ctx.num_edicts = ctx.edicts.len() as i32;
    ctx.max_edicts = ctx.edicts.capacity() as i32;
    crate::g_utils::g_use_targets(ctx, si, ai);
}

// ============================================================
// Monster start functions
// ============================================================

pub fn monster_start(ctx: &mut GameContext, self_idx: i32) -> bool {
    if ctx.deathmatch != 0.0 {
        let si = self_idx as usize;
        ctx.maxclients = ctx.game.maxclients as f32;
        crate::g_utils::g_free_edict(ctx, si);
        return false;
    }

    let ent = &mut ctx.edicts[self_idx as usize];

    if (ent.spawnflags & 4 != 0) && !ent.monsterinfo.aiflags.intersects(AI_GOOD_GUY) {
        ent.spawnflags &= !4;
        ent.spawnflags |= 1;
    }

    if !ent.monsterinfo.aiflags.intersects(AI_GOOD_GUY) {
        ctx.level.total_monsters += 1;
    }

    let time = ctx.level.time;
    let ent = &mut ctx.edicts[self_idx as usize];

    ent.nextthink = time + FRAMETIME;
    ent.svflags |= SVF_MONSTER;
    ent.s.renderfx |= RF_FRAMELERP;
    ent.takedamage = Damage::Aim as i32;
    ent.air_finished = time + 12.0;
    ent.use_fn = Some(crate::dispatch::USE_MONSTER_USE);
    ent.max_health = ent.health;
    ent.clipmask = MASK_MONSTERSOLID;

    ent.s.skinnum = 0;
    ent.deadflag = DEAD_NO;
    ent.svflags &= !SVF_DEADMONSTER;

    if ent.monsterinfo.checkattack_fn.is_none() {
        ent.monsterinfo.checkattack_fn = Some(crate::dispatch::MCHECKATTACK_DEFAULT);
    }

    // VectorCopy(self->s.origin, self->s.old_origin);
    ent.s.old_origin = ent.s.origin;

    if !ctx.st.item.is_empty() {
        // FindItemByClassname deferred: requires full item table GameContext
        // The caller should set ent.item after calling monster_start
    }

    // randomize what frame they start on
    // Deferred: requires mmove_t table to determine frame range

    true
}

pub fn monster_start_go(ctx: &mut GameContext, self_idx: i32) {
    let ent = &ctx.edicts[self_idx as usize];

    if ent.health <= 0 {
        return;
    }

    // check for target to combat_point and change to combattarget
    if !ent.target.is_empty() {
        let target_name = ent.target.clone();
        let mut notcombat = false;
        let mut fixup = false;

        // while ((target = G_Find(target, FOFS(targetname), self->target)) != NULL)
        // Iterate through all entities matching targetname
        for i in 0..ctx.edicts.len() {
            if ctx.edicts[i].targetname == target_name && ctx.edicts[i].inuse {
                if ctx.edicts[i].classname == "point_combat" {
                    let ent = &mut ctx.edicts[self_idx as usize];
                    ent.combattarget = target_name.clone();
                    fixup = true;
                } else {
                    notcombat = true;
                }
            }
        }

        let ent = &ctx.edicts[self_idx as usize];
        if notcombat && !ent.combattarget.is_empty() {
            gi_dprintf(&format!("{} at ({:.0} {:.0} {:.0}) has target with mixed types\n", ent.classname, ent.s.origin[0], ent.s.origin[1], ent.s.origin[2]));
        }
        if fixup {
            let ent = &mut ctx.edicts[self_idx as usize];
            ent.target.clear();
        }
    }

    // validate combattarget
    let ent = &ctx.edicts[self_idx as usize];
    if !ent.combattarget.is_empty() {
        let ct = ent.combattarget.clone();
        for i in 0..ctx.edicts.len() {
            if ctx.edicts[i].targetname == ct && ctx.edicts[i].inuse
                && ctx.edicts[i].classname != "point_combat" {
                    let self_ent = &ctx.edicts[self_idx as usize];
                    gi_dprintf(&format!(
                        "{} at ({} {} {}) has a bad combattarget {} : {} at ({} {} {})\n",
                        self_ent.classname,
                        self_ent.s.origin[0] as i32,
                        self_ent.s.origin[1] as i32,
                        self_ent.s.origin[2] as i32,
                        ct,
                        ctx.edicts[i].classname,
                        ctx.edicts[i].s.origin[0] as i32,
                        ctx.edicts[i].s.origin[1] as i32,
                        ctx.edicts[i].s.origin[2] as i32,
                    ));
                }
        }
    }

    let ent = &ctx.edicts[self_idx as usize];
    if !ent.target.is_empty() {
        let target_str = ent.target.clone();

        // self->goalentity = self->movetarget = G_PickTarget(self->target);
        // Placeholder: find a matching target entity
        let mut found_target: i32 = -1;
        for i in 0..ctx.edicts.len() {
            if ctx.edicts[i].targetname == target_str && ctx.edicts[i].inuse {
                found_target = i as i32;
                break;
            }
        }

        if found_target < 0 {
            let ent = &ctx.edicts[self_idx as usize];
            gi_dprintf(&format!("{} can't find target {} at ({} {} {})\n",
                ent.classname, target_str,
                ent.s.origin[0], ent.s.origin[1], ent.s.origin[2]));

            let ent = &mut ctx.edicts[self_idx as usize];
            ent.target.clear();
            ent.monsterinfo.pausetime = 100000000.0;
            crate::dispatch::call_stand(self_idx as usize, &mut ctx.edicts, &mut ctx.level);
        } else if ctx.edicts[found_target as usize].classname == "path_corner" {
            let goal_origin = ctx.edicts[found_target as usize].s.origin;
            let ent = &mut ctx.edicts[self_idx as usize];
            ent.goalentity = found_target;
            ent.movetarget = found_target;

            let self_origin = ent.s.origin;
            let v = [
                goal_origin[0] - self_origin[0],
                goal_origin[1] - self_origin[1],
                goal_origin[2] - self_origin[2],
            ];

            // self->ideal_yaw = self->s.angles[YAW] = vectoyaw(v);
            let yaw = v[1].atan2(v[0]).to_degrees();
            ent.ideal_yaw = yaw;
            ent.s.angles[YAW] = yaw;

            crate::dispatch::call_walk(self_idx as usize, &mut ctx.edicts, &mut ctx.level);
            ctx.edicts[self_idx as usize].target.clear();
        } else {
            let ent = &mut ctx.edicts[self_idx as usize];
            ent.goalentity = -1;
            ent.movetarget = -1;
            ent.monsterinfo.pausetime = 100000000.0;
            crate::dispatch::call_stand(self_idx as usize, &mut ctx.edicts, &mut ctx.level);
        }
    } else {
        let ent = &mut ctx.edicts[self_idx as usize];
        ent.monsterinfo.pausetime = 100000000.0;
        crate::dispatch::call_stand(self_idx as usize, &mut ctx.edicts, &mut ctx.level);
    }

    let time = ctx.level.time;
    let ent = &mut ctx.edicts[self_idx as usize];
    ent.think_fn = Some(crate::dispatch::THINK_MONSTER);
    ent.nextthink = time + FRAMETIME;
}

pub fn walkmonster_start_go(ctx: &mut GameContext, self_idx: i32) {
    let time = ctx.level.time;

    let ent = &ctx.edicts[self_idx as usize];
    if (ent.spawnflags & 2 == 0) && time < 1.0 {
        m_drop_to_floor(ctx, self_idx);

        let ent = &ctx.edicts[self_idx as usize];
        if ent.groundentity >= 0 {
            // M_walkmove solid check deferred: requires MoveContext not available here
        }
    }

    let ent = &mut ctx.edicts[self_idx as usize];
    if ent.yaw_speed == 0.0 {
        ent.yaw_speed = 20.0;
    }
    ent.viewheight = 25;

    monster_start_go(ctx, self_idx);

    let ent = &ctx.edicts[self_idx as usize];
    if ent.spawnflags & 2 != 0 {
        monster_triggered_start(ctx, self_idx);
    }
}

pub fn walkmonster_start(ctx: &mut GameContext, self_idx: i32) {
    let ent = &mut ctx.edicts[self_idx as usize];
    ent.think_fn = Some(crate::dispatch::THINK_WALKMONSTER_START_GO);

    monster_start(ctx, self_idx);
}

pub fn flymonster_start_go(ctx: &mut GameContext, self_idx: i32) {
    // M_walkmove solid check deferred: requires MoveContext not available here

    let ent = &mut ctx.edicts[self_idx as usize];
    if ent.yaw_speed == 0.0 {
        ent.yaw_speed = 10.0;
    }
    ent.viewheight = 25;

    monster_start_go(ctx, self_idx);

    let ent = &ctx.edicts[self_idx as usize];
    if ent.spawnflags & 2 != 0 {
        monster_triggered_start(ctx, self_idx);
    }
}

pub fn flymonster_start(ctx: &mut GameContext, self_idx: i32) {
    let ent = &mut ctx.edicts[self_idx as usize];
    ent.flags.insert(FL_FLY);
    ent.think_fn = Some(crate::dispatch::THINK_FLYMONSTER_START_GO);

    monster_start(ctx, self_idx);
}

pub fn swimmonster_start_go(ctx: &mut GameContext, self_idx: i32) {
    let ent = &mut ctx.edicts[self_idx as usize];
    if ent.yaw_speed == 0.0 {
        ent.yaw_speed = 10.0;
    }
    ent.viewheight = 10;

    monster_start_go(ctx, self_idx);

    let ent = &ctx.edicts[self_idx as usize];
    if ent.spawnflags & 2 != 0 {
        monster_triggered_start(ctx, self_idx);
    }
}

pub fn swimmonster_start(ctx: &mut GameContext, self_idx: i32) {
    let ent = &mut ctx.edicts[self_idx as usize];
    ent.flags.insert(FL_SWIM);
    ent.think_fn = Some(crate::dispatch::THINK_SWIMMONSTER_START_GO);

    monster_start(ctx, self_idx);
}

// ============================================================
// Unit tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::g_local::{Edict, GameCtx, LevelLocals, GameLocals, SpawnTemp};

    fn init_test_gi() {
        crate::game_import::set_gi(Box::new(crate::game_import::StubGameImport));
    }

    /// Helper: create a minimal GameContext with `n` default edicts.
    fn make_ctx(n: usize) -> GameContext {
        init_test_gi();
        let mut ctx = GameCtx::default();
        ctx.edicts = vec![Edict::default(); n];
        for (i, e) in ctx.edicts.iter_mut().enumerate() {
            e.inuse = i > 0;
        }
        ctx.game = GameLocals::default();
        ctx.level = LevelLocals::default();
        ctx.st = SpawnTemp::default();
        ctx
    }

    // ============================================================
    // monster_use tests
    // ============================================================

    #[test]
    fn test_monster_use_ignores_if_already_has_enemy() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].enemy = 2;  // already has an enemy
        ctx.edicts[1].health = 100;

        monster_use(&mut ctx, 1, 0, 2);
        // enemy should not change -- it stays as 2
        assert_eq!(ctx.edicts[1].enemy, 2);
    }

    #[test]
    fn test_monster_use_ignores_dead_monster() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].enemy = -1;
        ctx.edicts[1].health = 0; // dead

        monster_use(&mut ctx, 1, 0, 2);
        assert_eq!(ctx.edicts[1].enemy, -1);
    }

    #[test]
    fn test_monster_use_ignores_notarget_activator() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].enemy = -1;
        ctx.edicts[1].health = 100;
        ctx.edicts[2].flags = FL_NOTARGET;

        monster_use(&mut ctx, 1, 0, 2);
        assert_eq!(ctx.edicts[1].enemy, -1);
    }

    #[test]
    fn test_monster_use_ignores_non_client_non_goodguy() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].enemy = -1;
        ctx.edicts[1].health = 100;
        ctx.edicts[2].client = None;
        ctx.edicts[2].monsterinfo.aiflags = AiFlags::empty(); // not a good guy

        monster_use(&mut ctx, 1, 0, 2);
        assert_eq!(ctx.edicts[1].enemy, -1);
    }

    #[test]
    fn test_monster_use_sets_enemy_for_good_guy_activator() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].enemy = -1;
        ctx.edicts[1].health = 100;
        ctx.edicts[2].client = None;
        ctx.edicts[2].monsterinfo.aiflags = AI_GOOD_GUY;
        ctx.edicts[2].s.origin = [50.0, 60.0, 70.0];

        monster_use(&mut ctx, 1, 0, 2);
        assert_eq!(ctx.edicts[1].enemy, 2);
        assert_eq!(ctx.edicts[1].monsterinfo.last_sighting, [50.0, 60.0, 70.0]);
    }

    #[test]
    fn test_monster_use_sets_enemy_for_client_activator() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].enemy = -1;
        ctx.edicts[1].health = 100;
        ctx.edicts[2].client = Some(0); // has a client
        ctx.edicts[2].s.origin = [10.0, 20.0, 30.0];
        ctx.level.time = 5.0;

        monster_use(&mut ctx, 1, 0, 2);
        assert_eq!(ctx.edicts[1].enemy, 2);
        assert_eq!(ctx.edicts[1].monsterinfo.last_sighting, [10.0, 20.0, 30.0]);
        assert_eq!(ctx.edicts[1].monsterinfo.trail_time, 5.0);
    }

    // ============================================================
    // monster_triggered_start tests
    // ============================================================

    #[test]
    fn test_monster_triggered_start_hides_entity() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].solid = Solid::Bbox;
        ctx.edicts[1].movetype = MoveType::Step;
        ctx.edicts[1].svflags = 0;

        monster_triggered_start(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].solid, Solid::Not);
        assert_eq!(ctx.edicts[1].movetype, MoveType::None);
        assert_ne!(ctx.edicts[1].svflags & SVF_NOCLIENT, 0);
        assert_eq!(ctx.edicts[1].nextthink, 0.0);
        assert_eq!(ctx.edicts[1].use_fn, Some(crate::dispatch::USE_MONSTER_TRIGGERED_SPAWN_USE));
    }

    // ============================================================
    // monster_triggered_spawn_use tests
    // ============================================================

    #[test]
    fn test_monster_triggered_spawn_use_sets_think_and_enemy() {
        let mut ctx = make_ctx(3);
        ctx.level.time = 10.0;
        ctx.edicts[2].client = Some(0); // activator is a client

        monster_triggered_spawn_use(&mut ctx, 1, 0, 2);
        assert_eq!(ctx.edicts[1].think_fn, Some(crate::dispatch::THINK_MONSTER_TRIGGERED_SPAWN));
        assert!((ctx.edicts[1].nextthink - (10.0 + FRAMETIME)).abs() < 0.001);
        assert_eq!(ctx.edicts[1].enemy, 2);
        assert_eq!(ctx.edicts[1].use_fn, Some(crate::dispatch::USE_MONSTER_USE));
    }

    #[test]
    fn test_monster_triggered_spawn_use_non_client_activator() {
        let mut ctx = make_ctx(3);
        ctx.level.time = 10.0;
        ctx.edicts[2].client = None; // activator is NOT a client

        monster_triggered_spawn_use(&mut ctx, 1, 0, 2);
        assert_eq!(ctx.edicts[1].think_fn, Some(crate::dispatch::THINK_MONSTER_TRIGGERED_SPAWN));
        // enemy should not be set (stays at default 0)
        assert_eq!(ctx.edicts[1].enemy, 0);
    }

    // ============================================================
    // monster_start tests
    // ============================================================

    #[test]
    fn test_monster_start_deathmatch_frees_entity() {
        // Entity index must be > maxclients + BODY_QUEUE_SIZE for g_free_edict to work
        let mut ctx = make_ctx(12);
        ctx.deathmatch = 1.0;
        ctx.edicts[10].inuse = true;

        let result = monster_start(&mut ctx, 10);
        assert!(!result);
        assert!(!ctx.edicts[10].inuse);
    }

    #[test]
    fn test_monster_start_sets_up_entity() {
        let mut ctx = make_ctx(3);
        ctx.deathmatch = 0.0;
        ctx.level.time = 5.0;
        ctx.edicts[1].health = 200;

        let result = monster_start(&mut ctx, 1);
        assert!(result);

        let ent = &ctx.edicts[1];
        assert!((ent.nextthink - (5.0 + FRAMETIME)).abs() < 0.001);
        assert_ne!(ent.svflags & SVF_MONSTER, 0);
        assert_ne!(ent.s.renderfx & RF_FRAMELERP, 0);
        assert_eq!(ent.takedamage, Damage::Aim as i32);
        assert!((ent.air_finished - 17.0).abs() < 0.001); // 5.0 + 12.0
        assert_eq!(ent.use_fn, Some(crate::dispatch::USE_MONSTER_USE));
        assert_eq!(ent.max_health, 200);
        assert_eq!(ent.clipmask, MASK_MONSTERSOLID);
        assert_eq!(ent.deadflag, DEAD_NO);
        assert_eq!(ent.s.old_origin, ent.s.origin);
    }

    #[test]
    fn test_monster_start_increments_total_monsters() {
        let mut ctx = make_ctx(3);
        ctx.deathmatch = 0.0;
        ctx.level.total_monsters = 5;

        monster_start(&mut ctx, 1);
        assert_eq!(ctx.level.total_monsters, 6);
    }

    #[test]
    fn test_monster_start_good_guy_no_monster_count() {
        let mut ctx = make_ctx(3);
        ctx.deathmatch = 0.0;
        ctx.level.total_monsters = 5;
        ctx.edicts[1].monsterinfo.aiflags = AI_GOOD_GUY;

        monster_start(&mut ctx, 1);
        assert_eq!(ctx.level.total_monsters, 5); // should NOT increment
    }

    #[test]
    fn test_monster_start_converts_spawnflag_4_to_trigger() {
        let mut ctx = make_ctx(3);
        ctx.deathmatch = 0.0;
        ctx.edicts[1].spawnflags = 4;

        monster_start(&mut ctx, 1);
        // spawnflag 4 should be converted to spawnflag 1 (triggered spawn)
        assert_eq!(ctx.edicts[1].spawnflags & 4, 0);
        assert_ne!(ctx.edicts[1].spawnflags & 1, 0);
    }

    #[test]
    fn test_monster_start_sets_default_checkattack() {
        let mut ctx = make_ctx(3);
        ctx.deathmatch = 0.0;
        ctx.edicts[1].monsterinfo.checkattack_fn = None;

        monster_start(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].monsterinfo.checkattack_fn, Some(crate::dispatch::MCHECKATTACK_DEFAULT));
    }

    #[test]
    fn test_monster_start_preserves_existing_checkattack() {
        let mut ctx = make_ctx(3);
        ctx.deathmatch = 0.0;
        ctx.edicts[1].monsterinfo.checkattack_fn = Some(42);

        monster_start(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].monsterinfo.checkattack_fn, Some(42));
    }

    // ============================================================
    // monster_death_use tests
    // ============================================================

    #[test]
    fn test_monster_death_use_clears_flags() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].flags = FL_FLY | FL_SWIM | FL_GODMODE;
        ctx.edicts[1].monsterinfo.aiflags = AI_GOOD_GUY | AI_MEDIC | AI_BRUTAL;
        ctx.edicts[1].target = String::new(); // empty target, return early

        monster_death_use(&mut ctx, 1);
        // FL_FLY and FL_SWIM should be removed, FL_GODMODE kept
        assert!(!ctx.edicts[1].flags.intersects(FL_FLY | FL_SWIM));
        assert!(ctx.edicts[1].flags.intersects(FL_GODMODE));
        // aiflags should only keep AI_GOOD_GUY (bitwise AND)
        assert_eq!(ctx.edicts[1].monsterinfo.aiflags, AI_GOOD_GUY);
    }

    #[test]
    fn test_monster_death_use_copies_deathtarget() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].deathtarget = "my_target".to_string();
        ctx.edicts[1].target = "old_target".to_string();

        monster_death_use(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].target, "my_target");
    }

    #[test]
    fn test_monster_death_use_clears_item() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].item = Some(5);
        ctx.edicts[1].target = String::new();

        monster_death_use(&mut ctx, 1);
        assert!(ctx.edicts[1].item.is_none());
    }

    // ============================================================
    // attack_finished tests
    // ============================================================

    #[test]
    fn test_attack_finished_sets_time() {
        let mut ctx = make_ctx(2);
        ctx.level.time = 10.0;

        attack_finished(&mut ctx, 1, 3.0);
        assert!((ctx.edicts[1].monsterinfo.attack_finished - 13.0).abs() < 0.001);
    }

    // ============================================================
    // m_flies_off / m_flies_on tests
    // ============================================================

    #[test]
    fn test_m_flies_off_clears_effects() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].s.effects = EF_FLIES | EF_GIB;
        ctx.edicts[1].s.sound = 42;

        m_flies_off(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].s.effects & EF_FLIES, 0);
        // EF_GIB should be preserved
        assert_ne!(ctx.edicts[1].s.effects & EF_GIB, 0);
        assert_eq!(ctx.edicts[1].s.sound, 0);
    }

    #[test]
    fn test_m_flies_on_does_nothing_in_water() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].waterlevel = 1;
        ctx.edicts[1].s.effects = 0;

        m_flies_on(&mut ctx, 1);
        // Should return early, effects unchanged
        assert_eq!(ctx.edicts[1].s.effects & EF_FLIES, 0);
    }

    // ============================================================
    // m_set_effects tests
    // ============================================================

    #[test]
    fn test_m_set_effects_resurrecting() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].monsterinfo.aiflags = AI_RESURRECTING;
        ctx.edicts[1].health = 100;
        ctx.level.time = 1.0;

        m_set_effects(&mut ctx, 1);
        assert_ne!(ctx.edicts[1].s.effects & EF_COLOR_SHELL, 0);
        assert_ne!(ctx.edicts[1].s.renderfx & RF_SHELL_RED, 0);
    }

    #[test]
    fn test_m_set_effects_dead_returns_early() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].health = 0;
        ctx.edicts[1].powerarmor_time = 999.0;
        ctx.edicts[1].monsterinfo.power_armor_type = POWER_ARMOR_SCREEN;
        ctx.level.time = 1.0;

        m_set_effects(&mut ctx, 1);
        // Power armor effects should NOT be applied when dead
        assert_eq!(ctx.edicts[1].s.effects & EF_POWERSCREEN, 0);
    }

    #[test]
    fn test_m_set_effects_power_screen() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].health = 100;
        ctx.edicts[1].powerarmor_time = 10.0;
        ctx.edicts[1].monsterinfo.power_armor_type = POWER_ARMOR_SCREEN;
        ctx.level.time = 5.0;

        m_set_effects(&mut ctx, 1);
        assert_ne!(ctx.edicts[1].s.effects & EF_POWERSCREEN, 0);
    }

    #[test]
    fn test_m_set_effects_power_shield() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].health = 100;
        ctx.edicts[1].powerarmor_time = 10.0;
        ctx.edicts[1].monsterinfo.power_armor_type = POWER_ARMOR_SHIELD;
        ctx.level.time = 5.0;

        m_set_effects(&mut ctx, 1);
        assert_ne!(ctx.edicts[1].s.effects & EF_COLOR_SHELL, 0);
        assert_ne!(ctx.edicts[1].s.renderfx & RF_SHELL_GREEN, 0);
    }

    #[test]
    fn test_m_set_effects_clears_previous_shell_effects() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].health = 100;
        ctx.edicts[1].s.effects = EF_COLOR_SHELL | EF_POWERSCREEN;
        ctx.edicts[1].s.renderfx = RF_SHELL_RED | RF_SHELL_GREEN | RF_SHELL_BLUE;
        ctx.edicts[1].powerarmor_time = 0.0; // expired
        ctx.level.time = 1.0;

        m_set_effects(&mut ctx, 1);
        // All shell effects should be cleared since no power armor and no resurrecting
        assert_eq!(ctx.edicts[1].s.effects & (EF_COLOR_SHELL | EF_POWERSCREEN), 0);
        assert_eq!(ctx.edicts[1].s.renderfx & (RF_SHELL_RED | RF_SHELL_GREEN | RF_SHELL_BLUE), 0);
    }

    // ============================================================
    // walkmonster_start_go yaw_speed tests
    // ============================================================

    #[test]
    fn test_walkmonster_start_go_default_yaw_speed() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].yaw_speed = 0.0;
        ctx.edicts[1].health = 100;
        ctx.edicts[1].spawnflags = 2; // no-drop flag, skip m_drop_to_floor

        walkmonster_start_go(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].yaw_speed, 20.0);
        assert_eq!(ctx.edicts[1].viewheight, 25);
    }

    #[test]
    fn test_walkmonster_start_go_preserves_yaw_speed() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].yaw_speed = 30.0;
        ctx.edicts[1].health = 100;
        ctx.edicts[1].spawnflags = 2;

        walkmonster_start_go(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].yaw_speed, 30.0);
    }

    #[test]
    fn test_walkmonster_start_go_triggered_spawn() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].health = 100;
        ctx.edicts[1].spawnflags = 2; // triggered spawn flag

        walkmonster_start_go(&mut ctx, 1);
        // Should call monster_triggered_start
        assert_eq!(ctx.edicts[1].solid, Solid::Not);
        assert_eq!(ctx.edicts[1].movetype, MoveType::None);
        assert_ne!(ctx.edicts[1].svflags & SVF_NOCLIENT, 0);
    }

    // ============================================================
    // flymonster_start tests
    // ============================================================

    #[test]
    fn test_flymonster_start_sets_fly_flag() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].flags = EntityFlags::empty();

        flymonster_start(&mut ctx, 1);
        assert!(ctx.edicts[1].flags.intersects(FL_FLY));
    }

    // ============================================================
    // swimmonster_start tests
    // ============================================================

    #[test]
    fn test_swimmonster_start_sets_swim_flag() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].flags = EntityFlags::empty();

        swimmonster_start(&mut ctx, 1);
        assert!(ctx.edicts[1].flags.intersects(FL_SWIM));
    }

    // ============================================================
    // flymonster_start_go tests
    // ============================================================

    #[test]
    fn test_flymonster_start_go_default_yaw_speed() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].yaw_speed = 0.0;
        ctx.edicts[1].health = 100;
        ctx.edicts[1].spawnflags = 2; // triggered

        flymonster_start_go(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].yaw_speed, 10.0);
        assert_eq!(ctx.edicts[1].viewheight, 25);
    }

    // ============================================================
    // swimmonster_start_go tests
    // ============================================================

    #[test]
    fn test_swimmonster_start_go_default_yaw_speed() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].yaw_speed = 0.0;
        ctx.edicts[1].health = 100;
        ctx.edicts[1].spawnflags = 2; // triggered

        swimmonster_start_go(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].yaw_speed, 10.0);
        assert_eq!(ctx.edicts[1].viewheight, 10);
    }

    // ============================================================
    // monster_start_go with health <= 0 tests
    // ============================================================

    #[test]
    fn test_monster_start_go_dead_returns_early() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].health = 0;

        monster_start_go(&mut ctx, 1);
        // Should return early, nothing should be set on the entity
        assert!(ctx.edicts[1].think_fn.is_none());
    }

    // ============================================================
    // m_check_ground tests
    // ============================================================

    #[test]
    fn test_m_check_ground_skips_fly() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].flags = FL_FLY;
        ctx.edicts[1].velocity = [0.0, 0.0, 200.0];

        m_check_ground(&mut ctx, 1);
        // Should return early without modifying groundentity
        assert_eq!(ctx.edicts[1].groundentity, 0);
    }

    #[test]
    fn test_m_check_ground_skips_swim() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].flags = FL_SWIM;

        m_check_ground(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].groundentity, 0);
    }

    #[test]
    fn test_m_check_ground_high_upward_velocity() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].velocity = [0.0, 0.0, 200.0]; // > 100

        m_check_ground(&mut ctx, 1);
        assert_eq!(ctx.edicts[1].groundentity, -1);
    }
}

