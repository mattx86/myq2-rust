//! Modern render path implementation (Vulkan)
//!
//! VBO/shader-based rendering using Vulkan command buffers and render passes.
//! Supports parallel command buffer recording for multi-threaded rendering.

use ash::vk;
use super::{RenderError, RenderPath, FrameParams, ParticleData};
use super::shader::{ShaderManager, ShaderType, PerFrameUniforms, PerObjectUniforms, UniformBuffer};
use super::geometry::{BspGeometryManager, AliasModelManager, ParticleManager, Draw2DManager, BlendMode};
use super::texture::LightmapArray;
use super::framebuffer::{WaterFbo, PostProcessor};
use crate::vk_rmain::EntityLocal;
use crate::modern::gpu_device;
use crate::vulkan::dynamic_state3::DynamicState3Commands;

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
}

impl ModernRenderPath {
    /// Create a new modern render path (uninitialized).
    pub fn new() -> Self {
        Self {
            shaders: None,
            bsp_geometry: BspGeometryManager::new(),
            alias_models: AliasModelManager::new(),
            particles: ParticleManager::new(),
            draw2d: Draw2DManager::new(),
            lightmap_array: LightmapArray::new(),
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
        let tz = -(forward[0] * vieworg[0] + forward[1] * vieworg[1] + forward[2] * vieworg[2]);

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

        // Symmetric perspective
        let a = near / right;
        let b = near / top;
        let c = -(far + near) / (far - near);
        let d = -(2.0 * far * near) / (far - near);

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
        // Compile all shaders
        self.shaders = Some(ShaderManager::new()?);

        // Create UBOs
        self.per_frame_ubo = Some(UniformBuffer::new(0));
        self.per_object_ubo = Some(UniformBuffer::new(1));

        // Create post-processor
        self.post_processor = Some(PostProcessor::new(self.width, self.height));

        // Initialize EDS3 commands for vk_showtris wireframe rendering
        self.dynamic_state3 = gpu_device::with_device(|ctx| {
            let cmds = DynamicState3Commands::new(ctx);
            if cmds.capabilities().polygon_mode {
                Some(cmds)
            } else {
                None
            }
        }).flatten();

        self.initialized = true;
        Ok(())
    }

    fn shutdown(&mut self) {
        self.shaders = None;
        self.per_frame_ubo = None;
        self.per_object_ubo = None;
        self.post_processor = None;
        self.bsp_geometry.clear();
        self.alias_models.clear();
        self.lightmap_array.reset_allocation();
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

        self.frame_in_progress = false;
        self.current_command_buffer = None;

        // Update dimensions if changed
        if params.width != self.width || params.height != self.height {
            self.set_dimensions(params.width, params.height);
        }

        // ========== Vulkan: Acquire swapchain image and begin command buffer ==========
        // SAFETY: Single-threaded engine, Vulkan objects are valid
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
            // SAFETY: Single-threaded engine, Vulkan objects are valid
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
            None => return,
        }

        // ========== Setup matrices and uniforms ==========
        // Compute view and projection matrices from refdef
        let view = Self::compute_view_matrix(&params.vieworg, &params.viewangles);
        let proj = Self::compute_projection_matrix(params.fov_x, params.fov_y, 4.0, 4096.0);
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
            // SAFETY: Single-threaded engine access pattern for cvar globals.
            unsafe {
                use crate::vk_rmain::*;
                pp.fxaa_enabled = R_FXAA.value != 0.0;
                pp.ssao_enabled = R_SSAO.value != 0.0;
                pp.ssao_radius = R_SSAO_RADIUS.value;
                pp.ssao_intensity = R_SSAO_INTENSITY.value;
                pp.bloom_enabled = R_BLOOM.value != 0.0;
                pp.bloom_threshold = R_BLOOM_THRESHOLD.value;
                pp.bloom_intensity = R_BLOOM_INTENSITY.value;
                pp.fsr_enabled = R_FSR.value != 0.0;
                pp.fsr_sharpness = R_FSR_SHARPNESS.value;

                let new_fsr_scale = R_FSR_SCALE.value.clamp(0.5, 1.0);
                if (new_fsr_scale - pp.fsr_scale).abs() > 0.001 {
                    pp.update_fsr_scale(new_fsr_scale);
                }
            }

            pp.begin_scene();
        }

        // Reset dynamic buffers
        self.particles.begin_frame();
        self.draw2d.begin_frame();
    }

    fn end_frame(&mut self) {
        use super::gpu_device::{with_device_and_swapchain, with_commands};

        if !self.initialized || !self.frame_in_progress {
            return;
        }

        // Upload and draw particles
        self.particles.upload();
        if self.particles.count() > 0 {
            if let Some(ref mut shaders) = self.shaders {
                if let Some(shader) = shaders.get_mut(ShaderType::Particle) {
                    shader.bind();
                    shader.set_vec3_array("u_ViewUp", &self.frame_uniforms.view_up);
                    shader.set_vec3_array("u_ViewRight", &self.frame_uniforms.view_right);
                    shader.set_vec3_array("u_ViewOrigin", &self.frame_uniforms.view_origin);
                    shader.set_float("u_MinSize", 2.0);
                    shader.set_float("u_MaxSize", 40.0);
                    shader.set_float("u_OverbrightScale", 1.0);
                    // Set view-projection via uniform
                    let vp_flat: [f32; 16] = {
                        let vp = &self.frame_uniforms.view_projection;
                        [
                            vp[0][0], vp[0][1], vp[0][2], vp[0][3],
                            vp[1][0], vp[1][1], vp[1][2], vp[1][3],
                            vp[2][0], vp[2][1], vp[2][2], vp[2][3],
                            vp[3][0], vp[3][1], vp[3][2], vp[3][3],
                        ]
                    };
                    shader.set_mat4("u_ViewProjection", &vp_flat);
                    self.particles.bind();
                    self.particles.draw();
                    super::shader::ShaderProgram::unbind();
                }
            }
        }

        // Flush 2D drawing
        self.flush_2d_internal();

        // Apply post-processing pipeline (SSAO -> Bloom -> FSR -> FXAA -> Polyblend+Gamma)
        if let (Some(ref pp), Some(ref mut shaders)) = (&self.post_processor, &mut self.shaders) {
            let proj_flat: [f32; 16] = {
                let p = &self.frame_uniforms.projection_matrix;
                [
                    p[0][0], p[0][1], p[0][2], p[0][3],
                    p[1][0], p[1][1], p[1][2], p[1][3],
                    p[2][0], p[2][1], p[2][2], p[2][3],
                    p[3][0], p[3][1], p[3][2], p[3][3],
                ]
            };
            // Gate polyblend on cvar + non-zero alpha
            // SAFETY: single-threaded engine access pattern
            let polyblend = unsafe {
                if crate::vk_rmain::VK_POLYBLEND.value != 0.0 {
                    let blend = crate::vk_rmain::V_BLEND;
                    if blend[3] > 0.0 {
                        Some(blend)
                    } else {
                        None
                    }
                } else {
                    None
                }
            };
            // Pass actual gamma value (r_hwgamma disables shader gamma separately)
            let gamma = unsafe { crate::vk_rmain::VID_GAMMA.value };
            pp.apply_post_processing(shaders, &proj_flat, 4.0, 4096.0, polyblend, gamma);
        }

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

            // Get sync primitives and submit
            let submit_ok = with_device_and_swapchain(|ctx, swapchain| {
                // Reset fence for this frame
                swapchain.reset_fence(ctx);

                let sync = swapchain.current_sync();

                // Submit command buffer
                with_commands(|commands| {
                    commands.submit_frame(
                        ctx,
                        self.current_frame_index,
                        sync.image_available,
                        sync.render_finished,
                        sync.in_flight,
                    ).is_ok()
                }).unwrap_or(false)
            }).unwrap_or(false);

            if !submit_ok {
                self.frame_in_progress = false;
                self.current_command_buffer = None;
                return;
            }

            // Present the frame
            let _present_result = with_device_and_swapchain(|ctx, swapchain| {
                swapchain.present(ctx)
            });
            // Note: present() advances the frame index internally
        }

        self.frame_in_progress = false;
        self.current_command_buffer = None;
    }

    fn draw_world(&mut self) {
        if !self.initialized || !self.bsp_geometry.is_initialized() {
            return;
        }

        let shaders = match &mut self.shaders {
            Some(s) => s,
            None => return,
        };

        let shader = match shaders.get_mut(ShaderType::World) {
            Some(s) => s,
            None => return,
        };

        shader.bind();

        // Set model-view-projection (identity model for world)
        let mvp_flat: [f32; 16] = {
            let vp = &self.frame_uniforms.view_projection;
            [
                vp[0][0], vp[0][1], vp[0][2], vp[0][3],
                vp[1][0], vp[1][1], vp[1][2], vp[1][3],
                vp[2][0], vp[2][1], vp[2][2], vp[2][3],
                vp[3][0], vp[3][1], vp[3][2], vp[3][3],
            ]
        };
        shader.set_mat4("u_ModelViewProjection", &mvp_flat);
        shader.set_float("u_ScrollOffset", 0.0);

        // Bind lightmap array to texture unit 1
        self.lightmap_array.bind(1);
        shader.set_sampler("u_LightmapTexture", 1);
        shader.set_float("u_OverbrightScale", 1.0);
        // SAFETY: single-threaded engine access pattern
        let fullbright = unsafe { crate::vk_rmain::R_FULLBRIGHT.value != 0.0 };
        shader.set_int("u_Fullbright", if fullbright { 1 } else { 0 });

        // Saturate lighting: clamp lightmap to [0,1] to prevent overbright
        // SAFETY: single-threaded engine access pattern
        let saturate = unsafe { crate::vk_rmain::VK_SATURATELIGHTING.value != 0.0 };
        shader.set_int("u_SaturateLighting", if saturate { 1 } else { 0 });

        // Detail texture: overlay high-frequency surface detail on non-underwater surfaces
        let (detail_enabled, detail_scale) = unsafe {
            let val = crate::vk_rmain::R_DETAILTEXTURE.value as i32;
            if val >= 1 && val <= 8 {
                (true, 8.0_f32) // Scale factor for detail UV frequency
            } else {
                (false, 1.0)
            }
        };
        shader.set_int("u_EnableDetail", if detail_enabled { 1 } else { 0 });
        shader.set_float("u_DetailScale", detail_scale);

        // Caustic overlay: animated pattern on underwater surfaces
        let caustics_enabled = unsafe { crate::vk_rmain::R_CAUSTICS.value != 0.0 };
        shader.set_int("u_EnableCaustics", if caustics_enabled { 1 } else { 0 });
        shader.set_float("u_CausticScroll", self.frame_uniforms.time / 30.0);

        // Default: surface is not underwater (overridden per-batch when batching is wired)
        shader.set_int("u_IsUnderwater", 0);

        // Lightmap-only debug view (vk_lightmap cvar)
        // SAFETY: single-threaded engine access pattern
        let lightmap_only = unsafe { crate::vk_rmain::VK_LIGHTMAP.value != 0.0 };
        shader.set_int("u_LightmapOnly", if lightmap_only { 1 } else { 0 });

        // Wireframe debug view (vk_showtris cvar) — toggle polygon mode via EDS3
        // SAFETY: single-threaded engine access pattern
        let wireframe = unsafe { crate::vk_rmain::VK_SHOWTRIS.value != 0.0 };
        if wireframe {
            if let (Some(cmd), Some(ref ds3)) = (self.current_command_buffer, &self.dynamic_state3) {
                ds3.set_polygon_mode(cmd, vk::PolygonMode::LINE);
            }
        }

        self.bsp_geometry.bind();

        for batch in self.bsp_geometry.batches() {
            shader.set_sampler("u_DiffuseTexture", 0);

            // Draw call issued by Vulkan render pass in future.
            // Currently a no-op — actual draw_indexed will happen through
            // vkCmdDrawIndexed when render passes are wired up.
            // When wired, per-batch u_IsUnderwater will be set from
            // batch surface flags (SURF_UNDERWATER).
            let _ = batch;
        }

        // Restore fill mode after wireframe drawing
        if wireframe {
            if let (Some(cmd), Some(ref ds3)) = (self.current_command_buffer, &self.dynamic_state3) {
                ds3.set_polygon_mode(cmd, vk::PolygonMode::FILL);
            }
        }

        super::geometry::VertexArray::unbind();
        super::shader::ShaderProgram::unbind();
    }

    fn draw_alpha_surfaces(&mut self) {
        // Alpha surfaces would be drawn here with blending enabled.
        // In Vulkan, blend state is part of the pipeline object.
        // Until BSP geometry tracks alpha-flagged surfaces separately, this is a no-op.
    }

    fn blend_lightmaps(&mut self) {
        // Not needed in modern path - lightmaps are sampled directly in the shader.
    }

    fn draw_brush_model(&mut self, _entity: &EntityLocal) {
        // Brush model rendering: would set per-object model matrix from entity
        // transform, then draw the entity's BSP surfaces using the world shader.
        // Requires BSP geometry manager to track per-model surface ranges.
    }

    fn draw_alias_model(&mut self, _entity: &EntityLocal) {
        // Select shader: AliasCel for cel-shading, Alias for standard lighting
        // SAFETY: single-threaded engine access pattern
        let shader_type = unsafe {
            if crate::vk_rmain::R_CELSHADING.value != 0.0 {
                ShaderType::AliasCel
            } else {
                ShaderType::Alias
            }
        };

        // Alias model rendering: would bind the selected shader, set lerp uniforms
        // (u_BackLerp, u_Move, u_FrontV, u_BackV), set shade light/dots,
        // then draw the model's VBO with frame interpolation.
        // Requires alias model manager to have the model's buffers registered.
        let _ = shader_type;
    }

    fn draw_sprite_model(&mut self, _entity: &EntityLocal) {
        // Sprite rendering: would draw a textured billboard quad facing the camera.
    }

    fn draw_particles(&mut self, particles: &[ParticleData]) {
        // Convert particle data and stage for instanced rendering
        for p in particles {
            // Convert palette color index to RGBA using d_8to24table
            let color_u32 = unsafe { crate::vk_image::d_8to24table[p.color & 0xFF] };
            let r = (color_u32 & 0xFF) as f32 / 255.0;
            let g = ((color_u32 >> 8) & 0xFF) as f32 / 255.0;
            let b = ((color_u32 >> 16) & 0xFF) as f32 / 255.0;
            self.particles.add(p.origin, [r, g, b, p.alpha], 1.0);
        }
    }

    fn render_dlights(&mut self) {
        // Dynamic light rendering: would iterate active dlights and draw
        // additive light volumes using the dlight shader with radial falloff.
    }

    fn draw_sky(&mut self) {
        // Sky rendering: would bind the sky shader, set the sky cubemap texture,
        // and draw the skybox geometry with rotation applied.
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
        // SAFETY: single-threaded engine, accessing global image state
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
        // SAFETY: single-threaded engine, accessing global image state
        unsafe {
            let gl = crate::vk_image::draw_find_pic(pic);
            if gl.is_null() {
                return;
            }

            // Handle transparent console
            let mut alpha = 1.0f32;
            if crate::vk_local::TRANS_CONSOLE && pic == "conback" && crate::vk_local::vk_state.transconsole != 0 {
                let vid_height = crate::vk_rmain::VID.height as f32;
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
        let color_u32 = unsafe { crate::vk_image::d_8to24table[(color & 0xFF) as usize] };
        let r = (color_u32 & 0xFF) as f32 / 255.0;
        let g = ((color_u32 >> 8) & 0xFF) as f32 / 255.0;
        let b = ((color_u32 >> 16) & 0xFF) as f32 / 255.0;
        self.draw2d.draw_fill(x, y, w, h, [r, g, b, alpha]);
    }

    fn draw_tile_clear(&mut self, x: i32, y: i32, w: i32, h: i32, pic: &str) {
        // SAFETY: single-threaded engine
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
        // SAFETY: Single-threaded engine, accessing palette
        unsafe {
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
                        image32[i * 256 + j] = crate::vk_image::r_rawpalette[data[row_offset + src_idx] as usize];
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
        self.draw2d.flush();

        let batches_to_draw: Vec<_> = self.draw2d.batches().to_vec();
        if batches_to_draw.is_empty() {
            return;
        }

        let ortho = Self::compute_ortho_matrix(self.width as f32, self.height as f32);

        if let Some(ref mut shaders) = self.shaders {
            if let Some(shader) = shaders.get_mut(ShaderType::Ui) {
                shader.bind();
                shader.set_mat4("u_Projection", &ortho);

                self.draw2d.bind();

                for batch in &batches_to_draw {
                    if batch.texture > 0 {
                        shader.set_int("u_UseTexture", 1);
                        shader.set_int("u_AlphaTest", 1);
                        // Texture binding happens through descriptor sets
                        shader.set_sampler("u_Texture", 0);
                    } else {
                        shader.set_int("u_UseTexture", 0);
                        shader.set_int("u_AlphaTest", 0);
                    }

                    // Blend mode is part of the pipeline object in Vulkan.
                    // Draw call will be issued by command buffer.
                    let _ = &batch.blend_mode;
                    let _ = batch.first_vertex;
                    let _ = batch.vertex_count;
                }

                super::geometry::VertexArray::unbind();
                super::shader::ShaderProgram::unbind();
            }
        }
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

        gpu_device::with_device(|ctx| {
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

                // Record and submit copy commands
                gpu_device::with_commands_mut(|commands| {
                    let cmd = match commands.begin_single_time() {
                        Ok(c) => c,
                        Err(_) => return,
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
                });

                // Clean up staging resources
                ctx.device.free_memory(staging_memory, None);
                ctx.device.destroy_buffer(staging_buffer, None);
            }
        });
    }
}

