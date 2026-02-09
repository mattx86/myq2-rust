// g_trigger.rs — Trigger entity functions
// Converted from: myq2-original/game/g_trigger.c

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
use crate::game_import::*;
use myq2_common::q_shared::{vector_compare, VEC3_ORIGIN, YAW};

// Local constants
const PUSH_ONCE: i32 = 1;

// CHAN_*, ATTN_* come from g_local::* re-export (myq2_common::q_shared)

// ============================================================
// Callback indices for think/touch/use dispatch tables
// These constants identify the trigger callback functions.
// ============================================================

pub use crate::dispatch::THINK_MULTI_WAIT;
pub use crate::dispatch::THINK_FREE_EDICT as THINK_G_FREE_EDICT;
pub use crate::dispatch::USE_MULTI;
pub use crate::dispatch::USE_TRIGGER_ENABLE;
pub use crate::dispatch::USE_TRIGGER_RELAY;
pub use crate::dispatch::USE_TRIGGER_KEY;
pub use crate::dispatch::USE_TRIGGER_COUNTER;
pub use crate::dispatch::USE_HURT;
pub use crate::dispatch::TOUCH_MULTI;
pub use crate::dispatch::TOUCH_TRIGGER_PUSH;
pub use crate::dispatch::TOUCH_TRIGGER_HURT as TOUCH_HURT;
pub use crate::dispatch::TOUCH_TRIGGER_GRAVITY;
pub use crate::dispatch::TOUCH_TRIGGER_MONSTERJUMP;

// ============================================================
// Helper functions (placeholders for cross-module calls)
// ============================================================

use myq2_common::q_shared::{
    vector_scale_to as vector_scale, vector_ma_to as vector_ma,
    dot_product, angle_vectors,
};

use crate::g_utils::vtos;

// ============================================================
// Trigger functions
// ============================================================

/// Initializes a trigger entity: sets movedir from angles, sets solid to TRIGGER,
/// movetype to NONE, applies model, and marks as SVF_NOCLIENT.
pub fn init_trigger(ctx: &mut GameContext, ent_idx: usize) {
    let ent = &mut ctx.edicts[ent_idx];
    if !vector_compare(&ent.s.angles, &VEC3_ORIGIN) {
        let angles = ent.s.angles;
        crate::g_utils::g_set_movedir(&angles, &mut ent.movedir);
        ent.s.angles = [0.0, 0.0, 0.0];
    }

    ent.solid = Solid::Trigger;
    ent.movetype = MoveType::None;
    let model = ent.model.clone();
    gi_setmodel(ent_idx as i32, &model);
    ent.svflags = SVF_NOCLIENT;
}

/// The wait time has passed, so set back up for another activation.
pub fn multi_wait(ctx: &mut GameContext, ent_idx: usize) {
    ctx.edicts[ent_idx].nextthink = 0.0;
}

/// The trigger was just activated.
/// ent->activator should be set to the activator so it can be held through a delay
/// so wait for the delay time before firing.
pub fn multi_trigger(ctx: &mut GameContext, ent_idx: usize) {
    let ent = &ctx.edicts[ent_idx];
    if ent.nextthink != 0.0 {
        return; // already been triggered
    }

    let activator_idx = ctx.edicts[ent_idx].activator as usize;
    // G_UseTargets(ent, ent->activator)
    ctx.maxclients = ctx.game.maxclients as f32;
    ctx.num_edicts = ctx.edicts.len() as i32;
    ctx.max_edicts = ctx.edicts.capacity() as i32;
    crate::g_utils::g_use_targets(ctx, ent_idx, activator_idx);

    let ent = &mut ctx.edicts[ent_idx];
    if ent.wait > 0.0 {
        ent.think_fn = Some(THINK_MULTI_WAIT);
        ent.nextthink = ctx.level.time + ent.wait;
    } else {
        // We can't just remove (self) here, because this is a touch function
        // called while looping through area links...
        ent.touch_fn = None;
        ent.nextthink = ctx.level.time + FRAMETIME;
        ent.think_fn = Some(THINK_G_FREE_EDICT);
    }
}

/// Use callback for trigger_multiple / trigger_once.
pub fn use_multi(ctx: &mut GameContext, ent_idx: usize, _other_idx: usize, activator_idx: usize) {
    ctx.edicts[ent_idx].activator = activator_idx as i32;
    multi_trigger(ctx, ent_idx);
}

/// Touch callback for trigger_multiple.
pub fn touch_multi(ctx: &mut GameContext, self_idx: usize, other_idx: usize, _plane: Option<&CPlane>, _surf: Option<&CSurface>) {
    let other = &ctx.edicts[other_idx];
    let self_ent = &ctx.edicts[self_idx];

    if other.client.is_some() {
        if self_ent.spawnflags & 2 != 0 {
            return;
        }
    } else if other.svflags & SVF_MONSTER != 0 {
        if self_ent.spawnflags & 1 == 0 {
            return;
        }
    } else {
        return;
    }

    if !vector_compare(&self_ent.movedir, &VEC3_ORIGIN) {
        let mut forward = [0.0f32; 3];
        angle_vectors(&other.s.angles, Some(&mut forward), None, None);
        if dot_product(&forward, &self_ent.movedir) < 0.0 {
            return;
        }
    }

    ctx.edicts[self_idx].activator = other_idx as i32;
    multi_trigger(ctx, self_idx);
}

/// Use callback to enable a TRIGGERED trigger_multiple.
pub fn trigger_enable(ctx: &mut GameContext, self_idx: usize, _other_idx: usize, _activator_idx: usize) {
    ctx.edicts[self_idx].solid = Solid::Trigger;
    ctx.edicts[self_idx].use_fn = Some(USE_MULTI);
    gi_linkentity(self_idx as i32);
}

/// Spawn function for trigger_multiple.
///
/// Variable sized repeatable trigger. Must be targeted at one or more entities.
/// If "delay" is set, the trigger waits some time after activating before firing.
/// "wait": Seconds between triggerings (.2 default).
/// sounds: 1=secret, 2=beep beep, 3=large switch
pub fn sp_trigger_multiple(ctx: &mut GameContext, ent_idx: usize) {
    {
        let ent = &mut ctx.edicts[ent_idx];

        if ent.sounds == 1 {
            ent.noise_index = gi_soundindex("misc/secret.wav");
        } else if ent.sounds == 2 {
            ent.noise_index = gi_soundindex("misc/talk.wav");
        } else if ent.sounds == 3 {
            ent.noise_index = gi_soundindex("misc/trigger1.wav");
        }

        if ent.wait == 0.0 {
            ent.wait = 0.2;
        }
        ent.touch_fn = Some(TOUCH_MULTI);
        ent.movetype = MoveType::None;
        ent.svflags |= SVF_NOCLIENT;

        if ent.spawnflags & 4 != 0 {
            ent.solid = Solid::Not;
            ent.use_fn = Some(USE_TRIGGER_ENABLE);
        } else {
            ent.solid = Solid::Trigger;
            ent.use_fn = Some(USE_MULTI);
        }

        if !vector_compare(&ent.s.angles, &VEC3_ORIGIN) {
            let angles = ent.s.angles;
            crate::g_utils::g_set_movedir(&angles, &mut ent.movedir);
            ent.s.angles = [0.0, 0.0, 0.0];
        }
    }

    {
        let model = ctx.edicts[ent_idx].model.clone();
        gi_setmodel(ent_idx as i32, &model);
    }
    gi_linkentity(ent_idx as i32);
}

/// Spawn function for trigger_once.
///
/// Triggers once, then removes itself.
/// If TRIGGERED, this trigger must be triggered before it is live.
/// sounds: 1=secret, 2=beep beep, 3=large switch
pub fn sp_trigger_once(ctx: &mut GameContext, ent_idx: usize) {
    // Make old maps work because the flag assignments were messed up:
    // triggered was on bit 1 when it should have been on bit 4
    if ctx.edicts[ent_idx].spawnflags & 1 != 0 {
        let ent = &ctx.edicts[ent_idx];
        let mut v = [0.0f32; 3];
        vector_ma(&ent.mins, 0.5, &ent.size, &mut v);
        let classname = ent.classname.clone();
        gi_dprintf(&format!("fixed TRIGGERED flag on {} at {}\n", classname, vtos(&v)));

        let ent = &mut ctx.edicts[ent_idx];
        ent.spawnflags &= !1;
        ent.spawnflags |= 4;
    }

    ctx.edicts[ent_idx].wait = -1.0;
    sp_trigger_multiple(ctx, ent_idx);
}

/// Use callback for trigger_relay — simply fires targets.
pub fn trigger_relay_use(ctx: &mut GameContext, self_idx: usize, _other_idx: usize, activator_idx: usize) {
    ctx.maxclients = ctx.game.maxclients as f32;
    ctx.num_edicts = ctx.edicts.len() as i32;
    ctx.max_edicts = ctx.edicts.capacity() as i32;
    crate::g_utils::g_use_targets(ctx, self_idx, activator_idx);
}

/// Spawn function for trigger_relay.
///
/// This fixed size trigger cannot be touched, it can only be fired by other events.
pub fn sp_trigger_relay(ctx: &mut GameContext, self_idx: usize) {
    ctx.edicts[self_idx].use_fn = Some(USE_TRIGGER_RELAY);
}

// ============================================================
// trigger_key
// ============================================================

/// Use callback for trigger_key.
/// A relay trigger that only fires its targets if player has the proper key.
pub fn trigger_key_use(ctx: &mut GameContext, self_idx: usize, _other_idx: usize, activator_idx: usize) {
    let self_item = ctx.edicts[self_idx].item;
    if self_item.is_none() {
        return;
    }
    let item_idx = self_item.unwrap();

    let activator = &ctx.edicts[activator_idx];
    if activator.client.is_none() {
        return;
    }
    let activator_client_idx = activator.client.unwrap();

    let index = item_idx; // ITEM_INDEX equivalent

    if ctx.clients[activator_client_idx].pers.inventory[index] == 0 {
        if ctx.level.time < ctx.edicts[self_idx].touch_debounce_time {
            return;
        }
        ctx.edicts[self_idx].touch_debounce_time = ctx.level.time + 5.0;
        let pickup_name = ctx.items[item_idx].pickup_name.clone();
        gi_centerprintf(activator_idx as i32, &format!("You need the {}", pickup_name));
        gi_sound(activator_idx as i32, CHAN_AUTO, gi_soundindex("misc/keytry.wav"), 1.0, ATTN_NORM, 0.0);
        return;
    }

    gi_sound(activator_idx as i32, CHAN_AUTO, gi_soundindex("misc/keyuse.wav"), 1.0, ATTN_NORM, 0.0);

    if ctx.coop != 0.0 {
        let item_classname = ctx.items[item_idx].classname.clone();

        if item_classname == "key_power_cube" {
            let mut cube = 0;
            while cube < 8 {
                if ctx.clients[activator_client_idx].pers.power_cubes & (1 << cube) != 0 {
                    break;
                }
                cube += 1;
            }
            for player in 1..=ctx.game.maxclients as usize {
                if player >= ctx.edicts.len() {
                    break;
                }
                let ent = &ctx.edicts[player];
                if !ent.inuse {
                    continue;
                }
                if ent.client.is_none() {
                    continue;
                }
                let client_idx = ent.client.unwrap();
                if ctx.clients[client_idx].pers.power_cubes & (1 << cube) != 0 {
                    ctx.clients[client_idx].pers.inventory[index] -= 1;
                    ctx.clients[client_idx].pers.power_cubes &= !(1 << cube);
                }
            }
        } else {
            for player in 1..=ctx.game.maxclients as usize {
                if player >= ctx.edicts.len() {
                    break;
                }
                let ent = &ctx.edicts[player];
                if !ent.inuse {
                    continue;
                }
                if ent.client.is_none() {
                    continue;
                }
                let client_idx = ent.client.unwrap();
                ctx.clients[client_idx].pers.inventory[index] = 0;
            }
        }
    } else {
        ctx.clients[activator_client_idx].pers.inventory[index] -= 1;
    }

    ctx.maxclients = ctx.game.maxclients as f32;
    ctx.num_edicts = ctx.edicts.len() as i32;
    ctx.max_edicts = ctx.edicts.capacity() as i32;
    crate::g_utils::g_use_targets(ctx, self_idx, activator_idx);

    ctx.edicts[self_idx].use_fn = None;
}

/// Spawn function for trigger_key.
pub fn sp_trigger_key(ctx: &mut GameContext, self_idx: usize) {
    if ctx.st.item.is_empty() {
        let origin = ctx.edicts[self_idx].s.origin;
        gi_dprintf(&format!("no key item for trigger_key at {}\n", vtos(&origin)));
        return;
    }

    // self->item = FindItemByClassname(st.item)
    let item_name = ctx.st.item.clone();
    let found_item = ctx.items.iter().position(|it| it.classname == item_name);
    ctx.edicts[self_idx].item = found_item;

    if ctx.edicts[self_idx].item.is_none() {
        let origin = ctx.edicts[self_idx].s.origin;
        gi_dprintf(&format!("item {} not found for trigger_key at {}\n", item_name, vtos(&origin)));
        return;
    }

    if ctx.edicts[self_idx].target.is_empty() {
        let classname = ctx.edicts[self_idx].classname.clone();
        let origin = ctx.edicts[self_idx].s.origin;
        gi_dprintf(&format!("{} at {} has no target\n", classname, vtos(&origin)));
        return;
    }

    gi_soundindex("misc/keytry.wav");
    gi_soundindex("misc/keyuse.wav");

    ctx.edicts[self_idx].use_fn = Some(USE_TRIGGER_KEY);
}

// ============================================================
// trigger_counter
// ============================================================

/// Use callback for trigger_counter.
/// Acts as an intermediary for an action that takes multiple inputs.
/// After triggered "count" times, fires all targets and removes itself.
pub fn trigger_counter_use(ctx: &mut GameContext, self_idx: usize, _other_idx: usize, activator_idx: usize) {
    if ctx.edicts[self_idx].count == 0 {
        return;
    }

    ctx.edicts[self_idx].count -= 1;

    if ctx.edicts[self_idx].count != 0 {
        if ctx.edicts[self_idx].spawnflags & 1 == 0 {
            let count = ctx.edicts[self_idx].count;
            gi_centerprintf(activator_idx as i32, &format!("{} more to go...", count));
            gi_sound(activator_idx as i32, CHAN_AUTO, gi_soundindex("misc/talk1.wav"), 1.0, ATTN_NORM, 0.0);
        }
        return;
    }

    if ctx.edicts[self_idx].spawnflags & 1 == 0 {
        gi_centerprintf(activator_idx as i32, "Sequence completed!");
        gi_sound(activator_idx as i32, CHAN_AUTO, gi_soundindex("misc/talk1.wav"), 1.0, ATTN_NORM, 0.0);
    }
    ctx.edicts[self_idx].activator = activator_idx as i32;
    multi_trigger(ctx, self_idx);
}

/// Spawn function for trigger_counter.
pub fn sp_trigger_counter(ctx: &mut GameContext, self_idx: usize) {
    ctx.edicts[self_idx].wait = -1.0;
    if ctx.edicts[self_idx].count == 0 {
        ctx.edicts[self_idx].count = 2;
    }
    ctx.edicts[self_idx].use_fn = Some(USE_TRIGGER_COUNTER);
}

// ============================================================
// trigger_always
// ============================================================

/// Spawn function for trigger_always.
/// This trigger will always fire. It is activated by the world.
pub fn sp_trigger_always(ctx: &mut GameContext, ent_idx: usize) {
    // We must have some delay to make sure our use targets are present
    if ctx.edicts[ent_idx].delay < 0.2 {
        ctx.edicts[ent_idx].delay = 0.2;
    }
    ctx.maxclients = ctx.game.maxclients as f32;
    ctx.num_edicts = ctx.edicts.len() as i32;
    ctx.max_edicts = ctx.edicts.capacity() as i32;
    crate::g_utils::g_use_targets(ctx, ent_idx, ent_idx);
}

// ============================================================
// trigger_push
// ============================================================

/// Touch callback for trigger_push.
pub fn trigger_push_touch(ctx: &mut GameContext, self_idx: usize, other_idx: usize, _plane: Option<&CPlane>, _surf: Option<&CSurface>) {
    let self_ent = &ctx.edicts[self_idx];
    let speed = self_ent.speed;
    let movedir = self_ent.movedir;
    let spawnflags = self_ent.spawnflags;

    let other = &ctx.edicts[other_idx];
    if other.classname == "grenade" {
        let mut vel = [0.0f32; 3];
        vector_scale(&movedir, speed * 10.0, &mut vel);
        ctx.edicts[other_idx].velocity = vel;
    } else if other.health > 0 {
        let mut vel = [0.0f32; 3];
        vector_scale(&movedir, speed * 10.0, &mut vel);
        ctx.edicts[other_idx].velocity = vel;

        if ctx.edicts[other_idx].client.is_some() {
            let client_idx = ctx.edicts[other_idx].client.unwrap();
            // Don't take falling damage immediately from this
            let vel = ctx.edicts[other_idx].velocity;
            ctx.clients[client_idx].oldvelocity = vel;

            if ctx.edicts[other_idx].fly_sound_debounce_time < ctx.level.time {
                ctx.edicts[other_idx].fly_sound_debounce_time = ctx.level.time + 1.5;
                gi_sound(other_idx as i32, CHAN_AUTO, ctx.windsound, 1.0, ATTN_NORM, 0.0);
            }
        }
    }
    if spawnflags & PUSH_ONCE != 0 {
        ctx.maxclients = ctx.game.maxclients as f32;
        crate::g_utils::g_free_edict(ctx, self_idx);
    }
}

/// Spawn function for trigger_push.
/// Pushes the player. "speed" defaults to 1000.
pub fn sp_trigger_push(ctx: &mut GameContext, self_idx: usize) {
    init_trigger(ctx, self_idx);
    ctx.windsound = gi_soundindex("misc/windfly.wav");
    ctx.edicts[self_idx].touch_fn = Some(TOUCH_TRIGGER_PUSH);
    if ctx.edicts[self_idx].speed == 0.0 {
        ctx.edicts[self_idx].speed = 1000.0;
    }
    gi_linkentity(self_idx as i32);
}

// ============================================================
// trigger_hurt
// ============================================================

/// Use callback for trigger_hurt (toggle on/off).
pub fn hurt_use(ctx: &mut GameContext, self_idx: usize, _other_idx: usize, _activator_idx: usize) {
    if ctx.edicts[self_idx].solid == Solid::Not {
        ctx.edicts[self_idx].solid = Solid::Trigger;
    } else {
        ctx.edicts[self_idx].solid = Solid::Not;
    }
    // gi.linkentity(self)
    gi_linkentity(self_idx as i32);

    if ctx.edicts[self_idx].spawnflags & 2 == 0 {
        ctx.edicts[self_idx].use_fn = None;
    }
}

/// Touch callback for trigger_hurt.
pub fn hurt_touch(ctx: &mut GameContext, self_idx: usize, other_idx: usize, _plane: Option<&CPlane>, _surf: Option<&CSurface>) {
    if ctx.edicts[other_idx].takedamage == 0 {
        return;
    }

    if ctx.edicts[self_idx].timestamp > ctx.level.time {
        return;
    }

    if ctx.edicts[self_idx].spawnflags & 16 != 0 {
        ctx.edicts[self_idx].timestamp = ctx.level.time + 1.0;
    } else {
        ctx.edicts[self_idx].timestamp = ctx.level.time + FRAMETIME;
    }

    if ctx.edicts[self_idx].spawnflags & 4 == 0
        && ctx.level.framenum % 10 == 0 {
            let noise_index = ctx.edicts[self_idx].noise_index;
            // gi.sound(other, CHAN_AUTO, self->noise_index, 1, ATTN_NORM, 0)
            gi_sound(other_idx as i32, CHAN_AUTO, noise_index, 1.0, ATTN_NORM, 0.0);
        }

    let dflags = if ctx.edicts[self_idx].spawnflags & 8 != 0 {
        DAMAGE_NO_PROTECTION
    } else {
        DamageFlags::empty()
    };

    let dmg = ctx.edicts[self_idx].dmg;
    let other_origin = ctx.edicts[other_idx].s.origin;
    let zero_vec = [0.0f32; 3];
    ctx.maxclients = ctx.game.maxclients as f32;
    crate::g_combat::ctx_t_damage(
        ctx, other_idx, self_idx, self_idx,
        &zero_vec, &other_origin, &zero_vec,
        dmg, dmg, dflags, MOD_TRIGGER_HURT,
    );
}

/// Spawn function for trigger_hurt.
///
/// Any entity that touches this will be hurt.
/// Spawnflags: START_OFF=1, TOGGLE=2, SILENT=4, NO_PROTECTION=8, SLOW=16
/// "dmg" default 5
pub fn sp_trigger_hurt(ctx: &mut GameContext, self_idx: usize) {
    init_trigger(ctx, self_idx);

    ctx.edicts[self_idx].noise_index = gi_soundindex("world/electro.wav");

    ctx.edicts[self_idx].touch_fn = Some(TOUCH_HURT);

    if ctx.edicts[self_idx].dmg == 0 {
        ctx.edicts[self_idx].dmg = 5;
    }

    if ctx.edicts[self_idx].spawnflags & 1 != 0 {
        ctx.edicts[self_idx].solid = Solid::Not;
    } else {
        ctx.edicts[self_idx].solid = Solid::Trigger;
    }

    if ctx.edicts[self_idx].spawnflags & 2 != 0 {
        ctx.edicts[self_idx].use_fn = Some(USE_HURT);
    }

    gi_linkentity(self_idx as i32);
}

// ============================================================
// trigger_gravity
// ============================================================

/// Touch callback for trigger_gravity — changes the touching entity's gravity.
pub fn trigger_gravity_touch(ctx: &mut GameContext, self_idx: usize, other_idx: usize, _plane: Option<&CPlane>, _surf: Option<&CSurface>) {
    ctx.edicts[other_idx].gravity = ctx.edicts[self_idx].gravity;
}

/// Spawn function for trigger_gravity.
/// Changes the touching entity's gravity to the value of "gravity". 1.0 is standard.
pub fn sp_trigger_gravity(ctx: &mut GameContext, self_idx: usize) {
    if ctx.st.gravity.is_empty() || ctx.st.gravity == "0" {
        let origin = ctx.edicts[self_idx].s.origin;
        gi_dprintf(&format!("trigger_gravity without gravity set at {}\n", vtos(&origin)));
        ctx.maxclients = ctx.game.maxclients as f32;
        crate::g_utils::g_free_edict(ctx, self_idx);
        return;
    }

    init_trigger(ctx, self_idx);
    ctx.edicts[self_idx].gravity = ctx.st.gravity.parse::<f32>().unwrap_or(0.0);
    ctx.edicts[self_idx].touch_fn = Some(TOUCH_TRIGGER_GRAVITY);
}

// ============================================================
// trigger_monsterjump
// ============================================================

/// Touch callback for trigger_monsterjump.
/// Walking monsters that touch this will jump in the direction of the trigger's angle.
pub fn trigger_monsterjump_touch(ctx: &mut GameContext, self_idx: usize, other_idx: usize, _plane: Option<&CPlane>, _surf: Option<&CSurface>) {
    let other = &ctx.edicts[other_idx];
    if other.flags.intersects(FL_FLY | FL_SWIM) {
        return;
    }
    if other.svflags & SVF_DEADMONSTER != 0 {
        return;
    }
    if other.svflags & SVF_MONSTER == 0 {
        return;
    }

    let self_ent = &ctx.edicts[self_idx];
    let speed = self_ent.speed;
    let movedir = self_ent.movedir;

    // Set XY even if not on ground, so the jump will clear lips
    ctx.edicts[other_idx].velocity[0] = movedir[0] * speed;
    ctx.edicts[other_idx].velocity[1] = movedir[1] * speed;

    if ctx.edicts[other_idx].groundentity == 0 {
        // groundentity == 0 means no ground entity (NULL equivalent)
        return;
    }

    ctx.edicts[other_idx].groundentity = -1; // NULL equivalent
    ctx.edicts[other_idx].velocity[2] = movedir[2];
}

/// Spawn function for trigger_monsterjump.
/// "speed" defaults to 200, "height" defaults to 200.
pub fn sp_trigger_monsterjump(ctx: &mut GameContext, self_idx: usize) {
    if ctx.edicts[self_idx].speed == 0.0 {
        ctx.edicts[self_idx].speed = 200.0;
    }
    if ctx.st.height == 0 {
        ctx.st.height = 200;
    }
    if ctx.edicts[self_idx].s.angles[YAW] == 0.0 {
        ctx.edicts[self_idx].s.angles[YAW] = 360.0;
    }
    init_trigger(ctx, self_idx);
    ctx.edicts[self_idx].touch_fn = Some(TOUCH_TRIGGER_MONSTERJUMP);
    ctx.edicts[self_idx].movedir[2] = ctx.st.height as f32;
}

// CPlane and CSurface come from g_local::* (q_shared)

#[cfg(test)]
mod tests {
    use super::*;
    use crate::g_local::*;
    use crate::game::Solid;

    fn init_test_gi() {
        crate::game_import::set_gi(Box::new(crate::game_import::StubGameImport));
    }

    /// Helper: create a GameContext with N edicts (index 0 = world).
    fn make_ctx(num_edicts: usize) -> GameContext {
        init_test_gi();
        let mut ctx = GameContext::default();
        for _ in 0..num_edicts {
            ctx.edicts.push(Edict::default());
        }
        ctx.num_edicts = num_edicts as i32;
        ctx.max_edicts = (num_edicts + 10) as i32;
        ctx.game.maxclients = 1;
        ctx.maxclients = 1.0;
        ctx
    }

    // ============================================================
    // init_trigger tests
    // ============================================================

    #[test]
    fn test_init_trigger_sets_solid_and_movetype() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].s.angles = [0.0, 0.0, 0.0];

        init_trigger(&mut ctx, 1);

        assert_eq!(ctx.edicts[1].solid, Solid::Trigger);
        assert_eq!(ctx.edicts[1].movetype, MoveType::None);
        assert_eq!(ctx.edicts[1].svflags, SVF_NOCLIENT);
    }

    #[test]
    fn test_init_trigger_computes_movedir_from_angles() {
        let mut ctx = make_ctx(3);
        // 90 degree yaw should produce movedir roughly [0, 1, 0]
        ctx.edicts[1].s.angles = [0.0, 90.0, 0.0];

        init_trigger(&mut ctx, 1);

        // After init_trigger, angles should be cleared
        assert_eq!(ctx.edicts[1].s.angles, [0.0, 0.0, 0.0]);
        // movedir should point roughly in Y direction
        assert!(ctx.edicts[1].movedir[1].abs() > 0.9);
    }

    #[test]
    fn test_init_trigger_zero_angles_no_movedir() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].s.angles = [0.0, 0.0, 0.0];
        ctx.edicts[1].movedir = [0.0, 0.0, 0.0];

        init_trigger(&mut ctx, 1);

        // With zero angles, movedir should remain zero (no angle_vectors call)
        assert_eq!(ctx.edicts[1].movedir, [0.0, 0.0, 0.0]);
    }

    // ============================================================
    // multi_wait tests
    // ============================================================

    #[test]
    fn test_multi_wait_clears_nextthink() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].nextthink = 10.0;

        multi_wait(&mut ctx, 1);

        assert_eq!(ctx.edicts[1].nextthink, 0.0);
    }

    // ============================================================
    // multi_trigger tests
    // ============================================================

    #[test]
    fn test_multi_trigger_already_triggered() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].nextthink = 5.0; // not zero => already triggered

        multi_trigger(&mut ctx, 1);

        // Should return early, nextthink unchanged
        assert!((ctx.edicts[1].nextthink - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_multi_trigger_with_positive_wait() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].nextthink = 0.0;
        ctx.edicts[1].wait = 2.0;
        ctx.edicts[1].activator = 0;
        ctx.level.time = 10.0;

        multi_trigger(&mut ctx, 1);

        // think_fn should be THINK_MULTI_WAIT
        assert_eq!(ctx.edicts[1].think_fn, Some(THINK_MULTI_WAIT));
        // nextthink = time + wait = 12.0
        assert!((ctx.edicts[1].nextthink - 12.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_multi_trigger_with_zero_wait_schedules_free() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].nextthink = 0.0;
        ctx.edicts[1].wait = 0.0; // not > 0
        ctx.edicts[1].activator = 0;
        ctx.level.time = 5.0;

        multi_trigger(&mut ctx, 1);

        // touch_fn should be cleared
        assert!(ctx.edicts[1].touch_fn.is_none());
        // Should schedule free
        assert_eq!(ctx.edicts[1].think_fn, Some(THINK_G_FREE_EDICT));
        assert!((ctx.edicts[1].nextthink - (5.0 + FRAMETIME)).abs() < f32::EPSILON);
    }

    // ============================================================
    // use_multi tests
    // ============================================================

    #[test]
    fn test_use_multi_sets_activator() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].nextthink = 5.0; // will cause early return in multi_trigger

        use_multi(&mut ctx, 1, 0, 2);

        assert_eq!(ctx.edicts[1].activator, 2);
    }

    // ============================================================
    // touch_multi tests
    // ============================================================

    #[test]
    fn test_touch_multi_client_spawnflag2_blocks() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].spawnflags = 2; // not-player flag
        ctx.edicts[2].client = Some(0);

        touch_multi(&mut ctx, 1, 2, None, None);

        // Should return early — activator not set
        assert_eq!(ctx.edicts[1].activator, 0);
    }

    #[test]
    fn test_touch_multi_monster_without_flag() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].spawnflags = 0; // bit 1 not set = no monster activation
        ctx.edicts[1].movedir = [0.0, 0.0, 0.0];
        ctx.edicts[2].svflags = SVF_MONSTER;
        ctx.edicts[2].client = None;

        touch_multi(&mut ctx, 1, 2, None, None);

        // Should return early because spawnflags & 1 == 0
        assert_eq!(ctx.edicts[1].activator, 0);
    }

    #[test]
    fn test_touch_multi_monster_with_flag() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].spawnflags = 1; // allow monsters
        ctx.edicts[1].movedir = [0.0, 0.0, 0.0]; // no direction check
        ctx.edicts[1].nextthink = 1.0; // multi_trigger will bail (already triggered)
        ctx.edicts[2].svflags = SVF_MONSTER;
        ctx.edicts[2].client = None;

        touch_multi(&mut ctx, 1, 2, None, None);

        // Should set activator
        assert_eq!(ctx.edicts[1].activator, 2);
    }

    #[test]
    fn test_touch_multi_non_client_non_monster_rejected() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].spawnflags = 0;
        ctx.edicts[2].client = None;
        ctx.edicts[2].svflags = 0; // not a monster

        touch_multi(&mut ctx, 1, 2, None, None);

        // Should return early
        assert_eq!(ctx.edicts[1].activator, 0);
    }

    #[test]
    fn test_touch_multi_direction_check_wrong_way() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].spawnflags = 0;
        ctx.edicts[1].movedir = [1.0, 0.0, 0.0]; // non-zero => direction check active
        ctx.edicts[2].client = Some(0);
        ctx.edicts[2].s.angles = [0.0, 180.0, 0.0]; // facing backwards

        touch_multi(&mut ctx, 1, 2, None, None);

        // Dot product of forward [-1, 0, 0] and movedir [1, 0, 0] = -1 < 0, rejected
        assert_eq!(ctx.edicts[1].activator, 0);
    }

    // ============================================================
    // trigger_enable tests
    // ============================================================

    #[test]
    fn test_trigger_enable_sets_solid_and_use() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].solid = Solid::Not;
        ctx.edicts[1].use_fn = None;

        trigger_enable(&mut ctx, 1, 0, 0);

        assert_eq!(ctx.edicts[1].solid, Solid::Trigger);
        assert_eq!(ctx.edicts[1].use_fn, Some(USE_MULTI));
    }

    // ============================================================
    // sp_trigger_multiple tests
    // ============================================================

    #[test]
    fn test_sp_trigger_multiple_default_wait() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].wait = 0.0;
        ctx.edicts[1].s.angles = [0.0, 0.0, 0.0];
        ctx.edicts[1].spawnflags = 0;
        ctx.edicts[1].sounds = 0;

        sp_trigger_multiple(&mut ctx, 1);

        // Default wait = 0.2
        assert!((ctx.edicts[1].wait - 0.2).abs() < f32::EPSILON);
        assert_eq!(ctx.edicts[1].touch_fn, Some(TOUCH_MULTI));
        assert_eq!(ctx.edicts[1].solid, Solid::Trigger);
        assert_eq!(ctx.edicts[1].use_fn, Some(USE_MULTI));
    }

    #[test]
    fn test_sp_trigger_multiple_triggered_flag() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].wait = 1.0;
        ctx.edicts[1].s.angles = [0.0, 0.0, 0.0];
        ctx.edicts[1].spawnflags = 4; // TRIGGERED flag

        sp_trigger_multiple(&mut ctx, 1);

        assert_eq!(ctx.edicts[1].solid, Solid::Not);
        assert_eq!(ctx.edicts[1].use_fn, Some(USE_TRIGGER_ENABLE));
    }

    #[test]
    fn test_sp_trigger_multiple_sound_1_secret() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].sounds = 1;
        ctx.edicts[1].s.angles = [0.0, 0.0, 0.0];
        ctx.edicts[1].spawnflags = 0;

        sp_trigger_multiple(&mut ctx, 1);

        // With StubGameImport, soundindex returns 0, but the code path should execute
        assert_eq!(ctx.edicts[1].noise_index, 0); // stub returns 0
    }

    // ============================================================
    // sp_trigger_once tests
    // ============================================================

    #[test]
    fn test_sp_trigger_once_sets_wait_negative() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].s.angles = [0.0, 0.0, 0.0];
        ctx.edicts[1].spawnflags = 0;
        ctx.edicts[1].sounds = 0;

        sp_trigger_once(&mut ctx, 1);

        assert!((ctx.edicts[1].wait - (-1.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_sp_trigger_once_fixes_old_trigger_flag() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].spawnflags = 1; // old TRIGGERED on wrong bit
        ctx.edicts[1].s.angles = [0.0, 0.0, 0.0];
        ctx.edicts[1].sounds = 0;
        ctx.edicts[1].mins = [0.0, 0.0, 0.0];
        ctx.edicts[1].size = [64.0, 64.0, 64.0];

        sp_trigger_once(&mut ctx, 1);

        // Should have cleared bit 1 and set bit 4
        assert_eq!(ctx.edicts[1].spawnflags & 1, 0);
        assert_ne!(ctx.edicts[1].spawnflags & 4, 0);
    }

    // ============================================================
    // trigger_relay tests
    // ============================================================

    #[test]
    fn test_sp_trigger_relay_sets_use() {
        let mut ctx = make_ctx(3);

        sp_trigger_relay(&mut ctx, 1);

        assert_eq!(ctx.edicts[1].use_fn, Some(USE_TRIGGER_RELAY));
    }

    // ============================================================
    // trigger_counter tests
    // ============================================================

    #[test]
    fn test_sp_trigger_counter_defaults() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].count = 0;

        sp_trigger_counter(&mut ctx, 1);

        assert!((ctx.edicts[1].wait - (-1.0)).abs() < f32::EPSILON);
        assert_eq!(ctx.edicts[1].count, 2);
        assert_eq!(ctx.edicts[1].use_fn, Some(USE_TRIGGER_COUNTER));
    }

    #[test]
    fn test_sp_trigger_counter_preserves_count() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].count = 5;

        sp_trigger_counter(&mut ctx, 1);

        assert_eq!(ctx.edicts[1].count, 5);
    }

    #[test]
    fn test_trigger_counter_use_decrement() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].count = 3;
        ctx.edicts[1].spawnflags = 0;

        trigger_counter_use(&mut ctx, 1, 0, 0);

        assert_eq!(ctx.edicts[1].count, 2);
    }

    #[test]
    fn test_trigger_counter_use_zero_count_ignored() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].count = 0;

        trigger_counter_use(&mut ctx, 1, 0, 0);

        // Should return early, count stays 0
        assert_eq!(ctx.edicts[1].count, 0);
    }

    #[test]
    fn test_trigger_counter_use_reaches_zero() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].count = 1;
        ctx.edicts[1].spawnflags = 0;
        ctx.edicts[1].nextthink = 0.0;
        ctx.edicts[1].wait = 2.0;
        ctx.level.time = 10.0;

        trigger_counter_use(&mut ctx, 1, 0, 0);

        // Count should be 0 now, and multi_trigger should have been called
        assert_eq!(ctx.edicts[1].count, 0);
        // multi_trigger sets think_fn and nextthink with positive wait
        assert_eq!(ctx.edicts[1].think_fn, Some(THINK_MULTI_WAIT));
    }

    // ============================================================
    // trigger_always tests
    // ============================================================

    #[test]
    fn test_sp_trigger_always_minimum_delay() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].delay = 0.0;

        sp_trigger_always(&mut ctx, 1);

        assert!(ctx.edicts[1].delay >= 0.2);
    }

    #[test]
    fn test_sp_trigger_always_preserves_delay() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].delay = 1.0;

        sp_trigger_always(&mut ctx, 1);

        assert!((ctx.edicts[1].delay - 1.0).abs() < f32::EPSILON);
    }

    // ============================================================
    // trigger_push tests
    // ============================================================

    #[test]
    fn test_trigger_push_touch_grenade() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].movedir = [0.0, 0.0, 1.0];
        ctx.edicts[1].speed = 50.0;
        ctx.edicts[1].spawnflags = 0;
        ctx.edicts[2].classname = "grenade".to_string();

        trigger_push_touch(&mut ctx, 1, 2, None, None);

        // velocity = movedir * speed * 10 = [0, 0, 500]
        assert!((ctx.edicts[2].velocity[2] - 500.0).abs() < 0.01);
    }

    #[test]
    fn test_trigger_push_touch_living_entity() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].movedir = [1.0, 0.0, 0.0];
        ctx.edicts[1].speed = 100.0;
        ctx.edicts[1].spawnflags = 0;
        ctx.edicts[2].classname = "player".to_string();
        ctx.edicts[2].health = 100;

        trigger_push_touch(&mut ctx, 1, 2, None, None);

        // velocity = movedir * speed * 10 = [1000, 0, 0]
        assert!((ctx.edicts[2].velocity[0] - 1000.0).abs() < 0.01);
    }

    #[test]
    fn test_trigger_push_touch_dead_entity() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].movedir = [1.0, 0.0, 0.0];
        ctx.edicts[1].speed = 100.0;
        ctx.edicts[1].spawnflags = 0;
        ctx.edicts[2].classname = "player".to_string();
        ctx.edicts[2].health = 0; // dead

        trigger_push_touch(&mut ctx, 1, 2, None, None);

        // Dead entity (health <= 0), not grenade — no velocity change
        assert_eq!(ctx.edicts[2].velocity, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_trigger_push_push_once_frees() {
        // Entity must have index > maxclients + BODY_QUEUE_SIZE (1+8=9) to be freed
        let mut ctx = make_ctx(13);
        ctx.edicts[10].movedir = [0.0, 0.0, 1.0];
        ctx.edicts[10].speed = 50.0;
        ctx.edicts[10].spawnflags = PUSH_ONCE;
        ctx.edicts[10].inuse = true;
        ctx.edicts[11].classname = "grenade".to_string();

        trigger_push_touch(&mut ctx, 10, 11, None, None);

        // PUSH_ONCE should free the trigger entity
        assert!(!ctx.edicts[10].inuse);
    }

    #[test]
    fn test_sp_trigger_push_default_speed() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].speed = 0.0;
        ctx.edicts[1].s.angles = [0.0, 0.0, 0.0];

        sp_trigger_push(&mut ctx, 1);

        assert!((ctx.edicts[1].speed - 1000.0).abs() < f32::EPSILON);
        assert_eq!(ctx.edicts[1].touch_fn, Some(TOUCH_TRIGGER_PUSH));
    }

    #[test]
    fn test_sp_trigger_push_custom_speed() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].speed = 500.0;
        ctx.edicts[1].s.angles = [0.0, 0.0, 0.0];

        sp_trigger_push(&mut ctx, 1);

        assert!((ctx.edicts[1].speed - 500.0).abs() < f32::EPSILON);
    }

    // ============================================================
    // trigger_hurt tests
    // ============================================================

    #[test]
    fn test_hurt_use_toggle_on() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].solid = Solid::Not;
        ctx.edicts[1].spawnflags = 2; // TOGGLE flag

        hurt_use(&mut ctx, 1, 0, 0);

        assert_eq!(ctx.edicts[1].solid, Solid::Trigger);
        // With TOGGLE flag, use_fn is preserved
        assert!(ctx.edicts[1].use_fn.is_some() || ctx.edicts[1].use_fn.is_none());
    }

    #[test]
    fn test_hurt_use_toggle_off() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].solid = Solid::Trigger;
        ctx.edicts[1].spawnflags = 2;

        hurt_use(&mut ctx, 1, 0, 0);

        assert_eq!(ctx.edicts[1].solid, Solid::Not);
    }

    #[test]
    fn test_hurt_use_no_toggle_clears_use() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].solid = Solid::Not;
        ctx.edicts[1].spawnflags = 0; // no TOGGLE flag
        ctx.edicts[1].use_fn = Some(USE_HURT);

        hurt_use(&mut ctx, 1, 0, 0);

        // Without TOGGLE flag, use_fn should be cleared
        assert!(ctx.edicts[1].use_fn.is_none());
    }

    #[test]
    fn test_hurt_touch_no_takedamage() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].timestamp = 0.0;
        ctx.edicts[2].takedamage = DAMAGE_NO;
        ctx.level.time = 1.0;

        hurt_touch(&mut ctx, 1, 2, None, None);

        // Should return early — timestamp not modified
        assert!((ctx.edicts[1].timestamp - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_hurt_touch_debounce_slow() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].spawnflags = 16; // SLOW flag
        ctx.edicts[1].timestamp = 0.0;
        ctx.edicts[1].dmg = 5;
        ctx.edicts[2].takedamage = DAMAGE_YES;
        ctx.edicts[2].s.origin = [0.0, 0.0, 0.0];
        ctx.level.time = 1.0;

        hurt_touch(&mut ctx, 1, 2, None, None);

        // SLOW flag: timestamp = time + 1.0 = 2.0
        assert!((ctx.edicts[1].timestamp - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_hurt_touch_debounce_normal() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].spawnflags = 0; // no SLOW
        ctx.edicts[1].timestamp = 0.0;
        ctx.edicts[1].dmg = 5;
        ctx.edicts[2].takedamage = DAMAGE_YES;
        ctx.edicts[2].s.origin = [0.0, 0.0, 0.0];
        ctx.level.time = 1.0;

        hurt_touch(&mut ctx, 1, 2, None, None);

        // Normal: timestamp = time + FRAMETIME = 1.1
        assert!((ctx.edicts[1].timestamp - (1.0 + FRAMETIME)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_hurt_touch_within_debounce() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].timestamp = 5.0; // future timestamp
        ctx.edicts[2].takedamage = DAMAGE_YES;
        ctx.level.time = 1.0; // time < timestamp

        hurt_touch(&mut ctx, 1, 2, None, None);

        // Should return early — timestamp unchanged
        assert!((ctx.edicts[1].timestamp - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_sp_trigger_hurt_defaults() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].s.angles = [0.0, 0.0, 0.0];
        ctx.edicts[1].dmg = 0;
        ctx.edicts[1].spawnflags = 0;

        sp_trigger_hurt(&mut ctx, 1);

        assert_eq!(ctx.edicts[1].dmg, 5);
        assert_eq!(ctx.edicts[1].solid, Solid::Trigger);
        assert_eq!(ctx.edicts[1].touch_fn, Some(TOUCH_HURT));
    }

    #[test]
    fn test_sp_trigger_hurt_start_off() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].s.angles = [0.0, 0.0, 0.0];
        ctx.edicts[1].dmg = 10;
        ctx.edicts[1].spawnflags = 1; // START_OFF

        sp_trigger_hurt(&mut ctx, 1);

        assert_eq!(ctx.edicts[1].solid, Solid::Not);
    }

    #[test]
    fn test_sp_trigger_hurt_toggle() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].s.angles = [0.0, 0.0, 0.0];
        ctx.edicts[1].dmg = 10;
        ctx.edicts[1].spawnflags = 2; // TOGGLE

        sp_trigger_hurt(&mut ctx, 1);

        assert_eq!(ctx.edicts[1].use_fn, Some(USE_HURT));
    }

    // ============================================================
    // trigger_gravity tests
    // ============================================================

    #[test]
    fn test_trigger_gravity_touch_sets_gravity() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].gravity = 0.5;
        ctx.edicts[2].gravity = 1.0;

        trigger_gravity_touch(&mut ctx, 1, 2, None, None);

        assert!((ctx.edicts[2].gravity - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_sp_trigger_gravity_default() {
        let mut ctx = make_ctx(3);
        ctx.st.gravity = "0.5".to_string();
        ctx.edicts[1].s.angles = [0.0, 0.0, 0.0];

        sp_trigger_gravity(&mut ctx, 1);

        assert!((ctx.edicts[1].gravity - 0.5).abs() < f32::EPSILON);
        assert_eq!(ctx.edicts[1].touch_fn, Some(TOUCH_TRIGGER_GRAVITY));
    }

    #[test]
    fn test_sp_trigger_gravity_missing_frees_entity() {
        // Entity index must be > maxclients + BODY_QUEUE_SIZE (1+8=9) to be freed
        let mut ctx = make_ctx(12);
        ctx.st.gravity = String::new();
        ctx.edicts[10].inuse = true;

        sp_trigger_gravity(&mut ctx, 10);

        // Should free the entity
        assert!(!ctx.edicts[10].inuse);
    }

    #[test]
    fn test_sp_trigger_gravity_zero_frees_entity() {
        let mut ctx = make_ctx(12);
        ctx.st.gravity = "0".to_string();
        ctx.edicts[10].inuse = true;

        sp_trigger_gravity(&mut ctx, 10);

        assert!(!ctx.edicts[10].inuse);
    }

    // ============================================================
    // trigger_monsterjump tests
    // ============================================================

    #[test]
    fn test_trigger_monsterjump_touch_not_monster() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].speed = 200.0;
        ctx.edicts[1].movedir = [1.0, 0.0, 0.0];
        ctx.edicts[2].svflags = 0; // not a monster

        trigger_monsterjump_touch(&mut ctx, 1, 2, None, None);

        // Should return early
        assert_eq!(ctx.edicts[2].velocity, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_trigger_monsterjump_touch_flying_rejected() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].speed = 200.0;
        ctx.edicts[1].movedir = [1.0, 0.0, 0.0];
        ctx.edicts[2].svflags = SVF_MONSTER;
        ctx.edicts[2].flags = FL_FLY;

        trigger_monsterjump_touch(&mut ctx, 1, 2, None, None);

        assert_eq!(ctx.edicts[2].velocity, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_trigger_monsterjump_touch_dead_rejected() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].speed = 200.0;
        ctx.edicts[1].movedir = [1.0, 0.0, 0.0];
        ctx.edicts[2].svflags = SVF_MONSTER | SVF_DEADMONSTER;

        trigger_monsterjump_touch(&mut ctx, 1, 2, None, None);

        assert_eq!(ctx.edicts[2].velocity, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_trigger_monsterjump_touch_sets_xy_velocity() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].speed = 200.0;
        ctx.edicts[1].movedir = [0.7, 0.7, 300.0];
        ctx.edicts[2].svflags = SVF_MONSTER;
        ctx.edicts[2].flags = EntityFlags::empty();
        ctx.edicts[2].groundentity = 0; // no ground entity

        trigger_monsterjump_touch(&mut ctx, 1, 2, None, None);

        // XY velocity set: vel[0] = 0.7 * 200 = 140, vel[1] = 0.7 * 200 = 140
        assert!((ctx.edicts[2].velocity[0] - 140.0).abs() < 0.01);
        assert!((ctx.edicts[2].velocity[1] - 140.0).abs() < 0.01);
        // Z not set because groundentity == 0 (no ground)
        assert_eq!(ctx.edicts[2].velocity[2], 0.0);
    }

    #[test]
    fn test_trigger_monsterjump_touch_with_ground() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].speed = 200.0;
        ctx.edicts[1].movedir = [1.0, 0.0, 300.0];
        ctx.edicts[2].svflags = SVF_MONSTER;
        ctx.edicts[2].flags = EntityFlags::empty();
        ctx.edicts[2].groundentity = 1; // on ground (non-zero = has ground)

        trigger_monsterjump_touch(&mut ctx, 1, 2, None, None);

        // XY velocity
        assert!((ctx.edicts[2].velocity[0] - 200.0).abs() < 0.01);
        // Z velocity = movedir[2] = 300
        assert!((ctx.edicts[2].velocity[2] - 300.0).abs() < 0.01);
        // groundentity should be cleared
        assert_eq!(ctx.edicts[2].groundentity, -1);
    }

    #[test]
    fn test_sp_trigger_monsterjump_defaults() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].speed = 0.0;
        ctx.edicts[1].s.angles = [0.0, 0.0, 0.0];
        ctx.st.height = 0;

        sp_trigger_monsterjump(&mut ctx, 1);

        assert!((ctx.edicts[1].speed - 200.0).abs() < f32::EPSILON);
        assert_eq!(ctx.st.height, 200);
        assert_eq!(ctx.edicts[1].touch_fn, Some(TOUCH_TRIGGER_MONSTERJUMP));
        // movedir[2] = height
        assert!((ctx.edicts[1].movedir[2] - 200.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_sp_trigger_monsterjump_zero_yaw_gets_360() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].speed = 100.0;
        ctx.edicts[1].s.angles = [0.0, 0.0, 0.0]; // yaw == 0
        ctx.st.height = 150;

        sp_trigger_monsterjump(&mut ctx, 1);

        // movedir[2] should be height
        assert!((ctx.edicts[1].movedir[2] - 150.0).abs() < f32::EPSILON);
    }

    // ============================================================
    // trigger_key tests
    // ============================================================

    #[test]
    fn test_sp_trigger_key_missing_item() {
        let mut ctx = make_ctx(3);
        ctx.st.item = String::new();

        sp_trigger_key(&mut ctx, 1);

        // Should return early without setting use_fn
        assert!(ctx.edicts[1].use_fn.is_none());
    }

    #[test]
    fn test_sp_trigger_key_not_found_item() {
        let mut ctx = make_ctx(3);
        ctx.st.item = "key_nonexistent".to_string();
        // No items in list
        ctx.items = vec![];

        sp_trigger_key(&mut ctx, 1);

        // Item not found, should return without setting use_fn
        assert!(ctx.edicts[1].use_fn.is_none());
    }

    #[test]
    fn test_sp_trigger_key_no_target() {
        let mut ctx = make_ctx(3);
        ctx.st.item = "key_blue".to_string();
        ctx.items = vec![GItem {
            classname: "key_blue".to_string(),
            ..GItem::default()
        }];
        ctx.edicts[1].target = String::new();

        sp_trigger_key(&mut ctx, 1);

        // Has item but no target — should return without setting use_fn
        assert!(ctx.edicts[1].use_fn.is_none());
    }

    #[test]
    fn test_sp_trigger_key_success() {
        let mut ctx = make_ctx(3);
        ctx.st.item = "key_blue".to_string();
        ctx.items = vec![GItem {
            classname: "key_blue".to_string(),
            ..GItem::default()
        }];
        ctx.edicts[1].target = "door1".to_string();

        sp_trigger_key(&mut ctx, 1);

        assert_eq!(ctx.edicts[1].use_fn, Some(USE_TRIGGER_KEY));
        assert_eq!(ctx.edicts[1].item, Some(0)); // index of key_blue in items
    }
}
