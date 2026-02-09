//! Command buffer management and synchronization.
//!
//! Supports parallel command buffer recording via secondary command buffers.
//! Secondary buffers can be recorded concurrently from multiple threads,
//! then executed from the primary command buffer on the main thread.

use ash::vk;
use rayon::prelude::*;

use super::{VulkanContext, swapchain::MAX_FRAMES_IN_FLIGHT};

/// Command pool and buffers for a single frame.
pub struct FrameCommands {
    pub pool: vk::CommandPool,
    pub primary: vk::CommandBuffer,
    pub secondary: Vec<vk::CommandBuffer>,
}

/// Threshold for using parallel command buffer recording.
/// Below this count, sequential recording is more efficient.
const PARALLEL_RECORD_THRESHOLD: usize = 4;

/// A recorded secondary command buffer ready for execution.
pub struct RecordedSecondary {
    pub buffer: vk::CommandBuffer,
    pub index: usize,
}

// SAFETY: vk::CommandBuffer is just a handle (u64) and is Send when used correctly
unsafe impl Send for RecordedSecondary {}
unsafe impl Sync for RecordedSecondary {}

/// Work unit for parallel command buffer recording.
pub struct ParallelRecordWork<T> {
    /// Index for ordering
    pub index: usize,
    /// Data to use when recording
    pub data: T,
}

/// Command buffer manager with per-frame pools.
pub struct CommandManager {
    frames: Vec<FrameCommands>,
    transient_pool: vk::CommandPool,
    graphics_family: u32,
    device: ash::Device,
}

impl CommandManager {
    /// Create a new command manager.
    pub unsafe fn new(ctx: &VulkanContext) -> Result<Self, String> {
        let graphics_family = ctx.queue_families.graphics
            .ok_or("No graphics queue family")?;

        // Create per-frame command pools
        let mut frames = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);
        for i in 0..MAX_FRAMES_IN_FLIGHT {
            let pool_info = vk::CommandPoolCreateInfo::default()
                .queue_family_index(graphics_family)
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);

            let pool = ctx.device.create_command_pool(&pool_info, None)
                .map_err(|e| format!("Failed to create command pool: {:?}", e))?;

            let alloc_info = vk::CommandBufferAllocateInfo::default()
                .command_pool(pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1);

            let primary = ctx.device.allocate_command_buffers(&alloc_info)
                .map_err(|e| format!("Failed to allocate command buffer: {:?}", e))?[0];

            frames.push(FrameCommands {
                pool,
                primary,
                secondary: Vec::new(),
            });
        }

        // Create transient pool for one-shot commands
        let transient_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(graphics_family)
            .flags(vk::CommandPoolCreateFlags::TRANSIENT);

        let transient_pool = ctx.device.create_command_pool(&transient_info, None)
            .map_err(|e| format!("Failed to create transient pool: {:?}", e))?;

        Ok(Self {
            frames,
            transient_pool,
            graphics_family,
            device: ctx.device.clone(),
        })
    }

    /// Begin recording commands for a frame.
    pub unsafe fn begin_frame(&self, frame_index: usize) -> Result<vk::CommandBuffer, String> {
        let frame = &self.frames[frame_index];

        // Reset the command buffer
        self.device.reset_command_buffer(frame.primary, vk::CommandBufferResetFlags::empty())
            .map_err(|e| format!("Failed to reset command buffer: {:?}", e))?;

        // Begin recording
        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        self.device.begin_command_buffer(frame.primary, &begin_info)
            .map_err(|e| format!("Failed to begin command buffer: {:?}", e))?;

        Ok(frame.primary)
    }

    /// End recording commands for a frame.
    pub unsafe fn end_frame(&self, frame_index: usize) -> Result<(), String> {
        let frame = &self.frames[frame_index];
        self.device.end_command_buffer(frame.primary)
            .map_err(|e| format!("Failed to end command buffer: {:?}", e))
    }

    /// Submit the frame's commands to the graphics queue.
    pub unsafe fn submit_frame(
        &self,
        ctx: &VulkanContext,
        frame_index: usize,
        wait_semaphore: vk::Semaphore,
        signal_semaphore: vk::Semaphore,
        fence: vk::Fence,
    ) -> Result<(), String> {
        let frame = &self.frames[frame_index];

        let wait_semaphores = [wait_semaphore];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let signal_semaphores = [signal_semaphore];
        let command_buffers = [frame.primary];

        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(&wait_semaphores)
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(&command_buffers)
            .signal_semaphores(&signal_semaphores);

        ctx.device.queue_submit(ctx.graphics_queue, &[submit_info], fence)
            .map_err(|e| format!("Failed to submit command buffer: {:?}", e))
    }

    /// Get the primary command buffer for a frame.
    pub fn get_primary(&self, frame_index: usize) -> vk::CommandBuffer {
        self.frames[frame_index].primary
    }

    /// Begin a single-use command buffer.
    pub unsafe fn begin_single_time(&self) -> Result<vk::CommandBuffer, String> {
        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(self.transient_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);

        let cmd = self.device.allocate_command_buffers(&alloc_info)
            .map_err(|e| format!("Failed to allocate command buffer: {:?}", e))?[0];

        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        self.device.begin_command_buffer(cmd, &begin_info)
            .map_err(|e| format!("Failed to begin command buffer: {:?}", e))?;

        Ok(cmd)
    }

    /// End and submit a single-use command buffer, waiting for completion.
    pub unsafe fn end_single_time(&self, ctx: &VulkanContext, cmd: vk::CommandBuffer) -> Result<(), String> {
        self.device.end_command_buffer(cmd)
            .map_err(|e| format!("Failed to end command buffer: {:?}", e))?;

        let command_buffers = [cmd];
        let submit_info = vk::SubmitInfo::default()
            .command_buffers(&command_buffers);

        ctx.device.queue_submit(ctx.graphics_queue, &[submit_info], vk::Fence::null())
            .map_err(|e| format!("Failed to submit command buffer: {:?}", e))?;

        ctx.device.queue_wait_idle(ctx.graphics_queue)
            .map_err(|e| format!("Failed to wait for queue: {:?}", e))?;

        self.device.free_command_buffers(self.transient_pool, &command_buffers);

        Ok(())
    }

    /// Record an image layout transition.
    pub unsafe fn transition_image_layout(
        &self,
        cmd: vk::CommandBuffer,
        image: vk::Image,
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
        aspect_mask: vk::ImageAspectFlags,
    ) {
        let (src_access, dst_access, src_stage, dst_stage) = match (old_layout, new_layout) {
            (vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL) => (
                vk::AccessFlags::empty(),
                vk::AccessFlags::TRANSFER_WRITE,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::TRANSFER,
            ),
            (vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL) => (
                vk::AccessFlags::TRANSFER_WRITE,
                vk::AccessFlags::SHADER_READ,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
            ),
            (vk::ImageLayout::UNDEFINED, vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL) => (
                vk::AccessFlags::empty(),
                vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            ),
            (vk::ImageLayout::UNDEFINED, vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL) => (
                vk::AccessFlags::empty(),
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            ),
            (vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL) => (
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                vk::AccessFlags::SHADER_READ,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
            ),
            (vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL) => (
                vk::AccessFlags::SHADER_READ,
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            ),
            (vk::ImageLayout::UNDEFINED, vk::ImageLayout::GENERAL) => (
                vk::AccessFlags::empty(),
                vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::COMPUTE_SHADER,
            ),
            _ => (
                vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE,
                vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE,
                vk::PipelineStageFlags::ALL_COMMANDS,
                vk::PipelineStageFlags::ALL_COMMANDS,
            ),
        };

        let barrier = vk::ImageMemoryBarrier::default()
            .old_layout(old_layout)
            .new_layout(new_layout)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(image)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask,
                base_mip_level: 0,
                level_count: vk::REMAINING_MIP_LEVELS,
                base_array_layer: 0,
                layer_count: vk::REMAINING_ARRAY_LAYERS,
            })
            .src_access_mask(src_access)
            .dst_access_mask(dst_access);

        self.device.cmd_pipeline_barrier(
            cmd,
            src_stage,
            dst_stage,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[barrier],
        );
    }

    /// Copy buffer to image.
    pub unsafe fn copy_buffer_to_image(
        &self,
        cmd: vk::CommandBuffer,
        buffer: vk::Buffer,
        image: vk::Image,
        width: u32,
        height: u32,
    ) {
        let region = vk::BufferImageCopy::default()
            .buffer_offset(0)
            .buffer_row_length(0)
            .buffer_image_height(0)
            .image_subresource(vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            })
            .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
            .image_extent(vk::Extent3D { width, height, depth: 1 });

        self.device.cmd_copy_buffer_to_image(
            cmd,
            buffer,
            image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &[region],
        );
    }

    // ========================================================================
    // Parallel Command Buffer Recording
    // ========================================================================

    /// Allocate secondary command buffers for parallel recording.
    ///
    /// Returns the starting index of the allocated buffers.
    pub unsafe fn allocate_secondary_buffers(
        &mut self,
        frame_index: usize,
        count: usize,
    ) -> Result<usize, String> {
        if count == 0 {
            return Ok(0);
        }

        let frame = &mut self.frames[frame_index];
        let start_index = frame.secondary.len();

        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(frame.pool)
            .level(vk::CommandBufferLevel::SECONDARY)
            .command_buffer_count(count as u32);

        let buffers = self.device.allocate_command_buffers(&alloc_info)
            .map_err(|e| format!("Failed to allocate secondary buffers: {:?}", e))?;

        frame.secondary.extend(buffers);

        Ok(start_index)
    }

    /// Get a secondary command buffer by index.
    pub fn get_secondary(&self, frame_index: usize, buffer_index: usize) -> Option<vk::CommandBuffer> {
        self.frames.get(frame_index)
            .and_then(|f| f.secondary.get(buffer_index))
            .copied()
    }

    /// Record secondary command buffers in parallel.
    ///
    /// The `recorder` closure is called for each work item and must record
    /// commands to the provided command buffer. The closure receives:
    /// - The command buffer to record into
    /// - The work data of type T
    /// - The device handle for recording
    ///
    /// Returns a vector of recorded secondary buffers ready for execution.
    ///
    /// # Safety
    /// The recorder closure must only record valid Vulkan commands.
    pub unsafe fn record_secondary_parallel<T, F>(
        &mut self,
        frame_index: usize,
        work_items: Vec<ParallelRecordWork<T>>,
        inheritance_info: vk::CommandBufferInheritanceInfo,
        recorder: F,
    ) -> Result<Vec<RecordedSecondary>, String>
    where
        T: Send + Sync,
        F: Fn(vk::CommandBuffer, &T, &ash::Device) + Send + Sync,
    {
        let count = work_items.len();
        if count == 0 {
            return Ok(Vec::new());
        }

        // Allocate secondary buffers
        let start_index = self.allocate_secondary_buffers(frame_index, count)?;

        // Collect buffer handles for parallel access
        let frame = &self.frames[frame_index];
        let buffers: Vec<vk::CommandBuffer> = (start_index..start_index + count)
            .map(|i| frame.secondary[i])
            .collect();

        let device = &self.device;
        let inheritance = &inheritance_info;

        // Decide whether to use parallel recording
        if count >= PARALLEL_RECORD_THRESHOLD {
            // Parallel recording
            let results: Vec<Result<RecordedSecondary, String>> = work_items
                .into_par_iter()
                .enumerate()
                .map(|(i, work)| {
                    let cmd = buffers[i];

                    // Begin secondary command buffer
                    let begin_info = vk::CommandBufferBeginInfo::default()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT |
                               vk::CommandBufferUsageFlags::RENDER_PASS_CONTINUE)
                        .inheritance_info(inheritance);

                    device.begin_command_buffer(cmd, &begin_info)
                        .map_err(|e| format!("Failed to begin secondary buffer: {:?}", e))?;

                    // Record commands
                    recorder(cmd, &work.data, device);

                    // End command buffer
                    device.end_command_buffer(cmd)
                        .map_err(|e| format!("Failed to end secondary buffer: {:?}", e))?;

                    Ok(RecordedSecondary {
                        buffer: cmd,
                        index: work.index,
                    })
                })
                .collect();

            // Collect results, returning first error if any
            let mut recorded = Vec::with_capacity(count);
            for result in results {
                recorded.push(result?);
            }

            // Sort by original index to maintain ordering
            recorded.sort_by_key(|r| r.index);
            Ok(recorded)
        } else {
            // Sequential recording for small counts
            let mut recorded = Vec::with_capacity(count);

            for (i, work) in work_items.into_iter().enumerate() {
                let cmd = buffers[i];

                // Begin secondary command buffer
                let begin_info = vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT |
                           vk::CommandBufferUsageFlags::RENDER_PASS_CONTINUE)
                    .inheritance_info(inheritance);

                device.begin_command_buffer(cmd, &begin_info)
                    .map_err(|e| format!("Failed to begin secondary buffer: {:?}", e))?;

                // Record commands
                recorder(cmd, &work.data, device);

                // End command buffer
                device.end_command_buffer(cmd)
                    .map_err(|e| format!("Failed to end secondary buffer: {:?}", e))?;

                recorded.push(RecordedSecondary {
                    buffer: cmd,
                    index: work.index,
                });
            }

            Ok(recorded)
        }
    }

    /// Execute recorded secondary command buffers from the primary buffer.
    ///
    /// The secondary buffers should have been recorded with render pass continue flag.
    pub unsafe fn execute_secondary_buffers(
        &self,
        primary_cmd: vk::CommandBuffer,
        secondary_buffers: &[RecordedSecondary],
    ) {
        if secondary_buffers.is_empty() {
            return;
        }

        let buffers: Vec<vk::CommandBuffer> = secondary_buffers
            .iter()
            .map(|r| r.buffer)
            .collect();

        self.device.cmd_execute_commands(primary_cmd, &buffers);
    }

    /// Reset all secondary command buffers for a frame.
    ///
    /// Call this at the start of each frame to recycle secondary buffers.
    pub unsafe fn reset_secondary_buffers(&mut self, frame_index: usize) {
        let frame = &mut self.frames[frame_index];
        // Clear the secondary buffer list - they'll be reallocated as needed
        // The buffers themselves remain valid in the pool until the pool is reset
        frame.secondary.clear();
    }

    /// Destroy all command pools.
    pub unsafe fn destroy(&mut self, ctx: &VulkanContext) {
        for frame in &self.frames {
            ctx.device.destroy_command_pool(frame.pool, None);
        }
        ctx.device.destroy_command_pool(self.transient_pool, None);
    }
}
