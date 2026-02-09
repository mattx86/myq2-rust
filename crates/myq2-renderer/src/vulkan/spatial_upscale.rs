//! Spatial Upscaling for Video
//!
//! High-quality image upscaling beyond FSR:
//! - Edge-directed interpolation
//! - Detail enhancement
//! - Temporal stability
//! - Multiple quality presets

use ash::vk;

/// Upscaling algorithm type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpscaleAlgorithm {
    /// Bilinear (fastest, lowest quality).
    Bilinear,
    /// Bicubic (good balance).
    Bicubic,
    /// Lanczos (sharp, may ring).
    Lanczos,
    /// Edge-Adaptive (preserves edges).
    EdgeAdaptive,
    /// AI-style upscaling (compute intensive).
    Neural,
}

/// Upscaling configuration.
#[derive(Debug, Clone)]
pub struct UpscaleConfig {
    /// Source resolution.
    pub source_width: u32,
    pub source_height: u32,
    /// Target resolution.
    pub target_width: u32,
    pub target_height: u32,
    /// Algorithm to use.
    pub algorithm: UpscaleAlgorithm,
    /// Sharpness (0-1).
    pub sharpness: f32,
    /// Edge preservation (0-1).
    pub edge_preservation: f32,
    /// Denoise strength (0-1).
    pub denoise: f32,
}

impl Default for UpscaleConfig {
    fn default() -> Self {
        Self {
            source_width: 1280,
            source_height: 720,
            target_width: 1920,
            target_height: 1080,
            algorithm: UpscaleAlgorithm::EdgeAdaptive,
            sharpness: 0.5,
            edge_preservation: 0.7,
            denoise: 0.0,
        }
    }
}

impl UpscaleConfig {
    /// Get scale factor.
    pub fn scale_factor(&self) -> (f32, f32) {
        (
            self.target_width as f32 / self.source_width as f32,
            self.target_height as f32 / self.source_height as f32,
        )
    }

    /// Common presets.
    pub fn quality_720p_to_1080p() -> Self {
        Self {
            source_width: 1280,
            source_height: 720,
            target_width: 1920,
            target_height: 1080,
            ..Default::default()
        }
    }

    pub fn quality_1080p_to_4k() -> Self {
        Self {
            source_width: 1920,
            source_height: 1080,
            target_width: 3840,
            target_height: 2160,
            algorithm: UpscaleAlgorithm::Neural,
            sharpness: 0.6,
            ..Default::default()
        }
    }

    pub fn performance_540p_to_1080p() -> Self {
        Self {
            source_width: 960,
            source_height: 540,
            target_width: 1920,
            target_height: 1080,
            algorithm: UpscaleAlgorithm::EdgeAdaptive,
            sharpness: 0.4,
            ..Default::default()
        }
    }
}

/// Push constants for upscale shader.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct UpscalePushConstants {
    /// Source dimensions.
    pub src_size: [f32; 2],
    /// Target dimensions.
    pub dst_size: [f32; 2],
    /// 1.0 / source dimensions.
    pub src_texel_size: [f32; 2],
    /// Sharpness factor.
    pub sharpness: f32,
    /// Edge threshold.
    pub edge_threshold: f32,
    /// Denoise factor.
    pub denoise: f32,
    /// Algorithm ID.
    pub algorithm: u32,
    /// Padding.
    pub _padding: [f32; 2],
}

impl From<&UpscaleConfig> for UpscalePushConstants {
    fn from(config: &UpscaleConfig) -> Self {
        Self {
            src_size: [config.source_width as f32, config.source_height as f32],
            dst_size: [config.target_width as f32, config.target_height as f32],
            src_texel_size: [
                1.0 / config.source_width as f32,
                1.0 / config.source_height as f32,
            ],
            sharpness: config.sharpness,
            edge_threshold: config.edge_preservation,
            denoise: config.denoise,
            algorithm: config.algorithm as u32,
            _padding: [0.0; 2],
        }
    }
}

/// GLSL code for upscaling shaders.
pub mod glsl {
    /// Bicubic interpolation.
    pub const BICUBIC: &str = r#"
// Bicubic interpolation kernel
vec4 cubic(float x) {
    float x2 = x * x;
    float x3 = x2 * x;
    vec4 w;
    w.x = -x3 + 3.0*x2 - 3.0*x + 1.0;
    w.y = 3.0*x3 - 6.0*x2 + 4.0;
    w.z = -3.0*x3 + 3.0*x2 + 3.0*x + 1.0;
    w.w = x3;
    return w / 6.0;
}

vec4 textureBicubic(sampler2D tex, vec2 uv, vec2 texelSize) {
    vec2 coord = uv / texelSize - 0.5;
    vec2 fxy = fract(coord);
    coord -= fxy;

    vec4 xcubic = cubic(fxy.x);
    vec4 ycubic = cubic(fxy.y);

    vec4 c = coord.xxyy + vec2(-0.5, 1.5).xyxy;
    vec4 s = vec4(xcubic.xz + xcubic.yw, ycubic.xz + ycubic.yw);
    vec4 offset = c + vec4(xcubic.yw, ycubic.yw) / s;

    offset *= texelSize.xxyy;

    vec4 sample0 = texture(tex, offset.xz);
    vec4 sample1 = texture(tex, offset.yz);
    vec4 sample2 = texture(tex, offset.xw);
    vec4 sample3 = texture(tex, offset.yw);

    float sx = s.x / (s.x + s.y);
    float sy = s.z / (s.z + s.w);

    return mix(mix(sample3, sample2, sx), mix(sample1, sample0, sx), sy);
}
"#;

    /// Lanczos interpolation.
    pub const LANCZOS: &str = r#"
// Lanczos kernel
float lanczos(float x, float a) {
    if (abs(x) < 1e-5) return 1.0;
    if (abs(x) >= a) return 0.0;
    float pi_x = 3.14159265 * x;
    return a * sin(pi_x) * sin(pi_x / a) / (pi_x * pi_x);
}

vec4 textureLanczos(sampler2D tex, vec2 uv, vec2 texelSize) {
    const float a = 2.0; // Lanczos parameter

    vec2 center = uv / texelSize;
    vec2 f = fract(center);
    center = floor(center);

    vec4 sum = vec4(0.0);
    float weightSum = 0.0;

    for (int y = -2; y <= 2; y++) {
        for (int x = -2; x <= 2; x++) {
            float wx = lanczos(float(x) - f.x, a);
            float wy = lanczos(float(y) - f.y, a);
            float w = wx * wy;

            vec2 coord = (center + vec2(x, y) + 0.5) * texelSize;
            sum += texture(tex, coord) * w;
            weightSum += w;
        }
    }

    return sum / weightSum;
}
"#;

    /// Edge-adaptive upscaling.
    pub const EDGE_ADAPTIVE: &str = r#"
// Edge detection
float luminance(vec3 c) {
    return dot(c, vec3(0.299, 0.587, 0.114));
}

vec2 detectEdge(sampler2D tex, vec2 uv, vec2 texelSize) {
    float n = luminance(texture(tex, uv + vec2(0, -texelSize.y)).rgb);
    float s = luminance(texture(tex, uv + vec2(0, texelSize.y)).rgb);
    float e = luminance(texture(tex, uv + vec2(texelSize.x, 0)).rgb);
    float w = luminance(texture(tex, uv + vec2(-texelSize.x, 0)).rgb);

    vec2 gradient = vec2(e - w, s - n);
    return normalize(gradient + vec2(1e-5));
}

vec4 textureEdgeAdaptive(sampler2D tex, vec2 uv, vec2 texelSize, float edgeThreshold) {
    // Sample neighborhood
    vec4 center = texture(tex, uv);
    vec2 edge = detectEdge(tex, uv, texelSize);
    float edgeStrength = length(edge);

    if (edgeStrength < edgeThreshold) {
        // Smooth area - use bicubic
        return textureBicubic(tex, uv, texelSize);
    }

    // Edge area - interpolate along edge direction
    vec2 perpEdge = vec2(-edge.y, edge.x);

    vec4 along1 = texture(tex, uv + perpEdge * texelSize);
    vec4 along2 = texture(tex, uv - perpEdge * texelSize);

    // Blend based on edge alignment
    float blend = 0.5;
    return mix(center, (along1 + along2) * 0.5, blend * edgeStrength);
}
"#;

    /// Sharpening pass.
    pub const SHARPENING: &str = r#"
// Contrast-aware sharpening
vec4 sharpen(sampler2D tex, vec2 uv, vec2 texelSize, float strength) {
    vec4 center = texture(tex, uv);

    vec4 n = texture(tex, uv + vec2(0, -texelSize.y));
    vec4 s = texture(tex, uv + vec2(0, texelSize.y));
    vec4 e = texture(tex, uv + vec2(texelSize.x, 0));
    vec4 w = texture(tex, uv + vec2(-texelSize.x, 0));

    vec4 blur = (n + s + e + w) * 0.25;
    vec4 diff = center - blur;

    // Reduce sharpening in high-contrast areas to prevent ringing
    float contrast = length(diff.rgb);
    float adaptiveStrength = strength * (1.0 - min(contrast * 2.0, 0.8));

    return center + diff * adaptiveStrength;
}
"#;

    /// Complete upscale fragment shader.
    pub const UPSCALE_FRAGMENT: &str = r#"
#version 450

layout(binding = 0) uniform sampler2D inputTexture;

layout(push_constant) uniform PushConstants {
    vec2 srcSize;
    vec2 dstSize;
    vec2 srcTexelSize;
    float sharpness;
    float edgeThreshold;
    float denoise;
    uint algorithm;
} pc;

layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 fragColor;

// Include interpolation functions here...

void main() {
    vec4 color;

    switch (pc.algorithm) {
        case 0: // Bilinear
            color = texture(inputTexture, uv);
            break;
        case 1: // Bicubic
            color = textureBicubic(inputTexture, uv, pc.srcTexelSize);
            break;
        case 2: // Lanczos
            color = textureLanczos(inputTexture, uv, pc.srcTexelSize);
            break;
        case 3: // Edge-Adaptive
            color = textureEdgeAdaptive(inputTexture, uv, pc.srcTexelSize, pc.edgeThreshold);
            break;
        default:
            color = texture(inputTexture, uv);
    }

    // Apply sharpening
    if (pc.sharpness > 0.0) {
        color = sharpen(inputTexture, uv, pc.srcTexelSize, pc.sharpness);
    }

    fragColor = color;
}
"#;
}

/// Upscaler state.
pub struct SpatialUpscaler {
    config: UpscaleConfig,
    /// Push constants.
    push_constants: UpscalePushConstants,
}

impl SpatialUpscaler {
    /// Create a new upscaler.
    pub fn new(config: UpscaleConfig) -> Self {
        let push_constants = UpscalePushConstants::from(&config);

        Self {
            config,
            push_constants,
        }
    }

    /// Update configuration.
    pub fn update_config(&mut self, config: UpscaleConfig) {
        self.push_constants = UpscalePushConstants::from(&config);
        self.config = config;
    }

    /// Get push constants for shader.
    pub fn push_constants(&self) -> &UpscalePushConstants {
        &self.push_constants
    }

    /// Get workgroup count for compute shader.
    pub fn compute_workgroups(&self, local_size: u32) -> [u32; 3] {
        [
            (self.config.target_width + local_size - 1) / local_size,
            (self.config.target_height + local_size - 1) / local_size,
            1,
        ]
    }
}

/// Temporal upscaling for better quality with motion.
pub mod temporal {
    use super::*;

    /// Temporal upscale configuration.
    #[derive(Debug, Clone)]
    pub struct TemporalUpscaleConfig {
        /// Base spatial config.
        pub spatial: UpscaleConfig,
        /// Motion vector scale.
        pub motion_scale: f32,
        /// Temporal blend factor (0-1).
        pub temporal_blend: f32,
        /// Anti-ghosting strength.
        pub anti_ghosting: f32,
    }

    impl Default for TemporalUpscaleConfig {
        fn default() -> Self {
            Self {
                spatial: UpscaleConfig::default(),
                motion_scale: 1.0,
                temporal_blend: 0.9,
                anti_ghosting: 0.5,
            }
        }
    }

    /// Push constants for temporal upscaling.
    #[repr(C)]
    #[derive(Debug, Clone, Copy, Default)]
    pub struct TemporalPushConstants {
        /// Current frame index.
        pub frame_index: u32,
        /// Temporal blend factor.
        pub temporal_blend: f32,
        /// Anti-ghosting strength.
        pub anti_ghosting: f32,
        /// Motion scale.
        pub motion_scale: f32,
        /// Jitter offset for current frame.
        pub jitter: [f32; 2],
        /// Padding.
        pub _padding: [f32; 2],
    }

    /// Halton sequence for jitter.
    pub fn halton_jitter(frame: u32, base: u32) -> f32 {
        let mut result = 0.0f32;
        let mut f = 1.0 / base as f32;
        let mut i = frame;

        while i > 0 {
            result += f * (i % base) as f32;
            i /= base;
            f /= base as f32;
        }

        result - 0.5
    }

    /// Get jitter offset for frame.
    pub fn get_jitter(frame: u32) -> [f32; 2] {
        [
            halton_jitter(frame % 8 + 1, 2),
            halton_jitter(frame % 8 + 1, 3),
        ]
    }
}
