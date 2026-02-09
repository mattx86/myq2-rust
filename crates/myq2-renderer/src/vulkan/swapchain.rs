//! Vulkan swapchain management with triple-buffering.

use ash::vk;

use super::{VulkanContext, VulkanSurface};

/// Number of frames in flight for triple-buffering.
pub const MAX_FRAMES_IN_FLIGHT: usize = 3;

/// Per-frame synchronization primitives.
pub struct FrameSync {
    pub image_available: vk::Semaphore,
    pub render_finished: vk::Semaphore,
    pub in_flight: vk::Fence,
}

/// Vulkan swapchain with synchronization.
pub struct Swapchain {
    pub handle: vk::SwapchainKHR,
    pub images: Vec<vk::Image>,
    pub image_views: Vec<vk::ImageView>,
    pub format: vk::Format,
    pub extent: vk::Extent2D,
    pub frame_sync: Vec<FrameSync>,
    pub current_frame: usize,
    pub image_index: u32,
}

impl Swapchain {
    /// Create a new swapchain.
    pub unsafe fn new(
        ctx: &VulkanContext,
        surface: &VulkanSurface,
        width: u32,
        height: u32,
        old_swapchain: Option<vk::SwapchainKHR>,
    ) -> Result<Self, String> {
        let extent = surface.get_extent(width, height);

        // Determine image count (prefer triple-buffering)
        let min_images = surface.capabilities.min_image_count;
        let max_images = if surface.capabilities.max_image_count == 0 {
            u32::MAX
        } else {
            surface.capabilities.max_image_count
        };
        let image_count = (min_images + 1).min(max_images).max(MAX_FRAMES_IN_FLIGHT as u32);

        // Create swapchain
        let create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(surface.handle)
            .min_image_count(image_count)
            .image_format(surface.format.format)
            .image_color_space(surface.format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(surface.capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(surface.present_mode)
            .clipped(true)
            .old_swapchain(old_swapchain.unwrap_or(vk::SwapchainKHR::null()));

        let handle = ctx.swapchain_loader
            .create_swapchain(&create_info, None)
            .map_err(|e| format!("Failed to create swapchain: {:?}", e))?;

        // Destroy old swapchain if provided
        if let Some(old) = old_swapchain {
            ctx.swapchain_loader.destroy_swapchain(old, None);
        }

        // Get swapchain images
        let images = ctx.swapchain_loader
            .get_swapchain_images(handle)
            .map_err(|e| format!("Failed to get swapchain images: {:?}", e))?;

        // Create image views
        let image_views = Self::create_image_views(ctx, &images, surface.format.format)?;

        // Create synchronization objects
        let frame_sync = Self::create_sync_objects(ctx)?;

        Ok(Self {
            handle,
            images,
            image_views,
            format: surface.format.format,
            extent,
            frame_sync,
            current_frame: 0,
            image_index: 0,
        })
    }

    /// Create image views for swapchain images.
    unsafe fn create_image_views(
        ctx: &VulkanContext,
        images: &[vk::Image],
        format: vk::Format,
    ) -> Result<Vec<vk::ImageView>, String> {
        images.iter()
            .map(|&image| {
                let create_info = vk::ImageViewCreateInfo::default()
                    .image(image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(format)
                    .components(vk::ComponentMapping {
                        r: vk::ComponentSwizzle::IDENTITY,
                        g: vk::ComponentSwizzle::IDENTITY,
                        b: vk::ComponentSwizzle::IDENTITY,
                        a: vk::ComponentSwizzle::IDENTITY,
                    })
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    });

                ctx.device.create_image_view(&create_info, None)
                    .map_err(|e| format!("Failed to create image view: {:?}", e))
            })
            .collect()
    }

    /// Create synchronization objects for each frame in flight.
    unsafe fn create_sync_objects(ctx: &VulkanContext) -> Result<Vec<FrameSync>, String> {
        let semaphore_info = vk::SemaphoreCreateInfo::default();
        let fence_info = vk::FenceCreateInfo::default()
            .flags(vk::FenceCreateFlags::SIGNALED);

        (0..MAX_FRAMES_IN_FLIGHT)
            .map(|_| {
                let image_available = ctx.device.create_semaphore(&semaphore_info, None)
                    .map_err(|e| format!("Failed to create semaphore: {:?}", e))?;
                let render_finished = ctx.device.create_semaphore(&semaphore_info, None)
                    .map_err(|e| format!("Failed to create semaphore: {:?}", e))?;
                let in_flight = ctx.device.create_fence(&fence_info, None)
                    .map_err(|e| format!("Failed to create fence: {:?}", e))?;

                Ok(FrameSync {
                    image_available,
                    render_finished,
                    in_flight,
                })
            })
            .collect()
    }

    /// Acquire the next swapchain image.
    ///
    /// Returns `Ok(true)` if image acquired, `Ok(false)` if swapchain needs recreation.
    pub unsafe fn acquire_next_image(&mut self, ctx: &VulkanContext) -> Result<bool, String> {
        let sync = &self.frame_sync[self.current_frame];

        // Wait for previous frame to complete
        ctx.device.wait_for_fences(&[sync.in_flight], true, u64::MAX)
            .map_err(|e| format!("Failed to wait for fence: {:?}", e))?;

        // Acquire next image
        let result = ctx.swapchain_loader.acquire_next_image(
            self.handle,
            u64::MAX,
            sync.image_available,
            vk::Fence::null(),
        );

        match result {
            Ok((index, false)) => {
                self.image_index = index;
                Ok(true)
            }
            Ok((_, true)) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                // Swapchain needs recreation
                Ok(false)
            }
            Err(vk::Result::SUBOPTIMAL_KHR) => {
                // Suboptimal but still usable
                self.image_index = 0;
                Ok(true)
            }
            Err(e) => Err(format!("Failed to acquire swapchain image: {:?}", e)),
        }
    }

    /// Present the current frame.
    ///
    /// Returns `Ok(true)` if presented, `Ok(false)` if swapchain needs recreation.
    pub unsafe fn present(&mut self, ctx: &VulkanContext) -> Result<bool, String> {
        let sync = &self.frame_sync[self.current_frame];

        let swapchains = [self.handle];
        let image_indices = [self.image_index];
        let wait_semaphores = [sync.render_finished];

        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(&wait_semaphores)
            .swapchains(&swapchains)
            .image_indices(&image_indices);

        let result = ctx.swapchain_loader.queue_present(ctx.present_queue, &present_info);

        // Advance to next frame
        self.current_frame = (self.current_frame + 1) % MAX_FRAMES_IN_FLIGHT;

        match result {
            Ok(false) => Ok(true),
            Ok(true) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) | Err(vk::Result::SUBOPTIMAL_KHR) => {
                Ok(false)
            }
            Err(e) => Err(format!("Failed to present: {:?}", e)),
        }
    }

    /// Get the current frame's synchronization primitives.
    pub fn current_sync(&self) -> &FrameSync {
        &self.frame_sync[self.current_frame]
    }

    /// Reset the current frame's fence (call before submitting commands).
    pub unsafe fn reset_fence(&self, ctx: &VulkanContext) {
        let sync = &self.frame_sync[self.current_frame];
        let _ = ctx.device.reset_fences(&[sync.in_flight]);
    }

    /// Get the current swapchain image view.
    pub fn current_image_view(&self) -> vk::ImageView {
        self.image_views[self.image_index as usize]
    }

    /// Get the current swapchain image.
    pub fn current_image(&self) -> vk::Image {
        self.images[self.image_index as usize]
    }

    /// Recreate the swapchain (e.g., after window resize).
    pub unsafe fn recreate(
        &mut self,
        ctx: &VulkanContext,
        surface: &VulkanSurface,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        ctx.wait_idle();

        // Destroy old image views
        for view in &self.image_views {
            ctx.device.destroy_image_view(*view, None);
        }

        // Create new swapchain with old as base
        let old_swapchain = Some(self.handle);
        let new = Self::new(ctx, surface, width, height, old_swapchain)?;

        // Keep sync objects, update the rest
        self.handle = new.handle;
        self.images = new.images;
        self.image_views = new.image_views;
        self.extent = new.extent;

        // Don't drop new.frame_sync since we keep our own
        std::mem::forget(new.frame_sync);

        Ok(())
    }

    /// Destroy the swapchain and all associated resources.
    pub unsafe fn destroy(&mut self, ctx: &VulkanContext) {
        ctx.wait_idle();

        for sync in &self.frame_sync {
            ctx.device.destroy_semaphore(sync.image_available, None);
            ctx.device.destroy_semaphore(sync.render_finished, None);
            ctx.device.destroy_fence(sync.in_flight, None);
        }

        for view in &self.image_views {
            ctx.device.destroy_image_view(*view, None);
        }

        ctx.swapchain_loader.destroy_swapchain(self.handle, None);
    }
}
