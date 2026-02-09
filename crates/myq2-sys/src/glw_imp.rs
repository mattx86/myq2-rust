// glw_imp.rs -- Window creation and Vulkan device management using winit.
//
// Replaced SDL3 with winit for window management and direct Vulkan via ash.
// The Vulkan context is stored in myq2_renderer::vulkan module.

#![allow(dead_code)]

use std::sync::Arc;
use winit::dpi::PhysicalSize;
use winit::event_loop::EventLoop;
use winit::window::{Window, WindowAttributes, Fullscreen};
use raw_window_handle::HasDisplayHandle;

use myq2_common::common::{com_printf, DISTNAME};

// Win32 FFI for hardware gamma ramp support
#[cfg(target_os = "windows")]
extern "system" {
    fn GetDC(hwnd: isize) -> isize;
    fn ReleaseDC(hwnd: isize, hdc: isize) -> i32;
    fn GetDeviceGammaRamp(hdc: isize, lp_ramp: *mut std::ffi::c_void) -> i32;
    fn SetDeviceGammaRamp(hdc: isize, lp_ramp: *const std::ffi::c_void) -> i32;
}

// =============================================================================
// RsErr — imported from canonical definition in myq2-renderer
// =============================================================================

pub use myq2_renderer::vk_local::RsErr;

// =============================================================================
// GL window state (legacy compatibility)
// =============================================================================

#[derive(Default)]
pub struct GlwState {
    pub h_instance: usize,
    pub wndproc: usize,
    pub h_wnd: usize,
    pub h_dc: usize,
    pub h_glrc: usize,
    pub minidriver: bool,
    pub allow_display_depth_change: bool,
    pub mcd_accelerated: bool,
    pub log_fp: Option<()>,
    pub hinst_opengl: usize,
}

/// Platform-layer VkState — NOT the same as `myq2_renderer::vk_local::VkState`.
/// This holds only the window-management subset needed by the sys platform layer,
/// while the renderer's VkState tracks the full rendering pipeline state.
#[derive(Default)]
pub struct VkState {
    pub fullscreen: bool,
    pub stereo_enabled: bool,
}

/// Platform-layer VkConfig — NOT the same as `myq2_renderer::vk_local::VkConfig`.
/// This holds only the platform-relevant subset (gamma ramp support), while the
/// renderer's VkConfig tracks renderer capabilities and extensions.
#[derive(Default)]
pub struct VkConfig {
    pub gammaramp: bool,
}

// =============================================================================
// Gamma ramp data
// =============================================================================

pub struct GammaRampData {
    pub original_ramp: [[u16; 256]; 3],
    pub gamma_ramp: [[u16; 256]; 3],
}

impl Default for GammaRampData {
    fn default() -> Self {
        Self {
            original_ramp: [[0u16; 256]; 3],
            gamma_ramp: [[0u16; 256]; 3],
        }
    }
}

pub static mut HAVE_STENCIL: bool = false;

// =============================================================================
// GLimp state — holds winit Window
// =============================================================================

pub struct VkImpState {
    pub window: Arc<Window>,
}

// =============================================================================
// GL implementation context
// =============================================================================

/// Top-level context holding all window/Vulkan state.
pub struct GlImpContext {
    pub glw_state: GlwState,
    pub vk_state: VkState,
    pub vk_config: VkConfig,
    pub gamma: GammaRampData,
    pub state: Option<VkImpState>,
    /// winit event loop - consumed by main loop
    pub event_loop: Option<EventLoop<()>>,
}

impl Default for GlImpContext {
    fn default() -> Self {
        Self {
            glw_state: GlwState::default(),
            vk_state: VkState::default(),
            vk_config: VkConfig::default(),
            gamma: GammaRampData::default(),
            state: None,
            event_loop: None,
        }
    }
}

impl GlImpContext {
    fn verify_driver(&self) -> bool {
        true
    }

    /// Creates the winit window and initializes the Vulkan device.
    pub fn vid_create_window(
        &mut self,
        width: i32,
        height: i32,
        fullscreen: bool,
    ) -> bool {
        com_printf(&format!(
            "VID_CreateWindow: {}x{} {}\n",
            width,
            height,
            if fullscreen { "fullscreen" } else { "windowed" }
        ));

        // Create event loop if not already done
        let event_loop = match self.event_loop.take() {
            Some(el) => el,
            None => match EventLoop::new() {
                Ok(el) => el,
                Err(e) => {
                    com_printf(&format!("VID_CreateWindow() - event loop creation failed: {}\n", e));
                    return false;
                }
            },
        };

        // Build window attributes
        let mut window_attrs = WindowAttributes::default()
            .with_title(DISTNAME)
            .with_inner_size(PhysicalSize::new(width as u32, height as u32));

        if fullscreen {
            // Use borderless fullscreen on primary monitor
            window_attrs = window_attrs.with_fullscreen(Some(Fullscreen::Borderless(None)));
        }

        // Create the window using ActiveEventLoop pattern
        // Note: In winit 0.30, windows must be created from an ActiveEventLoop.
        // For initialization, we use the deprecated create_window_before_run approach.
        #[allow(deprecated)]
        let window: Arc<Window> = match event_loop.create_window(window_attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                com_printf(&format!("VID_CreateWindow() - window build failed: {}\n", e));
                self.event_loop = Some(event_loop);
                return false;
            }
        };

        com_printf("...winit window created\n");

        // Initialize Vulkan context
        let display_handle = match window.display_handle() {
            Ok(h) => h.as_raw(),
            Err(e) => {
                com_printf(&format!("VID_CreateWindow() - display handle failed: {}\n", e));
                self.event_loop = Some(event_loop);
                return false;
            }
        };

        // SAFETY: Vulkan initialization using valid window handles
        let vk_init_result: Result<(), String> = (|| unsafe {
            // 1. Create Vulkan context
            let ctx = myq2_renderer::vulkan::VulkanContext::new(display_handle, cfg!(debug_assertions))
                .map_err(|e| format!("Vulkan context failed: {}", e))?;
            com_printf("...Vulkan 1.3 context created\n");
            if ctx.rt_capabilities.supported {
                com_printf("...Ray tracing supported\n");
            } else {
                com_printf("...Ray tracing NOT supported (fallback to rasterization)\n");
            }

            // 2. Create surface (needs window handle)
            let surface = myq2_renderer::vulkan::VulkanSurface::from_winit(&ctx, &window)
                .map_err(|e| format!("Vulkan surface failed: {}", e))?;
            com_printf("...Vulkan surface created\n");

            // 3. Create swapchain
            let swapchain = myq2_renderer::vulkan::Swapchain::new(
                &ctx, &surface, width as u32, height as u32, None
            ).map_err(|e| format!("Vulkan swapchain failed: {}", e))?;
            com_printf(&format!("...Vulkan swapchain created ({}x{})\n", swapchain.extent.width, swapchain.extent.height));

            // 4. Create command manager
            let commands = myq2_renderer::vulkan::CommandManager::new(&ctx)
                .map_err(|e| format!("Vulkan command manager failed: {}", e))?;
            com_printf("...Vulkan command manager created\n");

            // 5. Store all objects in renderer module
            myq2_renderer::modern::gpu_device::init_device(ctx);
            myq2_renderer::modern::gpu_device::init_surface(surface);
            myq2_renderer::modern::gpu_device::init_swapchain(swapchain);
            myq2_renderer::modern::gpu_device::init_commands(commands);

            // 6. Initialize the frame manager
            myq2_renderer::modern::gpu_device::init_frame_manager();

            Ok(())
        })();

        if let Err(e) = vk_init_result {
            com_printf(&format!("VID_CreateWindow() - {}\n", e));
            self.event_loop = Some(event_loop);
            return false;
        }

        // SAFETY: Single-threaded engine
        unsafe {
            HAVE_STENCIL = true;
        }
        com_printf("...using stencil buffer (Vulkan managed)\n");

        // Save original gamma ramp for restore on shutdown
        self.save_original_gamma();
        if !self.vk_config.gammaramp {
            // Fallback: zero-init if platform gamma ramp not available
            self.gamma.original_ramp = [[0u16; 256]; 3];
        }

        self.glw_state.h_wnd = 1;
        self.glw_state.h_dc = 1;
        self.glw_state.h_glrc = 1;

        self.state = Some(VkImpState { window });
        self.event_loop = Some(event_loop);

        com_printf("VID_CreateWindow: ok (Vulkan + winit)\n");
        true
    }

    pub fn glimp_set_mode(
        &mut self,
        pwidth: &mut i32,
        pheight: &mut i32,
        mode: i32,
        fullscreen: bool,
    ) -> RsErr {
        com_printf("Initializing Vulkan display\n");
        com_printf(&format!("...setting mode {}:", mode));

        let width = *pwidth;
        let height = *pheight;

        let win_fs = if fullscreen { "FS" } else { "W" };
        com_printf(&format!(" {} {} {}\n", width, height, win_fs));

        if self.state.is_some() {
            self.glimp_shutdown();
        }

        if fullscreen {
            com_printf("...attempting fullscreen\n");
            *pwidth = width;
            *pheight = height;
            self.vk_state.fullscreen = true;

            if !self.vid_create_window(width, height, true) {
                return RsErr::InvalidMode;
            }
            return RsErr::Ok;
        } else {
            com_printf("...setting windowed mode\n");
            *pwidth = width;
            *pheight = height;
            self.vk_state.fullscreen = false;

            if !self.vid_create_window(width, height, false) {
                return RsErr::InvalidMode;
            }
        }

        RsErr::Ok
    }

    pub fn glimp_shutdown(&mut self) {
        // Restore original gamma ramp before shutdown
        self.restore_gamma_ramp();

        // Vulkan cleanup
        com_printf("GLimp_Shutdown: shutting down Vulkan\n");
        // SAFETY: Single-threaded engine, shutdown called before any other rendering
        unsafe {
            myq2_renderer::modern::gpu_device::shutdown_device();
        }

        if self.state.is_some() {
            com_printf("GLimp_Shutdown: destroying window\n");
        }

        self.state = None;

        self.glw_state.h_glrc = 0;
        self.glw_state.h_dc = 0;
        self.glw_state.h_wnd = 0;
        self.glw_state.log_fp = None;

        if self.vk_state.fullscreen {
            self.vk_state.fullscreen = false;
        }
    }

    pub fn glimp_init(&mut self, _hinstance: usize, _wndproc: usize) -> bool {
        self.glw_state.allow_display_depth_change = true;
        self.glw_state.h_instance = _hinstance;
        self.glw_state.wndproc = _wndproc;
        true
    }

    pub fn glimp_begin_frame(&self, _camera_separation: f32) {
        // Vulkan frame begin handled by render path
    }

    pub fn glimp_end_frame(&self) {
        // Vulkan present handled by render path
    }

    /// Compute and optionally apply a hardware gamma ramp.
    /// When r_hwgamma is enabled, the computed ramp is applied to the display
    /// via SetDeviceGammaRamp (Windows). Otherwise it just computes the table.
    pub fn update_gamma_ramp(&mut self, vid_gamma: f32) {
        if !self.vk_config.gammaramp {
            return;
        }

        // Compute gamma ramp from original ramp + gamma value
        self.gamma.gamma_ramp = self.gamma.original_ramp;
        for o in 0..3 {
            for i in 0..256 {
                let v = (255.0
                    * ((i as f64 + 0.5) * 0.003_913_894_324_853_229_f64)
                        .powf(vid_gamma as f64)
                    + 0.5) as i32;
                let v = v.clamp(0, 255);
                self.gamma.gamma_ramp[o][i] = (v as u16) << 8;
            }
        }

        // Apply to display via platform API
        self.apply_gamma_ramp_to_display();
    }

    /// Save the original gamma ramp from the display.
    /// Call during window creation to enable restore on shutdown.
    pub fn save_original_gamma(&mut self) {
        #[cfg(target_os = "windows")]
        {
            if let Some(ref state) = self.state {
                // SAFETY: Win32 FFI calls, single-threaded
                unsafe {
                    use raw_window_handle::HasWindowHandle;
                    if let Ok(handle) = state.window.window_handle() {
                        if let raw_window_handle::RawWindowHandle::Win32(win32) = handle.as_raw() {
                            let hwnd = win32.hwnd.get() as isize;
                            let hdc = GetDC(hwnd);
                            if hdc != 0 {
                                // Flatten [3][256] to [768] u16 for Win32 API
                                let mut ramp = [0u16; 768];
                                if GetDeviceGammaRamp(hdc, ramp.as_mut_ptr() as *mut _) != 0 {
                                    for ch in 0..3 {
                                        for i in 0..256 {
                                            self.gamma.original_ramp[ch][i] = ramp[ch * 256 + i];
                                        }
                                    }
                                    self.vk_config.gammaramp = true;
                                    // Set the renderer's VkConfig so it knows hw gamma is available
                                    if let Some(ref mut cfg) = myq2_renderer::vk_rmain::VK_CONFIG {
                                        cfg.gammaramp = 1;
                                    }
                                }
                                ReleaseDC(hwnd, hdc);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Apply the computed gamma ramp to the display.
    fn apply_gamma_ramp_to_display(&self) {
        #[cfg(target_os = "windows")]
        {
            if let Some(ref state) = self.state {
                // SAFETY: Win32 FFI calls, single-threaded
                unsafe {
                    use raw_window_handle::HasWindowHandle;
                    if let Ok(handle) = state.window.window_handle() {
                        if let raw_window_handle::RawWindowHandle::Win32(win32) = handle.as_raw() {
                            let hwnd = win32.hwnd.get() as isize;
                            let hdc = GetDC(hwnd);
                            if hdc != 0 {
                                // Flatten [3][256] to [768] u16 for Win32 API
                                let mut ramp = [0u16; 768];
                                for ch in 0..3 {
                                    for i in 0..256 {
                                        ramp[ch * 256 + i] = self.gamma.gamma_ramp[ch][i];
                                    }
                                }
                                SetDeviceGammaRamp(hdc, ramp.as_ptr() as *const _);
                                ReleaseDC(hwnd, hdc);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Restore the original gamma ramp on shutdown.
    pub fn restore_gamma_ramp(&self) {
        #[cfg(target_os = "windows")]
        {
            if !self.vk_config.gammaramp {
                return;
            }
            if let Some(ref state) = self.state {
                // SAFETY: Win32 FFI calls, single-threaded
                unsafe {
                    use raw_window_handle::HasWindowHandle;
                    if let Ok(handle) = state.window.window_handle() {
                        if let raw_window_handle::RawWindowHandle::Win32(win32) = handle.as_raw() {
                            let hwnd = win32.hwnd.get() as isize;
                            let hdc = GetDC(hwnd);
                            if hdc != 0 {
                                let mut ramp = [0u16; 768];
                                for ch in 0..3 {
                                    for i in 0..256 {
                                        ramp[ch * 256 + i] = self.gamma.original_ramp[ch][i];
                                    }
                                }
                                SetDeviceGammaRamp(hdc, ramp.as_ptr() as *const _);
                                ReleaseDC(hwnd, hdc);
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn glimp_app_activate(&mut self, active: bool, _vid_fullscreen: bool) {
        if let Some(ref state) = self.state {
            if active {
                state.window.focus_window();
            } else if _vid_fullscreen {
                state.window.set_minimized(true);
            }
        }
    }

    pub fn window(&self) -> Option<&Arc<Window>> {
        self.state.as_ref().map(|s| &s.window)
    }

    pub fn is_active(&self) -> bool {
        self.state.is_some()
    }

    /// Take the event loop for use in main. Returns None if already taken.
    pub fn take_event_loop(&mut self) -> Option<EventLoop<()>> {
        self.event_loop.take()
    }
}
