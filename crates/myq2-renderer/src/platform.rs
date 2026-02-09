// platform.rs — Platform-layer dispatch for GL context management
//
// The renderer crate cannot depend on myq2-sys (that would be circular),
// so we define callback function pointers here that myq2-sys registers
// at startup. This follows the same OnceLock<Mutex<...>> pattern used
// by myq2-common::net for network dispatch.

use std::sync::{Mutex, OnceLock};

// ============================================================
// Function pointer type aliases
// ============================================================

/// GLimp_Init(hinstance, hwnd) -> bool
pub type GlimpInitFn = Box<dyn Fn(usize, usize) -> bool + Send>;
/// GLimp_Shutdown()
pub type GlimpShutdownFn = Box<dyn Fn() + Send>;
/// GLimp_BeginFrame(camera_separation)
pub type GlimpBeginFrameFn = Box<dyn Fn(f32) + Send>;
/// GLimp_EndFrame()
pub type GlimpEndFrameFn = Box<dyn Fn() + Send>;
/// GLimp_SetMode(width, height, mode, fullscreen) -> rserr_t as i32
pub type GlimpSetModeFn = Box<dyn Fn(&mut i32, &mut i32, f32, bool) -> i32 + Send>;
/// QGL_Init(driver) -> bool
pub type QglInitFn = Box<dyn Fn(&str) -> bool + Send>;
/// QGL_Shutdown()
pub type QglShutdownFn = Box<dyn Fn() + Send>;
/// VID_MenuInit()
pub type VidMenuInitFn = Box<dyn Fn() + Send>;
/// UpdateGammaRamp()
pub type UpdateGammaRampFn = Box<dyn Fn() + Send>;

// ============================================================
// Dispatch table
// ============================================================

pub struct PlatformDispatch {
    pub glimp_init: Option<GlimpInitFn>,
    pub glimp_shutdown: Option<GlimpShutdownFn>,
    pub glimp_begin_frame: Option<GlimpBeginFrameFn>,
    pub glimp_end_frame: Option<GlimpEndFrameFn>,
    pub glimp_set_mode: Option<GlimpSetModeFn>,
    pub qvk_init: Option<QglInitFn>,
    pub qvk_shutdown: Option<QglShutdownFn>,
    pub vid_menu_init: Option<VidMenuInitFn>,
    pub update_gamma_ramp: Option<UpdateGammaRampFn>,
}

impl Default for PlatformDispatch {
    fn default() -> Self {
        Self {
            glimp_init: None,
            glimp_shutdown: None,
            glimp_begin_frame: None,
            glimp_end_frame: None,
            glimp_set_mode: None,
            qvk_init: None,
            qvk_shutdown: None,
            vid_menu_init: None,
            update_gamma_ramp: None,
        }
    }
}

static PLATFORM: OnceLock<Mutex<PlatformDispatch>> = OnceLock::new();

fn dispatch() -> &'static Mutex<PlatformDispatch> {
    PLATFORM.get_or_init(|| Mutex::new(PlatformDispatch::default()))
}

// ============================================================
// Registration API (called by myq2-sys at startup)
// ============================================================

/// Register all platform callbacks at once.
pub fn platform_register(d: PlatformDispatch) {
    *dispatch().lock().unwrap() = d;
}

// ============================================================
// Invocation API (called by vk_rmain.rs)
// ============================================================

pub fn glimp_init(hinstance: usize, hwnd: usize) -> bool {
    let d = dispatch().lock().unwrap();
    if let Some(ref f) = d.glimp_init {
        f(hinstance, hwnd)
    } else {
        myq2_common::common::com_printf("GLimp_Init: no platform dispatch registered!\n");
        false
    }
}

pub fn glimp_shutdown() {
    let d = dispatch().lock().unwrap();
    if let Some(ref f) = d.glimp_shutdown {
        f();
    }
}

pub fn glimp_begin_frame(camera_separation: f32) {
    let d = dispatch().lock().unwrap();
    if let Some(ref f) = d.glimp_begin_frame {
        f(camera_separation);
    }
}

pub fn glimp_end_frame() {
    let d = dispatch().lock().unwrap();
    if let Some(ref f) = d.glimp_end_frame {
        f();
    }
}

pub fn glimp_set_mode(width: &mut i32, height: &mut i32, mode: f32, fullscreen: bool) -> i32 {
    let d = dispatch().lock().unwrap();
    if let Some(ref f) = d.glimp_set_mode {
        f(width, height, mode, fullscreen)
    } else {
        0 // RSERR_OK
    }
}

pub fn qvk_init(driver: &str) -> bool {
    let d = dispatch().lock().unwrap();
    if let Some(ref f) = d.qvk_init {
        f(driver)
    } else {
        true
    }
}

pub fn qvk_shutdown() {
    let d = dispatch().lock().unwrap();
    if let Some(ref f) = d.qvk_shutdown {
        f();
    }
}

pub fn vid_menu_init() {
    let d = dispatch().lock().unwrap();
    if let Some(ref f) = d.vid_menu_init {
        f();
    }
}

pub fn update_gamma_ramp() {
    let d = dispatch().lock().unwrap();
    if let Some(ref f) = d.update_gamma_ramp {
        f();
    }
}
