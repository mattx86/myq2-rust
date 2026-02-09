// qvk_win.rs -- Converted from myq2-original/win32/qvk_win.c
// QGL: Operating system binding of GL to function pointers.
//
// The original file defines ~350 GL function pointer globals and implements
// QGL_Init() (LoadLibrary + GetProcAddress for every GL function) and
// QGL_Shutdown() (FreeLibrary + NULL all pointers).
//
// In the Rust port, GL function loading is handled via SDL2's
// `vk_get_proc_address` combined with the `gl` crate. This module
// provides the same public interface (QGL_Init / QGL_Shutdown) as stubs,
// plus re-exports of WGL function pointer types for compatibility.

#![allow(dead_code)]

use myq2_common::common::com_printf;

// =============================================================================
// WGL function pointer placeholders
// (In the original, these are WINAPI function pointers loaded from opengl32.dll)
// =============================================================================

/// Placeholder for all WGL function pointers.
/// In a real implementation, these would be loaded via SDL2's vk_get_proc_address.
#[derive(Default)]
pub struct WglFunctions {
    pub choose_pixel_format: Option<fn()>,
    pub describe_pixel_format: Option<fn()>,
    pub get_pixel_format: Option<fn()>,
    pub set_pixel_format: Option<fn()>,
    pub swap_buffers: Option<fn()>,
    pub copy_context: Option<fn()>,
    pub create_context: Option<fn()>,
    pub create_layer_context: Option<fn()>,
    pub delete_context: Option<fn()>,
    pub get_current_context: Option<fn()>,
    pub get_current_dc: Option<fn()>,
    pub get_proc_address: Option<fn()>,
    pub make_current: Option<fn()>,
    pub share_lists: Option<fn()>,
    pub use_font_bitmaps: Option<fn()>,
    pub use_font_outlines: Option<fn()>,
    pub describe_layer_plane: Option<fn()>,
    pub set_layer_palette_entries: Option<fn()>,
    pub get_layer_palette_entries: Option<fn()>,
    pub realize_layer_palette: Option<fn()>,
    pub swap_layer_buffers: Option<fn()>,
}


// =============================================================================
// QGL state
// =============================================================================

#[derive(Default)]
pub struct QglState {
    pub wgl: WglFunctions,
    pub vk_loaded: bool,
    /// Handle to the loaded OpenGL library (0 = not loaded)
    pub hinst_opengl: usize,
    /// Whether logging is enabled
    pub log_active: bool,
}


impl QglState {
    // =========================================================================
    // QGL_Shutdown
    //
    // Unloads the specified DLL then nulls out all the proc pointers.
    // Original: FreeLibrary(glw_state.hinstOpenGL), then sets all ~350
    // qvk* pointers to NULL.
    // =========================================================================

    pub fn qvk_shutdown(&mut self) {
        if self.hinst_opengl != 0 {
            // FreeLibrary equivalent
            self.hinst_opengl = 0;
        }

        self.vk_loaded = false;
        self.wgl = WglFunctions::default();

        // In the original, this also NULLs out every single qvk* function
        // pointer (qglAccum, qglAlphaFunc, qglBegin, ... ~350 of them).
        // With the `gl` crate approach, this is handled by dropping the
        // GL context.
    }

    // =========================================================================
    // QGL_Init
    //
    // Loads the GL DLL and resolves all function pointers.
    // Original: LoadLibrary(dllname), then GetProcAddress for every GL
    // function, plus wglChoosePixelFormat, wglCreateContext, etc.
    //
    // In the Rust port, GL loading is handled by the `gl` crate or
    // SDL2's `vk_get_proc_address`. This stub just records that GL is loaded.
    // =========================================================================

    pub fn qvk_init(&mut self, dllname: &str) -> bool {
        com_printf(&format!("QGL_Init: loading '{}' (stub -- using Rust GL loader)\n", dllname));

        // Original updates 3Dfx gamma env var:
        // {
        //     char envbuffer[1024];
        //     float g = (1.3 - vid_gamma->value + 1);
        //     snprintf(envbuffer, sizeof(envbuffer), "SSTV2_GAMMA=%f", g);
        //     putenv(envbuffer);
        //     snprintf(envbuffer, sizeof(envbuffer), "SST_GAMMA=%f", g);
        //     putenv(envbuffer);
        // }

        // Simulate LoadLibrary
        self.hinst_opengl = 1; // non-zero = loaded

        // In the real implementation, every GL function would be loaded via
        // GetProcAddress. With Rust's `gl` crate:
        //   gl::load_with(|s| vk_get_proc_address(s));
        // For now, just mark as loaded.

        // Load WGL functions (placeholders)
        // Original loads: wglChoosePixelFormat, wglDescribePixelFormat,
        // wglGetPixelFormat, wglSetPixelFormat, wglSwapBuffers,
        // wglCopyContext, wglCreateContext, wglCreateLayerContext,
        // wglDeleteContext, wglGetCurrentContext, wglGetCurrentDC,
        // wglGetProcAddress, wglMakeCurrent, wglShareLists,
        // wglUseFontBitmaps, wglUseFontOutlines, wglDescribeLayerPlane,
        // wglSetLayerPaletteEntries, wglGetLayerPaletteEntries,
        // wglRealizeLayerPalette, wglSwapLayerBuffers

        // Load all ~350 qvk* GL functions via GPA macro
        // (qglAccum, qglAlphaFunc, qglAreTexturesResident, qglArrayElement,
        //  qglBegin, qglBindTexture, qglBitmap, qglBlendFunc, qglCallList,
        //  qglCallLists, qglClear, qglClearAccum, qglClearColor,
        //  qglClearDepth, qglClearIndex, qglClearStencil, qglClipPlane,
        //  qglColor3b..qglColor4usv, qglColorMask, qglColorMaterial,
        //  qglColorPointer, qglCopyPixels, qglCopyTexImage1D/2D,
        //  qglCopyTexSubImage1D/2D, qglCullFace, qglDeleteLists,
        //  qglDeleteTextures, qglDepthFunc, qglDepthMask, qglDepthRange,
        //  qglDisable, qglDisableClientState, qglDrawArrays, qglDrawBuffer,
        //  qglDrawElements, qglDrawPixels, qglEdgeFlag*, qglEnable,
        //  qglEnableClientState, qglEnd, qglEndList, qglEvalCoord*,
        //  qglEvalMesh*, qglEvalPoint*, qglFeedbackBuffer, qglFinish,
        //  qglFlush, qglFog*, qglFrontFace, qglFrustum, qglGenLists,
        //  qglGenTextures, qglGet*, qglHint, qglIndex*, qglInitNames,
        //  qglInterleavedArrays, qglIs*, qglLightModel*, qglLight*,
        //  qglLineStipple, qglLineWidth, qglListBase, qglLoadIdentity,
        //  qglLoadMatrix*, qglLoadName, qglLogicOp, qglMap*, qglMapGrid*,
        //  qglMaterial*, qglMatrixMode, qglMultMatrix*, qglNewList,
        //  qglNormal3*, qglNormalPointer, qglOrtho, qglPassThrough,
        //  qglPixelMap*, qglPixelStore*, qglPixelTransfer*, qglPixelZoom,
        //  qglPointSize, qglPolygonMode, qglPolygonOffset,
        //  qglPolygonStipple, qglPop*, qglPrioritizeTextures, qglPush*,
        //  qglRasterPos*, qglReadBuffer, qglReadPixels, qglRect*,
        //  qglRenderMode, qglRotate*, qglScale*, qglScissor,
        //  qglSelectBuffer, qglShadeModel, qglStencilFunc, qglStencilMask,
        //  qglStencilOp, qglTexCoord*, qglTexCoordPointer, qglTexEnv*,
        //  qglTexGen*, qglTexImage1D/2D, qglTexParameter*,
        //  qglTexSubImage1D/2D, qglTranslate*, qglVertex2*/3*/4*,
        //  qglVertexPointer, qglViewport)
        //
        // All handled by `gl::load_with()` in the real Rust implementation.

        self.vk_loaded = true;

        com_printf("QGL_Init: GL functions loaded (stub)\n");
        true
    }

    // =========================================================================
    // QGL logging (GLimp_EnableLogging / GLimp_DisableLogging)
    // =========================================================================

    pub fn glimp_enable_logging(&mut self, _enable: bool) {
        // Original: opens/closes vk_log.txt and replaces all qvk* pointers
        // with logging wrappers (e.g. logAlphaFunc, logBegin, etc.)
        // that fprintf the call then forward to the real GL function.
        // Not needed in the Rust port -- use Renderdoc or SDL2 debug
        // callbacks instead.
        self.log_active = _enable;
    }

    pub fn glimp_log_new_frame(&self) {
        // Original: fprintf(glw_state.log_fp, "*** R_BeginFrame ***\n")
    }
}
