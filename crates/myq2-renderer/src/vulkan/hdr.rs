//! HDR rendering and tonemapping
//!
//! Provides HDR rendering pipeline with various tonemapping operators
//! for displaying HDR content on both SDR and HDR displays.
//!
//! Supported features:
//! - HDR10 output (VK_EXT_hdr_metadata)
//! - Various tonemapping operators (Reinhard, ACES, Uncharted 2, etc.)
//! - Auto-exposure with eye adaptation
//! - Bloom integration

use ash::vk;

/// HDR display capabilities.
#[derive(Debug, Clone)]
pub struct HdrCapabilities {
    /// Whether HDR output is supported.
    pub supported: bool,
    /// Whether HDR10 (ST2084 PQ) is supported.
    pub hdr10_supported: bool,
    /// Whether scRGB (extended sRGB) is supported.
    pub scrgb_supported: bool,
    /// Maximum luminance (nits).
    pub max_luminance: f32,
    /// Minimum luminance (nits).
    pub min_luminance: f32,
    /// Maximum full-frame luminance (nits).
    pub max_frame_luminance: f32,
}

impl Default for HdrCapabilities {
    fn default() -> Self {
        Self {
            supported: false,
            hdr10_supported: false,
            scrgb_supported: false,
            max_luminance: 80.0, // SDR
            min_luminance: 0.1,
            max_frame_luminance: 80.0,
        }
    }
}

/// Tonemapping operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TonemapOperator {
    /// No tonemapping (linear clamp).
    None,
    /// Reinhard simple.
    Reinhard,
    /// Reinhard extended with white point.
    ReinhardExtended,
    /// ACES filmic.
    Aces,
    /// Uncharted 2 filmic.
    Uncharted2,
    /// Khronos PBR Neutral.
    KhronosPbrNeutral,
    /// AgX (Blender's new default).
    AgX,
    /// Custom LUT-based.
    CustomLut,
}

/// Auto-exposure mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoExposureMode {
    /// Fixed exposure.
    Fixed,
    /// Simple average luminance.
    Average,
    /// Histogram-based with percentile.
    Histogram,
    /// Spot metering (center-weighted).
    Spot,
}

/// HDR rendering configuration.
#[derive(Debug, Clone)]
pub struct HdrConfig {
    /// Whether HDR output is enabled.
    pub hdr_output_enabled: bool,
    /// Tonemapping operator (for SDR output).
    pub tonemap_operator: TonemapOperator,
    /// Auto-exposure mode.
    pub auto_exposure_mode: AutoExposureMode,
    /// Manual exposure value (EV).
    pub manual_exposure: f32,
    /// Auto-exposure adaptation speed.
    pub adaptation_speed: f32,
    /// Minimum exposure (EV).
    pub min_exposure: f32,
    /// Maximum exposure (EV).
    pub max_exposure: f32,
    /// Bloom intensity.
    pub bloom_intensity: f32,
    /// Bloom threshold (luminance).
    pub bloom_threshold: f32,
    /// White point for Reinhard extended (nits).
    pub white_point: f32,
    /// Gamma for final output.
    pub gamma: f32,
}

impl Default for HdrConfig {
    fn default() -> Self {
        Self {
            hdr_output_enabled: false,
            tonemap_operator: TonemapOperator::Aces,
            auto_exposure_mode: AutoExposureMode::Histogram,
            manual_exposure: 0.0, // EV 0 = neutral
            adaptation_speed: 1.5,
            min_exposure: -4.0,
            max_exposure: 4.0,
            bloom_intensity: 0.5,
            bloom_threshold: 1.0,
            white_point: 4.0,
            gamma: 2.2,
        }
    }
}

/// Luminance histogram for auto-exposure.
#[derive(Clone)]
pub struct LuminanceHistogram {
    /// Histogram bins (log2 luminance).
    bins: [u32; 256],
    /// Total pixel count.
    total_pixels: u32,
    /// Minimum log2 luminance.
    min_log: f32,
    /// Maximum log2 luminance.
    max_log: f32,
}

impl Default for LuminanceHistogram {
    fn default() -> Self {
        Self {
            bins: [0; 256],
            total_pixels: 0,
            min_log: -10.0, // 0.001 nits
            max_log: 10.0,  // 1024 nits
        }
    }
}

impl LuminanceHistogram {
    /// Calculate average luminance from histogram.
    pub fn average_luminance(&self) -> f32 {
        if self.total_pixels == 0 {
            return 1.0;
        }

        let mut sum = 0.0f64;
        let log_range = self.max_log - self.min_log;

        for (i, &count) in self.bins.iter().enumerate() {
            let log_lum = self.min_log + (i as f32 / 255.0) * log_range;
            sum += (count as f64) * (2.0f64.powf(log_lum as f64));
        }

        (sum / self.total_pixels as f64) as f32
    }

    /// Calculate luminance at a percentile (0-100).
    pub fn percentile_luminance(&self, percentile: f32) -> f32 {
        if self.total_pixels == 0 {
            return 1.0;
        }

        let target = (self.total_pixels as f32 * percentile / 100.0) as u32;
        let mut accumulated = 0u32;
        let log_range = self.max_log - self.min_log;

        for (i, &count) in self.bins.iter().enumerate() {
            accumulated += count;
            if accumulated >= target {
                let log_lum = self.min_log + (i as f32 / 255.0) * log_range;
                return 2.0f32.powf(log_lum);
            }
        }

        2.0f32.powf(self.max_log)
    }
}

/// HDR rendering manager.
pub struct HdrManager {
    /// HDR capabilities.
    capabilities: HdrCapabilities,
    /// Current configuration.
    config: HdrConfig,
    /// Current exposure value.
    current_exposure: f32,
    /// Target exposure value (for adaptation).
    target_exposure: f32,
    /// Luminance histogram.
    histogram: LuminanceHistogram,
    /// Histogram buffer (GPU).
    histogram_buffer: vk::Buffer,
    histogram_memory: vk::DeviceMemory,
    /// Average luminance buffer.
    luminance_buffer: vk::Buffer,
    luminance_memory: vk::DeviceMemory,
    /// Custom LUT texture (if using LUT tonemapping).
    lut_texture: vk::Image,
    lut_view: vk::ImageView,
    lut_memory: vk::DeviceMemory,
}

impl HdrManager {
    /// Query HDR capabilities from the surface.
    pub fn query_capabilities(
        ctx: &super::context::VulkanContext,
        surface: vk::SurfaceKHR,
    ) -> HdrCapabilities {
        let formats = unsafe {
            ctx.surface_loader
                .get_physical_device_surface_formats(ctx.physical_device, surface)
                .unwrap_or_default()
        };

        let mut caps = HdrCapabilities::default();

        for format in &formats {
            match format.color_space {
                vk::ColorSpaceKHR::HDR10_ST2084_EXT => {
                    caps.hdr10_supported = true;
                    caps.supported = true;
                }
                vk::ColorSpaceKHR::EXTENDED_SRGB_LINEAR_EXT => {
                    caps.scrgb_supported = true;
                    caps.supported = true;
                }
                _ => {}
            }
        }

        if caps.hdr10_supported {
            // HDR10 typical values
            caps.max_luminance = 1000.0;
            caps.min_luminance = 0.001;
            caps.max_frame_luminance = 400.0;
        }

        caps
    }

    /// Create a new HDR manager.
    pub fn new(ctx: &super::context::VulkanContext, surface: vk::SurfaceKHR) -> Result<Self, String> {
        let capabilities = Self::query_capabilities(ctx, surface);

        // Create histogram buffer
        let histogram_size = std::mem::size_of::<[u32; 256]>() as u64;
        let (histogram_buffer, histogram_memory) = Self::create_buffer(
            ctx,
            histogram_size,
            vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
        )?;

        // Create luminance buffer
        let luminance_size = std::mem::size_of::<f32>() as u64;
        let (luminance_buffer, luminance_memory) = Self::create_buffer(
            ctx,
            luminance_size,
            vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_SRC,
        )?;

        Ok(Self {
            capabilities,
            config: HdrConfig::default(),
            current_exposure: 0.0,
            target_exposure: 0.0,
            histogram: LuminanceHistogram::default(),
            histogram_buffer,
            histogram_memory,
            luminance_buffer,
            luminance_memory,
            lut_texture: vk::Image::null(),
            lut_view: vk::ImageView::null(),
            lut_memory: vk::DeviceMemory::null(),
        })
    }

    fn create_buffer(
        ctx: &super::context::VulkanContext,
        size: u64,
        usage: vk::BufferUsageFlags,
    ) -> Result<(vk::Buffer, vk::DeviceMemory), String> {
        let buffer_info = vk::BufferCreateInfo::default()
            .size(size)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let buffer = unsafe {
            ctx.device.create_buffer(&buffer_info, None)
                .map_err(|e| format!("Failed to create buffer: {:?}", e))?
        };

        let mem_reqs = unsafe { ctx.device.get_buffer_memory_requirements(buffer) };

        let mem_props = unsafe {
            ctx.instance.get_physical_device_memory_properties(ctx.physical_device)
        };

        let memory_type = (0..mem_props.memory_type_count)
            .find(|i| {
                (mem_reqs.memory_type_bits & (1 << i)) != 0
                    && mem_props.memory_types[*i as usize].property_flags
                        .contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)
            })
            .ok_or_else(|| "Failed to find suitable memory type".to_string())?;

        let alloc_info = vk::MemoryAllocateInfo::default()
            .allocation_size(mem_reqs.size)
            .memory_type_index(memory_type);

        let memory = unsafe {
            ctx.device.allocate_memory(&alloc_info, None)
                .map_err(|e| format!("Failed to allocate buffer memory: {:?}", e))?
        };

        unsafe {
            ctx.device.bind_buffer_memory(buffer, memory, 0)
                .map_err(|e| format!("Failed to bind buffer memory: {:?}", e))?;
        }

        Ok((buffer, memory))
    }

    /// Get HDR capabilities.
    pub fn capabilities(&self) -> &HdrCapabilities {
        &self.capabilities
    }

    /// Get current configuration.
    pub fn config(&self) -> &HdrConfig {
        &self.config
    }

    /// Set configuration.
    pub fn set_config(&mut self, config: HdrConfig) {
        self.config = config;
    }

    /// Get current exposure value.
    pub fn current_exposure(&self) -> f32 {
        self.current_exposure
    }

    /// Update exposure based on scene luminance.
    pub fn update_exposure(&mut self, delta_time: f32) {
        match self.config.auto_exposure_mode {
            AutoExposureMode::Fixed => {
                self.target_exposure = self.config.manual_exposure;
            }
            AutoExposureMode::Average => {
                let avg_lum = self.histogram.average_luminance();
                self.target_exposure = self.luminance_to_ev(avg_lum);
            }
            AutoExposureMode::Histogram => {
                // Use 50th percentile for more stable exposure
                let median_lum = self.histogram.percentile_luminance(50.0);
                self.target_exposure = self.luminance_to_ev(median_lum);
            }
            AutoExposureMode::Spot => {
                // Would use center-weighted luminance
                let avg_lum = self.histogram.average_luminance();
                self.target_exposure = self.luminance_to_ev(avg_lum);
            }
        }

        // Clamp target
        self.target_exposure = self.target_exposure
            .clamp(self.config.min_exposure, self.config.max_exposure);

        // Adapt current exposure towards target
        let adapt_rate = self.config.adaptation_speed * delta_time;
        self.current_exposure += (self.target_exposure - self.current_exposure) * adapt_rate.min(1.0);
    }

    /// Convert luminance to EV (exposure value).
    fn luminance_to_ev(&self, luminance: f32) -> f32 {
        // EV = log2(luminance / 0.18)
        // 0.18 is middle gray
        (luminance / 0.18).max(0.001).log2()
    }

    /// Get exposure multiplier from EV.
    pub fn exposure_multiplier(&self) -> f32 {
        2.0f32.powf(-self.current_exposure)
    }

    /// Destroy the HDR manager.
    pub fn destroy(&mut self, ctx: &super::context::VulkanContext) {
        unsafe {
            ctx.device.destroy_buffer(self.histogram_buffer, None);
            ctx.device.free_memory(self.histogram_memory, None);
            ctx.device.destroy_buffer(self.luminance_buffer, None);
            ctx.device.free_memory(self.luminance_memory, None);

            if self.lut_texture != vk::Image::null() {
                ctx.device.destroy_image_view(self.lut_view, None);
                ctx.device.destroy_image(self.lut_texture, None);
                ctx.device.free_memory(self.lut_memory, None);
            }
        }
    }
}

/// GLSL code for tonemapping operators.
pub const TONEMAP_GLSL: &str = r#"
// Tonemapping operators for HDR rendering

// Exposure adjustment
vec3 applyExposure(vec3 color, float exposure) {
    return color * pow(2.0, exposure);
}

// Reinhard simple
vec3 tonemapReinhard(vec3 color) {
    return color / (1.0 + color);
}

// Reinhard extended with white point
vec3 tonemapReinhardExtended(vec3 color, float whitePoint) {
    vec3 numerator = color * (1.0 + color / (whitePoint * whitePoint));
    return numerator / (1.0 + color);
}

// ACES filmic (approximation by Krzysztof Narkowicz)
vec3 tonemapAces(vec3 x) {
    const float a = 2.51;
    const float b = 0.03;
    const float c = 2.43;
    const float d = 0.59;
    const float e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), 0.0, 1.0);
}

// Uncharted 2 filmic
vec3 uncharted2Tonemap(vec3 x) {
    const float A = 0.15;
    const float B = 0.50;
    const float C = 0.10;
    const float D = 0.20;
    const float E = 0.02;
    const float F = 0.30;
    return ((x * (A * x + C * B) + D * E) / (x * (A * x + B) + D * F)) - E / F;
}

vec3 tonemapUncharted2(vec3 color) {
    const float W = 11.2; // Linear white point
    const float exposureBias = 2.0;
    vec3 curr = uncharted2Tonemap(exposureBias * color);
    vec3 whiteScale = 1.0 / uncharted2Tonemap(vec3(W));
    return curr * whiteScale;
}

// Khronos PBR Neutral
vec3 tonemapKhronosPbrNeutral(vec3 color) {
    const float startCompression = 0.8 - 0.04;
    const float desaturation = 0.15;

    float x = min(color.r, min(color.g, color.b));
    float offset = x < 0.08 ? x - 6.25 * x * x : 0.04;
    color -= offset;

    float peak = max(color.r, max(color.g, color.b));
    if (peak < startCompression) return color;

    float d = 1.0 - startCompression;
    float newPeak = 1.0 - d * d / (peak + d - startCompression);
    color *= newPeak / peak;

    float g = 1.0 - 1.0 / (desaturation * (peak - newPeak) + 1.0);
    return mix(color, vec3(newPeak), g);
}

// Apply gamma correction
vec3 gammaCorrect(vec3 color, float gamma) {
    return pow(color, vec3(1.0 / gamma));
}

// sRGB OETF (gamma ~2.2)
vec3 linearToSrgb(vec3 color) {
    vec3 low = color * 12.92;
    vec3 high = pow(color, vec3(1.0 / 2.4)) * 1.055 - 0.055;
    return mix(low, high, step(vec3(0.0031308), color));
}

// PQ (ST.2084) EOTF for HDR10
vec3 linearToPq(vec3 color) {
    const float m1 = 0.1593017578125;
    const float m2 = 78.84375;
    const float c1 = 0.8359375;
    const float c2 = 18.8515625;
    const float c3 = 18.6875;

    vec3 Lp = pow(color / 10000.0, vec3(m1));
    return pow((c1 + c2 * Lp) / (1.0 + c3 * Lp), vec3(m2));
}
"#;
