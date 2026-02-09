// Copyright (C) 1997-2001 Id Software, Inc.
// GPL-2.0-or-later
//
// vk_model.h -> vk_model_types.rs
// d*_t structures are on-disk representations
// m*_t structures are in-memory

use myq2_common::q_shared::{CPlane, Vec3, MAX_QPATH};
use myq2_common::qfiles::{MAXLIGHTMAPS, MAX_MD2SKINS};

// Image type — canonical definition in myq2_common::q_shared
pub use myq2_common::q_shared::ImageType;

#[repr(C)]
pub struct Image {
    pub name: [u8; MAX_QPATH], // game path, including extension
    pub r#type: ImageType,
    pub width: i32,
    pub height: i32,           // source image
    pub upload_width: i32,
    pub upload_height: i32,    // after power of two and picmip
    pub registration_sequence: i32, // 0 = free
    pub texturechain: *mut MSurface, // for sort-by-texture world drawing
    pub texnum: i32,           // gl texture binding
    pub sl: f32,
    pub tl: f32,
    pub sh: f32,
    pub th: f32, // 0,0 - 1,1 unless part of the scrap
    pub scrap: i32,     // qboolean
    pub has_alpha: i32, // qboolean
    pub paletted: i32,  // qboolean
}

impl Clone for Image {
    fn clone(&self) -> Self {
        // SAFETY: Image is repr(C) with all POD fields + a raw pointer
        unsafe { std::ptr::read(self) }
    }
}

impl std::fmt::Debug for Image {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Image")
            .field("name", &std::str::from_utf8(&self.name).unwrap_or("<invalid>"))
            .field("texnum", &self.texnum)
            .finish()
    }
}

impl Default for Image {
    fn default() -> Self {
        // SAFETY: All-zero is valid (null pointer, zero ints/floats).
        unsafe { std::mem::zeroed() }
    }
}

// ============================================================================
// BRUSH MODELS
// ============================================================================

// --- Constants ---

pub const SIDE_FRONT: i32 = 0;
pub const SIDE_BACK: i32 = 1;
pub const SIDE_ON: i32 = 2;

pub const SURF_PLANEBACK: i32 = 2;
pub const SURF_DRAWSKY: i32 = 4;
pub const SURF_DRAWTURB: i32 = 0x10;
pub const SURF_DRAWBACKGROUND: i32 = 0x40;
pub const SURF_UNDERWATER: i32 = 0x80;

pub const VERTEXSIZE: usize = 7;

// --- In-memory representation ---

/// !!! if this is changed, it must be changed in asm_draw.h too !!!
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MVertex {
    pub position: Vec3,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MModel {
    pub mins: Vec3,
    pub maxs: Vec3,
    pub origin: Vec3, // for sounds or lights
    pub radius: f32,
    pub headnode: i32,
    pub visleafs: i32, // not including the solid leaf 0
    pub firstface: i32,
    pub numfaces: i32,
}

/// !!! if this is changed, it must be changed in asm_draw.h too !!!
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MEdge {
    pub v: [u16; 2],
    pub cachededgeoffset: u32,
}

#[repr(C)]
pub struct MTexInfo {
    pub vecs: [[f32; 4]; 2],
    pub flags: i32,
    pub numframes: i32,
    pub next: *mut MTexInfo, // animation chain
    pub image: *mut Image,
}

#[repr(C)]
pub struct GlPoly {
    pub next: *mut GlPoly,
    pub chain: *mut GlPoly,
    pub numverts: i32,
    pub flags: i32, // for SURF_UNDERWATER (not needed anymore?)
    pub caustics_chain: *mut GlPoly, // next caustic poly in chain
    pub verts: [[f32; VERTEXSIZE]; 4], // variable sized (xyz s1t1 s2t2)
}

#[repr(C)]
pub struct MSurface {
    pub visframe: i32, // should be drawn when node is crossed

    pub plane: *mut CPlane,
    pub flags: i32,

    pub firstedge: i32, // look up in model->surfedges[], negative numbers
    pub numedges: i32,  // are backwards edges

    pub texturemins: [i16; 2],
    pub extents: [i16; 2],

    pub light_s: i32,
    pub light_t: i32, // gl lightmap coordinates
    pub dlight_s: i32,
    pub dlight_t: i32, // gl lightmap coordinates for dynamic lightmaps

    pub polys: *mut GlPoly, // multiple if warped
    pub texturechain: *mut MSurface,
    pub lightmapchain: *mut MSurface,

    pub texinfo: *mut MTexInfo,

    // lighting info
    pub dlightframe: i32,
    pub dlightbits: i32,

    pub lightmaptexturenum: i32,
    pub styles: [u8; MAXLIGHTMAPS],
    pub cached_light: [f32; MAXLIGHTMAPS], // values currently used in lightmap
    pub samples: *mut u8,                  // [numstyles*surfsize]
    pub stains: *mut u8,
}

#[repr(C)]
pub struct MNode {
    // common with leaf
    pub contents: i32,  // -1, to differentiate from leafs
    pub visframe: i32,  // node needs to be traversed if current
    pub minmaxs: [f32; 6], // for bounding box culling
    pub parent: *mut MNode,

    // node specific
    pub plane: *mut CPlane,
    pub children: [*mut MNode; 2],

    pub firstsurface: u16,
    pub numsurfaces: u16,
}

#[repr(C)]
pub struct MLeaf {
    // common with node
    pub contents: i32,  // will be a negative contents number
    pub visframe: i32,  // node needs to be traversed if current
    pub minmaxs: [f32; 6], // for bounding box culling
    pub parent: *mut MNode,

    // leaf specific
    pub cluster: i32,
    pub area: i32,

    pub firstmarksurface: *mut *mut MSurface,
    pub nummarksurfaces: i32,
}

// ===================================================================
// Whole model
// ===================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ModType {
    Bad = 0,
    Brush = 1,
    Sprite = 2,
    Alias = 3,
}

/// On-disk visibility data header (variable length).
#[repr(C)]
pub struct DvisT {
    pub numclusters: i32,
    pub bitofs: [[i32; 2]; 1], // variable sized: [numclusters][2]
}

#[repr(C)]
pub struct Model {
    pub name: [u8; MAX_QPATH],

    pub registration_sequence: i32,

    pub r#type: ModType,
    pub numframes: i32,

    pub flags: i32,

    // volume occupied by the model graphics
    pub mins: Vec3,
    pub maxs: Vec3,
    pub radius: f32,

    // solid volume for clipping
    pub clipbox: i32, // qboolean
    pub clipmins: Vec3,
    pub clipmaxs: Vec3,

    // brush model
    pub firstmodelsurface: i32,
    pub nummodelsurfaces: i32,
    pub lightmap: i32, // only for submodels

    pub numsubmodels: i32,
    pub submodels: *mut MModel,

    pub numplanes: i32,
    pub planes: *mut CPlane,

    pub numleafs: i32, // number of visible leafs, not counting 0
    pub leafs: *mut MLeaf,

    pub numvertexes: i32,
    pub vertexes: *mut MVertex,

    pub numedges: i32,
    pub edges: *mut MEdge,

    pub numnodes: i32,
    pub firstnode: i32,
    pub nodes: *mut MNode,

    pub numtexinfo: i32,
    pub texinfo: *mut MTexInfo,

    pub numsurfaces: i32,
    pub surfaces: *mut MSurface,

    pub numsurfedges: i32,
    pub surfedges: *mut i32,

    pub nummarksurfaces: i32,
    pub marksurfaces: *mut *mut MSurface,

    pub vis: *mut DvisT,

    pub lightdata: *mut u8,

    // for alias models and skins
    pub skins: [*mut Image; MAX_MD2SKINS],

    pub extradatasize: i32,
    pub extradata: *mut u8,
}

// Default implementations for pointer-heavy structs

impl Default for Model {
    fn default() -> Self {
        // SAFETY: Model is repr(C) with all-zero being a valid state
        // (null pointers, zero integers).
        unsafe { std::mem::zeroed() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{size_of, align_of};

    // ============================================================
    // Constants
    // ============================================================

    #[test]
    fn test_side_constants() {
        assert_eq!(SIDE_FRONT, 0);
        assert_eq!(SIDE_BACK, 1);
        assert_eq!(SIDE_ON, 2);
    }

    #[test]
    fn test_surf_flag_constants() {
        assert_eq!(SURF_PLANEBACK, 2);
        assert_eq!(SURF_DRAWSKY, 4);
        assert_eq!(SURF_DRAWTURB, 0x10);
        assert_eq!(SURF_DRAWBACKGROUND, 0x40);
        assert_eq!(SURF_UNDERWATER, 0x80);
    }

    #[test]
    fn test_surf_flags_are_distinct_bits() {
        // Each flag should be a distinct bit, no overlap
        let flags = [SURF_PLANEBACK, SURF_DRAWSKY, SURF_DRAWTURB, SURF_DRAWBACKGROUND, SURF_UNDERWATER];
        for i in 0..flags.len() {
            for j in (i + 1)..flags.len() {
                assert_eq!(flags[i] & flags[j], 0,
                    "SURF flags {:x} and {:x} overlap", flags[i], flags[j]);
            }
        }
    }

    #[test]
    fn test_vertexsize() {
        // VERTEXSIZE = 7: xyz(3) + s1t1(2) + s2t2(2)
        assert_eq!(VERTEXSIZE, 7);
    }

    // ============================================================
    // Enum values
    // ============================================================

    #[test]
    fn test_mod_type_values() {
        assert_eq!(ModType::Bad as i32, 0);
        assert_eq!(ModType::Brush as i32, 1);
        assert_eq!(ModType::Sprite as i32, 2);
        assert_eq!(ModType::Alias as i32, 3);
    }

    #[test]
    fn test_mod_type_equality() {
        assert_eq!(ModType::Bad, ModType::Bad);
        assert_ne!(ModType::Bad, ModType::Brush);
        assert_ne!(ModType::Brush, ModType::Alias);
    }

    // ============================================================
    // Struct sizes — verify #[repr(C)] structs have expected sizes
    // ============================================================

    #[test]
    fn test_mvertex_size() {
        // MVertex = Vec3 (3 floats) = 12 bytes
        assert_eq!(size_of::<MVertex>(), 12);
    }

    #[test]
    fn test_mvertex_alignment() {
        // Should be 4-byte aligned (f32 alignment)
        assert_eq!(align_of::<MVertex>(), 4);
    }

    #[test]
    fn test_medge_size() {
        // MEdge = [u16; 2] (4 bytes) + u32 (4 bytes) = 8 bytes
        assert_eq!(size_of::<MEdge>(), 8);
    }

    #[test]
    fn test_mmodel_size() {
        // MModel = 3 * Vec3 (36 bytes) + f32 (4) + 4 * i32 (16) = 56 bytes
        assert_eq!(size_of::<MModel>(), 56);
    }

    // ============================================================
    // Struct default values
    // ============================================================

    #[test]
    fn test_image_default_is_zeroed() {
        let img = Image::default();
        assert_eq!(img.texnum, 0);
        assert_eq!(img.width, 0);
        assert_eq!(img.height, 0);
        assert_eq!(img.upload_width, 0);
        assert_eq!(img.upload_height, 0);
        assert_eq!(img.registration_sequence, 0);
        assert_eq!(img.has_alpha, 0);
        assert_eq!(img.paletted, 0);
        assert_eq!(img.scrap, 0);
        assert_eq!(img.sl, 0.0);
        assert_eq!(img.tl, 0.0);
        assert_eq!(img.sh, 0.0);
        assert_eq!(img.th, 0.0);
        // Name should be all zeros
        assert!(img.name.iter().all(|&b| b == 0));
    }

    #[test]
    fn test_image_clone() {
        let mut img = Image::default();
        img.texnum = 42;
        img.width = 256;
        img.height = 128;
        img.name[0] = b't';
        img.name[1] = b'e';
        img.name[2] = b's';
        img.name[3] = b't';

        let cloned = img.clone();
        assert_eq!(cloned.texnum, 42);
        assert_eq!(cloned.width, 256);
        assert_eq!(cloned.height, 128);
        assert_eq!(cloned.name[0], b't');
        assert_eq!(cloned.name[3], b't');
    }

    #[test]
    fn test_model_default_is_zeroed() {
        let model = Model::default();
        assert_eq!(model.registration_sequence, 0);
        assert_eq!(model.r#type, ModType::Bad); // ModType::Bad = 0
        assert_eq!(model.numframes, 0);
        assert_eq!(model.flags, 0);
        assert_eq!(model.radius, 0.0);
        assert_eq!(model.mins, [0.0; 3]);
        assert_eq!(model.maxs, [0.0; 3]);
        assert!(model.name.iter().all(|&b| b == 0));
    }

    // ============================================================
    // Image type
    // ============================================================

    #[test]
    fn test_image_name_capacity() {
        // Image.name is [u8; MAX_QPATH], MAX_QPATH = 64
        let img = Image::default();
        assert_eq!(img.name.len(), MAX_QPATH);
        assert_eq!(img.name.len(), 64);
    }

    #[test]
    fn test_model_name_capacity() {
        let model = Model::default();
        assert_eq!(model.name.len(), MAX_QPATH);
    }

    // ============================================================
    // Pointer safety in defaults
    // ============================================================

    #[test]
    fn test_image_default_texturechain_is_null() {
        let img = Image::default();
        assert!(img.texturechain.is_null());
    }

    #[test]
    fn test_model_default_pointers_are_null() {
        let model = Model::default();
        assert!(model.submodels.is_null());
        assert!(model.planes.is_null());
        assert!(model.leafs.is_null());
        assert!(model.vertexes.is_null());
        assert!(model.edges.is_null());
        assert!(model.nodes.is_null());
        assert!(model.texinfo.is_null());
        assert!(model.surfaces.is_null());
        assert!(model.surfedges.is_null());
        assert!(model.marksurfaces.is_null());
        assert!(model.vis.is_null());
        assert!(model.lightdata.is_null());
        assert!(model.extradata.is_null());
    }

    #[test]
    fn test_model_skins_default_null() {
        let model = Model::default();
        for skin in &model.skins {
            assert!(skin.is_null());
        }
        assert_eq!(model.skins.len(), MAX_MD2SKINS);
    }

    // ============================================================
    // GlPoly vertex size
    // ============================================================

    #[test]
    fn test_glpoly_verts_entry_size() {
        // Each vertex entry is [f32; VERTEXSIZE] = [f32; 7] = 28 bytes
        assert_eq!(size_of::<[f32; VERTEXSIZE]>(), 28);
    }

    // ============================================================
    // MSurface lightmap arrays
    // ============================================================

    #[test]
    fn test_msurface_styles_size() {
        // styles array has MAXLIGHTMAPS entries (4)
        assert_eq!(MAXLIGHTMAPS, 4);
    }

    #[test]
    fn test_msurface_cached_light_size() {
        // cached_light array has MAXLIGHTMAPS entries (4)
        assert_eq!(MAXLIGHTMAPS, 4);
    }
}
