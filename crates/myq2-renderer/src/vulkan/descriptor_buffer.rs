//! Descriptor Buffer for ultra-fast descriptor access
//!
//! VK_EXT_descriptor_buffer stores descriptors directly in GPU buffers,
//! providing the fastest possible descriptor access:
//! - No descriptor set allocation/management
//! - Direct GPU memory access for descriptors
//! - Optimal for bindless rendering
//! - Lower CPU overhead than push descriptors

use ash::vk;
use std::collections::HashMap;

/// Descriptor buffer capabilities.
#[derive(Debug, Clone, Default)]
pub struct DescriptorBufferCapabilities {
    /// Whether descriptor buffer is supported.
    pub supported: bool,
    /// Whether combined image sampler descriptors are supported.
    pub combined_image_sampler_supported: bool,
    /// Whether sampler descriptors can be in buffers.
    pub sampler_descriptor_buffer_supported: bool,
    /// Whether resource descriptors can be in buffers.
    pub resource_descriptor_buffer_supported: bool,
    /// Descriptor buffer offset alignment.
    pub descriptor_buffer_offset_alignment: u64,
    /// Maximum size of a descriptor buffer binding.
    pub max_descriptor_buffer_bindings: u32,
    /// Size of a sampler descriptor.
    pub sampler_descriptor_size: usize,
    /// Size of a combined image sampler descriptor.
    pub combined_image_sampler_descriptor_size: usize,
    /// Size of a sampled image descriptor.
    pub sampled_image_descriptor_size: usize,
    /// Size of a storage image descriptor.
    pub storage_image_descriptor_size: usize,
    /// Size of a uniform texel buffer descriptor.
    pub uniform_texel_buffer_descriptor_size: usize,
    /// Size of a storage texel buffer descriptor.
    pub storage_texel_buffer_descriptor_size: usize,
    /// Size of a uniform buffer descriptor.
    pub uniform_buffer_descriptor_size: usize,
    /// Size of a storage buffer descriptor.
    pub storage_buffer_descriptor_size: usize,
    /// Size of an acceleration structure descriptor.
    pub acceleration_structure_descriptor_size: usize,
}

/// Query descriptor buffer capabilities.
pub fn query_capabilities(ctx: &super::context::VulkanContext) -> DescriptorBufferCapabilities {
    let mut desc_buffer_props = vk::PhysicalDeviceDescriptorBufferPropertiesEXT::default();
    let mut props2 = vk::PhysicalDeviceProperties2::default()
        .push_next(&mut desc_buffer_props);

    unsafe {
        ctx.instance.get_physical_device_properties2(ctx.physical_device, &mut props2);
    }

    let mut desc_buffer_features = vk::PhysicalDeviceDescriptorBufferFeaturesEXT::default();
    let mut features2 = vk::PhysicalDeviceFeatures2::default()
        .push_next(&mut desc_buffer_features);

    unsafe {
        ctx.instance.get_physical_device_features2(ctx.physical_device, &mut features2);
    }

    DescriptorBufferCapabilities {
        supported: desc_buffer_features.descriptor_buffer == vk::TRUE,
        combined_image_sampler_supported: desc_buffer_features.descriptor_buffer_image_layout_ignored == vk::TRUE,
        sampler_descriptor_buffer_supported: desc_buffer_features.descriptor_buffer == vk::TRUE,
        resource_descriptor_buffer_supported: desc_buffer_features.descriptor_buffer == vk::TRUE,
        descriptor_buffer_offset_alignment: desc_buffer_props.descriptor_buffer_offset_alignment,
        max_descriptor_buffer_bindings: desc_buffer_props.max_descriptor_buffer_bindings,
        sampler_descriptor_size: desc_buffer_props.sampler_descriptor_size,
        combined_image_sampler_descriptor_size: desc_buffer_props.combined_image_sampler_descriptor_size,
        sampled_image_descriptor_size: desc_buffer_props.sampled_image_descriptor_size,
        storage_image_descriptor_size: desc_buffer_props.storage_image_descriptor_size,
        uniform_texel_buffer_descriptor_size: desc_buffer_props.uniform_texel_buffer_descriptor_size,
        storage_texel_buffer_descriptor_size: desc_buffer_props.storage_texel_buffer_descriptor_size,
        uniform_buffer_descriptor_size: desc_buffer_props.uniform_buffer_descriptor_size,
        storage_buffer_descriptor_size: desc_buffer_props.storage_buffer_descriptor_size,
        acceleration_structure_descriptor_size: desc_buffer_props.acceleration_structure_descriptor_size,
    }
}

/// Descriptor type for buffer layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DescriptorType {
    Sampler,
    CombinedImageSampler,
    SampledImage,
    StorageImage,
    UniformTexelBuffer,
    StorageTexelBuffer,
    UniformBuffer,
    StorageBuffer,
    AccelerationStructure,
}

impl DescriptorType {
    /// Get the size of this descriptor type.
    pub fn size(&self, caps: &DescriptorBufferCapabilities) -> usize {
        match self {
            DescriptorType::Sampler => caps.sampler_descriptor_size,
            DescriptorType::CombinedImageSampler => caps.combined_image_sampler_descriptor_size,
            DescriptorType::SampledImage => caps.sampled_image_descriptor_size,
            DescriptorType::StorageImage => caps.storage_image_descriptor_size,
            DescriptorType::UniformTexelBuffer => caps.uniform_texel_buffer_descriptor_size,
            DescriptorType::StorageTexelBuffer => caps.storage_texel_buffer_descriptor_size,
            DescriptorType::UniformBuffer => caps.uniform_buffer_descriptor_size,
            DescriptorType::StorageBuffer => caps.storage_buffer_descriptor_size,
            DescriptorType::AccelerationStructure => caps.acceleration_structure_descriptor_size,
        }
    }

    /// Convert to Vulkan descriptor type.
    pub fn to_vk(&self) -> vk::DescriptorType {
        match self {
            DescriptorType::Sampler => vk::DescriptorType::SAMPLER,
            DescriptorType::CombinedImageSampler => vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            DescriptorType::SampledImage => vk::DescriptorType::SAMPLED_IMAGE,
            DescriptorType::StorageImage => vk::DescriptorType::STORAGE_IMAGE,
            DescriptorType::UniformTexelBuffer => vk::DescriptorType::UNIFORM_TEXEL_BUFFER,
            DescriptorType::StorageTexelBuffer => vk::DescriptorType::STORAGE_TEXEL_BUFFER,
            DescriptorType::UniformBuffer => vk::DescriptorType::UNIFORM_BUFFER,
            DescriptorType::StorageBuffer => vk::DescriptorType::STORAGE_BUFFER,
            DescriptorType::AccelerationStructure => vk::DescriptorType::ACCELERATION_STRUCTURE_KHR,
        }
    }
}

/// Descriptor buffer binding entry.
#[derive(Debug, Clone)]
pub struct DescriptorBinding {
    /// Binding index.
    pub binding: u32,
    /// Descriptor type.
    pub descriptor_type: DescriptorType,
    /// Number of descriptors.
    pub count: u32,
    /// Shader stages.
    pub stages: vk::ShaderStageFlags,
}

/// Descriptor buffer layout.
pub struct DescriptorBufferLayout {
    /// Vulkan descriptor set layout.
    layout: vk::DescriptorSetLayout,
    /// Size of the layout in bytes.
    size: u64,
    /// Binding offsets within the layout.
    binding_offsets: HashMap<u32, u64>,
    /// Bindings.
    bindings: Vec<DescriptorBinding>,
}

impl DescriptorBufferLayout {
    /// Create a new descriptor buffer layout.
    pub fn new(
        ctx: &super::context::VulkanContext,
        bindings: &[DescriptorBinding],
    ) -> Result<Self, String> {
        // Create layout bindings
        let vk_bindings: Vec<vk::DescriptorSetLayoutBinding> = bindings
            .iter()
            .map(|b| {
                vk::DescriptorSetLayoutBinding::default()
                    .binding(b.binding)
                    .descriptor_type(b.descriptor_type.to_vk())
                    .descriptor_count(b.count)
                    .stage_flags(b.stages)
            })
            .collect();

        // Create with descriptor buffer flag
        let flags = vk::DescriptorSetLayoutCreateFlags::DESCRIPTOR_BUFFER_EXT;

        let create_info = vk::DescriptorSetLayoutCreateInfo::default()
            .flags(flags)
            .bindings(&vk_bindings);

        let layout = unsafe {
            ctx.device.create_descriptor_set_layout(&create_info, None)
                .map_err(|e| format!("Failed to create descriptor buffer layout: {:?}", e))?
        };

        // Query layout size
        let size = Self::query_layout_size(ctx, layout);

        // Query binding offsets
        let mut binding_offsets = HashMap::new();
        for b in bindings {
            let offset = Self::query_binding_offset(ctx, layout, b.binding);
            binding_offsets.insert(b.binding, offset);
        }

        Ok(Self {
            layout,
            size,
            binding_offsets,
            bindings: bindings.to_vec(),
        })
    }

    /// Query layout size using function pointer.
    fn query_layout_size(ctx: &super::context::VulkanContext, layout: vk::DescriptorSetLayout) -> u64 {
        // Get function pointer for vkGetDescriptorSetLayoutSizeEXT
        let fp = unsafe {
            let name = std::ffi::CStr::from_bytes_with_nul_unchecked(b"vkGetDescriptorSetLayoutSizeEXT\0");
            ctx.instance.get_device_proc_addr(ctx.device.handle(), name.as_ptr())
        };

        if let Some(fp) = fp {
            let mut size: u64 = 0;
            unsafe {
                let func: vk::PFN_vkGetDescriptorSetLayoutSizeEXT = std::mem::transmute(fp);
                func(ctx.device.handle(), layout, &mut size);
            }
            size
        } else {
            0
        }
    }

    /// Query binding offset.
    fn query_binding_offset(ctx: &super::context::VulkanContext, layout: vk::DescriptorSetLayout, binding: u32) -> u64 {
        let fp = unsafe {
            let name = std::ffi::CStr::from_bytes_with_nul_unchecked(b"vkGetDescriptorSetLayoutBindingOffsetEXT\0");
            ctx.instance.get_device_proc_addr(ctx.device.handle(), name.as_ptr())
        };

        if let Some(fp) = fp {
            let mut offset: u64 = 0;
            unsafe {
                let func: vk::PFN_vkGetDescriptorSetLayoutBindingOffsetEXT = std::mem::transmute(fp);
                func(ctx.device.handle(), layout, binding, &mut offset);
            }
            offset
        } else {
            0
        }
    }

    /// Get the Vulkan layout handle.
    pub fn handle(&self) -> vk::DescriptorSetLayout {
        self.layout
    }

    /// Get the layout size in bytes.
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Get the offset of a binding.
    pub fn binding_offset(&self, binding: u32) -> Option<u64> {
        self.binding_offsets.get(&binding).copied()
    }

    /// Destroy the layout.
    pub fn destroy(&self, ctx: &super::context::VulkanContext) {
        unsafe {
            ctx.device.destroy_descriptor_set_layout(self.layout, None);
        }
    }
}

/// GPU buffer for storing descriptors.
pub struct DescriptorBuffer {
    /// Buffer handle.
    buffer: vk::Buffer,
    /// Device memory.
    memory: vk::DeviceMemory,
    /// Buffer device address.
    address: vk::DeviceAddress,
    /// Buffer size.
    size: u64,
    /// Mapped memory pointer.
    mapped: *mut u8,
    /// Capabilities reference for descriptor sizes.
    caps: DescriptorBufferCapabilities,
}

impl DescriptorBuffer {
    /// Create a new descriptor buffer.
    pub fn new(
        ctx: &super::context::VulkanContext,
        size: u64,
        caps: DescriptorBufferCapabilities,
        is_sampler_buffer: bool,
    ) -> Result<Self, String> {
        let usage = if is_sampler_buffer {
            vk::BufferUsageFlags::SAMPLER_DESCRIPTOR_BUFFER_EXT | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
        } else {
            vk::BufferUsageFlags::RESOURCE_DESCRIPTOR_BUFFER_EXT | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
        };

        let create_info = vk::BufferCreateInfo::default()
            .size(size)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let buffer = unsafe {
            ctx.device.create_buffer(&create_info, None)
                .map_err(|e| format!("Failed to create descriptor buffer: {:?}", e))?
        };

        // Get memory requirements
        let mem_reqs = unsafe { ctx.device.get_buffer_memory_requirements(buffer) };

        // Allocate host-visible memory
        let alloc_info = vk::MemoryAllocateInfo::default()
            .allocation_size(mem_reqs.size)
            .memory_type_index(Self::find_memory_type(
                ctx,
                mem_reqs.memory_type_bits,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            )?);

        let memory = unsafe {
            ctx.device.allocate_memory(&alloc_info, None)
                .map_err(|e| format!("Failed to allocate descriptor buffer memory: {:?}", e))?
        };

        unsafe {
            ctx.device.bind_buffer_memory(buffer, memory, 0)
                .map_err(|e| format!("Failed to bind descriptor buffer memory: {:?}", e))?;
        }

        // Get buffer device address
        let address_info = vk::BufferDeviceAddressInfo::default().buffer(buffer);
        let address = unsafe { ctx.device.get_buffer_device_address(&address_info) };

        // Map memory
        let mapped = unsafe {
            ctx.device.map_memory(memory, 0, size, vk::MemoryMapFlags::empty())
                .map_err(|e| format!("Failed to map descriptor buffer: {:?}", e))?
        } as *mut u8;

        Ok(Self {
            buffer,
            memory,
            address,
            size,
            mapped,
            caps,
        })
    }

    /// Find a suitable memory type.
    fn find_memory_type(
        ctx: &super::context::VulkanContext,
        type_filter: u32,
        properties: vk::MemoryPropertyFlags,
    ) -> Result<u32, String> {
        let mem_props = unsafe {
            ctx.instance.get_physical_device_memory_properties(ctx.physical_device)
        };

        for i in 0..mem_props.memory_type_count {
            if (type_filter & (1 << i)) != 0 &&
               (mem_props.memory_types[i as usize].property_flags & properties) == properties {
                return Ok(i);
            }
        }

        Err("Failed to find suitable memory type".to_string())
    }

    /// Get the buffer device address.
    pub fn address(&self) -> vk::DeviceAddress {
        self.address
    }

    /// Get the buffer handle.
    pub fn handle(&self) -> vk::Buffer {
        self.buffer
    }

    /// Write a uniform buffer descriptor.
    pub fn write_uniform_buffer(&mut self, offset: u64, buffer: vk::Buffer, buffer_offset: u64, range: u64) {
        let desc_size = self.caps.uniform_buffer_descriptor_size;
        if offset + desc_size as u64 > self.size {
            return;
        }

        // Write descriptor data at offset
        let desc_info = vk::DescriptorAddressInfoEXT::default()
            .address(buffer_offset) // This should be the device address
            .range(range);

        unsafe {
            let ptr = self.mapped.add(offset as usize);
            std::ptr::copy_nonoverlapping(
                &desc_info as *const _ as *const u8,
                ptr,
                desc_size.min(std::mem::size_of::<vk::DescriptorAddressInfoEXT>()),
            );
        }
    }

    /// Write a storage buffer descriptor.
    pub fn write_storage_buffer(&mut self, offset: u64, buffer_address: vk::DeviceAddress, range: u64) {
        let desc_size = self.caps.storage_buffer_descriptor_size;
        if offset + desc_size as u64 > self.size {
            return;
        }

        let desc_info = vk::DescriptorAddressInfoEXT::default()
            .address(buffer_address)
            .range(range);

        unsafe {
            let ptr = self.mapped.add(offset as usize);
            std::ptr::copy_nonoverlapping(
                &desc_info as *const _ as *const u8,
                ptr,
                desc_size.min(std::mem::size_of::<vk::DescriptorAddressInfoEXT>()),
            );
        }
    }

    /// Write a combined image sampler descriptor.
    pub fn write_combined_image_sampler(
        &mut self,
        offset: u64,
        image_view: vk::ImageView,
        sampler: vk::Sampler,
        layout: vk::ImageLayout,
    ) {
        let desc_size = self.caps.combined_image_sampler_descriptor_size;
        if offset + desc_size as u64 > self.size {
            return;
        }

        let desc_info = vk::DescriptorImageInfo::default()
            .image_view(image_view)
            .sampler(sampler)
            .image_layout(layout);

        unsafe {
            let ptr = self.mapped.add(offset as usize);
            std::ptr::copy_nonoverlapping(
                &desc_info as *const _ as *const u8,
                ptr,
                desc_size.min(std::mem::size_of::<vk::DescriptorImageInfo>()),
            );
        }
    }

    /// Destroy the buffer.
    pub fn destroy(&mut self, ctx: &super::context::VulkanContext) {
        unsafe {
            ctx.device.unmap_memory(self.memory);
            ctx.device.destroy_buffer(self.buffer, None);
            ctx.device.free_memory(self.memory, None);
        }
    }
}

/// Bind descriptor buffers to a command buffer.
pub fn cmd_bind_descriptor_buffers(
    ctx: &super::context::VulkanContext,
    cmd: vk::CommandBuffer,
    buffers: &[&DescriptorBuffer],
) {
    if buffers.is_empty() {
        return;
    }

    let fp = unsafe {
        let name = std::ffi::CStr::from_bytes_with_nul_unchecked(b"vkCmdBindDescriptorBuffersEXT\0");
        ctx.instance.get_device_proc_addr(ctx.device.handle(), name.as_ptr())
    };

    if let Some(fp) = fp {
        let binding_infos: Vec<vk::DescriptorBufferBindingInfoEXT> = buffers
            .iter()
            .map(|b| {
                vk::DescriptorBufferBindingInfoEXT::default()
                    .address(b.address())
                    .usage(vk::BufferUsageFlags::RESOURCE_DESCRIPTOR_BUFFER_EXT)
            })
            .collect();

        unsafe {
            let func: vk::PFN_vkCmdBindDescriptorBuffersEXT = std::mem::transmute(fp);
            func(cmd, binding_infos.len() as u32, binding_infos.as_ptr());
        }
    }
}

/// Set descriptor buffer offsets.
pub fn cmd_set_descriptor_buffer_offsets(
    ctx: &super::context::VulkanContext,
    cmd: vk::CommandBuffer,
    pipeline_bind_point: vk::PipelineBindPoint,
    layout: vk::PipelineLayout,
    first_set: u32,
    buffer_indices: &[u32],
    offsets: &[vk::DeviceSize],
) {
    let fp = unsafe {
        let name = std::ffi::CStr::from_bytes_with_nul_unchecked(b"vkCmdSetDescriptorBufferOffsetsEXT\0");
        ctx.instance.get_device_proc_addr(ctx.device.handle(), name.as_ptr())
    };

    if let Some(fp) = fp {
        unsafe {
            let func: vk::PFN_vkCmdSetDescriptorBufferOffsetsEXT = std::mem::transmute(fp);
            func(
                cmd,
                pipeline_bind_point,
                layout,
                first_set,
                buffer_indices.len() as u32,
                buffer_indices.as_ptr(),
                offsets.as_ptr(),
            );
        }
    }
}
