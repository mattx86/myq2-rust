// g_svcmds.rs — Server command processing and IP filtering
// Converted from: myq2-original/game/g_svcmds.c
//
// Copyright (C) 1997-2001 Id Software, Inc.
//
// This program is free software; you can redistribute it and/or
// modify it under the terms of the GNU General Public License
// as published by the Free Software Foundation; either version 2
// of the License, or (at your option) any later version.

use crate::g_local::*;
use crate::game_import::*;
use std::fs::File;
use std::io::Write;

// PRINT_HIGH imported from g_local::* (via q_shared)

// ============================================================
// IP Filter types
// ============================================================

const MAX_IPFILTERS: usize = 1024;

#[derive(Debug, Clone, Copy, Default)]
pub struct IpFilter {
    pub mask: u32,
    pub compare: u32,
}

/// Holds the IP filter list state (replaces C globals `ipfilters` and `numipfilters`).
pub struct IpFilterState {
    pub filters: [IpFilter; MAX_IPFILTERS],
    pub num_filters: usize,
}

impl IpFilterState {
    pub fn new() -> Self {
        Self {
            filters: [IpFilter::default(); MAX_IPFILTERS],
            num_filters: 0,
        }
    }
}

impl Default for IpFilterState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// StringToFilter
// ============================================================

/// Parses a dotted IP string into an IpFilter (mask + compare).
/// Returns `None` if the address is malformed.
fn string_to_filter(s: &str) -> Option<IpFilter> {
    let mut b: [u8; 4] = [0; 4];
    let mut m: [u8; 4] = [0; 4];

    let parts: Vec<&str> = s.split('.').collect();

    for i in 0..4 {
        if i >= parts.len() {
            break;
        }
        let part = parts[i];
        if part.is_empty() || !part.bytes().next().is_some_and(|c| c.is_ascii_digit()) {
            gi_cprintf(-1, PRINT_HIGH, &format!("Bad filter address: {}\n", s));
            return None;
        }

        // Parse the numeric portion (stop at first non-digit)
        let num_str: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
        let val: u8 = match num_str.parse::<u32>() {
            Ok(v) => v as u8,
            Err(_) => {
                gi_cprintf(-1, PRINT_HIGH, &format!("Bad filter address: {}\n", s));
                return None;
            }
        };
        b[i] = val;
        if b[i] != 0 {
            m[i] = 255;
        }
    }

    Some(IpFilter {
        mask: u32::from_le_bytes(m),
        compare: u32::from_le_bytes(b),
    })
}

// ============================================================
// SV_FilterPacket
// ============================================================

/// Returns `true` if the packet from the given address should be filtered (blocked/allowed
/// depending on `filterban`).
pub fn sv_filter_packet(from: &str, state: &IpFilterState, filterban: f32) -> bool {
    let mut m: [u8; 4] = [0; 4];
    let mut i: usize = 0;
    let mut chars = from.chars().peekable();

    while chars.peek().is_some() && i < 4 {
        m[i] = 0;
        while let Some(&c) = chars.peek() {
            if !c.is_ascii_digit() {
                break;
            }
            m[i] = m[i].wrapping_mul(10).wrapping_add(c as u8 - b'0');
            chars.next();
        }
        match chars.peek() {
            None => break,
            Some(&':') => break,
            _ => {
                i += 1;
                chars.next();
            }
        }
    }

    let addr = u32::from_le_bytes(m);

    for j in 0..state.num_filters {
        if (addr & state.filters[j].mask) == state.filters[j].compare {
            return filterban as i32 != 0;
        }
    }

    filterban as i32 == 0
}

// ============================================================
// Svcmd_Test_f
// ============================================================

pub fn svcmd_test_f() {
    gi_cprintf(-1, PRINT_HIGH, "Svcmd_Test_f()\n");
}

// ============================================================
// SVCmd_AddIP_f
// ============================================================

/// Adds an IP filter. `args` represents the command arguments (gi.argv values).
/// args[0] = "sv", args[1] = "addip", args[2] = ip-mask
pub fn svcmd_addip_f(args: &[&str], state: &mut IpFilterState) {
    if args.len() < 3 {
        gi_cprintf(-1, PRINT_HIGH, "Usage:  addip <ip-mask>\n");
        return;
    }

    // Find a free spot (compare == 0xffffffff) or use the next slot
    let mut slot = state.num_filters;
    for i in 0..state.num_filters {
        if state.filters[i].compare == 0xffffffff {
            slot = i;
            break;
        }
    }
    if slot == state.num_filters {
        if state.num_filters == MAX_IPFILTERS {
            gi_cprintf(-1, PRINT_HIGH, "IP filter list is full\n");
            return;
        }
        state.num_filters += 1;
    }

    match string_to_filter(args[2]) {
        Some(f) => state.filters[slot] = f,
        None => state.filters[slot].compare = 0xffffffff,
    }
}

// ============================================================
// SVCmd_RemoveIP_f
// ============================================================

/// Removes an IP filter. `args` represents the command arguments.
pub fn svcmd_removeip_f(args: &[&str], state: &mut IpFilterState) {
    if args.len() < 3 {
        gi_cprintf(-1, PRINT_HIGH, "Usage:  sv removeip <ip-mask>\n");
        return;
    }

    let f = match string_to_filter(args[2]) {
        Some(f) => f,
        None => return,
    };

    for i in 0..state.num_filters {
        if state.filters[i].mask == f.mask && state.filters[i].compare == f.compare {
            // Shift remaining filters down
            for j in (i + 1)..state.num_filters {
                state.filters[j - 1] = state.filters[j];
            }
            state.num_filters -= 1;
            gi_cprintf(-1, PRINT_HIGH, "Removed.\n");
            return;
        }
    }

    gi_cprintf(-1, PRINT_HIGH, &format!("Didn't find {}.\n", args[2]));
}

// ============================================================
// SVCmd_ListIP_f
// ============================================================

/// Lists all current IP filters.
pub fn svcmd_listip_f(state: &IpFilterState) {
    gi_cprintf(-1, PRINT_HIGH, "Filter list:\n");
    for i in 0..state.num_filters {
        let b = state.filters[i].compare.to_le_bytes();
        gi_cprintf(-1, PRINT_HIGH, &format!("{:3}.{:3}.{:3}.{:3}\n", b[0], b[1], b[2], b[3]));
    }
}

// ============================================================
// SVCmd_WriteIP_f
// ============================================================

/// Writes current IP filters to `listip.cfg`.
pub fn svcmd_writeip_f(state: &IpFilterState, filterban: f32, game_cvar_string: &str) {
    let name = if game_cvar_string.is_empty() {
        format!("{}/listip.cfg", GAMEVERSION)
    } else {
        format!("{}/listip.cfg", game_cvar_string)
    };

    gi_cprintf(-1, PRINT_HIGH, &format!("Writing {}.\n", name));

    let mut f = match File::create(&name) {
        Ok(f) => f,
        Err(_) => {
            gi_cprintf(-1, PRINT_HIGH, &format!("Couldn't open {}\n", name));
            return;
        }
    };

    let _ = writeln!(f, "set filterban {}", filterban as i32);

    for i in 0..state.num_filters {
        let b = state.filters[i].compare.to_le_bytes();
        let _ = writeln!(f, "sv addip {}.{}.{}.{}", b[0], b[1], b[2], b[3]);
    }
}

// ============================================================
// ServerCommand
// ============================================================

/// Called when an "sv" command is issued. `args` contains all argv values from gi.
pub fn server_command(args: &[&str], ctx: &mut GameContext, ip_state: &mut IpFilterState) {
    if args.len() < 2 {
        gi_cprintf(-1, PRINT_HIGH, "Unknown server command\n");
        return;
    }

    let cmd = args[1];

    if cmd.eq_ignore_ascii_case("test") {
        svcmd_test_f();
    } else if cmd.eq_ignore_ascii_case("addip") {
        svcmd_addip_f(args, ip_state);
    } else if cmd.eq_ignore_ascii_case("removeip") {
        svcmd_removeip_f(args, ip_state);
    } else if cmd.eq_ignore_ascii_case("listip") {
        svcmd_listip_f(ip_state);
    } else if cmd.eq_ignore_ascii_case("writeip") {
        svcmd_writeip_f(ip_state, ctx.filterban, "");
    } else {
        gi_cprintf(-1, PRINT_HIGH, &format!("Unknown server command \"{}\"\n", cmd));
    }
}
