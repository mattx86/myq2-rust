//! Vulkan Graphics Pipeline management
//!
//! Manages pre-compiled SPIR-V shaders and baked pipeline state objects.
//! Replaces SDL3 GPU pipelines with direct Vulkan pipelines.

use std::collections::HashMap;
use ash::vk;
use crate::modern::gpu_device;
use crate::modern::RenderError;
use super::manager::ShaderType;

// ============================================================================
// SPIR-V bytecode (compiled at build time by glslc)
// ============================================================================

macro_rules! spv {
    ($name:expr) => {
        include_bytes!(concat!(env!("OUT_DIR"), "/spirv/", $name))
    };
}

// Core geometry
const WORLD_VERT_SPV: &[u8] = spv!("world.vert.spv");
const WORLD_FRAG_SPV: &[u8] = spv!("world.frag.spv");
const WATER_VERT_SPV: &[u8] = spv!("water.vert.spv");
const WATER_FRAG_SPV: &[u8] = spv!("water.frag.spv");
const ALIAS_VERT_SPV: &[u8] = spv!("alias.vert.spv");
const ALIAS_FRAG_SPV: &[u8] = spv!("alias.frag.spv");
const ALIAS_CEL_VERT_SPV: &[u8] = spv!("alias_cel.vert.spv");
const ALIAS_CEL_FRAG_SPV: &[u8] = spv!("alias_cel.frag.spv");
const SKY_VERT_SPV: &[u8] = spv!("sky.vert.spv");
const SKY_FRAG_SPV: &[u8] = spv!("sky.frag.spv");
const PARTICLE_VERT_SPV: &[u8] = spv!("particle.vert.spv");
const PARTICLE_FRAG_SPV: &[u8] = spv!("particle.frag.spv");
const UI_VERT_SPV: &[u8] = spv!("ui.vert.spv");
const UI_FRAG_SPV: &[u8] = spv!("ui.frag.spv");
const DLIGHT_VERT_SPV: &[u8] = spv!("dlight.vert.spv");
const DLIGHT_FRAG_SPV: &[u8] = spv!("dlight.frag.spv");

// Post-processing
const POSTPROCESS_VERT_SPV: &[u8] = spv!("postprocess.vert.spv");
const POSTPROCESS_FRAG_SPV: &[u8] = spv!("postprocess.frag.spv");
const FXAA_FRAG_SPV: &[u8] = spv!("fxaa.frag.spv");
const SSAO_FRAG_SPV: &[u8] = spv!("ssao.frag.spv");
const SSAO_BLUR_FRAG_SPV: &[u8] = spv!("ssao_blur.frag.spv");
const BLOOM_EXTRACT_FRAG_SPV: &[u8] = spv!("bloom_extract.frag.spv");
const BLOOM_BLUR_FRAG_SPV: &[u8] = spv!("bloom_blur.frag.spv");
const BLOOM_COMPOSITE_FRAG_SPV: &[u8] = spv!("bloom_composite.frag.spv");
const FSR_EASU_FRAG_SPV: &[u8] = spv!("fsr_easu.frag.spv");
const FSR_RCAS_FRAG_SPV: &[u8] = spv!("fsr_rcas.frag.spv");
const FSR2_TEMPORAL_FRAG_SPV: &[u8] = spv!("fsr2_temporal.frag.spv");
const MOTION_VECTORS_VERT_SPV: &[u8] = spv!("motion_vectors.vert.spv");
const MOTION_VECTORS_FRAG_SPV: &[u8] = spv!("motion_vectors.frag.spv");

// D3 lighting / shadow cubemap
const SHADOW_CUBE_VERT_SPV: &[u8] = spv!("shadow_cube.vert.spv");
const SHADOW_CUBE_FRAG_SPV: &[u8] = spv!("shadow_cube.frag.spv");
const WORLD_LIT_VERT_SPV: &[u8] = spv!("world_lit.vert.spv");
const WORLD_LIT_FRAG_SPV: &[u8] = spv!("world_lit.frag.spv");
// Projective dynamic shadows: caster reuses alias.vert; resolve is a fullscreen pass.
pub(crate) const SHADOW_CASTER_FRAG_SPV: &[u8] = spv!("shadow_caster.frag.spv");
pub(crate) const SHADOW_RESOLVE_VERT_SPV: &[u8] = spv!("shadow_resolve.vert.spv");
pub(crate) const SHADOW_RESOLVE_FRAG_SPV: &[u8] = spv!("shadow_resolve.frag.spv");
pub(crate) const VXGI_DEBUG_FRAG_SPV: &[u8] = spv!("vxgi_debug.frag.spv");
pub(crate) const VXGI_GI_FRAG_SPV: &[u8] = spv!("vxgi_gi.frag.spv");
pub(crate) const WATER_SHIMMER_FRAG_SPV: &[u8] = spv!("water_shimmer.frag.spv");
pub(crate) const SHADOW_CASTER_VERT_SPV: &[u8] = spv!("alias.vert.spv");
pub(crate) const SHADOW_BSP_VERT_SPV: &[u8] = spv!("shadow_bsp.vert.spv");

// ============================================================================
// Pipeline variant (blend/depth/cull state baked into pipeline)
// ============================================================================

/// Pre-defined pipeline state variants.
///
/// In GL, these were dynamic state changes (glEnable, glBlendFunc, etc.).
/// In Vulkan, they are baked into the pipeline at creation time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PipelineVariant {
    /// Depth test on, depth write on, cull back, no blend.
    Opaque,
    /// Depth test on, depth write off, cull none, alpha blend (src_alpha, 1-src_alpha).
    AlphaBlend,
    /// Depth test on, depth write off, cull none, additive blend (src_alpha, one).
    Additive,
    /// Depth test off, depth write off, cull none, alpha blend. For 2D UI.
    Ui,
    /// Depth test off, depth write off, cull none, no blend. For post-processing.
    PostProcess,
    /// Depth test on, depth write off, cull none, multiplicative blend (dst_color, zero).
    /// Used for detail texture overlay pass.
    Multiplicative,
    /// Depth-only pass (no color attachment). Used for shadow cubemap rendering.
    ShadowDepth,
    /// Additive blend for D3 per-pixel lit pass (depth test on, no depth write, additive blend).
    LitAdditive,
}

// ============================================================================
// Pipeline key (shader type + variant)
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PipelineKey {
    shader: ShaderType,
    variant: PipelineVariant,
}

// ============================================================================
// Vulkan pipeline wrapper
// ============================================================================

/// Wrapper for a Vulkan graphics pipeline.
pub struct GraphicsPipeline {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
}

// ============================================================================
// Pipeline Manager
// ============================================================================

/// Manages all graphics pipelines for the renderer.
///
/// Each pipeline is a combination of (ShaderType, PipelineVariant).
/// Pipelines are created at initialization time and looked up at draw time.
pub struct PipelineManager {
    pipelines: HashMap<PipelineKey, GraphicsPipeline>,
    /// Shared descriptor set layout for per-frame uniforms (set 0).
    descriptor_set_layout: Option<vk::DescriptorSetLayout>,
    /// UI texture descriptor set layout (set 1, binding 0 = COMBINED_IMAGE_SAMPLER).
    ui_texture_set_layout: Option<vk::DescriptorSetLayout>,
    /// Lightmap texture array descriptor set layout (set 2, binding 0 = sampler2DArray).
    lightmap_set_layout: Option<vk::DescriptorSetLayout>,
    /// Shared pipeline layout.
    pipeline_layout: Option<vk::PipelineLayout>,
    /// Minimal pipeline layout for shadow passes (push constants only, no descriptor sets).
    shadow_pipeline_layout: Option<vk::PipelineLayout>,
    /// Projective dynamic shadows — caster depth pass (alias.vert + shadow_caster.frag).
    shadow_caster_pipeline: Option<vk::Pipeline>,
    shadow_caster_layout: Option<vk::PipelineLayout>,
    /// Projective dynamic shadows — fullscreen resolve pass.
    shadow_resolve_pipeline: Option<vk::Pipeline>,
    shadow_resolve_layout: Option<vk::PipelineLayout>,
    vxgi_debug_pipeline: Option<vk::Pipeline>,
    vxgi_debug_layout: Option<vk::PipelineLayout>,
    vxgi_debug_set_layout: Option<vk::DescriptorSetLayout>,
    vxgi_gi_pipeline: Option<vk::Pipeline>,
    vxgi_gi_layout: Option<vk::PipelineLayout>,
    vxgi_gi_set_layout: Option<vk::DescriptorSetLayout>,
    /// Water ripple shimmer pass (caustic light on walls near water).
    water_shimmer_pipeline: Option<vk::Pipeline>,
    water_shimmer_layout: Option<vk::PipelineLayout>,
    water_shimmer_set_layout: Option<vk::DescriptorSetLayout>,
    /// Descriptor set layout for the resolve pass (binding0=scene depth, binding1=shadow map).
    shadow_resolve_set_layout: Option<vk::DescriptorSetLayout>,
    /// BSP/brush caster pipeline (movers) — shadow_bsp.vert + shadow_caster.frag.
    shadow_bsp_pipeline: Option<vk::Pipeline>,
    shadow_bsp_layout: Option<vk::PipelineLayout>,
    initialized: bool,
    color_format: vk::Format,
    depth_format: vk::Format,
    /// Scene FBO color format (R8G8B8A8_UNORM) for 3D pipelines.
    scene_color_format: vk::Format,
    /// Whether EDS3 polygon mode is supported (enables vk_showtris wireframe).
    dynamic_polygon_mode: bool,
}

impl PipelineManager {
    /// Create a new pipeline manager.
    ///
    /// Must be called after the GPU device is initialized.
    pub fn new(
        color_format: vk::Format,
        depth_format: vk::Format,
        dynamic_polygon_mode: bool,
    ) -> Result<Self, RenderError> {
        let mut manager = Self {
            pipelines: HashMap::new(),
            descriptor_set_layout: None,
            ui_texture_set_layout: None,
            lightmap_set_layout: None,
            pipeline_layout: None,
            shadow_pipeline_layout: None,
            shadow_caster_pipeline: None,
            shadow_caster_layout: None,
            shadow_resolve_pipeline: None,
            shadow_resolve_layout: None,
            shadow_resolve_set_layout: None,
            vxgi_debug_pipeline: None,
            vxgi_debug_layout: None,
            vxgi_debug_set_layout: None,
            vxgi_gi_pipeline: None,
            vxgi_gi_layout: None,
            vxgi_gi_set_layout: None,
            water_shimmer_pipeline: None,
            water_shimmer_layout: None,
            water_shimmer_set_layout: None,
            shadow_bsp_pipeline: None,
            shadow_bsp_layout: None,
            initialized: false,
            color_format,
            depth_format,
            scene_color_format: color_format, // default to swapchain format until set
            dynamic_polygon_mode,
        };

        // Initialize shared Vulkan resources
        if let Err(e) = manager.init_shared_resources() {
            return Err(RenderError::Vulkan(format!(
                "Failed to init pipeline resources: {}",
                e
            )));
        }

        manager.initialized = true;
        Ok(manager)
    }

    /// Initialize shared descriptor set layout and pipeline layout.
    fn init_shared_resources(&mut self) -> Result<(), String> {
        gpu_device::with_device(|ctx| {
            // SAFETY: Vulkan context is valid and we're on the main thread.
            unsafe {
                // Create descriptor set layout for uniforms (binding 0 = per-frame, binding 1 = per-object)
                let bindings = [
                    vk::DescriptorSetLayoutBinding::default()
                        .binding(0)
                        .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                        .descriptor_count(1)
                        .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT),
                    vk::DescriptorSetLayoutBinding::default()
                        .binding(1)
                        .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                        .descriptor_count(1)
                        .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT),
                    vk::DescriptorSetLayoutBinding::default()
                        .binding(2)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .descriptor_count(1)
                        .stage_flags(vk::ShaderStageFlags::FRAGMENT),
                ];

                let layout_info = vk::DescriptorSetLayoutCreateInfo::default()
                    .bindings(&bindings);

                let desc_layout = ctx.device
                    .create_descriptor_set_layout(&layout_info, None)
                    .map_err(|e| format!("Failed to create descriptor set layout: {:?}", e))?;

                self.descriptor_set_layout = Some(desc_layout);

                // Create UI texture descriptor set layout (set 1):
                // binding 0 = COMBINED_IMAGE_SAMPLER for 2D texture sampling
                let ui_texture_binding = [
                    vk::DescriptorSetLayoutBinding::default()
                        .binding(0)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .descriptor_count(1)
                        .stage_flags(vk::ShaderStageFlags::FRAGMENT),
                ];
                let ui_tex_layout_info = vk::DescriptorSetLayoutCreateInfo::default()
                    .bindings(&ui_texture_binding);
                let ui_tex_layout = ctx.device
                    .create_descriptor_set_layout(&ui_tex_layout_info, None)
                    .map_err(|e| format!("Failed to create UI texture set layout: {:?}", e))?;
                self.ui_texture_set_layout = Some(ui_tex_layout);

                // Create lightmap texture array descriptor set layout (set 2):
                // binding 0 = COMBINED_IMAGE_SAMPLER for sampler2DArray
                let lightmap_binding = [
                    vk::DescriptorSetLayoutBinding::default()
                        .binding(0)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .descriptor_count(1)
                        .stage_flags(vk::ShaderStageFlags::FRAGMENT),
                ];
                let lightmap_layout_info = vk::DescriptorSetLayoutCreateInfo::default()
                    .bindings(&lightmap_binding);
                let lightmap_layout = ctx.device
                    .create_descriptor_set_layout(&lightmap_layout_info, None)
                    .map_err(|e| format!("Failed to create lightmap set layout: {:?}", e))?;
                self.lightmap_set_layout = Some(lightmap_layout);

                // Create pipeline layout with push constant range covering both
                // vertex and fragment stages. 128 bytes is the Vulkan guaranteed minimum.
                // - UI shader: bytes 0-63 = mat4 projection (vertex stage)
                // - PostProcess shader: bytes 0-31 = polyblend/gamma (fragment stage)
                let push_constant_range = vk::PushConstantRange {
                    stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    offset: 0,
                    size: 128,
                };
                let push_constant_ranges = [push_constant_range];

                // Set 0 = per-frame/per-object uniforms, Set 1 = diffuse texture, Set 2 = lightmap array.
                // Set 3 = lightmap array AGAIN (world_lit dark-fill uses set 2 = shadow cubemap,
                // set 3 = baked lightmap). Set 4 = VXGI irradiance volume (world.frag samples it as
                // a 3D 'lightmap'). All are "1 combined image sampler"; pipelines that don't use a
                // set simply don't bind it, which is allowed.
                let layouts = [desc_layout, ui_tex_layout, lightmap_layout, lightmap_layout, lightmap_layout];
                let layout_info = vk::PipelineLayoutCreateInfo::default()
                    .set_layouts(&layouts)
                    .push_constant_ranges(&push_constant_ranges);

                let pipeline_layout = ctx.device
                    .create_pipeline_layout(&layout_info, None)
                    .map_err(|e| format!("Failed to create pipeline layout: {:?}", e))?;

                self.pipeline_layout = Some(pipeline_layout);

                Ok(())
            }
        }).ok_or_else(|| "No Vulkan context".to_string())?
    }

    /// Create a shader module from SPIR-V bytecode.
    fn create_shader_module(
        device: &ash::Device,
        spirv: &[u8],
    ) -> Result<vk::ShaderModule, vk::Result> {
        // SPIR-V bytecode must be aligned to 4 bytes
        let code: Vec<u32> = spirv
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();

        let create_info = vk::ShaderModuleCreateInfo::default().code(&code);

        // SAFETY: device is valid, create_info contains valid SPIR-V
        unsafe { device.create_shader_module(&create_info, None) }
    }

    /// Get SPIR-V bytecode for a shader type.
    fn get_shader_spirv(shader: ShaderType) -> (&'static [u8], &'static [u8]) {
        match shader {
            ShaderType::World | ShaderType::WorldFlowing => (WORLD_VERT_SPV, WORLD_FRAG_SPV),
            ShaderType::Water => (WATER_VERT_SPV, WATER_FRAG_SPV),
            ShaderType::Alias => (ALIAS_VERT_SPV, ALIAS_FRAG_SPV),
            ShaderType::AliasCel => (ALIAS_CEL_VERT_SPV, ALIAS_CEL_FRAG_SPV),
            ShaderType::Sky => (SKY_VERT_SPV, SKY_FRAG_SPV),
            ShaderType::Particle => (PARTICLE_VERT_SPV, PARTICLE_FRAG_SPV),
            ShaderType::Ui => (UI_VERT_SPV, UI_FRAG_SPV),
            ShaderType::DynamicLight => (DLIGHT_VERT_SPV, DLIGHT_FRAG_SPV),
            ShaderType::PostProcess => (POSTPROCESS_VERT_SPV, POSTPROCESS_FRAG_SPV),
            ShaderType::Fxaa => (POSTPROCESS_VERT_SPV, FXAA_FRAG_SPV),
            ShaderType::Ssao => (POSTPROCESS_VERT_SPV, SSAO_FRAG_SPV),
            ShaderType::SsaoBlur => (POSTPROCESS_VERT_SPV, SSAO_BLUR_FRAG_SPV),
            ShaderType::BloomExtract => (POSTPROCESS_VERT_SPV, BLOOM_EXTRACT_FRAG_SPV),
            ShaderType::BloomBlur => (POSTPROCESS_VERT_SPV, BLOOM_BLUR_FRAG_SPV),
            ShaderType::BloomComposite => (POSTPROCESS_VERT_SPV, BLOOM_COMPOSITE_FRAG_SPV),
            ShaderType::FsrEasu => (POSTPROCESS_VERT_SPV, FSR_EASU_FRAG_SPV),
            ShaderType::FsrRcas => (POSTPROCESS_VERT_SPV, FSR_RCAS_FRAG_SPV),
            ShaderType::Fsr2Temporal => (POSTPROCESS_VERT_SPV, FSR2_TEMPORAL_FRAG_SPV),
            ShaderType::MotionVectors => (MOTION_VECTORS_VERT_SPV, MOTION_VECTORS_FRAG_SPV),
            ShaderType::ShadowCube => (SHADOW_CUBE_VERT_SPV, SHADOW_CUBE_FRAG_SPV),
            ShaderType::WorldLitAdditive => (WORLD_LIT_VERT_SPV, WORLD_LIT_FRAG_SPV),
        }
    }

    /// Get vertex input descriptions for a shader type.
    fn vertex_input_for_shader(shader: ShaderType) -> (
        Vec<vk::VertexInputBindingDescription>,
        Vec<vk::VertexInputAttributeDescription>,
    ) {
        match shader {
            ShaderType::Ui => {
                // Draw2DVertex: pos2 + tex2 + color4 = 32 bytes
                let binding = vec![vk::VertexInputBindingDescription::default()
                    .binding(0)
                    .stride(32)
                    .input_rate(vk::VertexInputRate::VERTEX)];
                let attrs = vec![
                    vk::VertexInputAttributeDescription::default()
                        .binding(0)
                        .location(0)
                        .format(vk::Format::R32G32_SFLOAT)
                        .offset(0),     // pos2
                    vk::VertexInputAttributeDescription::default()
                        .binding(0)
                        .location(1)
                        .format(vk::Format::R32G32_SFLOAT)
                        .offset(8),     // tex2
                    vk::VertexInputAttributeDescription::default()
                        .binding(0)
                        .location(2)
                        .format(vk::Format::R32G32B32A32_SFLOAT)
                        .offset(16),    // color4
                ];
                (binding, attrs)
            }
            ShaderType::World | ShaderType::WorldFlowing | ShaderType::Sky | ShaderType::Water => {
                // BspVertex: pos3(12) + tex2(8) + lm2(8) + lm_layer(4) + normal3(12) + tangent4(16) = 60 bytes
                let binding = vec![vk::VertexInputBindingDescription::default()
                    .binding(0)
                    .stride(60)
                    .input_rate(vk::VertexInputRate::VERTEX)];
                let attrs = vec![
                    vk::VertexInputAttributeDescription::default()
                        .binding(0)
                        .location(0)
                        .format(vk::Format::R32G32B32_SFLOAT)
                        .offset(0),     // position
                    vk::VertexInputAttributeDescription::default()
                        .binding(0)
                        .location(1)
                        .format(vk::Format::R32G32_SFLOAT)
                        .offset(12),    // tex_coord
                    vk::VertexInputAttributeDescription::default()
                        .binding(0)
                        .location(2)
                        .format(vk::Format::R32G32_SFLOAT)
                        .offset(20),    // lm_coord
                    vk::VertexInputAttributeDescription::default()
                        .binding(0)
                        .location(3)
                        .format(vk::Format::R32_SFLOAT)
                        .offset(28),    // lightmap layer index
                ];
                (binding, attrs)
            }
            ShaderType::Particle => {
                // Two bindings: quad VBO (per-vertex) + instance VBO (per-instance)
                let bindings = vec![
                    // Binding 0: quad offset (vec2) — 8 bytes per vertex
                    vk::VertexInputBindingDescription::default()
                        .binding(0)
                        .stride(8)
                        .input_rate(vk::VertexInputRate::VERTEX),
                    // Binding 1: ParticleInstance — 32 bytes per instance
                    vk::VertexInputBindingDescription::default()
                        .binding(1)
                        .stride(32)
                        .input_rate(vk::VertexInputRate::INSTANCE),
                ];
                let attrs = vec![
                    // location 0: quad offset (vec2) from binding 0
                    vk::VertexInputAttributeDescription::default()
                        .binding(0)
                        .location(0)
                        .format(vk::Format::R32G32_SFLOAT)
                        .offset(0),
                    // location 1: origin (vec3) from binding 1
                    vk::VertexInputAttributeDescription::default()
                        .binding(1)
                        .location(1)
                        .format(vk::Format::R32G32B32_SFLOAT)
                        .offset(0),
                    // location 2: color (vec4) from binding 1
                    vk::VertexInputAttributeDescription::default()
                        .binding(1)
                        .location(2)
                        .format(vk::Format::R32G32B32A32_SFLOAT)
                        .offset(12),
                    // location 3: size (float) from binding 1
                    vk::VertexInputAttributeDescription::default()
                        .binding(1)
                        .location(3)
                        .format(vk::Format::R32_SFLOAT)
                        .offset(28),
                ];
                (bindings, attrs)
            }
            ShaderType::PostProcess | ShaderType::Fxaa | ShaderType::Ssao
            | ShaderType::SsaoBlur | ShaderType::BloomExtract | ShaderType::BloomBlur
            | ShaderType::BloomComposite | ShaderType::FsrEasu | ShaderType::FsrRcas
            | ShaderType::Fsr2Temporal => {
                // Fullscreen triangle generated from gl_VertexIndex — no vertex input
                (vec![], vec![])
            }
            ShaderType::Alias | ShaderType::AliasCel => {
                // AliasVertex: pos3 + oldpos3 + tex2 + normal_index(u8) + pad(3) = 36 bytes
                let binding = vec![vk::VertexInputBindingDescription::default()
                    .binding(0)
                    .stride(36)
                    .input_rate(vk::VertexInputRate::VERTEX)];
                let attrs = vec![
                    // location 0: position (vec3)
                    vk::VertexInputAttributeDescription::default()
                        .binding(0)
                        .location(0)
                        .format(vk::Format::R32G32B32_SFLOAT)
                        .offset(0),
                    // location 1: old_position (vec3)
                    vk::VertexInputAttributeDescription::default()
                        .binding(0)
                        .location(1)
                        .format(vk::Format::R32G32B32_SFLOAT)
                        .offset(12),
                    // location 2: tex_coord (vec2)
                    vk::VertexInputAttributeDescription::default()
                        .binding(0)
                        .location(2)
                        .format(vk::Format::R32G32_SFLOAT)
                        .offset(24),
                    // location 3: normal_index (uint8 as R8_UINT)
                    vk::VertexInputAttributeDescription::default()
                        .binding(0)
                        .location(3)
                        .format(vk::Format::R8_UINT)
                        .offset(32),
                ];
                (binding, attrs)
            }
            ShaderType::DynamicLight => {
                // DlightVertex: pos3 + color3 = 24 bytes
                let binding = vec![vk::VertexInputBindingDescription::default()
                    .binding(0)
                    .stride(24)
                    .input_rate(vk::VertexInputRate::VERTEX)];
                let attrs = vec![
                    vk::VertexInputAttributeDescription::default()
                        .binding(0)
                        .location(0)
                        .format(vk::Format::R32G32B32_SFLOAT)
                        .offset(0),     // position
                    vk::VertexInputAttributeDescription::default()
                        .binding(0)
                        .location(1)
                        .format(vk::Format::R32G32B32_SFLOAT)
                        .offset(12),    // color
                ];
                (binding, attrs)
            }
            ShaderType::ShadowCube => {
                // Depth-only pass: only position needed.
                // BspVertex stride is 60 bytes; position is at offset 0.
                let binding = vec![vk::VertexInputBindingDescription::default()
                    .binding(0)
                    .stride(60)
                    .input_rate(vk::VertexInputRate::VERTEX)];
                let attrs = vec![
                    vk::VertexInputAttributeDescription::default()
                        .binding(0)
                        .location(0)
                        .format(vk::Format::R32G32B32_SFLOAT)
                        .offset(0),
                ];
                (binding, attrs)
            }
            ShaderType::WorldLitAdditive => {
                // BspVertex: pos3(12) + tex2(8) + lm2(8) + lm_layer(4) + normal3(12) + tangent4(16) = 60 bytes
                let binding = vec![vk::VertexInputBindingDescription::default()
                    .binding(0)
                    .stride(60)
                    .input_rate(vk::VertexInputRate::VERTEX)];
                let attrs = vec![
                    vk::VertexInputAttributeDescription::default()
                        .binding(0)
                        .location(0)
                        .format(vk::Format::R32G32B32_SFLOAT)
                        .offset(0),     // position
                    vk::VertexInputAttributeDescription::default()
                        .binding(0)
                        .location(1)
                        .format(vk::Format::R32G32_SFLOAT)
                        .offset(12),    // tex_coord
                    vk::VertexInputAttributeDescription::default()
                        .binding(0)
                        .location(2)
                        .format(vk::Format::R32G32_SFLOAT)
                        .offset(20),    // lm_coord (unused in lit shader, but must match binding)
                    vk::VertexInputAttributeDescription::default()
                        .binding(0)
                        .location(3)
                        .format(vk::Format::R32_SFLOAT)
                        .offset(28),    // lm_layer (unused in lit shader)
                ];
                (binding, attrs)
            }
            _ => {
                // Default: pos3 + tex2 + norm3 = 32 bytes
                let binding = vec![vk::VertexInputBindingDescription::default()
                    .binding(0)
                    .stride(32)
                    .input_rate(vk::VertexInputRate::VERTEX)];
                let attrs = vec![
                    vk::VertexInputAttributeDescription::default()
                        .binding(0)
                        .location(0)
                        .format(vk::Format::R32G32B32_SFLOAT)
                        .offset(0),
                    vk::VertexInputAttributeDescription::default()
                        .binding(0)
                        .location(1)
                        .format(vk::Format::R32G32_SFLOAT)
                        .offset(12),
                    vk::VertexInputAttributeDescription::default()
                        .binding(0)
                        .location(2)
                        .format(vk::Format::R32G32B32_SFLOAT)
                        .offset(20),
                ];
                (binding, attrs)
            }
        }
    }

    /// Create a graphics pipeline for a shader type and variant.
    pub fn create_pipeline(
        &mut self,
        shader: ShaderType,
        variant: PipelineVariant,
    ) -> Result<(), String> {
        let key = PipelineKey { shader, variant };
        if self.pipelines.contains_key(&key) {
            return Ok(());
        }

        let pipeline_layout = self.pipeline_layout
            .ok_or("Pipeline layout not initialized")?;

        gpu_device::with_device(|ctx| {
            // SAFETY: Vulkan context is valid
            unsafe {
                let (vert_spv, frag_spv) = Self::get_shader_spirv(shader);

                // Create shader modules
                let vert_module = Self::create_shader_module(&ctx.device, vert_spv)
                    .map_err(|e| format!("Failed to create vertex shader: {:?}", e))?;
                let frag_module = Self::create_shader_module(&ctx.device, frag_spv)
                    .map_err(|e| format!("Failed to create fragment shader: {:?}", e))?;

                let entry_name = std::ffi::CString::new("main").unwrap();

                let shader_stages = [
                    vk::PipelineShaderStageCreateInfo::default()
                        .stage(vk::ShaderStageFlags::VERTEX)
                        .module(vert_module)
                        .name(&entry_name),
                    vk::PipelineShaderStageCreateInfo::default()
                        .stage(vk::ShaderStageFlags::FRAGMENT)
                        .module(frag_module)
                        .name(&entry_name),
                ];

                // Vertex input varies per shader type
                let (binding_descs, attr_descs) = Self::vertex_input_for_shader(shader);

                let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
                    .vertex_binding_descriptions(&binding_descs)
                    .vertex_attribute_descriptions(&attr_descs);

                let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
                    .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
                    .primitive_restart_enable(false);

                // Use dynamic viewport/scissor (+ polygon mode if EDS3 supported for vk_showtris)
                let mut dynamic_states_vec = vec![vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
                if self.dynamic_polygon_mode {
                    dynamic_states_vec.push(vk::DynamicState::POLYGON_MODE_EXT);
                }
                let dynamic_state = vk::PipelineDynamicStateCreateInfo::default()
                    .dynamic_states(&dynamic_states_vec);

                let viewport_state = vk::PipelineViewportStateCreateInfo::default()
                    .viewport_count(1)
                    .scissor_count(1);

                // Rasterization state based on variant
                // TODO: Re-enable back-face culling once winding order is verified
                let (cull_mode, depth_bias_enable) = match variant {
                    PipelineVariant::Opaque => (vk::CullModeFlags::NONE, false),
                    PipelineVariant::AlphaBlend | PipelineVariant::Additive | PipelineVariant::Multiplicative => {
                        (vk::CullModeFlags::NONE, false)
                    }
                    PipelineVariant::Ui | PipelineVariant::PostProcess => {
                        (vk::CullModeFlags::NONE, false)
                    }
                    // Shadow depth: cull back faces to avoid self-shadowing
                    PipelineVariant::ShadowDepth => (vk::CullModeFlags::BACK, false),
                    PipelineVariant::LitAdditive => (vk::CullModeFlags::NONE, false),
                };

                let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
                    .depth_clamp_enable(false)
                    .rasterizer_discard_enable(false)
                    .polygon_mode(vk::PolygonMode::FILL)
                    .line_width(1.0)
                    .cull_mode(cull_mode)
                    // CLOCKWISE because 3D viewports use negative height (Y flip),
                    // which reverses the apparent winding order in framebuffer space.
                    .front_face(vk::FrontFace::CLOCKWISE)
                    .depth_bias_enable(depth_bias_enable);

                let multisampling = vk::PipelineMultisampleStateCreateInfo::default()
                    .sample_shading_enable(false)
                    .rasterization_samples(vk::SampleCountFlags::TYPE_1);

                // Depth state based on variant
                let (depth_test, depth_write) = match variant {
                    PipelineVariant::Opaque => (true, true),
                    PipelineVariant::AlphaBlend | PipelineVariant::Additive | PipelineVariant::Multiplicative => (true, false),
                    PipelineVariant::Ui | PipelineVariant::PostProcess => (false, false),
                    PipelineVariant::ShadowDepth => (true, true),
                    PipelineVariant::LitAdditive => (true, false),
                };

                let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
                    .depth_test_enable(depth_test)
                    .depth_write_enable(depth_write)
                    .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL)
                    .depth_bounds_test_enable(false)
                    .stencil_test_enable(false);

                // Blend state based on variant
                // ShadowDepth has no color attachment; other variants get a color blend attachment.
                let has_color_attachment = variant != PipelineVariant::ShadowDepth;

                let color_blend_attachment = match variant {
                    PipelineVariant::Opaque | PipelineVariant::PostProcess => {
                        vk::PipelineColorBlendAttachmentState::default()
                            .color_write_mask(vk::ColorComponentFlags::RGBA)
                            .blend_enable(false)
                    }
                    PipelineVariant::AlphaBlend | PipelineVariant::Ui => {
                        vk::PipelineColorBlendAttachmentState::default()
                            .color_write_mask(vk::ColorComponentFlags::RGBA)
                            .blend_enable(true)
                            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
                            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                            .color_blend_op(vk::BlendOp::ADD)
                            .src_alpha_blend_factor(vk::BlendFactor::ONE)
                            .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
                            .alpha_blend_op(vk::BlendOp::ADD)
                    }
                    PipelineVariant::Additive | PipelineVariant::LitAdditive => {
                        vk::PipelineColorBlendAttachmentState::default()
                            .color_write_mask(vk::ColorComponentFlags::RGBA)
                            .blend_enable(true)
                            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
                            .dst_color_blend_factor(vk::BlendFactor::ONE)
                            .color_blend_op(vk::BlendOp::ADD)
                            .src_alpha_blend_factor(vk::BlendFactor::ONE)
                            .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
                            .alpha_blend_op(vk::BlendOp::ADD)
                    }
                    PipelineVariant::Multiplicative => {
                        // final = src_color * dst_color (detail texture overlay)
                        vk::PipelineColorBlendAttachmentState::default()
                            .color_write_mask(vk::ColorComponentFlags::RGBA)
                            .blend_enable(true)
                            .src_color_blend_factor(vk::BlendFactor::DST_COLOR)
                            .dst_color_blend_factor(vk::BlendFactor::ZERO)
                            .color_blend_op(vk::BlendOp::ADD)
                            .src_alpha_blend_factor(vk::BlendFactor::DST_ALPHA)
                            .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
                            .alpha_blend_op(vk::BlendOp::ADD)
                    }
                    // ShadowDepth: no color attachment — this value is unused
                    PipelineVariant::ShadowDepth => {
                        vk::PipelineColorBlendAttachmentState::default()
                            .color_write_mask(vk::ColorComponentFlags::empty())
                            .blend_enable(false)
                    }
                };

                let color_blend_attachments_slice: &[vk::PipelineColorBlendAttachmentState] =
                    if has_color_attachment { std::slice::from_ref(&color_blend_attachment) } else { &[] };
                let color_blending = vk::PipelineColorBlendStateCreateInfo::default()
                    .logic_op_enable(false)
                    .attachments(color_blend_attachments_slice);

                // Dynamic rendering info (Vulkan 1.3)
                // 3D variants (Opaque, AlphaBlend, Additive) render to scene_fbo
                // UI and PostProcess render to swapchain
                // ShadowDepth: no color attachment (depth-only cubemap face)
                let color_fmt = match variant {
                    PipelineVariant::Opaque | PipelineVariant::AlphaBlend | PipelineVariant::Additive
                    | PipelineVariant::Multiplicative | PipelineVariant::LitAdditive => {
                        self.scene_color_format
                    }
                    _ => self.color_format,
                };
                let color_formats_arr = [color_fmt];
                let color_formats_slice: &[vk::Format] =
                    if has_color_attachment { &color_formats_arr } else { &[] };
                let depth_fmt = match variant {
                    PipelineVariant::Ui | PipelineVariant::PostProcess => vk::Format::UNDEFINED,
                    PipelineVariant::ShadowDepth => vk::Format::D32_SFLOAT,
                    _ => self.depth_format,
                };
                let mut rendering_info = vk::PipelineRenderingCreateInfo::default()
                    .color_attachment_formats(color_formats_slice)
                    .depth_attachment_format(depth_fmt);

                // Create pipeline
                let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
                    .stages(&shader_stages)
                    .vertex_input_state(&vertex_input)
                    .input_assembly_state(&input_assembly)
                    .viewport_state(&viewport_state)
                    .rasterization_state(&rasterizer)
                    .multisample_state(&multisampling)
                    .depth_stencil_state(&depth_stencil)
                    .color_blend_state(&color_blending)
                    .dynamic_state(&dynamic_state)
                    .layout(pipeline_layout)
                    .push_next(&mut rendering_info);

                let pipelines = ctx.device
                    .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info], None)
                    .map_err(|e| format!("Failed to create pipeline: {:?}", e.1))?;

                // Clean up shader modules
                ctx.device.destroy_shader_module(vert_module, None);
                ctx.device.destroy_shader_module(frag_module, None);

                // Store pipeline
                self.pipelines.insert(
                    key,
                    GraphicsPipeline {
                        pipeline: pipelines[0],
                        layout: pipeline_layout,
                    },
                );

                Ok(())
            }
        }).ok_or_else(|| "No Vulkan context".to_string())?
    }

    /// Create the shadow cubemap depth pipeline using a traditional render pass.
    ///
    /// This must be called separately from `create_pipeline` because depth-only
    /// pipelines use a traditional render pass, not dynamic rendering.
    pub fn create_shadow_pipeline(&mut self, render_pass: vk::RenderPass) -> Result<(), String> {
        let key = PipelineKey { shader: ShaderType::ShadowCube, variant: PipelineVariant::ShadowDepth };
        if self.pipelines.contains_key(&key) {
            return Ok(());
        }

        // Create a minimal pipeline layout for the shadow pass: push constants only,
        // no descriptor sets.  Using the shared layout (which declares sets 0/1/2) would
        // require those sets to be bound before the shadow draw even though the shadow
        // shaders never access them.  Some drivers (notably Intel) silently discard draw
        // calls when descriptor sets declared in the layout are not bound.
        let pipeline_layout = if let Some(existing) = self.shadow_pipeline_layout {
            existing
        } else {
            let new_layout = gpu_device::with_device(|ctx| {
                unsafe {
                    let push_range = vk::PushConstantRange {
                        stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                        offset: 0,
                        size: 80, // ShadowCubePushConstants: mat4(64) + vec3(12) + f32(4)
                    };
                    let layout_info = vk::PipelineLayoutCreateInfo::default()
                        .push_constant_ranges(std::slice::from_ref(&push_range));
                    ctx.device.create_pipeline_layout(&layout_info, None).ok()
                }
            }).flatten().ok_or("Failed to create shadow pipeline layout")?;
            self.shadow_pipeline_layout = Some(new_layout);
            new_layout
        };

        gpu_device::with_device(|ctx| {
            // SAFETY: Vulkan context is valid; render_pass is a valid handle.
            unsafe {
                let vert_module = Self::create_shader_module(&ctx.device, SHADOW_CUBE_VERT_SPV)
                    .map_err(|e| format!("Failed to create shadow vert shader: {:?}", e))?;
                let frag_module = Self::create_shader_module(&ctx.device, SHADOW_CUBE_FRAG_SPV)
                    .map_err(|e| format!("Failed to create shadow frag shader: {:?}", e))?;

                let entry_name = std::ffi::CString::new("main").unwrap();

                let shader_stages = [
                    vk::PipelineShaderStageCreateInfo::default()
                        .stage(vk::ShaderStageFlags::VERTEX)
                        .module(vert_module)
                        .name(&entry_name),
                    vk::PipelineShaderStageCreateInfo::default()
                        .stage(vk::ShaderStageFlags::FRAGMENT)
                        .module(frag_module)
                        .name(&entry_name),
                ];

                // Position-only vertex input (stride=60 matches BspVertex, position at offset 0)
                let binding = [vk::VertexInputBindingDescription::default()
                    .binding(0)
                    .stride(60)
                    .input_rate(vk::VertexInputRate::VERTEX)];
                let attrs = [vk::VertexInputAttributeDescription::default()
                    .binding(0)
                    .location(0)
                    .format(vk::Format::R32G32B32_SFLOAT)
                    .offset(0)];

                let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
                    .vertex_binding_descriptions(&binding)
                    .vertex_attribute_descriptions(&attrs);

                let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
                    .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
                    .primitive_restart_enable(false);

                let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
                let dynamic_state = vk::PipelineDynamicStateCreateInfo::default()
                    .dynamic_states(&dynamic_states);

                let viewport_state = vk::PipelineViewportStateCreateInfo::default()
                    .viewport_count(1)
                    .scissor_count(1);

                let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
                    .depth_clamp_enable(false)
                    .rasterizer_discard_enable(false)
                    .polygon_mode(vk::PolygonMode::FILL)
                    .line_width(1.0)
                    // Shadow pass uses a standard (non-Y-flipped) viewport, so triangles
                    // appear in their natural CCW winding.  The main 3D pass uses CLOCKWISE
                    // because it flips viewport height; the shadow pass does not.
                    // Use CULL_NONE to match the main world pipeline and avoid accidentally
                    // discarding all geometry (which would leave depth maps at clear=1.0).
                    .cull_mode(vk::CullModeFlags::NONE)
                    .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
                    .depth_bias_enable(false);

                let multisampling = vk::PipelineMultisampleStateCreateInfo::default()
                    .sample_shading_enable(false)
                    .rasterization_samples(vk::SampleCountFlags::TYPE_1);

                let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
                    .depth_test_enable(true)
                    .depth_write_enable(true)
                    .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL)
                    .depth_bounds_test_enable(false)
                    .stencil_test_enable(false);

                // One R32_SFLOAT colour attachment — writes dist/radius (no blending needed).
                let color_blend_attachment = vk::PipelineColorBlendAttachmentState::default()
                    .color_write_mask(vk::ColorComponentFlags::R)
                    .blend_enable(false);
                let color_blending = vk::PipelineColorBlendStateCreateInfo::default()
                    .logic_op_enable(false)
                    .attachments(std::slice::from_ref(&color_blend_attachment));

                let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
                    .stages(&shader_stages)
                    .vertex_input_state(&vertex_input)
                    .input_assembly_state(&input_assembly)
                    .viewport_state(&viewport_state)
                    .rasterization_state(&rasterizer)
                    .multisample_state(&multisampling)
                    .depth_stencil_state(&depth_stencil)
                    .color_blend_state(&color_blending)
                    .dynamic_state(&dynamic_state)
                    .layout(pipeline_layout)
                    .render_pass(render_pass)
                    .subpass(0);

                let pipelines = ctx.device
                    .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info], None)
                    .map_err(|e| format!("Failed to create shadow pipeline: {:?}", e.1))?;

                ctx.device.destroy_shader_module(vert_module, None);
                ctx.device.destroy_shader_module(frag_module, None);

                self.pipelines.insert(
                    key,
                    GraphicsPipeline {
                        pipeline: pipelines[0],
                        layout: pipeline_layout,
                    },
                );

                Ok(())
            }
        }).ok_or_else(|| "No Vulkan context".to_string())?
    }

    pub fn shadow_caster_pipeline(&self) -> Option<(vk::Pipeline, vk::PipelineLayout)> {
        match (self.shadow_caster_pipeline, self.shadow_caster_layout) {
            (Some(p), Some(l)) => Some((p, l)),
            _ => None,
        }
    }
    pub fn shadow_resolve_pipeline(&self) -> Option<(vk::Pipeline, vk::PipelineLayout)> {
        match (self.shadow_resolve_pipeline, self.shadow_resolve_layout) {
            (Some(p), Some(l)) => Some((p, l)),
            _ => None,
        }
    }
    pub fn shadow_resolve_set_layout(&self) -> Option<vk::DescriptorSetLayout> {
        self.shadow_resolve_set_layout
    }
    pub fn shadow_bsp_pipeline(&self) -> Option<(vk::Pipeline, vk::PipelineLayout)> {
        match (self.shadow_bsp_pipeline, self.shadow_bsp_layout) {
            (Some(p), Some(l)) => Some((p, l)),
            _ => None,
        }
    }

    /// BSP/brush caster pipeline for projective shadows (movers). Position-only BspVertex
    /// (stride 60) + shadow_bsp.vert + shadow_caster.frag, into the RG caster render pass.
    pub fn create_shadow_bsp_pipeline(&mut self, render_pass: vk::RenderPass) -> Result<(), String> {
        if self.shadow_bsp_pipeline.is_some() {
            return Ok(());
        }
        let layout = if let Some(l) = self.shadow_bsp_layout {
            l
        } else {
            let l = gpu_device::with_device(|ctx| unsafe {
                let push = vk::PushConstantRange {
                    stage_flags: vk::ShaderStageFlags::VERTEX,
                    offset: 0,
                    size: 68, // mat4 MVP + float floor depth
                };
                let info = vk::PipelineLayoutCreateInfo::default()
                    .push_constant_ranges(std::slice::from_ref(&push));
                ctx.device.create_pipeline_layout(&info, None).ok()
            }).flatten().ok_or("bsp shadow layout")?;
            self.shadow_bsp_layout = Some(l);
            l
        };

        gpu_device::with_device(|ctx| unsafe {
            let vert = Self::create_shader_module(&ctx.device, SHADOW_BSP_VERT_SPV)
                .map_err(|e| format!("bsp vert: {:?}", e))?;
            let frag = Self::create_shader_module(&ctx.device, SHADOW_CASTER_FRAG_SPV)
                .map_err(|e| format!("bsp frag: {:?}", e))?;
            let name = std::ffi::CString::new("main").unwrap();
            let stages = [
                vk::PipelineShaderStageCreateInfo::default()
                    .stage(vk::ShaderStageFlags::VERTEX).module(vert).name(&name),
                vk::PipelineShaderStageCreateInfo::default()
                    .stage(vk::ShaderStageFlags::FRAGMENT).module(frag).name(&name),
            ];
            // BspVertex: position at offset 0, stride 60.
            let binding = [vk::VertexInputBindingDescription::default()
                .binding(0).stride(60).input_rate(vk::VertexInputRate::VERTEX)];
            let attrs = [vk::VertexInputAttributeDescription::default()
                .binding(0).location(0).format(vk::Format::R32G32B32_SFLOAT).offset(0)];
            let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
                .vertex_binding_descriptions(&binding).vertex_attribute_descriptions(&attrs);
            let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
                .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
            let dyn_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
            let dynamic_state = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyn_states);
            let viewport_state = vk::PipelineViewportStateCreateInfo::default()
                .viewport_count(1).scissor_count(1);
            let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
                .polygon_mode(vk::PolygonMode::FILL).line_width(1.0)
                .cull_mode(vk::CullModeFlags::NONE).front_face(vk::FrontFace::COUNTER_CLOCKWISE);
            let multisampling = vk::PipelineMultisampleStateCreateInfo::default()
                .rasterization_samples(vk::SampleCountFlags::TYPE_1);
            let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
                .depth_test_enable(true).depth_write_enable(true)
                .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL);
            let blend_attach = vk::PipelineColorBlendAttachmentState::default()
                .color_write_mask(vk::ColorComponentFlags::R | vk::ColorComponentFlags::G)
                .blend_enable(false);
            let blending = vk::PipelineColorBlendStateCreateInfo::default()
                .attachments(std::slice::from_ref(&blend_attach));
            let info = vk::GraphicsPipelineCreateInfo::default()
                .stages(&stages).vertex_input_state(&vertex_input)
                .input_assembly_state(&input_assembly).viewport_state(&viewport_state)
                .rasterization_state(&rasterizer).multisample_state(&multisampling)
                .depth_stencil_state(&depth_stencil).color_blend_state(&blending)
                .dynamic_state(&dynamic_state).layout(layout).render_pass(render_pass).subpass(0);
            let pipes = ctx.device
                .create_graphics_pipelines(vk::PipelineCache::null(), &[info], None)
                .map_err(|e| format!("bsp pipeline: {:?}", e.1))?;
            ctx.device.destroy_shader_module(vert, None);
            ctx.device.destroy_shader_module(frag, None);
            self.shadow_bsp_pipeline = Some(pipes[0]);
            Ok::<(), String>(())
        }).ok_or_else(|| "No Vulkan context".to_string())?
    }

    /// Caster depth pipeline for projective dynamic shadows: alias.vert (frame lerp) +
    /// shadow_caster.frag, drawn with the light's view-projection as the MVP into the shared
    /// shadow render pass (R32 colour = light-space depth + D32 depth test).
    pub fn create_shadow_caster_pipeline(&mut self, render_pass: vk::RenderPass) -> Result<(), String> {
        if self.shadow_caster_pipeline.is_some() {
            return Ok(());
        }
        // Layout: 128-byte alias push constants (we push the same struct, MVP = light VP*model),
        // no descriptor sets.
        let layout = if let Some(l) = self.shadow_caster_layout {
            l
        } else {
            let l = gpu_device::with_device(|ctx| unsafe {
                let push = vk::PushConstantRange {
                    stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    offset: 0,
                    size: 128,
                };
                let info = vk::PipelineLayoutCreateInfo::default()
                    .push_constant_ranges(std::slice::from_ref(&push));
                ctx.device.create_pipeline_layout(&info, None).ok()
            }).flatten().ok_or("shadow caster layout")?;
            self.shadow_caster_layout = Some(l);
            l
        };

        gpu_device::with_device(|ctx| unsafe {
            let vert = Self::create_shader_module(&ctx.device, SHADOW_CASTER_VERT_SPV)
                .map_err(|e| format!("caster vert: {:?}", e))?;
            let frag = Self::create_shader_module(&ctx.device, SHADOW_CASTER_FRAG_SPV)
                .map_err(|e| format!("caster frag: {:?}", e))?;
            let name = std::ffi::CString::new("main").unwrap();
            let stages = [
                vk::PipelineShaderStageCreateInfo::default()
                    .stage(vk::ShaderStageFlags::VERTEX).module(vert).name(&name),
                vk::PipelineShaderStageCreateInfo::default()
                    .stage(vk::ShaderStageFlags::FRAGMENT).module(frag).name(&name),
            ];
            // AliasVertex: pos3 + oldpos3 + tex2 + normal_index(u8) = stride 36.
            let binding = [vk::VertexInputBindingDescription::default()
                .binding(0).stride(36).input_rate(vk::VertexInputRate::VERTEX)];
            let attrs = [
                vk::VertexInputAttributeDescription::default().binding(0).location(0)
                    .format(vk::Format::R32G32B32_SFLOAT).offset(0),
                vk::VertexInputAttributeDescription::default().binding(0).location(1)
                    .format(vk::Format::R32G32B32_SFLOAT).offset(12),
                vk::VertexInputAttributeDescription::default().binding(0).location(2)
                    .format(vk::Format::R32G32_SFLOAT).offset(24),
                vk::VertexInputAttributeDescription::default().binding(0).location(3)
                    .format(vk::Format::R8_UINT).offset(32),
            ];
            let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
                .vertex_binding_descriptions(&binding).vertex_attribute_descriptions(&attrs);
            let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
                .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
            let dyn_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
            let dynamic_state = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyn_states);
            let viewport_state = vk::PipelineViewportStateCreateInfo::default()
                .viewport_count(1).scissor_count(1);
            let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
                .polygon_mode(vk::PolygonMode::FILL).line_width(1.0)
                .cull_mode(vk::CullModeFlags::NONE).front_face(vk::FrontFace::COUNTER_CLOCKWISE);
            let multisampling = vk::PipelineMultisampleStateCreateInfo::default()
                .rasterization_samples(vk::SampleCountFlags::TYPE_1);
            let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
                .depth_test_enable(true).depth_write_enable(true)
                .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL);
            let blend_attach = vk::PipelineColorBlendAttachmentState::default()
                .color_write_mask(vk::ColorComponentFlags::R | vk::ColorComponentFlags::G)
                .blend_enable(false);
            let blending = vk::PipelineColorBlendStateCreateInfo::default()
                .attachments(std::slice::from_ref(&blend_attach));
            let info = vk::GraphicsPipelineCreateInfo::default()
                .stages(&stages).vertex_input_state(&vertex_input)
                .input_assembly_state(&input_assembly).viewport_state(&viewport_state)
                .rasterization_state(&rasterizer).multisample_state(&multisampling)
                .depth_stencil_state(&depth_stencil).color_blend_state(&blending)
                .dynamic_state(&dynamic_state).layout(layout).render_pass(render_pass).subpass(0);
            let pipes = ctx.device
                .create_graphics_pipelines(vk::PipelineCache::null(), &[info], None)
                .map_err(|e| format!("caster pipeline: {:?}", e.1))?;
            ctx.device.destroy_shader_module(vert, None);
            ctx.device.destroy_shader_module(frag, None);
            self.shadow_caster_pipeline = Some(pipes[0]);
            Ok::<(), String>(())
        }).ok_or_else(|| "No Vulkan context".to_string())?
    }

    /// Fullscreen resolve pipeline for projective dynamic shadows. Samples scene depth
    /// (binding 0) and the caster shadow map (binding 1), and multiplicatively darkens the
    /// scene colour where shadowed. `render_pass` must have one colour attachment in the
    /// scene-colour format; depth is sampled, not attached.
    pub fn create_shadow_resolve_pipeline(&mut self, render_pass: vk::RenderPass) -> Result<(), String> {
        if self.shadow_resolve_pipeline.is_some() {
            return Ok(());
        }
        let set_layout = if let Some(sl) = self.shadow_resolve_set_layout {
            sl
        } else {
            let sl = gpu_device::with_device(|ctx| unsafe {
                let bindings = [
                    vk::DescriptorSetLayoutBinding::default().binding(0).descriptor_count(1)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .stage_flags(vk::ShaderStageFlags::FRAGMENT),
                    vk::DescriptorSetLayoutBinding::default().binding(1).descriptor_count(1)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .stage_flags(vk::ShaderStageFlags::FRAGMENT),
                ];
                let info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);
                ctx.device.create_descriptor_set_layout(&info, None).ok()
            }).flatten().ok_or("resolve set layout")?;
            self.shadow_resolve_set_layout = Some(sl);
            sl
        };
        let layout = if let Some(l) = self.shadow_resolve_layout {
            l
        } else {
            let sl = set_layout;
            let l = gpu_device::with_device(|ctx| unsafe {
                let push = vk::PushConstantRange {
                    stage_flags: vk::ShaderStageFlags::FRAGMENT,
                    offset: 0,
                    size: 112, // mat4(64) + 4 floats + vec3 cam_proj + cam_dim + near_skip
                };
                let info = vk::PipelineLayoutCreateInfo::default()
                    .set_layouts(std::slice::from_ref(&sl))
                    .push_constant_ranges(std::slice::from_ref(&push));
                ctx.device.create_pipeline_layout(&info, None).ok()
            }).flatten().ok_or("resolve layout")?;
            self.shadow_resolve_layout = Some(l);
            l
        };

        gpu_device::with_device(|ctx| unsafe {
            let vert = Self::create_shader_module(&ctx.device, SHADOW_RESOLVE_VERT_SPV)
                .map_err(|e| format!("resolve vert: {:?}", e))?;
            let frag = Self::create_shader_module(&ctx.device, SHADOW_RESOLVE_FRAG_SPV)
                .map_err(|e| format!("resolve frag: {:?}", e))?;
            let name = std::ffi::CString::new("main").unwrap();
            let stages = [
                vk::PipelineShaderStageCreateInfo::default()
                    .stage(vk::ShaderStageFlags::VERTEX).module(vert).name(&name),
                vk::PipelineShaderStageCreateInfo::default()
                    .stage(vk::ShaderStageFlags::FRAGMENT).module(frag).name(&name),
            ];
            // No vertex buffer — fullscreen triangle generated from gl_VertexIndex.
            let vertex_input = vk::PipelineVertexInputStateCreateInfo::default();
            let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
                .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
            let dyn_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
            let dynamic_state = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyn_states);
            let viewport_state = vk::PipelineViewportStateCreateInfo::default()
                .viewport_count(1).scissor_count(1);
            let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
                .polygon_mode(vk::PolygonMode::FILL).line_width(1.0)
                .cull_mode(vk::CullModeFlags::NONE).front_face(vk::FrontFace::COUNTER_CLOCKWISE);
            let multisampling = vk::PipelineMultisampleStateCreateInfo::default()
                .rasterization_samples(vk::SampleCountFlags::TYPE_1);
            let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
                .depth_test_enable(false).depth_write_enable(false);
            // Multiplicative blend: result = src * dst (src = vec3(1-darkness)) → darken scene.
            let blend_attach = vk::PipelineColorBlendAttachmentState::default()
                .color_write_mask(vk::ColorComponentFlags::RGBA)
                .blend_enable(true)
                .src_color_blend_factor(vk::BlendFactor::DST_COLOR)
                .dst_color_blend_factor(vk::BlendFactor::ZERO)
                .color_blend_op(vk::BlendOp::ADD)
                .src_alpha_blend_factor(vk::BlendFactor::ONE)
                .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
                .alpha_blend_op(vk::BlendOp::ADD);
            let blending = vk::PipelineColorBlendStateCreateInfo::default()
                .attachments(std::slice::from_ref(&blend_attach));
            let info = vk::GraphicsPipelineCreateInfo::default()
                .stages(&stages).vertex_input_state(&vertex_input)
                .input_assembly_state(&input_assembly).viewport_state(&viewport_state)
                .rasterization_state(&rasterizer).multisample_state(&multisampling)
                .depth_stencil_state(&depth_stencil).color_blend_state(&blending)
                .dynamic_state(&dynamic_state).layout(layout).render_pass(render_pass).subpass(0);
            let pipes = ctx.device
                .create_graphics_pipelines(vk::PipelineCache::null(), &[info], None)
                .map_err(|e| format!("resolve pipeline: {:?}", e.1))?;
            ctx.device.destroy_shader_module(vert, None);
            ctx.device.destroy_shader_module(frag, None);
            self.shadow_resolve_pipeline = Some(pipes[0]);
            Ok::<(), String>(())
        }).ok_or_else(|| "No Vulkan context".to_string())?
    }

    /// VXGI debug raymarch pipeline: fullscreen pass (dynamic rendering into the scene colour)
    /// sampling the voxel grid (set 0, binding 0). Opaque write on hit, discard on miss.
    pub fn create_vxgi_debug_pipeline(&mut self) -> Result<(), String> {
        if self.vxgi_debug_pipeline.is_some() {
            return Ok(());
        }
        let set_layout = if let Some(sl) = self.vxgi_debug_set_layout {
            sl
        } else {
            let sl = gpu_device::with_device(|ctx| unsafe {
                let b = [vk::DescriptorSetLayoutBinding::default().binding(0).descriptor_count(1)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .stage_flags(vk::ShaderStageFlags::FRAGMENT)];
                let info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&b);
                ctx.device.create_descriptor_set_layout(&info, None).ok()
            }).flatten().ok_or("vxgi set layout")?;
            self.vxgi_debug_set_layout = Some(sl);
            sl
        };
        let layout = if let Some(l) = self.vxgi_debug_layout {
            l
        } else {
            let sl = set_layout;
            let l = gpu_device::with_device(|ctx| unsafe {
                let push = vk::PushConstantRange {
                    stage_flags: vk::ShaderStageFlags::FRAGMENT,
                    offset: 0,
                    size: 96, // mat4 invVP(64) + cam(12+pad4) + gridMin(12) + extent(4)
                };
                let info = vk::PipelineLayoutCreateInfo::default()
                    .set_layouts(std::slice::from_ref(&sl))
                    .push_constant_ranges(std::slice::from_ref(&push));
                ctx.device.create_pipeline_layout(&info, None).ok()
            }).flatten().ok_or("vxgi layout")?;
            self.vxgi_debug_layout = Some(l);
            l
        };

        let color_fmt = self.scene_color_format;
        gpu_device::with_device(|ctx| unsafe {
            let vert = Self::create_shader_module(&ctx.device, SHADOW_RESOLVE_VERT_SPV)
                .map_err(|e| format!("vxgi vert: {:?}", e))?;
            let frag = Self::create_shader_module(&ctx.device, VXGI_DEBUG_FRAG_SPV)
                .map_err(|e| format!("vxgi frag: {:?}", e))?;
            let name = std::ffi::CString::new("main").unwrap();
            let stages = [
                vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(vert).name(&name),
                vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(frag).name(&name),
            ];
            let vertex_input = vk::PipelineVertexInputStateCreateInfo::default();
            let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
                .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
            let dyn_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
            let dynamic_state = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyn_states);
            let viewport_state = vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
            let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
                .polygon_mode(vk::PolygonMode::FILL).line_width(1.0)
                .cull_mode(vk::CullModeFlags::NONE).front_face(vk::FrontFace::COUNTER_CLOCKWISE);
            let multisampling = vk::PipelineMultisampleStateCreateInfo::default()
                .rasterization_samples(vk::SampleCountFlags::TYPE_1);
            let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
                .depth_test_enable(false).depth_write_enable(false);
            let blend_attach = vk::PipelineColorBlendAttachmentState::default()
                .color_write_mask(vk::ColorComponentFlags::RGBA).blend_enable(false);
            let blending = vk::PipelineColorBlendStateCreateInfo::default()
                .attachments(std::slice::from_ref(&blend_attach));
            // Dynamic rendering: one colour attachment in the scene HDR format, no depth.
            let color_formats = [color_fmt];
            let mut rendering = vk::PipelineRenderingCreateInfo::default()
                .color_attachment_formats(&color_formats);
            let info = vk::GraphicsPipelineCreateInfo::default()
                .stages(&stages).vertex_input_state(&vertex_input)
                .input_assembly_state(&input_assembly).viewport_state(&viewport_state)
                .rasterization_state(&rasterizer).multisample_state(&multisampling)
                .depth_stencil_state(&depth_stencil).color_blend_state(&blending)
                .dynamic_state(&dynamic_state).layout(layout)
                .push_next(&mut rendering);
            let pipes = ctx.device
                .create_graphics_pipelines(vk::PipelineCache::null(), &[info], None)
                .map_err(|e| format!("vxgi pipeline: {:?}", e.1))?;
            ctx.device.destroy_shader_module(vert, None);
            ctx.device.destroy_shader_module(frag, None);
            self.vxgi_debug_pipeline = Some(pipes[0]);
            Ok::<(), String>(())
        }).ok_or_else(|| "No Vulkan context".to_string())?
    }

    /// (pipeline, layout) for the VXGI debug pass.
    pub fn vxgi_debug_pipeline(&self) -> Option<(vk::Pipeline, vk::PipelineLayout)> {
        Some((self.vxgi_debug_pipeline?, self.vxgi_debug_layout?))
    }

    /// VXGI diffuse-GI pipeline: fullscreen, samples scene depth (set 0 b0) + radiance volume
    /// (set 0 b1), cone-traces, and ADDS the bounced light to the scene (additive blend).
    pub fn create_vxgi_gi_pipeline(&mut self) -> Result<(), String> {
        if self.vxgi_gi_pipeline.is_some() {
            return Ok(());
        }
        let set_layout = if let Some(sl) = self.vxgi_gi_set_layout {
            sl
        } else {
            let sl = gpu_device::with_device(|ctx| unsafe {
                let b = [
                    vk::DescriptorSetLayoutBinding::default().binding(0).descriptor_count(1)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                    vk::DescriptorSetLayoutBinding::default().binding(1).descriptor_count(1)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                    vk::DescriptorSetLayoutBinding::default().binding(2).descriptor_count(1)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                    vk::DescriptorSetLayoutBinding::default().binding(3).descriptor_count(1)
                        .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                ];
                let info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&b);
                ctx.device.create_descriptor_set_layout(&info, None).ok()
            }).flatten().ok_or("vxgi gi set layout")?;
            self.vxgi_gi_set_layout = Some(sl);
            sl
        };
        let layout = if let Some(l) = self.vxgi_gi_layout {
            l
        } else {
            let sl = set_layout;
            let l = gpu_device::with_device(|ctx| unsafe {
                let push = vk::PushConstantRange {
                    stage_flags: vk::ShaderStageFlags::FRAGMENT, offset: 0, size: 112,
                };
                let info = vk::PipelineLayoutCreateInfo::default()
                    .set_layouts(std::slice::from_ref(&sl)).push_constant_ranges(std::slice::from_ref(&push));
                ctx.device.create_pipeline_layout(&info, None).ok()
            }).flatten().ok_or("vxgi gi layout")?;
            self.vxgi_gi_layout = Some(l);
            l
        };

        let color_fmt = self.scene_color_format;
        gpu_device::with_device(|ctx| unsafe {
            let vert = Self::create_shader_module(&ctx.device, SHADOW_RESOLVE_VERT_SPV).map_err(|e| format!("gi vert: {:?}", e))?;
            let frag = Self::create_shader_module(&ctx.device, VXGI_GI_FRAG_SPV).map_err(|e| format!("gi frag: {:?}", e))?;
            let name = std::ffi::CString::new("main").unwrap();
            let stages = [
                vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(vert).name(&name),
                vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(frag).name(&name),
            ];
            let vertex_input = vk::PipelineVertexInputStateCreateInfo::default();
            let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
            let dyn_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
            let dynamic_state = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyn_states);
            let viewport_state = vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
            let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
                .polygon_mode(vk::PolygonMode::FILL).line_width(1.0)
                .cull_mode(vk::CullModeFlags::NONE).front_face(vk::FrontFace::COUNTER_CLOCKWISE);
            let multisampling = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
            let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default().depth_test_enable(false).depth_write_enable(false);
            // Multiplicative: result = scene × (1 + irradiance·strength). src×DST_COLOR + dst×ONE
            // = dst×(src+1). The GI shader outputs irradiance, so this brightens the per-texel
            // surface in place and KEEPS texture detail (vs. a flat additive average colour).
            let blend_attach = vk::PipelineColorBlendAttachmentState::default()
                .color_write_mask(vk::ColorComponentFlags::RGBA).blend_enable(true)
                .src_color_blend_factor(vk::BlendFactor::DST_COLOR).dst_color_blend_factor(vk::BlendFactor::ONE).color_blend_op(vk::BlendOp::ADD)
                .src_alpha_blend_factor(vk::BlendFactor::ONE).dst_alpha_blend_factor(vk::BlendFactor::ZERO).alpha_blend_op(vk::BlendOp::ADD);
            let blending = vk::PipelineColorBlendStateCreateInfo::default().attachments(std::slice::from_ref(&blend_attach));
            let color_formats = [color_fmt];
            let mut rendering = vk::PipelineRenderingCreateInfo::default().color_attachment_formats(&color_formats);
            let info = vk::GraphicsPipelineCreateInfo::default()
                .stages(&stages).vertex_input_state(&vertex_input).input_assembly_state(&input_assembly)
                .viewport_state(&viewport_state).rasterization_state(&rasterizer).multisample_state(&multisampling)
                .depth_stencil_state(&depth_stencil).color_blend_state(&blending).dynamic_state(&dynamic_state)
                .layout(layout).push_next(&mut rendering);
            let pipes = ctx.device.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None)
                .map_err(|e| format!("gi pipeline: {:?}", e.1))?;
            ctx.device.destroy_shader_module(vert, None);
            ctx.device.destroy_shader_module(frag, None);
            self.vxgi_gi_pipeline = Some(pipes[0]);
            Ok::<(), String>(())
        }).ok_or_else(|| "No Vulkan context".to_string())?
    }

    /// (pipeline, layout) for the VXGI GI pass.
    pub fn vxgi_gi_pipeline(&self) -> Option<(vk::Pipeline, vk::PipelineLayout)> {
        Some((self.vxgi_gi_pipeline?, self.vxgi_gi_layout?))
    }

    /// Descriptor set layout (set 0) for the VXGI GI pass (depth + radiance).
    pub fn vxgi_gi_set_layout(&self) -> Option<vk::DescriptorSetLayout> {
        self.vxgi_gi_set_layout
    }

    /// Descriptor set layout (set 0) for the VXGI debug voxel sampler.
    pub fn vxgi_debug_set_layout(&self) -> Option<vk::DescriptorSetLayout> {
        self.vxgi_debug_set_layout
    }

    /// Water ripple shimmer pipeline: fullscreen, samples scene depth (set 0 b0) + the VXGI
    /// irradiance volume (set 0 b1), and ADDS animated caustic ripple light around the active
    /// water plane (additive blend into the scene HDR target).
    pub fn create_water_shimmer_pipeline(&mut self) -> Result<(), String> {
        if self.water_shimmer_pipeline.is_some() {
            return Ok(());
        }
        let set_layout = if let Some(sl) = self.water_shimmer_set_layout {
            sl
        } else {
            let sl = gpu_device::with_device(|ctx| unsafe {
                let b = [
                    vk::DescriptorSetLayoutBinding::default().binding(0).descriptor_count(1)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                    vk::DescriptorSetLayoutBinding::default().binding(1).descriptor_count(1)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                ];
                let info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&b);
                ctx.device.create_descriptor_set_layout(&info, None).ok()
            }).flatten().ok_or("shimmer set layout")?;
            self.water_shimmer_set_layout = Some(sl);
            sl
        };
        let layout = if let Some(l) = self.water_shimmer_layout {
            l
        } else {
            let sl = set_layout;
            let l = gpu_device::with_device(|ctx| unsafe {
                let push = vk::PushConstantRange {
                    stage_flags: vk::ShaderStageFlags::FRAGMENT, offset: 0, size: 96,
                };
                let info = vk::PipelineLayoutCreateInfo::default()
                    .set_layouts(std::slice::from_ref(&sl)).push_constant_ranges(std::slice::from_ref(&push));
                ctx.device.create_pipeline_layout(&info, None).ok()
            }).flatten().ok_or("shimmer layout")?;
            self.water_shimmer_layout = Some(l);
            l
        };

        let color_fmt = self.scene_color_format;
        gpu_device::with_device(|ctx| unsafe {
            let vert = Self::create_shader_module(&ctx.device, SHADOW_RESOLVE_VERT_SPV).map_err(|e| format!("shimmer vert: {:?}", e))?;
            let frag = Self::create_shader_module(&ctx.device, WATER_SHIMMER_FRAG_SPV).map_err(|e| format!("shimmer frag: {:?}", e))?;
            let name = std::ffi::CString::new("main").unwrap();
            let stages = [
                vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(vert).name(&name),
                vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(frag).name(&name),
            ];
            let vertex_input = vk::PipelineVertexInputStateCreateInfo::default();
            let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
            let dyn_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
            let dynamic_state = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyn_states);
            let viewport_state = vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
            let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
                .polygon_mode(vk::PolygonMode::FILL).line_width(1.0)
                .cull_mode(vk::CullModeFlags::NONE).front_face(vk::FrontFace::COUNTER_CLOCKWISE);
            let multisampling = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
            let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default().depth_test_enable(false).depth_write_enable(false);
            // Pure additive: shimmer light adds on top of the shaded scene.
            let blend_attach = vk::PipelineColorBlendAttachmentState::default()
                .color_write_mask(vk::ColorComponentFlags::RGBA).blend_enable(true)
                .src_color_blend_factor(vk::BlendFactor::ONE).dst_color_blend_factor(vk::BlendFactor::ONE).color_blend_op(vk::BlendOp::ADD)
                .src_alpha_blend_factor(vk::BlendFactor::ZERO).dst_alpha_blend_factor(vk::BlendFactor::ONE).alpha_blend_op(vk::BlendOp::ADD);
            let blending = vk::PipelineColorBlendStateCreateInfo::default().attachments(std::slice::from_ref(&blend_attach));
            let color_formats = [color_fmt];
            let mut rendering = vk::PipelineRenderingCreateInfo::default().color_attachment_formats(&color_formats);
            let info = vk::GraphicsPipelineCreateInfo::default()
                .stages(&stages).vertex_input_state(&vertex_input).input_assembly_state(&input_assembly)
                .viewport_state(&viewport_state).rasterization_state(&rasterizer).multisample_state(&multisampling)
                .depth_stencil_state(&depth_stencil).color_blend_state(&blending).dynamic_state(&dynamic_state)
                .layout(layout).push_next(&mut rendering);
            let pipes = ctx.device.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None)
                .map_err(|e| format!("shimmer pipeline: {:?}", e.1))?;
            ctx.device.destroy_shader_module(vert, None);
            ctx.device.destroy_shader_module(frag, None);
            self.water_shimmer_pipeline = Some(pipes[0]);
            Ok::<(), String>(())
        }).ok_or_else(|| "No Vulkan context".to_string())?
    }

    /// (pipeline, layout) for the water shimmer pass.
    pub fn water_shimmer_pipeline(&self) -> Option<(vk::Pipeline, vk::PipelineLayout)> {
        Some((self.water_shimmer_pipeline?, self.water_shimmer_layout?))
    }

    /// Descriptor set layout (set 0) for the water shimmer pass (depth + irradiance).
    pub fn water_shimmer_set_layout(&self) -> Option<vk::DescriptorSetLayout> {
        self.water_shimmer_set_layout
    }

    /// Get a pipeline for rendering.
    pub fn get(
        &self,
        shader: ShaderType,
        variant: PipelineVariant,
    ) -> Option<&GraphicsPipeline> {
        self.pipelines.get(&PipelineKey { shader, variant })
    }

    /// Get descriptor set layout for binding.
    pub fn descriptor_set_layout(&self) -> Option<vk::DescriptorSetLayout> {
        self.descriptor_set_layout
    }

    /// Get the UI texture descriptor set layout (set 1) for TextureStore.
    pub fn ui_texture_set_layout(&self) -> Option<vk::DescriptorSetLayout> {
        self.ui_texture_set_layout
    }

    /// Get the lightmap texture array descriptor set layout (set 2).
    pub fn lightmap_set_layout(&self) -> Option<vk::DescriptorSetLayout> {
        self.lightmap_set_layout
    }

    /// Set the scene FBO color format for 3D pipelines.
    pub fn set_scene_format(&mut self, fmt: vk::Format) {
        self.scene_color_format = fmt;
    }

    /// Check if the pipeline manager is initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Shutdown and release all pipelines.
    pub fn shutdown(&mut self) {
        gpu_device::with_device(|ctx| {
            // SAFETY: Vulkan context is valid
            unsafe {
                // Destroy all pipelines
                for (_, pipeline) in self.pipelines.drain() {
                    ctx.device.destroy_pipeline(pipeline.pipeline, None);
                }

                // Destroy pipeline layouts
                if let Some(layout) = self.shadow_pipeline_layout.take() {
                    ctx.device.destroy_pipeline_layout(layout, None);
                }
                if let Some(layout) = self.pipeline_layout.take() {
                    ctx.device.destroy_pipeline_layout(layout, None);
                }

                // Destroy descriptor set layouts
                if let Some(layout) = self.lightmap_set_layout.take() {
                    ctx.device.destroy_descriptor_set_layout(layout, None);
                }
                if let Some(layout) = self.ui_texture_set_layout.take() {
                    ctx.device.destroy_descriptor_set_layout(layout, None);
                }
                if let Some(layout) = self.descriptor_set_layout.take() {
                    ctx.device.destroy_descriptor_set_layout(layout, None);
                }
            }
        });
        self.initialized = false;
    }
}

impl Default for PipelineManager {
    fn default() -> Self {
        Self {
            pipelines: HashMap::new(),
            descriptor_set_layout: None,
            ui_texture_set_layout: None,
            lightmap_set_layout: None,
            pipeline_layout: None,
            shadow_pipeline_layout: None,
            shadow_caster_pipeline: None,
            shadow_caster_layout: None,
            shadow_resolve_pipeline: None,
            shadow_resolve_layout: None,
            shadow_resolve_set_layout: None,
            vxgi_debug_pipeline: None,
            vxgi_debug_layout: None,
            vxgi_debug_set_layout: None,
            vxgi_gi_pipeline: None,
            vxgi_gi_layout: None,
            vxgi_gi_set_layout: None,
            water_shimmer_pipeline: None,
            water_shimmer_layout: None,
            water_shimmer_set_layout: None,
            shadow_bsp_pipeline: None,
            shadow_bsp_layout: None,
            initialized: false,
            color_format: vk::Format::R8G8B8A8_UNORM,
            depth_format: vk::Format::D32_SFLOAT,
            scene_color_format: vk::Format::R8G8B8A8_UNORM,
            dynamic_polygon_mode: false,
        }
    }
}
