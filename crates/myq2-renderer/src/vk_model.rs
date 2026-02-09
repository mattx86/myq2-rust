// Copyright (C) 1997-2001 Id Software, Inc.
// GPL-2.0-or-later
//
// vk_model.c -> vk_model.rs
// Model loading and caching

#![allow(dead_code, non_upper_case_globals, static_mut_refs)]

use crate::vk_local::*;
use crate::vk_rmain::vid_printf;
use myq2_common::q_shared::{
    CPlane, Vec3, dot_product, vector_length, little_float,
    CONTENTS_WATER, CONTENTS_SLIME, CONTENTS_LAVA,
    SURF_SKY, SURF_TRANS33, SURF_TRANS66, SURF_WARP,
    MAX_QPATH, PRINT_ALL, ERR_DROP,
};
use myq2_common::common::com_error;
use myq2_common::qfiles::*;

// little_short/little_long from q_shared (canonical location)
use myq2_common::q_shared::{little_short, little_long};

// =============================================================
//  Constants
// =============================================================

pub const MAX_MOD_KNOWN: usize = 512;

// =============================================================
//  Module-level state (matches C globals)
// =============================================================

/// Length of the file being loaded.
static mut modfilelen: i32 = 0;

/// No-vis data: all clusters visible.
static mut mod_novis: [u8; MAX_MAP_LEAFS / 8] = [0xFF; MAX_MAP_LEAFS / 8];

/// All known models.
static mut mod_known: [Model; MAX_MOD_KNOWN] = unsafe { std::mem::zeroed() };
static mut mod_numknown: i32 = 0;

/// Inline submodels from the current map, kept separate.
static mut mod_inline: [Model; MAX_MOD_KNOWN] = unsafe { std::mem::zeroed() };

/// Registration sequence counter.
pub static mut registration_sequence: i32 = 0;

/// Raw base pointer for the currently-loading BSP.
static mut mod_base: *const u8 = std::ptr::null();

// =============================================================
//  Hunk allocator stubs
//  In the original C code, Hunk_Begin/Hunk_Alloc/Hunk_End
//  manage a contiguous memory region. We simulate this with
//  a simple bump allocator backed by a Vec<u8>.
// =============================================================

static mut hunk_buf: Vec<u8> = Vec::new();
static mut hunk_cur: usize = 0;

unsafe fn hunk_begin(maxsize: usize) -> *mut u8 {
    hunk_buf = Vec::with_capacity(maxsize);
    hunk_buf.resize(maxsize, 0);
    hunk_cur = 0;
    hunk_buf.as_mut_ptr()
}

unsafe fn hunk_alloc(size: usize) -> *mut u8 {
    // Align to 16 bytes
    let aligned = (hunk_cur + 15) & !15;
    if aligned + size > hunk_buf.len() {
        com_error(ERR_DROP, "Hunk_Alloc: overflow");
    }
    let ptr = hunk_buf.as_mut_ptr().add(aligned);
    hunk_cur = aligned + size;
    ptr
}

unsafe fn hunk_end() -> i32 {
    hunk_buf.truncate(hunk_cur);
    hunk_buf.shrink_to_fit();
    hunk_cur as i32
}

unsafe fn hunk_free(_data: *mut u8) {
    // In a real implementation this would free the hunk.
    // For now, leak — matching the C code's lifetime semantics.
}

// =============================================================
//  FS stubs
// =============================================================

unsafe fn fs_load_file(name: &[u8; MAX_QPATH]) -> Option<Vec<u8>> {
    let name_str = std::str::from_utf8(name)
        .unwrap_or("")
        .trim_end_matches('\0');
    if name_str.is_empty() {
        return None;
    }
    myq2_common::files::fs_load_file(name_str)
}

// =============================================================
//  GL stubs referenced by this module
// =============================================================

unsafe fn vk_build_polygon_from_surface(fa: *mut MSurface) {
    // Delegates to the full implementation in vk_rsurf.rs
    crate::vk_rsurf::vk_build_polygon_from_surface(&mut *fa);
}

unsafe fn vk_create_surface_lightmap(surf: *mut MSurface) {
    // Delegates to the full implementation in vk_rsurf.rs
    crate::vk_rsurf::vk_create_surface_lightmap(&mut *surf);
}

unsafe fn vk_create_surface_stainmap(surf: *mut MSurface) {
    // Delegates to the full implementation in vk_rsurf.rs
    crate::vk_rsurf::vk_create_surface_stainmap(&mut *surf);
}

unsafe fn vk_begin_building_lightmaps(m: *mut Model) {
    // Delegates to the full implementation in vk_rsurf.rs
    crate::vk_rsurf::vk_begin_building_lightmaps(m);
}

unsafe fn vk_end_building_lightmaps() {
    // Delegates to the full implementation in vk_rsurf.rs
    crate::vk_rsurf::vk_end_building_lightmaps();
}

unsafe fn vk_subdivide_surface(fa: *mut MSurface) {
    // Delegates to the full implementation in vk_warp.rs
    crate::vk_warp::vk_subdivide_surface(fa);
}

unsafe fn vk_free_unused_images() {
    // Delegates to the full implementation in vk_image.rs
    crate::vk_image::vk_free_unused_images_impl();
}

unsafe fn vk_find_image_c(name: *const u8, img_type: ImageType) -> *mut Image {
    // Convert C string pointer to Rust &str, then delegate to vk_image.rs
    if name.is_null() {
        return std::ptr::null_mut();
    }
    let c_str = std::ffi::CStr::from_ptr(name as *const i8);
    let name_str = match c_str.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    crate::vk_image::vk_find_image_impl(name_str, img_type)
}

// =============================================================
//  Helper: model name as &str
// =============================================================

fn model_name_str(name: &[u8; MAX_QPATH]) -> &str {
    let len = name.iter().position(|&b| b == 0).unwrap_or(MAX_QPATH);
    std::str::from_utf8(&name[..len]).unwrap_or("")
}

fn set_model_name(dst: &mut [u8; MAX_QPATH], src: &str) {
    let bytes = src.as_bytes();
    let len = bytes.len().min(MAX_QPATH - 1);
    dst[..len].copy_from_slice(&bytes[..len]);
    dst[len] = 0;
    for i in (len + 1)..MAX_QPATH {
        dst[i] = 0;
    }
}

fn model_name_is_empty(name: &[u8; MAX_QPATH]) -> bool {
    name[0] == 0
}

fn model_names_match(a: &[u8; MAX_QPATH], b: &[u8; MAX_QPATH]) -> bool {
    a == b
}

fn model_name_matches_str(name: &[u8; MAX_QPATH], s: &str) -> bool {
    model_name_str(name) == s
}

// =============================================================
//  Mod_PointInLeaf
// =============================================================

/// Walk the BSP tree to find the leaf containing point `p`.
///
/// # Safety
/// Dereferences raw model pointers.
pub unsafe fn mod_point_in_leaf(p: &Vec3, model: *mut Model) -> *mut MLeaf {
    if model.is_null() || (*model).nodes.is_null() {
        com_error(ERR_DROP, "Mod_PointInLeaf: bad model");
    }

    let mut node = (*model).nodes;
    loop {
        if (*node).contents != -1 {
            return node as *mut MLeaf;
        }
        let plane = (*node).plane;
        let d = dot_product(p, &(*plane).normal) - (*plane).dist;
        if d > 0.0 {
            node = (*node).children[0];
        } else {
            node = (*node).children[1];
        }
    }
}

// =============================================================
//  Mod_DecompressVis
// =============================================================

/// Decompress run-length encoded PVS data.
///
/// # Safety
/// Dereferences raw pointers.
pub unsafe fn mod_decompress_vis(input: *mut u8, model: *mut Model) -> *mut u8 {
    static mut decompressed: [u8; MAX_MAP_LEAFS / 8] = [0u8; MAX_MAP_LEAFS / 8];

    let row = (((*(*model).vis).numclusters + 7) >> 3) as usize;
    let out_base = decompressed.as_mut_ptr();
    let mut out = out_base;

    if input.is_null() {
        // no vis info, so make all visible
        for i in 0..row {
            *out_base.add(i) = 0xFF;
        }
        return out_base;
    }

    let mut inp = input;
    loop {
        if (out as usize - out_base as usize) >= row {
            break;
        }
        if *inp != 0 {
            *out = *inp;
            inp = inp.add(1);
            out = out.add(1);
            continue;
        }

        // Run of zeros
        let c = *inp.add(1) as usize;
        inp = inp.add(2);
        for _ in 0..c {
            *out = 0;
            out = out.add(1);
        }
    }

    out_base
}

// =============================================================
//  Mod_ClusterPVS
// =============================================================

/// Get the PVS for a cluster.
///
/// # Safety
/// Dereferences raw model pointers.
pub unsafe fn mod_cluster_pvs_raw(cluster: i32, model: *mut Model) -> *mut u8 {
    if cluster == -1 || (*model).vis.is_null() {
        return mod_novis.as_mut_ptr();
    }
    let vis = (*model).vis;
    // SAFETY: bitofs is declared as [[i32; 2]; 1] but is variable-sized.
    // Access via raw pointer arithmetic.
    let bitofs_ptr = (*vis).bitofs.as_ptr() as *const [i32; 2];
    let ofs = (*bitofs_ptr.add(cluster as usize))[DVIS_PVS as usize];
    mod_decompress_vis((vis as *mut u8).add(ofs as usize), model)
}

// =============================================================
//  Mod_Modellist_f
// =============================================================

/// Console command: list all loaded models and their sizes.
///
/// # Safety
/// Accesses global model arrays.
pub unsafe fn mod_modellist_f() {
    let mut total = 0i32;
    vid_printf(PRINT_ALL, "Loaded models:\n");
    for i in 0..mod_numknown {
        let m = &mod_known[i as usize];
        if model_name_is_empty(&m.name) {
            continue;
        }
        vid_printf(PRINT_ALL, &format!("{:8} : {}\n", m.extradatasize, model_name_str(&m.name)));
        total += m.extradatasize;
    }
    vid_printf(PRINT_ALL, &format!("Total resident: {}\n", total));
}

// =============================================================
//  Mod_Init
// =============================================================

/// Initialize the model subsystem.
///
/// # Safety
/// Writes to global state.
pub unsafe fn mod_init() {
    mod_novis = [0xFF; MAX_MAP_LEAFS / 8];
}

// =============================================================
//  Mod_ForName
// =============================================================

/// Load a model by name. If `crash` is true, errors are fatal.
///
/// # Safety
/// Accesses global model arrays and filesystem.
pub unsafe fn mod_for_name(name: &str, crash: bool) -> *mut Model {
    if name.is_empty() {
        com_error(ERR_DROP, "Mod_ForName: NULL name");
    }

    // inline models are grabbed only from worldmodel
    if name.starts_with('*') {
        let i: i32 = name[1..].parse().unwrap_or(0);
        if i < 1 || r_worldmodel.is_null() || i >= (*r_worldmodel).numsubmodels {
            com_error(ERR_DROP, "bad inline model number");
        }
        return &mut mod_inline[i as usize];
    }

    // search the currently loaded models
    for i in 0..mod_numknown as usize {
        if model_name_is_empty(&mod_known[i].name) {
            continue;
        }
        if model_name_matches_str(&mod_known[i].name, name) {
            return &mut mod_known[i];
        }
    }

    // find a free model slot
    let mut slot: usize = mod_numknown as usize;
    for i in 0..mod_numknown as usize {
        if model_name_is_empty(&mod_known[i].name) {
            slot = i;
            break;
        }
    }
    if slot == mod_numknown as usize {
        if mod_numknown as usize == MAX_MOD_KNOWN {
            com_error(ERR_DROP, "mod_numknown == MAX_MOD_KNOWN");
        }
        mod_numknown += 1;
    }
    set_model_name(&mut mod_known[slot].name, name);

    // load the file
    let buf = fs_load_file(&mod_known[slot].name);
    if buf.is_none() {
        if crash {
            com_error(ERR_DROP, &format!("Mod_NumForName: {} not found", name));
        }
        mod_known[slot].name = [0u8; MAX_QPATH];
        return std::ptr::null_mut();
    }
    let buf = buf.unwrap();
    modfilelen = buf.len() as i32;

    loadmodel = &mut mod_known[slot];

    // call the appropriate loader based on file magic
    if buf.len() >= 4 {
        let ident = little_long(i32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]));
        match ident {
            IDALIASHEADER => {
                (*loadmodel).extradata = hunk_begin(0x200000);
                mod_load_alias_model(&mut mod_known[slot], buf.as_ptr() as *mut u8);
            }
            IDSPRITEHEADER => {
                (*loadmodel).extradata = hunk_begin(0x10000);
                mod_load_sprite_model(&mut mod_known[slot], buf.as_ptr() as *mut u8);
            }
            IDBSPHEADER => {
                (*loadmodel).extradata = hunk_begin(0x1000000);
                mod_load_brush_model(&mut mod_known[slot], buf.as_ptr() as *mut u8);
            }
            _ => {
                com_error(ERR_DROP, &format!("Mod_NumForName: unknown fileid for {}", name));
            }
        }
    }

    (*loadmodel).extradatasize = hunk_end();

    &mut mod_known[slot]
}

// =============================================================
//  RadiusFromBounds
// =============================================================

pub fn radius_from_bounds(mins: &Vec3, maxs: &Vec3) -> f32 {
    let mut corner = [0.0f32; 3];
    for i in 0..3 {
        corner[i] = if mins[i].abs() > maxs[i].abs() {
            mins[i].abs()
        } else {
            maxs[i].abs()
        };
    }
    vector_length(&corner)
}

// ===============================================================
//  BRUSHMODEL LOADING
// ===============================================================

/// Load lighting lump from BSP.
///
/// # Safety
/// Dereferences raw pointers, writes to loadmodel.
unsafe fn mod_load_lighting(l: *const Lump) {
    if (*l).filelen == 0 {
        (*loadmodel).lightdata = std::ptr::null_mut();
        return;
    }
    let size = (*l).filelen as usize;
    let dst = hunk_alloc(size);
    std::ptr::copy_nonoverlapping(mod_base.add((*l).fileofs as usize), dst, size);
    (*loadmodel).lightdata = dst;
}

/// Load visibility lump from BSP.
///
/// # Safety
/// Dereferences raw pointers, writes to loadmodel.
unsafe fn mod_load_visibility(l: *const Lump) {
    if (*l).filelen == 0 {
        (*loadmodel).vis = std::ptr::null_mut();
        return;
    }
    let size = (*l).filelen as usize;
    let dst = hunk_alloc(size) as *mut DvisT;
    std::ptr::copy_nonoverlapping(mod_base.add((*l).fileofs as usize), dst as *mut u8, size);
    (*loadmodel).vis = dst;

    (*dst).numclusters = little_long((*dst).numclusters);
    let bitofs_ptr = (*dst).bitofs.as_mut_ptr() as *mut [i32; 2];
    for i in 0..(*dst).numclusters as usize {
        (*bitofs_ptr.add(i))[0] = little_long((*bitofs_ptr.add(i))[0]);
        (*bitofs_ptr.add(i))[1] = little_long((*bitofs_ptr.add(i))[1]);
    }
}

/// Load vertexes lump from BSP.
///
/// # Safety
/// Dereferences raw pointers, writes to loadmodel.
unsafe fn mod_load_vertexes(l: *const Lump) {
    let in_size = std::mem::size_of::<DVertex>();
    if !((*l).filelen as usize).is_multiple_of(in_size) {
        com_error(ERR_DROP, &format!("MOD_LoadBmodel: funny lump size in {}", model_name_str(&(*loadmodel).name)));
    }
    let count = (*l).filelen as usize / in_size;
    let out = hunk_alloc(count * std::mem::size_of::<MVertex>()) as *mut MVertex;

    (*loadmodel).vertexes = out;
    (*loadmodel).numvertexes = count as i32;

    let mut inp = (mod_base.add((*l).fileofs as usize)) as *const DVertex;
    let mut outp = out;
    for _ in 0..count {
        (*outp).position[0] = little_float((*inp).point[0]);
        (*outp).position[1] = little_float((*inp).point[1]);
        (*outp).position[2] = little_float((*inp).point[2]);
        inp = inp.add(1);
        outp = outp.add(1);
    }
}

/// Load submodels lump from BSP.
///
/// # Safety
/// Dereferences raw pointers, writes to loadmodel.
unsafe fn mod_load_submodels(l: *const Lump) {
    let in_size = std::mem::size_of::<DModel>();
    if !((*l).filelen as usize).is_multiple_of(in_size) {
        com_error(ERR_DROP, &format!("MOD_LoadBmodel: funny lump size in {}", model_name_str(&(*loadmodel).name)));
    }
    let count = (*l).filelen as usize / in_size;
    let out = hunk_alloc(count * std::mem::size_of::<MModel>()) as *mut MModel;

    (*loadmodel).submodels = out;
    (*loadmodel).numsubmodels = count as i32;

    let mut inp = (mod_base.add((*l).fileofs as usize)) as *const DModel;
    let mut outp = out;
    for _ in 0..count {
        for j in 0..3 {
            (*outp).mins[j] = little_float((*inp).mins[j]) - 1.0;
            (*outp).maxs[j] = little_float((*inp).maxs[j]) + 1.0;
            (*outp).origin[j] = little_float((*inp).origin[j]);
        }
        (*outp).radius = radius_from_bounds(&(*outp).mins, &(*outp).maxs);
        (*outp).headnode = little_long((*inp).headnode);
        (*outp).firstface = little_long((*inp).firstface);
        (*outp).numfaces = little_long((*inp).numfaces);
        inp = inp.add(1);
        outp = outp.add(1);
    }
}

/// Load edges lump from BSP.
///
/// # Safety
/// Dereferences raw pointers, writes to loadmodel.
unsafe fn mod_load_edges(l: *const Lump) {
    let in_size = std::mem::size_of::<DEdge>();
    if !((*l).filelen as usize).is_multiple_of(in_size) {
        com_error(ERR_DROP, &format!("MOD_LoadBmodel: funny lump size in {}", model_name_str(&(*loadmodel).name)));
    }
    let count = (*l).filelen as usize / in_size;
    let out = hunk_alloc((count + 1) * std::mem::size_of::<MEdge>()) as *mut MEdge;

    (*loadmodel).edges = out;
    (*loadmodel).numedges = count as i32;

    let mut inp = (mod_base.add((*l).fileofs as usize)) as *const DEdge;
    let mut outp = out;
    for _ in 0..count {
        (*outp).v[0] = little_short((*inp).v[0] as i16) as u16;
        (*outp).v[1] = little_short((*inp).v[1] as i16) as u16;
        inp = inp.add(1);
        outp = outp.add(1);
    }
}

/// Load texinfo lump from BSP.
///
/// # Safety
/// Dereferences raw pointers, writes to loadmodel.
unsafe fn mod_load_texinfo(l: *const Lump) {
    let in_size = std::mem::size_of::<TexInfo>();
    if !((*l).filelen as usize).is_multiple_of(in_size) {
        com_error(ERR_DROP, &format!("MOD_LoadBmodel: funny lump size in {}", model_name_str(&(*loadmodel).name)));
    }
    let count = (*l).filelen as usize / in_size;
    let out = hunk_alloc(count * std::mem::size_of::<MTexInfo>()) as *mut MTexInfo;

    (*loadmodel).texinfo = out;
    (*loadmodel).numtexinfo = count as i32;

    // Phase 1: Parse texinfo and collect all texture names
    let mut texture_names: Vec<String> = Vec::with_capacity(count);
    let mut inp = (mod_base.add((*l).fileofs as usize)) as *const TexInfo;
    let mut outp = out;

    for _i in 0..count {
        // vecs is [[f32;4];2] — 8 floats total
        let src_floats = (*inp).vecs.as_ptr() as *const f32;
        let dst_floats = (*outp).vecs.as_mut_ptr() as *mut f32;
        for j in 0..8 {
            *dst_floats.add(j) = little_float(*src_floats.add(j));
        }

        (*outp).flags = little_long((*inp).flags);
        let next = little_long((*inp).nexttexinfo);
        if next > 0 {
            (*outp).next = (*loadmodel).texinfo.add(next as usize);
        } else {
            (*outp).next = std::ptr::null_mut();
        }

        // Build texture name: "textures/<texture>.wal"
        let texture_str = {
            let tex = &(*inp).texture;
            let len = tex.iter().position(|&b| b == 0).unwrap_or(tex.len());
            std::str::from_utf8(&tex[..len]).unwrap_or("")
        };
        let full_name = format!("textures/{}.wal", texture_str);
        texture_names.push(full_name);

        // Store name in texinfo for later reference
        let mut name_buf = [0u8; MAX_QPATH];
        set_model_name(
            &mut *(&mut name_buf as *mut [u8; MAX_QPATH]),
            &texture_names[texture_names.len() - 1],
        );

        inp = inp.add(1);
        outp = outp.add(1);
    }

    // Phase 2: Batch load all textures (parallel decode, sequential upload)
    let loaded_images = crate::vk_image::vk_batch_load_textures(&texture_names, ImageType::Wall);

    // Phase 3: Assign loaded images to texinfo entries
    for (idx, img_ptr) in loaded_images.into_iter().enumerate() {
        let ti = (*loadmodel).texinfo.add(idx);
        if img_ptr.is_null() {
            vid_printf(PRINT_ALL, &format!("Couldn't load {}\n", texture_names[idx]));
            (*ti).image = r_notexture;
        } else {
            (*ti).image = img_ptr;
        }
    }

    // count animation frames
    for idx in 0..count {
        let ti = (*loadmodel).texinfo.add(idx);
        (*ti).numframes = 1;
        let mut step = (*ti).next;
        while !step.is_null() && step != ti {
            (*ti).numframes += 1;
            step = (*step).next;
        }
    }
}

/// Calculate surface extents from vertices and texinfo.
///
/// # Safety
/// Dereferences raw model pointers.
unsafe fn calc_surface_extents(s: *mut MSurface) {
    let mut mins = [999999.0f32; 2];
    let mut maxs = [-99999.0f32; 2];

    let tex = (*s).texinfo;

    for i in 0..(*s).numedges {
        let e = *(*loadmodel).surfedges.add(((*s).firstedge + i) as usize);
        let v: *const MVertex;
        if e >= 0 {
            v = (*loadmodel).vertexes.add((*(*loadmodel).edges.add(e as usize)).v[0] as usize);
        } else {
            v = (*loadmodel).vertexes.add((*(*loadmodel).edges.add((-e) as usize)).v[1] as usize);
        }

        for j in 0..2usize {
            let val = (*v).position[0] * (*tex).vecs[j][0]
                + (*v).position[1] * (*tex).vecs[j][1]
                + (*v).position[2] * (*tex).vecs[j][2]
                + (*tex).vecs[j][3];
            if val < mins[j] { mins[j] = val; }
            if val > maxs[j] { maxs[j] = val; }
        }
    }

    for i in 0..2usize {
        let bmins = (mins[i] / 16.0).floor() as i32;
        let bmaxs = (maxs[i] / 16.0).ceil() as i32;
        (*s).texturemins[i] = (bmins * 16) as i16;
        (*s).extents[i] = ((bmaxs - bmins) * 16) as i16;
    }
}

/// Load faces lump from BSP.
///
/// # Safety
/// Dereferences raw pointers, writes to loadmodel.
unsafe fn mod_load_faces(l: *const Lump) {
    let in_size = std::mem::size_of::<DFace>();
    if !((*l).filelen as usize).is_multiple_of(in_size) {
        com_error(ERR_DROP, &format!("MOD_LoadBmodel: funny lump size in {}", model_name_str(&(*loadmodel).name)));
    }
    let count = (*l).filelen as usize / in_size;
    let out = hunk_alloc(count * std::mem::size_of::<MSurface>()) as *mut MSurface;

    (*loadmodel).surfaces = out;
    (*loadmodel).numsurfaces = count as i32;

    currentmodel = loadmodel;

    vk_begin_building_lightmaps(loadmodel);

    let mut inp = (mod_base.add((*l).fileofs as usize)) as *const DFace;
    let mut outp = out;
    for _surfnum in 0..count {
        (*outp).firstedge = little_long((*inp).firstedge);
        (*outp).numedges = little_short((*inp).numedges) as i32;
        (*outp).flags = 0;
        (*outp).polys = std::ptr::null_mut();

        let planenum = little_short((*inp).planenum as i16) as usize;
        let side = little_short((*inp).side);
        if side != 0 {
            (*outp).flags |= SURF_PLANEBACK;
        }

        (*outp).plane = (*loadmodel).planes.add(planenum);

        let ti = little_short((*inp).texinfo) as i32;
        if ti < 0 || ti >= (*loadmodel).numtexinfo {
            com_error(ERR_DROP, "MOD_LoadBmodel: bad texinfo number");
        }
        (*outp).texinfo = (*loadmodel).texinfo.add(ti as usize);

        calc_surface_extents(outp);

        // lighting info
        for i in 0..MAXLIGHTMAPS {
            (*outp).styles[i] = (*inp).styles[i];
        }
        let lightofs = little_long((*inp).lightofs);
        if lightofs == -1 {
            (*outp).samples = std::ptr::null_mut();
            (*outp).stains = std::ptr::null_mut();
        } else {
            (*outp).samples = (*loadmodel).lightdata.add(lightofs as usize);
        }

        // set the drawing flags
        if (*(*outp).texinfo).flags & SURF_WARP != 0 {
            (*outp).flags |= SURF_DRAWTURB;
            for i in 0..2 {
                (*outp).extents[i] = 16384;
                (*outp).texturemins[i] = -8192;
            }
            vk_subdivide_surface(outp); // cut up polygon for warps
        }

        // create lightmaps and polygons
        if (*(*outp).texinfo).flags & (SURF_SKY | SURF_TRANS33 | SURF_TRANS66 | SURF_WARP) == 0 {
            vk_create_surface_lightmap(outp);
            vk_create_surface_stainmap(outp);
        }

        if (*(*outp).texinfo).flags & SURF_WARP == 0 {
            vk_build_polygon_from_surface(outp);
        }

        inp = inp.add(1);
        outp = outp.add(1);
    }

    vk_end_building_lightmaps();
}

/// Recursively set parent pointers for BSP nodes.
///
/// # Safety
/// Dereferences raw node pointers.
unsafe fn mod_set_parent(node: *mut MNode, parent: *mut MNode) {
    (*node).parent = parent;
    if (*node).contents != -1 {
        return;
    }
    mod_set_parent((*node).children[0], node);
    mod_set_parent((*node).children[1], node);
}

/// Load nodes lump from BSP.
///
/// # Safety
/// Dereferences raw pointers, writes to loadmodel.
unsafe fn mod_load_nodes(l: *const Lump) {
    let in_size = std::mem::size_of::<DNode>();
    if !((*l).filelen as usize).is_multiple_of(in_size) {
        com_error(ERR_DROP, &format!("MOD_LoadBmodel: funny lump size in {}", model_name_str(&(*loadmodel).name)));
    }
    let count = (*l).filelen as usize / in_size;
    let out = hunk_alloc(count * std::mem::size_of::<MNode>()) as *mut MNode;

    (*loadmodel).nodes = out;
    (*loadmodel).numnodes = count as i32;

    let mut inp = (mod_base.add((*l).fileofs as usize)) as *const DNode;
    let mut outp = out;
    for _ in 0..count {
        for j in 0..3 {
            (*outp).minmaxs[j] = little_short((*inp).mins[j]) as f32;
            (*outp).minmaxs[3 + j] = little_short((*inp).maxs[j]) as f32;
        }

        let p = little_long((*inp).planenum);
        (*outp).plane = (*loadmodel).planes.add(p as usize);

        (*outp).firstsurface = little_short((*inp).firstface as i16) as u16;
        (*outp).numsurfaces = little_short((*inp).numfaces as i16) as u16;
        (*outp).contents = -1; // differentiate from leafs

        for j in 0..2 {
            let child = little_long((*inp).children[j]);
            if child >= 0 {
                (*outp).children[j] = (*loadmodel).nodes.add(child as usize);
            } else {
                (*outp).children[j] = (*loadmodel).leafs.add((-1 - child) as usize) as *mut MNode;
            }
        }

        inp = inp.add(1);
        outp = outp.add(1);
    }

    mod_set_parent((*loadmodel).nodes, std::ptr::null_mut()); // sets nodes and leafs
}

/// Load leafs lump from BSP.
///
/// # Safety
/// Dereferences raw pointers, writes to loadmodel.
unsafe fn mod_load_leafs(l: *const Lump) {
    let in_size = std::mem::size_of::<DLeaf>();
    if !((*l).filelen as usize).is_multiple_of(in_size) {
        com_error(ERR_DROP, &format!("MOD_LoadBmodel: funny lump size in {}", model_name_str(&(*loadmodel).name)));
    }
    let count = (*l).filelen as usize / in_size;
    let out = hunk_alloc(count * std::mem::size_of::<MLeaf>()) as *mut MLeaf;

    (*loadmodel).leafs = out;
    (*loadmodel).numleafs = count as i32;

    let mut inp = (mod_base.add((*l).fileofs as usize)) as *const DLeaf;
    let mut outp = out;
    for _ in 0..count {
        for j in 0..3 {
            (*outp).minmaxs[j] = little_short((*inp).mins[j]) as f32;
            (*outp).minmaxs[3 + j] = little_short((*inp).maxs[j]) as f32;
        }

        (*outp).contents = little_long((*inp).contents);
        (*outp).cluster = little_short((*inp).cluster) as i32;
        (*outp).area = little_short((*inp).area) as i32;

        (*outp).firstmarksurface = (*loadmodel).marksurfaces.add(
            little_short((*inp).firstleafface as i16) as usize,
        );
        (*outp).nummarksurfaces = little_short((*inp).numleaffaces as i16) as i32;

        // gl underwater warp
        if (*outp).contents & (CONTENTS_WATER | CONTENTS_SLIME | CONTENTS_LAVA) != 0 {
            for j in 0..(*outp).nummarksurfaces {
                let surf = *(*outp).firstmarksurface.add(j as usize);
                (*surf).flags |= SURF_UNDERWATER;
                let mut poly = (*surf).polys;
                while !poly.is_null() {
                    (*poly).flags |= SURF_UNDERWATER;
                    poly = (*poly).next;
                }
            }
        }

        inp = inp.add(1);
        outp = outp.add(1);
    }
}

/// Load marksurfaces lump from BSP.
///
/// # Safety
/// Dereferences raw pointers, writes to loadmodel.
unsafe fn mod_load_marksurfaces(l: *const Lump) {
    let in_size = std::mem::size_of::<i16>();
    if !((*l).filelen as usize).is_multiple_of(in_size) {
        com_error(ERR_DROP, &format!("MOD_LoadBmodel: funny lump size in {}", model_name_str(&(*loadmodel).name)));
    }
    let count = (*l).filelen as usize / in_size;
    let out = hunk_alloc(count * std::mem::size_of::<*mut MSurface>()) as *mut *mut MSurface;

    (*loadmodel).marksurfaces = out;
    (*loadmodel).nummarksurfaces = count as i32;

    let inp = mod_base.add((*l).fileofs as usize) as *const i16;
    for i in 0..count {
        let j = little_short(*inp.add(i)) as i32;
        if j < 0 || j >= (*loadmodel).numsurfaces {
            com_error(ERR_DROP, "Mod_ParseMarksurfaces: bad surface number");
        }
        *out.add(i) = (*loadmodel).surfaces.add(j as usize);
    }
}

/// Load surfedges lump from BSP.
///
/// # Safety
/// Dereferences raw pointers, writes to loadmodel.
unsafe fn mod_load_surfedges(l: *const Lump) {
    let in_size = std::mem::size_of::<i32>();
    if !((*l).filelen as usize).is_multiple_of(in_size) {
        com_error(ERR_DROP, &format!("MOD_LoadBmodel: funny lump size in {}", model_name_str(&(*loadmodel).name)));
    }
    let count = (*l).filelen as usize / in_size;
    if !(1..MAX_MAP_SURFEDGES).contains(&count) {
        com_error(ERR_DROP, &format!(
            "MOD_LoadBmodel: bad surfedges count in {}: {}",
            model_name_str(&(*loadmodel).name),
            count
        ));
    }

    let out = hunk_alloc(count * std::mem::size_of::<i32>()) as *mut i32;

    (*loadmodel).surfedges = out;
    (*loadmodel).numsurfedges = count as i32;

    let inp = mod_base.add((*l).fileofs as usize) as *const i32;
    for i in 0..count {
        *out.add(i) = little_long(*inp.add(i));
    }
}

/// Load planes lump from BSP.
///
/// # Safety
/// Dereferences raw pointers, writes to loadmodel.
unsafe fn mod_load_planes(l: *const Lump) {
    let in_size = std::mem::size_of::<DPlane>();
    if !((*l).filelen as usize).is_multiple_of(in_size) {
        com_error(ERR_DROP, &format!("MOD_LoadBmodel: funny lump size in {}", model_name_str(&(*loadmodel).name)));
    }
    let count = (*l).filelen as usize / in_size;
    let out = hunk_alloc(count * 2 * std::mem::size_of::<CPlane>()) as *mut CPlane;

    (*loadmodel).planes = out;
    (*loadmodel).numplanes = count as i32;

    let mut inp = (mod_base.add((*l).fileofs as usize)) as *const DPlane;
    let mut outp = out;
    for _ in 0..count {
        let mut bits: u8 = 0;
        for j in 0..3 {
            (*outp).normal[j] = little_float((*inp).normal[j]);
            if (*outp).normal[j] < 0.0 {
                bits |= 1 << j;
            }
        }
        (*outp).dist = little_float((*inp).dist);
        (*outp).plane_type = little_long((*inp).plane_type) as u8;
        (*outp).signbits = bits;

        inp = inp.add(1);
        outp = outp.add(1);
    }
}

/// Load a brush (BSP) model from raw file data.
///
/// # Safety
/// Dereferences raw pointers, accesses global model state.
unsafe fn mod_load_brush_model(model: *mut Model, buffer: *mut u8) {
    (*loadmodel).r#type = ModType::Brush;
    if loadmodel != mod_known.as_mut_ptr() {
        com_error(ERR_DROP, "Loaded a brush model after the world");
    }

    let header = buffer as *mut DHeader;

    let version = little_long((*header).version);
    if version != BSPVERSION {
        com_error(ERR_DROP, &format!(
            "Mod_LoadBrushModel: {} has wrong version number ({} should be {})",
            model_name_str(&(*model).name),
            version,
            BSPVERSION
        ));
    }

    // swap all the lumps
    mod_base = buffer as *const u8;
    let header_ints = header as *mut i32;
    let num_ints = std::mem::size_of::<DHeader>() / 4;
    for i in 0..num_ints {
        *header_ints.add(i) = little_long(*header_ints.add(i));
    }

    // load into heap
    mod_load_vertexes(&(*header).lumps[LUMP_VERTEXES]);
    mod_load_edges(&(*header).lumps[LUMP_EDGES]);
    mod_load_surfedges(&(*header).lumps[LUMP_SURFEDGES]);
    mod_load_lighting(&(*header).lumps[LUMP_LIGHTING]);
    mod_load_planes(&(*header).lumps[LUMP_PLANES]);
    mod_load_texinfo(&(*header).lumps[LUMP_TEXINFO]);
    mod_load_faces(&(*header).lumps[LUMP_FACES]);
    mod_load_marksurfaces(&(*header).lumps[LUMP_LEAFFACES]);
    mod_load_visibility(&(*header).lumps[LUMP_VISIBILITY]);
    mod_load_leafs(&(*header).lumps[LUMP_LEAFS]);
    mod_load_nodes(&(*header).lumps[LUMP_NODES]);
    mod_load_submodels(&(*header).lumps[LUMP_MODELS]);
    (*model).numframes = 2; // regular and alternate animation

    // set up the submodels
    for i in 0..(*model).numsubmodels {
        let bm = &*(*model).submodels.add(i as usize);
        let starmod = &mut mod_inline[i as usize];

        // Copy the entire loadmodel into starmod
        std::ptr::copy_nonoverlapping(loadmodel as *const Model, starmod as *mut Model, 1);

        starmod.firstmodelsurface = bm.firstface;
        starmod.nummodelsurfaces = bm.numfaces;
        starmod.firstnode = bm.headnode;
        if starmod.firstnode >= (*loadmodel).numnodes {
            com_error(ERR_DROP, &format!("Inline model {} has bad firstnode", i));
        }

        starmod.maxs = bm.maxs;
        starmod.mins = bm.mins;
        starmod.radius = bm.radius;

        if i == 0 {
            std::ptr::copy_nonoverlapping(starmod as *const Model, loadmodel, 1);
        }

        starmod.numleafs = bm.visleafs;
    }
}

// ===============================================================
//  ALIAS MODELS
// ===============================================================

/// Load an alias (MD2) model from raw file data.
///
/// # Safety
/// Dereferences raw pointers, accesses global model state.
unsafe fn mod_load_alias_model(model: *mut Model, buffer: *mut u8) {
    let pinmodel = buffer as *const DMdl;

    let version = little_long((*pinmodel).version);
    if version != ALIAS_VERSION {
        com_error(ERR_DROP, &format!(
            "{} has wrong version number ({} should be {})",
            model_name_str(&(*model).name),
            version,
            ALIAS_VERSION
        ));
    }

    let ofs_end = little_long((*pinmodel).ofs_end) as usize;
    let pheader = hunk_alloc(ofs_end) as *mut DMdl;

    // byte swap the header fields and sanity check
    let num_ints = std::mem::size_of::<DMdl>() / 4;
    let src_ints = buffer as *const i32;
    let dst_ints = pheader as *mut i32;
    for i in 0..num_ints {
        *dst_ints.add(i) = little_long(*src_ints.add(i));
    }

    if (*pheader).skinheight > MAX_LBM_HEIGHT {
        com_error(ERR_DROP, &format!(
            "model {} has a skin taller than {}",
            model_name_str(&(*model).name),
            MAX_LBM_HEIGHT
        ));
    }
    if (*pheader).num_xyz <= 0 {
        com_error(ERR_DROP, &format!("model {} has no vertices", model_name_str(&(*model).name)));
    }
    if (*pheader).num_xyz > MAX_VERTS as i32 {
        com_error(ERR_DROP, &format!("model {} has too many vertices", model_name_str(&(*model).name)));
    }
    if (*pheader).num_st <= 0 {
        com_error(ERR_DROP, &format!("model {} has no st vertices", model_name_str(&(*model).name)));
    }
    if (*pheader).num_tris <= 0 {
        com_error(ERR_DROP, &format!("model {} has no triangles", model_name_str(&(*model).name)));
    }
    if (*pheader).num_frames <= 0 {
        com_error(ERR_DROP, &format!("model {} has no frames", model_name_str(&(*model).name)));
    }

    // load base s and t vertices
    let pinst = (buffer as *const u8).add((*pheader).ofs_st as usize) as *const DStVert;
    let poutst = (pheader as *mut u8).add((*pheader).ofs_st as usize) as *mut DStVert;
    for i in 0..(*pheader).num_st as usize {
        (*poutst.add(i)).s = little_short((*pinst.add(i)).s);
        (*poutst.add(i)).t = little_short((*pinst.add(i)).t);
    }

    // load triangle lists
    let pintri = (buffer as *const u8).add((*pheader).ofs_tris as usize) as *const DTriangle;
    let pouttri = (pheader as *mut u8).add((*pheader).ofs_tris as usize) as *mut DTriangle;
    for i in 0..(*pheader).num_tris as usize {
        for j in 0..3 {
            (*pouttri.add(i)).index_xyz[j] = little_short((*pintri.add(i)).index_xyz[j]);
            (*pouttri.add(i)).index_st[j] = little_short((*pintri.add(i)).index_st[j]);
        }
    }

    // load the frames
    for i in 0..(*pheader).num_frames as usize {
        let frame_ofs = (*pheader).ofs_frames as usize + i * (*pheader).framesize as usize;
        let pinframe = (buffer as *const u8).add(frame_ofs) as *const DAliasFrame;
        let poutframe = (pheader as *mut u8).add(frame_ofs) as *mut DAliasFrame;

        std::ptr::copy_nonoverlapping(
            (*pinframe).name.as_ptr(),
            (*poutframe).name.as_mut_ptr(),
            16,
        );
        for j in 0..3 {
            (*poutframe).scale[j] = little_float((*pinframe).scale[j]);
            (*poutframe).translate[j] = little_float((*pinframe).translate[j]);
        }
        // verts are all 8 bit, so no swapping needed
        let verts_size = (*pheader).num_xyz as usize * std::mem::size_of::<DTriVertx>();
        let verts_ofs = frame_ofs + std::mem::size_of::<DAliasFrame>();
        std::ptr::copy_nonoverlapping(
            (buffer as *const u8).add(verts_ofs),
            (pheader as *mut u8).add(verts_ofs),
            verts_size,
        );
    }

    (*model).r#type = ModType::Alias;

    // load the glcmds
    let pincmd = (buffer as *const u8).add((*pheader).ofs_glcmds as usize) as *const i32;
    let poutcmd = (pheader as *mut u8).add((*pheader).ofs_glcmds as usize) as *mut i32;
    for i in 0..(*pheader).num_glcmds as usize {
        *poutcmd.add(i) = little_long(*pincmd.add(i));
    }

    // register all skins
    let skin_src = (buffer as *const u8).add((*pheader).ofs_skins as usize);
    let skin_dst = (pheader as *mut u8).add((*pheader).ofs_skins as usize);
    std::ptr::copy_nonoverlapping(
        skin_src,
        skin_dst,
        (*pheader).num_skins as usize * MAX_SKINNAME,
    );
    for i in 0..(*pheader).num_skins as usize {
        let skin_name_ptr = skin_dst.add(i * MAX_SKINNAME);
        (*model).skins[i] = vk_find_image_c(skin_name_ptr, ImageType::Skin);
    }

    (*model).mins = [-32.0, -32.0, -32.0];
    (*model).maxs = [32.0, 32.0, 32.0];
}

// ===============================================================
//  SPRITE MODELS
// ===============================================================

/// Load a sprite (SP2) model from raw file data.
///
/// # Safety
/// Dereferences raw pointers, accesses global model state.
unsafe fn mod_load_sprite_model(model: *mut Model, buffer: *mut u8) {
    let sprin = buffer as *const DSprite;
    let sprout = hunk_alloc(modfilelen as usize) as *mut DSprite;

    (*sprout).ident = little_long((*sprin).ident);
    (*sprout).version = little_long((*sprin).version);
    (*sprout).numframes = little_long((*sprin).numframes);

    if (*sprout).version != SPRITE_VERSION {
        com_error(ERR_DROP, &format!(
            "{} has wrong version number ({} should be {})",
            model_name_str(&(*model).name),
            (*sprout).version,
            SPRITE_VERSION
        ));
    }
    if (*sprout).numframes > MAX_MD2SKINS as i32 {
        com_error(ERR_DROP, &format!(
            "{} has too many frames ({} > {})",
            model_name_str(&(*model).name),
            (*sprout).numframes,
            MAX_MD2SKINS
        ));
    }

    // byte swap frame data and register skins
    let sprite_header_size = std::mem::size_of::<DSprite>();
    let frame_size = std::mem::size_of::<DSprFrame>();
    for i in 0..(*sprout).numframes as usize {
        let in_frame = (buffer as *const u8).add(sprite_header_size + i * frame_size) as *const DSprFrame;
        let out_frame = (sprout as *mut u8).add(sprite_header_size + i * frame_size) as *mut DSprFrame;

        (*out_frame).width = little_long((*in_frame).width);
        (*out_frame).height = little_long((*in_frame).height);
        (*out_frame).origin_x = little_long((*in_frame).origin_x);
        (*out_frame).origin_y = little_long((*in_frame).origin_y);
        std::ptr::copy_nonoverlapping(
            (*in_frame).name.as_ptr(),
            (*out_frame).name.as_mut_ptr(),
            MAX_SKINNAME,
        );
        (*model).skins[i] = vk_find_image_c((*out_frame).name.as_ptr(), ImageType::Sprite);
    }

    (*model).r#type = ModType::Sprite;
}

// =============================================================
//  Registration
// =============================================================

/// Begin model registration for a new map.
///
/// # Safety
/// Accesses global model state.
pub unsafe fn r_begin_registration(model_name: &str) {
    registration_sequence += 1;
    r_oldviewcluster = -1; // force markleafs

    let fullname = format!("maps/{}.bsp", model_name);

    // explicitly free the old map if different
    // this guarantees that mod_known[0] is the world map
    // Also flush if flushmap cvar is set
    let flushmap = myq2_common::cvar::cvar_variable_value("flushmap");
    if flushmap != 0.0 || !model_name_matches_str(&mod_known[0].name, &fullname) {
        mod_free(&mut mod_known[0]);
    }
    r_worldmodel = mod_for_name(&fullname, true);

    r_viewcluster = -1;

    // Build modern BSP geometry from loaded world model
    build_modern_bsp_geometry();
}

/// Walk all world model surfaces, extract vertices from GlPoly chains,
/// triangulate, and upload to the modern renderer's BspGeometryManager.
unsafe fn build_modern_bsp_geometry() {
    use crate::modern::geometry::{BspVertex, SurfaceDrawInfo};

    if r_worldmodel.is_null() {
        return;
    }

    let modern = match crate::vk_rmain::MODERN.as_mut() {
        Some(m) => m,
        None => return,
    };

    let model = &*r_worldmodel;
    let num_surfaces = model.numsurfaces;
    let surfaces_ptr = model.surfaces;
    if surfaces_ptr.is_null() || num_surfaces <= 0 {
        return;
    }

    let mut vertices: Vec<BspVertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut surface_infos: Vec<SurfaceDrawInfo> = Vec::new();

    for i in 0..num_surfaces as usize {
        let surface = &*surfaces_ptr.add(i);

        // Skip special surfaces
        if surface.flags & (SURF_DRAWSKY as i32 | SURF_DRAWTURB as i32) != 0 {
            continue;
        }

        let mut poly = surface.polys;
        if poly.is_null() {
            continue;
        }

        // Get texture ID from texinfo
        let texture_id = if !surface.texinfo.is_null() && !(*surface.texinfo).image.is_null() {
            (*(*surface.texinfo).image).texnum as u32
        } else {
            0
        };

        let lightmap_id = surface.lightmaptexturenum as u32;
        let flags = surface.flags as u32;

        // Walk the GlPoly chain for this surface
        while !poly.is_null() {
            let numverts = (*poly).numverts;
            if numverts < 3 {
                poly = (*poly).next;
                continue;
            }

            let first_index = indices.len() as u32;
            let base_vertex = vertices.len() as u32;

            // Extract vertices from the GlPoly
            for v in 0..numverts {
                let vert_ptr = glpoly_vert_ptr(poly, v);
                let bsp_vert = BspVertex::new(
                    [*vert_ptr, *vert_ptr.add(1), *vert_ptr.add(2)],
                    [*vert_ptr.add(3), *vert_ptr.add(4)],
                    [*vert_ptr.add(5), *vert_ptr.add(6)],
                );
                vertices.push(bsp_vert);
            }

            // Triangulate as a fan: (0,1,2), (0,2,3), (0,3,4), ...
            for v in 2..numverts {
                indices.push(base_vertex);
                indices.push(base_vertex + (v - 1) as u32);
                indices.push(base_vertex + v as u32);
            }

            let index_count = indices.len() as u32 - first_index;

            surface_infos.push(SurfaceDrawInfo {
                first_index,
                index_count,
                texture_id,
                lightmap_id,
                flags,
            });

            poly = (*poly).next;
        }
    }

    if !vertices.is_empty() {
        modern.bsp_geometry_mut().build(&vertices, &indices, surface_infos);
        crate::vk_rmain::vid_printf(
            myq2_common::q_shared::PRINT_ALL,
            &format!(
                "Modern BSP: {} vertices, {} indices, {} surfaces\n",
                vertices.len(), indices.len(),
                modern.bsp_geometry().surfaces().len()
            ),
        );
    }
}

/// Register a model by name.
///
/// # Safety
/// Accesses global model state.
pub unsafe fn r_register_model(name: &str) -> *mut Model {
    let model = mod_for_name(name, false);
    if model.is_null() {
        return std::ptr::null_mut();
    }

    (*model).registration_sequence = registration_sequence;

    // register any images used by the models
    match (*model).r#type {
        ModType::Sprite => {
            let sprout = (*model).extradata as *const DSprite;
            let sprite_header_size = std::mem::size_of::<DSprite>();
            let frame_size = std::mem::size_of::<DSprFrame>();
            for i in 0..(*sprout).numframes as usize {
                let frame = (sprout as *const u8).add(sprite_header_size + i * frame_size) as *const DSprFrame;
                (*model).skins[i] = vk_find_image_c((*frame).name.as_ptr(), ImageType::Sprite);
            }
        }
        ModType::Alias => {
            let pheader = (*model).extradata as *const DMdl;
            for i in 0..(*pheader).num_skins as usize {
                let skin_name_ptr = (pheader as *const u8).add((*pheader).ofs_skins as usize + i * MAX_SKINNAME);
                (*model).skins[i] = vk_find_image_c(skin_name_ptr, ImageType::Skin);
            }
            (*model).numframes = (*pheader).num_frames;
        }
        ModType::Brush => {
            for i in 0..(*model).numtexinfo {
                let ti = &mut *(*model).texinfo.add(i as usize);
                if !ti.image.is_null() {
                    (*ti.image).registration_sequence = registration_sequence;
                }
            }
        }
        _ => {}
    }

    model
}

/// End model registration: free unused models and images.
///
/// # Safety
/// Accesses global model state.
pub unsafe fn r_end_registration() {
    for i in 0..mod_numknown as usize {
        if model_name_is_empty(&mod_known[i].name) {
            continue;
        }
        if mod_known[i].registration_sequence != registration_sequence {
            mod_free(&mut mod_known[i]);
        }
    }

    vk_free_unused_images();
}

// =============================================================
//  Cleanup
// =============================================================

/// Free a single model.
///
/// # Safety
/// Mutates the model struct.
pub unsafe fn mod_free(model: *mut Model) {
    hunk_free((*model).extradata);
    // SAFETY: zeroing a repr(C) struct with raw pointers is valid (null pointers).
    std::ptr::write_bytes(model as *mut u8, 0, std::mem::size_of::<Model>());
}

/// Free all models.
///
/// # Safety
/// Mutates global model array.
pub unsafe fn mod_free_all() {
    for i in 0..mod_numknown as usize {
        if mod_known[i].extradatasize != 0 {
            mod_free(&mut mod_known[i]);
        }
    }
}

// =============================================================
//  Tests
// =============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------
    //  radius_from_bounds
    // ---------------------------------------------------------

    #[test]
    fn test_radius_from_bounds_symmetric() {
        let mins = [-10.0, -10.0, -10.0];
        let maxs = [10.0, 10.0, 10.0];
        let r = radius_from_bounds(&mins, &maxs);
        let expected = (10.0f32 * 10.0 + 10.0 * 10.0 + 10.0 * 10.0).sqrt();
        assert!((r - expected).abs() < 1e-4, "expected ~{}, got {}", expected, r);
    }

    #[test]
    fn test_radius_from_bounds_asymmetric() {
        // mins further from origin on X, maxs further on Y/Z
        let mins = [-20.0, -5.0, -3.0];
        let maxs = [10.0, 15.0, 8.0];
        // corner should be [20, 15, 8]
        let expected = (20.0f32 * 20.0 + 15.0 * 15.0 + 8.0 * 8.0).sqrt();
        let r = radius_from_bounds(&mins, &maxs);
        assert!((r - expected).abs() < 1e-4, "expected ~{}, got {}", expected, r);
    }

    #[test]
    fn test_radius_from_bounds_zero() {
        let mins = [0.0, 0.0, 0.0];
        let maxs = [0.0, 0.0, 0.0];
        let r = radius_from_bounds(&mins, &maxs);
        assert!((r - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_radius_from_bounds_negative_only() {
        // Both mins and maxs negative
        let mins = [-30.0, -20.0, -10.0];
        let maxs = [-5.0, -2.0, -1.0];
        // corner should pick abs-largest: [30, 20, 10]
        let expected = (30.0f32 * 30.0 + 20.0 * 20.0 + 10.0 * 10.0).sqrt();
        let r = radius_from_bounds(&mins, &maxs);
        assert!((r - expected).abs() < 1e-4, "expected ~{}, got {}", expected, r);
    }

    #[test]
    fn test_radius_from_bounds_single_axis() {
        let mins = [0.0, 0.0, -100.0];
        let maxs = [0.0, 0.0, 50.0];
        // corner: [0, 0, 100]
        let r = radius_from_bounds(&mins, &maxs);
        assert!((r - 100.0).abs() < 1e-4);
    }

    // ---------------------------------------------------------
    //  model_name_str
    // ---------------------------------------------------------

    #[test]
    fn test_model_name_str_basic() {
        let mut name = [0u8; MAX_QPATH];
        let src = b"models/weapon.md2";
        name[..src.len()].copy_from_slice(src);
        assert_eq!(model_name_str(&name), "models/weapon.md2");
    }

    #[test]
    fn test_model_name_str_empty() {
        let name = [0u8; MAX_QPATH];
        assert_eq!(model_name_str(&name), "");
    }

    #[test]
    fn test_model_name_str_full() {
        // Fill to exactly MAX_QPATH-1 non-null bytes + null terminator
        let mut name = [b'A'; MAX_QPATH];
        name[MAX_QPATH - 1] = 0;
        let s = model_name_str(&name);
        assert_eq!(s.len(), MAX_QPATH - 1);
    }

    #[test]
    fn test_model_name_str_no_null_terminator() {
        // If the entire buffer is non-null, we should still get a valid string
        let name = [b'z'; MAX_QPATH];
        let s = model_name_str(&name);
        assert_eq!(s.len(), MAX_QPATH);
    }

    // ---------------------------------------------------------
    //  set_model_name
    // ---------------------------------------------------------

    #[test]
    fn test_set_model_name_basic() {
        let mut name = [0xFFu8; MAX_QPATH];
        set_model_name(&mut name, "hello");
        assert_eq!(model_name_str(&name), "hello");
        // Verify null termination
        assert_eq!(name[5], 0);
        // Verify remaining bytes are zeroed
        for &b in &name[6..] {
            assert_eq!(b, 0);
        }
    }

    #[test]
    fn test_set_model_name_empty() {
        let mut name = [0xFFu8; MAX_QPATH];
        set_model_name(&mut name, "");
        assert_eq!(model_name_str(&name), "");
        assert_eq!(name[0], 0);
    }

    #[test]
    fn test_set_model_name_max_length() {
        // String of exactly MAX_QPATH-1 chars should fit
        let long_str: String = "A".repeat(MAX_QPATH - 1);
        let mut name = [0u8; MAX_QPATH];
        set_model_name(&mut name, &long_str);
        let s = model_name_str(&name);
        assert_eq!(s.len(), MAX_QPATH - 1);
        assert_eq!(name[MAX_QPATH - 1], 0);
    }

    #[test]
    fn test_set_model_name_too_long_is_truncated() {
        // String longer than MAX_QPATH-1 should be truncated
        let long_str: String = "B".repeat(MAX_QPATH + 10);
        let mut name = [0u8; MAX_QPATH];
        set_model_name(&mut name, &long_str);
        let s = model_name_str(&name);
        assert_eq!(s.len(), MAX_QPATH - 1);
    }

    // ---------------------------------------------------------
    //  model_name_is_empty
    // ---------------------------------------------------------

    #[test]
    fn test_model_name_is_empty_true() {
        let name = [0u8; MAX_QPATH];
        assert!(model_name_is_empty(&name));
    }

    #[test]
    fn test_model_name_is_empty_false() {
        let mut name = [0u8; MAX_QPATH];
        name[0] = b'x';
        assert!(!model_name_is_empty(&name));
    }

    // ---------------------------------------------------------
    //  model_names_match
    // ---------------------------------------------------------

    #[test]
    fn test_model_names_match_identical() {
        let mut a = [0u8; MAX_QPATH];
        let mut b = [0u8; MAX_QPATH];
        set_model_name(&mut a, "maps/q2dm1.bsp");
        set_model_name(&mut b, "maps/q2dm1.bsp");
        assert!(model_names_match(&a, &b));
    }

    #[test]
    fn test_model_names_match_different() {
        let mut a = [0u8; MAX_QPATH];
        let mut b = [0u8; MAX_QPATH];
        set_model_name(&mut a, "maps/q2dm1.bsp");
        set_model_name(&mut b, "maps/q2dm2.bsp");
        assert!(!model_names_match(&a, &b));
    }

    // ---------------------------------------------------------
    //  model_name_matches_str
    // ---------------------------------------------------------

    #[test]
    fn test_model_name_matches_str_match() {
        let mut name = [0u8; MAX_QPATH];
        set_model_name(&mut name, "models/items/armor/tris.md2");
        assert!(model_name_matches_str(&name, "models/items/armor/tris.md2"));
    }

    #[test]
    fn test_model_name_matches_str_no_match() {
        let mut name = [0u8; MAX_QPATH];
        set_model_name(&mut name, "models/items/armor/tris.md2");
        assert!(!model_name_matches_str(&name, "models/items/armor/skin.pcx"));
    }

    #[test]
    fn test_model_name_matches_str_empty_both() {
        let name = [0u8; MAX_QPATH];
        assert!(model_name_matches_str(&name, ""));
    }

    // ---------------------------------------------------------
    //  radius_from_bounds edge cases (Quake 2 model bounding)
    // ---------------------------------------------------------

    #[test]
    fn test_radius_from_bounds_typical_player_model() {
        // Typical player-like bounding box
        let mins = [-16.0, -16.0, -24.0];
        let maxs = [16.0, 16.0, 32.0];
        // Corner: [16, 16, 32]
        let expected = (16.0f32 * 16.0 + 16.0 * 16.0 + 32.0 * 32.0).sqrt();
        let r = radius_from_bounds(&mins, &maxs);
        assert!((r - expected).abs() < 1e-4);
    }

    #[test]
    fn test_radius_from_bounds_alias_default() {
        // Default alias model bounds: [-32, -32, -32] to [32, 32, 32]
        let mins = [-32.0, -32.0, -32.0];
        let maxs = [32.0, 32.0, 32.0];
        let expected = (32.0f32 * 32.0 * 3.0).sqrt();
        let r = radius_from_bounds(&mins, &maxs);
        assert!((r - expected).abs() < 1e-4);
    }
}
