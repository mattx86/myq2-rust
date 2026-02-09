// platform.rs — Platform-layer dispatch for client subsystems
//
// The client crate cannot depend on myq2-sys (that would be circular),
// so we define callback function pointers here that myq2-sys registers
// at startup. This follows the same OnceLock<Mutex<...>> pattern used
// by myq2-renderer::platform for GL context management.

use std::sync::{Mutex, OnceLock};


// ============================================================
// Function pointer type aliases
// ============================================================

/// VID_Init()
pub type VidInitFn = Box<dyn Fn() + Send>;
/// VID_Shutdown()
pub type VidShutdownFn = Box<dyn Fn() + Send>;
/// VID_CheckChanges()
pub type VidCheckChangesFn = Box<dyn Fn() + Send>;
/// R_SetPalette(palette: Option<&[u8]>)
pub type RSetPaletteFn = Box<dyn Fn(Option<&[u8]>) + Send>;

/// IN_Init()
pub type InInitFn = Box<dyn Fn() + Send>;
/// IN_Shutdown()
pub type InShutdownFn = Box<dyn Fn() + Send>;
/// IN_Commands()
pub type InCommandsFn = Box<dyn Fn() + Send>;
/// IN_Frame()
pub type InFrameFn = Box<dyn Fn() + Send>;

/// Sys_SendKeyEvents()
pub type SysSendKeyEventsFn = Box<dyn Fn() + Send>;
/// Sys_AppActivate()
pub type SysAppActivateFn = Box<dyn Fn() + Send>;

/// NET_Config(multiplayer: bool)
pub type NetConfigFn = Box<dyn Fn(bool) + Send>;

/// SV_Shutdown(msg, reconnect)
pub type SvShutdownFn = Box<dyn Fn(&str, bool) + Send>;

// ============================================================
// Dispatch table
// ============================================================

#[derive(Default)]
pub struct ClientPlatformDispatch {
    pub vid_init: Option<VidInitFn>,
    pub vid_shutdown: Option<VidShutdownFn>,
    pub vid_check_changes: Option<VidCheckChangesFn>,
    pub r_set_palette: Option<RSetPaletteFn>,

    pub in_init: Option<InInitFn>,
    pub in_shutdown: Option<InShutdownFn>,
    pub in_commands: Option<InCommandsFn>,
    pub in_frame: Option<InFrameFn>,

    pub sys_send_key_events: Option<SysSendKeyEventsFn>,
    pub sys_app_activate: Option<SysAppActivateFn>,

    pub net_config: Option<NetConfigFn>,
    pub sv_shutdown: Option<SvShutdownFn>,
}



static CLIENT_PLATFORM: OnceLock<Mutex<ClientPlatformDispatch>> = OnceLock::new();

fn dispatch() -> &'static Mutex<ClientPlatformDispatch> {
    CLIENT_PLATFORM.get_or_init(|| Mutex::new(ClientPlatformDispatch::default()))
}

// ============================================================
// Registration API (called by myq2-sys at startup)
// ============================================================

/// Register all client platform callbacks at once.
pub fn client_platform_register(d: ClientPlatformDispatch) {
    *dispatch().lock().unwrap() = d;
}

// ============================================================
// Invocation API (called by cl_main.rs and other client code)
// ============================================================

pub fn vid_init() {
    let d = dispatch().lock().unwrap();
    if let Some(ref f) = d.vid_init { f(); }
}

pub fn vid_shutdown() {
    let d = dispatch().lock().unwrap();
    if let Some(ref f) = d.vid_shutdown { f(); }
}

pub fn vid_check_changes() {
    let d = dispatch().lock().unwrap();
    if let Some(ref f) = d.vid_check_changes { f(); }
}

pub fn r_set_palette(palette: Option<&[u8]>) {
    let d = dispatch().lock().unwrap();
    if let Some(ref f) = d.r_set_palette { f(palette); }
}

pub fn in_init() {
    let d = dispatch().lock().unwrap();
    if let Some(ref f) = d.in_init { f(); }
}

pub fn in_shutdown() {
    let d = dispatch().lock().unwrap();
    if let Some(ref f) = d.in_shutdown { f(); }
}

pub fn in_commands() {
    let d = dispatch().lock().unwrap();
    if let Some(ref f) = d.in_commands { f(); }
}

pub fn in_frame() {
    let d = dispatch().lock().unwrap();
    if let Some(ref f) = d.in_frame { f(); }
}

pub fn sys_send_key_events() {
    let d = dispatch().lock().unwrap();
    if let Some(ref f) = d.sys_send_key_events { f(); }
}

pub fn sys_app_activate() {
    let d = dispatch().lock().unwrap();
    if let Some(ref f) = d.sys_app_activate { f(); }
}

pub fn net_config(multiplayer: bool) {
    let d = dispatch().lock().unwrap();
    if let Some(ref f) = d.net_config {
        f(multiplayer);
    } else {
        // Fall back to myq2_common::net::net_config stub
        myq2_common::net::net_config(multiplayer);
    }
}

pub fn sv_shutdown(msg: &str, reconnect: bool) {
    let d = dispatch().lock().unwrap();
    if let Some(ref f) = d.sv_shutdown { f(msg, reconnect); }
}
