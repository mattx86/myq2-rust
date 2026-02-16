// in_win.rs -- mouse and input handling
// Converted from: myq2-original/win32/in_win.c
//
// Uses winit for cursor grab mode. On Windows, also uses Win32 SetCapture/
// ReleaseCapture to ensure mouse button events are captured (matching the
// original C code's behavior).

use std::sync::Arc;
use std::sync::Mutex;

use winit::window::{Window, CursorGrabMode};

use myq2_common::cvar::CvarContext;
use myq2_common::q_shared::*;

// Win32 FFI for mouse capture (matches original in_win.c behavior)
#[cfg(target_os = "windows")]
extern "system" {
    fn SetCapture(hwnd: isize) -> isize;
    fn ReleaseCapture() -> i32;
    fn ClipCursor(lp_rect: *const WinRect) -> i32;
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct WinRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

/// Extract the Win32 HWND from a winit Window.
#[cfg(target_os = "windows")]
fn get_hwnd(window: &Window) -> Option<isize> {
    use raw_window_handle::HasWindowHandle;
    if let Ok(handle) = window.window_handle() {
        if let raw_window_handle::RawWindowHandle::Win32(win32) = handle.as_raw() {
            return Some(win32.hwnd.get() as isize);
        }
    }
    None
}

/// Global input state, accessible from the event pump in sys_win.rs.
pub static INPUT_STATE: Mutex<InputState> = Mutex::new(InputState::new_const());

/// All input state, replacing the C globals.
pub struct InputState {
    // Mouse variables
    pub mlooking: bool,
    pub mouse_buttons: i32,
    pub mouse_oldbuttonstate: i32,
    pub mouse_x: f64,
    pub mouse_y: f64,
    pub old_mouse_x: f64,
    pub old_mouse_y: f64,
    /// Accumulated mouse X delta (f64 to avoid truncating sub-pixel touchpad motion).
    pub mx_accum: f64,
    /// Accumulated mouse Y delta (f64 to avoid truncating sub-pixel touchpad motion).
    pub my_accum: f64,
    pub mouseactive: bool,
    pub mouseinitialized: bool,
    pub window_center_x: i32,
    pub window_center_y: i32,

    // App active
    pub in_appactive: bool,

    // Fallback: track last cursor position for WindowEvent::CursorMoved
    pub last_cursor_x: f64,
    pub last_cursor_y: f64,
    pub has_cursor_pos: bool,
    /// True if DeviceEvent::MouseMotion has ever fired (raw input works).
    pub raw_input_works: bool,
    /// True when CursorGrabMode::Locked succeeded; false when using Confined fallback.
    /// Used to decide whether Win32 SetCapture/ClipCursor are needed.
    pub cursor_locked: bool,

    // Cvar indices (into CvarContext)
    pub in_mouse: Option<usize>,
    pub m_filter: Option<usize>,
}

impl InputState {
    /// Const-compatible constructor for use in static Mutex initializer.
    pub const fn new_const() -> Self {
        Self {
            mlooking: false,
            mouse_buttons: 0,
            mouse_oldbuttonstate: 0,
            mouse_x: 0.0,
            mouse_y: 0.0,
            old_mouse_x: 0.0,
            old_mouse_y: 0.0,
            mx_accum: 0.0,
            my_accum: 0.0,
            mouseactive: false,
            mouseinitialized: false,
            window_center_x: 0,
            window_center_y: 0,
            in_appactive: false,
            last_cursor_x: 0.0,
            last_cursor_y: 0.0,
            has_cursor_pos: false,
            raw_input_works: false,
            cursor_locked: false,
            in_mouse: None,
            m_filter: None,
        }
    }
}

impl Default for InputState {
    fn default() -> Self {
        Self::new_const()
    }
}

// ============================================================
// Mouse control
// ============================================================

/// IN_MLookDown
pub fn in_mlook_down(input: &mut InputState) {
    input.mlooking = true;
}

/// IN_MLookUp
pub fn in_mlook_up(input: &mut InputState, freelook_value: f32, lookspring_value: f32) {
    input.mlooking = false;
    if freelook_value == 0.0 && lookspring_value != 0.0 {
        in_center_view();
    }
}

/// IN_CenterView — centers the player's vertical view angle.
pub fn in_center_view() {
    myq2_client::cl_main::cl_center_view();
}

/// IN_ActivateMouse — called when the window gains focus or changes.
///
/// Enables winit cursor grab mode + Win32 SetCapture for FPS-style mouse capture.
/// SetCapture ensures mouse button events are routed to our window even if the
/// cursor briefly escapes confinement (matches original C code behavior).
pub fn in_activate_mouse(input: &mut InputState, cvars: &CvarContext, window: &Arc<Window>) {
    if !input.mouseinitialized {
        return;
    }

    if let Some(idx) = input.in_mouse {
        if let Some(cv) = cvars.cvar_vars.get(idx) {
            if cv.value == 0.0 {
                input.mouseactive = false;
                return;
            }
        }
    }

    if input.mouseactive {
        return;
    }

    input.mouseactive = true;

    // For FPS games, prefer Locked mode (raw relative motion, cursor hidden)
    // Fall back to Confined (cursor stays in window bounds) if Locked isn't available
    let locked = window.set_cursor_grab(CursorGrabMode::Locked).is_ok();
    if !locked {
        let _ = window.set_cursor_grab(CursorGrabMode::Confined);
    }
    input.cursor_locked = locked;

    window.set_cursor_visible(false); // Hide cursor during gameplay

    // Win32: Always use SetCapture to ensure mouse button events are routed to our
    // window, even if CursorGrabMode::Locked is active. Without SetCapture, clicks
    // can escape to the window frame (title bar) and trigger minimize/maximize.
    // ClipCursor is only used in Confined fallback mode since it can conflict with
    // the raw input that Locked mode uses for cursor confinement.
    #[cfg(target_os = "windows")]
    if let Some(hwnd) = get_hwnd(window) {
        // SAFETY: SetCapture/ClipCursor are standard Win32 APIs called with a valid HWND.
        unsafe {
            SetCapture(hwnd);

            if !locked {
                // Clip cursor to window client area (Confined fallback only)
                let size = window.inner_size();
                let pos = window.inner_position().unwrap_or_default();
                let rect = WinRect {
                    left: pos.x,
                    top: pos.y,
                    right: pos.x + size.width as i32,
                    bottom: pos.y + size.height as i32,
                };
                ClipCursor(&rect);
            }
        }
    }
}

/// IN_DeactivateMouse — called when the window loses focus.
pub fn in_deactivate_mouse(input: &mut InputState, window: Option<&Arc<Window>>) {
    if !input.mouseinitialized {
        return;
    }
    if !input.mouseactive {
        return;
    }

    input.mouseactive = false;

    // Win32: Always release SetCapture (we now call SetCapture in both modes).
    // Only release ClipCursor if we used it (Confined fallback mode).
    #[cfg(target_os = "windows")]
    {
        // SAFETY: ReleaseCapture/ClipCursor(NULL) are safe Win32 calls
        unsafe {
            ReleaseCapture();
            if !input.cursor_locked {
                ClipCursor(std::ptr::null());
            }
        }
    }
    input.cursor_locked = false;

    if let Some(win) = window {
        let _ = win.set_cursor_grab(CursorGrabMode::None); // Release cursor grab
        win.set_cursor_visible(true); // Show cursor
    }
}

/// IN_StartupMouse
pub fn in_startup_mouse(input: &mut InputState, cvars: &mut CvarContext) {
    let cv_idx = cvars.get_or_create("in_initmouse", "1", CVAR_NOSET | CVAR_ARCHIVE);
    if let Some(cv) = cvars.cvar_vars.get(cv_idx) {
        if cv.value == 0.0 {
            return;
        }
    }

    input.mouseinitialized = true;
    input.mouse_buttons = 5; // winit supports at least 5 buttons
}

/// IN_MouseMove — process accumulated mouse motion from the winit event pump.
///
/// Instead of calling GetCursorPos / SetCursorPos, we consume the relative
/// motion accumulated in mx_accum / my_accum from winit DeviceEvent::MouseMotion.
pub fn in_mouse_move(
    input: &mut InputState,
    cmd: &mut UserCmd,
    cvars: &CvarContext,
    viewangles: &mut Vec3,
    sensitivity_value: f32,
    m_yaw_value: f32,
    m_pitch_value: f32,
    m_forward_value: f32,
    m_side_value: f32,
    in_strafe_state: i32,
    lookstrafe_value: f32,
    freelook_value: f32,
) {
    if !input.mouseactive {
        return;
    }

    // Consume accumulated relative motion from winit events (f64 to preserve sub-pixel precision)
    let mx = input.mx_accum;
    let my = input.my_accum;
    input.mx_accum = 0.0;
    input.my_accum = 0.0;

    let m_filter_value = input
        .m_filter
        .and_then(|idx| cvars.cvar_vars.get(idx))
        .map(|cv| cv.value)
        .unwrap_or(0.0);

    if m_filter_value != 0.0 {
        input.mouse_x = (mx + input.old_mouse_x) * 0.5;
        input.mouse_y = (my + input.old_mouse_y) * 0.5;
    } else {
        input.mouse_x = mx;
        input.mouse_y = my;
    }

    input.old_mouse_x = mx;
    input.old_mouse_y = my;

    let mouse_x_f = input.mouse_x as f32 * sensitivity_value;
    let mouse_y_f = input.mouse_y as f32 * sensitivity_value;

    // add mouse X/Y movement to cmd
    if (in_strafe_state & 1) != 0 || (lookstrafe_value != 0.0 && input.mlooking) {
        cmd.sidemove = cmd.sidemove.saturating_add((m_side_value * mouse_x_f) as i16);
    } else {
        viewangles[YAW] -= m_yaw_value * mouse_x_f;
    }

    if (input.mlooking || freelook_value != 0.0) && (in_strafe_state & 1) == 0 {
        viewangles[PITCH] += m_pitch_value * mouse_y_f;
    } else {
        cmd.forwardmove = cmd.forwardmove.saturating_sub((m_forward_value * mouse_y_f) as i16);
    }
}

// ============================================================
// Input system init / shutdown / frame
// ============================================================

/// IN_Init — register all input cvars and start mouse.
pub fn in_init(input: &mut InputState, cvars: &mut CvarContext) {
    // mouse variables
    input.m_filter = Some(cvars.get_or_create("m_filter", "0", CVAR_ARCHIVE));
    input.in_mouse = Some(cvars.get_or_create("in_mouse", "1", CVAR_ARCHIVE));

    in_startup_mouse(input, cvars);
}

/// IN_Shutdown
pub fn in_shutdown(input: &mut InputState, window: Option<&Arc<Window>>) {
    in_deactivate_mouse(input, window);
}

/// IN_Activate — called when the main window gains or loses focus.
pub fn in_activate(input: &mut InputState, active: bool) {
    input.in_appactive = active;
    input.mouseactive = !active; // force a new window check or turn off
}

/// IN_Frame — called every frame, even if not generating commands.
pub fn in_frame(
    input: &mut InputState,
    cvars: &CvarContext,
    window: Option<&Arc<Window>>,
    refresh_prepped: bool,
    key_dest_is_console_or_menu: bool,
    vid_fullscreen_value: f32,
) {
    if !input.mouseinitialized {
        return;
    }

    let in_mouse_value = input
        .in_mouse
        .and_then(|idx| cvars.cvar_vars.get(idx))
        .map(|cv| cv.value)
        .unwrap_or(0.0);

    if in_mouse_value == 0.0 || !input.in_appactive {
        in_deactivate_mouse(input, window);
        return;
    }

    if !refresh_prepped || key_dest_is_console_or_menu {
        // temporarily deactivate if not in fullscreen
        if vid_fullscreen_value == 0.0 {
            in_deactivate_mouse(input, window);
            return;
        }
    }

    if let Some(win) = window {
        in_activate_mouse(input, cvars, win);
    }
}

/// IN_Move
pub fn in_move(
    input: &mut InputState,
    cmd: &mut UserCmd,
    cvars: &CvarContext,
    viewangles: &mut Vec3,
    _active_app: bool,
    sensitivity_value: f32,
    m_yaw_value: f32,
    m_pitch_value: f32,
    m_forward_value: f32,
    m_side_value: f32,
    in_strafe_state: i32,
    lookstrafe_value: f32,
    freelook_value: f32,
    _in_speed_state: i32,
    _cl_run_value: f32,
    _frametime: f32,
    _cl_forwardspeed_value: f32,
    _cl_sidespeed_value: f32,
    _cl_upspeed_value: f32,
    _cl_pitchspeed_value: f32,
    _cl_yawspeed_value: f32,
) {
    in_mouse_move(
        input,
        cmd,
        cvars,
        viewangles,
        sensitivity_value,
        m_yaw_value,
        m_pitch_value,
        m_forward_value,
        m_side_value,
        in_strafe_state,
        lookstrafe_value,
        freelook_value,
    );
}

/// IN_Commands — no-op now that joystick support has been removed.
pub fn in_commands(_input: &mut InputState) {
}

/// IN_ClearStates
pub fn in_clear_states(input: &mut InputState) {
    input.mx_accum = 0.0;
    input.my_accum = 0.0;
    input.mouse_oldbuttonstate = 0;
}
