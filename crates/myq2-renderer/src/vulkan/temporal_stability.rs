//! Temporal Stability Improvements for TAA
//!
//! Enhanced temporal anti-aliasing with better ghosting rejection:
//! - Motion-aware blending
//! - Color clamping/clipping
//! - History validation
//! - Variance clipping

use ash::vk;

/// Temporal stability configuration.
#[derive(Debug, Clone)]
pub struct TemporalStabilityConfig {
    /// Blend factor for static pixels (0-1).
    pub static_blend: f32,
    /// Blend factor for moving pixels (0-1).
    pub motion_blend: f32,
    /// Color clamp mode.
    pub clamp_mode: ClampMode,
    /// History rejection threshold.
    pub rejection_threshold: f32,
    /// Sharpening after TAA (0-1).
    pub sharpening: f32,
    /// Jitter sequence length.
    pub jitter_sequence: u32,
    /// Enable catmull-rom history sampling.
    pub catmull_rom_history: bool,
}

impl Default for TemporalStabilityConfig {
    fn default() -> Self {
        Self {
            static_blend: 0.95,
            motion_blend: 0.7,
            clamp_mode: ClampMode::VarianceClipping,
            rejection_threshold: 0.1,
            sharpening: 0.2,
            jitter_sequence: 8,
            catmull_rom_history: true,
        }
    }
}

/// Color clamping mode for ghosting rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClampMode {
    /// No clamping (may ghost).
    None,
    /// Min-max clamping (conservative).
    MinMax,
    /// Variance-based clipping (better quality).
    VarianceClipping,
    /// AABB clipping (most aggressive).
    AabbClipping,
}

/// Push constants for temporal stability shader.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct TemporalStabilityPushConstants {
    /// Resolution.
    pub resolution: [f32; 2],
    /// 1/resolution.
    pub texel_size: [f32; 2],
    /// Current jitter offset.
    pub jitter: [f32; 2],
    /// Previous jitter offset.
    pub prev_jitter: [f32; 2],
    /// Static blend factor.
    pub static_blend: f32,
    /// Motion blend factor.
    pub motion_blend: f32,
    /// Rejection threshold.
    pub rejection_threshold: f32,
    /// Sharpening strength.
    pub sharpening: f32,
    /// Clamp mode.
    pub clamp_mode: u32,
    /// Frame index.
    pub frame_index: u32,
    /// Padding.
    pub _padding: [f32; 2],
}

/// GLSL code for temporal stability.
pub mod glsl {
    /// Color space conversion.
    pub const COLOR_SPACE: &str = r#"
// Convert to YCoCg for better clamping
vec3 RGBToYCoCg(vec3 rgb) {
    return vec3(
        0.25 * rgb.r + 0.5 * rgb.g + 0.25 * rgb.b,
        0.5 * rgb.r - 0.5 * rgb.b,
        -0.25 * rgb.r + 0.5 * rgb.g - 0.25 * rgb.b
    );
}

vec3 YCoCgToRGB(vec3 ycocg) {
    return vec3(
        ycocg.x + ycocg.y - ycocg.z,
        ycocg.x + ycocg.z,
        ycocg.x - ycocg.y - ycocg.z
    );
}
"#;

    /// Neighborhood sampling.
    pub const NEIGHBORHOOD: &str = r#"
// Sample 3x3 neighborhood
void sampleNeighborhood(sampler2D tex, vec2 uv, vec2 texelSize,
                         out vec3 minColor, out vec3 maxColor,
                         out vec3 avgColor, out vec3 variance) {
    vec3 sum = vec3(0.0);
    vec3 sumSq = vec3(0.0);
    minColor = vec3(1e10);
    maxColor = vec3(-1e10);

    for (int y = -1; y <= 1; y++) {
        for (int x = -1; x <= 1; x++) {
            vec3 sample = texture(tex, uv + vec2(x, y) * texelSize).rgb;
            vec3 sampleYCoCg = RGBToYCoCg(sample);

            sum += sampleYCoCg;
            sumSq += sampleYCoCg * sampleYCoCg;
            minColor = min(minColor, sampleYCoCg);
            maxColor = max(maxColor, sampleYCoCg);
        }
    }

    avgColor = sum / 9.0;
    variance = sqrt(max(sumSq / 9.0 - avgColor * avgColor, vec3(0.0)));
}
"#;

    /// Variance clipping.
    pub const VARIANCE_CLIPPING: &str = r#"
// Clip history color using variance
vec3 clipVariance(vec3 history, vec3 avg, vec3 variance, float gamma) {
    vec3 minColor = avg - gamma * variance;
    vec3 maxColor = avg + gamma * variance;
    return clamp(history, minColor, maxColor);
}
"#;

    /// AABB clipping.
    pub const AABB_CLIPPING: &str = r#"
// Clip to AABB towards center
vec3 clipAABB(vec3 history, vec3 aabbMin, vec3 aabbMax, vec3 center) {
    vec3 dir = history - center;
    vec3 invDir = 1.0 / (dir + vec3(1e-10));

    vec3 t1 = (aabbMin - center) * invDir;
    vec3 t2 = (aabbMax - center) * invDir;

    vec3 tMin = min(t1, t2);
    vec3 tMax = max(t1, t2);

    float t = max(max(tMin.x, tMin.y), tMin.z);
    t = min(t, 1.0);
    t = max(t, 0.0);

    if (t < 1.0) {
        return center + dir * t;
    }
    return history;
}
"#;

    /// Catmull-Rom history sampling.
    pub const CATMULL_ROM: &str = r#"
// Catmull-Rom filter for sharper history
vec4 sampleHistoryCatmullRom(sampler2D tex, vec2 uv, vec2 texelSize) {
    vec2 samplePos = uv / texelSize;
    vec2 tc = floor(samplePos - 0.5) + 0.5;
    vec2 f = samplePos - tc;

    vec2 w0 = f * (-0.5 + f * (1.0 - 0.5 * f));
    vec2 w1 = 1.0 + f * f * (-2.5 + 1.5 * f);
    vec2 w2 = f * (0.5 + f * (2.0 - 1.5 * f));
    vec2 w3 = f * f * (-0.5 + 0.5 * f);

    vec2 w12 = w1 + w2;
    vec2 tc0 = (tc - 1.0) * texelSize;
    vec2 tc12 = (tc + w2 / w12) * texelSize;
    vec2 tc3 = (tc + 2.0) * texelSize;

    vec4 result =
        texture(tex, vec2(tc0.x, tc0.y)) * w0.x * w0.y +
        texture(tex, vec2(tc12.x, tc0.y)) * w12.x * w0.y +
        texture(tex, vec2(tc3.x, tc0.y)) * w3.x * w0.y +
        texture(tex, vec2(tc0.x, tc12.y)) * w0.x * w12.y +
        texture(tex, vec2(tc12.x, tc12.y)) * w12.x * w12.y +
        texture(tex, vec2(tc3.x, tc12.y)) * w3.x * w12.y +
        texture(tex, vec2(tc0.x, tc3.y)) * w0.x * w3.y +
        texture(tex, vec2(tc12.x, tc3.y)) * w12.x * w3.y +
        texture(tex, vec2(tc3.x, tc3.y)) * w3.x * w3.y;

    return max(result, vec4(0.0));
}
"#;

    /// Motion detection.
    pub const MOTION_DETECTION: &str = r#"
// Detect motion for adaptive blending
float detectMotion(vec2 motion, vec2 texelSize) {
    float motionLength = length(motion);
    // Normalize motion relative to pixel size
    float normalizedMotion = motionLength / length(texelSize);
    return clamp(normalizedMotion * 0.5, 0.0, 1.0);
}
"#;

    /// History validation.
    pub const HISTORY_VALIDATION: &str = r#"
// Validate history sample
float validateHistory(vec3 current, vec3 history, float threshold) {
    vec3 diff = abs(current - history);
    float maxDiff = max(max(diff.r, diff.g), diff.b);

    // Luminance-weighted validation
    float lumCurrent = dot(current, vec3(0.299, 0.587, 0.114));
    float lumHistory = dot(history, vec3(0.299, 0.587, 0.114));
    float lumDiff = abs(lumCurrent - lumHistory);

    float validity = 1.0 - smoothstep(threshold * 0.5, threshold, lumDiff);
    return validity;
}
"#;

    /// Complete TAA fragment shader.
    pub const TAA_FRAGMENT: &str = r#"
#version 450

layout(binding = 0) uniform sampler2D currentFrame;
layout(binding = 1) uniform sampler2D historyFrame;
layout(binding = 2) uniform sampler2D motionVectors;
layout(binding = 3) uniform sampler2D depthBuffer;

layout(push_constant) uniform PushConstants {
    vec2 resolution;
    vec2 texelSize;
    vec2 jitter;
    vec2 prevJitter;
    float staticBlend;
    float motionBlend;
    float rejectionThreshold;
    float sharpening;
    uint clampMode;
    uint frameIndex;
} pc;

layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 fragColor;

// Include helper functions here...

void main() {
    // Unjitter current UV
    vec2 currentUV = uv - pc.jitter * pc.texelSize;

    // Sample current frame
    vec3 current = texture(currentFrame, currentUV).rgb;

    // Get motion vector
    vec2 motion = texture(motionVectors, uv).rg;

    // Sample history with motion compensation
    vec2 historyUV = uv - motion;
    vec3 history = sampleHistoryCatmullRom(historyFrame, historyUV, pc.texelSize).rgb;

    // Sample neighborhood for clamping
    vec3 minColor, maxColor, avgColor, variance;
    sampleNeighborhood(currentFrame, currentUV, pc.texelSize,
                       minColor, maxColor, avgColor, variance);

    // Convert history to YCoCg for clamping
    vec3 historyYCoCg = RGBToYCoCg(history);

    // Apply clamping based on mode
    if (pc.clampMode == 1) {
        // Min-max clamp
        historyYCoCg = clamp(historyYCoCg, minColor, maxColor);
    } else if (pc.clampMode == 2) {
        // Variance clipping
        historyYCoCg = clipVariance(historyYCoCg, avgColor, variance, 1.25);
    } else if (pc.clampMode == 3) {
        // AABB clipping
        historyYCoCg = clipAABB(historyYCoCg, minColor, maxColor, avgColor);
    }

    history = YCoCgToRGB(historyYCoCg);

    // Detect motion for adaptive blending
    float motionFactor = detectMotion(motion, pc.texelSize);

    // Validate history
    float validity = validateHistory(current, history, pc.rejectionThreshold);

    // Compute blend factor
    float blend = mix(pc.staticBlend, pc.motionBlend, motionFactor);
    blend *= validity;

    // Final blend
    vec3 result = mix(current, history, blend);

    // Optional sharpening
    if (pc.sharpening > 0.0) {
        vec3 blur = avgColor;
        result = result + (result - YCoCgToRGB(blur)) * pc.sharpening;
    }

    fragColor = vec4(result, 1.0);
}
"#;
}

/// Jitter sequence generator.
pub struct JitterSequence {
    /// Sequence length.
    length: u32,
    /// Current index.
    index: u32,
    /// Cached jitter values.
    cache: Vec<[f32; 2]>,
}

impl JitterSequence {
    /// Create new jitter sequence.
    pub fn new(length: u32) -> Self {
        let cache: Vec<[f32; 2]> = (0..length)
            .map(|i| Self::halton_2d(i + 1))
            .collect();

        Self {
            length,
            index: 0,
            cache,
        }
    }

    /// Get next jitter value.
    pub fn next(&mut self) -> [f32; 2] {
        let jitter = self.cache[self.index as usize];
        self.index = (self.index + 1) % self.length;
        jitter
    }

    /// Get current jitter without advancing.
    pub fn current(&self) -> [f32; 2] {
        self.cache[self.index as usize]
    }

    /// Get previous jitter.
    pub fn previous(&self) -> [f32; 2] {
        let prev_idx = if self.index == 0 {
            self.length - 1
        } else {
            self.index - 1
        };
        self.cache[prev_idx as usize]
    }

    /// Halton sequence in 2D.
    fn halton_2d(index: u32) -> [f32; 2] {
        [
            Self::halton(index, 2) - 0.5,
            Self::halton(index, 3) - 0.5,
        ]
    }

    /// Halton sequence element.
    fn halton(mut index: u32, base: u32) -> f32 {
        let mut result = 0.0f32;
        let mut f = 1.0 / base as f32;

        while index > 0 {
            result += f * (index % base) as f32;
            index /= base;
            f /= base as f32;
        }

        result
    }

    /// Reset sequence.
    pub fn reset(&mut self) {
        self.index = 0;
    }
}

/// Temporal stability manager.
pub struct TemporalStabilityManager {
    config: TemporalStabilityConfig,
    jitter: JitterSequence,
    frame_index: u32,
}

impl TemporalStabilityManager {
    /// Create new manager.
    pub fn new(config: TemporalStabilityConfig) -> Self {
        let jitter = JitterSequence::new(config.jitter_sequence);

        Self {
            config,
            jitter,
            frame_index: 0,
        }
    }

    /// Get push constants for current frame.
    pub fn get_push_constants(&mut self, width: u32, height: u32) -> TemporalStabilityPushConstants {
        let prev_jitter = self.jitter.current();
        let jitter = self.jitter.next();

        let pc = TemporalStabilityPushConstants {
            resolution: [width as f32, height as f32],
            texel_size: [1.0 / width as f32, 1.0 / height as f32],
            jitter,
            prev_jitter,
            static_blend: self.config.static_blend,
            motion_blend: self.config.motion_blend,
            rejection_threshold: self.config.rejection_threshold,
            sharpening: self.config.sharpening,
            clamp_mode: self.config.clamp_mode as u32,
            frame_index: self.frame_index,
            _padding: [0.0; 2],
        };

        self.frame_index += 1;
        pc
    }

    /// Get current jitter for projection matrix.
    pub fn current_jitter(&self) -> [f32; 2] {
        self.jitter.current()
    }

    /// Update configuration.
    pub fn update_config(&mut self, config: TemporalStabilityConfig) {
        if config.jitter_sequence != self.config.jitter_sequence {
            self.jitter = JitterSequence::new(config.jitter_sequence);
        }
        self.config = config;
    }
}
