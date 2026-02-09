//! Depth of Field (DoF)
//!
//! Physically-based depth of field simulation:
//! - Bokeh shape simulation (circle, hexagon, octagon)
//! - Physically accurate CoC (Circle of Confusion)
//! - Separated near/far field processing
//! - Gather-based blur for quality
//! - Optional scatter-based bokeh for artistic effect

use ash::vk;

/// Depth of Field configuration.
#[derive(Debug, Clone)]
pub struct DofConfig {
    /// Focal distance (where focus is sharpest).
    pub focal_distance: f32,
    /// Focal length (affects DoF range).
    pub focal_length: f32,
    /// Aperture (f-stop, lower = more blur).
    pub aperture: f32,
    /// Sensor size (affects CoC calculation).
    pub sensor_size: f32,
    /// Near plane distance.
    pub near_plane: f32,
    /// Far plane distance.
    pub far_plane: f32,
    /// Bokeh shape.
    pub bokeh_shape: BokehShape,
    /// Maximum CoC radius in pixels.
    pub max_coc_radius: f32,
    /// Number of blur samples.
    pub num_samples: u32,
    /// Enable near field blur.
    pub near_field: bool,
    /// Enable far field blur.
    pub far_field: bool,
    /// Resolution scale for blur pass.
    pub resolution_scale: f32,
    /// Highlight threshold for bokeh.
    pub highlight_threshold: f32,
    /// Highlight boost factor.
    pub highlight_boost: f32,
}

impl Default for DofConfig {
    fn default() -> Self {
        Self {
            focal_distance: 10.0,
            focal_length: 50.0, // 50mm lens
            aperture: 2.8,      // f/2.8
            sensor_size: 36.0,  // Full frame
            near_plane: 0.1,
            far_plane: 1000.0,
            bokeh_shape: BokehShape::Circle,
            max_coc_radius: 16.0,
            num_samples: 64,
            near_field: true,
            far_field: true,
            resolution_scale: 0.5,
            highlight_threshold: 1.0,
            highlight_boost: 1.5,
        }
    }
}

/// Bokeh shape options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BokehShape {
    /// Circular bokeh (ideal lens).
    Circle,
    /// Hexagonal bokeh (6 aperture blades).
    Hexagon,
    /// Octagonal bokeh (8 aperture blades).
    Octagon,
    /// Cat-eye bokeh (vignetting effect).
    CatEye,
}

impl BokehShape {
    /// Get shader constant for shape.
    pub fn to_shader_value(&self) -> u32 {
        match self {
            BokehShape::Circle => 0,
            BokehShape::Hexagon => 1,
            BokehShape::Octagon => 2,
            BokehShape::CatEye => 3,
        }
    }
}

/// DoF quality presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DofQuality {
    /// Low quality - fewer samples.
    Low,
    /// Medium quality.
    Medium,
    /// High quality.
    High,
    /// Cinematic quality.
    Cinematic,
}

impl DofQuality {
    /// Get config for quality preset.
    pub fn to_config(&self) -> DofConfig {
        match self {
            DofQuality::Low => DofConfig {
                num_samples: 16,
                max_coc_radius: 8.0,
                resolution_scale: 0.25,
                ..Default::default()
            },
            DofQuality::Medium => DofConfig {
                num_samples: 32,
                max_coc_radius: 12.0,
                resolution_scale: 0.5,
                ..Default::default()
            },
            DofQuality::High => DofConfig {
                num_samples: 64,
                max_coc_radius: 16.0,
                resolution_scale: 0.5,
                ..Default::default()
            },
            DofQuality::Cinematic => DofConfig {
                num_samples: 128,
                max_coc_radius: 24.0,
                resolution_scale: 1.0,
                bokeh_shape: BokehShape::Octagon,
                ..Default::default()
            },
        }
    }
}

/// Push constants for DoF shader.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DofPushConstants {
    /// Screen resolution.
    pub resolution: [f32; 2],
    /// 1/resolution.
    pub texel_size: [f32; 2],
    /// Focal distance.
    pub focal_distance: f32,
    /// Focal length.
    pub focal_length: f32,
    /// Aperture (f-stop).
    pub aperture: f32,
    /// Sensor size.
    pub sensor_size: f32,
    /// Near plane.
    pub near_plane: f32,
    /// Far plane.
    pub far_plane: f32,
    /// Maximum CoC radius.
    pub max_coc_radius: f32,
    /// Number of samples.
    pub num_samples: u32,
    /// Bokeh shape.
    pub bokeh_shape: u32,
    /// Highlight threshold.
    pub highlight_threshold: f32,
    /// Highlight boost.
    pub highlight_boost: f32,
    /// Padding.
    pub _padding: f32,
}

/// GLSL code for DoF.
pub mod glsl {
    /// Circle of Confusion calculation.
    pub const COC_CALCULATION: &str = r#"
// Calculate Circle of Confusion size
float calculateCoC(float depth, float focalDistance, float focalLength,
                   float aperture, float sensorSize) {
    // Convert depth to linear
    float linearDepth = depth;

    // Thin lens equation for CoC
    // CoC = abs(A * f * (S1 - f) * (1/d - 1/S1) / (d - f))
    // Simplified: CoC = abs(A * (d - S1) / d) * f^2 / (S1 * (S1 - f))

    float A = focalLength / aperture; // Aperture diameter
    float S1 = focalDistance;         // Focus distance

    // Signed CoC (negative = near field, positive = far field)
    float coc = A * focalLength * (linearDepth - S1) /
                (linearDepth * (S1 - focalLength));

    // Convert to pixels (approximate)
    coc *= sensorSize / focalLength;

    return coc;
}

// Simplified CoC for games
float calculateCoCSimple(float depth, float focalDistance, float focalRange,
                          float maxCoC) {
    float delta = depth - focalDistance;
    float coc = delta / focalRange;
    return clamp(coc, -1.0, 1.0) * maxCoC;
}
"#;

    /// Bokeh shape functions.
    pub const BOKEH_SHAPES: &str = r#"
// Circular disk sampling (Poisson)
const int POISSON_SAMPLES = 64;
const vec2 poissonDisk[64] = vec2[](
    vec2(-0.934812, 0.366741), vec2(-0.918943, -0.0181635),
    vec2(-0.873226, 0.62389), vec2(-0.8352, 0.937803),
    vec2(-0.822138, -0.281877), vec2(-0.812983, 0.10416),
    vec2(-0.786126, -0.767632), vec2(-0.739494, -0.535813),
    vec2(-0.681692, 0.284707), vec2(-0.61742, -0.234535),
    vec2(-0.601184, 0.562426), vec2(-0.607105, 0.847591),
    vec2(-0.581835, -0.00485244), vec2(-0.554247, -0.771111),
    vec2(-0.483383, -0.498426), vec2(-0.476669, 0.396472),
    vec2(-0.439802, 0.098913), vec2(-0.420473, -0.217932),
    vec2(-0.377162, 0.685095), vec2(-0.371636, -0.605101),
    vec2(-0.350199, 0.973258), vec2(-0.32012, -0.890632),
    vec2(-0.284518, 0.432337), vec2(-0.244147, 0.172505),
    vec2(-0.218772, -0.40614), vec2(-0.203315, -0.117453),
    vec2(-0.17781, 0.640318), vec2(-0.152798, -0.708282),
    vec2(-0.141479, 0.918274), vec2(-0.0591985, -0.287501),
    vec2(-0.0381787, 0.429443), vec2(-0.0339955, 0.132066),
    vec2(0.0, -0.523389), vec2(0.0324465, -0.0992157),
    vec2(0.0636255, 0.653378), vec2(0.0804569, 0.933088),
    vec2(0.0963729, -0.786003), vec2(0.110148, 0.360475),
    vec2(0.134514, -0.322976), vec2(0.153024, 0.0967251),
    vec2(0.180958, -0.571268), vec2(0.200777, 0.551788),
    vec2(0.230315, -0.116298), vec2(0.263497, -0.91942),
    vec2(0.291532, 0.816681), vec2(0.305661, 0.30593),
    vec2(0.345166, -0.426088), vec2(0.373026, -0.693163),
    vec2(0.388555, 0.0954853), vec2(0.413978, 0.564549),
    vec2(0.460327, -0.211632), vec2(0.500154, -0.515381),
    vec2(0.514098, 0.332559), vec2(0.553571, -0.832772),
    vec2(0.56573, 0.777456), vec2(0.579691, 0.0507273),
    vec2(0.634826, -0.348779), vec2(0.651294, 0.54523),
    vec2(0.676199, -0.638112), vec2(0.722807, 0.248254),
    vec2(0.747629, -0.0965302), vec2(0.785301, -0.423664),
    vec2(0.818453, 0.507976), vec2(0.847228, -0.697655),
    vec2(0.868661, 0.143486), vec2(0.925847, -0.313786)
);

// Hexagonal kernel
vec2 hexagonSample(int index, int numSamples, float rotation) {
    float angle = float(index) / float(numSamples) * 6.28318530718 + rotation;
    float radius = sqrt(float(index + 1) / float(numSamples));

    // Hexagon shape factor
    float hexAngle = mod(angle, 1.047197551); // PI/3
    float hexFactor = cos(hexAngle - 0.523598776); // PI/6

    return vec2(cos(angle), sin(angle)) * radius / hexFactor;
}

// Octagonal kernel
vec2 octagonSample(int index, int numSamples, float rotation) {
    float angle = float(index) / float(numSamples) * 6.28318530718 + rotation;
    float radius = sqrt(float(index + 1) / float(numSamples));

    // Octagon shape factor
    float octAngle = mod(angle, 0.785398163); // PI/4
    float octFactor = cos(octAngle - 0.392699082); // PI/8

    return vec2(cos(angle), sin(angle)) * radius / octFactor;
}

// Cat-eye bokeh (off-center vignetting)
float catEyeFactor(vec2 sampleOffset, vec2 uvFromCenter) {
    // Vignette based on distance from center
    float dist = length(uvFromCenter);
    float vignetteDir = normalize(uvFromCenter);
    float sampleDir = dot(normalize(sampleOffset), vignetteDir);

    // Clip samples on far side from center
    return smoothstep(0.0, 0.3, 1.0 - dist * sampleDir * 0.5);
}
"#;

    /// Gather-based DoF blur.
    pub const DOF_GATHER: &str = r#"
// Gather-based DoF blur
vec4 gatherDoF(
    sampler2D colorTex,
    sampler2D cocTex,
    vec2 uv,
    vec2 texelSize,
    float maxCoC,
    int numSamples,
    uint bokehShape
) {
    float centerCoC = texture(cocTex, uv).r;
    float absCoC = abs(centerCoC);

    if (absCoC < 0.5) {
        return texture(colorTex, uv);
    }

    vec4 colorSum = vec4(0.0);
    float weightSum = 0.0;

    for (int i = 0; i < numSamples; i++) {
        // Get sample offset based on bokeh shape
        vec2 offset;
        if (bokehShape == 0) {
            offset = poissonDisk[i % 64];
        } else if (bokehShape == 1) {
            offset = hexagonSample(i, numSamples, 0.0);
        } else if (bokehShape == 2) {
            offset = octagonSample(i, numSamples, 0.0);
        } else {
            offset = poissonDisk[i % 64];
        }

        vec2 sampleUV = uv + offset * absCoC * texelSize * maxCoC;
        float sampleCoC = texture(cocTex, sampleUV).r;

        // Weight based on CoC overlap
        float sampleRadius = abs(sampleCoC);
        float dist = length(offset) * absCoC;

        // Near field: sample contributes if it's in front AND has enough blur
        // Far field: sample contributes based on its own CoC
        float weight = 1.0;
        if (centerCoC < 0.0) {
            // Near field - accumulate from blurry near objects
            weight = smoothstep(0.0, 1.0, sampleRadius - dist * 0.5);
        } else {
            // Far field
            weight = smoothstep(0.0, 1.0, sampleRadius);
        }

        vec4 sampleColor = texture(colorTex, sampleUV);

        colorSum += sampleColor * weight;
        weightSum += weight;
    }

    return colorSum / max(weightSum, 0.001);
}
"#;

    /// CoC calculation compute shader.
    pub const COC_COMPUTE: &str = r#"
#version 450

layout(binding = 0) uniform sampler2D depthTex;
layout(binding = 1, r16f) uniform writeonly image2D cocTex;

layout(push_constant) uniform PushConstants {
    vec2 resolution;
    vec2 texelSize;
    float focalDistance;
    float focalLength;
    float aperture;
    float sensorSize;
    float nearPlane;
    float farPlane;
    float maxCoCRadius;
    uint numSamples;
    uint bokehShape;
    float highlightThreshold;
    float highlightBoost;
} pc;

layout(local_size_x = 8, local_size_y = 8) in;

// Linearize depth
float linearizeDepth(float depth) {
    return pc.nearPlane * pc.farPlane /
           (pc.farPlane - depth * (pc.farPlane - pc.nearPlane));
}

void main() {
    ivec2 pos = ivec2(gl_GlobalInvocationID.xy);
    vec2 uv = (vec2(pos) + 0.5) / pc.resolution;

    float depth = texture(depthTex, uv).r;
    float linearDepth = linearizeDepth(depth);

    // Calculate CoC
    float coc = calculateCoC(linearDepth, pc.focalDistance, pc.focalLength,
                              pc.aperture, pc.sensorSize);

    // Clamp to max radius
    coc = clamp(coc, -pc.maxCoCRadius, pc.maxCoCRadius);

    // Normalize to -1..1 range
    coc /= pc.maxCoCRadius;

    imageStore(cocTex, pos, vec4(coc));
}
"#;

    /// DoF blur compute shader.
    pub const DOF_BLUR_COMPUTE: &str = r#"
#version 450

layout(binding = 0) uniform sampler2D colorTex;
layout(binding = 1) uniform sampler2D cocTex;
layout(binding = 2, rgba16f) uniform writeonly image2D outputTex;

layout(push_constant) uniform PushConstants {
    vec2 resolution;
    vec2 texelSize;
    float focalDistance;
    float focalLength;
    float aperture;
    float sensorSize;
    float nearPlane;
    float farPlane;
    float maxCoCRadius;
    uint numSamples;
    uint bokehShape;
    float highlightThreshold;
    float highlightBoost;
} pc;

layout(local_size_x = 8, local_size_y = 8) in;

// Include helper functions...

void main() {
    ivec2 pos = ivec2(gl_GlobalInvocationID.xy);
    vec2 uv = (vec2(pos) + 0.5) / pc.resolution;

    vec4 result = gatherDoF(colorTex, cocTex, uv, pc.texelSize,
                            pc.maxCoCRadius, int(pc.numSamples), pc.bokehShape);

    // Boost highlights for more visible bokeh
    float luminance = dot(result.rgb, vec3(0.299, 0.587, 0.114));
    if (luminance > pc.highlightThreshold) {
        result.rgb *= pc.highlightBoost;
    }

    imageStore(outputTex, pos, result);
}
"#;

    /// DoF composite shader.
    pub const DOF_COMPOSITE: &str = r#"
#version 450

layout(binding = 0) uniform sampler2D sharpTex;
layout(binding = 1) uniform sampler2D blurTex;
layout(binding = 2) uniform sampler2D cocTex;

layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 fragColor;

void main() {
    vec4 sharp = texture(sharpTex, uv);
    vec4 blur = texture(blurTex, uv);
    float coc = abs(texture(cocTex, uv).r);

    // Smooth blend based on CoC
    float blend = smoothstep(0.0, 0.5, coc);

    fragColor = mix(sharp, blur, blend);
}
"#;

    /// Separable DoF blur (faster alternative).
    pub const DOF_SEPARABLE: &str = r#"
// Horizontal pass
vec4 dofBlurH(sampler2D tex, sampler2D cocTex, vec2 uv, vec2 texelSize, float maxCoC) {
    float coc = abs(texture(cocTex, uv).r) * maxCoC;
    vec4 color = vec4(0.0);
    float weight = 0.0;

    for (int i = -8; i <= 8; i++) {
        vec2 offset = vec2(float(i) * texelSize.x * coc, 0.0);
        float w = 1.0 - abs(float(i)) / 8.0;
        color += texture(tex, uv + offset) * w;
        weight += w;
    }

    return color / weight;
}

// Vertical pass
vec4 dofBlurV(sampler2D tex, sampler2D cocTex, vec2 uv, vec2 texelSize, float maxCoC) {
    float coc = abs(texture(cocTex, uv).r) * maxCoC;
    vec4 color = vec4(0.0);
    float weight = 0.0;

    for (int i = -8; i <= 8; i++) {
        vec2 offset = vec2(0.0, float(i) * texelSize.y * coc);
        float w = 1.0 - abs(float(i)) / 8.0;
        color += texture(tex, uv + offset) * w;
        weight += w;
    }

    return color / weight;
}
"#;
}

/// DoF manager.
pub struct DofManager {
    config: DofConfig,
}

impl DofManager {
    /// Create new DoF manager.
    pub fn new(config: DofConfig) -> Self {
        Self { config }
    }

    /// Create with quality preset.
    pub fn with_quality(quality: DofQuality) -> Self {
        Self::new(quality.to_config())
    }

    /// Get push constants.
    pub fn get_push_constants(&self, width: u32, height: u32) -> DofPushConstants {
        DofPushConstants {
            resolution: [width as f32, height as f32],
            texel_size: [1.0 / width as f32, 1.0 / height as f32],
            focal_distance: self.config.focal_distance,
            focal_length: self.config.focal_length,
            aperture: self.config.aperture,
            sensor_size: self.config.sensor_size,
            near_plane: self.config.near_plane,
            far_plane: self.config.far_plane,
            max_coc_radius: self.config.max_coc_radius,
            num_samples: self.config.num_samples,
            bokeh_shape: self.config.bokeh_shape.to_shader_value(),
            highlight_threshold: self.config.highlight_threshold,
            highlight_boost: self.config.highlight_boost,
            _padding: 0.0,
        }
    }

    /// Set focal distance.
    pub fn set_focal_distance(&mut self, distance: f32) {
        self.config.focal_distance = distance;
    }

    /// Set aperture (f-stop).
    pub fn set_aperture(&mut self, aperture: f32) {
        self.config.aperture = aperture.max(1.0);
    }

    /// Auto-focus on depth at screen position.
    pub fn auto_focus(&mut self, depth: f32) {
        self.config.focal_distance = depth;
    }

    /// Update configuration.
    pub fn update_config(&mut self, config: DofConfig) {
        self.config = config;
    }

    /// Get current config.
    pub fn config(&self) -> &DofConfig {
        &self.config
    }
}
