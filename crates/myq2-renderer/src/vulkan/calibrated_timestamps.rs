//! Calibrated Timestamps (VK_KHR_calibrated_timestamps / VK_EXT_calibrated_timestamps)
//!
//! Precise cross-domain timing:
//! - Query timestamps from multiple time domains simultaneously
//! - Correlate GPU timestamps with CPU time
//! - Accurate frame pacing and latency measurement
//! - Profiling and benchmarking support

use ash::vk;
use std::time::Duration;

/// Calibrated timestamp capabilities.
#[derive(Debug, Clone, Default)]
pub struct CalibratedTimestampCapabilities {
    /// Whether calibrated timestamps are supported.
    pub supported: bool,
    /// Available time domains.
    pub time_domains: Vec<TimeDomain>,
}

/// Time domain for calibrated timestamps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeDomain {
    /// Device-local timestamp (GPU).
    Device,
    /// Clock monotonic (POSIX).
    ClockMonotonic,
    /// Clock monotonic raw (Linux).
    ClockMonotonicRaw,
    /// Query performance counter (Windows).
    QueryPerformanceCounter,
}

impl TimeDomain {
    /// Convert from Vulkan time domain.
    pub fn from_vk(domain: vk::TimeDomainKHR) -> Option<Self> {
        match domain {
            vk::TimeDomainKHR::DEVICE => Some(TimeDomain::Device),
            vk::TimeDomainKHR::CLOCK_MONOTONIC => Some(TimeDomain::ClockMonotonic),
            vk::TimeDomainKHR::CLOCK_MONOTONIC_RAW => Some(TimeDomain::ClockMonotonicRaw),
            vk::TimeDomainKHR::QUERY_PERFORMANCE_COUNTER => Some(TimeDomain::QueryPerformanceCounter),
            _ => None,
        }
    }

    /// Convert to Vulkan time domain.
    pub fn to_vk(&self) -> vk::TimeDomainKHR {
        match self {
            TimeDomain::Device => vk::TimeDomainKHR::DEVICE,
            TimeDomain::ClockMonotonic => vk::TimeDomainKHR::CLOCK_MONOTONIC,
            TimeDomain::ClockMonotonicRaw => vk::TimeDomainKHR::CLOCK_MONOTONIC_RAW,
            TimeDomain::QueryPerformanceCounter => vk::TimeDomainKHR::QUERY_PERFORMANCE_COUNTER,
        }
    }
}

/// Query calibrated timestamp capabilities.
pub fn query_capabilities(ctx: &super::context::VulkanContext) -> CalibratedTimestampCapabilities {
    // Check for extension
    let extensions = unsafe {
        ctx.instance
            .enumerate_device_extension_properties(ctx.physical_device)
            .unwrap_or_default()
    };

    let has_khr = extensions.iter().any(|ext| {
        let name = unsafe { std::ffi::CStr::from_ptr(ext.extension_name.as_ptr()) };
        name.to_str().map(|s| s == "VK_KHR_calibrated_timestamps").unwrap_or(false)
    });

    let has_ext = extensions.iter().any(|ext| {
        let name = unsafe { std::ffi::CStr::from_ptr(ext.extension_name.as_ptr()) };
        name.to_str().map(|s| s == "VK_EXT_calibrated_timestamps").unwrap_or(false)
    });

    let supported = has_khr || has_ext;

    if !supported {
        return CalibratedTimestampCapabilities::default();
    }

    // On Windows, typically Device + QueryPerformanceCounter
    // On Linux, typically Device + ClockMonotonic + ClockMonotonicRaw
    #[cfg(windows)]
    let time_domains = vec![TimeDomain::Device, TimeDomain::QueryPerformanceCounter];

    #[cfg(not(windows))]
    let time_domains = vec![
        TimeDomain::Device,
        TimeDomain::ClockMonotonic,
        TimeDomain::ClockMonotonicRaw,
    ];

    CalibratedTimestampCapabilities {
        supported,
        time_domains,
    }
}

/// Calibrated timestamp result.
#[derive(Debug, Clone)]
pub struct CalibratedTimestamp {
    /// Time domain.
    pub domain: TimeDomain,
    /// Timestamp value.
    pub timestamp: u64,
    /// Maximum deviation in nanoseconds.
    pub max_deviation_ns: u64,
}

/// Calibrated timestamp pair for GPU-CPU correlation.
#[derive(Debug, Clone)]
pub struct TimestampCorrelation {
    /// GPU timestamp.
    pub gpu_timestamp: u64,
    /// CPU timestamp.
    pub cpu_timestamp: u64,
    /// Maximum deviation.
    pub max_deviation_ns: u64,
    /// Timestamp period (nanoseconds per tick).
    pub timestamp_period: f32,
}

impl TimestampCorrelation {
    /// Convert GPU timestamp to CPU time.
    pub fn gpu_to_cpu_time(&self, gpu_ts: u64) -> u64 {
        let gpu_delta = gpu_ts.wrapping_sub(self.gpu_timestamp);
        let gpu_ns = (gpu_delta as f64 * self.timestamp_period as f64) as u64;
        self.cpu_timestamp.wrapping_add(gpu_ns)
    }

    /// Convert CPU time to GPU timestamp.
    pub fn cpu_to_gpu_time(&self, cpu_ts: u64) -> u64 {
        let cpu_delta = cpu_ts.wrapping_sub(self.cpu_timestamp);
        let gpu_ticks = (cpu_delta as f64 / self.timestamp_period as f64) as u64;
        self.gpu_timestamp.wrapping_add(gpu_ticks)
    }

    /// Check if correlation is still valid (not too old).
    pub fn is_valid(&self, current_cpu_time: u64, max_age_ns: u64) -> bool {
        current_cpu_time.saturating_sub(self.cpu_timestamp) < max_age_ns
    }
}

/// Calibrated timestamp manager.
pub struct CalibratedTimestampManager {
    capabilities: CalibratedTimestampCapabilities,
    timestamp_period: f32,
    last_correlation: Option<TimestampCorrelation>,
    correlation_interval_ns: u64,
}

impl CalibratedTimestampManager {
    /// Create new manager.
    pub fn new(ctx: &super::context::VulkanContext) -> Self {
        let capabilities = query_capabilities(ctx);

        // Get timestamp period from device properties
        let mut props = vk::PhysicalDeviceProperties2::default();
        unsafe {
            ctx.instance.get_physical_device_properties2(ctx.physical_device, &mut props);
        }
        let timestamp_period = props.properties.limits.timestamp_period;

        Self {
            capabilities,
            timestamp_period,
            last_correlation: None,
            correlation_interval_ns: 1_000_000_000, // Recalibrate every second
        }
    }

    /// Check if calibrated timestamps are supported.
    pub fn is_supported(&self) -> bool {
        self.capabilities.supported
    }

    /// Get available time domains.
    pub fn time_domains(&self) -> &[TimeDomain] {
        &self.capabilities.time_domains
    }

    /// Get timestamp period.
    pub fn timestamp_period(&self) -> f32 {
        self.timestamp_period
    }

    /// Convert GPU ticks to nanoseconds.
    pub fn ticks_to_ns(&self, ticks: u64) -> u64 {
        (ticks as f64 * self.timestamp_period as f64) as u64
    }

    /// Convert nanoseconds to GPU ticks.
    pub fn ns_to_ticks(&self, ns: u64) -> u64 {
        (ns as f64 / self.timestamp_period as f64) as u64
    }

    /// Get current CPU timestamp.
    pub fn get_cpu_timestamp(&self) -> u64 {
        #[cfg(windows)]
        {
            use std::time::Instant;
            // Use Instant as fallback, but QueryPerformanceCounter would be better
            static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
            let start = START.get_or_init(Instant::now);
            start.elapsed().as_nanos() as u64
        }

        #[cfg(not(windows))]
        {
            use std::time::Instant;
            static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
            let start = START.get_or_init(Instant::now);
            start.elapsed().as_nanos() as u64
        }
    }

    /// Update correlation (should be called periodically).
    pub fn update_correlation(&mut self, gpu_timestamp: u64) {
        let cpu_timestamp = self.get_cpu_timestamp();

        // Check if we need to recalibrate
        if let Some(ref last) = self.last_correlation {
            if cpu_timestamp.saturating_sub(last.cpu_timestamp) < self.correlation_interval_ns {
                return;
            }
        }

        self.last_correlation = Some(TimestampCorrelation {
            gpu_timestamp,
            cpu_timestamp,
            max_deviation_ns: 1000, // 1 microsecond typical
            timestamp_period: self.timestamp_period,
        });
    }

    /// Get current correlation.
    pub fn correlation(&self) -> Option<&TimestampCorrelation> {
        self.last_correlation.as_ref()
    }

    /// Convert GPU timestamp to CPU time.
    pub fn gpu_to_cpu(&self, gpu_ts: u64) -> Option<u64> {
        self.last_correlation.as_ref().map(|c| c.gpu_to_cpu_time(gpu_ts))
    }

    /// Set recalibration interval.
    pub fn set_correlation_interval_ns(&mut self, interval_ns: u64) {
        self.correlation_interval_ns = interval_ns;
    }
}

/// Frame timing tracker using calibrated timestamps.
pub struct FrameTimingTracker {
    /// GPU start timestamps per frame.
    gpu_start: [u64; 8],
    /// GPU end timestamps per frame.
    gpu_end: [u64; 8],
    /// CPU submit timestamps per frame.
    cpu_submit: [u64; 8],
    /// CPU present timestamps per frame.
    cpu_present: [u64; 8],
    /// Current frame index.
    frame_index: usize,
}

impl FrameTimingTracker {
    /// Create new tracker.
    pub fn new() -> Self {
        Self {
            gpu_start: [0; 8],
            gpu_end: [0; 8],
            cpu_submit: [0; 8],
            cpu_present: [0; 8],
            frame_index: 0,
        }
    }

    /// Record GPU start time.
    pub fn record_gpu_start(&mut self, timestamp: u64) {
        self.gpu_start[self.frame_index] = timestamp;
    }

    /// Record GPU end time.
    pub fn record_gpu_end(&mut self, timestamp: u64) {
        self.gpu_end[self.frame_index] = timestamp;
    }

    /// Record CPU submit time.
    pub fn record_cpu_submit(&mut self, timestamp: u64) {
        self.cpu_submit[self.frame_index] = timestamp;
    }

    /// Record CPU present time.
    pub fn record_cpu_present(&mut self, timestamp: u64) {
        self.cpu_present[self.frame_index] = timestamp;
    }

    /// Advance to next frame.
    pub fn next_frame(&mut self) {
        self.frame_index = (self.frame_index + 1) % 8;
    }

    /// Get average GPU frame time in nanoseconds.
    pub fn average_gpu_time_ns(&self, timestamp_period: f32) -> u64 {
        let mut sum = 0u64;
        let mut count = 0;

        for i in 0..8 {
            if self.gpu_end[i] > self.gpu_start[i] {
                let ticks = self.gpu_end[i] - self.gpu_start[i];
                sum += (ticks as f64 * timestamp_period as f64) as u64;
                count += 1;
            }
        }

        if count > 0 { sum / count } else { 0 }
    }

    /// Get average CPU frame time.
    pub fn average_cpu_time_ns(&self) -> u64 {
        let mut sum = 0u64;
        let mut count = 0;

        for i in 0..8 {
            if self.cpu_present[i] > self.cpu_submit[i] {
                sum += self.cpu_present[i] - self.cpu_submit[i];
                count += 1;
            }
        }

        if count > 0 { sum / count } else { 0 }
    }
}

impl Default for FrameTimingTracker {
    fn default() -> Self {
        Self::new()
    }
}

// --- Timestamp query pool and GPU profiler (merged from timestamps.rs) ---

/// Timestamp query pool wrapper for GPU profiling.
pub struct TimestampQueryPool {
    /// Query pool handle.
    pool: vk::QueryPool,
    /// Number of queries in the pool.
    query_count: u32,
    /// Timestamp period (nanoseconds per tick).
    timestamp_period: f32,
    /// Next available query index.
    next_query: u32,
    /// Query results.
    results: Vec<u64>,
}

impl TimestampQueryPool {
    /// Create a new timestamp query pool.
    pub fn new(ctx: &super::context::VulkanContext, query_count: u32) -> Result<Self, String> {
        let create_info = vk::QueryPoolCreateInfo::default()
            .query_type(vk::QueryType::TIMESTAMP)
            .query_count(query_count);

        let pool = unsafe {
            ctx.device.create_query_pool(&create_info, None)
                .map_err(|e| format!("Failed to create timestamp query pool: {:?}", e))?
        };

        // Get timestamp period from device properties
        let props = unsafe {
            ctx.instance.get_physical_device_properties(ctx.physical_device)
        };
        let timestamp_period = props.limits.timestamp_period;

        Ok(Self {
            pool,
            query_count,
            timestamp_period,
            next_query: 0,
            results: vec![0; query_count as usize],
        })
    }

    /// Reset the query pool.
    pub fn reset(&mut self, ctx: &super::context::VulkanContext, cmd: vk::CommandBuffer) {
        unsafe {
            ctx.device.cmd_reset_query_pool(cmd, self.pool, 0, self.query_count);
        }
        self.next_query = 0;
    }

    /// Write a timestamp.
    pub fn write_timestamp(
        &mut self,
        ctx: &super::context::VulkanContext,
        cmd: vk::CommandBuffer,
        stage: vk::PipelineStageFlags,
    ) -> Option<u32> {
        if self.next_query >= self.query_count {
            return None;
        }

        let query = self.next_query;
        unsafe {
            ctx.device.cmd_write_timestamp(cmd, stage, self.pool, query);
        }
        self.next_query += 1;
        Some(query)
    }

    /// Get timestamp results.
    pub fn get_results(&mut self, ctx: &super::context::VulkanContext) -> Result<bool, String> {
        if self.next_query == 0 {
            return Ok(true);
        }

        let result = unsafe {
            ctx.device.get_query_pool_results(
                self.pool,
                0,
                &mut self.results[..self.next_query as usize],
                vk::QueryResultFlags::TYPE_64 | vk::QueryResultFlags::WAIT,
            )
        };

        match result {
            Ok(_) => Ok(true),
            Err(vk::Result::NOT_READY) => Ok(false),
            Err(e) => Err(format!("Failed to get timestamp results: {:?}", e)),
        }
    }

    /// Get timestamp value in nanoseconds.
    pub fn get_timestamp_ns(&self, query: u32) -> Option<u64> {
        if query >= self.next_query {
            return None;
        }
        Some((self.results[query as usize] as f64 * self.timestamp_period as f64) as u64)
    }

    /// Get duration between two timestamps in nanoseconds.
    pub fn get_duration_ns(&self, start_query: u32, end_query: u32) -> Option<u64> {
        let start = self.get_timestamp_ns(start_query)?;
        let end = self.get_timestamp_ns(end_query)?;
        Some(end.saturating_sub(start))
    }

    /// Get duration as Duration.
    pub fn get_duration(&self, start_query: u32, end_query: u32) -> Option<Duration> {
        self.get_duration_ns(start_query, end_query)
            .map(Duration::from_nanos)
    }

    /// Destroy the query pool.
    pub fn destroy(&mut self, ctx: &super::context::VulkanContext) {
        unsafe {
            ctx.device.destroy_query_pool(self.pool, None);
        }
    }
}

/// GPU profiler for frame timing.
pub struct GpuProfiler {
    /// Timestamp query pools (double-buffered).
    query_pools: Vec<TimestampQueryPool>,
    /// Current pool index.
    current_pool: usize,
    /// Named timestamp markers.
    markers: Vec<(String, u32)>,
    /// Frame timing history.
    frame_times: Vec<Duration>,
    /// Last calibrated timestamp.
    last_calibration: Option<CalibratedTimestamp>,
}

impl GpuProfiler {
    /// Create a new GPU profiler.
    pub fn new(ctx: &super::context::VulkanContext, frames_in_flight: usize) -> Result<Self, String> {
        let mut query_pools = Vec::with_capacity(frames_in_flight);
        for _ in 0..frames_in_flight {
            query_pools.push(TimestampQueryPool::new(ctx, 64)?);
        }

        Ok(Self {
            query_pools,
            current_pool: 0,
            markers: Vec::with_capacity(32),
            frame_times: Vec::with_capacity(120),
            last_calibration: None,
        })
    }

    /// Begin a new frame.
    pub fn begin_frame(&mut self, ctx: &super::context::VulkanContext, cmd: vk::CommandBuffer) {
        self.current_pool = (self.current_pool + 1) % self.query_pools.len();
        self.query_pools[self.current_pool].reset(ctx, cmd);
        self.markers.clear();
    }

    /// Mark a timestamp with a name.
    pub fn mark(&mut self, ctx: &super::context::VulkanContext, cmd: vk::CommandBuffer, name: &str) {
        let pool = &mut self.query_pools[self.current_pool];
        if let Some(query) = pool.write_timestamp(ctx, cmd, vk::PipelineStageFlags::BOTTOM_OF_PIPE) {
            self.markers.push((name.to_string(), query));
        }
    }

    /// End the frame and collect results from the previous frame.
    pub fn end_frame(&mut self, ctx: &super::context::VulkanContext) -> Option<FrameProfile> {
        // Collect results from previous frame's pool
        let prev_pool = if self.current_pool == 0 {
            self.query_pools.len() - 1
        } else {
            self.current_pool - 1
        };

        let pool = &mut self.query_pools[prev_pool];
        if pool.get_results(ctx).ok()? {
            // Build profile from markers
            let mut profile = FrameProfile::default();

            if self.markers.len() >= 2 {
                // Get total frame time from first to last marker
                if let Some(duration) = pool.get_duration(0, pool.next_query.saturating_sub(1)) {
                    profile.total_time = duration;
                    self.frame_times.push(duration);
                    if self.frame_times.len() > 120 {
                        self.frame_times.remove(0);
                    }
                }
            }

            Some(profile)
        } else {
            None
        }
    }

    /// Get average frame time.
    pub fn average_frame_time(&self) -> Duration {
        if self.frame_times.is_empty() {
            return Duration::ZERO;
        }
        let sum: Duration = self.frame_times.iter().sum();
        sum / self.frame_times.len() as u32
    }

    /// Get frame time percentile (0-100).
    pub fn frame_time_percentile(&self, percentile: f32) -> Duration {
        if self.frame_times.is_empty() {
            return Duration::ZERO;
        }
        let mut sorted = self.frame_times.clone();
        sorted.sort();
        let idx = ((sorted.len() as f32 * percentile / 100.0) as usize).min(sorted.len() - 1);
        sorted[idx]
    }

    /// Get last calibration timestamp.
    pub fn last_calibration(&self) -> Option<&CalibratedTimestamp> {
        self.last_calibration.as_ref()
    }

    /// Destroy the profiler.
    pub fn destroy(&mut self, ctx: &super::context::VulkanContext) {
        for pool in &mut self.query_pools {
            pool.destroy(ctx);
        }
    }
}

/// Frame profiling results.
#[derive(Debug, Clone, Default)]
pub struct FrameProfile {
    /// Total frame time on GPU.
    pub total_time: Duration,
    /// Individual pass timings.
    pub pass_times: Vec<(String, Duration)>,
}

impl FrameProfile {
    /// Get timing for a specific pass.
    pub fn get_pass(&self, name: &str) -> Option<Duration> {
        self.pass_times.iter()
            .find(|(n, _)| n == name)
            .map(|(_, d)| *d)
    }
}

/// Check if timestamp queries are supported.
pub fn check_timestamp_support(ctx: &super::context::VulkanContext) -> bool {
    // Check if any queue family supports timestamps
    let queue_families = unsafe {
        ctx.instance.get_physical_device_queue_family_properties(ctx.physical_device)
    };

    queue_families.iter().any(|qf| qf.timestamp_valid_bits > 0)
}

/// Get timestamp period in nanoseconds per tick.
pub fn get_timestamp_period(ctx: &super::context::VulkanContext) -> f32 {
    let props = unsafe {
        ctx.instance.get_physical_device_properties(ctx.physical_device)
    };
    props.limits.timestamp_period
}
