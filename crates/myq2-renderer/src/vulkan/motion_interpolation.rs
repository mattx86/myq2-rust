//! Motion Compensated Frame Interpolation
//!
//! Generate intermediate frames using motion estimation:
//! - Block-matching motion estimation
//! - Optical flow computation
//! - Bidirectional motion compensation
//! - Occlusion handling
//! - Artifact reduction
//! - GPU-accelerated frame interpolation with Vulkan compute pipelines

use ash::vk;

/// Motion interpolation configuration (for motion estimation pipeline).
#[derive(Debug, Clone)]
pub struct MotionInterpConfig {
    /// Motion estimation block size.
    pub block_size: u32,
    /// Search radius for motion estimation.
    pub search_radius: u32,
    /// Number of pyramid levels for hierarchical estimation.
    pub pyramid_levels: u32,
    /// Bidirectional estimation (better quality, slower).
    pub bidirectional: bool,
    /// Occlusion detection threshold.
    pub occlusion_threshold: f32,
    /// Motion smoothing strength.
    pub motion_smoothing: f32,
    /// Artifact reduction strength.
    pub artifact_reduction: f32,
    /// Interpolation method.
    pub method: InterpMethod,
}

impl Default for MotionInterpConfig {
    fn default() -> Self {
        Self {
            block_size: 8,
            search_radius: 16,
            pyramid_levels: 4,
            bidirectional: true,
            occlusion_threshold: 0.1,
            motion_smoothing: 0.5,
            artifact_reduction: 0.3,
            method: InterpMethod::Bidirectional,
        }
    }
}

/// Frame interpolation configuration (for GPU compute interpolator).
#[derive(Debug, Clone)]
pub struct FrameInterpConfig {
    /// Whether interpolation is enabled.
    pub enabled: bool,
    /// Quality preset.
    pub quality: InterpQuality,
    /// Target multiplier (2 = double frame rate).
    pub multiplier: u32,
    /// Motion vector scale.
    pub mv_scale: f32,
    /// Blend weight for generated frames.
    pub blend_weight: f32,
    /// Occlusion detection threshold.
    pub occlusion_threshold: f32,
}

impl Default for FrameInterpConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            quality: InterpQuality::Balanced,
            multiplier: 2,
            mv_scale: 1.0,
            blend_weight: 0.5,
            occlusion_threshold: 0.1,
        }
    }
}

/// Interpolation method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterpMethod {
    /// Simple blend (no motion compensation).
    Blend,
    /// Forward motion compensation only.
    Forward,
    /// Backward motion compensation only.
    Backward,
    /// Bidirectional motion compensation.
    Bidirectional,
    /// Adaptive based on occlusion.
    Adaptive,
}

impl InterpMethod {
    /// Get shader constant.
    pub fn to_shader_value(&self) -> u32 {
        match self {
            InterpMethod::Blend => 0,
            InterpMethod::Forward => 1,
            InterpMethod::Backward => 2,
            InterpMethod::Bidirectional => 3,
            InterpMethod::Adaptive => 4,
        }
    }
}

/// Interpolation quality presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterpQuality {
    /// Fast - simple blending.
    Fast,
    /// Balanced - forward motion only.
    Balanced,
    /// High - bidirectional.
    High,
    /// Ultra - adaptive with occlusion handling.
    Ultra,
}

impl InterpQuality {
    /// Get config for preset.
    pub fn to_config(&self) -> MotionInterpConfig {
        match self {
            InterpQuality::Fast => MotionInterpConfig {
                method: InterpMethod::Blend,
                pyramid_levels: 2,
                ..Default::default()
            },
            InterpQuality::Balanced => MotionInterpConfig {
                method: InterpMethod::Forward,
                bidirectional: false,
                pyramid_levels: 3,
                ..Default::default()
            },
            InterpQuality::High => MotionInterpConfig::default(),
            InterpQuality::Ultra => MotionInterpConfig {
                block_size: 4,
                search_radius: 24,
                pyramid_levels: 5,
                method: InterpMethod::Adaptive,
                ..Default::default()
            },
        }
    }
}

/// Push constants for motion estimation shader.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct MotionEstimationPushConstants {
    /// Resolution.
    pub resolution: [f32; 2],
    /// Texel size.
    pub texel_size: [f32; 2],
    /// Block size.
    pub block_size: i32,
    /// Search radius.
    pub search_radius: i32,
    /// Current pyramid level.
    pub level: i32,
    /// Padding.
    pub _padding: i32,
}

/// Push constants for motion-compensated interpolation shader.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct InterpPushConstants {
    /// Resolution.
    pub resolution: [f32; 2],
    /// Texel size.
    pub texel_size: [f32; 2],
    /// Interpolation factor (0-1, 0.5 = middle frame).
    pub t: f32,
    /// Occlusion threshold.
    pub occlusion_threshold: f32,
    /// Motion smoothing.
    pub motion_smoothing: f32,
    /// Artifact reduction.
    pub artifact_reduction: f32,
    /// Interpolation method.
    pub method: u32,
    /// Padding.
    pub _padding: [u32; 3],
}

/// Push constants for the GPU frame interpolator's compute shader.
#[repr(C)]
#[derive(Clone, Copy)]
struct FrameInterpPushConstants {
    t: f32,
    mv_scale: f32,
    blend_weight: f32,
    occlusion_threshold: f32,
}

/// Frame data for GPU-accelerated interpolation.
#[derive(Clone)]
pub struct FrameData {
    /// Frame index.
    pub index: u64,
    /// Frame timestamp in microseconds.
    pub timestamp_us: u64,
    /// Color image.
    pub color: vk::Image,
    /// Depth image.
    pub depth: vk::Image,
    /// Motion vectors image.
    pub motion_vectors: vk::Image,
    /// View-projection matrix.
    pub view_proj: [[f32; 4]; 4],
    /// Inverse view-projection matrix.
    pub inv_view_proj: [[f32; 4]; 4],
}

/// GLSL code for motion interpolation.
pub mod glsl {
    /// Block matching motion estimation.
    pub const BLOCK_MATCHING: &str = r#"
// Sum of absolute differences
float SAD(sampler2D tex1, sampler2D tex2, vec2 uv1, vec2 uv2,
          vec2 texelSize, int blockSize) {
    float sad = 0.0;

    for (int y = -blockSize/2; y <= blockSize/2; y++) {
        for (int x = -blockSize/2; x <= blockSize/2; x++) {
            vec2 offset = vec2(x, y) * texelSize;
            vec3 c1 = texture(tex1, uv1 + offset).rgb;
            vec3 c2 = texture(tex2, uv2 + offset).rgb;
            sad += dot(abs(c1 - c2), vec3(1.0));
        }
    }

    return sad;
}

// Diamond search pattern
vec2 diamondSearch(sampler2D frame0, sampler2D frame1, vec2 uv,
                    vec2 texelSize, int blockSize, int searchRadius) {
    vec2 bestMV = vec2(0.0);
    float bestSAD = SAD(frame0, frame1, uv, uv, texelSize, blockSize);

    // Large diamond pattern
    const vec2 largeDiamond[9] = vec2[](
        vec2(0, 0), vec2(0, -2), vec2(0, 2),
        vec2(-2, 0), vec2(2, 0), vec2(-1, -1),
        vec2(1, -1), vec2(-1, 1), vec2(1, 1)
    );

    // Small diamond pattern
    const vec2 smallDiamond[5] = vec2[](
        vec2(0, 0), vec2(0, -1), vec2(0, 1),
        vec2(-1, 0), vec2(1, 0)
    );

    vec2 center = vec2(0.0);

    // Large diamond search
    for (int iter = 0; iter < searchRadius; iter++) {
        vec2 bestPoint = center;

        for (int i = 0; i < 9; i++) {
            vec2 searchPoint = center + largeDiamond[i] * texelSize;
            float sad = SAD(frame0, frame1, uv, uv + searchPoint, texelSize, blockSize);

            if (sad < bestSAD) {
                bestSAD = sad;
                bestPoint = searchPoint;
                bestMV = searchPoint;
            }
        }

        if (bestPoint == center) break;
        center = bestPoint;
    }

    // Small diamond refinement
    for (int iter = 0; iter < 2; iter++) {
        vec2 bestPoint = center;

        for (int i = 0; i < 5; i++) {
            vec2 searchPoint = center + smallDiamond[i] * texelSize;
            float sad = SAD(frame0, frame1, uv, uv + searchPoint, texelSize, blockSize);

            if (sad < bestSAD) {
                bestSAD = sad;
                bestPoint = searchPoint;
                bestMV = searchPoint;
            }
        }

        if (bestPoint == center) break;
        center = bestPoint;
    }

    return bestMV;
}
"#;

    /// Optical flow using Lucas-Kanade.
    pub const OPTICAL_FLOW: &str = r#"
// Spatial gradient
vec2 computeGradient(sampler2D tex, vec2 uv, vec2 texelSize) {
    float left = dot(texture(tex, uv - vec2(texelSize.x, 0)).rgb, vec3(0.299, 0.587, 0.114));
    float right = dot(texture(tex, uv + vec2(texelSize.x, 0)).rgb, vec3(0.299, 0.587, 0.114));
    float up = dot(texture(tex, uv - vec2(0, texelSize.y)).rgb, vec3(0.299, 0.587, 0.114));
    float down = dot(texture(tex, uv + vec2(0, texelSize.y)).rgb, vec3(0.299, 0.587, 0.114));

    return vec2(right - left, down - up) * 0.5;
}

// Temporal gradient
float computeTemporalGradient(sampler2D tex0, sampler2D tex1, vec2 uv) {
    float lum0 = dot(texture(tex0, uv).rgb, vec3(0.299, 0.587, 0.114));
    float lum1 = dot(texture(tex1, uv).rgb, vec3(0.299, 0.587, 0.114));
    return lum1 - lum0;
}

// Lucas-Kanade optical flow (simplified)
vec2 lucasKanade(sampler2D frame0, sampler2D frame1, vec2 uv,
                  vec2 texelSize, int windowSize) {
    float sumIxIx = 0.0;
    float sumIyIy = 0.0;
    float sumIxIy = 0.0;
    float sumIxIt = 0.0;
    float sumIyIt = 0.0;

    for (int y = -windowSize/2; y <= windowSize/2; y++) {
        for (int x = -windowSize/2; x <= windowSize/2; x++) {
            vec2 offset = vec2(x, y) * texelSize;
            vec2 grad = computeGradient(frame0, uv + offset, texelSize);
            float It = computeTemporalGradient(frame0, frame1, uv + offset);

            sumIxIx += grad.x * grad.x;
            sumIyIy += grad.y * grad.y;
            sumIxIy += grad.x * grad.y;
            sumIxIt += grad.x * It;
            sumIyIt += grad.y * It;
        }
    }

    // Solve 2x2 system
    float det = sumIxIx * sumIyIy - sumIxIy * sumIxIy;

    if (abs(det) < 0.0001) {
        return vec2(0.0);
    }

    float vx = (sumIyIy * sumIxIt - sumIxIy * sumIyIt) / det;
    float vy = (sumIxIx * sumIyIt - sumIxIy * sumIxIt) / det;

    return -vec2(vx, vy);
}
"#;

    /// Motion vector filtering.
    pub const MOTION_FILTERING: &str = r#"
// Median filter for motion vectors
vec2 medianFilterMV(sampler2D mvTex, vec2 uv, vec2 texelSize) {
    vec2 samples[9];
    int idx = 0;

    for (int y = -1; y <= 1; y++) {
        for (int x = -1; x <= 1; x++) {
            samples[idx++] = texture(mvTex, uv + vec2(x, y) * texelSize).rg;
        }
    }

    // Simple bubble sort by magnitude
    for (int i = 0; i < 8; i++) {
        for (int j = i + 1; j < 9; j++) {
            if (length(samples[i]) > length(samples[j])) {
                vec2 temp = samples[i];
                samples[i] = samples[j];
                samples[j] = temp;
            }
        }
    }

    return samples[4]; // Median
}

// Bilateral filter for motion vectors
vec2 bilateralFilterMV(sampler2D mvTex, sampler2D colorTex, vec2 uv,
                        vec2 texelSize, float spatialSigma, float colorSigma) {
    vec2 centerMV = texture(mvTex, uv).rg;
    vec3 centerColor = texture(colorTex, uv).rgb;

    vec2 sum = vec2(0.0);
    float weightSum = 0.0;

    for (int y = -2; y <= 2; y++) {
        for (int x = -2; x <= 2; x++) {
            vec2 offset = vec2(x, y);
            vec2 sampleUV = uv + offset * texelSize;

            vec2 sampleMV = texture(mvTex, sampleUV).rg;
            vec3 sampleColor = texture(colorTex, sampleUV).rgb;

            float spatialWeight = exp(-dot(offset, offset) / (2.0 * spatialSigma * spatialSigma));
            float colorDiff = length(sampleColor - centerColor);
            float colorWeight = exp(-colorDiff * colorDiff / (2.0 * colorSigma * colorSigma));

            float weight = spatialWeight * colorWeight;
            sum += sampleMV * weight;
            weightSum += weight;
        }
    }

    return sum / max(weightSum, 0.0001);
}
"#;

    /// Occlusion detection.
    pub const OCCLUSION: &str = r#"
// Forward-backward consistency check
float checkOcclusion(sampler2D fwdMV, sampler2D bwdMV, vec2 uv,
                      vec2 texelSize, float threshold) {
    vec2 mv_fwd = texture(fwdMV, uv).rg;
    vec2 reprojUV = uv + mv_fwd;

    if (reprojUV.x < 0.0 || reprojUV.x > 1.0 ||
        reprojUV.y < 0.0 || reprojUV.y > 1.0) {
        return 1.0; // Out of bounds = occluded
    }

    vec2 mv_bwd = texture(bwdMV, reprojUV).rg;

    // Forward + backward should be near zero for consistent motion
    vec2 residual = mv_fwd + mv_bwd;
    float error = length(residual) / (length(mv_fwd) + 0.001);

    return smoothstep(threshold * 0.5, threshold, error);
}
"#;

    /// Frame interpolation.
    pub const INTERPOLATION: &str = r#"
// Simple blend interpolation
vec3 blendInterp(sampler2D frame0, sampler2D frame1, vec2 uv, float t) {
    vec3 c0 = texture(frame0, uv).rgb;
    vec3 c1 = texture(frame1, uv).rgb;
    return mix(c0, c1, t);
}

// Forward motion compensation
vec3 forwardInterp(sampler2D frame0, sampler2D mvTex, vec2 uv,
                    vec2 texelSize, float t) {
    vec2 mv = texture(mvTex, uv).rg;
    vec2 srcUV = uv - mv * t;
    return texture(frame0, srcUV).rgb;
}

// Backward motion compensation
vec3 backwardInterp(sampler2D frame1, sampler2D mvTex, vec2 uv,
                     vec2 texelSize, float t) {
    vec2 mv = texture(mvTex, uv).rg;
    vec2 srcUV = uv + mv * (1.0 - t);
    return texture(frame1, srcUV).rgb;
}

// Bidirectional interpolation
vec3 bidirectionalInterp(sampler2D frame0, sampler2D frame1,
                          sampler2D fwdMV, sampler2D bwdMV,
                          vec2 uv, vec2 texelSize, float t) {
    // Sample from frame0 moving forward
    vec2 mv_fwd = texture(fwdMV, uv).rg;
    vec2 uv0 = uv - mv_fwd * t;
    vec3 c0 = texture(frame0, uv0).rgb;

    // Sample from frame1 moving backward
    vec2 mv_bwd = texture(bwdMV, uv).rg;
    vec2 uv1 = uv + mv_bwd * (1.0 - t);
    vec3 c1 = texture(frame1, uv1).rgb;

    // Blend based on temporal position
    return mix(c0, c1, t);
}

// Adaptive interpolation with occlusion handling
vec3 adaptiveInterp(sampler2D frame0, sampler2D frame1,
                     sampler2D fwdMV, sampler2D bwdMV,
                     vec2 uv, vec2 texelSize, float t, float occlusionThreshold) {
    // Check occlusion
    float occ_fwd = checkOcclusion(fwdMV, bwdMV, uv, texelSize, occlusionThreshold);
    float occ_bwd = checkOcclusion(bwdMV, fwdMV, uv, texelSize, occlusionThreshold);

    // Sample from both frames
    vec2 mv_fwd = texture(fwdMV, uv).rg;
    vec2 mv_bwd = texture(bwdMV, uv).rg;

    vec2 uv0 = uv - mv_fwd * t;
    vec2 uv1 = uv + mv_bwd * (1.0 - t);

    vec3 c0 = texture(frame0, uv0).rgb;
    vec3 c1 = texture(frame1, uv1).rgb;

    // Weight based on occlusion
    float w0 = (1.0 - occ_fwd) * (1.0 - t);
    float w1 = (1.0 - occ_bwd) * t;

    if (w0 + w1 < 0.001) {
        // Both occluded - fallback to simple blend
        return blendInterp(frame0, frame1, uv, t);
    }

    return (c0 * w0 + c1 * w1) / (w0 + w1);
}
"#;

    /// Artifact reduction.
    pub const ARTIFACT_REDUCTION: &str = r#"
// Detect and reduce interpolation artifacts
vec3 reduceArtifacts(vec3 interpolated, sampler2D frame0, sampler2D frame1,
                      vec2 uv, vec2 texelSize, float strength) {
    // Check for ghosting/halo by comparing with original frames
    vec3 c0 = texture(frame0, uv).rgb;
    vec3 c1 = texture(frame1, uv).rgb;

    // Compute range
    vec3 minColor = min(c0, c1);
    vec3 maxColor = max(c0, c1);

    // Extend range slightly
    vec3 range = maxColor - minColor;
    minColor -= range * 0.1;
    maxColor += range * 0.1;

    // Clamp interpolated to valid range
    vec3 clamped = clamp(interpolated, minColor, maxColor);

    // Blend clamped with interpolated based on strength
    return mix(interpolated, clamped, strength);
}
"#;

    /// Complete motion estimation compute shader.
    pub const MOTION_ESTIMATION_COMPUTE: &str = r#"
#version 450

layout(binding = 0) uniform sampler2D frame0;
layout(binding = 1) uniform sampler2D frame1;
layout(binding = 2) uniform sampler2D prevMV; // From previous pyramid level
layout(binding = 3, rg16f) uniform writeonly image2D motionVectors;

layout(push_constant) uniform PushConstants {
    vec2 resolution;
    vec2 texelSize;
    int blockSize;
    int searchRadius;
    int level;
} pc;

layout(local_size_x = 8, local_size_y = 8) in;

// Include helper functions...

void main() {
    ivec2 pos = ivec2(gl_GlobalInvocationID.xy);
    vec2 uv = (vec2(pos) + 0.5) / pc.resolution;

    // Get initial guess from previous level
    vec2 initialMV = vec2(0.0);
    if (pc.level > 0) {
        initialMV = texture(prevMV, uv).rg * 2.0; // Scale up
    }

    // Refine with diamond search
    vec2 refinedMV = diamondSearch(frame0, frame1, uv, pc.texelSize,
                                    pc.blockSize, pc.searchRadius);

    vec2 mv = initialMV + refinedMV;

    imageStore(motionVectors, pos, vec4(mv, 0.0, 0.0));
}
"#;

    /// Complete interpolation compute shader.
    pub const INTERPOLATION_COMPUTE: &str = r#"
#version 450

layout(binding = 0) uniform sampler2D frame0;
layout(binding = 1) uniform sampler2D frame1;
layout(binding = 2) uniform sampler2D fwdMV;
layout(binding = 3) uniform sampler2D bwdMV;
layout(binding = 4, rgba16f) uniform writeonly image2D interpolated;

layout(push_constant) uniform PushConstants {
    vec2 resolution;
    vec2 texelSize;
    float t;
    float occlusionThreshold;
    float motionSmoothing;
    float artifactReduction;
    uint method;
} pc;

layout(local_size_x = 8, local_size_y = 8) in;

// Include helper functions...

void main() {
    ivec2 pos = ivec2(gl_GlobalInvocationID.xy);
    vec2 uv = (vec2(pos) + 0.5) / pc.resolution;

    vec3 result;

    if (pc.method == 0) {
        result = blendInterp(frame0, frame1, uv, pc.t);
    } else if (pc.method == 1) {
        result = forwardInterp(frame0, fwdMV, uv, pc.texelSize, pc.t);
    } else if (pc.method == 2) {
        result = backwardInterp(frame1, bwdMV, uv, pc.texelSize, pc.t);
    } else if (pc.method == 3) {
        result = bidirectionalInterp(frame0, frame1, fwdMV, bwdMV,
                                      uv, pc.texelSize, pc.t);
    } else {
        result = adaptiveInterp(frame0, frame1, fwdMV, bwdMV,
                                 uv, pc.texelSize, pc.t, pc.occlusionThreshold);
    }

    // Artifact reduction
    if (pc.artifactReduction > 0.0) {
        result = reduceArtifacts(result, frame0, frame1, uv,
                                  pc.texelSize, pc.artifactReduction);
    }

    imageStore(interpolated, pos, vec4(result, 1.0));
}
"#;

    /// GLSL compute shader for GPU frame interpolation using external motion vectors.
    ///
    /// This shader uses pre-computed motion vectors (e.g., from the engine) rather than
    /// estimating motion via block matching. It performs motion-compensated warping with
    /// depth-based occlusion detection.
    pub const FRAME_INTERP_COMPUTE: &str = r#"
#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(local_size_x = 8, local_size_y = 8, local_size_z = 1) in;

layout(set = 0, binding = 0) uniform sampler2D prev_frame;
layout(set = 0, binding = 1) uniform sampler2D curr_frame;
layout(set = 0, binding = 2) uniform sampler2D motion_vectors;
layout(set = 0, binding = 3) uniform sampler2D depth;
layout(set = 0, binding = 4, rgba16f) writeonly uniform image2D output_frame;
layout(set = 0, binding = 5, r8) writeonly uniform image2D occlusion_mask;

layout(push_constant) uniform PushConstants {
    float t;           // Interpolation factor (0 = prev, 1 = curr)
    float mv_scale;    // Motion vector scale
    float blend_weight;
    float occlusion_threshold;
};

void main() {
    ivec2 coord = ivec2(gl_GlobalInvocationID.xy);
    ivec2 size = imageSize(output_frame);

    if (coord.x >= size.x || coord.y >= size.y) {
        return;
    }

    vec2 uv = (vec2(coord) + 0.5) / vec2(size);

    // Sample motion vectors (screen-space displacement)
    vec2 mv = texture(motion_vectors, uv).xy * mv_scale;

    // Compute warped UVs for both frames
    vec2 uv_prev = uv + mv * t;
    vec2 uv_curr = uv - mv * (1.0 - t);

    // Sample both frames at warped positions
    vec4 color_prev = texture(prev_frame, uv_prev);
    vec4 color_curr = texture(curr_frame, uv_curr);

    // Detect occlusion based on depth difference
    float depth_prev = texture(depth, uv_prev).r;
    float depth_curr = texture(depth, uv_curr).r;
    float depth_diff = abs(depth_prev - depth_curr);
    float occlusion = step(occlusion_threshold, depth_diff);

    // Blend frames with occlusion awareness
    float weight = blend_weight;
    if (occlusion > 0.5) {
        // In occluded regions, prefer the closer frame
        weight = depth_prev < depth_curr ? 0.0 : 1.0;
    }

    vec4 interp_color = mix(color_prev, color_curr, mix(t, weight, occlusion));

    imageStore(output_frame, coord, interp_color);
    imageStore(occlusion_mask, coord, vec4(occlusion));
}
"#;
}

/// Motion interpolation manager.
pub struct MotionInterpolationManager {
    config: MotionInterpConfig,
}

impl MotionInterpolationManager {
    /// Create new manager.
    pub fn new(config: MotionInterpConfig) -> Self {
        Self { config }
    }

    /// Create with quality preset.
    pub fn with_quality(quality: InterpQuality) -> Self {
        Self::new(quality.to_config())
    }

    /// Get motion estimation push constants.
    pub fn get_motion_estimation_constants(
        &self,
        width: u32,
        height: u32,
        level: u32,
    ) -> MotionEstimationPushConstants {
        // Scale for pyramid level
        let scale = 1 << level;
        let w = width / scale;
        let h = height / scale;

        MotionEstimationPushConstants {
            resolution: [w as f32, h as f32],
            texel_size: [1.0 / w as f32, 1.0 / h as f32],
            block_size: self.config.block_size as i32,
            search_radius: self.config.search_radius as i32,
            level: level as i32,
            _padding: 0,
        }
    }

    /// Get interpolation push constants.
    pub fn get_interp_constants(&self, width: u32, height: u32, t: f32) -> InterpPushConstants {
        InterpPushConstants {
            resolution: [width as f32, height as f32],
            texel_size: [1.0 / width as f32, 1.0 / height as f32],
            t,
            occlusion_threshold: self.config.occlusion_threshold,
            motion_smoothing: self.config.motion_smoothing,
            artifact_reduction: self.config.artifact_reduction,
            method: self.config.method.to_shader_value(),
            _padding: [0; 3],
        }
    }

    /// Get number of pyramid levels.
    pub fn pyramid_levels(&self) -> u32 {
        self.config.pyramid_levels
    }

    /// Set configuration.
    pub fn set_config(&mut self, config: MotionInterpConfig) {
        self.config = config;
    }

    /// Get configuration.
    pub fn config(&self) -> &MotionInterpConfig {
        &self.config
    }
}

// =============================================================
//  Tests
// =============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------
    //  MotionInterpConfig defaults
    // ---------------------------------------------------------

    #[test]
    fn test_motion_interp_config_defaults() {
        let config = MotionInterpConfig::default();
        assert_eq!(config.block_size, 8);
        assert_eq!(config.search_radius, 16);
        assert_eq!(config.pyramid_levels, 4);
        assert!(config.bidirectional);
        assert!((config.occlusion_threshold - 0.1).abs() < 1e-6);
        assert!((config.motion_smoothing - 0.5).abs() < 1e-6);
        assert!((config.artifact_reduction - 0.3).abs() < 1e-6);
        assert_eq!(config.method, InterpMethod::Bidirectional);
    }

    // ---------------------------------------------------------
    //  FrameInterpConfig defaults
    // ---------------------------------------------------------

    #[test]
    fn test_frame_interp_config_defaults() {
        let config = FrameInterpConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.quality, InterpQuality::Balanced);
        assert_eq!(config.multiplier, 2);
        assert!((config.mv_scale - 1.0).abs() < 1e-6);
        assert!((config.blend_weight - 0.5).abs() < 1e-6);
        assert!((config.occlusion_threshold - 0.1).abs() < 1e-6);
    }

    // ---------------------------------------------------------
    //  InterpMethod::to_shader_value
    // ---------------------------------------------------------

    #[test]
    fn test_interp_method_shader_values() {
        assert_eq!(InterpMethod::Blend.to_shader_value(), 0);
        assert_eq!(InterpMethod::Forward.to_shader_value(), 1);
        assert_eq!(InterpMethod::Backward.to_shader_value(), 2);
        assert_eq!(InterpMethod::Bidirectional.to_shader_value(), 3);
        assert_eq!(InterpMethod::Adaptive.to_shader_value(), 4);
    }

    #[test]
    fn test_interp_method_shader_values_are_contiguous() {
        let methods = [
            InterpMethod::Blend,
            InterpMethod::Forward,
            InterpMethod::Backward,
            InterpMethod::Bidirectional,
            InterpMethod::Adaptive,
        ];
        for (i, m) in methods.iter().enumerate() {
            assert_eq!(m.to_shader_value(), i as u32);
        }
    }

    // ---------------------------------------------------------
    //  InterpQuality::to_config
    // ---------------------------------------------------------

    #[test]
    fn test_quality_fast_config() {
        let config = InterpQuality::Fast.to_config();
        assert_eq!(config.method, InterpMethod::Blend);
        assert_eq!(config.pyramid_levels, 2);
    }

    #[test]
    fn test_quality_balanced_config() {
        let config = InterpQuality::Balanced.to_config();
        assert_eq!(config.method, InterpMethod::Forward);
        assert!(!config.bidirectional);
        assert_eq!(config.pyramid_levels, 3);
    }

    #[test]
    fn test_quality_high_config() {
        let config = InterpQuality::High.to_config();
        // High uses the default config
        let default = MotionInterpConfig::default();
        assert_eq!(config.method, default.method);
        assert_eq!(config.block_size, default.block_size);
        assert_eq!(config.search_radius, default.search_radius);
        assert_eq!(config.pyramid_levels, default.pyramid_levels);
        assert!(config.bidirectional);
    }

    #[test]
    fn test_quality_ultra_config() {
        let config = InterpQuality::Ultra.to_config();
        assert_eq!(config.method, InterpMethod::Adaptive);
        assert_eq!(config.block_size, 4);
        assert_eq!(config.search_radius, 24);
        assert_eq!(config.pyramid_levels, 5);
    }

    // ---------------------------------------------------------
    //  MotionInterpolationManager
    // ---------------------------------------------------------

    #[test]
    fn test_manager_creation() {
        let manager = MotionInterpolationManager::new(MotionInterpConfig::default());
        assert_eq!(manager.pyramid_levels(), 4);
        assert_eq!(manager.config().block_size, 8);
    }

    #[test]
    fn test_manager_with_quality() {
        let manager = MotionInterpolationManager::with_quality(InterpQuality::Ultra);
        assert_eq!(manager.config().method, InterpMethod::Adaptive);
        assert_eq!(manager.pyramid_levels(), 5);
    }

    #[test]
    fn test_manager_set_config() {
        let mut manager = MotionInterpolationManager::new(MotionInterpConfig::default());
        let new_config = MotionInterpConfig {
            block_size: 16,
            search_radius: 32,
            pyramid_levels: 6,
            ..Default::default()
        };
        manager.set_config(new_config);
        assert_eq!(manager.config().block_size, 16);
        assert_eq!(manager.config().search_radius, 32);
        assert_eq!(manager.pyramid_levels(), 6);
    }

    // ---------------------------------------------------------
    //  Motion estimation push constants
    // ---------------------------------------------------------

    #[test]
    fn test_motion_estimation_constants_level_0() {
        let manager = MotionInterpolationManager::new(MotionInterpConfig::default());
        let pc = manager.get_motion_estimation_constants(1920, 1080, 0);

        assert_eq!(pc.resolution, [1920.0, 1080.0]);
        assert!((pc.texel_size[0] - 1.0 / 1920.0).abs() < 1e-6);
        assert!((pc.texel_size[1] - 1.0 / 1080.0).abs() < 1e-6);
        assert_eq!(pc.block_size, 8);
        assert_eq!(pc.search_radius, 16);
        assert_eq!(pc.level, 0);
    }

    #[test]
    fn test_motion_estimation_constants_pyramid_scaling() {
        let manager = MotionInterpolationManager::new(MotionInterpConfig::default());

        // Level 0: full resolution
        let pc0 = manager.get_motion_estimation_constants(1024, 512, 0);
        assert_eq!(pc0.resolution, [1024.0, 512.0]);

        // Level 1: half resolution
        let pc1 = manager.get_motion_estimation_constants(1024, 512, 1);
        assert_eq!(pc1.resolution, [512.0, 256.0]);
        assert!((pc1.texel_size[0] - 1.0 / 512.0).abs() < 1e-6);
        assert!((pc1.texel_size[1] - 1.0 / 256.0).abs() < 1e-6);

        // Level 2: quarter resolution
        let pc2 = manager.get_motion_estimation_constants(1024, 512, 2);
        assert_eq!(pc2.resolution, [256.0, 128.0]);

        // Level 3: eighth resolution
        let pc3 = manager.get_motion_estimation_constants(1024, 512, 3);
        assert_eq!(pc3.resolution, [128.0, 64.0]);
    }

    #[test]
    fn test_motion_estimation_constants_level_matches() {
        let manager = MotionInterpolationManager::new(MotionInterpConfig::default());
        for level in 0..4 {
            let pc = manager.get_motion_estimation_constants(800, 600, level);
            assert_eq!(pc.level, level as i32);
        }
    }

    // ---------------------------------------------------------
    //  Interpolation push constants
    // ---------------------------------------------------------

    #[test]
    fn test_interp_constants_basic() {
        let manager = MotionInterpolationManager::new(MotionInterpConfig::default());
        let pc = manager.get_interp_constants(1920, 1080, 0.5);

        assert_eq!(pc.resolution, [1920.0, 1080.0]);
        assert!((pc.texel_size[0] - 1.0 / 1920.0).abs() < 1e-6);
        assert!((pc.texel_size[1] - 1.0 / 1080.0).abs() < 1e-6);
        assert!((pc.t - 0.5).abs() < 1e-6);
        assert!((pc.occlusion_threshold - 0.1).abs() < 1e-6);
        assert!((pc.motion_smoothing - 0.5).abs() < 1e-6);
        assert!((pc.artifact_reduction - 0.3).abs() < 1e-6);
        assert_eq!(pc.method, InterpMethod::Bidirectional.to_shader_value());
    }

    #[test]
    fn test_interp_constants_t_boundaries() {
        let manager = MotionInterpolationManager::new(MotionInterpConfig::default());

        let pc0 = manager.get_interp_constants(640, 480, 0.0);
        assert!((pc0.t - 0.0).abs() < 1e-6);

        let pc1 = manager.get_interp_constants(640, 480, 1.0);
        assert!((pc1.t - 1.0).abs() < 1e-6);

        let pc_mid = manager.get_interp_constants(640, 480, 0.25);
        assert!((pc_mid.t - 0.25).abs() < 1e-6);
    }

    #[test]
    fn test_interp_constants_method_varies_by_config() {
        let blend_manager = MotionInterpolationManager::with_quality(InterpQuality::Fast);
        let pc = blend_manager.get_interp_constants(640, 480, 0.5);
        assert_eq!(pc.method, InterpMethod::Blend.to_shader_value());

        let adaptive_manager = MotionInterpolationManager::with_quality(InterpQuality::Ultra);
        let pc = adaptive_manager.get_interp_constants(640, 480, 0.5);
        assert_eq!(pc.method, InterpMethod::Adaptive.to_shader_value());
    }

    // ---------------------------------------------------------
    //  Push constant struct layout (repr(C) alignment)
    // ---------------------------------------------------------

    #[test]
    fn test_motion_estimation_push_constants_size() {
        // 2 floats (resolution) + 2 floats (texel_size) + 3 ints + 1 padding = 32 bytes
        assert_eq!(
            std::mem::size_of::<MotionEstimationPushConstants>(),
            32,
            "MotionEstimationPushConstants should be 32 bytes"
        );
    }

    #[test]
    fn test_interp_push_constants_size() {
        // 2 floats (resolution) + 2 floats (texel_size) + 4 floats + 1 uint + 3 uint padding = 48 bytes
        assert_eq!(
            std::mem::size_of::<InterpPushConstants>(),
            48,
            "InterpPushConstants should be 48 bytes"
        );
    }

    // ---------------------------------------------------------
    //  Texel size consistency
    // ---------------------------------------------------------

    #[test]
    fn test_texel_size_is_inverse_of_resolution() {
        let manager = MotionInterpolationManager::new(MotionInterpConfig::default());
        let widths = [320, 640, 1280, 1920, 3840];
        let heights = [240, 480, 720, 1080, 2160];

        for (&w, &h) in widths.iter().zip(heights.iter()) {
            let pc = manager.get_motion_estimation_constants(w, h, 0);
            let expected_tx = 1.0 / w as f32;
            let expected_ty = 1.0 / h as f32;
            assert!(
                (pc.texel_size[0] - expected_tx).abs() < 1e-6,
                "texel_size[0] mismatch for {}x{}", w, h
            );
            assert!(
                (pc.texel_size[1] - expected_ty).abs() < 1e-6,
                "texel_size[1] mismatch for {}x{}", w, h
            );
        }
    }

    // ---------------------------------------------------------
    //  GLSL shader sources are non-empty
    // ---------------------------------------------------------

    #[test]
    fn test_glsl_sources_non_empty() {
        assert!(!glsl::BLOCK_MATCHING.is_empty());
        assert!(!glsl::OPTICAL_FLOW.is_empty());
        assert!(!glsl::MOTION_FILTERING.is_empty());
        assert!(!glsl::OCCLUSION.is_empty());
        assert!(!glsl::INTERPOLATION.is_empty());
        assert!(!glsl::ARTIFACT_REDUCTION.is_empty());
        assert!(!glsl::MOTION_ESTIMATION_COMPUTE.is_empty());
        assert!(!glsl::INTERPOLATION_COMPUTE.is_empty());
        assert!(!glsl::FRAME_INTERP_COMPUTE.is_empty());
    }

    #[test]
    fn test_glsl_compute_shaders_have_version_directive() {
        assert!(glsl::MOTION_ESTIMATION_COMPUTE.contains("#version 450"));
        assert!(glsl::INTERPOLATION_COMPUTE.contains("#version 450"));
        assert!(glsl::FRAME_INTERP_COMPUTE.contains("#version 450"));
    }

    #[test]
    fn test_glsl_compute_shaders_have_local_size() {
        assert!(glsl::MOTION_ESTIMATION_COMPUTE.contains("local_size_x"));
        assert!(glsl::INTERPOLATION_COMPUTE.contains("local_size_x"));
        assert!(glsl::FRAME_INTERP_COMPUTE.contains("local_size_x"));
    }
}

/// GPU-accelerated frame interpolation system.
///
/// Uses Vulkan compute shaders with pre-computed motion vectors to generate
/// intermediate frames. Supports occlusion detection via depth comparison.
pub struct FrameInterpolator {
    /// Configuration.
    config: FrameInterpConfig,
    /// Previous frame.
    prev_frame: Option<FrameData>,
    /// Current frame.
    curr_frame: Option<FrameData>,
    /// Interpolated frame image.
    interp_image: vk::Image,
    interp_view: vk::ImageView,
    interp_memory: vk::DeviceMemory,
    /// Optical flow image (computed from motion vectors).
    flow_image: vk::Image,
    flow_view: vk::ImageView,
    flow_memory: vk::DeviceMemory,
    /// Occlusion mask.
    occlusion_image: vk::Image,
    occlusion_view: vk::ImageView,
    occlusion_memory: vk::DeviceMemory,
    /// Compute pipeline for interpolation.
    compute_pipeline: vk::Pipeline,
    /// Pipeline layout.
    pipeline_layout: vk::PipelineLayout,
    /// Descriptor set layout.
    descriptor_layout: vk::DescriptorSetLayout,
    /// Descriptor pool.
    descriptor_pool: vk::DescriptorPool,
    /// Descriptor set.
    descriptor_set: vk::DescriptorSet,
    /// Frame dimensions.
    width: u32,
    height: u32,
    /// Whether initialized.
    initialized: bool,
}

impl FrameInterpolator {
    /// Create a new frame interpolator.
    pub fn new(ctx: &super::context::VulkanContext, width: u32, height: u32) -> Result<Self, String> {
        // Create descriptor set layout
        let bindings = [
            // Binding 0: Previous frame color (read)
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE),
            // Binding 1: Current frame color (read)
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE),
            // Binding 2: Motion vectors (read)
            vk::DescriptorSetLayoutBinding::default()
                .binding(2)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE),
            // Binding 3: Depth (read)
            vk::DescriptorSetLayoutBinding::default()
                .binding(3)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE),
            // Binding 4: Interpolated output (write)
            vk::DescriptorSetLayoutBinding::default()
                .binding(4)
                .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE),
            // Binding 5: Occlusion mask (write)
            vk::DescriptorSetLayoutBinding::default()
                .binding(5)
                .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE),
        ];

        let layout_info = vk::DescriptorSetLayoutCreateInfo::default()
            .bindings(&bindings);

        // SAFETY: Vulkan descriptor set layout creation with valid device and layout info.
        let descriptor_layout = unsafe {
            ctx.device.create_descriptor_set_layout(&layout_info, None)
                .map_err(|e| format!("Failed to create interp descriptor layout: {:?}", e))?
        };

        // Create pipeline layout with push constants for interpolation params
        let push_constant_range = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::COMPUTE)
            .offset(0)
            .size(std::mem::size_of::<FrameInterpPushConstants>() as u32);

        let layouts = [descriptor_layout];
        let layout_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(&layouts)
            .push_constant_ranges(std::slice::from_ref(&push_constant_range));

        // SAFETY: Vulkan pipeline layout creation with valid device and layout info.
        let pipeline_layout = unsafe {
            ctx.device.create_pipeline_layout(&layout_info, None)
                .map_err(|e| format!("Failed to create interp pipeline layout: {:?}", e))?
        };

        // Create descriptor pool
        let pool_sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: 4,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::STORAGE_IMAGE,
                descriptor_count: 2,
            },
        ];

        let pool_info = vk::DescriptorPoolCreateInfo::default()
            .pool_sizes(&pool_sizes)
            .max_sets(1);

        // SAFETY: Vulkan descriptor pool creation with valid device and pool info.
        let descriptor_pool = unsafe {
            ctx.device.create_descriptor_pool(&pool_info, None)
                .map_err(|e| format!("Failed to create interp descriptor pool: {:?}", e))?
        };

        // Allocate descriptor set
        let alloc_layouts = [descriptor_layout];
        let alloc_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&alloc_layouts);

        // SAFETY: Vulkan descriptor set allocation with valid device, pool, and layout.
        let descriptor_set = unsafe {
            ctx.device.allocate_descriptor_sets(&alloc_info)
                .map_err(|e| format!("Failed to allocate interp descriptor set: {:?}", e))?[0]
        };

        // Create images
        let (interp_image, interp_view, interp_memory) = Self::create_image(
            ctx, width, height, vk::Format::R16G16B16A16_SFLOAT,
            vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_SRC,
        )?;

        let (flow_image, flow_view, flow_memory) = Self::create_image(
            ctx, width, height, vk::Format::R16G16_SFLOAT,
            vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::SAMPLED,
        )?;

        let (occlusion_image, occlusion_view, occlusion_memory) = Self::create_image(
            ctx, width, height, vk::Format::R8_UNORM,
            vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::SAMPLED,
        )?;

        Ok(Self {
            config: FrameInterpConfig::default(),
            prev_frame: None,
            curr_frame: None,
            interp_image,
            interp_view,
            interp_memory,
            flow_image,
            flow_view,
            flow_memory,
            occlusion_image,
            occlusion_view,
            occlusion_memory,
            compute_pipeline: vk::Pipeline::null(),
            pipeline_layout,
            descriptor_layout,
            descriptor_pool,
            descriptor_set,
            width,
            height,
            initialized: true,
        })
    }

    fn create_image(
        ctx: &super::context::VulkanContext,
        width: u32,
        height: u32,
        format: vk::Format,
        usage: vk::ImageUsageFlags,
    ) -> Result<(vk::Image, vk::ImageView, vk::DeviceMemory), String> {
        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(format)
            .extent(vk::Extent3D { width, height, depth: 1 })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED);

        // SAFETY: Vulkan image creation with valid device and image info.
        let image = unsafe {
            ctx.device.create_image(&image_info, None)
                .map_err(|e| format!("Failed to create image: {:?}", e))?
        };

        // SAFETY: Querying memory requirements for a valid image handle.
        let mem_reqs = unsafe { ctx.device.get_image_memory_requirements(image) };

        // SAFETY: Querying physical device memory properties.
        let mem_props = unsafe {
            ctx.instance.get_physical_device_memory_properties(ctx.physical_device)
        };

        let memory_type = (0..mem_props.memory_type_count)
            .find(|i| {
                (mem_reqs.memory_type_bits & (1 << i)) != 0
                    && mem_props.memory_types[*i as usize].property_flags
                        .contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)
            })
            .ok_or_else(|| "Failed to find suitable memory type".to_string())?;

        let alloc_info = vk::MemoryAllocateInfo::default()
            .allocation_size(mem_reqs.size)
            .memory_type_index(memory_type);

        // SAFETY: Vulkan memory allocation with valid device and allocation info.
        let memory = unsafe {
            ctx.device.allocate_memory(&alloc_info, None)
                .map_err(|e| format!("Failed to allocate image memory: {:?}", e))?
        };

        // SAFETY: Binding allocated memory to the image at offset 0.
        unsafe {
            ctx.device.bind_image_memory(image, memory, 0)
                .map_err(|e| format!("Failed to bind image memory: {:?}", e))?;
        }

        let view_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(format)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });

        // SAFETY: Vulkan image view creation with valid device and view info.
        let view = unsafe {
            ctx.device.create_image_view(&view_info, None)
                .map_err(|e| format!("Failed to create image view: {:?}", e))?
        };

        Ok((image, view, memory))
    }

    /// Get configuration.
    pub fn config(&self) -> &FrameInterpConfig {
        &self.config
    }

    /// Set configuration.
    pub fn set_config(&mut self, config: FrameInterpConfig) {
        self.config = config;
    }

    /// Submit a new frame for interpolation.
    pub fn submit_frame(&mut self, frame: FrameData) {
        self.prev_frame = self.curr_frame.take();
        self.curr_frame = Some(frame);
    }

    /// Check if interpolation is ready (has two frames).
    pub fn is_ready(&self) -> bool {
        self.prev_frame.is_some() && self.curr_frame.is_some()
    }

    /// Compute interpolated frame at time t (0.0 = prev, 1.0 = curr).
    pub fn interpolate(&self, ctx: &super::context::VulkanContext, cmd: vk::CommandBuffer, t: f32) {
        if !self.config.enabled || !self.is_ready() || self.compute_pipeline == vk::Pipeline::null() {
            return;
        }

        let push_constants = FrameInterpPushConstants {
            t,
            mv_scale: self.config.mv_scale,
            blend_weight: self.config.blend_weight,
            occlusion_threshold: self.config.occlusion_threshold,
        };

        // SAFETY: Recording compute dispatch commands to a valid command buffer.
        // The pipeline, descriptor set, and push constants are all valid.
        unsafe {
            ctx.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, self.compute_pipeline);

            ctx.device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::COMPUTE,
                self.pipeline_layout,
                0,
                &[self.descriptor_set],
                &[],
            );

            ctx.device.cmd_push_constants(
                cmd,
                self.pipeline_layout,
                vk::ShaderStageFlags::COMPUTE,
                0,
                std::slice::from_raw_parts(
                    &push_constants as *const _ as *const u8,
                    std::mem::size_of::<FrameInterpPushConstants>(),
                ),
            );

            let groups_x = (self.width + 7) / 8;
            let groups_y = (self.height + 7) / 8;
            ctx.device.cmd_dispatch(cmd, groups_x, groups_y, 1);
        }
    }

    /// Get the interpolated image view.
    pub fn output_view(&self) -> vk::ImageView {
        self.interp_view
    }

    /// Get the interpolated image.
    pub fn output_image(&self) -> vk::Image {
        self.interp_image
    }

    /// Resize the interpolator.
    pub fn resize(&mut self, ctx: &super::context::VulkanContext, width: u32, height: u32) -> Result<(), String> {
        if width == self.width && height == self.height {
            return Ok(());
        }

        // Destroy old images
        self.destroy_images(ctx);

        // Create new images
        let (interp_image, interp_view, interp_memory) = Self::create_image(
            ctx, width, height, vk::Format::R16G16B16A16_SFLOAT,
            vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_SRC,
        )?;

        let (flow_image, flow_view, flow_memory) = Self::create_image(
            ctx, width, height, vk::Format::R16G16_SFLOAT,
            vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::SAMPLED,
        )?;

        let (occlusion_image, occlusion_view, occlusion_memory) = Self::create_image(
            ctx, width, height, vk::Format::R8_UNORM,
            vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::SAMPLED,
        )?;

        self.interp_image = interp_image;
        self.interp_view = interp_view;
        self.interp_memory = interp_memory;
        self.flow_image = flow_image;
        self.flow_view = flow_view;
        self.flow_memory = flow_memory;
        self.occlusion_image = occlusion_image;
        self.occlusion_view = occlusion_view;
        self.occlusion_memory = occlusion_memory;
        self.width = width;
        self.height = height;

        Ok(())
    }

    fn destroy_images(&mut self, ctx: &super::context::VulkanContext) {
        // SAFETY: Destroying Vulkan image views, images, and freeing memory.
        // These handles were created by this struct and are valid.
        unsafe {
            ctx.device.destroy_image_view(self.interp_view, None);
            ctx.device.destroy_image(self.interp_image, None);
            ctx.device.free_memory(self.interp_memory, None);

            ctx.device.destroy_image_view(self.flow_view, None);
            ctx.device.destroy_image(self.flow_image, None);
            ctx.device.free_memory(self.flow_memory, None);

            ctx.device.destroy_image_view(self.occlusion_view, None);
            ctx.device.destroy_image(self.occlusion_image, None);
            ctx.device.free_memory(self.occlusion_memory, None);
        }
    }

    /// Destroy the interpolator.
    pub fn destroy(&mut self, ctx: &super::context::VulkanContext) {
        self.destroy_images(ctx);

        // SAFETY: Destroying Vulkan pipeline, pipeline layout, descriptor pool, and
        // descriptor set layout. All handles were created by this struct.
        unsafe {
            if self.compute_pipeline != vk::Pipeline::null() {
                ctx.device.destroy_pipeline(self.compute_pipeline, None);
            }
            ctx.device.destroy_pipeline_layout(self.pipeline_layout, None);
            ctx.device.destroy_descriptor_pool(self.descriptor_pool, None);
            ctx.device.destroy_descriptor_set_layout(self.descriptor_layout, None);
        }

        self.initialized = false;
    }
}
