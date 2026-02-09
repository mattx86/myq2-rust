//! BSP world geometry management
//!
//! Batches BSP surfaces into VBOs grouped by texture.
//! Uses parallel processing for batch construction.

use super::{VertexBuffer, IndexBuffer, VertexArray};
use rayon::prelude::*;

/// Vertex format for BSP world surfaces.
///
/// Matches the original glpoly_t VERTEXSIZE=7 layout.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct BspVertex {
    /// Vertex position (xyz).
    pub position: [f32; 3],
    /// Diffuse texture coordinates.
    pub tex_coord: [f32; 2],
    /// Lightmap texture coordinates.
    pub lm_coord: [f32; 2],
}

impl BspVertex {
    /// Size of vertex in bytes.
    pub const SIZE: usize = std::mem::size_of::<Self>();

    /// Create a new BSP vertex.
    pub fn new(position: [f32; 3], tex_coord: [f32; 2], lm_coord: [f32; 2]) -> Self {
        Self {
            position,
            tex_coord,
            lm_coord,
        }
    }
}

/// Per-surface draw information for PVS culling.
#[derive(Clone, Debug)]
pub struct SurfaceDrawInfo {
    /// First index in the index buffer.
    pub first_index: u32,
    /// Number of indices for this surface.
    pub index_count: u32,
    /// Diffuse texture ID.
    pub texture_id: u32,
    /// Lightmap texture ID (or layer in texture array).
    pub lightmap_id: u32,
    /// Surface flags (SURF_DRAWTURB, SURF_FLOWING, etc.).
    pub flags: u32,
}

/// A batch of surfaces sharing the same texture.
pub struct TextureBatch {
    /// Diffuse texture ID.
    pub texture_id: u32,
    /// First index in the global index buffer.
    pub first_index: u32,
    /// Number of indices in this batch.
    pub index_count: u32,
    /// Surfaces in this batch (for PVS visibility checks).
    pub surfaces: Vec<usize>,
}

/// Manages BSP world geometry.
pub struct BspGeometryManager {
    /// Vertex buffer containing all BSP vertices.
    vbo: VertexBuffer,
    /// Index buffer for all surfaces.
    ibo: IndexBuffer,
    /// VAO configuration.
    vao: VertexArray,
    /// Per-surface metadata.
    surfaces: Vec<SurfaceDrawInfo>,
    /// Batches grouped by texture.
    batches: Vec<TextureBatch>,
    /// Total vertex count.
    vertex_count: u32,
    /// Total index count.
    index_count: u32,
    /// Whether geometry has been built.
    initialized: bool,
}

impl BspGeometryManager {
    /// Create a new uninitialized BSP geometry manager.
    pub fn new() -> Self {
        Self {
            vbo: VertexBuffer::new(),
            ibo: IndexBuffer::new(),
            vao: VertexArray::new(),
            surfaces: Vec::new(),
            batches: Vec::new(),
            vertex_count: 0,
            index_count: 0,
            initialized: false,
        }
    }

    /// Build geometry from BSP surfaces.
    ///
    /// This should be called at level load time.
    pub fn build(&mut self, vertices: &[BspVertex], indices: &[u32], surfaces: Vec<SurfaceDrawInfo>) {
        // Upload vertex data
        self.vbo.upload(vertices, 0);
        self.vertex_count = vertices.len() as u32;

        // Upload index data
        self.ibo.upload_u32(indices, 0);
        self.index_count = indices.len() as u32;

        // Store surface info
        self.surfaces = surfaces;

        // Build texture batches
        self.build_batches();

        // Configure VAO
        self.setup_vao();

        self.initialized = true;
    }

    /// Group surfaces into batches by texture.
    ///
    /// Uses parallel processing to calculate index ranges for each batch.
    fn build_batches(&mut self) {
        use std::collections::HashMap;

        // Phase 1: Group surfaces by texture ID (sequential HashMap insertion)
        let mut batch_map: HashMap<u32, Vec<usize>> = HashMap::new();
        for (i, surface) in self.surfaces.iter().enumerate() {
            batch_map
                .entry(surface.texture_id)
                .or_insert_with(Vec::new)
                .push(i);
        }

        // Phase 2: Calculate batch index ranges in parallel
        // Collect into Vec first so we can process in parallel
        let batch_entries: Vec<_> = batch_map.into_iter().collect();

        // Reference to surfaces for parallel closure
        let surfaces = &self.surfaces;

        // Process each texture group in parallel
        let mut batches: Vec<TextureBatch> = batch_entries
            .into_par_iter()
            .map(|(texture_id, surface_indices)| {
                // Calculate first index and count for this batch
                let (first_index, last_index) = surface_indices
                    .iter()
                    .fold((u32::MAX, 0u32), |(first, last), &si| {
                        let surf = &surfaces[si];
                        (
                            first.min(surf.first_index),
                            last.max(surf.first_index + surf.index_count),
                        )
                    });

                TextureBatch {
                    texture_id,
                    first_index,
                    index_count: last_index.saturating_sub(first_index),
                    surfaces: surface_indices,
                }
            })
            .collect();

        // Sort batches by texture ID for consistent rendering order
        batches.sort_by_key(|b| b.texture_id);
        self.batches = batches;
    }

    /// Configure the VAO with vertex attributes.
    fn setup_vao(&mut self) {
        self.vao.bind();
        self.vbo.bind();
        self.ibo.bind();

        // Position: location 0, vec3
        self.vao.set_attribute_float(0, 3, BspVertex::SIZE as i32, 0);
        // Tex coord: location 1, vec2
        self.vao.set_attribute_float(1, 2, BspVertex::SIZE as i32, 12);
        // Lightmap coord: location 2, vec2
        self.vao.set_attribute_float(2, 2, BspVertex::SIZE as i32, 20);

        VertexArray::unbind();
        VertexBuffer::unbind();
        IndexBuffer::unbind();
    }

    /// Bind for rendering.
    pub fn bind(&self) {
        self.vao.bind();
    }

    /// Get the texture batches for rendering.
    pub fn batches(&self) -> &[TextureBatch] {
        &self.batches
    }

    /// Get surface information.
    pub fn surfaces(&self) -> &[SurfaceDrawInfo] {
        &self.surfaces
    }

    /// Check if geometry has been built.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get the vertex buffer for render pass binding.
    pub fn vertex_buffer(&self) -> &VertexBuffer {
        &self.vbo
    }

    /// Get the index buffer for render pass binding.
    pub fn index_buffer(&self) -> &IndexBuffer {
        &self.ibo
    }

    /// Get total index count.
    pub fn index_count(&self) -> u32 {
        self.index_count
    }

    /// Clear all geometry.
    pub fn clear(&mut self) {
        self.surfaces.clear();
        self.batches.clear();
        self.vertex_count = 0;
        self.index_count = 0;
        self.initialized = false;
    }
}

impl Default for BspGeometryManager {
    fn default() -> Self {
        Self::new()
    }
}
