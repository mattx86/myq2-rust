// p_view.rs — Player view calculations
// Converted from: myq2-original/game/p_view.c
//
// Copyright (C) 1997-2001 Id Software, Inc.
// Licensed under the GNU General Public License v2

use crate::g_local::*;
use crate::game_import::*;
use myq2_common::q_shared::{
    Vec3, PITCH, YAW, ROLL, STAT_FLASHES,
    PMF_DUCKED, CONTENTS_LAVA, CONTENTS_SLIME, CONTENTS_WATER, CONTENTS_SOLID,
    RDF_UNDERWATER,
    EF_POWERSCREEN, EF_COLOR_SHELL, EF_QUAD, EF_PENT,
    RF_SHELL_RED, RF_SHELL_GREEN, RF_SHELL_BLUE,
    DF_NO_FALLING, DmFlags, EntityEvent,
    CHAN_VOICE, CHAN_BODY, CHAN_ITEM, CHAN_AUTO,
    ATTN_NORM, ATTN_STATIC,
    vector_clear, vector_normalize,
    dot_product, vector_copy_to as vector_copy, vector_add_to as vector_add,
    vector_subtract_to as vector_subtract, vector_ma_to as vector_ma,
    angle_vectors,
};
use crate::m_player_frames::*;

// ============================================================
// View context — replaces C static/global state
// ============================================================

/// Holds per-frame view calculation state that was stored in C statics/globals.
pub struct ViewContext {
    pub forward: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    pub xyspeed: f32,
    pub bobmove: f32,
    pub bobcycle: i32,
    pub bobfracsin: f32,
    /// Static pain animation index (persists across calls like C `static int i`)
    pub pain_anim_index: i32,
}

impl Default for ViewContext {
    fn default() -> Self {
        Self {
            forward: [0.0; 3],
            right: [0.0; 3],
            up: [0.0; 3],
            xyspeed: 0.0,
            bobmove: 0.0,
            bobcycle: 0,
            bobfracsin: 0.0,
            pain_anim_index: 0,
        }
    }
}

// ============================================================
// Cvar placeholders — in the full engine these come from the cvar system
// ============================================================

/// Placeholder cvar value struct.
pub struct CvarRef {
    pub value: f32,
}

/// Placeholder cvar references. In a full integration these would be
/// resolved from the cvar system. For now we provide sensible defaults.
pub struct ViewCvars {
    pub sv_rollangle: CvarRef,
    pub sv_rollspeed: CvarRef,
    pub run_pitch: CvarRef,
    pub run_roll: CvarRef,
    pub bob_up: CvarRef,
    pub bob_pitch: CvarRef,
    pub bob_roll: CvarRef,
    pub gun_x: CvarRef,
    pub gun_y: CvarRef,
    pub gun_z: CvarRef,
    pub deathmatch: CvarRef,
    pub dmflags: CvarRef,
}

impl Default for ViewCvars {
    fn default() -> Self {
        Self {
            sv_rollangle: CvarRef { value: 2.0 },
            sv_rollspeed: CvarRef { value: 200.0 },
            run_pitch: CvarRef { value: 0.002 },
            run_roll: CvarRef { value: 0.005 },
            bob_up: CvarRef { value: 0.005 },
            bob_pitch: CvarRef { value: 0.002 },
            bob_roll: CvarRef { value: 0.002 },
            gun_x: CvarRef { value: 0.0 },
            gun_y: CvarRef { value: 0.0 },
            gun_z: CvarRef { value: 0.0 },
            deathmatch: CvarRef { value: 0.0 },
            dmflags: CvarRef { value: 0.0 },
        }
    }
}

// ============================================================
// Helper functions (inline equivalents of Q2 macros)
// ============================================================


// ============================================================
// SV_CalcRoll
// ============================================================

pub fn sv_calc_roll(
    _angles: &Vec3,
    velocity: &Vec3,
    right: &Vec3,
    cvars: &ViewCvars,
) -> f32 {
    let mut side = dot_product(velocity, right);
    let sign: f32 = if side < 0.0 { -1.0 } else { 1.0 };
    side = side.abs();

    let value = cvars.sv_rollangle.value;

    if side < cvars.sv_rollspeed.value {
        side = side * value / cvars.sv_rollspeed.value;
    } else {
        side = value;
    }

    side * sign
}

// ============================================================
// P_DamageFeedback
//
// Handles color blends and view kicks
// ============================================================

pub fn p_damage_feedback(
    ent: &mut Edict,
    client: &mut GClient,
    level: &LevelLocals,
    vctx: &mut ViewContext,
) {
    let power_color: Vec3 = [0.0, 1.0, 0.0];
    let acolor: Vec3 = [1.0, 1.0, 1.0];
    let bcolor: Vec3 = [1.0, 0.0, 0.0];

    // flash the backgrounds behind the status numbers
    client.ps.stats[STAT_FLASHES as usize] = 0;
    if client.damage_blood != 0 {
        client.ps.stats[STAT_FLASHES as usize] |= 1;
    }
    if client.damage_armor != 0
        && !ent.flags.intersects(FL_GODMODE)
        && client.invincible_framenum <= level.framenum as f32
    {
        client.ps.stats[STAT_FLASHES as usize] |= 2;
    }

    // total points of damage shot at the player this frame
    let count_total = client.damage_blood + client.damage_armor + client.damage_parmor;
    if count_total == 0 {
        return; // didn't take any damage
    }
    let count_f = count_total as f32;

    // start a pain animation if still in the player model
    if client.anim_priority < ANIM_PAIN && ent.s.modelindex == 255 {
        client.anim_priority = ANIM_PAIN;
        if (client.ps.pmove.pm_flags & PMF_DUCKED) != 0 {
            ent.s.frame = FRAME_CRPAIN1 - 1;
            client.anim_end = FRAME_CRPAIN4;
        } else {
            vctx.pain_anim_index = (vctx.pain_anim_index + 1) % 3;
            match vctx.pain_anim_index {
                0 => {
                    ent.s.frame = FRAME_PAIN101 - 1;
                    client.anim_end = FRAME_PAIN104;
                }
                1 => {
                    ent.s.frame = FRAME_PAIN201 - 1;
                    client.anim_end = FRAME_PAIN204;
                }
                2 => {
                    ent.s.frame = FRAME_PAIN301 - 1;
                    client.anim_end = FRAME_PAIN304;
                }
                _ => unreachable!(),
            }
        }
    }

    let realcount = count_f;
    let count = if count_f < 10.0 { 10.0 } else { count_f };

    // play an appropriate pain sound
    if level.time > ent.pain_debounce_time
        && !ent.flags.intersects(FL_GODMODE)
        && client.invincible_framenum <= level.framenum as f32
    {
        let r = 1 + (rand_int() & 1);
        ent.pain_debounce_time = level.time + 0.7;
        let l = if ent.health < 25 {
            25
        } else if ent.health < 50 {
            50
        } else if ent.health < 75 {
            75
        } else {
            100
        };
        gi_sound(ent.s.number, CHAN_VOICE, gi_soundindex(&format!("*pain{}_{}.wav", l, r)), 1.0, ATTN_NORM as f32, 0.0);
    }

    // the total alpha of the blend is always proportional to count
    if client.damage_alpha < 0.0 {
        client.damage_alpha = 0.0;
    }
    client.damage_alpha += count * 0.01;
    if client.damage_alpha < 0.2 {
        client.damage_alpha = 0.2;
    }
    if client.damage_alpha > 0.6 {
        client.damage_alpha = 0.6; // don't go too saturated
    }

    // the color of the blend will vary based on how much was absorbed
    // by different armors
    let mut v: Vec3 = [0.0; 3];
    if client.damage_parmor != 0 {
        let scale = client.damage_parmor as f32 / realcount;
        let tmp = v;
        vector_ma(&tmp, scale, &power_color, &mut v);
    }
    if client.damage_armor != 0 {
        let scale = client.damage_armor as f32 / realcount;
        let tmp = v;
        vector_ma(&tmp, scale, &acolor, &mut v);
    }
    if client.damage_blood != 0 {
        let scale = client.damage_blood as f32 / realcount;
        let tmp = v;
        vector_ma(&tmp, scale, &bcolor, &mut v);
    }
    vector_copy(&v, &mut client.damage_blend);

    //
    // calculate view angle kicks
    //
    let kick_raw = client.damage_knockback.abs() as f32;
    if kick_raw != 0.0 && ent.health > 0 {
        let mut kick = kick_raw * 100.0 / ent.health as f32;

        if kick < count * 0.5 {
            kick = count * 0.5;
        }
        if kick > 50.0 {
            kick = 50.0;
        }

        vector_subtract(&client.damage_from, &ent.s.origin, &mut v);
        vector_normalize(&mut v);

        let side = dot_product(&v, &vctx.right);
        client.v_dmg_roll = kick * side * 0.3;

        let side = -dot_product(&v, &vctx.forward);
        client.v_dmg_pitch = kick * side * 0.3;

        client.v_dmg_time = level.time + DAMAGE_TIME;
    }

    //
    // clear totals
    //
    client.damage_blood = 0;
    client.damage_armor = 0;
    client.damage_parmor = 0;
    client.damage_knockback = 0;
}

// ============================================================
// SV_CalcViewOffset
// ============================================================

pub fn sv_calc_view_offset(
    ent: &mut Edict,
    client: &mut GClient,
    level: &LevelLocals,
    vctx: &ViewContext,
    cvars: &ViewCvars,
) {
    //===================================
    // base angles
    let mut angles: Vec3 = [0.0; 3];

    // if dead, fix the angle and don't add any kick
    if ent.deadflag != 0 {
        vector_clear(&mut angles);

        client.ps.viewangles[ROLL] = 40.0;
        client.ps.viewangles[PITCH] = -15.0;
        client.ps.viewangles[YAW] = client.killer_yaw;
    } else {
        // add angles based on weapon kick
        vector_copy(&client.kick_angles, &mut angles);

        // add angles based on damage kick
        let mut ratio = (client.v_dmg_time - level.time) / DAMAGE_TIME;
        if ratio < 0.0 {
            ratio = 0.0;
            client.v_dmg_pitch = 0.0;
            client.v_dmg_roll = 0.0;
        }
        angles[PITCH] += ratio * client.v_dmg_pitch;
        angles[ROLL] += ratio * client.v_dmg_roll;

        // add pitch based on fall kick
        let mut ratio = (client.fall_time - level.time) / FALL_TIME;
        if ratio < 0.0 {
            ratio = 0.0;
        }
        angles[PITCH] += ratio * client.fall_value;

        // add angles based on velocity
        let delta = dot_product(&ent.velocity, &vctx.forward);
        angles[PITCH] += delta * cvars.run_pitch.value;

        let delta = dot_product(&ent.velocity, &vctx.right);
        angles[ROLL] += delta * cvars.run_roll.value;

        // add angles based on bob
        let mut delta = vctx.bobfracsin * cvars.bob_pitch.value * vctx.xyspeed;
        if (client.ps.pmove.pm_flags & PMF_DUCKED) != 0 {
            delta *= 6.0; // crouching
        }
        angles[PITCH] += delta;
        let mut delta = vctx.bobfracsin * cvars.bob_roll.value * vctx.xyspeed;
        if (client.ps.pmove.pm_flags & PMF_DUCKED) != 0 {
            delta *= 6.0; // crouching
        }
        if (vctx.bobcycle & 1) != 0 {
            delta = -delta;
        }
        angles[ROLL] += delta;
    }

    // copy computed kick angles to player state
    vector_copy(&angles, &mut client.ps.kick_angles);

    //===================================
    // base origin
    let mut v: Vec3 = [0.0; 3];

    // add view height
    v[2] += ent.viewheight as f32;

    // add fall height
    let mut ratio = (client.fall_time - level.time) / FALL_TIME;
    if ratio < 0.0 {
        ratio = 0.0;
    }
    v[2] -= ratio * client.fall_value * 0.4;

    // add bob height
    let mut bob = vctx.bobfracsin * vctx.xyspeed * cvars.bob_up.value;
    if bob > 6.0 {
        bob = 6.0;
    }
    v[2] += bob;

    // add kick offset
    let kick = client.kick_origin;
    let tmp = v;
    vector_add(&tmp, &kick, &mut v);

    // absolutely bound offsets
    // so the view can never be outside the player box
    if v[0] < -14.0 {
        v[0] = -14.0;
    } else if v[0] > 14.0 {
        v[0] = 14.0;
    }
    if v[1] < -14.0 {
        v[1] = -14.0;
    } else if v[1] > 14.0 {
        v[1] = 14.0;
    }
    if v[2] < -22.0 {
        v[2] = -22.0;
    } else if v[2] > 30.0 {
        v[2] = 30.0;
    }

    vector_copy(&v, &mut client.ps.viewoffset);
}

// ============================================================
// SV_CalcGunOffset
// ============================================================

pub fn sv_calc_gun_offset(
    _ent: &Edict,
    client: &mut GClient,
    vctx: &ViewContext,
    cvars: &ViewCvars,
) {
    // gun angles from bobbing
    client.ps.gunangles[ROLL] = vctx.xyspeed * vctx.bobfracsin * 0.005;
    client.ps.gunangles[YAW] = vctx.xyspeed * vctx.bobfracsin * 0.01;
    if (vctx.bobcycle & 1) != 0 {
        client.ps.gunangles[ROLL] = -client.ps.gunangles[ROLL];
        client.ps.gunangles[YAW] = -client.ps.gunangles[YAW];
    }

    client.ps.gunangles[PITCH] = vctx.xyspeed * vctx.bobfracsin * 0.005;

    // gun angles from delta movement
    for i in 0..3 {
        let mut delta = client.oldviewangles[i] - client.ps.viewangles[i];
        if delta > 180.0 {
            delta -= 360.0;
        }
        if delta < -180.0 {
            delta += 360.0;
        }
        if delta > 45.0 {
            delta = 45.0;
        }
        if delta < -45.0 {
            delta = -45.0;
        }
        if i == YAW {
            client.ps.gunangles[ROLL] += 0.1 * delta;
        }
        client.ps.gunangles[i] += 0.2 * delta;
    }

    // gun height
    vector_clear(&mut client.ps.gunoffset);

    // gun_x / gun_y / gun_z are development tools
    for i in 0..3 {
        client.ps.gunoffset[i] += vctx.forward[i] * cvars.gun_y.value;
        client.ps.gunoffset[i] += vctx.right[i] * cvars.gun_x.value;
        client.ps.gunoffset[i] += vctx.up[i] * (-cvars.gun_z.value);
    }
}

// ============================================================
// SV_AddBlend
// ============================================================

pub fn sv_add_blend(r: f32, g: f32, b: f32, a: f32, v_blend: &mut [f32; 4]) {
    if a <= 0.0 {
        return;
    }
    let a2 = v_blend[3] + (1.0 - v_blend[3]) * a; // new total alpha
    let a3 = v_blend[3] / a2; // fraction of color from old

    v_blend[0] = v_blend[0] * a3 + r * (1.0 - a3);
    v_blend[1] = v_blend[1] * a3 + g * (1.0 - a3);
    v_blend[2] = v_blend[2] * a3 + b * (1.0 - a3);
    v_blend[3] = a2;
}

// ============================================================
// SV_CalcBlend
// ============================================================

pub fn sv_calc_blend(
    ent: &mut Edict,
    client: &mut GClient,
    level: &LevelLocals,
) {
    client.ps.blend[0] = 0.0;
    client.ps.blend[1] = 0.0;
    client.ps.blend[2] = 0.0;
    client.ps.blend[3] = 0.0;

    // add for contents
    let mut vieworg: Vec3 = [0.0; 3];
    vector_add(&ent.s.origin, &client.ps.viewoffset, &mut vieworg);
    // gi.pointcontents placeholder
    let contents = gi_pointcontents(&vieworg);
    if (contents & (CONTENTS_LAVA | CONTENTS_SLIME | CONTENTS_WATER)) != 0 {
        client.ps.rdflags |= RDF_UNDERWATER;
    } else {
        client.ps.rdflags &= !RDF_UNDERWATER;
    }

    if (contents & (CONTENTS_SOLID | CONTENTS_LAVA)) != 0 {
        sv_add_blend(1.0, 0.3, 0.0, 0.6, &mut client.ps.blend);
    } else if (contents & CONTENTS_SLIME) != 0 {
        sv_add_blend(0.0, 0.1, 0.05, 0.6, &mut client.ps.blend);
    } else if (contents & CONTENTS_WATER) != 0 {
        sv_add_blend(0.5, 0.3, 0.2, 0.4, &mut client.ps.blend);
    }

    // add for powerups
    if client.quad_framenum > level.framenum as f32 {
        let remaining = (client.quad_framenum - level.framenum as f32) as i32;
        if remaining == 30 {
            // beginning to fade
            gi_sound(ent.s.number, CHAN_ITEM, gi_soundindex("items/damage2.wav"), 1.0, ATTN_NORM as f32, 0.0);
        }
        if remaining > 30 || (remaining & 4) != 0 {
            sv_add_blend(0.0, 0.0, 1.0, 0.08, &mut client.ps.blend);
        }
    } else if client.invincible_framenum > level.framenum as f32 {
        let remaining = (client.invincible_framenum - level.framenum as f32) as i32;
        if remaining == 30 {
            gi_sound(ent.s.number, CHAN_ITEM, gi_soundindex("items/protect2.wav"), 1.0, ATTN_NORM as f32, 0.0);
        }
        if remaining > 30 || (remaining & 4) != 0 {
            sv_add_blend(1.0, 1.0, 0.0, 0.08, &mut client.ps.blend);
        }
    } else if client.enviro_framenum > level.framenum as f32 {
        let remaining = (client.enviro_framenum - level.framenum as f32) as i32;
        if remaining == 30 {
            gi_sound(ent.s.number, CHAN_ITEM, gi_soundindex("items/airout.wav"), 1.0, ATTN_NORM as f32, 0.0);
        }
        if remaining > 30 || (remaining & 4) != 0 {
            sv_add_blend(0.0, 1.0, 0.0, 0.08, &mut client.ps.blend);
        }
    } else if client.breather_framenum > level.framenum as f32 {
        let remaining = (client.breather_framenum - level.framenum as f32) as i32;
        if remaining == 30 {
            gi_sound(ent.s.number, CHAN_ITEM, gi_soundindex("items/airout.wav"), 1.0, ATTN_NORM as f32, 0.0);
        }
        if remaining > 30 || (remaining & 4) != 0 {
            sv_add_blend(0.4, 1.0, 0.4, 0.04, &mut client.ps.blend);
        }
    }

    // add for damage
    if client.damage_alpha > 0.0 {
        sv_add_blend(
            client.damage_blend[0],
            client.damage_blend[1],
            client.damage_blend[2],
            client.damage_alpha,
            &mut client.ps.blend,
        );
    }

    if client.bonus_alpha > 0.0 {
        sv_add_blend(0.85, 0.7, 0.3, client.bonus_alpha, &mut client.ps.blend);
    }

    // drop the damage value
    client.damage_alpha -= 0.06;
    if client.damage_alpha < 0.0 {
        client.damage_alpha = 0.0;
    }

    // drop the bonus value
    client.bonus_alpha -= 0.1;
    if client.bonus_alpha < 0.0 {
        client.bonus_alpha = 0.0;
    }
}

// ============================================================
// P_FallingDamage
// ============================================================

pub fn p_falling_damage(
    ent: &mut Edict,
    client: &mut GClient,
    level: &LevelLocals,
    cvars: &ViewCvars,
) {
    if ent.s.modelindex != 255 {
        return; // not in the player model
    }

    if ent.movetype == MoveType::Noclip {
        return;
    }

    let delta;
    if client.oldvelocity[2] < 0.0
        && ent.velocity[2] > client.oldvelocity[2]
        && ent.groundentity == -1
    {
        delta = client.oldvelocity[2];
    } else {
        if ent.groundentity == -1 {
            return;
        }
        delta = ent.velocity[2] - client.oldvelocity[2];
    }
    let mut delta = delta * delta * 0.0001;

    // never take falling damage if completely underwater
    if ent.waterlevel == 3 {
        return;
    }
    if ent.waterlevel == 2 {
        delta *= 0.25;
    }
    if ent.waterlevel == 1 {
        delta *= 0.5;
    }

    if delta < 1.0 {
        return;
    }

    if delta < 15.0 {
        ent.s.event = EntityEvent::Footstep as i32;
        return;
    }

    client.fall_value = delta * 0.5;
    if client.fall_value > 40.0 {
        client.fall_value = 40.0;
    }
    client.fall_time = level.time + FALL_TIME;

    if delta > 30.0 {
        if ent.health > 0 {
            if delta >= 55.0 {
                ent.s.event = EntityEvent::FallFar as i32;
            } else {
                ent.s.event = EntityEvent::Fall as i32;
            }
        }
        ent.pain_debounce_time = level.time; // no normal pain sound
        let mut damage = ((delta - 30.0) / 2.0) as i32;
        if damage < 1 {
            damage = 1;
        }
        let _dir: Vec3 = [0.0, 0.0, 1.0];

        if cvars.deathmatch.value == 0.0
            || !DmFlags::from_bits_truncate(cvars.dmflags.value as i32).intersects(DF_NO_FALLING)
        {
            // Apply falling damage directly: reduce health by damage amount.
            // In full engine this calls T_Damage(ent, world, world, dir, ent.s.origin, vec3_origin, damage, 0, 0, MOD_FALLING).
            // We apply the damage effect here since we have direct mut access to the entity.
            ent.health -= damage;
            client.damage_blood += damage;
            client.damage_knockback = 0;
            client.damage_from = ent.s.origin;
        }
    } else {
        ent.s.event = EntityEvent::FallShort as i32;
    }
}

// ============================================================
// P_WorldEffects
// ============================================================

pub fn p_world_effects(
    ent: &mut Edict,
    client: &mut GClient,
    level: &LevelLocals,
) {
    if ent.movetype == MoveType::Noclip {
        ent.air_finished = level.time + 12.0; // don't need air
        return;
    }

    let waterlevel = ent.waterlevel;
    let old_waterlevel = client.old_waterlevel;
    client.old_waterlevel = waterlevel;

    let breather = client.breather_framenum > level.framenum as f32;
    let envirosuit = client.enviro_framenum > level.framenum as f32;

    //
    // if just entered a water volume, play a sound
    //
    if old_waterlevel == 0 && waterlevel != 0 {
        // PlayerNoise is called from the game loop after this function returns.
        // The noise dispatch is handled at the ClientEndServerFrame level.
        if (ent.watertype & CONTENTS_LAVA) != 0 {
            gi_sound(ent.s.number, CHAN_BODY, gi_soundindex("player/lava_in.wav"), 1.0, ATTN_NORM as f32, 0.0);
        } else if (ent.watertype & CONTENTS_SLIME) != 0 {
            gi_sound(ent.s.number, CHAN_BODY, gi_soundindex("player/watr_in.wav"), 1.0, ATTN_NORM as f32, 0.0);
        } else if (ent.watertype & CONTENTS_WATER) != 0 {
            gi_sound(ent.s.number, CHAN_BODY, gi_soundindex("player/watr_in.wav"), 1.0, ATTN_NORM as f32, 0.0);
        }
        ent.flags |= FL_INWATER;

        // clear damage_debounce, so the pain sound will play immediately
        ent.damage_debounce_time = level.time - 1.0;
    }

    //
    // if just completely exited a water volume, play a sound
    //
    if old_waterlevel != 0 && waterlevel == 0 {
        // PlayerNoise is called from the game loop after this function returns.
        // The noise dispatch is handled at the ClientEndServerFrame level.
        gi_sound(ent.s.number, CHAN_BODY, gi_soundindex("player/watr_out.wav"), 1.0, ATTN_NORM as f32, 0.0);
        ent.flags &= !FL_INWATER;
    }

    //
    // check for head just going under water
    //
    if old_waterlevel != 3 && waterlevel == 3 {
        gi_sound(ent.s.number, CHAN_BODY, gi_soundindex("player/watr_un.wav"), 1.0, ATTN_NORM as f32, 0.0);
    }

    //
    // check for head just coming out of water
    //
    if old_waterlevel == 3 && waterlevel != 3 {
        if ent.air_finished < level.time {
            // gasp for air
            gi_sound(ent.s.number, CHAN_VOICE, gi_soundindex("player/gasp1.wav"), 1.0, ATTN_NORM as f32, 0.0);
            // PlayerNoise is called from the game loop after this function returns.
        // The noise dispatch is handled at the ClientEndServerFrame level.
        } else if ent.air_finished < level.time + 11.0 {
            // just break surface
            gi_sound(ent.s.number, CHAN_VOICE, gi_soundindex("player/gasp2.wav"), 1.0, ATTN_NORM as f32, 0.0);
        }
    }

    //
    // check for drowning
    //
    if waterlevel == 3 {
        // breather or envirosuit give air
        if breather || envirosuit {
            ent.air_finished = level.time + 10.0;

            if ((client.breather_framenum as i32 - level.framenum) % 25) == 0 {
                if client.breather_sound == 0 {
                    gi_sound(ent.s.number, CHAN_AUTO, gi_soundindex("player/u_breath1.wav"), 1.0, ATTN_NORM as f32, 0.0);
                } else {
                    gi_sound(ent.s.number, CHAN_AUTO, gi_soundindex("player/u_breath2.wav"), 1.0, ATTN_NORM as f32, 0.0);
                }
                client.breather_sound ^= 1;
                // PlayerNoise is called from the game loop after this function returns.
        // The noise dispatch is handled at the ClientEndServerFrame level.
            }
        }

        // if out of air, start drowning
        if ent.air_finished < level.time {
            // drown!
            if client.next_drown_time < level.time && ent.health > 0 {
                client.next_drown_time = level.time + 1.0;

                // take more damage the longer underwater
                ent.dmg += 2;
                if ent.dmg > 15 {
                    ent.dmg = 15;
                }

                // play a gurp sound instead of a normal pain sound
                if ent.health <= ent.dmg {
                    gi_sound(ent.s.number, CHAN_VOICE, gi_soundindex("player/drown1.wav"), 1.0, ATTN_NORM as f32, 0.0);
                } else if (rand_int() & 1) != 0 {
                    gi_sound(ent.s.number, CHAN_VOICE, gi_soundindex("*gurp1.wav"), 1.0, ATTN_NORM as f32, 0.0);
                } else {
                    gi_sound(ent.s.number, CHAN_VOICE, gi_soundindex("*gurp2.wav"), 1.0, ATTN_NORM as f32, 0.0);
                }

                ent.pain_debounce_time = level.time;

                // Apply drowning damage directly
                ent.health -= ent.dmg;
                client.damage_blood += ent.dmg;
                client.damage_knockback = 0;
                client.damage_from = ent.s.origin;
            }
        }
    } else {
        ent.air_finished = level.time + 12.0;
        ent.dmg = 2;
    }

    //
    // check for sizzle damage
    //
    if waterlevel != 0 && (ent.watertype & (CONTENTS_LAVA | CONTENTS_SLIME)) != 0 {
        if (ent.watertype & CONTENTS_LAVA) != 0 {
            if ent.health > 0
                && ent.pain_debounce_time <= level.time
                && client.invincible_framenum < level.framenum as f32
            {
                if (rand_int() & 1) != 0 {
                    gi_sound(ent.s.number, CHAN_VOICE, gi_soundindex("player/burn1.wav"), 1.0, ATTN_NORM as f32, 0.0);
                } else {
                    gi_sound(ent.s.number, CHAN_VOICE, gi_soundindex("player/burn2.wav"), 1.0, ATTN_NORM as f32, 0.0);
                }
                ent.pain_debounce_time = level.time + 1.0;
            }

            if envirosuit {
                // take 1/3 damage with envirosuit
                let lava_dmg = 1 * waterlevel;
                ent.health -= lava_dmg;
                client.damage_blood += lava_dmg;
                client.damage_knockback = 0;
                client.damage_from = ent.s.origin;
            } else {
                let lava_dmg = 3 * waterlevel;
                ent.health -= lava_dmg;
                client.damage_blood += lava_dmg;
                client.damage_knockback = 0;
                client.damage_from = ent.s.origin;
            }
        }

        if (ent.watertype & CONTENTS_SLIME) != 0
            && !envirosuit {
                // no damage from slime with envirosuit
                let slime_dmg = 1 * waterlevel;
                ent.health -= slime_dmg;
                client.damage_blood += slime_dmg;
                client.damage_knockback = 0;
                client.damage_from = ent.s.origin;
            }
    }
}

// ============================================================
// G_SetClientEffects
// ============================================================

pub fn g_set_client_effects(
    ent: &mut Edict,
    client: &GClient,
    level: &LevelLocals,
    items: &[GItem],
) {
    ent.s.effects = 0;
    ent.s.renderfx = 0;

    if ent.health <= 0 || level.intermissiontime != 0.0 {
        return;
    }

    if ent.powerarmor_time > level.time {
        let pa_type = power_armor_type(ent, client, items);
        if pa_type == POWER_ARMOR_SCREEN {
            ent.s.effects |= EF_POWERSCREEN;
        } else if pa_type == POWER_ARMOR_SHIELD {
            ent.s.effects |= EF_COLOR_SHELL;
            ent.s.renderfx |= RF_SHELL_GREEN;
        }
    }

    if client.quad_framenum > level.framenum as f32 {
        let remaining = (client.quad_framenum - level.framenum as f32) as i32;
        if remaining > 30 || (remaining & 4) != 0 {
            ent.s.effects |= EF_QUAD;
        }
    }

    if client.invincible_framenum > level.framenum as f32 {
        let remaining = (client.invincible_framenum - level.framenum as f32) as i32;
        if remaining > 30 || (remaining & 4) != 0 {
            ent.s.effects |= EF_PENT;
        }
    }

    // show cheaters!!!
    if ent.flags.intersects(FL_GODMODE) {
        ent.s.effects |= EF_COLOR_SHELL;
        ent.s.renderfx |= RF_SHELL_RED | RF_SHELL_GREEN | RF_SHELL_BLUE;
    }
}

// ============================================================
// G_SetClientEvent
// ============================================================

pub fn g_set_client_event(
    ent: &mut Edict,
    client: &GClient,
    vctx: &ViewContext,
) {
    if ent.s.event != 0 {
        return;
    }

    if ent.groundentity != -1 && vctx.xyspeed > 225.0
        && (client.bobtime + vctx.bobmove) as i32 != vctx.bobcycle {
            ent.s.event = EntityEvent::Footstep as i32;
        }
}

// ============================================================
// G_SetClientSound
// ============================================================

pub fn g_set_client_sound(
    ent: &mut Edict,
    client: &mut GClient,
    level: &LevelLocals,
    game: &GameLocals,
    items: &[GItem],
    snd_fry: i32,
) {
    if client.pers.game_helpchanged != game.helpchanged {
        client.pers.game_helpchanged = game.helpchanged;
        client.pers.helpchanged = 1;
    }

    // help beep (no more than three times)
    if client.pers.helpchanged != 0
        && client.pers.helpchanged <= 3
        && (level.framenum & 63) == 0
    {
        client.pers.helpchanged += 1;
        gi_sound(ent.s.number, CHAN_VOICE, gi_soundindex("misc/pc_up.wav"), 1.0, ATTN_STATIC as f32, 0.0);
    }

    let weap = if let Some(weapon_idx) = client.pers.weapon {
        items.get(weapon_idx).map(|item| item.classname.as_str()).unwrap_or("")
    } else {
        ""
    };

    if ent.waterlevel != 0 && (ent.watertype & (CONTENTS_LAVA | CONTENTS_SLIME)) != 0 {
        ent.s.sound = snd_fry;
    } else if weap == "weapon_railgun" {
        ent.s.sound = gi_soundindex("weapons/rg_hum.wav");
    } else if weap == "weapon_bfg" {
        ent.s.sound = gi_soundindex("weapons/bfg_hum.wav");
    } else if client.weapon_sound != 0 {
        ent.s.sound = client.weapon_sound;
    } else {
        ent.s.sound = 0;
    }
}

// ============================================================
// G_SetClientFrame
// ============================================================

pub fn g_set_client_frame(
    ent: &mut Edict,
    client: &mut GClient,
    vctx: &ViewContext,
) {
    if ent.s.modelindex != 255 {
        return; // not in the player model
    }

    let duck = (client.ps.pmove.pm_flags & PMF_DUCKED) != 0;
    let run = vctx.xyspeed != 0.0;

    // check for stand/duck and stop/go transitions
    let mut newanim = false;
    if duck != client.anim_duck && client.anim_priority < ANIM_DEATH {
        newanim = true;
    }
    if !newanim && run != client.anim_run && client.anim_priority == ANIM_BASIC {
        newanim = true;
    }
    if !newanim && ent.groundentity == -1 && client.anim_priority <= ANIM_WAVE {
        newanim = true;
    }

    if !newanim {
        if client.anim_priority == ANIM_REVERSE {
            if ent.s.frame > client.anim_end {
                ent.s.frame -= 1;
                return;
            }
        } else if ent.s.frame < client.anim_end {
            // continue an animation
            ent.s.frame += 1;
            return;
        }

        if client.anim_priority == ANIM_DEATH {
            return; // stay there
        }
        if client.anim_priority == ANIM_JUMP {
            if ent.groundentity == -1 {
                return; // stay there
            }
            client.anim_priority = ANIM_WAVE;
            ent.s.frame = FRAME_JUMP3;
            client.anim_end = FRAME_JUMP6;
            return;
        }
    }

    // newanim:
    // return to either a running or standing frame
    client.anim_priority = ANIM_BASIC;
    client.anim_duck = duck;
    client.anim_run = run;

    if ent.groundentity == -1 {
        client.anim_priority = ANIM_JUMP;
        if ent.s.frame != FRAME_JUMP2 {
            ent.s.frame = FRAME_JUMP1;
        }
        client.anim_end = FRAME_JUMP2;
    } else if run {
        // running
        if duck {
            ent.s.frame = FRAME_CRWALK1;
            client.anim_end = FRAME_CRWALK6;
        } else {
            ent.s.frame = FRAME_RUN1;
            client.anim_end = FRAME_RUN6;
        }
    } else {
        // standing
        if duck {
            ent.s.frame = FRAME_CRSTND01;
            client.anim_end = FRAME_CRSTND19;
        } else {
            ent.s.frame = FRAME_STAND01;
            client.anim_end = FRAME_STAND40;
        }
    }
}

// ============================================================
// ClientEndServerFrame
//
// Called for each player at the end of the server frame
// and right after spawning.
// ============================================================

pub fn client_end_server_frame(
    ent: &mut Edict,
    client: &mut GClient,
    level: &LevelLocals,
    game: &GameLocals,
    items: &[GItem],
    vctx: &mut ViewContext,
    cvars: &ViewCvars,
    snd_fry: i32,
) {
    //
    // If the origin or velocity have changed since ClientThink(),
    // update the pmove values. This will happen when the client
    // is pushed by a bmodel or kicked by an explosion.
    //
    for i in 0..3 {
        client.ps.pmove.origin[i] = (ent.s.origin[i] * 8.0) as i16;
        client.ps.pmove.velocity[i] = (ent.velocity[i] * 8.0) as i16;
    }

    //
    // If the end of unit layout is displayed, don't give
    // the player any normal movement attributes
    //
    if level.intermissiontime != 0.0 {
        // Intermission: clear view effects, freeze player view
        client.ps.blend[3] = 0.0;
        client.ps.fov = 90.0;
        // G_SetStats is called from p_hud module via the game loop.
        // At intermission, stats are not updated — the HUD is frozen.
        return;
    }

    angle_vectors(
        &client.v_angle,
        Some(&mut vctx.forward),
        Some(&mut vctx.right),
        Some(&mut vctx.up),
    );

    // burn from lava, etc
    p_world_effects(ent, client, level);

    //
    // set model angles from view angles so other things in
    // the world can tell which direction you are looking
    //
    if client.v_angle[PITCH] > 180.0 {
        ent.s.angles[PITCH] = (-360.0 + client.v_angle[PITCH]) / 3.0;
    } else {
        ent.s.angles[PITCH] = client.v_angle[PITCH] / 3.0;
    }
    ent.s.angles[YAW] = client.v_angle[YAW];
    ent.s.angles[ROLL] = 0.0;
    ent.s.angles[ROLL] = sv_calc_roll(&ent.s.angles, &ent.velocity, &vctx.right, cvars) * 4.0;

    //
    // calculate speed and cycle to be used for
    // all cyclic walking effects
    //
    vctx.xyspeed =
        (ent.velocity[0] * ent.velocity[0] + ent.velocity[1] * ent.velocity[1]).sqrt();

    if vctx.xyspeed < 5.0 {
        vctx.bobmove = 0.0;
        client.bobtime = 0.0; // start at beginning of cycle again
    } else if ent.groundentity != -1 {
        // so bobbing only cycles when on ground
        if vctx.xyspeed > 210.0 {
            vctx.bobmove = 0.25;
        } else if vctx.xyspeed > 100.0 {
            vctx.bobmove = 0.125;
        } else {
            vctx.bobmove = 0.0625;
        }
    }

    client.bobtime += vctx.bobmove;
    let mut bobtime = client.bobtime;

    if (client.ps.pmove.pm_flags & PMF_DUCKED) != 0 {
        bobtime *= 4.0;
    }

    vctx.bobcycle = bobtime as i32;
    vctx.bobfracsin = (bobtime * std::f32::consts::PI).sin().abs();

    // detect hitting the floor
    p_falling_damage(ent, client, level, cvars);

    // apply all the damage taken this frame
    p_damage_feedback(ent, client, level, vctx);

    // determine the view offsets
    sv_calc_view_offset(ent, client, level, vctx, cvars);

    // determine the gun offsets
    sv_calc_gun_offset(ent, client, vctx, cvars);

    // Determine the full screen color blend.
    // Must be after viewoffset, so eye contents can be accurately determined.
    // Note: With client prediction enabled, clients may also calculate this locally.
    sv_calc_blend(ent, client, level);

    // chase cam stuff
    // NOTE: G_SetSpectatorStats, G_SetStats, and G_CheckChaseStats are called
    // from the game dispatch layer (dispatch.rs) after client_end_server_frame,
    // because they require the full GameContext (p_hud::g_set_stats etc.).
    // In C these were inline calls since all state was global.

    g_set_client_event(ent, client, vctx);

    g_set_client_effects(ent, client, level, items);

    g_set_client_sound(ent, client, level, game, items, snd_fry);

    g_set_client_frame(ent, client, vctx);

    vector_copy(&ent.velocity, &mut client.oldvelocity);
    vector_copy(&client.ps.viewangles, &mut client.oldviewangles);

    // clear weapon kicks
    vector_clear(&mut client.kick_origin);
    vector_clear(&mut client.kick_angles);

    // if the scoreboard is up, update it
    if client.showscores && (level.framenum & 31) == 0 {
        // NOTE: DeathmatchScoreboardMessage is called from the game dispatch layer
        // after client_end_server_frame, because it requires the full GameContext
        // (p_hud::deathmatch_scoreboard_message). The gi_unicast is also deferred.
    }
}

// ============================================================
// Placeholder functions for cross-module calls
// ============================================================

/// PowerArmorType — checks what type of power armor the entity has equipped.
fn power_armor_type(ent: &Edict, client: &GClient, items: &[GItem]) -> i32 {
    if !ent.flags.intersects(FL_POWER_ARMOR) {
        return POWER_ARMOR_NONE;
    }

    // Check for power screen
    for (i, item) in items.iter().enumerate() {
        if item.pickup_name.eq_ignore_ascii_case("Power Screen") {
            if client.pers.inventory[i] > 0 {
                return POWER_ARMOR_SCREEN;
            }
        }
    }

    // Check for power shield
    for (i, item) in items.iter().enumerate() {
        if item.pickup_name.eq_ignore_ascii_case("Power Shield") {
            if client.pers.inventory[i] > 0 {
                return POWER_ARMOR_SHIELD;
            }
        }
    }

    POWER_ARMOR_NONE
}

use myq2_common::common::rand_i32 as rand_int;

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sv_add_blend() {
        let mut blend = [0.0f32; 4];
        sv_add_blend(1.0, 0.0, 0.0, 0.5, &mut blend);
        assert!((blend[0] - 1.0).abs() < 0.001);
        assert!((blend[1] - 0.0).abs() < 0.001);
        assert!((blend[2] - 0.0).abs() < 0.001);
        assert!((blend[3] - 0.5).abs() < 0.001);

        // blend another color on top
        sv_add_blend(0.0, 1.0, 0.0, 0.5, &mut blend);
        assert!(blend[3] > 0.5);
    }

    #[test]
    fn test_sv_calc_roll() {
        let cvars = ViewCvars::default();
        let angles: Vec3 = [0.0; 3];
        let velocity: Vec3 = [100.0, 0.0, 0.0];
        let right: Vec3 = [1.0, 0.0, 0.0];
        let roll = sv_calc_roll(&angles, &velocity, &right, &cvars);
        assert!(roll > 0.0);
    }

    #[test]
    fn test_vector_normalize() {
        let mut v: Vec3 = [3.0, 4.0, 0.0];
        let len = vector_normalize(&mut v);
        assert!((len - 5.0).abs() < 0.001);
        assert!((v[0] - 0.6).abs() < 0.001);
        assert!((v[1] - 0.8).abs() < 0.001);
    }
}
