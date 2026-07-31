//! Voxel Cone-Traced Global Illumination (VXGI).
//!
//! The static world is voxelized ONCE at level load into cubic 3D textures (no per-frame
//! voxelization cost — important on the Iris Xe iGPU this targets). Two volumes:
//!  - `albedo` (RGBA8): RGB = surface normal encoded (Phase-1 debug / later albedo), A = coverage.
//!  - `radiance` (RGBA8): emission injected from light-texture and sky surfaces (Phase 2). This
//!    is the light source that cone tracing (Phase 4) gathers; its mip chain comes in Phase 3.
//!
//! Grid is a CUBE covering the world AABB's largest extent, centered on the AABB — uniform
//! voxels keep cone tracing simple.

use ash::vk;
use crate::modern::gpu_device;

/// Voxelized representation of the static world (two aligned 3D volumes).
pub struct VoxelGrid {
    pub image: vk::Image,
    pub memory: vk::DeviceMemory,
    pub view: vk::ImageView,
    pub sampler: vk::Sampler,
    pub rad_image: vk::Image,
    pub rad_memory: vk::DeviceMemory,
    pub rad_view: vk::ImageView,
    pub rad_sampler: vk::Sampler,
    /// Grid resolution (voxels per axis).
    pub res: u32,
    /// World-space corner (minimum) of the cubic grid.
    pub grid_min: [f32; 3],
    /// World-space edge length of the whole cube.
    pub extent: f32,
}

static VOXEL_GRID: std::sync::Mutex<Option<VoxelGrid>> = std::sync::Mutex::new(None);

/// Run `f` with the current voxel grid if one has been built.
pub fn with_voxel_grid<R>(f: impl FnOnce(&VoxelGrid) -> R) -> Option<R> {
    VOXEL_GRID.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(f)
}

/// Destroy the current voxel grid (level unload / shutdown).
pub fn destroy_voxel_grid() {
    let taken = VOXEL_GRID.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(vg) = taken {
        gpu_device::with_device(|ctx| unsafe {
            // SAFETY: handles were created by this module and are no longer in use; the device
            // is idle at level transitions.
            for (s, v, i, m) in [
                (vg.sampler, vg.view, vg.image, vg.memory),
                (vg.rad_sampler, vg.rad_view, vg.rad_image, vg.rad_memory),
            ] {
                ctx.device.destroy_sampler(s, None);
                ctx.device.destroy_image_view(v, None);
                ctx.device.destroy_image(i, None);
                ctx.device.free_memory(m, None);
            }
        });
    }
}

/// Create one R8G8B8A8 3D texture from a CPU byte buffer (res³·4 bytes), uploaded + transitioned
/// to SHADER_READ_ONLY. With `gen_mips`, a full mip chain is generated (trilinear blit) so cone
/// tracing can sample coarser radiance/opacity at distance. Returns (image, memory, view, sampler).
fn create_voxel_texture(res: u32, bytes: &[u8], gen_mips: bool) -> Option<(vk::Image, vk::DeviceMemory, vk::ImageView, vk::Sampler)> {
    let mips: u32 = if gen_mips { 32 - res.leading_zeros() } else { 1 }; // floor(log2(res))+1
    gpu_device::with_device_and_commands_mut(|ctx, commands| unsafe {
        // SAFETY: valid context, main thread; all handles freed on failure paths.
        let usage = vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST
            | if gen_mips { vk::ImageUsageFlags::TRANSFER_SRC } else { vk::ImageUsageFlags::empty() };
        let img_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_3D)
            .format(vk::Format::R8G8B8A8_UNORM)
            .extent(vk::Extent3D { width: res, height: res, depth: res })
            .mip_levels(mips).array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED);
        let image = ctx.device.create_image(&img_info, None).ok()?;

        let reqs = ctx.device.get_image_memory_requirements(image);
        let mem_props = ctx.instance.get_physical_device_memory_properties(ctx.physical_device);
        let mem_type = (0..mem_props.memory_type_count).find(|&i| {
            (reqs.memory_type_bits & (1 << i)) != 0
                && mem_props.memory_types[i as usize].property_flags.contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)
        })?;
        let alloc = vk::MemoryAllocateInfo::default().allocation_size(reqs.size).memory_type_index(mem_type);
        let memory = match ctx.device.allocate_memory(&alloc, None) {
            Ok(m) => m,
            Err(_) => { ctx.device.destroy_image(image, None); return None; }
        };
        if ctx.device.bind_image_memory(image, memory, 0).is_err() {
            ctx.device.free_memory(memory, None);
            ctx.device.destroy_image(image, None);
            return None;
        }

        let data_size = bytes.len() as vk::DeviceSize;
        let buf_info = vk::BufferCreateInfo::default()
            .size(data_size).usage(vk::BufferUsageFlags::TRANSFER_SRC).sharing_mode(vk::SharingMode::EXCLUSIVE);
        let staging = ctx.device.create_buffer(&buf_info, None).ok()?;
        let sreqs = ctx.device.get_buffer_memory_requirements(staging);
        let smem_type = (0..mem_props.memory_type_count).find(|&i| {
            (sreqs.memory_type_bits & (1 << i)) != 0
                && mem_props.memory_types[i as usize].property_flags.contains(
                    vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT)
        })?;
        let salloc = vk::MemoryAllocateInfo::default().allocation_size(sreqs.size).memory_type_index(smem_type);
        let smem = ctx.device.allocate_memory(&salloc, None).ok()?;
        let _ = ctx.device.bind_buffer_memory(staging, smem, 0);
        if let Ok(ptr) = ctx.device.map_memory(smem, 0, data_size, vk::MemoryMapFlags::empty()) {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr as *mut u8, bytes.len());
            ctx.device.unmap_memory(smem);
        }

        let cmd = commands.begin_single_time().ok()?;
        // helper: barrier one mip level between layouts
        let mip_range = |lvl: u32, count: u32| vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR, base_mip_level: lvl, level_count: count, base_array_layer: 0, layer_count: 1,
        };
        let barrier = |cmd: vk::CommandBuffer, lvl: u32, count: u32, ol: vk::ImageLayout, nl: vk::ImageLayout,
                       sa: vk::AccessFlags, da: vk::AccessFlags, ss: vk::PipelineStageFlags, ds: vk::PipelineStageFlags| {
            let b = vk::ImageMemoryBarrier::default().old_layout(ol).new_layout(nl)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED).dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(image).subresource_range(mip_range(lvl, count))
                .src_access_mask(sa).dst_access_mask(da);
            ctx.device.cmd_pipeline_barrier(cmd, ss, ds, vk::DependencyFlags::empty(), &[], &[], &[b]);
        };

        // Upload mip 0.
        barrier(cmd, 0, 1, vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::AccessFlags::empty(), vk::AccessFlags::TRANSFER_WRITE,
            vk::PipelineStageFlags::TOP_OF_PIPE, vk::PipelineStageFlags::TRANSFER);
        let region = vk::BufferImageCopy::default()
            .image_subresource(vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR, mip_level: 0, base_array_layer: 0, layer_count: 1,
            })
            .image_extent(vk::Extent3D { width: res, height: res, depth: res });
        ctx.device.cmd_copy_buffer_to_image(cmd, staging, image, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[region]);

        // Generate mip chain by trilinear blit (mip i-1 → mip i).
        let mut mw = res as i32; let (mut mh, mut md) = (res as i32, res as i32);
        for i in 1..mips {
            barrier(cmd, i - 1, 1, vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                vk::AccessFlags::TRANSFER_WRITE, vk::AccessFlags::TRANSFER_READ,
                vk::PipelineStageFlags::TRANSFER, vk::PipelineStageFlags::TRANSFER);
            barrier(cmd, i, 1, vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                vk::AccessFlags::empty(), vk::AccessFlags::TRANSFER_WRITE,
                vk::PipelineStageFlags::TRANSFER, vk::PipelineStageFlags::TRANSFER);
            let (nw, nh, nd) = (mw.max(2) / 2, mh.max(2) / 2, md.max(2) / 2);
            let blit = vk::ImageBlit::default()
                .src_subresource(vk::ImageSubresourceLayers { aspect_mask: vk::ImageAspectFlags::COLOR, mip_level: i - 1, base_array_layer: 0, layer_count: 1 })
                .src_offsets([vk::Offset3D { x: 0, y: 0, z: 0 }, vk::Offset3D { x: mw, y: mh, z: md }])
                .dst_subresource(vk::ImageSubresourceLayers { aspect_mask: vk::ImageAspectFlags::COLOR, mip_level: i, base_array_layer: 0, layer_count: 1 })
                .dst_offsets([vk::Offset3D { x: 0, y: 0, z: 0 }, vk::Offset3D { x: nw, y: nh, z: nd }]);
            ctx.device.cmd_blit_image(cmd, image, vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                image, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[blit], vk::Filter::LINEAR);
            mw = nw; mh = nh; md = nd;
        }
        // All but the last mip are TRANSFER_SRC; the last is TRANSFER_DST. Move everything to read.
        if mips > 1 {
            barrier(cmd, 0, mips - 1, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                vk::AccessFlags::TRANSFER_READ, vk::AccessFlags::SHADER_READ,
                vk::PipelineStageFlags::TRANSFER, vk::PipelineStageFlags::FRAGMENT_SHADER);
            barrier(cmd, mips - 1, 1, vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                vk::AccessFlags::TRANSFER_WRITE, vk::AccessFlags::SHADER_READ,
                vk::PipelineStageFlags::TRANSFER, vk::PipelineStageFlags::FRAGMENT_SHADER);
        } else {
            barrier(cmd, 0, 1, vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                vk::AccessFlags::TRANSFER_WRITE, vk::AccessFlags::SHADER_READ,
                vk::PipelineStageFlags::TRANSFER, vk::PipelineStageFlags::FRAGMENT_SHADER);
        }
        let _ = commands.end_single_time(ctx, cmd);
        ctx.device.destroy_buffer(staging, None);
        ctx.device.free_memory(smem, None);

        let view_info = vk::ImageViewCreateInfo::default()
            .image(image).view_type(vk::ImageViewType::TYPE_3D)
            .format(vk::Format::R8G8B8A8_UNORM).subresource_range(mip_range(0, mips));
        let view = ctx.device.create_image_view(&view_info, None).ok()?;
        let samp_info = vk::SamplerCreateInfo::default()
            .mag_filter(vk::Filter::LINEAR).min_filter(vk::Filter::LINEAR)
            .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
            .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .min_lod(0.0).max_lod(8.0);
        let sampler = ctx.device.create_sampler(&samp_info, None).ok()?;
        Some((image, memory, view, sampler))
    }).flatten()
}

/// Averaged incoming radiance at voxel `idx`: along 14 directions, the first surface hit beyond
/// 2 voxels (room-facing), normalized by the room-facing count (proper hemisphere gather).
fn gather_at(idx: usize, rad: &[[f32; 3]], opac: &[f32], is_sky: &[bool], dirs: &[[f32; 3]], n: usize, ni: i32, range: i32) -> [f32; 3] {
    let vz = (idx / (n * n)) as i32;
    let vy = ((idx / n) % n) as i32;
    let vx = (idx % n) as i32;
    let mut g = [0.0f32; 3];
    let mut cnt = 0u32;
    for d in dirs {
        for t in 1..=range {
            let gx = (vx as f32 + 0.5 + d[0] * t as f32) as i32;
            let gy = (vy as f32 + 0.5 + d[1] * t as f32) as i32;
            let gz = (vz as f32 + 0.5 + d[2] * t as f32) as i32;
            if gx < 0 || gy < 0 || gz < 0 || gx >= ni || gy >= ni || gz >= ni { break; }
            let nidx = ((gz as usize) * n + gy as usize) * n + gx as usize;
            if opac[nidx] > 0.5 {
                if t > 2 {
                    // Inverse-distance falloff. WITHOUT this every ray's first hit counted equally
                    // regardless of distance, so a surface gathered every light in the level at full
                    // strength → the whole volume floods to a uniform bright (a normal hallway lit as
                    // hard as a room full of lights). Weighting by 1/t makes nearby lights dominate
                    // and distant ones fade, restoring contrast (lit near fixtures, dimmer between).
                    // The .max() floor keeps a small baseline for distant hits so a surface with only
                    // far light still gets *some* fill instead of going black (splotches), while the
                    // 1/t shape still lets nearby lights dominate (keeps the contrast).
                    // SKY is the exception: it is light at INFINITY (an area source overhead), so it
                    // must NOT be distance-attenuated — 1/t would kill it for any open area, leaving
                    // sky-lit spaces dark/blotchy (the blocky splotches in open areas). Sky hits get a
                    // flat weight so a sky-exposed surface is lit uniformly however far the sky is.
                    let atten = if is_sky[nidx] { 0.5 } else { (1.0 / (t as f32)).max(0.07) };
                    let e = rad[nidx];
                    g[0] += e[0] * atten; g[1] += e[1] * atten; g[2] += e[2] * atten;
                    cnt += 1;
                }
                break;
            }
        }
    }
    // Gentle fixed normalization (NOT ÷cnt). Dividing by the full room-facing count diluted a
    // surface that sees one light among mostly-dark directions down to light/7, so each bounce
    // lost ~7× and the light died within a couple of bounces (black splotches far from lights).
    // A small fixed divisor lets light actually propagate across bounces; over-bright near lights
    // is clamped by storage + ACES.
    if cnt == 0 { return [0.0; 3]; }
    // Fixed divisor scaled for the 26-direction set (≈1.6 × 26/14) so brightness matches the old
    // 14-dir gather while the extra directions only make the spatial result smoother.
    let inv = 1.0 / 3.0;
    [g[0] * inv, g[1] * inv, g[2] * inv]
}

/// One level of a radiance mip chain: `rad` = emission/bounced colour, `opac` = coverage, `res`³ voxels.
struct RadMip { rad: Vec<[f32; 3]>, opac: Vec<f32>, res: usize }

/// Build a trilinear mip chain of the radiance + opacity grids. Each level averages 2³ children.
/// Cone tracing samples progressively coarser levels as a cone widens with distance, which AVERAGES
/// many voxels per step — the reason cone-traced GI is smooth where single-ray gathering is speckled.
fn build_rad_mips(rad: &[[f32; 3]], opac: &[f32], n: usize) -> Vec<RadMip> {
    use rayon::prelude::*;
    let mut mips = vec![RadMip { rad: rad.to_vec(), opac: opac.to_vec(), res: n }];
    let mut res = n;
    while res > 2 {
        let nr = res / 2;
        let pr = { let p = mips.last().unwrap(); p.res };
        let (prad, popac) = { let p = mips.last().unwrap(); (&p.rad, &p.opac) };
        let mut r = vec![[0.0f32; 3]; nr * nr * nr];
        let mut o = vec![0.0f32; nr * nr * nr];
        r.par_iter_mut().zip(o.par_iter_mut()).enumerate().for_each(|(oi, (rr, oo))| {
            let oz = oi / (nr * nr); let oy = (oi / nr) % nr; let ox = oi % nr;
            let mut sr = [0.0f32; 3]; let mut so = 0.0f32;
            for dz in 0..2 { for dy in 0..2 { for dx in 0..2 {
                let ix = (ox * 2 + dx).min(pr - 1);
                let iy = (oy * 2 + dy).min(pr - 1);
                let iz = (oz * 2 + dz).min(pr - 1);
                let ii = (iz * pr + iy) * pr + ix;
                sr[0] += prad[ii][0]; sr[1] += prad[ii][1]; sr[2] += prad[ii][2];
                so += popac[ii];
            }}}
            *rr = [sr[0] / 8.0, sr[1] / 8.0, sr[2] / 8.0];
            *oo = so / 8.0;
        });
        mips.push(RadMip { rad: r, opac: o, res: nr });
        res = nr;
    }
    mips
}

/// Trilinearly sample (radiance, opacity) from a mip level at a position given in MIP-0 voxel coords.
fn sample_mip(mip: &RadMip, n0: usize, x: f32, y: f32, z: f32) -> ([f32; 3], f32) {
    let scale = mip.res as f32 / n0 as f32;
    let r = mip.res as i32;
    let fx = (x * scale - 0.5).clamp(0.0, mip.res as f32 - 1.0);
    let fy = (y * scale - 0.5).clamp(0.0, mip.res as f32 - 1.0);
    let fz = (z * scale - 0.5).clamp(0.0, mip.res as f32 - 1.0);
    let x0 = fx.floor() as i32; let y0 = fy.floor() as i32; let z0 = fz.floor() as i32;
    let x1 = (x0 + 1).min(r - 1); let y1 = (y0 + 1).min(r - 1); let z1 = (z0 + 1).min(r - 1);
    let tx = fx - x0 as f32; let ty = fy - y0 as f32; let tz = fz - z0 as f32;
    let res = mip.res;
    let at = |xi: i32, yi: i32, zi: i32| -> ([f32; 3], f32) {
        let i = (zi as usize * res + yi as usize) * res + xi as usize;
        (mip.rad[i], mip.opac[i])
    };
    let lerp3 = |a: [f32; 3], b: [f32; 3], t: f32| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
    let (c000, a000) = at(x0, y0, z0); let (c100, a100) = at(x1, y0, z0);
    let (c010, a010) = at(x0, y1, z0); let (c110, a110) = at(x1, y1, z0);
    let (c001, a001) = at(x0, y0, z1); let (c101, a101) = at(x1, y0, z1);
    let (c011, a011) = at(x0, y1, z1); let (c111, a111) = at(x1, y1, z1);
    let c00 = lerp3(c000, c100, tx); let c10 = lerp3(c010, c110, tx);
    let c01 = lerp3(c001, c101, tx); let c11 = lerp3(c011, c111, tx);
    let c0 = lerp3(c00, c10, ty); let c1 = lerp3(c01, c11, ty);
    let c = lerp3(c0, c1, tz);
    let a00 = a000 + (a100 - a000) * tx; let a10 = a010 + (a110 - a010) * tx;
    let a01 = a001 + (a101 - a001) * tx; let a11 = a011 + (a111 - a011) * tx;
    let a0 = a00 + (a10 - a00) * ty; let a1 = a01 + (a11 - a01) * ty;
    (c, a0 + (a1 - a0) * tz)
}

/// Diffuse cone trace the radiance mip chain from a voxel. Marches `cones` (6 axis-aligned wide cones
/// covering the sphere) front-to-back, sampling a mip level that grows with the cone's width at each
/// step; the mip averaging is what makes the result smooth (vs the speckly single-ray gather). Sky and
/// distance falloff fall out naturally from occlusion accumulation — no 1/t hack, no sky special-case.
fn cone_trace_at(idx: usize, mips: &[RadMip], n0: usize, cones: &[[f32; 3]]) -> [f32; 3] {
    let n_lvl = mips.len();
    let vz = (idx / (n0 * n0)) as f32;
    let vy = ((idx / n0) % n0) as f32;
    let vx = (idx % n0) as f32;
    let (ox, oy, oz) = (vx + 0.5, vy + 0.5, vz + 0.5);
    const APERTURE: f32 = 0.55; // tan(half-angle) ≈ 29°
    let nf = n0 as f32;
    let mut total = [0.0f32; 3];
    for d in cones {
        let mut acc = [0.0f32; 3];
        let mut aa = 0.0f32;
        let mut dist = 1.5f32;
        while aa < 0.95 {
            let px = ox + d[0] * dist; let py = oy + d[1] * dist; let pz = oz + d[2] * dist;
            if px < 0.0 || py < 0.0 || pz < 0.0 || px >= nf || py >= nf || pz >= nf { break; }
            let diam = (2.0 * dist * APERTURE).max(1.0);
            let lvl = (diam.log2().floor() as i32).clamp(0, n_lvl as i32 - 1) as usize;
            let (c, a) = sample_mip(&mips[lvl], n0, px, py, pz);
            let af = a.clamp(0.0, 1.0);
            acc[0] += (1.0 - aa) * af * c[0];
            acc[1] += (1.0 - aa) * af * c[1];
            acc[2] += (1.0 - aa) * af * c[2];
            aa += (1.0 - aa) * af;
            dist += ((1u32 << lvl) as f32).max(1.0);
            if dist > nf { break; }
        }
        total[0] += acc[0]; total[1] += acc[1]; total[2] += acc[2];
    }
    let inv = 1.0 / cones.len() as f32;
    [total[0] * inv, total[1] * inv, total[2] * inv]
}

/// HDR storage scale for the irradiance volume (stored value × this = world-space irradiance).
/// The world shader multiplies the sampled value by the same constant.
pub const IRRADIANCE_HDR_SCALE: f32 = 8.0;

/// Voxelize the loaded world. Call once at level load (after world surfaces/polys are built).
/// `res` is the per-axis voxel count; memory is 2·res³·4 bytes (64³ = 2 MiB for both volumes).
pub fn voxelize_world(res: u32) {
    let res = res.clamp(16, 256);

    use std::sync::atomic::{AtomicU32, Ordering};
    use rayon::prelude::*;
    let t_start = std::time::Instant::now();

    // --- Pass 1 (single thread, raw world pointers): gather triangles + cubic bounds. Each
    // triangle records `alb` (normal+coverage, 0 = no coverage e.g. sky) and `emit` (emission
    // colour, 0 = not an emitter). ---
    struct Tri { v0: [f32; 3], e1: [f32; 3], e2: [f32; 3], alb: u32, emit: u32 }
    // SAFETY: world surface/polygon data is fully built by now (mirrors parse_surface_lights).
    let (grid_min, extent, tris) = unsafe {
        use crate::vk_model_types::{SURF_DRAWSKY, VERTEXSIZE};
        use myq2_common::q_shared::SURF_LIGHT;

        let world = crate::vk_local::rfs().loadmodel;
        if world.is_null() {
            return;
        }
        let mut amin = [f32::INFINITY; 3];
        let mut amax = [f32::NEG_INFINITY; 3];
        let mut acc = |p: [f32; 3]| {
            for c in 0..3 {
                amin[c] = amin[c].min(p[c]);
                amax[c] = amax[c].max(p[c]);
            }
        };
        // Pack an 0..1 RGB into RGBA8 little-endian with full alpha.
        let pack = |c: [f32; 3]| -> u32 {
            let r = (c[0].clamp(0.0, 1.0) * 255.0) as u32;
            let g = (c[1].clamp(0.0, 1.0) * 255.0) as u32;
            let b = (c[2].clamp(0.0, 1.0) * 255.0) as u32;
            r | (g << 8) | (b << 16) | 0xFF00_0000
        };

        let mut tris: Vec<Tri> = Vec::new();
        let nsurf = (*world).numsurfaces;
        let surfaces = (*world).surfaces;
        if !surfaces.is_null() && nsurf > 0 {
            for i in 0..nsurf {
                let surf = &*surfaces.offset(i as isize);
                let is_sky = surf.flags & SURF_DRAWSKY != 0;
                let is_turb = surf.flags & crate::vk_model_types::SURF_DRAWTURB != 0;
                let ti = surf.texinfo;
                let is_light = !ti.is_null() && ((*ti).flags & SURF_LIGHT) != 0;
                // Emission colour: light textures emit near-neutral, sky emits cool. Liquids emit
                // a modest amount of their own tint so water/lava bounce coloured light onto the
                // walls and ceiling around them through the GI (light "cast back" from the water).
                let emit = if is_sky {
                    pack([0.75, 0.85, 1.0])
                } else if is_light {
                    // Emit the texture's own hue (normalized to full brightness), not a fixed
                    // near-white: lava bounces orange onto its cave, coloured light panels bounce
                    // their actual colour. Grey textures normalize to ~white = old behaviour.
                    let c = if !ti.is_null() && !(*ti).image.is_null() {
                        crate::vk_image::texture_avg_color((*(*ti).image).texnum)
                    } else {
                        [255u8, 247, 235]
                    };
                    let m = c[0].max(c[1]).max(c[2]).max(1) as f32;
                    pack([c[0] as f32 / m, c[1] as f32 / m, c[2] as f32 / m])
                } else if is_turb {
                    let c = if !ti.is_null() && !(*ti).image.is_null() {
                        crate::vk_image::texture_avg_color((*(*ti).image).texnum)
                    } else {
                        [60u8, 90, 110]
                    };
                    const LIQUID_EMIT: f32 = 0.35;
                    pack([
                        c[0] as f32 / 255.0 * LIQUID_EMIT,
                        c[1] as f32 / 255.0 * LIQUID_EMIT,
                        c[2] as f32 / 255.0 * LIQUID_EMIT,
                    ])
                } else {
                    0
                };
                // Albedo = this surface's average texture colour (sky stores none). Packed
                // RGBA8 with full coverage alpha; 0 = no coverage.
                let alb = if is_sky {
                    0
                } else {
                    let c = if !ti.is_null() && !(*ti).image.is_null() {
                        crate::vk_image::texture_avg_color((*(*ti).image).texnum)
                    } else {
                        [150u8, 150, 150]
                    };
                    (c[0] as u32) | ((c[1] as u32) << 8) | ((c[2] as u32) << 16) | 0xFF00_0000
                };
                let poly = surf.polys;
                if poly.is_null() {
                    continue;
                }
                let nv = (*poly).numverts;
                if nv < 3 {
                    continue;
                }
                let vptr = (*poly).verts.as_ptr() as *const f32;
                let read = |v: usize| -> [f32; 3] {
                    let b = vptr.add(v * VERTEXSIZE);
                    [*b, *b.add(1), *b.add(2)]
                };
                let v0 = read(0);
                acc(v0);
                for k in 1..(nv as usize - 1) {
                    let v1 = read(k);
                    let v2 = read(k + 1);
                    acc(v1);
                    acc(v2);
                    let e1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
                    let e2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];
                    tris.push(Tri { v0, e1, e2, alb, emit });
                }
            }
        }

        // Extend the AABB to EVERY BSP vertex, so the grid spans the whole level. Some surfaces
        // (water/warp, or polys not built at bake time) are skipped in the rasterization loop
        // above and would otherwise fall outside the grid → their world positions sample the
        // border → black patches when you walk into those areas.
        let nvtx = (*world).numvertexes;
        let vtx = (*world).vertexes;
        if !vtx.is_null() && nvtx > 0 {
            for i in 0..nvtx {
                acc((*vtx.offset(i as isize)).position);
            }
        }

        if tris.is_empty() || amin[0] > amax[0] {
            eprintln!("[vxgi] no world geometry to voxelize (tris={})", tris.len());
            return;
        }
        let size = [amax[0] - amin[0], amax[1] - amin[1], amax[2] - amin[2]];
        let extent = size[0].max(size[1]).max(size[2]).max(1.0) * 1.02;
        let center = [(amin[0] + amax[0]) * 0.5, (amin[1] + amax[1]) * 0.5, (amin[2] + amax[2]) * 0.5];
        let grid_min = [center[0] - extent * 0.5, center[1] - extent * 0.5, center[2] - extent * 0.5];
        eprintln!(
            "[vxgi] AABB mins=({:.0},{:.0},{:.0}) maxs=({:.0},{:.0},{:.0}) extent={:.0} nsurf={}",
            amin[0], amin[1], amin[2], amax[0], amax[1], amax[2], extent, nsurf
        );
        (grid_min, extent, tris)
    };

    // --- Pass 2 (parallel): rasterize into the albedo + radiance grids. `steps` is CAPPED at
    // `res` so a giant surface can never explode into billions of samples. ---
    let n = res as usize;
    let inv_voxel = res as f32 / extent;
    let albedo: Vec<AtomicU32> = (0..n * n * n).map(|_| AtomicU32::new(0)).collect();
    let radiance: Vec<AtomicU32> = (0..n * n * n).map(|_| AtomicU32::new(0)).collect();
    tris.par_iter().for_each(|tri| {
        let len1 = (tri.e1[0] * tri.e1[0] + tri.e1[1] * tri.e1[1] + tri.e1[2] * tri.e1[2]).sqrt();
        let len2 = (tri.e2[0] * tri.e2[0] + tri.e2[1] * tri.e2[1] + tri.e2[2] * tri.e2[2]).sqrt();
        let steps = (len1.max(len2) * inv_voxel * 1.2).ceil().clamp(1.0, res as f32) as i32;
        let inv_steps = 1.0 / steps as f32;
        for a in 0..=steps {
            for b in 0..=(steps - a) {
                let fa = a as f32 * inv_steps;
                let fb = b as f32 * inv_steps;
                let gx = ((tri.v0[0] + tri.e1[0] * fa + tri.e2[0] * fb - grid_min[0]) * inv_voxel) as i32;
                let gy = ((tri.v0[1] + tri.e1[1] * fa + tri.e2[1] * fb - grid_min[1]) * inv_voxel) as i32;
                let gz = ((tri.v0[2] + tri.e1[2] * fa + tri.e2[2] * fb - grid_min[2]) * inv_voxel) as i32;
                if gx < 0 || gy < 0 || gz < 0 || gx as u32 >= res || gy as u32 >= res || gz as u32 >= res {
                    continue;
                }
                let idx = ((gz as usize) * n + gy as usize) * n + gx as usize;
                if tri.alb != 0 {
                    albedo[idx].store(tri.alb, Ordering::Relaxed);
                }
                // Radiance volume: EVERY surface voxel is opaque (A=255) so cones get occluded
                // by walls; emitters additionally carry their emission colour in RGB.
                radiance[idx].store((tri.emit & 0x00FF_FFFF) | 0xFF00_0000, Ordering::Relaxed);
            }
        }
    });

    // Pass 3: splat static point-light entities (parse_static_lights) into the radiance volume
    // as emission spheres. Surface emitters (light textures, sky) are already in the grid from
    // the rasterization; this adds the lights that have NO visible surface, so VXGI has every
    // light source when it drives the lighting (r_vxgi_bake < 1). Sphere radius scales mildly
    // with intensity. These voxels emit but carry no opacity beyond a covered surface's, so
    // they don't wall off cones.
    {
        let lights = crate::vk_rsurf::STATIC_LIGHTS.lock().unwrap_or_else(|e| e.into_inner());
        for l in lights.iter() {
            let lr = (l.intensity / 130.0).clamp(1.0, 4.0);
            let cx = ((l.origin[0] - grid_min[0]) * inv_voxel) as i32;
            let cy = ((l.origin[1] - grid_min[1]) * inv_voxel) as i32;
            let cz = ((l.origin[2] - grid_min[2]) * inv_voxel) as i32;
            let packed = {
                let r = (l.color[0].clamp(0.0, 1.0) * 255.0) as u32;
                let g = (l.color[1].clamp(0.0, 1.0) * 255.0) as u32;
                let b = (l.color[2].clamp(0.0, 1.0) * 255.0) as u32;
                r | (g << 8) | (b << 16) | 0xFF00_0000
            };
            let ri = lr.ceil() as i32;
            for dz in -ri..=ri {
                for dy in -ri..=ri {
                    for dx in -ri..=ri {
                        if (dx * dx + dy * dy + dz * dz) as f32 > lr * lr {
                            continue;
                        }
                        let (gx, gy, gz) = (cx + dx, cy + dy, cz + dz);
                        if gx < 0 || gy < 0 || gz < 0 || gx as u32 >= res || gy as u32 >= res || gz as u32 >= res {
                            continue;
                        }
                        let idx = ((gz as usize) * n + gy as usize) * n + gx as usize;
                        radiance[idx].store(packed, Ordering::Relaxed);
                    }
                }
            }
        }
    }

    let alb_bytes: Vec<u8> = albedo.iter().flat_map(|c| c.load(Ordering::Relaxed).to_le_bytes()).collect();

    // --- Multi-bounce radiosity baked into the radiance volume ---
    // One bounce only lights surfaces right around each emitter; iterating spreads light
    // globally like offline radiosity. Each pass, every surface voxel gathers the first surface
    // hit along 14 directions (occluded by opacity) and re-emits direct + albedo·gathered.
    // The radiance volume now holds the converged INCOMING IRRADIANCE per surface voxel (a 3D
    // lightmap), which the world shader adds to the baked term before the per-texel texture
    // multiply — that keeps texture detail AND lets VXGI light dark areas. Emission is boosted
    // so bright lights actually read bright; the result is stored / IRRADIANCE_HDR_SCALE.
    let bounces = crate::vk_rmain::rcvars().r_vxgi_bounces.value.max(0.0) as u32;
    const EMISSION_BOOST: f32 = 6.0;
    let rad_bytes: Vec<u8> = {
        let t_b = std::time::Instant::now();
        let count = n * n * n;
        let unpack = |c: u32| -> [f32; 3] {
            [(c & 0xFF) as f32 / 255.0, ((c >> 8) & 0xFF) as f32 / 255.0, ((c >> 16) & 0xFF) as f32 / 255.0]
        };
        let opac: Vec<f32> = radiance.iter().map(|c| (((c.load(Ordering::Relaxed) >> 24) & 0xFF) as f32) / 255.0).collect();
        // Saturation boost on the bounce albedo. Quake wall textures average to a muted grey-brown,
        // so unboosted bounce light is near-grey ("washed out") even with perfect color storage.
        // Push each surface's average colour away from its own luminance (amt>1 = more saturated)
        // so coloured walls throw visibly coloured light, while neutral walls stay neutral.
        const ALB_SAT: f32 = 2.2;
        let albf: Vec<[f32; 3]> = albedo.iter().map(|c| {
            let a = unpack(c.load(Ordering::Relaxed));
            let l = 0.2126 * a[0] + 0.7152 * a[1] + 0.0722 * a[2];
            [
                (l + (a[0] - l) * ALB_SAT).clamp(0.0, 1.0),
                (l + (a[1] - l) * ALB_SAT).clamp(0.0, 1.0),
                (l + (a[2] - l) * ALB_SAT).clamp(0.0, 1.0),
            ]
        }).collect();
        // Boosted direct emission (light sources).
        let direct: Vec<[f32; 3]> = radiance.iter().map(|c| {
            let e = unpack(c.load(Ordering::Relaxed));
            [e[0] * EMISSION_BOOST, e[1] * EMISSION_BOOST, e[2] * EMISSION_BOOST]
        }).collect();
        // Mark sky voxels by their injected emission colour (cool/blue-dominant [0.75,0.85,1.0],
        // vs warm light textures [1,.97,.92]). gather_at uses this to skip distance falloff on sky
        // so open areas are lit by the sky regardless of how far overhead it is.
        let is_sky: Vec<bool> = radiance.iter().map(|c| {
            let e = unpack(c.load(Ordering::Relaxed) & 0x00FF_FFFF);
            e[2] > 0.9 && e[2] > e[0] + 0.1
        }).collect();
        let surf: Vec<usize> = (0..count).filter(|&i| opac[i] > 0.5).collect();
        // 26 gather directions (every neighbour of the 3×3×3 shell, normalized). More directions =
        // each voxel integrates the surrounding light far more uniformly, so adjacent voxels get
        // similar results instead of catching wildly different single first-hits = much less patchy.
        let dirs: Vec<[f32; 3]> = {
            let mut v = Vec::with_capacity(26);
            for dz in -1i32..=1 {
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        if dx == 0 && dy == 0 && dz == 0 { continue; }
                        let l = ((dx * dx + dy * dy + dz * dz) as f32).sqrt();
                        v.push([dx as f32 / l, dy as f32 / l, dz as f32 / l]);
                    }
                }
            }
            v
        };
        // Gather reach in voxels. Span the WHOLE grid so every surface can see the nearest lit
        // wall/ceiling however large the room — the first-hit break keeps it cheap in enclosed
        // areas (it stops at the first surface). Too short = large rooms gather nothing = black.
        let range: i32 = n as i32 - 2;
        let ni = n as i32;
        // Iterate the outgoing radiance (direct + albedo·gathered).
        let mut rad = direct.clone();
        for _ in 0..bounces {
            let rad_ref = &rad;
            let new_vals: Vec<[f32; 3]> = surf.par_iter().map(|&idx| {
                let gth = gather_at(idx, rad_ref, &opac, &is_sky, &dirs, n, ni, range);
                [direct[idx][0] + albf[idx][0] * gth[0], direct[idx][1] + albf[idx][1] * gth[1], direct[idx][2] + albf[idx][2] * gth[2]]
            }).collect();
            for (k, &idx) in surf.iter().enumerate() {
                rad[idx] = new_vals[k];
            }
        }
        // Saturation restore AT THE SOURCE, before the mip chain is built. Cone tracing averages many
        // differently-coloured voxels through the mips, which pulls light toward grey; saturating the
        // final radiance grid FIRST means every mip level (and thus every cone sample) carries the
        // boosted hue, instead of trying to re-tint an already-greyed average after the trace.
        const GI_SAT: f32 = 2.0;
        {
            use rayon::prelude::*;
            rad.par_iter_mut().for_each(|c| {
                let l = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
                c[0] = (l + (c[0] - l) * GI_SAT).max(0.0);
                c[1] = (l + (c[1] - l) * GI_SAT).max(0.0);
                c[2] = (l + (c[2] - l) * GI_SAT).max(0.0);
            });
        }
        // Final INCOMING irradiance via CONE TRACING (not single-ray gather). Build a mip chain of the
        // final bounced radiance, then trace wide cones per surface voxel through it. Each cone
        // samples coarser mips as it widens with distance, averaging many voxels per step — this is
        // what makes the result SMOOTH where the single-ray first-hit gather was speckly (the black
        // splotches). Distance falloff and sky both fall out of front-to-back occlusion naturally.
        // 14 cones (6 axis + 8 corners) gather more uniformly than 6 — smoother, more directional.
        let mut irr = vec![[0.0f32; 3]; count];
        let mips = build_rad_mips(&rad, &opac, n);
        const CC: f32 = 0.5773502691; // 1/sqrt(3)
        let cones: [[f32; 3]; 14] = [
            [1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, -1.0],
            [CC, CC, CC], [CC, CC, -CC], [CC, -CC, CC], [CC, -CC, -CC],
            [-CC, CC, CC], [-CC, CC, -CC], [-CC, -CC, CC], [-CC, -CC, -CC],
        ];
        let vals: Vec<[f32; 3]> = surf.par_iter().map(|&idx| {
            let gth = cone_trace_at(idx, &mips, n, &cones);
            [direct[idx][0] + gth[0], direct[idx][1] + gth[1], direct[idx][2] + gth[2]]
        }).collect();
        for (k, &idx) in surf.iter().enumerate() {
            irr[idx] = vals[k];
        }

        // Dilate the irradiance a few voxels outward — into empty space around surfaces AND into
        // surface voxels whose gather came back ~zero. The volume is sparse (data only AT surface
        // voxels), so a sample landing in an adjacent empty voxel blends with zero → black splotch.
        // Worse, deep-shadow surface voxels (especially WALLS, whose gather rays mostly point into
        // their own face = instant occlusion) gather zero and form hard black clusters that no
        // strength can fill. Treating those zero surface voxels as UNFILLED lets the flood pull light
        // from their lit neighbours, so a shadow patch in a lit area fills in smoothly — while a zero
        // cluster in a genuinely dark room has only dark neighbours and correctly stays dark.
        const DILATE: u32 = 14;
        let mut filled: Vec<bool> = (0..count)
            .map(|i| opac[i] > 0.5 && (irr[i][0] + irr[i][1] + irr[i][2]) > 1e-4)
            .collect();
        for _ in 0..DILATE {
            let irr_ref = &irr;
            let filled_ref = &filled;
            let updates: Vec<(usize, [f32; 3])> = (0..count).into_par_iter().filter_map(|idx| {
                if filled_ref[idx] { return None; }
                let z = (idx / (n * n)) as i32;
                let y = ((idx / n) % n) as i32;
                let x = (idx % n) as i32;
                let mut s = [0.0f32; 3];
                let mut c = 0u32;
                let nb = [(-1, 0, 0), (1, 0, 0), (0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1)];
                for (dx, dy, dz) in nb {
                    let (nx, ny, nz) = (x + dx, y + dy, z + dz);
                    if nx < 0 || ny < 0 || nz < 0 || nx >= ni || ny >= ni || nz >= ni { continue; }
                    let nidx = ((nz as usize) * n + ny as usize) * n + nx as usize;
                    if filled_ref[nidx] {
                        let e = irr_ref[nidx];
                        s[0] += e[0]; s[1] += e[1]; s[2] += e[2]; c += 1;
                    }
                }
                if c > 0 { Some((idx, [s[0] / c as f32, s[1] / c as f32, s[2] / c as f32])) } else { None }
            }).collect();
            for (idx, v) in updates {
                irr[idx] = v;
                filled[idx] = true;
            }
        }

        // Smooth the filled irradiance so neighbouring voxels that received very different amounts
        // of bounce don't show as hard light/dark patches. A few 6-neighbour box-blur iterations
        // over only the filled voxels (center weighted so light still tracks position) evens the
        // spread without bleeding into the empty exterior.
        const SMOOTH: u32 = 5;
        for _ in 0..SMOOTH {
            let irr_ref = &irr;
            let filled_ref = &filled;
            let blurred: Vec<(usize, [f32; 3])> = (0..count).into_par_iter().filter_map(|idx| {
                if !filled_ref[idx] { return None; }
                let z = (idx / (n * n)) as i32;
                let y = ((idx / n) % n) as i32;
                let x = (idx % n) as i32;
                // Center carries weight 2; each filled neighbour weight 1.
                let mut s = [irr_ref[idx][0] * 2.0, irr_ref[idx][1] * 2.0, irr_ref[idx][2] * 2.0];
                let mut c = 2.0f32;
                let nb = [(-1, 0, 0), (1, 0, 0), (0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1)];
                for (dx, dy, dz) in nb {
                    let (nx, ny, nz) = (x + dx, y + dy, z + dz);
                    if nx < 0 || ny < 0 || nz < 0 || nx >= ni || ny >= ni || nz >= ni { continue; }
                    let nidx = ((nz as usize) * n + ny as usize) * n + nx as usize;
                    if filled_ref[nidx] {
                        let e = irr_ref[nidx];
                        s[0] += e[0]; s[1] += e[1]; s[2] += e[2]; c += 1.0;
                    }
                }
                Some((idx, [s[0] / c, s[1] / c, s[2] / c]))
            }).collect();
            for (idx, v) in blurred {
                irr[idx] = v;
            }
        }

        eprintln!("[vxgi] {} bounce(s) over {} surface voxels in {:.0} ms",
            bounces, surf.len(), t_b.elapsed().as_secs_f32() * 1000.0);
        // Normalize the volume to its own bright-surface level instead of a fixed HDR clamp. A fixed
        // clamp saturates each channel independently at 1.0 → colored bounce (e.g. warm [6,5,4])
        // becomes white [1,1,1], desaturating exactly where light is strongest (the washout). And
        // the over-1 values reconstruct to 4-8× in-shader → surfaces blow into HDR → bloom explodes.
        // Instead: scale so the 95th-percentile BOUNCE-lit surface (emitters excluded — they SHOULD
        // clamp white) maps to ~0.82. Color is preserved below 1.0, and `strength 1` is a unit term.
        let mut lumas: Vec<f32> = surf.iter()
            .filter(|&&i| direct[i][0] + direct[i][1] + direct[i][2] < 0.01) // pure bounce, not a light
            .map(|&i| 0.2126 * irr[i][0] + 0.7152 * irr[i][1] + 0.0722 * irr[i][2])
            .filter(|&l| l > 1e-4)
            .collect();
        lumas.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let p95 = if lumas.is_empty() { 1.0 } else { lumas[(lumas.len() as f32 * 0.95) as usize % lumas.len()] };
        // Tonal compression (Reinhard on luminance), NOT a linear clamp. A linear scale can't fit the
        // huge bounce range — bright near-light voxels (luma ~13) and dim far surfaces in one 8-bit
        // volume — so any scale that brightens surfaces clamps the bright voxels per-channel to 255 =
        // white = washout. Instead map luma L -> L/(L+k) (always < 1, never white) and scale RGB by
        // the same factor so HUE IS PRESERVED at every brightness. k is set from the 95th percentile
        // so a bright bounce surface lands at ~0.8 and the curve compresses everything above it.
        let k = (p95 * 0.25).max(1e-3);
        // Compress on the MAX channel (not luminance): mapping the brightest channel through
        // m/(m+k) and scaling all three by the same factor guarantees every channel ends < 1 — so
        // nothing clamps to 255 and the hue is preserved EXACTLY even for strongly saturated voxels
        // (a luminance-based scale would push a dominant channel past 1.0 and re-clamp = wash).
        let tone = |c: [f32; 3]| -> [f32; 3] {
            let m = c[0].max(c[1]).max(c[2]);
            if m <= 1e-6 { return [0.0; 3]; }
            let mp = m / (m + k);        // compressed peak in [0,1)
            let s = mp / m;             // hue-preserving scale; all channels become <= mp < 1
            [c[0] * s, c[1] * s, c[2] * s]
        };
        (0..count).flat_map(|i| {
            let t = tone(irr[i]);
            [
                (t[0] * 255.0) as u8,
                (t[1] * 255.0) as u8,
                (t[2] * 255.0) as u8,
                (opac[i].clamp(0.0, 1.0) * 255.0) as u8,
            ]
        }).collect()
    };
    let occupied = albedo.iter().filter(|c| c.load(Ordering::Relaxed) != 0).count();
    // Emitters = voxels with non-zero emission RGB (every covered voxel now has A=255).
    let emitters = radiance.iter().filter(|c| c.load(Ordering::Relaxed) & 0x00FF_FFFF != 0).count();
    eprintln!(
        "[vxgi] voxelized res={} tris={} occupied={} emitters={} in {:.0} ms",
        res, tris.len(), occupied, emitters, t_start.elapsed().as_secs_f32() * 1000.0
    );

    // --- Create both 3D textures and store the grid. ---
    let alb_tex = create_voxel_texture(res, &alb_bytes, false);
    let rad_tex = create_voxel_texture(res, &rad_bytes, true);
    match (alb_tex, rad_tex) {
        (Some((image, memory, view, sampler)), Some((rad_image, rad_memory, rad_view, rad_sampler))) => {
            destroy_voxel_grid();
            *VOXEL_GRID.lock().unwrap_or_else(|e| e.into_inner()) = Some(VoxelGrid {
                image, memory, view, sampler,
                rad_image, rad_memory, rad_view, rad_sampler,
                res, grid_min, extent,
            });
        }
        _ => eprintln!("[vxgi] voxel grid creation FAILED"),
    }
}
