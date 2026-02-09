//! Extended Dynamic State 3 for reduced pipeline permutations
//!
//! VK_EXT_extended_dynamic_state3 adds more state that can be set dynamically,
//! reducing the number of pipeline objects needed:
//! - Polygon mode (fill, line, point)
//! - Rasterization samples
//! - Sample mask
//! - Alpha to coverage
//! - Alpha to one
//! - Logic op enable
//! - Color blend enable/equation/write mask
//! - Depth clamp enable
//! - Viewport/scissor with count
//! - Conservative rasterization
//! - Line rasterization mode

use ash::vk;

/// Extended dynamic state 3 capabilities.
#[derive(Debug, Clone, Default)]
pub struct DynamicState3Capabilities {
    /// Whether basic EDS3 is supported.
    pub supported: bool,
    /// Tessellation domain origin.
    pub tessellation_domain_origin: bool,
    /// Depth clamp enable.
    pub depth_clamp_enable: bool,
    /// Polygon mode.
    pub polygon_mode: bool,
    /// Rasterization samples.
    pub rasterization_samples: bool,
    /// Sample mask.
    pub sample_mask: bool,
    /// Alpha to coverage enable.
    pub alpha_to_coverage_enable: bool,
    /// Alpha to one enable.
    pub alpha_to_one_enable: bool,
    /// Logic op enable.
    pub logic_op_enable: bool,
    /// Color blend enable.
    pub color_blend_enable: bool,
    /// Color blend equation.
    pub color_blend_equation: bool,
    /// Color write mask.
    pub color_write_mask: bool,
    /// Rasterization stream.
    pub rasterization_stream: bool,
    /// Conservative rasterization mode.
    pub conservative_rasterization_mode: bool,
    /// Extra primitive overestimation size.
    pub extra_primitive_overestimation_size: bool,
    /// Depth clip enable.
    pub depth_clip_enable: bool,
    /// Sample locations enable.
    pub sample_locations_enable: bool,
    /// Color blend advanced.
    pub color_blend_advanced: bool,
    /// Provoking vertex mode.
    pub provoking_vertex_mode: bool,
    /// Line rasterization mode.
    pub line_rasterization_mode: bool,
    /// Line stipple enable.
    pub line_stipple_enable: bool,
    /// Depth clip negative one to one.
    pub depth_clip_negative_one_to_one: bool,
    /// Viewport W scaling enable.
    pub viewport_w_scaling_enable: bool,
    /// Viewport swizzle.
    pub viewport_swizzle: bool,
    /// Coverage to color enable.
    pub coverage_to_color_enable: bool,
    /// Coverage to color location.
    pub coverage_to_color_location: bool,
    /// Coverage modulation mode.
    pub coverage_modulation_mode: bool,
    /// Coverage modulation table enable.
    pub coverage_modulation_table_enable: bool,
    /// Coverage modulation table.
    pub coverage_modulation_table: bool,
    /// Coverage reduction mode.
    pub coverage_reduction_mode: bool,
    /// Representative fragment test enable.
    pub representative_fragment_test_enable: bool,
    /// Shading rate image enable.
    pub shading_rate_image_enable: bool,
}

/// Query dynamic state 3 capabilities.
pub fn query_capabilities(ctx: &super::context::VulkanContext) -> DynamicState3Capabilities {
    let mut eds3_features = vk::PhysicalDeviceExtendedDynamicState3FeaturesEXT::default();
    let mut features2 = vk::PhysicalDeviceFeatures2::default()
        .push_next(&mut eds3_features);

    unsafe {
        ctx.instance.get_physical_device_features2(ctx.physical_device, &mut features2);
    }

    DynamicState3Capabilities {
        supported: eds3_features.extended_dynamic_state3_tessellation_domain_origin == vk::TRUE
            || eds3_features.extended_dynamic_state3_polygon_mode == vk::TRUE,
        tessellation_domain_origin: eds3_features.extended_dynamic_state3_tessellation_domain_origin == vk::TRUE,
        depth_clamp_enable: eds3_features.extended_dynamic_state3_depth_clamp_enable == vk::TRUE,
        polygon_mode: eds3_features.extended_dynamic_state3_polygon_mode == vk::TRUE,
        rasterization_samples: eds3_features.extended_dynamic_state3_rasterization_samples == vk::TRUE,
        sample_mask: eds3_features.extended_dynamic_state3_sample_mask == vk::TRUE,
        alpha_to_coverage_enable: eds3_features.extended_dynamic_state3_alpha_to_coverage_enable == vk::TRUE,
        alpha_to_one_enable: eds3_features.extended_dynamic_state3_alpha_to_one_enable == vk::TRUE,
        logic_op_enable: eds3_features.extended_dynamic_state3_logic_op_enable == vk::TRUE,
        color_blend_enable: eds3_features.extended_dynamic_state3_color_blend_enable == vk::TRUE,
        color_blend_equation: eds3_features.extended_dynamic_state3_color_blend_equation == vk::TRUE,
        color_write_mask: eds3_features.extended_dynamic_state3_color_write_mask == vk::TRUE,
        rasterization_stream: eds3_features.extended_dynamic_state3_rasterization_stream == vk::TRUE,
        conservative_rasterization_mode: eds3_features.extended_dynamic_state3_conservative_rasterization_mode == vk::TRUE,
        extra_primitive_overestimation_size: eds3_features.extended_dynamic_state3_extra_primitive_overestimation_size == vk::TRUE,
        depth_clip_enable: eds3_features.extended_dynamic_state3_depth_clip_enable == vk::TRUE,
        sample_locations_enable: eds3_features.extended_dynamic_state3_sample_locations_enable == vk::TRUE,
        color_blend_advanced: eds3_features.extended_dynamic_state3_color_blend_advanced == vk::TRUE,
        provoking_vertex_mode: eds3_features.extended_dynamic_state3_provoking_vertex_mode == vk::TRUE,
        line_rasterization_mode: eds3_features.extended_dynamic_state3_line_rasterization_mode == vk::TRUE,
        line_stipple_enable: eds3_features.extended_dynamic_state3_line_stipple_enable == vk::TRUE,
        depth_clip_negative_one_to_one: eds3_features.extended_dynamic_state3_depth_clip_negative_one_to_one == vk::TRUE,
        viewport_w_scaling_enable: eds3_features.extended_dynamic_state3_viewport_w_scaling_enable == vk::TRUE,
        viewport_swizzle: eds3_features.extended_dynamic_state3_viewport_swizzle == vk::TRUE,
        coverage_to_color_enable: eds3_features.extended_dynamic_state3_coverage_to_color_enable == vk::TRUE,
        coverage_to_color_location: eds3_features.extended_dynamic_state3_coverage_to_color_location == vk::TRUE,
        coverage_modulation_mode: eds3_features.extended_dynamic_state3_coverage_modulation_mode == vk::TRUE,
        coverage_modulation_table_enable: eds3_features.extended_dynamic_state3_coverage_modulation_table_enable == vk::TRUE,
        coverage_modulation_table: eds3_features.extended_dynamic_state3_coverage_modulation_table == vk::TRUE,
        coverage_reduction_mode: eds3_features.extended_dynamic_state3_coverage_reduction_mode == vk::TRUE,
        representative_fragment_test_enable: eds3_features.extended_dynamic_state3_representative_fragment_test_enable == vk::TRUE,
        shading_rate_image_enable: eds3_features.extended_dynamic_state3_shading_rate_image_enable == vk::TRUE,
    }
}

/// Dynamic state 3 command helper.
pub struct DynamicState3Commands {
    /// Function pointers for EDS3 commands.
    fp_set_polygon_mode: Option<vk::PFN_vkCmdSetPolygonModeEXT>,
    fp_set_rasterization_samples: Option<vk::PFN_vkCmdSetRasterizationSamplesEXT>,
    fp_set_sample_mask: Option<vk::PFN_vkCmdSetSampleMaskEXT>,
    fp_set_alpha_to_coverage: Option<vk::PFN_vkCmdSetAlphaToCoverageEnableEXT>,
    fp_set_alpha_to_one: Option<vk::PFN_vkCmdSetAlphaToOneEnableEXT>,
    fp_set_logic_op_enable: Option<vk::PFN_vkCmdSetLogicOpEnableEXT>,
    fp_set_color_blend_enable: Option<vk::PFN_vkCmdSetColorBlendEnableEXT>,
    fp_set_color_blend_equation: Option<vk::PFN_vkCmdSetColorBlendEquationEXT>,
    fp_set_color_write_mask: Option<vk::PFN_vkCmdSetColorWriteMaskEXT>,
    fp_set_depth_clamp_enable: Option<vk::PFN_vkCmdSetDepthClampEnableEXT>,
    fp_set_line_rasterization_mode: Option<vk::PFN_vkCmdSetLineRasterizationModeEXT>,
    fp_set_line_stipple_enable: Option<vk::PFN_vkCmdSetLineStippleEnableEXT>,
    /// Capabilities.
    caps: DynamicState3Capabilities,
}

impl DynamicState3Commands {
    /// Create dynamic state 3 command helper.
    pub fn new(ctx: &super::context::VulkanContext) -> Self {
        let caps = query_capabilities(ctx);

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
            fp_set_polygon_mode: get_fp!("vkCmdSetPolygonModeEXT"),
            fp_set_rasterization_samples: get_fp!("vkCmdSetRasterizationSamplesEXT"),
            fp_set_sample_mask: get_fp!("vkCmdSetSampleMaskEXT"),
            fp_set_alpha_to_coverage: get_fp!("vkCmdSetAlphaToCoverageEnableEXT"),
            fp_set_alpha_to_one: get_fp!("vkCmdSetAlphaToOneEnableEXT"),
            fp_set_logic_op_enable: get_fp!("vkCmdSetLogicOpEnableEXT"),
            fp_set_color_blend_enable: get_fp!("vkCmdSetColorBlendEnableEXT"),
            fp_set_color_blend_equation: get_fp!("vkCmdSetColorBlendEquationEXT"),
            fp_set_color_write_mask: get_fp!("vkCmdSetColorWriteMaskEXT"),
            fp_set_depth_clamp_enable: get_fp!("vkCmdSetDepthClampEnableEXT"),
            fp_set_line_rasterization_mode: get_fp!("vkCmdSetLineRasterizationModeEXT"),
            fp_set_line_stipple_enable: get_fp!("vkCmdSetLineStippleEnableEXT"),
            caps,
        }
    }

    /// Get capabilities.
    pub fn capabilities(&self) -> &DynamicState3Capabilities {
        &self.caps
    }

    /// Set polygon mode dynamically.
    pub fn set_polygon_mode(&self, cmd: vk::CommandBuffer, mode: vk::PolygonMode) {
        if let Some(fp) = self.fp_set_polygon_mode {
            unsafe { fp(cmd, mode); }
        }
    }

    /// Set rasterization samples dynamically.
    pub fn set_rasterization_samples(&self, cmd: vk::CommandBuffer, samples: vk::SampleCountFlags) {
        if let Some(fp) = self.fp_set_rasterization_samples {
            unsafe { fp(cmd, samples); }
        }
    }

    /// Set sample mask dynamically.
    pub fn set_sample_mask(&self, cmd: vk::CommandBuffer, samples: vk::SampleCountFlags, sample_mask: &[vk::SampleMask]) {
        if let Some(fp) = self.fp_set_sample_mask {
            unsafe { fp(cmd, samples, sample_mask.as_ptr()); }
        }
    }

    /// Set alpha to coverage enable dynamically.
    pub fn set_alpha_to_coverage_enable(&self, cmd: vk::CommandBuffer, enable: bool) {
        if let Some(fp) = self.fp_set_alpha_to_coverage {
            unsafe { fp(cmd, if enable { vk::TRUE } else { vk::FALSE }); }
        }
    }

    /// Set alpha to one enable dynamically.
    pub fn set_alpha_to_one_enable(&self, cmd: vk::CommandBuffer, enable: bool) {
        if let Some(fp) = self.fp_set_alpha_to_one {
            unsafe { fp(cmd, if enable { vk::TRUE } else { vk::FALSE }); }
        }
    }

    /// Set logic op enable dynamically.
    pub fn set_logic_op_enable(&self, cmd: vk::CommandBuffer, enable: bool) {
        if let Some(fp) = self.fp_set_logic_op_enable {
            unsafe { fp(cmd, if enable { vk::TRUE } else { vk::FALSE }); }
        }
    }

    /// Set color blend enable dynamically.
    pub fn set_color_blend_enable(&self, cmd: vk::CommandBuffer, first_attachment: u32, enables: &[vk::Bool32]) {
        if let Some(fp) = self.fp_set_color_blend_enable {
            unsafe { fp(cmd, first_attachment, enables.len() as u32, enables.as_ptr()); }
        }
    }

    /// Set color blend equation dynamically.
    pub fn set_color_blend_equation(&self, cmd: vk::CommandBuffer, first_attachment: u32, equations: &[vk::ColorBlendEquationEXT]) {
        if let Some(fp) = self.fp_set_color_blend_equation {
            unsafe { fp(cmd, first_attachment, equations.len() as u32, equations.as_ptr()); }
        }
    }

    /// Set color write mask dynamically.
    pub fn set_color_write_mask(&self, cmd: vk::CommandBuffer, first_attachment: u32, masks: &[vk::ColorComponentFlags]) {
        if let Some(fp) = self.fp_set_color_write_mask {
            unsafe { fp(cmd, first_attachment, masks.len() as u32, masks.as_ptr()); }
        }
    }

    /// Set depth clamp enable dynamically.
    pub fn set_depth_clamp_enable(&self, cmd: vk::CommandBuffer, enable: bool) {
        if let Some(fp) = self.fp_set_depth_clamp_enable {
            unsafe { fp(cmd, if enable { vk::TRUE } else { vk::FALSE }); }
        }
    }

    /// Set line rasterization mode dynamically.
    pub fn set_line_rasterization_mode(&self, cmd: vk::CommandBuffer, mode: vk::LineRasterizationModeEXT) {
        if let Some(fp) = self.fp_set_line_rasterization_mode {
            unsafe { fp(cmd, mode); }
        }
    }

    /// Set line stipple enable dynamically.
    pub fn set_line_stipple_enable(&self, cmd: vk::CommandBuffer, enable: bool) {
        if let Some(fp) = self.fp_set_line_stipple_enable {
            unsafe { fp(cmd, if enable { vk::TRUE } else { vk::FALSE }); }
        }
    }
}

/// Get the list of dynamic states for EDS3.
pub fn get_dynamic_states(caps: &DynamicState3Capabilities) -> Vec<vk::DynamicState> {
    let mut states = Vec::new();

    // Always include core dynamic states
    states.push(vk::DynamicState::VIEWPORT);
    states.push(vk::DynamicState::SCISSOR);
    states.push(vk::DynamicState::LINE_WIDTH);
    states.push(vk::DynamicState::DEPTH_BIAS);
    states.push(vk::DynamicState::BLEND_CONSTANTS);
    states.push(vk::DynamicState::DEPTH_BOUNDS);
    states.push(vk::DynamicState::STENCIL_COMPARE_MASK);
    states.push(vk::DynamicState::STENCIL_WRITE_MASK);
    states.push(vk::DynamicState::STENCIL_REFERENCE);

    // EDS3 states
    if caps.polygon_mode {
        states.push(vk::DynamicState::POLYGON_MODE_EXT);
    }
    if caps.rasterization_samples {
        states.push(vk::DynamicState::RASTERIZATION_SAMPLES_EXT);
    }
    if caps.sample_mask {
        states.push(vk::DynamicState::SAMPLE_MASK_EXT);
    }
    if caps.alpha_to_coverage_enable {
        states.push(vk::DynamicState::ALPHA_TO_COVERAGE_ENABLE_EXT);
    }
    if caps.alpha_to_one_enable {
        states.push(vk::DynamicState::ALPHA_TO_ONE_ENABLE_EXT);
    }
    if caps.logic_op_enable {
        states.push(vk::DynamicState::LOGIC_OP_ENABLE_EXT);
    }
    if caps.color_blend_enable {
        states.push(vk::DynamicState::COLOR_BLEND_ENABLE_EXT);
    }
    if caps.color_blend_equation {
        states.push(vk::DynamicState::COLOR_BLEND_EQUATION_EXT);
    }
    if caps.color_write_mask {
        states.push(vk::DynamicState::COLOR_WRITE_MASK_EXT);
    }
    if caps.depth_clamp_enable {
        states.push(vk::DynamicState::DEPTH_CLAMP_ENABLE_EXT);
    }
    if caps.line_rasterization_mode {
        states.push(vk::DynamicState::LINE_RASTERIZATION_MODE_EXT);
    }
    if caps.line_stipple_enable {
        states.push(vk::DynamicState::LINE_STIPPLE_ENABLE_EXT);
    }

    states
}

/// Blend equation helper.
#[derive(Debug, Clone, Copy)]
pub struct BlendEquation {
    pub src_color: vk::BlendFactor,
    pub dst_color: vk::BlendFactor,
    pub color_op: vk::BlendOp,
    pub src_alpha: vk::BlendFactor,
    pub dst_alpha: vk::BlendFactor,
    pub alpha_op: vk::BlendOp,
}

impl Default for BlendEquation {
    fn default() -> Self {
        Self {
            src_color: vk::BlendFactor::SRC_ALPHA,
            dst_color: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
            color_op: vk::BlendOp::ADD,
            src_alpha: vk::BlendFactor::ONE,
            dst_alpha: vk::BlendFactor::ZERO,
            alpha_op: vk::BlendOp::ADD,
        }
    }
}

impl BlendEquation {
    /// Alpha blending.
    pub fn alpha_blend() -> Self {
        Self::default()
    }

    /// Additive blending.
    pub fn additive() -> Self {
        Self {
            src_color: vk::BlendFactor::ONE,
            dst_color: vk::BlendFactor::ONE,
            color_op: vk::BlendOp::ADD,
            src_alpha: vk::BlendFactor::ONE,
            dst_alpha: vk::BlendFactor::ONE,
            alpha_op: vk::BlendOp::ADD,
        }
    }

    /// Premultiplied alpha blending.
    pub fn premultiplied_alpha() -> Self {
        Self {
            src_color: vk::BlendFactor::ONE,
            dst_color: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
            color_op: vk::BlendOp::ADD,
            src_alpha: vk::BlendFactor::ONE,
            dst_alpha: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
            alpha_op: vk::BlendOp::ADD,
        }
    }

    /// Convert to Vulkan struct.
    pub fn to_vk(&self) -> vk::ColorBlendEquationEXT {
        vk::ColorBlendEquationEXT {
            src_color_blend_factor: self.src_color,
            dst_color_blend_factor: self.dst_color,
            color_blend_op: self.color_op,
            src_alpha_blend_factor: self.src_alpha,
            dst_alpha_blend_factor: self.dst_alpha,
            alpha_blend_op: self.alpha_op,
        }
    }
}
