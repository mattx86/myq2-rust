// cmodel.rs — Collision model loading and tracing
// Converted from: myq2-original/qcommon/cmodel.c

use crate::q_shared::{
    angle_vectors, box_on_plane_side, dot_product, vector_subtract, CModel, CPlane,
    MapSurface, Trace, Vec3,
};
use crate::qfiles::{
    DAreaPortal, Lump,
    BSPVERSION, DVIS_PHS, DVIS_PVS, HEADER_LUMPS, LUMP_AREAS, LUMP_AREAPORTALS,
    LUMP_BRUSHES, LUMP_BRUSHSIDES, LUMP_ENTITIES, LUMP_LEAFBRUSHES, LUMP_LEAFS, LUMP_MODELS,
    LUMP_NODES, LUMP_PLANES, LUMP_TEXINFO, LUMP_VISIBILITY, MAX_MAP_AREAPORTALS, MAX_MAP_AREAS,
    MAX_MAP_BRUSHES, MAX_MAP_BRUSHSIDES, MAX_MAP_ENTSTRING, MAX_MAP_LEAFBRUSHES, MAX_MAP_LEAFS,
    MAX_MAP_MODELS, MAX_MAP_NODES, MAX_MAP_PLANES, MAX_MAP_TEXINFO, MAX_MAP_VISIBILITY,
};
use crate::q_shared::CONTENTS_MONSTER;
use crate::q_shared::CONTENTS_SOLID;
use rayon::prelude::*;


// ============================================================
// Internal structures (not in the BSP file, but used at runtime)
// ============================================================

#[derive(Debug, Clone)]
#[derive(Default)]
pub struct CNode {
    pub plane_idx: usize,
    pub children: [i32; 2], // negative numbers are leafs
}


#[derive(Debug, Clone)]
pub struct CBrushSide {
    pub plane_idx: usize,
    pub surface_idx: usize, // index into map_surfaces, usize::MAX = nullsurface
}

impl Default for CBrushSide {
    fn default() -> Self {
        Self {
            plane_idx: 0,
            surface_idx: usize::MAX,
        }
    }
}

#[derive(Debug, Clone)]
#[derive(Default)]
pub struct CLeaf {
    pub contents: i32,
    pub cluster: i32,
    pub area: i32,
    pub firstleafbrush: u16,
    pub numleafbrushes: u16,
}


#[derive(Debug, Clone)]
#[derive(Default)]
pub struct CBrush {
    pub contents: i32,
    pub numsides: i32,
    pub firstbrushside: i32,
    pub checkcount: i32,
}


#[derive(Debug, Clone)]
#[derive(Default)]
pub struct CArea {
    pub numareaportals: i32,
    pub firstareaportal: i32,
    pub floodnum: i32,
    pub floodvalid: i32,
}


// ============================================================
// Visibility data helper
// ============================================================

#[derive(Debug, Clone)]
#[derive(Default)]
pub struct VisData {
    pub numclusters: i32,
    /// bitofs[cluster][0] = PVS offset, bitofs[cluster][1] = PHS offset
    pub bitofs: Vec<[i32; 2]>,
}


// ============================================================
// Constants
// ============================================================

const DIST_EPSILON: f32 = 0.03125;

// ============================================================
// Context: holds all loaded map state
// ============================================================

pub struct CModelContext {
    // Map name
    pub map_name: String,

    // Counters / performance stats
    pub checkcount: i32,
    pub c_pointcontents: i32,
    pub c_traces: i32,
    pub c_brush_traces: i32,

    // Map data arrays
    pub map_brushsides: Vec<CBrushSide>,
    pub map_surfaces: Vec<MapSurface>,
    pub map_planes: Vec<CPlane>,
    pub map_nodes: Vec<CNode>,
    pub map_leafs: Vec<CLeaf>,
    pub map_leafbrushes: Vec<u16>,
    pub map_cmodels: Vec<CModel>,
    pub map_brushes: Vec<CBrush>,
    pub map_visibility: Vec<u8>,
    pub vis_data: VisData,
    pub map_entitystring: String,
    pub map_areas: Vec<CArea>,
    pub map_areaportals: Vec<DAreaPortal>,

    // Counts
    pub numbrushsides: usize,
    pub numtexinfo: usize,
    pub numplanes: usize,
    pub numnodes: usize,
    pub numleafs: usize,
    pub numleafbrushes: usize,
    pub numcmodels: usize,
    pub numbrushes: usize,
    pub numvisibility: usize,
    pub numentitychars: usize,
    pub numareas: usize,
    pub numareaportals: usize,
    pub numclusters: usize,

    // Special leaf indices
    pub emptyleaf: i32,
    pub solidleaf: i32,

    // Null surface (used for brush sides with no texture)
    pub nullsurface: MapSurface,

    // Box hull
    pub box_headnode: usize,
    pub box_planes_start: usize, // index into map_planes where box planes begin
    pub box_brush_idx: usize,
    pub box_leaf_idx: usize,

    // Area portals
    pub floodvalid: i32,
    pub portalopen: Vec<bool>,

    // map_noareas cvar equivalent
    pub map_noareas: bool,

    // Last checksum for reload detection
    pub last_checksum: u32,

    // PVS/PHS row buffers
    pub pvsrow: Vec<u8>,
    pub phsrow: Vec<u8>,
}

impl CModelContext {
    pub fn new() -> Self {
        Self {
            map_name: String::new(),
            checkcount: 0,
            c_pointcontents: 0,
            c_traces: 0,
            c_brush_traces: 0,

            map_brushsides: Vec::new(),
            map_surfaces: Vec::new(),
            map_planes: Vec::new(),
            map_nodes: Vec::new(),
            map_leafs: vec![CLeaf::default()], // allow leaf funcs to be called without a map
            map_leafbrushes: Vec::new(),
            map_cmodels: Vec::new(),
            map_brushes: Vec::new(),
            map_visibility: Vec::new(),
            vis_data: VisData::default(),
            map_entitystring: String::new(),
            map_areas: Vec::new(),
            map_areaportals: Vec::new(),

            numbrushsides: 0,
            numtexinfo: 0,
            numplanes: 0,
            numnodes: 0,
            numleafs: 1,
            numleafbrushes: 0,
            numcmodels: 0,
            numbrushes: 0,
            numvisibility: 0,
            numentitychars: 0,
            numareas: 1,
            numareaportals: 0,
            numclusters: 1,

            emptyleaf: -1,
            solidleaf: 0,

            nullsurface: MapSurface::default(),

            box_headnode: 0,
            box_planes_start: 0,
            box_brush_idx: 0,
            box_leaf_idx: 0,

            floodvalid: 0,
            portalopen: vec![false; MAX_MAP_AREAPORTALS],

            map_noareas: false,
            last_checksum: 0,

            pvsrow: vec![0u8; MAX_MAP_LEAFS / 8],
            phsrow: vec![0u8; MAX_MAP_LEAFS / 8],
        }
    }

    // ============================================================
    // BSP byte helpers
    // ============================================================

    fn read_i32_le(data: &[u8], offset: usize) -> i32 {
        i32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ])
    }

    fn read_u16_le(data: &[u8], offset: usize) -> u16 {
        u16::from_le_bytes([data[offset], data[offset + 1]])
    }

    fn read_i16_le(data: &[u8], offset: usize) -> i16 {
        i16::from_le_bytes([data[offset], data[offset + 1]])
    }

    fn read_f32_le(data: &[u8], offset: usize) -> f32 {
        f32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ])
    }

    // ============================================================
    // Lump loaders
    // ============================================================

    fn load_submodels(&mut self, data: &[u8], lump: &Lump) {
        let ofs = lump.fileofs as usize;
        let len = lump.filelen as usize;
        // sizeof(DModel) = 9 * 4 = 36 in C, but use Rust struct size
        let stride = 48; // 3*4 + 3*4 + 3*4 + 4 + 4 + 4 = 48
        if !len.is_multiple_of(stride) {
            panic!("MOD_LoadBmodel: funny lump size (submodels)");
        }
        let count = len / stride;
        if count < 1 {
            panic!("Map with no models");
        }
        if count > MAX_MAP_MODELS {
            panic!("Map has too many models");
        }

        self.numcmodels = count;
        self.map_cmodels.clear();
        self.map_cmodels.reserve(count);

        // Use parallel parsing for large model counts
        if count >= Self::PARALLEL_LUMP_THRESHOLD {
            let cmodels: Vec<CModel> = (0..count)
                .into_par_iter()
                .map(|i| {
                    let base = ofs + i * stride;
                    let mut cm = CModel::default();
                    for j in 0..3 {
                        cm.mins[j] = Self::read_f32_le(data, base + j * 4) - 1.0;
                        cm.maxs[j] = Self::read_f32_le(data, base + 12 + j * 4) + 1.0;
                        cm.origin[j] = Self::read_f32_le(data, base + 24 + j * 4);
                    }
                    cm.headnode = Self::read_i32_le(data, base + 36);
                    cm
                })
                .collect();
            self.map_cmodels = cmodels;
        } else {
            for i in 0..count {
                let base = ofs + i * stride;
                let mut cm = CModel::default();
                for j in 0..3 {
                    cm.mins[j] = Self::read_f32_le(data, base + j * 4) - 1.0;
                    cm.maxs[j] = Self::read_f32_le(data, base + 12 + j * 4) + 1.0;
                    cm.origin[j] = Self::read_f32_le(data, base + 24 + j * 4);
                }
                cm.headnode = Self::read_i32_le(data, base + 36);
                self.map_cmodels.push(cm);
            }
        }
    }

    /// Parallel threshold for lump parsing - below this count, sequential is faster
    const PARALLEL_LUMP_THRESHOLD: usize = 64;

    fn load_surfaces(&mut self, data: &[u8], lump: &Lump) {
        let ofs = lump.fileofs as usize;
        let len = lump.filelen as usize;
        // sizeof(texinfo_t) = 76: vecs[2][4]*4=32, flags=4, value=4, texture[32]=32, nexttexinfo=4
        let stride = 76;
        if !len.is_multiple_of(stride) {
            panic!("MOD_LoadBmodel: funny lump size (surfaces)");
        }
        let count = len / stride;
        if count < 1 {
            panic!("Map with no surfaces");
        }
        if count > MAX_MAP_TEXINFO {
            panic!("Map has too many surfaces");
        }

        self.numtexinfo = count;
        self.map_surfaces.clear();
        self.map_surfaces.reserve(count);

        // Use parallel parsing for large surface counts
        if count >= Self::PARALLEL_LUMP_THRESHOLD {
            let surfaces: Vec<MapSurface> = (0..count)
                .into_par_iter()
                .map(|i| {
                    let base = ofs + i * stride;
                    let mut surf = MapSurface::default();

                    let tex_offset = base + 40;
                    let tex_bytes = &data[tex_offset..tex_offset + 32];
                    let name_len = tex_bytes.iter().position(|&b| b == 0).unwrap_or(32);
                    let copy_len_c = name_len.min(15);
                    surf.c.name[..copy_len_c].copy_from_slice(&tex_bytes[..copy_len_c]);
                    let copy_len_r = name_len.min(31);
                    surf.rname[..copy_len_r].copy_from_slice(&tex_bytes[..copy_len_r]);

                    surf.c.flags = Self::read_i32_le(data, base + 32);
                    surf.c.value = Self::read_i32_le(data, base + 36);
                    surf
                })
                .collect();
            self.map_surfaces = surfaces;
        } else {
            for i in 0..count {
                let base = ofs + i * stride;
                let mut surf = MapSurface::default();

                // texture name is at offset 40 (after vecs[2][4]*4=32 + flags 4 + value 4)
                let tex_offset = base + 40;
                let tex_bytes = &data[tex_offset..tex_offset + 32];
                // Copy into c.name (16 bytes) and rname (32 bytes)
                let name_len = tex_bytes.iter().position(|&b| b == 0).unwrap_or(32);
                let copy_len_c = name_len.min(15);
                surf.c.name[..copy_len_c].copy_from_slice(&tex_bytes[..copy_len_c]);
                let copy_len_r = name_len.min(31);
                surf.rname[..copy_len_r].copy_from_slice(&tex_bytes[..copy_len_r]);

                // flags at offset 32, value at offset 36
                surf.c.flags = Self::read_i32_le(data, base + 32);
                surf.c.value = Self::read_i32_le(data, base + 36);

                self.map_surfaces.push(surf);
            }
        }
    }

    fn load_nodes(&mut self, data: &[u8], lump: &Lump) {
        let ofs = lump.fileofs as usize;
        let len = lump.filelen as usize;
        // sizeof(dnode_t) = 4 + 2*4 + 3*2 + 3*2 + 2 + 2 = 28
        let stride = 28;
        if !len.is_multiple_of(stride) {
            panic!("MOD_LoadBmodel: funny lump size (nodes)");
        }
        let count = len / stride;
        if count < 1 {
            panic!("Map has no nodes");
        }
        if count > MAX_MAP_NODES {
            panic!("Map has too many nodes");
        }

        self.numnodes = count;
        self.map_nodes.clear();
        self.map_nodes.reserve(count + 6); // extra for box hull

        // Use parallel parsing for large node counts
        if count >= Self::PARALLEL_LUMP_THRESHOLD {
            let nodes: Vec<CNode> = (0..count)
                .into_par_iter()
                .map(|i| {
                    let base = ofs + i * stride;
                    CNode {
                        plane_idx: Self::read_i32_le(data, base) as usize,
                        children: [
                            Self::read_i32_le(data, base + 4),
                            Self::read_i32_le(data, base + 8),
                        ],
                    }
                })
                .collect();
            self.map_nodes = nodes;
        } else {
            for i in 0..count {
                let base = ofs + i * stride;
                let mut node = CNode::default();
                node.plane_idx = Self::read_i32_le(data, base) as usize;
                node.children[0] = Self::read_i32_le(data, base + 4);
                node.children[1] = Self::read_i32_le(data, base + 8);
                self.map_nodes.push(node);
            }
        }
    }

    fn load_brushes(&mut self, data: &[u8], lump: &Lump) {
        let ofs = lump.fileofs as usize;
        let len = lump.filelen as usize;
        // sizeof(dbrush_t) = 12
        let stride = 12;
        if !len.is_multiple_of(stride) {
            panic!("MOD_LoadBmodel: funny lump size (brushes)");
        }
        let count = len / stride;
        if count > MAX_MAP_BRUSHES {
            panic!("Map has too many brushes");
        }

        self.numbrushes = count;
        self.map_brushes.clear();
        self.map_brushes.reserve(count + 1); // extra for box brush

        // Use parallel parsing for large brush counts
        if count >= Self::PARALLEL_LUMP_THRESHOLD {
            let brushes: Vec<CBrush> = (0..count)
                .into_par_iter()
                .map(|i| {
                    let base = ofs + i * stride;
                    CBrush {
                        firstbrushside: Self::read_i32_le(data, base),
                        numsides: Self::read_i32_le(data, base + 4),
                        contents: Self::read_i32_le(data, base + 8),
                        checkcount: 0,
                    }
                })
                .collect();
            self.map_brushes = brushes;
        } else {
            for i in 0..count {
                let base = ofs + i * stride;
                let mut brush = CBrush::default();
                brush.firstbrushside = Self::read_i32_le(data, base);
                brush.numsides = Self::read_i32_le(data, base + 4);
                brush.contents = Self::read_i32_le(data, base + 8);
                self.map_brushes.push(brush);
            }
        }
    }

    fn load_leafs(&mut self, data: &[u8], lump: &Lump) {
        let ofs = lump.fileofs as usize;
        let len = lump.filelen as usize;
        // sizeof(dleaf_t) = 4 + 2 + 2 + 3*2 + 3*2 + 2 + 2 + 2 + 2 = 28
        let stride = 28;
        if !len.is_multiple_of(stride) {
            panic!("MOD_LoadBmodel: funny lump size (leafs)");
        }
        let count = len / stride;
        if count < 1 {
            panic!("Map with no leafs");
        }
        if count > MAX_MAP_LEAFS {
            panic!("Map has too many leafs");
        }

        self.numleafs = count;
        self.map_leafs.clear();
        self.map_leafs.reserve(count + 1); // extra for box leaf

        // Use parallel parsing for large leaf counts
        // dleaf_t layout:
        // contents: i32 (4), cluster: i16 (2), area: i16 (2)
        // mins: [i16; 3] (6), maxs: [i16; 3] (6)
        // firstleafface: u16 (2), numleaffaces: u16 (2)
        // firstleafbrush: u16 (2), numleafbrushes: u16 (2)
        // total = 28
        if count >= Self::PARALLEL_LUMP_THRESHOLD {
            let leafs: Vec<CLeaf> = (0..count)
                .into_par_iter()
                .map(|i| {
                    let base = ofs + i * stride;
                    CLeaf {
                        contents: Self::read_i32_le(data, base),
                        cluster: Self::read_i16_le(data, base + 4) as i32,
                        area: Self::read_i16_le(data, base + 6) as i32,
                        firstleafbrush: Self::read_u16_le(data, base + 24),
                        numleafbrushes: Self::read_u16_le(data, base + 26),
                    }
                })
                .collect();
            self.map_leafs = leafs;
        } else {
            for i in 0..count {
                let base = ofs + i * stride;
                let leaf = CLeaf {
                    contents: Self::read_i32_le(data, base),
                    cluster: Self::read_i16_le(data, base + 4) as i32,
                    area: Self::read_i16_le(data, base + 6) as i32,
                    firstleafbrush: Self::read_u16_le(data, base + 24),
                    numleafbrushes: Self::read_u16_le(data, base + 26),
                };
                self.map_leafs.push(leaf);
            }
        }

        // Sequential post-processing: find max cluster and empty/solid leaf
        self.numclusters = self.map_leafs
            .iter()
            .map(|l| if l.cluster >= 0 { (l.cluster + 1) as usize } else { 0 })
            .max()
            .unwrap_or(0);

        if self.map_leafs[0].contents != CONTENTS_SOLID {
            panic!("Map leaf 0 is not CONTENTS_SOLID");
        }
        self.solidleaf = 0;
        self.emptyleaf = -1;
        for i in 1..self.numleafs {
            if self.map_leafs[i].contents == 0 {
                self.emptyleaf = i as i32;
                break;
            }
        }
        if self.emptyleaf == -1 {
            panic!("Map does not have an empty leaf");
        }
    }

    fn load_planes(&mut self, data: &[u8], lump: &Lump) {
        let ofs = lump.fileofs as usize;
        let len = lump.filelen as usize;
        // sizeof(dplane_t) = 3*4 + 4 + 4 = 20
        let stride = 20;
        if !len.is_multiple_of(stride) {
            panic!("MOD_LoadBmodel: funny lump size (planes)");
        }
        let count = len / stride;
        if count < 1 {
            panic!("Map with no planes");
        }
        if count > MAX_MAP_PLANES {
            panic!("Map has too many planes");
        }

        self.numplanes = count;
        self.map_planes.clear();
        self.map_planes.reserve(count + 12); // extra for box hull

        // Use parallel parsing for large plane counts
        if count >= Self::PARALLEL_LUMP_THRESHOLD {
            let planes: Vec<CPlane> = (0..count)
                .into_par_iter()
                .map(|i| {
                    let base = ofs + i * stride;
                    let mut plane = CPlane::default();
                    let mut bits: u8 = 0;
                    for j in 0..3 {
                        plane.normal[j] = Self::read_f32_le(data, base + j * 4);
                        if plane.normal[j] < 0.0 {
                            bits |= 1 << j;
                        }
                    }
                    plane.dist = Self::read_f32_le(data, base + 12);
                    plane.plane_type = Self::read_i32_le(data, base + 16) as u8;
                    plane.signbits = bits;
                    plane
                })
                .collect();
            self.map_planes = planes;
        } else {
            for i in 0..count {
                let base = ofs + i * stride;
                let mut plane = CPlane::default();
                let mut bits: u8 = 0;
                for j in 0..3 {
                    plane.normal[j] = Self::read_f32_le(data, base + j * 4);
                    if plane.normal[j] < 0.0 {
                        bits |= 1 << j;
                    }
                }
                plane.dist = Self::read_f32_le(data, base + 12);
                plane.plane_type = Self::read_i32_le(data, base + 16) as u8;
                plane.signbits = bits;
                self.map_planes.push(plane);
            }
        }
    }

    fn load_leaf_brushes(&mut self, data: &[u8], lump: &Lump) {
        let ofs = lump.fileofs as usize;
        let len = lump.filelen as usize;
        let stride = 2; // u16
        if !len.is_multiple_of(stride) {
            panic!("MOD_LoadBmodel: funny lump size (leafbrushes)");
        }
        let count = len / stride;
        if count < 1 {
            panic!("Map with no leafbrushes");
        }
        if count > MAX_MAP_LEAFBRUSHES {
            panic!("Map has too many leafbrushes");
        }

        self.numleafbrushes = count;
        self.map_leafbrushes.clear();
        self.map_leafbrushes.reserve(count + 1); // extra for box

        // Use parallel parsing for large leafbrush counts
        if count >= Self::PARALLEL_LUMP_THRESHOLD {
            let leafbrushes: Vec<u16> = (0..count)
                .into_par_iter()
                .map(|i| {
                    let base = ofs + i * stride;
                    Self::read_u16_le(data, base)
                })
                .collect();
            self.map_leafbrushes = leafbrushes;
        } else {
            for i in 0..count {
                let base = ofs + i * stride;
                self.map_leafbrushes.push(Self::read_u16_le(data, base));
            }
        }
    }

    fn load_brush_sides(&mut self, data: &[u8], lump: &Lump) {
        let ofs = lump.fileofs as usize;
        let len = lump.filelen as usize;
        // sizeof(dbrushside_t) = 2 + 2 = 4
        let stride = 4;
        if !len.is_multiple_of(stride) {
            panic!("MOD_LoadBmodel: funny lump size (brushsides)");
        }
        let count = len / stride;
        if count > MAX_MAP_BRUSHSIDES {
            panic!("Map has too many brushsides");
        }

        self.numbrushsides = count;
        self.map_brushsides.clear();
        self.map_brushsides.reserve(count + 6); // extra for box

        let numtexinfo = self.numtexinfo;

        // Use parallel parsing for large brushside counts
        if count >= Self::PARALLEL_LUMP_THRESHOLD {
            let brushsides: Vec<CBrushSide> = (0..count)
                .into_par_iter()
                .map(|i| {
                    let base = ofs + i * stride;
                    let planenum = Self::read_u16_le(data, base) as usize;
                    let texinfo = Self::read_i16_le(data, base + 2);
                    // Note: validation is done in post-processing to keep parallel code panic-free
                    let surface_idx = if texinfo >= 0 {
                        texinfo as usize
                    } else {
                        usize::MAX // null surface
                    };
                    CBrushSide {
                        plane_idx: planenum,
                        surface_idx,
                    }
                })
                .collect();

            // Validate texinfo indices sequentially
            for (i, side) in brushsides.iter().enumerate() {
                if side.surface_idx != usize::MAX && side.surface_idx >= numtexinfo {
                    panic!("Bad brushside texinfo at index {}", i);
                }
            }
            self.map_brushsides = brushsides;
        } else {
            for i in 0..count {
                let base = ofs + i * stride;
                let mut side = CBrushSide::default();
                let planenum = Self::read_u16_le(data, base) as usize;
                side.plane_idx = planenum;
                let texinfo = Self::read_i16_le(data, base + 2);
                if texinfo >= numtexinfo as i16 {
                    panic!("Bad brushside texinfo");
                }
                if texinfo >= 0 {
                    side.surface_idx = texinfo as usize;
                } else {
                    side.surface_idx = usize::MAX; // null surface
                }
                self.map_brushsides.push(side);
            }
        }
    }

    fn load_areas(&mut self, data: &[u8], lump: &Lump) {
        let ofs = lump.fileofs as usize;
        let len = lump.filelen as usize;
        // sizeof(darea_t) = 4 + 4 = 8
        let stride = 8;
        if !len.is_multiple_of(stride) {
            panic!("MOD_LoadBmodel: funny lump size (areas)");
        }
        let count = len / stride;
        if count > MAX_MAP_AREAS {
            panic!("Map has too many areas");
        }

        self.numareas = count;
        self.map_areas.clear();
        self.map_areas.reserve(count);

        // Use parallel parsing for large area counts
        if count >= Self::PARALLEL_LUMP_THRESHOLD {
            let areas: Vec<CArea> = (0..count)
                .into_par_iter()
                .map(|i| {
                    let base = ofs + i * stride;
                    CArea {
                        numareaportals: Self::read_i32_le(data, base),
                        firstareaportal: Self::read_i32_le(data, base + 4),
                        floodnum: 0,
                        floodvalid: 0,
                    }
                })
                .collect();
            self.map_areas = areas;
        } else {
            for i in 0..count {
                let base = ofs + i * stride;
                let mut area = CArea::default();
                area.numareaportals = Self::read_i32_le(data, base);
                area.firstareaportal = Self::read_i32_le(data, base + 4);
                self.map_areas.push(area);
            }
        }
    }

    fn load_area_portals(&mut self, data: &[u8], lump: &Lump) {
        let ofs = lump.fileofs as usize;
        let len = lump.filelen as usize;
        // sizeof(dareaportal_t) = 4 + 4 = 8
        let stride = 8;
        if !len.is_multiple_of(stride) {
            panic!("MOD_LoadBmodel: funny lump size (areaportals)");
        }
        let count = len / stride;
        if count > MAX_MAP_AREAPORTALS {
            panic!("Map has too many areaportals");
        }

        self.numareaportals = count;
        self.map_areaportals.clear();
        self.map_areaportals.reserve(count);

        // Use parallel parsing for large areaportal counts
        if count >= Self::PARALLEL_LUMP_THRESHOLD {
            let portals: Vec<DAreaPortal> = (0..count)
                .into_par_iter()
                .map(|i| {
                    let base = ofs + i * stride;
                    DAreaPortal {
                        portalnum: Self::read_i32_le(data, base),
                        otherarea: Self::read_i32_le(data, base + 4),
                    }
                })
                .collect();
            self.map_areaportals = portals;
        } else {
            for i in 0..count {
                let base = ofs + i * stride;
                let portal = DAreaPortal {
                    portalnum: Self::read_i32_le(data, base),
                    otherarea: Self::read_i32_le(data, base + 4),
                };
                self.map_areaportals.push(portal);
            }
        }
    }

    fn load_visibility(&mut self, data: &[u8], lump: &Lump) {
        let ofs = lump.fileofs as usize;
        let len = lump.filelen as usize;
        if len > MAX_MAP_VISIBILITY {
            panic!("Map has too large visibility lump");
        }

        self.numvisibility = len;
        self.map_visibility = data[ofs..ofs + len].to_vec();

        if len >= 4 {
            let numclusters = Self::read_i32_le(&self.map_visibility, 0);
            self.vis_data.numclusters = numclusters;
            self.vis_data.bitofs.clear();
            for i in 0..numclusters as usize {
                let base = 4 + i * 8;
                if base + 8 <= len {
                    let pvs_ofs = Self::read_i32_le(&self.map_visibility, base);
                    let phs_ofs = Self::read_i32_le(&self.map_visibility, base + 4);
                    self.vis_data.bitofs.push([pvs_ofs, phs_ofs]);
                }
            }
        }
    }

    fn load_entity_string(&mut self, data: &[u8], lump: &Lump) {
        let ofs = lump.fileofs as usize;
        let len = lump.filelen as usize;
        if len > MAX_MAP_ENTSTRING {
            panic!("Map has too large entity lump");
        }
        self.numentitychars = len;
        self.map_entitystring = String::from_utf8_lossy(&data[ofs..ofs + len]).to_string();
    }

    // ============================================================
    // CM_LoadMap
    // ============================================================

    /// Load a BSP map from raw file bytes. Returns the index of the first cmodel (always 0).
    /// `checksum` is set to a simple checksum of the file data.
    /// If `name` is empty, clears the map and returns a default model.
    pub fn load_map(&mut self, name: &str, clientload: bool, buf: Option<&[u8]>) -> (usize, u32) {
        // If same map is already loaded
        if self.map_name == name && !name.is_empty() {
            if !clientload {
                self.portalopen.iter_mut().for_each(|p| *p = false);
                self.flood_area_connections();
            }
            return (0, self.last_checksum);
        }

        // Free old stuff
        self.numplanes = 0;
        self.numnodes = 0;
        self.numleafs = 0;
        self.numcmodels = 0;
        self.numvisibility = 0;
        self.numentitychars = 0;
        self.map_entitystring.clear();
        self.map_name.clear();

        if name.is_empty() {
            self.numleafs = 1;
            self.numclusters = 1;
            self.numareas = 1;
            self.map_leafs = vec![CLeaf::default()];
            self.map_cmodels = vec![CModel::default()];
            return (0, 0);
        }

        let data = match buf {
            Some(d) => d,
            None => panic!("Couldn't load {}", name),
        };

        // Simple checksum (sum of all u32 words)
        let checksum = {
            let mut sum: u32 = 0;
            let mut i = 0;
            while i + 4 <= data.len() {
                sum = sum.wrapping_add(u32::from_le_bytes([
                    data[i],
                    data[i + 1],
                    data[i + 2],
                    data[i + 3],
                ]));
                i += 4;
            }
            sum
        };
        self.last_checksum = checksum;

        // Parse header
        if data.len() < 8 + HEADER_LUMPS * 8 {
            panic!("BSP file too short");
        }
        let _ident = Self::read_i32_le(data, 0);
        let version = Self::read_i32_le(data, 4);

        if version != BSPVERSION {
            panic!(
                "CMod_LoadBrushModel: {} has wrong version number ({} should be {})",
                name, version, BSPVERSION
            );
        }

        // Read lumps
        let mut lumps = [Lump { fileofs: 0, filelen: 0 }; HEADER_LUMPS];
        for i in 0..HEADER_LUMPS {
            let base = 8 + i * 8;
            lumps[i].fileofs = Self::read_i32_le(data, base);
            lumps[i].filelen = Self::read_i32_le(data, base + 4);
        }

        // Load in order matching the original
        self.load_surfaces(data, &lumps[LUMP_TEXINFO]);
        self.load_leafs(data, &lumps[LUMP_LEAFS]);
        self.load_leaf_brushes(data, &lumps[LUMP_LEAFBRUSHES]);
        self.load_planes(data, &lumps[LUMP_PLANES]);
        self.load_brushes(data, &lumps[LUMP_BRUSHES]);
        self.load_brush_sides(data, &lumps[LUMP_BRUSHSIDES]);
        self.load_submodels(data, &lumps[LUMP_MODELS]);
        self.load_nodes(data, &lumps[LUMP_NODES]);
        self.load_areas(data, &lumps[LUMP_AREAS]);
        self.load_area_portals(data, &lumps[LUMP_AREAPORTALS]);
        self.load_visibility(data, &lumps[LUMP_VISIBILITY]);
        self.load_entity_string(data, &lumps[LUMP_ENTITIES]);

        self.init_box_hull();

        self.portalopen.iter_mut().for_each(|p| *p = false);
        self.flood_area_connections();

        self.map_name = name.to_string();

        (0, checksum)
    }

    // ============================================================
    // CM_InitBoxHull
    // ============================================================

    fn init_box_hull(&mut self) {
        self.box_headnode = self.numnodes;
        self.box_planes_start = self.numplanes;

        if self.numnodes + 6 > MAX_MAP_NODES
            || self.numbrushes + 1 > MAX_MAP_BRUSHES
            || self.numleafbrushes + 1 > MAX_MAP_LEAFBRUSHES
            || self.numbrushsides + 6 > MAX_MAP_BRUSHSIDES
            || self.numplanes + 12 > MAX_MAP_PLANES
        {
            panic!("Not enough room for box tree");
        }

        // Ensure vectors are large enough
        while self.map_brushes.len() <= self.numbrushes {
            self.map_brushes.push(CBrush::default());
        }
        self.box_brush_idx = self.numbrushes;
        self.map_brushes[self.box_brush_idx].numsides = 6;
        self.map_brushes[self.box_brush_idx].firstbrushside = self.numbrushsides as i32;
        self.map_brushes[self.box_brush_idx].contents = CONTENTS_MONSTER;

        while self.map_leafs.len() <= self.numleafs {
            self.map_leafs.push(CLeaf::default());
        }
        self.box_leaf_idx = self.numleafs;
        self.map_leafs[self.box_leaf_idx].contents = CONTENTS_MONSTER;
        self.map_leafs[self.box_leaf_idx].firstleafbrush = self.numleafbrushes as u16;
        self.map_leafs[self.box_leaf_idx].numleafbrushes = 1;

        while self.map_leafbrushes.len() <= self.numleafbrushes {
            self.map_leafbrushes.push(0);
        }
        self.map_leafbrushes[self.numleafbrushes] = self.numbrushes as u16;

        // Ensure enough space for 6 extra nodes, 6 extra brushsides, 12 extra planes
        while self.map_nodes.len() < self.numnodes + 6 {
            self.map_nodes.push(CNode::default());
        }
        while self.map_brushsides.len() < self.numbrushsides + 6 {
            self.map_brushsides.push(CBrushSide::default());
        }
        while self.map_planes.len() < self.numplanes + 12 {
            self.map_planes.push(CPlane::default());
        }

        for i in 0..6 {
            let side = i & 1;

            // brush sides
            let bs_idx = self.numbrushsides + i;
            self.map_brushsides[bs_idx].plane_idx = self.numplanes + i * 2 + side;
            self.map_brushsides[bs_idx].surface_idx = usize::MAX; // null surface

            // nodes
            let node_idx = self.box_headnode + i;
            self.map_nodes[node_idx].plane_idx = self.numplanes + i * 2;
            self.map_nodes[node_idx].children[side] = -1 - self.emptyleaf;
            if i != 5 {
                self.map_nodes[node_idx].children[side ^ 1] = (self.box_headnode + i + 1) as i32;
            } else {
                self.map_nodes[node_idx].children[side ^ 1] = -1 - self.numleafs as i32;
            }

            // planes
            let p_idx = self.box_planes_start + i * 2;
            self.map_planes[p_idx].plane_type = (i >> 1) as u8;
            self.map_planes[p_idx].signbits = 0;
            self.map_planes[p_idx].normal = [0.0; 3];
            self.map_planes[p_idx].normal[i >> 1] = 1.0;

            let p_idx2 = self.box_planes_start + i * 2 + 1;
            self.map_planes[p_idx2].plane_type = (3 + (i >> 1)) as u8;
            self.map_planes[p_idx2].signbits = 0;
            self.map_planes[p_idx2].normal = [0.0; 3];
            self.map_planes[p_idx2].normal[i >> 1] = -1.0;
        }
    }

    // ============================================================
    // Public accessors
    // ============================================================

    /// Set up box planes for a bounding box trace, returns the box headnode index.
    pub fn headnode_for_box(&mut self, mins: &Vec3, maxs: &Vec3) -> usize {
        let bp = self.box_planes_start;
        self.map_planes[bp].dist = maxs[0];
        self.map_planes[bp + 1].dist = -maxs[0];
        self.map_planes[bp + 2].dist = mins[0];
        self.map_planes[bp + 3].dist = -mins[0];
        self.map_planes[bp + 4].dist = maxs[1];
        self.map_planes[bp + 5].dist = -maxs[1];
        self.map_planes[bp + 6].dist = mins[1];
        self.map_planes[bp + 7].dist = -mins[1];
        self.map_planes[bp + 8].dist = maxs[2];
        self.map_planes[bp + 9].dist = -maxs[2];
        self.map_planes[bp + 10].dist = mins[2];
        self.map_planes[bp + 11].dist = -mins[2];
        self.box_headnode
    }

    pub fn inline_model(&self, name: &str) -> &CModel {
        if !name.starts_with('*') {
            panic!("CM_InlineModel: bad name");
        }
        let num: usize = name[1..].parse().expect("CM_InlineModel: bad number");
        if num < 1 || num >= self.numcmodels {
            panic!("CM_InlineModel: bad number");
        }
        &self.map_cmodels[num]
    }

    pub fn num_clusters(&self) -> usize {
        self.numclusters
    }

    pub fn num_inline_models(&self) -> usize {
        self.numcmodels
    }

    pub fn entity_string(&self) -> &str {
        &self.map_entitystring
    }

    pub fn leaf_contents(&self, leafnum: usize) -> i32 {
        if leafnum >= self.numleafs {
            panic!("CM_LeafContents: bad number");
        }
        self.map_leafs[leafnum].contents
    }

    pub fn leaf_cluster(&self, leafnum: usize) -> i32 {
        if leafnum >= self.numleafs {
            panic!("CM_LeafCluster: bad number");
        }
        self.map_leafs[leafnum].cluster
    }

    pub fn leaf_area(&self, leafnum: usize) -> i32 {
        if leafnum >= self.numleafs {
            panic!("CM_LeafArea: bad number");
        }
        self.map_leafs[leafnum].area
    }

    // ============================================================
    // Point / leaf queries
    // ============================================================

    pub fn point_leafnum_r(&mut self, p: &Vec3, mut num: i32) -> usize {
        while num >= 0 {
            let node = &self.map_nodes[num as usize];
            let plane_idx = node.plane_idx;
            let children = node.children;
            let plane = &self.map_planes[plane_idx];

            let d = if (plane.plane_type as usize) < 3 {
                p[plane.plane_type as usize] - plane.dist
            } else {
                dot_product(&plane.normal, p) - plane.dist
            };

            if d < 0.0 {
                num = children[1];
            } else {
                num = children[0];
            }
        }
        self.c_pointcontents += 1;
        (-1 - num) as usize
    }

    pub fn point_leafnum(&mut self, p: &Vec3) -> usize {
        if self.numplanes == 0 {
            return 0;
        }
        self.point_leafnum_r(p, 0)
    }

    // ============================================================
    // Box leaf enumeration
    // ============================================================

    pub fn box_leafnums_r(
        &self,
        mut nodenum: i32,
        leaf_list: &mut Vec<usize>,
        leaf_maxcount: usize,
        leaf_mins: &Vec3,
        leaf_maxs: &Vec3,
        leaf_topnode: &mut i32,
    ) {
        loop {
            if nodenum < 0 {
                if leaf_list.len() >= leaf_maxcount {
                    return;
                }
                leaf_list.push((-1 - nodenum) as usize);
                return;
            }

            let node = &self.map_nodes[nodenum as usize];
            let plane = &self.map_planes[node.plane_idx];
            let s = box_on_plane_side(leaf_mins, leaf_maxs, plane);

            if s == 1 {
                nodenum = node.children[0];
            } else if s == 2 {
                nodenum = node.children[1];
            } else {
                if *leaf_topnode == -1 {
                    *leaf_topnode = nodenum;
                }
                self.box_leafnums_r(
                    node.children[0],
                    leaf_list,
                    leaf_maxcount,
                    leaf_mins,
                    leaf_maxs,
                    leaf_topnode,
                );
                nodenum = node.children[1];
            }
        }
    }

    pub fn box_leafnums_headnode(
        &self,
        mins: &Vec3,
        maxs: &Vec3,
        listsize: usize,
        headnode: i32,
    ) -> (Vec<usize>, i32) {
        let mut leaf_list = Vec::with_capacity(listsize);
        let mut topnode: i32 = -1;
        self.box_leafnums_r(headnode, &mut leaf_list, listsize, mins, maxs, &mut topnode);
        (leaf_list, topnode)
    }

    pub fn box_leafnums(
        &self,
        mins: &Vec3,
        maxs: &Vec3,
        listsize: usize,
    ) -> (Vec<usize>, i32) {
        let headnode = if self.numcmodels > 0 {
            self.map_cmodels[0].headnode
        } else {
            0
        };
        self.box_leafnums_headnode(mins, maxs, listsize, headnode)
    }

    // ============================================================
    // Point contents
    // ============================================================

    pub fn point_contents(&mut self, p: &Vec3, headnode: i32) -> i32 {
        if self.numnodes == 0 {
            return 0;
        }
        let l = self.point_leafnum_r(p, headnode);
        self.map_leafs[l].contents
    }

    pub fn transformed_point_contents(
        &mut self,
        p: &Vec3,
        headnode: i32,
        origin: &Vec3,
        angles: &Vec3,
    ) -> i32 {
        let mut p_l = vector_subtract(p, origin);

        if headnode as usize != self.box_headnode
            && (angles[0] != 0.0 || angles[1] != 0.0 || angles[2] != 0.0)
        {
            let mut forward = [0.0f32; 3];
            let mut right = [0.0f32; 3];
            let mut up = [0.0f32; 3];
            angle_vectors(angles, Some(&mut forward), Some(&mut right), Some(&mut up));

            let temp = p_l;
            p_l[0] = dot_product(&temp, &forward);
            p_l[1] = -dot_product(&temp, &right);
            p_l[2] = dot_product(&temp, &up);
        }

        let l = self.point_leafnum_r(&p_l, headnode);
        self.map_leafs[l].contents
    }

    // ============================================================
    // Box tracing
    // ============================================================
    // Note: Collision brush testing (clip_box_to_brush, trace_to_leaf, test_in_leaf)
    // is NOT parallelizable due to:
    // 1. Sequential checkcount mechanism - avoids re-testing same brush across leaves
    // 2. Early exit when trace.fraction == 0.0 depends on previous brush results
    // 3. Small data sets - most traces test only 1-10 brushes, below parallel threshold
    // 4. The minimum-fraction finding requires sequential comparison
    // ============================================================

    fn clip_box_to_brush(
        &mut self,
        mins: &Vec3,
        maxs: &Vec3,
        p1: &Vec3,
        p2: &Vec3,
        trace: &mut Trace,
        brush_idx: usize,
        trace_ispoint: bool,
    ) {
        let numsides = self.map_brushes[brush_idx].numsides;
        let firstbrushside = self.map_brushes[brush_idx].firstbrushside;
        let brush_contents = self.map_brushes[brush_idx].contents;

        if numsides == 0 {
            return;
        }

        self.c_brush_traces += 1;

        let mut enterfrac: f32 = -1.0;
        let mut leavefrac: f32 = 1.0;
        let mut clipplane_idx: Option<usize> = None;

        let mut getout = false;
        let mut startout = false;
        let mut leadside_idx: Option<usize> = None;

        for i in 0..numsides {
            let side_idx = (firstbrushside + i) as usize;
            let plane_idx = self.map_brushsides[side_idx].plane_idx;
            let plane = self.map_planes[plane_idx];

            let dist;
            if !trace_ispoint {
                let mut ofs = [0.0f32; 3];
                for j in 0..3 {
                    if plane.normal[j] < 0.0 {
                        ofs[j] = maxs[j];
                    } else {
                        ofs[j] = mins[j];
                    }
                }
                dist = plane.dist - dot_product(&ofs, &plane.normal);
            } else {
                dist = plane.dist;
            }

            let d1 = dot_product(p1, &plane.normal) - dist;
            let d2 = dot_product(p2, &plane.normal) - dist;

            if d2 > 0.0 {
                getout = true;
            }
            if d1 > 0.0 {
                startout = true;
            }

            if d1 > 0.0 && d2 >= d1 {
                return;
            }
            if d1 <= 0.0 && d2 <= 0.0 {
                continue;
            }

            if d1 > d2 {
                let f = (d1 - DIST_EPSILON) / (d1 - d2);
                if f > enterfrac {
                    enterfrac = f;
                    clipplane_idx = Some(plane_idx);
                    leadside_idx = Some(side_idx);
                }
            } else {
                let f = (d1 + DIST_EPSILON) / (d1 - d2);
                if f < leavefrac {
                    leavefrac = f;
                }
            }
        }

        if !startout {
            trace.startsolid = true;
            if !getout {
                trace.allsolid = true;
            }
            return;
        }

        if enterfrac < leavefrac
            && enterfrac > -1.0 && enterfrac < trace.fraction {
                if enterfrac < 0.0 {
                    enterfrac = 0.0;
                }
                trace.fraction = enterfrac;
                if let Some(cp_idx) = clipplane_idx {
                    trace.plane = self.map_planes[cp_idx];
                }
                if let Some(ls_idx) = leadside_idx {
                    let surf_idx = self.map_brushsides[ls_idx].surface_idx;
                    if surf_idx != usize::MAX {
                        trace.surface = Some(self.map_surfaces[surf_idx].c.clone());
                    } else {
                        trace.surface = Some(self.nullsurface.c.clone());
                    }
                }
                trace.contents = brush_contents;
            }
    }

    fn test_box_in_brush(
        &self,
        mins: &Vec3,
        maxs: &Vec3,
        p1: &Vec3,
        trace: &mut Trace,
        brush_idx: usize,
    ) {
        let numsides = self.map_brushes[brush_idx].numsides;
        let firstbrushside = self.map_brushes[brush_idx].firstbrushside;
        let brush_contents = self.map_brushes[brush_idx].contents;

        if numsides == 0 {
            return;
        }

        for i in 0..numsides {
            let side_idx = (firstbrushside + i) as usize;
            let plane_idx = self.map_brushsides[side_idx].plane_idx;
            let plane = &self.map_planes[plane_idx];

            let mut ofs = [0.0f32; 3];
            for j in 0..3 {
                if plane.normal[j] < 0.0 {
                    ofs[j] = maxs[j];
                } else {
                    ofs[j] = mins[j];
                }
            }
            let dist = plane.dist - dot_product(&ofs, &plane.normal);
            let d1 = dot_product(p1, &plane.normal) - dist;

            if d1 > 0.0 {
                return;
            }
        }

        trace.startsolid = true;
        trace.allsolid = true;
        trace.fraction = 0.0;
        trace.contents = brush_contents;
    }

    fn trace_to_leaf(
        &mut self,
        leafnum: usize,
        trace_contents: i32,
        trace_mins: &Vec3,
        trace_maxs: &Vec3,
        trace_start: &Vec3,
        trace_end: &Vec3,
        trace_ispoint: bool,
        trace: &mut Trace,
    ) {
        let leaf_contents = self.map_leafs[leafnum].contents;
        if leaf_contents & trace_contents == 0 {
            return;
        }
        let first = self.map_leafs[leafnum].firstleafbrush as usize;
        let count = self.map_leafs[leafnum].numleafbrushes as usize;

        for k in 0..count {
            let brushnum = self.map_leafbrushes[first + k] as usize;
            if self.map_brushes[brushnum].checkcount == self.checkcount {
                continue;
            }
            self.map_brushes[brushnum].checkcount = self.checkcount;

            if self.map_brushes[brushnum].contents & trace_contents == 0 {
                continue;
            }
            self.clip_box_to_brush(
                trace_mins,
                trace_maxs,
                trace_start,
                trace_end,
                trace,
                brushnum,
                trace_ispoint,
            );
            if trace.fraction == 0.0 {
                return;
            }
        }
    }

    fn test_in_leaf(
        &mut self,
        leafnum: usize,
        trace_contents: i32,
        trace_mins: &Vec3,
        trace_maxs: &Vec3,
        trace_start: &Vec3,
        trace: &mut Trace,
    ) {
        let leaf_contents = self.map_leafs[leafnum].contents;
        if leaf_contents & trace_contents == 0 {
            return;
        }
        let first = self.map_leafs[leafnum].firstleafbrush as usize;
        let count = self.map_leafs[leafnum].numleafbrushes as usize;

        for k in 0..count {
            let brushnum = self.map_leafbrushes[first + k] as usize;
            if self.map_brushes[brushnum].checkcount == self.checkcount {
                continue;
            }
            self.map_brushes[brushnum].checkcount = self.checkcount;

            if self.map_brushes[brushnum].contents & trace_contents == 0 {
                continue;
            }
            self.test_box_in_brush(trace_mins, trace_maxs, trace_start, trace, brushnum);
            if trace.fraction == 0.0 {
                return;
            }
        }
    }

    fn recursive_hull_check(
        &mut self,
        num: i32,
        p1f: f32,
        p2f: f32,
        p1: &Vec3,
        p2: &Vec3,
        trace_contents: i32,
        trace_mins: &Vec3,
        trace_maxs: &Vec3,
        trace_start: &Vec3,
        trace_end: &Vec3,
        trace_extents: &Vec3,
        trace_ispoint: bool,
        trace: &mut Trace,
    ) {
        if trace.fraction <= p1f {
            return;
        }

        if num < 0 {
            self.trace_to_leaf(
                (-1 - num) as usize,
                trace_contents,
                trace_mins,
                trace_maxs,
                trace_start,
                trace_end,
                trace_ispoint,
                trace,
            );
            return;
        }

        let node = &self.map_nodes[num as usize];
        let plane_idx = node.plane_idx;
        let children = node.children;
        let plane = &self.map_planes[plane_idx];

        let (t1, t2, offset);
        if (plane.plane_type as usize) < 3 {
            let pt = plane.plane_type as usize;
            t1 = p1[pt] - plane.dist;
            t2 = p2[pt] - plane.dist;
            offset = trace_extents[pt];
        } else {
            t1 = dot_product(&plane.normal, p1) - plane.dist;
            t2 = dot_product(&plane.normal, p2) - plane.dist;
            if trace_ispoint {
                offset = 0.0;
            } else {
                offset = (trace_extents[0] * plane.normal[0]).abs()
                    + (trace_extents[1] * plane.normal[1]).abs()
                    + (trace_extents[2] * plane.normal[2]).abs();
            }
        }

        if t1 >= offset && t2 >= offset {
            self.recursive_hull_check(
                children[0], p1f, p2f, p1, p2, trace_contents, trace_mins, trace_maxs,
                trace_start, trace_end, trace_extents, trace_ispoint, trace,
            );
            return;
        }
        if t1 < -offset && t2 < -offset {
            self.recursive_hull_check(
                children[1], p1f, p2f, p1, p2, trace_contents, trace_mins, trace_maxs,
                trace_start, trace_end, trace_extents, trace_ispoint, trace,
            );
            return;
        }

        let (side, frac, frac2);
        if t1 < t2 {
            let idist = 1.0 / (t1 - t2);
            side = 1usize;
            frac = ((t1 - offset + DIST_EPSILON) * idist).clamp(0.0, 1.0);
            frac2 = ((t1 + offset + DIST_EPSILON) * idist).clamp(0.0, 1.0);
        } else if t1 > t2 {
            let idist = 1.0 / (t1 - t2);
            side = 0usize;
            frac = ((t1 + offset + DIST_EPSILON) * idist).clamp(0.0, 1.0);
            frac2 = ((t1 - offset - DIST_EPSILON) * idist).clamp(0.0, 1.0);
        } else {
            side = 0;
            frac = 1.0;
            frac2 = 0.0;
        };

        let midf = p1f + (p2f - p1f) * frac;
        let mid: Vec3 = [
            p1[0] + frac * (p2[0] - p1[0]),
            p1[1] + frac * (p2[1] - p1[1]),
            p1[2] + frac * (p2[2] - p1[2]),
        ];

        self.recursive_hull_check(
            children[side], p1f, midf, p1, &mid, trace_contents, trace_mins, trace_maxs,
            trace_start, trace_end, trace_extents, trace_ispoint, trace,
        );

        let midf2 = p1f + (p2f - p1f) * frac2;
        let mid2: Vec3 = [
            p1[0] + frac2 * (p2[0] - p1[0]),
            p1[1] + frac2 * (p2[1] - p1[1]),
            p1[2] + frac2 * (p2[2] - p1[2]),
        ];

        self.recursive_hull_check(
            children[side ^ 1], midf2, p2f, &mid2, p2, trace_contents, trace_mins, trace_maxs,
            trace_start, trace_end, trace_extents, trace_ispoint, trace,
        );
    }

    // ============================================================
    // CM_BoxTrace
    // ============================================================

    pub fn box_trace(
        &mut self,
        start: &Vec3,
        end: &Vec3,
        mins: &Vec3,
        maxs: &Vec3,
        headnode: i32,
        brushmask: i32,
    ) -> Trace {
        self.checkcount += 1;
        self.c_traces += 1;

        let mut trace = Trace::default();
        trace.fraction = 1.0;
        trace.surface = Some(self.nullsurface.c.clone());

        if self.numnodes == 0 {
            return trace;
        }

        let trace_contents = brushmask;
        let trace_start = *start;
        let trace_end = *end;
        let trace_mins = *mins;
        let trace_maxs = *maxs;

        // Position test special case
        if start[0] == end[0] && start[1] == end[1] && start[2] == end[2] {
            let c1 = [
                start[0] + mins[0] - 1.0,
                start[1] + mins[1] - 1.0,
                start[2] + mins[2] - 1.0,
            ];
            let c2 = [
                start[0] + maxs[0] + 1.0,
                start[1] + maxs[1] + 1.0,
                start[2] + maxs[2] + 1.0,
            ];

            let (leafs, _topnode) =
                self.box_leafnums_headnode(&c1, &c2, 1024, headnode);
            for &leafnum in &leafs {
                self.test_in_leaf(
                    leafnum,
                    trace_contents,
                    &trace_mins,
                    &trace_maxs,
                    &trace_start,
                    &mut trace,
                );
                if trace.allsolid {
                    break;
                }
            }
            trace.endpos = *start;
            return trace;
        }

        // Point special case
        let trace_ispoint;
        let trace_extents;
        if mins[0] == 0.0
            && mins[1] == 0.0
            && mins[2] == 0.0
            && maxs[0] == 0.0
            && maxs[1] == 0.0
            && maxs[2] == 0.0
        {
            trace_ispoint = true;
            trace_extents = [0.0f32; 3];
        } else {
            trace_ispoint = false;
            trace_extents = [
                if -mins[0] > maxs[0] { -mins[0] } else { maxs[0] },
                if -mins[1] > maxs[1] { -mins[1] } else { maxs[1] },
                if -mins[2] > maxs[2] { -mins[2] } else { maxs[2] },
            ];
        }

        self.recursive_hull_check(
            headnode,
            0.0,
            1.0,
            start,
            end,
            trace_contents,
            &trace_mins,
            &trace_maxs,
            &trace_start,
            &trace_end,
            &trace_extents,
            trace_ispoint,
            &mut trace,
        );

        if trace.fraction == 1.0 {
            trace.endpos = *end;
        } else {
            for i in 0..3 {
                trace.endpos[i] = start[i] + trace.fraction * (end[i] - start[i]);
            }
        }

        trace
    }

    // ============================================================
    // CM_TransformedBoxTrace
    // ============================================================

    pub fn transformed_box_trace(
        &mut self,
        start: &Vec3,
        end: &Vec3,
        mins: &Vec3,
        maxs: &Vec3,
        headnode: i32,
        brushmask: i32,
        origin: &Vec3,
        angles: &Vec3,
    ) -> Trace {
        let mut start_l = vector_subtract(start, origin);
        let mut end_l = vector_subtract(end, origin);

        let rotated = headnode as usize != self.box_headnode
            && (angles[0] != 0.0 || angles[1] != 0.0 || angles[2] != 0.0);

        if rotated {
            let mut forward = [0.0f32; 3];
            let mut right = [0.0f32; 3];
            let mut up = [0.0f32; 3];
            angle_vectors(angles, Some(&mut forward), Some(&mut right), Some(&mut up));

            let temp = start_l;
            start_l[0] = dot_product(&temp, &forward);
            start_l[1] = -dot_product(&temp, &right);
            start_l[2] = dot_product(&temp, &up);

            let temp = end_l;
            end_l[0] = dot_product(&temp, &forward);
            end_l[1] = -dot_product(&temp, &right);
            end_l[2] = dot_product(&temp, &up);
        }

        let mut trace = self.box_trace(&start_l, &end_l, mins, maxs, headnode, brushmask);

        if rotated && trace.fraction != 1.0 {
            let a = [-angles[0], -angles[1], -angles[2]];
            let mut forward = [0.0f32; 3];
            let mut right = [0.0f32; 3];
            let mut up = [0.0f32; 3];
            angle_vectors(&a, Some(&mut forward), Some(&mut right), Some(&mut up));

            let temp = trace.plane.normal;
            trace.plane.normal[0] = dot_product(&temp, &forward);
            trace.plane.normal[1] = -dot_product(&temp, &right);
            trace.plane.normal[2] = dot_product(&temp, &up);
        }

        trace.endpos[0] = start[0] + trace.fraction * (end[0] - start[0]);
        trace.endpos[1] = start[1] + trace.fraction * (end[1] - start[1]);
        trace.endpos[2] = start[2] + trace.fraction * (end[2] - start[2]);

        trace
    }

    // ============================================================
    // PVS / PHS
    // ============================================================

    fn decompress_vis(&self, in_offset: usize, out: &mut [u8]) {
        let row = (self.numclusters + 7) >> 3;
        let mut out_p = 0;

        if in_offset == 0 || self.numvisibility == 0 {
            // no vis info, make all visible
            for _i in 0..row {
                if out_p < out.len() {
                    out[out_p] = 0xff;
                    out_p += 1;
                }
            }
            return;
        }

        let vis = &self.map_visibility;
        let mut inp = in_offset;

        while out_p < row {
            if inp >= vis.len() {
                break;
            }
            if vis[inp] != 0 {
                out[out_p] = vis[inp];
                out_p += 1;
                inp += 1;
                continue;
            }

            // Run-length zero
            if inp + 1 >= vis.len() {
                break;
            }
            let mut c = vis[inp + 1] as usize;
            inp += 2;
            if out_p + c > row {
                c = row - out_p;
                crate::common::com_dprintf("warning: Vis decompression overrun\n");
            }
            for _ in 0..c {
                if out_p < out.len() {
                    out[out_p] = 0;
                    out_p += 1;
                }
            }
        }
    }

    pub fn cluster_pvs(&mut self, cluster: i32) -> &[u8] {
        let row = (self.numclusters + 7) >> 3;
        if cluster == -1 {
            self.pvsrow[..row].fill(0);
        } else {
            let c = cluster as usize;
            let offset = if c < self.vis_data.bitofs.len() {
                self.vis_data.bitofs[c][DVIS_PVS as usize] as usize
            } else {
                0
            };
            self.decompress_vis(offset, &mut self.pvsrow.clone());
            // We need to write into self.pvsrow, so do it again properly:
            let mut buf = vec![0u8; row];
            self.decompress_vis(offset, &mut buf);
            self.pvsrow[..row].copy_from_slice(&buf[..row]);
        }
        &self.pvsrow[..row]
    }

    pub fn cluster_phs(&mut self, cluster: i32) -> &[u8] {
        let row = (self.numclusters + 7) >> 3;
        if cluster == -1 {
            self.phsrow[..row].fill(0);
        } else {
            let c = cluster as usize;
            let offset = if c < self.vis_data.bitofs.len() {
                self.vis_data.bitofs[c][DVIS_PHS as usize] as usize
            } else {
                0
            };
            let mut buf = vec![0u8; row];
            self.decompress_vis(offset, &mut buf);
            self.phsrow[..row].copy_from_slice(&buf[..row]);
        }
        &self.phsrow[..row]
    }

    // ============================================================
    // Area portals / flooding
    // ============================================================

    fn flood_area_r(&mut self, area_idx: usize, floodnum: i32) {
        if self.map_areas[area_idx].floodvalid == self.floodvalid {
            if self.map_areas[area_idx].floodnum == floodnum {
                return;
            }
            panic!("FloodArea_r: reflooded");
        }

        self.map_areas[area_idx].floodnum = floodnum;
        self.map_areas[area_idx].floodvalid = self.floodvalid;

        let first = self.map_areas[area_idx].firstareaportal as usize;
        let count = self.map_areas[area_idx].numareaportals as usize;

        for i in 0..count {
            let portal = &self.map_areaportals[first + i];
            let portalnum = portal.portalnum as usize;
            let otherarea = portal.otherarea as usize;
            if portalnum < self.portalopen.len() && self.portalopen[portalnum] {
                self.flood_area_r(otherarea, floodnum);
            }
        }
    }

    pub fn flood_area_connections(&mut self) {
        self.floodvalid += 1;
        let mut floodnum = 0;

        for i in 1..self.numareas {
            if self.map_areas[i].floodvalid == self.floodvalid {
                continue;
            }
            floodnum += 1;
            self.flood_area_r(i, floodnum);
        }
    }

    pub fn set_area_portal_state(&mut self, portalnum: usize, open: bool) {
        if portalnum > self.numareaportals {
            panic!("areaportal > numareaportals");
        }
        self.portalopen[portalnum] = open;
        self.flood_area_connections();
    }

    pub fn areas_connected(&self, area1: usize, area2: usize) -> bool {
        if self.map_noareas {
            return true;
        }
        if area1 >= self.numareas || area2 >= self.numareas {
            panic!("area > numareas");
        }
        self.map_areas[area1].floodnum == self.map_areas[area2].floodnum
    }

    /// Write area bits for the given area. Returns number of bytes written.
    pub fn write_area_bits(&self, buffer: &mut [u8], area: usize) -> usize {
        let bytes = (self.numareas + 7) >> 3;

        if self.map_noareas {
            for i in 0..bytes {
                if i < buffer.len() {
                    buffer[i] = 0xff;
                }
            }
        } else {
            for i in 0..bytes {
                if i < buffer.len() {
                    buffer[i] = 0;
                }
            }

            let floodnum = if area < self.map_areas.len() {
                self.map_areas[area].floodnum
            } else {
                0
            };

            for i in 0..self.numareas {
                if self.map_areas[i].floodnum == floodnum || area == 0 {
                    let byte_idx = i >> 3;
                    if byte_idx < buffer.len() {
                        buffer[byte_idx] |= 1 << (i & 7);
                    }
                }
            }
        }

        bytes
    }

    /// Write portal state for savegame.
    pub fn write_portal_state(&self, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        // Write portalopen as a sequence of bytes (1 byte per bool)
        let bytes: Vec<u8> = self.portalopen.iter().map(|&b| if b { 1 } else { 0 }).collect();
        writer.write_all(&bytes)
    }

    /// Read portal state from savegame.
    pub fn read_portal_state(&mut self, reader: &mut dyn std::io::Read) -> std::io::Result<()> {
        let mut bytes = vec![0u8; self.portalopen.len()];
        reader.read_exact(&mut bytes)?;
        for (i, &b) in bytes.iter().enumerate() {
            self.portalopen[i] = b != 0;
        }
        self.flood_area_connections();
        Ok(())
    }

    // ============================================================
    // CM_HeadnodeVisible
    // ============================================================

    pub fn headnode_visible(&self, nodenum: i32, visbits: &[u8]) -> bool {
        if nodenum < 0 {
            let leafnum = (-1 - nodenum) as usize;
            if leafnum >= self.map_leafs.len() {
                return false;
            }
            let cluster = self.map_leafs[leafnum].cluster;
            if cluster == -1 {
                return false;
            }
            let byte_idx = (cluster >> 3) as usize;
            let bit = 1u8 << (cluster & 7);
            if byte_idx < visbits.len() && (visbits[byte_idx] & bit) != 0 {
                return true;
            }
            return false;
        }

        let node = &self.map_nodes[nodenum as usize];
        if self.headnode_visible(node.children[0], visbits) {
            return true;
        }
        self.headnode_visible(node.children[1], visbits)
    }
}

impl Default for CModelContext {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// Global singleton
// ============================================================

use std::sync::Mutex;

static CMODEL_CTX: Mutex<Option<CModelContext>> = Mutex::new(None);

pub fn cmodel_init() {
    let mut g = CMODEL_CTX.lock().unwrap();
    *g = Some(CModelContext::new());
}

/// Access the global CMODEL_CTX with a closure. Returns None if not initialized.
pub fn with_cmodel_ctx<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut CModelContext) -> R,
{
    let mut g = CMODEL_CTX.lock().unwrap();
    g.as_mut().map(f)
}

/// Returns the number of inline models in the currently loaded map.
pub fn cm_num_inline_models() -> usize {
    with_cmodel_ctx(|c| c.num_inline_models()).unwrap_or(0)
}

/// Returns a clone of the inline model for the given name (e.g. "*1").
pub fn cm_inline_model(name: &str) -> CModel {
    with_cmodel_ctx(|c| *c.inline_model(name)).unwrap_or_default()
}

/// Returns a clone of the entity string from the currently loaded map.
pub fn cm_entity_string() -> String {
    with_cmodel_ctx(|c| c.entity_string().to_string()).unwrap_or_default()
}

/// Returns the contents at a point given a headnode.
pub fn cm_point_contents(p: &Vec3, headnode: i32) -> i32 {
    with_cmodel_ctx(|c| c.point_contents(p, headnode)).unwrap_or(0)
}

/// Returns the number of clusters in the currently loaded map.
pub fn cm_num_clusters() -> usize {
    with_cmodel_ctx(|c| c.num_clusters()).unwrap_or(0)
}

/// Returns leaf contents for the given leaf number.
pub fn cm_leaf_contents(leafnum: usize) -> i32 {
    with_cmodel_ctx(|c| c.leaf_contents(leafnum)).unwrap_or(0)
}

/// Returns leaf cluster for the given leaf number.
pub fn cm_leaf_cluster(leafnum: usize) -> i32 {
    with_cmodel_ctx(|c| c.leaf_cluster(leafnum)).unwrap_or(-1)
}

/// Returns leaf area for the given leaf number.
pub fn cm_leaf_area(leafnum: usize) -> i32 {
    with_cmodel_ctx(|c| c.leaf_area(leafnum)).unwrap_or(0)
}

/// Returns the leaf containing the given point.
pub fn cm_point_leafnum(p: &Vec3) -> usize {
    with_cmodel_ctx(|c| c.point_leafnum(p)).unwrap_or(0)
}

/// Returns the PVS (Potentially Visible Set) for the given cluster.
/// Returns an empty slice if no collision model is loaded.
pub fn cm_cluster_pvs(cluster: i32) -> Vec<u8> {
    with_cmodel_ctx(|c| c.cluster_pvs(cluster).to_vec()).unwrap_or_default()
}

/// Returns the PHS (Potentially Hearable Set) for the given cluster.
/// Returns an empty slice if no collision model is loaded.
pub fn cm_cluster_phs(cluster: i32) -> Vec<u8> {
    with_cmodel_ctx(|c| c.cluster_phs(cluster).to_vec()).unwrap_or_default()
}

/// Check if two areas are connected through open portals.
pub fn cm_areas_connected(area1: usize, area2: usize) -> bool {
    with_cmodel_ctx(|c| c.areas_connected(area1, area2)).unwrap_or(true)
}

/// Perform a box trace through the collision model.
pub fn cm_box_trace(start: &Vec3, end: &Vec3, mins: &Vec3, maxs: &Vec3, headnode: i32, brushmask: i32) -> Trace {
    with_cmodel_ctx(|c| c.box_trace(start, end, mins, maxs, headnode, brushmask)).unwrap_or_default()
}

/// CM_HeadnodeForBox — Create a temporary BSP headnode that represents
/// the given axis-aligned bounding box. Used by sv_link_edict to determine
/// which BSP leaves an entity's bounding box overlaps.
pub fn cm_headnode_for_box(mins: &Vec3, maxs: &Vec3) -> i32 {
    with_cmodel_ctx(|c| c.headnode_for_box(mins, maxs) as i32).unwrap_or(0)
}

/// CM_BoxLeafnums — Return a list of BSP leaf indices that the given
/// axis-aligned bounding box overlaps. Used by sv_link_edict to determine
/// PVS cluster membership for entities.
pub fn cm_box_leafnums(mins: &Vec3, maxs: &Vec3, _top_node: i32) -> Vec<i32> {
    with_cmodel_ctx(|c| {
        let (leafs, _topnode) = c.box_leafnums(mins, maxs, 1024);
        leafs.iter().map(|&l| l as i32).collect()
    }).unwrap_or_default()
}

/// CM_TransformedBoxTrace — free function wrapper.
pub fn cm_transformed_box_trace(
    start: &Vec3, end: &Vec3, mins: &Vec3, maxs: &Vec3,
    headnode: i32, brushmask: i32, origin: &Vec3, angles: &Vec3,
) -> Trace {
    with_cmodel_ctx(|c| c.transformed_box_trace(start, end, mins, maxs, headnode, brushmask, origin, angles))
        .unwrap_or_default()
}

/// CM_TransformedPointContents — free function wrapper.
pub fn cm_transformed_point_contents(p: &Vec3, headnode: i32, origin: &Vec3, angles: &Vec3) -> i32 {
    with_cmodel_ctx(|c| c.transformed_point_contents(p, headnode, origin, angles)).unwrap_or(0)
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_creation() {
        let ctx = CModelContext::new();
        assert_eq!(ctx.numleafs, 1);
        assert_eq!(ctx.numclusters, 1);
        assert_eq!(ctx.numareas, 1);
        assert!(ctx.map_name.is_empty());
    }

    #[test]
    fn test_load_empty_map() {
        let mut ctx = CModelContext::new();
        let (idx, checksum) = ctx.load_map("", false, None);
        assert_eq!(idx, 0);
        assert_eq!(checksum, 0);
        assert_eq!(ctx.numleafs, 1);
        assert_eq!(ctx.numclusters, 1);
    }

    #[test]
    fn test_box_trace_no_map() {
        let mut ctx = CModelContext::new();
        ctx.numnodes = 0;
        let start = [0.0, 0.0, 0.0];
        let end = [100.0, 0.0, 0.0];
        let mins = [-16.0, -16.0, -24.0];
        let maxs = [16.0, 16.0, 32.0];
        let trace = ctx.box_trace(&start, &end, &mins, &maxs, 0, -1);
        assert_eq!(trace.fraction, 1.0);
    }

    #[test]
    fn test_areas_connected_noareas() {
        let mut ctx = CModelContext::new();
        ctx.map_noareas = true;
        ctx.numareas = 10;
        ctx.map_areas = (0..10).map(|_| CArea::default()).collect();
        assert!(ctx.areas_connected(1, 5));
    }

    #[test]
    fn test_write_area_bits_noareas() {
        let mut ctx = CModelContext::new();
        ctx.map_noareas = true;
        ctx.numareas = 8;
        ctx.map_areas = (0..8).map(|_| CArea::default()).collect();
        let mut buf = [0u8; 4];
        let bytes = ctx.write_area_bits(&mut buf, 0);
        assert_eq!(bytes, 1);
        assert_eq!(buf[0], 0xff);
    }

    #[test]
    fn test_headnode_visible_leaf() {
        let ctx = CModelContext::new();
        let visbits = [0xffu8; 8];
        // nodenum -1 means leaf 0
        // leaf 0 has cluster 0 by default
        assert!(ctx.headnode_visible(-1, &visbits));
    }

    #[test]
    fn test_headnode_visible_no_cluster() {
        let mut ctx = CModelContext::new();
        ctx.map_leafs[0].cluster = -1;
        let visbits = [0xffu8; 8];
        assert!(!ctx.headnode_visible(-1, &visbits));
    }

    #[test]
    fn test_byte_readers() {
        let data: Vec<u8> = vec![0x01, 0x00, 0x00, 0x00, 0xFF, 0x7F];
        assert_eq!(CModelContext::read_i32_le(&data, 0), 1);
        assert_eq!(CModelContext::read_u16_le(&data, 4), 0x7FFF);
        assert_eq!(CModelContext::read_i16_le(&data, 4), 0x7FFF);
    }

    #[test]
    fn test_portal_state_roundtrip() {
        let mut ctx = CModelContext::new();
        ctx.portalopen[0] = true;
        ctx.portalopen[5] = true;

        let mut buf = Vec::new();
        ctx.write_portal_state(&mut buf).unwrap();

        let mut ctx2 = CModelContext::new();
        ctx2.numareas = 1;
        ctx2.map_areas = vec![CArea::default()];
        let mut cursor = std::io::Cursor::new(buf);
        ctx2.read_portal_state(&mut cursor).unwrap();

        assert!(ctx2.portalopen[0]);
        assert!(!ctx2.portalopen[1]);
        assert!(ctx2.portalopen[5]);
    }

    // =========================================================================
    // Helper: create a CModelContext with init_box_hull set up for trace tests
    // =========================================================================

    fn make_box_hull_ctx() -> CModelContext {
        let mut ctx = CModelContext::new();
        // Set the empty leaf index to 0 (leaf 0 has no contents)
        ctx.emptyleaf = 0;
        // We need at least 1 leaf for the empty leaf
        ctx.numleafs = 1;
        ctx.map_leafs = vec![CLeaf::default()];

        // init_box_hull needs room for nodes/planes/brushsides/leafs/brushes/leafbrushes
        ctx.numnodes = 0;
        ctx.numplanes = 0;
        ctx.numbrushes = 0;
        ctx.numbrushsides = 0;
        ctx.numleafbrushes = 0;

        ctx.init_box_hull();

        // After init, numnodes should still be 0 (box_headnode is separate)
        // but box_headnode is set. We need numnodes > 0 for box_trace to not
        // return early. Set it to the box headnode count.
        ctx.numnodes = ctx.box_headnode + 6;

        ctx
    }

    // =========================================================================
    // cm_box_trace with zero-size box (point trace)
    // =========================================================================

    #[test]
    fn test_box_trace_point_trace_through_empty() {
        let mut ctx = make_box_hull_ctx();

        let start = [0.0, 0.0, 0.0];
        let end = [100.0, 0.0, 0.0];
        let mins = [0.0, 0.0, 0.0]; // point trace
        let maxs = [0.0, 0.0, 0.0];

        // headnode_for_box sets up a box around some extents
        // If we trace through empty space with no geometry, fraction should be 1.0
        let hn = ctx.headnode_for_box(&[-1000.0, -1000.0, -1000.0], &[1000.0, 1000.0, 1000.0]);
        let trace = ctx.box_trace(&start, &end, &mins, &maxs, hn as i32, CONTENTS_SOLID);

        // Inside the box hull, the trace should detect the box brush
        // The box_brush has CONTENTS_MONSTER, and we're tracing with CONTENTS_SOLID mask
        // so it should not be hit
        assert_eq!(trace.fraction, 1.0,
            "Point trace with wrong content mask should pass through, fraction={}",
            trace.fraction);
    }

    #[test]
    fn test_box_trace_point_inside_box() {
        let mut ctx = make_box_hull_ctx();

        // Set up a box from -16 to 16 in all axes
        let hn = ctx.headnode_for_box(&[-16.0, -16.0, -16.0], &[16.0, 16.0, 16.0]);

        // Point trace from origin to origin (position test)
        let start = [0.0, 0.0, 0.0];
        let end = [0.0, 0.0, 0.0];
        let mins = [0.0, 0.0, 0.0];
        let maxs = [0.0, 0.0, 0.0];

        // CONTENTS_MONSTER is what the box brush contains
        let trace = ctx.box_trace(&start, &end, &mins, &maxs, hn as i32, CONTENTS_MONSTER);

        // A position test at the origin should find the point inside the brush
        assert!(trace.startsolid || trace.allsolid || trace.contents != 0,
            "Point inside box should detect contents. startsolid={}, allsolid={}, contents={}",
            trace.startsolid, trace.allsolid, trace.contents);
    }

    // =========================================================================
    // cm_box_trace with degenerate brush (zero volume) - start == end
    // =========================================================================

    #[test]
    fn test_box_trace_start_equals_end() {
        let mut ctx = make_box_hull_ctx();

        let point = [50.0, 50.0, 50.0];
        let mins = [-16.0, -16.0, -24.0];
        let maxs = [16.0, 16.0, 32.0];

        // Trace from a point to itself (position test)
        let hn = ctx.headnode_for_box(&[-100.0, -100.0, -100.0], &[100.0, 100.0, 100.0]);
        let trace = ctx.box_trace(&point, &point, &mins, &maxs, hn as i32, CONTENTS_MONSTER);

        // Should return endpos == start (the C code explicitly sets this)
        assert_eq!(trace.endpos, point,
            "Position test should set endpos = start");
    }

    // =========================================================================
    // Trace starting inside a brush (startsolid case)
    // =========================================================================

    #[test]
    fn test_box_trace_startsolid() {
        let mut ctx = make_box_hull_ctx();

        // Create a small box, then trace a large player-sized hull from inside
        let hn = ctx.headnode_for_box(&[-8.0, -8.0, -8.0], &[8.0, 8.0, 8.0]);

        let start = [0.0, 0.0, 0.0]; // inside the box
        let end = [100.0, 0.0, 0.0]; // moving out
        let mins = [-16.0, -16.0, -24.0]; // player bbox larger than box
        let maxs = [16.0, 16.0, 32.0];

        let trace = ctx.box_trace(&start, &end, &mins, &maxs, hn as i32, CONTENTS_MONSTER);

        // The player starts inside the brush, should detect startsolid
        // Note: startsolid detection depends on the position test path
        // Since start != end, this goes through the recursive hull check
        // The trace should either detect startsolid or have fraction < 1.0
        assert!(trace.startsolid || trace.allsolid || trace.fraction < 1.0,
            "Trace starting inside brush should detect collision. \
             startsolid={}, allsolid={}, fraction={}",
            trace.startsolid, trace.allsolid, trace.fraction);
    }

    // =========================================================================
    // Area connectivity: all portals open/closed
    // =========================================================================

    #[test]
    fn test_area_connectivity_all_closed() {
        let mut ctx = CModelContext::new();
        ctx.numareas = 3;
        ctx.map_areas = vec![CArea::default(); 3];
        ctx.numareaportals = 1;
        ctx.map_areaportals = vec![DAreaPortal {
            portalnum: 0,
            otherarea: 2,
        }];

        // area 1 has one portal to area 2
        ctx.map_areas[1].firstareaportal = 0;
        ctx.map_areas[1].numareaportals = 1;

        // All portals closed
        ctx.portalopen[0] = false;
        ctx.flood_area_connections();

        // Areas 1 and 2 should NOT be connected (portal closed)
        assert!(!ctx.areas_connected(1, 2),
            "Areas 1 and 2 should not be connected when portal is closed");
    }

    #[test]
    fn test_area_connectivity_all_open() {
        let mut ctx = CModelContext::new();
        ctx.numareas = 3;
        ctx.map_areas = vec![CArea::default(); 3];
        ctx.numareaportals = 1;
        ctx.map_areaportals = vec![DAreaPortal {
            portalnum: 0,
            otherarea: 2,
        }];

        // area 1 has one portal to area 2
        ctx.map_areas[1].firstareaportal = 0;
        ctx.map_areas[1].numareaportals = 1;

        // Also need reverse portal: area 2 back to area 1
        ctx.map_areaportals.push(DAreaPortal {
            portalnum: 0,
            otherarea: 1,
        });
        ctx.map_areas[2].firstareaportal = 1;
        ctx.map_areas[2].numareaportals = 1;
        ctx.numareaportals = 2;

        // Open portal
        ctx.portalopen[0] = true;
        ctx.flood_area_connections();

        // Areas 1 and 2 should be connected
        assert!(ctx.areas_connected(1, 2),
            "Areas 1 and 2 should be connected when portal is open");
    }

    #[test]
    fn test_area_connectivity_noareas_always_connected() {
        let mut ctx = CModelContext::new();
        ctx.map_noareas = true;
        ctx.numareas = 5;
        ctx.map_areas = vec![CArea::default(); 5];

        // With map_noareas=true, everything is connected regardless
        assert!(ctx.areas_connected(1, 4));
        assert!(ctx.areas_connected(2, 3));
    }

    #[test]
    fn test_set_area_portal_state() {
        let mut ctx = CModelContext::new();
        ctx.numareas = 3;
        ctx.map_areas = vec![CArea::default(); 3];
        ctx.numareaportals = 2;
        ctx.map_areaportals = vec![
            DAreaPortal { portalnum: 0, otherarea: 2 },
            DAreaPortal { portalnum: 0, otherarea: 1 },
        ];
        ctx.map_areas[1].firstareaportal = 0;
        ctx.map_areas[1].numareaportals = 1;
        ctx.map_areas[2].firstareaportal = 1;
        ctx.map_areas[2].numareaportals = 1;

        // All portals closed initially, flood to assign distinct floodnums
        ctx.portalopen[0] = false;
        ctx.flood_area_connections();
        assert!(!ctx.areas_connected(1, 2),
            "Areas should not be connected when portal is closed");

        // Open portal via set_area_portal_state (which calls flood_area_connections)
        ctx.set_area_portal_state(0, true);
        assert!(ctx.areas_connected(1, 2),
            "Areas should be connected after opening portal");

        // Close portal
        ctx.set_area_portal_state(0, false);
        assert!(!ctx.areas_connected(1, 2),
            "Areas should be disconnected after closing portal");
    }

    // =========================================================================
    // Box trace: verify DIST_EPSILON constant
    // =========================================================================

    #[test]
    fn test_dist_epsilon_matches_c() {
        // C: #define DIST_EPSILON 0.03125 (1/32)
        assert_eq!(DIST_EPSILON, 0.03125,
            "DIST_EPSILON should be 1/32 = 0.03125 to match C");
    }

    // =========================================================================
    // Trace result defaults match C initialization
    // =========================================================================

    #[test]
    fn test_trace_default_matches_c() {
        let trace = Trace::default();
        assert!(!trace.allsolid, "C initializes allsolid = false");
        assert!(!trace.startsolid, "C initializes startsolid = false");
        assert_eq!(trace.fraction, 1.0, "C initializes fraction = 1.0");
        assert_eq!(trace.endpos, [0.0, 0.0, 0.0]);
        assert_eq!(trace.contents, 0);
        assert_eq!(trace.ent_index, -1, "C initializes ent = NULL (we use -1)");
    }

    // =========================================================================
    // Box trace no-map: fraction should be 1.0
    // =========================================================================

    #[test]
    fn test_box_trace_no_map_returns_full_fraction() {
        let mut ctx = CModelContext::new();
        ctx.numnodes = 0; // no map loaded

        let start = [-100.0, -100.0, -100.0];
        let end = [100.0, 100.0, 100.0];
        let mins = [-16.0, -16.0, -24.0];
        let maxs = [16.0, 16.0, 32.0];

        let trace = ctx.box_trace(&start, &end, &mins, &maxs, 0, -1);
        assert_eq!(trace.fraction, 1.0);
        assert!(!trace.allsolid);
        assert!(!trace.startsolid);
    }

    // =========================================================================
    // headnode_for_box: verify planes are set correctly
    // =========================================================================

    #[test]
    fn test_headnode_for_box_plane_setup() {
        let mut ctx = make_box_hull_ctx();

        let mins = [-32.0, -32.0, -24.0];
        let maxs = [32.0, 32.0, 40.0];
        let _hn = ctx.headnode_for_box(&mins, &maxs);

        let bp = ctx.box_planes_start;
        // Verify the 12 box planes are set correctly:
        // planes[bp+0].dist = maxs[0] = 32
        assert_eq!(ctx.map_planes[bp].dist, 32.0);
        // planes[bp+1].dist = -maxs[0] = -32
        assert_eq!(ctx.map_planes[bp + 1].dist, -32.0);
        // planes[bp+2].dist = mins[0] = -32
        assert_eq!(ctx.map_planes[bp + 2].dist, -32.0);
        // planes[bp+3].dist = -mins[0] = 32
        assert_eq!(ctx.map_planes[bp + 3].dist, 32.0);
        // planes[bp+4].dist = maxs[1] = 32
        assert_eq!(ctx.map_planes[bp + 4].dist, 32.0);
        // planes[bp+5].dist = -maxs[1] = -32
        assert_eq!(ctx.map_planes[bp + 5].dist, -32.0);
        // planes[bp+6].dist = mins[1] = -32
        assert_eq!(ctx.map_planes[bp + 6].dist, -32.0);
        // planes[bp+7].dist = -mins[1] = 32
        assert_eq!(ctx.map_planes[bp + 7].dist, 32.0);
        // planes[bp+8].dist = maxs[2] = 40
        assert_eq!(ctx.map_planes[bp + 8].dist, 40.0);
        // planes[bp+9].dist = -maxs[2] = -40
        assert_eq!(ctx.map_planes[bp + 9].dist, -40.0);
        // planes[bp+10].dist = mins[2] = -24
        assert_eq!(ctx.map_planes[bp + 10].dist, -24.0);
        // planes[bp+11].dist = -mins[2] = 24
        assert_eq!(ctx.map_planes[bp + 11].dist, 24.0);
    }
}
