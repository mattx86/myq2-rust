//! Device Generated Commands for GPU-driven rendering
//!
//! VK_NV_device_generated_commands allows the GPU to generate command buffers:
//! - GPU-driven indirect rendering
//! - Dynamic draw call generation
//! - Reduced CPU overhead for complex scenes
//! - Optimal for visibility culling results

use ash::vk;

/// Device generated commands capabilities.
#[derive(Debug, Clone, Default)]
pub struct DeviceGeneratedCommandsCapabilities {
    /// Whether device generated commands are supported.
    pub supported: bool,
    /// Maximum indirect sequence count.
    pub max_indirect_sequence_count: u32,
    /// Maximum indirect commands token count.
    pub max_indirect_commands_token_count: u32,
    /// Minimum sequences count buffer offset alignment.
    pub min_sequences_count_buffer_offset_alignment: u32,
    /// Minimum sequences index buffer offset alignment.
    pub min_sequences_index_buffer_offset_alignment: u32,
    /// Minimum indirect commands buffer offset alignment.
    pub min_indirect_commands_buffer_offset_alignment: u32,
}

/// Query device generated commands capabilities.
pub fn query_capabilities(ctx: &super::context::VulkanContext) -> DeviceGeneratedCommandsCapabilities {
    let mut dgc_props = vk::PhysicalDeviceDeviceGeneratedCommandsPropertiesNV::default();
    let mut props2 = vk::PhysicalDeviceProperties2::default()
        .push_next(&mut dgc_props);

    unsafe {
        ctx.instance.get_physical_device_properties2(ctx.physical_device, &mut props2);
    }

    let mut dgc_features = vk::PhysicalDeviceDeviceGeneratedCommandsFeaturesNV::default();
    let mut features2 = vk::PhysicalDeviceFeatures2::default()
        .push_next(&mut dgc_features);

    unsafe {
        ctx.instance.get_physical_device_features2(ctx.physical_device, &mut features2);
    }

    DeviceGeneratedCommandsCapabilities {
        supported: dgc_features.device_generated_commands == vk::TRUE,
        max_indirect_sequence_count: dgc_props.max_indirect_sequence_count,
        max_indirect_commands_token_count: dgc_props.max_indirect_commands_token_count,
        min_sequences_count_buffer_offset_alignment: dgc_props.min_sequences_count_buffer_offset_alignment,
        min_sequences_index_buffer_offset_alignment: dgc_props.min_sequences_index_buffer_offset_alignment,
        min_indirect_commands_buffer_offset_alignment: dgc_props.min_indirect_commands_buffer_offset_alignment,
    }
}

/// Indirect command token type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndirectCommandToken {
    /// Shader group binding.
    ShaderGroup,
    /// State flags.
    StateFlags,
    /// Index buffer binding.
    IndexBuffer,
    /// Vertex buffer binding.
    VertexBuffer,
    /// Push constants.
    PushConstant,
    /// Draw indexed command.
    DrawIndexed,
    /// Draw command.
    Draw,
    /// Draw tasks command.
    DrawTasks,
}

impl IndirectCommandToken {
    /// Convert to Vulkan token type.
    pub fn to_vk(&self) -> vk::IndirectCommandsTokenTypeNV {
        match self {
            IndirectCommandToken::ShaderGroup => vk::IndirectCommandsTokenTypeNV::SHADER_GROUP,
            IndirectCommandToken::StateFlags => vk::IndirectCommandsTokenTypeNV::STATE_FLAGS,
            IndirectCommandToken::IndexBuffer => vk::IndirectCommandsTokenTypeNV::INDEX_BUFFER,
            IndirectCommandToken::VertexBuffer => vk::IndirectCommandsTokenTypeNV::VERTEX_BUFFER,
            IndirectCommandToken::PushConstant => vk::IndirectCommandsTokenTypeNV::PUSH_CONSTANT,
            IndirectCommandToken::DrawIndexed => vk::IndirectCommandsTokenTypeNV::DRAW_INDEXED,
            IndirectCommandToken::Draw => vk::IndirectCommandsTokenTypeNV::DRAW,
            IndirectCommandToken::DrawTasks => vk::IndirectCommandsTokenTypeNV::DRAW_TASKS,
        }
    }

    /// Get the size of data for this token.
    pub fn data_size(&self) -> u32 {
        match self {
            IndirectCommandToken::ShaderGroup => 4,
            IndirectCommandToken::StateFlags => 4,
            IndirectCommandToken::IndexBuffer => 16,
            IndirectCommandToken::VertexBuffer => 16,
            IndirectCommandToken::PushConstant => 0, // Variable
            IndirectCommandToken::DrawIndexed => std::mem::size_of::<vk::DrawIndexedIndirectCommand>() as u32,
            IndirectCommandToken::Draw => std::mem::size_of::<vk::DrawIndirectCommand>() as u32,
            IndirectCommandToken::DrawTasks => 8,
        }
    }
}

/// Indirect commands layout token data.
#[derive(Debug, Clone)]
pub struct IndirectCommandsLayoutToken {
    /// Token type.
    pub token_type: IndirectCommandToken,
    /// Stream index.
    pub stream: u32,
    /// Offset within stream.
    pub offset: u32,
}

/// Device generated commands function pointers.
pub struct DeviceGeneratedCommandsFunctions {
    fp_create_indirect_commands_layout: Option<vk::PFN_vkCreateIndirectCommandsLayoutNV>,
    fp_destroy_indirect_commands_layout: Option<vk::PFN_vkDestroyIndirectCommandsLayoutNV>,
    fp_cmd_preprocess: Option<vk::PFN_vkCmdPreprocessGeneratedCommandsNV>,
    fp_cmd_execute: Option<vk::PFN_vkCmdExecuteGeneratedCommandsNV>,
    fp_get_memory_reqs: Option<vk::PFN_vkGetGeneratedCommandsMemoryRequirementsNV>,
}

impl DeviceGeneratedCommandsFunctions {
    /// Load function pointers.
    pub fn new(ctx: &super::context::VulkanContext) -> Self {
        macro_rules! get_fp {
            ($name:literal) => {
                unsafe {
                    let name = std::ffi::CStr::from_bytes_with_nul_unchecked(concat!($name, "\0").as_bytes());
                    ctx.instance.get_device_proc_addr(ctx.device.handle(), name.as_ptr())
                        .map(|fp| std::mem::transmute(fp))
                }
            };
        }

        Self {
            fp_create_indirect_commands_layout: get_fp!("vkCreateIndirectCommandsLayoutNV"),
            fp_destroy_indirect_commands_layout: get_fp!("vkDestroyIndirectCommandsLayoutNV"),
            fp_cmd_preprocess: get_fp!("vkCmdPreprocessGeneratedCommandsNV"),
            fp_cmd_execute: get_fp!("vkCmdExecuteGeneratedCommandsNV"),
            fp_get_memory_reqs: get_fp!("vkGetGeneratedCommandsMemoryRequirementsNV"),
        }
    }

    /// Check if the extension is available.
    pub fn is_available(&self) -> bool {
        self.fp_create_indirect_commands_layout.is_some()
    }
}

/// Indirect commands layout.
pub struct IndirectCommandsLayout {
    /// Layout handle.
    layout: vk::IndirectCommandsLayoutNV,
    /// Tokens in this layout.
    #[allow(dead_code)]
    tokens: Vec<IndirectCommandToken>,
    /// Stream stride.
    stride: u32,
}

impl IndirectCommandsLayout {
    /// Create a new indirect commands layout.
    pub fn new(
        ctx: &super::context::VulkanContext,
        funcs: &DeviceGeneratedCommandsFunctions,
        tokens: &[IndirectCommandToken],
        pipeline_bind_point: vk::PipelineBindPoint,
    ) -> Result<Self, String> {
        let fp = funcs.fp_create_indirect_commands_layout
            .ok_or("Device generated commands not supported")?;

        // Calculate stride and create token infos
        let mut offset = 0u32;
        let token_infos: Vec<vk::IndirectCommandsLayoutTokenNV> = tokens
            .iter()
            .map(|t| {
                let info = vk::IndirectCommandsLayoutTokenNV::default()
                    .token_type(t.to_vk())
                    .stream(0)
                    .offset(offset);
                offset += t.data_size();
                info
            })
            .collect();

        let stride = offset;
        let stream_strides = [stride];

        let create_info = vk::IndirectCommandsLayoutCreateInfoNV::default()
            .flags(vk::IndirectCommandsLayoutUsageFlagsNV::EXPLICIT_PREPROCESS)
            .pipeline_bind_point(pipeline_bind_point)
            .tokens(&token_infos)
            .stream_strides(&stream_strides);

        let mut layout = vk::IndirectCommandsLayoutNV::null();
        let result = unsafe {
            fp(
                ctx.device.handle(),
                &create_info,
                std::ptr::null(),
                &mut layout,
            )
        };

        if result != vk::Result::SUCCESS {
            return Err(format!("Failed to create indirect commands layout: {:?}", result));
        }

        Ok(Self {
            layout,
            tokens: tokens.to_vec(),
            stride,
        })
    }

    /// Get the layout handle.
    pub fn handle(&self) -> vk::IndirectCommandsLayoutNV {
        self.layout
    }

    /// Get the stride.
    pub fn stride(&self) -> u32 {
        self.stride
    }

    /// Destroy the layout.
    pub fn destroy(&self, ctx: &super::context::VulkanContext, funcs: &DeviceGeneratedCommandsFunctions) {
        if let Some(fp) = funcs.fp_destroy_indirect_commands_layout {
            unsafe {
                fp(ctx.device.handle(), self.layout, std::ptr::null());
            }
        }
    }
}

/// Generated commands info for preprocessing.
#[derive(Clone)]
pub struct GeneratedCommandsInfo {
    /// Indirect commands layout.
    pub layout: vk::IndirectCommandsLayoutNV,
    /// Pipeline.
    pub pipeline: vk::Pipeline,
    /// Stream buffer.
    pub stream_buffer: vk::Buffer,
    /// Stream offset.
    pub stream_offset: vk::DeviceSize,
    /// Preprocess buffer.
    pub preprocess_buffer: vk::Buffer,
    /// Preprocess buffer offset.
    pub preprocess_offset: vk::DeviceSize,
    /// Preprocess buffer size.
    pub preprocess_size: vk::DeviceSize,
    /// Sequence count.
    pub sequence_count: u32,
    /// Sequence count buffer (optional).
    pub sequence_count_buffer: vk::Buffer,
    /// Sequence count buffer offset.
    pub sequence_count_offset: vk::DeviceSize,
}

/// Preprocess generated commands.
pub fn preprocess_generated_commands(
    funcs: &DeviceGeneratedCommandsFunctions,
    cmd: vk::CommandBuffer,
    info: &GeneratedCommandsInfo,
) {
    let Some(fp) = funcs.fp_cmd_preprocess else { return };

    let streams = [vk::IndirectCommandsStreamNV {
        buffer: info.stream_buffer,
        offset: info.stream_offset,
    }];

    let gen_info = vk::GeneratedCommandsInfoNV::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .pipeline(info.pipeline)
        .indirect_commands_layout(info.layout)
        .streams(&streams)
        .sequences_count(info.sequence_count)
        .preprocess_buffer(info.preprocess_buffer)
        .preprocess_offset(info.preprocess_offset)
        .preprocess_size(info.preprocess_size)
        .sequences_count_buffer(info.sequence_count_buffer)
        .sequences_count_offset(info.sequence_count_offset);

    unsafe {
        fp(cmd, &gen_info);
    }
}

/// Execute generated commands.
pub fn execute_generated_commands(
    funcs: &DeviceGeneratedCommandsFunctions,
    cmd: vk::CommandBuffer,
    info: &GeneratedCommandsInfo,
    is_preprocessed: bool,
) {
    let Some(fp) = funcs.fp_cmd_execute else { return };

    let streams = [vk::IndirectCommandsStreamNV {
        buffer: info.stream_buffer,
        offset: info.stream_offset,
    }];

    let gen_info = vk::GeneratedCommandsInfoNV::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .pipeline(info.pipeline)
        .indirect_commands_layout(info.layout)
        .streams(&streams)
        .sequences_count(info.sequence_count)
        .preprocess_buffer(info.preprocess_buffer)
        .preprocess_offset(info.preprocess_offset)
        .preprocess_size(info.preprocess_size)
        .sequences_count_buffer(info.sequence_count_buffer)
        .sequences_count_offset(info.sequence_count_offset);

    unsafe {
        fp(cmd, vk::Bool32::from(is_preprocessed), &gen_info);
    }
}

/// Get memory requirements for generated commands preprocessing.
pub fn get_generated_commands_memory_requirements(
    ctx: &super::context::VulkanContext,
    funcs: &DeviceGeneratedCommandsFunctions,
    layout: vk::IndirectCommandsLayoutNV,
    pipeline: vk::Pipeline,
    max_sequence_count: u32,
) -> Option<vk::MemoryRequirements2<'static>> {
    let fp = funcs.fp_get_memory_reqs?;

    let gen_info = vk::GeneratedCommandsMemoryRequirementsInfoNV::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .pipeline(pipeline)
        .indirect_commands_layout(layout)
        .max_sequences_count(max_sequence_count);

    let mut mem_reqs = vk::MemoryRequirements2::default();

    unsafe {
        fp(ctx.device.handle(), &gen_info, &mut mem_reqs);
    }

    Some(mem_reqs)
}

/// Draw command data for indirect buffer.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DrawCommand {
    pub vertex_count: u32,
    pub instance_count: u32,
    pub first_vertex: u32,
    pub first_instance: u32,
}

/// Draw indexed command data for indirect buffer.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DrawIndexedCommand {
    pub index_count: u32,
    pub instance_count: u32,
    pub first_index: u32,
    pub vertex_offset: i32,
    pub first_instance: u32,
}
