// vk_rmain.rs — Main renderer routines
// Converted from: myq2-original/ref_gl/vk_rmain.c
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
    clippy::approx_constant,
    clippy::needless_return
)]

use myq2_common::q_shared::*;
// Note: we import only specific items from vk_local to avoid name conflicts,
// since vk_rmain defines its own GL constants and stub functions.
use std::sync::OnceLock;
use crate::vk_local::{Image, VkConfig, VkState, VidDef, CvarRef,
    PT_MAX, REF_VERSION,
};
use crate::modern::{ModernRenderPath, RenderPath, FrameParams, ParticleData};

// ============================================================
// Global state — RendererGlobals behind Mutex
// ============================================================

/// All renderer globals consolidated into a single Mutex-protected struct.
pub struct RendererGlobals {
    /// Modern VBO/shader-based renderer instance
    pub modern: Option<ModernRenderPath>,
    pub fog_type: i32,
    pub fog_density: f32,
    pub vid: VidDef,
    pub r_worldmodel: Option<Box<RWorldModel>>,
    pub gldepthmin: f32,
    pub gldepthmax: f32,
    pub vk_config: Option<VkConfig>,
    pub vk_state: Option<VkState>,
    pub r_notexture: Option<Image>,
    pub r_particletexture: [Option<Image>; PT_MAX],
    pub frustum: [CPlane; 4],
    pub v_blend: [f32; 4],
    pub max_aniso: i32,
    pub max_tsize: i32,
    pub r_newrefdef: Option<RefdefLocal>,
}

// SAFETY: RendererGlobals contains types with raw pointers (EntityLocal has *const Model,
// Image has *mut pointers, etc.) but all access is serialized by the Mutex.
unsafe impl Send for RendererGlobals {}

static RENDERER_GLOBALS: std::sync::Mutex<RendererGlobals> = std::sync::Mutex::new(RendererGlobals {
    modern: None,
    fog_type: 3,
    fog_density: 0.0,
    vid: VidDef { width: 0, height: 0 },
    r_worldmodel: None,
    gldepthmin: 0.0,
    gldepthmax: 0.0,
    vk_config: None,
    vk_state: None,
    r_notexture: None,
    r_particletexture: [None, None, None, None, None],
    frustum: [
        CPlane { normal: [0.0; 3], dist: 0.0, plane_type: 0, signbits: 0, pad: [0; 2] },
        CPlane { normal: [0.0; 3], dist: 0.0, plane_type: 0, signbits: 0, pad: [0; 2] },
        CPlane { normal: [0.0; 3], dist: 0.0, plane_type: 0, signbits: 0, pad: [0; 2] },
        CPlane { normal: [0.0; 3], dist: 0.0, plane_type: 0, signbits: 0, pad: [0; 2] },
    ],
    v_blend: [0.0; 4],
    max_aniso: 0,
    max_tsize: 0,
    r_newrefdef: None,
});

/// Lock the renderer globals. Recovers from poisoned mutex.
pub fn rg() -> std::sync::MutexGuard<'static, RendererGlobals> {
    RENDERER_GLOBALS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Access the modern renderer. Returns `None` if not yet initialized.
pub fn with_modern<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut ModernRenderPath) -> R,
{
    rg().modern.as_mut().map(f)
}

// viewcluster state uses vk_local::r_viewcluster* (lowercase) exclusively

// ============================================================
// Renderer cvar references — thread-safe via OnceLock
// ============================================================

/// All renderer cvars, initialized once in r_register().
pub struct RendererCvars {
    pub r_norefresh: CvarRef,
    pub r_drawentities: CvarRef,
    pub r_drawworld: CvarRef,
    pub r_speeds: CvarRef,
    pub r_fullbright: CvarRef,
    pub r_novis: CvarRef,
    pub r_nocull: CvarRef,
    pub r_lightlevel: CvarRef,
    pub r_overbrightbits: CvarRef,
    pub vk_ext_multitexture: CvarRef,
    pub vk_log: CvarRef,
    pub vk_drawbuffer: CvarRef,
    pub vk_driver: CvarRef,
    pub vk_lightmap: CvarRef,
    pub vk_shadows: CvarRef,
    pub vk_mode: CvarRef,
    pub vk_dynamic: CvarRef,
    pub vk_monolightmap: CvarRef,
    pub vk_modulate: CvarRef,
    pub vk_picmip: CvarRef,
    pub vk_skymip: CvarRef,
    pub vk_showtris: CvarRef,
    pub vk_ztrick: CvarRef,
    pub vk_finish: CvarRef,
    pub vk_clear: CvarRef,
    pub vk_cull: CvarRef,
    pub vk_polyblend: CvarRef,
    pub vk_flashblend: CvarRef,
    /// Draw an HDR-bright additive glow core at every light (dynamic + static map light) so
    /// the source itself blooms through the composite bloom pass. 0 = off.
    pub r_light_bloom: CvarRef,
    pub vk_saturatelighting: CvarRef,
    pub vk_swapinterval: CvarRef,
    pub vk_texturemode: CvarRef,
    pub vk_texturealphamode: CvarRef,
    pub vk_texturesolidmode: CvarRef,
    pub vk_lockpvs: CvarRef,
    pub vk_ext_texture_filter_anisotropic: CvarRef,
    pub vk_sgis_generate_mipmap: CvarRef,
    pub r_celshading: CvarRef,
    pub r_fog: CvarRef,
    pub r_timebasedfx: CvarRef,
    pub r_detailtexture: CvarRef,
    pub r_caustics: CvarRef,
    pub r_hwgamma: CvarRef,
    pub r_stainmap: CvarRef,
    pub r_verbose: CvarRef,
    pub r_fxaa: CvarRef,
    pub r_ssao: CvarRef,
    pub r_ssao_radius: CvarRef,
    pub r_ssao_intensity: CvarRef,
    pub r_bloom: CvarRef,
    pub r_bloom_threshold: CvarRef,
    pub r_bloom_intensity: CvarRef,
    pub r_fsr: CvarRef,
    pub r_fsr_scale: CvarRef,
    pub r_fsr_sharpness: CvarRef,
    /// MSAA sample count: 0=disabled, 2, 4, or 8
    pub r_msaa: CvarRef,
    /// Anisotropic filtering level: 1=disabled, 2, 4, 8, or 16
    pub r_anisotropy: CvarRef,
    /// Screenshot format: "tga", "png", or "jpg"
    pub vk_screenshot_format: CvarRef,
    /// JPEG screenshot quality: 0-100 (only used for jpg format)
    pub vk_screenshot_quality: CvarRef,
    pub vk_3dlabs_broken: CvarRef,
    pub vid_fullscreen: CvarRef,
    pub vid_gamma: CvarRef,
    pub vid_ref: CvarRef,
    /// Saturation multiplier for postprocess (1.0 = neutral, >1.0 = more vivid).
    pub r_saturation: CvarRef,
    /// Contrast multiplier for postprocess (1.0 = neutral).
    pub r_contrast: CvarRef,
    /// Brightness (exposure) multiplier for postprocess (1.0 = neutral).
    pub r_brightness: CvarRef,
    /// Shadow lift — raises pitch-black floors (0.0 = none, 0.05 = subtle).
    pub r_shadowlift: CvarRef,
    /// Color grade preset: 0=off, 1=enhanced, 2=cool, 3=warm, 4=high_contrast.
    pub r_color_grade: CvarRef,
    /// Color grade LUT blend intensity (0.0-1.0).
    pub r_color_grade_intensity: CvarRef,
    /// Lightmap gamma — power curve applied to lightmap before overbright multiply.
    /// Values > 1.0 deepen shadows in dark rooms (1.0 = neutral, 1.5 = moderate darkening).
    pub r_lightmap_gamma: CvarRef,
    /// Lightmap contrast — linear contrast around pivot 0.35 applied after gamma.
    /// Steepens gradient in shadow zone; 1.0 = neutral, 1.3 = moderate, 1.8 = aggressive.
    pub r_lightmap_contrast: CvarRef,
    /// Ambient floor for Doom 3 lighting (0.0=pitch-black, 0.05=subtle fill).
    pub r_d3_ambient: CvarRef,
    /// Maximum point lights evaluated per frame (default 8).
    pub r_d3_maxlights: CvarRef,
    /// Shadow volume extrude distance multiplier (default 1.0).
    pub r_d3_extrude: CvarRef,
    /// Blinn-Phong specular exponent (default 32.0).
    pub r_d3_spec_power: CvarRef,
    /// Real-time surface-light emission: number of nearest light emitters that additively
    /// fill the DARK areas of nearby surfaces (the baked lightmap stays the base; this only
    /// brightens shadowed walls/floors near a fixture, avoiding the global warm tint of naive
    /// additive lighting). 0 = off. Uses shadow cubemaps for occlusion.
    pub r_surf_emit: CvarRef,
    /// Voxel cone-traced GI master switch. When on, the static world is voxelized at level
    /// load (Phase 1). 0 = off.
    pub r_vxgi: CvarRef,
    /// VXGI voxel grid resolution per axis (memory is res³·4 bytes; 64 = 1 MiB, 128 = 8 MiB).
    pub r_vxgi_res: CvarRef,
    /// Debug: raymarch and display the voxel grid instead of the scene (1 = on).
    pub r_vxgi_debug: CvarRef,
    /// Enable the diffuse cone-traced GI pass (adds bounced light to the scene). 0 = off.
    pub r_vxgi_gi: CvarRef,
    /// GI strength multiplier (how bright the gathered indirect light is added).
    pub r_vxgi_strength: CvarRef,
    /// Baked-lightmap scale: 1.0 = full baked (classic), lower lets real-time VXGI DRIVE the
    /// lighting instead of the baked radiosity (e.g. 0.3). 0 = baked fully off.
    pub r_vxgi_bake: CvarRef,
    /// Multi-bounce radiosity passes baked into the voxel radiance volume at load (so light
    /// spreads globally like radiosity). 0 = direct emission only. Changing it needs a map reload.
    pub r_vxgi_bounces: CvarRef,
    /// Planar water reflections: 0 = off, 1 = mirrored world+sky rendered into a half-res
    /// target and Fresnel-blended onto the dominant horizontal water plane.
    pub r_water_reflect: CvarRef,
    /// Water reflection blend strength (scales the Fresnel term).
    pub r_water_reflect_strength: CvarRef,
    /// Animated water-ripple light on walls/floors near the active water plane.
    /// 0 = off; otherwise a strength multiplier (1 = default look).
    pub r_water_shimmer: CvarRef,
}

static RENDERER_CVARS: OnceLock<RendererCvars> = OnceLock::new();

/// Access the renderer cvars. Panics if r_register() has not been called.
pub fn rcvars() -> &'static RendererCvars {
    RENDERER_CVARS.get().expect("r_register() not called")
}

// r_rawpalette — canonical definition in vk_image.rs
// Use crate::vk_image::r_rawpalette directly.

// ============================================================
// Helper types used only in this module
// ============================================================

// CPlane is imported from myq2_common::q_shared::*

/// Minimal world model reference
#[derive(Debug, Clone, Default)]
pub struct RWorldModel {
    pub nodes: Vec<u8>, // Placeholder; proper mnode_t tree is in vk_model_types::MNode
}

/// Local refdef matching C refdef_t
#[derive(Debug, Clone, Default)]
pub struct RefdefLocal {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub fov_x: f32,
    pub fov_y: f32,
    pub vieworg: Vec3,
    pub viewangles: Vec3,
    pub blend: [f32; 4],
    pub rdflags: i32,
    pub num_entities: usize,
    pub entities: Vec<EntityLocal>,
    pub num_particles: usize,
    pub particles: Vec<ParticleLocal>,
    pub dlights: Vec<DLight>,
}

#[derive(Debug, Clone)]
pub struct EntityLocal {
    pub origin: Vec3,
    pub oldorigin: Vec3,
    pub angles: Vec3,
    /// Raw pointer to the renderer's Model struct (from mod_known table).
    /// Null means no model. The pointer is valid for the duration of the frame.
    pub model: *const crate::vk_model_types::Model,
    pub frame: i32,
    pub oldframe: i32,
    pub backlerp: f32,
    pub flags: i32,
    pub alpha: f32,
    pub skinnum: i32,
    pub lightstyle: i32,
    pub skin: isize,
}

impl Default for EntityLocal {
    fn default() -> Self {
        Self {
            origin: Vec3::default(),
            oldorigin: Vec3::default(),
            angles: Vec3::default(),
            model: std::ptr::null(),
            frame: 0,
            oldframe: 0,
            backlerp: 0.0,
            flags: 0,
            alpha: 0.0,
            skinnum: 0,
            lightstyle: 0,
            skin: 0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ModelLocal {
    pub model_type: i32, // mod_alias, mod_brush, mod_sprite
    pub skins: Vec<Option<Image>>,
    pub extradata: Vec<u8>,
}

pub const MOD_ALIAS: i32 = 3;
pub const MOD_BRUSH: i32 = 1;
pub const MOD_SPRITE: i32 = 2;

#[derive(Debug, Clone, Default)]
pub struct ParticleLocal {
    pub origin: Vec3,
    pub color: usize,
    pub alpha: f32,
    pub particle_type: usize, // PT_DEFAULT, PT_FIRE, etc.
}

// PT_* particle constants come from vk_local

// RF_* and RDF_* come from myq2_common::q_shared::* (imported above)

// GL constants needed locally
pub const VK_FRONT: u32 = 0x0404;
pub const VK_BACK: u32 = 0x0405;
pub const VK_BACK_LEFT: u32 = 0x0402;
pub const VK_FRONT_AND_BACK: u32 = 0x0408;
pub const VK_DEPTH_TEST: u32 = 0x0B71;
pub const VK_CULL_FACE: u32 = 0x0B44;
// GL_FOG removed — fixed-function fog replaced by GLSL shader fog
pub const VK_SCISSOR_TEST: u32 = 0x0C11;
pub const VK_STENCIL_BUFFER_BIT: u32 = 0x00000400;
pub const VK_COLOR_BUFFER_BIT: u32 = 0x00004000;
pub const VK_DEPTH_BUFFER_BIT: u32 = 0x00000100;
pub const VK_MODULATE: u32 = 0x2100;
pub const VK_COMBINE_EXT: u32 = 0x8570;
pub const VK_TEXTURE_ENV_MODE_C: u32 = 0x2200;
pub const VK_COMBINE_RGB_EXT: u32 = 0x8571;
pub const VK_COMBINE_ALPHA_ARB: u32 = 0x8572;
pub const VK_RGB_SCALE_ARB: u32 = 0x8573;
pub const VK_LEQUAL: u32 = 0x0203;
pub const VK_GEQUAL: u32 = 0x0206;
pub const VK_GREATER: u32 = 0x0204;
pub const VK_PROJECTION: u32 = 0x1701;
pub const VK_MODELVIEW: u32 = 0x1700;
pub const VK_MODELVIEW_MATRIX: u32 = 0x0BA6;
pub const VK_SRC_ALPHA: u32 = 0x0302;
pub const VK_ONE_MINUS_SRC_ALPHA: u32 = 0x0303;
pub const VK_TRIANGLE_FAN: u32 = 0x0006;
pub const VK_TRIANGLE_STRIP: u32 = 0x0005;
pub const VK_LINES: u32 = 0x0001;
pub const VK_NO_ERROR: u32 = 0;
pub const VK_VENDOR: u32 = 0x1F00;
pub const VK_RENDERER_ID: u32 = 0x1F01;
pub const VK_VERSION: u32 = 0x1F02;
pub const VK_EXTENSIONS: u32 = 0x1F03;
pub const VK_MAX_TEXTURE_MAX_ANISOTROPY_EXT: u32 = 0x84FF;
pub const VK_MAX_TEXTURE_SIZE: u32 = 0x0D33;
pub const VK_MAX_TEXTURE_UNITS: u32 = 0x84E2;
pub const VK_TEXTURE0_ARB: i32 = 0x84C0;
pub const VK_TEXTURE1_ARB: i32 = 0x84C1;
pub const VK_TEXTURE2_ARB: i32 = 0x84C2;
pub const VK_TEXTURE3_ARB: i32 = 0x84C3;
pub const VK_TEXTURE0_SGIS: i32 = 0x835E;
pub const VK_TEXTURE1_SGIS: i32 = 0x835F;
pub const VK_TEXTURE2_SGIS: i32 = 0x8360;
pub const VK_TEXTURE3_SGIS: i32 = 0x8361;
// GL_FOG_MODE, GL_FOG_COLOR, GL_FOG_START, GL_FOG_END, GL_FOG_DENSITY,
// GL_FOG_HINT, GL_LINEAR_C removed — fixed-function fog replaced by
// GLSL shader fog via u_FogDensity / u_FogColor uniforms.
pub const VK_NICEST: u32 = 0x1102;
pub const VK_CLIP_PLANE0: u32 = 0x3000;
pub const VK_FALSE: u8 = 0;
pub const VK_TRUE_U8: u8 = 1;
pub const VK_BLEND: u32 = 0x0BE2;
// GL_ALPHA_TEST removed — alpha testing handled by GLSL discard
pub const VK_TEXTURE_2D: u32 = 0x0DE1;
pub const VK_TEXTURE_ENV: u32 = 0x2300;
pub const VK_REPLACE: u32 = 0x1E01;
pub const VK_ONE: u32 = 1;
pub const VK_STENCIL_TEST: u32 = 0x0B90;

// GL renderer identification flags
pub const SKYBOX_SIZE: f64 = 4096.0;
pub const NUM_BEAM_SEGS: usize = 6;

// CVAR flags come from myq2_common::q_shared::* (imported above)

// d_8to24table — canonical definition in vk_image.rs
// Use crate::vk_image::d_8to24table directly.

// ============================================================
// Placeholder external functions
// ============================================================

// --- Already wired to real implementations ---
unsafe fn r_light_point(p: &Vec3, color: &mut Vec3) { crate::vk_light::r_light_point(p, color); }
unsafe fn r_push_dlights() { crate::vk_light::r_push_dlights(); }
unsafe fn r_mark_leaves() { crate::vk_rsurf::r_mark_leaves(); }
unsafe fn vk_init_images() { crate::vk_image::vk_init_images(); }
unsafe fn vk_shutdown_images() { crate::vk_image::vk_shutdown_images(); }
fn mod_init() { crate::vk_model::mod_init(); }
fn mod_free_all() { crate::vk_model::mod_free_all(); }
unsafe fn draw_get_palette() { crate::vk_image::draw_get_palette(); }
unsafe fn draw_init_local() { crate::vk_draw::draw_init_local(); }

// --- Wired to vk_image.rs ---
fn vk_image_list_f() { unsafe { crate::vk_image::vk_image_list_f(); } }
fn vk_texture_mode(mode: &str) { unsafe { crate::vk_image::vk_texture_mode(mode); } }
fn vk_texture_alpha_mode(mode: &str) { unsafe { crate::vk_image::vk_texture_alpha_mode(mode); } }
fn vk_texture_solid_mode(mode: &str) { unsafe { crate::vk_image::vk_texture_solid_mode(mode); } }
fn vk_bind(texnum: i32) { unsafe { crate::vk_image::vk_bind_impl(texnum); } }
fn vk_tex_env(mode: u32) { unsafe { crate::vk_image::vk_tex_env_impl(mode as i32); } }

// --- Wired to vk_model.rs ---
fn mod_modellist_f() { crate::vk_model::mod_modellist_f(); }

// --- Entity rendering bridges ---
// EntityLocal (vk_rmain.rs local type) and Entity (vk_local.rs / vk_model.rs raw pointer type)
// are structurally different. Once the model system is fully unified under vk_model::Model,
// these bridges will convert EntityLocal -> Entity and call the real implementations.
// For now they are no-ops; the geometry will not render until the type unification is complete.
fn r_draw_alias_model(_e: &EntityLocal, _translucent: bool) {
    // No-op stub. Modern renderer handles MD2 alias models via modern::geometry::alias.
}
fn r_draw_brush_model(_e: &EntityLocal) {
    // Requires EntityLocal -> vk_local::Entity conversion.
    // Will call crate::vk_rsurf::r_draw_brush_model once types are unified.
}

// --- mod_point_in_leaf bridge ---
// RWorldModel is a local placeholder; vk_model::mod_point_in_leaf needs a raw Model*.
// Once the world model is loaded via vk_model, rg().r_worldmodel will hold the real Model*
// and this bridge can delegate directly.
fn mod_point_in_leaf(_p: &Vec3, _model: &RWorldModel) -> MleafLocal {
    // Returns a default (empty) leaf until the world model type is unified.
    MleafLocal::default()
}

fn cm_point_contents(p: &Vec3, headnode: i32) -> i32 {
    myq2_common::cmodel::cm_point_contents(p, headnode)
}

// --- Platform layer dispatch (GLimp/QGL init/shutdown) ---
// These delegate to myq2-sys via crate::platform, which holds function
// pointers registered by the platform layer at startup.
fn vid_menu_init() { crate::platform::vid_menu_init(); }
fn glimp_init(hinstance: usize, hwnd: usize) -> bool { crate::platform::glimp_init(hinstance, hwnd) }
fn glimp_shutdown() { crate::platform::glimp_shutdown(); }
fn glimp_begin_frame(camera_separation: f32) { crate::platform::glimp_begin_frame(camera_separation); }
fn glimp_end_frame() { crate::platform::glimp_end_frame(); }
fn glimp_set_mode(
    width: &mut i32,
    height: &mut i32,
    mode: f32,
    fullscreen: bool,
) -> i32 {
    crate::platform::glimp_set_mode(width, height, mode, fullscreen)
}

// --- Vulkan call logging (mirrors GLimp_EnableLogging / GLimp_LogNewFrame from original qgl_win.c) ---
// The original OpenGL implementation swapped function pointers to logging wrappers.
// With Vulkan 1.3 via ash, validation layers provide comprehensive API tracing.
// This log file supports the vk_log cvar for frame markers and debug output.
static VK_LOG_FP: std::sync::Mutex<Option<std::fs::File>> = std::sync::Mutex::new(None);

fn glimp_enable_logging(enable: f32) {
    let mut log_fp = VK_LOG_FP.lock().unwrap_or_else(|e| e.into_inner());
    if enable != 0.0 {
        if log_fp.is_none() {
            let gamedir = myq2_common::files::fs_gamedir();
            let path = format!("{}/gl.log", gamedir);
            match std::fs::File::create(&path) {
                Ok(f) => {
                    use std::io::Write;
                    let mut f = f;
                    let _ = writeln!(f, "GL log opened");
                    myq2_common::common::com_printf(&format!("GL logging to {}\n", path));
                    *log_fp = Some(f);
                }
                Err(e) => {
                    myq2_common::common::com_printf(
                        &format!("GLimp_EnableLogging: failed to open {}: {}\n", path, e),
                    );
                }
            }
        }
    } else {
        // Disable logging — close the file
        *log_fp = None;
    }
}

fn glimp_log_new_frame() {
    let mut log_fp = VK_LOG_FP.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(ref mut f) = *log_fp {
        use std::io::Write;
        let _ = writeln!(f, "*** R_BeginFrame ***");
    }
}

// --- Platform gamma ramp ---
fn update_gamma_ramp() { crate::platform::update_gamma_ramp(); }

// --- Wired to myq2_common ---
fn cbuf_add_text(text: &str) {
    myq2_common::cmd::cbuf_add_text(text);
}

const RSERR_OK: i32 = 0;
const RSERR_INVALID_FULLSCREEN: i32 = 1;
const RSERR_INVALID_MODE: i32 = 2;

// Cvar wrapper — adapt return type from Option<usize> to CvarRef
fn cvar_get(name: &str, default: &'static str, flags: i32) -> CvarRef {
    let handle = myq2_common::cvar::cvar_get(name, default, flags).unwrap_or(usize::MAX);
    // NOTE: r_register() is called very early (before cvar system is fully initialized),
    // so cvar_variable_value/string may not return the correct values yet.
    // Use the default value directly for the initial cached value.
    let value = default.parse::<f32>().unwrap_or(0.0);
    let string = default;
    CvarRef {
        string,
        value,
        handle,
    }
}
fn cvar_set(name: &str, value: &str) {
    myq2_common::cvar::cvar_set(name, value);
}
fn cvar_set_value(name: &str, value: f32) {
    myq2_common::cvar::cvar_set_value(name, value);
}
fn cmd_add_command(name: &str, func: fn()) {
    myq2_common::cmd::cmd_add_command_simple(name, func);
}
fn cmd_remove_command(name: &str) {
    myq2_common::cmd::cmd_remove_command(name);
}

pub fn vid_printf(level: i32, msg: &str) {
    if level == PRINT_DEVELOPER {
        myq2_common::common::com_dprintf(msg);
    } else {
        myq2_common::common::com_printf(msg);
    }
}

// ============================================================
// GL wrapper functions — wired to crate::vk_local where available,
// remaining ones are stubs pending full GL bindings.
// ============================================================

// --- Wired to crate::vk_local (used by 2D rendering) ---
fn qvk_enable(cap: u32) { crate::vk_local::qvk_enable(cap); }
fn qvk_disable(cap: u32) { crate::vk_local::qvk_disable(cap); }
fn qvk_color4f(r: f32, g: f32, b: f32, a: f32) { crate::vk_local::qvk_color4f(r, g, b, a); }

// Vestigial GL wrappers removed: qvk_translate_f, qvk_rotate_f, qvk_matrix_mode,
// qvk_load_identity, qvk_ortho, qvk_frustum, qvk_cull_face - modern renderer ignores GL state

fn qvk_viewport(x: i32, y: i32, w: i32, h: i32) {
    // SAFETY: Delegates to OpenGL.
    unsafe { crate::vk_bindings::Viewport(x, y, w, h); }
}
fn qvk_scissor(x: i32, y: i32, w: i32, h: i32) {
    // SAFETY: Delegates to OpenGL.
    unsafe { crate::vk_bindings::Scissor(x, y, w, h); }
}
fn qvk_depth_func(func: u32) {
    // SAFETY: Delegates to OpenGL.
    unsafe { crate::vk_bindings::DepthFunc(func); }
}
fn qvk_depth_range(near: f64, far: f64) {
    // SAFETY: Delegates to OpenGL.
    unsafe { crate::vk_bindings::DepthRange(near, far); }
}
fn qvk_alpha_func(func: u32, ref_val: f32) {
    // SAFETY: Delegates to OpenGL.
    unsafe { crate::vk_bindings::AlphaFunc(func, ref_val); }
}
fn qvk_clear_color(r: f32, g: f32, b: f32, a: f32) {
    // SAFETY: Delegates to OpenGL.
    unsafe { crate::vk_bindings::ClearColor(r, g, b, a); }
}
fn qvk_clear(mask: u32) {
    // SAFETY: Delegates to OpenGL.
    unsafe { crate::vk_bindings::Clear(mask); }
}
fn qvk_clear_stencil(s: i32) {
    // SAFETY: Delegates to OpenGL.
    unsafe { crate::vk_bindings::ClearStencil(s); }
}
fn qvk_finish() {
    // SAFETY: Delegates to OpenGL.
    unsafe { crate::vk_bindings::Finish(); }
}
fn qvk_get_floatv(pname: u32, params: &mut [f32; 16]) {
    // SAFETY: Delegates to OpenGL; params points to valid memory.
    unsafe { crate::vk_bindings::GetFloatv(pname, params.as_mut_ptr()); }
}
fn qvk_get_integerv(pname: u32, params: &mut i32) {
    // SAFETY: Delegates to OpenGL; params points to valid memory.
    unsafe { crate::vk_bindings::GetIntegerv(pname, params as *mut i32); }
}
fn qvk_get_string(name: u32) -> *const u8 {
    // SAFETY: Delegates to OpenGL; returns a pointer to a static string owned by the GL driver.
    unsafe { crate::vk_bindings::GetString(name) }
}

/// Helper to convert a *const u8 (null-terminated C string) to &str
unsafe fn cptr_to_str<'a>(p: *const u8) -> &'a str {
    if p.is_null() { return ""; }
    std::ffi::CStr::from_ptr(p as *const i8).to_str().unwrap_or("")
}
fn qvk_get_error() -> u32 {
    // SAFETY: Delegates to OpenGL.
    unsafe { crate::vk_bindings::GetError() }
}
fn qvk_draw_buffer(mode: u32) {
    // SAFETY: Delegates to OpenGL.
    unsafe { crate::vk_bindings::DrawBuffer(mode); }
}
// qvk_color3fv, qvk_color4fv, qvk_color4ubv removed — legacy fixed-function
// color calls are not used; the modern shader pipeline sets colors via uniforms.
// qvk_fogi, qvk_fogfv, qvk_fogf removed — fixed-function fog is now
// handled entirely by GLSL shaders via u_FogDensity / u_FogColor uniforms.
fn qvk_hint(target: u32, mode: u32) {
    // SAFETY: Delegates to OpenGL.
    unsafe { crate::vk_bindings::Hint(target, mode); }
}
fn qvk_clip_plane(plane: u32, equation: &[f64; 4]) {
    // SAFETY: Delegates to OpenGL; equation points to 4 doubles.
    unsafe { crate::vk_bindings::ClipPlane(plane, equation.as_ptr()); }
}
fn qvk_stencil_func(func: u32, ref_val: i32, mask: u32) {
    // SAFETY: Delegates to OpenGL.
    unsafe { crate::vk_bindings::StencilFunc(func, ref_val, mask); }
}
fn qvk_stencil_op(fail: u32, zfail: u32, zpass: u32) {
    // SAFETY: Delegates to OpenGL.
    unsafe { crate::vk_bindings::StencilOp(fail, zfail, zpass); }
}

#[derive(Debug, Clone, Default)]
struct MleafLocal {
    pub cluster: i32,
    pub contents: i32,
}

// Math helpers are imported from myq2_common::q_shared::* (vector_normalize, perpendicular_vector, rotate_point_around_vector, box_on_plane_side, etc.)

// ============================================================
// R_CullBox
// Returns true if the box is completely outside the frustum
// ============================================================
pub fn r_cull_box(mins: &Vec3, maxs: &Vec3) -> bool {
    if rcvars().r_nocull.value != 0.0 {
        return false;
    }
    let rg = rg();
    for i in 0..4 {
        if box_on_plane_side(mins, maxs, &rg.frustum[i]) == 2 {
            return true;
        }
    }
    false
}

// ============================================================
// r_rotate_for_entity - REMOVED (never called, vestigial GL matrix code)
// ============================================================

// ============================================================
// R_DrawSpriteModel
// ============================================================
// r_draw_sprite_model — removed (legacy immediate-mode GL; modern renderer handles sprites)

// r_draw_null_model — removed (legacy immediate-mode GL)

// r_draw_entities_on_list — removed (legacy immediate-mode GL; modern renderer handles entities)

// r_draw_particles — removed (legacy immediate-mode GL; modern renderer handles particles)

// r_poly_blend — removed (legacy immediate-mode GL; modern PostProcessor handles polyblend)

// ============================================================
// SignbitsForPlane
// ============================================================
pub fn signbits_for_plane(out: &CPlane) -> u8 {
    let mut bits = 0u8;
    for j in 0..3 {
        if out.normal[j] < 0.0 {
            bits |= 1 << j;
        }
    }
    bits
}

// ============================================================
// R_SetFrustum
// ============================================================
pub fn r_set_frustum() {
    let mut rg = rg();
    // Extract fov values to avoid holding an immutable borrow on rg.r_newrefdef
    // while mutating rg.frustum below.
    let (fov_x, fov_y) = match rg.r_newrefdef.as_ref() {
        Some(r) => (r.fov_x, r.fov_y),
        None => return,
    };

    // Main-thread-only access to vk_local statics (vup, vpn, vright, r_origin).
    {
        // rotate VPN right by FOV_X/2 degrees
        rotate_point_around_vector(
            &mut rg.frustum[0].normal, &crate::vk_local::rfs().vup, &crate::vk_local::rfs().vpn,
            -(90.0 - fov_x / 2.0),
        );
        // rotate VPN left by FOV_X/2 degrees
        rotate_point_around_vector(
            &mut rg.frustum[1].normal, &crate::vk_local::rfs().vup, &crate::vk_local::rfs().vpn,
            90.0 - fov_x / 2.0,
        );
        // rotate VPN up by FOV_Y/2 degrees
        rotate_point_around_vector(
            &mut rg.frustum[2].normal, &crate::vk_local::rfs().vright, &crate::vk_local::rfs().vpn,
            90.0 - fov_y / 2.0,
        );
        // rotate VPN down by FOV_Y/2 degrees
        rotate_point_around_vector(
            &mut rg.frustum[3].normal, &crate::vk_local::rfs().vright, &crate::vk_local::rfs().vpn,
            -(90.0 - fov_y / 2.0),
        );

        for i in 0..4 {
            rg.frustum[i].plane_type = PLANE_ANYZ;
            rg.frustum[i].dist = dot_product(&crate::vk_local::rfs().r_origin, &rg.frustum[i].normal);
            rg.frustum[i].signbits = signbits_for_plane(&rg.frustum[i]);
        }
    }
}

const PLANE_ANYZ: u8 = 5;

// ============================================================
// R_SetupFrame
// ============================================================
pub fn r_setup_frame() {
    let mut rg = rg();
    // SAFETY: vk_local statics accessed from main thread only
    unsafe {
        crate::vk_local::rfs().r_framecount += 1;

        let refdef = match rg.r_newrefdef.as_ref() {
            Some(r) => r.clone(),
            None => return,
        };

        // build the transformation matrix for the given view angles
        crate::vk_local::rfs().r_origin = refdef.vieworg;
        angle_vectors(&refdef.viewangles, Some(&mut crate::vk_local::rfs().vpn), Some(&mut crate::vk_local::rfs().vright), Some(&mut crate::vk_local::rfs().vup));

        // current viewcluster
        if refdef.rdflags & RDF_NOWORLDMODEL == 0 {
            crate::vk_local::rfs().r_oldviewcluster = crate::vk_local::rfs().r_viewcluster;
            crate::vk_local::rfs().r_oldviewcluster2 = crate::vk_local::rfs().r_viewcluster2;

            let wm = crate::vk_local::rfs().r_worldmodel;
            if !wm.is_null() {
                let leaf = crate::vk_model::mod_point_in_leaf(&crate::vk_local::rfs().r_origin, wm);
                crate::vk_local::rfs().r_viewcluster = (*leaf).cluster;
                crate::vk_local::rfs().r_viewcluster2 = (*leaf).cluster;

                // check above and below so crossing solid water doesn't draw wrong
                if (*leaf).contents == 0 {
                    // look down a bit
                    let mut temp = crate::vk_local::rfs().r_origin;
                    temp[2] -= 16.0;
                    let leaf2 = crate::vk_model::mod_point_in_leaf(&temp, wm);
                    if (*leaf2).contents & CONTENTS_SOLID == 0
                        && (*leaf2).cluster != crate::vk_local::rfs().r_viewcluster2
                    {
                        crate::vk_local::rfs().r_viewcluster2 = (*leaf2).cluster;
                    }
                } else {
                    // look up a bit
                    let mut temp = crate::vk_local::rfs().r_origin;
                    temp[2] += 16.0;
                    let leaf2 = crate::vk_model::mod_point_in_leaf(&temp, wm);
                    if (*leaf2).contents & CONTENTS_SOLID == 0
                        && (*leaf2).cluster != crate::vk_local::rfs().r_viewcluster2
                    {
                        crate::vk_local::rfs().r_viewcluster2 = (*leaf2).cluster;
                    }
                }
            }
        }

        for i in 0..4 {
            rg.v_blend[i] = refdef.blend[i];
        }

        crate::vk_local::rfs().c_brush_polys = 0;
        crate::vk_local::rfs().c_alias_polys = 0;

        // clear out the portion of the screen that the NOWORLDMODEL defines
        if refdef.rdflags & RDF_NOWORLDMODEL != 0 {
            qvk_enable(VK_SCISSOR_TEST);
            qvk_clear_color(0.3, 0.3, 0.3, 1.0);
            qvk_scissor(
                refdef.x,
                rg.vid.height as i32 - refdef.height - refdef.y,
                refdef.width,
                refdef.height,
            );
            qvk_clear(VK_COLOR_BUFFER_BIT | VK_DEPTH_BUFFER_BIT);
            qvk_clear_color(1.0, 0.0, 0.5, 0.5);
            qvk_disable(VK_SCISSOR_TEST);
        }
    }
}

// ============================================================
// my_glu_perspective - REMOVED (vestigial GL frustum setup, modern renderer uses FrameParams)
// ============================================================

// ============================================================
// r_setup_gl - REMOVED (legacy GL matrix/state setup)
// ============================================================
// Modern Vulkan renderer computes all matrices directly from FrameParams
// passed to ModernRenderPath::begin_frame(). GL viewport/matrix/state calls
// were vestigial and had no effect on rendering.
pub fn r_setup_gl() {
    // No-op: Modern renderer handles all view setup via FrameParams
}

// ============================================================
// R_Clear
// ============================================================
/// Frame counter for depth buffer trick.
static TRICKFRAME: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

pub fn r_clear() {
    let mut rg = rg();
    if rcvars().vk_ztrick.value != 0.0 {
        if rcvars().vk_clear.value != 0.0 {
            qvk_clear(VK_COLOR_BUFFER_BIT);
        }

        let trickframe = TRICKFRAME.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        if trickframe & 1 != 0 {
            rg.gldepthmin = 0.0;
            rg.gldepthmax = 0.49999;
            qvk_depth_func(VK_LEQUAL);
        } else {
            rg.gldepthmin = 1.0;
            rg.gldepthmax = 0.5;
            qvk_depth_func(VK_GEQUAL);
        }
    } else {
        if rcvars().vk_clear.value != 0.0 {
            qvk_clear(VK_COLOR_BUFFER_BIT | VK_DEPTH_BUFFER_BIT);
        } else {
            qvk_clear(VK_DEPTH_BUFFER_BIT);
        }
        rg.gldepthmin = 0.0;
        rg.gldepthmax = 1.0;
        qvk_depth_func(VK_LEQUAL);
    }

    qvk_depth_range(rg.gldepthmin as f64, rg.gldepthmax as f64);

    // Stencil shadows - MrG
    if rcvars().vk_shadows.value != 0.0 {
        qvk_clear_stencil(1);
        qvk_clear(VK_STENCIL_BUFFER_BIT);
    }
}

// ============================================================
// R_Flash
// ============================================================
pub fn r_flash() {
    // Legacy r_poly_blend removed; modern PostProcessor handles polyblend via rg().v_blend.
}

// ============================================================
// R_SetupFog — mattx86: engine_fog
// ============================================================
pub fn r_setup_fog() {
    let mut rg = rg();
    // timebasedfx arrays
    let ampmarray: [[f32; 13]; 2] = [
        // PM
        [0.00000, 0.00000, 0.00000, 0.00000, 0.00000, 0.00000, 0.00000,
         0.00000, 0.00000, 0.00000, 0.00020, 0.00040, 0.00000],
        // AM
        [0.00000, 0.00050, 0.00040, 0.00030, 0.00020, 0.00010, 0.00005,
         0.00000, 0.00000, 0.00000, 0.00000, 0.00000, 0.00060],
    ];

    let refdef = match rg.r_newrefdef.as_ref() {
        Some(r) => r,
        None => return,
    };

    let point_contents = cm_point_contents(&refdef.vieworg, 0);
    if point_contents & CONTENTS_WATER != 0 {
        rg.fog_type = 0;
    } else if point_contents & CONTENTS_SLIME != 0 {
        rg.fog_type = 1;
    } else if point_contents & CONTENTS_LAVA != 0 {
        rg.fog_type = 2;
    } else {
        rg.fog_type = 3;
    }

    if rcvars().r_fog.value != 0.0 || (rcvars().r_fog.value == 0.0 && (rg.fog_type == 1 || rg.fog_type == 2)) {
        if rcvars().r_timebasedfx.value != 0.0 && (rg.fog_type == 0 || rg.fog_type == 3) {
            // time-based fog
            use std::time::SystemTime;
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default();
            let secs = now.as_secs();
            // approximate hour extraction
            let hour_24 = ((secs % 86400) / 3600) as i32;

            let (am, hour_12) = if hour_24 <= 11 {
                let h = if hour_24 == 0 { 12 } else { hour_24 };
                (1usize, h as usize)
            } else {
                let h = if hour_24 > 12 { hour_24 - 12 } else { hour_24 };
                (0usize, h as usize)
            };
            rg.fog_density = ampmarray[am][hour_12];
        } else if rg.fog_type == 1 || rg.fog_type == 2 {
            rg.fog_density = 0.1200;
        } else {
            rg.fog_density = 0.0675;
        }

        // Fixed-function GL_FOG calls removed — fog is now handled
        // entirely by GLSL shaders via u_FogDensity / u_FogColor uniforms.
        // rg.fog_density is still computed above so the shader can read it.
    } else {
        rg.fog_density = 0.0;
    }
}

// ============================================================
// R_RenderView — r_newrefdef must be set before the first call
// ============================================================
pub fn r_render_view(fd: &RefdefLocal) {
    if rcvars().r_norefresh.value != 0.0 {
        return;
    }

    // Set r_newrefdef and check r_worldmodel, then drop the lock before
    // calling sub-functions that also need rg().
    {
        let mut rg = rg();
        rg.r_newrefdef = Some(fd.clone());
    }

    // Check worldmodel via rfs() (the actual loaded BSP pointer from r_begin_registration)
    // Main-thread-only access via rfs().
    {
        if crate::vk_local::rfs().r_worldmodel.is_null()
            && (fd.rdflags & RDF_NOWORLDMODEL == 0)
        {
            vid_printf(ERR_DROP, "R_RenderView: NULL worldmodel");
            return;
        }
    }

    // SAFETY: vk_local statics accessed from main thread only
    unsafe {
        if rcvars().r_speeds.value != 0.0 {
            crate::vk_local::rfs().c_brush_polys = 0;
            crate::vk_local::rfs().c_alias_polys = 0;
        }

        r_push_dlights();

        if rcvars().vk_finish.value != 0.0 {
            qvk_finish();
        }

        // Legacy state computation (frustum, leaves, fog)
        // Each of these functions locks rg() internally.
        r_setup_frame();
        r_set_frustum();
        // r_setup_gl() removed - modern renderer handles view setup via FrameParams
        r_mark_leaves();
        r_setup_fog();

        // Modern renderer: begin 3D pass with view parameters
        let params = FrameParams {
            time: crate::vk_local::rfs().r_newrefdef.time,
            vieworg: fd.vieworg,
            viewangles: fd.viewangles,
            fov_x: fd.fov_x,
            fov_y: fd.fov_y,
            width: fd.width as u32,
            height: fd.height as u32,
            blend: fd.blend,
            rdflags: fd.rdflags,
        };

        let modern = rg().modern.as_mut().unwrap() as *mut ModernRenderPath;
        // SAFETY: modern pointer is valid for the duration of the frame;
        // the renderer globals lock is not held while calling modern methods.
        let modern = &mut *modern;
        modern.begin_frame(&params);

        // Transfer dlight list so render_dlights() (called from end_frame) can access it.
        // RefdefLocal.dlights was populated in renderer_bridge from the client's RefDef.
        modern.set_frame_dlights(fd.dlights.clone());

        // Populate sorted D3 point-light list for this frame.
        {
            // The lit pass serves two modes: the (abandoned, default-off) D3 flat-base mode
            // (r_d3_maxlights) and the dark-fill surface emission (r_surf_emit, keeps the
            // baked base). Run it for whichever wants more lights.
            let d3_lights   = rcvars().r_d3_maxlights.fresh_value().max(0.0) as usize;
            let surf_lights = rcvars().r_surf_emit.fresh_value().max(0.0) as usize;
            let max_lights = d3_lights.max(surf_lights);
            let ambient    = rcvars().r_d3_ambient.fresh_value().max(0.0);
            let extrude    = rcvars().r_d3_extrude.fresh_value().max(0.1);
            let spec_power = rcvars().r_d3_spec_power.fresh_value().max(1.0);
            modern.update_d3_lights(max_lights, ambient, extrude, spec_power, fd.vieworg);
        }

        // World geometry
        if rcvars().r_drawworld.value != 0.0 {
            // BSP traversal: detect sky surfaces and chain alpha/texture surfaces.
            // r_clear_sky_box() resets accumulated sky bounds from the previous frame.
            // r_recursive_world_node() walks the BSP tree, calling r_add_sky_surface()
            // for each sky surface encountered and chaining other surfaces.
            crate::vk_warp::r_clear_sky_box();
            let root = crate::vk_local::r_worldmodel_nodes();
            if !root.is_null() {
                let mut alpha_surfaces: *mut crate::vk_model_types::MSurface = std::ptr::null_mut();
                crate::vk_rsurf::r_recursive_world_node(root, &mut alpha_surfaces);
            }
            modern.draw_world();
        }

        // Entities
        if rcvars().r_drawentities.value != 0.0 {
            for entity in &fd.entities {
                if entity.model.is_null() {
                    // Beam entities (RF_BEAM flag) have null models and are
                    // rendered as lines between origin and oldorigin. Skip for now
                    // as they require specialized line/billboard rendering.
                    continue;
                }
                // SAFETY: entity.model is *mut RefModel (opaque). Cast to
                // the concrete renderer Model type for field access.
                let model = &*(entity.model as *mut crate::vk_model_types::Model);
                match model.r#type {
                    crate::vk_model_types::ModType::Alias => {
                        modern.draw_alias_model(entity);
                    }
                    crate::vk_model_types::ModType::Brush => {
                        modern.draw_brush_model(entity);
                    }
                    crate::vk_model_types::ModType::Sprite => {
                        modern.draw_sprite_model(entity);
                    }
                    _ => {}
                }
            }
        }

        // Particles
        let particle_data: Vec<ParticleData> = fd.particles.iter().map(|p| {
            ParticleData {
                origin: p.origin,
                color: p.color,
                alpha: p.alpha,
                particle_type: p.particle_type,
            }
        }).collect();
        modern.draw_particles(&particle_data);

        modern.draw_alpha_surfaces();
        modern.draw_sky();

        r_flash();

        if rcvars().r_speeds.value != 0.0 {
            vid_printf(PRINT_ALL, &format!(
                "{:4} wpoly {:4} epoly {} tex {} lmaps\n",
                crate::vk_local::rfs().c_brush_polys, crate::vk_local::rfs().c_alias_polys,
                0, 0, // visible textures/lightmaps counters (not yet tracked)
            ));
        }
    }
}

// ============================================================
// R_SetGL2D
// ============================================================
pub fn r_set_gl2d() {
    let mut rg = rg();
    let vid_w = rg.vid.width;
    let vid_h = rg.vid.height;

    qvk_viewport(0, 0, vid_w as i32, vid_h as i32);
    // GL matrix setup removed - modern renderer handles 2D projection internally
    qvk_disable(VK_DEPTH_TEST);
    qvk_disable(VK_CULL_FACE);
    qvk_disable(VK_BLEND);
    qvk_color4f(1.0, 1.0, 1.0, 1.0);

    if let Some(ref mut state) = rg.vk_state {
        state.transconsole = 1; // mattx86: trans_console
    }
}

// GL_DrawColoredStereoLinePair / GL_DrawStereoPattern — removed (legacy immediate-mode GL)

// ============================================================
// R_SetLightLevel
// ============================================================
pub fn r_set_light_level() {
    // Clone the refdef data we need, then drop the lock before calling r_light_point
    let refdef = match rg().r_newrefdef.clone() {
        Some(r) => r,
        None => return,
    };

    if refdef.rdflags & RDF_NOWORLDMODEL != 0 {
        return;
    }

    // save off light value for server to look at (BIG HACK!)
    let mut shadelight: Vec3 = [0.0; 3];
    // SAFETY: r_light_point accesses vk_local statics (main thread only)
    unsafe { r_light_point(&refdef.vieworg, &mut shadelight); }

    // pick the greatest component, write via cvar system
    let light_val = if shadelight[0] > shadelight[1] {
        if shadelight[0] > shadelight[2] {
            150.0 * shadelight[0]
        } else {
            150.0 * shadelight[2]
        }
    } else if shadelight[1] > shadelight[2] {
        150.0 * shadelight[1]
    } else {
        150.0 * shadelight[2]
    };
    myq2_common::cvar::cvar_set_value("r_lightlevel", light_val);
}

// ============================================================
// R_RenderFrame
// ============================================================
pub fn r_render_frame(fd: &RefdefLocal) {
    r_render_view(fd);
    r_set_light_level();
    // Note: Do NOT call end_frame() here. In the original OpenGL code, all drawing
    // (3D world + 2D HUD/console) goes to the same backbuffer. Our Vulkan end_frame()
    // does acquire→submit→present, so calling it here would present the 3D frame
    // immediately, then vk_imp_end_frame would present a second 2D-only frame on black.
    // Instead, let the single end_frame() in vk_imp_end_frame handle everything:
    // flush 3D scene + composite + flush 2D + submit + present.
    r_set_gl2d();
}

/// Public function to present the current Vulkan frame.
/// This flushes all pending 2D drawing, submits commands, and presents to the swapchain.
pub fn r_present_frame() {
    // Get raw pointer and drop the lock before calling end_frame().
    // end_frame() -> flush_3d_scene() / composite_scene_to_swapchain() call rg()
    // internally, so holding the lock here would deadlock.
    // SAFETY: modern pointer is valid for the frame duration; main thread only.
    let modern = rg().modern.as_mut().unwrap() as *mut ModernRenderPath;
    unsafe { (*modern).end_frame(); }
}

// ============================================================
// R_Register
// ============================================================
pub fn r_register() {
    myq2_common::common::com_printf("r_register: ENTERED\n");
    RENDERER_CVARS.get_or_init(|| {
        myq2_common::common::com_printf("r_register: Inside get_or_init closure, before first cvar_get\n");
        // flushmap — read via cvar_variable_value in vk_model.rs, no static needed.
        cvar_get("flushmap", "0", CVAR_ZERO);

        myq2_common::common::com_printf("r_register: Starting cvar registration\n");
        let cvars = RendererCvars {
            r_norefresh: cvar_get("r_norefresh", "0", CVAR_ZERO),
            r_fullbright: cvar_get("r_fullbright", "0", CVAR_ZERO),
            r_drawentities: cvar_get("r_drawentities", "1", CVAR_ZERO),
            r_drawworld: cvar_get("r_drawworld", "1", CVAR_ZERO),
            r_novis: cvar_get("r_novis", "0", CVAR_ZERO),
            r_nocull: cvar_get("r_nocull", "0", CVAR_ZERO),
            r_speeds: cvar_get("r_speeds", "0", CVAR_ZERO),
            r_lightlevel: cvar_get("r_lightlevel", "0", CVAR_ZERO),
            r_overbrightbits: cvar_get("r_overbrightbits", "2", CVAR_ARCHIVE),
            vk_modulate: cvar_get("vk_modulate", "2.0", CVAR_ARCHIVE),
            vk_log: cvar_get("vk_log", "0", CVAR_ZERO),
            vk_mode: cvar_get("vk_mode", "4", CVAR_ARCHIVE),
            vk_lightmap: cvar_get("vk_lightmap", "0", CVAR_ZERO),
            vk_shadows: cvar_get("vk_shadows", "1", CVAR_ARCHIVE),
            vk_dynamic: cvar_get("vk_dynamic", "1", CVAR_ARCHIVE),
            vk_picmip: cvar_get("vk_picmip", "0", CVAR_ARCHIVE),
            vk_skymip: cvar_get("vk_skymip", "0", CVAR_ARCHIVE),
            vk_showtris: cvar_get("vk_showtris", "0", CVAR_ZERO),
            vk_ztrick: cvar_get("vk_ztrick", "0", CVAR_ARCHIVE),
            vk_finish: cvar_get("vk_finish", "0", CVAR_ARCHIVE),
            vk_clear: cvar_get("vk_clear", "0", CVAR_ZERO),
            vk_cull: cvar_get("vk_cull", "1", CVAR_ARCHIVE),
            vk_polyblend: cvar_get("vk_polyblend", "1", CVAR_ARCHIVE),
            vk_flashblend: cvar_get("vk_flashblend", "1", CVAR_ARCHIVE),
            r_light_bloom: cvar_get("r_light_bloom", "1", CVAR_ARCHIVE),
            vk_monolightmap: cvar_get("vk_monolightmap","0", CVAR_ZERO),
            vk_driver: cvar_get("vk_driver","opengl32", CVAR_ARCHIVE),
            vk_texturemode: cvar_get("vk_texturemode","VK_LINEAR_MIPMAP_LINEAR", CVAR_ARCHIVE),
            vk_texturealphamode: cvar_get("vk_texturealphamode","default", CVAR_ZERO),
            vk_texturesolidmode: cvar_get("vk_texturesolidmode","default", CVAR_ZERO),
            vk_lockpvs: cvar_get("vk_lockpvs","0", CVAR_ZERO),
            vk_ext_multitexture: cvar_get("vk_ext_multitexture","1", CVAR_ARCHIVE),
            vk_drawbuffer: cvar_get("vk_drawbuffer","VK_BACK", CVAR_ARCHIVE),
            vk_swapinterval: cvar_get("vk_swapinterval","1", CVAR_ARCHIVE),
            vk_saturatelighting: cvar_get("vk_saturatelighting","0", CVAR_ARCHIVE),
            vk_3dlabs_broken: cvar_get("vk_3dlabs_broken","0", CVAR_ARCHIVE),
            vk_ext_texture_filter_anisotropic: cvar_get("vk_ext_texture_filter_anisotropic","1", CVAR_ARCHIVE),
            vk_sgis_generate_mipmap: cvar_get("vk_sgis_generate_mipmap","0", CVAR_ARCHIVE),
            r_celshading: cvar_get("r_celshading","0", CVAR_ARCHIVE),
            r_fog: cvar_get("r_fog","0", CVAR_ARCHIVE),
            r_timebasedfx: cvar_get("r_timebasedfx","1", CVAR_ARCHIVE),
            r_detailtexture: cvar_get("r_detailtexture","7", CVAR_ARCHIVE),
            r_caustics: cvar_get("r_caustics","1", CVAR_ARCHIVE),
            r_hwgamma: cvar_get("r_hwgamma","0", CVAR_ARCHIVE),
            r_stainmap: cvar_get("r_stainmap","1", CVAR_ARCHIVE),
            r_verbose: cvar_get("r_verbose","0", CVAR_ZERO),
            r_fxaa: cvar_get("r_fxaa","1", CVAR_ARCHIVE),
            r_ssao: cvar_get("r_ssao","1", CVAR_ARCHIVE),
            r_ssao_radius: cvar_get("r_ssao_radius","0.5", CVAR_ARCHIVE),
            r_ssao_intensity: cvar_get("r_ssao_intensity","0.80", CVAR_ARCHIVE),
            r_bloom: cvar_get("r_bloom","1", CVAR_ARCHIVE),
            r_bloom_threshold: cvar_get("r_bloom_threshold","0.8", CVAR_ARCHIVE),
            r_bloom_intensity: cvar_get("r_bloom_intensity","0.3", CVAR_ARCHIVE),
            r_fsr: cvar_get("r_fsr","1", CVAR_ARCHIVE),
            r_fsr_scale: cvar_get("r_fsr_scale","0.75", CVAR_ARCHIVE),
            r_fsr_sharpness: cvar_get("r_fsr_sharpness","0.2", CVAR_ARCHIVE),
            r_msaa: cvar_get("r_msaa","0", CVAR_ARCHIVE),
            r_anisotropy: cvar_get("r_anisotropy","8", CVAR_ARCHIVE),
            vk_screenshot_format: cvar_get("vk_screenshot_format","tga", CVAR_ARCHIVE),
            vk_screenshot_quality: cvar_get("vk_screenshot_quality","85", CVAR_ARCHIVE),
            vid_fullscreen: cvar_get("vid_fullscreen","0", CVAR_ARCHIVE),
            vid_gamma: cvar_get("vid_gamma","0.90", CVAR_ARCHIVE),
            vid_ref: cvar_get("vid_ref","gl", CVAR_ARCHIVE),
            r_saturation: cvar_get("r_saturation","1.0", CVAR_ARCHIVE),
            r_contrast: cvar_get("r_contrast","1.0", CVAR_ARCHIVE),
            r_brightness: cvar_get("r_brightness","1.0", CVAR_ARCHIVE),
            r_shadowlift: cvar_get("r_shadowlift","0.0", CVAR_ARCHIVE),
            r_color_grade: cvar_get("r_color_grade","1", CVAR_ARCHIVE),
            r_color_grade_intensity: cvar_get("r_color_grade_intensity","0.3", CVAR_ARCHIVE),
            r_lightmap_gamma: cvar_get("r_lightmap_gamma","1.0", CVAR_ARCHIVE),
            r_lightmap_contrast: cvar_get("r_lightmap_contrast","1.0", CVAR_ARCHIVE),
            r_d3_ambient: cvar_get("r_d3_ambient","0.15", CVAR_ARCHIVE),
            r_d3_maxlights: cvar_get("r_d3_maxlights","0", CVAR_ARCHIVE),
            r_d3_extrude: cvar_get("r_d3_extrude","1.0", CVAR_ARCHIVE),
            r_d3_spec_power: cvar_get("r_d3_spec_power","32.0", CVAR_ARCHIVE),
            r_surf_emit: cvar_get("r_surf_emit","0", CVAR_ARCHIVE),
            r_vxgi: cvar_get("r_vxgi","1", CVAR_ARCHIVE),
            r_vxgi_res: cvar_get("r_vxgi_res","128", CVAR_ARCHIVE),
            r_vxgi_debug: cvar_get("r_vxgi_debug","0", CVAR_ARCHIVE),
            r_vxgi_gi: cvar_get("r_vxgi_gi","0", CVAR_ARCHIVE),
            r_vxgi_strength: cvar_get("r_vxgi_strength","2", CVAR_ARCHIVE),
            r_vxgi_bake: cvar_get("r_vxgi_bake","0.7", CVAR_ARCHIVE),
            r_vxgi_bounces: cvar_get("r_vxgi_bounces","5", CVAR_ARCHIVE),
            r_water_reflect: cvar_get("r_water_reflect","1", CVAR_ARCHIVE),
            r_water_reflect_strength: cvar_get("r_water_reflect_strength","0.4", CVAR_ARCHIVE),
            r_water_shimmer: cvar_get("r_water_shimmer","1", CVAR_ARCHIVE),
        };

        myq2_common::common::com_printf("r_register: Cvars registered, calling with_device\n");
        // Initialize Vulkan render configuration with MSAA and anisotropy settings
        crate::modern::gpu_device::with_device(|ctx| {
            crate::vulkan::init_render_config(ctx, cvars.r_msaa.value as i32, cvars.r_anisotropy.value as i32);
        });
        myq2_common::common::com_printf("r_register: with_device completed\n");

        cvars
    });

    // Command registration (runs every call, but cmd_add_command is idempotent)
    cmd_add_command("imagelist", vk_image_list_f);
    cmd_add_command("screenshot", crate::vk_rmisc::vk_screen_shot_f);
    cmd_add_command("modellist", mod_modellist_f);
    cmd_add_command("vk_strings", crate::vk_rmisc::vk_strings_f);
}

// ============================================================
// VID_GetModeInfo — convert mode number to resolution
// ============================================================
fn vid_get_mode_info(mode: i32) -> Option<(i32, i32)> {
    const MODES: &[(i32, i32)] = &[
        (320, 240), (400, 300), (512, 384), (640, 480),
        (800, 600), (960, 720), (1024, 768), (1152, 864),
        (1280, 960), (1600, 1200), (2048, 1536),
    ];
    MODES.get(mode as usize).copied()
}

// ============================================================
// R_SetMode
// ============================================================
pub fn r_set_mode() -> bool {
    let cv = rcvars();
    let mut rg = rg();

    // Ensure vk_config and vk_state are initialized
    if rg.vk_config.is_none() { rg.vk_config = Some(VkConfig::default()); }
    if rg.vk_state.is_none() { rg.vk_state = Some(VkState::default()); }

    // FIXME: Disabled during initialization to avoid CVAR_CTX deadlock
    // This check requires accessing cvars which would cause nested lock
    // TODO: Refactor to pass CvarContext through or defer until after init
    /*
    if cv.vid_fullscreen.is_modified() && rg.vk_config.as_ref().unwrap().allow_cds == 0 {
        myq2_common::common::com_printf("R_SetMode() - CDS not allowed with this driver\n");
        cvar_set_value("vid_fullscreen", if cv.vid_fullscreen.fresh_value() != 0.0 { 0.0 } else { 1.0 });
        cv.vid_fullscreen.clear_modified();
    }
    */

    // Use the cached value from CvarRef instead of fresh_value() to avoid lock
    let fullscreen = cv.vid_fullscreen.value != 0.0;
    // Skip clear_modified() during init to avoid CVAR_CTX lock
    // cv.vid_fullscreen.clear_modified();
    // cv.vk_mode.clear_modified();

    myq2_common::common::com_printf("R_SetMode: Getting mode value\n");
    // Use cached value instead of fresh_value() to avoid CVAR_CTX lock during init
    let mode_val = cv.vk_mode.value;
    myq2_common::common::com_printf(&format!("R_SetMode: mode_val = {}\n", mode_val));

    // Convert mode to resolution (use mode table instead of uninitialized vid.width/height)
    let mode_int = mode_val as i32;
    let (mut w, mut h) = vid_get_mode_info(mode_int).unwrap_or((800, 600));
    myq2_common::common::com_printf(&format!("R_SetMode: Calling glimp_set_mode(w={}, h={}, mode={}, fullscreen={})\n", w, h, mode_val, fullscreen));
    let err = glimp_set_mode(&mut w, &mut h, mode_val, fullscreen);
    myq2_common::common::com_printf(&format!("R_SetMode: glimp_set_mode returned {}\n", err));
    rg.vid.width = w;
    rg.vid.height = h;

    if err == RSERR_OK {
        rg.vk_state.as_mut().unwrap().prev_mode = mode_val as i32;
    } else {
        if err == RSERR_INVALID_FULLSCREEN {
            cvar_set_value("vid_fullscreen", 0.0);
            cv.vid_fullscreen.clear_modified();
            vid_printf(PRINT_ALL, "ref_gl::R_SetMode() - fullscreen unavailable in this mode\n");
            let (mut w, mut h) = vid_get_mode_info(mode_int).unwrap_or((800, 600));
            let ok = glimp_set_mode(&mut w, &mut h, mode_val, false) == RSERR_OK;
            rg.vid.width = w;
            rg.vid.height = h;
            if ok {
                return true;
            }
        } else if err == RSERR_INVALID_MODE {
            cvar_set_value("vk_mode", rg.vk_state.as_ref().unwrap().prev_mode as f32);
            cv.vk_mode.clear_modified();
            vid_printf(PRINT_ALL, "ref_gl::R_SetMode() - invalid mode\n");
        }

        // try setting it back to something safe
        let prev_mode = rg.vk_state.as_ref().unwrap().prev_mode;
        let (mut w, mut h) = vid_get_mode_info(prev_mode).unwrap_or((800, 600));
        let ok = glimp_set_mode(&mut w, &mut h, prev_mode as f32, false) != RSERR_OK;
        rg.vid.width = w;
        rg.vid.height = h;
        if ok {
            vid_printf(PRINT_ALL, "ref_gl::R_SetMode() - could not revert to safe mode\n");
            return false;
        }
    }
    true
}

// ============================================================
// PowerofTwo
// ============================================================
pub fn power_of_two(var: &mut i32) {
    let powers: [i32; 13] = [2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192];
    for i in 0..13 {
        if powers[i] == *var {
            break;
        } else if i + 1 < 13 && powers[i + 1] > *var {
            *var = powers[i];
            break;
        }
    }
}

// ============================================================
// R_Init
// ============================================================
pub fn r_init(hinstance: usize, hwnd: usize) -> i32 {
    myq2_common::common::com_printf("R_Init: Starting renderer initialization\n");
    myq2_common::common::com_printf(&format!("ref_gl version: {}\n", REF_VERSION));

    // reversed for saturation control
    myq2_common::common::com_printf("R_Init: About to call r_register\n");
    r_register();
    myq2_common::common::com_printf("R_Init: r_register completed\n");

    // SAFETY: draw_get_palette accesses vk_image statics (main thread only)
    myq2_common::common::com_printf("R_Init: About to call draw_get_palette\n");
    unsafe { draw_get_palette(); }
    myq2_common::common::com_printf("R_Init: draw_get_palette completed\n");

    // initialize OS-specific parts of Vulkan/windowing
    // NOTE: glimp_init is called directly from vid_init to avoid nested PLATFORM_STATE lock
    myq2_common::common::com_printf("R_Init: Skipping glimp_init (handled by vid_init)\n");

    // set our "safe" modes
    myq2_common::common::com_printf("R_Init: Setting safe modes\n");
    {
        let mut rg = rg();
        let vk_state = rg.vk_state.get_or_insert_with(VkState::default);
        vk_state.prev_mode = 3;
    }
    myq2_common::common::com_printf("R_Init: Safe modes set\n");

    // create the window and set up the context (r_set_mode locks rg internally)
    myq2_common::common::com_printf("R_Init: Calling r_set_mode\n");
    if !r_set_mode() {
        myq2_common::common::com_printf("ref_gl::R_Init() - could not R_SetMode()\n");
        return -1;
    }
    myq2_common::common::com_printf("R_Init: r_set_mode succeeded\n");

    // If Vulkan GPU device is active, initialize modern renderer
    if crate::modern::gpu_device::is_initialized() {
        myq2_common::common::com_printf("R_Init: Vulkan device active\n");
        myq2_common::common::com_printf("R_Init: Calling vid_menu_init\n");
        vid_menu_init();
        myq2_common::common::com_printf("R_Init: vid_menu_init completed\n");
        // draw_get_palette already called above

        // Initialize image and model systems
        myq2_common::common::com_printf("R_Init: Calling vk_init_images\n");
        unsafe {
            vk_init_images();
            myq2_common::common::com_printf("R_Init: vk_init_images completed\n");
            myq2_common::common::com_printf("R_Init: Calling mod_init\n");
            mod_init();
            myq2_common::common::com_printf("R_Init: mod_init completed\n");
        }

        // Create and initialize ModernRenderPath
        myq2_common::common::com_printf("R_Init: Creating ModernRenderPath\n");
        let mut modern = ModernRenderPath::new();
        {
            let rg = rg();
            modern.set_dimensions(rg.vid.width as u32, rg.vid.height as u32);
        }
        myq2_common::common::com_printf("R_Init: Initializing ModernRenderPath\n");
        myq2_common::common::com_printf("R_Init: About to call modern.init()\n");
        let init_result = modern.init();
        myq2_common::common::com_printf("R_Init: modern.init() returned\n");
        match init_result {
            Ok(()) => myq2_common::common::com_printf("Modern renderer initialized\n"),
            Err(e) => myq2_common::common::com_printf(&format!("Modern renderer init failed: {}\n", e)),
        }
        myq2_common::common::com_printf("R_Init: ModernRenderPath initialization complete\n");

        // Load textures (TextureStore intercepts vk_upload32 calls)
        // NOTE: All texture loading must happen AFTER modern.init() which creates the TextureStore.
        myq2_common::common::com_printf("R_Init: Loading overlay textures\n");
        unsafe {
            crate::vk_warp::load_detail_texture();
            crate::vk_warp::load_caustic_texture();
        }
        myq2_common::common::com_printf("R_Init: Overlay textures loaded\n");
        myq2_common::common::com_printf("R_Init: Loading particle textures\n");
        crate::vk_rmisc::r_init_particle_texture();
        myq2_common::common::com_printf("R_Init: Calling draw_init_local\n");
        unsafe { draw_init_local(); }
        myq2_common::common::com_printf("R_Init: draw_init_local completed\n");

        // Set char texture
        myq2_common::common::com_printf("R_Init: Loading conchars texture\n");
        unsafe {
            let chars = crate::vk_image::draw_find_pic("conchars");
            if !chars.is_null() {
                myq2_common::common::com_printf("R_Init: Setting char texture\n");
                modern.draw2d_set_char_texture((*chars).texnum as u32);
            } else {
                myq2_common::common::com_printf("R_Init: Warning - conchars texture not found\n");
            }
        }

        myq2_common::common::com_printf("R_Init: Storing modern renderer\n");
        rg().modern = Some(modern);

        myq2_common::common::com_printf("R_Init: Returning success (1)\n");
        return 1;  // success
    } else {
        vid_printf(PRINT_ALL, "R_Init: Vulkan device not initialized\n");
        return -1;  // failure
    }
}

// ============================================================
// R_Shutdown
// ============================================================
pub fn r_shutdown() {
    cmd_remove_command("modellist");
    cmd_remove_command("screenshot");
    cmd_remove_command("imagelist");
    cmd_remove_command("vk_strings");

    // Shutdown modern renderer before releasing GL context
    {
        let mut rg = rg();
        if let Some(ref mut m) = rg.modern {
            m.shutdown();
        }
        rg.modern = None;
    }

    // SAFETY: mod_free_all and vk_shutdown_images access module-level statics
    unsafe {
        mod_free_all();
        vk_shutdown_images();
    }
    glimp_shutdown();
}

// ============================================================
// R_BeginFrame
// ============================================================
pub fn r_begin_frame(camera_separation: f32) {
    let cv = rcvars();

    // Set camera separation and check gamma
    {
        let mut rg = rg();
        if rg.vk_state.is_none() { rg.vk_state = Some(VkState::default()); }
        rg.vk_state.as_mut().unwrap().camera_separation = camera_separation;
    }

    // change modes if necessary
    if cv.vk_mode.is_modified() || cv.vid_fullscreen.is_modified() {
        cv.vk_mode.clear_modified();
        cv.vid_fullscreen.clear_modified();
        cbuf_add_text("vid_restart\n");
    }

    if cv.vk_log.is_modified() {
        glimp_enable_logging(cv.vk_log.fresh_value());
        cv.vk_log.clear_modified();
    }

    if cv.vk_log.fresh_value() != 0.0 {
        glimp_log_new_frame();
    }

    // Update gamma: hardware ramp (r_hwgamma) or shader gamma (postprocess)
    if cv.vid_gamma.is_modified() {
        cv.vid_gamma.clear_modified();

        let gammaramp = {
            let mut rg = rg();
            if rg.vk_config.is_none() { rg.vk_config = Some(VkConfig::default()); }
            rg.vk_config.as_ref().unwrap().gammaramp
        };
        // Apply hardware gamma ramp when r_hwgamma is enabled and platform supports it
        if cv.r_hwgamma.fresh_value() != 0.0 && gammaramp != 0 {
            update_gamma_ramp();
        }
        // When r_hwgamma is disabled, gamma is applied in the postprocess shader
        // (handled by the polyblend+gamma pass in PostProcessor)
    }

    // Update MSAA and anisotropy settings if cvars changed
    if cv.r_msaa.is_modified() || cv.r_anisotropy.is_modified() {
        let msaa_changed = cv.r_msaa.is_modified();
        cv.r_msaa.clear_modified();
        cv.r_anisotropy.clear_modified();
        let msaa_val = cv.r_msaa.fresh_value() as i32;
        let aniso_val = cv.r_anisotropy.fresh_value() as i32;
        crate::modern::gpu_device::with_device(|ctx| {
            crate::vulkan::update_render_config(ctx, msaa_val, aniso_val);
        });
        // MSAA changes require pipeline recreation (vid_restart)
        if msaa_changed {
            cbuf_add_text("vid_restart\n");
        }
    }

    glimp_begin_frame(camera_separation);

    // go into 2D mode — extract vid dimensions and vk_state info
    let (vid_w, vid_h, cam_sep, stereo_en) = {
        let rg = rg();
        let (cs, se) = rg.vk_state.as_ref()
            .map_or((0.0f32, 0i32), |s| (s.camera_separation, s.stereo_enabled));
        (rg.vid.width, rg.vid.height, cs, se)
    };

    qvk_viewport(0, 0, vid_w as i32, vid_h as i32);
    // GL matrix setup removed - modern renderer handles 2D projection internally
    qvk_disable(VK_DEPTH_TEST);
    qvk_disable(VK_CULL_FACE);
    qvk_disable(VK_BLEND);
    qvk_color4f(1.0, 1.0, 1.0, 1.0);

    // draw buffer stuff
    if cv.vk_drawbuffer.is_modified() {
        cv.vk_drawbuffer.clear_modified();
        let drawbuf_str = myq2_common::cvar::cvar_variable_string("vk_drawbuffer");

        if cam_sep == 0.0 || stereo_en == 0 {
            if drawbuf_str.eq_ignore_ascii_case("VK_FRONT") {
                qvk_draw_buffer(VK_FRONT);
            } else {
                qvk_draw_buffer(VK_BACK);
            }
        }
    }

    // texturemode stuff
    if cv.vk_texturemode.is_modified() {
        let s = myq2_common::cvar::cvar_variable_string("vk_texturemode");
        vk_texture_mode(&s);
        cv.vk_texturemode.clear_modified();
    }

    if cv.vk_texturealphamode.is_modified() {
        let s = myq2_common::cvar::cvar_variable_string("vk_texturealphamode");
        vk_texture_alpha_mode(&s);
        cv.vk_texturealphamode.clear_modified();
    }

    if cv.vk_texturesolidmode.is_modified() {
        let s = myq2_common::cvar::cvar_variable_string("vk_texturesolidmode");
        vk_texture_solid_mode(&s);
        cv.vk_texturesolidmode.clear_modified();
    }

    // swapinterval stuff (locks rg internally)
    crate::vk_rmisc::vk_update_swap_interval();

    // clear screen if desired (locks rg internally)
    r_clear();
}

// ============================================================
// R_SetPalette
// ============================================================
pub fn r_set_palette(palette: Option<&[u8]>) {
    let mut rp = [0u32; 256];
    match palette {
        Some(pal) => {
            for i in 0..256 {
                let r = pal[i * 3] as u32;
                let g = pal[i * 3 + 1] as u32;
                let b = pal[i * 3 + 2] as u32;
                rp[i] = r | (g << 8) | (b << 16) | (0xFF << 24);
            }
        }
        None => {
            let table = crate::vk_image::d_8to24table();
            for i in 0..256 {
                let c = table[i];
                let r = c & 0xFF;
                let g = (c >> 8) & 0xFF;
                let b = (c >> 16) & 0xFF;
                rp[i] = r | (g << 8) | (b << 16) | (0xFF << 24);
            }
        }
    }
    crate::vk_image::set_rawpalette(rp);

    // Main-thread-only renderer state access.
    {
        qvk_clear_color(0.0, 0.0, 0.0, 0.0);
        qvk_clear(VK_COLOR_BUFFER_BIT);
        qvk_clear_color(1.0, 0.0, 0.5, 0.5);
    }
}

// R_DrawBeam — removed (legacy immediate-mode GL; modern renderer will handle beams)

// ============================================================
// Placeholder GL call wrappers not already in vk_local
// ============================================================

fn qvk_tex_envf(target: u32, pname: u32, param: f32) {
    crate::vk_local::qvk_tex_envf(target, pname, param);
}

// qvk_color4ubv_call removed — legacy fixed-function color calls are not
// used; the modern shader pipeline sets colors via uniforms.



