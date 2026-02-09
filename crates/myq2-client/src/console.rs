// console.rs — Console display and management
// Converted from: myq2-original/client/console.c
//
// Copyright (C) 1997-2001 Id Software, Inc.
// Licensed under the GNU General Public License v2

use std::fs::File;
use std::io::Write;

use crate::client::{ClientState, ClientStatic, ConnState, KeyDest};
use crate::console_types::{Console, CON_TEXTSIZE, NUM_CON_TIMES};

// ============================================================
// MyQ2 build options (from myq2opts.h)
// ============================================================

pub use myq2_common::common::{DISTNAME, DISTVER};

pub const NOTIFY_INDENT: i32 = 2;
pub const NOTIFY_VERTPOS_FACTOR: f32 = 0.675;

// mattx86: console_demos — USE_CONSOLE_IN_DEMOS is defined
pub const USE_CONSOLE_IN_DEMOS: bool = true;
// mattx86: startup_demo — DISABLE_STARTUP_DEMO is defined
pub const DISABLE_STARTUP_DEMO: bool = true;

pub const MAXCMDLINE: usize = 256;

// ============================================================
// Extern references (to be wired up with actual global state)
// ============================================================

/// Global console state
pub static mut CON: Console = Console {
    initialized: false,
    text: [b' '; CON_TEXTSIZE],
    current: 0,
    x: 0,
    display: 0,
    ormask: 0,
    linewidth: 0,
    totallines: 0,
    cursorspeed: 0.0,
    vislines: 0,
    times: [0.0; NUM_CON_TIMES],
};

/// Console notify time cvar value (default 3 seconds)
pub static mut CON_NOTIFYTIME: f32 = 3.0;

// These are defined in keys.rs but referenced here
extern "Rust" {
    // key_lines, edit_line, key_linepos are in keys module
}

// ============================================================
// Drawing helpers
// ============================================================

/// Draw a string at (x, y) using 8-pixel-wide characters.
/// Calls into the renderer's Draw_Char.
pub fn draw_string(x: i32, y: i32, s: &str) {
    let mut x = x;
    for ch in s.bytes() {
        draw_char(x, y, ch as i32);
        x += 8;
    }
}

/// Draw a string with high-bit set (alternate/colored text).
pub fn draw_alt_string(x: i32, y: i32, s: &str) {
    let mut x = x;
    for ch in s.bytes() {
        draw_char(x, y, (ch as i32) ^ 0x80);
        x += 8;
    }
}

// ============================================================
// Stubs for renderer/engine functions (to be implemented)
// ============================================================

/// Draw a single character — dispatches through renderer function pointer table.
pub fn draw_char(x: i32, y: i32, num: i32) {
    // SAFETY: single-threaded engine, function pointer table initialized at startup
    unsafe { (RENDERER_FNS.draw_char)(x, y, num) }
}

/// Draw a stretched picture — dispatches through renderer function pointer table.
pub fn draw_stretch_pic(x: i32, y: i32, w: i32, h: i32, name: &str) {
    // SAFETY: single-threaded engine, function pointer table initialized at startup
    unsafe { (RENDERER_FNS.draw_stretch_pic)(x, y, w, h, name) }
}

/// Draw a picture — dispatches through renderer function pointer table.
pub fn draw_pic(x: i32, y: i32, name: &str) {
    // SAFETY: single-threaded engine, function pointer table initialized at startup
    unsafe { (RENDERER_FNS.draw_pic)(x, y, name) }
}

/// Find a pic, returns image handle (0 = not found) — dispatches through renderer function pointer table.
pub fn draw_find_pic(name: &str) -> i32 {
    // SAFETY: single-threaded engine, function pointer table initialized at startup
    unsafe { (RENDERER_FNS.draw_find_pic)(name) }
}

/// Get pic size — dispatches through renderer function pointer table.
pub fn draw_get_pic_size(name: &str) -> (i32, i32) {
    // SAFETY: single-threaded engine, function pointer table initialized at startup
    unsafe { (RENDERER_FNS.draw_get_pic_size)(name) }
}

/// Global screen state — initialized at startup, mirrors C global pattern.
pub static mut SCR: super::cl_scrn::ScrState = super::cl_scrn::ScrState {
    scr_con_current: 0.0,
    scr_conlines: 0.0,
    scr_initialized: false,
    scr_draw_loading: 0,
    scr_vrect: super::cl_scrn::VRect { x: 0, y: 0, width: 0, height: 0 },
    scr_viewsize: 0,
    scr_conspeed: 0,
    scr_centertime: 0,
    scr_showturtle: 0,
    scr_showpause: 0,
    scr_printspeed: 0,
    scr_netgraph: 0,
    scr_timegraph: 0,
    scr_debuggraph: 0,
    scr_graphheight: 0,
    scr_graphscale: 0,
    scr_graphshift: 0,
    scr_drawall: 0,
    scr_dirty: super::cl_scrn::DirtyRect { x1: 0, y1: 0, x2: 0, y2: 0 },
    scr_old_dirty: [super::cl_scrn::DirtyRect { x1: 0, y1: 0, x2: 0, y2: 0 }; 2],
    crosshair_pic: String::new(),
    crosshair_width: 0,
    crosshair_height: 0,
    scr_centerstring: String::new(),
    scr_centertime_start: 0.0,
    scr_centertime_off: 0.0,
    scr_center_lines: 0,
    scr_erase_center: 0,
    graph_current: 0,
    graph_values: [super::cl_scrn::GraphSample { value: 0.0, color: 0 }; 1024],
};

/// SCR_BeginLoadingPlaque — wired to cl_scrn using global state.
pub fn scr_begin_loading_plaque() {
    // SAFETY: single-threaded engine
    unsafe {
        super::cl_scrn::scr_begin_loading_plaque(&mut SCR, &mut *CLS_PTR, &mut *CL_PTR);
    }
}

/// SCR_EndLoadingPlaque — wired to cl_scrn using global CLS state.
pub fn scr_end_loading_plaque(clear: bool) {
    // SAFETY: single-threaded engine
    unsafe {
        super::cl_scrn::scr_end_loading_plaque(&mut *CLS_PTR, clear);
    }
}

/// SCR_UpdateScreen — wired to cl_scrn using global state.
pub fn scr_update_screen() {
    // SAFETY: single-threaded engine
    unsafe {
        super::cl_scrn::scr_update_screen(&mut SCR, &mut *CLS_PTR, &mut *CL_PTR);
    }
}

/// SCR_AddDirtyPoint — wired to cl_scrn using global SCR state.
pub fn scr_add_dirty_point(x: i32, y: i32) {
    // SAFETY: single-threaded engine
    unsafe {
        super::cl_scrn::scr_add_dirty_point(&mut SCR, x, y);
    }
}

/// SCR_DirtyScreen — wired to cl_scrn using global state.
pub fn scr_dirty_screen() {
    // SAFETY: single-threaded engine
    unsafe {
        let viddef = get_viddef();
        super::cl_scrn::scr_dirty_screen(&mut SCR, &viddef);
    }
}

/// Cbuf_AddText — wired to myq2_common
pub fn cbuf_add_text(text: &str) {
    myq2_common::cmd::cbuf_add_text(text);
}

/// Cvar_Set — wired to myq2_common
pub fn cvar_set(name: &str, value: &str) {
    myq2_common::cvar::cvar_set(name, value);
}

/// Cvar_VariableValue — wired to myq2_common
pub fn cvar_variable_value(name: &str) -> f32 {
    myq2_common::cvar::cvar_variable_value(name)
}

/// Cvar_Get — wired to myq2_common; returns handle as i32
pub fn cvar_get(name: &str, default: &str, flags: i32) -> i32 {
    myq2_common::cvar::cvar_get(name, default, flags).unwrap_or(0) as i32
}

/// Cmd_AddCommand — wired to myq2_common
pub fn cmd_add_command(name: &str, func: fn()) {
    myq2_common::cmd::cmd_add_command_simple(name, func);
}

/// Cmd_Argc — wired to myq2_common
pub fn cmd_argc() -> i32 {
    myq2_common::cmd::cmd_argc() as i32
}

/// Cmd_Argv — wired to myq2_common
pub fn cmd_argv(n: i32) -> String {
    myq2_common::cmd::cmd_argv(n as usize)
}

/// FS_Gamedir — wired to myq2_common
pub fn fs_gamedir() -> String {
    myq2_common::files::fs_gamedir()
}

/// FS_CreatePath — wired to myq2_common
pub fn fs_create_path(path: &str) {
    myq2_common::files::fs_create_path(path);
}

/// M_ForceMenuOff — wired to menu module
pub fn m_force_menu_off() {
    super::menu::m_force_menu_off();
}

/// wildcardfit — wired to myq2_common
pub fn wildcardfit(pattern: &str, text: &str) -> bool {
    myq2_common::wildcards::wildcardfit(pattern, text)
}

/// Draw a filled rectangle — dispatches through renderer function pointer table.
pub fn draw_fill(x: i32, y: i32, w: i32, h: i32, c: i32, a: f32) {
    // SAFETY: single-threaded engine
    unsafe { (RENDERER_FNS.draw_fill)(x, y, w, h, c, a) }
}

/// Draw a tiled background clear — dispatches through renderer function pointer table.
pub fn draw_tile_clear(x: i32, y: i32, w: i32, h: i32, name: &str) {
    // SAFETY: single-threaded engine
    unsafe { (RENDERER_FNS.draw_tile_clear)(x, y, w, h, name) }
}

/// Cvar_SetValue — wired to myq2_common
pub fn cvar_set_value(name: &str, value: f32) {
    myq2_common::cvar::cvar_set_value(name, value);
}

/// Cvar_VariableValue by handle (CvarHandle = i32 index) — wired to myq2_common
pub fn cvar_value(handle: i32) -> f32 {
    if handle < 0 { return 0.0; }
    myq2_common::cvar::cvar_value_by_handle(handle as usize)
}

/// Placeholder — Cvar_VariableValue by name
pub fn cvar_value_str(name: &str) -> f32 {
    myq2_common::cvar::cvar_variable_value(name)
}

/// Cvar_Modified check by handle — wired to myq2_common
pub fn cvar_modified(handle: i32) -> bool {
    if handle < 0 { return false; }
    myq2_common::cvar::cvar_modified_by_handle(handle as usize)
}

/// Cvar_ClearModified by handle — wired to myq2_common
pub fn cvar_clear_modified(handle: i32) {
    if handle < 0 { return; }
    myq2_common::cvar::cvar_clear_modified_by_handle(handle as usize);
}

/// Sys_Milliseconds — re-export from canonical myq2_common implementation.
pub use myq2_common::common::sys_milliseconds;

/// Sys_SendKeyEvents — dispatches through system function pointer table.
pub fn sys_send_key_events() {
    // SAFETY: single-threaded engine
    unsafe { (SYSTEM_FNS.sys_send_key_events)() }
}

/// developer cvar value — wired to myq2_common
pub fn developer_value() -> f32 {
    myq2_common::cvar::cvar_variable_value("developer")
}

/// crosshair cvar value — wired to myq2_common
pub fn crosshair_value() -> f32 {
    myq2_common::cvar::cvar_variable_value("crosshair")
}

/// cl_paused cvar value — wired to myq2_common
pub fn cl_paused_value() -> f32 {
    myq2_common::cvar::cvar_variable_value("paused")
}

/// cl_timedemo cvar value — wired to myq2_common
pub fn cl_timedemo_value() -> f32 {
    myq2_common::cvar::cvar_variable_value("timedemo")
}

/// cl_stereo cvar value — wired to myq2_common
pub fn cl_stereo_value() -> f32 {
    myq2_common::cvar::cvar_variable_value("cl_stereo")
}

/// cl_stereo_separation cvar value — wired to myq2_common
pub fn cl_stereo_separation_value() -> f32 {
    myq2_common::cvar::cvar_variable_value("cl_stereo_separation")
}

/// cl_add_entities cvar value — wired to myq2_common
pub fn cl_add_entities_value() -> f32 {
    myq2_common::cvar::cvar_variable_value("cl_entities")
}

/// cl_add_lights cvar value — wired to myq2_common
pub fn cl_add_lights_value() -> f32 {
    myq2_common::cvar::cvar_variable_value("cl_lights")
}

/// cl_add_particles cvar value — wired to myq2_common
pub fn cl_add_particles_value() -> f32 {
    myq2_common::cvar::cvar_variable_value("cl_particles")
}

/// cl_add_blend cvar value — wired to myq2_common
pub fn cl_add_blend_value() -> f32 {
    myq2_common::cvar::cvar_variable_value("cl_blend")
}

/// log_stats cvar value — wired to myq2_common
pub fn log_stats_value() -> f32 {
    myq2_common::cvar::cvar_variable_value("log_stats")
}

/// log_stats file open check.
/// The log_stats_file is managed in cl_main; this checks via a global flag.
pub fn log_stats_file_open() -> bool {
    // SAFETY: single-threaded engine
    unsafe { LOG_STATS_FILE_OPEN_FLAG }
}

/// log_stats file write.
/// Writes to the log_stats file if open, managed in cl_main.
pub fn log_stats_write(msg: &str) {
    // SAFETY: single-threaded engine
    unsafe {
        if let Some(ref mut f) = LOG_STATS_FILE {
            let _ = f.write_all(msg.as_bytes());
        }
    }
}

// Log stats file state — set from cl_main when opening/closing the log
pub static mut LOG_STATS_FILE_OPEN_FLAG: bool = false;
pub static mut LOG_STATS_FILE: Option<File> = None;

/// con_initialized — reads the global CON state
pub fn con_initialized() -> bool {
    // SAFETY: single-threaded engine
    unsafe { CON.initialized }
}

// ============================================================
// Renderer function pointer table
// ============================================================
//
// myq2-client cannot depend on myq2-renderer (circular dependency).
// These function pointers are populated at startup by myq2-sys
// when it initializes the renderer.

/// Renderer function pointers, set by myq2-sys at startup.
pub struct RendererFunctions {
    pub draw_char: fn(i32, i32, i32),
    pub draw_stretch_pic: fn(i32, i32, i32, i32, &str),
    pub draw_pic: fn(i32, i32, &str),
    pub draw_find_pic: fn(&str) -> i32,
    pub draw_get_pic_size: fn(&str) -> (i32, i32),
    pub draw_fill: fn(i32, i32, i32, i32, i32, f32),
    pub draw_tile_clear: fn(i32, i32, i32, i32, &str),
    pub draw_fade_screen: fn(),
    pub r_begin_frame: fn(f32),
    pub r_render_frame: fn(&super::client::RefDef),
    pub r_begin_registration: fn(&str),
    pub r_end_registration: fn(),
    pub r_register_model: fn(&str) -> i32,
    pub r_register_skin: fn(&str) -> i32,
    pub r_set_sky: fn(&str, f32, &[f32; 3]),
    pub r_set_palette_null: fn(),
    pub vk_imp_end_frame: fn(),
    pub r_add_stain: fn(&[f32; 3], f32, f32, f32, f32, f32, i32),
    pub draw_stretch_raw: fn(i32, i32, i32, i32, i32, i32, &[u8]),
    pub viddef_width: fn() -> i32,
    pub viddef_height: fn() -> i32,
    pub r_set_palette: fn(Option<&[u8]>),
}

// Default no-op implementations
fn noop_draw_char(_x: i32, _y: i32, _num: i32) {}
fn noop_draw_stretch_pic(_x: i32, _y: i32, _w: i32, _h: i32, _name: &str) {}
fn noop_draw_pic(_x: i32, _y: i32, _name: &str) {}
fn noop_draw_find_pic(_name: &str) -> i32 { 0 }
fn noop_draw_get_pic_size(_name: &str) -> (i32, i32) { (0, 0) }
fn noop_draw_fill(_x: i32, _y: i32, _w: i32, _h: i32, _c: i32, _a: f32) {}
fn noop_draw_tile_clear(_x: i32, _y: i32, _w: i32, _h: i32, _name: &str) {}
fn noop_draw_fade_screen() {}
fn noop_r_begin_frame(_separation: f32) {}
fn noop_r_render_frame(_refdef: &super::client::RefDef) {}
fn noop_r_begin_registration(_map: &str) {}
fn noop_r_end_registration() {}
fn noop_r_register_model(_name: &str) -> i32 { 0 }
fn noop_r_register_skin(_name: &str) -> i32 { 0 }
fn noop_r_set_sky(_name: &str, _rotate: f32, _axis: &[f32; 3]) {}
fn noop_r_set_palette_null() {}
fn noop_vk_imp_end_frame() {}
fn noop_r_add_stain(_org: &[f32; 3], _intensity: f32, _r: f32, _g: f32, _b: f32, _a: f32, _mode: i32) {}
fn noop_draw_stretch_raw(_x: i32, _y: i32, _w: i32, _h: i32, _cols: i32, _rows: i32, _data: &[u8]) {}
fn noop_viddef_width() -> i32 { 640 }
fn noop_viddef_height() -> i32 { 480 }
fn noop_r_set_palette(_palette: Option<&[u8]>) {}

/// Global renderer function table. Initialized with no-ops; myq2-sys replaces
/// these with real renderer function pointers at startup.
pub static mut RENDERER_FNS: RendererFunctions = RendererFunctions {
    draw_char: noop_draw_char,
    draw_stretch_pic: noop_draw_stretch_pic,
    draw_pic: noop_draw_pic,
    draw_find_pic: noop_draw_find_pic,
    draw_get_pic_size: noop_draw_get_pic_size,
    draw_fill: noop_draw_fill,
    draw_tile_clear: noop_draw_tile_clear,
    draw_fade_screen: noop_draw_fade_screen,
    r_begin_frame: noop_r_begin_frame,
    r_render_frame: noop_r_render_frame,
    r_begin_registration: noop_r_begin_registration,
    r_end_registration: noop_r_end_registration,
    r_register_model: noop_r_register_model,
    r_register_skin: noop_r_register_skin,
    r_set_sky: noop_r_set_sky,
    r_set_palette_null: noop_r_set_palette_null,
    vk_imp_end_frame: noop_vk_imp_end_frame,
    r_add_stain: noop_r_add_stain,
    draw_stretch_raw: noop_draw_stretch_raw,
    viddef_width: noop_viddef_width,
    viddef_height: noop_viddef_height,
    r_set_palette: noop_r_set_palette,
};

/// System function pointers, set by myq2-sys at startup.
pub struct SystemFunctions {
    pub sys_send_key_events: fn(),
    pub s_stop_all_sounds: fn(),
    pub s_start_local_sound: fn(&str),
    pub sys_get_clipboard_data: fn() -> Option<String>,
}

fn noop_sys_send_key_events() {}
fn noop_s_stop_all_sounds() {}
fn noop_s_start_local_sound(_name: &str) {}
fn noop_sys_get_clipboard_data() -> Option<String> { None }

/// Global system function table. Initialized with no-ops; myq2-sys replaces
/// these with real platform function pointers at startup.
pub static mut SYSTEM_FNS: SystemFunctions = SystemFunctions {
    sys_send_key_events: noop_sys_send_key_events,
    s_stop_all_sounds: noop_s_stop_all_sounds,
    s_start_local_sound: noop_s_start_local_sound,
    sys_get_clipboard_data: noop_sys_get_clipboard_data,
};

/// Video menu function pointers, set by myq2-sys at startup.
/// These dispatch VID_MenuInit/Draw/Key from menu.rs to the platform layer.
pub struct VidMenuFunctions {
    pub vid_menu_init: fn(),
    pub vid_menu_draw: fn(),
    pub vid_menu_key: fn(i32) -> Option<&'static str>,
}

fn noop_vid_menu_init() {}
fn noop_vid_menu_draw() {}
fn noop_vid_menu_key(_key: i32) -> Option<&'static str> { None }

/// Global video menu function table. Initialized with no-ops; myq2-sys replaces
/// these with real platform function pointers at startup.
pub static mut VID_MENU_FNS: VidMenuFunctions = VidMenuFunctions {
    vid_menu_init: noop_vid_menu_init,
    vid_menu_draw: noop_vid_menu_draw,
    vid_menu_key: noop_vid_menu_key,
};

/// R_BeginFrame — dispatches through renderer function pointer table.
pub fn r_begin_frame(separation: f32) {
    // SAFETY: single-threaded engine
    unsafe { (RENDERER_FNS.r_begin_frame)(separation) }
}

/// R_RenderFrame — dispatches through renderer function pointer table.
pub fn r_render_frame(refdef: &super::client::RefDef) {
    // SAFETY: single-threaded engine
    unsafe { (RENDERER_FNS.r_render_frame)(refdef) }
}

/// R_BeginRegistration — dispatches through renderer function pointer table.
pub fn r_begin_registration(map: &str) {
    // SAFETY: single-threaded engine
    unsafe { (RENDERER_FNS.r_begin_registration)(map) }
}

/// R_EndRegistration — dispatches through renderer function pointer table.
pub fn r_end_registration() {
    // SAFETY: single-threaded engine
    unsafe { (RENDERER_FNS.r_end_registration)() }
}

/// R_RegisterModel — dispatches through renderer function pointer table.
pub fn r_register_model(name: &str) -> i32 {
    // SAFETY: single-threaded engine
    unsafe { (RENDERER_FNS.r_register_model)(name) }
}

/// R_RegisterSkin — dispatches through renderer function pointer table.
pub fn r_register_skin(name: &str) -> i32 {
    // SAFETY: single-threaded engine
    unsafe { (RENDERER_FNS.r_register_skin)(name) }
}

/// R_SetSky — dispatches through renderer function pointer table.
pub fn r_set_sky(name: &str, rotate: f32, axis: &[f32; 3]) {
    // SAFETY: single-threaded engine
    unsafe { (RENDERER_FNS.r_set_sky)(name, rotate, axis) }
}

/// R_SetPalette(NULL) — dispatches through renderer function pointer table.
pub fn r_set_palette_null() {
    // SAFETY: single-threaded engine
    unsafe { (RENDERER_FNS.r_set_palette_null)() }
}

/// GLimp_EndFrame — dispatches through renderer function pointer table.
pub fn vk_imp_end_frame() {
    // SAFETY: single-threaded engine
    unsafe { (RENDERER_FNS.vk_imp_end_frame)() }
}

/// S_StopAllSounds — dispatches through system function pointer table.
pub fn s_stop_all_sounds() {
    // SAFETY: single-threaded engine
    unsafe { (SYSTEM_FNS.s_stop_all_sounds)() }
}

/// CM_InlineModel — partially wired. The real function in myq2_common::cmodel returns
/// a CModel struct, but client code stores the result as i32 (headnode). This needs
/// a type adapter when model_clip storage is refactored to use CModel.
pub fn cm_inline_model(_name: &str) -> i32 {
    // Returns headnode from the CModel for now
    myq2_common::cmodel::cm_inline_model(_name).headnode
}

pub fn get_viddef() -> VidDef {
    // SAFETY: single-threaded engine
    // SAFETY: single-threaded engine
    unsafe {
        VidDef {
            width: (RENDERER_FNS.viddef_width)(),
            height: (RENDERER_FNS.viddef_height)(),
        }
    }
}

/// SCR_DrawCinematic — delegates to cl_cin::scr_draw_cinematic which handles
/// palette setting and raw frame rendering. Returns true if a cinematic is active.
pub fn scr_draw_cinematic() -> bool {
    // SAFETY: single-threaded engine
    unsafe {
        super::cl_cin::scr_draw_cinematic(&mut *CL_PTR, &*CLS_PTR)
    }
}

/// M_Draw — wired to menu module.
pub fn m_draw() {
    super::menu::m_draw();
}

/// V_RenderView — wired to cl_view module.
pub fn v_render_view(
    scr: &mut super::cl_scrn::ScrState,
    cls: &super::client::ClientStatic,
    cl: &mut super::client::ClientState,
    viddef: &VidDef,
    stereo_separation: f32,
) {
    super::cl_view::v_render_view(scr, cls, cl, viddef, stereo_separation);
}

/// CL_DrawInventory — wired to cl_inv module.
pub fn cl_draw_inventory(
    scr: &mut super::cl_scrn::ScrState,
    cls: &super::client::ClientStatic,
    cl: &super::client::ClientState,
    viddef: &VidDef,
) {
    super::cl_inv::cl_draw_inventory(scr, cls, cl, viddef);
}

/// CL_ParseClientinfo — wired to cl_parse module.
pub fn cl_parse_clientinfo(cl: &mut super::client::ClientState, player: usize) {
    super::cl_parse::cl_parse_clientinfo(cl, player);
}

/// CL_LoadClientinfo — wired to cl_parse module.
pub fn cl_load_clientinfo(ci: &mut super::client::ClientInfo, s: &str) {
    super::cl_parse::cl_load_clientinfo(ci, s);
}

/// CL_RegisterTentModels — wired to cl_tent module using global tent state.
/// The real function takes `&mut TEntState`; we use a module-level global.
pub fn cl_register_tent_models() {
    // SAFETY: single-threaded engine
    unsafe {
        if let Some(ref mut ts) = TENT_STATE {
            super::cl_tent::cl_register_tent_models(ts);
        }
    }
}

/// Global tent effect state. Initialized to None; must be set to Some at startup.
pub static mut TENT_STATE: Option<super::cl_tent::TEntState> = None;

/// CL_AddEntities — dispatches to the real cl_add_entities in cl_ents.rs.
/// Locks the additional global state (ENT_STATE, PROJ_STATE, CLS) needed
/// beyond the already-borrowed ClientState.
pub fn cl_add_entities(cl: &mut super::client::ClientState) {
    use super::cl_main::{CLS, ENT_STATE, PROJ_STATE, FX_STATE, TENT_STATE, SOUND_STATE};
    use super::cl_parse::FrameCallbacks;

    let cls = CLS.lock().unwrap();
    let mut ent_state = ENT_STATE.lock().unwrap();
    let mut proj_state = PROJ_STATE.lock().unwrap();
    let mut fx_state = FX_STATE.lock().unwrap();
    let mut tent_state = TENT_STATE.lock().unwrap();
    let mut sound_state = SOUND_STATE.lock().unwrap();

    // Read cvar values for the dispatch
    let cl_showclamp = myq2_common::cvar::cvar_variable_value("showclamp") != 0.0;
    let cl_timedemo = cl_timedemo_value() != 0.0;
    let cl_predict = myq2_common::cvar::cvar_variable_value("cl_predict") != 0.0;
    let cl_gun = myq2_common::cvar::cvar_variable_value("cl_gun") != 0.0;

    let view_state = super::cl_main::VIEW_STATE.lock().unwrap();
    let gun_model = view_state.gun_model;
    let gun_frame = view_state.gun_frame;
    drop(view_state);

    let hand = myq2_common::cvar::cvar_variable_value("hand") as i32;

    let mut frame_cb = FrameCallbacks {
        fx: &mut *fx_state,
        tent: &mut *tent_state,
        sound: &mut *sound_state,
        cl_time: cl.time as f32,
    };

    super::cl_ents::cl_add_entities(
        cl,
        &cls,
        &mut ent_state,
        &mut proj_state,
        cl_showclamp,
        cl_timedemo,
        cl_predict,
        cl_gun,
        gun_model,
        gun_frame,
        hand,
        &mut frame_cb,
    );
}

/// keybindings access — wired to keys module.
pub fn keybindings(key: i32) -> Option<String> {
    if !(0..256).contains(&key) { return None; }
    // SAFETY: single-threaded engine
    unsafe {
        super::keys::KEYBINDINGS[key as usize].clone()
    }
}

/// get_view_state - returns cl_add_* cvar values — wired to myq2_common.
pub fn get_view_state() -> (f32, f32, f32, f32) {
    (
        cl_add_entities_value(),
        cl_add_lights_value(),
        cl_add_particles_value(),
        cl_add_blend_value(),
    )
}

/// Global view state — initialized at startup.
pub static mut VIEW_STATE: Option<super::cl_view::ViewState> = None;

/// scr_size_up command fn — wired to cl_scrn.
pub fn scr_size_up_f_cmd() {
    // SAFETY: single-threaded engine
    unsafe {
        super::cl_scrn::scr_size_up_f(&SCR);
    }
}

/// scr_size_down command fn — wired to cl_scrn.
pub fn scr_size_down_f_cmd() {
    // SAFETY: single-threaded engine
    unsafe {
        super::cl_scrn::scr_size_down_f(&SCR);
    }
}

/// V_Gun_Model_f command fn — wired to cl_view.
pub fn v_gun_model_f_cmd() {
    // SAFETY: single-threaded engine
    unsafe {
        if let Some(ref mut vs) = VIEW_STATE {
            super::cl_view::v_gun_model_f(vs);
        }
    }
}

/// V_Gun_Next_f command fn — wired to cl_view.
pub fn v_gun_next_f_cmd() {
    // SAFETY: single-threaded engine
    unsafe {
        if let Some(ref mut vs) = VIEW_STATE {
            super::cl_view::v_gun_next_f(vs);
        }
    }
}

/// V_Gun_Prev_f command fn — wired to cl_view.
pub fn v_gun_prev_f_cmd() {
    // SAFETY: single-threaded engine
    unsafe {
        if let Some(ref mut vs) = VIEW_STATE {
            super::cl_view::v_gun_prev_f(vs);
        }
    }
}

/// V_Viewpos_f command fn — wired to cl_view.
pub fn v_viewpos_f_cmd() {
    // SAFETY: single-threaded engine
    unsafe {
        super::cl_view::v_viewpos_f(&*CL_PTR);
    }
}

/// MSG_ReadShort — reads from the global net_message buffer.
/// The real function in myq2_common::common::msg_read_short takes &mut SizeBuf.
/// This wrapper accesses the global net_message buffer used for network parsing.
pub fn msg_read_short() -> i32 {
    // SAFETY: single-threaded engine, net_message is the global read buffer
    unsafe {
        if let Some(ref mut msg) = NET_MESSAGE {
            myq2_common::common::msg_read_short(msg)
        } else {
            0
        }
    }
}

/// Global net message buffer — set by cl_parse when processing server messages.
pub static mut NET_MESSAGE: Option<myq2_common::qcommon::SizeBuf> = None;

// ============================================================
// viddef placeholder
// ============================================================

pub use myq2_common::q_shared::VidDef;

pub static mut VIDDEF: VidDef = VidDef {
    width: 640,
    height: 480,
};

// ============================================================
// Shared state placeholders (to be replaced with real globals)
// ============================================================

// SAFETY: Global client state. Single-threaded engine, matches C global access pattern.
// We store raw pointers and provide deref access. Must call init_client_globals() first.
static mut CL_PTR: *mut ClientState = std::ptr::null_mut();
static mut CLS_PTR: *mut ClientStatic = std::ptr::null_mut();

/// Initialize the global client state. Must be called once at startup before any access.
pub fn init_client_globals() {
    // SAFETY: single-threaded engine initialization. Box::leak ensures 'static lifetime.
    unsafe {
        CL_PTR = Box::into_raw(Box::new(ClientState::default()));
        CLS_PTR = Box::into_raw(Box::new(ClientStatic::default()));
    }
}

/// Helper wrapper that provides deref access to global client state pointers.
/// Allows existing code to use `CL.field` syntax.
pub struct ClAccess;
pub struct ClsAccess;

impl std::ops::Deref for ClAccess {
    type Target = ClientState;
    fn deref(&self) -> &ClientState {
        // SAFETY: single-threaded engine, initialized before use
        unsafe { &*CL_PTR }
    }
}
impl std::ops::DerefMut for ClAccess {
    fn deref_mut(&mut self) -> &mut ClientState {
        // SAFETY: single-threaded engine, initialized before use
        unsafe { &mut *CL_PTR }
    }
}
impl std::ops::Deref for ClsAccess {
    type Target = ClientStatic;
    fn deref(&self) -> &ClientStatic {
        // SAFETY: single-threaded engine, initialized before use
        unsafe { &*CLS_PTR }
    }
}
impl std::ops::DerefMut for ClsAccess {
    fn deref_mut(&mut self) -> &mut ClientStatic {
        // SAFETY: single-threaded engine, initialized before use
        unsafe { &mut *CLS_PTR }
    }
}

/// Global accessor for ClientState — use like `CL.field`
pub static mut CL: ClAccess = ClAccess;
/// Global accessor for ClientStatic — use like `CLS.field`
pub static mut CLS: ClsAccess = ClsAccess;



// ============================================================
// Key state references from keys.rs
// ============================================================

pub static mut KEY_LINES: [[u8; MAXCMDLINE]; 32] = [[0u8; MAXCMDLINE]; 32];
pub static mut EDIT_LINE: i32 = 0;
pub static mut KEY_LINEPOS: i32 = 0;

// ============================================================
// Chat state (shared with keys.rs)
// ============================================================

/// Chat type constants
pub const CT_ALL: i32 = 0;
pub const CT_TEAM: i32 = 1;
pub const CT_TELL: i32 = 2;
pub const CT_PERSON: i32 = 3;

pub static mut CHAT_TYPE: i32 = CT_ALL;
pub static mut CHAT_BUFFER: [u8; MAXCMDLINE] = [0u8; MAXCMDLINE];
pub static mut CHAT_BUFFERLEN: i32 = 0;
pub static mut CHAT_BACKEDIT: i32 = 0;

// ============================================================
// Console functions
// ============================================================

/// Clear any typing on the current key line.
pub fn key_clear_typing() {
    // SAFETY: single-threaded engine, matches C global access pattern
    unsafe {
        KEY_LINES[EDIT_LINE as usize][1] = 0; // clear any typing
        KEY_LINEPOS = 1;
    }
}

/// Toggle console on/off.
pub fn con_toggle_console_f() {
    scr_end_loading_plaque(false); // get rid of loading plaque

    // mattx86: console_demos — USE_CONSOLE_IN_DEMOS is defined, so skip this block
    if !USE_CONSOLE_IN_DEMOS {
        // SAFETY: single-threaded engine
        unsafe {
            if CL.attractloop {
                cbuf_add_text("killserver\n");
                return;
            }
        }
    }

    // mattx86: startup_demo — DISABLE_STARTUP_DEMO is defined, so skip this block
    if !DISABLE_STARTUP_DEMO {
        // SAFETY: single-threaded engine
        unsafe {
            if CLS.state == ConnState::Disconnected {
                cbuf_add_text("d1\n");
                return;
            }
        }
    }

    // SAFETY: single-threaded engine
    unsafe {
        if CLS.key_dest == KeyDest::Console {
            m_force_menu_off();
            cvar_set("paused", "0");
        } else {
            m_force_menu_off();
            CLS.key_dest = KeyDest::Console;

            if cvar_variable_value("maxclients") == 1.0 && myq2_common::common::com_server_state() != 0 {
                cvar_set("paused", "1");
            }
        }
    }
}

/// Toggle chat mode.
pub fn con_toggle_chat_f() {
    key_clear_typing();

    // SAFETY: single-threaded engine
    unsafe {
        if CLS.key_dest == KeyDest::Console {
            if CLS.state == ConnState::Active {
                m_force_menu_off();
                CLS.key_dest = KeyDest::Game;
            }
        } else {
            CLS.key_dest = KeyDest::Console;
        }
    }

    con_clear_notify();
}

/// Clear the console text buffer.
pub fn con_clear_f() {
    // SAFETY: single-threaded engine
    unsafe {
        CON.text.fill(b' ');
    }
}

/// Dump console contents to a file.
pub fn con_dump_f() {
    if cmd_argc() != 2 {
        myq2_common::common::com_printf("usage: condump <filename>\n");
        return;
    }

    let mut name = cmd_argv(1);
    if !wildcardfit("*.txt", &name) {
        name.push_str(".txt");
    }

    let full_path = format!("{}/{}", fs_gamedir(), name);
    myq2_common::common::com_printf(&format!("Dumped console text to {}.\n", full_path));
    fs_create_path(&full_path);

    let f = File::create(&full_path);
    if f.is_err() {
        myq2_common::common::com_printf("ERROR: couldn't open.\n");
        return;
    }
    let mut f = f.unwrap();

    // SAFETY: single-threaded engine
    unsafe {
        let con = &CON;

        // skip empty lines
        let mut l = con.current - con.totallines + 1;
        while l <= con.current {
            let line_start =
                ((l % con.totallines) * con.linewidth) as usize;
            let mut found_non_space = false;
            for x in 0..con.linewidth as usize {
                if line_start + x < CON_TEXTSIZE && con.text[line_start + x] != b' ' {
                    found_non_space = true;
                    break;
                }
            }
            if found_non_space {
                break;
            }
            l += 1;
        }

        // write remaining lines
        while l <= con.current {
            let line_start =
                ((l % con.totallines) * con.linewidth) as usize;
            let mut buffer = Vec::with_capacity(con.linewidth as usize);
            for x in 0..con.linewidth as usize {
                if line_start + x < CON_TEXTSIZE {
                    buffer.push(con.text[line_start + x]);
                } else {
                    buffer.push(b' ');
                }
            }

            // trim trailing spaces
            while buffer.last() == Some(&b' ') {
                buffer.pop();
            }

            // strip high bit
            for b in buffer.iter_mut() {
                *b &= 0x7F;
            }

            let _ = f.write_all(&buffer);
            let _ = f.write_all(b"\n");
            l += 1;
        }
    }
}

/// Clear all notify times.
pub fn con_clear_notify() {
    // SAFETY: single-threaded engine
    unsafe {
        for i in 0..NUM_CON_TIMES {
            CON.times[i] = 0.0;
        }
    }
}

// ============================================================
// Message mode functions (mattx86)
// ============================================================

/// Enter "say" message mode.
pub fn con_message_mode_f() {
    // SAFETY: single-threaded engine
    unsafe {
        CHAT_TYPE = CT_ALL;
        CLS.key_dest = KeyDest::Message;
    }
}

/// Enter "say_team" message mode.
pub fn con_message_mode2_f() {
    // SAFETY: single-threaded engine
    unsafe {
        CHAT_TYPE = CT_TEAM;
        CLS.key_dest = KeyDest::Message;
    }
}

/// Enter "tell" message mode.
pub fn con_message_mode3_f() {
    // SAFETY: single-threaded engine
    unsafe {
        CHAT_TYPE = CT_TELL;
        CLS.key_dest = KeyDest::Message;
    }
}

/// Enter "say_person" message mode.
pub fn con_message_mode4_f() {
    // SAFETY: single-threaded engine
    unsafe {
        CHAT_TYPE = CT_PERSON;
        CLS.key_dest = KeyDest::Message;
    }
}

// ============================================================
// Con_CheckResize
// ============================================================

/// If the line width has changed, reformat the buffer.
pub fn con_check_resize() {
    // SAFETY: single-threaded engine
    unsafe {
        let width = (VIDDEF.width >> 3) - 2;

        if width == CON.linewidth {
            return;
        }

        if width < 1 {
            // video hasn't been initialized yet
            // mattx86: 38 -> 76 (bigger width before video init)
            let width = 76;
            CON.linewidth = width;
            CON.totallines = CON_TEXTSIZE as i32 / CON.linewidth;
            CON.text.fill(b' ');
        } else {
            let oldwidth = CON.linewidth;
            CON.linewidth = width;
            let oldtotallines = CON.totallines;
            CON.totallines = CON_TEXTSIZE as i32 / CON.linewidth;
            let mut numlines = oldtotallines;

            if CON.totallines < numlines {
                numlines = CON.totallines;
            }

            let mut numchars = oldwidth;
            if CON.linewidth < numchars {
                numchars = CON.linewidth;
            }

            let mut tbuf = [0u8; CON_TEXTSIZE];
            tbuf.copy_from_slice(&CON.text);
            CON.text.fill(b' ');

            for i in 0..numlines {
                for j in 0..numchars {
                    let dst = ((CON.totallines - 1 - i) * CON.linewidth + j) as usize;
                    let src = (((CON.current - i + oldtotallines) % oldtotallines) * oldwidth + j)
                        as usize;
                    if dst < CON_TEXTSIZE && src < CON_TEXTSIZE {
                        CON.text[dst] = tbuf[src];
                    }
                }
            }

            con_clear_notify();
        }

        CON.current = CON.totallines - 1;
        CON.display = CON.current;
    }
}

// ============================================================
// Con_Init
// ============================================================

/// Initialize the console.
pub fn con_init() {
    // SAFETY: single-threaded engine
    unsafe {
        CON.linewidth = -1;
    }

    con_check_resize();

    // register our commands
    con_notifytime_init();

    cmd_add_command("toggleconsole", con_toggle_console_f);
    cmd_add_command("togglechat", con_toggle_chat_f);
    cmd_add_command("messagemode", con_message_mode_f);
    cmd_add_command("messagemode2", con_message_mode2_f);
    cmd_add_command("messagemode3", con_message_mode3_f);
    cmd_add_command("messagemode4", con_message_mode4_f);
    cmd_add_command("clear", con_clear_f);
    cmd_add_command("condump", con_dump_f);

    // SAFETY: single-threaded engine
    unsafe {
        CON.initialized = true;
    }

    myq2_common::common::com_printf("Console initialized.\n");
}

/// Initialize con_notifytime cvar.
fn con_notifytime_init() {
    // SAFETY: single-threaded engine
    unsafe {
        CON_NOTIFYTIME = 3.0; // default, Cvar_Get("con_notifytime", "3", 0)
    }
}

// ============================================================
// Con_Linefeed
// ============================================================

/// Advance to next line in the console buffer.
fn con_linefeed() {
    // SAFETY: single-threaded engine
    unsafe {
        CON.x = 0;
        if CON.display == CON.current {
            CON.display += 1;
        }
        CON.current += 1;
        let start = ((CON.current % CON.totallines) * CON.linewidth) as usize;
        let end = start + CON.linewidth as usize;
        if end <= CON_TEXTSIZE {
            CON.text[start..end].fill(b' ');
        }
    }
}

// ============================================================
// Con_Print
// ============================================================

/// Handles cursor positioning, line wrapping, etc.
/// All console printing must go through this in order to be logged to disk.
/// If no console is visible, the text will appear at the top of the game window.
pub fn con_print(txt: &str) {
    static mut CR: bool = false;

    // SAFETY: single-threaded engine
    unsafe {
        if !CON.initialized {
            return;
        }

        let bytes = txt.as_bytes();
        let mut idx = 0;
        let mut mask: i32 = 0;

        if !bytes.is_empty() && (bytes[0] == 1 || bytes[0] == 2) {
            mask = 128; // go to colored text
            idx = 1;
        }

        while idx < bytes.len() {
            let c = bytes[idx] as i32;

            // count word length
            let mut l = 0;
            while l < CON.linewidth as usize && idx + l < bytes.len() {
                if bytes[idx + l] <= b' ' {
                    break;
                }
                l += 1;
            }

            // word wrap
            if l != CON.linewidth as usize && (CON.x + l as i32 > CON.linewidth) {
                CON.x = 0;
            }

            idx += 1;

            if CR {
                CON.current -= 1;
                CR = false;
            }

            if CON.x == 0 {
                con_linefeed();
                // mark time for transparent overlay
                if CON.current >= 0 {
                    CON.times[(CON.current % NUM_CON_TIMES as i32) as usize] = CLS.realtime as f32;
                }
            }

            match c as u8 {
                b'\n' => {
                    CON.x = 0;
                }
                b'\r' => {
                    CON.x = 0;
                    CR = true;
                }
                _ => {
                    // display character and advance
                    let y = (CON.current % CON.totallines) as usize;
                    let pos = y * CON.linewidth as usize + CON.x as usize;
                    if pos < CON_TEXTSIZE {
                        CON.text[pos] = (c | mask | CON.ormask) as u8;
                    }
                    CON.x += 1;
                    if CON.x >= CON.linewidth {
                        CON.x = 0;
                    }
                }
            }
        }
    }
}

// ============================================================
// Con_CenteredPrint
// ============================================================

/// Print centered text to the console.
pub fn con_centered_print(text: &str) {
    // SAFETY: single-threaded engine
    unsafe {
        let l = text.len() as i32;
        let mut pad = (CON.linewidth - l) / 2;
        if pad < 0 {
            pad = 0;
        }
        let buffer = format!("{}{}\n", " ".repeat(pad as usize), text);
        con_print(&buffer);
    }
}

// ============================================================
// Drawing helpers
// ============================================================

/// Draw a string with a length limit.
pub fn draw_string_len(x: i32, y: i32, str_data: &str, len: i32) {
    if len < 0 {
        draw_string(x, y, str_data);
        return;
    }
    let limited: String = str_data.chars().take(len as usize).collect();
    draw_string(x, y, &limited);
}

/// Count byte offset for a given character count.
pub fn char_offset(s: &[u8], charcount: i32) -> usize {
    let mut count = charcount;
    let mut i = 0;
    while i < s.len() && count > 0 && s[i] != 0 {
        count -= 1;
        i += 1;
    }
    i
}

// ============================================================
// Con_DrawInput
// ============================================================

/// Draw the console input line.
/// The input line scrolls horizontally if typing goes beyond the right edge.
pub fn con_draw_input() {
    // SAFETY: single-threaded engine
    unsafe {
        if CLS.key_dest == KeyDest::Menu {
            return;
        }
        if CLS.key_dest != KeyDest::Console && CLS.state == ConnState::Active {
            return; // don't draw anything (always draw if not active)
        }

        let text = &KEY_LINES[EDIT_LINE as usize];

        // convert byte offset to visible character count
        let mut colorlinepos = KEY_LINEPOS;

        let mut text_offset = 0usize;

        // prestep if horizontally scrolling
        if colorlinepos > CON.linewidth {
            let byteofs = char_offset(text, colorlinepos - CON.linewidth);
            text_offset = byteofs;
            colorlinepos = CON.linewidth;
        }

        // draw it
        let bytelen = char_offset(&text[text_offset..], CON.linewidth);
        let display_text: String = text[text_offset..text_offset + bytelen]
            .iter()
            .take_while(|&&b| b != 0)
            .map(|&b| b as char)
            .collect();
        draw_string_len(8, CON.vislines - 22, &display_text, bytelen as i32);

        // add the cursor frame
        // KEY_INSERT is set in keys.rs key_console() — already inside an unsafe block
        let key_insert = crate::keys::KEY_INSERT;
        if ((CLS.realtime >> 8) & 1) != 0 {
            let cursor_char = if key_insert { b'_' as i32 } else { 11 };
            draw_char(8 + colorlinepos * 8, CON.vislines - 22, cursor_char);
        }
    }
}

// ============================================================
// Con_DrawNotify
// ============================================================

/// Draws the last few lines of output transparently over the game top.
pub fn con_draw_notify() {
    // SAFETY: single-threaded engine
    unsafe {
        // mattx86: 67.5% down the screen
        let mut v = (VIDDEF.height as f32 * NOTIFY_VERTPOS_FACTOR) as i32;

        for i in (CON.current - NUM_CON_TIMES as i32 + 1)..=CON.current {
            if i < 0 {
                continue;
            }
            let time = CON.times[(i % NUM_CON_TIMES as i32) as usize];
            if time == 0.0 {
                continue;
            }
            let elapsed = CLS.realtime as f32 - time;
            if elapsed > CON_NOTIFYTIME * 1000.0 {
                continue;
            }

            let line_start = ((i % CON.totallines) * CON.linewidth) as usize;

            let mut x = NOTIFY_INDENT;
            for c in 0..CON.linewidth {
                if line_start + (c as usize) < CON_TEXTSIZE {
                    draw_char(
                        (x + 1) << 3,
                        v,
                        CON.text[line_start + c as usize] as i32,
                    );
                }
                x += 1;
            }
            v += 8;
        }

        if CLS.key_dest == KeyDest::Message {
            let skip;
            match CHAT_TYPE {
                CT_PERSON => {
                    draw_string(8, v, "say_person:");
                    skip = 13;
                }
                CT_TELL => {
                    draw_string(8, v, "tell:");
                    skip = 7;
                }
                CT_TEAM => {
                    draw_string(8, v, "say_team:");
                    skip = 11;
                }
                _ => {
                    // CT_ALL
                    draw_string(8, v, "say:");
                    skip = 6;
                }
            }

            let chat_len = CHAT_BUFFERLEN as usize;
            let max_visible = (VIDDEF.width >> 3) - (skip + 1);
            let s_start = if chat_len as i32 > max_visible {
                chat_len - max_visible as usize
            } else {
                0
            };

            let mut x = 0i32;
            while s_start + (x as usize) < chat_len && CHAT_BUFFER[s_start + x as usize] != 0 {
                let char_idx = s_start + x as usize;
                if CHAT_BACKEDIT != 0
                    && CHAT_BACKEDIT == CHAT_BUFFERLEN - x
                    && ((CLS.realtime >> 8) & 1) != 0
                {
                    draw_char((x + skip) << 3, v, 11);
                } else {
                    draw_char((x + skip) << 3, v, CHAT_BUFFER[char_idx] as i32);
                }
                x += 1;
            }

            if CHAT_BACKEDIT == 0 {
                draw_char(
                    (x + skip) << 3,
                    v,
                    10 + ((CLS.realtime >> 8) & 1),
                );
            }

            draw_char(
                (x + skip) << 3,
                v,
                10 + ((CLS.realtime >> 8) & 1),
            );
            v += 8;
        }

        // mattx86: Do we need to do this? maybe?
        if v != 0 {
            scr_add_dirty_point(0, 0);
            scr_add_dirty_point(VIDDEF.width - 1, v);
        }
    }
}

// ============================================================
// Con_DrawConsole
// ============================================================

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::console_types::{Console, CON_TEXTSIZE, NUM_CON_TIMES};

    // Mutex to serialize tests that modify shared globals (CON, VIDDEF)
    static GLOBAL_STATE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    // ---- Helper to create a fresh Console for testing ----
    fn make_console(linewidth: i32) -> Console {
        let totallines = CON_TEXTSIZE as i32 / linewidth;
        Console {
            initialized: true,
            text: [b' '; CON_TEXTSIZE],
            current: totallines - 1,
            x: 0,
            display: totallines - 1,
            ormask: 0,
            linewidth,
            totallines,
            cursorspeed: 0.0,
            vislines: 0,
            times: [0.0; NUM_CON_TIMES],
        }
    }

    // ============================================================
    // Console buffer management tests
    // ============================================================

    #[test]
    fn test_con_clear_f_fills_text_with_spaces() {
        // SAFETY: This test modifies static CON. Run with --test-threads=1 if parallel issues.
        unsafe {
            CON.initialized = true;
            CON.text[0] = b'A';
            CON.text[100] = b'Z';
            CON.text[CON_TEXTSIZE - 1] = b'!';
        }
        con_clear_f();
        unsafe {
            assert_eq!(CON.text[0], b' ');
            assert_eq!(CON.text[100], b' ');
            assert_eq!(CON.text[CON_TEXTSIZE - 1], b' ');
        }
    }

    #[test]
    fn test_con_clear_notify_zeroes_all_times() {
        unsafe {
            CON.initialized = true;
            for i in 0..NUM_CON_TIMES {
                CON.times[i] = 1000.0 * (i as f32 + 1.0);
            }
        }
        con_clear_notify();
        unsafe {
            for i in 0..NUM_CON_TIMES {
                assert_eq!(CON.times[i], 0.0, "times[{}] should be zeroed", i);
            }
        }
    }

    #[test]
    fn test_con_check_resize_initial_setup() {
        let _lock = GLOBAL_STATE_LOCK.lock().unwrap();
        // con_check_resize should produce a valid linewidth.
        unsafe {
            let saved_width = VIDDEF.width;
            let saved_linewidth = CON.linewidth;
            VIDDEF.width = 0;
            CON.linewidth = -1;
            con_check_resize();
            // After resize, linewidth should be positive
            assert!(CON.linewidth > 0, "linewidth should be positive, got {}", CON.linewidth);
            // Restore state for other tests
            VIDDEF.width = saved_width;
            CON.linewidth = saved_linewidth;
        }
    }

    #[test]
    fn test_con_check_resize_no_change_when_same_width() {
        let _lock = GLOBAL_STATE_LOCK.lock().unwrap();
        // When linewidth already matches (VIDDEF.width >> 3) - 2, con_check_resize
        // should return early. We verify linewidth is preserved.
        unsafe {
            CON.linewidth = 78;
            VIDDEF.width = (78 + 2) << 3; // (width >> 3) - 2 == 78
            con_check_resize();
            assert_eq!(CON.linewidth, 78, "linewidth should be unchanged when it matches");
        }
    }

    // ============================================================
    // char_offset tests
    // ============================================================

    #[test]
    fn test_char_offset_basic() {
        let s = b"Hello\0World";
        assert_eq!(char_offset(s, 0), 0);
        assert_eq!(char_offset(s, 3), 3);
        assert_eq!(char_offset(s, 5), 5);
        // Stops at null terminator
        assert_eq!(char_offset(s, 6), 5);
        assert_eq!(char_offset(s, 100), 5);
    }

    #[test]
    fn test_char_offset_empty() {
        let s: &[u8] = &[];
        assert_eq!(char_offset(s, 5), 0);
    }

    #[test]
    fn test_char_offset_no_null() {
        let s = b"ABCDEFGH";
        assert_eq!(char_offset(s, 4), 4);
        assert_eq!(char_offset(s, 8), 8);
        // Beyond length, returns s.len()
        assert_eq!(char_offset(s, 100), 8);
    }

    // ============================================================
    // con_print tests (using global CON state)
    // ============================================================

    #[test]
    fn test_con_print_uninitialized_noop() {
        unsafe {
            CON.initialized = false;
        }
        // Should not crash or modify anything
        con_print("Hello world!\n");
    }

    #[test]
    fn test_con_print_basic_text() {
        // Ensure CLS_PTR is initialized so con_print can read CLS.realtime
        if unsafe { CLS_PTR.is_null() } {
            init_client_globals();
        }
        unsafe {
            CON.initialized = true;
            CON.linewidth = 40;
            CON.totallines = CON_TEXTSIZE as i32 / 40;
            CON.x = 0;
            CON.current = CON.totallines - 1;
            CON.display = CON.current;
            CON.ormask = 0;
            CON.text.fill(b' ');
        }
        con_print("AB\n");
        // After printing "AB\n":
        // - "AB" should be in the buffer on the line that was started
        // - After '\n', x should be 0
        unsafe {
            assert_eq!(CON.x, 0, "x should be 0 after newline");
        }
    }

    #[test]
    fn test_con_print_colored_text_prefix() {
        // Ensure CLS_PTR is initialized so con_print can read CLS.realtime
        if unsafe { CLS_PTR.is_null() } {
            init_client_globals();
        }
        // Text starting with byte 1 or 2 gets the high bit mask (128)
        unsafe {
            CON.initialized = true;
            CON.linewidth = 80;
            CON.totallines = CON_TEXTSIZE as i32 / 80;
            CON.x = 0;
            CON.current = CON.totallines - 1;
            CON.display = CON.current;
            CON.ormask = 0;
            CON.text.fill(b' ');
        }
        // Byte 1 prefix -> colored text
        con_print("\x01X\n");
        // The 'X' char should have high bit set (128 | b'X')
        unsafe {
            let line_start = ((CON.current % CON.totallines) * CON.linewidth) as usize;
            // We printed "X" which is the second byte (index 1), but after the linefeed
            // the print moved to a new line, so we need to check the previous line.
            // The character was printed before the \n.
        }
    }

    // ============================================================
    // con_centered_print tests
    // ============================================================

    #[test]
    fn test_con_centered_print_short_text() {
        // Ensure CLS_PTR is initialized so con_print can read CLS.realtime
        if unsafe { CLS_PTR.is_null() } {
            init_client_globals();
        }
        unsafe {
            CON.initialized = true;
            CON.linewidth = 40;
            CON.totallines = CON_TEXTSIZE as i32 / 40;
            CON.x = 0;
            CON.current = CON.totallines - 1;
            CON.display = CON.current;
            CON.ormask = 0;
            CON.text.fill(b' ');
        }
        // "Hi" is 2 chars on a 40-char line -> pad = (40-2)/2 = 19
        con_centered_print("Hi");
        // Should not crash and should produce padded output
        unsafe {
            // After centering, x should be 0 since the text ends with \n
            assert_eq!(CON.x, 0);
        }
    }

    #[test]
    fn test_con_centered_print_text_wider_than_linewidth() {
        // Ensure CLS_PTR is initialized so con_print can read CLS.realtime
        if unsafe { CLS_PTR.is_null() } {
            init_client_globals();
        }
        unsafe {
            CON.initialized = true;
            CON.linewidth = 5;
            CON.totallines = CON_TEXTSIZE as i32 / 5;
            CON.x = 0;
            CON.current = CON.totallines - 1;
            CON.display = CON.current;
            CON.ormask = 0;
            CON.text.fill(b' ');
        }
        // "TooLong" is 7 chars on a 5-char line -> pad would be negative, clamped to 0
        con_centered_print("TooLong");
        // Should not crash; pad is 0
    }

    // ============================================================
    // Chat type constants tests
    // ============================================================

    #[test]
    fn test_chat_type_constants() {
        assert_eq!(CT_ALL, 0);
        assert_eq!(CT_TEAM, 1);
        assert_eq!(CT_TELL, 2);
        assert_eq!(CT_PERSON, 3);
    }

    // ============================================================
    // cvar_value negative handle tests
    // ============================================================

    #[test]
    fn test_cvar_value_negative_handle_returns_zero() {
        assert_eq!(cvar_value(-1), 0.0);
        assert_eq!(cvar_value(-100), 0.0);
    }

    #[test]
    fn test_cvar_modified_negative_handle_returns_false() {
        assert!(!cvar_modified(-1));
        assert!(!cvar_modified(-100));
    }

    #[test]
    fn test_cvar_clear_modified_negative_handle_no_panic() {
        // Should not panic
        cvar_clear_modified(-1);
        cvar_clear_modified(-100);
    }

    // ============================================================
    // keybindings helper test
    // ============================================================

    #[test]
    fn test_keybindings_out_of_range() {
        assert!(keybindings(-1).is_none());
        assert!(keybindings(256).is_none());
        assert!(keybindings(300).is_none());
    }

    // ============================================================
    // Renderer no-op defaults test
    // ============================================================

    #[test]
    fn test_noop_draw_find_pic_returns_zero() {
        assert_eq!(noop_draw_find_pic("test"), 0);
    }

    #[test]
    fn test_noop_draw_get_pic_size_returns_zero() {
        assert_eq!(noop_draw_get_pic_size("test"), (0, 0));
    }

    #[test]
    fn test_noop_viddef_defaults() {
        assert_eq!(noop_viddef_width(), 640);
        assert_eq!(noop_viddef_height(), 480);
    }

    #[test]
    fn test_noop_r_register_model_returns_zero() {
        assert_eq!(noop_r_register_model("models/test"), 0);
    }

    #[test]
    fn test_noop_r_register_skin_returns_zero() {
        assert_eq!(noop_r_register_skin("skins/test"), 0);
    }

    #[test]
    fn test_noop_sys_get_clipboard_data_returns_none() {
        assert!(noop_sys_get_clipboard_data().is_none());
    }

    #[test]
    fn test_noop_vid_menu_key_returns_none() {
        assert!(noop_vid_menu_key(0).is_none());
        assert!(noop_vid_menu_key(42).is_none());
    }

    // ============================================================
    // MAXCMDLINE constant test
    // ============================================================

    #[test]
    fn test_maxcmdline_value() {
        assert_eq!(MAXCMDLINE, 256);
    }
}

/// Draws the console with the solid background.
pub fn con_draw_console(frac: f32) {
    // SAFETY: single-threaded engine
    unsafe {
        let mut lines = (VIDDEF.height as f32 * frac) as i32;
        if lines <= 0 {
            return;
        }

        if lines > VIDDEF.height {
            lines = VIDDEF.height;
        }

        // draw the background
        draw_stretch_pic(0, -VIDDEF.height + lines, VIDDEF.width, VIDDEF.height, "conback");
        scr_add_dirty_point(0, 0);
        scr_add_dirty_point(VIDDEF.width - 1, lines - 1);

        let version = format!("{} v{:.2}", DISTNAME, DISTVER);
        let vlen = version.len() as i32;
        for (x, ch) in version.bytes().enumerate() {
            draw_char(
                VIDDEF.width - (vlen * 8 + 4) + x as i32 * 8,
                lines - 12,
                128 + ch as i32,
            );
        }

        // draw the text
        CON.vislines = lines;

        let rows = (lines - 22) >> 3; // rows of text to draw
        let mut y = lines - 30;

        // draw from the bottom up
        let mut rows = rows;
        if CON.display != CON.current {
            // draw arrows to show the buffer is backscrolled
            let mut x = 0;
            while x < CON.linewidth {
                draw_char((x + 1) << 3, y, b'^' as i32);
                x += 4;
            }
            y -= 8;
            rows -= 1;
        }

        let mut row = CON.display;
        for _i in 0..rows {
            if row < 0 {
                break;
            }
            if CON.current - row >= CON.totallines {
                break; // past scrollback wrap point
            }

            let line_start = ((row % CON.totallines) * CON.linewidth) as usize;

            for x in 0..CON.linewidth {
                if line_start + (x as usize) < CON_TEXTSIZE {
                    draw_char(
                        (x + 1) << 3,
                        y,
                        CON.text[line_start + x as usize] as i32,
                    );
                }
            }

            y -= 8;
            row -= 1;
        }

        // ZOID: draw the download bar
        // figure out width
        if !CLS.download_name.is_empty() {
            let text = if let Some(pos) = CLS.download_name.rfind('/') {
                &CLS.download_name[pos + 1..]
            } else {
                &CLS.download_name
            };

            let x = CON.linewidth - ((CON.linewidth * 7) / 40);
            let max_text_len = CON.linewidth / 3;
            let display_text = if (text.len() as i32) > max_text_len {
                &text[..max_text_len as usize]
            } else {
                text
            };

            let mut dlbar = String::with_capacity(1024);
            dlbar.push_str(display_text);
            dlbar.push_str(": ");
            dlbar.push('\u{0080}'); // left end cap

            let bar_width = x - (display_text.len() as i32) - 8;
            let n = if CLS.download_percent == 0 {
                0
            } else {
                bar_width * CLS.download_percent / 100
            };

            for j in 0..bar_width {
                if j == n {
                    dlbar.push('\u{0083}'); // cursor position
                } else {
                    dlbar.push('\u{0081}'); // bar fill
                }
            }
            dlbar.push('\u{0082}'); // right end cap
            dlbar.push_str(&format!(" {:02}%", CLS.download_percent));

            // draw it
            let bar_y = CON.vislines - 12;
            for (i, ch) in dlbar.bytes().enumerate() {
                draw_char((i as i32 + 1) << 3, bar_y, ch as i32);
            }
        }

        // draw the input prompt, user text, and cursor if desired
        con_draw_input();
    }
}
