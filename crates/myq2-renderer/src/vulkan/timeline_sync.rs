//! Timeline semaphores for advanced GPU synchronization
//!
//! Timeline semaphores (Vulkan 1.2 core) provide a more flexible synchronization
//! primitive than binary semaphores:
//! - 64-bit monotonically increasing counter
//! - CPU can wait on or signal specific values
//! - Multiple operations can wait on different values
//! - Better for multi-queue and async compute workloads

use ash::vk;
use std::sync::atomic::{AtomicU64, Ordering};

/// Timeline semaphore handle.
pub struct TimelineSemaphore {
    /// Vulkan semaphore handle.
    semaphore: vk::Semaphore,
    /// Current signaled value (CPU-side tracking).
    current_value: AtomicU64,
    /// Name for debugging.
    name: String,
}

impl TimelineSemaphore {
    /// Create a new timeline semaphore.
    pub fn new(ctx: &super::context::VulkanContext, name: &str, initial_value: u64) -> Result<Self, String> {
        let mut type_info = vk::SemaphoreTypeCreateInfo::default()
            .semaphore_type(vk::SemaphoreType::TIMELINE)
            .initial_value(initial_value);

        let create_info = vk::SemaphoreCreateInfo::default()
            .push_next(&mut type_info);

        let semaphore = unsafe {
            ctx.device.create_semaphore(&create_info, None)
                .map_err(|e| format!("Failed to create timeline semaphore: {:?}", e))?
        };

        Ok(Self {
            semaphore,
            current_value: AtomicU64::new(initial_value),
            name: name.to_string(),
        })
    }

    /// Get the Vulkan semaphore handle.
    pub fn handle(&self) -> vk::Semaphore {
        self.semaphore
    }

    /// Get the current signaled value (as tracked on CPU).
    pub fn current_value(&self) -> u64 {
        self.current_value.load(Ordering::SeqCst)
    }

    /// Query the current value from the GPU.
    pub fn query_value(&self, ctx: &super::context::VulkanContext) -> Result<u64, String> {
        let value = unsafe {
            ctx.device.get_semaphore_counter_value(self.semaphore)
                .map_err(|e| format!("Failed to query semaphore value: {:?}", e))?
        };
        self.current_value.store(value, Ordering::SeqCst);
        Ok(value)
    }

    /// Signal from CPU to a specific value.
    pub fn signal(&self, ctx: &super::context::VulkanContext, value: u64) -> Result<(), String> {
        let signal_info = vk::SemaphoreSignalInfo::default()
            .semaphore(self.semaphore)
            .value(value);

        unsafe {
            ctx.device.signal_semaphore(&signal_info)
                .map_err(|e| format!("Failed to signal semaphore: {:?}", e))?;
        }

        self.current_value.store(value, Ordering::SeqCst);
        Ok(())
    }

    /// Increment and return the next value.
    pub fn next_value(&self) -> u64 {
        self.current_value.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Wait on CPU for semaphore to reach a value.
    pub fn wait(&self, ctx: &super::context::VulkanContext, value: u64, timeout_ns: u64) -> Result<bool, String> {
        let semaphores = [self.semaphore];
        let values = [value];

        let wait_info = vk::SemaphoreWaitInfo::default()
            .semaphores(&semaphores)
            .values(&values);

        let result = unsafe {
            ctx.device.wait_semaphores(&wait_info, timeout_ns)
        };

        match result {
            Ok(_) => Ok(true),
            Err(vk::Result::TIMEOUT) => Ok(false),
            Err(e) => Err(format!("Failed to wait on semaphore: {:?}", e)),
        }
    }

    /// Destroy the semaphore.
    pub fn destroy(&self, ctx: &super::context::VulkanContext) {
        unsafe {
            ctx.device.destroy_semaphore(self.semaphore, None);
        }
    }
}

/// Frame synchronization using timeline semaphores.
pub struct TimelineFrameSync {
    /// Graphics timeline.
    graphics: TimelineSemaphore,
    /// Compute timeline (for async compute).
    compute: TimelineSemaphore,
    /// Transfer timeline (for async uploads).
    transfer: TimelineSemaphore,
    /// Current frame number.
    frame_number: u64,
    /// Number of frames in flight.
    frames_in_flight: u32,
}

impl TimelineFrameSync {
    /// Create frame synchronization primitives.
    pub fn new(ctx: &super::context::VulkanContext, frames_in_flight: u32) -> Result<Self, String> {
        let graphics = TimelineSemaphore::new(ctx, "graphics_timeline", 0)?;
        let compute = TimelineSemaphore::new(ctx, "compute_timeline", 0)?;
        let transfer = TimelineSemaphore::new(ctx, "transfer_timeline", 0)?;

        Ok(Self {
            graphics,
            compute,
            transfer,
            frame_number: 0,
            frames_in_flight,
        })
    }

    /// Begin a new frame, waiting for the oldest in-flight frame to complete.
    pub fn begin_frame(&mut self, ctx: &super::context::VulkanContext) -> Result<u64, String> {
        self.frame_number += 1;

        // Wait for frame N - frames_in_flight to complete
        if self.frame_number > self.frames_in_flight as u64 {
            let wait_frame = self.frame_number - self.frames_in_flight as u64;
            self.graphics.wait(ctx, wait_frame, u64::MAX)?;
        }

        Ok(self.frame_number)
    }

    /// Get submit info for graphics queue with timeline semaphore.
    pub fn graphics_submit_info(&self) -> (vk::Semaphore, u64, u64) {
        let wait_value = if self.frame_number > 1 {
            self.frame_number - 1
        } else {
            0
        };
        let signal_value = self.frame_number;

        (self.graphics.handle(), wait_value, signal_value)
    }

    /// Signal that compute work for this frame is done.
    pub fn signal_compute(&self, ctx: &super::context::VulkanContext) -> Result<(), String> {
        self.compute.signal(ctx, self.frame_number)
    }

    /// Signal that transfer work for this frame is done.
    pub fn signal_transfer(&self, ctx: &super::context::VulkanContext) -> Result<(), String> {
        self.transfer.signal(ctx, self.frame_number)
    }

    /// Wait for compute to complete for a specific frame.
    pub fn wait_compute(&self, ctx: &super::context::VulkanContext, frame: u64, timeout_ns: u64) -> Result<bool, String> {
        self.compute.wait(ctx, frame, timeout_ns)
    }

    /// Wait for transfer to complete for a specific frame.
    pub fn wait_transfer(&self, ctx: &super::context::VulkanContext, frame: u64, timeout_ns: u64) -> Result<bool, String> {
        self.transfer.wait(ctx, frame, timeout_ns)
    }

    /// Get current frame number.
    pub fn current_frame(&self) -> u64 {
        self.frame_number
    }

    /// Destroy all semaphores.
    pub fn destroy(&self, ctx: &super::context::VulkanContext) {
        self.graphics.destroy(ctx);
        self.compute.destroy(ctx);
        self.transfer.destroy(ctx);
    }
}

/// Helper for building timeline submit info.
pub struct TimelineSubmitBuilder {
    wait_semaphores: Vec<vk::Semaphore>,
    wait_values: Vec<u64>,
    wait_stages: Vec<vk::PipelineStageFlags>,
    signal_semaphores: Vec<vk::Semaphore>,
    signal_values: Vec<u64>,
    command_buffers: Vec<vk::CommandBuffer>,
}

impl TimelineSubmitBuilder {
    /// Create a new submit builder.
    pub fn new() -> Self {
        Self {
            wait_semaphores: Vec::new(),
            wait_values: Vec::new(),
            wait_stages: Vec::new(),
            signal_semaphores: Vec::new(),
            signal_values: Vec::new(),
            command_buffers: Vec::new(),
        }
    }

    /// Add a wait operation.
    pub fn wait(mut self, semaphore: &TimelineSemaphore, value: u64, stage: vk::PipelineStageFlags) -> Self {
        self.wait_semaphores.push(semaphore.handle());
        self.wait_values.push(value);
        self.wait_stages.push(stage);
        self
    }

    /// Add a signal operation.
    pub fn signal(mut self, semaphore: &TimelineSemaphore, value: u64) -> Self {
        self.signal_semaphores.push(semaphore.handle());
        self.signal_values.push(value);
        self
    }

    /// Add a command buffer.
    pub fn command_buffer(mut self, cmd: vk::CommandBuffer) -> Self {
        self.command_buffers.push(cmd);
        self
    }

    /// Build the submit info.
    /// Returns (SubmitInfo, TimelineSemaphoreSubmitInfo) - caller must keep TimelineSemaphoreSubmitInfo alive.
    pub fn build(&self) -> (vk::SubmitInfo<'_>, vk::TimelineSemaphoreSubmitInfo<'_>) {
        let timeline_info = vk::TimelineSemaphoreSubmitInfo::default()
            .wait_semaphore_values(&self.wait_values)
            .signal_semaphore_values(&self.signal_values);

        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(&self.wait_semaphores)
            .wait_dst_stage_mask(&self.wait_stages)
            .command_buffers(&self.command_buffers)
            .signal_semaphores(&self.signal_semaphores);

        (submit_info, timeline_info)
    }
}

impl Default for TimelineSubmitBuilder {
    fn default() -> Self {
        Self::new()
    }
}
