// glw_imp.rs -- Window creation and Vulkan device management using winit.
//
// Replaced SDL3 with winit for window management and direct Vulkan via ash.
// The Vulkan context is stored in myq2_renderer::vulkan module.

#![allow(dead_code)]

use std::sync::Arc;
use winit::dpi::PhysicalSize;
use winit::event_loop::EventLoop;
use winit::window::{Icon, Window, WindowAttributes, Fullscreen};
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
#[derive(Default)]
pub struct GlImpContext {
    pub glw_state: GlwState,
    pub vk_state: VkState,
    pub vk_config: VkConfig,
    pub gamma: GammaRampData,
    pub state: Option<VkImpState>,
    /// winit event loop - consumed by main loop
    pub event_loop: Option<EventLoop<()>>,
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
            .with_inner_size(PhysicalSize::new(width as u32, height as u32))
            .with_resizable(false);

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

        // Set window icon
        if let Some(icon) = generate_q2_icon() {
            window.set_window_icon(Some(icon));
        }

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
            eprintln!("VID_CreateWindow: Creating VulkanContext with validation DISABLED");
            let ctx = myq2_renderer::vulkan::VulkanContext::new(display_handle, false)
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

        // Request keyboard focus so the window receives input events
        window.focus_window();

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

            // Update width/height to match actual swapchain extent (may differ from requested)
            if let Some(extent) = myq2_renderer::modern::gpu_device::get_swapchain_extent() {
                com_printf(&format!("glimp_set_mode: Requested {}x{}, actual swapchain extent {}x{}\n",
                    width, height, extent.width, extent.height));
                *pwidth = extent.width as i32;
                *pheight = extent.height as i32;

                // Update client-side console viddef to match actual framebuffer dimensions
                let mut cs = myq2_client::console::cs();
                cs.viddef.width = extent.width as i32;
                cs.viddef.height = extent.height as i32;
                com_printf(&format!("glimp_set_mode: Updated client viddef to {}x{}\n",
                    cs.viddef.width, cs.viddef.height));
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

        // Update width/height to match actual swapchain extent (may differ from requested)
        if let Some(extent) = myq2_renderer::modern::gpu_device::get_swapchain_extent() {
            com_printf(&format!("glimp_set_mode: Requested {}x{}, actual swapchain extent {}x{}\n",
                width, height, extent.width, extent.height));
            *pwidth = extent.width as i32;
            *pheight = extent.height as i32;

            // Update client-side console viddef to match actual framebuffer dimensions
            let mut cs = myq2_client::console::cs();
            cs.viddef.width = extent.width as i32;
            cs.viddef.height = extent.height as i32;
            com_printf(&format!("glimp_set_mode: Updated client viddef to {}x{}\n",
                cs.viddef.width, cs.viddef.height));
        }

        RsErr::Ok
    }

    pub fn glimp_shutdown(&mut self) {
        // Restore original gamma ramp before shutdown
        self.restore_gamma_ramp();

        // Vulkan cleanup
        com_printf("GLimp_Shutdown: shutting down Vulkan\n");
        myq2_renderer::modern::gpu_device::shutdown_device();

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
        // Present the Vulkan frame (flushes 2D drawing, submits commands, and presents swapchain)
        myq2_renderer::vk_rmain::r_present_frame();
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
                // SAFETY: Win32 FFI calls, called from main thread only
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
                                    if let Some(ref mut cfg) = myq2_renderer::vk_rmain::rg().vk_config {
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

        // Wayland does not expose hardware gamma ramp control — it is managed
        // by the compositor for color management. Disable hw gamma so
        // update_gamma_ramp() early-returns. The engine's internal gamma math
        // still works; it just doesn't apply to the display hardware.
        #[cfg(target_os = "linux")]
        {
            self.vk_config.gammaramp = false;
        }
    }

    /// Apply the computed gamma ramp to the display.
    fn apply_gamma_ramp_to_display(&self) {
        #[cfg(target_os = "windows")]
        {
            if let Some(ref state) = self.state {
                // SAFETY: Win32 FFI calls, called from main thread only
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

        // No-op on Linux: Wayland has no gamma ramp API.
        #[cfg(target_os = "linux")]
        {}
    }

    /// Restore the original gamma ramp on shutdown.
    pub fn restore_gamma_ramp(&self) {
        #[cfg(target_os = "windows")]
        {
            if !self.vk_config.gammaramp {
                return;
            }
            if let Some(ref state) = self.state {
                // SAFETY: Win32 FFI calls, called from main thread only
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

        // No-op on Linux: Wayland has no gamma ramp API.
        #[cfg(target_os = "linux")]
        {}
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

    /// Request a window redraw to trigger the next frame
    pub fn request_redraw(&self) {
        if let Some(ref state) = self.state {
            state.window.request_redraw();
        }
    }

    /// Take the event loop for use in main. Returns None if already taken.
    pub fn take_event_loop(&mut self) -> Option<EventLoop<()>> {
        self.event_loop.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_gamma_ramp_disabled() {
        let ctx = GlImpContext::default();
        assert!(!ctx.vk_config.gammaramp);
    }

    #[test]
    fn test_update_gamma_ramp_noop_when_disabled() {
        let mut ctx = GlImpContext::default();
        assert!(!ctx.vk_config.gammaramp);
        // Should early-return without panicking when gammaramp is false
        ctx.update_gamma_ramp(1.0);
        // Gamma ramp data should remain zeroed
        assert!(ctx.gamma.gamma_ramp[0].iter().all(|&v| v == 0));
    }

    #[test]
    fn test_gamma_ramp_computation() {
        let mut ctx = GlImpContext::default();
        ctx.vk_config.gammaramp = true;
        ctx.update_gamma_ramp(1.0);
        // With gamma=1.0, the ramp should be close to linear (identity)
        // Each entry should be (i << 8) approximately
        for i in 1..256 {
            let expected = (i as u16) << 8;
            let actual = ctx.gamma.gamma_ramp[0][i];
            // Allow small rounding error
            let diff = (actual as i32 - expected as i32).unsigned_abs();
            assert!(diff <= 256, "gamma_ramp[0][{}] = {}, expected ~{}", i, actual, expected);
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_save_original_gamma_disables_gammaramp_on_linux() {
        let mut ctx = GlImpContext::default();
        ctx.vk_config.gammaramp = true; // pretend it was enabled
        ctx.save_original_gamma();
        // On Linux/Wayland, save_original_gamma should disable hw gamma
        assert!(!ctx.vk_config.gammaramp);
    }
}

/// Generate a 64x64 RGBA Quake 2 window icon.
///
/// Green "Q" with "II" roman numerals inside — matching the classic Quake 2 style.
/// Same design as the exe icon generated in build.rs.
fn generate_q2_icon() -> Option<Icon> {
    const SIZE: u32 = 64;
    let center = SIZE as f32 / 2.0;
    let scale = 1.0f32; // 64px reference
    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];

    let outer_r = center - 3.0 * scale;
    let ring_thick = 8.0 * scale;
    let inner_r = outer_r - ring_thick;
    let bg_r = center - 1.0 * scale;

    let bg = [12u8, 16, 12];
    let q_green = [0u8, 200, 40];
    let q_bright = [80u8, 255, 100];
    let q_dark = [0u8, 120, 20];

    let tail_angle: f32 = std::f32::consts::FRAC_PI_4;
    let tail_cos = tail_angle.cos();
    let tail_sin = tail_angle.sin();
    let tail_width = 4.5 * scale;

    let ii_height = ring_thick * 1.5;
    let ii_bar_width = 2.2 * scale;
    let ii_gap = 3.0 * scale;
    let ii_y_center = center - 0.5 * scale;

    fn lerp(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t.clamp(0.0, 1.0) }
    fn ablend(pixel: &mut [u8], r: f32, g: f32, b: f32, a: f32) {
        let old_a = pixel[3] as f32 / 255.0;
        let new_a = a / 255.0;
        let out_a = new_a + old_a * (1.0 - new_a);
        if out_a > 0.001 {
            pixel[0] = ((r * new_a + pixel[0] as f32 * old_a * (1.0 - new_a)) / out_a).clamp(0.0, 255.0) as u8;
            pixel[1] = ((g * new_a + pixel[1] as f32 * old_a * (1.0 - new_a)) / out_a).clamp(0.0, 255.0) as u8;
            pixel[2] = ((b * new_a + pixel[2] as f32 * old_a * (1.0 - new_a)) / out_a).clamp(0.0, 255.0) as u8;
            pixel[3] = (out_a * 255.0).clamp(0.0, 255.0) as u8;
        }
    }

    for y in 0..SIZE {
        for x in 0..SIZE {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let dx = px - center;
            let dy = py - center;
            let dist = (dx * dx + dy * dy).sqrt();
            let idx = ((y * SIZE + x) * 4) as usize;

            if dist < bg_r {
                let aa = (bg_r - dist).clamp(0.0, 1.5) / 1.5;
                rgba[idx] = bg[0]; rgba[idx+1] = bg[1]; rgba[idx+2] = bg[2];
                rgba[idx+3] = (aa * 255.0) as u8;
            }

            let ring_alpha = ((outer_r - dist).clamp(0.0, 1.5) / 1.5)
                .min((dist - inner_r).clamp(0.0, 1.5) / 1.5);
            if ring_alpha > 0.0 {
                let light = ((-dx - dy) / (center * 2.0) + 0.5).clamp(0.15, 1.0);
                ablend(&mut rgba[idx..idx+4],
                    lerp(q_green[0] as f32, q_bright[0] as f32, (light - 0.4) * 0.8),
                    lerp(q_green[1] as f32, q_bright[1] as f32, (light - 0.4) * 0.8),
                    lerp(q_green[2] as f32, q_bright[2] as f32, (light - 0.4) * 0.8),
                    ring_alpha * 255.0);
            }

            let tail_proj = dx * tail_cos + dy * tail_sin;
            let tail_perp = (-dx * tail_sin + dy * tail_cos).abs();
            if tail_proj > inner_r * 0.2 && tail_proj < outer_r + 7.0 * scale && tail_perp < tail_width {
                let ta = ((tail_width - tail_perp) / (1.5*scale)).clamp(0.0,1.0)
                    * ((outer_r + 7.0*scale - tail_proj) / (2.0*scale)).clamp(0.0,1.0)
                    * ((tail_proj - inner_r * 0.2) / (2.0*scale)).clamp(0.0,1.0);
                if ta > 0.01 {
                    let f = ((tail_width - tail_perp) / (1.5*scale)).clamp(0.0,1.0);
                    ablend(&mut rgba[idx..idx+4],
                        lerp(q_dark[0] as f32, q_green[0] as f32, f),
                        lerp(q_dark[1] as f32, q_green[1] as f32, f),
                        lerp(q_dark[2] as f32, q_green[2] as f32, f),
                        ta * 255.0);
                }
            }

            let fy = py - ii_y_center;
            if fy.abs() < ii_height / 2.0 && dist < inner_r - 1.0 * scale {
                let left_bar_x = center - ii_gap / 2.0 - ii_bar_width / 2.0;
                let right_bar_x = center + ii_gap / 2.0 + ii_bar_width / 2.0;
                let bl = ((ii_bar_width / 2.0 - (px - left_bar_x).abs()) / scale).clamp(0.0, 1.0);
                let br = ((ii_bar_width / 2.0 - (px - right_bar_x).abs()) / scale).clamp(0.0, 1.0);
                let by = ((ii_height / 2.0 - fy.abs()) / scale).clamp(0.0, 1.0);
                let bar_a = bl.max(br) * by;

                let serif_h = 1.5 * scale;
                let serif_extra = 1.5 * scale;
                let near_top = (ii_height / 2.0 - fy.abs()) < serif_h;
                if near_top {
                    let sl = ((ii_bar_width / 2.0 + serif_extra - (px - left_bar_x).abs()) / scale).clamp(0.0, 1.0);
                    let sr = ((ii_bar_width / 2.0 + serif_extra - (px - right_bar_x).abs()) / scale).clamp(0.0, 1.0);
                    let sy = ((serif_h - (ii_height / 2.0 - fy.abs())) / scale).clamp(0.0, 1.0);
                    let sa = sl.max(sr) * by * sy;
                    if sa > bar_a && sa > 0.01 {
                        let l = 0.7 + 0.3 * (1.0 - fy.abs() / (ii_height / 2.0));
                        ablend(&mut rgba[idx..idx+4], q_bright[0] as f32*l, q_bright[1] as f32*l, q_bright[2] as f32*l, sa * 255.0);
                    }
                }
                if bar_a > 0.01 {
                    let l = 0.7 + 0.3 * (1.0 - fy.abs() / (ii_height / 2.0));
                    ablend(&mut rgba[idx..idx+4], q_bright[0] as f32*l, q_bright[1] as f32*l, q_bright[2] as f32*l, bar_a * 255.0);
                }
            }
        }
    }

    Icon::from_rgba(rgba, SIZE, SIZE).ok()
}
