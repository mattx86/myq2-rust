//! Video Denoising
//!
//! Temporal and spatial denoising for video/game frames:
//! - Bilateral filtering for edge-preserving blur
//! - Temporal accumulation with motion compensation
//! - Variance-guided filtering
//! - A-trous wavelet denoising

/// Video denoise configuration.
#[derive(Debug, Clone)]
pub struct VideoDenoiseConfig {
    /// Spatial filter strength (0-1).
    pub spatial_strength: f32,
    /// Temporal filter strength (0-1).
    pub temporal_strength: f32,
    /// Edge sensitivity (higher = preserve more edges).
    pub edge_sensitivity: f32,
    /// Number of spatial filter iterations.
    pub iterations: u32,
    /// Filter radius in pixels.
    pub radius: u32,
    /// Enable temporal filtering.
    pub temporal_enabled: bool,
    /// Enable variance-guided filtering.
    pub variance_guided: bool,
    /// Variance threshold.
    pub variance_threshold: f32,
    /// Denoising method.
    pub method: DenoiseMethod,
}

impl Default for VideoDenoiseConfig {
    fn default() -> Self {
        Self {
            spatial_strength: 0.5,
            temporal_strength: 0.9,
            edge_sensitivity: 2.0,
            iterations: 3,
            radius: 3,
            temporal_enabled: true,
            variance_guided: true,
            variance_threshold: 0.01,
            method: DenoiseMethod::Bilateral,
        }
    }
}

/// Denoising method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DenoiseMethod {
    /// Simple box blur.
    Box,
    /// Gaussian blur.
    Gaussian,
    /// Edge-preserving bilateral filter.
    Bilateral,
    /// A-trous wavelet transform.
    ATrous,
    /// Non-local means (expensive).
    NonLocalMeans,
}

impl DenoiseMethod {
    /// Get shader constant.
    pub fn to_shader_value(&self) -> u32 {
        match self {
            DenoiseMethod::Box => 0,
            DenoiseMethod::Gaussian => 1,
            DenoiseMethod::Bilateral => 2,
            DenoiseMethod::ATrous => 3,
            DenoiseMethod::NonLocalMeans => 4,
        }
    }
}

/// Denoise quality presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DenoiseQuality {
    /// Fast - minimal denoising.
    Fast,
    /// Balanced quality and performance.
    Balanced,
    /// High quality denoising.
    High,
    /// Maximum quality (expensive).
    Ultra,
}

impl DenoiseQuality {
    /// Get config for preset.
    pub fn to_config(&self) -> VideoDenoiseConfig {
        match self {
            DenoiseQuality::Fast => VideoDenoiseConfig {
                spatial_strength: 0.3,
                iterations: 1,
                radius: 2,
                method: DenoiseMethod::Box,
                ..Default::default()
            },
            DenoiseQuality::Balanced => VideoDenoiseConfig::default(),
            DenoiseQuality::High => VideoDenoiseConfig {
                spatial_strength: 0.6,
                iterations: 4,
                radius: 4,
                method: DenoiseMethod::ATrous,
                ..Default::default()
            },
            DenoiseQuality::Ultra => VideoDenoiseConfig {
                spatial_strength: 0.7,
                iterations: 5,
                radius: 5,
                method: DenoiseMethod::NonLocalMeans,
                ..Default::default()
            },
        }
    }
}

/// Push constants for denoise shader.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DenoisePushConstants {
    /// Resolution.
    pub resolution: [f32; 2],
    /// Texel size.
    pub texel_size: [f32; 2],
    /// Spatial strength.
    pub spatial_strength: f32,
    /// Temporal strength.
    pub temporal_strength: f32,
    /// Edge sensitivity.
    pub edge_sensitivity: f32,
    /// Variance threshold.
    pub variance_threshold: f32,
    /// Filter radius.
    pub radius: i32,
    /// Method.
    pub method: u32,
    /// Iteration index (for A-trous).
    pub iteration: u32,
    /// Temporal enabled.
    pub temporal_enabled: u32,
}

/// GLSL code for video denoising.
pub mod glsl {
    /// Bilateral filter.
    pub const BILATERAL: &str = r#"
// Bilateral filter - edge-preserving blur
vec3 bilateralFilter(sampler2D tex, vec2 uv, vec2 texelSize, int radius,
                      float spatialSigma, float rangeSigma) {
    vec3 center = texture(tex, uv).rgb;
    float centerLum = dot(center, vec3(0.299, 0.587, 0.114));

    vec3 sum = vec3(0.0);
    float weightSum = 0.0;

    float spatialFactor = -0.5 / (spatialSigma * spatialSigma);
    float rangeFactor = -0.5 / (rangeSigma * rangeSigma);

    for (int y = -radius; y <= radius; y++) {
        for (int x = -radius; x <= radius; x++) {
            vec2 offset = vec2(x, y);
            vec2 sampleUV = uv + offset * texelSize;

            vec3 sample = texture(tex, sampleUV).rgb;
            float sampleLum = dot(sample, vec3(0.299, 0.587, 0.114));

            // Spatial weight
            float spatialDist = dot(offset, offset);
            float spatialWeight = exp(spatialDist * spatialFactor);

            // Range weight
            float rangeDist = (sampleLum - centerLum) * (sampleLum - centerLum);
            float rangeWeight = exp(rangeDist * rangeFactor);

            float weight = spatialWeight * rangeWeight;
            sum += sample * weight;
            weightSum += weight;
        }
    }

    return sum / max(weightSum, 0.0001);
}
"#;

    /// A-trous wavelet filter.
    pub const ATROUS: &str = r#"
// A-trous wavelet filter kernel
const float kernel[25] = float[](
    1.0/256.0, 4.0/256.0, 6.0/256.0, 4.0/256.0, 1.0/256.0,
    4.0/256.0, 16.0/256.0, 24.0/256.0, 16.0/256.0, 4.0/256.0,
    6.0/256.0, 24.0/256.0, 36.0/256.0, 24.0/256.0, 6.0/256.0,
    4.0/256.0, 16.0/256.0, 24.0/256.0, 16.0/256.0, 4.0/256.0,
    1.0/256.0, 4.0/256.0, 6.0/256.0, 4.0/256.0, 1.0/256.0
);

vec3 atrousFilter(sampler2D tex, vec2 uv, vec2 texelSize, int stepSize,
                   float colorSigma, float normalSigma, float depthSigma) {
    vec3 center = texture(tex, uv).rgb;
    float centerLum = dot(center, vec3(0.299, 0.587, 0.114));

    vec3 sum = vec3(0.0);
    float weightSum = 0.0;

    int idx = 0;
    for (int y = -2; y <= 2; y++) {
        for (int x = -2; x <= 2; x++) {
            vec2 offset = vec2(x, y) * float(stepSize);
            vec2 sampleUV = uv + offset * texelSize;

            vec3 sample = texture(tex, sampleUV).rgb;
            float sampleLum = dot(sample, vec3(0.299, 0.587, 0.114));

            // Edge-stopping weight based on luminance difference
            float lumDiff = abs(sampleLum - centerLum);
            float edgeWeight = exp(-lumDiff * lumDiff / (colorSigma * colorSigma));

            float weight = kernel[idx] * edgeWeight;
            sum += sample * weight;
            weightSum += weight;

            idx++;
        }
    }

    return sum / max(weightSum, 0.0001);
}
"#;

    /// Non-local means.
    pub const NON_LOCAL_MEANS: &str = r#"
// Non-local means denoising
vec3 nonLocalMeans(sampler2D tex, vec2 uv, vec2 texelSize, int searchRadius,
                    int patchRadius, float h) {
    vec3 center = texture(tex, uv).rgb;

    vec3 sum = vec3(0.0);
    float weightSum = 0.0;

    float h2 = h * h;

    for (int sy = -searchRadius; sy <= searchRadius; sy++) {
        for (int sx = -searchRadius; sx <= searchRadius; sx++) {
            vec2 searchOffset = vec2(sx, sy) * texelSize;
            vec2 searchUV = uv + searchOffset;

            // Compare patches
            float patchDist = 0.0;
            float patchCount = 0.0;

            for (int py = -patchRadius; py <= patchRadius; py++) {
                for (int px = -patchRadius; px <= patchRadius; px++) {
                    vec2 patchOffset = vec2(px, py) * texelSize;

                    vec3 p1 = texture(tex, uv + patchOffset).rgb;
                    vec3 p2 = texture(tex, searchUV + patchOffset).rgb;

                    vec3 diff = p1 - p2;
                    patchDist += dot(diff, diff);
                    patchCount += 1.0;
                }
            }

            patchDist /= patchCount;

            // Weight based on patch similarity
            float weight = exp(-patchDist / h2);

            vec3 sample = texture(tex, searchUV).rgb;
            sum += sample * weight;
            weightSum += weight;
        }
    }

    return sum / max(weightSum, 0.0001);
}
"#;

    /// Temporal accumulation.
    pub const TEMPORAL: &str = r#"
// Temporal accumulation with motion compensation
vec3 temporalAccumulate(
    sampler2D currentTex,
    sampler2D historyTex,
    sampler2D motionTex,
    vec2 uv,
    vec2 texelSize,
    float blend
) {
    vec3 current = texture(currentTex, uv).rgb;

    // Get motion vector
    vec2 motion = texture(motionTex, uv).rg;
    vec2 historyUV = uv - motion;

    // Check if history is valid
    if (historyUV.x < 0.0 || historyUV.x > 1.0 ||
        historyUV.y < 0.0 || historyUV.y > 1.0) {
        return current;
    }

    vec3 history = texture(historyTex, historyUV).rgb;

    // Color clamping to reject ghosting
    vec3 minColor = current;
    vec3 maxColor = current;

    for (int y = -1; y <= 1; y++) {
        for (int x = -1; x <= 1; x++) {
            vec3 neighbor = texture(currentTex, uv + vec2(x, y) * texelSize).rgb;
            minColor = min(minColor, neighbor);
            maxColor = max(maxColor, neighbor);
        }
    }

    history = clamp(history, minColor, maxColor);

    // Blend
    return mix(current, history, blend);
}
"#;

    /// Variance estimation.
    pub const VARIANCE: &str = r#"
// Estimate local variance for adaptive filtering
float estimateVariance(sampler2D tex, vec2 uv, vec2 texelSize, int radius) {
    vec3 sum = vec3(0.0);
    vec3 sumSq = vec3(0.0);
    float count = 0.0;

    for (int y = -radius; y <= radius; y++) {
        for (int x = -radius; x <= radius; x++) {
            vec3 sample = texture(tex, uv + vec2(x, y) * texelSize).rgb;
            sum += sample;
            sumSq += sample * sample;
            count += 1.0;
        }
    }

    vec3 mean = sum / count;
    vec3 variance = sumSq / count - mean * mean;

    return dot(variance, vec3(0.299, 0.587, 0.114));
}

// Variance-guided filter strength
float varianceGuidedStrength(float variance, float threshold, float baseStrength) {
    // More filtering in noisy areas, less in clean areas
    float noiseFactor = smoothstep(0.0, threshold, variance);
    return baseStrength * (0.5 + noiseFactor * 0.5);
}
"#;

    /// Complete denoise compute shader.
    pub const DENOISE_COMPUTE: &str = r#"
#version 450

layout(binding = 0) uniform sampler2D inputTex;
layout(binding = 1) uniform sampler2D historyTex;
layout(binding = 2) uniform sampler2D motionTex;
layout(binding = 3, rgba16f) uniform writeonly image2D outputTex;

layout(push_constant) uniform PushConstants {
    vec2 resolution;
    vec2 texelSize;
    float spatialStrength;
    float temporalStrength;
    float edgeSensitivity;
    float varianceThreshold;
    int radius;
    uint method;
    uint iteration;
    uint temporalEnabled;
} pc;

layout(local_size_x = 8, local_size_y = 8) in;

// Include helper functions...

void main() {
    ivec2 pos = ivec2(gl_GlobalInvocationID.xy);
    vec2 uv = (vec2(pos) + 0.5) / pc.resolution;

    vec3 color = texture(inputTex, uv).rgb;

    // Estimate variance for adaptive filtering
    float variance = estimateVariance(inputTex, uv, pc.texelSize, 2);
    float strength = varianceGuidedStrength(variance, pc.varianceThreshold, pc.spatialStrength);

    // Apply spatial filter based on method
    vec3 filtered;
    if (pc.method == 0) {
        // Box filter (not implemented here for brevity)
        filtered = color;
    } else if (pc.method == 1) {
        // Gaussian (not implemented here)
        filtered = color;
    } else if (pc.method == 2) {
        // Bilateral
        filtered = bilateralFilter(inputTex, uv, pc.texelSize, pc.radius,
                                   float(pc.radius), pc.edgeSensitivity);
    } else if (pc.method == 3) {
        // A-trous
        int stepSize = 1 << pc.iteration;
        filtered = atrousFilter(inputTex, uv, pc.texelSize, stepSize,
                                pc.edgeSensitivity, 0.1, 0.1);
    } else {
        // Non-local means
        filtered = nonLocalMeans(inputTex, uv, pc.texelSize, 5, 2, pc.edgeSensitivity);
    }

    // Mix based on strength
    color = mix(color, filtered, strength);

    // Temporal accumulation
    if (pc.temporalEnabled != 0) {
        color = temporalAccumulate(inputTex, historyTex, motionTex, uv,
                                   pc.texelSize, pc.temporalStrength);
    }

    imageStore(outputTex, pos, vec4(color, 1.0));
}
"#;
}

/// Video denoise manager.
pub struct VideoDenoiseManager {
    config: VideoDenoiseConfig,
}

impl VideoDenoiseManager {
    /// Create new manager.
    pub fn new(config: VideoDenoiseConfig) -> Self {
        Self { config }
    }

    /// Create with quality preset.
    pub fn with_quality(quality: DenoiseQuality) -> Self {
        Self::new(quality.to_config())
    }

    /// Get push constants.
    pub fn get_push_constants(&self, width: u32, height: u32, iteration: u32) -> DenoisePushConstants {
        DenoisePushConstants {
            resolution: [width as f32, height as f32],
            texel_size: [1.0 / width as f32, 1.0 / height as f32],
            spatial_strength: self.config.spatial_strength,
            temporal_strength: self.config.temporal_strength,
            edge_sensitivity: self.config.edge_sensitivity,
            variance_threshold: self.config.variance_threshold,
            radius: self.config.radius as i32,
            method: self.config.method.to_shader_value(),
            iteration,
            temporal_enabled: if self.config.temporal_enabled { 1 } else { 0 },
        }
    }

    /// Get number of iterations needed.
    pub fn iterations(&self) -> u32 {
        self.config.iterations
    }

    /// Set configuration.
    pub fn set_config(&mut self, config: VideoDenoiseConfig) {
        self.config = config;
    }

    /// Get configuration.
    pub fn config(&self) -> &VideoDenoiseConfig {
        &self.config
    }
}
