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
use crate::vk_local::{Image, VkConfig, VkState, VidDef, CvarRef,
    PT_MAX, REF_VERSION,
};
use crate::modern::{ModernRenderPath, RenderPath, FrameParams, ParticleData};

// ============================================================
// Global state (corresponds to C file-scope globals)
// ============================================================

/// Modern VBO/shader-based renderer instance.
pub static mut MODERN: Option<ModernRenderPath> = None;

/// Access the modern renderer. Returns `None` if not yet initialized.
/// SAFETY: Must only be called from the single-threaded engine main loop.
pub unsafe fn with_modern<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut ModernRenderPath) -> R,
{
    MODERN.as_mut().map(f)
}

pub static mut FOG_TYPE: i32 = 3;
pub static mut FOG_DENSITY: f32 = 0.0;

pub static mut VID: VidDef = VidDef { width: 0, height: 0 };

pub static mut VK_TEXTURE0_ID: i32 = 0;
pub static mut VK_TEXTURE1_ID: i32 = 0;
pub static mut VK_TEXTURE2_ID: i32 = 0;
pub static mut VK_TEXTURE3_ID: i32 = 0;

pub static mut R_WORLDMODEL: Option<Box<RWorldModel>> = None;

pub static mut GLDEPTHMIN: f32 = 0.0;
pub static mut GLDEPTHMAX: f32 = 0.0;

pub static mut VK_CONFIG: Option<VkConfig> = None;
pub static mut VK_STATE: Option<VkState> = None;

pub static mut R_NOTEXTURE: Option<Image> = None;
pub static mut R_PARTICLETEXTURE: [Option<Image>; PT_MAX] = [
    None, None, None, None, None,
];

pub static mut CURRENTENTITY: Option<usize> = None; // index into refdef entities
pub static mut CURRENTMODEL: Option<usize> = None;

pub static mut FRUSTUM: [CPlane; 4] = [
    CPlane { normal: [0.0; 3], dist: 0.0, plane_type: 0, signbits: 0, pad: [0; 2] },
    CPlane { normal: [0.0; 3], dist: 0.0, plane_type: 0, signbits: 0, pad: [0; 2] },
    CPlane { normal: [0.0; 3], dist: 0.0, plane_type: 0, signbits: 0, pad: [0; 2] },
    CPlane { normal: [0.0; 3], dist: 0.0, plane_type: 0, signbits: 0, pad: [0; 2] },
];


pub static mut V_BLEND: [f32; 4] = [0.0; 4];

// opengl queries
pub static mut MAX_ANISO: i32 = 0;
pub static mut MAX_TSIZE: i32 = 0;

// view origin — use canonical versions from crate::vk_local

pub static mut R_WORLD_MATRIX: [f32; 16] = [0.0; 16];
pub static mut R_BASE_WORLD_MATRIX: [f32; 16] = [0.0; 16];

pub static mut R_NEWREFDEF: Option<RefdefLocal> = None;

pub static mut R_VIEWCLUSTER: i32 = 0;
pub static mut R_VIEWCLUSTER2: i32 = 0;
pub static mut R_OLDVIEWCLUSTER: i32 = 0;
pub static mut R_OLDVIEWCLUSTER2: i32 = 0;

// ============================================================
// Cvar references
// ============================================================

pub static mut R_NOREFRESH: CvarRef = CvarRef { value: 0.0, string: "", modified: false };
pub static mut R_DRAWENTITIES: CvarRef = CvarRef { value: 1.0, string: "", modified: false };
pub static mut R_DRAWWORLD: CvarRef = CvarRef { value: 1.0, string: "", modified: false };
pub static mut R_SPEEDS: CvarRef = CvarRef { value: 0.0, string: "", modified: false };
pub static mut R_FULLBRIGHT: CvarRef = CvarRef { value: 0.0, string: "", modified: false };
pub static mut R_NOVIS: CvarRef = CvarRef { value: 0.0, string: "", modified: false };
pub static mut R_NOCULL: CvarRef = CvarRef { value: 0.0, string: "", modified: false };
pub static mut R_LIGHTLEVEL: CvarRef = CvarRef { value: 0.0, string: "", modified: false };
pub static mut R_OVERBRIGHTBITS: CvarRef = CvarRef { value: 2.0, string: "", modified: false };

pub static mut VK_EXT_MULTITEXTURE: CvarRef = CvarRef { value: 1.0, string: "", modified: false };

pub static mut VK_LOG: CvarRef = CvarRef { value: 0.0, string: "", modified: false };
pub static mut VK_DRAWBUFFER: CvarRef = CvarRef { value: 0.0, string: "", modified: false };
pub static mut VK_DRIVER: CvarRef = CvarRef { value: 0.0, string: "", modified: false };
pub static mut VK_LIGHTMAP: CvarRef = CvarRef { value: 0.0, string: "", modified: false };
pub static mut VK_SHADOWS: CvarRef = CvarRef { value: 1.0, string: "", modified: false };
pub static mut VK_MODE: CvarRef = CvarRef { value: 4.0, string: "", modified: false };
pub static mut VK_DYNAMIC: CvarRef = CvarRef { value: 1.0, string: "", modified: false };
pub static mut VK_MONOLIGHTMAP: CvarRef = CvarRef { value: 0.0, string: "", modified: false };
pub static mut VK_MODULATE_CVAR: CvarRef = CvarRef { value: 1.5, string: "", modified: false };
pub static mut VK_PICMIP: CvarRef = CvarRef { value: 0.0, string: "", modified: false };
pub static mut VK_SKYMIP: CvarRef = CvarRef { value: 0.0, string: "", modified: false };
pub static mut VK_SHOWTRIS: CvarRef = CvarRef { value: 0.0, string: "", modified: false };
pub static mut VK_ZTRICK: CvarRef = CvarRef { value: 0.0, string: "", modified: false };
pub static mut VK_FINISH: CvarRef = CvarRef { value: 0.0, string: "", modified: false };
pub static mut VK_CLEAR_CVAR: CvarRef = CvarRef { value: 0.0, string: "", modified: false };
pub static mut VK_CULL: CvarRef = CvarRef { value: 1.0, string: "", modified: false };
pub static mut VK_POLYBLEND: CvarRef = CvarRef { value: 0.0, string: "", modified: false };
pub static mut VK_FLASHBLEND: CvarRef = CvarRef { value: 0.0, string: "", modified: false };
pub static mut VK_SATURATELIGHTING: CvarRef = CvarRef { value: 0.0, string: "", modified: false };
pub static mut VK_SWAPINTERVAL: CvarRef = CvarRef { value: 1.0, string: "", modified: false };
pub static mut VK_TEXTUREMODE: CvarRef = CvarRef { value: 0.0, string: "", modified: false };
pub static mut VK_TEXTUREALPHAMODE: CvarRef = CvarRef { value: 0.0, string: "", modified: false };
pub static mut VK_TEXTURESOLIDMODE: CvarRef = CvarRef { value: 0.0, string: "", modified: false };
pub static mut VK_LOCKPVS: CvarRef = CvarRef { value: 0.0, string: "", modified: false };

pub static mut VK_EXT_TEXTURE_FILTER_ANISOTROPIC: CvarRef = CvarRef { value: 0.0, string: "", modified: false };
pub static mut VK_SGIS_GENERATE_MIPMAP: CvarRef = CvarRef { value: 0.0, string: "", modified: false };
pub static mut R_CELSHADING: CvarRef = CvarRef { value: 0.0, string: "", modified: false };
pub static mut R_FOG: CvarRef = CvarRef { value: 0.0, string: "", modified: false };
pub static mut R_TIMEBASEDFX: CvarRef = CvarRef { value: 1.0, string: "", modified: false };
pub static mut R_DETAILTEXTURE: CvarRef = CvarRef { value: 7.0, string: "", modified: false };
pub static mut R_CAUSTICS: CvarRef = CvarRef { value: 1.0, string: "", modified: false };
pub static mut R_HWGAMMA: CvarRef = CvarRef { value: 0.0, string: "", modified: false };
pub static mut R_STAINMAP: CvarRef = CvarRef { value: 1.0, string: "", modified: false };
pub static mut R_VERBOSE: CvarRef = CvarRef { value: 0.0, string: "", modified: false };

// Post-processing effect cvars (all enabled by default)
pub static mut R_FXAA: CvarRef = CvarRef { value: 1.0, string: "", modified: false };
pub static mut R_SSAO: CvarRef = CvarRef { value: 1.0, string: "", modified: false };
pub static mut R_SSAO_RADIUS: CvarRef = CvarRef { value: 0.5, string: "", modified: false };
pub static mut R_SSAO_INTENSITY: CvarRef = CvarRef { value: 1.0, string: "", modified: false };
pub static mut R_BLOOM: CvarRef = CvarRef { value: 1.0, string: "", modified: false };
pub static mut R_BLOOM_THRESHOLD: CvarRef = CvarRef { value: 0.8, string: "", modified: false };
pub static mut R_BLOOM_INTENSITY: CvarRef = CvarRef { value: 0.3, string: "", modified: false };
pub static mut R_FSR: CvarRef = CvarRef { value: 1.0, string: "", modified: false };
pub static mut R_FSR_SCALE: CvarRef = CvarRef { value: 0.75, string: "", modified: false };
pub static mut R_FSR_SHARPNESS: CvarRef = CvarRef { value: 0.2, string: "", modified: false };

// MSAA and Anisotropic filtering (R1Q2/Q2Pro feature)
/// MSAA sample count: 0=disabled, 2, 4, or 8
pub static mut R_MSAA: CvarRef = CvarRef { value: 0.0, string: "", modified: false };
/// Anisotropic filtering level: 1=disabled, 2, 4, 8, or 16
pub static mut R_ANISOTROPY: CvarRef = CvarRef { value: 1.0, string: "", modified: false };

// Screenshot format and quality (R1Q2/Q2Pro feature)
/// Screenshot format: "tga", "png", or "jpg"
pub static mut VK_SCREENSHOT_FORMAT: CvarRef = CvarRef { value: 0.0, string: "tga", modified: false };
/// JPEG screenshot quality: 0-100 (only used for jpg format)
pub static mut VK_SCREENSHOT_QUALITY: CvarRef = CvarRef { value: 85.0, string: "", modified: false };

pub static mut VK_3DLABS_BROKEN: CvarRef = CvarRef { value: 0.0, string: "", modified: false };

pub static mut VID_FULLSCREEN: CvarRef = CvarRef { value: 1.0, string: "", modified: false };
pub static mut VID_GAMMA: CvarRef = CvarRef { value: 0.6, string: "", modified: false };
pub static mut VID_REF: CvarRef = CvarRef { value: 0.0, string: "", modified: false };

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
    pub flags: i32,
    pub alpha: f32,
    pub skinnum: i32,
}

impl Default for EntityLocal {
    fn default() -> Self {
        Self {
            origin: Vec3::default(),
            oldorigin: Vec3::default(),
            angles: Vec3::default(),
            model: std::ptr::null(),
            frame: 0,
            flags: 0,
            alpha: 0.0,
            skinnum: 0,
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
pub const VK_RENDERER_VOODOO_C: u32 = 0x00000001;
pub const VK_RENDERER_VOODOO_RUSH: u32 = 0x00000002;
pub const VK_RENDERER_SGI: u32 = 0x00000004;
pub const VK_RENDERER_PERMEDIA2: u32 = 0x00000008;
pub const VK_RENDERER_GLINT_MX: u32 = 0x00000010;
pub const VK_RENDERER_REALIZM: u32 = 0x00000020;
pub const VK_RENDERER_MCD_C: u32 = 0x00000040;
pub const VK_RENDERER_PCX2: u32 = 0x00000080;
pub const VK_RENDERER_RENDITION_C: u32 = 0x00000100;
pub const VK_RENDERER_INTERGRAPH: u32 = 0x00000200;
pub const VK_RENDERER_3DLABS: u32 = 0x00000400;
pub const VK_RENDERER_POWERVR: u32 = 0x00000800;
pub const VK_RENDERER_OTHER: u32 = 0x80000000;

pub const SKYBOX_SIZE: f64 = 4096.0;
pub const NUM_BEAM_SEGS: usize = 6;

// CVAR flags come from myq2_common::q_shared::* (imported above)

// d_8to24table — canonical definition in vk_image.rs
// Use crate::vk_image::d_8to24table directly.

// visible counters from vk_rsurf
pub static mut C_VISIBLE_TEXTURES: i32 = 0;
pub static mut C_VISIBLE_LIGHTMAPS: i32 = 0;

// r_turbsin table
pub static mut R_TURBSIN: [f32; 256] = [0.0f32; 256];

// ============================================================
// Placeholder external functions
// ============================================================

// --- Already wired to real implementations ---
unsafe fn r_light_point(p: &Vec3, color: &mut Vec3) { crate::vk_light::r_light_point(p, color); }
unsafe fn r_push_dlights() { crate::vk_light::r_push_dlights(); }
unsafe fn r_mark_leaves() { crate::vk_rsurf::r_mark_leaves(); }
unsafe fn vk_init_images() { crate::vk_image::vk_init_images(); }
unsafe fn vk_shutdown_images() { crate::vk_image::vk_shutdown_images(); }
unsafe fn mod_init() { crate::vk_model::mod_init(); }
unsafe fn mod_free_all() { crate::vk_model::mod_free_all(); }
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
fn mod_modellist_f() { unsafe { crate::vk_model::mod_modellist_f(); } }

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
// Once the world model is loaded via vk_model, R_WORLDMODEL will hold the real Model*
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
fn qvk_init(driver: &str) -> bool { crate::platform::qvk_init(driver) }
fn qvk_shutdown() { crate::platform::qvk_shutdown(); }

// --- Vulkan call logging (mirrors GLimp_EnableLogging / GLimp_LogNewFrame from original qgl_win.c) ---
// The original OpenGL implementation swapped function pointers to logging wrappers.
// With Vulkan 1.3 via ash, validation layers provide comprehensive API tracing.
// This log file supports the vk_log cvar for frame markers and debug output.
static mut VK_LOG_FP: Option<std::fs::File> = None;

fn glimp_enable_logging(enable: f32) {
    unsafe {
        if enable != 0.0 {
            if VK_LOG_FP.is_none() {
                let gamedir = myq2_common::files::fs_gamedir();
                let path = format!("{}/gl.log", gamedir);
                match std::fs::File::create(&path) {
                    Ok(f) => {
                        use std::io::Write;
                        let mut f = f;
                        let _ = writeln!(f, "GL log opened");
                        VK_LOG_FP = Some(f);
                        myq2_common::common::com_printf(&format!("GL logging to {}\n", path));
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
            VK_LOG_FP = None;
        }
    }
}

fn glimp_log_new_frame() {
    unsafe {
        if let Some(ref mut f) = VK_LOG_FP {
            use std::io::Write;
            let _ = writeln!(f, "*** R_BeginFrame ***");
        }
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
    let _idx = myq2_common::cvar::cvar_get(name, default, flags);
    let s = myq2_common::cvar::cvar_variable_string(name);
    let v = myq2_common::cvar::cvar_variable_value(name);
    // Leak the String to get a &'static str, matching CvarRef's field type.
    // These are long-lived cvar registrations so the leak is acceptable.
    let leaked: &'static str = Box::leak(s.into_boxed_str());
    CvarRef {
        string: leaked,
        value: v,
        modified: false,
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

// --- Wired to crate::vk_local ---
fn qvk_translate_f(x: f32, y: f32, z: f32) { crate::vk_local::qvk_translatef(x, y, z); }
fn qvk_rotate_f(angle: f32, x: f32, y: f32, z: f32) { crate::vk_local::qvk_rotatef(angle, x, y, z); }
fn qvk_enable(cap: u32) { crate::vk_local::qvk_enable(cap); }
fn qvk_disable(cap: u32) { crate::vk_local::qvk_disable(cap); }
fn qvk_color4f(r: f32, g: f32, b: f32, a: f32) { crate::vk_local::qvk_color4f(r, g, b, a); }

// --- GL functions wired to crate::vk_bindings ---
fn qvk_load_identity() {
    // SAFETY: Delegates to OpenGL.
    unsafe { crate::vk_bindings::LoadIdentity(); }
}
fn qvk_matrix_mode(mode: u32) {
    // SAFETY: Delegates to OpenGL.
    unsafe { crate::vk_bindings::MatrixMode(mode); }
}
fn qvk_ortho(l: f64, r: f64, b: f64, t: f64, n: f64, f: f64) {
    // SAFETY: Delegates to OpenGL.
    unsafe { crate::vk_bindings::Ortho(l, r, b, t, n, f); }
}
fn qvk_frustum(l: f64, r: f64, b: f64, t: f64, n: f64, f: f64) {
    // SAFETY: Delegates to OpenGL.
    unsafe { crate::vk_bindings::Frustum(l, r, b, t, n, f); }
}
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
fn qvk_cull_face(mode: u32) {
    // SAFETY: Delegates to OpenGL.
    unsafe { crate::vk_bindings::CullFace(mode); }
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
    // SAFETY: single-threaded engine access pattern
    unsafe {
        if R_NOCULL.value != 0.0 {
            return false;
        }
        for i in 0..4 {
            if box_on_plane_side(mins, maxs, &FRUSTUM[i]) == 2 {
                return true;
            }
        }
    }
    false
}

// ============================================================
// R_RotateForEntity
// ============================================================
pub fn r_rotate_for_entity(e: &EntityLocal) {
    qvk_translate_f(e.origin[0], e.origin[1], e.origin[2]);
    qvk_rotate_f(e.angles[1], 0.0, 0.0, 1.0);
    qvk_rotate_f(-e.angles[0], 0.0, 1.0, 0.0);
    qvk_rotate_f(-e.angles[2], 1.0, 0.0, 0.0);
}

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
    // SAFETY: single-threaded engine access pattern
    unsafe {
        let refdef = match R_NEWREFDEF.as_ref() {
            Some(r) => r,
            None => return,
        };

        // rotate VPN right by FOV_X/2 degrees
        rotate_point_around_vector(
            &mut FRUSTUM[0].normal, &crate::vk_local::vup, &crate::vk_local::vpn,
            -(90.0 - refdef.fov_x / 2.0),
        );
        // rotate VPN left by FOV_X/2 degrees
        rotate_point_around_vector(
            &mut FRUSTUM[1].normal, &crate::vk_local::vup, &crate::vk_local::vpn,
            90.0 - refdef.fov_x / 2.0,
        );
        // rotate VPN up by FOV_Y/2 degrees
        rotate_point_around_vector(
            &mut FRUSTUM[2].normal, &crate::vk_local::vright, &crate::vk_local::vpn,
            90.0 - refdef.fov_y / 2.0,
        );
        // rotate VPN down by FOV_Y/2 degrees
        rotate_point_around_vector(
            &mut FRUSTUM[3].normal, &crate::vk_local::vright, &crate::vk_local::vpn,
            -(90.0 - refdef.fov_y / 2.0),
        );

        for i in 0..4 {
            FRUSTUM[i].plane_type = PLANE_ANYZ;
            FRUSTUM[i].dist = dot_product(&crate::vk_local::r_origin, &FRUSTUM[i].normal);
            FRUSTUM[i].signbits = signbits_for_plane(&FRUSTUM[i]);
        }
    }
}

const PLANE_ANYZ: u8 = 5;

// ============================================================
// R_SetupFrame
// ============================================================
pub fn r_setup_frame() {
    // SAFETY: single-threaded engine access pattern
    unsafe {
        crate::vk_local::r_framecount += 1;

        let refdef = match R_NEWREFDEF.as_ref() {
            Some(r) => r.clone(),
            None => return,
        };

        // build the transformation matrix for the given view angles
        crate::vk_local::r_origin = refdef.vieworg;
        angle_vectors(&refdef.viewangles, Some(&mut crate::vk_local::vpn), Some(&mut crate::vk_local::vright), Some(&mut crate::vk_local::vup));

        // current viewcluster
        if refdef.rdflags & RDF_NOWORLDMODEL == 0 {
            R_OLDVIEWCLUSTER = R_VIEWCLUSTER;
            R_OLDVIEWCLUSTER2 = R_VIEWCLUSTER2;

            if let Some(ref worldmodel) = R_WORLDMODEL {
                let leaf = mod_point_in_leaf(&crate::vk_local::r_origin, worldmodel);
                R_VIEWCLUSTER = leaf.cluster;
                R_VIEWCLUSTER2 = leaf.cluster;

                // check above and below so crossing solid water doesn't draw wrong
                if leaf.contents == 0 {
                    // look down a bit
                    let mut temp = crate::vk_local::r_origin;
                    temp[2] -= 16.0;
                    let leaf2 = mod_point_in_leaf(&temp, worldmodel);
                    if leaf2.contents & CONTENTS_SOLID == 0
                        && leaf2.cluster != R_VIEWCLUSTER2
                    {
                        R_VIEWCLUSTER2 = leaf2.cluster;
                    }
                } else {
                    // look up a bit
                    let mut temp = crate::vk_local::r_origin;
                    temp[2] += 16.0;
                    let leaf2 = mod_point_in_leaf(&temp, worldmodel);
                    if leaf2.contents & CONTENTS_SOLID == 0
                        && leaf2.cluster != R_VIEWCLUSTER2
                    {
                        R_VIEWCLUSTER2 = leaf2.cluster;
                    }
                }
            }
        }

        for i in 0..4 {
            V_BLEND[i] = refdef.blend[i];
        }

        crate::vk_local::c_brush_polys = 0;
        crate::vk_local::c_alias_polys = 0;

        // clear out the portion of the screen that the NOWORLDMODEL defines
        if refdef.rdflags & RDF_NOWORLDMODEL != 0 {
            qvk_enable(VK_SCISSOR_TEST);
            qvk_clear_color(0.3, 0.3, 0.3, 1.0);
            qvk_scissor(
                refdef.x,
                VID.height as i32 - refdef.height - refdef.y,
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
// MYgluPerspective
// ============================================================
pub fn my_glu_perspective(fovy: f64, aspect: f64, z_near: f64, z_far: f64) {
    // SAFETY: single-threaded engine access pattern
    unsafe {
        let ymax = z_near * (fovy * std::f64::consts::PI / 360.0).tan();
        let ymin = -ymax;
        let mut xmin = ymin * aspect;
        let mut xmax = ymax * aspect;

        let vk_state = VK_STATE.as_ref();
        let camera_sep = vk_state.map_or(0.0, |s| s.camera_separation);
        xmin += -(2.0 * camera_sep as f64) / z_near;
        xmax += -(2.0 * camera_sep as f64) / z_near;

        qvk_frustum(xmin, xmax, ymin, ymax, z_near, z_far);
    }
}

// ============================================================
// R_SetupGL
// ============================================================
pub fn r_setup_gl() {
    // SAFETY: single-threaded engine access pattern
    unsafe {
        static mut RUNONCE: bool = false;
        static mut FARZ: f64 = 0.0;

        let refdef = match R_NEWREFDEF.as_ref() {
            Some(r) => r,
            None => return,
        };

        // set up viewport
        let x = (refdef.x as f32 * VID.width as f32 / VID.width as f32).floor() as i32;
        let x2 = ((refdef.x + refdef.width) as f32 * VID.width as f32 / VID.width as f32).ceil() as i32;
        let y = (VID.height as f32 - refdef.y as f32 * VID.height as f32 / VID.height as f32).floor() as i32;
        let y2 = (VID.height as f32 - (refdef.y + refdef.height) as f32 * VID.height as f32 / VID.height as f32).ceil() as i32;

        let w = x2 - x;
        let h = y - y2;

        qvk_viewport(x, y2, w, h);

        // DMP: calc farz value from skybox size
        if !RUNONCE {
            RUNONCE = true;
            let mut boxsize = SKYBOX_SIZE;
            boxsize -= 252.0 * (boxsize / 2300.0).ceil();
            FARZ = 1.0;
            while FARZ < boxsize {
                FARZ *= 2.0;
                if FARZ >= 65536.0 {
                    break;
                }
            }
            FARZ *= 2.0;
            vid_printf(PRINT_DEVELOPER, &format!("farz now set to {}\n", FARZ));
        }

        // set up projection matrix
        let screenaspect = refdef.width as f32 / refdef.height as f32;
        qvk_matrix_mode(VK_PROJECTION);
        qvk_load_identity();

        my_glu_perspective(refdef.fov_y as f64, screenaspect as f64, 4.0, FARZ);

        qvk_cull_face(VK_FRONT);

        qvk_matrix_mode(VK_MODELVIEW);
        qvk_load_identity();

        qvk_rotate_f(-90.0, 1.0, 0.0, 0.0); // put Z going up
        qvk_rotate_f(90.0, 0.0, 0.0, 1.0);  // put Z going up

        qvk_rotate_f(-refdef.viewangles[2], 1.0, 0.0, 0.0);
        qvk_rotate_f(-refdef.viewangles[0], 0.0, 1.0, 0.0);
        qvk_rotate_f(-refdef.viewangles[1], 0.0, 0.0, 1.0);
        qvk_translate_f(-refdef.vieworg[0], -refdef.vieworg[1], -refdef.vieworg[2]);

        qvk_get_floatv(VK_MODELVIEW_MATRIX, &mut R_WORLD_MATRIX);

        // set drawing parms
        if VK_CULL.value != 0.0 {
            qvk_enable(VK_CULL_FACE);
        } else {
            qvk_disable(VK_CULL_FACE);
        }

        qvk_disable(VK_BLEND);
        // GL_ALPHA_TEST disable removed — alpha testing handled by GLSL discard
        qvk_enable(VK_DEPTH_TEST);
    }
}

// ============================================================
// R_Clear
// ============================================================
pub fn r_clear() {
    // SAFETY: single-threaded engine access pattern
    unsafe {
        static mut TRICKFRAME: i32 = 0;

        if VK_ZTRICK.value != 0.0 {
            if VK_CLEAR_CVAR.value != 0.0 {
                qvk_clear(VK_COLOR_BUFFER_BIT);
            }

            TRICKFRAME += 1;
            if TRICKFRAME & 1 != 0 {
                GLDEPTHMIN = 0.0;
                GLDEPTHMAX = 0.49999;
                qvk_depth_func(VK_LEQUAL);
            } else {
                GLDEPTHMIN = 1.0;
                GLDEPTHMAX = 0.5;
                qvk_depth_func(VK_GEQUAL);
            }
        } else {
            if VK_CLEAR_CVAR.value != 0.0 {
                qvk_clear(VK_COLOR_BUFFER_BIT | VK_DEPTH_BUFFER_BIT);
            } else {
                qvk_clear(VK_DEPTH_BUFFER_BIT);
            }
            GLDEPTHMIN = 0.0;
            GLDEPTHMAX = 1.0;
            qvk_depth_func(VK_LEQUAL);
        }

        qvk_depth_range(GLDEPTHMIN as f64, GLDEPTHMAX as f64);

        // Stencil shadows - MrG
        if VK_SHADOWS.value != 0.0 {
            qvk_clear_stencil(1);
            qvk_clear(VK_STENCIL_BUFFER_BIT);
        }
    }
}

// ============================================================
// R_Flash
// ============================================================
pub fn r_flash() {
    // Legacy r_poly_blend removed; modern PostProcessor handles polyblend via V_BLEND.
}

// ============================================================
// R_SetupFog — mattx86: engine_fog
// ============================================================
pub fn r_setup_fog() {
    // SAFETY: single-threaded engine access pattern
    unsafe {
        // timebasedfx arrays
        let ampmarray: [[f32; 13]; 2] = [
            // PM
            [0.00000, 0.00000, 0.00000, 0.00000, 0.00000, 0.00000, 0.00000,
             0.00000, 0.00000, 0.00000, 0.00020, 0.00040, 0.00000],
            // AM
            [0.00000, 0.00050, 0.00040, 0.00030, 0.00020, 0.00010, 0.00005,
             0.00000, 0.00000, 0.00000, 0.00000, 0.00000, 0.00060],
        ];

        let refdef = match R_NEWREFDEF.as_ref() {
            Some(r) => r,
            None => return,
        };

        let point_contents = cm_point_contents(&refdef.vieworg, 0);
        if point_contents & CONTENTS_WATER != 0 {
            FOG_TYPE = 0;
        } else if point_contents & CONTENTS_SLIME != 0 {
            FOG_TYPE = 1;
        } else if point_contents & CONTENTS_LAVA != 0 {
            FOG_TYPE = 2;
        } else {
            FOG_TYPE = 3;
        }

        if R_FOG.value != 0.0 || (R_FOG.value == 0.0 && (FOG_TYPE == 1 || FOG_TYPE == 2)) {
            if R_TIMEBASEDFX.value != 0.0 && (FOG_TYPE == 0 || FOG_TYPE == 3) {
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
                FOG_DENSITY = ampmarray[am][hour_12];
            } else if FOG_TYPE == 1 || FOG_TYPE == 2 {
                FOG_DENSITY = 0.1200;
            } else {
                FOG_DENSITY = 0.0675;
            }

            // Fixed-function GL_FOG calls removed — fog is now handled
            // entirely by GLSL shaders via u_FogDensity / u_FogColor uniforms.
            // FOG_DENSITY is still computed above so the shader can read it.
        } else {
            FOG_DENSITY = 0.0;
        }
    }
}

// ============================================================
// R_RenderView — r_newrefdef must be set before the first call
// ============================================================
pub fn r_render_view(fd: &RefdefLocal) {
    // SAFETY: single-threaded engine access pattern
    unsafe {
        if R_NOREFRESH.value != 0.0 {
            return;
        }

        R_NEWREFDEF = Some(fd.clone());

        if R_WORLDMODEL.is_none()
            && (fd.rdflags & RDF_NOWORLDMODEL == 0)
        {
            vid_printf(ERR_DROP, "R_RenderView: NULL worldmodel");
            return;
        }

        if R_SPEEDS.value != 0.0 {
            crate::vk_local::c_brush_polys = 0;
            crate::vk_local::c_alias_polys = 0;
        }

        r_push_dlights();

        if VK_FINISH.value != 0.0 {
            qvk_finish();
        }

        // Legacy state computation (frustum, leaves, etc.)
        r_setup_frame();
        r_set_frustum();
        r_setup_gl();
        r_mark_leaves();
        r_setup_fog();

        // Modern renderer: begin 3D pass with view parameters
        let params = FrameParams {
            time: crate::vk_local::r_newrefdef.time,
            vieworg: fd.vieworg,
            viewangles: fd.viewangles,
            fov_x: fd.fov_x,
            fov_y: fd.fov_y,
            width: fd.width as u32,
            height: fd.height as u32,
            blend: fd.blend,
            rdflags: fd.rdflags,
        };

        let modern = MODERN.as_mut().unwrap();
        modern.begin_frame(&params);

        // World geometry
        if R_DRAWWORLD.value != 0.0 {
            modern.draw_world();
        }

        // Entities
        if R_DRAWENTITIES.value != 0.0 {
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

        // Effects
        modern.render_dlights();

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

        if R_SPEEDS.value != 0.0 {
            vid_printf(PRINT_ALL, &format!(
                "{:4} wpoly {:4} epoly {} tex {} lmaps\n",
                crate::vk_local::c_brush_polys, crate::vk_local::c_alias_polys,
                C_VISIBLE_TEXTURES, C_VISIBLE_LIGHTMAPS,
            ));
        }
    }
}

// ============================================================
// R_SetGL2D
// ============================================================
pub fn r_set_gl2d() {
    // SAFETY: single-threaded engine access pattern
    unsafe {
        qvk_viewport(0, 0, VID.width as i32, VID.height as i32);
        qvk_matrix_mode(VK_PROJECTION);
        qvk_load_identity();
        qvk_ortho(0.0, VID.width as f64, VID.height as f64, 0.0, -99999.0, 99999.0);
        qvk_matrix_mode(VK_MODELVIEW);
        qvk_load_identity();
        qvk_disable(VK_DEPTH_TEST);
        qvk_disable(VK_CULL_FACE);
        qvk_disable(VK_BLEND);
        // GL_ALPHA_TEST enable removed — alpha testing handled by GLSL discard
        qvk_color4f(1.0, 1.0, 1.0, 1.0);

        if let Some(ref mut state) = VK_STATE {
            state.transconsole = 1; // mattx86: trans_console
        }
    }
}

// GL_DrawColoredStereoLinePair / GL_DrawStereoPattern — removed (legacy immediate-mode GL)

// ============================================================
// R_SetLightLevel
// ============================================================
pub fn r_set_light_level() {
    // SAFETY: single-threaded engine access pattern
    unsafe {
        let refdef = match R_NEWREFDEF.as_ref() {
            Some(r) => r,
            None => return,
        };

        if refdef.rdflags & RDF_NOWORLDMODEL != 0 {
            return;
        }

        // save off light value for server to look at (BIG HACK!)
        let mut shadelight: Vec3 = [0.0; 3];
        r_light_point(&refdef.vieworg, &mut shadelight);

        // pick the greatest component
        if shadelight[0] > shadelight[1] {
            if shadelight[0] > shadelight[2] {
                R_LIGHTLEVEL.value = 150.0 * shadelight[0];
            } else {
                R_LIGHTLEVEL.value = 150.0 * shadelight[2];
            }
        } else if shadelight[1] > shadelight[2] {
            R_LIGHTLEVEL.value = 150.0 * shadelight[1];
        } else {
            R_LIGHTLEVEL.value = 150.0 * shadelight[2];
        }
    }
}

// ============================================================
// R_RenderFrame
// ============================================================
pub fn r_render_frame(fd: &RefdefLocal) {
    r_render_view(fd);
    r_set_light_level();
    // SAFETY: single-threaded engine access pattern
    unsafe {
        // Flush 2D batches and apply post-processing before switching to 2D mode
        MODERN.as_mut().unwrap().end_frame();
    }
    r_set_gl2d();
}

// ============================================================
// R_Register
// ============================================================
pub fn r_register() {
    // SAFETY: single-threaded engine access pattern
    unsafe {
        R_NOREFRESH = cvar_get("r_norefresh", "0", CVAR_ZERO);
        R_FULLBRIGHT = cvar_get("r_fullbright", "0", CVAR_ZERO);
        R_DRAWENTITIES = cvar_get("r_drawentities", "1", CVAR_ZERO);
        R_DRAWWORLD = cvar_get("r_drawworld", "1", CVAR_ZERO);
        R_NOVIS = cvar_get("r_novis", "0", CVAR_ZERO);
        R_NOCULL = cvar_get("r_nocull", "0", CVAR_ZERO);
        R_SPEEDS = cvar_get("r_speeds", "0", CVAR_ZERO);
        R_LIGHTLEVEL = cvar_get("r_lightlevel", "0", CVAR_ZERO);
        R_OVERBRIGHTBITS = cvar_get("r_overbrightbits", "2", CVAR_ARCHIVE);

        // flushmap — read via cvar_variable_value in vk_model.rs, no static needed.
        cvar_get("flushmap", "0", CVAR_ZERO);

        VK_MODULATE_CVAR = cvar_get("vk_modulate", "1.5", CVAR_ARCHIVE);
        VK_LOG = cvar_get("vk_log", "0", CVAR_ZERO);
        VK_MODE = cvar_get("vk_mode", "4", CVAR_ARCHIVE);
        VK_LIGHTMAP = cvar_get("vk_lightmap", "0", CVAR_ZERO);
        VK_SHADOWS = cvar_get("vk_shadows", "1", CVAR_ARCHIVE);
        VK_DYNAMIC = cvar_get("vk_dynamic", "1", CVAR_ARCHIVE);
        VK_PICMIP = cvar_get("vk_picmip", "0", CVAR_ARCHIVE);
        VK_SKYMIP = cvar_get("vk_skymip", "0", CVAR_ARCHIVE);
        VK_SHOWTRIS = cvar_get("vk_showtris", "0", CVAR_ZERO);
        VK_ZTRICK = cvar_get("vk_ztrick", "0", CVAR_ARCHIVE);
        VK_FINISH = cvar_get("vk_finish", "0", CVAR_ARCHIVE);
        VK_CLEAR_CVAR = cvar_get("vk_clear", "0", CVAR_ZERO);
        VK_CULL = cvar_get("vk_cull", "1", CVAR_ARCHIVE);
        VK_POLYBLEND = cvar_get("vk_polyblend", "1", CVAR_ARCHIVE);
        VK_FLASHBLEND = cvar_get("vk_flashblend", "0", CVAR_ARCHIVE);
        VK_MONOLIGHTMAP = cvar_get("vk_monolightmap", "0", CVAR_ZERO);
        VK_DRIVER = cvar_get("vk_driver", "opengl32", CVAR_ARCHIVE);
        VK_TEXTUREMODE = cvar_get("vk_texturemode", "VK_LINEAR_MIPMAP_LINEAR", CVAR_ARCHIVE);
        VK_TEXTUREALPHAMODE = cvar_get("vk_texturealphamode", "default", CVAR_ZERO);
        VK_TEXTURESOLIDMODE = cvar_get("vk_texturesolidmode", "default", CVAR_ZERO);
        VK_LOCKPVS = cvar_get("vk_lockpvs", "0", CVAR_ZERO);

        VK_EXT_MULTITEXTURE = cvar_get("vk_ext_multitexture", "1", CVAR_ARCHIVE);

        VK_DRAWBUFFER = cvar_get("vk_drawbuffer", "VK_BACK", CVAR_ARCHIVE);
        VK_SWAPINTERVAL = cvar_get("vk_swapinterval", "1", CVAR_ARCHIVE);
        VK_SATURATELIGHTING = cvar_get("vk_saturatelighting", "0", CVAR_ARCHIVE);

        VK_3DLABS_BROKEN = cvar_get("vk_3dlabs_broken", "0", CVAR_ARCHIVE);

        VK_EXT_TEXTURE_FILTER_ANISOTROPIC = cvar_get("vk_ext_texture_filter_anisotropic", "1", CVAR_ARCHIVE);
        VK_SGIS_GENERATE_MIPMAP = cvar_get("vk_sgis_generate_mipmap", "0", CVAR_ARCHIVE);
        R_CELSHADING = cvar_get("r_celshading", "0", CVAR_ARCHIVE);
        R_FOG = cvar_get("r_fog", "0", CVAR_ARCHIVE);
        R_TIMEBASEDFX = cvar_get("r_timebasedfx", "1", CVAR_ARCHIVE);
        R_DETAILTEXTURE = cvar_get("r_detailtexture", "7", CVAR_ARCHIVE);
        R_CAUSTICS = cvar_get("r_caustics", "1", CVAR_ARCHIVE);
        R_HWGAMMA = cvar_get("r_hwgamma", "0", CVAR_ARCHIVE);
        R_STAINMAP = cvar_get("r_stainmap", "1", CVAR_ARCHIVE);
        R_VERBOSE = cvar_get("r_verbose", "0", CVAR_ZERO);

        // Post-processing effect cvars (all enabled by default)
        R_FXAA = cvar_get("r_fxaa", "1", CVAR_ARCHIVE);
        R_SSAO = cvar_get("r_ssao", "1", CVAR_ARCHIVE);
        R_SSAO_RADIUS = cvar_get("r_ssao_radius", "0.5", CVAR_ARCHIVE);
        R_SSAO_INTENSITY = cvar_get("r_ssao_intensity", "1.0", CVAR_ARCHIVE);
        R_BLOOM = cvar_get("r_bloom", "1", CVAR_ARCHIVE);
        R_BLOOM_THRESHOLD = cvar_get("r_bloom_threshold", "0.8", CVAR_ARCHIVE);
        R_BLOOM_INTENSITY = cvar_get("r_bloom_intensity", "0.3", CVAR_ARCHIVE);
        R_FSR = cvar_get("r_fsr", "1", CVAR_ARCHIVE);
        R_FSR_SCALE = cvar_get("r_fsr_scale", "0.75", CVAR_ARCHIVE);
        R_FSR_SHARPNESS = cvar_get("r_fsr_sharpness", "0.2", CVAR_ARCHIVE);

        // MSAA and anisotropic filtering (R1Q2/Q2Pro feature)
        // r_msaa: 0=disabled, 2, 4, or 8 samples
        R_MSAA = cvar_get("r_msaa", "0", CVAR_ARCHIVE);
        // r_anisotropy: 1=disabled, 2, 4, 8, or 16
        R_ANISOTROPY = cvar_get("r_anisotropy", "8", CVAR_ARCHIVE);

        // Screenshot format and quality (R1Q2/Q2Pro feature)
        // vk_screenshot_format: "tga", "png", or "jpg"
        VK_SCREENSHOT_FORMAT = cvar_get("vk_screenshot_format", "tga", CVAR_ARCHIVE);
        // vk_screenshot_quality: 0-100 (JPEG quality, only used for jpg format)
        VK_SCREENSHOT_QUALITY = cvar_get("vk_screenshot_quality", "85", CVAR_ARCHIVE);

        // Initialize Vulkan render configuration with MSAA and anisotropy settings
        crate::modern::gpu_device::with_device(|ctx| {
            crate::vulkan::init_render_config(ctx, R_MSAA.value as i32, R_ANISOTROPY.value as i32);
        });

        VID_FULLSCREEN = cvar_get("vid_fullscreen", "1", CVAR_ARCHIVE);
        VID_GAMMA = cvar_get("vid_gamma", "0.6", CVAR_ARCHIVE);
        VID_REF = cvar_get("vid_ref", "gl", CVAR_ARCHIVE);

        cmd_add_command("imagelist", vk_image_list_f);
        cmd_add_command("screenshot", crate::vk_rmisc::vk_screen_shot_f);
        cmd_add_command("modellist", mod_modellist_f);
        cmd_add_command("vk_strings", crate::vk_rmisc::vk_strings_f);
    }
}

// ============================================================
// R_SetMode
// ============================================================
pub fn r_set_mode() -> bool {
    // SAFETY: single-threaded engine access pattern
    unsafe {
        let vk_config = VK_CONFIG.get_or_insert_with(VkConfig::default);

        if VID_FULLSCREEN.modified && vk_config.allow_cds == 0 {
            vid_printf(PRINT_ALL, "R_SetMode() - CDS not allowed with this driver\n");
            cvar_set_value("vid_fullscreen", if VID_FULLSCREEN.value != 0.0 { 0.0 } else { 1.0 });
            VID_FULLSCREEN.modified = false;
        }

        let fullscreen = VID_FULLSCREEN.value != 0.0;
        VID_FULLSCREEN.modified = false;
        VK_MODE.modified = false;

        let vk_state = VK_STATE.get_or_insert_with(VkState::default);

        let err = glimp_set_mode(&mut VID.width, &mut VID.height, VK_MODE.value, fullscreen);
        if err == RSERR_OK {
            vk_state.prev_mode = VK_MODE.value as i32;
        } else {
            if err == RSERR_INVALID_FULLSCREEN {
                cvar_set_value("vid_fullscreen", 0.0);
                VID_FULLSCREEN.modified = false;
                vid_printf(PRINT_ALL, "ref_gl::R_SetMode() - fullscreen unavailable in this mode\n");
                if glimp_set_mode(&mut VID.width, &mut VID.height, VK_MODE.value, false) == RSERR_OK {
                    return true;
                }
            } else if err == RSERR_INVALID_MODE {
                cvar_set_value("vk_mode", vk_state.prev_mode as f32);
                VK_MODE.modified = false;
                vid_printf(PRINT_ALL, "ref_gl::R_SetMode() - invalid mode\n");
            }

            // try setting it back to something safe
            if glimp_set_mode(&mut VID.width, &mut VID.height, vk_state.prev_mode as f32, false) != RSERR_OK {
                vid_printf(PRINT_ALL, "ref_gl::R_SetMode() - could not revert to safe mode\n");
                return false;
            }
        }
        true
    }
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
    // SAFETY: single-threaded engine access pattern
    unsafe {
        for j in 0..256 {
            R_TURBSIN[j] *= 0.5;
        }

        vid_printf(PRINT_INFO, &format!("ref_gl version: {}\n", REF_VERSION));

        // reversed for saturation control
        r_register();
        draw_get_palette();

        // initialize our QGL dynamic bindings
        if !qvk_init(VK_DRIVER.string) {
            qvk_shutdown();
            vid_printf(PRINT_ALL, &format!("ref_gl::R_Init() - could not load \"{}\"\n", VK_DRIVER.string));
            return -1;
        }

        // initialize OS-specific parts of OpenGL
        if !glimp_init(hinstance, hwnd) {
            qvk_shutdown();
            return -1;
        }

        // set our "safe" modes
        let vk_state = VK_STATE.get_or_insert_with(VkState::default);
        vk_state.prev_mode = 3;

        // create the window and set up the context
        if !r_set_mode() {
            qvk_shutdown();
            vid_printf(PRINT_ALL, "ref_gl::R_Init() - could not R_SetMode()\n");
            return -1;
        }

        // If SDL3 GPU device is active (Vulkan/Metal/DX12), skip all GL-based
        // initialization. The modern renderer will be initialized incrementally
        // as the GL→GPU migration progresses (phases B3-B7).
        if crate::modern::gpu_device::is_initialized() {
            vid_printf(PRINT_ALL, "R_Init: SDL3 GPU device active, skipping GL initialization\n");
            vid_menu_init();
            r_register();
            draw_get_palette();
            // Load overlay textures (detail + caustic) for world shader
            crate::vk_warp::load_detail_texture();
            crate::vk_warp::load_caustic_texture();
            // Create an uninitialized ModernRenderPath (all render methods are no-ops
            // until the GPU pipeline is fully wired in B5+).
            let mut modern = ModernRenderPath::new();
            modern.set_dimensions(VID.width as u32, VID.height as u32);
            MODERN = Some(modern);
            return 0;
        }

        vid_menu_init();

        // get our various GL strings
        let vk_config = VK_CONFIG.get_or_insert_with(VkConfig::default);
        vk_config.vendor_string = qvk_get_string(VK_VENDOR);
        vid_printf(PRINT_INFO, &format!("VK_VENDOR: {}\n", cptr_to_str(vk_config.vendor_string)));
        vk_config.renderer_string = qvk_get_string(VK_RENDERER_ID);
        vid_printf(PRINT_INFO, &format!("VK_RENDERER: {}\n", cptr_to_str(vk_config.renderer_string)));
        vk_config.version_string = qvk_get_string(VK_VERSION);
        vid_printf(PRINT_INFO, &format!("VK_VERSION: {}\n", cptr_to_str(vk_config.version_string)));
        vk_config.extensions_string = qvk_get_string(VK_EXTENSIONS);
        vid_printf(PRINT_INFO, &format!("VK_EXTENSIONS: {}\n", cptr_to_str(vk_config.extensions_string)));

        let renderer_buffer = cptr_to_str(vk_config.renderer_string).to_lowercase();
        let vendor_buffer = cptr_to_str(vk_config.vendor_string).to_lowercase();

        if renderer_buffer.contains("voodoo") {
            if !renderer_buffer.contains("rush") {
                vk_config.renderer = VK_RENDERER_VOODOO_C as i32;
            } else {
                vk_config.renderer = VK_RENDERER_VOODOO_RUSH as i32;
            }
        } else if vendor_buffer.contains("sgi") {
            vk_config.renderer = VK_RENDERER_SGI as i32;
        } else if renderer_buffer.contains("permedia") {
            vk_config.renderer = VK_RENDERER_PERMEDIA2 as i32;
        } else if renderer_buffer.contains("glint") {
            vk_config.renderer = VK_RENDERER_GLINT_MX as i32;
        } else if renderer_buffer.contains("glzicd") {
            vk_config.renderer = VK_RENDERER_REALIZM as i32;
        } else if renderer_buffer.contains("gdi") {
            vk_config.renderer = VK_RENDERER_MCD_C as i32;
        } else if renderer_buffer.contains("pcx2") {
            vk_config.renderer = VK_RENDERER_PCX2 as i32;
        } else if renderer_buffer.contains("verite") {
            vk_config.renderer = VK_RENDERER_RENDITION_C as i32;
        } else {
            vk_config.renderer = VK_RENDERER_OTHER as i32;
        }

        // monolightmap handling
        if VK_MONOLIGHTMAP.string.len() > 1
            && !VK_MONOLIGHTMAP.string.as_bytes()[1].eq_ignore_ascii_case(&b'F')
        {
            if vk_config.renderer == VK_RENDERER_PERMEDIA2 as i32 {
                cvar_set("vk_monolightmap", "A");
                vid_printf(PRINT_INFO, "...using vk_monolightmap 'a'\n");
            } else {
                cvar_set("vk_monolightmap", "0");
            }
        }

        // power vr framebuffer
        if vk_config.renderer as u32 & VK_RENDERER_POWERVR != 0 {
            cvar_set("scr_drawall", "1");
        } else {
            cvar_set("scr_drawall", "0");
        }

        // MCD has buffering issues
        if vk_config.renderer == VK_RENDERER_MCD_C as i32 {
            cvar_set_value("vk_finish", 1.0);
        }

        if vk_config.renderer as u32 & VK_RENDERER_3DLABS != 0 {
            if VK_3DLABS_BROKEN.value != 0.0 {
                vk_config.allow_cds = 0;
            } else {
                vk_config.allow_cds = 1;
            }
        } else {
            vk_config.allow_cds = 1;
        }

        if vk_config.allow_cds != 0 {
            vid_printf(PRINT_INFO, "...allowing CDS\n");
        } else {
            vid_printf(PRINT_INFO, "...disabling CDS\n");
        }

        // grab extensions
        if cptr_to_str(vk_config.extensions_string).contains("GL_EXT_compiled_vertex_array")
            || cptr_to_str(vk_config.extensions_string).contains("GL_SGI_compiled_vertex_array")
        {
            vid_printf(PRINT_INFO, "...enabling GL_EXT_compiled_vertex_array\n");
            // Legacy GL extension detection - no longer used with SDL3 GPU backend.
        } else {
            vid_printf(PRINT_INFO, "...GL_EXT_compiled_vertex_array not found\n");
        }

        // WGL_EXT_swap_control
        if cptr_to_str(vk_config.extensions_string).contains("WGL_EXT_swap_control") {
            vid_printf(PRINT_INFO, "...enabling WGL_EXT_swap_control\n");
        } else {
            vid_printf(PRINT_INFO, "...WGL_EXT_swap_control not found\n");
        }

        // GL_ARB_multitexture
        if cptr_to_str(vk_config.extensions_string).contains("GL_ARB_multitexture") {
            if VK_EXT_MULTITEXTURE.value != 0.0 {
                vid_printf(PRINT_INFO, "...using GL_ARB_multitexture\n");
                VK_TEXTURE0_ID = VK_TEXTURE0_ARB;
                VK_TEXTURE1_ID = VK_TEXTURE1_ARB;
                VK_TEXTURE2_ID = VK_TEXTURE2_ARB;
                VK_TEXTURE3_ID = VK_TEXTURE3_ARB;
            } else {
                vid_printf(PRINT_INFO, "...ignoring GL_ARB_multitexture\n");
            }
        } else {
            vid_printf(PRINT_INFO, "...GL_ARB_multitexture not found\n");
        }

        // GL_SGIS_multitexture (fallback)
        if cptr_to_str(vk_config.extensions_string).contains("GL_SGIS_multitexture") {
            if VK_TEXTURE0_ID != 0 {
                // ARB already loaded
                vid_printf(PRINT_INFO, "...GL_SGIS_multitexture deprecated in favor of ARB_multitexture\n");
            } else if VK_EXT_MULTITEXTURE.value != 0.0 {
                vid_printf(PRINT_INFO, "...using GL_SGIS_multitexture\n");
                VK_TEXTURE0_ID = VK_TEXTURE0_SGIS;
                VK_TEXTURE1_ID = VK_TEXTURE1_SGIS;
                VK_TEXTURE2_ID = VK_TEXTURE2_SGIS;
                VK_TEXTURE3_ID = VK_TEXTURE3_SGIS;
            } else {
                vid_printf(PRINT_INFO, "...ignoring GL_SGIS_multitexture\n");
            }
        } else {
            vid_printf(PRINT_INFO, "...GL_SGIS_multitexture not found\n");
        }

        // Vic - texture env combine
        vk_config.mtexcombine = 0;
        if cptr_to_str(vk_config.extensions_string).contains("GL_ARB_texture_env_combine") {
            if R_OVERBRIGHTBITS.value != 0.0 {
                vid_printf(PRINT_INFO, "...using GL_ARB_texture_env_combine\n");
                vk_config.mtexcombine = 1;
            } else {
                vid_printf(PRINT_INFO, "...ignoring GL_ARB_texture_env_combine\n");
            }
        } else {
            vid_printf(PRINT_INFO, "...GL_ARB_texture_env_combine not found\n");
        }

        if vk_config.mtexcombine == 0 {
            if cptr_to_str(vk_config.extensions_string).contains("GL_EXT_texture_env_combine") {
                if R_OVERBRIGHTBITS.value != 0.0 {
                    vid_printf(PRINT_INFO, "...using GL_EXT_texture_env_combine\n");
                    vk_config.mtexcombine = 1;
                } else {
                    vid_printf(PRINT_INFO, "...ignoring GL_EXT_texture_env_combine\n");
                }
            } else {
                vid_printf(PRINT_INFO, "...GL_EXT_texture_env_combine not found\n");
            }
        }

        // anisotropy
        vk_config.anisotropy = 0;
        if cptr_to_str(vk_config.extensions_string).contains("GL_EXT_texture_filter_anisotropic") {
            if VK_EXT_TEXTURE_FILTER_ANISOTROPIC.value != 0.0 {
                vk_config.anisotropy = 1;
                vid_printf(PRINT_INFO, "...using GL_EXT_texture_filter_anisotropic\n");
            } else {
                vid_printf(PRINT_INFO, "...ignoring GL_EXT_texture_filter_anisotropic\n");
            }
        } else {
            vid_printf(PRINT_INFO, "...GL_EXT_texture_filter_anisotropic not found\n");
        }

        // SGIS generate mipmap
        vk_config.sgismipmap = 0;
        if cptr_to_str(vk_config.extensions_string).contains("GL_SGIS_generate_mipmap") {
            if VK_SGIS_GENERATE_MIPMAP.value != 0.0 {
                vk_config.sgismipmap = 1;
                vid_printf(PRINT_INFO, "...using GL_SGIS_generate_mipmap\n");
            } else {
                vid_printf(PRINT_INFO, "...ignoring GL_SGIS_generate_mipmap\n");
            }
        } else {
            vid_printf(PRINT_INFO, "...GL_SGIS_generate_mipmap not found\n");
        }

        // retrieve information
        qvk_get_integerv(VK_MAX_TEXTURE_MAX_ANISOTROPY_EXT, &mut MAX_ANISO);
        qvk_get_integerv(VK_MAX_TEXTURE_SIZE, &mut MAX_TSIZE);
        qvk_get_integerv(VK_MAX_TEXTURE_UNITS, &mut vk_state.num_tmu);
        power_of_two(&mut MAX_TSIZE);

        // display information
        vid_printf(PRINT_INFO, "---------- OpenGL Queries ----------\n");
        vid_printf(PRINT_INFO, &format!("Maximum Anisotropy: {}\n", MAX_ANISO));
        vid_printf(PRINT_INFO, &format!("Maximum Texture Size: {}x{}\n", MAX_TSIZE, MAX_TSIZE));
        vid_printf(PRINT_INFO, &format!("Maximum TMU: {}\n", vk_state.num_tmu));

        crate::vk_rmisc::vk_set_default_state();

        vk_init_images();
        mod_init();
        crate::vk_rmisc::r_init_particle_texture();
        draw_init_local();

        // Initialize modern VBO/shader-based renderer
        let mut modern = ModernRenderPath::new();
        modern.set_dimensions(VID.width as u32, VID.height as u32);
        match modern.init() {
            Ok(()) => vid_printf(PRINT_ALL, "Modern renderer initialized\n"),
            Err(e) => vid_printf(PRINT_ALL, &format!("Modern renderer init failed: {}\n", e)),
        }
        MODERN = Some(modern);

        let err = qvk_get_error();
        if err != VK_NO_ERROR {
            vid_printf(PRINT_ALL, &format!("glGetError() = 0x{:x}\n", err));
        }
    }
    0 // success
}

// ============================================================
// R_Shutdown
// ============================================================
pub fn r_shutdown() {
    cmd_remove_command("modellist");
    cmd_remove_command("screenshot");
    cmd_remove_command("imagelist");
    cmd_remove_command("vk_strings");

    // SAFETY: single-threaded engine shutdown sequence
    unsafe {
        // Shutdown modern renderer before releasing GL context
        if let Some(ref mut m) = MODERN {
            m.shutdown();
        }
        MODERN = None;

        mod_free_all();
        vk_shutdown_images();
    }
    glimp_shutdown();
    qvk_shutdown();
}

// ============================================================
// R_BeginFrame
// ============================================================
pub fn r_begin_frame(camera_separation: f32) {
    // SAFETY: single-threaded engine access pattern
    unsafe {
        let vk_state = VK_STATE.get_or_insert_with(VkState::default);
        vk_state.camera_separation = camera_separation;

        // change modes if necessary
        if VK_MODE.modified || VID_FULLSCREEN.modified {
            cbuf_add_text("vid_restart\n");
        }

        if VK_LOG.modified {
            glimp_enable_logging(VK_LOG.value);
            VK_LOG.modified = false;
        }

        if VK_LOG.value != 0.0 {
            glimp_log_new_frame();
        }

        // Update gamma: hardware ramp (r_hwgamma) or shader gamma (postprocess)
        if VID_GAMMA.modified {
            VID_GAMMA.modified = false;

            let vk_config = VK_CONFIG.get_or_insert_with(VkConfig::default);
            // Apply hardware gamma ramp when r_hwgamma is enabled and platform supports it
            if R_HWGAMMA.value != 0.0 && vk_config.gammaramp != 0 {
                update_gamma_ramp();
            }
            // When r_hwgamma is disabled, gamma is applied in the postprocess shader
            // (handled by the polyblend+gamma pass in PostProcessor)
        }

        // Update MSAA and anisotropy settings if cvars changed
        if R_MSAA.modified || R_ANISOTROPY.modified {
            let msaa_changed = R_MSAA.modified;
            R_MSAA.modified = false;
            R_ANISOTROPY.modified = false;
            crate::modern::gpu_device::with_device(|ctx| {
                crate::vulkan::update_render_config(ctx, R_MSAA.value as i32, R_ANISOTROPY.value as i32);
            });
            // MSAA changes require pipeline recreation (vid_restart)
            if msaa_changed {
                cbuf_add_text("vid_restart\n");
            }
        }

        glimp_begin_frame(camera_separation);

        // go into 2D mode
        qvk_viewport(0, 0, VID.width as i32, VID.height as i32);
        qvk_matrix_mode(VK_PROJECTION);
        qvk_load_identity();
        qvk_ortho(0.0, VID.width as f64, VID.height as f64, 0.0, -99999.0, 99999.0);
        qvk_matrix_mode(VK_MODELVIEW);
        qvk_load_identity();
        qvk_disable(VK_DEPTH_TEST);
        qvk_disable(VK_CULL_FACE);
        qvk_disable(VK_BLEND);
        // GL_ALPHA_TEST enable removed — alpha testing handled by GLSL discard
        qvk_color4f(1.0, 1.0, 1.0, 1.0);

        // draw buffer stuff
        if VK_DRAWBUFFER.modified {
            VK_DRAWBUFFER.modified = false;

            if vk_state.camera_separation == 0.0 || vk_state.stereo_enabled == 0 {
                if VK_DRAWBUFFER.string.eq_ignore_ascii_case("VK_FRONT") {
                    qvk_draw_buffer(VK_FRONT);
                } else {
                    qvk_draw_buffer(VK_BACK);
                }
            }
        }

        // texturemode stuff
        if VK_TEXTUREMODE.modified {
            vk_texture_mode(VK_TEXTUREMODE.string);
            VK_TEXTUREMODE.modified = false;
        }

        if VK_TEXTUREALPHAMODE.modified {
            vk_texture_alpha_mode(VK_TEXTUREALPHAMODE.string);
            VK_TEXTUREALPHAMODE.modified = false;
        }

        if VK_TEXTURESOLIDMODE.modified {
            vk_texture_solid_mode(VK_TEXTURESOLIDMODE.string);
            VK_TEXTURESOLIDMODE.modified = false;
        }

        // swapinterval stuff
        crate::vk_rmisc::vk_update_swap_interval();

        // clear screen if desired
        r_clear();
    }
}

// ============================================================
// R_SetPalette
// ============================================================
pub fn r_set_palette(palette: Option<&[u8]>) {
    // SAFETY: single-threaded engine access pattern
    unsafe {
        let rp = &mut crate::vk_image::r_rawpalette;
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
                for i in 0..256 {
                    let c = crate::vk_image::d_8to24table[i];
                    let r = c & 0xFF;
                    let g = (c >> 8) & 0xFF;
                    let b = (c >> 16) & 0xFF;
                    rp[i] = r | (g << 8) | (b << 16) | (0xFF << 24);
                }
            }
        }

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


