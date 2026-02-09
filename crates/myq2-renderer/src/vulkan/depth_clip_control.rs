//! Depth Clip Control (VK_EXT_depth_clip_control)
//!
//! Control depth clipping behavior:
//! - Use negative one to one depth range (OpenGL style)
//! - Better infinite far plane handling
//! - Reversed depth buffer support
//! - Improved precision for distant objects

use ash::vk;

/// Depth clip control capabilities.
#[derive(Debug, Clone, Default)]
pub struct DepthClipControlCapabilities {
    /// Whether depth clip control is supported.
    pub supported: bool,
}

/// Query depth clip control capabilities.
pub fn query_capabilities(ctx: &super::context::VulkanContext) -> DepthClipControlCapabilities {
    let mut dcc_features = vk::PhysicalDeviceDepthClipControlFeaturesEXT::default();
    let mut features2 = vk::PhysicalDeviceFeatures2::default()
        .push_next(&mut dcc_features);

    unsafe {
        ctx.instance.get_physical_device_features2(ctx.physical_device, &mut features2);
    }

    let _ = features2;

    DepthClipControlCapabilities {
        supported: dcc_features.depth_clip_control == vk::TRUE,
    }
}

/// Depth range mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthRangeMode {
    /// Zero to one depth range (Vulkan default).
    ZeroToOne,
    /// Negative one to one depth range (OpenGL style).
    NegativeOneToOne,
}

/// Depth buffer configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthBufferMode {
    /// Standard depth buffer (near=0, far=1).
    Standard,
    /// Reversed depth buffer (near=1, far=0).
    Reversed,
    /// Reversed with infinite far plane.
    ReversedInfinite,
}

impl DepthBufferMode {
    /// Get near plane depth value.
    pub fn near_depth(&self) -> f32 {
        match self {
            DepthBufferMode::Standard => 0.0,
            DepthBufferMode::Reversed | DepthBufferMode::ReversedInfinite => 1.0,
        }
    }

    /// Get far plane depth value.
    pub fn far_depth(&self) -> f32 {
        match self {
            DepthBufferMode::Standard => 1.0,
            DepthBufferMode::Reversed | DepthBufferMode::ReversedInfinite => 0.0,
        }
    }

    /// Get comparison operator for depth test.
    pub fn compare_op(&self) -> vk::CompareOp {
        match self {
            DepthBufferMode::Standard => vk::CompareOp::LESS,
            DepthBufferMode::Reversed | DepthBufferMode::ReversedInfinite => vk::CompareOp::GREATER,
        }
    }

    /// Get comparison operator for depth test (or equal).
    pub fn compare_op_equal(&self) -> vk::CompareOp {
        match self {
            DepthBufferMode::Standard => vk::CompareOp::LESS_OR_EQUAL,
            DepthBufferMode::Reversed | DepthBufferMode::ReversedInfinite => vk::CompareOp::GREATER_OR_EQUAL,
        }
    }

    /// Get clear value for depth buffer.
    pub fn clear_value(&self) -> f32 {
        self.far_depth()
    }
}

/// Create viewport state with negative depth range.
pub fn create_negative_depth_viewport_state() -> vk::PipelineViewportDepthClipControlCreateInfoEXT<'static> {
    vk::PipelineViewportDepthClipControlCreateInfoEXT::default()
        .negative_one_to_one(true)
}

/// Depth clip configuration.
#[derive(Debug, Clone)]
pub struct DepthClipConfig {
    /// Depth range mode.
    pub range_mode: DepthRangeMode,
    /// Depth buffer mode.
    pub buffer_mode: DepthBufferMode,
    /// Enable depth clipping.
    pub clip_enable: bool,
    /// Enable depth clamping.
    pub clamp_enable: bool,
}

impl Default for DepthClipConfig {
    fn default() -> Self {
        Self {
            range_mode: DepthRangeMode::ZeroToOne,
            buffer_mode: DepthBufferMode::Standard,
            clip_enable: true,
            clamp_enable: false,
        }
    }
}

impl DepthClipConfig {
    /// Create config for reversed depth with infinite far plane.
    pub fn reversed_infinite() -> Self {
        Self {
            range_mode: DepthRangeMode::ZeroToOne,
            buffer_mode: DepthBufferMode::ReversedInfinite,
            clip_enable: false, // Disable clipping for infinite far
            clamp_enable: true,
        }
    }

    /// Create config for OpenGL-style depth.
    pub fn opengl_style() -> Self {
        Self {
            range_mode: DepthRangeMode::NegativeOneToOne,
            buffer_mode: DepthBufferMode::Standard,
            clip_enable: true,
            clamp_enable: false,
        }
    }
}

/// Projection matrix helpers for different depth modes.
pub mod projection {
    /// Create perspective projection matrix for reversed infinite depth.
    pub fn perspective_reversed_infinite(
        fov_y: f32,
        aspect: f32,
        near: f32,
    ) -> [[f32; 4]; 4] {
        let f = 1.0 / (fov_y / 2.0).tan();

        [
            [f / aspect, 0.0, 0.0, 0.0],
            [0.0, f, 0.0, 0.0],
            [0.0, 0.0, 0.0, -1.0],
            [0.0, 0.0, near, 0.0],
        ]
    }

    /// Create perspective projection matrix for reversed depth.
    pub fn perspective_reversed(
        fov_y: f32,
        aspect: f32,
        near: f32,
        far: f32,
    ) -> [[f32; 4]; 4] {
        let f = 1.0 / (fov_y / 2.0).tan();
        let nf = 1.0 / (near - far);

        [
            [f / aspect, 0.0, 0.0, 0.0],
            [0.0, f, 0.0, 0.0],
            [0.0, 0.0, near * nf, -1.0],
            [0.0, 0.0, far * near * nf, 0.0],
        ]
    }

    /// Create perspective projection matrix for standard depth.
    pub fn perspective_standard(
        fov_y: f32,
        aspect: f32,
        near: f32,
        far: f32,
    ) -> [[f32; 4]; 4] {
        let f = 1.0 / (fov_y / 2.0).tan();
        let nf = 1.0 / (near - far);

        [
            [f / aspect, 0.0, 0.0, 0.0],
            [0.0, f, 0.0, 0.0],
            [0.0, 0.0, far * nf, -1.0],
            [0.0, 0.0, far * near * nf, 0.0],
        ]
    }

    /// Create orthographic projection matrix for reversed depth.
    pub fn orthographic_reversed(
        left: f32,
        right: f32,
        bottom: f32,
        top: f32,
        near: f32,
        far: f32,
    ) -> [[f32; 4]; 4] {
        let rml = 1.0 / (right - left);
        let tmb = 1.0 / (top - bottom);
        let fmn = 1.0 / (far - near);

        [
            [2.0 * rml, 0.0, 0.0, 0.0],
            [0.0, 2.0 * tmb, 0.0, 0.0],
            [0.0, 0.0, fmn, 0.0],
            [-(right + left) * rml, -(top + bottom) * tmb, far * fmn, 1.0],
        ]
    }

    /// Linearize depth for reversed depth buffer.
    pub fn linearize_depth_reversed(depth: f32, near: f32) -> f32 {
        near / depth
    }

    /// Linearize depth for reversed infinite depth buffer.
    pub fn linearize_depth_reversed_infinite(depth: f32, near: f32) -> f32 {
        near / depth
    }

    /// Linearize depth for standard depth buffer.
    pub fn linearize_depth_standard(depth: f32, near: f32, far: f32) -> f32 {
        near * far / (far - depth * (far - near))
    }
}

/// GLSL code for depth utilities.
pub mod glsl {
    /// Depth linearization functions.
    pub const LINEARIZE_DEPTH: &str = r#"
// Linearize reversed depth
float linearizeDepthReversed(float depth, float near) {
    return near / depth;
}

// Linearize reversed infinite depth
float linearizeDepthReversedInfinite(float depth, float near) {
    return near / depth;
}

// Linearize standard depth
float linearizeDepthStandard(float depth, float near, float far) {
    return near * far / (far - depth * (far - near));
}
"#;

    /// Depth reconstruction.
    pub const RECONSTRUCT_POSITION: &str = r#"
// Reconstruct world position from depth (reversed infinite)
vec3 reconstructPositionReversedInfinite(vec2 uv, float depth, mat4 invViewProj, float near) {
    // For reversed infinite: depth = near / linearDepth
    float linearDepth = near / depth;

    vec4 clipPos = vec4(uv * 2.0 - 1.0, depth, 1.0);
    vec4 worldPos = invViewProj * clipPos;
    return worldPos.xyz / worldPos.w;
}

// Reconstruct view-space position from depth
vec3 reconstructViewPosition(vec2 uv, float depth, mat4 invProj) {
    vec4 clipPos = vec4(uv * 2.0 - 1.0, depth, 1.0);
    vec4 viewPos = invProj * clipPos;
    return viewPos.xyz / viewPos.w;
}
"#;
}

/// Depth clip control manager.
pub struct DepthClipControlManager {
    capabilities: DepthClipControlCapabilities,
    config: DepthClipConfig,
}

impl DepthClipControlManager {
    /// Create new manager.
    pub fn new(ctx: &super::context::VulkanContext) -> Self {
        let capabilities = query_capabilities(ctx);

        Self {
            capabilities,
            config: DepthClipConfig::default(),
        }
    }

    /// Check if depth clip control is supported.
    pub fn is_supported(&self) -> bool {
        self.capabilities.supported
    }

    /// Set configuration.
    pub fn set_config(&mut self, config: DepthClipConfig) {
        self.config = config;
    }

    /// Get current configuration.
    pub fn config(&self) -> &DepthClipConfig {
        &self.config
    }

    /// Check if negative one to one is supported and enabled.
    pub fn uses_negative_one_to_one(&self) -> bool {
        self.capabilities.supported && self.config.range_mode == DepthRangeMode::NegativeOneToOne
    }

    /// Get depth compare op for current mode.
    pub fn compare_op(&self) -> vk::CompareOp {
        self.config.buffer_mode.compare_op()
    }

    /// Get depth clear value for current mode.
    pub fn clear_value(&self) -> f32 {
        self.config.buffer_mode.clear_value()
    }
}
