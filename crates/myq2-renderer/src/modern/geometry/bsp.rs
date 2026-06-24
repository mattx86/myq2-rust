//! BSP world geometry management
//!
//! Batches BSP surfaces into VBOs grouped by texture.

use std::collections::HashMap;
use super::{VertexBuffer, IndexBuffer, VertexArray};

/// Vertex format for BSP world surfaces.
///
/// Includes normal and tangent vectors for Doom 3 style per-pixel normal-mapped lighting.
/// Stride: 60 bytes.
///
/// Layout:
///   position   [f32; 3]  offset  0  (12 bytes)
///   tex_coord  [f32; 2]  offset 12  ( 8 bytes)
///   lm_coord   [f32; 2]  offset 20  ( 8 bytes)
///   lm_layer   f32       offset 28  ( 4 bytes)
///   normal     [f32; 3]  offset 32  (12 bytes)
///   tangent    [f32; 4]  offset 44  (16 bytes) — w = handedness (+1/-1)
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct BspVertex {
    /// Vertex position (xyz).
    pub position: [f32; 3],
    /// Diffuse texture coordinates.
    pub tex_coord: [f32; 2],
    /// Lightmap texture coordinates (normalized [0,1] within the lightmap block).
    pub lm_coord: [f32; 2],
    /// Lightmap texture array layer index (-1.0 = no lightmap / full bright).
    pub lm_layer: f32,
    /// Surface normal in world space (for per-pixel normal mapping).
    pub normal: [f32; 3],
    /// Surface tangent in world space; w = handedness (+1/-1) for bitangent reconstruction.
    pub tangent: [f32; 4],
}

impl BspVertex {
    /// Size of vertex in bytes.
    pub const SIZE: usize = std::mem::size_of::<Self>();

    /// Create a new BSP vertex with no lightmap (full bright), no normal/tangent.
    pub fn new(position: [f32; 3], tex_coord: [f32; 2], lm_coord: [f32; 2]) -> Self {
        Self {
            position,
            tex_coord,
            lm_coord,
            lm_layer: -1.0,
            normal: [0.0, 0.0, 1.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
        }
    }

    /// Create a new BSP vertex with lightmap layer index for per-pixel sampling.
    pub fn with_lightmap(position: [f32; 3], tex_coord: [f32; 2], lm_coord: [f32; 2], layer: f32) -> Self {
        Self {
            position,
            tex_coord,
            lm_coord,
            lm_layer: layer,
            normal: [0.0, 0.0, 1.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
        }
    }

    /// Create a vertex with normal and tangent for Doom 3 style lighting.
    pub fn with_tangent_frame(
        position: [f32; 3],
        tex_coord: [f32; 2],
        lm_coord: [f32; 2],
        layer: f32,
        normal: [f32; 3],
        tangent: [f32; 4],
    ) -> Self {
        Self { position, tex_coord, lm_coord, lm_layer: layer, normal, tangent }
    }
}

/// Water/lava/slime turbulent surface flag.
pub const SURF_DRAWTURB: u32 = 0x10;
/// Translucent surface flag (33% opacity).
pub const SURF_TRANS33: u32 = 0x20;
/// Translucent surface flag (66% opacity).
pub const SURF_TRANS66: u32 = 0x40;
/// Flowing water/lava texture scroll flag.
pub const SURF_FLOWING: u32 = 0x80;

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
    /// BSP leaf cluster this surface belongs to (-1 = unclassified).
    pub cluster: i32,
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
    /// CPU-side vertex positions (xyz only) for shadow volume generation.
    pub cpu_positions: Vec<[f32; 3]>,
    /// CPU-side sorted index buffer for shadow volume generation.
    pub cpu_indices: Vec<u32>,
    /// Per-cluster draw list: cluster → Vec<(first_index, index_count, texture_id)>.
    /// Used by the D3 lit pass to restrict illumination to the light's own BSP cluster.
    cluster_draws: HashMap<i32, Vec<(u32, u32, u32)>>,
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
            cpu_positions: Vec::new(),
            cpu_indices: Vec::new(),
            cluster_draws: HashMap::new(),
        }
    }

    /// Build geometry from BSP surfaces.
    ///
    /// This should be called at level load time. Sorts indices by texture so
    /// that same-texture surfaces are contiguous, enabling correct batching.
    pub fn build(&mut self, vertices: &[BspVertex], indices: &[u32], surfaces: Vec<SurfaceDrawInfo>) {
        // Upload vertex data
        self.vbo.upload(vertices, 0);
        self.vertex_count = vertices.len() as u32;

        // Sort surfaces by texture_id (opaque first, then alpha), then rebuild
        // index buffer in that order so same-texture surfaces are contiguous.
        let mut sorted_order: Vec<usize> = (0..surfaces.len()).collect();
        sorted_order.sort_by_key(|&i| {
            let is_alpha = surfaces[i].flags & (SURF_TRANS33 | SURF_TRANS66) != 0;
            (is_alpha, surfaces[i].texture_id)
        });

        let mut sorted_indices: Vec<u32> = Vec::with_capacity(indices.len());
        let mut sorted_surfaces: Vec<SurfaceDrawInfo> = Vec::with_capacity(surfaces.len());

        for &orig_idx in &sorted_order {
            let surf = &surfaces[orig_idx];
            let new_first = sorted_indices.len() as u32;

            // Copy this surface's indices into the sorted buffer
            let start = surf.first_index as usize;
            let end = start + surf.index_count as usize;
            if end <= indices.len() {
                sorted_indices.extend_from_slice(&indices[start..end]);
            }

            sorted_surfaces.push(SurfaceDrawInfo {
                first_index: new_first,
                index_count: surf.index_count,
                texture_id: surf.texture_id,
                lightmap_id: surf.lightmap_id,
                flags: surf.flags,
                cluster: surf.cluster,
            });
        }

        // Upload sorted index data
        self.ibo.upload_u32(&sorted_indices, 0);
        self.index_count = sorted_indices.len() as u32;

        // Store CPU-side positions for shadow volume generation (positions only, not full BspVertex)
        self.cpu_positions = vertices.iter().map(|v| v.position).collect();
        self.cpu_indices = sorted_indices.clone();

        // Store sorted surface info
        self.surfaces = sorted_surfaces;

        // Build per-cluster draw list for D3 lit pass cluster culling.
        // Maps cluster → opaque (non-alpha, non-turb) surface draw tuples.
        let mut cluster_draws: HashMap<i32, Vec<(u32, u32, u32)>> = HashMap::new();
        for surf in &self.surfaces {
            if surf.flags & (SURF_TRANS33 | SURF_TRANS66 | SURF_DRAWTURB) != 0 {
                continue;
            }
            if surf.cluster < 0 {
                continue;
            }
            cluster_draws
                .entry(surf.cluster)
                .or_default()
                .push((surf.first_index, surf.index_count, surf.texture_id));
        }
        self.cluster_draws = cluster_draws;

        // Build texture batches (now contiguous by texture)
        self.build_batches();

        // Configure VAO
        self.setup_vao();

        self.initialized = true;

        // Diagnostic: log batch statistics
        let total_batch_indices: u32 = self.batches.iter().map(|b| b.index_count).sum();
        let alpha_count = self.surfaces.iter()
            .filter(|s| s.flags & (SURF_TRANS33 | SURF_TRANS66) != 0)
            .count();
        eprintln!("BSP build: {} verts, {} indices, {} surfaces ({} opaque, {} alpha), {} batches, {} batch indices",
            self.vertex_count, self.index_count, self.surfaces.len(),
            self.surfaces.len() - alpha_count, alpha_count,
            self.batches.len(), total_batch_indices);
    }

    /// Group surfaces into batches by texture.
    ///
    /// Assumes surfaces are sorted by texture_id so same-texture surfaces
    /// have contiguous index ranges in the index buffer.
    fn build_batches(&mut self) {
        let mut batches: Vec<TextureBatch> = Vec::new();

        for (i, surface) in self.surfaces.iter().enumerate() {
            // Skip translucent surfaces (drawn separately with alpha blending)
            if surface.flags & (SURF_TRANS33 | SURF_TRANS66) != 0 {
                continue;
            }

            // Skip turb surfaces (drawn separately with water shader for animation)
            if surface.flags & SURF_DRAWTURB != 0 {
                continue;
            }

            // Extend existing batch if same texture, or start a new one
            if let Some(last) = batches.last_mut() {
                if last.texture_id == surface.texture_id {
                    // Extend: surfaces are contiguous after sorting
                    last.index_count += surface.index_count;
                    last.surfaces.push(i);
                    continue;
                }
            }

            // New batch
            batches.push(TextureBatch {
                texture_id: surface.texture_id,
                first_index: surface.first_index,
                index_count: surface.index_count,
                surfaces: vec![i],
            });
        }

        self.batches = batches;
    }

    /// Configure the VAO with vertex attributes (including normal and tangent).
    fn setup_vao(&mut self) {
        self.vao.bind();
        self.vbo.bind();
        self.ibo.bind();

        // Position: location 0, vec3, offset 0
        self.vao.set_attribute_float(0, 3, BspVertex::SIZE as i32, 0);
        // Tex coord: location 1, vec2, offset 12
        self.vao.set_attribute_float(1, 2, BspVertex::SIZE as i32, 12);
        // Lightmap coord: location 2, vec2, offset 20
        self.vao.set_attribute_float(2, 2, BspVertex::SIZE as i32, 20);
        // Lightmap layer: location 3, float, offset 28
        self.vao.set_attribute_float(3, 1, BspVertex::SIZE as i32, 28);
        // Normal: location 4, vec3, offset 32
        self.vao.set_attribute_float(4, 3, BspVertex::SIZE as i32, 32);
        // Tangent: location 5, vec4, offset 44
        self.vao.set_attribute_float(5, 4, BspVertex::SIZE as i32, 44);

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

    /// Get surface draw info for a range of surfaces (used by brush models).
    ///
    /// Returns a slice of SurfaceDrawInfo for surfaces in [first..first+count].
    pub fn draw_info_for_range(&self, first: usize, count: usize) -> &[SurfaceDrawInfo] {
        let end = (first + count).min(self.surfaces.len());
        if first >= self.surfaces.len() {
            return &[];
        }
        &self.surfaces[first..end]
    }

    /// Get all translucent surfaces (SURF_TRANS33 or SURF_TRANS66).
    pub fn alpha_surfaces(&self) -> Vec<&SurfaceDrawInfo> {
        self.surfaces.iter()
            .filter(|s| s.flags & (SURF_TRANS33 | SURF_TRANS66) != 0)
            .collect()
    }

    /// Get opaque draw tuples for a specific BSP cluster (for D3 lit pass culling).
    ///
    /// Returns `(first_index, index_count, texture_id)` for every opaque surface
    /// whose centroid is in `cluster`.  Returns an empty slice if the cluster is
    /// unknown or -1.
    pub fn surfaces_for_cluster(&self, cluster: i32) -> &[(u32, u32, u32)] {
        if cluster < 0 {
            return &[];
        }
        self.cluster_draws.get(&cluster).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get all turbulent surfaces (water/lava/slime with SURF_DRAWTURB).
    pub fn turb_surfaces(&self) -> Vec<&SurfaceDrawInfo> {
        self.surfaces.iter()
            .filter(|s| s.flags & SURF_DRAWTURB != 0)
            .collect()
    }

    /// Clear all geometry.
    pub fn clear(&mut self) {
        self.surfaces.clear();
        self.batches.clear();
        self.cluster_draws.clear();
        self.vertex_count = 0;
        self.index_count = 0;
        self.initialized = false;
        self.cpu_positions.clear();
        self.cpu_indices.clear();
    }
}

impl Default for BspGeometryManager {
    fn default() -> Self {
        Self::new()
    }
}
