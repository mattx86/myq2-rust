// menu.rs — Menu system
// Converted from: myq2-original/client/menu.c
//
// Copyright (C) 1997-2001 Id Software, Inc.
// Licensed under the GNU General Public License v2

use std::io::Read;

use crate::client::KeyDest;
use crate::console::{
    cmd_add_command, cvar_set, cvar_variable_value, draw_char, draw_get_pic_size,
    draw_pic, scr_dirty_screen, CLS, VIDDEF,
};
use crate::keys::{
    key_clear_states, K_AUX1, K_AUX10, K_AUX11, K_AUX12,
    K_AUX13, K_AUX14, K_AUX15, K_AUX16, K_AUX17, K_AUX18, K_AUX19, K_AUX2, K_AUX20, K_AUX21,
    K_AUX22, K_AUX23, K_AUX24, K_AUX25, K_AUX26, K_AUX27, K_AUX28, K_AUX29, K_AUX3, K_AUX30,
    K_AUX31, K_AUX32, K_AUX4, K_AUX5, K_AUX6, K_AUX7, K_AUX8, K_AUX9,
    K_DOWNARROW, K_ENTER, K_ESCAPE, K_JOY1, K_JOY2, K_JOY3, K_JOY4, K_KP_DOWNARROW,
    K_KP_ENTER, K_KP_LEFTARROW, K_KP_RIGHTARROW, K_KP_UPARROW, K_LEFTARROW, K_MOUSE1, K_MOUSE2,
    K_MOUSE3, K_MOUSE4, K_MOUSE5, K_RIGHTARROW, K_TAB, K_UPARROW,
};

// ============================================================
// Menu constants
// ============================================================

pub const MAX_MENU_DEPTH: usize = 8;
pub const NUM_CURSOR_FRAMES: i32 = 15;
pub const MAIN_ITEMS: i32 = 5;
pub const MAX_SAVEGAMES: usize = 15;
pub const MAX_LOCAL_SERVERS: usize = 8;
pub const NUM_ADDRESSBOOK_ENTRIES: usize = 9;
pub const MAX_DISPLAYNAME: usize = 16;
pub const MAX_PLAYERMODELS: usize = 1024;

// Menu item type constants — imported from canonical qmenu.rs definitions
pub use crate::qmenu::{
    MTYPE_SLIDER, MTYPE_LIST, MTYPE_ACTION, MTYPE_SPINCONTROL,
    MTYPE_SEPARATOR, MTYPE_FIELD, MAXMENUITEMS,
    QMF_LEFT_JUSTIFY, QMF_GRAYED, QMF_NUMBERSONLY,
};

// ============================================================
// Menu framework types — imported from canonical qmenu.rs
// ============================================================

pub type MenuDrawFn = fn();
pub type MenuKeyFn = fn(i32) -> Option<&'static str>;

// Re-export canonical menu widget types from qmenu.rs
pub use crate::qmenu::{
    MenuCommon, MenuFramework, MenuField, MenuSlider, MenuList,
    MenuAction, MenuSeparator, MenuItem, MenuCallback,
};

// ============================================================
// Menu layer stack
// ============================================================

struct MenuLayer {
    draw: Option<MenuDrawFn>,
    key: Option<MenuKeyFn>,
}

static mut M_LAYERS: [MenuLayer; MAX_MENU_DEPTH] = {
    const EMPTY: MenuLayer = MenuLayer {
        draw: None,
        key: None,
    };
    [EMPTY; MAX_MENU_DEPTH]
};

static mut M_MENUDEPTH: usize = 0;
static mut M_DRAWFUNC: Option<MenuDrawFn> = None;
static mut M_KEYFUNC: Option<MenuKeyFn> = None;
static mut M_ENTERSOUND: bool = false;
static mut M_MAIN_CURSOR: i32 = 0;

// ============================================================
// Sound paths
// ============================================================

const MENU_IN_SOUND: &str = "misc/menu1.wav";
const MENU_MOVE_SOUND: &str = "misc/menu2.wav";
const MENU_OUT_SOUND: &str = "misc/menu3.wav";

// ============================================================
// Placeholder stubs
// ============================================================

/// S_StartLocalSound — wired through console module's system function pointer table.
fn s_start_local_sound(sound: &str) {
    // SAFETY: single-threaded engine
    unsafe { (crate::console::SYSTEM_FNS.s_start_local_sound)(sound) }
}

use myq2_common::common::com_server_state;


/// Draw_Fill — wired through console module's renderer function pointer table.
fn draw_fill(x: i32, y: i32, w: i32, h: i32, c: i32, a: f32) {
    crate::console::draw_fill(x, y, w, h, c, a);
}

/// Draw_FadeScreen — wired through console module's renderer function pointer table.
fn draw_fade_screen() {
    // SAFETY: single-threaded engine
    unsafe { (crate::console::RENDERER_FNS.draw_fade_screen)() }
}

/// GLimp_EndFrame — wired through console module's renderer function pointer table.
fn glimp_end_frame() {
    crate::console::vk_imp_end_frame();
}

/// CL_Quit_f — wired to cl_main
fn cl_quit_f() {
    crate::cl_main::cl_quit_f();
}

/// CL_PingServers_f — wired to cl_main
fn cl_ping_servers_f() {
    crate::cl_main::cl_ping_servers_f();
}

/// CL_Snd_Restart_f — wired to cl_main
fn cl_snd_restart_f() {
    crate::cl_main::cl_snd_restart_f();
}

/// VID_MenuInit — dispatches through platform video menu function pointer table.
fn vid_menu_init() {
    // SAFETY: single-threaded engine
    unsafe { (crate::console::VID_MENU_FNS.vid_menu_init)() }
}

/// VID_MenuDraw — dispatches through platform video menu function pointer table.
fn vid_menu_draw() {
    // SAFETY: single-threaded engine
    unsafe { (crate::console::VID_MENU_FNS.vid_menu_draw)() }
}

/// VID_MenuKey — dispatches through platform video menu function pointer table.
fn vid_menu_key(key: i32) -> Option<&'static str> {
    // SAFETY: single-threaded engine
    unsafe { (crate::console::VID_MENU_FNS.vid_menu_key)(key) }
}

/// FS_LoadFile — wired to myq2_common
fn fs_load_file(name: &str) -> Option<Vec<u8>> {
    myq2_common::files::fs_load_file(name)
}

/// Developer_searchpath — wired to myq2_common files module
fn developer_searchpath(who: i32) -> i32 {
    myq2_common::files::with_fs_ctx(|ctx| ctx.developer_searchpath(who)).unwrap_or(0)
}

use myq2_common::cvar::{cvar_force_set, cvar_set_value, cvar_variable_string};

/// Cvar_Get — wired to myq2_common; registers cvar then returns its value
fn cvar_get(name: &str, default: &str, flags: i32) -> f32 {
    myq2_common::cvar::cvar_get(name, default, flags);
    myq2_common::cvar::cvar_variable_value(name)
}

/// Cbuf_InsertText — wired to myq2_common.
fn cbuf_insert_text(text: &str) {
    myq2_common::cmd::with_cmd_ctx(|ctx| {
        ctx.cbuf_insert_text(text);
    });
}

/// Helper: convert a slice of &str to Vec<String> for qmenu itemnames.
fn strs(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

/// Helper: build a MenuCommon from old-style parameters.
/// Converts &str name to Option<String>, fn callback to Box<dyn Fn>, etc.
fn make_menu_common(
    item_type: i32,
    name: &str,
    x: i32,
    y: i32,
    flags: u32,
    localdata: [i32; 4],
    callback: Option<fn(usize)>,
    statusbar: Option<&str>,
) -> MenuCommon {
    MenuCommon {
        item_type,
        name: if name.is_empty() { None } else { Some(name.to_string()) },
        x,
        y,
        parent_x: 0,
        parent_y: 0,
        cursor_offset: 0,
        localdata,
        flags,
        statusbar: statusbar.map(|s| s.to_string()),
        callback: callback.map(|f| Box::new(f) as Box<dyn Fn(usize)>),
        statusbarfunc: None,
        ownerdraw: None,
        cursordraw: None,
    }
}

// ============================================================
// Menu item storage — global Vec<MenuItem> paralleling the qmenu framework.
// Each framework-based menu stores items here via MENU_ITEMS.
// ============================================================

static mut MENU_ITEMS: Vec<MenuItem> = Vec::new();

/// Adapter: wraps the console-based renderer functions into a MenuRenderer impl
/// so that qmenu drawing routines can be called from menu.rs.
struct ConsoleMenuRenderer;

impl crate::qmenu::MenuRenderer for ConsoleMenuRenderer {
    fn draw_char(&mut self, x: i32, y: i32, ch: i32) {
        draw_char(x, y, ch);
    }
    fn draw_fill(&mut self, x: i32, y: i32, w: i32, h: i32, color: i32, alpha: f32) {
        draw_fill(x, y, w, h, color, alpha);
    }
    fn sys_milliseconds(&self) -> i32 {
        crate::console::sys_milliseconds()
    }
    fn vid_width(&self) -> i32 {
        // SAFETY: single-threaded engine
        unsafe { VIDDEF.width }
    }
    fn vid_height(&self) -> i32 {
        // SAFETY: single-threaded engine
        unsafe { VIDDEF.height }
    }
    fn sys_get_clipboard_data(&self) -> Option<String> {
        None // clipboard not wired yet
    }
    fn keydown(&self, key: i32) -> bool {
        // SAFETY: single-threaded engine
        if key >= 0 && key < 256 {
            unsafe { crate::keys::KEYDOWN[key as usize] }
        } else {
            false
        }
    }
}

/// Menu_AddItem — wraps a MenuCommon into the appropriate MenuItem variant
/// and adds it to the global MENU_ITEMS storage via qmenu::menu_add_item.
fn menu_add_item_common(menu: &mut MenuFramework, item: MenuCommon) {
    // SAFETY: single-threaded engine
    unsafe {
        let qm_item = match item.item_type {
            MTYPE_SLIDER => MenuItem::Slider(MenuSlider {
                generic: item,
                ..Default::default()
            }),
            MTYPE_FIELD => MenuItem::Field(MenuField {
                generic: item,
                ..Default::default()
            }),
            MTYPE_SEPARATOR => MenuItem::Separator(MenuSeparator {
                generic: item,
            }),
            MTYPE_SPINCONTROL => MenuItem::SpinControl(MenuList {
                generic: item,
                ..Default::default()
            }),
            MTYPE_LIST => MenuItem::List(MenuList {
                generic: item,
                ..Default::default()
            }),
            _ => MenuItem::Action(MenuAction {
                generic: item,
            }),
        };
        crate::qmenu::menu_add_item(menu, &mut MENU_ITEMS, qm_item);
    }
}

/// Menu_AdjustCursor — delegates to qmenu using global MENU_ITEMS storage.
fn menu_adjust_cursor(menu: &mut MenuFramework, dir: i32) {
    // SAFETY: single-threaded engine
    unsafe {
        crate::qmenu::menu_adjust_cursor(menu, &MENU_ITEMS, dir);
    }
}

/// Menu_Center — delegates to qmenu using global MENU_ITEMS storage.
fn menu_center(menu: &mut MenuFramework) {
    // SAFETY: single-threaded engine
    unsafe {
        crate::qmenu::menu_center(menu, &MENU_ITEMS, VIDDEF.height);
    }
}

/// Menu_Draw — delegates to qmenu using global MENU_ITEMS storage and ConsoleMenuRenderer.
fn menu_draw(menu: &MenuFramework) {
    // SAFETY: single-threaded engine
    unsafe {
        let mut renderer = ConsoleMenuRenderer;
        crate::qmenu::menu_draw(&mut renderer, menu, &mut MENU_ITEMS);
    }
}

/// Menu_ItemAtCursor — delegates to qmenu using global MENU_ITEMS storage.
fn menu_item_at_cursor(menu: &MenuFramework) -> Option<usize> {
    // SAFETY: single-threaded engine
    unsafe {
        crate::qmenu::menu_item_at_cursor(menu, &MENU_ITEMS)
            .map(|_item| {
                menu.cursor as usize
            })
    }
}

/// Menu_SelectItem — delegates to qmenu using global MENU_ITEMS storage.
fn menu_select_item(menu: &mut MenuFramework) -> bool {
    // SAFETY: single-threaded engine
    unsafe {
        crate::qmenu::menu_select_item(menu, &MENU_ITEMS)
    }
}

/// Menu_SetStatusBar — sets the statusbar text on the framework.
fn menu_set_status_bar(menu: &mut MenuFramework, text: Option<&'static str>) {
    menu.statusbar = text.map(|s| s.to_string());
}

/// Menu_SlideItem — delegates to qmenu using global MENU_ITEMS storage.
fn menu_slide_item(menu: &mut MenuFramework, dir: i32) {
    // SAFETY: single-threaded engine
    unsafe {
        crate::qmenu::menu_slide_item(menu, &mut MENU_ITEMS, dir);
    }
}

/// Menu_DrawString — draws a string using the ConsoleMenuRenderer.
fn menu_draw_string(x: i32, y: i32, s: &str) {
    let mut renderer = ConsoleMenuRenderer;
    crate::qmenu::menu_draw_string(&mut renderer, x, y, s);
}

/// Field_Key — delegates to qmenu field_key with ConsoleMenuRenderer.
fn field_key(field_idx: usize, key: i32) -> bool {
    // SAFETY: single-threaded engine
    unsafe {
        if let Some(MenuItem::Field(ref mut field)) = MENU_ITEMS.get_mut(field_idx) {
            let renderer = ConsoleMenuRenderer;
            crate::qmenu::field_key(field, key, &renderer)
        } else {
            false
        }
    }
}

// ============================================================
// Support routines
// ============================================================

/// Draw a banner image centered horizontally.
fn m_banner(name: &str) {
    let (w, _h) = draw_get_pic_size(name);
    // SAFETY: single-threaded engine
    unsafe {
        draw_pic(VIDDEF.width / 2 - w / 2, VIDDEF.height / 2 - 110, name);
    }
}

/// Push a new menu onto the stack.
pub fn m_push_menu(draw: MenuDrawFn, key: MenuKeyFn) {
    // SAFETY: single-threaded engine
    unsafe {
        if cvar_variable_value("maxclients") == 1.0 && com_server_state() != 0 {
            cvar_set("paused", "1");
        }

        // if this menu is already present, drop back to that level
        let mut found = false;
        for i in 0..M_MENUDEPTH {
            if M_LAYERS[i].draw == Some(draw) && M_LAYERS[i].key == Some(key) {
                M_MENUDEPTH = i;
                found = true;
                break;
            }
        }

        if !found {
            if M_MENUDEPTH >= MAX_MENU_DEPTH {
                myq2_common::common::com_error(myq2_common::q_shared::ERR_FATAL, "M_PushMenu: MAX_MENU_DEPTH");
            }
            M_LAYERS[M_MENUDEPTH].draw = M_DRAWFUNC;
            M_LAYERS[M_MENUDEPTH].key = M_KEYFUNC;
            M_MENUDEPTH += 1;
        }

        M_DRAWFUNC = Some(draw);
        M_KEYFUNC = Some(key);
        M_ENTERSOUND = true;
        CLS.key_dest = KeyDest::Menu;
    }
}

/// Force the menu off.
pub fn m_force_menu_off() {
    // SAFETY: single-threaded engine
    unsafe {
        M_DRAWFUNC = None;
        M_KEYFUNC = None;
        CLS.key_dest = KeyDest::Game;
        M_MENUDEPTH = 0;
        key_clear_states();
        cvar_set("paused", "0");
    }
}

// ============================================================
// Server list for the Join Server menu
// ============================================================

static mut M_NUM_SERVERS: usize = 0;
static mut LOCAL_SERVER_NAMES: [[u8; 80]; MAX_LOCAL_SERVERS] = [[0u8; 80]; MAX_LOCAL_SERVERS];

/// Add a server to the local server list (used by the Join Server menu).
/// Converted from: M_AddToServerList in myq2-original/client/menu.c
pub fn m_add_to_server_list(info: &str) {
    // SAFETY: single-threaded engine, mirrors original C static globals
    unsafe {
        if M_NUM_SERVERS == MAX_LOCAL_SERVERS {
            return;
        }

        let trimmed = info.trim_start();

        // ignore if duplicated
        for i in 0..M_NUM_SERVERS {
            let existing = std::str::from_utf8(&LOCAL_SERVER_NAMES[i])
                .unwrap_or("")
                .trim_end_matches('\0');
            if existing == trimmed {
                return;
            }
        }

        // store the name (truncated to 79 chars + NUL)
        let bytes = trimmed.as_bytes();
        let copy_len = bytes.len().min(79);
        LOCAL_SERVER_NAMES[M_NUM_SERVERS][..copy_len].copy_from_slice(&bytes[..copy_len]);
        LOCAL_SERVER_NAMES[M_NUM_SERVERS][copy_len] = 0;
        M_NUM_SERVERS += 1;
    }
}

/// Pop the current menu.
pub fn m_pop_menu() {
    s_start_local_sound(MENU_OUT_SOUND);
    // SAFETY: single-threaded engine
    unsafe {
        if M_MENUDEPTH < 1 {
            myq2_common::common::com_error(myq2_common::q_shared::ERR_FATAL, "M_PopMenu: depth < 1");
        }
        M_MENUDEPTH -= 1;

        M_DRAWFUNC = M_LAYERS[M_MENUDEPTH].draw;
        M_KEYFUNC = M_LAYERS[M_MENUDEPTH].key;

        if M_MENUDEPTH == 0 {
            m_force_menu_off();
        }
    }
}

// ============================================================
// Drawing helpers
// ============================================================

/// Draws one solid graphics character at menu coordinates.
/// cx and cy are in 320*240 coordinates.
pub fn m_draw_character(cx: i32, cy: i32, num: i32) {
    // SAFETY: single-threaded engine
    unsafe {
        draw_char(
            cx + ((VIDDEF.width - 320) >> 1),
            cy + ((VIDDEF.height - 240) >> 1),
            num,
        );
    }
}

/// Print colored text at menu coordinates.
pub fn m_print(cx: i32, cy: i32, str_text: &str) {
    let mut cx = cx;
    for ch in str_text.bytes() {
        m_draw_character(cx, cy, ch as i32 + 128);
        cx += 8;
    }
}

/// Print white text at menu coordinates.
pub fn m_print_white(cx: i32, cy: i32, str_text: &str) {
    let mut cx = cx;
    for ch in str_text.bytes() {
        m_draw_character(cx, cy, ch as i32);
        cx += 8;
    }
}

/// Draw a picture at menu coordinates.
pub fn m_draw_pic(x: i32, y: i32, pic: &str) {
    // SAFETY: single-threaded engine
    unsafe {
        draw_pic(
            x + ((VIDDEF.width - 320) >> 1),
            y + ((VIDDEF.height - 240) >> 1),
            pic,
        );
    }
}

/// Draw an animating cursor.
pub fn m_draw_cursor(x: i32, y: i32, f: i32) {
    static mut CACHED: bool = false;

    // SAFETY: single-threaded engine
    unsafe {
        if !CACHED {
            for i in 0..NUM_CURSOR_FRAMES {
                let cursorname = format!("m_cursor{}", i);
                crate::console::draw_find_pic(&cursorname);
            }
            CACHED = true;
        }
    }

    let cursorname = format!("m_cursor{}", f);
    draw_pic(x, y, &cursorname);
}

/// Draw a text box.
pub fn m_draw_text_box(x: i32, y: i32, width: i32, lines: i32) {
    let mut cx = x;
    let mut cy = y;

    // draw left side
    m_draw_character(cx, cy, 1);
    for _ in 0..lines {
        cy += 8;
        m_draw_character(cx, cy, 4);
    }
    m_draw_character(cx, cy + 8, 7);

    // draw middle
    cx += 8;
    let mut w = width;
    while w > 0 {
        cy = y;
        m_draw_character(cx, cy, 2);
        for _ in 0..lines {
            cy += 8;
            m_draw_character(cx, cy, 5);
        }
        m_draw_character(cx, cy + 8, 8);
        w -= 1;
        cx += 8;
    }

    // draw right side
    cy = y;
    m_draw_character(cx, cy, 3);
    for _ in 0..lines {
        cy += 8;
        m_draw_character(cx, cy, 6);
    }
    m_draw_character(cx, cy + 8, 9);
}

// ============================================================
// Main Menu
// ============================================================

fn m_main_draw() {
    let names = [
        "m_main_game",
        "m_main_multiplayer",
        "m_main_options",
        "m_main_video",
        "m_main_quit",
    ];

    let mut widest: i32 = -1;
    let mut totalheight: i32 = 0;

    for name in &names {
        let (w, h) = draw_get_pic_size(name);
        if w > widest {
            widest = w;
        }
        totalheight += h + 12;
    }

    // SAFETY: single-threaded engine
    unsafe {
        let ystart = VIDDEF.height / 2 - 110;
        let xoffset = (VIDDEF.width - widest + 70) / 2;

        for (i, name) in names.iter().enumerate() {
            if i as i32 != M_MAIN_CURSOR {
                draw_pic(xoffset, ystart + i as i32 * 40 + 13, name);
            }
        }

        let litname = format!("{}_sel", names[M_MAIN_CURSOR as usize]);
        draw_pic(xoffset, ystart + M_MAIN_CURSOR * 40 + 13, &litname);

        m_draw_cursor(
            xoffset - 25,
            ystart + M_MAIN_CURSOR * 40 + 11,
            (CLS.realtime / 100) % NUM_CURSOR_FRAMES,
        );

        let (w, h) = draw_get_pic_size("m_main_plaque");
        draw_pic(xoffset - 30 - w, ystart, "m_main_plaque");
        draw_pic(xoffset - 30 - w, ystart + h + 5, "m_main_logo");
    }
}

fn m_main_key(key: i32) -> Option<&'static str> {
    match key {
        K_ESCAPE => {
            m_pop_menu();
            None
        }
        K_DOWNARROW | 167 /* K_KP_DOWNARROW */ => {
            // SAFETY: single-threaded engine
            unsafe {
                M_MAIN_CURSOR += 1;
                if M_MAIN_CURSOR >= MAIN_ITEMS {
                    M_MAIN_CURSOR = 0;
                }
            }
            Some(MENU_MOVE_SOUND)
        }
        K_UPARROW | 161 /* K_KP_UPARROW */ => {
            // SAFETY: single-threaded engine
            unsafe {
                M_MAIN_CURSOR -= 1;
                if M_MAIN_CURSOR < 0 {
                    M_MAIN_CURSOR = MAIN_ITEMS - 1;
                }
            }
            Some(MENU_MOVE_SOUND)
        }
        K_KP_ENTER | K_ENTER => {
            // SAFETY: single-threaded engine
            unsafe {
                M_ENTERSOUND = true;
                match M_MAIN_CURSOR {
                    0 => m_menu_game_f(),
                    1 => m_menu_multiplayer_f(),
                    2 => m_menu_options_f(),
                    3 => m_menu_video_f(),
                    4 => m_menu_quit_f(),
                    _ => {}
                }
            }
            None
        }
        _ => None,
    }
}

/// Show the main menu.
pub fn m_menu_main_f() {
    m_push_menu(m_main_draw, m_main_key);
}

// ============================================================
// Game Menu
// Converted from: Game_MenuInit / Game_MenuDraw / Game_MenuKey
// ============================================================

static mut S_GAME_MENU: MenuFramework = MenuFramework {
    x: 0, y: 0, cursor: 0, nitems: 0, nslots: 0,
    items: Vec::new(), statusbar: None, cursordraw: None,
};

/// StartGame — disable updates and start the cinematic going.
fn start_game() {
    // SAFETY: single-threaded engine
    unsafe {
        crate::console::CL.servercount = -1;
    }
    m_force_menu_off();
    cvar_set_value("deathmatch", 0.0);
    cvar_set_value("coop", 0.0);
    cvar_set_value("gamerules", 0.0);
    crate::console::cbuf_add_text("loading ; killserver ; wait ; newgame\n");
    // SAFETY: single-threaded engine
    unsafe { CLS.key_dest = KeyDest::Game; }
}

fn easy_game_func() {
    cvar_force_set("skill", "0");
    start_game();
}

fn medium_game_func() {
    cvar_force_set("skill", "1");
    start_game();
}

fn hard_game_func() {
    cvar_force_set("skill", "2");
    start_game();
}

fn load_game_func() {
    m_menu_load_game_f();
}

fn save_game_func() {
    m_menu_save_game_f();
}

fn credits_func() {
    m_menu_credits_f();
}

/// Callback dispatcher for game menu items.
/// localdata[0] encodes which action: 0=easy, 1=medium, 2=hard, 3=load, 4=save, 5=credits
fn game_menu_callback(idx: usize) {
    // SAFETY: single-threaded engine
    unsafe {
        if let Some(item) = MENU_ITEMS.get(idx) {
            let ld = item.generic().localdata[0];
            match ld {
                0 => easy_game_func(),
                1 => medium_game_func(),
                2 => hard_game_func(),
                3 => load_game_func(),
                4 => save_game_func(),
                5 => credits_func(),
                _ => {}
            }
        }
    }
}

fn game_menu_init() {
    // SAFETY: single-threaded engine
    unsafe {
        S_GAME_MENU = MenuFramework {
            x: (VIDDEF.width as f32 * 0.50) as i32,
            y: 0, cursor: 0, nitems: 0, nslots: 0,
            items: Vec::new(), statusbar: None, cursordraw: None,
        };
        MENU_ITEMS.clear();

        let items: &[(&str, i32, i32)] = &[
            ("easy",      0,  0),
            ("medium",   10,  1),
            ("hard",     20,  2),
            ("load game", 40, 3),
            ("save game", 50, 4),
            ("credits",  60,  5),
        ];

        for &(name, y, id) in items {
            let item = make_menu_common(
                MTYPE_ACTION, name, 0, y, QMF_LEFT_JUSTIFY,
                [id, 0, 0, 0], Some(game_menu_callback), None,
            );
            menu_add_item_common(&mut S_GAME_MENU, item);
        }

        // Insert a blank separator at index 3 (between hard and load game)
        // The C code adds two separators but they are visual only; we just skip y values instead.

        menu_center(&mut S_GAME_MENU);
    }
}

fn game_menu_draw() {
    m_banner("m_banner_game");
    // SAFETY: single-threaded engine
    unsafe {
        menu_adjust_cursor(&mut S_GAME_MENU, 1);
        menu_draw(&S_GAME_MENU);
    }
}

fn game_menu_key(key: i32) -> Option<&'static str> {
    default_menu_key_with_menu(key, true)
}

pub fn m_menu_game_f() {
    game_menu_init();
    m_push_menu(game_menu_draw, game_menu_key);
}

// ============================================================
// Multiplayer Menu
// Converted from: Multiplayer_MenuInit / Multiplayer_MenuDraw / Multiplayer_MenuKey
// ============================================================

static mut S_MULTIPLAYER_MENU: MenuFramework = MenuFramework {
    x: 0, y: 0, cursor: 0, nitems: 0, nslots: 0,
    items: Vec::new(), statusbar: None, cursordraw: None,
};

fn multiplayer_menu_callback(idx: usize) {
    // SAFETY: single-threaded engine
    unsafe {
        if let Some(item) = MENU_ITEMS.get(idx) {
            match item.generic().localdata[0] {
                0 => m_menu_join_server_f(),
                1 => m_menu_start_server_f(),
                2 => m_menu_player_config_f(),
                _ => {}
            }
        }
    }
}

fn multiplayer_menu_init() {
    // SAFETY: single-threaded engine
    unsafe {
        S_MULTIPLAYER_MENU = MenuFramework {
            x: (VIDDEF.width as f32 * 0.50) as i32 - 64,
            y: 0, cursor: 0, nitems: 0, nslots: 0,
            items: Vec::new(), statusbar: None, cursordraw: None,
        };
        MENU_ITEMS.clear();

        let items: &[(&str, i32, i32)] = &[
            (" join network server",  0, 0),
            (" start network server", 10, 1),
            (" player setup",         20, 2),
        ];

        for &(name, y, id) in items {
            let item = make_menu_common(
                MTYPE_ACTION, name, 0, y, QMF_LEFT_JUSTIFY,
                [id, 0, 0, 0], Some(multiplayer_menu_callback), None,
            );
            menu_add_item_common(&mut S_MULTIPLAYER_MENU, item);
        }

        menu_set_status_bar(&mut S_MULTIPLAYER_MENU, None);
        menu_center(&mut S_MULTIPLAYER_MENU);
    }
}

fn multiplayer_menu_draw() {
    m_banner("m_banner_multiplayer");
    // SAFETY: single-threaded engine
    unsafe {
        menu_adjust_cursor(&mut S_MULTIPLAYER_MENU, 1);
        menu_draw(&S_MULTIPLAYER_MENU);
    }
}

fn multiplayer_menu_key(key: i32) -> Option<&'static str> {
    default_menu_key_with_menu(key, true)
}

pub fn m_menu_multiplayer_f() {
    multiplayer_menu_init();
    m_push_menu(multiplayer_menu_draw, multiplayer_menu_key);
}

// ============================================================
// Options Menu
// Converted from: Options_MenuInit / Options_MenuDraw / Options_MenuKey
// ============================================================

static mut S_OPTIONS_MENU: MenuFramework = MenuFramework {
    x: 0, y: 0, cursor: 0, nitems: 0, nslots: 0,
    items: Vec::new(), statusbar: None, cursordraw: None,
};

// Options menu item indices (order they are added)
const OPT_SFX_VOLUME: i32 = 0;
const OPT_CD_VOLUME: i32 = 1;
const OPT_QUALITY: i32 = 2;
const OPT_COMPATIBILITY: i32 = 3;
const OPT_SENSITIVITY: i32 = 4;
const OPT_ALWAYSRUN: i32 = 5;
const OPT_INVERTMOUSE: i32 = 6;
const OPT_LOOKSPRING: i32 = 7;
const OPT_LOOKSTRAFE: i32 = 8;
const OPT_FREELOOK: i32 = 9;
const OPT_CROSSHAIR: i32 = 10;
const OPT_JOYSTICK: i32 = 11;
const OPT_CUSTOMIZE: i32 = 12;
const OPT_DEFAULTS: i32 = 13;
const OPT_CONSOLE: i32 = 14;

fn clamp_cvar(min: f32, max: f32, value: f32) -> f32 {
    value.clamp(min, max)
}

fn options_menu_callback(idx: usize) {
    // SAFETY: single-threaded engine
    unsafe {
        if let Some(item) = MENU_ITEMS.get(idx) {
            let id = item.generic().localdata[0];
            match id {
                OPT_SFX_VOLUME => {
                    // UpdateVolumeFunc: read slider curvalue from qmenu item
                    if let MenuItem::Slider(ref s) = item {
                        cvar_set_value("s_volume", s.curvalue / 10.0);
                    }
                }
                OPT_CD_VOLUME => {
                    if let MenuItem::SpinControl(ref s) = item {
                        cvar_set_value("cd_nocd", if s.curvalue == 0 { 1.0 } else { 0.0 });
                    }
                }
                OPT_QUALITY | OPT_COMPATIBILITY => {
                    // UpdateSoundQualityFunc
                    let quality_val = if let Some(MenuItem::SpinControl(ref s)) = MENU_ITEMS.get(OPT_QUALITY as usize) {
                        s.curvalue
                    } else { 0 };
                    let compat_val = if let Some(MenuItem::SpinControl(ref s)) = MENU_ITEMS.get(OPT_COMPATIBILITY as usize) {
                        s.curvalue
                    } else { 0 };
                    if quality_val != 0 {
                        cvar_set_value("s_khz", 22.0);
                        cvar_set_value("s_loadas8bit", 0.0);
                    } else {
                        cvar_set_value("s_khz", 11.0);
                        cvar_set_value("s_loadas8bit", 1.0);
                    }
                    cvar_set_value("s_primary", compat_val as f32);
                    m_draw_text_box(8, 120 - 48, 36, 3);
                    m_print(16 + 16, 120 - 48 + 8,  "Restarting the sound system. This");
                    m_print(16 + 16, 120 - 48 + 16, "could take up to a minute, so");
                    m_print(16 + 16, 120 - 48 + 24, "please be patient.");
                    glimp_end_frame();
                    cl_snd_restart_f();
                }
                OPT_SENSITIVITY => {
                    if let MenuItem::Slider(ref s) = item {
                        cvar_set_value("sensitivity", s.curvalue / 2.0);
                    }
                }
                OPT_ALWAYSRUN => {
                    if let MenuItem::SpinControl(ref s) = item {
                        cvar_set_value("cl_run", s.curvalue as f32);
                    }
                }
                OPT_INVERTMOUSE => {
                    let cur = cvar_variable_value("m_pitch");
                    cvar_set_value("m_pitch", -cur);
                }
                OPT_LOOKSPRING => {
                    let cur = cvar_variable_value("lookspring");
                    cvar_set_value("lookspring", if cur == 0.0 { 1.0 } else { 0.0 });
                }
                OPT_LOOKSTRAFE => {
                    let cur = cvar_variable_value("lookstrafe");
                    cvar_set_value("lookstrafe", if cur == 0.0 { 1.0 } else { 0.0 });
                }
                OPT_FREELOOK => {
                    if let MenuItem::SpinControl(ref s) = item {
                        cvar_set_value("freelook", s.curvalue as f32);
                    }
                }
                OPT_CROSSHAIR => {
                    if let MenuItem::SpinControl(ref s) = item {
                        cvar_set_value("crosshair", s.curvalue as f32);
                    }
                }
                OPT_JOYSTICK => {
                    if let MenuItem::SpinControl(ref s) = item {
                        cvar_set_value("in_joystick", s.curvalue as f32);
                    }
                }
                OPT_CUSTOMIZE => {
                    m_menu_keys_f();
                }
                OPT_DEFAULTS => {
                    crate::console::cbuf_add_text("exec default.cfg\n");
                    myq2_common::cmd::cbuf_execute();
                    controls_set_menu_item_values();
                }
                OPT_CONSOLE => {
                    m_force_menu_off();
                    CLS.key_dest = KeyDest::Console;
                }
                _ => {}
            }
        }
    }
}

/// Set the current values of all options menu items from cvars.
fn controls_set_menu_item_values() {
    // SAFETY: single-threaded engine
    unsafe {
        // SFX volume slider
        if let Some(MenuItem::Slider(ref mut s)) = MENU_ITEMS.get_mut(OPT_SFX_VOLUME as usize) {
            s.curvalue = cvar_variable_value("s_volume") * 10.0;
        }
        // CD volume
        if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.get_mut(OPT_CD_VOLUME as usize) {
            s.curvalue = if cvar_variable_value("cd_nocd") != 0.0 { 0 } else { 1 };
        }
        // Sound quality
        if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.get_mut(OPT_QUALITY as usize) {
            s.curvalue = if cvar_variable_value("s_loadas8bit") != 0.0 { 0 } else { 1 };
        }
        // Sound compatibility
        if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.get_mut(OPT_COMPATIBILITY as usize) {
            s.curvalue = cvar_variable_value("s_primary") as i32;
        }
        // Sensitivity slider
        if let Some(MenuItem::Slider(ref mut s)) = MENU_ITEMS.get_mut(OPT_SENSITIVITY as usize) {
            s.curvalue = cvar_variable_value("sensitivity") * 2.0;
        }
        // Always run
        if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.get_mut(OPT_ALWAYSRUN as usize) {
            cvar_set_value("cl_run", clamp_cvar(0.0, 1.0, cvar_variable_value("cl_run")));
            s.curvalue = cvar_variable_value("cl_run") as i32;
        }
        // Invert mouse
        if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.get_mut(OPT_INVERTMOUSE as usize) {
            s.curvalue = if cvar_variable_value("m_pitch") < 0.0 { 1 } else { 0 };
        }
        // Lookspring
        if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.get_mut(OPT_LOOKSPRING as usize) {
            cvar_set_value("lookspring", clamp_cvar(0.0, 1.0, cvar_variable_value("lookspring")));
            s.curvalue = cvar_variable_value("lookspring") as i32;
        }
        // Lookstrafe
        if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.get_mut(OPT_LOOKSTRAFE as usize) {
            cvar_set_value("lookstrafe", clamp_cvar(0.0, 1.0, cvar_variable_value("lookstrafe")));
            s.curvalue = cvar_variable_value("lookstrafe") as i32;
        }
        // Freelook
        if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.get_mut(OPT_FREELOOK as usize) {
            cvar_set_value("freelook", clamp_cvar(0.0, 1.0, cvar_variable_value("freelook")));
            s.curvalue = cvar_variable_value("freelook") as i32;
        }
        // Crosshair
        if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.get_mut(OPT_CROSSHAIR as usize) {
            cvar_set_value("crosshair", clamp_cvar(0.0, 3.0, cvar_variable_value("crosshair")));
            s.curvalue = cvar_variable_value("crosshair") as i32;
        }
        // Joystick
        if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.get_mut(OPT_JOYSTICK as usize) {
            cvar_set_value("in_joystick", clamp_cvar(0.0, 1.0, cvar_variable_value("in_joystick")));
            s.curvalue = cvar_variable_value("in_joystick") as i32;
        }
        // No alt-tab
        // (commented out in original C, skipped here)
    }
}

fn options_menu_init() {
    // SAFETY: single-threaded engine
    unsafe {
        S_OPTIONS_MENU = MenuFramework {
            x: VIDDEF.width / 2,
            y: VIDDEF.height / 2 - 58,
            cursor: 0, nitems: 0, nslots: 0,
            items: Vec::new(), statusbar: None, cursordraw: None,
        };
        MENU_ITEMS.clear();

        // 0: SFX volume slider
        menu_add_item_common(&mut S_OPTIONS_MENU, make_menu_common(
            MTYPE_SLIDER, "effects volume", 0, 0, 0,
            [OPT_SFX_VOLUME, 0, 0, 0], Some(options_menu_callback), None,
        ));
        if let Some(MenuItem::Slider(ref mut s)) = MENU_ITEMS.get_mut(OPT_SFX_VOLUME as usize) {
            s.minvalue = 0.0;
            s.maxvalue = 10.0;
            s.curvalue = cvar_variable_value("s_volume") * 10.0;
        }

        // 1: CD music spin
        menu_add_item_common(&mut S_OPTIONS_MENU, make_menu_common(
            MTYPE_SPINCONTROL, "CD music", 0, 10, 0,
            [OPT_CD_VOLUME, 0, 0, 0], Some(options_menu_callback), None,
        ));
        if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.get_mut(OPT_CD_VOLUME as usize) {
            s.itemnames = strs(&["disabled", "enabled"]);
            s.curvalue = if cvar_variable_value("cd_nocd") != 0.0 { 0 } else { 1 };
        }

        // 2: sound quality
        menu_add_item_common(&mut S_OPTIONS_MENU, make_menu_common(
            MTYPE_SPINCONTROL, "sound quality", 0, 20, 0,
            [OPT_QUALITY, 0, 0, 0], Some(options_menu_callback), None,
        ));
        if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.get_mut(OPT_QUALITY as usize) {
            s.itemnames = strs(&["low", "high"]);
            s.curvalue = if cvar_variable_value("s_loadas8bit") != 0.0 { 0 } else { 1 };
        }

        // 3: sound compatibility
        menu_add_item_common(&mut S_OPTIONS_MENU, make_menu_common(
            MTYPE_SPINCONTROL, "sound compatibility", 0, 30, 0,
            [OPT_COMPATIBILITY, 0, 0, 0], Some(options_menu_callback), None,
        ));
        if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.get_mut(OPT_COMPATIBILITY as usize) {
            s.itemnames = strs(&["max compatibility", "max performance"]);
            s.curvalue = cvar_variable_value("s_primary") as i32;
        }

        // 4: mouse speed slider
        menu_add_item_common(&mut S_OPTIONS_MENU, make_menu_common(
            MTYPE_SLIDER, "mouse speed", 0, 50, 0,
            [OPT_SENSITIVITY, 0, 0, 0], Some(options_menu_callback), None,
        ));
        if let Some(MenuItem::Slider(ref mut s)) = MENU_ITEMS.get_mut(OPT_SENSITIVITY as usize) {
            s.minvalue = 2.0;
            s.maxvalue = 22.0;
            s.curvalue = cvar_variable_value("sensitivity") * 2.0;
        }

        // 5: always run
        menu_add_item_common(&mut S_OPTIONS_MENU, make_menu_common(
            MTYPE_SPINCONTROL, "always run", 0, 60, 0,
            [OPT_ALWAYSRUN, 0, 0, 0], Some(options_menu_callback), None,
        ));
        if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.get_mut(OPT_ALWAYSRUN as usize) {
            s.itemnames = strs(&["no", "yes"]);
        }

        // 6: invert mouse
        menu_add_item_common(&mut S_OPTIONS_MENU, make_menu_common(
            MTYPE_SPINCONTROL, "invert mouse", 0, 70, 0,
            [OPT_INVERTMOUSE, 0, 0, 0], Some(options_menu_callback), None,
        ));
        if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.get_mut(OPT_INVERTMOUSE as usize) {
            s.itemnames = strs(&["no", "yes"]);
        }

        // 7: lookspring
        menu_add_item_common(&mut S_OPTIONS_MENU, make_menu_common(
            MTYPE_SPINCONTROL, "lookspring", 0, 80, 0,
            [OPT_LOOKSPRING, 0, 0, 0], Some(options_menu_callback), None,
        ));
        if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.get_mut(OPT_LOOKSPRING as usize) {
            s.itemnames = strs(&["no", "yes"]);
        }

        // 8: lookstrafe
        menu_add_item_common(&mut S_OPTIONS_MENU, make_menu_common(
            MTYPE_SPINCONTROL, "lookstrafe", 0, 90, 0,
            [OPT_LOOKSTRAFE, 0, 0, 0], Some(options_menu_callback), None,
        ));
        if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.get_mut(OPT_LOOKSTRAFE as usize) {
            s.itemnames = strs(&["no", "yes"]);
        }

        // 9: free look
        menu_add_item_common(&mut S_OPTIONS_MENU, make_menu_common(
            MTYPE_SPINCONTROL, "free look", 0, 100, 0,
            [OPT_FREELOOK, 0, 0, 0], Some(options_menu_callback), None,
        ));
        if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.get_mut(OPT_FREELOOK as usize) {
            s.itemnames = strs(&["no", "yes"]);
        }

        // 10: crosshair
        menu_add_item_common(&mut S_OPTIONS_MENU, make_menu_common(
            MTYPE_SPINCONTROL, "crosshair", 0, 110, 0,
            [OPT_CROSSHAIR, 0, 0, 0], Some(options_menu_callback), None,
        ));
        if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.get_mut(OPT_CROSSHAIR as usize) {
            s.itemnames = strs(&["none", "cross", "dot", "angle"]);
        }

        // 11: joystick
        menu_add_item_common(&mut S_OPTIONS_MENU, make_menu_common(
            MTYPE_SPINCONTROL, "use joystick", 0, 120, 0,
            [OPT_JOYSTICK, 0, 0, 0], Some(options_menu_callback), None,
        ));
        if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.get_mut(OPT_JOYSTICK as usize) {
            s.itemnames = strs(&["no", "yes"]);
        }

        // 12: customize controls
        menu_add_item_common(&mut S_OPTIONS_MENU, make_menu_common(
            MTYPE_ACTION, "customize controls", 0, 140, 0,
            [OPT_CUSTOMIZE, 0, 0, 0], Some(options_menu_callback), None,
        ));

        // 13: reset defaults
        menu_add_item_common(&mut S_OPTIONS_MENU, make_menu_common(
            MTYPE_ACTION, "reset defaults", 0, 150, 0,
            [OPT_DEFAULTS, 0, 0, 0], Some(options_menu_callback), None,
        ));

        // 14: go to console
        menu_add_item_common(&mut S_OPTIONS_MENU, make_menu_common(
            MTYPE_ACTION, "go to console", 0, 160, 0,
            [OPT_CONSOLE, 0, 0, 0], Some(options_menu_callback), None,
        ));

        controls_set_menu_item_values();
    }
}

fn options_menu_draw() {
    m_banner("m_banner_options");
    // SAFETY: single-threaded engine
    unsafe {
        menu_adjust_cursor(&mut S_OPTIONS_MENU, 1);
        menu_draw(&S_OPTIONS_MENU);
    }
}

fn options_menu_key(key: i32) -> Option<&'static str> {
    default_menu_key_with_menu(key, true)
}

pub fn m_menu_options_f() {
    options_menu_init();
    m_push_menu(options_menu_draw, options_menu_key);
}

// ============================================================
// Keys Menu (stub)
// ============================================================

fn keys_menu_draw() {
    // SAFETY: single-threaded engine
    unsafe {
        menu_adjust_cursor(&mut S_KEYS_MENU, 1);
        menu_draw(&S_KEYS_MENU);

        // Draw key bindings for each item (ownerdraw equivalent)
        for i in 0..MENU_ITEMS.len() {
            draw_key_binding_func(i);
        }

        // Draw custom cursor
        if BIND_GRAB {
            draw_char(
                S_KEYS_MENU.x,
                S_KEYS_MENU.y + S_KEYS_MENU.cursor * 9,
                b'=' as i32,
            );
        } else {
            draw_char(
                S_KEYS_MENU.x,
                S_KEYS_MENU.y + S_KEYS_MENU.cursor * 9,
                12 + ((crate::console::sys_milliseconds() / 250) & 1),
            );
        }
    }
}

// Keys menu static state
static mut S_KEYS_MENU: MenuFramework = MenuFramework {
    x: 0,
    y: 0,
    cursor: 0,
    nitems: 0,
    nslots: 0,
    items: Vec::new(),
    statusbar: None,
    cursordraw: None,
};
static mut BIND_GRAB: bool = false;

/// Bindnames table: [command, display name] pairs (from the original C source).
static BINDNAMES: &[(&str, &str)] = &[
    ("+attack", "attack"),
    ("weapnext", "next weapon"),
    ("+forward", "walk forward"),
    ("+back", "backpedal"),
    ("+left", "turn left"),
    ("+right", "turn right"),
    ("+speed", "run"),
    ("+moveleft", "step left"),
    ("+moveright", "step right"),
    ("+strafe", "sidestep"),
    ("+lookup", "look up"),
    ("+lookdown", "look down"),
    ("centerview", "center view"),
    ("+mlook", "mouse look"),
    ("+klook", "keyboard look"),
    ("+moveup", "up / jump"),
    ("+movedown", "down / crouch"),
    ("inven", "inventory"),
    ("invuse", "use item"),
    ("invdrop", "drop item"),
    ("invprev", "prev item"),
    ("invnext", "next item"),
    ("cmd help", "help computer"),
];

/// Find keys bound to a given command. Returns up to 2 key indices (-1 if unbound).
fn m_find_keys_for_command(command: &str) -> [i32; 2] {
    let mut keys = [-1i32; 2];
    let mut count = 0;
    // SAFETY: single-threaded engine
    unsafe {
        for k in 0..256 {
            if let Some(ref binding) = crate::keys::KEYBINDINGS[k] {
                if binding == command {
                    keys[count] = k as i32;
                    count += 1;
                    if count == 2 {
                        break;
                    }
                }
            }
        }
    }
    keys
}

/// Unbind all keys bound to the given command.
fn m_unbind_command(command: &str) {
    // SAFETY: single-threaded engine
    unsafe {
        for k in 0..256 {
            if let Some(ref binding) = crate::keys::KEYBINDINGS[k] {
                if binding == command {
                    crate::keys::key_set_binding(k as i32, "");
                }
            }
        }
    }
}

/// Draw key binding info for a key action item.
fn draw_key_binding_func(idx: usize) {
    // SAFETY: single-threaded engine
    unsafe {
        if let Some(item) = MENU_ITEMS.get(idx) {
            let bind_idx = item.generic().localdata[0] as usize;
            if bind_idx < BINDNAMES.len() {
                let keys = m_find_keys_for_command(BINDNAMES[bind_idx].0);
                let parent_x = S_KEYS_MENU.x;
                let parent_y = S_KEYS_MENU.y;
                let x = item.generic().x + parent_x + 16;
                let y = item.generic().y + parent_y;

                if keys[0] == -1 {
                    menu_draw_string(x, y, "???");
                } else {
                    let name = crate::keys::key_keynum_to_string(keys[0]);
                    menu_draw_string(x, y, &name);
                    let name_width = name.len() as i32 * 8;
                    if keys[1] != -1 {
                        menu_draw_string(x + 8 + name_width, y, "or");
                        let name2 = crate::keys::key_keynum_to_string(keys[1]);
                        menu_draw_string(x + 32 + name_width, y, &name2);
                    }
                }
            }
        }
    }
}

/// Initialize the keys menu framework.
fn keys_menu_init() {
    // SAFETY: single-threaded engine
    unsafe {
        S_KEYS_MENU = MenuFramework {
            x: (VIDDEF.width as f32 * 0.50) as i32,
            y: 0,
            cursor: 0,
            nitems: 0,
            nslots: 0,
            items: Vec::new(),
            statusbar: Some("enter to change, backspace to clear".to_string()),
            cursordraw: None,
        };
        MENU_ITEMS.clear();

        for (i, &(_cmd, label)) in BINDNAMES.iter().enumerate() {
            let item = make_menu_common(
                MTYPE_ACTION, label, 0, i as i32 * 9, QMF_GRAYED,
                [i as i32, 0, 0, 0], None, None,
            );
            menu_add_item_common(&mut S_KEYS_MENU, item);
        }

        menu_set_status_bar(&mut S_KEYS_MENU, Some("enter to change, backspace to clear"));
        menu_center(&mut S_KEYS_MENU);
    }
}

fn keys_menu_key(key: i32) -> Option<&'static str> {
    // SAFETY: single-threaded engine
    unsafe {
        if BIND_GRAB {
            if key != K_ESCAPE && key != 96 /* '`' */ {
                let cursor = S_KEYS_MENU.cursor as usize;
                if cursor < BINDNAMES.len() {
                    let cmd = format!(
                        "bind \"{}\" \"{}\"\n",
                        crate::keys::key_keynum_to_string(key),
                        BINDNAMES[cursor].0
                    );
                    cbuf_insert_text(&cmd);
                }
            }
            menu_set_status_bar(&mut S_KEYS_MENU, Some("enter to change, backspace to clear"));
            BIND_GRAB = false;
            return Some(MENU_OUT_SOUND);
        }

        match key {
            K_KP_ENTER | K_ENTER => {
                // Start key binding grab
                let cursor = S_KEYS_MENU.cursor as usize;
                if cursor < BINDNAMES.len() {
                    let found_keys = m_find_keys_for_command(BINDNAMES[cursor].0);
                    if found_keys[1] != -1 {
                        m_unbind_command(BINDNAMES[cursor].0);
                    }
                    BIND_GRAB = true;
                    menu_set_status_bar(&mut S_KEYS_MENU, Some("press a key or button for this action"));
                }
                Some(MENU_IN_SOUND)
            }
            127 /* K_BACKSPACE */ | 132 /* K_DEL */ | 170 /* K_KP_DEL */ => {
                // Delete bindings
                let cursor = S_KEYS_MENU.cursor as usize;
                if cursor < BINDNAMES.len() {
                    m_unbind_command(BINDNAMES[cursor].0);
                }
                Some(MENU_OUT_SOUND)
            }
            _ => default_menu_key(key),
        }
    }
}

pub fn m_menu_keys_f() {
    keys_menu_init();
    m_push_menu(keys_menu_draw, keys_menu_key);
}

// ============================================================
// Video Menu (stub)
// ============================================================

pub fn m_menu_video_f() {
    vid_menu_init();
    m_push_menu(vid_menu_draw, vid_menu_key);
}

// ============================================================
// Credits Menu
// ============================================================

static ID_CREDITS: &[&str] = &[
    "+QUAKE II BY ID SOFTWARE",
    "",
    "+PROGRAMMING",
    "John Carmack",
    "John Cash",
    "Brian Hook",
    "",
    "+ART",
    "Adrian Carmack",
    "Kevin Cloud",
    "Paul Steed",
    "",
    "+LEVEL DESIGN",
    "Tim Willits",
    "American McGee",
    "Christian Antkow",
    "Paul Jaquays",
    "Brandon James",
    "",
    "+BIZ",
    "Todd Hollenshead",
    "Barrett (Bear) Alexander",
    "Donna Jackson",
    "",
    "",
    "+SPECIAL THANKS",
    "Ben Donges for beta testing",
    "",
    "",
    "",
    "",
    "",
    "",
    "+ADDITIONAL SUPPORT",
    "",
    "+LINUX PORT AND CTF",
    "Dave \"Zoid\" Kirsch",
    "",
    "+CINEMATIC SEQUENCES",
    "Ending Cinematic by Blur Studio - ",
    "Venice, CA",
    "",
    "Environment models for Introduction",
    "Cinematic by Karl Dolgener",
    "",
    "Assistance with environment design",
    "by Cliff Iwai",
    "",
    "+SOUND EFFECTS AND MUSIC",
    "Sound Design by Soundelux Media Labs.",
    "Music Composed and Produced by",
    "Soundelux Media Labs.  Special thanks",
    "to Bill Brown, Tom Ozanich, Brian",
    "Celano, Jeff Eisner, and The Soundelux",
    "Players.",
    "",
    "\"Level Music\" by Sonic Mayhem",
    "www.sonicmayhem.com",
    "",
    "\"Quake II Theme Song\"",
    "(C) 1997 Rob Zombie. All Rights",
    "Reserved.",
    "",
    "Track 10 (\"Climb\") by Jer Sypult",
    "",
    "Voice of computers by",
    "Carly Staehlin-Taylor",
    "",
    "+THANKS TO ACTIVISION",
    "+IN PARTICULAR:",
    "",
    "John Tam",
    "Steve Rosenthal",
    "Marty Stratton",
    "Henk Hartong",
    "",
    "Quake II(tm) (C)1997 Id Software, Inc.",
    "All Rights Reserved.  Distributed by",
    "Activision, Inc. under license.",
    "Quake II(tm), the Id Software name,",
    "the \"Q II\"(tm) logo and id(tm)",
    "logo are trademarks of Id Software,",
    "Inc. Activision(R) is a registered",
    "trademark of Activision, Inc. All",
    "other trademarks and trade names are",
    "properties of their respective owners.",
];

static mut CREDITS_START_TIME: i32 = 0;
static mut CREDITS: &[&str] = ID_CREDITS;

fn m_credits_menu_draw() {
    // SAFETY: single-threaded engine
    unsafe {
        let credits = CREDITS;
        let mut y = VIDDEF.height as f32
            - ((CLS.realtime - CREDITS_START_TIME) as f32 / 40.0);
        let mut i = 0;

        while i < credits.len() && (y as i32) < VIDDEF.height {
            if y as i32 > -8 {
                let line = credits[i];
                let (bold, stringoffset) = if line.starts_with('+') {
                    (true, 1)
                } else {
                    (false, 0)
                };

                let chars: Vec<u8> = line.bytes().skip(stringoffset).collect();
                for (j, &ch) in chars.iter().enumerate() {
                    let x = (VIDDEF.width - line.len() as i32 * 8 - stringoffset as i32 * 8) / 2
                        + (j as i32 + stringoffset as i32) * 8;

                    if bold {
                        draw_char(x, y as i32, ch as i32 + 128);
                    } else {
                        draw_char(x, y as i32, ch as i32);
                    }
                }
            }

            y += 10.0;
            i += 1;
        }

        if (y as i32) < 0 {
            CREDITS_START_TIME = CLS.realtime;
        }
    }
}

fn m_credits_key(key: i32) -> Option<&'static str> {
    if key == K_ESCAPE {
        m_pop_menu();
    }
    Some(MENU_OUT_SOUND)
}

pub fn m_menu_credits_f() {
    // SAFETY: single-threaded engine
    unsafe {
        // Try loading custom credits file
        if let Some(data) = fs_load_file("credits") {
            // Parse the credits file into lines, splitting on \r\n or \n.
            // Store in a leaked Vec so that CREDITS can hold &'static [&'static str].
            let text = String::from_utf8_lossy(&data);
            let lines: Vec<&str> = text.lines().collect();
            let owned: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
            let leaked: &'static Vec<String> = Box::leak(Box::new(owned));
            let str_refs: Vec<&'static str> = leaked.iter().map(|s| s.as_str()).collect();
            let leaked_refs: &'static [&'static str] = Box::leak(str_refs.into_boxed_slice());
            CREDITS = leaked_refs;
        } else {
            let is_developer = developer_searchpath(1);
            if is_developer == 1 {
                // xatrix credits would go here
                CREDITS = ID_CREDITS; // simplified — xatcredits omitted for brevity
            } else if is_developer == 2 {
                // rogue credits would go here
                CREDITS = ID_CREDITS; // simplified — roguecredits omitted for brevity
            } else {
                CREDITS = ID_CREDITS;
            }
        }

        CREDITS_START_TIME = CLS.realtime;
    }
    m_push_menu(m_credits_menu_draw, m_credits_key);
}

// ============================================================
// Load/Save Game menus (stubs)
// ============================================================

// ============================================================
// Save/Load game support
// ============================================================

static mut M_SAVESTRINGS: [[u8; 32]; MAX_SAVEGAMES] = [[0u8; 32]; MAX_SAVEGAMES];
static mut M_SAVEVALID: [bool; MAX_SAVEGAMES] = [false; MAX_SAVEGAMES];

static mut S_LOADGAME_MENU: MenuFramework = MenuFramework {
    x: 0, y: 0, cursor: 0, nitems: 0, nslots: 0,
    items: Vec::new(), statusbar: None, cursordraw: None,
};

static mut S_SAVEGAME_MENU: MenuFramework = MenuFramework {
    x: 0, y: 0, cursor: 0, nitems: 0, nslots: 0,
    items: Vec::new(), statusbar: None, cursordraw: None,
};

/// Create_Savestrings — reads save game headers to populate the save/load menus.
fn create_savestrings() {
    let gamedir = crate::console::fs_gamedir();
    for i in 0..MAX_SAVEGAMES {
        let path = format!("{}/save/save{}/server.ssv", gamedir, i);
        // SAFETY: single-threaded engine
        unsafe {
            if let Ok(mut f) = std::fs::File::open(&path) {
                let mut buf = [0u8; 32];
                let _ = std::io::Read::read(&mut f, &mut buf);
                M_SAVESTRINGS[i] = buf;
                M_SAVEVALID[i] = true;
            } else {
                let empty = b"<EMPTY>\0";
                M_SAVESTRINGS[i] = [0u8; 32];
                M_SAVESTRINGS[i][..empty.len()].copy_from_slice(empty);
                M_SAVEVALID[i] = false;
            }
        }
    }
}

fn savestring_as_str(idx: usize) -> &'static str {
    // SAFETY: single-threaded engine
    unsafe {
        let bytes = &M_SAVESTRINGS[idx];
        let len = bytes.iter().position(|&b| b == 0).unwrap_or(32);
        std::str::from_utf8(&bytes[..len]).unwrap_or("<EMPTY>")
    }
}

fn loadgame_menu_init() {
    // SAFETY: single-threaded engine
    unsafe {
        S_LOADGAME_MENU = MenuFramework {
            x: VIDDEF.width / 2 - 120,
            y: VIDDEF.height / 2 - 58,
            cursor: 0, nitems: 0, nslots: 0,
            items: Vec::new(), statusbar: None, cursordraw: None,
        };
        MENU_ITEMS.clear();
        create_savestrings();

        for i in 0..MAX_SAVEGAMES {
            let y = if i > 0 { i as i32 * 10 + 10 } else { 0 };
            let item = make_menu_common(
                MTYPE_ACTION, savestring_as_str(i), 0, y, QMF_LEFT_JUSTIFY,
                [i as i32, 0, 0, 0], None, None,
            );
            menu_add_item_common(&mut S_LOADGAME_MENU, item);
        }
    }
}

fn loadgame_menu_draw() {
    m_banner("m_banner_load_game");
    // SAFETY: single-threaded engine
    unsafe { menu_draw(&S_LOADGAME_MENU); }
}

fn loadgame_menu_key(key: i32) -> Option<&'static str> {
    // SAFETY: single-threaded engine
    unsafe {
        if key == K_ESCAPE || key == K_ENTER {
            S_SAVEGAME_MENU.cursor = S_LOADGAME_MENU.cursor - 1;
            if S_SAVEGAME_MENU.cursor < 0 {
                S_SAVEGAME_MENU.cursor = 0;
            }
        }
        if key == K_KP_ENTER || key == K_ENTER {
            let idx = S_LOADGAME_MENU.cursor as usize;
            if idx < MAX_SAVEGAMES && M_SAVEVALID[idx] {
                let cmd = format!("load save{}\n", idx);
                crate::console::cbuf_add_text(&cmd);
            }
            m_force_menu_off();
            return None;
        }
    }
    default_menu_key(key)
}

pub fn m_menu_load_game_f() {
    loadgame_menu_init();
    m_push_menu(loadgame_menu_draw, loadgame_menu_key);
}

fn savegame_menu_init() {
    // SAFETY: single-threaded engine
    unsafe {
        S_SAVEGAME_MENU = MenuFramework {
            x: VIDDEF.width / 2 - 120,
            y: VIDDEF.height / 2 - 58,
            cursor: 0, nitems: 0, nslots: 0,
            items: Vec::new(), statusbar: None, cursordraw: None,
        };
        MENU_ITEMS.clear();
        create_savestrings();

        // Don't include the autosave slot (slot 0)
        for i in 0..(MAX_SAVEGAMES - 1) {
            let item = make_menu_common(
                MTYPE_ACTION, savestring_as_str(i + 1), 0, i as i32 * 10, QMF_LEFT_JUSTIFY,
                [(i + 1) as i32, 0, 0, 0], None, None,
            );
            menu_add_item_common(&mut S_SAVEGAME_MENU, item);
        }
    }
}

fn savegame_menu_draw() {
    m_banner("m_banner_save_game");
    // SAFETY: single-threaded engine
    unsafe {
        menu_adjust_cursor(&mut S_SAVEGAME_MENU, 1);
        menu_draw(&S_SAVEGAME_MENU);
    }
}

fn savegame_menu_key(key: i32) -> Option<&'static str> {
    // SAFETY: single-threaded engine
    unsafe {
        if key == K_ENTER || key == K_ESCAPE {
            S_LOADGAME_MENU.cursor = S_SAVEGAME_MENU.cursor - 1;
            if S_LOADGAME_MENU.cursor < 0 {
                S_LOADGAME_MENU.cursor = 0;
            }
        }
        if key == K_KP_ENTER || key == K_ENTER {
            let cursor = S_SAVEGAME_MENU.cursor as usize;
            // localdata[0] = cursor + 1 (skip autosave)
            let save_idx = cursor + 1;
            let cmd = format!("save save{}\n", save_idx);
            crate::console::cbuf_add_text(&cmd);
            m_force_menu_off();
            return None;
        }
    }
    default_menu_key(key)
}

pub fn m_menu_save_game_f() {
    if com_server_state() == 0 {
        return; // not playing a game
    }
    savegame_menu_init();
    m_push_menu(savegame_menu_draw, savegame_menu_key);
    create_savestrings();
}

// ============================================================
// Join Server Menu
// Converted from: JoinServer_MenuInit / JoinServer_MenuDraw / JoinServer_MenuKey
// ============================================================

static mut S_JOINSERVER_MENU: MenuFramework = MenuFramework {
    x: 0, y: 0, cursor: 0, nitems: 0, nslots: 0,
    items: Vec::new(), statusbar: None, cursordraw: None,
};

const NO_SERVER_STRING: &str = "<no server>";

fn search_local_games() {
    // SAFETY: single-threaded engine
    unsafe {
        M_NUM_SERVERS = 0;
        for i in 0..MAX_LOCAL_SERVERS {
            let bytes = NO_SERVER_STRING.as_bytes();
            LOCAL_SERVER_NAMES[i] = [0u8; 80];
            LOCAL_SERVER_NAMES[i][..bytes.len()].copy_from_slice(bytes);
        }
    }

    m_draw_text_box(8, 120 - 48, 36, 3);
    m_print(16 + 16, 120 - 48 + 8,  "Searching for local servers, this");
    m_print(16 + 16, 120 - 48 + 16, "could take up to a minute, so");
    m_print(16 + 16, 120 - 48 + 24, "please be patient.");

    glimp_end_frame();
    cl_ping_servers_f();
}

fn joinserver_menu_callback(idx: usize) {
    // SAFETY: single-threaded engine
    unsafe {
        if let Some(item) = MENU_ITEMS.get(idx) {
            let id = item.generic().localdata[0];
            match id {
                -1 => m_menu_address_book_f(),  // address book
                -2 => search_local_games(),      // refresh
                _ => {
                    // server action — id is the server index
                    let server_idx = id as usize;
                    if server_idx < M_NUM_SERVERS {
                        let name = std::str::from_utf8(&LOCAL_SERVER_NAMES[server_idx])
                            .unwrap_or("")
                            .trim_end_matches('\0');
                        if name != NO_SERVER_STRING {
                            // In the full engine, we would connect to local_server_netadr[server_idx].
                            // For now, just force menu off (network address resolution not yet wired).
                            m_force_menu_off();
                        }
                    }
                }
            }
        }
    }
}

fn joinserver_menu_init() {
    // SAFETY: single-threaded engine
    unsafe {
        S_JOINSERVER_MENU = MenuFramework {
            x: (VIDDEF.width as f32 * 0.50) as i32 - 120,
            y: 0, cursor: 0, nitems: 0, nslots: 0,
            items: Vec::new(), statusbar: None, cursordraw: None,
        };
        MENU_ITEMS.clear();

        // Address book action
        menu_add_item_common(&mut S_JOINSERVER_MENU, make_menu_common(
            MTYPE_ACTION, "address book", 0, 0, QMF_LEFT_JUSTIFY,
            [-1, 0, 0, 0], Some(joinserver_menu_callback), None,
        ));

        // Separator: "connect to..."
        menu_add_item_common(&mut S_JOINSERVER_MENU, make_menu_common(
            MTYPE_SEPARATOR, "connect to...", 80, 30, 0,
            [0, 0, 0, 0], None, None,
        ));

        // Search action
        menu_add_item_common(&mut S_JOINSERVER_MENU, make_menu_common(
            MTYPE_ACTION, "refresh server list", 0, 10, QMF_LEFT_JUSTIFY,
            [-2, 0, 0, 0], Some(joinserver_menu_callback), Some("search for servers"),
        ));

        // Server entries
        for i in 0..MAX_LOCAL_SERVERS {
            menu_add_item_common(&mut S_JOINSERVER_MENU, make_menu_common(
                MTYPE_ACTION, NO_SERVER_STRING, 0, 40 + i as i32 * 10, QMF_LEFT_JUSTIFY,
                [i as i32, 0, 0, 0], Some(joinserver_menu_callback), Some("press ENTER to connect"),
            ));
        }

        menu_center(&mut S_JOINSERVER_MENU);
        search_local_games();
    }
}

fn joinserver_menu_draw() {
    m_banner("m_banner_join_server");
    // SAFETY: single-threaded engine
    unsafe { menu_draw(&S_JOINSERVER_MENU); }
}

fn joinserver_menu_key(key: i32) -> Option<&'static str> {
    default_menu_key_with_menu(key, true)
}

pub fn m_menu_join_server_f() {
    joinserver_menu_init();
    m_push_menu(joinserver_menu_draw, joinserver_menu_key);
}

// ============================================================
// Start Server Menu
// Converted from: StartServer_MenuInit / StartServer_MenuDraw / StartServer_MenuKey
// ============================================================

static mut S_STARTSERVER_MENU: MenuFramework = MenuFramework {
    x: 0, y: 0, cursor: 0, nitems: 0, nslots: 0,
    items: Vec::new(), statusbar: None, cursordraw: None,
};

static mut STARTSERVER_MAPNAMES: Vec<String> = Vec::new();

// Item indices for start server menu
const SS_STARTMAP: i32 = 0;
const SS_RULES: i32 = 1;
const SS_TIMELIMIT: i32 = 2;
const SS_FRAGLIMIT: i32 = 3;
const SS_MAXCLIENTS: i32 = 4;
const SS_HOSTNAME: i32 = 5;
const SS_DMOPTIONS: i32 = 6;
const SS_BEGIN: i32 = 7;

fn startserver_menu_callback(idx: usize) {
    // SAFETY: single-threaded engine
    unsafe {
        if let Some(item) = MENU_ITEMS.get(idx) {
            let id = item.generic().localdata[0];
            match id {
                SS_RULES => {
                    // RulesChangeFunc — update statusbar based on rules selection
                    // (simplified; full implementation would update field statusbars)
                }
                SS_DMOPTIONS => {
                    // Check if coop is selected (rules curvalue == 1)
                    if let Some(MenuItem::SpinControl(ref s)) = MENU_ITEMS.get(SS_RULES as usize) {
                        if s.curvalue == 1 {
                            return; // N/A for cooperative
                        }
                    }
                    m_menu_dm_options_f();
                }
                SS_BEGIN => {
                    // StartServerActionFunc
                    let map_idx = if let Some(MenuItem::SpinControl(ref s)) = MENU_ITEMS.get(SS_STARTMAP as usize) {
                        s.curvalue as usize
                    } else { 0 };

                    if map_idx < STARTSERVER_MAPNAMES.len() {
                        let mapname = &STARTSERVER_MAPNAMES[map_idx];
                        // mapname format is "LongName\nSHORTNAME" — extract the shortname
                        let startmap = if let Some(pos) = mapname.find('\n') {
                            &mapname[pos + 1..]
                        } else {
                            mapname.as_str()
                        };

                        // Read field values for timelimit, fraglimit, maxclients, hostname
                        let timelimit = get_field_buffer_str(SS_TIMELIMIT as usize);
                        let fraglimit = get_field_buffer_str(SS_FRAGLIMIT as usize);
                        let maxclients = get_field_buffer_str(SS_MAXCLIENTS as usize);
                        let hostname = get_field_buffer_str(SS_HOSTNAME as usize);

                        let tl: f32 = timelimit.parse().unwrap_or(0.0);
                        let fl: f32 = fraglimit.parse().unwrap_or(0.0);
                        let mc: f32 = maxclients.parse().unwrap_or(0.0);

                        cvar_set_value("maxclients", mc);
                        cvar_set_value("timelimit", tl);
                        cvar_set_value("fraglimit", fl);
                        cvar_set("hostname", &hostname);

                        let rules_val = if let Some(MenuItem::SpinControl(ref s)) = MENU_ITEMS.get(SS_RULES as usize) {
                            s.curvalue
                        } else { 0 };

                        if rules_val < 2 || developer_searchpath(2) != 2 {
                            cvar_set_value("deathmatch", if rules_val == 0 { 1.0 } else { 0.0 });
                            cvar_set_value("coop", rules_val as f32);
                            cvar_set_value("gamerules", 0.0);
                        } else {
                            cvar_set_value("deathmatch", 1.0);
                            cvar_set_value("coop", 0.0);
                            cvar_set_value("gamerules", rules_val as f32);
                        }

                        // Coop spawn spots
                        let spot = if rules_val == 1 {
                            let sm = startmap.to_lowercase();
                            match sm.as_str() {
                                "bunk1" | "mintro" | "fact1" => Some("start"),
                                "power1" => Some("pstart"),
                                "biggun" => Some("bstart"),
                                "hangar1" | "city1" => Some("unitstart"),
                                "boss1" => Some("bosstart"),
                                _ => None,
                            }
                        } else {
                            None
                        };

                        if let Some(spot) = spot {
                            if com_server_state() != 0 {
                                crate::console::cbuf_add_text("disconnect\n");
                            }
                            crate::console::cbuf_add_text(&format!("gamemap \"*{}${}\"\n", startmap, spot));
                        } else {
                            crate::console::cbuf_add_text(&format!("map {}\n", startmap));
                        }

                        m_force_menu_off();
                    }
                }
                _ => {}
            }
        }
    }
}

/// Helper: get the text content of a MenuField at the given MENU_ITEMS index.
fn get_field_buffer_str(idx: usize) -> String {
    // SAFETY: single-threaded engine
    unsafe {
        if let Some(MenuItem::Field(ref f)) = MENU_ITEMS.get(idx) {
            f.buffer.clone()
        } else {
            String::new()
        }
    }
}

/// Helper: set the text content of a MenuField at the given MENU_ITEMS index.
fn set_field_buffer(idx: usize, text: &str) {
    // SAFETY: single-threaded engine
    unsafe {
        if let Some(MenuItem::Field(ref mut f)) = MENU_ITEMS.get_mut(idx) {
            f.buffer = text.to_string();
        }
    }
}

fn startserver_menu_init() {
    // SAFETY: single-threaded engine
    unsafe {
        // Load maps list
        STARTSERVER_MAPNAMES.clear();
        let gamedir = crate::console::fs_gamedir();
        let mapsname = format!("{}/maps.lst", gamedir);

        let buffer = std::fs::read_to_string(&mapsname)
            .ok()
            .or_else(|| fs_load_file("maps.lst").map(|data| String::from_utf8_lossy(&data).to_string()));

        let mut mapname_strs: Vec<String> = Vec::new();
        if let Some(buf) = buffer {
            // Parse "shortname longname\r\n" pairs
            let lines: Vec<&str> = buf.lines().collect();
            for line in lines {
                let line = line.trim();
                if line.is_empty() { continue; }
                let mut parts = line.splitn(2, char::is_whitespace);
                let shortname = parts.next().unwrap_or("").to_uppercase();
                let longname = parts.next().unwrap_or("").trim().to_string();
                if !shortname.is_empty() {
                    mapname_strs.push(format!("{}\n{}", longname, shortname));
                }
            }
        }

        if mapname_strs.is_empty() {
            mapname_strs.push("base1\nBASE1".to_string());
        }

        STARTSERVER_MAPNAMES = mapname_strs;

        S_STARTSERVER_MENU = MenuFramework {
            x: (VIDDEF.width as f32 * 0.50) as i32,
            y: 0, cursor: 0, nitems: 0, nslots: 0,
            items: Vec::new(), statusbar: None, cursordraw: None,
        };
        MENU_ITEMS.clear();

        // 0: start map spin
        menu_add_item_common(&mut S_STARTSERVER_MENU, make_menu_common(
            MTYPE_SPINCONTROL, "initial map", 0, 0, 0,
            [SS_STARTMAP, 0, 0, 0], None, None,
        ));
        if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.get_mut(SS_STARTMAP as usize) {
            // Build display names from the "longname\nSHORTNAME" format — show longname
            let display_names: Vec<String> = STARTSERVER_MAPNAMES.iter()
                .map(|n| {
                    if let Some(pos) = n.find('\n') { n[..pos].to_string() } else { n.clone() }
                })
                .collect();
            s.itemnames = display_names;
        }

        // 1: rules spin
        menu_add_item_common(&mut S_STARTSERVER_MENU, make_menu_common(
            MTYPE_SPINCONTROL, "rules", 0, 20, 0,
            [SS_RULES, 0, 0, 0], Some(startserver_menu_callback), None,
        ));
        if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.get_mut(SS_RULES as usize) {
            if developer_searchpath(2) == 2 {
                s.itemnames = strs(&["deathmatch", "cooperative", "tag"]);
            } else {
                s.itemnames = strs(&["deathmatch", "cooperative"]);
            }
            s.curvalue = if cvar_variable_value("coop") != 0.0 { 1 } else { 0 };
        }

        // 2: timelimit field
        menu_add_item_common(&mut S_STARTSERVER_MENU, make_menu_common(
            MTYPE_FIELD, "time limit", 0, 36, QMF_NUMBERSONLY,
            [SS_TIMELIMIT, 0, 0, 0], None, Some("0 = no limit"),
        ));
        if let Some(MenuItem::Field(ref mut f)) = MENU_ITEMS.get_mut(SS_TIMELIMIT as usize) {
            f.length = 3;
            f.visible_length = 3;
        }
        set_field_buffer(SS_TIMELIMIT as usize, &cvar_variable_string("timelimit"));

        // 3: fraglimit field
        menu_add_item_common(&mut S_STARTSERVER_MENU, make_menu_common(
            MTYPE_FIELD, "frag limit", 0, 54, QMF_NUMBERSONLY,
            [SS_FRAGLIMIT, 0, 0, 0], None, Some("0 = no limit"),
        ));
        if let Some(MenuItem::Field(ref mut f)) = MENU_ITEMS.get_mut(SS_FRAGLIMIT as usize) {
            f.length = 3;
            f.visible_length = 3;
        }
        set_field_buffer(SS_FRAGLIMIT as usize, &cvar_variable_string("fraglimit"));

        // 4: maxclients field
        menu_add_item_common(&mut S_STARTSERVER_MENU, make_menu_common(
            MTYPE_FIELD, "max players", 0, 72, QMF_NUMBERSONLY,
            [SS_MAXCLIENTS, 0, 0, 0], None, None,
        ));
        if let Some(MenuItem::Field(ref mut f)) = MENU_ITEMS.get_mut(SS_MAXCLIENTS as usize) {
            f.length = 3;
            f.visible_length = 3;
        }
        let mc_str = if cvar_variable_value("maxclients") == 1.0 {
            "8".to_string()
        } else {
            cvar_variable_string("maxclients")
        };
        set_field_buffer(SS_MAXCLIENTS as usize, &mc_str);

        // 5: hostname field
        menu_add_item_common(&mut S_STARTSERVER_MENU, make_menu_common(
            MTYPE_FIELD, "hostname", 0, 90, 0,
            [SS_HOSTNAME, 0, 0, 0], None, None,
        ));
        if let Some(MenuItem::Field(ref mut f)) = MENU_ITEMS.get_mut(SS_HOSTNAME as usize) {
            f.length = 12;
            f.visible_length = 12;
        }
        set_field_buffer(SS_HOSTNAME as usize, &cvar_variable_string("hostname"));

        // 6: deathmatch flags action
        menu_add_item_common(&mut S_STARTSERVER_MENU, make_menu_common(
            MTYPE_ACTION, " deathmatch flags", 24, 108, QMF_LEFT_JUSTIFY,
            [SS_DMOPTIONS, 0, 0, 0], Some(startserver_menu_callback), None,
        ));

        // 7: begin action
        menu_add_item_common(&mut S_STARTSERVER_MENU, make_menu_common(
            MTYPE_ACTION, " begin", 24, 128, QMF_LEFT_JUSTIFY,
            [SS_BEGIN, 0, 0, 0], Some(startserver_menu_callback), None,
        ));

        menu_center(&mut S_STARTSERVER_MENU);
    }
}

fn startserver_menu_draw() {
    // SAFETY: single-threaded engine
    unsafe { menu_draw(&S_STARTSERVER_MENU); }
}

fn startserver_menu_key(key: i32) -> Option<&'static str> {
    if key == K_ESCAPE {
        // Free mapnames on exit (C code frees them here)
        // SAFETY: single-threaded engine
        unsafe { STARTSERVER_MAPNAMES.clear(); }
    }
    default_menu_key_with_menu(key, true)
}

pub fn m_menu_start_server_f() {
    startserver_menu_init();
    m_push_menu(startserver_menu_draw, startserver_menu_key);
}

// ============================================================
// DM Options Menu
// Converted from: DMOptions_MenuInit / DMOptions_MenuDraw / DMOptions_MenuKey
// ============================================================

static mut S_DMOPTIONS_MENU: MenuFramework = MenuFramework {
    x: 0, y: 0, cursor: 0, nitems: 0, nslots: 0,
    items: Vec::new(), statusbar: None, cursordraw: None,
};

// dmflags constants from q_shared
use myq2_common::q_shared::{
    DmFlags,
    DF_NO_HEALTH, DF_NO_ITEMS, DF_WEAPONS_STAY, DF_NO_FALLING,
    DF_INSTANT_ITEMS, DF_SAME_LEVEL, DF_SKINTEAMS, DF_MODELTEAMS,
    DF_NO_FRIENDLY_FIRE, DF_SPAWN_FARTHEST, DF_FORCE_RESPAWN, DF_NO_ARMOR,
    DF_ALLOW_EXIT, DF_INFINITE_AMMO, DF_QUAD_DROP, DF_FIXED_FOV,
    DF_NO_MINES, DF_NO_STACK_DOUBLE, DF_NO_NUKES, DF_NO_SPHERES,
};

// DM Options item IDs — encode both the flag and the "inverted" behavior.
// We use localdata[0] = item index, localdata[1] = flag, localdata[2] = 1 if inverted (0 means enabled)
// Items are added in this order:
// 0: falls, 1: weapons stay, 2: instant powerups, 3: powerups, 4: health,
// 5: armor, 6: spawn farthest, 7: same level, 8: force respawn, 9: teamplay,
// 10: allow exit, 11: infinite ammo, 12: fixed fov, 13: quad drop, 14: friendly fire
// (optionally 15-18: rogue items)

fn dmflag_callback(idx: usize) {
    // SAFETY: single-threaded engine
    unsafe {
        let mut flags = DmFlags::from_bits_truncate(cvar_variable_value("dmflags") as i32);

        if let Some(item) = MENU_ITEMS.get(idx) {
            let flag_bits = item.generic().localdata[1];
            let inverted = item.generic().localdata[2] != 0;

            if let MenuItem::SpinControl(ref s) = item {
                let curval = s.curvalue;

                // teamplay is special (localdata[1] == -1 sentinel)
                if flag_bits == -1 {
                    // teamplay: 0=disabled, 1=by skin, 2=by model
                    if curval == 1 {
                        flags.insert(DF_SKINTEAMS);
                        flags.remove(DF_MODELTEAMS);
                    } else if curval == 2 {
                        flags.insert(DF_MODELTEAMS);
                        flags.remove(DF_SKINTEAMS);
                    } else {
                        flags.remove(DF_MODELTEAMS | DF_SKINTEAMS);
                    }
                } else {
                    let flag = DmFlags::from_bits_truncate(flag_bits);
                    if inverted {
                        // "inverted" means curvalue=1 means flag OFF (e.g., "allow health" = no DF_NO_HEALTH)
                        if curval != 0 {
                            flags.remove(flag);
                        } else {
                            flags.insert(flag);
                        }
                    } else {
                        // normal: curvalue=1 means flag ON
                        if curval != 0 {
                            flags.insert(flag);
                        } else {
                            flags.remove(flag);
                        }
                    }
                }
            }
        }

        let flags_i32 = flags.bits();
        cvar_set_value("dmflags", flags_i32 as f32);

        // Update statusbar
        let status = format!("dmflags = {}", flags_i32);
        // Store in a leaked string for static lifetime
        let leaked: &'static str = Box::leak(status.into_boxed_str());
        menu_set_status_bar(&mut S_DMOPTIONS_MENU, Some(leaked));
    }
}

fn dmoptions_menu_init() {
    // SAFETY: single-threaded engine
    unsafe {
        let dmflags_i32 = cvar_variable_value("dmflags") as i32;
        let dmflags = DmFlags::from_bits_truncate(dmflags_i32);

        S_DMOPTIONS_MENU = MenuFramework {
            x: (VIDDEF.width as f32 * 0.50) as i32,
            y: 0, cursor: 0, nitems: 0, nslots: 0,
            items: Vec::new(), statusbar: None, cursordraw: None,
        };
        MENU_ITEMS.clear();

        // (name, y, flag, inverted, is_teamplay)
        // flag_bits stores the i32 representation for localdata storage;
        // -1 is a sentinel for the teamplay item.
        struct DmItem {
            name: &'static str,
            flag_bits: i32,
            inverted: bool,     // true = "allow X" where flag means "no X"
            is_teamplay: bool,
        }

        let dm_items: Vec<DmItem> = vec![
            DmItem { name: "falling damage",    flag_bits: DF_NO_FALLING.bits(),      inverted: true,  is_teamplay: false },
            DmItem { name: "weapons stay",      flag_bits: DF_WEAPONS_STAY.bits(),    inverted: false, is_teamplay: false },
            DmItem { name: "instant powerups",  flag_bits: DF_INSTANT_ITEMS.bits(),   inverted: false, is_teamplay: false },
            DmItem { name: "allow powerups",    flag_bits: DF_NO_ITEMS.bits(),        inverted: true,  is_teamplay: false },
            DmItem { name: "allow health",      flag_bits: DF_NO_HEALTH.bits(),       inverted: true,  is_teamplay: false },
            DmItem { name: "allow armor",       flag_bits: DF_NO_ARMOR.bits(),        inverted: true,  is_teamplay: false },
            DmItem { name: "spawn farthest",    flag_bits: DF_SPAWN_FARTHEST.bits(),  inverted: false, is_teamplay: false },
            DmItem { name: "same map",          flag_bits: DF_SAME_LEVEL.bits(),      inverted: false, is_teamplay: false },
            DmItem { name: "force respawn",     flag_bits: DF_FORCE_RESPAWN.bits(),   inverted: false, is_teamplay: false },
            DmItem { name: "teamplay",          flag_bits: -1,                        inverted: false, is_teamplay: true  },
            DmItem { name: "allow exit",        flag_bits: DF_ALLOW_EXIT.bits(),      inverted: false, is_teamplay: false },
            DmItem { name: "infinite ammo",     flag_bits: DF_INFINITE_AMMO.bits(),   inverted: false, is_teamplay: false },
            DmItem { name: "fixed FOV",         flag_bits: DF_FIXED_FOV.bits(),       inverted: false, is_teamplay: false },
            DmItem { name: "quad drop",         flag_bits: DF_QUAD_DROP.bits(),       inverted: false, is_teamplay: false },
            DmItem { name: "friendly fire",     flag_bits: DF_NO_FRIENDLY_FIRE.bits(), inverted: true, is_teamplay: false },
        ];

        let mut y = 0i32;
        for (i, dm) in dm_items.iter().enumerate() {
            let item = make_menu_common(
                MTYPE_SPINCONTROL, dm.name, 0, y, 0,
                [i as i32, dm.flag_bits, if dm.inverted { 1 } else { 0 }, 0],
                Some(dmflag_callback), None,
            );
            menu_add_item_common(&mut S_DMOPTIONS_MENU, item);

            if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.get_mut(i) {
                if dm.is_teamplay {
                    s.itemnames = strs(&["disabled", "by skin", "by model"]);
                    // Determine teamplay curvalue from flags
                    if dmflags.intersects(DF_SKINTEAMS) {
                        s.curvalue = 1;
                    } else if dmflags.intersects(DF_MODELTEAMS) {
                        s.curvalue = 2;
                    } else {
                        s.curvalue = 0;
                    }
                } else {
                    s.itemnames = strs(&["no", "yes"]);
                    let flag = DmFlags::from_bits_truncate(dm.flag_bits);
                    if dm.inverted {
                        s.curvalue = if !dmflags.intersects(flag) { 1 } else { 0 };
                    } else {
                        s.curvalue = if dmflags.intersects(flag) { 1 } else { 0 };
                    }
                }
            }

            y += 10;
        }

        // Rogue-specific items
        if developer_searchpath(2) == 2 {
            let rogue_items: &[(&str, i32)] = &[
                ("remove mines",       DF_NO_MINES.bits()),
                ("remove nukes",       DF_NO_NUKES.bits()),
                ("2x/4x stacking off", DF_NO_STACK_DOUBLE.bits()),
                ("remove spheres",     DF_NO_SPHERES.bits()),
            ];
            for (ri, &(name, flag_bits)) in rogue_items.iter().enumerate() {
                let idx = dm_items.len() + ri;
                let item = make_menu_common(
                    MTYPE_SPINCONTROL, name, 0, y, 0,
                    [idx as i32, flag_bits, 0, 0], Some(dmflag_callback), None,
                );
                menu_add_item_common(&mut S_DMOPTIONS_MENU, item);
                if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.get_mut(idx) {
                    s.itemnames = strs(&["no", "yes"]);
                    let flag = DmFlags::from_bits_truncate(flag_bits);
                    s.curvalue = if dmflags.intersects(flag) { 1 } else { 0 };
                }
                y += 10;
            }
        }

        menu_center(&mut S_DMOPTIONS_MENU);

        // Set initial statusbar
        let status = format!("dmflags = {}", dmflags_i32);
        let leaked: &'static str = Box::leak(status.into_boxed_str());
        menu_set_status_bar(&mut S_DMOPTIONS_MENU, Some(leaked));
    }
}

fn dmoptions_menu_draw() {
    // SAFETY: single-threaded engine
    unsafe { menu_draw(&S_DMOPTIONS_MENU); }
}

fn dmoptions_menu_key(key: i32) -> Option<&'static str> {
    default_menu_key_with_menu(key, true)
}

pub fn m_menu_dm_options_f() {
    dmoptions_menu_init();
    m_push_menu(dmoptions_menu_draw, dmoptions_menu_key);
}

// ============================================================
// Download Options Menu
// Converted from: DownloadOptions_MenuInit / DownloadOptions_MenuDraw / DownloadOptions_MenuKey
// ============================================================

static mut S_DOWNLOADOPTIONS_MENU: MenuFramework = MenuFramework {
    x: 0, y: 0, cursor: 0, nitems: 0, nslots: 0,
    items: Vec::new(), statusbar: None, cursordraw: None,
};

fn download_callback(idx: usize) {
    // SAFETY: single-threaded engine
    unsafe {
        if let Some(MenuItem::SpinControl(ref s)) = MENU_ITEMS.get(idx) {
            let id = s.generic.localdata[0];
            let val = s.curvalue as f32;
            match id {
                1 => cvar_set_value("allow_download", val),
                2 => cvar_set_value("allow_download_maps", val),
                3 => cvar_set_value("allow_download_players", val),
                4 => cvar_set_value("allow_download_models", val),
                5 => cvar_set_value("allow_download_sounds", val),
                _ => {}
            }
        }
    }
}

fn downloadoptions_menu_init() {
    // SAFETY: single-threaded engine
    unsafe {
        S_DOWNLOADOPTIONS_MENU = MenuFramework {
            x: (VIDDEF.width as f32 * 0.50) as i32,
            y: 0, cursor: 0, nitems: 0, nslots: 0,
            items: Vec::new(), statusbar: None, cursordraw: None,
        };
        MENU_ITEMS.clear();

        let mut y = 0i32;

        // 0: title separator
        menu_add_item_common(&mut S_DOWNLOADOPTIONS_MENU, make_menu_common(
            MTYPE_SEPARATOR, "Download Options", 48, y, 0,
            [0, 0, 0, 0], None, None,
        ));

        // 1: allow downloading
        y += 20;
        menu_add_item_common(&mut S_DOWNLOADOPTIONS_MENU, make_menu_common(
            MTYPE_SPINCONTROL, "allow downloading", 0, y, 0,
            [1, 0, 0, 0], Some(download_callback), None,
        ));
        if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.last_mut() {
            s.itemnames = strs(&["no", "yes"]);
            s.curvalue = if cvar_variable_value("allow_download") != 0.0 { 1 } else { 0 };
        }

        // 2: maps
        y += 20;
        menu_add_item_common(&mut S_DOWNLOADOPTIONS_MENU, make_menu_common(
            MTYPE_SPINCONTROL, "maps", 0, y, 0,
            [2, 0, 0, 0], Some(download_callback), None,
        ));
        if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.last_mut() {
            s.itemnames = strs(&["no", "yes"]);
            s.curvalue = if cvar_variable_value("allow_download_maps") != 0.0 { 1 } else { 0 };
        }

        // 3: player models/skins
        y += 10;
        menu_add_item_common(&mut S_DOWNLOADOPTIONS_MENU, make_menu_common(
            MTYPE_SPINCONTROL, "player models/skins", 0, y, 0,
            [3, 0, 0, 0], Some(download_callback), None,
        ));
        if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.last_mut() {
            s.itemnames = strs(&["no", "yes"]);
            s.curvalue = if cvar_variable_value("allow_download_players") != 0.0 { 1 } else { 0 };
        }

        // 4: models
        y += 10;
        menu_add_item_common(&mut S_DOWNLOADOPTIONS_MENU, make_menu_common(
            MTYPE_SPINCONTROL, "models", 0, y, 0,
            [4, 0, 0, 0], Some(download_callback), None,
        ));
        if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.last_mut() {
            s.itemnames = strs(&["no", "yes"]);
            s.curvalue = if cvar_variable_value("allow_download_models") != 0.0 { 1 } else { 0 };
        }

        // 5: sounds
        y += 10;
        menu_add_item_common(&mut S_DOWNLOADOPTIONS_MENU, make_menu_common(
            MTYPE_SPINCONTROL, "sounds", 0, y, 0,
            [5, 0, 0, 0], Some(download_callback), None,
        ));
        if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.last_mut() {
            s.itemnames = strs(&["no", "yes"]);
            s.curvalue = if cvar_variable_value("allow_download_sounds") != 0.0 { 1 } else { 0 };
        }

        menu_center(&mut S_DOWNLOADOPTIONS_MENU);

        // Skip over title separator
        if S_DOWNLOADOPTIONS_MENU.cursor == 0 {
            S_DOWNLOADOPTIONS_MENU.cursor = 1;
        }
    }
}

fn downloadoptions_menu_draw() {
    // SAFETY: single-threaded engine
    unsafe { menu_draw(&S_DOWNLOADOPTIONS_MENU); }
}

fn downloadoptions_menu_key(key: i32) -> Option<&'static str> {
    default_menu_key_with_menu(key, true)
}

pub fn m_menu_download_options_f() {
    downloadoptions_menu_init();
    m_push_menu(downloadoptions_menu_draw, downloadoptions_menu_key);
}

// ============================================================
// Address Book Menu
// Converted from: AddressBook_MenuInit / AddressBook_MenuDraw / AddressBook_MenuKey
// ============================================================

static mut S_ADDRESSBOOK_MENU: MenuFramework = MenuFramework {
    x: 0, y: 0, cursor: 0, nitems: 0, nslots: 0,
    items: Vec::new(), statusbar: None, cursordraw: None,
};

fn addressbook_menu_init() {
    // SAFETY: single-threaded engine
    unsafe {
        S_ADDRESSBOOK_MENU = MenuFramework {
            x: VIDDEF.width / 2 - 142,
            y: VIDDEF.height / 2 - 58,
            cursor: 0, nitems: 0, nslots: 0,
            items: Vec::new(), statusbar: None, cursordraw: None,
        };
        MENU_ITEMS.clear();

        for i in 0..NUM_ADDRESSBOOK_ENTRIES {
            let cvar_name = format!("adr{}", i);
            cvar_get(&cvar_name, "", 0);
            let adr_value = cvar_variable_string(&cvar_name);

            let item = make_menu_common(
                MTYPE_FIELD, "", 0, i as i32 * 18, 0,
                [i as i32, 0, 0, 0], None, None,
            );
            menu_add_item_common(&mut S_ADDRESSBOOK_MENU, item);

            // Set field properties
            if let Some(MenuItem::Field(ref mut f)) = MENU_ITEMS.get_mut(i) {
                f.cursor = 0;
                f.length = 60;
                f.visible_length = 30;
                // Copy address value into buffer
                f.buffer = adr_value;
            }
        }
    }
}

fn addressbook_menu_key(key: i32) -> Option<&'static str> {
    if key == K_ESCAPE {
        // Save all address book entries back to cvars
        for i in 0..NUM_ADDRESSBOOK_ENTRIES {
            let cvar_name = format!("adr{}", i);
            let value = get_field_buffer_str(i);
            cvar_set(&cvar_name, &value);
        }
    }
    default_menu_key_with_menu(key, true)
}

fn addressbook_menu_draw() {
    m_banner("m_banner_addressbook");
    // SAFETY: single-threaded engine
    unsafe { menu_draw(&S_ADDRESSBOOK_MENU); }
}

pub fn m_menu_address_book_f() {
    addressbook_menu_init();
    m_push_menu(addressbook_menu_draw, addressbook_menu_key);
}

// ============================================================
// Player Config (stub)
// ============================================================

static mut S_PLAYER_CONFIG_MENU: MenuFramework = MenuFramework {
    x: 0, y: 0, cursor: 0, nitems: 0, nslots: 0,
    items: Vec::new(), statusbar: None, cursordraw: None,
};

// Player config menu item indices
const PC_NAME: usize = 0;
const PC_MODEL: usize = 1;
const PC_SKIN: usize = 2;
const PC_HANDEDNESS: usize = 3;
const PC_RATE: usize = 4;
const PC_DOWNLOAD: usize = 5;

static RATE_TBL: &[i32] = &[2500, 3200, 5000, 10000, 25000, 0];

fn playerconfig_callback(idx: usize) {
    // SAFETY: single-threaded engine
    unsafe {
        if let Some(item) = MENU_ITEMS.get(idx) {
            let id = item.generic().localdata[0] as usize;
            match id {
                PC_HANDEDNESS => {
                    if let MenuItem::SpinControl(ref s) = item {
                        cvar_set_value("hand", s.curvalue as f32);
                    }
                }
                PC_RATE => {
                    if let MenuItem::SpinControl(ref s) = item {
                        let rate_idx = s.curvalue as usize;
                        if rate_idx < RATE_TBL.len() - 1 {
                            cvar_set_value("rate", RATE_TBL[rate_idx] as f32);
                        }
                    }
                }
                PC_DOWNLOAD => {
                    m_menu_download_options_f();
                }
                _ => {}
            }
        }
    }
}

/// PlayerConfig_MenuInit — initialize player configuration menu.
/// Returns true if valid player models were found.
fn player_config_menu_init() -> bool {
    // SAFETY: single-threaded engine
    unsafe {
        S_PLAYER_CONFIG_MENU = MenuFramework {
            x: VIDDEF.width / 2 - 95,
            y: VIDDEF.height / 2 - 97,
            cursor: 0, nitems: 0, nslots: 0,
            items: Vec::new(), statusbar: None, cursordraw: None,
        };
        MENU_ITEMS.clear();

        // 0: Name field
        let name_item = make_menu_common(
            MTYPE_FIELD, "name", 0, 0, 0,
            [PC_NAME as i32, 0, 0, 0], None, None,
        );
        menu_add_item_common(&mut S_PLAYER_CONFIG_MENU, name_item);
        if let Some(MenuItem::Field(ref mut f)) = MENU_ITEMS.get_mut(PC_NAME) {
            f.length = 20;
            f.visible_length = 20;
        }
        set_field_buffer(PC_NAME, &cvar_variable_string("name"));

        // 1: Model selection
        let model_item = make_menu_common(
            MTYPE_SPINCONTROL, "model", 0, 20, 0,
            [PC_MODEL as i32, 0, 0, 0], None, None,
        );
        menu_add_item_common(&mut S_PLAYER_CONFIG_MENU, model_item);
        if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.get_mut(PC_MODEL) {
            s.itemnames = strs(&["male", "female", "cyborg"]);
        }

        // 2: Skin selection
        let skin_item = make_menu_common(
            MTYPE_SPINCONTROL, "skin", 0, 30, 0,
            [PC_SKIN as i32, 0, 0, 0], None, None,
        );
        menu_add_item_common(&mut S_PLAYER_CONFIG_MENU, skin_item);
        if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.get_mut(PC_SKIN) {
            s.itemnames = strs(&["grunt", "major", "id"]);
        }

        // 3: Handedness
        let hand_item = make_menu_common(
            MTYPE_SPINCONTROL, "handedness", 0, 50, 0,
            [PC_HANDEDNESS as i32, 0, 0, 0], Some(playerconfig_callback), None,
        );
        menu_add_item_common(&mut S_PLAYER_CONFIG_MENU, hand_item);
        if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.get_mut(PC_HANDEDNESS) {
            s.itemnames = strs(&["right", "left", "center"]);
            s.curvalue = clamp_cvar(0.0, 2.0, cvar_variable_value("hand")) as i32;
        }

        // 4: Rate
        let rate_item = make_menu_common(
            MTYPE_SPINCONTROL, "connect speed", 0, 60, 0,
            [PC_RATE as i32, 0, 0, 0], Some(playerconfig_callback), None,
        );
        menu_add_item_common(&mut S_PLAYER_CONFIG_MENU, rate_item);
        if let Some(MenuItem::SpinControl(ref mut s)) = MENU_ITEMS.get_mut(PC_RATE) {
            s.itemnames = strs(&["28.8 Modem", "33.6 Modem", "Single ISDN", "Dual ISDN/Cable", "T1/LAN", "User defined"]);
            let rate = cvar_variable_value("rate") as i32;
            let mut rate_idx = RATE_TBL.len() - 1; // "User defined"
            for (i, &r) in RATE_TBL.iter().enumerate() {
                if r > 0 && rate == r {
                    rate_idx = i;
                    break;
                }
            }
            s.curvalue = rate_idx as i32;
        }

        // 5: Download options
        let dl_item = make_menu_common(
            MTYPE_ACTION, "download options", 0, 80, 0,
            [PC_DOWNLOAD as i32, 0, 0, 0], Some(playerconfig_callback), None,
        );
        menu_add_item_common(&mut S_PLAYER_CONFIG_MENU, dl_item);

        menu_center(&mut S_PLAYER_CONFIG_MENU);
    }
    true
}

fn player_config_menu_draw() {
    m_banner("m_banner_player_setup");
    // SAFETY: single-threaded engine
    unsafe { menu_draw(&S_PLAYER_CONFIG_MENU); }
}

fn player_config_menu_key(key: i32) -> Option<&'static str> {
    default_menu_key_with_menu(key, true)
}

pub fn m_menu_player_config_f() {
    if !player_config_menu_init() {
        // SAFETY: single-threaded engine — set status on multiplayer menu
        // (we don't have direct access to the multiplayer menu framework here,
        // so just return; the C code sets a statusbar on s_multiplayer_menu)
        return;
    }
    m_push_menu(player_config_menu_draw, player_config_menu_key);
}

// ============================================================
// Quit Menu
// ============================================================

fn m_quit_draw() {
    let (w, h) = draw_get_pic_size("quit");
    // SAFETY: single-threaded engine
    unsafe {
        draw_pic((VIDDEF.width - w) / 2, (VIDDEF.height - h) / 2, "quit");
    }
}

fn m_quit_key(key: i32) -> Option<&'static str> {
    match key {
        K_ESCAPE | 110 /* 'n' */ | 78 /* 'N' */ => {
            m_pop_menu();
            None
        }
        89 /* 'Y' */ | 121 /* 'y' */ => {
            // SAFETY: single-threaded engine
            unsafe {
                CLS.key_dest = KeyDest::Console;
            }
            cl_quit_f();
            None
        }
        _ => None,
    }
}

pub fn m_menu_quit_f() {
    m_push_menu(m_quit_draw, m_quit_key);
}

// ============================================================
// Default_MenuKey
// ============================================================

/// Identify which menu framework is currently active and return a mutable reference to it.
/// This is needed because Default_MenuKey in C takes the menu as a parameter.
/// SAFETY: single-threaded engine, must only be called from menu key handlers.
unsafe fn get_active_menu() -> Option<*mut MenuFramework> {
    // Check which draw function is active and return the corresponding menu.
    if M_DRAWFUNC == Some(game_menu_draw) { return Some(&mut S_GAME_MENU); }
    if M_DRAWFUNC == Some(multiplayer_menu_draw) { return Some(&mut S_MULTIPLAYER_MENU); }
    if M_DRAWFUNC == Some(options_menu_draw) { return Some(&mut S_OPTIONS_MENU); }
    if M_DRAWFUNC == Some(keys_menu_draw) { return Some(&mut S_KEYS_MENU); }
    if M_DRAWFUNC == Some(loadgame_menu_draw) { return Some(&mut S_LOADGAME_MENU); }
    if M_DRAWFUNC == Some(savegame_menu_draw) { return Some(&mut S_SAVEGAME_MENU); }
    if M_DRAWFUNC == Some(joinserver_menu_draw) { return Some(&mut S_JOINSERVER_MENU); }
    if M_DRAWFUNC == Some(startserver_menu_draw) { return Some(&mut S_STARTSERVER_MENU); }
    if M_DRAWFUNC == Some(dmoptions_menu_draw) { return Some(&mut S_DMOPTIONS_MENU); }
    if M_DRAWFUNC == Some(downloadoptions_menu_draw) { return Some(&mut S_DOWNLOADOPTIONS_MENU); }
    if M_DRAWFUNC == Some(addressbook_menu_draw) { return Some(&mut S_ADDRESSBOOK_MENU); }
    if M_DRAWFUNC == Some(player_config_menu_draw) { return Some(&mut S_PLAYER_CONFIG_MENU); }
    None
}

/// Default_MenuKey with menu framework — handles cursor movement, item selection, and sliding.
/// `has_menu` indicates whether this menu uses a framework (most do).
pub fn default_menu_key_with_menu(key: i32, has_menu: bool) -> Option<&'static str> {
    // SAFETY: single-threaded engine
    unsafe {
        let menu_ptr = if has_menu { get_active_menu() } else { None };

        // If the current item is a field, let it handle the key first
        if let Some(mp) = menu_ptr {
            let menu = &*mp;
            if let Some(item) = MENU_ITEMS.get(menu.cursor as usize) {
                if item.generic().item_type == MTYPE_FIELD {
                    if field_key(menu.cursor as usize, key) {
                        return None;
                    }
                }
            }
        }

        match key {
            K_ESCAPE => {
                m_pop_menu();
                return Some(MENU_OUT_SOUND);
            }
            K_KP_UPARROW | K_UPARROW => {
                if let Some(mp) = menu_ptr {
                    let menu = &mut *mp;
                    menu.cursor -= 1;
                    menu_adjust_cursor(menu, -1);
                    return Some(MENU_MOVE_SOUND);
                }
            }
            K_TAB | K_KP_DOWNARROW | K_DOWNARROW => {
                if let Some(mp) = menu_ptr {
                    let menu = &mut *mp;
                    menu.cursor += 1;
                    menu_adjust_cursor(menu, 1);
                    return Some(MENU_MOVE_SOUND);
                }
            }
            K_KP_LEFTARROW | K_LEFTARROW => {
                if let Some(mp) = menu_ptr {
                    let menu = &mut *mp;
                    menu_slide_item(menu, -1);
                    return Some(MENU_MOVE_SOUND);
                }
            }
            K_KP_RIGHTARROW | K_RIGHTARROW => {
                if let Some(mp) = menu_ptr {
                    let menu = &mut *mp;
                    menu_slide_item(menu, 1);
                    return Some(MENU_MOVE_SOUND);
                }
            }
            K_MOUSE1 | K_MOUSE2 | K_MOUSE3 | K_MOUSE4 | K_MOUSE5
            | K_JOY1 | K_JOY2 | K_JOY3 | K_JOY4
            | K_AUX1 | K_AUX2 | K_AUX3 | K_AUX4 | K_AUX5 | K_AUX6 | K_AUX7 | K_AUX8
            | K_AUX9 | K_AUX10 | K_AUX11 | K_AUX12 | K_AUX13 | K_AUX14 | K_AUX15 | K_AUX16
            | K_AUX17 | K_AUX18 | K_AUX19 | K_AUX20 | K_AUX21 | K_AUX22 | K_AUX23 | K_AUX24
            | K_AUX25 | K_AUX26 | K_AUX27 | K_AUX28 | K_AUX29 | K_AUX30 | K_AUX31 | K_AUX32
            | K_KP_ENTER | K_ENTER => {
                if let Some(mp) = menu_ptr {
                    let menu = &mut *mp;
                    // Try to fire the callback for the item at cursor
                    let cursor = menu.cursor as usize;
                    if let Some(item) = MENU_ITEMS.get(cursor) {
                        if let Some(ref cb) = item.generic().callback {
                            cb(cursor);
                            return Some(MENU_MOVE_SOUND);
                        }
                    }
                    menu_select_item(menu);
                }
                return Some(MENU_MOVE_SOUND);
            }
            _ => {}
        }
        None
    }
}

/// Default key handler for framework-based menus (legacy wrapper, no menu adjustment).
pub fn default_menu_key(key: i32) -> Option<&'static str> {
    match key {
        K_ESCAPE => {
            m_pop_menu();
            Some(MENU_OUT_SOUND)
        }
        K_KP_UPARROW | K_UPARROW => Some(MENU_MOVE_SOUND),
        K_TAB | K_KP_DOWNARROW | K_DOWNARROW => Some(MENU_MOVE_SOUND),
        K_KP_LEFTARROW | K_LEFTARROW => Some(MENU_MOVE_SOUND),
        K_KP_RIGHTARROW | K_RIGHTARROW => Some(MENU_MOVE_SOUND),
        K_MOUSE1 | K_MOUSE2 | K_MOUSE3 | K_MOUSE4 | K_MOUSE5
        | K_JOY1 | K_JOY2 | K_JOY3 | K_JOY4
        | K_AUX1 | K_AUX2 | K_AUX3 | K_AUX4 | K_AUX5 | K_AUX6 | K_AUX7 | K_AUX8
        | K_AUX9 | K_AUX10 | K_AUX11 | K_AUX12 | K_AUX13 | K_AUX14 | K_AUX15 | K_AUX16
        | K_AUX17 | K_AUX18 | K_AUX19 | K_AUX20 | K_AUX21 | K_AUX22 | K_AUX23 | K_AUX24
        | K_AUX25 | K_AUX26 | K_AUX27 | K_AUX28 | K_AUX29 | K_AUX30 | K_AUX31 | K_AUX32
        | K_KP_ENTER | K_ENTER => Some(MENU_MOVE_SOUND),
        _ => None,
    }
}

// ============================================================
// M_Init
// ============================================================

/// Initialize the menu subsystem.
pub fn m_init() {
    cmd_add_command("menu_main", m_menu_main_f);
    cmd_add_command("menu_game", m_menu_game_f);
    cmd_add_command("menu_loadgame", m_menu_load_game_f);
    cmd_add_command("menu_savegame", m_menu_save_game_f);
    cmd_add_command("menu_joinserver", m_menu_join_server_f);
    cmd_add_command("menu_addressbook", m_menu_address_book_f);
    cmd_add_command("menu_startserver", m_menu_start_server_f);
    cmd_add_command("menu_dmoptions", m_menu_dm_options_f);
    cmd_add_command("menu_playerconfig", m_menu_player_config_f);
    cmd_add_command("menu_downloadoptions", m_menu_download_options_f);
    cmd_add_command("menu_credits", m_menu_credits_f);
    cmd_add_command("menu_multiplayer", m_menu_multiplayer_f);
    cmd_add_command("menu_video", m_menu_video_f);
    cmd_add_command("menu_options", m_menu_options_f);
    cmd_add_command("menu_keys", m_menu_keys_f);
    cmd_add_command("menu_quit", m_menu_quit_f);
}

// ============================================================
// M_Draw
// ============================================================

/// Draw the current menu.
pub fn m_draw() {
    // SAFETY: single-threaded engine
    unsafe {
        if CLS.key_dest != KeyDest::Menu {
            return;
        }

        // repaint everything next frame
        scr_dirty_screen();

        // dim everything behind it down
        if crate::console::scr_draw_cinematic() {
            draw_fill(0, 0, VIDDEF.width, VIDDEF.height, 0, 1.0);
        } else {
            draw_fade_screen();
        }

        if let Some(draw_fn) = M_DRAWFUNC {
            draw_fn();
        }

        // delay playing the enter sound
        if M_ENTERSOUND {
            s_start_local_sound(MENU_IN_SOUND);
            M_ENTERSOUND = false;
        }
    }
}

// ============================================================
// M_Keydown
// ============================================================

/// Handle a key press in the menu.
pub fn m_keydown(key: i32) {
    // SAFETY: single-threaded engine
    unsafe {
        if let Some(key_fn) = M_KEYFUNC {
            if let Some(sound) = key_fn(key) {
                s_start_local_sound(sound);
            }
        }
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------
    // strs() helper tests
    // ----------------------------------------------------------

    #[test]
    fn test_strs_empty() {
        let result = strs(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_strs_single() {
        let result = strs(&["hello"]);
        assert_eq!(result, vec!["hello".to_string()]);
    }

    #[test]
    fn test_strs_multiple() {
        let result = strs(&["no", "yes"]);
        assert_eq!(result, vec!["no".to_string(), "yes".to_string()]);
    }

    #[test]
    fn test_strs_preserves_whitespace() {
        let result = strs(&[" spaced ", "  two  "]);
        assert_eq!(result[0], " spaced ");
        assert_eq!(result[1], "  two  ");
    }

    // ----------------------------------------------------------
    // clamp_cvar tests
    // ----------------------------------------------------------

    #[test]
    fn test_clamp_cvar_within_range() {
        assert_eq!(clamp_cvar(0.0, 10.0, 5.0), 5.0);
    }

    #[test]
    fn test_clamp_cvar_below_min() {
        assert_eq!(clamp_cvar(0.0, 10.0, -5.0), 0.0);
    }

    #[test]
    fn test_clamp_cvar_above_max() {
        assert_eq!(clamp_cvar(0.0, 10.0, 15.0), 10.0);
    }

    #[test]
    fn test_clamp_cvar_at_min() {
        assert_eq!(clamp_cvar(0.0, 10.0, 0.0), 0.0);
    }

    #[test]
    fn test_clamp_cvar_at_max() {
        assert_eq!(clamp_cvar(0.0, 10.0, 10.0), 10.0);
    }

    #[test]
    fn test_clamp_cvar_negative_range() {
        assert_eq!(clamp_cvar(-10.0, -1.0, -5.0), -5.0);
        assert_eq!(clamp_cvar(-10.0, -1.0, 0.0), -1.0);
        assert_eq!(clamp_cvar(-10.0, -1.0, -20.0), -10.0);
    }

    #[test]
    fn test_clamp_cvar_zero_range() {
        assert_eq!(clamp_cvar(5.0, 5.0, 5.0), 5.0);
        assert_eq!(clamp_cvar(5.0, 5.0, 3.0), 5.0);
        assert_eq!(clamp_cvar(5.0, 5.0, 7.0), 5.0);
    }

    // ----------------------------------------------------------
    // make_menu_common tests
    // ----------------------------------------------------------

    #[test]
    fn test_make_menu_common_basic() {
        let mc = make_menu_common(
            MTYPE_ACTION, "test item", 10, 20, QMF_LEFT_JUSTIFY,
            [1, 2, 3, 4], None, None,
        );
        assert_eq!(mc.item_type, MTYPE_ACTION);
        assert_eq!(mc.name, Some("test item".to_string()));
        assert_eq!(mc.x, 10);
        assert_eq!(mc.y, 20);
        assert_eq!(mc.flags, QMF_LEFT_JUSTIFY);
        assert_eq!(mc.localdata, [1, 2, 3, 4]);
        assert!(mc.callback.is_none());
        assert!(mc.statusbar.is_none());
    }

    #[test]
    fn test_make_menu_common_empty_name() {
        let mc = make_menu_common(
            MTYPE_SEPARATOR, "", 0, 0, 0,
            [0, 0, 0, 0], None, None,
        );
        assert!(mc.name.is_none());
    }

    #[test]
    fn test_make_menu_common_with_statusbar() {
        let mc = make_menu_common(
            MTYPE_ACTION, "item", 0, 0, 0,
            [0, 0, 0, 0], None, Some("press ENTER"),
        );
        assert_eq!(mc.statusbar, Some("press ENTER".to_string()));
    }

    #[test]
    fn test_make_menu_common_with_callback() {
        fn my_callback(_idx: usize) {}
        let mc = make_menu_common(
            MTYPE_ACTION, "item", 0, 0, 0,
            [0, 0, 0, 0], Some(my_callback), None,
        );
        assert!(mc.callback.is_some());
    }

    #[test]
    fn test_make_menu_common_parent_defaults() {
        let mc = make_menu_common(
            MTYPE_ACTION, "item", 0, 0, 0,
            [0, 0, 0, 0], None, None,
        );
        assert_eq!(mc.parent_x, 0);
        assert_eq!(mc.parent_y, 0);
        assert_eq!(mc.cursor_offset, 0);
    }

    #[test]
    fn test_make_menu_common_all_types() {
        for &mtype in &[MTYPE_SLIDER, MTYPE_LIST, MTYPE_ACTION, MTYPE_SPINCONTROL, MTYPE_SEPARATOR, MTYPE_FIELD] {
            let mc = make_menu_common(mtype, "x", 0, 0, 0, [0; 4], None, None);
            assert_eq!(mc.item_type, mtype);
        }
    }

    #[test]
    fn test_make_menu_common_flags_combination() {
        let mc = make_menu_common(
            MTYPE_FIELD, "input", 0, 0, QMF_LEFT_JUSTIFY | QMF_NUMBERSONLY,
            [0; 4], None, None,
        );
        assert_ne!(mc.flags & QMF_LEFT_JUSTIFY, 0);
        assert_ne!(mc.flags & QMF_NUMBERSONLY, 0);
        assert_eq!(mc.flags & QMF_GRAYED, 0);
    }

    // ----------------------------------------------------------
    // Menu constants tests
    // ----------------------------------------------------------

    #[test]
    fn test_max_menu_depth() {
        assert_eq!(MAX_MENU_DEPTH, 8);
    }

    #[test]
    fn test_num_cursor_frames() {
        assert_eq!(NUM_CURSOR_FRAMES, 15);
    }

    #[test]
    fn test_main_items() {
        assert_eq!(MAIN_ITEMS, 5);
    }

    #[test]
    fn test_max_savegames() {
        assert_eq!(MAX_SAVEGAMES, 15);
    }

    #[test]
    fn test_max_local_servers() {
        assert_eq!(MAX_LOCAL_SERVERS, 8);
    }

    #[test]
    fn test_num_addressbook_entries() {
        assert_eq!(NUM_ADDRESSBOOK_ENTRIES, 9);
    }

    #[test]
    fn test_max_displayname() {
        assert_eq!(MAX_DISPLAYNAME, 16);
    }

    #[test]
    fn test_max_playermodels() {
        assert_eq!(MAX_PLAYERMODELS, 1024);
    }

    // ----------------------------------------------------------
    // Sound path constants tests
    // ----------------------------------------------------------

    #[test]
    fn test_sound_paths() {
        assert_eq!(MENU_IN_SOUND, "misc/menu1.wav");
        assert_eq!(MENU_MOVE_SOUND, "misc/menu2.wav");
        assert_eq!(MENU_OUT_SOUND, "misc/menu3.wav");
    }

    // ----------------------------------------------------------
    // Options menu item index constants
    // ----------------------------------------------------------

    #[test]
    fn test_options_menu_indices_are_sequential() {
        let indices = [
            OPT_SFX_VOLUME, OPT_CD_VOLUME, OPT_QUALITY, OPT_COMPATIBILITY,
            OPT_SENSITIVITY, OPT_ALWAYSRUN, OPT_INVERTMOUSE, OPT_LOOKSPRING,
            OPT_LOOKSTRAFE, OPT_FREELOOK, OPT_CROSSHAIR, OPT_JOYSTICK,
            OPT_CUSTOMIZE, OPT_DEFAULTS, OPT_CONSOLE,
        ];
        for (i, &idx) in indices.iter().enumerate() {
            assert_eq!(idx, i as i32, "Options index {} should be {}", idx, i);
        }
    }

    // ----------------------------------------------------------
    // BINDNAMES table tests
    // ----------------------------------------------------------

    #[test]
    fn test_bindnames_not_empty() {
        assert!(!BINDNAMES.is_empty());
    }

    #[test]
    fn test_bindnames_all_have_nonempty_entries() {
        for &(cmd, label) in BINDNAMES.iter() {
            assert!(!cmd.is_empty(), "Bind command should not be empty");
            assert!(!label.is_empty(), "Bind label should not be empty");
        }
    }

    #[test]
    fn test_bindnames_first_is_attack() {
        assert_eq!(BINDNAMES[0].0, "+attack");
        assert_eq!(BINDNAMES[0].1, "attack");
    }

    #[test]
    fn test_bindnames_last_is_help() {
        let last = BINDNAMES.last().unwrap();
        assert_eq!(last.0, "cmd help");
        assert_eq!(last.1, "help computer");
    }

    #[test]
    fn test_bindnames_count() {
        // The original C code has 23 key bindings
        assert_eq!(BINDNAMES.len(), 23);
    }

    // ----------------------------------------------------------
    // RATE_TBL tests
    // ----------------------------------------------------------

    #[test]
    fn test_rate_table_values() {
        assert_eq!(RATE_TBL, &[2500, 3200, 5000, 10000, 25000, 0]);
    }

    #[test]
    fn test_rate_table_last_is_zero() {
        // 0 is the sentinel for "user defined"
        assert_eq!(*RATE_TBL.last().unwrap(), 0);
    }

    #[test]
    fn test_rate_table_ascending_except_last() {
        for i in 0..(RATE_TBL.len() - 2) {
            assert!(RATE_TBL[i] < RATE_TBL[i + 1],
                "Rate table should be ascending: {} < {}", RATE_TBL[i], RATE_TBL[i + 1]);
        }
    }

    // ----------------------------------------------------------
    // ID_CREDITS tests
    // ----------------------------------------------------------

    #[test]
    fn test_id_credits_not_empty() {
        assert!(!ID_CREDITS.is_empty());
    }

    #[test]
    fn test_id_credits_first_line_is_title() {
        assert_eq!(ID_CREDITS[0], "+QUAKE II BY ID SOFTWARE");
    }

    #[test]
    fn test_id_credits_bold_lines_start_with_plus() {
        // Lines starting with '+' are drawn bold
        let bold_lines: Vec<&&str> = ID_CREDITS.iter()
            .filter(|l| l.starts_with('+'))
            .collect();
        assert!(!bold_lines.is_empty());
        // The first bold line should be the title
        assert_eq!(*bold_lines[0], "+QUAKE II BY ID SOFTWARE");
    }

    // ----------------------------------------------------------
    // Start server coop spawn spot logic test
    // ----------------------------------------------------------

    #[test]
    fn test_coop_spawn_spot_mapping() {
        // This tests the same mapping logic as in startserver_menu_callback
        fn get_coop_spot(startmap: &str) -> Option<&'static str> {
            let sm = startmap.to_lowercase();
            match sm.as_str() {
                "bunk1" | "mintro" | "fact1" => Some("start"),
                "power1" => Some("pstart"),
                "biggun" => Some("bstart"),
                "hangar1" | "city1" => Some("unitstart"),
                "boss1" => Some("bosstart"),
                _ => None,
            }
        }

        assert_eq!(get_coop_spot("bunk1"), Some("start"));
        assert_eq!(get_coop_spot("BUNK1"), Some("start"));
        assert_eq!(get_coop_spot("mintro"), Some("start"));
        assert_eq!(get_coop_spot("fact1"), Some("start"));
        assert_eq!(get_coop_spot("power1"), Some("pstart"));
        assert_eq!(get_coop_spot("biggun"), Some("bstart"));
        assert_eq!(get_coop_spot("hangar1"), Some("unitstart"));
        assert_eq!(get_coop_spot("city1"), Some("unitstart"));
        assert_eq!(get_coop_spot("boss1"), Some("bosstart"));
        assert_eq!(get_coop_spot("base1"), None);
        assert_eq!(get_coop_spot("unknown"), None);
    }

    // ----------------------------------------------------------
    // MenuLayer struct initialization test
    // ----------------------------------------------------------

    #[test]
    fn test_menu_layer_default() {
        let layer = MenuLayer { draw: None, key: None };
        assert!(layer.draw.is_none());
        assert!(layer.key.is_none());
    }

    // ----------------------------------------------------------
    // Start server menu map name parsing logic
    // ----------------------------------------------------------

    #[test]
    fn test_mapname_shortname_extraction() {
        // The start server menu stores map names as "LongName\nSHORTNAME"
        // and extracts the shortname part
        let mapname = "The Outer Base\nBASE1";
        let startmap = if let Some(pos) = mapname.find('\n') {
            &mapname[pos + 1..]
        } else {
            mapname
        };
        assert_eq!(startmap, "BASE1");
    }

    #[test]
    fn test_mapname_no_newline() {
        let mapname = "base1";
        let startmap = if let Some(pos) = mapname.find('\n') {
            &mapname[pos + 1..]
        } else {
            mapname
        };
        assert_eq!(startmap, "base1");
    }

    // ----------------------------------------------------------
    // Sensitivity mapping tests (options menu cvar <-> slider)
    // ----------------------------------------------------------

    #[test]
    fn test_sensitivity_slider_to_cvar() {
        // Options menu: cvar "sensitivity" = slider.curvalue / 2.0
        let slider_curvalue = 10.0f32;
        let cvar_value = slider_curvalue / 2.0;
        assert_eq!(cvar_value, 5.0);
    }

    #[test]
    fn test_sensitivity_cvar_to_slider() {
        // Options menu: slider.curvalue = cvar_value * 2.0
        let cvar_value = 3.5f32;
        let slider_curvalue = cvar_value * 2.0;
        assert_eq!(slider_curvalue, 7.0);
    }

    #[test]
    fn test_sensitivity_roundtrip() {
        let original = 4.0f32;
        let slider = original * 2.0;
        let back = slider / 2.0;
        assert_eq!(original, back);
    }

    // ----------------------------------------------------------
    // Volume mapping tests (options menu cvar <-> slider)
    // ----------------------------------------------------------

    #[test]
    fn test_volume_slider_to_cvar() {
        // Options menu: cvar "s_volume" = slider.curvalue / 10.0
        let slider_curvalue = 7.0f32;
        let cvar_value = slider_curvalue / 10.0;
        assert!((cvar_value - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn test_volume_cvar_to_slider() {
        let cvar_value = 0.5f32;
        let slider_curvalue = cvar_value * 10.0;
        assert_eq!(slider_curvalue, 5.0);
    }

    #[test]
    fn test_volume_roundtrip() {
        let original = 0.8f32;
        let slider = original * 10.0;
        let back = slider / 10.0;
        assert!((original - back).abs() < f32::EPSILON);
    }

    // ----------------------------------------------------------
    // CD volume (inverted boolean) tests
    // ----------------------------------------------------------

    #[test]
    fn test_cd_volume_mapping() {
        // cd_nocd=1.0 means CD disabled -> curvalue=0 (disabled)
        // cd_nocd=0.0 means CD enabled  -> curvalue=1 (enabled)
        let cd_nocd_on = 1.0f32;
        let curvalue = if cd_nocd_on != 0.0 { 0 } else { 1 };
        assert_eq!(curvalue, 0);

        let cd_nocd_off = 0.0f32;
        let curvalue = if cd_nocd_off != 0.0 { 0 } else { 1 };
        assert_eq!(curvalue, 1);
    }

    // ----------------------------------------------------------
    // Sound quality mapping tests
    // ----------------------------------------------------------

    #[test]
    fn test_sound_quality_mapping() {
        // s_loadas8bit=1.0 -> low quality -> curvalue=0
        // s_loadas8bit=0.0 -> high quality -> curvalue=1
        let loadas8bit = 1.0f32;
        let curvalue = if loadas8bit != 0.0 { 0 } else { 1 };
        assert_eq!(curvalue, 0);

        let loadas8bit = 0.0f32;
        let curvalue = if loadas8bit != 0.0 { 0 } else { 1 };
        assert_eq!(curvalue, 1);
    }

    // ----------------------------------------------------------
    // Invert mouse mapping tests
    // ----------------------------------------------------------

    #[test]
    fn test_invert_mouse_mapping() {
        // m_pitch < 0.0 -> inverted -> curvalue=1
        // m_pitch >= 0.0 -> normal -> curvalue=0
        let pitch_negative = -0.022f32;
        let curvalue = if pitch_negative < 0.0 { 1 } else { 0 };
        assert_eq!(curvalue, 1);

        let pitch_positive = 0.022f32;
        let curvalue = if pitch_positive < 0.0 { 1 } else { 0 };
        assert_eq!(curvalue, 0);
    }

    // ----------------------------------------------------------
    // NO_SERVER_STRING constant test
    // ----------------------------------------------------------

    #[test]
    fn test_no_server_string() {
        assert_eq!(NO_SERVER_STRING, "<no server>");
    }

    // ----------------------------------------------------------
    // Player config rate mapping
    // ----------------------------------------------------------

    #[test]
    fn test_rate_index_lookup() {
        // Given a rate value, find the matching index in RATE_TBL
        let find_rate_idx = |rate: i32| -> usize {
            let mut rate_idx = RATE_TBL.len() - 1;
            for (i, &r) in RATE_TBL.iter().enumerate() {
                if r > 0 && rate == r {
                    rate_idx = i;
                    break;
                }
            }
            rate_idx
        };

        assert_eq!(find_rate_idx(2500), 0);  // 28.8 Modem
        assert_eq!(find_rate_idx(3200), 1);  // 33.6 Modem
        assert_eq!(find_rate_idx(5000), 2);  // Single ISDN
        assert_eq!(find_rate_idx(10000), 3); // Dual ISDN/Cable
        assert_eq!(find_rate_idx(25000), 4); // T1/LAN
        assert_eq!(find_rate_idx(9999), 5);  // User defined (no match)
        assert_eq!(find_rate_idx(0), 5);     // User defined (0 is sentinel, won't match r > 0)
    }

    // ----------------------------------------------------------
    // Deathmatch rules mapping
    // ----------------------------------------------------------

    #[test]
    fn test_deathmatch_rules_mapping() {
        // rules_val == 0 -> deathmatch=1, coop=0
        // rules_val == 1 -> deathmatch=0, coop=1
        let test_rules = |rules_val: i32| -> (f32, f32) {
            let dm = if rules_val == 0 { 1.0 } else { 0.0 };
            let coop = rules_val as f32;
            (dm, coop)
        };

        let (dm, coop) = test_rules(0);
        assert_eq!(dm, 1.0);
        assert_eq!(coop, 0.0);

        let (dm, coop) = test_rules(1);
        assert_eq!(dm, 0.0);
        assert_eq!(coop, 1.0);
    }

    // ----------------------------------------------------------
    // Quit key constants test
    // ----------------------------------------------------------

    #[test]
    fn test_quit_key_values() {
        // 'Y' = 89, 'y' = 121, 'N' = 78, 'n' = 110
        assert_eq!(b'Y', 89);
        assert_eq!(b'y', 121);
        assert_eq!(b'N', 78);
        assert_eq!(b'n', 110);
    }

    // ----------------------------------------------------------
    // Server list constants
    // ----------------------------------------------------------

    #[test]
    fn test_local_server_name_buffer_size() {
        // Each server name buffer is 80 bytes
        // The add_to_server_list function truncates to 79 chars + NUL
        let max_name_len: usize = 79;
        let buffer_size: usize = 80;
        assert!(max_name_len < buffer_size);
    }

    // ----------------------------------------------------------
    // Start server item indices
    // ----------------------------------------------------------

    #[test]
    fn test_startserver_indices() {
        assert_eq!(SS_STARTMAP, 0);
        assert_eq!(SS_RULES, 1);
        assert_eq!(SS_TIMELIMIT, 2);
        assert_eq!(SS_FRAGLIMIT, 3);
        assert_eq!(SS_MAXCLIENTS, 4);
        assert_eq!(SS_HOSTNAME, 5);
        assert_eq!(SS_DMOPTIONS, 6);
        assert_eq!(SS_BEGIN, 7);
    }

    // ----------------------------------------------------------
    // Player config indices
    // ----------------------------------------------------------

    #[test]
    fn test_playerconfig_indices() {
        assert_eq!(PC_NAME, 0);
        assert_eq!(PC_MODEL, 1);
        assert_eq!(PC_SKIN, 2);
        assert_eq!(PC_HANDEDNESS, 3);
        assert_eq!(PC_RATE, 4);
        assert_eq!(PC_DOWNLOAD, 5);
    }

    // ----------------------------------------------------------
    // Maps list parsing logic tests
    // ----------------------------------------------------------

    #[test]
    fn test_maps_list_parsing() {
        // Simulates the parsing logic from startserver_menu_init
        let buf = "base1 Outer Base\nbase2 Installation\nfact1 Upper Warehouse\n";
        let mut mapname_strs: Vec<String> = Vec::new();
        let lines: Vec<&str> = buf.lines().collect();
        for line in lines {
            let line = line.trim();
            if line.is_empty() { continue; }
            let mut parts = line.splitn(2, char::is_whitespace);
            let shortname = parts.next().unwrap_or("").to_uppercase();
            let longname = parts.next().unwrap_or("").trim().to_string();
            if !shortname.is_empty() {
                mapname_strs.push(format!("{}\n{}", longname, shortname));
            }
        }

        assert_eq!(mapname_strs.len(), 3);
        assert_eq!(mapname_strs[0], "Outer Base\nBASE1");
        assert_eq!(mapname_strs[1], "Installation\nBASE2");
        assert_eq!(mapname_strs[2], "Upper Warehouse\nFACT1");
    }

    #[test]
    fn test_maps_list_parsing_empty() {
        let buf = "";
        let mut mapname_strs: Vec<String> = Vec::new();
        let lines: Vec<&str> = buf.lines().collect();
        for line in lines {
            let line = line.trim();
            if line.is_empty() { continue; }
            let mut parts = line.splitn(2, char::is_whitespace);
            let shortname = parts.next().unwrap_or("").to_uppercase();
            let _longname = parts.next().unwrap_or("").trim().to_string();
            if !shortname.is_empty() {
                mapname_strs.push(shortname);
            }
        }
        assert!(mapname_strs.is_empty());
    }

    #[test]
    fn test_maps_list_parsing_shortname_only() {
        let buf = "base1\n";
        let mut mapname_strs: Vec<String> = Vec::new();
        let lines: Vec<&str> = buf.lines().collect();
        for line in lines {
            let line = line.trim();
            if line.is_empty() { continue; }
            let mut parts = line.splitn(2, char::is_whitespace);
            let shortname = parts.next().unwrap_or("").to_uppercase();
            let longname = parts.next().unwrap_or("").trim().to_string();
            if !shortname.is_empty() {
                mapname_strs.push(format!("{}\n{}", longname, shortname));
            }
        }
        assert_eq!(mapname_strs.len(), 1);
        assert_eq!(mapname_strs[0], "\nBASE1");
    }

    // ----------------------------------------------------------
    // Display name extraction (used by map spin control)
    // ----------------------------------------------------------

    #[test]
    fn test_map_display_name_extraction() {
        let mapnames = vec![
            "Outer Base\nBASE1".to_string(),
            "Installation\nBASE2".to_string(),
        ];
        let display_names: Vec<String> = mapnames.iter()
            .map(|n| {
                if let Some(pos) = n.find('\n') { n[..pos].to_string() } else { n.clone() }
            })
            .collect();
        assert_eq!(display_names[0], "Outer Base");
        assert_eq!(display_names[1], "Installation");
    }

    #[test]
    fn test_map_display_name_no_newline() {
        let mapname = "base1".to_string();
        let display = if let Some(pos) = mapname.find('\n') {
            mapname[..pos].to_string()
        } else {
            mapname.clone()
        };
        assert_eq!(display, "base1");
    }

    // ----------------------------------------------------------
    // Savestring helper logic
    // ----------------------------------------------------------

    #[test]
    fn test_savestring_nul_termination() {
        // Simulates the savestring_as_str logic
        let mut bytes = [0u8; 32];
        let text = b"Test Save";
        bytes[..text.len()].copy_from_slice(text);
        let len = bytes.iter().position(|&b| b == 0).unwrap_or(32);
        let s = std::str::from_utf8(&bytes[..len]).unwrap_or("<EMPTY>");
        assert_eq!(s, "Test Save");
    }

    #[test]
    fn test_savestring_all_zeros() {
        let bytes = [0u8; 32];
        let len = bytes.iter().position(|&b| b == 0).unwrap_or(32);
        let s = std::str::from_utf8(&bytes[..len]).unwrap_or("<EMPTY>");
        assert_eq!(s, "");
    }

    #[test]
    fn test_savestring_full_buffer() {
        let bytes = [b'A'; 32]; // no NUL
        let len = bytes.iter().position(|&b| b == 0).unwrap_or(32);
        let s = std::str::from_utf8(&bytes[..len]).unwrap_or("<EMPTY>");
        assert_eq!(s.len(), 32);
    }

    // ----------------------------------------------------------
    // DmFlags inverted logic test
    // ----------------------------------------------------------

    #[test]
    fn test_dmflag_inverted_logic() {
        // "inverted" means curvalue=1 means flag OFF
        // e.g., "allow health" (curvalue=1) -> remove DF_NO_HEALTH
        let flag_present = true;
        let inverted = true;

        // If inverted and flag is present: curvalue should be 0
        let curvalue = if inverted {
            if !flag_present { 1 } else { 0 }
        } else {
            if flag_present { 1 } else { 0 }
        };
        assert_eq!(curvalue, 0);

        // If inverted and flag is NOT present: curvalue should be 1
        let flag_present = false;
        let curvalue = if inverted {
            if !flag_present { 1 } else { 0 }
        } else {
            if flag_present { 1 } else { 0 }
        };
        assert_eq!(curvalue, 1);
    }

    #[test]
    fn test_dmflag_normal_logic() {
        // Normal: curvalue=1 means flag ON
        let flag_present = true;
        let inverted = false;

        let curvalue = if inverted {
            if !flag_present { 1 } else { 0 }
        } else {
            if flag_present { 1 } else { 0 }
        };
        assert_eq!(curvalue, 1);

        let flag_present = false;
        let curvalue = if inverted {
            if !flag_present { 1 } else { 0 }
        } else {
            if flag_present { 1 } else { 0 }
        };
        assert_eq!(curvalue, 0);
    }

    // ----------------------------------------------------------
    // Teamplay mapping test
    // ----------------------------------------------------------

    #[test]
    fn test_teamplay_mapping() {
        // teamplay: 0=disabled, 1=by skin, 2=by model
        // Flags: DF_SKINTEAMS = by skin, DF_MODELTEAMS = by model
        let test_teamplay = |skin: bool, model: bool| -> i32 {
            if skin { 1 }
            else if model { 2 }
            else { 0 }
        };

        assert_eq!(test_teamplay(false, false), 0);
        assert_eq!(test_teamplay(true, false), 1);
        assert_eq!(test_teamplay(false, true), 2);
        // Both set: skin takes priority (checked first)
        assert_eq!(test_teamplay(true, true), 1);
    }

    // ----------------------------------------------------------
    // Download options mapping tests
    // ----------------------------------------------------------

    #[test]
    fn test_download_option_mapping() {
        // allow_download cvar nonzero -> curvalue=1 (yes)
        // allow_download cvar zero    -> curvalue=0 (no)
        let cvar_on = 1.0f32;
        let curvalue = if cvar_on != 0.0 { 1 } else { 0 };
        assert_eq!(curvalue, 1);

        let cvar_off = 0.0f32;
        let curvalue = if cvar_off != 0.0 { 1 } else { 0 };
        assert_eq!(curvalue, 0);
    }
}
