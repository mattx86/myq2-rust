//! Depth Bias Control (VK_EXT_depth_bias_control)
//!
//! Enhanced depth bias for shadow mapping:
//! - Per-primitive depth bias representation
//! - Exact depth bias specification
//! - Better shadow acne prevention
//! - Reduced peter-panning artifacts

use ash::vk;

/// Depth bias control capabilities.
#[derive(Debug, Clone, Default)]
pub struct DepthBiasControlCapabilities {
    /// Whether depth bias control is supported.
    pub supported: bool,
    /// Whether least representable value format is supported.
    pub least_representable_value_force_unorm_representation: bool,
    /// Whether float representation is supported.
    pub float_representation: bool,
    /// Whether depth bias exact is supported.
    pub depth_bias_exact: bool,
}

/// Query depth bias control capabilities.
pub fn query_capabilities(ctx: &super::context::VulkanContext) -> DepthBiasControlCapabilities {
    let mut dbc_features = vk::PhysicalDeviceDepthBiasControlFeaturesEXT::default();
    let mut features2 = vk::PhysicalDeviceFeatures2::default()
        .push_next(&mut dbc_features);

    unsafe {
        ctx.instance.get_physical_device_features2(ctx.physical_device, &mut features2);
    }

    let _ = features2;

    DepthBiasControlCapabilities {
        supported: dbc_features.depth_bias_control == vk::TRUE,
        least_representable_value_force_unorm_representation:
            dbc_features.least_representable_value_force_unorm_representation == vk::TRUE,
        float_representation: dbc_features.float_representation == vk::TRUE,
        depth_bias_exact: dbc_features.depth_bias_exact == vk::TRUE,
    }
}

/// Depth bias representation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthBiasRepresentation {
    /// Least representable value (default).
    LeastRepresentableValue,
    /// Least representable value with forced UNORM interpretation.
    LeastRepresentableValueForceUnorm,
    /// Float representation.
    Float,
}

impl DepthBiasRepresentation {
    /// Convert to Vulkan representation.
    pub fn to_vk(&self) -> vk::DepthBiasRepresentationEXT {
        match self {
            DepthBiasRepresentation::LeastRepresentableValue => {
                vk::DepthBiasRepresentationEXT::LEAST_REPRESENTABLE_VALUE_FORMAT
            }
            DepthBiasRepresentation::LeastRepresentableValueForceUnorm => {
                vk::DepthBiasRepresentationEXT::LEAST_REPRESENTABLE_VALUE_FORCE_UNORM
            }
            DepthBiasRepresentation::Float => {
                vk::DepthBiasRepresentationEXT::FLOAT
            }
        }
    }
}

/// Depth bias configuration.
#[derive(Debug, Clone, Copy)]
pub struct DepthBiasConfig {
    /// Constant bias added to depth.
    pub constant_factor: f32,
    /// Bias multiplied by slope.
    pub slope_factor: f32,
    /// Maximum (or minimum) bias value.
    pub clamp: f32,
    /// Depth bias representation.
    pub representation: DepthBiasRepresentation,
    /// Use exact depth bias.
    pub exact: bool,
}

impl Default for DepthBiasConfig {
    fn default() -> Self {
        Self {
            constant_factor: 0.0,
            slope_factor: 0.0,
            clamp: 0.0,
            representation: DepthBiasRepresentation::LeastRepresentableValue,
            exact: false,
        }
    }
}

impl DepthBiasConfig {
    /// Create config for shadow mapping.
    pub fn for_shadow_mapping() -> Self {
        Self {
            constant_factor: 1.0,
            slope_factor: 1.75,
            clamp: 0.0,
            representation: DepthBiasRepresentation::LeastRepresentableValue,
            exact: true,
        }
    }

    /// Create config for decals.
    pub fn for_decals() -> Self {
        Self {
            constant_factor: -1.0,
            slope_factor: -1.0,
            clamp: 0.0,
            representation: DepthBiasRepresentation::LeastRepresentableValue,
            exact: false,
        }
    }

    /// Create config for outline rendering.
    pub fn for_outlines() -> Self {
        Self {
            constant_factor: 1.0,
            slope_factor: 0.0,
            clamp: 0.0,
            representation: DepthBiasRepresentation::LeastRepresentableValue,
            exact: false,
        }
    }
}

/// Shadow map bias presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowBiasPreset {
    /// Low bias (may have shadow acne).
    Low,
    /// Medium bias (balanced).
    Medium,
    /// High bias (may have peter-panning).
    High,
    /// Adaptive based on light angle.
    Adaptive,
}

impl ShadowBiasPreset {
    /// Get depth bias config for preset.
    pub fn to_config(&self) -> DepthBiasConfig {
        match self {
            ShadowBiasPreset::Low => DepthBiasConfig {
                constant_factor: 0.5,
                slope_factor: 1.0,
                clamp: 0.0,
                ..Default::default()
            },
            ShadowBiasPreset::Medium => DepthBiasConfig {
                constant_factor: 1.0,
                slope_factor: 1.75,
                clamp: 0.0,
                ..Default::default()
            },
            ShadowBiasPreset::High => DepthBiasConfig {
                constant_factor: 2.0,
                slope_factor: 3.0,
                clamp: 0.0,
                ..Default::default()
            },
            ShadowBiasPreset::Adaptive => DepthBiasConfig {
                constant_factor: 1.0,
                slope_factor: 2.0,
                clamp: 0.01,
                ..Default::default()
            },
        }
    }
}

/// Create depth bias info structure.
pub fn create_depth_bias_info(config: &DepthBiasConfig) -> vk::DepthBiasInfoEXT<'static> {
    vk::DepthBiasInfoEXT::default()
        .depth_bias_constant_factor(config.constant_factor)
        .depth_bias_slope_factor(config.slope_factor)
        .depth_bias_clamp(config.clamp)
}

/// Create depth bias representation info.
pub fn create_representation_info(
    representation: DepthBiasRepresentation,
    exact: bool,
) -> vk::DepthBiasRepresentationInfoEXT<'static> {
    vk::DepthBiasRepresentationInfoEXT::default()
        .depth_bias_representation(representation.to_vk())
        .depth_bias_exact(exact)
}

/// Depth bias manager.
pub struct DepthBiasManager {
    capabilities: DepthBiasControlCapabilities,
    current_config: DepthBiasConfig,
}

impl DepthBiasManager {
    /// Create new manager.
    pub fn new(ctx: &super::context::VulkanContext) -> Self {
        let capabilities = query_capabilities(ctx);

        Self {
            capabilities,
            current_config: DepthBiasConfig::default(),
        }
    }

    /// Check if depth bias control is supported.
    pub fn is_supported(&self) -> bool {
        self.capabilities.supported
    }

    /// Check if exact depth bias is supported.
    pub fn supports_exact(&self) -> bool {
        self.capabilities.depth_bias_exact
    }

    /// Check if float representation is supported.
    pub fn supports_float(&self) -> bool {
        self.capabilities.float_representation
    }

    /// Set depth bias configuration.
    pub fn set_config(&mut self, config: DepthBiasConfig) {
        self.current_config = config;
    }

    /// Get current configuration.
    pub fn config(&self) -> &DepthBiasConfig {
        &self.current_config
    }

    /// Apply preset.
    pub fn apply_preset(&mut self, preset: ShadowBiasPreset) {
        self.current_config = preset.to_config();
    }

    /// Calculate adaptive bias based on light direction and surface normal.
    pub fn calculate_adaptive_bias(
        &self,
        light_dir: [f32; 3],
        surface_normal: [f32; 3],
        base_config: &DepthBiasConfig,
    ) -> DepthBiasConfig {
        // Dot product between light and normal
        let dot = light_dir[0] * surface_normal[0]
            + light_dir[1] * surface_normal[1]
            + light_dir[2] * surface_normal[2];

        let cos_angle = dot.abs();

        // Scale bias based on angle (steeper angles need more bias)
        let angle_factor = if cos_angle > 0.001 {
            (1.0 / cos_angle).min(10.0)
        } else {
            10.0
        };

        DepthBiasConfig {
            constant_factor: base_config.constant_factor * angle_factor,
            slope_factor: base_config.slope_factor,
            clamp: base_config.clamp,
            representation: base_config.representation,
            exact: base_config.exact,
        }
    }
}

/// Cascaded shadow map bias settings.
#[derive(Debug, Clone)]
pub struct CascadedShadowBias {
    /// Bias per cascade level.
    pub cascade_biases: Vec<DepthBiasConfig>,
}

impl CascadedShadowBias {
    /// Create for number of cascades.
    pub fn new(num_cascades: usize) -> Self {
        let mut cascade_biases = Vec::with_capacity(num_cascades);

        for i in 0..num_cascades {
            // Increase bias for farther cascades (larger texels)
            let scale = 1.0 + i as f32 * 0.5;
            cascade_biases.push(DepthBiasConfig {
                constant_factor: 1.0 * scale,
                slope_factor: 1.75 * scale,
                clamp: 0.0,
                ..Default::default()
            });
        }

        Self { cascade_biases }
    }

    /// Get bias for cascade.
    pub fn get_cascade_bias(&self, cascade: usize) -> &DepthBiasConfig {
        &self.cascade_biases[cascade.min(self.cascade_biases.len() - 1)]
    }

    /// Set bias for cascade.
    pub fn set_cascade_bias(&mut self, cascade: usize, config: DepthBiasConfig) {
        if cascade < self.cascade_biases.len() {
            self.cascade_biases[cascade] = config;
        }
    }
}
