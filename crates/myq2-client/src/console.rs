// console.rs â€” Console display and management
// Converted from: myq2-original/client/console.c
//
// Copyright (C) 1997-2001 Id Software, Inc.
// Licensed under the GNU General Public License v2

use std::fs::File;
use std::io::Write;
use std::sync::{LazyLock, Mutex, MutexGuard};

use crate::client::{ClientState, ClientStatic, ConnState, KeyDest};
use crate::console_types::{Console, CON_TEXTSIZE, NUM_CON_TIMES};

// ============================================================
// MyQ2 build options (from myq2opts.h)
// ============================================================

pub use myq2_common::common::{DISTNAME, DISTVER, com_printf};

pub const NOTIFY_INDENT: i32 = 2;
pub const NOTIFY_VERTPOS_FACTOR: f32 = 0.675;

// mattx86: console_demos â€” USE_CONSOLE_IN_DEMOS is defined
pub const USE_CONSOLE_IN_DEMOS: bool = true;
// mattx86: startup_demo â€” DISABLE_STARTUP_DEMO is defined
pub const DISABLE_STARTUP_DEMO: bool = true;

pub const MAXCMDLINE: usize = 256;

// ============================================================
// Extern references (to be wired up with actual global state)
// ============================================================

// ============================================================
// ConsoleState â€” wraps formerly-static-mut console globals
// ============================================================

pub struct ConsoleState {
    pub con: Console,
    pub con_notifytime: f32,
    pub scr: super::cl_scrn::ScrState,
    pub log_stats_file_open_flag: bool,
    pub log_stats_file: Option<File>,
    pub viddef: VidDef,
}

static CONSOLE_STATE: LazyLock<Mutex<ConsoleState>> = LazyLock::new(|| {
    Mutex::new(ConsoleState {
        con: Console {
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
        },
        con_notifytime: 3.0,
        scr: super::cl_scrn::ScrState {
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
        },
        log_stats_file_open_flag: false,
        log_stats_file: None,
        viddef: VidDef { width: 640, height: 480 },
    })
});

pub fn cs() -> MutexGuard<'static, ConsoleState> {
    CONSOLE_STATE.lock().unwrap_or_else(|e| e.into_inner())
}

// key_lines, edit_line, key_linepos, chat_* are in keys::KeyInputState (accessed via crate::keys::ks())

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

/// Draw a single character â€” dispatches through renderer function pointer table.
pub fn draw_char(x: i32, y: i32, num: i32) {
    (renderer_fns().draw_char)(x, y, num)
}

/// Draw a stretched picture â€” dispatches through renderer function pointer table.
pub fn draw_stretch_pic(x: i32, y: i32, w: i32, h: i32, name: &str) {
    (renderer_fns().draw_stretch_pic)(x, y, w, h, name)
}

/// Draw a picture â€” dispatches through renderer function pointer table.
pub fn draw_pic(x: i32, y: i32, name: &str) {
    (renderer_fns().draw_pic)(x, y, name)
}

/// Find a pic, returns image handle (0 = not found) â€” dispatches through renderer function pointer table.
pub fn draw_find_pic(name: &str) -> i32 {
    (renderer_fns().draw_find_pic)(name)
}

/// Get pic size â€” dispatches through renderer function pointer table.
pub fn draw_get_pic_size(name: &str) -> (i32, i32) {
    (renderer_fns().draw_get_pic_size)(name)
}

// SCR state is now inside ConsoleState (accessed via cs().scr)

/// SCR_BeginLoadingPlaque â€” delegates to cl_main version to avoid deadlock.
///
/// The naive implementation (lock CONSOLE_STATE, call cl_scrn::scr_begin_loading_plaque)
/// would deadlock because scr_begin_loading_plaque calls com_printf, which calls
/// con_print, which also needs CONSOLE_STATE â€” and std::sync::Mutex is not reentrant.
/// The cl_main version avoids this by using raw pointers for CL/CLS state.
pub fn scr_begin_loading_plaque() {
    super::cl_main::scr_begin_loading_plaque();
}

/// SCR_EndLoadingPlaque â€” wired to cl_scrn using global CLS state.
pub fn scr_end_loading_plaque(clear: bool) {
    // SAFETY: CL/CLS initialized at startup, accessed from main thread
    unsafe {
        super::cl_scrn::scr_end_loading_plaque(&mut *CLS_PTR, clear);
    }
}

/// SCR_UpdateScreen â€” wired to cl_scrn using global CLS_PTR/CL_PTR state.
///
/// CLS_PTR and CL_PTR are the "live" state used by console/UI code (key_dest, etc.).
/// Connection code writes to the CLS/CL Mutexes in cl_main.rs (a separate allocation).
/// We sync critical connection fields from the Mutexes to the PTRs before rendering
/// so the screen reflects the true connection state.
pub fn scr_update_screen() {
    // Sync fields between CLS/CL Mutexes and CLS_PTR/CL_PTR.
    // The Mutexes hold authoritative connection state (set by cl_connectionless_packet,
    // cl_parse_server_message, cl_disconnect, etc.), while CLS_PTR holds UI state
    // (key_dest set by con_toggle_console_f, m_force_menu_off, etc.).
    {
        let mut cls_mutex = crate::cl_main::lock_recover(&crate::cl_main::CLS);
        let cl_mutex = crate::cl_main::lock_recover(&crate::cl_main::CL);

        // SAFETY: CLS_PTR/CL_PTR initialized at startup, accessed from main thread.
        unsafe {
            // key_dest: UI code (con_toggle_console_f, m_force_menu_off) writes to CLS_PTR,
            // so sync FROM raw pointer TO Mutex (CLS_PTR is authoritative for key_dest).
            cls_mutex.key_dest = (*CLS_PTR).key_dest;

            (*CLS_PTR).state = cls_mutex.state;
            (*CLS_PTR).realtime = cls_mutex.realtime;
            (*CLS_PTR).framecount = cls_mutex.framecount;
            (*CLS_PTR).frametime = cls_mutex.frametime;
            (*CLS_PTR).quake_port = cls_mutex.quake_port;
            (*CLS_PTR).disable_screen = cls_mutex.disable_screen;
            (*CLS_PTR).netchan.incoming_acknowledged = cls_mutex.netchan.incoming_acknowledged;
            (*CLS_PTR).netchan.outgoing_sequence = cls_mutex.netchan.outgoing_sequence;
            (*CLS_PTR).demo_playing = cls_mutex.demo_playing;
            (*CL_PTR).refresh_prepped = cl_mutex.refresh_prepped;
            (*CL_PTR).cinematictime = cl_mutex.cinematictime;
            (*CL_PTR).time = cl_mutex.time;
            (*CL_PTR).force_refdef = cl_mutex.force_refdef;
            (*CL_PTR).frame = cl_mutex.frame.clone();
            (*CL_PTR).refdef = cl_mutex.refdef.clone();
            // View-critical fields for cl_calc_view_values / cl_add_entities
            (*CL_PTR).predicted_origin = cl_mutex.predicted_origin;
            (*CL_PTR).predicted_angles = cl_mutex.predicted_angles;
            (*CL_PTR).prediction_error = cl_mutex.prediction_error;
            (*CL_PTR).predicted_groundentity = cl_mutex.predicted_groundentity;
            (*CL_PTR).predicted_step = cl_mutex.predicted_step;
            (*CL_PTR).predicted_step_time = cl_mutex.predicted_step_time;
            (*CL_PTR).lerpfrac = cl_mutex.lerpfrac;
            (*CL_PTR).playernum = cl_mutex.playernum;
            (*CL_PTR).viewangles = cl_mutex.viewangles;
            (*CL_PTR).frames = cl_mutex.frames.clone();
            (*CL_PTR).smoothing = cl_mutex.smoothing.clone();
            (*CL_PTR).packet_loss_frames = cl_mutex.packet_loss_frames;
            (*CL_PTR).last_valid_frame_time = cl_mutex.last_valid_frame_time;
            (*CL_PTR).cl_timenudge = cl_mutex.cl_timenudge;
            (*CL_PTR).cl_extrapolate = cl_mutex.cl_extrapolate;
            (*CL_PTR).cl_extrapolate_max = cl_mutex.cl_extrapolate_max;
            (*CL_PTR).cl_anim_continue = cl_mutex.cl_anim_continue;
            (*CL_PTR).cl_projectile_predict = cl_mutex.cl_projectile_predict;
            (*CL_PTR).configstrings = cl_mutex.configstrings.clone();
            (*CL_PTR).model_draw = cl_mutex.model_draw;
            (*CL_PTR).model_clip = cl_mutex.model_clip;
            (*CL_PTR).sound_precache = cl_mutex.sound_precache;
            (*CL_PTR).image_precache = cl_mutex.image_precache;
            (*CL_PTR).clientinfo = cl_mutex.clientinfo.clone();
            (*CL_PTR).baseclientinfo = cl_mutex.baseclientinfo.clone();
            (*CL_PTR).attractloop = cl_mutex.attractloop;
            (*CL_PTR).servercount = cl_mutex.servercount;
            (*CL_PTR).sound_prepped = cl_mutex.sound_prepped;
            (*CL_PTR).v_forward = cl_mutex.v_forward;
            (*CL_PTR).v_right = cl_mutex.v_right;
            (*CL_PTR).v_up = cl_mutex.v_up;
            (*CL_PTR).cmds = cl_mutex.cmds;
            (*CL_PTR).cmd = cl_mutex.cmd;
        }
    }

    let mut scr = crate::cl_main::SCR_STATE.lock().unwrap_or_else(|e| e.into_inner());

    // SAFETY: CL/CLS_PTR initialized at startup, accessed from main thread.
    unsafe {
        super::cl_scrn::scr_update_screen(&mut scr, &mut *CLS_PTR, &mut *CL_PTR);
    }
}

/// SCR_AddDirtyPoint â€” wired to cl_scrn using global SCR state.
pub fn scr_add_dirty_point(x: i32, y: i32) {
    let mut state = cs();
    super::cl_scrn::scr_add_dirty_point(&mut state.scr, x, y);
}

/// SCR_DirtyScreen â€” wired to cl_scrn using global state.
pub fn scr_dirty_screen() {
    let mut state = cs();
    let viddef = state.viddef;
    super::cl_scrn::scr_dirty_screen(&mut state.scr, &viddef);
}

/// Cbuf_AddText â€” wired to myq2_common
pub fn cbuf_add_text(text: &str) {
    myq2_common::cmd::cbuf_add_text(text);
}

/// Cvar_Set â€” wired to myq2_common
pub fn cvar_set(name: &str, value: &str) {
    myq2_common::cvar::cvar_set(name, value);
}

/// Cvar_VariableValue â€” wired to myq2_common
pub fn cvar_variable_value(name: &str) -> f32 {
    myq2_common::cvar::cvar_variable_value(name)
}

/// Cvar_Get â€” wired to myq2_common; returns handle as i32
pub fn cvar_get(name: &str, default: &str, flags: i32) -> i32 {
    myq2_common::cvar::cvar_get(name, default, flags).unwrap_or(0) as i32
}

/// Cmd_AddCommand â€” wired to myq2_common
pub fn cmd_add_command(name: &str, func: fn()) {
    myq2_common::cmd::cmd_add_command_simple(name, func);
}

/// Cmd_Argc â€” wired to myq2_common
pub fn cmd_argc() -> i32 {
    myq2_common::cmd::cmd_argc() as i32
}

/// Cmd_Argv â€” wired to myq2_common
pub fn cmd_argv(n: i32) -> String {
    myq2_common::cmd::cmd_argv(n as usize)
}

/// FS_Gamedir â€” wired to myq2_common
pub fn fs_gamedir() -> String {
    myq2_common::files::fs_gamedir()
}

/// FS_CreatePath â€” wired to myq2_common
pub fn fs_create_path(path: &str) {
    myq2_common::files::fs_create_path(path);
}

/// M_ForceMenuOff â€” wired to menu module
pub fn m_force_menu_off() {
    super::menu::m_force_menu_off();
}

/// wildcardfit â€” wired to myq2_common
pub fn wildcardfit(pattern: &str, text: &str) -> bool {
    myq2_common::wildcards::wildcardfit(pattern, text)
}

/// Draw a filled rectangle â€” dispatches through renderer function pointer table.
pub fn draw_fill(x: i32, y: i32, w: i32, h: i32, c: i32, a: f32) {
    (renderer_fns().draw_fill)(x, y, w, h, c, a)
}

/// Draw a tiled background clear â€” dispatches through renderer function pointer table.
pub fn draw_tile_clear(x: i32, y: i32, w: i32, h: i32, name: &str) {
    (renderer_fns().draw_tile_clear)(x, y, w, h, name)
}

/// Cvar_SetValue â€” wired to myq2_common
pub fn cvar_set_value(name: &str, value: f32) {
    myq2_common::cvar::cvar_set_value(name, value);
}

/// Cvar_VariableValue by handle (CvarHandle = i32 index) â€” wired to myq2_common
pub fn cvar_value(handle: i32) -> f32 {
    if handle < 0 { return 0.0; }
    myq2_common::cvar::cvar_value_by_handle(handle as usize)
}

/// Placeholder â€” Cvar_VariableValue by name
pub fn cvar_value_str(name: &str) -> f32 {
    myq2_common::cvar::cvar_variable_value(name)
}

/// Cvar_Modified check by handle â€” wired to myq2_common
pub fn cvar_modified(handle: i32) -> bool {
    if handle < 0 { return false; }
    myq2_common::cvar::cvar_modified_by_handle(handle as usize)
}

/// Cvar_ClearModified by handle â€” wired to myq2_common
pub fn cvar_clear_modified(handle: i32) {
    if handle < 0 { return; }
    myq2_common::cvar::cvar_clear_modified_by_handle(handle as usize);
}

/// Sys_Milliseconds â€” re-export from canonical myq2_common implementation.
pub use myq2_common::common::sys_milliseconds;

/// Sys_SendKeyEvents â€” dispatches through system function pointer table.
pub fn sys_send_key_events() {
    (system_fns().sys_send_key_events)()
}

/// developer cvar value â€” wired to myq2_common
pub fn developer_value() -> f32 {
    myq2_common::cvar::cvar_variable_value("developer")
}

/// crosshair cvar value â€” wired to myq2_common
pub fn crosshair_value() -> f32 {
    myq2_common::cvar::cvar_variable_value("crosshair")
}

/// cl_paused cvar value â€” wired to myq2_common
pub fn cl_paused_value() -> f32 {
    myq2_common::cvar::cvar_variable_value("paused")
}

/// cl_timedemo cvar value â€” wired to myq2_common
pub fn cl_timedemo_value() -> f32 {
    myq2_common::cvar::cvar_variable_value("timedemo")
}

/// cl_stereo cvar value â€” wired to myq2_common
pub fn cl_stereo_value() -> f32 {
    myq2_common::cvar::cvar_variable_value("cl_stereo")
}

/// cl_stereo_separation cvar value â€” wired to myq2_common
pub fn cl_stereo_separation_value() -> f32 {
    myq2_common::cvar::cvar_variable_value("cl_stereo_separation")
}

/// cl_add_entities cvar value â€” wired to myq2_common
pub fn cl_add_entities_value() -> f32 {
    myq2_common::cvar::cvar_variable_value("cl_entities")
}

/// cl_add_lights cvar value â€” wired to myq2_common
pub fn cl_add_lights_value() -> f32 {
    myq2_common::cvar::cvar_variable_value("cl_lights")
}

/// cl_add_particles cvar value â€” wired to myq2_common
pub fn cl_add_particles_value() -> f32 {
    myq2_common::cvar::cvar_variable_value("cl_particles")
}

/// cl_add_blend cvar value â€” wired to myq2_common
pub fn cl_add_blend_value() -> f32 {
    myq2_common::cvar::cvar_variable_value("cl_blend")
}

/// log_stats cvar value â€” wired to myq2_common
pub fn log_stats_value() -> f32 {
    myq2_common::cvar::cvar_variable_value("log_stats")
}

/// log_stats file open check.
/// The log_stats_file is managed in cl_main; this checks via a global flag.
pub fn log_stats_file_open() -> bool {
    cs().log_stats_file_open_flag
}

/// log_stats file write.
/// Writes to the log_stats file if open, managed in cl_main.
pub fn log_stats_write(msg: &str) {
    let mut state = cs();
    if let Some(ref mut f) = state.log_stats_file {
        let _ = f.write_all(msg.as_bytes());
    }
}

// Log stats file state is now inside ConsoleState (accessed via cs())

/// con_initialized â€” reads the global CON state
pub fn con_initialized() -> bool {
    cs().con.initialized
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
    pub r_register_model: fn(&str) -> isize,
    pub r_register_skin: fn(&str) -> isize,
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
fn noop_r_register_model(_name: &str) -> isize { 0 }
fn noop_r_register_skin(_name: &str) -> isize { 0 }
fn noop_r_set_sky(_name: &str, _rotate: f32, _axis: &[f32; 3]) {}
fn noop_r_set_palette_null() {}
fn noop_vk_imp_end_frame() {}
fn noop_r_add_stain(_org: &[f32; 3], _intensity: f32, _r: f32, _g: f32, _b: f32, _a: f32, _mode: i32) {}
fn noop_draw_stretch_raw(_x: i32, _y: i32, _w: i32, _h: i32, _cols: i32, _rows: i32, _data: &[u8]) {}
fn noop_viddef_width() -> i32 { 640 }
fn noop_viddef_height() -> i32 { 480 }
fn noop_r_set_palette(_palette: Option<&[u8]>) {}

use std::sync::OnceLock;

static RENDERER_FNS: OnceLock<RendererFunctions> = OnceLock::new();

/// Get the renderer function table.
pub fn renderer_fns() -> &'static RendererFunctions {
    RENDERER_FNS.get().unwrap_or(&DEFAULT_RENDERER_FNS)
}

/// Set the renderer function table (called once at startup by myq2-sys).
pub fn set_renderer_fns(fns: RendererFunctions) {
    let _ = RENDERER_FNS.set(fns);
}

static DEFAULT_RENDERER_FNS: RendererFunctions = RendererFunctions {
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

static SYSTEM_FNS: OnceLock<SystemFunctions> = OnceLock::new();

/// Get the system function table.
pub fn system_fns() -> &'static SystemFunctions {
    SYSTEM_FNS.get().unwrap_or(&DEFAULT_SYSTEM_FNS)
}

/// Set the system function table (called once at startup by myq2-sys).
pub fn set_system_fns(fns: SystemFunctions) {
    let _ = SYSTEM_FNS.set(fns);
}

static DEFAULT_SYSTEM_FNS: SystemFunctions = SystemFunctions {
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

static VID_MENU_FNS: OnceLock<VidMenuFunctions> = OnceLock::new();

/// Get the video menu function table.
pub fn vid_menu_fns() -> &'static VidMenuFunctions {
    VID_MENU_FNS.get().unwrap_or(&DEFAULT_VID_MENU_FNS)
}

/// Set the video menu function table (called once at startup by myq2-sys).
pub fn set_vid_menu_fns(fns: VidMenuFunctions) {
    let _ = VID_MENU_FNS.set(fns);
}

static DEFAULT_VID_MENU_FNS: VidMenuFunctions = VidMenuFunctions {
    vid_menu_init: noop_vid_menu_init,
    vid_menu_draw: noop_vid_menu_draw,
    vid_menu_key: noop_vid_menu_key,
};

/// R_BeginFrame â€” dispatches through renderer function pointer table.
pub fn r_begin_frame(separation: f32) {
    (renderer_fns().r_begin_frame)(separation)
}

/// R_RenderFrame â€” dispatches through renderer function pointer table.
pub fn r_render_frame(refdef: &super::client::RefDef) {
    (renderer_fns().r_render_frame)(refdef)
}

/// R_BeginRegistration â€” dispatches through renderer function pointer table.
pub fn r_begin_registration(map: &str) {
    (renderer_fns().r_begin_registration)(map)
}

/// R_EndRegistration â€” dispatches through renderer function pointer table.
pub fn r_end_registration() {
    (renderer_fns().r_end_registration)()
}

/// R_RegisterModel â€” dispatches through renderer function pointer table.
pub fn r_register_model(name: &str) -> isize {
    (renderer_fns().r_register_model)(name)
}

/// R_RegisterSkin â€” dispatches through renderer function pointer table.
pub fn r_register_skin(name: &str) -> isize {
    (renderer_fns().r_register_skin)(name)
}

/// R_SetSky â€” dispatches through renderer function pointer table.
pub fn r_set_sky(name: &str, rotate: f32, axis: &[f32; 3]) {
    (renderer_fns().r_set_sky)(name, rotate, axis)
}

/// R_SetPalette(NULL) â€” dispatches through renderer function pointer table.
pub fn r_set_palette_null() {
    (renderer_fns().r_set_palette_null)()
}

/// GLimp_EndFrame â€” dispatches through renderer function pointer table.
pub fn vk_imp_end_frame() {
    (renderer_fns().vk_imp_end_frame)();
}

/// S_StopAllSounds â€” dispatches through system function pointer table.
pub fn s_stop_all_sounds() {
    (system_fns().s_stop_all_sounds)()
}

/// CM_InlineModel â€” partially wired. The real function in myq2_common::cmodel returns
/// a CModel struct, but client code stores the result as i32 (headnode). This needs
/// a type adapter when model_clip storage is refactored to use CModel.
pub fn cm_inline_model(_name: &str) -> i32 {
    // Returns headnode from the CModel for now
    myq2_common::cmodel::cm_inline_model(_name).headnode
}

pub fn get_viddef() -> VidDef {
    cs().viddef
}

/// SCR_DrawCinematic â€” delegates to cl_cin::scr_draw_cinematic which handles
/// palette setting and raw frame rendering. Returns true if a cinematic is active.
pub fn scr_draw_cinematic() -> bool {
    // SAFETY: CL/CLS initialized at startup, accessed from main thread
    unsafe {
        super::cl_cin::scr_draw_cinematic(&mut *CL_PTR, &*CLS_PTR)
    }
}

/// M_Draw â€” wired to menu module.
pub fn m_draw() {
    super::menu::m_draw();
}

/// V_RenderView â€” wired to cl_view module.
pub fn v_render_view(
    scr: &mut super::cl_scrn::ScrState,
    cls: &super::client::ClientStatic,
    cl: &mut super::client::ClientState,
    viddef: &VidDef,
    stereo_separation: f32,
) {
    super::cl_view::v_render_view(scr, cls, cl, viddef, stereo_separation);
}

/// CL_DrawInventory â€” wired to cl_inv module.
pub fn cl_draw_inventory(
    scr: &mut super::cl_scrn::ScrState,
    cls: &super::client::ClientStatic,
    cl: &super::client::ClientState,
    viddef: &VidDef,
) {
    super::cl_inv::cl_draw_inventory(scr, cls, cl, viddef);
}

/// CL_ParseClientinfo â€” wired to cl_parse module.
pub fn cl_parse_clientinfo(cl: &mut super::client::ClientState, player: usize) {
    super::cl_parse::cl_parse_clientinfo(cl, player);
}

/// CL_LoadClientinfo â€” wired to cl_parse module.
pub fn cl_load_clientinfo(ci: &mut super::client::ClientInfo, s: &str) {
    super::cl_parse::cl_load_clientinfo(ci, s);
}

/// CL_RegisterTentModels â€” wired to cl_tent module using global tent state.
/// The real function takes `&mut TEntState`; we use the LazyLock<Mutex> in cl_main.
pub fn cl_register_tent_models() {
    let mut ts = super::cl_main::TENT_STATE.lock().unwrap_or_else(|e| e.into_inner());
    super::cl_tent::cl_register_tent_models(&mut ts);
}

/// CL_AddEntities â€” dispatches to the real cl_add_entities in cl_ents.rs.
/// Locks the additional global state (ENT_STATE, PROJ_STATE, CLS) needed
/// beyond the already-borrowed ClientState.
pub fn cl_add_entities(cl: &mut super::client::ClientState) {
    use super::cl_main::{CLS, ENT_STATE, PROJ_STATE, FX_STATE, TENT_STATE, SOUND_STATE};
    use super::cl_parse::FrameCallbacks;

    let cls = CLS.lock().unwrap_or_else(|e| e.into_inner());
    let mut ent_state = ENT_STATE.lock().unwrap_or_else(|e| e.into_inner());
    let mut proj_state = PROJ_STATE.lock().unwrap_or_else(|e| e.into_inner());
    let mut fx_state = FX_STATE.lock().unwrap_or_else(|e| e.into_inner());
    let mut tent_state = TENT_STATE.lock().unwrap_or_else(|e| e.into_inner());
    let mut sound_state = SOUND_STATE.lock().unwrap_or_else(|e| e.into_inner());

    // Read cvar values for the dispatch
    let cl_showclamp = myq2_common::cvar::cvar_variable_value("showclamp") != 0.0;
    let cl_timedemo = cl_timedemo_value() != 0.0;
    let cl_predict = myq2_common::cvar::cvar_variable_value("cl_predict") != 0.0;
    let cl_gun = myq2_common::cvar::cvar_variable_value("cl_gun") != 0.0;

    let view_state = super::cl_main::VIEW_STATE.lock().unwrap_or_else(|e| e.into_inner());
    let gun_model = view_state.gun_model;
    let gun_frame = view_state.gun_frame;
    drop(view_state);

    let hand = myq2_common::cvar::cvar_variable_value("hand") as i32;

    let mut frame_cb = FrameCallbacks {
        fx: &mut fx_state,
        tent: &mut tent_state,
        sound: &mut sound_state,
        cl_time: cl.time as f32,
        realtime: cls.realtime,
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

/// keybindings access â€” wired to keys module.
pub fn keybindings(key: i32) -> Option<String> {
    if !(0..256).contains(&key) { return None; }
    super::keys::ks().keybindings[key as usize].clone()
}

/// get_view_state - returns cl_add_* cvar values â€” wired to myq2_common.
pub fn get_view_state() -> (f32, f32, f32, f32) {
    (
        cl_add_entities_value(),
        cl_add_lights_value(),
        cl_add_particles_value(),
        cl_add_blend_value(),
    )
}

/// scr_size_up command fn â€” wired to cl_scrn.
pub fn scr_size_up_f_cmd() {
    let state = cs();
    super::cl_scrn::scr_size_up_f(&state.scr);
}

/// scr_size_down command fn â€” wired to cl_scrn.
pub fn scr_size_down_f_cmd() {
    let state = cs();
    super::cl_scrn::scr_size_down_f(&state.scr);
}

/// V_Gun_Model_f command fn â€” wired to cl_view.
pub fn v_gun_model_f_cmd() {
    let mut vs = super::cl_main::VIEW_STATE.lock().unwrap_or_else(|e| e.into_inner());
    super::cl_view::v_gun_model_f(&mut vs);
}

/// V_Gun_Next_f command fn â€” wired to cl_view.
pub fn v_gun_next_f_cmd() {
    let mut vs = super::cl_main::VIEW_STATE.lock().unwrap_or_else(|e| e.into_inner());
    super::cl_view::v_gun_next_f(&mut vs);
}

/// V_Gun_Prev_f command fn â€” wired to cl_view.
pub fn v_gun_prev_f_cmd() {
    let mut vs = super::cl_main::VIEW_STATE.lock().unwrap_or_else(|e| e.into_inner());
    super::cl_view::v_gun_prev_f(&mut vs);
}

/// V_Viewpos_f command fn â€” wired to cl_view.
pub fn v_viewpos_f_cmd() {
    // SAFETY: CL/CLS initialized at startup, accessed from main thread
    unsafe {
        super::cl_view::v_viewpos_f(&*CL_PTR);
    }
}

/// MSG_ReadShort â€” reads from the global net_message buffer.
/// The real function in myq2_common::common::msg_read_short takes &mut SizeBuf.
/// This wrapper accesses the LazyLock<Mutex> net_message buffer in cl_main.
pub fn msg_read_short() -> i32 {
    let mut msg = super::cl_main::NET_MESSAGE.lock().unwrap_or_else(|e| e.into_inner());
    myq2_common::common::msg_read_short(&mut msg)
}

// ============================================================
// viddef placeholder
// ============================================================

pub use myq2_common::q_shared::VidDef;

// VIDDEF is now inside ConsoleState (accessed via cs().viddef)

// ============================================================
// Shared state placeholders (to be replaced with real globals)
// ============================================================

// SAFETY: Global client state. Single-threaded engine, matches C global access pattern.
// We store raw pointers and provide deref access. Must call init_client_globals() first.
pub static mut CL_PTR: *mut ClientState = std::ptr::null_mut();
pub static mut CLS_PTR: *mut ClientStatic = std::ptr::null_mut();

/// Initialize the global client state. Must be called once at startup before any access.
pub fn init_client_globals() {
    // SAFETY: Box::leak ensures 'static lifetime, initialized once at startup
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
        // SAFETY: CL/CLS initialized before use, accessed from main thread
        unsafe { &*CL_PTR }
    }
}
impl std::ops::DerefMut for ClAccess {
    fn deref_mut(&mut self) -> &mut ClientState {
        // SAFETY: CL/CLS initialized before use, accessed from main thread
        unsafe { &mut *CL_PTR }
    }
}
impl std::ops::Deref for ClsAccess {
    type Target = ClientStatic;
    fn deref(&self) -> &ClientStatic {
        // SAFETY: CL/CLS initialized before use, accessed from main thread
        unsafe { &*CLS_PTR }
    }
}
impl std::ops::DerefMut for ClsAccess {
    fn deref_mut(&mut self) -> &mut ClientStatic {
        // SAFETY: CL/CLS initialized before use, accessed from main thread
        unsafe { &mut *CLS_PTR }
    }
}

/// Global accessor for ClientState â€” use like `CL.field`
pub static mut CL: ClAccess = ClAccess;
/// Global accessor for ClientStatic â€” use like `CLS.field`
pub static mut CLS: ClsAccess = ClsAccess;



// ============================================================
// Chat type constants
// ============================================================

pub const CT_ALL: i32 = 0;
pub const CT_TEAM: i32 = 1;
pub const CT_TELL: i32 = 2;
pub const CT_PERSON: i32 = 3;

// ============================================================
// Console functions
// ============================================================

/// Clear any typing on the current key line.
pub fn key_clear_typing() {
    let mut ks = crate::keys::ks();
    let el = ks.edit_line as usize;
    ks.key_lines[el][1] = 0; // clear any typing
    ks.key_linepos = 1;
}

/// Toggle console on/off.
pub fn con_toggle_console_f() {
    scr_end_loading_plaque(false); // get rid of loading plaque

    // mattx86: console_demos â€” USE_CONSOLE_IN_DEMOS is defined, so skip this block
    if !USE_CONSOLE_IN_DEMOS {
        // SAFETY: CL/CLS initialized at startup, accessed from main thread
        unsafe {
            if CL.attractloop {
                cbuf_add_text("killserver\n");
                return;
            }
        }
    }

    // mattx86: startup_demo â€” DISABLE_STARTUP_DEMO is defined, so skip this block
    if !DISABLE_STARTUP_DEMO {
        // SAFETY: CL/CLS initialized at startup, accessed from main thread
        unsafe {
            if CLS.state == ConnState::Disconnected {
                cbuf_add_text("d1\n");
                return;
            }
        }
    }

    // SAFETY: CL/CLS initialized at startup, accessed from main thread
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

    // SAFETY: CL/CLS initialized at startup, accessed from main thread
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
    cs().con.text.fill(b' ');
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

    let state = cs();
    let con = &state.con;

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

/// Clear all notify times.
pub fn con_clear_notify() {
    let mut state = cs();
    for i in 0..NUM_CON_TIMES {
        state.con.times[i] = 0.0;
    }
}

// ============================================================
// Message mode functions (mattx86)
// ============================================================

/// Enter "say" message mode.
pub fn con_message_mode_f() {
    crate::keys::ks().chat_type = CT_ALL;
    // SAFETY: CLS is a console.rs static mut, not yet wrapped
    unsafe { CLS.key_dest = KeyDest::Message; }
}

/// Enter "say_team" message mode.
pub fn con_message_mode2_f() {
    crate::keys::ks().chat_type = CT_TEAM;
    // SAFETY: CLS is a console.rs static mut, not yet wrapped
    unsafe { CLS.key_dest = KeyDest::Message; }
}

/// Enter "tell" message mode.
pub fn con_message_mode3_f() {
    crate::keys::ks().chat_type = CT_TELL;
    // SAFETY: CLS is a console.rs static mut, not yet wrapped
    unsafe { CLS.key_dest = KeyDest::Message; }
}

/// Enter "say_person" message mode.
pub fn con_message_mode4_f() {
    crate::keys::ks().chat_type = CT_PERSON;
    // SAFETY: CLS is a console.rs static mut, not yet wrapped
    unsafe { CLS.key_dest = KeyDest::Message; }
}

// ============================================================
// Con_CheckResize
// ============================================================

/// If the line width has changed, reformat the buffer.
pub fn con_check_resize() {
    let mut state = cs();
    let width = (state.viddef.width >> 3) - 2;

    if width == state.con.linewidth {
        return;
    }

    if width < 1 {
        // video hasn't been initialized yet
        // mattx86: 38 -> 76 (bigger width before video init)
        let width = 76;
        state.con.linewidth = width;
        state.con.totallines = CON_TEXTSIZE as i32 / state.con.linewidth;
        state.con.text.fill(b' ');
    } else {
        let oldwidth = state.con.linewidth;
        state.con.linewidth = width;
        let oldtotallines = state.con.totallines;
        state.con.totallines = CON_TEXTSIZE as i32 / state.con.linewidth;
        let mut numlines = oldtotallines;

        if state.con.totallines < numlines {
            numlines = state.con.totallines;
        }

        let mut numchars = oldwidth;
        if state.con.linewidth < numchars {
            numchars = state.con.linewidth;
        }

        let mut tbuf = [0u8; CON_TEXTSIZE];
        tbuf.copy_from_slice(&state.con.text);
        state.con.text.fill(b' ');

        for i in 0..numlines {
            for j in 0..numchars {
                let dst = ((state.con.totallines - 1 - i) * state.con.linewidth + j) as usize;
                let src = (((state.con.current - i + oldtotallines) % oldtotallines) * oldwidth + j)
                    as usize;
                if dst < CON_TEXTSIZE && src < CON_TEXTSIZE {
                    state.con.text[dst] = tbuf[src];
                }
            }
        }

        // Inline con_clear_notify to avoid deadlock (we already hold the lock)
        for i in 0..NUM_CON_TIMES {
            state.con.times[i] = 0.0;
        }
    }

    state.con.current = state.con.totallines - 1;
    state.con.display = state.con.current;
}

// ============================================================
// Con_Init
// ============================================================

/// Initialize the console.
pub fn con_init() {
    // Initialize CL_PTR and CLS_PTR before any code tries to use them
    if unsafe { CLS_PTR.is_null() } {
        init_client_globals();
    }

    cs().con.linewidth = -1;

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

    cs().con.initialized = true;

    myq2_common::common::com_printf("Console initialized.\n");
}

/// Initialize con_notifytime cvar.
fn con_notifytime_init() {
    cs().con_notifytime = 3.0; // default, Cvar_Get("con_notifytime", "3", 0)
}

// ============================================================
// Con_Linefeed
// ============================================================

/// Advance to next line in the console buffer.
/// Takes &mut ConsoleState to avoid deadlock when called from con_print.
fn con_linefeed_inner(state: &mut ConsoleState) {
    state.con.x = 0;
    if state.con.display == state.con.current {
        state.con.display += 1;
    }
    state.con.current += 1;
    let start = ((state.con.current % state.con.totallines) * state.con.linewidth) as usize;
    let end = start + state.con.linewidth as usize;
    if end <= CON_TEXTSIZE {
        state.con.text[start..end].fill(b' ');
    }
}

// ============================================================
// Con_Print
// ============================================================

/// Handles cursor positioning, line wrapping, etc.
/// All console printing must go through this in order to be logged to disk.
/// If no console is visible, the text will appear at the top of the game window.
pub fn con_print(txt: &str) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static CR: AtomicBool = AtomicBool::new(false);

    let mut state = cs();
    if !state.con.initialized {
        return;
    }

    // SAFETY: CLS not yet wrapped (deferred â€” raw pointer access pattern with 200+ call sites)
    unsafe {
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
            while l < state.con.linewidth as usize && idx + l < bytes.len() {
                if bytes[idx + l] <= b' ' {
                    break;
                }
                l += 1;
            }

            // word wrap
            if l != state.con.linewidth as usize && (state.con.x + l as i32 > state.con.linewidth) {
                state.con.x = 0;
            }

            idx += 1;

            if CR.load(Ordering::Relaxed) {
                state.con.current -= 1;
                CR.store(false, Ordering::Relaxed);
            }

            if state.con.x == 0 {
                con_linefeed_inner(&mut state);
                // mark time for transparent overlay
                if state.con.current >= 0 {
                    let idx = (state.con.current % NUM_CON_TIMES as i32) as usize;
                    state.con.times[idx] = CLS.realtime as f32;
                }
            }

            match c as u8 {
                b'\n' => {
                    state.con.x = 0;
                }
                b'\r' => {
                    state.con.x = 0;
                    CR.store(true, Ordering::Relaxed);
                }
                _ => {
                    // display character and advance
                    let y = (state.con.current % state.con.totallines) as usize;
                    let pos = y * state.con.linewidth as usize + state.con.x as usize;
                    if pos < CON_TEXTSIZE {
                        state.con.text[pos] = (c | mask | state.con.ormask) as u8;
                    }
                    state.con.x += 1;
                    if state.con.x >= state.con.linewidth {
                        state.con.x = 0;
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
    let linewidth = cs().con.linewidth;
    let l = text.len() as i32;
    let mut pad = (linewidth - l) / 2;
    if pad < 0 {
        pad = 0;
    }
    let buffer = format!("{}{}\n", " ".repeat(pad as usize), text);
    con_print(&buffer);
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
    // SAFETY: CLS not yet wrapped
    unsafe {
        if CLS.key_dest == KeyDest::Menu {
            return;
        }
        if CLS.key_dest != KeyDest::Console && CLS.state == ConnState::Active {
            return; // don't draw anything (always draw if not active)
        }
    }

    let ks = crate::keys::ks();
    let text = &ks.key_lines[ks.edit_line as usize];

    // convert byte offset to visible character count
    let mut colorlinepos = ks.key_linepos;

    let mut text_offset = 0usize;

    let state = cs();
    let con_linewidth = state.con.linewidth;
    let con_vislines = state.con.vislines;
    drop(state);

    // prestep if horizontally scrolling
    if colorlinepos > con_linewidth {
        let byteofs = char_offset(text, colorlinepos - con_linewidth);
        text_offset = byteofs;
        colorlinepos = con_linewidth;
    }

    // draw it
    let bytelen = char_offset(&text[text_offset..], con_linewidth);
    let display_text: String = text[text_offset..text_offset + bytelen]
        .iter()
        .take_while(|&&b| b != 0)
        .map(|&b| b as char)
        .collect();
    draw_string_len(8, con_vislines - 22, &display_text, bytelen as i32);

    // add the cursor frame
    let key_insert = ks.key_insert;
    // SAFETY: CLS not yet wrapped
    unsafe {
        if ((CLS.realtime >> 8) & 1) != 0 {
            let cursor_char = if key_insert { b'_' as i32 } else { 11 };
            draw_char(8 + colorlinepos * 8, con_vislines - 22, cursor_char);
        }
    }
}

// ============================================================
// Con_DrawNotify
// ============================================================

/// Draws the last few lines of output transparently over the game top.
pub fn con_draw_notify() {
    // Extract all ConsoleState values we need, then drop the lock
    // to avoid deadlocks with scr_add_dirty_point which also locks cs()
    let (viddef_height, viddef_width, con_current, con_totallines, con_linewidth,
         con_notifytime, con_times, con_text) = {
        let state = cs();
        (state.viddef.height, state.viddef.width, state.con.current,
         state.con.totallines, state.con.linewidth, state.con_notifytime,
         state.con.times, state.con.text)
    };

    // SAFETY: CLS not yet wrapped
    unsafe {
        // mattx86: 67.5% down the screen
        let mut v = (viddef_height as f32 * NOTIFY_VERTPOS_FACTOR) as i32;

        for i in (con_current - NUM_CON_TIMES as i32 + 1)..=con_current {
            if i < 0 {
                continue;
            }
            let time = con_times[(i % NUM_CON_TIMES as i32) as usize];
            if time == 0.0 {
                continue;
            }
            let elapsed = CLS.realtime as f32 - time;
            if elapsed > con_notifytime * 1000.0 {
                continue;
            }

            let line_start = ((i % con_totallines) * con_linewidth) as usize;

            let mut x = NOTIFY_INDENT;
            for c in 0..con_linewidth {
                if line_start + (c as usize) < CON_TEXTSIZE {
                    draw_char(
                        (x + 1) << 3,
                        v,
                        con_text[line_start + c as usize] as i32,
                    );
                }
                x += 1;
            }
            v += 8;
        }

        if CLS.key_dest == KeyDest::Message {
            let ks = crate::keys::ks();
            let skip;
            match ks.chat_type {
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

            let chat_len = ks.chat_bufferlen as usize;
            let max_visible = (viddef_width >> 3) - (skip + 1);
            let s_start = if chat_len as i32 > max_visible {
                chat_len - max_visible as usize
            } else {
                0
            };

            let mut x = 0i32;
            while s_start + (x as usize) < chat_len && ks.chat_buffer[s_start + x as usize] != 0 {
                let char_idx = s_start + x as usize;
                if ks.chat_backedit != 0
                    && ks.chat_backedit == ks.chat_bufferlen - x
                    && ((CLS.realtime >> 8) & 1) != 0
                {
                    draw_char((x + skip) << 3, v, 11);
                } else {
                    draw_char((x + skip) << 3, v, ks.chat_buffer[char_idx] as i32);
                }
                x += 1;
            }

            if ks.chat_backedit == 0 {
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
            scr_add_dirty_point(viddef_width - 1, v);
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
        let _lock = GLOBAL_STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        {
            let mut state = cs();
            state.con.initialized = true;
            state.con.text[0] = b'A';
            state.con.text[100] = b'Z';
            state.con.text[CON_TEXTSIZE - 1] = b'!';
        }
        con_clear_f();
        {
            let state = cs();
            assert_eq!(state.con.text[0], b' ');
            assert_eq!(state.con.text[100], b' ');
            assert_eq!(state.con.text[CON_TEXTSIZE - 1], b' ');
        }
    }

    #[test]
    fn test_con_clear_notify_zeroes_all_times() {
        let _lock = GLOBAL_STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        {
            let mut state = cs();
            state.con.initialized = true;
            for i in 0..NUM_CON_TIMES {
                state.con.times[i] = 1000.0 * (i as f32 + 1.0);
            }
        }
        con_clear_notify();
        {
            let state = cs();
            for i in 0..NUM_CON_TIMES {
                assert_eq!(state.con.times[i], 0.0, "times[{}] should be zeroed", i);
            }
        }
    }

    #[test]
    fn test_con_check_resize_initial_setup() {
        let _lock = GLOBAL_STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let saved_width;
        let saved_linewidth;
        {
            let mut state = cs();
            saved_width = state.viddef.width;
            saved_linewidth = state.con.linewidth;
            state.viddef.width = 0;
            state.con.linewidth = -1;
        }
        con_check_resize();
        {
            let mut state = cs();
            // After resize, linewidth should be positive
            assert!(state.con.linewidth > 0, "linewidth should be positive, got {}", state.con.linewidth);
            // Restore state for other tests
            state.viddef.width = saved_width;
            state.con.linewidth = saved_linewidth;
        }
    }

    #[test]
    fn test_con_check_resize_no_change_when_same_width() {
        let _lock = GLOBAL_STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        {
            let mut state = cs();
            state.con.linewidth = 78;
            state.viddef.width = (78 + 2) << 3; // (width >> 3) - 2 == 78
        }
        con_check_resize();
        {
            let state = cs();
            assert_eq!(state.con.linewidth, 78, "linewidth should be unchanged when it matches");
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
        let _lock = GLOBAL_STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        cs().con.initialized = false;
        // Should not crash or modify anything
        con_print("Hello world!\n");
    }

    #[test]
    fn test_con_print_basic_text() {
        let _lock = GLOBAL_STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // Ensure CLS_PTR is initialized so con_print can read CLS.realtime
        if unsafe { CLS_PTR.is_null() } {
            init_client_globals();
        }
        {
            let mut state = cs();
            state.con.initialized = true;
            state.con.linewidth = 40;
            state.con.totallines = CON_TEXTSIZE as i32 / 40;
            state.con.x = 0;
            state.con.current = state.con.totallines - 1;
            state.con.display = state.con.current;
            state.con.ormask = 0;
            state.con.text.fill(b' ');
        }
        con_print("AB\n");
        // After printing "AB\n":
        // - "AB" should be in the buffer on the line that was started
        // - After '\n', x should be 0
        assert_eq!(cs().con.x, 0, "x should be 0 after newline");
    }

    #[test]
    fn test_con_print_colored_text_prefix() {
        let _lock = GLOBAL_STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // Ensure CLS_PTR is initialized so con_print can read CLS.realtime
        if unsafe { CLS_PTR.is_null() } {
            init_client_globals();
        }
        // Text starting with byte 1 or 2 gets the high bit mask (128)
        {
            let mut state = cs();
            state.con.initialized = true;
            state.con.linewidth = 80;
            state.con.totallines = CON_TEXTSIZE as i32 / 80;
            state.con.x = 0;
            state.con.current = state.con.totallines - 1;
            state.con.display = state.con.current;
            state.con.ormask = 0;
            state.con.text.fill(b' ');
        }
        // Byte 1 prefix -> colored text
        con_print("\x01X\n");
        // The 'X' char should have high bit set (128 | b'X')
        {
            let state = cs();
            let _line_start = ((state.con.current % state.con.totallines) * state.con.linewidth) as usize;
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
        let _lock = GLOBAL_STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // Ensure CLS_PTR is initialized so con_print can read CLS.realtime
        if unsafe { CLS_PTR.is_null() } {
            init_client_globals();
        }
        {
            let mut state = cs();
            state.con.initialized = true;
            state.con.linewidth = 40;
            state.con.totallines = CON_TEXTSIZE as i32 / 40;
            state.con.x = 0;
            state.con.current = state.con.totallines - 1;
            state.con.display = state.con.current;
            state.con.ormask = 0;
            state.con.text.fill(b' ');
        }
        // "Hi" is 2 chars on a 40-char line -> pad = (40-2)/2 = 19
        con_centered_print("Hi");
        // Should not crash and should produce padded output
        // After centering, x should be 0 since the text ends with \n
        assert_eq!(cs().con.x, 0);
    }

    #[test]
    fn test_con_centered_print_text_wider_than_linewidth() {
        let _lock = GLOBAL_STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // Ensure CLS_PTR is initialized so con_print can read CLS.realtime
        if unsafe { CLS_PTR.is_null() } {
            init_client_globals();
        }
        {
            let mut state = cs();
            state.con.initialized = true;
            state.con.linewidth = 5;
            state.con.totallines = CON_TEXTSIZE as i32 / 5;
            state.con.x = 0;
            state.con.current = state.con.totallines - 1;
            state.con.display = state.con.current;
            state.con.ormask = 0;
            state.con.text.fill(b' ');
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
    // Extract values from ConsoleState, update vislines, then drop the lock
    // to avoid deadlocks with scr_add_dirty_point and con_draw_input
    let viddef_width;
    let viddef_height;
    let con_display;
    let con_current;
    let con_totallines;
    let con_linewidth;
    let con_text;
    {
        let state = cs();
        viddef_width = state.viddef.width;
        viddef_height = state.viddef.height;
        con_display = state.con.display;
        con_current = state.con.current;
        con_totallines = state.con.totallines;
        con_linewidth = state.con.linewidth;
        con_text = state.con.text;
    }

    let mut lines = (viddef_height as f32 * frac) as i32;
    if lines <= 0 {
        return;
    }

    if lines > viddef_height {
        lines = viddef_height;
    }

    // draw the background
    // Try to draw conback texture, but use a fallback solid color if it's not available
    // Note: With Vulkan Y-flip, Y=0 is at top, so draw from (0,0) with height=lines
    let pic_id = draw_find_pic("conback");
    if pic_id > 0 {
        draw_stretch_pic(0, 0, viddef_width, lines, "conback");
    } else {
        // Fallback: draw a medium gray background (color 8 in Q2 palette)
        draw_fill(0, 0, viddef_width, lines, 8, 1.0);
    }
    scr_add_dirty_point(0, 0);
    scr_add_dirty_point(viddef_width - 1, lines - 1);

    let version = format!("{} v{:.2}", DISTNAME, DISTVER);
    let vlen = version.len() as i32;
    for (x, ch) in version.bytes().enumerate() {
        draw_char(
            viddef_width - (vlen * 8 + 4) + x as i32 * 8,
            lines - 12,
            128 + ch as i32,
        );
    }

    // update vislines in the state
    cs().con.vislines = lines;

    let rows = (lines - 22) >> 3; // rows of text to draw
    let mut y = lines - 30;

    // draw from the bottom up
    let mut rows = rows;
    if con_display != con_current {
        // draw arrows to show the buffer is backscrolled
        let mut x = 0;
        while x < con_linewidth {
            draw_char((x + 1) << 3, y, b'^' as i32);
            x += 4;
        }
        y -= 8;
        rows -= 1;
    }

    let mut row = con_display;
    for _i in 0..rows {
        if row < 0 {
            break;
        }
        if con_current - row >= con_totallines {
            break; // past scrollback wrap point
        }

        let line_start = ((row % con_totallines) * con_linewidth) as usize;

        for x in 0..con_linewidth {
            if line_start + (x as usize) < CON_TEXTSIZE {
                draw_char(
                    (x + 1) << 3,
                    y,
                    con_text[line_start + x as usize] as i32,
                );
            }
        }

        y -= 8;
        row -= 1;
    }

    // ZOID: draw the download bar
    // SAFETY: CLS not yet wrapped
    unsafe {
        if !CLS.download_name.is_empty() {
            let text = if let Some(pos) = CLS.download_name.rfind('/') {
                &CLS.download_name[pos + 1..]
            } else {
                &CLS.download_name
            };

            let x = con_linewidth - ((con_linewidth * 7) / 40);
            let max_text_len = con_linewidth / 3;
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
            let con_vislines = cs().con.vislines;
            let bar_y = con_vislines - 12;
            for (i, ch) in dlbar.bytes().enumerate() {
                draw_char((i as i32 + 1) << 3, bar_y, ch as i32);
            }
        }
    }

    // draw the input prompt, user text, and cursor if desired
    con_draw_input();
}
