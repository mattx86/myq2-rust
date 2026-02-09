//! Fragment Shader Barycentric for per-vertex attribute access
//!
//! VK_KHR_fragment_shader_barycentric allows fragment shaders to access
//! per-vertex attributes directly without interpolation:
//! - Wireframe rendering without geometry shaders
//! - Per-vertex color/normal access
//! - Custom interpolation modes
//! - Silhouette edge detection
//! - Displacement mapping effects

use ash::vk;

/// Barycentric capabilities.
#[derive(Debug, Clone, Default)]
pub struct BarycentricCapabilities {
    /// Whether barycentric coordinates are supported.
    pub supported: bool,
    /// Whether tri-fan provoking vertex is supported.
    pub tri_strip_vertex_order_independent: bool,
}

/// Query barycentric capabilities.
pub fn query_capabilities(ctx: &super::context::VulkanContext) -> BarycentricCapabilities {
    let mut bary_features = vk::PhysicalDeviceFragmentShaderBarycentricFeaturesKHR::default();
    let mut features2 = vk::PhysicalDeviceFeatures2::default()
        .push_next(&mut bary_features);

    unsafe {
        ctx.instance.get_physical_device_features2(ctx.physical_device, &mut features2);
    }

    let mut bary_props = vk::PhysicalDeviceFragmentShaderBarycentricPropertiesKHR::default();
    let mut props2 = vk::PhysicalDeviceProperties2::default()
        .push_next(&mut bary_props);

    unsafe {
        ctx.instance.get_physical_device_properties2(ctx.physical_device, &mut props2);
    }

    BarycentricCapabilities {
        supported: bary_features.fragment_shader_barycentric == vk::TRUE,
        tri_strip_vertex_order_independent: bary_props.tri_strip_vertex_order_independent_of_provoking_vertex == vk::TRUE,
    }
}

/// Barycentric interpolation mode for shader usage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarycentricInterpolation {
    /// Standard perspective-correct interpolation.
    Smooth,
    /// Flat shading (first vertex).
    Flat,
    /// No perspective correction.
    NoPerspective,
    /// Centroid sampling.
    Centroid,
    /// Sample-rate interpolation.
    Sample,
    /// Per-vertex attribute access (barycentric).
    PerVertex,
}

/// GLSL code snippets for barycentric shaders.
pub mod glsl {
    /// Extension requirement for barycentric shaders.
    pub const EXTENSION: &str = "#extension GL_EXT_fragment_shader_barycentric : require";

    /// Get barycentric coordinates in fragment shader.
    /// Returns vec3 where x,y,z are weights for vertices 0,1,2.
    pub const BARYCENTRIC_COORD: &str = "gl_BaryCoordEXT";

    /// Get barycentric coordinates without perspective correction.
    pub const BARYCENTRIC_COORD_NO_PERSP: &str = "gl_BaryCoordNoPerspEXT";

    /// Declare a per-vertex input (array of 3 values).
    /// Usage: `pervertex_input("vec3", "vertexNormal")`
    pub fn pervertex_input(type_name: &str, name: &str) -> String {
        format!("pervertexEXT in {} {}[3];", type_name, name)
    }

    /// Access per-vertex attribute with barycentric interpolation.
    /// Usage: `interpolate_pervertex("vertexNormal", "gl_BaryCoordEXT")`
    pub fn interpolate_pervertex(attrib_name: &str, bary_coord: &str) -> String {
        format!(
            "{}[0] * {}.x + {}[1] * {}.y + {}[2] * {}.z",
            attrib_name, bary_coord,
            attrib_name, bary_coord,
            attrib_name, bary_coord
        )
    }

    /// Generate wireframe fragment shader code.
    /// Returns color blend factor for edge (1.0 = edge, 0.0 = interior).
    pub const WIREFRAME_EDGE_FACTOR: &str = r#"
float wireframeEdgeFactor(vec3 bary, float lineWidth) {
    vec3 d = fwidth(bary);
    vec3 a3 = smoothstep(vec3(0.0), d * lineWidth, bary);
    return min(min(a3.x, a3.y), a3.z);
}
"#;

    /// Generate silhouette edge detection code.
    pub const SILHOUETTE_DETECTION: &str = r#"
bool isSilhouetteEdge(vec3 normal0, vec3 normal1, vec3 normal2, vec3 viewDir) {
    float d0 = dot(normal0, viewDir);
    float d1 = dot(normal1, viewDir);
    float d2 = dot(normal2, viewDir);
    // Edge between front and back facing normals
    return (d0 * d1 < 0.0) || (d1 * d2 < 0.0) || (d2 * d0 < 0.0);
}
"#;

    /// Example wireframe fragment shader.
    pub const WIREFRAME_FRAGMENT_SHADER: &str = r#"
#version 450
#extension GL_EXT_fragment_shader_barycentric : require

layout(location = 0) out vec4 outColor;

layout(push_constant) uniform PushConstants {
    vec4 fillColor;
    vec4 wireColor;
    float lineWidth;
} pc;

void main() {
    vec3 bary = gl_BaryCoordEXT;
    vec3 d = fwidth(bary);
    vec3 a3 = smoothstep(vec3(0.0), d * pc.lineWidth, bary);
    float edgeFactor = min(min(a3.x, a3.y), a3.z);
    outColor = mix(pc.wireColor, pc.fillColor, edgeFactor);
}
"#;
}

/// Wireframe rendering configuration.
#[derive(Debug, Clone, Copy)]
pub struct WireframeConfig {
    /// Fill color.
    pub fill_color: [f32; 4],
    /// Wire color.
    pub wire_color: [f32; 4],
    /// Line width (in pixels, approximate).
    pub line_width: f32,
    /// Whether to show fill.
    pub show_fill: bool,
    /// Whether to show wireframe.
    pub show_wire: bool,
}

impl Default for WireframeConfig {
    fn default() -> Self {
        Self {
            fill_color: [0.2, 0.2, 0.2, 1.0],
            wire_color: [1.0, 1.0, 1.0, 1.0],
            line_width: 1.5,
            show_fill: true,
            show_wire: true,
        }
    }
}

impl WireframeConfig {
    /// White wireframe on black.
    pub fn white_on_black() -> Self {
        Self {
            fill_color: [0.0, 0.0, 0.0, 1.0],
            wire_color: [1.0, 1.0, 1.0, 1.0],
            ..Default::default()
        }
    }

    /// Green wireframe (matrix style).
    pub fn matrix_style() -> Self {
        Self {
            fill_color: [0.0, 0.02, 0.0, 1.0],
            wire_color: [0.0, 1.0, 0.0, 1.0],
            line_width: 1.0,
            ..Default::default()
        }
    }

    /// Wireframe only (no fill).
    pub fn wire_only() -> Self {
        Self {
            fill_color: [0.0, 0.0, 0.0, 0.0],
            wire_color: [1.0, 1.0, 1.0, 1.0],
            show_fill: false,
            ..Default::default()
        }
    }
}

/// Push constants for wireframe shader.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WireframePushConstants {
    pub fill_color: [f32; 4],
    pub wire_color: [f32; 4],
    pub line_width: f32,
    pub _padding: [f32; 3],
}

impl From<&WireframeConfig> for WireframePushConstants {
    fn from(config: &WireframeConfig) -> Self {
        let fill = if config.show_fill {
            config.fill_color
        } else {
            [0.0, 0.0, 0.0, 0.0]
        };

        let wire = if config.show_wire {
            config.wire_color
        } else {
            config.fill_color
        };

        Self {
            fill_color: fill,
            wire_color: wire,
            line_width: config.line_width,
            _padding: [0.0; 3],
        }
    }
}

/// Barycentric utility functions for CPU-side calculations.
pub mod math {
    /// Calculate barycentric coordinates for a point in a triangle.
    pub fn barycentric_coords(
        p: [f32; 3],
        v0: [f32; 3],
        v1: [f32; 3],
        v2: [f32; 3],
    ) -> [f32; 3] {
        let e0 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
        let e1 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];
        let ep = [p[0] - v0[0], p[1] - v0[1], p[2] - v0[2]];

        let d00 = dot(e0, e0);
        let d01 = dot(e0, e1);
        let d11 = dot(e1, e1);
        let d20 = dot(ep, e0);
        let d21 = dot(ep, e1);

        let denom = d00 * d11 - d01 * d01;
        if denom.abs() < 1e-10 {
            return [1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0];
        }

        let v = (d11 * d20 - d01 * d21) / denom;
        let w = (d00 * d21 - d01 * d20) / denom;
        let u = 1.0 - v - w;

        [u, v, w]
    }

    /// Interpolate a value using barycentric coordinates.
    pub fn interpolate<T: Copy + Default>(
        bary: [f32; 3],
        v0: T,
        v1: T,
        v2: T,
    ) -> T
    where
        T: std::ops::Mul<f32, Output = T> + std::ops::Add<Output = T>,
    {
        v0 * bary[0] + v1 * bary[1] + v2 * bary[2]
    }

    /// Interpolate a vec3 using barycentric coordinates.
    pub fn interpolate_vec3(
        bary: [f32; 3],
        v0: [f32; 3],
        v1: [f32; 3],
        v2: [f32; 3],
    ) -> [f32; 3] {
        [
            v0[0] * bary[0] + v1[0] * bary[1] + v2[0] * bary[2],
            v0[1] * bary[0] + v1[1] * bary[1] + v2[1] * bary[2],
            v0[2] * bary[0] + v1[2] * bary[1] + v2[2] * bary[2],
        ]
    }

    fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }
}
