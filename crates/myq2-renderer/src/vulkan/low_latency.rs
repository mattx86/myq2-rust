//! Low Latency Rendering (VK_NV_low_latency2)
//!
//! NVIDIA Reflex-like low latency mode:
//! - Reduce input-to-display latency
//! - Frame timing optimization
//! - Sleep before rendering to minimize queue depth
//! - Latency markers for measurement

/// Low latency capabilities.
#[derive(Debug, Clone, Default)]
pub struct LowLatencyCapabilities {
    /// Whether low latency mode is supported.
    pub supported: bool,
    /// Minimum supported sleep duration in nanoseconds.
    pub min_sleep_duration_ns: u64,
}

/// Query low latency capabilities.
pub fn query_capabilities(ctx: &super::context::VulkanContext) -> LowLatencyCapabilities {
    // Check for VK_NV_low_latency2 extension
    let extensions = unsafe {
        ctx.instance
            .enumerate_device_extension_properties(ctx.physical_device)
            .unwrap_or_default()
    };

    let has_extension = extensions.iter().any(|ext| {
        let name = unsafe { std::ffi::CStr::from_ptr(ext.extension_name.as_ptr()) };
        name.to_str().map(|s| s == "VK_NV_low_latency2").unwrap_or(false)
    });

    LowLatencyCapabilities {
        supported: has_extension,
        min_sleep_duration_ns: 100_000, // 100 microseconds typical minimum
    }
}

/// Low latency mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LowLatencyMode {
    /// Low latency disabled.
    Off,
    /// Low latency enabled.
    On,
    /// Low latency with boost (higher power, lower latency).
    Boost,
}

/// Latency marker type for timing measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatencyMarker {
    /// Simulation start (game logic begins).
    SimulationStart,
    /// Simulation end.
    SimulationEnd,
    /// Render submit start.
    RenderSubmitStart,
    /// Render submit end.
    RenderSubmitEnd,
    /// Present start.
    PresentStart,
    /// Present end.
    PresentEnd,
    /// Input sample (when input is read).
    InputSample,
    /// Trigger flash (for latency measurement tools).
    TriggerFlash,
    /// PC latency ping (end-to-end measurement).
    PcLatencyPing,
    /// Out of band render submit start.
    OutOfBandRenderSubmitStart,
    /// Out of band render submit end.
    OutOfBandRenderSubmitEnd,
    /// Out of band present start.
    OutOfBandPresentStart,
    /// Out of band present end.
    OutOfBandPresentEnd,
}

impl LatencyMarker {
    /// Convert to Vulkan marker type.
    pub fn to_vk(&self) -> u32 {
        match self {
            LatencyMarker::SimulationStart => 0,
            LatencyMarker::SimulationEnd => 1,
            LatencyMarker::RenderSubmitStart => 2,
            LatencyMarker::RenderSubmitEnd => 3,
            LatencyMarker::PresentStart => 4,
            LatencyMarker::PresentEnd => 5,
            LatencyMarker::InputSample => 6,
            LatencyMarker::TriggerFlash => 7,
            LatencyMarker::PcLatencyPing => 8,
            LatencyMarker::OutOfBandRenderSubmitStart => 9,
            LatencyMarker::OutOfBandRenderSubmitEnd => 10,
            LatencyMarker::OutOfBandPresentStart => 11,
            LatencyMarker::OutOfBandPresentEnd => 12,
        }
    }
}

/// Low latency sleep info.
#[derive(Debug, Clone)]
pub struct SleepInfo {
    /// Target frame time in nanoseconds.
    pub target_frame_time_ns: u64,
    /// Whether to use boost mode.
    pub boost: bool,
    /// Minimum interval between sleeps.
    pub min_interval_us: u32,
}

impl Default for SleepInfo {
    fn default() -> Self {
        Self {
            target_frame_time_ns: 16_666_667, // ~60 FPS
            boost: false,
            min_interval_us: 1000,
        }
    }
}

impl SleepInfo {
    /// Create for target FPS.
    pub fn for_fps(fps: f32) -> Self {
        Self {
            target_frame_time_ns: (1_000_000_000.0 / fps) as u64,
            ..Default::default()
        }
    }

    /// Enable boost mode.
    pub fn with_boost(mut self) -> Self {
        self.boost = true;
        self
    }
}

/// Latency timing report.
#[derive(Debug, Clone, Default)]
pub struct LatencyTimings {
    /// Frame ID.
    pub frame_id: u64,
    /// Input sample time.
    pub input_sample_time_us: u64,
    /// Simulation start time.
    pub sim_start_time_us: u64,
    /// Simulation end time.
    pub sim_end_time_us: u64,
    /// Render submit start time.
    pub render_submit_start_time_us: u64,
    /// Render submit end time.
    pub render_submit_end_time_us: u64,
    /// Present start time.
    pub present_start_time_us: u64,
    /// Present end time.
    pub present_end_time_us: u64,
    /// GPU render start time.
    pub gpu_render_start_time_us: u64,
    /// GPU render end time.
    pub gpu_render_end_time_us: u64,
}

impl LatencyTimings {
    /// Calculate total input-to-display latency.
    pub fn total_latency_us(&self) -> u64 {
        self.present_end_time_us.saturating_sub(self.input_sample_time_us)
    }

    /// Calculate CPU latency (input to render submit).
    pub fn cpu_latency_us(&self) -> u64 {
        self.render_submit_end_time_us.saturating_sub(self.input_sample_time_us)
    }

    /// Calculate GPU latency.
    pub fn gpu_latency_us(&self) -> u64 {
        self.gpu_render_end_time_us.saturating_sub(self.gpu_render_start_time_us)
    }

    /// Calculate present latency.
    pub fn present_latency_us(&self) -> u64 {
        self.present_end_time_us.saturating_sub(self.present_start_time_us)
    }
}

/// Low latency configuration.
#[derive(Debug, Clone)]
pub struct LowLatencyConfig {
    /// Low latency mode.
    pub mode: LowLatencyMode,
    /// Target frame rate.
    pub target_fps: f32,
    /// Use frame pacing.
    pub frame_pacing: bool,
    /// Maximum queued frames.
    pub max_queued_frames: u32,
    /// Enable latency markers.
    pub latency_markers: bool,
}

impl Default for LowLatencyConfig {
    fn default() -> Self {
        Self {
            mode: LowLatencyMode::On,
            target_fps: 60.0,
            frame_pacing: true,
            max_queued_frames: 1,
            latency_markers: true,
        }
    }
}

/// Low latency manager.
pub struct LowLatencyManager {
    capabilities: LowLatencyCapabilities,
    config: LowLatencyConfig,
    frame_id: u64,
    last_frame_time_ns: u64,
    timing_history: Vec<LatencyTimings>,
}

impl LowLatencyManager {
    /// Create new low latency manager.
    pub fn new(ctx: &super::context::VulkanContext) -> Self {
        let capabilities = query_capabilities(ctx);

        Self {
            capabilities,
            config: LowLatencyConfig::default(),
            frame_id: 0,
            last_frame_time_ns: 0,
            timing_history: Vec::with_capacity(64),
        }
    }

    /// Check if low latency is supported.
    pub fn is_supported(&self) -> bool {
        self.capabilities.supported
    }

    /// Set configuration.
    pub fn set_config(&mut self, config: LowLatencyConfig) {
        self.config = config;
    }

    /// Get current configuration.
    pub fn config(&self) -> &LowLatencyConfig {
        &self.config
    }

    /// Begin new frame.
    pub fn begin_frame(&mut self) -> u64 {
        self.frame_id += 1;
        self.frame_id
    }

    /// Get current frame ID.
    pub fn frame_id(&self) -> u64 {
        self.frame_id
    }

    /// Calculate sleep duration before rendering.
    pub fn calculate_sleep_duration(&self, current_time_ns: u64) -> u64 {
        if self.config.mode == LowLatencyMode::Off || !self.capabilities.supported {
            return 0;
        }

        let target_frame_time = (1_000_000_000.0 / self.config.target_fps) as u64;
        let elapsed = current_time_ns.saturating_sub(self.last_frame_time_ns);

        if elapsed < target_frame_time {
            let remaining = target_frame_time - elapsed;
            // Sleep for a portion of remaining time to reduce queue depth
            // but leave some headroom for rendering
            remaining.saturating_sub(remaining / 4)
        } else {
            0
        }
    }

    /// Record frame end time.
    pub fn end_frame(&mut self, current_time_ns: u64) {
        self.last_frame_time_ns = current_time_ns;
    }

    /// Add timing record.
    pub fn add_timing(&mut self, timing: LatencyTimings) {
        if self.timing_history.len() >= 64 {
            self.timing_history.remove(0);
        }
        self.timing_history.push(timing);
    }

    /// Get average latency over recent frames.
    pub fn average_latency_us(&self) -> u64 {
        if self.timing_history.is_empty() {
            return 0;
        }

        let sum: u64 = self.timing_history.iter()
            .map(|t| t.total_latency_us())
            .sum();
        sum / self.timing_history.len() as u64
    }

    /// Get timing history.
    pub fn timing_history(&self) -> &[LatencyTimings] {
        &self.timing_history
    }

    /// Get sleep info for current configuration.
    pub fn get_sleep_info(&self) -> SleepInfo {
        SleepInfo {
            target_frame_time_ns: (1_000_000_000.0 / self.config.target_fps) as u64,
            boost: self.config.mode == LowLatencyMode::Boost,
            min_interval_us: 1000,
        }
    }
}

/// Frame pacing helper for consistent frame times.
pub struct FramePacer {
    target_frame_time_ns: u64,
    last_present_time_ns: u64,
    frame_times: [u64; 8],
    frame_index: usize,
}

impl FramePacer {
    /// Create new frame pacer for target FPS.
    pub fn new(target_fps: f32) -> Self {
        Self {
            target_frame_time_ns: (1_000_000_000.0 / target_fps) as u64,
            last_present_time_ns: 0,
            frame_times: [0; 8],
            frame_index: 0,
        }
    }

    /// Set target FPS.
    pub fn set_target_fps(&mut self, fps: f32) {
        self.target_frame_time_ns = (1_000_000_000.0 / fps) as u64;
    }

    /// Record present time.
    pub fn record_present(&mut self, time_ns: u64) {
        if self.last_present_time_ns > 0 {
            let frame_time = time_ns - self.last_present_time_ns;
            self.frame_times[self.frame_index] = frame_time;
            self.frame_index = (self.frame_index + 1) % 8;
        }
        self.last_present_time_ns = time_ns;
    }

    /// Get average frame time.
    pub fn average_frame_time_ns(&self) -> u64 {
        let sum: u64 = self.frame_times.iter().sum();
        sum / 8
    }

    /// Get current FPS.
    pub fn current_fps(&self) -> f32 {
        let avg = self.average_frame_time_ns();
        if avg > 0 {
            1_000_000_000.0 / avg as f32
        } else {
            0.0
        }
    }

    /// Calculate wait time to hit target frame time.
    pub fn calculate_wait(&self, current_time_ns: u64) -> u64 {
        if self.last_present_time_ns == 0 {
            return 0;
        }

        let elapsed = current_time_ns.saturating_sub(self.last_present_time_ns);
        if elapsed < self.target_frame_time_ns {
            self.target_frame_time_ns - elapsed
        } else {
            0
        }
    }

    /// Check if frame is late.
    pub fn is_frame_late(&self, current_time_ns: u64) -> bool {
        if self.last_present_time_ns == 0 {
            return false;
        }

        let elapsed = current_time_ns.saturating_sub(self.last_present_time_ns);
        elapsed > self.target_frame_time_ns
    }
}
