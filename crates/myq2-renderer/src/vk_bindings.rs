//! OpenGL bindings stub (SDL3 GPU migration)
//!
//! This module provides no-op stubs for the GL functions that the legacy
//! code calls. The modern renderer uses SDL3 GPU API directly.
//! These stubs exist only to allow the legacy code to compile.

#![allow(
    clippy::all,
    non_camel_case_types,
    non_upper_case_globals,
    non_snake_case,
    unused_imports,
    dead_code,
    unused_variables
)]

use std::ffi::c_void;

// ============================================================================
// GL Type definitions
// ============================================================================

pub use crate::qvk::{
    GLenum, GLboolean, GLbitfield, GLbyte, GLshort, GLint,
    GLubyte, GLushort, GLuint, GLsizei, GLfloat, GLclampf,
    GLdouble, GLclampd,
};

// ============================================================================
// GL Constants
// ============================================================================

pub const TEXTURE_2D: GLenum = 0x0DE1;
pub const TEXTURE_ENV: GLenum = 0x2300;
pub const TEXTURE_ENV_MODE: GLenum = 0x2200;

// ============================================================================
// No-op GL function stubs
// ============================================================================

pub unsafe fn load_with<F>(_loader: F) where F: FnMut(&str) -> *const c_void {
    // No-op: SDL3 GPU doesn't need OpenGL loading
}

pub unsafe fn TexParameterf(_target: GLenum, _pname: GLenum, _param: GLfloat) {}
pub unsafe fn TexParameteri(_target: GLenum, _pname: GLenum, _param: GLint) {}
pub unsafe fn Color4f(_r: GLfloat, _g: GLfloat, _b: GLfloat, _a: GLfloat) {}
pub unsafe fn Enable(_cap: GLenum) {}
pub unsafe fn Disable(_cap: GLenum) {}
pub unsafe fn BindTexture(_target: GLenum, _texture: GLuint) {}
pub unsafe fn TexImage2D(
    _target: GLenum, _level: GLint, _internal_format: GLint,
    _width: GLsizei, _height: GLsizei, _border: GLint,
    _format: GLenum, _data_type: GLenum, _data: *const c_void,
) {}
pub unsafe fn DeleteTextures(_n: GLsizei, _textures: *const GLuint) {}
pub unsafe fn TexEnvf(_target: GLenum, _pname: GLenum, _param: GLfloat) {}
pub unsafe fn TexEnvi(_target: GLenum, _pname: GLenum, _param: GLint) {}
pub unsafe fn ActiveTexture(_texture: GLenum) {}
pub unsafe fn ClientActiveTexture(_texture: GLenum) {}
pub unsafe fn Translatef(_x: GLfloat, _y: GLfloat, _z: GLfloat) {}
pub unsafe fn Rotatef(_angle: GLfloat, _x: GLfloat, _y: GLfloat, _z: GLfloat) {}
pub unsafe fn TexSubImage2D(
    _target: GLenum, _level: GLint, _xoffset: GLint, _yoffset: GLint,
    _width: GLsizei, _height: GLsizei, _format: GLenum, _data_type: GLenum,
    _data: *const c_void,
) {}
pub unsafe fn LoadIdentity() {}
pub unsafe fn MatrixMode(_mode: GLenum) {}
pub unsafe fn Ortho(_l: GLdouble, _r: GLdouble, _b: GLdouble, _t: GLdouble, _n: GLdouble, _f: GLdouble) {}
pub unsafe fn Frustum(_l: GLdouble, _r: GLdouble, _b: GLdouble, _t: GLdouble, _n: GLdouble, _f: GLdouble) {}
pub unsafe fn Viewport(_x: GLint, _y: GLint, _w: GLsizei, _h: GLsizei) {}
pub unsafe fn Scissor(_x: GLint, _y: GLint, _w: GLsizei, _h: GLsizei) {}
pub unsafe fn DepthFunc(_func: GLenum) {}
pub unsafe fn DepthRange(_near: GLdouble, _far: GLdouble) {}
pub unsafe fn AlphaFunc(_func: GLenum, _ref_val: GLclampf) {}
pub unsafe fn CullFace(_mode: GLenum) {}
pub unsafe fn ClearColor(_r: GLclampf, _g: GLclampf, _b: GLclampf, _a: GLclampf) {}
pub unsafe fn Clear(_mask: GLbitfield) {}
pub unsafe fn ClearStencil(_s: GLint) {}
pub unsafe fn Finish() {}
pub unsafe fn GetFloatv(_pname: GLenum, _params: *mut GLfloat) {}
pub unsafe fn GetIntegerv(_pname: GLenum, _params: *mut GLint) {}
pub unsafe fn GetString(_name: GLenum) -> *const GLubyte { std::ptr::null() }
pub unsafe fn GetError() -> GLenum { 0 }
pub unsafe fn DrawBuffer(_mode: GLenum) {}
pub unsafe fn Hint(_target: GLenum, _mode: GLenum) {}
pub unsafe fn ClipPlane(_plane: GLenum, _equation: *const GLdouble) {}
pub unsafe fn StencilFunc(_func: GLenum, _ref_val: GLint, _mask: GLuint) {}
pub unsafe fn StencilOp(_fail: GLenum, _zfail: GLenum, _zpass: GLenum) {}
pub unsafe fn ReadPixels(
    _x: GLint, _y: GLint, _width: GLsizei, _height: GLsizei,
    _format: GLenum, _data_type: GLenum, _pixels: *mut c_void,
) {}
