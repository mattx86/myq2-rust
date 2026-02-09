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

use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::ModifiersState;
use winit::window::WindowId;

use myq2_sys::sys_win;
use myq2_sys::platform_register;
use myq2_common::common;
use myq2_server::sv_init::{sv_register_client_callbacks, SvClientCallbacks};

/// Current keyboard modifiers state (for Alt+Enter detection).
static MODIFIERS: Mutex<ModifiersState> = Mutex::new(ModifiersState::empty());

/// Application state for the winit event loop.
struct Q2App {
    oldtime: i32,
    initialized: bool,
}

impl Q2App {
    fn new() -> Self {
        Self {
            oldtime: 0,
            initialized: false,
        }
    }

    /// Run one game frame if enough time has elapsed.
    fn run_frame(&mut self) {
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

        // Run one engine frame
        common::qcommon_frame(msec);

        // Swap buffers via platform dispatch
        platform_register::with_platform(|s| {
            s.vk_imp.glimp_end_frame();
        });

        self.oldtime = newtime;
    }
}

impl ApplicationHandler for Q2App {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        // Called when the application resumes (after suspend on mobile, or at startup)
    }

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        if let StartCause::Poll = cause {
            // Poll mode - run game frame every iteration
        }
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
        // Called after all events have been processed - run game frame here
        if self.initialized {
            // Check if minimized or dedicated - sleep to avoid busy-waiting
            {
                let minimized = *sys_win::MINIMIZED.lock().unwrap();
                let dedicated = myq2_common::cvar::cvar_variable_value("dedicated") != 0.0;
                if minimized || dedicated {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            }

            self.run_frame();

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
    unsafe {
        myq2_client::console::VID_MENU_FNS = myq2_client::console::VidMenuFunctions {
            vid_menu_init: myq2_sys::vid_menu::vid_menu_init_global,
            vid_menu_draw: myq2_sys::vid_menu::vid_menu_draw_global,
            vid_menu_key: myq2_sys::vid_menu::vid_menu_key_global,
        };
    }

    // Register real renderer functions with the client's dispatch table.
    // SAFETY: single-threaded engine, must happen before any rendering code runs.
    unsafe {
        myq2_client::console::RENDERER_FNS = myq2_sys::renderer_bridge::make_renderer_fns();
    }

    // Register system function pointers with the client's dispatch table.
    // SAFETY: single-threaded engine, must happen before any client code runs.
    unsafe {
        myq2_client::console::SYSTEM_FNS = myq2_client::console::SystemFunctions {
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
        };
    }

    // Register the OpenAL Soft sound backend with the client's sound system.
    myq2_client::cl_main::cl_register_sound_backend(
        Box::new(myq2_sys::snd_openal::OpenAlBackend::new()),
    );

    // Initialize all engine subsystems (Qcommon_Init)
    common::qcommon_init(&args);

    // Register client callbacks so the server can call cl_drop / scr_begin_loading_plaque
    sv_register_client_callbacks(SvClientCallbacks {
        cl_drop: myq2_client::cl_main::cl_drop,
        scr_begin_loading_plaque: || {
            myq2_client::console::scr_begin_loading_plaque();
        },
    });

    // Create the window via the platform dispatch.
    // Read width/height/fullscreen from cvars and call vid_create_window
    // through the shared platform state.
    {
        let vid_fullscreen = myq2_common::cvar::cvar_variable_value("vid_fullscreen") != 0.0;
        let vid_mode = myq2_common::cvar::cvar_variable_value("vid_mode") as i32;
        let (w, h) = match vid_mode {
            0 => (320, 240), 1 => (400, 300), 2 => (512, 384), 3 => (640, 480),
            4 => (800, 600), 5 => (960, 720), 6 => (1024, 768), 7 => (1152, 864),
            8 => (1280, 960), 9 => (1600, 1200), 10 => (2048, 1536),
            _ => (800, 600),
        };
        platform_register::with_platform(|s| {
            s.vk_imp.vid_create_window(w, h, vid_fullscreen);
        });
    }

    // Take the event loop from GlImpContext for the main loop
    let event_loop: EventLoop<()> = platform_register::with_platform(|s| {
        s.vk_imp.take_event_loop()
    }).expect("Event loop not available");

    // Set control flow to Poll so we run game frames continuously
    event_loop.set_control_flow(ControlFlow::Poll);

    // Create application state and record initial time
    let mut app = Q2App::new();
    app.oldtime = sys_win::sys_milliseconds();
    app.initialized = true;

    // Run the winit event loop - this never returns
    event_loop.run_app(&mut app).expect("Event loop error");
}
