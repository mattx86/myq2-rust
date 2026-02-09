//! Graphics Pipeline Library for modular pipeline creation
//!
//! VK_EXT_graphics_pipeline_library allows splitting pipeline creation into
//! independent parts that can be compiled separately and linked at draw time:
//! - Vertex Input Interface
//! - Pre-Rasterization Shaders
//! - Fragment Shader
//! - Fragment Output Interface
//!
//! Benefits:
//! - Faster pipeline creation (parallel compilation)
//! - Reduced memory usage (shared library parts)
//! - Runtime shader combination without full recompilation

use ash::vk;
use ash::vk::Handle;
use std::collections::HashMap;
use std::sync::Arc;

/// Pipeline library part types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LibraryPart {
    /// Vertex input state (bindings, attributes).
    VertexInput,
    /// Pre-rasterization shaders (vertex, tessellation, geometry).
    PreRasterization,
    /// Fragment shader and state.
    FragmentShader,
    /// Fragment output state (attachments, blending).
    FragmentOutput,
}

/// Pipeline library capabilities.
#[derive(Debug, Clone)]
pub struct PipelineLibraryCapabilities {
    /// Whether pipeline library is supported.
    pub supported: bool,
    /// Whether optimized linking is supported.
    pub graphics_pipeline_library: bool,
    /// Whether retain link time optimization is supported.
    pub retain_link_time_optimization: bool,
}

impl Default for PipelineLibraryCapabilities {
    fn default() -> Self {
        Self {
            supported: false,
            graphics_pipeline_library: false,
            retain_link_time_optimization: false,
        }
    }
}

/// A compiled pipeline library part.
pub struct PipelineLibraryPart {
    /// Pipeline handle.
    pub pipeline: vk::Pipeline,
    /// Part type.
    pub part_type: LibraryPart,
    /// Hash for caching.
    pub hash: u64,
}

/// Pipeline library manager.
pub struct PipelineLibraryManager {
    /// Capabilities.
    capabilities: PipelineLibraryCapabilities,
    /// Cached vertex input libraries.
    vertex_input_cache: HashMap<u64, Arc<PipelineLibraryPart>>,
    /// Cached pre-rasterization libraries.
    pre_raster_cache: HashMap<u64, Arc<PipelineLibraryPart>>,
    /// Cached fragment shader libraries.
    fragment_shader_cache: HashMap<u64, Arc<PipelineLibraryPart>>,
    /// Cached fragment output libraries.
    fragment_output_cache: HashMap<u64, Arc<PipelineLibraryPart>>,
    /// Linked pipeline cache.
    linked_cache: HashMap<u64, vk::Pipeline>,
}

impl PipelineLibraryManager {
    /// Query pipeline library capabilities.
    pub fn query_capabilities(ctx: &super::context::VulkanContext) -> PipelineLibraryCapabilities {
        let mut library_features = vk::PhysicalDeviceGraphicsPipelineLibraryFeaturesEXT::default();
        let mut features2 = vk::PhysicalDeviceFeatures2::default()
            .push_next(&mut library_features);

        unsafe {
            ctx.instance.get_physical_device_features2(ctx.physical_device, &mut features2);
        }

        PipelineLibraryCapabilities {
            supported: library_features.graphics_pipeline_library == vk::TRUE,
            graphics_pipeline_library: library_features.graphics_pipeline_library == vk::TRUE,
            retain_link_time_optimization: false, // Would check additional properties
        }
    }

    /// Create a new pipeline library manager.
    pub fn new(ctx: &super::context::VulkanContext) -> Self {
        let capabilities = Self::query_capabilities(ctx);

        Self {
            capabilities,
            vertex_input_cache: HashMap::new(),
            pre_raster_cache: HashMap::new(),
            fragment_shader_cache: HashMap::new(),
            fragment_output_cache: HashMap::new(),
            linked_cache: HashMap::new(),
        }
    }

    /// Check if pipeline library is supported.
    pub fn is_supported(&self) -> bool {
        self.capabilities.supported
    }

    /// Get capabilities.
    pub fn capabilities(&self) -> &PipelineLibraryCapabilities {
        &self.capabilities
    }

    /// Create a vertex input library.
    pub fn create_vertex_input_library(
        &mut self,
        ctx: &super::context::VulkanContext,
        vertex_bindings: &[vk::VertexInputBindingDescription],
        vertex_attributes: &[vk::VertexInputAttributeDescription],
    ) -> Result<Arc<PipelineLibraryPart>, String> {
        // Hash the vertex input state
        let hash = Self::hash_vertex_input(vertex_bindings, vertex_attributes);

        // Check cache
        if let Some(cached) = self.vertex_input_cache.get(&hash) {
            return Ok(cached.clone());
        }

        // Create vertex input state
        let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(vertex_bindings)
            .vertex_attribute_descriptions(vertex_attributes);

        let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
            .primitive_restart_enable(false);

        // Library creation info
        let mut library_info = vk::GraphicsPipelineLibraryCreateInfoEXT::default()
            .flags(vk::GraphicsPipelineLibraryFlagsEXT::VERTEX_INPUT_INTERFACE);

        let create_info = vk::GraphicsPipelineCreateInfo::default()
            .flags(vk::PipelineCreateFlags::LIBRARY_KHR)
            .vertex_input_state(&vertex_input_state)
            .input_assembly_state(&input_assembly_state)
            .push_next(&mut library_info);

        let pipeline = unsafe {
            ctx.device.create_graphics_pipelines(
                vk::PipelineCache::null(),
                &[create_info],
                None,
            ).map_err(|e| format!("Failed to create vertex input library: {:?}", e.1))?[0]
        };

        let part = Arc::new(PipelineLibraryPart {
            pipeline,
            part_type: LibraryPart::VertexInput,
            hash,
        });

        self.vertex_input_cache.insert(hash, part.clone());
        Ok(part)
    }

    /// Create a pre-rasterization library.
    pub fn create_pre_rasterization_library(
        &mut self,
        ctx: &super::context::VulkanContext,
        layout: vk::PipelineLayout,
        vertex_shader: vk::ShaderModule,
        viewport_state: &vk::PipelineViewportStateCreateInfo,
        rasterization_state: &vk::PipelineRasterizationStateCreateInfo,
    ) -> Result<Arc<PipelineLibraryPart>, String> {
        // Hash the pre-rasterization state
        let hash = Self::hash_shader_module(vertex_shader);

        // Check cache
        if let Some(cached) = self.pre_raster_cache.get(&hash) {
            return Ok(cached.clone());
        }

        let stage = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vertex_shader)
            .name(c"main");

        let stages = [stage];

        let mut library_info = vk::GraphicsPipelineLibraryCreateInfoEXT::default()
            .flags(vk::GraphicsPipelineLibraryFlagsEXT::PRE_RASTERIZATION_SHADERS);

        let mut dynamic_state_enables = [
            vk::DynamicState::VIEWPORT,
            vk::DynamicState::SCISSOR,
        ];
        let dynamic_state = vk::PipelineDynamicStateCreateInfo::default()
            .dynamic_states(&dynamic_state_enables);

        let create_info = vk::GraphicsPipelineCreateInfo::default()
            .flags(vk::PipelineCreateFlags::LIBRARY_KHR)
            .stages(&stages)
            .viewport_state(viewport_state)
            .rasterization_state(rasterization_state)
            .dynamic_state(&dynamic_state)
            .layout(layout)
            .push_next(&mut library_info);

        let pipeline = unsafe {
            ctx.device.create_graphics_pipelines(
                vk::PipelineCache::null(),
                &[create_info],
                None,
            ).map_err(|e| format!("Failed to create pre-rasterization library: {:?}", e.1))?[0]
        };

        let part = Arc::new(PipelineLibraryPart {
            pipeline,
            part_type: LibraryPart::PreRasterization,
            hash,
        });

        self.pre_raster_cache.insert(hash, part.clone());
        Ok(part)
    }

    /// Create a fragment shader library.
    pub fn create_fragment_shader_library(
        &mut self,
        ctx: &super::context::VulkanContext,
        layout: vk::PipelineLayout,
        fragment_shader: vk::ShaderModule,
        multisample_state: &vk::PipelineMultisampleStateCreateInfo,
        depth_stencil_state: Option<&vk::PipelineDepthStencilStateCreateInfo>,
    ) -> Result<Arc<PipelineLibraryPart>, String> {
        let hash = Self::hash_shader_module(fragment_shader);

        // Check cache
        if let Some(cached) = self.fragment_shader_cache.get(&hash) {
            return Ok(cached.clone());
        }

        let stage = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(fragment_shader)
            .name(c"main");

        let stages = [stage];

        let mut library_info = vk::GraphicsPipelineLibraryCreateInfoEXT::default()
            .flags(vk::GraphicsPipelineLibraryFlagsEXT::FRAGMENT_SHADER);

        let default_depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default();
        let depth_stencil = depth_stencil_state.unwrap_or(&default_depth_stencil);

        let create_info = vk::GraphicsPipelineCreateInfo::default()
            .flags(vk::PipelineCreateFlags::LIBRARY_KHR)
            .stages(&stages)
            .multisample_state(multisample_state)
            .depth_stencil_state(depth_stencil)
            .layout(layout)
            .push_next(&mut library_info);

        let pipeline = unsafe {
            ctx.device.create_graphics_pipelines(
                vk::PipelineCache::null(),
                &[create_info],
                None,
            ).map_err(|e| format!("Failed to create fragment shader library: {:?}", e.1))?[0]
        };

        let part = Arc::new(PipelineLibraryPart {
            pipeline,
            part_type: LibraryPart::FragmentShader,
            hash,
        });

        self.fragment_shader_cache.insert(hash, part.clone());
        Ok(part)
    }

    /// Create a fragment output library.
    pub fn create_fragment_output_library(
        &mut self,
        ctx: &super::context::VulkanContext,
        color_blend_state: &vk::PipelineColorBlendStateCreateInfo,
        format: vk::Format,
        samples: vk::SampleCountFlags,
    ) -> Result<Arc<PipelineLibraryPart>, String> {
        let hash = Self::hash_output_state(format, samples);

        // Check cache
        if let Some(cached) = self.fragment_output_cache.get(&hash) {
            return Ok(cached.clone());
        }

        let mut library_info = vk::GraphicsPipelineLibraryCreateInfoEXT::default()
            .flags(vk::GraphicsPipelineLibraryFlagsEXT::FRAGMENT_OUTPUT_INTERFACE);

        // For dynamic rendering
        let color_formats = [format];
        let mut rendering_info = vk::PipelineRenderingCreateInfo::default()
            .color_attachment_formats(&color_formats);

        let multisample_state = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(samples);

        let create_info = vk::GraphicsPipelineCreateInfo::default()
            .flags(vk::PipelineCreateFlags::LIBRARY_KHR)
            .color_blend_state(color_blend_state)
            .multisample_state(&multisample_state)
            .push_next(&mut library_info)
            .push_next(&mut rendering_info);

        let pipeline = unsafe {
            ctx.device.create_graphics_pipelines(
                vk::PipelineCache::null(),
                &[create_info],
                None,
            ).map_err(|e| format!("Failed to create fragment output library: {:?}", e.1))?[0]
        };

        let part = Arc::new(PipelineLibraryPart {
            pipeline,
            part_type: LibraryPart::FragmentOutput,
            hash,
        });

        self.fragment_output_cache.insert(hash, part.clone());
        Ok(part)
    }

    /// Link library parts into a complete pipeline.
    pub fn link_pipeline(
        &mut self,
        ctx: &super::context::VulkanContext,
        layout: vk::PipelineLayout,
        parts: &[&PipelineLibraryPart],
    ) -> Result<vk::Pipeline, String> {
        // Compute combined hash
        let mut hash = 0u64;
        for part in parts {
            hash = hash.wrapping_mul(31).wrapping_add(part.hash);
        }

        // Check cache
        if let Some(&cached) = self.linked_cache.get(&hash) {
            return Ok(cached);
        }

        let library_pipelines: Vec<vk::Pipeline> = parts.iter().map(|p| p.pipeline).collect();

        let mut library_info = vk::PipelineLibraryCreateInfoKHR::default()
            .libraries(&library_pipelines);

        let create_info = vk::GraphicsPipelineCreateInfo::default()
            .flags(vk::PipelineCreateFlags::LINK_TIME_OPTIMIZATION_EXT)
            .layout(layout)
            .push_next(&mut library_info);

        let pipeline = unsafe {
            ctx.device.create_graphics_pipelines(
                vk::PipelineCache::null(),
                &[create_info],
                None,
            ).map_err(|e| format!("Failed to link pipeline: {:?}", e.1))?[0]
        };

        self.linked_cache.insert(hash, pipeline);
        Ok(pipeline)
    }

    fn hash_vertex_input(
        bindings: &[vk::VertexInputBindingDescription],
        attributes: &[vk::VertexInputAttributeDescription],
    ) -> u64 {
        let mut hash = 0u64;
        for binding in bindings {
            hash = hash.wrapping_mul(31).wrapping_add(binding.binding as u64);
            hash = hash.wrapping_mul(31).wrapping_add(binding.stride as u64);
        }
        for attr in attributes {
            hash = hash.wrapping_mul(31).wrapping_add(attr.location as u64);
            hash = hash.wrapping_mul(31).wrapping_add(attr.format.as_raw() as u64);
        }
        hash
    }

    fn hash_shader_module(module: vk::ShaderModule) -> u64 {
        module.as_raw()
    }

    fn hash_output_state(format: vk::Format, samples: vk::SampleCountFlags) -> u64 {
        (format.as_raw() as u64).wrapping_mul(31).wrapping_add(samples.as_raw() as u64)
    }

    /// Clear all caches.
    pub fn clear_caches(&mut self, ctx: &super::context::VulkanContext) {
        unsafe {
            for part in self.vertex_input_cache.values() {
                ctx.device.destroy_pipeline(part.pipeline, None);
            }
            for part in self.pre_raster_cache.values() {
                ctx.device.destroy_pipeline(part.pipeline, None);
            }
            for part in self.fragment_shader_cache.values() {
                ctx.device.destroy_pipeline(part.pipeline, None);
            }
            for part in self.fragment_output_cache.values() {
                ctx.device.destroy_pipeline(part.pipeline, None);
            }
            for &pipeline in self.linked_cache.values() {
                ctx.device.destroy_pipeline(pipeline, None);
            }
        }

        self.vertex_input_cache.clear();
        self.pre_raster_cache.clear();
        self.fragment_shader_cache.clear();
        self.fragment_output_cache.clear();
        self.linked_cache.clear();
    }

    /// Destroy the manager.
    pub fn destroy(&mut self, ctx: &super::context::VulkanContext) {
        self.clear_caches(ctx);
    }
}
