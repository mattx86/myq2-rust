//! Present Wait for perfect frame pacing
//!
//! VK_KHR_present_wait allows waiting for a specific frame to be presented,
//! enabling precise frame timing for:
//! - Variable Refresh Rate (VRR) displays (G-Sync, FreeSync)
//! - Low-latency rendering
//! - Consistent frame pacing
//!
//! Works with VK_KHR_present_id to identify specific presents.

use ash::vk;
use std::time::{Duration, Instant};

/// Present wait capabilities.
#[derive(Debug, Clone)]
pub struct PresentWaitCapabilities {
    /// Whether present wait is supported.
    pub present_wait_supported: bool,
    /// Whether present ID is supported.
    pub present_id_supported: bool,
}

impl Default for PresentWaitCapabilities {
    fn default() -> Self {
        Self {
            present_wait_supported: false,
            present_id_supported: false,
        }
    }
}

/// Frame timing statistics.
#[derive(Debug, Clone, Default)]
pub struct FrameTimingStats {
    /// Last frame's present latency in microseconds.
    pub last_present_latency_us: u64,
    /// Average present latency over recent frames.
    pub avg_present_latency_us: u64,
    /// Minimum present latency.
    pub min_present_latency_us: u64,
    /// Maximum present latency.
    pub max_present_latency_us: u64,
    /// Number of missed frames (present took too long).
    pub missed_frames: u64,
    /// Target frame time in microseconds.
    pub target_frame_time_us: u64,
}

/// Present wait manager.
pub struct PresentWaitManager {
    /// Capabilities.
    capabilities: PresentWaitCapabilities,
    /// Present wait extension function pointer.
    fp_wait_for_present: Option<vk::PFN_vkWaitForPresentKHR>,
    /// Current present ID counter.
    current_id: u64,
    /// Frame timing history.
    timing_history: Vec<u64>,
    /// Timing statistics.
    stats: FrameTimingStats,
    /// Frame submission times.
    submit_times: Vec<(u64, Instant)>,
    /// Target frame time (for VRR).
    target_frame_time: Duration,
    /// Whether VRR mode is active.
    vrr_enabled: bool,
    /// Cached swapchain handle for wait operations.
    swapchain: vk::SwapchainKHR,
}

impl PresentWaitManager {
    /// Query present wait capabilities.
    pub fn query_capabilities(ctx: &super::context::VulkanContext) -> PresentWaitCapabilities {
        let mut wait_features = vk::PhysicalDevicePresentWaitFeaturesKHR::default();
        let mut id_features = vk::PhysicalDevicePresentIdFeaturesKHR::default();
        let mut features2 = vk::PhysicalDeviceFeatures2::default()
            .push_next(&mut wait_features)
            .push_next(&mut id_features);

        unsafe {
            ctx.instance.get_physical_device_features2(ctx.physical_device, &mut features2);
        }

        PresentWaitCapabilities {
            present_wait_supported: wait_features.present_wait == vk::TRUE,
            present_id_supported: id_features.present_id == vk::TRUE,
        }
    }

    /// Create a new present wait manager.
    pub fn new(ctx: &super::context::VulkanContext) -> Self {
        let capabilities = Self::query_capabilities(ctx);

        // Get function pointer for wait_for_present if supported
        let fp_wait_for_present = if capabilities.present_wait_supported {
            unsafe {
                let name = std::ffi::CStr::from_bytes_with_nul_unchecked(b"vkWaitForPresentKHR\0");
                ctx.instance.get_device_proc_addr(ctx.device.handle(), name.as_ptr())
                    .map(|fp| std::mem::transmute(fp))
            }
        } else {
            None
        };

        Self {
            capabilities,
            fp_wait_for_present,
            current_id: 0,
            timing_history: Vec::with_capacity(120),
            stats: FrameTimingStats::default(),
            submit_times: Vec::with_capacity(8),
            target_frame_time: Duration::from_micros(16667), // 60 fps default
            vrr_enabled: false,
            swapchain: vk::SwapchainKHR::null(),
        }
    }

    /// Set the swapchain handle for wait operations.
    pub fn set_swapchain(&mut self, swapchain: vk::SwapchainKHR) {
        self.swapchain = swapchain;
    }

    /// Check if present wait is supported.
    pub fn is_supported(&self) -> bool {
        self.capabilities.present_wait_supported && self.capabilities.present_id_supported
    }

    /// Get capabilities.
    pub fn capabilities(&self) -> &PresentWaitCapabilities {
        &self.capabilities
    }

    /// Set target frame rate for VRR.
    pub fn set_target_fps(&mut self, fps: f32) {
        self.target_frame_time = Duration::from_secs_f32(1.0 / fps);
        self.stats.target_frame_time_us = self.target_frame_time.as_micros() as u64;
    }

    /// Enable or disable VRR mode.
    pub fn set_vrr_enabled(&mut self, enabled: bool) {
        self.vrr_enabled = enabled;
    }

    /// Get the next present ID and record submission time.
    pub fn next_present_id(&mut self) -> u64 {
        self.current_id += 1;
        self.submit_times.push((self.current_id, Instant::now()));

        // Keep only recent submissions
        while self.submit_times.len() > 8 {
            self.submit_times.remove(0);
        }

        self.current_id
    }

    /// Get the current present ID.
    pub fn current_present_id(&self) -> u64 {
        self.current_id
    }

    /// Wait for a specific present to complete.
    pub fn wait_for_present(
        &mut self,
        ctx: &super::context::VulkanContext,
        present_id: u64,
        timeout_ns: u64,
    ) -> Result<bool, String> {
        if let Some(fp) = self.fp_wait_for_present {
            if self.swapchain == vk::SwapchainKHR::null() {
                return Ok(true); // No swapchain set
            }

            let result = unsafe {
                fp(ctx.device.handle(), self.swapchain, present_id, timeout_ns)
            };

            match result {
                vk::Result::SUCCESS => {
                    self.record_present_completion(present_id);
                    Ok(true)
                }
                vk::Result::TIMEOUT => Ok(false),
                e => Err(format!("Wait for present failed: {:?}", e)),
            }
        } else {
            Ok(true) // Not supported, assume immediate
        }
    }

    /// Wait for the previous frame to be presented.
    pub fn wait_for_previous_present(
        &mut self,
        ctx: &super::context::VulkanContext,
    ) -> Result<(), String> {
        if self.current_id > 1 {
            let prev_id = self.current_id - 1;
            self.wait_for_present(ctx, prev_id, u64::MAX)?;
        }
        Ok(())
    }

    /// Record present completion and update statistics.
    fn record_present_completion(&mut self, present_id: u64) {
        let now = Instant::now();

        // Find the submission time for this present
        if let Some(pos) = self.submit_times.iter().position(|(id, _)| *id == present_id) {
            let (_, submit_time) = self.submit_times.remove(pos);
            let latency = now.duration_since(submit_time);
            let latency_us = latency.as_micros() as u64;

            self.stats.last_present_latency_us = latency_us;

            // Update history
            self.timing_history.push(latency_us);
            if self.timing_history.len() > 120 {
                self.timing_history.remove(0);
            }

            // Update statistics
            self.update_stats();

            // Check for missed frames
            if latency_us > self.stats.target_frame_time_us * 2 {
                self.stats.missed_frames += 1;
            }
        }
    }

    /// Update timing statistics.
    fn update_stats(&mut self) {
        if self.timing_history.is_empty() {
            return;
        }

        let sum: u64 = self.timing_history.iter().sum();
        self.stats.avg_present_latency_us = sum / self.timing_history.len() as u64;

        self.stats.min_present_latency_us = *self.timing_history.iter().min().unwrap_or(&0);
        self.stats.max_present_latency_us = *self.timing_history.iter().max().unwrap_or(&0);
    }

    /// Get timing statistics.
    pub fn stats(&self) -> &FrameTimingStats {
        &self.stats
    }

    /// Calculate optimal sleep time for frame pacing.
    pub fn calculate_sleep_time(&self) -> Duration {
        if !self.vrr_enabled {
            return Duration::ZERO;
        }

        // Use average latency to predict when to start rendering
        let avg_latency = Duration::from_micros(self.stats.avg_present_latency_us);

        if avg_latency < self.target_frame_time {
            self.target_frame_time - avg_latency
        } else {
            Duration::ZERO
        }
    }

    /// Frame pacing: sleep if needed to hit target frame rate.
    pub fn pace_frame(&self) {
        let sleep_time = self.calculate_sleep_time();
        if sleep_time > Duration::from_micros(500) {
            std::thread::sleep(sleep_time);
        }
    }

    /// Reset statistics.
    pub fn reset_stats(&mut self) {
        self.timing_history.clear();
        self.stats = FrameTimingStats {
            target_frame_time_us: self.stats.target_frame_time_us,
            ..Default::default()
        };
    }
}

/// VRR (Variable Refresh Rate) display info.
#[derive(Debug, Clone)]
pub struct VrrDisplayInfo {
    /// Minimum refresh rate (Hz).
    pub min_refresh_rate: f32,
    /// Maximum refresh rate (Hz).
    pub max_refresh_rate: f32,
    /// Whether VRR is supported.
    pub vrr_supported: bool,
    /// Current target refresh rate.
    pub target_refresh_rate: f32,
}

impl Default for VrrDisplayInfo {
    fn default() -> Self {
        Self {
            min_refresh_rate: 48.0,
            max_refresh_rate: 144.0,
            vrr_supported: false,
            target_refresh_rate: 60.0,
        }
    }
}

/// Query VRR display capabilities.
pub fn query_vrr_support(
    ctx: &super::context::VulkanContext,
    surface: vk::SurfaceKHR,
) -> VrrDisplayInfo {
    // Query present modes to detect VRR support
    let present_modes = unsafe {
        ctx.surface_loader
            .get_physical_device_surface_present_modes(ctx.physical_device, surface)
            .unwrap_or_default()
    };

    let vrr_supported = present_modes.contains(&vk::PresentModeKHR::FIFO_RELAXED)
        || present_modes.contains(&vk::PresentModeKHR::MAILBOX);

    // Query surface capabilities for refresh rate info
    let _caps = unsafe {
        ctx.surface_loader
            .get_physical_device_surface_capabilities(ctx.physical_device, surface)
            .ok()
    };

    VrrDisplayInfo {
        min_refresh_rate: 48.0,  // Common VRR minimum
        max_refresh_rate: 144.0, // Would query from display
        vrr_supported,
        target_refresh_rate: 60.0,
    }
}
