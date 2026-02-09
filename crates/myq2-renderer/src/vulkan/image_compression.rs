//! Image Compression Control
//!
//! VK_EXT_image_compression_control provides control over GPU image compression:
//! - Query available compression types
//! - Force specific compression modes
//! - Optimize for bandwidth vs quality
//! - Useful for render targets and textures

use ash::vk;

/// Image compression capabilities.
#[derive(Debug, Clone, Default)]
pub struct ImageCompressionCapabilities {
    /// Whether image compression control is supported.
    pub supported: bool,
    /// Available fixed-rate compression flags.
    pub fixed_rate_flags: Vec<FixedRateCompressionFlag>,
}

/// Fixed-rate compression flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedRateCompressionFlag {
    /// Bits per pixel for this compression mode.
    pub bits_per_pixel: u32,
    /// Vulkan flag value.
    pub flag: vk::ImageCompressionFixedRateFlagsEXT,
}

/// Query image compression capabilities.
pub fn query_capabilities(ctx: &super::context::VulkanContext) -> ImageCompressionCapabilities {
    let mut compression_features = vk::PhysicalDeviceImageCompressionControlFeaturesEXT::default();
    let mut features2 = vk::PhysicalDeviceFeatures2::default()
        .push_next(&mut compression_features);

    unsafe {
        ctx.instance.get_physical_device_features2(ctx.physical_device, &mut features2);
    }

    let _ = features2;
    let supported = compression_features.image_compression_control == vk::TRUE;

    if !supported {
        return ImageCompressionCapabilities::default();
    }

    // Common fixed-rate compression modes
    let fixed_rate_flags = vec![
        FixedRateCompressionFlag {
            bits_per_pixel: 1,
            flag: vk::ImageCompressionFixedRateFlagsEXT::TYPE_1BPC,
        },
        FixedRateCompressionFlag {
            bits_per_pixel: 2,
            flag: vk::ImageCompressionFixedRateFlagsEXT::TYPE_2BPC,
        },
        FixedRateCompressionFlag {
            bits_per_pixel: 3,
            flag: vk::ImageCompressionFixedRateFlagsEXT::TYPE_3BPC,
        },
        FixedRateCompressionFlag {
            bits_per_pixel: 4,
            flag: vk::ImageCompressionFixedRateFlagsEXT::TYPE_4BPC,
        },
        FixedRateCompressionFlag {
            bits_per_pixel: 5,
            flag: vk::ImageCompressionFixedRateFlagsEXT::TYPE_5BPC,
        },
        FixedRateCompressionFlag {
            bits_per_pixel: 6,
            flag: vk::ImageCompressionFixedRateFlagsEXT::TYPE_6BPC,
        },
    ];

    ImageCompressionCapabilities {
        supported,
        fixed_rate_flags,
    }
}

/// Compression mode for image creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionMode {
    /// Default compression (driver decides).
    Default,
    /// No compression (highest bandwidth usage).
    Disabled,
    /// Fixed-rate compression with specific bits per component.
    FixedRate(u32),
}

impl CompressionMode {
    /// Convert to Vulkan compression flags.
    pub fn to_vk_flags(&self) -> vk::ImageCompressionFlagsEXT {
        match self {
            CompressionMode::Default => vk::ImageCompressionFlagsEXT::DEFAULT,
            CompressionMode::Disabled => vk::ImageCompressionFlagsEXT::DISABLED,
            CompressionMode::FixedRate(_) => vk::ImageCompressionFlagsEXT::FIXED_RATE_EXPLICIT,
        }
    }

    /// Get fixed-rate flag for this mode.
    pub fn to_fixed_rate_flag(&self) -> vk::ImageCompressionFixedRateFlagsEXT {
        match self {
            CompressionMode::FixedRate(bpc) => match bpc {
                1 => vk::ImageCompressionFixedRateFlagsEXT::TYPE_1BPC,
                2 => vk::ImageCompressionFixedRateFlagsEXT::TYPE_2BPC,
                3 => vk::ImageCompressionFixedRateFlagsEXT::TYPE_3BPC,
                4 => vk::ImageCompressionFixedRateFlagsEXT::TYPE_4BPC,
                5 => vk::ImageCompressionFixedRateFlagsEXT::TYPE_5BPC,
                6 => vk::ImageCompressionFixedRateFlagsEXT::TYPE_6BPC,
                _ => vk::ImageCompressionFixedRateFlagsEXT::NONE,
            },
            _ => vk::ImageCompressionFixedRateFlagsEXT::NONE,
        }
    }
}

/// Image compression properties for a specific format.
#[derive(Debug, Clone)]
pub struct FormatCompressionProperties {
    /// Format being queried.
    pub format: vk::Format,
    /// Supported compression flags.
    pub compression_flags: vk::ImageCompressionFlagsEXT,
    /// Supported fixed-rate flags.
    pub fixed_rate_flags: vk::ImageCompressionFixedRateFlagsEXT,
}

/// Query compression properties for a format.
pub fn query_format_compression(
    ctx: &super::context::VulkanContext,
    format: vk::Format,
    image_type: vk::ImageType,
    tiling: vk::ImageTiling,
    usage: vk::ImageUsageFlags,
) -> Option<FormatCompressionProperties> {
    let mut compression_props = vk::ImageCompressionPropertiesEXT::default();
    let mut format_props = vk::ImageFormatProperties2::default()
        .push_next(&mut compression_props);

    let image_info = vk::PhysicalDeviceImageFormatInfo2::default()
        .format(format)
        .ty(image_type)
        .tiling(tiling)
        .usage(usage)
        .flags(vk::ImageCreateFlags::empty());

    let result = unsafe {
        ctx.instance.get_physical_device_image_format_properties2(
            ctx.physical_device,
            &image_info,
            &mut format_props,
        )
    };

    if result.is_err() {
        return None;
    }

    Some(FormatCompressionProperties {
        format,
        compression_flags: compression_props.image_compression_flags,
        fixed_rate_flags: compression_props.image_compression_fixed_rate_flags,
    })
}

/// Create image compression control info for image creation.
pub fn create_compression_control(
    mode: CompressionMode,
) -> vk::ImageCompressionControlEXT<'static> {
    vk::ImageCompressionControlEXT::default()
        .flags(mode.to_vk_flags())
}

/// Recommendations for compression based on usage.
#[derive(Debug, Clone, Copy)]
pub enum CompressionRecommendation {
    /// Use for render targets (favor speed).
    RenderTarget,
    /// Use for textures (favor quality).
    Texture,
    /// Use for depth/stencil buffers.
    DepthStencil,
    /// Use for storage images (favor bandwidth).
    Storage,
    /// Use for swapchain images.
    Swapchain,
}

impl CompressionRecommendation {
    /// Get recommended compression mode.
    pub fn recommended_mode(&self) -> CompressionMode {
        match self {
            CompressionRecommendation::RenderTarget => CompressionMode::Default,
            CompressionRecommendation::Texture => CompressionMode::Default,
            CompressionRecommendation::DepthStencil => CompressionMode::Default,
            CompressionRecommendation::Storage => CompressionMode::FixedRate(4),
            CompressionRecommendation::Swapchain => CompressionMode::Default,
        }
    }

    /// Get recommended fixed-rate bits per component.
    pub fn recommended_bpc(&self) -> Option<u32> {
        match self {
            CompressionRecommendation::Storage => Some(4),
            _ => None,
        }
    }
}

/// Compression statistics for monitoring.
#[derive(Debug, Clone, Default)]
pub struct CompressionStats {
    /// Number of images with compression enabled.
    pub compressed_image_count: u32,
    /// Number of images with compression disabled.
    pub uncompressed_image_count: u32,
    /// Estimated memory savings (bytes).
    pub estimated_savings_bytes: u64,
}

impl CompressionStats {
    /// Add an image to stats.
    pub fn add_image(&mut self, compressed: bool, original_size: u64, compressed_size: u64) {
        if compressed {
            self.compressed_image_count += 1;
            self.estimated_savings_bytes += original_size.saturating_sub(compressed_size);
        } else {
            self.uncompressed_image_count += 1;
        }
    }

    /// Get compression ratio.
    pub fn compression_ratio(&self) -> f32 {
        let total = self.compressed_image_count + self.uncompressed_image_count;
        if total == 0 {
            0.0
        } else {
            self.compressed_image_count as f32 / total as f32
        }
    }
}
