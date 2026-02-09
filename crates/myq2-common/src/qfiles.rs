// qfiles.rs — Quake 2 file format structures
// Converted from: myq2-original/qcommon/qfiles.h

// ============================================================
// PAK files
// ============================================================

/// PAK file magic: "PACK" in little-endian
pub const IDPAKHEADER: i32 = (b'K' as i32) << 24 | (b'C' as i32) << 16 | (b'A' as i32) << 8 | b'P' as i32;

/// ZIP local file header magic
pub const ZPAKHEADER: u32 = 0x504B0304;
/// ZIP central directory header magic
pub const ZPAKDIRHEADER: u32 = 0x504B0102;

#[derive(Debug, Clone)]
#[repr(C)]
pub struct DPackFile {
    pub name: [u8; 56],
    pub filepos: i32,
    pub filelen: i32,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DPackHeader {
    pub ident: i32,
    pub dirofs: i32,
    pub dirlen: i32,
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct DZipHeader {
    pub ident: u32,
    pub version: u16,
    pub flags: u16,
    pub compression: u16,
    pub modtime: u16,
    pub moddate: u16,
    pub crc32: u32,
    pub compressed_size: u32,
    pub uncompressed_size: u32,
    pub filename_length: u16,
    pub extra_field_length: u16,
}

pub const MAX_FILES_IN_PACK: usize = 4096;

// ============================================================
// PCX format
// ============================================================

#[derive(Debug, Clone)]
#[repr(C)]
pub struct Pcx {
    pub manufacturer: u8,
    pub version: u8,
    pub encoding: u8,
    pub bits_per_pixel: u8,
    pub xmin: u16,
    pub ymin: u16,
    pub xmax: u16,
    pub ymax: u16,
    pub hres: u16,
    pub vres: u16,
    pub palette: [u8; 48],
    pub reserved: u8,
    pub color_planes: u8,
    pub bytes_per_line: u16,
    pub palette_type: u16,
    pub filler: [u8; 58],
    // data follows (variable length)
}

/// PCX header size in bytes
pub const PCX_HEADER_SIZE: usize = 128;
/// PCX palette size in bytes (at end of file)
pub const PCX_PALETTE_SIZE: usize = 768;

/// Result of decoding a PCX image
pub struct PcxDecodeResult {
    /// Palette-indexed pixel data (1 byte per pixel)
    pub pixels: Vec<u8>,
    /// Image width
    pub width: u32,
    /// Image height
    pub height: u32,
    /// 768-byte RGB palette (256 colors * 3 bytes)
    pub palette: [u8; PCX_PALETTE_SIZE],
}

/// Decode a PCX image from raw bytes.
///
/// Returns `Some(PcxDecodeResult)` on success, `None` on failure.
/// Validates the PCX header and decodes the RLE-compressed pixel data.
///
/// # Arguments
/// * `raw` - Raw PCX file data including header and palette
///
/// # Supported formats
/// - 8-bit paletted PCX (manufacturer 0x0a, version 5, encoding 1)
/// - Maximum dimensions: 640x480 (matching original Q2 limits)
pub fn pcx_decode(raw: &[u8]) -> Option<PcxDecodeResult> {
    // Minimum size: header + palette
    if raw.len() < PCX_HEADER_SIZE + PCX_PALETTE_SIZE {
        return None;
    }

    // Parse header fields
    let manufacturer = raw[0];
    let version = raw[1];
    let encoding = raw[2];
    let bits_per_pixel = raw[3];
    let xmin = u16::from_le_bytes([raw[4], raw[5]]) as u32;
    let ymin = u16::from_le_bytes([raw[6], raw[7]]) as u32;
    let xmax = u16::from_le_bytes([raw[8], raw[9]]) as u32;
    let ymax = u16::from_le_bytes([raw[10], raw[11]]) as u32;

    // Validate header - must be valid 8-bit PCX
    if manufacturer != 0x0a || version != 5 || encoding != 1
        || bits_per_pixel != 8 || xmax >= 640 || ymax >= 480
    {
        return None;
    }

    let width = xmax - xmin + 1;
    let height = ymax - ymin + 1;

    let mut pixels = vec![0u8; (width * height) as usize];

    // Extract palette from last 768 bytes
    let mut palette = [0u8; PCX_PALETTE_SIZE];
    palette.copy_from_slice(&raw[raw.len() - PCX_PALETTE_SIZE..]);

    // Decode RLE pixel data starting at offset 128
    let mut src = PCX_HEADER_SIZE;
    for y in 0..height {
        let row_start = (y * width) as usize;
        let mut x = 0u32;
        while x < width {
            if src >= raw.len() - PCX_PALETTE_SIZE {
                return None; // Ran out of data before palette
            }
            let data_byte = raw[src];
            src += 1;

            let (run_length, pixel);
            if (data_byte & 0xC0) == 0xC0 {
                // RLE run
                run_length = (data_byte & 0x3F) as u32;
                if src >= raw.len() - PCX_PALETTE_SIZE {
                    return None;
                }
                pixel = raw[src];
                src += 1;
            } else {
                // Literal pixel
                run_length = 1;
                pixel = data_byte;
            }

            // Write run of pixels
            for _ in 0..run_length {
                if x < width {
                    pixels[row_start + x as usize] = pixel;
                    x += 1;
                }
            }
        }
    }

    Some(PcxDecodeResult {
        pixels,
        width,
        height,
        palette,
    })
}

// ============================================================
// MD2 model format
// ============================================================

/// MD2 magic: "IDP2" in little-endian
pub const IDALIASHEADER: i32 = (b'2' as i32) << 24 | (b'P' as i32) << 16 | (b'D' as i32) << 8 | b'I' as i32;
pub const ALIAS_VERSION: i32 = 8;

pub const MAX_TRIANGLES: usize = 4096;
pub const MAX_VERTS: usize = 2048;
pub const MAX_FRAMES: usize = 512;
pub const MAX_MD2SKINS: usize = 32;
pub const MAX_SKINNAME: usize = 64;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DStVert {
    pub s: i16,
    pub t: i16,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DTriangle {
    pub index_xyz: [i16; 3],
    pub index_st: [i16; 3],
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DTriVertx {
    pub v: [u8; 3],
    pub lightnormalindex: u8,
}

pub const DTRIVERTX_V0: usize = 0;
pub const DTRIVERTX_V1: usize = 1;
pub const DTRIVERTX_V2: usize = 2;
pub const DTRIVERTX_LNI: usize = 3;
pub const DTRIVERTX_SIZE: usize = 4;

#[derive(Debug, Clone)]
#[repr(C)]
pub struct DAliasFrame {
    pub scale: [f32; 3],
    pub translate: [f32; 3],
    pub name: [u8; 16],
    // verts follow (variable sized)
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DMdl {
    pub ident: i32,
    pub version: i32,
    pub skinwidth: i32,
    pub skinheight: i32,
    pub framesize: i32,
    pub num_skins: i32,
    pub num_xyz: i32,
    pub num_st: i32,
    pub num_tris: i32,
    pub num_glcmds: i32,
    pub num_frames: i32,
    pub ofs_skins: i32,
    pub ofs_st: i32,
    pub ofs_tris: i32,
    pub ofs_frames: i32,
    pub ofs_glcmds: i32,
    pub ofs_end: i32,
}

// ============================================================
// SP2 sprite format
// ============================================================

/// Sprite magic: "IDS2" in little-endian
pub const IDSPRITEHEADER: i32 = (b'2' as i32) << 24 | (b'S' as i32) << 16 | (b'D' as i32) << 8 | b'I' as i32;
pub const SPRITE_VERSION: i32 = 2;

#[derive(Debug, Clone)]
#[repr(C)]
pub struct DSprFrame {
    pub width: i32,
    pub height: i32,
    pub origin_x: i32,
    pub origin_y: i32,
    pub name: [u8; MAX_SKINNAME],
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct DSprite {
    pub ident: i32,
    pub version: i32,
    pub numframes: i32,
    // frames follow (variable sized)
}

// ============================================================
// WAL texture format
// ============================================================

pub const MIPLEVELS: usize = 4;

#[derive(Debug, Clone)]
#[repr(C)]
pub struct MipTex {
    pub name: [u8; 32],
    pub width: u32,
    pub height: u32,
    pub offsets: [u32; MIPLEVELS],
    pub animname: [u8; 32],
    pub flags: i32,
    pub contents: i32,
    pub value: i32,
}

// ============================================================
// BSP file format
// ============================================================

/// BSP magic: "IBSP" in little-endian
pub const IDBSPHEADER: i32 = (b'P' as i32) << 24 | (b'S' as i32) << 16 | (b'B' as i32) << 8 | b'I' as i32;
pub const BSPVERSION: i32 = 38;

// Upper design bounds
pub const MAX_MAP_MODELS: usize = 1024;
pub const MAX_MAP_BRUSHES: usize = 8192;
pub const MAX_MAP_ENTITIES: usize = 2048;
pub const MAX_MAP_ENTSTRING: usize = 0x40000;
pub const MAX_MAP_TEXINFO: usize = 8192;
pub const MAX_MAP_AREAS: usize = 256;
pub const MAX_MAP_AREAPORTALS: usize = 1024;
pub const MAX_MAP_PLANES: usize = 65536;
pub const MAX_MAP_NODES: usize = 65536;
pub const MAX_MAP_BRUSHSIDES: usize = 65536;
pub const MAX_MAP_LEAFS: usize = 65536;
pub const MAX_MAP_VERTS: usize = 65536;
pub const MAX_MAP_FACES: usize = 65536;
pub const MAX_MAP_LEAFFACES: usize = 65536;
pub const MAX_MAP_LEAFBRUSHES: usize = 65536;
pub const MAX_MAP_PORTALS: usize = 65536;
pub const MAX_MAP_EDGES: usize = 128000;
pub const MAX_MAP_SURFEDGES: usize = 256000;
pub const MAX_MAP_LIGHTING: usize = 0x200000;
pub const MAX_MAP_VISIBILITY: usize = 0x100000;

pub const MAX_KEY: usize = 32;
pub const MAX_VALUE: usize = 1024;

// Lump indices
pub const LUMP_ENTITIES: usize = 0;
pub const LUMP_PLANES: usize = 1;
pub const LUMP_VERTEXES: usize = 2;
pub const LUMP_VISIBILITY: usize = 3;
pub const LUMP_NODES: usize = 4;
pub const LUMP_TEXINFO: usize = 5;
pub const LUMP_FACES: usize = 6;
pub const LUMP_LIGHTING: usize = 7;
pub const LUMP_LEAFS: usize = 8;
pub const LUMP_LEAFFACES: usize = 9;
pub const LUMP_LEAFBRUSHES: usize = 10;
pub const LUMP_EDGES: usize = 11;
pub const LUMP_SURFEDGES: usize = 12;
pub const LUMP_MODELS: usize = 13;
pub const LUMP_BRUSHES: usize = 14;
pub const LUMP_BRUSHSIDES: usize = 15;
pub const LUMP_POP: usize = 16;
pub const LUMP_AREAS: usize = 17;
pub const LUMP_AREAPORTALS: usize = 18;
pub const HEADER_LUMPS: usize = 19;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Lump {
    pub fileofs: i32,
    pub filelen: i32,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DHeader {
    pub ident: i32,
    pub version: i32,
    pub lumps: [Lump; HEADER_LUMPS],
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DModel {
    pub mins: [f32; 3],
    pub maxs: [f32; 3],
    pub origin: [f32; 3],
    pub headnode: i32,
    pub firstface: i32,
    pub numfaces: i32,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DVertex {
    pub point: [f32; 3],
}

// Plane types
pub const PLANE_X: i32 = 0;
pub const PLANE_Y: i32 = 1;
pub const PLANE_Z: i32 = 2;
pub const PLANE_ANYX: i32 = 3;
pub const PLANE_ANYY: i32 = 4;
pub const PLANE_ANYZ: i32 = 5;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DPlane {
    pub normal: [f32; 3],
    pub dist: f32,
    pub plane_type: i32,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DNode {
    pub planenum: i32,
    pub children: [i32; 2],
    pub mins: [i16; 3],
    pub maxs: [i16; 3],
    pub firstface: u16,
    pub numfaces: u16,
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct TexInfo {
    pub vecs: [[f32; 4]; 2],
    pub flags: i32,
    pub value: i32,
    pub texture: [u8; 32],
    pub nexttexinfo: i32,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DEdge {
    pub v: [u16; 2],
}

pub const MAXLIGHTMAPS: usize = 4;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DFace {
    pub planenum: u16,
    pub side: i16,
    pub firstedge: i32,
    pub numedges: i16,
    pub texinfo: i16,
    pub styles: [u8; MAXLIGHTMAPS],
    pub lightofs: i32,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DLeaf {
    pub contents: i32,
    pub cluster: i16,
    pub area: i16,
    pub mins: [i16; 3],
    pub maxs: [i16; 3],
    pub firstleafface: u16,
    pub numleaffaces: u16,
    pub firstleafbrush: u16,
    pub numleafbrushes: u16,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DBrushSide {
    pub planenum: u16,
    pub texinfo: i16,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DBrush {
    pub firstside: i32,
    pub numsides: i32,
    pub contents: i32,
}

pub const ANGLE_UP: i32 = -1;
pub const ANGLE_DOWN: i32 = -2;

// Visibility
pub const DVIS_PVS: i32 = 0;
pub const DVIS_PHS: i32 = 1;

#[derive(Debug, Clone)]
#[repr(C)]
pub struct DVis {
    pub numclusters: i32,
    // bitofs[numclusters][2] follows — variable sized
    // Access via raw byte slicing at runtime
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DAreaPortal {
    pub portalnum: i32,
    pub otherarea: i32,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DArea {
    pub numareaportals: i32,
    pub firstareaportal: i32,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    // =========================================================================
    // Struct size verification — binary-layout-critical structs
    // =========================================================================

    #[test]
    fn size_of_dpack_header() {
        // 3 x i32 = 12 bytes
        assert_eq!(size_of::<DPackHeader>(), 12);
    }

    #[test]
    fn size_of_dpack_file() {
        // [u8; 56] + i32 + i32 = 56 + 4 + 4 = 64 bytes
        assert_eq!(size_of::<DPackFile>(), 64);
    }

    #[test]
    fn size_of_pcx() {
        // PCX header is exactly 128 bytes
        assert_eq!(size_of::<Pcx>(), PCX_HEADER_SIZE);
    }

    #[test]
    fn size_of_dmdl() {
        // 17 x i32 = 68 bytes
        assert_eq!(size_of::<DMdl>(), 68);
    }

    #[test]
    fn size_of_dheader() {
        // ident(4) + version(4) + 19 * Lump(8) = 8 + 152 = 160
        assert_eq!(size_of::<DHeader>(), 8 + HEADER_LUMPS * size_of::<Lump>());
    }

    #[test]
    fn size_of_lump() {
        // fileofs(4) + filelen(4) = 8
        assert_eq!(size_of::<Lump>(), 8);
    }

    #[test]
    fn size_of_dface() {
        // planenum(2) + side(2) + firstedge(4) + numedges(2) + texinfo(2) + styles(4) + lightofs(4) = 20
        assert_eq!(size_of::<DFace>(), 20);
    }

    #[test]
    fn size_of_dnode() {
        // planenum(4) + children(8) + mins(6) + maxs(6) + firstface(2) + numfaces(2) = 28
        assert_eq!(size_of::<DNode>(), 28);
    }

    #[test]
    fn size_of_dleaf() {
        // contents(4) + cluster(2) + area(2) + mins(6) + maxs(6) + firstleafface(2) +
        // numleaffaces(2) + firstleafbrush(2) + numleafbrushes(2) = 28
        assert_eq!(size_of::<DLeaf>(), 28);
    }

    #[test]
    fn size_of_dedge() {
        // 2 x u16 = 4
        assert_eq!(size_of::<DEdge>(), 4);
    }

    #[test]
    fn size_of_dplane() {
        // normal(12) + dist(4) + plane_type(4) = 20
        assert_eq!(size_of::<DPlane>(), 20);
    }

    #[test]
    fn size_of_dmodel() {
        // mins(12) + maxs(12) + origin(12) + headnode(4) + firstface(4) + numfaces(4) = 48
        assert_eq!(size_of::<DModel>(), 48);
    }

    #[test]
    fn size_of_dvertex() {
        // 3 x f32 = 12
        assert_eq!(size_of::<DVertex>(), 12);
    }

    #[test]
    fn size_of_dbrush() {
        // firstside(4) + numsides(4) + contents(4) = 12
        assert_eq!(size_of::<DBrush>(), 12);
    }

    #[test]
    fn size_of_dbrush_side() {
        // planenum(2) + texinfo(2) = 4
        assert_eq!(size_of::<DBrushSide>(), 4);
    }

    #[test]
    fn size_of_darea() {
        // numareaportals(4) + firstareaportal(4) = 8
        assert_eq!(size_of::<DArea>(), 8);
    }

    #[test]
    fn size_of_darea_portal() {
        // portalnum(4) + otherarea(4) = 8
        assert_eq!(size_of::<DAreaPortal>(), 8);
    }

    #[test]
    fn size_of_dst_vert() {
        // s(2) + t(2) = 4
        assert_eq!(size_of::<DStVert>(), 4);
    }

    #[test]
    fn size_of_dtriangle() {
        // index_xyz(6) + index_st(6) = 12
        assert_eq!(size_of::<DTriangle>(), 12);
    }

    #[test]
    fn size_of_dtrivertx() {
        // v(3) + lightnormalindex(1) = 4
        assert_eq!(size_of::<DTriVertx>(), 4);
    }

    #[test]
    fn size_of_dsprite() {
        // ident(4) + version(4) + numframes(4) = 12
        assert_eq!(size_of::<DSprite>(), 12);
    }

    #[test]
    fn size_of_dzip_header() {
        // packed struct: 4+2+2+2+2+2+4+4+4+2+2 = 30
        assert_eq!(size_of::<DZipHeader>(), 30);
    }

    // =========================================================================
    // PCX decode
    // =========================================================================

    /// Build a minimal valid 2x2 PCX buffer.
    /// The image is 2x2 with pixels: [0x10, 0x20, 0x30, 0x40].
    fn make_valid_pcx_2x2() -> Vec<u8> {
        let mut buf = vec![0u8; PCX_HEADER_SIZE];

        // Header fields
        buf[0] = 0x0a;  // manufacturer
        buf[1] = 5;     // version
        buf[2] = 1;     // encoding (RLE)
        buf[3] = 8;     // bits_per_pixel

        // xmin=0, ymin=0
        buf[4] = 0; buf[5] = 0;
        buf[6] = 0; buf[7] = 0;
        // xmax=1 (LE), ymax=1 (LE) => 2x2 image
        buf[8] = 1; buf[9] = 0;
        buf[10] = 1; buf[11] = 0;

        // Pixel data (no RLE needed for values < 0xC0):
        // Row 0: 0x10, 0x20
        // Row 1: 0x30, 0x40
        buf.push(0x10);
        buf.push(0x20);
        buf.push(0x30);
        buf.push(0x40);

        // Palette: 768 bytes at the end
        let palette = vec![0xABu8; PCX_PALETTE_SIZE];
        buf.extend_from_slice(&palette);

        buf
    }

    #[test]
    fn pcx_decode_valid_2x2() {
        let data = make_valid_pcx_2x2();
        let result = pcx_decode(&data).expect("should decode valid PCX");
        assert_eq!(result.width, 2);
        assert_eq!(result.height, 2);
        assert_eq!(result.pixels.len(), 4);
        assert_eq!(result.pixels[0], 0x10);
        assert_eq!(result.pixels[1], 0x20);
        assert_eq!(result.pixels[2], 0x30);
        assert_eq!(result.pixels[3], 0x40);
        // Check palette was extracted
        assert_eq!(result.palette[0], 0xAB);
        assert_eq!(result.palette[767], 0xAB);
    }

    #[test]
    fn pcx_decode_rle_run() {
        // Build a 4x1 image where two pixels are encoded as a RLE run
        let mut buf = vec![0u8; PCX_HEADER_SIZE];
        buf[0] = 0x0a;
        buf[1] = 5;
        buf[2] = 1;
        buf[3] = 8;
        // xmin=0, ymin=0
        buf[4] = 0; buf[5] = 0;
        buf[6] = 0; buf[7] = 0;
        // xmax=3, ymax=0 => 4x1 image
        buf[8] = 3; buf[9] = 0;
        buf[10] = 0; buf[11] = 0;

        // Pixel data: literal 0x05, then RLE run of 3 x 0x42
        buf.push(0x05);            // literal pixel = 0x05
        buf.push(0xC0 | 3);       // RLE marker: run of 3
        buf.push(0x42);            // pixel value = 0x42

        // Palette
        buf.extend_from_slice(&[0u8; PCX_PALETTE_SIZE]);

        let result = pcx_decode(&buf).expect("should decode RLE PCX");
        assert_eq!(result.width, 4);
        assert_eq!(result.height, 1);
        assert_eq!(result.pixels, vec![0x05, 0x42, 0x42, 0x42]);
    }

    #[test]
    fn pcx_decode_bad_manufacturer() {
        let mut data = make_valid_pcx_2x2();
        data[0] = 0x00; // wrong manufacturer
        assert!(pcx_decode(&data).is_none());
    }

    #[test]
    fn pcx_decode_bad_version() {
        let mut data = make_valid_pcx_2x2();
        data[1] = 3; // wrong version (expected 5)
        assert!(pcx_decode(&data).is_none());
    }

    #[test]
    fn pcx_decode_bad_encoding() {
        let mut data = make_valid_pcx_2x2();
        data[2] = 0; // wrong encoding (expected 1)
        assert!(pcx_decode(&data).is_none());
    }

    #[test]
    fn pcx_decode_bad_bits_per_pixel() {
        let mut data = make_valid_pcx_2x2();
        data[3] = 4; // wrong bpp (expected 8)
        assert!(pcx_decode(&data).is_none());
    }

    #[test]
    fn pcx_decode_too_small() {
        // Buffer smaller than header + palette
        let data = vec![0u8; 100];
        assert!(pcx_decode(&data).is_none());
    }

    #[test]
    fn pcx_decode_dimensions_too_large() {
        let mut data = make_valid_pcx_2x2();
        // Set xmax = 640 which is >= 640, should fail
        data[8] = 0x80; data[9] = 0x02; // xmax = 640
        assert!(pcx_decode(&data).is_none());
    }

    #[test]
    fn pcx_decode_exactly_header_plus_palette() {
        // A valid header but no pixel data at all (0x0 image is not valid
        // since xmax >= 640 check catches 0 as fine but we need pixel data)
        // Actually a 1x1 image needs at least 1 pixel byte between header and palette
        let mut buf = vec![0u8; PCX_HEADER_SIZE];
        buf[0] = 0x0a;
        buf[1] = 5;
        buf[2] = 1;
        buf[3] = 8;
        // xmin=0, ymin=0, xmax=0, ymax=0 => 1x1 image
        buf[8] = 0; buf[9] = 0;
        buf[10] = 0; buf[11] = 0;

        // Missing pixel data -- just palette
        buf.extend_from_slice(&[0u8; PCX_PALETTE_SIZE]);

        // Should fail since there's no pixel data
        assert!(pcx_decode(&buf).is_none());
    }

    // =========================================================================
    // Magic number constants verification
    // =========================================================================

    #[test]
    fn pak_header_magic() {
        // "PACK" => bytes: P=0x50, A=0x41, C=0x43, K=0x4B
        // As stored in little-endian i32: K<<24 | C<<16 | A<<8 | P
        let bytes = IDPAKHEADER.to_le_bytes();
        assert_eq!(&bytes, b"PACK");
    }

    #[test]
    fn bsp_header_magic() {
        let bytes = IDBSPHEADER.to_le_bytes();
        assert_eq!(&bytes, b"IBSP");
    }

    #[test]
    fn alias_header_magic() {
        let bytes = IDALIASHEADER.to_le_bytes();
        assert_eq!(&bytes, b"IDP2");
    }

    #[test]
    fn sprite_header_magic() {
        let bytes = IDSPRITEHEADER.to_le_bytes();
        assert_eq!(&bytes, b"IDS2");
    }

    #[test]
    fn bsp_version() {
        assert_eq!(BSPVERSION, 38);
    }

    #[test]
    fn alias_version() {
        assert_eq!(ALIAS_VERSION, 8);
    }

    #[test]
    fn sprite_version() {
        assert_eq!(SPRITE_VERSION, 2);
    }

    #[test]
    fn header_lumps_count() {
        assert_eq!(HEADER_LUMPS, 19);
    }
}
