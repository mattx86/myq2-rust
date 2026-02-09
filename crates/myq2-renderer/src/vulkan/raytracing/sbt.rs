//! Shader Binding Table (SBT) management for ray tracing pipelines.

use ash::vk;

use crate::vulkan::{VulkanContext, MemoryManager, Buffer};

/// Shader Binding Table for ray tracing.
pub struct ShaderBindingTable {
    pub buffer: Buffer,
    pub raygen_region: vk::StridedDeviceAddressRegionKHR,
    pub miss_region: vk::StridedDeviceAddressRegionKHR,
    pub hit_region: vk::StridedDeviceAddressRegionKHR,
    pub callable_region: vk::StridedDeviceAddressRegionKHR,
}

impl ShaderBindingTable {
    /// Create a shader binding table from a ray tracing pipeline.
    ///
    /// # Arguments
    /// * `pipeline` - The ray tracing pipeline
    /// * `raygen_count` - Number of ray generation shaders (usually 1)
    /// * `miss_count` - Number of miss shaders
    /// * `hit_count` - Number of hit groups
    /// * `callable_count` - Number of callable shaders
    pub unsafe fn new(
        ctx: &VulkanContext,
        memory: &MemoryManager,
        pipeline: vk::Pipeline,
        raygen_count: u32,
        miss_count: u32,
        hit_count: u32,
        callable_count: u32,
    ) -> Result<Self, String> {
        let rt_loader = ctx.rt_pipeline_loader.as_ref()
            .ok_or("Ray tracing not supported")?;

        let props = &ctx.rt_capabilities;
        let handle_size = props.shader_group_handle_size;
        let handle_alignment = 32u32; // Typical alignment requirement
        let handle_size_aligned = align_up(handle_size, handle_alignment);

        let group_count = raygen_count + miss_count + hit_count + callable_count;

        // Get shader group handles
        let handles_size = (group_count * handle_size) as usize;
        let handles = rt_loader.get_ray_tracing_shader_group_handles(
            pipeline,
            0,
            group_count,
            handles_size,
        ).map_err(|e| format!("Failed to get shader group handles: {:?}", e))?;

        // Calculate regions
        let raygen_size = align_up(handle_size_aligned * raygen_count, 64);
        let miss_size = align_up(handle_size_aligned * miss_count, 64);
        let hit_size = align_up(handle_size_aligned * hit_count, 64);
        let callable_size = align_up(handle_size_aligned * callable_count, 64);

        let total_size = (raygen_size + miss_size + hit_size + callable_size) as vk::DeviceSize;

        // Create SBT buffer
        let buffer = memory.create_buffer(
            total_size,
            vk::BufferUsageFlags::SHADER_BINDING_TABLE_KHR |
            vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            gpu_allocator::MemoryLocation::CpuToGpu,
            "shader_binding_table",
        )?;

        let base_address = buffer.device_address.unwrap();

        // Copy handles to buffer with proper alignment
        if let Some(ptr) = buffer.mapped_ptr() {
            let mut offset = 0usize;
            let mut handle_offset = 0usize;

            // Raygen
            for _ in 0..raygen_count {
                std::ptr::copy_nonoverlapping(
                    handles[handle_offset..].as_ptr(),
                    ptr.add(offset),
                    handle_size as usize,
                );
                offset += handle_size_aligned as usize;
                handle_offset += handle_size as usize;
            }
            offset = raygen_size as usize;

            // Miss
            for _ in 0..miss_count {
                std::ptr::copy_nonoverlapping(
                    handles[handle_offset..].as_ptr(),
                    ptr.add(offset),
                    handle_size as usize,
                );
                offset += handle_size_aligned as usize;
                handle_offset += handle_size as usize;
            }
            offset = (raygen_size + miss_size) as usize;

            // Hit
            for _ in 0..hit_count {
                std::ptr::copy_nonoverlapping(
                    handles[handle_offset..].as_ptr(),
                    ptr.add(offset),
                    handle_size as usize,
                );
                offset += handle_size_aligned as usize;
                handle_offset += handle_size as usize;
            }
            offset = (raygen_size + miss_size + hit_size) as usize;

            // Callable
            for _ in 0..callable_count {
                std::ptr::copy_nonoverlapping(
                    handles[handle_offset..].as_ptr(),
                    ptr.add(offset),
                    handle_size as usize,
                );
                offset += handle_size_aligned as usize;
                handle_offset += handle_size as usize;
            }
        }

        let raygen_region = vk::StridedDeviceAddressRegionKHR {
            device_address: base_address,
            stride: handle_size_aligned as vk::DeviceSize,
            size: raygen_size as vk::DeviceSize,
        };

        let miss_region = vk::StridedDeviceAddressRegionKHR {
            device_address: if miss_count > 0 { base_address + raygen_size as u64 } else { 0 },
            stride: handle_size_aligned as vk::DeviceSize,
            size: miss_size as vk::DeviceSize,
        };

        let hit_region = vk::StridedDeviceAddressRegionKHR {
            device_address: if hit_count > 0 { base_address + (raygen_size + miss_size) as u64 } else { 0 },
            stride: handle_size_aligned as vk::DeviceSize,
            size: hit_size as vk::DeviceSize,
        };

        let callable_region = vk::StridedDeviceAddressRegionKHR {
            device_address: if callable_count > 0 { base_address + (raygen_size + miss_size + hit_size) as u64 } else { 0 },
            stride: handle_size_aligned as vk::DeviceSize,
            size: callable_size as vk::DeviceSize,
        };

        Ok(Self {
            buffer,
            raygen_region,
            miss_region,
            hit_region,
            callable_region,
        })
    }

    /// Record a trace rays command.
    pub unsafe fn trace_rays(
        &self,
        ctx: &VulkanContext,
        cmd: vk::CommandBuffer,
        width: u32,
        height: u32,
        depth: u32,
    ) {
        if let Some(rt_loader) = &ctx.rt_pipeline_loader {
            rt_loader.cmd_trace_rays(
                cmd,
                &self.raygen_region,
                &self.miss_region,
                &self.hit_region,
                &self.callable_region,
                width,
                height,
                depth,
            );
        }
    }

    /// Destroy the shader binding table.
    pub unsafe fn destroy(self, memory: &MemoryManager) {
        memory.destroy_buffer(self.buffer);
    }
}

/// Align a value up to the given alignment.
fn align_up(value: u32, alignment: u32) -> u32 {
    (value + alignment - 1) & !(alignment - 1)
}
