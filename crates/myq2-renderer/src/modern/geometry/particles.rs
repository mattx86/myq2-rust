//! Particle system geometry
//!
//! Manages particle rendering via instanced billboard quads.

use super::{VertexBuffer, VertexArray};

/// Per-particle instance data.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ParticleInstance {
    /// Particle origin in world space.
    pub origin: [f32; 3],
    /// Particle color (RGBA).
    pub color: [f32; 4],
    /// Particle size.
    pub size: f32,
}

impl ParticleInstance {
    /// Size in bytes.
    pub const SIZE: usize = std::mem::size_of::<Self>();
}

/// Manages particle rendering.
pub struct ParticleManager {
    /// Static unit quad VBO.
    quad_vbo: VertexBuffer,
    /// Dynamic instance data VBO.
    instance_vbo: VertexBuffer,
    /// VAO configuration.
    vao: VertexArray,
    /// Current particle count.
    count: usize,
    /// Maximum particles.
    capacity: usize,
    /// Staging buffer.
    staging: Vec<ParticleInstance>,
}

impl ParticleManager {
    /// Default maximum particles.
    pub const DEFAULT_CAPACITY: usize = 4096;

    /// Create a new particle manager.
    pub fn new() -> Self {
        Self::with_capacity(Self::DEFAULT_CAPACITY)
    }

    /// Create with specified capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        let mut manager = Self {
            quad_vbo: VertexBuffer::new(),
            instance_vbo: VertexBuffer::new(),
            vao: VertexArray::new(),
            count: 0,
            capacity,
            staging: Vec::with_capacity(capacity),
        };

        manager.setup_quad();
        manager.setup_vao();

        manager
    }

    /// Create the unit quad VBO.
    fn setup_quad(&mut self) {
        // Billboard quad offsets: 4 vertices forming a quad
        // Layout: x, y (offsets from center)
        let quad_vertices: [f32; 8] = [
            -1.0, -1.0,  // Bottom-left
            -1.0,  1.0,  // Top-left
             1.0,  1.0,  // Top-right
             1.0, -1.0,  // Bottom-right
        ];

        self.quad_vbo.upload(&quad_vertices, 0);
    }

    /// Configure the vertex layout descriptors.
    fn setup_vao(&mut self) {
        self.vao.bind();

        // Quad vertex attribute (location 0): vec2
        self.quad_vbo.bind();
        self.vao.set_attribute_float(0, 2, 8, 0);

        // Instance attributes
        self.instance_vbo.bind();
        self.vao.set_attribute_float(1, 3, ParticleInstance::SIZE as i32, 0);  // origin
        self.vao.set_attribute_float(2, 4, ParticleInstance::SIZE as i32, 12); // color
        self.vao.set_attribute_float(3, 1, ParticleInstance::SIZE as i32, 28); // size

        VertexArray::unbind();
        VertexBuffer::unbind();
    }

    /// Clear staging buffer for new frame.
    pub fn begin_frame(&mut self) {
        self.staging.clear();
        self.count = 0;
    }

    /// Add a particle to the staging buffer.
    pub fn add(&mut self, origin: [f32; 3], color: [f32; 4], size: f32) {
        if self.staging.len() < self.capacity {
            self.staging.push(ParticleInstance { origin, color, size });
        }
    }

    /// Upload staged particles to GPU.
    pub fn upload(&mut self) {
        if self.staging.is_empty() {
            self.count = 0;
            return;
        }

        self.instance_vbo.upload(&self.staging, 0);
        self.count = self.staging.len();
    }

    /// Bind for rendering.
    pub fn bind(&self) {
        self.vao.bind();
    }

    /// Draw all particles. In SDL3 GPU mode, this is a no-op —
    /// the render path issues draw commands via command buffers.
    pub fn draw(&self) {
        // Draw commands are issued by the render path via command buffers.
        // This method is kept for API compatibility during transition.
    }

    /// Get the quad VBO for render pass binding.
    pub fn quad_buffer(&self) -> &VertexBuffer {
        &self.quad_vbo
    }

    /// Get the instance VBO for render pass binding.
    pub fn instance_buffer(&self) -> &VertexBuffer {
        &self.instance_vbo
    }

    /// Get current particle count.
    pub fn count(&self) -> usize {
        self.count
    }
}

impl Default for ParticleManager {
    fn default() -> Self {
        Self::new()
    }
}
