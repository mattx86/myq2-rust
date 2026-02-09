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
    /// Shared descriptor set layout for per-frame uniforms.
    descriptor_set_layout: Option<vk::DescriptorSetLayout>,
    /// Shared pipeline layout.
    pipeline_layout: Option<vk::PipelineLayout>,
    initialized: bool,
    color_format: vk::Format,
    depth_format: vk::Format,
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
            pipeline_layout: None,
            initialized: false,
            color_format,
            depth_format,
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

                // Create pipeline layout
                let layouts = [desc_layout];
                let layout_info = vk::PipelineLayoutCreateInfo::default()
                    .set_layouts(&layouts);

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

                // Vertex input (position, texcoord, normal)
                let binding_desc = [vk::VertexInputBindingDescription::default()
                    .binding(0)
                    .stride(32) // 3 pos + 2 tex + 3 norm = 8 floats
                    .input_rate(vk::VertexInputRate::VERTEX)];

                let attr_descs = [
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

                let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
                    .vertex_binding_descriptions(&binding_desc)
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
                let (cull_mode, depth_bias_enable) = match variant {
                    PipelineVariant::Opaque => (vk::CullModeFlags::BACK, false),
                    PipelineVariant::AlphaBlend | PipelineVariant::Additive => {
                        (vk::CullModeFlags::NONE, false)
                    }
                    PipelineVariant::Ui | PipelineVariant::PostProcess => {
                        (vk::CullModeFlags::NONE, false)
                    }
                };

                let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
                    .depth_clamp_enable(false)
                    .rasterizer_discard_enable(false)
                    .polygon_mode(vk::PolygonMode::FILL)
                    .line_width(1.0)
                    .cull_mode(cull_mode)
                    .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
                    .depth_bias_enable(depth_bias_enable);

                let multisampling = vk::PipelineMultisampleStateCreateInfo::default()
                    .sample_shading_enable(false)
                    .rasterization_samples(vk::SampleCountFlags::TYPE_1);

                // Depth state based on variant
                let (depth_test, depth_write) = match variant {
                    PipelineVariant::Opaque => (true, true),
                    PipelineVariant::AlphaBlend | PipelineVariant::Additive => (true, false),
                    PipelineVariant::Ui | PipelineVariant::PostProcess => (false, false),
                };

                let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
                    .depth_test_enable(depth_test)
                    .depth_write_enable(depth_write)
                    .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL)
                    .depth_bounds_test_enable(false)
                    .stencil_test_enable(false);

                // Blend state based on variant
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
                    PipelineVariant::Additive => {
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
                };

                let color_blend_attachments = [color_blend_attachment];
                let color_blending = vk::PipelineColorBlendStateCreateInfo::default()
                    .logic_op_enable(false)
                    .attachments(&color_blend_attachments);

                // Dynamic rendering info (Vulkan 1.3)
                let color_formats = [self.color_format];
                let mut rendering_info = vk::PipelineRenderingCreateInfo::default()
                    .color_attachment_formats(&color_formats)
                    .depth_attachment_format(self.depth_format);

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

                // Destroy pipeline layout
                if let Some(layout) = self.pipeline_layout.take() {
                    ctx.device.destroy_pipeline_layout(layout, None);
                }

                // Destroy descriptor set layout
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
            pipeline_layout: None,
            initialized: false,
            color_format: vk::Format::R8G8B8A8_UNORM,
            depth_format: vk::Format::D32_SFLOAT,
            dynamic_polygon_mode: false,
        }
    }
}
