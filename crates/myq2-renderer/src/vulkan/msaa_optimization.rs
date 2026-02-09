//! MSAA Optimization (VK_EXT_multisampled_render_to_single_sampled)
//!
//! Efficient MSAA without separate multisample attachments:
//! - Render directly to single-sampled images
//! - Hardware resolves on tile-based GPUs
//! - Reduced memory bandwidth
//! - Automatic MSAA resolve during render pass

use ash::vk;

/// MSAA optimization capabilities.
#[derive(Debug, Clone, Default)]
pub struct MsaaOptimizationCapabilities {
    /// Whether the extension is supported.
    pub supported: bool,
    /// Whether framebuffer no attachments is supported.
    pub framebuffer_no_attachments: bool,
}

/// Query MSAA optimization capabilities.
pub fn query_capabilities(ctx: &super::context::VulkanContext) -> MsaaOptimizationCapabilities {
    let mut ms_single_features = vk::PhysicalDeviceMultisampledRenderToSingleSampledFeaturesEXT::default();
    let mut features2 = vk::PhysicalDeviceFeatures2::default()
        .push_next(&mut ms_single_features);

    unsafe {
        ctx.instance.get_physical_device_features2(ctx.physical_device, &mut features2);
    }

    let _ = features2;
    let supported = ms_single_features.multisampled_render_to_single_sampled == vk::TRUE;

    MsaaOptimizationCapabilities {
        supported,
        framebuffer_no_attachments: false, // Would need additional query
    }
}

/// MSAA sample count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsaaSampleCount {
    /// No MSAA.
    None,
    /// 2x MSAA.
    X2,
    /// 4x MSAA.
    X4,
    /// 8x MSAA.
    X8,
    /// 16x MSAA (if supported).
    X16,
}

impl MsaaSampleCount {
    /// Convert to Vulkan sample count flags.
    pub fn to_vk(&self) -> vk::SampleCountFlags {
        match self {
            MsaaSampleCount::None => vk::SampleCountFlags::TYPE_1,
            MsaaSampleCount::X2 => vk::SampleCountFlags::TYPE_2,
            MsaaSampleCount::X4 => vk::SampleCountFlags::TYPE_4,
            MsaaSampleCount::X8 => vk::SampleCountFlags::TYPE_8,
            MsaaSampleCount::X16 => vk::SampleCountFlags::TYPE_16,
        }
    }

    /// Get sample count as u32.
    pub fn count(&self) -> u32 {
        match self {
            MsaaSampleCount::None => 1,
            MsaaSampleCount::X2 => 2,
            MsaaSampleCount::X4 => 4,
            MsaaSampleCount::X8 => 8,
            MsaaSampleCount::X16 => 16,
        }
    }

    /// Check if MSAA is enabled.
    pub fn is_enabled(&self) -> bool {
        !matches!(self, MsaaSampleCount::None)
    }
}

/// MSAA resolve mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolveMode {
    /// No resolve (keep multisampled).
    None,
    /// Average all samples.
    Average,
    /// Use sample 0.
    SampleZero,
    /// Use minimum sample value.
    Min,
    /// Use maximum sample value.
    Max,
}

impl ResolveMode {
    /// Convert to Vulkan resolve mode.
    pub fn to_vk(&self) -> vk::ResolveModeFlags {
        match self {
            ResolveMode::None => vk::ResolveModeFlags::NONE,
            ResolveMode::Average => vk::ResolveModeFlags::AVERAGE,
            ResolveMode::SampleZero => vk::ResolveModeFlags::SAMPLE_ZERO,
            ResolveMode::Min => vk::ResolveModeFlags::MIN,
            ResolveMode::Max => vk::ResolveModeFlags::MAX,
        }
    }
}

/// Configuration for optimized MSAA.
#[derive(Debug, Clone)]
pub struct OptimizedMsaaConfig {
    /// Sample count.
    pub sample_count: MsaaSampleCount,
    /// Color resolve mode.
    pub color_resolve: ResolveMode,
    /// Depth resolve mode.
    pub depth_resolve: ResolveMode,
    /// Stencil resolve mode.
    pub stencil_resolve: ResolveMode,
    /// Use per-sample shading.
    pub per_sample_shading: bool,
    /// Sample shading minimum fraction.
    pub min_sample_shading: f32,
    /// Alpha to coverage.
    pub alpha_to_coverage: bool,
    /// Alpha to one.
    pub alpha_to_one: bool,
}

impl Default for OptimizedMsaaConfig {
    fn default() -> Self {
        Self {
            sample_count: MsaaSampleCount::X4,
            color_resolve: ResolveMode::Average,
            depth_resolve: ResolveMode::SampleZero,
            stencil_resolve: ResolveMode::SampleZero,
            per_sample_shading: false,
            min_sample_shading: 1.0,
            alpha_to_coverage: false,
            alpha_to_one: false,
        }
    }
}

/// Create multisample state for pipeline.
pub fn create_multisample_state(config: &OptimizedMsaaConfig) -> vk::PipelineMultisampleStateCreateInfo<'static> {
    vk::PipelineMultisampleStateCreateInfo::default()
        .rasterization_samples(config.sample_count.to_vk())
        .sample_shading_enable(config.per_sample_shading)
        .min_sample_shading(config.min_sample_shading)
        .alpha_to_coverage_enable(config.alpha_to_coverage)
        .alpha_to_one_enable(config.alpha_to_one)
}

/// Multisampled render to single-sampled info for rendering attachment.
pub fn create_ms_to_single_info(
    sample_count: MsaaSampleCount,
) -> vk::MultisampledRenderToSingleSampledInfoEXT<'static> {
    vk::MultisampledRenderToSingleSampledInfoEXT::default()
        .multisampled_render_to_single_sampled_enable(true)
        .rasterization_samples(sample_count.to_vk())
}

/// Create rendering attachment info with resolve.
pub fn create_attachment_with_resolve(
    image_view: vk::ImageView,
    resolve_view: vk::ImageView,
    resolve_mode: ResolveMode,
    load_op: vk::AttachmentLoadOp,
    store_op: vk::AttachmentStoreOp,
    clear_value: vk::ClearValue,
) -> vk::RenderingAttachmentInfo<'static> {
    vk::RenderingAttachmentInfo::default()
        .image_view(image_view)
        .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
        .resolve_image_view(resolve_view)
        .resolve_image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
        .resolve_mode(resolve_mode.to_vk())
        .load_op(load_op)
        .store_op(store_op)
        .clear_value(clear_value)
}

/// Sample pattern for custom MSAA.
#[derive(Debug, Clone)]
pub struct SamplePattern {
    /// Sample positions (normalized 0-1 within pixel).
    pub positions: Vec<[f32; 2]>,
}

impl SamplePattern {
    /// Standard 2x pattern.
    pub fn standard_2x() -> Self {
        Self {
            positions: vec![
                [0.25, 0.25],
                [0.75, 0.75],
            ],
        }
    }

    /// Standard 4x pattern.
    pub fn standard_4x() -> Self {
        Self {
            positions: vec![
                [0.375, 0.125],
                [0.875, 0.375],
                [0.125, 0.625],
                [0.625, 0.875],
            ],
        }
    }

    /// Standard 8x pattern.
    pub fn standard_8x() -> Self {
        Self {
            positions: vec![
                [0.5625, 0.3125],
                [0.4375, 0.6875],
                [0.8125, 0.5625],
                [0.3125, 0.1875],
                [0.1875, 0.8125],
                [0.0625, 0.4375],
                [0.6875, 0.9375],
                [0.9375, 0.0625],
            ],
        }
    }

    /// Rotated grid 4x (better diagonal coverage).
    pub fn rotated_grid_4x() -> Self {
        Self {
            positions: vec![
                [0.125, 0.375],
                [0.375, 0.875],
                [0.625, 0.125],
                [0.875, 0.625],
            ],
        }
    }

    /// Get sample count.
    pub fn sample_count(&self) -> u32 {
        self.positions.len() as u32
    }
}

/// MSAA quality settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsaaQuality {
    /// Off - no MSAA.
    Off,
    /// Low - 2x MSAA.
    Low,
    /// Medium - 4x MSAA.
    Medium,
    /// High - 8x MSAA.
    High,
    /// Ultra - 8x MSAA with sample shading.
    Ultra,
}

impl MsaaQuality {
    /// Get config for quality preset.
    pub fn to_config(&self) -> OptimizedMsaaConfig {
        match self {
            MsaaQuality::Off => OptimizedMsaaConfig {
                sample_count: MsaaSampleCount::None,
                ..Default::default()
            },
            MsaaQuality::Low => OptimizedMsaaConfig {
                sample_count: MsaaSampleCount::X2,
                ..Default::default()
            },
            MsaaQuality::Medium => OptimizedMsaaConfig {
                sample_count: MsaaSampleCount::X4,
                ..Default::default()
            },
            MsaaQuality::High => OptimizedMsaaConfig {
                sample_count: MsaaSampleCount::X8,
                ..Default::default()
            },
            MsaaQuality::Ultra => OptimizedMsaaConfig {
                sample_count: MsaaSampleCount::X8,
                per_sample_shading: true,
                min_sample_shading: 0.5,
                ..Default::default()
            },
        }
    }
}

/// GLSL code for MSAA-aware shading.
pub mod glsl {
    /// Sample mask manipulation.
    pub const SAMPLE_MASK: &str = r#"
// Get sample coverage mask
uint getSampleMask() {
    return gl_SampleMaskIn[0];
}

// Custom alpha-to-coverage
uint alphaToSampleMask(float alpha, uint sampleCount) {
    uint mask = 0u;
    float threshold = 1.0 / float(sampleCount);

    for (uint i = 0u; i < sampleCount; i++) {
        if (alpha > threshold * float(i)) {
            mask |= (1u << i);
        }
    }

    return mask;
}

// Dithered alpha-to-coverage for better transitions
uint alphaToSampleMaskDithered(float alpha, uint sampleCount, vec2 screenPos) {
    // Add noise based on screen position
    float noise = fract(sin(dot(screenPos, vec2(12.9898, 78.233))) * 43758.5453);
    float adjustedAlpha = alpha + (noise - 0.5) * 0.1;

    return alphaToSampleMask(clamp(adjustedAlpha, 0.0, 1.0), sampleCount);
}
"#;

    /// Centroid and sample interpolation.
    pub const INTERPOLATION: &str = r#"
// Sample interpolation qualifiers example:
// layout(location = 0) centroid in vec2 uv;        // Centroid interpolation
// layout(location = 1) sample in vec3 normal;      // Per-sample interpolation
// layout(location = 2) noperspective in vec2 pos;  // No perspective correction

// Manual centroid calculation for complex cases
vec2 calculateCentroid(vec2[8] samplePositions, uint sampleMask, uint sampleCount) {
    vec2 centroid = vec2(0.0);
    uint count = 0u;

    for (uint i = 0u; i < sampleCount; i++) {
        if ((sampleMask & (1u << i)) != 0u) {
            centroid += samplePositions[i];
            count++;
        }
    }

    return count > 0u ? centroid / float(count) : vec2(0.5);
}
"#;

    /// Super-sample shading.
    pub const SUPER_SAMPLE: &str = r#"
// Per-sample shading example
#extension GL_ARB_sample_shading : enable

void main() {
    // gl_SampleID gives current sample index (0 to sampleCount-1)
    // gl_SamplePosition gives normalized position within pixel

    vec2 sampleOffset = gl_SamplePosition - vec2(0.5);

    // Adjust UV for per-sample evaluation
    vec2 adjustedUV = baseUV + sampleOffset * texelSize;

    // Sample texture at adjusted position
    vec4 color = texture(albedoTex, adjustedUV);

    // Output for this sample
    fragColor = color;
}
"#;

    /// MSAA edge detection.
    pub const EDGE_DETECTION: &str = r#"
// Detect MSAA edges for selective processing
bool isMsaaEdge(sampler2DMS depthTex, ivec2 pos, int sampleCount, float threshold) {
    float minDepth = 1.0;
    float maxDepth = 0.0;

    for (int i = 0; i < sampleCount; i++) {
        float d = texelFetch(depthTex, pos, i).r;
        minDepth = min(minDepth, d);
        maxDepth = max(maxDepth, d);
    }

    return (maxDepth - minDepth) > threshold;
}

// Get coverage for adaptive shading
float getMsaaCoverage(sampler2DMS coverageTex, ivec2 pos, int sampleCount) {
    float coverage = 0.0;
    for (int i = 0; i < sampleCount; i++) {
        coverage += texelFetch(coverageTex, pos, i).r;
    }
    return coverage / float(sampleCount);
}
"#;

    /// Custom resolve shader.
    pub const CUSTOM_RESOLVE: &str = r#"
#version 450

layout(binding = 0) uniform sampler2DMS msaaColor;
layout(binding = 1) uniform sampler2DMS msaaDepth;
layout(binding = 0, rgba16f) uniform writeonly image2D resolvedColor;

layout(push_constant) uniform PushConstants {
    ivec2 resolution;
    int sampleCount;
    int resolveMode; // 0=average, 1=min, 2=max, 3=nearest
} pc;

layout(local_size_x = 8, local_size_y = 8) in;

void main() {
    ivec2 pos = ivec2(gl_GlobalInvocationID.xy);

    if (pos.x >= pc.resolution.x || pos.y >= pc.resolution.y) {
        return;
    }

    vec4 result;

    if (pc.resolveMode == 0) {
        // Average resolve
        result = vec4(0.0);
        for (int i = 0; i < pc.sampleCount; i++) {
            result += texelFetch(msaaColor, pos, i);
        }
        result /= float(pc.sampleCount);
    }
    else if (pc.resolveMode == 1) {
        // Min resolve (useful for depth)
        result = texelFetch(msaaColor, pos, 0);
        for (int i = 1; i < pc.sampleCount; i++) {
            result = min(result, texelFetch(msaaColor, pos, i));
        }
    }
    else if (pc.resolveMode == 2) {
        // Max resolve
        result = texelFetch(msaaColor, pos, 0);
        for (int i = 1; i < pc.sampleCount; i++) {
            result = max(result, texelFetch(msaaColor, pos, i));
        }
    }
    else {
        // Nearest depth sample
        float nearestDepth = texelFetch(msaaDepth, pos, 0).r;
        int nearestSample = 0;
        for (int i = 1; i < pc.sampleCount; i++) {
            float d = texelFetch(msaaDepth, pos, i).r;
            if (d < nearestDepth) {
                nearestDepth = d;
                nearestSample = i;
            }
        }
        result = texelFetch(msaaColor, pos, nearestSample);
    }

    imageStore(resolvedColor, pos, result);
}
"#;
}

/// MSAA optimization manager.
pub struct MsaaManager {
    capabilities: MsaaOptimizationCapabilities,
    config: OptimizedMsaaConfig,
}

impl MsaaManager {
    /// Create new MSAA manager.
    pub fn new(ctx: &super::context::VulkanContext) -> Self {
        let capabilities = query_capabilities(ctx);

        Self {
            capabilities,
            config: OptimizedMsaaConfig::default(),
        }
    }

    /// Check if optimized MSAA is available.
    pub fn is_optimized_available(&self) -> bool {
        self.capabilities.supported
    }

    /// Get capabilities.
    pub fn capabilities(&self) -> &MsaaOptimizationCapabilities {
        &self.capabilities
    }

    /// Set quality preset.
    pub fn set_quality(&mut self, quality: MsaaQuality) {
        self.config = quality.to_config();
    }

    /// Set configuration.
    pub fn set_config(&mut self, config: OptimizedMsaaConfig) {
        self.config = config;
    }

    /// Get current configuration.
    pub fn config(&self) -> &OptimizedMsaaConfig {
        &self.config
    }

    /// Check if MSAA is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.sample_count.is_enabled()
    }

    /// Get sample count.
    pub fn sample_count(&self) -> u32 {
        self.config.sample_count.count()
    }

    /// Get Vulkan sample count flags.
    pub fn vk_sample_count(&self) -> vk::SampleCountFlags {
        self.config.sample_count.to_vk()
    }

    /// Create pipeline multisample state.
    pub fn create_multisample_state(&self) -> vk::PipelineMultisampleStateCreateInfo<'static> {
        create_multisample_state(&self.config)
    }

    /// Calculate memory savings vs traditional MSAA.
    pub fn calculate_memory_savings(&self, width: u32, height: u32, bytes_per_pixel: u32) -> u64 {
        if !self.capabilities.supported || !self.is_enabled() {
            return 0;
        }

        let base_size = width as u64 * height as u64 * bytes_per_pixel as u64;
        let sample_count = self.sample_count() as u64;

        // Traditional MSAA needs sample_count * base_size for MSAA buffer
        // Optimized MSAA only needs base_size (resolve happens on-chip)
        base_size * (sample_count - 1)
    }
}
