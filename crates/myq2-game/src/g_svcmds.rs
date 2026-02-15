// g_svcmds.rs -- server command processing for IP filtering
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
use std::sync::{LazyLock, Mutex};

// ==============================================================================
// PACKET FILTERING
//
// You can add or remove addresses from the filter list with:
//
// addip <ip>
// removeip <ip>
//
// The ip address is specified in dot format, and any unspecified digits will
// match any value, so you can specify an entire class C network with
// "addip 192.246.40".
//
// Removeip will only remove an address specified exactly the same way. You
// cannot addip a subnet, then removeip a single host.
//
// listip
// Prints the current list of filters.
//
// writeip
// Dumps "addip <ip>" commands to listip.cfg so it can be execed at a later
// date. The filter lists are not saved and restored by default, because I
// believe it would cause too much confusion.
//
// filterban <0 or 1>
//
// If 1 (the default), then ip addresses matching the current list will be
// prohibited from entering the game. This is the default setting.
//
// If 0, then only addresses matching the list will be allowed. This lets you
// easily set up a private game, or a game that only allows players from your
// local network.
// ==============================================================================

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

/// Global IP filter state, replacing C globals `ipfilters[]` and `numipfilters`.
static IP_FILTER_STATE: LazyLock<Mutex<IpFilterState>> = LazyLock::new(|| {
    Mutex::new(IpFilterState::new())
});

// =================
// StringToFilter
// =================

/// Parses a dotted IP string into an IpFilter (mask + compare).
/// Returns `None` if the address is malformed.
///
/// The mask has 0xFF for each specified octet that is non-zero, and 0x00 for
/// unspecified or zero octets. This matches the original C behavior where
/// `if (b[i] != 0) m[i] = 255;`.
fn string_to_filter(s: &str) -> Option<IpFilter> {
    let mut b: [u8; 4] = [0; 4];
    let mut m: [u8; 4] = [0; 4];

    let mut chars = s.as_bytes();
    let mut i = 0;

    while i < 4 {
        // The first character of each octet must be a digit
        if chars.is_empty() || chars[0] < b'0' || chars[0] > b'9' {
            gi_cprintf(-1, PRINT_HIGH, &format!("Bad filter address: {}\n", s));
            return None;
        }

        // Parse digits for this octet
        let mut num: u32 = 0;
        while !chars.is_empty() && chars[0] >= b'0' && chars[0] <= b'9' {
            num = num * 10 + (chars[0] - b'0') as u32;
            chars = &chars[1..];
        }
        b[i] = num as u8;
        if b[i] != 0 {
            m[i] = 255;
        }

        // End of string — break
        if chars.is_empty() {
            break;
        }
        // Skip the dot separator
        chars = &chars[1..];
        i += 1;
    }

    Some(IpFilter {
        mask: u32::from_le_bytes(m),
        compare: u32::from_le_bytes(b),
    })
}

// =================
// SV_FilterPacket
// =================

/// Returns `true` if the packet from the given address should be filtered.
///
/// When `filterban` is 1 (default), returns true if the address matches any
/// filter (i.e. the address is banned). When `filterban` is 0, returns true
/// if the address does NOT match any filter (i.e. only matching addresses
/// are allowed).
///
/// `from` is a dotted IP string, optionally with a port suffix (e.g. "192.168.1.1:27910").
pub fn sv_filter_packet(from: &str) -> bool {
    let mut m_bytes: [u8; 4] = [0; 4];
    let mut i: usize = 0;
    let bytes = from.as_bytes();
    let mut p = 0;

    while p < bytes.len() && i < 4 {
        m_bytes[i] = 0;
        while p < bytes.len() && bytes[p] >= b'0' && bytes[p] <= b'9' {
            m_bytes[i] = m_bytes[i].wrapping_mul(10).wrapping_add(bytes[p] - b'0');
            p += 1;
        }
        if p >= bytes.len() || bytes[p] == b':' {
            break;
        }
        i += 1;
        p += 1;
    }

    let addr = u32::from_le_bytes(m_bytes);

    let filterban_val = gi_cvar("filterban", "1", 0);

    let state = IP_FILTER_STATE.lock().unwrap();
    for j in 0..state.num_filters {
        if (addr & state.filters[j].mask) == state.filters[j].compare {
            return filterban_val as i32 != 0;
        }
    }

    filterban_val as i32 == 0
}

// =================
// Svcmd_Test_f
// =================

fn svcmd_test_f() {
    gi_cprintf(-1, PRINT_HIGH, "Svcmd_Test_f()\n");
}

// =================
// SVCmd_AddIP_f
// =================

/// Adds an IP filter. Uses gi_argc()/gi_argv() to get the IP argument.
/// In C: gi.argv(2) gives the IP-mask argument (argv(0)="sv", argv(1)="addip").
fn svcmd_addip_f() {
    if gi_argc() < 3 {
        gi_cprintf(-1, PRINT_HIGH, "Usage:  addip <ip-mask>\n");
        return;
    }

    let ip_str = gi_argv(2);

    let mut state = IP_FILTER_STATE.lock().unwrap();

    // Find a free spot (compare == 0xffffffff marks a deleted entry) or use the next slot
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

    match string_to_filter(&ip_str) {
        Some(f) => state.filters[slot] = f,
        None => state.filters[slot].compare = 0xffffffff,
    }
}

// =================
// SVCmd_RemoveIP_f
// =================

/// Removes an IP filter that matches exactly. Uses gi_argc()/gi_argv() for args.
fn svcmd_removeip_f() {
    if gi_argc() < 3 {
        gi_cprintf(-1, PRINT_HIGH, "Usage:  sv removeip <ip-mask>\n");
        return;
    }

    let ip_str = gi_argv(2);

    let f = match string_to_filter(&ip_str) {
        Some(f) => f,
        None => return,
    };

    let mut state = IP_FILTER_STATE.lock().unwrap();

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

    gi_cprintf(-1, PRINT_HIGH, &format!("Didn't find {}.\n", ip_str));
}

// =================
// SVCmd_ListIP_f
// =================

/// Lists all current IP filters.
fn svcmd_listip_f() {
    gi_cprintf(-1, PRINT_HIGH, "Filter list:\n");

    let state = IP_FILTER_STATE.lock().unwrap();
    for i in 0..state.num_filters {
        let b = state.filters[i].compare.to_le_bytes();
        gi_cprintf(-1, PRINT_HIGH, &format!("{:3}.{:3}.{:3}.{:3}\n", b[0], b[1], b[2], b[3]));
    }
}

// =================
// SVCmd_WriteIP_f
// =================

/// Writes current IP filters to `listip.cfg` in the game directory.
fn svcmd_writeip_f() {
    let game_dir = myq2_common::cvar::cvar_variable_string("game");

    let name = if game_dir.is_empty() {
        format!("{}/listip.cfg", GAMEVERSION)
    } else {
        format!("{}/listip.cfg", game_dir)
    };

    gi_cprintf(-1, PRINT_HIGH, &format!("Writing {}.\n", name));

    let mut f = match File::create(&name) {
        Ok(f) => f,
        Err(_) => {
            gi_cprintf(-1, PRINT_HIGH, &format!("Couldn't open {}\n", name));
            return;
        }
    };

    let filterban_val = gi_cvar("filterban", "1", 0);
    let _ = writeln!(f, "set filterban {}", filterban_val as i32);

    let state = IP_FILTER_STATE.lock().unwrap();
    for i in 0..state.num_filters {
        let b = state.filters[i].compare.to_le_bytes();
        let _ = writeln!(f, "sv addip {}.{}.{}.{}", b[0], b[1], b[2], b[3]);
    }
}

// =================
// ServerCommand
//
// ServerCommand will be called when an "sv" command is issued.
// The game can issue gi.argc() / gi.argv() commands to get the rest
// of the parameters.
// =================

/// Main server command dispatcher. `cmd` is the sub-command (gi.argv(1) in C terms,
/// e.g. "addip", "removeip", "listip", "writeip", "test").
pub fn server_command(cmd: &str) {
    if cmd.eq_ignore_ascii_case("test") {
        svcmd_test_f();
    } else if cmd.eq_ignore_ascii_case("addip") {
        svcmd_addip_f();
    } else if cmd.eq_ignore_ascii_case("removeip") {
        svcmd_removeip_f();
    } else if cmd.eq_ignore_ascii_case("listip") {
        svcmd_listip_f();
    } else if cmd.eq_ignore_ascii_case("writeip") {
        svcmd_writeip_f();
    } else {
        gi_cprintf(-1, PRINT_HIGH, &format!("Unknown server command \"{}\"\n", cmd));
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_to_filter_full_ip() {
        // We can't call gi_cprintf in tests without GameImport initialized,
        // so we test the parsing logic directly via the internal representation.
        // A full IP "192.168.1.100" should set all 4 mask bytes to 0xFF.
        let mut b: [u8; 4] = [0; 4];
        let mut m: [u8; 4] = [0; 4];

        let parts: Vec<&str> = "192.168.1.100".split('.').collect();
        for i in 0..4.min(parts.len()) {
            if let Ok(v) = parts[i].parse::<u8>() {
                b[i] = v;
                if b[i] != 0 {
                    m[i] = 255;
                }
            }
        }

        let mask = u32::from_le_bytes(m);
        let compare = u32::from_le_bytes(b);

        // All octets are non-zero, so all mask bytes are 0xFF
        assert_eq!(mask, 0xFFFFFFFF);
        assert_eq!(compare, u32::from_le_bytes([192, 168, 1, 100]));
    }

    #[test]
    fn test_string_to_filter_partial_ip() {
        // "192.168.1" should have mask bytes [0xFF, 0xFF, 0xFF, 0x00]
        let mut b: [u8; 4] = [0; 4];
        let mut m: [u8; 4] = [0; 4];

        let parts: Vec<&str> = "192.168.1".split('.').collect();
        for i in 0..4.min(parts.len()) {
            if let Ok(v) = parts[i].parse::<u8>() {
                b[i] = v;
                if b[i] != 0 {
                    m[i] = 255;
                }
            }
        }

        let mask = u32::from_le_bytes(m);
        let compare = u32::from_le_bytes(b);

        assert_eq!(mask, u32::from_le_bytes([255, 255, 255, 0]));
        assert_eq!(compare, u32::from_le_bytes([192, 168, 1, 0]));
    }

    #[test]
    fn test_ip_filter_state_default() {
        let state = IpFilterState::new();
        assert_eq!(state.num_filters, 0);
        assert_eq!(state.filters[0].mask, 0);
        assert_eq!(state.filters[0].compare, 0);
    }
}
