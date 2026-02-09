// keys.rs — Key input handling
// Converted from: myq2-original/client/keys.c
//
// Copyright (C) 1997-2001 Id Software, Inc.
// Licensed under the GNU General Public License v2

use myq2_common::common::com_printf;
use myq2_common::completion::complete_line;
use crate::client::{ConnState, KeyDest};
use crate::console::{
    cbuf_add_text, cmd_add_command, cmd_argc, cmd_argv, con_toggle_console_f, scr_update_screen, wildcardfit, CL, CLS,
    CHAT_BACKEDIT, CHAT_BUFFER, CHAT_BUFFERLEN, CHAT_TYPE, CON, CT_PERSON, CT_TEAM,
    CT_TELL, EDIT_LINE, KEY_LINEPOS, KEY_LINES, MAXCMDLINE,
};

// ============================================================
// Key constants (from keys.h)
// ============================================================

pub const K_TAB: i32 = 9;
pub const K_ENTER: i32 = 13;
pub const K_ESCAPE: i32 = 27;
pub const K_SPACE: i32 = 32;
pub const K_BACKSPACE: i32 = 127;
pub const K_UPARROW: i32 = 128;
pub const K_DOWNARROW: i32 = 129;
pub const K_LEFTARROW: i32 = 130;
pub const K_RIGHTARROW: i32 = 131;
pub const K_ALT: i32 = 132;
pub const K_CTRL: i32 = 133;
pub const K_SHIFT: i32 = 134;
pub const K_F1: i32 = 135;
pub const K_F2: i32 = 136;
pub const K_F3: i32 = 137;
pub const K_F4: i32 = 138;
pub const K_F5: i32 = 139;
pub const K_F6: i32 = 140;
pub const K_F7: i32 = 141;
pub const K_F8: i32 = 142;
pub const K_F9: i32 = 143;
pub const K_F10: i32 = 144;
pub const K_F11: i32 = 145;
pub const K_F12: i32 = 146;
pub const K_INS: i32 = 147;
pub const K_DEL: i32 = 148;
pub const K_PGDN: i32 = 149;
pub const K_PGUP: i32 = 150;
pub const K_HOME: i32 = 151;
pub const K_END: i32 = 152;

pub const K_KP_HOME: i32 = 160;
pub const K_KP_UPARROW: i32 = 161;
pub const K_KP_PGUP: i32 = 162;
pub const K_KP_LEFTARROW: i32 = 163;
pub const K_KP_5: i32 = 164;
pub const K_KP_RIGHTARROW: i32 = 165;
pub const K_KP_END: i32 = 166;
pub const K_KP_DOWNARROW: i32 = 167;
pub const K_KP_PGDN: i32 = 168;
pub const K_KP_ENTER: i32 = 169;
pub const K_KP_INS: i32 = 170;
pub const K_KP_DEL: i32 = 171;
pub const K_KP_SLASH: i32 = 172;
pub const K_KP_MINUS: i32 = 173;
pub const K_KP_PLUS: i32 = 174;

pub const K_PAUSE: i32 = 255;

pub const K_MOUSE1: i32 = 200;
pub const K_MOUSE2: i32 = 201;
pub const K_MOUSE3: i32 = 202;
// mattx86: mouse4_mouse5
pub const K_MOUSE4: i32 = 241;
pub const K_MOUSE5: i32 = 242;

pub const K_JOY1: i32 = 203;
pub const K_JOY2: i32 = 204;
pub const K_JOY3: i32 = 205;
pub const K_JOY4: i32 = 206;

pub const K_AUX1: i32 = 207;
pub const K_AUX2: i32 = 208;
pub const K_AUX3: i32 = 209;
pub const K_AUX4: i32 = 210;
pub const K_AUX5: i32 = 211;
pub const K_AUX6: i32 = 212;
pub const K_AUX7: i32 = 213;
pub const K_AUX8: i32 = 214;
pub const K_AUX9: i32 = 215;
pub const K_AUX10: i32 = 216;
pub const K_AUX11: i32 = 217;
pub const K_AUX12: i32 = 218;
pub const K_AUX13: i32 = 219;
pub const K_AUX14: i32 = 220;
pub const K_AUX15: i32 = 221;
pub const K_AUX16: i32 = 222;
pub const K_AUX17: i32 = 223;
pub const K_AUX18: i32 = 224;
pub const K_AUX19: i32 = 225;
pub const K_AUX20: i32 = 226;
pub const K_AUX21: i32 = 227;
pub const K_AUX22: i32 = 228;
pub const K_AUX23: i32 = 229;
pub const K_AUX24: i32 = 230;
pub const K_AUX25: i32 = 231;
pub const K_AUX26: i32 = 232;
pub const K_AUX27: i32 = 233;
pub const K_AUX28: i32 = 234;
pub const K_AUX29: i32 = 235;
pub const K_AUX30: i32 = 236;
pub const K_AUX31: i32 = 237;
pub const K_AUX32: i32 = 238;

pub const K_MWHEELDOWN: i32 = 239;
pub const K_MWHEELUP: i32 = 240;

// ============================================================
// Key name table
// ============================================================

struct KeyName {
    name: &'static str,
    keynum: i32,
}

static KEYNAMES: &[KeyName] = &[
    KeyName { name: "TAB", keynum: K_TAB },
    KeyName { name: "ENTER", keynum: K_ENTER },
    KeyName { name: "ESCAPE", keynum: K_ESCAPE },
    KeyName { name: "SPACE", keynum: K_SPACE },
    KeyName { name: "BACKSPACE", keynum: K_BACKSPACE },
    KeyName { name: "UPARROW", keynum: K_UPARROW },
    KeyName { name: "DOWNARROW", keynum: K_DOWNARROW },
    KeyName { name: "LEFTARROW", keynum: K_LEFTARROW },
    KeyName { name: "RIGHTARROW", keynum: K_RIGHTARROW },
    KeyName { name: "ALT", keynum: K_ALT },
    KeyName { name: "CTRL", keynum: K_CTRL },
    KeyName { name: "SHIFT", keynum: K_SHIFT },
    KeyName { name: "F1", keynum: K_F1 },
    KeyName { name: "F2", keynum: K_F2 },
    KeyName { name: "F3", keynum: K_F3 },
    KeyName { name: "F4", keynum: K_F4 },
    KeyName { name: "F5", keynum: K_F5 },
    KeyName { name: "F6", keynum: K_F6 },
    KeyName { name: "F7", keynum: K_F7 },
    KeyName { name: "F8", keynum: K_F8 },
    KeyName { name: "F9", keynum: K_F9 },
    KeyName { name: "F10", keynum: K_F10 },
    KeyName { name: "F11", keynum: K_F11 },
    KeyName { name: "F12", keynum: K_F12 },
    KeyName { name: "INS", keynum: K_INS },
    KeyName { name: "DEL", keynum: K_DEL },
    KeyName { name: "PGDN", keynum: K_PGDN },
    KeyName { name: "PGUP", keynum: K_PGUP },
    KeyName { name: "HOME", keynum: K_HOME },
    KeyName { name: "END", keynum: K_END },
    KeyName { name: "MOUSE1", keynum: K_MOUSE1 },
    KeyName { name: "MOUSE2", keynum: K_MOUSE2 },
    KeyName { name: "MOUSE3", keynum: K_MOUSE3 },
    KeyName { name: "MOUSE4", keynum: K_MOUSE4 },
    KeyName { name: "MOUSE5", keynum: K_MOUSE5 },
    KeyName { name: "JOY1", keynum: K_JOY1 },
    KeyName { name: "JOY2", keynum: K_JOY2 },
    KeyName { name: "JOY3", keynum: K_JOY3 },
    KeyName { name: "JOY4", keynum: K_JOY4 },
    KeyName { name: "AUX1", keynum: K_AUX1 },
    KeyName { name: "AUX2", keynum: K_AUX2 },
    KeyName { name: "AUX3", keynum: K_AUX3 },
    KeyName { name: "AUX4", keynum: K_AUX4 },
    KeyName { name: "AUX5", keynum: K_AUX5 },
    KeyName { name: "AUX6", keynum: K_AUX6 },
    KeyName { name: "AUX7", keynum: K_AUX7 },
    KeyName { name: "AUX8", keynum: K_AUX8 },
    KeyName { name: "AUX9", keynum: K_AUX9 },
    KeyName { name: "AUX10", keynum: K_AUX10 },
    KeyName { name: "AUX11", keynum: K_AUX11 },
    KeyName { name: "AUX12", keynum: K_AUX12 },
    KeyName { name: "AUX13", keynum: K_AUX13 },
    KeyName { name: "AUX14", keynum: K_AUX14 },
    KeyName { name: "AUX15", keynum: K_AUX15 },
    KeyName { name: "AUX16", keynum: K_AUX16 },
    KeyName { name: "AUX17", keynum: K_AUX17 },
    KeyName { name: "AUX18", keynum: K_AUX18 },
    KeyName { name: "AUX19", keynum: K_AUX19 },
    KeyName { name: "AUX20", keynum: K_AUX20 },
    KeyName { name: "AUX21", keynum: K_AUX21 },
    KeyName { name: "AUX22", keynum: K_AUX22 },
    KeyName { name: "AUX23", keynum: K_AUX23 },
    KeyName { name: "AUX24", keynum: K_AUX24 },
    KeyName { name: "AUX25", keynum: K_AUX25 },
    KeyName { name: "AUX26", keynum: K_AUX26 },
    KeyName { name: "AUX27", keynum: K_AUX27 },
    KeyName { name: "AUX28", keynum: K_AUX28 },
    KeyName { name: "AUX29", keynum: K_AUX29 },
    KeyName { name: "AUX30", keynum: K_AUX30 },
    KeyName { name: "AUX31", keynum: K_AUX31 },
    KeyName { name: "AUX32", keynum: K_AUX32 },
    KeyName { name: "KP_HOME", keynum: K_KP_HOME },
    KeyName { name: "KP_UPARROW", keynum: K_KP_UPARROW },
    KeyName { name: "KP_PGUP", keynum: K_KP_PGUP },
    KeyName { name: "KP_LEFTARROW", keynum: K_KP_LEFTARROW },
    KeyName { name: "KP_5", keynum: K_KP_5 },
    KeyName { name: "KP_RIGHTARROW", keynum: K_KP_RIGHTARROW },
    KeyName { name: "KP_END", keynum: K_KP_END },
    KeyName { name: "KP_DOWNARROW", keynum: K_KP_DOWNARROW },
    KeyName { name: "KP_PGDN", keynum: K_KP_PGDN },
    KeyName { name: "KP_ENTER", keynum: K_KP_ENTER },
    KeyName { name: "KP_INS", keynum: K_KP_INS },
    KeyName { name: "KP_DEL", keynum: K_KP_DEL },
    KeyName { name: "KP_SLASH", keynum: K_KP_SLASH },
    KeyName { name: "KP_MINUS", keynum: K_KP_MINUS },
    KeyName { name: "KP_PLUS", keynum: K_KP_PLUS },
    KeyName { name: "MWHEELUP", keynum: K_MWHEELUP },
    KeyName { name: "MWHEELDOWN", keynum: K_MWHEELDOWN },
    KeyName { name: "PAUSE", keynum: K_PAUSE },
    KeyName { name: "SEMICOLON", keynum: b';' as i32 },
];

// ============================================================
// Key state globals
// ============================================================

pub static mut SHIFT_DOWN: bool = false;
pub static mut ANYKEYDOWN: i32 = 0;
pub static mut HISTORY_LINE: i32 = 0;
pub static mut KEY_WAITING: i32 = 0;
pub static mut KEYBINDINGS: [Option<String>; 256] = {
    const NONE: Option<String> = None;
    [NONE; 256]
};
pub static mut CONSOLEKEYS: [bool; 256] = [false; 256];
pub static mut MENUBOUND: [bool; 256] = [false; 256];
pub static mut KEYSHIFT: [i32; 256] = [0i32; 256];
pub static mut KEY_REPEATS: [i32; 256] = [0i32; 256];
pub static mut KEYDOWN: [bool; 256] = [false; 256];
pub static mut KEY_INSERT: bool = true;

// ============================================================
// Placeholder stubs
// ============================================================




/// Placeholder — Cmd_CompleteCommand
fn cmd_complete_command(partial: &str) -> Option<String> {
    myq2_common::cmd::with_cmd_ctx(|ctx| {
        ctx.cmd_complete_command(partial).map(|s| s.to_string())
    }).flatten()
}

/// Placeholder — Cvar_CompleteVariable
fn cvar_complete_variable(partial: &str) -> Option<String> {
    myq2_common::cvar::with_cvar_ctx(|ctx| {
        ctx.complete_variable(partial).map(|s| s.to_string())
    }).flatten()
}

fn sys_get_clipboard_data() -> Option<String> {
    // SAFETY: single-threaded engine
    unsafe { (crate::console::SYSTEM_FNS.sys_get_clipboard_data)() }
}

/// Placeholder — Cbuf_InsertText
fn cbuf_insert_text(text: &str) {
    myq2_common::cmd::with_cmd_ctx(|ctx| {
        ctx.cbuf_insert_text(text);
    });
}

/// Sys_SendKeyEvents — wired through console module's system function pointer table.
fn sys_send_key_events() {
    crate::console::sys_send_key_events();
}

use myq2_common::common::sys_milliseconds;

/// Placeholder — Z_Free / Z_Malloc (not needed in Rust, use String)

/// M_Keydown — wired to menu module.
fn m_keydown(key: i32) {
    crate::menu::m_keydown(key);
}

/// M_Menu_Main_f — wired to menu module.
fn m_menu_main_f() {
    crate::menu::m_menu_main_f();
}

/// S_StartLocalSound — wired through console module's system function pointer table.
fn s_start_local_sound(name: &str) {
    // SAFETY: single-threaded engine
    unsafe { (crate::console::SYSTEM_FNS.s_start_local_sound)(name) }
}

/// Placeholder — Com_sprintf (not needed — use format!)

const STAT_LAYOUTS: usize = myq2_common::q_shared::STAT_LAYOUTS as usize;

// ============================================================
// Helper: get key line length (null-terminated)
// ============================================================

fn key_line_len(line: &[u8; MAXCMDLINE]) -> usize {
    line.iter().position(|&b| b == 0).unwrap_or(MAXCMDLINE)
}

// ============================================================
// CompleteCommand - Enhanced with multi-completion support
// ============================================================

/// Apply a completion to the current edit line.
/// If `add_space` is true, adds a trailing space after the completion.
fn apply_completion(text: &str, add_space: bool) {
    // SAFETY: single-threaded engine
    unsafe {
        KEY_LINES[EDIT_LINE as usize][1] = b'/';
        let bytes = text.as_bytes();
        for (i, &b) in bytes.iter().enumerate() {
            if i + 2 < MAXCMDLINE {
                KEY_LINES[EDIT_LINE as usize][i + 2] = b;
            }
        }
        KEY_LINEPOS = bytes.len() as i32 + 2;
        if add_space && (KEY_LINEPOS as usize) < MAXCMDLINE {
            KEY_LINES[EDIT_LINE as usize][KEY_LINEPOS as usize] = b' ';
            KEY_LINEPOS += 1;
        }
        KEY_LINES[EDIT_LINE as usize][KEY_LINEPOS as usize] = 0;
    }
}

fn complete_command() {
    // SAFETY: single-threaded engine
    unsafe {
        let line = &KEY_LINES[EDIT_LINE as usize];
        let mut start = 1usize;
        if start < MAXCMDLINE && (line[start] == b'\\' || line[start] == b'/') {
            start += 1;
        }

        // Build the line string (without leading ] and optional / or \)
        let len = key_line_len(line);
        let line_str: String = line[start..len].iter().map(|&b| b as char).collect();

        // Get completion result
        let result = complete_line(&line_str);

        match result.matches.len() {
            0 => {
                // No matches - do nothing
            }
            1 => {
                // Single match - complete fully with trailing space
                apply_completion(&result.matches[0], true);
            }
            _ => {
                // Multiple matches
                let trimmed_len = line_str.trim().len();

                // Complete to common prefix if it's longer than what we have
                if result.common_prefix.len() > trimmed_len {
                    apply_completion(&result.common_prefix, false);
                }

                // Print all matches to console
                com_printf("\n");
                for m in &result.matches {
                    com_printf(&format!("    {}\n", m));
                }
                com_printf(&format!("{} possible completions\n", result.matches.len()));
            }
        }
    }
}

// ============================================================
// CharOffset (from console.c, also used here)
// ============================================================

fn char_offset(s: &[u8], charcount: i32) -> usize {
    let mut count = charcount;
    let mut i = 0;
    while i < s.len() && count > 0 && s[i] != 0 {
        count -= 1;
        i += 1;
    }
    i
}

// ============================================================
// Key_Console
// ============================================================

/// Interactive line editing and console scrollback.
pub fn key_console(mut key: i32) {
    // SAFETY: single-threaded engine
    unsafe {
        // numpad to ascii conversion
        key = match key {
            K_KP_SLASH => b'/' as i32,
            K_KP_MINUS => b'-' as i32,
            K_KP_PLUS => b'+' as i32,
            K_KP_HOME => b'7' as i32,
            K_KP_UPARROW => b'8' as i32,
            K_KP_PGUP => b'9' as i32,
            K_KP_LEFTARROW => b'4' as i32,
            K_KP_5 => b'5' as i32,
            K_KP_RIGHTARROW => b'6' as i32,
            K_KP_END => b'1' as i32,
            K_KP_DOWNARROW => b'2' as i32,
            K_KP_PGDN => b'3' as i32,
            K_KP_INS => b'0' as i32,
            K_KP_DEL => b'.' as i32,
            _ => key,
        };

        // Ctrl+V or Shift+Ins paste
        if ((key as u8).eq_ignore_ascii_case(&b'V') && KEYDOWN[K_CTRL as usize])
            || ((key == K_INS || key == K_KP_INS) && KEYDOWN[K_SHIFT as usize])
        {
            if let Some(cbd) = sys_get_clipboard_data() {
                let cbd = cbd.replace(&['\n', '\r', '\x08'][..], "");
                let cbd_bytes = cbd.as_bytes();
                let mut i = cbd_bytes.len();
                if i + KEY_LINEPOS as usize >= MAXCMDLINE {
                    i = MAXCMDLINE - KEY_LINEPOS as usize;
                }
                if i > 0 {
                    let line = &mut KEY_LINES[EDIT_LINE as usize];
                    let pos = KEY_LINEPOS as usize;
                    for j in 0..i {
                        if pos + j < MAXCMDLINE {
                            line[pos + j] = cbd_bytes[j];
                        }
                    }
                    KEY_LINEPOS += i as i32;
                    if (KEY_LINEPOS as usize) < MAXCMDLINE {
                        line[KEY_LINEPOS as usize] = 0;
                    }
                }
            }
            return;
        }

        // Ctrl+L => clear
        if key == b'l' as i32 && KEYDOWN[K_CTRL as usize] {
            cbuf_add_text("clear\n");
            return;
        }

        // Enter
        if key == K_ENTER || key == K_KP_ENTER {
            let line = &KEY_LINES[EDIT_LINE as usize];
            if line[1] == b'\\' || line[1] == b'/' {
                // skip the command prefix
                let s: String = line[2..].iter().take_while(|&&b| b != 0).map(|&b| b as char).collect();
                cbuf_add_text(&s);
            } else {
                let s: String = line[1..].iter().take_while(|&&b| b != 0).map(|&b| b as char).collect();
                cbuf_add_text(&s);
            }
            cbuf_add_text("\n");

            let display: String = line.iter().take_while(|&&b| b != 0).map(|&b| b as char).collect();
            com_printf(&format!("{}\n", display));

            EDIT_LINE = (EDIT_LINE + 1) & 31;
            HISTORY_LINE = EDIT_LINE;
            KEY_LINES[EDIT_LINE as usize][0] = b']';
            KEY_LINES[EDIT_LINE as usize][1] = 0;
            KEY_LINEPOS = 1;

            if CLS.state == ConnState::Disconnected {
                scr_update_screen(); // force an update
            }
            return;
        }

        // Tab completion
        if key == K_TAB {
            complete_command();
            return;
        }

        // Left arrow
        if key == K_LEFTARROW || key == K_KP_LEFTARROW
            || (key == b'h' as i32 && KEYDOWN[K_CTRL as usize])
        {
            if KEY_LINEPOS > 1 {
                KEY_LINEPOS = char_offset(&KEY_LINES[EDIT_LINE as usize], KEY_LINEPOS - 1) as i32;
            }
            return;
        }

        // Backspace
        if key == K_BACKSPACE {
            if KEY_LINEPOS > 1 {
                let line = &mut KEY_LINES[EDIT_LINE as usize];
                let pos = KEY_LINEPOS as usize;
                let len = key_line_len(line);
                if pos <= len {
                    for i in (pos - 1)..len {
                        line[i] = if i + 1 < MAXCMDLINE { line[i + 1] } else { 0 };
                    }
                }
                KEY_LINEPOS -= 1;
            }
            return;
        }

        // Delete
        if key == K_DEL {
            let line = &mut KEY_LINES[EDIT_LINE as usize];
            let pos = KEY_LINEPOS as usize;
            let len = key_line_len(line);
            if pos < len {
                for i in pos..len {
                    line[i] = if i + 1 < MAXCMDLINE { line[i + 1] } else { 0 };
                }
            }
            return;
        }

        // Insert toggle
        if key == K_INS {
            KEY_INSERT = !KEY_INSERT;
            return;
        }

        // Right arrow
        if key == K_RIGHTARROW {
            let line = &mut KEY_LINES[EDIT_LINE as usize];
            let len = key_line_len(line);
            if len == KEY_LINEPOS as usize {
                // mattx86: right arrow key fix
                let prev_line = (EDIT_LINE + 31) & 31;
                let prev_len = key_line_len(&KEY_LINES[prev_line as usize]);
                if prev_len >= KEY_LINEPOS as usize {
                    return;
                }
                line[KEY_LINEPOS as usize] =
                    KEY_LINES[prev_line as usize][KEY_LINEPOS as usize];
                KEY_LINEPOS += 1;
                if (KEY_LINEPOS as usize) < MAXCMDLINE {
                    line[KEY_LINEPOS as usize] = 0;
                }
            } else {
                KEY_LINEPOS = char_offset(line, KEY_LINEPOS + 1) as i32;
            }
            return;
        }

        // Up arrow — history
        if key == K_UPARROW || key == K_KP_UPARROW
            || (key == b'p' as i32 && KEYDOWN[K_CTRL as usize])
        {
            loop {
                HISTORY_LINE = (HISTORY_LINE - 1) & 31;
                if HISTORY_LINE == EDIT_LINE || KEY_LINES[HISTORY_LINE as usize][1] != 0 {
                    break;
                }
            }
            if HISTORY_LINE == EDIT_LINE {
                HISTORY_LINE = (EDIT_LINE + 1) & 31;
            }
            KEY_LINES[EDIT_LINE as usize] = KEY_LINES[HISTORY_LINE as usize];
            KEY_LINEPOS = key_line_len(&KEY_LINES[EDIT_LINE as usize]) as i32;
            return;
        }

        // Down arrow — history
        if key == K_DOWNARROW || key == K_KP_DOWNARROW
            || (key == b'n' as i32 && KEYDOWN[K_CTRL as usize])
        {
            if HISTORY_LINE == EDIT_LINE {
                return;
            }
            loop {
                HISTORY_LINE = (HISTORY_LINE + 1) & 31;
                if HISTORY_LINE == EDIT_LINE || KEY_LINES[HISTORY_LINE as usize][1] != 0 {
                    break;
                }
            }
            if HISTORY_LINE == EDIT_LINE {
                KEY_LINES[EDIT_LINE as usize][0] = b']';
                KEY_LINES[EDIT_LINE as usize][1] = 0;
                KEY_LINEPOS = 1;
            } else {
                KEY_LINES[EDIT_LINE as usize] = KEY_LINES[HISTORY_LINE as usize];
                KEY_LINEPOS = key_line_len(&KEY_LINES[EDIT_LINE as usize]) as i32;
            }
            return;
        }

        // Page up / mouse wheel up
        if key == K_PGUP || key == K_KP_PGUP || key == K_MWHEELUP {
            CON.display -= 3;
            return;
        }

        // Page down / mouse wheel down
        if key == K_PGDN || key == K_KP_PGDN || key == K_MWHEELDOWN {
            CON.display += 3;
            if CON.display > CON.current {
                CON.display = CON.current;
            }
            return;
        }

        // Home
        if key == K_HOME || key == K_KP_HOME {
            if KEYDOWN[K_CTRL as usize] {
                CON.display = CON.current - CON.totallines + 10;
            } else {
                KEY_LINEPOS = 1;
            }
            return;
        }

        // End
        if key == K_END || key == K_KP_END {
            if KEYDOWN[K_CTRL as usize] {
                CON.display = CON.current;
            } else {
                KEY_LINEPOS = key_line_len(&KEY_LINES[EDIT_LINE as usize]) as i32;
            }
            return;
        }

        // Non-printable
        if !(32..=127).contains(&key) {
            return;
        }

        // Insert character
        if (KEY_LINEPOS as usize) < MAXCMDLINE - 1 {
            let line = &mut KEY_LINES[EDIT_LINE as usize];

            if KEY_INSERT {
                let mut i = key_line_len(line);
                if i == 254 {
                    i -= 1;
                }
                while i >= KEY_LINEPOS as usize {
                    if i + 1 < MAXCMDLINE {
                        line[i + 1] = line[i];
                    }
                    if i == 0 {
                        break;
                    }
                    i -= 1;
                }
            }

            let old = line[KEY_LINEPOS as usize];
            line[KEY_LINEPOS as usize] = key as u8;
            KEY_LINEPOS += 1;
            if old == 0 && (KEY_LINEPOS as usize) < MAXCMDLINE {
                line[KEY_LINEPOS as usize] = 0;
            }
        }
    }
}

// ============================================================
// Key_Message
// ============================================================

/// Handle key input in message (chat) mode.
pub fn key_message(key: i32) {
    // SAFETY: single-threaded engine
    unsafe {
        if key == K_ENTER || key == K_KP_ENTER {
            match CHAT_TYPE {
                CT_PERSON => cbuf_add_text("say_person "),
                CT_TELL => cbuf_add_text("tell "),
                CT_TEAM => cbuf_add_text("say_team \""),
                _ => cbuf_add_text("say \""),
            }

            let chat_str: String = CHAT_BUFFER[..CHAT_BUFFERLEN as usize]
                .iter()
                .take_while(|&&b| b != 0)
                .map(|&b| b as char)
                .collect();
            cbuf_add_text(&chat_str);

            if CHAT_TYPE != CT_PERSON && CHAT_TYPE != CT_TELL {
                cbuf_add_text("\"");
            }
            cbuf_add_text("\n");

            CLS.key_dest = KeyDest::Game;
            CHAT_BUFFERLEN = 0;
            CHAT_BUFFER[0] = 0;
            CHAT_BACKEDIT = 0;
            return;
        }

        if key == K_ESCAPE {
            CLS.key_dest = KeyDest::Game;
            CHAT_BUFFERLEN = 0;
            CHAT_BUFFER[0] = 0;
            CHAT_BACKEDIT = 0;
            return;
        }

        if key == K_BACKSPACE {
            if CHAT_BUFFERLEN > 0 {
                if CHAT_BACKEDIT != 0 {
                    let start = (CHAT_BUFFERLEN - CHAT_BACKEDIT - 1) as usize;
                    if CHAT_BUFFERLEN - CHAT_BACKEDIT == 0 {
                        return;
                    }
                    for i in start..CHAT_BUFFERLEN as usize {
                        CHAT_BUFFER[i] = if i + 1 < MAXCMDLINE { CHAT_BUFFER[i + 1] } else { 0 };
                    }
                    CHAT_BUFFERLEN -= 1;
                    CHAT_BUFFER[CHAT_BUFFERLEN as usize] = 0;
                } else {
                    CHAT_BUFFERLEN -= 1;
                    CHAT_BUFFER[CHAT_BUFFERLEN as usize] = 0;
                }
            }
            return;
        }

        if key == K_DEL {
            if CHAT_BUFFERLEN > 0 && CHAT_BACKEDIT > 0 {
                let start = (CHAT_BUFFERLEN - CHAT_BACKEDIT) as usize;
                for i in start..CHAT_BUFFERLEN as usize {
                    CHAT_BUFFER[i] = if i + 1 < MAXCMDLINE { CHAT_BUFFER[i + 1] } else { 0 };
                }
                CHAT_BACKEDIT -= 1;
                CHAT_BUFFERLEN -= 1;
                CHAT_BUFFER[CHAT_BUFFERLEN as usize] = 0;
            }
            return;
        }

        if key == K_LEFTARROW {
            if CHAT_BUFFERLEN > 0 {
                CHAT_BACKEDIT += 1;
                if CHAT_BACKEDIT > CHAT_BUFFERLEN {
                    CHAT_BACKEDIT = CHAT_BUFFERLEN;
                }
                if CHAT_BACKEDIT < 0 {
                    CHAT_BACKEDIT = 0;
                }
            }
            return;
        }

        if key == K_RIGHTARROW {
            if CHAT_BUFFERLEN > 0 {
                CHAT_BACKEDIT -= 1;
                if CHAT_BACKEDIT > CHAT_BUFFERLEN {
                    CHAT_BACKEDIT = CHAT_BUFFERLEN;
                }
                if CHAT_BACKEDIT < 0 {
                    CHAT_BACKEDIT = 0;
                }
            }
            return;
        }

        // non printable
        if !(32..=127).contains(&key) {
            return;
        }
        // all full
        if CHAT_BUFFERLEN as usize == MAXCMDLINE - 1 {
            return;
        }

        if CHAT_BACKEDIT != 0 {
            // insert character
            let mut i = CHAT_BUFFERLEN as usize;
            let insert_pos = (CHAT_BUFFERLEN - CHAT_BACKEDIT) as usize;
            while i > insert_pos {
                if i < MAXCMDLINE {
                    CHAT_BUFFER[i] = CHAT_BUFFER[i - 1];
                }
                i -= 1;
            }
            CHAT_BUFFER[insert_pos] = key as u8;
            CHAT_BUFFERLEN += 1;
            CHAT_BUFFER[CHAT_BUFFERLEN as usize] = 0;
        } else {
            CHAT_BUFFER[CHAT_BUFFERLEN as usize] = key as u8;
            CHAT_BUFFERLEN += 1;
            CHAT_BUFFER[CHAT_BUFFERLEN as usize] = 0;
        }
    }
}

// ============================================================
// Key_StringToKeynum
// ============================================================

/// Returns a key number from a key name string.
/// Single ascii characters return themselves.
pub fn key_string_to_keynum(str_key: &str) -> i32 {
    if str_key.is_empty() {
        return -1;
    }
    if str_key.len() == 1 {
        return str_key.as_bytes()[0] as i32;
    }

    for kn in KEYNAMES.iter() {
        if kn.name.eq_ignore_ascii_case(str_key) {
            return kn.keynum;
        }
    }
    -1
}

// ============================================================
// Key_KeynumToString
// ============================================================

/// Returns a string for the given keynum.
pub fn key_keynum_to_string(keynum: i32) -> String {
    if keynum == -1 {
        return "<KEY NOT FOUND>".to_string();
    }
    if keynum > 32 && keynum < 127 {
        return String::from(keynum as u8 as char);
    }

    for kn in KEYNAMES.iter() {
        if keynum == kn.keynum {
            return kn.name.to_string();
        }
    }

    "<UNKNOWN KEYNUM>".to_string()
}

// ============================================================
// Key_SetBinding
// ============================================================

/// Set a key binding.
pub fn key_set_binding(keynum: i32, binding: &str) {
    if !(0..256).contains(&keynum) {
        return;
    }

    // SAFETY: single-threaded engine
    unsafe {
        if binding.is_empty() {
            KEYBINDINGS[keynum as usize] = None;
        } else {
            KEYBINDINGS[keynum as usize] = Some(binding.to_string());
        }
    }
}

// ============================================================
// Key_Unbind_f
// ============================================================

fn key_unbind_f() {
    if cmd_argc() != 2 {
        com_printf("unbind <key> : remove commands from a key\n");
        return;
    }

    let b = key_string_to_keynum(&cmd_argv(1));
    if b == -1 {
        com_printf(&format!("\"{}\" isn't a valid key\n", cmd_argv(1)));
        return;
    }

    key_set_binding(b, "");
}

fn key_unbindall_f() {
    for i in 0..256 {
        // SAFETY: single-threaded engine
        unsafe {
            if KEYBINDINGS[i].is_some() {
                key_set_binding(i as i32, "");
            }
        }
    }
}

// ============================================================
// Key_Bindlist_f
// ============================================================

fn key_bindlist_f() {
    let s = if cmd_argc() == 2 {
        cmd_argv(1)
    } else {
        "*".to_string()
    };

    let mut j = 0;
    let mut k = 0;
    for i in 0..256 {
        let t = key_keynum_to_string(i);
        if !wildcardfit("<*>", &t) {
            if wildcardfit(&s, &t) {
                // SAFETY: single-threaded engine
                unsafe {
                    if let Some(ref binding) = KEYBINDINGS[i as usize] {
                        com_printf(&format!("{} \"{}\"\n", t, binding));
                    } else {
                        com_printf(&format!("{} \"\"\n", t));
                    }
                }
                k += 1;
            }
            j += 1;
        }
    }

    com_printf(&format!("{} binds, {} matching\n", j, k));
}

// ============================================================
// Key_Bind_f
// ============================================================

fn key_bind_f() {
    let c = cmd_argc();

    if c <= 2 {
        key_bindlist_f(); // mattx86
        return;
    }

    let b = key_string_to_keynum(&cmd_argv(1));
    if b == -1 {
        com_printf(&format!("\"{}\" isn't a valid key\n", cmd_argv(1)));
        return;
    }

    if c == 2 {
        // SAFETY: single-threaded engine
        unsafe {
            if let Some(ref binding) = KEYBINDINGS[b as usize] {
                com_printf(&format!("\"{}\" = \"{}\"\n", cmd_argv(1), binding));
            } else {
                com_printf(&format!("\"{}\" is not bound\n", cmd_argv(1)));
            }
        }
        return;
    }

    // copy the rest of the command line
    let mut cmd = String::new();
    for i in 2..c {
        cmd.push_str(&cmd_argv(i));
        if i != c - 1 {
            cmd.push(' ');
        }
    }

    key_set_binding(b, &cmd);
}

// ============================================================
// Key_WriteBindings
// ============================================================

/// Write all key bindings to a file.
pub fn key_write_bindings(f: &mut dyn std::io::Write) {
    // SAFETY: single-threaded engine
    unsafe {
        for i in 0..256 {
            if let Some(ref binding) = KEYBINDINGS[i] {
                if !binding.is_empty() {
                    let _ = writeln!(f, "bind {} \"{}\"", key_keynum_to_string(i as i32), binding);
                }
            }
        }
    }
}

// ============================================================
// Key_Init
// ============================================================

/// Initialize the key system.
pub fn key_init() {
    // SAFETY: single-threaded engine
    unsafe {
        for i in 0..32 {
            KEY_LINES[i][0] = b']';
            KEY_LINES[i][1] = 0;
        }
        KEY_LINEPOS = 1;

        // init ascii characters in console mode
        for i in 32..128 {
            CONSOLEKEYS[i] = true;
        }
        CONSOLEKEYS[K_ENTER as usize] = true;
        CONSOLEKEYS[K_KP_ENTER as usize] = true;
        CONSOLEKEYS[K_TAB as usize] = true;
        CONSOLEKEYS[K_LEFTARROW as usize] = true;
        CONSOLEKEYS[K_KP_LEFTARROW as usize] = true;
        CONSOLEKEYS[K_RIGHTARROW as usize] = true;
        CONSOLEKEYS[K_KP_RIGHTARROW as usize] = true;
        CONSOLEKEYS[K_UPARROW as usize] = true;
        CONSOLEKEYS[K_KP_UPARROW as usize] = true;
        CONSOLEKEYS[K_DOWNARROW as usize] = true;
        CONSOLEKEYS[K_KP_DOWNARROW as usize] = true;
        CONSOLEKEYS[K_BACKSPACE as usize] = true;
        CONSOLEKEYS[K_DEL as usize] = true;
        CONSOLEKEYS[K_HOME as usize] = true;
        CONSOLEKEYS[K_KP_HOME as usize] = true;
        CONSOLEKEYS[K_END as usize] = true;
        CONSOLEKEYS[K_KP_END as usize] = true;
        CONSOLEKEYS[K_PGUP as usize] = true;
        CONSOLEKEYS[K_KP_PGUP as usize] = true;
        CONSOLEKEYS[K_PGDN as usize] = true;
        CONSOLEKEYS[K_KP_PGDN as usize] = true;
        CONSOLEKEYS[K_SHIFT as usize] = true;
        CONSOLEKEYS[K_INS as usize] = true;
        CONSOLEKEYS[K_KP_INS as usize] = true;
        CONSOLEKEYS[K_KP_DEL as usize] = true;
        CONSOLEKEYS[K_KP_SLASH as usize] = true;
        CONSOLEKEYS[K_KP_PLUS as usize] = true;
        CONSOLEKEYS[K_KP_MINUS as usize] = true;
        CONSOLEKEYS[K_KP_5 as usize] = true;

        // mattx86: mouse_wheel
        CONSOLEKEYS[K_MWHEELUP as usize] = true;
        CONSOLEKEYS[K_MWHEELDOWN as usize] = true;

        CONSOLEKEYS[b'`' as usize] = false;
        CONSOLEKEYS[b'~' as usize] = false;

        for i in 0..256 {
            KEYSHIFT[i] = i as i32;
        }
        for i in b'a'..=b'z' {
            KEYSHIFT[i as usize] = (i - b'a' + b'A') as i32;
        }
        KEYSHIFT[b'1' as usize] = b'!' as i32;
        KEYSHIFT[b'2' as usize] = b'@' as i32;
        KEYSHIFT[b'3' as usize] = b'#' as i32;
        KEYSHIFT[b'4' as usize] = b'$' as i32;
        KEYSHIFT[b'5' as usize] = b'%' as i32;
        KEYSHIFT[b'6' as usize] = b'^' as i32;
        KEYSHIFT[b'7' as usize] = b'&' as i32;
        KEYSHIFT[b'8' as usize] = b'*' as i32;
        KEYSHIFT[b'9' as usize] = b'(' as i32;
        KEYSHIFT[b'0' as usize] = b')' as i32;
        KEYSHIFT[b'-' as usize] = b'_' as i32;
        KEYSHIFT[b'=' as usize] = b'+' as i32;
        KEYSHIFT[b',' as usize] = b'<' as i32;
        KEYSHIFT[b'.' as usize] = b'>' as i32;
        KEYSHIFT[b'/' as usize] = b'?' as i32;
        KEYSHIFT[b';' as usize] = b':' as i32;
        KEYSHIFT[b'\'' as usize] = b'"' as i32;
        KEYSHIFT[b'[' as usize] = b'{' as i32;
        KEYSHIFT[b']' as usize] = b'}' as i32;
        KEYSHIFT[b'`' as usize] = b'~' as i32;
        KEYSHIFT[b'\\' as usize] = b'|' as i32;

        MENUBOUND[K_ESCAPE as usize] = true;
        for i in 0..12 {
            MENUBOUND[(K_F1 + i) as usize] = true;
        }

        // register our functions
        cmd_add_command("bind", key_bind_f);
        cmd_add_command("unbind", key_unbind_f);
        cmd_add_command("unbindall", key_unbindall_f);
        cmd_add_command("bindlist", key_bindlist_f);
    }
}

// ============================================================
// Key_Event
// ============================================================

/// Called by the system between frames for both key up and key down events.
/// Should NOT be called during an interrupt!
pub fn key_event(key: i32, down: bool, time: u32) {
    // SAFETY: single-threaded engine
    unsafe {
        // hack for modal presses
        if KEY_WAITING == -1 {
            if down {
                KEY_WAITING = key;
            }
            return;
        }

        // update auto-repeat status
        if down {
            if key >= 200
                && KEYBINDINGS[key as usize].is_none()
                && key != K_MWHEELUP
                && key != K_MWHEELDOWN
            {
                com_printf(&format!(
                    "{} is unbound, hit F4 to set.\n",
                    key_keynum_to_string(key)
                ));
            }
        } else {
            KEY_REPEATS[key as usize] = 0;
        }

        if key == K_SHIFT {
            SHIFT_DOWN = down;
        }

        // console key is hardcoded
        if key == b'`' as i32 || key == b'~' as i32 {
            if !down {
                return;
            }
            con_toggle_console_f();
            return;
        }

        // any key during attract mode will bring up the menu
        // mattx86: console_demos — USE_CONSOLE_IN_DEMOS, so skip attract key override
        let mut key = key;
        if !crate::console::USE_CONSOLE_IN_DEMOS
            && CLS.key_dest != KeyDest::Menu {
                // cl.attractloop check
                if !(K_F1..=K_F12).contains(&key) {
                    key = K_ESCAPE;
                }
            }

        // menu key is hardcoded
        if key == K_ESCAPE {
            if !down {
                return;
            }

            // If the player has a layout active (scoreboard/inventory) and we're
            // in game mode, pressing ESC sends "cmd putaway" to dismiss it.
            if CL.frame.playerstate.stats[STAT_LAYOUTS] != 0
                && CLS.key_dest == KeyDest::Game
            {
                myq2_common::cmd::cbuf_add_text("cmd putaway\n");
                return;
            }
            match CLS.key_dest {
                KeyDest::Message => {
                    key_message(key);
                }
                KeyDest::Menu => {
                    m_keydown(key);
                }
                KeyDest::Game | KeyDest::Console => {
                    m_menu_main_f();
                }
            }
            return;
        }

        // track if any key is down for BUTTON_ANY
        KEYDOWN[key as usize] = down;
        if down {
            if KEY_REPEATS[key as usize] == 1 {
                ANYKEYDOWN += 1;
            }
        } else {
            ANYKEYDOWN -= 1;
            if ANYKEYDOWN < 0 {
                ANYKEYDOWN = 0;
            }
        }

        // key up events only generate commands for button commands (leading +)
        if !down {
            if let Some(ref kb) = KEYBINDINGS[key as usize] {
                if kb.starts_with('+') {
                    let cmd = format!("-{} {} {}\n", &kb[1..], key, time);
                    cbuf_add_text(&cmd);
                }
            }
            if KEYSHIFT[key as usize] != key {
                let shifted = KEYSHIFT[key as usize] as usize;
                if let Some(ref kb) = KEYBINDINGS[shifted] {
                    if kb.starts_with('+') {
                        let cmd = format!("-{} {} {}\n", &kb[1..], key, time);
                        cbuf_add_text(&cmd);
                    }
                }
            }
            return;
        }

        // if not a consolekey, send to the interpreter
        if (CLS.key_dest == KeyDest::Menu && MENUBOUND[key as usize])
            || (CLS.key_dest == KeyDest::Console && !CONSOLEKEYS[key as usize])
            || (CLS.key_dest == KeyDest::Game
                && (CLS.state == ConnState::Active || !CONSOLEKEYS[key as usize]))
        {
            if let Some(ref kb) = KEYBINDINGS[key as usize] {
                if kb.starts_with('+') {
                    let cmd = format!("{} {} {}\n", kb, key, time);
                    cbuf_add_text(&cmd);
                } else {
                    cbuf_add_text(kb);
                    cbuf_add_text("\n");
                }
            }
            return;
        }

        if !down {
            return;
        }

        let mut key = key;
        if SHIFT_DOWN {
            key = KEYSHIFT[key as usize];
        }

        match CLS.key_dest {
            KeyDest::Message => {
                key_message(key);
            }
            KeyDest::Menu => {
                m_keydown(key);
            }
            KeyDest::Game | KeyDest::Console => {
                key_console(key);
            }
        }
    }
}

// ============================================================
// Key_ClearStates
// ============================================================

/// Clear all key states.
pub fn key_clear_states() {
    // SAFETY: single-threaded engine
    unsafe {
        ANYKEYDOWN = 0;

        for i in 0..256 {
            if KEYDOWN[i] || KEY_REPEATS[i] != 0 {
                key_event(i as i32, false, 0);
            }
            KEYDOWN[i] = false;
            KEY_REPEATS[i] = 0;
        }
    }
}

// ============================================================
// Key_GetKey
// ============================================================

/// Returns true if the given key is currently held down.
pub fn key_is_down(key: i32) -> bool {
    if key < 0 || key >= 256 {
        return false;
    }
    // SAFETY: single-threaded engine
    unsafe { KEYDOWN[key as usize] }
}

/// Wait for a key press and return it.
pub fn key_get_key() -> i32 {
    // SAFETY: single-threaded engine
    unsafe {
        KEY_WAITING = -1;

        while KEY_WAITING == -1 {
            sys_send_key_events();
        }

        KEY_WAITING
    }
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // key_string_to_keynum tests
    // ============================================================

    #[test]
    fn test_key_string_to_keynum_empty() {
        assert_eq!(key_string_to_keynum(""), -1);
    }

    #[test]
    fn test_key_string_to_keynum_single_ascii() {
        // Single character returns its ASCII value
        assert_eq!(key_string_to_keynum("a"), b'a' as i32);
        assert_eq!(key_string_to_keynum("Z"), b'Z' as i32);
        assert_eq!(key_string_to_keynum("1"), b'1' as i32);
        assert_eq!(key_string_to_keynum(" "), b' ' as i32);
    }

    #[test]
    fn test_key_string_to_keynum_named_keys() {
        assert_eq!(key_string_to_keynum("TAB"), K_TAB);
        assert_eq!(key_string_to_keynum("ENTER"), K_ENTER);
        assert_eq!(key_string_to_keynum("ESCAPE"), K_ESCAPE);
        assert_eq!(key_string_to_keynum("SPACE"), K_SPACE);
        assert_eq!(key_string_to_keynum("BACKSPACE"), K_BACKSPACE);
        assert_eq!(key_string_to_keynum("UPARROW"), K_UPARROW);
        assert_eq!(key_string_to_keynum("DOWNARROW"), K_DOWNARROW);
        assert_eq!(key_string_to_keynum("LEFTARROW"), K_LEFTARROW);
        assert_eq!(key_string_to_keynum("RIGHTARROW"), K_RIGHTARROW);
    }

    #[test]
    fn test_key_string_to_keynum_case_insensitive() {
        assert_eq!(key_string_to_keynum("tab"), K_TAB);
        assert_eq!(key_string_to_keynum("Tab"), K_TAB);
        assert_eq!(key_string_to_keynum("enter"), K_ENTER);
        assert_eq!(key_string_to_keynum("Escape"), K_ESCAPE);
        assert_eq!(key_string_to_keynum("f1"), K_F1);
        assert_eq!(key_string_to_keynum("F12"), K_F12);
    }

    #[test]
    fn test_key_string_to_keynum_modifier_keys() {
        assert_eq!(key_string_to_keynum("ALT"), K_ALT);
        assert_eq!(key_string_to_keynum("CTRL"), K_CTRL);
        assert_eq!(key_string_to_keynum("SHIFT"), K_SHIFT);
    }

    #[test]
    fn test_key_string_to_keynum_function_keys() {
        assert_eq!(key_string_to_keynum("F1"), K_F1);
        assert_eq!(key_string_to_keynum("F2"), K_F2);
        assert_eq!(key_string_to_keynum("F10"), K_F10);
        assert_eq!(key_string_to_keynum("F11"), K_F11);
        assert_eq!(key_string_to_keynum("F12"), K_F12);
    }

    #[test]
    fn test_key_string_to_keynum_navigation_keys() {
        assert_eq!(key_string_to_keynum("INS"), K_INS);
        assert_eq!(key_string_to_keynum("DEL"), K_DEL);
        assert_eq!(key_string_to_keynum("PGDN"), K_PGDN);
        assert_eq!(key_string_to_keynum("PGUP"), K_PGUP);
        assert_eq!(key_string_to_keynum("HOME"), K_HOME);
        assert_eq!(key_string_to_keynum("END"), K_END);
    }

    #[test]
    fn test_key_string_to_keynum_mouse_buttons() {
        assert_eq!(key_string_to_keynum("MOUSE1"), K_MOUSE1);
        assert_eq!(key_string_to_keynum("MOUSE2"), K_MOUSE2);
        assert_eq!(key_string_to_keynum("MOUSE3"), K_MOUSE3);
        assert_eq!(key_string_to_keynum("MOUSE4"), K_MOUSE4);
        assert_eq!(key_string_to_keynum("MOUSE5"), K_MOUSE5);
    }

    #[test]
    fn test_key_string_to_keynum_mouse_wheel() {
        assert_eq!(key_string_to_keynum("MWHEELUP"), K_MWHEELUP);
        assert_eq!(key_string_to_keynum("MWHEELDOWN"), K_MWHEELDOWN);
    }

    #[test]
    fn test_key_string_to_keynum_keypad_keys() {
        assert_eq!(key_string_to_keynum("KP_HOME"), K_KP_HOME);
        assert_eq!(key_string_to_keynum("KP_UPARROW"), K_KP_UPARROW);
        assert_eq!(key_string_to_keynum("KP_PGUP"), K_KP_PGUP);
        assert_eq!(key_string_to_keynum("KP_LEFTARROW"), K_KP_LEFTARROW);
        assert_eq!(key_string_to_keynum("KP_5"), K_KP_5);
        assert_eq!(key_string_to_keynum("KP_RIGHTARROW"), K_KP_RIGHTARROW);
        assert_eq!(key_string_to_keynum("KP_END"), K_KP_END);
        assert_eq!(key_string_to_keynum("KP_DOWNARROW"), K_KP_DOWNARROW);
        assert_eq!(key_string_to_keynum("KP_PGDN"), K_KP_PGDN);
        assert_eq!(key_string_to_keynum("KP_ENTER"), K_KP_ENTER);
        assert_eq!(key_string_to_keynum("KP_INS"), K_KP_INS);
        assert_eq!(key_string_to_keynum("KP_DEL"), K_KP_DEL);
        assert_eq!(key_string_to_keynum("KP_SLASH"), K_KP_SLASH);
        assert_eq!(key_string_to_keynum("KP_MINUS"), K_KP_MINUS);
        assert_eq!(key_string_to_keynum("KP_PLUS"), K_KP_PLUS);
    }

    #[test]
    fn test_key_string_to_keynum_semicolon() {
        assert_eq!(key_string_to_keynum("SEMICOLON"), b';' as i32);
    }

    #[test]
    fn test_key_string_to_keynum_pause() {
        assert_eq!(key_string_to_keynum("PAUSE"), K_PAUSE);
    }

    #[test]
    fn test_key_string_to_keynum_joy_keys() {
        assert_eq!(key_string_to_keynum("JOY1"), K_JOY1);
        assert_eq!(key_string_to_keynum("JOY2"), K_JOY2);
        assert_eq!(key_string_to_keynum("JOY3"), K_JOY3);
        assert_eq!(key_string_to_keynum("JOY4"), K_JOY4);
    }

    #[test]
    fn test_key_string_to_keynum_aux_keys() {
        assert_eq!(key_string_to_keynum("AUX1"), K_AUX1);
        assert_eq!(key_string_to_keynum("AUX16"), K_AUX16);
        assert_eq!(key_string_to_keynum("AUX32"), K_AUX32);
    }

    #[test]
    fn test_key_string_to_keynum_unknown() {
        assert_eq!(key_string_to_keynum("NONEXISTENT"), -1);
        assert_eq!(key_string_to_keynum("FOOBAR"), -1);
    }

    // ============================================================
    // key_keynum_to_string tests
    // ============================================================

    #[test]
    fn test_key_keynum_to_string_not_found() {
        assert_eq!(key_keynum_to_string(-1), "<KEY NOT FOUND>");
    }

    #[test]
    fn test_key_keynum_to_string_printable_ascii() {
        // 33..127 return the character
        assert_eq!(key_keynum_to_string(b'A' as i32), "A");
        assert_eq!(key_keynum_to_string(b'z' as i32), "z");
        assert_eq!(key_keynum_to_string(b'5' as i32), "5");
        assert_eq!(key_keynum_to_string(b'!' as i32), "!");
        assert_eq!(key_keynum_to_string(b'~' as i32), "~");
    }

    #[test]
    fn test_key_keynum_to_string_named_keys() {
        assert_eq!(key_keynum_to_string(K_TAB), "TAB");
        assert_eq!(key_keynum_to_string(K_ENTER), "ENTER");
        assert_eq!(key_keynum_to_string(K_ESCAPE), "ESCAPE");
        assert_eq!(key_keynum_to_string(K_SPACE), "SPACE");
        assert_eq!(key_keynum_to_string(K_BACKSPACE), "BACKSPACE");
        assert_eq!(key_keynum_to_string(K_UPARROW), "UPARROW");
        assert_eq!(key_keynum_to_string(K_F1), "F1");
        assert_eq!(key_keynum_to_string(K_F12), "F12");
        assert_eq!(key_keynum_to_string(K_MOUSE1), "MOUSE1");
        assert_eq!(key_keynum_to_string(K_MWHEELUP), "MWHEELUP");
        assert_eq!(key_keynum_to_string(K_PAUSE), "PAUSE");
    }

    #[test]
    fn test_key_keynum_to_string_unknown_keynum() {
        // A keynum that is neither printable ASCII nor in the table
        assert_eq!(key_keynum_to_string(1), "<UNKNOWN KEYNUM>");
        assert_eq!(key_keynum_to_string(0), "<UNKNOWN KEYNUM>");
    }

    #[test]
    fn test_key_string_to_keynum_roundtrip() {
        // Named keys should roundtrip
        let names = [
            "TAB", "ENTER", "ESCAPE", "SPACE", "BACKSPACE",
            "UPARROW", "DOWNARROW", "LEFTARROW", "RIGHTARROW",
            "ALT", "CTRL", "SHIFT",
            "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12",
            "INS", "DEL", "PGDN", "PGUP", "HOME", "END",
            "MOUSE1", "MOUSE2", "MOUSE3", "MOUSE4", "MOUSE5",
            "KP_ENTER", "KP_SLASH", "KP_PLUS", "KP_MINUS",
            "MWHEELUP", "MWHEELDOWN", "PAUSE",
        ];
        for name in &names {
            let keynum = key_string_to_keynum(name);
            assert_ne!(keynum, -1, "key_string_to_keynum({}) should not be -1", name);
            let back = key_keynum_to_string(keynum);
            assert_eq!(&back, name, "roundtrip failed for {}", name);
        }
    }

    // ============================================================
    // key_set_binding tests
    // ============================================================

    #[test]
    fn test_key_set_binding_basic() {
        unsafe {
            KEYBINDINGS[b'x' as usize] = None;
        }
        key_set_binding(b'x' as i32, "+attack");
        unsafe {
            assert_eq!(KEYBINDINGS[b'x' as usize].as_deref(), Some("+attack"));
        }
    }

    #[test]
    fn test_key_set_binding_empty_clears() {
        unsafe {
            KEYBINDINGS[b'y' as usize] = Some("jump".to_string());
        }
        key_set_binding(b'y' as i32, "");
        unsafe {
            assert!(KEYBINDINGS[b'y' as usize].is_none());
        }
    }

    #[test]
    fn test_key_set_binding_out_of_range() {
        // Should not panic or modify anything
        key_set_binding(-1, "test");
        key_set_binding(256, "test");
        key_set_binding(300, "test");
    }

    #[test]
    fn test_key_set_binding_replace() {
        key_set_binding(b'z' as i32, "first");
        key_set_binding(b'z' as i32, "second");
        unsafe {
            assert_eq!(KEYBINDINGS[b'z' as usize].as_deref(), Some("second"));
        }
        // Cleanup
        key_set_binding(b'z' as i32, "");
    }

    // ============================================================
    // key_is_down tests
    // ============================================================

    #[test]
    fn test_key_is_down_out_of_range() {
        assert!(!key_is_down(-1));
        assert!(!key_is_down(256));
        assert!(!key_is_down(1000));
    }

    #[test]
    fn test_key_is_down_initially_false() {
        // Most keys should not be down by default
        // (may be modified by other tests, but basic sanity)
        // Pick a less-used key index
        unsafe { KEYDOWN[255] = false; }
        assert!(!key_is_down(255));
    }

    #[test]
    fn test_key_is_down_after_setting() {
        unsafe {
            KEYDOWN[254] = true;
        }
        assert!(key_is_down(254));
        unsafe {
            KEYDOWN[254] = false;
        }
        assert!(!key_is_down(254));
    }

    // ============================================================
    // key_line_len tests
    // ============================================================

    #[test]
    fn test_key_line_len_null_terminated() {
        let mut line = [0u8; MAXCMDLINE];
        line[0] = b']';
        line[1] = b'h';
        line[2] = b'e';
        line[3] = b'l';
        line[4] = 0;
        assert_eq!(key_line_len(&line), 4);
    }

    #[test]
    fn test_key_line_len_full_line() {
        let line = [b'A'; MAXCMDLINE]; // no null
        assert_eq!(key_line_len(&line), MAXCMDLINE);
    }

    #[test]
    fn test_key_line_len_empty() {
        let mut line = [0u8; MAXCMDLINE];
        assert_eq!(key_line_len(&line), 0);
    }

    // ============================================================
    // char_offset tests (keys.rs local version)
    // ============================================================

    #[test]
    fn test_keys_char_offset_basic() {
        let s = b"test\0rest";
        assert_eq!(char_offset(s, 0), 0);
        assert_eq!(char_offset(s, 2), 2);
        assert_eq!(char_offset(s, 4), 4); // stops at null
        assert_eq!(char_offset(s, 10), 4); // can't go past null
    }

    #[test]
    fn test_keys_char_offset_no_null() {
        let s = b"ABCD";
        assert_eq!(char_offset(s, 4), 4);
        assert_eq!(char_offset(s, 5), 4); // at end of slice
    }

    // ============================================================
    // Key constants sanity tests
    // ============================================================

    #[test]
    fn test_key_constants_distinct_values() {
        // Ensure arrow keys have distinct values
        let arrows = [K_UPARROW, K_DOWNARROW, K_LEFTARROW, K_RIGHTARROW];
        for i in 0..arrows.len() {
            for j in (i + 1)..arrows.len() {
                assert_ne!(arrows[i], arrows[j], "Arrow key constants must be distinct");
            }
        }
    }

    #[test]
    fn test_key_constants_function_keys_sequential() {
        assert_eq!(K_F2, K_F1 + 1);
        assert_eq!(K_F3, K_F1 + 2);
        assert_eq!(K_F12, K_F1 + 11);
    }

    #[test]
    fn test_key_constants_control_keys_in_range() {
        // All special keys should be >= 128 (non-ASCII)
        assert!(K_UPARROW >= 128);
        assert!(K_DOWNARROW >= 128);
        assert!(K_ALT >= 128);
        assert!(K_CTRL >= 128);
        assert!(K_SHIFT >= 128);
        assert!(K_F1 >= 128);
        assert!(K_F12 >= 128);
    }

    #[test]
    fn test_key_constants_ascii_keys() {
        assert_eq!(K_TAB, 9);
        assert_eq!(K_ENTER, 13);
        assert_eq!(K_ESCAPE, 27);
        assert_eq!(K_SPACE, 32);
        assert_eq!(K_BACKSPACE, 127);
    }

    // ============================================================
    // key_write_bindings test
    // ============================================================

    #[test]
    fn test_key_write_bindings() {
        // Set up a binding
        unsafe {
            KEYBINDINGS[b'q' as usize] = Some("+forward".to_string());
            KEYBINDINGS[b'w' as usize] = None;
        }
        let mut buf: Vec<u8> = Vec::new();
        key_write_bindings(&mut buf);
        let output = String::from_utf8(buf).unwrap();

        // Should contain the 'q' binding
        assert!(output.contains("bind q \"+forward\""), "output should contain q binding: {}", output);
        // 'w' has no binding so should not appear with a value
    }

    // ============================================================
    // KEYNAMES table completeness test
    // ============================================================

    #[test]
    fn test_keynames_table_has_no_duplicate_names() {
        for i in 0..KEYNAMES.len() {
            for j in (i + 1)..KEYNAMES.len() {
                assert_ne!(
                    KEYNAMES[i].name.to_uppercase(),
                    KEYNAMES[j].name.to_uppercase(),
                    "Duplicate key name: {} and {}",
                    KEYNAMES[i].name,
                    KEYNAMES[j].name
                );
            }
        }
    }

    #[test]
    fn test_keynames_table_has_no_duplicate_keynums() {
        for i in 0..KEYNAMES.len() {
            for j in (i + 1)..KEYNAMES.len() {
                assert_ne!(
                    KEYNAMES[i].keynum,
                    KEYNAMES[j].keynum,
                    "Duplicate keynum {} for names '{}' and '{}'",
                    KEYNAMES[i].keynum,
                    KEYNAMES[i].name,
                    KEYNAMES[j].name
                );
            }
        }
    }

    // ============================================================
    // Cleanup binding after tests
    // ============================================================

    #[test]
    fn test_key_set_binding_cleanup() {
        // Clean up any bindings we set in tests
        key_set_binding(b'x' as i32, "");
        key_set_binding(b'y' as i32, "");
        key_set_binding(b'z' as i32, "");
        key_set_binding(b'q' as i32, "");
    }
}
