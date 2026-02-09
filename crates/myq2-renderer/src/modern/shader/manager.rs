//! Shader program manager
//!
//! Manages loading, caching, and accessing shader programs by type.

use super::ShaderProgram;
use crate::modern::RenderError;
use std::collections::HashMap;

/// Types of shaders used by the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderType {
    /// BSP world surfaces with lightmaps.
    World,
    /// BSP surfaces with flowing/scrolling textures.
    WorldFlowing,
    /// Water and other liquid surfaces with warp effect.
    Water,
    /// Alias (MD2) models with per-vertex lighting and frame lerping.
    Alias,
    /// Alias models with cel-shading/toon effect.
    AliasCel,
    /// Skybox rendering.
    Sky,
    /// Particle rendering (billboards).
    Particle,
    /// 2D UI elements (console, menus, HUD).
    Ui,
    /// Dynamic light overlays.
    DynamicLight,
    /// Post-processing effects.
    PostProcess,
    /// FXAA anti-aliasing.
    Fxaa,
    /// Screen-space ambient occlusion.
    Ssao,
    /// SSAO bilateral blur.
    SsaoBlur,
    /// Bloom bright-pass extraction.
    BloomExtract,
    /// Bloom Gaussian blur (separable).
    BloomBlur,
    /// Bloom composite.
    BloomComposite,
    /// FSR 1.0 EASU (Edge Adaptive Spatial Upsampling).
    FsrEasu,
    /// FSR 1.0 RCAS (Robust Contrast Adaptive Sharpening).
    FsrRcas,
    /// FSR 2.0 temporal upscaling.
    Fsr2Temporal,
    /// Motion vectors generation for temporal effects.
    MotionVectors,
}

/// Manages all shader programs.
pub struct ShaderManager {
    programs: HashMap<ShaderType, ShaderProgram>,
}

impl ShaderManager {
    /// Create a new shader manager and compile all shaders.
    pub fn new() -> Result<Self, RenderError> {
        let mut manager = Self {
            programs: HashMap::new(),
        };
        manager.load_all_shaders()?;
        Ok(manager)
    }

    /// Load and compile all shader programs.
    fn load_all_shaders(&mut self) -> Result<(), RenderError> {
        // World shader
        self.programs.insert(
            ShaderType::World,
            ShaderProgram::from_source(WORLD_VERT, WORLD_FRAG)?,
        );

        // World flowing shader (same vertex, different uniform usage)
        self.programs.insert(
            ShaderType::WorldFlowing,
            ShaderProgram::from_source(WORLD_VERT, WORLD_FRAG)?,
        );

        // Water shader
        self.programs.insert(
            ShaderType::Water,
            ShaderProgram::from_source(WATER_VERT, WATER_FRAG)?,
        );

        // Alias model shader
        self.programs.insert(
            ShaderType::Alias,
            ShaderProgram::from_source(ALIAS_VERT, ALIAS_FRAG)?,
        );

        // Alias cel-shading shader
        self.programs.insert(
            ShaderType::AliasCel,
            ShaderProgram::from_source(ALIAS_CEL_VERT, ALIAS_CEL_FRAG)?,
        );

        // Sky shader
        self.programs.insert(
            ShaderType::Sky,
            ShaderProgram::from_source(SKY_VERT, SKY_FRAG)?,
        );

        // Particle shader
        self.programs.insert(
            ShaderType::Particle,
            ShaderProgram::from_source(PARTICLE_VERT, PARTICLE_FRAG)?,
        );

        // UI shader
        self.programs.insert(
            ShaderType::Ui,
            ShaderProgram::from_source(UI_VERT, UI_FRAG)?,
        );

        // Dynamic light shader
        self.programs.insert(
            ShaderType::DynamicLight,
            ShaderProgram::from_source(DLIGHT_VERT, DLIGHT_FRAG)?,
        );

        // Post-process shader
        self.programs.insert(
            ShaderType::PostProcess,
            ShaderProgram::from_source(POSTPROCESS_VERT, POSTPROCESS_FRAG)?,
        );

        // FXAA anti-aliasing shader
        self.programs.insert(
            ShaderType::Fxaa,
            ShaderProgram::from_source(POSTPROCESS_VERT, FXAA_FRAG)?,
        );

        // SSAO generation shader
        self.programs.insert(
            ShaderType::Ssao,
            ShaderProgram::from_source(POSTPROCESS_VERT, SSAO_FRAG)?,
        );

        // SSAO blur shader
        self.programs.insert(
            ShaderType::SsaoBlur,
            ShaderProgram::from_source(POSTPROCESS_VERT, SSAO_BLUR_FRAG)?,
        );

        // Bloom bright-pass extraction shader
        self.programs.insert(
            ShaderType::BloomExtract,
            ShaderProgram::from_source(POSTPROCESS_VERT, BLOOM_EXTRACT_FRAG)?,
        );

        // Bloom Gaussian blur shader
        self.programs.insert(
            ShaderType::BloomBlur,
            ShaderProgram::from_source(POSTPROCESS_VERT, BLOOM_BLUR_FRAG)?,
        );

        // Bloom composite shader
        self.programs.insert(
            ShaderType::BloomComposite,
            ShaderProgram::from_source(POSTPROCESS_VERT, BLOOM_COMPOSITE_FRAG)?,
        );

        // FSR 1.0 EASU upscale shader
        self.programs.insert(
            ShaderType::FsrEasu,
            ShaderProgram::from_source(POSTPROCESS_VERT, FSR_EASU_FRAG)?,
        );

        // FSR 1.0 RCAS sharpening shader
        self.programs.insert(
            ShaderType::FsrRcas,
            ShaderProgram::from_source(POSTPROCESS_VERT, FSR_RCAS_FRAG)?,
        );

        // FSR 2.0 temporal upscaling shader
        self.programs.insert(
            ShaderType::Fsr2Temporal,
            ShaderProgram::from_source(POSTPROCESS_VERT, FSR2_TEMPORAL_FRAG)?,
        );

        // Motion vectors generation shader
        self.programs.insert(
            ShaderType::MotionVectors,
            ShaderProgram::from_source(MOTION_VECTORS_VERT, MOTION_VECTORS_FRAG)?,
        );

        Ok(())
    }

    /// Get a shader program by type.
    pub fn get(&self, shader_type: ShaderType) -> Option<&ShaderProgram> {
        self.programs.get(&shader_type)
    }

    /// Get a mutable shader program by type.
    pub fn get_mut(&mut self, shader_type: ShaderType) -> Option<&mut ShaderProgram> {
        self.programs.get_mut(&shader_type)
    }

    /// Reload all shaders (useful for development hot-reload).
    pub fn reload_all(&mut self) -> Result<(), RenderError> {
        self.programs.clear();
        self.load_all_shaders()
    }
}

// ============================================================================
// Shader Sources (loaded from external files)
// ============================================================================

const WORLD_VERT: &str = include_str!("../../../shaders/world.vert.glsl");
const WORLD_FRAG: &str = include_str!("../../../shaders/world.frag.glsl");
const WATER_VERT: &str = include_str!("../../../shaders/water.vert.glsl");
const WATER_FRAG: &str = include_str!("../../../shaders/water.frag.glsl");
const ALIAS_VERT: &str = include_str!("../../../shaders/alias.vert.glsl");
const ALIAS_FRAG: &str = include_str!("../../../shaders/alias.frag.glsl");
const ALIAS_CEL_VERT: &str = include_str!("../../../shaders/alias_cel.vert.glsl");
const ALIAS_CEL_FRAG: &str = include_str!("../../../shaders/alias_cel.frag.glsl");
const SKY_VERT: &str = include_str!("../../../shaders/sky.vert.glsl");
const SKY_FRAG: &str = include_str!("../../../shaders/sky.frag.glsl");
const PARTICLE_VERT: &str = include_str!("../../../shaders/particle.vert.glsl");
const PARTICLE_FRAG: &str = include_str!("../../../shaders/particle.frag.glsl");
const UI_VERT: &str = include_str!("../../../shaders/ui.vert.glsl");
const UI_FRAG: &str = include_str!("../../../shaders/ui.frag.glsl");
const DLIGHT_VERT: &str = include_str!("../../../shaders/dlight.vert.glsl");
const DLIGHT_FRAG: &str = include_str!("../../../shaders/dlight.frag.glsl");
const POSTPROCESS_VERT: &str = include_str!("../../../shaders/postprocess.vert.glsl");
const POSTPROCESS_FRAG: &str = include_str!("../../../shaders/postprocess.frag.glsl");
const FXAA_FRAG: &str = include_str!("../../../shaders/fxaa.frag.glsl");
const SSAO_FRAG: &str = include_str!("../../../shaders/ssao.frag.glsl");
const SSAO_BLUR_FRAG: &str = include_str!("../../../shaders/ssao_blur.frag.glsl");
const BLOOM_EXTRACT_FRAG: &str = include_str!("../../../shaders/bloom_extract.frag.glsl");
const BLOOM_BLUR_FRAG: &str = include_str!("../../../shaders/bloom_blur.frag.glsl");
const BLOOM_COMPOSITE_FRAG: &str = include_str!("../../../shaders/bloom_composite.frag.glsl");
const FSR_EASU_FRAG: &str = include_str!("../../../shaders/fsr_easu.frag.glsl");
const FSR_RCAS_FRAG: &str = include_str!("../../../shaders/fsr_rcas.frag.glsl");
const FSR2_TEMPORAL_FRAG: &str = include_str!("../../../shaders/fsr2_temporal.frag.glsl");
const MOTION_VECTORS_VERT: &str = include_str!("../../../shaders/motion_vectors.vert.glsl");
const MOTION_VECTORS_FRAG: &str = include_str!("../../../shaders/motion_vectors.frag.glsl");
