// Build script for myq2-sys
// Adds Vulkan SDK library path for linking and embeds the application icon.

fn main() {
    // Add Vulkan SDK library path on Windows
    #[cfg(target_os = "windows")]
    {
        if let Ok(vulkan_sdk) = std::env::var("VULKAN_SDK") {
            println!("cargo:rustc-link-search=native={}/Lib", vulkan_sdk);
        } else {
            // Common installation paths
            let paths = [
                "C:/VulkanSDK/1.4.341.1/Lib",
                "C:/VulkanSDK/1.3.296.0/Lib",
                "C:/VulkanSDK/1.3.280.0/Lib",
            ];
            for path in &paths {
                if std::path::Path::new(path).exists() {
                    println!("cargo:rustc-link-search=native={}", path);
                    break;
                }
            }
        }
    }

    // Link the Vulkan loader library
    #[cfg(target_os = "windows")]
    println!("cargo:rustc-link-lib=vulkan-1");

    #[cfg(target_os = "linux")]
    {
        if let Ok(vulkan_sdk) = std::env::var("VULKAN_SDK") {
            println!("cargo:rustc-link-search=native={}/lib", vulkan_sdk);
        }
        // Common system library paths on Debian/Ubuntu x86_64
        println!("cargo:rustc-link-search=native=/usr/lib/x86_64-linux-gnu");
        println!("cargo:rustc-link-lib=vulkan");
    }

    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-lib=vulkan");

    // Generate the application icon and embed it in the Windows executable
    #[cfg(target_os = "windows")]
    {
        let out_dir = std::env::var("OUT_DIR").unwrap();
        let ico_path = format!("{}/myq2.ico", out_dir);

        // Generate the .ico file with multiple sizes
        generate_ico_file(&ico_path);

        // Embed icon into the Windows executable resource table
        let mut res = winresource::WindowsResource::new();
        res.set_icon(&ico_path);
        res.set("ProductName", "MyQ2 Rust");
        res.set("FileDescription", "MyQ2 - Quake 2 Engine (Rust Port)");
        if let Err(e) = res.compile() {
            eprintln!("Warning: Failed to embed Windows resource: {}", e);
        }
    }
}

/// Generate a .ico file with 16x16, 32x32, 48x48, and 64x64 images.
///
/// The icon is a green "Q" with "II" overlaid, matching the classic Quake 2 style:
/// - Dark background
/// - Green ring (the "Q" shape) with a diagonal tail
/// - Roman numeral "II" centered inside
#[cfg(target_os = "windows")]
fn generate_ico_file(path: &str) {
    let sizes = [16u32, 32, 48, 64];
    let mut images: Vec<(u32, Vec<u8>)> = Vec::new();

    for &size in &sizes {
        let rgba = render_q2_icon(size);
        images.push((size, rgba));
    }

    write_ico(path, &images);
}

/// Render the Q2 icon at a given size, returning RGBA pixel data.
#[cfg(target_os = "windows")]
fn render_q2_icon(size: u32) -> Vec<u8> {
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let center = size as f32 / 2.0;
    let scale = size as f32 / 64.0; // Normalize to 64px reference

    // Q ring parameters
    let outer_r = center - 3.0 * scale;
    let ring_thick = 8.0 * scale;
    let inner_r = outer_r - ring_thick;
    let bg_r = center - 1.0 * scale;

    // Colors - classic Quake 2 green
    let bg = [12u8, 16, 12];            // Dark green-black
    let q_green = [0u8, 200, 40];       // Quake 2 green
    let q_bright = [80u8, 255, 100];    // Bright green highlight
    let q_dark = [0u8, 120, 20];        // Dark green for outline/shadow

    // Q tail parameters
    let tail_angle: f32 = std::f32::consts::FRAC_PI_4;
    let tail_cos = tail_angle.cos();
    let tail_sin = tail_angle.sin();
    let tail_width = 4.5 * scale;

    // "II" parameters
    let ii_height = ring_thick * 1.5;
    let ii_bar_width = 2.2 * scale;
    let ii_gap = 3.0 * scale; // gap between the two bars
    let ii_y_center = center - 0.5 * scale; // slightly above center

    for y in 0..size {
        for x in 0..size {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let dx = px - center;
            let dy = py - center;
            let dist = (dx * dx + dy * dy).sqrt();
            let idx = ((y * size + x) * 4) as usize;

            // Background circle
            if dist < bg_r {
                let aa = (bg_r - dist).clamp(0.0, 1.5) / 1.5;
                rgba[idx] = bg[0];
                rgba[idx + 1] = bg[1];
                rgba[idx + 2] = bg[2];
                rgba[idx + 3] = (aa * 255.0) as u8;
            }

            // Q ring
            let ring_outer_aa = (outer_r - dist).clamp(0.0, 1.5) / 1.5;
            let ring_inner_aa = (dist - inner_r).clamp(0.0, 1.5) / 1.5;
            let ring_alpha = ring_outer_aa.min(ring_inner_aa);

            if ring_alpha > 0.0 {
                let light = ((-dx - dy) / (center * 2.0) + 0.5).clamp(0.15, 1.0);
                let r = lerp(q_green[0] as f32, q_bright[0] as f32, (light - 0.4) * 0.8);
                let g = lerp(q_green[1] as f32, q_bright[1] as f32, (light - 0.4) * 0.8);
                let b = lerp(q_green[2] as f32, q_bright[2] as f32, (light - 0.4) * 0.8);
                alpha_blend(&mut rgba[idx..idx + 4], r, g, b, ring_alpha * 255.0);
            }

            // Q tail (diagonal from lower ring area toward bottom-right)
            let tail_proj = dx * tail_cos + dy * tail_sin;
            let tail_perp = (-dx * tail_sin + dy * tail_cos).abs();

            if tail_proj > inner_r * 0.2 && tail_proj < outer_r + 7.0 * scale && tail_perp < tail_width {
                let aa_perp = ((tail_width - tail_perp) / (1.5 * scale)).clamp(0.0, 1.0);
                let aa_end = ((outer_r + 7.0 * scale - tail_proj) / (2.0 * scale)).clamp(0.0, 1.0);
                let aa_start = ((tail_proj - inner_r * 0.2) / (2.0 * scale)).clamp(0.0, 1.0);
                let tail_alpha = aa_perp * aa_end * aa_start;

                if tail_alpha > 0.01 {
                    let r = lerp(q_dark[0] as f32, q_green[0] as f32, aa_perp);
                    let g = lerp(q_dark[1] as f32, q_green[1] as f32, aa_perp);
                    let b = lerp(q_dark[2] as f32, q_green[2] as f32, aa_perp);
                    alpha_blend(&mut rgba[idx..idx + 4], r, g, b, tail_alpha * 255.0);
                }
            }

            // "II" Roman numeral inside the Q ring
            let fy = py - ii_y_center;
            if fy.abs() < ii_height / 2.0 && dist < inner_r - 1.0 * scale {
                // Left bar of II
                let left_bar_x = center - ii_gap / 2.0 - ii_bar_width / 2.0;
                let left_dist = (px - left_bar_x).abs();
                // Right bar of II
                let right_bar_x = center + ii_gap / 2.0 + ii_bar_width / 2.0;
                let right_dist = (px - right_bar_x).abs();

                let bar_aa_left = ((ii_bar_width / 2.0 - left_dist) / (1.0 * scale)).clamp(0.0, 1.0);
                let bar_aa_right = ((ii_bar_width / 2.0 - right_dist) / (1.0 * scale)).clamp(0.0, 1.0);
                let bar_aa_y = ((ii_height / 2.0 - fy.abs()) / (1.0 * scale)).clamp(0.0, 1.0);
                let bar_alpha = bar_aa_left.max(bar_aa_right) * bar_aa_y;

                // Top/bottom serifs for each bar
                let serif_height = 1.5 * scale;
                let serif_extra = 1.5 * scale;
                let near_top = (ii_height / 2.0 - fy.abs()) < serif_height;

                if near_top {
                    // Widen the bars at top and bottom (serifs)
                    let left_serif = ((ii_bar_width / 2.0 + serif_extra - left_dist) / (1.0 * scale)).clamp(0.0, 1.0);
                    let right_serif = ((ii_bar_width / 2.0 + serif_extra - right_dist) / (1.0 * scale)).clamp(0.0, 1.0);
                    let serif_y_aa = ((serif_height - (ii_height / 2.0 - fy.abs()).abs()) / (1.0 * scale)).clamp(0.0, 1.0);
                    let serif_alpha = left_serif.max(right_serif) * bar_aa_y * serif_y_aa;

                    if serif_alpha > bar_alpha && serif_alpha > 0.01 {
                        let light = 0.7 + 0.3 * (1.0 - fy.abs() / (ii_height / 2.0));
                        let r = q_bright[0] as f32 * light;
                        let g = q_bright[1] as f32 * light;
                        let b = q_bright[2] as f32 * light;
                        alpha_blend(&mut rgba[idx..idx + 4], r, g, b, serif_alpha * 255.0);
                    }
                }

                if bar_alpha > 0.01 {
                    // Slight vertical gradient on the bars
                    let light = 0.7 + 0.3 * (1.0 - fy.abs() / (ii_height / 2.0));
                    let r = q_bright[0] as f32 * light;
                    let g = q_bright[1] as f32 * light;
                    let b = q_bright[2] as f32 * light;
                    alpha_blend(&mut rgba[idx..idx + 4], r, g, b, bar_alpha * 255.0);
                }
            }
        }
    }

    rgba
}

#[cfg(target_os = "windows")]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

#[cfg(target_os = "windows")]
fn alpha_blend(pixel: &mut [u8], r: f32, g: f32, b: f32, a: f32) {
    let old_a = pixel[3] as f32 / 255.0;
    let new_a = a / 255.0;
    let out_a = new_a + old_a * (1.0 - new_a);
    if out_a > 0.001 {
        pixel[0] = ((r * new_a + pixel[0] as f32 * old_a * (1.0 - new_a)) / out_a).clamp(0.0, 255.0) as u8;
        pixel[1] = ((g * new_a + pixel[1] as f32 * old_a * (1.0 - new_a)) / out_a).clamp(0.0, 255.0) as u8;
        pixel[2] = ((b * new_a + pixel[2] as f32 * old_a * (1.0 - new_a)) / out_a).clamp(0.0, 255.0) as u8;
        pixel[3] = (out_a * 255.0).clamp(0.0, 255.0) as u8;
    }
}

/// Write a multi-size .ico file.
/// Each image is stored as an uncompressed 32-bit BGRA BMP in the ICO container.
#[cfg(target_os = "windows")]
fn write_ico(path: &str, images: &[(u32, Vec<u8>)]) {
    use std::io::Write;

    let count = images.len() as u16;
    let header_size = 6 + count as usize * 16; // ICO header + directory entries

    // Calculate offsets for each image's BMP data
    let mut offsets = Vec::new();
    let mut offset = header_size as u32;
    for (size, _) in images {
        let bmp_size = 40 + size * size * 4; // BITMAPINFOHEADER + pixel data
        offsets.push(offset);
        offset += bmp_size;
    }

    let mut buf: Vec<u8> = Vec::new();

    // ICO header
    buf.extend_from_slice(&0u16.to_le_bytes()); // reserved
    buf.extend_from_slice(&1u16.to_le_bytes()); // type = ICO
    buf.extend_from_slice(&count.to_le_bytes()); // count

    // Directory entries
    for (i, (size, rgba)) in images.iter().enumerate() {
        let w = if *size >= 256 { 0u8 } else { *size as u8 };
        let h = w;
        let bmp_data_size = 40u32 + size * size * 4;
        buf.push(w);                                        // width
        buf.push(h);                                        // height
        buf.push(0);                                        // color count (0 = no palette)
        buf.push(0);                                        // reserved
        buf.extend_from_slice(&1u16.to_le_bytes());         // color planes
        buf.extend_from_slice(&32u16.to_le_bytes());        // bits per pixel
        buf.extend_from_slice(&bmp_data_size.to_le_bytes()); // size of BMP data
        buf.extend_from_slice(&offsets[i].to_le_bytes());   // offset to BMP data
        let _ = rgba; // used below
    }

    // BMP data for each image (BITMAPINFOHEADER + bottom-up BGRA pixels)
    for (size, rgba) in images {
        // BITMAPINFOHEADER (40 bytes)
        buf.extend_from_slice(&40u32.to_le_bytes());        // header size
        buf.extend_from_slice(&(*size as i32).to_le_bytes()); // width
        buf.extend_from_slice(&((*size as i32) * 2).to_le_bytes()); // height (2x for XOR+AND mask)
        buf.extend_from_slice(&1u16.to_le_bytes());         // planes
        buf.extend_from_slice(&32u16.to_le_bytes());        // bits per pixel
        buf.extend_from_slice(&0u32.to_le_bytes());         // compression (none)
        buf.extend_from_slice(&(size * size * 4).to_le_bytes()); // image size
        buf.extend_from_slice(&0i32.to_le_bytes());         // x pixels per meter
        buf.extend_from_slice(&0i32.to_le_bytes());         // y pixels per meter
        buf.extend_from_slice(&0u32.to_le_bytes());         // colors used
        buf.extend_from_slice(&0u32.to_le_bytes());         // important colors

        // Pixel data: BMP is bottom-up, and uses BGRA byte order
        for y in (0..*size).rev() {
            for x in 0..*size {
                let idx = ((y * size + x) * 4) as usize;
                buf.push(rgba[idx + 2]); // B
                buf.push(rgba[idx + 1]); // G
                buf.push(rgba[idx]);     // R
                buf.push(rgba[idx + 3]); // A
            }
        }
    }

    let mut f = std::fs::File::create(path).expect("Failed to create icon file");
    f.write_all(&buf).expect("Failed to write icon file");
}
