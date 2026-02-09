//! GPU-driven culling using compute shaders and indirect draw
//!
//! Performs frustum and occlusion culling entirely on the GPU, eliminating
//! CPU-side visibility determination and draw call generation.
//!
//! Pipeline:
//! 1. Upload object data (bounds, transforms) to GPU
//! 2. Compute shader performs frustum culling against camera planes
//! 3. Optional: Hierarchical Z-buffer occlusion culling
//! 4. Generate indirect draw commands for visible objects
//! 5. Execute indirect draw with single API call

use ash::vk;
use std::mem::size_of;

/// Maximum number of objects for GPU culling.
pub const MAX_CULL_OBJECTS: u32 = 65536;

/// Object data for culling (GPU-side).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CullObjectData {
    /// Bounding sphere center (world space).
    pub center: [f32; 3],
    /// Bounding sphere radius.
    pub radius: f32,
    /// AABB min (world space).
    pub aabb_min: [f32; 3],
    /// Object index in the draw list.
    pub object_index: u32,
    /// AABB max (world space).
    pub aabb_max: [f32; 3],
    /// LOD distance or flags.
    pub lod_flags: u32,
}

/// Indirect draw command (matches VkDrawIndexedIndirectCommand).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct IndirectDrawCommand {
    pub index_count: u32,
    pub instance_count: u32,
    pub first_index: u32,
    pub vertex_offset: i32,
    pub first_instance: u32,
}

/// Culling uniforms passed to compute shader.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CullUniforms {
    /// View-projection matrix.
    pub view_proj: [[f32; 4]; 4],
    /// Frustum planes (6 planes, each as vec4).
    pub frustum_planes: [[f32; 4]; 6],
    /// Camera position.
    pub camera_pos: [f32; 3],
    /// Total number of objects.
    pub object_count: u32,
    /// Near plane distance.
    pub near_plane: f32,
    /// Far plane distance.
    pub far_plane: f32,
    /// Enable occlusion culling.
    pub occlusion_enabled: u32,
    /// Padding.
    pub _pad: u32,
}

impl Default for CullUniforms {
    fn default() -> Self {
        Self {
            view_proj: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
            frustum_planes: [[0.0; 4]; 6],
            camera_pos: [0.0; 3],
            object_count: 0,
            near_plane: 0.1,
            far_plane: 4096.0,
            occlusion_enabled: 0,
            _pad: 0,
        }
    }
}

/// GPU culling statistics.
#[derive(Clone, Copy, Debug, Default)]
pub struct CullStats {
    /// Total objects submitted.
    pub total_objects: u32,
    /// Objects visible after frustum culling.
    pub visible_objects: u32,
    /// Objects culled by frustum.
    pub frustum_culled: u32,
    /// Objects culled by occlusion.
    pub occlusion_culled: u32,
}

/// GPU-driven culling system.
pub struct GpuCullingSystem {
    /// Whether the system is initialized.
    initialized: bool,
    /// Object data buffer.
    object_buffer: vk::Buffer,
    object_memory: vk::DeviceMemory,
    /// Indirect draw buffer.
    indirect_buffer: vk::Buffer,
    indirect_memory: vk::DeviceMemory,
    /// Draw count buffer (for indirect count).
    count_buffer: vk::Buffer,
    count_memory: vk::DeviceMemory,
    /// Visibility buffer (output from compute).
    visibility_buffer: vk::Buffer,
    visibility_memory: vk::DeviceMemory,
    /// Uniform buffer.
    uniform_buffer: vk::Buffer,
    uniform_memory: vk::DeviceMemory,
    /// Compute pipeline for culling.
    cull_pipeline: vk::Pipeline,
    /// Pipeline layout.
    pipeline_layout: vk::PipelineLayout,
    /// Descriptor set layout.
    descriptor_layout: vk::DescriptorSetLayout,
    /// Descriptor pool.
    descriptor_pool: vk::DescriptorPool,
    /// Descriptor set.
    descriptor_set: vk::DescriptorSet,
    /// Hierarchical Z-buffer for occlusion culling.
    hiz_image: vk::Image,
    hiz_view: vk::ImageView,
    hiz_memory: vk::DeviceMemory,
    /// HiZ dimensions.
    hiz_width: u32,
    hiz_height: u32,
    hiz_mip_levels: u32,
    /// Current frame's uniforms.
    uniforms: CullUniforms,
    /// Number of registered objects.
    object_count: u32,
}

impl GpuCullingSystem {
    /// Create a new GPU culling system.
    pub fn new(ctx: &super::context::VulkanContext) -> Result<Self, String> {
        // Create descriptor set layout
        let bindings = [
            // Binding 0: Object data (read)
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE),
            // Binding 1: Indirect commands (write)
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE),
            // Binding 2: Draw count (write)
            vk::DescriptorSetLayoutBinding::default()
                .binding(2)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE),
            // Binding 3: Visibility output (write)
            vk::DescriptorSetLayoutBinding::default()
                .binding(3)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE),
            // Binding 4: Uniforms
            vk::DescriptorSetLayoutBinding::default()
                .binding(4)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE),
            // Binding 5: HiZ texture (optional)
            vk::DescriptorSetLayoutBinding::default()
                .binding(5)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE),
        ];

        let layout_info = vk::DescriptorSetLayoutCreateInfo::default()
            .bindings(&bindings);

        let descriptor_layout = unsafe {
            ctx.device.create_descriptor_set_layout(&layout_info, None)
                .map_err(|e| format!("Failed to create cull descriptor layout: {:?}", e))?
        };

        // Create pipeline layout
        let layouts = [descriptor_layout];
        let layout_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(&layouts);

        let pipeline_layout = unsafe {
            ctx.device.create_pipeline_layout(&layout_info, None)
                .map_err(|e| format!("Failed to create cull pipeline layout: {:?}", e))?
        };

        // Create buffers
        let object_size = (MAX_CULL_OBJECTS as usize) * size_of::<CullObjectData>();
        let indirect_size = (MAX_CULL_OBJECTS as usize) * size_of::<IndirectDrawCommand>();
        let count_size = size_of::<u32>();
        let visibility_size = (MAX_CULL_OBJECTS as usize) * size_of::<u32>();
        let uniform_size = size_of::<CullUniforms>();

        let (object_buffer, object_memory) = Self::create_buffer(
            ctx,
            object_size as u64,
            vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
        )?;

        let (indirect_buffer, indirect_memory) = Self::create_buffer(
            ctx,
            indirect_size as u64,
            vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::INDIRECT_BUFFER,
        )?;

        let (count_buffer, count_memory) = Self::create_buffer(
            ctx,
            count_size as u64,
            vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::INDIRECT_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
        )?;

        let (visibility_buffer, visibility_memory) = Self::create_buffer(
            ctx,
            visibility_size as u64,
            vk::BufferUsageFlags::STORAGE_BUFFER,
        )?;

        let (uniform_buffer, uniform_memory) = Self::create_buffer(
            ctx,
            uniform_size as u64,
            vk::BufferUsageFlags::UNIFORM_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
        )?;

        // Create descriptor pool
        let pool_sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::STORAGE_BUFFER,
                descriptor_count: 4,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: 1,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: 1,
            },
        ];

        let pool_info = vk::DescriptorPoolCreateInfo::default()
            .pool_sizes(&pool_sizes)
            .max_sets(1);

        let descriptor_pool = unsafe {
            ctx.device.create_descriptor_pool(&pool_info, None)
                .map_err(|e| format!("Failed to create cull descriptor pool: {:?}", e))?
        };

        // Allocate descriptor set
        let layouts = [descriptor_layout];
        let alloc_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&layouts);

        let descriptor_set = unsafe {
            ctx.device.allocate_descriptor_sets(&alloc_info)
                .map_err(|e| format!("Failed to allocate cull descriptor set: {:?}", e))?[0]
        };

        // Update descriptors for buffers
        Self::update_buffer_descriptors(
            ctx,
            descriptor_set,
            object_buffer,
            indirect_buffer,
            count_buffer,
            visibility_buffer,
            uniform_buffer,
        );

        Ok(Self {
            initialized: true,
            object_buffer,
            object_memory,
            indirect_buffer,
            indirect_memory,
            count_buffer,
            count_memory,
            visibility_buffer,
            visibility_memory,
            uniform_buffer,
            uniform_memory,
            cull_pipeline: vk::Pipeline::null(), // Created when shader is loaded
            pipeline_layout,
            descriptor_layout,
            descriptor_pool,
            descriptor_set,
            hiz_image: vk::Image::null(),
            hiz_view: vk::ImageView::null(),
            hiz_memory: vk::DeviceMemory::null(),
            hiz_width: 0,
            hiz_height: 0,
            hiz_mip_levels: 0,
            uniforms: CullUniforms::default(),
            object_count: 0,
        })
    }

    /// Create a buffer with device-local memory.
    fn create_buffer(
        ctx: &super::context::VulkanContext,
        size: u64,
        usage: vk::BufferUsageFlags,
    ) -> Result<(vk::Buffer, vk::DeviceMemory), String> {
        let buffer_info = vk::BufferCreateInfo::default()
            .size(size)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let buffer = unsafe {
            ctx.device.create_buffer(&buffer_info, None)
                .map_err(|e| format!("Failed to create buffer: {:?}", e))?
        };

        let mem_reqs = unsafe { ctx.device.get_buffer_memory_requirements(buffer) };

        let mem_props = unsafe {
            ctx.instance.get_physical_device_memory_properties(ctx.physical_device)
        };

        let memory_type = Self::find_memory_type(
            mem_reqs.memory_type_bits,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
            &mem_props,
        ).ok_or_else(|| "Failed to find suitable memory type".to_string())?;

        let alloc_info = vk::MemoryAllocateInfo::default()
            .allocation_size(mem_reqs.size)
            .memory_type_index(memory_type);

        let memory = unsafe {
            ctx.device.allocate_memory(&alloc_info, None)
                .map_err(|e| format!("Failed to allocate buffer memory: {:?}", e))?
        };

        unsafe {
            ctx.device.bind_buffer_memory(buffer, memory, 0)
                .map_err(|e| format!("Failed to bind buffer memory: {:?}", e))?;
        }

        Ok((buffer, memory))
    }

    fn find_memory_type(
        type_filter: u32,
        properties: vk::MemoryPropertyFlags,
        mem_props: &vk::PhysicalDeviceMemoryProperties,
    ) -> Option<u32> {
        for i in 0..mem_props.memory_type_count {
            if (type_filter & (1 << i)) != 0
                && mem_props.memory_types[i as usize].property_flags.contains(properties)
            {
                return Some(i);
            }
        }
        None
    }

    fn update_buffer_descriptors(
        ctx: &super::context::VulkanContext,
        descriptor_set: vk::DescriptorSet,
        object_buffer: vk::Buffer,
        indirect_buffer: vk::Buffer,
        count_buffer: vk::Buffer,
        visibility_buffer: vk::Buffer,
        uniform_buffer: vk::Buffer,
    ) {
        let object_info = vk::DescriptorBufferInfo::default()
            .buffer(object_buffer)
            .offset(0)
            .range(vk::WHOLE_SIZE);

        let indirect_info = vk::DescriptorBufferInfo::default()
            .buffer(indirect_buffer)
            .offset(0)
            .range(vk::WHOLE_SIZE);

        let count_info = vk::DescriptorBufferInfo::default()
            .buffer(count_buffer)
            .offset(0)
            .range(vk::WHOLE_SIZE);

        let visibility_info = vk::DescriptorBufferInfo::default()
            .buffer(visibility_buffer)
            .offset(0)
            .range(vk::WHOLE_SIZE);

        let uniform_info = vk::DescriptorBufferInfo::default()
            .buffer(uniform_buffer)
            .offset(0)
            .range(vk::WHOLE_SIZE);

        let writes = [
            vk::WriteDescriptorSet::default()
                .dst_set(descriptor_set)
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .buffer_info(std::slice::from_ref(&object_info)),
            vk::WriteDescriptorSet::default()
                .dst_set(descriptor_set)
                .dst_binding(1)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .buffer_info(std::slice::from_ref(&indirect_info)),
            vk::WriteDescriptorSet::default()
                .dst_set(descriptor_set)
                .dst_binding(2)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .buffer_info(std::slice::from_ref(&count_info)),
            vk::WriteDescriptorSet::default()
                .dst_set(descriptor_set)
                .dst_binding(3)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .buffer_info(std::slice::from_ref(&visibility_info)),
            vk::WriteDescriptorSet::default()
                .dst_set(descriptor_set)
                .dst_binding(4)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .buffer_info(std::slice::from_ref(&uniform_info)),
        ];

        unsafe {
            ctx.device.update_descriptor_sets(&writes, &[]);
        }
    }

    /// Update frustum planes from view-projection matrix.
    pub fn set_view_projection(&mut self, view_proj: [[f32; 4]; 4], camera_pos: [f32; 3]) {
        self.uniforms.view_proj = view_proj;
        self.uniforms.camera_pos = camera_pos;

        // Extract frustum planes from view-projection matrix
        self.uniforms.frustum_planes = Self::extract_frustum_planes(&view_proj);
    }

    /// Extract frustum planes from a view-projection matrix.
    fn extract_frustum_planes(m: &[[f32; 4]; 4]) -> [[f32; 4]; 6] {
        let mut planes = [[0.0f32; 4]; 6];

        // Left plane
        planes[0] = [
            m[0][3] + m[0][0],
            m[1][3] + m[1][0],
            m[2][3] + m[2][0],
            m[3][3] + m[3][0],
        ];

        // Right plane
        planes[1] = [
            m[0][3] - m[0][0],
            m[1][3] - m[1][0],
            m[2][3] - m[2][0],
            m[3][3] - m[3][0],
        ];

        // Bottom plane
        planes[2] = [
            m[0][3] + m[0][1],
            m[1][3] + m[1][1],
            m[2][3] + m[2][1],
            m[3][3] + m[3][1],
        ];

        // Top plane
        planes[3] = [
            m[0][3] - m[0][1],
            m[1][3] - m[1][1],
            m[2][3] - m[2][1],
            m[3][3] - m[3][1],
        ];

        // Near plane
        planes[4] = [
            m[0][3] + m[0][2],
            m[1][3] + m[1][2],
            m[2][3] + m[2][2],
            m[3][3] + m[3][2],
        ];

        // Far plane
        planes[5] = [
            m[0][3] - m[0][2],
            m[1][3] - m[1][2],
            m[2][3] - m[2][2],
            m[3][3] - m[3][2],
        ];

        // Normalize planes
        for plane in &mut planes {
            let len = (plane[0] * plane[0] + plane[1] * plane[1] + plane[2] * plane[2]).sqrt();
            if len > 0.0 {
                plane[0] /= len;
                plane[1] /= len;
                plane[2] /= len;
                plane[3] /= len;
            }
        }

        planes
    }

    /// Get the indirect draw buffer.
    pub fn indirect_buffer(&self) -> vk::Buffer {
        self.indirect_buffer
    }

    /// Get the draw count buffer.
    pub fn count_buffer(&self) -> vk::Buffer {
        self.count_buffer
    }

    /// Get the pipeline layout.
    pub fn pipeline_layout(&self) -> vk::PipelineLayout {
        self.pipeline_layout
    }

    /// Dispatch the culling compute shader.
    pub fn dispatch_culling(&self, ctx: &super::context::VulkanContext, cmd: vk::CommandBuffer) {
        if self.cull_pipeline == vk::Pipeline::null() || self.object_count == 0 {
            return;
        }

        unsafe {
            // Bind compute pipeline
            ctx.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, self.cull_pipeline);

            // Bind descriptor set
            ctx.device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::COMPUTE,
                self.pipeline_layout,
                0,
                &[self.descriptor_set],
                &[],
            );

            // Dispatch with 64 threads per workgroup
            let workgroups = (self.object_count + 63) / 64;
            ctx.device.cmd_dispatch(cmd, workgroups, 1, 1);
        }
    }

    /// Record indirect draw commands.
    pub fn record_indirect_draw(
        &self,
        ctx: &super::context::VulkanContext,
        cmd: vk::CommandBuffer,
        max_draw_count: u32,
    ) {
        unsafe {
            // Use vkCmdDrawIndexedIndirectCount for variable draw count
            ctx.device.cmd_draw_indexed_indirect_count(
                cmd,
                self.indirect_buffer,
                0,
                self.count_buffer,
                0,
                max_draw_count,
                std::mem::size_of::<IndirectDrawCommand>() as u32,
            );
        }
    }

    /// Destroy the GPU culling system.
    pub fn destroy(&mut self, ctx: &super::context::VulkanContext) {
        unsafe {
            if self.cull_pipeline != vk::Pipeline::null() {
                ctx.device.destroy_pipeline(self.cull_pipeline, None);
            }
            ctx.device.destroy_pipeline_layout(self.pipeline_layout, None);
            ctx.device.destroy_descriptor_pool(self.descriptor_pool, None);
            ctx.device.destroy_descriptor_set_layout(self.descriptor_layout, None);

            ctx.device.destroy_buffer(self.object_buffer, None);
            ctx.device.free_memory(self.object_memory, None);
            ctx.device.destroy_buffer(self.indirect_buffer, None);
            ctx.device.free_memory(self.indirect_memory, None);
            ctx.device.destroy_buffer(self.count_buffer, None);
            ctx.device.free_memory(self.count_memory, None);
            ctx.device.destroy_buffer(self.visibility_buffer, None);
            ctx.device.free_memory(self.visibility_memory, None);
            ctx.device.destroy_buffer(self.uniform_buffer, None);
            ctx.device.free_memory(self.uniform_memory, None);

            if self.hiz_image != vk::Image::null() {
                ctx.device.destroy_image_view(self.hiz_view, None);
                ctx.device.destroy_image(self.hiz_image, None);
                ctx.device.free_memory(self.hiz_memory, None);
            }
        }

        self.initialized = false;
    }
}

/// GLSL compute shader for GPU culling.
pub const CULL_COMPUTE_GLSL: &str = r#"
#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(local_size_x = 64, local_size_y = 1, local_size_z = 1) in;

struct CullObjectData {
    vec3 center;
    float radius;
    vec3 aabb_min;
    uint object_index;
    vec3 aabb_max;
    uint lod_flags;
};

struct IndirectDrawCommand {
    uint index_count;
    uint instance_count;
    uint first_index;
    int vertex_offset;
    uint first_instance;
};

layout(std430, set = 0, binding = 0) readonly buffer ObjectBuffer {
    CullObjectData objects[];
};

layout(std430, set = 0, binding = 1) writeonly buffer IndirectBuffer {
    IndirectDrawCommand commands[];
};

layout(std430, set = 0, binding = 2) buffer CountBuffer {
    uint draw_count;
};

layout(std430, set = 0, binding = 3) writeonly buffer VisibilityBuffer {
    uint visibility[];
};

layout(std140, set = 0, binding = 4) uniform CullUniforms {
    mat4 view_proj;
    vec4 frustum_planes[6];
    vec3 camera_pos;
    uint object_count;
    float near_plane;
    float far_plane;
    uint occlusion_enabled;
    uint _pad;
};

layout(set = 0, binding = 5) uniform sampler2D hiz_texture;

// Test sphere against frustum planes
bool frustumCullSphere(vec3 center, float radius) {
    for (int i = 0; i < 6; i++) {
        float dist = dot(frustum_planes[i].xyz, center) + frustum_planes[i].w;
        if (dist < -radius) {
            return true; // Culled
        }
    }
    return false; // Visible
}

void main() {
    uint gid = gl_GlobalInvocationID.x;

    if (gid >= object_count) {
        return;
    }

    CullObjectData obj = objects[gid];

    // Frustum culling
    bool culled = frustumCullSphere(obj.center, obj.radius);

    if (!culled) {
        // Object is visible, add to draw list
        uint draw_idx = atomicAdd(draw_count, 1);

        // Write indirect command (placeholder - actual data comes from mesh)
        commands[draw_idx].instance_count = 1;
        commands[draw_idx].first_instance = obj.object_index;

        visibility[gid] = 1;
    } else {
        visibility[gid] = 0;
    }
}
"#;
