//! Volumetric Lighting / God Rays
//!
//! Atmospheric scattering and light shafts:
//! - Ray marching through participating media
//! - Single scattering approximation
//! - Light shaft rendering from directional lights
//! - Temporal reprojection for stability
//! - 3D noise for heterogeneous fog

use ash::vk;

/// Volumetric lighting configuration.
#[derive(Debug, Clone)]
pub struct VolumetricConfig {
    /// Resolution scale (0.5 = half resolution).
    pub resolution_scale: f32,
    /// Number of ray march samples.
    pub num_samples: u32,
    /// Maximum ray distance.
    pub max_distance: f32,
    /// Scattering coefficient.
    pub scattering: f32,
    /// Absorption coefficient.
    pub absorption: f32,
    /// Anisotropy factor (-1 to 1, 0 = isotropic).
    pub anisotropy: f32,
    /// Global fog density.
    pub density: f32,
    /// Height fog falloff.
    pub height_falloff: f32,
    /// Height fog base.
    pub height_base: f32,
    /// Enable temporal filtering.
    pub temporal_filtering: bool,
    /// Temporal blend factor.
    pub temporal_blend: f32,
    /// Enable noise-based density variation.
    pub noise_enabled: bool,
    /// Noise scale.
    pub noise_scale: f32,
    /// Noise intensity.
    pub noise_intensity: f32,
}

impl Default for VolumetricConfig {
    fn default() -> Self {
        Self {
            resolution_scale: 0.5,
            num_samples: 64,
            max_distance: 200.0,
            scattering: 0.05,
            absorption: 0.01,
            anisotropy: 0.5,
            density: 0.02,
            height_falloff: 0.1,
            height_base: 0.0,
            temporal_filtering: true,
            temporal_blend: 0.9,
            noise_enabled: true,
            noise_scale: 0.1,
            noise_intensity: 0.5,
        }
    }
}

/// Volumetric quality presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumetricQuality {
    /// Low quality - fewer samples.
    Low,
    /// Medium quality.
    Medium,
    /// High quality.
    High,
    /// Ultra quality.
    Ultra,
}

impl VolumetricQuality {
    /// Get config for quality preset.
    pub fn to_config(&self) -> VolumetricConfig {
        match self {
            VolumetricQuality::Low => VolumetricConfig {
                resolution_scale: 0.25,
                num_samples: 16,
                noise_enabled: false,
                ..Default::default()
            },
            VolumetricQuality::Medium => VolumetricConfig {
                resolution_scale: 0.5,
                num_samples: 32,
                ..Default::default()
            },
            VolumetricQuality::High => VolumetricConfig {
                resolution_scale: 0.5,
                num_samples: 64,
                ..Default::default()
            },
            VolumetricQuality::Ultra => VolumetricConfig {
                resolution_scale: 0.75,
                num_samples: 128,
                ..Default::default()
            },
        }
    }
}

/// Light source for volumetrics.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct VolumetricLight {
    /// Light position (w = type: 0=directional, 1=point, 2=spot).
    pub position: [f32; 4],
    /// Light direction (for directional/spot).
    pub direction: [f32; 4],
    /// Light color and intensity.
    pub color: [f32; 4],
    /// Spot light angles (inner, outer) and range.
    pub params: [f32; 4],
}

/// Push constants for volumetric shader.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct VolumetricPushConstants {
    /// Inverse view-projection matrix.
    pub inv_view_proj: [[f32; 4]; 4],
    /// Previous view-projection for reprojection.
    pub prev_view_proj: [[f32; 4]; 4],
    /// Camera position.
    pub camera_pos: [f32; 4],
    /// Screen resolution.
    pub resolution: [f32; 2],
    /// 1/resolution.
    pub texel_size: [f32; 2],
    /// Scattering coefficient.
    pub scattering: f32,
    /// Absorption coefficient.
    pub absorption: f32,
    /// Anisotropy factor.
    pub anisotropy: f32,
    /// Global density.
    pub density: f32,
    /// Height fog falloff.
    pub height_falloff: f32,
    /// Height fog base.
    pub height_base: f32,
    /// Maximum ray distance.
    pub max_distance: f32,
    /// Number of samples.
    pub num_samples: u32,
    /// Frame index.
    pub frame_index: u32,
    /// Temporal blend.
    pub temporal_blend: f32,
    /// Noise scale.
    pub noise_scale: f32,
    /// Noise intensity.
    pub noise_intensity: f32,
    /// Time for animated noise.
    pub time: f32,
    /// Padding.
    pub _padding: [f32; 3],
}

/// GLSL code for volumetric lighting.
pub mod glsl {
    /// Phase functions for scattering.
    pub const PHASE_FUNCTIONS: &str = r#"
// Henyey-Greenstein phase function
float phaseHenyeyGreenstein(float cosTheta, float g) {
    float g2 = g * g;
    float denom = 1.0 + g2 - 2.0 * g * cosTheta;
    return (1.0 - g2) / (4.0 * 3.14159265 * pow(denom, 1.5));
}

// Schlick approximation (faster)
float phaseSchlick(float cosTheta, float g) {
    float k = 1.55 * g - 0.55 * g * g * g;
    float denom = 1.0 + k * cosTheta;
    return (1.0 - k * k) / (4.0 * 3.14159265 * denom * denom);
}

// Combined Rayleigh + Mie
float phaseCombined(float cosTheta, float g) {
    float rayleigh = 0.75 * (1.0 + cosTheta * cosTheta);
    float mie = phaseHenyeyGreenstein(cosTheta, g);
    return mix(rayleigh, mie, 0.5);
}
"#;

    /// Density functions.
    pub const DENSITY_FUNCTIONS: &str = r#"
// Height-based exponential fog
float heightDensity(vec3 pos, float base, float falloff) {
    float h = pos.y - base;
    return exp(-max(h, 0.0) * falloff);
}

// 3D noise for heterogeneous fog
float noiseDensity(vec3 pos, float scale, float time) {
    // Simple gradient noise approximation
    vec3 p = pos * scale;
    vec3 i = floor(p);
    vec3 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);

    // Hash function
    float n = i.x + i.y * 157.0 + i.z * 113.0;
    vec4 h = fract(sin(vec4(n, n+1.0, n+157.0, n+158.0)) * 43758.5453);

    float a = mix(h.x, h.y, f.x);
    float b = mix(h.z, h.w, f.x);
    return mix(a, b, f.y);
}

// Combined density
float getDensity(vec3 pos, float baseDensity, float heightBase, float heightFalloff,
                 float noiseScale, float noiseIntensity, float time) {
    float density = baseDensity;

    // Height falloff
    density *= heightDensity(pos, heightBase, heightFalloff);

    // Noise variation
    if (noiseIntensity > 0.0) {
        float noise = noiseDensity(pos, noiseScale, time);
        density *= mix(1.0, noise, noiseIntensity);
    }

    return density;
}
"#;

    /// Shadow sampling for volumetrics.
    pub const SHADOW_SAMPLING: &str = r#"
// Sample shadow map for volumetric shadows
float sampleVolumetricShadow(vec3 worldPos, mat4 lightViewProj, sampler2D shadowMap) {
    vec4 lightClip = lightViewProj * vec4(worldPos, 1.0);
    vec3 lightNDC = lightClip.xyz / lightClip.w;
    vec2 shadowUV = lightNDC.xy * 0.5 + 0.5;

    if (shadowUV.x < 0.0 || shadowUV.x > 1.0 ||
        shadowUV.y < 0.0 || shadowUV.y > 1.0) {
        return 1.0; // Outside shadow map
    }

    float shadowDepth = texture(shadowMap, shadowUV).r;
    float currentDepth = lightNDC.z;

    return currentDepth < shadowDepth + 0.001 ? 1.0 : 0.0;
}
"#;

    /// Light contribution calculation.
    pub const LIGHT_CONTRIBUTION: &str = r#"
// Calculate light contribution at a point
vec3 calculateLightContribution(
    vec3 pos,
    vec3 viewDir,
    vec3 lightPos,
    vec3 lightDir,
    vec3 lightColor,
    float lightType,
    float anisotropy
) {
    vec3 toLight;
    float attenuation = 1.0;

    if (lightType < 0.5) {
        // Directional light
        toLight = -lightDir;
    } else {
        // Point/spot light
        toLight = lightPos - pos;
        float dist = length(toLight);
        toLight /= dist;
        attenuation = 1.0 / (dist * dist + 1.0);
    }

    // Phase function
    float cosTheta = dot(-viewDir, toLight);
    float phase = phaseHenyeyGreenstein(cosTheta, anisotropy);

    return lightColor * phase * attenuation;
}
"#;

    /// Ray marching.
    pub const RAY_MARCH: &str = r#"
// Ray march through volume
vec4 raymarchVolume(
    vec3 rayOrigin,
    vec3 rayDir,
    float maxDist,
    int numSamples,
    float scattering,
    float absorption,
    float anisotropy,
    float baseDensity,
    float heightBase,
    float heightFalloff,
    float noiseScale,
    float noiseIntensity,
    float time,
    vec3 lightDir,
    vec3 lightColor
) {
    float stepSize = maxDist / float(numSamples);
    vec3 accumLight = vec3(0.0);
    float transmittance = 1.0;

    // Jitter starting position
    float jitter = fract(sin(dot(rayDir.xy, vec2(12.9898, 78.233))) * 43758.5453);
    float t = jitter * stepSize;

    for (int i = 0; i < numSamples; i++) {
        vec3 pos = rayOrigin + rayDir * t;

        // Get density at this point
        float density = getDensity(pos, baseDensity, heightBase, heightFalloff,
                                    noiseScale, noiseIntensity, time);

        if (density > 0.001) {
            // Calculate extinction
            float extinction = (scattering + absorption) * density * stepSize;

            // Light contribution
            float cosTheta = dot(-rayDir, lightDir);
            float phase = phaseHenyeyGreenstein(cosTheta, anisotropy);
            vec3 inScatter = lightColor * phase * scattering * density;

            // Accumulate with transmittance
            accumLight += inScatter * transmittance * stepSize;
            transmittance *= exp(-extinction);

            // Early exit if fully opaque
            if (transmittance < 0.01) {
                break;
            }
        }

        t += stepSize;
        if (t > maxDist) {
            break;
        }
    }

    return vec4(accumLight, 1.0 - transmittance);
}
"#;

    /// Complete volumetric compute shader.
    pub const VOLUMETRIC_COMPUTE: &str = r#"
#version 450

layout(binding = 0) uniform sampler2D depthTex;
layout(binding = 1) uniform sampler2D shadowMap;
layout(binding = 2) uniform sampler2D historyTex;
layout(binding = 3, rgba16f) uniform writeonly image2D outputTex;

layout(binding = 4) uniform LightBuffer {
    vec4 lightPosition;
    vec4 lightDirection;
    vec4 lightColor;
    mat4 lightViewProj;
} light;

layout(push_constant) uniform PushConstants {
    mat4 invViewProj;
    mat4 prevViewProj;
    vec4 cameraPos;
    vec2 resolution;
    vec2 texelSize;
    float scattering;
    float absorption;
    float anisotropy;
    float density;
    float heightFalloff;
    float heightBase;
    float maxDistance;
    uint numSamples;
    uint frameIndex;
    float temporalBlend;
    float noiseScale;
    float noiseIntensity;
    float time;
} pc;

layout(local_size_x = 8, local_size_y = 8) in;

// Include helper functions...

void main() {
    ivec2 pos = ivec2(gl_GlobalInvocationID.xy);
    vec2 uv = (vec2(pos) + 0.5) / pc.resolution;

    // Reconstruct world position from depth
    float depth = texture(depthTex, uv).r;

    vec4 clipPos = vec4(uv * 2.0 - 1.0, depth, 1.0);
    vec4 worldPos = pc.invViewProj * clipPos;
    worldPos.xyz /= worldPos.w;

    // Ray setup
    vec3 rayOrigin = pc.cameraPos.xyz;
    vec3 rayDir = normalize(worldPos.xyz - rayOrigin);
    float rayDist = min(length(worldPos.xyz - rayOrigin), pc.maxDistance);

    // Ray march
    vec4 volumetric = raymarchVolume(
        rayOrigin, rayDir, rayDist, int(pc.numSamples),
        pc.scattering, pc.absorption, pc.anisotropy,
        pc.density, pc.heightBase, pc.heightFalloff,
        pc.noiseScale, pc.noiseIntensity, pc.time,
        light.lightDirection.xyz, light.lightColor.rgb
    );

    // Temporal reprojection
    vec4 prevClip = pc.prevViewProj * vec4(worldPos.xyz, 1.0);
    vec2 prevUV = (prevClip.xy / prevClip.w) * 0.5 + 0.5;

    if (prevUV.x >= 0.0 && prevUV.x <= 1.0 &&
        prevUV.y >= 0.0 && prevUV.y <= 1.0) {
        vec4 history = texture(historyTex, prevUV);
        volumetric = mix(volumetric, history, pc.temporalBlend);
    }

    imageStore(outputTex, pos, volumetric);
}
"#;

    /// God rays radial blur shader.
    pub const GOD_RAYS_RADIAL: &str = r#"
#version 450

layout(binding = 0) uniform sampler2D occlusionTex;

layout(push_constant) uniform PushConstants {
    vec2 lightScreenPos;
    float exposure;
    float decay;
    float density;
    float weight;
    int numSamples;
} pc;

layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 fragColor;

void main() {
    vec2 deltaUV = (uv - pc.lightScreenPos) * pc.density / float(pc.numSamples);
    vec2 sampleUV = uv;
    float illumination = 0.0;
    float decayFactor = 1.0;

    for (int i = 0; i < pc.numSamples; i++) {
        sampleUV -= deltaUV;
        float sample = texture(occlusionTex, sampleUV).r;
        illumination += sample * decayFactor * pc.weight;
        decayFactor *= pc.decay;
    }

    illumination *= pc.exposure;
    fragColor = vec4(vec3(illumination), 1.0);
}
"#;

    /// Volumetric composite shader.
    pub const VOLUMETRIC_COMPOSITE: &str = r#"
#version 450

layout(binding = 0) uniform sampler2D sceneTex;
layout(binding = 1) uniform sampler2D volumetricTex;

layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 fragColor;

void main() {
    vec3 scene = texture(sceneTex, uv).rgb;
    vec4 volumetric = texture(volumetricTex, uv);

    // Apply volumetric fog
    // volumetric.rgb = in-scattered light
    // volumetric.a = 1 - transmittance (opacity)
    vec3 result = scene * (1.0 - volumetric.a) + volumetric.rgb;

    fragColor = vec4(result, 1.0);
}
"#;
}

/// Volumetric lighting manager.
pub struct VolumetricManager {
    config: VolumetricConfig,
    frame_index: u32,
    time: f32,
    prev_view_proj: [[f32; 4]; 4],
}

impl VolumetricManager {
    /// Create new volumetric manager.
    pub fn new(config: VolumetricConfig) -> Self {
        Self {
            config,
            frame_index: 0,
            time: 0.0,
            prev_view_proj: [[0.0; 4]; 4],
        }
    }

    /// Create with quality preset.
    pub fn with_quality(quality: VolumetricQuality) -> Self {
        Self::new(quality.to_config())
    }

    /// Get push constants for current frame.
    pub fn get_push_constants(
        &mut self,
        inv_view_proj: [[f32; 4]; 4],
        camera_pos: [f32; 3],
        width: u32,
        height: u32,
        delta_time: f32,
    ) -> VolumetricPushConstants {
        self.time += delta_time;

        let pc = VolumetricPushConstants {
            inv_view_proj,
            prev_view_proj: self.prev_view_proj,
            camera_pos: [camera_pos[0], camera_pos[1], camera_pos[2], 1.0],
            resolution: [width as f32, height as f32],
            texel_size: [1.0 / width as f32, 1.0 / height as f32],
            scattering: self.config.scattering,
            absorption: self.config.absorption,
            anisotropy: self.config.anisotropy,
            density: self.config.density,
            height_falloff: self.config.height_falloff,
            height_base: self.config.height_base,
            max_distance: self.config.max_distance,
            num_samples: self.config.num_samples,
            frame_index: self.frame_index,
            temporal_blend: if self.config.temporal_filtering {
                self.config.temporal_blend
            } else {
                0.0
            },
            noise_scale: self.config.noise_scale,
            noise_intensity: if self.config.noise_enabled {
                self.config.noise_intensity
            } else {
                0.0
            },
            time: self.time,
            _padding: [0.0; 3],
        };

        // Store for next frame reprojection
        self.prev_view_proj = inv_view_proj; // Should be view_proj, not inv
        self.frame_index += 1;

        pc
    }

    /// Update configuration.
    pub fn update_config(&mut self, config: VolumetricConfig) {
        self.config = config;
    }

    /// Get current config.
    pub fn config(&self) -> &VolumetricConfig {
        &self.config
    }
}

/// God rays configuration.
#[derive(Debug, Clone)]
pub struct GodRaysConfig {
    /// Light screen position.
    pub light_screen_pos: [f32; 2],
    /// Exposure multiplier.
    pub exposure: f32,
    /// Decay factor per sample.
    pub decay: f32,
    /// Ray density.
    pub density: f32,
    /// Sample weight.
    pub weight: f32,
    /// Number of samples.
    pub num_samples: u32,
}

impl Default for GodRaysConfig {
    fn default() -> Self {
        Self {
            light_screen_pos: [0.5, 0.5],
            exposure: 0.3,
            decay: 0.96,
            density: 1.0,
            weight: 0.04,
            num_samples: 100,
        }
    }
}

/// God rays push constants.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct GodRaysPushConstants {
    pub light_screen_pos: [f32; 2],
    pub exposure: f32,
    pub decay: f32,
    pub density: f32,
    pub weight: f32,
    pub num_samples: i32,
    pub _padding: f32,
}

impl From<&GodRaysConfig> for GodRaysPushConstants {
    fn from(config: &GodRaysConfig) -> Self {
        Self {
            light_screen_pos: config.light_screen_pos,
            exposure: config.exposure,
            decay: config.decay,
            density: config.density,
            weight: config.weight,
            num_samples: config.num_samples as i32,
            _padding: 0.0,
        }
    }
}
