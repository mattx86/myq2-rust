// platform_register.rs — Register myq2-sys platform callbacks with myq2-renderer
//
// Stores GlImpContext and VidState in a global Mutex so the callback
// closures registered with myq2_renderer::platform can access them.

use std::sync::Mutex;

use crate::glw_imp::GlImpContext;
use crate::in_win::INPUT_STATE;
use crate::vid_dll::VidState;
use myq2_renderer::platform::{self, PlatformDispatch};

/// Shared platform state accessible from dispatch callbacks.
pub struct SharedPlatformState {
    pub vk_imp: GlImpContext,
    pub vid: VidState,
}

// SAFETY: winit objects (Window, etc.) are not Send, but they are only accessed
// from the main thread. The Mutex provides interior mutability, not cross-thread
// synchronization.
unsafe impl Send for SharedPlatformState {}

pub static PLATFORM_STATE: Mutex<Option<SharedPlatformState>> = Mutex::new(None);

/// Access the shared platform state with a closure.
pub fn with_platform<F, R>(f: F) -> R
where
    F: FnOnce(&mut SharedPlatformState) -> R,
{
    let mut guard = PLATFORM_STATE.lock().unwrap();
    let state = guard.as_mut().expect("platform not initialized");
    f(state)
}

/// Direct access to glimp_init when already holding PLATFORM_STATE lock.
/// Used to avoid nested mutex deadlock during initialization.
pub fn glimp_init_direct(vk_imp: &mut GlImpContext, hinstance: usize, hwnd: usize) -> bool {
    vk_imp.glimp_init(hinstance, hwnd)
}

/// Direct access to glimp_set_mode when already holding PLATFORM_STATE lock.
/// Used to avoid nested mutex deadlock during initialization.
pub fn glimp_set_mode_direct(vk_imp: &mut GlImpContext, width: &mut i32, height: &mut i32, mode: i32, fullscreen: bool) -> i32 {
    let result = vk_imp.glimp_set_mode(width, height, mode, fullscreen);
    match result {
        crate::glw_imp::RsErr::Ok => 0,
        crate::glw_imp::RsErr::InvalidFullscreen => 1,
        crate::glw_imp::RsErr::InvalidMode => 2,
        crate::glw_imp::RsErr::Unknown => 3,
    }
}

/// Initialize the shared platform state and register all dispatch callbacks
/// with the renderer crate.
pub fn platform_init() {
    // Store initial state
    *PLATFORM_STATE.lock().unwrap() = Some(SharedPlatformState {
        vk_imp: GlImpContext::default(),
        vid: VidState::default(),
    });

    // Register callbacks
    platform::platform_register(PlatformDispatch {
        glimp_init: Some(Box::new(|hinstance, hwnd| {
            with_platform(|s| s.vk_imp.glimp_init(hinstance, hwnd))
        })),
        glimp_shutdown: Some(Box::new(|| {
            with_platform(|s| s.vk_imp.glimp_shutdown())
        })),
        glimp_begin_frame: Some(Box::new(|camera_separation| {
            with_platform(|s| s.vk_imp.glimp_begin_frame(camera_separation))
        })),
        glimp_end_frame: Some(Box::new(|| {
            with_platform(|s| s.vk_imp.glimp_end_frame())
        })),
        glimp_set_mode: Some(Box::new(|width, height, mode, fullscreen| {
            let result = with_platform(|s| {
                s.vk_imp.glimp_set_mode(width, height, mode as i32, fullscreen)
            });
            // Map RsErr to i32
            match result {
                crate::glw_imp::RsErr::Ok => 0,
                crate::glw_imp::RsErr::InvalidFullscreen => 1,
                crate::glw_imp::RsErr::InvalidMode => 2,
                crate::glw_imp::RsErr::Unknown => 3,
            }
        })),
        vid_menu_init: Some(Box::new(|| {
            // vid_menu_init requires CvarContext and VidDef — for now just log
            myq2_common::common::com_printf("VID_MenuInit (platform dispatch)\n");
        })),
        update_gamma_ramp: Some(Box::new(|| {
            let vid_gamma = myq2_common::cvar::cvar_variable_value("vid_gamma") as f32;
            with_platform(|s| s.vk_imp.update_gamma_ramp(vid_gamma))
        })),
    });

    // Register client platform callbacks
    use myq2_client::platform::{ClientPlatformDispatch, client_platform_register};

    client_platform_register(ClientPlatformDispatch {
        vid_init: Some(Box::new(|| {
            // Temporarily extract platform callbacks to avoid holding PLATFORM_STATE during initialization
            // This prevents nested mutex deadlocks when vid_init→r_init→r_set_mode calls glimp_* functions
            myq2_common::common::com_printf("vid_init: Starting\n");

            // Phase 1: Call glimp_init while holding PLATFORM_STATE lock
            with_platform(|s| {
                myq2_common::common::com_printf("vid_init: Calling glimp_init_direct\n");
                glimp_init_direct(&mut s.vk_imp, 0, 0);
                myq2_common::common::com_printf("vid_init: glimp_init_direct completed\n");
            });

            // Phase 2: Extract vid_state first without holding any locks
            let mut vid_state = {
                let mut guard = PLATFORM_STATE.lock().unwrap();
                let s = guard.as_mut().expect("platform not initialized");
                std::mem::take(&mut s.vid)
            };

            // Phase 3: Call vid_dll::vid_init_wrapper WITHOUT holding CVAR_CTX
            // CRITICAL: vid_init→vid_check_changes→vid_load_refresh→r_init→r_register
            // will internally call cvar_get which acquires CVAR_CTX, so we must NOT
            // hold it here or we'll deadlock
            crate::vid_dll::vid_init_wrapper(&mut vid_state, 0, 0);

            // Phase 4: Put vid_state back
            {
                let mut guard = PLATFORM_STATE.lock().unwrap();
                let s = guard.as_mut().expect("platform not initialized");
                s.vid = vid_state;
            }

            myq2_common::common::com_printf("vid_init: Completed\n");
        })),
        vid_shutdown: Some(Box::new(|| {
            with_platform(|s| {
                crate::vid_dll::vid_shutdown(&mut s.vid);
            });
        })),
        vid_check_changes: Some(Box::new(|| {
            myq2_common::cvar::with_cvar_ctx(|cvars| {
                with_platform(|s| {
                    let mut disable_screen = false;
                    let mut force_refdef = false;
                    let mut refresh_prepped = false;
                    crate::vid_dll::vid_check_changes(
                        &mut s.vid, cvars,
                        &mut disable_screen, &mut force_refdef, &mut refresh_prepped,
                        0, 0, 0,
                        Some(&s.vk_imp),
                    );
                });
            });
        })),
        r_set_palette: Some(Box::new(|_palette: Option<&[u8]>| {
            // R_SetPalette handled by renderer directly
        })),
        in_init: Some(Box::new(|| {
            myq2_common::cvar::with_cvar_ctx(|cvars| {
                let mut input = INPUT_STATE.lock()
                    .unwrap_or_else(|e| e.into_inner());
                crate::in_win::in_init(&mut input, cvars);
            });
        })),
        in_shutdown: Some(Box::new(|| {
            with_platform(|s| {
                let mut input = INPUT_STATE.lock()
                    .unwrap_or_else(|e| e.into_inner());
                let window = s.vk_imp.window();
                crate::in_win::in_shutdown(&mut input, window);
            });
        })),
        in_commands: Some(Box::new(|| {
            let mut input = INPUT_STATE.lock()
                .unwrap_or_else(|e| e.into_inner());
            crate::in_win::in_commands(&mut input);
        })),
        in_frame: Some(Box::new(|| {
            // Read actual client state BEFORE locking other resources
            // SAFETY: CL/CLS initialized at startup, accessed from main thread
            let (refresh_prepped, key_dest_is_console_or_menu) = unsafe {
                use myq2_client::console::{CL, CLS};
                use myq2_client::client::KeyDest;
                let rp = CL.refresh_prepped;
                let kd = matches!(CLS.key_dest, KeyDest::Console | KeyDest::Menu);
                (rp, kd)
            };

            myq2_common::cvar::with_cvar_ctx(|cvars| {
                // Get vid_fullscreen from cvars BEFORE locking PLATFORM_STATE
                // to avoid nested lock attempts
                let vid_fullscreen = cvars.variable_value("vid_fullscreen");

                with_platform(|s| {
                    let mut input = INPUT_STATE.lock()
                        .unwrap_or_else(|e| e.into_inner());
                    let window = s.vk_imp.window();
                    crate::in_win::in_frame(
                        &mut input, cvars, window,
                        refresh_prepped, key_dest_is_console_or_menu, vid_fullscreen,
                    );
                });
            });
        })),
        in_move: Some(Box::new(|cmd, viewangles, in_strafe_state| {
            myq2_common::cvar::with_cvar_ctx(|cvars| {
                let mut input = INPUT_STATE.lock()
                    .unwrap_or_else(|e| e.into_inner());
                let sensitivity = cvars.variable_value("sensitivity") as f32;
                let m_yaw = cvars.variable_value("m_yaw") as f32;
                let m_pitch = cvars.variable_value("m_pitch") as f32;
                let m_forward = cvars.variable_value("m_forward") as f32;
                let m_side = cvars.variable_value("m_side") as f32;
                let lookstrafe = cvars.variable_value("lookstrafe") as f32;
                let freelook = cvars.variable_value("freelook") as f32;
                crate::in_win::in_move(
                    &mut input, cmd, cvars, viewangles,
                    true, // active_app
                    sensitivity, m_yaw, m_pitch, m_forward, m_side,
                    in_strafe_state, lookstrafe, freelook,
                    0,    // in_speed_state (unused)
                    0.0,  // cl_run_value (unused)
                    0.0,  // frametime (unused)
                    0.0,  // cl_forwardspeed_value (unused)
                    0.0,  // cl_sidespeed_value (unused)
                    0.0,  // cl_upspeed_value (unused)
                    0.0,  // cl_pitchspeed_value (unused)
                    0.0,  // cl_yawspeed_value (unused)
                );
            });
        })),
        sys_send_key_events: Some(Box::new(|| {
            // Pump the Win32 message queue so the window stays responsive
            // during long synchronous operations (e.g., cl_prep_refresh asset loading).
            // This matches the original C Sys_SendKeyEvents which calls PeekMessage/DispatchMessage.
            #[cfg(target_os = "windows")]
            {
                use windows_sys::Win32::UI::WindowsAndMessaging::*;
                // SAFETY: We're on the main thread (same thread that owns the window).
                // PeekMessage/TranslateMessage/DispatchMessage are safe to call here.
                unsafe {
                    let mut msg = std::mem::zeroed::<MSG>();
                    while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            }
        })),
        sys_app_activate: Some(Box::new(|| {
            // App activation handled by winit event pump
        })),
        net_config: Some(Box::new(|multiplayer: bool| {
            myq2_common::net::net_config(multiplayer);
        })),
        sv_shutdown: Some(Box::new(|msg: &str, _reconnect: bool| {
            myq2_common::common::com_printf(&format!("SV_Shutdown: {}\n", msg));
        })),
        sv_start_map: Some(Box::new(|mapname: &str| {
            // Check if already loading a map
            let already_loading = myq2_server::sv_init::with_server_context(|server_ctx| {
                !matches!(server_ctx.map_load_progress,
                    myq2_server::sv_init::MapLoadPhase::Idle |
                    myq2_server::sv_init::MapLoadPhase::Complete)
            }).unwrap_or(false);

            if already_loading {
                myq2_common::common::com_printf("Map load already in progress\n");
                return;
            }

            // Directly call the server's map loading function
            myq2_server::sv_init::with_server_context(|ctx| {
                // Create a cmd_argv closure that returns our map name
                let cmd_argv = |idx: usize| -> String {
                    match idx {
                        0 => "map".to_string(),
                        1 => mapname.to_string(),
                        _ => String::new(),
                    }
                };
                myq2_server::sv_ccmds::sv_map_f(ctx, 2, &cmd_argv);
            });
        })),
        con_print: Some(Box::new(|text: &str| {
            myq2_client::console::con_print(text);
        })),
    });
}
