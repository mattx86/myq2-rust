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
        match generate_q2_icon() {
            Some(icon) => {
                eprintln!("[ICON] Setting 128x128 window icon");
                window.set_window_icon(Some(icon));
            }
            None => {
                eprintln!("[ICON] generate_q2_icon() returned None - icon not set!");
            }
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

/// Generate a 128x128 RGBA Quake 2 window icon.
///
/// Uses Yamagi Q2 icon SVG bezier path data for accurate shape.
/// Same design as the exe icon in build.rs.
fn generate_q2_icon() -> Option<Icon> {
    const SIZE: u32 = 128;

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
    fn sd_polygon(px: f32, py: f32, v: &[(f32, f32)]) -> f32 {
        let n = v.len();
        let mut d = (px - v[0].0) * (px - v[0].0) + (py - v[0].1) * (py - v[0].1);
        let mut s = 1.0f32;
        let mut j = n - 1;
        for i in 0..n {
            let ex = v[j].0 - v[i].0;
            let ey = v[j].1 - v[i].1;
            let wx = px - v[i].0;
            let wy = py - v[i].1;
            let dot_ee = ex * ex + ey * ey;
            let h = if dot_ee > 0.0 {
                ((wx * ex + wy * ey) / dot_ee).clamp(0.0, 1.0)
            } else { 0.0 };
            let dx = wx - ex * h;
            let dy = wy - ey * h;
            d = d.min(dx * dx + dy * dy);
            let c1 = py >= v[i].1;
            let c2 = py < v[j].1;
            let c3 = ex * wy > ey * wx;
            if (c1 && c2 && c3) || (!c1 && !c2 && !c3) { s = -s; }
            j = i;
        }
        s * d.sqrt()
    }
    fn flatten_beziers(start: (f32,f32), segs: &[[f32;6]], steps: usize) -> Vec<(f32,f32)> {
        let mut v = Vec::with_capacity(segs.len() * steps);
        let mut c = start;
        for s in segs {
            let p0 = (c.0/128.0-1.0, 1.0-c.1/128.0);
            let p1 = (s[0]/128.0-1.0, 1.0-s[1]/128.0);
            let p2 = (s[2]/128.0-1.0, 1.0-s[3]/128.0);
            let p3 = (s[4]/128.0-1.0, 1.0-s[5]/128.0);
            for i in 0..steps {
                let t = i as f32 / steps as f32;
                let u = 1.0 - t;
                v.push((
                    u*u*u*p0.0+3.0*u*u*t*p1.0+3.0*u*t*t*p2.0+t*t*t*p3.0,
                    u*u*u*p0.1+3.0*u*u*t*p1.1+3.0*u*t*t*p2.1+t*t*t*p3.1,
                ));
            }
            c = (s[4], s[5]);
        }
        v
    }

    // SVG path data from Yamagi Q2 icon (viewBox 0 0 256 256)
    #[rustfmt::skip]
    let outer_segs: [[f32;6]; 108] = [
        [166.83,4.67, 167.73,4.65, 168.64,4.63],
        [178.90,10.42, 189.31,16.64, 196.76,25.99],
        [202.91,32.89, 208.08,40.69, 211.69,49.21],
        [213.80,53.95, 216.37,58.73, 216.49,64.05],
        [216.44,66.82, 217.86,69.27, 218.62,71.86],
        [219.55,74.35, 219.26,77.06, 219.47,79.67],
        [220.20,81.23, 220.50,82.93, 220.61,84.64],
        [220.12,86.00, 219.69,87.37, 219.31,88.76],
        [220.18,90.67, 220.56,92.74, 220.72,94.82],
        [219.07,96.87, 217.56,99.03, 216.06,101.18],
        [216.90,101.99, 217.74,102.80, 218.59,103.62],
        [215.42,115.29, 210.62,126.65, 203.57,136.54],
        [200.30,141.66, 195.38,145.40, 191.95,150.41],
        [191.17,150.48, 189.60,150.62, 188.81,150.69],
        [188.57,148.95, 188.32,147.23, 188.03,145.50],
        [187.66,147.36, 187.29,149.23, 186.84,151.07],
        [186.64,152.83, 187.20,155.17, 185.33,156.22],
        [180.25,159.98, 174.71,163.11, 169.09,166.01],
        [162.14,169.11, 154.91,171.57, 147.60,173.67],
        [148.96,180.02, 147.39,186.43, 147.83,192.83],
        [148.07,202.36, 145.50,211.59, 144.49,221.00],
        [142.76,228.57, 142.03,236.31, 141.34,244.03],
        [141.15,246.67, 140.38,249.23, 139.30,251.64],
        [138.93,250.74, 138.20,248.93, 137.83,248.03],
        [134.82,234.62, 134.97,220.74, 131.94,207.33],
        [131.55,203.53, 130.98,199.76, 130.59,195.96],
        [130.24,193.12, 133.45,190.66, 131.84,187.87],
        [130.20,183.87, 130.71,179.49, 130.72,175.28],
        [128.60,175.25, 126.49,175.23, 124.37,175.21],
        [123.92,179.97, 124.79,184.81, 123.64,189.50],
        [123.15,191.31, 123.61,193.15, 123.91,194.94],
        [123.78,198.93, 124.00,202.95, 123.20,206.89],
        [122.19,211.89, 120.63,216.82, 120.46,221.94],
        [120.19,226.99, 118.67,231.88, 118.46,236.94],
        [118.25,242.16, 117.68,247.44, 115.91,252.39],
        [114.18,249.81, 114.07,246.66, 113.66,243.68],
        [113.11,239.43, 111.77,235.31, 111.54,231.02],
        [111.21,223.62, 109.49,216.37, 109.00,208.98],
        [107.85,203.54, 106.97,198.03, 107.22,192.44],
        [107.81,192.41, 109.00,192.34, 109.59,192.31],
        [108.66,191.02, 107.39,189.80, 107.39,188.08],
        [106.49,183.06, 106.92,177.94, 106.69,172.87],
        [103.15,171.96, 99.54,171.23, 96.17,169.80],
        [95.66,168.37, 95.41,166.87, 95.09,165.39],
        [92.46,165.52, 89.87,166.39, 87.23,166.18],
        [81.05,163.47, 75.83,159.10, 69.95,155.86],
        [68.88,153.58, 67.80,151.30, 66.56,149.11],
        [66.94,147.92, 67.33,146.74, 67.71,145.56],
        [66.67,146.78, 65.98,148.85, 64.00,148.69],
        [59.22,146.06, 55.94,141.54, 52.53,137.42],
        [49.29,133.32, 45.72,129.17, 44.35,124.01],
        [44.16,122.24, 46.39,121.72, 47.35,120.61],
        [44.34,120.99, 41.23,119.84, 40.06,116.85],
        [35.34,106.14, 36.30,94.11, 34.37,82.78],
        [34.69,82.39, 35.33,81.62, 35.65,81.23],
        [34.48,70.99, 39.38,61.65, 41.75,52.00],
        [42.12,51.55, 42.86,50.66, 43.22,50.21],
        [43.75,47.97, 44.40,45.71, 45.83,43.85],
        [47.59,41.44, 48.35,38.42, 50.41,36.23],
        [53.63,32.69, 55.81,28.37, 58.92,24.74],
        [59.48,24.60, 60.59,24.30, 61.15,24.15],
        [61.31,23.60, 61.62,22.48, 61.78,21.92],
        [65.10,19.13, 68.22,15.94, 72.25,14.16],
        [77.49,9.56, 83.79,5.09, 91.13,5.58],
        [90.56,9.16, 86.88,10.29, 84.14,11.82],
        [71.67,18.23, 61.44,28.94, 55.82,41.81],
        [54.06,45.90, 52.11,49.93, 50.90,54.24],
        [49.00,58.58, 49.30,63.42, 48.26,67.98],
        [47.05,72.28, 48.80,76.64, 48.21,81.01],
        [48.36,84.15, 49.02,87.42, 47.42,90.33],
        [46.71,90.45, 45.30,90.68, 44.59,90.80],
        [46.53,91.42, 48.48,92.03, 50.42,92.66],
        [50.45,94.06, 50.49,95.47, 50.53,96.87],
        [54.67,104.94, 57.62,113.68, 63.05,121.03],
        [64.87,123.15, 66.84,125.14, 68.67,127.26],
        [70.74,129.95, 73.97,131.28, 76.41,133.57],
        [78.47,135.53, 80.67,137.35, 82.98,139.03],
        [83.96,140.93, 85.05,142.78, 86.32,144.51],
        [88.47,142.34, 91.20,144.18, 93.56,144.82],
        [95.62,146.03, 97.80,147.02, 100.05,147.85],
        [100.57,149.50, 101.11,151.14, 101.66,152.78],
        [103.55,151.87, 105.42,150.97, 107.31,150.08],
        [107.29,148.98, 107.27,147.88, 107.26,146.77],
        [107.71,146.30, 108.62,145.36, 109.07,144.89],
        [108.64,144.43, 107.77,143.51, 107.34,143.06],
        [107.64,134.90, 107.51,126.69, 105.27,118.78],
        [102.29,117.83, 99.27,117.03, 96.25,116.23],
        [96.33,114.98, 96.43,113.73, 96.54,112.48],
        [105.70,113.68, 115.10,114.31, 124.25,112.70],
        [130.18,111.99, 136.05,113.79, 141.99,113.39],
        [148.02,113.44, 154.39,111.15, 160.18,113.77],
        [158.05,117.99, 152.11,115.60, 149.59,119.45],
        [146.20,124.71, 148.85,131.37, 146.86,137.05],
        [147.72,141.23, 147.75,145.52, 147.65,149.77],
        [158.44,148.05, 167.98,142.09, 176.61,135.68],
        [181.54,131.62, 186.54,127.47, 190.18,122.16],
        [192.57,120.06, 194.69,117.71, 195.93,114.73],
        [196.49,114.59, 197.63,114.31, 198.20,114.16],
        [199.57,108.17, 202.51,102.70, 204.36,96.87],
        [205.77,90.29, 207.53,83.71, 207.60,76.94],
        [208.82,76.14, 210.05,75.33, 211.29,74.54],
        [210.40,74.52, 208.63,74.47, 207.75,74.45],
        [207.10,70.66, 206.73,66.83, 206.54,63.00],
        [206.23,56.65, 203.14,50.87, 202.46,44.59],
        [199.20,41.58, 198.16,37.10, 195.34,33.78],
        [191.61,28.83, 188.17,23.55, 183.19,19.73],
        [177.24,15.10, 171.50,9.90, 164.38,7.14],
        [164.77,6.52, 165.55,5.30, 165.94,4.69],
    ];
    #[rustfmt::skip]
    let inner_segs: [[f32;6]; 7] = [
        [125.79,122.33, 124.75,125.06, 124.41,127.95],
        [123.87,131.89, 124.65,135.87, 124.21,139.82],
        [123.75,143.61, 123.61,147.43, 123.41,151.25],
        [126.06,151.29, 128.70,151.32, 131.35,151.35],
        [131.28,144.57, 131.15,137.79, 131.31,131.01],
        [131.52,127.09, 130.38,123.27, 129.27,119.55],
        [128.41,119.56, 127.54,119.58, 126.68,119.59],
    ];

    let mut outer_poly = flatten_beziers((165.94, 4.69), &outer_segs, 4);
    let mut inner_poly = flatten_beziers((126.68, 119.59), &inner_segs, 4);

    // Widen the "II" prong gap for readability at small sizes
    for v in outer_poly.iter_mut() {
        if v.1 < 0.15 && v.0.abs() < 0.25 {
            let offset = 0.06;
            if v.0 >= 0.0 { v.0 += offset; } else { v.0 -= offset; }
        }
    }
    for v in inner_poly.iter_mut() {
        let offset = 0.06;
        if v.0 >= 0.0 { v.0 += offset; } else { v.0 -= offset; }
    }

    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];
    let px_size = 2.0 / SIZE as f32;
    let aa = px_size * 1.5;

    for y in 0..SIZE {
        for x in 0..SIZE {
            let nx = (x as f32 + 0.5) / SIZE as f32 * 2.0 - 1.0;
            let ny = 1.0 - (y as f32 + 0.5) / SIZE as f32 * 2.0;
            let idx = ((y * SIZE + x) * 4) as usize;

            let d = sd_polygon(nx, ny, &outer_poly)
                .max(-sd_polygon(nx, ny, &inner_poly));

            // Dark outline for contrast on any background
            let glow_a = ((aa * 2.0 - d) / (aa * 2.0)).clamp(0.0, 1.0)
                       - ((-d) / aa).clamp(0.0, 1.0);
            if glow_a > 0.01 {
                ablend(&mut rgba[idx..idx + 4], 5.0, 10.0, 30.0, glow_a * 180.0);
            }

            let shape_a = ((-d) / aa).clamp(0.0, 1.0);

            if shape_a > 0.0 {
                let t = ((ny + 0.97) / 1.93).clamp(0.0, 1.0);
                let mut r = lerp(20.0, 110.0, t);
                let mut g = lerp(50.0, 190.0, t);
                let mut b = lerp(130.0, 255.0, t);

                let sdx = nx;
                let sdy = ny - 0.34;
                let spec_dist = (sdx * sdx + sdy * sdy).sqrt();
                let spec = ((0.35 - spec_dist) / 0.35).clamp(0.0, 1.0).powf(2.0) * 0.6;
                r += spec * 200.0;
                g += spec * 180.0;
                b += spec * 120.0;

                let inner_d = (-d).clamp(0.0, 0.03);
                let edge = (1.0 - inner_d / 0.03) * 0.35;
                r += edge * 120.0;
                g += edge * 150.0;
                b += edge * 100.0;

                ablend(
                    &mut rgba[idx..idx + 4],
                    r.clamp(0.0, 255.0),
                    g.clamp(0.0, 255.0),
                    b.clamp(0.0, 255.0),
                    shape_a * 255.0,
                );
            }
        }
    }

    Icon::from_rgba(rgba, SIZE, SIZE).ok()
}
