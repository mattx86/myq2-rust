// qvk.rs -- Quake Vulkan constants and legacy type definitions
// Originally converted from myq2-original/ref_gl/qgl.h, now migrated to Vulkan 1.3.
//
// This file contains:
// - Legacy type aliases (retained for compatibility with existing code)
// - Internal format/mode constants (renamed from GL_ to VK_ prefix)
// - Legacy function pointer struct (retained for compatibility, resolves to no-ops)
//
// The actual Vulkan API is accessed through ash in the vulkan/ module.

#![allow(non_snake_case, dead_code, clippy::too_many_arguments)]

use std::ffi::CStr;
use std::os::raw::{c_int, c_void};

// Legacy type aliases (retained for code compatibility)
pub type GLenum = u32;
pub type GLboolean = u8;
pub type GLbitfield = u32;
pub type GLvoid = c_void;
pub type GLbyte = i8;
pub type GLshort = i16;
pub type GLint = i32;
pub type GLubyte = u8;
pub type GLushort = u16;
pub type GLuint = u32;
pub type GLsizei = i32;
pub type GLfloat = f32;
pub type GLclampf = f32;
pub type GLdouble = f64;
pub type GLclampd = f64;

// ==============================
// Internal format/mode constants
// (Legacy GL values with VK_ prefix)
// ==============================

pub const VK_FALSE: GLboolean = 0;
pub const VK_TRUE: GLboolean = 1;

// Primitive types
pub const VK_POINTS: GLenum = 0x0000;
pub const VK_LINES: GLenum = 0x0001;
pub const VK_LINE_LOOP: GLenum = 0x0002;
pub const VK_LINE_STRIP: GLenum = 0x0003;
pub const VK_TRIANGLES: GLenum = 0x0004;
pub const VK_TRIANGLE_STRIP: GLenum = 0x0005;
pub const VK_TRIANGLE_FAN: GLenum = 0x0006;

// Blending factors
pub const VK_ZERO: GLenum = 0;
pub const VK_ONE: GLenum = 1;
pub const VK_SRC_COLOR: GLenum = 0x0300;
pub const VK_ONE_MINUS_SRC_COLOR: GLenum = 0x0301;
pub const VK_SRC_ALPHA: GLenum = 0x0302;
pub const VK_ONE_MINUS_SRC_ALPHA: GLenum = 0x0303;
pub const VK_DST_ALPHA: GLenum = 0x0304;
pub const VK_ONE_MINUS_DST_ALPHA: GLenum = 0x0305;
pub const VK_DST_COLOR: GLenum = 0x0306;
pub const VK_ONE_MINUS_DST_COLOR: GLenum = 0x0307;
pub const VK_SRC_ALPHA_SATURATE: GLenum = 0x0308;

// Enable/disable caps
pub const VK_TEXTURE_2D: GLenum = 0x0DE1;
pub const VK_BLEND: GLenum = 0x0BE2;
pub const VK_DEPTH_TEST: GLenum = 0x0B71;
pub const VK_CULL_FACE: GLenum = 0x0B44;
// GL_ALPHA_TEST removed — alpha testing handled by GLSL discard
pub const VK_STENCIL_TEST: GLenum = 0x0B90;
// GL_FOG removed — fog handled by GLSL shaders
pub const VK_SCISSOR_TEST: GLenum = 0x0C11;
pub const VK_COLOR_MATERIAL: GLenum = 0x0B57;

// Texture parameters
pub const VK_TEXTURE_MIN_FILTER: GLenum = 0x2801;
pub const VK_TEXTURE_MAG_FILTER: GLenum = 0x2800;
pub const VK_TEXTURE_WRAP_S: GLenum = 0x2802;
pub const VK_TEXTURE_WRAP_T: GLenum = 0x2803;
pub const VK_TEXTURE_ENV: GLenum = 0x2300;
pub const VK_TEXTURE_ENV_MODE: GLenum = 0x2200;

// Texture filter modes
pub const VK_NEAREST: GLenum = 0x2600;
pub const VK_LINEAR: GLenum = 0x2601;
pub const VK_NEAREST_MIPMAP_NEAREST: GLenum = 0x2700;
pub const VK_LINEAR_MIPMAP_NEAREST: GLenum = 0x2701;
pub const VK_NEAREST_MIPMAP_LINEAR: GLenum = 0x2702;
pub const VK_LINEAR_MIPMAP_LINEAR: GLenum = 0x2703;

// Texture wrap modes
pub const VK_REPEAT: GLenum = 0x2901;
pub const VK_CLAMP: GLenum = 0x2900;
pub const VK_CLAMP_TO_EDGE: GLenum = 0x812F;

// Texture env modes
pub const VK_MODULATE: GLenum = 0x2100;
pub const VK_DECAL: GLenum = 0x2101;
pub const VK_REPLACE: GLenum = 0x1E01;
pub const VK_ADD: GLenum = 0x0104;

// Pixel formats
pub const VK_RGB: GLenum = 0x1907;
pub const VK_RGBA: GLenum = 0x1908;
pub const VK_LUMINANCE: GLenum = 0x1909;
pub const VK_ALPHA: GLenum = 0x1906;
pub const VK_RGB8: GLenum = 0x8051;
pub const VK_RGBA8: GLenum = 0x8058;
pub const VK_RGB5_A1: GLenum = 0x8057;
pub const VK_RGBA4: GLenum = 0x8056;
pub const VK_RGBA2: GLenum = 0x8055;
pub const VK_RGB5: GLenum = 0x8D62;
pub const VK_RGB4: GLenum = 0x804F;
pub const VK_R3_G3_B2: GLenum = 0x2A10;
pub const VK_INTENSITY8: GLenum = 0x804B;
pub const VK_LUMINANCE8: GLenum = 0x8040;

// Data types
pub const VK_UNSIGNED_BYTE: GLenum = 0x1401;
pub const VK_BYTE: GLenum = 0x1400;
pub const VK_UNSIGNED_SHORT: GLenum = 0x1403;
pub const VK_SHORT: GLenum = 0x1402;
pub const VK_UNSIGNED_INT: GLenum = 0x1405;
pub const VK_INT: GLenum = 0x1404;
pub const VK_FLOAT: GLenum = 0x1406;

// Clear buffer bits
pub const VK_COLOR_BUFFER_BIT: GLbitfield = 0x00004000;
pub const VK_DEPTH_BUFFER_BIT: GLbitfield = 0x00000100;
pub const VK_STENCIL_BUFFER_BIT: GLbitfield = 0x00000400;

// Matrix mode
pub const VK_MODELVIEW: GLenum = 0x1700;
pub const VK_PROJECTION: GLenum = 0x1701;
pub const VK_TEXTURE: GLenum = 0x1702;

// Depth function
pub const VK_LEQUAL: GLenum = 0x0203;
pub const VK_GEQUAL: GLenum = 0x0206;
pub const VK_LESS: GLenum = 0x0201;
pub const VK_GREATER: GLenum = 0x0204;
pub const VK_EQUAL: GLenum = 0x0202;
pub const VK_NOTEQUAL: GLenum = 0x0205;
pub const VK_ALWAYS: GLenum = 0x0207;
pub const VK_NEVER: GLenum = 0x0200;

// Cull face
pub const VK_FRONT: GLenum = 0x0404;
pub const VK_BACK: GLenum = 0x0405;
pub const VK_FRONT_AND_BACK: GLenum = 0x0408;
pub const VK_CW: GLenum = 0x0900;
pub const VK_CCW: GLenum = 0x0901;

// Draw buffer
pub const VK_BACK_LEFT: GLenum = 0x0402;
pub const VK_BACK_RIGHT: GLenum = 0x0403;

// GetString targets
pub const VK_VENDOR: GLenum = 0x1F00;
pub const VK_RENDERER: GLenum = 0x1F01;
pub const VK_VERSION: GLenum = 0x1F02;
pub const VK_EXTENSIONS: GLenum = 0x1F03;

// Polygon mode
pub const VK_POINT: GLenum = 0x1B00;
pub const VK_LINE: GLenum = 0x1B01;
pub const VK_FILL: GLenum = 0x1B02;

// Client state
pub const VK_VERTEX_ARRAY: GLenum = 0x8074;
pub const VK_TEXTURE_COORD_ARRAY: GLenum = 0x8078;
pub const VK_COLOR_ARRAY: GLenum = 0x8076;

// ==============================
// Extension constants
// ==============================

pub const VK_POINT_SIZE_MIN_EXT: GLenum = 0x8126;
pub const VK_POINT_SIZE_MAX_EXT: GLenum = 0x8127;
pub const VK_POINT_FADE_THRESHOLD_SIZE_EXT: GLenum = 0x8128;
pub const VK_DISTANCE_ATTENUATION_EXT: GLenum = 0x8129;

pub const VK_SHARED_TEXTURE_PALETTE_EXT: GLenum = 0x81FB;

pub const VK_TEXTURE0_SGIS: GLenum = 0x835E;
pub const VK_TEXTURE1_SGIS: GLenum = 0x835F;
pub const VK_TEXTURE2_SGIS: GLenum = 0x8360;
pub const VK_TEXTURE3_SGIS: GLenum = 0x8361;
pub const VK_TEXTURE0_ARB: GLenum = 0x84C0;
pub const VK_TEXTURE1_ARB: GLenum = 0x84C1;
pub const VK_TEXTURE2_ARB: GLenum = 0x84C2;
pub const VK_TEXTURE3_ARB: GLenum = 0x84C3;

pub const VK_GENERATE_MIPMAP_SGIS: GLenum = 0x8191;
pub const VK_TEXTURE_MAX_ANISOTROPY_EXT: GLenum = 0x84FE;
pub const VK_MAX_TEXTURE_MAX_ANISOTROPY_EXT: GLenum = 0x84FF;
pub const VK_MAX_TEXTURE_UNITS: GLenum = 0x84E2;

// EXT_texture_env_combine
pub const VK_COMBINE_EXT: GLenum = 0x8570;
pub const VK_COMBINE_RGB_EXT: GLenum = 0x8571;
pub const VK_COMBINE_ALPHA_EXT: GLenum = 0x8572;
pub const VK_RGB_SCALE_EXT: GLenum = 0x8573;
pub const VK_ADD_SIGNED_EXT: GLenum = 0x8574;
pub const VK_INTERPOLATE_EXT: GLenum = 0x8575;
pub const VK_CONSTANT_EXT: GLenum = 0x8576;
pub const VK_PRIMARY_COLOR_EXT: GLenum = 0x8577;
pub const VK_PREVIOUS_EXT: GLenum = 0x8578;
pub const VK_SOURCE0_RGB_EXT: GLenum = 0x8580;
pub const VK_SOURCE1_RGB_EXT: GLenum = 0x8581;
pub const VK_SOURCE2_RGB_EXT: GLenum = 0x8582;
pub const VK_SOURCE3_RGB_EXT: GLenum = 0x8583;
pub const VK_SOURCE4_RGB_EXT: GLenum = 0x8584;
pub const VK_SOURCE5_RGB_EXT: GLenum = 0x8585;
pub const VK_SOURCE6_RGB_EXT: GLenum = 0x8586;
pub const VK_SOURCE7_RGB_EXT: GLenum = 0x8587;
pub const VK_SOURCE0_ALPHA_EXT: GLenum = 0x8588;
pub const VK_SOURCE1_ALPHA_EXT: GLenum = 0x8589;
pub const VK_SOURCE2_ALPHA_EXT: GLenum = 0x858A;
pub const VK_SOURCE3_ALPHA_EXT: GLenum = 0x858B;
pub const VK_SOURCE4_ALPHA_EXT: GLenum = 0x858C;
pub const VK_SOURCE5_ALPHA_EXT: GLenum = 0x858D;
pub const VK_SOURCE6_ALPHA_EXT: GLenum = 0x858E;
pub const VK_SOURCE7_ALPHA_EXT: GLenum = 0x858F;
pub const VK_OPERAND0_RGB_EXT: GLenum = 0x8590;
pub const VK_OPERAND1_RGB_EXT: GLenum = 0x8591;
pub const VK_OPERAND2_RGB_EXT: GLenum = 0x8592;
pub const VK_OPERAND3_RGB_EXT: GLenum = 0x8593;
pub const VK_OPERAND4_RGB_EXT: GLenum = 0x8594;
pub const VK_OPERAND5_RGB_EXT: GLenum = 0x8595;
pub const VK_OPERAND6_RGB_EXT: GLenum = 0x8596;
pub const VK_OPERAND7_RGB_EXT: GLenum = 0x8597;
pub const VK_OPERAND0_ALPHA_EXT: GLenum = 0x8598;
pub const VK_OPERAND1_ALPHA_EXT: GLenum = 0x8599;
pub const VK_OPERAND2_ALPHA_EXT: GLenum = 0x859A;
pub const VK_OPERAND3_ALPHA_EXT: GLenum = 0x859B;
pub const VK_OPERAND4_ALPHA_EXT: GLenum = 0x859C;
pub const VK_OPERAND5_ALPHA_EXT: GLenum = 0x859D;
pub const VK_OPERAND6_ALPHA_EXT: GLenum = 0x859E;
pub const VK_OPERAND7_ALPHA_EXT: GLenum = 0x859F;

pub const VK_COMBINE_ALPHA_ARB: GLenum = 0x8572;
pub const VK_RGB_SCALE_ARB: GLenum = 0x8573;

/// Dynamic texture unit variables (set at runtime based on extension availability)
pub struct GlTextureUnits {
    pub vk_texture0: GLenum,
    pub vk_texture1: GLenum,
    pub vk_texture2: GLenum,
    pub vk_texture3: GLenum,
}

impl Default for GlTextureUnits {
    fn default() -> Self {
        Self {
            vk_texture0: VK_TEXTURE0_ARB,
            vk_texture1: VK_TEXTURE1_ARB,
            vk_texture2: VK_TEXTURE2_ARB,
            vk_texture3: VK_TEXTURE3_ARB,
        }
    }
}

/// QGL function pointers struct -- legacy OpenGL function pointers.
///
/// These are retained for compatibility with legacy code paths that haven't been
/// fully removed. The modern renderer uses SDL3 GPU API instead. All function
/// pointers resolve to no-op stubs in vk_bindings.rs.
///
/// Fields are `Option<fn(...)>` so they can be `None` when the GL library is not loaded.
#[allow(non_snake_case)]
pub struct QglFunctions {
    // ------------------------------------------------------------------
    // Legacy GL function pointers (all resolve to no-op stubs)
    // ------------------------------------------------------------------
    pub qglBegin: Option<unsafe extern "system" fn(GLenum)>,
    pub qglBindTexture: Option<unsafe extern "system" fn(GLenum, GLuint)>,
    pub qglClear: Option<unsafe extern "system" fn(GLbitfield)>,
    pub qglClearColor: Option<unsafe extern "system" fn(GLclampf, GLclampf, GLclampf, GLclampf)>,
    pub qglCopyTexSubImage2D: Option<unsafe extern "system" fn(GLenum, GLint, GLint, GLint, GLint, GLint, GLsizei, GLsizei)>,
    pub qglEnd: Option<unsafe extern "system" fn()>,
    pub qglGenTextures: Option<unsafe extern "system" fn(GLsizei, *mut GLuint)>,
    pub qglGetFloatv: Option<unsafe extern "system" fn(GLenum, *mut GLfloat)>,
    pub qglLoadIdentity: Option<unsafe extern "system" fn()>,
    pub qglMatrixMode: Option<unsafe extern "system" fn(GLenum)>,
    pub qglMultMatrixd: Option<unsafe extern "system" fn(*const GLdouble)>,
    pub qglPixelStorei: Option<unsafe extern "system" fn(GLenum, GLint)>,
    pub qglRotatef: Option<unsafe extern "system" fn(GLfloat, GLfloat, GLfloat, GLfloat)>,
    pub qglScalef: Option<unsafe extern "system" fn(GLfloat, GLfloat, GLfloat)>,
    pub qglTexCoord2f: Option<unsafe extern "system" fn(GLfloat, GLfloat)>,
    pub qglTexImage2D: Option<unsafe extern "system" fn(GLenum, GLint, GLint, GLsizei, GLsizei, GLint, GLenum, GLenum, *const GLvoid)>,
    pub qglTexParameteri: Option<unsafe extern "system" fn(GLenum, GLenum, GLint)>,
    pub qglTranslatef: Option<unsafe extern "system" fn(GLfloat, GLfloat, GLfloat)>,
    pub qglVertex3f: Option<unsafe extern "system" fn(GLfloat, GLfloat, GLfloat)>,

    // ------------------------------------------------------------------
    // Extension functions
    // ------------------------------------------------------------------
    pub qglPointParameterfEXT: Option<unsafe extern "system" fn(GLenum, GLfloat)>,
    pub qglPointParameterfvEXT: Option<unsafe extern "system" fn(GLenum, *const GLfloat)>,
    pub qglColorTableEXT: Option<unsafe extern "system" fn(c_int, c_int, c_int, c_int, c_int, *const c_void)>,

    pub qglLockArraysEXT: Option<unsafe extern "system" fn(c_int, c_int)>,
    pub qglUnlockArraysEXT: Option<unsafe extern "system" fn()>,

    pub qglMTexCoord2fSGIS: Option<unsafe extern "system" fn(GLenum, GLfloat, GLfloat)>,
    pub qglSelectTextureSGIS: Option<unsafe extern "system" fn(GLenum)>,

    pub qglActiveTextureARB: Option<unsafe extern "system" fn(GLenum)>,
    pub qglClientActiveTextureARB: Option<unsafe extern "system" fn(GLenum)>,
}

impl QglFunctions {
    /// Create a new QglFunctions with all pointers set to None.
    pub fn new() -> Self {
        // SAFETY: All fields are Option types, and None is represented as all zeros
        // for Option<fn(...)> on all platforms Rust supports.
        // However, we initialize explicitly for clarity.
        unsafe { std::mem::zeroed() }
    }

    /// Returns true if the core GL functions appear to be loaded.
    pub fn is_loaded(&self) -> bool {
        self.qglBegin.is_some() && self.qglEnd.is_some() && self.qglClear.is_some()
    }

    /// Clear all function pointers (set to None). Called on QGL_Shutdown.
    pub fn shutdown(&mut self) {
        *self = Self::new();
    }
}

impl Default for QglFunctions {
    fn default() -> Self {
        Self::new()
    }
}

// Win32-specific WGL function pointers
#[cfg(target_os = "windows")]
pub mod wgl {
    use std::os::raw::c_void;

    /// WGL function pointers for Windows OpenGL context management.
    /// These correspond to the qwgl* function pointers in the original qvk.h.
    #[allow(non_snake_case)]
    pub struct WglFunctions {
        // Using opaque pointer types since we don't want to pull in winapi types here.
        // At the platform layer these will be cast to the correct types.
        pub qwglChoosePixelFormat: Option<unsafe extern "system" fn(isize, *const c_void) -> i32>,
        pub qwglDescribePixelFormat: Option<unsafe extern "system" fn(isize, i32, u32, *mut c_void) -> i32>,
        pub qwglGetPixelFormat: Option<unsafe extern "system" fn(isize) -> i32>,
        pub qwglSetPixelFormat: Option<unsafe extern "system" fn(isize, i32, *const c_void) -> i32>,
        pub qwglSwapBuffers: Option<unsafe extern "system" fn(isize) -> i32>,
        pub qwglCopyContext: Option<unsafe extern "system" fn(isize, isize, u32) -> i32>,
        pub qwglCreateContext: Option<unsafe extern "system" fn(isize) -> isize>,
        pub qwglCreateLayerContext: Option<unsafe extern "system" fn(isize, i32) -> isize>,
        pub qwglDeleteContext: Option<unsafe extern "system" fn(isize) -> i32>,
        pub qwglGetCurrentContext: Option<unsafe extern "system" fn() -> isize>,
        pub qwglGetCurrentDC: Option<unsafe extern "system" fn() -> isize>,
        pub qwglGetProcAddress: Option<unsafe extern "system" fn(*const i8) -> *const c_void>,
        pub qwglMakeCurrent: Option<unsafe extern "system" fn(isize, isize) -> i32>,
        pub qwglShareLists: Option<unsafe extern "system" fn(isize, isize) -> i32>,
        pub qwglUseFontBitmaps: Option<unsafe extern "system" fn(isize, u32, u32, u32) -> i32>,
        pub qwglSwapIntervalEXT: Option<unsafe extern "system" fn(i32) -> i32>,
        pub qwglGetDeviceGammaRampEXT: Option<unsafe extern "system" fn(*mut u8, *mut u8, *mut u8) -> i32>,
        pub qwglSetDeviceGammaRampEXT: Option<unsafe extern "system" fn(*const u8, *const u8, *const u8) -> i32>,
    }

    impl WglFunctions {
        pub fn new() -> Self {
            // SAFETY: All fields are Option types; zeroed memory is valid (all None).
            unsafe { std::mem::zeroed() }
        }
    }

    impl Default for WglFunctions {
        fn default() -> Self {
            Self::new()
        }
    }
}

/// Initialize QVK (Quake Vulkan) bindings.
/// Corresponds to `QGL_Init` in the original code.
///
/// In the Vulkan 1.3 port, initialization is handled by the VulkanContext
/// which creates the instance, device, and loads all Vulkan functions via ash.
/// This function is a compatibility stub that returns true.
pub fn qvk_init(_dllname: &CStr) -> bool {
    // Vulkan initialization is now handled by VulkanContext::new() in the
    // vulkan module. This stub exists for compatibility with legacy code paths.
    true
}

/// Legacy function pointer loading stub.
///
/// In the original OpenGL code, this loaded all GL function pointers via
/// wglGetProcAddress/SDL_GL_GetProcAddress. In the Vulkan 1.3 port, all
/// Vulkan functions are loaded automatically by ash when creating the
/// VulkanContext. This stub is retained for API compatibility.
pub fn qvk_load_vk_bindings<F>(mut loader: F)
where
    F: FnMut(&str) -> *const std::os::raw::c_void,
{
    // No-op: Vulkan functions are loaded by ash, not via manual proc address loading.
    unsafe { crate::vk_bindings::load_with(|s| loader(s)); }
}

/// Shutdown QGL, clearing all function pointers.
/// Corresponds to `QGL_Shutdown` in the original code.
pub fn qvk_shutdown(qvk: &mut QglFunctions) {
    qvk.shutdown();
}
