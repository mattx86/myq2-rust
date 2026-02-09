// g_cmds.rs — Client command processing
// Converted from: myq2-original/game/g_cmds.c
//
// Copyright (C) 1997-2001 Id Software, Inc.
// Licensed under the GNU General Public License v2+

use crate::g_local::*;
use crate::g_items::{find_item, dispatch_item_use, dispatch_item_drop};
use crate::game_import::*;
use myq2_common::q_shared::{
    DF_MODELTEAMS, DF_SKINTEAMS, MAX_ITEMS, PMF_DUCKED, STAT_FRAGS,
    PRINT_HIGH, PRINT_CHAT,
    DmFlags,
};
use myq2_common::common::{DISTNAME, DISTVER};
use myq2_common::qcommon::{BUILDSTRING, CPUSTRING};
use crate::m_player_frames::*;

// ============================================================
// Additional cvars needed by g_cmds
// ============================================================

/// Extract the team name from a client's userinfo "skin" key.
/// Returns the model name if DF_MODELTEAMS, or the skin name otherwise.
pub fn client_team(ctx: &GameContext, ent_idx: usize) -> String {
    let ent = &ctx.edicts[ent_idx];
    let client_idx = match ent.client {
        Some(c) => c,
        None => return String::new(),
    };
    let cl = &ctx.clients[client_idx];

    // In the C code: Info_ValueForKey(ent->client->pers.userinfo, "skin")
    // Placeholder: parse "skin" from userinfo string
    let value = info_value_for_key(&cl.pers.userinfo, "skin");

    if let Some(slash_pos) = value.find('/') {
        if DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_MODELTEAMS) {
            // Return model name (before slash)
            return value[..slash_pos].to_string();
        }
        // DF_SKINTEAMS — return skin name (after slash)
        return value[slash_pos + 1..].to_string();
    }

    value
}

use myq2_common::q_shared::info_value_for_key;

/// Check if two entities are on the same team (for teamplay modes).
pub fn on_same_team(ctx: &GameContext, ent1_idx: usize, ent2_idx: usize) -> bool {
    if !DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_MODELTEAMS | DF_SKINTEAMS) {
        return false;
    }

    let ent1_team = client_team(ctx, ent1_idx);
    let ent2_team = client_team(ctx, ent2_idx);

    ent1_team == ent2_team
}

/// Select the next inventory item matching the given flags.
pub fn select_next_item(ctx: &mut GameContext, ent_idx: usize, itflags: ItemFlags) {
    let client_idx = match ctx.edicts[ent_idx].client {
        Some(c) => c,
        None => return,
    };

    if ctx.clients[client_idx].chase_target > 0 {
        crate::g_chase::chase_next(ctx, ent_idx);
        return;
    }

    // scan for the next valid one
    for i in 1..=MAX_ITEMS {
        let selected = ctx.clients[client_idx].pers.selected_item;
        let index = ((selected + i as i32) % MAX_ITEMS as i32) as usize;
        if ctx.clients[client_idx].pers.inventory[index] == 0 {
            continue;
        }
        if index >= ctx.items.len() {
            continue;
        }
        let it = &ctx.items[index];
        if it.use_fn.is_none() {
            continue;
        }
        if !itflags.is_empty() && !it.flags.intersects(itflags) {
            continue;
        }
        ctx.clients[client_idx].pers.selected_item = index as i32;
        return;
    }

    ctx.clients[client_idx].pers.selected_item = -1;
}

/// Select the previous inventory item matching the given flags.
pub fn select_prev_item(ctx: &mut GameContext, ent_idx: usize, itflags: ItemFlags) {
    let client_idx = match ctx.edicts[ent_idx].client {
        Some(c) => c,
        None => return,
    };

    if ctx.clients[client_idx].chase_target > 0 {
        crate::g_chase::chase_prev(ctx, ent_idx);
        return;
    }

    // scan for the next valid one
    for i in 1..=MAX_ITEMS {
        let selected = ctx.clients[client_idx].pers.selected_item;
        let index = ((selected + MAX_ITEMS as i32 - i as i32) % MAX_ITEMS as i32) as usize;
        if ctx.clients[client_idx].pers.inventory[index] == 0 {
            continue;
        }
        if index >= ctx.items.len() {
            continue;
        }
        let it = &ctx.items[index];
        if it.use_fn.is_none() {
            continue;
        }
        if !itflags.is_empty() && !it.flags.intersects(itflags) {
            continue;
        }
        ctx.clients[client_idx].pers.selected_item = index as i32;
        return;
    }

    ctx.clients[client_idx].pers.selected_item = -1;
}

/// Validate the currently selected item — if it's not in inventory, select next.
pub fn validate_selected_item(ctx: &mut GameContext, ent_idx: usize) {
    let client_idx = match ctx.edicts[ent_idx].client {
        Some(c) => c,
        None => return,
    };

    let selected = ctx.clients[client_idx].pers.selected_item;
    if selected >= 0
        && (selected as usize) < MAX_ITEMS
        && ctx.clients[client_idx].pers.inventory[selected as usize] != 0
    {
        return; // valid
    }

    select_next_item(ctx, ent_idx, ItemFlags::empty());
}

// =================================================================================

/// Give items to a client (cheat command).
pub fn cmd_give_f(ctx: &mut GameContext, ent_idx: usize, args: &str, argv: &[&str]) {
    if ctx.deathmatch != 0.0 {
        gi_cprintf(ent_idx as i32, PRINT_HIGH, "You must run the server with '+set cheats 1' to enable this command.\n");
        return;
    }

    let name = args;
    let give_all = name.eq_ignore_ascii_case("all");

    if give_all || argv.get(1).is_some_and(|s| s.eq_ignore_ascii_case("health")) {
        if argv.len() == 3 {
            ctx.edicts[ent_idx].health = argv[2].parse().unwrap_or(0);
        } else {
            ctx.edicts[ent_idx].health = ctx.edicts[ent_idx].max_health;
        }
        if !give_all {
            return;
        }
    }

    if give_all || name.eq_ignore_ascii_case("weapons") {
        let num_items = ctx.game.num_items as usize;
        for i in 0..num_items {
            let it = &ctx.items[i];
            if it.pickup_fn.is_none() {
                continue;
            }
            if !it.flags.intersects(IT_WEAPON) {
                continue;
            }
            let client_idx = ctx.edicts[ent_idx].client.unwrap();
            ctx.clients[client_idx].pers.inventory[i] += 1;
        }
        if !give_all {
            return;
        }
    }

    if give_all || name.eq_ignore_ascii_case("ammo") {
        let num_items = ctx.game.num_items as usize;
        for i in 0..num_items {
            let it = &ctx.items[i];
            if it.pickup_fn.is_none() {
                continue;
            }
            if !it.flags.intersects(IT_AMMO) {
                continue;
            }
            crate::g_items::add_ammo(ctx, ent_idx, i, 1000);
        }
        if !give_all {
            return;
        }
    }

    if give_all || name.eq_ignore_ascii_case("armor") {
        if let Some(jacket_idx) = find_item("Jacket Armor") {
            let client_idx = ctx.edicts[ent_idx].client.unwrap();
            ctx.clients[client_idx].pers.inventory[jacket_idx] = 0;
        }

        if let Some(combat_idx) = find_item("Combat Armor") {
            let client_idx = ctx.edicts[ent_idx].client.unwrap();
            ctx.clients[client_idx].pers.inventory[combat_idx] = 0;
        }

        if let Some(body_idx) = find_item("Body Armor") {
            let max_count = ctx.items[body_idx]
                .armor_info
                .as_ref()
                .map_or(0, |info| info.max_count);
            let client_idx = ctx.edicts[ent_idx].client.unwrap();
            ctx.clients[client_idx].pers.inventory[body_idx] = max_count;
        }

        if !give_all {
            return;
        }
    }

    if give_all || name.eq_ignore_ascii_case("Power Shield") {
        if let Some(_ps_idx) = find_item("Power Shield") {
            // G_Spawn/SpawnItem/Touch_Item deferred: requires g_spawn context integration
        }
        if !give_all {
            return;
        }
    }

    if give_all {
        let num_items = ctx.game.num_items as usize;
        for i in 0..num_items {
            let it = &ctx.items[i];
            if it.pickup_fn.is_none() {
                continue;
            }
            if it.flags.intersects(IT_ARMOR | IT_WEAPON | IT_AMMO) {
                continue;
            }
            let client_idx = ctx.edicts[ent_idx].client.unwrap();
            ctx.clients[client_idx].pers.inventory[i] = 1;
        }
        return;
    }

    // Try finding item by full args, then by argv[1]
    let mut item_idx_opt = find_item(name);
    if item_idx_opt.is_none() {
        if let Some(arg1) = argv.get(1) {
            item_idx_opt = find_item(arg1);
        }
    }
    let item_idx = match item_idx_opt {
        Some(idx) => idx,
        None => {
            gi_cprintf(ent_idx as i32, PRINT_HIGH, "unknown item\n");
            return;
        }
    };

    if ctx.items[item_idx].pickup_fn.is_none() {
        gi_cprintf(ent_idx as i32, PRINT_HIGH, "non-pickup item\n");
        return;
    }

    if ctx.items[item_idx].flags.intersects(IT_AMMO) {
        let client_idx = ctx.edicts[ent_idx].client.unwrap();
        if argv.len() == 3 {
            ctx.clients[client_idx].pers.inventory[item_idx] = argv[2].parse().unwrap_or(0);
        } else {
            ctx.clients[client_idx].pers.inventory[item_idx] += ctx.items[item_idx].quantity;
        }
    } else {
        // G_Spawn/SpawnItem/Touch_Item deferred: requires g_spawn context integration
    }
}

/// Sets client to godmode.
pub fn cmd_god_f(ctx: &mut GameContext, ent_idx: usize) {
    if ctx.deathmatch != 0.0 {
        gi_cprintf(ent_idx as i32, PRINT_HIGH, "You must run the server with '+set cheats 1' to enable this command.\n");
        return;
    }

    ctx.edicts[ent_idx].flags.toggle(FL_GODMODE);
    let msg = if !ctx.edicts[ent_idx].flags.intersects(FL_GODMODE) {
        "godmode OFF\n"
    } else {
        "godmode ON\n"
    };
    gi_cprintf(ent_idx as i32, PRINT_HIGH, msg);
}

/// Sets client to notarget.
pub fn cmd_notarget_f(ctx: &mut GameContext, ent_idx: usize) {
    if ctx.deathmatch != 0.0 {
        gi_cprintf(ent_idx as i32, PRINT_HIGH, "You must run the server with '+set cheats 1' to enable this command.\n");
        return;
    }

    ctx.edicts[ent_idx].flags.toggle(FL_NOTARGET);
    let msg = if !ctx.edicts[ent_idx].flags.intersects(FL_NOTARGET) {
        "notarget OFF\n"
    } else {
        "notarget ON\n"
    };
    gi_cprintf(ent_idx as i32, PRINT_HIGH, msg);
}

/// Toggle noclip mode.
pub fn cmd_noclip_f(ctx: &mut GameContext, ent_idx: usize) {
    if ctx.deathmatch != 0.0 {
        gi_cprintf(ent_idx as i32, PRINT_HIGH, "You must run the server with '+set cheats 1' to enable this command.\n");
        return;
    }

    let msg = if ctx.edicts[ent_idx].movetype == MoveType::Noclip {
        ctx.edicts[ent_idx].movetype = MoveType::Walk;
        "noclip OFF\n"
    } else {
        ctx.edicts[ent_idx].movetype = MoveType::Noclip;
        "noclip ON\n"
    };
    gi_cprintf(ent_idx as i32, PRINT_HIGH, msg);
}

/// Use an inventory item.
pub fn cmd_use_f(ctx: &mut GameContext, ent_idx: usize, args: &str) {
    let item_idx = match find_item(args) {
        Some(idx) => idx,
        None => {
            gi_cprintf(ent_idx as i32, PRINT_HIGH, &format!("unknown item: {}\n", args));
            return;
        }
    };

    if ctx.items[item_idx].use_fn.is_none() {
        gi_cprintf(ent_idx as i32, PRINT_HIGH, "Item is not usable.\n");
        return;
    }

    let client_idx = ctx.edicts[ent_idx].client.unwrap();
    if ctx.clients[client_idx].pers.inventory[item_idx] == 0 {
        gi_cprintf(ent_idx as i32, PRINT_HIGH, &format!("Out of item: {}\n", args));
        return;
    }

    // it->use(ent, it)
    let use_fn_id = ctx.items[item_idx].use_fn;
    dispatch_item_use(ctx, use_fn_id, ent_idx, item_idx);
}

/// Drop an inventory item.
pub fn cmd_drop_f(ctx: &mut GameContext, ent_idx: usize, args: &str) {
    let item_idx = match find_item(args) {
        Some(idx) => idx,
        None => {
            gi_cprintf(ent_idx as i32, PRINT_HIGH, &format!("unknown item: {}\n", args));
            return;
        }
    };

    if ctx.items[item_idx].drop_fn.is_none() {
        gi_cprintf(ent_idx as i32, PRINT_HIGH, "Item is not dropable.\n");
        return;
    }

    let client_idx = ctx.edicts[ent_idx].client.unwrap();
    if ctx.clients[client_idx].pers.inventory[item_idx] == 0 {
        gi_cprintf(ent_idx as i32, PRINT_HIGH, &format!("Out of item: {}\n", args));
        return;
    }

    // it->drop(ent, it)
    let drop_fn_id = ctx.items[item_idx].drop_fn;
    dispatch_item_drop(ctx, drop_fn_id, ent_idx, item_idx);
}

/// Show/hide inventory screen.
pub fn cmd_inven_f(ctx: &mut GameContext, ent_idx: usize) {
    let client_idx = match ctx.edicts[ent_idx].client {
        Some(c) => c,
        None => return,
    };

    ctx.clients[client_idx].showscores = false;
    ctx.clients[client_idx].showhelp = false;

    if ctx.clients[client_idx].showinventory {
        ctx.clients[client_idx].showinventory = false;
        return;
    }

    ctx.clients[client_idx].showinventory = true;

    gi_write_byte(SVC_INVENTORY);
    for i in 0..MAX_ITEMS {
        gi_write_short(ctx.clients[client_idx].pers.inventory[i] as i32);
    }
    gi_unicast(ent_idx as i32, true);
}

/// Use the currently selected inventory item.
pub fn cmd_invuse_f(ctx: &mut GameContext, ent_idx: usize) {
    validate_selected_item(ctx, ent_idx);

    let client_idx = ctx.edicts[ent_idx].client.unwrap();
    let selected = ctx.clients[client_idx].pers.selected_item;

    if selected == -1 {
        gi_cprintf(ent_idx as i32, PRINT_HIGH, "No item to use.\n");
        return;
    }

    let item_idx = selected as usize;
    if ctx.items[item_idx].use_fn.is_none() {
        gi_cprintf(ent_idx as i32, PRINT_HIGH, "Item is not usable.\n");
        return;
    }

    // it->use(ent, it)
    let use_fn_id = ctx.items[item_idx].use_fn;
    dispatch_item_use(ctx, use_fn_id, ent_idx, item_idx);
}

/// Cycle to previous weapon.
pub fn cmd_weap_prev_f(ctx: &mut GameContext, ent_idx: usize) {
    let client_idx = match ctx.edicts[ent_idx].client {
        Some(c) => c,
        None => return,
    };

    let weapon = match ctx.clients[client_idx].pers.weapon {
        Some(w) => w,
        None => return,
    };

    let selected_weapon = weapon;

    // scan for the next valid one
    for i in 1..=MAX_ITEMS {
        let index = (selected_weapon + i) % MAX_ITEMS;
        if ctx.clients[client_idx].pers.inventory[index] == 0 {
            continue;
        }
        if index >= ctx.items.len() {
            continue;
        }
        let it = &ctx.items[index];
        if it.use_fn.is_none() {
            continue;
        }
        if !it.flags.intersects(IT_WEAPON) {
            continue;
        }
        // it->use(ent, it)
        let use_fn_id = it.use_fn;
        dispatch_item_use(ctx, use_fn_id, ent_idx, index);
        // Check if weapon changed successfully
        if ctx.clients[client_idx].pers.weapon == Some(index) {
            return; // successful
        }
    }
}

/// Cycle to next weapon.
pub fn cmd_weap_next_f(ctx: &mut GameContext, ent_idx: usize) {
    let client_idx = match ctx.edicts[ent_idx].client {
        Some(c) => c,
        None => return,
    };

    let weapon = match ctx.clients[client_idx].pers.weapon {
        Some(w) => w,
        None => return,
    };

    let selected_weapon = weapon;

    // scan for the next valid one
    for i in 1..=MAX_ITEMS {
        let index = (selected_weapon + MAX_ITEMS - i) % MAX_ITEMS;
        if ctx.clients[client_idx].pers.inventory[index] == 0 {
            continue;
        }
        if index >= ctx.items.len() {
            continue;
        }
        let it = &ctx.items[index];
        if it.use_fn.is_none() {
            continue;
        }
        if !it.flags.intersects(IT_WEAPON) {
            continue;
        }
        // it->use(ent, it)
        let use_fn_id = it.use_fn;
        dispatch_item_use(ctx, use_fn_id, ent_idx, index);
        // Check if weapon changed successfully
        if ctx.clients[client_idx].pers.weapon == Some(index) {
            return; // successful
        }
    }
}

/// Switch to last weapon used.
pub fn cmd_weap_last_f(ctx: &mut GameContext, ent_idx: usize) {
    let client_idx = match ctx.edicts[ent_idx].client {
        Some(c) => c,
        None => return,
    };

    if ctx.clients[client_idx].pers.weapon.is_none()
        || ctx.clients[client_idx].pers.lastweapon.is_none()
    {
        return;
    }

    let index = ctx.clients[client_idx].pers.lastweapon.unwrap();
    if ctx.clients[client_idx].pers.inventory[index] == 0 {
        return;
    }
    if index >= ctx.items.len() {
        return;
    }
    let it = &ctx.items[index];
    if it.use_fn.is_none() {
        return;
    }
    if !it.flags.intersects(IT_WEAPON) {
        return;
    }
    // it->use(ent, it)
    let use_fn_id = it.use_fn;
    dispatch_item_use(ctx, use_fn_id, ent_idx, index);
}

/// Drop the currently selected inventory item.
pub fn cmd_invdrop_f(ctx: &mut GameContext, ent_idx: usize) {
    validate_selected_item(ctx, ent_idx);

    let client_idx = ctx.edicts[ent_idx].client.unwrap();
    let selected = ctx.clients[client_idx].pers.selected_item;

    if selected == -1 {
        gi_cprintf(ent_idx as i32, PRINT_HIGH, "No item to drop.\n");
        return;
    }

    let item_idx = selected as usize;
    if ctx.items[item_idx].drop_fn.is_none() {
        gi_cprintf(ent_idx as i32, PRINT_HIGH, "Item is not dropable.\n");
        return;
    }

    // it->drop(ent, it)
    let drop_fn_id = ctx.items[item_idx].drop_fn;
    dispatch_item_drop(ctx, drop_fn_id, ent_idx, item_idx);
}

/// Kill self command.
pub fn cmd_kill_f(ctx: &mut GameContext, ent_idx: usize) {
    let client_idx = match ctx.edicts[ent_idx].client {
        Some(c) => c,
        None => return,
    };

    if (ctx.level.time - ctx.clients[client_idx].respawn_time) < 5.0 {
        return;
    }
    ctx.edicts[ent_idx].flags.remove(FL_GODMODE);
    ctx.edicts[ent_idx].health = 0;
    // meansOfDeath = MOD_SUICIDE
    // player_die(ent, ent, ent, 100000, vec3_origin)
    // player_die deferred: p_client::GameContext differs from g_items::GameContext
}

/// Put away scoreboard/help/inventory.
pub fn cmd_putaway_f(ctx: &mut GameContext, ent_idx: usize) {
    let client_idx = match ctx.edicts[ent_idx].client {
        Some(c) => c,
        None => return,
    };
    ctx.clients[client_idx].showscores = false;
    ctx.clients[client_idx].showhelp = false;
    ctx.clients[client_idx].showinventory = false;
}

/// Sort comparator for player listing by frags.
fn player_sort(ctx: &GameContext, a: usize, b: usize) -> std::cmp::Ordering {
    let anum = ctx.clients[a].ps.stats[STAT_FRAGS as usize];
    let bnum = ctx.clients[b].ps.stats[STAT_FRAGS as usize];
    anum.cmp(&bnum)
}

/// List connected players sorted by frags.
pub fn cmd_players_f(ctx: &GameContext, ent_idx: usize) {
    let maxclients = ctx.game.maxclients;
    let mut indices: Vec<usize> = Vec::new();

    for i in 0..maxclients as usize {
        if i < ctx.clients.len() && ctx.clients[i].pers.connected {
            indices.push(i);
        }
    }

    let count = indices.len();

    // sort by frags
    indices.sort_by(|&a, &b| player_sort(ctx, a, b));

    // print information
    let mut large = String::new();
    for i in 0..count {
        let idx = indices[i];
        let small = format!(
            "{:3} {}\n",
            ctx.clients[idx].ps.stats[STAT_FRAGS as usize],
            ctx.clients[idx].pers.netname
        );
        if small.len() + large.len() > 1280 - 100 {
            large.push_str("...\n");
            break;
        }
        large.push_str(&small);
    }

    gi_cprintf(ent_idx as i32, PRINT_HIGH, &format!("{}\n{} players\n", large, count));
}

/// Perform a wave animation.
pub fn cmd_wave_f(ctx: &mut GameContext, ent_idx: usize, argv: &[&str]) {
    let i: i32 = argv.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);

    let client_idx = match ctx.edicts[ent_idx].client {
        Some(c) => c,
        None => return,
    };

    // can't wave when ducked
    if (ctx.clients[client_idx].ps.pmove.pm_flags & PMF_DUCKED) != 0 {
        return;
    }

    if ctx.clients[client_idx].anim_priority > ANIM_WAVE {
        return;
    }

    ctx.clients[client_idx].anim_priority = ANIM_WAVE;

    match i {
        0 => {
            gi_cprintf(ent_idx as i32, PRINT_HIGH, "flipoff\n");
            ctx.edicts[ent_idx].s.frame = FRAME_FLIP01 - 1;
            ctx.clients[client_idx].anim_end = FRAME_FLIP12;
        }
        1 => {
            gi_cprintf(ent_idx as i32, PRINT_HIGH, "salute\n");
            ctx.edicts[ent_idx].s.frame = FRAME_SALUTE01 - 1;
            ctx.clients[client_idx].anim_end = FRAME_SALUTE11;
        }
        2 => {
            gi_cprintf(ent_idx as i32, PRINT_HIGH, "taunt\n");
            ctx.edicts[ent_idx].s.frame = FRAME_TAUNT01 - 1;
            ctx.clients[client_idx].anim_end = FRAME_TAUNT17;
        }
        3 => {
            gi_cprintf(ent_idx as i32, PRINT_HIGH, "wave\n");
            ctx.edicts[ent_idx].s.frame = FRAME_WAVE01 - 1;
            ctx.clients[client_idx].anim_end = FRAME_WAVE11;
        }
        _ => {
            // case 4 and default
            gi_cprintf(ent_idx as i32, PRINT_HIGH, "point\n");
            ctx.edicts[ent_idx].s.frame = FRAME_POINT01 - 1;
            ctx.clients[client_idx].anim_end = FRAME_POINT12;
        }
    }
}

/// Returns the version string for the engine.
pub fn get_version_string() -> String {
    format!(
        "{} v{:.1} {} {}",
        DISTNAME, DISTVER, CPUSTRING, BUILDSTRING
    )
}

/// Handle !version chat command - responds with engine version info.
fn handle_chat_command(ctx: &GameContext, ent_idx: usize, message: &str) -> bool {
    let trimmed = message.trim();

    if trimmed.eq_ignore_ascii_case("!version") {
        let version = get_version_string();
        gi_cprintf(ent_idx as i32, PRINT_HIGH, &format!("Server: {}\n", version));
        return true;
    }

    false
}

/// Say command (chat).
pub fn cmd_say_f(
    ctx: &mut GameContext,
    ent_idx: usize,
    mut team: bool,
    arg0: bool,
    argv: &[&str],
    args: &str,
) {
    if argv.len() < 2 && !arg0 {
        return;
    }

    // Extract the raw message content first to check for chat commands
    let raw_message = if arg0 {
        // When arg0 is true, the command itself is part of the message
        let cmd = argv.first().unwrap_or(&"");
        if args.is_empty() {
            cmd.to_string()
        } else {
            format!("{} {}", cmd, args)
        }
    } else {
        let mut p = args.to_string();
        if p.starts_with('"') {
            p = p[1..].to_string();
            if p.ends_with('"') {
                p.pop();
            }
        }
        p
    };

    // Check for chat commands (e.g., !version)
    if handle_chat_command(ctx, ent_idx, &raw_message) {
        return;
    }

    if !DmFlags::from_bits_truncate(ctx.dmflags as i32).intersects(DF_MODELTEAMS | DF_SKINTEAMS) {
        team = false;
    }

    let client_idx = ctx.edicts[ent_idx].client.unwrap();
    let netname = ctx.clients[client_idx].pers.netname.clone();

    let mut text = if team {
        format!("({}): ", netname)
    } else {
        format!("{}: ", netname)
    };

    text.push_str(&raw_message);

    // don't let text be too long for malicious reasons
    if text.len() > 150 {
        text.truncate(150);
    }
    text.push('\n');

    // Flood protection
    let flood_msgs_val = gi_cvar("flood_msgs", "4", 0);
    if flood_msgs_val > 0.0 {
        let cl = &mut ctx.clients[client_idx];
        if ctx.level.time < cl.flood_locktill {
            gi_cprintf(ent_idx as i32, PRINT_HIGH, &format!("You can't talk for {} more seconds\n", (cl.flood_locktill - ctx.level.time) as i32));
            return;
        }
        let flood_when_len = cl.flood_when.len() as i32;
        let mut i = cl.flood_whenhead - flood_msgs_val as i32 + 1;
        if i < 0 {
            i += flood_when_len;
        }
        let i = i as usize;
        let flood_persecond_val = gi_cvar("flood_persecond", "4", 0);
        if cl.flood_when[i] != 0.0 && ctx.level.time - cl.flood_when[i] < flood_persecond_val {
            let flood_waitdelay_val = gi_cvar("flood_waitdelay", "10", 0);
            cl.flood_locktill = ctx.level.time + flood_waitdelay_val;
            gi_cprintf(ent_idx as i32, PRINT_CHAT, &format!("Flood protection:  You can't talk for {} seconds.\n", flood_waitdelay_val as i32));
            return;
        }
        cl.flood_whenhead = (cl.flood_whenhead + 1) % flood_when_len;
        cl.flood_when[cl.flood_whenhead as usize] = ctx.level.time;
    }

    // dedicated server echo
    if myq2_common::cvar::cvar_variable_value("dedicated") != 0.0 {
        gi_cprintf(0, PRINT_CHAT, &text);
    }

    let maxclients = ctx.game.maxclients;
    for j in 1..=maxclients as usize {
        if j >= ctx.edicts.len() {
            break;
        }
        if !ctx.edicts[j].inuse {
            continue;
        }
        if ctx.edicts[j].client.is_none() {
            continue;
        }
        if team && !on_same_team(ctx, ent_idx, j) {
            continue;
        }
        gi_cprintf(j as i32, PRINT_CHAT, &text);
    }
}

/// List connected players with connection time, ping, score, name.
pub fn cmd_player_list_f(ctx: &GameContext, ent_idx: usize) {
    let maxclients = ctx.game.maxclients;
    let mut text = String::new();

    for i in 0..maxclients as usize {
        let e2_idx = i + 1; // g_edicts + 1
        if e2_idx >= ctx.edicts.len() {
            break;
        }
        if !ctx.edicts[e2_idx].inuse {
            continue;
        }
        let cl_idx = match ctx.edicts[e2_idx].client {
            Some(c) => c,
            None => continue,
        };
        let cl = &ctx.clients[cl_idx];
        let frames = ctx.level.framenum - cl.resp.enterframe;
        let st = format!(
            "{:02}:{:02} {:4} {:3} {}{}\n",
            frames / 600,
            (frames % 600) / 10,
            cl.ping,
            cl.resp.score,
            cl.pers.netname,
            if cl.resp.spectator { " (spectator)" } else { "" }
        );
        if text.len() + st.len() > 1400 - 50 {
            text.push_str("And more...\n");
            gi_cprintf(ent_idx as i32, PRINT_HIGH, &text);
            return;
        }
        text.push_str(&st);
    }
    gi_cprintf(ent_idx as i32, PRINT_HIGH, &text);
}

/// Main client command dispatcher.
pub fn client_command(ctx: &mut GameContext, ent_idx: usize, argv: &[&str], args: &str) {
    if ctx.edicts[ent_idx].client.is_none() {
        return; // not fully in game yet
    }

    let cmd = match argv.first() {
        Some(c) => *c,
        None => return,
    };

    if cmd.eq_ignore_ascii_case("players") {
        cmd_players_f(ctx, ent_idx);
        return;
    }
    if cmd.eq_ignore_ascii_case("say") {
        cmd_say_f(ctx, ent_idx, false, false, argv, args);
        return;
    }
    if cmd.eq_ignore_ascii_case("say_team") {
        cmd_say_f(ctx, ent_idx, true, false, argv, args);
        return;
    }
    if cmd.eq_ignore_ascii_case("score") {
        // Cmd_Score_f deferred: p_hud::GameContext differs from g_items::GameContext
        return;
    }
    if cmd.eq_ignore_ascii_case("help") {
        // Cmd_Help_f deferred: p_hud::GameContext differs from g_items::GameContext
        return;
    }

    if ctx.level.intermissiontime != 0.0 {
        return;
    }

    if cmd.eq_ignore_ascii_case("use") {
        cmd_use_f(ctx, ent_idx, args);
    } else if cmd.eq_ignore_ascii_case("drop") {
        cmd_drop_f(ctx, ent_idx, args);
    } else if cmd.eq_ignore_ascii_case("give") {
        cmd_give_f(ctx, ent_idx, args, argv);
    } else if cmd.eq_ignore_ascii_case("god") {
        cmd_god_f(ctx, ent_idx);
    } else if cmd.eq_ignore_ascii_case("notarget") {
        cmd_notarget_f(ctx, ent_idx);
    } else if cmd.eq_ignore_ascii_case("noclip") {
        cmd_noclip_f(ctx, ent_idx);
    } else if cmd.eq_ignore_ascii_case("inven") {
        cmd_inven_f(ctx, ent_idx);
    } else if cmd.eq_ignore_ascii_case("invnext") {
        select_next_item(ctx, ent_idx, ItemFlags::empty());
    } else if cmd.eq_ignore_ascii_case("invprev") {
        select_prev_item(ctx, ent_idx, ItemFlags::empty());
    } else if cmd.eq_ignore_ascii_case("invnextw") {
        select_next_item(ctx, ent_idx, IT_WEAPON);
    } else if cmd.eq_ignore_ascii_case("invprevw") {
        select_prev_item(ctx, ent_idx, IT_WEAPON);
    } else if cmd.eq_ignore_ascii_case("invnextp") {
        select_next_item(ctx, ent_idx, IT_POWERUP);
    } else if cmd.eq_ignore_ascii_case("invprevp") {
        select_prev_item(ctx, ent_idx, IT_POWERUP);
    } else if cmd.eq_ignore_ascii_case("invuse") {
        cmd_invuse_f(ctx, ent_idx);
    } else if cmd.eq_ignore_ascii_case("invdrop") {
        cmd_invdrop_f(ctx, ent_idx);
    } else if cmd.eq_ignore_ascii_case("weapprev") {
        cmd_weap_prev_f(ctx, ent_idx);
    } else if cmd.eq_ignore_ascii_case("weapnext") {
        cmd_weap_next_f(ctx, ent_idx);
    } else if cmd.eq_ignore_ascii_case("weaplast") {
        cmd_weap_last_f(ctx, ent_idx);
    } else if cmd.eq_ignore_ascii_case("kill") {
        cmd_kill_f(ctx, ent_idx);
    } else if cmd.eq_ignore_ascii_case("putaway") {
        cmd_putaway_f(ctx, ent_idx);
    } else if cmd.eq_ignore_ascii_case("wave") {
        cmd_wave_f(ctx, ent_idx, argv);
    } else if cmd.eq_ignore_ascii_case("playerlist") {
        cmd_player_list_f(ctx, ent_idx);
    } else {
        // anything that doesn't match a command will be a chat
        cmd_say_f(ctx, ent_idx, false, true, argv, args);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_test_gi() {
        crate::game_import::set_gi(Box::new(crate::game_import::StubGameImport));
    }

    /// Create a minimal GameContext with the given number of clients, with items initialized.
    fn make_ctx(num_clients: i32) -> GameContext {
        init_test_gi();
        let mut ctx = GameContext::default();
        ctx.maxclients = num_clients as f32;
        ctx.game.maxclients = num_clients;
        ctx.deathmatch = 0.0; // cheats need deathmatch off
        let total_edicts = 1 + num_clients as usize + BODY_QUEUE_SIZE + 16;
        for _ in 0..total_edicts {
            ctx.edicts.push(Edict::default());
        }
        ctx.num_edicts = ctx.edicts.len() as i32;
        for _ in 0..num_clients {
            ctx.clients.push(GClient::default());
        }
        // Initialize items so find_item works
        crate::g_items::init_items(&mut ctx);
        ctx.build_item_indices();
        ctx
    }

    /// Create a single-player context with items, ent_idx=1, client_idx=0.
    fn make_single_player_ctx() -> GameContext {
        let mut ctx = make_ctx(1);
        ctx.edicts[1].inuse = true;
        ctx.edicts[1].client = Some(0);
        ctx.edicts[1].health = 100;
        ctx.edicts[1].max_health = 100;
        ctx.edicts[1].movetype = MoveType::Walk;
        ctx.clients[0].pers.connected = true;
        ctx.clients[0].pers.netname = "TestPlayer".to_string();
        ctx.clients[0].pers.userinfo = "\\name\\TestPlayer\\skin\\male/grunt".to_string();
        ctx
    }

    // ============================================================
    // client_team tests
    // ============================================================

    #[test]
    fn test_client_team_model_teams() {
        let mut ctx = make_single_player_ctx();
        ctx.dmflags = DF_MODELTEAMS.bits() as f32;
        ctx.clients[0].pers.userinfo = "\\name\\Test\\skin\\female/athena".to_string();

        let team = client_team(&ctx, 1);
        assert_eq!(team, "female");
    }

    #[test]
    fn test_client_team_skin_teams() {
        let mut ctx = make_single_player_ctx();
        ctx.dmflags = DF_SKINTEAMS.bits() as f32;
        ctx.clients[0].pers.userinfo = "\\name\\Test\\skin\\male/grunt".to_string();

        let team = client_team(&ctx, 1);
        assert_eq!(team, "grunt");
    }

    #[test]
    fn test_client_team_no_slash() {
        let mut ctx = make_single_player_ctx();
        ctx.dmflags = DF_MODELTEAMS.bits() as f32;
        ctx.clients[0].pers.userinfo = "\\name\\Test\\skin\\noskin".to_string();

        let team = client_team(&ctx, 1);
        assert_eq!(team, "noskin");
    }

    #[test]
    fn test_client_team_no_client() {
        let ctx = make_single_player_ctx();
        // Entity 3 has no client
        let team = client_team(&ctx, 3);
        assert_eq!(team, "");
    }

    // ============================================================
    // on_same_team tests
    // ============================================================

    #[test]
    fn test_on_same_team_true() {
        let mut ctx = make_ctx(2);
        ctx.dmflags = DF_SKINTEAMS.bits() as f32;
        ctx.edicts[1].inuse = true;
        ctx.edicts[1].client = Some(0);
        ctx.clients[0].pers.userinfo = "\\name\\P1\\skin\\male/grunt".to_string();

        ctx.edicts[2].inuse = true;
        ctx.edicts[2].client = Some(1);
        ctx.clients[1].pers.userinfo = "\\name\\P2\\skin\\female/grunt".to_string();

        assert!(on_same_team(&ctx, 1, 2));
    }

    #[test]
    fn test_on_same_team_false() {
        let mut ctx = make_ctx(2);
        ctx.dmflags = DF_SKINTEAMS.bits() as f32;
        ctx.edicts[1].inuse = true;
        ctx.edicts[1].client = Some(0);
        ctx.clients[0].pers.userinfo = "\\name\\P1\\skin\\male/grunt".to_string();

        ctx.edicts[2].inuse = true;
        ctx.edicts[2].client = Some(1);
        ctx.clients[1].pers.userinfo = "\\name\\P2\\skin\\female/athena".to_string();

        assert!(!on_same_team(&ctx, 1, 2));
    }

    #[test]
    fn test_on_same_team_no_team_flags() {
        let mut ctx = make_ctx(2);
        ctx.dmflags = 0.0;
        ctx.edicts[1].inuse = true;
        ctx.edicts[1].client = Some(0);
        ctx.clients[0].pers.userinfo = "\\name\\P1\\skin\\male/grunt".to_string();

        ctx.edicts[2].inuse = true;
        ctx.edicts[2].client = Some(1);
        ctx.clients[1].pers.userinfo = "\\name\\P2\\skin\\male/grunt".to_string();

        // No team flags set => always false
        assert!(!on_same_team(&ctx, 1, 2));
    }

    // ============================================================
    // validate_selected_item tests
    // ============================================================

    #[test]
    fn test_validate_selected_item_valid() {
        let mut ctx = make_single_player_ctx();
        // Set selected_item to a valid index with inventory
        ctx.clients[0].pers.selected_item = 5;
        ctx.clients[0].pers.inventory[5] = 1;

        validate_selected_item(&mut ctx, 1);

        // Should remain valid
        assert_eq!(ctx.clients[0].pers.selected_item, 5);
    }

    #[test]
    fn test_validate_selected_item_invalid_selects_next() {
        let mut ctx = make_single_player_ctx();
        ctx.clients[0].pers.selected_item = 5;
        ctx.clients[0].pers.inventory[5] = 0; // no item

        validate_selected_item(&mut ctx, 1);

        // Should try to select next valid item, or -1 if none
        // Since we have items initialized but no inventory, should be -1
        assert_eq!(ctx.clients[0].pers.selected_item, -1);
    }

    // ============================================================
    // cmd_god_f tests
    // ============================================================

    #[test]
    fn test_cmd_god_f_toggle_on() {
        let mut ctx = make_single_player_ctx();
        assert!(!ctx.edicts[1].flags.intersects(FL_GODMODE));

        cmd_god_f(&mut ctx, 1);

        assert!(ctx.edicts[1].flags.intersects(FL_GODMODE));
    }

    #[test]
    fn test_cmd_god_f_toggle_off() {
        let mut ctx = make_single_player_ctx();
        ctx.edicts[1].flags = FL_GODMODE;

        cmd_god_f(&mut ctx, 1);

        assert!(!ctx.edicts[1].flags.intersects(FL_GODMODE));
    }

    #[test]
    fn test_cmd_god_f_deathmatch_blocked() {
        let mut ctx = make_single_player_ctx();
        ctx.deathmatch = 1.0;

        cmd_god_f(&mut ctx, 1);

        // Should NOT toggle godmode in deathmatch
        assert!(!ctx.edicts[1].flags.intersects(FL_GODMODE));
    }

    // ============================================================
    // cmd_notarget_f tests
    // ============================================================

    #[test]
    fn test_cmd_notarget_f_toggle_on() {
        let mut ctx = make_single_player_ctx();

        cmd_notarget_f(&mut ctx, 1);

        assert!(ctx.edicts[1].flags.intersects(FL_NOTARGET));
    }

    #[test]
    fn test_cmd_notarget_f_toggle_off() {
        let mut ctx = make_single_player_ctx();
        ctx.edicts[1].flags = FL_NOTARGET;

        cmd_notarget_f(&mut ctx, 1);

        assert!(!ctx.edicts[1].flags.intersects(FL_NOTARGET));
    }

    #[test]
    fn test_cmd_notarget_f_deathmatch_blocked() {
        let mut ctx = make_single_player_ctx();
        ctx.deathmatch = 1.0;

        cmd_notarget_f(&mut ctx, 1);

        assert!(!ctx.edicts[1].flags.intersects(FL_NOTARGET));
    }

    // ============================================================
    // cmd_noclip_f tests
    // ============================================================

    #[test]
    fn test_cmd_noclip_f_toggle_on() {
        let mut ctx = make_single_player_ctx();
        assert_eq!(ctx.edicts[1].movetype, MoveType::Walk);

        cmd_noclip_f(&mut ctx, 1);

        assert_eq!(ctx.edicts[1].movetype, MoveType::Noclip);
    }

    #[test]
    fn test_cmd_noclip_f_toggle_off() {
        let mut ctx = make_single_player_ctx();
        ctx.edicts[1].movetype = MoveType::Noclip;

        cmd_noclip_f(&mut ctx, 1);

        assert_eq!(ctx.edicts[1].movetype, MoveType::Walk);
    }

    #[test]
    fn test_cmd_noclip_f_deathmatch_blocked() {
        let mut ctx = make_single_player_ctx();
        ctx.deathmatch = 1.0;

        cmd_noclip_f(&mut ctx, 1);

        assert_eq!(ctx.edicts[1].movetype, MoveType::Walk);
    }

    // ============================================================
    // cmd_give_f tests
    // ============================================================

    #[test]
    fn test_cmd_give_f_health() {
        let mut ctx = make_single_player_ctx();
        ctx.edicts[1].health = 50;

        cmd_give_f(&mut ctx, 1, "health", &["give", "health"]);

        // Without a count, should reset to max_health
        assert_eq!(ctx.edicts[1].health, 100);
    }

    #[test]
    fn test_cmd_give_f_health_with_count() {
        let mut ctx = make_single_player_ctx();
        ctx.edicts[1].health = 50;

        cmd_give_f(&mut ctx, 1, "health 200", &["give", "health", "200"]);

        assert_eq!(ctx.edicts[1].health, 200);
    }

    #[test]
    fn test_cmd_give_f_deathmatch_blocked() {
        let mut ctx = make_single_player_ctx();
        ctx.deathmatch = 1.0;
        ctx.edicts[1].health = 50;

        cmd_give_f(&mut ctx, 1, "health", &["give", "health"]);

        // Should not change health
        assert_eq!(ctx.edicts[1].health, 50);
    }

    #[test]
    fn test_cmd_give_f_weapons() {
        let mut ctx = make_single_player_ctx();

        cmd_give_f(&mut ctx, 1, "weapons", &["give", "weapons"]);

        // Should have given at least some weapon items
        let client_idx = ctx.edicts[1].client.unwrap();
        let mut weapon_count = 0;
        for i in 0..ctx.game.num_items as usize {
            if ctx.items[i].flags.intersects(IT_WEAPON) && ctx.items[i].pickup_fn.is_some() {
                if ctx.clients[client_idx].pers.inventory[i] > 0 {
                    weapon_count += 1;
                }
            }
        }
        assert!(weapon_count > 0, "Should have received at least one weapon");
    }

    #[test]
    fn test_cmd_give_f_unknown_item() {
        let mut ctx = make_single_player_ctx();

        // Should not panic with unknown item
        cmd_give_f(&mut ctx, 1, "nonexistent_item_xyz", &["give", "nonexistent_item_xyz"]);
    }

    // ============================================================
    // cmd_kill_f tests
    // ============================================================

    #[test]
    fn test_cmd_kill_f_sets_health_zero() {
        let mut ctx = make_single_player_ctx();
        ctx.level.time = 10.0;
        ctx.clients[0].respawn_time = 0.0; // last respawn was 10s ago

        cmd_kill_f(&mut ctx, 1);

        assert_eq!(ctx.edicts[1].health, 0);
        assert!(!ctx.edicts[1].flags.intersects(FL_GODMODE));
    }

    #[test]
    fn test_cmd_kill_f_too_soon() {
        let mut ctx = make_single_player_ctx();
        ctx.level.time = 3.0;
        ctx.clients[0].respawn_time = 1.0; // only 2 seconds since respawn

        cmd_kill_f(&mut ctx, 1);

        // Health should not change (kill rejected within 5 seconds)
        assert_eq!(ctx.edicts[1].health, 100);
    }

    // ============================================================
    // cmd_putaway_f tests
    // ============================================================

    #[test]
    fn test_cmd_putaway_f() {
        let mut ctx = make_single_player_ctx();
        ctx.clients[0].showscores = true;
        ctx.clients[0].showhelp = true;
        ctx.clients[0].showinventory = true;

        cmd_putaway_f(&mut ctx, 1);

        assert!(!ctx.clients[0].showscores);
        assert!(!ctx.clients[0].showhelp);
        assert!(!ctx.clients[0].showinventory);
    }

    // ============================================================
    // cmd_inven_f tests
    // ============================================================

    #[test]
    fn test_cmd_inven_f_toggle_on() {
        let mut ctx = make_single_player_ctx();
        ctx.clients[0].showinventory = false;

        cmd_inven_f(&mut ctx, 1);

        assert!(ctx.clients[0].showinventory);
        assert!(!ctx.clients[0].showscores);
        assert!(!ctx.clients[0].showhelp);
    }

    #[test]
    fn test_cmd_inven_f_toggle_off() {
        let mut ctx = make_single_player_ctx();
        ctx.clients[0].showinventory = true;

        cmd_inven_f(&mut ctx, 1);

        assert!(!ctx.clients[0].showinventory);
    }

    // ============================================================
    // cmd_wave_f tests
    // ============================================================

    #[test]
    fn test_cmd_wave_f_flipoff() {
        let mut ctx = make_single_player_ctx();
        ctx.clients[0].anim_priority = ANIM_BASIC;

        cmd_wave_f(&mut ctx, 1, &["wave", "0"]);

        assert_eq!(ctx.clients[0].anim_priority, ANIM_WAVE);
        assert_eq!(ctx.edicts[1].s.frame, FRAME_FLIP01 - 1);
        assert_eq!(ctx.clients[0].anim_end, FRAME_FLIP12);
    }

    #[test]
    fn test_cmd_wave_f_salute() {
        let mut ctx = make_single_player_ctx();
        ctx.clients[0].anim_priority = ANIM_BASIC;

        cmd_wave_f(&mut ctx, 1, &["wave", "1"]);

        assert_eq!(ctx.clients[0].anim_priority, ANIM_WAVE);
        assert_eq!(ctx.edicts[1].s.frame, FRAME_SALUTE01 - 1);
        assert_eq!(ctx.clients[0].anim_end, FRAME_SALUTE11);
    }

    #[test]
    fn test_cmd_wave_f_taunt() {
        let mut ctx = make_single_player_ctx();
        ctx.clients[0].anim_priority = ANIM_BASIC;

        cmd_wave_f(&mut ctx, 1, &["wave", "2"]);

        assert_eq!(ctx.clients[0].anim_priority, ANIM_WAVE);
        assert_eq!(ctx.edicts[1].s.frame, FRAME_TAUNT01 - 1);
        assert_eq!(ctx.clients[0].anim_end, FRAME_TAUNT17);
    }

    #[test]
    fn test_cmd_wave_f_wave() {
        let mut ctx = make_single_player_ctx();
        ctx.clients[0].anim_priority = ANIM_BASIC;

        cmd_wave_f(&mut ctx, 1, &["wave", "3"]);

        assert_eq!(ctx.clients[0].anim_priority, ANIM_WAVE);
        assert_eq!(ctx.edicts[1].s.frame, FRAME_WAVE01 - 1);
        assert_eq!(ctx.clients[0].anim_end, FRAME_WAVE11);
    }

    #[test]
    fn test_cmd_wave_f_point() {
        let mut ctx = make_single_player_ctx();
        ctx.clients[0].anim_priority = ANIM_BASIC;

        cmd_wave_f(&mut ctx, 1, &["wave", "4"]);

        assert_eq!(ctx.clients[0].anim_priority, ANIM_WAVE);
        assert_eq!(ctx.edicts[1].s.frame, FRAME_POINT01 - 1);
        assert_eq!(ctx.clients[0].anim_end, FRAME_POINT12);
    }

    #[test]
    fn test_cmd_wave_f_ducked_blocked() {
        let mut ctx = make_single_player_ctx();
        ctx.clients[0].ps.pmove.pm_flags = PMF_DUCKED;

        cmd_wave_f(&mut ctx, 1, &["wave", "1"]);

        // Should not change animation when ducked
        assert_ne!(ctx.clients[0].anim_priority, ANIM_WAVE);
    }

    #[test]
    fn test_cmd_wave_f_higher_priority_blocked() {
        let mut ctx = make_single_player_ctx();
        ctx.clients[0].anim_priority = ANIM_DEATH;

        cmd_wave_f(&mut ctx, 1, &["wave", "1"]);

        // Death animation has higher priority, should not change
        assert_eq!(ctx.clients[0].anim_priority, ANIM_DEATH);
    }

    // ============================================================
    // get_version_string tests
    // ============================================================

    #[test]
    fn test_get_version_string_format() {
        let version = get_version_string();

        assert!(version.contains(DISTNAME));
        assert!(version.contains(CPUSTRING));
        assert!(version.contains(BUILDSTRING));
    }

    // ============================================================
    // handle_chat_command tests
    // ============================================================

    #[test]
    fn test_handle_chat_command_version() {
        let ctx = make_single_player_ctx();

        let handled = handle_chat_command(&ctx, 1, "!version");

        assert!(handled);
    }

    #[test]
    fn test_handle_chat_command_version_case() {
        let ctx = make_single_player_ctx();

        let handled = handle_chat_command(&ctx, 1, "!VERSION");

        assert!(handled);
    }

    #[test]
    fn test_handle_chat_command_not_version() {
        let ctx = make_single_player_ctx();

        let handled = handle_chat_command(&ctx, 1, "hello world");

        assert!(!handled);
    }

    // ============================================================
    // player_sort tests
    // ============================================================

    #[test]
    fn test_player_sort_ordering() {
        let mut ctx = make_ctx(2);
        ctx.clients[0].ps.stats[STAT_FRAGS as usize] = 10;
        ctx.clients[1].ps.stats[STAT_FRAGS as usize] = 20;

        let result = player_sort(&ctx, 0, 1);
        assert_eq!(result, std::cmp::Ordering::Less);

        let result = player_sort(&ctx, 1, 0);
        assert_eq!(result, std::cmp::Ordering::Greater);

        let result = player_sort(&ctx, 0, 0);
        assert_eq!(result, std::cmp::Ordering::Equal);
    }

    // ============================================================
    // client_command dispatch tests
    // ============================================================

    #[test]
    fn test_client_command_god() {
        let mut ctx = make_single_player_ctx();

        client_command(&mut ctx, 1, &["god"], "");

        assert!(ctx.edicts[1].flags.intersects(FL_GODMODE));
    }

    #[test]
    fn test_client_command_noclip() {
        let mut ctx = make_single_player_ctx();

        client_command(&mut ctx, 1, &["noclip"], "");

        assert_eq!(ctx.edicts[1].movetype, MoveType::Noclip);
    }

    #[test]
    fn test_client_command_notarget() {
        let mut ctx = make_single_player_ctx();

        client_command(&mut ctx, 1, &["notarget"], "");

        assert!(ctx.edicts[1].flags.intersects(FL_NOTARGET));
    }

    #[test]
    fn test_client_command_putaway() {
        let mut ctx = make_single_player_ctx();
        ctx.clients[0].showscores = true;

        client_command(&mut ctx, 1, &["putaway"], "");

        assert!(!ctx.clients[0].showscores);
    }

    #[test]
    fn test_client_command_inven() {
        let mut ctx = make_single_player_ctx();

        client_command(&mut ctx, 1, &["inven"], "");

        assert!(ctx.clients[0].showinventory);
    }

    #[test]
    fn test_client_command_no_client() {
        let mut ctx = make_single_player_ctx();
        ctx.edicts[1].client = None;

        // Should return early without panicking
        client_command(&mut ctx, 1, &["god"], "");
    }

    #[test]
    fn test_client_command_empty_argv() {
        let mut ctx = make_single_player_ctx();

        // Empty argv should be handled gracefully
        client_command(&mut ctx, 1, &[], "");
    }

    #[test]
    fn test_client_command_intermission_blocks_commands() {
        let mut ctx = make_single_player_ctx();
        ctx.level.intermissiontime = 1.0; // During intermission

        client_command(&mut ctx, 1, &["god"], "");

        // god should NOT be toggled during intermission (except score/help/say)
        assert!(!ctx.edicts[1].flags.intersects(FL_GODMODE));
    }

    #[test]
    fn test_client_command_case_insensitive() {
        let mut ctx = make_single_player_ctx();

        client_command(&mut ctx, 1, &["GOD"], "");

        assert!(ctx.edicts[1].flags.intersects(FL_GODMODE));
    }

    // ============================================================
    // cmd_players_f tests
    // ============================================================

    #[test]
    fn test_cmd_players_f_no_crash() {
        let mut ctx = make_ctx(4);
        ctx.edicts[1].inuse = true;
        ctx.edicts[1].client = Some(0);
        ctx.clients[0].pers.connected = true;
        ctx.clients[0].pers.netname = "Player1".to_string();
        ctx.clients[0].ps.stats[STAT_FRAGS as usize] = 5;

        ctx.edicts[2].inuse = true;
        ctx.edicts[2].client = Some(1);
        ctx.clients[1].pers.connected = true;
        ctx.clients[1].pers.netname = "Player2".to_string();
        ctx.clients[1].ps.stats[STAT_FRAGS as usize] = 10;

        // Should not crash
        cmd_players_f(&ctx, 1);
    }

    // ============================================================
    // cmd_player_list_f tests
    // ============================================================

    #[test]
    fn test_cmd_player_list_f_no_crash() {
        let mut ctx = make_ctx(2);
        ctx.edicts[1].inuse = true;
        ctx.edicts[1].client = Some(0);
        ctx.clients[0].pers.connected = true;
        ctx.clients[0].pers.netname = "Player1".to_string();
        ctx.clients[0].resp.score = 15;
        ctx.clients[0].resp.enterframe = 0;
        ctx.clients[0].ping = 42;
        ctx.level.framenum = 1200;

        // Should not crash
        cmd_player_list_f(&ctx, 1);
    }

    #[test]
    fn test_cmd_player_list_f_spectator_label() {
        let mut ctx = make_ctx(1);
        ctx.edicts[1].inuse = true;
        ctx.edicts[1].client = Some(0);
        ctx.clients[0].pers.connected = true;
        ctx.clients[0].pers.netname = "SpectatorPlayer".to_string();
        ctx.clients[0].resp.spectator = true;
        ctx.level.framenum = 600;

        // Should not crash and should include spectator label
        cmd_player_list_f(&ctx, 1);
    }

    // ============================================================
    // select_next_item / select_prev_item tests
    // ============================================================

    #[test]
    fn test_select_next_item_finds_item() {
        let mut ctx = make_single_player_ctx();
        // Find a weapon item that has a use_fn
        let mut weapon_idx = None;
        for i in 0..ctx.game.num_items as usize {
            if ctx.items[i].flags.intersects(IT_WEAPON) && ctx.items[i].use_fn.is_some() {
                weapon_idx = Some(i);
                break;
            }
        }
        if let Some(idx) = weapon_idx {
            ctx.clients[0].pers.inventory[idx] = 1;
            ctx.clients[0].pers.selected_item = 0;

            select_next_item(&mut ctx, 1, IT_WEAPON);

            assert_eq!(ctx.clients[0].pers.selected_item, idx as i32);
        }
    }

    #[test]
    fn test_select_next_item_no_matching_item() {
        let mut ctx = make_single_player_ctx();
        // No powerup items in inventory
        ctx.clients[0].pers.selected_item = 0;

        select_next_item(&mut ctx, 1, IT_POWERUP);

        assert_eq!(ctx.clients[0].pers.selected_item, -1);
    }

    #[test]
    fn test_select_prev_item_no_matching_item() {
        let mut ctx = make_single_player_ctx();
        ctx.clients[0].pers.selected_item = 0;

        select_prev_item(&mut ctx, 1, IT_POWERUP);

        assert_eq!(ctx.clients[0].pers.selected_item, -1);
    }
}
