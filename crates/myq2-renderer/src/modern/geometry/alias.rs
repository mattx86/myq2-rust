//! Alias model (MD2) geometry management
//!
//! Handles VBO storage for animated models with GPU-side frame interpolation.

use super::{VertexBuffer, IndexBuffer, VertexArray};
use std::collections::HashMap;

/// Vertex format for alias models.
///
/// Contains both current and previous frame data for GPU interpolation.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct AliasVertex {
    /// Current frame position.
    pub position: [f32; 3],
    /// Previous frame position (for lerping).
    pub old_position: [f32; 3],
    /// Texture coordinates.
    pub tex_coord: [f32; 2],
    /// Normal index into precomputed normal table (0-161).
    pub normal_index: u8,
    /// Padding for alignment.
    pub _pad: [u8; 3],
}

impl AliasVertex {
    /// Size of vertex in bytes.
    pub const SIZE: usize = std::mem::size_of::<Self>();
}

/// Buffers for a single alias model.
pub struct AliasModelBuffers {
    /// VBO containing all frame vertices.
    vbo: VertexBuffer,
    /// Index buffer from GL commands.
    ibo: IndexBuffer,
    /// VAO configuration.
    vao: VertexArray,
    /// Vertices per frame.
    vertices_per_frame: u32,
    /// Total number of frames.
    num_frames: u32,
    /// Total number of indices.
    num_indices: u32,
}

impl AliasModelBuffers {
    /// Create buffers for an alias model.
    ///
    /// # Arguments
    /// * `all_frame_vertices` - All vertices for all frames concatenated
    /// * `indices` - Index buffer data
    /// * `vertices_per_frame` - Number of vertices in each frame
    /// * `num_frames` - Total number of animation frames
    pub fn new(
        all_frame_vertices: &[[f32; 3]],
        tex_coords: &[[f32; 2]],
        normal_indices: &[u8],
        indices: &[u32],
        vertices_per_frame: u32,
        num_frames: u32,
    ) -> Self {
        let mut buffers = Self {
            vbo: VertexBuffer::new(),
            ibo: IndexBuffer::new(),
            vao: VertexArray::new(),
            vertices_per_frame,
            num_frames,
            num_indices: indices.len() as u32,
        };

        // Build vertex data - we'll update old_position per-frame
        // For static upload, we store only positions and tex coords
        // The lerping is done via uniforms pointing to frame offsets
        let mut vertices: Vec<AliasVertex> = Vec::with_capacity(all_frame_vertices.len());

        for (i, &pos) in all_frame_vertices.iter().enumerate() {
            let vertex_in_frame = i % vertices_per_frame as usize;
            vertices.push(AliasVertex {
                position: pos,
                old_position: pos,  // Will be overridden by shader uniforms
                tex_coord: tex_coords.get(vertex_in_frame).copied().unwrap_or([0.0, 0.0]),
                normal_index: normal_indices.get(vertex_in_frame).copied().unwrap_or(0),
                _pad: [0, 0, 0],
            });
        }

        buffers.vbo.upload(&vertices, 0);
        buffers.ibo.upload_u32(indices, 0);
        buffers.setup_vao();

        buffers
    }

    /// Configure the VAO.
    fn setup_vao(&mut self) {
        self.vao.bind();
        self.vbo.bind();
        self.ibo.bind();

        // Position: location 0, vec3
        self.vao.set_attribute_float(0, 3, AliasVertex::SIZE as i32, 0);
        // Old position: location 1, vec3
        self.vao.set_attribute_float(1, 3, AliasVertex::SIZE as i32, 12);
        // Tex coord: location 2, vec2
        self.vao.set_attribute_float(2, 2, AliasVertex::SIZE as i32, 24);
        // Normal index: location 3, int (type constant ignored in SDL3 GPU — format
        // is derived from `size` inside VertexArray::set_attribute_int)
        self.vao.set_attribute_int(3, 1, 0, AliasVertex::SIZE as i32, 32);

        VertexArray::unbind();
        VertexBuffer::unbind();
        IndexBuffer::unbind();
    }

    /// Bind for rendering.
    pub fn bind(&self) {
        self.vao.bind();
    }

    /// Get number of indices.
    pub fn index_count(&self) -> u32 {
        self.num_indices
    }

    /// Get vertices per frame.
    pub fn vertices_per_frame(&self) -> u32 {
        self.vertices_per_frame
    }

    /// Get total frame count.
    pub fn frame_count(&self) -> u32 {
        self.num_frames
    }

    /// Calculate byte offset for a frame in the VBO.
    pub fn frame_offset(&self, frame: u32) -> usize {
        (frame * self.vertices_per_frame) as usize * AliasVertex::SIZE
    }

    /// Get the vertex buffer for render pass binding.
    pub fn vertex_buffer(&self) -> &VertexBuffer {
        &self.vbo
    }

    /// Get the index buffer for render pass binding.
    pub fn index_buffer(&self) -> &IndexBuffer {
        &self.ibo
    }
}

/// Manages alias model buffers.
pub struct AliasModelManager {
    /// Model buffers keyed by model pointer (as usize for simplicity).
    models: HashMap<usize, AliasModelBuffers>,
}

impl AliasModelManager {
    /// Create a new alias model manager.
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
        }
    }

    /// Register a model's buffers.
    pub fn register(&mut self, model_id: usize, buffers: AliasModelBuffers) {
        self.models.insert(model_id, buffers);
    }

    /// Get buffers for a model.
    pub fn get(&self, model_id: usize) -> Option<&AliasModelBuffers> {
        self.models.get(&model_id)
    }

    /// Remove a model's buffers.
    pub fn remove(&mut self, model_id: usize) {
        self.models.remove(&model_id);
    }

    /// Clear all model buffers.
    pub fn clear(&mut self) {
        self.models.clear();
    }
}

impl Default for AliasModelManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Instanced Rendering Support
// ============================================================================

/// Per-instance data for instanced alias model rendering.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct AliasInstance {
    /// Model matrix (column-major, 4x4).
    pub model_matrix: [[f32; 4]; 4],
    /// Shade light color.
    pub shade_light: [f32; 3],
    /// Alpha value.
    pub alpha: f32,
    /// Current frame index.
    pub frame: u32,
    /// Old frame index for lerping.
    pub old_frame: u32,
    /// Lerp factor (0.0 = current, 1.0 = old).
    pub backlerp: f32,
    /// Flags (RF_* values).
    pub flags: u32,
}

impl AliasInstance {
    /// Size in bytes.
    pub const SIZE: usize = std::mem::size_of::<Self>();
}

/// Batches multiple instances of the same model for instanced rendering.
pub struct InstancedAliasBatch {
    /// Instance data VBO.
    instance_vbo: VertexBuffer,
    /// Instances for this batch.
    instances: Vec<AliasInstance>,
    /// Model ID this batch is for.
    model_id: usize,
    /// Maximum instances.
    capacity: usize,
}

impl InstancedAliasBatch {
    /// Default capacity.
    pub const DEFAULT_CAPACITY: usize = 256;

    /// Create a new instanced batch for a model.
    pub fn new(model_id: usize) -> Self {
        Self::with_capacity(model_id, Self::DEFAULT_CAPACITY)
    }

    /// Create with specific capacity.
    pub fn with_capacity(model_id: usize, capacity: usize) -> Self {
        Self {
            instance_vbo: VertexBuffer::new(),
            instances: Vec::with_capacity(capacity),
            model_id,
            capacity,
        }
    }

    /// Clear instances for new frame.
    pub fn begin_frame(&mut self) {
        self.instances.clear();
    }

    /// Add an instance.
    pub fn add(&mut self, instance: AliasInstance) {
        if self.instances.len() < self.capacity {
            self.instances.push(instance);
        }
    }

    /// Upload instance data to GPU via SDL3 GPU transfer buffer.
    pub fn upload(&mut self) {
        if self.instances.is_empty() {
            return;
        }
        self.instance_vbo.upload(&self.instances, 0);
    }

    /// Get the model ID.
    pub fn model_id(&self) -> usize {
        self.model_id
    }

    /// Get instance count.
    pub fn count(&self) -> usize {
        self.instances.len()
    }

    /// Get the instance VBO for render pass binding.
    pub fn instance_buffer(&self) -> &VertexBuffer {
        &self.instance_vbo
    }

    /// Draw all instances.
    ///
    /// In SDL3 GPU, draw commands are issued by the render path via command
    /// buffers and render passes. This method is kept for API compatibility
    /// during the transition.
    pub fn draw_instanced(&self, _index_count: u32) {
        // Draw commands are issued by the render path via command buffers.
        // This method is a no-op stub during the SDL3 GPU transition.
    }
}

/// Manages instanced batches by model.
pub struct InstancedAliasRenderer {
    /// Batches keyed by model ID.
    batches: HashMap<usize, InstancedAliasBatch>,
}

impl InstancedAliasRenderer {
    /// Create a new instanced renderer.
    pub fn new() -> Self {
        Self {
            batches: HashMap::new(),
        }
    }

    /// Clear all batches for new frame.
    pub fn begin_frame(&mut self) {
        for batch in self.batches.values_mut() {
            batch.begin_frame();
        }
    }

    /// Add an instance for a model.
    pub fn add_instance(&mut self, model_id: usize, instance: AliasInstance) {
        self.batches
            .entry(model_id)
            .or_insert_with(|| InstancedAliasBatch::new(model_id))
            .add(instance);
    }

    /// Upload all instance data.
    pub fn upload(&mut self) {
        for batch in self.batches.values_mut() {
            batch.upload();
        }
    }

    /// Get batch for a model.
    pub fn get_batch(&self, model_id: usize) -> Option<&InstancedAliasBatch> {
        self.batches.get(&model_id)
    }

    /// Iterate over all batches with instances.
    pub fn batches(&self) -> impl Iterator<Item = &InstancedAliasBatch> {
        self.batches.values().filter(|b| b.count() > 0)
    }
}

impl Default for InstancedAliasRenderer {
    fn default() -> Self {
        Self::new()
    }
}
