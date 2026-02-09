//! Push Descriptors for fast descriptor updates
//!
//! VK_KHR_push_descriptor allows updating descriptors directly in the command
//! buffer without allocating descriptor sets. Ideal for:
//! - Per-draw uniform data
//! - Frequently changing textures
//! - Small descriptor updates
//!
//! Benefits:
//! - No descriptor pool allocation
//! - No descriptor set management
//! - Lower CPU overhead for dynamic data

use ash::vk;
use ash::khr::push_descriptor;
use std::ffi::CStr;

/// Push descriptor extension name.
pub const PUSH_DESCRIPTOR_EXTENSION: &CStr = push_descriptor::NAME;

/// Push descriptor capabilities.
#[derive(Debug, Clone)]
pub struct PushDescriptorCapabilities {
    /// Whether push descriptors are supported.
    pub supported: bool,
    /// Maximum number of push descriptors.
    pub max_push_descriptors: u32,
}

impl Default for PushDescriptorCapabilities {
    fn default() -> Self {
        Self {
            supported: false,
            max_push_descriptors: 0,
        }
    }
}

/// Push descriptor manager.
pub struct PushDescriptorManager {
    /// Capabilities.
    capabilities: PushDescriptorCapabilities,
    /// Extension loader.
    loader: Option<push_descriptor::Device>,
}

impl PushDescriptorManager {
    /// Query push descriptor capabilities.
    pub fn query_capabilities(ctx: &super::context::VulkanContext) -> PushDescriptorCapabilities {
        // Check extension support
        let extensions = unsafe {
            ctx.instance.enumerate_device_extension_properties(ctx.physical_device)
                .unwrap_or_default()
        };

        let supported = extensions.iter().any(|ext| {
            let name = unsafe { CStr::from_ptr(ext.extension_name.as_ptr()) };
            name == PUSH_DESCRIPTOR_EXTENSION
        });

        if !supported {
            return PushDescriptorCapabilities::default();
        }

        // Query properties
        let mut push_desc_props = vk::PhysicalDevicePushDescriptorPropertiesKHR::default();
        let mut props2 = vk::PhysicalDeviceProperties2::default()
            .push_next(&mut push_desc_props);

        unsafe {
            ctx.instance.get_physical_device_properties2(ctx.physical_device, &mut props2);
        }

        PushDescriptorCapabilities {
            supported: true,
            max_push_descriptors: push_desc_props.max_push_descriptors,
        }
    }

    /// Create a new push descriptor manager.
    pub fn new(ctx: &super::context::VulkanContext) -> Self {
        let capabilities = Self::query_capabilities(ctx);

        let loader = if capabilities.supported {
            Some(push_descriptor::Device::new(&ctx.instance, &ctx.device))
        } else {
            None
        };

        Self {
            capabilities,
            loader,
        }
    }

    /// Check if push descriptors are supported.
    pub fn is_supported(&self) -> bool {
        self.capabilities.supported
    }

    /// Get capabilities.
    pub fn capabilities(&self) -> &PushDescriptorCapabilities {
        &self.capabilities
    }

    /// Push a uniform buffer descriptor.
    pub fn push_uniform_buffer(
        &self,
        cmd: vk::CommandBuffer,
        pipeline_layout: vk::PipelineLayout,
        set: u32,
        binding: u32,
        buffer: vk::Buffer,
        offset: vk::DeviceSize,
        range: vk::DeviceSize,
    ) {
        if let Some(ref loader) = self.loader {
            let buffer_info = vk::DescriptorBufferInfo {
                buffer,
                offset,
                range,
            };

            let write = vk::WriteDescriptorSet::default()
                .dst_binding(binding)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .buffer_info(std::slice::from_ref(&buffer_info));

            unsafe {
                loader.cmd_push_descriptor_set(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    pipeline_layout,
                    set,
                    std::slice::from_ref(&write),
                );
            }
        }
    }

    /// Push a storage buffer descriptor.
    pub fn push_storage_buffer(
        &self,
        cmd: vk::CommandBuffer,
        pipeline_layout: vk::PipelineLayout,
        set: u32,
        binding: u32,
        buffer: vk::Buffer,
        offset: vk::DeviceSize,
        range: vk::DeviceSize,
    ) {
        if let Some(ref loader) = self.loader {
            let buffer_info = vk::DescriptorBufferInfo {
                buffer,
                offset,
                range,
            };

            let write = vk::WriteDescriptorSet::default()
                .dst_binding(binding)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .buffer_info(std::slice::from_ref(&buffer_info));

            unsafe {
                loader.cmd_push_descriptor_set(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    pipeline_layout,
                    set,
                    std::slice::from_ref(&write),
                );
            }
        }
    }

    /// Push a combined image sampler descriptor.
    pub fn push_combined_image_sampler(
        &self,
        cmd: vk::CommandBuffer,
        pipeline_layout: vk::PipelineLayout,
        set: u32,
        binding: u32,
        sampler: vk::Sampler,
        image_view: vk::ImageView,
        image_layout: vk::ImageLayout,
    ) {
        if let Some(ref loader) = self.loader {
            let image_info = vk::DescriptorImageInfo {
                sampler,
                image_view,
                image_layout,
            };

            let write = vk::WriteDescriptorSet::default()
                .dst_binding(binding)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(std::slice::from_ref(&image_info));

            unsafe {
                loader.cmd_push_descriptor_set(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    pipeline_layout,
                    set,
                    std::slice::from_ref(&write),
                );
            }
        }
    }

    /// Push a sampled image descriptor.
    pub fn push_sampled_image(
        &self,
        cmd: vk::CommandBuffer,
        pipeline_layout: vk::PipelineLayout,
        set: u32,
        binding: u32,
        image_view: vk::ImageView,
        image_layout: vk::ImageLayout,
    ) {
        if let Some(ref loader) = self.loader {
            let image_info = vk::DescriptorImageInfo {
                sampler: vk::Sampler::null(),
                image_view,
                image_layout,
            };

            let write = vk::WriteDescriptorSet::default()
                .dst_binding(binding)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .image_info(std::slice::from_ref(&image_info));

            unsafe {
                loader.cmd_push_descriptor_set(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    pipeline_layout,
                    set,
                    std::slice::from_ref(&write),
                );
            }
        }
    }

    /// Push multiple descriptors at once.
    pub fn push_descriptors(
        &self,
        cmd: vk::CommandBuffer,
        pipeline_bind_point: vk::PipelineBindPoint,
        pipeline_layout: vk::PipelineLayout,
        set: u32,
        writes: &[vk::WriteDescriptorSet],
    ) {
        if let Some(ref loader) = self.loader {
            unsafe {
                loader.cmd_push_descriptor_set(
                    cmd,
                    pipeline_bind_point,
                    pipeline_layout,
                    set,
                    writes,
                );
            }
        }
    }
}

/// Helper for building push descriptor writes.
pub struct PushDescriptorBuilder {
    writes: Vec<vk::WriteDescriptorSet<'static>>,
    buffer_infos: Vec<vk::DescriptorBufferInfo>,
    image_infos: Vec<vk::DescriptorImageInfo>,
}

impl PushDescriptorBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            writes: Vec::new(),
            buffer_infos: Vec::new(),
            image_infos: Vec::new(),
        }
    }

    /// Add a uniform buffer.
    pub fn uniform_buffer(
        mut self,
        binding: u32,
        buffer: vk::Buffer,
        offset: vk::DeviceSize,
        range: vk::DeviceSize,
    ) -> Self {
        self.buffer_infos.push(vk::DescriptorBufferInfo {
            buffer,
            offset,
            range,
        });
        self.writes.push(
            vk::WriteDescriptorSet::default()
                .dst_binding(binding)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
        );
        self
    }

    /// Add a combined image sampler.
    pub fn combined_image_sampler(
        mut self,
        binding: u32,
        sampler: vk::Sampler,
        image_view: vk::ImageView,
        layout: vk::ImageLayout,
    ) -> Self {
        self.image_infos.push(vk::DescriptorImageInfo {
            sampler,
            image_view,
            image_layout: layout,
        });
        self.writes.push(
            vk::WriteDescriptorSet::default()
                .dst_binding(binding)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        );
        self
    }

    /// Get the number of descriptors.
    pub fn count(&self) -> usize {
        self.writes.len()
    }
}

impl Default for PushDescriptorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a descriptor set layout with push descriptor flag.
pub fn create_push_descriptor_layout(
    ctx: &super::context::VulkanContext,
    bindings: &[vk::DescriptorSetLayoutBinding],
) -> Result<vk::DescriptorSetLayout, String> {
    let create_info = vk::DescriptorSetLayoutCreateInfo::default()
        .bindings(bindings)
        .flags(vk::DescriptorSetLayoutCreateFlags::PUSH_DESCRIPTOR_KHR);

    unsafe {
        ctx.device.create_descriptor_set_layout(&create_info, None)
            .map_err(|e| format!("Failed to create push descriptor layout: {:?}", e))
    }
}
