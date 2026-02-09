//! Modern Vulkan Renderer
//!
//! This module provides the Vulkan 1.3 rendering implementation using:
//! - Vulkan buffers for geometry batching
//! - SPIR-V shaders for all rendering
//! - Texture arrays for lightmaps
//! - Descriptor sets for uniform data
//! - Render targets for effects and post-processing
//! - Instanced rendering for models
//!
//! This replaces the legacy SDL3 GPU renderer with direct Vulkan.

pub mod gpu_device;
pub mod shader;
pub mod geometry;
pub mod texture;
pub mod framebuffer;
mod render_path;

pub use render_path::ModernRenderPath;

/// The standard renderer type.
pub type Renderer = ModernRenderPath;

use crate::vk_rmain::EntityLocal;

/// Per-frame parameters passed to the modern renderer.
#[derive(Clone, Debug, Default)]
pub struct FrameParams {
    pub time: f32,
    pub vieworg: [f32; 3],
    pub viewangles: [f32; 3],
    pub fov_x: f32,
    pub fov_y: f32,
    pub width: u32,
    pub height: u32,
    pub blend: [f32; 4],
    pub rdflags: i32,
}

/// Trait defining the renderer interface.
pub trait RenderPath {
    /// Initialize the render path. Called once at startup.
    fn init(&mut self) -> Result<(), RenderError>;

    /// Shutdown and release resources.
    fn shutdown(&mut self);

    /// Called at the start of each 3D rendering pass.
    fn begin_frame(&mut self, params: &FrameParams);

    /// Called at the end of each 3D rendering pass.
    fn end_frame(&mut self);

    // ========== World Rendering ==========

    /// Draw the BSP world geometry.
    fn draw_world(&mut self);

    /// Draw alpha-blended world surfaces (water, glass, etc.).
    fn draw_alpha_surfaces(&mut self);

    /// Blend lightmaps onto world surfaces.
    fn blend_lightmaps(&mut self);

    // ========== Entity Rendering ==========

    /// Draw a brush model entity (doors, platforms, etc.).
    fn draw_brush_model(&mut self, entity: &EntityLocal);

    /// Draw an alias model entity (MD2 - enemies, items, weapons).
    fn draw_alias_model(&mut self, entity: &EntityLocal);

    /// Draw a sprite model entity.
    fn draw_sprite_model(&mut self, entity: &EntityLocal);

    // ========== Effects ==========

    /// Draw all active particles.
    fn draw_particles(&mut self, particles: &[ParticleData]);

    /// Render dynamic lights.
    fn render_dlights(&mut self);

    /// Draw the sky.
    fn draw_sky(&mut self);

    // ========== 2D Drawing ==========

    /// Draw a character from the console font.
    fn draw_char(&mut self, x: i32, y: i32, num: i32);

    /// Draw a named picture at its native size.
    fn draw_pic(&mut self, x: i32, y: i32, pic: &str);

    /// Draw a stretched picture.
    fn draw_stretch_pic(&mut self, x: i32, y: i32, w: i32, h: i32, pic: &str);

    /// Draw a filled rectangle.
    fn draw_fill(&mut self, x: i32, y: i32, w: i32, h: i32, color: i32, alpha: f32);

    /// Draw a tiling background picture.
    fn draw_tile_clear(&mut self, x: i32, y: i32, w: i32, h: i32, pic: &str);

    /// Fade the screen with a dark overlay.
    fn draw_fade_screen(&mut self);

    /// Draw a string of characters.
    fn draw_string(&mut self, x: i32, y: i32, s: &str);

    /// Draw raw image data (for cinematics).
    fn draw_stretch_raw(&mut self, x: i32, y: i32, w: i32, h: i32, cols: i32, rows: i32, data: &[u8]);

    /// Flush all batched 2D draw calls.
    fn flush_2d(&mut self);
}

/// Errors that can occur during rendering.
#[derive(Debug)]
pub enum RenderError {
    /// Shader compilation failed.
    ShaderCompilation(String),
    /// Shader linking failed.
    ShaderLinking(String),
    /// Vulkan error.
    Vulkan(String),
    /// Resource not found.
    NotFound(String),
    /// Generic error.
    Other(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::ShaderCompilation(msg) => write!(f, "Shader compilation error: {}", msg),
            RenderError::ShaderLinking(msg) => write!(f, "Shader linking error: {}", msg),
            RenderError::Vulkan(msg) => write!(f, "Vulkan error: {}", msg),
            RenderError::NotFound(name) => write!(f, "Resource not found: {}", name),
            RenderError::Other(msg) => write!(f, "Render error: {}", msg),
        }
    }
}

impl std::error::Error for RenderError {}

/// Particle data for rendering.
#[derive(Clone, Debug)]
pub struct ParticleData {
    pub origin: [f32; 3],
    pub color: usize,
    pub alpha: f32,
    pub particle_type: usize,
}
