// vk_rmisc.rs — Miscellaneous renderer routines
// Converted from: myq2-original/ref_gl/vk_rmisc.c
//
// 1-to-1 conversion: every C function has a Rust equivalent.
// GL calls are stub function calls via crate::vk_local::* (pending GL bindings).

#![allow(
    non_upper_case_globals,
    non_camel_case_types,
    non_snake_case,
    dead_code,
    unused_variables,
    unused_mut,
    clippy::excessive_precision,
    clippy::needless_return
)]

// Import only specific items to avoid name conflicts with vk_rmain re-exports
use crate::vk_local::{Image, ImageType,
    qvk_enable, qvk_disable, qvk_color4f,
    VK_TEXTURE_2D, VK_BLEND, VK_REPLACE, PT_MAX,
};
use crate::vk_rmain::{
    R_PARTICLETEXTURE, R_NOTEXTURE, VK_CONFIG, VK_STATE,
    VK_SWAPINTERVAL, VK_TEXTUREMODE, VK_TEXTUREALPHAMODE, VK_TEXTURESOLIDMODE,
    VID, VID_GAMMA, vid_printf,
    VK_SCREENSHOT_FORMAT, VK_SCREENSHOT_QUALITY,
};
use myq2_common::q_shared::PRINT_ALL;

// ============================================================
// notexture pattern (16x16)
// ============================================================
pub static NOTEXTURE: [[u8; 16]; 16] = [
    [1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1],
    [1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1],
    [1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1],
    [1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1],
    [1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1],
    [1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1],
    [1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1],
    [1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1],
    [1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1],
    [1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1],
    [1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1],
    [1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1],
    [1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1],
    [1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1],
    [1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1],
    [1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1],
];

// ============================================================
// R_InitParticleTexture
// ============================================================
pub fn r_init_particle_texture() {
    // SAFETY: single-threaded engine access pattern
    unsafe {
        let mut data = [[[0u8; 4]; 16]; 16];

        // ===========================================
        // PARTICLE TEXTURES
        // ===========================================
        for x in 0..16i32 {
            let xx = (x - 8) * (x - 8);
            for y in 0..16i32 {
                let mut alpha = 255 - 4 * (xx + (y - 8) * (y - 8));
                if alpha <= 0 {
                    alpha = 0;
                    data[y as usize][x as usize][0] = 0;
                    data[y as usize][x as usize][1] = 0;
                    data[y as usize][x as usize][2] = 0;
                } else {
                    data[y as usize][x as usize][0] = 255;
                    data[y as usize][x as usize][1] = 255;
                    data[y as usize][x as usize][2] = 255;
                }
                data[y as usize][x as usize][3] = alpha as u8;
            }
        }

        // Try to load named particle textures via Draw_FindPic
        let particle_names = [
            "particles/default",
            "particles/fire",
            "particles/smoke",
            "particles/bubble",
            "particles/blood",
        ];

        for (idx, name) in particle_names.iter().enumerate() {
            let pic = crate::vk_image::draw_find_pic(name);
            if !pic.is_null() {
                R_PARTICLETEXTURE[idx] = Some((*pic).clone());
            } else {
                R_PARTICLETEXTURE[idx] = None;
            }
        }

        // Fall back to generated texture for any that failed to load
        let data_flat: Vec<u8> = data.iter().flat_map(|row| row.iter().flat_map(|px| px.iter().copied())).collect();
        for x in 0..PT_MAX {
            if R_PARTICLETEXTURE[x].is_none() {
                // In C: vk_load_pic("***particle***", (byte*)data, 16, 16, it_sprite, 32)
                let pic_ptr = crate::vk_image::vk_load_pic(
                    "***particle***",
                    data_flat.as_ptr(),
                    16, 16,
                    ImageType::Sprite,
                    32,
                );
                if !pic_ptr.is_null() {
                    R_PARTICLETEXTURE[x] = Some((*pic_ptr).clone());
                }
            }
        }

        // ===========================================
        // NO_TEXTURE TEXTURE
        // ===========================================
        for x in 0..16usize {
            for y in 0..16usize {
                data[y][x][0] = NOTEXTURE[x & 3][y & 3] * 255;
                data[y][x][1] = 0;
                data[y][x][2] = 0;
                data[y][x][3] = 255;
            }
        }
        {
            let mut img = Image::default();
            let name_bytes = b"***r_notexture***";
            img.name[..name_bytes.len()].copy_from_slice(name_bytes);
            img.r#type = ImageType::Wall;
            img.width = 16;
            img.height = 16;
            img.upload_width = 16;
            img.upload_height = 16;
            img.sh = 1.0;
            img.th = 1.0;
            R_NOTEXTURE = Some(img);
        }
    }
}

// ============================================================
// vk_screenshot_f - R1Q2/Q2Pro format selection support
// Supports TGA, PNG, and JPG formats via gl_screenshot_format cvar
// JPEG quality controlled by gl_screenshot_quality cvar (0-100)
// ============================================================
pub fn vk_screen_shot_f() {
    // SAFETY: single-threaded engine access pattern
    unsafe {
        use std::fs;
        use std::path::Path;
        use image::{ImageBuffer, Rgb, codecs::jpeg::JpegEncoder};

        // Determine format from cvar
        let format_str = VK_SCREENSHOT_FORMAT.string.to_lowercase();
        let (ext, format) = match format_str.as_str() {
            "png" => ("png", ScreenshotFormat::Png),
            "jpg" | "jpeg" => ("jpg", ScreenshotFormat::Jpg),
            _ => ("tga", ScreenshotFormat::Tga),
        };

        // JPEG quality (0-100), default 85
        let jpeg_quality = (VK_SCREENSHOT_QUALITY.value as u8).clamp(1, 100);

        // create the scrnshots directory if it doesn't exist
        let gamedir = myq2_common::files::fs_gamedir();
        let scrnshot_dir = format!("{}/scrnshot", gamedir);
        let _ = fs::create_dir_all(&scrnshot_dir);

        // find a file name to save it to
        let mut picname = format!("quake00.{}", ext);
        let mut checkname = String::new();
        let mut found = false;

        for i in 0..=99i32 {
            let tens = (i / 10) as u8 + b'0';
            let ones = (i % 10) as u8 + b'0';
            picname = format!("quake{}{}.{}", tens as char, ones as char, ext);
            checkname = format!("{}/{}", scrnshot_dir, picname);
            if !Path::new(&checkname).exists() {
                found = true;
                break;
            }
        }

        if !found {
            vid_printf(PRINT_ALL, "SCR_ScreenShot_f: Couldn't create a file\n");
            return;
        }

        let width = VID.width as usize;
        let height = VID.height as usize;
        let pixel_count = width * height * 3;

        // Read pixels from framebuffer
        let mut pixels = vec![0u8; pixel_count];
        qvk_read_pixels(0, 0, width as i32, height as i32, &mut pixels);

        // apply gamma correction if necessary
        let vk_config = VK_CONFIG.as_ref();
        if vk_config.is_some_and(|c| c.gammaramp != 0) {
            let mut gamma_table = [0u8; 256];
            for i in 0..256 {
                let v = (255.0
                    * ((i as f64 + 0.5) * 0.0039138943248532289628180039138943)
                        .powf(VID_GAMMA.value as f64)
                    + 0.5) as i32;
                gamma_table[i] = v.clamp(0, 255) as u8;
            }

            for i in 0..pixel_count {
                pixels[i] = gamma_table[pixels[i] as usize];
            }
        }

        // Write based on format
        match format {
            ScreenshotFormat::Tga => {
                // TGA format (original behavior)
                use std::io::Write;

                let buf_size = pixel_count + 18;
                let mut buffer = vec![0u8; buf_size];

                // TGA header
                buffer[2] = 2; // uncompressed type
                buffer[12] = (width & 0xFF) as u8;
                buffer[13] = (width >> 8) as u8;
                buffer[14] = (height & 0xFF) as u8;
                buffer[15] = (height >> 8) as u8;
                buffer[16] = 24; // pixel size

                // Copy pixels and swap RGB to BGR for TGA
                for i in 0..width * height {
                    let src = i * 3;
                    let dst = 18 + i * 3;
                    buffer[dst] = pixels[src + 2];     // B
                    buffer[dst + 1] = pixels[src + 1]; // G
                    buffer[dst + 2] = pixels[src];     // R
                }

                match fs::File::create(&checkname) {
                    Ok(mut f) => {
                        if f.write_all(&buffer).is_err() {
                            vid_printf(PRINT_ALL, "SCR_ScreenShot_f: Could not write file\n");
                            return;
                        }
                    }
                    Err(_) => {
                        vid_printf(PRINT_ALL, "SCR_ScreenShot_f: Could not create file\n");
                        return;
                    }
                }
            }
            ScreenshotFormat::Png | ScreenshotFormat::Jpg => {
                // Create image buffer (flip vertically since GL reads bottom-up)
                let mut img: ImageBuffer<Rgb<u8>, Vec<u8>> =
                    ImageBuffer::new(width as u32, height as u32);

                for y in 0..height {
                    for x in 0..width {
                        let src_y = height - 1 - y; // Flip vertically
                        let idx = (src_y * width + x) * 3;
                        let pixel = Rgb([pixels[idx], pixels[idx + 1], pixels[idx + 2]]);
                        img.put_pixel(x as u32, y as u32, pixel);
                    }
                }

                let write_ok = match format {
                    ScreenshotFormat::Png => img.save(&checkname).is_ok(),
                    ScreenshotFormat::Jpg => {
                        // Use JPEG encoder with quality setting
                        match fs::File::create(&checkname) {
                            Ok(f) => {
                                let mut encoder = JpegEncoder::new_with_quality(f, jpeg_quality);
                                encoder.encode(
                                    img.as_raw(),
                                    width as u32,
                                    height as u32,
                                    image::ExtendedColorType::Rgb8,
                                ).is_ok()
                            }
                            Err(_) => false,
                        }
                    }
                    _ => unreachable!(),
                };

                if !write_ok {
                    vid_printf(PRINT_ALL, "SCR_ScreenShot_f: Could not write file\n");
                    return;
                }
            }
        }

        vid_printf(PRINT_ALL, &format!("Wrote {}\n", picname));
    }
}

/// Screenshot format enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScreenshotFormat {
    Tga,
    Png,
    Jpg,
}

/// qglReadPixels wrapper — reads pixel data from the framebuffer.
/// In C: qglReadPixels(x, y, w, h, VK_RGB, VK_UNSIGNED_BYTE, pixels)
fn qvk_read_pixels(x: i32, y: i32, w: i32, h: i32, pixels: &mut [u8]) {
    // SAFETY: pixels buffer is allocated by caller with sufficient size (w*h*3).
    // GL call requires valid context which is guaranteed when screenshot is taken.
    unsafe {
        crate::vk_bindings::ReadPixels(
            x,
            y,
            w,
            h,
            0x1907, // VK_RGB
            0x1401, // VK_UNSIGNED_BYTE
            pixels.as_mut_ptr() as *mut std::ffi::c_void,
        );
    }
}

// ============================================================
// vk_strings_f
// ============================================================
pub fn vk_strings_f() {
    // SAFETY: single-threaded engine access pattern
    unsafe {
        let vk_config = match VK_CONFIG.as_ref() {
            Some(c) => c,
            None => {
                vid_printf(PRINT_ALL, "GL config not initialized\n");
                return;
            }
        };
        let vendor = if vk_config.vendor_string.is_null() { "<null>" } else { std::str::from_utf8_unchecked(std::ffi::CStr::from_ptr(vk_config.vendor_string as *const i8).to_bytes()) };
        let renderer = if vk_config.renderer_string.is_null() { "<null>" } else { std::str::from_utf8_unchecked(std::ffi::CStr::from_ptr(vk_config.renderer_string as *const i8).to_bytes()) };
        let version = if vk_config.version_string.is_null() { "<null>" } else { std::str::from_utf8_unchecked(std::ffi::CStr::from_ptr(vk_config.version_string as *const i8).to_bytes()) };
        let extensions = if vk_config.extensions_string.is_null() { "<null>" } else { std::str::from_utf8_unchecked(std::ffi::CStr::from_ptr(vk_config.extensions_string as *const i8).to_bytes()) };
        vid_printf(PRINT_ALL, &format!("VK_VENDOR: {}\n", vendor));
        vid_printf(PRINT_ALL, &format!("VK_RENDERER: {}\n", renderer));
        vid_printf(PRINT_ALL, &format!("VK_VERSION: {}\n", version));
        vid_printf(PRINT_ALL, &format!("VK_EXTENSIONS: {}\n", extensions));
    }
}

// ============================================================
// vk_set_default_state
// ============================================================
pub fn vk_set_default_state() {
    // SAFETY: single-threaded engine access pattern
    unsafe {
        qvk_enable(VK_TEXTURE_2D);

        // GL_ALPHA_TEST enable removed — alpha testing handled by GLSL discard

        qvk_disable(0x0B71); // VK_DEPTH_TEST
        qvk_disable(0x0B44); // VK_CULL_FACE
        qvk_disable(VK_BLEND);
        // GL_FOG disable removed — fog handled by GLSL shaders

        qvk_color4f(1.0, 1.0, 1.0, 1.0);

        vk_texture_mode(VK_TEXTUREMODE.string);
        vk_texture_alpha_mode(VK_TEXTUREALPHAMODE.string);
        vk_texture_solid_mode(VK_TEXTURESOLIDMODE.string);

        vk_tex_env(VK_REPLACE as u32);

        vk_update_swap_interval();
    }
}

// ============================================================
// vk_update_swap_interval
// ============================================================
pub fn vk_update_swap_interval() {
    // SAFETY: single-threaded engine access pattern
    unsafe {
        if VK_SWAPINTERVAL.modified {
            VK_SWAPINTERVAL.modified = false;

            let vk_state = match VK_STATE.as_ref() {
                Some(s) => s,
                None => return,
            };

            if vk_state.stereo_enabled == 0 {
                // On Windows: qwglSwapIntervalEXT(vk_swapinterval->value)
                // Placeholder -- would call platform-specific swap interval
            }
        }
    }
}

// ============================================================
// vk_texture_mode / vk_texture_alpha_mode / vk_texture_solid_mode
// Delegates to the real implementations in vk_image.
// ============================================================
unsafe fn vk_texture_mode(mode: &str) { crate::vk_image::vk_texture_mode(mode); }
unsafe fn vk_texture_alpha_mode(mode: &str) { crate::vk_image::vk_texture_alpha_mode(mode); }
unsafe fn vk_texture_solid_mode(mode: &str) { crate::vk_image::vk_texture_solid_mode(mode); }
unsafe fn vk_tex_env(mode: u32) { crate::vk_image::vk_tex_env_impl(mode as i32); }
unsafe fn vk_bind(texnum: i32) { crate::vk_image::vk_bind_impl(texnum); }
