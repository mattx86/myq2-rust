//! Shader Objects for pipeline-less rendering
//!
//! VK_EXT_shader_object allows using shaders directly without pipeline objects:
//! - No upfront pipeline compilation
//! - Dynamic shader binding
//! - Ultimate flexibility for shader combinations
//! - Ideal for tools and editors with frequent shader changes

use ash::vk;
use std::collections::HashMap;
use std::ffi::CString;

/// Shader object capabilities.
#[derive(Debug, Clone, Default)]
pub struct ShaderObjectCapabilities {
    /// Whether shader objects are supported.
    pub supported: bool,
    /// Whether tessellation shaders are supported.
    pub tessellation: bool,
    /// Whether geometry shaders are supported.
    pub geometry: bool,
    /// Whether mesh shaders are supported.
    pub mesh: bool,
}

/// Query shader object capabilities.
pub fn query_capabilities(ctx: &super::context::VulkanContext) -> ShaderObjectCapabilities {
    // Query base features separately to avoid borrow conflicts
    let base_features = unsafe {
        ctx.instance.get_physical_device_features(ctx.physical_device)
    };

    let tessellation = base_features.tessellation_shader == vk::TRUE;
    let geometry = base_features.geometry_shader == vk::TRUE;

    // Query shader object extension features
    let mut shader_object_features = vk::PhysicalDeviceShaderObjectFeaturesEXT::default();
    let mut features2 = vk::PhysicalDeviceFeatures2::default()
        .push_next(&mut shader_object_features);

    unsafe {
        ctx.instance.get_physical_device_features2(ctx.physical_device, &mut features2);
    }

    // Now we can access shader_object_features after features2 is done being used
    let _ = features2;
    let supported = shader_object_features.shader_object == vk::TRUE;

    ShaderObjectCapabilities {
        supported,
        tessellation,
        geometry,
        mesh: false, // Would check mesh shader features
    }
}

/// Shader stage for shader objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderStage {
    Vertex,
    TessellationControl,
    TessellationEvaluation,
    Geometry,
    Fragment,
    Compute,
    Task,
    Mesh,
}

impl ShaderStage {
    /// Convert to Vulkan shader stage flags.
    pub fn to_vk(&self) -> vk::ShaderStageFlags {
        match self {
            ShaderStage::Vertex => vk::ShaderStageFlags::VERTEX,
            ShaderStage::TessellationControl => vk::ShaderStageFlags::TESSELLATION_CONTROL,
            ShaderStage::TessellationEvaluation => vk::ShaderStageFlags::TESSELLATION_EVALUATION,
            ShaderStage::Geometry => vk::ShaderStageFlags::GEOMETRY,
            ShaderStage::Fragment => vk::ShaderStageFlags::FRAGMENT,
            ShaderStage::Compute => vk::ShaderStageFlags::COMPUTE,
            ShaderStage::Task => vk::ShaderStageFlags::TASK_EXT,
            ShaderStage::Mesh => vk::ShaderStageFlags::MESH_EXT,
        }
    }

    /// Get the next stage in the pipeline.
    pub fn next_stage(&self) -> Option<vk::ShaderStageFlags> {
        match self {
            ShaderStage::Vertex => Some(vk::ShaderStageFlags::TESSELLATION_CONTROL | vk::ShaderStageFlags::GEOMETRY | vk::ShaderStageFlags::FRAGMENT),
            ShaderStage::TessellationControl => Some(vk::ShaderStageFlags::TESSELLATION_EVALUATION),
            ShaderStage::TessellationEvaluation => Some(vk::ShaderStageFlags::GEOMETRY | vk::ShaderStageFlags::FRAGMENT),
            ShaderStage::Geometry => Some(vk::ShaderStageFlags::FRAGMENT),
            ShaderStage::Fragment => None,
            ShaderStage::Compute => None,
            ShaderStage::Task => Some(vk::ShaderStageFlags::MESH_EXT),
            ShaderStage::Mesh => Some(vk::ShaderStageFlags::FRAGMENT),
        }
    }
}

/// Shader object handle.
pub struct ShaderObject {
    /// Shader object handle.
    handle: vk::ShaderEXT,
    /// Shader stage.
    stage: ShaderStage,
    /// Entry point name.
    entry_point: CString,
    /// Shader name for debugging.
    name: String,
}

impl ShaderObject {
    /// Get the shader handle.
    pub fn handle(&self) -> vk::ShaderEXT {
        self.handle
    }

    /// Get the shader stage.
    pub fn stage(&self) -> ShaderStage {
        self.stage
    }

    /// Get the entry point.
    pub fn entry_point(&self) -> &CString {
        &self.entry_point
    }
}

/// Shader object manager.
pub struct ShaderObjectManager {
    /// Function pointer for creating shader objects.
    fp_create_shaders: Option<vk::PFN_vkCreateShadersEXT>,
    /// Function pointer for destroying shader objects.
    fp_destroy_shader: Option<vk::PFN_vkDestroyShaderEXT>,
    /// Function pointer for binding shaders.
    fp_cmd_bind_shaders: Option<vk::PFN_vkCmdBindShadersEXT>,
    /// Cached shader objects.
    shaders: HashMap<String, ShaderObject>,
    /// Capabilities.
    caps: ShaderObjectCapabilities,
}

impl ShaderObjectManager {
    /// Create a new shader object manager.
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
            fp_create_shaders: get_fp!("vkCreateShadersEXT"),
            fp_destroy_shader: get_fp!("vkDestroyShaderEXT"),
            fp_cmd_bind_shaders: get_fp!("vkCmdBindShadersEXT"),
            shaders: HashMap::new(),
            caps,
        }
    }

    /// Check if shader objects are supported.
    pub fn is_supported(&self) -> bool {
        self.caps.supported && self.fp_create_shaders.is_some()
    }

    /// Get capabilities.
    pub fn capabilities(&self) -> &ShaderObjectCapabilities {
        &self.caps
    }

    /// Create a shader object from SPIR-V.
    pub fn create_shader(
        &mut self,
        ctx: &super::context::VulkanContext,
        name: &str,
        stage: ShaderStage,
        spirv: &[u8],
        entry_point: &str,
        descriptor_set_layouts: &[vk::DescriptorSetLayout],
        push_constant_ranges: &[vk::PushConstantRange],
    ) -> Result<vk::ShaderEXT, String> {
        let fp = self.fp_create_shaders.ok_or("Shader objects not supported")?;

        let entry_point_cstr = CString::new(entry_point)
            .map_err(|_| "Invalid entry point name")?;

        let code_size = spirv.len();

        let next_stage = stage.next_stage().unwrap_or(vk::ShaderStageFlags::empty());

        let create_info = vk::ShaderCreateInfoEXT::default()
            .flags(vk::ShaderCreateFlagsEXT::LINK_STAGE)
            .stage(stage.to_vk())
            .next_stage(next_stage)
            .code_type(vk::ShaderCodeTypeEXT::SPIRV)
            .code(spirv)
            .name(&entry_point_cstr)
            .set_layouts(descriptor_set_layouts)
            .push_constant_ranges(push_constant_ranges);

        let create_infos = [create_info];
        let mut shaders = [vk::ShaderEXT::null()];

        let result = unsafe {
            fp(
                ctx.device.handle(),
                1,
                create_infos.as_ptr(),
                std::ptr::null(),
                shaders.as_mut_ptr(),
            )
        };

        if result != vk::Result::SUCCESS {
            return Err(format!("Failed to create shader object: {:?}", result));
        }

        let shader = ShaderObject {
            handle: shaders[0],
            stage,
            entry_point: entry_point_cstr,
            name: name.to_string(),
        };

        let handle = shader.handle;
        self.shaders.insert(name.to_string(), shader);

        Ok(handle)
    }

    /// Create linked shader objects (vertex + fragment).
    pub fn create_linked_shaders(
        &mut self,
        ctx: &super::context::VulkanContext,
        vertex_name: &str,
        vertex_spirv: &[u8],
        fragment_name: &str,
        fragment_spirv: &[u8],
        descriptor_set_layouts: &[vk::DescriptorSetLayout],
        push_constant_ranges: &[vk::PushConstantRange],
    ) -> Result<(vk::ShaderEXT, vk::ShaderEXT), String> {
        let fp = self.fp_create_shaders.ok_or("Shader objects not supported")?;

        let vertex_entry = CString::new("main").unwrap();
        let fragment_entry = CString::new("main").unwrap();

        let vertex_info = vk::ShaderCreateInfoEXT::default()
            .flags(vk::ShaderCreateFlagsEXT::LINK_STAGE)
            .stage(vk::ShaderStageFlags::VERTEX)
            .next_stage(vk::ShaderStageFlags::FRAGMENT)
            .code_type(vk::ShaderCodeTypeEXT::SPIRV)
            .code(vertex_spirv)
            .name(&vertex_entry)
            .set_layouts(descriptor_set_layouts)
            .push_constant_ranges(push_constant_ranges);

        let fragment_info = vk::ShaderCreateInfoEXT::default()
            .flags(vk::ShaderCreateFlagsEXT::LINK_STAGE)
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .next_stage(vk::ShaderStageFlags::empty())
            .code_type(vk::ShaderCodeTypeEXT::SPIRV)
            .code(fragment_spirv)
            .name(&fragment_entry)
            .set_layouts(descriptor_set_layouts)
            .push_constant_ranges(push_constant_ranges);

        let create_infos = [vertex_info, fragment_info];
        let mut shaders = [vk::ShaderEXT::null(); 2];

        let result = unsafe {
            fp(
                ctx.device.handle(),
                2,
                create_infos.as_ptr(),
                std::ptr::null(),
                shaders.as_mut_ptr(),
            )
        };

        if result != vk::Result::SUCCESS {
            return Err(format!("Failed to create linked shaders: {:?}", result));
        }

        // Store the shaders
        self.shaders.insert(vertex_name.to_string(), ShaderObject {
            handle: shaders[0],
            stage: ShaderStage::Vertex,
            entry_point: vertex_entry,
            name: vertex_name.to_string(),
        });

        self.shaders.insert(fragment_name.to_string(), ShaderObject {
            handle: shaders[1],
            stage: ShaderStage::Fragment,
            entry_point: fragment_entry,
            name: fragment_name.to_string(),
        });

        Ok((shaders[0], shaders[1]))
    }

    /// Get a shader by name.
    pub fn get_shader(&self, name: &str) -> Option<vk::ShaderEXT> {
        self.shaders.get(name).map(|s| s.handle)
    }

    /// Bind shaders to a command buffer.
    pub fn bind_shaders(
        &self,
        cmd: vk::CommandBuffer,
        stages: &[vk::ShaderStageFlags],
        shaders: &[vk::ShaderEXT],
    ) {
        if let Some(fp) = self.fp_cmd_bind_shaders {
            unsafe {
                fp(cmd, stages.len() as u32, stages.as_ptr(), shaders.as_ptr());
            }
        }
    }

    /// Bind vertex and fragment shaders.
    pub fn bind_graphics_shaders(
        &self,
        cmd: vk::CommandBuffer,
        vertex: vk::ShaderEXT,
        fragment: vk::ShaderEXT,
    ) {
        let stages = [vk::ShaderStageFlags::VERTEX, vk::ShaderStageFlags::FRAGMENT];
        let shaders = [vertex, fragment];
        self.bind_shaders(cmd, &stages, &shaders);
    }

    /// Unbind shaders (set to null).
    pub fn unbind_shaders(&self, cmd: vk::CommandBuffer, stages: &[vk::ShaderStageFlags]) {
        if let Some(fp) = self.fp_cmd_bind_shaders {
            let null_shaders: Vec<vk::ShaderEXT> = vec![vk::ShaderEXT::null(); stages.len()];
            unsafe {
                fp(cmd, stages.len() as u32, stages.as_ptr(), null_shaders.as_ptr());
            }
        }
    }

    /// Destroy a shader by name.
    pub fn destroy_shader(&mut self, ctx: &super::context::VulkanContext, name: &str) {
        if let Some(shader) = self.shaders.remove(name) {
            if let Some(fp) = self.fp_destroy_shader {
                unsafe {
                    fp(ctx.device.handle(), shader.handle, std::ptr::null());
                }
            }
        }
    }

    /// Destroy all shaders.
    pub fn destroy_all(&mut self, ctx: &super::context::VulkanContext) {
        if let Some(fp) = self.fp_destroy_shader {
            for (_, shader) in self.shaders.drain() {
                unsafe {
                    fp(ctx.device.handle(), shader.handle, std::ptr::null());
                }
            }
        }
    }
}

/// Helper to set required dynamic state for shader objects.
pub fn set_default_dynamic_state(
    ctx: &super::context::VulkanContext,
    cmd: vk::CommandBuffer,
    viewport: vk::Viewport,
    scissor: vk::Rect2D,
) {
    unsafe {
        // Required dynamic state for shader objects
        ctx.device.cmd_set_viewport(cmd, 0, &[viewport]);
        ctx.device.cmd_set_scissor(cmd, 0, &[scissor]);

        // Vulkan 1.3 dynamic state
        ctx.device.cmd_set_cull_mode(cmd, vk::CullModeFlags::BACK);
        ctx.device.cmd_set_front_face(cmd, vk::FrontFace::COUNTER_CLOCKWISE);
        ctx.device.cmd_set_depth_test_enable(cmd, true);
        ctx.device.cmd_set_depth_write_enable(cmd, true);
        ctx.device.cmd_set_depth_compare_op(cmd, vk::CompareOp::LESS_OR_EQUAL);
        ctx.device.cmd_set_depth_bounds_test_enable(cmd, false);
        ctx.device.cmd_set_stencil_test_enable(cmd, false);
        ctx.device.cmd_set_primitive_topology(cmd, vk::PrimitiveTopology::TRIANGLE_LIST);
        ctx.device.cmd_set_primitive_restart_enable(cmd, false);
        ctx.device.cmd_set_rasterizer_discard_enable(cmd, false);
        ctx.device.cmd_set_depth_bias_enable(cmd, false);
    }
}
