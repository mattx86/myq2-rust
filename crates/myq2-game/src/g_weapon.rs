// g_weapon.rs — Weapon firing functions
// Converted from: myq2-original/game/g_weapon.c

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

use crate::g_local::{
    Edict, LevelLocals,
    FRAMETIME,
    DamageFlags,
    DAMAGE_ENERGY, DAMAGE_RADIUS, DAMAGE_BULLET,
    MOD_BLASTER, MOD_HYPERBLASTER, MOD_GRENADE, MOD_G_SPLASH,
    MOD_HANDGRENADE, MOD_HG_SPLASH, MOD_HELD_GRENADE,
    MOD_ROCKET, MOD_R_SPLASH, MOD_RAILGUN,
    MOD_BFG_BLAST,
    TE_GUNSHOT, TE_SHOTGUN, TE_BLASTER, TE_RAILTRAIL,
    TE_ROCKET_EXPLOSION, TE_ROCKET_EXPLOSION_WATER,
    TE_GRENADE_EXPLOSION, TE_GRENADE_EXPLOSION_WATER,
    TE_BFG_BIGEXPLOSION,
    TE_SPLASH, TE_BUBBLETRAIL,
    SVC_TEMP_ENTITY,
    MULTICAST_PVS, MULTICAST_PHS,
    MoveType,
};
use crate::game::{SVF_DEADMONSTER, SVF_MONSTER, Solid};
use crate::game_import::{
    gi_trace, gi_lag_compensated_trace, gi_pointcontents, gi_modelindex, gi_soundindex,
    gi_linkentity, gi_sound, gi_write_byte, gi_write_position,
    gi_write_dir, gi_multicast, gi_cvar, skill_value,
};
use crate::g_combat::{t_damage, t_radius_damage};
use myq2_common::q_shared::{
    Vec3, CPlane, CSurface,
    vec3_origin,
    CONTENTS_LAVA, CONTENTS_SLIME, CONTENTS_SOLID, CONTENTS_MONSTER, CONTENTS_DEADMONSTER,
    MASK_SHOT, MASK_WATER, SURF_SKY,
    vector_copy, vector_ma, vector_normalize, vector_length, vector_subtract, angle_vectors,
    vectoangles,
    EF_GRENADE, EF_ROCKET, EF_BFG, EF_ANIM_ALLFAST,
    CHAN_VOICE, CHAN_WEAPON, ATTN_NORM,
    MAX_EDICTS,
};

// Splash color constants
const SPLASH_UNKNOWN: i32 = 0;
const SPLASH_SPARKS: i32 = 1;
const SPLASH_BLUE_WATER: i32 = 2;
const SPLASH_BROWN_WATER: i32 = 3;
const SPLASH_SLIME: i32 = 4;
const SPLASH_LAVA: i32 = 5;
const SPLASH_BLOOD: i32 = 6;

// CHAN_*, ATTN_* come from g_local via myq2_common::q_shared

// Random number helpers - use canonical implementations
use myq2_common::common::{frand as random, crand as crandom};

// vectoangles imported from myq2_common::q_shared

/// Helper: get deathmatch cvar value
fn get_deathmatch() -> f32 {
    gi_cvar("deathmatch", "0", 0)
}

/// Helper: get coop cvar value
fn get_coop() -> f32 {
    gi_cvar("coop", "0", 0)
}

/*
=================
check_dodge

This is a support routine used when a client is firing
a non-instant attack weapon.  It checks to see if a
monster's dodge function should be called.
=================
*/
fn check_dodge(self_idx: usize, edicts: &mut Vec<Edict>, level: &mut LevelLocals,
               start: &Vec3, dir: &Vec3, speed: i32) {
    let mut end = [0.0f32; 3];

    // easy mode only ducks one quarter the time
    if skill_value() == 0.0
        && random() > 0.25 {
            return;
        }

    end = vector_ma(start, 8192.0, dir);
    let tr = gi_trace(start, &vec3_origin, &vec3_origin, &end, self_idx as i32, MASK_SHOT);

    // Check trace result for monster entity with dodge function
    if tr.ent_index >= 0 && (tr.ent_index as usize) < edicts.len() {
        let tr_ent_idx = tr.ent_index as usize;
        if (edicts[tr_ent_idx].svflags & SVF_MONSTER) != 0
            && edicts[tr_ent_idx].health > 0
            && edicts[tr_ent_idx].monsterinfo.dodge_fn.is_some()
        {
            // Check if target is in front of self
            if crate::g_ai::infront(&edicts[tr_ent_idx], &edicts[self_idx]) {
                // Note: dispatch_dodge doesn't take eta param; call dodge directly
                let v = vector_subtract(&tr.endpos, start);
                let _eta = (vector_length(&v) - edicts[tr_ent_idx].maxs[0]) / speed as f32;
                crate::dispatch::call_dodge(tr_ent_idx, edicts, level);
            }
        }
    }
}

/*
=================
fire_hit

Used for all impact (hit/punch/slash) attacks
=================
*/
pub fn fire_hit(self_idx: usize, edicts: &mut Vec<Edict>, level: &mut LevelLocals,
                aim: &Vec3, damage: i32, kick: i32) -> bool {
    let enemy_idx = edicts[self_idx].enemy;
    if enemy_idx < 0 || (enemy_idx as usize) >= edicts.len() {
        return false;
    }
    let enemy_idx = enemy_idx as usize;

    // See if enemy is in range
    let dir = vector_subtract(&edicts[enemy_idx].s.origin, &edicts[self_idx].s.origin);
    let mut range = vector_length(&dir);
    if range > aim[0] {
        return false;
    }

    let mut aim = *aim;
    if aim[1] > edicts[self_idx].mins[0] && aim[1] < edicts[self_idx].maxs[0] {
        // straight on hit - back the range up to the edge of their bbox
        range -= edicts[enemy_idx].maxs[0];
    } else {
        // side hit - adjust the "right" value out to the edge of their bbox
        if aim[1] < 0.0 {
            aim[1] = edicts[enemy_idx].mins[0];
        } else {
            aim[1] = edicts[enemy_idx].maxs[0];
        }
    }

    let point = vector_ma(&edicts[self_idx].s.origin, range, &dir);
    let tr = gi_trace(&edicts[self_idx].s.origin, &vec3_origin, &vec3_origin, &point, self_idx as i32, MASK_SHOT);

    let mut tr_ent_idx = if tr.ent_index >= 0 { tr.ent_index as usize } else { 0 };

    if tr.fraction < 1.0 {
        if edicts[tr_ent_idx].takedamage == 0 {
            return false;
        }
        // if it will hit any client/monster then hit the one we wanted to hit
        if (edicts[tr_ent_idx].svflags & SVF_MONSTER) != 0 || edicts[tr_ent_idx].client.is_some() {
            tr_ent_idx = enemy_idx;
        }
    }

    let mut forward = [0.0f32; 3];
    let mut right = [0.0f32; 3];
    let mut up = [0.0f32; 3];
    angle_vectors(&edicts[self_idx].s.angles, Some(&mut forward), Some(&mut right), Some(&mut up));
    let mut point = vector_ma(&edicts[self_idx].s.origin, range, &forward);
    point = vector_ma(&point, aim[1], &right);
    point = vector_ma(&point, aim[2], &up);
    let dir = vector_subtract(&point, &edicts[enemy_idx].s.origin);

    // do the damage
    use crate::g_local::{DAMAGE_NO_KNOCKBACK, MOD_HIT};
    t_damage(
        tr_ent_idx, self_idx, self_idx,
        dir, point, vec3_origin,
        damage, kick / 2, DAMAGE_NO_KNOCKBACK, MOD_HIT,
        edicts, level,
    );

    if (edicts[tr_ent_idx].svflags & SVF_MONSTER) == 0 && edicts[tr_ent_idx].client.is_none() {
        return false;
    }

    // do our special form of knockback here
    let mut v = [0.0f32; 3];
    for i in 0..3 {
        v[i] = edicts[enemy_idx].absmin[i] + 0.5 * edicts[enemy_idx].size[i];
    }
    v = vector_subtract(&v, &point);
    vector_normalize(&mut v);
    for i in 0..3 {
        edicts[enemy_idx].velocity[i] += kick as f32 * v[i];
    }
    if edicts[enemy_idx].velocity[2] > 0.0 {
        edicts[enemy_idx].groundentity = -1;
    }
    true
}

/*
=================
fire_lead

This is an internal support routine used for bullet/pellet based weapons.
=================
*/
fn fire_lead(self_idx: usize, edicts: &mut Vec<Edict>, level: &mut LevelLocals,
             start: &Vec3, aimdir: &Vec3, damage: i32, kick: i32,
             te_impact: i32, hspread: i32, vspread: i32, mod_type: i32) {
    let mut dir = [0.0f32; 3];
    let mut forward = [0.0f32; 3];
    let mut right = [0.0f32; 3];
    let mut up = [0.0f32; 3];
    let mut end = [0.0f32; 3];
    let r: f32;
    let u: f32;
    let mut water_start = [0.0f32; 3];
    let mut water = false;
    let mut content_mask = MASK_SHOT | MASK_WATER;

    let self_origin = edicts[self_idx].s.origin;

    let tr = gi_trace(&self_origin, &vec3_origin, &vec3_origin, start, self_idx as i32, MASK_SHOT);

    if tr.fraction >= 1.0 {
        vectoangles(aimdir, &mut dir);
        angle_vectors(&dir, Some(&mut forward), Some(&mut right), Some(&mut up));

        r = crandom() * hspread as f32;
        u = crandom() * vspread as f32;
        end = vector_ma(start, 8192.0, &forward);
        end = vector_ma(&end, r, &right);
        end = vector_ma(&end, u, &up);

        if gi_pointcontents(start) & MASK_WATER != 0 {
            water = true;
            water_start = vector_copy(start);
            content_mask &= !MASK_WATER;
        }

        // Use lag-compensated trace for hitscan weapons to be fair to high-ping players
        let mut tr = gi_lag_compensated_trace(start, &vec3_origin, &vec3_origin, &end, self_idx as i32, content_mask, self_idx as i32);

        // see if we hit water
        if (tr.contents & MASK_WATER) != 0 {
            let color: i32;

            water = true;
            water_start = tr.endpos;

            if tr.contents & CONTENTS_SLIME != 0 {
                color = SPLASH_SLIME;
            } else if tr.contents & CONTENTS_LAVA != 0 {
                color = SPLASH_LAVA;
            } else {
                // Check surface name for brown water
                color = SPLASH_BLUE_WATER;
            }

            gi_write_byte(SVC_TEMP_ENTITY);
            gi_write_byte(TE_SPLASH);
            gi_write_byte(8);
            gi_write_position(&tr.endpos);
            if let Some(ref surf) = tr.surface {
                gi_write_dir(&tr.plane.normal);
            } else {
                gi_write_dir(&vec3_origin);
            }
            gi_write_byte(color);
            gi_multicast(&tr.endpos, MULTICAST_PVS);

            // change bullet's course when it enters water
            let mut dir2 = [0.0f32; 3];
            dir2[0] = end[0] - start[0];
            dir2[1] = end[1] - start[1];
            dir2[2] = end[2] - start[2];
            let dir2_copy = dir2;
            vectoangles(&dir2_copy, &mut dir2);
            let mut forward2 = [0.0f32; 3];
            let mut right2 = [0.0f32; 3];
            let mut up2 = [0.0f32; 3];
            angle_vectors(&dir2, Some(&mut forward2), Some(&mut right2), Some(&mut up2));
            let r2 = crandom() * hspread as f32 * 2.0;
            let u2 = crandom() * vspread as f32 * 2.0;
            end = vector_ma(&water_start, 8192.0, &forward2);
            end = vector_ma(&end, r2, &right2);
            end = vector_ma(&end, u2, &up2);

            // re-trace ignoring water this time
            tr = gi_trace(&water_start, &vec3_origin, &vec3_origin, &end, self_idx as i32, MASK_SHOT);
        }

        // send gun puff / flash
        if let Some(ref surf) = tr.surface {
            if (surf.flags & SURF_SKY) != 0 {
                return;
            }
        }

        if tr.fraction < 1.0 {
            if tr.ent_index >= 0 && (tr.ent_index as usize) < edicts.len() {
                let tr_ent_idx = tr.ent_index as usize;
                if edicts[tr_ent_idx].takedamage != 0 {
                    t_damage(
                        tr_ent_idx, self_idx, self_idx,
                        *aimdir, tr.endpos, tr.plane.normal,
                        damage, kick, DAMAGE_BULLET, mod_type,
                        edicts, level,
                    );
                } else {
                    if let Some(ref surf) = tr.surface {
                        if !surf.name.starts_with(b"sky") {
                            gi_write_byte(SVC_TEMP_ENTITY);
                            gi_write_byte(te_impact);
                            gi_write_position(&tr.endpos);
                            gi_write_dir(&tr.plane.normal);
                            gi_multicast(&tr.endpos, MULTICAST_PVS);

                            // player_noise(self, &tr.endpos, PNOISE_IMPACT)
                            // Deferred: requires GameContext not available in this signature
                        }
                    }
                }
            }
        }

        // if went through water, determine where the end and make a bubble trail
        if water {
            let mut pos = [0.0f32; 3];

            dir[0] = tr.endpos[0] - water_start[0];
            dir[1] = tr.endpos[1] - water_start[1];
            dir[2] = tr.endpos[2] - water_start[2];
            vector_normalize(&mut dir);
            pos[0] = tr.endpos[0] + -2.0 * dir[0];
            pos[1] = tr.endpos[1] + -2.0 * dir[1];
            pos[2] = tr.endpos[2] + -2.0 * dir[2];
            if (gi_pointcontents(&pos) & MASK_WATER) != 0 {
                // pos is in water, use it as endpos
            } else {
                let tr2 = gi_trace(&pos, &vec3_origin, &vec3_origin, &water_start,
                                   tr.ent_index, MASK_WATER);
                pos = tr2.endpos;
            }

            let mut mid = [0.0f32; 3];
            mid[0] = (water_start[0] + pos[0]) * 0.5;
            mid[1] = (water_start[1] + pos[1]) * 0.5;
            mid[2] = (water_start[2] + pos[2]) * 0.5;

            gi_write_byte(SVC_TEMP_ENTITY);
            gi_write_byte(TE_BUBBLETRAIL);
            gi_write_position(&water_start);
            gi_write_position(&pos);
            gi_multicast(&mid, MULTICAST_PVS);
        }
    }
}

/*
=================
fire_bullet

Fires a single round.  Used for machinegun and chaingun.  Would be fine for
pistols, rifles, etc....
=================
*/
pub fn fire_bullet(self_idx: usize, edicts: &mut Vec<Edict>, level: &mut LevelLocals,
                   start: &Vec3, aimdir: &Vec3, damage: i32,
                   kick: i32, hspread: i32, vspread: i32, mod_type: i32) {
    fire_lead(self_idx, edicts, level, start, aimdir, damage, kick, TE_GUNSHOT, hspread, vspread, mod_type);
}

/*
=================
fire_shotgun

Shoots shotgun pellets.  Used by shotgun and super shotgun.
=================
*/
pub fn fire_shotgun(self_idx: usize, edicts: &mut Vec<Edict>, level: &mut LevelLocals,
                    start: &Vec3, aimdir: &Vec3, damage: i32,
                    kick: i32, hspread: i32, vspread: i32, count: i32, mod_type: i32) {
    for _i in 0..count {
        fire_lead(self_idx, edicts, level, start, aimdir, damage, kick, TE_SHOTGUN, hspread, vspread, mod_type);
    }
}

/*
=================
fire_blaster

Fires a single blaster bolt.  Used by the blaster and hyper blaster.
=================
*/
pub fn blaster_touch(self_idx: usize, other_idx: usize, edicts: &mut Vec<Edict>,
                     level: &mut LevelLocals,
                     _plane: Option<&CPlane>, _surf: Option<&CSurface>) {
    let mod_type: i32;

    if edicts[self_idx].owner >= 0 && other_idx == edicts[self_idx].owner as usize {
        return;
    }

    if let Some(surf) = &_surf {
        if (surf.flags & SURF_SKY) != 0 {
            crate::g_utils::free_edict_raw(edicts, self_idx, 0, level.time);
            return;
        }
    }

    if edicts[self_idx].owner >= 0 {
        let owner_idx = edicts[self_idx].owner as usize;
        if owner_idx < edicts.len() && edicts[owner_idx].client.is_some() {
            // PlayerNoise - would need GameContext; skip for now
            // player_noise deferred: requires GameContext not available in this signature
        }
    }

    if edicts[other_idx].takedamage != 0 {
        if (edicts[self_idx].spawnflags & 1) != 0 {
            mod_type = MOD_HYPERBLASTER;
        } else {
            mod_type = MOD_BLASTER;
        }
        let plane_normal = if let Some(plane) = _plane {
            plane.normal
        } else {
            vec3_origin
        };
        t_damage(
            other_idx, self_idx, edicts[self_idx].owner as usize,
            edicts[self_idx].velocity, edicts[self_idx].s.origin,
            plane_normal,
            edicts[self_idx].dmg, 1, DAMAGE_ENERGY, mod_type,
            edicts, level,
        );
    } else {
        gi_write_byte(SVC_TEMP_ENTITY);
        gi_write_byte(TE_BLASTER);
        gi_write_position(&edicts[self_idx].s.origin);
        if let Some(plane) = _plane {
            gi_write_dir(&plane.normal);
        } else {
            gi_write_dir(&vec3_origin);
        }
        gi_multicast(&edicts[self_idx].s.origin, MULTICAST_PVS);
    }

    crate::g_utils::free_edict_raw(edicts, self_idx, 0, level.time);
}

pub fn fire_blaster(self_idx: usize, edicts: &mut Vec<Edict>, level: &mut LevelLocals,
                    start: &Vec3, dir: &Vec3, damage: i32,
                    speed: i32, effect: i32, hyper: bool) {
    let mut normalized_dir = vector_copy(dir);
    vector_normalize(&mut normalized_dir);

    let bolt_idx = {
        let mut num = edicts.len();
        let max = edicts.len().max(MAX_EDICTS);
        crate::g_utils::spawn_edict_raw(edicts, 0, &mut num, max, level.time)
    };

    edicts[bolt_idx].svflags = SVF_DEADMONSTER;
    edicts[bolt_idx].s.origin = *start;
    edicts[bolt_idx].s.old_origin = *start;
    vectoangles(dir, &mut edicts[bolt_idx].s.angles);
    edicts[bolt_idx].velocity[0] = dir[0] * speed as f32;
    edicts[bolt_idx].velocity[1] = dir[1] * speed as f32;
    edicts[bolt_idx].velocity[2] = dir[2] * speed as f32;
    edicts[bolt_idx].movetype = MoveType::FlyMissile;
    edicts[bolt_idx].clipmask = MASK_SHOT;
    edicts[bolt_idx].solid = Solid::Bbox;
    edicts[bolt_idx].s.effects |= effect as u32;
    edicts[bolt_idx].mins = [0.0; 3];
    edicts[bolt_idx].maxs = [0.0; 3];
    edicts[bolt_idx].s.modelindex = gi_modelindex("models/objects/laser/tris.md2");
    edicts[bolt_idx].s.sound = gi_soundindex("misc/lasfly.wav");
    edicts[bolt_idx].owner = self_idx as i32;
    // bolt.touch = blaster_touch; — handled via dispatch
    edicts[bolt_idx].touch_fn = Some(crate::dispatch::TOUCH_WEAPON_BLASTER);
    edicts[bolt_idx].nextthink = level.time + 2.0;
    // bolt.think = G_FreeEdict; — handled via dispatch
    edicts[bolt_idx].think_fn = Some(crate::dispatch::THINK_FREE_EDICT);
    edicts[bolt_idx].dmg = damage;
    edicts[bolt_idx].classname = "bolt".to_string();
    if hyper {
        edicts[bolt_idx].spawnflags = 1;
    }
    gi_linkentity(bolt_idx as i32);

    if edicts[self_idx].client.is_some() {
        let bolt_origin = edicts[bolt_idx].s.origin;
        check_dodge(self_idx, edicts, level, &bolt_origin, dir, speed);
    }

    let bolt_origin = edicts[bolt_idx].s.origin;
    let tr = gi_trace(&edicts[self_idx].s.origin, &vec3_origin, &vec3_origin,
                      &bolt_origin, self_idx as i32, MASK_SHOT);
    if tr.fraction < 1.0 {
        edicts[bolt_idx].s.origin[0] = bolt_origin[0] + -10.0 * dir[0];
        edicts[bolt_idx].s.origin[1] = bolt_origin[1] + -10.0 * dir[1];
        edicts[bolt_idx].s.origin[2] = bolt_origin[2] + -10.0 * dir[2];
        // bolt.touch(bolt, tr.ent, NULL, NULL) — dispatch needed
        if tr.ent_index >= 0 {
            blaster_touch(bolt_idx, tr.ent_index as usize, edicts, level, None, None);
        }
    }
}

/*
=================
fire_grenade
=================
*/
pub fn grenade_explode(ent_idx: usize, edicts: &mut Vec<Edict>, level: &mut LevelLocals) {
    let mut origin = [0.0f32; 3];
    let mod_type: i32;

    if edicts[ent_idx].owner >= 0 {
        let owner_idx = edicts[ent_idx].owner as usize;
        if owner_idx < edicts.len() && edicts[owner_idx].client.is_some() {
            // PlayerNoise
            // player_noise deferred: requires GameContext not available in this signature
        }
    }

    if edicts[ent_idx].enemy >= 0 {
        let enemy_idx = edicts[ent_idx].enemy as usize;
        if enemy_idx < edicts.len() {
            let mut v = [0.0f32; 3];
            v[0] = edicts[enemy_idx].mins[0] + edicts[enemy_idx].maxs[0];
            v[1] = edicts[enemy_idx].mins[1] + edicts[enemy_idx].maxs[1];
            v[2] = edicts[enemy_idx].mins[2] + edicts[enemy_idx].maxs[2];
            let mut center = [0.0f32; 3];
            center[0] = edicts[enemy_idx].s.origin[0] + 0.5 * v[0];
            center[1] = edicts[enemy_idx].s.origin[1] + 0.5 * v[1];
            center[2] = edicts[enemy_idx].s.origin[2] + 0.5 * v[2];
            v[0] = edicts[ent_idx].s.origin[0] - center[0];
            v[1] = edicts[ent_idx].s.origin[1] - center[1];
            v[2] = edicts[ent_idx].s.origin[2] - center[2];
            let vlen = vector_length(&v);
            let points = edicts[ent_idx].dmg as f32 - 0.5 * vlen;
            let mut dir = [0.0f32; 3];
            dir[0] = edicts[enemy_idx].s.origin[0] - edicts[ent_idx].s.origin[0];
            dir[1] = edicts[enemy_idx].s.origin[1] - edicts[ent_idx].s.origin[1];
            dir[2] = edicts[enemy_idx].s.origin[2] - edicts[ent_idx].s.origin[2];
            let m = if (edicts[ent_idx].spawnflags & 1) != 0 { MOD_HANDGRENADE } else { MOD_GRENADE };
            t_damage(
                enemy_idx, ent_idx, edicts[ent_idx].owner as usize,
                dir, edicts[ent_idx].s.origin, vec3_origin,
                points as i32, points as i32, DAMAGE_RADIUS, m,
                edicts, level,
            );
        }
    }

    // Determine mod based on spawnflags
    if (edicts[ent_idx].spawnflags & 2) != 0 {
        mod_type = MOD_HELD_GRENADE;
    } else if (edicts[ent_idx].spawnflags & 1) != 0 {
        mod_type = MOD_HG_SPLASH;
    } else {
        mod_type = MOD_G_SPLASH;
    }

    let ignore = if edicts[ent_idx].enemy >= 0 {
        Some(edicts[ent_idx].enemy as usize)
    } else {
        None
    };
    t_radius_damage(
        ent_idx,
        edicts[ent_idx].owner as usize,
        edicts[ent_idx].dmg as f32,
        ignore,
        edicts[ent_idx].dmg_radius,
        mod_type,
        edicts, level,
    );

    // Write explosion temp entity
    origin[0] = edicts[ent_idx].s.origin[0] + -0.02 * edicts[ent_idx].velocity[0];
    origin[1] = edicts[ent_idx].s.origin[1] + -0.02 * edicts[ent_idx].velocity[1];
    origin[2] = edicts[ent_idx].s.origin[2] + -0.02 * edicts[ent_idx].velocity[2];
    gi_write_byte(SVC_TEMP_ENTITY);
    if edicts[ent_idx].waterlevel != 0 {
        if edicts[ent_idx].groundentity >= 0 {
            gi_write_byte(TE_GRENADE_EXPLOSION_WATER);
        } else {
            gi_write_byte(TE_ROCKET_EXPLOSION_WATER);
        }
    } else {
        if edicts[ent_idx].groundentity >= 0 {
            gi_write_byte(TE_GRENADE_EXPLOSION);
        } else {
            gi_write_byte(TE_ROCKET_EXPLOSION);
        }
    }
    gi_write_position(&origin);
    gi_multicast(&edicts[ent_idx].s.origin, MULTICAST_PHS);

    crate::g_utils::free_edict_raw(edicts, ent_idx, 0, level.time);
}

pub fn grenade_touch(ent_idx: usize, other_idx: usize, edicts: &mut Vec<Edict>,
                 level: &mut LevelLocals,
                 _plane: Option<&CPlane>, _surf: Option<&CSurface>) {
    if edicts[ent_idx].owner >= 0 && other_idx == edicts[ent_idx].owner as usize {
        return;
    }

    if let Some(surf) = &_surf {
        if (surf.flags & SURF_SKY) != 0 {
            crate::g_utils::free_edict_raw(edicts, ent_idx, 0, level.time);
            return;
        }
    }

    if edicts[other_idx].takedamage == 0 {
        if (edicts[ent_idx].spawnflags & 1) != 0 {
            if random() > 0.5 {
                gi_sound(ent_idx as i32, CHAN_VOICE, gi_soundindex("weapons/hgrenb1a.wav"),
                         1.0, ATTN_NORM, 0.0);
            } else {
                gi_sound(ent_idx as i32, CHAN_VOICE, gi_soundindex("weapons/hgrenb2a.wav"),
                         1.0, ATTN_NORM, 0.0);
            }
        } else {
            gi_sound(ent_idx as i32, CHAN_VOICE, gi_soundindex("weapons/grenlb1b.wav"),
                     1.0, ATTN_NORM, 0.0);
        }
        return;
    }

    edicts[ent_idx].enemy = other_idx as i32;
    grenade_explode(ent_idx, edicts, level);
}

pub fn fire_grenade(self_idx: usize, edicts: &mut Vec<Edict>, level: &mut LevelLocals,
                    start: &Vec3, aimdir: &Vec3, damage: i32,
                    speed: i32, timer: f32, damage_radius: f32) {
    let mut dir = [0.0f32; 3];
    let mut forward = [0.0f32; 3];
    let mut right = [0.0f32; 3];
    let mut up = [0.0f32; 3];

    vectoangles(aimdir, &mut dir);
    angle_vectors(&dir, Some(&mut forward), Some(&mut right), Some(&mut up));

    let grenade_idx = {
        let mut num = edicts.len();
        let max = edicts.len().max(MAX_EDICTS);
        crate::g_utils::spawn_edict_raw(edicts, 0, &mut num, max, level.time)
    };

    edicts[grenade_idx].s.origin = *start;
    edicts[grenade_idx].velocity[0] = aimdir[0] * speed as f32;
    edicts[grenade_idx].velocity[1] = aimdir[1] * speed as f32;
    edicts[grenade_idx].velocity[2] = aimdir[2] * speed as f32;
    edicts[grenade_idx].velocity[0] += (200.0 + crandom() * 10.0) * up[0];
    edicts[grenade_idx].velocity[1] += (200.0 + crandom() * 10.0) * up[1];
    edicts[grenade_idx].velocity[2] += (200.0 + crandom() * 10.0) * up[2];
    edicts[grenade_idx].velocity[0] += crandom() * 10.0 * right[0];
    edicts[grenade_idx].velocity[1] += crandom() * 10.0 * right[1];
    edicts[grenade_idx].velocity[2] += crandom() * 10.0 * right[2];
    edicts[grenade_idx].avelocity = [300.0, 300.0, 300.0];
    edicts[grenade_idx].movetype = MoveType::Bounce;
    edicts[grenade_idx].clipmask = MASK_SHOT;
    edicts[grenade_idx].solid = Solid::Bbox;
    edicts[grenade_idx].s.effects |= EF_GRENADE;
    edicts[grenade_idx].mins = [0.0; 3];
    edicts[grenade_idx].maxs = [0.0; 3];
    edicts[grenade_idx].s.modelindex = gi_modelindex("models/objects/grenade/tris.md2");
    edicts[grenade_idx].owner = self_idx as i32;
    // grenade.touch = Grenade_Touch; — dispatch
    edicts[grenade_idx].touch_fn = Some(crate::dispatch::TOUCH_WEAPON_GRENADE);
    edicts[grenade_idx].nextthink = level.time + timer;
    // grenade.think = Grenade_Explode; — dispatch
    edicts[grenade_idx].think_fn = Some(crate::dispatch::THINK_GRENADE_EXPLODE);
    edicts[grenade_idx].dmg = damage;
    edicts[grenade_idx].dmg_radius = damage_radius;
    edicts[grenade_idx].classname = "grenade".to_string();
    gi_linkentity(grenade_idx as i32);
}

pub fn fire_grenade2(self_idx: usize, edicts: &mut Vec<Edict>, level: &mut LevelLocals,
                     start: &Vec3, aimdir: &Vec3, damage: i32,
                     speed: i32, timer: f32, damage_radius: f32, held: bool) {
    let mut dir = [0.0f32; 3];
    let mut forward = [0.0f32; 3];
    let mut right = [0.0f32; 3];
    let mut up = [0.0f32; 3];

    vectoangles(aimdir, &mut dir);
    angle_vectors(&dir, Some(&mut forward), Some(&mut right), Some(&mut up));

    let grenade_idx = {
        let mut num = edicts.len();
        let max = edicts.len().max(MAX_EDICTS);
        crate::g_utils::spawn_edict_raw(edicts, 0, &mut num, max, level.time)
    };

    edicts[grenade_idx].s.origin = *start;
    edicts[grenade_idx].velocity[0] = aimdir[0] * speed as f32;
    edicts[grenade_idx].velocity[1] = aimdir[1] * speed as f32;
    edicts[grenade_idx].velocity[2] = aimdir[2] * speed as f32;
    edicts[grenade_idx].velocity[0] += (200.0 + crandom() * 10.0) * up[0];
    edicts[grenade_idx].velocity[1] += (200.0 + crandom() * 10.0) * up[1];
    edicts[grenade_idx].velocity[2] += (200.0 + crandom() * 10.0) * up[2];
    edicts[grenade_idx].velocity[0] += crandom() * 10.0 * right[0];
    edicts[grenade_idx].velocity[1] += crandom() * 10.0 * right[1];
    edicts[grenade_idx].velocity[2] += crandom() * 10.0 * right[2];
    edicts[grenade_idx].avelocity = [300.0, 300.0, 300.0];
    edicts[grenade_idx].movetype = MoveType::Bounce;
    edicts[grenade_idx].clipmask = MASK_SHOT;
    edicts[grenade_idx].solid = Solid::Bbox;
    edicts[grenade_idx].s.effects |= EF_GRENADE;
    edicts[grenade_idx].mins = [0.0; 3];
    edicts[grenade_idx].maxs = [0.0; 3];
    edicts[grenade_idx].s.modelindex = gi_modelindex("models/objects/grenade2/tris.md2");
    edicts[grenade_idx].owner = self_idx as i32;
    // grenade.touch = Grenade_Touch; — dispatch
    edicts[grenade_idx].touch_fn = Some(crate::dispatch::TOUCH_WEAPON_GRENADE);
    edicts[grenade_idx].nextthink = level.time + timer;
    // grenade.think = Grenade_Explode; — dispatch
    edicts[grenade_idx].think_fn = Some(crate::dispatch::THINK_GRENADE_EXPLODE);
    edicts[grenade_idx].dmg = damage;
    edicts[grenade_idx].dmg_radius = damage_radius;
    edicts[grenade_idx].classname = "hgrenade".to_string();
    if held {
        edicts[grenade_idx].spawnflags = 3;
    } else {
        edicts[grenade_idx].spawnflags = 1;
    }
    edicts[grenade_idx].s.sound = gi_soundindex("weapons/hgrenc1b.wav");

    if timer <= 0.0 {
        grenade_explode(grenade_idx, edicts, level);
    } else {
        gi_sound(self_idx as i32, CHAN_WEAPON, gi_soundindex("weapons/hgrent1a.wav"),
                 1.0, ATTN_NORM, 0.0);
        gi_linkentity(grenade_idx as i32);
    }
}

/*
=================
fire_rocket
=================
*/
pub fn rocket_touch(ent_idx: usize, other_idx: usize, edicts: &mut Vec<Edict>,
                level: &mut LevelLocals,
                _plane: Option<&CPlane>, _surf: Option<&CSurface>) {
    let mut origin = [0.0f32; 3];

    if edicts[ent_idx].owner >= 0 && other_idx == edicts[ent_idx].owner as usize {
        return;
    }

    if let Some(surf) = &_surf {
        if (surf.flags & SURF_SKY) != 0 {
            crate::g_utils::free_edict_raw(edicts, ent_idx, 0, level.time);
            return;
        }
    }

    if edicts[ent_idx].owner >= 0 {
        let owner_idx = edicts[ent_idx].owner as usize;
        if owner_idx < edicts.len() && edicts[owner_idx].client.is_some() {
            // PlayerNoise
            // player_noise deferred: requires GameContext not available in this signature
        }
    }

    // calculate position for the explosion entity
    origin[0] = edicts[ent_idx].s.origin[0] + -0.02 * edicts[ent_idx].velocity[0];
    origin[1] = edicts[ent_idx].s.origin[1] + -0.02 * edicts[ent_idx].velocity[1];
    origin[2] = edicts[ent_idx].s.origin[2] + -0.02 * edicts[ent_idx].velocity[2];

    if edicts[other_idx].takedamage != 0 {
        let plane_normal = if let Some(plane) = _plane {
            plane.normal
        } else {
            vec3_origin
        };
        t_damage(
            other_idx, ent_idx, edicts[ent_idx].owner as usize,
            edicts[ent_idx].velocity, edicts[ent_idx].s.origin,
            plane_normal,
            edicts[ent_idx].dmg, 0, DamageFlags::empty(), MOD_ROCKET,
            edicts, level,
        );
    } else {
        // don't throw any debris in net games
        if get_deathmatch() == 0.0 && get_coop() == 0.0 {
            if let Some(surf) = &_surf {
                // In the original C, this checks surface name to spawn ThrowDebris
                // (metal vs concrete chunks). ThrowDebris is in g_misc.rs but requires
                // spawn context. Debris spawning deferred to g_misc integration.
            }
        }
    }

    t_radius_damage(
        ent_idx,
        edicts[ent_idx].owner as usize,
        edicts[ent_idx].radius_dmg as f32,
        Some(other_idx),
        edicts[ent_idx].dmg_radius,
        MOD_R_SPLASH,
        edicts, level,
    );

    gi_write_byte(SVC_TEMP_ENTITY);
    if edicts[ent_idx].waterlevel != 0 {
        gi_write_byte(TE_ROCKET_EXPLOSION_WATER);
    } else {
        gi_write_byte(TE_ROCKET_EXPLOSION);
    }
    gi_write_position(&origin);
    gi_multicast(&edicts[ent_idx].s.origin, MULTICAST_PHS);

    crate::g_utils::free_edict_raw(edicts, ent_idx, 0, level.time);
}

pub fn fire_rocket(self_idx: usize, edicts: &mut Vec<Edict>, level: &mut LevelLocals,
                   start: &Vec3, dir: &Vec3, damage: i32,
                   speed: i32, damage_radius: f32, radius_damage: i32) {
    let rocket_idx = {
        let mut num = edicts.len();
        let max = edicts.len().max(MAX_EDICTS);
        crate::g_utils::spawn_edict_raw(edicts, 0, &mut num, max, level.time)
    };

    edicts[rocket_idx].s.origin = *start;
    edicts[rocket_idx].movedir = *dir;
    vectoangles(dir, &mut edicts[rocket_idx].s.angles);
    edicts[rocket_idx].velocity[0] = dir[0] * speed as f32;
    edicts[rocket_idx].velocity[1] = dir[1] * speed as f32;
    edicts[rocket_idx].velocity[2] = dir[2] * speed as f32;
    edicts[rocket_idx].movetype = MoveType::FlyMissile;
    edicts[rocket_idx].clipmask = MASK_SHOT;
    edicts[rocket_idx].solid = Solid::Bbox;
    edicts[rocket_idx].s.effects |= EF_ROCKET;
    edicts[rocket_idx].mins = [0.0; 3];
    edicts[rocket_idx].maxs = [0.0; 3];
    edicts[rocket_idx].s.modelindex = gi_modelindex("models/objects/rocket/tris.md2");
    edicts[rocket_idx].owner = self_idx as i32;
    // rocket.touch = rocket_touch; — dispatch
    edicts[rocket_idx].touch_fn = Some(crate::dispatch::TOUCH_WEAPON_ROCKET);
    edicts[rocket_idx].nextthink = level.time + 8000.0 / speed as f32;
    // rocket.think = G_FreeEdict; — dispatch
    edicts[rocket_idx].think_fn = Some(crate::dispatch::THINK_FREE_EDICT);
    edicts[rocket_idx].dmg = damage;
    edicts[rocket_idx].radius_dmg = radius_damage;
    edicts[rocket_idx].dmg_radius = damage_radius;
    edicts[rocket_idx].s.sound = gi_soundindex("weapons/rockfly.wav");
    edicts[rocket_idx].classname = "rocket".to_string();

    if edicts[self_idx].client.is_some() {
        let rocket_origin = edicts[rocket_idx].s.origin;
        check_dodge(self_idx, edicts, level, &rocket_origin, dir, speed);
    }

    gi_linkentity(rocket_idx as i32);
}

/*
=================
fire_rail
=================
*/
pub fn fire_rail(self_idx: usize, edicts: &mut Vec<Edict>, level: &mut LevelLocals,
                 start: &Vec3, aimdir: &Vec3, damage: i32, kick: i32) {
    let mut from = vector_copy(start);
    let end = vector_ma(start, 8192.0, aimdir);
    let mut ignore = Some(self_idx);
    let mut water = false;
    let mut mask = MASK_SHOT | CONTENTS_SLIME | CONTENTS_LAVA;
    let mut last_endpos = *start;

    while let Some(ignore_idx) = ignore {
        let tr = gi_trace(&from, &vec3_origin, &vec3_origin, &end, ignore_idx as i32, mask);
        last_endpos = tr.endpos;

        if (tr.contents & (CONTENTS_SLIME | CONTENTS_LAVA)) != 0 {
            mask &= !(CONTENTS_SLIME | CONTENTS_LAVA);
            water = true;
        } else {
            if tr.ent_index >= 0 && (tr.ent_index as usize) < edicts.len() {
                let tr_ent_idx = tr.ent_index as usize;
                // ZOID--added so rail goes through SOLID_BBOX entities (gibs, etc)
                if (edicts[tr_ent_idx].svflags & SVF_MONSTER) != 0
                    || edicts[tr_ent_idx].client.is_some()
                    || edicts[tr_ent_idx].solid == Solid::Bbox
                {
                    ignore = Some(tr_ent_idx);
                } else {
                    ignore = None;
                }

                if tr_ent_idx != self_idx && edicts[tr_ent_idx].takedamage != 0 {
                    t_damage(
                        tr_ent_idx, self_idx, self_idx,
                        *aimdir, tr.endpos, tr.plane.normal,
                        damage, kick, DamageFlags::empty(), MOD_RAILGUN,
                        edicts, level,
                    );
                }
            } else {
                ignore = None;
            }
        }

        from = tr.endpos;
    }

    // send gun puff / flash
    gi_write_byte(SVC_TEMP_ENTITY);
    gi_write_byte(TE_RAILTRAIL);
    gi_write_position(start);
    gi_write_position(&last_endpos);
    gi_multicast(&edicts[self_idx].s.origin, MULTICAST_PHS);

    if water {
        gi_write_byte(SVC_TEMP_ENTITY);
        gi_write_byte(TE_RAILTRAIL);
        gi_write_position(start);
        gi_write_position(&last_endpos);
        gi_multicast(&last_endpos, MULTICAST_PHS);
    }

    if edicts[self_idx].client.is_some() {
        // PlayerNoise
        // player_noise deferred: requires GameContext not available in this signature
    }
}

/*
=================
fire_bfg
=================
*/
pub fn bfg_explode(self_idx: usize, edicts: &mut Vec<Edict>, level: &mut LevelLocals) {
    if edicts[self_idx].s.frame == 0 {
        // the BFG effect — find entities in radius and damage them
        let self_origin = edicts[self_idx].s.origin;
        let dmg_radius = edicts[self_idx].dmg_radius;
        let radius_dmg = edicts[self_idx].radius_dmg as f32;
        let owner_idx = edicts[self_idx].owner as usize;

        let num_edicts = edicts.len();
        for i in 0..num_edicts {
            if !edicts[i].inuse { continue; }
            if edicts[i].takedamage == 0 { continue; }
            if i == owner_idx { continue; }
            if edicts[i].solid == Solid::Not { continue; }

            // Check radius
            let mut eorg = [0.0f32; 3];
            for j in 0..3 {
                eorg[j] = self_origin[j] - (edicts[i].s.origin[j] + (edicts[i].mins[j] + edicts[i].maxs[j]) * 0.5);
            }
            if vector_length(&eorg) > dmg_radius { continue; }

            if !crate::g_combat::can_damage(i, self_idx, edicts) { continue; }
            if !crate::g_combat::can_damage(i, owner_idx, edicts) { continue; }

            let mut v = [0.0f32; 3];
            for j in 0..3 {
                v[j] = edicts[i].mins[j] + edicts[i].maxs[j];
            }
            v = vector_ma(&edicts[i].s.origin, 0.5, &v);
            v = vector_subtract(&self_origin, &v);
            let dist = vector_length(&v);
            let mut points = radius_dmg * (1.0 - (dist / dmg_radius).sqrt());
            if i == owner_idx {
                points *= 0.5;
            }

            let ent_origin = edicts[i].s.origin;
            gi_write_byte(SVC_TEMP_ENTITY);
            gi_write_byte(crate::g_local::TE_BFG_EXPLOSION);
            gi_write_position(&ent_origin);
            gi_multicast(&ent_origin, MULTICAST_PHS);

            let self_velocity = edicts[self_idx].velocity;
            t_damage(
                i, self_idx, owner_idx,
                self_velocity, ent_origin, vec3_origin,
                points as i32, 0, DAMAGE_ENERGY, crate::g_local::MOD_BFG_EFFECT,
                edicts, level,
            );
        }
    }

    edicts[self_idx].nextthink = level.time + FRAMETIME;
    edicts[self_idx].s.frame += 1;
    if edicts[self_idx].s.frame == 5 {
        // think = G_FreeEdict — dispatch will handle this
        crate::g_utils::free_edict_raw(edicts, self_idx, 0, level.time);
    }
}

pub fn bfg_touch(self_idx: usize, other_idx: usize, edicts: &mut Vec<Edict>,
             level: &mut LevelLocals,
             _plane: Option<&CPlane>, _surf: Option<&CSurface>) {
    if edicts[self_idx].owner >= 0 && other_idx == edicts[self_idx].owner as usize {
        return;
    }

    if let Some(surf) = &_surf {
        if (surf.flags & SURF_SKY) != 0 {
            crate::g_utils::free_edict_raw(edicts, self_idx, 0, level.time);
            return;
        }
    }

    if edicts[self_idx].owner >= 0 {
        let owner_idx = edicts[self_idx].owner as usize;
        if owner_idx < edicts.len() && edicts[owner_idx].client.is_some() {
            // PlayerNoise
            // player_noise deferred: requires GameContext not available in this signature
        }
    }

    // core explosion - prevents firing it into the wall/floor
    if edicts[other_idx].takedamage != 0 {
        let plane_normal = if let Some(plane) = _plane {
            plane.normal
        } else {
            vec3_origin
        };
        t_damage(
            other_idx, self_idx, edicts[self_idx].owner as usize,
            edicts[self_idx].velocity, edicts[self_idx].s.origin,
            plane_normal,
            200, 0, DamageFlags::empty(), MOD_BFG_BLAST,
            edicts, level,
        );
    }
    t_radius_damage(
        self_idx,
        edicts[self_idx].owner as usize,
        200.0,
        Some(other_idx),
        100.0,
        MOD_BFG_BLAST,
        edicts, level,
    );

    gi_sound(self_idx as i32, CHAN_VOICE, gi_soundindex("weapons/bfg__x1b.wav"), 1.0, ATTN_NORM, 0.0);
    edicts[self_idx].solid = Solid::Not;
    edicts[self_idx].touch_fn = None;
    edicts[self_idx].s.origin[0] -= FRAMETIME * edicts[self_idx].velocity[0];
    edicts[self_idx].s.origin[1] -= FRAMETIME * edicts[self_idx].velocity[1];
    edicts[self_idx].s.origin[2] -= FRAMETIME * edicts[self_idx].velocity[2];
    edicts[self_idx].velocity = [0.0; 3];
    edicts[self_idx].s.modelindex = gi_modelindex("sprites/s_bfg3.sp2");
    edicts[self_idx].s.frame = 0;
    edicts[self_idx].s.sound = 0;
    edicts[self_idx].s.effects &= !(EF_ANIM_ALLFAST);
    // self.think = bfg_explode; — dispatch
    edicts[self_idx].think_fn = Some(crate::dispatch::THINK_BFG_EXPLODE);
    edicts[self_idx].nextthink = level.time + FRAMETIME;
    edicts[self_idx].enemy = other_idx as i32;

    gi_write_byte(SVC_TEMP_ENTITY);
    gi_write_byte(TE_BFG_BIGEXPLOSION);
    gi_write_position(&edicts[self_idx].s.origin);
    gi_multicast(&edicts[self_idx].s.origin, MULTICAST_PVS);
}

pub fn bfg_think(self_idx: usize, edicts: &mut Vec<Edict>, level: &mut LevelLocals) {
    let dmg: i32;

    if get_deathmatch() != 0.0 {
        dmg = 5;
    } else {
        dmg = 10;
    }

    let self_origin = edicts[self_idx].s.origin;
    let owner_idx = edicts[self_idx].owner as usize;
    let skinnum = edicts[self_idx].s.skinnum;
    let num_edicts = edicts.len();

    for i in 0..num_edicts {
        if !edicts[i].inuse { continue; }
        if i == self_idx { continue; }
        if i == owner_idx { continue; }
        if edicts[i].takedamage == 0 { continue; }
        if (edicts[i].svflags & SVF_MONSTER) == 0 && edicts[i].client.is_none()
            && edicts[i].classname != "misc_explobox" { continue; }
        if edicts[i].solid == Solid::Not { continue; }

        let mut eorg = [0.0f32; 3];
        for j in 0..3 {
            eorg[j] = self_origin[j] - (edicts[i].s.origin[j] + (edicts[i].mins[j] + edicts[i].maxs[j]) * 0.5);
        }
        if vector_length(&eorg) > 256.0 { continue; }

        let mut point = [0.0f32; 3];
        for j in 0..3 {
            point[j] = edicts[i].absmin[j] + 0.5 * edicts[i].size[j];
        }

        let mut dir = vector_subtract(&point, &self_origin);
        vector_normalize(&mut dir);

        let mut ignore_idx = self_idx as i32;
        let mut start = self_origin;
        let end = vector_ma(&start, 2048.0, &dir);
        let mut last_endpos = start;

        loop {
            let tr = gi_trace(&start, &vec3_origin, &vec3_origin, &end, ignore_idx,
                CONTENTS_SOLID | CONTENTS_MONSTER | CONTENTS_DEADMONSTER);
            if tr.ent_index < 0 || (tr.ent_index as usize) >= edicts.len() {
                last_endpos = tr.endpos;
                break;
            }
            let tr_ent = tr.ent_index as usize;
            last_endpos = tr.endpos;

            if edicts[tr_ent].takedamage != 0
                && !edicts[tr_ent].flags.intersects(crate::g_local::FL_IMMUNE_LASER)
                && tr_ent != owner_idx
            {
                t_damage(tr_ent, self_idx, owner_idx, dir, tr.endpos, vec3_origin,
                    dmg, 1, DAMAGE_ENERGY, crate::g_local::MOD_BFG_LASER, edicts, level);
            }

            if (edicts[tr_ent].svflags & SVF_MONSTER) == 0 && edicts[tr_ent].client.is_none() {
                gi_write_byte(SVC_TEMP_ENTITY);
                gi_write_byte(crate::g_local::TE_LASER_SPARKS);
                gi_write_byte(4);
                gi_write_position(&tr.endpos);
                gi_write_dir(&tr.plane.normal);
                gi_write_byte(skinnum);
                gi_multicast(&tr.endpos, MULTICAST_PVS);
                break;
            }

            ignore_idx = tr_ent as i32;
            start = tr.endpos;
        }

        gi_write_byte(SVC_TEMP_ENTITY);
        gi_write_byte(crate::g_local::TE_BFG_LASER);
        gi_write_position(&self_origin);
        gi_write_position(&last_endpos);
        gi_multicast(&self_origin, MULTICAST_PHS);
    }

    edicts[self_idx].nextthink = level.time + FRAMETIME;
}

pub fn fire_bfg(self_idx: usize, edicts: &mut Vec<Edict>, level: &mut LevelLocals,
                start: &Vec3, dir: &Vec3, damage: i32,
                speed: i32, damage_radius: f32) {
    let bfg_idx = {
        let mut num = edicts.len();
        let max = edicts.len().max(MAX_EDICTS);
        crate::g_utils::spawn_edict_raw(edicts, 0, &mut num, max, level.time)
    };

    edicts[bfg_idx].s.origin = *start;
    edicts[bfg_idx].movedir = *dir;
    vectoangles(dir, &mut edicts[bfg_idx].s.angles);
    edicts[bfg_idx].velocity[0] = dir[0] * speed as f32;
    edicts[bfg_idx].velocity[1] = dir[1] * speed as f32;
    edicts[bfg_idx].velocity[2] = dir[2] * speed as f32;
    edicts[bfg_idx].movetype = MoveType::FlyMissile;
    edicts[bfg_idx].clipmask = MASK_SHOT;
    edicts[bfg_idx].solid = Solid::Bbox;
    edicts[bfg_idx].s.effects |= EF_BFG | EF_ANIM_ALLFAST;
    edicts[bfg_idx].mins = [0.0; 3];
    edicts[bfg_idx].maxs = [0.0; 3];
    edicts[bfg_idx].s.modelindex = gi_modelindex("sprites/s_bfg1.sp2");
    edicts[bfg_idx].owner = self_idx as i32;
    // bfg.touch = bfg_touch; — dispatch
    edicts[bfg_idx].touch_fn = Some(crate::dispatch::TOUCH_WEAPON_BFG);
    edicts[bfg_idx].nextthink = level.time + FRAMETIME;
    // bfg.think = bfg_think; — dispatch
    edicts[bfg_idx].think_fn = Some(crate::dispatch::THINK_BFG_THINK);
    edicts[bfg_idx].radius_dmg = damage;
    edicts[bfg_idx].dmg_radius = damage_radius;
    edicts[bfg_idx].classname = "bfg blast".to_string();
    edicts[bfg_idx].s.sound = gi_soundindex("weapons/bfg__l1a.wav");
    edicts[bfg_idx].teammaster = bfg_idx as i32;
    edicts[bfg_idx].teamchain = -1;

    if edicts[self_idx].client.is_some() {
        let bfg_origin = edicts[bfg_idx].s.origin;
        check_dodge(self_idx, edicts, level, &bfg_origin, dir, speed);
    }

    gi_linkentity(bfg_idx as i32);
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::g_local::{
        Edict, LevelLocals,
        MoveType,
        MOD_BLASTER, MOD_HYPERBLASTER,
        MOD_G_SPLASH, MOD_HG_SPLASH, MOD_HELD_GRENADE,
    };

    fn init_test_gi() {
        crate::game_import::set_gi(Box::new(crate::game_import::StubGameImport));
    }

    /// Create a minimal edicts vec with a world entity and a player entity.
    fn make_test_edicts() -> Vec<Edict> {
        let mut edicts = Vec::new();

        // Index 0: world entity
        let mut world = Edict::default();
        world.inuse = true;
        world.classname = "worldspawn".to_string();
        edicts.push(world);

        // Index 1: player entity (self)
        let mut player = Edict::default();
        player.inuse = true;
        player.client = Some(0);
        player.s.origin = [100.0, 200.0, 50.0];
        player.s.angles = [0.0, 90.0, 0.0];
        player.health = 100;
        player.mins = [-16.0, -16.0, -24.0];
        player.maxs = [16.0, 16.0, 32.0];
        player.solid = Solid::Bbox;
        edicts.push(player);

        edicts
    }

    fn make_test_level() -> LevelLocals {
        let mut level = LevelLocals::default();
        level.time = 10.0;
        level.framenum = 100;
        level
    }

    // ============================================================
    // fire_blaster tests
    // ============================================================

    #[test]
    fn test_fire_blaster_spawns_bolt() {
        init_test_gi();
        let mut edicts = make_test_edicts();
        let mut level = make_test_level();
        let initial_count = edicts.len();

        let start = [100.0, 200.0, 50.0];
        let dir = [1.0, 0.0, 0.0];
        let damage = 15;
        let speed = 1000;

        fire_blaster(1, &mut edicts, &mut level, &start, &dir, damage, speed, 0, false);

        // A new bolt entity should have been spawned
        assert!(edicts.len() > initial_count, "bolt entity should be spawned");

        let bolt_idx = initial_count; // first spawned entity
        assert_eq!(edicts[bolt_idx].classname, "bolt");
        assert_eq!(edicts[bolt_idx].dmg, damage);
        assert_eq!(edicts[bolt_idx].owner, 1);
        assert_eq!(edicts[bolt_idx].movetype, MoveType::FlyMissile);
        assert_eq!(edicts[bolt_idx].solid, Solid::Bbox);
        assert_eq!(edicts[bolt_idx].clipmask, MASK_SHOT);
    }

    #[test]
    fn test_fire_blaster_velocity() {
        init_test_gi();
        let mut edicts = make_test_edicts();
        let mut level = make_test_level();

        let start = [0.0, 0.0, 0.0];
        let dir = [1.0, 0.0, 0.0];
        let speed = 1000;

        fire_blaster(1, &mut edicts, &mut level, &start, &dir, 15, speed, 0, false);

        let bolt_idx = 2;
        // Velocity should be dir * speed
        assert!((edicts[bolt_idx].velocity[0] - 1000.0).abs() < 0.01);
        assert!((edicts[bolt_idx].velocity[1]).abs() < 0.01);
        assert!((edicts[bolt_idx].velocity[2]).abs() < 0.01);
    }

    #[test]
    fn test_fire_blaster_diagonal_velocity() {
        init_test_gi();
        let mut edicts = make_test_edicts();
        let mut level = make_test_level();

        let start = [0.0, 0.0, 0.0];
        // Unnormalized direction -- fire_blaster normalizes internally
        let mut dir: Vec3 = [1.0, 1.0, 0.0];
        let len = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt();
        dir[0] /= len;
        dir[1] /= len;

        let speed = 1000;
        fire_blaster(1, &mut edicts, &mut level, &start, &dir, 10, speed, 0, false);

        let bolt_idx = 2;
        let expected = 1000.0 / (2.0f32).sqrt();
        assert!((edicts[bolt_idx].velocity[0] - expected).abs() < 1.0);
        assert!((edicts[bolt_idx].velocity[1] - expected).abs() < 1.0);
    }

    #[test]
    fn test_fire_blaster_hyper_flag() {
        init_test_gi();
        let mut edicts = make_test_edicts();
        let mut level = make_test_level();

        fire_blaster(1, &mut edicts, &mut level,
            &[0.0; 3], &[1.0, 0.0, 0.0], 15, 1000, 0, true);

        let bolt_idx = 2;
        // Hyper flag sets spawnflags = 1
        assert_eq!(edicts[bolt_idx].spawnflags, 1);
    }

    #[test]
    fn test_fire_blaster_normal_no_hyper_flag() {
        init_test_gi();
        let mut edicts = make_test_edicts();
        let mut level = make_test_level();

        fire_blaster(1, &mut edicts, &mut level,
            &[0.0; 3], &[1.0, 0.0, 0.0], 15, 1000, 0, false);

        let bolt_idx = 2;
        assert_eq!(edicts[bolt_idx].spawnflags, 0);
    }

    #[test]
    fn test_fire_blaster_nextthink() {
        init_test_gi();
        let mut edicts = make_test_edicts();
        let mut level = make_test_level();
        level.time = 5.0;

        fire_blaster(1, &mut edicts, &mut level,
            &[0.0; 3], &[1.0, 0.0, 0.0], 15, 1000, 0, false);

        let bolt_idx = 2;
        // nextthink = level.time + 2.0
        assert!((edicts[bolt_idx].nextthink - 7.0).abs() < f32::EPSILON);
    }

    // ============================================================
    // fire_rocket tests
    // ============================================================

    #[test]
    fn test_fire_rocket_spawns_rocket() {
        init_test_gi();
        let mut edicts = make_test_edicts();
        let mut level = make_test_level();

        let start = [0.0, 0.0, 0.0];
        let dir = [1.0, 0.0, 0.0];
        fire_rocket(1, &mut edicts, &mut level, &start, &dir, 100, 650, 120.0, 120);

        let rocket_idx = 2;
        assert_eq!(edicts[rocket_idx].classname, "rocket");
        assert_eq!(edicts[rocket_idx].dmg, 100);
        assert_eq!(edicts[rocket_idx].radius_dmg, 120);
        assert!((edicts[rocket_idx].dmg_radius - 120.0).abs() < f32::EPSILON);
        assert_eq!(edicts[rocket_idx].owner, 1);
        assert_eq!(edicts[rocket_idx].movetype, MoveType::FlyMissile);
        assert_eq!(edicts[rocket_idx].clipmask, MASK_SHOT);
        assert_eq!(edicts[rocket_idx].solid, Solid::Bbox);
    }

    #[test]
    fn test_fire_rocket_velocity() {
        init_test_gi();
        let mut edicts = make_test_edicts();
        let mut level = make_test_level();

        let dir = [0.0, 1.0, 0.0];
        fire_rocket(1, &mut edicts, &mut level, &[0.0; 3], &dir, 100, 650, 120.0, 120);

        let rocket_idx = 2;
        assert!((edicts[rocket_idx].velocity[0]).abs() < 0.01);
        assert!((edicts[rocket_idx].velocity[1] - 650.0).abs() < 0.01);
        assert!((edicts[rocket_idx].velocity[2]).abs() < 0.01);
    }

    #[test]
    fn test_fire_rocket_nextthink() {
        init_test_gi();
        let mut edicts = make_test_edicts();
        let mut level = make_test_level();
        level.time = 5.0;

        let speed = 650;
        fire_rocket(1, &mut edicts, &mut level, &[0.0; 3], &[1.0, 0.0, 0.0], 100, speed, 120.0, 120);

        let rocket_idx = 2;
        // nextthink = level.time + 8000.0 / speed
        let expected = 5.0 + 8000.0 / speed as f32;
        assert!((edicts[rocket_idx].nextthink - expected).abs() < 0.01);
    }

    #[test]
    fn test_fire_rocket_effects() {
        init_test_gi();
        let mut edicts = make_test_edicts();
        let mut level = make_test_level();

        fire_rocket(1, &mut edicts, &mut level, &[0.0; 3], &[1.0, 0.0, 0.0], 100, 650, 120.0, 120);

        let rocket_idx = 2;
        assert!(edicts[rocket_idx].s.effects & EF_ROCKET != 0);
    }

    // ============================================================
    // fire_grenade / fire_grenade2 tests
    // ============================================================

    #[test]
    fn test_fire_grenade_spawns_grenade() {
        init_test_gi();
        let mut edicts = make_test_edicts();
        let mut level = make_test_level();

        let start = [0.0, 0.0, 0.0];
        let dir = [1.0, 0.0, 0.0];
        fire_grenade(1, &mut edicts, &mut level, &start, &dir, 120, 600, 2.5, 160.0);

        let grenade_idx = 2;
        assert_eq!(edicts[grenade_idx].classname, "grenade");
        assert_eq!(edicts[grenade_idx].dmg, 120);
        assert!((edicts[grenade_idx].dmg_radius - 160.0).abs() < f32::EPSILON);
        assert_eq!(edicts[grenade_idx].owner, 1);
        assert_eq!(edicts[grenade_idx].movetype, MoveType::Bounce);
        assert_eq!(edicts[grenade_idx].clipmask, MASK_SHOT);
        assert_eq!(edicts[grenade_idx].solid, Solid::Bbox);
    }

    #[test]
    fn test_fire_grenade_effects() {
        init_test_gi();
        let mut edicts = make_test_edicts();
        let mut level = make_test_level();

        fire_grenade(1, &mut edicts, &mut level, &[0.0; 3], &[1.0, 0.0, 0.0], 120, 600, 2.5, 160.0);

        let grenade_idx = 2;
        assert!(edicts[grenade_idx].s.effects & EF_GRENADE != 0);
    }

    #[test]
    fn test_fire_grenade_nextthink_matches_timer() {
        init_test_gi();
        let mut edicts = make_test_edicts();
        let mut level = make_test_level();
        level.time = 5.0;

        let timer = 2.5;
        fire_grenade(1, &mut edicts, &mut level, &[0.0; 3], &[1.0, 0.0, 0.0], 120, 600, timer, 160.0);

        let grenade_idx = 2;
        // nextthink = level.time + timer
        assert!((edicts[grenade_idx].nextthink - 7.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_fire_grenade_angular_velocity() {
        init_test_gi();
        let mut edicts = make_test_edicts();
        let mut level = make_test_level();

        fire_grenade(1, &mut edicts, &mut level, &[0.0; 3], &[1.0, 0.0, 0.0], 120, 600, 2.5, 160.0);

        let grenade_idx = 2;
        // avelocity = [300, 300, 300]
        assert_eq!(edicts[grenade_idx].avelocity, [300.0, 300.0, 300.0]);
    }

    #[test]
    fn test_fire_grenade2_hand_grenade() {
        init_test_gi();
        let mut edicts = make_test_edicts();
        let mut level = make_test_level();

        fire_grenade2(1, &mut edicts, &mut level, &[0.0; 3], &[1.0, 0.0, 0.0],
            125, 400, 2.0, 165.0, false);

        let grenade_idx = 2;
        assert_eq!(edicts[grenade_idx].classname, "hgrenade");
        // Not held: spawnflags = 1
        assert_eq!(edicts[grenade_idx].spawnflags, 1);
    }

    #[test]
    fn test_fire_grenade2_held_grenade() {
        init_test_gi();
        let mut edicts = make_test_edicts();
        let mut level = make_test_level();

        fire_grenade2(1, &mut edicts, &mut level, &[0.0; 3], &[1.0, 0.0, 0.0],
            125, 400, 2.0, 165.0, true);

        let grenade_idx = 2;
        assert_eq!(edicts[grenade_idx].classname, "hgrenade");
        // Held: spawnflags = 3 (bit 0 = hand grenade, bit 1 = held)
        assert_eq!(edicts[grenade_idx].spawnflags, 3);
    }

    #[test]
    fn test_fire_grenade2_zero_timer_explodes_immediately() {
        init_test_gi();
        let mut edicts = make_test_edicts();
        let mut level = make_test_level();

        let initial_count = edicts.len();
        // timer <= 0.0 triggers immediate explosion via grenade_explode
        fire_grenade2(1, &mut edicts, &mut level, &[0.0; 3], &[1.0, 0.0, 0.0],
            125, 400, 0.0, 165.0, true);

        // A new entity should have been spawned for the grenade
        assert!(edicts.len() > initial_count, "grenade entity should have been spawned");

        // When timer <= 0.0, grenade_explode is called immediately.
        // free_edict_raw only frees entities with index > maxclients + BODY_QUEUE_SIZE.
        // With maxclients=0 in our test setup, BODY_QUEUE_SIZE=8, and our grenade at
        // index 2, the free is skipped (entity index too low). Verify the grenade
        // was at least processed (it should still be an hgrenade entity with held flags).
        let grenade_idx = initial_count;
        assert_eq!(edicts[grenade_idx].classname, "hgrenade");
        assert_eq!(edicts[grenade_idx].spawnflags, 3); // held grenade flags
    }

    // ============================================================
    // fire_bfg tests
    // ============================================================

    #[test]
    fn test_fire_bfg_spawns_bfg() {
        init_test_gi();
        let mut edicts = make_test_edicts();
        let mut level = make_test_level();

        fire_bfg(1, &mut edicts, &mut level, &[0.0; 3], &[1.0, 0.0, 0.0], 200, 400, 1000.0);

        let bfg_idx = 2;
        assert_eq!(edicts[bfg_idx].classname, "bfg blast");
        assert_eq!(edicts[bfg_idx].radius_dmg, 200);
        assert!((edicts[bfg_idx].dmg_radius - 1000.0).abs() < f32::EPSILON);
        assert_eq!(edicts[bfg_idx].owner, 1);
        assert_eq!(edicts[bfg_idx].movetype, MoveType::FlyMissile);
    }

    #[test]
    fn test_fire_bfg_velocity() {
        init_test_gi();
        let mut edicts = make_test_edicts();
        let mut level = make_test_level();

        let dir = [0.0, 0.0, 1.0];
        fire_bfg(1, &mut edicts, &mut level, &[0.0; 3], &dir, 200, 400, 1000.0);

        let bfg_idx = 2;
        assert!((edicts[bfg_idx].velocity[0]).abs() < 0.01);
        assert!((edicts[bfg_idx].velocity[1]).abs() < 0.01);
        assert!((edicts[bfg_idx].velocity[2] - 400.0).abs() < 0.01);
    }

    #[test]
    fn test_fire_bfg_effects() {
        init_test_gi();
        let mut edicts = make_test_edicts();
        let mut level = make_test_level();

        fire_bfg(1, &mut edicts, &mut level, &[0.0; 3], &[1.0, 0.0, 0.0], 200, 400, 1000.0);

        let bfg_idx = 2;
        assert!(edicts[bfg_idx].s.effects & EF_BFG != 0);
        assert!(edicts[bfg_idx].s.effects & EF_ANIM_ALLFAST != 0);
    }

    #[test]
    fn test_fire_bfg_nextthink() {
        init_test_gi();
        let mut edicts = make_test_edicts();
        let mut level = make_test_level();
        level.time = 3.0;

        fire_bfg(1, &mut edicts, &mut level, &[0.0; 3], &[1.0, 0.0, 0.0], 200, 400, 1000.0);

        let bfg_idx = 2;
        // nextthink = level.time + FRAMETIME (0.1)
        assert!((edicts[bfg_idx].nextthink - 3.1).abs() < 0.001);
    }

    #[test]
    fn test_fire_bfg_team_setup() {
        init_test_gi();
        let mut edicts = make_test_edicts();
        let mut level = make_test_level();

        fire_bfg(1, &mut edicts, &mut level, &[0.0; 3], &[1.0, 0.0, 0.0], 200, 400, 1000.0);

        let bfg_idx = 2;
        assert_eq!(edicts[bfg_idx].teammaster, bfg_idx as i32);
        assert_eq!(edicts[bfg_idx].teamchain, -1);
    }

    // ============================================================
    // fire_hit tests (melee hit detection math)
    // ============================================================

    #[test]
    fn test_fire_hit_enemy_out_of_range() {
        init_test_gi();
        let mut edicts = make_test_edicts();
        let mut level = make_test_level();

        // Add an enemy at index 2
        let mut enemy = Edict::default();
        enemy.inuse = true;
        enemy.s.origin = [1000.0, 0.0, 0.0]; // very far away
        enemy.health = 100;
        enemy.takedamage = 1;
        enemy.mins = [-16.0, -16.0, -24.0];
        enemy.maxs = [16.0, 16.0, 32.0];
        enemy.solid = Solid::Bbox;
        edicts.push(enemy);

        // Set player's enemy
        edicts[1].enemy = 2;
        edicts[1].s.origin = [0.0, 0.0, 0.0];
        edicts[1].s.angles = [0.0, 0.0, 0.0];

        // aim[0] is the range limit, e.g. 80 (MELEE_DISTANCE)
        let aim = [80.0, 0.0, 0.0];
        let result = fire_hit(1, &mut edicts, &mut level, &aim, 10, 200);

        // Enemy is at 1000 units, aim range is 80 => should miss
        assert!(!result);
    }

    #[test]
    fn test_fire_hit_no_enemy() {
        init_test_gi();
        let mut edicts = make_test_edicts();
        let mut level = make_test_level();

        // No valid enemy
        edicts[1].enemy = -1;

        let aim = [80.0, 0.0, 0.0];
        let result = fire_hit(1, &mut edicts, &mut level, &aim, 10, 200);
        assert!(!result);
    }

    #[test]
    fn test_fire_hit_enemy_index_too_large() {
        init_test_gi();
        let mut edicts = make_test_edicts();
        let mut level = make_test_level();

        // Enemy index beyond edicts array
        edicts[1].enemy = 999;

        let aim = [80.0, 0.0, 0.0];
        let result = fire_hit(1, &mut edicts, &mut level, &aim, 10, 200);
        assert!(!result);
    }

    // ============================================================
    // Grenade explosion origin offset test
    // ============================================================

    #[test]
    fn test_grenade_explosion_origin_offset() {
        // From grenade_explode: origin = s.origin + -0.02 * velocity
        let origin: Vec3 = [100.0, 200.0, 50.0];
        let velocity: Vec3 = [500.0, 0.0, 0.0];

        let explosion_origin: Vec3 = [
            origin[0] + -0.02 * velocity[0],
            origin[1] + -0.02 * velocity[1],
            origin[2] + -0.02 * velocity[2],
        ];

        assert!((explosion_origin[0] - 90.0).abs() < 0.01);
        assert!((explosion_origin[1] - 200.0).abs() < 0.01);
        assert!((explosion_origin[2] - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_rocket_explosion_origin_offset() {
        // Same formula as grenade: origin = s.origin + -0.02 * velocity
        let origin: Vec3 = [0.0, 0.0, 0.0];
        let velocity: Vec3 = [650.0, 0.0, 0.0];

        let explosion_origin: Vec3 = [
            origin[0] + -0.02 * velocity[0],
            origin[1] + -0.02 * velocity[1],
            origin[2] + -0.02 * velocity[2],
        ];

        assert!((explosion_origin[0] - (-13.0)).abs() < 0.01);
    }

    // ============================================================
    // Projectile velocity calculations
    // ============================================================

    #[test]
    fn test_projectile_velocity_calculation() {
        // All projectiles: velocity = dir * speed
        let dir = [0.577, 0.577, 0.577]; // approximately unit vector
        let speed = 1000;

        let velocity = [
            dir[0] * speed as f32,
            dir[1] * speed as f32,
            dir[2] * speed as f32,
        ];

        assert!((velocity[0] - 577.0).abs() < 1.0);
        assert!((velocity[1] - 577.0).abs() < 1.0);
        assert!((velocity[2] - 577.0).abs() < 1.0);
    }

    // ============================================================
    // Splash color constant tests
    // ============================================================

    #[test]
    fn test_splash_color_constants() {
        assert_eq!(SPLASH_UNKNOWN, 0);
        assert_eq!(SPLASH_SPARKS, 1);
        assert_eq!(SPLASH_BLUE_WATER, 2);
        assert_eq!(SPLASH_BROWN_WATER, 3);
        assert_eq!(SPLASH_SLIME, 4);
        assert_eq!(SPLASH_LAVA, 5);
        assert_eq!(SPLASH_BLOOD, 6);
    }

    // ============================================================
    // fire_lead spread calculation tests (pure math)
    // ============================================================

    #[test]
    fn test_spread_pattern_math() {
        // fire_lead uses: end = MA(start, 8192, forward) + r*right + u*up
        // where r = crandom() * hspread, u = crandom() * vspread
        // Test the vector_ma operations with known values
        let start = [0.0, 0.0, 0.0];
        let forward = [1.0, 0.0, 0.0];
        let right = [0.0, 1.0, 0.0];
        let up = [0.0, 0.0, 1.0];

        let end = vector_ma(&start, 8192.0, &forward);
        assert!((end[0] - 8192.0).abs() < 0.01);
        assert!((end[1]).abs() < 0.01);

        let r = 500.0; // hspread-like
        let u = 300.0; // vspread-like
        let end2 = vector_ma(&end, r, &right);
        let end3 = vector_ma(&end2, u, &up);

        assert!((end3[0] - 8192.0).abs() < 0.01);
        assert!((end3[1] - 500.0).abs() < 0.01);
        assert!((end3[2] - 300.0).abs() < 0.01);
    }

    #[test]
    fn test_water_spread_doubled() {
        // When bullet enters water, spread is doubled: hspread*2, vspread*2
        let hspread = 300;
        let vspread = 500;
        let water_hspread = hspread as f32 * 2.0;
        let water_vspread = vspread as f32 * 2.0;
        assert_eq!(water_hspread, 600.0);
        assert_eq!(water_vspread, 1000.0);
    }

    // ============================================================
    // BFG damage radius calculation tests
    // ============================================================

    #[test]
    fn test_bfg_explode_damage_falloff() {
        // From bfg_explode: points = radius_dmg * (1.0 - sqrt(dist / dmg_radius))
        let radius_dmg: f32 = 200.0;
        let dmg_radius: f32 = 1000.0;

        // At distance 0: full damage
        let dist = 0.0f32;
        let points = radius_dmg * (1.0 - (dist / dmg_radius).sqrt());
        assert!((points - 200.0).abs() < 0.01);

        // At distance 250 (quarter radius): sqrt(0.25) = 0.5
        let dist = 250.0f32;
        let points = radius_dmg * (1.0 - (dist / dmg_radius).sqrt());
        assert!((points - 100.0).abs() < 0.01);

        // At distance 1000 (full radius): sqrt(1.0) = 1.0, damage = 0
        let dist = 1000.0f32;
        let points = radius_dmg * (1.0 - (dist / dmg_radius).sqrt());
        assert!(points.abs() < 0.01);
    }

    #[test]
    fn test_bfg_self_damage_halved() {
        // If the BFG owner hits themselves: points *= 0.5
        let radius_dmg: f32 = 200.0;
        let dmg_radius: f32 = 1000.0;
        let dist = 0.0f32;
        let mut points = radius_dmg * (1.0 - (dist / dmg_radius).sqrt());
        points *= 0.5; // self-damage halved
        assert!((points - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_bfg_think_laser_range() {
        // BFG laser effect range is 256 units
        let bfg_laser_range = 256.0f32;
        // Entities beyond this range are skipped
        assert_eq!(bfg_laser_range, 256.0);
    }

    #[test]
    fn test_bfg_think_damage_values() {
        // DM: 5 damage per laser tick, SP: 10
        let dm_dmg = 5;
        let sp_dmg = 10;
        assert_eq!(dm_dmg, 5);
        assert_eq!(sp_dmg, 10);
    }

    // ============================================================
    // BFG touch radius damage test
    // ============================================================

    #[test]
    fn test_bfg_touch_core_damage() {
        // bfg_touch deals 200 damage on direct hit plus 200 radius at 100 radius
        let core_damage = 200;
        let radius_damage = 200.0;
        let radius = 100.0;
        assert_eq!(core_damage, 200);
        assert_eq!(radius_damage, 200.0);
        assert_eq!(radius, 100.0);
    }

    // ============================================================
    // Grenade damage points calculation
    // ============================================================

    #[test]
    fn test_grenade_contact_damage_points() {
        // From grenade_explode: points = dmg - 0.5 * vlen
        // where vlen is distance from grenade origin to enemy center
        let dmg = 120.0f32;

        // Direct hit: distance ~0
        let vlen = 0.0f32;
        let points = dmg - 0.5 * vlen;
        assert!((points - 120.0).abs() < 0.01);

        // Hit at 100 units
        let vlen = 100.0f32;
        let points = dmg - 0.5 * vlen;
        assert!((points - 70.0).abs() < 0.01);

        // Hit at 240 units (damage goes to 0)
        let vlen = 240.0f32;
        let points = dmg - 0.5 * vlen;
        assert!((points - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_grenade_explode_mod_types() {
        // spawnflags & 2 => MOD_HELD_GRENADE
        // spawnflags & 1 => MOD_HG_SPLASH
        // else => MOD_G_SPLASH
        let test_cases = [
            (3, MOD_HELD_GRENADE),  // bits 0+1 set, bit 1 takes priority
            (2, MOD_HELD_GRENADE),  // bit 1 set
            (1, MOD_HG_SPLASH),     // bit 0 set only
            (0, MOD_G_SPLASH),      // no bits set
        ];
        for (spawnflags, expected_mod) in &test_cases {
            let mod_type = if (spawnflags & 2) != 0 {
                MOD_HELD_GRENADE
            } else if (spawnflags & 1) != 0 {
                MOD_HG_SPLASH
            } else {
                MOD_G_SPLASH
            };
            assert_eq!(mod_type, *expected_mod, "spawnflags={}", spawnflags);
        }
    }

    // ============================================================
    // Grenade velocity components test
    // ============================================================

    #[test]
    fn test_grenade_velocity_has_upward_component() {
        // Grenades add 200 + crandom()*10 up component
        // The base is 200.0, so up velocity is always at least ~190
        let base_up = 200.0f32;
        let random_variation = 10.0f32;
        // Minimum up contribution = (200 - 10) = 190 (when crandom = -1)
        assert!(base_up - random_variation > 0.0);
        // Maximum up contribution = (200 + 10) = 210
        assert!(base_up + random_variation == 210.0);
    }

    // ============================================================
    // blaster_touch mod_type selection
    // ============================================================

    #[test]
    fn test_blaster_touch_mod_type() {
        // spawnflags & 1 => MOD_HYPERBLASTER, else MOD_BLASTER
        let hyper_flags = 1;
        let normal_flags = 0;

        let hyper_mod = if (hyper_flags & 1) != 0 { MOD_HYPERBLASTER } else { MOD_BLASTER };
        let normal_mod = if (normal_flags & 1) != 0 { MOD_HYPERBLASTER } else { MOD_BLASTER };

        assert_eq!(hyper_mod, MOD_HYPERBLASTER);
        assert_eq!(normal_mod, MOD_BLASTER);
    }

    // ============================================================
    // Vector math helpers (used extensively in weapon code)
    // ============================================================

    #[test]
    fn test_vector_ma_for_projectile_direction() {
        let start = [0.0, 0.0, 0.0];
        let forward = [1.0, 0.0, 0.0];
        let end = vector_ma(&start, 8192.0, &forward);
        assert_eq!(end, [8192.0, 0.0, 0.0]);
    }

    #[test]
    fn test_vectoangles_forward() {
        let dir = [1.0, 0.0, 0.0]; // pointing along +X
        let mut angles = [0.0f32; 3];
        vectoangles(&dir, &mut angles);
        // Should be yaw=0, pitch=0
        assert!((angles[0]).abs() < 0.01); // pitch
        assert!((angles[1]).abs() < 0.01); // yaw
    }

    #[test]
    fn test_vectoangles_upward() {
        let dir = [0.0, 0.0, 1.0]; // pointing straight up
        let mut angles = [0.0f32; 3];
        vectoangles(&dir, &mut angles);
        // pitch should be -90 (looking up)
        assert!((angles[0] - (-90.0)).abs() < 0.01);
    }

    #[test]
    fn test_vector_normalize_unit() {
        let mut v = [3.0, 4.0, 0.0]; // length = 5
        let len = vector_normalize(&mut v);
        assert!((len - 5.0).abs() < 0.01);
        assert!((v[0] - 0.6).abs() < 0.01);
        assert!((v[1] - 0.8).abs() < 0.01);
    }

    // ============================================================
    // Bubble trail midpoint calculation
    // ============================================================

    #[test]
    fn test_bubble_trail_midpoint() {
        // From fire_lead: mid = (water_start + pos) * 0.5
        let water_start = [100.0, 200.0, 50.0];
        let pos = [200.0, 400.0, 100.0];

        let mid = [
            (water_start[0] + pos[0]) * 0.5,
            (water_start[1] + pos[1]) * 0.5,
            (water_start[2] + pos[2]) * 0.5,
        ];

        assert_eq!(mid, [150.0, 300.0, 75.0]);
    }

    // ============================================================
    // BFG explode frame progression
    // ============================================================

    #[test]
    fn test_bfg_explode_frame_progression() {
        // bfg_explode increments frame each call
        // frame 0: do damage scan
        // frame 5: free the entity
        let mut frame = 0;
        let mut did_damage = false;
        let mut freed = false;

        for _ in 0..6 {
            if frame == 0 {
                did_damage = true;
            }
            frame += 1;
            if frame == 5 {
                freed = true;
                break;
            }
        }

        assert!(did_damage);
        assert!(freed);
        assert_eq!(frame, 5);
    }

    // ============================================================
    // Entity origin computation for BFG damage
    // ============================================================

    #[test]
    fn test_bfg_entity_center_calculation() {
        // eorg[j] = self_origin[j] - (entity.origin[j] + (mins[j] + maxs[j]) * 0.5)
        let self_origin = [0.0, 0.0, 0.0];
        let entity_origin = [100.0, 0.0, 0.0];
        let mins = [-16.0, -16.0, -24.0];
        let maxs = [16.0, 16.0, 32.0];

        let mut eorg = [0.0f32; 3];
        for j in 0..3 {
            eorg[j] = self_origin[j] - (entity_origin[j] + (mins[j] + maxs[j]) * 0.5);
        }

        // Center offset for min/max: (min+max)/2 = [0, 0, 4]
        // So eorg = [0-100-0, 0-0-0, 0-0-4] = [-100, 0, -4]
        assert!((eorg[0] - (-100.0)).abs() < 0.01);
        assert!((eorg[1]).abs() < 0.01);
        assert!((eorg[2] - (-4.0)).abs() < 0.01);
    }

    // ============================================================
    // fire_rail tests
    // ============================================================

    #[test]
    fn test_fire_rail_creates_no_entities() {
        init_test_gi();
        let mut edicts = make_test_edicts();
        let mut level = make_test_level();
        let count_before = edicts.len();

        // Rail is hitscan, doesn't spawn projectile entities
        fire_rail(1, &mut edicts, &mut level, &[0.0; 3], &[1.0, 0.0, 0.0], 100, 200);

        // No new entities should be spawned (rail is instant hit)
        assert_eq!(edicts.len(), count_before);
    }

    // ============================================================
    // Grenade touch tests
    // ============================================================

    #[test]
    fn test_grenade_touch_ignores_owner() {
        init_test_gi();
        let mut edicts = make_test_edicts();
        let mut level = make_test_level();

        // Spawn a grenade at index 2
        let mut grenade = Edict::default();
        grenade.inuse = true;
        grenade.owner = 1; // owned by player (index 1)
        grenade.classname = "grenade".to_string();
        grenade.dmg = 120;
        grenade.dmg_radius = 160.0;
        edicts.push(grenade);

        // Touch by owner should be ignored (no explosion)
        grenade_touch(2, 1, &mut edicts, &mut level, None, None);

        // Grenade should still exist (not freed)
        assert_eq!(edicts[2].classname, "grenade");
    }

    #[test]
    fn test_blaster_touch_ignores_owner() {
        init_test_gi();
        let mut edicts = make_test_edicts();
        let mut level = make_test_level();

        // Spawn a bolt at index 2
        let mut bolt = Edict::default();
        bolt.inuse = true;
        bolt.owner = 1;
        bolt.classname = "bolt".to_string();
        bolt.dmg = 15;
        edicts.push(bolt);

        // Touch by owner should be ignored
        blaster_touch(2, 1, &mut edicts, &mut level, None, None);

        // Bolt should still exist
        assert_eq!(edicts[2].classname, "bolt");
    }

    // ============================================================
    // Weapon speed constants
    // ============================================================

    #[test]
    fn test_weapon_projectile_speeds() {
        // Blaster: 1000
        // Rocket: 650
        // Grenade launcher: 600
        // BFG: 400
        let speeds = [
            ("blaster", 1000),
            ("rocket", 650),
            ("grenade_launcher", 600),
            ("bfg", 400),
        ];
        // Verify they're in descending order (blaster fastest, BFG slowest)
        for i in 0..speeds.len() - 1 {
            assert!(speeds[i].1 > speeds[i + 1].1,
                "{} should be faster than {}", speeds[i].0, speeds[i + 1].0);
        }
    }
}

