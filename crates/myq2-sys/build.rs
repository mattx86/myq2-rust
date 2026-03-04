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

/// Generate a .ico file with multiple sizes for high-DPI support.
///
/// Uses the Yamagi Quake II icon SVG path data for accurate shape:
/// - Dark navy background circle
/// - Q2 logo mark (horseshoe + two prongs) from SVG bezier curves
/// - Metallic blue gradient with specular highlight
#[cfg(target_os = "windows")]
fn generate_ico_file(path: &str) {
    let sizes = [16u32, 24, 32, 48, 64, 128, 256];
    let mut images: Vec<(u32, Vec<u8>)> = Vec::new();

    for &size in &sizes {
        let rgba = render_q2_icon(size);
        images.push((size, rgba));
    }

    write_ico(path, &images);
}

/// Signed distance to a filled polygon (Inigo Quilez algorithm).
/// Negative inside, positive outside.
#[cfg(target_os = "windows")]
fn sd_polygon(px: f32, py: f32, v: &[(f32, f32)]) -> f32 {
    let n = v.len();
    let mut d = (px - v[0].0) * (px - v[0].0) + (py - v[0].1) * (py - v[0].1);
    let mut s = 1.0f32;
    let mut j = n - 1;
    for i in 0..n {
        let ex = v[j].0 - v[i].0;
        let ey = v[j].1 - v[i].1;
        let wx = px - v[i].0;
        let wy = py - v[i].1;
        let dot_ee = ex * ex + ey * ey;
        let h = if dot_ee > 0.0 {
            ((wx * ex + wy * ey) / dot_ee).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let dx = wx - ex * h;
        let dy = wy - ey * h;
        d = d.min(dx * dx + dy * dy);
        let c1 = py >= v[i].1;
        let c2 = py < v[j].1;
        let c3 = ex * wy > ey * wx;
        if (c1 && c2 && c3) || (!c1 && !c2 && !c3) {
            s = -s;
        }
        j = i;
    }
    s * d.sqrt()
}

/// Flatten cubic bezier segments into polygon vertices in normalized [-1,1] space.
/// SVG viewBox is 0..256: x→(x/128-1), y→(1-y/128).
#[cfg(target_os = "windows")]
fn flatten_beziers(start: (f32, f32), segs: &[[f32; 6]], steps: usize) -> Vec<(f32, f32)> {
    let mut verts = Vec::with_capacity(segs.len() * steps);
    let mut cur = start;
    for seg in segs {
        let p0 = (cur.0 / 128.0 - 1.0, 1.0 - cur.1 / 128.0);
        let p1 = (seg[0] / 128.0 - 1.0, 1.0 - seg[1] / 128.0);
        let p2 = (seg[2] / 128.0 - 1.0, 1.0 - seg[3] / 128.0);
        let p3 = (seg[4] / 128.0 - 1.0, 1.0 - seg[5] / 128.0);
        for i in 0..steps {
            let t = i as f32 / steps as f32;
            let u = 1.0 - t;
            verts.push((
                u*u*u*p0.0 + 3.0*u*u*t*p1.0 + 3.0*u*t*t*p2.0 + t*t*t*p3.0,
                u*u*u*p0.1 + 3.0*u*u*t*p1.1 + 3.0*u*t*t*p2.1 + t*t*t*p3.1,
            ));
        }
        cur = (seg[4], seg[5]);
    }
    verts
}

/// Render the Q2 logo icon at a given size, returning RGBA pixel data.
/// Uses the Yamagi Quake II icon SVG path for an accurate shape.
#[cfg(target_os = "windows")]
fn render_q2_icon(size: u32) -> Vec<u8> {
    // SVG path data from Yamagi Q2 icon (viewBox 0 0 256 256).
    // Outer boundary: 108 cubic bezier segments. [cp1x,cp1y, cp2x,cp2y, endx,endy]
    #[rustfmt::skip]
    const OUTER: [[f32; 6]; 108] = [
        [166.83,4.67, 167.73,4.65, 168.64,4.63],
        [178.90,10.42, 189.31,16.64, 196.76,25.99],
        [202.91,32.89, 208.08,40.69, 211.69,49.21],
        [213.80,53.95, 216.37,58.73, 216.49,64.05],
        [216.44,66.82, 217.86,69.27, 218.62,71.86],
        [219.55,74.35, 219.26,77.06, 219.47,79.67],
        [220.20,81.23, 220.50,82.93, 220.61,84.64],
        [220.12,86.00, 219.69,87.37, 219.31,88.76],
        [220.18,90.67, 220.56,92.74, 220.72,94.82],
        [219.07,96.87, 217.56,99.03, 216.06,101.18],
        [216.90,101.99, 217.74,102.80, 218.59,103.62],
        [215.42,115.29, 210.62,126.65, 203.57,136.54],
        [200.30,141.66, 195.38,145.40, 191.95,150.41],
        [191.17,150.48, 189.60,150.62, 188.81,150.69],
        [188.57,148.95, 188.32,147.23, 188.03,145.50],
        [187.66,147.36, 187.29,149.23, 186.84,151.07],
        [186.64,152.83, 187.20,155.17, 185.33,156.22],
        [180.25,159.98, 174.71,163.11, 169.09,166.01],
        [162.14,169.11, 154.91,171.57, 147.60,173.67],
        [148.96,180.02, 147.39,186.43, 147.83,192.83],
        [148.07,202.36, 145.50,211.59, 144.49,221.00],
        [142.76,228.57, 142.03,236.31, 141.34,244.03],
        [141.15,246.67, 140.38,249.23, 139.30,251.64],
        [138.93,250.74, 138.20,248.93, 137.83,248.03],
        [134.82,234.62, 134.97,220.74, 131.94,207.33],
        [131.55,203.53, 130.98,199.76, 130.59,195.96],
        [130.24,193.12, 133.45,190.66, 131.84,187.87],
        [130.20,183.87, 130.71,179.49, 130.72,175.28],
        [128.60,175.25, 126.49,175.23, 124.37,175.21],
        [123.92,179.97, 124.79,184.81, 123.64,189.50],
        [123.15,191.31, 123.61,193.15, 123.91,194.94],
        [123.78,198.93, 124.00,202.95, 123.20,206.89],
        [122.19,211.89, 120.63,216.82, 120.46,221.94],
        [120.19,226.99, 118.67,231.88, 118.46,236.94],
        [118.25,242.16, 117.68,247.44, 115.91,252.39],
        [114.18,249.81, 114.07,246.66, 113.66,243.68],
        [113.11,239.43, 111.77,235.31, 111.54,231.02],
        [111.21,223.62, 109.49,216.37, 109.00,208.98],
        [107.85,203.54, 106.97,198.03, 107.22,192.44],
        [107.81,192.41, 109.00,192.34, 109.59,192.31],
        [108.66,191.02, 107.39,189.80, 107.39,188.08],
        [106.49,183.06, 106.92,177.94, 106.69,172.87],
        [103.15,171.96, 99.54,171.23, 96.17,169.80],
        [95.66,168.37, 95.41,166.87, 95.09,165.39],
        [92.46,165.52, 89.87,166.39, 87.23,166.18],
        [81.05,163.47, 75.83,159.10, 69.95,155.86],
        [68.88,153.58, 67.80,151.30, 66.56,149.11],
        [66.94,147.92, 67.33,146.74, 67.71,145.56],
        [66.67,146.78, 65.98,148.85, 64.00,148.69],
        [59.22,146.06, 55.94,141.54, 52.53,137.42],
        [49.29,133.32, 45.72,129.17, 44.35,124.01],
        [44.16,122.24, 46.39,121.72, 47.35,120.61],
        [44.34,120.99, 41.23,119.84, 40.06,116.85],
        [35.34,106.14, 36.30,94.11, 34.37,82.78],
        [34.69,82.39, 35.33,81.62, 35.65,81.23],
        [34.48,70.99, 39.38,61.65, 41.75,52.00],
        [42.12,51.55, 42.86,50.66, 43.22,50.21],
        [43.75,47.97, 44.40,45.71, 45.83,43.85],
        [47.59,41.44, 48.35,38.42, 50.41,36.23],
        [53.63,32.69, 55.81,28.37, 58.92,24.74],
        [59.48,24.60, 60.59,24.30, 61.15,24.15],
        [61.31,23.60, 61.62,22.48, 61.78,21.92],
        [65.10,19.13, 68.22,15.94, 72.25,14.16],
        [77.49,9.56, 83.79,5.09, 91.13,5.58],
        [90.56,9.16, 86.88,10.29, 84.14,11.82],
        [71.67,18.23, 61.44,28.94, 55.82,41.81],
        [54.06,45.90, 52.11,49.93, 50.90,54.24],
        [49.00,58.58, 49.30,63.42, 48.26,67.98],
        [47.05,72.28, 48.80,76.64, 48.21,81.01],
        [48.36,84.15, 49.02,87.42, 47.42,90.33],
        [46.71,90.45, 45.30,90.68, 44.59,90.80],
        [46.53,91.42, 48.48,92.03, 50.42,92.66],
        [50.45,94.06, 50.49,95.47, 50.53,96.87],
        [54.67,104.94, 57.62,113.68, 63.05,121.03],
        [64.87,123.15, 66.84,125.14, 68.67,127.26],
        [70.74,129.95, 73.97,131.28, 76.41,133.57],
        [78.47,135.53, 80.67,137.35, 82.98,139.03],
        [83.96,140.93, 85.05,142.78, 86.32,144.51],
        [88.47,142.34, 91.20,144.18, 93.56,144.82],
        [95.62,146.03, 97.80,147.02, 100.05,147.85],
        [100.57,149.50, 101.11,151.14, 101.66,152.78],
        [103.55,151.87, 105.42,150.97, 107.31,150.08],
        [107.29,148.98, 107.27,147.88, 107.26,146.77],
        [107.71,146.30, 108.62,145.36, 109.07,144.89],
        [108.64,144.43, 107.77,143.51, 107.34,143.06],
        [107.64,134.90, 107.51,126.69, 105.27,118.78],
        [102.29,117.83, 99.27,117.03, 96.25,116.23],
        [96.33,114.98, 96.43,113.73, 96.54,112.48],
        [105.70,113.68, 115.10,114.31, 124.25,112.70],
        [130.18,111.99, 136.05,113.79, 141.99,113.39],
        [148.02,113.44, 154.39,111.15, 160.18,113.77],
        [158.05,117.99, 152.11,115.60, 149.59,119.45],
        [146.20,124.71, 148.85,131.37, 146.86,137.05],
        [147.72,141.23, 147.75,145.52, 147.65,149.77],
        [158.44,148.05, 167.98,142.09, 176.61,135.68],
        [181.54,131.62, 186.54,127.47, 190.18,122.16],
        [192.57,120.06, 194.69,117.71, 195.93,114.73],
        [196.49,114.59, 197.63,114.31, 198.20,114.16],
        [199.57,108.17, 202.51,102.70, 204.36,96.87],
        [205.77,90.29, 207.53,83.71, 207.60,76.94],
        [208.82,76.14, 210.05,75.33, 211.29,74.54],
        [210.40,74.52, 208.63,74.47, 207.75,74.45],
        [207.10,70.66, 206.73,66.83, 206.54,63.00],
        [206.23,56.65, 203.14,50.87, 202.46,44.59],
        [199.20,41.58, 198.16,37.10, 195.34,33.78],
        [191.61,28.83, 188.17,23.55, 183.19,19.73],
        [177.24,15.10, 171.50,9.90, 164.38,7.14],
        [164.77,6.52, 165.55,5.30, 165.94,4.69],
    ];
    // Inner cutout: 7 cubic bezier segments
    #[rustfmt::skip]
    const INNER: [[f32; 6]; 7] = [
        [125.79,122.33, 124.75,125.06, 124.41,127.95],
        [123.87,131.89, 124.65,135.87, 124.21,139.82],
        [123.75,143.61, 123.61,147.43, 123.41,151.25],
        [126.06,151.29, 128.70,151.32, 131.35,151.35],
        [131.28,144.57, 131.15,137.79, 131.31,131.01],
        [131.52,127.09, 130.38,123.27, 129.27,119.55],
        [128.41,119.56, 127.54,119.58, 126.68,119.59],
    ];

    // Flatten bezier curves into polygon vertices (4 subdivisions per segment)
    let mut outer_poly = flatten_beziers((165.94, 4.69), &OUTER, 4);
    let mut inner_poly = flatten_beziers((126.68, 119.59), &INNER, 4);

    // Widen the gap between the two "II" prongs so they read at small sizes.
    // Apply uniform spread along the full prong length (top to bottom).
    for v in outer_poly.iter_mut() {
        if v.1 < 0.15 && v.0.abs() < 0.25 {
            let offset = 0.06;
            if v.0 >= 0.0 { v.0 += offset; } else { v.0 -= offset; }
        }
    }
    // Also widen the inner cutout (gap between prong tops)
    for v in inner_poly.iter_mut() {
        let offset = 0.06;
        if v.0 >= 0.0 { v.0 += offset; } else { v.0 -= offset; }
    }

    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let px_size = 2.0 / size as f32;
    let aa = px_size * 1.5;

    for y in 0..size {
        for x in 0..size {
            let nx = (x as f32 + 0.5) / size as f32 * 2.0 - 1.0;
            let ny = 1.0 - (y as f32 + 0.5) / size as f32 * 2.0;
            let idx = ((y * size + x) * 4) as usize;

            // Q2 logo: outer shape minus inner cutout
            let d = sd_polygon(nx, ny, &outer_poly)
                .max(-sd_polygon(nx, ny, &inner_poly));

            // Outer glow / dark outline for contrast on any background
            let glow_a = ((aa * 2.0 - d) / (aa * 2.0)).clamp(0.0, 1.0)
                       - ((-d) / aa).clamp(0.0, 1.0);
            if glow_a > 0.01 {
                alpha_blend(
                    &mut rgba[idx..idx + 4],
                    5.0, 10.0, 30.0,
                    glow_a * 180.0,
                );
            }

            let shape_a = ((-d) / aa).clamp(0.0, 1.0);

            if shape_a > 0.0 {
                // Vertical gradient (0 at prong tips bottom, 1 at horseshoe top)
                let t = ((ny + 0.97) / 1.93).clamp(0.0, 1.0);

                // Brighter metallic blue gradient for punch
                let mut r = lerp(20.0, 110.0, t);
                let mut g = lerp(50.0, 190.0, t);
                let mut b = lerp(130.0, 255.0, t);

                // Strong specular highlight on upper horseshoe
                let sdx = nx;
                let sdy = ny - 0.34;
                let spec_dist = (sdx * sdx + sdy * sdy).sqrt();
                let spec = ((0.35 - spec_dist) / 0.35).clamp(0.0, 1.0).powf(2.0) * 0.6;
                r += spec * 200.0;
                g += spec * 180.0;
                b += spec * 120.0;

                // Bright edge outline for definition at small sizes
                let inner_d = (-d).clamp(0.0, 0.03);
                let edge = (1.0 - inner_d / 0.03) * 0.35;
                r += edge * 120.0;
                g += edge * 150.0;
                b += edge * 100.0;

                alpha_blend(
                    &mut rgba[idx..idx + 4],
                    r.clamp(0.0, 255.0),
                    g.clamp(0.0, 255.0),
                    b.clamp(0.0, 255.0),
                    shape_a * 255.0,
                );
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
