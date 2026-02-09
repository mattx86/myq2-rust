//! 2D drawing geometry
//!
//! Batched rendering for console, menus, and HUD elements.

use super::{VertexBuffer, VertexArray};

/// Vertex format for 2D drawing.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Draw2DVertex {
    /// Screen position (x, y).
    pub position: [f32; 2],
    /// Texture coordinates.
    pub tex_coord: [f32; 2],
    /// Vertex color (RGBA).
    pub color: [f32; 4],
}

impl Draw2DVertex {
    /// Size in bytes.
    pub const SIZE: usize = std::mem::size_of::<Self>();

    /// Create a new 2D vertex.
    pub fn new(position: [f32; 2], tex_coord: [f32; 2], color: [f32; 4]) -> Self {
        Self {
            position,
            tex_coord,
            color,
        }
    }
}

/// A batch of 2D draw calls with the same texture.
#[derive(Clone)]
pub struct Draw2DBatch {
    /// Texture ID for this batch.
    pub texture: u32,
    /// First vertex in the buffer.
    pub first_vertex: u32,
    /// Number of vertices.
    pub vertex_count: u32,
    /// Blend mode.
    pub blend_mode: BlendMode,
}

/// Blend modes for 2D drawing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendMode {
    /// Standard alpha blending.
    Alpha,
    /// Additive blending.
    Additive,
    /// No blending.
    None,
}

impl Default for BlendMode {
    fn default() -> Self {
        BlendMode::Alpha
    }
}

/// Manages batched 2D drawing.
pub struct Draw2DManager {
    /// Dynamic VBO for batched quads.
    vbo: VertexBuffer,
    /// VAO configuration.
    vao: VertexArray,
    /// Staging buffer for vertices.
    vertices: Vec<Draw2DVertex>,
    /// Batches to draw.
    batches: Vec<Draw2DBatch>,
    /// Current texture.
    current_texture: u32,
    /// Current blend mode.
    current_blend: BlendMode,
    /// Character texture ID.
    char_texture: u32,
}

impl Draw2DManager {
    /// Create a new 2D draw manager.
    pub fn new() -> Self {
        let mut manager = Self {
            vbo: VertexBuffer::new(),
            vao: VertexArray::new(),
            vertices: Vec::with_capacity(4096),
            batches: Vec::with_capacity(64),
            current_texture: 0,
            current_blend: BlendMode::Alpha,
            char_texture: 0,
        };

        manager.setup_vao();
        manager
    }

    /// Configure the VAO.
    fn setup_vao(&mut self) {
        self.vao.bind();
        self.vbo.bind();

        // Position: location 0, vec2
        self.vao.set_attribute_float(0, 2, Draw2DVertex::SIZE as i32, 0);
        // Tex coord: location 1, vec2
        self.vao.set_attribute_float(1, 2, Draw2DVertex::SIZE as i32, 8);
        // Color: location 2, vec4
        self.vao.set_attribute_float(2, 4, Draw2DVertex::SIZE as i32, 16);

        VertexArray::unbind();
        VertexBuffer::unbind();
    }

    /// Set the character texture ID.
    pub fn set_char_texture(&mut self, texture: u32) {
        self.char_texture = texture;
    }

    /// Clear for new frame.
    pub fn begin_frame(&mut self) {
        self.vertices.clear();
        self.batches.clear();
        self.current_texture = 0;
        self.current_blend = BlendMode::Alpha;
    }

    /// Flush current batch if texture changed.
    fn flush_if_changed(&mut self, texture: u32, blend: BlendMode) {
        if texture != self.current_texture || blend != self.current_blend {
            if !self.vertices.is_empty() && self.current_texture != 0 {
                let start = self.batches.last().map(|b| b.first_vertex + b.vertex_count).unwrap_or(0);
                let count = self.vertices.len() as u32 - start;
                if count > 0 {
                    self.batches.push(Draw2DBatch {
                        texture: self.current_texture,
                        first_vertex: start,
                        vertex_count: count,
                        blend_mode: self.current_blend,
                    });
                }
            }
            self.current_texture = texture;
            self.current_blend = blend;
        }
    }

    /// Add a quad to the batch.
    pub fn push_quad(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        s1: f32,
        t1: f32,
        s2: f32,
        t2: f32,
        color: [f32; 4],
        texture: u32,
        blend: BlendMode,
    ) {
        self.flush_if_changed(texture, blend);

        // Two triangles forming a quad
        // Triangle 1: bottom-left, top-left, top-right
        self.vertices.push(Draw2DVertex::new([x, y + h], [s1, t2], color));
        self.vertices.push(Draw2DVertex::new([x, y], [s1, t1], color));
        self.vertices.push(Draw2DVertex::new([x + w, y], [s2, t1], color));

        // Triangle 2: bottom-left, top-right, bottom-right
        self.vertices.push(Draw2DVertex::new([x, y + h], [s1, t2], color));
        self.vertices.push(Draw2DVertex::new([x + w, y], [s2, t1], color));
        self.vertices.push(Draw2DVertex::new([x + w, y + h], [s2, t2], color));
    }

    /// Draw a character from the console font.
    pub fn draw_char(&mut self, x: i32, y: i32, num: u8) {
        let row = (num >> 4) as f32;
        let col = (num & 15) as f32;
        let size = 0.0625;  // 1/16

        let frow = row * size;
        let fcol = col * size;

        self.push_quad(
            x as f32,
            y as f32,
            8.0,
            8.0,
            fcol,
            frow,
            fcol + size,
            frow + size,
            [1.0, 1.0, 1.0, 1.0],
            self.char_texture,
            BlendMode::Alpha,
        );
    }

    /// Draw a filled rectangle.
    pub fn draw_fill(&mut self, x: i32, y: i32, w: i32, h: i32, color: [f32; 4]) {
        // Use texture 0 for solid color (no texture)
        self.push_quad(
            x as f32,
            y as f32,
            w as f32,
            h as f32,
            0.0,
            0.0,
            1.0,
            1.0,
            color,
            0,
            BlendMode::Alpha,
        );
    }

    /// Upload and flush all batches.
    pub fn flush(&mut self) {
        if self.vertices.is_empty() {
            return;
        }

        // Finalize current batch
        let start = self.batches.last().map(|b| b.first_vertex + b.vertex_count).unwrap_or(0);
        let count = self.vertices.len() as u32 - start;
        if count > 0 && self.current_texture != 0 {
            self.batches.push(Draw2DBatch {
                texture: self.current_texture,
                first_vertex: start,
                vertex_count: count,
                blend_mode: self.current_blend,
            });
        }

        // Upload vertex data via SDL3 GPU transfer buffer
        self.vbo.upload(&self.vertices, 0);
    }

    /// Bind for rendering.
    pub fn bind(&self) {
        self.vao.bind();
    }

    /// Get the VBO for render pass binding.
    pub fn vertex_buffer(&self) -> &VertexBuffer {
        &self.vbo
    }

    /// Get batches for rendering.
    pub fn batches(&self) -> &[Draw2DBatch] {
        &self.batches
    }
}

impl Default for Draw2DManager {
    fn default() -> Self {
        Self::new()
    }
}
