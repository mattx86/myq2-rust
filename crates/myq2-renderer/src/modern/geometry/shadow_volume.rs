//! Stencil shadow volume generation (Doom 3 / Carmack's Reverse z-fail).
//!
//! Builds extruded shadow volumes from triangle meshes for a given light position.
//! The z-fail algorithm correctly handles the camera being inside the shadow volume.
//!
//! Algorithm:
//!   1. Classify each triangle as front-facing (lit) or back-facing toward the light.
//!   2. Find silhouette edges: edges shared by exactly one front-facing and one back-facing tri.
//!   3. For each silhouette edge, emit an extruded quad (the shadow volume side wall).
//!   4. Cap the volume: front cap = front-facing tris as-is, back cap = extruded copies.
//!
//! The extruded positions are: `v + normalize(v - light_pos) * extrude_dist`.

use std::collections::HashMap;

// ============================================================================
// Edge key
// ============================================================================

/// Canonical edge identifier — vertex indices sorted low→high so that the edge
/// is identified the same way regardless of which triangle walks it which direction.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct EdgeKey(u32, u32);

impl EdgeKey {
    fn new(a: u32, b: u32) -> Self {
        if a < b { Self(a, b) } else { Self(b, a) }
    }
}

// ============================================================================
// Shadow volume output
// ============================================================================

/// CPU-side shadow volume for a single mesh rendered from a given light.
#[derive(Default, Clone)]
pub struct ShadowVolume {
    /// Extruded position-only vertices (world space).
    pub vertices: Vec<[f32; 3]>,
    /// Index buffer (u32 triangles).
    pub indices: Vec<u32>,
}

impl ShadowVolume {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }
}

// ============================================================================
// Shadow volume cache
// ============================================================================

/// Cache of pre-built shadow volumes for static BSP geometry.
///
/// Key: `(surface_group_id, light_idx)`.
/// Values are invalidated per-frame when lights move (dynamic lights always
/// bypass the cache).
pub struct ShadowVolumeCache {
    cache: HashMap<(u32, u32), ShadowVolume>,
    /// Generation counter. When incremented, the whole cache is cleared.
    generation: u32,
}

impl ShadowVolumeCache {
    pub fn new() -> Self {
        Self { cache: HashMap::new(), generation: 0 }
    }

    /// Invalidate the entire cache (call when any static light moves or map changes).
    pub fn invalidate(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.cache.clear();
    }

    /// Get or build the shadow volume for `(group_id, light_idx)`.
    pub fn get_or_build(
        &mut self,
        group_id: u32,
        light_idx: u32,
        positions: &[[f32; 3]],
        indices: &[u32],
        light_pos: [f32; 3],
        extrude_dist: f32,
    ) -> &ShadowVolume {
        self.cache
            .entry((group_id, light_idx))
            .or_insert_with(|| build_shadow_volume(positions, indices, light_pos, extrude_dist))
    }
}

impl Default for ShadowVolumeCache {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Shadow volume generation
// ============================================================================

/// Build a shadow volume for the given triangle mesh and light position.
///
/// `positions` — world-space vertex positions.
/// `indices` — triangle list indices (must be a multiple of 3).
/// `light_pos` — world-space light origin.
/// `extrude_dist` — distance to push shadow volume caps away from light
///                  (should be the light's attenuation radius).
pub fn build_shadow_volume(
    positions: &[[f32; 3]],
    indices: &[u32],
    light_pos: [f32; 3],
    extrude_dist: f32,
) -> ShadowVolume {
    if indices.len() < 3 || positions.is_empty() {
        return ShadowVolume::default();
    }

    let tri_count = indices.len() / 3;
    // Only include triangles whose nearest vertex is within the light radius.
    // This prevents distant geometry from creating shadow volumes that punch
    // through walls into adjacent rooms.
    let radius_sq = extrude_dist * extrude_dist;

    // -------------------------------------------------------------------------
    // Step 1: Classify each triangle as front-facing toward the light.
    // front_facing[i] = true  ↔  light is on the same side as the face normal.
    // Triangles completely outside the light radius are marked false (skipped).
    // -------------------------------------------------------------------------
    let mut front_facing: Vec<bool> = Vec::with_capacity(tri_count);
    for tri in 0..tri_count {
        let i0 = indices[tri * 3] as usize;
        let i1 = indices[tri * 3 + 1] as usize;
        let i2 = indices[tri * 3 + 2] as usize;
        if i0 >= positions.len() || i1 >= positions.len() || i2 >= positions.len() {
            front_facing.push(false);
            continue;
        }
        let p0 = positions[i0];
        let p1 = positions[i1];
        let p2 = positions[i2];

        // Skip triangles whose centroid is outside the light radius — these
        // can't cast a shadow visible within the light volume anyway.
        let centroid = [
            (p0[0] + p1[0] + p2[0]) / 3.0,
            (p0[1] + p1[1] + p2[1]) / 3.0,
            (p0[2] + p1[2] + p2[2]) / 3.0,
        ];
        let dc = sub3(centroid, light_pos);
        if dc[0]*dc[0] + dc[1]*dc[1] + dc[2]*dc[2] > radius_sq * 4.0 {
            // Triangle centroid more than 2× radius away — skip
            front_facing.push(false);
            continue;
        }

        let e1 = sub3(p1, p0);
        let e2 = sub3(p2, p0);
        let normal = cross3(e1, e2);

        // Vector from triangle to light
        let to_light = sub3(light_pos, p0);
        let is_front = dot3(normal, to_light) > 0.0;
        front_facing.push(is_front);
    }

    // -------------------------------------------------------------------------
    // Step 2: Build edge adjacency map.
    // Each entry stores the indices of (up to) two triangles sharing the edge,
    // plus the winding direction for each.
    // -------------------------------------------------------------------------

    // edge_map: EdgeKey → (face_a, dir_a_is_ab, face_b_opt, dir_b_is_ab)
    // dir_is_ab = true means the edge was stored as (a→b) in the face's winding.
    struct EdgeEntry {
        face_a: usize,
        dir_a_is_ab: bool,  // true: face_a winding is (low→high)
        face_b: Option<usize>,
    }

    let mut edge_map: HashMap<EdgeKey, EdgeEntry> = HashMap::with_capacity(tri_count * 3);

    for tri in 0..tri_count {
        let v = [
            indices[tri * 3] as u32,
            indices[tri * 3 + 1] as u32,
            indices[tri * 3 + 2] as u32,
        ];
        for e in 0..3 {
            let va = v[e];
            let vb = v[(e + 1) % 3];
            let key = EdgeKey::new(va, vb);
            let dir_is_ab = va < vb;
            edge_map.entry(key)
                .and_modify(|entry| {
                    if entry.face_b.is_none() {
                        entry.face_b = Some(tri);
                    }
                })
                .or_insert(EdgeEntry { face_a: tri, dir_a_is_ab: dir_is_ab, face_b: None });
        }
    }

    // -------------------------------------------------------------------------
    // Step 3: Collect silhouette edges: shared by exactly one front + one back face.
    // -------------------------------------------------------------------------

    // We'll build the output vertex list and index list.
    // Strategy: output the original positions first, then the extruded copies.
    // output_pos[i] = positions[i] (original)
    // output_pos[n + i] = extruded(positions[i])  (n = positions.len())
    let n = positions.len();

    let mut out_verts: Vec<[f32; 3]> = Vec::with_capacity(n * 2);
    out_verts.extend_from_slice(positions);
    // Fill extruded positions
    for &p in positions {
        let dir = sub3(p, light_pos);
        let len = len3(dir);
        let extruded = if len > 0.001 {
            let norm = [dir[0] / len, dir[1] / len, dir[2] / len];
            add3(p, scale3(norm, extrude_dist))
        } else {
            // Degenerate: vertex is at the light origin; push in arbitrary direction
            add3(p, [0.0, 0.0, extrude_dist])
        };
        out_verts.push(extruded);
    }

    let mut out_idx: Vec<u32> = Vec::new();

    // -------------------------------------------------------------------------
    // Silhouette quads (side walls)
    // -------------------------------------------------------------------------
    for (key, entry) in &edge_map {
        let face_b = match entry.face_b {
            Some(b) => b,
            None => continue, // boundary edge — skip (no silhouette)
        };
        let face_a_front = front_facing[entry.face_a];
        let face_b_front = front_facing[face_b];

        if face_a_front == face_b_front {
            continue; // both front or both back — not a silhouette
        }

        // Silhouette edge — the front-facing side determines the winding of the quad.
        // The quad must face outward from the shadow volume (away from the light on
        // the lit side).

        // Recover the original vertex indices in CCW order on the front-facing face.
        let (v_lo, v_hi) = (key.0 as usize, key.1 as usize);
        let (v0, v1) = if face_a_front {
            // face_a is front-facing; use its winding direction
            if entry.dir_a_is_ab {
                (v_lo, v_hi) // key is (lo, hi) = original winding a→b in face_a
            } else {
                (v_hi, v_lo) // original winding was b→a in face_a
            }
        } else {
            // face_b is front-facing; its winding is opposite to face_a on this edge
            if entry.dir_a_is_ab {
                (v_hi, v_lo)
            } else {
                (v_lo, v_hi)
            }
        };

        // Quad vertices (two triangles):
        //   v0 → v1 → v1_ext → v0_ext   (looking from outside the shadow volume)
        let v0u = v0 as u32;
        let v1u = v1 as u32;
        let v0e = (n + v0) as u32;  // extruded v0
        let v1e = (n + v1) as u32;  // extruded v1

        // Triangle 1: v0, v1, v1_ext
        out_idx.push(v0u);
        out_idx.push(v1u);
        out_idx.push(v1e);
        // Triangle 2: v0, v1_ext, v0_ext
        out_idx.push(v0u);
        out_idx.push(v1e);
        out_idx.push(v0e);
    }

    // -------------------------------------------------------------------------
    // Step 4: Caps
    // Front cap: front-facing triangles as-is (original winding, facing the light).
    // Back cap:  front-facing triangles extruded + winding reversed (face away).
    // -------------------------------------------------------------------------
    for tri in 0..tri_count {
        if !front_facing[tri] {
            continue;
        }
        let i0 = indices[tri * 3] as u32;
        let i1 = indices[tri * 3 + 1] as u32;
        let i2 = indices[tri * 3 + 2] as u32;

        // Front cap (original positions, normal faces toward light)
        out_idx.push(i0);
        out_idx.push(i1);
        out_idx.push(i2);

        // Back cap (extruded positions, reversed winding — normal faces away from light)
        out_idx.push((n as u32) + i0);
        out_idx.push((n as u32) + i2);
        out_idx.push((n as u32) + i1);
    }

    ShadowVolume {
        vertices: out_verts,
        indices: out_idx,
    }
}

// ============================================================================
// Math helpers
// ============================================================================

#[inline]
fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[inline]
fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

#[inline]
fn scale3(a: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

#[inline]
fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[inline]
fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
fn len3(a: [f32; 3]) -> f32 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}
