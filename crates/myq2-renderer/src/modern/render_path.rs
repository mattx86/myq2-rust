//! Modern render path implementation (Vulkan)
//!
//! VBO/shader-based rendering using Vulkan command buffers and render passes.
//! Supports parallel command buffer recording for multi-threaded rendering.

use ash::vk;
use super::{RenderError, RenderPath, FrameParams, ParticleData};
use super::shader::{ShaderManager, ShaderType, PerFrameUniforms, PerObjectUniforms, UniformBuffer, PipelineManager, PipelineVariant, PostProcessUniforms};
use super::geometry::{BspGeometryManager, BspVertex, AliasModelManager, AliasInstance, InstancedAliasRenderer, ParticleManager, Draw2DManager, BlendMode, VertexBuffer, IndexBuffer};
use super::texture::LightmapArray;
use super::framebuffer::{WaterFbo, PostProcessor};
use crate::vk_rmain::EntityLocal;
use crate::modern::gpu_device;
use crate::vulkan::dynamic_state3::DynamicState3Commands;
use std::f32::consts::PI;

/// Queued brush model draw data (doors, platforms, etc.).
struct BrushModelDraw {
    /// Model-view-projection matrix (column-major flat).
    mvp: [f32; 16],
    /// First surface index in the BSP surface array.
    first_surface: usize,
    /// Number of surfaces for this brush model.
    num_surfaces: usize,
    /// Dynamic relight block pushed at offset 92..128:
    /// [use_dyn, c0, c1, c2, c3, min.x, min.y, invSize.x, invSize.y].
    /// c0..c3 are the world light at the 4 footprint corners, packed RGB-in-a-float;
    /// min/invSize are the authored footprint XY bounds for in-shader bilinear interp.
    /// use_dyn=1.0 only for movers that have LEFT their authored position; static inline
    /// models keep use_dyn=0 and their baked lightmap.
    dyn_block: [f32; 9],
    /// Model matrix (model->world) and world-space centre, for casting the mover's shadow
    /// into the directional shadow map (moved movers only).
    model_matrix: [[f32; 4]; 4],
    center: [f32; 3],
}

/// Push constants for alias model rendering (128 bytes total).
#[repr(C)]
struct AliasPushConstants {
    mvp: [f32; 16],           // 64 bytes
    shade_light: [f32; 3],    // 12 bytes
    alpha: f32,               // 4 bytes
    move_vec: [f32; 3],       // 12 bytes
    backlerp: f32,            // 4 bytes
    front_v: [f32; 3],        // 12 bytes
    shell_scale: f32,         // 4 bytes
    back_v: [f32; 3],         // 12 bytes
    is_shell: i32,            // 4 bytes
}

/// Queued sprite draw data (billboard quads).
struct SpriteDrawData {
    /// Pre-computed billboard quad vertices (in world space).
    vertices: [BspVertex; 4],
    /// Texture ID for the sprite frame.
    texture_id: u32,
    /// Alpha value (1.0 for opaque, <1.0 for RF_TRANSLUCENT).
    alpha: f32,
}

/// Push constants for dynamic light rendering (96 bytes total).
#[repr(C)]
struct DlightPushConstants {
    mvp: [f32; 16],           // 64 bytes
    light_origin: [f32; 3],   // 12 bytes
    light_radius: f32,        // 4 bytes
    light_color: [f32; 3],    // 12 bytes
    _pad: f32,                // 4 bytes
}

/// A D3-style (Doom 3) point light for per-pixel additive lighting.
#[repr(C)]
#[derive(Clone, Debug, Default)]
pub struct GpuLightD3 {
    pub pos: [f32; 3],
    pub radius: f32,
    pub color: [f32; 3],
    pub intensity: f32,
    pub specular: [f32; 3],
    pub spec_pow: f32,
}

/// Push constants for the D3 lit pass (world_lit.vert/frag).
/// Total: 124 bytes (within Vulkan's 128-byte minimum guarantee).
#[repr(C)]
#[repr(C)]
struct D3LitPushConstants {
    mvp: [f32; 16],           // 64 bytes
    light_pos: [f32; 3],      // 12 bytes
    light_radius: f32,        // 4 bytes
    light_color: [f32; 3],    // 12 bytes
    light_intensity: f32,     // 4 bytes
    spec_power: f32,          // 4 bytes
    shadow_bias: f32,         // 4 bytes  (was _pad0 in spec)
    _pad1: f32,               // 4 bytes
    _pad2: f32,               // 4 bytes
    view_origin: [f32; 3],    // 12 bytes
}

/// Push constants for the shadow cubemap depth pass (shadow_cube.vert/frag).
/// Total: 80 bytes.
#[repr(C)]
struct ShadowCubePushConstants {
    mvp: [f32; 16],           // 64 bytes
    light_pos: [f32; 3],      // 12 bytes
    light_radius: f32,        // 4 bytes
}

/// A pre-computed shadow cubemap for a single D3 point light.
///
/// The cubemap is rendered once at level load and sampled each frame
/// to determine per-fragment occlusion from the light.
///
/// Format: R32_SFLOAT (colour attachment). Stores `dist/radius` for the
/// nearest opaque surface in each direction. Sampling with a regular
/// `samplerCube` is unambiguous — avoids depth-format sampling issues.
pub struct ShadowCubemap {
    /// R32_SFLOAT colour cubemap image (6 layers) — stores dist/radius.
    pub image: vk::Image,
    /// Device memory backing the colour cubemap.
    pub memory: vk::DeviceMemory,
    /// Cube image view (TYPE_CUBE, for shader sampling at set=2).
    pub cube_view: vk::ImageView,
    /// Per-face colour image views (TYPE_2D, for framebuffer colour attachment).
    pub face_views: [vk::ImageView; 6],
    /// Temporary D32_SFLOAT depth image used only during shadow-map construction.
    /// Kept alive so the framebuffers remain valid until explicitly destroyed.
    pub depth_image: vk::Image,
    /// Device memory backing the depth image.
    pub depth_memory: vk::DeviceMemory,
    /// Depth image view (TYPE_2D, for framebuffer depth attachment).
    pub depth_view: vk::ImageView,
    /// Per-face framebuffers: attachment[0] = face colour view, attachment[1] = depth view.
    pub framebuffers: [vk::Framebuffer; 6],
    /// Descriptor set at set=2 binding cube_view as a samplerCube.
    pub descriptor_set: vk::DescriptorSet,
    /// Shadow map resolution (pixels per face).
    pub resolution: u32,
}

// SAFETY: ShadowCubemap contains only Vulkan handles (opaque integers).
// All access is serialized by the surrounding Mutex on ModernRenderPath.
unsafe impl Send for ShadowCubemap {}
unsafe impl Sync for ShadowCubemap {}

/// Modern VBO/shader-based render path.
pub struct ModernRenderPath {
    /// Shader manager.
    shaders: Option<ShaderManager>,
    /// BSP world geometry.
    bsp_geometry: BspGeometryManager,
    /// Alias model buffers.
    alias_models: AliasModelManager,
    /// Particle manager.
    particles: ParticleManager,
    /// 2D drawing manager.
    draw2d: Draw2DManager,
    /// Lightmap texture array.
    lightmap_array: LightmapArray,
    /// Water effect FBOs.
    water_fbo: WaterFbo,
    /// Post-processor.
    post_processor: Option<PostProcessor>,
    /// Per-frame uniform buffer.
    per_frame_ubo: Option<UniformBuffer<PerFrameUniforms>>,
    /// Per-object uniform buffer.
    per_object_ubo: Option<UniformBuffer<PerObjectUniforms>>,
    /// Current per-frame uniforms.
    frame_uniforms: PerFrameUniforms,
    /// Initialized flag.
    initialized: bool,
    /// Screen width.
    width: u32,
    /// Screen height.
    height: u32,
    /// Cinematic texture (Vulkan image).
    cinematic_texture: Option<vk::Image>,
    /// Cinematic texture view.
    cinematic_image_view: Option<vk::ImageView>,
    /// Cinematic texture memory.
    cinematic_memory: Option<vk::DeviceMemory>,
    /// Cinematic texture sampler.
    cinematic_sampler: Option<vk::Sampler>,
    /// Cinematic texture ID used for 2D batching (compatibility stub).
    cinematic_texture_id: u32,
    /// Current frame command buffer (set during begin_frame).
    current_command_buffer: Option<vk::CommandBuffer>,
    /// Current swapchain frame index.
    current_frame_index: usize,
    /// Whether we successfully acquired a swapchain image this frame.
    frame_in_progress: bool,
    /// Dynamic state 3 commands for vk_showtris wireframe (None if EDS3 not supported).
    dynamic_state3: Option<DynamicState3Commands>,
    /// Instanced alias model renderer — collects per-entity instances each frame.
    alias_instanced: InstancedAliasRenderer,
    /// Vulkan graphics pipeline manager for real draw commands.
    pipelines: Option<PipelineManager>,
    /// Whether 3D scene rendering was performed this frame.
    scene_rendered: bool,
    /// Queued brush model draws for the current frame.
    brush_models: Vec<BrushModelDraw>,
    /// Per-mover smoothed dynamic light (keyed by model pointer), eased toward the
    /// freshly sampled value each frame so a mover's brightness ramps smoothly instead
    /// of snapping when it crosses a lighting boundary. See draw_brush_model().
    /// One smoothed light per footprint corner ([(min,min),(max,min),(min,max),(max,max)]).
    mover_light: std::collections::HashMap<usize, [[f32; 3]; 4]>,
    /// Viewer position at the last D3-light reselection — limits how often the active
    /// (nearest-N) light set is recomputed, so the shadow cubemaps aren't rebuilt per frame.
    last_light_select_pos: [f32; 3],
    /// Sorted static-light indices currently active, to detect when the set actually changes.
    last_selected_lights: Vec<usize>,
    /// Queued sprite draws for the current frame.
    sprite_draws: Vec<SpriteDrawData>,
    /// Sprite VBO (temporary, uploaded each frame).
    sprite_vbo: VertexBuffer,
    /// Sprite IBO (temporary, uploaded each frame).
    sprite_ibo: IndexBuffer,
    /// Sky face vertices (generated each frame by draw_sky).
    sky_vertices: Vec<BspVertex>,
    /// Sky face indices (generated each frame by draw_sky).
    sky_indices: Vec<u32>,
    /// Sky face draw info: (texture_id, first_index, index_count).
    sky_face_draws: Vec<(u32, u32, u32)>,
    /// Sky model-view-projection matrix.
    sky_mvp: [f32; 16],
    /// Sky VBO (temporary, uploaded each frame).
    sky_vbo: VertexBuffer,
    /// Sky IBO (temporary, uploaded each frame).
    sky_ibo: IndexBuffer,
    /// Dynamic light vertices (pos3 + color3, generated each frame).
    dlight_vertices: Vec<[f32; 6]>,
    /// Dynamic light indices (generated each frame).
    dlight_indices: Vec<u32>,
    /// Dynamic light push constants (one per light).
    dlight_draws: Vec<DlightPushConstants>,
    /// Dynamic light VBO (temporary, uploaded each frame).
    dlight_vbo: VertexBuffer,
    /// Dynamic light IBO (temporary, uploaded each frame).
    dlight_ibo: IndexBuffer,
    /// Lightmap texture array descriptor set (set 2, binding 0).
    lightmap_descriptor_set: Option<vk::DescriptorSet>,
    /// Descriptor pool for the lightmap descriptor set.
    lightmap_descriptor_pool: Option<vk::DescriptorPool>,
    /// D3 point lights for per-pixel additive lighting.
    d3_lights: Vec<GpuLightD3>,
    /// Pre-computed shadow cubemaps, one per D3 light (built at level load).
    shadow_cubemaps: Vec<ShadowCubemap>,
    /// Depth-only render pass for shadow cubemap rendering (created once).
    shadow_render_pass: Option<vk::RenderPass>,
    /// Descriptor pool for shadow cubemap descriptor sets.
    shadow_descriptor_pool: Option<vk::DescriptorPool>,
    /// Sampler for shadow cubemap sampling.
    shadow_sampler: Option<vk::Sampler>,
    /// Projective dynamic-shadow resources (directional shadow map + resolve pass).
    projective_shadow: super::shadow_project::ProjectiveShadow,
    /// VXGI debug raymarch: descriptor pool + set for the voxel grid sampler.
    vxgi_debug_pool: Option<vk::DescriptorPool>,
    vxgi_debug_set: Option<vk::DescriptorSet>,
    /// VXGI GI pass: descriptor pool + set (depth + radiance + albedo + dlight UBO).
    vxgi_gi_pool: Option<vk::DescriptorPool>,
    vxgi_gi_set: Option<vk::DescriptorSet>,
    /// Host-visible UBO holding the frame's dynamic lights for GI bounce.
    vxgi_gi_dlight_buf: Option<vk::Buffer>,
    vxgi_gi_dlight_mem: Option<vk::DeviceMemory>,
    /// Descriptor (set 4) binding the VXGI irradiance volume into the world shader.
    vxgi_world_irr_pool: Option<vk::DescriptorPool>,
    vxgi_world_irr_set: Option<vk::DescriptorSet>,
    /// This frame's irradiance params for the world push: (grid_min, extent, gi_scale).
    frame_irr_params: Option<([f32; 3], f32, f32)>,
    /// The irradiance view the descriptor currently points at (recreate the set when it changes,
    /// e.g. on map reload — updating an in-flight descriptor in place is ignored by the driver).
    vxgi_irr_view: Option<vk::ImageView>,
    /// View origin captured this frame (focus point for the directional shadow map).
    frame_vieworg: [f32; 3],
    /// Planar water reflection: half-res mirrored world+sky render target.
    refl_target: Option<super::framebuffer::RenderTarget>,
    /// Descriptor (set 3 of the water pipeline) binding the reflection texture.
    refl_desc_pool: Option<vk::DescriptorPool>,
    refl_desc_set: Option<vk::DescriptorSet>,
    /// The reflection view the descriptor points at (recreate the set when it changes).
    refl_desc_view: Option<vk::ImageView>,
    /// Sky model matrix (translate to camera + sky rotation), flat column-major. Stored so the
    /// mirrored reflection pass can rebuild the sky MVP with the mirrored view-projection.
    sky_model_flat: [f32; 16],
    /// This frame's active water plane z (reflection plane), if any — drives the shimmer pass.
    frame_refl_plane: Option<f32>,
    /// Water shimmer pass: descriptor pool + set (scene depth + irradiance volume).
    shimmer_pool: Option<vk::DescriptorPool>,
    shimmer_set: Option<vk::DescriptorSet>,
}

impl ModernRenderPath {
    /// Create a new modern render path (uninitialized).
    pub fn new() -> Self {
        myq2_common::common::com_printf("ModernRenderPath::new: Creating BSP geometry manager\n");
        let bsp_geometry = BspGeometryManager::new();
        myq2_common::common::com_printf("ModernRenderPath::new: Creating alias models manager\n");
        let alias_models = AliasModelManager::new();
        myq2_common::common::com_printf("ModernRenderPath::new: Creating particles manager\n");
        let particles = ParticleManager::new();
        myq2_common::common::com_printf("ModernRenderPath::new: Creating draw2d manager\n");
        let draw2d = Draw2DManager::new();
        myq2_common::common::com_printf("ModernRenderPath::new: Creating lightmap array\n");
        let lightmap_array = LightmapArray::new();
        myq2_common::common::com_printf("ModernRenderPath::new: Creating instanced alias renderer\n");
        let alias_instanced = InstancedAliasRenderer::new();
        myq2_common::common::com_printf("ModernRenderPath::new: All managers created, building struct\n");

        Self {
            shaders: None,
            bsp_geometry,
            alias_models,
            particles,
            draw2d,
            lightmap_array,
            water_fbo: WaterFbo::default(),
            post_processor: None,
            per_frame_ubo: None,
            per_object_ubo: None,
            frame_uniforms: PerFrameUniforms::default(),
            initialized: false,
            width: 640,
            height: 480,
            cinematic_texture: None,
            cinematic_image_view: None,
            cinematic_memory: None,
            cinematic_sampler: None,
            cinematic_texture_id: 0,
            current_command_buffer: None,
            current_frame_index: 0,
            frame_in_progress: false,
            dynamic_state3: None,
            alias_instanced,
            pipelines: None,
            scene_rendered: false,
            brush_models: Vec::new(),
            mover_light: std::collections::HashMap::new(),
            last_light_select_pos: [f32::MAX, f32::MAX, f32::MAX],
            last_selected_lights: Vec::new(),
            sprite_draws: Vec::new(),
            sprite_vbo: VertexBuffer::new(),
            sprite_ibo: IndexBuffer::new(),
            sky_vertices: Vec::new(),
            sky_indices: Vec::new(),
            sky_face_draws: Vec::new(),
            sky_mvp: [0.0; 16],
            sky_vbo: VertexBuffer::new(),
            sky_ibo: IndexBuffer::new(),
            dlight_vertices: Vec::new(),
            dlight_indices: Vec::new(),
            dlight_draws: Vec::new(),
            dlight_vbo: VertexBuffer::new(),
            dlight_ibo: IndexBuffer::new(),
            lightmap_descriptor_set: None,
            lightmap_descriptor_pool: None,
            d3_lights: Vec::new(),
            shadow_cubemaps: Vec::new(),
            shadow_render_pass: None,
            shadow_descriptor_pool: None,
            shadow_sampler: None,
            projective_shadow: super::shadow_project::ProjectiveShadow::default(),
            vxgi_debug_pool: None,
            vxgi_debug_set: None,
            vxgi_gi_pool: None,
            vxgi_gi_set: None,
            vxgi_gi_dlight_buf: None,
            vxgi_gi_dlight_mem: None,
            vxgi_world_irr_pool: None,
            vxgi_world_irr_set: None,
            frame_irr_params: None,
            vxgi_irr_view: None,
            frame_vieworg: [0.0, 0.0, 0.0],
            refl_target: None,
            refl_desc_pool: None,
            refl_desc_set: None,
            refl_desc_view: None,
            sky_model_flat: [
                1.0, 0.0, 0.0, 0.0,
                0.0, 1.0, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
                0.0, 0.0, 0.0, 1.0,
            ],
            frame_refl_plane: None,
            shimmer_pool: None,
            shimmer_set: None,
        }
    }

    /// Set screen dimensions.
    pub fn set_dimensions(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        if let Some(ref mut pp) = self.post_processor {
            pp.resize(width, height);
        }
        self.water_fbo.resize(width, height);
    }

    /// Set the character texture ID for console font rendering.
    pub fn draw2d_set_char_texture(&mut self, texnum: u32) {
        self.draw2d.set_char_texture(texnum);
    }

    /// Get the shader manager.
    pub fn shaders(&self) -> Option<&ShaderManager> {
        self.shaders.as_ref()
    }

    /// Get the shader manager mutably.
    pub fn shaders_mut(&mut self) -> Option<&mut ShaderManager> {
        self.shaders.as_mut()
    }

    /// Get the BSP geometry manager.
    pub fn bsp_geometry(&self) -> &BspGeometryManager {
        &self.bsp_geometry
    }

    /// Get the BSP geometry manager mutably.
    pub fn bsp_geometry_mut(&mut self) -> &mut BspGeometryManager {
        &mut self.bsp_geometry
    }

    /// Get the alias model manager.
    pub fn alias_models(&self) -> &AliasModelManager {
        &self.alias_models
    }

    /// Get the alias model manager mutably.
    pub fn alias_models_mut(&mut self) -> &mut AliasModelManager {
        &mut self.alias_models
    }

    /// Get the lightmap array.
    pub fn lightmap_array(&self) -> &LightmapArray {
        &self.lightmap_array
    }

    /// Get the lightmap array mutably.
    pub fn lightmap_array_mut(&mut self) -> &mut LightmapArray {
        &mut self.lightmap_array
    }

    /// Initialize the lightmap texture array GPU resources, upload all layers,
    /// and create the descriptor set for per-pixel lightmap sampling.
    pub fn init_lightmap_gpu(&mut self, layers: &[Vec<u8>]) {
        // Create GPU resources for the lightmap texture array
        self.lightmap_array.create_gpu_resources(layers.len().max(1) as u32);

        if layers.is_empty() {
            return;
        }

        // Batch upload all lightmap layers
        let layer_data: Vec<(u32, Vec<u8>)> = layers.iter()
            .enumerate()
            .filter(|(_, data)| !data.is_empty())
            .map(|(i, data)| (i as u32, data.clone()))
            .collect();

        if !layer_data.is_empty() {
            self.lightmap_array.batch_upload_layers(&layer_data);
            eprintln!("[LIGHTMAP] Uploaded {} lightmap layers to GPU texture array", layer_data.len());
        }

        // Create descriptor set for the lightmap array at set 2
        let image_view = match self.lightmap_array.vk_image_view() {
            Some(v) => v,
            None => return,
        };
        let sampler = match self.lightmap_array.vk_sampler() {
            Some(s) => s,
            None => return,
        };

        let lm_set_layout = match self.pipelines.as_ref().and_then(|pm| pm.lightmap_set_layout()) {
            Some(l) => l,
            None => {
                eprintln!("[LIGHTMAP] No lightmap descriptor set layout available");
                return;
            }
        };

        gpu_device::with_device(|ctx| {
            // SAFETY: Vulkan context valid, main thread only
            unsafe {
                // Create a small descriptor pool for the lightmap set
                let pool_sizes = [vk::DescriptorPoolSize {
                    ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                    descriptor_count: 1,
                }];
                let pool_info = vk::DescriptorPoolCreateInfo::default()
                    .pool_sizes(&pool_sizes)
                    .max_sets(1)
                    .flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET);

                let pool = match ctx.device.create_descriptor_pool(&pool_info, None) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("[LIGHTMAP] Failed to create descriptor pool: {:?}", e);
                        return;
                    }
                };

                // Allocate descriptor set
                let layouts = [lm_set_layout];
                let alloc_info = vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(pool)
                    .set_layouts(&layouts);

                let ds = match ctx.device.allocate_descriptor_sets(&alloc_info) {
                    Ok(sets) => sets[0],
                    Err(e) => {
                        eprintln!("[LIGHTMAP] Failed to allocate descriptor set: {:?}", e);
                        ctx.device.destroy_descriptor_pool(pool, None);
                        return;
                    }
                };

                // Write lightmap texture array to the descriptor set
                let image_info = vk::DescriptorImageInfo::default()
                    .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .image_view(image_view)
                    .sampler(sampler);

                let write = vk::WriteDescriptorSet::default()
                    .dst_set(ds)
                    .dst_binding(0)
                    .dst_array_element(0)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(std::slice::from_ref(&image_info));

                ctx.device.update_descriptor_sets(&[write], &[]);

                self.lightmap_descriptor_set = Some(ds);
                self.lightmap_descriptor_pool = Some(pool);

                eprintln!("[LIGHTMAP] Descriptor set created for per-pixel lightmap sampling");
            }
        });
    }

    /// Get the lightmap descriptor set (set 2) for rendering.
    pub fn lightmap_descriptor_set(&self) -> Option<vk::DescriptorSet> {
        self.lightmap_descriptor_set
    }

    /// Get the 2D draw manager mutably.
    pub fn draw2d_mut(&mut self) -> &mut Draw2DManager {
        &mut self.draw2d
    }

    /// Compute a 4x4 view matrix from vieworg and viewangles (Quake 2 convention).
    fn compute_view_matrix(vieworg: &[f32; 3], viewangles: &[f32; 3]) -> [f32; 16] {
        // Quake 2 angles: [pitch, yaw, roll] in degrees
        let pitch = viewangles[0].to_radians();
        let yaw = viewangles[1].to_radians();
        let roll = viewangles[2].to_radians();

        let (sp, cp) = (pitch.sin(), pitch.cos());
        let (sy, cy) = (yaw.sin(), yaw.cos());
        let (sr, cr) = (roll.sin(), roll.cos());

        // Forward, right, up vectors (Quake convention)
        let forward = [cp * cy, cp * sy, -sp];
        let right = [
            -sr * sp * cy + cr * sy,
            -sr * sp * sy - cr * cy,
            -sr * cp,
        ];
        let up = [
            cr * sp * cy + sr * sy,
            cr * sp * sy - sr * cy,
            cr * cp,
        ];

        // View matrix = inverse of camera transform
        // Dot products for translation
        let tx = -(right[0] * vieworg[0] + right[1] * vieworg[1] + right[2] * vieworg[2]);
        let ty = -(up[0] * vieworg[0] + up[1] * vieworg[1] + up[2] * vieworg[2]);
        let tz = forward[0] * vieworg[0] + forward[1] * vieworg[1] + forward[2] * vieworg[2];

        // Column-major
        [
            right[0], up[0], -forward[0], 0.0,
            right[1], up[1], -forward[1], 0.0,
            right[2], up[2], -forward[2], 0.0,
            tx, ty, tz, 1.0,
        ]
    }

    /// Compute a perspective projection matrix.
    fn compute_projection_matrix(fov_x: f32, fov_y: f32, near: f32, far: f32) -> [f32; 16] {
        let half_fov_x = (fov_x * 0.5).to_radians();
        let half_fov_y = (fov_y * 0.5).to_radians();
        let right = near * half_fov_x.tan();
        let top = near * half_fov_y.tan();

        // Symmetric perspective for Vulkan depth range [0, 1]
        // (Y flip handled by negative viewport height, not the projection matrix)
        let a = near / right;
        let b = near / top;
        let c = -far / (far - near);
        let d = -(near * far) / (far - near);

        // Column-major
        [
            a,   0.0, 0.0,  0.0,
            0.0, b,   0.0,  0.0,
            0.0, 0.0, c,   -1.0,
            0.0, 0.0, d,    0.0,
        ]
    }

    /// Compute an orthographic projection matrix for 2D drawing.
    fn compute_ortho_matrix(width: f32, height: f32) -> [f32; 16] {
        let right = width;
        let bottom = height;
        // Column-major, maps (0..width, 0..height) to (-1..1, 1..-1)
        [
            2.0 / right, 0.0,          0.0, 0.0,
            0.0,         -2.0 / bottom, 0.0, 0.0,
            0.0,         0.0,          -1.0, 0.0,
            -1.0,        1.0,           0.0, 1.0,
        ]
    }

    /// Multiply two 4x4 column-major matrices: result = a * b.
    pub fn mat4_multiply(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
        let mut result = [0.0f32; 16];
        for col in 0..4 {
            for row in 0..4 {
                let mut sum = 0.0;
                for k in 0..4 {
                    sum += a[k * 4 + row] * b[col * 4 + k];
                }
                result[col * 4 + row] = sum;
            }
        }
        result
    }

    /// Convert column-major 16-float array to 4x4 array of arrays.
    fn to_mat4x4(m: &[f32; 16]) -> [[f32; 4]; 4] {
        [
            [m[0], m[1], m[2], m[3]],
            [m[4], m[5], m[6], m[7]],
            [m[8], m[9], m[10], m[11]],
            [m[12], m[13], m[14], m[15]],
        ]
    }

    // =========================================================================
    // D3 Lighting and Shadow Cubemap
    // =========================================================================

    /// Update the set of D3 point lights and pre-compute their shadow cubemaps.
    ///
    /// Called every frame from vk_rmain.  Rebuilds d3_lights from STATIC_LIGHTS
    /// using the cvar-driven limits, then (re-)builds shadow cubemaps whenever the
    /// light list changes or the BSP geometry is freshly uploaded.
    pub fn update_d3_lights(
        &mut self,
        max_lights: usize,
        _ambient: f32,
        _extrude: f32,
        spec_power: f32,
        view_origin: [f32; 3],
    ) {
        use crate::vk_rsurf::STATIC_LIGHTS;

        let static_lights = STATIC_LIGHTS.lock().unwrap_or_else(|e| e.into_inner());

        // Classic mode (max_lights == 0): no D3 additive pass.  Lightmaps encode full
        // baked lighting already.  Any warm-coloured additive pass on top shifts the
        // red/green/blue balance further warm and produces a visible red tint regardless
        // of framebuffer precision, because Q2 lights are overwhelmingly warm-coloured
        // so red_additive >> blue_additive even at very low intensities.
        // D3 mode: cap at max_lights.  Intensity 0.07 with the float16 HDR FBO and
        // ACES tonemapping keeps warm overlap from saturating the red channel.
        if max_lights == 0 {
            self.d3_lights = Vec::new();
            return;
        }
        // HARD safety cap: each active light costs a full-screen cubemap (~1.75 MB), a
        // 6-face build pass, and a whole-scene additive pass every frame. Past a few dozen
        // this exhausts GPU memory / framebuffers / the per-frame budget and crashes (device
        // lost). Until the lit pass is scissored/clustered, clamp regardless of the cvar.
        const SAFE_MAX_LIGHTS: usize = 32;
        let limit = max_lights.min(SAFE_MAX_LIGHTS);
        // Per-light brightness scales DOWN with the count. The lit pass now dark-fill modulates
        // by (1 - baked luma), so this only brightens already-dark surfaces near a fixture —
        // lit surfaces are protected from the warm-tint problem, so we can run hotter than the
        // old flat-base D3 value (0.07).
        let per_light_intensity: f32 = (4.0 / limit as f32).clamp(0.06, 0.35);

        // There can be far more lights in a level (entity lights + every emissive surface +
        // sky) than we can give a shadow cubemap. Pick the ones NEAREST the viewer so the
        // player's local lights (their room's ceiling lights, the nearby sky) are the active
        // set — selecting by global brightness instead lets distant sky lights monopolize
        // every slot and leaves rooms unlit. Reselect only after the viewer has moved a good
        // way, because each active light costs a 6-face scene render to rebuild its cubemap.
        let d2 = |a: &[f32; 3], b: &[f32; 3]| {
            let (x, y, z) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
            x * x + y * y + z * z
        };
        // Reselect rarely: each reselection rebuilds every active cubemap (a hard hitch)
        // and changes which lights are active (a visible lighting "pop"). A large threshold
        // keeps the set stable while the player explores a region.
        let must_select = self.d3_lights.is_empty()
            || self.shadow_cubemaps.is_empty()
            || d2(&view_origin, &self.last_light_select_pos) > 768.0 * 768.0;
        if !must_select {
            return; // keep the current active set and its cubemaps
        }
        self.last_light_select_pos = view_origin;

        // Nearest `limit` lights to the viewer.
        let mut order: Vec<usize> = (0..static_lights.len()).collect();
        order.sort_by(|&a, &b| {
            d2(&static_lights[a].origin, &view_origin)
                .partial_cmp(&d2(&static_lights[b].origin, &view_origin))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        order.truncate(limit);

        // If the same lights are still nearest, there is nothing to rebuild.
        let mut selected_sorted = order.clone();
        selected_sorted.sort_unstable();
        if selected_sorted == self.last_selected_lights && !self.shadow_cubemaps.is_empty() {
            return;
        }
        self.last_selected_lights = selected_sorted;

        self.d3_lights = order.iter().map(|&i| {
            let sl = &static_lights[i];
            // LOCAL radius. A large radius (the old intensity*1.5 → 450-600u) made every one of
            // the nearest-N lights overlap the whole room, so the additive pass lit every
            // surface at once and washed the scene to an over-bright near-white ("red screen").
            // Keep it tight so a fixture only fills the dark surfaces immediately around it.
            let radius = (sl.intensity * 0.5).clamp(96.0, 220.0);
            GpuLightD3 {
                pos: sl.origin,
                radius,
                color: sl.color,
                intensity: per_light_intensity,
                specular: sl.color,
                spec_pow: spec_power,
            }
        }).collect();
        drop(static_lights);

        let bsp_vbo = self.bsp_geometry.vertex_buffer().vk_buffer().unwrap_or(vk::Buffer::null());
        let bsp_ibo = self.bsp_geometry.index_buffer().vk_buffer().unwrap_or(vk::Buffer::null());
        let bsp_index_count = self.bsp_geometry.index_count();
        self.build_shadow_maps(bsp_vbo, bsp_ibo, bsp_index_count);
    }

    /// Accept dynamic lights for the current frame (used by dlight rendering).
    pub fn set_frame_dlights(&mut self, _dlights: Vec<myq2_common::q_shared::DLight>) {
        // Dynamic lights are handled by the dlight disc renderer; no action needed here.
    }

    /// Get the D3 lights slice (read-only).
    pub fn d3_lights(&self) -> &[GpuLightD3] {
        &self.d3_lights
    }

    /// Get the shadow cubemaps slice (read-only).
    pub fn shadow_cubemaps(&self) -> &[ShadowCubemap] {
        &self.shadow_cubemaps
    }

    /// Build a right-hand look-at matrix (column-major, for Vulkan).
    ///
    /// Produces a view matrix where the camera is at `eye`, looking at `center`, with `up`.
    fn look_at_rh(eye: [f32; 3], center: [f32; 3], up: [f32; 3]) -> [f32; 16] {
        let f = {
            let dx = center[0] - eye[0];
            let dy = center[1] - eye[1];
            let dz = center[2] - eye[2];
            let len = (dx*dx + dy*dy + dz*dz).sqrt().max(1e-10);
            [dx/len, dy/len, dz/len]
        };
        let s = {
            // s = normalize(cross(f, up))
            let cx = f[1]*up[2] - f[2]*up[1];
            let cy = f[2]*up[0] - f[0]*up[2];
            let cz = f[0]*up[1] - f[1]*up[0];
            let len = (cx*cx + cy*cy + cz*cz).sqrt().max(1e-10);
            [cx/len, cy/len, cz/len]
        };
        let u = [
            s[1]*f[2] - s[2]*f[1],
            s[2]*f[0] - s[0]*f[2],
            s[0]*f[1] - s[1]*f[0],
        ];
        let tx = -(s[0]*eye[0] + s[1]*eye[1] + s[2]*eye[2]);
        let ty = -(u[0]*eye[0] + u[1]*eye[1] + u[2]*eye[2]);
        let tz =  f[0]*eye[0] + f[1]*eye[1] + f[2]*eye[2];

        // Column-major matrix:
        // | s.x  u.x  -f.x  0 |
        // | s.y  u.y  -f.y  0 |
        // | s.z  u.z  -f.z  0 |
        // | tx   ty    tz   1 |
        [
            s[0], u[0], -f[0], 0.0,
            s[1], u[1], -f[1], 0.0,
            s[2], u[2], -f[2], 0.0,
            tx,   ty,    tz,   1.0,
        ]
    }

    /// Build a perspective projection matrix for Vulkan (depth range [0, 1], RH).
    fn perspective_rh(fov_y_rad: f32, aspect: f32, near: f32, far: f32) -> [f32; 16] {
        let tan_half = (fov_y_rad * 0.5).tan();
        let sy = 1.0 / tan_half;
        let sx = sy / aspect;
        let nf = far / (near - far);
        // Column-major
        [
            sx,  0.0,  0.0,  0.0,
            0.0, sy,   0.0,  0.0,
            0.0, 0.0,  nf,  -1.0,
            0.0, 0.0,  near * nf, 0.0,
        ]
    }

    /// Compute the 6 MVP matrices for rendering a cubemap from `light_pos` with radius `light_radius`.
    ///
    /// Returns column-major MVP matrices for faces [+X, -X, +Y, -Y, +Z, -Z].
    fn cubemap_face_mvps(light_pos: [f32; 3], light_radius: f32) -> [[f32; 16]; 6] {
        // Vulkan cubemap face look-directions and up vectors (right-hand Y-down convention).
        // The view directions match Vulkan's cubemap face ordering.
        let lp = light_pos;
        let r = light_radius.max(1.0);

        let faces: [([f32; 3], [f32; 3]); 6] = [
            ([1.0,  0.0,  0.0], [0.0, -1.0,  0.0]),  // +X
            ([-1.0, 0.0,  0.0], [0.0, -1.0,  0.0]),  // -X
            ([0.0,  1.0,  0.0], [0.0,  0.0,  1.0]),  // +Y
            ([0.0, -1.0,  0.0], [0.0,  0.0, -1.0]),  // -Y
            ([0.0,  0.0,  1.0], [0.0, -1.0,  0.0]),  // +Z
            ([0.0,  0.0, -1.0], [0.0, -1.0,  0.0]),  // -Z
        ];

        let proj = Self::perspective_rh(PI / 2.0, 1.0, 1.0, r);

        let mut mvps = [[0.0f32; 16]; 6];
        for (i, (dir, up)) in faces.iter().enumerate() {
            let center = [lp[0] + dir[0], lp[1] + dir[1], lp[2] + dir[2]];
            let view = Self::look_at_rh(lp, center, *up);
            mvps[i] = Self::mat4_multiply(&proj, &view);
        }
        mvps
    }

    /// Find a suitable memory type index for device-local image memory.
    fn find_memory_type(
        ctx: &crate::vulkan::VulkanContext,
        type_filter: u32,
        properties: vk::MemoryPropertyFlags,
    ) -> Option<u32> {
        // SAFETY: query only, no mutation
        let mem_props = unsafe {
            ctx.instance.get_physical_device_memory_properties(ctx.physical_device)
        };
        (0..mem_props.memory_type_count).find(|&i| {
            (type_filter & (1 << i)) != 0
                && mem_props.memory_types[i as usize].property_flags.contains(properties)
        })
    }

    /// Destroy all existing shadow cubemaps and release their Vulkan resources.
    fn destroy_shadow_cubemaps(&mut self) {
        if self.shadow_cubemaps.is_empty() {
            return;
        }
        gpu_device::with_device(|ctx| {
            // SAFETY: All handles were created by this renderer and are no longer in use.
            unsafe {
                // Wait for device idle before releasing resources
                let _ = ctx.device.device_wait_idle();

                let pool_opt = self.shadow_descriptor_pool;
                for sc in self.shadow_cubemaps.drain(..) {
                    // Free descriptor set (if pool allows individual free)
                    if let Some(pool) = pool_opt {
                        let _ = ctx.device.free_descriptor_sets(pool, &[sc.descriptor_set]);
                    }
                    for fb in &sc.framebuffers {
                        ctx.device.destroy_framebuffer(*fb, None);
                    }
                    // Per-face R32_SFLOAT colour views
                    for fv in &sc.face_views {
                        ctx.device.destroy_image_view(*fv, None);
                    }
                    // D32_SFLOAT depth resources (kept alive for framebuffer validity)
                    ctx.device.destroy_image_view(sc.depth_view, None);
                    ctx.device.free_memory(sc.depth_memory, None);
                    ctx.device.destroy_image(sc.depth_image, None);
                    // R32_SFLOAT colour cubemap
                    ctx.device.destroy_image_view(sc.cube_view, None);
                    ctx.device.free_memory(sc.memory, None);
                    ctx.device.destroy_image(sc.image, None);
                }
            }
        });
    }

    /// Create the render pass used for shadow cubemap face rendering.
    ///
    /// Attachment 0: R32_SFLOAT colour (stores dist/radius — sampled in world_lit pass).
    ///   initial = COLOR_ATTACHMENT_OPTIMAL  (caller transitions before first face)
    ///   final   = COLOR_ATTACHMENT_OPTIMAL  (caller transitions to SHADER_READ_ONLY_OPTIMAL
    ///                                        after all 6 faces)
    ///
    /// Attachment 1: D32_SFLOAT depth (z-testing only, discarded after each face).
    ///   initial = DEPTH_STENCIL_ATTACHMENT_OPTIMAL
    ///   final   = DEPTH_STENCIL_ATTACHMENT_OPTIMAL
    ///
    /// Using a colour-format cubemap avoids the depth-image sampling issues
    /// that prevent D32_SFLOAT from being reliably read with a plain samplerCube.
    fn create_shadow_render_pass(&mut self) {
        if self.shadow_render_pass.is_some() {
            return; // Already created
        }
        gpu_device::with_device(|ctx| {
            // SAFETY: Vulkan context valid, single-threaded.
            unsafe {
                // Attachment 0: R32_SFLOAT colour — stores dist/radius
                let color_attachment = vk::AttachmentDescription::default()
                    .format(vk::Format::R32_SFLOAT)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .load_op(vk::AttachmentLoadOp::CLEAR)
                    .store_op(vk::AttachmentStoreOp::STORE)
                    .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                    .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                    .initial_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .final_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

                // Attachment 1: D32_SFLOAT depth — z-testing only (DONT_CARE store)
                let depth_attachment = vk::AttachmentDescription::default()
                    .format(vk::Format::D32_SFLOAT)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .load_op(vk::AttachmentLoadOp::CLEAR)
                    .store_op(vk::AttachmentStoreOp::DONT_CARE)
                    .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                    .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                    .initial_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                    .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);

                let color_ref = vk::AttachmentReference::default()
                    .attachment(0)
                    .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

                let depth_ref = vk::AttachmentReference::default()
                    .attachment(1)
                    .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);

                let color_refs = [color_ref];
                let subpass = vk::SubpassDescription::default()
                    .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
                    .color_attachments(&color_refs)
                    .depth_stencil_attachment(&depth_ref);

                // Ensure prior shader reads finish before this pass writes colour/depth,
                // and that colour writes finish before the next shader read.
                let dependencies = [
                    vk::SubpassDependency::default()
                        .src_subpass(vk::SUBPASS_EXTERNAL)
                        .dst_subpass(0)
                        .src_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER)
                        .dst_stage_mask(
                            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                                | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
                        )
                        .src_access_mask(vk::AccessFlags::SHADER_READ)
                        .dst_access_mask(
                            vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                                | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                        )
                        .dependency_flags(vk::DependencyFlags::BY_REGION),
                    vk::SubpassDependency::default()
                        .src_subpass(0)
                        .dst_subpass(vk::SUBPASS_EXTERNAL)
                        .src_stage_mask(
                            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                                | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                        )
                        .dst_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER)
                        .src_access_mask(
                            vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                                | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                        )
                        .dst_access_mask(vk::AccessFlags::SHADER_READ)
                        .dependency_flags(vk::DependencyFlags::BY_REGION),
                ];

                let attachments = [color_attachment, depth_attachment];
                let render_pass_info = vk::RenderPassCreateInfo::default()
                    .attachments(&attachments)
                    .subpasses(std::slice::from_ref(&subpass))
                    .dependencies(&dependencies);

                match ctx.device.create_render_pass(&render_pass_info, None) {
                    Ok(rp) => {
                        self.shadow_render_pass = Some(rp);
                        eprintln!("[SHADOW] Created shadow render pass (R32_SFLOAT colour + D32_SFLOAT depth)");
                    }
                    Err(e) => {
                        eprintln!("[SHADOW] Failed to create shadow render pass: {:?}", e);
                    }
                }
            }
        });
    }

    /// Lazily create the projective-shadow resources (shadow-map images/views/framebuffer on
    /// the shared shadow render pass, the resolve render pass, sampler, descriptor pool/set,
    /// and the two pipelines). Returns false (and casts no shadows) if anything fails.
    fn ensure_projective_shadow_resources(&mut self) -> bool {
        use super::shadow_project as sp;
        if self.projective_shadow.framebuffer.is_none() {
            let created = gpu_device::with_device(|ctx| unsafe {
                let alloc = |img: vk::Image| -> Option<vk::DeviceMemory> {
                    let r = ctx.device.get_image_memory_requirements(img);
                    let t = Self::find_memory_type(ctx, r.memory_type_bits, vk::MemoryPropertyFlags::DEVICE_LOCAL)?;
                    let ai = vk::MemoryAllocateInfo::default().allocation_size(r.size).memory_type_index(t);
                    ctx.device.allocate_memory(&ai, None).ok()
                };
                // Caster render pass: RG32 colour (R=caster depth, G=floor depth) + D32 depth.
                let cca = vk::AttachmentDescription::default()
                    .format(vk::Format::R32G32_SFLOAT).samples(vk::SampleCountFlags::TYPE_1)
                    .load_op(vk::AttachmentLoadOp::CLEAR).store_op(vk::AttachmentStoreOp::STORE)
                    .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE).stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                    .initial_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .final_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
                let cda = vk::AttachmentDescription::default()
                    .format(vk::Format::D32_SFLOAT).samples(vk::SampleCountFlags::TYPE_1)
                    .load_op(vk::AttachmentLoadOp::CLEAR).store_op(vk::AttachmentStoreOp::DONT_CARE)
                    .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE).stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                    .initial_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                    .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
                let ccar = vk::AttachmentReference::default().attachment(0).layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
                let cdar = vk::AttachmentReference::default().attachment(1).layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
                let ccar_refs = [ccar];
                let csub = vk::SubpassDescription::default()
                    .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
                    .color_attachments(&ccar_refs).depth_stencil_attachment(&cdar);
                let catt = [cca, cda];
                let caster_rp = ctx.device.create_render_pass(&vk::RenderPassCreateInfo::default()
                    .attachments(&catt).subpasses(std::slice::from_ref(&csub)), None).ok()?;

                // RG32 colour (R = caster light-space depth, G = floor light-space depth).
                let ci = vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D).format(vk::Format::R32G32_SFLOAT)
                    .extent(vk::Extent3D { width: sp::SHADOW_SIZE, height: sp::SHADOW_SIZE, depth: 1 })
                    .mip_levels(1).array_layers(1).samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED)
                    .initial_layout(vk::ImageLayout::UNDEFINED);
                let color = ctx.device.create_image(&ci, None).ok()?;
                let color_mem = alloc(color)?;
                ctx.device.bind_image_memory(color, color_mem, 0).ok()?;
                let di = vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D).format(vk::Format::D32_SFLOAT)
                    .extent(vk::Extent3D { width: sp::SHADOW_SIZE, height: sp::SHADOW_SIZE, depth: 1 })
                    .mip_levels(1).array_layers(1).samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
                    .initial_layout(vk::ImageLayout::UNDEFINED);
                let depth = ctx.device.create_image(&di, None).ok()?;
                let depth_mem = alloc(depth)?;
                ctx.device.bind_image_memory(depth, depth_mem, 0).ok()?;
                let cv = ctx.device.create_image_view(&vk::ImageViewCreateInfo::default()
                    .image(color).view_type(vk::ImageViewType::TYPE_2D).format(vk::Format::R32G32_SFLOAT)
                    .subresource_range(vk::ImageSubresourceRange::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR).level_count(1).layer_count(1)), None).ok()?;
                let dv = ctx.device.create_image_view(&vk::ImageViewCreateInfo::default()
                    .image(depth).view_type(vk::ImageViewType::TYPE_2D).format(vk::Format::D32_SFLOAT)
                    .subresource_range(vk::ImageSubresourceRange::default()
                        .aspect_mask(vk::ImageAspectFlags::DEPTH).level_count(1).layer_count(1)), None).ok()?;
                let attaches = [cv, dv];
                let fb = ctx.device.create_framebuffer(&vk::FramebufferCreateInfo::default()
                    .render_pass(caster_rp).attachments(&attaches)
                    .width(sp::SHADOW_SIZE).height(sp::SHADOW_SIZE).layers(1), None).ok()?;
                // Resolve render pass: one colour attachment = scene colour, LOAD/STORE, stays in
                // COLOR_ATTACHMENT_OPTIMAL (the composite later transitions it to SHADER_READ).
                let ca = vk::AttachmentDescription::default()
                    .format(vk::Format::R16G16B16A16_SFLOAT).samples(vk::SampleCountFlags::TYPE_1)
                    .load_op(vk::AttachmentLoadOp::LOAD).store_op(vk::AttachmentStoreOp::STORE)
                    .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE).stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                    .initial_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .final_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
                let car = vk::AttachmentReference::default().attachment(0).layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
                let car_refs = [car];
                let sub = vk::SubpassDescription::default()
                    .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS).color_attachments(&car_refs);
                let dep = vk::SubpassDependency::default()
                    .src_subpass(vk::SUBPASS_EXTERNAL).dst_subpass(0)
                    .src_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER)
                    .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                    .src_access_mask(vk::AccessFlags::SHADER_READ)
                    .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE);
                let resolve_rp = ctx.device.create_render_pass(&vk::RenderPassCreateInfo::default()
                    .attachments(std::slice::from_ref(&ca)).subpasses(std::slice::from_ref(&sub))
                    .dependencies(std::slice::from_ref(&dep)), None).ok()?;
                let samp = ctx.device.create_sampler(&vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::NEAREST).min_filter(vk::Filter::NEAREST)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE), None).ok()?;
                let ps = &mut self.projective_shadow;
                ps.color_image = Some(color); ps.color_mem = Some(color_mem); ps.color_view = Some(cv);
                ps.depth_image = Some(depth); ps.depth_mem = Some(depth_mem); ps.depth_view = Some(dv);
                ps.framebuffer = Some(fb); ps.resolve_rp = Some(resolve_rp); ps.sampler = Some(samp);
                ps.caster_rp = Some(caster_rp);
                Some(())
            }).flatten().is_some();
            if !created { return false; }
        }

        let resolve_rp = self.projective_shadow.resolve_rp.unwrap();
        let caster_rp = self.projective_shadow.caster_rp.unwrap();
        match self.pipelines.as_mut() {
            Some(pm) => {
                if pm.create_shadow_caster_pipeline(caster_rp).is_err() { return false; }
                if pm.create_shadow_bsp_pipeline(caster_rp).is_err() { return false; }
                if pm.create_shadow_resolve_pipeline(resolve_rp).is_err() { return false; }
            }
            None => return false,
        }

        if self.projective_shadow.resolve_set.is_none() {
            let set_layout = match self.pipelines.as_ref().and_then(|pm| pm.shadow_resolve_set_layout()) {
                Some(sl) => sl, None => return false,
            };
            let ok = gpu_device::with_device(|ctx| unsafe {
                let sizes = [vk::DescriptorPoolSize::default()
                    .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).descriptor_count(2)];
                let pool = ctx.device.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default()
                    .max_sets(1).pool_sizes(&sizes), None).ok()?;
                let layouts = [set_layout];
                let set = ctx.device.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(pool).set_layouts(&layouts)).ok()?[0];
                let ps = &mut self.projective_shadow;
                ps.resolve_pool = Some(pool); ps.resolve_set = Some(set);
                Some(())
            }).flatten().is_some();
            if !ok { return false; }
        }
        true
    }

    /// Render projective dynamic shadows: depth-render all casters from the light into the
    /// shadow map, then a fullscreen resolve that darkens the scene where shadowed. Called
    /// right after the scene's dynamic-rendering pass ends (scene colour in COLOR_ATTACHMENT,
    /// depth in DEPTH_ATTACHMENT). Defensive: any missing resource just skips shadows.
    fn render_projective_shadows(&mut self, cmd: vk::CommandBuffer, view_origin: [f32; 3], view_proj_flat: [f32; 16]) {
        use super::shadow_project as sp;
        if crate::vk_rmain::rcvars().vk_shadows.value == 0.0 { return; }
        if !self.ensure_projective_shadow_resources() { return; }

        // Scene colour/depth from the scene FBO.
        let (scene_color_view, scene_depth_view, scene_depth_image) =
            match self.post_processor.as_ref().map(|pp| pp.scene_fbo()) {
                Some(fbo) => match (fbo.color_view(), fbo.depth_view(), fbo.depth_image()) {
                    (Some(cv), Some(dv), Some(di)) => (cv, dv, di),
                    _ => return,
                },
                None => return,
            };

        // (Re)create the resolve framebuffer if the scene colour view changed (resize).
        if self.projective_shadow.resolve_fb_view != Some(scene_color_view) {
            let resolve_rp = self.projective_shadow.resolve_rp.unwrap();
            let (w, h) = (self.width, self.height);
            let old = self.projective_shadow.resolve_fb;
            let newfb = gpu_device::with_device(|ctx| unsafe {
                if let Some(o) = old { ctx.device.destroy_framebuffer(o, None); }
                let at = [scene_color_view];
                ctx.device.create_framebuffer(&vk::FramebufferCreateInfo::default()
                    .render_pass(resolve_rp).attachments(&at).width(w).height(h).layers(1), None).ok()
            }).flatten();
            self.projective_shadow.resolve_fb = newfb;
            self.projective_shadow.resolve_fb_view = Some(scene_color_view);
        }
        let resolve_fb = match self.projective_shadow.resolve_fb { Some(f) => f, None => return };

        // (Re)create a DEPTH-aspect-only view of the scene depth for sampling (the scene's
        // own view is a combined depth+stencil view and can't be sampled).
        if self.projective_shadow.depth_sample_src != Some(scene_depth_image) {
            let old = self.projective_shadow.depth_sample_view;
            let newv = gpu_device::with_device(|ctx| unsafe {
                if let Some(o) = old { ctx.device.destroy_image_view(o, None); }
                ctx.device.create_image_view(&vk::ImageViewCreateInfo::default()
                    .image(scene_depth_image).view_type(vk::ImageViewType::TYPE_2D)
                    .format(vk::Format::D32_SFLOAT_S8_UINT)
                    .subresource_range(vk::ImageSubresourceRange::default()
                        .aspect_mask(vk::ImageAspectFlags::DEPTH).level_count(1).layer_count(1)), None).ok()
            }).flatten();
            self.projective_shadow.depth_sample_view = newv;
            self.projective_shadow.depth_sample_src = Some(scene_depth_image);
        }
        let depth_sample_view = match self.projective_shadow.depth_sample_view { Some(v) => v, None => return };
        let _ = scene_depth_view; // superseded by the depth-aspect-only view above

        // Light matrices. combined = light_vp * inv(view_proj) maps screen NDC -> light clip.
        let light_vp = sp::light_view_proj(view_origin, sp::DEFAULT_LIGHT_DIR, sp::COVERAGE, sp::LIGHT_HEIGHT);
        let inv_vp = sp::invert(&view_proj_flat);
        let combined = sp::mul(&light_vp, &inv_vp);

        // Gather alias casters as (push constants with light-space MVP, vbo, ibo, idx, frame off).
        let mut casters: Vec<(AliasPushConstants, vk::Buffer, vk::Buffer, u32, u64)> = Vec::new();
        for batch in self.alias_instanced.batches() {
            let mb = match self.alias_models.get(batch.model_id()) { Some(b) => b, None => continue };
            let (vbo, ibo) = match (mb.vertex_buffer().vk_buffer(), mb.index_buffer().vk_buffer()) {
                (Some(v), Some(i)) => (v, i), _ => continue,
            };
            let index_count = mb.index_count();
            for inst in batch.instances() {
                // Skip weapon-view and translucent casters (match the blob-shadow gate).
                if inst.flags & (myq2_common::q_shared::RF_WEAPONMODEL | myq2_common::q_shared::RF_TRANSLUCENT) as u32 != 0 {
                    continue;
                }
                // The local player's body (RF_VIEWERMODEL) carries the full view pitch/roll in
                // its model matrix; casting the shadow with it makes the self-shadow tilt into
                // the floor when looking up/down or crouching. A standing body is upright —
                // cast with a yaw-only rebuild of the matrix so the shadow stays flat on the
                // ground (and still drapes down/across walls via the per-pixel resolve).
                let model_flat = if inst.flags & myq2_common::q_shared::RF_VIEWERMODEL as u32 != 0 {
                    let m = &inst.model_matrix;
                    let yaw_deg = m[0][1].atan2(m[0][0]).to_degrees();
                    mat4_to_flat(&build_entity_matrix(
                        &[m[3][0], m[3][1], m[3][2]],
                        &[0.0, yaw_deg, 0.0],
                    ))
                } else {
                    mat4_to_flat(&inst.model_matrix)
                };
                let mvp = ModernRenderPath::mat4_multiply(&light_vp, &model_flat);
                let frontlerp = 1.0 - inst.backlerp;
                let frame = inst.frame.min(mb.frame_count().saturating_sub(1));
                let frame_offset = mb.frame_offset(frame) as u64;
                // Floor below this caster, in light-NDC depth: trace straight down from the
                // caster's origin (model translation) to the world floor, project that point
                // into the light. The shader bounds the caster's shadow to this depth so it
                // lands on the surface it sits above and can't bleed through onto things below.
                let origin = [inst.model_matrix[3][0], inst.model_matrix[3][1], inst.model_matrix[3][2]];
                // SAFETY: r_ground_z accesses vk_local statics, main thread only.
                let floor_depth = unsafe {
                    match crate::vk_light::r_ground_z(&origin) {
                        Some(fz) => {
                            let p = [origin[0], origin[1], fz, 1.0];
                            let mut c = [0.0f32; 4];
                            for i in 0..4 {
                                c[i] = light_vp[i] * p[0] + light_vp[4 + i] * p[1]
                                    + light_vp[8 + i] * p[2] + light_vp[12 + i] * p[3];
                            }
                            if c[3].abs() > 1e-6 { (c[2] / c[3]).clamp(0.0, 1.0) } else { 1.0 }
                        }
                        None => 1.0, // no floor found ⇒ unbounded (far)
                    }
                };
                let pc = AliasPushConstants {
                    mvp,
                    shade_light: [0.0, 0.0, 0.0],
                    alpha: floor_depth, // passed to shadow_caster.frag as the floor depth (G)
                    move_vec: [0.0, 0.0, 0.0],
                    backlerp: inst.backlerp,
                    front_v: [frontlerp, frontlerp, frontlerp],
                    shell_scale: 0.0,
                    back_v: [inst.backlerp, inst.backlerp, inst.backlerp],
                    is_shell: 0,
                };
                casters.push((pc, vbo, ibo, index_count, frame_offset));
            }
        }

        // Gather MOVED brush models (lifts/doors) as BSP shadow casters: light-space MVP,
        // floor depth (so the shadow lands on the surface below), and their surface index
        // ranges in the BSP buffers. Static brush models are skipped (their shadows are baked).
        let mut movers: Vec<([f32; 16], f32, Vec<(u32, u32)>)> = Vec::new();
        for bm in &self.brush_models {
            if bm.dyn_block[0] <= 0.5 { continue; }
            let model_flat = mat4_to_flat(&bm.model_matrix);
            let light_mvp = ModernRenderPath::mat4_multiply(&light_vp, &model_flat);
            // SAFETY: r_ground_z accesses vk_local statics, main thread only.
            let floor_depth = unsafe {
                match crate::vk_light::r_ground_z(&bm.center) {
                    Some(fz) => {
                        let p = [bm.center[0], bm.center[1], fz, 1.0];
                        let mut c = [0.0f32; 4];
                        for i in 0..4 {
                            c[i] = light_vp[i] * p[0] + light_vp[4 + i] * p[1]
                                + light_vp[8 + i] * p[2] + light_vp[12 + i] * p[3];
                        }
                        if c[3].abs() > 1e-6 { (c[2] / c[3]).clamp(0.0, 1.0) } else { 1.0 }
                    }
                    None => 1.0,
                }
            };
            let ranges: Vec<(u32, u32)> = self.bsp_geometry
                .draw_info_for_range(bm.first_surface, bm.num_surfaces)
                .iter().map(|s| (s.first_index, s.index_count)).collect();
            if !ranges.is_empty() { movers.push((light_mvp, floor_depth, ranges)); }
        }
        let bsp_vbo = self.bsp_geometry.vertex_buffer().vk_buffer();
        let bsp_ibo = self.bsp_geometry.index_buffer().vk_buffer();
        let bsp_pipe = self.pipelines.as_ref().and_then(|pm| pm.shadow_bsp_pipeline());

        let shadow_rp = self.projective_shadow.caster_rp.unwrap();
        let ps_fb = self.projective_shadow.framebuffer.unwrap();
        let shadow_color = self.projective_shadow.color_image.unwrap();
        let shadow_color_view = self.projective_shadow.color_view.unwrap();
        let resolve_rp = self.projective_shadow.resolve_rp.unwrap();
        let sampler = self.projective_shadow.sampler.unwrap();
        let resolve_set = self.projective_shadow.resolve_set.unwrap();
        let (caster_pipe, caster_layout) =
            match self.pipelines.as_ref().and_then(|pm| pm.shadow_caster_pipeline()) { Some(p) => p, None => return };
        let (resolve_pipe, resolve_layout) =
            match self.pipelines.as_ref().and_then(|pm| pm.shadow_resolve_pipeline()) { Some(p) => p, None => return };
        let (sw, sh) = (self.width, self.height);
        if casters.is_empty() && movers.is_empty() { return; }

        gpu_device::with_device(|ctx| unsafe {
            // ---- Caster depth pass into the shadow map ----
            // Clear R (caster depth) and G (floor depth) to 1.0 (far) so empty texels cast
            // nothing (a receiver is never "behind" a depth of 1.0).
            let clears = [
                vk::ClearValue { color: vk::ClearColorValue { float32: [1.0, 1.0, 0.0, 0.0] } },
                vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 } },
            ];
            let area = vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 },
                extent: vk::Extent2D { width: sp::SHADOW_SIZE, height: sp::SHADOW_SIZE } };
            ctx.device.cmd_begin_render_pass(cmd, &vk::RenderPassBeginInfo::default()
                .render_pass(shadow_rp).framebuffer(ps_fb).render_area(area).clear_values(&clears),
                vk::SubpassContents::INLINE);
            let vp = vk::Viewport { x: 0.0, y: 0.0, width: sp::SHADOW_SIZE as f32, height: sp::SHADOW_SIZE as f32, min_depth: 0.0, max_depth: 1.0 };
            ctx.device.cmd_set_viewport(cmd, 0, &[vp]);
            ctx.device.cmd_set_scissor(cmd, 0, &[area]);
            ctx.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, caster_pipe);
            for (pc, vbo, ibo, index_count, frame_offset) in &casters {
                let bytes = std::slice::from_raw_parts(
                    pc as *const AliasPushConstants as *const u8, std::mem::size_of::<AliasPushConstants>());
                ctx.device.cmd_push_constants(cmd, caster_layout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT, 0, bytes);
                ctx.device.cmd_bind_vertex_buffers(cmd, 0, &[*vbo], &[*frame_offset]);
                ctx.device.cmd_bind_index_buffer(cmd, *ibo, 0, vk::IndexType::UINT32);
                ctx.device.cmd_draw_indexed(cmd, *index_count, 1, 0, 0, 0);
            }
            // ---- Mover (brush) casters into the same shadow map ----
            if let (Some((bsp_pipe, bsp_layout)), Some(bvbo), Some(bibo)) = (bsp_pipe, bsp_vbo, bsp_ibo) {
                if !movers.is_empty() {
                    ctx.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, bsp_pipe);
                    ctx.device.cmd_bind_vertex_buffers(cmd, 0, &[bvbo], &[0]);
                    ctx.device.cmd_bind_index_buffer(cmd, bibo, 0, vk::IndexType::UINT32);
                    #[repr(C)]
                    struct BspPush { mvp: [f32; 16], floor: f32 }
                    for (mvp, floor, ranges) in &movers {
                        let bp = BspPush { mvp: *mvp, floor: *floor };
                        let bytes = std::slice::from_raw_parts(&bp as *const BspPush as *const u8, 68);
                        ctx.device.cmd_push_constants(cmd, bsp_layout, vk::ShaderStageFlags::VERTEX, 0, bytes);
                        for (first_index, index_count) in ranges {
                            ctx.device.cmd_draw_indexed(cmd, *index_count, 1, *first_index, 0, 0);
                        }
                    }
                }
            }
            ctx.device.cmd_end_render_pass(cmd);

            // ---- Barriers: shadow colour + scene depth -> SHADER_READ for sampling ----
            let b_shadow = vk::ImageMemoryBarrier::default()
                .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                .dst_access_mask(vk::AccessFlags::SHADER_READ)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED).dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(shadow_color)
                .subresource_range(vk::ImageSubresourceRange::default()
                    .aspect_mask(vk::ImageAspectFlags::COLOR).level_count(1).layer_count(1));
            let b_depth = vk::ImageMemoryBarrier::default()
                .old_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .src_access_mask(vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE)
                .dst_access_mask(vk::AccessFlags::SHADER_READ)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED).dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(scene_depth_image)
                .subresource_range(vk::ImageSubresourceRange::default()
                    .aspect_mask(vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL)
                    .level_count(1).layer_count(1));
            ctx.device.cmd_pipeline_barrier(cmd,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                vk::PipelineStageFlags::FRAGMENT_SHADER, vk::DependencyFlags::empty(),
                &[], &[], &[b_shadow, b_depth]);

            // ---- Bind resolve inputs (scene depth = binding 0, shadow map = binding 1) ----
            let depth_info = vk::DescriptorImageInfo::default().image_view(depth_sample_view)
                .sampler(sampler).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
            let shadow_info = vk::DescriptorImageInfo::default().image_view(shadow_color_view)
                .sampler(sampler).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
            let writes = [
                vk::WriteDescriptorSet::default().dst_set(resolve_set).dst_binding(0)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(std::slice::from_ref(&depth_info)),
                vk::WriteDescriptorSet::default().dst_set(resolve_set).dst_binding(1)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(std::slice::from_ref(&shadow_info)),
            ];
            ctx.device.update_descriptor_sets(&writes, &[]);

            // ---- Resolve pass: darken the scene colour where shadowed ----
            let sarea = vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent: vk::Extent2D { width: sw, height: sh } };
            ctx.device.cmd_begin_render_pass(cmd, &vk::RenderPassBeginInfo::default()
                .render_pass(resolve_rp).framebuffer(resolve_fb).render_area(sarea),
                vk::SubpassContents::INLINE);
            let vp2 = vk::Viewport { x: 0.0, y: 0.0, width: sw as f32, height: sh as f32, min_depth: 0.0, max_depth: 1.0 };
            ctx.device.cmd_set_viewport(cmd, 0, &[vp2]);
            ctx.device.cmd_set_scissor(cmd, 0, &[sarea]);
            ctx.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, resolve_pipe);
            ctx.device.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, resolve_layout, 0, &[resolve_set], &[]);
            // Camera position in light NDC, for the "you're in a caster's shadow" view dim.
            let cam_proj = {
                let v = [view_origin[0], view_origin[1], view_origin[2], 1.0];
                let mut c = [0.0f32; 4];
                for i in 0..4 {
                    c[i] = light_vp[i] * v[0] + light_vp[4 + i] * v[1]
                        + light_vp[8 + i] * v[2] + light_vp[12 + i] * v[3];
                }
                let w = if c[3].abs() > 1e-6 { c[3] } else { 1.0 };
                [c[0] / w, c[1] / w, c[2] / w]
            };
            #[repr(C)]
            struct ResolvePush {
                m: [f32; 16], bias: f32, darkness: f32, cam_min_gap: f32, floor_band: f32,
                cam_proj: [f32; 3], cam_dim: f32, near_skip: f32,
            }
            // cam_min_gap (light-space): an occluder must be at least ~70 world units above the
            // camera to dim the view, so the player's own body at the eye is ignored. The light
            // ortho maps [near,far]→[0,1], so 1 NDC-z ≈ (far-near) world units.
            let ortho_range = 2.0 * sp::LIGHT_HEIGHT + 2.0 * sp::COVERAGE - 1.0;
            let cam_min_gap = 70.0 / ortho_range;
            // floor_band is a soft fade (light-space depth) just below the floor so the shadow
            // doesn't hard-cut where it meets the surface below the caster.
            let rp = ResolvePush {
                // floor_band: wide enough that a step/ledge just below the caster's floor gets a
                // faded shadow instead of a hard cut at the surface seam (0.002 cut sharply).
                m: combined, bias: 0.0015, darkness: 0.5, cam_min_gap, floor_band: 0.02,
                cam_proj, cam_dim: 0.4,
                // Skip shadows on pixels nearer than ~0.9 depth (the held view weapon, which
                // sits closer to the eye than the floor your shadow falls on).
                near_skip: 0.9,
            };
            let pbytes = std::slice::from_raw_parts(&rp as *const ResolvePush as *const u8, 100);
            ctx.device.cmd_push_constants(cmd, resolve_layout, vk::ShaderStageFlags::FRAGMENT, 0, pbytes);
            ctx.device.cmd_draw(cmd, 3, 1, 0, 0);
            ctx.device.cmd_end_render_pass(cmd);
        });
    }

    /// Create or recreate the shadow descriptor pool sized for `light_count` lights.
    fn create_shadow_descriptor_pool(&mut self, light_count: usize) {
        // Destroy existing pool
        if let Some(pool) = self.shadow_descriptor_pool.take() {
            gpu_device::with_device(|ctx| {
                // SAFETY: pool is valid and no descriptor sets are in use
                unsafe { ctx.device.destroy_descriptor_pool(pool, None); }
            });
        }

        if light_count == 0 {
            return;
        }

        gpu_device::with_device(|ctx| {
            // SAFETY: Vulkan context valid.
            unsafe {
                let pool_sizes = [vk::DescriptorPoolSize {
                    ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                    descriptor_count: light_count as u32,
                }];
                let pool_info = vk::DescriptorPoolCreateInfo::default()
                    .pool_sizes(&pool_sizes)
                    .max_sets(light_count as u32)
                    .flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET);

                match ctx.device.create_descriptor_pool(&pool_info, None) {
                    Ok(pool) => {
                        self.shadow_descriptor_pool = Some(pool);
                    }
                    Err(e) => {
                        eprintln!("[SHADOW] Failed to create descriptor pool: {:?}", e);
                    }
                }
            }
        });
    }

    /// Create or retrieve the shadow sampler (linear, clamp-to-edge).
    fn ensure_shadow_sampler(&mut self) {
        if self.shadow_sampler.is_some() {
            return;
        }
        gpu_device::with_device(|ctx| {
            // SAFETY: Vulkan context valid.
            unsafe {
                let sampler_info = vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::LINEAR)
                    .min_filter(vk::Filter::LINEAR)
                    .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .min_lod(0.0)
                    .max_lod(0.0)
                    .border_color(vk::BorderColor::FLOAT_OPAQUE_WHITE);

                match ctx.device.create_sampler(&sampler_info, None) {
                    Ok(s) => { self.shadow_sampler = Some(s); }
                    Err(e) => { eprintln!("[SHADOW] Failed to create sampler: {:?}", e); }
                }
            }
        });
    }

    /// Build per-light shadow cubemaps.
    ///
    /// Renders the entire BSP geometry into a 256×256 D32_SFLOAT cubemap for
    /// each D3 light. Called once at level load via `update_d3_lights`.
    pub fn build_shadow_maps(
        &mut self,
        bsp_vbo: vk::Buffer,
        bsp_ibo: vk::Buffer,
        bsp_index_count: u32,
    ) {
        const SHADOW_RES: u32 = 256;

        // Bail early if no lights or if BSP not yet built (index_count==0 means
        // geometry hasn't been uploaded yet; building now would produce all-1.0
        // shadow maps that are never re-built, causing permanent full-brights).
        if self.d3_lights.is_empty() || bsp_vbo == vk::Buffer::null() || bsp_index_count == 0 {
            // BSP geometry not uploaded yet — retry on a later frame (silently).
            return;
        }

        // Ensure shared resources exist
        self.create_shadow_render_pass();
        self.ensure_shadow_sampler();

        // Create shadow pipeline (needs render pass to already exist)
        let render_pass = match self.shadow_render_pass {
            Some(rp) => rp,
            None => {
                return;
            }
        };

        // Create the shadow cubemap pipeline if not yet done
        if let Some(pm) = self.pipelines.as_mut() {
            if pm.create_shadow_pipeline(render_pass).is_err() {
                return;
            }
            // Also ensure the lit additive pipeline exists
            let _ = pm.create_pipeline(ShaderType::WorldLitAdditive, PipelineVariant::LitAdditive);
        }

        let shadow_sampler = match self.shadow_sampler {
            Some(s) => s,
            None => {
                return;
            }
        };

        // Get the set=2 descriptor layout (COMBINED_IMAGE_SAMPLER) from the pipeline manager.
        let ds_layout = match self.pipelines.as_ref().and_then(|pm| pm.lightmap_set_layout()) {
            Some(l) => l,
            None => {
                return;
            }
        };

        // Destroy previous shadow cubemaps
        self.destroy_shadow_cubemaps();

        // Create descriptor pool for all lights
        let light_count = self.d3_lights.len();
        self.create_shadow_descriptor_pool(light_count);

        let desc_pool = match self.shadow_descriptor_pool {
            Some(p) => p,
            None => {
                return;
            }
        };

        // Snapshot lights to avoid borrow conflicts
        let lights: Vec<GpuLightD3> = self.d3_lights.clone();

        // Get shadow pipeline
        let shadow_pipeline = self.pipelines.as_ref()
            .and_then(|pm| pm.get(ShaderType::ShadowCube, PipelineVariant::ShadowDepth))
            .map(|gp| (gp.pipeline, gp.layout));

        let (shadow_pip, shadow_pip_layout) = match shadow_pipeline {
            Some(pair) => pair,
            None => {
                return;
            }
        };

        for light in &lights {
            let light_pos = light.pos;
            let light_radius = light.radius;
            let mvps = Self::cubemap_face_mvps(light_pos, light_radius);

            // Allocate a ShadowCubemap inside gpu_device closures
            // We do all Vulkan calls in a single closure to avoid lock re-entrancy issues.
            // with_device_and_commands_mut returns Option<Option<ShadowCubemap>>;
            // .flatten() collapses to Option<ShadowCubemap>.
            let result: Option<ShadowCubemap> = gpu_device::with_device_and_commands_mut(|ctx, cmds| {
                // SAFETY: Vulkan context valid; single-threaded renderer.
                unsafe {
                    // -------------------------------------------------------
                    // Helper: allocate and bind device-local memory for an image.
                    // Returns (memory) or None on failure (image already destroyed by caller).
                    // -------------------------------------------------------
                    let alloc_image_mem = |img: vk::Image| -> Option<vk::DeviceMemory> {
                        let mem_reqs = ctx.device.get_image_memory_requirements(img);
                        let mem_type = Self::find_memory_type(
                            ctx, mem_reqs.memory_type_bits, vk::MemoryPropertyFlags::DEVICE_LOCAL,
                        )?;
                        let alloc_info = vk::MemoryAllocateInfo::default()
                            .allocation_size(mem_reqs.size)
                            .memory_type_index(mem_type);
                        ctx.device.allocate_memory(&alloc_info, None).ok()
                    };

                    // -------------------------------------------------------
                    // 1. R32_SFLOAT colour cubemap (6 layers) — stores dist/radius
                    // -------------------------------------------------------
                    let color_image_info = vk::ImageCreateInfo::default()
                        .image_type(vk::ImageType::TYPE_2D)
                        .format(vk::Format::R32_SFLOAT)
                        .extent(vk::Extent3D { width: SHADOW_RES, height: SHADOW_RES, depth: 1 })
                        .mip_levels(1)
                        .array_layers(6)
                        .samples(vk::SampleCountFlags::TYPE_1)
                        .tiling(vk::ImageTiling::OPTIMAL)
                        .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED)
                        .sharing_mode(vk::SharingMode::EXCLUSIVE)
                        .initial_layout(vk::ImageLayout::UNDEFINED)
                        .flags(vk::ImageCreateFlags::CUBE_COMPATIBLE);

                    let image = match ctx.device.create_image(&color_image_info, None) {
                        Ok(img) => img,
                        Err(e) => { eprintln!("[SHADOW] create colour image failed: {:?}", e); return None; }
                    };
                    let memory = match alloc_image_mem(image) {
                        Some(m) => m,
                        None => { eprintln!("[SHADOW] alloc colour memory failed"); ctx.device.destroy_image(image, None); return None; }
                    };
                    if let Err(e) = ctx.device.bind_image_memory(image, memory, 0) {
                        eprintln!("[SHADOW] bind colour image memory failed: {:?}", e);
                        ctx.device.free_memory(memory, None); ctx.device.destroy_image(image, None); return None;
                    }

                    // -------------------------------------------------------
                    // 2. D32_SFLOAT depth image (2D, single layer) — z-testing only
                    // -------------------------------------------------------
                    let depth_image_info = vk::ImageCreateInfo::default()
                        .image_type(vk::ImageType::TYPE_2D)
                        .format(vk::Format::D32_SFLOAT)
                        .extent(vk::Extent3D { width: SHADOW_RES, height: SHADOW_RES, depth: 1 })
                        .mip_levels(1)
                        .array_layers(1)
                        .samples(vk::SampleCountFlags::TYPE_1)
                        .tiling(vk::ImageTiling::OPTIMAL)
                        .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
                        .sharing_mode(vk::SharingMode::EXCLUSIVE)
                        .initial_layout(vk::ImageLayout::UNDEFINED);

                    let depth_image = match ctx.device.create_image(&depth_image_info, None) {
                        Ok(img) => img,
                        Err(e) => {
                            eprintln!("[SHADOW] create depth image failed: {:?}", e);
                            ctx.device.free_memory(memory, None); ctx.device.destroy_image(image, None); return None;
                        }
                    };
                    let depth_memory = match alloc_image_mem(depth_image) {
                        Some(m) => m,
                        None => {
                            eprintln!("[SHADOW] alloc depth memory failed");
                            ctx.device.destroy_image(depth_image, None);
                            ctx.device.free_memory(memory, None); ctx.device.destroy_image(image, None); return None;
                        }
                    };
                    if let Err(e) = ctx.device.bind_image_memory(depth_image, depth_memory, 0) {
                        eprintln!("[SHADOW] bind depth memory failed: {:?}", e);
                        ctx.device.free_memory(depth_memory, None); ctx.device.destroy_image(depth_image, None);
                        ctx.device.free_memory(memory, None); ctx.device.destroy_image(image, None); return None;
                    }

                    // -------------------------------------------------------
                    // 3. Create image views
                    // -------------------------------------------------------
                    // R32_SFLOAT cube view (for samplerCube sampling at set=2)
                    let cube_view_info = vk::ImageViewCreateInfo::default()
                        .image(image)
                        .view_type(vk::ImageViewType::CUBE)
                        .format(vk::Format::R32_SFLOAT)
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0, level_count: 1,
                            base_array_layer: 0, layer_count: 6,
                        });
                    let cube_view = match ctx.device.create_image_view(&cube_view_info, None) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("[SHADOW] create cube view failed: {:?}", e);
                            ctx.device.free_memory(depth_memory, None); ctx.device.destroy_image(depth_image, None);
                            ctx.device.free_memory(memory, None); ctx.device.destroy_image(image, None); return None;
                        }
                    };

                    // D32_SFLOAT depth view (single layer, for framebuffer depth attachment)
                    let depth_view_info = vk::ImageViewCreateInfo::default()
                        .image(depth_image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(vk::Format::D32_SFLOAT)
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::DEPTH,
                            base_mip_level: 0, level_count: 1,
                            base_array_layer: 0, layer_count: 1,
                        });
                    let depth_view = match ctx.device.create_image_view(&depth_view_info, None) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("[SHADOW] create depth view failed: {:?}", e);
                            ctx.device.destroy_image_view(cube_view, None);
                            ctx.device.free_memory(depth_memory, None); ctx.device.destroy_image(depth_image, None);
                            ctx.device.free_memory(memory, None); ctx.device.destroy_image(image, None); return None;
                        }
                    };

                    // R32_SFLOAT per-face 2D colour views (for framebuffer colour attachments)
                    let mut face_views = [vk::ImageView::null(); 6];
                    for face in 0..6u32 {
                        let view_info = vk::ImageViewCreateInfo::default()
                            .image(image)
                            .view_type(vk::ImageViewType::TYPE_2D)
                            .format(vk::Format::R32_SFLOAT)
                            .subresource_range(vk::ImageSubresourceRange {
                                aspect_mask: vk::ImageAspectFlags::COLOR,
                                base_mip_level: 0, level_count: 1,
                                base_array_layer: face, layer_count: 1,
                            });
                        face_views[face as usize] = match ctx.device.create_image_view(&view_info, None) {
                            Ok(v) => v,
                            Err(e) => {
                                eprintln!("[SHADOW] create face view {} failed: {:?}", face, e);
                                for j in 0..face as usize { ctx.device.destroy_image_view(face_views[j], None); }
                                ctx.device.destroy_image_view(depth_view, None);
                                ctx.device.destroy_image_view(cube_view, None);
                                ctx.device.free_memory(depth_memory, None); ctx.device.destroy_image(depth_image, None);
                                ctx.device.free_memory(memory, None); ctx.device.destroy_image(image, None); return None;
                            }
                        };
                    }

                    // -------------------------------------------------------
                    // 4. Create 6 framebuffers (colour face view + depth view)
                    // -------------------------------------------------------
                    let mut framebuffers = [vk::Framebuffer::null(); 6];
                    for face in 0..6usize {
                        let attachments = [face_views[face], depth_view];
                        let fb_info = vk::FramebufferCreateInfo::default()
                            .render_pass(render_pass)
                            .attachments(&attachments)
                            .width(SHADOW_RES)
                            .height(SHADOW_RES)
                            .layers(1);
                        framebuffers[face] = match ctx.device.create_framebuffer(&fb_info, None) {
                            Ok(fb) => fb,
                            Err(e) => {
                                eprintln!("[SHADOW] create_framebuffer face {} failed: {:?}", face, e);
                                for j in 0..face { ctx.device.destroy_framebuffer(framebuffers[j], None); }
                                for fv in &face_views { ctx.device.destroy_image_view(*fv, None); }
                                ctx.device.destroy_image_view(depth_view, None);
                                ctx.device.destroy_image_view(cube_view, None);
                                ctx.device.free_memory(depth_memory, None); ctx.device.destroy_image(depth_image, None);
                                ctx.device.free_memory(memory, None); ctx.device.destroy_image(image, None); return None;
                            }
                        };
                    }

                    // -------------------------------------------------------
                    // 5. Allocate descriptor set (set=2, samplerCube)
                    // -------------------------------------------------------
                    let layouts = [ds_layout];
                    let ds_alloc_info = vk::DescriptorSetAllocateInfo::default()
                        .descriptor_pool(desc_pool)
                        .set_layouts(&layouts);
                    let descriptor_set = match ctx.device.allocate_descriptor_sets(&ds_alloc_info) {
                        Ok(sets) => sets[0],
                        Err(e) => {
                            eprintln!("[SHADOW] allocate_descriptor_sets failed: {:?}", e);
                            for fb in &framebuffers { ctx.device.destroy_framebuffer(*fb, None); }
                            for fv in &face_views { ctx.device.destroy_image_view(*fv, None); }
                            ctx.device.destroy_image_view(depth_view, None);
                            ctx.device.destroy_image_view(cube_view, None);
                            ctx.device.free_memory(depth_memory, None); ctx.device.destroy_image(depth_image, None);
                            ctx.device.free_memory(memory, None); ctx.device.destroy_image(image, None); return None;
                        }
                    };

                    // Bind the R32_SFLOAT cube view to set=2.
                    // R32_SFLOAT is a colour format → use SHADER_READ_ONLY_OPTIMAL (not depth layout).
                    let desc_image_info = vk::DescriptorImageInfo::default()
                        .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                        .image_view(cube_view)
                        .sampler(shadow_sampler);
                    let write = vk::WriteDescriptorSet::default()
                        .dst_set(descriptor_set)
                        .dst_binding(0)
                        .dst_array_element(0)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .image_info(std::slice::from_ref(&desc_image_info));
                    ctx.device.update_descriptor_sets(&[write], &[]);

                    // -------------------------------------------------------
                    // 6. Record & submit shadow render pass for all 6 faces
                    // -------------------------------------------------------
                    let cmd = match cmds.begin_single_time() {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!("[SHADOW] begin_single_time failed: {}", e);
                            let _ = ctx.device.free_descriptor_sets(desc_pool, &[descriptor_set]);
                            for fb in &framebuffers { ctx.device.destroy_framebuffer(*fb, None); }
                            for fv in &face_views { ctx.device.destroy_image_view(*fv, None); }
                            ctx.device.destroy_image_view(depth_view, None);
                            ctx.device.destroy_image_view(cube_view, None);
                            ctx.device.free_memory(depth_memory, None); ctx.device.destroy_image(depth_image, None);
                            ctx.device.free_memory(memory, None); ctx.device.destroy_image(image, None); return None;
                        }
                    };

                    // Transition colour cubemap (all 6 layers): UNDEFINED → COLOR_ATTACHMENT_OPTIMAL
                    let color_to_attachment = vk::ImageMemoryBarrier::default()
                        .old_layout(vk::ImageLayout::UNDEFINED)
                        .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                        .image(image)
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0, level_count: 1,
                            base_array_layer: 0, layer_count: 6,
                        })
                        .src_access_mask(vk::AccessFlags::empty())
                        .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE);

                    // Transition depth image: UNDEFINED → DEPTH_STENCIL_ATTACHMENT_OPTIMAL
                    let depth_to_attachment = vk::ImageMemoryBarrier::default()
                        .old_layout(vk::ImageLayout::UNDEFINED)
                        .new_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                        .image(depth_image)
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::DEPTH,
                            base_mip_level: 0, level_count: 1,
                            base_array_layer: 0, layer_count: 1,
                        })
                        .src_access_mask(vk::AccessFlags::empty())
                        .dst_access_mask(vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE);

                    let pre_barriers = [color_to_attachment, depth_to_attachment];
                    ctx.device.cmd_pipeline_barrier(
                        cmd,
                        vk::PipelineStageFlags::TOP_OF_PIPE,
                        vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                            | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
                        vk::DependencyFlags::empty(),
                        &[], &[], &pre_barriers,
                    );

                    // Render each cubemap face
                    for face in 0..6usize {
                        // Clear colour to 1.0 = "no occluder in this direction" (fully lit).
                        // Geometry writes dist/radius < 1.0 so fragments closer to the light
                        // pass the shadow test; directions with no geometry remain lit.
                        let clear_values = [
                            vk::ClearValue { color: vk::ClearColorValue { float32: [1.0, 0.0, 0.0, 0.0] } },
                            vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 } },
                        ];

                        let render_pass_begin = vk::RenderPassBeginInfo::default()
                            .render_pass(render_pass)
                            .framebuffer(framebuffers[face])
                            .render_area(vk::Rect2D {
                                offset: vk::Offset2D { x: 0, y: 0 },
                                extent: vk::Extent2D { width: SHADOW_RES, height: SHADOW_RES },
                            })
                            .clear_values(&clear_values);

                        ctx.device.cmd_begin_render_pass(cmd, &render_pass_begin, vk::SubpassContents::INLINE);
                        ctx.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, shadow_pip);

                        let viewport = vk::Viewport {
                            x: 0.0, y: 0.0,
                            width: SHADOW_RES as f32, height: SHADOW_RES as f32,
                            min_depth: 0.0, max_depth: 1.0,
                        };
                        let scissor = vk::Rect2D {
                            offset: vk::Offset2D { x: 0, y: 0 },
                            extent: vk::Extent2D { width: SHADOW_RES, height: SHADOW_RES },
                        };
                        ctx.device.cmd_set_viewport(cmd, 0, std::slice::from_ref(&viewport));
                        ctx.device.cmd_set_scissor(cmd, 0, std::slice::from_ref(&scissor));

                        let push = ShadowCubePushConstants { mvp: mvps[face], light_pos, light_radius };
                        let push_bytes = std::slice::from_raw_parts(
                            &push as *const ShadowCubePushConstants as *const u8,
                            std::mem::size_of::<ShadowCubePushConstants>(),
                        );
                        ctx.device.cmd_push_constants(
                            cmd, shadow_pip_layout,
                            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                            0, push_bytes,
                        );

                        ctx.device.cmd_bind_vertex_buffers(cmd, 0, &[bsp_vbo], &[0]);
                        ctx.device.cmd_bind_index_buffer(cmd, bsp_ibo, 0, vk::IndexType::UINT32);
                        if face == 0 {
                            eprintln!("[SHADOW] face0 draw: indices={} vbo={:?} ibo={:?}",
                                bsp_index_count, bsp_vbo, bsp_ibo);
                        }
                        ctx.device.cmd_draw_indexed(cmd, bsp_index_count, 1, 0, 0, 0);

                        ctx.device.cmd_end_render_pass(cmd);
                    }

                    // Transition colour cubemap: COLOR_ATTACHMENT_OPTIMAL → SHADER_READ_ONLY_OPTIMAL
                    let color_to_shader = vk::ImageMemoryBarrier::default()
                        .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                        .image(image)
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0, level_count: 1,
                            base_array_layer: 0, layer_count: 6,
                        })
                        .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                        .dst_access_mask(vk::AccessFlags::SHADER_READ);

                    ctx.device.cmd_pipeline_barrier(
                        cmd,
                        vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                        vk::PipelineStageFlags::FRAGMENT_SHADER,
                        vk::DependencyFlags::empty(),
                        &[], &[], std::slice::from_ref(&color_to_shader),
                    );

                    if let Err(e) = cmds.end_single_time(ctx, cmd) {
                        eprintln!("[SHADOW] end_single_time failed: {}", e);
                        let _ = ctx.device.free_descriptor_sets(desc_pool, &[descriptor_set]);
                        for fb in &framebuffers { ctx.device.destroy_framebuffer(*fb, None); }
                        for fv in &face_views { ctx.device.destroy_image_view(*fv, None); }
                        ctx.device.destroy_image_view(depth_view, None);
                        ctx.device.destroy_image_view(cube_view, None);
                        ctx.device.free_memory(depth_memory, None); ctx.device.destroy_image(depth_image, None);
                        ctx.device.free_memory(memory, None); ctx.device.destroy_image(image, None); return None;
                    }

                    Some(ShadowCubemap {
                        image, memory, cube_view, face_views,
                        depth_image, depth_memory, depth_view,
                        framebuffers, descriptor_set,
                        resolution: SHADOW_RES,
                    })
                }
            }).flatten();

            if let Some(sc) = result {
                self.shadow_cubemaps.push(sc);
            }
        }
    }

    /// Draw all D3 lights with their shadow cubemaps onto the given command buffer.
    ///
    /// For each light, binds the shadow cubemap descriptor set at set=2 and issues
    /// additive lit draw calls for all visible BSP batches.
    ///
    /// `visible_batches`: (first_index, index_count, diffuse_descriptor_set)
    /// Draw all D3 per-pixel lights (additive pass).
    ///
    /// Must be called from within a `gpu_device::with_device` closure — takes `ctx`
    /// directly to avoid re-entrant mutex acquisition.
    ///
    /// # Safety
    /// `cmd` must be a valid, recording command buffer.
    unsafe fn flush_doom3_lights_with_ctx(
        &self,
        ctx: &crate::vulkan::VulkanContext,
        cmd: vk::CommandBuffer,
        bsp_vbo: vk::Buffer,
        bsp_ibo: vk::Buffer,
        visible_batches: &[(u32, u32, Option<vk::DescriptorSet>, bool)],
        mvp: [f32; 16],
        view_origin: [f32; 3],
    ) {
        if self.d3_lights.is_empty() || visible_batches.is_empty() {
            return;
        }

        let lit_pipeline = self.pipelines.as_ref()
            .and_then(|pm| pm.get(ShaderType::WorldLitAdditive, PipelineVariant::LitAdditive))
            .map(|gp| (gp.pipeline, gp.layout));

        let (lit_pip, lit_pip_layout) = match lit_pipeline {
            Some(pair) => pair,
            None => return,
        };

        // Dark-fill needs the baked lightmap at set 3. If it isn't ready, skip the whole pass
        // rather than sample an unbound set.
        let lightmap_ds = match self.lightmap_descriptor_set() {
            Some(ds) => ds,
            None => return,
        };
        // Set 3 (lightmap) is constant for every light/batch; bind it once up front. Sets 1
        // (diffuse) and 2 (shadow cubemap) are rebound per batch/light below.
        ctx.device.cmd_bind_descriptor_sets(
            cmd, vk::PipelineBindPoint::GRAPHICS,
            lit_pip_layout, 3, &[lightmap_ds], &[],
        );

        for (light_idx, light) in self.d3_lights.iter().enumerate() {
            // Shadow cubemap is required: the shader always samples set=2.
            // Skip lights that don't have a cubemap yet (build still pending).
            let shadow_ds = match self.shadow_cubemaps.get(light_idx)
                .map(|sc| sc.descriptor_set)
            {
                Some(ds) => ds,
                None => continue,
            };

            ctx.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, lit_pip);

            ctx.device.cmd_bind_descriptor_sets(
                cmd, vk::PipelineBindPoint::GRAPHICS,
                lit_pip_layout, 2, &[shadow_ds], &[],
            );

            ctx.device.cmd_bind_vertex_buffers(cmd, 0, &[bsp_vbo], &[0]);
            ctx.device.cmd_bind_index_buffer(cmd, bsp_ibo, 0, vk::IndexType::UINT32);

            for &(first_index, index_count, diffuse_ds, _is_light) in visible_batches {
                if let Some(ds) = diffuse_ds {
                    ctx.device.cmd_bind_descriptor_sets(
                        cmd, vk::PipelineBindPoint::GRAPHICS,
                        lit_pip_layout, 1, &[ds], &[],
                    );
                }

                let push = D3LitPushConstants {
                    mvp,
                    light_pos: light.pos,
                    light_radius: light.radius,
                    light_color: light.color,
                    light_intensity: light.intensity,
                    spec_power: light.spec_pow,
                    // Larger bias to stop cubemap self-shadowing acne (the "red lines"
                    // along surfaces lit at grazing angles).
                    shadow_bias: 0.04,
                    _pad1: 0.0,
                    _pad2: 0.0,
                    view_origin,
                };

                let push_bytes = std::slice::from_raw_parts(
                    &push as *const D3LitPushConstants as *const u8,
                    std::mem::size_of::<D3LitPushConstants>(),
                );

                ctx.device.cmd_push_constants(
                    cmd, lit_pip_layout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    0, push_bytes,
                );

                ctx.device.cmd_draw_indexed(cmd, index_count, 1, first_index, 0, 0);
            }
        }
    }

    /// Cleanup shadow cubemap resources (call before level unload or shutdown).
    pub fn shutdown_shadow_maps(&mut self) {
        self.destroy_shadow_cubemaps();

        if let Some(pool) = self.shadow_descriptor_pool.take() {
            gpu_device::with_device(|ctx| {
                // SAFETY: pool is valid and no descriptor sets remain.
                unsafe { ctx.device.destroy_descriptor_pool(pool, None); }
            });
        }

        if let Some(sampler) = self.shadow_sampler.take() {
            gpu_device::with_device(|ctx| {
                // SAFETY: sampler is valid and not in use.
                unsafe { ctx.device.destroy_sampler(sampler, None); }
            });
        }

        if let Some(rp) = self.shadow_render_pass.take() {
            gpu_device::with_device(|ctx| {
                // SAFETY: render pass is valid and not in use.
                unsafe { ctx.device.destroy_render_pass(rp, None); }
            });
        }

        // Projective dynamic-shadow resources.
        let ps = std::mem::take(&mut self.projective_shadow);
        gpu_device::with_device(|ctx| unsafe {
            // SAFETY: all handles are valid (or None) and not in use at shutdown.
            if let Some(x) = ps.framebuffer { ctx.device.destroy_framebuffer(x, None); }
            if let Some(x) = ps.resolve_fb { ctx.device.destroy_framebuffer(x, None); }
            if let Some(x) = ps.color_view { ctx.device.destroy_image_view(x, None); }
            if let Some(x) = ps.depth_view { ctx.device.destroy_image_view(x, None); }
            if let Some(x) = ps.depth_sample_view { ctx.device.destroy_image_view(x, None); }
            if let Some(x) = ps.color_image { ctx.device.destroy_image(x, None); }
            if let Some(x) = ps.depth_image { ctx.device.destroy_image(x, None); }
            if let Some(x) = ps.color_mem { ctx.device.free_memory(x, None); }
            if let Some(x) = ps.depth_mem { ctx.device.free_memory(x, None); }
            if let Some(x) = ps.resolve_rp { ctx.device.destroy_render_pass(x, None); }
            if let Some(x) = ps.caster_rp { ctx.device.destroy_render_pass(x, None); }
            if let Some(x) = ps.resolve_pool { ctx.device.destroy_descriptor_pool(x, None); }
            if let Some(x) = ps.sampler { ctx.device.destroy_sampler(x, None); }
        });
    }
}

impl Default for ModernRenderPath {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================
//  Tests
// =============================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: approximately compare two f32 values.
    fn approx_eq(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() < eps
    }

    /// Helper: approximately compare two 16-element column-major matrices.
    fn mat_approx_eq(a: &[f32; 16], b: &[f32; 16], eps: f32) -> bool {
        a.iter().zip(b.iter()).all(|(x, y)| approx_eq(*x, *y, eps))
    }

    // ---------------------------------------------------------
    //  mat4_multiply
    // ---------------------------------------------------------

    #[test]
    fn test_mat4_multiply_identity() {
        #[rustfmt::skip]
        let identity: [f32; 16] = [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ];
        let result = ModernRenderPath::mat4_multiply(&identity, &identity);
        assert!(mat_approx_eq(&result, &identity, 1e-6));
    }

    #[test]
    fn test_mat4_multiply_identity_left() {
        #[rustfmt::skip]
        let identity: [f32; 16] = [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ];
        // Column-major: translation of (3, 5, 7)
        #[rustfmt::skip]
        let trans: [f32; 16] = [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            3.0, 5.0, 7.0, 1.0,
        ];
        let result = ModernRenderPath::mat4_multiply(&identity, &trans);
        assert!(mat_approx_eq(&result, &trans, 1e-6));
    }

    #[test]
    fn test_mat4_multiply_identity_right() {
        #[rustfmt::skip]
        let identity: [f32; 16] = [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ];
        #[rustfmt::skip]
        let trans: [f32; 16] = [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            3.0, 5.0, 7.0, 1.0,
        ];
        let result = ModernRenderPath::mat4_multiply(&trans, &identity);
        assert!(mat_approx_eq(&result, &trans, 1e-6));
    }

    #[test]
    fn test_mat4_multiply_scale() {
        // Scale by 2 on all axes
        #[rustfmt::skip]
        let scale: [f32; 16] = [
            2.0, 0.0, 0.0, 0.0,
            0.0, 2.0, 0.0, 0.0,
            0.0, 0.0, 2.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ];
        let result = ModernRenderPath::mat4_multiply(&scale, &scale);
        // Scale 2 * Scale 2 = Scale 4
        assert!(approx_eq(result[0], 4.0, 1e-6));
        assert!(approx_eq(result[5], 4.0, 1e-6));
        assert!(approx_eq(result[10], 4.0, 1e-6));
        assert!(approx_eq(result[15], 1.0, 1e-6));
    }

    #[test]
    fn test_mat4_multiply_translation_composition() {
        // T1: translate by (1, 0, 0)
        #[rustfmt::skip]
        let t1: [f32; 16] = [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            1.0, 0.0, 0.0, 1.0,
        ];
        // T2: translate by (0, 2, 0)
        #[rustfmt::skip]
        let t2: [f32; 16] = [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 2.0, 0.0, 1.0,
        ];
        let result = ModernRenderPath::mat4_multiply(&t1, &t2);
        // Result: translate by (1, 2, 0)
        assert!(approx_eq(result[12], 1.0, 1e-6)); // tx
        assert!(approx_eq(result[13], 2.0, 1e-6)); // ty
        assert!(approx_eq(result[14], 0.0, 1e-6)); // tz
    }

    // ---------------------------------------------------------
    //  to_mat4x4
    // ---------------------------------------------------------

    #[test]
    fn test_to_mat4x4_identity() {
        #[rustfmt::skip]
        let flat: [f32; 16] = [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ];
        let mat = ModernRenderPath::to_mat4x4(&flat);
        // Column 0
        assert_eq!(mat[0], [1.0, 0.0, 0.0, 0.0]);
        // Column 1
        assert_eq!(mat[1], [0.0, 1.0, 0.0, 0.0]);
        // Column 2
        assert_eq!(mat[2], [0.0, 0.0, 1.0, 0.0]);
        // Column 3
        assert_eq!(mat[3], [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_to_mat4x4_translation() {
        #[rustfmt::skip]
        let flat: [f32; 16] = [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            5.0, 6.0, 7.0, 1.0,
        ];
        let mat = ModernRenderPath::to_mat4x4(&flat);
        // Translation is in column 3
        assert_eq!(mat[3], [5.0, 6.0, 7.0, 1.0]);
    }

    #[test]
    fn test_to_mat4x4_roundtrip() {
        // Verify that elements map correctly: flat[col*4+row] == mat[col][row]
        let mut flat = [0.0f32; 16];
        for i in 0..16 {
            flat[i] = i as f32;
        }
        let mat = ModernRenderPath::to_mat4x4(&flat);
        for col in 0..4 {
            for row in 0..4 {
                assert_eq!(mat[col][row], flat[col * 4 + row],
                    "mismatch at [{col}][{row}]");
            }
        }
    }

    // ---------------------------------------------------------
    //  compute_view_matrix
    // ---------------------------------------------------------

    #[test]
    fn test_view_matrix_at_origin_no_rotation() {
        let vieworg = [0.0, 0.0, 0.0];
        let viewangles = [0.0, 0.0, 0.0]; // pitch=0, yaw=0, roll=0
        let view = ModernRenderPath::compute_view_matrix(&vieworg, &viewangles);

        // At origin with no rotation:
        // Forward = [1, 0, 0] (yaw=0, pitch=0)
        // Right = [0, 1, 0] (from cross product)
        // Up = [0, 0, 1]
        // Translation should be [0, 0, 0]
        // Last column (translation): all zeros since origin is [0,0,0]
        assert!(approx_eq(view[12], 0.0, 1e-4));
        assert!(approx_eq(view[13], 0.0, 1e-4));
        assert!(approx_eq(view[14], 0.0, 1e-4));
        assert!(approx_eq(view[15], 1.0, 1e-4));
    }

    #[test]
    fn test_view_matrix_translation_only() {
        let vieworg = [10.0, 20.0, 30.0];
        let viewangles = [0.0, 0.0, 0.0];
        let view = ModernRenderPath::compute_view_matrix(&vieworg, &viewangles);

        // With pitch=0, yaw=0, roll=0:
        // forward = [1, 0, 0], right = [0, -1, 0], up = [0, 0, 1]
        // Actually with the Quake convention and the formula:
        // sp=0, cp=1, sy=0, cy=1, sr=0, cr=1
        // forward = [1, 0, 0]
        // right = [0*0*1 + 1*0, 0*0*0 - 1*1, 0*1] = [0, -1, 0]
        // up = [1*0*1 + 0*0, 1*0*0 - 0*1, 1*1] = [0, 0, 1]
        // tx = -(right.vieworg) = -(0*10 + (-1)*20 + 0*30) = 20
        // ty = -(up.vieworg) = -(0*10 + 0*20 + 1*30) = -30
        // Row 2 of the rotation is -forward, so its translation is +(fwd.vieworg)
        // (this is what maps the eye position to the view-space origin).
        // tz = +(fwd.vieworg) = (1*10 + 0*20 + 0*30) = 10
        assert!(approx_eq(view[12], 20.0, 1e-4), "tx={}", view[12]);
        assert!(approx_eq(view[13], -30.0, 1e-4), "ty={}", view[13]);
        assert!(approx_eq(view[14], 10.0, 1e-4), "tz={}", view[14]);
    }

    #[test]
    fn test_view_matrix_is_orthogonal_rotation() {
        // For any valid rotation, the upper-left 3x3 should be orthogonal
        // (columns are unit vectors, mutually perpendicular)
        let vieworg = [0.0, 0.0, 0.0];
        let viewangles = [15.0, 45.0, 10.0];
        let view = ModernRenderPath::compute_view_matrix(&vieworg, &viewangles);

        // Extract columns (column-major layout)
        let col0 = [view[0], view[1], view[2]];
        let col1 = [view[4], view[5], view[6]];
        let col2 = [view[8], view[9], view[10]];

        // Each column should be unit length
        let len0 = (col0[0]*col0[0] + col0[1]*col0[1] + col0[2]*col0[2]).sqrt();
        let len1 = (col1[0]*col1[0] + col1[1]*col1[1] + col1[2]*col1[2]).sqrt();
        let len2 = (col2[0]*col2[0] + col2[1]*col2[1] + col2[2]*col2[2]).sqrt();
        assert!(approx_eq(len0, 1.0, 1e-4), "col0 length = {}", len0);
        assert!(approx_eq(len1, 1.0, 1e-4), "col1 length = {}", len1);
        assert!(approx_eq(len2, 1.0, 1e-4), "col2 length = {}", len2);

        // Columns should be mutually perpendicular (dot product ~ 0)
        let dot01 = col0[0]*col1[0] + col0[1]*col1[1] + col0[2]*col1[2];
        let dot02 = col0[0]*col2[0] + col0[1]*col2[1] + col0[2]*col2[2];
        let dot12 = col1[0]*col2[0] + col1[1]*col2[1] + col1[2]*col2[2];
        assert!(approx_eq(dot01, 0.0, 1e-4), "dot(col0,col1) = {}", dot01);
        assert!(approx_eq(dot02, 0.0, 1e-4), "dot(col0,col2) = {}", dot02);
        assert!(approx_eq(dot12, 0.0, 1e-4), "dot(col1,col2) = {}", dot12);
    }

    #[test]
    fn test_view_matrix_yaw_90() {
        let vieworg = [0.0, 0.0, 0.0];
        let viewangles = [0.0, 90.0, 0.0]; // 90 degree yaw
        let view = ModernRenderPath::compute_view_matrix(&vieworg, &viewangles);

        // With yaw=90: forward = [0, 1, 0]
        // The -forward row in the view matrix (row 2 in column-major) should be [0, -1, 0]
        // view[2] = -forward[0], view[6] = -forward[1], view[10] = -forward[2]
        assert!(approx_eq(view[2], 0.0, 1e-4), "view[2]={}", view[2]);
        assert!(approx_eq(view[6], -1.0, 1e-4), "view[6]={}", view[6]);
        assert!(approx_eq(view[10], 0.0, 1e-4), "view[10]={}", view[10]);
    }

    // ---------------------------------------------------------
    //  compute_projection_matrix
    // ---------------------------------------------------------

    #[test]
    fn test_projection_matrix_is_perspective() {
        let proj = ModernRenderPath::compute_projection_matrix(90.0, 73.74, 4.0, 4096.0);

        // For a perspective matrix, [3] (column 0, row 3) should be 0
        assert!(approx_eq(proj[3], 0.0, 1e-6));
        // [7] (column 1, row 3) should be 0
        assert!(approx_eq(proj[7], 0.0, 1e-6));
        // [11] (column 2, row 3) should be -1 (perspective divide)
        assert!(approx_eq(proj[11], -1.0, 1e-6));
        // [15] (column 3, row 3) should be 0
        assert!(approx_eq(proj[15], 0.0, 1e-6));
    }

    #[test]
    fn test_projection_matrix_diagonal_positive() {
        let proj = ModernRenderPath::compute_projection_matrix(90.0, 73.74, 4.0, 4096.0);
        // [0] (a) and [5] (b) should be positive
        assert!(proj[0] > 0.0, "proj[0]={}", proj[0]);
        assert!(proj[5] > 0.0, "proj[5]={}", proj[5]);
    }

    #[test]
    fn test_projection_matrix_fov_effect() {
        // Wider FOV = smaller diagonal value (less zoom)
        let proj_wide = ModernRenderPath::compute_projection_matrix(120.0, 90.0, 4.0, 4096.0);
        let proj_narrow = ModernRenderPath::compute_projection_matrix(60.0, 45.0, 4.0, 4096.0);
        assert!(proj_narrow[0] > proj_wide[0], "narrow FOV should have larger a");
        assert!(proj_narrow[5] > proj_wide[5], "narrow FOV should have larger b");
    }

    #[test]
    fn test_projection_matrix_near_far_planes() {
        let near = 4.0f32;
        let far = 4096.0f32;
        let proj = ModernRenderPath::compute_projection_matrix(90.0, 73.74, near, far);

        // Vulkan depth range [0, 1] (NOT OpenGL [-1, 1]):
        // c = -far/(far-near)
        let expected_c = -far / (far - near);
        assert!(approx_eq(proj[10], expected_c, 1e-4), "c={}, expected {}", proj[10], expected_c);

        // d = -(near*far)/(far-near)
        let expected_d = -(near * far) / (far - near);
        assert!(approx_eq(proj[14], expected_d, 1e-2), "d={}, expected {}", proj[14], expected_d);
    }

    // ---------------------------------------------------------
    //  compute_ortho_matrix
    // ---------------------------------------------------------

    #[test]
    fn test_ortho_matrix_basic() {
        let ortho = ModernRenderPath::compute_ortho_matrix(640.0, 480.0);

        // [0] = 2.0 / width
        assert!(approx_eq(ortho[0], 2.0 / 640.0, 1e-6));
        // [5] = -2.0 / height
        assert!(approx_eq(ortho[5], -2.0 / 480.0, 1e-6));
        // [10] = -1.0
        assert!(approx_eq(ortho[10], -1.0, 1e-6));
        // Translation: [-1, 1, 0, 1]
        assert!(approx_eq(ortho[12], -1.0, 1e-6));
        assert!(approx_eq(ortho[13], 1.0, 1e-6));
        assert!(approx_eq(ortho[14], 0.0, 1e-6));
        assert!(approx_eq(ortho[15], 1.0, 1e-6));
    }

    #[test]
    fn test_ortho_matrix_maps_corners() {
        // The ortho matrix maps (0,0) to (-1,1) and (width,height) to (1,-1)
        let w = 800.0f32;
        let h = 600.0f32;
        let ortho = ModernRenderPath::compute_ortho_matrix(w, h);

        // Transform point (0, 0, 0, 1) - column-major: result = ortho * point
        let p0_x = ortho[0] * 0.0 + ortho[4] * 0.0 + ortho[8] * 0.0 + ortho[12] * 1.0;
        let p0_y = ortho[1] * 0.0 + ortho[5] * 0.0 + ortho[9] * 0.0 + ortho[13] * 1.0;
        assert!(approx_eq(p0_x, -1.0, 1e-4), "origin X = {}", p0_x);
        assert!(approx_eq(p0_y, 1.0, 1e-4), "origin Y = {}", p0_y);

        // Transform point (width, height, 0, 1)
        let pw_x = ortho[0] * w + ortho[4] * h + ortho[8] * 0.0 + ortho[12] * 1.0;
        let pw_y = ortho[1] * w + ortho[5] * h + ortho[9] * 0.0 + ortho[13] * 1.0;
        assert!(approx_eq(pw_x, 1.0, 1e-4), "far corner X = {}", pw_x);
        assert!(approx_eq(pw_y, -1.0, 1e-4), "far corner Y = {}", pw_y);
    }

    #[test]
    fn test_ortho_matrix_off_diagonals_zero() {
        let ortho = ModernRenderPath::compute_ortho_matrix(1024.0, 768.0);
        // Off-diagonal elements of the upper-left 3x3 should be zero
        assert!(approx_eq(ortho[1], 0.0, 1e-6));
        assert!(approx_eq(ortho[2], 0.0, 1e-6));
        assert!(approx_eq(ortho[4], 0.0, 1e-6));
        assert!(approx_eq(ortho[6], 0.0, 1e-6));
        assert!(approx_eq(ortho[8], 0.0, 1e-6));
        assert!(approx_eq(ortho[9], 0.0, 1e-6));
    }

    // ---------------------------------------------------------
    //  Matrix multiplication with projection and view
    // ---------------------------------------------------------

    #[test]
    fn test_view_projection_multiply() {
        // Basic sanity: multiplying valid view and projection matrices
        // should produce a valid (finite, non-NaN) result
        let view = ModernRenderPath::compute_view_matrix(
            &[100.0, 200.0, 50.0],
            &[10.0, 45.0, 0.0],
        );
        let proj = ModernRenderPath::compute_projection_matrix(90.0, 73.74, 4.0, 4096.0);
        let vp = ModernRenderPath::mat4_multiply(&proj, &view);

        for (i, val) in vp.iter().enumerate() {
            assert!(val.is_finite(), "VP[{}] is not finite: {}", i, val);
        }
    }

    // ---------------------------------------------------------
    //  ModernRenderPath::new defaults
    // ---------------------------------------------------------

    #[test]
    fn test_modern_render_path_new_defaults() {
        let path = ModernRenderPath::new();
        assert_eq!(path.width, 640);
        assert_eq!(path.height, 480);
        assert!(!path.initialized);
        assert!(!path.frame_in_progress);
    }

    #[test]
    fn test_modern_render_path_set_dimensions() {
        let mut path = ModernRenderPath::new();
        path.width = 1920;
        path.height = 1080;
        assert_eq!(path.width, 1920);
        assert_eq!(path.height, 1080);
    }
}

impl RenderPath for ModernRenderPath {
    fn init(&mut self) -> Result<(), RenderError> {
        myq2_common::common::com_printf("ModernRenderPath::init: *** ENTERED INIT FUNCTION ***\n");
        myq2_common::common::com_printf("ModernRenderPath::init: Creating ShaderManager\n");
        // Compile all shaders
        myq2_common::common::com_printf("ModernRenderPath::init: Calling ShaderManager::new()\n");
        self.shaders = Some(ShaderManager::new()?);
        myq2_common::common::com_printf("ModernRenderPath::init: ShaderManager::new() returned\n");
        myq2_common::common::com_printf("ModernRenderPath::init: ShaderManager created\n");

        myq2_common::common::com_printf("ModernRenderPath::init: Creating UBOs\n");
        // Create UBOs
        self.per_frame_ubo = Some(UniformBuffer::new(0));
        self.per_object_ubo = Some(UniformBuffer::new(1));
        myq2_common::common::com_printf("ModernRenderPath::init: UBOs created\n");

        myq2_common::common::com_printf("ModernRenderPath::init: Creating PostProcessor\n");
        // Create post-processor
        self.post_processor = Some(PostProcessor::new(self.width, self.height));
        myq2_common::common::com_printf("ModernRenderPath::init: PostProcessor created\n");

        myq2_common::common::com_printf("ModernRenderPath::init: Initializing EDS3 commands\n");
        // Initialize EDS3 commands for vk_showtris wireframe rendering
        self.dynamic_state3 = gpu_device::with_device(|ctx| {
            let cmds = DynamicState3Commands::new(ctx);
            if cmds.capabilities().polygon_mode {
                Some(cmds)
            } else {
                None
            }
        }).flatten();
        myq2_common::common::com_printf("ModernRenderPath::init: EDS3 commands initialized\n");

        // Create Vulkan pipeline manager and build UI pipeline
        myq2_common::common::com_printf("ModernRenderPath::init: Creating PipelineManager\n");
        let swapchain_format = gpu_device::with_swapchain(|sc| sc.format)
            .unwrap_or(vk::Format::B8G8R8A8_SRGB);
        let dynamic_polygon_mode = self.dynamic_state3.is_some();
        match PipelineManager::new(swapchain_format, vk::Format::D32_SFLOAT, dynamic_polygon_mode) {
            Ok(mut pm) => {
                myq2_common::common::com_printf("ModernRenderPath::init: PipelineManager created\n");
                myq2_common::common::com_printf("ModernRenderPath::init: Creating UI pipeline\n");
                if let Err(e) = pm.create_pipeline(ShaderType::Ui, PipelineVariant::Ui) {
                    myq2_common::common::com_printf(&format!("Failed to create UI pipeline: {}\n", e));
                }
                myq2_common::common::com_printf("ModernRenderPath::init: UI pipeline created\n");
                // Initialize TextureStore for Vulkan texture management
                myq2_common::common::com_printf("ModernRenderPath::init: Initializing TextureStore\n");
                if let Some(ui_tex_layout) = pm.ui_texture_set_layout() {
                    super::texture::init_texture_store(ui_tex_layout);
                    myq2_common::common::com_printf("ModernRenderPath::init: TextureStore initialized\n");
                    // Create the white texture now (nested lock issues are fixed)
                    myq2_common::common::com_printf("ModernRenderPath::init: Creating white texture (1x1 white for solid colors)\n");
                    super::texture::create_white_texture();
                    myq2_common::common::com_printf("ModernRenderPath::init: White texture created\n");
                }
                // Create 3D scene pipelines (render to R16G16B16A16_SFLOAT HDR scene FBO)
                myq2_common::common::com_printf("ModernRenderPath::init: Creating 3D scene pipelines\n");
                pm.set_scene_format(vk::Format::R16G16B16A16_SFLOAT);
                myq2_common::common::com_printf("ModernRenderPath::init: Creating World/Opaque pipeline\n");
                if let Err(e) = pm.create_pipeline(ShaderType::World, PipelineVariant::Opaque) {
                    eprintln!("Failed to create World/Opaque pipeline: {}", e);
                }
                if let Err(e) = pm.create_pipeline(ShaderType::World, PipelineVariant::AlphaBlend) {
                    eprintln!("Failed to create World/AlphaBlend pipeline: {}", e);
                }
                if let Err(e) = pm.create_pipeline(ShaderType::Particle, PipelineVariant::Additive) {
                    eprintln!("Failed to create Particle/Additive pipeline: {}", e);
                }
                if let Err(e) = pm.create_pipeline(ShaderType::PostProcess, PipelineVariant::PostProcess) {
                    eprintln!("Failed to create PostProcess pipeline: {}", e);
                }
                if let Err(e) = pm.create_pipeline(ShaderType::Alias, PipelineVariant::Opaque) {
                    eprintln!("Failed to create Alias/Opaque pipeline: {}", e);
                }
                if let Err(e) = pm.create_pipeline(ShaderType::Alias, PipelineVariant::AlphaBlend) {
                    eprintln!("Failed to create Alias/AlphaBlend pipeline: {}", e);
                }
                if let Err(e) = pm.create_pipeline(ShaderType::Sky, PipelineVariant::Opaque) {
                    eprintln!("Failed to create Sky/Opaque pipeline: {}", e);
                }
                if let Err(e) = pm.create_pipeline(ShaderType::DynamicLight, PipelineVariant::Additive) {
                    eprintln!("Failed to create DynamicLight/Additive pipeline: {}", e);
                }
                if let Err(e) = pm.create_pipeline(ShaderType::World, PipelineVariant::Multiplicative) {
                    eprintln!("Failed to create World/Multiplicative pipeline: {}", e);
                }
                if let Err(e) = pm.create_pipeline(ShaderType::Water, PipelineVariant::AlphaBlend) {
                    eprintln!("Failed to create Water/AlphaBlend pipeline: {}", e);
                }
                // Opaque (depth-writing) water: Q2 water is solid and writes depth. Without depth
                // write, deferred passes (VXGI GI) sample the floor BEHIND the water and paint its
                // light onto the surface, making solid water look see-through.
                if let Err(e) = pm.create_pipeline(ShaderType::Water, PipelineVariant::Opaque) {
                    eprintln!("Failed to create Water/Opaque pipeline: {}", e);
                }
                self.pipelines = Some(pm);
            }
            Err(e) => {
                eprintln!("Failed to create PipelineManager: {:?}", e);
            }
        }

        // Create scene FBO descriptor set for post-processing
        if let Some(ref pp) = self.post_processor {
            let scene = pp.scene_fbo();
            if let (Some(view), Some(sampler)) = (scene.color_view(), scene.sampler()) {
                super::texture::create_descriptor_for_view(-1, view, sampler, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
            }
        }

        self.initialized = true;
        Ok(())
    }

    fn shutdown(&mut self) {
        // Shutdown TextureStore before pipeline manager (descriptor pool depends on device)
        super::texture::shutdown_texture_store();

        if let Some(ref mut pm) = self.pipelines {
            pm.shutdown();
        }
        self.pipelines = None;
        self.shaders = None;
        self.per_frame_ubo = None;
        self.per_object_ubo = None;
        self.post_processor = None;
        self.bsp_geometry.clear();
        self.alias_models.clear();
        self.lightmap_array.reset_allocation();
        // Destroy lightmap descriptor resources
        gpu_device::with_device(|ctx| {
            unsafe {
                if let Some(pool) = self.lightmap_descriptor_pool.take() {
                    ctx.device.destroy_descriptor_pool(pool, None);
                }
                self.lightmap_descriptor_set = None;
            }
        });
        // Destroy cinematic Vulkan resources
        gpu_device::with_device(|ctx| {
            // SAFETY: Vulkan context is valid and we're on the main thread.
            unsafe {
                if let Some(sampler) = self.cinematic_sampler.take() {
                    ctx.device.destroy_sampler(sampler, None);
                }
                if let Some(view) = self.cinematic_image_view.take() {
                    ctx.device.destroy_image_view(view, None);
                }
                if let Some(image) = self.cinematic_texture.take() {
                    ctx.device.destroy_image(image, None);
                }
                if let Some(memory) = self.cinematic_memory.take() {
                    ctx.device.free_memory(memory, None);
                }
            }
        });
        self.cinematic_texture_id = 0;
        self.initialized = false;
    }

    fn begin_frame(&mut self, params: &FrameParams) {
        use super::gpu_device::{with_device_and_swapchain, with_device_swapchain_surface, with_commands};

        if !self.initialized {
            return;
        }

        // Guard against zero-size refdef (happens on first frame after PREP
        // when cl.frame.valid is false and refdef was never populated)
        if params.width == 0 || params.height == 0 {
            return;
        }

        self.frame_in_progress = false;
        self.current_command_buffer = None;
        self.scene_rendered = false;
        self.brush_models.clear();
        self.sprite_draws.clear();
        self.sky_vertices.clear();
        self.sky_indices.clear();
        self.sky_face_draws.clear();
        self.dlight_vertices.clear();
        self.dlight_indices.clear();
        self.dlight_draws.clear();

        // Update dimensions if changed
        if params.width != self.width || params.height != self.height {
            self.set_dimensions(params.width, params.height);
        }

        // ========== Vulkan: Acquire swapchain image and begin command buffer ==========
        // SAFETY: Vulkan objects valid, called from main thread only
        let acquired = unsafe {
            with_device_and_swapchain(|ctx, swapchain| {
                // Acquire next swapchain image
                match swapchain.acquire_next_image(ctx) {
                    Ok(true) => {
                        self.current_frame_index = swapchain.current_frame;
                        true
                    }
                    Ok(false) => {
                        // Swapchain needs recreation (window resize, etc.)
                        false
                    }
                    Err(_e) => {
                        // Failed to acquire image
                        false
                    }
                }
            }).unwrap_or(false)
        };

        if !acquired {
            // Attempt swapchain recreation
            // SAFETY: Vulkan objects valid, called from main thread only
            unsafe {
                with_device_swapchain_surface(|ctx, swapchain, surface| {
                    let _ = swapchain.recreate(ctx, surface, self.width, self.height);
                });
            }
            // Resize post-processor to match
            if let Some(ref mut pp) = self.post_processor {
                pp.resize(self.width, self.height);
            }
            return;
        }

        // Begin command buffer recording
        let cmd_buffer: Option<vk::CommandBuffer> = unsafe {
            with_commands(|commands| {
                commands.begin_frame(self.current_frame_index).ok()
            }).flatten()
        };

        match cmd_buffer {
            Some(cmd) => {
                self.current_command_buffer = Some(cmd);
                self.frame_in_progress = true;
            }
            None => {
                eprintln!("ModernRenderPath::begin_frame - failed to get command buffer");
                return;
            }
        }

        // ========== Setup matrices and uniforms ==========

        // Compute view and projection matrices from refdef
        // Far plane computed from SKYBOX_SIZE to match original MyQ2 (gl_rmain.c):
        //   boxsize = SKYBOX_SIZE - 252*ceil(SKYBOX_SIZE/2300) → 4096
        //   farz = next_power_of_2(boxsize) * 2 → 8192
        self.frame_vieworg = params.vieworg;
        let view = Self::compute_view_matrix(&params.vieworg, &params.viewangles);
        let proj = Self::compute_projection_matrix(params.fov_x, params.fov_y, 4.0, 8192.0);
        let view_proj = Self::mat4_multiply(&proj, &view);

        // Extract view vectors from the view matrix (row-major interpretation of the rotation)
        let forward = [view[2], view[6], view[10]]; // Negated forward is stored
        let right_vec = [view[0], view[4], view[8]];
        let up_vec = [view[1], view[5], view[9]];

        // Update per-frame uniforms
        self.frame_uniforms.time = params.time;
        self.frame_uniforms.view_matrix = Self::to_mat4x4(&view);
        self.frame_uniforms.projection_matrix = Self::to_mat4x4(&proj);
        self.frame_uniforms.view_projection = Self::to_mat4x4(&view_proj);
        self.frame_uniforms.view_origin = params.vieworg;
        self.frame_uniforms.view_up = up_vec;
        self.frame_uniforms.view_right = right_vec;
        self.frame_uniforms.view_forward = [-forward[0], -forward[1], -forward[2]];

        if let Some(ref ubo) = self.per_frame_ubo {
            ubo.update(&self.frame_uniforms);
        }

        // Sync post-processing cvars
        if let Some(ref mut pp) = self.post_processor {
            {
                let cv = crate::vk_rmain::rcvars();
                pp.fxaa_enabled = cv.r_fxaa.value != 0.0;
                pp.ssao_enabled = cv.r_ssao.value != 0.0;
                pp.ssao_radius = cv.r_ssao_radius.value;
                pp.ssao_intensity = cv.r_ssao_intensity.value;
                pp.bloom_enabled = cv.r_bloom.value != 0.0;
                pp.bloom_threshold = cv.r_bloom_threshold.value;
                pp.bloom_intensity = cv.r_bloom_intensity.value;
                pp.fsr_enabled = cv.r_fsr.value != 0.0;
                pp.fsr_sharpness = cv.r_fsr_sharpness.value;

                let new_fsr_scale = cv.r_fsr_scale.value.clamp(0.5, 1.0);
                // Skip FSR scale update during begin_frame as update_fsr_scale() calls init_resources()
                // which conflicts with active frame state. Just update the field directly.
                if (new_fsr_scale - pp.fsr_scale).abs() > 0.001 {
                    pp.fsr_scale = new_fsr_scale;
                }
            }

            pp.begin_scene();
        }

        // Reset dynamic buffers
        self.particles.begin_frame();
        self.draw2d.begin_frame();
        self.alias_instanced.begin_frame();
    }

    fn end_frame(&mut self) {
        use super::gpu_device::{with_device_and_swapchain, with_commands};

        if !self.initialized {
            return;
        }

        // If no frame was started (2D-only rendering like console), start one now
        // BUT don't clear 2D vertices - they've already been queued!
        if !self.frame_in_progress {
            use super::gpu_device::{with_device_and_swapchain, with_device_swapchain_surface, with_commands};


            // Upload 2D vertex data (with buffer reuse to avoid recreation hang)
            self.draw2d.flush();

            // Just acquire swapchain and start command buffer, don't clear any draw buffers
            let acquired = unsafe {
                with_device_and_swapchain(|ctx, swapchain| {
                    match swapchain.acquire_next_image(ctx) {
                        Ok(true) => {
                            self.current_frame_index = swapchain.current_frame;
                            true
                        }
                        _ => false,
                    }
                }).unwrap_or(false)
            };

            if !acquired {
                unsafe {
                    with_device_swapchain_surface(|ctx, swapchain, surface| {
                        let _ = swapchain.recreate(ctx, surface, self.width, self.height);
                    });
                }
                if let Some(ref mut pp) = self.post_processor {
                    pp.resize(self.width, self.height);
                }
                return;
            }

            // Begin command buffer recording
            let cmd_buffer = unsafe {
                with_commands(|commands| {
                    commands.begin_frame(self.current_frame_index).ok()
                }).flatten()
            };

            match cmd_buffer {
                Some(cmd) => {
                    self.current_command_buffer = Some(cmd);
                    self.frame_in_progress = true;
                }
                None => {
                    eprintln!("ModernRenderPath::end_frame - failed to get command buffer");
                    return;
                }
            }
        }

        // Upload particle instance data to GPU
        self.particles.upload();

        // Step 1: Render 3D scene to scene FBO (BSP world + particles)
        if self.scene_rendered {
            self.flush_3d_scene();
            // Step 2: Composite scene to swapchain with post-processing (polyblend + gamma)
            self.composite_scene_to_swapchain();
        }

        // Step 3: Flush 2D drawing (console, menus, HUD) on top of the composited scene
        self.flush_2d_internal();

        // ========== Vulkan: End command buffer, submit, and present ==========
        // SAFETY: Single-threaded engine, Vulkan objects are valid
        unsafe {
            // End command buffer recording
            let end_ok = with_commands(|commands| {
                commands.end_frame(self.current_frame_index).is_ok()
            }).unwrap_or(false);

            if !end_ok {
                self.frame_in_progress = false;
                self.current_command_buffer = None;
                return;
            }

            // Get sync primitives and submit - use combined accessor to avoid deadlock
            let submit_ok = super::gpu_device::with_device_swapchain_commands(|ctx, swapchain, commands| {
                swapchain.reset_fence(ctx);
                let sync = swapchain.current_sync();
                commands.submit_frame(
                    ctx,
                    self.current_frame_index,
                    sync.image_available,
                    sync.render_finished,
                    sync.in_flight,
                ).is_ok()
            }).unwrap_or(false);

            if !submit_ok {
                self.frame_in_progress = false;
                self.current_command_buffer = None;
                return;
            }

            // Present the frame
            let present_result = with_device_and_swapchain(|ctx, swapchain| {
                swapchain.present(ctx)
            });
            // Note: present() advances the frame index internally
        }

        // Clear 2D buffers for next frame (must happen AFTER rendering is complete)
        self.draw2d.begin_frame();

        self.frame_in_progress = false;
        self.current_command_buffer = None;
    }

    fn draw_world(&mut self) {
        if !self.initialized || !self.bsp_geometry.is_initialized() {
            return;
        }

        // Mark that 3D scene rendering should happen this frame.
        // Actual draw commands (vkCmdDrawIndexed) are issued in flush_3d_scene().
        self.scene_rendered = true;
    }

    fn draw_alpha_surfaces(&mut self) {
        // Alpha surfaces would be drawn here with blending enabled.
        // In Vulkan, blend state is part of the pipeline object.
        // Until BSP geometry tracks alpha-flagged surfaces separately, this is a no-op.
    }

    fn blend_lightmaps(&mut self) {
        // Not needed in modern path - lightmaps are sampled directly in the shader.
    }

    fn draw_brush_model(&mut self, entity: &EntityLocal) {
        if entity.model.is_null() {
            return;
        }

        // SAFETY: model pointer valid for frame duration, main thread only
        let model = unsafe { &*entity.model };

        // Build per-entity model matrix (with Quake's pitch negation hack)
        let model_matrix = build_entity_matrix(&entity.origin, &entity.angles);

        // Compute model-view-projection for this entity
        let mvp = mat4_multiply(&self.frame_uniforms.view_projection, &model_matrix);

        // Only MOVERS that have left their authored position get dynamically relit;
        // static inline models (func_wall/detail, resting movers) keep use_dyn=0 and
        // their baked lightmap. Sampling light at a static model's centre can miss and
        // black it out, so we gate strictly on movement.
        let moved = entity.origin[0].abs() > 1.0
            || entity.origin[1].abs() > 1.0
            || entity.origin[2].abs() > 1.0;
        let dyn_block = if moved {
            let cz = entity.origin[2] + (model.mins[2] + model.maxs[2]) * 0.5;
            let (minx, maxx) = (model.mins[0], model.maxs[0]);
            let (miny, maxy) = (model.mins[1], model.maxs[1]);
            let (ox, oy) = (entity.origin[0], entity.origin[1]);

            // Sample the world AREA light at a world point. A downward floor trace can't see
            // a shaft's vertical light gradient (every sample below the mover shares the same
            // floor), so trace HORIZONTALLY toward the surrounding walls, which DO carry the
            // gradient — reading the wall light at the mover's current height varies
            // continuously as it travels. Fall back to the floor trace if no wall is in range.
            let sample = |x: f32, y: f32, z: f32| -> [f32; 3] {
                let p = [x, y, z];
                const REACH: f32 = 1024.0;
                let dirs = [
                    [x + REACH, y, z], [x - REACH, y, z],
                    [x, y + REACH, z], [x, y - REACH, z],
                ];
                let mut sum = [0.0_f32; 3];
                let mut hits = 0u32;
                for end in &dirs {
                    let mut l = [1.0_f32; 3];
                    // SAFETY: r_light_point_dir accesses vk_local statics, main thread only.
                    let hit = unsafe { crate::vk_light::r_light_point_dir(&p, end, &mut l) };
                    if hit { sum[0] += l[0]; sum[1] += l[1]; sum[2] += l[2]; hits += 1; }
                }
                if hits > 0 {
                    let n = hits as f32;
                    [sum[0] / n, sum[1] / n, sum[2] / n]
                } else {
                    let mut l = [1.0_f32; 3];
                    // SAFETY: as above.
                    unsafe { crate::vk_light::r_light_point(&p, &mut l); }
                    l
                }
            };

            // Sample at the 4 footprint corners (inset 15% so the trace sits over the deck,
            // not on the very edge / inside a wall). Order matches the shader: (min,min),
            // (max,min),(min,max),(max,max). Each corner samples a different part of the
            // surrounding light, so the area's light/shadow gradient falls across the lift.
            let inset = 0.15_f32;
            let lx0 = minx + (maxx - minx) * inset;
            let lx1 = maxx - (maxx - minx) * inset;
            let ly0 = miny + (maxy - miny) * inset;
            let ly1 = maxy - (maxy - miny) * inset;
            // Desaturate the sampled room light toward its own luminance. Quake light textures
            // and r_light_point_dir return strongly warm/red values near many fixtures, and
            // mixing that raw into the mover's lightmap (0.6) tinted the whole lift red. Keep
            // the brightness gradient (so the lift still lights/darkens as it travels) but pull
            // most of the hue out so it reads as neutral-lit metal, not a red deck.
            let desat = |c: [f32; 3]| -> [f32; 3] {
                let luma = c[0] * 0.299 + c[1] * 0.587 + c[2] * 0.114;
                const KEEP: f32 = 0.25; // fraction of original hue retained
                [
                    luma + (c[0] - luma) * KEEP,
                    luma + (c[1] - luma) * KEEP,
                    luma + (c[2] - luma) * KEEP,
                ]
            };
            let targets = [
                desat(sample(lx0 + ox, ly0 + oy, cz)),
                desat(sample(lx1 + ox, ly0 + oy, cz)),
                desat(sample(lx0 + ox, ly1 + oy, cz)),
                desat(sample(lx1 + ox, ly1 + oy, cz)),
            ];

            // Per-corner temporal smoothing (fixed alpha — the refdef time fed to the
            // renderer does not advance reliably per render frame, so a time-based ease
            // freezes; see git history).
            let key = entity.model as usize;
            let prev = self.mover_light.get(&key).copied().unwrap_or(targets);
            let a = 0.12_f32; // ~8-frame ease toward target
            let mut smoothed = [[0.0_f32; 3]; 4];
            for c in 0..4 {
                for i in 0..3 {
                    smoothed[c][i] = prev[c][i] + (targets[c][i] - prev[c][i]) * a;
                }
            }
            self.mover_light.insert(key, smoothed);

            // Pack each corner's RGB into one float (0..1 -> 0..255 per channel; all such
            // integers are exact in f32). The shader unpacks and bilinearly interpolates.
            let pack = |c: [f32; 3]| -> f32 {
                let r = (c[0].clamp(0.0, 1.0) * 255.0).round();
                let g = (c[1].clamp(0.0, 1.0) * 255.0).round();
                let b = (c[2].clamp(0.0, 1.0) * 255.0).round();
                r + g * 256.0 + b * 65536.0
            };
            // Bounds in AUTHORED space (v_FragPos = a_Position, not offset by origin).
            let inv_w = if (maxx - minx).abs() > 1e-3 { 1.0 / (maxx - minx) } else { 0.0 };
            let inv_h = if (maxy - miny).abs() > 1e-3 { 1.0 / (maxy - miny) } else { 0.0 };
            [
                1.0,
                pack(smoothed[0]), pack(smoothed[1]), pack(smoothed[2]), pack(smoothed[3]),
                minx, miny, inv_w, inv_h,
            ]
        } else {
            [0.0; 9]
        };

        // Queue brush model for drawing in flush_3d_scene()
        let center = [
            entity.origin[0] + (model.mins[0] + model.maxs[0]) * 0.5,
            entity.origin[1] + (model.mins[1] + model.maxs[1]) * 0.5,
            entity.origin[2] + (model.mins[2] + model.maxs[2]) * 0.5,
        ];
        self.brush_models.push(BrushModelDraw {
            mvp: mat4_to_flat(&mvp),
            first_surface: model.firstmodelsurface as usize,
            num_surfaces: model.nummodelsurfaces as usize,
            dyn_block,
            model_matrix,
            center,
        });
        self.scene_rendered = true;
    }

    fn draw_alias_model(&mut self, entity: &EntityLocal) {
        if entity.model.is_null() {
            return;
        }

        // SAFETY: model pointer valid for frame duration, main thread only
        let model_id = entity.model as usize;

        // Build model matrix from entity origin + angles
        let model_matrix = build_entity_matrix(&entity.origin, &entity.angles);

        // Blob-shadow ground plane = the entity's feet in world Z, so the shadow lands on
        // whatever surface it stands on (including a moving lift deck). +1 bias so it sits
        // just above the surface and doesn't z-fight.
        // SAFETY: model pointer valid for frame duration, main thread only.
        // Trace down to the floor below the entity for the shadow plane (the model's stored
        // bounds are a fixed [-32..32] placeholder, useless for this). +1 bias so the shadow
        // sits just above the floor. NaN ⇒ no floor found ⇒ no shadow cast.
        // SAFETY: r_ground_z accesses vk_local statics, main thread only.
        let shadow_ground_z = unsafe {
            crate::vk_light::r_ground_z(&entity.origin).map(|z| z + 1.0).unwrap_or(f32::NAN)
        };

        // Shadow shear: lean the shadow away from the nearest light. Find the closest static
        // light, take the horizontal direction toward it; the matrix shears points by their
        // height so the shadow falls on the far side. Scaled modestly so it stays grounded.
        let shadow_skew = {
            let lights = crate::vk_rsurf::STATIC_LIGHTS.lock().unwrap_or_else(|e| e.into_inner());
            let mut best_d2 = f32::MAX;
            let mut dir = [0.0_f32, 0.0];
            for l in lights.iter() {
                let dx = l.origin[0] - entity.origin[0];
                let dy = l.origin[1] - entity.origin[1];
                let dz = l.origin[2] - entity.origin[2];
                let d2 = dx * dx + dy * dy + dz * dz;
                if d2 < best_d2 && d2 > 1.0 {
                    best_d2 = d2;
                    let hlen = (dx * dx + dy * dy).sqrt();
                    if hlen > 1.0 {
                        dir = [dx / hlen, dy / hlen];
                    }
                }
            }
            const SKEW: f32 = 0.4;
            [dir[0] * SKEW, dir[1] * SKEW]
        };

        // Compute shade lighting for this entity position
        let mut shade_light = [1.0_f32; 3];
        if entity.flags & myq2_common::q_shared::RF_FULLBRIGHT != 0 {
            shade_light = [1.0, 1.0, 1.0];
        } else {
            // SAFETY: r_light_point accesses vk_local statics (main thread only)
            unsafe {
                crate::vk_light::r_light_point(&entity.origin, &mut shade_light);
            }

            // RF_MINLIGHT: ensure minimum brightness
            if entity.flags & myq2_common::q_shared::RF_MINLIGHT != 0 {
                for c in &mut shade_light {
                    if *c < 0.1 {
                        *c = 0.1;
                    }
                }
            }

            // RF_GLOW: pulsing brightness effect
            if entity.flags & myq2_common::q_shared::RF_GLOW != 0 {
                let scale = 1.0 + 0.2 * (self.frame_uniforms.time * 7.0).sin();
                for c in &mut shade_light {
                    *c *= scale;
                    if *c > 1.0 {
                        *c = 1.0;
                    }
                }
            }
        }

        let alpha = if entity.flags & myq2_common::q_shared::RF_TRANSLUCENT != 0 {
            entity.alpha
        } else {
            1.0
        };

        // Build instance data for the instanced renderer
        let instance = AliasInstance {
            model_matrix,
            shade_light,
            alpha,
            frame: entity.frame as u32,
            old_frame: entity.oldframe as u32,
            backlerp: entity.backlerp,
            flags: entity.flags as u32,
            shadow_ground_z,
            shadow_skew,
        };

        // Queue instance — actual draw commands will be issued when
        // Vulkan render pass draw calls are wired up.
        self.alias_instanced.add_instance(model_id, instance);
    }

    fn draw_sprite_model(&mut self, entity: &EntityLocal) {
        if entity.model.is_null() {
            return;
        }

        // SAFETY: model pointer valid for frame duration, main thread only
        let model = unsafe { &*entity.model };
        let extradata = model.extradata;
        if extradata.is_null() {
            return;
        }

        // SAFETY: extradata points to DSprite header followed by DSprFrame array
        let sprite = unsafe { &*(extradata as *const myq2_common::qfiles::DSprite) };
        let frame_idx = (entity.frame as usize) % sprite.numframes.max(1) as usize;

        // Get frame dimensions
        let frame = unsafe {
            let header_size = std::mem::size_of::<myq2_common::qfiles::DSprite>();
            let frame_size = std::mem::size_of::<myq2_common::qfiles::DSprFrame>();
            &*((extradata as *const u8).add(header_size + frame_idx * frame_size)
                as *const myq2_common::qfiles::DSprFrame)
        };

        let alpha = if entity.flags & myq2_common::q_shared::RF_TRANSLUCENT != 0 {
            entity.alpha
        } else {
            1.0
        };

        // Get skin texture for this frame
        let skin = model.skins[frame_idx];
        if skin.is_null() {
            return;
        }
        // SAFETY: skin pointer valid for frame duration
        let texnum = unsafe { (*skin).texnum as u32 };

        let up = self.frame_uniforms.view_up;
        let right = self.frame_uniforms.view_right;
        let o = entity.origin;
        let ox = frame.origin_x as f32;
        let oy = frame.origin_y as f32;
        let w = frame.width as f32;
        let h = frame.height as f32;

        // Billboard quad corners (matches original R_DrawSpriteModel)
        let p0 = [
            o[0] - oy * up[0] - ox * right[0],
            o[1] - oy * up[1] - ox * right[1],
            o[2] - oy * up[2] - ox * right[2],
        ];
        let p1 = [
            o[0] + (h - oy) * up[0] - ox * right[0],
            o[1] + (h - oy) * up[1] - ox * right[1],
            o[2] + (h - oy) * up[2] - ox * right[2],
        ];
        let p2 = [
            o[0] + (h - oy) * up[0] + (w - ox) * right[0],
            o[1] + (h - oy) * up[1] + (w - ox) * right[1],
            o[2] + (h - oy) * up[2] + (w - ox) * right[2],
        ];
        let p3 = [
            o[0] - oy * up[0] + (w - ox) * right[0],
            o[1] - oy * up[1] + (w - ox) * right[1],
            o[2] - oy * up[2] + (w - ox) * right[2],
        ];

        self.sprite_draws.push(SpriteDrawData {
            vertices: [
                BspVertex::new(p0, [0.0, 1.0], [0.0, 0.0]),
                BspVertex::new(p1, [0.0, 0.0], [0.0, 0.0]),
                BspVertex::new(p2, [1.0, 0.0], [0.0, 0.0]),
                BspVertex::new(p3, [1.0, 1.0], [0.0, 0.0]),
            ],
            texture_id: texnum,
            alpha,
        });
        self.scene_rendered = true;
    }

    fn draw_particles(&mut self, particles: &[ParticleData]) {
        // Convert particle data and stage for instanced rendering
        for p in particles {
            // Convert palette color index to RGBA using d_8to24table
            let color_u32 = crate::vk_image::d_8to24table()[p.color & 0xFF];
            let r = (color_u32 & 0xFF) as f32 / 255.0;
            let g = ((color_u32 >> 8) & 0xFF) as f32 / 255.0;
            let b = ((color_u32 >> 16) & 0xFF) as f32 / 255.0;
            self.particles.add(p.origin, [r, g, b, p.alpha], 1.0);
        }
    }

    fn render_dlights(&mut self) {
        let flashblend = crate::vk_rmain::rcvars().vk_flashblend.value != 0.0;
        let light_bloom = crate::vk_rmain::rcvars().r_light_bloom.value != 0.0;
        // Nothing to draw unless classic flashblend OR the new bloom-glow path is on.
        if !flashblend && !light_bloom {
            return;
        }

        self.dlight_draws.clear();
        self.dlight_vertices.clear();
        self.dlight_indices.clear();

        let vp = &self.frame_uniforms.view_projection;
        let vpn = self.frame_uniforms.view_forward;
        let vright = self.frame_uniforms.view_right;
        let vup = self.frame_uniforms.view_up;
        let vp_flat = mat4_to_flat(vp);
        let vorg = self.frame_uniforms.view_origin;

        // The disc is additive (blend = src,ONE) with a bright center fading to black at the
        // rim. In classic flashblend the center is dim (0.2). With r_light_bloom we push the
        // center HDR-bright (>1.0) so the composite bloom pass blooms the source itself.
        const GLOW_CORE: f32 = 1.6;
        let center_scale = if light_bloom { GLOW_CORE } else { 0.2 };

        // Gather every disc to draw first (origin, rim radius, base color), then emit geometry.
        let mut discs: Vec<([f32; 3], f32, [f32; 3])> = Vec::new();

        // --- Dynamic lights only (rockets, explosions, muzzle flashes, item glows) ---
        // Static map lights are NOT orbed: their emissive textures already bloom, and floating
        // orbs at ceiling-light / sky-emitter positions read as weird balls in the air.
        // r_newrefdef is valid for frame duration, main thread only
        let num_dlights = crate::vk_local::rfs().r_newrefdef.num_dlights;
        for i in 0..num_dlights as usize {
            // SAFETY: dlight pointer valid, index in bounds
            let dl = unsafe { crate::vk_local::rfs().r_newrefdef.dlight(i) };
            let rad = dl.intensity * 0.35;
            if rad < 1.0 {
                continue;
            }
            discs.push((dl.origin, rad, dl.color));
        }
        let _ = vorg;

        // --- Emit fan geometry (1 center + 17 rim verts, 16 tris) per disc ---
        for (origin, rad, color) in &discs {
            let (rad, origin, color) = (*rad, *origin, *color);
            let base_vtx = self.dlight_vertices.len() as u32;

            // Center: bright, offset slightly toward the camera so it isn't z-clipped by the
            // surface it sits on.
            self.dlight_vertices.push([
                origin[0] - vpn[0] * rad,
                origin[1] - vpn[1] * rad,
                origin[2] - vpn[2] * rad,
                color[0] * center_scale, color[1] * center_scale, color[2] * center_scale,
            ]);

            // 17 rim verts (closed fan), black — rasterizer interpolates center→rim.
            for j in (0..=16).rev() {
                let a = (j as f32) / 16.0 * std::f32::consts::TAU;
                let (sin_a, cos_a) = a.sin_cos();
                self.dlight_vertices.push([
                    origin[0] + vright[0] * cos_a * rad + vup[0] * sin_a * rad,
                    origin[1] + vright[1] * cos_a * rad + vup[1] * sin_a * rad,
                    origin[2] + vright[2] * cos_a * rad + vup[2] * sin_a * rad,
                    0.0, 0.0, 0.0,
                ]);
            }

            for j in 0..16u32 {
                self.dlight_indices.push(base_vtx);
                self.dlight_indices.push(base_vtx + 1 + j);
                self.dlight_indices.push(base_vtx + 2 + j);
            }

            self.dlight_draws.push(DlightPushConstants {
                mvp: vp_flat,
                light_origin: origin,
                light_radius: rad,
                light_color: color,
                _pad: 0.0,
            });
        }
    }

    fn draw_sky(&mut self) {
        self.sky_vertices.clear();
        self.sky_indices.clear();
        self.sky_face_draws.clear();

        let (skyrotate, skyaxis, sky_images, skymins, skymaxs, sky_min, sky_max) =
            crate::vk_warp::with_warp_state(|ws| {
                (
                    ws.skyrotate,
                    ws.skyaxis,
                    ws.sky_images,
                    ws.skymins,
                    ws.skymaxs,
                    ws.sky_min,
                    ws.sky_max,
                )
            });

        let time = self.frame_uniforms.time;

        // Build sky model matrix: translate(vieworg) * rotate(skyrotate*time, skyaxis)
        let sky_model = build_sky_matrix(
            &self.frame_uniforms.view_origin,
            skyrotate,
            &skyaxis,
            time,
        );
        let sky_mvp = mat4_multiply(&self.frame_uniforms.view_projection, &sky_model);
        self.sky_mvp = mat4_to_flat(&sky_mvp);
        // Kept for the mirrored water-reflection pass (rebuilds sky MVP with the mirrored VP).
        self.sky_model_flat = mat4_to_flat(&sky_model);

        for i in 0..6 {
            let mut s_min = skymins[0][i];
            let mut t_min = skymins[1][i];
            let mut s_max = skymaxs[0][i];
            let mut t_max = skymaxs[1][i];

            // If sky is rotating, draw full face
            if skyrotate != 0.0 {
                s_min = -1.0;
                t_min = -1.0;
                s_max = 1.0;
                t_max = 1.0;
            }

            // Skip invisible faces
            if s_min >= s_max || t_min >= t_max {
                continue;
            }

            let base = self.sky_vertices.len() as u32;

            // 4 corners via make_sky_vec (same order as original GL_QUADS)
            for &(s, t) in &[
                (s_min, t_min),
                (s_min, t_max),
                (s_max, t_max),
                (s_max, t_min),
            ] {
                let (pos, tc) = make_sky_vec(s, t, i, sky_min, sky_max);
                self.sky_vertices.push(BspVertex::new(pos, tc, [0.0, 0.0]));
            }

            // 2 triangles per quad
            self.sky_indices.extend_from_slice(&[
                base, base + 1, base + 2,
                base, base + 2, base + 3,
            ]);

            // Track which texture for this face
            let tex_image = sky_images[crate::vk_warp::SKYTEXORDER[i]];
            // SAFETY: sky_images are loaded during r_set_sky, valid for session
            let texnum = if !tex_image.is_null() {
                unsafe { (*tex_image).texnum as u32 }
            } else {
                0
            };
            let first_idx = self.sky_indices.len() as u32 - 6;
            self.sky_face_draws.push((texnum, first_idx, 6));
        }

        // Diagnostic: only count frames that have sky faces (skip empty early frames)
        static SKY_HAS_DIAG: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        if !self.sky_face_draws.is_empty() {
            let n = SKY_HAS_DIAG.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if n < 5 {
                eprintln!("[SKY] draw_sky (sky frame {}): {} faces, {} verts, {} indices",
                    n, self.sky_face_draws.len(), self.sky_vertices.len(), self.sky_indices.len());
                for (tex_id, first_idx, count) in &self.sky_face_draws {
                    let has_ds = super::texture::ensure_descriptor_set(*tex_id as i32).is_some();
                    eprintln!("[SKY]   texnum={} first={} count={} descriptor_set={}", tex_id, first_idx, count, has_ds);
                }
                // Dump first few vertices
                for (vi, v) in self.sky_vertices.iter().enumerate().take(8) {
                    eprintln!("[SKY]   vert[{}] pos=({:.1},{:.1},{:.1}) tc=({:.3},{:.3})",
                        vi, v.position[0], v.position[1], v.position[2], v.tex_coord[0], v.tex_coord[1]);
                }
            }
        }
    }

    fn draw_char(&mut self, x: i32, y: i32, num: i32) {
        let num = num & 255;
        if (num & 127) == 32 || y <= -8 {
            return;
        }
        self.draw2d.draw_char(x, y, num as u8);
    }

    fn draw_pic(&mut self, x: i32, y: i32, pic: &str) {
        // Look up the image to get texture ID and dimensions
        // SAFETY: renderer state accessed from main thread only
        unsafe {
            let gl = crate::vk_image::draw_find_pic(pic);
            if gl.is_null() {
                return;
            }
            self.draw2d.push_quad(
                x as f32, y as f32,
                (*gl).width as f32, (*gl).height as f32,
                (*gl).sl, (*gl).tl, (*gl).sh, (*gl).th,
                [1.0, 1.0, 1.0, 1.0],
                (*gl).texnum as u32,
                BlendMode::Alpha,
            );
        }
    }

    fn draw_stretch_pic(&mut self, x: i32, y: i32, w: i32, h: i32, pic: &str) {
        // SAFETY: renderer state accessed from main thread only
        unsafe {
            let gl = crate::vk_image::draw_find_pic(pic);
            if gl.is_null() {
                return;
            }

            // Handle transparent console
            let mut alpha = 1.0f32;
            if crate::vk_local::TRANS_CONSOLE && pic == "conback" && crate::vk_local::rfs().vk_state.transconsole != 0 {
                let vid_height = crate::vk_rmain::rg().vid.height as f32;
                alpha = crate::vk_local::TRANS_CONSOLE_VALUE * ((vid_height + y as f32) / (vid_height / 2.0));
            }

            self.draw2d.push_quad(
                x as f32, y as f32,
                w as f32, h as f32,
                (*gl).sl, (*gl).tl, (*gl).sh, (*gl).th,
                [1.0, 1.0, 1.0, alpha],
                (*gl).texnum as u32,
                BlendMode::Alpha,
            );
        }
    }

    fn draw_fill(&mut self, x: i32, y: i32, w: i32, h: i32, color: i32, alpha: f32) {
        // Convert palette color to RGBA
        let color_u32 = crate::vk_image::d_8to24table()[(color & 0xFF) as usize];
        let r = (color_u32 & 0xFF) as f32 / 255.0;
        let g = ((color_u32 >> 8) & 0xFF) as f32 / 255.0;
        let b = ((color_u32 >> 16) & 0xFF) as f32 / 255.0;
        self.draw2d.draw_fill(x, y, w, h, [r, g, b, alpha]);
    }

    fn draw_tile_clear(&mut self, x: i32, y: i32, w: i32, h: i32, pic: &str) {
        // SAFETY: renderer state accessed from main thread only
        unsafe {
            let gl = crate::vk_image::draw_find_pic(pic);
            if gl.is_null() {
                return;
            }
            // Tile the texture at 64x64 intervals
            self.draw2d.push_quad(
                x as f32, y as f32,
                w as f32, h as f32,
                x as f32 / 64.0, y as f32 / 64.0,
                (x + w) as f32 / 64.0, (y + h) as f32 / 64.0,
                [1.0, 1.0, 1.0, 1.0],
                (*gl).texnum as u32,
                BlendMode::None,
            );
        }
    }

    fn draw_fade_screen(&mut self) {
        let w = self.width;
        let h = self.height;
        self.draw2d.draw_fill(0, 0, w as i32, h as i32, [0.0, 0.0, 0.0, 0.8]);
    }

    fn draw_string(&mut self, x: i32, y: i32, s: &str) {
        let mut cx = x;
        for c in s.bytes() {
            self.draw2d.draw_char(cx, y, c);
            cx += 8;
        }
    }

    fn draw_stretch_raw(&mut self, x: i32, y: i32, w: i32, h: i32, cols: i32, rows: i32, data: &[u8]) {
        // Convert palettized cinematic data to RGBA and upload to a GPU texture
        // Single-threaded engine
        {
            let rawpal = crate::vk_image::with_rawpalette(|p| *p);
            let mut image32 = vec![0u32; 256 * 256];

            let hscale = if rows <= 256 { 1.0_f32 } else { rows as f32 / 256.0 };
            let trows = if rows <= 256 { rows as usize } else { 256 };
            let t = rows as f32 * hscale / 256.0;

            for i in 0..trows {
                let row = (i as f32 * hscale) as usize;
                if row >= rows as usize { break; }
                let row_offset = cols as usize * row;
                let fracstep = (cols as u32).wrapping_mul(0x10000) / 256;
                let mut frac = fracstep >> 1;
                for j in 0..256 {
                    let src_idx = (frac >> 16) as usize;
                    if row_offset + src_idx < data.len() {
                        image32[i * 256 + j] = rawpal[data[row_offset + src_idx] as usize];
                    }
                    frac = frac.wrapping_add(fracstep);
                }
            }

            // Create cinematic texture on first use
            if self.cinematic_texture.is_none() {
                self.create_cinematic_texture();
            }

            // Upload image data via staging buffer
            self.upload_cinematic_data(&image32);

            // Push a textured quad for the cinematic frame
            // Use cinematic_texture_id as a placeholder for 2D batch texture reference
            self.draw2d.push_quad(
                x as f32, y as f32,
                w as f32, h as f32,
                0.0, 0.0, 1.0, t.min(1.0),
                [1.0, 1.0, 1.0, 1.0],
                self.cinematic_texture_id,
                BlendMode::None,
            );
        }
    }

    fn flush_2d(&mut self) {
        self.flush_2d_internal();
    }
}

impl ModernRenderPath {
    /// Internal 2D flush - uploads and draws all batched 2D quads.
    ///
    /// In Vulkan, 2D drawing state (depth off, blend on) is baked into
    /// the pipeline object. Texture binding and draw calls happen through
    /// descriptor sets and command buffers.
    fn flush_2d_internal(&mut self) {
        // Ensure scrap atlas is uploaded if dirty (small pics packed into 256x256 atlas)
        // SAFETY: Accesses vk_image statics, main thread only.
        unsafe { crate::vk_image::scrap_upload_if_dirty(); }

        self.draw2d.flush();

        let cmd = match self.current_command_buffer {
            Some(cmd) => cmd,
            None => return,
        };

        // Get swapchain image and extent for rendering
        let sc_data = gpu_device::with_swapchain(|sc| {
            (sc.current_image(), sc.current_image_view(), sc.extent)
        });
        let (sc_image, sc_image_view, sc_extent) = match sc_data {
            Some(v) => v,
            None => return,
        };

        // Gather batch and pipeline data before entering with_device closure
        let batches_to_draw: Vec<_> = self.draw2d.batches().to_vec();
        let vbo = self.draw2d.vertex_buffer().vk_buffer();
        let pipeline_data = self.pipelines.as_ref().and_then(|pm| {
            pm.get(ShaderType::Ui, PipelineVariant::Ui)
                .map(|p| (p.pipeline, p.layout))
        });
        let has_draw_data = !batches_to_draw.is_empty() && vbo.is_some() && pipeline_data.is_some();

        // Pre-gather descriptor sets for each batch (outside with_device to avoid deadlock)
        // Use ensure_descriptor_set for lazy creation - works because we're outside init locks
        let fallback_set = super::texture::ensure_descriptor_set(0);
        let batch_descriptor_sets: Vec<Option<vk::DescriptorSet>> = batches_to_draw.iter()
            .map(|b| {
                super::texture::ensure_descriptor_set(b.texture as i32)
                    .or(fallback_set)
            })
            .collect();

        // Compute ortho projection for 2D
        let ortho = Self::compute_ortho_matrix(self.width as f32, self.height as f32);

        let scene_was_rendered = self.scene_rendered;

        gpu_device::with_device(|ctx| {
            // SAFETY: Vulkan context valid, called from main thread only, within
            // active command buffer recording started in begin_frame().
            unsafe {
                // 1. Transition swapchain image if no scene was composited
                // When scene_rendered, composite_scene_to_swapchain() already
                // transitioned to COLOR_ATTACHMENT_OPTIMAL and left it there.
                if !scene_was_rendered {
                    let to_color_barrier = vk::ImageMemoryBarrier::default()
                        .old_layout(vk::ImageLayout::UNDEFINED)
                        .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                        .image(sc_image)
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        })
                        .src_access_mask(vk::AccessFlags::empty())
                        .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE);
                    ctx.device.cmd_pipeline_barrier(
                        cmd,
                        vk::PipelineStageFlags::TOP_OF_PIPE,
                        vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        &[to_color_barrier],
                    );
                }

                // 2. Begin dynamic rendering on swapchain image
                // When scene was rendered, use LOAD to preserve the composited 3D scene.
                // When no scene (menu/loading), CLEAR to black.
                let (load_op, clear_value) = if scene_was_rendered {
                    (vk::AttachmentLoadOp::LOAD, vk::ClearValue::default())
                } else {
                    (vk::AttachmentLoadOp::CLEAR, vk::ClearValue {
                        color: vk::ClearColorValue {
                            float32: [0.0, 0.0, 0.0, 1.0],
                        },
                    })
                };

                let color_attachment = vk::RenderingAttachmentInfo::default()
                    .image_view(sc_image_view)
                    .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .load_op(load_op)
                    .store_op(vk::AttachmentStoreOp::STORE)
                    .clear_value(clear_value);
                let rendering_info = vk::RenderingInfo::default()
                    .render_area(vk::Rect2D {
                        offset: vk::Offset2D { x: 0, y: 0 },
                        extent: sc_extent,
                    })
                    .layer_count(1)
                    .color_attachments(std::slice::from_ref(&color_attachment));
                ctx.device.cmd_begin_rendering(cmd, &rendering_info);

                // 3. Draw 2D batches if we have data
                if has_draw_data {
                    let (pipeline, layout) = pipeline_data.unwrap();
                    let vbo_handle = vbo.unwrap();

                    ctx.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline);

                    // Use negative height for Vulkan Y-flip (to match OpenGL Y-up coordinates)
                    let viewport = vk::Viewport {
                        x: 0.0,
                        y: sc_extent.height as f32,
                        width: sc_extent.width as f32,
                        height: -(sc_extent.height as f32),
                        min_depth: 0.0,
                        max_depth: 1.0,
                    };
                    ctx.device.cmd_set_viewport(cmd, 0, &[viewport]);
                    let scissor = vk::Rect2D {
                        offset: vk::Offset2D { x: 0, y: 0 },
                        extent: sc_extent,
                    };
                    ctx.device.cmd_set_scissor(cmd, 0, &[scissor]);

                    // Push projection matrix (64 bytes = mat4, vertex stage)
                    let ortho_bytes: &[u8] = std::slice::from_raw_parts(
                        ortho.as_ptr() as *const u8,
                        64,
                    );
                    ctx.device.cmd_push_constants(
                        cmd,
                        layout,
                        vk::ShaderStageFlags::VERTEX,
                        0,
                        ortho_bytes,
                    );

                    ctx.device.cmd_bind_vertex_buffers(cmd, 0, &[vbo_handle], &[0]);

                    // Draw each batch with its texture descriptor bound at set 1
                    for (i, batch) in batches_to_draw.iter().enumerate() {
                        // Only draw if we have a valid descriptor set
                        if let Some(ds) = batch_descriptor_sets[i] {
                            ctx.device.cmd_bind_descriptor_sets(
                                cmd,
                                vk::PipelineBindPoint::GRAPHICS,
                                layout,
                                1, // firstSet = 1 (UI texture at set 1)
                                &[ds],
                                &[],
                            );
                            ctx.device.cmd_draw(cmd, batch.vertex_count, 1, batch.first_vertex, 0);
                        }
                    }
                }

                // 4. End rendering
                ctx.device.cmd_end_rendering(cmd);

                // 5. Transition swapchain image: COLOR_ATTACHMENT_OPTIMAL → PRESENT_SRC_KHR
                let to_present_barrier = vk::ImageMemoryBarrier::default()
                    .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .image(sc_image)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    })
                    .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                    .dst_access_mask(vk::AccessFlags::empty());
                ctx.device.cmd_pipeline_barrier(
                    cmd,
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[to_present_barrier],
                );
            }
        });
    }

    /// Create the cinematic texture (Vulkan).
    fn create_cinematic_texture(&mut self) {
        // Cinematic frames are 256x256 RGBA
        const CINEMATIC_SIZE: u32 = 256;

        gpu_device::with_device(|ctx| {
            // SAFETY: Vulkan context is valid and we're on the main thread.
            unsafe {
                // Create image
                let image_info = vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(vk::Format::R8G8B8A8_UNORM)
                    .extent(vk::Extent3D {
                        width: CINEMATIC_SIZE,
                        height: CINEMATIC_SIZE,
                        depth: 1,
                    })
                    .mip_levels(1)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .initial_layout(vk::ImageLayout::UNDEFINED);

                let image = match ctx.device.create_image(&image_info, None) {
                    Ok(img) => img,
                    Err(_) => return,
                };

                // Allocate memory
                let mem_reqs = ctx.device.get_image_memory_requirements(image);
                let mem_props = ctx.instance.get_physical_device_memory_properties(ctx.physical_device);

                let mem_type = (0..mem_props.memory_type_count).find(|&i| {
                    (mem_reqs.memory_type_bits & (1 << i)) != 0 &&
                    mem_props.memory_types[i as usize].property_flags.contains(
                        vk::MemoryPropertyFlags::DEVICE_LOCAL
                    )
                });

                let mem_type = match mem_type {
                    Some(i) => i,
                    None => {
                        ctx.device.destroy_image(image, None);
                        return;
                    }
                };

                let alloc_info = vk::MemoryAllocateInfo::default()
                    .allocation_size(mem_reqs.size)
                    .memory_type_index(mem_type);

                let memory = match ctx.device.allocate_memory(&alloc_info, None) {
                    Ok(mem) => mem,
                    Err(_) => {
                        ctx.device.destroy_image(image, None);
                        return;
                    }
                };

                if ctx.device.bind_image_memory(image, memory, 0).is_err() {
                    ctx.device.free_memory(memory, None);
                    ctx.device.destroy_image(image, None);
                    return;
                }

                // Create image view
                let view_info = vk::ImageViewCreateInfo::default()
                    .image(image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(vk::Format::R8G8B8A8_UNORM)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    });

                let view = match ctx.device.create_image_view(&view_info, None) {
                    Ok(v) => v,
                    Err(_) => {
                        ctx.device.free_memory(memory, None);
                        ctx.device.destroy_image(image, None);
                        return;
                    }
                };

                // Create sampler with linear filtering
                let sampler_info = vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::LINEAR)
                    .min_filter(vk::Filter::LINEAR)
                    .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .mip_lod_bias(0.0)
                    .anisotropy_enable(false)
                    .max_anisotropy(1.0)
                    .compare_enable(false)
                    .min_lod(0.0)
                    .max_lod(0.0)
                    .border_color(vk::BorderColor::FLOAT_OPAQUE_BLACK)
                    .unnormalized_coordinates(false);

                let sampler = match ctx.device.create_sampler(&sampler_info, None) {
                    Ok(s) => s,
                    Err(_) => {
                        ctx.device.destroy_image_view(view, None);
                        ctx.device.free_memory(memory, None);
                        ctx.device.destroy_image(image, None);
                        return;
                    }
                };

                self.cinematic_texture = Some(image);
                self.cinematic_image_view = Some(view);
                self.cinematic_memory = Some(memory);
                self.cinematic_sampler = Some(sampler);
                self.cinematic_texture_id = 1; // Non-zero means "textured"
            }
        });
    }

    /// Upload cinematic frame data to the GPU texture.
    fn upload_cinematic_data(&self, image32: &[u32]) {
        let texture = match self.cinematic_texture {
            Some(t) => t,
            None => return,
        };

        // Cinematic frames are 256x256 RGBA
        const CINEMATIC_SIZE: u32 = 256;
        let data_size = (CINEMATIC_SIZE * CINEMATIC_SIZE * 4) as usize;

        // Convert u32 RGBA to bytes
        let byte_data: Vec<u8> = image32.iter()
            .flat_map(|&pixel| pixel.to_le_bytes())
            .collect();

        if byte_data.len() < data_size {
            return;
        }

        // Use with_device_and_commands_mut to avoid deadlock — both with_device
        // and with_commands_mut lock the same VK_DEVICE_STATE mutex.
        gpu_device::with_device_and_commands_mut(|ctx, commands| {
            unsafe {
                // Create staging buffer
                let buffer_info = vk::BufferCreateInfo::default()
                    .size(data_size as vk::DeviceSize)
                    .usage(vk::BufferUsageFlags::TRANSFER_SRC)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE);

                let staging_buffer = match ctx.device.create_buffer(&buffer_info, None) {
                    Ok(buf) => buf,
                    Err(_) => return,
                };

                let mem_requirements = ctx.device.get_buffer_memory_requirements(staging_buffer);
                let memory_properties = ctx.instance.get_physical_device_memory_properties(ctx.physical_device);

                // Find host-visible memory type
                let memory_type_index = (0..memory_properties.memory_type_count)
                    .find(|&i| {
                        (mem_requirements.memory_type_bits & (1 << i)) != 0 &&
                        memory_properties.memory_types[i as usize].property_flags.contains(
                            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT
                        )
                    });

                let memory_type_index = match memory_type_index {
                    Some(i) => i,
                    None => {
                        ctx.device.destroy_buffer(staging_buffer, None);
                        return;
                    }
                };

                let alloc_info = vk::MemoryAllocateInfo::default()
                    .allocation_size(mem_requirements.size)
                    .memory_type_index(memory_type_index);

                let staging_memory = match ctx.device.allocate_memory(&alloc_info, None) {
                    Ok(mem) => mem,
                    Err(_) => {
                        ctx.device.destroy_buffer(staging_buffer, None);
                        return;
                    }
                };

                if ctx.device.bind_buffer_memory(staging_buffer, staging_memory, 0).is_err() {
                    ctx.device.free_memory(staging_memory, None);
                    ctx.device.destroy_buffer(staging_buffer, None);
                    return;
                }

                // Map and copy data to staging buffer
                let mapped_ptr = match ctx.device.map_memory(
                    staging_memory, 0, data_size as vk::DeviceSize, vk::MemoryMapFlags::empty()
                ) {
                    Ok(ptr) => ptr as *mut u8,
                    Err(_) => {
                        ctx.device.free_memory(staging_memory, None);
                        ctx.device.destroy_buffer(staging_buffer, None);
                        return;
                    }
                };

                std::ptr::copy_nonoverlapping(
                    byte_data.as_ptr(),
                    mapped_ptr,
                    data_size,
                );

                ctx.device.unmap_memory(staging_memory);

                // Build buffer→image copy region
                let copy_region = vk::BufferImageCopy::default()
                    .buffer_offset(0)
                    .buffer_row_length(0)
                    .buffer_image_height(0)
                    .image_subresource(vk::ImageSubresourceLayers {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        mip_level: 0,
                        base_array_layer: 0,
                        layer_count: 1,
                    })
                    .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
                    .image_extent(vk::Extent3D {
                        width: CINEMATIC_SIZE,
                        height: CINEMATIC_SIZE,
                        depth: 1,
                    });

                // Record and submit copy commands (commands already available, no nested lock)
                let cmd = match commands.begin_single_time() {
                    Ok(c) => c,
                    Err(_) => {
                        ctx.device.free_memory(staging_memory, None);
                        ctx.device.destroy_buffer(staging_buffer, None);
                        return;
                    }
                };

                // Transition image to TRANSFER_DST
                let barrier = vk::ImageMemoryBarrier::default()
                    .old_layout(vk::ImageLayout::UNDEFINED)
                    .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .image(texture)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    })
                    .src_access_mask(vk::AccessFlags::empty())
                    .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE);

                ctx.device.cmd_pipeline_barrier(
                    cmd,
                    vk::PipelineStageFlags::TOP_OF_PIPE,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[barrier],
                );

                // Copy buffer to image
                ctx.device.cmd_copy_buffer_to_image(
                    cmd,
                    staging_buffer,
                    texture,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &[copy_region],
                );

                // Transition image to SHADER_READ_ONLY
                let barrier = vk::ImageMemoryBarrier::default()
                    .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                    .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .image(texture)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    })
                    .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                    .dst_access_mask(vk::AccessFlags::SHADER_READ);

                ctx.device.cmd_pipeline_barrier(
                    cmd,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::FRAGMENT_SHADER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[barrier],
                );

                let _ = commands.end_single_time(ctx, cmd);

                // Clean up staging resources
                ctx.device.free_memory(staging_memory, None);
                ctx.device.destroy_buffer(staging_buffer, None);
            }
        });
    }

    /// Render 3D scene (BSP world + particles) to the scene FBO.
    ///
    /// Records Vulkan draw commands for all batched BSP surfaces and particles
    /// into the current command buffer, targeting the PostProcessor's scene FBO
    /// (R8G8B8A8_UNORM color + D32_SFLOAT depth).
    fn flush_3d_scene(&mut self) {
        // Build dynamic-light disc geometry (and static orbs in classic mode).
        // Must be called here because render_dlights() reads per-frame view vectors
        // that are set during begin_frame().
        self.render_dlights();

        // Prepare the VXGI irradiance volume descriptor (set 4) + grid params for the world pass.
        self.prepare_world_irradiance();

        let cmd = match self.current_command_buffer {
            Some(cmd) => cmd,
            None => return,
        };

        // Get scene FBO images
        let (scene_color, scene_color_view, scene_depth_view, scene_width, scene_height) = {
            let pp = match self.post_processor.as_ref() {
                Some(pp) => pp,
                None => return,
            };
            let scene = pp.scene_fbo();
            match (scene.color_image(), scene.color_view(), scene.depth_view()) {
                (Some(img), Some(cv), Some(dv)) => (img, cv, dv, scene.width(), scene.height()),
                _ => return,
            }
        };

        let scene_extent = vk::Extent2D { width: scene_width, height: scene_height };

        // Get pipeline data
        let (world_pipeline, world_layout) = match self.pipelines.as_ref()
            .and_then(|pm| pm.get(ShaderType::World, PipelineVariant::Opaque))
            .map(|p| (p.pipeline, p.layout))
        {
            Some(data) => data,
            None => return,
        };

        // Pre-gather BSP batch data and descriptor sets
        let bsp_vbo = self.bsp_geometry.vertex_buffer().vk_buffer();
        let bsp_ibo = self.bsp_geometry.index_buffer().vk_buffer();
        let bsp_batches: Vec<(u32, u32, Option<vk::DescriptorSet>, bool)> = self.bsp_geometry.batches().iter()
            .map(|b| {
                let ds = super::texture::ensure_descriptor_set(b.texture_id as i32)
                    .or_else(|| super::texture::ensure_descriptor_set(0));
                (b.first_index, b.index_count, ds, b.is_light)
            })
            .collect();
        let has_bsp = bsp_vbo.is_some() && bsp_ibo.is_some() && !bsp_batches.is_empty();

        // Particle data
        let particle_count = self.particles.count();
        let particle_quad_vbo = self.particles.quad_buffer().vk_buffer();
        let particle_instance_vbo = self.particles.instance_buffer().vk_buffer();
        let particle_pipeline_data = self.pipelines.as_ref()
            .and_then(|pm| pm.get(ShaderType::Particle, PipelineVariant::Additive))
            .map(|p| (p.pipeline, p.layout));
        let has_particles = particle_count > 0
            && particle_quad_vbo.is_some()
            && particle_instance_vbo.is_some()
            && particle_pipeline_data.is_some();

        // Get particle texture descriptor
        let particle_ds = {
            // Accessing renderer globals for particle texture, main thread only
            let texnum = {
                let globals = crate::vk_rmain::rg();
                globals.r_particletexture[0].as_ref().map(|img| img.texnum).unwrap_or(0)
            };
            super::texture::ensure_descriptor_set(texnum)
                .or_else(|| super::texture::ensure_descriptor_set(0))
        };

        // Compute view-projection as flat [f32; 16]
        let vp_flat: [f32; 16] = {
            let vp = &self.frame_uniforms.view_projection;
            [
                vp[0][0], vp[0][1], vp[0][2], vp[0][3],
                vp[1][0], vp[1][1], vp[1][2], vp[1][3],
                vp[2][0], vp[2][1], vp[2][2], vp[2][3],
                vp[3][0], vp[3][1], vp[3][2], vp[3][3],
            ]
        };
        // World MVP = VP (identity model matrix)
        let mvp = vp_flat;

        // Pre-gather brush model draw data (surface ranges with per-surface descriptors)
        let brush_model_draws: Vec<([f32; 16], [f32; 9], Vec<(u32, u32, Option<vk::DescriptorSet>)>)> =
            self.brush_models.iter().map(|bm| {
                let surfaces: Vec<_> = self.bsp_geometry
                    .draw_info_for_range(bm.first_surface, bm.num_surfaces)
                    .iter()
                    .map(|surf| {
                        let ds = super::texture::ensure_descriptor_set(surf.texture_id as i32)
                            .or_else(|| super::texture::ensure_descriptor_set(0));
                        (surf.first_index, surf.index_count, ds)
                    })
                    .collect();
                (bm.mvp, bm.dyn_block, surfaces)
            })
            .collect();

        // Pre-gather alias model draw data
        // Each entry: (vbo, ibo, index_count, skin_ds, Vec<(push_constants, frame_offset)>)
        struct AliasDrawEntry {
            vbo: vk::Buffer,
            ibo: vk::Buffer,
            index_count: u32,
            skin_ds: Option<vk::DescriptorSet>,
            // (push_constants, frame_offset, depth_hack) — depth_hack = RF_DEPTHHACK view
            // weapon, drawn in a compressed depth range.
            instances: Vec<(AliasPushConstants, u64, bool)>,
            // Blob-shadow pass: same posed geometry flattened onto the entity's ground
            // plane, drawn black + translucent on the alpha-blend pipeline.
            shadow_instances: Vec<(AliasPushConstants, u64)>,
        }

        let alias_pipeline_data = self.pipelines.as_ref()
            .and_then(|pm| pm.get(ShaderType::Alias, PipelineVariant::Opaque))
            .map(|p| (p.pipeline, p.layout));
        // Blob-shadow pass reuses the alias vertex shader on the alpha-blend pipeline.
        let alias_shadow_pipeline_data = self.pipelines.as_ref()
            .and_then(|pm| pm.get(ShaderType::Alias, PipelineVariant::AlphaBlend))
            .map(|p| (p.pipeline, p.layout));
        let cast_shadows = crate::vk_rmain::rcvars().vk_shadows.value != 0.0;

        let mut alias_draw_entries: Vec<AliasDrawEntry> = Vec::new();
        let alias_registered_count = self.alias_models.model_count();
        if alias_pipeline_data.is_some() {
            let mut alias_batch_found = 0u32;
            for batch in self.alias_instanced.batches() {
                alias_batch_found += 1;
                let model_id = batch.model_id();
                let model_buffers = match self.alias_models.get(model_id) {
                    Some(b) => b,
                    None => {
                        eprintln!("ALIAS: model_id={:#x} NOT FOUND in alias_models registry", model_id);
                        continue;
                    }
                };
                let alias_vbo = match model_buffers.vertex_buffer().vk_buffer() {
                    Some(b) => b,
                    None => continue,
                };
                let alias_ibo = match model_buffers.index_buffer().vk_buffer() {
                    Some(b) => b,
                    None => continue,
                };

                // Get skin texture descriptor from model
                let skin_ds = {
                    // SAFETY: model_id is a valid Model pointer, main thread only
                    let texnum = unsafe {
                        let model = &*(model_id as *const crate::vk_model_types::Model);
                        // Use first non-null skin, fallback to 0
                        model.skins.iter()
                            .find_map(|&s| {
                                if s.is_null() { None }
                                else { Some((*s).texnum) }
                            })
                            .unwrap_or(0)
                    };
                    super::texture::ensure_descriptor_set(texnum)
                        .or_else(|| super::texture::ensure_descriptor_set(0))
                };

                let index_count = model_buffers.index_count();
                let mut instances = Vec::new();
                let mut shadow_instances = Vec::new();

                for instance in batch.instances() {
                    // RF_VIEWERMODEL = the player's own body: a shadow-only caster (it's still
                    // rendered into the shadow map by render_projective_shadows). Don't draw it
                    // in the first-person view.
                    if instance.flags & myq2_common::q_shared::RF_VIEWERMODEL as u32 != 0 {
                        continue;
                    }
                    let model_flat = mat4_to_flat(&instance.model_matrix);
                    let inst_mvp = ModernRenderPath::mat4_multiply(&vp_flat, &model_flat);
                    let frontlerp = 1.0 - instance.backlerp;
                    // RF_SHELL_RED=0x04000, RF_SHELL_GREEN=0x08000, RF_SHELL_BLUE=0x10000
                    let is_shell = (instance.flags & 0x1C000) != 0;
                    let frame = instance.frame.min(model_buffers.frame_count().saturating_sub(1));
                    let frame_offset = model_buffers.frame_offset(frame) as u64;

                    let pc = AliasPushConstants {
                        mvp: inst_mvp,
                        shade_light: instance.shade_light,
                        alpha: instance.alpha,
                        move_vec: [0.0, 0.0, 0.0],
                        backlerp: instance.backlerp,
                        front_v: [frontlerp, frontlerp, frontlerp],
                        shell_scale: if is_shell { 1.0 } else { 0.0 },
                        back_v: [instance.backlerp, instance.backlerp, instance.backlerp],
                        is_shell: if is_shell { 1 } else { 0 },
                    };
                    // RF_DEPTHHACK (view weapon): draw in a compressed depth range so the gun
                    // never intersects the near plane or pokes into world geometry — the
                    // original's glDepthRange(0, 0.3). Applied via the dynamic viewport.
                    let depth_hack =
                        instance.flags & myq2_common::q_shared::RF_DEPTHHACK as u32 != 0;
                    instances.push((pc, frame_offset, depth_hack));

                    // Blob shadow: project the SAME posed geometry onto the entity's ground
                    // plane (a world-space z-flatten baked into the MVP), drawn black and
                    // translucent. Skip weapon-view and already-translucent models, matching
                    // classic Q2. Gated by the vk_shadows cvar.
                    // Blob shadows retired in favour of the projective shadow system
                    // (render_projective_shadows), which conforms to geometry / climbs walls.
                    let casts_shadow = false
                        && cast_shadows
                        && instance.shadow_ground_z.is_finite()
                        && (instance.flags
                            & (myq2_common::q_shared::RF_WEAPONMODEL
                                | myq2_common::q_shared::RF_TRANSLUCENT) as u32)
                            == 0;
                    if casts_shadow {
                        let f = shadow_flatten_matrix(instance.shadow_skew, instance.shadow_ground_z);
                        let shadow_world = ModernRenderPath::mat4_multiply(&f, &model_flat);
                        let shadow_mvp = ModernRenderPath::mat4_multiply(&vp_flat, &shadow_world);
                        let shadow_pc = AliasPushConstants {
                            mvp: shadow_mvp,
                            shade_light: [0.0, 0.0, 0.0], // black silhouette
                            alpha: 0.5,                   // shadow darkness
                            move_vec: [0.0, 0.0, 0.0],
                            backlerp: instance.backlerp,
                            front_v: [frontlerp, frontlerp, frontlerp],
                            shell_scale: 0.0,
                            back_v: [instance.backlerp, instance.backlerp, instance.backlerp],
                            is_shell: 0,
                        };
                        shadow_instances.push((shadow_pc, frame_offset));
                    }
                }

                if !instances.is_empty() {
                    alias_draw_entries.push(AliasDrawEntry {
                        vbo: alias_vbo,
                        ibo: alias_ibo,
                        index_count,
                        skin_ds,
                        instances,
                        shadow_instances,
                    });
                }
            }
        }

        // Pre-gather alpha surface data
        let alpha_blend_pipeline_data = self.pipelines.as_ref()
            .and_then(|pm| pm.get(ShaderType::World, PipelineVariant::AlphaBlend))
            .map(|p| (p.pipeline, p.layout));

        let alpha_surface_draws: Vec<(u32, u32, f32, Option<vk::DescriptorSet>)> = {
            let vo = self.frame_uniforms.view_origin;
            let mut draws: Vec<(f32, u32, u32, f32, Option<vk::DescriptorSet>)> =
                self.bsp_geometry.alpha_surfaces().iter().map(|surf| {
                    let alpha = if surf.flags & super::geometry::SURF_TRANS33 != 0 {
                        0.33_f32
                    } else {
                        0.66
                    };
                    let ds = super::texture::ensure_descriptor_set(surf.texture_id as i32)
                        .or_else(|| super::texture::ensure_descriptor_set(0));
                    let c = surf.centroid;
                    let d2 = (c[0] - vo[0]).powi(2) + (c[1] - vo[1]).powi(2) + (c[2] - vo[2]).powi(2);
                    (d2, surf.first_index, surf.index_count, alpha, ds)
                })
                .collect();
            // Back-to-front so overlapping translucent surfaces composite correctly (they were
            // drawn in texture order, which layers near glass under far glass at random).
            draws.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            draws.into_iter().map(|(_, fi, ic, a, ds)| (fi, ic, a, ds)).collect()
        };

        // Pre-gather turb (water/lava/slime) surface data
        // Opaque/depth-writing water (see pipeline creation note) so it reads as solid and the
        // GI pass samples the water surface, not the floor behind it.
        let water_pipeline_data = self.pipelines.as_ref()
            .and_then(|pm| pm.get(ShaderType::Water, PipelineVariant::Opaque))
            .map(|p| (p.pipeline, p.layout));
        // TRANS-flagged water draws late with real alpha blending (see the translucent-water
        // block after sky/entities); the Opaque pipeline handles untagged liquids (lava etc.).
        let water_ab_pipeline_data = self.pipelines.as_ref()
            .and_then(|pm| pm.get(ShaderType::Water, PipelineVariant::AlphaBlend))
            .map(|p| (p.pipeline, p.layout));

        // Planar reflection plane: pick the dominant horizontal water plane (largest summed
        // index count across surfaces at the same height). Surfaces on other heights simply
        // don't reflect this frame. Bucket by 4-unit-rounded z so co-planar faces group.
        let refl_enabled = crate::vk_rmain::rcvars().r_water_reflect.fresh_value() != 0.0;
        let refl_plane_z: Option<f32> = if refl_enabled {
            let mut planes: std::collections::HashMap<i32, (u32, f32)> = std::collections::HashMap::new();
            for surf in self.bsp_geometry.turb_surfaces() {
                if !surf.is_horizontal {
                    continue;
                }
                let key = (surf.plane_z / 4.0).round() as i32;
                let e = planes.entry(key).or_insert((0, surf.plane_z));
                e.0 += surf.index_count;
            }
            planes.values().max_by_key(|(count, _)| *count).map(|&(_, z)| z)
        } else {
            None
        };

        let turb_surface_draws: Vec<(u32, u32, f32, [f32; 3], Option<vk::DescriptorSet>, bool)> =
            self.bsp_geometry.turb_surfaces().iter().map(|surf| {
                // Turb surfaces with TRANS flags get appropriate alpha, otherwise opaque
                let alpha = if surf.flags & super::geometry::SURF_TRANS33 != 0 {
                    0.33_f32
                } else if surf.flags & super::geometry::SURF_TRANS66 != 0 {
                    0.66
                } else {
                    1.0 // Opaque turb (lava, etc.)
                };
                let ds = super::texture::ensure_descriptor_set(surf.texture_id as i32)
                    .or_else(|| super::texture::ensure_descriptor_set(0));
                // Reflect only translucent water sitting on the chosen plane (not lava/slime,
                // whose opaque alpha=1.0 look would be wrong with a mirror on top).
                let on_plane = alpha < 1.0
                    && surf.is_horizontal
                    && refl_plane_z.map(|pz| (surf.plane_z - pz).abs() < 4.0).unwrap_or(false);
                (surf.first_index, surf.index_count, alpha, surf.flat_light, ds, on_plane)
            })
            .collect();
        let has_turb = !turb_surface_draws.is_empty() && water_pipeline_data.is_some() && has_bsp;
        let water_time = self.frame_uniforms.time;
        // Reflection pass runs only when a plane was picked AND some visible water sits on it.
        let refl_plane_z = refl_plane_z
            .filter(|_| turb_surface_draws.iter().any(|&(.., on_plane)| on_plane));
        self.frame_refl_plane = refl_plane_z; // shimmer pass reads this after the scene
        if refl_plane_z.is_some() {
            self.prepare_water_reflection(scene_width, scene_height);
        }
        // Everything the mirrored pass needs, gathered before the device closure:
        // (color image, color view, depth view, w, h, mirrored VP, mirrored sky MVP).
        let refl_data: Option<(vk::Image, vk::Image, vk::ImageView, vk::ImageView, u32, u32, [f32; 16], [f32; 16], f32, f32)> =
            refl_plane_z.and_then(|pz| {
                if !has_bsp || self.refl_desc_set.is_none() {
                    return None;
                }
                let t = self.refl_target.as_ref()?;
                let (img, d_img, cv, dv) = (t.color_image()?, t.depth_image()?, t.color_view()?, t.depth_view()?);
                // Reflect about the plane z = pz (column-major flat): z' = 2·pz − z.
                let mirror: [f32; 16] = [
                    1.0, 0.0, 0.0, 0.0,
                    0.0, 1.0, 0.0, 0.0,
                    0.0, 0.0, -1.0, 0.0,
                    0.0, 0.0, 2.0 * pz, 1.0,
                ];
                let refl_vp = Self::mat4_multiply(&vp_flat, &mirror);
                let refl_sky_mvp = Self::mat4_multiply(&refl_vp, &self.sky_model_flat);
                // Which side of the plane the camera is on decides which half of the world the
                // mirror shows: above → the room above the water; below (swimming) → the
                // submerged world reflecting off the surface's underside.
                let side = if self.frame_uniforms.view_origin[2] >= pz { 1.0_f32 } else { -1.0 };
                Some((img, d_img, cv, dv, t.width(), t.height(), refl_vp, refl_sky_mvp, pz, side))
            });
        // DIAGNOSTIC (temporary): which reflection gate fails. Logged every ~5s.
        if refl_enabled {
            static RF: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            if RF.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % 300 == 0 {
                let turbs = self.bsp_geometry.turb_surfaces();
                let horiz = turbs.iter().filter(|s| s.is_horizontal).count();
                let on_plane_ct = turb_surface_draws.iter().filter(|&&(.., op)| op).count();
                eprintln!(
                    "[refl] turb={} horiz={} trans={} plane={:?} target={} set={} data={}",
                    turbs.len(),
                    horiz,
                    turb_surface_draws.iter().filter(|&&(_, _, a, ..)| a < 1.0).count(),
                    refl_plane_z,
                    self.refl_target.is_some(),
                    self.refl_desc_set.is_some(),
                    refl_data.is_some(),
                );
                let _ = on_plane_ct;
            }
        }

        // Pre-gather sprite draw data
        let sprite_pipeline_data = alpha_blend_pipeline_data; // Sprites use World/AlphaBlend
        let sprite_verts: Vec<BspVertex> = self.sprite_draws.iter()
            .flat_map(|d| d.vertices.iter().copied())
            .collect();
        let sprite_indices: Vec<u32> = (0..self.sprite_draws.len() as u32)
            .flat_map(|i| {
                let base = i * 4;
                [base, base + 1, base + 2, base, base + 2, base + 3]
            })
            .collect();
        let sprite_draw_info: Vec<(f32, Option<vk::DescriptorSet>)> = self.sprite_draws.iter()
            .map(|d| {
                let ds = super::texture::ensure_descriptor_set(d.texture_id as i32)
                    .or_else(|| super::texture::ensure_descriptor_set(0));
                (d.alpha, ds)
            })
            .collect();
        let has_sprites = !sprite_verts.is_empty() && sprite_pipeline_data.is_some();

        // Pre-gather sky draw data
        let sky_pipeline_data = self.pipelines.as_ref()
            .and_then(|pm| pm.get(ShaderType::Sky, PipelineVariant::Opaque))
            .map(|p| (p.pipeline, p.layout));
        let sky_verts = self.sky_vertices.clone();
        let sky_idxs = self.sky_indices.clone();
        let sky_draws: Vec<(u32, u32, u32, Option<vk::DescriptorSet>)> =
            self.sky_face_draws.iter().map(|&(tex_id, first_idx, count)| {
                let ds = super::texture::ensure_descriptor_set(tex_id as i32)
                    .or_else(|| super::texture::ensure_descriptor_set(0));
                (tex_id, first_idx, count, ds)
            }).collect();
        let sky_mvp_flat = self.sky_mvp;
        let has_sky = !sky_verts.is_empty() && sky_pipeline_data.is_some();

        // Pre-gather dynamic light draw data
        let dlight_pipeline_data = self.pipelines.as_ref()
            .and_then(|pm| pm.get(ShaderType::DynamicLight, PipelineVariant::Additive))
            .map(|p| (p.pipeline, p.layout));
        let dlight_verts = self.dlight_vertices.clone();
        let dlight_idxs = self.dlight_indices.clone();
        let dlight_pcs = self.dlight_draws.iter().map(|d| {
            DlightPushConstants {
                mvp: d.mvp,
                light_origin: d.light_origin,
                light_radius: d.light_radius,
                light_color: d.light_color,
                _pad: 0.0,
            }
        }).collect::<Vec<_>>();
        let has_dlights = !dlight_verts.is_empty() && dlight_pipeline_data.is_some();

        // Pre-gather detail texture overlay data
        let detail_pipeline_data = self.pipelines.as_ref()
            .and_then(|pm| pm.get(ShaderType::World, PipelineVariant::Multiplicative))
            .map(|p| (p.pipeline, p.layout));
        let detail_descriptor_set = crate::vk_warp::with_warp_state(|ws| {
            if ws.detailtexture.is_null() { return None; }
            // SAFETY: detailtexture is a valid Image pointer loaded during R_Init
            let texnum = unsafe { (*ws.detailtexture).texnum };
            super::texture::ensure_descriptor_set(texnum)
        });
        let has_detail_overlay = detail_pipeline_data.is_some()
            && detail_descriptor_set.is_some()
            && has_bsp;

        {
            static DETAIL_DIAG: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = DETAIL_DIAG.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if n < 3 {
                eprintln!("[DETAIL-RENDER] pipeline={} descriptor={} has_bsp={} -> has_detail_overlay={}",
                    detail_pipeline_data.is_some(), detail_descriptor_set.is_some(), has_bsp, has_detail_overlay);
            }
        }

        // Build particle push constants (112 bytes)
        #[repr(C)]
        struct ParticlePushConstants {
            view_projection: [f32; 16],
            view_up: [f32; 3],
            min_size: f32,
            view_right: [f32; 3],
            max_size: f32,
            view_origin: [f32; 3],
            _pad0: f32,
        }

        let particle_pc = ParticlePushConstants {
            view_projection: mvp,
            view_up: self.frame_uniforms.view_up,
            min_size: 2.0,
            view_right: self.frame_uniforms.view_right,
            max_size: 40.0,
            view_origin: self.frame_uniforms.view_origin,
            _pad0: 0.0,
        };

        // Pre-gather lightmap descriptor set for per-pixel sampling (set 2)
        let lightmap_ds = self.lightmap_descriptor_set;

        // Upload sprite/sky/dlight vertex data BEFORE the with_device closure
        // to avoid re-entrant deadlock — upload() calls with_device() internally.
        if has_sprites {
            self.sprite_vbo.upload(&sprite_verts, 0);
            self.sprite_ibo.upload_u32(&sprite_indices, 0);
        }
        if has_sky {
            self.sky_vbo.upload(&sky_verts, 0);
            self.sky_ibo.upload_u32(&sky_idxs, 0);
        }
        if has_dlights {
            // SAFETY: [f32; 6] is 24 bytes (pos3 + color3)
            unsafe {
                let dlight_bytes: &[u8] = std::slice::from_raw_parts(
                    dlight_verts.as_ptr() as *const u8,
                    dlight_verts.len() * 24,
                );
                self.dlight_vbo.upload(std::slice::from_raw_parts(
                    dlight_bytes.as_ptr() as *const [f32; 6],
                    dlight_verts.len(),
                ), 0);
            }
            self.dlight_ibo.upload_u32(&dlight_idxs, 0);
        }

        gpu_device::with_device(|ctx| {
            // SAFETY: Vulkan context valid, called from main thread only, within
            // active command buffer recording started in begin_frame().
            unsafe {
                // === Planar water reflection: mirrored world+sky into the half-res target ===
                // Rendered BEFORE the main scene so the water pass can sample the result. Uses
                // the ordinary world/sky pipelines (they cull NONE, so the mirrored winding is
                // fine) with MVP = VP × mirror(z = plane). world.frag discards geometry below
                // the plane via the u_DynInvSize clip push (see shader comment).
                if let Some((r_img, r_d_img, r_cv, r_dv, r_w, r_h, refl_vp, refl_sky_mvp, plane_z, refl_side)) = refl_data {
                    // Both attachments start UNDEFINED (contents are cleared, and last frame's
                    // colour was left in SHADER_READ). Depth also transitions from UNDEFINED
                    // every frame — RenderTarget does no initial layout transition of its own.
                    let barriers = [
                        vk::ImageMemoryBarrier::default()
                            .old_layout(vk::ImageLayout::UNDEFINED)
                            .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                            .image(r_img)
                            .subresource_range(vk::ImageSubresourceRange {
                                aspect_mask: vk::ImageAspectFlags::COLOR,
                                base_mip_level: 0,
                                level_count: 1,
                                base_array_layer: 0,
                                layer_count: 1,
                            })
                            .src_access_mask(vk::AccessFlags::empty())
                            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE),
                        vk::ImageMemoryBarrier::default()
                            .old_layout(vk::ImageLayout::UNDEFINED)
                            .new_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                            .image(r_d_img)
                            .subresource_range(vk::ImageSubresourceRange {
                                aspect_mask: vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL,
                                base_mip_level: 0,
                                level_count: 1,
                                base_array_layer: 0,
                                layer_count: 1,
                            })
                            .src_access_mask(vk::AccessFlags::empty())
                            .dst_access_mask(
                                vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                            ),
                    ];
                    ctx.device.cmd_pipeline_barrier(
                        cmd,
                        vk::PipelineStageFlags::FRAGMENT_SHADER,
                        vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                            | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                            | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                        vk::DependencyFlags::empty(),
                        &[], &[], &barriers,
                    );

                    let r_color_att = vk::RenderingAttachmentInfo::default()
                        .image_view(r_cv)
                        .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .load_op(vk::AttachmentLoadOp::CLEAR)
                        .store_op(vk::AttachmentStoreOp::STORE)
                        .clear_value(vk::ClearValue {
                            color: vk::ClearColorValue { float32: [0.0, 0.0, 0.0, 1.0] },
                        });
                    let r_depth_att = vk::RenderingAttachmentInfo::default()
                        .image_view(r_dv)
                        .image_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                        .load_op(vk::AttachmentLoadOp::CLEAR)
                        .store_op(vk::AttachmentStoreOp::DONT_CARE)
                        .clear_value(vk::ClearValue {
                            depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 },
                        });
                    let r_extent = vk::Extent2D { width: r_w, height: r_h };
                    let r_info = vk::RenderingInfo::default()
                        .render_area(vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent: r_extent })
                        .layer_count(1)
                        .color_attachments(std::slice::from_ref(&r_color_att))
                        .depth_attachment(&r_depth_att);
                    ctx.device.cmd_begin_rendering(cmd, &r_info);

                    // Same negative-height viewport convention as the main pass, so the mirrored
                    // render maps to the same normalized screen positions the water shader samples.
                    let r_viewport = vk::Viewport {
                        x: 0.0,
                        y: r_extent.height as f32,
                        width: r_extent.width as f32,
                        height: -(r_extent.height as f32),
                        min_depth: 0.0,
                        max_depth: 1.0,
                    };
                    ctx.device.cmd_set_viewport(cmd, 0, &[r_viewport]);
                    ctx.device.cmd_set_scissor(cmd, 0, &[vk::Rect2D {
                        offset: vk::Offset2D { x: 0, y: 0 },
                        extent: r_extent,
                    }]);

                    if has_bsp {
                        let bsp_vbo_handle = bsp_vbo.unwrap();
                        let bsp_ibo_handle = bsp_ibo.unwrap();
                        ctx.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, world_pipeline);
                        ctx.device.cmd_bind_vertex_buffers(cmd, 0, &[bsp_vbo_handle], &[0]);
                        ctx.device.cmd_bind_index_buffer(cmd, bsp_ibo_handle, 0, vk::IndexType::UINT32);

                        let mvp_bytes: &[u8] = std::slice::from_raw_parts(refl_vp.as_ptr() as *const u8, 64);
                        ctx.device.cmd_push_constants(
                            cmd, world_layout,
                            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                            0, mvp_bytes,
                        );
                        // alpha=1, overbright, then tail 72..96: scroll, gamma, contrast, lift,
                        // lm_scale=1.0 (classic full baked — no GI sampling in the mirror),
                        // use_dyn=0. Emit=1 at 96 (no HDR boost in the reflection).
                        let r_ob = crate::vk_rmain::rcvars().r_overbrightbits.value.max(1.0);
                        let head: [f32; 2] = [1.0, r_ob];
                        ctx.device.cmd_push_constants(
                            cmd, world_layout,
                            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                            64, std::slice::from_raw_parts(head.as_ptr() as *const u8, 8),
                        );
                        let tail: [f32; 7] = [
                            0.0,
                            crate::vk_rmain::rcvars().r_lightmap_gamma.value,
                            crate::vk_rmain::rcvars().r_lightmap_contrast.value,
                            crate::vk_rmain::rcvars().r_shadowlift.value,
                            1.0, // lm_scale: full baked, GI branch off
                            0.0, // use_dyn
                            1.0, // emit
                        ];
                        ctx.device.cmd_push_constants(
                            cmd, world_layout,
                            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                            72, std::slice::from_raw_parts(tail.as_ptr() as *const u8, 28),
                        );
                        // Clip: keep only geometry on the camera's side of the plane (+1 = above,
                        // −1 = below for an underwater camera) — the other half's mirror image
                        // would wrongly occlude the reflections.
                        let clip: [f32; 2] = [refl_side, plane_z];
                        ctx.device.cmd_push_constants(
                            cmd, world_layout,
                            vk::ShaderStageFlags::FRAGMENT,
                            120, std::slice::from_raw_parts(clip.as_ptr() as *const u8, 8),
                        );
                        if let Some(lm_ds) = lightmap_ds {
                            ctx.device.cmd_bind_descriptor_sets(
                                cmd, vk::PipelineBindPoint::GRAPHICS, world_layout, 2, &[lm_ds], &[],
                            );
                        }
                        if let Some(irr_set) = self.vxgi_world_irr_set {
                            ctx.device.cmd_bind_descriptor_sets(
                                cmd, vk::PipelineBindPoint::GRAPHICS, world_layout, 4, &[irr_set], &[],
                            );
                        }
                        for &(first_index, index_count, ds, _is_light) in &bsp_batches {
                            if let Some(ds) = ds {
                                ctx.device.cmd_bind_descriptor_sets(
                                    cmd, vk::PipelineBindPoint::GRAPHICS, world_layout, 1, &[ds], &[],
                                );
                            }
                            ctx.device.cmd_draw_indexed(cmd, index_count, 1, first_index, 0, 0);
                        }
                    }

                    // Mirrored sky so open-air water reflects the skybox. Skipped underwater —
                    // the submerged world has no sky; unlit areas stay the dark clear colour,
                    // which reads correctly as murky depth in the internal reflection.
                    if has_sky && refl_side > 0.0 {
                        let (sky_pipeline, sky_layout) = sky_pipeline_data.unwrap();
                        ctx.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, sky_pipeline);
                        let sm_bytes: &[u8] = std::slice::from_raw_parts(refl_sky_mvp.as_ptr() as *const u8, 64);
                        ctx.device.cmd_push_constants(
                            cmd, sky_layout,
                            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                            0, sm_bytes,
                        );
                        let sky_head: [f32; 2] = [1.0, 1.0]; // alpha, overbright
                        ctx.device.cmd_push_constants(
                            cmd, sky_layout,
                            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                            64, std::slice::from_raw_parts(sky_head.as_ptr() as *const u8, 8),
                        );
                        if let (Some(s_vbo), Some(s_ibo)) = (self.sky_vbo.vk_buffer(), self.sky_ibo.vk_buffer()) {
                            ctx.device.cmd_bind_vertex_buffers(cmd, 0, &[s_vbo], &[0]);
                            ctx.device.cmd_bind_index_buffer(cmd, s_ibo, 0, vk::IndexType::UINT32);
                            for &(_, first_idx, count, ds) in &sky_draws {
                                if let Some(ds) = ds {
                                    ctx.device.cmd_bind_descriptor_sets(
                                        cmd, vk::PipelineBindPoint::GRAPHICS, sky_layout, 1, &[ds], &[],
                                    );
                                }
                                ctx.device.cmd_draw_indexed(cmd, count, 1, first_idx, 0, 0);
                            }
                        }
                    }

                    ctx.device.cmd_end_rendering(cmd);

                    // Reflection colour → sampleable for the water pass in the main scene.
                    let to_read = vk::ImageMemoryBarrier::default()
                        .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                        .image(r_img)
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        })
                        .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                        .dst_access_mask(vk::AccessFlags::SHADER_READ);
                    ctx.device.cmd_pipeline_barrier(
                        cmd,
                        vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                        vk::PipelineStageFlags::FRAGMENT_SHADER,
                        vk::DependencyFlags::empty(),
                        &[], &[], &[to_read],
                    );
                }

                // Transition scene_fbo color: UNDEFINED → COLOR_ATTACHMENT_OPTIMAL
                let color_barrier = vk::ImageMemoryBarrier::default()
                    .old_layout(vk::ImageLayout::UNDEFINED)
                    .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .image(scene_color)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    })
                    .src_access_mask(vk::AccessFlags::empty())
                    .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE);

                ctx.device.cmd_pipeline_barrier(
                    cmd,
                    vk::PipelineStageFlags::TOP_OF_PIPE,
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    vk::DependencyFlags::empty(),
                    &[], &[], &[color_barrier],
                );

                // Begin dynamic rendering with color + depth
                let color_att = vk::RenderingAttachmentInfo::default()
                    .image_view(scene_color_view)
                    .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .load_op(vk::AttachmentLoadOp::CLEAR)
                    .store_op(vk::AttachmentStoreOp::STORE)
                    .clear_value(vk::ClearValue {
                        color: vk::ClearColorValue { float32: [0.0, 0.0, 0.0, 1.0] },
                    });
                let depth_att = vk::RenderingAttachmentInfo::default()
                    .image_view(scene_depth_view)
                    .image_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                    .load_op(vk::AttachmentLoadOp::CLEAR)
                    .store_op(vk::AttachmentStoreOp::STORE)
                    .clear_value(vk::ClearValue {
                        depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 },
                    });

                let rendering_info = vk::RenderingInfo::default()
                    .render_area(vk::Rect2D {
                        offset: vk::Offset2D { x: 0, y: 0 },
                        extent: scene_extent,
                    })
                    .layer_count(1)
                    .color_attachments(std::slice::from_ref(&color_att))
                    .depth_attachment(&depth_att);

                ctx.device.cmd_begin_rendering(cmd, &rendering_info);

                // Use negative height for Vulkan Y-flip (to match OpenGL Y-up coordinates)
                let viewport = vk::Viewport {
                    x: 0.0,
                    y: scene_extent.height as f32,
                    width: scene_extent.width as f32,
                    height: -(scene_extent.height as f32),
                    min_depth: 0.0, max_depth: 1.0,
                };
                ctx.device.cmd_set_viewport(cmd, 0, &[viewport]);
                let scissor = vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: scene_extent,
                };
                ctx.device.cmd_set_scissor(cmd, 0, &[scissor]);

                // === BSP World (opaque) ===
                if has_bsp {
                    let bsp_vbo_handle = bsp_vbo.unwrap();
                    let bsp_ibo_handle = bsp_ibo.unwrap();

                    ctx.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, world_pipeline);

                    // Push MVP (64 bytes) + alpha=1.0 (4 bytes) for opaque world
                    let mvp_bytes: &[u8] = std::slice::from_raw_parts(
                        mvp.as_ptr() as *const u8, 64,
                    );
                    ctx.device.cmd_push_constants(
                        cmd, world_layout,
                        vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                        0, mvp_bytes,
                    );
                    let opaque_alpha = 1.0_f32;
                    let alpha_bytes: &[u8] = std::slice::from_raw_parts(
                        &opaque_alpha as *const f32 as *const u8, 4,
                    );
                    ctx.device.cmd_push_constants(
                        cmd, world_layout,
                        vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                        64, alpha_bytes,
                    );

                    // Push overbright scale at offset 68 (4 bytes) — recovers dynamic
                    // range lost when lightmap values are stored at reduced scale
                    let overbright = crate::vk_rmain::rcvars().r_overbrightbits.value.max(1.0);
                    let overbright_bytes: &[u8] = std::slice::from_raw_parts(
                        &overbright as *const f32 as *const u8, 4,
                    );
                    ctx.device.cmd_push_constants(
                        cmd, world_layout,
                        vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                        68, overbright_bytes,
                    );

                    // Push remaining world push constants (offsets 72–88):
                    //   72: u_UvScroll        — UV scroll for turb/water (0.0 for static world)
                    //   76: u_LightmapGamma   — power curve for lightmap values
                    //   80: u_LightmapContrast — linear contrast around pivot 0.35
                    //   84: u_ShadowLift      — minimum brightness floor
                    //   88: u_LightmapScale   — >=1.0 = classic lightmap, <1.0 = D3 flat ambient
                    let max_d3 = crate::vk_rmain::rcvars().r_d3_maxlights.fresh_value() as i32;
                    let lm_scale = if max_d3 > 0 {
                        // D3 mode: clamp to 0.99 so ambient=1.0 doesn't accidentally
                        // flip to the classic lightmap branch (which requires >= 1.0).
                        crate::vk_rmain::rcvars().r_d3_ambient.fresh_value().clamp(0.0, 0.99)
                    } else if crate::vk_rmain::rcvars().r_vxgi.fresh_value() != 0.0 {
                        // r_vxgi_bake dims the baked lightmap so VXGI can drive the lighting. Only
                        // when VXGI is on (its irradiance volume / set 4 must be bound to sample).
                        crate::vk_rmain::rcvars().r_vxgi_bake.fresh_value().clamp(0.0, 1.0)
                    } else {
                        1.0_f32
                    };
                    let world_tail: [f32; 6] = [
                        0.0,  // u_UvScroll
                        crate::vk_rmain::rcvars().r_lightmap_gamma.value,
                        crate::vk_rmain::rcvars().r_lightmap_contrast.value,
                        crate::vk_rmain::rcvars().r_shadowlift.value,
                        lm_scale,
                        0.0,  // u_UseDynLight = 0 for the static world
                    ];
                    let tail_bytes: &[u8] = std::slice::from_raw_parts(
                        world_tail.as_ptr() as *const u8, 24,
                    );
                    ctx.device.cmd_push_constants(
                        cmd, world_layout,
                        vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                        72, tail_bytes,
                    );
                    // Clear the reflection-clip slot (offset 120): the mirrored pass earlier in
                    // this command buffer left (1, planeZ) there, which would discard every main-
                    // view fragment below the water plane. Push-constant state persists across
                    // pipeline binds, so this must be explicit.
                    let clip_off: [f32; 2] = [0.0, 0.0];
                    ctx.device.cmd_push_constants(
                        cmd, world_layout,
                        vk::ShaderStageFlags::FRAGMENT,
                        120, std::slice::from_raw_parts(clip_off.as_ptr() as *const u8, 8),
                    );

                    ctx.device.cmd_bind_vertex_buffers(cmd, 0, &[bsp_vbo_handle], &[0]);
                    ctx.device.cmd_bind_index_buffer(cmd, bsp_ibo_handle, 0, vk::IndexType::UINT32);

                    // Bind lightmap texture array at set 2 for per-pixel sampling
                    if let Some(lm_ds) = lightmap_ds {
                        ctx.device.cmd_bind_descriptor_sets(
                            cmd, vk::PipelineBindPoint::GRAPHICS,
                            world_layout, 2, &[lm_ds], &[],
                        );
                    }

                    // VXGI: bind the irradiance volume at set 4 and push grid params into the
                    // (static) dyn-block slots at offset 100, so world.frag can add real-time GI
                    // to the dimmed baked lightmap when r_vxgi_bake < 1.
                    if let Some(irr_set) = self.vxgi_world_irr_set {
                        ctx.device.cmd_bind_descriptor_sets(
                            cmd, vk::PipelineBindPoint::GRAPHICS, world_layout, 4, &[irr_set], &[],
                        );
                    }
                    if let Some((gmin, extent, giscale)) = self.frame_irr_params {
                        let params = [gmin[0], gmin[1], gmin[2], extent, giscale];
                        let pbytes = std::slice::from_raw_parts(params.as_ptr() as *const u8, 20);
                        ctx.device.cmd_push_constants(
                            cmd, world_layout, vk::ShaderStageFlags::FRAGMENT, 100, pbytes,
                        );
                    }
                    for &(first_index, index_count, ds, is_light) in &bsp_batches {
                        if let Some(ds) = ds {
                            ctx.device.cmd_bind_descriptor_sets(
                                cmd, vk::PipelineBindPoint::GRAPHICS,
                                world_layout, 1, &[ds], &[],
                            );
                        }
                        // Emit multiplier at offset 96 (reused u_DynC0 when not relighting):
                        // light textures glow (well past the bloom threshold -> HDR -> bloom),
                        // everything else stays 1.0.
                        let emit: f32 = if is_light { 6.0 } else { 1.0 };
                        let emit_bytes: &[u8] = std::slice::from_raw_parts(
                            &emit as *const f32 as *const u8, 4,
                        );
                        ctx.device.cmd_push_constants(
                            cmd, world_layout,
                            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                            96, emit_bytes,
                        );
                        ctx.device.cmd_draw_indexed(cmd, index_count, 1, first_index, 0, 0);
                    }

                    // === Brush Models (opaque) ===
                    // World pipeline + BSP VBO/IBO still bound, alpha=1.0 still set
                    for (bm_mvp, bm_dyn, bm_surfaces) in &brush_model_draws {
                        // Push brush model's per-entity MVP
                        let bm_mvp_bytes: &[u8] = std::slice::from_raw_parts(
                            bm_mvp.as_ptr() as *const u8, 64,
                        );
                        ctx.device.cmd_push_constants(
                            cmd, world_layout,
                            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                            0, bm_mvp_bytes,
                        );
                        // Dynamic relight block (9 floats) at offset 92..128:
                        // [use_dyn, c0,c1,c2,c3, min.x,min.y, invSize.x,invSize.y].
                        let bm_dyn_bytes: &[u8] = std::slice::from_raw_parts(
                            bm_dyn.as_ptr() as *const u8, 36,
                        );
                        ctx.device.cmd_push_constants(
                            cmd, world_layout,
                            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                            92, bm_dyn_bytes,
                        );
                        // A RESTING mover (use_dyn=0) takes the GI branch like the world, but the
                        // bm_dyn block just zeroed the GI grid params at offset 100..119 (they share
                        // those bytes with the mover's packed-corner relight). Restore the real grid
                        // params so a resting mover samples the irradiance volume correctly instead
                        // of grid_min=0/extent=0 (which makes uvw blow up). A MOVING mover keeps its
                        // packed colours here and uses the use_dyn=1 relight path, so skip it then.
                        if bm_dyn[0] < 0.5 {
                            if let Some((gmin, extent, giscale)) = self.frame_irr_params {
                                let params = [gmin[0], gmin[1], gmin[2], extent, giscale];
                                let pbytes = std::slice::from_raw_parts(params.as_ptr() as *const u8, 20);
                                ctx.device.cmd_push_constants(
                                    cmd, world_layout, vk::ShaderStageFlags::FRAGMENT, 100, pbytes,
                                );
                            }
                        }

                        for &(first_index, index_count, ds) in bm_surfaces {
                            if let Some(ds) = ds {
                                ctx.device.cmd_bind_descriptor_sets(
                                    cmd, vk::PipelineBindPoint::GRAPHICS,
                                    world_layout, 1, &[ds], &[],
                                );
                            }
                            ctx.device.cmd_draw_indexed(cmd, index_count, 1, first_index, 0, 0);
                        }
                    }

                    // Moving movers clobbered the GI grid params at offset 100..119 with their packed
                    // relight colours, and those colours CHANGE as the platform travels. Any later
                    // GI-sampling pass on the world layout (alpha surfaces, sprites) would read a
                    // moving platform's colours as grid coordinates → the sampled voxel shifts every
                    // frame → surfaces flicker in sync with the platform. Restore the real params.
                    if let Some((gmin, extent, giscale)) = self.frame_irr_params {
                        let params = [gmin[0], gmin[1], gmin[2], extent, giscale];
                        let pbytes = std::slice::from_raw_parts(params.as_ptr() as *const u8, 20);
                        ctx.device.cmd_push_constants(
                            cmd, world_layout, vk::ShaderStageFlags::FRAGMENT, 100, pbytes,
                        );
                    }
                    // Movers also overwrote the reflection-clip slot (120..127, their footprint
                    // inv-size) — re-zero it so later world-shader passes can't accidentally clip.
                    let clip_off2: [f32; 2] = [0.0, 0.0];
                    ctx.device.cmd_push_constants(
                        cmd, world_layout,
                        vk::ShaderStageFlags::FRAGMENT,
                        120, std::slice::from_raw_parts(clip_off2.as_ptr() as *const u8, 8),
                    );
                }

                // === Detail Texture Overlay (multiplicative second pass) ===
                if has_detail_overlay {
                    let (detail_pipeline, detail_layout) = detail_pipeline_data.unwrap();
                    let detail_ds = detail_descriptor_set.unwrap();

                    ctx.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, detail_pipeline);

                    // Push MVP (same as world) + alpha=1.0
                    let mvp_bytes: &[u8] = std::slice::from_raw_parts(
                        mvp.as_ptr() as *const u8, 64,
                    );
                    ctx.device.cmd_push_constants(
                        cmd, detail_layout,
                        vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                        0, mvp_bytes,
                    );
                    let opaque_alpha = 1.0_f32;
                    let alpha_bytes: &[u8] = std::slice::from_raw_parts(
                        &opaque_alpha as *const f32 as *const u8, 4,
                    );
                    ctx.device.cmd_push_constants(
                        cmd, detail_layout,
                        vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                        64, alpha_bytes,
                    );
                    // The detail overlay re-draws the world with the world shader; force
                    // u_UseDynLight=0 (offset 92) so a mover's relight flag can't leak in, and
                    // the emit multiplier (offset 96) to 1.0 so a light batch's emit can't
                    // leak in and multiply the detail pass.
                    let zero_emit: [f32; 2] = [0.0, 1.0];
                    let zero_bytes: &[u8] = std::slice::from_raw_parts(
                        zero_emit.as_ptr() as *const u8, 8,
                    );
                    ctx.device.cmd_push_constants(
                        cmd, detail_layout,
                        vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                        92, zero_bytes,
                    );

                    // Bind detail texture at set 1
                    ctx.device.cmd_bind_descriptor_sets(
                        cmd, vk::PipelineBindPoint::GRAPHICS,
                        detail_layout, 1, &[detail_ds], &[],
                    );

                    // Re-bind BSP geometry (pipeline change may invalidate state)
                    let bsp_vbo_handle = bsp_vbo.unwrap();
                    let bsp_ibo_handle = bsp_ibo.unwrap();
                    ctx.device.cmd_bind_vertex_buffers(cmd, 0, &[bsp_vbo_handle], &[0]);
                    ctx.device.cmd_bind_index_buffer(cmd, bsp_ibo_handle, 0, vk::IndexType::UINT32);

                    // Redraw all world surfaces with detail texture (multiplicative blend)
                    for &(first_index, index_count, _, _) in &bsp_batches {
                        ctx.device.cmd_draw_indexed(cmd, index_count, 1, first_index, 0, 0);
                    }
                }

                // === Water / Turb Surfaces ===
                if has_turb {
                    let (water_pipeline, water_layout) = water_pipeline_data.unwrap();

                    ctx.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, water_pipeline);

                    // Re-bind BSP geometry (pipeline change invalidates state)
                    let bsp_vbo_handle = bsp_vbo.unwrap();
                    let bsp_ibo_handle = bsp_ibo.unwrap();
                    ctx.device.cmd_bind_vertex_buffers(cmd, 0, &[bsp_vbo_handle], &[0]);
                    ctx.device.cmd_bind_index_buffer(cmd, bsp_ibo_handle, 0, vk::IndexType::UINT32);

                    // Water push constants: mat4 MVP (64) + float alpha (4) + float time (4) + float scroll (4) = 76 bytes
                    // Push MVP first (64 bytes)
                    let mvp_bytes: &[u8] = std::slice::from_raw_parts(
                        mvp.as_ptr() as *const u8, 64,
                    );
                    ctx.device.cmd_push_constants(
                        cmd, water_layout,
                        vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                        0, mvp_bytes,
                    );

                    // Push time at offset 68 (4 bytes)
                    let time_bytes: &[u8] = std::slice::from_raw_parts(
                        &water_time as *const f32 as *const u8, 4,
                    );
                    ctx.device.cmd_push_constants(
                        cmd, water_layout,
                        vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                        68, time_bytes,
                    );

                    // Push scroll=0 at offset 72 (4 bytes)
                    let scroll: f32 = 0.0;
                    let scroll_bytes: &[u8] = std::slice::from_raw_parts(
                        &scroll as *const f32 as *const u8, 4,
                    );
                    ctx.device.cmd_push_constants(
                        cmd, water_layout,
                        vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                        72, scroll_bytes,
                    );

                    // Bind the lightmap array (set 2) so liquids are lit like the world,
                    // and push the same lightmap-processing params the world pass uses
                    // (offsets 76..96) so the math matches exactly.
                    if let Some(lm_ds) = lightmap_ds {
                        ctx.device.cmd_bind_descriptor_sets(
                            cmd, vk::PipelineBindPoint::GRAPHICS,
                            water_layout, 2, &[lm_ds], &[],
                        );
                    }
                    let w_overbright = crate::vk_rmain::rcvars().r_overbrightbits.value.max(1.0);
                    let w_max_d3 = crate::vk_rmain::rcvars().r_d3_maxlights.fresh_value() as i32;
                    let w_lm_scale = if w_max_d3 > 0 {
                        crate::vk_rmain::rcvars().r_d3_ambient.fresh_value().clamp(0.0, 0.99)
                    } else {
                        1.0_f32
                    };
                    let water_lm_params: [f32; 5] = [
                        w_overbright,
                        crate::vk_rmain::rcvars().r_lightmap_gamma.value,
                        crate::vk_rmain::rcvars().r_lightmap_contrast.value,
                        crate::vk_rmain::rcvars().r_shadowlift.value,
                        w_lm_scale,
                    ];
                    let water_lm_bytes: &[u8] = std::slice::from_raw_parts(
                        water_lm_params.as_ptr() as *const u8, 20,
                    );
                    ctx.device.cmd_push_constants(
                        cmd, water_layout,
                        vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                        76, water_lm_bytes,
                    );

                    // Planar reflection: bind the mirrored render at set 3 and push the camera
                    // position (Fresnel). Per-surface strength (offset 108) gates which water
                    // actually reflects — only surfaces on the chosen plane, and only when the
                    // mirrored pass actually ran this frame.
                    let refl_ran = refl_data.is_some();
                    if refl_ran {
                        if let Some(refl_set) = self.refl_desc_set {
                            ctx.device.cmd_bind_descriptor_sets(
                                cmd, vk::PipelineBindPoint::GRAPHICS,
                                water_layout, 3, &[refl_set], &[],
                            );
                        }
                        let cam = self.frame_uniforms.view_origin;
                        ctx.device.cmd_push_constants(
                            cmd, water_layout,
                            vk::ShaderStageFlags::FRAGMENT,
                            112, std::slice::from_raw_parts(cam.as_ptr() as *const u8, 12),
                        );
                    }
                    let refl_strength =
                        crate::vk_rmain::rcvars().r_water_reflect_strength.fresh_value().max(0.0);

                    for &(first_index, index_count, alpha, flat_light, ds, on_plane) in &turb_surface_draws {
                        // TRANS-flagged water is drawn later (translucent-water block after
                        // sky/entities) with real blending; this pass keeps only opaque turb.
                        if alpha < 1.0 {
                            continue;
                        }
                        // Push per-surface alpha at offset 64
                        let alpha_bytes: &[u8] = std::slice::from_raw_parts(
                            &alpha as *const f32 as *const u8, 4,
                        );
                        ctx.device.cmd_push_constants(
                            cmd, water_layout,
                            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                            64, alpha_bytes,
                        );

                        // Push the body flat light (offset 96..108). [-1,-1,-1] = none,
                        // so the shader falls back to its per-pixel lightmap.
                        let fl_bytes: &[u8] = std::slice::from_raw_parts(
                            flat_light.as_ptr() as *const u8, 12,
                        );
                        ctx.device.cmd_push_constants(
                            cmd, water_layout,
                            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                            96, fl_bytes,
                        );

                        // Reflection strength at offset 108: 0 disables sampling entirely.
                        let strength: f32 = if refl_ran && on_plane { refl_strength } else { 0.0 };
                        ctx.device.cmd_push_constants(
                            cmd, water_layout,
                            vk::ShaderStageFlags::FRAGMENT,
                            108, std::slice::from_raw_parts(&strength as *const f32 as *const u8, 4),
                        );

                        if let Some(ds) = ds {
                            ctx.device.cmd_bind_descriptor_sets(
                                cmd, vk::PipelineBindPoint::GRAPHICS,
                                water_layout, 1, &[ds], &[],
                            );
                        }
                        ctx.device.cmd_draw_indexed(cmd, index_count, 1, first_index, 0, 0);
                    }
                }

                // === Alias Models ===
                if let Some((a_pipeline, a_layout)) = alias_pipeline_data {
                    if !alias_draw_entries.is_empty() {
                        ctx.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, a_pipeline);

                        // RF_DEPTHHACK (view weapon): draw those instances in a compressed depth
                        // range (max_depth 0.3) via the dynamic viewport — the original's
                        // glDepthRange(0, 0.3) — so the gun sits at the true view origin without
                        // clipping the near plane or poking into walls.
                        let mut vp_depth_hacked = false;
                        let set_alias_viewport = |hack: bool| {
                            let vp = vk::Viewport {
                                x: 0.0,
                                y: scene_extent.height as f32,
                                width: scene_extent.width as f32,
                                height: -(scene_extent.height as f32),
                                min_depth: 0.0,
                                max_depth: if hack { 0.3 } else { 1.0 },
                            };
                            // SAFETY: recording command buffer, viewport is a dynamic state.
                            unsafe { ctx.device.cmd_set_viewport(cmd, 0, &[vp]) };
                        };

                        for entry in &alias_draw_entries {
                            // Bind skin texture (same for all instances of this model)
                            if let Some(ds) = entry.skin_ds {
                                ctx.device.cmd_bind_descriptor_sets(
                                    cmd, vk::PipelineBindPoint::GRAPHICS,
                                    a_layout, 1, &[ds], &[],
                                );
                            }

                            for (pc, frame_offset, depth_hack) in &entry.instances {
                                if *depth_hack != vp_depth_hacked {
                                    set_alias_viewport(*depth_hack);
                                    vp_depth_hacked = *depth_hack;
                                }
                                // Push alias constants (128 bytes)
                                let pc_bytes: &[u8] = std::slice::from_raw_parts(
                                    pc as *const AliasPushConstants as *const u8,
                                    std::mem::size_of::<AliasPushConstants>(),
                                );
                                ctx.device.cmd_push_constants(
                                    cmd, a_layout,
                                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                                    0, pc_bytes,
                                );

                                // Bind VBO at frame offset
                                ctx.device.cmd_bind_vertex_buffers(
                                    cmd, 0, &[entry.vbo], &[*frame_offset],
                                );
                                ctx.device.cmd_bind_index_buffer(
                                    cmd, entry.ibo, 0, vk::IndexType::UINT32,
                                );

                                ctx.device.cmd_draw_indexed(
                                    cmd, entry.index_count, 1, 0, 0, 0,
                                );
                            }
                        }
                        // Restore the full depth range for every subsequent pass.
                        if vp_depth_hacked {
                            set_alias_viewport(false);
                        }
                    }
                }

                // TEMP DIAGNOSTIC: how many alias models and shadow instances are submitted.
                {
                    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
                    if N.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 30 {
                        let models = alias_draw_entries.len();
                        let normal: usize = alias_draw_entries.iter().map(|e| e.instances.len()).sum();
                        let shadows: usize = alias_draw_entries.iter().map(|e| e.shadow_instances.len()).sum();
                        let gz = alias_draw_entries.iter().flat_map(|e| e.shadow_instances.first())
                            .map(|(pc, _)| pc.mvp[14]).next().unwrap_or(0.0);
                        eprintln!("[shadow] cast_on={} alias_models={} normal_inst={} shadow_inst={} pipeline={} sample_mvp14={:.2}",
                            cast_shadows, models, normal, shadows, alias_shadow_pipeline_data.is_some(), gz);
                    }
                }

                // === Alias Blob Shadows ===
                // Same posed geometry flattened onto each entity's ground plane, drawn black
                // and translucent on the alpha-blend pipeline. Lands on whatever surface the
                // entity stands on (including a moving lift deck). Drawn after the models so
                // it blends over the floor/deck below them.
                if let Some((s_pipeline, s_layout)) = alias_shadow_pipeline_data {
                    if alias_draw_entries.iter().any(|e| !e.shadow_instances.is_empty()) {
                        ctx.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, s_pipeline);
                        for entry in &alias_draw_entries {
                            if entry.shadow_instances.is_empty() { continue; }
                            // Bind the skin — alias.frag samples its alpha (rgb is * black).
                            if let Some(ds) = entry.skin_ds {
                                ctx.device.cmd_bind_descriptor_sets(
                                    cmd, vk::PipelineBindPoint::GRAPHICS,
                                    s_layout, 1, &[ds], &[],
                                );
                            }
                            for (pc, frame_offset) in &entry.shadow_instances {
                                let pc_bytes: &[u8] = std::slice::from_raw_parts(
                                    pc as *const AliasPushConstants as *const u8,
                                    std::mem::size_of::<AliasPushConstants>(),
                                );
                                ctx.device.cmd_push_constants(
                                    cmd, s_layout,
                                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                                    0, pc_bytes,
                                );
                                ctx.device.cmd_bind_vertex_buffers(cmd, 0, &[entry.vbo], &[*frame_offset]);
                                ctx.device.cmd_bind_index_buffer(cmd, entry.ibo, 0, vk::IndexType::UINT32);
                                ctx.device.cmd_draw_indexed(cmd, entry.index_count, 1, 0, 0, 0);
                            }
                        }
                    }
                }

                // === Sprites ===
                if has_sprites {
                    let (sp_pipeline, sp_layout) = sprite_pipeline_data.unwrap();
                    ctx.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, sp_pipeline);

                    if let (Some(s_vbo), Some(s_ibo)) = (self.sprite_vbo.vk_buffer(), self.sprite_ibo.vk_buffer()) {
                        ctx.device.cmd_bind_vertex_buffers(cmd, 0, &[s_vbo], &[0]);
                        ctx.device.cmd_bind_index_buffer(cmd, s_ibo, 0, vk::IndexType::UINT32);

                        // Push world MVP (sprites are in world space)
                        let mvp_bytes: &[u8] = std::slice::from_raw_parts(
                            mvp.as_ptr() as *const u8, 64,
                        );
                        ctx.device.cmd_push_constants(
                            cmd, sp_layout,
                            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                            0, mvp_bytes,
                        );
                        // Sprites use the world shader; force u_UseDynLight=0 (offset 92) and
                        // emit=1.0 (offset 96) so a mover's relight flag or a light batch's
                        // emit can't bleed in.
                        let zero_emit: [f32; 2] = [0.0, 1.0];
                        let zero_bytes: &[u8] = std::slice::from_raw_parts(
                            zero_emit.as_ptr() as *const u8, 8,
                        );
                        ctx.device.cmd_push_constants(
                            cmd, sp_layout,
                            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                            92, zero_bytes,
                        );
                        // Zero the reflection-clip slot (120..127): the water pass just wrote its
                        // camera Z there (world shader reads it as u_DynInvSize.x), which would
                        // make the clip gate discard sprite fragments below z≈0.
                        let sp_clip_off: [f32; 2] = [0.0, 0.0];
                        ctx.device.cmd_push_constants(
                            cmd, sp_layout, vk::ShaderStageFlags::FRAGMENT,
                            120, std::slice::from_raw_parts(sp_clip_off.as_ptr() as *const u8, 8),
                        );

                        let mut idx_offset = 0u32;
                        for (alpha, ds) in &sprite_draw_info {
                            // Push per-sprite alpha at offset 64
                            let alpha_bytes: &[u8] = std::slice::from_raw_parts(
                                alpha as *const f32 as *const u8, 4,
                            );
                            ctx.device.cmd_push_constants(
                                cmd, sp_layout,
                                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                                64, alpha_bytes,
                            );

                            if let Some(ds) = ds {
                                ctx.device.cmd_bind_descriptor_sets(
                                    cmd, vk::PipelineBindPoint::GRAPHICS,
                                    sp_layout, 1, &[*ds], &[],
                                );
                            }
                            ctx.device.cmd_draw_indexed(cmd, 6, 1, idx_offset, 0, 0);
                            idx_offset += 6;
                        }
                    }
                }

                // === Sky ===
                // Sky is drawn with the xyww trick (sky.vert.glsl sets gl_Position = pos.xyww)
                // which forces all sky fragments to depth = 1.0. With LESS_OR_EQUAL depth test
                // and no depth write, sky renders behind all BSP geometry but in front of the
                // cleared depth buffer (also 1.0, so EQUAL passes).
                if has_sky {
                    let (sky_pipeline, sky_layout) = sky_pipeline_data.unwrap();

                    ctx.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, sky_pipeline);

                    // Push sky MVP
                    let sky_mvp_bytes: &[u8] = std::slice::from_raw_parts(
                        sky_mvp_flat.as_ptr() as *const u8, 64,
                    );
                    ctx.device.cmd_push_constants(
                        cmd, sky_layout,
                        vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                        0, sky_mvp_bytes,
                    );
                    // Alpha = 1.0 (opaque sky)
                    let sky_alpha = 1.0_f32;
                    ctx.device.cmd_push_constants(
                        cmd, sky_layout,
                        vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                        64, std::slice::from_raw_parts(&sky_alpha as *const f32 as *const u8, 4),
                    );
                    // Overbright = 1.0 (no overbright for sky)
                    let sky_ob = 1.0_f32;
                    ctx.device.cmd_push_constants(
                        cmd, sky_layout,
                        vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                        68, std::slice::from_raw_parts(&sky_ob as *const f32 as *const u8, 4),
                    );

                    if let Some(s_vbo) = self.sky_vbo.vk_buffer() {
                        ctx.device.cmd_bind_vertex_buffers(cmd, 0, &[s_vbo], &[0]);

                        if let Some(s_ibo) = self.sky_ibo.vk_buffer() {
                            ctx.device.cmd_bind_index_buffer(cmd, s_ibo, 0, vk::IndexType::UINT32);

                            for &(_, first_idx, count, ds) in &sky_draws {
                                if let Some(ds) = ds {
                                    ctx.device.cmd_bind_descriptor_sets(
                                        cmd, vk::PipelineBindPoint::GRAPHICS,
                                        sky_layout, 1, &[ds], &[],
                                    );
                                }
                                ctx.device.cmd_draw_indexed(cmd, count, 1, first_idx, 0, 0);
                            }
                        }
                    }
                }

                // === Dynamic Lights ===
                if has_dlights {
                    let (dl_pipeline, dl_layout) = dlight_pipeline_data.unwrap();

                    if let (Some(d_vbo), Some(d_ibo)) = (self.dlight_vbo.vk_buffer(), self.dlight_ibo.vk_buffer()) {
                        ctx.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, dl_pipeline);
                        ctx.device.cmd_bind_vertex_buffers(cmd, 0, &[d_vbo], &[0]);
                        ctx.device.cmd_bind_index_buffer(cmd, d_ibo, 0, vk::IndexType::UINT32);

                        let mut idx_offset = 0u32;
                        for pc in &dlight_pcs {
                            // Push dlight constants (96 bytes)
                            let pc_bytes: &[u8] = std::slice::from_raw_parts(
                                pc as *const DlightPushConstants as *const u8,
                                std::mem::size_of::<DlightPushConstants>(),
                            );
                            ctx.device.cmd_push_constants(
                                cmd, dl_layout,
                                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                                0, pc_bytes,
                            );
                            ctx.device.cmd_draw_indexed(cmd, 48, 1, idx_offset, 0, 0);
                            idx_offset += 48;
                        }
                    }
                }

                // === Per-pixel additive lit pass: surface-light dark-fill (r_surf_emit) and/or
                // the abandoned D3 flat-base mode (r_d3_maxlights). Runs whenever lights were
                // selected this frame. ===
                if has_bsp && !self.d3_lights.is_empty() {
                    let bsp_vbo_handle = bsp_vbo.unwrap();
                    let bsp_ibo_handle = bsp_ibo.unwrap();
                    let vo = self.frame_uniforms.view_origin;
                    // cmd is valid; BSP handles and d3_lights are valid for this frame.
                    {
                        self.flush_doom3_lights_with_ctx(
                            ctx, cmd,
                            bsp_vbo_handle, bsp_ibo_handle,
                            &bsp_batches, mvp, vo,
                        );
                    }
                }

                // === Translucent Water (blended) ===
                // TRANS-flagged water draws HERE — after world, entities and sky, with real
                // alpha blending (the original Q2 alpha-chain order) — so it reads as
                // transparent: you see the pool bottom through it, with the reflection
                // Fresnel-blended on top. The earlier Opaque water pass now only draws
                // untagged liquids (lava, deliberately solid water).
                if has_turb && turb_surface_draws.iter().any(|&(_, _, a, ..)| a < 1.0) {
                    if let Some((tw_pipeline, tw_layout)) = water_ab_pipeline_data {
                        ctx.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, tw_pipeline);
                        let bsp_vbo_handle = bsp_vbo.unwrap();
                        let bsp_ibo_handle = bsp_ibo.unwrap();
                        ctx.device.cmd_bind_vertex_buffers(cmd, 0, &[bsp_vbo_handle], &[0]);
                        ctx.device.cmd_bind_index_buffer(cmd, bsp_ibo_handle, 0, vk::IndexType::UINT32);

                        let mvp_bytes: &[u8] = std::slice::from_raw_parts(mvp.as_ptr() as *const u8, 64);
                        ctx.device.cmd_push_constants(
                            cmd, tw_layout,
                            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                            0, mvp_bytes,
                        );
                        // time(68), scroll(72), then the lm params (76..96) — water shader layout.
                        let head: [f32; 2] = [water_time, 0.0];
                        ctx.device.cmd_push_constants(
                            cmd, tw_layout,
                            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                            68, std::slice::from_raw_parts(head.as_ptr() as *const u8, 8),
                        );
                        let tw_overbright = crate::vk_rmain::rcvars().r_overbrightbits.value.max(1.0);
                        let tw_lm_params: [f32; 5] = [
                            tw_overbright,
                            crate::vk_rmain::rcvars().r_lightmap_gamma.value,
                            crate::vk_rmain::rcvars().r_lightmap_contrast.value,
                            crate::vk_rmain::rcvars().r_shadowlift.value,
                            1.0,
                        ];
                        ctx.device.cmd_push_constants(
                            cmd, tw_layout,
                            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                            76, std::slice::from_raw_parts(tw_lm_params.as_ptr() as *const u8, 20),
                        );
                        if let Some(lm_ds) = lightmap_ds {
                            ctx.device.cmd_bind_descriptor_sets(
                                cmd, vk::PipelineBindPoint::GRAPHICS, tw_layout, 2, &[lm_ds], &[],
                            );
                        }
                        let refl_ran = refl_data.is_some();
                        if refl_ran {
                            if let Some(refl_set) = self.refl_desc_set {
                                ctx.device.cmd_bind_descriptor_sets(
                                    cmd, vk::PipelineBindPoint::GRAPHICS, tw_layout, 3, &[refl_set], &[],
                                );
                            }
                            let cam = self.frame_uniforms.view_origin;
                            ctx.device.cmd_push_constants(
                                cmd, tw_layout,
                                vk::ShaderStageFlags::FRAGMENT,
                                112, std::slice::from_raw_parts(cam.as_ptr() as *const u8, 12),
                            );
                        }
                        let refl_strength =
                            crate::vk_rmain::rcvars().r_water_reflect_strength.fresh_value().max(0.0);

                        for &(first_index, index_count, alpha, flat_light, ds, on_plane) in &turb_surface_draws {
                            if alpha >= 1.0 {
                                continue; // opaque turb already drawn
                            }
                            // Slightly more transparent than the raw TRANS flag (user pref).
                            let a = alpha * 0.85;
                            ctx.device.cmd_push_constants(
                                cmd, tw_layout,
                                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                                64, std::slice::from_raw_parts(&a as *const f32 as *const u8, 4),
                            );
                            let fl_bytes: &[u8] = std::slice::from_raw_parts(flat_light.as_ptr() as *const u8, 12);
                            ctx.device.cmd_push_constants(
                                cmd, tw_layout,
                                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                                96, fl_bytes,
                            );
                            let strength: f32 = if refl_ran && on_plane { refl_strength } else { 0.0 };
                            ctx.device.cmd_push_constants(
                                cmd, tw_layout,
                                vk::ShaderStageFlags::FRAGMENT,
                                108, std::slice::from_raw_parts(&strength as *const f32 as *const u8, 4),
                            );
                            if let Some(ds) = ds {
                                ctx.device.cmd_bind_descriptor_sets(
                                    cmd, vk::PipelineBindPoint::GRAPHICS, tw_layout, 1, &[ds], &[],
                                );
                            }
                            ctx.device.cmd_draw_indexed(cmd, index_count, 1, first_index, 0, 0);
                        }
                    }
                }

                // === Alpha Surfaces ===
                if let Some((ab_pipeline, ab_layout)) = alpha_blend_pipeline_data {
                    if !alpha_surface_draws.is_empty() && has_bsp {
                        let bsp_vbo_handle = bsp_vbo.unwrap();
                        let bsp_ibo_handle = bsp_ibo.unwrap();

                        ctx.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, ab_pipeline);

                        // Push world MVP
                        let mvp_bytes: &[u8] = std::slice::from_raw_parts(
                            mvp.as_ptr() as *const u8, 64,
                        );
                        ctx.device.cmd_push_constants(
                            cmd, ab_layout,
                            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                            0, mvp_bytes,
                        );

                        ctx.device.cmd_bind_vertex_buffers(cmd, 0, &[bsp_vbo_handle], &[0]);
                        ctx.device.cmd_bind_index_buffer(cmd, bsp_ibo_handle, 0, vk::IndexType::UINT32);

                        // Push overbright scale at offset 68 (same value for all alpha surfaces)
                        let overbright = crate::vk_rmain::rcvars().r_overbrightbits.value.max(1.0);
                        let overbright_bytes: &[u8] = std::slice::from_raw_parts(
                            &overbright as *const f32 as *const u8, 4,
                        );
                        ctx.device.cmd_push_constants(
                            cmd, ab_layout,
                            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                            68, overbright_bytes,
                        );
                        // Alpha surfaces use the world shader, so push its FULL parameter block.
                        // The water pass ran just before this one with a DIFFERENT push layout
                        // (its overbright sits where the world shader reads gamma, etc.), so any
                        // slot not re-pushed here reads water values one slot off — that lit
                        // translucent surfaces with garbage (washed-out glass). Tail 72..96:
                        // scroll, gamma, contrast, lift, lm_scale (same as opaque world so alpha
                        // surfaces get the same VXGI treatment), use_dyn=0.
                        let ab_max_d3 = crate::vk_rmain::rcvars().r_d3_maxlights.fresh_value() as i32;
                        let ab_lm_scale = if ab_max_d3 > 0 {
                            crate::vk_rmain::rcvars().r_d3_ambient.fresh_value().clamp(0.0, 0.99)
                        } else if crate::vk_rmain::rcvars().r_vxgi.fresh_value() != 0.0 {
                            crate::vk_rmain::rcvars().r_vxgi_bake.fresh_value().clamp(0.0, 1.0)
                        } else {
                            1.0_f32
                        };
                        let ab_tail: [f32; 7] = [
                            0.0, // u_UvScroll
                            crate::vk_rmain::rcvars().r_lightmap_gamma.value,
                            crate::vk_rmain::rcvars().r_lightmap_contrast.value,
                            crate::vk_rmain::rcvars().r_shadowlift.value,
                            ab_lm_scale,
                            0.0, // u_UseDynLight
                            1.0, // emit
                        ];
                        ctx.device.cmd_push_constants(
                            cmd, ab_layout,
                            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                            72, std::slice::from_raw_parts(ab_tail.as_ptr() as *const u8, 28),
                        );
                        // The water pass also clobbered the GI grid slots (100..119, its flat
                        // light/reflection params live there) and the reflection-clip slot
                        // (120..127, its camera Z — which is >0.5 on most maps and would make
                        // the clip gate DISCARD alpha fragments). Restore both.
                        if let Some((gmin, extent, giscale)) = self.frame_irr_params {
                            let params = [gmin[0], gmin[1], gmin[2], extent, giscale];
                            ctx.device.cmd_push_constants(
                                cmd, ab_layout, vk::ShaderStageFlags::FRAGMENT,
                                100, std::slice::from_raw_parts(params.as_ptr() as *const u8, 20),
                            );
                        }
                        let ab_clip_off: [f32; 2] = [0.0, 0.0];
                        ctx.device.cmd_push_constants(
                            cmd, ab_layout, vk::ShaderStageFlags::FRAGMENT,
                            120, std::slice::from_raw_parts(ab_clip_off.as_ptr() as *const u8, 8),
                        );

                        // Bind lightmap texture array at set 2 for alpha surfaces
                        if let Some(lm_ds) = lightmap_ds {
                            ctx.device.cmd_bind_descriptor_sets(
                                cmd, vk::PipelineBindPoint::GRAPHICS,
                                ab_layout, 2, &[lm_ds], &[],
                            );
                        }

                        for &(first_index, index_count, alpha, ds) in &alpha_surface_draws {
                            // Push per-surface alpha at offset 64
                            let alpha_bytes: &[u8] = std::slice::from_raw_parts(
                                &alpha as *const f32 as *const u8, 4,
                            );
                            ctx.device.cmd_push_constants(
                                cmd, ab_layout,
                                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                                64, alpha_bytes,
                            );

                            if let Some(ds) = ds {
                                ctx.device.cmd_bind_descriptor_sets(
                                    cmd, vk::PipelineBindPoint::GRAPHICS,
                                    ab_layout, 1, &[ds], &[],
                                );
                            }
                            ctx.device.cmd_draw_indexed(cmd, index_count, 1, first_index, 0, 0);
                        }
                    }
                }

                // === Particles ===
                if has_particles {
                    let (p_pipeline, p_layout) = particle_pipeline_data.unwrap();
                    let quad_vbo = particle_quad_vbo.unwrap();
                    let inst_vbo = particle_instance_vbo.unwrap();

                    ctx.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, p_pipeline);

                    // Push particle uniforms (112 bytes, vertex stage)
                    let pc_bytes: &[u8] = std::slice::from_raw_parts(
                        &particle_pc as *const ParticlePushConstants as *const u8,
                        std::mem::size_of::<ParticlePushConstants>(),
                    );
                    ctx.device.cmd_push_constants(
                        cmd, p_layout, vk::ShaderStageFlags::VERTEX, 0, pc_bytes,
                    );

                    ctx.device.cmd_bind_vertex_buffers(cmd, 0, &[quad_vbo, inst_vbo], &[0, 0]);

                    if let Some(ds) = particle_ds {
                        ctx.device.cmd_bind_descriptor_sets(
                            cmd, vk::PipelineBindPoint::GRAPHICS,
                            p_layout, 1, &[ds], &[],
                        );
                    }

                    ctx.device.cmd_draw(cmd, 4, particle_count as u32, 0, 0);
                }

                ctx.device.cmd_end_rendering(cmd);
            }
        });

        // Projective dynamic shadows: cast all dynamic casters into the directional shadow
        // map and darken the scene where occluded. Runs on the same command buffer now that
        // the scene's dynamic-rendering pass has ended (scene colour in COLOR_ATTACHMENT,
        // depth in DEPTH_ATTACHMENT), before the composite reads the scene colour.
        if let Some(cmd) = self.current_command_buffer {
            let vieworg = self.frame_vieworg;
            self.render_projective_shadows(cmd, vieworg, vp_flat);
        }

        // VXGI Phase 4: diffuse cone-traced GI. Runs after the projective shadow pass so the
        // scene depth is already in SHADER_READ with a sampleable depth-aspect view.
        if crate::vk_rmain::rcvars().r_vxgi_gi.fresh_value() != 0.0
            && crate::vk_rmain::rcvars().r_vxgi_debug.fresh_value() == 0.0 {
            if let Some(cmd) = self.current_command_buffer {
                let vieworg = self.frame_vieworg;
                self.render_vxgi_gi(cmd, vieworg, vp_flat);
            }
        }

        // Water ripple shimmer: animated caustic light thrown by the active water plane onto
        // nearby walls/floors. Same depth-view dependency as the GI pass above.
        if let Some(pz) = self.frame_refl_plane {
            if crate::vk_rmain::rcvars().r_water_shimmer.fresh_value() > 0.0 {
                if let Some(cmd) = self.current_command_buffer {
                    self.render_water_shimmer(cmd, vp_flat, pz);
                }
            }
        }

        // VXGI Phase 1 debug: raymarch the voxel grid over the scene (r_vxgi_debug). Use
        // fresh_value() — .value is a registration-time snapshot, so a runtime `r_vxgi_debug 1`
        // would otherwise never be seen.
        if crate::vk_rmain::rcvars().r_vxgi_debug.fresh_value() != 0.0 {
            if let Some(cmd) = self.current_command_buffer {
                let vieworg = self.frame_vieworg;
                self.render_vxgi_debug(cmd, vieworg, vp_flat);
            }
        }
    }

    /// Raymarch the static voxel grid and draw it over the scene colour (debug view).
    fn render_vxgi_debug(&mut self, cmd: vk::CommandBuffer, view_origin: [f32; 3], view_proj_flat: [f32; 16]) {
        use super::shadow_project as sp;
        // Grid params (skip if not voxelized yet).
        let grid = match super::vxgi::with_voxel_grid(|vg| {
            (vg.view, vg.sampler, vg.rad_view, vg.rad_sampler, vg.grid_min, vg.extent)
        }) {
            Some(g) => g,
            None => return,
        };
        let (a_view, a_samp, r_view, r_samp, grid_min, extent) = grid;
        // Mode from the cvar: 1 = albedo/normal volume, 2 = radiance (emitters).
        let mode = crate::vk_rmain::rcvars().r_vxgi_debug.fresh_value();
        let (vox_view, vox_sampler) = if mode > 1.5 { (r_view, r_samp) } else { (a_view, a_samp) };

        let (pipe, layout) = match self.pipelines.as_ref().and_then(|pm| pm.vxgi_debug_pipeline()) {
            Some(p) => p,
            None => {
                if let Some(pm) = self.pipelines.as_mut() {
                    let _ = pm.create_vxgi_debug_pipeline();
                }
                match self.pipelines.as_ref().and_then(|pm| pm.vxgi_debug_pipeline()) {
                    Some(p) => p,
                    None => return,
                }
            }
        };
        let set_layout = match self.pipelines.as_ref().and_then(|pm| pm.vxgi_debug_set_layout()) {
            Some(l) => l,
            None => return,
        };

        // Scene colour view (render target).
        let scene_color_view = match self.post_processor.as_ref().and_then(|pp| pp.scene_fbo().color_view()) {
            Some(v) => v,
            None => return,
        };
        let (sw, sh) = (self.width, self.height);
        let inv_vp = sp::invert(&view_proj_flat);

        gpu_device::with_device(|ctx| unsafe {
            // Lazily create the descriptor pool + set, then (re)point it at the current grid.
            if self.vxgi_debug_set.is_none() {
                let sizes = [vk::DescriptorPoolSize::default()
                    .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).descriptor_count(1)];
                let pool = match ctx.device.create_descriptor_pool(
                    &vk::DescriptorPoolCreateInfo::default().max_sets(1).pool_sizes(&sizes), None) {
                    Ok(p) => p, Err(_) => return,
                };
                let layouts = [set_layout];
                let set = match ctx.device.allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&layouts)) {
                    Ok(s) => s[0], Err(_) => { ctx.device.destroy_descriptor_pool(pool, None); return; }
                };
                self.vxgi_debug_pool = Some(pool);
                self.vxgi_debug_set = Some(set);
            }
            let set = self.vxgi_debug_set.unwrap();
            let info = vk::DescriptorImageInfo::default().image_view(vox_view)
                .sampler(vox_sampler).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
            let write = vk::WriteDescriptorSet::default().dst_set(set).dst_binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(std::slice::from_ref(&info));
            ctx.device.update_descriptor_sets(std::slice::from_ref(&write), &[]);

            // Fullscreen pass into the scene colour (LOAD: keep the rendered scene; the shader
            // discards on a miss so only voxels overwrite it).
            let att = vk::RenderingAttachmentInfo::default()
                .image_view(scene_color_view)
                .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .load_op(vk::AttachmentLoadOp::LOAD)
                .store_op(vk::AttachmentStoreOp::STORE);
            let area = vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent: vk::Extent2D { width: sw, height: sh } };
            let atts = [att];
            let ri = vk::RenderingInfo::default().render_area(area).layer_count(1).color_attachments(&atts);
            ctx.device.cmd_begin_rendering(cmd, &ri);
            let vp = vk::Viewport { x: 0.0, y: 0.0, width: sw as f32, height: sh as f32, min_depth: 0.0, max_depth: 1.0 };
            ctx.device.cmd_set_viewport(cmd, 0, &[vp]);
            ctx.device.cmd_set_scissor(cmd, 0, &[area]);
            ctx.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipe);
            ctx.device.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, layout, 0, &[set], &[]);
            #[repr(C)]
            struct Push { inv_vp: [f32; 16], cam: [f32; 3], mode: f32, grid_min: [f32; 3], extent: f32 }
            let push = Push { inv_vp, cam: view_origin, mode, grid_min, extent };
            let bytes = std::slice::from_raw_parts(&push as *const Push as *const u8, 96);
            ctx.device.cmd_push_constants(cmd, layout, vk::ShaderStageFlags::FRAGMENT, 0, bytes);
            ctx.device.cmd_draw(cmd, 3, 1, 0, 0);
            ctx.device.cmd_end_rendering(cmd);
        });
    }

    /// Prepare the VXGI irradiance descriptor (set 4) + this frame's grid params for the world
    /// shader, so the world pass can bind the irradiance volume and push the grid params. Called
    /// before the 3D scene is recorded. No-op if there is no voxel grid.
    fn prepare_world_irradiance(&mut self) {
        self.frame_irr_params = None;
        let grid = super::vxgi::with_voxel_grid(|vg| (vg.rad_view, vg.rad_sampler, vg.grid_min, vg.extent));
        let (view, samp, gmin, extent) = match grid { Some(g) => g, None => return };
        let set_layout = match self.pipelines.as_ref().and_then(|pm| pm.lightmap_set_layout()) { Some(l) => l, None => return };
        gpu_device::with_device(|ctx| unsafe {
            // Make a FRESH set and write it exactly ONCE, only when the view changes (first build /
            // map reload). No every-frame re-write and no destroy of the in-flight old set — both
            // are update-/free-after-bind, which the driver ignores and leaves the descriptor
            // reading zero. The old pool is leaked (tiny, a few per session); correctness first.
            if self.vxgi_irr_view != Some(view) {
                let sizes = [vk::DescriptorPoolSize::default().ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).descriptor_count(1)];
                let pool = match ctx.device.create_descriptor_pool(
                    &vk::DescriptorPoolCreateInfo::default().max_sets(1).pool_sizes(&sizes)
                        .flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET), None) { Ok(p) => p, Err(_) => return };
                let layouts = [set_layout];
                let set = match ctx.device.allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&layouts)) {
                    Ok(s) => s[0], Err(_) => { ctx.device.destroy_descriptor_pool(pool, None); return; }
                };
                let info = vk::DescriptorImageInfo::default().image_view(view).sampler(samp)
                    .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
                let write = vk::WriteDescriptorSet::default().dst_set(set).dst_binding(0).dst_array_element(0)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(std::slice::from_ref(&info));
                ctx.device.update_descriptor_sets(std::slice::from_ref(&write), &[]);
                self.vxgi_world_irr_pool = Some(pool);
                self.vxgi_world_irr_set = Some(set);
                self.vxgi_irr_view = Some(view);
            }
        });
        let strength = crate::vk_rmain::rcvars().r_vxgi_strength.fresh_value().max(0.0);
        self.frame_irr_params = Some((gmin, extent, strength * super::vxgi::IRRADIANCE_HDR_SCALE));
    }

    /// Ensure the half-res planar-reflection target and its water-pipeline set-3 descriptor
    /// exist (recreated on scene resize / view change). Runs its own `with_device` scopes, so
    /// it must be called OUTSIDE the frame's main `with_device` closure (re-entrancy deadlock).
    fn prepare_water_reflection(&mut self, scene_w: u32, scene_h: u32) {
        let rw = (scene_w / 2).max(1);
        let rh = (scene_h / 2).max(1);
        let recreate = match self.refl_target.as_ref() {
            Some(t) => t.width() != rw || t.height() != rh,
            None => true,
        };
        if recreate {
            if let Some(mut old) = self.refl_target.take() {
                // Resize is driven by swapchain recreation, which has already waited for idle.
                old.destroy();
            }
            self.refl_target = Some(super::framebuffer::RenderTarget::new(rw, rh, true));
        }
        let (view, sampler) = match self
            .refl_target
            .as_ref()
            .and_then(|t| t.color_view().zip(t.sampler()))
        {
            Some(vs) => vs,
            None => return,
        };
        if self.refl_desc_view == Some(view) {
            return; // descriptor already points at the current view
        }
        let set_layout = match self.pipelines.as_ref().and_then(|pm| pm.lightmap_set_layout()) {
            Some(l) => l,
            None => return,
        };
        gpu_device::with_device(|ctx| unsafe {
            // SAFETY: device valid, main thread. Same write-once pattern as the VXGI irradiance
            // set: a FRESH set written before first use — updating an in-flight set is ignored
            // by the driver. The old pool is leaked (tiny, resize-rare); correctness first.
            let sizes = [vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)];
            let pool = match ctx.device.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(1)
                    .pool_sizes(&sizes)
                    .flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET),
                None,
            ) {
                Ok(p) => p,
                Err(_) => return,
            };
            let layouts = [set_layout];
            let set = match ctx.device.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(pool)
                    .set_layouts(&layouts),
            ) {
                Ok(s) => s[0],
                Err(_) => {
                    ctx.device.destroy_descriptor_pool(pool, None);
                    return;
                }
            };
            let info = vk::DescriptorImageInfo::default()
                .image_view(view)
                .sampler(sampler)
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
            let write = vk::WriteDescriptorSet::default()
                .dst_set(set)
                .dst_binding(0)
                .dst_array_element(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(std::slice::from_ref(&info));
            ctx.device.update_descriptor_sets(std::slice::from_ref(&write), &[]);
            self.refl_desc_pool = Some(pool);
            self.refl_desc_set = Some(set);
            self.refl_desc_view = Some(view);
        });
    }

    /// Diffuse cone-traced GI: gather bounced light from the radiance volume and add it to the
    /// scene. Reuses the projective-shadow pass's sampleable depth-aspect view (already in
    /// SHADER_READ), so it must run after that pass.
    fn render_vxgi_gi(&mut self, cmd: vk::CommandBuffer, view_origin: [f32; 3], view_proj_flat: [f32; 16]) {
        use super::shadow_project as sp;
        let grid = match super::vxgi::with_voxel_grid(|vg| {
            (vg.rad_view, vg.rad_sampler, vg.view, vg.sampler, vg.grid_min, vg.extent, vg.res)
        }) {
            Some(g) => g,
            None => return,
        };
        let (rad_view, rad_samp, alb_view, alb_samp, grid_min, extent, res) = grid;
        // Depth-aspect view + sampler from the projective shadow pass (valid this frame because
        // the player body is always a caster). If absent, skip GI this frame.
        let depth_view = match self.projective_shadow.depth_sample_view { Some(v) => v, None => return };
        let depth_samp = match self.projective_shadow.sampler { Some(s) => s, None => return };

        let (pipe, layout) = match self.pipelines.as_ref().and_then(|pm| pm.vxgi_gi_pipeline()) {
            Some(p) => p,
            None => {
                if let Some(pm) = self.pipelines.as_mut() { let _ = pm.create_vxgi_gi_pipeline(); }
                match self.pipelines.as_ref().and_then(|pm| pm.vxgi_gi_pipeline()) { Some(p) => p, None => return }
            }
        };
        let set_layout = match self.pipelines.as_ref().and_then(|pm| pm.vxgi_gi_set_layout()) { Some(l) => l, None => return };
        let scene_color_view = match self.post_processor.as_ref().and_then(|pp| pp.scene_fbo().color_view()) { Some(v) => v, None => return };
        let (sw, sh) = (self.width, self.height);
        let inv_vp = sp::invert(&view_proj_flat);
        let strength = crate::vk_rmain::rcvars().r_vxgi_strength.fresh_value().max(0.0);
        let voxel_size = extent / res as f32;

        // Build this frame's dynamic lights for the GI UBO (std140: int + 32×(vec4 pos_radius,
        // vec4 color) = 1040 bytes). radius scales with intensity (bounce reach).
        #[repr(C)]
        #[derive(Clone, Copy)]
        struct GiDLight { pos_radius: [f32; 4], color: [f32; 4] }
        #[repr(C)]
        struct GiDLightUbo { num: i32, _pad: [i32; 3], lights: [GiDLight; 32] }
        let mut ubo = GiDLightUbo { num: 0, _pad: [0; 3], lights: [GiDLight { pos_radius: [0.0; 4], color: [0.0; 4] }; 32] };
        let nd = (crate::vk_local::rfs().r_newrefdef.num_dlights).min(32) as usize;
        for i in 0..nd {
            // SAFETY: dlight index in bounds; r_newrefdef valid for the frame, main thread.
            let d = unsafe { crate::vk_local::rfs().r_newrefdef.dlight(i) };
            ubo.lights[i].pos_radius = [d.origin[0], d.origin[1], d.origin[2], (d.intensity * 0.6).max(32.0)];
            ubo.lights[i].color = [d.color[0], d.color[1], d.color[2], 0.0];
            ubo.num += 1;
        }
        let ubo_size = std::mem::size_of::<GiDLightUbo>() as vk::DeviceSize;

        gpu_device::with_device(|ctx| unsafe {
            if self.vxgi_gi_set.is_none() {
                let sizes = [
                    vk::DescriptorPoolSize::default().ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).descriptor_count(3),
                    vk::DescriptorPoolSize::default().ty(vk::DescriptorType::UNIFORM_BUFFER).descriptor_count(1),
                ];
                let pool = match ctx.device.create_descriptor_pool(
                    &vk::DescriptorPoolCreateInfo::default().max_sets(1).pool_sizes(&sizes), None) { Ok(p) => p, Err(_) => return };
                let layouts = [set_layout];
                let set = match ctx.device.allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&layouts)) {
                    Ok(s) => s[0], Err(_) => { ctx.device.destroy_descriptor_pool(pool, None); return; }
                };
                self.vxgi_gi_pool = Some(pool);
                self.vxgi_gi_set = Some(set);

                // Host-visible UBO for the dlights (created once, written each frame).
                let bi = vk::BufferCreateInfo::default().size(ubo_size)
                    .usage(vk::BufferUsageFlags::UNIFORM_BUFFER).sharing_mode(vk::SharingMode::EXCLUSIVE);
                if let Ok(buf) = ctx.device.create_buffer(&bi, None) {
                    let reqs = ctx.device.get_buffer_memory_requirements(buf);
                    let mp = ctx.instance.get_physical_device_memory_properties(ctx.physical_device);
                    if let Some(mt) = (0..mp.memory_type_count).find(|&i| (reqs.memory_type_bits & (1 << i)) != 0
                        && mp.memory_types[i as usize].property_flags.contains(vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT)) {
                        if let Ok(mem) = ctx.device.allocate_memory(&vk::MemoryAllocateInfo::default().allocation_size(reqs.size).memory_type_index(mt), None) {
                            let _ = ctx.device.bind_buffer_memory(buf, mem, 0);
                            self.vxgi_gi_dlight_buf = Some(buf);
                            self.vxgi_gi_dlight_mem = Some(mem);
                        }
                    }
                }
            }
            let set = self.vxgi_gi_set.unwrap();
            // Upload this frame's dlights into the UBO.
            if let Some(mem) = self.vxgi_gi_dlight_mem {
                if let Ok(ptr) = ctx.device.map_memory(mem, 0, ubo_size, vk::MemoryMapFlags::empty()) {
                    std::ptr::copy_nonoverlapping(&ubo as *const GiDLightUbo as *const u8, ptr as *mut u8, ubo_size as usize);
                    ctx.device.unmap_memory(mem);
                }
            }
            let dlight_buf = match self.vxgi_gi_dlight_buf { Some(b) => b, None => return };
            let d_info = vk::DescriptorImageInfo::default().image_view(depth_view).sampler(depth_samp).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
            let r_info = vk::DescriptorImageInfo::default().image_view(rad_view).sampler(rad_samp).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
            let a_info = vk::DescriptorImageInfo::default().image_view(alb_view).sampler(alb_samp).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
            let u_info = vk::DescriptorBufferInfo::default().buffer(dlight_buf).offset(0).range(ubo_size);
            let writes = [
                vk::WriteDescriptorSet::default().dst_set(set).dst_binding(0).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(std::slice::from_ref(&d_info)),
                vk::WriteDescriptorSet::default().dst_set(set).dst_binding(1).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(std::slice::from_ref(&r_info)),
                vk::WriteDescriptorSet::default().dst_set(set).dst_binding(2).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(std::slice::from_ref(&a_info)),
                vk::WriteDescriptorSet::default().dst_set(set).dst_binding(3).descriptor_type(vk::DescriptorType::UNIFORM_BUFFER).buffer_info(std::slice::from_ref(&u_info)),
            ];
            ctx.device.update_descriptor_sets(&writes, &[]);

            let att = vk::RenderingAttachmentInfo::default()
                .image_view(scene_color_view).image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .load_op(vk::AttachmentLoadOp::LOAD).store_op(vk::AttachmentStoreOp::STORE);
            let area = vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent: vk::Extent2D { width: sw, height: sh } };
            let atts = [att];
            let ri = vk::RenderingInfo::default().render_area(area).layer_count(1).color_attachments(&atts);
            ctx.device.cmd_begin_rendering(cmd, &ri);
            let vp = vk::Viewport { x: 0.0, y: 0.0, width: sw as f32, height: sh as f32, min_depth: 0.0, max_depth: 1.0 };
            ctx.device.cmd_set_viewport(cmd, 0, &[vp]);
            ctx.device.cmd_set_scissor(cmd, 0, &[area]);
            ctx.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipe);
            ctx.device.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, layout, 0, &[set], &[]);
            #[repr(C)]
            struct Push { inv_vp: [f32; 16], cam: [f32; 3], strength: f32, grid_min: [f32; 3], extent: f32, voxel_size: f32 }
            let push = Push { inv_vp, cam: view_origin, strength, grid_min, extent, voxel_size };
            let bytes = std::slice::from_raw_parts(&push as *const Push as *const u8, 104);
            ctx.device.cmd_push_constants(cmd, layout, vk::ShaderStageFlags::FRAGMENT, 0, bytes);
            ctx.device.cmd_draw(cmd, 3, 1, 0, 0);
            ctx.device.cmd_end_rendering(cmd);
        });
    }

    /// Water ripple shimmer: fullscreen additive pass that throws animated caustic light from
    /// the active water plane onto nearby geometry. Reuses the projective-shadow pass's
    /// sampleable depth view (so it must run after that pass) and the VXGI irradiance volume
    /// (which gates the effect to the water body's footprint and tints it).
    fn render_water_shimmer(&mut self, cmd: vk::CommandBuffer, view_proj_flat: [f32; 16], plane_z: f32) {
        use super::shadow_project as sp;
        let grid = match super::vxgi::with_voxel_grid(|vg| (vg.rad_view, vg.rad_sampler, vg.grid_min, vg.extent)) {
            Some(g) => g,
            None => return,
        };
        let (rad_view, rad_samp, grid_min, extent) = grid;
        let depth_view = match self.projective_shadow.depth_sample_view { Some(v) => v, None => return };
        let depth_samp = match self.projective_shadow.sampler { Some(s) => s, None => return };
        let (pipe, layout) = match self.pipelines.as_ref().and_then(|pm| pm.water_shimmer_pipeline()) {
            Some(p) => p,
            None => {
                if let Some(pm) = self.pipelines.as_mut() {
                    if let Err(e) = pm.create_water_shimmer_pipeline() {
                        eprintln!("water shimmer pipeline: {e}");
                    }
                }
                match self.pipelines.as_ref().and_then(|pm| pm.water_shimmer_pipeline()) { Some(p) => p, None => return }
            }
        };
        let set_layout = match self.pipelines.as_ref().and_then(|pm| pm.water_shimmer_set_layout()) { Some(l) => l, None => return };
        let scene_color_view = match self.post_processor.as_ref().and_then(|pp| pp.scene_fbo().color_view()) { Some(v) => v, None => return };
        {
            static ONCE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            if !ONCE.swap(true, std::sync::atomic::Ordering::Relaxed) {
                eprintln!("[shimmer] pass active: plane_z={plane_z:.0}");
            }
        }
        let (sw, sh) = (self.width, self.height);
        let inv_vp = sp::invert(&view_proj_flat);
        let time = self.frame_uniforms.time;
        // Base scale chosen so r_water_shimmer 1 is clearly visible on walls near water
        // (the caustic pattern averages well below 1, and the water light tint dims it too).
        let strength = crate::vk_rmain::rcvars().r_water_shimmer.fresh_value().max(0.0) * 1.5;

        gpu_device::with_device(|ctx| unsafe {
            // SAFETY: device valid, main thread, recording command buffer. Set is (re)written
            // every frame BEFORE this pass's submission like the GI pass — acceptable because
            // the views only change on map reload, when the device idles anyway.
            if self.shimmer_set.is_none() {
                let sizes = [vk::DescriptorPoolSize::default()
                    .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).descriptor_count(2)];
                let pool = match ctx.device.create_descriptor_pool(
                    &vk::DescriptorPoolCreateInfo::default().max_sets(1).pool_sizes(&sizes), None) { Ok(p) => p, Err(_) => return };
                let layouts = [set_layout];
                let set = match ctx.device.allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&layouts)) {
                    Ok(s) => s[0], Err(_) => { ctx.device.destroy_descriptor_pool(pool, None); return; }
                };
                self.shimmer_pool = Some(pool);
                self.shimmer_set = Some(set);
            }
            let set = self.shimmer_set.unwrap();
            let d_info = vk::DescriptorImageInfo::default().image_view(depth_view).sampler(depth_samp)
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
            let r_info = vk::DescriptorImageInfo::default().image_view(rad_view).sampler(rad_samp)
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
            let writes = [
                vk::WriteDescriptorSet::default().dst_set(set).dst_binding(0)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(std::slice::from_ref(&d_info)),
                vk::WriteDescriptorSet::default().dst_set(set).dst_binding(1)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(std::slice::from_ref(&r_info)),
            ];
            ctx.device.update_descriptor_sets(&writes, &[]);

            let att = vk::RenderingAttachmentInfo::default()
                .image_view(scene_color_view).image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .load_op(vk::AttachmentLoadOp::LOAD).store_op(vk::AttachmentStoreOp::STORE);
            let area = vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent: vk::Extent2D { width: sw, height: sh } };
            let atts = [att];
            let ri = vk::RenderingInfo::default().render_area(area).layer_count(1).color_attachments(&atts);
            ctx.device.cmd_begin_rendering(cmd, &ri);
            let vp = vk::Viewport { x: 0.0, y: 0.0, width: sw as f32, height: sh as f32, min_depth: 0.0, max_depth: 1.0 };
            ctx.device.cmd_set_viewport(cmd, 0, &[vp]);
            ctx.device.cmd_set_scissor(cmd, 0, &[area]);
            ctx.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipe);
            ctx.device.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, layout, 0, &[set], &[]);
            #[repr(C)]
            struct Push {
                inv_vp: [f32; 16],
                grid_min: [f32; 3],
                plane_z: f32,
                extent: f32,
                time: f32,
                strength: f32,
                pad: f32,
            }
            let push = Push { inv_vp, grid_min, plane_z, extent, time, strength, pad: 0.0 };
            let bytes = std::slice::from_raw_parts(&push as *const Push as *const u8, 96);
            ctx.device.cmd_push_constants(cmd, layout, vk::ShaderStageFlags::FRAGMENT, 0, bytes);
            ctx.device.cmd_draw(cmd, 3, 1, 0, 0);
            ctx.device.cmd_end_rendering(cmd);
        });
    }

    /// Composite the 3D scene to the swapchain with post-processing (polyblend + gamma).
    ///
    /// Draws a fullscreen triangle that samples the scene FBO and applies
    /// polyblend overlay and gamma correction via push constants.
    fn composite_scene_to_swapchain(&self) {
        let cmd = match self.current_command_buffer {
            Some(cmd) => cmd,
            None => return,
        };

        // Get scene FBO color image for transition
        let scene_color = match self.post_processor.as_ref()
            .and_then(|pp| pp.scene_fbo().color_image())
        {
            Some(img) => img,
            None => return,
        };

        // Get swapchain image
        let sc_data = gpu_device::with_swapchain(|sc| {
            (sc.current_image(), sc.current_image_view(), sc.extent)
        });
        let (sc_image, sc_image_view, sc_extent) = match sc_data {
            Some(v) => v,
            None => return,
        };

        // Get PostProcess pipeline
        let (pp_pipeline, pp_layout) = match self.pipelines.as_ref()
            .and_then(|pm| pm.get(ShaderType::PostProcess, PipelineVariant::PostProcess))
            .map(|p| (p.pipeline, p.layout))
        {
            Some(data) => data,
            None => return,
        };

        // Get scene FBO descriptor set (stored at texnum -1)
        let scene_ds = match super::texture::ensure_descriptor_set(-1) {
            Some(ds) => ds,
            None => return,
        };

        // Build PostProcess push constants
        let polyblend = if crate::vk_rmain::rcvars().vk_polyblend.value != 0.0 {
            let blend = crate::vk_rmain::rg().v_blend;
            if blend[3] > 0.0 { blend } else { [0.0; 4] }
        } else {
            [0.0; 4]
        };
        let gamma = crate::vk_rmain::rcvars().vid_gamma.value;
        let enable_gamma = crate::vk_rmain::rcvars().r_hwgamma.value == 0.0;
        let enable_polyblend = polyblend[3] > 0.0;

        let shadow_lift  = crate::vk_rmain::rcvars().r_shadowlift.value;
        let saturation   = crate::vk_rmain::rcvars().r_saturation.value;
        let contrast     = crate::vk_rmain::rcvars().r_contrast.value;
        let brightness   = crate::vk_rmain::rcvars().r_brightness.value;
        let lut_intensity = crate::vk_rmain::rcvars().r_color_grade_intensity.value;
        let lut_enabled  = crate::vk_rmain::rcvars().r_color_grade.value != 0.0;
        // Bloom is folded into the composite (the standalone PostProcessor bloom path is dead
        // code). 0 disables it; gated by r_bloom, scaled by r_bloom_intensity.
        let bloom_intensity = if crate::vk_rmain::rcvars().r_bloom.value != 0.0 {
            crate::vk_rmain::rcvars().r_bloom_intensity.value
        } else {
            0.0
        };

        let uniforms = PostProcessUniforms {
            polyblend_color: polyblend,
            enable_polyblend: if enable_polyblend { 1 } else { 0 },
            gamma,
            enable_gamma: if enable_gamma { 1 } else { 0 },
            saturation,
            contrast,
            brightness,
            shadow_lift,
            lut_enabled: if lut_enabled { 1 } else { 0 },
            lut_intensity,
            bloom_intensity,
        };

        gpu_device::with_device(|ctx| {
            // SAFETY: Vulkan context valid, called from main thread only.
            unsafe {
                // Transition scene_fbo: COLOR_ATTACHMENT → SHADER_READ_ONLY
                let scene_to_read = vk::ImageMemoryBarrier::default()
                    .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .image(scene_color)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0, level_count: 1,
                        base_array_layer: 0, layer_count: 1,
                    })
                    .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                    .dst_access_mask(vk::AccessFlags::SHADER_READ);

                // Transition swapchain: UNDEFINED → COLOR_ATTACHMENT_OPTIMAL
                let sc_to_color = vk::ImageMemoryBarrier::default()
                    .old_layout(vk::ImageLayout::UNDEFINED)
                    .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .image(sc_image)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0, level_count: 1,
                        base_array_layer: 0, layer_count: 1,
                    })
                    .src_access_mask(vk::AccessFlags::empty())
                    .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE);

                ctx.device.cmd_pipeline_barrier(
                    cmd,
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    vk::PipelineStageFlags::FRAGMENT_SHADER | vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    vk::DependencyFlags::empty(),
                    &[], &[], &[scene_to_read, sc_to_color],
                );

                // Begin dynamic rendering on swapchain (CLEAR to black)
                let color_att = vk::RenderingAttachmentInfo::default()
                    .image_view(sc_image_view)
                    .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .load_op(vk::AttachmentLoadOp::CLEAR)
                    .store_op(vk::AttachmentStoreOp::STORE)
                    .clear_value(vk::ClearValue {
                        color: vk::ClearColorValue { float32: [0.0, 0.0, 0.0, 1.0] },
                    });

                let rendering_info = vk::RenderingInfo::default()
                    .render_area(vk::Rect2D {
                        offset: vk::Offset2D { x: 0, y: 0 },
                        extent: sc_extent,
                    })
                    .layer_count(1)
                    .color_attachments(std::slice::from_ref(&color_att));

                ctx.device.cmd_begin_rendering(cmd, &rendering_info);

                ctx.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pp_pipeline);

                // Post-processing blit: use NORMAL viewport (no Y-flip).
                // The 3D scene was already rendered correctly to the scene FBO with
                // a Y-flip viewport. The fullscreen triangle's UV mapping (v=0 at top,
                // v=1 at bottom) is designed for Vulkan's default coordinate system.
                // Using a Y-flip here would flip the scene texture upside down.
                let viewport = vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: sc_extent.width as f32,
                    height: sc_extent.height as f32,
                    min_depth: 0.0, max_depth: 1.0,
                };
                ctx.device.cmd_set_viewport(cmd, 0, &[viewport]);
                let scissor = vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: sc_extent,
                };
                ctx.device.cmd_set_scissor(cmd, 0, &[scissor]);

                // Bind scene FBO descriptor at set 1
                ctx.device.cmd_bind_descriptor_sets(
                    cmd, vk::PipelineBindPoint::GRAPHICS,
                    pp_layout, 1, &[scene_ds], &[],
                );

                // Push PostProcess uniforms (fragment stage)
                let pp_bytes: &[u8] = std::slice::from_raw_parts(
                    &uniforms as *const PostProcessUniforms as *const u8,
                    std::mem::size_of::<PostProcessUniforms>(),
                );
                ctx.device.cmd_push_constants(
                    cmd, pp_layout, vk::ShaderStageFlags::FRAGMENT, 0, pp_bytes,
                );

                // Fullscreen triangle (3 vertices from gl_VertexIndex, no VBO)
                ctx.device.cmd_draw(cmd, 3, 1, 0, 0);

                ctx.device.cmd_end_rendering(cmd);
                // Swapchain left in COLOR_ATTACHMENT_OPTIMAL for 2D pass
            }
        });
    }
}

// ============================================================
// Entity transform helpers
// ============================================================

/// Build a 4x4 model matrix from entity origin and Euler angles.
///
/// Applies Quake's pitch negation hack (pitch is stored negated for models).
/// World-space planar projection that flattens geometry onto the horizontal plane
/// z = ground_z, for blob shadows. `skew` is a horizontal shadow vector (pointing toward the
/// light): points higher above the ground are sheared by `-skew * height`, so the shadow
/// leans AWAY from the light instead of sitting straight under the model. Straight-down when
/// skew = (0,0). Column-major flat (mat4_multiply order). For p=(x,y,z,1):
///   M·p = (x - skew.x*(z-g), y - skew.y*(z-g), g, 1).
fn shadow_flatten_matrix(skew: [f32; 2], ground_z: f32) -> [f32; 16] {
    let g = ground_z;
    [
        1.0,         0.0,         0.0, 0.0, // column 0 (x coeff)
        0.0,         1.0,         0.0, 0.0, // column 1 (y coeff)
        -skew[0],    -skew[1],    0.0, 0.0, // column 2 (z coeff: shear + discard z)
        skew[0] * g, skew[1] * g, g,   1.0, // column 3 (const: restore shear at ground + set z)
    ]
}

fn build_entity_matrix(origin: &myq2_common::q_shared::Vec3, angles: &myq2_common::q_shared::Vec3) -> [[f32; 4]; 4] {
    let pitch = -angles[0].to_radians(); // Quake pitch negation
    let yaw = angles[1].to_radians();
    let roll = angles[2].to_radians();

    let (sp, cp) = (pitch.sin(), pitch.cos());
    let (sy, cy) = (yaw.sin(), yaw.cos());
    let (sr, cr) = (roll.sin(), roll.cos());

    // Rotation: R = Ryaw * Rpitch * Rroll (Quake convention)
    [
        [cp * cy,                     cp * sy,                     -sp,    0.0],
        [sr * sp * cy - cr * sy,      sr * sp * sy + cr * cy,      sr * cp, 0.0],
        [cr * sp * cy + sr * sy,      cr * sp * sy - sr * cy,      cr * cp, 0.0],
        [origin[0],                   origin[1],                   origin[2], 1.0],
    ]
}

/// Multiply two 4x4 matrices (column-major).
fn mat4_multiply(a: &[[f32; 4]; 4], b: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut result = [[0.0_f32; 4]; 4];
    for col in 0..4 {
        for row in 0..4 {
            result[col][row] = a[0][row] * b[col][0]
                + a[1][row] * b[col][1]
                + a[2][row] * b[col][2]
                + a[3][row] * b[col][3];
        }
    }
    result
}

/// Flatten a column-major 4x4 matrix to a flat [f32; 16] array.
fn mat4_to_flat(m: &[[f32; 4]; 4]) -> [f32; 16] {
    [
        m[0][0], m[0][1], m[0][2], m[0][3],
        m[1][0], m[1][1], m[1][2], m[1][3],
        m[2][0], m[2][1], m[2][2], m[2][3],
        m[3][0], m[3][1], m[3][2], m[3][3],
    ]
}

/// Port of MakeSkyVec from gl_warp.c.
/// Maps (s, t, axis) → (3D position, 2D texture coordinates).
fn make_sky_vec(s: f32, t: f32, axis: usize, sky_min: f32, sky_max: f32) -> ([f32; 3], [f32; 2]) {
    let b = [
        s * crate::vk_warp::SKYBOX_SIZE,
        t * crate::vk_warp::SKYBOX_SIZE,
        crate::vk_warp::SKYBOX_SIZE,
    ];

    let st = &crate::vk_warp::ST_TO_VEC[axis];
    let mut v = [0.0_f32; 3];
    for j in 0..3 {
        let k = st[j];
        if k < 0 {
            v[j] = -b[(-k - 1) as usize];
        } else {
            v[j] = b[(k - 1) as usize];
        }
    }

    // Texture coordinates: s and t mapped to [sky_min, sky_max]
    let tc_s = ((s + 1.0) * 0.5).clamp(sky_min, sky_max);
    let tc_t = (1.0 - (t + 1.0) * 0.5).clamp(sky_min, sky_max);

    (v, [tc_s, tc_t])
}

/// Build sky model matrix: translate(vieworg) * rotate(skyrotate*time, skyaxis).
fn build_sky_matrix(
    vieworg: &[f32; 3],
    skyrotate: f32,
    skyaxis: &[f32; 3],
    time: f32,
) -> [[f32; 4]; 4] {
    // Translation matrix
    let translate: [[f32; 4]; 4] = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [vieworg[0], vieworg[1], vieworg[2], 1.0],
    ];

    if skyrotate == 0.0 {
        return translate;
    }

    // Rotation around arbitrary axis (Rodrigues' rotation formula)
    let angle = (skyrotate * time).to_radians();
    let (s, c) = angle.sin_cos();
    let t = 1.0 - c;

    // Normalize axis
    let len = (skyaxis[0] * skyaxis[0] + skyaxis[1] * skyaxis[1] + skyaxis[2] * skyaxis[2]).sqrt();
    if len < 1e-6 {
        return translate;
    }
    let ax = skyaxis[0] / len;
    let ay = skyaxis[1] / len;
    let az = skyaxis[2] / len;

    let rot: [[f32; 4]; 4] = [
        [t * ax * ax + c,       t * ax * ay + az * s, t * ax * az - ay * s, 0.0],
        [t * ax * ay - az * s,  t * ay * ay + c,      t * ay * az + ax * s, 0.0],
        [t * ax * az + ay * s,  t * ay * az - ax * s, t * az * az + c,      0.0],
        [0.0,                   0.0,                  0.0,                  1.0],
    ];

    mat4_multiply(&translate, &rot)
}

