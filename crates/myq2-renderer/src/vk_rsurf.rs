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

const BLOCK_WIDTH: i32 = 128;
const BLOCK_HEIGHT: i32 = 128;

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
        let mut ls = dot_product(
            &vec,
            &[texinfo.vecs[0][0], texinfo.vecs[0][1], texinfo.vecs[0][2]],
        ) + texinfo.vecs[0][3];
        ls -= fa.texturemins[0] as f32;
        ls += fa.light_s as f32 * 16.0;
        ls += 8.0;
        ls /= (BLOCK_WIDTH * 16) as f32;

        let mut lt = dot_product(
            &vec,
            &[texinfo.vecs[1][0], texinfo.vecs[1][1], texinfo.vecs[1][2]],
        ) + texinfo.vecs[1][3];
        lt -= fa.texturemins[1] as f32;
        lt += fa.light_t as f32 * 16.0;
        lt += 8.0;
        lt /= (BLOCK_HEIGHT * 16) as f32;

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

    if surf.flags & (SURF_DRAWSKY | SURF_DRAWTURB) != 0 {
        return;
    }

    let smax = (surf.extents[0] as i32 >> 4) + 1;
    let tmax = (surf.extents[1] as i32 >> 4) + 1;

    if !lm_alloc_block(&mut ss, smax, tmax, &mut surf.light_s, &mut surf.light_t) {
        lm_upload_block(&mut ss, false);
        lm_init_block(&mut ss);
        if !lm_alloc_block(&mut ss, smax, tmax, &mut surf.light_s, &mut surf.light_t) {
            vid_printf(ERR_FATAL, &format!(
                "Consecutive calls to LM_AllocBlock({},{}) failed\n",
                smax, tmax
            ));
        }
    }

    surf.lightmaptexturenum = ss.vk_lms.current_lightmap_texture;

    let base_offset =
        (surf.light_t * BLOCK_WIDTH + surf.light_s) * LIGHTMAP_BYTES;
    let base = ss.vk_lms
        .lightmap_buffer
        .as_mut_ptr()
        .offset(base_offset as isize);

    r_set_cache_state(surf);
    r_build_light_map(surf, base, BLOCK_WIDTH * LIGHTMAP_BYTES);
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

/// Begin building lightmaps for a model.
///
/// # Safety
/// Accesses GL state and lightmap system.
pub unsafe fn vk_begin_building_lightmaps(_m: *mut Model) {
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

    let dummy = [0u32; 128 * 128];
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
        assert_eq!(BLOCK_WIDTH, 128);
        assert_eq!(BLOCK_HEIGHT, 128);
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
