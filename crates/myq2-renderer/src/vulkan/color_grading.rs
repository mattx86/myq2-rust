//! Color Grading with Look-Up Tables (LUTs)
//!
//! Implements LUT-based color correction for cinematic color grading:
//! - 3D LUT support for complex color transforms
//! - Multiple built-in presets (warm, cool, vintage, etc.)
//! - Runtime LUT blending/interpolation
//! - HDR-aware color grading

/// LUT size (typically 32x32x32 or 64x64x64).
pub const LUT_SIZE: usize = 32;

/// Number of elements in a 3D LUT.
pub const LUT_ELEMENTS: usize = LUT_SIZE * LUT_SIZE * LUT_SIZE;

/// RGB color.
#[derive(Debug, Clone, Copy, Default)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Color {
    pub fn new(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b }
    }

    pub fn lerp(&self, other: &Color, t: f32) -> Color {
        Color {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
        }
    }

    pub fn clamp(&self) -> Color {
        Color {
            r: self.r.clamp(0.0, 1.0),
            g: self.g.clamp(0.0, 1.0),
            b: self.b.clamp(0.0, 1.0),
        }
    }

    /// Convert to packed u32 (RGBA8).
    pub fn to_u32(&self) -> u32 {
        let r = (self.r.clamp(0.0, 1.0) * 255.0) as u32;
        let g = (self.g.clamp(0.0, 1.0) * 255.0) as u32;
        let b = (self.b.clamp(0.0, 1.0) * 255.0) as u32;
        r | (g << 8) | (b << 16) | (255 << 24)
    }
}

/// 3D LUT data.
pub struct Lut3D {
    /// LUT data (r,g,b indexed).
    pub data: Vec<Color>,
    /// LUT size per dimension.
    pub size: usize,
    /// LUT name.
    pub name: String,
}

impl Lut3D {
    /// Create identity LUT (no color change).
    pub fn identity() -> Self {
        let mut data = Vec::with_capacity(LUT_ELEMENTS);
        let size = LUT_SIZE;

        for b in 0..size {
            for g in 0..size {
                for r in 0..size {
                    data.push(Color::new(
                        r as f32 / (size - 1) as f32,
                        g as f32 / (size - 1) as f32,
                        b as f32 / (size - 1) as f32,
                    ));
                }
            }
        }

        Self {
            data,
            size,
            name: "Identity".to_string(),
        }
    }

    /// Sample the LUT with trilinear interpolation.
    pub fn sample(&self, color: &Color) -> Color {
        let size = self.size as f32;

        // Scale to LUT coordinates
        let r = color.r.clamp(0.0, 1.0) * (size - 1.0);
        let g = color.g.clamp(0.0, 1.0) * (size - 1.0);
        let b = color.b.clamp(0.0, 1.0) * (size - 1.0);

        // Integer and fractional parts
        let r0 = r.floor() as usize;
        let g0 = g.floor() as usize;
        let b0 = b.floor() as usize;
        let r1 = (r0 + 1).min(self.size - 1);
        let g1 = (g0 + 1).min(self.size - 1);
        let b1 = (b0 + 1).min(self.size - 1);

        let fr = r - r.floor();
        let fg = g - g.floor();
        let fb = b - b.floor();

        // Trilinear interpolation
        let c000 = self.get(r0, g0, b0);
        let c100 = self.get(r1, g0, b0);
        let c010 = self.get(r0, g1, b0);
        let c110 = self.get(r1, g1, b0);
        let c001 = self.get(r0, g0, b1);
        let c101 = self.get(r1, g0, b1);
        let c011 = self.get(r0, g1, b1);
        let c111 = self.get(r1, g1, b1);

        let c00 = c000.lerp(&c100, fr);
        let c10 = c010.lerp(&c110, fr);
        let c01 = c001.lerp(&c101, fr);
        let c11 = c011.lerp(&c111, fr);

        let c0 = c00.lerp(&c10, fg);
        let c1 = c01.lerp(&c11, fg);

        c0.lerp(&c1, fb)
    }

    /// Get color at integer coordinates.
    fn get(&self, r: usize, g: usize, b: usize) -> Color {
        let idx = r + g * self.size + b * self.size * self.size;
        self.data.get(idx).copied().unwrap_or_default()
    }

    /// Set color at integer coordinates.
    fn set(&mut self, r: usize, g: usize, b: usize, color: Color) {
        let idx = r + g * self.size + b * self.size * self.size;
        if idx < self.data.len() {
            self.data[idx] = color;
        }
    }

    /// Blend two LUTs.
    pub fn blend(a: &Lut3D, b: &Lut3D, t: f32) -> Self {
        let mut result = Lut3D::identity();
        result.name = format!("Blend({}, {})", a.name, b.name);

        for i in 0..result.data.len() {
            result.data[i] = a.data[i].lerp(&b.data[i], t);
        }

        result
    }

    /// Get raw data for GPU upload (RGBA8 format).
    pub fn to_rgba8(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(self.data.len() * 4);
        for color in &self.data {
            data.push((color.r.clamp(0.0, 1.0) * 255.0) as u8);
            data.push((color.g.clamp(0.0, 1.0) * 255.0) as u8);
            data.push((color.b.clamp(0.0, 1.0) * 255.0) as u8);
            data.push(255);
        }
        data
    }
}

/// Color grading presets.
pub mod presets {
    use super::*;

    /// Warm/sunset look.
    pub fn warm() -> Lut3D {
        let mut lut = Lut3D::identity();
        lut.name = "Warm".to_string();

        for b in 0..lut.size {
            for g in 0..lut.size {
                for r in 0..lut.size {
                    let mut c = lut.get(r, g, b);

                    // Boost reds/oranges, reduce blues
                    c.r = (c.r * 1.1 + 0.02).clamp(0.0, 1.0);
                    c.g = (c.g * 1.02).clamp(0.0, 1.0);
                    c.b = (c.b * 0.85).clamp(0.0, 1.0);

                    lut.set(r, g, b, c);
                }
            }
        }

        lut
    }

    /// Cool/blue look.
    pub fn cool() -> Lut3D {
        let mut lut = Lut3D::identity();
        lut.name = "Cool".to_string();

        for b in 0..lut.size {
            for g in 0..lut.size {
                for r in 0..lut.size {
                    let mut c = lut.get(r, g, b);

                    // Boost blues, reduce reds
                    c.r = (c.r * 0.9).clamp(0.0, 1.0);
                    c.g = (c.g * 0.95 + 0.02).clamp(0.0, 1.0);
                    c.b = (c.b * 1.15 + 0.03).clamp(0.0, 1.0);

                    lut.set(r, g, b, c);
                }
            }
        }

        lut
    }

    /// High contrast.
    pub fn high_contrast() -> Lut3D {
        let mut lut = Lut3D::identity();
        lut.name = "High Contrast".to_string();

        for b in 0..lut.size {
            for g in 0..lut.size {
                for r in 0..lut.size {
                    let c = lut.get(r, g, b);

                    // S-curve contrast
                    let contrast = |x: f32| -> f32 {
                        let a = 0.5;
                        let s = 2.0; // Contrast strength
                        a + (x - a) * s
                    };

                    lut.set(r, g, b, Color::new(
                        contrast(c.r).clamp(0.0, 1.0),
                        contrast(c.g).clamp(0.0, 1.0),
                        contrast(c.b).clamp(0.0, 1.0),
                    ));
                }
            }
        }

        lut
    }

    /// Vintage/faded look.
    pub fn vintage() -> Lut3D {
        let mut lut = Lut3D::identity();
        lut.name = "Vintage".to_string();

        for b in 0..lut.size {
            for g in 0..lut.size {
                for r in 0..lut.size {
                    let c = lut.get(r, g, b);

                    // Lift blacks, fade highlights, sepia tint
                    let mut new_c = Color::new(
                        c.r * 0.9 + 0.05,
                        c.g * 0.85 + 0.05,
                        c.b * 0.75 + 0.05,
                    );

                    // Reduce saturation
                    let luma = new_c.r * 0.299 + new_c.g * 0.587 + new_c.b * 0.114;
                    new_c = new_c.lerp(&Color::new(luma, luma, luma), 0.3);

                    lut.set(r, g, b, new_c.clamp());
                }
            }
        }

        lut
    }

    /// Noir/black and white with high contrast.
    pub fn noir() -> Lut3D {
        let mut lut = Lut3D::identity();
        lut.name = "Noir".to_string();

        for b in 0..lut.size {
            for g in 0..lut.size {
                for r in 0..lut.size {
                    let c = lut.get(r, g, b);

                    // Convert to B&W
                    let luma = c.r * 0.299 + c.g * 0.587 + c.b * 0.114;

                    // High contrast S-curve
                    let contrast = if luma < 0.5 {
                        2.0 * luma * luma
                    } else {
                        1.0 - 2.0 * (1.0 - luma).powi(2)
                    };

                    lut.set(r, g, b, Color::new(contrast, contrast, contrast));
                }
            }
        }

        lut
    }

    /// Teal and orange (cinematic look).
    pub fn teal_orange() -> Lut3D {
        let mut lut = Lut3D::identity();
        lut.name = "Teal & Orange".to_string();

        for b in 0..lut.size {
            for g in 0..lut.size {
                for r in 0..lut.size {
                    let c = lut.get(r, g, b);

                    // Get luminance
                    let luma = c.r * 0.299 + c.g * 0.587 + c.b * 0.114;

                    // Skin tones go orange, everything else goes teal
                    let skin_mask = ((c.r - c.g).abs() < 0.2 && c.r > c.b) as i32 as f32;

                    let orange = Color::new(
                        (luma * 1.2 + 0.1).clamp(0.0, 1.0),
                        (luma * 0.8).clamp(0.0, 1.0),
                        (luma * 0.4).clamp(0.0, 1.0),
                    );

                    let teal = Color::new(
                        (luma * 0.5).clamp(0.0, 1.0),
                        (luma * 0.9).clamp(0.0, 1.0),
                        (luma * 1.1 + 0.05).clamp(0.0, 1.0),
                    );

                    let result = orange.lerp(&teal, 1.0 - skin_mask * 0.5);
                    lut.set(r, g, b, result);
                }
            }
        }

        lut
    }

    /// Cyberpunk neon look.
    pub fn cyberpunk() -> Lut3D {
        let mut lut = Lut3D::identity();
        lut.name = "Cyberpunk".to_string();

        for b in 0..lut.size {
            for g in 0..lut.size {
                for r in 0..lut.size {
                    let c = lut.get(r, g, b);

                    // Push magentas and cyans
                    let new_c = Color::new(
                        (c.r * 1.1 + c.b * 0.2).clamp(0.0, 1.0),
                        (c.g * 0.8).clamp(0.0, 1.0),
                        (c.b * 1.2 + c.r * 0.1).clamp(0.0, 1.0),
                    );

                    lut.set(r, g, b, new_c);
                }
            }
        }

        lut
    }
}

/// GLSL shader code for LUT sampling.
pub mod shader {
    /// Fragment shader code for 3D LUT sampling.
    pub const LUT_SAMPLE_FUNCTION: &str = r#"
// Sample a 3D LUT texture
vec3 sampleLUT(sampler3D lut, vec3 color) {
    // Clamp to valid range
    color = clamp(color, 0.0, 1.0);

    // Scale to LUT coordinates (accounting for texel centers)
    float lutSize = float(textureSize(lut, 0).x);
    vec3 scale = (lutSize - 1.0) / lutSize;
    vec3 offset = 0.5 / lutSize;

    return texture(lut, color * scale + offset).rgb;
}

// Apply LUT with blend
vec3 applyLUT(sampler3D lut, vec3 color, float intensity) {
    vec3 graded = sampleLUT(lut, color);
    return mix(color, graded, intensity);
}
"#;

    /// Simple post-process shader using LUT.
    pub const POST_PROCESS_SHADER: &str = r#"
#version 450

layout(binding = 0) uniform sampler2D inputTexture;
layout(binding = 1) uniform sampler3D colorLUT;

layout(push_constant) uniform PushConstants {
    float lutIntensity;
    float exposure;
    float gamma;
} pc;

layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 fragColor;

void main() {
    vec3 color = texture(inputTexture, uv).rgb;

    // Apply exposure
    color *= pc.exposure;

    // Apply LUT
    vec3 graded = texture(colorLUT, color).rgb;
    color = mix(color, graded, pc.lutIntensity);

    // Apply gamma
    color = pow(color, vec3(1.0 / pc.gamma));

    fragColor = vec4(color, 1.0);
}
"#;
}

/// Color grading manager.
pub struct ColorGradingManager {
    /// Current LUT.
    current_lut: Lut3D,
    /// Target LUT for blending.
    target_lut: Option<Lut3D>,
    /// Blend progress (0-1).
    blend_progress: f32,
    /// Blend speed (per second).
    blend_speed: f32,
    /// LUT intensity (0-1).
    intensity: f32,
    /// Exposure adjustment.
    exposure: f32,
    /// Gamma adjustment.
    gamma: f32,
    /// Whether color grading is enabled.
    enabled: bool,
}

impl ColorGradingManager {
    /// Create a new color grading manager.
    pub fn new() -> Self {
        Self {
            current_lut: Lut3D::identity(),
            target_lut: None,
            blend_progress: 1.0,
            blend_speed: 2.0,
            intensity: 1.0,
            exposure: 1.0,
            gamma: 2.2,
            enabled: true,
        }
    }

    /// Set the current LUT immediately.
    pub fn set_lut(&mut self, lut: Lut3D) {
        self.current_lut = lut;
        self.target_lut = None;
        self.blend_progress = 1.0;
    }

    /// Transition to a new LUT.
    pub fn transition_to(&mut self, lut: Lut3D, duration: f32) {
        self.target_lut = Some(lut);
        self.blend_progress = 0.0;
        self.blend_speed = 1.0 / duration.max(0.01);
    }

    /// Update blending.
    pub fn update(&mut self, dt: f32) {
        if let Some(ref target) = self.target_lut {
            self.blend_progress += self.blend_speed * dt;

            if self.blend_progress >= 1.0 {
                self.blend_progress = 1.0;
                self.current_lut = Lut3D::blend(&self.current_lut, target, 1.0);
                self.target_lut = None;
            }
        }
    }

    /// Get the effective LUT for rendering.
    pub fn get_effective_lut(&self) -> Lut3D {
        if let Some(ref target) = self.target_lut {
            Lut3D::blend(&self.current_lut, target, self.blend_progress)
        } else {
            // Clone identity if no grading
            if self.intensity <= 0.0 {
                Lut3D::identity()
            } else {
                Lut3D::blend(&Lut3D::identity(), &self.current_lut, self.intensity)
            }
        }
    }

    /// Set intensity.
    pub fn set_intensity(&mut self, intensity: f32) {
        self.intensity = intensity.clamp(0.0, 1.0);
    }

    /// Set exposure.
    pub fn set_exposure(&mut self, exposure: f32) {
        self.exposure = exposure.max(0.0);
    }

    /// Set gamma.
    pub fn set_gamma(&mut self, gamma: f32) {
        self.gamma = gamma.clamp(1.0, 3.0);
    }

    /// Enable/disable.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Get current settings.
    pub fn get_settings(&self) -> (f32, f32, f32) {
        (self.intensity, self.exposure, self.gamma)
    }
}

impl Default for ColorGradingManager {
    fn default() -> Self {
        Self::new()
    }
}
