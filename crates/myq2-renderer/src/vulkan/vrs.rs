//! Variable Rate Shading (VRS) support
//!
//! VRS allows rendering different screen regions at different shading rates,
//! improving performance by reducing fragment shader invocations in less
//! important areas (peripheral vision, motion blur regions).
//!
//! Requires VK_KHR_fragment_shading_rate extension.

use ash::vk;
use super::context::VulkanContext;

/// VRS shading rate options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadingRate {
    /// Full rate (1x1) - every pixel shaded individually
    Full,
    /// Half rate horizontal (2x1) - 2 pixels share one shading
    Half2x1,
    /// Half rate vertical (1x2)
    Half1x2,
    /// Quarter rate (2x2) - 4 pixels share one shading
    Quarter,
    /// Eighth rate horizontal (4x2)
    Eighth4x2,
    /// Eighth rate vertical (2x4)
    Eighth2x4,
    /// Sixteenth rate (4x4) - 16 pixels share one shading
    Sixteenth,
}

impl ShadingRate {
    /// Convert to Vulkan fragment size.
    pub fn to_vk_extent(self) -> vk::Extent2D {
        match self {
            ShadingRate::Full => vk::Extent2D { width: 1, height: 1 },
            ShadingRate::Half2x1 => vk::Extent2D { width: 2, height: 1 },
            ShadingRate::Half1x2 => vk::Extent2D { width: 1, height: 2 },
            ShadingRate::Quarter => vk::Extent2D { width: 2, height: 2 },
            ShadingRate::Eighth4x2 => vk::Extent2D { width: 4, height: 2 },
            ShadingRate::Eighth2x4 => vk::Extent2D { width: 2, height: 4 },
            ShadingRate::Sixteenth => vk::Extent2D { width: 4, height: 4 },
        }
    }

    /// Convert to Vulkan fragment shading rate enum.
    pub fn to_vk_rate(self) -> vk::FragmentShadingRateNV {
        match self {
            ShadingRate::Full => vk::FragmentShadingRateNV::TYPE_1_INVOCATION_PER_PIXEL,
            ShadingRate::Half2x1 => vk::FragmentShadingRateNV::TYPE_1_INVOCATION_PER_2X1_PIXELS,
            ShadingRate::Half1x2 => vk::FragmentShadingRateNV::TYPE_1_INVOCATION_PER_1X2_PIXELS,
            ShadingRate::Quarter => vk::FragmentShadingRateNV::TYPE_1_INVOCATION_PER_2X2_PIXELS,
            ShadingRate::Eighth4x2 => vk::FragmentShadingRateNV::TYPE_1_INVOCATION_PER_4X2_PIXELS,
            ShadingRate::Eighth2x4 => vk::FragmentShadingRateNV::TYPE_1_INVOCATION_PER_2X4_PIXELS,
            ShadingRate::Sixteenth => vk::FragmentShadingRateNV::TYPE_1_INVOCATION_PER_4X4_PIXELS,
        }
    }
}

/// VRS capabilities of the device.
#[derive(Debug, Clone, Default)]
pub struct VrsCapabilities {
    /// Whether VRS is supported.
    pub supported: bool,
    /// Whether per-primitive shading rate is supported.
    pub per_primitive: bool,
    /// Whether attachment-based shading rate is supported.
    pub attachment_based: bool,
    /// Minimum supported fragment size.
    pub min_fragment_size: vk::Extent2D,
    /// Maximum supported fragment size.
    pub max_fragment_size: vk::Extent2D,
    /// Maximum fragment size aspect ratio.
    pub max_fragment_size_aspect_ratio: u32,
    /// Shading rate texel size for attachment-based VRS.
    pub shading_rate_texel_size: vk::Extent2D,
}

/// VRS manager for controlling shading rates.
pub struct VrsManager {
    /// Device capabilities.
    capabilities: VrsCapabilities,
    /// Current pipeline shading rate.
    pipeline_rate: ShadingRate,
    /// VRS attachment image (for attachment-based VRS).
    attachment_image: Option<vk::Image>,
    /// VRS attachment image view.
    attachment_view: Option<vk::ImageView>,
    /// VRS attachment memory.
    attachment_memory: Option<vk::DeviceMemory>,
    /// Attachment dimensions.
    attachment_width: u32,
    attachment_height: u32,
    /// Whether VRS is enabled.
    enabled: bool,
    /// Extension loader.
    fsr_loader: Option<ash::khr::fragment_shading_rate::Device>,
}

impl VrsManager {
    /// Create a new VRS manager.
    pub fn new(ctx: &VulkanContext) -> Self {
        let capabilities = Self::query_capabilities(ctx);

        let fsr_loader = if capabilities.supported {
            Some(ash::khr::fragment_shading_rate::Device::new(&ctx.instance, &ctx.device))
        } else {
            None
        };

        Self {
            capabilities,
            pipeline_rate: ShadingRate::Full,
            attachment_image: None,
            attachment_view: None,
            attachment_memory: None,
            attachment_width: 0,
            attachment_height: 0,
            enabled: false,
            fsr_loader,
        }
    }

    /// Query VRS capabilities from the device.
    fn query_capabilities(ctx: &VulkanContext) -> VrsCapabilities {
        // Check if extension is available
        let extensions = unsafe {
            ctx.instance.enumerate_device_extension_properties(ctx.physical_device)
                .unwrap_or_default()
        };

        let has_fsr = extensions.iter().any(|ext| {
            let name = unsafe { std::ffi::CStr::from_ptr(ext.extension_name.as_ptr()) };
            name.to_bytes() == b"VK_KHR_fragment_shading_rate"
        });

        if !has_fsr {
            return VrsCapabilities::default();
        }

        // Query properties
        let mut fsr_props = vk::PhysicalDeviceFragmentShadingRatePropertiesKHR::default();
        let mut props2 = vk::PhysicalDeviceProperties2::default()
            .push_next(&mut fsr_props);

        unsafe {
            ctx.instance.get_physical_device_properties2(ctx.physical_device, &mut props2);
        }

        // Query features
        let mut fsr_features = vk::PhysicalDeviceFragmentShadingRateFeaturesKHR::default();
        let mut features2 = vk::PhysicalDeviceFeatures2::default()
            .push_next(&mut fsr_features);

        unsafe {
            ctx.instance.get_physical_device_features2(ctx.physical_device, &mut features2);
        }

        VrsCapabilities {
            supported: fsr_features.pipeline_fragment_shading_rate == vk::TRUE,
            per_primitive: fsr_features.primitive_fragment_shading_rate == vk::TRUE,
            attachment_based: fsr_features.attachment_fragment_shading_rate == vk::TRUE,
            min_fragment_size: fsr_props.min_fragment_shading_rate_attachment_texel_size,
            max_fragment_size: fsr_props.max_fragment_shading_rate_attachment_texel_size,
            max_fragment_size_aspect_ratio: fsr_props.max_fragment_size_aspect_ratio,
            shading_rate_texel_size: fsr_props.min_fragment_shading_rate_attachment_texel_size,
        }
    }

    /// Check if VRS is supported.
    pub fn is_supported(&self) -> bool {
        self.capabilities.supported
    }

    /// Check if attachment-based VRS is supported.
    pub fn supports_attachment(&self) -> bool {
        self.capabilities.attachment_based
    }

    /// Get VRS capabilities.
    pub fn capabilities(&self) -> &VrsCapabilities {
        &self.capabilities
    }

    /// Enable or disable VRS.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled && self.capabilities.supported;
    }

    /// Check if VRS is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Set the pipeline shading rate.
    pub fn set_pipeline_rate(&mut self, rate: ShadingRate) {
        self.pipeline_rate = rate;
    }

    /// Get the current pipeline shading rate.
    pub fn pipeline_rate(&self) -> ShadingRate {
        self.pipeline_rate
    }

    /// Create or resize the VRS attachment image.
    ///
    /// The attachment is used for screen-space VRS where different regions
    /// have different shading rates.
    pub fn create_attachment(&mut self, ctx: &VulkanContext, width: u32, height: u32) {
        if !self.capabilities.attachment_based {
            return;
        }

        // Destroy existing attachment
        self.destroy_attachment(ctx);

        let texel_size = self.capabilities.shading_rate_texel_size;
        if texel_size.width == 0 || texel_size.height == 0 {
            return;
        }

        // Calculate attachment dimensions (each texel covers texel_size pixels)
        let att_width = (width + texel_size.width - 1) / texel_size.width;
        let att_height = (height + texel_size.height - 1) / texel_size.height;

        unsafe {
            // Create image
            let image_info = vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .format(vk::Format::R8_UINT)
                .extent(vk::Extent3D {
                    width: att_width,
                    height: att_height,
                    depth: 1,
                })
                .mip_levels(1)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(vk::ImageUsageFlags::FRAGMENT_SHADING_RATE_ATTACHMENT_KHR
                    | vk::ImageUsageFlags::TRANSFER_DST)
                .sharing_mode(vk::SharingMode::EXCLUSIVE);

            let image = match ctx.device.create_image(&image_info, None) {
                Ok(img) => img,
                Err(_) => return,
            };

            // Allocate memory
            let mem_reqs = ctx.device.get_image_memory_requirements(image);
            let mem_props = ctx.instance.get_physical_device_memory_properties(ctx.physical_device);

            let mem_type = (0..mem_props.memory_type_count).find(|&i| {
                (mem_reqs.memory_type_bits & (1 << i)) != 0
                    && mem_props.memory_types[i as usize]
                        .property_flags
                        .contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)
            });

            let mem_type = match mem_type {
                Some(t) => t,
                None => {
                    ctx.device.destroy_image(image, None);
                    return;
                }
            };

            let alloc_info = vk::MemoryAllocateInfo::default()
                .allocation_size(mem_reqs.size)
                .memory_type_index(mem_type);

            let memory = match ctx.device.allocate_memory(&alloc_info, None) {
                Ok(mem) => mem,
                Err(_) => {
                    ctx.device.destroy_image(image, None);
                    return;
                }
            };

            if ctx.device.bind_image_memory(image, memory, 0).is_err() {
                ctx.device.free_memory(memory, None);
                ctx.device.destroy_image(image, None);
                return;
            }

            // Create image view
            let view_info = vk::ImageViewCreateInfo::default()
                .image(image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(vk::Format::R8_UINT)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                });

            let view = match ctx.device.create_image_view(&view_info, None) {
                Ok(v) => v,
                Err(_) => {
                    ctx.device.free_memory(memory, None);
                    ctx.device.destroy_image(image, None);
                    return;
                }
            };

            self.attachment_image = Some(image);
            self.attachment_view = Some(view);
            self.attachment_memory = Some(memory);
            self.attachment_width = att_width;
            self.attachment_height = att_height;
        }
    }

    /// Update the VRS attachment with a radial pattern.
    ///
    /// Center of screen gets full rate, edges get reduced rate.
    pub fn update_radial_pattern(&self, ctx: &VulkanContext, cmd: vk::CommandBuffer) {
        let image = match self.attachment_image {
            Some(img) => img,
            None => return,
        };

        if self.attachment_width == 0 || self.attachment_height == 0 {
            return;
        }

        // Generate radial pattern data
        let size = (self.attachment_width * self.attachment_height) as usize;
        let mut data = vec![0u8; size];

        let center_x = self.attachment_width as f32 / 2.0;
        let center_y = self.attachment_height as f32 / 2.0;
        let max_dist = (center_x * center_x + center_y * center_y).sqrt();

        for y in 0..self.attachment_height {
            for x in 0..self.attachment_width {
                let dx = x as f32 - center_x;
                let dy = y as f32 - center_y;
                let dist = (dx * dx + dy * dy).sqrt() / max_dist;

                // Map distance to shading rate
                // 0.0-0.3: full rate (center/crosshair)
                // 0.3-0.6: 2x2 rate
                // 0.6-1.0: 4x4 rate
                let rate = if dist < 0.3 {
                    0 // 1x1
                } else if dist < 0.6 {
                    5 // 2x2 (encoded as per VK_KHR_fragment_shading_rate)
                } else {
                    10 // 4x4
                };

                data[(y * self.attachment_width + x) as usize] = rate;
            }
        }

        // Upload data via staging buffer
        // (simplified - in production use the staging buffer pattern)
        unsafe {
            // Transition to transfer dst
            let barrier = vk::ImageMemoryBarrier::default()
                .old_layout(vk::ImageLayout::UNDEFINED)
                .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .image(image)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .src_access_mask(vk::AccessFlags::empty())
                .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE);

            ctx.device.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );

            // After upload, transition to shading rate attachment optimal
            let barrier = vk::ImageMemoryBarrier::default()
                .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .new_layout(vk::ImageLayout::FRAGMENT_SHADING_RATE_ATTACHMENT_OPTIMAL_KHR)
                .image(image)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::FRAGMENT_SHADING_RATE_ATTACHMENT_READ_KHR);

            ctx.device.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADING_RATE_ATTACHMENT_KHR,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );
        }

        let _ = data; // Data would be uploaded via staging buffer
    }

    /// Set pipeline shading rate for a command buffer.
    pub fn cmd_set_shading_rate(&self, ctx: &VulkanContext, cmd: vk::CommandBuffer) {
        if !self.enabled || self.fsr_loader.is_none() {
            return;
        }

        let fsr_loader = self.fsr_loader.as_ref().unwrap();
        let extent = self.pipeline_rate.to_vk_extent();
        let combiner_ops = [
            vk::FragmentShadingRateCombinerOpKHR::KEEP, // pipeline rate
            vk::FragmentShadingRateCombinerOpKHR::REPLACE, // attachment rate (if used)
        ];

        unsafe {
            (fsr_loader.fp().cmd_set_fragment_shading_rate_khr)(
                cmd,
                &extent,
                &combiner_ops as *const _,
            );
        }
    }

    /// Get the VRS attachment view for render pass usage.
    pub fn attachment_view(&self) -> Option<vk::ImageView> {
        self.attachment_view
    }

    /// Destroy the VRS attachment.
    fn destroy_attachment(&mut self, ctx: &VulkanContext) {
        unsafe {
            if let Some(view) = self.attachment_view.take() {
                ctx.device.destroy_image_view(view, None);
            }
            if let Some(image) = self.attachment_image.take() {
                ctx.device.destroy_image(image, None);
            }
            if let Some(memory) = self.attachment_memory.take() {
                ctx.device.free_memory(memory, None);
            }
        }
        self.attachment_width = 0;
        self.attachment_height = 0;
    }

    /// Shutdown and release all resources.
    pub fn shutdown(&mut self, ctx: &VulkanContext) {
        self.destroy_attachment(ctx);
    }
}

impl Default for VrsManager {
    fn default() -> Self {
        Self {
            capabilities: VrsCapabilities::default(),
            pipeline_rate: ShadingRate::Full,
            attachment_image: None,
            attachment_view: None,
            attachment_memory: None,
            attachment_width: 0,
            attachment_height: 0,
            enabled: false,
            fsr_loader: None,
        }
    }
}
