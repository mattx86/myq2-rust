// sys_win.rs — Main system/platform layer
// Converted from: myq2-original/win32/sys_win.c
//
// Uses winit for window/input instead of SDL3.

use std::process;
use std::sync::Mutex;

use winit::event::{ElementState, MouseButton, MouseScrollDelta};
use winit::keyboard::{KeyCode, PhysicalKey};

use myq2_common::common;
use myq2_common::cvar::with_cvar_ctx;
use myq2_client::cl_main;
use myq2_client::keys;
use crate::conproc;

// ============================================================
// Constants
// ============================================================

const MINIMUM_WIN_MEMORY: usize = 0x0a00000;
const MAXIMUM_WIN_MEMORY: usize = 0x1000000;

const MAX_NUM_ARGVS: usize = 128;

// ============================================================
// Global state
// ============================================================

/// Windows 95 flag (legacy, always false on modern systems).
pub static S_WIN95: Mutex<bool> = Mutex::new(false);

pub static START_TIME: Mutex<i32> = Mutex::new(0);
pub static ACTIVE_APP: Mutex<i32> = Mutex::new(0);
pub static MINIMIZED: Mutex<bool> = Mutex::new(false);

/// System message time — updated during message pump.
pub static SYS_MSG_TIME: Mutex<u32> = Mutex::new(0);

/// System frame time — updated each frame.
pub static SYS_FRAME_TIME: Mutex<u32> = Mutex::new(0);

/// Parsed command-line arguments.
pub static CMD_ARGS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Check whether the "dedicated" cvar is set to a non-zero value.
///
/// Original: `dedicated && dedicated->value`
fn is_dedicated() -> bool {
    with_cvar_ctx(|ctx| ctx.variable_value("dedicated") != 0.0).unwrap_or(false)
}

// ============================================================
// SYSTEM IO
// ============================================================

/// Fatal error handler. Shuts down client, engine, and exits.
///
/// Original: `void Sys_Error(char *error, ...)`
///
/// The original called CL_Shutdown(), Qcommon_Shutdown(), showed a MessageBox,
/// closed qwclsemaphore, and called DeinitConProc before exit(1).
/// CL_Shutdown and Qcommon_Shutdown will be wired in once those modules are converted.
pub fn sys_error(error: &str) -> ! {
    cl_main::cl_shutdown();
    common::qcommon_shutdown();

    // Print to stderr (replaces MessageBox for now — this is the lowest-level error handler)
    eprintln!("Error: {}", error);

    // Shut down QHOST hooks if necessary
    conproc::deinit_con_proc();

    process::exit(1);
}

/// Clean shutdown.
///
/// Original: `void Sys_Quit(void)`
///
/// The original called timeEndPeriod(1), CL_Shutdown(), Qcommon_Shutdown(),
/// CloseHandle(qwclsemaphore), FreeConsole() if dedicated, and DeinitConProc().
/// timeEndPeriod is unnecessary (Rust timing uses Instant, not timeGetTime).
/// CL_Shutdown and Qcommon_Shutdown will be wired in once those modules are converted.
pub fn sys_quit() -> ! {
    cl_main::cl_shutdown();
    common::qcommon_shutdown();

    // Original: if (dedicated && dedicated->value) FreeConsole();
    // FreeConsole is a Win32 API for detaching from the console. On modern systems
    // with winit, this is unnecessary — the process simply exits. Kept as a no-op
    // for documentation fidelity.
    if is_dedicated() {
        // No-op: Rust process cleanup handles console detachment.
    }

    // Shut down QHOST hooks if necessary
    conproc::deinit_con_proc();

    process::exit(0);
}

/// Display last Win32 error.
///
/// Original: `void WinError(void)`
///
/// The original used FormatMessage + MessageBox to display GetLastError().
/// We use std::io::Error::last_os_error() which calls FormatMessage internally
/// on Windows, and print to stderr (since this is the lowest-level error handler).
pub fn win_error() {
    let err = std::io::Error::last_os_error();
    eprintln!("WinError: {}", err);
}

// ============================================================
// CD Scanning (legacy)
// ============================================================

/// Scan drives for the Quake 2 CD.
///
/// Original: `char *Sys_ScanForCD(void)`
pub fn sys_scan_for_cd() -> Option<String> {
    // Legacy CD-ROM scanning — no-op in modern builds.
    // The original scanned drives c: through z: for install\data\quake2.exe
    // on CD-ROM drives. CD audio support has been removed.
    None
}

/// Copy protection check (legacy, no-op).
///
/// Original: `void Sys_CopyProtect(void)`
pub fn sys_copy_protect() {
    // #ifndef DEMO
    // In original: calls Sys_ScanForCD and errors if not found.
    // Intentionally a no-op in modern builds.
}

// ============================================================
// System Init
// ============================================================

/// Initialize system layer.
///
/// Original: `void Sys_Init(void)`
pub fn sys_init() {
    // Original called timeBeginPeriod(1) for 1ms timer resolution.
    // Not needed — Rust's std::time::Instant uses QueryPerformanceCounter on Windows,
    // which provides sub-microsecond resolution without timeBeginPeriod.

    // Original checked Windows version via GetVersionEx:
    //   - Required major version >= 4 (Windows NT 4.0 / Windows 95)
    //   - Rejected Win32s
    //   - Set s_win95 flag for VER_PLATFORM_WIN32_WINDOWS
    // Modern Windows always satisfies these checks. s_win95 is always false.
    {
        let mut win95 = S_WIN95.lock().unwrap();
        *win95 = false;
    }

    // Initialize the timing system (ensures the Instant base is set)
    let _ = crate::q_shwin::sys_milliseconds();

    // Original: if (dedicated->value) { AllocConsole(); hinput = GetStdHandle(...); ... InitConProc(argc, argv); }
    // In the Rust port, stdin/stdout are always available (no AllocConsole needed).
    // We still initialize QHOST hooks if running as a dedicated server.
    if is_dedicated() {
        let args = CMD_ARGS.lock().unwrap().clone();
        conproc::init_con_proc(&args);
    }
}

// ============================================================
// Dedicated Console I/O
// ============================================================

/// Console text buffer for dedicated server input.
struct ConsoleBuffer {
    text: [u8; 256],
    len: usize,
}

static CONSOLE_BUF: Mutex<ConsoleBuffer> = Mutex::new(ConsoleBuffer {
    text: [0u8; 256],
    len: 0,
});

/// Read a line from the dedicated server console (non-blocking).
///
/// Original: `char *Sys_ConsoleInput(void)`
pub fn sys_console_input() -> Option<String> {
    if !is_dedicated() {
        return None;
    }

    // Original used ReadConsoleInput to process KEY_EVENT records one at a time,
    // building up a line buffer with backspace support and echoing to houtput.
    //
    // In the Rust port, we use the Windows console input API directly to match
    // the original non-blocking behavior. On non-Windows, we return None.
    #[cfg(target_os = "windows")]
    {
        extern "system" {
            fn GetStdHandle(nStdHandle: u32) -> *mut std::ffi::c_void;
            fn GetNumberOfConsoleInputEvents(
                hConsoleInput: *mut std::ffi::c_void,
                lpNumberOfEvents: *mut u32,
            ) -> i32;
            fn ReadConsoleInputA(
                hConsoleInput: *mut std::ffi::c_void,
                lpBuffer: *mut ConsoleInputRecord,
                nLength: u32,
                lpNumberOfEventsRead: *mut u32,
            ) -> i32;
            fn WriteConsoleA(
                hConsoleOutput: *mut std::ffi::c_void,
                lpBuffer: *const u8,
                nNumberOfCharsToWrite: u32,
                lpNumberOfCharsWritten: *mut u32,
                lpReserved: *mut std::ffi::c_void,
            ) -> i32;
        }

        const STD_INPUT_HANDLE: u32 = 0xFFFF_FFF6; // (DWORD)-10
        const STD_OUTPUT_HANDLE: u32 = 0xFFFF_FFF5; // (DWORD)-11
        const KEY_EVENT_TYPE: u16 = 0x0001;

        #[repr(C)]
        struct KeyEventRecord {
            b_key_down: i32,
            w_repeat_count: u16,
            w_virtual_key_code: u16,
            w_virtual_scan_code: u16,
            u_char: u16, // union — AsciiChar is low byte
            dw_control_key_state: u32,
        }

        #[repr(C)]
        struct ConsoleInputRecord {
            event_type: u16,
            _padding: u16,
            event: KeyEventRecord,
        }

        let mut buf = CONSOLE_BUF.lock().unwrap();

        // SAFETY: Standard Win32 console APIs, matching the original C code exactly.
        // GetStdHandle returns process-wide handles; ReadConsoleInputA and
        // WriteConsoleA are safe for single-threaded console I/O.
        unsafe {
            let hinput = GetStdHandle(STD_INPUT_HANDLE);
            let houtput = GetStdHandle(STD_OUTPUT_HANDLE);

            loop {
                let mut num_events: u32 = 0;
                if GetNumberOfConsoleInputEvents(hinput, &mut num_events) == 0 {
                    break;
                }
                if num_events == 0 {
                    break;
                }

                let mut rec: ConsoleInputRecord = std::mem::zeroed();
                let mut num_read: u32 = 0;
                if ReadConsoleInputA(hinput, &mut rec, 1, &mut num_read) == 0 {
                    break;
                }
                if num_read != 1 {
                    break;
                }

                // Only process key-up events (matching original C code)
                if rec.event_type == KEY_EVENT_TYPE && rec.event.b_key_down == 0 {
                    let ch = (rec.event.u_char & 0xFF) as u8;
                    let mut dummy: u32 = 0;

                    match ch {
                        b'\r' => {
                            WriteConsoleA(houtput, b"\r\n".as_ptr(), 2, &mut dummy, std::ptr::null_mut());
                            if buf.len > 0 {
                                let len = buf.len;
                                buf.text[len] = 0;
                                let result = std::str::from_utf8(&buf.text[..len])
                                    .unwrap_or("")
                                    .to_string();
                                buf.len = 0;
                                return Some(result);
                            }
                        }
                        b'\x08' => {
                            // Backspace
                            if buf.len > 0 {
                                buf.len -= 1;
                                WriteConsoleA(houtput, b"\x08 \x08".as_ptr(), 3, &mut dummy, std::ptr::null_mut());
                            }
                        }
                        _ => {
                            if ch >= b' ' && buf.len < 254 {
                                WriteConsoleA(houtput, &ch as *const u8, 1, &mut dummy, std::ptr::null_mut());
                                let idx = buf.len;
                                buf.text[idx] = ch;
                                buf.len += 1;
                            }
                        }
                    }
                }
            }
        }
        None
    }

    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

/// Print text to the dedicated server console.
///
/// Original: `void Sys_ConsoleOutput(char *string)`
pub fn sys_console_output(string: &str) {
    if !is_dedicated() {
        return;
    }

    use std::io::Write;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();

    let buf = CONSOLE_BUF.lock().unwrap();
    let textlen = buf.len;

    // If there's a partially typed input line, clear it first
    if textlen > 0 {
        // Overwrite the current input with spaces, then carriage-return
        let _ = handle.write_all(b"\r");
        for _ in 0..textlen {
            let _ = handle.write_all(b" ");
        }
        let _ = handle.write_all(b"\r");
    }

    // Write the output string
    let _ = handle.write_all(string.as_bytes());

    // Redraw the current input line if any
    if textlen > 0 {
        let _ = handle.write_all(&buf.text[..textlen]);
    }

    let _ = handle.flush();
}

// ============================================================
// winit Event Processing
// ============================================================

/// Process a winit keyboard event.
pub fn handle_keyboard_input(
    key_code: PhysicalKey,
    state: ElementState,
    modifiers: winit::keyboard::ModifiersState,
    time: u32,
) {
    let pressed = state == ElementState::Pressed;

    // Alt+Enter fullscreen toggle (mirrors WM_SYSKEYDOWN VK_RETURN)
    if let PhysicalKey::Code(KeyCode::Enter) = key_code {
        if pressed && modifiers.alt_key() {
            let current = myq2_common::cvar::cvar_variable_value("vid_fullscreen");
            let new_val = if current != 0.0 { "0" } else { "1" };
            myq2_common::cvar::cvar_set("vid_fullscreen", new_val);
            return;
        }
    }

    if let PhysicalKey::Code(code) = key_code {
        if let Some(q2key) = winit_keycode_to_q2(code) {
            keys::key_event(q2key, pressed, time);
        }
    }
}

/// Process a winit mouse button event.
pub fn handle_mouse_button(button: MouseButton, state: ElementState, time: u32) {
    let pressed = state == ElementState::Pressed;
    if let Some(q2key) = winit_mouse_button_to_q2(button) {
        keys::key_event(q2key, pressed, time);
    }
}

/// Process a winit mouse motion event.
pub fn handle_mouse_motion(delta_x: f64, delta_y: f64) {
    let mut input = crate::in_win::INPUT_STATE.lock().unwrap();
    input.mx_accum += delta_x as i32;
    input.my_accum += delta_y as i32;
}

/// Process a winit mouse wheel event.
pub fn handle_mouse_wheel(delta: MouseScrollDelta, time: u32) {
    let y = match delta {
        MouseScrollDelta::LineDelta(_, y) => y,
        MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 10.0, // Approximate
    };

    // Wheel up/down mapped to MOUSE4/MOUSE5 (matches Q2 convention)
    if y > 0.0 {
        keys::key_event(keys::K_MOUSE4, true, time);
        keys::key_event(keys::K_MOUSE4, false, time);
    } else if y < 0.0 {
        keys::key_event(keys::K_MOUSE5, true, time);
        keys::key_event(keys::K_MOUSE5, false, time);
    }
}

/// Handle window focus gained.
pub fn handle_focus_gained() {
    let minimized = *MINIMIZED.lock().unwrap();
    let active = !minimized;
    {
        let mut aa = ACTIVE_APP.lock().unwrap();
        *aa = if active { 1 } else { 0 };
    }
    // Key_ClearStates — prevents stuck keys across alt-tab
    keys::key_clear_states();
    // IN_Activate
    {
        let mut input = crate::in_win::INPUT_STATE.lock().unwrap();
        crate::in_win::in_activate(&mut input, active);
    }
    // S_Activate — resume audio
    myq2_client::cl_main::cl_s_activate(active);
}

/// Handle window focus lost.
pub fn handle_focus_lost() {
    {
        let mut aa = ACTIVE_APP.lock().unwrap();
        *aa = 0;
    }
    keys::key_clear_states();
    {
        let mut input = crate::in_win::INPUT_STATE.lock().unwrap();
        crate::in_win::in_activate(&mut input, false);
    }
    // S_Activate — pause audio
    myq2_client::cl_main::cl_s_activate(false);
}

/// Handle window minimized.
pub fn handle_minimized() {
    let mut m = MINIMIZED.lock().unwrap();
    *m = true;
    // Deactivate when minimized
    {
        let mut aa = ACTIVE_APP.lock().unwrap();
        *aa = 0;
    }
    keys::key_clear_states();
    {
        let mut input = crate::in_win::INPUT_STATE.lock().unwrap();
        crate::in_win::in_activate(&mut input, false);
    }
    // S_Activate — pause audio
    myq2_client::cl_main::cl_s_activate(false);
}

/// Handle window restored.
pub fn handle_restored() {
    let mut m = MINIMIZED.lock().unwrap();
    *m = false;
}

/// Handle window exposed (needs redraw).
pub fn handle_exposed() {
    myq2_client::console::scr_dirty_screen();
}

/// Handle window moved.
pub fn handle_moved(x: i32, y: i32) {
    let fs = myq2_common::cvar::cvar_variable_value("vid_fullscreen");
    if fs == 0.0 {
        myq2_common::cvar::cvar_set("vid_xpos", &x.to_string());
        myq2_common::cvar::cvar_set("vid_ypos", &y.to_string());
    }
}

/// Update message time.
pub fn update_msg_time(time: u32) {
    let mut mt = SYS_MSG_TIME.lock().unwrap();
    *mt = time;
}

/// Update frame time.
pub fn update_frame_time() {
    let elapsed = sys_milliseconds();
    let mut ft = SYS_FRAME_TIME.lock().unwrap();
    *ft = elapsed as u32;
}

/// Map a winit KeyCode to a Quake 2 key constant.
fn winit_keycode_to_q2(kc: KeyCode) -> Option<i32> {
    match kc {
        KeyCode::Tab => Some(keys::K_TAB),
        KeyCode::Enter => Some(keys::K_ENTER),
        KeyCode::Escape => Some(keys::K_ESCAPE),
        KeyCode::Space => Some(keys::K_SPACE),
        KeyCode::Backspace => Some(keys::K_BACKSPACE),
        KeyCode::ArrowUp => Some(keys::K_UPARROW),
        KeyCode::ArrowDown => Some(keys::K_DOWNARROW),
        KeyCode::ArrowLeft => Some(keys::K_LEFTARROW),
        KeyCode::ArrowRight => Some(keys::K_RIGHTARROW),
        KeyCode::AltLeft | KeyCode::AltRight => Some(keys::K_ALT),
        KeyCode::ControlLeft | KeyCode::ControlRight => Some(keys::K_CTRL),
        KeyCode::ShiftLeft | KeyCode::ShiftRight => Some(keys::K_SHIFT),
        KeyCode::F1 => Some(keys::K_F1),
        KeyCode::F2 => Some(keys::K_F2),
        KeyCode::F3 => Some(keys::K_F3),
        KeyCode::F4 => Some(keys::K_F4),
        KeyCode::F5 => Some(keys::K_F5),
        KeyCode::F6 => Some(keys::K_F6),
        KeyCode::F7 => Some(keys::K_F7),
        KeyCode::F8 => Some(keys::K_F8),
        KeyCode::F9 => Some(keys::K_F9),
        KeyCode::F10 => Some(keys::K_F10),
        KeyCode::F11 => Some(keys::K_F11),
        KeyCode::F12 => Some(keys::K_F12),
        KeyCode::Insert => Some(keys::K_INS),
        KeyCode::Delete => Some(keys::K_DEL),
        KeyCode::PageDown => Some(keys::K_PGDN),
        KeyCode::PageUp => Some(keys::K_PGUP),
        KeyCode::Home => Some(keys::K_HOME),
        KeyCode::End => Some(keys::K_END),
        KeyCode::NumpadEnter => Some(keys::K_KP_ENTER),
        KeyCode::Numpad0 => Some(keys::K_KP_INS),
        KeyCode::Numpad1 => Some(keys::K_KP_END),
        KeyCode::Numpad2 => Some(keys::K_KP_DOWNARROW),
        KeyCode::Numpad3 => Some(keys::K_KP_PGDN),
        KeyCode::Numpad4 => Some(keys::K_KP_LEFTARROW),
        KeyCode::Numpad5 => Some(keys::K_KP_5),
        KeyCode::Numpad6 => Some(keys::K_KP_RIGHTARROW),
        KeyCode::Numpad7 => Some(keys::K_KP_HOME),
        KeyCode::Numpad8 => Some(keys::K_KP_UPARROW),
        KeyCode::Numpad9 => Some(keys::K_KP_PGUP),
        KeyCode::NumpadSubtract => Some(keys::K_KP_MINUS),
        KeyCode::NumpadAdd => Some(keys::K_KP_PLUS),
        KeyCode::NumpadDivide => Some(keys::K_KP_SLASH),
        KeyCode::NumpadDecimal => Some(keys::K_KP_DEL),
        KeyCode::Pause => Some(keys::K_PAUSE),
        // Letter keys (A-Z map to 'a'-'z' = 97-122)
        KeyCode::KeyA => Some(b'a' as i32),
        KeyCode::KeyB => Some(b'b' as i32),
        KeyCode::KeyC => Some(b'c' as i32),
        KeyCode::KeyD => Some(b'd' as i32),
        KeyCode::KeyE => Some(b'e' as i32),
        KeyCode::KeyF => Some(b'f' as i32),
        KeyCode::KeyG => Some(b'g' as i32),
        KeyCode::KeyH => Some(b'h' as i32),
        KeyCode::KeyI => Some(b'i' as i32),
        KeyCode::KeyJ => Some(b'j' as i32),
        KeyCode::KeyK => Some(b'k' as i32),
        KeyCode::KeyL => Some(b'l' as i32),
        KeyCode::KeyM => Some(b'm' as i32),
        KeyCode::KeyN => Some(b'n' as i32),
        KeyCode::KeyO => Some(b'o' as i32),
        KeyCode::KeyP => Some(b'p' as i32),
        KeyCode::KeyQ => Some(b'q' as i32),
        KeyCode::KeyR => Some(b'r' as i32),
        KeyCode::KeyS => Some(b's' as i32),
        KeyCode::KeyT => Some(b't' as i32),
        KeyCode::KeyU => Some(b'u' as i32),
        KeyCode::KeyV => Some(b'v' as i32),
        KeyCode::KeyW => Some(b'w' as i32),
        KeyCode::KeyX => Some(b'x' as i32),
        KeyCode::KeyY => Some(b'y' as i32),
        KeyCode::KeyZ => Some(b'z' as i32),
        // Number keys (0-9 map to '0'-'9' = 48-57)
        KeyCode::Digit0 => Some(b'0' as i32),
        KeyCode::Digit1 => Some(b'1' as i32),
        KeyCode::Digit2 => Some(b'2' as i32),
        KeyCode::Digit3 => Some(b'3' as i32),
        KeyCode::Digit4 => Some(b'4' as i32),
        KeyCode::Digit5 => Some(b'5' as i32),
        KeyCode::Digit6 => Some(b'6' as i32),
        KeyCode::Digit7 => Some(b'7' as i32),
        KeyCode::Digit8 => Some(b'8' as i32),
        KeyCode::Digit9 => Some(b'9' as i32),
        // Punctuation
        KeyCode::Minus => Some(b'-' as i32),
        KeyCode::Equal => Some(b'=' as i32),
        KeyCode::BracketLeft => Some(b'[' as i32),
        KeyCode::BracketRight => Some(b']' as i32),
        KeyCode::Backslash => Some(b'\\' as i32),
        KeyCode::Semicolon => Some(b';' as i32),
        KeyCode::Quote => Some(b'\'' as i32),
        KeyCode::Backquote => Some(b'`' as i32),
        KeyCode::Comma => Some(b',' as i32),
        KeyCode::Period => Some(b'.' as i32),
        KeyCode::Slash => Some(b'/' as i32),
        _ => None,
    }
}

/// Map a winit MouseButton to a Quake 2 key constant.
fn winit_mouse_button_to_q2(btn: MouseButton) -> Option<i32> {
    match btn {
        MouseButton::Left => Some(keys::K_MOUSE1),
        MouseButton::Right => Some(keys::K_MOUSE2),
        MouseButton::Middle => Some(keys::K_MOUSE3),
        MouseButton::Back => Some(keys::K_MOUSE4),
        MouseButton::Forward => Some(keys::K_MOUSE5),
        _ => None,
    }
}

// ============================================================
// Clipboard
// ============================================================

/// Get text from the system clipboard.
///
/// Original: `char *Sys_GetClipboardData(void)`
pub fn sys_get_clipboard_data() -> Option<String> {
    // Original: OpenClipboard(NULL), GetClipboardData(CF_TEXT), GlobalLock,
    //           GlobalSize, GlobalUnlock, CloseClipboard.
    //
    // Use the Windows clipboard API directly, matching the original C implementation.
    // This is the platform layer, so Windows-specific code is appropriate here.
    #[cfg(target_os = "windows")]
    {
        use std::ffi::CStr;
        use std::ptr;

        const CF_TEXT: u32 = 1;

        #[link(name = "user32")]
        extern "system" {
            fn OpenClipboard(hwnd: *mut std::ffi::c_void) -> i32;
            fn GetClipboardData(format: u32) -> *mut std::ffi::c_void;
            fn CloseClipboard() -> i32;
        }

        #[link(name = "kernel32")]
        extern "system" {
            fn GlobalLock(hmem: *mut std::ffi::c_void) -> *const i8;
            fn GlobalUnlock(hmem: *mut std::ffi::c_void) -> i32;
        }

        // SAFETY: These are standard Win32 clipboard APIs. OpenClipboard(NULL) is
        // safe to call from any thread that doesn't already have the clipboard open.
        // We ensure CloseClipboard is always called via the early-return guard.
        unsafe {
            if OpenClipboard(ptr::null_mut()) == 0 {
                return None;
            }

            let result = (|| {
                let handle = GetClipboardData(CF_TEXT);
                if handle.is_null() {
                    return None;
                }
                let cliptext = GlobalLock(handle);
                if cliptext.is_null() {
                    return None;
                }
                let s = CStr::from_ptr(cliptext).to_str().ok().map(|s| s.to_string());
                GlobalUnlock(handle);
                s
            })();

            CloseClipboard();
            result.filter(|s| !s.is_empty())
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        // Non-Windows platforms: clipboard not implemented in platform layer.
        None
    }
}

// ============================================================
// App Activation
// ============================================================

/// Bring the application window to the foreground.
///
/// Original: `void Sys_AppActivate(void)`
pub fn sys_app_activate() {
    // Original: ShowWindow(cl_hwnd, SW_RESTORE); SetForegroundWindow(cl_hwnd);
    // In the winit port, the window is managed by GlImpContext. We access it
    // through the global GL_IMP_CTX if available; otherwise this is a no-op
    // (window not yet created).
    //
    // The actual raise is performed in glw_imp::glimp_app_activate.
}

// ============================================================
// Game DLL
// ============================================================

/// Whether a game library is currently loaded.
static GAME_LIBRARY_LOADED: Mutex<bool> = Mutex::new(false);

/// Unload the game DLL.
///
/// Original: `void Sys_UnloadGame(void)`
pub fn sys_unload_game() {
    // Original: FreeLibrary(game_library)
    // In the Rust port, the game module is linked statically (myq2_game crate),
    // so there is no dynamic library to free. This toggles the loaded flag
    // for API compatibility with code that checks load/unload sequencing.
    let mut loaded = GAME_LIBRARY_LOADED.lock().unwrap();
    if !*loaded {
        // Com_Error(ERR_FATAL, "FreeLibrary failed for game library");
        panic!("FreeLibrary failed for game library");
    }
    *loaded = false;
}

/// Load the game DLL and return the game API.
///
/// Original: `void *Sys_GetGameAPI(void *parms)`
///
/// In the original C code, this dynamically loaded gamex86.dll via LoadLibrary
/// and resolved GetGameAPI via GetProcAddress. In the Rust port, the game
/// module is compiled directly, so this returns a reference to the statically
/// linked game API.
pub fn sys_get_game_api() -> Option<()> {
    let mut loaded = GAME_LIBRARY_LOADED.lock().unwrap();
    if *loaded {
        // Com_Error(ERR_FATAL, "Sys_GetGameAPI without Sys_UnloadGame");
        panic!("Sys_GetGameAPI without Sys_UnloadGame");
    }

    // Original code searched in this order:
    //   1. cwd/debug(or release)/gamex86.dll
    //   2. (DEBUG only) cwd/gamex86.dll
    //   3. Each FS_NextPath/gamex86.dll
    // Then called GetProcAddress for "GetGameAPI" and invoked it.
    //
    // In the Rust port, the game module is statically linked via the myq2_game
    // crate. Instead of LoadLibrary/GetProcAddress, the caller invokes
    // myq2_game::g_main::get_game_api() directly. This function just manages
    // the loaded flag for sequencing correctness (matching the original's
    // requirement that GetGameAPI is not called twice without UnloadGame).
    *loaded = true;
    Some(())
}

// ============================================================
// Command Line Parsing
// ============================================================

/// Parse a Windows command line string into argv-style tokens.
///
/// Original: `void ParseCommandLine(LPSTR lpCmdLine)`
pub fn parse_command_line(cmd_line: &str) {
    let mut args = CMD_ARGS.lock().unwrap();
    args.clear();
    args.push("exe".to_string());

    let chars = cmd_line.as_bytes();
    let mut i = 0;

    while i < chars.len() && args.len() < MAX_NUM_ARGVS {
        // Skip whitespace and non-printable characters
        while i < chars.len() && (chars[i] <= 32 || chars[i] > 126) {
            i += 1;
        }

        if i < chars.len() {
            let start = i;
            // Consume printable non-whitespace
            while i < chars.len() && chars[i] > 32 && chars[i] <= 126 {
                i += 1;
            }
            if let Ok(s) = std::str::from_utf8(&chars[start..i]) {
                args.push(s.to_string());
            }
        }
    }
}

// ============================================================
// Milliseconds timer (used by main loop; definition in q_shwin.rs)
// ============================================================

/// Re-export from q_shwin for convenience.
pub fn sys_milliseconds() -> i32 {
    crate::q_shwin::sys_milliseconds()
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::keyboard::KeyCode;
    use winit::event::MouseButton;

    // -------------------------------------------------------
    // Constants
    // -------------------------------------------------------

    #[test]
    fn test_memory_constants() {
        assert_eq!(MINIMUM_WIN_MEMORY, 0x0a00000);
        assert_eq!(MAXIMUM_WIN_MEMORY, 0x1000000);
        assert!(MINIMUM_WIN_MEMORY < MAXIMUM_WIN_MEMORY);
        // MINIMUM_WIN_MEMORY = 10 MB, MAXIMUM_WIN_MEMORY = 16 MB
        assert_eq!(MINIMUM_WIN_MEMORY, 10 * 1024 * 1024);
        assert_eq!(MAXIMUM_WIN_MEMORY, 16 * 1024 * 1024);
    }

    #[test]
    fn test_max_num_argvs() {
        assert_eq!(MAX_NUM_ARGVS, 128);
    }

    // -------------------------------------------------------
    // parse_command_line
    // -------------------------------------------------------

    #[test]
    fn test_parse_empty_command_line() {
        parse_command_line("");
        let args = CMD_ARGS.lock().unwrap();
        // Should always have at least the "exe" placeholder
        assert_eq!(args.len(), 1);
        assert_eq!(args[0], "exe");
    }

    #[test]
    fn test_parse_single_arg() {
        parse_command_line("+map q2dm1");
        let args = CMD_ARGS.lock().unwrap();
        assert_eq!(args.len(), 3);
        assert_eq!(args[0], "exe");
        assert_eq!(args[1], "+map");
        assert_eq!(args[2], "q2dm1");
    }

    #[test]
    fn test_parse_multiple_args() {
        parse_command_line("+set dedicated 1 +map q2dm1");
        let args = CMD_ARGS.lock().unwrap();
        assert_eq!(args.len(), 6);
        assert_eq!(args[0], "exe");
        assert_eq!(args[1], "+set");
        assert_eq!(args[2], "dedicated");
        assert_eq!(args[3], "1");
        assert_eq!(args[4], "+map");
        assert_eq!(args[5], "q2dm1");
    }

    #[test]
    fn test_parse_leading_trailing_whitespace() {
        parse_command_line("   hello   world   ");
        let args = CMD_ARGS.lock().unwrap();
        assert_eq!(args.len(), 3);
        assert_eq!(args[0], "exe");
        assert_eq!(args[1], "hello");
        assert_eq!(args[2], "world");
    }

    #[test]
    fn test_parse_whitespace_only() {
        parse_command_line("     ");
        let args = CMD_ARGS.lock().unwrap();
        assert_eq!(args.len(), 1);
        assert_eq!(args[0], "exe");
    }

    #[test]
    fn test_parse_replaces_previous() {
        parse_command_line("first");
        parse_command_line("second third");
        let args = CMD_ARGS.lock().unwrap();
        // The second call should replace the first
        assert_eq!(args.len(), 3);
        assert_eq!(args[0], "exe");
        assert_eq!(args[1], "second");
        assert_eq!(args[2], "third");
    }

    // -------------------------------------------------------
    // winit_keycode_to_q2 mapping
    // -------------------------------------------------------

    #[test]
    fn test_keycode_special_keys() {
        assert_eq!(winit_keycode_to_q2(KeyCode::Tab), Some(keys::K_TAB));
        assert_eq!(winit_keycode_to_q2(KeyCode::Enter), Some(keys::K_ENTER));
        assert_eq!(winit_keycode_to_q2(KeyCode::Escape), Some(keys::K_ESCAPE));
        assert_eq!(winit_keycode_to_q2(KeyCode::Space), Some(keys::K_SPACE));
        assert_eq!(winit_keycode_to_q2(KeyCode::Backspace), Some(keys::K_BACKSPACE));
    }

    #[test]
    fn test_keycode_arrow_keys() {
        assert_eq!(winit_keycode_to_q2(KeyCode::ArrowUp), Some(keys::K_UPARROW));
        assert_eq!(winit_keycode_to_q2(KeyCode::ArrowDown), Some(keys::K_DOWNARROW));
        assert_eq!(winit_keycode_to_q2(KeyCode::ArrowLeft), Some(keys::K_LEFTARROW));
        assert_eq!(winit_keycode_to_q2(KeyCode::ArrowRight), Some(keys::K_RIGHTARROW));
    }

    #[test]
    fn test_keycode_modifier_keys() {
        assert_eq!(winit_keycode_to_q2(KeyCode::AltLeft), Some(keys::K_ALT));
        assert_eq!(winit_keycode_to_q2(KeyCode::AltRight), Some(keys::K_ALT));
        assert_eq!(winit_keycode_to_q2(KeyCode::ControlLeft), Some(keys::K_CTRL));
        assert_eq!(winit_keycode_to_q2(KeyCode::ControlRight), Some(keys::K_CTRL));
        assert_eq!(winit_keycode_to_q2(KeyCode::ShiftLeft), Some(keys::K_SHIFT));
        assert_eq!(winit_keycode_to_q2(KeyCode::ShiftRight), Some(keys::K_SHIFT));
    }

    #[test]
    fn test_keycode_function_keys() {
        assert_eq!(winit_keycode_to_q2(KeyCode::F1), Some(keys::K_F1));
        assert_eq!(winit_keycode_to_q2(KeyCode::F2), Some(keys::K_F2));
        assert_eq!(winit_keycode_to_q2(KeyCode::F3), Some(keys::K_F3));
        assert_eq!(winit_keycode_to_q2(KeyCode::F4), Some(keys::K_F4));
        assert_eq!(winit_keycode_to_q2(KeyCode::F5), Some(keys::K_F5));
        assert_eq!(winit_keycode_to_q2(KeyCode::F6), Some(keys::K_F6));
        assert_eq!(winit_keycode_to_q2(KeyCode::F7), Some(keys::K_F7));
        assert_eq!(winit_keycode_to_q2(KeyCode::F8), Some(keys::K_F8));
        assert_eq!(winit_keycode_to_q2(KeyCode::F9), Some(keys::K_F9));
        assert_eq!(winit_keycode_to_q2(KeyCode::F10), Some(keys::K_F10));
        assert_eq!(winit_keycode_to_q2(KeyCode::F11), Some(keys::K_F11));
        assert_eq!(winit_keycode_to_q2(KeyCode::F12), Some(keys::K_F12));
    }

    #[test]
    fn test_keycode_navigation_keys() {
        assert_eq!(winit_keycode_to_q2(KeyCode::Insert), Some(keys::K_INS));
        assert_eq!(winit_keycode_to_q2(KeyCode::Delete), Some(keys::K_DEL));
        assert_eq!(winit_keycode_to_q2(KeyCode::PageDown), Some(keys::K_PGDN));
        assert_eq!(winit_keycode_to_q2(KeyCode::PageUp), Some(keys::K_PGUP));
        assert_eq!(winit_keycode_to_q2(KeyCode::Home), Some(keys::K_HOME));
        assert_eq!(winit_keycode_to_q2(KeyCode::End), Some(keys::K_END));
    }

    #[test]
    fn test_keycode_numpad() {
        assert_eq!(winit_keycode_to_q2(KeyCode::NumpadEnter), Some(keys::K_KP_ENTER));
        assert_eq!(winit_keycode_to_q2(KeyCode::Numpad0), Some(keys::K_KP_INS));
        assert_eq!(winit_keycode_to_q2(KeyCode::Numpad1), Some(keys::K_KP_END));
        assert_eq!(winit_keycode_to_q2(KeyCode::Numpad2), Some(keys::K_KP_DOWNARROW));
        assert_eq!(winit_keycode_to_q2(KeyCode::Numpad3), Some(keys::K_KP_PGDN));
        assert_eq!(winit_keycode_to_q2(KeyCode::Numpad4), Some(keys::K_KP_LEFTARROW));
        assert_eq!(winit_keycode_to_q2(KeyCode::Numpad5), Some(keys::K_KP_5));
        assert_eq!(winit_keycode_to_q2(KeyCode::Numpad6), Some(keys::K_KP_RIGHTARROW));
        assert_eq!(winit_keycode_to_q2(KeyCode::Numpad7), Some(keys::K_KP_HOME));
        assert_eq!(winit_keycode_to_q2(KeyCode::Numpad8), Some(keys::K_KP_UPARROW));
        assert_eq!(winit_keycode_to_q2(KeyCode::Numpad9), Some(keys::K_KP_PGUP));
        assert_eq!(winit_keycode_to_q2(KeyCode::NumpadSubtract), Some(keys::K_KP_MINUS));
        assert_eq!(winit_keycode_to_q2(KeyCode::NumpadAdd), Some(keys::K_KP_PLUS));
        assert_eq!(winit_keycode_to_q2(KeyCode::NumpadDivide), Some(keys::K_KP_SLASH));
        assert_eq!(winit_keycode_to_q2(KeyCode::NumpadDecimal), Some(keys::K_KP_DEL));
    }

    #[test]
    fn test_keycode_letters() {
        // Letters map to lowercase ASCII: 'a' = 97, 'z' = 122
        assert_eq!(winit_keycode_to_q2(KeyCode::KeyA), Some(b'a' as i32));
        assert_eq!(winit_keycode_to_q2(KeyCode::KeyZ), Some(b'z' as i32));
        assert_eq!(winit_keycode_to_q2(KeyCode::KeyM), Some(b'm' as i32));
    }

    #[test]
    fn test_keycode_digits() {
        // Digits map to '0' = 48 through '9' = 57
        assert_eq!(winit_keycode_to_q2(KeyCode::Digit0), Some(b'0' as i32));
        assert_eq!(winit_keycode_to_q2(KeyCode::Digit9), Some(b'9' as i32));
        assert_eq!(winit_keycode_to_q2(KeyCode::Digit5), Some(b'5' as i32));
    }

    #[test]
    fn test_keycode_punctuation() {
        assert_eq!(winit_keycode_to_q2(KeyCode::Minus), Some(b'-' as i32));
        assert_eq!(winit_keycode_to_q2(KeyCode::Equal), Some(b'=' as i32));
        assert_eq!(winit_keycode_to_q2(KeyCode::BracketLeft), Some(b'[' as i32));
        assert_eq!(winit_keycode_to_q2(KeyCode::BracketRight), Some(b']' as i32));
        assert_eq!(winit_keycode_to_q2(KeyCode::Backslash), Some(b'\\' as i32));
        assert_eq!(winit_keycode_to_q2(KeyCode::Semicolon), Some(b';' as i32));
        assert_eq!(winit_keycode_to_q2(KeyCode::Quote), Some(b'\'' as i32));
        assert_eq!(winit_keycode_to_q2(KeyCode::Backquote), Some(b'`' as i32));
        assert_eq!(winit_keycode_to_q2(KeyCode::Comma), Some(b',' as i32));
        assert_eq!(winit_keycode_to_q2(KeyCode::Period), Some(b'.' as i32));
        assert_eq!(winit_keycode_to_q2(KeyCode::Slash), Some(b'/' as i32));
    }

    #[test]
    fn test_keycode_pause() {
        assert_eq!(winit_keycode_to_q2(KeyCode::Pause), Some(keys::K_PAUSE));
    }

    #[test]
    fn test_keycode_unknown_returns_none() {
        // Keys without a Q2 mapping should return None
        assert_eq!(winit_keycode_to_q2(KeyCode::CapsLock), None);
        assert_eq!(winit_keycode_to_q2(KeyCode::NumLock), None);
        assert_eq!(winit_keycode_to_q2(KeyCode::ScrollLock), None);
        assert_eq!(winit_keycode_to_q2(KeyCode::PrintScreen), None);
    }

    // -------------------------------------------------------
    // winit_mouse_button_to_q2 mapping
    // -------------------------------------------------------

    #[test]
    fn test_mouse_button_left() {
        assert_eq!(winit_mouse_button_to_q2(MouseButton::Left), Some(keys::K_MOUSE1));
    }

    #[test]
    fn test_mouse_button_right() {
        assert_eq!(winit_mouse_button_to_q2(MouseButton::Right), Some(keys::K_MOUSE2));
    }

    #[test]
    fn test_mouse_button_middle() {
        assert_eq!(winit_mouse_button_to_q2(MouseButton::Middle), Some(keys::K_MOUSE3));
    }

    #[test]
    fn test_mouse_button_back() {
        assert_eq!(winit_mouse_button_to_q2(MouseButton::Back), Some(keys::K_MOUSE4));
    }

    #[test]
    fn test_mouse_button_forward() {
        assert_eq!(winit_mouse_button_to_q2(MouseButton::Forward), Some(keys::K_MOUSE5));
    }

    #[test]
    fn test_mouse_button_other_returns_none() {
        assert_eq!(winit_mouse_button_to_q2(MouseButton::Other(6)), None);
        assert_eq!(winit_mouse_button_to_q2(MouseButton::Other(99)), None);
    }

    // -------------------------------------------------------
    // sys_scan_for_cd (legacy no-op)
    // -------------------------------------------------------

    #[test]
    fn test_sys_scan_for_cd_returns_none() {
        assert!(sys_scan_for_cd().is_none());
    }

    // -------------------------------------------------------
    // Game library loaded flag (initial state)
    //
    // Note: sys_get_game_api and sys_unload_game use a shared global
    // Mutex (GAME_LIBRARY_LOADED) and call .lock().unwrap(). Panicking
    // while holding the lock poisons it for other tests running in
    // parallel. We verify the initial state only to avoid poisoning.
    // -------------------------------------------------------

    #[test]
    fn test_game_library_initially_not_loaded() {
        // GAME_LIBRARY_LOADED starts as false
        let loaded = GAME_LIBRARY_LOADED.lock().unwrap();
        assert!(!*loaded, "Game library should not be loaded at initialization");
    }

    // -------------------------------------------------------
    // ConsoleBuffer
    // -------------------------------------------------------

    #[test]
    fn test_console_buffer_initial_state() {
        let buf = ConsoleBuffer {
            text: [0u8; 256],
            len: 0,
        };
        assert_eq!(buf.len, 0);
        assert!(buf.text.iter().all(|&b| b == 0));
    }
}
