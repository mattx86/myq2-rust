//! SDR to HDR Reconstruction
//!
//! Convert Standard Dynamic Range content to High Dynamic Range:
//! - Inverse tone mapping
//! - Highlight reconstruction
//! - Color gamut expansion
//! - Local adaptation for better results

/// SDR to HDR configuration.
#[derive(Debug, Clone)]
pub struct SdrToHdrConfig {
    /// Peak brightness for HDR output (nits).
    pub peak_brightness: f32,
    /// SDR reference white (typically 100-203 nits).
    pub sdr_white: f32,
    /// Highlight expansion factor.
    pub highlight_expansion: f32,
    /// Color saturation boost.
    pub saturation_boost: f32,
    /// Enable local adaptation.
    pub local_adaptation: bool,
    /// Local adaptation radius.
    pub adaptation_radius: u32,
    /// Shadow boost.
    pub shadow_boost: f32,
    /// Method for inverse tone mapping.
    pub method: InverseToneMapMethod,
}

impl Default for SdrToHdrConfig {
    fn default() -> Self {
        Self {
            peak_brightness: 1000.0,
            sdr_white: 203.0,
            highlight_expansion: 2.0,
            saturation_boost: 1.2,
            local_adaptation: true,
            adaptation_radius: 16,
            shadow_boost: 1.1,
            method: InverseToneMapMethod::Reinhard,
        }
    }
}

/// Inverse tone mapping method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InverseToneMapMethod {
    /// Simple linear expansion.
    Linear,
    /// Reinhard inverse.
    Reinhard,
    /// Hable (Uncharted 2) inverse.
    Hable,
    /// ACES inverse.
    Aces,
    /// Adaptive local mapping.
    AdaptiveLocal,
}

impl InverseToneMapMethod {
    /// Get shader constant.
    pub fn to_shader_value(&self) -> u32 {
        match self {
            InverseToneMapMethod::Linear => 0,
            InverseToneMapMethod::Reinhard => 1,
            InverseToneMapMethod::Hable => 2,
            InverseToneMapMethod::Aces => 3,
            InverseToneMapMethod::AdaptiveLocal => 4,
        }
    }
}

/// Push constants for SDR to HDR shader.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct SdrToHdrPushConstants {
    /// Peak brightness.
    pub peak_brightness: f32,
    /// SDR white level.
    pub sdr_white: f32,
    /// Highlight expansion.
    pub highlight_expansion: f32,
    /// Saturation boost.
    pub saturation_boost: f32,
    /// Shadow boost.
    pub shadow_boost: f32,
    /// Inverse tone map method.
    pub method: u32,
    /// Adaptation radius.
    pub adaptation_radius: u32,
    /// Enable local adaptation.
    pub local_adaptation: u32,
    /// Resolution.
    pub resolution: [f32; 2],
    /// Padding.
    pub _padding: [f32; 2],
}

impl From<&SdrToHdrConfig> for SdrToHdrPushConstants {
    fn from(config: &SdrToHdrConfig) -> Self {
        Self {
            peak_brightness: config.peak_brightness,
            sdr_white: config.sdr_white,
            highlight_expansion: config.highlight_expansion,
            saturation_boost: config.saturation_boost,
            shadow_boost: config.shadow_boost,
            method: config.method.to_shader_value(),
            adaptation_radius: config.adaptation_radius,
            local_adaptation: if config.local_adaptation { 1 } else { 0 },
            resolution: [0.0, 0.0],
            _padding: [0.0; 2],
        }
    }
}

/// GLSL code for SDR to HDR conversion.
pub mod glsl {
    /// Color space conversions.
    pub const COLOR_SPACE: &str = r#"
// sRGB to linear
vec3 sRGBToLinear(vec3 srgb) {
    vec3 low = srgb / 12.92;
    vec3 high = pow((srgb + 0.055) / 1.055, vec3(2.4));
    return mix(low, high, step(0.04045, srgb));
}

// Linear to sRGB
vec3 linearToSRGB(vec3 linear) {
    vec3 low = linear * 12.92;
    vec3 high = 1.055 * pow(linear, vec3(1.0 / 2.4)) - 0.055;
    return mix(low, high, step(0.0031308, linear));
}

// Rec.709 to Rec.2020
vec3 rec709ToRec2020(vec3 color) {
    mat3 m = mat3(
        0.6274, 0.0691, 0.0164,
        0.3293, 0.9195, 0.0880,
        0.0433, 0.0114, 0.8956
    );
    return m * color;
}

// RGB to luminance
float luminance(vec3 color) {
    return dot(color, vec3(0.2126, 0.7152, 0.0722));
}
"#;

    /// Inverse tone mapping functions.
    pub const INVERSE_TONE_MAP: &str = r#"
// Linear expansion
vec3 inverseToneMapLinear(vec3 sdr, float expansion) {
    return sdr * expansion;
}

// Inverse Reinhard
vec3 inverseToneMapReinhard(vec3 sdr, float maxL) {
    float L = luminance(sdr);
    if (L < 0.0001) return sdr;

    // Inverse of L' = L / (1 + L) is L = L' / (1 - L')
    float invL = L / max(1.0 - L, 0.0001);

    // Scale to max luminance
    invL = min(invL, maxL);

    return sdr * (invL / L);
}

// Inverse Hable (approximation)
vec3 inverseToneMapHable(vec3 sdr, float maxL) {
    // Hable parameters
    const float A = 0.15;
    const float B = 0.50;
    const float C = 0.10;
    const float D = 0.20;
    const float E = 0.02;
    const float F = 0.30;

    // Iterative inversion (simplified)
    vec3 result = sdr;
    for (int i = 0; i < 3; i++) {
        vec3 x = result;
        vec3 num = (x * (A * x + C * B) + D * E);
        vec3 den = (x * (A * x + B) + D * F);
        vec3 tm = num / den;
        result = result + (sdr - tm) * 2.0;
    }

    return min(result, vec3(maxL));
}

// Inverse ACES (approximation)
vec3 inverseToneMapAces(vec3 sdr, float maxL) {
    // ACES parameters
    const float a = 2.51;
    const float b = 0.03;
    const float c = 2.43;
    const float d = 0.59;
    const float e = 0.14;

    // Quadratic formula solution
    vec3 x = sdr;
    vec3 result = (-d * x + sqrt(max((d * x) * (d * x) - 4.0 * (c * x - a) * (e * x - b), 0.0))) /
                  (2.0 * (c * x - a + 0.0001));

    return clamp(result, vec3(0.0), vec3(maxL));
}
"#;

    /// Highlight reconstruction.
    pub const HIGHLIGHT_RECONSTRUCTION: &str = r#"
// Detect and expand highlights
vec3 expandHighlights(vec3 color, float threshold, float expansion) {
    float L = luminance(color);

    if (L > threshold) {
        // How much above threshold
        float excess = (L - threshold) / (1.0 - threshold);

        // Exponential expansion for highlights
        float newL = threshold + excess * expansion * (1.0 - threshold);

        // Preserve color ratio
        return color * (newL / max(L, 0.0001));
    }

    return color;
}

// Specular highlight reconstruction
vec3 reconstructSpecular(vec3 color, float expansion) {
    // Detect near-white areas (likely clipped highlights)
    float minChannel = min(min(color.r, color.g), color.b);
    float maxChannel = max(max(color.r, color.g), color.b);

    if (minChannel > 0.9) {
        // Near white - expand uniformly
        return color * expansion;
    } else if (maxChannel > 0.95 && minChannel > 0.7) {
        // Colored highlight - expand while preserving hue
        float sat = 1.0 - minChannel / maxChannel;
        float expandFactor = 1.0 + (expansion - 1.0) * (1.0 - sat);
        return color * expandFactor;
    }

    return color;
}
"#;

    /// Local adaptation.
    pub const LOCAL_ADAPTATION: &str = r#"
// Compute local average luminance
float computeLocalLuminance(sampler2D tex, vec2 uv, vec2 texelSize, int radius) {
    float sum = 0.0;
    float count = 0.0;

    for (int y = -radius; y <= radius; y++) {
        for (int x = -radius; x <= radius; x++) {
            vec2 offset = vec2(x, y) * texelSize;
            vec3 sample = texture(tex, uv + offset).rgb;
            sum += luminance(sample);
            count += 1.0;
        }
    }

    return sum / count;
}

// Local adaptation inverse tone map
vec3 localAdaptiveExpand(vec3 color, float localL, float globalL, float maxL) {
    // Adaptation factor based on local vs global luminance
    float adaptation = localL / max(globalL, 0.001);
    adaptation = clamp(adaptation, 0.5, 2.0);

    // Expand more in dark areas, less in bright areas
    float expansion = maxL / (adaptation * 100.0);

    return inverseToneMapReinhard(color, expansion);
}
"#;

    /// Saturation and color enhancement.
    pub const COLOR_ENHANCEMENT: &str = r#"
// Boost saturation
vec3 boostSaturation(vec3 color, float boost) {
    float L = luminance(color);
    return mix(vec3(L), color, boost);
}

// Enhance shadow detail
vec3 enhanceShadows(vec3 color, float boost) {
    float L = luminance(color);

    if (L < 0.18) { // Shadow threshold
        float shadowFactor = 1.0 + boost * (0.18 - L) / 0.18;
        return color * shadowFactor;
    }

    return color;
}

// Color gamut expansion hint
vec3 expandGamut(vec3 rec709, float expansion) {
    // Simple saturation-based expansion toward Rec.2020
    vec3 rec2020 = rec709ToRec2020(rec709);
    return mix(rec709, rec2020, expansion);
}
"#;

    /// Complete SDR to HDR compute shader.
    pub const SDR_TO_HDR_COMPUTE: &str = r#"
#version 450

layout(binding = 0) uniform sampler2D sdrInput;
layout(binding = 1, rgba16f) uniform writeonly image2D hdrOutput;

layout(push_constant) uniform PushConstants {
    float peakBrightness;
    float sdrWhite;
    float highlightExpansion;
    float saturationBoost;
    float shadowBoost;
    uint method;
    uint adaptationRadius;
    uint localAdaptation;
    vec2 resolution;
} pc;

layout(local_size_x = 8, local_size_y = 8) in;

// Include helper functions...

void main() {
    ivec2 pos = ivec2(gl_GlobalInvocationID.xy);
    vec2 uv = (vec2(pos) + 0.5) / pc.resolution;
    vec2 texelSize = 1.0 / pc.resolution;

    // Sample SDR input
    vec3 sdr = texture(sdrInput, uv).rgb;

    // Convert to linear
    vec3 linear = sRGBToLinear(sdr);

    // Calculate expansion ratio
    float maxL = pc.peakBrightness / pc.sdrWhite;

    // Apply inverse tone mapping based on method
    vec3 hdr;
    if (pc.method == 0) {
        hdr = inverseToneMapLinear(linear, maxL);
    } else if (pc.method == 1) {
        hdr = inverseToneMapReinhard(linear, maxL);
    } else if (pc.method == 2) {
        hdr = inverseToneMapHable(linear, maxL);
    } else if (pc.method == 3) {
        hdr = inverseToneMapAces(linear, maxL);
    } else {
        // Adaptive local
        float localL = computeLocalLuminance(sdrInput, uv, texelSize, int(pc.adaptationRadius));
        float globalL = 0.18; // Middle gray assumption
        hdr = localAdaptiveExpand(linear, localL, globalL, maxL);
    }

    // Reconstruct highlights
    hdr = expandHighlights(hdr, 0.8, pc.highlightExpansion);
    hdr = reconstructSpecular(hdr, pc.highlightExpansion);

    // Enhance shadows
    hdr = enhanceShadows(hdr, pc.shadowBoost - 1.0);

    // Boost saturation
    hdr = boostSaturation(hdr, pc.saturationBoost);

    // Scale to output nits (assume scRGB output where 1.0 = 80 nits)
    hdr *= pc.sdrWhite / 80.0;

    imageStore(hdrOutput, pos, vec4(hdr, 1.0));
}
"#;
}

/// SDR to HDR conversion manager.
pub struct SdrToHdrManager {
    config: SdrToHdrConfig,
}

impl SdrToHdrManager {
    /// Create new manager.
    pub fn new(config: SdrToHdrConfig) -> Self {
        Self { config }
    }

    /// Get push constants.
    pub fn get_push_constants(&self, width: u32, height: u32) -> SdrToHdrPushConstants {
        let mut pc = SdrToHdrPushConstants::from(&self.config);
        pc.resolution = [width as f32, height as f32];
        pc
    }

    /// Set configuration.
    pub fn set_config(&mut self, config: SdrToHdrConfig) {
        self.config = config;
    }

    /// Get configuration.
    pub fn config(&self) -> &SdrToHdrConfig {
        &self.config
    }

    /// Set peak brightness.
    pub fn set_peak_brightness(&mut self, nits: f32) {
        self.config.peak_brightness = nits.clamp(400.0, 10000.0);
    }

    /// Set method.
    pub fn set_method(&mut self, method: InverseToneMapMethod) {
        self.config.method = method;
    }
}

/// HDR display capabilities.
#[derive(Debug, Clone)]
pub struct HdrDisplayInfo {
    /// Peak brightness in nits.
    pub peak_brightness: f32,
    /// Minimum brightness in nits.
    pub min_brightness: f32,
    /// Reference white in nits.
    pub reference_white: f32,
    /// Color gamut coverage (Rec.2020).
    pub rec2020_coverage: f32,
}

impl Default for HdrDisplayInfo {
    fn default() -> Self {
        Self {
            peak_brightness: 1000.0,
            min_brightness: 0.001,
            reference_white: 203.0,
            rec2020_coverage: 0.75,
        }
    }
}
