//! Render target (Vulkan)
//!
//! Replaces SDL3 GPU render targets with Vulkan images.
//! Render targets are images with RENDER_TARGET usage flags.

use ash::vk;
use crate::modern::gpu_device;

/// A render target with color and optional depth attachments.
pub struct RenderTarget {
    /// Color image.
    color: Option<vk::Image>,
    /// Color image view.
    color_view: Option<vk::ImageView>,
    /// Color image memory.
    color_memory: Option<vk::DeviceMemory>,
    /// Depth image.
    depth: Option<vk::Image>,
    /// Depth image view.
    depth_view: Option<vk::ImageView>,
    /// Depth image memory.
    depth_memory: Option<vk::DeviceMemory>,
    /// Sampler for the color texture.
    sampler: Option<vk::Sampler>,
    /// Width in pixels.
    width: u32,
    /// Height in pixels.
    height: u32,
    /// Whether depth is attached.
    has_depth: bool,
    /// Whether depth is sampleable (for SSAO etc.).
    depth_sampleable: bool,
}

impl RenderTarget {
    /// Create a new render target.
    pub fn new(width: u32, height: u32, with_depth: bool) -> Self {
        Self::new_internal(width, height, with_depth, false)
    }

    /// Create a render target with sampleable depth texture (for SSAO).
    pub fn new_with_depth_texture(width: u32, height: u32) -> Self {
        Self::new_internal(width, height, true, true)
    }

    fn new_internal(width: u32, height: u32, with_depth: bool, depth_sampleable: bool) -> Self {
        let mut target = Self {
            color: None,
            color_view: None,
            color_memory: None,
            depth: None,
            depth_view: None,
            depth_memory: None,
            sampler: None,
            width,
            height,
            has_depth: with_depth,
            depth_sampleable,
        };
        target.create_resources();
        target
    }

    fn create_resources(&mut self) {
        // Destroy existing resources first
        self.destroy();

        if self.width == 0 || self.height == 0 {
            return;
        }

        gpu_device::with_device(|ctx| {
            // SAFETY: Vulkan context is valid and we're on the main thread.
            unsafe {
                // === Create color image ===
                let color_info = vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(vk::Format::R8G8B8A8_UNORM)
                    .extent(vk::Extent3D {
                        width: self.width,
                        height: self.height,
                        depth: 1,
                    })
                    .mip_levels(1)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .initial_layout(vk::ImageLayout::UNDEFINED);

                let color_image = match ctx.device.create_image(&color_info, None) {
                    Ok(img) => img,
                    Err(_) => return,
                };

                // Allocate memory for color image
                let color_mem_reqs = ctx.device.get_image_memory_requirements(color_image);
                let mem_props = ctx.instance.get_physical_device_memory_properties(ctx.physical_device);

                let color_mem_type = (0..mem_props.memory_type_count)
                    .find(|&i| {
                        (color_mem_reqs.memory_type_bits & (1 << i)) != 0 &&
                        mem_props.memory_types[i as usize].property_flags.contains(
                            vk::MemoryPropertyFlags::DEVICE_LOCAL
                        )
                    });

                let color_mem_type = match color_mem_type {
                    Some(i) => i,
                    None => {
                        ctx.device.destroy_image(color_image, None);
                        return;
                    }
                };

                let color_alloc_info = vk::MemoryAllocateInfo::default()
                    .allocation_size(color_mem_reqs.size)
                    .memory_type_index(color_mem_type);

                let color_memory = match ctx.device.allocate_memory(&color_alloc_info, None) {
                    Ok(mem) => mem,
                    Err(_) => {
                        ctx.device.destroy_image(color_image, None);
                        return;
                    }
                };

                if ctx.device.bind_image_memory(color_image, color_memory, 0).is_err() {
                    ctx.device.free_memory(color_memory, None);
                    ctx.device.destroy_image(color_image, None);
                    return;
                }

                // Create color image view
                let color_view_info = vk::ImageViewCreateInfo::default()
                    .image(color_image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(vk::Format::R8G8B8A8_UNORM)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    });

                let color_view = match ctx.device.create_image_view(&color_view_info, None) {
                    Ok(view) => view,
                    Err(_) => {
                        ctx.device.free_memory(color_memory, None);
                        ctx.device.destroy_image(color_image, None);
                        return;
                    }
                };

                self.color = Some(color_image);
                self.color_view = Some(color_view);
                self.color_memory = Some(color_memory);

                // === Create sampler for color texture ===
                let sampler_info = vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::LINEAR)
                    .min_filter(vk::Filter::LINEAR)
                    .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .mip_lod_bias(0.0)
                    .anisotropy_enable(false)
                    .max_anisotropy(1.0)
                    .compare_enable(false)
                    .min_lod(0.0)
                    .max_lod(0.0)
                    .border_color(vk::BorderColor::FLOAT_OPAQUE_BLACK)
                    .unnormalized_coordinates(false);

                if let Ok(sampler) = ctx.device.create_sampler(&sampler_info, None) {
                    self.sampler = Some(sampler);
                }

                // === Create depth image if needed ===
                if self.has_depth {
                    let depth_usage = if self.depth_sampleable {
                        vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED
                    } else {
                        vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT
                    };

                    let depth_info = vk::ImageCreateInfo::default()
                        .image_type(vk::ImageType::TYPE_2D)
                        .format(vk::Format::D32_SFLOAT)
                        .extent(vk::Extent3D {
                            width: self.width,
                            height: self.height,
                            depth: 1,
                        })
                        .mip_levels(1)
                        .array_layers(1)
                        .samples(vk::SampleCountFlags::TYPE_1)
                        .tiling(vk::ImageTiling::OPTIMAL)
                        .usage(depth_usage)
                        .sharing_mode(vk::SharingMode::EXCLUSIVE)
                        .initial_layout(vk::ImageLayout::UNDEFINED);

                    let depth_image = match ctx.device.create_image(&depth_info, None) {
                        Ok(img) => img,
                        Err(_) => return,
                    };

                    // Allocate memory for depth image
                    let depth_mem_reqs = ctx.device.get_image_memory_requirements(depth_image);

                    let depth_mem_type = (0..mem_props.memory_type_count)
                        .find(|&i| {
                            (depth_mem_reqs.memory_type_bits & (1 << i)) != 0 &&
                            mem_props.memory_types[i as usize].property_flags.contains(
                                vk::MemoryPropertyFlags::DEVICE_LOCAL
                            )
                        });

                    let depth_mem_type = match depth_mem_type {
                        Some(i) => i,
                        None => {
                            ctx.device.destroy_image(depth_image, None);
                            return;
                        }
                    };

                    let depth_alloc_info = vk::MemoryAllocateInfo::default()
                        .allocation_size(depth_mem_reqs.size)
                        .memory_type_index(depth_mem_type);

                    let depth_memory = match ctx.device.allocate_memory(&depth_alloc_info, None) {
                        Ok(mem) => mem,
                        Err(_) => {
                            ctx.device.destroy_image(depth_image, None);
                            return;
                        }
                    };

                    if ctx.device.bind_image_memory(depth_image, depth_memory, 0).is_err() {
                        ctx.device.free_memory(depth_memory, None);
                        ctx.device.destroy_image(depth_image, None);
                        return;
                    }

                    // Create depth image view
                    let depth_view_info = vk::ImageViewCreateInfo::default()
                        .image(depth_image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(vk::Format::D32_SFLOAT)
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::DEPTH,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        });

                    let depth_view = match ctx.device.create_image_view(&depth_view_info, None) {
                        Ok(view) => view,
                        Err(_) => {
                            ctx.device.free_memory(depth_memory, None);
                            ctx.device.destroy_image(depth_image, None);
                            return;
                        }
                    };

                    self.depth = Some(depth_image);
                    self.depth_view = Some(depth_view);
                    self.depth_memory = Some(depth_memory);
                }
            }
        });
    }

    /// Resize the render target.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == self.width && height == self.height {
            return;
        }
        self.width = width;
        self.height = height;
        self.create_resources();
    }

    /// Get width.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Get height.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Get color image.
    pub fn color_image(&self) -> Option<vk::Image> {
        self.color
    }

    /// Get color image view.
    pub fn color_view(&self) -> Option<vk::ImageView> {
        self.color_view
    }

    /// Get depth image view.
    pub fn depth_view(&self) -> Option<vk::ImageView> {
        self.depth_view
    }

    /// Get sampler.
    pub fn sampler(&self) -> Option<vk::Sampler> {
        self.sampler
    }

    /// Check if valid.
    pub fn is_valid(&self) -> bool {
        self.color.is_some()
    }

    /// Has depth attachment.
    pub fn has_depth(&self) -> bool {
        self.has_depth
    }

    /// Bind as render target (no-op in Vulkan - handled via render pass attachments).
    pub fn bind(&self) {
        // In Vulkan, render target binding happens through VkRenderingInfo attachments
        // This is a compatibility stub
    }

    /// Unbind render target (no-op in Vulkan).
    pub fn unbind() {
        // Compatibility stub
    }

    /// Bind the color texture to a texture unit (no-op in Vulkan - uses descriptor sets).
    pub fn bind_color_texture(&self, _unit: u32) {
        // In Vulkan, texture binding happens through descriptor sets
        // This is a compatibility stub
    }

    /// Bind the depth texture to a texture unit (no-op in Vulkan - uses descriptor sets).
    pub fn bind_depth_texture(&self, _unit: u32) {
        // Compatibility stub
    }

    /// Destroy resources.
    pub fn destroy(&mut self) {
        gpu_device::with_device(|ctx| {
            // SAFETY: Vulkan context is valid and we're on the main thread.
            unsafe {
                // Destroy sampler
                if let Some(sampler) = self.sampler.take() {
                    ctx.device.destroy_sampler(sampler, None);
                }

                // Destroy depth resources
                if let Some(view) = self.depth_view.take() {
                    ctx.device.destroy_image_view(view, None);
                }
                if let Some(image) = self.depth.take() {
                    ctx.device.destroy_image(image, None);
                }
                if let Some(memory) = self.depth_memory.take() {
                    ctx.device.free_memory(memory, None);
                }

                // Destroy color resources
                if let Some(view) = self.color_view.take() {
                    ctx.device.destroy_image_view(view, None);
                }
                if let Some(image) = self.color.take() {
                    ctx.device.destroy_image(image, None);
                }
                if let Some(memory) = self.color_memory.take() {
                    ctx.device.free_memory(memory, None);
                }
            }
        });
    }
}

impl Default for RenderTarget {
    fn default() -> Self {
        Self {
            color: None,
            color_view: None,
            color_memory: None,
            depth: None,
            depth_view: None,
            depth_memory: None,
            sampler: None,
            width: 0,
            height: 0,
            has_depth: false,
            depth_sampleable: false,
        }
    }
}

impl Drop for RenderTarget {
    fn drop(&mut self) {
        self.destroy();
    }
}
