// Entry point — converted from myq2-original/win32/sys_win.c WinMain()
//
// The C original WinMain does:
//   1. Parse command line arguments
//   2. Scan for CD (legacy, no-op)
//   3. Qcommon_Init(argc, argv)
//   4. Main loop: compute frame delta via Sys_Milliseconds, call Qcommon_Frame(msec)
//
// With winit, we use an event-driven architecture where the event loop
// drives both input handling and game frame execution.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::ModifiersState;
use winit::window::WindowId;

use myq2_sys::sys_win;
use myq2_sys::platform_register;
use myq2_common::common;
use myq2_server::sv_init::{sv_register_client_callbacks, SvClientCallbacks};

/// Current keyboard modifiers state (for Alt+Enter detection).
static MODIFIERS: Mutex<ModifiersState> = Mutex::new(ModifiersState::empty());

/// Re-entrancy guard: prevents run_frame() from being called recursively
/// when sys_send_key_events pumps Win32 messages during cl_prep_refresh.
/// Without this, dispatched messages (WM_PAINT → RedrawRequested) would
/// trigger run_frame() which tries to acquire locks already held by the
/// loading code, causing a deadlock.
static IN_FRAME: AtomicBool = AtomicBool::new(false);

/// Application state for the winit event loop.
struct Q2App {
    oldtime: i32,
    initialized: bool,
    late_commands_executed: bool,
    frame_count: u32,
}

impl Q2App {
    fn new() -> Self {
        Self {
            oldtime: 0,
            initialized: false,
            late_commands_executed: false,
            frame_count: 0,
        }
    }

    /// Run one game frame if enough time has elapsed.
    fn run_frame(&mut self) {
        // Re-entrancy guard: if we're already inside run_frame (e.g., message pump
        // during cl_prep_refresh dispatched a WM_PAINT → RedrawRequested), skip.
        if IN_FRAME.swap(true, Ordering::SeqCst) {
            return;
        }

        // Use a drop guard to ALWAYS reset IN_FRAME, even if a panic occurs.
        // Without this, a panic in rendering would leave IN_FRAME=true permanently,
        // causing all subsequent frames to be skipped.
        struct InFrameGuard;
        impl Drop for InFrameGuard {
            fn drop(&mut self) {
                IN_FRAME.store(false, Ordering::SeqCst);
            }
        }
        let _guard = InFrameGuard;

        self.frame_count += 1;

        // Execute late commands (+map, +connect, etc.) on first frame
        if !self.late_commands_executed {
            myq2_common::common::qcommon_exec_late_commands();
            self.late_commands_executed = true;
        }

        // Poll chunked map loading (prevents "Not Responding" freeze)
        myq2_server::sv_init::with_server_context(|ctx| {
            use myq2_server::sv_init::MapLoadPhase;
            if !matches!(ctx.map_load_progress, MapLoadPhase::Idle | MapLoadPhase::Complete) {
                // Process one chunk of map loading
                let complete = myq2_server::sv_init::sv_spawn_server_tick(ctx);
                if complete {
                    // No explicit connect needed — CL_CheckForResend (in CL_Frame)
                    // auto-detects the local server is running and connects via loopback.
                }
            }
        });

        // Spin until at least 1ms has elapsed (matches C: do { ... } while (time < 1))
        let newtime;
        loop {
            let t = sys_win::sys_milliseconds();
            if t - self.oldtime >= 1 {
                newtime = t;
                break;
            }
            // Small yield to avoid busy-waiting
            std::hint::spin_loop();
        }

        let msec = newtime - self.oldtime;

        // Run one engine frame (includes rendering and buffer swap via SCR_UpdateScreen)
        // Catch panics to diagnose rendering crashes without killing the frame loop.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            common::qcommon_frame(msec);
        }));
        if let Err(e) = result {
            let msg = if let Some(s) = e.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = e.downcast_ref::<String>() {
                s.clone()
            } else {
                "unknown panic".to_string()
            };
            eprintln!("PANIC in qcommon_frame (frame {}): {}", self.frame_count, msg);
        }

        // Note: glimp_end_frame() is called from within scr_update_screen(), not here!
        // The original C code follows the same pattern.

        // Request next frame redraw (required for winit 0.30 continuous rendering)
        platform_register::with_platform(|s| {
            s.vk_imp.request_redraw();
        });

        self.oldtime = newtime;
    }
}

impl ApplicationHandler for Q2App {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        // Called when the application resumes (after suspend on mobile, or at startup)
        // Request initial redraw to kickstart rendering loop
        platform_register::with_platform(|s| {
            s.vk_imp.request_redraw();
        });
    }

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, _cause: StartCause) {
        // With winit 0.30 RedrawRequested pattern, we don't need ControlFlow::Poll
        // The redraw loop is driven by request_redraw() calls
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let time = sys_win::sys_milliseconds() as u32;
        sys_win::update_msg_time(time);

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }

            WindowEvent::KeyboardInput { event, .. } => {
                let modifiers = *MODIFIERS.lock().unwrap();
                sys_win::handle_keyboard_input(
                    event.physical_key,
                    event.state,
                    modifiers,
                    time,
                );
            }

            WindowEvent::ModifiersChanged(new_modifiers) => {
                *MODIFIERS.lock().unwrap() = new_modifiers.state();
            }

            WindowEvent::MouseInput { button, state, .. } => {
                sys_win::handle_mouse_button(button, state, time);
            }

            WindowEvent::MouseWheel { delta, .. } => {
                sys_win::handle_mouse_wheel(delta, time);
            }

            WindowEvent::Focused(focused) => {
                if focused {
                    sys_win::handle_focus_gained();
                } else {
                    sys_win::handle_focus_lost();
                }
            }

            WindowEvent::Occluded(occluded) => {
                if occluded {
                    sys_win::handle_minimized();
                } else {
                    sys_win::handle_restored();
                }
            }

            WindowEvent::RedrawRequested => {
                sys_win::handle_exposed();

                // Run game frame on redraw (winit 0.30 pattern)
                if self.initialized {
                    self.run_frame();
                }
            }

            WindowEvent::Moved(pos) => {
                sys_win::handle_moved(pos.x, pos.y);
            }

            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        // Handle raw mouse motion for FPS controls
        if let DeviceEvent::MouseMotion { delta } = event {
            sys_win::handle_mouse_motion(delta.0, delta.1);
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Called after all events have been processed
        if self.initialized {
            // Check if minimized or dedicated - sleep to avoid busy-waiting
            {
                let minimized = *sys_win::MINIMIZED.lock().unwrap();
                let dedicated = myq2_common::cvar::cvar_variable_value("dedicated") != 0.0;
                if minimized || dedicated {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            }

            // Update frame time
            sys_win::update_frame_time();
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        // Clean shutdown
        myq2_client::cl_main::cl_shutdown();
        common::qcommon_shutdown();
    }
}

fn main() {
    // Collect command-line arguments (equivalent to ParseCommandLine in C)
    let args: Vec<String> = std::env::args().collect();

    // Initialize platform layer (Sys_Init)
    sys_win::sys_init();

    // Register platform callbacks (GLimp/QGL) with the renderer crate.
    // This must happen before Qcommon_Init so that R_Init can use them.
    platform_register::platform_init();

    // Register video menu callbacks with the client's dispatch table.
    // SAFETY: single-threaded engine, must happen before any menu code runs.
    myq2_client::console::set_vid_menu_fns(myq2_client::console::VidMenuFunctions {
        vid_menu_init: myq2_sys::vid_menu::vid_menu_init_global,
        vid_menu_draw: myq2_sys::vid_menu::vid_menu_draw_global,
        vid_menu_key: myq2_sys::vid_menu::vid_menu_key_global,
    });

    // Register real renderer functions with the client's dispatch table.
    // SAFETY: single-threaded engine, must happen before any rendering code runs.
    myq2_client::console::set_renderer_fns(myq2_sys::renderer_bridge::make_renderer_fns());

    // Register system function pointers with the client's dispatch table.
    // SAFETY: single-threaded engine, must happen before any client code runs.
    myq2_client::console::set_system_fns(myq2_client::console::SystemFunctions {
        sys_send_key_events: || {
            // With winit, events are processed in the event loop callback
            // This is now a no-op
        },
        s_stop_all_sounds: || {
            myq2_client::cl_main::cl_s_stop_all_sounds();
        },
        s_start_local_sound: |name: &str| {
            myq2_client::cl_main::cl_s_start_local_sound(name);
        },
        sys_get_clipboard_data: sys_win::sys_get_clipboard_data,
    });

    // Register the OpenAL Soft sound backend with the client's sound system.
    myq2_client::cl_main::cl_register_sound_backend(
        Box::new(myq2_sys::snd_openal::OpenAlBackend::new()),
    );

    // Initialize all engine subsystems (Qcommon_Init)
    myq2_common::common::com_printf("main: About to call qcommon_init\n");
    common::qcommon_init(&args);
    myq2_common::common::com_printf("main: qcommon_init returned\n");

    // Initialize networking (register loopback + UDP packet dispatch)
    myq2_sys::net_udp::net_global_init();

    // Initialize server subsystem (SV_Init)
    myq2_common::common::com_printf("main: About to call sv_init\n");
    myq2_server::sv_init::sv_init_global_context();
    myq2_server::sv_init::with_server_context(|ctx| {
        myq2_server::sv_main::sv_init(ctx);
    });
    myq2_common::common::com_printf("main: sv_init returned\n");

    // Initialize client subsystem (CL_Init) - this will call vid_init which calls r_init
    myq2_common::common::com_printf("main: About to call cl_init\n");
    myq2_client::cl_main::cl_init();
    myq2_common::common::com_printf("main: cl_init returned\n");

    // Register frame callbacks so qcommon_frame calls sv_frame and cl_frame
    myq2_common::common::com_printf("main: Registering frame callbacks\n");
    myq2_common::common::register_frame_callbacks(myq2_common::common::FrameCallbacks {
        sv_frame: myq2_server::sv_init::sv_frame_global,
        cl_frame: myq2_client::cl_main::cl_frame,
    });
    myq2_common::common::com_printf("main: Frame callbacks registered\n");

    // Register client callbacks so the server can call cl_drop / scr_begin_loading_plaque
    sv_register_client_callbacks(SvClientCallbacks {
        cl_drop: myq2_client::cl_main::cl_drop,
        scr_begin_loading_plaque: || {
            // Use cl_main version (CL/CLS/SCR_STATE locks) instead of
            // console version (CONSOLE_STATE lock) to avoid deadlock:
            // console version holds CONSOLE_STATE, then com_printf calls
            // con_print which also needs CONSOLE_STATE → deadlock.
            myq2_client::cl_main::scr_begin_loading_plaque();
        },
    });

    // Take the event loop from GlImpContext for the main loop
    // (cl_init has already created the window via vid_init)
    let event_loop: EventLoop<()> = platform_register::with_platform(|s| {
        s.vk_imp.take_event_loop()
    }).expect("Event loop not available");

    // Create application state and record initial time
    let mut app = Q2App::new();
    app.oldtime = sys_win::sys_milliseconds();
    app.initialized = true;
    app.late_commands_executed = false; // Will execute in first frame

    // Run the winit event loop - this never returns
    // Note: ControlFlow::Poll is set inside about_to_wait() callback
    event_loop.run_app(&mut app).expect("Event loop error");
}
