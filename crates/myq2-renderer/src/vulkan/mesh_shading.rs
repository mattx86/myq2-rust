//! Mesh Shader support for GPU-driven rendering
//!
//! Mesh shaders replace the traditional vertex/geometry shader pipeline with
//! a more flexible task/mesh shader model. This enables:
//! - GPU-side culling (frustum, occlusion)
//! - Meshlet-based rendering for better cache utilization
//! - Reduced CPU draw call overhead
//!
//! Requires VK_EXT_mesh_shader extension.

use ash::vk;
use super::context::VulkanContext;

/// Maximum vertices per meshlet.
pub const MESHLET_MAX_VERTICES: u32 = 64;
/// Maximum triangles per meshlet.
pub const MESHLET_MAX_TRIANGLES: u32 = 124;

/// Mesh shader capabilities.
#[derive(Debug, Clone, Default)]
pub struct MeshShaderCapabilities {
    /// Whether mesh shaders are supported.
    pub supported: bool,
    /// Whether task shaders are supported.
    pub task_shader: bool,
    /// Maximum task work group count.
    pub max_task_work_group_count: [u32; 3],
    /// Maximum task work group invocations.
    pub max_task_work_group_invocations: u32,
    /// Maximum task work group size.
    pub max_task_work_group_size: [u32; 3],
    /// Maximum mesh work group count.
    pub max_mesh_work_group_count: [u32; 3],
    /// Maximum mesh work group invocations.
    pub max_mesh_work_group_invocations: u32,
    /// Maximum mesh work group size.
    pub max_mesh_work_group_size: [u32; 3],
    /// Maximum output vertices per mesh shader.
    pub max_mesh_output_vertices: u32,
    /// Maximum output primitives per mesh shader.
    pub max_mesh_output_primitives: u32,
    /// Maximum total memory size for mesh shaders.
    pub max_mesh_total_memory_size: u32,
    /// Whether multiview rendering is supported in mesh shaders.
    pub mesh_shader_multiview: bool,
}

/// A meshlet represents a small cluster of triangles for efficient GPU processing.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Meshlet {
    /// Offset into the vertex index buffer.
    pub vertex_offset: u32,
    /// Number of vertices in this meshlet.
    pub vertex_count: u32,
    /// Offset into the triangle index buffer.
    pub triangle_offset: u32,
    /// Number of triangles in this meshlet.
    pub triangle_count: u32,
    /// Bounding sphere center (for culling).
    pub center: [f32; 3],
    /// Bounding sphere radius.
    pub radius: f32,
    /// Cone axis for backface culling (normalized).
    pub cone_axis: [f32; 3],
    /// Cone cutoff angle (cos of half-angle).
    pub cone_cutoff: f32,
}

/// Meshlet bounds for culling.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct MeshletBounds {
    /// Bounding sphere center.
    pub center: [f32; 3],
    /// Bounding sphere radius.
    pub radius: f32,
    /// Normal cone axis.
    pub cone_axis: [f32; 3],
    /// Cone cutoff (cos of half-angle, negative if > 90 degrees).
    pub cone_cutoff: f32,
}

/// Mesh shader pipeline manager.
pub struct MeshShaderManager {
    /// Device capabilities.
    capabilities: MeshShaderCapabilities,
    /// Extension loader.
    mesh_loader: Option<ash::ext::mesh_shader::Device>,
    /// Whether mesh shaders are enabled.
    enabled: bool,
}

impl MeshShaderManager {
    /// Create a new mesh shader manager.
    pub fn new(ctx: &VulkanContext) -> Self {
        let capabilities = Self::query_capabilities(ctx);

        let mesh_loader = if capabilities.supported {
            Some(ash::ext::mesh_shader::Device::new(&ctx.instance, &ctx.device))
        } else {
            None
        };

        Self {
            capabilities,
            mesh_loader,
            enabled: false,
        }
    }

    /// Query mesh shader capabilities.
    fn query_capabilities(ctx: &VulkanContext) -> MeshShaderCapabilities {
        // Check if extension is available
        let extensions = unsafe {
            ctx.instance
                .enumerate_device_extension_properties(ctx.physical_device)
                .unwrap_or_default()
        };

        let has_mesh = extensions.iter().any(|ext| {
            let name = unsafe { std::ffi::CStr::from_ptr(ext.extension_name.as_ptr()) };
            name.to_bytes() == b"VK_EXT_mesh_shader"
        });

        if !has_mesh {
            return MeshShaderCapabilities::default();
        }

        // Query properties
        let mut mesh_props = vk::PhysicalDeviceMeshShaderPropertiesEXT::default();
        let mut props2 = vk::PhysicalDeviceProperties2::default().push_next(&mut mesh_props);

        unsafe {
            ctx.instance
                .get_physical_device_properties2(ctx.physical_device, &mut props2);
        }

        // Query features
        let mut mesh_features = vk::PhysicalDeviceMeshShaderFeaturesEXT::default();
        let mut features2 = vk::PhysicalDeviceFeatures2::default().push_next(&mut mesh_features);

        unsafe {
            ctx.instance
                .get_physical_device_features2(ctx.physical_device, &mut features2);
        }

        MeshShaderCapabilities {
            supported: mesh_features.mesh_shader == vk::TRUE,
            task_shader: mesh_features.task_shader == vk::TRUE,
            max_task_work_group_count: mesh_props.max_task_work_group_count,
            max_task_work_group_invocations: mesh_props.max_task_work_group_invocations,
            max_task_work_group_size: mesh_props.max_task_work_group_size,
            max_mesh_work_group_count: mesh_props.max_mesh_work_group_count,
            max_mesh_work_group_invocations: mesh_props.max_mesh_work_group_invocations,
            max_mesh_work_group_size: mesh_props.max_mesh_work_group_size,
            max_mesh_output_vertices: mesh_props.max_mesh_output_vertices,
            max_mesh_output_primitives: mesh_props.max_mesh_output_primitives,
            max_mesh_total_memory_size: mesh_props.max_mesh_output_memory_size,
            mesh_shader_multiview: mesh_features.multiview_mesh_shader == vk::TRUE,
        }
    }

    /// Check if mesh shaders are supported.
    pub fn is_supported(&self) -> bool {
        self.capabilities.supported
    }

    /// Check if task shaders are supported.
    pub fn supports_task_shader(&self) -> bool {
        self.capabilities.task_shader
    }

    /// Get capabilities.
    pub fn capabilities(&self) -> &MeshShaderCapabilities {
        &self.capabilities
    }

    /// Enable or disable mesh shaders.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled && self.capabilities.supported;
    }

    /// Check if mesh shaders are enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Record a mesh shader draw command.
    ///
    /// # Arguments
    /// * `cmd` - Command buffer
    /// * `group_count_x` - Number of task/mesh work groups in X
    /// * `group_count_y` - Number of task/mesh work groups in Y
    /// * `group_count_z` - Number of task/mesh work groups in Z
    pub fn cmd_draw_mesh_tasks(
        &self,
        cmd: vk::CommandBuffer,
        group_count_x: u32,
        group_count_y: u32,
        group_count_z: u32,
    ) {
        if !self.enabled {
            return;
        }

        if let Some(ref loader) = self.mesh_loader {
            unsafe {
                loader.cmd_draw_mesh_tasks(cmd, group_count_x, group_count_y, group_count_z);
            }
        }
    }

    /// Record an indirect mesh shader draw command.
    ///
    /// # Arguments
    /// * `cmd` - Command buffer
    /// * `buffer` - Buffer containing VkDrawMeshTasksIndirectCommandEXT
    /// * `offset` - Byte offset into buffer
    /// * `draw_count` - Number of draws
    /// * `stride` - Stride between commands
    pub fn cmd_draw_mesh_tasks_indirect(
        &self,
        cmd: vk::CommandBuffer,
        buffer: vk::Buffer,
        offset: vk::DeviceSize,
        draw_count: u32,
        stride: u32,
    ) {
        if !self.enabled {
            return;
        }

        if let Some(ref loader) = self.mesh_loader {
            unsafe {
                loader.cmd_draw_mesh_tasks_indirect(cmd, buffer, offset, draw_count, stride);
            }
        }
    }

    /// Record an indirect mesh shader draw command with count buffer.
    pub fn cmd_draw_mesh_tasks_indirect_count(
        &self,
        cmd: vk::CommandBuffer,
        buffer: vk::Buffer,
        offset: vk::DeviceSize,
        count_buffer: vk::Buffer,
        count_buffer_offset: vk::DeviceSize,
        max_draw_count: u32,
        stride: u32,
    ) {
        if !self.enabled {
            return;
        }

        if let Some(ref loader) = self.mesh_loader {
            unsafe {
                loader.cmd_draw_mesh_tasks_indirect_count(
                    cmd,
                    buffer,
                    offset,
                    count_buffer,
                    count_buffer_offset,
                    max_draw_count,
                    stride,
                );
            }
        }
    }
}

impl Default for MeshShaderManager {
    fn default() -> Self {
        Self {
            capabilities: MeshShaderCapabilities::default(),
            mesh_loader: None,
            enabled: false,
        }
    }
}

/// Generate meshlets from a triangle mesh.
///
/// # Arguments
/// * `vertices` - Vertex positions (3 floats per vertex)
/// * `indices` - Triangle indices (3 indices per triangle)
///
/// # Returns
/// Vector of meshlets covering all triangles.
pub fn generate_meshlets(vertices: &[[f32; 3]], indices: &[u32]) -> Vec<Meshlet> {
    let triangle_count = indices.len() / 3;
    let mut meshlets = Vec::new();

    let mut current_meshlet = Meshlet::default();
    let mut meshlet_vertices: Vec<u32> = Vec::with_capacity(MESHLET_MAX_VERTICES as usize);
    let mut meshlet_triangles: Vec<[u32; 3]> = Vec::with_capacity(MESHLET_MAX_TRIANGLES as usize);

    for tri_idx in 0..triangle_count {
        let i0 = indices[tri_idx * 3];
        let i1 = indices[tri_idx * 3 + 1];
        let i2 = indices[tri_idx * 3 + 2];

        // Check if we need to add new vertices
        let mut new_vertex_count = 0;
        if !meshlet_vertices.contains(&i0) {
            new_vertex_count += 1;
        }
        if !meshlet_vertices.contains(&i1) {
            new_vertex_count += 1;
        }
        if !meshlet_vertices.contains(&i2) {
            new_vertex_count += 1;
        }

        // Check if we exceed limits
        let would_exceed_vertices =
            meshlet_vertices.len() + new_vertex_count > MESHLET_MAX_VERTICES as usize;
        let would_exceed_triangles = meshlet_triangles.len() >= MESHLET_MAX_TRIANGLES as usize;

        if would_exceed_vertices || would_exceed_triangles {
            // Finalize current meshlet
            if !meshlet_triangles.is_empty() {
                current_meshlet.vertex_count = meshlet_vertices.len() as u32;
                current_meshlet.triangle_count = meshlet_triangles.len() as u32;
                compute_meshlet_bounds(&mut current_meshlet, vertices, &meshlet_vertices);
                meshlets.push(current_meshlet);
            }

            // Start new meshlet
            current_meshlet = Meshlet::default();
            current_meshlet.vertex_offset = meshlets
                .iter()
                .map(|m| m.vertex_count)
                .sum::<u32>();
            current_meshlet.triangle_offset = meshlets
                .iter()
                .map(|m| m.triangle_count)
                .sum::<u32>();
            meshlet_vertices.clear();
            meshlet_triangles.clear();
        }

        // Add vertices if not present
        if !meshlet_vertices.contains(&i0) {
            meshlet_vertices.push(i0);
        }
        if !meshlet_vertices.contains(&i1) {
            meshlet_vertices.push(i1);
        }
        if !meshlet_vertices.contains(&i2) {
            meshlet_vertices.push(i2);
        }

        // Add triangle
        meshlet_triangles.push([i0, i1, i2]);
    }

    // Finalize last meshlet
    if !meshlet_triangles.is_empty() {
        current_meshlet.vertex_count = meshlet_vertices.len() as u32;
        current_meshlet.triangle_count = meshlet_triangles.len() as u32;
        compute_meshlet_bounds(&mut current_meshlet, vertices, &meshlet_vertices);
        meshlets.push(current_meshlet);
    }

    meshlets
}

/// Compute bounding sphere and normal cone for a meshlet.
fn compute_meshlet_bounds(meshlet: &mut Meshlet, vertices: &[[f32; 3]], vertex_indices: &[u32]) {
    if vertex_indices.is_empty() {
        return;
    }

    // Compute center (centroid)
    let mut center = [0.0f32; 3];
    for &idx in vertex_indices {
        let v = vertices[idx as usize];
        center[0] += v[0];
        center[1] += v[1];
        center[2] += v[2];
    }
    let n = vertex_indices.len() as f32;
    center[0] /= n;
    center[1] /= n;
    center[2] /= n;

    // Compute radius (max distance from center)
    let mut radius = 0.0f32;
    for &idx in vertex_indices {
        let v = vertices[idx as usize];
        let dx = v[0] - center[0];
        let dy = v[1] - center[1];
        let dz = v[2] - center[2];
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();
        if dist > radius {
            radius = dist;
        }
    }

    meshlet.center = center;
    meshlet.radius = radius;

    // Simple normal cone: use average normal direction
    // In production, compute from actual triangle normals
    meshlet.cone_axis = [0.0, 0.0, 1.0];
    meshlet.cone_cutoff = -1.0; // Always visible (no backface culling)
}
