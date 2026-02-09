// Copyright (C) 1997-2001 Id Software, Inc.
// GPL-2.0-or-later
//
// vk_local.h -> vk_local.rs
// Renderer local definitions

#![allow(dead_code, non_upper_case_globals)]

use myq2_common::q_shared::*;

// Re-export Image and ImageType from vk_model_types (defined there to break
// the circular dependency: Image references MSurface, MSurface references Image).
pub use crate::vk_model_types::{Image, ImageType};

// Particle types — canonical i32 definitions in myq2_common::q_shared.
// Re-exported here as usize for array indexing convenience.
pub const PT_DEFAULT: usize = myq2_common::q_shared::PT_DEFAULT as usize;
pub const PT_FIRE: usize = myq2_common::q_shared::PT_FIRE as usize;
pub const PT_SMOKE: usize = myq2_common::q_shared::PT_SMOKE as usize;
pub const PT_BUBBLE: usize = myq2_common::q_shared::PT_BUBBLE as usize;
pub const PT_BLOOD: usize = myq2_common::q_shared::PT_BLOOD as usize;
pub const PT_MAX: usize = myq2_common::q_shared::PT_MAX as usize;

// ============================================================================
// MyQ2 build options (from myq2opts.h)
// ============================================================================

pub use myq2_common::common::{DISTNAME, DISTVER};

pub const NOTIFY_INDENT: i32 = 2;
pub const CON_TEXTSIZE: i32 = 131072;
pub const TRANS_CONSOLE_VALUE: f32 = 0.675;
pub const SKYBOX_SIZE: i32 = 4600;
pub const DLIGHT_CUTOFF: i32 = 16;
pub const OUTLINEDROPOFF: f32 = 1000.0;
pub const CEL_WIDTH: f32 = 1.50;

// Feature flags (compile-time options from myq2opts.h)
pub const HIRES_TEX_SCALING: bool = true;
pub const TGAPNG_TEX_LOADING: bool = true;
pub const SEED_RANDOM: bool = true;
pub const CONSOLE_INIT_EARLY: bool = true;
pub const AUTO_CVAR: bool = true;
pub const USE_WSAECONNRESET_FIX: bool = true;
pub const ENABLE_BOBBING_ITEMS: bool = true;
pub const AUTO_MOUSE_XPFIX: bool = true;
pub const DLIGHT_SURFACE_FIX: bool = true;
pub const TASKBAR_FIX: bool = true;
pub const PLAYER_MENU_FIX: bool = true;
pub const USE_CONSOLE_IN_DEMOS: bool = true;
pub const TRANS_CONSOLE: bool = true;
pub const BETTER_DLIGHT_FALLOFF: bool = true;
pub const DO_WATER_WAVES: bool = false; // disabled by default in myq2opts.h
pub const PRED_OUT_OF_DATE_FREEZE: bool = false;
pub const USE_UGLY_SKIN_FIX: bool = true;
pub const ENABLE_MOUSE4_MOUSE5: bool = true;
pub const PLAYER_OVERFLOW_FIX: bool = true;
pub const VISIBLE_GUN_WIDEANGLE: bool = true;
pub const CENTERED_GUN: bool = true;
pub const DISABLE_STARTUP_DEMO: bool = true;
pub const SWAP_UDP_FOR_TCP: bool = false;
pub const DO_REFLECTIVE_WATER: bool = true;

// ============================================================================
// Version
// ============================================================================

pub const REF_VERSION: &str = "GL 0.01";

// ============================================================================
// Angles — re-exported from q_shared
// ============================================================================
// PITCH, YAW, ROLL are defined in myq2_common::q_shared and re-exported
// via `use myq2_common::q_shared::*` at the top of this module.

// Re-export VidDef from the shared common crate so that vk_rmain and other
// renderer modules can continue to import it via `crate::vk_local::VidDef`.
pub use myq2_common::q_shared::VidDef;

// ============================================================================
// Texture constants
// ============================================================================

pub const TEXNUM_LIGHTMAPS: i32 = 1024;
pub const TEXNUM_SCRAPS: i32 = 1152;
pub const TEXNUM_IMAGES: i32 = 1153;
pub const MAX_GLTEXTURES: usize = 1024;

// ============================================================================
// rserr_t
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum RsErr {
    Ok = 0,
    InvalidFullscreen = 1,
    InvalidMode = 2,
    Unknown = 3,
}

// ============================================================================
// glvert_t
// ============================================================================

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct GlVert {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub s: f32,
    pub t: f32,
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

// ============================================================================
// Misc constants
// ============================================================================

pub const MAX_LBM_HEIGHT: i32 = 480;
pub const BACKFACE_EPSILON: f32 = 0.01;

// ============================================================================
// glconfig_t
// ============================================================================

#[repr(C)]
pub struct VkConfig {
    pub renderer: i32,
    pub renderer_string: *const u8,
    pub vendor_string: *const u8,
    pub version_string: *const u8,
    pub extensions_string: *const u8,

    pub allow_cds: i32, // qboolean

    pub mtexcombine: i32, // Vic - overbrightbits, qboolean

    pub anisotropy: i32,  // qboolean
    pub sgismipmap: i32,  // qboolean
    pub gammaramp: i32,   // MrG - BeefQuake - hardware gammaramp, qboolean
}

impl Default for VkConfig {
    fn default() -> Self {
        // SAFETY: All-zero is valid (null pointers, zero ints).
        unsafe { std::mem::zeroed() }
    }
}

// ============================================================================
// glstate_t
// ============================================================================

#[repr(C)]
pub struct VkState {
    pub inverse_intensity: f32,
    pub fullscreen: i32, // qboolean

    pub prev_mode: i32,

    pub lightmap_textures: i32,

    pub currenttextures: [i32; 4],
    pub currenttmu: i32,

    pub camera_separation: f32,
    pub stereo_enabled: i32, // qboolean

    pub original_red_gamma_table: [u8; 256],
    pub original_green_gamma_table: [u8; 256],
    pub original_blue_gamma_table: [u8; 256],

    // NiceAss: Set to true only after a R_SetGL2D
    pub transconsole: i32, // mattx86: trans_console, qboolean

    pub num_tmu: i32, // ep::multitexturing
}

impl Default for VkState {
    fn default() -> Self {
        // SAFETY: All-zero is valid for this repr(C) struct.
        unsafe { std::mem::zeroed() }
    }
}

// ============================================================================
// Cvar definitions (for modules that reference cvar pointers)
// ============================================================================

pub struct CvarRef {
    pub value: f32,
    pub string: &'static str,
    pub modified: bool,
}

impl Default for CvarRef {
    fn default() -> Self {
        Self {
            value: 0.0,
            string: "",
            modified: false,
        }
    }
}

// ============================================================================
// GL constants (matching OpenGL values)
// ============================================================================

pub const VK_TEXTURE_2D: u32 = 0x0DE1;
pub const VK_TEXTURE_MIN_FILTER: u32 = 0x2801;
pub const VK_TEXTURE_MAG_FILTER: u32 = 0x2800;
pub const VK_NEAREST: i32 = 0x2600;
pub const VK_LINEAR: i32 = 0x2601;
pub const VK_NEAREST_MIPMAP_NEAREST: i32 = 0x2700;
pub const VK_LINEAR_MIPMAP_NEAREST: i32 = 0x2701;
pub const VK_NEAREST_MIPMAP_LINEAR: i32 = 0x2702;
pub const VK_LINEAR_MIPMAP_LINEAR: i32 = 0x2703;
// GL_ALPHA_TEST removed — alpha testing handled by GLSL discard
pub const VK_BLEND: u32 = 0x0BE2;
pub const VK_RGBA: i32 = 0x1908;
pub const VK_RGB: i32 = 0x1907;
pub const VK_RGBA8: i32 = 0x8058;
pub const VK_RGB8: i32 = 0x8051;
pub const VK_RGB5_A1: i32 = 0x8057;
pub const VK_RGBA4: i32 = 0x8056;
pub const VK_RGBA2: i32 = 0x8055;
pub const VK_RGB5: i32 = 0x8D62;
pub const VK_RGB4: i32 = 0x804F;
pub const VK_R3_G3_B2: i32 = 0x2A10;
pub const VK_UNSIGNED_BYTE: u32 = 0x1401;
pub const VK_TEXTURE_ENV: u32 = 0x2300;
pub const VK_TEXTURE_ENV_MODE: u32 = 0x2200;
pub const VK_REPLACE: i32 = 0x1E01;
pub const VK_TEXTURE0: u32 = 0x84C0;
pub const VK_TEXTURE1: u32 = 0x84C1;
pub const VK_TEXTURE2: u32 = 0x84C2;
pub const VK_TEXTURE3: u32 = 0x84C3;
pub const VK_GENERATE_MIPMAP_SGIS: u32 = 0x8191;
pub const VK_TRUE: i32 = 1;
pub const VK_TEXTURE_MAX_ANISOTROPY_EXT: u32 = 0x84FE;

// ============================================================================
// Placeholder GL function calls
// ============================================================================

pub fn qvk_tex_parameterf(target: u32, pname: u32, param: f32) {
    // SAFETY: Delegates to OpenGL; GL must be loaded via crate::vk_bindings::load_with first.
    unsafe { crate::vk_bindings::TexParameterf(target, pname, param); }
}
pub fn qvk_tex_parameteri(target: u32, pname: u32, param: i32) {
    // SAFETY: Delegates to OpenGL; GL must be loaded via crate::vk_bindings::load_with first.
    unsafe { crate::vk_bindings::TexParameteri(target, pname, param); }
}
pub fn qvk_color4f(r: f32, g: f32, b: f32, a: f32) {
    // SAFETY: Delegates to OpenGL; GL must be loaded via crate::vk_bindings::load_with first.
    unsafe { crate::vk_bindings::Color4f(r, g, b, a); }
}
pub fn qvk_enable(cap: u32) {
    // SAFETY: Delegates to OpenGL; GL must be loaded via crate::vk_bindings::load_with first.
    unsafe { crate::vk_bindings::Enable(cap); }
}
pub fn qvk_disable(cap: u32) {
    // SAFETY: Delegates to OpenGL; GL must be loaded via crate::vk_bindings::load_with first.
    unsafe { crate::vk_bindings::Disable(cap); }
}
pub fn qvk_bind_texture(target: u32, texture: i32) {
    // SAFETY: Delegates to OpenGL; GL must be loaded via crate::vk_bindings::load_with first.
    unsafe { crate::vk_bindings::BindTexture(target, texture as u32); }
}
pub fn qvk_tex_image2d(
    target: u32, level: i32, internal_format: i32,
    width: i32, height: i32, border: i32,
    format: u32, data_type: u32, data: *const u8,
) {
    // SAFETY: Delegates to OpenGL; caller must ensure data pointer is valid.
    unsafe { crate::vk_bindings::TexImage2D(target, level, internal_format, width, height, border, format, data_type, data as *const std::ffi::c_void); }
}
pub fn qvk_delete_textures(n: i32, textures: &i32) {
    // SAFETY: Delegates to OpenGL; caller must ensure textures pointer is valid for n elements.
    unsafe { crate::vk_bindings::DeleteTextures(n, textures as *const i32 as *const u32); }
}
pub fn qvk_tex_envf(target: u32, pname: u32, param: f32) {
    // SAFETY: Delegates to OpenGL; GL must be loaded via crate::vk_bindings::load_with first.
    unsafe { crate::vk_bindings::TexEnvf(target, pname, param); }
}
pub fn qvk_select_texture_sgis(texture: u32) {
    // SAFETY: SGIS texture select maps to glActiveTexture in modern GL.
    unsafe { crate::vk_bindings::ActiveTexture(texture); }
}
pub fn qvk_active_texture_arb(texture: u32) {
    // SAFETY: ARB active texture maps to glActiveTexture.
    unsafe { crate::vk_bindings::ActiveTexture(texture); }
}
pub fn qvk_client_active_texture_arb(texture: u32) {
    // SAFETY: ARB client active texture maps to glClientActiveTexture.
    unsafe { crate::vk_bindings::ClientActiveTexture(texture); }
}

// ============================================================================
// Utility functions
// ============================================================================

// q_strcasecmp removed — use q_streq_nocase from myq2_common::q_shared::* instead
// com_strip_extension available from myq2_common::q_shared::* (imported above)
// little_short, little_long available from myq2_common::q_shared::* (imported above)

// ============================================================================
// Re-exports from vk_model_types used throughout the renderer
// ============================================================================

pub use crate::vk_model_types::{
    DvisT, GlPoly, MEdge, MLeaf, MModel, MNode, MSurface, MTexInfo, MVertex, Model, ModType,
    SURF_DRAWBACKGROUND, SURF_DRAWSKY, SURF_DRAWTURB, SURF_PLANEBACK, SURF_UNDERWATER,
    VERTEXSIZE,
};

// ============================================================================
// Entity and RefDef types (from ref.h) — imported from myq2-common
// ============================================================================

// DLight and LightStyle re-exported for use by other renderer modules.
pub use myq2_common::q_shared::{DLight, LightStyle};

/// entity_t — canonical definition in myq2-common, aliased here for renderer use.
/// The opaque RefModel/RefImage pointers can be cast to the concrete
/// renderer Model/Image types where needed.
pub use myq2_common::q_shared::RefEntity as Entity;

/// refdef_t — canonical definition in myq2-common, aliased here for renderer use.
pub use myq2_common::q_shared::RefRefDef as RefDef;

// ============================================================================
// Additional GL constants
// ============================================================================

pub const VK_ONE: u32 = 1;
pub const VK_ZERO: u32 = 0;
pub const VK_SRC_ALPHA: u32 = 0x0302;
pub const VK_SRC_COLOR: u32 = 0x0300;
pub const VK_ONE_MINUS_SRC_ALPHA: u32 = 0x0303;
pub const VK_DEPTH_TEST: u32 = 0x0B71;
pub const VK_MODULATE: u32 = 0x2100;
pub const VK_COMBINE_EXT: u32 = 0x8570;
pub const VK_COMBINE_RGB_EXT: u32 = 0x8571;
pub const VK_COMBINE_ALPHA_EXT: u32 = 0x8572;
pub const VK_COMBINE_ALPHA_ARB: u32 = 0x8572;
pub const VK_SOURCE0_RGB_EXT: u32 = 0x8580;
pub const VK_SOURCE1_RGB_EXT: u32 = 0x8581;
pub const VK_SOURCE0_ALPHA_EXT: u32 = 0x8588;
pub const VK_SOURCE1_ALPHA_EXT: u32 = 0x8589;
pub const VK_PREVIOUS_EXT: u32 = 0x8578;
pub const VK_RGB_SCALE_ARB: u32 = 0x8573;
pub const VK_RGB_SCALE_EXT: u32 = 0x8573;
pub const VK_INTENSITY8: u32 = 0x804B;
pub const VK_LUMINANCE8: u32 = 0x8040;
pub const VK_TEXTURE: u32 = 0x1702;

// ============================================================================
// Global renderer state (mutable statics — matches C globals)
// ============================================================================

// vid — canonical definition in vk_rmain.rs (VID). Use crate::vk_rmain::VID.

pub static mut r_origin: Vec3 = [0.0; 3];
pub static mut vup: Vec3 = [0.0; 3];
pub static mut vpn: Vec3 = [0.0; 3];
pub static mut vright: Vec3 = [0.0; 3];

pub static mut r_newrefdef: RefDef = unsafe { std::mem::zeroed() };

pub static mut r_worldmodel: *mut Model = std::ptr::null_mut();
pub static mut currentmodel: *mut Model = std::ptr::null_mut();
pub static mut currententity: *mut Entity = std::ptr::null_mut();

pub static mut r_framecount: i32 = 0;
pub static mut r_visframecount: i32 = 0;
pub static mut r_viewcluster: i32 = 0;
pub static mut r_viewcluster2: i32 = 0;
pub static mut r_oldviewcluster: i32 = 0;
pub static mut r_oldviewcluster2: i32 = 0;

pub static mut c_brush_polys: i32 = 0;
pub static mut c_alias_polys: i32 = 0;

pub static mut r_world_matrix: [f32; 16] = [0.0; 16];

pub static mut r_notexture: *mut Image = std::ptr::null_mut();

pub static mut gltextures: [Image; MAX_GLTEXTURES] = unsafe { std::mem::zeroed() };
pub static mut numgltextures: i32 = 0;

// ============================================================================
// Cvar refs — canonical definitions in vk_rmain.rs
// All cvar registration and updates happen there. Use crate::vk_rmain::* directly.
// ============================================================================

pub static mut vk_state: VkState = unsafe { std::mem::zeroed() };
pub static mut vk_config: VkConfig = unsafe { std::mem::zeroed() };

pub static mut vk_tex_solid_format_val: i32 = VK_RGB8;
pub static mut vk_tex_alpha_format_val: i32 = VK_RGBA8;

// ============================================================================
// Placeholder GL function stubs (additional)
// ============================================================================

pub fn qvk_translatef(x: f32, y: f32, z: f32) {
    // SAFETY: Delegates to OpenGL.
    unsafe { crate::vk_bindings::Translatef(x, y, z); }
}
pub fn qvk_rotatef(angle: f32, x: f32, y: f32, z: f32) {
    // SAFETY: Delegates to OpenGL.
    unsafe { crate::vk_bindings::Rotatef(angle, x, y, z); }
}
pub fn qvk_tex_envi(target: u32, pname: u32, param: i32) {
    // SAFETY: Delegates to OpenGL.
    unsafe { crate::vk_bindings::TexEnvi(target, pname, param); }
}
pub fn qvk_tex_sub_image_2d(
    target: u32, level: i32, xoffset: i32, yoffset: i32,
    width: i32, height: i32, format: u32, data_type: u32, data: *const u8,
) {
    // SAFETY: Delegates to OpenGL; caller must ensure data pointer is valid.
    unsafe { crate::vk_bindings::TexSubImage2D(target, level, xoffset, yoffset, width, height, format, data_type, data as *const std::ffi::c_void); }
}
pub fn qvk_tex_image_2d(
    target: u32, level: i32, internal_format: i32,
    width: i32, height: i32, border: i32,
    format: u32, data_type: u32, data: *const u8,
) {
    // SAFETY: Delegates to OpenGL; caller must ensure data pointer is valid.
    unsafe { crate::vk_bindings::TexImage2D(target, level, internal_format, width, height, border, format, data_type, data as *const std::ffi::c_void); }
}
// ============================================================================
// Accessor / helper functions for global state
// ============================================================================

/// Bind a texture if it's not already bound on the current TMU.
pub unsafe fn vk_bind(texnum: i32) {
    if vk_state.currenttextures[vk_state.currenttmu as usize] == texnum {
        return;
    }
    vk_state.currenttextures[vk_state.currenttmu as usize] = texnum;
    // SAFETY: Delegates to OpenGL.
    crate::vk_bindings::BindTexture(crate::vk_bindings::TEXTURE_2D, texnum as u32);
}

/// Bind a texture on a specific multitexture unit if not already bound.
pub unsafe fn vk_mbind(target: u32, texnum: i32) {
    let tmu = (target - VK_TEXTURE0) as usize;
    if vk_state.currenttextures[tmu] == texnum {
        return;
    }
    vk_select_texture(target);
    vk_state.currenttextures[tmu] = texnum;
    crate::vk_bindings::BindTexture(crate::vk_bindings::TEXTURE_2D, texnum as u32);
}

/// Set the texture environment mode for the current TMU.
pub unsafe fn vk_tex_env(value: u32) {
    // SAFETY: Delegates to OpenGL.
    crate::vk_bindings::TexEnvf(crate::vk_bindings::TEXTURE_ENV, crate::vk_bindings::TEXTURE_ENV_MODE, value as f32);
}

/// Enable or disable multitexturing.
pub unsafe fn vk_enable_multitexture(enable: bool) {
    if enable {
        vk_select_texture(VK_TEXTURE1);
        crate::vk_bindings::Enable(crate::vk_bindings::TEXTURE_2D);
        vk_tex_env(VK_REPLACE as u32);
    } else {
        vk_select_texture(VK_TEXTURE1);
        crate::vk_bindings::Disable(crate::vk_bindings::TEXTURE_2D);
        vk_tex_env(VK_REPLACE as u32);
        vk_select_texture(VK_TEXTURE0);
        vk_tex_env(VK_REPLACE as u32);
    }
}

/// Select a texture unit (VK_TEXTURE0, VK_TEXTURE1, etc.).
pub unsafe fn vk_select_texture(texture: u32) {
    let tmu = (texture - VK_TEXTURE0) as i32;
    if tmu == vk_state.currenttmu {
        return;
    }
    vk_state.currenttmu = tmu;
    // SAFETY: Delegates to OpenGL.
    crate::vk_bindings::ActiveTexture(texture);
    crate::vk_bindings::ClientActiveTexture(texture);
}

pub unsafe fn r_cull_box(mins: &Vec3, maxs: &Vec3) -> bool {
    crate::vk_rmain::r_cull_box(mins, maxs)
}
pub unsafe fn r_cull_box_raw(mins: *const f32, maxs: *const f32) -> bool {
    let mins_ref = &*(mins as *const Vec3);
    let maxs_ref = &*(maxs as *const Vec3);
    crate::vk_rmain::r_cull_box(mins_ref, maxs_ref)
}

pub unsafe fn vk_monolightmap_char() -> u8 {
    if crate::vk_rmain::VK_MONOLIGHTMAP.string.is_empty() {
        b'0'
    } else {
        crate::vk_rmain::VK_MONOLIGHTMAP.string.as_bytes()[0]
    }
}

pub unsafe fn vk_picmip_inc() { crate::vk_rmain::VK_PICMIP.value += 1.0; }
pub unsafe fn vk_picmip_dec() { crate::vk_rmain::VK_PICMIP.value -= 1.0; }

pub unsafe fn vk_find_image(name: &str, img_type: ImageType) -> *mut Image {
    crate::vk_image::vk_find_image_impl(name, img_type)
}

pub unsafe fn image_texnum(img: *mut Image) -> i32 {
    if img.is_null() { 0 } else { (*img).texnum }
}

pub unsafe fn image_has_alpha(img: *mut Image) -> bool {
    if img.is_null() { false } else { (*img).has_alpha != 0 }
}

// World model accessors
pub unsafe fn r_worldmodel_ptr() -> *mut Model { r_worldmodel }
pub unsafe fn r_worldmodel_surfaces() -> *mut MSurface {
    if r_worldmodel.is_null() { std::ptr::null_mut() } else { (*r_worldmodel).surfaces }
}
pub unsafe fn r_worldmodel_nodes() -> *mut MNode {
    if r_worldmodel.is_null() { std::ptr::null_mut() } else { (*r_worldmodel).nodes }
}
pub unsafe fn r_worldmodel_lightdata() -> *mut u8 {
    if r_worldmodel.is_null() { std::ptr::null_mut() } else { (*r_worldmodel).lightdata }
}
pub unsafe fn r_worldmodel_vis() -> *mut DvisT {
    if r_worldmodel.is_null() { std::ptr::null_mut() } else { (*r_worldmodel).vis }
}
pub unsafe fn r_worldmodel_numleafs() -> i32 {
    if r_worldmodel.is_null() { 0 } else { (*r_worldmodel).numleafs }
}
pub unsafe fn r_worldmodel_numnodes() -> i32 {
    if r_worldmodel.is_null() { 0 } else { (*r_worldmodel).numnodes }
}
pub unsafe fn r_worldmodel_leaf(i: i32) -> &'static mut MLeaf {
    &mut *(*r_worldmodel).leafs.offset(i as isize)
}
pub unsafe fn r_worldmodel_node(i: i32) -> *mut MNode {
    (*r_worldmodel).nodes.offset(i as isize)
}

// Current model accessors
pub unsafe fn currentmodel_nummodelsurfaces() -> i32 {
    if currentmodel.is_null() { 0 } else { (*currentmodel).nummodelsurfaces }
}
pub unsafe fn currentmodel_firstmodelsurface() -> i32 {
    if currentmodel.is_null() { 0 } else { (*currentmodel).firstmodelsurface }
}
pub unsafe fn currentmodel_firstnode() -> i32 {
    if currentmodel.is_null() { 0 } else { (*currentmodel).firstnode }
}
pub unsafe fn currentmodel_node(idx: i32) -> *mut MNode {
    (*currentmodel).nodes.offset(idx as isize)
}
pub unsafe fn currentmodel_surface(idx: i32) -> *mut MSurface {
    (*currentmodel).surfaces.offset(idx as isize)
}
pub unsafe fn currentmodel_radius() -> f32 {
    if currentmodel.is_null() { 0.0 } else { (*currentmodel).radius }
}
pub unsafe fn currentmodel_mins() -> Vec3 {
    if currentmodel.is_null() { [0.0; 3] } else { (*currentmodel).mins }
}
pub unsafe fn currentmodel_maxs() -> Vec3 {
    if currentmodel.is_null() { [0.0; 3] } else { (*currentmodel).maxs }
}
pub unsafe fn currentmodel_surfedge(idx: i32) -> i32 {
    *(*currentmodel).surfedges.offset(idx as isize)
}
pub unsafe fn currentmodel_vertex_position(idx: i32) -> Vec3 {
    (*(*currentmodel).vertexes.offset(idx as isize)).position
}
pub unsafe fn currentmodel_edge_v(edge_idx: i32, v: usize) -> i32 {
    (*(*currentmodel).edges.offset(edge_idx as isize)).v[v] as i32
}

// Load model accessors (used during surface subdivision)
pub static mut loadmodel: *mut Model = std::ptr::null_mut();

pub unsafe fn loadmodel_surfedge(idx: i32) -> i32 {
    *(*loadmodel).surfedges.offset(idx as isize)
}
pub unsafe fn loadmodel_vertex_position(idx: i32) -> Vec3 {
    (*(*loadmodel).vertexes.offset(idx as isize)).position
}
pub unsafe fn loadmodel_edge_v(edge_idx: i32, v: usize) -> i32 {
    (*(*loadmodel).edges.offset(edge_idx as isize)).v[v] as i32
}

// GL state accessors
pub unsafe fn vk_state_lightmap_textures() -> i32 { vk_state.lightmap_textures }
pub unsafe fn vk_state_inverse_intensity() -> f32 { vk_state.inverse_intensity }
pub unsafe fn vk_state_num_tmu() -> i32 { vk_state.num_tmu }
pub unsafe fn set_lightmap_textures(val: i32) { vk_state.lightmap_textures = val; }
pub unsafe fn set_current_textures(t0: i32, t1: i32) {
    vk_state.currenttextures[0] = t0;
    vk_state.currenttextures[1] = t1;
}
pub unsafe fn set_current_textures_all(val: i32) {
    for t in vk_state.currenttextures.iter_mut() { *t = val; }
}
pub unsafe fn vk_config_mtexcombine() -> bool { vk_config.mtexcombine != 0 }
pub unsafe fn vk_tex_solid_format() -> i32 { vk_tex_solid_format_val }
pub unsafe fn vk_tex_alpha_format() -> i32 { vk_tex_alpha_format_val }

// Multitexture query
pub unsafe fn has_multitexture() -> bool {
    // In the C code this checks qglSelectTextureSGIS || qglActiveTextureARB.
    // Without GL extension function pointers, use num_tmu > 1 as a proxy.
    vk_state.num_tmu > 1
}
pub unsafe fn has_mtex_coord() -> bool {
    // In the C code this checks qglMTexCoord2fSGIS != NULL.
    // Without GL extension function pointers, use num_tmu > 1 as a proxy.
    vk_state.num_tmu > 1
}

// Texture array accessors
pub unsafe fn num_vk_textures() -> i32 { numgltextures }
pub unsafe fn vk_texture_at(i: i32) -> *mut Image {
    &mut gltextures[i as usize]
}

// Fog density accessor
pub unsafe fn fog_density() -> f32 { crate::vk_rmain::FOG_DENSITY }

// RefDef accessors — lightstyle() and dlight() are defined on RefRefDef in myq2-common.

/// Mod_ClusterPVS — returns the PVS for the given cluster in the world model.
///
/// # Safety
/// Accesses global r_worldmodel pointer. Caller must ensure model is loaded.
pub unsafe fn mod_cluster_pvs(cluster: i32) -> *const u8 {
    crate::vk_model::mod_cluster_pvs_raw(cluster, r_worldmodel) as *const u8
}

// ============================================================================
// GlPoly vertex access helpers
// ============================================================================

/// Allocate a GlPoly with `numverts` vertices via heap allocation (replaces C Hunk_Alloc).
pub unsafe fn hunk_alloc_glpoly(numverts: usize) -> *mut GlPoly {
    let extra_verts = numverts.saturating_sub(4);
    let size = std::mem::size_of::<GlPoly>() + extra_verts * VERTEXSIZE * std::mem::size_of::<f32>();
    let layout = std::alloc::Layout::from_size_align(size, std::mem::align_of::<GlPoly>()).unwrap();
    let ptr = std::alloc::alloc_zeroed(layout) as *mut GlPoly;
    (*ptr).numverts = numverts as i32;
    ptr
}

/// Get a pointer to the vertex data for vertex `idx` in a GlPoly.
pub unsafe fn glpoly_vert_ptr(poly: *mut GlPoly, idx: i32) -> *mut f32 {
    let verts_ptr = (*poly).verts.as_mut_ptr() as *mut f32;
    verts_ptr.add(idx as usize * VERTEXSIZE)
}

/// Set the xyz position of vertex `idx`.
pub unsafe fn glpoly_set_vert(poly: *mut GlPoly, idx: i32, pos: &Vec3) {
    let v = glpoly_vert_ptr(poly, idx);
    *v.offset(0) = pos[0];
    *v.offset(1) = pos[1];
    *v.offset(2) = pos[2];
}

/// Set texture coordinates (s, t) at offsets [3] and [4] for vertex `idx`.
pub unsafe fn glpoly_set_st(poly: *mut GlPoly, idx: i32, s: f32, t: f32) {
    let v = glpoly_vert_ptr(poly, idx);
    *v.offset(3) = s;
    *v.offset(4) = t;
}

/// Set lightmap texture coordinates at offsets [5] and [6] for vertex `idx`.
pub unsafe fn glpoly_set_lm_st(poly: *mut GlPoly, idx: i32, s: f32, t: f32) {
    let v = glpoly_vert_ptr(poly, idx);
    *v.offset(5) = s;
    *v.offset(6) = t;
}

/// Copy all vertex data from src_idx to dst_idx.
pub unsafe fn glpoly_copy_vert(poly: *mut GlPoly, dst_idx: i32, src_idx: i32) {
    let src = glpoly_vert_ptr(poly, src_idx);
    let dst = glpoly_vert_ptr(poly, dst_idx);
    std::ptr::copy_nonoverlapping(src, dst, VERTEXSIZE);
}
