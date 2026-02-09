//! Shader and pipeline system
//!
//! Provides shader compilation, linking, uniform management, and shader program caching.
//! The pipeline module handles SDL3 GPU pipeline creation from SPIR-V shaders.

mod program;
mod manager;
mod uniforms;
pub mod pipeline;

pub use program::ShaderProgram;
pub use manager::{ShaderManager, ShaderType};
pub use uniforms::{PerFrameUniforms, PerObjectUniforms, UniformBuffer};
pub use pipeline::{PipelineManager, PipelineVariant};
