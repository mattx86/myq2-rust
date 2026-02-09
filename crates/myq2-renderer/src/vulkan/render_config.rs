//! Vulkan render configuration.
//!
//! Manages MSAA sample count and anisotropic filtering settings.
//! Reads from cvars and validates against device capabilities.

use ash::vk;

use super::VulkanContext;

/// Render configuration settings.
#[derive(Debug, Clone, Copy)]
pub struct RenderConfig {
    /// MSAA sample count (1, 2, 4, or 8)
    pub msaa_samples: vk::SampleCountFlags,
    /// Anisotropic filtering level (1.0 = disabled, up to device max)
    pub anisotropy_level: f32,
    /// Maximum anisotropy supported by device
    pub max_anisotropy: f32,
    /// Whether MSAA is enabled
    pub msaa_enabled: bool,
    /// Whether anisotropy is enabled
    pub anisotropy_enabled: bool,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            msaa_samples: vk::SampleCountFlags::TYPE_1,
            anisotropy_level: 1.0,
            max_anisotropy: 1.0,
            msaa_enabled: false,
            anisotropy_enabled: false,
        }
    }
}

impl RenderConfig {
    /// Create a new render config from cvar values and device limits.
    ///
    /// # Arguments
    /// * `ctx` - Vulkan context for querying device limits
    /// * `r_msaa` - MSAA sample count from cvar (0=disabled, 2, 4, or 8)
    /// * `r_anisotropy` - Anisotropy level from cvar (1=disabled, 2, 4, 8, or 16)
    pub fn new(ctx: &VulkanContext, r_msaa: i32, r_anisotropy: i32) -> Self {
        let device_limits = &ctx.device_properties.limits;

        // Query max anisotropy from device
        let max_anisotropy = if ctx.device_features.sampler_anisotropy == vk::TRUE {
            device_limits.max_sampler_anisotropy
        } else {
            1.0
        };

        // Determine MSAA sample count
        let (msaa_samples, msaa_enabled) = Self::select_msaa_samples(
            r_msaa,
            device_limits.framebuffer_color_sample_counts,
            device_limits.framebuffer_depth_sample_counts,
        );

        // Determine anisotropy level
        let (anisotropy_level, anisotropy_enabled) = if r_anisotropy > 1 && max_anisotropy > 1.0 {
            let level = (r_anisotropy as f32).min(max_anisotropy);
            (level, true)
        } else {
            (1.0, false)
        };

        Self {
            msaa_samples,
            anisotropy_level,
            max_anisotropy,
            msaa_enabled,
            anisotropy_enabled,
        }
    }

    /// Select MSAA sample count based on requested value and device support.
    fn select_msaa_samples(
        requested: i32,
        color_samples: vk::SampleCountFlags,
        depth_samples: vk::SampleCountFlags,
    ) -> (vk::SampleCountFlags, bool) {
        if requested <= 1 {
            return (vk::SampleCountFlags::TYPE_1, false);
        }

        // Both color and depth must support the sample count
        let supported = color_samples & depth_samples;

        // Try requested sample count, then fall back to lower counts
        let candidates = [
            (8, vk::SampleCountFlags::TYPE_8),
            (4, vk::SampleCountFlags::TYPE_4),
            (2, vk::SampleCountFlags::TYPE_2),
        ];

        for (count, flag) in candidates {
            if requested >= count && supported.contains(flag) {
                return (flag, true);
            }
        }

        (vk::SampleCountFlags::TYPE_1, false)
    }

    /// Convert sample count flags to integer for display.
    pub fn sample_count_as_int(&self) -> u32 {
        match self.msaa_samples {
            vk::SampleCountFlags::TYPE_1 => 1,
            vk::SampleCountFlags::TYPE_2 => 2,
            vk::SampleCountFlags::TYPE_4 => 4,
            vk::SampleCountFlags::TYPE_8 => 8,
            vk::SampleCountFlags::TYPE_16 => 16,
            vk::SampleCountFlags::TYPE_32 => 32,
            vk::SampleCountFlags::TYPE_64 => 64,
            _ => 1,
        }
    }

    /// Update configuration from new cvar values.
    pub fn update(&mut self, ctx: &VulkanContext, r_msaa: i32, r_anisotropy: i32) {
        *self = Self::new(ctx, r_msaa, r_anisotropy);
    }
}

/// Global render configuration.
/// SAFETY: Single-threaded engine, accessed only from main thread.
static mut RENDER_CONFIG: RenderConfig = RenderConfig {
    msaa_samples: vk::SampleCountFlags::TYPE_1,
    anisotropy_level: 1.0,
    max_anisotropy: 1.0,
    msaa_enabled: false,
    anisotropy_enabled: false,
};

/// Initialize the global render configuration.
///
/// # Safety
/// Must be called from the main thread after Vulkan context is initialized.
pub unsafe fn init_render_config(ctx: &VulkanContext, r_msaa: i32, r_anisotropy: i32) {
    RENDER_CONFIG = RenderConfig::new(ctx, r_msaa, r_anisotropy);

    println!(
        "Render config: MSAA={}x ({}), Anisotropy={:.0}x ({})",
        RENDER_CONFIG.sample_count_as_int(),
        if RENDER_CONFIG.msaa_enabled { "enabled" } else { "disabled" },
        RENDER_CONFIG.anisotropy_level,
        if RENDER_CONFIG.anisotropy_enabled { "enabled" } else { "disabled" },
    );
}

/// Update the global render configuration.
///
/// # Safety
/// Must be called from the main thread.
pub unsafe fn update_render_config(ctx: &VulkanContext, r_msaa: i32, r_anisotropy: i32) {
    RENDER_CONFIG.update(ctx, r_msaa, r_anisotropy);
}

/// Get the current render configuration.
pub fn render_config() -> RenderConfig {
    // SAFETY: Single-threaded engine.
    unsafe { RENDER_CONFIG }
}

/// Get the current MSAA sample count.
pub fn msaa_samples() -> vk::SampleCountFlags {
    unsafe { RENDER_CONFIG.msaa_samples }
}

/// Check if MSAA is enabled.
pub fn is_msaa_enabled() -> bool {
    unsafe { RENDER_CONFIG.msaa_enabled }
}

/// Get the current anisotropy level.
pub fn anisotropy_level() -> f32 {
    unsafe { RENDER_CONFIG.anisotropy_level }
}

/// Check if anisotropic filtering is enabled.
pub fn is_anisotropy_enabled() -> bool {
    unsafe { RENDER_CONFIG.anisotropy_enabled }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // RenderConfig::default
    // ============================================================

    #[test]
    fn test_render_config_default() {
        let config = RenderConfig::default();
        assert_eq!(config.msaa_samples, vk::SampleCountFlags::TYPE_1);
        assert_eq!(config.anisotropy_level, 1.0);
        assert_eq!(config.max_anisotropy, 1.0);
        assert!(!config.msaa_enabled);
        assert!(!config.anisotropy_enabled);
    }

    // ============================================================
    // sample_count_as_int
    // ============================================================

    #[test]
    fn test_sample_count_as_int_type_1() {
        let mut config = RenderConfig::default();
        config.msaa_samples = vk::SampleCountFlags::TYPE_1;
        assert_eq!(config.sample_count_as_int(), 1);
    }

    #[test]
    fn test_sample_count_as_int_type_2() {
        let mut config = RenderConfig::default();
        config.msaa_samples = vk::SampleCountFlags::TYPE_2;
        assert_eq!(config.sample_count_as_int(), 2);
    }

    #[test]
    fn test_sample_count_as_int_type_4() {
        let mut config = RenderConfig::default();
        config.msaa_samples = vk::SampleCountFlags::TYPE_4;
        assert_eq!(config.sample_count_as_int(), 4);
    }

    #[test]
    fn test_sample_count_as_int_type_8() {
        let mut config = RenderConfig::default();
        config.msaa_samples = vk::SampleCountFlags::TYPE_8;
        assert_eq!(config.sample_count_as_int(), 8);
    }

    #[test]
    fn test_sample_count_as_int_type_16() {
        let mut config = RenderConfig::default();
        config.msaa_samples = vk::SampleCountFlags::TYPE_16;
        assert_eq!(config.sample_count_as_int(), 16);
    }

    #[test]
    fn test_sample_count_as_int_type_32() {
        let mut config = RenderConfig::default();
        config.msaa_samples = vk::SampleCountFlags::TYPE_32;
        assert_eq!(config.sample_count_as_int(), 32);
    }

    #[test]
    fn test_sample_count_as_int_type_64() {
        let mut config = RenderConfig::default();
        config.msaa_samples = vk::SampleCountFlags::TYPE_64;
        assert_eq!(config.sample_count_as_int(), 64);
    }

    // ============================================================
    // select_msaa_samples (internal logic via direct construction)
    // ============================================================

    #[test]
    fn test_select_msaa_disabled_when_requested_0() {
        let all_supported = vk::SampleCountFlags::TYPE_1
            | vk::SampleCountFlags::TYPE_2
            | vk::SampleCountFlags::TYPE_4
            | vk::SampleCountFlags::TYPE_8;
        let (samples, enabled) = RenderConfig::select_msaa_samples(0, all_supported, all_supported);
        assert_eq!(samples, vk::SampleCountFlags::TYPE_1);
        assert!(!enabled);
    }

    #[test]
    fn test_select_msaa_disabled_when_requested_1() {
        let all_supported = vk::SampleCountFlags::TYPE_1
            | vk::SampleCountFlags::TYPE_2
            | vk::SampleCountFlags::TYPE_4
            | vk::SampleCountFlags::TYPE_8;
        let (samples, enabled) = RenderConfig::select_msaa_samples(1, all_supported, all_supported);
        assert_eq!(samples, vk::SampleCountFlags::TYPE_1);
        assert!(!enabled);
    }

    #[test]
    fn test_select_msaa_2x_supported() {
        let supported = vk::SampleCountFlags::TYPE_1 | vk::SampleCountFlags::TYPE_2;
        let (samples, enabled) = RenderConfig::select_msaa_samples(2, supported, supported);
        assert_eq!(samples, vk::SampleCountFlags::TYPE_2);
        assert!(enabled);
    }

    #[test]
    fn test_select_msaa_4x_supported() {
        let supported = vk::SampleCountFlags::TYPE_1
            | vk::SampleCountFlags::TYPE_2
            | vk::SampleCountFlags::TYPE_4;
        let (samples, enabled) = RenderConfig::select_msaa_samples(4, supported, supported);
        assert_eq!(samples, vk::SampleCountFlags::TYPE_4);
        assert!(enabled);
    }

    #[test]
    fn test_select_msaa_8x_supported() {
        let supported = vk::SampleCountFlags::TYPE_1
            | vk::SampleCountFlags::TYPE_2
            | vk::SampleCountFlags::TYPE_4
            | vk::SampleCountFlags::TYPE_8;
        let (samples, enabled) = RenderConfig::select_msaa_samples(8, supported, supported);
        assert_eq!(samples, vk::SampleCountFlags::TYPE_8);
        assert!(enabled);
    }

    #[test]
    fn test_select_msaa_fallback_when_not_supported() {
        // Request 8x but only 4x is supported
        let supported = vk::SampleCountFlags::TYPE_1
            | vk::SampleCountFlags::TYPE_2
            | vk::SampleCountFlags::TYPE_4;
        let (samples, enabled) = RenderConfig::select_msaa_samples(8, supported, supported);
        // Should fall back to 4x
        assert_eq!(samples, vk::SampleCountFlags::TYPE_4);
        assert!(enabled);
    }

    #[test]
    fn test_select_msaa_fallback_to_2x() {
        // Request 8x but only 2x is supported
        let supported = vk::SampleCountFlags::TYPE_1 | vk::SampleCountFlags::TYPE_2;
        let (samples, enabled) = RenderConfig::select_msaa_samples(8, supported, supported);
        assert_eq!(samples, vk::SampleCountFlags::TYPE_2);
        assert!(enabled);
    }

    #[test]
    fn test_select_msaa_fallback_to_1x_when_nothing_supported() {
        // Request 2x but only 1x is supported
        let supported = vk::SampleCountFlags::TYPE_1;
        let (samples, enabled) = RenderConfig::select_msaa_samples(2, supported, supported);
        assert_eq!(samples, vk::SampleCountFlags::TYPE_1);
        assert!(!enabled);
    }

    #[test]
    fn test_select_msaa_color_depth_intersection() {
        // Color supports 4x and 8x, depth only supports 4x
        let color = vk::SampleCountFlags::TYPE_1
            | vk::SampleCountFlags::TYPE_2
            | vk::SampleCountFlags::TYPE_4
            | vk::SampleCountFlags::TYPE_8;
        let depth = vk::SampleCountFlags::TYPE_1
            | vk::SampleCountFlags::TYPE_2
            | vk::SampleCountFlags::TYPE_4;
        let (samples, enabled) = RenderConfig::select_msaa_samples(8, color, depth);
        // Should get 4x because depth doesn't support 8x
        assert_eq!(samples, vk::SampleCountFlags::TYPE_4);
        assert!(enabled);
    }

    // ============================================================
    // RenderConfig field modification
    // ============================================================

    #[test]
    fn test_render_config_manual_construction() {
        let config = RenderConfig {
            msaa_samples: vk::SampleCountFlags::TYPE_4,
            anisotropy_level: 8.0,
            max_anisotropy: 16.0,
            msaa_enabled: true,
            anisotropy_enabled: true,
        };
        assert_eq!(config.sample_count_as_int(), 4);
        assert_eq!(config.anisotropy_level, 8.0);
        assert_eq!(config.max_anisotropy, 16.0);
        assert!(config.msaa_enabled);
        assert!(config.anisotropy_enabled);
    }

    #[test]
    fn test_render_config_copy_semantics() {
        let config1 = RenderConfig {
            msaa_samples: vk::SampleCountFlags::TYPE_8,
            anisotropy_level: 16.0,
            max_anisotropy: 16.0,
            msaa_enabled: true,
            anisotropy_enabled: true,
        };
        let config2 = config1; // Copy
        assert_eq!(config2.sample_count_as_int(), 8);
        assert_eq!(config2.anisotropy_level, 16.0);
    }
}
