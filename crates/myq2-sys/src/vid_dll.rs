// vid_dll.rs -- main windowed and fullscreen graphics interface module
// Converted from: myq2-original/win32/vid_dll.c

use myq2_common::common::{com_printf, com_dprintf, com_error};
use myq2_common::cvar::{CvarContext, cvar_variable_value};
use myq2_common::q_shared::*;
use myq2_common::cmd::cmd_add_command;
use myq2_client::keys::key_clear_states;
use myq2_client::console::con_toggle_console_f;
use myq2_renderer::vk_rmain::{r_init, r_shutdown};
use std::sync::Mutex;

/// Global VidState accessible from console command callbacks.
static GLOBAL_VID_STATE: Mutex<Option<VidState>> = Mutex::new(None);

/// Store a VidState in the global so console commands can access it.
pub fn vid_set_global_state(vid: VidState) {
    *GLOBAL_VID_STATE.lock().unwrap() = Some(vid);
}

/// Access the global VidState with a closure.
pub fn with_vid_state<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut VidState) -> R,
{
    GLOBAL_VID_STATE.lock().unwrap().as_mut().map(f)
}

// ============================================================
// Constants
// ============================================================

// ============================================================
// Video mode table
// ============================================================

pub struct VidMode {
    pub description: &'static str,
    pub width: i32,
    pub height: i32,
    pub mode: i32,
}

pub const VID_MODES: &[VidMode] = &[
    VidMode { description: "Mode 0: 320x240",   width: 320,  height: 240,  mode: 0 },
    VidMode { description: "Mode 1: 400x300",   width: 400,  height: 300,  mode: 1 },
    VidMode { description: "Mode 2: 512x384",   width: 512,  height: 384,  mode: 2 },
    VidMode { description: "Mode 3: 640x480",   width: 640,  height: 480,  mode: 3 },
    VidMode { description: "Mode 4: 800x600",   width: 800,  height: 600,  mode: 4 },
    VidMode { description: "Mode 5: 960x720",   width: 960,  height: 720,  mode: 5 },
    VidMode { description: "Mode 6: 1024x768",  width: 1024, height: 768,  mode: 6 },
    VidMode { description: "Mode 7: 1152x864",  width: 1152, height: 864,  mode: 7 },
    VidMode { description: "Mode 8: 1280x960",  width: 1280, height: 960,  mode: 8 },
    VidMode { description: "Mode 9: 1600x1200", width: 1600, height: 1200, mode: 9 },
    VidMode { description: "Mode 10: 2048x1536", width: 2048, height: 1536, mode: 10 },
];

pub const VID_NUM_MODES: usize = 11;

// ============================================================
// Scancode to Quake key mapping table
// ============================================================

/// Map from Win32 scancodes (0..127) to Quake key numbers.
/// Matches the original C scantokey[] table exactly.
use myq2_common::keys::*;

pub const SCANTOKEY: [i32; 128] = [
//  0            1       2       3       4       5       6       7
//  8            9       A       B       C       D       E       F
    0,           27,     b'1' as i32, b'2' as i32, b'3' as i32, b'4' as i32, b'5' as i32, b'6' as i32,
    b'7' as i32, b'8' as i32, b'9' as i32, b'0' as i32, b'-' as i32, b'=' as i32, K_BACKSPACE, 9,  // 0
    b'q' as i32, b'w' as i32, b'e' as i32, b'r' as i32, b't' as i32, b'y' as i32, b'u' as i32, b'i' as i32,
    b'o' as i32, b'p' as i32, b'[' as i32, b']' as i32, 13, K_CTRL, b'a' as i32, b's' as i32,     // 1
    b'd' as i32, b'f' as i32, b'g' as i32, b'h' as i32, b'j' as i32, b'k' as i32, b'l' as i32, b';' as i32,
    b'\'' as i32, b'`' as i32, K_SHIFT, b'\\' as i32, b'z' as i32, b'x' as i32, b'c' as i32, b'v' as i32, // 2
    b'b' as i32, b'n' as i32, b'm' as i32, b',' as i32, b'.' as i32, b'/' as i32, K_SHIFT, b'*' as i32,
    K_ALT, b' ' as i32, 0, K_F1, K_F2, K_F3, K_F4, K_F5,                                          // 3
    K_F6, K_F7, K_F8, K_F9, K_F10, K_PAUSE, 0, K_HOME,
    K_UPARROW, K_PGUP, K_KP_MINUS, K_LEFTARROW, K_KP_5, K_RIGHTARROW, K_KP_PLUS, K_END,           // 4
    K_DOWNARROW, K_PGDN, K_INS, K_DEL, 0, 0, 0, K_F11,
    K_F12, 0, 0, 0, 0, 0, 0, 0,                                                                    // 5
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,                                                                         // 6
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,                                                                         // 7
];

// ============================================================
// Video state
// ============================================================

// VidDef is imported from myq2_common::q_shared via the glob import above.

/// All vid_dll state, replacing C globals.
pub struct VidState {
    pub viddef: VidDef,
    pub reflib_active: bool,
    pub vid_ref_modified: bool,

    // Alt-tab state
    pub s_alttab_disabled: bool,

    // Cvar indices
    pub win_noalttab: Option<usize>,
    pub vid_ref: Option<usize>,
    pub vid_gamma: Option<usize>,
    pub vid_xpos: Option<usize>,
    pub vid_ypos: Option<usize>,
    pub vid_fullscreen: Option<usize>,
}

impl Default for VidState {
    fn default() -> Self {
        Self::new()
    }
}

impl VidState {
    pub fn new() -> Self {
        Self {
            viddef: VidDef::default(),
            reflib_active: false,
            vid_ref_modified: false,
            s_alttab_disabled: false,
            win_noalttab: None,
            vid_ref: None,
            vid_gamma: None,
            vid_xpos: None,
            vid_ypos: None,
            vid_fullscreen: None,
        }
    }
}

// ============================================================
// Win32 helper stubs
// ============================================================

/// WIN_DisableAltTab — stub.
pub fn win_disable_alt_tab(vid: &mut VidState) {
    if vid.s_alttab_disabled {
        return;
    }
    // Stub: RegisterHotKey / SystemParametersInfo
    vid.s_alttab_disabled = true;
}

/// WIN_EnableAltTab — stub.
pub fn win_enable_alt_tab(vid: &mut VidState) {
    if vid.s_alttab_disabled {
        // Stub: UnregisterHotKey / SystemParametersInfo
        vid.s_alttab_disabled = false;
    }
}

// ============================================================
// DLL glue
// ============================================================

/// VID_Printf — route print messages by level.
pub fn vid_printf(print_level: i32, msg: &str) {
    match print_level {
        x if x == PRINT_ALL => {
            com_printf(msg);
        }
        x if x == PRINT_INFO => {
            if cvar_variable_value("r_verbose") != 0.0 {
                com_printf(msg);
            }
        }
        x if x == PRINT_DEVELOPER => {
            com_dprintf(msg);
        }
        x if x == PRINT_ALERT => {
            com_printf(msg);
        }
        _ => {
            com_error(print_level, msg);
        }
    }
}

// ============================================================
// Key mapping
// ============================================================

/// MapKey — map from Windows key lParam to Quake keynum.
pub fn map_key(key: i32) -> i32 {
    let modified = ((key >> 16) & 255) as usize;
    if modified > 127 {
        return 0;
    }

    let is_extended = (key & (1 << 24)) != 0;
    let result = SCANTOKEY[modified];

    if !is_extended {
        // Non-extended keys: remap to numpad KP variants
        match result {
            K_HOME => K_KP_HOME,
            K_UPARROW => K_KP_UPARROW,
            K_PGUP => K_KP_PGUP,
            K_LEFTARROW => K_KP_LEFTARROW,
            K_RIGHTARROW => K_KP_RIGHTARROW,
            K_END => K_KP_END,
            K_DOWNARROW => K_KP_DOWNARROW,
            K_PGDN => K_KP_PGDN,
            K_INS => K_KP_INS,
            K_DEL => K_KP_DEL,
            _ => result,
        }
    } else {
        // Extended keys: remap Enter and Slash to KP variants
        match result {
            13 => K_KP_ENTER,
            0x2F => K_KP_SLASH,
            _ => result,
        }
    }
}

/// AppActivate — handle window activation/deactivation.
pub fn app_activate(
    vid: &mut VidState,
    cvars: &CvarContext,
    f_active: bool,
    minimize: bool,
    active_app: &mut bool,
    minimized: &mut bool,
) {
    *minimized = minimize;
    key_clear_states();

    *active_app = f_active && !minimize;

    if !*active_app {
        // Stub: IN_Activate(false), S_Activate(false)
        if let Some(idx) = vid.win_noalttab {
            if let Some(cv) = cvars.cvar_vars.get(idx) {
                if cv.value != 0.0 {
                    win_enable_alt_tab(vid);
                }
            }
        }
    } else {
        // Stub: IN_Activate(true), S_Activate(true)
        if let Some(idx) = vid.win_noalttab {
            if let Some(cv) = cvars.cvar_vars.get(idx) {
                if cv.value != 0.0 {
                    win_disable_alt_tab(vid);
                }
            }
        }
    }
}

/// MainWndProc — stub; in the Rust port, window messages will be handled by
/// the windowing library (e.g., winit). This is kept as documentation of the
/// original message handling flow.
pub fn main_wnd_proc_stub() {
    // The original C code handled WM_MOUSEWHEEL, WM_HOTKEY, WM_CREATE,
    // WM_PAINT, WM_DESTROY, WM_ACTIVATE, WM_MOVE, WM_LBUTTONDOWN/UP,
    // WM_RBUTTONDOWN/UP, WM_MBUTTONDOWN/UP, WM_XBUTTONDOWN/UP,
    // WM_MOUSEMOVE, WM_SYSCOMMAND, WM_SYSKEYDOWN, WM_KEYDOWN,
    // WM_SYSKEYUP, WM_KEYUP, MM_MCINOTIFY.
    //
    // In the Rust port these are replaced by the platform windowing events.
}

// ============================================================
// Video mode queries
// ============================================================

/// VID_Restart_f — console command to re-start video mode.
pub fn vid_restart_f(vid: &mut VidState) {
    vid.vid_ref_modified = true;
}

/// VID_Front_f — stub (SetWindowLong / SetForegroundWindow).
pub fn vid_front_f() {
    // Stub: SetWindowLong(cl_hwnd, GWL_EXSTYLE, WS_EX_TOPMOST)
    // Stub: SetForegroundWindow(cl_hwnd)
}

/// VID_GetModeInfo — retrieve width/height for a given video mode.
pub fn vid_get_mode_info(mode: i32) -> Option<(i32, i32)> {
    if mode < 0 || (mode as usize) >= VID_NUM_MODES {
        return None;
    }
    let m = &VID_MODES[mode as usize];
    Some((m.width, m.height))
}

/// VID_UpdateWindowPosAndSize — stub (MoveWindow).
pub fn vid_update_window_pos_and_size(vid: &VidState, _x: f32, _y: f32) {
    // Stub: GetWindowLong, AdjustWindowRect, MoveWindow
    let _w = vid.viddef.width;
    let _h = vid.viddef.height;
}

/// VID_NewWindow
pub fn vid_new_window(vid: &mut VidState, width: i32, height: i32, force_refdef: &mut bool) {
    vid.viddef.width = width;
    vid.viddef.height = height;
    *force_refdef = true; // can't use a paused refdef
}

/// VID_FreeReflib
pub fn vid_free_reflib(vid: &mut VidState) {
    r_shutdown();
    vid.reflib_active = false;
}

/// VID_LoadRefresh — load/initialize the OpenGL renderer.
pub fn vid_load_refresh(vid: &mut VidState, hinstance: usize, hwnd: usize) -> bool {
    if vid.reflib_active {
        vid_free_reflib(vid);
    }

    vid_printf(PRINT_INFO, "-------- Loading OpenGL Ref --------\n");

    // Stub: Swap_Init()
    let result = r_init(hinstance, hwnd);
    if result == 0 {
        vid_printf(PRINT_INFO, "Failed to initialize OpenGL renderer\n");
        return false;
    }

    vid_printf(PRINT_INFO, "------------------------------------\n");
    vid.reflib_active = true;

    true
}

/// VID_CheckChanges — called once per frame before drawing to check for video
/// mode parameter changes.
pub fn vid_check_changes(vid: &mut VidState, cvars: &mut CvarContext, cls_disable_screen: &mut bool, force_refdef: &mut bool, refresh_prepped: &mut bool, hinstance: usize, hwnd: usize, key_dest: i32) {
    // Check win_noalttab
    if let Some(idx) = vid.win_noalttab {
        if let Some(cv) = cvars.cvar_vars.get(idx) {
            if cv.modified {
                if cv.value != 0.0 {
                    win_disable_alt_tab(vid);
                } else {
                    win_enable_alt_tab(vid);
                }
            }
        }
        if let Some(cv) = cvars.cvar_vars.get_mut(idx) {
            cv.modified = false;
        }
    }

    if vid.vid_ref_modified {
        *force_refdef = true;
        // Stub: S_StopAllSounds()
    }

    while vid.vid_ref_modified {
        vid.vid_ref_modified = false;
        if let Some(idx) = vid.vid_fullscreen {
            if let Some(cv) = cvars.cvar_vars.get_mut(idx) {
                cv.modified = true;
            }
        }
        *refresh_prepped = false;
        *cls_disable_screen = true;

        if !vid_load_refresh(vid, hinstance, hwnd) {
            // KEY_CONSOLE = 1 (from client.h)
            if key_dest != 1 {
                con_toggle_console_f();
            }
        }
        *cls_disable_screen = false;
    }

    // Update window position
    let xpos_modified = vid.vid_xpos
        .and_then(|idx| cvars.cvar_vars.get(idx))
        .map(|cv| cv.modified)
        .unwrap_or(false);
    let ypos_modified = vid.vid_ypos
        .and_then(|idx| cvars.cvar_vars.get(idx))
        .map(|cv| cv.modified)
        .unwrap_or(false);

    if xpos_modified || ypos_modified {
        let fs_value = vid.vid_fullscreen
            .and_then(|idx| cvars.cvar_vars.get(idx))
            .map(|cv| cv.value)
            .unwrap_or(0.0);

        if fs_value == 0.0 {
            let xpos = vid.vid_xpos
                .and_then(|idx| cvars.cvar_vars.get(idx))
                .map(|cv| cv.value)
                .unwrap_or(0.0);
            let ypos = vid.vid_ypos
                .and_then(|idx| cvars.cvar_vars.get(idx))
                .map(|cv| cv.value)
                .unwrap_or(0.0);
            vid_update_window_pos_and_size(vid, xpos, ypos);
        }

        if let Some(idx) = vid.vid_xpos {
            if let Some(cv) = cvars.cvar_vars.get_mut(idx) {
                cv.modified = false;
            }
        }
        if let Some(idx) = vid.vid_ypos {
            if let Some(cv) = cvars.cvar_vars.get_mut(idx) {
                cv.modified = false;
            }
        }
    }
}

/// VID_Init — initialize the video subsystem.
pub fn vid_init(vid: &mut VidState, cvars: &mut CvarContext, hinstance: usize, hwnd: usize) {
    vid.vid_ref = Some(cvars.get_or_create("vid_ref", "gl", CVAR_ARCHIVE));
    vid.vid_xpos = Some(cvars.get_or_create("vid_xpos", "3", CVAR_ARCHIVE));
    vid.vid_ypos = Some(cvars.get_or_create("vid_ypos", "22", CVAR_ARCHIVE));
    vid.vid_fullscreen = Some(cvars.get_or_create("vid_fullscreen", "1", CVAR_ARCHIVE));
    vid.vid_gamma = Some(cvars.get_or_create("vid_gamma", "0.6", CVAR_ARCHIVE));
    vid.win_noalttab = Some(cvars.get_or_create("win_noalttab", "0", CVAR_ARCHIVE));

    // Register console commands
    cmd_add_command("vid_restart", Some(Box::new(|_ctx| {
        with_vid_state(vid_restart_f);
    })));
    cmd_add_command("vid_front", Some(Box::new(|_ctx| {
        vid_front_f();
    })));

    // Disable 3Dfx splash screen
    std::env::set_var("FX_GLIDE_NO_SPLASH", "0");

    // Start the graphics mode
    let mut disable_screen = false;
    let mut force_refdef = false;
    let mut refresh_prepped = false;
    let key_dest = 0; // Stub: will be passed from caller in full integration
    vid_check_changes(vid, cvars, &mut disable_screen, &mut force_refdef, &mut refresh_prepped, hinstance, hwnd, key_dest);
}

/// VID_Shutdown
pub fn vid_shutdown(vid: &mut VidState) {
    if vid.reflib_active {
        vid_free_reflib(vid);
    }
}
