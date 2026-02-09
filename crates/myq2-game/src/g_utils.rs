// g_utils.rs — Game utility functions
// Converted from: myq2-original/game/g_utils.c

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

use crate::g_local::{Edict, GameContext, BODY_QUEUE_SIZE, MOD_TELEFRAG, DAMAGE_NO_PROTECTION};
use crate::game::Solid;
use crate::game_import::*;
use myq2_common::q_shared::{
    Vec3, angle_vectors, vector_compare, q_stricmp,
    MASK_PLAYERSOLID, CHAN_AUTO, ATTN_NORM,
    MAX_EDICTS, AREA_TRIGGERS, AREA_SOLID,
};
const MAXCHOICES: usize = 8;

/// Projects a point in 3D space using forward and right vectors.
/// Used for weapon muzzle positioning.
pub fn g_project_source(
    point: &Vec3,
    distance: &Vec3,
    forward: &Vec3,
    right: &Vec3,
) -> Vec3 {
    [
        point[0] + forward[0] * distance[0] + right[0] * distance[1],
        point[1] + forward[1] * distance[0] + right[1] * distance[1],
        point[2] + forward[2] * distance[0] + right[2] * distance[1] + distance[2],
    ]
}


/// Searches all active entities for the next one matching `field`/`match_val`.
/// Starts searching at index `from` (pass 0 to search from the beginning).
/// Returns the entity index if found.
///
/// Uses O(1) HashMap lookup via entity indices built by `build_entity_indices()`.
pub fn g_find(ctx: &GameContext, from: usize, field: &str, match_val: &str) -> Option<usize> {
    let indexed_matches = match field {
        "targetname" => ctx.find_entities_by_targetname(match_val),
        "classname" => ctx.find_entities_by_classname(match_val),
        _ => return None,
    };

    for &idx in indexed_matches {
        let idx_usize = idx as usize;
        if idx_usize >= from && ctx.edicts[idx_usize].inuse {
            return Some(idx_usize);
        }
    }

    None
}


/// Pick a random target from all entities with the matching targetname.
/// Returns None if no matching entities are found.
pub fn g_pick_target(game: &GameContext, targetname: &str) -> Option<usize> {
    if targetname.is_empty() {
        gi_dprintf("G_PickTarget called with NULL targetname\n");
        return None;
    }

    let mut choices: [Option<usize>; MAXCHOICES] = [None; MAXCHOICES];
    let mut num_choices = 0;
    let mut search_from = 0;

    loop {
        match g_find(game, search_from, "targetname", targetname) {
            None => break,
            Some(idx) => {
                choices[num_choices] = Some(idx);
                num_choices += 1;
                if num_choices == MAXCHOICES {
                    break;
                }
                search_from = idx + 1;
            }
        }
    }

    if num_choices == 0 {
        gi_dprintf(&format!("G_PickTarget: target {} not found\n", targetname));
        return None;
    }

    // Use a simple random selection (in real code this would use the game's RNG)
    // For now, just return the first one
    choices[0]
}

/// Think function for delayed entity use.
/// Called after a delay to trigger the actual target use.
pub fn think_delay(game: &mut GameContext, ent_idx: i32) {
    let idx = ent_idx as usize;
    let activator = game.edicts[idx].activator;
    g_use_targets(game, idx, activator as usize);
    g_free_edict(game, idx);
}

// Thread-local storage for temporary vectors.
// In C this used static arrays; in Rust we use thread-local for safety.
thread_local! {
    static TEMP_VECTORS: std::cell::RefCell<(usize, [[f32; 3]; 8])> =
        const { std::cell::RefCell::new((0, [[0.0; 3]; 8])) };
}

/// Temporary vector helper for function calls.
/// Returns a temporary vector that will be valid until the next 7 calls.
pub fn tv(x: f32, y: f32, z: f32) -> Vec3 {
    TEMP_VECTORS.with(|tv| {
        let mut tv = tv.borrow_mut();
        let idx = tv.0;
        tv.0 = (idx + 1) & 7;
        tv.1[idx][0] = x;
        tv.1[idx][1] = y;
        tv.1[idx][2] = z;
        tv.1[idx]
    })
}

// Thread-local storage for vector-to-string conversions.
thread_local! {
    static TEMP_STRINGS: std::cell::RefCell<(usize, [String; 8])> =
        std::cell::RefCell::new((0, Default::default()));
}

/// Convert a vector to a string for printing.
pub fn vtos(v: &Vec3) -> String {
    TEMP_STRINGS.with(|ts| {
        let mut ts = ts.borrow_mut();
        let idx = ts.0;
        ts.0 = (idx + 1) & 7;
        ts.1[idx] = format!("({} {} {})", v[0] as i32, v[1] as i32, v[2] as i32);
        ts.1[idx].clone()
    })
}

// Special angle vectors for movement direction
const VEC_UP: Vec3 = [0.0, -1.0, 0.0];
const MOVEDIR_UP: Vec3 = [0.0, 0.0, 1.0];
const VEC_DOWN: Vec3 = [0.0, -2.0, 0.0];
const MOVEDIR_DOWN: Vec3 = [0.0, 0.0, -1.0];

/// Set movement direction from angles.
pub fn g_set_movedir(angles: &Vec3, movedir: &mut Vec3) {
    if vector_compare(angles, &VEC_UP) {
        *movedir = MOVEDIR_UP;
    } else if vector_compare(angles, &VEC_DOWN) {
        *movedir = MOVEDIR_DOWN;
    } else {
        let mut forward = [0.0_f32; 3];
        angle_vectors(angles, Some(&mut forward), None, None);
        *movedir = forward;
    }
}

pub use myq2_common::q_shared::vectoyaw;
pub use myq2_common::q_shared::vectoangles;

/// Copy a string (allocate memory and duplicate).
/// In C this used gi.TagMalloc; in Rust we just clone the String.
pub fn g_copy_string(s: &str) -> String {
    s.to_string()
}

/// Initialize a newly allocated edict.
pub fn g_init_edict(_game: &GameContext, e: &mut Edict, index: i32) {
    init_edict_raw(e, index);
}

/// Raw edict initialization — no context needed.
pub fn init_edict_raw(e: &mut Edict, index: i32) {
    e.inuse = true;
    e.classname = "noclass".to_string();
    e.gravity = 1.0;
    e.s.number = index;
}

/// Core spawn logic — operates on raw edict data, decoupled from any context type.
/// Searches for a free edict or allocates a new one. Returns the edict index.
pub fn spawn_edict_raw(
    edicts: &mut Vec<Edict>,
    maxclients: usize,
    num_edicts: &mut usize,
    max_edicts: usize,
    level_time: f32,
) -> usize {
    // Search for a free entity — avoid reusing recently freed ones
    for i in (maxclients + 1)..*num_edicts {
        if !edicts[i].inuse && (edicts[i].freetime < 2.0 || level_time - edicts[i].freetime > 0.5) {
            edicts[i] = Edict::default();
            init_edict_raw(&mut edicts[i], i as i32);
            return i;
        }
    }

    if *num_edicts >= max_edicts {
        gi_error("ED_Alloc: no free edicts");
    }

    let i = *num_edicts;
    *num_edicts += 1;

    // Ensure edicts vec is large enough
    while edicts.len() <= i {
        edicts.push(Edict::default());
    }

    edicts[i] = Edict::default();
    init_edict_raw(&mut edicts[i], i as i32);
    i
}

/// Core free edict logic — operates on raw edict data, decoupled from any context type.
pub fn free_edict_raw(
    edicts: &mut [Edict],
    ent_idx: usize,
    maxclients: usize,
    level_time: f32,
) {
    gi_unlinkentity(ent_idx as i32);

    if ent_idx <= maxclients + BODY_QUEUE_SIZE {
        return;
    }

    if ent_idx < edicts.len() {
        edicts[ent_idx] = Edict::default();
        edicts[ent_idx].classname = "freed".to_string();
        edicts[ent_idx].freetime = level_time;
        edicts[ent_idx].inuse = false;
    }
}

/// Either finds a free edict, or allocates a new one. Returns the entity index.
///
/// Try to avoid reusing an entity that was recently freed, because it
/// can cause the client to think the entity morphed into something else
/// instead of being removed and recreated, which can cause interpolated
/// angles and bad trails.
pub fn g_spawn(ctx: &mut GameContext) -> usize {
    let mut num = ctx.num_edicts as usize;
    let result = spawn_edict_raw(
        &mut ctx.edicts,
        ctx.maxclients as usize,
        &mut num,
        ctx.max_edicts as usize,
        ctx.level.time,
    );
    ctx.num_edicts = num as i32;
    result
}

/// Marks the edict as free and clears it.
pub fn g_free_edict(ctx: &mut GameContext, ent_idx: usize) {
    free_edict_raw(
        &mut ctx.edicts,
        ent_idx,
        ctx.maxclients as usize,
        ctx.level.time,
    );
}

/// Fires all targets of an entity. Handles delays, killtargets, and messages.
///
/// The global "activator" should be set to the entity that initiated the firing.
/// If self.delay is set, a DelayedUse entity will be created that will actually
/// do the SUB_UseTargets after that many seconds have passed.
///
/// Centerprints any self.message to the activator.
/// Search for (string)targetname in all entities that match (string)self.target
/// and call their .use function.
pub fn g_use_targets(ctx: &mut GameContext, ent_idx: usize, activator_idx: usize) {
    // Gather entity data to avoid borrow issues
    let (delay, message, target, killtarget, noise_index) = {
        let ent = &ctx.edicts[ent_idx];
        (
            ent.delay,
            ent.message.clone(),
            ent.target.clone(),
            ent.killtarget.clone(),
            ent.noise_index,
        )
    };

    // Check for a delay
    if delay > 0.0 {
        let t_idx = g_spawn(ctx);
        let level_time = ctx.level.time;
        let t = &mut ctx.edicts[t_idx];
        t.classname = "DelayedUse".to_string();
        t.nextthink = level_time + delay;
        t.activator = activator_idx as i32;

        if (activator_idx as i32) < 0 {
            gi_dprintf("Think_Delay with no activator\n");
        }

        t.message = message;
        t.target = target;
        t.killtarget = killtarget;
        return;
    }

    // Print the message
    if !message.is_empty() {
        let activator = &ctx.edicts[activator_idx];
        if (activator.svflags & crate::game::SVF_MONSTER) == 0 {
            gi_centerprintf(activator_idx as i32, &message);

            if noise_index != 0 {
                gi_sound(activator_idx as i32, CHAN_AUTO, noise_index, 1.0, ATTN_NORM, 0.0);
            } else {
                gi_sound(activator_idx as i32, CHAN_AUTO, gi_soundindex("misc/talk1.wav"), 1.0, ATTN_NORM, 0.0);
            }
        }
    }

    // Kill killtargets
    if !killtarget.is_empty() {
        let mut search_from = 0;
        loop {
            match g_find(ctx, search_from, "targetname", &killtarget) {
                None => break,
                Some(t_idx) => {
                    g_free_edict(ctx, t_idx);
                    if !ctx.edicts[ent_idx].inuse {
                        gi_dprintf("entity was removed while using killtargets\n");
                        return;
                    }
                    search_from = t_idx + 1;
                }
            }
        }
    }

    // Fire targets
    if !target.is_empty() {
        let mut search_from = 0;
        loop {
            match g_find(ctx, search_from, "targetname", &target) {
                None => break,
                Some(t_idx) => {
                    let t_classname = ctx.edicts[t_idx].classname.clone();
                    let t_use_fn = ctx.edicts[t_idx].use_fn;
                    let ent_classname = ctx.edicts[ent_idx].classname.clone();

                    // Doors fire area portals in a specific way
                    if q_stricmp(&t_classname, "func_areaportal") == std::cmp::Ordering::Equal
                        && (q_stricmp(&ent_classname, "func_door") == std::cmp::Ordering::Equal
                            || q_stricmp(&ent_classname, "func_door_rotating") == std::cmp::Ordering::Equal)
                    {
                        search_from = t_idx + 1;
                        continue;
                    }

                    if t_idx == ent_idx {
                        gi_dprintf("WARNING: Entity used itself.\n");
                    } else if t_use_fn.is_some() {
                        crate::dispatch::call_use(
                            t_idx, ent_idx, activator_idx,
                            &mut ctx.edicts, &mut ctx.level,
                        );
                    }

                    if !ctx.edicts[ent_idx].inuse {
                        gi_dprintf("entity was removed while using targets\n");
                        return;
                    }
                    search_from = t_idx + 1;
                }
            }
        }
    }
}

/// Touches all triggers that the entity is in contact with.
pub fn g_touch_triggers(ctx: &mut GameContext, ent_idx: usize) {
    let ent = &ctx.edicts[ent_idx];

    // Dead things don't activate triggers
    let is_dead = (ent.client.is_some() || (ent.svflags & crate::game::SVF_MONSTER) != 0)
        && ent.health <= 0;
    if is_dead {
        return;
    }

    let absmin = ent.absmin;
    let absmax = ent.absmax;

    let touch = gi_box_edicts(&absmin, &absmax, MAX_EDICTS as i32, AREA_TRIGGERS);

    for &hit_idx in &touch {
        if let Some(hit) = ctx.edicts.get(hit_idx as usize) {
            if !hit.inuse {
                continue;
            }
            if hit.touch_fn.is_none() {
                continue;
            }
            crate::dispatch::call_touch(
                hit_idx as usize, ent_idx,
                &mut ctx.edicts, &mut ctx.level,
                None, None,
            );
        }
    }
}

/// Force all entities that the trigger covers to immediately touch it.
/// Called after linking a new trigger in during gameplay.
pub fn g_touch_solids(ctx: &mut GameContext, ent_idx: usize) {
    let (absmin, absmax, touch_fn) = {
        let ent = &ctx.edicts[ent_idx];
        (ent.absmin, ent.absmax, ent.touch_fn)
    };

    let touch = gi_box_edicts(&absmin, &absmax, MAX_EDICTS as i32, AREA_SOLID);

    for &hit_idx in &touch {
        if let Some(hit) = ctx.edicts.get(hit_idx as usize) {
            if !hit.inuse {
                continue;
            }
            if touch_fn.is_some() {
                crate::dispatch::call_touch(
                    ent_idx, hit_idx as usize,
                    &mut ctx.edicts, &mut ctx.level,
                    None, None,
                );
            }
            if let Some(ent) = ctx.edicts.get(ent_idx) {
                if !ent.inuse {
                    break;
                }
            } else {
                break;
            }
        }
    }
}

/// Kills all entities that would touch the proposed new positioning of ent.
/// Ent should be unlinked before calling this!
/// Returns true if all blocking entities were killed.
pub fn killbox(ctx: &mut GameContext, ent_idx: usize) -> bool {
    let origin = ctx.edicts[ent_idx].s.origin;
    let mins = ctx.edicts[ent_idx].mins;
    let maxs = ctx.edicts[ent_idx].maxs;

    loop {
        let tr = gi_trace(&origin, &mins, &maxs, &origin, -1, MASK_PLAYERSOLID);

        if tr.ent_index < 0 {
            break;
        }

        let tr_ent_idx = tr.ent_index as usize;

        let zero_vec = [0.0f32; 3];
        crate::g_combat::ctx_t_damage(
            ctx, tr_ent_idx, ent_idx, ent_idx,
            &zero_vec, &origin, &zero_vec,
            100000, 0, DAMAGE_NO_PROTECTION, MOD_TELEFRAG,
        );

        if ctx.edicts[tr_ent_idx].solid != Solid::Not {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_g_project_source() {
        let point = [0.0, 0.0, 0.0];
        let distance = [10.0, 5.0, 2.0];
        let forward = [1.0, 0.0, 0.0];
        let right = [0.0, 1.0, 0.0];

        let result = g_project_source(&point, &distance, &forward, &right);

        assert_eq!(result[0], 10.0); // forward * distance[0]
        assert_eq!(result[1], 5.0);  // right * distance[1]
        assert_eq!(result[2], 2.0);  // distance[2]
    }

    #[test]
    fn test_tv() {
        let v1 = tv(1.0, 2.0, 3.0);
        assert_eq!(v1, [1.0, 2.0, 3.0]);

        let v2 = tv(4.0, 5.0, 6.0);
        assert_eq!(v2, [4.0, 5.0, 6.0]);
    }

    #[test]
    fn test_vtos() {
        let v = [10.5, 20.7, 30.2];
        let s = vtos(&v);
        assert_eq!(s, "(10 20 30)");
    }

    #[test]
    fn test_vectoyaw() {
        // East direction
        let vec = [1.0, 0.0, 0.0];
        let yaw = vectoyaw(&vec);
        assert!((yaw - 0.0).abs() < 0.01);

        // North direction
        let vec = [0.0, 1.0, 0.0];
        let yaw = vectoyaw(&vec);
        assert!((yaw - 90.0).abs() < 0.01);
    }

    #[test]
    fn test_vectoangles() {
        let mut angles = [0.0; 3];

        // Straight up
        let v = [0.0, 0.0, 1.0];
        vectoangles(&v, &mut angles);
        assert_eq!(angles[0], -90.0); // pitch
        assert_eq!(angles[1], 0.0);   // yaw
        assert_eq!(angles[2], 0.0);   // roll

        // East direction
        let v = [1.0, 0.0, 0.0];
        vectoangles(&v, &mut angles);
        assert_eq!(angles[0], 0.0);   // pitch
        assert_eq!(angles[1], 0.0);   // yaw
        assert_eq!(angles[2], 0.0);   // roll
    }

    #[test]
    fn test_g_copy_string() {
        let original = "test string";
        let copied = g_copy_string(original);
        assert_eq!(copied, original);
    }
}
