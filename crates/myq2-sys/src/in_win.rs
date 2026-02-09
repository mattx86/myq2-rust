// in_win.rs -- mouse and input handling
// Converted from: myq2-original/win32/in_win.c
//
// Uses winit for mouse capture (cursor grab mode) instead of Win32/SDL APIs.

use std::sync::Arc;
use std::sync::Mutex;

use winit::window::{Window, CursorGrabMode};

use myq2_common::cvar::CvarContext;
use myq2_common::q_shared::*;

/// Global input state, accessible from the event pump in sys_win.rs.
pub static INPUT_STATE: Mutex<InputState> = Mutex::new(InputState::new_const());

/// All input state, replacing the C globals.
pub struct InputState {
    // Mouse variables
    pub mlooking: bool,
    pub mouse_buttons: i32,
    pub mouse_oldbuttonstate: i32,
    pub mouse_x: i32,
    pub mouse_y: i32,
    pub old_mouse_x: i32,
    pub old_mouse_y: i32,
    pub mx_accum: i32,
    pub my_accum: i32,
    pub mouseactive: bool,
    pub mouseinitialized: bool,
    pub window_center_x: i32,
    pub window_center_y: i32,

    // App active
    pub in_appactive: bool,

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
            mouse_x: 0,
            mouse_y: 0,
            old_mouse_x: 0,
            old_mouse_y: 0,
            mx_accum: 0,
            my_accum: 0,
            mouseactive: false,
            mouseinitialized: false,
            window_center_x: 0,
            window_center_y: 0,
            in_appactive: false,
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
/// Enables winit cursor grab mode for FPS-style mouse capture.
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

    // Try to confine the cursor first, then try locked mode if available
    // Confined keeps cursor in window bounds, Locked hides and reports relative motion
    if window.set_cursor_grab(CursorGrabMode::Confined).is_err() {
        // Fallback: some platforms support Locked but not Confined
        let _ = window.set_cursor_grab(CursorGrabMode::Locked);
    }

    window.set_cursor_visible(false); // Hide cursor during gameplay
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

    // Consume accumulated relative motion from winit events
    let mx = input.mx_accum;
    let my = input.my_accum;
    input.mx_accum = 0;
    input.my_accum = 0;

    let m_filter_value = input
        .m_filter
        .and_then(|idx| cvars.cvar_vars.get(idx))
        .map(|cv| cv.value)
        .unwrap_or(0.0);

    if m_filter_value != 0.0 {
        input.mouse_x = ((mx + input.old_mouse_x) as f32 * 0.5) as i32;
        input.mouse_y = ((my + input.old_mouse_y) as f32 * 0.5) as i32;
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
    input.mx_accum = 0;
    input.my_accum = 0;
    input.mouse_oldbuttonstate = 0;
}
