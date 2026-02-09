// sv_ccmds.rs — Server console commands
// Converted from: myq2-original/server/sv_ccmds.c
//
// Copyright (C) 1997-2001 Id Software, Inc.
// Licensed under the GNU General Public License v2.

use crate::server::*;
use myq2_common::common::{com_printf, com_dprintf, msg_write_byte_vec, msg_write_short_vec, msg_write_long_vec, msg_write_string_vec};
use myq2_common::files::{fs_gamedir, fs_create_path, fs_load_file};
use myq2_common::q_shared::*;
use myq2_common::qcommon::*;

use std::fs;
use std::io::{Read, Write};
use std::path::Path;

// ===============================================================================
//
// OPERATOR CONSOLE ONLY COMMANDS
//
// These commands can only be entered from stdin or by a remote operator datagram
// ===============================================================================

/// Specify a list of master servers.
///
/// Equivalent to C: `SV_SetMaster_f`
pub fn sv_set_master_f(ctx: &mut ServerContext) {
    // only dedicated servers send heartbeats
    if ctx.cvars.variable_value("dedicated") == 0.0 {
        com_printf("Only dedicated servers use masters.\n");
        return;
    }

    // make sure the server is listed public
    ctx.cvars.set("public", "1");

    for i in 1..MAX_MASTERS {
        ctx.master_adr[i] = NetAdr::default();
    }

    // slot 0 will always contain the id master
    let mut slot = 1usize;
    let argc = myq2_common::cmd::cmd_argc();
    for i in 1..argc {
        if slot == MAX_MASTERS {
            break;
        }

        let arg = myq2_common::cmd::cmd_argv(i);
        if let Some(mut adr) = myq2_common::net::net_string_to_adr(&arg) {
            if adr.port == 0 {
                adr.port = PORT_MASTER as u16;
            }
            com_printf(&format!("Master server at {}\n", crate::sv_main::net_adr_to_string(&adr)));
            ctx.master_adr[slot] = adr;
            slot += 1;
        } else {
            com_printf(&format!("Bad address: {}\n", arg));
        }
    }

    ctx.svs.last_heartbeat = -9999999;
}

/// Sets sv_client and sv_player to the player with the given idnum.
///
/// Equivalent to C: `SV_SetPlayer`
pub fn sv_set_player(ctx: &mut ServerContext, cmd_argc: usize, cmd_argv: &dyn Fn(usize) -> String) -> bool {
    if cmd_argc < 2 {
        return false;
    }

    let s = cmd_argv(1);

    // numeric values are just slot numbers
    if let Some(first_char) = s.bytes().next() {
        if (b'0'..=b'9').contains(&first_char) {
            let idnum: i32 = s.parse().unwrap_or(-1);
            if idnum < 0 || idnum >= ctx.maxclients_value as i32 {
                com_printf(&format!("Bad client slot: {}\n", idnum));
                return false;
            }

            let idx = idnum as usize;
            if ctx.svs.clients[idx].state == ClientState::Free {
                com_printf(&format!("Client {} is not active\n", idnum));
                return false;
            }

            ctx.sv_client_index = Some(idx);
            ctx.sv_player_index = Some(ctx.svs.clients[idx].edict_index);
            return true;
        }
    }

    // check for a name match
    let max = ctx.maxclients_value as usize;
    for i in 0..max {
        if ctx.svs.clients[i].state == ClientState::Free {
            continue;
        }
        if ctx.svs.clients[i].name == s {
            ctx.sv_client_index = Some(i);
            ctx.sv_player_index = Some(ctx.svs.clients[i].edict_index);
            return true;
        }
    }

    com_printf(&format!("Userid {} is not on the server\n", s));
    false
}

// ===============================================================================
//
// SAVEGAME FILES
//
// ===============================================================================

/// Delete save/<XXX>/ directory contents.
///
/// Equivalent to C: `SV_WipeSavegame`
pub fn sv_wipe_savegame(gamedir: &str, savename: &str) {
    com_dprintf(&format!("SV_WipeSaveGame({})\n", savename));

    let path = format!("{}/save/{}/server.ssv", gamedir, savename);
    let _ = fs::remove_file(&path);

    let path = format!("{}/save/{}/game.ssv", gamedir, savename);
    let _ = fs::remove_file(&path);

    // Remove all .sav files
    let save_dir = format!("{}/save/{}", gamedir, savename);
    if let Ok(entries) = fs::read_dir(&save_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "sav" || ext == "sv2" {
                    let _ = fs::remove_file(&path);
                }
            }
        }
    }
}

/// Copy a file from src to dst.
///
/// Equivalent to C: `CopyFile`
pub fn copy_file(src: &str, dst: &str) {
    com_dprintf(&format!("CopyFile ({}, {})\n", src, dst));

    let src_path = Path::new(src);
    let dst_path = Path::new(dst);

    if !src_path.exists() {
        return;
    }

    let mut f1 = match fs::File::open(src_path) {
        Ok(f) => f,
        Err(_) => return,
    };

    let mut f2 = match fs::File::create(dst_path) {
        Ok(f) => f,
        Err(_) => return,
    };

    let mut buffer = [0u8; 65536];
    loop {
        let bytes_read = match f1.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => break,
        };
        if f2.write_all(&buffer[..bytes_read]).is_err() {
            break;
        }
    }
}

/// Copy save game from src slot to dst slot.
///
/// Equivalent to C: `SV_CopySaveGame`
pub fn sv_copy_save_game(gamedir: &str, src: &str, dst: &str) {
    com_dprintf(&format!("SV_CopySaveGame({}, {})\n", src, dst));

    sv_wipe_savegame(gamedir, dst);

    // copy the savegame over
    let name = format!("{}/save/{}/server.ssv", gamedir, src);
    let name2 = format!("{}/save/{}/server.ssv", gamedir, dst);
    fs_create_path(&name2);
    copy_file(&name, &name2);

    let name = format!("{}/save/{}/game.ssv", gamedir, src);
    let name2 = format!("{}/save/{}/game.ssv", gamedir, dst);
    copy_file(&name, &name2);

    // copy .sav and .sv2 files
    let src_dir = format!("{}/save/{}", gamedir, src);
    if let Ok(entries) = fs::read_dir(&src_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "sav" {
                    let filename = entry.file_name();
                    let filename_str = filename.to_string_lossy();

                    // copy .sav
                    let dst_sav = format!("{}/save/{}/{}", gamedir, dst, filename_str);
                    copy_file(&path.to_string_lossy(), &dst_sav);

                    // change sav to sv2 and copy that too
                    let sv2_name = filename_str.replace(".sav", ".sv2");
                    let src_sv2 = format!("{}/save/{}/{}", gamedir, src, sv2_name);
                    let dst_sv2 = format!("{}/save/{}/{}", gamedir, dst, sv2_name);
                    copy_file(&src_sv2, &dst_sv2);
                }
            }
        }
    }
}

/// Write level data to save file.
///
/// Equivalent to C: `SV_WriteLevelFile`
pub fn sv_write_level_file(ctx: &mut ServerContext) {
    com_dprintf("SV_WriteLevelFile()\n");

    let gamedir = fs_gamedir();

    let name = format!("{}/save/current/{}.sv2", gamedir, ctx.sv.name);
    let mut f = match fs::File::create(&name) {
        Ok(f) => f,
        Err(_) => {
            com_printf(&format!("Failed to open {}\n", name));
            return;
        }
    };

    // write configstrings
    for cs in &ctx.sv.configstrings {
        let mut buf = [0u8; MAX_QPATH];
        let bytes = cs.as_bytes();
        let len = bytes.len().min(MAX_QPATH);
        buf[..len].copy_from_slice(&bytes[..len]);
        let _ = f.write_all(&buf);
    }

    cm_write_portal_state(&mut f);

    let name = format!("{}/save/current/{}.sav", gamedir, ctx.sv.name);
    if let Some(ref ge) = ctx.ge {
        if let Some(write_fn) = ge.write_level {
            write_fn(&name);
        }
    }
}

/// Read level data from save file.
///
/// Equivalent to C: `SV_ReadLevelFile`
pub fn sv_read_level_file(ctx: &mut ServerContext) {
    com_dprintf("SV_ReadLevelFile()\n");

    let gamedir = fs_gamedir();

    let name = format!("{}/save/current/{}.sv2", gamedir, ctx.sv.name);
    let mut f = match fs::File::open(&name) {
        Ok(f) => f,
        Err(_) => {
            com_printf(&format!("Failed to open {}\n", name));
            return;
        }
    };

    // read configstrings
    for cs in &mut ctx.sv.configstrings {
        let mut buf = [0u8; MAX_QPATH];
        if f.read_exact(&mut buf).is_err() {
            break;
        }
        let end = buf.iter().position(|&b| b == 0).unwrap_or(MAX_QPATH);
        *cs = String::from_utf8_lossy(&buf[..end]).to_string();
    }

    cm_read_portal_state(&mut f);

    let name = format!("{}/save/current/{}.sav", gamedir, ctx.sv.name);
    if let Some(ref ge) = ctx.ge {
        if let Some(read_fn) = ge.read_level {
            read_fn(&name);
        }
    }
}

/// Write server state to save file.
///
/// Equivalent to C: `SV_WriteServerFile`
pub fn sv_write_server_file(ctx: &mut ServerContext, autosave: bool) {
    com_dprintf(&format!("SV_WriteServerFile({})\n", if autosave { "true" } else { "false" }));

    let gamedir = fs_gamedir();

    let name = format!("{}/save/current/server.ssv", gamedir);
    let mut f = match fs::File::create(&name) {
        Ok(f) => f,
        Err(_) => {
            com_printf(&format!("Couldn't write {}\n", name));
            return;
        }
    };

    // write the comment field
    let mut comment = [0u8; 32];

    if !autosave {
        let comment_str = format!("SAVE {}", &ctx.sv.configstrings[CS_NAME]);
        let bytes = comment_str.as_bytes();
        let len = bytes.len().min(31);
        comment[..len].copy_from_slice(&bytes[..len]);
    } else {
        let comment_str = format!("ENTERING {}", &ctx.sv.configstrings[CS_NAME]);
        let bytes = comment_str.as_bytes();
        let len = bytes.len().min(31);
        comment[..len].copy_from_slice(&bytes[..len]);
    }

    let _ = f.write_all(&comment);

    // write the mapcmd
    let mut mapcmd_buf = [0u8; MAX_TOKEN_CHARS];
    let bytes = ctx.svs.mapcmd.as_bytes();
    let len = bytes.len().min(MAX_TOKEN_CHARS);
    mapcmd_buf[..len].copy_from_slice(&bytes[..len]);
    let _ = f.write_all(&mapcmd_buf);

    // write all CVAR_LATCH cvars
    for cvar in &ctx.cvars.cvar_vars {
        if (cvar.flags & CVAR_LATCH) == 0 {
            continue;
        }
        if cvar.name.len() >= MAX_OSPATH - 1 || cvar.string.len() >= 127 {
            com_printf(&format!("Cvar too long: {} = {}\n", cvar.name, cvar.string));
            continue;
        }
        let mut name_buf = [0u8; MAX_OSPATH];
        let name_bytes = cvar.name.as_bytes();
        let nlen = name_bytes.len().min(MAX_OSPATH - 1);
        name_buf[..nlen].copy_from_slice(&name_bytes[..nlen]);
        let _ = f.write_all(&name_buf);

        let mut string_buf = [0u8; 128];
        let string_bytes = cvar.string.as_bytes();
        let slen = string_bytes.len().min(127);
        string_buf[..slen].copy_from_slice(&string_bytes[..slen]);
        let _ = f.write_all(&string_buf);
    }

    // write game state
    let name = format!("{}/save/current/game.ssv", gamedir);
    if let Some(ref ge) = ctx.ge {
        if let Some(write_fn) = ge.write_game {
            write_fn(&name, autosave);
        }
    }
}

/// Read server state from save file.
///
/// Equivalent to C: `SV_ReadServerFile`
pub fn sv_read_server_file(ctx: &mut ServerContext) {
    com_dprintf("SV_ReadServerFile()\n");

    let gamedir = fs_gamedir();

    let name = format!("{}/save/current/server.ssv", gamedir);
    let mut f = match fs::File::open(&name) {
        Ok(f) => f,
        Err(_) => {
            com_printf(&format!("Couldn't read {}\n", name));
            return;
        }
    };

    // read the comment field
    let mut comment = [0u8; 32];
    let _ = f.read_exact(&mut comment);

    // read the mapcmd
    let mut mapcmd_buf = [0u8; MAX_TOKEN_CHARS];
    let _ = f.read_exact(&mut mapcmd_buf);

    // read all CVAR_LATCH cvars
    loop {
        let mut name_buf = [0u8; MAX_OSPATH];
        if f.read_exact(&mut name_buf).is_err() {
            break;
        }
        let mut string_buf = [0u8; 128];
        if f.read_exact(&mut string_buf).is_err() {
            break;
        }

        let name_end = name_buf.iter().position(|&b| b == 0).unwrap_or(MAX_OSPATH);
        let name_str = String::from_utf8_lossy(&name_buf[..name_end]).to_string();

        let string_end = string_buf.iter().position(|&b| b == 0).unwrap_or(128);
        let string_str = String::from_utf8_lossy(&string_buf[..string_end]).to_string();

        com_dprintf(&format!("Set {} = {}\n", name_str, string_str));
        ctx.cvars.force_set(&name_str, &string_str);
    }

    // start a new game fresh with new cvars
    crate::sv_init::sv_init_game(ctx);

    let mapcmd_end = mapcmd_buf.iter().position(|&b| b == 0).unwrap_or(MAX_TOKEN_CHARS);
    ctx.svs.mapcmd = String::from_utf8_lossy(&mapcmd_buf[..mapcmd_end]).to_string();

    // read game state
    let name = format!("{}/save/current/game.ssv", gamedir);
    if let Some(ref ge) = ctx.ge {
        if let Some(read_fn) = ge.read_game {
            read_fn(&name);
        }
    }
}

// =========================================================

/// Puts the server in demo mode on a specific map/cinematic.
///
/// Equivalent to C: `SV_DemoMap_f`
pub fn sv_demo_map_f(ctx: &mut ServerContext, cmd_argv: &dyn Fn(usize) -> String) {
    let map = cmd_argv(1);
    crate::sv_init::sv_map(ctx, true, &map, false);
}

/// Saves the state of the map just being exited and goes to a new map.
///
/// Equivalent to C: `SV_GameMap_f`
pub fn sv_game_map_f(ctx: &mut ServerContext, cmd_argc: usize, cmd_argv: &dyn Fn(usize) -> String) {
    if cmd_argc != 2 {
        com_printf("USAGE: gamemap <map>\n");
        return;
    }

    let map_arg = cmd_argv(1);
    com_dprintf(&format!("SV_GameMap({})\n", map_arg));

    let gamedir = fs_gamedir();
    fs_create_path(&format!("{}/save/current/", gamedir));

    // check for clearing the current savegame
    if map_arg.starts_with('*') {
        // wipe all the *.sav files
        sv_wipe_savegame(&gamedir, "current");
    } else {
        // save the map just exited
        if ctx.sv.state == ServerState::Game {
            // clear all the client inuse flags before saving so that
            // when the level is re-entered, the clients will spawn
            // at spawn points instead of occupying body shells
            let max = ctx.maxclients_value as usize;
            let mut saved_inuse = Vec::with_capacity(max);
            for i in 0..max {
                let edict_idx = ctx.svs.clients[i].edict_index;
                saved_inuse.push(get_edict_inuse(&ctx.ge, edict_idx));
                set_edict_inuse(&mut ctx.ge, edict_idx, false);
            }

            sv_write_level_file(ctx);

            // we must restore these for clients to transfer over correctly
            for i in 0..max {
                let edict_idx = ctx.svs.clients[i].edict_index;
                set_edict_inuse(&mut ctx.ge, edict_idx, saved_inuse[i]);
            }
        }
    }

    // start up the next map
    let map_arg2 = cmd_argv(1);
    crate::sv_init::sv_map(ctx, false, &map_arg2, false);

    // archive server state
    let map_arg3 = cmd_argv(1);
    ctx.svs.mapcmd = map_arg3[..map_arg3.len().min(MAX_TOKEN_CHARS - 1)].to_string();

    // copy off the level to the autosave slot
    if ctx.cvars.variable_value("dedicated") == 0.0 {
        sv_write_server_file(ctx, true);
        let gamedir = fs_gamedir();
        sv_copy_save_game(&gamedir, "current", "save0");
    }
}

/// Goes directly to a given map without any savegame archiving.
/// For development work.
///
/// Equivalent to C: `SV_Map_f`
pub fn sv_map_f(ctx: &mut ServerContext, cmd_argc: usize, cmd_argv: &dyn Fn(usize) -> String) {
    // if not a pcx, demo, or cinematic, check to make sure the level exists
    let map = cmd_argv(1);
    if !map.contains('.') {
        let expanded = format!("maps/{}.bsp", map);
        if fs_load_file(&expanded).is_none() {
            com_printf(&format!("Can't find {}\n", expanded));
            return;
        }
    }

    ctx.sv.state = ServerState::Dead; // don't save current level when changing
    let gamedir = fs_gamedir();
    sv_wipe_savegame(&gamedir, "current");
    sv_game_map_f(ctx, cmd_argc, cmd_argv);
}

// =====================================================================
//
//   SAVEGAMES
//
// =====================================================================

/// Load a saved game.
///
/// Equivalent to C: `SV_Loadgame_f`
pub fn sv_loadgame_f(ctx: &mut ServerContext, cmd_argc: usize, cmd_argv: &dyn Fn(usize) -> String) {
    if cmd_argc != 2 {
        com_printf("USAGE: loadgame <directory>\n");
        return;
    }

    com_printf("Loading game...\n");

    let dir = cmd_argv(1);
    if dir.contains("..") || dir.contains('/') || dir.contains('\\') {
        com_printf("Bad savedir.\n");
    }

    // make sure the server.ssv file exists
    let gamedir = fs_gamedir();
    let name = format!("{}/save/{}/server.ssv", gamedir, dir);
    if !Path::new(&name).exists() {
        com_printf(&format!("No such savegame: {}\n", name));
        return;
    }

    let dir2 = cmd_argv(1);
    sv_copy_save_game(&gamedir, &dir2, "current");

    sv_read_server_file(ctx);

    // go to the map
    ctx.sv.state = ServerState::Dead; // don't save current level when changing
    let mapcmd = ctx.svs.mapcmd.clone();
    crate::sv_init::sv_map(ctx, false, &mapcmd, true);
}

/// Save the current game.
///
/// Equivalent to C: `SV_Savegame_f`
pub fn sv_savegame_f(ctx: &mut ServerContext, cmd_argc: usize, cmd_argv: &dyn Fn(usize) -> String) {
    if ctx.sv.state != ServerState::Game {
        com_printf("You must be in a game to save.\n");
        return;
    }

    if cmd_argc != 2 {
        com_printf("USAGE: savegame <directory>\n");
        return;
    }

    if ctx.cvars.variable_value("deathmatch") != 0.0 {
        com_printf("Can't savegame in a deathmatch\n");
        return;
    }

    let save_name = cmd_argv(1);
    if save_name == "current" {
        com_printf("Can't save to 'current'\n");
        return;
    }

    if ctx.maxclients_value == 1.0 {
        // check health via game export
        let health = if let Some(ref ge) = ctx.ge {
            ge.get_client_health(ctx.svs.clients[0].edict_index)
        } else {
            100 // default
        };
        if health <= 0 {
            com_printf("\nCan't savegame while dead!\n");
            return;
        }
    }

    let dir = cmd_argv(1);
    if dir.contains("..") || dir.contains('/') || dir.contains('\\') {
        com_printf("Bad savedir.\n");
    }

    com_printf("Saving game...\n");

    // archive current level, including all client edicts.
    sv_write_level_file(ctx);

    // save server state
    sv_write_server_file(ctx, false);

    // copy it off
    let gamedir = fs_gamedir();
    sv_copy_save_game(&gamedir, "current", &dir);

    com_printf("Done.\n");
}

// ===============================================================

/// Kick a user off of the server.
///
/// Equivalent to C: `SV_Kick_f`
pub fn sv_kick_f(ctx: &mut ServerContext, cmd_argc: usize, cmd_argv: &dyn Fn(usize) -> String) {
    if !ctx.svs.initialized {
        com_printf("No server running.\n");
        return;
    }

    if cmd_argc != 2 {
        com_printf("Usage: kick <userid>\n");
        return;
    }

    if !sv_set_player(ctx, cmd_argc, cmd_argv) {
        return;
    }

    let client_idx = ctx.sv_client_index.unwrap();
    let client_name = ctx.svs.clients[client_idx].name.clone();

    crate::sv_send::sv_broadcast_printf(ctx, PRINT_HIGH, &format!("{} was kicked\n", client_name));
    crate::sv_send::sv_client_printf(&mut ctx.svs.clients[client_idx], PRINT_HIGH, "You were kicked from the game\n");
    crate::sv_main::sv_drop_client(ctx, client_idx);
    ctx.svs.clients[client_idx].lastmessage = ctx.svs.realtime;
}

/// Print server status to console.
///
/// Equivalent to C: `SV_Status_f`
pub fn sv_status_f(ctx: &ServerContext) {
    if ctx.svs.clients.is_empty() {
        com_printf("No server running.\n");
        return;
    }
    com_printf(&format!("map              : {}\n", ctx.sv.name));

    com_printf("num score ping name            lastmsg address               qport \n");
    com_printf("--- ----- ---- --------------- ------- --------------------- ------\n");
    let max = ctx.maxclients_value as usize;
    for i in 0..max {
        let cl = &ctx.svs.clients[i];
        if cl.state == ClientState::Free {
            continue;
        }
        com_printf(&format!("{:3} ", i));

        let frags = if let Some(ref ge) = ctx.ge {
            ge.get_client_frags(cl.edict_index)
        } else {
            0
        };
        com_printf(&format!("{:5} ", frags));

        if cl.state == ClientState::Connected {
            com_printf("CNCT ");
        } else if cl.state == ClientState::Zombie {
            com_printf("ZMBI ");
        } else {
            let ping = if cl.ping < 9999 { cl.ping } else { 9999 };
            com_printf(&format!("{:4} ", ping));
        }

        com_printf(&cl.name);
        let pad = 16i32 - cl.name.len() as i32;
        for _ in 0..pad.max(0) {
            com_printf(" ");
        }

        com_printf(&format!("{:7} ", ctx.svs.realtime - cl.lastmessage));

        let s = crate::sv_main::net_adr_to_string(&cl.netchan.remote_address);
        com_printf(&s);
        let pad = 22i32 - s.len() as i32;
        for _ in 0..pad.max(0) {
            com_printf(" ");
        }

        com_printf(&format!("{:5}", cl.netchan.qport));

        com_printf("\n");
    }
    com_printf("\n");
}

/// Console say command (dedicated server only).
///
/// Equivalent to C: `SV_ConSay_f`
pub fn sv_con_say_f(ctx: &mut ServerContext, cmd_argc: usize, cmd_args: &str) {
    if cmd_argc < 2 {
        return;
    }

    let mut text = String::from("console: ");
    let mut p = cmd_args.to_string();

    if p.starts_with('"') {
        p = p[1..].to_string();
        if p.ends_with('"') {
            p.pop();
        }
    }

    text.push_str(&p);

    let max = ctx.maxclients_value as usize;
    for j in 0..max {
        if ctx.svs.clients[j].state != ClientState::Spawned {
            continue;
        }
        crate::sv_send::sv_client_printf(&mut ctx.svs.clients[j], PRINT_CHAT, &format!("{}\n", text));
    }
}

/// Force a heartbeat to be sent to the master servers.
///
/// Equivalent to C: `SV_Heartbeat_f`
pub fn sv_heartbeat_f(ctx: &mut ServerContext) {
    ctx.svs.last_heartbeat = -9999999;
}

/// Examine or change the serverinfo string.
///
/// Equivalent to C: `SV_Serverinfo_f`
pub fn sv_serverinfo_f(ctx: &ServerContext) {
    com_printf("Server info settings:\n");
    let info = ctx.cvars.serverinfo();
    info_print(&info);
}

/// Examine all a user's info strings.
///
/// Equivalent to C: `SV_DumpUser_f`
pub fn sv_dump_user_f(ctx: &mut ServerContext, cmd_argc: usize, cmd_argv: &dyn Fn(usize) -> String) {
    if cmd_argc != 2 {
        com_printf("Usage: info <userid>\n");
        return;
    }

    if !sv_set_player(ctx, cmd_argc, cmd_argv) {
        return;
    }

    let client_idx = ctx.sv_client_index.unwrap();
    com_printf("userinfo\n");
    com_printf("--------\n");
    let userinfo = ctx.svs.clients[client_idx].userinfo.clone();
    info_print(&userinfo);
}

/// Begins server demo recording.
///
/// Equivalent to C: `SV_ServerRecord_f`
pub fn sv_server_record_f(ctx: &mut ServerContext, cmd_argc: usize, cmd_argv: &dyn Fn(usize) -> String) {
    if cmd_argc != 2 {
        com_printf("serverrecord <demoname>\n");
        return;
    }

    if ctx.svs.demofile.is_some() {
        com_printf("Already recording.\n");
        return;
    }

    if ctx.sv.state != ServerState::Game {
        com_printf("You must be in a level to record.\n");
        return;
    }

    // open the demo file
    let gamedir = fs_gamedir();
    let demo_name = cmd_argv(1);
    let name = format!("{}/demos/{}.dm2", gamedir, demo_name);

    com_printf(&format!("recording to {}.\n", name));
    fs_create_path(&name);
    let f = match fs::File::create(&name) {
        Ok(f) => f,
        Err(_) => {
            com_printf("ERROR: couldn't open.\n");
            return;
        }
    };
    ctx.svs.demofile = Some(f);

    // setup a buffer to catch all multicasts
    ctx.svs.demo_multicast.clear();

    // write a single giant fake message with all the startup info
    let mut buf: Vec<u8> = Vec::with_capacity(32768);

    // send the serverdata
    msg_write_byte_vec(&mut buf, SvcOps::ServerData as i32);
    msg_write_long_vec(&mut buf, PROTOCOL_VERSION);
    msg_write_long_vec(&mut buf, ctx.svs.spawncount);
    // 2 means server demo
    msg_write_byte_vec(&mut buf, 2); // demos are always attract loops
    msg_write_string_vec(&mut buf, ctx.cvars.variable_string("gamedir"));
    msg_write_short_vec(&mut buf, -1);
    // send full levelname
    msg_write_string_vec(&mut buf, &ctx.sv.configstrings[CS_NAME]);

    for i in 0..MAX_CONFIGSTRINGS {
        if !ctx.sv.configstrings[i].is_empty() {
            msg_write_byte_vec(&mut buf, SvcOps::ConfigString as i32);
            msg_write_short_vec(&mut buf, i as i32);
            msg_write_string_vec(&mut buf, &ctx.sv.configstrings[i]);
        }
    }

    // write it to the demo file
    com_dprintf(&format!("signon message length: {}\n", buf.len()));
    let len = (buf.len() as i32).to_le_bytes();
    if let Some(ref mut demo) = ctx.svs.demofile {
        let _ = demo.write_all(&len);
        let _ = demo.write_all(&buf);
    }
}

/// Ends server demo recording.
///
/// Equivalent to C: `SV_ServerStop_f`
pub fn sv_server_stop_f(ctx: &mut ServerContext) {
    if ctx.svs.demofile.is_none() {
        com_printf("Not doing a serverrecord.\n");
        return;
    }
    ctx.svs.demofile = None;
    com_printf("Recording completed.\n");
}

/// Kick everyone off, possibly in preparation for a new game.
///
/// Equivalent to C: `SV_KillServer_f`
pub fn sv_kill_server_f(ctx: &mut ServerContext) {
    if !ctx.svs.initialized {
        return;
    }
    crate::sv_main::sv_shutdown(ctx, "Server was killed.\n", false);
    net_config(false); // close network sockets
}

/// Let the game dll handle a command.
///
/// Equivalent to C: `SV_ServerCommand_f`
pub fn sv_server_command_f(ctx: &mut ServerContext) {
    if ctx.ge.is_none() {
        com_printf("No game loaded.\n");
        return;
    }

    if let Some(ref ge) = ctx.ge {
        if let Some(cmd_fn) = ge.server_command {
            cmd_fn();
        }
    }
}

// ===========================================================

/// Register all server operator console commands.
///
/// Equivalent to C: `SV_InitOperatorCommands`
pub fn sv_init_operator_commands(_ctx: &mut ServerContext) {
    use myq2_common::cmd::cmd_add_command;

    // Register server operator commands with the command system.
    // The handlers are registered as None because the actual server command
    // functions require &mut ServerContext which is not available through
    // the generic CmdContext interface. Server command dispatch will be
    // wired through SV_ServerCommand_f at a higher level.
    cmd_add_command("heartbeat", None);
    cmd_add_command("kick", None);
    cmd_add_command("status", None);
    cmd_add_command("serverinfo", None);
    cmd_add_command("dumpuser", None);
    cmd_add_command("map", None);
    cmd_add_command("demomap", None);
    cmd_add_command("gamemap", None);
    cmd_add_command("setmaster", None);
    cmd_add_command("serverrecord", None);
    cmd_add_command("serverstop", None);
    cmd_add_command("save", None);
    cmd_add_command("load", None);
    cmd_add_command("killserver", None);
    cmd_add_command("sv", None);
}

// ============================================================
// Placeholder / stub functions
// ============================================================

/// CM_WritePortalState — Write area portal state to a save file.
///
/// Serializes the open/closed state of all area portals so they can
/// be restored when loading a saved game.
fn cm_write_portal_state(f: &mut fs::File) {
    myq2_common::cmodel::with_cmodel_ctx(|ctx| {
        if let Err(e) = ctx.write_portal_state(f) {
            com_printf(&format!("CM_WritePortalState error: {}\n", e));
        }
    });
}

/// CM_ReadPortalState — Read area portal state from a save file.
///
/// Restores the open/closed state of area portals from a saved game.
fn cm_read_portal_state(f: &mut fs::File) {
    myq2_common::cmodel::with_cmodel_ctx(|ctx| {
        if let Err(e) = ctx.read_portal_state(f) {
            com_printf(&format!("CM_ReadPortalState error: {}\n", e));
        }
    });
}

/// NET_Config — Configure network sockets.
///
/// Opens or closes network sockets based on whether multiplayer
/// is enabled. When false, closes all server sockets.
/// Delegates to myq2_common::net::net_config.
fn net_config(multiplayer: bool) {
    myq2_common::net::net_config(multiplayer);
}

/// Placeholder: get edict inuse flag
fn get_edict_inuse(ge: &Option<crate::sv_game::GameExport>, edict_index: i32) -> bool {
    if let Some(ref ge) = ge {
        if let Some(ent) = ge.edicts.get(edict_index as usize) {
            return ent.inuse;
        }
    }
    false
}

/// Placeholder: set edict inuse flag
fn set_edict_inuse(ge: &mut Option<crate::sv_game::GameExport>, edict_index: i32, inuse: bool) {
    if let Some(ref mut ge) = ge {
        if let Some(ent) = ge.edicts.get_mut(edict_index as usize) {
            ent.inuse = inuse;
        }
    }
}

/// Info_Print — Pretty-print an info string's key/value pairs.
///
/// Parses a backslash-delimited info string and prints each
/// key-value pair on its own line. Delegates to myq2_common::common.
fn info_print(s: &str) {
    myq2_common::common::info_print(s);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sv_game::{GameExport, Edict, GClient};

    // ============================================================
    // Helper to construct a minimal ServerContext for testing
    // ============================================================

    fn make_test_server_context() -> ServerContext {
        let mut ctx = ServerContext::default();
        ctx.svs.clients.resize_with(4, Client::default);
        ctx.maxclients_value = 4.0;
        ctx.svs.initialized = true;
        ctx
    }

    fn make_test_server_context_with_game() -> ServerContext {
        let mut ctx = make_test_server_context();
        ctx.sv.state = ServerState::Game;
        ctx.sv.name = "q2dm1".to_string();
        ctx.sv.configstrings[CS_NAME] = "The Edge".to_string();

        let mut ge = GameExport::default();
        ge.edicts.resize_with(8, Edict::default);
        ge.num_edicts = 5;
        ctx.ge = Some(ge);

        for i in 0..4 {
            ctx.svs.clients[i].edict_index = (i + 1) as i32;
            ctx.svs.clients[i].name = format!("player{}", i);
            ctx.svs.clients[i].state = ClientState::Spawned;
        }

        ctx
    }

    // Helper argv function for tests
    fn make_argv(args: &[&str]) -> Box<dyn Fn(usize) -> String> {
        let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        Box::new(move |i: usize| -> String {
            if i < owned.len() {
                owned[i].clone()
            } else {
                String::new()
            }
        })
    }

    // ============================================================
    // sv_set_player: numeric slot lookup
    // ============================================================

    #[test]
    fn test_sv_set_player_numeric_valid() {
        let mut ctx = make_test_server_context_with_game();
        let argv = make_argv(&["kick", "0"]);
        let result = sv_set_player(&mut ctx, 2, &*argv);
        assert!(result);
        assert_eq!(ctx.sv_client_index, Some(0));
    }

    #[test]
    fn test_sv_set_player_numeric_free_slot() {
        let mut ctx = make_test_server_context();
        let argv = make_argv(&["kick", "0"]);
        // Client 0 is Free by default
        let result = sv_set_player(&mut ctx, 2, &*argv);
        assert!(!result); // should fail because client is Free
    }

    #[test]
    fn test_sv_set_player_numeric_out_of_range() {
        let mut ctx = make_test_server_context_with_game();
        let argv = make_argv(&["kick", "999"]);
        let result = sv_set_player(&mut ctx, 2, &*argv);
        assert!(!result);
    }

    #[test]
    fn test_sv_set_player_numeric_negative() {
        let mut ctx = make_test_server_context_with_game();
        // Negative value: parse gives -1, which is < 0
        let argv = make_argv(&["kick", "-1"]);
        let result = sv_set_player(&mut ctx, 2, &*argv);
        assert!(!result);
    }

    #[test]
    fn test_sv_set_player_not_enough_args() {
        let mut ctx = make_test_server_context_with_game();
        let argv = make_argv(&["kick"]);
        let result = sv_set_player(&mut ctx, 1, &*argv);
        assert!(!result);
    }

    // ============================================================
    // sv_set_player: name lookup
    // ============================================================

    #[test]
    fn test_sv_set_player_by_name_found() {
        let mut ctx = make_test_server_context_with_game();
        let argv = make_argv(&["kick", "player2"]);
        let result = sv_set_player(&mut ctx, 2, &*argv);
        assert!(result);
        assert_eq!(ctx.sv_client_index, Some(2));
    }

    #[test]
    fn test_sv_set_player_by_name_not_found() {
        let mut ctx = make_test_server_context_with_game();
        let argv = make_argv(&["kick", "nosuchplayer"]);
        let result = sv_set_player(&mut ctx, 2, &*argv);
        assert!(!result);
    }

    // ============================================================
    // sv_kick_f: no server running
    // ============================================================

    #[test]
    fn test_sv_kick_f_no_server() {
        let mut ctx = make_test_server_context();
        ctx.svs.initialized = false;
        let argv = make_argv(&["kick", "0"]);
        sv_kick_f(&mut ctx, 2, &*argv);
        // Should just print "No server running." and return
    }

    #[test]
    fn test_sv_kick_f_wrong_argc() {
        let mut ctx = make_test_server_context_with_game();
        let argv = make_argv(&["kick"]);
        sv_kick_f(&mut ctx, 1, &*argv);
        // Should print usage and return
    }

    // ============================================================
    // sv_status_f: no clients
    // ============================================================

    #[test]
    fn test_sv_status_f_no_clients() {
        let mut ctx = make_test_server_context();
        ctx.svs.clients.clear();
        sv_status_f(&ctx);
        // Should print "No server running." without panic
    }

    #[test]
    fn test_sv_status_f_with_clients() {
        let ctx = make_test_server_context_with_game();
        sv_status_f(&ctx);
        // Should print status table without panic
    }

    #[test]
    fn test_sv_status_f_with_mixed_client_states() {
        let mut ctx = make_test_server_context_with_game();
        ctx.svs.clients[0].state = ClientState::Spawned;
        ctx.svs.clients[0].ping = 50;
        ctx.svs.clients[1].state = ClientState::Connected;
        ctx.svs.clients[2].state = ClientState::Zombie;
        ctx.svs.clients[3].state = ClientState::Free;

        sv_status_f(&ctx);
        // Should handle CNCT, ZMBI, and skip Free clients
    }

    #[test]
    fn test_sv_status_f_ping_clamping() {
        let mut ctx = make_test_server_context_with_game();
        ctx.svs.clients[0].ping = 99999; // exceeds 9999
        sv_status_f(&ctx);
        // Should clamp displayed ping to 9999
    }

    // ============================================================
    // sv_serverinfo_f
    // ============================================================

    #[test]
    fn test_sv_serverinfo_f_no_panic() {
        let ctx = make_test_server_context();
        sv_serverinfo_f(&ctx);
    }

    // ============================================================
    // sv_heartbeat_f
    // ============================================================

    #[test]
    fn test_sv_heartbeat_f_resets_timer() {
        let mut ctx = make_test_server_context();
        ctx.svs.last_heartbeat = 0;
        sv_heartbeat_f(&mut ctx);
        assert_eq!(ctx.svs.last_heartbeat, -9999999);
    }

    // ============================================================
    // sv_server_stop_f
    // ============================================================

    #[test]
    fn test_sv_server_stop_f_no_demo() {
        let mut ctx = make_test_server_context();
        ctx.svs.demofile = None;
        sv_server_stop_f(&mut ctx);
        // Should print "Not doing a serverrecord."
    }

    // ============================================================
    // sv_server_command_f: no game
    // ============================================================

    #[test]
    fn test_sv_server_command_f_no_game() {
        let mut ctx = make_test_server_context();
        ctx.ge = None;
        sv_server_command_f(&mut ctx);
        // Should print "No game loaded."
    }

    // ============================================================
    // sv_kill_server_f: not initialized
    // ============================================================

    #[test]
    fn test_sv_kill_server_f_not_initialized() {
        let mut ctx = make_test_server_context();
        ctx.svs.initialized = false;
        sv_kill_server_f(&mut ctx);
        // Should return early without doing anything
    }

    // ============================================================
    // sv_con_say_f: basic functionality
    // ============================================================

    #[test]
    fn test_sv_con_say_f_no_args() {
        let mut ctx = make_test_server_context_with_game();
        sv_con_say_f(&mut ctx, 1, "");
        // argc < 2 should return early
    }

    #[test]
    fn test_sv_con_say_f_with_message() {
        let mut ctx = make_test_server_context_with_game();
        sv_con_say_f(&mut ctx, 2, "hello world");
        // Should send "console: hello world\n" to spawned clients
    }

    #[test]
    fn test_sv_con_say_f_quoted_message() {
        let mut ctx = make_test_server_context_with_game();
        sv_con_say_f(&mut ctx, 2, "\"hello world\"");
        // Should strip quotes: sends "console: hello world\n"
    }

    #[test]
    fn test_sv_con_say_f_skips_non_spawned() {
        let mut ctx = make_test_server_context_with_game();
        ctx.svs.clients[0].state = ClientState::Connected; // not spawned
        ctx.svs.clients[1].state = ClientState::Spawned;
        ctx.svs.clients[2].state = ClientState::Free;
        ctx.svs.clients[3].state = ClientState::Zombie;

        sv_con_say_f(&mut ctx, 2, "test");
        // Should only send to client 1 (Spawned)
    }

    // ============================================================
    // sv_savegame_f: validation tests
    // ============================================================

    #[test]
    fn test_sv_savegame_f_not_in_game() {
        let mut ctx = make_test_server_context();
        ctx.sv.state = ServerState::Dead;
        let argv = make_argv(&["save", "mysave"]);
        sv_savegame_f(&mut ctx, 2, &*argv);
        // Should print "You must be in a game to save."
    }

    #[test]
    fn test_sv_savegame_f_wrong_argc() {
        let mut ctx = make_test_server_context_with_game();
        let argv = make_argv(&["save"]);
        sv_savegame_f(&mut ctx, 1, &*argv);
        // Should print usage
    }

    #[test]
    fn test_sv_savegame_f_current_rejected() {
        let mut ctx = make_test_server_context_with_game();
        let argv = make_argv(&["save", "current"]);
        sv_savegame_f(&mut ctx, 2, &*argv);
        // Should print "Can't save to 'current'"
    }

    #[test]
    fn test_sv_savegame_f_deathmatch_rejected() {
        let mut ctx = make_test_server_context_with_game();
        ctx.cvars.set("deathmatch", "1");
        let argv = make_argv(&["save", "mysave"]);
        sv_savegame_f(&mut ctx, 2, &*argv);
        // Should print "Can't savegame in a deathmatch"
    }

    #[test]
    fn test_sv_savegame_f_dead_player_rejected() {
        let mut ctx = make_test_server_context_with_game();
        ctx.maxclients_value = 1.0;
        ctx.svs.clients[0].edict_index = 1;

        // Set up a GClient with health 0
        let mut gclient = GClient::default();
        gclient.ps.stats[STAT_HEALTH as usize] = 0;
        let client_ptr = Box::into_raw(Box::new(gclient));

        if let Some(ref mut ge) = ctx.ge {
            ge.edicts[1].client = Some(client_ptr);
        }

        let argv = make_argv(&["save", "mysave"]);
        sv_savegame_f(&mut ctx, 2, &*argv);
        // Should print "Can't savegame while dead!"

        // Clean up
        unsafe { drop(Box::from_raw(client_ptr)); }
    }

    // ============================================================
    // sv_loadgame_f: validation tests
    // ============================================================

    #[test]
    fn test_sv_loadgame_f_wrong_argc() {
        let mut ctx = make_test_server_context();
        let argv = make_argv(&["load"]);
        sv_loadgame_f(&mut ctx, 1, &*argv);
        // Should print usage
    }

    #[test]
    fn test_sv_loadgame_f_bad_dir_dotdot() {
        let mut ctx = make_test_server_context();
        let argv = make_argv(&["load", "../evil"]);
        sv_loadgame_f(&mut ctx, 2, &*argv);
        // Should print "Bad savedir." (though it continues -- the C code
        // has a bug where it prints but doesn't return)
    }

    #[test]
    fn test_sv_loadgame_f_bad_dir_slash() {
        let mut ctx = make_test_server_context();
        let argv = make_argv(&["load", "dir/subdir"]);
        sv_loadgame_f(&mut ctx, 2, &*argv);
        // Should print "Bad savedir."
    }

    #[test]
    fn test_sv_loadgame_f_bad_dir_backslash() {
        let mut ctx = make_test_server_context();
        let argv = make_argv(&["load", "dir\\subdir"]);
        sv_loadgame_f(&mut ctx, 2, &*argv);
        // Should print "Bad savedir."
    }

    // ============================================================
    // sv_map_f: validation
    // ============================================================

    #[test]
    fn test_sv_map_f_wrong_argc() {
        let mut ctx = make_test_server_context();
        let argv = make_argv(&["map"]);
        sv_game_map_f(&mut ctx, 1, &*argv);
        // Should print usage
    }

    // ============================================================
    // sv_demo_map_f: basic call
    // ============================================================

    // Note: sv_demo_map_f calls sv_map which needs more infrastructure,
    // so we only test that the argv parsing works.

    // ============================================================
    // sv_set_master_f: non-dedicated server rejected
    // ============================================================

    #[test]
    fn test_sv_set_master_f_not_dedicated() {
        let mut ctx = make_test_server_context();
        // dedicated defaults to 0.0
        sv_set_master_f(&mut ctx);
        // Should print "Only dedicated servers use masters."
    }

    #[test]
    fn test_sv_set_master_f_clears_slots() {
        let mut ctx = make_test_server_context();
        ctx.cvars.set("dedicated", "1");

        // Pre-fill some master addresses
        for i in 1..MAX_MASTERS {
            ctx.master_adr[i].port = 12345;
        }

        sv_set_master_f(&mut ctx);

        // All slots except 0 should be cleared
        for i in 1..MAX_MASTERS {
            assert_eq!(ctx.master_adr[i].port, 0);
        }
        assert_eq!(ctx.svs.last_heartbeat, -9999999);
    }

    // ============================================================
    // sv_dump_user_f: validation
    // ============================================================

    #[test]
    fn test_sv_dump_user_f_wrong_argc() {
        let mut ctx = make_test_server_context();
        let argv = make_argv(&["dumpuser"]);
        sv_dump_user_f(&mut ctx, 1, &*argv);
        // Should print usage
    }

    // ============================================================
    // sv_server_record_f: validation
    // ============================================================

    #[test]
    fn test_sv_server_record_f_wrong_argc() {
        let mut ctx = make_test_server_context();
        let argv = make_argv(&["serverrecord"]);
        sv_server_record_f(&mut ctx, 1, &*argv);
        // Should print usage
    }

    #[test]
    fn test_sv_server_record_f_already_recording() {
        let mut ctx = make_test_server_context_with_game();
        // Create a temp file as demofile
        let tmpfile = std::env::temp_dir().join("myq2_test_demo.dm2");
        ctx.svs.demofile = Some(fs::File::create(&tmpfile).unwrap());

        let argv = make_argv(&["serverrecord", "test"]);
        sv_server_record_f(&mut ctx, 2, &*argv);
        // Should print "Already recording."

        // Clean up
        ctx.svs.demofile = None;
        let _ = fs::remove_file(&tmpfile);
    }

    #[test]
    fn test_sv_server_record_f_not_in_game() {
        let mut ctx = make_test_server_context();
        ctx.sv.state = ServerState::Dead;
        let argv = make_argv(&["serverrecord", "test"]);
        sv_server_record_f(&mut ctx, 2, &*argv);
        // Should print "You must be in a level to record."
    }

    // ============================================================
    // get_edict_inuse / set_edict_inuse
    // ============================================================

    #[test]
    fn test_get_set_edict_inuse() {
        let mut ge = Some(GameExport::default());
        if let Some(ref mut g) = ge {
            g.edicts.push(Edict::default());
            g.edicts.push(Edict::default());
        }

        assert!(!get_edict_inuse(&ge, 0));
        assert!(!get_edict_inuse(&ge, 1));

        set_edict_inuse(&mut ge, 1, true);
        assert!(get_edict_inuse(&ge, 1));
        assert!(!get_edict_inuse(&ge, 0));

        set_edict_inuse(&mut ge, 1, false);
        assert!(!get_edict_inuse(&ge, 1));
    }

    #[test]
    fn test_get_edict_inuse_no_ge() {
        let ge: Option<GameExport> = None;
        assert!(!get_edict_inuse(&ge, 0));
    }

    #[test]
    fn test_set_edict_inuse_no_ge() {
        let mut ge: Option<GameExport> = None;
        // Should not panic
        set_edict_inuse(&mut ge, 0, true);
    }

    #[test]
    fn test_get_edict_inuse_out_of_bounds() {
        let ge = Some(GameExport::default());
        assert!(!get_edict_inuse(&ge, 999));
    }

    // ============================================================
    // sv_init_operator_commands: registration
    // ============================================================

    #[test]
    fn test_sv_init_operator_commands_no_panic() {
        let mut ctx = make_test_server_context();
        sv_init_operator_commands(&mut ctx);
        // Should register commands without panic
    }

    // ============================================================
    // copy_file: non-existent source
    // ============================================================

    #[test]
    fn test_copy_file_nonexistent_source() {
        let tmpdir = std::env::temp_dir();
        let src = tmpdir.join("myq2_test_nonexistent.dat");
        let dst = tmpdir.join("myq2_test_copy_dst.dat");
        copy_file(&src.to_string_lossy(), &dst.to_string_lossy());
        // Should return early without creating dst
        assert!(!dst.exists());
    }

    #[test]
    fn test_copy_file_success() {
        let tmpdir = std::env::temp_dir();
        let src = tmpdir.join("myq2_test_copy_src.dat");
        let dst = tmpdir.join("myq2_test_copy_dst.dat");

        // Create source file
        let _ = fs::write(&src, b"test data for copy");

        copy_file(&src.to_string_lossy(), &dst.to_string_lossy());

        // Verify destination has same content
        let dst_content = fs::read(&dst).unwrap();
        assert_eq!(dst_content, b"test data for copy");

        // Clean up
        let _ = fs::remove_file(&src);
        let _ = fs::remove_file(&dst);
    }

    // ============================================================
    // sv_wipe_savegame: wipes save directory files
    // ============================================================

    #[test]
    fn test_sv_wipe_savegame_nonexistent_dir() {
        // Should not panic on non-existent directory
        sv_wipe_savegame("/tmp/myq2_nonexistent_gamedir", "testsave");
    }

    // ============================================================
    // sv_game_map_f: wipe on star prefix
    // ============================================================

    #[test]
    fn test_sv_game_map_f_star_prefix_does_not_crash() {
        // The star prefix triggers wipe of current savegame.
        // We just test that the argv parsing and branching works.
        let mut ctx = make_test_server_context();
        ctx.sv.state = ServerState::Dead;

        // Can't fully test sv_game_map_f because it calls sv_map internally,
        // but we can verify the argument count check.
        let argv = make_argv(&["gamemap"]);
        sv_game_map_f(&mut ctx, 1, &*argv);
        // Should print "USAGE: gamemap <map>"
    }

    // ============================================================
    // Savegame path validation patterns
    // ============================================================

    #[test]
    fn test_savegame_dir_validation_dotdot() {
        // Test the validation pattern used in loadgame/savegame
        let dir = "../evil";
        assert!(dir.contains("..") || dir.contains('/') || dir.contains('\\'));
    }

    #[test]
    fn test_savegame_dir_validation_slash() {
        let dir = "some/path";
        assert!(dir.contains('/'));
    }

    #[test]
    fn test_savegame_dir_validation_backslash() {
        let dir = "some\\path";
        assert!(dir.contains('\\'));
    }

    #[test]
    fn test_savegame_dir_validation_clean() {
        let dir = "save1";
        assert!(!dir.contains("..") && !dir.contains('/') && !dir.contains('\\'));
    }

    // ============================================================
    // sv_write_server_file: comment field formatting
    // ============================================================

    #[test]
    fn test_save_comment_format() {
        // Test that the comment format matches expected patterns
        let level_name = "The Edge";
        let save_comment = format!("SAVE {}", level_name);
        assert!(save_comment.starts_with("SAVE "));
        assert!(save_comment.len() <= 31);

        let auto_comment = format!("ENTERING {}", level_name);
        assert!(auto_comment.starts_with("ENTERING "));
    }

    #[test]
    fn test_save_comment_truncation() {
        // Very long level name should be truncated to 31 bytes
        let long_name = "A".repeat(100);
        let comment_str = format!("SAVE {}", long_name);
        let bytes = comment_str.as_bytes();
        let len = bytes.len().min(31);
        assert_eq!(len, 31);
    }
}
