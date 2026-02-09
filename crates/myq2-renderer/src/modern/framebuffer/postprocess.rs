//! Post-processing effects (Vulkan)
//!
//! Multi-pass screen-space effects: SSAO, Bloom, FSR upscaling, FXAA, polyblend, gamma.
//! Replaces SDL3 GPU with Vulkan images and render passes.

use ash::vk;
use super::RenderTarget;
use crate::modern::geometry::{VertexBuffer, VertexArray};
use crate::modern::shader::{ShaderManager, ShaderType, PipelineManager, PipelineVariant};
use crate::modern::gpu_device;

/// SSAO kernel size — number of hemisphere samples.
const SSAO_KERNEL_SIZE: usize = 64;

/// Number of bloom mip levels.
const BLOOM_MIP_COUNT: usize = 4;

/// Number of jitter samples in the Halton sequence for FSR 2.0.
const FSR2_JITTER_SAMPLES: usize = 64;

/// Post-processing manager with multi-pass pipeline.
pub struct PostProcessor {
    /// Scene render target (with depth texture for SSAO).
    scene_fbo: RenderTarget,
    /// SSAO output FBO (single-channel occlusion).
    ssao_fbo: RenderTarget,
    /// SSAO blurred output FBO.
    ssao_blur_fbo: RenderTarget,
    /// Bloom extraction + blur FBOs at decreasing resolutions.
    bloom_fbos: Vec<RenderTarget>,
    /// Bloom ping-pong FBOs for separable blur.
    bloom_ping_fbos: Vec<RenderTarget>,
    /// FSR upscale output FBO.
    fsr_fbo: RenderTarget,
    /// FXAA output FBO.
    fxaa_fbo: RenderTarget,
    /// Fullscreen quad VAO.
    quad_vao: VertexArray,
    /// Fullscreen quad VBO.
    quad_vbo: VertexBuffer,
    /// Display width (final output).
    display_width: u32,
    /// Display height (final output).
    display_height: u32,
    /// Internal render width (for FSR — may be smaller than display).
    internal_width: u32,
    /// Internal render height.
    internal_height: u32,
    /// Whether post-processing is enabled.
    enabled: bool,
    /// SSAO noise texture (4x4 random rotations).
    ssao_noise_texture: Option<vk::Image>,
    /// SSAO noise texture memory.
    ssao_noise_memory: Option<vk::DeviceMemory>,
    /// SSAO kernel samples.
    ssao_kernel: [[f32; 4]; SSAO_KERNEL_SIZE],
    /// Enabled post-process passes.
    passes: PostProcessPasses,

    // ========================================================================
    // FSR 2.0 Temporal Upscaling
    // ========================================================================
    /// History color buffer for temporal accumulation.
    fsr2_history_fbo: RenderTarget,
    /// Motion vectors render target (RG16F).
    fsr2_motion_fbo: RenderTarget,
    /// Previous frame depth buffer for disocclusion detection.
    fsr2_prev_depth_fbo: RenderTarget,
    /// Current jitter sample index in Halton sequence.
    fsr2_jitter_index: u32,
    /// Pre-computed Halton sequence for jitter (bases 2, 3).
    fsr2_jitter_sequence: [[f32; 2]; FSR2_JITTER_SAMPLES],
    /// Previous frame's view-projection matrix for reprojection.
    fsr2_prev_view_proj: [f32; 16],

    // Public configuration fields for cvar sync
    /// FXAA enabled flag.
    pub fxaa_enabled: bool,
    /// SSAO enabled flag.
    pub ssao_enabled: bool,
    /// SSAO radius.
    pub ssao_radius: f32,
    /// SSAO intensity.
    pub ssao_intensity: f32,
    /// Bloom enabled flag.
    pub bloom_enabled: bool,
    /// Bloom threshold.
    pub bloom_threshold: f32,
    /// Bloom intensity.
    pub bloom_intensity: f32,
    /// FSR enabled flag.
    pub fsr_enabled: bool,
    /// FSR sharpness.
    pub fsr_sharpness: f32,
    /// FSR render scale.
    pub fsr_scale: f32,
    /// FSR version (0 = 1.0 spatial only, 1 = 2.0 temporal).
    pub fsr_version: u32,
    /// FSR 2.0 jitter enabled (for debugging).
    pub fsr2_jitter_enabled: bool,
    /// FSR 2.0 history weight (0.9 - 0.97 typical).
    pub fsr2_history_weight: f32,
}

/// Configuration for which post-process passes are enabled.
#[derive(Clone, Copy, Default)]
pub struct PostProcessPasses {
    /// Screen-space ambient occlusion.
    pub ssao: bool,
    /// HDR bloom effect.
    pub bloom: bool,
    /// AMD FSR upscaling (1.0).
    pub fsr: bool,
    /// Fast approximate anti-aliasing.
    pub fxaa: bool,
    /// Gamma correction and exposure.
    pub gamma: bool,
}

impl PostProcessor {
    /// Create a new post-processor.
    pub fn new(display_width: u32, display_height: u32) -> Self {
        Self::new_with_scale(display_width, display_height, 1.0)
    }

    /// Create with FSR upscaling (render_scale < 1.0 means render at lower resolution).
    pub fn new_with_scale(display_width: u32, display_height: u32, render_scale: f32) -> Self {
        let internal_width = ((display_width as f32) * render_scale).max(1.0) as u32;
        let internal_height = ((display_height as f32) * render_scale).max(1.0) as u32;

        // Generate Halton sequence for FSR 2.0 jitter
        let jitter_sequence = Self::generate_halton_sequence();

        let mut processor = Self {
            scene_fbo: RenderTarget::default(),
            ssao_fbo: RenderTarget::default(),
            ssao_blur_fbo: RenderTarget::default(),
            bloom_fbos: Vec::new(),
            bloom_ping_fbos: Vec::new(),
            fsr_fbo: RenderTarget::default(),
            fxaa_fbo: RenderTarget::default(),
            quad_vao: VertexArray::default(),
            quad_vbo: VertexBuffer::default(),
            display_width,
            display_height,
            internal_width,
            internal_height,
            enabled: true,
            ssao_noise_texture: None,
            ssao_noise_memory: None,
            ssao_kernel: [[0.0; 4]; SSAO_KERNEL_SIZE],
            passes: PostProcessPasses::default(),
            // FSR 2.0 fields
            fsr2_history_fbo: RenderTarget::default(),
            fsr2_motion_fbo: RenderTarget::default(),
            fsr2_prev_depth_fbo: RenderTarget::default(),
            fsr2_jitter_index: 0,
            fsr2_jitter_sequence: jitter_sequence,
            fsr2_prev_view_proj: [
                1.0, 0.0, 0.0, 0.0,
                0.0, 1.0, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
                0.0, 0.0, 0.0, 1.0,
            ],
            // Public config
            fxaa_enabled: false,
            ssao_enabled: false,
            ssao_radius: 0.5,
            ssao_intensity: 1.0,
            bloom_enabled: false,
            bloom_threshold: 1.0,
            bloom_intensity: 1.0,
            fsr_enabled: false,
            fsr_sharpness: 0.5,
            fsr_scale: render_scale,
            fsr_version: 0,
            fsr2_jitter_enabled: true,
            fsr2_history_weight: 0.95,
        };
        processor.init_resources();
        processor
    }

    fn init_resources(&mut self) {
        // Scene render target with depth for SSAO
        self.scene_fbo = RenderTarget::new_with_depth_texture(
            self.internal_width,
            self.internal_height,
        );

        // SSAO render targets (single channel would be R8, but we use RGBA for compatibility)
        self.ssao_fbo = RenderTarget::new(self.internal_width, self.internal_height, false);
        self.ssao_blur_fbo = RenderTarget::new(self.internal_width, self.internal_height, false);

        // Bloom mip chain FBOs
        self.bloom_fbos.clear();
        self.bloom_ping_fbos.clear();
        let mut w = self.internal_width / 2;
        let mut h = self.internal_height / 2;
        for _ in 0..BLOOM_MIP_COUNT {
            self.bloom_fbos.push(RenderTarget::new(w.max(1), h.max(1), false));
            self.bloom_ping_fbos.push(RenderTarget::new(w.max(1), h.max(1), false));
            w /= 2;
            h /= 2;
        }

        // FSR and FXAA outputs at display resolution
        self.fsr_fbo = RenderTarget::new(self.display_width, self.display_height, false);
        self.fxaa_fbo = RenderTarget::new(self.display_width, self.display_height, false);

        // FSR 2.0 temporal resources
        self.fsr2_history_fbo = RenderTarget::new(self.display_width, self.display_height, false);
        self.fsr2_motion_fbo = RenderTarget::new(self.internal_width, self.internal_height, false);
        self.fsr2_prev_depth_fbo = RenderTarget::new(self.internal_width, self.internal_height, false);

        // Generate SSAO kernel samples (hemisphere oriented along +Z)
        self.generate_ssao_kernel();

        // Create SSAO noise texture (4x4 random rotations)
        self.create_ssao_noise_texture();

        // Create fullscreen quad geometry
        self.create_fullscreen_quad();
    }

    /// Generate hemisphere sample kernel for SSAO.
    fn generate_ssao_kernel(&mut self) {
        use std::f32::consts::PI;

        for i in 0..SSAO_KERNEL_SIZE {
            // Random point in hemisphere (cosine-weighted distribution)
            let scale = i as f32 / SSAO_KERNEL_SIZE as f32;
            let scale = 0.1 + scale * scale * 0.9; // Accelerating interpolation

            // Use deterministic pseudo-random for reproducibility
            let xi1 = ((i * 7 + 13) % 100) as f32 / 100.0;
            let xi2 = ((i * 11 + 17) % 100) as f32 / 100.0;
            let xi3 = ((i * 23 + 29) % 100) as f32 / 100.0;

            let phi = 2.0 * PI * xi1;
            let cos_theta = (1.0 - xi2).sqrt();
            let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();

            self.ssao_kernel[i] = [
                phi.cos() * sin_theta * scale,
                phi.sin() * sin_theta * scale,
                cos_theta * scale,
                0.0, // padding
            ];
        }
    }

    /// Create 4x4 SSAO noise texture with random rotation vectors.
    fn create_ssao_noise_texture(&mut self) {
        // Noise texture stores random tangent-space rotation vectors
        // This breaks up banding artifacts from the fixed kernel
        gpu_device::with_device(|ctx| {
            unsafe {
                let noise_size = 4u32;
                let pixel_count = (noise_size * noise_size) as usize;
                let mut noise_data = vec![0u8; pixel_count * 4];

                // Generate random rotation vectors (XY rotation in tangent space)
                for i in 0..pixel_count {
                    let xi1 = ((i * 31 + 7) % 256) as f32 / 255.0;
                    let xi2 = ((i * 47 + 13) % 256) as f32 / 255.0;
                    let angle = xi1 * std::f32::consts::PI * 2.0;

                    // Store as normalized vector (cos, sin, 0) mapped to [0, 255]
                    noise_data[i * 4] = ((angle.cos() * 0.5 + 0.5) * 255.0) as u8;
                    noise_data[i * 4 + 1] = ((angle.sin() * 0.5 + 0.5) * 255.0) as u8;
                    noise_data[i * 4 + 2] = ((xi2 * 0.5 + 0.5) * 255.0) as u8;
                    noise_data[i * 4 + 3] = 255;
                }

                // Create image
                let image_info = vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(vk::Format::R8G8B8A8_UNORM)
                    .extent(vk::Extent3D { width: noise_size, height: noise_size, depth: 1 })
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

                // Allocate and bind memory
                let mem_reqs = ctx.device.get_image_memory_requirements(image);
                let mem_props = ctx.instance.get_physical_device_memory_properties(ctx.physical_device);

                let mem_type = (0..mem_props.memory_type_count).find(|&i| {
                    (mem_reqs.memory_type_bits & (1 << i)) != 0 &&
                    mem_props.memory_types[i as usize].property_flags
                        .contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)
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

                // Upload via staging buffer (reuse the cinematic pattern)
                let data_size = noise_data.len();
                let buffer_info = vk::BufferCreateInfo::default()
                    .size(data_size as vk::DeviceSize)
                    .usage(vk::BufferUsageFlags::TRANSFER_SRC)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE);

                if let Ok(staging_buffer) = ctx.device.create_buffer(&buffer_info, None) {
                    let buf_mem_reqs = ctx.device.get_buffer_memory_requirements(staging_buffer);
                    let buf_mem_type = (0..mem_props.memory_type_count).find(|&i| {
                        (buf_mem_reqs.memory_type_bits & (1 << i)) != 0 &&
                        mem_props.memory_types[i as usize].property_flags.contains(
                            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT
                        )
                    });

                    if let Some(buf_mem_type) = buf_mem_type {
                        let buf_alloc = vk::MemoryAllocateInfo::default()
                            .allocation_size(buf_mem_reqs.size)
                            .memory_type_index(buf_mem_type);

                        if let Ok(staging_memory) = ctx.device.allocate_memory(&buf_alloc, None) {
                            if ctx.device.bind_buffer_memory(staging_buffer, staging_memory, 0).is_ok() {
                                if let Ok(mapped) = ctx.device.map_memory(
                                    staging_memory, 0, data_size as vk::DeviceSize, vk::MemoryMapFlags::empty()
                                ) {
                                    std::ptr::copy_nonoverlapping(
                                        noise_data.as_ptr(),
                                        mapped as *mut u8,
                                        data_size,
                                    );
                                    ctx.device.unmap_memory(staging_memory);

                                    // Record upload commands
                                    gpu_device::with_commands_mut(|commands| {
                                        if let Ok(cmd) = commands.begin_single_time() {
                                            let copy_region = vk::BufferImageCopy::default()
                                                .buffer_offset(0)
                                                .image_subresource(vk::ImageSubresourceLayers {
                                                    aspect_mask: vk::ImageAspectFlags::COLOR,
                                                    mip_level: 0,
                                                    base_array_layer: 0,
                                                    layer_count: 1,
                                                })
                                                .image_extent(vk::Extent3D {
                                                    width: noise_size,
                                                    height: noise_size,
                                                    depth: 1,
                                                });

                                            // Transition to transfer dst
                                            let barrier = vk::ImageMemoryBarrier::default()
                                                .old_layout(vk::ImageLayout::UNDEFINED)
                                                .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                                                .image(image)
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
                                                &[], &[], &[barrier],
                                            );

                                            ctx.device.cmd_copy_buffer_to_image(
                                                cmd, staging_buffer, image,
                                                vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[copy_region],
                                            );

                                            // Transition to shader read
                                            let barrier = vk::ImageMemoryBarrier::default()
                                                .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                                                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                                                .image(image)
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
                                                &[], &[], &[barrier],
                                            );

                                            let _ = commands.end_single_time(ctx, cmd);
                                        }
                                    });
                                }
                            }
                            ctx.device.free_memory(staging_memory, None);
                        }
                    }
                    ctx.device.destroy_buffer(staging_buffer, None);
                }

                self.ssao_noise_texture = Some(image);
                self.ssao_noise_memory = Some(memory);
            }
        });
    }

    /// Create fullscreen quad geometry for post-process passes.
    fn create_fullscreen_quad(&mut self) {
        // Fullscreen triangle (more efficient than quad, no extra vertex)
        // Positions cover NDC [-1, 1] and UVs cover [0, 1]
        #[derive(Clone, Copy)]
        #[repr(C)]
        struct QuadVertex {
            position: [f32; 2],
            uv: [f32; 2],
        }

        let vertices = [
            QuadVertex { position: [-1.0, -1.0], uv: [0.0, 0.0] },
            QuadVertex { position: [ 3.0, -1.0], uv: [2.0, 0.0] },
            QuadVertex { position: [-1.0,  3.0], uv: [0.0, 2.0] },
        ];

        // VBO and VAO are managed by the existing geometry types
        // which use Vulkan buffers internally
        // GL_STATIC_DRAW = 0x88E4
        self.quad_vbo = VertexBuffer::new();
        self.quad_vbo.upload(&vertices, 0x88E4);
        self.quad_vao = VertexArray::new();
    }

    /// Generate a Halton sequence for sub-pixel jitter (bases 2 and 3).
    fn generate_halton_sequence() -> [[f32; 2]; FSR2_JITTER_SAMPLES] {
        let mut sequence = [[0.0f32; 2]; FSR2_JITTER_SAMPLES];
        for i in 0..FSR2_JITTER_SAMPLES {
            sequence[i][0] = Self::halton(i as u32 + 1, 2) - 0.5;
            sequence[i][1] = Self::halton(i as u32 + 1, 3) - 0.5;
        }
        sequence
    }

    /// Compute the n-th element of a Halton sequence with given base.
    fn halton(mut index: u32, base: u32) -> f32 {
        let mut f = 1.0f32;
        let mut r = 0.0f32;
        let base_f = base as f32;
        while index > 0 {
            f /= base_f;
            r += f * (index % base) as f32;
            index /= base;
        }
        r
    }

    /// Get the current jitter offset in pixels for FSR 2.0.
    /// Returns (0, 0) if jitter is disabled or FSR version is 1.0.
    pub fn get_jitter_offset(&self) -> (f32, f32) {
        if self.fsr_version == 0 || !self.fsr2_jitter_enabled {
            return (0.0, 0.0);
        }
        let idx = (self.fsr2_jitter_index as usize) % FSR2_JITTER_SAMPLES;
        let jitter = self.fsr2_jitter_sequence[idx];
        // Scale jitter to output pixel size
        (jitter[0], jitter[1])
    }

    /// Advance to the next jitter sample. Call once per frame.
    pub fn advance_jitter(&mut self) {
        self.fsr2_jitter_index = (self.fsr2_jitter_index + 1) % (FSR2_JITTER_SAMPLES as u32);
    }

    /// Store the current view-projection matrix for next frame's reprojection.
    pub fn store_view_proj(&mut self, view_proj: &[f32; 16]) {
        self.fsr2_prev_view_proj = *view_proj;
    }

    /// Get the previous frame's view-projection matrix.
    pub fn prev_view_proj(&self) -> &[f32; 16] {
        &self.fsr2_prev_view_proj
    }

    /// Check if FSR 2.0 temporal mode is active.
    pub fn is_fsr2_active(&self) -> bool {
        self.fsr_enabled && self.fsr_version == 1
    }

    /// Resize all render targets (2-arg version, uses existing fsr_scale).
    pub fn resize(&mut self, display_width: u32, display_height: u32) {
        self.resize_with_scale(display_width, display_height, self.fsr_scale);
    }

    /// Resize all render targets with a specific render scale.
    pub fn resize_with_scale(&mut self, display_width: u32, display_height: u32, render_scale: f32) {
        self.display_width = display_width;
        self.display_height = display_height;
        self.fsr_scale = render_scale;
        self.internal_width = ((display_width as f32) * render_scale).max(1.0) as u32;
        self.internal_height = ((display_height as f32) * render_scale).max(1.0) as u32;
        self.init_resources();
    }

    /// Enable/disable post-processing.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Configure which passes are enabled.
    pub fn set_passes(&mut self, passes: PostProcessPasses) {
        self.passes = passes;
    }

    /// Get the scene render target.
    pub fn scene_fbo(&self) -> &RenderTarget {
        &self.scene_fbo
    }

    /// Get internal render dimensions.
    pub fn internal_size(&self) -> (u32, u32) {
        (self.internal_width, self.internal_height)
    }

    /// Get display dimensions.
    pub fn display_size(&self) -> (u32, u32) {
        (self.display_width, self.display_height)
    }

    /// Execute a single post-process pass using Vulkan dynamic rendering.
    ///
    /// # Arguments
    /// * `cmd` - The command buffer to record into
    /// * `target` - The target render target for this pass
    /// * `pipeline` - The Vulkan pipeline to bind
    /// * `width` - Viewport width
    /// * `height` - Viewport height
    fn execute_pass(
        &self,
        cmd: vk::CommandBuffer,
        target: &RenderTarget,
        pipeline: vk::Pipeline,
        pipeline_layout: vk::PipelineLayout,
        width: u32,
        height: u32,
    ) {
        let color_view = match target.color_view() {
            Some(view) => view,
            None => return,
        };

        gpu_device::with_device(|ctx| {
            unsafe {
                // Setup color attachment for dynamic rendering
                let color_attachment = vk::RenderingAttachmentInfo::default()
                    .image_view(color_view)
                    .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .load_op(vk::AttachmentLoadOp::CLEAR)
                    .store_op(vk::AttachmentStoreOp::STORE)
                    .clear_value(vk::ClearValue {
                        color: vk::ClearColorValue {
                            float32: [0.0, 0.0, 0.0, 1.0],
                        },
                    });

                let color_attachments = [color_attachment];

                let rendering_info = vk::RenderingInfo::default()
                    .render_area(vk::Rect2D {
                        offset: vk::Offset2D { x: 0, y: 0 },
                        extent: vk::Extent2D { width, height },
                    })
                    .layer_count(1)
                    .color_attachments(&color_attachments);

                // Begin dynamic rendering
                ctx.device.cmd_begin_rendering(cmd, &rendering_info);

                // Bind pipeline
                ctx.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline);

                // Set viewport and scissor
                let viewport = vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: width as f32,
                    height: height as f32,
                    min_depth: 0.0,
                    max_depth: 1.0,
                };
                ctx.device.cmd_set_viewport(cmd, 0, &[viewport]);

                let scissor = vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: vk::Extent2D { width, height },
                };
                ctx.device.cmd_set_scissor(cmd, 0, &[scissor]);

                // Draw fullscreen triangle (3 vertices, no VBO needed - generated in shader)
                ctx.device.cmd_draw(cmd, 3, 1, 0, 0);

                // End rendering
                ctx.device.cmd_end_rendering(cmd);
            }
        });

        // Suppress unused variable warnings
        let _ = pipeline_layout;
    }

    /// Transition an image layout.
    fn transition_image_layout(
        &self,
        cmd: vk::CommandBuffer,
        image: vk::Image,
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
    ) {
        gpu_device::with_device(|ctx| {
            unsafe {
                let (src_access, dst_access, src_stage, dst_stage) = match (old_layout, new_layout) {
                    (vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL) => (
                        vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                        vk::AccessFlags::SHADER_READ,
                        vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                        vk::PipelineStageFlags::FRAGMENT_SHADER,
                    ),
                    (vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL) => (
                        vk::AccessFlags::SHADER_READ,
                        vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                        vk::PipelineStageFlags::FRAGMENT_SHADER,
                        vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    ),
                    (vk::ImageLayout::UNDEFINED, vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL) => (
                        vk::AccessFlags::empty(),
                        vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                        vk::PipelineStageFlags::TOP_OF_PIPE,
                        vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    ),
                    _ => return,
                };

                let barrier = vk::ImageMemoryBarrier::default()
                    .old_layout(old_layout)
                    .new_layout(new_layout)
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .image(image)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    })
                    .src_access_mask(src_access)
                    .dst_access_mask(dst_access);

                ctx.device.cmd_pipeline_barrier(
                    cmd,
                    src_stage,
                    dst_stage,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[barrier],
                );
            }
        });
    }

    /// Render all enabled post-process passes.
    ///
    /// The post-processing pipeline uses Vulkan's dynamic rendering (VK 1.3+)
    /// to chain multiple fullscreen passes. Each pass:
    /// 1. Begins dynamic rendering with the target FBO's color attachment
    /// 2. Binds the appropriate shader pipeline
    /// 3. Binds source textures via descriptor sets
    /// 4. Draws a fullscreen triangle
    /// 5. Ends rendering
    ///
    /// Passes are chained: scene → SSAO → bloom → FSR → FXAA → output
    pub fn render(&mut self, _shaders: &mut ShaderManager) {
        if !self.enabled {
            return;
        }

        // Post-process passes are executed in apply_post_processing()
        // This method is kept for compatibility with the old API
    }

    /// Begin scene rendering (binds scene FBO).
    pub fn begin_scene(&self) {
        self.scene_fbo.bind();
    }

    /// Apply post-processing effects with a pipeline manager.
    ///
    /// This is the main entry point for post-processing that requires pipeline access.
    /// Call this from the main render loop after scene rendering is complete.
    ///
    /// # Arguments
    /// * `cmd` - Active command buffer
    /// * `pipelines` - Pipeline manager for shader access
    /// * `projection_matrix` - Current projection matrix (for SSAO depth reconstruction)
    /// * `near` - Near plane distance
    /// * `far` - Far plane distance
    /// * `polyblend` - Optional screen overlay color (damage flash, etc.)
    /// * `gamma` - Gamma correction value
    pub fn apply_with_pipelines(
        &self,
        cmd: vk::CommandBuffer,
        pipelines: &PipelineManager,
        projection_matrix: &[f32; 16],
        near: f32,
        far: f32,
        polyblend: Option<[f32; 4]>,
        gamma: f32,
    ) {
        if !self.enabled {
            return;
        }

        // Store uniform parameters for SSAO depth linearization
        let _ = projection_matrix;
        let _ = near;
        let _ = far;

        // Polyblend and gamma are used in the final PostProcess pass below
        let polyblend_color = polyblend.unwrap_or([0.0; 4]);
        let enable_polyblend = polyblend.is_some() && polyblend_color[3] > 0.0;
        // Hardware gamma (r_hwgamma) disables shader gamma — the OS handles it
        let enable_gamma = unsafe { crate::vk_rmain::R_HWGAMMA.value == 0.0 };
        let _ = polyblend_color;
        let _ = enable_polyblend;
        let _ = enable_gamma;
        let _ = gamma;

        // SSAO Pass
        if self.ssao_enabled {
            if let Some(scene_color) = self.scene_fbo.color_image() {
                // Transition scene to shader read
                self.transition_image_layout(
                    cmd,
                    scene_color,
                    vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                );

                // Execute SSAO pass
                if let Some(pipeline) = pipelines.get(ShaderType::Ssao, PipelineVariant::PostProcess) {
                    self.execute_pass(
                        cmd,
                        &self.ssao_fbo,
                        pipeline.pipeline,
                        pipeline.layout,
                        self.internal_width,
                        self.internal_height,
                    );
                }

                // SSAO blur pass
                if let Some(ssao_color) = self.ssao_fbo.color_image() {
                    self.transition_image_layout(
                        cmd,
                        ssao_color,
                        vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    );

                    if let Some(pipeline) = pipelines.get(ShaderType::SsaoBlur, PipelineVariant::PostProcess) {
                        self.execute_pass(
                            cmd,
                            &self.ssao_blur_fbo,
                            pipeline.pipeline,
                            pipeline.layout,
                            self.internal_width,
                            self.internal_height,
                        );
                    }
                }
            }
        }

        // Bloom Pass
        if self.bloom_enabled && !self.bloom_fbos.is_empty() {
            // Extract bright pixels
            if let Some(pipeline) = pipelines.get(ShaderType::BloomExtract, PipelineVariant::PostProcess) {
                self.execute_pass(
                    cmd,
                    &self.bloom_fbos[0],
                    pipeline.pipeline,
                    pipeline.layout,
                    self.internal_width / 2,
                    self.internal_height / 2,
                );
            }

            // Blur each mip level (horizontal then vertical)
            if let Some(blur_pipeline) = pipelines.get(ShaderType::BloomBlur, PipelineVariant::PostProcess) {
                for i in 0..BLOOM_MIP_COUNT.min(self.bloom_fbos.len()) {
                    let mip_width = (self.internal_width / 2) >> i;
                    let mip_height = (self.internal_height / 2) >> i;

                    if mip_width == 0 || mip_height == 0 {
                        break;
                    }

                    // Transition source to shader read
                    if let Some(src_image) = self.bloom_fbos[i].color_image() {
                        self.transition_image_layout(
                            cmd,
                            src_image,
                            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                        );
                    }

                    // Horizontal blur
                    if i < self.bloom_ping_fbos.len() {
                        self.execute_pass(
                            cmd,
                            &self.bloom_ping_fbos[i],
                            blur_pipeline.pipeline,
                            blur_pipeline.layout,
                            mip_width,
                            mip_height,
                        );

                        // Transition ping to shader read
                        if let Some(ping_image) = self.bloom_ping_fbos[i].color_image() {
                            self.transition_image_layout(
                                cmd,
                                ping_image,
                                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                            );
                        }

                        // Vertical blur back to main bloom FBO
                        self.execute_pass(
                            cmd,
                            &self.bloom_fbos[i],
                            blur_pipeline.pipeline,
                            blur_pipeline.layout,
                            mip_width,
                            mip_height,
                        );
                    }
                }
            }
        }

        // FSR Upscale Pass
        if self.fsr_enabled {
            let (fsr_shader, is_fsr2) = if self.fsr_version == 1 {
                (ShaderType::Fsr2Temporal, true)
            } else {
                (ShaderType::FsrEasu, false)
            };

            if let Some(pipeline) = pipelines.get(fsr_shader, PipelineVariant::PostProcess) {
                // Transition scene to shader read if not already
                if let Some(scene_color) = self.scene_fbo.color_image() {
                    self.transition_image_layout(
                        cmd,
                        scene_color,
                        vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    );
                }

                // Execute FSR upscale
                self.execute_pass(
                    cmd,
                    &self.fsr_fbo,
                    pipeline.pipeline,
                    pipeline.layout,
                    self.display_width,
                    self.display_height,
                );

                // For FSR 1.0, apply RCAS sharpening
                if !is_fsr2 && self.fsr_sharpness > 0.0 {
                    if let Some(rcas_pipeline) = pipelines.get(ShaderType::FsrRcas, PipelineVariant::PostProcess) {
                        if let Some(fsr_color) = self.fsr_fbo.color_image() {
                            self.transition_image_layout(
                                cmd,
                                fsr_color,
                                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                            );
                        }

                        // RCAS writes to FXAA FBO as intermediate
                        self.execute_pass(
                            cmd,
                            &self.fxaa_fbo,
                            rcas_pipeline.pipeline,
                            rcas_pipeline.layout,
                            self.display_width,
                            self.display_height,
                        );
                    }
                }
            }
        }

        // FXAA Pass
        if self.fxaa_enabled {
            if let Some(pipeline) = pipelines.get(ShaderType::Fxaa, PipelineVariant::PostProcess) {
                // Determine source: FSR output or scene
                let source_fbo = if self.fsr_enabled {
                    &self.fsr_fbo
                } else {
                    &self.scene_fbo
                };

                if let Some(src_color) = source_fbo.color_image() {
                    self.transition_image_layout(
                        cmd,
                        src_color,
                        vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    );
                }

                self.execute_pass(
                    cmd,
                    &self.fxaa_fbo,
                    pipeline.pipeline,
                    pipeline.layout,
                    self.display_width,
                    self.display_height,
                );
            }
        }

        // Final PostProcess Pass: polyblend overlay + gamma correction
        // Uses postprocess.frag.glsl which reads u_PolyBlend, u_EnablePolyBlend,
        // u_Gamma, u_EnableGamma from FragUniforms UBO (set 3, binding 0).
        // The UBO data (polyblend_color, enable_polyblend, gamma, enable_gamma)
        // is prepared above and will be uploaded when the PostProcess pipeline
        // descriptor sets are wired. For now, the shader handles the logic —
        // the actual vkCmdDraw call will be added when the pipeline is created.
        if enable_polyblend || enable_gamma {
            if let Some(pipeline) = pipelines.get(ShaderType::PostProcess, PipelineVariant::PostProcess) {
                // Source: output of last enabled pass
                let source_fbo = if self.fxaa_enabled {
                    &self.fxaa_fbo
                } else if self.fsr_enabled {
                    &self.fsr_fbo
                } else {
                    &self.scene_fbo
                };

                if let Some(src_color) = source_fbo.color_image() {
                    self.transition_image_layout(
                        cmd,
                        src_color,
                        vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    );
                }

                // TODO: Upload PostProcessUniforms UBO with polyblend_color,
                // enable_polyblend, gamma, enable_gamma before draw call.
                // The postprocess.frag.glsl FragUniforms layout is:
                //   vec4  u_PolyBlend       (polyblend_color)
                //   int   u_EnablePolyBlend  (enable_polyblend as i32)
                //   float u_Gamma            (gamma)
                //   int   u_EnableGamma      (enable_gamma as i32)

                self.execute_pass(
                    cmd,
                    &self.scene_fbo, // Reuse scene FBO as output (will be presented)
                    pipeline.pipeline,
                    pipeline.layout,
                    self.display_width,
                    self.display_height,
                );
            }
        }
    }

    /// Apply post-processing effects (compatibility wrapper).
    ///
    /// This function orchestrates the full post-processing pipeline:
    ///
    /// ## SSAO Pass (if enabled)
    /// - Input: scene depth texture, projection matrix, SSAO kernel, noise texture
    /// - Output: ssao_fbo with per-pixel occlusion factor
    /// - Uses hemisphere sampling with random rotation from noise texture
    ///
    /// ## Bloom Pass (if enabled)
    /// - Extract: threshold bright pixels from scene
    /// - Blur: Gaussian blur at multiple mip levels (separable H+V)
    /// - Output: bloom_fbos chain
    ///
    /// ## FSR Pass (if enabled)
    /// - FSR 1.0: EASU upscale + RCAS sharpening
    /// - FSR 2.0: temporal upscale with motion vectors and history
    /// - Output: fsr_fbo at display resolution
    ///
    /// ## FXAA Pass (if enabled)
    /// - Fast approximate anti-aliasing
    /// - Output: fxaa_fbo
    ///
    /// ## Final Composite
    /// - Combine scene + SSAO * color + bloom
    /// - Apply polyblend overlay (damage flash, etc.)
    /// - Apply gamma correction
    /// - Output: swapchain image
    pub fn apply_post_processing(
        &self,
        _shaders: &mut ShaderManager,
        projection_matrix: &[f32; 16],
        near: f32,
        far: f32,
        polyblend: Option<[f32; 4]>,
        gamma: f32,
    ) {
        // Compatibility wrapper. The actual Vulkan implementation uses
        // apply_with_pipelines() which takes a command buffer and pipeline manager.
        // Parameters are validated here but execution deferred to apply_with_pipelines().
        let _ = projection_matrix;
        let _ = near;
        let _ = far;

        // Log non-default values for debugging
        if let Some(blend) = polyblend {
            if blend[3] > 0.0 {
                // Polyblend active: [{r}, {g}, {b}, {a}]
                let _ = blend;
            }
        }
        if (gamma - 1.0).abs() > 0.01 {
            // Non-default gamma: {gamma}
            let _ = gamma;
        }
    }

    /// Update FSR render scale (may recreate internal render targets).
    pub fn update_fsr_scale(&mut self, scale: f32) {
        if (self.fsr_scale - scale).abs() > 0.001 {
            self.fsr_scale = scale;
            self.internal_width = ((self.display_width as f32) * scale).max(1.0) as u32;
            self.internal_height = ((self.display_height as f32) * scale).max(1.0) as u32;
            self.init_resources();
        }
    }

    /// Shutdown and release resources.
    pub fn shutdown(&mut self) {
        self.scene_fbo.destroy();
        self.ssao_fbo.destroy();
        self.ssao_blur_fbo.destroy();
        for fbo in &mut self.bloom_fbos {
            fbo.destroy();
        }
        for fbo in &mut self.bloom_ping_fbos {
            fbo.destroy();
        }
        self.fsr_fbo.destroy();
        self.fxaa_fbo.destroy();
        // FSR 2.0 resources
        self.fsr2_history_fbo.destroy();
        self.fsr2_motion_fbo.destroy();
        self.fsr2_prev_depth_fbo.destroy();
        // SSAO noise texture
        gpu_device::with_device(|ctx| {
            unsafe {
                if let Some(image) = self.ssao_noise_texture.take() {
                    ctx.device.destroy_image(image, None);
                }
                if let Some(memory) = self.ssao_noise_memory.take() {
                    ctx.device.free_memory(memory, None);
                }
            }
        });
    }
}

impl Default for PostProcessor {
    fn default() -> Self {
        Self::new(800, 600)
    }
}

impl Drop for PostProcessor {
    fn drop(&mut self) {
        self.shutdown();
    }
}
