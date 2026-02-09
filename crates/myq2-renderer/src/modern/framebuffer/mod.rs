//! Framebuffer objects
//!
//! Render targets for water effects, reflections, and post-processing.

mod render_target;
mod water_fbo;
mod postprocess;

pub use render_target::RenderTarget;
pub use water_fbo::WaterFbo;
pub use postprocess::PostProcessor;
