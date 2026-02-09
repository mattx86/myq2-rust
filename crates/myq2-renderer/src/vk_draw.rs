// vk_draw.rs -- 2D drawing functions
// Converted from: myq2-original/ref_gl/vk_draw.c

#![allow(dead_code, unused_variables, unused_mut, unused_imports, non_upper_case_globals,
         clippy::too_many_arguments, unused_unsafe, static_mut_refs)]

use crate::vk_local::*;
use crate::vk_image;
use crate::vk_rmain::{vid_printf, MODERN};
use crate::modern::RenderPath;
use myq2_common::q_shared::{MAX_QPATH, PRINT_ALL};

// ============================================================
// Module state
// ============================================================

static mut draw_chars: *mut Image = std::ptr::null_mut();

extern "C" {
    // These are defined in vk_image
}

// ============================================================
// Draw_InitLocal
// ============================================================

/// Load console characters (don't bilerp characters).
pub unsafe fn draw_init_local() {
    draw_chars = vk_find_image("pics/conchars.pcx", ImageType::Pic);
    if !draw_chars.is_null() {
        vk_bind((*draw_chars).texnum);
        qvk_tex_parameterf(VK_TEXTURE_2D, VK_TEXTURE_MIN_FILTER, VK_NEAREST as f32);
        qvk_tex_parameterf(VK_TEXTURE_2D, VK_TEXTURE_MAG_FILTER, VK_NEAREST as f32);
    }
}

// ============================================================
// Draw_Char
// ============================================================

pub unsafe fn draw_char(x: i32, y: i32, num: i32) {
    // SAFETY: MODERN is always initialized before drawing begins
    MODERN.as_mut().unwrap().draw_char(x, y, num);
}

// ============================================================
// Draw_GetPicSize
// ============================================================

pub unsafe fn draw_get_pic_size(w: &mut i32, h: &mut i32, pic: &str) {
    let gl = crate::vk_image::draw_find_pic(pic);
    if gl.is_null() {
        *w = -1;
        *h = -1;
        return;
    }
    *w = (*gl).width;
    *h = (*gl).height;
}

// ============================================================
// Draw_StretchPic
// ============================================================

pub unsafe fn draw_stretch_pic(x: i32, y: i32, w: i32, h: i32, pic: &str) {
    // SAFETY: MODERN is always initialized before drawing begins
    MODERN.as_mut().unwrap().draw_stretch_pic(x, y, w, h, pic);
}

// ============================================================
// Draw_Pic
// ============================================================

pub unsafe fn draw_pic(x: i32, y: i32, pic: &str) {
    // SAFETY: MODERN is always initialized before drawing begins
    MODERN.as_mut().unwrap().draw_pic(x, y, pic);
}

// ============================================================
// Draw_TileClear
// ============================================================

pub unsafe fn draw_tile_clear(x: i32, y: i32, w: i32, h: i32, pic: &str) {
    // SAFETY: MODERN is always initialized before drawing begins
    MODERN.as_mut().unwrap().draw_tile_clear(x, y, w, h, pic);
}

// ============================================================
// Draw_Fill
// ============================================================

pub unsafe fn draw_fill(x: i32, y: i32, w: i32, h: i32, c: i32, alpha: f32) {
    // SAFETY: MODERN is always initialized before drawing begins
    MODERN.as_mut().unwrap().draw_fill(x, y, w, h, c, alpha);
}

// ============================================================
// Draw_FadeScreen
// ============================================================

pub unsafe fn draw_fade_screen() {
    // SAFETY: MODERN is always initialized before drawing begins
    MODERN.as_mut().unwrap().draw_fade_screen();
}

// ============================================================
// Draw_StretchRaw
// ============================================================

pub unsafe fn draw_stretch_raw(
    x: i32, y: i32, w: i32, h: i32,
    cols: i32, rows: i32, data: *const u8,
) {
    // Convert raw pointer to slice for the modern renderer
    let data_len = (cols * rows) as usize;
    // SAFETY: data points to valid pixel data of size cols*rows from cinematic decoder
    let data_slice = std::slice::from_raw_parts(data, data_len);
    // SAFETY: MODERN is always initialized before drawing begins
    MODERN.as_mut().unwrap().draw_stretch_raw(x, y, w, h, cols, rows, data_slice);
}
