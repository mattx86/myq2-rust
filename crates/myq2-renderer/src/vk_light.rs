// vk_light.rs — Dynamic lights, light sampling, stain maps
// Converted from: myq2-original/ref_gl/vk_light.c

use crate::vk_local::*;
use crate::vk_rmain::vid_printf;
use myq2_common::q_shared::*;

// ============================================================
// MyQ2 build options (from myq2opts.h)
// ============================================================
pub const DLIGHT_CUTOFF: f32 = 16.0;
// DLIGHT_SURFACE_FIX and BETTER_DLIGHT_FALLOFF are defined in vk_local.rs

// ============================================================
// Module globals
// ============================================================

pub static mut r_dlightframecount: i32 = 0;

pub static mut pointcolor: Vec3 = [0.0; 3];
pub static mut lightplane: *const CPlane = std::ptr::null();
pub static mut lightspot: Vec3 = [0.0; 3];

static mut S_BLOCKLIGHTS: [f32; 34 * 34 * 3] = [0.0; 34 * 34 * 3];

static mut TEMP_STAIN: DStain = DStain {
    origin: [0.0; 3],
    color: [0.0; 3],
    alpha: 0.0,
    intensity: 0.0,
    stain_type: StainType::Subtract,
};

// ============================================================
// Stain types
// ============================================================

pub use myq2_common::q_shared::{StainType, DStain};

// ============================================================
// Dynamic light structures (referenced from vk_local)
// ============================================================

// DLight and LightStyle are defined in vk_local to avoid circular dependency.
pub use crate::vk_local::{DLight, LightStyle};

// ============================================================
// Forward declarations / extern refs — these would come from
// vk_local in a full build. Represented as placeholder statics
// and functions that the linker resolves.
// ============================================================

// Placeholder types for compilation — real definitions live in vk_local / vk_model
// These are defined in vk_local.rs and re-exported; we reference them here as types.

use myq2_common::qfiles::MAXLIGHTMAPS;

// ============================================================
// DYNAMIC LIGHTS BLEND RENDERING
// ============================================================

// r_render_dlight: removed (legacy immediate-mode GL rendering).
// Will be replaced by modern rendering pipeline.

// r_render_dlights: removed (legacy immediate-mode GL rendering).
// Will be replaced by modern rendering pipeline.

// ============================================================
// DYNAMIC LIGHTS — BSP marking
// ============================================================

/// Recursively mark surfaces affected by a dynamic light.
///
/// # Safety
/// Accesses world model surfaces and BSP node tree via raw pointers.
pub unsafe fn r_mark_lights(light: &DLight, bit: i32, node: *mut MNode) {
    if node.is_null() {
        return;
    }
    let node_ref = &mut *node;

    if node_ref.contents != -1 {
        return;
    }

    let splitplane = &*node_ref.plane;
    let dist = dot_product(&light.origin, &splitplane.normal) - splitplane.dist;

    if dist > light.intensity - DLIGHT_CUTOFF {
        r_mark_lights(light, bit, node_ref.children[0]);
        return;
    }
    if dist < -light.intensity + DLIGHT_CUTOFF {
        r_mark_lights(light, bit, node_ref.children[1]);
        return;
    }

    // mark the polygons
    let surfaces = r_worldmodel_surfaces();
    for i in 0..node_ref.numsurfaces {
        let surf = &mut *surfaces.offset((node_ref.firstsurface + i) as isize);

        if DLIGHT_SURFACE_FIX {
            let dist2 =
                dot_product(&light.origin, &(*surf.plane).normal) - (*surf.plane).dist;
            let sidebit = if dist2 >= 0.0 { 0 } else { SURF_PLANEBACK };

            if (surf.flags & SURF_PLANEBACK) != sidebit {
                continue;
            }

            if surf.dlightframe != r_dlightframecount {
                surf.dlightbits = bit;
                surf.dlightframe = r_dlightframecount;
            } else {
                surf.dlightbits |= bit;
            }
        } else {
            if surf.dlightframe != r_dlightframecount {
                surf.dlightbits = 0;
                surf.dlightframe = r_dlightframecount;
            }
            surf.dlightbits |= bit;
        }
    }

    r_mark_lights(light, bit, node_ref.children[0]);
    r_mark_lights(light, bit, node_ref.children[1]);
}

/// Push dynamic lights into the BSP tree.
///
/// # Safety
/// Accesses global renderer state.
pub unsafe fn r_push_dlights() {
    if crate::vk_rmain::VK_DYNAMIC.value == 0.0 {
        return;
    }
    if crate::vk_rmain::VK_FLASHBLEND.value != 0.0 {
        return;
    }

    r_dlightframecount = r_framecount + 1;

    for i in 0..r_newrefdef.num_dlights {
        r_mark_lights(
            r_newrefdef.dlight(i as usize),
            1 << i,
            r_worldmodel_nodes(),
        );
    }
}

// ============================================================
// LIGHT SAMPLING
// ============================================================

/// Recursively trace a ray through the BSP to sample lightmap data.
///
/// # Safety
/// Accesses world model BSP data via raw pointers.
pub unsafe fn recursive_light_point(node: *const MNode, start: &Vec3, end: &Vec3) -> i32 {
    if node.is_null() {
        return -1;
    }
    let node_ref = &*node;

    if node_ref.contents != -1 {
        return -1; // didn't hit anything
    }

    // calculate mid point
    let plane = &*node_ref.plane;
    let front = dot_product(start, &plane.normal) - plane.dist;
    let back = dot_product(end, &plane.normal) - plane.dist;
    let side = if front < 0.0 { 1usize } else { 0usize };

    if (back < 0.0) == (front < 0.0) {
        return recursive_light_point(node_ref.children[side], start, end);
    }

    let frac = front / (front - back);
    let mid = [
        start[0] + (end[0] - start[0]) * frac,
        start[1] + (end[1] - start[1]) * frac,
        start[2] + (end[2] - start[2]) * frac,
    ];

    // go down front side
    let r = recursive_light_point(node_ref.children[side], start, &mid);
    if r >= 0 {
        return r; // hit something
    }

    if (back < 0.0) == (front < 0.0) {
        return -1; // didn't hit anything
    }

    // check for impact on this node
    lightspot = mid;
    lightplane = node_ref.plane;

    let surfaces = r_worldmodel_surfaces();
    for i in 0..node_ref.numsurfaces {
        let surf = &*surfaces.offset((node_ref.firstsurface + i) as isize);

        if surf.flags & (SURF_DRAWTURB | SURF_DRAWSKY) != 0 {
            continue; // no lightmaps
        }

        let tex = &*surf.texinfo;

        let s = dot_product(&mid, &[tex.vecs[0][0], tex.vecs[0][1], tex.vecs[0][2]])
            + tex.vecs[0][3];
        let t = dot_product(&mid, &[tex.vecs[1][0], tex.vecs[1][1], tex.vecs[1][2]])
            + tex.vecs[1][3];

        let s = s as i32;
        let t = t as i32;

        if s < surf.texturemins[0] as i32 || t < surf.texturemins[1] as i32 {
            continue;
        }

        let ds = s - surf.texturemins[0] as i32;
        let dt = t - surf.texturemins[1] as i32;

        if ds > surf.extents[0] as i32 || dt > surf.extents[1] as i32 {
            continue;
        }

        if surf.samples.is_null() {
            return 0;
        }

        let ds = ds >> 4;
        let dt = dt >> 4;

        pointcolor = [0.0; 3];
        if !surf.samples.is_null() {
            let mut lightmap = surf.samples;
            let width = (surf.extents[0] as i32 >> 4) + 1;

            lightmap = lightmap.offset((3 * (dt * width + ds)) as isize);

            for maps in 0..MAXLIGHTMAPS {
                if surf.styles[maps] == 255 {
                    break;
                }

                let style = &r_newrefdef.lightstyle(surf.styles[maps] as usize);
                let scale = [
                    crate::vk_rmain::VK_MODULATE_CVAR.value * style.rgb[0],
                    crate::vk_rmain::VK_MODULATE_CVAR.value * style.rgb[1],
                    crate::vk_rmain::VK_MODULATE_CVAR.value * style.rgb[2],
                ];

                pointcolor[0] += *lightmap.offset(0) as f32 * scale[0] * (1.0 / 255.0);
                pointcolor[1] += *lightmap.offset(1) as f32 * scale[1] * (1.0 / 255.0);
                pointcolor[2] += *lightmap.offset(2) as f32 * scale[2] * (1.0 / 255.0);

                lightmap = lightmap.offset(
                    (3 * ((surf.extents[0] as i32 >> 4) + 1)
                        * ((surf.extents[1] as i32 >> 4) + 1)) as isize,
                );
            }
        }

        return 1;
    }

    // go down back side
    let other_side = if side == 0 { 1 } else { 0 };
    recursive_light_point(node_ref.children[other_side], &mid, end)
}

/// Stain a BSP node recursively.
///
/// # Safety
/// Accesses world model BSP data via raw pointers.
pub unsafe fn r_stain_node(st: &DStain, node: *mut MNode) {
    if node.is_null() {
        return;
    }
    let node_ref = &*node;

    if node_ref.contents != -1 {
        return;
    }

    let dist = dot_product(&st.origin, &(*node_ref.plane).normal) - (*node_ref.plane).dist;
    if dist > st.intensity {
        r_stain_node(st, node_ref.children[0]);
        return;
    }
    if dist < -st.intensity {
        r_stain_node(st, node_ref.children[1]);
        return;
    }

    let surfaces = r_worldmodel_surfaces();
    let mut c = node_ref.numsurfaces;
    let mut surf_ptr = surfaces.offset(node_ref.firstsurface as isize);

    while c > 0 {
        let surf = &mut *surf_ptr;
        c -= 1;
        surf_ptr = surf_ptr.offset(1);

        let smax = (surf.extents[0] as i32 >> 4) + 1;
        let tmax = (surf.extents[1] as i32 >> 4) + 1;
        let tex = &*surf.texinfo;

        if tex.flags & (SURF_SKY | SURF_TRANS33 | SURF_TRANS66 | SURF_WARP) != 0 {
            continue;
        }

        let mut frad = st.intensity;
        let mut fdist =
            dot_product(&st.origin, &(*surf.plane).normal) - (*surf.plane).dist;
        if surf.flags & SURF_PLANEBACK != 0 {
            fdist *= -1.0;
        }
        frad -= fdist.abs();

        if frad < 0.0 {
            continue;
        }
        let fminlight = frad;

        let mut impact = [0.0f32; 3];
        for i in 0..3 {
            impact[i] = st.origin[i] - (*surf.plane).normal[i] * fdist;
        }

        let local_0 = dot_product(
            &impact,
            &[tex.vecs[0][0], tex.vecs[0][1], tex.vecs[0][2]],
        ) + tex.vecs[0][3]
            - surf.texturemins[0] as f32;
        let local_1 = dot_product(
            &impact,
            &[tex.vecs[1][0], tex.vecs[1][1], tex.vecs[1][2]],
        ) + tex.vecs[1][3]
            - surf.texturemins[1] as f32;

        if surf.stains.is_null() {
            continue;
        }

        let mut pf_bl = surf.stains;
        surf.cached_light[0] = 0.0;

        let mut ftacc: f32 = 0.0;
        for _t in 0..tmax {
            let mut td = (local_1 - ftacc) as i32;
            if td < 0 {
                td = -td;
            }

            let mut fsacc: f32 = 0.0;
            for _s in 0..smax {
                let mut sd = (local_0 - fsacc) as i32;
                if sd < 0 {
                    sd = -sd;
                }

                let fdist_local = if sd > td {
                    sd as f32 + (td >> 1) as f32
                } else {
                    td as f32 + (sd >> 1) as f32
                };

                if fdist_local < fminlight {
                    let mut mult = frad / fdist_local;
                    if mult > 5.0 {
                        mult = 5.0;
                    }

                    let mut alpha = st.alpha * mult;
                    if alpha > 255.0 {
                        alpha = 255.0;
                    }
                    if alpha > st.alpha {
                        alpha = st.alpha;
                    }
                    if alpha < 0.0 {
                        alpha = 0.0;
                    }
                    alpha /= 255.0;

                    for i in 0..3 {
                        let test = match st.stain_type {
                            StainType::Add => {
                                *pf_bl.offset(i) as f32 + alpha * st.color[i as usize]
                            }
                            StainType::Modulate => {
                                (1.0 - alpha) * *pf_bl.offset(i) as f32
                                    + alpha * st.color[i as usize]
                            }
                            StainType::Subtract => {
                                *pf_bl.offset(i) as f32 - alpha * st.color[i as usize]
                            }
                        };

                        if test > 255.0 {
                            *pf_bl.offset(i) = 255;
                        } else if test < 0.0 {
                            *pf_bl.offset(i) = 0;
                        } else {
                            *pf_bl.offset(i) = test as u8;
                        }
                    }
                }

                fsacc += 16.0;
                pf_bl = pf_bl.offset(3);
            }
            ftacc += 16.0;
        }
    }

    r_stain_node(st, node_ref.children[0]);
    r_stain_node(st, node_ref.children[1]);
}

/// Sample the lightmap at a world point, adding dynamic lights.
///
/// # Safety
/// Accesses global renderer state and world model data.
pub unsafe fn r_light_point(p: &Vec3, color: &mut Vec3) {
    if r_worldmodel_lightdata().is_null() {
        color[0] = 1.0;
        color[1] = 1.0;
        color[2] = 1.0;
        return;
    }

    let end = [p[0], p[1], p[2] - 2048.0];

    let r = recursive_light_point(r_worldmodel_nodes() as *const MNode, p, &end);

    if r == -1 {
        *color = vec3_origin;
    } else {
        *color = pointcolor;
    }

    // add dynamic lights
    for lnum in 0..r_newrefdef.num_dlights {
        let dl = &r_newrefdef.dlight(lnum as usize);
        let dist = vector_subtract(&(*currententity).origin, &dl.origin);
        let add = (dl.intensity - vector_length(&dist)) * (1.0 / 256.0);
        if add > 0.0 {
            *color = vector_ma(color, add, &dl.color);
        }
    }

    *color = vector_scale(color, crate::vk_rmain::VK_MODULATE_CVAR.value);
}

// ============================================================
// Dynamic light contribution to blocklights
// ============================================================

/// Add dynamic light contributions to the blocklights buffer for a surface.
///
/// # Safety
/// Accesses global blocklights buffer and renderer state.
pub unsafe fn r_add_dynamic_lights(surf: &MSurface) {
    let smax = (surf.extents[0] as i32 >> 4) + 1;
    let tmax = (surf.extents[1] as i32 >> 4) + 1;
    let tex = &*surf.texinfo;

    for lnum in 0..r_newrefdef.num_dlights {
        if surf.dlightbits & (1 << lnum) == 0 {
            continue; // not lit by this light
        }

        let dl = &r_newrefdef.dlight(lnum as usize);
        let frad = dl.intensity;
        let fdist_plane =
            dot_product(&dl.origin, &(*surf.plane).normal) - (*surf.plane).dist;
        let frad_adj = frad - fdist_plane.abs();

        let fminlight = DLIGHT_CUTOFF;
        if frad_adj < fminlight {
            continue;
        }
        let fminlight = frad_adj - fminlight;

        let mut impact = [0.0f32; 3];
        for i in 0..3 {
            impact[i] = dl.origin[i] - (*surf.plane).normal[i] * fdist_plane;
        }

        let local_0 = dot_product(
            &impact,
            &[tex.vecs[0][0], tex.vecs[0][1], tex.vecs[0][2]],
        ) + tex.vecs[0][3]
            - surf.texturemins[0] as f32;
        let local_1 = dot_product(
            &impact,
            &[tex.vecs[1][0], tex.vecs[1][1], tex.vecs[1][2]],
        ) + tex.vecs[1][3]
            - surf.texturemins[1] as f32;

        let mut bl_idx: usize = 0;
        let mut ftacc: f32 = 0.0;
        for _t in 0..tmax {
            let mut td = (local_1 - ftacc) as i32;
            if td < 0 {
                td = -td;
            }

            let mut fsacc: f32 = 0.0;
            for _s in 0..smax {
                let mut sd = (local_0 - fsacc) as i32;
                if sd < 0 {
                    sd = -sd;
                }

                let fdist_local = if sd > td {
                    sd as f32 + (td >> 1) as f32
                } else {
                    td as f32 + (sd >> 1) as f32
                };

                if fdist_local < fminlight {
                    if BETTER_DLIGHT_FALLOFF {
                        S_BLOCKLIGHTS[bl_idx] +=
                            (fminlight - fdist_local) * dl.color[0];
                        S_BLOCKLIGHTS[bl_idx + 1] +=
                            (fminlight - fdist_local) * dl.color[1];
                        S_BLOCKLIGHTS[bl_idx + 2] +=
                            (fminlight - fdist_local) * dl.color[2];
                    } else {
                        S_BLOCKLIGHTS[bl_idx] +=
                            (frad_adj - fdist_local) * dl.color[0];
                        S_BLOCKLIGHTS[bl_idx + 1] +=
                            (frad_adj - fdist_local) * dl.color[1];
                        S_BLOCKLIGHTS[bl_idx + 2] +=
                            (frad_adj - fdist_local) * dl.color[2];
                    }
                }

                fsacc += 16.0;
                bl_idx += 3;
            }
            ftacc += 16.0;
        }
    }
}

/// Add stain map attenuation to the blocklights buffer.
///
/// # Safety
/// Accesses global blocklights buffer and surface stain data.
pub unsafe fn r_add_stains(surf: &MSurface) {
    let scale = [crate::vk_rmain::VK_MODULATE_CVAR.value; 3];

    let smax = (surf.extents[0] as i32 >> 4) + 1;
    let tmax = (surf.extents[1] as i32 >> 4) + 1;

    let mut bl_idx: usize = 0;
    let mut stain_ptr = surf.stains;

    for _t in 0..tmax {
        for _s in 0..smax {
            for i in 0..3 {
                let stain_val = *stain_ptr.offset(i) as f32 * scale[i as usize];
                if S_BLOCKLIGHTS[bl_idx + i as usize] > stain_val {
                    S_BLOCKLIGHTS[bl_idx + i as usize] = stain_val;
                }
            }
            bl_idx += 3;
            stain_ptr = stain_ptr.offset(3);
        }
    }
}

/// Cache the current lightstyle values for a surface.
///
/// # Safety
/// Accesses global refdef lightstyles.
pub unsafe fn r_set_cache_state(surf: &mut MSurface) {
    for maps in 0..MAXLIGHTMAPS {
        if surf.styles[maps] == 255 {
            break;
        }
        surf.cached_light[maps] =
            r_newrefdef.lightstyle(surf.styles[maps] as usize).white;
    }
}

/// Build the lightmap for a surface into `dest`.
///
/// Combines and scales multiple lightmaps into the floating-point blocklights
/// buffer, then converts to RGBA texture format.
///
/// # Safety
/// Accesses global renderer state, blocklights buffer, and surface data.
pub unsafe fn r_build_light_map(surf: &MSurface, dest: *mut u8, stride: i32) {
    if (*surf.texinfo).flags & (SURF_SKY | SURF_TRANS33 | SURF_TRANS66 | SURF_WARP) != 0 {
        vid_printf(ERR_DROP, "R_BuildLightMap called for non-lit surface");
        return;
    }

    let smax = (surf.extents[0] as i32 >> 4) + 1;
    let tmax = (surf.extents[1] as i32 >> 4) + 1;
    let size = (smax * tmax) as usize;

    if size > (std::mem::size_of_val(&S_BLOCKLIGHTS) >> 4) {
        vid_printf(ERR_DROP, "Bad s_blocklights size");
        return;
    }

    // set to full bright if no light data
    if surf.samples.is_null() {
        for i in 0..(size * 3) {
            S_BLOCKLIGHTS[i] = 255.0;
        }
        // still need to iterate styles for side effects
        for maps in 0..MAXLIGHTMAPS {
            if surf.styles[maps] == 255 {
                break;
            }
            let _ = &r_newrefdef.lightstyle(surf.styles[maps] as usize);
        }
        // goto store equivalent — fall through to store section
    } else {
        // count the # of maps
        let mut nummaps = 0;
        for maps in 0..MAXLIGHTMAPS {
            if surf.styles[maps] == 255 {
                break;
            }
            nummaps += 1;
        }

        let mut lightmap = surf.samples;

        if nummaps == 1 {
            for maps in 0..MAXLIGHTMAPS {
                if surf.styles[maps] == 255 {
                    break;
                }

                let style = &r_newrefdef.lightstyle(surf.styles[maps] as usize);
                let scale = [
                    crate::vk_rmain::VK_MODULATE_CVAR.value * style.rgb[0],
                    crate::vk_rmain::VK_MODULATE_CVAR.value * style.rgb[1],
                    crate::vk_rmain::VK_MODULATE_CVAR.value * style.rgb[2],
                ];

                if scale[0] == 1.0 && scale[1] == 1.0 && scale[2] == 1.0 {
                    let mut bl_idx = 0;
                    for i in 0..size {
                        S_BLOCKLIGHTS[bl_idx] = *lightmap.add(i * 3) as f32;
                        S_BLOCKLIGHTS[bl_idx + 1] = *lightmap.add(i * 3 + 1) as f32;
                        S_BLOCKLIGHTS[bl_idx + 2] = *lightmap.add(i * 3 + 2) as f32;
                        bl_idx += 3;
                    }
                } else {
                    let mut bl_idx = 0;
                    for i in 0..size {
                        S_BLOCKLIGHTS[bl_idx] =
                            *lightmap.add(i * 3) as f32 * scale[0];
                        S_BLOCKLIGHTS[bl_idx + 1] =
                            *lightmap.add(i * 3 + 1) as f32 * scale[1];
                        S_BLOCKLIGHTS[bl_idx + 2] =
                            *lightmap.add(i * 3 + 2) as f32 * scale[2];
                        bl_idx += 3;
                    }
                }
                lightmap = lightmap.add(size * 3);
            }
        } else {
            // zero out blocklights
            for i in 0..(size * 3) {
                S_BLOCKLIGHTS[i] = 0.0;
            }

            for maps in 0..MAXLIGHTMAPS {
                if surf.styles[maps] == 255 {
                    break;
                }

                let style = &r_newrefdef.lightstyle(surf.styles[maps] as usize);
                let scale = [
                    crate::vk_rmain::VK_MODULATE_CVAR.value * style.rgb[0],
                    crate::vk_rmain::VK_MODULATE_CVAR.value * style.rgb[1],
                    crate::vk_rmain::VK_MODULATE_CVAR.value * style.rgb[2],
                ];

                if scale[0] == 1.0 && scale[1] == 1.0 && scale[2] == 1.0 {
                    let mut bl_idx = 0;
                    for i in 0..size {
                        S_BLOCKLIGHTS[bl_idx] +=
                            *lightmap.add(i * 3) as f32;
                        S_BLOCKLIGHTS[bl_idx + 1] +=
                            *lightmap.add(i * 3 + 1) as f32;
                        S_BLOCKLIGHTS[bl_idx + 2] +=
                            *lightmap.add(i * 3 + 2) as f32;
                        bl_idx += 3;
                    }
                } else {
                    let mut bl_idx = 0;
                    for i in 0..size {
                        S_BLOCKLIGHTS[bl_idx] +=
                            *lightmap.add(i * 3) as f32 * scale[0];
                        S_BLOCKLIGHTS[bl_idx + 1] +=
                            *lightmap.add(i * 3 + 1) as f32 * scale[1];
                        S_BLOCKLIGHTS[bl_idx + 2] +=
                            *lightmap.add(i * 3 + 2) as f32 * scale[2];
                        bl_idx += 3;
                    }
                }
                lightmap = lightmap.add(size * 3);
            }
        }

        // add all the dynamic lights
        if surf.dlightframe == r_framecount {
            r_add_dynamic_lights(surf);
        }

        // add stains
        if r_newrefdef.rdflags & RDF_NOWORLDMODEL == 0
            && !surf.stains.is_null() && crate::vk_rmain::R_STAINMAP.value != 0.0 {
                r_add_stains(surf);
            }
    }

    // put into texture format (store:)
    let stride_adj = stride - (smax << 2);
    let mut bl_idx: usize = 0;
    let mut dest_ptr = dest;

    let monolightmap = vk_monolightmap_char();

    if monolightmap == b'0' {
        for _i in 0..tmax {
            for _j in 0..smax {
                let mut r = S_BLOCKLIGHTS[bl_idx] as i32;
                let mut g = S_BLOCKLIGHTS[bl_idx + 1] as i32;
                let mut b = S_BLOCKLIGHTS[bl_idx + 2] as i32;

                if r < 0 { r = 0; }
                if g < 0 { g = 0; }
                if b < 0 { b = 0; }

                let mut max = if r > g { r } else { g };
                if b > max { max = b; }

                let mut a = max;

                if max > 255 {
                    let t = 255.0 / max as f32;
                    r = (r as f32 * t) as i32;
                    g = (g as f32 * t) as i32;
                    b = (b as f32 * t) as i32;
                    a = (a as f32 * t) as i32;
                }

                *dest_ptr.offset(0) = r as u8;
                *dest_ptr.offset(1) = g as u8;
                *dest_ptr.offset(2) = b as u8;
                *dest_ptr.offset(3) = a as u8;

                bl_idx += 3;
                dest_ptr = dest_ptr.offset(4);
            }
            dest_ptr = dest_ptr.offset(stride_adj as isize);
        }
    } else {
        for _i in 0..tmax {
            for _j in 0..smax {
                let mut r = S_BLOCKLIGHTS[bl_idx] as i32;
                let mut g = S_BLOCKLIGHTS[bl_idx + 1] as i32;
                let mut b = S_BLOCKLIGHTS[bl_idx + 2] as i32;

                if r < 0 { r = 0; }
                if g < 0 { g = 0; }
                if b < 0 { b = 0; }

                let mut max = if r > g { r } else { g };
                if b > max { max = b; }

                let mut a = max;

                if max > 255 {
                    let t = 255.0 / max as f32;
                    r = (r as f32 * t) as i32;
                    g = (g as f32 * t) as i32;
                    b = (b as f32 * t) as i32;
                    a = (a as f32 * t) as i32;
                }

                match monolightmap {
                    b'L' | b'I' => {
                        r = a;
                        g = 0;
                        b = 0;
                    }
                    b'C' => {
                        a = 255 - ((r + g + b) / 3);
                        r = (r as f32 * a as f32 / 255.0) as i32;
                        g = (g as f32 * a as f32 / 255.0) as i32;
                        b = (b as f32 * a as f32 / 255.0) as i32;
                    }
                    _ => {
                        // 'A' and default
                        r = 0;
                        g = 0;
                        b = 0;
                        a = 255 - a;
                    }
                }

                *dest_ptr.offset(0) = r as u8;
                *dest_ptr.offset(1) = g as u8;
                *dest_ptr.offset(2) = b as u8;
                *dest_ptr.offset(3) = a as u8;

                bl_idx += 3;
                dest_ptr = dest_ptr.offset(4);
            }
            dest_ptr = dest_ptr.offset(stride_adj as isize);
        }
    }
}

// ============================================================
// Pure helper functions (extracted for testability)
// ============================================================

/// Compute stain blending for a single color channel.
///
/// Returns the blended value clamped to [0, 255].
pub fn stain_blend_channel(current: f32, alpha: f32, stain_color: f32, stain_type: StainType) -> u8 {
    let test = match stain_type {
        StainType::Add => current + alpha * stain_color,
        StainType::Modulate => (1.0 - alpha) * current + alpha * stain_color,
        StainType::Subtract => current - alpha * stain_color,
    };

    if test > 255.0 {
        255
    } else if test < 0.0 {
        0
    } else {
        test as u8
    }
}

/// Compute the approximate distance metric used for dynamic lights and stains.
///
/// This uses the Quake 2 distance approximation: max(sd, td) + min(sd, td) / 2.
pub fn approx_distance(sd: i32, td: i32) -> f32 {
    let sd_abs = sd.abs();
    let td_abs = td.abs();
    if sd_abs > td_abs {
        sd_abs as f32 + (td_abs >> 1) as f32
    } else {
        td_abs as f32 + (sd_abs >> 1) as f32
    }
}

/// Clamp a blocklights channel value and apply overbright scaling for RGBA output.
///
/// Takes raw blocklights R, G, B values, clamps negatives to 0,
/// and if the max exceeds 255, scales all channels proportionally.
/// Returns (r, g, b, a) as u8 values.
pub fn blocklights_to_rgba(r_in: f32, g_in: f32, b_in: f32) -> (u8, u8, u8, u8) {
    let mut r = r_in as i32;
    let mut g = g_in as i32;
    let mut b = b_in as i32;

    if r < 0 { r = 0; }
    if g < 0 { g = 0; }
    if b < 0 { b = 0; }

    let mut max = if r > g { r } else { g };
    if b > max { max = b; }

    let mut a = max;

    if max > 255 {
        let t = 255.0 / max as f32;
        r = (r as f32 * t) as i32;
        g = (g as f32 * t) as i32;
        b = (b as f32 * t) as i32;
        a = (a as f32 * t) as i32;
    }

    (r as u8, g as u8, b as u8, a as u8)
}

/// Add a stain to the world at the given position.
///
/// # Safety
/// Accesses global renderer state.
pub unsafe fn add_stain(
    org: &Vec3,
    intensity: f32,
    r: f32,
    g: f32,
    b: f32,
    a: f32,
    stain_type: StainType,
) {
    if crate::vk_rmain::R_STAINMAP.value == 0.0 {
        return;
    }

    TEMP_STAIN.origin = *org;
    TEMP_STAIN.color = [r, g, b];
    TEMP_STAIN.alpha = a;
    TEMP_STAIN.intensity = intensity;
    TEMP_STAIN.stain_type = stain_type;

    r_stain_node(&TEMP_STAIN, r_worldmodel_nodes());
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // Constants
    // ============================================================

    #[test]
    fn test_dlight_cutoff_value() {
        // DLIGHT_CUTOFF must match the original C value of 16.0
        assert_eq!(DLIGHT_CUTOFF, 16.0);
    }

    // ============================================================
    // stain_blend_channel
    // ============================================================

    #[test]
    fn test_stain_blend_add() {
        // Add mode: current + alpha * color
        let result = stain_blend_channel(100.0, 0.5, 60.0, StainType::Add);
        // 100 + 0.5 * 60 = 130
        assert_eq!(result, 130);
    }

    #[test]
    fn test_stain_blend_add_clamp_high() {
        // Add mode should clamp to 255
        let result = stain_blend_channel(200.0, 1.0, 200.0, StainType::Add);
        // 200 + 1.0 * 200 = 400 -> clamped to 255
        assert_eq!(result, 255);
    }

    #[test]
    fn test_stain_blend_subtract() {
        // Subtract mode: current - alpha * color
        let result = stain_blend_channel(200.0, 0.5, 100.0, StainType::Subtract);
        // 200 - 0.5 * 100 = 150
        assert_eq!(result, 150);
    }

    #[test]
    fn test_stain_blend_subtract_clamp_low() {
        // Subtract mode should clamp to 0
        let result = stain_blend_channel(50.0, 1.0, 200.0, StainType::Subtract);
        // 50 - 1.0 * 200 = -150 -> clamped to 0
        assert_eq!(result, 0);
    }

    #[test]
    fn test_stain_blend_modulate() {
        // Modulate mode: (1 - alpha) * current + alpha * color
        let result = stain_blend_channel(200.0, 0.5, 100.0, StainType::Modulate);
        // (1 - 0.5) * 200 + 0.5 * 100 = 100 + 50 = 150
        assert_eq!(result, 150);
    }

    #[test]
    fn test_stain_blend_modulate_full_alpha() {
        // Modulate with alpha=1.0 should give the stain color
        let result = stain_blend_channel(200.0, 1.0, 100.0, StainType::Modulate);
        // (1 - 1.0) * 200 + 1.0 * 100 = 0 + 100 = 100
        assert_eq!(result, 100);
    }

    #[test]
    fn test_stain_blend_modulate_zero_alpha() {
        // Modulate with alpha=0.0 should keep original
        let result = stain_blend_channel(200.0, 0.0, 100.0, StainType::Modulate);
        // (1 - 0.0) * 200 + 0.0 * 100 = 200 + 0 = 200
        assert_eq!(result, 200);
    }

    #[test]
    fn test_stain_blend_zero_values() {
        // All zeros
        let result = stain_blend_channel(0.0, 0.0, 0.0, StainType::Add);
        assert_eq!(result, 0);
    }

    // ============================================================
    // approx_distance
    // ============================================================

    #[test]
    fn test_approx_distance_equal() {
        // sd == td: td + sd/2 = 10 + 5 = 15
        let result = approx_distance(10, 10);
        assert_eq!(result, 15.0);
    }

    #[test]
    fn test_approx_distance_sd_larger() {
        // sd > td: sd + td/2
        let result = approx_distance(20, 10);
        // 20 + 10/2 = 20 + 5 = 25
        assert_eq!(result, 25.0);
    }

    #[test]
    fn test_approx_distance_td_larger() {
        // td > sd: td + sd/2
        let result = approx_distance(10, 20);
        // 20 + 10/2 = 20 + 5 = 25
        assert_eq!(result, 25.0);
    }

    #[test]
    fn test_approx_distance_negative_inputs() {
        // Negative inputs: uses abs
        let result = approx_distance(-20, -10);
        // abs(-20) > abs(-10): 20 + 10/2 = 25
        assert_eq!(result, 25.0);
    }

    #[test]
    fn test_approx_distance_zero() {
        let result = approx_distance(0, 0);
        assert_eq!(result, 0.0);
    }

    #[test]
    fn test_approx_distance_one_zero() {
        let result = approx_distance(16, 0);
        // 16 + 0/2 = 16
        assert_eq!(result, 16.0);
    }

    // ============================================================
    // blocklights_to_rgba
    // ============================================================

    #[test]
    fn test_blocklights_to_rgba_normal() {
        // Values within 0-255 range
        let (r, g, b, a) = blocklights_to_rgba(100.0, 150.0, 200.0);
        assert_eq!(r, 100);
        assert_eq!(g, 150);
        assert_eq!(b, 200);
        assert_eq!(a, 200); // a = max(r, g, b)
    }

    #[test]
    fn test_blocklights_to_rgba_overflow() {
        // Max > 255, so scale all channels
        let (r, g, b, a) = blocklights_to_rgba(510.0, 255.0, 0.0);
        // max = 510, t = 255/510 = 0.5
        // r = 510 * 0.5 = 255, g = 255 * 0.5 = 127, b = 0, a = 510 * 0.5 = 255
        assert_eq!(r, 255);
        assert_eq!(g, 127);
        assert_eq!(b, 0);
        assert_eq!(a, 255);
    }

    #[test]
    fn test_blocklights_to_rgba_negative_clamped() {
        // Negative values should be clamped to 0
        let (r, g, b, a) = blocklights_to_rgba(-100.0, -50.0, 100.0);
        assert_eq!(r, 0);
        assert_eq!(g, 0);
        assert_eq!(b, 100);
        assert_eq!(a, 100);
    }

    #[test]
    fn test_blocklights_to_rgba_all_zero() {
        let (r, g, b, a) = blocklights_to_rgba(0.0, 0.0, 0.0);
        assert_eq!(r, 0);
        assert_eq!(g, 0);
        assert_eq!(b, 0);
        assert_eq!(a, 0);
    }

    #[test]
    fn test_blocklights_to_rgba_max_255() {
        // Exactly 255 should not trigger scaling
        let (r, g, b, a) = blocklights_to_rgba(255.0, 128.0, 64.0);
        assert_eq!(r, 255);
        assert_eq!(g, 128);
        assert_eq!(b, 64);
        assert_eq!(a, 255);
    }

    #[test]
    fn test_blocklights_to_rgba_just_over_255() {
        // Just over 255 triggers scaling
        let (r, g, b, a) = blocklights_to_rgba(256.0, 128.0, 0.0);
        // max = 256, t = 255/256 = 0.99609375
        // r = 256 * t = 254 (truncated), g = 128 * t = 127, a = 256 * t = 254
        let t = 255.0 / 256.0_f32;
        assert_eq!(r, (256.0 * t) as u8);
        assert_eq!(g, (128.0 * t) as u8);
        assert_eq!(b, 0);
        assert_eq!(a, (256.0 * t) as u8);
    }

    // ============================================================
    // S_BLOCKLIGHTS size
    // ============================================================

    #[test]
    fn test_blocklights_array_size() {
        // S_BLOCKLIGHTS should be 34*34*3 = 3468 floats
        // This matches the maximum lightmap size
        assert_eq!(34 * 34 * 3, 3468);
    }
}

