// vk_image.rs -- Image/texture loading and management
// Converted from: myq2-original/ref_gl/vk_image.c

#![allow(dead_code, unused_variables, unused_mut, unused_imports, non_upper_case_globals,
         clippy::too_many_arguments, unused_unsafe, static_mut_refs, clippy::manual_range_contains)]

use crate::vk_local::*;
use crate::vk_rmain::vid_printf;
use myq2_common::q_shared::{MAX_QPATH, PRINT_ALL, ERR_DROP, q_streq_nocase};
use myq2_common::common::com_error;
use rayon::prelude::*;

// ============================================================
// Module-level state (C globals)
// ============================================================

pub static mut d_8to24table: [u32; 256] = [0u32; 256];
pub static mut r_rawpalette: [u32; 256] = [0u32; 256];

static mut intensitytable: [u8; 256] = [0u8; 256];
static mut gammatable: [u8; 256] = [0u8; 256];

pub static mut vk_solid_format: i32 = 3;
pub static mut vk_alpha_format: i32 = 4;

static mut vk_filter_min: i32 = VK_LINEAR_MIPMAP_LINEAR;
static mut vk_filter_max: i32 = VK_LINEAR;

static mut upload_width: i32 = 0;
static mut upload_height: i32 = 0;

// ============================================================
// Scrap allocation
// ============================================================

pub const MAX_SCRAPS: usize = 1;
pub const BLOCK_WIDTH: usize = 256;
pub const BLOCK_HEIGHT: usize = 256;

pub static mut scrap_allocated: [[i32; BLOCK_WIDTH]; MAX_SCRAPS] = [[0i32; BLOCK_WIDTH]; MAX_SCRAPS];
pub static mut scrap_texels: [[u8; BLOCK_WIDTH * BLOCK_HEIGHT]; MAX_SCRAPS] = [[0u8; BLOCK_WIDTH * BLOCK_HEIGHT]; MAX_SCRAPS];
pub static mut scrap_dirty: i32 = 0; // qboolean
pub static mut scrap_uploads: i32 = 0;

// ============================================================
// GL texture mode tables
// ============================================================

struct GlMode {
    name: &'static str,
    minimize: i32,
    maximize: i32,
}

const MODES: &[GlMode] = &[
    GlMode { name: "VK_NEAREST",                minimize: VK_NEAREST,                maximize: VK_NEAREST },
    GlMode { name: "VK_LINEAR",                 minimize: VK_LINEAR,                 maximize: VK_LINEAR },
    GlMode { name: "VK_NEAREST_MIPMAP_NEAREST", minimize: VK_NEAREST_MIPMAP_NEAREST, maximize: VK_NEAREST },
    GlMode { name: "VK_LINEAR_MIPMAP_NEAREST",  minimize: VK_LINEAR_MIPMAP_NEAREST,  maximize: VK_LINEAR },
    GlMode { name: "VK_NEAREST_MIPMAP_LINEAR",  minimize: VK_NEAREST_MIPMAP_LINEAR,  maximize: VK_NEAREST },
    GlMode { name: "VK_LINEAR_MIPMAP_LINEAR",   minimize: VK_LINEAR_MIPMAP_LINEAR,   maximize: VK_LINEAR },
];

struct GlTMode {
    name: &'static str,
    mode: i32,
}

const VK_ALPHA_MODES: &[GlTMode] = &[
    GlTMode { name: "default",    mode: 4 },
    GlTMode { name: "VK_RGBA",    mode: VK_RGBA },
    GlTMode { name: "VK_RGBA8",   mode: VK_RGBA8 },
    GlTMode { name: "VK_RGB5_A1", mode: VK_RGB5_A1 },
    GlTMode { name: "VK_RGBA4",   mode: VK_RGBA4 },
    GlTMode { name: "VK_RGBA2",   mode: VK_RGBA2 },
];

const VK_SOLID_MODES: &[GlTMode] = &[
    GlTMode { name: "default",     mode: 3 },
    GlTMode { name: "VK_RGB",      mode: VK_RGB },
    GlTMode { name: "VK_RGB8",     mode: VK_RGB8 },
    GlTMode { name: "VK_RGB5",     mode: VK_RGB5 },
    GlTMode { name: "VK_RGB4",     mode: VK_RGB4 },
    GlTMode { name: "VK_R3_G3_B2", mode: VK_R3_G3_B2 },
];

// ============================================================
// vk_enable_multitexture
// ============================================================

pub unsafe fn vk_enable_multitexture_impl(enable: bool) {
    if !has_multitexture() {
        return;
    }

    vk_select_texture(VK_TEXTURE3);
    if enable { qvk_enable(VK_TEXTURE_2D); } else { qvk_disable(VK_TEXTURE_2D); }

    vk_select_texture(VK_TEXTURE2);
    if enable { qvk_enable(VK_TEXTURE_2D); } else { qvk_disable(VK_TEXTURE_2D); }

    vk_select_texture(VK_TEXTURE1);
    if enable { qvk_enable(VK_TEXTURE_2D); } else { qvk_disable(VK_TEXTURE_2D); }

    vk_tex_env(VK_REPLACE as u32);

    vk_select_texture(VK_TEXTURE0);
    vk_tex_env(VK_REPLACE as u32);
}

// ============================================================
// vk_select_texture
// ============================================================

pub unsafe fn vk_select_texture_impl(texture: u32) {
    if !has_multitexture() {
        return;
    }

    let tmu: i32 = if texture == VK_TEXTURE0 {
        0
    } else if texture == VK_TEXTURE2 {
        2
    } else if texture == VK_TEXTURE3 {
        3
    } else {
        1
    };

    if tmu == vk_state.currenttmu {
        return;
    }

    vk_state.currenttmu = tmu;

    // In C: qglSelectTextureSGIS or qglActiveTextureARB
    qvk_active_texture_arb(texture);
    qvk_client_active_texture_arb(texture);
}

// ============================================================
// vk_tex_env
// ============================================================

static mut lastmodes: [i32; 4] = [-1, -1, -1, -1];

pub unsafe fn vk_tex_env_impl(mode: i32) {
    if mode != lastmodes[vk_state.currenttmu as usize] {
        qvk_tex_envf(VK_TEXTURE_ENV, VK_TEXTURE_ENV_MODE, mode as f32);
        lastmodes[vk_state.currenttmu as usize] = mode;
    }
}

// ============================================================
// vk_bind
// ============================================================

pub unsafe fn vk_bind_impl(texnum: i32) {
    extern "C" {
        // draw_chars is in vk_draw
    }
    // performance evaluation option (vk_nobind) omitted for simplicity

    if vk_state.currenttextures[vk_state.currenttmu as usize] == texnum {
        return;
    }
    vk_state.currenttextures[vk_state.currenttmu as usize] = texnum;
    qvk_bind_texture(VK_TEXTURE_2D, texnum);
}

// ============================================================
// vk_m_bind
// ============================================================

pub unsafe fn vk_mbind_impl(target: u32, texnum: i32) {
    vk_select_texture(target);
    let tmu = if target == VK_TEXTURE0 { 0 }
              else if target == VK_TEXTURE2 { 2 }
              else if target == VK_TEXTURE3 { 3 }
              else { 1 };
    if vk_state.currenttextures[tmu] == texnum {
        return;
    }
    vk_bind(texnum);
}

// ============================================================
// vk_texture_mode
// ============================================================

pub unsafe fn vk_texture_mode(string: &str) {
    let mut found = None;
    for (i, mode) in MODES.iter().enumerate() {
        if q_streq_nocase(mode.name, string) {
            found = Some(i);
            break;
        }
    }

    let idx = match found {
        Some(i) => i,
        None => {
            vid_printf(PRINT_ALL, "bad filter name\n");
            return;
        }
    };

    vk_filter_min = MODES[idx].minimize;
    vk_filter_max = MODES[idx].maximize;

    // change all the existing mipmap texture objects
    for i in 0..numgltextures as usize {
        let glt = &gltextures[i];
        if glt.r#type != ImageType::Pic && glt.r#type != ImageType::Sky {
            vk_bind(glt.texnum);
            qvk_tex_parameterf(VK_TEXTURE_2D, VK_TEXTURE_MIN_FILTER, vk_filter_min as f32);
            qvk_tex_parameterf(VK_TEXTURE_2D, VK_TEXTURE_MAG_FILTER, vk_filter_max as f32);
        }
    }
}

// ============================================================
// vk_texture_alpha_mode
// ============================================================

pub unsafe fn vk_texture_alpha_mode(string: &str) {
    for mode in VK_ALPHA_MODES.iter() {
        if q_streq_nocase(mode.name, string) {
            vk_tex_alpha_format_val = mode.mode;
            return;
        }
    }
    vid_printf(PRINT_ALL, "bad alpha texture mode name\n");
}

// ============================================================
// vk_texture_solid_mode
// ============================================================

pub unsafe fn vk_texture_solid_mode(string: &str) {
    for mode in VK_SOLID_MODES.iter() {
        if q_streq_nocase(mode.name, string) {
            vk_tex_solid_format_val = mode.mode;
            return;
        }
    }
    vid_printf(PRINT_ALL, "bad solid texture mode name\n");
}

// ============================================================
// vk_image_list_f
// ============================================================

pub unsafe fn vk_image_list_f() {
    vid_printf(PRINT_ALL, "------------------\n");
    let mut texels: i32 = 0;

    for i in 0..numgltextures as usize {
        let image = &gltextures[i];
        if image.texnum <= 0 {
            continue;
        }
        texels += image.upload_width * image.upload_height;
        let type_char = match image.r#type {
            ImageType::Skin => "M",
            ImageType::Sprite => "S",
            ImageType::Wall => "W",
            ImageType::Pic => "P",
            _ => " ",
        };
        // Convert name bytes to string
        let name_len = image.name.iter().position(|&b| b == 0).unwrap_or(MAX_QPATH);
        let name_str = std::str::from_utf8(&image.name[..name_len]).unwrap_or("<invalid>");
        vid_printf(PRINT_ALL, &format!("{} {:3} {:3}: {}\n", type_char, image.upload_width, image.upload_height, name_str));
    }
    vid_printf(PRINT_ALL, &format!("Total texel count (not counting mipmaps): {}\n", texels));
}

// ============================================================
// Scrap_AllocBlock
// ============================================================

pub unsafe fn scrap_alloc_block(w: i32, h: i32, x: &mut i32, y: &mut i32) -> i32 {
    for texnum in 0..MAX_SCRAPS {
        let mut best = BLOCK_HEIGHT as i32;

        for i in 0..(BLOCK_WIDTH as i32 - w) {
            let mut best2 = 0i32;
            let mut j = 0i32;
            while j < w {
                if scrap_allocated[texnum][(i + j) as usize] >= best {
                    break;
                }
                if scrap_allocated[texnum][(i + j) as usize] > best2 {
                    best2 = scrap_allocated[texnum][(i + j) as usize];
                }
                j += 1;
            }
            if j == w {
                *x = i;
                *y = best2;
                best = best2;
            }
        }

        if best + h > BLOCK_HEIGHT as i32 {
            continue;
        }

        for i in 0..w {
            scrap_allocated[texnum][(*x + i) as usize] = best + h;
        }

        return texnum as i32;
    }

    -1
}

// ============================================================
// Scrap_Upload
// ============================================================

pub unsafe fn scrap_upload() {
    scrap_uploads += 1;
    vk_bind(TEXNUM_SCRAPS);
    vk_upload8(scrap_texels[0].as_ptr(), BLOCK_WIDTH as i32, BLOCK_HEIGHT as i32, false, std::ptr::null_mut());
    scrap_dirty = 0;
}

// ============================================================
// Floodfill
// ============================================================

const FLOODFILL_FIFO_SIZE: usize = 0x1000;
const FLOODFILL_FIFO_MASK: usize = FLOODFILL_FIFO_SIZE - 1;

pub unsafe fn r_flood_fill_skin(skin: *mut u8, skinwidth: i32, skinheight: i32) {
    let fillcolor = *skin;

    let mut fifo_x = [0i16; FLOODFILL_FIFO_SIZE];
    let mut fifo_y = [0i16; FLOODFILL_FIFO_SIZE];
    let mut inpt: usize = 0;
    let mut outpt: usize = 0;

    // attempt to find opaque black
    let mut filledcolor: u8 = 0;
    for i in 0..256 {
        if d_8to24table[i] == 255 {
            filledcolor = i as u8;
            break;
        }
    }

    if fillcolor == filledcolor || fillcolor == 255 {
        return;
    }

    fifo_x[inpt] = 0;
    fifo_y[inpt] = 0;
    inpt = (inpt + 1) & FLOODFILL_FIFO_MASK;

    while outpt != inpt {
        let x = fifo_x[outpt] as i32;
        let y = fifo_y[outpt] as i32;
        let mut fdc = filledcolor;
        let pos = skin.offset((x + skinwidth * y) as isize);

        outpt = (outpt + 1) & FLOODFILL_FIFO_MASK;

        macro_rules! floodfill_step {
            ($off:expr, $dx:expr, $dy:expr) => {
                if *pos.offset($off) == fillcolor {
                    *pos.offset($off) = 255;
                    fifo_x[inpt] = (x + $dx) as i16;
                    fifo_y[inpt] = (y + $dy) as i16;
                    inpt = (inpt + 1) & FLOODFILL_FIFO_MASK;
                } else if *pos.offset($off) != 255 {
                    fdc = *pos.offset($off);
                }
            };
        }

        if x > 0                { floodfill_step!(-1, -1, 0); }
        if x < skinwidth - 1    { floodfill_step!(1, 1, 0); }
        if y > 0                { floodfill_step!(-(skinwidth as isize), 0, -1); }
        if y < skinheight - 1   { floodfill_step!(skinwidth as isize, 0, 1); }
        *skin.offset((x + skinwidth * y) as isize) = fdc;
    }
}

// ============================================================
// vk_resample_texture
// ============================================================

static mut p1: *mut u32 = std::ptr::null_mut();
static mut p2: *mut u32 = std::ptr::null_mut();

pub unsafe fn vk_resample_texture(
    in_data: *const u32, inwidth: i32, inheight: i32,
    out_data: *mut u32, outwidth: i32, outheight: i32,
) {
    // lazy alloc p1, p2
    // max_tsize should match the maximum texture dimension supported.
    // 2048 matches the original C code's MAX_TEXTURE_SIZE assumption.
    let max_sz = 2048;
    if p1.is_null() {
        let layout = std::alloc::Layout::from_size_align(max_sz * 4, 4).unwrap();
        p1 = std::alloc::alloc_zeroed(layout) as *mut u32;
        p2 = std::alloc::alloc_zeroed(layout) as *mut u32;
    }

    let fracstep = ((inwidth as u32).wrapping_mul(0x10000)) / outwidth as u32;

    let mut frac = fracstep >> 2;
    for i in 0..outwidth {
        *p1.offset(i as isize) = 4 * (frac >> 16);
        frac = frac.wrapping_add(fracstep);
    }

    frac = 3 * (fracstep >> 2);
    for i in 0..outwidth {
        *p2.offset(i as isize) = 4 * (frac >> 16);
        frac = frac.wrapping_add(fracstep);
    }

    let mut out = out_data;
    for i in 0..outheight {
        let inrow = in_data.offset((inwidth as isize) * ((i as f32 + 0.25) * inheight as f32 / outheight as f32) as isize);
        let inrow2 = in_data.offset((inwidth as isize) * ((i as f32 + 0.75) * inheight as f32 / outheight as f32) as isize);
        for j in 0..outwidth {
            let pix1 = (inrow as *const u8).offset(*p1.offset(j as isize) as isize);
            let pix2 = (inrow as *const u8).offset(*p2.offset(j as isize) as isize);
            let pix3 = (inrow2 as *const u8).offset(*p1.offset(j as isize) as isize);
            let pix4 = (inrow2 as *const u8).offset(*p2.offset(j as isize) as isize);

            let outp = out.offset(j as isize) as *mut u8;
            *outp.offset(0) = ((*pix1.offset(0) as u32 + *pix2.offset(0) as u32 + *pix3.offset(0) as u32 + *pix4.offset(0) as u32) >> 2) as u8;
            *outp.offset(1) = ((*pix1.offset(1) as u32 + *pix2.offset(1) as u32 + *pix3.offset(1) as u32 + *pix4.offset(1) as u32) >> 2) as u8;
            *outp.offset(2) = ((*pix1.offset(2) as u32 + *pix2.offset(2) as u32 + *pix3.offset(2) as u32 + *pix4.offset(2) as u32) >> 2) as u8;
            *outp.offset(3) = ((*pix1.offset(3) as u32 + *pix2.offset(3) as u32 + *pix3.offset(3) as u32 + *pix4.offset(3) as u32) >> 2) as u8;
        }
        out = out.offset(outwidth as isize);
    }
}

// ============================================================
// vk_light_scale_texture
// ============================================================

pub unsafe fn vk_light_scale_texture(in_data: *mut u8, inwidth: i32, inheight: i32, only_gamma: bool, bpp: i32) {
    let inc: isize = if bpp == 24 { 3 } else { 4 };
    let c = (inwidth * inheight) as isize;
    let mut p = in_data;

    if only_gamma {
        for _ in 0..c {
            *p.offset(0) = gammatable[*p.offset(0) as usize];
            *p.offset(1) = gammatable[*p.offset(1) as usize];
            *p.offset(2) = gammatable[*p.offset(2) as usize];
            p = p.offset(inc);
        }
    } else {
        for _ in 0..c {
            *p.offset(0) = gammatable[intensitytable[*p.offset(0) as usize] as usize];
            *p.offset(1) = gammatable[intensitytable[*p.offset(1) as usize] as usize];
            *p.offset(2) = gammatable[intensitytable[*p.offset(2) as usize] as usize];
            p = p.offset(inc);
        }
    }
}

// ============================================================
// vk_mipmap
// ============================================================

pub unsafe fn vk_mip_map(in_data: *mut u8, width: i32, height: i32) {
    let w = (width << 2) as isize;
    let h = (height >> 1) as isize;
    let mut out = in_data;
    let mut inp = in_data;

    for _ in 0..h {
        let mut j: isize = 0;
        while j < w {
            *out.offset(0) = ((*inp.offset(0) as u32 + *inp.offset(4) as u32 + *inp.offset(w) as u32 + *inp.offset(w + 4) as u32) >> 2) as u8;
            *out.offset(1) = ((*inp.offset(1) as u32 + *inp.offset(5) as u32 + *inp.offset(w + 1) as u32 + *inp.offset(w + 5) as u32) >> 2) as u8;
            *out.offset(2) = ((*inp.offset(2) as u32 + *inp.offset(6) as u32 + *inp.offset(w + 2) as u32 + *inp.offset(w + 6) as u32) >> 2) as u8;
            *out.offset(3) = ((*inp.offset(3) as u32 + *inp.offset(7) as u32 + *inp.offset(w + 3) as u32 + *inp.offset(w + 7) as u32) >> 2) as u8;
            out = out.offset(4);
            inp = inp.offset(8);
            j += 8;
        }
        inp = inp.offset(w); // skip a row
    }
}

// ============================================================
// vk_upload32 — Returns has_alpha (qboolean as i32)
// ============================================================

pub unsafe fn vk_upload32(
    data: *const u32, width: i32, height: i32,
    mipmap: bool, bpp: i32, image: *mut Image,
) -> i32 {
    let mut scaled_width = 1i32;
    while scaled_width < width { scaled_width <<= 1; }
    if crate::vk_rmain::VK_PICMIP.value != 0.0 && scaled_width > width && mipmap { scaled_width >>= 1; }

    let mut scaled_height = 1i32;
    while scaled_height < height { scaled_height <<= 1; }
    if crate::vk_rmain::VK_PICMIP.value != 0.0 && scaled_height > height && mipmap { scaled_height >>= 1; }

    if mipmap {
        scaled_width >>= crate::vk_rmain::VK_PICMIP.value as i32;
        scaled_height >>= crate::vk_rmain::VK_PICMIP.value as i32;
    }

    // clamp
    if scaled_width > 2048 { scaled_width = 2048; }
    if scaled_width < 1 { scaled_width = 1; }
    if scaled_height > 2048 { scaled_height = 2048; }
    if scaled_height < 1 { scaled_height = 1; }

    // scan for non-255 alpha
    let mut samples = vk_solid_format;
    if bpp != 24 {
        let c = (width * height) as usize;
        let scan = (data as *const u8).offset(3);
        for i in 0..c {
            if *scan.add(i * 4) != 255 {
                samples = vk_alpha_format;
                break;
            }
        }
    }

    let comp = if samples == vk_solid_format { vk_tex_solid_format_val } else { vk_tex_alpha_format_val };

    let mut scaled: *mut u32;
    let mut scaled_needs_free = false;

    if scaled_width == width && scaled_height == height {
        if !mipmap {
            qvk_tex_image2d(VK_TEXTURE_2D, 0, comp, scaled_width, scaled_height, 0, VK_RGBA as u32, VK_UNSIGNED_BYTE, data as *const u8);

            upload_width = scaled_width;
            upload_height = scaled_height;
            qvk_tex_parameterf(VK_TEXTURE_2D, VK_TEXTURE_MIN_FILTER, vk_filter_max as f32);
            qvk_tex_parameterf(VK_TEXTURE_2D, VK_TEXTURE_MAG_FILTER, vk_filter_max as f32);
            return if samples == vk_alpha_format { 1 } else { 0 };
        }
        scaled = data as *mut u32;
    } else {
        let layout = std::alloc::Layout::from_size_align((scaled_width * scaled_height * 4) as usize, 4).unwrap();
        scaled = std::alloc::alloc_zeroed(layout) as *mut u32;
        scaled_needs_free = true;
        vk_resample_texture(data, width, height, scaled, scaled_width, scaled_height);
    }

    if !image.is_null() && (*image).r#type != ImageType::Pic {
        let name_str = {
            let img_ref = &*image;
            let len = img_ref.name.iter().position(|&b| b == 0).unwrap_or(MAX_QPATH);
            std::str::from_utf8(&img_ref.name[..len]).unwrap_or("")
        };
        if !name_str.contains("fx/caustic") {
            vk_light_scale_texture(scaled as *mut u8, scaled_width, scaled_height, !mipmap, bpp);
        }
    }

    if vk_config.sgismipmap != 0 {
        for mode in MODES.iter() {
            if q_streq_nocase(mode.name, crate::vk_rmain::VK_SKYMIP.string) {
                qvk_tex_parameteri(VK_TEXTURE_2D, VK_TEXTURE_MIN_FILTER, mode.minimize);
                break;
            }
        }
        qvk_tex_parameteri(VK_TEXTURE_2D, VK_GENERATE_MIPMAP_SGIS, VK_TRUE);
    }

    if vk_config.anisotropy != 0 {
        // In C: vk_ext_texture_filter_anisotropic->value clamped to max_aniso
        let aniso_val = crate::vk_rmain::VK_EXT_TEXTURE_FILTER_ANISOTROPIC.value;
        let max_aniso = crate::vk_rmain::MAX_ANISO as f32;
        let aniso = if aniso_val > max_aniso { max_aniso } else { aniso_val };
        qvk_tex_parameterf(VK_TEXTURE_2D, VK_TEXTURE_MAX_ANISOTROPY_EXT, aniso);
    }

    qvk_tex_image2d(VK_TEXTURE_2D, 0, comp, scaled_width, scaled_height, 0, VK_RGBA as u32, VK_UNSIGNED_BYTE, scaled as *const u8);

    if mipmap && vk_config.sgismipmap == 0 {
        let mut miplevel = 0i32;
        let mut sw = scaled_width;
        let mut sh = scaled_height;
        while sw > 1 || sh > 1 {
            vk_mip_map(scaled as *mut u8, sw, sh);
            sw >>= 1;
            sh >>= 1;
            if sw < 1 { sw = 1; }
            if sh < 1 { sh = 1; }
            miplevel += 1;
            qvk_tex_image2d(VK_TEXTURE_2D, miplevel, comp, sw, sh, 0, VK_RGBA as u32, VK_UNSIGNED_BYTE, scaled as *const u8);
        }
    }

    upload_width = scaled_width;
    upload_height = scaled_height;

    qvk_tex_parameterf(VK_TEXTURE_2D, VK_TEXTURE_MIN_FILTER, if mipmap { vk_filter_min as f32 } else { vk_filter_max as f32 });
    qvk_tex_parameterf(VK_TEXTURE_2D, VK_TEXTURE_MAG_FILTER, vk_filter_max as f32);

    if scaled_needs_free {
        let layout = std::alloc::Layout::from_size_align((scaled_width * scaled_height * 4) as usize, 4).unwrap();
        std::alloc::dealloc(scaled as *mut u8, layout);
    }

    if samples == vk_alpha_format { 1 } else { 0 }
}

// ============================================================
// vk_upload8 — Returns has_alpha (qboolean as i32)
// ============================================================

pub unsafe fn vk_upload8(
    data: *const u8, width: i32, height: i32,
    mipmap: bool, image: *mut Image,
) -> i32 {
    let s = (width * height) as usize;
    let layout = std::alloc::Layout::from_size_align(s * 4, 4).unwrap();
    let trans = std::alloc::alloc_zeroed(layout) as *mut u32;

    for i in 0..s {
        let p = *data.add(i) as usize;
        *trans.add(i) = d_8to24table[p];

        if p == 255 {
            // transparent — scan around for another color to avoid alpha fringes
            let replacement = if i > width as usize && *data.add(i - width as usize) != 255 {
                *data.add(i - width as usize) as usize
            } else if i < s - width as usize && *data.add(i + width as usize) != 255 {
                *data.add(i + width as usize) as usize
            } else if i > 0 && *data.add(i - 1) != 255 {
                *data.add(i - 1) as usize
            } else if i < s - 1 && *data.add(i + 1) != 255 {
                *data.add(i + 1) as usize
            } else {
                0
            };
            // copy rgb components
            let src = &d_8to24table[replacement] as *const u32 as *const u8;
            let dst = trans.add(i) as *mut u8;
            *dst.offset(0) = *src.offset(0);
            *dst.offset(1) = *src.offset(1);
            *dst.offset(2) = *src.offset(2);
        }
    }

    let result = vk_upload32(trans, width, height, mipmap, 8, image);
    std::alloc::dealloc(trans as *mut u8, layout);
    result
}

// ============================================================
// vk_load_pic
// ============================================================

/// Helper to copy a Rust &str into Image.name [u8; MAX_QPATH]
unsafe fn set_image_name(image: *mut Image, name: &str) {
    let bytes = name.as_bytes();
    let len = bytes.len().min(MAX_QPATH - 1);
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), (*image).name.as_mut_ptr(), len);
    (*image).name[len] = 0;
}

/// Helper to get image name as &str
unsafe fn get_image_name(image: *const Image) -> &'static str {
    let img_ref = &*image;
    let len = img_ref.name.iter().position(|&b| b == 0).unwrap_or(MAX_QPATH);
    std::str::from_utf8(&img_ref.name[..len]).unwrap_or("")
}

pub unsafe fn vk_load_pic(
    name: &str, pic: *const u8,
    width: i32, height: i32,
    img_type: ImageType, bits: i32,
) -> *mut Image {
    // find a free image_t
    let mut i = 0i32;
    while i < numgltextures {
        if gltextures[i as usize].texnum == 0 {
            break;
        }
        i += 1;
    }
    if i == numgltextures {
        if numgltextures as usize == MAX_GLTEXTURES {
            com_error(ERR_DROP, "MAX_GLTEXTURES");
        }
        numgltextures += 1;
    }
    let image = &mut gltextures[i as usize] as *mut Image;

    if name.len() >= MAX_QPATH {
        com_error(ERR_DROP, &format!("Draw_LoadPic: \"{}\" is too long", name));
    }
    set_image_name(image, name);
    (*image).registration_sequence = registration_sequence();
    (*image).width = width;
    (*image).height = height;
    (*image).r#type = img_type;

    // load little pics into the scrap
    if (*image).r#type == ImageType::Pic && (*image).width < 64 && (*image).height < 64
        && bits == 8 {
            let mut x = 0i32;
            let mut y = 0i32;
            let texnum = scrap_alloc_block((*image).width, (*image).height, &mut x, &mut y);
            if texnum >= 0 {
                scrap_dirty = 1;

                let mut k = 0usize;
                for iy in 0..(*image).height {
                    for jx in 0..(*image).width {
                        scrap_texels[texnum as usize][(y + iy) as usize * BLOCK_WIDTH + (x + jx) as usize] = *pic.add(k);
                        k += 1;
                    }
                }
                (*image).texnum = TEXNUM_SCRAPS + texnum;
                (*image).has_alpha = 1;
                (*image).sl = (x as f32 + 0.01) / BLOCK_WIDTH as f32;
                (*image).sh = (x as f32 + (*image).width as f32 - 0.01) / BLOCK_WIDTH as f32;
                (*image).tl = (y as f32 + 0.01) / BLOCK_WIDTH as f32;
                (*image).th = (y as f32 + (*image).height as f32 - 0.01) / BLOCK_WIDTH as f32;
                return image;
            }
        }

    // nonscrap
    (*image).texnum = TEXNUM_IMAGES + i;
    vk_bind((*image).texnum);

    let mipmap = (*image).r#type != ImageType::Pic && (*image).r#type != ImageType::Sky;

    if bits == 8 {
        (*image).has_alpha = vk_upload8(pic, width, height, mipmap, image);
    } else {
        (*image).has_alpha = vk_upload32(pic as *const u32, width, height, mipmap, bits, image);
    }

    (*image).upload_width = upload_width;
    (*image).upload_height = upload_height;
    (*image).sl = 0.0;
    (*image).sh = 1.0;
    (*image).tl = 0.0;
    (*image).th = 1.0;

    image
}

// ============================================================
// LoadPCX — PCX image decoder (uses unified decoder from myq2-common)
// Returns (palette_indexed_pixels, width, height, Option<palette_768_bytes>)
// ============================================================

fn load_pcx(raw: &[u8]) -> Option<(Vec<u8>, u32, u32, Option<[u8; 768]>)> {
    myq2_common::qfiles::pcx_decode(raw).map(|result| {
        (result.pixels, result.width, result.height, Some(result.palette))
    })
}

// ============================================================
// LoadTGA — TGA image decoder (supports types 1,2,3,9,10,11)
// Returns (rgba_pixels, width, height)
// ============================================================

// ============================================================
// LoadPNG — decode a PNG image to RGBA pixels
// ============================================================

fn load_png(raw: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    load_image(raw, image::ImageFormat::Png)
}

fn load_tga(raw: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    load_image(raw, image::ImageFormat::Tga)
}

fn load_jpg(raw: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    load_image(raw, image::ImageFormat::Jpeg)
}

fn load_image(raw: &[u8], format: image::ImageFormat) -> Option<(Vec<u8>, u32, u32)> {
    let img = image::load_from_memory_with_format(raw, format).ok()?;
    let rgba = img.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    Some((rgba.into_raw(), width, height))
}

// ============================================================
// vk_load_wal — load a .wal texture
// ============================================================

unsafe fn vk_load_wal(name: &str) -> *mut Image {
    let data = match myq2_common::files::fs_load_file(name) {
        Some(d) => d,
        None => {
            vid_printf(PRINT_ALL, &format!("vk_find_image: can't load {}\n", name));
            return r_notexture;
        }
    };

    // miptex_t header: 32 bytes name, 4 bytes width, 4 bytes height, 4*4 bytes offsets
    if data.len() < 32 + 4 + 4 + 16 {
        vid_printf(PRINT_ALL, &format!("vk_find_image: can't load {}\n", name));
        return r_notexture;
    }

    let width = i32::from_le_bytes([data[32], data[33], data[34], data[35]]);
    let height = i32::from_le_bytes([data[36], data[37], data[38], data[39]]);
    let ofs = i32::from_le_bytes([data[40], data[41], data[42], data[43]]) as usize;

    if ofs + (width * height) as usize > data.len() {
        vid_printf(PRINT_ALL, &format!("vk_find_image: can't load {}\n", name));
        return r_notexture;
    }

    vk_load_pic(name, data[ofs..].as_ptr(), width, height, ImageType::Wall, 8)
}

// ============================================================
// vk_find_image — finds or loads the given image
// ============================================================

pub unsafe fn vk_find_image_impl(name: &str, img_type: ImageType) -> *mut Image {
    if name.is_empty() {
        return std::ptr::null_mut();
    }
    let len = name.len();
    if len < 5 {
        return std::ptr::null_mut();
    }

    // look for it
    for i in 0..numgltextures as usize {
        let img_name = get_image_name(&gltextures[i]);
        if img_name == name {
            gltextures[i].registration_sequence = registration_sequence();
            return &mut gltextures[i] as *mut Image;
        }
    }

    // load the pic from disk
    let ext = &name[len - 4..];

    if ext.eq_ignore_ascii_case(".pcx") {
        if let Some(raw) = myq2_common::files::fs_load_file(name) {
            if let Some((pixels, w, h, _palette)) = load_pcx(&raw) {
                return vk_load_pic(name, pixels.as_ptr(), w as i32, h as i32, img_type, 8);
            }
        }
        return std::ptr::null_mut();
    } else if ext.eq_ignore_ascii_case(".wal") {
        return vk_load_wal(name);
    } else if ext.eq_ignore_ascii_case(".tga") {
        if let Some(raw) = myq2_common::files::fs_load_file(name) {
            if let Some((pixels, w, h)) = load_tga(&raw) {
                return vk_load_pic(name, pixels.as_ptr(), w as i32, h as i32, img_type, 32);
            }
        }
        return std::ptr::null_mut();
    } else if ext.eq_ignore_ascii_case(".png") {
        if let Some(raw) = myq2_common::files::fs_load_file(name) {
            if let Some((pixels, w, h)) = load_png(&raw) {
                return vk_load_pic(name, pixels.as_ptr(), w as i32, h as i32, img_type, 32);
            }
        }
        return std::ptr::null_mut();
    } else if ext.eq_ignore_ascii_case(".jpg") {
        if let Some(raw) = myq2_common::files::fs_load_file(name) {
            if let Some((pixels, w, h)) = load_jpg(&raw) {
                return vk_load_pic(name, pixels.as_ptr(), w as i32, h as i32, img_type, 32);
            }
        }
        return std::ptr::null_mut();
    }

    // Try .jpeg extension (5 chars)
    if len >= 6 && name[len - 5..].eq_ignore_ascii_case(".jpeg") {
        if let Some(raw) = myq2_common::files::fs_load_file(name) {
            if let Some((pixels, w, h)) = load_jpg(&raw) {
                return vk_load_pic(name, pixels.as_ptr(), w as i32, h as i32, img_type, 32);
            }
        }
        return std::ptr::null_mut();
    }

    std::ptr::null_mut()
}

// ============================================================
// draw_find_pic — finds or loads a 2D pic by name
// ============================================================

/// Find a 2D picture by name.
/// If name doesn't start with '/' or '\', prepends "pics/" and appends ".pcx".
pub unsafe fn draw_find_pic(name: &str) -> *mut Image {
    if !name.starts_with('/') && !name.starts_with('\\') {
        let fullname = format!("pics/{}.pcx", name);
        vk_find_image(&fullname, ImageType::Pic)
    } else {
        vk_find_image(&name[1..], ImageType::Pic)
    }
}

// ============================================================
// R_RegisterSkin
// ============================================================

pub unsafe fn r_register_skin(name: &str) -> *mut Image {
    vk_find_image(name, ImageType::Skin)
}

// ============================================================
// vk_free_unused_images
// ============================================================

pub unsafe fn vk_free_unused_images_impl() {
    let reg_seq = registration_sequence();

    // never free r_notexture or particle textures
    if !r_notexture.is_null() {
        (*r_notexture).registration_sequence = reg_seq;
    }

    for i in 0..numgltextures as usize {
        if gltextures[i].registration_sequence == reg_seq {
            continue; // used this sequence
        }
        if gltextures[i].registration_sequence == 0 {
            continue; // free image_t slot
        }
        if gltextures[i].r#type == ImageType::Pic {
            continue; // don't free pics
        }
        // free it
        qvk_delete_textures(1, &gltextures[i].texnum);
        gltextures[i] = Image::default();
    }
}

// ============================================================
// Draw_GetPalette
// ============================================================

pub unsafe fn draw_get_palette() -> i32 {
    // Load the palette from pics/colormap.pcx via the filesystem.
    // The PCX file contains a 768-byte palette at the end (256 RGB triplets).
    // Full PCX decoding is not yet implemented, but we can extract the palette
    // from the raw file data: the last 769 bytes are 0x0C followed by 768 bytes of RGB.
    if let Some(data) = myq2_common::files::fs_load_file("pics/colormap.pcx") {
        if data.len() > 769 {
            let pal_start = data.len() - 768;
            // Verify the palette marker byte (0x0C) precedes the palette data
            if data[pal_start - 1] == 0x0C {
                let pal = &data[pal_start..];
                for i in 0..256 {
                    let r = pal[i * 3] as u32;
                    let g = pal[i * 3 + 1] as u32;
                    let b = pal[i * 3 + 2] as u32;
                    d_8to24table[i] = (255 << 24) | (b << 16) | (g << 8) | r;
                }
                // Entry 255 is transparent
                d_8to24table[255] &= 0x00FFFFFF; // alpha = 0
                return 0;
            }
        }
    }
    vid_printf(PRINT_ALL, "Draw_GetPalette: couldn't load pics/colormap.pcx\n");
    0
}

// ============================================================
// vk_init_images
// ============================================================

pub unsafe fn vk_init_images() {
    set_registration_sequence(1);

    // init intensity conversions
    // In C: intensity = Cvar_Get("intensity", "2", 0); vk_state.inverse_intensity = 1.0 / intensity->value;
    // Ensure the cvar exists with default "2", then read its current value.
    let _ = myq2_common::cvar::cvar_get("intensity", "2", 0);
    let intensity_val = myq2_common::cvar::cvar_variable_value("intensity");
    vk_state.inverse_intensity = 1.0 / intensity_val;

    draw_get_palette();

    // In C: vid_gamma->value
    let g = crate::vk_rmain::VID_GAMMA.value;

    for i in 0..256 {
        // gammatable
        if g == 1.0 {
            gammatable[i] = i as u8;
        } else {
            let inf = (255.0 * ((i as f32 + 0.5) / 255.5f32).powf(g) + 0.5) as i32;
            gammatable[i] = inf.clamp(0, 255) as u8;
        }

        // intensitytable
        let j = (i as f32 * intensity_val) as i32;
        intensitytable[i] = j.min(255) as u8;
    }
}

// ============================================================
// vk_shutdown_images
// ============================================================

pub unsafe fn vk_shutdown_images() {
    for i in 0..numgltextures as usize {
        if gltextures[i].registration_sequence == 0 {
            continue; // free image_t slot
        }
        // free it
        qvk_delete_textures(1, &gltextures[i].texnum);
        gltextures[i] = Image::default();
    }
}

// ============================================================
// Helper: registration_sequence accessor
// ============================================================

static mut REGISTRATION_SEQUENCE: i32 = 0;

unsafe fn registration_sequence() -> i32 {
    REGISTRATION_SEQUENCE
}

unsafe fn set_registration_sequence(val: i32) {
    REGISTRATION_SEQUENCE = val;
}

// ============================================================
// Parallel texture decoding infrastructure
// ============================================================

/// A decoded texture ready for GPU upload.
///
/// CPU-side decoding can happen in parallel; this struct holds
/// the decoded pixels until they can be uploaded to the GPU.
pub struct DecodedTexture {
    /// Texture name (path)
    pub name: String,
    /// Decoded pixel data (either paletted 8-bit or RGBA 32-bit)
    pub pixels: Vec<u8>,
    /// Texture width in pixels
    pub width: i32,
    /// Texture height in pixels
    pub height: i32,
    /// Image type (Skin, Sprite, Wall, Pic, Sky)
    pub img_type: ImageType,
    /// Bits per pixel (8 for paletted, 32 for RGBA)
    pub bits: i32,
}

/// Decode a single texture from disk (thread-safe, no GPU access).
///
/// Returns None if the texture cannot be loaded or decoded.
fn decode_single_texture(name: &str, img_type: ImageType) -> Option<DecodedTexture> {
    let len = name.len();
    if len < 5 {
        return None;
    }

    let ext = &name[len - 4..];
    let raw = myq2_common::files::fs_load_file(name)?;

    if ext.eq_ignore_ascii_case(".pcx") {
        let (pixels, w, h, _palette) = load_pcx(&raw)?;
        Some(DecodedTexture {
            name: name.to_string(),
            pixels,
            width: w as i32,
            height: h as i32,
            img_type,
            bits: 8,
        })
    } else if ext.eq_ignore_ascii_case(".wal") {
        // WAL format: 32 bytes name, 4 bytes width, 4 bytes height, 4*4 bytes offsets
        if raw.len() < 32 + 4 + 4 + 16 {
            return None;
        }
        let width = i32::from_le_bytes([raw[32], raw[33], raw[34], raw[35]]);
        let height = i32::from_le_bytes([raw[36], raw[37], raw[38], raw[39]]);
        let ofs = i32::from_le_bytes([raw[40], raw[41], raw[42], raw[43]]) as usize;

        if ofs + (width * height) as usize > raw.len() {
            return None;
        }

        Some(DecodedTexture {
            name: name.to_string(),
            pixels: raw[ofs..ofs + (width * height) as usize].to_vec(),
            width,
            height,
            img_type,
            bits: 8,
        })
    } else if ext.eq_ignore_ascii_case(".tga") {
        let (pixels, w, h) = load_tga(&raw)?;
        Some(DecodedTexture {
            name: name.to_string(),
            pixels,
            width: w as i32,
            height: h as i32,
            img_type,
            bits: 32,
        })
    } else if ext.eq_ignore_ascii_case(".png") {
        let (pixels, w, h) = load_png(&raw)?;
        Some(DecodedTexture {
            name: name.to_string(),
            pixels,
            width: w as i32,
            height: h as i32,
            img_type,
            bits: 32,
        })
    } else if ext.eq_ignore_ascii_case(".jpg") || (len >= 6 && name[len - 5..].eq_ignore_ascii_case(".jpeg")) {
        let (pixels, w, h) = load_jpg(&raw)?;
        Some(DecodedTexture {
            name: name.to_string(),
            pixels,
            width: w as i32,
            height: h as i32,
            img_type,
            bits: 32,
        })
    } else {
        None
    }
}

/// Decode multiple textures in parallel on all available CPU cores.
///
/// This function performs CPU-intensive texture decoding (PCX, TGA, PNG, WAL)
/// in parallel using rayon. The returned `DecodedTexture` structs can then
/// be uploaded to the GPU sequentially.
///
/// # Arguments
/// * `names` - Texture file paths to decode
/// * `img_type` - Image type for all textures in this batch
///
/// # Returns
/// A vector of successfully decoded textures (failed loads are filtered out).
pub fn decode_textures(names: &[String], img_type: ImageType) -> Vec<DecodedTexture> {
    names.par_iter()
        .filter_map(|name| decode_single_texture(name, img_type))
        .collect()
}

/// Decode multiple textures in parallel, preserving order with Option results.
///
/// Unlike `decode_textures`, this preserves the input order and
/// returns None for textures that failed to load, allowing the caller to
/// correlate results with input names.
///
/// # Arguments
/// * `names` - Texture file paths to decode
/// * `img_type` - Image type for all textures in this batch
///
/// # Returns
/// A vector of Option<DecodedTexture> in the same order as input names.
pub fn decode_textures_ordered(
    names: &[String],
    img_type: ImageType,
) -> Vec<Option<DecodedTexture>> {
    names.par_iter()
        .map(|name| decode_single_texture(name, img_type))
        .collect()
}

/// Upload previously decoded textures to the GPU (must be called from main thread).
///
/// This function takes the results of parallel CPU decoding and uploads them
/// to the GPU sequentially. OpenGL/GPU commands must execute on the main thread.
///
/// # Safety
/// Must be called from the main thread with OpenGL context current.
///
/// # Arguments
/// * `textures` - Pre-decoded textures to upload
///
/// # Returns
/// Pointers to the uploaded Image structs.
pub unsafe fn upload_decoded_textures(textures: Vec<DecodedTexture>) -> Vec<*mut Image> {
    textures.into_iter()
        .map(|tex| {
            vk_load_pic(
                &tex.name,
                tex.pixels.as_ptr(),
                tex.width,
                tex.height,
                tex.img_type,
                tex.bits,
            )
        })
        .collect()
}

/// Load multiple textures in parallel (convenience wrapper).
///
/// This combines parallel CPU decoding with sequential GPU upload,
/// providing the best of both worlds: parallel I/O and decoding
/// with proper GPU synchronization.
///
/// # Safety
/// Must be called from the main thread with OpenGL context current.
///
/// # Arguments
/// * `names` - Texture file paths to load
/// * `img_type` - Image type for all textures
///
/// # Returns
/// Pointers to loaded Image structs (null for failed loads).
pub unsafe fn load_textures(names: &[String], img_type: ImageType) -> Vec<*mut Image> {
    // Phase 1: Parallel CPU decoding (file I/O + format decoding)
    let decoded = decode_textures(names, img_type);

    // Phase 2: Sequential GPU upload (OpenGL must be on main thread)
    upload_decoded_textures(decoded)
}

/// Check if a texture is already in the cache.
///
/// Returns the cached Image pointer if found, or null if not cached.
///
/// # Safety
/// Accesses global texture array.
pub unsafe fn vk_find_cached_image(name: &str) -> *mut Image {
    if name.is_empty() || name.len() < 5 {
        return std::ptr::null_mut();
    }

    for i in 0..numgltextures as usize {
        let img_name = get_image_name(&gltextures[i]);
        if img_name == name {
            gltextures[i].registration_sequence = registration_sequence();
            return &mut gltextures[i] as *mut Image;
        }
    }

    std::ptr::null_mut()
}

/// Batch load textures with cache check.
///
/// This is the optimal function for loading multiple textures during map load:
/// 1. First checks cache for already-loaded textures (sequential)
/// 2. Parallel decodes textures not in cache
/// 3. Sequential GPU upload
///
/// # Safety
/// Must be called from the main thread with OpenGL context current.
///
/// # Arguments
/// * `names` - Texture file paths to load
/// * `img_type` - Image type for all textures
///
/// # Returns
/// Vector of Image pointers in the same order as input names (null for failed loads).
pub unsafe fn vk_batch_load_textures(
    names: &[String],
    img_type: ImageType,
) -> Vec<*mut Image> {
    let mut results: Vec<*mut Image> = vec![std::ptr::null_mut(); names.len()];
    let mut to_load: Vec<(usize, &String)> = Vec::new();

    // Phase 1: Check cache (sequential)
    for (idx, name) in names.iter().enumerate() {
        let cached = vk_find_cached_image(name);
        if !cached.is_null() {
            results[idx] = cached;
        } else {
            to_load.push((idx, name));
        }
    }

    if to_load.is_empty() {
        return results;
    }

    // Phase 2: Parallel decode uncached textures
    let names_to_load: Vec<String> = to_load.iter().map(|(_, n)| (*n).clone()).collect();
    let decoded = decode_textures_ordered(&names_to_load, img_type);

    // Phase 3: Sequential GPU upload and assign to results
    for ((idx, _), decoded_opt) in to_load.into_iter().zip(decoded.into_iter()) {
        if let Some(tex) = decoded_opt {
            let img_ptr = vk_load_pic(
                &tex.name,
                tex.pixels.as_ptr(),
                tex.width,
                tex.height,
                tex.img_type,
                tex.bits,
            );
            results[idx] = img_ptr;
        }
    }

    results
}

// ============================================================
// Pure helper functions (extracted for testability)
// ============================================================

/// Compute the next power-of-two dimension >= `n`.
///
/// This mirrors the scaling logic used in `vk_upload32`:
/// `while scaled < n { scaled <<= 1 }`
pub fn next_power_of_two(n: i32) -> i32 {
    let mut scaled = 1i32;
    while scaled < n {
        scaled <<= 1;
    }
    scaled
}

/// Count the number of mipmap levels for a given width and height.
///
/// Mipmap generation continues while either dimension is > 1,
/// halving each dimension each level (clamped to 1).
pub fn mipmap_level_count(width: i32, height: i32) -> i32 {
    let mut levels = 1i32;
    let mut w = width;
    let mut h = height;
    while w > 1 || h > 1 {
        w >>= 1;
        h >>= 1;
        if w < 1 { w = 1; }
        if h < 1 { h = 1; }
        levels += 1;
    }
    levels
}

/// Build a single d_8to24table entry from RGB values.
///
/// Format: 0xAARRGGBB stored as little-endian u32 = (A << 24) | (B << 16) | (G << 8) | R.
pub fn make_palette_entry(r: u8, g: u8, b: u8, a: u8) -> u32 {
    (a as u32) << 24 | (b as u32) << 16 | (g as u32) << 8 | (r as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // Constants
    // ============================================================

    #[test]
    fn test_scrap_constants() {
        assert_eq!(MAX_SCRAPS, 1);
        assert_eq!(BLOCK_WIDTH, 256);
        assert_eq!(BLOCK_HEIGHT, 256);
    }

    #[test]
    fn test_floodfill_fifo_size_is_power_of_two() {
        // FLOODFILL_FIFO_SIZE must be a power of 2 for the mask to work correctly
        assert_eq!(FLOODFILL_FIFO_SIZE, 0x1000);
        assert!(FLOODFILL_FIFO_SIZE.is_power_of_two());
        assert_eq!(FLOODFILL_FIFO_MASK, FLOODFILL_FIFO_SIZE - 1);
    }

    #[test]
    fn test_floodfill_fifo_mask_wrapping() {
        // The mask must correctly wrap indices
        assert_eq!(0 & FLOODFILL_FIFO_MASK, 0);
        assert_eq!(FLOODFILL_FIFO_SIZE & FLOODFILL_FIFO_MASK, 0);
        assert_eq!((FLOODFILL_FIFO_SIZE - 1) & FLOODFILL_FIFO_MASK, FLOODFILL_FIFO_SIZE - 1);
        assert_eq!((FLOODFILL_FIFO_SIZE + 1) & FLOODFILL_FIFO_MASK, 1);
    }

    // ============================================================
    // next_power_of_two
    // ============================================================

    #[test]
    fn test_next_power_of_two_exact() {
        // Already power of two
        assert_eq!(next_power_of_two(1), 1);
        assert_eq!(next_power_of_two(2), 2);
        assert_eq!(next_power_of_two(4), 4);
        assert_eq!(next_power_of_two(64), 64);
        assert_eq!(next_power_of_two(256), 256);
        assert_eq!(next_power_of_two(1024), 1024);
        assert_eq!(next_power_of_two(2048), 2048);
    }

    #[test]
    fn test_next_power_of_two_non_power() {
        // Not a power of two -> rounds up
        assert_eq!(next_power_of_two(3), 4);
        assert_eq!(next_power_of_two(5), 8);
        assert_eq!(next_power_of_two(7), 8);
        assert_eq!(next_power_of_two(9), 16);
        assert_eq!(next_power_of_two(100), 128);
        assert_eq!(next_power_of_two(200), 256);
        assert_eq!(next_power_of_two(500), 512);
        assert_eq!(next_power_of_two(1000), 1024);
    }

    #[test]
    fn test_next_power_of_two_typical_textures() {
        // Common texture sizes in Quake 2
        assert_eq!(next_power_of_two(320), 512);
        assert_eq!(next_power_of_two(240), 256);
        assert_eq!(next_power_of_two(480), 512);
        assert_eq!(next_power_of_two(640), 1024);
    }

    // ============================================================
    // mipmap_level_count
    // ============================================================

    #[test]
    fn test_mipmap_level_count_1x1() {
        assert_eq!(mipmap_level_count(1, 1), 1);
    }

    #[test]
    fn test_mipmap_level_count_power_of_two() {
        // 256x256 -> 128x128 -> 64x64 -> 32x32 -> 16x16 -> 8x8 -> 4x4 -> 2x2 -> 1x1
        // That's 9 levels
        assert_eq!(mipmap_level_count(256, 256), 9);
    }

    #[test]
    fn test_mipmap_level_count_2x2() {
        // 2x2 -> 1x1 = 2 levels
        assert_eq!(mipmap_level_count(2, 2), 2);
    }

    #[test]
    fn test_mipmap_level_count_4x4() {
        // 4x4 -> 2x2 -> 1x1 = 3 levels
        assert_eq!(mipmap_level_count(4, 4), 3);
    }

    #[test]
    fn test_mipmap_level_count_non_square() {
        // 256x64: levels continue until both dims are 1
        // 256x64 -> 128x32 -> 64x16 -> 32x8 -> 16x4 -> 8x2 -> 4x1 -> 2x1 -> 1x1
        // That's 9 levels
        assert_eq!(mipmap_level_count(256, 64), 9);
    }

    #[test]
    fn test_mipmap_level_count_1024x1024() {
        // 1024 -> 512 -> 256 -> 128 -> 64 -> 32 -> 16 -> 8 -> 4 -> 2 -> 1
        // 11 levels
        assert_eq!(mipmap_level_count(1024, 1024), 11);
    }

    // ============================================================
    // make_palette_entry
    // ============================================================

    #[test]
    fn test_make_palette_entry_opaque_white() {
        let entry = make_palette_entry(255, 255, 255, 255);
        assert_eq!(entry, 0xFFFFFFFF);
    }

    #[test]
    fn test_make_palette_entry_opaque_black() {
        let entry = make_palette_entry(0, 0, 0, 255);
        assert_eq!(entry, 0xFF000000);
    }

    #[test]
    fn test_make_palette_entry_transparent() {
        let entry = make_palette_entry(0, 0, 0, 0);
        assert_eq!(entry, 0x00000000);
    }

    #[test]
    fn test_make_palette_entry_red() {
        // R=255, G=0, B=0, A=255
        // In ABGR: (255 << 24) | (0 << 16) | (0 << 8) | 255 = 0xFF0000FF
        let entry = make_palette_entry(255, 0, 0, 255);
        assert_eq!(entry, 0xFF0000FF);
    }

    #[test]
    fn test_make_palette_entry_green() {
        let entry = make_palette_entry(0, 255, 0, 255);
        assert_eq!(entry, 0xFF00FF00);
    }

    #[test]
    fn test_make_palette_entry_blue() {
        let entry = make_palette_entry(0, 0, 255, 255);
        assert_eq!(entry, 0xFFFF0000);
    }

    #[test]
    fn test_make_palette_entry_matches_draw_get_palette_format() {
        // The format must match draw_get_palette: (255 << 24) | (b << 16) | (g << 8) | r
        let r = 100u8;
        let g = 150u8;
        let b = 200u8;
        let expected = (255u32 << 24) | ((b as u32) << 16) | ((g as u32) << 8) | (r as u32);
        assert_eq!(make_palette_entry(r, g, b, 255), expected);
    }

    #[test]
    fn test_palette_entry_255_transparency() {
        // Entry 255 in Quake 2 palette is transparent: alpha should be 0
        let entry = make_palette_entry(0, 0, 0, 0);
        // Check alpha byte is zero
        assert_eq!(entry & 0xFF000000, 0);
    }

    // ============================================================
    // Texture mode tables
    // ============================================================

    #[test]
    fn test_modes_table_length() {
        assert_eq!(MODES.len(), 6);
    }

    #[test]
    fn test_alpha_modes_table_length() {
        assert_eq!(VK_ALPHA_MODES.len(), 6);
    }

    #[test]
    fn test_solid_modes_table_length() {
        assert_eq!(VK_SOLID_MODES.len(), 6);
    }

    #[test]
    fn test_modes_have_valid_names() {
        // All mode names should start with "VK_"
        for mode in MODES.iter() {
            assert!(mode.name.starts_with("VK_"), "Mode name '{}' should start with VK_", mode.name);
        }
    }

    #[test]
    fn test_alpha_modes_default_entry() {
        // First entry should be "default" with mode 4
        assert_eq!(VK_ALPHA_MODES[0].name, "default");
        assert_eq!(VK_ALPHA_MODES[0].mode, 4);
    }

    #[test]
    fn test_solid_modes_default_entry() {
        // First entry should be "default" with mode 3
        assert_eq!(VK_SOLID_MODES[0].name, "default");
        assert_eq!(VK_SOLID_MODES[0].mode, 3);
    }

    // ============================================================
    // DecodedTexture struct
    // ============================================================

    #[test]
    fn test_decoded_texture_creation() {
        let dt = DecodedTexture {
            name: "test/texture.pcx".to_string(),
            pixels: vec![0u8; 64 * 64],
            width: 64,
            height: 64,
            img_type: ImageType::Wall,
            bits: 8,
        };
        assert_eq!(dt.name, "test/texture.pcx");
        assert_eq!(dt.pixels.len(), 64 * 64);
        assert_eq!(dt.width, 64);
        assert_eq!(dt.height, 64);
        assert_eq!(dt.bits, 8);
    }

    #[test]
    fn test_decoded_texture_rgba() {
        let dt = DecodedTexture {
            name: "test/texture.tga".to_string(),
            pixels: vec![0u8; 128 * 128 * 4],
            width: 128,
            height: 128,
            img_type: ImageType::Skin,
            bits: 32,
        };
        assert_eq!(dt.pixels.len(), 128 * 128 * 4);
        assert_eq!(dt.bits, 32);
    }
}
