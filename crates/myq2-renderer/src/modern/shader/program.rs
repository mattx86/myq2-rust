//! Shader program (SDL3 GPU compatibility stubs)
//!
//! In SDL3 GPU, shaders are compiled to SPIR-V at build time and loaded into
//! GraphicsPipeline objects by PipelineManager. This module provides a
//! compatibility interface that the existing code calls into, but all methods
//! are no-ops — actual pipeline binding and uniform data happen through
//! render passes and uniform buffers.

use crate::modern::RenderError;
use std::collections::HashMap;

/// A shader program (compatibility stub for SDL3 GPU transition).
///
/// In the GL era, this compiled and linked GLSL shaders. Now it's a no-op
/// placeholder — pipelines are managed by `PipelineManager`.
pub struct ShaderProgram {
    /// Dummy ID (compatibility).
    id: u32,
    /// Cached uniform locations (compatibility — values are meaningless).
    uniform_cache: HashMap<String, i32>,
}

impl ShaderProgram {
    /// Create a shader program (no-op — returns a dummy program).
    ///
    /// In SDL3 GPU, shaders are compiled to SPIR-V at build time.
    pub fn from_source(_vertex_src: &str, _fragment_src: &str) -> Result<Self, RenderError> {
        Ok(Self {
            id: 0,
            uniform_cache: HashMap::new(),
        })
    }

    /// Create a shader program with geometry shader (no-op).
    pub fn from_source_with_geometry(
        _vertex_src: &str,
        _geometry_src: &str,
        _fragment_src: &str,
    ) -> Result<Self, RenderError> {
        Ok(Self {
            id: 0,
            uniform_cache: HashMap::new(),
        })
    }

    /// Bind this shader program (no-op).
    ///
    /// In SDL3 GPU, pipeline binding happens through render_pass.bind_graphics_pipeline().
    pub fn bind(&self) {}

    /// Unbind any shader program (no-op).
    pub fn unbind() {}

    /// Get the program ID (compatibility stub, returns 0).
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Get or cache a uniform location (returns dummy values).
    pub fn get_uniform_location(&mut self, name: &str) -> i32 {
        if let Some(&loc) = self.uniform_cache.get(name) {
            return loc;
        }
        let loc = self.uniform_cache.len() as i32;
        self.uniform_cache.insert(name.to_string(), loc);
        loc
    }

    /// Set a uniform integer value (no-op).
    pub fn set_int(&mut self, _name: &str, _value: i32) {}

    /// Set a uniform float value (no-op).
    pub fn set_float(&mut self, _name: &str, _value: f32) {}

    /// Set a uniform vec2 value (no-op).
    pub fn set_vec2(&mut self, _name: &str, _x: f32, _y: f32) {}

    /// Set a uniform vec3 value (no-op).
    pub fn set_vec3(&mut self, _name: &str, _x: f32, _y: f32, _z: f32) {}

    /// Set a uniform vec3 value from an array (no-op).
    pub fn set_vec3_array(&mut self, _name: &str, _value: &[f32; 3]) {}

    /// Set a uniform vec4 value (no-op).
    pub fn set_vec4(&mut self, _name: &str, _x: f32, _y: f32, _z: f32, _w: f32) {}

    /// Set a uniform vec4 value from an array (no-op).
    pub fn set_vec4_array(&mut self, _name: &str, _value: &[f32; 4]) {}

    /// Set a uniform mat4 value (no-op).
    pub fn set_mat4(&mut self, _name: &str, _value: &[f32; 16]) {}

    /// Set a uniform sampler to a texture unit (no-op).
    pub fn set_sampler(&mut self, _name: &str, _texture_unit: i32) {}

    /// Set a uniform float array (no-op).
    pub fn set_float_array(&mut self, _name: &str, _values: &[f32]) {}

    /// Set a uniform vec3 array (no-op).
    pub fn set_vec3_array_multi(&mut self, _name: &str, _values: &[[f32; 3]]) {}
}
