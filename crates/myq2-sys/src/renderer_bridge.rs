//! Thin wrappers that adapt myq2-renderer functions to the fn pointer signatures
//! expected by myq2-client's `RendererFunctions` table.

use myq2_client::client::RefDef;
use myq2_client::console::RendererFunctions;
use myq2_renderer::vk_draw;
use myq2_renderer::vk_image;
use myq2_renderer::vk_model;
use myq2_renderer::vk_rmain;
use myq2_renderer::vk_warp;

fn bridge_draw_char(x: i32, y: i32, num: i32) {
    // SAFETY: single-threaded engine
    unsafe { vk_draw::draw_char(x, y, num); }
}

fn bridge_draw_stretch_pic(x: i32, y: i32, w: i32, h: i32, name: &str) {
    // SAFETY: single-threaded engine
    unsafe { vk_draw::draw_stretch_pic(x, y, w, h, name); }
}

fn bridge_draw_pic(x: i32, y: i32, name: &str) {
    // SAFETY: single-threaded engine
    unsafe { vk_draw::draw_pic(x, y, name); }
}

fn bridge_draw_find_pic(name: &str) -> i32 {
    // SAFETY: single-threaded engine
    // Returns non-null pointer as 1 (found), null as 0 (not found).
    unsafe {
        let ptr = vk_image::draw_find_pic(name);
        if ptr.is_null() { 0 } else { 1 }
    }
}

fn bridge_draw_get_pic_size(name: &str) -> (i32, i32) {
    // SAFETY: single-threaded engine
    unsafe {
        let mut w: i32 = 0;
        let mut h: i32 = 0;
        vk_draw::draw_get_pic_size(&mut w, &mut h, name);
        (w, h)
    }
}

fn bridge_draw_fill(x: i32, y: i32, w: i32, h: i32, c: i32, alpha: f32) {
    // SAFETY: single-threaded engine
    unsafe { vk_draw::draw_fill(x, y, w, h, c, alpha); }
}

fn bridge_draw_tile_clear(x: i32, y: i32, w: i32, h: i32, name: &str) {
    // SAFETY: single-threaded engine
    unsafe { vk_draw::draw_tile_clear(x, y, w, h, name); }
}

fn bridge_draw_fade_screen() {
    // SAFETY: single-threaded engine
    unsafe { vk_draw::draw_fade_screen(); }
}

fn bridge_r_begin_frame(separation: f32) {
    vk_rmain::r_begin_frame(separation);
}

fn bridge_r_render_frame(refdef: &RefDef) {
    // Convert client RefDef to renderer RefdefLocal
    let local = vk_rmain::RefdefLocal {
        x: refdef.x,
        y: refdef.y,
        width: refdef.width,
        height: refdef.height,
        fov_x: refdef.fov_x,
        fov_y: refdef.fov_y,
        vieworg: refdef.vieworg,
        viewangles: refdef.viewangles,
        blend: refdef.blend,
        rdflags: refdef.rdflags,
        num_entities: refdef.num_entities as usize,
        entities: refdef.entities.iter().map(|e| {
            vk_rmain::EntityLocal {
                origin: e.origin,
                oldorigin: e.oldorigin,
                angles: e.angles,
                // SAFETY: model handle is a *mut Model cast to i32 by bridge_r_register_model.
                // Cast it back; null (0) becomes a null pointer.
                model: if e.model == 0 { std::ptr::null() } else { e.model as *const myq2_renderer::vk_model_types::Model },
                frame: e.frame,
                flags: e.flags,
                alpha: e.alpha,
                skinnum: e.skinnum,
            }
        }).collect(),
        num_particles: refdef.num_particles as usize,
        particles: refdef.particles.iter().map(|p| {
            vk_rmain::ParticleLocal {
                origin: p.origin,
                color: p.color as usize,
                alpha: p.alpha,
                particle_type: p.particle_type as usize,
            }
        }).collect(),
    };
    vk_rmain::r_render_frame(&local);
}

fn bridge_r_begin_registration(map: &str) {
    // SAFETY: single-threaded engine
    unsafe { vk_model::r_begin_registration(map); }
}

fn bridge_r_end_registration() {
    // SAFETY: single-threaded engine
    unsafe { vk_model::r_end_registration(); }
}

fn bridge_r_register_model(name: &str) -> i32 {
    // SAFETY: single-threaded engine
    // Returns non-null pointer as 1, null as 0. The client uses non-zero to
    // mean "valid handle".
    unsafe {
        let ptr = vk_model::r_register_model(name);
        if ptr.is_null() { 0 } else { ptr as i32 }
    }
}

fn bridge_r_register_skin(name: &str) -> i32 {
    // SAFETY: single-threaded engine
    // Returns non-null pointer as 1, null as 0.
    unsafe {
        let ptr = vk_image::r_register_skin(name);
        if ptr.is_null() { 0 } else { ptr as i32 }
    }
}

fn bridge_r_set_sky(name: &str, rotate: f32, axis: &[f32; 3]) {
    // SAFETY: single-threaded engine
    unsafe { vk_warp::r_set_sky(name, rotate, axis); }
}

fn bridge_r_set_palette_null() {
    vk_rmain::r_set_palette(None);
}

fn bridge_draw_stretch_raw(x: i32, y: i32, w: i32, h: i32, cols: i32, rows: i32, data: &[u8]) {
    // SAFETY: single-threaded engine, renderer global state
    unsafe {
        myq2_renderer::vk_draw::draw_stretch_raw(x, y, w, h, cols, rows, data.as_ptr());
    }
}

fn bridge_viddef_width() -> i32 {
    // SAFETY: single-threaded engine
    unsafe { myq2_renderer::vk_rmain::VID.width }
}

fn bridge_viddef_height() -> i32 {
    // SAFETY: single-threaded engine
    unsafe { myq2_renderer::vk_rmain::VID.height }
}

fn bridge_r_set_palette(palette: Option<&[u8]>) {
    myq2_renderer::vk_rmain::r_set_palette(palette);
}

fn bridge_r_add_stain(org: &[f32; 3], intensity: f32, r: f32, g: f32, b: f32, a: f32, mode: i32) {
    use myq2_renderer::vk_light::StainType;
    let stain_type = match mode {
        1 => StainType::Subtract,
        2 => StainType::Add,
        _ => StainType::Modulate,
    };
    // SAFETY: single-threaded engine, renderer global state
    unsafe {
        myq2_renderer::vk_light::add_stain(org, intensity, r, g, b, a, stain_type);
    }
}

fn bridge_vk_imp_end_frame() {
    crate::platform_register::with_platform(|s| {
        s.vk_imp.glimp_end_frame();
    });
}

/// Build a `RendererFunctions` table with real renderer wrappers.
pub fn make_renderer_fns() -> RendererFunctions {
    RendererFunctions {
        draw_char: bridge_draw_char,
        draw_stretch_pic: bridge_draw_stretch_pic,
        draw_pic: bridge_draw_pic,
        draw_find_pic: bridge_draw_find_pic,
        draw_get_pic_size: bridge_draw_get_pic_size,
        draw_fill: bridge_draw_fill,
        draw_tile_clear: bridge_draw_tile_clear,
        draw_fade_screen: bridge_draw_fade_screen,
        r_begin_frame: bridge_r_begin_frame,
        r_render_frame: bridge_r_render_frame,
        r_begin_registration: bridge_r_begin_registration,
        r_end_registration: bridge_r_end_registration,
        r_register_model: bridge_r_register_model,
        r_register_skin: bridge_r_register_skin,
        r_set_sky: bridge_r_set_sky,
        r_set_palette_null: bridge_r_set_palette_null,
        vk_imp_end_frame: bridge_vk_imp_end_frame,
        r_add_stain: bridge_r_add_stain,
        draw_stretch_raw: bridge_draw_stretch_raw,
        viddef_width: bridge_viddef_width,
        viddef_height: bridge_viddef_height,
        r_set_palette: bridge_r_set_palette,
    }
}
