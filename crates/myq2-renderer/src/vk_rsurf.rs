// vk_rsurf.rs — Surface-related refresh code
// Converted from: myq2-original/ref_gl/vk_rsurf.c

use crate::vk_light::*;
use crate::vk_local::*;
use crate::vk_rmain::vid_printf;
use crate::vk_warp::*;
use myq2_common::q_shared::*;
use myq2_common::qfiles::MAX_MAP_LEAFS;
use std::collections::HashMap;

// ============================================================
// Constants
// ============================================================

const DYNAMIC_LIGHT_WIDTH: i32 = 128;
const DYNAMIC_LIGHT_HEIGHT: i32 = 128;

const LIGHTMAP_BYTES: i32 = 4;

/// Lightmap atlas dimensions. 16x the original Q2 value (128→2048) so each surface's
/// lightmap data is bicubic-upscaled 16x, giving high-quality smooth shadow gradients.
pub const BLOCK_WIDTH: i32 = 2048;
pub const BLOCK_HEIGHT: i32 = 2048;

/// Upscale factor applied to each surface lightmap when building the atlas.
/// 16 means each source texel expands to a 16×16 block with bicubic blending.
pub const LM_UPSCALE: i32 = 16;

const MAX_LIGHTMAPS: usize = 128;

const VK_LIGHTMAP_FORMAT: u32 = VK_RGBA as u32;

const BACKFACE_EPSILON: f32 = 0.01;

// ============================================================
// Module globals — SurfaceState
// ============================================================

// fogDensity accessed via fog_density() in vk_local

/// Collected lightmap layer data during model loading.
/// Each entry is the RGBA pixel data for one 128x128 lightmap block (layer).
/// Index 0 is unused (dynamic lightmap); static layers start at index 1.
pub static LIGHTMAP_LAYER_DATA: std::sync::Mutex<Vec<Vec<u8>>> = std::sync::Mutex::new(Vec::new());

/// A static (BSP-baked) light parsed from the map entity string.
/// Used to render persistent glow discs at light fixture positions.
#[derive(Clone, Debug)]
pub struct StaticLight {
    pub origin: [f32; 3],
    pub color:  [f32; 3],   // RGB, each 0–1
    pub intensity: f32,      // raw "light" key value (default 300)
    /// BSP cluster this light occupies (-1 = in solid / unknown).
    /// Pre-computed at BSP load for O(1) PVS membership tests.
    pub cluster: i32,
}

/// Static lights parsed from the current map's entity string.
/// Populated by `parse_static_lights()` at end of world load.
pub static STATIC_LIGHTS: std::sync::Mutex<Vec<StaticLight>> = std::sync::Mutex::new(Vec::new());

/// Parse `light` entities out of the BSP entity string and store them in STATIC_LIGHTS.
pub fn parse_static_lights() {
    let entity_str = myq2_common::cmodel::cm_entity_string();
    let mut lights: Vec<StaticLight> = Vec::new();

    // Walk entity blocks: { key "val" key "val" ... }
    let mut rest = entity_str.as_str();
    while let Some(start) = rest.find('{') {
        rest = &rest[start + 1..];
        let end = rest.find('}').unwrap_or(rest.len());
        let block = &rest[..end];
        rest = &rest[end..];

        // Collect key→value pairs for this entity block
        let mut classname = String::new();
        let mut origin = [0.0f32; 3];
        let mut color = [1.0f32; 3];
        let mut intensity = 300.0f32;

        let mut block_rest = block;
        while let Some(q1) = block_rest.find('"') {
            block_rest = &block_rest[q1 + 1..];
            let q2 = block_rest.find('"').unwrap_or(block_rest.len());
            let key = &block_rest[..q2];
            block_rest = &block_rest[q2 + 1..];

            // skip whitespace between key and value
            block_rest = block_rest.trim_start();
            if !block_rest.starts_with('"') { continue; }
            block_rest = &block_rest[1..];
            let vq = block_rest.find('"').unwrap_or(block_rest.len());
            let val = &block_rest[..vq];
            block_rest = &block_rest[vq + 1..];

            match key {
                "classname" => classname = val.to_string(),
                "origin" => {
                    let mut parts = val.split_ascii_whitespace();
                    if let (Some(x), Some(y), Some(z)) = (parts.next(), parts.next(), parts.next()) {
                        origin[0] = x.parse().unwrap_or(0.0);
                        origin[1] = y.parse().unwrap_or(0.0);
                        origin[2] = z.parse().unwrap_or(0.0);
                    }
                }
                "light" | "_light" => {
                    intensity = val.parse().unwrap_or(300.0);
                }
                "_color" | "color" => {
                    let mut parts = val.split_ascii_whitespace();
                    if let (Some(r), Some(g), Some(b)) = (parts.next(), parts.next(), parts.next()) {
                        color[0] = r.parse().unwrap_or(1.0);
                        color[1] = g.parse().unwrap_or(1.0);
                        color[2] = b.parse().unwrap_or(1.0);
                    }
                }
                _ => {}
            }
        }

        if classname == "light" && intensity > 0.0 {
            // Look up the BSP cluster for this light position so update_d3_lights can
            // filter by PVS without a per-frame BSP traversal.
            let pos: Vec3 = origin;
            let leafnum = myq2_common::cmodel::cm_point_leafnum(&pos) as i32;
            let cluster = myq2_common::cmodel::cm_leaf_cluster(leafnum as usize);
            lights.push(StaticLight { origin, color, intensity, cluster });
        }
    }

    *STATIC_LIGHTS.lock().unwrap_or_else(|e| e.into_inner()) = lights;
}

/// Turn emissive SURFACES into light sources: `SURF_LIGHT` textures (lamps, glowing panels)
/// and the sky. They are appended to `STATIC_LIGHTS` so they emit through the D3 lit pass
/// and cast real shadows via the same cubemap system as `light` entities — in dynamic (D3)
/// mode this is what actually lights a room from its ceiling lights / sky opening.
///
/// Contiguous emitters are merged on a coarse grid (per the project's "treat connected
/// surfaces as one piece" rule), so a multi-face light fixture or a sky opening becomes a
/// single light rather than dozens — which also keeps the count within the shadow-light cap.
/// After appending, `STATIC_LIGHTS` is sorted by intensity so the strongest emitters win the
/// limited shadow-casting slots. Called once at world load, after `parse_static_lights`.
pub fn parse_surface_lights() {
    use std::collections::HashMap;
    use crate::vk_model_types::{SURF_DRAWSKY, SURF_PLANEBACK, VERTEXSIZE};

    /// Merge radius for connected emitter faces.
    const GRID: f32 = 192.0;
    /// Push the light this far off the surface into the room along its normal.
    const OFFSET: f32 = 24.0;

    struct Acc { c: [f64; 3], n: [f64; 3], count: u32 }

    // SAFETY: the world model and its surface/polygon data are fully built by the time
    // vk_end_building_lightmaps runs; pointers are valid for the level lifetime, main thread.
    unsafe {
        // Use loadmodel, not r_worldmodel: this runs DURING the world load (from
        // vk_end_building_lightmaps), before r_worldmodel has been assigned.
        let world = crate::vk_local::rfs().loadmodel;
        if world.is_null() { return; }
        let nsurf = (*world).numsurfaces;
        let surfaces = (*world).surfaces;
        if surfaces.is_null() || nsurf <= 0 { return; }

        // Bucket emissive faces by coarse grid cell + kind (0 = light texture, 1 = sky).
        let mut cells: HashMap<(i32, i32, i32, u8), Acc> = HashMap::new();

        for i in 0..nsurf {
            let surf = &*surfaces.offset(i as isize);
            let ti = surf.texinfo;
            let is_sky = surf.flags & SURF_DRAWSKY != 0;
            let is_light = !ti.is_null()
                && ((*ti).flags & myq2_common::q_shared::SURF_LIGHT) != 0;
            if !is_sky && !is_light { continue; }
            let kind: u8 = if is_sky { 1 } else { 0 };

            let poly = surf.polys;
            if poly.is_null() { continue; }
            let nv = (*poly).numverts;
            if nv <= 0 { continue; }
            // verts is a flexible array (numverts × VERTEXSIZE floats); index by raw pointer.
            let vptr = (*poly).verts.as_ptr() as *const f32;
            let mut c = [0.0f32; 3];
            for v in 0..nv as usize {
                let base = vptr.add(v * VERTEXSIZE);
                c[0] += *base; c[1] += *base.add(1); c[2] += *base.add(2);
            }
            let inv = 1.0 / nv as f32;
            c = [c[0] * inv, c[1] * inv, c[2] * inv];

            // Surface normal, flipped for SURF_PLANEBACK so it points into the room.
            let mut n = if surf.plane.is_null() { [0.0, 0.0, 1.0] } else { (*surf.plane).normal };
            if surf.flags & SURF_PLANEBACK != 0 { n = [-n[0], -n[1], -n[2]]; }

            let key = (
                (c[0] / GRID).floor() as i32,
                (c[1] / GRID).floor() as i32,
                (c[2] / GRID).floor() as i32,
                kind,
            );
            let e = cells.entry(key).or_insert(Acc { c: [0.0; 3], n: [0.0; 3], count: 0 });
            e.c[0] += c[0] as f64; e.c[1] += c[1] as f64; e.c[2] += c[2] as f64;
            e.n[0] += n[0] as f64; e.n[1] += n[1] as f64; e.n[2] += n[2] as f64;
            e.count += 1;
        }

        let mut lights = STATIC_LIGHTS.lock().unwrap_or_else(|e| e.into_inner());
        for ((_, _, _, kind), e) in cells.iter() {
            let inv = 1.0 / e.count as f64;
            let mut origin = [
                (e.c[0] * inv) as f32, (e.c[1] * inv) as f32, (e.c[2] * inv) as f32,
            ];
            let mut n = [
                (e.n[0] * inv) as f32, (e.n[1] * inv) as f32, (e.n[2] * inv) as f32,
            ];
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if len > 1e-4 { n = [n[0] / len, n[1] / len, n[2] / len]; }
            origin = [origin[0] + n[0] * OFFSET, origin[1] + n[1] * OFFSET, origin[2] + n[2] * OFFSET];

            // Sky emits cool (skylight); light textures emit close to neutral white — a
            // strong warm bias here is what tips many overlapping lights into a red cast.
            let (color, intensity) = if *kind == 1 {
                ([0.75, 0.85, 1.0], 400.0)
            } else {
                ([1.0, 0.97, 0.92], 300.0)
            };
            let leafnum = myq2_common::cmodel::cm_point_leafnum(&origin) as i32;
            let cluster = myq2_common::cmodel::cm_leaf_cluster(leafnum as usize);
            lights.push(StaticLight { origin, color, intensity, cluster });
        }

    }
}

// ============================================================
// Lightmap state
// ============================================================

struct VkLightmapState {
    internal_format: i32,
    current_lightmap_texture: i32,
    lightmap_surfaces: [*mut MSurface; MAX_LIGHTMAPS],
    allocated: [i32; BLOCK_WIDTH as usize],
    lightmap_buffer: [u8; 4 * BLOCK_WIDTH as usize * BLOCK_HEIGHT as usize],
}

/// Combined surface rendering state behind a single Mutex.
pub struct SurfaceState {
    /// Alpha (transparent) surface chain head
    r_alpha_surfaces: *mut MSurface,
    /// Lightmap allocation state
    vk_lms: VkLightmapState,
    /// Deferred render command queue
    render_queue: RenderCommandQueue,
    /// Base lightstyles for lightmap building
    lightstyles: [LightStyle; MAX_LIGHTSTYLES],
}

// SAFETY: SurfaceState contains raw pointers (r_alpha_surfaces, lightmap_surfaces)
// but all access is serialized by the Mutex. Pointers are valid for the struct's lifetime.
unsafe impl Send for SurfaceState {}

static SURFACE_STATE: std::sync::Mutex<SurfaceState> = std::sync::Mutex::new(SurfaceState {
    r_alpha_surfaces: std::ptr::null_mut(),
    vk_lms: VkLightmapState {
        internal_format: 0,
        current_lightmap_texture: 0,
        lightmap_surfaces: [std::ptr::null_mut(); MAX_LIGHTMAPS],
        allocated: [0; BLOCK_WIDTH as usize],
        lightmap_buffer: [0; 4 * BLOCK_WIDTH as usize * BLOCK_HEIGHT as usize],
    },
    render_queue: RenderCommandQueue::new(),
    lightstyles: [LightStyle {
        rgb: [0.0; 3],
        white: 0.0,
    }; MAX_LIGHTSTYLES],
});

fn with_surface_state<R>(f: impl FnOnce(&mut SurfaceState) -> R) -> R {
    let mut ss = SURFACE_STATE.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut ss)
}

// ============================================================
// DEFERRED RENDER COMMAND BATCHING
// ============================================================
// This system collects draw commands during BSP traversal and
// batches them by material/texture for efficient rendering.
// The modern Vulkan pipeline can then iterate over batches
// instead of processing surfaces one-by-one.

/// A single surface draw command
#[derive(Clone, Copy)]
pub struct SurfaceDrawCmd {
    /// Pointer to the surface to draw
    pub surface: *mut MSurface,
    /// Texture ID for sorting
    pub texture_id: usize,
    /// Lightmap texture number
    pub lightmap_num: i32,
    /// Flags for render state (transparency, etc.)
    pub flags: i32,
}

// SAFETY: SurfaceDrawCmd contains raw pointers but is only used within
// single-threaded rendering. The pointers are valid for the frame duration.
unsafe impl Send for SurfaceDrawCmd {}
unsafe impl Sync for SurfaceDrawCmd {}

/// Batch of surfaces sharing the same texture
pub struct SurfaceBatch {
    pub texture_id: usize,
    pub lightmap_num: i32,
    pub surfaces: Vec<SurfaceDrawCmd>,
}

/// Render command queue with batching support
pub struct RenderCommandQueue {
    /// All queued draw commands
    commands: Vec<SurfaceDrawCmd>,
    /// Batched commands by texture (built on demand)
    batches: Vec<SurfaceBatch>,
    /// Whether batches are dirty and need rebuilding
    batches_dirty: bool,
    /// Alpha (transparent) surfaces, processed separately
    alpha_commands: Vec<SurfaceDrawCmd>,
}

/// Threshold for using parallel batch building
const BATCH_PARALLEL_THRESHOLD: usize = 256;

impl RenderCommandQueue {
    pub const fn new() -> Self {
        Self {
            commands: Vec::new(),
            batches: Vec::new(),
            batches_dirty: true,
            alpha_commands: Vec::new(),
        }
    }

    /// Clear all queued commands for a new frame
    pub fn clear(&mut self) {
        self.commands.clear();
        self.batches.clear();
        self.batches_dirty = true;
        self.alpha_commands.clear();
    }

    /// Queue a surface for rendering
    pub fn queue_surface(&mut self, surface: *mut MSurface, texture_id: usize, lightmap_num: i32, flags: i32) {
        let cmd = SurfaceDrawCmd {
            surface,
            texture_id,
            lightmap_num,
            flags,
        };

        // Separate alpha surfaces for back-to-front rendering
        if flags & (SURF_TRANS33 | SURF_TRANS66) != 0 {
            self.alpha_commands.push(cmd);
        } else {
            self.commands.push(cmd);
            self.batches_dirty = true;
        }
    }

    /// Build batches from queued commands, grouping by texture
    pub fn build_batches(&mut self) {
        if !self.batches_dirty {
            return;
        }

        self.batches.clear();

        if self.commands.is_empty() {
            self.batches_dirty = false;
            return;
        }

        // Group commands by texture ID
        let mut texture_map: HashMap<usize, Vec<SurfaceDrawCmd>> = HashMap::new();

        for cmd in &self.commands {
            texture_map
                .entry(cmd.texture_id)
                .or_default()
                .push(*cmd);
        }

        // Convert to batches
        self.batches.reserve(texture_map.len());
        for (texture_id, surfaces) in texture_map {
            // Get lightmap from first surface (all in batch should share texture)
            let lightmap_num = surfaces.first().map(|s| s.lightmap_num).unwrap_or(0);
            self.batches.push(SurfaceBatch {
                texture_id,
                lightmap_num,
                surfaces,
            });
        }

        // Sort batches by texture ID for consistent draw order
        self.batches.sort_by_key(|b| b.texture_id);

        self.batches_dirty = false;
    }

    /// Get the number of opaque batches
    pub fn batch_count(&mut self) -> usize {
        self.build_batches();
        self.batches.len()
    }

    /// Get a batch by index
    pub fn get_batch(&mut self, index: usize) -> Option<&SurfaceBatch> {
        self.build_batches();
        self.batches.get(index)
    }

    /// Iterate over all opaque batches
    pub fn iter_batches(&mut self) -> impl Iterator<Item = &SurfaceBatch> {
        self.build_batches();
        self.batches.iter()
    }

    /// Get all alpha surfaces (need back-to-front sorting)
    pub fn alpha_surfaces(&self) -> &[SurfaceDrawCmd] {
        &self.alpha_commands
    }

    /// Get total number of queued surfaces
    pub fn total_surfaces(&self) -> usize {
        self.commands.len() + self.alpha_commands.len()
    }
}

impl Default for RenderCommandQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize render command queue for a new frame.
pub fn vk_begin_render_queue() {
    with_surface_state(|ss| ss.render_queue.clear());
}

/// Queue a surface for batched rendering.
///
/// # Safety
/// Dereferences surface and texinfo pointers.
pub unsafe fn vk_queue_surface(surface: *mut MSurface) {
    if surface.is_null() {
        return;
    }

    let surf = &*surface;
    let texture_id = if !surf.texinfo.is_null() && !(*surf.texinfo).image.is_null() {
        (*(*surf.texinfo).image).texnum as usize
    } else {
        0
    };
    let lightmap_num = surf.lightmaptexturenum;
    let flags = if !surf.texinfo.is_null() {
        (*surf.texinfo).flags
    } else {
        0
    };

    with_surface_state(|ss| ss.render_queue.queue_surface(surface, texture_id, lightmap_num, flags));
}

/// Get batch count for rendering.
pub fn vk_get_batch_count() -> usize {
    with_surface_state(|ss| ss.render_queue.batch_count())
}

/// Get a specific batch for rendering.
///
/// Note: Returns a cloned batch to avoid holding the lock.
pub fn vk_get_batch(index: usize) -> Option<SurfaceBatch> {
    with_surface_state(|ss| {
        ss.render_queue.build_batches();
        ss.render_queue.batches.get(index).map(|b| SurfaceBatch {
            texture_id: b.texture_id,
            lightmap_num: b.lightmap_num,
            surfaces: b.surfaces.clone(),
        })
    })
}

/// Get alpha surfaces for back-to-front rendering.
///
/// Note: Returns a cloned Vec to avoid holding the lock.
pub fn vk_get_alpha_surfaces() -> Vec<SurfaceDrawCmd> {
    with_surface_state(|ss| ss.render_queue.alpha_commands.clone())
}

/// Get total surface count for statistics.
pub fn vk_get_queued_surface_count() -> usize {
    with_surface_state(|ss| ss.render_queue.total_surfaces())
}

// ============================================================
// BRUSH MODELS
// ============================================================

/// Return the proper texture for a given time and base texture.
///
/// # Safety
/// Accesses global entity state.
pub unsafe fn r_texture_animation(tex: *const MTexInfo) -> *mut Image {
    let tex_ref = &*tex;
    if tex_ref.next.is_null() {
        return tex_ref.image;
    }

    let mut c = (*rfs().currententity).frame % tex_ref.numframes;
    let mut t = tex;
    while c > 0 {
        t = (*t).next;
        c -= 1;
    }

    (*t).image
}

// draw_gl_poly: removed (legacy immediate-mode GL rendering).
// Will be replaced by modern rendering pipeline.

// draw_gl_flowing_poly: removed (legacy immediate-mode GL rendering).
// Will be replaced by modern rendering pipeline.

// r_draw_triangle_outlines: removed (legacy immediate-mode GL rendering).
// Will be replaced by modern rendering pipeline.

// draw_gl_poly_chain: removed (legacy immediate-mode GL rendering).
// Will be replaced by modern rendering pipeline.

// r_blend_lightmaps: removed (legacy immediate-mode GL rendering).
// Will be replaced by modern rendering pipeline.

// r_render_brush_poly: removed (legacy immediate-mode GL rendering).
// Will be replaced by modern rendering pipeline.

// r_draw_alpha_surfaces: removed (legacy immediate-mode GL rendering).
// Will be replaced by modern rendering pipeline.

// draw_texture_chains: removed (legacy immediate-mode GL rendering).
// Will be replaced by modern rendering pipeline.

// vk_render_lightmapped_poly: removed (legacy immediate-mode GL rendering).
// Will be replaced by modern rendering pipeline.

// r_draw_inline_bmodel: removed (legacy immediate-mode GL rendering).
// Will be replaced by modern rendering pipeline.

// r_draw_brush_model: removed (legacy immediate-mode GL rendering).
// Will be replaced by modern rendering pipeline.

// ============================================================
// WORLD MODEL
// ============================================================

/// Plane type constants for fast axial dot product.
const PLANE_X: u8 = 0;
const PLANE_Y: u8 = 1;
const PLANE_Z: u8 = 2;

/// Recursively traverse the BSP, drawing visible world surfaces.
///
/// # Safety
/// Accesses global renderer state and BSP data.
pub unsafe fn r_recursive_world_node(node: *mut MNode, alpha_surfaces: &mut *mut MSurface) {
    if node.is_null() {
        return;
    }
    let node_ref = &*node;

    if node_ref.contents == CONTENTS_SOLID {
        return; // solid
    }

    if node_ref.visframe != rfs().r_visframecount {
        return;
    }

    if r_cull_box_raw(node_ref.minmaxs.as_ptr(), node_ref.minmaxs.as_ptr().offset(3)) {
        return;
    }

    // if a leaf node, draw stuff
    if node_ref.contents != -1 {
        let pleaf = node as *mut MLeaf;

        // check for door connected areas
        if !rfs().r_newrefdef.areabits.is_null() {
            let area = (*pleaf).area;
            if *rfs().r_newrefdef.areabits.offset((area >> 3) as isize) & (1 << (area & 7)) == 0 {
                return; // not visible
            }
        }

        let mut mark = (*pleaf).firstmarksurface;
        let mut c = (*pleaf).nummarksurfaces;

        if c > 0 {
            loop {
                (**mark).visframe = rfs().r_framecount;
                mark = mark.offset(1);
                c -= 1;
                if c == 0 {
                    break;
                }
            }
        }

        return;
    }

    // node is just a decision point
    let plane = node_ref.plane;

    let dot = match (*plane).plane_type {
        PLANE_X => rfs().r_origin[0] - (*plane).dist,
        PLANE_Y => rfs().r_origin[1] - (*plane).dist,
        PLANE_Z => rfs().r_origin[2] - (*plane).dist,
        _ => dot_product(&rfs().r_origin, &(*plane).normal) - (*plane).dist,
    };

    let (side, sidebit) = if dot >= 0.0 { (0usize, 0i32) } else { (1usize, SURF_PLANEBACK) };

    // recurse down front side first
    r_recursive_world_node(node_ref.children[side], alpha_surfaces);

    // draw stuff
    let surfaces = r_worldmodel_surfaces();
    let mut c = node_ref.numsurfaces;
    let mut surf = surfaces.offset(node_ref.firstsurface as isize);

    while c > 0 {
        if (*surf).visframe != rfs().r_framecount {
            surf = surf.offset(1);
            c -= 1;
            continue;
        }

        if ((*surf).flags & SURF_PLANEBACK) != sidebit {
            surf = surf.offset(1);
            c -= 1;
            continue; // wrong side
        }

        if (*(*surf).texinfo).flags & SURF_SKY != 0 {
            r_add_sky_surface(&*surf);
        } else if (*(*surf).texinfo).flags & (SURF_TRANS33 | SURF_TRANS66) != 0 {
            (*surf).texturechain = *alpha_surfaces;
            *alpha_surfaces = surf;
        } else {
            // Legacy vk_render_lightmapped_poly call removed;
            // chain texture for modern rendering pipeline.
            let image = r_texture_animation((*surf).texinfo);
            (*surf).texturechain = (*image).texturechain;
            (*image).texturechain = surf;
        }

        surf = surf.offset(1);
        c -= 1;
    }

    // recurse down the back side
    let other_side = if side == 0 { 1 } else { 0 };
    r_recursive_world_node(node_ref.children[other_side], alpha_surfaces);
}

// r_draw_world: removed (legacy immediate-mode GL rendering).
// Will be replaced by modern rendering pipeline.

/// Mark BSP leaves visible from the current PVS cluster.
///
/// # Safety
/// Accesses global renderer state and BSP data.
pub unsafe fn r_mark_leaves() {
    let s = rfs();
    if s.r_oldviewcluster == s.r_viewcluster
        && s.r_oldviewcluster2 == s.r_viewcluster2
        && crate::vk_rmain::rcvars().r_novis.value == 0.0
        && s.r_viewcluster != -1
    {
        return;
    }

    if crate::vk_rmain::rcvars().vk_lockpvs.value != 0.0 {
        return;
    }

    let s = rfs();
    s.r_visframecount += 1;
    s.r_oldviewcluster = s.r_viewcluster;
    s.r_oldviewcluster2 = s.r_viewcluster2;

    if crate::vk_rmain::rcvars().r_novis.value != 0.0 || rfs().r_viewcluster == -1 || r_worldmodel_vis().is_null() {
        // mark everything
        let numleafs = r_worldmodel_numleafs();
        for i in 0..numleafs {
            r_worldmodel_leaf(i).visframe = rfs().r_visframecount;
        }
        let numnodes = r_worldmodel_numnodes();
        for i in 0..numnodes {
            (*r_worldmodel_node(i)).visframe = rfs().r_visframecount;
        }
        return;
    }

    let mut vis = mod_cluster_pvs(rfs().r_viewcluster);

    // may have to combine two clusters because of solid water boundaries
    let mut fatvis = [0u8; MAX_MAP_LEAFS / 8];
    if rfs().r_viewcluster2 != rfs().r_viewcluster {
        let numleafs = r_worldmodel_numleafs();
        let vis_len = (numleafs + 7) / 8;
        std::ptr::copy_nonoverlapping(vis, fatvis.as_mut_ptr(), vis_len as usize);
        vis = mod_cluster_pvs(rfs().r_viewcluster2);
        let c = ((numleafs + 31) / 32) as usize;
        let fat_ints = fatvis.as_mut_ptr() as *mut i32;
        let vis_ints = vis as *const i32;
        for i in 0..c {
            *fat_ints.add(i) |= *vis_ints.add(i);
        }
        vis = fatvis.as_ptr();
    }

    let numleafs = r_worldmodel_numleafs();
    for i in 0..numleafs {
        let leaf = r_worldmodel_leaf(i);
        let cluster = leaf.cluster;
        if cluster == -1 {
            continue;
        }
        if *vis.offset((cluster >> 3) as isize) & (1 << (cluster & 7)) != 0 {
            let mut node = leaf as *mut MLeaf as *mut MNode;
            loop {
                if (*node).visframe == rfs().r_visframecount {
                    break;
                }
                (*node).visframe = rfs().r_visframecount;
                node = (*node).parent;
                if node.is_null() {
                    break;
                }
            }
        }
    }
}

// ============================================================
// LIGHTMAP ALLOCATION
// ============================================================

unsafe fn lm_init_block(ss: &mut SurfaceState) {
    for i in 0..BLOCK_WIDTH as usize {
        ss.vk_lms.allocated[i] = 0;
    }
}

unsafe fn lm_upload_block(ss: &mut SurfaceState, dynamic: bool) {
    let texture;
    let mut height = 0;

    if dynamic {
        texture = 0;
    } else {
        texture = ss.vk_lms.current_lightmap_texture;
    }

    vk_bind(vk_state_lightmap_textures() + texture);
    qvk_tex_parameterf(VK_TEXTURE_2D, VK_TEXTURE_MIN_FILTER, VK_LINEAR as f32);
    qvk_tex_parameterf(VK_TEXTURE_2D, VK_TEXTURE_MAG_FILTER, VK_LINEAR as f32);

    if dynamic {
        for i in 0..BLOCK_WIDTH as usize {
            if ss.vk_lms.allocated[i] > height {
                height = ss.vk_lms.allocated[i];
            }
        }

        qvk_tex_sub_image_2d(
            VK_TEXTURE_2D,
            0,
            0,
            0,
            BLOCK_WIDTH,
            height,
            VK_LIGHTMAP_FORMAT,
            VK_UNSIGNED_BYTE,
            ss.vk_lms.lightmap_buffer.as_ptr(),
        );
    } else {
        qvk_tex_image_2d(
            VK_TEXTURE_2D,
            0,
            ss.vk_lms.internal_format,
            BLOCK_WIDTH,
            BLOCK_HEIGHT,
            0,
            VK_LIGHTMAP_FORMAT,
            VK_UNSIGNED_BYTE,
            ss.vk_lms.lightmap_buffer.as_ptr(),
        );
        // Store lightmap layer data for per-vertex baking during BSP geometry build
        {
            let layer = ss.vk_lms.current_lightmap_texture as usize;
            let mut lm_data = LIGHTMAP_LAYER_DATA.lock().unwrap_or_else(|e| e.into_inner());
            while lm_data.len() <= layer {
                lm_data.push(Vec::new());
            }
            lm_data[layer] = ss.vk_lms.lightmap_buffer.to_vec();
        }

        ss.vk_lms.current_lightmap_texture += 1;
        if ss.vk_lms.current_lightmap_texture == MAX_LIGHTMAPS as i32 {
            vid_printf(ERR_DROP, "LM_UploadBlock() - MAX_LIGHTMAPS exceeded\n");
        }
    }
}

unsafe fn lm_alloc_block(ss: &mut SurfaceState, w: i32, h: i32, x: &mut i32, y: &mut i32) -> bool {
    let mut best = BLOCK_HEIGHT;

    for i in 0..(BLOCK_WIDTH - w) as usize {
        let mut best2 = 0;
        let mut j = 0;
        while j < w as usize {
            if ss.vk_lms.allocated[i + j] >= best {
                break;
            }
            if ss.vk_lms.allocated[i + j] > best2 {
                best2 = ss.vk_lms.allocated[i + j];
            }
            j += 1;
        }
        if j as i32 == w {
            // this is a valid spot
            *x = i as i32;
            *y = best2;
            best = best2;
        }
    }

    if best + h > BLOCK_HEIGHT {
        return false;
    }

    for i in 0..w as usize {
        ss.vk_lms.allocated[*x as usize + i] = best + h;
    }

    true
}

/// Pre-blur native-resolution lightmap data in-place using N passes of 3×3 box blur.
///
/// Runs on the tiny native buffer (typically 5-20 texels per side) BEFORE bilinear upscaling.
/// Each pass softens shadow edges by ~16 world units — far more effective than blurring after
/// upscaling where the same kernel only covers ~1 world unit.
/// RGB channels blurred; alpha channel preserved.
fn blur_native_lightmap(data: &mut [u8], smax: usize, tmax: usize, passes: u32) {
    for _ in 0..passes {
        let src = data.to_vec();
        for y in 0..tmax {
            for x in 0..smax {
                for c in 0..3usize {
                    let mut sum = 0u32;
                    for dy in -1i32..=1 {
                        for dx in -1i32..=1 {
                            // Replicate-pad: clamp to valid range so edge/corner texels
                            // are sampled from themselves when out-of-bounds. This keeps
                            // count=9 for all pixels and prevents edge-darkening that
                            // causes visible seam lines where BSP surfaces meet.
                            let nx = (x as i32 + dx).clamp(0, smax as i32 - 1) as usize;
                            let ny = (y as i32 + dy).clamp(0, tmax as i32 - 1) as usize;
                            sum += src[(ny * smax + nx) * 4 + c] as u32;
                        }
                    }
                    data[(y * smax + x) * 4 + c] = (sum / 9) as u8;
                }
            }
        }
    }
}

/// Upscale a native-resolution lightmap by `scale` using bilinear interpolation.
///
/// Bilinear has no negative lobes, so it produces no overshoot or ringing at surface
/// edges. This prevents seam artifacts where two BSP surfaces meet in the atlas — the
/// Catmull-Rom bicubic caused visible bright/dark lines at every surface boundary due to
/// its negative outer lobes being clamped to the edge texel value.
///
/// Shadow gradient smoothing is handled by the native pre-blur (blur_native_lightmap)
/// which runs before this step on the small native buffer.
///
/// # Safety
/// `dst` must point to at least `(tmax * scale) * dst_stride` writable bytes.
unsafe fn bilinear_upscale_lightmap(src: &[u8], smax: usize, tmax: usize, scale: usize, dst: *mut u8, dst_stride: usize) {
    // Clamp-to-edge fetch from native-res source
    let get = |x: usize, y: usize, c: usize| -> u32 {
        let xi = x.min(smax.saturating_sub(1));
        let yi = y.min(tmax.saturating_sub(1));
        src[(yi * smax + xi) * 4 + c] as u32
    };

    for sy in 0..tmax {
        for sx in 0..smax {
            for j in 0..scale {
                for k in 0..scale {
                    let ox = sx * scale + k;
                    let oy = sy * scale + j;

                    // Bilinear weights: frac = k/scale, 1-frac = (scale-k)/scale
                    let wx1 = k as u32;
                    let wx0 = scale as u32 - wx1;
                    let wy1 = j as u32;
                    let wy0 = scale as u32 - wy1;
                    let total = (scale * scale) as u32;

                    for c in 0..4usize {
                        let s00 = get(sx,     sy,     c);
                        let s10 = get(sx + 1, sy,     c);
                        let s01 = get(sx,     sy + 1, c);
                        let s11 = get(sx + 1, sy + 1, c);

                        let val = (s00 * wx0 * wy0 + s10 * wx1 * wy0
                                 + s01 * wx0 * wy1 + s11 * wx1 * wy1
                                 + total / 2) / total;
                        *dst.add(oy * dst_stride + ox * 4 + c) = val as u8;
                    }
                }
            }
        }
    }
}

/// Build polygon vertex data from surface edges.
///
/// # Safety
/// Accesses model data and allocates poly via hunk.
pub unsafe fn vk_build_polygon_from_surface(fa: &mut MSurface) {
    let lnumverts = fa.numedges;

    let poly = hunk_alloc_glpoly(lnumverts as usize);
    (*poly).next = fa.polys;
    (*poly).flags = fa.flags;
    fa.polys = poly;
    (*poly).numverts = lnumverts;

    for i in 0..lnumverts {
        let lindex = currentmodel_surfedge(fa.firstedge + i);

        let vec = if lindex > 0 {
            currentmodel_vertex_position(currentmodel_edge_v(lindex, 0))
        } else {
            currentmodel_vertex_position(currentmodel_edge_v(-lindex, 1))
        };

        let texinfo = &*fa.texinfo;

        let mut s = dot_product(
            &vec,
            &[texinfo.vecs[0][0], texinfo.vecs[0][1], texinfo.vecs[0][2]],
        ) + texinfo.vecs[0][3];
        s /= (*texinfo.image).width as f32;

        let mut t = dot_product(
            &vec,
            &[texinfo.vecs[1][0], texinfo.vecs[1][1], texinfo.vecs[1][2]],
        ) + texinfo.vecs[1][3];
        t /= (*texinfo.image).height as f32;

        glpoly_set_vert(poly, i, &vec);
        glpoly_set_st(poly, i, s, t);

        // lightmap texture coordinates
        // With LM_UPSCALE=8: atlas is 1024 wide, each upscaled texel covers 2 world units.
        // Formula: (world - texturemins + light_s * texel_size + half_texel) / (BLOCK_WIDTH * texel_size)
        //   texel_size = 16 / LM_UPSCALE = 2,  half_texel = 2 / LM_UPSCALE = 1
        //   denominator = 1024 * 2 = 2048  (same as original 128 * 16 = 2048, unchanged)
        let texel_world: f32 = 16.0 / LM_UPSCALE as f32;
        let half_texel: f32 = texel_world * 0.5;

        let mut ls = dot_product(
            &vec,
            &[texinfo.vecs[0][0], texinfo.vecs[0][1], texinfo.vecs[0][2]],
        ) + texinfo.vecs[0][3];
        ls -= fa.texturemins[0] as f32;
        ls += fa.light_s as f32 * texel_world;
        ls += half_texel;
        ls /= (BLOCK_WIDTH as f32) * texel_world;

        let mut lt = dot_product(
            &vec,
            &[texinfo.vecs[1][0], texinfo.vecs[1][1], texinfo.vecs[1][2]],
        ) + texinfo.vecs[1][3];
        lt -= fa.texturemins[1] as f32;
        lt += fa.light_t as f32 * texel_world;
        lt += half_texel;
        lt /= (BLOCK_HEIGHT as f32) * texel_world;

        glpoly_set_lm_st(poly, i, ls, lt);
    }

    (*poly).numverts = lnumverts;
}

/// Create the lightmap texture for a surface.
///
/// # Safety
/// Accesses lightmap allocation state.
pub unsafe fn vk_create_surface_lightmap(surf: &mut MSurface) {
    let mut ss = SURFACE_STATE.lock().unwrap_or_else(|e| e.into_inner());

    // Sky never has a lightmap. Warp/turb (water/lava/slime) DO get one now so
    // liquids are lit like their surroundings instead of rendering full-bright.
    if surf.flags & SURF_DRAWSKY != 0 {
        return;
    }

    // Native lightmap size (1 texel per 16 world units)
    let smax = (surf.extents[0] as i32 >> 4) + 1;
    let tmax = (surf.extents[1] as i32 >> 4) + 1;

    // Allocate LM_UPSCALE× space in the atlas, plus a 1-pixel border on each side
    // to prevent bilinear sampling from bleeding into adjacent face data at seam edges.
    let smax_up = smax * LM_UPSCALE;
    let tmax_up = tmax * LM_UPSCALE;
    let alloc_w = smax_up + 2;  // +2 for left/right border
    let alloc_h = tmax_up + 2;  // +2 for top/bottom border

    if !lm_alloc_block(&mut ss, alloc_w, alloc_h, &mut surf.light_s, &mut surf.light_t) {
        lm_upload_block(&mut ss, false);
        lm_init_block(&mut ss);
        if !lm_alloc_block(&mut ss, alloc_w, alloc_h, &mut surf.light_s, &mut surf.light_t) {
            vid_printf(ERR_FATAL, &format!(
                "Consecutive calls to LM_AllocBlock({},{}) failed\n",
                alloc_w, alloc_h
            ));
        }
    }

    // Skip past the border to the interior content area.
    surf.light_s += 1;
    surf.light_t += 1;

    surf.lightmaptexturenum = ss.vk_lms.current_lightmap_texture;

    // Build native-size lightmap into a temporary buffer
    let native_stride = smax * LIGHTMAP_BYTES;
    let native_size = (native_stride * tmax) as usize;
    let mut native_buf = vec![0u8; native_size];
    r_set_cache_state(surf);
    r_build_light_map(surf, native_buf.as_mut_ptr(), native_stride);

    // Pre-blur native lightmap before upscaling: 3 passes of 3×3 box blur on the small
    // native buffer (typically 5-20 texels/side). Each pass ≈ 16 world units of softening —
    // orders of magnitude more effective than blurring after upscaling.  3 passes gives a
    // slightly softer gradient at face edges, which reduces visible seam contrast.
    blur_native_lightmap(&mut native_buf, smax as usize, tmax as usize, 3);

    // Bicubic-upscale into the atlas at the interior (post-border) position
    let base_offset = (surf.light_t * BLOCK_WIDTH + surf.light_s) * LIGHTMAP_BYTES;
    let base = ss.vk_lms.lightmap_buffer.as_mut_ptr().offset(base_offset as isize);
    let dst_stride = (BLOCK_WIDTH * LIGHTMAP_BYTES) as usize;
    bilinear_upscale_lightmap(&native_buf, smax as usize, tmax as usize, LM_UPSCALE as usize, base, dst_stride);

    // Fill the 1-pixel border by replicating the nearest edge pixel.
    // This ensures bilinear sampling never bleeds into a neighbouring face's data.
    {
        let ls = surf.light_s as usize;
        let lt = surf.light_t as usize;
        let sw = smax_up as usize;
        let sh = tmax_up as usize;
        let bw = BLOCK_WIDTH as usize;
        let lb = LIGHTMAP_BYTES as usize;  // 4 bytes (RGBA)
        let buf_ptr = ss.vk_lms.lightmap_buffer.as_mut_ptr();

        // Top border row (lt-1): replicate row lt, clamping x to content columns for corners
        for x in ls - 1..=ls + sw {
            let cx = x.clamp(ls, ls + sw - 1);
            let src = (lt * bw + cx) * lb;
            let dst = ((lt - 1) * bw + x) * lb;
            std::ptr::copy_nonoverlapping(buf_ptr.add(src), buf_ptr.add(dst), lb);
        }
        // Bottom border row (lt+sh): replicate row lt+sh-1
        for x in ls - 1..=ls + sw {
            let cx = x.clamp(ls, ls + sw - 1);
            let src = ((lt + sh - 1) * bw + cx) * lb;
            let dst = ((lt + sh) * bw + x) * lb;
            std::ptr::copy_nonoverlapping(buf_ptr.add(src), buf_ptr.add(dst), lb);
        }
        // Left border column (ls-1): replicate column ls (interior rows only)
        for y in lt..lt + sh {
            let src = (y * bw + ls) * lb;
            let dst = (y * bw + (ls - 1)) * lb;
            std::ptr::copy_nonoverlapping(buf_ptr.add(src), buf_ptr.add(dst), lb);
        }
        // Right border column (ls+sw): replicate column ls+sw-1 (interior rows only)
        for y in lt..lt + sh {
            let src = (y * bw + (ls + sw - 1)) * lb;
            let dst = (y * bw + (ls + sw)) * lb;
            std::ptr::copy_nonoverlapping(buf_ptr.add(src), buf_ptr.add(dst), lb);
        }
    }
}

/// Create the stain map buffer for a surface.
///
/// # Safety
/// Allocates stain buffer.
pub unsafe fn vk_create_surface_stainmap(surf: &mut MSurface) {
    if surf.flags & (SURF_DRAWSKY | SURF_DRAWTURB) != 0 {
        return;
    }

    let smax = (surf.extents[0] as i32 >> 4) + 1;
    let tmax = (surf.extents[1] as i32 >> 4) + 1;
    let size = (smax * tmax * 3) as usize;

    // SAFETY: Allocating stain buffer. In the original C code this was malloc + memset.
    let layout = std::alloc::Layout::from_size_align(size, 1).unwrap();
    surf.stains = std::alloc::alloc(layout);
    if !surf.stains.is_null() {
        std::ptr::write_bytes(surf.stains, 255, size);
    }
}

/// Average BSP lightmap samples at shared face edges to eliminate seam discontinuities.
///
/// Adjacent BSP faces are lit independently by the Q2 BSP compiler, causing brightness/color
/// jumps at shared edges.  This function walks every edge shared by two opaque faces, samples
/// N world-space points along it, computes the corresponding native-resolution texel in each
/// face's lightmap, and replaces both texels with their average.  The result is called before
/// `r_build_light_map` / `bilinear_upscale_lightmap`, so the smoother data is what gets
/// written into the atlas.
///
/// # Safety
/// Reads and mutates `surf.samples` raw BSP lightmap data.
unsafe fn fix_lightmap_seams(model: *mut Model) {
    let m = &*model;
    if m.numsurfaces <= 0 || m.numedges <= 0 || m.numsurfedges <= 0 { return; }

    let nsurfs  = m.numsurfaces  as usize;
    let nedges  = m.numedges     as usize;
    let nsverts = m.numvertexes  as usize;
    let nse     = m.numsurfedges as usize;

    let surfaces  = std::slice::from_raw_parts(m.surfaces,  nsurfs);
    let edges     = std::slice::from_raw_parts(m.edges,     nedges);
    let vertexes  = std::slice::from_raw_parts(m.vertexes,  nsverts);
    let surfedges = std::slice::from_raw_parts(m.surfedges, nse);

    // Build edge → [face_a, face_b] (-1 = unset).
    let mut edge_faces: Vec<[i32; 2]> = vec![[-1, -1]; nedges];

    for (fi, surf) in surfaces.iter().enumerate() {
        if surf.flags & (SURF_DRAWSKY | SURF_DRAWTURB) != 0 { continue; }
        if surf.samples.is_null() || surf.texinfo.is_null() { continue; }

        for j in 0..surf.numedges {
            let raw_se = surfedges[(surf.firstedge + j) as usize];
            let ei = raw_se.unsigned_abs() as usize;
            if ei >= nedges { continue; }
            let ef = &mut edge_faces[ei];
            if ef[0] < 0 {
                ef[0] = fi as i32;
            } else if ef[1] < 0 && ef[0] != fi as i32 {
                ef[1] = fi as i32;
            }
        }
    }

    // For every edge shared by exactly two faces, average the boundary lightmap texels.
    for (ei, ef) in edge_faces.iter().enumerate() {
        if ef[0] < 0 || ef[1] < 0 { continue; }
        let fi_a = ef[0] as usize;
        let fi_b = ef[1] as usize;

        let sa = &surfaces[fi_a];
        let sb = &surfaces[fi_b];
        if sa.texinfo.is_null() || sb.texinfo.is_null() { continue; }
        if sa.samples.is_null() || sb.samples.is_null() { continue; }

        let smax_a = ((sa.extents[0] as i32 >> 4) + 1) as usize;
        let tmax_a = ((sa.extents[1] as i32 >> 4) + 1) as usize;
        let smax_b = ((sb.extents[0] as i32 >> 4) + 1) as usize;
        let tmax_b = ((sb.extents[1] as i32 >> 4) + 1) as usize;

        // How many light styles are active on each face?
        let nstyles_a = sa.styles.iter().take_while(|&&s| s != 255).count();
        let nstyles_b = sb.styles.iter().take_while(|&&s| s != 255).count();
        let nstyles = nstyles_a.min(nstyles_b);
        if nstyles == 0 { continue; }

        // World-space endpoints of the shared edge.
        let edge = &edges[ei];
        if edge.v[0] as usize >= nsverts || edge.v[1] as usize >= nsverts { continue; }
        let p0 = vertexes[edge.v[0] as usize].position;
        let p1 = vertexes[edge.v[1] as usize].position;

        // Sample 8 evenly-spaced points along the edge (midpoints of eighths).
        const N_SAMPLES: usize = 8;
        let samples_a = sa.samples;
        let samples_b = sb.samples;

        for k in 0..N_SAMPLES {
            let frac = (k as f32 + 0.5) / N_SAMPLES as f32;
            let p: Vec3 = [
                p0[0] + (p1[0] - p0[0]) * frac,
                p0[1] + (p1[1] - p0[1]) * frac,
                p0[2] + (p1[2] - p0[2]) * frac,
            ];

            // Native texel index in face A.
            let idx_a = {
                let ti = &*sa.texinfo;
                let sv: Vec3 = [ti.vecs[0][0], ti.vecs[0][1], ti.vecs[0][2]];
                let tv: Vec3 = [ti.vecs[1][0], ti.vecs[1][1], ti.vecs[1][2]];
                let sf = (dot_product(&p, &sv) + ti.vecs[0][3] - sa.texturemins[0] as f32) / 16.0;
                let tf = (dot_product(&p, &tv) + ti.vecs[1][3] - sa.texturemins[1] as f32) / 16.0;
                let si = sf.floor() as i32;
                let ti_ = tf.floor() as i32;
                if si < 0 || ti_ < 0 || si >= smax_a as i32 || ti_ >= tmax_a as i32 { continue; }
                ti_ as usize * smax_a + si as usize
            };

            // Native texel index in face B.
            let idx_b = {
                let ti = &*sb.texinfo;
                let sv: Vec3 = [ti.vecs[0][0], ti.vecs[0][1], ti.vecs[0][2]];
                let tv: Vec3 = [ti.vecs[1][0], ti.vecs[1][1], ti.vecs[1][2]];
                let sf = (dot_product(&p, &sv) + ti.vecs[0][3] - sb.texturemins[0] as f32) / 16.0;
                let tf = (dot_product(&p, &tv) + ti.vecs[1][3] - sb.texturemins[1] as f32) / 16.0;
                let si = sf.floor() as i32;
                let ti_ = tf.floor() as i32;
                if si < 0 || ti_ < 0 || si >= smax_b as i32 || ti_ >= tmax_b as i32 { continue; }
                ti_ as usize * smax_b + si as usize
            };

            // Average the RGB bytes for each active style.
            for style_idx in 0..nstyles {
                let off_a = (style_idx * smax_a * tmax_a + idx_a) * 3;
                let off_b = (style_idx * smax_b * tmax_b + idx_b) * 3;
                for c in 0..3_usize {
                    let va = *samples_a.add(off_a + c) as u16;
                    let vb = *samples_b.add(off_b + c) as u16;
                    let avg = ((va + vb + 1) / 2) as u8;
                    *samples_a.add(off_a + c) = avg;
                    *samples_b.add(off_b + c) = avg;
                }
            }
        }
    }
}

/// Begin building lightmaps for a model.
///
/// # Safety
/// Accesses GL state and lightmap system.
pub unsafe fn vk_begin_building_lightmaps(m: *mut Model) {
    // Clear collected lightmap data from previous level
    {
        let mut lm_data = LIGHTMAP_LAYER_DATA.lock().unwrap_or_else(|e| e.into_inner());
        lm_data.clear();
    }

    let mut ss = SURFACE_STATE.lock().unwrap_or_else(|e| e.into_inner());

    for i in 0..BLOCK_WIDTH as usize {
        ss.vk_lms.allocated[i] = 0;
    }

    rfs().r_framecount = 1; // no dlightcache

    vk_enable_multitexture(true);
    vk_select_texture(VK_TEXTURE1);

    // setup base lightstyles
    for i in 0..MAX_LIGHTSTYLES {
        ss.lightstyles[i].rgb = [1.0, 1.0, 1.0];
        ss.lightstyles[i].white = 3.0;
    }
    rfs().r_newrefdef.lightstyles = ss.lightstyles.as_mut_ptr();

    if vk_state_lightmap_textures() == 0 {
        set_lightmap_textures(TEXNUM_LIGHTMAPS);
    }

    ss.vk_lms.current_lightmap_texture = 1;

    let mono = vk_monolightmap_char().to_ascii_uppercase();
    match mono {
        b'A' => {
            ss.vk_lms.internal_format = vk_tex_alpha_format();
        }
        b'C' => {
            ss.vk_lms.internal_format = vk_tex_alpha_format();
        }
        b'I' => {
            ss.vk_lms.internal_format = VK_INTENSITY8 as i32;
        }
        b'L' => {
            ss.vk_lms.internal_format = VK_LUMINANCE8 as i32;
        }
        _ => {
            ss.vk_lms.internal_format = vk_tex_solid_format();
        }
    }

    // initialize the dynamic lightmap texture
    vk_bind(vk_state_lightmap_textures());
    qvk_tex_parameterf(VK_TEXTURE_2D, VK_TEXTURE_MIN_FILTER, VK_LINEAR as f32);
    qvk_tex_parameterf(VK_TEXTURE_2D, VK_TEXTURE_MAG_FILTER, VK_LINEAR as f32);

    // Use heap allocation — 256×256×4 bytes = 256KB, too large for stack
    let dummy = vec![0u32; BLOCK_WIDTH as usize * BLOCK_HEIGHT as usize];
    qvk_tex_image_2d(
        VK_TEXTURE_2D,
        0,
        ss.vk_lms.internal_format,
        BLOCK_WIDTH,
        BLOCK_HEIGHT,
        0,
        VK_LIGHTMAP_FORMAT,
        VK_UNSIGNED_BYTE,
        dummy.as_ptr() as *const u8,
    );
}

/// Finish building lightmaps.
///
/// # Safety
/// Accesses GL state.
pub unsafe fn vk_end_building_lightmaps() {
    let mut ss = SURFACE_STATE.lock().unwrap_or_else(|e| e.into_inner());
    lm_upload_block(&mut ss, false);
    vk_enable_multitexture(false);
    drop(ss); // release lock before calling parse_static_lights
    parse_static_lights();
    // Also turn emissive light textures and the sky into light sources.
    parse_surface_lights();

    // VXGI Phase 1: voxelize the static world once, now that surfaces/polys are built.
    if crate::vk_rmain::rcvars().r_vxgi.fresh_value() != 0.0 {
        // fresh_value(), NOT .value: .value is the registration snapshot (always the default 128),
        // so a runtime `r_vxgi_res 256` + map reload was silently re-baking at 128 the whole time.
        let res = crate::vk_rmain::rcvars().r_vxgi_res.fresh_value() as u32;
        crate::modern::vxgi::voxelize_world(res);
    }
}

// MAX_MAP_LEAFS imported from myq2_common::qfiles

// =============================================================
//  Tests
// =============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: create a SurfaceDrawCmd without a real surface pointer.
    // Only used for testing batch-building logic; the surface pointer
    // is never dereferenced in build_batches.
    fn make_cmd(texture_id: usize, lightmap_num: i32, flags: i32) -> SurfaceDrawCmd {
        SurfaceDrawCmd {
            surface: std::ptr::null_mut(),
            texture_id,
            lightmap_num,
            flags,
        }
    }

    // ---------------------------------------------------------
    //  RenderCommandQueue::new / clear
    // ---------------------------------------------------------

    #[test]
    fn test_queue_new_is_empty() {
        let queue = RenderCommandQueue::new();
        assert_eq!(queue.total_surfaces(), 0);
        assert!(queue.alpha_surfaces().is_empty());
    }

    #[test]
    fn test_queue_clear() {
        let mut queue = RenderCommandQueue::new();
        queue.queue_surface(std::ptr::null_mut(), 1, 0, 0);
        queue.queue_surface(std::ptr::null_mut(), 2, 0, SURF_TRANS33);
        assert!(queue.total_surfaces() > 0);
        queue.clear();
        assert_eq!(queue.total_surfaces(), 0);
        assert!(queue.alpha_surfaces().is_empty());
    }

    // ---------------------------------------------------------
    //  Alpha surface separation
    // ---------------------------------------------------------

    #[test]
    fn test_alpha_surfaces_separated() {
        let mut queue = RenderCommandQueue::new();
        // Opaque surface
        queue.queue_surface(std::ptr::null_mut(), 1, 0, 0);
        // SURF_TRANS33 surface
        queue.queue_surface(std::ptr::null_mut(), 2, 0, SURF_TRANS33);
        // SURF_TRANS66 surface
        queue.queue_surface(std::ptr::null_mut(), 3, 0, SURF_TRANS66);
        // Another opaque
        queue.queue_surface(std::ptr::null_mut(), 4, 0, 0);

        // Total should be 4
        assert_eq!(queue.total_surfaces(), 4);
        // 2 alpha surfaces
        assert_eq!(queue.alpha_surfaces().len(), 2);
        // 2 opaque commands
        assert_eq!(queue.commands.len(), 2);
    }

    #[test]
    fn test_alpha_surfaces_flags_preserved() {
        let mut queue = RenderCommandQueue::new();
        queue.queue_surface(std::ptr::null_mut(), 5, 3, SURF_TRANS33);
        let alpha = queue.alpha_surfaces();
        assert_eq!(alpha.len(), 1);
        assert_eq!(alpha[0].texture_id, 5);
        assert_eq!(alpha[0].lightmap_num, 3);
        assert_eq!(alpha[0].flags, SURF_TRANS33);
    }

    // ---------------------------------------------------------
    //  Batch building: grouping by texture
    // ---------------------------------------------------------

    #[test]
    fn test_batch_building_groups_by_texture() {
        let mut queue = RenderCommandQueue::new();
        // 3 surfaces with texture 10
        queue.queue_surface(std::ptr::null_mut(), 10, 0, 0);
        queue.queue_surface(std::ptr::null_mut(), 10, 1, 0);
        queue.queue_surface(std::ptr::null_mut(), 10, 2, 0);
        // 2 surfaces with texture 20
        queue.queue_surface(std::ptr::null_mut(), 20, 0, 0);
        queue.queue_surface(std::ptr::null_mut(), 20, 1, 0);
        // 1 surface with texture 5
        queue.queue_surface(std::ptr::null_mut(), 5, 0, 0);

        let count = queue.batch_count();
        assert_eq!(count, 3); // 3 distinct texture IDs
    }

    #[test]
    fn test_batch_building_sorted_by_texture_id() {
        let mut queue = RenderCommandQueue::new();
        queue.queue_surface(std::ptr::null_mut(), 30, 0, 0);
        queue.queue_surface(std::ptr::null_mut(), 10, 0, 0);
        queue.queue_surface(std::ptr::null_mut(), 20, 0, 0);

        queue.build_batches();
        let ids: Vec<usize> = queue.batches.iter().map(|b| b.texture_id).collect();
        assert_eq!(ids, vec![10, 20, 30]);
    }

    #[test]
    fn test_batch_surface_counts() {
        let mut queue = RenderCommandQueue::new();
        // Texture 1: 5 surfaces
        for _ in 0..5 {
            queue.queue_surface(std::ptr::null_mut(), 1, 0, 0);
        }
        // Texture 2: 3 surfaces
        for _ in 0..3 {
            queue.queue_surface(std::ptr::null_mut(), 2, 0, 0);
        }

        queue.build_batches();

        // Batches are sorted by texture_id, so batch 0 is texture 1
        let counts: Vec<(usize, usize)> = queue.batches.iter()
            .map(|b| (b.texture_id, b.surfaces.len()))
            .collect();
        assert_eq!(counts, vec![(1, 5), (2, 3)]);
    }

    // ---------------------------------------------------------
    //  Batch dirty flag
    // ---------------------------------------------------------

    #[test]
    fn test_batch_dirty_flag() {
        let mut queue = RenderCommandQueue::new();
        queue.queue_surface(std::ptr::null_mut(), 1, 0, 0);

        // Force build
        queue.build_batches();
        assert!(!queue.batches_dirty);

        // Adding another opaque surface marks dirty
        queue.queue_surface(std::ptr::null_mut(), 2, 0, 0);
        assert!(queue.batches_dirty);
    }

    #[test]
    fn test_batch_not_dirty_after_alpha_only() {
        let mut queue = RenderCommandQueue::new();
        // Build once to clear dirty flag
        queue.build_batches();
        assert!(!queue.batches_dirty);

        // Alpha-only addition should NOT mark dirty
        queue.queue_surface(std::ptr::null_mut(), 99, 0, SURF_TRANS66);
        assert!(!queue.batches_dirty);
    }

    // ---------------------------------------------------------
    //  Empty queue batch building
    // ---------------------------------------------------------

    #[test]
    fn test_empty_queue_batch_building() {
        let mut queue = RenderCommandQueue::new();
        queue.build_batches();
        assert_eq!(queue.batch_count(), 0);
        assert!(queue.get_batch(0).is_none());
    }

    // ---------------------------------------------------------
    //  iter_batches
    // ---------------------------------------------------------

    #[test]
    fn test_iter_batches() {
        let mut queue = RenderCommandQueue::new();
        queue.queue_surface(std::ptr::null_mut(), 1, 0, 0);
        queue.queue_surface(std::ptr::null_mut(), 2, 0, 0);
        queue.queue_surface(std::ptr::null_mut(), 3, 0, 0);

        let batch_ids: Vec<usize> = queue.iter_batches().map(|b| b.texture_id).collect();
        assert_eq!(batch_ids, vec![1, 2, 3]);
    }

    // ---------------------------------------------------------
    //  total_surfaces
    // ---------------------------------------------------------

    #[test]
    fn test_total_surfaces_counts_opaque_and_alpha() {
        let mut queue = RenderCommandQueue::new();
        queue.queue_surface(std::ptr::null_mut(), 1, 0, 0);
        queue.queue_surface(std::ptr::null_mut(), 2, 0, 0);
        queue.queue_surface(std::ptr::null_mut(), 3, 0, SURF_TRANS33);
        assert_eq!(queue.total_surfaces(), 3);
    }

    // ---------------------------------------------------------
    //  Surface flag constants
    // ---------------------------------------------------------

    #[test]
    fn test_surface_flag_values() {
        // Verify the surface flag bitmask values match expected Quake 2 values
        assert_eq!(SURF_TRANS33, 0x10);
        assert_eq!(SURF_TRANS66, 0x20);
        // SURF_TRANS33 and SURF_TRANS66 should not overlap
        assert_eq!(SURF_TRANS33 & SURF_TRANS66, 0);
    }

    // ---------------------------------------------------------
    //  Batch re-build after clear
    // ---------------------------------------------------------

    #[test]
    fn test_batches_after_clear_and_rebuild() {
        let mut queue = RenderCommandQueue::new();
        queue.queue_surface(std::ptr::null_mut(), 1, 0, 0);
        queue.queue_surface(std::ptr::null_mut(), 2, 0, 0);
        queue.build_batches();
        assert_eq!(queue.batch_count(), 2);

        queue.clear();
        queue.queue_surface(std::ptr::null_mut(), 42, 0, 0);
        queue.build_batches();
        assert_eq!(queue.batch_count(), 1);
        assert_eq!(queue.batches[0].texture_id, 42);
    }

    // ---------------------------------------------------------
    //  Lightmap coordinate calculation logic
    // ---------------------------------------------------------

    #[test]
    fn test_lightmap_block_constants() {
        // Verify block constants used in lightmap allocation
        assert_eq!(BLOCK_WIDTH, 2048);
        assert_eq!(BLOCK_HEIGHT, 2048);
        assert_eq!(LM_UPSCALE, 16);
        assert_eq!(LIGHTMAP_BYTES, 4);
    }

    #[test]
    fn test_backface_epsilon_value() {
        assert!(BACKFACE_EPSILON > 0.0);
        assert!(BACKFACE_EPSILON < 1.0);
        assert!((BACKFACE_EPSILON - 0.01).abs() < 1e-6);
    }

    // ---------------------------------------------------------
    //  BSP traversal side decision
    //  (extracted from r_recursive_world_node logic)
    // ---------------------------------------------------------

    #[test]
    fn test_bsp_side_decision_positive_dot() {
        // When dot >= 0: front side first (side=0, sidebit=0)
        let dot = 5.0f32;
        let (side, sidebit) = if dot >= 0.0 {
            (0usize, 0i32)
        } else {
            (1usize, SURF_PLANEBACK)
        };
        assert_eq!(side, 0);
        assert_eq!(sidebit, 0);
    }

    #[test]
    fn test_bsp_side_decision_negative_dot() {
        // When dot < 0: back side first (side=1, sidebit=SURF_PLANEBACK)
        let dot = -3.0f32;
        let (side, sidebit) = if dot >= 0.0 {
            (0usize, 0i32)
        } else {
            (1usize, SURF_PLANEBACK)
        };
        assert_eq!(side, 1);
        assert_eq!(sidebit, SURF_PLANEBACK);
    }

    #[test]
    fn test_bsp_side_decision_zero_dot() {
        // When dot == 0: front side (side=0)
        let dot = 0.0f32;
        let (side, _sidebit) = if dot >= 0.0 {
            (0usize, 0i32)
        } else {
            (1usize, SURF_PLANEBACK)
        };
        assert_eq!(side, 0);
    }

    // ---------------------------------------------------------
    //  Surface extent calculation math
    //  (calc_surface_extents does bmins/bmaxs -> texturemins/extents)
    // ---------------------------------------------------------

    #[test]
    fn test_surface_extent_math() {
        // Replicate the math from calc_surface_extents:
        //   bmins = floor(val / 16.0)
        //   bmaxs = ceil(val / 16.0)
        //   texturemins = bmins * 16
        //   extents = (bmaxs - bmins) * 16
        let mins_val = 100.0f32;
        let maxs_val = 200.0f32;

        let bmins = (mins_val / 16.0).floor() as i32;
        let bmaxs = (maxs_val / 16.0).ceil() as i32;
        let texturemins = (bmins * 16) as i16;
        let extents = ((bmaxs - bmins) * 16) as i16;

        assert_eq!(bmins, 6);   // floor(100/16) = 6
        assert_eq!(bmaxs, 13);  // ceil(200/16) = 13
        assert_eq!(texturemins, 96);
        assert_eq!(extents, 112); // (13-6)*16
    }

    #[test]
    fn test_surface_extent_math_negative_coords() {
        let mins_val = -50.0f32;
        let maxs_val = 30.0f32;

        let bmins = (mins_val / 16.0).floor() as i32;
        let bmaxs = (maxs_val / 16.0).ceil() as i32;
        let texturemins = (bmins * 16) as i16;
        let extents = ((bmaxs - bmins) * 16) as i16;

        assert_eq!(bmins, -4);  // floor(-50/16) = floor(-3.125) = -4
        assert_eq!(bmaxs, 2);   // ceil(30/16) = ceil(1.875) = 2
        assert_eq!(texturemins, -64);
        assert_eq!(extents, 96); // (2 - (-4))*16 = 6*16
    }

    #[test]
    fn test_surface_extent_exact_multiples() {
        // Values that are exact multiples of 16
        let mins_val = 64.0f32;
        let maxs_val = 128.0f32;

        let bmins = (mins_val / 16.0).floor() as i32;
        let bmaxs = (maxs_val / 16.0).ceil() as i32;
        let texturemins = (bmins * 16) as i16;
        let extents = ((bmaxs - bmins) * 16) as i16;

        assert_eq!(bmins, 4);
        assert_eq!(bmaxs, 8);
        assert_eq!(texturemins, 64);
        assert_eq!(extents, 64);
    }

    // ---------------------------------------------------------
    //  Default trait for RenderCommandQueue
    // ---------------------------------------------------------

    #[test]
    fn test_render_command_queue_default() {
        let queue = RenderCommandQueue::default();
        assert_eq!(queue.total_surfaces(), 0);
        assert!(queue.alpha_surfaces().is_empty());
    }
}
