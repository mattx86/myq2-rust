// g_items.rs — Item pickup, use, drop logic and item table
// Converted from: myq2-original/game/g_items.c
//
// Copyright (C) 1997-2001 Id Software, Inc.
// Licensed under the GNU General Public License v2+

use crate::g_local::*;
use crate::game::*;
use crate::game_import::*;
use crate::g_cmds::validate_selected_item;
use crate::g_utils::g_spawn;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

/// Global item lookup indices — populated once during build_item_indices, read by find_item/find_item_by_classname.
static ITEM_BY_CLASSNAME_INDEX: LazyLock<Mutex<HashMap<String, usize>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static ITEM_BY_PICKUP_NAME_INDEX: LazyLock<Mutex<HashMap<String, usize>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

// ============================================================
// Callback function IDs for item dispatch
// ============================================================

// Pickup function IDs
pub const PICKUP_ARMOR: usize = 1;
pub const PICKUP_WEAPON: usize = 2;
pub const PICKUP_AMMO: usize = 3;
pub const PICKUP_POWERUP: usize = 4;
pub const PICKUP_KEY: usize = 5;
pub const PICKUP_HEALTH: usize = 6;
pub const PICKUP_ADRENALINE: usize = 7;
pub const PICKUP_ANCIENTHEAD: usize = 8;
pub const PICKUP_BANDOLIER: usize = 9;
pub const PICKUP_PACK: usize = 10;
pub const PICKUP_POWERARMOR: usize = 11;

// Use function IDs
pub const USE_WEAPON: usize = 1;
pub const USE_QUAD: usize = 2;
pub const USE_BREATHER: usize = 3;
pub const USE_ENVIROSUIT: usize = 4;
pub const USE_INVULNERABILITY: usize = 5;
pub const USE_SILENCER: usize = 6;
pub const USE_POWERARMOR: usize = 7;

// Drop function IDs
pub const DROP_WEAPON: usize = 1;
pub const DROP_AMMO: usize = 2;
pub const DROP_GENERAL: usize = 3;
pub const DROP_POWERARMOR: usize = 4;

// Weapon think function IDs
pub const WEAPTHINK_BLASTER: usize = 1;
pub const WEAPTHINK_SHOTGUN: usize = 2;
pub const WEAPTHINK_SUPERSHOTGUN: usize = 3;
pub const WEAPTHINK_MACHINEGUN: usize = 4;
pub const WEAPTHINK_CHAINGUN: usize = 5;
pub const WEAPTHINK_HYPERBLASTER: usize = 6;
pub const WEAPTHINK_ROCKETLAUNCHER: usize = 7;
pub const WEAPTHINK_GRENADE: usize = 8;
pub const WEAPTHINK_GRENADELAUNCHER: usize = 9;
pub const WEAPTHINK_RAILGUN: usize = 10;
pub const WEAPTHINK_BFG: usize = 11;

// Think function IDs (for edict think callbacks) — use central dispatch indices
pub use crate::dispatch::THINK_DO_RESPAWN;
pub use crate::dispatch::THINK_MEGAHEALTH_THINK as THINK_MEGAHEALTH;
pub use crate::dispatch::THINK_DROP_MAKE_TOUCHABLE;
pub use crate::dispatch::THINK_DROPTOFLOOR;

// Touch function IDs — use central dispatch indices
pub use crate::dispatch::TOUCH_ITEM;
pub use crate::dispatch::TOUCH_DROP_TEMP;

// Use (entity trigger) function IDs — use central dispatch indices
pub use crate::dispatch::USE_ITEM_TRIGGER;

// ============================================================
// Armor info definitions
// ============================================================

pub fn jacketarmor_info() -> GItemArmor {
    GItemArmor {
        base_count: 25,
        max_count: 50,
        normal_protection: 0.30,
        energy_protection: 0.00,
        armor: ARMOR_JACKET,
    }
}

pub fn combatarmor_info() -> GItemArmor {
    GItemArmor {
        base_count: 50,
        max_count: 100,
        normal_protection: 0.60,
        energy_protection: 0.30,
        armor: ARMOR_COMBAT,
    }
}

pub fn bodyarmor_info() -> GItemArmor {
    GItemArmor {
        base_count: 100,
        max_count: 200,
        normal_protection: 0.80,
        energy_protection: 0.60,
        armor: ARMOR_BODY,
    }
}

// ============================================================
// Health style flags
// ============================================================

pub const HEALTH_IGNORE_MAX: i32 = 1;
pub const HEALTH_TIMED: i32 = 2;

// ============================================================
// Ammo tag constants (matching C AMMO_* values)
// ============================================================

pub const AMMO_BULLETS: i32 = 0;
pub const AMMO_SHELLS: i32 = 1;
pub const AMMO_ROCKETS: i32 = 2;
pub const AMMO_GRENADES: i32 = 3;
pub const AMMO_CELLS: i32 = 4;
pub const AMMO_SLUGS: i32 = 5;

// EF_*, RF_*, CHAN_*, ATTN_* come from g_local::* re-export (myq2_common::q_shared)
// Use ATTN_NORM_F for f32 attenuation in gi_sound calls
// STAT_PICKUP_ICON, STAT_PICKUP_STRING, STAT_SELECTED_ITEM come from g_local::* (q_shared)
// CS_ITEMS comes from g_local (re-exported from q_shared)
// DF_* flags come from g_local::* (q_shared)

// CONTENTS_SOLID, MASK_SOLID, MAX_QPATH come from g_local::* (q_shared)

// ============================================================
// Module-level state (replaces C statics)
// ============================================================

#[derive(Default)]
pub struct ItemsState {
    pub jacket_armor_index: usize,
    pub combat_armor_index: usize,
    pub body_armor_index: usize,
    pub power_screen_index: usize,
    pub power_shield_index: usize,
    pub quad_drop_timeout_hack: i32,
}


impl GameContext {
    /// Build item lookup indices for O(1) search. Call after itemlist is populated.
    /// Also updates global static indices so cross-module lookups work without context.
    pub fn build_item_indices(&mut self) {
        self.item_by_classname.clear();
        self.item_by_pickup_name.clear();

        let num_items = self.game.num_items as usize;
        for (i, item) in self.items.iter().take(num_items).enumerate() {
            if !item.classname.is_empty() {
                self.item_by_classname.insert(item.classname.to_lowercase(), i);
            }
            if !item.pickup_name.is_empty() {
                self.item_by_pickup_name.insert(item.pickup_name.to_lowercase(), i);
            }
        }

        // Update global statics for cross-module access
        *ITEM_BY_CLASSNAME_INDEX.lock().unwrap() = self.item_by_classname.clone();
        *ITEM_BY_PICKUP_NAME_INDEX.lock().unwrap() = self.item_by_pickup_name.clone();
    }

    /// O(1) lookup of item by classname
    pub fn find_item_by_classname_fast(&self, classname: &str) -> Option<usize> {
        self.item_by_classname.get(&classname.to_lowercase()).copied()
    }

    /// O(1) lookup of item by pickup name
    pub fn find_item_fast(&self, pickup_name: &str) -> Option<usize> {
        self.item_by_pickup_name.get(&pickup_name.to_lowercase()).copied()
    }
}

// ============================================================
// Helper: ITEM_INDEX — in C this is pointer arithmetic (item - itemlist).
// In Rust, items are stored in a Vec and referenced by usize index.
// ============================================================

/// Given an item index (Option<usize>), return the index value or 0.
pub fn item_index(item: Option<usize>) -> usize {
    item.unwrap_or(0)
}

// ============================================================
// GetItemByIndex
// ============================================================

pub fn get_item_by_index(ctx: &GameContext, index: usize) -> Option<usize> {
    if index == 0 || index >= ctx.game.num_items as usize {
        return None;
    }
    Some(index)
}

// ============================================================
// FindItemByClassname
// ============================================================

/// O(1) lookup using global HashMap index. Works from any module.
pub fn find_item_by_classname(classname: &str) -> Option<usize> {
    ITEM_BY_CLASSNAME_INDEX.lock().unwrap().get(&classname.to_lowercase()).copied()
}

// ============================================================
// FindItem
// ============================================================

/// O(1) lookup using global HashMap index. Works from any module.
pub fn find_item(pickup_name: &str) -> Option<usize> {
    ITEM_BY_PICKUP_NAME_INDEX.lock().unwrap().get(&pickup_name.to_lowercase()).copied()
}

// ============================================================
// DoRespawn
// ============================================================

pub fn do_respawn(ctx: &mut GameContext, ent_idx: usize) {
    let ent = &ctx.edicts[ent_idx];

    let mut final_ent_idx = ent_idx;

    if !ent.team.is_empty() {
        let master_idx = ent.teammaster as usize;

        // Count entities in the chain
        let mut count = 0;
        let mut walk = master_idx as i32;
        while walk >= 0 {
            count += 1;
            walk = ctx.edicts[walk as usize].chain;
        }

        let choice = rand_int() % count;

        let mut walk = master_idx as i32;
        let mut c = 0;
        while c < choice {
            walk = ctx.edicts[walk as usize].chain;
            c += 1;
        }
        final_ent_idx = walk as usize;
    }

    let ent = &mut ctx.edicts[final_ent_idx];
    ent.svflags &= !SVF_NOCLIENT;
    ent.solid = Solid::Trigger;
    gi_linkentity(final_ent_idx as i32);

    // Send an effect
    ent.s.event = EV_ITEM_RESPAWN;
}

// ============================================================
// SetRespawn
// ============================================================

pub fn set_respawn(ctx: &mut GameContext, ent_idx: usize, delay: f32) {
    let time = ctx.level.time;
    let ent = &mut ctx.edicts[ent_idx];
    ent.flags |= FL_RESPAWN;
    ent.svflags |= SVF_NOCLIENT;
    ent.solid = Solid::Not;
    ent.nextthink = time + delay;
    ent.think_fn = Some(THINK_DO_RESPAWN);
    gi_linkentity(ent_idx as i32);
}

// ============================================================
// Pickup_Powerup
// ============================================================

pub fn pickup_powerup(ctx: &mut GameContext, ent_idx: usize, other_idx: usize) -> bool {
    let ent_item = ctx.edicts[ent_idx].item;
    let item_idx = item_index(ent_item);

    let client_idx = ctx.edicts[other_idx].client.expect("other must have client");
    let quantity = ctx.clients[client_idx].pers.inventory[item_idx];

    if (ctx.skill == 1.0 && quantity >= 2) || (ctx.skill >= 2.0 && quantity >= 1) {
        return false;
    }

    let item_flags = ctx.items[item_idx].flags;
    if ctx.coop != 0.0 && item_flags.intersects(IT_STAY_COOP) && quantity > 0 {
        return false;
    }

    ctx.clients[client_idx].pers.inventory[item_idx] += 1;

    if ctx.deathmatch != 0.0 {
        let ent_spawnflags = ctx.edicts[ent_idx].spawnflags;
        let item_quantity = ctx.items[item_idx].quantity;
        if (ent_spawnflags & DROPPED_ITEM) == 0 {
            set_respawn(ctx, ent_idx, item_quantity as f32);
        }

        let item_use = ctx.items[item_idx].use_fn;
        let ent_spawnflags = ctx.edicts[ent_idx].spawnflags;

        if DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_INSTANT_ITEMS)
            || (item_use == Some(USE_QUAD) && (ent_spawnflags & DROPPED_PLAYER_ITEM) != 0)
        {
            if item_use == Some(USE_QUAD) && (ent_spawnflags & DROPPED_PLAYER_ITEM) != 0 {
                let ent_nextthink = ctx.edicts[ent_idx].nextthink;
                let time = ctx.level.time;
                ctx.items_state.quad_drop_timeout_hack =
                    ((ent_nextthink - time) / FRAMETIME) as i32;
            }
            // ent->item->use(other, ent->item)
            dispatch_item_use(ctx, item_use, other_idx, item_idx);
        }
    }

    true
}

// ============================================================
// Drop_General
// ============================================================

pub fn drop_general(ctx: &mut GameContext, ent_idx: usize, item_idx: usize) {
    drop_item(ctx, ent_idx, item_idx);
    let client_idx = ctx.edicts[ent_idx].client.expect("must have client");
    ctx.clients[client_idx].pers.inventory[item_idx] -= 1;
    validate_selected_item(ctx, ent_idx);
}

// ============================================================
// Pickup_Adrenaline
// ============================================================

pub fn pickup_adrenaline(ctx: &mut GameContext, ent_idx: usize, other_idx: usize) -> bool {
    if ctx.deathmatch == 0.0 {
        ctx.edicts[other_idx].max_health += 1;
    }

    let max_health = ctx.edicts[other_idx].max_health;
    if ctx.edicts[other_idx].health < max_health {
        ctx.edicts[other_idx].health = max_health;
    }

    let ent_spawnflags = ctx.edicts[ent_idx].spawnflags;
    if (ent_spawnflags & DROPPED_ITEM) == 0 && ctx.deathmatch != 0.0 {
        let item_idx = item_index(ctx.edicts[ent_idx].item);
        let qty = ctx.items[item_idx].quantity;
        set_respawn(ctx, ent_idx, qty as f32);
    }

    true
}

// ============================================================
// Pickup_AncientHead
// ============================================================

pub fn pickup_ancient_head(ctx: &mut GameContext, ent_idx: usize, other_idx: usize) -> bool {
    ctx.edicts[other_idx].max_health += 2;

    let ent_spawnflags = ctx.edicts[ent_idx].spawnflags;
    if (ent_spawnflags & DROPPED_ITEM) == 0 && ctx.deathmatch != 0.0 {
        let item_idx = item_index(ctx.edicts[ent_idx].item);
        let qty = ctx.items[item_idx].quantity;
        set_respawn(ctx, ent_idx, qty as f32);
    }

    true
}

// ============================================================
// Pickup_Bandolier
// ============================================================

pub fn pickup_bandolier(ctx: &mut GameContext, ent_idx: usize, other_idx: usize) -> bool {
    let client_idx = ctx.edicts[other_idx].client.expect("must have client");

    if ctx.clients[client_idx].pers.max_bullets < 250 {
        ctx.clients[client_idx].pers.max_bullets = 250;
    }
    if ctx.clients[client_idx].pers.max_shells < 150 {
        ctx.clients[client_idx].pers.max_shells = 150;
    }
    if ctx.clients[client_idx].pers.max_cells < 250 {
        ctx.clients[client_idx].pers.max_cells = 250;
    }
    if ctx.clients[client_idx].pers.max_slugs < 75 {
        ctx.clients[client_idx].pers.max_slugs = 75;
    }

    if let Some(bullets_idx) = find_item("Bullets") {
        let qty = ctx.items[bullets_idx].quantity;
        ctx.clients[client_idx].pers.inventory[bullets_idx] += qty;
        let max = ctx.clients[client_idx].pers.max_bullets;
        if ctx.clients[client_idx].pers.inventory[bullets_idx] > max {
            ctx.clients[client_idx].pers.inventory[bullets_idx] = max;
        }
    }

    if let Some(shells_idx) = find_item("Shells") {
        let qty = ctx.items[shells_idx].quantity;
        ctx.clients[client_idx].pers.inventory[shells_idx] += qty;
        let max = ctx.clients[client_idx].pers.max_shells;
        if ctx.clients[client_idx].pers.inventory[shells_idx] > max {
            ctx.clients[client_idx].pers.inventory[shells_idx] = max;
        }
    }

    let ent_spawnflags = ctx.edicts[ent_idx].spawnflags;
    if (ent_spawnflags & DROPPED_ITEM) == 0 && ctx.deathmatch != 0.0 {
        let item_idx = item_index(ctx.edicts[ent_idx].item);
        let qty = ctx.items[item_idx].quantity;
        set_respawn(ctx, ent_idx, qty as f32);
    }

    true
}

// ============================================================
// Pickup_Pack
// ============================================================

pub fn pickup_pack(ctx: &mut GameContext, ent_idx: usize, other_idx: usize) -> bool {
    let client_idx = ctx.edicts[other_idx].client.expect("must have client");

    if ctx.clients[client_idx].pers.max_bullets < 300 {
        ctx.clients[client_idx].pers.max_bullets = 300;
    }
    if ctx.clients[client_idx].pers.max_shells < 200 {
        ctx.clients[client_idx].pers.max_shells = 200;
    }
    if ctx.clients[client_idx].pers.max_rockets < 100 {
        ctx.clients[client_idx].pers.max_rockets = 100;
    }
    if ctx.clients[client_idx].pers.max_grenades < 100 {
        ctx.clients[client_idx].pers.max_grenades = 100;
    }
    if ctx.clients[client_idx].pers.max_cells < 300 {
        ctx.clients[client_idx].pers.max_cells = 300;
    }
    if ctx.clients[client_idx].pers.max_slugs < 100 {
        ctx.clients[client_idx].pers.max_slugs = 100;
    }

    // Add each ammo type
    let ammo_getters: &[(&str, fn(&ClientPersistant) -> i32)] = &[
        ("Bullets", |c: &ClientPersistant| c.max_bullets),
        ("Shells", |c: &ClientPersistant| c.max_shells),
        ("Cells", |c: &ClientPersistant| c.max_cells),
        ("Grenades", |c: &ClientPersistant| c.max_grenades),
        ("Rockets", |c: &ClientPersistant| c.max_rockets),
        ("Slugs", |c: &ClientPersistant| c.max_slugs),
    ];
    for (name, max_fn) in ammo_getters {
        if let Some(ammo_idx) = find_item(name) {
            let qty = ctx.items[ammo_idx].quantity;
            ctx.clients[client_idx].pers.inventory[ammo_idx] += qty;
            let max = max_fn(&ctx.clients[client_idx].pers);
            if ctx.clients[client_idx].pers.inventory[ammo_idx] > max {
                ctx.clients[client_idx].pers.inventory[ammo_idx] = max;
            }
        }
    }

    let ent_spawnflags = ctx.edicts[ent_idx].spawnflags;
    if (ent_spawnflags & DROPPED_ITEM) == 0 && ctx.deathmatch != 0.0 {
        let item_idx = item_index(ctx.edicts[ent_idx].item);
        let qty = ctx.items[item_idx].quantity;
        set_respawn(ctx, ent_idx, qty as f32);
    }

    true
}

// ============================================================
// Use_Quad
// ============================================================

pub fn use_quad(ctx: &mut GameContext, ent_idx: usize, item_idx: usize) {
    let client_idx = ctx.edicts[ent_idx].client.expect("must have client");
    ctx.clients[client_idx].pers.inventory[item_idx] -= 1;
    validate_selected_item(ctx, ent_idx);

    let timeout;
    if ctx.items_state.quad_drop_timeout_hack != 0 {
        timeout = ctx.items_state.quad_drop_timeout_hack;
        ctx.items_state.quad_drop_timeout_hack = 0;
    } else {
        timeout = 300;
    }

    let framenum = ctx.level.framenum as f32;
    if ctx.clients[client_idx].quad_framenum > framenum {
        ctx.clients[client_idx].quad_framenum += timeout as f32;
    } else {
        ctx.clients[client_idx].quad_framenum = framenum + timeout as f32;
    }

    gi_sound(ent_idx as i32, CHAN_ITEM, gi_soundindex("items/damage.wav"), 1.0, ATTN_NORM, 0.0);
}

// ============================================================
// Use_Breather
// ============================================================

pub fn use_breather(ctx: &mut GameContext, ent_idx: usize, item_idx: usize) {
    let client_idx = ctx.edicts[ent_idx].client.expect("must have client");
    ctx.clients[client_idx].pers.inventory[item_idx] -= 1;
    validate_selected_item(ctx, ent_idx);

    let framenum = ctx.level.framenum as f32;
    if ctx.clients[client_idx].breather_framenum > framenum {
        ctx.clients[client_idx].breather_framenum += 300.0;
    } else {
        ctx.clients[client_idx].breather_framenum = framenum + 300.0;
    }
}

// ============================================================
// Use_Envirosuit
// ============================================================

pub fn use_envirosuit(ctx: &mut GameContext, ent_idx: usize, item_idx: usize) {
    let client_idx = ctx.edicts[ent_idx].client.expect("must have client");
    ctx.clients[client_idx].pers.inventory[item_idx] -= 1;
    validate_selected_item(ctx, ent_idx);

    let framenum = ctx.level.framenum as f32;
    if ctx.clients[client_idx].enviro_framenum > framenum {
        ctx.clients[client_idx].enviro_framenum += 300.0;
    } else {
        ctx.clients[client_idx].enviro_framenum = framenum + 300.0;
    }
}

// ============================================================
// Use_Invulnerability
// ============================================================

pub fn use_invulnerability(ctx: &mut GameContext, ent_idx: usize, item_idx: usize) {
    let client_idx = ctx.edicts[ent_idx].client.expect("must have client");
    ctx.clients[client_idx].pers.inventory[item_idx] -= 1;
    validate_selected_item(ctx, ent_idx);

    let framenum = ctx.level.framenum as f32;
    if ctx.clients[client_idx].invincible_framenum > framenum {
        ctx.clients[client_idx].invincible_framenum += 300.0;
    } else {
        ctx.clients[client_idx].invincible_framenum = framenum + 300.0;
    }

    gi_sound(ent_idx as i32, CHAN_ITEM, gi_soundindex("items/protect.wav"), 1.0, ATTN_NORM, 0.0);
}

// ============================================================
// Use_Silencer
// ============================================================

pub fn use_silencer(ctx: &mut GameContext, ent_idx: usize, item_idx: usize) {
    let client_idx = ctx.edicts[ent_idx].client.expect("must have client");
    ctx.clients[client_idx].pers.inventory[item_idx] -= 1;
    validate_selected_item(ctx, ent_idx);
    ctx.clients[client_idx].silencer_shots += 30;
}

// ============================================================
// Pickup_Key
// ============================================================

pub fn pickup_key(ctx: &mut GameContext, ent_idx: usize, other_idx: usize) -> bool {
    let ent_item = item_index(ctx.edicts[ent_idx].item);
    let client_idx = ctx.edicts[other_idx].client.expect("must have client");

    if ctx.coop != 0.0 {
        let classname = ctx.edicts[ent_idx].classname.clone();
        if classname == "key_power_cube" {
            let spawnflags = ctx.edicts[ent_idx].spawnflags;
            if (ctx.clients[client_idx].pers.power_cubes & ((spawnflags & 0x0000ff00) >> 8)) != 0 {
                return false;
            }
            ctx.clients[client_idx].pers.inventory[ent_item] += 1;
            ctx.clients[client_idx].pers.power_cubes |= (spawnflags & 0x0000ff00) >> 8;
        } else {
            if ctx.clients[client_idx].pers.inventory[ent_item] != 0 {
                return false;
            }
            ctx.clients[client_idx].pers.inventory[ent_item] = 1;
        }
        return true;
    }
    ctx.clients[client_idx].pers.inventory[ent_item] += 1;
    true
}

// ============================================================
// Add_Ammo
// ============================================================

pub fn add_ammo(ctx: &mut GameContext, ent_idx: usize, item_idx: usize, count: i32) -> bool {
    if ctx.edicts[ent_idx].client.is_none() {
        return false;
    }
    let client_idx = ctx.edicts[ent_idx].client.unwrap();

    let tag = ctx.items[item_idx].tag;
    let max = if tag == AMMO_BULLETS {
        ctx.clients[client_idx].pers.max_bullets
    } else if tag == AMMO_SHELLS {
        ctx.clients[client_idx].pers.max_shells
    } else if tag == AMMO_ROCKETS {
        ctx.clients[client_idx].pers.max_rockets
    } else if tag == AMMO_GRENADES {
        ctx.clients[client_idx].pers.max_grenades
    } else if tag == AMMO_CELLS {
        ctx.clients[client_idx].pers.max_cells
    } else if tag == AMMO_SLUGS {
        ctx.clients[client_idx].pers.max_slugs
    } else {
        return false;
    };

    if ctx.clients[client_idx].pers.inventory[item_idx] == max {
        return false;
    }

    ctx.clients[client_idx].pers.inventory[item_idx] += count;

    if ctx.clients[client_idx].pers.inventory[item_idx] > max {
        ctx.clients[client_idx].pers.inventory[item_idx] = max;
    }

    true
}

// ============================================================
// Pickup_Ammo
// ============================================================

pub fn pickup_ammo(ctx: &mut GameContext, ent_idx: usize, other_idx: usize) -> bool {
    let ent_item = item_index(ctx.edicts[ent_idx].item);
    let item_flags = ctx.items[ent_item].flags;
    let weapon = item_flags.intersects(IT_WEAPON);

    let count;
    if weapon && DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_INFINITE_AMMO) {
        count = 1000;
    } else if ctx.edicts[ent_idx].count != 0 {
        count = ctx.edicts[ent_idx].count;
    } else {
        count = ctx.items[ent_item].quantity;
    }

    let client_idx = ctx.edicts[other_idx].client.expect("must have client");
    let oldcount = ctx.clients[client_idx].pers.inventory[ent_item];

    if !add_ammo(ctx, other_idx, ent_item, count) {
        return false;
    }

    if weapon && oldcount == 0 {
        let client_idx = ctx.edicts[other_idx].client.unwrap();
        let pers_weapon = ctx.clients[client_idx].pers.weapon;
        let blaster_idx = find_item("blaster");
        if pers_weapon != ctx.edicts[ent_idx].item
            && (ctx.deathmatch == 0.0 || pers_weapon == blaster_idx)
        {
            ctx.clients[client_idx].newweapon = ctx.edicts[ent_idx].item;
        }
    }

    let ent_spawnflags = ctx.edicts[ent_idx].spawnflags;
    if (ent_spawnflags & (DROPPED_ITEM | DROPPED_PLAYER_ITEM)) == 0
        && ctx.deathmatch != 0.0
    {
        set_respawn(ctx, ent_idx, 30.0);
    }
    true
}

// ============================================================
// Drop_Ammo
// ============================================================

pub fn drop_ammo(ctx: &mut GameContext, ent_idx: usize, item_idx: usize) {
    let client_idx = ctx.edicts[ent_idx].client.expect("must have client");
    let dropped_idx = drop_item(ctx, ent_idx, item_idx);

    let inv_count = ctx.clients[client_idx].pers.inventory[item_idx];
    let qty = ctx.items[item_idx].quantity;

    if inv_count >= qty {
        ctx.edicts[dropped_idx].count = qty;
    } else {
        ctx.edicts[dropped_idx].count = inv_count;
    }

    let dropped_count = ctx.edicts[dropped_idx].count;

    // Check if dropping current grenade weapon with nothing left
    if let Some(weapon_idx) = ctx.clients[client_idx].pers.weapon {
        if ctx.items[weapon_idx].tag == AMMO_GRENADES
            && ctx.items[item_idx].tag == AMMO_GRENADES
            && inv_count - dropped_count <= 0
        {
            gi_cprintf(ent_idx as i32, PRINT_HIGH, "Can't drop current weapon\n");
            crate::g_utils::g_free_edict(ctx, dropped_idx);
            return;
        }
    }

    ctx.clients[client_idx].pers.inventory[item_idx] -= dropped_count;
    validate_selected_item(ctx, ent_idx);
}

// ============================================================
// MegaHealth_think
// ============================================================

pub fn megahealth_think(ctx: &mut GameContext, self_idx: usize) {
    let owner_idx = ctx.edicts[self_idx].owner as usize;
    let owner_health = ctx.edicts[owner_idx].health;
    let owner_max_health = ctx.edicts[owner_idx].max_health;

    if owner_health > owner_max_health {
        ctx.edicts[self_idx].nextthink = ctx.level.time + 1.0;
        ctx.edicts[owner_idx].health -= 1;
        return;
    }

    let spawnflags = ctx.edicts[self_idx].spawnflags;
    if (spawnflags & DROPPED_ITEM) == 0 && ctx.deathmatch != 0.0 {
        set_respawn(ctx, self_idx, 20.0);
    } else {
        crate::g_utils::g_free_edict(ctx, self_idx);
    }
}

// ============================================================
// Pickup_Health
// ============================================================

pub fn pickup_health(ctx: &mut GameContext, ent_idx: usize, other_idx: usize) -> bool {
    let style = ctx.edicts[ent_idx].style;

    if (style & HEALTH_IGNORE_MAX) == 0
        && ctx.edicts[other_idx].health >= ctx.edicts[other_idx].max_health {
            return false;
        }

    let ent_count = ctx.edicts[ent_idx].count;
    ctx.edicts[other_idx].health += ent_count;

    if (style & HEALTH_IGNORE_MAX) == 0 {
        let max_health = ctx.edicts[other_idx].max_health;
        if ctx.edicts[other_idx].health > max_health {
            ctx.edicts[other_idx].health = max_health;
        }
    }

    if (style & HEALTH_TIMED) != 0 {
        ctx.edicts[ent_idx].think_fn = Some(THINK_MEGAHEALTH);
        ctx.edicts[ent_idx].nextthink = ctx.level.time + 5.0;
        ctx.edicts[ent_idx].owner = other_idx as i32;
        ctx.edicts[ent_idx].flags |= FL_RESPAWN;
        ctx.edicts[ent_idx].svflags |= SVF_NOCLIENT;
        ctx.edicts[ent_idx].solid = Solid::Not;
    } else {
        let spawnflags = ctx.edicts[ent_idx].spawnflags;
        if (spawnflags & DROPPED_ITEM) == 0 && ctx.deathmatch != 0.0 {
            set_respawn(ctx, ent_idx, 30.0);
        }
    }

    true
}

// ============================================================
// ArmorIndex
// ============================================================

pub fn armor_index(ctx: &GameContext, ent_idx: usize) -> usize {
    if ctx.edicts[ent_idx].client.is_none() {
        return 0;
    }
    let client_idx = ctx.edicts[ent_idx].client.unwrap();

    let jai = ctx.items_state.jacket_armor_index;
    let cai = ctx.items_state.combat_armor_index;
    let bai = ctx.items_state.body_armor_index;

    if ctx.clients[client_idx].pers.inventory[jai] > 0 {
        return jai;
    }
    if ctx.clients[client_idx].pers.inventory[cai] > 0 {
        return cai;
    }
    if ctx.clients[client_idx].pers.inventory[bai] > 0 {
        return bai;
    }

    0
}

// ============================================================
// Pickup_Armor
// ============================================================

pub fn pickup_armor(ctx: &mut GameContext, ent_idx: usize, other_idx: usize) -> bool {
    let ent_item = item_index(ctx.edicts[ent_idx].item);
    let newinfo = ctx.items[ent_item]
        .armor_info
        .clone()
        .unwrap_or_default();

    let old_armor_index = armor_index(ctx, other_idx);
    let client_idx = ctx.edicts[other_idx].client.expect("must have client");
    let item_tag = ctx.items[ent_item].tag;
    let jai = ctx.items_state.jacket_armor_index;
    let cai = ctx.items_state.combat_armor_index;
    let _bai = ctx.items_state.body_armor_index;

    if item_tag == ARMOR_SHARD {
        // Handle armor shards specially
        if old_armor_index == 0 {
            ctx.clients[client_idx].pers.inventory[jai] = 2;
        } else {
            ctx.clients[client_idx].pers.inventory[old_armor_index] += 2;
        }
    } else if old_armor_index == 0 {
        // No armor, just use it
        ctx.clients[client_idx].pers.inventory[ent_item] = newinfo.base_count;
    } else {
        // Use the better armor
        let oldinfo = if old_armor_index == jai {
            jacketarmor_info()
        } else if old_armor_index == cai {
            combatarmor_info()
        } else {
            bodyarmor_info()
        };

        if newinfo.normal_protection > oldinfo.normal_protection {
            let salvage = oldinfo.normal_protection / newinfo.normal_protection;
            let salvagecount =
                (salvage * ctx.clients[client_idx].pers.inventory[old_armor_index] as f32) as i32;
            let mut newcount = newinfo.base_count + salvagecount;
            if newcount > newinfo.max_count {
                newcount = newinfo.max_count;
            }
            ctx.clients[client_idx].pers.inventory[old_armor_index] = 0;
            ctx.clients[client_idx].pers.inventory[ent_item] = newcount;
        } else {
            let salvage = newinfo.normal_protection / oldinfo.normal_protection;
            let salvagecount = (salvage * newinfo.base_count as f32) as i32;
            let mut newcount =
                ctx.clients[client_idx].pers.inventory[old_armor_index] + salvagecount;
            if newcount > oldinfo.max_count {
                newcount = oldinfo.max_count;
            }
            if ctx.clients[client_idx].pers.inventory[old_armor_index] >= newcount {
                return false;
            }
            ctx.clients[client_idx].pers.inventory[old_armor_index] = newcount;
        }
    }

    let spawnflags = ctx.edicts[ent_idx].spawnflags;
    if (spawnflags & DROPPED_ITEM) == 0 && ctx.deathmatch != 0.0 {
        set_respawn(ctx, ent_idx, 20.0);
    }

    true
}

// ============================================================
// PowerArmorType
// ============================================================

pub fn power_armor_type(ctx: &GameContext, ent_idx: usize) -> i32 {
    if ctx.edicts[ent_idx].client.is_none() {
        return POWER_ARMOR_NONE;
    }
    let client_idx = ctx.edicts[ent_idx].client.unwrap();

    if !ctx.edicts[ent_idx].flags.intersects(FL_POWER_ARMOR) {
        return POWER_ARMOR_NONE;
    }

    let psi = ctx.items_state.power_shield_index;
    if ctx.clients[client_idx].pers.inventory[psi] > 0 {
        return POWER_ARMOR_SHIELD;
    }

    let pci = ctx.items_state.power_screen_index;
    if ctx.clients[client_idx].pers.inventory[pci] > 0 {
        return POWER_ARMOR_SCREEN;
    }

    POWER_ARMOR_NONE
}

// ============================================================
// Use_PowerArmor
// ============================================================

pub fn use_power_armor(ctx: &mut GameContext, ent_idx: usize, _item_idx: usize) {
    let client_idx = ctx.edicts[ent_idx].client.expect("must have client");

    if ctx.edicts[ent_idx].flags.intersects(FL_POWER_ARMOR) {
        ctx.edicts[ent_idx].flags.remove(FL_POWER_ARMOR);
        gi_sound(ent_idx as i32, CHAN_AUTO, gi_soundindex("misc/power2.wav"), 1.0, ATTN_NORM, 0.0);
    } else {
        if let Some(cells_idx) = find_item("cells") {
            if ctx.clients[client_idx].pers.inventory[cells_idx] == 0 {
                gi_cprintf(ent_idx as i32, PRINT_HIGH, "No cells for power armor.\n");
                return;
            }
        }
        ctx.edicts[ent_idx].flags |= FL_POWER_ARMOR;
        gi_sound(ent_idx as i32, CHAN_AUTO, gi_soundindex("misc/power1.wav"), 1.0, ATTN_NORM, 0.0);
    }
}

// ============================================================
// Pickup_PowerArmor
// ============================================================

pub fn pickup_power_armor(ctx: &mut GameContext, ent_idx: usize, other_idx: usize) -> bool {
    let ent_item = item_index(ctx.edicts[ent_idx].item);
    let client_idx = ctx.edicts[other_idx].client.expect("must have client");
    let quantity = ctx.clients[client_idx].pers.inventory[ent_item];

    ctx.clients[client_idx].pers.inventory[ent_item] += 1;

    if ctx.deathmatch != 0.0 {
        let spawnflags = ctx.edicts[ent_idx].spawnflags;
        if (spawnflags & DROPPED_ITEM) == 0 {
            let qty = ctx.items[ent_item].quantity;
            set_respawn(ctx, ent_idx, qty as f32);
        }
        // Auto-use for DM only if we didn't already have one
        if quantity == 0 {
            let use_fn = ctx.items[ent_item].use_fn;
            dispatch_item_use(ctx, use_fn, other_idx, ent_item);
        }
    }

    true
}

// ============================================================
// Drop_PowerArmor
// ============================================================

pub fn drop_power_armor(ctx: &mut GameContext, ent_idx: usize, item_idx: usize) {
    let client_idx = ctx.edicts[ent_idx].client.expect("must have client");
    if ctx.edicts[ent_idx].flags.intersects(FL_POWER_ARMOR)
        && ctx.clients[client_idx].pers.inventory[item_idx] == 1
    {
        use_power_armor(ctx, ent_idx, item_idx);
    }
    drop_general(ctx, ent_idx, item_idx);
}

// ============================================================
// Touch_Item
// ============================================================

pub fn touch_item(ctx: &mut GameContext, ent_idx: usize, other_idx: usize) {
    if ctx.edicts[other_idx].client.is_none() {
        return;
    }
    if ctx.edicts[other_idx].health < 1 {
        return; // dead people can't pickup
    }
    let ent_item = ctx.edicts[ent_idx].item;
    if ent_item.is_none() {
        return;
    }
    let item_idx = ent_item.unwrap();
    let pickup_fn = ctx.items[item_idx].pickup_fn;
    if pickup_fn.is_none() {
        return; // not a grabbable item
    }

    let taken = dispatch_pickup(ctx, pickup_fn, ent_idx, other_idx);

    if taken {
        let client_idx = ctx.edicts[other_idx].client.unwrap();
        // Flash the screen
        ctx.clients[client_idx].bonus_alpha = 0.25;

        // Show icon and name on status bar
        let icon = ctx.items[item_idx].icon.clone();
        gi_imageindex(&icon);
        // In a real implementation, store the result in ps.stats[STAT_PICKUP_ICON]

        let ent_count = ctx.edicts[ent_idx].count;
        let item_pickup_sound = ctx.items[item_idx].pickup_sound.clone();

        // Change selected item
        let use_fn = ctx.items[item_idx].use_fn;
        if use_fn.is_some() {
            ctx.clients[client_idx].pers.selected_item = item_idx as i32;
        }

        // Play sound based on pickup type
        if pickup_fn == Some(PICKUP_HEALTH) {
            let sound = match ent_count {
                2 => "items/s_health.wav",
                10 => "items/n_health.wav",
                25 => "items/l_health.wav",
                _ => "items/m_health.wav",
            };
            gi_sound(other_idx as i32, CHAN_ITEM, gi_soundindex(sound), 1.0, ATTN_NORM, 0.0);
        } else if !item_pickup_sound.is_empty() {
            gi_sound(other_idx as i32, CHAN_ITEM, gi_soundindex(&item_pickup_sound), 1.0, ATTN_NORM, 0.0);
        }
    }

    let spawnflags = ctx.edicts[ent_idx].spawnflags;
    if (spawnflags & ITEM_TARGETS_USED) == 0 {
        crate::g_utils::g_use_targets(ctx, ent_idx, other_idx);
        ctx.edicts[ent_idx].spawnflags |= ITEM_TARGETS_USED;
    }

    if !taken {
        return;
    }

    let item_idx = item_index(ctx.edicts[ent_idx].item);
    let item_flags = ctx.items[item_idx].flags;
    let spawnflags = ctx.edicts[ent_idx].spawnflags;

    if !((ctx.coop != 0.0) && item_flags.intersects(IT_STAY_COOP))
        || (spawnflags & (DROPPED_ITEM | DROPPED_PLAYER_ITEM)) != 0
    {
        if ctx.edicts[ent_idx].flags.intersects(FL_RESPAWN) {
            ctx.edicts[ent_idx].flags.remove(FL_RESPAWN);
        } else {
            crate::g_utils::g_free_edict(ctx, ent_idx);
        }
    }
}

// ============================================================
// drop_temp_touch
// ============================================================

pub fn drop_temp_touch(ctx: &mut GameContext, ent_idx: usize, other_idx: usize) {
    if other_idx == ctx.edicts[ent_idx].owner as usize {
        return;
    }
    touch_item(ctx, ent_idx, other_idx);
}

// ============================================================
// drop_make_touchable
// ============================================================

pub fn drop_make_touchable(ctx: &mut GameContext, ent_idx: usize) {
    ctx.edicts[ent_idx].touch_fn = Some(TOUCH_ITEM);
    if ctx.deathmatch != 0.0 {
        ctx.edicts[ent_idx].nextthink = ctx.level.time + 29.0;
        // ent->think = G_FreeEdict — will be handled by dispatch
        ctx.edicts[ent_idx].think_fn = Some(crate::dispatch::THINK_FREE_EDICT);
    }
}

// ============================================================
// Drop_Item
// ============================================================

pub fn drop_item(ctx: &mut GameContext, ent_idx: usize, item_idx: usize) -> usize {
    let dropped_idx = g_spawn(ctx);

    ctx.edicts[dropped_idx].classname = ctx.items[item_idx].classname.clone();
    ctx.edicts[dropped_idx].item = Some(item_idx);
    ctx.edicts[dropped_idx].spawnflags = DROPPED_ITEM;
    ctx.edicts[dropped_idx].s.effects = ctx.items[item_idx].world_model_flags;
    ctx.edicts[dropped_idx].s.renderfx = RF_GLOW;
    ctx.edicts[dropped_idx].mins = [-15.0, -15.0, -15.0];
    ctx.edicts[dropped_idx].maxs = [15.0, 15.0, 15.0];
    let world_model = ctx.items[item_idx].world_model.clone();
    gi_setmodel(dropped_idx as i32, &world_model);
    ctx.edicts[dropped_idx].solid = Solid::Trigger;
    ctx.edicts[dropped_idx].movetype = MoveType::Toss;
    ctx.edicts[dropped_idx].touch_fn = Some(TOUCH_DROP_TEMP);
    ctx.edicts[dropped_idx].owner = ent_idx as i32;

    if ctx.edicts[ent_idx].client.is_some() {
        let client_idx = ctx.edicts[ent_idx].client.unwrap();
        let v_angle = ctx.clients[client_idx].v_angle;
        let mut forward = [0.0f32; 3];
        let mut right = [0.0f32; 3];
        myq2_common::q_shared::angle_vectors(&v_angle, Some(&mut forward), Some(&mut right), None);
        // Project source from view offset
        let origin = ctx.edicts[ent_idx].s.origin;
        ctx.edicts[dropped_idx].s.origin = [
            origin[0] + forward[0] * 24.0,
            origin[1] + forward[1] * 24.0,
            origin[2] + forward[2] * 24.0,
        ];
    } else {
        let mut forward = [0.0f32; 3];
        myq2_common::q_shared::angle_vectors(&ctx.edicts[ent_idx].s.angles, Some(&mut forward), None, None);
        let origin = ctx.edicts[ent_idx].s.origin;
        ctx.edicts[dropped_idx].s.origin = [
            origin[0] + forward[0] * 24.0,
            origin[1] + forward[1] * 24.0,
            origin[2] + forward[2] * 24.0,
        ];
    }

    // VectorScale(forward, 100, dropped->velocity)
    // dropped->velocity[2] = 300
    ctx.edicts[dropped_idx].velocity = [0.0, 0.0, 300.0]; // forward*100 + up placeholder

    ctx.edicts[dropped_idx].think_fn = Some(THINK_DROP_MAKE_TOUCHABLE);
    ctx.edicts[dropped_idx].nextthink = ctx.level.time + 1.0;

    gi_linkentity(dropped_idx as i32);

    dropped_idx
}

// ============================================================
// Use_Item (entity trigger use callback)
// ============================================================

pub fn use_item_trigger(ctx: &mut GameContext, ent_idx: usize) {
    ctx.edicts[ent_idx].svflags &= !SVF_NOCLIENT;
    ctx.edicts[ent_idx].use_fn = None;

    let spawnflags = ctx.edicts[ent_idx].spawnflags;
    if (spawnflags & ITEM_NO_TOUCH) != 0 {
        ctx.edicts[ent_idx].solid = Solid::Bbox;
        ctx.edicts[ent_idx].touch_fn = None;
    } else {
        ctx.edicts[ent_idx].solid = Solid::Trigger;
        ctx.edicts[ent_idx].touch_fn = Some(TOUCH_ITEM);
    }

    gi_linkentity(ent_idx as i32);
}

// ============================================================
// droptofloor
// ============================================================

pub fn droptofloor(ctx: &mut GameContext, ent_idx: usize) {
    ctx.edicts[ent_idx].mins = [-15.0, -15.0, -15.0];
    ctx.edicts[ent_idx].maxs = [15.0, 15.0, 15.0];

    let model = ctx.edicts[ent_idx].model.clone();
    if !model.is_empty() {
        gi_setmodel(ent_idx as i32, &model);
    } else {
        let item_idx = item_index(ctx.edicts[ent_idx].item);
        let world_model = ctx.items[item_idx].world_model.clone();
        gi_setmodel(ent_idx as i32, &world_model);
    }
    ctx.edicts[ent_idx].solid = Solid::Trigger;
    ctx.edicts[ent_idx].movetype = MoveType::Toss;
    ctx.edicts[ent_idx].touch_fn = Some(TOUCH_ITEM);

    // Trace down 128 units
    let origin = ctx.edicts[ent_idx].s.origin;
    let _dest = [origin[0], origin[1], origin[2] - 128.0];
    let _tr = gi_trace(&origin, &ctx.edicts[ent_idx].mins, &ctx.edicts[ent_idx].maxs, &_dest, ent_idx as i32, MASK_SOLID);

    // Placeholder: check if startsolid
    let startsolid = false; // Would come from trace result
    if startsolid {
        let classname = ctx.edicts[ent_idx].classname.clone();
        gi_dprintf(&format!("droptofloor: {} startsolid at {:?}\n", classname, origin));
        crate::g_utils::g_free_edict(ctx, ent_idx);
        return;
    }

    // VectorCopy(tr.endpos, ent->s.origin) — placeholder, trace result would set this

    let team = ctx.edicts[ent_idx].team.clone();
    if !team.is_empty() {
        ctx.edicts[ent_idx].flags.remove(FL_TEAMSLAVE);
        let teamchain = ctx.edicts[ent_idx].teamchain;
        ctx.edicts[ent_idx].chain = teamchain;
        ctx.edicts[ent_idx].teamchain = -1;

        ctx.edicts[ent_idx].svflags |= SVF_NOCLIENT;
        ctx.edicts[ent_idx].solid = Solid::Not;

        let teammaster = ctx.edicts[ent_idx].teammaster;
        if ent_idx == teammaster as usize {
            ctx.edicts[ent_idx].nextthink = ctx.level.time + FRAMETIME;
            ctx.edicts[ent_idx].think_fn = Some(THINK_DO_RESPAWN);
        }
    }

    let spawnflags = ctx.edicts[ent_idx].spawnflags;
    if (spawnflags & ITEM_NO_TOUCH) != 0 {
        ctx.edicts[ent_idx].solid = Solid::Bbox;
        ctx.edicts[ent_idx].touch_fn = None;
        ctx.edicts[ent_idx].s.effects &= !EF_ROTATE;
        ctx.edicts[ent_idx].s.renderfx &= !RF_GLOW;
    }

    if (spawnflags & ITEM_TRIGGER_SPAWN) != 0 {
        ctx.edicts[ent_idx].svflags |= SVF_NOCLIENT;
        ctx.edicts[ent_idx].solid = Solid::Not;
        ctx.edicts[ent_idx].use_fn = Some(USE_ITEM_TRIGGER);
    }

    gi_linkentity(ent_idx as i32);
}

// ============================================================
// PrecacheItem
// ============================================================

pub fn precache_item(ctx: &GameContext, item_idx: usize) {
    if item_idx == 0 || item_idx >= ctx.items.len() {
        return;
    }

    let it = &ctx.items[item_idx];

    if !it.pickup_sound.is_empty() {
        gi_soundindex(&it.pickup_sound);
    }
    if !it.world_model.is_empty() {
        gi_modelindex(&it.world_model);
    }
    if !it.view_model.is_empty() {
        gi_modelindex(&it.view_model);
    }
    if !it.icon.is_empty() {
        gi_imageindex(&it.icon);
    }

    // Parse ammo
    if !it.ammo.is_empty() {
        if let Some(ammo_idx) = find_item(&it.ammo) {
            if ammo_idx != item_idx {
                precache_item(ctx, ammo_idx);
            }
        }
    }

    // Parse space-separated precache string
    if it.precaches.is_empty() {
        return;
    }

    for token in it.precaches.split_whitespace() {
        if token.len() >= MAX_QPATH || token.len() < 5 {
            panic!(
                "PrecacheItem: {} has bad precache string",
                it.classname
            );
        }
        if token.ends_with("md2") {
            gi_modelindex(token);
        } else if token.ends_with("sp2") {
            gi_modelindex(token);
        } else if token.ends_with("wav") {
            gi_soundindex(token);
        }
        if token.ends_with("pcx") {
            gi_imageindex(token);
        }
    }
}

// ============================================================
// SpawnItem
// ============================================================

pub fn spawn_item(ctx: &mut GameContext, ent_idx: usize, item_idx: usize) {
    precache_item(ctx, item_idx);

    let spawnflags = ctx.edicts[ent_idx].spawnflags;
    if spawnflags != 0 {
        let classname = ctx.edicts[ent_idx].classname.clone();
        if classname != "key_power_cube" {
            ctx.edicts[ent_idx].spawnflags = 0;
            let origin = ctx.edicts[ent_idx].s.origin;
            gi_dprintf(&format!("{} at {:?} has invalid spawnflags set\n", classname, origin));
        }
    }

    // Some items prevented in DM
    if ctx.deathmatch != 0.0 {
        let dmflags_val = DmFlags::from_bits_truncate(ctx.dmflags as i32);
        let pickup_fn = ctx.items[item_idx].pickup_fn;
        let item_flags = ctx.items[item_idx].flags;
        let classname = ctx.edicts[ent_idx].classname.clone();

        if dmflags_val.intersects(DF_NO_ARMOR)
            && (pickup_fn == Some(PICKUP_ARMOR) || pickup_fn == Some(PICKUP_POWERARMOR)) {
                crate::g_utils::g_free_edict(ctx, ent_idx);
                return;
            }
        if dmflags_val.intersects(DF_NO_ITEMS)
            && pickup_fn == Some(PICKUP_POWERUP) {
                crate::g_utils::g_free_edict(ctx, ent_idx);
                return;
            }
        if dmflags_val.intersects(DF_NO_HEALTH)
            && (pickup_fn == Some(PICKUP_HEALTH)
                || pickup_fn == Some(PICKUP_ADRENALINE)
                || pickup_fn == Some(PICKUP_ANCIENTHEAD))
            {
                crate::g_utils::g_free_edict(ctx, ent_idx);
                return;
            }
        if dmflags_val.intersects(DF_INFINITE_AMMO)
            && (item_flags == IT_AMMO || classname == "weapon_bfg") {
                crate::g_utils::g_free_edict(ctx, ent_idx);
                return;
            }
    }

    let classname = ctx.edicts[ent_idx].classname.clone();
    if ctx.coop != 0.0 && classname == "key_power_cube" {
        let pc = ctx.level.power_cubes;
        ctx.edicts[ent_idx].spawnflags |= 1 << (8 + pc);
        ctx.level.power_cubes += 1;
    }

    // Don't let them drop items that stay in a coop game
    if ctx.coop != 0.0 && ctx.items[item_idx].flags.intersects(IT_STAY_COOP) {
        ctx.items[item_idx].drop_fn = None;
    }

    ctx.edicts[ent_idx].item = Some(item_idx);
    ctx.edicts[ent_idx].nextthink = ctx.level.time + 2.0 * FRAMETIME;
    ctx.edicts[ent_idx].think_fn = Some(THINK_DROPTOFLOOR);
    ctx.edicts[ent_idx].s.effects = ctx.items[item_idx].world_model_flags;
    ctx.edicts[ent_idx].s.renderfx = RF_GLOW;

    let model = ctx.edicts[ent_idx].model.clone();
    if !model.is_empty() {
        gi_modelindex(&model);
    }
}

// ============================================================
// SP_item_health variants
// ============================================================

pub fn sp_item_health(ctx: &mut GameContext, self_idx: usize) {
    if ctx.deathmatch != 0.0 && DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_NO_HEALTH) {
        crate::g_utils::g_free_edict(ctx, self_idx);
        return;
    }

    ctx.edicts[self_idx].model = "models/items/healing/medium/tris.md2".to_string();
    ctx.edicts[self_idx].count = 10;
    if let Some(health_idx) = find_item("Health") {
        spawn_item(ctx, self_idx, health_idx);
    }
    gi_soundindex("items/n_health.wav");
}

pub fn sp_item_health_small(ctx: &mut GameContext, self_idx: usize) {
    if ctx.deathmatch != 0.0 && DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_NO_HEALTH) {
        crate::g_utils::g_free_edict(ctx, self_idx);
        return;
    }

    ctx.edicts[self_idx].model = "models/items/healing/stimpack/tris.md2".to_string();
    ctx.edicts[self_idx].count = 2;
    if let Some(health_idx) = find_item("Health") {
        spawn_item(ctx, self_idx, health_idx);
    }
    ctx.edicts[self_idx].style = HEALTH_IGNORE_MAX;
    gi_soundindex("items/s_health.wav");
}

pub fn sp_item_health_large(ctx: &mut GameContext, self_idx: usize) {
    if ctx.deathmatch != 0.0 && DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_NO_HEALTH) {
        crate::g_utils::g_free_edict(ctx, self_idx);
        return;
    }

    ctx.edicts[self_idx].model = "models/items/healing/large/tris.md2".to_string();
    ctx.edicts[self_idx].count = 25;
    if let Some(health_idx) = find_item("Health") {
        spawn_item(ctx, self_idx, health_idx);
    }
    gi_soundindex("items/l_health.wav");
}

pub fn sp_item_health_mega(ctx: &mut GameContext, self_idx: usize) {
    if ctx.deathmatch != 0.0 && DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_NO_HEALTH) {
        crate::g_utils::g_free_edict(ctx, self_idx);
        return;
    }

    ctx.edicts[self_idx].model = "models/items/mega_h/tris.md2".to_string();
    ctx.edicts[self_idx].count = 100;
    if let Some(health_idx) = find_item("Health") {
        spawn_item(ctx, self_idx, health_idx);
    }
    gi_soundindex("items/m_health.wav");
    ctx.edicts[self_idx].style = HEALTH_IGNORE_MAX | HEALTH_TIMED;
}

// ============================================================
// InitItems
// ============================================================

pub fn init_items(ctx: &mut GameContext) {
    let itemlist = build_itemlist();
    // num_items = len - 1 (last entry is the null terminator)
    ctx.game.num_items = (itemlist.len() - 1) as i32;
    ctx.items = itemlist;
}

// ============================================================
// SetItemNames
// ============================================================

pub fn set_item_names(ctx: &mut GameContext) {
    for i in 0..ctx.game.num_items as usize {
        let name = ctx.items[i].pickup_name.clone();
        gi_configstring(CS_ITEMS as i32 + i as i32, &name);
    }

    ctx.items_state.jacket_armor_index = find_item("Jacket Armor").unwrap_or(0);
    ctx.items_state.combat_armor_index = find_item("Combat Armor").unwrap_or(0);
    ctx.items_state.body_armor_index = find_item("Body Armor").unwrap_or(0);
    ctx.items_state.power_screen_index = find_item("Power Screen").unwrap_or(0);
    ctx.items_state.power_shield_index = find_item("Power Shield").unwrap_or(0);
}

// ============================================================
// Item table (itemlist[])
// ============================================================

/// Build the complete item list. This is the Rust equivalent of the C `itemlist[]` array.
/// Index 0 is a blank/null entry. The last entry is also blank (end-of-list marker).
pub fn build_itemlist() -> Vec<GItem> {
    vec![
        // 0: leave index 0 alone
        GItem::default(),

        // ============================================================
        // ARMOR
        // ============================================================

        // 1: item_armor_body
        GItem {
            classname: "item_armor_body".into(),
            pickup_fn: Some(PICKUP_ARMOR),
            use_fn: None,
            drop_fn: None,
            weaponthink_fn: None,
            pickup_sound: "misc/ar1_pkup.wav".into(),
            world_model: "models/items/armor/body/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: String::new(),
            icon: "i_bodyarmor".into(),
            pickup_name: "Body Armor".into(),
            count_width: 3,
            quantity: 0,
            ammo: String::new(),
            flags: IT_ARMOR,
            weapmodel: 0,
            armor_info: Some(bodyarmor_info()),
            tag: ARMOR_BODY,
            precaches: String::new(),
        },

        // 2: item_armor_combat
        GItem {
            classname: "item_armor_combat".into(),
            pickup_fn: Some(PICKUP_ARMOR),
            use_fn: None,
            drop_fn: None,
            weaponthink_fn: None,
            pickup_sound: "misc/ar1_pkup.wav".into(),
            world_model: "models/items/armor/combat/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: String::new(),
            icon: "i_combatarmor".into(),
            pickup_name: "Combat Armor".into(),
            count_width: 3,
            quantity: 0,
            ammo: String::new(),
            flags: IT_ARMOR,
            weapmodel: 0,
            armor_info: Some(combatarmor_info()),
            tag: ARMOR_COMBAT,
            precaches: String::new(),
        },

        // 3: item_armor_jacket
        GItem {
            classname: "item_armor_jacket".into(),
            pickup_fn: Some(PICKUP_ARMOR),
            use_fn: None,
            drop_fn: None,
            weaponthink_fn: None,
            pickup_sound: "misc/ar1_pkup.wav".into(),
            world_model: "models/items/armor/jacket/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: String::new(),
            icon: "i_jacketarmor".into(),
            pickup_name: "Jacket Armor".into(),
            count_width: 3,
            quantity: 0,
            ammo: String::new(),
            flags: IT_ARMOR,
            weapmodel: 0,
            armor_info: Some(jacketarmor_info()),
            tag: ARMOR_JACKET,
            precaches: String::new(),
        },

        // 4: item_armor_shard
        GItem {
            classname: "item_armor_shard".into(),
            pickup_fn: Some(PICKUP_ARMOR),
            use_fn: None,
            drop_fn: None,
            weaponthink_fn: None,
            pickup_sound: "misc/ar2_pkup.wav".into(),
            world_model: "models/items/armor/shard/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: String::new(),
            icon: "i_jacketarmor".into(),
            pickup_name: "Armor Shard".into(),
            count_width: 3,
            quantity: 0,
            ammo: String::new(),
            flags: IT_ARMOR,
            weapmodel: 0,
            armor_info: None,
            tag: ARMOR_SHARD,
            precaches: String::new(),
        },

        // 5: item_power_screen
        GItem {
            classname: "item_power_screen".into(),
            pickup_fn: Some(PICKUP_POWERARMOR),
            use_fn: Some(USE_POWERARMOR),
            drop_fn: Some(DROP_POWERARMOR),
            weaponthink_fn: None,
            pickup_sound: "misc/ar3_pkup.wav".into(),
            world_model: "models/items/armor/screen/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: String::new(),
            icon: "i_powerscreen".into(),
            pickup_name: "Power Screen".into(),
            count_width: 0,
            quantity: 60,
            ammo: String::new(),
            flags: IT_ARMOR,
            weapmodel: 0,
            armor_info: None,
            tag: 0,
            precaches: String::new(),
        },

        // 6: item_power_shield
        GItem {
            classname: "item_power_shield".into(),
            pickup_fn: Some(PICKUP_POWERARMOR),
            use_fn: Some(USE_POWERARMOR),
            drop_fn: Some(DROP_POWERARMOR),
            weaponthink_fn: None,
            pickup_sound: "misc/ar3_pkup.wav".into(),
            world_model: "models/items/armor/shield/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: String::new(),
            icon: "i_powershield".into(),
            pickup_name: "Power Shield".into(),
            count_width: 0,
            quantity: 60,
            ammo: String::new(),
            flags: IT_ARMOR,
            weapmodel: 0,
            armor_info: None,
            tag: 0,
            precaches: "misc/power2.wav misc/power1.wav".into(),
        },

        // ============================================================
        // WEAPONS
        // ============================================================

        // 7: weapon_blaster
        GItem {
            classname: "weapon_blaster".into(),
            pickup_fn: None,
            use_fn: Some(USE_WEAPON),
            drop_fn: None,
            weaponthink_fn: Some(WEAPTHINK_BLASTER),
            pickup_sound: "misc/w_pkup.wav".into(),
            world_model: String::new(),
            world_model_flags: 0,
            view_model: "models/weapons/v_blast/tris.md2".into(),
            icon: "w_blaster".into(),
            pickup_name: "Blaster".into(),
            count_width: 0,
            quantity: 0,
            ammo: String::new(),
            flags: IT_WEAPON | IT_STAY_COOP,
            weapmodel: WEAP_BLASTER,
            armor_info: None,
            tag: 0,
            precaches: "weapons/blastf1a.wav misc/lasfly.wav".into(),
        },

        // 8: weapon_shotgun
        GItem {
            classname: "weapon_shotgun".into(),
            pickup_fn: Some(PICKUP_WEAPON),
            use_fn: Some(USE_WEAPON),
            drop_fn: Some(DROP_WEAPON),
            weaponthink_fn: Some(WEAPTHINK_SHOTGUN),
            pickup_sound: "misc/w_pkup.wav".into(),
            world_model: "models/weapons/g_shotg/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: "models/weapons/v_shotg/tris.md2".into(),
            icon: "w_shotgun".into(),
            pickup_name: "Shotgun".into(),
            count_width: 0,
            quantity: 1,
            ammo: "Shells".into(),
            flags: IT_WEAPON | IT_STAY_COOP,
            weapmodel: WEAP_SHOTGUN,
            armor_info: None,
            tag: 0,
            precaches: "weapons/shotgf1b.wav weapons/shotgr1b.wav".into(),
        },

        // 9: weapon_supershotgun
        GItem {
            classname: "weapon_supershotgun".into(),
            pickup_fn: Some(PICKUP_WEAPON),
            use_fn: Some(USE_WEAPON),
            drop_fn: Some(DROP_WEAPON),
            weaponthink_fn: Some(WEAPTHINK_SUPERSHOTGUN),
            pickup_sound: "misc/w_pkup.wav".into(),
            world_model: "models/weapons/g_shotg2/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: "models/weapons/v_shotg2/tris.md2".into(),
            icon: "w_sshotgun".into(),
            pickup_name: "Super Shotgun".into(),
            count_width: 0,
            quantity: 2,
            ammo: "Shells".into(),
            flags: IT_WEAPON | IT_STAY_COOP,
            weapmodel: WEAP_SUPERSHOTGUN,
            armor_info: None,
            tag: 0,
            precaches: "weapons/sshotf1b.wav".into(),
        },

        // 10: weapon_machinegun
        GItem {
            classname: "weapon_machinegun".into(),
            pickup_fn: Some(PICKUP_WEAPON),
            use_fn: Some(USE_WEAPON),
            drop_fn: Some(DROP_WEAPON),
            weaponthink_fn: Some(WEAPTHINK_MACHINEGUN),
            pickup_sound: "misc/w_pkup.wav".into(),
            world_model: "models/weapons/g_machn/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: "models/weapons/v_machn/tris.md2".into(),
            icon: "w_machinegun".into(),
            pickup_name: "Machinegun".into(),
            count_width: 0,
            quantity: 1,
            ammo: "Bullets".into(),
            flags: IT_WEAPON | IT_STAY_COOP,
            weapmodel: WEAP_MACHINEGUN,
            armor_info: None,
            tag: 0,
            precaches: "weapons/machgf1b.wav weapons/machgf2b.wav weapons/machgf3b.wav weapons/machgf4b.wav weapons/machgf5b.wav".into(),
        },

        // 11: weapon_chaingun
        GItem {
            classname: "weapon_chaingun".into(),
            pickup_fn: Some(PICKUP_WEAPON),
            use_fn: Some(USE_WEAPON),
            drop_fn: Some(DROP_WEAPON),
            weaponthink_fn: Some(WEAPTHINK_CHAINGUN),
            pickup_sound: "misc/w_pkup.wav".into(),
            world_model: "models/weapons/g_chain/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: "models/weapons/v_chain/tris.md2".into(),
            icon: "w_chaingun".into(),
            pickup_name: "Chaingun".into(),
            count_width: 0,
            quantity: 1,
            ammo: "Bullets".into(),
            flags: IT_WEAPON | IT_STAY_COOP,
            weapmodel: WEAP_CHAINGUN,
            armor_info: None,
            tag: 0,
            precaches: "weapons/chngnu1a.wav weapons/chngnl1a.wav weapons/machgf3b.wav` weapons/chngnd1a.wav".into(),
        },

        // 12: ammo_grenades (weapon + ammo)
        GItem {
            classname: "ammo_grenades".into(),
            pickup_fn: Some(PICKUP_AMMO),
            use_fn: Some(USE_WEAPON),
            drop_fn: Some(DROP_AMMO),
            weaponthink_fn: Some(WEAPTHINK_GRENADE),
            pickup_sound: "misc/am_pkup.wav".into(),
            world_model: "models/items/ammo/grenades/medium/tris.md2".into(),
            world_model_flags: 0,
            view_model: "models/weapons/v_handgr/tris.md2".into(),
            icon: "a_grenades".into(),
            pickup_name: "Grenades".into(),
            count_width: 3,
            quantity: 5,
            ammo: "grenades".into(),
            flags: IT_AMMO | IT_WEAPON,
            weapmodel: WEAP_GRENADES,
            armor_info: None,
            tag: AMMO_GRENADES,
            precaches: "weapons/hgrent1a.wav weapons/hgrena1b.wav weapons/hgrenc1b.wav weapons/hgrenb1a.wav weapons/hgrenb2a.wav ".into(),
        },

        // 13: weapon_grenadelauncher
        GItem {
            classname: "weapon_grenadelauncher".into(),
            pickup_fn: Some(PICKUP_WEAPON),
            use_fn: Some(USE_WEAPON),
            drop_fn: Some(DROP_WEAPON),
            weaponthink_fn: Some(WEAPTHINK_GRENADELAUNCHER),
            pickup_sound: "misc/w_pkup.wav".into(),
            world_model: "models/weapons/g_launch/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: "models/weapons/v_launch/tris.md2".into(),
            icon: "w_glauncher".into(),
            pickup_name: "Grenade Launcher".into(),
            count_width: 0,
            quantity: 1,
            ammo: "Grenades".into(),
            flags: IT_WEAPON | IT_STAY_COOP,
            weapmodel: WEAP_GRENADELAUNCHER,
            armor_info: None,
            tag: 0,
            precaches: "models/objects/grenade/tris.md2 weapons/grenlf1a.wav weapons/grenlr1b.wav weapons/grenlb1b.wav".into(),
        },

        // 14: weapon_rocketlauncher
        GItem {
            classname: "weapon_rocketlauncher".into(),
            pickup_fn: Some(PICKUP_WEAPON),
            use_fn: Some(USE_WEAPON),
            drop_fn: Some(DROP_WEAPON),
            weaponthink_fn: Some(WEAPTHINK_ROCKETLAUNCHER),
            pickup_sound: "misc/w_pkup.wav".into(),
            world_model: "models/weapons/g_rocket/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: "models/weapons/v_rocket/tris.md2".into(),
            icon: "w_rlauncher".into(),
            pickup_name: "Rocket Launcher".into(),
            count_width: 0,
            quantity: 1,
            ammo: "Rockets".into(),
            flags: IT_WEAPON | IT_STAY_COOP,
            weapmodel: WEAP_ROCKETLAUNCHER,
            armor_info: None,
            tag: 0,
            precaches: "models/objects/rocket/tris.md2 weapons/rockfly.wav weapons/rocklf1a.wav weapons/rocklr1b.wav models/objects/debris2/tris.md2".into(),
        },

        // 15: weapon_hyperblaster
        GItem {
            classname: "weapon_hyperblaster".into(),
            pickup_fn: Some(PICKUP_WEAPON),
            use_fn: Some(USE_WEAPON),
            drop_fn: Some(DROP_WEAPON),
            weaponthink_fn: Some(WEAPTHINK_HYPERBLASTER),
            pickup_sound: "misc/w_pkup.wav".into(),
            world_model: "models/weapons/g_hyperb/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: "models/weapons/v_hyperb/tris.md2".into(),
            icon: "w_hyperblaster".into(),
            pickup_name: "HyperBlaster".into(),
            count_width: 0,
            quantity: 1,
            ammo: "Cells".into(),
            flags: IT_WEAPON | IT_STAY_COOP,
            weapmodel: WEAP_HYPERBLASTER,
            armor_info: None,
            tag: 0,
            precaches: "weapons/hyprbu1a.wav weapons/hyprbl1a.wav weapons/hyprbf1a.wav weapons/hyprbd1a.wav misc/lasfly.wav".into(),
        },

        // 16: weapon_railgun
        GItem {
            classname: "weapon_railgun".into(),
            pickup_fn: Some(PICKUP_WEAPON),
            use_fn: Some(USE_WEAPON),
            drop_fn: Some(DROP_WEAPON),
            weaponthink_fn: Some(WEAPTHINK_RAILGUN),
            pickup_sound: "misc/w_pkup.wav".into(),
            world_model: "models/weapons/g_rail/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: "models/weapons/v_rail/tris.md2".into(),
            icon: "w_railgun".into(),
            pickup_name: "Railgun".into(),
            count_width: 0,
            quantity: 1,
            ammo: "Slugs".into(),
            flags: IT_WEAPON | IT_STAY_COOP,
            weapmodel: WEAP_RAILGUN,
            armor_info: None,
            tag: 0,
            precaches: "weapons/rg_hum.wav".into(),
        },

        // 17: weapon_bfg
        GItem {
            classname: "weapon_bfg".into(),
            pickup_fn: Some(PICKUP_WEAPON),
            use_fn: Some(USE_WEAPON),
            drop_fn: Some(DROP_WEAPON),
            weaponthink_fn: Some(WEAPTHINK_BFG),
            pickup_sound: "misc/w_pkup.wav".into(),
            world_model: "models/weapons/g_bfg/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: "models/weapons/v_bfg/tris.md2".into(),
            icon: "w_bfg".into(),
            pickup_name: "BFG10K".into(),
            count_width: 0,
            quantity: 50,
            ammo: "Cells".into(),
            flags: IT_WEAPON | IT_STAY_COOP,
            weapmodel: WEAP_BFG,
            armor_info: None,
            tag: 0,
            precaches: "sprites/s_bfg1.sp2 sprites/s_bfg2.sp2 sprites/s_bfg3.sp2 weapons/bfg__f1y.wav weapons/bfg__l1a.wav weapons/bfg__x1b.wav weapons/bfg_hum.wav".into(),
        },

        // ============================================================
        // AMMO ITEMS
        // ============================================================

        // 18: ammo_shells
        GItem {
            classname: "ammo_shells".into(),
            pickup_fn: Some(PICKUP_AMMO),
            use_fn: None,
            drop_fn: Some(DROP_AMMO),
            weaponthink_fn: None,
            pickup_sound: "misc/am_pkup.wav".into(),
            world_model: "models/items/ammo/shells/medium/tris.md2".into(),
            world_model_flags: 0,
            view_model: String::new(),
            icon: "a_shells".into(),
            pickup_name: "Shells".into(),
            count_width: 3,
            quantity: 10,
            ammo: String::new(),
            flags: IT_AMMO,
            weapmodel: 0,
            armor_info: None,
            tag: AMMO_SHELLS,
            precaches: String::new(),
        },

        // 19: ammo_bullets
        GItem {
            classname: "ammo_bullets".into(),
            pickup_fn: Some(PICKUP_AMMO),
            use_fn: None,
            drop_fn: Some(DROP_AMMO),
            weaponthink_fn: None,
            pickup_sound: "misc/am_pkup.wav".into(),
            world_model: "models/items/ammo/bullets/medium/tris.md2".into(),
            world_model_flags: 0,
            view_model: String::new(),
            icon: "a_bullets".into(),
            pickup_name: "Bullets".into(),
            count_width: 3,
            quantity: 50,
            ammo: String::new(),
            flags: IT_AMMO,
            weapmodel: 0,
            armor_info: None,
            tag: AMMO_BULLETS,
            precaches: String::new(),
        },

        // 20: ammo_cells
        GItem {
            classname: "ammo_cells".into(),
            pickup_fn: Some(PICKUP_AMMO),
            use_fn: None,
            drop_fn: Some(DROP_AMMO),
            weaponthink_fn: None,
            pickup_sound: "misc/am_pkup.wav".into(),
            world_model: "models/items/ammo/cells/medium/tris.md2".into(),
            world_model_flags: 0,
            view_model: String::new(),
            icon: "a_cells".into(),
            pickup_name: "Cells".into(),
            count_width: 3,
            quantity: 50,
            ammo: String::new(),
            flags: IT_AMMO,
            weapmodel: 0,
            armor_info: None,
            tag: AMMO_CELLS,
            precaches: String::new(),
        },

        // 21: ammo_rockets
        GItem {
            classname: "ammo_rockets".into(),
            pickup_fn: Some(PICKUP_AMMO),
            use_fn: None,
            drop_fn: Some(DROP_AMMO),
            weaponthink_fn: None,
            pickup_sound: "misc/am_pkup.wav".into(),
            world_model: "models/items/ammo/rockets/medium/tris.md2".into(),
            world_model_flags: 0,
            view_model: String::new(),
            icon: "a_rockets".into(),
            pickup_name: "Rockets".into(),
            count_width: 3,
            quantity: 5,
            ammo: String::new(),
            flags: IT_AMMO,
            weapmodel: 0,
            armor_info: None,
            tag: AMMO_ROCKETS,
            precaches: String::new(),
        },

        // 22: ammo_slugs
        GItem {
            classname: "ammo_slugs".into(),
            pickup_fn: Some(PICKUP_AMMO),
            use_fn: None,
            drop_fn: Some(DROP_AMMO),
            weaponthink_fn: None,
            pickup_sound: "misc/am_pkup.wav".into(),
            world_model: "models/items/ammo/slugs/medium/tris.md2".into(),
            world_model_flags: 0,
            view_model: String::new(),
            icon: "a_slugs".into(),
            pickup_name: "Slugs".into(),
            count_width: 3,
            quantity: 10,
            ammo: String::new(),
            flags: IT_AMMO,
            weapmodel: 0,
            armor_info: None,
            tag: AMMO_SLUGS,
            precaches: String::new(),
        },

        // ============================================================
        // POWERUP ITEMS
        // ============================================================

        // 23: item_quad
        GItem {
            classname: "item_quad".into(),
            pickup_fn: Some(PICKUP_POWERUP),
            use_fn: Some(USE_QUAD),
            drop_fn: Some(DROP_GENERAL),
            weaponthink_fn: None,
            pickup_sound: "items/pkup.wav".into(),
            world_model: "models/items/quaddama/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: String::new(),
            icon: "p_quad".into(),
            pickup_name: "Quad Damage".into(),
            count_width: 2,
            quantity: 60,
            ammo: String::new(),
            flags: IT_POWERUP,
            weapmodel: 0,
            armor_info: None,
            tag: 0,
            precaches: "items/damage.wav items/damage2.wav items/damage3.wav".into(),
        },

        // 24: item_invulnerability
        GItem {
            classname: "item_invulnerability".into(),
            pickup_fn: Some(PICKUP_POWERUP),
            use_fn: Some(USE_INVULNERABILITY),
            drop_fn: Some(DROP_GENERAL),
            weaponthink_fn: None,
            pickup_sound: "items/pkup.wav".into(),
            world_model: "models/items/invulner/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: String::new(),
            icon: "p_invulnerability".into(),
            pickup_name: "Invulnerability".into(),
            count_width: 2,
            quantity: 300,
            ammo: String::new(),
            flags: IT_POWERUP,
            weapmodel: 0,
            armor_info: None,
            tag: 0,
            precaches: "items/protect.wav items/protect2.wav items/protect4.wav".into(),
        },

        // 25: item_silencer
        GItem {
            classname: "item_silencer".into(),
            pickup_fn: Some(PICKUP_POWERUP),
            use_fn: Some(USE_SILENCER),
            drop_fn: Some(DROP_GENERAL),
            weaponthink_fn: None,
            pickup_sound: "items/pkup.wav".into(),
            world_model: "models/items/silencer/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: String::new(),
            icon: "p_silencer".into(),
            pickup_name: "Silencer".into(),
            count_width: 2,
            quantity: 60,
            ammo: String::new(),
            flags: IT_POWERUP,
            weapmodel: 0,
            armor_info: None,
            tag: 0,
            precaches: String::new(),
        },

        // 26: item_breather
        GItem {
            classname: "item_breather".into(),
            pickup_fn: Some(PICKUP_POWERUP),
            use_fn: Some(USE_BREATHER),
            drop_fn: Some(DROP_GENERAL),
            weaponthink_fn: None,
            pickup_sound: "items/pkup.wav".into(),
            world_model: "models/items/breather/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: String::new(),
            icon: "p_rebreather".into(),
            pickup_name: "Rebreather".into(),
            count_width: 2,
            quantity: 60,
            ammo: String::new(),
            flags: IT_STAY_COOP | IT_POWERUP,
            weapmodel: 0,
            armor_info: None,
            tag: 0,
            precaches: "items/airout.wav".into(),
        },

        // 27: item_enviro
        GItem {
            classname: "item_enviro".into(),
            pickup_fn: Some(PICKUP_POWERUP),
            use_fn: Some(USE_ENVIROSUIT),
            drop_fn: Some(DROP_GENERAL),
            weaponthink_fn: None,
            pickup_sound: "items/pkup.wav".into(),
            world_model: "models/items/enviro/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: String::new(),
            icon: "p_envirosuit".into(),
            pickup_name: "Environment Suit".into(),
            count_width: 2,
            quantity: 60,
            ammo: String::new(),
            flags: IT_STAY_COOP | IT_POWERUP,
            weapmodel: 0,
            armor_info: None,
            tag: 0,
            precaches: "items/airout.wav".into(),
        },

        // 28: item_ancient_head
        GItem {
            classname: "item_ancient_head".into(),
            pickup_fn: Some(PICKUP_ANCIENTHEAD),
            use_fn: None,
            drop_fn: None,
            weaponthink_fn: None,
            pickup_sound: "items/pkup.wav".into(),
            world_model: "models/items/c_head/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: String::new(),
            icon: "i_fixme".into(),
            pickup_name: "Ancient Head".into(),
            count_width: 2,
            quantity: 60,
            ammo: String::new(),
            flags: ItemFlags::empty(),
            weapmodel: 0,
            armor_info: None,
            tag: 0,
            precaches: String::new(),
        },

        // 29: item_adrenaline
        GItem {
            classname: "item_adrenaline".into(),
            pickup_fn: Some(PICKUP_ADRENALINE),
            use_fn: None,
            drop_fn: None,
            weaponthink_fn: None,
            pickup_sound: "items/pkup.wav".into(),
            world_model: "models/items/adrenal/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: String::new(),
            icon: "p_adrenaline".into(),
            pickup_name: "Adrenaline".into(),
            count_width: 2,
            quantity: 60,
            ammo: String::new(),
            flags: ItemFlags::empty(),
            weapmodel: 0,
            armor_info: None,
            tag: 0,
            precaches: String::new(),
        },

        // 30: item_bandolier
        GItem {
            classname: "item_bandolier".into(),
            pickup_fn: Some(PICKUP_BANDOLIER),
            use_fn: None,
            drop_fn: None,
            weaponthink_fn: None,
            pickup_sound: "items/pkup.wav".into(),
            world_model: "models/items/band/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: String::new(),
            icon: "p_bandolier".into(),
            pickup_name: "Bandolier".into(),
            count_width: 2,
            quantity: 60,
            ammo: String::new(),
            flags: ItemFlags::empty(),
            weapmodel: 0,
            armor_info: None,
            tag: 0,
            precaches: String::new(),
        },

        // 31: item_pack
        GItem {
            classname: "item_pack".into(),
            pickup_fn: Some(PICKUP_PACK),
            use_fn: None,
            drop_fn: None,
            weaponthink_fn: None,
            pickup_sound: "items/pkup.wav".into(),
            world_model: "models/items/pack/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: String::new(),
            icon: "i_pack".into(),
            pickup_name: "Ammo Pack".into(),
            count_width: 2,
            quantity: 180,
            ammo: String::new(),
            flags: ItemFlags::empty(),
            weapmodel: 0,
            armor_info: None,
            tag: 0,
            precaches: String::new(),
        },

        // ============================================================
        // KEYS
        // ============================================================

        // 32: key_data_cd
        GItem {
            classname: "key_data_cd".into(),
            pickup_fn: Some(PICKUP_KEY),
            use_fn: None,
            drop_fn: Some(DROP_GENERAL),
            weaponthink_fn: None,
            pickup_sound: "items/pkup.wav".into(),
            world_model: "models/items/keys/data_cd/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: String::new(),
            icon: "k_datacd".into(),
            pickup_name: "Data CD".into(),
            count_width: 2,
            quantity: 0,
            ammo: String::new(),
            flags: IT_STAY_COOP | IT_KEY,
            weapmodel: 0,
            armor_info: None,
            tag: 0,
            precaches: String::new(),
        },

        // 33: key_power_cube
        GItem {
            classname: "key_power_cube".into(),
            pickup_fn: Some(PICKUP_KEY),
            use_fn: None,
            drop_fn: Some(DROP_GENERAL),
            weaponthink_fn: None,
            pickup_sound: "items/pkup.wav".into(),
            world_model: "models/items/keys/power/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: String::new(),
            icon: "k_powercube".into(),
            pickup_name: "Power Cube".into(),
            count_width: 2,
            quantity: 0,
            ammo: String::new(),
            flags: IT_STAY_COOP | IT_KEY,
            weapmodel: 0,
            armor_info: None,
            tag: 0,
            precaches: String::new(),
        },

        // 34: key_pyramid
        GItem {
            classname: "key_pyramid".into(),
            pickup_fn: Some(PICKUP_KEY),
            use_fn: None,
            drop_fn: Some(DROP_GENERAL),
            weaponthink_fn: None,
            pickup_sound: "items/pkup.wav".into(),
            world_model: "models/items/keys/pyramid/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: String::new(),
            icon: "k_pyramid".into(),
            pickup_name: "Pyramid Key".into(),
            count_width: 2,
            quantity: 0,
            ammo: String::new(),
            flags: IT_STAY_COOP | IT_KEY,
            weapmodel: 0,
            armor_info: None,
            tag: 0,
            precaches: String::new(),
        },

        // 35: key_data_spinner
        GItem {
            classname: "key_data_spinner".into(),
            pickup_fn: Some(PICKUP_KEY),
            use_fn: None,
            drop_fn: Some(DROP_GENERAL),
            weaponthink_fn: None,
            pickup_sound: "items/pkup.wav".into(),
            world_model: "models/items/keys/spinner/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: String::new(),
            icon: "k_dataspin".into(),
            pickup_name: "Data Spinner".into(),
            count_width: 2,
            quantity: 0,
            ammo: String::new(),
            flags: IT_STAY_COOP | IT_KEY,
            weapmodel: 0,
            armor_info: None,
            tag: 0,
            precaches: String::new(),
        },

        // 36: key_pass
        GItem {
            classname: "key_pass".into(),
            pickup_fn: Some(PICKUP_KEY),
            use_fn: None,
            drop_fn: Some(DROP_GENERAL),
            weaponthink_fn: None,
            pickup_sound: "items/pkup.wav".into(),
            world_model: "models/items/keys/pass/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: String::new(),
            icon: "k_security".into(),
            pickup_name: "Security Pass".into(),
            count_width: 2,
            quantity: 0,
            ammo: String::new(),
            flags: IT_STAY_COOP | IT_KEY,
            weapmodel: 0,
            armor_info: None,
            tag: 0,
            precaches: String::new(),
        },

        // 37: key_blue_key
        GItem {
            classname: "key_blue_key".into(),
            pickup_fn: Some(PICKUP_KEY),
            use_fn: None,
            drop_fn: Some(DROP_GENERAL),
            weaponthink_fn: None,
            pickup_sound: "items/pkup.wav".into(),
            world_model: "models/items/keys/key/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: String::new(),
            icon: "k_bluekey".into(),
            pickup_name: "Blue Key".into(),
            count_width: 2,
            quantity: 0,
            ammo: String::new(),
            flags: IT_STAY_COOP | IT_KEY,
            weapmodel: 0,
            armor_info: None,
            tag: 0,
            precaches: String::new(),
        },

        // 38: key_red_key
        GItem {
            classname: "key_red_key".into(),
            pickup_fn: Some(PICKUP_KEY),
            use_fn: None,
            drop_fn: Some(DROP_GENERAL),
            weaponthink_fn: None,
            pickup_sound: "items/pkup.wav".into(),
            world_model: "models/items/keys/red_key/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: String::new(),
            icon: "k_redkey".into(),
            pickup_name: "Red Key".into(),
            count_width: 2,
            quantity: 0,
            ammo: String::new(),
            flags: IT_STAY_COOP | IT_KEY,
            weapmodel: 0,
            armor_info: None,
            tag: 0,
            precaches: String::new(),
        },

        // 39: key_commander_head
        GItem {
            classname: "key_commander_head".into(),
            pickup_fn: Some(PICKUP_KEY),
            use_fn: None,
            drop_fn: Some(DROP_GENERAL),
            weaponthink_fn: None,
            pickup_sound: "items/pkup.wav".into(),
            world_model: "models/monsters/commandr/head/tris.md2".into(),
            world_model_flags: EF_GIB,
            view_model: String::new(),
            icon: "k_comhead".into(),
            pickup_name: "Commander's Head".into(),
            count_width: 2,
            quantity: 0,
            ammo: String::new(),
            flags: IT_STAY_COOP | IT_KEY,
            weapmodel: 0,
            armor_info: None,
            tag: 0,
            precaches: String::new(),
        },

        // 40: key_airstrike_target
        GItem {
            classname: "key_airstrike_target".into(),
            pickup_fn: Some(PICKUP_KEY),
            use_fn: None,
            drop_fn: Some(DROP_GENERAL),
            weaponthink_fn: None,
            pickup_sound: "items/pkup.wav".into(),
            world_model: "models/items/keys/target/tris.md2".into(),
            world_model_flags: EF_ROTATE,
            view_model: String::new(),
            icon: "i_airstrike".into(),
            pickup_name: "Airstrike Marker".into(),
            count_width: 2,
            quantity: 0,
            ammo: String::new(),
            flags: IT_STAY_COOP | IT_KEY,
            weapmodel: 0,
            armor_info: None,
            tag: 0,
            precaches: String::new(),
        },

        // 41: Health (generic, used by SP_item_health* spawn functions)
        GItem {
            classname: String::new(),
            pickup_fn: Some(PICKUP_HEALTH),
            use_fn: None,
            drop_fn: None,
            weaponthink_fn: None,
            pickup_sound: "items/pkup.wav".into(),
            world_model: String::new(),
            world_model_flags: 0,
            view_model: String::new(),
            icon: "i_health".into(),
            pickup_name: "Health".into(),
            count_width: 3,
            quantity: 0,
            ammo: String::new(),
            flags: ItemFlags::empty(),
            weapmodel: 0,
            armor_info: None,
            tag: 0,
            precaches: "items/s_health.wav items/n_health.wav items/l_health.wav items/m_health.wav".into(),
        },

        // End of list marker
        GItem::default(),
    ]
}

// ============================================================
// Dispatch helpers
// ============================================================

/// Dispatch a pickup function by ID. Returns true if the item was picked up.
fn dispatch_pickup(
    ctx: &mut GameContext,
    pickup_fn: Option<usize>,
    ent_idx: usize,
    other_idx: usize,
) -> bool {
    match pickup_fn {
        Some(PICKUP_ARMOR) => pickup_armor(ctx, ent_idx, other_idx),
        Some(PICKUP_WEAPON) => {
            // Pickup_Weapon deferred: p_weapon::GameContext differs from g_items::GameContext
            true
        }
        Some(PICKUP_AMMO) => pickup_ammo(ctx, ent_idx, other_idx),
        Some(PICKUP_POWERUP) => pickup_powerup(ctx, ent_idx, other_idx),
        Some(PICKUP_KEY) => pickup_key(ctx, ent_idx, other_idx),
        Some(PICKUP_HEALTH) => pickup_health(ctx, ent_idx, other_idx),
        Some(PICKUP_ADRENALINE) => pickup_adrenaline(ctx, ent_idx, other_idx),
        Some(PICKUP_ANCIENTHEAD) => pickup_ancient_head(ctx, ent_idx, other_idx),
        Some(PICKUP_BANDOLIER) => pickup_bandolier(ctx, ent_idx, other_idx),
        Some(PICKUP_PACK) => pickup_pack(ctx, ent_idx, other_idx),
        Some(PICKUP_POWERARMOR) => pickup_power_armor(ctx, ent_idx, other_idx),
        _ => false,
    }
}

/// Dispatch a use function by ID.
pub fn dispatch_item_use(
    ctx: &mut GameContext,
    use_fn: Option<usize>,
    ent_idx: usize,
    item_idx: usize,
) {
    match use_fn {
        Some(USE_WEAPON) => {
            // Inlined Use_Weapon logic (p_weapon.rs has a different GameContext type)
            if let Some(client_idx) = ctx.edicts[ent_idx].client {
                // Already using this weapon?
                if ctx.clients[client_idx].pers.weapon == Some(item_idx) {
                    return;
                }
                let item = &ctx.items[item_idx];
                if !item.ammo.is_empty() && !item.flags.intersects(IT_AMMO) {
                    if let Some(ammo_idx) = find_item(&item.ammo.clone()) {
                        if ctx.clients[client_idx].pers.inventory[ammo_idx] == 0 {
                            gi_cprintf(
                                ctx.edicts[ent_idx].s.number,
                                PRINT_HIGH,
                                &format!("No {} for {}.\n", ctx.items[ammo_idx].pickup_name, ctx.items[item_idx].pickup_name),
                            );
                            return;
                        }
                    }
                }
                ctx.clients[client_idx].newweapon = Some(item_idx);
            }
        }
        Some(USE_QUAD) => use_quad(ctx, ent_idx, item_idx),
        Some(USE_BREATHER) => use_breather(ctx, ent_idx, item_idx),
        Some(USE_ENVIROSUIT) => use_envirosuit(ctx, ent_idx, item_idx),
        Some(USE_INVULNERABILITY) => use_invulnerability(ctx, ent_idx, item_idx),
        Some(USE_SILENCER) => use_silencer(ctx, ent_idx, item_idx),
        Some(USE_POWERARMOR) => use_power_armor(ctx, ent_idx, item_idx),
        _ => {}
    }
}

/// Dispatch a drop function by ID.
pub fn dispatch_item_drop(
    ctx: &mut GameContext,
    drop_fn: Option<usize>,
    ent_idx: usize,
    item_idx: usize,
) {
    match drop_fn {
        Some(DROP_WEAPON) => {
            // Inlined Drop_Weapon logic (p_weapon.rs has a different GameContext type)
            if DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_WEAPONS_STAY) {
                return;
            }
            if let Some(client_idx) = ctx.edicts[ent_idx].client {
                if (ctx.clients[client_idx].pers.weapon == Some(item_idx)
                    || ctx.clients[client_idx].newweapon == Some(item_idx))
                    && ctx.clients[client_idx].pers.inventory[item_idx] == 1
                {
                    gi_cprintf(ctx.edicts[ent_idx].s.number, PRINT_HIGH, "Can't drop current weapon\n");
                    return;
                }
                // Drop_Item + decrement inventory
                drop_item(ctx, ent_idx, item_idx);
                ctx.clients[client_idx].pers.inventory[item_idx] -= 1;
            }
        }
        Some(DROP_AMMO) => drop_ammo(ctx, ent_idx, item_idx),
        Some(DROP_GENERAL) => drop_general(ctx, ent_idx, item_idx),
        Some(DROP_POWERARMOR) => drop_power_armor(ctx, ent_idx, item_idx),
        _ => {}
    }
}

// ============================================================
// Placeholder cross-module functions
// ============================================================


use myq2_common::common::rand_i32 as rand_int;

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a GameContext with the full item table populated and indices built.
    fn make_ctx_with_items() -> GameContext {
        let mut ctx = GameContext::default();
        let itemlist = build_itemlist();
        ctx.game.num_items = (itemlist.len() - 1) as i32; // last entry is null terminator
        ctx.items = itemlist;
        ctx.build_item_indices();
        ctx
    }

    // ============================================================
    // find_item_by_classname tests
    // ============================================================

    #[test]
    fn test_find_item_by_classname_weapon_shotgun() {
        let _ctx = make_ctx_with_items();
        let result = find_item_by_classname("weapon_shotgun");
        assert!(result.is_some(), "weapon_shotgun should be found");
    }

    #[test]
    fn test_find_item_by_classname_weapon_blaster() {
        let _ctx = make_ctx_with_items();
        let result = find_item_by_classname("weapon_blaster");
        assert!(result.is_some(), "weapon_blaster should be found");
    }

    #[test]
    fn test_find_item_by_classname_case_insensitive() {
        let _ctx = make_ctx_with_items();
        let result = find_item_by_classname("WEAPON_SHOTGUN");
        assert!(result.is_some(), "lookup should be case-insensitive");
    }

    #[test]
    fn test_find_item_by_classname_armor() {
        let _ctx = make_ctx_with_items();
        let result = find_item_by_classname("item_armor_body");
        assert!(result.is_some(), "item_armor_body should be found");
    }

    #[test]
    fn test_find_item_by_classname_unknown() {
        let _ctx = make_ctx_with_items();
        let result = find_item_by_classname("weapon_nonexistent");
        assert!(result.is_none(), "unknown classname should return None");
    }

    #[test]
    fn test_find_item_by_classname_empty() {
        let _ctx = make_ctx_with_items();
        let result = find_item_by_classname("");
        assert!(result.is_none(), "empty classname should return None");
    }

    #[test]
    fn test_find_item_by_classname_returns_correct_index() {
        let ctx = make_ctx_with_items();
        let idx = find_item_by_classname("weapon_shotgun").unwrap();
        assert_eq!(ctx.items[idx].classname, "weapon_shotgun");
    }

    // ============================================================
    // find_item (by pickup name) tests
    // ============================================================

    #[test]
    fn test_find_item_shotgun() {
        let _ctx = make_ctx_with_items();
        let result = find_item("Shotgun");
        assert!(result.is_some(), "Shotgun should be found by pickup name");
    }

    #[test]
    fn test_find_item_blaster() {
        let _ctx = make_ctx_with_items();
        let result = find_item("Blaster");
        assert!(result.is_some(), "Blaster should be found by pickup name");
    }

    #[test]
    fn test_find_item_case_insensitive() {
        let _ctx = make_ctx_with_items();
        let result = find_item("SHOTGUN");
        assert!(result.is_some(), "lookup should be case-insensitive");
    }

    #[test]
    fn test_find_item_body_armor() {
        let _ctx = make_ctx_with_items();
        let result = find_item("Body Armor");
        assert!(result.is_some(), "Body Armor should be found by pickup name");
    }

    #[test]
    fn test_find_item_unknown() {
        let _ctx = make_ctx_with_items();
        let result = find_item("Nonexistent Item");
        assert!(result.is_none(), "unknown pickup name should return None");
    }

    #[test]
    fn test_find_item_empty() {
        let _ctx = make_ctx_with_items();
        let result = find_item("");
        assert!(result.is_none(), "empty pickup name should return None");
    }

    #[test]
    fn test_find_item_returns_correct_index() {
        let ctx = make_ctx_with_items();
        let idx = find_item("Shotgun").unwrap();
        assert_eq!(ctx.items[idx].pickup_name, "Shotgun");
    }

    #[test]
    fn test_find_item_rockets() {
        let _ctx = make_ctx_with_items();
        let result = find_item("Rockets");
        assert!(result.is_some(), "Rockets ammo should be found by pickup name");
    }
}
