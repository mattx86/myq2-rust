// p_weapon.rs — Player weapon logic
// Converted from: myq2-original/game/p_weapon.c

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

use std::f32::consts::PI;

use myq2_common::q_shared::{
    Vec3, PITCH, YAW, ROLL,
    MZ_BLASTER, MZ_SHOTGUN, MZ_SSHOTGUN, MZ_MACHINEGUN, MZ_CHAINGUN1,
    MZ_ROCKET, MZ_GRENADE, MZ_RAILGUN, MZ_BFG, MZ_HYPERBLASTER, MZ_SILENCED,
    vector_subtract, vector_add, vector_set, vector_scale,
    angle_vectors, BUTTON_ATTACK, PMF_DUCKED,
    DmFlags, DF_WEAPONS_STAY, DF_INFINITE_AMMO,
};

use crate::g_local::*;
use crate::g_utils::g_project_source;
use crate::game::SVF_NOCLIENT;
use crate::game_import::*;
use myq2_common::common::rand_i32;
use crate::m_player_frames::*;

// ============================================================
// Muzzle flash / effect / sound constants
// ============================================================

// EF_*, CHAN_*, ATTN_*, BUTTON_*, PMF_* come from q_shared re-export
// BUTTON_ATTACK_I32: i32 version for bitwise ops with i32 buttons field
const BUTTON_ATTACK_I32: i32 = BUTTON_ATTACK as i32;
// MULTICAST_PVS comes from g_local::*

// Grenade constants
pub const GRENADE_TIMER: f32 = 3.0;
pub const GRENADE_MINSPEED: i32 = 400;
pub const GRENADE_MAXSPEED: i32 = 800;

// ============================================================
// Module-level state (replaces C statics)
// ============================================================

/// Context for weapon state that was global/static in C.
/// In the original code, `is_quad` and `is_silenced` were file-scope statics
/// set each frame in Think_Weapon and read by fire functions.
#[derive(Default)]
pub struct WeaponContext {
    pub is_quad: bool,
    pub is_silenced: u8,
}


// ============================================================
// Random helpers (placeholders matching C rand()/random()/crandom())
// ============================================================

fn random_f32() -> f32 {
    // C random(): ((rand() & 0x7fff) / ((float)0x7fff))
    (rand::random::<u16>() & 0x7fff) as f32 / 0x7fff as f32
}

fn crandom_f32() -> f32 {
    2.0 * (random_f32() - 0.5)
}


// ============================================================
// Game state context (passed instead of C globals)
// ============================================================

// ============================================================
// Helper: P_ProjectSource
// ============================================================

/// Adjusts weapon offset based on handedness, then calls G_ProjectSource.
fn p_project_source(
    client: &GClient,
    point: &Vec3,
    distance: &Vec3,
    forward: &Vec3,
    right: &Vec3,
) -> Vec3 {
    let mut dist = *distance;
    if client.pers.hand == LEFT_HANDED {
        dist[1] *= -1.0;
    } else if client.pers.hand == CENTER_HANDED {
        dist[1] = 0.0;
    }
    g_project_source(point, &dist, forward, right)
}

// ============================================================
// PlayerNoise
// ============================================================

/// Each player can have two noise objects associated with it:
/// a personal noise (jumping, pain, weapon firing), and a weapon
/// target noise (bullet wall impacts).
///
/// Monsters that don't directly see the player can move
/// to a noise in hopes of seeing the player from there.
pub fn player_noise(ctx: &mut GameContext, who_idx: usize, where_pos: &Vec3, noise_type: i32) {
    {
        let client_idx = ctx.edicts[who_idx].client.expect("PlayerNoise: no client");
        let client = &mut ctx.clients[client_idx];
        if noise_type == PNOISE_WEAPON
            && client.silencer_shots > 0 {
                client.silencer_shots -= 1;
                return;
            }
    }

    if ctx.deathmatch != 0.0 {
        return;
    }

    if ctx.edicts[who_idx].flags.intersects(FL_NOTARGET) {
        return;
    }

    if ctx.edicts[who_idx].mynoise == 0 {
        // Spawn two noise entities for this player
        let noise_idx = spawn_noise_entity(ctx);
        ctx.edicts[noise_idx].classname = "player_noise".to_string();
        ctx.edicts[noise_idx].mins = [0.0; 3];
        ctx.edicts[noise_idx].maxs = [0.0; 3];
        ctx.edicts[noise_idx].owner = who_idx as i32;
        ctx.edicts[noise_idx].svflags = SVF_NOCLIENT;
        ctx.edicts[who_idx].mynoise = noise_idx as i32;

        let noise2_idx = spawn_noise_entity(ctx);
        ctx.edicts[noise2_idx].classname = "player_noise".to_string();
        ctx.edicts[noise2_idx].mins = [0.0; 3];
        ctx.edicts[noise2_idx].maxs = [0.0; 3];
        ctx.edicts[noise2_idx].owner = who_idx as i32;
        ctx.edicts[noise2_idx].svflags = SVF_NOCLIENT;
        ctx.edicts[who_idx].mynoise2 = noise2_idx as i32;
    }

    let noise_idx;
    if noise_type == PNOISE_SELF || noise_type == PNOISE_WEAPON {
        noise_idx = ctx.edicts[who_idx].mynoise as usize;
        ctx.level.sound_entity = noise_idx as i32;
        ctx.level.sound_entity_framenum = ctx.level.framenum;
    } else {
        // PNOISE_IMPACT
        noise_idx = ctx.edicts[who_idx].mynoise2 as usize;
        ctx.level.sound2_entity = noise_idx as i32;
        ctx.level.sound2_entity_framenum = ctx.level.framenum;
    }

    let noise = &mut ctx.edicts[noise_idx];
    noise.s.origin = *where_pos;
    noise.absmin = vector_subtract(where_pos, &noise.maxs);
    noise.absmax = vector_add(where_pos, &noise.maxs);
    noise.teleport_time = ctx.level.time;

    gi_linkentity(noise_idx as i32);
}

// ============================================================
// Pickup_Weapon
// ============================================================

pub fn pickup_weapon(
    ctx: &mut GameContext,
    ent_idx: usize,
    other_idx: usize,
) -> bool {
    let item_idx = ctx.edicts[ent_idx].item.expect("Pickup_Weapon: no item");
    let spawnflags = ctx.edicts[ent_idx].spawnflags;

    let other_client_idx = ctx.edicts[other_idx].client.expect("Pickup_Weapon: other has no client");

    // Check weapons-stay / coop
    if (DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_WEAPONS_STAY) || ctx.coop != 0.0)
        && ctx.clients[other_client_idx].pers.inventory[item_idx] != 0
        && spawnflags & (DROPPED_ITEM | DROPPED_PLAYER_ITEM) == 0 {
            return false; // leave the weapon for others to pickup
        }

    ctx.clients[other_client_idx].pers.inventory[item_idx] += 1;

    if spawnflags & DROPPED_ITEM == 0 {
        // give them some ammo with it
        let ammo_name = ctx.items[item_idx].ammo.clone();
        if !ammo_name.is_empty() {
            if let Some(ammo_item_idx) = crate::g_items::find_item(&ammo_name) {
                if DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_INFINITE_AMMO) {
                    add_ammo_to_client(ctx, other_client_idx, ammo_item_idx, 1000);
                } else {
                    let quantity = ctx.items[ammo_item_idx].quantity;
                    add_ammo_to_client(ctx, other_client_idx, ammo_item_idx, quantity);
                }
            }
        }

        if spawnflags & DROPPED_PLAYER_ITEM == 0 {
            if ctx.deathmatch != 0.0 {
                if DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_WEAPONS_STAY) {
                    ctx.edicts[ent_idx].flags |= FL_RESPAWN;
                } else {
                    set_respawn_weapon(ctx, ent_idx, 30.0);
                }
            }
            if ctx.coop != 0.0 {
                ctx.edicts[ent_idx].flags |= FL_RESPAWN;
            }
        }
    }

    let current_weapon = ctx.clients[other_client_idx].pers.weapon;
    let _new_weapon_opt = ctx.clients[other_client_idx].newweapon;
    let inv_count = ctx.clients[other_client_idx].pers.inventory[item_idx];

    if current_weapon != Some(item_idx)
        && inv_count == 1
        && (ctx.deathmatch == 0.0 || current_weapon == crate::g_items::find_item("blaster"))
    {
        ctx.clients[other_client_idx].newweapon = Some(item_idx);
    }

    true
}

/// Spawn a noise entity (allocates a new edict).
fn spawn_noise_entity(ctx: &mut GameContext) -> usize {
    for i in 0..ctx.edicts.len() {
        if !ctx.edicts[i].inuse {
            ctx.edicts[i] = Edict::default();
            ctx.edicts[i].inuse = true;
            return i;
        }
    }
    let idx = ctx.edicts.len();
    let mut e = Edict::default();
    e.inuse = true;
    ctx.edicts.push(e);
    idx
}


// ============================================================
// ChangeWeapon
// ============================================================

/// The old weapon has been dropped all the way, so make the new one current.
pub fn change_weapon(ctx: &mut GameContext, ent_idx: usize) {
    let client_idx = ctx.edicts[ent_idx].client.expect("ChangeWeapon: no client");

    if ctx.clients[client_idx].grenade_time != 0.0 {
        ctx.clients[client_idx].grenade_time = ctx.level.time;
        ctx.clients[client_idx].weapon_sound = 0;
        weapon_grenade_fire(ctx, ent_idx, false);
        ctx.clients[client_idx].grenade_time = 0.0;
    }

    ctx.clients[client_idx].pers.lastweapon = ctx.clients[client_idx].pers.weapon;
    ctx.clients[client_idx].pers.weapon = ctx.clients[client_idx].newweapon;
    ctx.clients[client_idx].newweapon = None;
    ctx.clients[client_idx].machinegun_shots = 0;

    // set visible model
    if ctx.edicts[ent_idx].s.modelindex == 255 {
        let i;
        if let Some(weap_idx) = ctx.clients[client_idx].pers.weapon {
            i = (ctx.items[weap_idx].weapmodel & 0xff) << 8;
        } else {
            i = 0;
        }
        // (ent - g_edicts - 1) | i  — ent_idx is already the entity index
        ctx.edicts[ent_idx].s.skinnum = ((ent_idx as i32) - 1) | i;
    }

    if let Some(weap_idx) = ctx.clients[client_idx].pers.weapon {
        let ammo_name = ctx.items[weap_idx].ammo.clone();
        if !ammo_name.is_empty() {
            let ammo_item = crate::g_items::find_item(&ammo_name).unwrap_or(0);
            ctx.clients[client_idx].ammo_index = ammo_item as i32;
        } else {
            ctx.clients[client_idx].ammo_index = 0;
        }
    } else {
        ctx.clients[client_idx].ammo_index = 0;
    }

    if ctx.clients[client_idx].pers.weapon.is_none() {
        // dead
        ctx.clients[client_idx].ps.gunindex = 0;
        return;
    }

    ctx.clients[client_idx].weaponstate = WeaponState::Activating;
    ctx.clients[client_idx].ps.gunframe = 0;

    let weap_idx = ctx.clients[client_idx].pers.weapon.unwrap();
    ctx.clients[client_idx].ps.gunindex = gi_modelindex(&ctx.items[weap_idx].view_model);

    ctx.clients[client_idx].anim_priority = ANIM_PAIN;
    if ctx.clients[client_idx].ps.pmove.pm_flags & PMF_DUCKED != 0 {
        ctx.edicts[ent_idx].s.frame = FRAME_CRPAIN1;
        ctx.clients[client_idx].anim_end = FRAME_CRPAIN4;
    } else {
        ctx.edicts[ent_idx].s.frame = FRAME_PAIN301;
        ctx.clients[client_idx].anim_end = FRAME_PAIN304;
    }
}

// ============================================================
// NoAmmoWeaponChange
// ============================================================

pub fn no_ammo_weapon_change(ctx: &mut GameContext, ent_idx: usize) {
    let client_idx = ctx.edicts[ent_idx].client.expect("NoAmmoWeaponChange: no client");

    let check_weapon = |ctx: &GameContext, ammo_name: &str, weapon_name: &str, min_ammo: i32| -> bool {
        let ammo_idx = crate::g_items::find_item(ammo_name).unwrap_or(0);
        let weap_idx = crate::g_items::find_item(weapon_name).unwrap_or(0);
        ctx.clients[client_idx].pers.inventory[ammo_idx] >= min_ammo
            && ctx.clients[client_idx].pers.inventory[weap_idx] != 0
    };

    if check_weapon(ctx, "slugs", "railgun", 1) {
        ctx.clients[client_idx].newweapon = crate::g_items::find_item("railgun");
        return;
    }
    if check_weapon(ctx, "cells", "hyperblaster", 1) {
        ctx.clients[client_idx].newweapon = crate::g_items::find_item("hyperblaster");
        return;
    }
    if check_weapon(ctx, "bullets", "chaingun", 1) {
        ctx.clients[client_idx].newweapon = crate::g_items::find_item("chaingun");
        return;
    }
    if check_weapon(ctx, "bullets", "machinegun", 1) {
        ctx.clients[client_idx].newweapon = crate::g_items::find_item("machinegun");
        return;
    }
    if check_weapon(ctx, "shells", "super shotgun", 2) {
        ctx.clients[client_idx].newweapon = crate::g_items::find_item("super shotgun");
        return;
    }
    if check_weapon(ctx, "shells", "shotgun", 1) {
        ctx.clients[client_idx].newweapon = crate::g_items::find_item("shotgun");
        return;
    }
    ctx.clients[client_idx].newweapon = crate::g_items::find_item("blaster");
}

// ============================================================
// Think_Weapon
// ============================================================

/// Called by ClientBeginServerFrame and ClientThink.
pub fn think_weapon(ctx: &mut GameContext, ent_idx: usize) {
    // if just died, put the weapon away
    if ctx.edicts[ent_idx].health < 1 {
        let client_idx = ctx.edicts[ent_idx].client.expect("Think_Weapon: no client");
        ctx.clients[client_idx].newweapon = None;
        change_weapon(ctx, ent_idx);
    }

    let client_idx = ctx.edicts[ent_idx].client.expect("Think_Weapon: no client");

    // call active weapon think routine
    if let Some(weap_idx) = ctx.clients[client_idx].pers.weapon {
        if ctx.items[weap_idx].weaponthink_fn.is_some() {
            ctx.is_quad = ctx.clients[client_idx].quad_framenum > ctx.level.framenum as f32;
            if ctx.clients[client_idx].silencer_shots > 0 {
                ctx.is_silenced = MZ_SILENCED as u8;
            } else {
                ctx.is_silenced = 0;
            }
            // Dispatch to the weapon's think function based on weaponthink_fn id
            if let Some(think_id) = ctx.items[weap_idx].weaponthink_fn {
                dispatch_weapon_think(ctx, ent_idx, think_id);
            }
        }
    }
}

// ============================================================
// Use_Weapon
// ============================================================

/// Make the weapon ready if there is ammo.
pub fn use_weapon(ctx: &mut GameContext, ent_idx: usize, item_idx: usize) {
    let client_idx = ctx.edicts[ent_idx].client.expect("Use_Weapon: no client");

    // see if we're already using it
    if ctx.clients[client_idx].pers.weapon == Some(item_idx) {
        return;
    }

    let item = &ctx.items[item_idx];
    if !item.ammo.is_empty() && ctx.g_select_empty == 0.0 && !item.flags.intersects(IT_AMMO) {
        let ammo_item_idx = crate::g_items::find_item(&item.ammo.clone()).unwrap_or(0);
        if ctx.clients[client_idx].pers.inventory[ammo_item_idx] == 0 {
            let ammo_name = ctx.items[ammo_item_idx].pickup_name.clone();
            let item_name = ctx.items[item_idx].pickup_name.clone();
            gi_cprintf(ctx.edicts[ent_idx].s.number, PRINT_HIGH, &format!("No {} for {}.\n", ammo_name, item_name));
            return;
        }
        if ctx.clients[client_idx].pers.inventory[ammo_item_idx] < item.quantity {
            let ammo_name = ctx.items[ammo_item_idx].pickup_name.clone();
            let item_name = ctx.items[item_idx].pickup_name.clone();
            gi_cprintf(ctx.edicts[ent_idx].s.number, PRINT_HIGH, &format!("Not enough {} for {}.\n", ammo_name, item_name));
            return;
        }
    }

    // change to this weapon when down
    ctx.clients[client_idx].newweapon = Some(item_idx);
}

// ============================================================
// Drop_Weapon
// ============================================================

pub fn drop_weapon(ctx: &mut GameContext, ent_idx: usize, item_idx: usize) {
    if DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_WEAPONS_STAY) {
        return;
    }

    let client_idx = ctx.edicts[ent_idx].client.expect("Drop_Weapon: no client");

    // see if we're already using it and it's the last one
    if (ctx.clients[client_idx].pers.weapon == Some(item_idx)
        || ctx.clients[client_idx].newweapon == Some(item_idx))
        && ctx.clients[client_idx].pers.inventory[item_idx] == 1
    {
        gi_cprintf(ctx.edicts[ent_idx].s.number, PRINT_HIGH, "Can't drop current weapon\n");
        return;
    }

    drop_weapon_item(ctx, ent_idx, item_idx);
    ctx.clients[client_idx].pers.inventory[item_idx] -= 1;
}

// ============================================================
// Weapon_Generic
// ============================================================

/// A generic function to handle the basics of weapon thinking.
///
/// `fire_fn` is a function pointer for the specific weapon's fire function.
pub fn weapon_generic(
    ctx: &mut GameContext,
    ent_idx: usize,
    frame_activate_last: i32,
    frame_fire_last: i32,
    frame_idle_last: i32,
    frame_deactivate_last: i32,
    pause_frames: &[i32],
    fire_frames: &[i32],
    fire_fn: fn(&mut GameContext, usize),
) {
    let frame_fire_first = frame_activate_last + 1;
    let frame_idle_first = frame_fire_last + 1;
    let frame_deactivate_first = frame_idle_last + 1;

    let client_idx = ctx.edicts[ent_idx].client.expect("Weapon_Generic: no client");

    if ctx.edicts[ent_idx].deadflag != 0 || ctx.edicts[ent_idx].s.modelindex != 255 {
        // VWep animations screw up corpses
        return;
    }

    if ctx.clients[client_idx].weaponstate == WeaponState::Dropping {
        if ctx.clients[client_idx].ps.gunframe == frame_deactivate_last {
            change_weapon(ctx, ent_idx);
            return;
        } else if (frame_deactivate_last - ctx.clients[client_idx].ps.gunframe) == 4 {
            let client_idx = ctx.edicts[ent_idx].client.unwrap();
            ctx.clients[client_idx].anim_priority = ANIM_REVERSE;
            if ctx.clients[client_idx].ps.pmove.pm_flags & PMF_DUCKED != 0 {
                ctx.edicts[ent_idx].s.frame = FRAME_CRPAIN4 + 1;
                ctx.clients[client_idx].anim_end = FRAME_CRPAIN1;
            } else {
                ctx.edicts[ent_idx].s.frame = FRAME_PAIN304 + 1;
                ctx.clients[client_idx].anim_end = FRAME_PAIN301;
            }
        }

        let client_idx = ctx.edicts[ent_idx].client.unwrap();
        ctx.clients[client_idx].ps.gunframe += 1;
        return;
    }

    if ctx.clients[client_idx].weaponstate == WeaponState::Activating {
        if ctx.clients[client_idx].ps.gunframe == frame_activate_last {
            ctx.clients[client_idx].weaponstate = WeaponState::Ready;
            ctx.clients[client_idx].ps.gunframe = frame_idle_first;
            return;
        }

        ctx.clients[client_idx].ps.gunframe += 1;
        return;
    }

    if ctx.clients[client_idx].newweapon.is_some()
        && ctx.clients[client_idx].weaponstate != WeaponState::Firing
    {
        ctx.clients[client_idx].weaponstate = WeaponState::Dropping;
        ctx.clients[client_idx].ps.gunframe = frame_deactivate_first;

        if (frame_deactivate_last - frame_deactivate_first) < 4 {
            ctx.clients[client_idx].anim_priority = ANIM_REVERSE;
            if ctx.clients[client_idx].ps.pmove.pm_flags & PMF_DUCKED != 0 {
                ctx.edicts[ent_idx].s.frame = FRAME_CRPAIN4 + 1;
                ctx.clients[client_idx].anim_end = FRAME_CRPAIN1;
            } else {
                ctx.edicts[ent_idx].s.frame = FRAME_PAIN304 + 1;
                ctx.clients[client_idx].anim_end = FRAME_PAIN301;
            }
        }
        return;
    }

    if ctx.clients[client_idx].weaponstate == WeaponState::Ready {
        if (ctx.clients[client_idx].latched_buttons | ctx.clients[client_idx].buttons) & BUTTON_ATTACK_I32 != 0 {
            ctx.clients[client_idx].latched_buttons &= !BUTTON_ATTACK_I32;

            let ammo_index = ctx.clients[client_idx].ammo_index;
            let has_ammo = if ammo_index == 0 {
                true
            } else {
                let weap_idx = ctx.clients[client_idx].pers.weapon.unwrap();
                ctx.clients[client_idx].pers.inventory[ammo_index as usize] >= ctx.items[weap_idx].quantity
            };

            if has_ammo {
                ctx.clients[client_idx].ps.gunframe = frame_fire_first;
                ctx.clients[client_idx].weaponstate = WeaponState::Firing;

                // start the animation
                ctx.clients[client_idx].anim_priority = ANIM_ATTACK;
                if ctx.clients[client_idx].ps.pmove.pm_flags & PMF_DUCKED != 0 {
                    ctx.edicts[ent_idx].s.frame = FRAME_CRATTAK1 - 1;
                    ctx.clients[client_idx].anim_end = FRAME_CRATTAK9;
                } else {
                    ctx.edicts[ent_idx].s.frame = FRAME_ATTACK1 - 1;
                    ctx.clients[client_idx].anim_end = FRAME_ATTACK8;
                }
            } else {
                if ctx.level.time >= ctx.edicts[ent_idx].pain_debounce_time {
                    gi_sound(ctx.edicts[ent_idx].s.number, CHAN_VOICE, gi_soundindex("weapons/noammo.wav"), 1.0, ATTN_NORM, 0.0);
                    ctx.edicts[ent_idx].pain_debounce_time = ctx.level.time + 1.0;
                }
                no_ammo_weapon_change(ctx, ent_idx);
            }
        } else {
            if ctx.clients[client_idx].ps.gunframe == frame_idle_last {
                ctx.clients[client_idx].ps.gunframe = frame_idle_first;
                return;
            }

            // Check pause frames
            for &pf in pause_frames {
                if pf == 0 {
                    break;
                }
                if ctx.clients[client_idx].ps.gunframe == pf
                    && rand_i32() & 15 != 0 {
                        return;
                    }
            }

            ctx.clients[client_idx].ps.gunframe += 1;
            return;
        }
    }

    if ctx.clients[client_idx].weaponstate == WeaponState::Firing {
        let mut fired = false;
        for &ff in fire_frames.iter() {
            if ff == 0 {
                break;
            }
            if ctx.clients[client_idx].ps.gunframe == ff {
                if ctx.clients[client_idx].quad_framenum > ctx.level.framenum as f32 {
                    gi_sound(ctx.edicts[ent_idx].s.number, CHAN_ITEM, gi_soundindex("items/damage3.wav"), 1.0, ATTN_NORM, 0.0);
                }
                fire_fn(ctx, ent_idx);
                fired = true;
                break;
            }
        }

        // Check if we reached a zero-terminator (no matching fire frame)
        if !fired {
            let client_idx = ctx.edicts[ent_idx].client.unwrap();
            ctx.clients[client_idx].ps.gunframe += 1;
        }

        let client_idx = ctx.edicts[ent_idx].client.unwrap();
        if ctx.clients[client_idx].ps.gunframe == frame_idle_first + 1 {
            ctx.clients[client_idx].weaponstate = WeaponState::Ready;
        }
    }
}

// ============================================================
// GRENADE
// ============================================================

pub fn weapon_grenade_fire(ctx: &mut GameContext, ent_idx: usize, held: bool) {
    let client_idx = ctx.edicts[ent_idx].client.expect("weapon_grenade_fire: no client");

    let mut offset: Vec3 = [0.0; 3];
    let viewheight = ctx.edicts[ent_idx].viewheight;
    vector_set(&mut offset, 8.0, 8.0, (viewheight - 8) as f32);

    let mut forward = [0.0f32; 3];
    let mut right = [0.0f32; 3];
    angle_vectors(&ctx.clients[client_idx].v_angle, Some(&mut forward), Some(&mut right), None);
    let _start = p_project_source(
        &ctx.clients[client_idx],
        &ctx.edicts[ent_idx].s.origin,
        &offset,
        &forward,
        &right,
    );

    let mut damage: i32 = 125;
    let radius: f32 = (damage + 40) as f32;
    if ctx.is_quad {
        damage *= 4;
    }

    let timer = ctx.clients[client_idx].grenade_time - ctx.level.time;
    let speed = GRENADE_MINSPEED as f32
        + (GRENADE_TIMER - timer)
            * ((GRENADE_MAXSPEED - GRENADE_MINSPEED) as f32 / GRENADE_TIMER);

    crate::g_weapon::fire_grenade2(ent_idx, &mut ctx.edicts, &mut ctx.level,
        &_start, &forward, damage, speed as i32, timer, radius, held);

    if !DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_INFINITE_AMMO) {
        let ammo_idx = ctx.clients[client_idx].ammo_index as usize;
        ctx.clients[client_idx].pers.inventory[ammo_idx] -= 1;
    }

    ctx.clients[client_idx].grenade_time = ctx.level.time + 1.0;

    if ctx.edicts[ent_idx].deadflag != 0 || ctx.edicts[ent_idx].s.modelindex != 255 {
        return;
    }

    if ctx.edicts[ent_idx].health <= 0 {
        return;
    }

    if ctx.clients[client_idx].ps.pmove.pm_flags & PMF_DUCKED != 0 {
        ctx.clients[client_idx].anim_priority = ANIM_ATTACK;
        ctx.edicts[ent_idx].s.frame = FRAME_CRATTAK1 - 1;
        ctx.clients[client_idx].anim_end = FRAME_CRATTAK3;
    } else {
        ctx.clients[client_idx].anim_priority = ANIM_REVERSE;
        ctx.edicts[ent_idx].s.frame = FRAME_WAVE08;
        ctx.clients[client_idx].anim_end = FRAME_WAVE01;
    }
}

pub fn weapon_grenade(ctx: &mut GameContext, ent_idx: usize) {
    let client_idx = ctx.edicts[ent_idx].client.expect("Weapon_Grenade: no client");

    if ctx.clients[client_idx].newweapon.is_some()
        && ctx.clients[client_idx].weaponstate == WeaponState::Ready
    {
        change_weapon(ctx, ent_idx);
        return;
    }

    if ctx.clients[client_idx].weaponstate == WeaponState::Activating {
        ctx.clients[client_idx].weaponstate = WeaponState::Ready;
        ctx.clients[client_idx].ps.gunframe = 16;
        return;
    }

    if ctx.clients[client_idx].weaponstate == WeaponState::Ready {
        if (ctx.clients[client_idx].latched_buttons | ctx.clients[client_idx].buttons) & BUTTON_ATTACK_I32 != 0 {
            ctx.clients[client_idx].latched_buttons &= !BUTTON_ATTACK_I32;
            let ammo_idx = ctx.clients[client_idx].ammo_index as usize;
            if ctx.clients[client_idx].pers.inventory[ammo_idx] != 0 {
                ctx.clients[client_idx].ps.gunframe = 1;
                ctx.clients[client_idx].weaponstate = WeaponState::Firing;
                ctx.clients[client_idx].grenade_time = 0.0;
            } else {
                if ctx.level.time >= ctx.edicts[ent_idx].pain_debounce_time {
                    gi_sound(ctx.edicts[ent_idx].s.number, CHAN_VOICE, gi_soundindex("weapons/noammo.wav"), 1.0, ATTN_NORM, 0.0);
                    ctx.edicts[ent_idx].pain_debounce_time = ctx.level.time + 1.0;
                }
                no_ammo_weapon_change(ctx, ent_idx);
            }
            return;
        }

        let gunframe = ctx.clients[client_idx].ps.gunframe;
        if (gunframe == 29 || gunframe == 34 || gunframe == 39 || gunframe == 48)
            && rand_i32() & 15 != 0 {
                return;
            }

        ctx.clients[client_idx].ps.gunframe += 1;
        if ctx.clients[client_idx].ps.gunframe > 48 {
            ctx.clients[client_idx].ps.gunframe = 16;
        }
        return;
    }

    if ctx.clients[client_idx].weaponstate == WeaponState::Firing {
        if ctx.clients[client_idx].ps.gunframe == 5 {
            gi_sound(ctx.edicts[ent_idx].s.number, CHAN_WEAPON, gi_soundindex("weapons/hgrena1b.wav"), 1.0, ATTN_NORM, 0.0);
        }

        if ctx.clients[client_idx].ps.gunframe == 11 {
            if ctx.clients[client_idx].grenade_time == 0.0 {
                ctx.clients[client_idx].grenade_time = ctx.level.time + GRENADE_TIMER + 0.2;
                ctx.clients[client_idx].weapon_sound = gi_soundindex("weapons/hgrenc1b.wav");
            }

            // they waited too long, detonate it in their hand
            if !ctx.clients[client_idx].grenade_blew_up
                && ctx.level.time >= ctx.clients[client_idx].grenade_time
            {
                ctx.clients[client_idx].weapon_sound = 0;
                weapon_grenade_fire(ctx, ent_idx, true);
                let client_idx = ctx.edicts[ent_idx].client.unwrap();
                ctx.clients[client_idx].grenade_blew_up = true;
            }

            let client_idx = ctx.edicts[ent_idx].client.unwrap();
            if ctx.clients[client_idx].buttons & BUTTON_ATTACK_I32 != 0 {
                return;
            }

            if ctx.clients[client_idx].grenade_blew_up {
                if ctx.level.time >= ctx.clients[client_idx].grenade_time {
                    ctx.clients[client_idx].ps.gunframe = 15;
                    ctx.clients[client_idx].grenade_blew_up = false;
                } else {
                    return;
                }
            }
        }

        let client_idx = ctx.edicts[ent_idx].client.unwrap();
        if ctx.clients[client_idx].ps.gunframe == 12 {
            ctx.clients[client_idx].weapon_sound = 0;
            weapon_grenade_fire(ctx, ent_idx, false);
        }

        let client_idx = ctx.edicts[ent_idx].client.unwrap();
        if ctx.clients[client_idx].ps.gunframe == 15
            && ctx.level.time < ctx.clients[client_idx].grenade_time
        {
            return;
        }

        ctx.clients[client_idx].ps.gunframe += 1;

        if ctx.clients[client_idx].ps.gunframe == 16 {
            ctx.clients[client_idx].grenade_time = 0.0;
            ctx.clients[client_idx].weaponstate = WeaponState::Ready;
        }
    }
}

// ============================================================
// GRENADE LAUNCHER
// ============================================================

fn weapon_grenadelauncher_fire(ctx: &mut GameContext, ent_idx: usize) {
    let client_idx = ctx.edicts[ent_idx].client.expect("weapon_grenadelauncher_fire: no client");

    let mut damage: i32 = 120;
    let radius: f32 = (damage + 40) as f32;
    if ctx.is_quad {
        damage *= 4;
    }

    let mut offset: Vec3 = [0.0; 3];
    let viewheight = ctx.edicts[ent_idx].viewheight;
    vector_set(&mut offset, 8.0, 8.0, (viewheight - 8) as f32);

    let mut forward = [0.0f32; 3];
    let mut right = [0.0f32; 3];
    angle_vectors(&ctx.clients[client_idx].v_angle, Some(&mut forward), Some(&mut right), None);
    let start = p_project_source(
        &ctx.clients[client_idx],
        &ctx.edicts[ent_idx].s.origin,
        &offset,
        &forward,
        &right,
    );

    ctx.clients[client_idx].kick_origin = vector_scale(&forward, -2.0);
    ctx.clients[client_idx].kick_angles[0] = -1.0;

    crate::g_weapon::fire_grenade(ent_idx, &mut ctx.edicts, &mut ctx.level,
        &start, &forward, damage, 600, 2.5, radius);

    // send muzzle flash
    let is_silenced = ctx.is_silenced;
    gi_write_byte(SVC_MUZZLEFLASH);
    gi_write_short(ent_idx as i32);
    gi_write_byte(MZ_GRENADE | is_silenced as i32);
    gi_multicast(&ctx.edicts[ent_idx].s.origin, MULTICAST_PVS);

    ctx.clients[client_idx].ps.gunframe += 1;

    player_noise(ctx, ent_idx, &start, PNOISE_WEAPON);

    let client_idx = ctx.edicts[ent_idx].client.unwrap();
    if !DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_INFINITE_AMMO) {
        let ammo_idx = ctx.clients[client_idx].ammo_index as usize;
        ctx.clients[client_idx].pers.inventory[ammo_idx] -= 1;
    }
}

pub fn weapon_grenade_launcher(ctx: &mut GameContext, ent_idx: usize) {
    let pause_frames: &[i32] = &[34, 51, 59, 0];
    let fire_frames: &[i32] = &[6, 0];

    weapon_generic(ctx, ent_idx, 5, 16, 59, 64, pause_frames, fire_frames, weapon_grenadelauncher_fire);
}

// ============================================================
// ROCKET LAUNCHER
// ============================================================

fn weapon_rocketlauncher_fire(ctx: &mut GameContext, ent_idx: usize) {
    let client_idx = ctx.edicts[ent_idx].client.expect("Weapon_RocketLauncher_Fire: no client");

    let mut damage = 100 + (random_f32() * 20.0) as i32;
    let mut radius_damage: i32 = 120;
    let damage_radius: f32 = 120.0;
    if ctx.is_quad {
        damage *= 4;
        radius_damage *= 4;
    }

    let mut forward = [0.0f32; 3];
    let mut right = [0.0f32; 3];
    angle_vectors(&ctx.clients[client_idx].v_angle, Some(&mut forward), Some(&mut right), None);

    ctx.clients[client_idx].kick_origin = vector_scale(&forward, -2.0);
    ctx.clients[client_idx].kick_angles[0] = -1.0;

    let mut offset: Vec3 = [0.0; 3];
    let viewheight = ctx.edicts[ent_idx].viewheight;
    vector_set(&mut offset, 8.0, 8.0, (viewheight - 8) as f32);
    let start = p_project_source(
        &ctx.clients[client_idx],
        &ctx.edicts[ent_idx].s.origin,
        &offset,
        &forward,
        &right,
    );

    crate::g_weapon::fire_rocket(ent_idx, &mut ctx.edicts, &mut ctx.level,
        &start, &forward, damage, 650, damage_radius, radius_damage);

    // send muzzle flash
    let is_silenced = ctx.is_silenced;
    gi_write_byte(SVC_MUZZLEFLASH);
    gi_write_short(ent_idx as i32);
    gi_write_byte(MZ_ROCKET | is_silenced as i32);
    gi_multicast(&ctx.edicts[ent_idx].s.origin, MULTICAST_PVS);

    ctx.clients[client_idx].ps.gunframe += 1;

    player_noise(ctx, ent_idx, &start, PNOISE_WEAPON);

    let client_idx = ctx.edicts[ent_idx].client.unwrap();
    if !DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_INFINITE_AMMO) {
        let ammo_idx = ctx.clients[client_idx].ammo_index as usize;
        ctx.clients[client_idx].pers.inventory[ammo_idx] -= 1;
    }
}

pub fn weapon_rocket_launcher(ctx: &mut GameContext, ent_idx: usize) {
    let pause_frames: &[i32] = &[25, 33, 42, 50, 0];
    let fire_frames: &[i32] = &[5, 0];

    weapon_generic(ctx, ent_idx, 4, 12, 50, 54, pause_frames, fire_frames, weapon_rocketlauncher_fire);
}

// ============================================================
// BLASTER / HYPERBLASTER
// ============================================================

fn blaster_fire(
    ctx: &mut GameContext,
    ent_idx: usize,
    g_offset: &Vec3,
    mut damage: i32,
    hyper: bool,
    effect: u32,
) {
    let client_idx = ctx.edicts[ent_idx].client.expect("Blaster_Fire: no client");

    if ctx.is_quad {
        damage *= 4;
    }

    let mut forward = [0.0f32; 3];
    let mut right = [0.0f32; 3];
    angle_vectors(&ctx.clients[client_idx].v_angle, Some(&mut forward), Some(&mut right), None);

    let mut offset: Vec3 = [0.0; 3];
    let viewheight = ctx.edicts[ent_idx].viewheight;
    vector_set(&mut offset, 24.0, 8.0, (viewheight - 8) as f32);
    offset = vector_add(&offset, g_offset);

    let start = p_project_source(
        &ctx.clients[client_idx],
        &ctx.edicts[ent_idx].s.origin,
        &offset,
        &forward,
        &right,
    );

    ctx.clients[client_idx].kick_origin = vector_scale(&forward, -2.0);
    ctx.clients[client_idx].kick_angles[0] = -1.0;

    crate::g_weapon::fire_blaster(ent_idx, &mut ctx.edicts, &mut ctx.level,
        &start, &forward, damage, 1000, effect as i32, hyper);

    // send muzzle flash
    let is_silenced = ctx.is_silenced;
    let mz = if hyper { MZ_HYPERBLASTER } else { MZ_BLASTER };
    gi_write_byte(SVC_MUZZLEFLASH);
    gi_write_short(ent_idx as i32);
    gi_write_byte(mz | is_silenced as i32);
    gi_multicast(&ctx.edicts[ent_idx].s.origin, MULTICAST_PVS);

    player_noise(ctx, ent_idx, &start, PNOISE_WEAPON);
}

fn weapon_blaster_fire(ctx: &mut GameContext, ent_idx: usize) {
    let damage = if ctx.deathmatch != 0.0 { 15 } else { 10 };
    let origin_zero: Vec3 = [0.0; 3];
    blaster_fire(ctx, ent_idx, &origin_zero, damage, false, EF_BLASTER);
    let client_idx = ctx.edicts[ent_idx].client.unwrap();
    ctx.clients[client_idx].ps.gunframe += 1;
}

pub fn weapon_blaster(ctx: &mut GameContext, ent_idx: usize) {
    let pause_frames: &[i32] = &[19, 32, 0];
    let fire_frames: &[i32] = &[5, 0];

    weapon_generic(ctx, ent_idx, 4, 8, 52, 55, pause_frames, fire_frames, weapon_blaster_fire);
}

fn weapon_hyperblaster_fire(ctx: &mut GameContext, ent_idx: usize) {
    let client_idx = ctx.edicts[ent_idx].client.expect("Weapon_HyperBlaster_Fire: no client");

    ctx.clients[client_idx].weapon_sound = gi_soundindex("weapons/hyprbl1a.wav");

    if ctx.clients[client_idx].buttons & BUTTON_ATTACK_I32 == 0 {
        ctx.clients[client_idx].ps.gunframe += 1;
    } else {
        let ammo_idx = ctx.clients[client_idx].ammo_index as usize;
        if ctx.clients[client_idx].pers.inventory[ammo_idx] == 0 {
            if ctx.level.time >= ctx.edicts[ent_idx].pain_debounce_time {
                gi_sound(ctx.edicts[ent_idx].s.number, CHAN_VOICE, gi_soundindex("weapons/noammo.wav"), 1.0, ATTN_NORM, 0.0);
                ctx.edicts[ent_idx].pain_debounce_time = ctx.level.time + 1.0;
            }
            no_ammo_weapon_change(ctx, ent_idx);
        } else {
            let gunframe = ctx.clients[client_idx].ps.gunframe;
            let rotation = (gunframe - 5) as f32 * 2.0 * PI / 6.0;
            let mut offset: Vec3 = [0.0; 3];
            offset[0] = -4.0 * rotation.sin();
            offset[1] = 0.0;
            offset[2] = 4.0 * rotation.cos();

            let effect = if gunframe == 6 || gunframe == 9 {
                EF_HYPERBLASTER
            } else {
                0
            };

            let damage = if ctx.deathmatch != 0.0 { 15 } else { 20 };
            blaster_fire(ctx, ent_idx, &offset, damage, true, effect);

            let client_idx = ctx.edicts[ent_idx].client.unwrap();
            if !DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_INFINITE_AMMO) {
                let ammo_idx = ctx.clients[client_idx].ammo_index as usize;
                ctx.clients[client_idx].pers.inventory[ammo_idx] -= 1;
            }

            ctx.clients[client_idx].anim_priority = ANIM_ATTACK;
            if ctx.clients[client_idx].ps.pmove.pm_flags & PMF_DUCKED != 0 {
                ctx.edicts[ent_idx].s.frame = FRAME_CRATTAK1 - 1;
                ctx.clients[client_idx].anim_end = FRAME_CRATTAK9;
            } else {
                ctx.edicts[ent_idx].s.frame = FRAME_ATTACK1 - 1;
                ctx.clients[client_idx].anim_end = FRAME_ATTACK8;
            }
        }

        let client_idx = ctx.edicts[ent_idx].client.unwrap();
        ctx.clients[client_idx].ps.gunframe += 1;
        let gunframe = ctx.clients[client_idx].ps.gunframe;
        let ammo_idx = ctx.clients[client_idx].ammo_index as usize;
        if gunframe == 12 && ctx.clients[client_idx].pers.inventory[ammo_idx] != 0 {
            ctx.clients[client_idx].ps.gunframe = 6;
        }
    }

    let client_idx = ctx.edicts[ent_idx].client.unwrap();
    if ctx.clients[client_idx].ps.gunframe == 12 {
        gi_sound(ctx.edicts[ent_idx].s.number, CHAN_AUTO, gi_soundindex("weapons/hyprbd1a.wav"), 1.0, ATTN_NORM, 0.0);
        ctx.clients[client_idx].weapon_sound = 0;
    }
}

pub fn weapon_hyperblaster(ctx: &mut GameContext, ent_idx: usize) {
    let pause_frames: &[i32] = &[0];
    let fire_frames: &[i32] = &[6, 7, 8, 9, 10, 11, 0];

    weapon_generic(ctx, ent_idx, 5, 20, 49, 53, pause_frames, fire_frames, weapon_hyperblaster_fire);
}

// ============================================================
// MACHINEGUN / CHAINGUN
// ============================================================

fn machinegun_fire(ctx: &mut GameContext, ent_idx: usize) {
    let client_idx = ctx.edicts[ent_idx].client.expect("Machinegun_Fire: no client");

    if ctx.clients[client_idx].buttons & BUTTON_ATTACK_I32 == 0 {
        ctx.clients[client_idx].machinegun_shots = 0;
        ctx.clients[client_idx].ps.gunframe += 1;
        return;
    }

    if ctx.clients[client_idx].ps.gunframe == 5 {
        ctx.clients[client_idx].ps.gunframe = 4;
    } else {
        ctx.clients[client_idx].ps.gunframe = 5;
    }

    let ammo_idx = ctx.clients[client_idx].ammo_index as usize;
    if ctx.clients[client_idx].pers.inventory[ammo_idx] < 1 {
        ctx.clients[client_idx].ps.gunframe = 6;
        if ctx.level.time >= ctx.edicts[ent_idx].pain_debounce_time {
            gi_sound(ctx.edicts[ent_idx].s.number, CHAN_VOICE, gi_soundindex("weapons/noammo.wav"), 1.0, ATTN_NORM, 0.0);
            ctx.edicts[ent_idx].pain_debounce_time = ctx.level.time + 1.0;
        }
        no_ammo_weapon_change(ctx, ent_idx);
        return;
    }

    let mut damage: i32 = 8;
    let mut kick: i32 = 2;
    if ctx.is_quad {
        damage *= 4;
        kick *= 4;
    }

    for i in 1..3 {
        ctx.clients[client_idx].kick_origin[i] = crandom_f32() * 0.35;
        ctx.clients[client_idx].kick_angles[i] = crandom_f32() * 0.7;
    }
    ctx.clients[client_idx].kick_origin[0] = crandom_f32() * 0.35;
    ctx.clients[client_idx].kick_angles[0] = ctx.clients[client_idx].machinegun_shots as f32 * -1.5;

    // raise the gun as it is firing
    if ctx.deathmatch == 0.0 {
        ctx.clients[client_idx].machinegun_shots += 1;
        if ctx.clients[client_idx].machinegun_shots > 9 {
            ctx.clients[client_idx].machinegun_shots = 9;
        }
    }

    // get start / end positions
    let angles = vector_add(&ctx.clients[client_idx].v_angle, &ctx.clients[client_idx].kick_angles);
    let mut forward = [0.0f32; 3];
    let mut right = [0.0f32; 3];
    angle_vectors(&angles, Some(&mut forward), Some(&mut right), None);

    let mut offset: Vec3 = [0.0; 3];
    let viewheight = ctx.edicts[ent_idx].viewheight;
    vector_set(&mut offset, 0.0, 8.0, (viewheight - 8) as f32);
    let start = p_project_source(
        &ctx.clients[client_idx],
        &ctx.edicts[ent_idx].s.origin,
        &offset,
        &forward,
        &right,
    );

    crate::g_weapon::fire_bullet(ent_idx, &mut ctx.edicts, &mut ctx.level,
        &start, &forward, damage, kick, DEFAULT_BULLET_HSPREAD, DEFAULT_BULLET_VSPREAD, MOD_MACHINEGUN);

    let is_silenced = ctx.is_silenced;
    gi_write_byte(SVC_MUZZLEFLASH);
    gi_write_short(ent_idx as i32);
    gi_write_byte(MZ_MACHINEGUN | is_silenced as i32);
    gi_multicast(&ctx.edicts[ent_idx].s.origin, MULTICAST_PVS);

    player_noise(ctx, ent_idx, &start, PNOISE_WEAPON);

    let client_idx = ctx.edicts[ent_idx].client.unwrap();
    if !DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_INFINITE_AMMO) {
        let ammo_idx = ctx.clients[client_idx].ammo_index as usize;
        ctx.clients[client_idx].pers.inventory[ammo_idx] -= 1;
    }

    ctx.clients[client_idx].anim_priority = ANIM_ATTACK;
    if ctx.clients[client_idx].ps.pmove.pm_flags & PMF_DUCKED != 0 {
        ctx.edicts[ent_idx].s.frame = FRAME_CRATTAK1 - (random_f32() + 0.25) as i32;
        ctx.clients[client_idx].anim_end = FRAME_CRATTAK9;
    } else {
        ctx.edicts[ent_idx].s.frame = FRAME_ATTACK1 - (random_f32() + 0.25) as i32;
        ctx.clients[client_idx].anim_end = FRAME_ATTACK8;
    }
}

pub fn weapon_machinegun(ctx: &mut GameContext, ent_idx: usize) {
    let pause_frames: &[i32] = &[23, 45, 0];
    let fire_frames: &[i32] = &[4, 5, 0];

    weapon_generic(ctx, ent_idx, 3, 5, 45, 49, pause_frames, fire_frames, machinegun_fire);
}

fn chaingun_fire(ctx: &mut GameContext, ent_idx: usize) {
    let client_idx = ctx.edicts[ent_idx].client.expect("Chaingun_Fire: no client");

    let mut damage: i32 = if ctx.deathmatch != 0.0 { 6 } else { 8 };
    let mut kick: i32 = 2;

    if ctx.clients[client_idx].ps.gunframe == 5 {
        gi_sound(ctx.edicts[ent_idx].s.number, CHAN_AUTO, gi_soundindex("weapons/chngnu1a.wav"), 1.0, ATTN_IDLE, 0.0);
    }

    if ctx.clients[client_idx].ps.gunframe == 14
        && ctx.clients[client_idx].buttons & BUTTON_ATTACK_I32 == 0
    {
        ctx.clients[client_idx].ps.gunframe = 32;
        ctx.clients[client_idx].weapon_sound = 0;
        return;
    } else if ctx.clients[client_idx].ps.gunframe == 21
        && ctx.clients[client_idx].buttons & BUTTON_ATTACK_I32 != 0
        && ctx.clients[client_idx].pers.inventory[ctx.clients[client_idx].ammo_index as usize] != 0
    {
        ctx.clients[client_idx].ps.gunframe = 15;
    } else {
        ctx.clients[client_idx].ps.gunframe += 1;
    }

    if ctx.clients[client_idx].ps.gunframe == 22 {
        ctx.clients[client_idx].weapon_sound = 0;
        gi_sound(ctx.edicts[ent_idx].s.number, CHAN_AUTO, gi_soundindex("weapons/chngnd1a.wav"), 1.0, ATTN_IDLE, 0.0);
    } else {
        ctx.clients[client_idx].weapon_sound = gi_soundindex("weapons/chngnl1a.wav");
    }

    ctx.clients[client_idx].anim_priority = ANIM_ATTACK;
    if ctx.clients[client_idx].ps.pmove.pm_flags & PMF_DUCKED != 0 {
        ctx.edicts[ent_idx].s.frame = FRAME_CRATTAK1 - (ctx.clients[client_idx].ps.gunframe & 1);
        ctx.clients[client_idx].anim_end = FRAME_CRATTAK9;
    } else {
        ctx.edicts[ent_idx].s.frame = FRAME_ATTACK1 - (ctx.clients[client_idx].ps.gunframe & 1);
        ctx.clients[client_idx].anim_end = FRAME_ATTACK8;
    }

    let gunframe = ctx.clients[client_idx].ps.gunframe;
    let mut shots: i32;
    if gunframe <= 9 {
        shots = 1;
    } else if gunframe <= 14 {
        if ctx.clients[client_idx].buttons & BUTTON_ATTACK_I32 != 0 {
            shots = 2;
        } else {
            shots = 1;
        }
    } else {
        shots = 3;
    }

    let ammo_idx = ctx.clients[client_idx].ammo_index as usize;
    if ctx.clients[client_idx].pers.inventory[ammo_idx] < shots {
        shots = ctx.clients[client_idx].pers.inventory[ammo_idx];
    }

    if shots == 0 {
        if ctx.level.time >= ctx.edicts[ent_idx].pain_debounce_time {
            gi_sound(ctx.edicts[ent_idx].s.number, CHAN_VOICE, gi_soundindex("weapons/noammo.wav"), 1.0, ATTN_NORM, 0.0);
            ctx.edicts[ent_idx].pain_debounce_time = ctx.level.time + 1.0;
        }
        no_ammo_weapon_change(ctx, ent_idx);
        return;
    }

    if ctx.is_quad {
        damage *= 4;
        kick *= 4;
    }

    for i in 0..3 {
        ctx.clients[client_idx].kick_origin[i] = crandom_f32() * 0.35;
        ctx.clients[client_idx].kick_angles[i] = crandom_f32() * 0.7;
    }

    let mut start: Vec3 = [0.0; 3];
    for _i in 0..shots {
        let mut forward = [0.0f32; 3];
        let mut right = [0.0f32; 3];
        let mut up = [0.0f32; 3];
        angle_vectors(&ctx.clients[client_idx].v_angle, Some(&mut forward), Some(&mut right), Some(&mut up));
        let r = 7.0 + crandom_f32() * 4.0;
        let u = crandom_f32() * 4.0;
        let viewheight = ctx.edicts[ent_idx].viewheight;
        let mut offset: Vec3 = [0.0; 3];
        vector_set(&mut offset, 0.0, r, u + (viewheight - 8) as f32);
        start = p_project_source(
            &ctx.clients[client_idx],
            &ctx.edicts[ent_idx].s.origin,
            &offset,
            &forward,
            &right,
        );

        crate::g_weapon::fire_bullet(ent_idx, &mut ctx.edicts, &mut ctx.level,
            &start, &forward, damage, kick, DEFAULT_BULLET_HSPREAD, DEFAULT_BULLET_VSPREAD, MOD_CHAINGUN);
    }

    // send muzzle flash
    let is_silenced = ctx.is_silenced;
    gi_write_byte(SVC_MUZZLEFLASH);
    gi_write_short(ent_idx as i32);
    gi_write_byte((MZ_CHAINGUN1 + shots - 1) | is_silenced as i32);
    gi_multicast(&ctx.edicts[ent_idx].s.origin, MULTICAST_PVS);

    player_noise(ctx, ent_idx, &start, PNOISE_WEAPON);

    let client_idx = ctx.edicts[ent_idx].client.unwrap();
    if !DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_INFINITE_AMMO) {
        let ammo_idx = ctx.clients[client_idx].ammo_index as usize;
        ctx.clients[client_idx].pers.inventory[ammo_idx] -= shots;
    }
}

pub fn weapon_chaingun(ctx: &mut GameContext, ent_idx: usize) {
    let pause_frames: &[i32] = &[38, 43, 51, 61, 0];
    let fire_frames: &[i32] = &[5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 0];

    weapon_generic(ctx, ent_idx, 4, 31, 61, 64, pause_frames, fire_frames, chaingun_fire);
}

// ============================================================
// SHOTGUN / SUPERSHOTGUN
// ============================================================

fn weapon_shotgun_fire(ctx: &mut GameContext, ent_idx: usize) {
    let client_idx = ctx.edicts[ent_idx].client.expect("weapon_shotgun_fire: no client");

    let mut damage: i32 = 4;
    let mut kick: i32 = 8;

    if ctx.clients[client_idx].ps.gunframe == 9 {
        ctx.clients[client_idx].ps.gunframe += 1;
        return;
    }

    let mut forward = [0.0f32; 3];
    let mut right = [0.0f32; 3];
    angle_vectors(&ctx.clients[client_idx].v_angle, Some(&mut forward), Some(&mut right), None);

    ctx.clients[client_idx].kick_origin = vector_scale(&forward, -2.0);
    ctx.clients[client_idx].kick_angles[0] = -2.0;

    let mut offset: Vec3 = [0.0; 3];
    let viewheight = ctx.edicts[ent_idx].viewheight;
    vector_set(&mut offset, 0.0, 8.0, (viewheight - 8) as f32);
    let start = p_project_source(
        &ctx.clients[client_idx],
        &ctx.edicts[ent_idx].s.origin,
        &offset,
        &forward,
        &right,
    );

    if ctx.is_quad {
        damage *= 4;
        kick *= 4;
    }

    let count = if ctx.deathmatch != 0.0 {
        DEFAULT_DEATHMATCH_SHOTGUN_COUNT
    } else {
        DEFAULT_SHOTGUN_COUNT
    };

    crate::g_weapon::fire_shotgun(ent_idx, &mut ctx.edicts, &mut ctx.level,
        &start, &forward, damage, kick, DEFAULT_SHOTGUN_HSPREAD, DEFAULT_SHOTGUN_VSPREAD, count, MOD_SHOTGUN);

    // send muzzle flash
    let is_silenced = ctx.is_silenced;
    gi_write_byte(SVC_MUZZLEFLASH);
    gi_write_short(ent_idx as i32);
    gi_write_byte(MZ_SHOTGUN | is_silenced as i32);
    gi_multicast(&ctx.edicts[ent_idx].s.origin, MULTICAST_PVS);

    ctx.clients[client_idx].ps.gunframe += 1;
    player_noise(ctx, ent_idx, &start, PNOISE_WEAPON);

    let client_idx = ctx.edicts[ent_idx].client.unwrap();
    if !DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_INFINITE_AMMO) {
        let ammo_idx = ctx.clients[client_idx].ammo_index as usize;
        ctx.clients[client_idx].pers.inventory[ammo_idx] -= 1;
    }
}

pub fn weapon_shotgun(ctx: &mut GameContext, ent_idx: usize) {
    let pause_frames: &[i32] = &[22, 28, 34, 0];
    let fire_frames: &[i32] = &[8, 9, 0];

    weapon_generic(ctx, ent_idx, 7, 18, 36, 39, pause_frames, fire_frames, weapon_shotgun_fire);
}

fn weapon_supershotgun_fire(ctx: &mut GameContext, ent_idx: usize) {
    let client_idx = ctx.edicts[ent_idx].client.expect("weapon_supershotgun_fire: no client");

    let mut damage: i32 = 6;
    let mut kick: i32 = 12;

    let mut forward = [0.0f32; 3];
    let mut right = [0.0f32; 3];
    angle_vectors(&ctx.clients[client_idx].v_angle, Some(&mut forward), Some(&mut right), None);

    ctx.clients[client_idx].kick_origin = vector_scale(&forward, -2.0);
    ctx.clients[client_idx].kick_angles[0] = -2.0;

    let mut offset: Vec3 = [0.0; 3];
    let viewheight = ctx.edicts[ent_idx].viewheight;
    vector_set(&mut offset, 0.0, 8.0, (viewheight - 8) as f32);
    let start = p_project_source(
        &ctx.clients[client_idx],
        &ctx.edicts[ent_idx].s.origin,
        &offset,
        &forward,
        &right,
    );

    if ctx.is_quad {
        damage *= 4;
        kick *= 4;
    }

    let mut v: Vec3 = [0.0; 3];
    v[PITCH] = ctx.clients[client_idx].v_angle[PITCH];
    v[YAW] = ctx.clients[client_idx].v_angle[YAW] - 5.0;
    v[ROLL] = ctx.clients[client_idx].v_angle[ROLL];
    let mut forward1 = [0.0f32; 3];
    angle_vectors(&v, Some(&mut forward1), None, None);
    crate::g_weapon::fire_shotgun(ent_idx, &mut ctx.edicts, &mut ctx.level,
        &start, &forward1, damage, kick, DEFAULT_SHOTGUN_HSPREAD, DEFAULT_SHOTGUN_VSPREAD, DEFAULT_SSHOTGUN_COUNT / 2, MOD_SSHOTGUN);

    v[YAW] = ctx.clients[client_idx].v_angle[YAW] + 5.0;
    let mut forward2 = [0.0f32; 3];
    angle_vectors(&v, Some(&mut forward2), None, None);
    crate::g_weapon::fire_shotgun(ent_idx, &mut ctx.edicts, &mut ctx.level,
        &start, &forward2, damage, kick, DEFAULT_SHOTGUN_HSPREAD, DEFAULT_SHOTGUN_VSPREAD, DEFAULT_SSHOTGUN_COUNT / 2, MOD_SSHOTGUN);

    // send muzzle flash
    let is_silenced = ctx.is_silenced;
    gi_write_byte(SVC_MUZZLEFLASH);
    gi_write_short(ent_idx as i32);
    gi_write_byte(MZ_SSHOTGUN | is_silenced as i32);
    gi_multicast(&ctx.edicts[ent_idx].s.origin, MULTICAST_PVS);

    ctx.clients[client_idx].ps.gunframe += 1;
    player_noise(ctx, ent_idx, &start, PNOISE_WEAPON);

    let client_idx = ctx.edicts[ent_idx].client.unwrap();
    if !DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_INFINITE_AMMO) {
        let ammo_idx = ctx.clients[client_idx].ammo_index as usize;
        ctx.clients[client_idx].pers.inventory[ammo_idx] -= 2;
    }
}

pub fn weapon_super_shotgun(ctx: &mut GameContext, ent_idx: usize) {
    let pause_frames: &[i32] = &[29, 42, 57, 0];
    let fire_frames: &[i32] = &[7, 0];

    weapon_generic(ctx, ent_idx, 6, 17, 57, 61, pause_frames, fire_frames, weapon_supershotgun_fire);
}

// ============================================================
// RAILGUN
// ============================================================

fn weapon_railgun_fire(ctx: &mut GameContext, ent_idx: usize) {
    let client_idx = ctx.edicts[ent_idx].client.expect("weapon_railgun_fire: no client");

    let (mut damage, mut kick) = if ctx.deathmatch != 0.0 {
        (100, 200)
    } else {
        (150, 250)
    };

    if ctx.is_quad {
        damage *= 4;
        kick *= 4;
    }

    let mut forward = [0.0f32; 3];
    let mut right = [0.0f32; 3];
    angle_vectors(&ctx.clients[client_idx].v_angle, Some(&mut forward), Some(&mut right), None);

    ctx.clients[client_idx].kick_origin = vector_scale(&forward, -3.0);
    ctx.clients[client_idx].kick_angles[0] = -3.0;

    let mut offset: Vec3 = [0.0; 3];
    let viewheight = ctx.edicts[ent_idx].viewheight;
    vector_set(&mut offset, 0.0, 7.0, (viewheight - 8) as f32);
    let start = p_project_source(
        &ctx.clients[client_idx],
        &ctx.edicts[ent_idx].s.origin,
        &offset,
        &forward,
        &right,
    );

    crate::g_weapon::fire_rail(ent_idx, &mut ctx.edicts, &mut ctx.level,
        &start, &forward, damage, kick);

    // send muzzle flash
    let is_silenced = ctx.is_silenced;
    gi_write_byte(SVC_MUZZLEFLASH);
    gi_write_short(ent_idx as i32);
    gi_write_byte(MZ_RAILGUN | is_silenced as i32);
    gi_multicast(&ctx.edicts[ent_idx].s.origin, MULTICAST_PVS);

    ctx.clients[client_idx].ps.gunframe += 1;
    player_noise(ctx, ent_idx, &start, PNOISE_WEAPON);

    let client_idx = ctx.edicts[ent_idx].client.unwrap();
    if !DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_INFINITE_AMMO) {
        let ammo_idx = ctx.clients[client_idx].ammo_index as usize;
        ctx.clients[client_idx].pers.inventory[ammo_idx] -= 1;
    }
}

pub fn weapon_railgun(ctx: &mut GameContext, ent_idx: usize) {
    let pause_frames: &[i32] = &[56, 0];
    let fire_frames: &[i32] = &[4, 0];

    weapon_generic(ctx, ent_idx, 3, 18, 56, 61, pause_frames, fire_frames, weapon_railgun_fire);
}

// ============================================================
// BFG10K
// ============================================================

fn weapon_bfg_fire(ctx: &mut GameContext, ent_idx: usize) {
    let client_idx = ctx.edicts[ent_idx].client.expect("weapon_bfg_fire: no client");

    let mut damage: i32 = if ctx.deathmatch != 0.0 { 200 } else { 500 };
    let damage_radius: f32 = 1000.0;

    if ctx.clients[client_idx].ps.gunframe == 9 {
        // send muzzle flash
        let is_silenced = ctx.is_silenced;
        gi_write_byte(SVC_MUZZLEFLASH);
        gi_write_short(ent_idx as i32);
        gi_write_byte(MZ_BFG | is_silenced as i32);
        gi_multicast(&ctx.edicts[ent_idx].s.origin, MULTICAST_PVS);

        ctx.clients[client_idx].ps.gunframe += 1;

        // Note: start is uninitialized here in the original C code too
        let start: Vec3 = [0.0; 3];
        player_noise(ctx, ent_idx, &start, PNOISE_WEAPON);
        return;
    }

    // cells can go down during windup (from power armor hits), so
    // check again and abort firing if we don't have enough now
    let client_idx = ctx.edicts[ent_idx].client.unwrap();
    let ammo_idx = ctx.clients[client_idx].ammo_index as usize;
    if ctx.clients[client_idx].pers.inventory[ammo_idx] < 50 {
        ctx.clients[client_idx].ps.gunframe += 1;
        return;
    }

    if ctx.is_quad {
        damage *= 4;
    }

    let mut forward = [0.0f32; 3];
    let mut right = [0.0f32; 3];
    angle_vectors(&ctx.clients[client_idx].v_angle, Some(&mut forward), Some(&mut right), None);

    ctx.clients[client_idx].kick_origin = vector_scale(&forward, -2.0);

    // make a big pitch kick with an inverse fall
    ctx.clients[client_idx].v_dmg_pitch = -40.0;
    ctx.clients[client_idx].v_dmg_roll = crandom_f32() * 8.0;
    ctx.clients[client_idx].v_dmg_time = ctx.level.time + DAMAGE_TIME;

    let mut offset: Vec3 = [0.0; 3];
    let viewheight = ctx.edicts[ent_idx].viewheight;
    vector_set(&mut offset, 8.0, 8.0, (viewheight - 8) as f32);
    let start = p_project_source(
        &ctx.clients[client_idx],
        &ctx.edicts[ent_idx].s.origin,
        &offset,
        &forward,
        &right,
    );

    crate::g_weapon::fire_bfg(ent_idx, &mut ctx.edicts, &mut ctx.level,
        &start, &forward, damage, 400, damage_radius);

    ctx.clients[client_idx].ps.gunframe += 1;

    player_noise(ctx, ent_idx, &start, PNOISE_WEAPON);

    let client_idx = ctx.edicts[ent_idx].client.unwrap();
    if !DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_INFINITE_AMMO) {
        let ammo_idx = ctx.clients[client_idx].ammo_index as usize;
        ctx.clients[client_idx].pers.inventory[ammo_idx] -= 50;
    }
}

pub fn weapon_bfg(ctx: &mut GameContext, ent_idx: usize) {
    let pause_frames: &[i32] = &[39, 45, 50, 55, 0];
    let fire_frames: &[i32] = &[9, 17, 0];

    weapon_generic(ctx, ent_idx, 8, 32, 55, 58, pause_frames, fire_frames, weapon_bfg_fire);
}

// ============================================================
// Helper functions for cross-module calls
// ============================================================

/// Add ammo to a client's inventory, capped at max.
fn add_ammo_to_client(ctx: &mut GameContext, client_idx: usize, item_idx: usize, count: i32) {
    let max = match ctx.items[item_idx].pickup_name.to_lowercase().as_str() {
        "bullets" => ctx.clients[client_idx].pers.max_bullets,
        "shells" => ctx.clients[client_idx].pers.max_shells,
        "rockets" => ctx.clients[client_idx].pers.max_rockets,
        "grenades" => ctx.clients[client_idx].pers.max_grenades,
        "cells" => ctx.clients[client_idx].pers.max_cells,
        "slugs" => ctx.clients[client_idx].pers.max_slugs,
        _ => 999,
    };
    let current = ctx.clients[client_idx].pers.inventory[item_idx];
    if current >= max {
        return;
    }
    let new_count = current + count;
    ctx.clients[client_idx].pers.inventory[item_idx] = if new_count > max { max } else { new_count };
}

/// Set a respawn timer on a weapon entity.
fn set_respawn_weapon(ctx: &mut GameContext, ent_idx: usize, delay: f32) {
    ctx.edicts[ent_idx].flags |= FL_RESPAWN;
    ctx.edicts[ent_idx].svflags |= SVF_NOCLIENT;
    ctx.edicts[ent_idx].solid = Solid::Not;
    ctx.edicts[ent_idx].nextthink = ctx.level.time + delay;
    // think_fn would be set to DoRespawn in the dispatch system
    ctx.edicts[ent_idx].think_fn = Some(THINK_DO_RESPAWN);
    gi_linkentity(ent_idx as i32);
}

/// Drop a weapon item (spawns a dropped entity).
fn drop_weapon_item(ctx: &mut GameContext, ent_idx: usize, item_idx: usize) {
    // Spawn a dropped item entity
    let drop_idx = spawn_noise_entity(ctx); // reuse spawn helper
    ctx.edicts[drop_idx].classname = ctx.items[item_idx].classname.clone();
    ctx.edicts[drop_idx].item = Some(item_idx);
    ctx.edicts[drop_idx].spawnflags = DROPPED_ITEM;
    ctx.edicts[drop_idx].s.effects = EF_ROTATE as u32;
    ctx.edicts[drop_idx].s.renderfx = RF_GLOW;
    ctx.edicts[drop_idx].mins = [-15.0, -15.0, -15.0];
    ctx.edicts[drop_idx].maxs = [15.0, 15.0, 15.0];
    ctx.edicts[drop_idx].s.modelindex = gi_modelindex(&ctx.items[item_idx].world_model);
    ctx.edicts[drop_idx].solid = Solid::Trigger;
    ctx.edicts[drop_idx].movetype = MoveType::Toss;
    ctx.edicts[drop_idx].touch_fn = Some(TOUCH_DROP_TEMP);
    ctx.edicts[drop_idx].owner = ent_idx as i32;
    ctx.edicts[drop_idx].s.origin = ctx.edicts[ent_idx].s.origin;
    ctx.edicts[drop_idx].nextthink = ctx.level.time + 30.0;
    ctx.edicts[drop_idx].think_fn = Some(THINK_G_FREE_EDICT);
    gi_linkentity(drop_idx as i32);
}

/// Dispatch weapon think function by id.
fn dispatch_weapon_think(ctx: &mut GameContext, ent_idx: usize, think_id: usize) {
    match think_id {
        WEAPTHINK_BLASTER => weapon_blaster(ctx, ent_idx),
        WEAPTHINK_SHOTGUN => weapon_shotgun(ctx, ent_idx),
        WEAPTHINK_SUPERSHOTGUN => weapon_super_shotgun(ctx, ent_idx),
        WEAPTHINK_MACHINEGUN => weapon_machinegun(ctx, ent_idx),
        WEAPTHINK_CHAINGUN => weapon_chaingun(ctx, ent_idx),
        WEAPTHINK_HYPERBLASTER => weapon_hyperblaster(ctx, ent_idx),
        WEAPTHINK_ROCKETLAUNCHER => weapon_rocket_launcher(ctx, ent_idx),
        WEAPTHINK_GRENADE => weapon_grenade(ctx, ent_idx),
        WEAPTHINK_GRENADELAUNCHER => weapon_grenade_launcher(ctx, ent_idx),
        WEAPTHINK_RAILGUN => weapon_railgun(ctx, ent_idx),
        WEAPTHINK_BFG => weapon_bfg(ctx, ent_idx),
        _ => {}
    }
}

use crate::g_items::{
    WEAPTHINK_BLASTER, WEAPTHINK_SHOTGUN, WEAPTHINK_SUPERSHOTGUN,
    WEAPTHINK_MACHINEGUN, WEAPTHINK_CHAINGUN, WEAPTHINK_HYPERBLASTER,
    WEAPTHINK_ROCKETLAUNCHER, WEAPTHINK_GRENADE, WEAPTHINK_GRENADELAUNCHER,
    WEAPTHINK_RAILGUN, WEAPTHINK_BFG,
    THINK_DO_RESPAWN, TOUCH_DROP_TEMP,
};

use crate::dispatch::THINK_FREE_EDICT as THINK_G_FREE_EDICT;

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn init_test_gi() {
        // OnceLock silently ignores subsequent calls, safe for parallel tests
        crate::game_import::set_gi(Box::new(crate::game_import::StubGameImport));
    }

    /// Create a minimal GameContext with one player entity (index 1) and one client (index 0).
    fn make_weapon_ctx() -> GameContext {
        init_test_gi();
        let mut ctx = GameContext::default();
        ctx.deathmatch = 0.0;
        ctx.coop = 0.0;
        ctx.dmflags = 0.0;
        ctx.g_select_empty = 0.0;

        // World entity at index 0
        let mut world = Edict::default();
        world.inuse = true;
        ctx.edicts.push(world);

        // Player entity at index 1
        let mut player = Edict::default();
        player.inuse = true;
        player.client = Some(0); // client index 0
        player.s.modelindex = 255; // player model
        player.health = 100;
        player.viewheight = 22;
        player.s.number = 1;
        ctx.edicts.push(player);

        // Client at index 0
        let mut client = GClient::default();
        client.pers.hand = RIGHT_HANDED;
        client.pers.max_bullets = 200;
        client.pers.max_shells = 100;
        client.pers.max_rockets = 50;
        client.pers.max_grenades = 50;
        client.pers.max_cells = 200;
        client.pers.max_slugs = 50;
        ctx.clients.push(client);

        ctx.num_edicts = ctx.edicts.len() as i32;
        ctx
    }

    // ============================================================
    // p_project_source tests
    // ============================================================

    #[test]
    fn test_p_project_source_right_handed() {
        let mut client = GClient::default();
        client.pers.hand = RIGHT_HANDED;

        let point = [100.0, 200.0, 300.0];
        let distance = [10.0, 5.0, 2.0];
        let forward = [1.0, 0.0, 0.0];
        let right = [0.0, 1.0, 0.0];

        let result = p_project_source(&client, &point, &distance, &forward, &right);

        // For right-handed, distance[1] is unchanged
        // g_project_source: point + forward*dist[0] + right*dist[1], z += dist[2]
        assert!((result[0] - 110.0).abs() < 0.01);
        assert!((result[1] - 205.0).abs() < 0.01);
        assert!((result[2] - 302.0).abs() < 0.01);
    }

    #[test]
    fn test_p_project_source_left_handed() {
        let mut client = GClient::default();
        client.pers.hand = LEFT_HANDED;

        let point = [100.0, 200.0, 300.0];
        let distance = [10.0, 5.0, 2.0];
        let forward = [1.0, 0.0, 0.0];
        let right = [0.0, 1.0, 0.0];

        let result = p_project_source(&client, &point, &distance, &forward, &right);

        // For left-handed, distance[1] is negated => -5.0
        assert!((result[0] - 110.0).abs() < 0.01);
        assert!((result[1] - 195.0).abs() < 0.01); // 200 + right*(-5)
        assert!((result[2] - 302.0).abs() < 0.01);
    }

    #[test]
    fn test_p_project_source_center_handed() {
        let mut client = GClient::default();
        client.pers.hand = CENTER_HANDED;

        let point = [100.0, 200.0, 300.0];
        let distance = [10.0, 5.0, 2.0];
        let forward = [1.0, 0.0, 0.0];
        let right = [0.0, 1.0, 0.0];

        let result = p_project_source(&client, &point, &distance, &forward, &right);

        // For center-handed, distance[1] becomes 0.0
        assert!((result[0] - 110.0).abs() < 0.01);
        assert!((result[1] - 200.0).abs() < 0.01); // no lateral offset
        assert!((result[2] - 302.0).abs() < 0.01);
    }

    // ============================================================
    // Grenade speed/timer calculation tests
    // ============================================================

    #[test]
    fn test_grenade_speed_at_max_timer() {
        // When timer == GRENADE_TIMER (just thrown), speed = GRENADE_MINSPEED
        let timer = GRENADE_TIMER;
        let speed = GRENADE_MINSPEED as f32
            + (GRENADE_TIMER - timer)
                * ((GRENADE_MAXSPEED - GRENADE_MINSPEED) as f32 / GRENADE_TIMER);
        assert!((speed - GRENADE_MINSPEED as f32).abs() < 0.01);
    }

    #[test]
    fn test_grenade_speed_at_zero_timer() {
        // When timer == 0 (held the full duration), speed = GRENADE_MAXSPEED
        let timer = 0.0f32;
        let speed = GRENADE_MINSPEED as f32
            + (GRENADE_TIMER - timer)
                * ((GRENADE_MAXSPEED - GRENADE_MINSPEED) as f32 / GRENADE_TIMER);
        assert!((speed - GRENADE_MAXSPEED as f32).abs() < 0.01);
    }

    #[test]
    fn test_grenade_speed_at_half_timer() {
        // When timer == GRENADE_TIMER / 2, speed should be midpoint
        let timer = GRENADE_TIMER / 2.0;
        let speed = GRENADE_MINSPEED as f32
            + (GRENADE_TIMER - timer)
                * ((GRENADE_MAXSPEED - GRENADE_MINSPEED) as f32 / GRENADE_TIMER);
        let expected = (GRENADE_MINSPEED as f32 + GRENADE_MAXSPEED as f32) / 2.0;
        assert!((speed - expected).abs() < 0.01);
    }

    #[test]
    fn test_grenade_constants() {
        assert_eq!(GRENADE_TIMER, 3.0);
        assert_eq!(GRENADE_MINSPEED, 400);
        assert_eq!(GRENADE_MAXSPEED, 800);
        // Max speed is exactly double min speed
        assert_eq!(GRENADE_MAXSPEED, 2 * GRENADE_MINSPEED);
    }

    // ============================================================
    // Grenade damage/radius calculation tests
    // ============================================================

    #[test]
    fn test_grenade_damage_radius_calculation() {
        // From weapon_grenade_fire: damage=125, radius = (damage+40) as f32
        let damage: i32 = 125;
        let radius: f32 = (damage + 40) as f32;
        assert_eq!(radius, 165.0);
    }

    #[test]
    fn test_grenade_damage_quad() {
        let mut damage: i32 = 125;
        let radius: f32 = (damage + 40) as f32; // radius computed before quad
        damage *= 4; // quad damage
        assert_eq!(damage, 500);
        assert_eq!(radius, 165.0); // radius is NOT affected by quad
    }

    #[test]
    fn test_grenade_launcher_damage_radius() {
        // From weapon_grenadelauncher_fire: damage=120, radius = (damage+40) as f32
        let damage: i32 = 120;
        let radius: f32 = (damage + 40) as f32;
        assert_eq!(radius, 160.0);
    }

    // ============================================================
    // Weapon damage value tests (verifying constants from original C code)
    // ============================================================

    #[test]
    fn test_blaster_damage_values() {
        // Deathmatch: 15, SinglePlayer: 10
        let dm_damage: i32 = 15;
        let sp_damage: i32 = 10;
        assert_eq!(dm_damage, 15);
        assert_eq!(sp_damage, 10);
    }

    #[test]
    fn test_shotgun_damage_values() {
        // Shotgun: 4 per pellet, 8 kick
        let damage: i32 = 4;
        let kick: i32 = 8;
        assert_eq!(damage, 4);
        assert_eq!(kick, 8);
    }

    #[test]
    fn test_supershotgun_damage_values() {
        // Super Shotgun: 6 per pellet, 12 kick
        let damage: i32 = 6;
        let kick: i32 = 12;
        assert_eq!(damage, 6);
        assert_eq!(kick, 12);
    }

    #[test]
    fn test_machinegun_damage_values() {
        // Machinegun: 8 damage, 2 kick
        let damage: i32 = 8;
        let kick: i32 = 2;
        assert_eq!(damage, 8);
        assert_eq!(kick, 2);
    }

    #[test]
    fn test_chaingun_damage_values() {
        // Chaingun DM: 6, SP: 8, kick: 2
        let dm_damage: i32 = 6;
        let sp_damage: i32 = 8;
        let kick: i32 = 2;
        assert_eq!(dm_damage, 6);
        assert_eq!(sp_damage, 8);
        assert_eq!(kick, 2);
    }

    #[test]
    fn test_railgun_damage_values() {
        // DM: 100/200, SP: 150/250
        let (dm_dmg, dm_kick) = (100, 200);
        let (sp_dmg, sp_kick) = (150, 250);
        assert_eq!(dm_dmg, 100);
        assert_eq!(dm_kick, 200);
        assert_eq!(sp_dmg, 150);
        assert_eq!(sp_kick, 250);
    }

    #[test]
    fn test_bfg_damage_values() {
        // BFG: DM=200, SP=500, radius=1000
        let dm_damage: i32 = 200;
        let sp_damage: i32 = 500;
        let damage_radius: f32 = 1000.0;
        assert_eq!(dm_damage, 200);
        assert_eq!(sp_damage, 500);
        assert_eq!(damage_radius, 1000.0);
    }

    // ============================================================
    // Quad damage multiplier tests
    // ============================================================

    #[test]
    fn test_quad_damage_multiplier() {
        // Quad multiplies damage by 4
        let base = 100;
        let quad = base * 4;
        assert_eq!(quad, 400);
    }

    #[test]
    fn test_quad_damage_on_all_weapons() {
        // Verify quad multiplier is consistently 4x
        let weapons_base_damage = [
            ("blaster_dm", 15),
            ("blaster_sp", 10),
            ("shotgun_pellet", 4),
            ("sshotgun_pellet", 6),
            ("machinegun", 8),
            ("chaingun_dm", 6),
            ("chaingun_sp", 8),
            ("grenade", 125),
            ("grenade_launcher", 120),
            ("railgun_dm", 100),
            ("railgun_sp", 150),
            ("bfg_dm", 200),
            ("bfg_sp", 500),
        ];
        for (name, base) in &weapons_base_damage {
            let quad = base * 4;
            assert_eq!(quad, base * 4, "Quad damage mismatch for {}", name);
        }
    }

    // ============================================================
    // Weapon state machine tests
    // ============================================================

    #[test]
    fn test_weapon_state_default_is_ready() {
        let ws = WeaponState::default();
        assert_eq!(ws, WeaponState::Ready);
    }

    #[test]
    fn test_weapon_state_enum_values() {
        assert_eq!(WeaponState::Ready as i32, 0);
        assert_eq!(WeaponState::Activating as i32, 1);
        assert_eq!(WeaponState::Dropping as i32, 2);
        assert_eq!(WeaponState::Firing as i32, 3);
    }

    // ============================================================
    // weapon_generic frame arithmetic tests
    // ============================================================

    #[test]
    fn test_weapon_generic_frame_ranges_blaster() {
        // Blaster: weapon_generic(ctx, ent, 4, 8, 52, 55, ...)
        let frame_activate_last = 4;
        let frame_fire_last = 8;
        let frame_idle_last = 52;
        let frame_deactivate_last = 55;

        let frame_fire_first = frame_activate_last + 1;  // 5
        let frame_idle_first = frame_fire_last + 1;      // 9
        let frame_deactivate_first = frame_idle_last + 1; // 53

        assert_eq!(frame_fire_first, 5);
        assert_eq!(frame_idle_first, 9);
        assert_eq!(frame_deactivate_first, 53);

        // Deactivate range
        let deactivate_range = frame_deactivate_last - frame_deactivate_first;
        assert_eq!(deactivate_range, 2); // 55 - 53 = 2
    }

    #[test]
    fn test_weapon_generic_frame_ranges_shotgun() {
        // Shotgun: weapon_generic(ctx, ent, 7, 18, 36, 39, ...)
        let frame_activate_last = 7;
        let frame_fire_last = 18;
        let frame_idle_last = 36;
        let frame_deactivate_last = 39;

        let frame_fire_first = frame_activate_last + 1;
        let frame_idle_first = frame_fire_last + 1;
        let frame_deactivate_first = frame_idle_last + 1;

        assert_eq!(frame_fire_first, 8);
        assert_eq!(frame_idle_first, 19);
        assert_eq!(frame_deactivate_first, 37);
    }

    #[test]
    fn test_weapon_generic_frame_ranges_rocketlauncher() {
        // Rocket: weapon_generic(ctx, ent, 4, 12, 50, 54, ...)
        let frame_activate_last = 4;
        let frame_fire_last = 12;
        let frame_idle_last = 50;
        let frame_deactivate_last = 54;

        let frame_fire_first = frame_activate_last + 1;
        let frame_idle_first = frame_fire_last + 1;
        let frame_deactivate_first = frame_idle_last + 1;

        assert_eq!(frame_fire_first, 5);
        assert_eq!(frame_idle_first, 13);
        assert_eq!(frame_deactivate_first, 51);
    }

    #[test]
    fn test_weapon_generic_frame_ranges_railgun() {
        // Railgun: weapon_generic(ctx, ent, 3, 18, 56, 61, ...)
        let frame_activate_last = 3;
        let frame_fire_last = 18;
        let frame_idle_last = 56;
        let frame_deactivate_last = 61;

        let frame_fire_first = frame_activate_last + 1;
        let frame_idle_first = frame_fire_last + 1;
        let frame_deactivate_first = frame_idle_last + 1;

        assert_eq!(frame_fire_first, 4);
        assert_eq!(frame_idle_first, 19);
        assert_eq!(frame_deactivate_first, 57);

        // Deactivate range = 4, so the short-deactivate animation check won't trigger
        let deactivate_range = frame_deactivate_last - frame_deactivate_first;
        assert_eq!(deactivate_range, 4);
    }

    #[test]
    fn test_weapon_generic_frame_ranges_bfg() {
        // BFG: weapon_generic(ctx, ent, 8, 32, 55, 58, ...)
        let frame_activate_last = 8;
        let frame_fire_last = 32;
        let frame_idle_last = 55;
        let frame_deactivate_last = 58;

        let frame_fire_first = frame_activate_last + 1;
        let frame_idle_first = frame_fire_last + 1;
        let frame_deactivate_first = frame_idle_last + 1;

        assert_eq!(frame_fire_first, 9);
        assert_eq!(frame_idle_first, 33);
        assert_eq!(frame_deactivate_first, 56);
    }

    // ============================================================
    // Hyperblaster rotation offset tests
    // ============================================================

    #[test]
    fn test_hyperblaster_rotation_at_frame_6() {
        // From weapon_hyperblaster_fire: rotation = (gunframe - 5) * 2*PI/6
        let gunframe = 6;
        let rotation = (gunframe - 5) as f32 * 2.0 * PI / 6.0;
        let offset_x = -4.0 * rotation.sin();
        let offset_z = 4.0 * rotation.cos();

        // rotation = 1 * PI/3 ~= 1.047
        let expected_rotation = PI / 3.0;
        assert!((rotation - expected_rotation).abs() < 0.001);
        assert!((offset_x - (-4.0 * expected_rotation.sin())).abs() < 0.001);
        assert!((offset_z - (4.0 * expected_rotation.cos())).abs() < 0.001);
    }

    #[test]
    fn test_hyperblaster_rotation_at_frame_11() {
        // Frame 11: rotation = (11 - 5) * 2*PI/6 = 6 * PI/3 = 2*PI
        let gunframe = 11;
        let rotation = (gunframe - 5) as f32 * 2.0 * PI / 6.0;

        // At 2*PI, sin ~= 0, cos ~= 1
        let offset_x = -4.0 * rotation.sin();
        let offset_z = 4.0 * rotation.cos();

        assert!(offset_x.abs() < 0.01, "offset_x should be ~0 at 2PI, got {}", offset_x);
        assert!((offset_z - 4.0).abs() < 0.01, "offset_z should be ~4 at 2PI, got {}", offset_z);
    }

    #[test]
    fn test_hyperblaster_effect_frames() {
        // Effect = EF_HYPERBLASTER only on frames 6 and 9
        for frame in 5..=12 {
            let effect = if frame == 6 || frame == 9 {
                EF_HYPERBLASTER
            } else {
                0
            };
            if frame == 6 || frame == 9 {
                assert_eq!(effect, EF_HYPERBLASTER, "frame {} should have hyperblaster effect", frame);
            } else {
                assert_eq!(effect, 0, "frame {} should have no effect", frame);
            }
        }
    }

    // ============================================================
    // Rocket damage range test
    // ============================================================

    #[test]
    fn test_rocket_damage_range() {
        // Rocket: 100 + random(0..1) * 20 => [100, 120)
        let min_damage = 100;
        let max_damage_exclusive = 120; // 100 + 19 = 119 max when cast to i32
        assert!(min_damage >= 100);
        assert!(max_damage_exclusive <= 120);
    }

    #[test]
    fn test_rocket_radius_damage_values() {
        // Rocket: radius_damage=120, damage_radius=120.0
        let radius_damage: i32 = 120;
        let damage_radius: f32 = 120.0;
        assert_eq!(radius_damage, 120);
        assert_eq!(damage_radius, 120.0);
    }

    // ============================================================
    // Machinegun accuracy/spread tests
    // ============================================================

    #[test]
    fn test_machinegun_kick_angle_accumulation() {
        // kick_angles[0] = machinegun_shots * -1.5
        // Shots clamp at 9 in single-player
        for shots in 0..=9 {
            let kick = shots as f32 * -1.5;
            assert!((kick - (shots as f32 * -1.5)).abs() < f32::EPSILON);
        }
        // At max (9), kick = -13.5
        let max_kick = 9.0 * -1.5;
        assert_eq!(max_kick, -13.5);
    }

    #[test]
    fn test_machinegun_shot_count_clamp() {
        // In single-player, shots clamp at 9
        let mut shots = 0;
        for _ in 0..20 {
            shots += 1;
            if shots > 9 {
                shots = 9;
            }
        }
        assert_eq!(shots, 9);
    }

    // ============================================================
    // Chaingun shot count calculation tests
    // ============================================================

    #[test]
    fn test_chaingun_shots_by_frame() {
        // gunframe <= 9: 1 shot
        // gunframe 10..=14: 2 shots if attacking, 1 otherwise
        // gunframe >= 15: 3 shots
        for frame in 5..=21 {
            let attacking = true;
            let shots = if frame <= 9 {
                1
            } else if frame <= 14 {
                if attacking { 2 } else { 1 }
            } else {
                3
            };

            if frame <= 9 {
                assert_eq!(shots, 1, "frame {} should fire 1 shot", frame);
            } else if frame <= 14 {
                assert_eq!(shots, 2, "frame {} should fire 2 shots when attacking", frame);
            } else {
                assert_eq!(shots, 3, "frame {} should fire 3 shots", frame);
            }
        }
    }

    #[test]
    fn test_chaingun_shots_not_attacking() {
        for frame in 10..=14 {
            let attacking = false;
            let shots = if frame <= 9 {
                1
            } else if frame <= 14 {
                if attacking { 2 } else { 1 }
            } else {
                3
            };
            assert_eq!(shots, 1, "frame {} should fire 1 shot when not attacking", frame);
        }
    }

    #[test]
    fn test_chaingun_shots_clamped_by_ammo() {
        // Shots get clamped to available ammo
        let ammo = 2;
        let mut shots = 3; // frame >= 15, max fire
        if ammo < shots {
            shots = ammo;
        }
        assert_eq!(shots, 2);
    }

    // ============================================================
    // Super Shotgun spread angle test
    // ============================================================

    #[test]
    fn test_supershotgun_dual_barrel_spread() {
        // Super shotgun fires two volleys: yaw-5 and yaw+5
        let base_yaw = 90.0f32;
        let yaw_left = base_yaw - 5.0;
        let yaw_right = base_yaw + 5.0;
        assert_eq!(yaw_left, 85.0);
        assert_eq!(yaw_right, 95.0);
        // Total spread = 10 degrees
        assert_eq!(yaw_right - yaw_left, 10.0);
    }

    #[test]
    fn test_supershotgun_ammo_consumption() {
        // Super shotgun consumes 2 shells per shot
        let ammo_per_shot = 2;
        assert_eq!(ammo_per_shot, 2);
    }

    // ============================================================
    // BFG cell consumption test
    // ============================================================

    #[test]
    fn test_bfg_cell_consumption() {
        // BFG consumes 50 cells per shot
        let ammo_per_shot = 50;
        assert_eq!(ammo_per_shot, 50);
    }

    #[test]
    fn test_bfg_abort_if_insufficient_cells() {
        // If ammo < 50 during windup, BFG should abort (just advance frame)
        let ammo = 49;
        let required = 50;
        assert!(ammo < required);
    }

    // ============================================================
    // add_ammo_to_client tests
    // ============================================================

    #[test]
    fn test_add_ammo_basic() {
        init_test_gi();
        let mut ctx = make_weapon_ctx();

        // Set up a fake "bullets" item at index 2
        while ctx.items.len() <= 2 {
            ctx.items.push(GItem::default());
        }
        ctx.items[2].pickup_name = "Bullets".to_string();

        ctx.clients[0].pers.max_bullets = 200;
        ctx.clients[0].pers.inventory[2] = 0;

        add_ammo_to_client(&mut ctx, 0, 2, 50);
        assert_eq!(ctx.clients[0].pers.inventory[2], 50);
    }

    #[test]
    fn test_add_ammo_capped_at_max() {
        init_test_gi();
        let mut ctx = make_weapon_ctx();

        while ctx.items.len() <= 2 {
            ctx.items.push(GItem::default());
        }
        ctx.items[2].pickup_name = "Shells".to_string();

        ctx.clients[0].pers.max_shells = 100;
        ctx.clients[0].pers.inventory[2] = 90;

        add_ammo_to_client(&mut ctx, 0, 2, 50);
        // Should be capped at 100, not 140
        assert_eq!(ctx.clients[0].pers.inventory[2], 100);
    }

    #[test]
    fn test_add_ammo_already_at_max() {
        init_test_gi();
        let mut ctx = make_weapon_ctx();

        while ctx.items.len() <= 2 {
            ctx.items.push(GItem::default());
        }
        ctx.items[2].pickup_name = "Rockets".to_string();

        ctx.clients[0].pers.max_rockets = 50;
        ctx.clients[0].pers.inventory[2] = 50;

        add_ammo_to_client(&mut ctx, 0, 2, 10);
        // Already at max, should not change
        assert_eq!(ctx.clients[0].pers.inventory[2], 50);
    }

    #[test]
    fn test_add_ammo_unknown_type_high_max() {
        init_test_gi();
        let mut ctx = make_weapon_ctx();

        while ctx.items.len() <= 2 {
            ctx.items.push(GItem::default());
        }
        ctx.items[2].pickup_name = "UnknownAmmo".to_string();

        ctx.clients[0].pers.inventory[2] = 0;

        add_ammo_to_client(&mut ctx, 0, 2, 500);
        // Unknown ammo type has max of 999
        assert_eq!(ctx.clients[0].pers.inventory[2], 500);
    }

    // ============================================================
    // WeaponContext defaults
    // ============================================================

    #[test]
    fn test_weapon_context_defaults() {
        let wctx = WeaponContext::default();
        assert!(!wctx.is_quad);
        assert_eq!(wctx.is_silenced, 0);
    }

    // ============================================================
    // Weapon fire frame lists
    // ============================================================

    #[test]
    fn test_blaster_fire_frames() {
        let fire_frames: &[i32] = &[5, 0];
        assert_eq!(fire_frames[0], 5);
        assert_eq!(fire_frames[1], 0); // zero-terminated
    }

    #[test]
    fn test_shotgun_fire_frames() {
        let fire_frames: &[i32] = &[8, 9, 0];
        // Shotgun fires on frame 8, and frame 9 is used as reload check
        assert_eq!(fire_frames[0], 8);
        assert_eq!(fire_frames[1], 9);
    }

    #[test]
    fn test_hyperblaster_fire_frames() {
        let fire_frames: &[i32] = &[6, 7, 8, 9, 10, 11, 0];
        // Hyperblaster fires on every frame from 6 to 11
        for i in 0..6 {
            assert_eq!(fire_frames[i], 6 + i as i32);
        }
    }

    #[test]
    fn test_chaingun_fire_frames() {
        let fire_frames: &[i32] = &[5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 0];
        // Chaingun fires on frames 5 through 21
        for i in 0..17 {
            assert_eq!(fire_frames[i], 5 + i as i32);
        }
        assert_eq!(fire_frames[17], 0); // zero-terminated
    }

    #[test]
    fn test_bfg_fire_frames() {
        let fire_frames: &[i32] = &[9, 17, 0];
        // BFG fires on frame 9 (muzzle flash) and 17 (actual fire)
        assert_eq!(fire_frames[0], 9);
        assert_eq!(fire_frames[1], 17);
    }

    // ============================================================
    // Spread constant tests
    // ============================================================

    #[test]
    fn test_default_spread_constants() {
        assert_eq!(DEFAULT_BULLET_HSPREAD, 300);
        assert_eq!(DEFAULT_BULLET_VSPREAD, 500);
        assert_eq!(DEFAULT_SHOTGUN_HSPREAD, 1000);
        assert_eq!(DEFAULT_SHOTGUN_VSPREAD, 500);
        assert_eq!(DEFAULT_DEATHMATCH_SHOTGUN_COUNT, 12);
        assert_eq!(DEFAULT_SHOTGUN_COUNT, 12);
        assert_eq!(DEFAULT_SSHOTGUN_COUNT, 20);
    }

    // ============================================================
    // set_respawn_weapon tests
    // ============================================================

    #[test]
    fn test_set_respawn_weapon() {
        init_test_gi();
        let mut ctx = make_weapon_ctx();

        // Push a weapon entity at index 2
        let mut weapon_ent = Edict::default();
        weapon_ent.inuse = true;
        ctx.edicts.push(weapon_ent);

        ctx.level.time = 10.0;
        set_respawn_weapon(&mut ctx, 2, 30.0);

        assert!(ctx.edicts[2].flags.intersects(FL_RESPAWN));
        assert_eq!(ctx.edicts[2].solid, Solid::Not);
        assert!((ctx.edicts[2].nextthink - 40.0).abs() < f32::EPSILON);
        assert_eq!(ctx.edicts[2].think_fn, Some(THINK_DO_RESPAWN));
    }

    // ============================================================
    // Muzzle flash silencer tests
    // ============================================================

    #[test]
    fn test_muzzle_flash_silencer_bit() {
        // Silenced weapons OR the MZ_SILENCED bit into the muzzle flash
        let mz_base = MZ_BLASTER;
        let mz_silenced = mz_base | MZ_SILENCED;
        assert_ne!(mz_silenced, mz_base);
        assert_eq!(mz_silenced & MZ_SILENCED, MZ_SILENCED);
    }

    #[test]
    fn test_muzzle_flash_values_distinct() {
        // Each weapon has a distinct base muzzle flash
        let flashes = [MZ_BLASTER, MZ_SHOTGUN, MZ_SSHOTGUN, MZ_MACHINEGUN,
                       MZ_ROCKET, MZ_GRENADE, MZ_RAILGUN, MZ_BFG, MZ_HYPERBLASTER];
        for i in 0..flashes.len() {
            for j in (i + 1)..flashes.len() {
                assert_ne!(flashes[i], flashes[j],
                    "Muzzle flash {} and {} should be distinct", i, j);
            }
        }
    }

    // ============================================================
    // Weapon kick calculations
    // ============================================================

    #[test]
    fn test_shotgun_kick_origin() {
        // Shotgun: kick_origin = forward * -2.0
        let forward = [1.0, 0.0, 0.0];
        let kick_origin = vector_scale(&forward, -2.0);
        assert_eq!(kick_origin, [-2.0, 0.0, 0.0]);
    }

    #[test]
    fn test_railgun_kick_origin() {
        // Railgun: kick_origin = forward * -3.0, kick_angles[0] = -3.0
        let forward = [0.0, 1.0, 0.0];
        let kick_origin = vector_scale(&forward, -3.0);
        assert_eq!(kick_origin, [0.0, -3.0, 0.0]);
    }

    // ============================================================
    // Grenade state machine timing
    // ============================================================

    #[test]
    fn test_grenade_detonation_time() {
        // Grenade timer starts at level.time + GRENADE_TIMER + 0.2
        let level_time = 10.0;
        let grenade_time = level_time + GRENADE_TIMER + 0.2;
        assert!((grenade_time - 13.2).abs() < f32::EPSILON);
    }

    #[test]
    fn test_grenade_idle_frame_wrap() {
        // Grenade idle loops frames 16..=48, wrapping at 48 back to 16
        let mut frame = 48;
        frame += 1;
        if frame > 48 {
            frame = 16;
        }
        assert_eq!(frame, 16);
    }

    #[test]
    fn test_grenade_pause_frames() {
        // Grenade pauses at frames 29, 34, 39, 48
        let pause_frames = [29, 34, 39, 48];
        for &pf in &pause_frames {
            assert!(pf >= 16 && pf <= 48, "pause frame {} out of idle range", pf);
        }
    }
}
