//! Water reflection/refraction FBO
//!
//! Framebuffer for water effects like reflections and underwater distortion.

use super::RenderTarget;

/// Water effect framebuffers.
pub struct WaterFbo {
    /// Reflection render target.
    reflection: Option<RenderTarget>,
    /// Refraction render target.
    refraction: Option<RenderTarget>,
    /// Whether water effects are enabled.
    enabled: bool,
}

impl WaterFbo {
    /// Create water FBOs.
    pub fn new(width: u32, height: u32, enabled: bool) -> Self {
        let (reflection, refraction) = if enabled {
            // Use half resolution for performance
            let w = width / 2;
            let h = height / 2;
            (
                Some(RenderTarget::new(w, h, true)),
                Some(RenderTarget::new(w, h, true)),
            )
        } else {
            (None, None)
        };

        Self {
            reflection,
            refraction,
            enabled,
        }
    }

    /// Enable or disable water effects.
    pub fn set_enabled(&mut self, enabled: bool, width: u32, height: u32) {
        if enabled == self.enabled {
            return;
        }

        self.enabled = enabled;
        if enabled {
            let w = width / 2;
            let h = height / 2;
            self.reflection = Some(RenderTarget::new(w, h, true));
            self.refraction = Some(RenderTarget::new(w, h, true));
        } else {
            self.reflection = None;
            self.refraction = None;
        }
    }

    /// Resize FBOs.
    pub fn resize(&mut self, width: u32, height: u32) {
        if !self.enabled {
            return;
        }

        let w = width / 2;
        let h = height / 2;

        if let Some(ref mut r) = self.reflection {
            r.resize(w, h);
        }
        if let Some(ref mut r) = self.refraction {
            r.resize(w, h);
        }
    }

    /// Bind reflection FBO for rendering.
    pub fn bind_reflection(&self) {
        if let Some(ref r) = self.reflection {
            r.bind();
        }
    }

    /// Bind refraction FBO for rendering.
    pub fn bind_refraction(&self) {
        if let Some(ref r) = self.refraction {
            r.bind();
        }
    }

    /// Unbind to default framebuffer.
    pub fn unbind(&self) {
        RenderTarget::unbind();
    }

    /// Bind reflection texture for sampling.
    pub fn bind_reflection_texture(&self, unit: u32) {
        if let Some(ref r) = self.reflection {
            r.bind_color_texture(unit);
        }
    }

    /// Bind refraction texture for sampling.
    pub fn bind_refraction_texture(&self, unit: u32) {
        if let Some(ref r) = self.refraction {
            r.bind_color_texture(unit);
        }
    }

    /// Check if water effects are enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for WaterFbo {
    fn default() -> Self {
        Self {
            reflection: None,
            refraction: None,
            enabled: false,
        }
    }
}
