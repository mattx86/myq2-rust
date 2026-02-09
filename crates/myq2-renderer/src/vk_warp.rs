// vk_warp.rs — Sky and water polygon warping
// Converted from: myq2-original/ref_gl/vk_warp.c

use crate::vk_local::*;
use crate::vk_rmain::vid_printf;
use myq2_common::q_shared::*;

// ============================================================
// MyQ2 build options (from myq2opts.h)
// ============================================================
pub const SKYBOX_SIZE: f32 = 4600.0;
// DO_WATER_WAVES and DO_REFLECTIVE_WATER are defined in vk_local.rs

// ============================================================
// Constants
// ============================================================

const SUBDIVIDE_SIZE: f32 = 64.0;
const ON_EPSILON: f32 = 0.1;
const MAX_CLIP_VERTS: usize = 64;

const SIDE_FRONT: i32 = 0;
const SIDE_BACK: i32 = 1;
const SIDE_ON: i32 = 2;

// ============================================================
// Module globals
// ============================================================

pub static mut detailtexture: *mut Image = std::ptr::null_mut();
pub static mut caustic_texture: *mut Image = std::ptr::null_mut();

/// Load the detail texture based on the r_detailtexture cvar value (1-8).
/// Selects fx/detail{N}.png where N is the cvar value.
/// Call on init and when the cvar is modified.
///
/// SAFETY: Must be called from the main thread. Accesses global renderer state.
pub unsafe fn load_detail_texture() {
    let val = crate::vk_rmain::R_DETAILTEXTURE.value as i32;
    if val >= 1 && val <= 8 {
        let path = format!("fx/detail{}.png", val);
        detailtexture = vk_find_image(&path, ImageType::Wall);
        if detailtexture.is_null() {
            vid_printf(PRINT_ALL, &format!("Warning: could not load {}\n", path));
        }
    } else {
        detailtexture = std::ptr::null_mut();
    }
}

/// Load the caustic texture (fx/caustic.png) for underwater surface overlay.
/// Call on renderer init.
///
/// SAFETY: Must be called from the main thread. Accesses global renderer state.
pub unsafe fn load_caustic_texture() {
    caustic_texture = vk_find_image("fx/caustic.png", ImageType::Wall);
    if caustic_texture.is_null() {
        vid_printf(PRINT_ALL, "Warning: could not load fx/caustic.png\n");
    }
}

pub static mut skyname: [u8; MAX_QPATH] = [0; MAX_QPATH];
pub static mut skyrotate: f32 = 0.0;
pub static mut skyaxis: Vec3 = [0.0; 3];
pub static mut sky_images: [*mut Image; 6] = [std::ptr::null_mut(); 6];

pub static mut warpface: *mut MSurface = std::ptr::null_mut();

pub static mut c_sky: i32 = 0;

pub static mut skymins: [[f32; 6]; 2] = [[0.0; 6]; 2];
pub static mut skymaxs: [[f32; 6]; 2] = [[0.0; 6]; 2];
pub static mut sky_min: f32 = 0.0;
pub static mut sky_max: f32 = 0.0;

// ============================================================
// Turbulent sine table (from warpsin.h)
// ============================================================

pub static R_TURBSIN: [f32; 256] = [
    0.0, 0.19633, 0.392541, 0.588517, 0.784137, 0.979285, 1.17384, 1.3677,
    1.56072, 1.75281, 1.94384, 2.1337, 2.32228, 2.50945, 2.69512, 2.87916,
    3.06147, 3.24193, 3.42044, 3.59689, 3.77117, 3.94319, 4.11282, 4.27998,
    4.44456, 4.60647, 4.76559, 4.92185, 5.07515, 5.22538, 5.37247, 5.51632,
    5.65685, 5.79398, 5.92761, 6.05767, 6.18408, 6.30677, 6.42566, 6.54068,
    6.65176, 6.75883, 6.86183, 6.9607, 7.05537, 7.14579, 7.23191, 7.31368,
    7.39104, 7.46394, 7.53235, 7.59623, 7.65552, 7.71021, 7.76025, 7.80562,
    7.84628, 7.88222, 7.91341, 7.93984, 7.96148, 7.97832, 7.99036, 7.99759,
    8.0, 7.99759, 7.99036, 7.97832, 7.96148, 7.93984, 7.91341, 7.88222,
    7.84628, 7.80562, 7.76025, 7.71021, 7.65552, 7.59623, 7.53235, 7.46394,
    7.39104, 7.31368, 7.23191, 7.14579, 7.05537, 6.9607, 6.86183, 6.75883,
    6.65176, 6.54068, 6.42566, 6.30677, 6.18408, 6.05767, 5.92761, 5.79398,
    5.65685, 5.51632, 5.37247, 5.22538, 5.07515, 4.92185, 4.76559, 4.60647,
    4.44456, 4.27998, 4.11282, 3.94319, 3.77117, 3.59689, 3.42044, 3.24193,
    3.06147, 2.87916, 2.69512, 2.50945, 2.32228, 2.1337, 1.94384, 1.75281,
    1.56072, 1.3677, 1.17384, 0.979285, 0.784137, 0.588517, 0.392541, 0.19633,
    9.79717e-16, -0.19633, -0.392541, -0.588517, -0.784137, -0.979285, -1.17384, -1.3677,
    -1.56072, -1.75281, -1.94384, -2.1337, -2.32228, -2.50945, -2.69512, -2.87916,
    -3.06147, -3.24193, -3.42044, -3.59689, -3.77117, -3.94319, -4.11282, -4.27998,
    -4.44456, -4.60647, -4.76559, -4.92185, -5.07515, -5.22538, -5.37247, -5.51632,
    -5.65685, -5.79398, -5.92761, -6.05767, -6.18408, -6.30677, -6.42566, -6.54068,
    -6.65176, -6.75883, -6.86183, -6.9607, -7.05537, -7.14579, -7.23191, -7.31368,
    -7.39104, -7.46394, -7.53235, -7.59623, -7.65552, -7.71021, -7.76025, -7.80562,
    -7.84628, -7.88222, -7.91341, -7.93984, -7.96148, -7.97832, -7.99036, -7.99759,
    -8.0, -7.99759, -7.99036, -7.97832, -7.96148, -7.93984, -7.91341, -7.88222,
    -7.84628, -7.80562, -7.76025, -7.71021, -7.65552, -7.59623, -7.53235, -7.46394,
    -7.39104, -7.31368, -7.23191, -7.14579, -7.05537, -6.9607, -6.86183, -6.75883,
    -6.65176, -6.54068, -6.42566, -6.30677, -6.18408, -6.05767, -5.92761, -5.79398,
    -5.65685, -5.51632, -5.37247, -5.22538, -5.07515, -4.92185, -4.76559, -4.60647,
    -4.44456, -4.27998, -4.11282, -3.94319, -3.77117, -3.59689, -3.42044, -3.24193,
    -3.06147, -2.87916, -2.69512, -2.50945, -2.32228, -2.1337, -1.94384, -1.75281,
    -1.56072, -1.3677, -1.17384, -0.979285, -0.784137, -0.588517, -0.392541, -0.19633,
];

const TURBSCALE: f32 = 256.0 / (2.0 * std::f32::consts::PI);

// ============================================================
// Sky clip planes
// ============================================================

pub static SKYCLIP: [[f32; 3]; 6] = [
    [1.0, 1.0, 0.0],
    [1.0, -1.0, 0.0],
    [0.0, -1.0, 1.0],
    [0.0, 1.0, 1.0],
    [1.0, 0.0, 1.0],
    [-1.0, 0.0, 1.0],
];

// 1 = s, 2 = t, 3 = 2048
pub static ST_TO_VEC: [[i32; 3]; 6] = [
    [3, -1, 2],
    [-3, 1, 2],
    [1, 3, 2],
    [-1, -3, 2],
    [-2, -1, 3],  // 0 degrees yaw, look straight up
    [2, -1, -3],  // look straight down
];

// s = [0]/[2], t = [1]/[2]
pub static VEC_TO_ST: [[i32; 3]; 6] = [
    [-2, 3, 1],
    [2, 3, -1],
    [1, 3, 2],
    [-1, 3, -2],
    [-2, -1, 3],
    [-2, 1, -3],
];

pub static SKYTEXORDER: [usize; 6] = [0, 2, 1, 3, 4, 5];
static SUF: [&str; 6] = ["rt", "bk", "lf", "ft", "up", "dn"];

// ============================================================
// Polygon subdivision
// ============================================================

/// Compute axis-aligned bounds for a set of vertices.
pub fn bound_poly(numverts: usize, verts: &[f32], mins: &mut Vec3, maxs: &mut Vec3) {
    mins[0] = 9999.0;
    mins[1] = 9999.0;
    mins[2] = 9999.0;
    maxs[0] = -9999.0;
    maxs[1] = -9999.0;
    maxs[2] = -9999.0;

    let mut idx = 0;
    for _i in 0..numverts {
        for j in 0..3 {
            if verts[idx] < mins[j] {
                mins[j] = verts[idx];
            }
            if verts[idx] > maxs[j] {
                maxs[j] = verts[idx];
            }
            idx += 1;
        }
    }
}

/// Recursively subdivide a polygon for warp effects.
///
/// # Safety
/// Accesses global warpface pointer and allocates polys via Hunk_Alloc equivalent.
pub unsafe fn subdivide_polygon(numverts: usize, verts: &mut [f32]) {
    if numverts > 60 {
        vid_printf(ERR_DROP, &format!("numverts = {}", numverts));
        return;
    }

    let mut mins = [0.0f32; 3];
    let mut maxs = [0.0f32; 3];
    bound_poly(numverts, verts, &mut mins, &mut maxs);

    for i in 0..3 {
        let m_raw = (mins[i] + maxs[i]) * 0.5;
        let m = SUBDIVIDE_SIZE * (m_raw / SUBDIVIDE_SIZE + 0.5).floor();
        if maxs[i] - m < 8.0 {
            continue;
        }
        if m - mins[i] < 8.0 {
            continue;
        }

        // cut it
        let mut dist = [0.0f32; 65];
        for j in 0..numverts {
            dist[j] = verts[j * 3 + i] - m;
        }

        // wrap cases
        dist[numverts] = dist[0];
        // copy first vertex to end position
        verts[numverts * 3] = verts[0];
        verts[numverts * 3 + 1] = verts[1];
        verts[numverts * 3 + 2] = verts[2];

        let mut front = [[0.0f32; 3]; 64];
        let mut back = [[0.0f32; 3]; 64];
        let mut f = 0usize;
        let mut b = 0usize;

        for j in 0..numverts {
            let v_off = j * 3;

            if dist[j] >= 0.0 {
                front[f] = [verts[v_off], verts[v_off + 1], verts[v_off + 2]];
                f += 1;
            }
            if dist[j] <= 0.0 {
                back[b] = [verts[v_off], verts[v_off + 1], verts[v_off + 2]];
                b += 1;
            }
            if dist[j] == 0.0 || dist[j + 1] == 0.0 {
                continue;
            }
            if (dist[j] > 0.0) != (dist[j + 1] > 0.0) {
                // clip point
                let frac = dist[j] / (dist[j] - dist[j + 1]);
                let v_next = (j + 1) * 3;
                for k in 0..3 {
                    let e = verts[v_off + k] + frac * (verts[v_next + k] - verts[v_off + k]);
                    front[f][k] = e;
                    back[b][k] = e;
                }
                f += 1;
                b += 1;
            }
        }

        // flatten front/back into flat arrays and recurse
        let mut front_flat = vec![0.0f32; f * 3];
        for j in 0..f {
            front_flat[j * 3] = front[j][0];
            front_flat[j * 3 + 1] = front[j][1];
            front_flat[j * 3 + 2] = front[j][2];
        }
        let mut back_flat = vec![0.0f32; b * 3];
        for j in 0..b {
            back_flat[j * 3] = back[j][0];
            back_flat[j * 3 + 1] = back[j][1];
            back_flat[j * 3 + 2] = back[j][2];
        }

        subdivide_polygon(f, &mut front_flat);
        subdivide_polygon(b, &mut back_flat);
        return;
    }

    // add a point in the center to help keep warp valid
    let poly = hunk_alloc_glpoly(numverts + 2);
    (*poly).next = (*warpface).polys;
    (*warpface).polys = poly;
    (*poly).numverts = (numverts + 2) as i32;

    let mut total = [0.0f32; 3];
    let mut total_s: f32 = 0.0;
    let mut total_t: f32 = 0.0;

    let wf = &*warpface;
    let texinfo = &*wf.texinfo;

    for idx in 0..numverts {
        let v_off = idx * 3;
        let vert = &[verts[v_off], verts[v_off + 1], verts[v_off + 2]];

        // poly->verts[i+1]
        glpoly_set_vert(poly, (idx + 1) as i32, vert);

        let s = dot_product(
            vert,
            &[texinfo.vecs[0][0], texinfo.vecs[0][1], texinfo.vecs[0][2]],
        );
        let t = dot_product(
            vert,
            &[texinfo.vecs[1][0], texinfo.vecs[1][1], texinfo.vecs[1][2]],
        );

        total_s += s;
        total_t += t;
        total = vector_add(&total, vert);

        glpoly_set_st(poly, (idx + 1) as i32, s, t);
    }

    let center = vector_scale(&total, 1.0 / numverts as f32);
    glpoly_set_vert(poly, 0, &center);
    glpoly_set_st(poly, 0, total_s / numverts as f32, total_t / numverts as f32);

    // copy first vertex to last
    glpoly_copy_vert(poly, (numverts + 1) as i32, 1);
}

/// Break a surface polygon into subdivided pieces for warp/sky effects.
///
/// # Safety
/// Accesses global loadmodel and warpface.
pub unsafe fn vk_subdivide_surface(fa: *mut MSurface) {
    warpface = fa;

    let mut verts = [[0.0f32; 3]; 64];
    let mut numverts = 0usize;

    let fa_ref = &*fa;
    for i in 0..fa_ref.numedges {
        let lindex = loadmodel_surfedge(fa_ref.firstedge + i);

        let vec = if lindex > 0 {
            loadmodel_vertex_position(loadmodel_edge_v(lindex, 0))
        } else {
            loadmodel_vertex_position(loadmodel_edge_v(-lindex, 1))
        };

        verts[numverts] = vec;
        numverts += 1;
    }

    let mut flat = vec![0.0f32; numverts * 3];
    for i in 0..numverts {
        flat[i * 3] = verts[i][0];
        flat[i * 3 + 1] = verts[i][1];
        flat[i * 3 + 2] = verts[i][2];
    }
    subdivide_polygon(numverts, &mut flat);
}

// ============================================================
// Water polygon emission
// ============================================================

// emit_water_polys: removed (legacy immediate-mode GL rendering).
// Will be replaced by modern rendering pipeline.

// ============================================================
// Sky polygon clipping and drawing
// ============================================================

/// Draw a sky polygon, projecting it onto the appropriate sky face.
///
/// # Safety
/// Accesses global sky state.
pub unsafe fn draw_sky_polygon(nump: usize, vecs: &[f32]) {
    c_sky += 1;

    // decide which face it maps to
    let mut v = [0.0f32; 3];
    for i in 0..nump {
        v[0] += vecs[i * 3];
        v[1] += vecs[i * 3 + 1];
        v[2] += vecs[i * 3 + 2];
    }

    let av = [v[0].abs(), v[1].abs(), v[2].abs()];

    let axis = if av[0] > av[1] && av[0] > av[2] {
        if v[0] < 0.0 { 1 } else { 0 }
    } else if av[1] > av[2] && av[1] > av[0] {
        if v[1] < 0.0 { 3 } else { 2 }
    } else if v[2] < 0.0 { 5 } else { 4 };

    // project new texture coords
    for i in 0..nump {
        let vp = &vecs[i * 3..];

        let j = VEC_TO_ST[axis][2];
        let dv = if j > 0 {
            vp[(j - 1) as usize]
        } else {
            -vp[(-j - 1) as usize]
        };
        if dv < 0.001 {
            continue; // don't divide by zero
        }

        let j = VEC_TO_ST[axis][0];
        let s = if j < 0 {
            -vp[(-j - 1) as usize] / dv
        } else {
            vp[(j - 1) as usize] / dv
        };

        let j = VEC_TO_ST[axis][1];
        let t = if j < 0 {
            -vp[(-j - 1) as usize] / dv
        } else {
            vp[(j - 1) as usize] / dv
        };

        if s < skymins[0][axis] {
            skymins[0][axis] = s;
        }
        if t < skymins[1][axis] {
            skymins[1][axis] = t;
        }
        if s > skymaxs[0][axis] {
            skymaxs[0][axis] = s;
        }
        if t > skymaxs[1][axis] {
            skymaxs[1][axis] = t;
        }
    }
}

/// Clip a sky polygon against the 6 sky clip planes.
///
/// # Safety
/// Accesses global sky state.
pub unsafe fn clip_sky_polygon(nump: usize, vecs: &[f32], stage: usize) {
    if nump > MAX_CLIP_VERTS - 2 {
        vid_printf(ERR_DROP, "ClipSkyPolygon: MAX_CLIP_VERTS");
        return;
    }
    if stage == 6 {
        // fully clipped, so draw it
        draw_sky_polygon(nump, vecs);
        return;
    }

    let norm = &SKYCLIP[stage];
    let mut front = false;
    let mut back = false;
    let mut dists = [0.0f32; MAX_CLIP_VERTS];
    let mut sides = [0i32; MAX_CLIP_VERTS];

    for i in 0..nump {
        let v = &vecs[i * 3..];
        let d = v[0] * norm[0] + v[1] * norm[1] + v[2] * norm[2];
        if d > ON_EPSILON {
            front = true;
            sides[i] = SIDE_FRONT;
        } else if d < -ON_EPSILON {
            back = true;
            sides[i] = SIDE_BACK;
        } else {
            sides[i] = SIDE_ON;
        }
        dists[i] = d;
    }

    if !front || !back {
        // not clipped
        clip_sky_polygon(nump, vecs, stage + 1);
        return;
    }

    // clip it
    sides[nump] = sides[0];
    dists[nump] = dists[0];

    // We need a mutable working copy that includes wrapped vertex
    let mut work = vec![0.0f32; (nump + 1) * 3];
    work[..nump * 3].copy_from_slice(&vecs[..nump * 3]);
    work[nump * 3] = vecs[0];
    work[nump * 3 + 1] = vecs[1];
    work[nump * 3 + 2] = vecs[2];

    let mut newv = [[[0.0f32; 3]; MAX_CLIP_VERTS]; 2];
    let mut newc = [0usize; 2];

    for i in 0..nump {
        let v = &work[i * 3..];

        match sides[i] {
            SIDE_FRONT => {
                newv[0][newc[0]] = [v[0], v[1], v[2]];
                newc[0] += 1;
            }
            SIDE_BACK => {
                newv[1][newc[1]] = [v[0], v[1], v[2]];
                newc[1] += 1;
            }
            SIDE_ON => {
                newv[0][newc[0]] = [v[0], v[1], v[2]];
                newc[0] += 1;
                newv[1][newc[1]] = [v[0], v[1], v[2]];
                newc[1] += 1;
            }
            _ => {}
        }

        if sides[i] == SIDE_ON || sides[i + 1] == SIDE_ON || sides[i + 1] == sides[i] {
            continue;
        }

        let d = dists[i] / (dists[i] - dists[i + 1]);
        for j in 0..3 {
            let e = v[j] + d * (v[j + 3] - v[j]);
            newv[0][newc[0]][j] = e;
            newv[1][newc[1]][j] = e;
        }
        newc[0] += 1;
        newc[1] += 1;
    }

    // flatten and continue
    let mut flat0 = vec![0.0f32; newc[0] * 3];
    for i in 0..newc[0] {
        flat0[i * 3] = newv[0][i][0];
        flat0[i * 3 + 1] = newv[0][i][1];
        flat0[i * 3 + 2] = newv[0][i][2];
    }
    let mut flat1 = vec![0.0f32; newc[1] * 3];
    for i in 0..newc[1] {
        flat1[i * 3] = newv[1][i][0];
        flat1[i * 3 + 1] = newv[1][i][1];
        flat1[i * 3 + 2] = newv[1][i][2];
    }

    clip_sky_polygon(newc[0], &flat0, stage + 1);
    clip_sky_polygon(newc[1], &flat1, stage + 1);
}

/// Add a sky surface to the sky bounds.
///
/// # Safety
/// Accesses global GL state and renderer globals.
pub unsafe fn r_add_sky_surface(fa: &MSurface) {
    let mut p = fa.polys;
    while !p.is_null() {
        let nv = (*p).numverts as usize;
        let mut verts = vec![0.0f32; nv * 3];
        for i in 0..nv {
            let pv = glpoly_vert_ptr(p, i as i32);
            verts[i * 3] = *pv.offset(0) - r_origin[0];
            verts[i * 3 + 1] = *pv.offset(1) - r_origin[1];
            verts[i * 3 + 2] = *pv.offset(2) - r_origin[2];
        }
        clip_sky_polygon(nv, &verts, 0);
        p = (*p).next;
    }
}

/// Clear the sky box bounds.
pub unsafe fn r_clear_sky_box() {
    for i in 0..6 {
        skymins[0][i] = 9999.0;
        skymins[1][i] = 9999.0;
        skymaxs[0][i] = -9999.0;
        skymaxs[1][i] = -9999.0;
    }
}

// make_sky_vec: removed (legacy immediate-mode GL rendering).
// Will be replaced by modern rendering pipeline.

// r_draw_sky_box: removed (legacy immediate-mode GL rendering).
// Will be replaced by modern rendering pipeline.

/// Set the sky parameters (texture name, rotation, axis).
///
/// # Safety
/// Accesses global sky state and texture system.
pub unsafe fn r_set_sky(name: &str, rotate: f32, axis: &Vec3) {
    // copy name
    let name_bytes = name.as_bytes();
    let copy_len = name_bytes.len().min(MAX_QPATH - 1);
    skyname[..copy_len].copy_from_slice(&name_bytes[..copy_len]);
    skyname[copy_len] = 0;

    skyrotate = rotate;
    skyaxis = *axis;

    for i in 0..6 {
        // chop down rotating skies for less memory
        if crate::vk_rmain::VK_SKYMIP.value != 0.0 || skyrotate != 0.0 {
            vk_picmip_inc();
        }

        let pathname = format!("env/{}{}.pcx", name, SUF[i]);
        sky_images[i] = vk_find_image(&pathname, ImageType::Sky);
        if sky_images[i].is_null() {
            sky_images[i] = r_notexture;
        }

        if crate::vk_rmain::VK_SKYMIP.value != 0.0 || skyrotate != 0.0 {
            // take less memory
            vk_picmip_dec();
            sky_min = 1.0 / 256.0;
            sky_max = 255.0 / 256.0;
        } else {
            sky_min = 1.0 / 512.0;
            sky_max = 511.0 / 512.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // Constants
    // ============================================================

    #[test]
    fn test_skybox_size() {
        assert_eq!(SKYBOX_SIZE, 4600.0);
    }

    #[test]
    fn test_subdivide_size() {
        assert_eq!(SUBDIVIDE_SIZE, 64.0);
    }

    #[test]
    fn test_on_epsilon() {
        assert_eq!(ON_EPSILON, 0.1);
    }

    #[test]
    fn test_max_clip_verts() {
        assert_eq!(MAX_CLIP_VERTS, 64);
    }

    #[test]
    fn test_side_constants() {
        assert_eq!(SIDE_FRONT, 0);
        assert_eq!(SIDE_BACK, 1);
        assert_eq!(SIDE_ON, 2);
    }

    #[test]
    fn test_turbscale_value() {
        // TURBSCALE = 256 / (2 * PI)
        let expected = 256.0 / (2.0 * std::f32::consts::PI);
        assert!((TURBSCALE - expected).abs() < 1e-6);
    }

    // ============================================================
    // R_TURBSIN table
    // ============================================================

    #[test]
    fn test_turbsin_table_length() {
        assert_eq!(R_TURBSIN.len(), 256);
    }

    #[test]
    fn test_turbsin_first_entry_is_zero() {
        assert_eq!(R_TURBSIN[0], 0.0);
    }

    #[test]
    fn test_turbsin_peak_at_index_64() {
        // Peak of the sine wave at index 64 (quarter period)
        assert_eq!(R_TURBSIN[64], 8.0);
    }

    #[test]
    fn test_turbsin_negative_peak_at_index_192() {
        // Negative peak at index 192 (three-quarter period)
        assert_eq!(R_TURBSIN[192], -8.0);
    }

    #[test]
    fn test_turbsin_symmetry() {
        // The table should be symmetric: R_TURBSIN[i] == -R_TURBSIN[i + 128] for first half
        for i in 1..128 {
            let diff = (R_TURBSIN[i] + R_TURBSIN[i + 128]).abs();
            assert!(diff < 1e-4, "Symmetry check failed at index {}: {} vs {}", i, R_TURBSIN[i], R_TURBSIN[i + 128]);
        }
    }

    #[test]
    fn test_turbsin_range() {
        // All values should be in [-8.0, 8.0]
        for (i, &val) in R_TURBSIN.iter().enumerate() {
            assert!(val >= -8.0 && val <= 8.0,
                "R_TURBSIN[{}] = {} is out of range [-8, 8]", i, val);
        }
    }

    // ============================================================
    // Sky clip tables
    // ============================================================

    #[test]
    fn test_skyclip_table_length() {
        assert_eq!(SKYCLIP.len(), 6);
    }

    #[test]
    fn test_st_to_vec_table_length() {
        assert_eq!(ST_TO_VEC.len(), 6);
    }

    #[test]
    fn test_vec_to_st_table_length() {
        assert_eq!(VEC_TO_ST.len(), 6);
    }

    #[test]
    fn test_skytexorder_length() {
        assert_eq!(SKYTEXORDER.len(), 6);
    }

    #[test]
    fn test_skytexorder_values() {
        // Standard Quake 2 skybox face ordering
        assert_eq!(SKYTEXORDER, [0, 2, 1, 3, 4, 5]);
    }

    #[test]
    fn test_suf_suffixes() {
        assert_eq!(SUF, ["rt", "bk", "lf", "ft", "up", "dn"]);
    }

    // ============================================================
    // bound_poly
    // ============================================================

    #[test]
    fn test_bound_poly_single_vertex() {
        let verts = [1.0, 2.0, 3.0];
        let mut mins = [0.0f32; 3];
        let mut maxs = [0.0f32; 3];
        bound_poly(1, &verts, &mut mins, &mut maxs);

        assert_eq!(mins, [1.0, 2.0, 3.0]);
        assert_eq!(maxs, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_bound_poly_two_vertices() {
        let verts = [
            -5.0, 10.0, 3.0,
            5.0, -10.0, 7.0,
        ];
        let mut mins = [0.0f32; 3];
        let mut maxs = [0.0f32; 3];
        bound_poly(2, &verts, &mut mins, &mut maxs);

        assert_eq!(mins, [-5.0, -10.0, 3.0]);
        assert_eq!(maxs, [5.0, 10.0, 7.0]);
    }

    #[test]
    fn test_bound_poly_triangle() {
        let verts = [
            0.0, 0.0, 0.0,
            10.0, 0.0, 0.0,
            5.0, 10.0, 5.0,
        ];
        let mut mins = [0.0f32; 3];
        let mut maxs = [0.0f32; 3];
        bound_poly(3, &verts, &mut mins, &mut maxs);

        assert_eq!(mins, [0.0, 0.0, 0.0]);
        assert_eq!(maxs, [10.0, 10.0, 5.0]);
    }

    #[test]
    fn test_bound_poly_negative_coordinates() {
        let verts = [
            -100.0, -200.0, -300.0,
            -50.0, -100.0, -150.0,
            -75.0, -150.0, -225.0,
        ];
        let mut mins = [0.0f32; 3];
        let mut maxs = [0.0f32; 3];
        bound_poly(3, &verts, &mut mins, &mut maxs);

        assert_eq!(mins, [-100.0, -200.0, -300.0]);
        assert_eq!(maxs, [-50.0, -100.0, -150.0]);
    }

    #[test]
    fn test_bound_poly_all_same() {
        let verts = [
            5.0, 5.0, 5.0,
            5.0, 5.0, 5.0,
            5.0, 5.0, 5.0,
        ];
        let mut mins = [0.0f32; 3];
        let mut maxs = [0.0f32; 3];
        bound_poly(3, &verts, &mut mins, &mut maxs);

        assert_eq!(mins, [5.0, 5.0, 5.0]);
        assert_eq!(maxs, [5.0, 5.0, 5.0]);
    }

    #[test]
    fn test_bound_poly_quad() {
        // A quad in the XY plane
        let verts = [
            0.0, 0.0, 0.0,
            100.0, 0.0, 0.0,
            100.0, 100.0, 0.0,
            0.0, 100.0, 0.0,
        ];
        let mut mins = [0.0f32; 3];
        let mut maxs = [0.0f32; 3];
        bound_poly(4, &verts, &mut mins, &mut maxs);

        assert_eq!(mins, [0.0, 0.0, 0.0]);
        assert_eq!(maxs, [100.0, 100.0, 0.0]);
    }

    // ============================================================
    // Sky clip plane normals
    // ============================================================

    #[test]
    fn test_skyclip_planes_are_unit_length_or_normalized() {
        // The sky clip planes in Quake 2 are NOT unit-length; they
        // are [1,1,0], [1,-1,0], etc. with length sqrt(2).
        // This is intentional -- the clip code uses dot product directly.
        for (i, plane) in SKYCLIP.iter().enumerate() {
            let len_sq = plane[0] * plane[0] + plane[1] * plane[1] + plane[2] * plane[2];
            // They should all have the same non-zero length
            assert!(len_sq > 0.0, "SKYCLIP[{}] has zero length", i);
            // All should be length sqrt(2) = 1.414...
            let len = len_sq.sqrt();
            assert!((len - std::f32::consts::SQRT_2).abs() < 1e-6,
                "SKYCLIP[{}] length is {} expected {}", i, len, std::f32::consts::SQRT_2);
        }
    }

    // ============================================================
    // ST_TO_VEC / VEC_TO_ST consistency
    // ============================================================

    #[test]
    fn test_st_to_vec_values_in_range() {
        // All values in ST_TO_VEC should be in [-3, 3]
        for (i, row) in ST_TO_VEC.iter().enumerate() {
            for (j, &val) in row.iter().enumerate() {
                assert!(val >= -3 && val <= 3,
                    "ST_TO_VEC[{}][{}] = {} is out of range [-3, 3]", i, j, val);
            }
        }
    }

    #[test]
    fn test_vec_to_st_values_in_range() {
        // All values in VEC_TO_ST should be in [-3, 3]
        for (i, row) in VEC_TO_ST.iter().enumerate() {
            for (j, &val) in row.iter().enumerate() {
                assert!(val >= -3 && val <= 3,
                    "VEC_TO_ST[{}][{}] = {} is out of range [-3, 3]", i, j, val);
            }
        }
    }

    #[test]
    fn test_st_to_vec_no_zeros() {
        // Each row in ST_TO_VEC should have no zero entries
        // (each maps s, t, and distance to an axis)
        for (i, row) in ST_TO_VEC.iter().enumerate() {
            for (j, &val) in row.iter().enumerate() {
                assert!(val != 0,
                    "ST_TO_VEC[{}][{}] should not be zero", i, j);
            }
        }
    }
}
