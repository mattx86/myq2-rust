// platform_register.rs — Register myq2-sys platform callbacks with myq2-renderer
//
// Stores GlImpContext and QglState in a global Mutex so the callback
// closures registered with myq2_renderer::platform can access them.

use std::sync::Mutex;

use crate::glw_imp::GlImpContext;
use crate::qvk_win::QglState;
use crate::in_win::INPUT_STATE;
use crate::vid_dll::VidState;
use myq2_renderer::platform::{self, PlatformDispatch};

/// Shared platform state accessible from dispatch callbacks.
pub struct SharedPlatformState {
    pub vk_imp: GlImpContext,
    pub qvk: QglState,
    pub vid: VidState,
}

// SAFETY: The Quake 2 engine is single-threaded. winit objects (Window, etc.)
// are not Send, but we only ever access them from the main thread. The Mutex is
// used for interior mutability, not cross-thread synchronization.
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

/// Initialize the shared platform state and register all dispatch callbacks
/// with the renderer crate.
pub fn platform_init() {
    // Store initial state
    *PLATFORM_STATE.lock().unwrap() = Some(SharedPlatformState {
        vk_imp: GlImpContext::default(),
        qvk: QglState::default(),
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
        qvk_init: Some(Box::new(|driver: &str| {
            with_platform(|s| s.qvk.qvk_init(driver))
        })),
        qvk_shutdown: Some(Box::new(|| {
            with_platform(|s| s.qvk.qvk_shutdown())
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
            myq2_common::cvar::with_cvar_ctx(|cvars| {
                with_platform(|s| {
                    crate::vid_dll::vid_init(&mut s.vid, cvars, 0, 0);
                });
            });
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
                    );
                });
            });
        })),
        r_set_palette: Some(Box::new(|_palette: Option<&[u8]>| {
            // R_SetPalette handled by renderer directly
        })),
        in_init: Some(Box::new(|| {
            myq2_common::cvar::with_cvar_ctx(|cvars| {
                let mut input = INPUT_STATE.lock().unwrap();
                crate::in_win::in_init(&mut input, cvars);
            });
        })),
        in_shutdown: Some(Box::new(|| {
            with_platform(|s| {
                let mut input = INPUT_STATE.lock().unwrap();
                let window = s.vk_imp.window();
                crate::in_win::in_shutdown(&mut input, window);
            });
        })),
        in_commands: Some(Box::new(|| {
            let mut input = INPUT_STATE.lock().unwrap();
            crate::in_win::in_commands(&mut input);
        })),
        in_frame: Some(Box::new(|| {
            myq2_common::cvar::with_cvar_ctx(|cvars| {
                with_platform(|s| {
                    let mut input = INPUT_STATE.lock().unwrap();
                    let window = s.vk_imp.window();
                    let vid_fullscreen = myq2_common::cvar::cvar_variable_value("vid_fullscreen") as f32;
                    crate::in_win::in_frame(
                        &mut input, cvars, window,
                        true, false, vid_fullscreen,
                    );
                });
            });
        })),
        sys_send_key_events: Some(Box::new(|| {
            // With winit, events are processed in the main event loop callback
            // This is now a no-op as event processing happens in main.rs
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
    });
}
