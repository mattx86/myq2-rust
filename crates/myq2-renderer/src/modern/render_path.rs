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

/// Queued brush model draw data (doors, platforms, etc.).
struct BrushModelDraw {
    /// Model-view-projection matrix (column-major flat).
    mvp: [f32; 16],
    /// First surface index in the BSP surface array.
    first_surface: usize,
    /// Number of surfaces for this brush model.
    num_surfaces: usize,
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
        self.lightmap_array.create_gpu_resources();

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
        // tz = -(fwd.vieworg) = -(1*10 + 0*20 + 0*30) = -10
        assert!(approx_eq(view[12], 20.0, 1e-4), "tx={}", view[12]);
        assert!(approx_eq(view[13], -30.0, 1e-4), "ty={}", view[13]);
        assert!(approx_eq(view[14], -10.0, 1e-4), "tz={}", view[14]);
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

        // c = -(far+near)/(far-near)
        let expected_c = -(far + near) / (far - near);
        assert!(approx_eq(proj[10], expected_c, 1e-4), "c={}, expected {}", proj[10], expected_c);

        // d = -(2*far*near)/(far-near)
        let expected_d = -(2.0 * far * near) / (far - near);
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
                // Create 3D scene pipelines (render to R8G8B8A8_UNORM scene FBO)
                myq2_common::common::com_printf("ModernRenderPath::init: Creating 3D scene pipelines\n");
                pm.set_scene_format(vk::Format::R8G8B8A8_UNORM);
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
                super::texture::create_descriptor_for_view(-1, view, sampler);
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

        // Queue brush model for drawing in flush_3d_scene()
        self.brush_models.push(BrushModelDraw {
            mvp: mat4_to_flat(&mvp),
            first_surface: model.firstmodelsurface as usize,
            num_surfaces: model.nummodelsurfaces as usize,
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
        // Only render flashblend-style lights when the cvar is enabled
        if crate::vk_rmain::rcvars().vk_flashblend.value == 0.0 {
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

        // SAFETY: r_newrefdef is valid for frame duration, main thread only
        let num_dlights = unsafe { crate::vk_local::rfs().r_newrefdef.num_dlights };
        for i in 0..num_dlights as usize {
            // SAFETY: dlight pointer valid, index in bounds
            let dl = unsafe { crate::vk_local::rfs().r_newrefdef.dlight(i) };
            let rad = dl.intensity * 0.35;
            if rad < 1.0 {
                continue;
            }

            let base_vtx = self.dlight_vertices.len() as u32;

            // Center vertex: bright color (matches original GL vertex color)
            let center_color = [
                dl.color[0] * 0.2,
                dl.color[1] * 0.2,
                dl.color[2] * 0.2,
            ];
            // Center vertex: offset slightly toward camera
            self.dlight_vertices.push([
                dl.origin[0] - vpn[0] * rad,
                dl.origin[1] - vpn[1] * rad,
                dl.origin[2] - vpn[2] * rad,
                center_color[0], center_color[1], center_color[2],
            ]);

            // 17 edge vertices (closed fan: vertex 17 == vertex 1)
            // Edge vertices are black — rasterizer interpolates smoothly from center
            for j in (0..=16).rev() {
                let a = (j as f32) / 16.0 * std::f32::consts::TAU;
                let (sin_a, cos_a) = a.sin_cos();
                self.dlight_vertices.push([
                    dl.origin[0] + vright[0] * cos_a * rad + vup[0] * sin_a * rad,
                    dl.origin[1] + vright[1] * cos_a * rad + vup[1] * sin_a * rad,
                    dl.origin[2] + vright[2] * cos_a * rad + vup[2] * sin_a * rad,
                    0.0, 0.0, 0.0, // black at edges
                ]);
            }

            // 16 triangles (triangle list from fan)
            for j in 0..16u32 {
                self.dlight_indices.push(base_vtx);         // center
                self.dlight_indices.push(base_vtx + 1 + j); // edge j
                self.dlight_indices.push(base_vtx + 2 + j); // edge j+1
            }

            self.dlight_draws.push(DlightPushConstants {
                mvp: vp_flat,
                light_origin: dl.origin,
                light_radius: rad,
                light_color: dl.color,
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
        // SAFETY: Single-threaded engine
        unsafe {
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
        let bsp_batches: Vec<(u32, u32, Option<vk::DescriptorSet>)> = self.bsp_geometry.batches().iter()
            .map(|b| {
                let ds = super::texture::ensure_descriptor_set(b.texture_id as i32)
                    .or_else(|| super::texture::ensure_descriptor_set(0));
                (b.first_index, b.index_count, ds)
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
            // SAFETY: Accessing renderer globals for particle texture, main thread only
            let texnum = unsafe {
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
        let brush_model_draws: Vec<([f32; 16], Vec<(u32, u32, Option<vk::DescriptorSet>)>)> =
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
                (bm.mvp, surfaces)
            })
            .collect();

        // Pre-gather alias model draw data
        // Each entry: (vbo, ibo, index_count, skin_ds, Vec<(push_constants, frame_offset)>)
        struct AliasDrawEntry {
            vbo: vk::Buffer,
            ibo: vk::Buffer,
            index_count: u32,
            skin_ds: Option<vk::DescriptorSet>,
            instances: Vec<(AliasPushConstants, u64)>, // (push_constants, frame_offset)
        }

        let alias_pipeline_data = self.pipelines.as_ref()
            .and_then(|pm| pm.get(ShaderType::Alias, PipelineVariant::Opaque))
            .map(|p| (p.pipeline, p.layout));

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

                for instance in batch.instances() {
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
                    instances.push((pc, frame_offset));
                }

                if !instances.is_empty() {
                    alias_draw_entries.push(AliasDrawEntry {
                        vbo: alias_vbo,
                        ibo: alias_ibo,
                        index_count,
                        skin_ds,
                        instances,
                    });
                }
            }
            if alias_batch_found > 0 || alias_registered_count > 0 {
                eprintln!("ALIAS: pipeline=true, batches_with_instances={}, registered_models={}, draw_entries={}",
                    alias_batch_found, alias_registered_count, alias_draw_entries.len());
            }
        } else if alias_registered_count > 0 {
            eprintln!("ALIAS: pipeline=NONE, registered_models={}", alias_registered_count);
        }

        // Pre-gather alpha surface data
        let alpha_blend_pipeline_data = self.pipelines.as_ref()
            .and_then(|pm| pm.get(ShaderType::World, PipelineVariant::AlphaBlend))
            .map(|p| (p.pipeline, p.layout));

        let alpha_surface_draws: Vec<(u32, u32, f32, Option<vk::DescriptorSet>)> =
            self.bsp_geometry.alpha_surfaces().iter().map(|surf| {
                let alpha = if surf.flags & super::geometry::SURF_TRANS33 != 0 {
                    0.33_f32
                } else {
                    0.66
                };
                let ds = super::texture::ensure_descriptor_set(surf.texture_id as i32)
                    .or_else(|| super::texture::ensure_descriptor_set(0));
                (surf.first_index, surf.index_count, alpha, ds)
            })
            .collect();

        // Pre-gather turb (water/lava/slime) surface data
        let water_pipeline_data = self.pipelines.as_ref()
            .and_then(|pm| pm.get(ShaderType::Water, PipelineVariant::AlphaBlend))
            .map(|p| (p.pipeline, p.layout));

        let turb_surface_draws: Vec<(u32, u32, f32, Option<vk::DescriptorSet>)> =
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
                (surf.first_index, surf.index_count, alpha, ds)
            })
            .collect();
        let has_turb = !turb_surface_draws.is_empty() && water_pipeline_data.is_some() && has_bsp;
        let water_time = self.frame_uniforms.time;

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

                    ctx.device.cmd_bind_vertex_buffers(cmd, 0, &[bsp_vbo_handle], &[0]);
                    ctx.device.cmd_bind_index_buffer(cmd, bsp_ibo_handle, 0, vk::IndexType::UINT32);

                    // Bind lightmap texture array at set 2 for per-pixel sampling
                    if let Some(lm_ds) = lightmap_ds {
                        ctx.device.cmd_bind_descriptor_sets(
                            cmd, vk::PipelineBindPoint::GRAPHICS,
                            world_layout, 2, &[lm_ds], &[],
                        );
                    }

                    for &(first_index, index_count, ds) in &bsp_batches {
                        if let Some(ds) = ds {
                            ctx.device.cmd_bind_descriptor_sets(
                                cmd, vk::PipelineBindPoint::GRAPHICS,
                                world_layout, 1, &[ds], &[],
                            );
                        }
                        ctx.device.cmd_draw_indexed(cmd, index_count, 1, first_index, 0, 0);
                    }

                    // === Brush Models (opaque) ===
                    // World pipeline + BSP VBO/IBO still bound, alpha=1.0 still set
                    for (bm_mvp, bm_surfaces) in &brush_model_draws {
                        // Push brush model's per-entity MVP
                        let bm_mvp_bytes: &[u8] = std::slice::from_raw_parts(
                            bm_mvp.as_ptr() as *const u8, 64,
                        );
                        ctx.device.cmd_push_constants(
                            cmd, world_layout,
                            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                            0, bm_mvp_bytes,
                        );

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
                    for &(first_index, index_count, _) in &bsp_batches {
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

                    for &(first_index, index_count, alpha, ds) in &turb_surface_draws {
                        // Push per-surface alpha at offset 64
                        let alpha_bytes: &[u8] = std::slice::from_raw_parts(
                            &alpha as *const f32 as *const u8, 4,
                        );
                        ctx.device.cmd_push_constants(
                            cmd, water_layout,
                            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                            64, alpha_bytes,
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

                        for entry in &alias_draw_entries {
                            // Bind skin texture (same for all instances of this model)
                            if let Some(ds) = entry.skin_ds {
                                ctx.device.cmd_bind_descriptor_sets(
                                    cmd, vk::PipelineBindPoint::GRAPHICS,
                                    a_layout, 1, &[ds], &[],
                                );
                            }

                            for (pc, frame_offset) in &entry.instances {
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

        let uniforms = PostProcessUniforms {
            polyblend_color: polyblend,
            enable_polyblend: if enable_polyblend { 1 } else { 0 },
            gamma,
            enable_gamma: if enable_gamma { 1 } else { 0 },
            _pad: 0,
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

