//! Pipeline Robustness (VK_EXT_pipeline_robustness)
//!
//! Control robustness behavior per-pipeline:
//! - Handle out-of-bounds buffer access
//! - Handle out-of-bounds image access
//! - Prevent GPU crashes from shader bugs
//! - Trade-off between safety and performance

use ash::vk;

/// Pipeline robustness capabilities.
#[derive(Debug, Clone, Default)]
pub struct PipelineRobustnessCapabilities {
    /// Whether pipeline robustness is supported.
    pub supported: bool,
    /// Default robustness for storage buffers.
    pub default_storage_buffers: RobustnessLevel,
    /// Default robustness for uniform buffers.
    pub default_uniform_buffers: RobustnessLevel,
    /// Default robustness for vertex inputs.
    pub default_vertex_inputs: RobustnessLevel,
    /// Default robustness for images.
    pub default_images: RobustnessLevel,
}

/// Query pipeline robustness capabilities.
pub fn query_capabilities(ctx: &super::context::VulkanContext) -> PipelineRobustnessCapabilities {
    let mut pr_features = vk::PhysicalDevicePipelineRobustnessFeaturesEXT::default();
    let mut features2 = vk::PhysicalDeviceFeatures2::default()
        .push_next(&mut pr_features);

    unsafe {
        ctx.instance.get_physical_device_features2(ctx.physical_device, &mut features2);
    }

    let _ = features2;
    let supported = pr_features.pipeline_robustness == vk::TRUE;

    if !supported {
        return PipelineRobustnessCapabilities::default();
    }

    // Query properties for default behaviors
    let mut pr_props = vk::PhysicalDevicePipelineRobustnessPropertiesEXT::default();
    let mut props2 = vk::PhysicalDeviceProperties2::default()
        .push_next(&mut pr_props);

    unsafe {
        ctx.instance.get_physical_device_properties2(ctx.physical_device, &mut props2);
    }

    PipelineRobustnessCapabilities {
        supported,
        default_storage_buffers: RobustnessLevel::from_vk(pr_props.default_robustness_storage_buffers),
        default_uniform_buffers: RobustnessLevel::from_vk(pr_props.default_robustness_uniform_buffers),
        default_vertex_inputs: RobustnessLevel::from_vk(pr_props.default_robustness_vertex_inputs),
        default_images: RobustnessLevel::from_vk_image(pr_props.default_robustness_images),
    }
}

/// Robustness level for resource access.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RobustnessLevel {
    /// Device default behavior.
    #[default]
    DeviceDefault,
    /// Disabled - undefined behavior on out-of-bounds (fastest).
    Disabled,
    /// Robust buffer access - clamp to valid range.
    RobustBufferAccess,
    /// Robust buffer access 2 - return zero on out-of-bounds.
    RobustBufferAccess2,
}

impl RobustnessLevel {
    /// Convert from Vulkan buffer robustness behavior.
    pub fn from_vk(behavior: vk::PipelineRobustnessBufferBehaviorEXT) -> Self {
        match behavior {
            vk::PipelineRobustnessBufferBehaviorEXT::DEVICE_DEFAULT => RobustnessLevel::DeviceDefault,
            vk::PipelineRobustnessBufferBehaviorEXT::DISABLED => RobustnessLevel::Disabled,
            vk::PipelineRobustnessBufferBehaviorEXT::ROBUST_BUFFER_ACCESS => RobustnessLevel::RobustBufferAccess,
            vk::PipelineRobustnessBufferBehaviorEXT::ROBUST_BUFFER_ACCESS_2 => RobustnessLevel::RobustBufferAccess2,
            _ => RobustnessLevel::DeviceDefault,
        }
    }

    /// Convert from Vulkan image robustness behavior.
    pub fn from_vk_image(behavior: vk::PipelineRobustnessImageBehaviorEXT) -> Self {
        match behavior {
            vk::PipelineRobustnessImageBehaviorEXT::DEVICE_DEFAULT => RobustnessLevel::DeviceDefault,
            vk::PipelineRobustnessImageBehaviorEXT::DISABLED => RobustnessLevel::Disabled,
            vk::PipelineRobustnessImageBehaviorEXT::ROBUST_IMAGE_ACCESS => RobustnessLevel::RobustBufferAccess,
            vk::PipelineRobustnessImageBehaviorEXT::ROBUST_IMAGE_ACCESS_2 => RobustnessLevel::RobustBufferAccess2,
            _ => RobustnessLevel::DeviceDefault,
        }
    }

    /// Convert to Vulkan buffer robustness behavior.
    pub fn to_vk_buffer(&self) -> vk::PipelineRobustnessBufferBehaviorEXT {
        match self {
            RobustnessLevel::DeviceDefault => vk::PipelineRobustnessBufferBehaviorEXT::DEVICE_DEFAULT,
            RobustnessLevel::Disabled => vk::PipelineRobustnessBufferBehaviorEXT::DISABLED,
            RobustnessLevel::RobustBufferAccess => vk::PipelineRobustnessBufferBehaviorEXT::ROBUST_BUFFER_ACCESS,
            RobustnessLevel::RobustBufferAccess2 => vk::PipelineRobustnessBufferBehaviorEXT::ROBUST_BUFFER_ACCESS_2,
        }
    }

    /// Convert to Vulkan image robustness behavior.
    pub fn to_vk_image(&self) -> vk::PipelineRobustnessImageBehaviorEXT {
        match self {
            RobustnessLevel::DeviceDefault => vk::PipelineRobustnessImageBehaviorEXT::DEVICE_DEFAULT,
            RobustnessLevel::Disabled => vk::PipelineRobustnessImageBehaviorEXT::DISABLED,
            RobustnessLevel::RobustBufferAccess | RobustnessLevel::RobustBufferAccess2 => {
                vk::PipelineRobustnessImageBehaviorEXT::ROBUST_IMAGE_ACCESS_2
            }
        }
    }
}

/// Pipeline robustness configuration.
#[derive(Debug, Clone)]
pub struct PipelineRobustnessConfig {
    /// Robustness for storage buffers.
    pub storage_buffers: RobustnessLevel,
    /// Robustness for uniform buffers.
    pub uniform_buffers: RobustnessLevel,
    /// Robustness for vertex inputs.
    pub vertex_inputs: RobustnessLevel,
    /// Robustness for images.
    pub images: RobustnessLevel,
}

impl Default for PipelineRobustnessConfig {
    fn default() -> Self {
        Self {
            storage_buffers: RobustnessLevel::DeviceDefault,
            uniform_buffers: RobustnessLevel::DeviceDefault,
            vertex_inputs: RobustnessLevel::DeviceDefault,
            images: RobustnessLevel::DeviceDefault,
        }
    }
}

impl PipelineRobustnessConfig {
    /// Create config with full robustness (safest, slowest).
    pub fn full_robustness() -> Self {
        Self {
            storage_buffers: RobustnessLevel::RobustBufferAccess2,
            uniform_buffers: RobustnessLevel::RobustBufferAccess2,
            vertex_inputs: RobustnessLevel::RobustBufferAccess2,
            images: RobustnessLevel::RobustBufferAccess2,
        }
    }

    /// Create config with no robustness (fastest, may crash).
    pub fn no_robustness() -> Self {
        Self {
            storage_buffers: RobustnessLevel::Disabled,
            uniform_buffers: RobustnessLevel::Disabled,
            vertex_inputs: RobustnessLevel::Disabled,
            images: RobustnessLevel::Disabled,
        }
    }

    /// Create config for development (robust storage, fast other).
    pub fn development() -> Self {
        Self {
            storage_buffers: RobustnessLevel::RobustBufferAccess2,
            uniform_buffers: RobustnessLevel::RobustBufferAccess,
            vertex_inputs: RobustnessLevel::DeviceDefault,
            images: RobustnessLevel::RobustBufferAccess2,
        }
    }

    /// Create config for release (minimal robustness).
    pub fn release() -> Self {
        Self {
            storage_buffers: RobustnessLevel::RobustBufferAccess,
            uniform_buffers: RobustnessLevel::DeviceDefault,
            vertex_inputs: RobustnessLevel::DeviceDefault,
            images: RobustnessLevel::DeviceDefault,
        }
    }
}

/// Create pipeline robustness create info.
pub fn create_robustness_info(config: &PipelineRobustnessConfig) -> vk::PipelineRobustnessCreateInfoEXT<'static> {
    vk::PipelineRobustnessCreateInfoEXT::default()
        .storage_buffers(config.storage_buffers.to_vk_buffer())
        .uniform_buffers(config.uniform_buffers.to_vk_buffer())
        .vertex_inputs(config.vertex_inputs.to_vk_buffer())
        .images(config.images.to_vk_image())
}

/// Pipeline robustness manager.
pub struct PipelineRobustnessManager {
    capabilities: PipelineRobustnessCapabilities,
    default_config: PipelineRobustnessConfig,
}

impl PipelineRobustnessManager {
    /// Create new manager.
    pub fn new(ctx: &super::context::VulkanContext) -> Self {
        let capabilities = query_capabilities(ctx);

        // Choose default config based on build mode
        #[cfg(debug_assertions)]
        let default_config = PipelineRobustnessConfig::development();

        #[cfg(not(debug_assertions))]
        let default_config = PipelineRobustnessConfig::release();

        Self {
            capabilities,
            default_config,
        }
    }

    /// Check if pipeline robustness is supported.
    pub fn is_supported(&self) -> bool {
        self.capabilities.supported
    }

    /// Get capabilities.
    pub fn capabilities(&self) -> &PipelineRobustnessCapabilities {
        &self.capabilities
    }

    /// Set default configuration.
    pub fn set_default_config(&mut self, config: PipelineRobustnessConfig) {
        self.default_config = config;
    }

    /// Get default configuration.
    pub fn default_config(&self) -> &PipelineRobustnessConfig {
        &self.default_config
    }

    /// Create robustness info with default config.
    pub fn create_default_info(&self) -> vk::PipelineRobustnessCreateInfoEXT<'static> {
        create_robustness_info(&self.default_config)
    }

    /// Create robustness info with custom config.
    pub fn create_info(&self, config: &PipelineRobustnessConfig) -> vk::PipelineRobustnessCreateInfoEXT<'static> {
        create_robustness_info(config)
    }
}

/// Robustness presets for different pipeline types.
pub mod presets {
    use super::*;

    /// Config for compute pipelines (often needs robust storage access).
    pub fn compute_pipeline() -> PipelineRobustnessConfig {
        PipelineRobustnessConfig {
            storage_buffers: RobustnessLevel::RobustBufferAccess2,
            uniform_buffers: RobustnessLevel::RobustBufferAccess,
            vertex_inputs: RobustnessLevel::Disabled,
            images: RobustnessLevel::RobustBufferAccess2,
        }
    }

    /// Config for ray tracing pipelines.
    pub fn raytracing_pipeline() -> PipelineRobustnessConfig {
        PipelineRobustnessConfig {
            storage_buffers: RobustnessLevel::RobustBufferAccess2,
            uniform_buffers: RobustnessLevel::RobustBufferAccess,
            vertex_inputs: RobustnessLevel::Disabled,
            images: RobustnessLevel::RobustBufferAccess2,
        }
    }

    /// Config for mesh shader pipelines.
    pub fn mesh_pipeline() -> PipelineRobustnessConfig {
        PipelineRobustnessConfig {
            storage_buffers: RobustnessLevel::RobustBufferAccess2,
            uniform_buffers: RobustnessLevel::RobustBufferAccess,
            vertex_inputs: RobustnessLevel::Disabled, // Mesh shaders don't use vertex input
            images: RobustnessLevel::RobustBufferAccess,
        }
    }

    /// Config for static geometry pipelines (known-good vertex data).
    pub fn static_geometry() -> PipelineRobustnessConfig {
        PipelineRobustnessConfig {
            storage_buffers: RobustnessLevel::DeviceDefault,
            uniform_buffers: RobustnessLevel::DeviceDefault,
            vertex_inputs: RobustnessLevel::Disabled,
            images: RobustnessLevel::DeviceDefault,
        }
    }

    /// Config for dynamic/user-provided content.
    pub fn dynamic_content() -> PipelineRobustnessConfig {
        PipelineRobustnessConfig::full_robustness()
    }
}
