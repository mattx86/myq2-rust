//! GPU upscaling technologies
//!
//! Supports multiple upscaling backends:
//! - FSR 1.0/2.0 (AMD, open source) - already implemented
//! - DLSS (NVIDIA, proprietary NGX SDK)
//! - XeSS (Intel, open SDK)

pub mod dlss;
pub mod xess;

use ash::vk;

/// Upscaler quality presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpscalerQuality {
    /// Ultra Performance - maximum performance, lowest quality (3x scale)
    UltraPerformance,
    /// Performance - good balance for high framerates (2x scale)
    Performance,
    /// Balanced - balanced quality and performance (1.7x scale)
    Balanced,
    /// Quality - high quality with moderate upscaling (1.5x scale)
    Quality,
    /// Ultra Quality - minimal upscaling, best quality (1.3x scale)
    UltraQuality,
    /// Native AA - no upscaling, just anti-aliasing (1x scale)
    NativeAA,
}

impl UpscalerQuality {
    /// Get the render scale for this quality level.
    pub fn render_scale(&self) -> f32 {
        match self {
            UpscalerQuality::UltraPerformance => 0.33,
            UpscalerQuality::Performance => 0.5,
            UpscalerQuality::Balanced => 0.58,
            UpscalerQuality::Quality => 0.67,
            UpscalerQuality::UltraQuality => 0.77,
            UpscalerQuality::NativeAA => 1.0,
        }
    }

    /// Get the display name for this quality level.
    pub fn name(&self) -> &'static str {
        match self {
            UpscalerQuality::UltraPerformance => "Ultra Performance",
            UpscalerQuality::Performance => "Performance",
            UpscalerQuality::Balanced => "Balanced",
            UpscalerQuality::Quality => "Quality",
            UpscalerQuality::UltraQuality => "Ultra Quality",
            UpscalerQuality::NativeAA => "Native AA",
        }
    }
}

/// Upscaler backend type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpscalerBackend {
    /// AMD FidelityFX Super Resolution
    Fsr,
    /// NVIDIA Deep Learning Super Sampling
    Dlss,
    /// Intel Xe Super Sampling
    XeSS,
    /// No upscaling
    None,
}

/// Common interface for upscaler implementations.
pub trait Upscaler {
    /// Check if the upscaler is available on the current hardware.
    fn is_available(&self) -> bool;

    /// Get the backend type.
    fn backend(&self) -> UpscalerBackend;

    /// Get the current quality preset.
    fn quality(&self) -> UpscalerQuality;

    /// Set the quality preset.
    fn set_quality(&mut self, quality: UpscalerQuality);

    /// Get the jitter offset for the current frame.
    fn jitter_offset(&self) -> (f32, f32);

    /// Advance to the next frame (update jitter, etc.).
    fn next_frame(&mut self);

    /// Resize the internal buffers.
    fn resize(&mut self, render_width: u32, render_height: u32, display_width: u32, display_height: u32);

    /// Execute the upscaling pass.
    fn execute(
        &self,
        cmd: vk::CommandBuffer,
        color_input: vk::ImageView,
        depth_input: vk::ImageView,
        motion_vectors: vk::ImageView,
        output: vk::ImageView,
    );

    /// Shutdown and release resources.
    fn shutdown(&mut self);
}

/// Upscaler manager that handles multiple backends.
pub struct UpscalerManager {
    /// Currently active backend.
    active_backend: UpscalerBackend,
    /// DLSS instance (if available).
    dlss: Option<dlss::DlssUpscaler>,
    /// XeSS instance (if available).
    xess: Option<xess::XeSSUpscaler>,
    /// Current quality preset.
    quality: UpscalerQuality,
    /// Render dimensions.
    render_width: u32,
    render_height: u32,
    /// Display dimensions.
    display_width: u32,
    display_height: u32,
}

impl UpscalerManager {
    /// Create a new upscaler manager and detect available backends.
    pub fn new(ctx: &super::context::VulkanContext) -> Self {
        let dlss = dlss::DlssUpscaler::new(ctx);
        let xess = xess::XeSSUpscaler::new(ctx);

        // Select best available backend
        let active_backend = if dlss.as_ref().is_some_and(|d| d.is_available()) {
            UpscalerBackend::Dlss
        } else if xess.as_ref().is_some_and(|x| x.is_available()) {
            UpscalerBackend::XeSS
        } else {
            UpscalerBackend::Fsr // Fallback to FSR (always available)
        };

        Self {
            active_backend,
            dlss,
            xess,
            quality: UpscalerQuality::Quality,
            render_width: 0,
            render_height: 0,
            display_width: 0,
            display_height: 0,
        }
    }

    /// Get available upscaler backends.
    pub fn available_backends(&self) -> Vec<UpscalerBackend> {
        let mut backends = vec![UpscalerBackend::Fsr]; // FSR always available

        if self.dlss.as_ref().is_some_and(|d| d.is_available()) {
            backends.push(UpscalerBackend::Dlss);
        }
        if self.xess.as_ref().is_some_and(|x| x.is_available()) {
            backends.push(UpscalerBackend::XeSS);
        }

        backends
    }

    /// Get the currently active backend.
    pub fn active_backend(&self) -> UpscalerBackend {
        self.active_backend
    }

    /// Set the active backend.
    pub fn set_backend(&mut self, backend: UpscalerBackend) {
        // Validate backend is available
        match backend {
            UpscalerBackend::Dlss => {
                if self.dlss.as_ref().is_some_and(|d| d.is_available()) {
                    self.active_backend = backend;
                }
            }
            UpscalerBackend::XeSS => {
                if self.xess.as_ref().is_some_and(|x| x.is_available()) {
                    self.active_backend = backend;
                }
            }
            UpscalerBackend::Fsr | UpscalerBackend::None => {
                self.active_backend = backend;
            }
        }
    }

    /// Get the current quality preset.
    pub fn quality(&self) -> UpscalerQuality {
        self.quality
    }

    /// Set the quality preset.
    pub fn set_quality(&mut self, quality: UpscalerQuality) {
        self.quality = quality;

        if let Some(ref mut dlss) = self.dlss {
            dlss.set_quality(quality);
        }
        if let Some(ref mut xess) = self.xess {
            xess.set_quality(quality);
        }
    }

    /// Get the render scale for the current quality.
    pub fn render_scale(&self) -> f32 {
        self.quality.render_scale()
    }

    /// Get the jitter offset for the current frame.
    pub fn jitter_offset(&self) -> (f32, f32) {
        match self.active_backend {
            UpscalerBackend::Dlss => {
                self.dlss.as_ref().map(|d| d.jitter_offset()).unwrap_or((0.0, 0.0))
            }
            UpscalerBackend::XeSS => {
                self.xess.as_ref().map(|x| x.jitter_offset()).unwrap_or((0.0, 0.0))
            }
            _ => (0.0, 0.0),
        }
    }

    /// Advance to the next frame.
    pub fn next_frame(&mut self) {
        if let Some(ref mut dlss) = self.dlss {
            dlss.next_frame();
        }
        if let Some(ref mut xess) = self.xess {
            xess.next_frame();
        }
    }

    /// Resize internal buffers.
    pub fn resize(&mut self, render_width: u32, render_height: u32, display_width: u32, display_height: u32) {
        self.render_width = render_width;
        self.render_height = render_height;
        self.display_width = display_width;
        self.display_height = display_height;

        if let Some(ref mut dlss) = self.dlss {
            dlss.resize(render_width, render_height, display_width, display_height);
        }
        if let Some(ref mut xess) = self.xess {
            xess.resize(render_width, render_height, display_width, display_height);
        }
    }

    /// Execute the active upscaler.
    pub fn execute(
        &self,
        cmd: vk::CommandBuffer,
        color_input: vk::ImageView,
        depth_input: vk::ImageView,
        motion_vectors: vk::ImageView,
        output: vk::ImageView,
    ) {
        match self.active_backend {
            UpscalerBackend::Dlss => {
                if let Some(ref dlss) = self.dlss {
                    dlss.execute(cmd, color_input, depth_input, motion_vectors, output);
                }
            }
            UpscalerBackend::XeSS => {
                if let Some(ref xess) = self.xess {
                    xess.execute(cmd, color_input, depth_input, motion_vectors, output);
                }
            }
            UpscalerBackend::Fsr | UpscalerBackend::None => {
                // FSR is handled by PostProcessor, None does nothing
            }
        }
    }

    /// Shutdown and release resources.
    pub fn shutdown(&mut self) {
        if let Some(ref mut dlss) = self.dlss {
            dlss.shutdown();
        }
        if let Some(ref mut xess) = self.xess {
            xess.shutdown();
        }
    }
}

impl Default for UpscalerManager {
    fn default() -> Self {
        Self {
            active_backend: UpscalerBackend::Fsr,
            dlss: None,
            xess: None,
            quality: UpscalerQuality::Quality,
            render_width: 0,
            render_height: 0,
            display_width: 0,
            display_height: 0,
        }
    }
}
