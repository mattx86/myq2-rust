//! Opacity Micromap for Optimized Alpha Testing in Ray Tracing
//!
//! VK_EXT_opacity_micromap accelerates ray tracing with alpha-tested geometry:
//! - Pre-computed opacity data per micro-triangle
//! - Faster intersection for foliage, fences, etc.
//! - Reduces any-hit shader invocations
//! - Significant performance gains for complex scenes

use ash::vk;

/// Opacity micromap capabilities.
#[derive(Debug, Clone, Default)]
pub struct OpacityMicromapCapabilities {
    /// Whether opacity micromap is supported.
    pub supported: bool,
    /// Maximum micromap subdivision level.
    pub max_subdivision_level: u32,
}

/// Query opacity micromap capabilities.
pub fn query_capabilities(ctx: &super::context::VulkanContext) -> OpacityMicromapCapabilities {
    let mut omm_features = vk::PhysicalDeviceOpacityMicromapFeaturesEXT::default();
    let mut features2 = vk::PhysicalDeviceFeatures2::default()
        .push_next(&mut omm_features);

    unsafe {
        ctx.instance.get_physical_device_features2(ctx.physical_device, &mut features2);
    }

    let _ = features2;
    let supported = omm_features.micromap == vk::TRUE;

    if !supported {
        return OpacityMicromapCapabilities::default();
    }

    let mut omm_props = vk::PhysicalDeviceOpacityMicromapPropertiesEXT::default();
    let mut props2 = vk::PhysicalDeviceProperties2::default()
        .push_next(&mut omm_props);

    unsafe {
        ctx.instance.get_physical_device_properties2(ctx.physical_device, &mut props2);
    }

    OpacityMicromapCapabilities {
        supported,
        max_subdivision_level: omm_props.max_opacity2_state_subdivision_level,
    }
}

/// Opacity state for a micro-triangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpacityState {
    /// Fully transparent (skip intersection).
    Transparent,
    /// Fully opaque (accept intersection).
    Opaque,
    /// Unknown/mixed (run any-hit shader).
    UnknownTransparent,
    /// Unknown opaque.
    UnknownOpaque,
}

impl OpacityState {
    /// Convert to Vulkan opacity state.
    pub fn to_vk(&self) -> u8 {
        match self {
            OpacityState::Transparent => 0,
            OpacityState::Opaque => 1,
            OpacityState::UnknownTransparent => 2,
            OpacityState::UnknownOpaque => 3,
        }
    }

    /// Create from alpha value.
    pub fn from_alpha(alpha: f32, threshold: f32) -> Self {
        if alpha < threshold * 0.1 {
            OpacityState::Transparent
        } else if alpha > 1.0 - threshold * 0.1 {
            OpacityState::Opaque
        } else if alpha < threshold {
            OpacityState::UnknownTransparent
        } else {
            OpacityState::UnknownOpaque
        }
    }
}

/// Micromap format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicromapFormat {
    /// 2-state (transparent/opaque).
    TwoState,
    /// 4-state (includes unknown states).
    FourState,
}

impl MicromapFormat {
    /// Convert to Vulkan format.
    pub fn to_vk(&self) -> vk::OpacityMicromapFormatEXT {
        match self {
            MicromapFormat::TwoState => vk::OpacityMicromapFormatEXT::TYPE_2_STATE,
            MicromapFormat::FourState => vk::OpacityMicromapFormatEXT::TYPE_4_STATE,
        }
    }

    /// Bits per micro-triangle.
    pub fn bits_per_micro_triangle(&self) -> u32 {
        match self {
            MicromapFormat::TwoState => 1,
            MicromapFormat::FourState => 2,
        }
    }
}

/// Subdivision level for micromap.
#[derive(Debug, Clone, Copy)]
pub struct SubdivisionLevel(pub u32);

impl SubdivisionLevel {
    /// Number of micro-triangles at this level.
    pub fn micro_triangle_count(&self) -> u32 {
        1 << (2 * self.0) // 4^level
    }

    /// Number of micro-vertices at this level.
    pub fn micro_vertex_count(&self) -> u32 {
        let n = 1 << self.0;
        (n + 1) * (n + 2) / 2
    }

    /// Create from triangle size in texels.
    pub fn from_triangle_size(texels: u32) -> Self {
        let level = (texels as f32).log2().ceil() as u32;
        SubdivisionLevel(level.min(15))
    }
}

/// Triangle opacity data for micromap building.
#[derive(Debug, Clone)]
pub struct TriangleOpacityData {
    /// Subdivision level for this triangle.
    pub subdivision_level: SubdivisionLevel,
    /// Opacity states for each micro-triangle.
    pub opacity_states: Vec<OpacityState>,
}

impl TriangleOpacityData {
    /// Create from alpha texture data.
    pub fn from_alpha_texture(
        uvs: [[f32; 2]; 3],
        texture_data: &[u8],
        texture_width: u32,
        texture_height: u32,
        alpha_threshold: f32,
        max_subdivision: u32,
    ) -> Self {
        // Determine subdivision level based on triangle size in texture space
        let uv_extent = compute_uv_extent(&uvs);
        let texel_size = (uv_extent * texture_width.max(texture_height) as f32) as u32;
        let level = SubdivisionLevel::from_triangle_size(texel_size.min(1 << max_subdivision));

        let micro_count = level.micro_triangle_count() as usize;
        let mut opacity_states = Vec::with_capacity(micro_count);

        // Sample opacity for each micro-triangle
        for i in 0..micro_count {
            let (u, v) = micro_triangle_center(i as u32, level.0);
            let sample_uv = barycentric_to_uv(&uvs, u, v);

            let alpha = sample_alpha(
                &sample_uv,
                texture_data,
                texture_width,
                texture_height,
            );

            opacity_states.push(OpacityState::from_alpha(alpha, alpha_threshold));
        }

        TriangleOpacityData {
            subdivision_level: level,
            opacity_states,
        }
    }

    /// Create fully opaque.
    pub fn fully_opaque() -> Self {
        TriangleOpacityData {
            subdivision_level: SubdivisionLevel(0),
            opacity_states: vec![OpacityState::Opaque],
        }
    }

    /// Create fully transparent.
    pub fn fully_transparent() -> Self {
        TriangleOpacityData {
            subdivision_level: SubdivisionLevel(0),
            opacity_states: vec![OpacityState::Transparent],
        }
    }

    /// Pack opacity data into bytes.
    pub fn pack(&self, format: MicromapFormat) -> Vec<u8> {
        let bits_per = format.bits_per_micro_triangle();
        let total_bits = self.opacity_states.len() as u32 * bits_per;
        let byte_count = (total_bits + 7) / 8;

        let mut bytes = vec![0u8; byte_count as usize];

        for (i, state) in self.opacity_states.iter().enumerate() {
            let bit_offset = i as u32 * bits_per;
            let byte_index = (bit_offset / 8) as usize;
            let bit_in_byte = bit_offset % 8;

            let value = state.to_vk() & ((1 << bits_per) - 1);
            bytes[byte_index] |= value << bit_in_byte;

            // Handle spanning bytes
            if bit_in_byte + bits_per > 8 && byte_index + 1 < bytes.len() {
                bytes[byte_index + 1] |= value >> (8 - bit_in_byte);
            }
        }

        bytes
    }
}

/// Micromap usage hints.
#[derive(Debug, Clone, Copy, Default)]
pub struct MicromapUsageHints {
    /// Triangle has mostly opaque micro-triangles.
    pub prefer_fast_trace: bool,
    /// Triangle has uniform opacity.
    pub uniform_opacity: bool,
}

/// Helper to compute UV extent of a triangle.
fn compute_uv_extent(uvs: &[[f32; 2]; 3]) -> f32 {
    let mut max_dist = 0.0f32;
    for i in 0..3 {
        for j in (i + 1)..3 {
            let dx = uvs[i][0] - uvs[j][0];
            let dy = uvs[i][1] - uvs[j][1];
            max_dist = max_dist.max((dx * dx + dy * dy).sqrt());
        }
    }
    max_dist
}

/// Get barycentric coordinates of micro-triangle center.
fn micro_triangle_center(index: u32, level: u32) -> (f32, f32) {
    let n = 1 << level;
    let row = ((((8 * index + 1) as f32).sqrt() - 1.0) / 2.0) as u32;
    let col = index - row * (row + 1) / 2;

    let u = (col as f32 + 0.5) / n as f32;
    let v = (row as f32 + 0.5) / n as f32;

    (u.min(1.0 - v), v)
}

/// Convert barycentric to UV coordinates.
fn barycentric_to_uv(uvs: &[[f32; 2]; 3], u: f32, v: f32) -> [f32; 2] {
    let w = 1.0 - u - v;
    [
        uvs[0][0] * w + uvs[1][0] * u + uvs[2][0] * v,
        uvs[0][1] * w + uvs[1][1] * u + uvs[2][1] * v,
    ]
}

/// Sample alpha from texture.
fn sample_alpha(uv: &[f32; 2], data: &[u8], width: u32, height: u32) -> f32 {
    let x = ((uv[0].fract() + 1.0).fract() * width as f32) as u32 % width;
    let y = ((uv[1].fract() + 1.0).fract() * height as f32) as u32 % height;
    let index = ((y * width + x) * 4 + 3) as usize; // Assume RGBA

    if index < data.len() {
        data[index] as f32 / 255.0
    } else {
        1.0
    }
}

/// Micromap build info.
#[derive(Debug, Clone)]
pub struct MicromapBuildInfo {
    /// Total size needed for micromap buffer.
    pub buffer_size: vk::DeviceSize,
    /// Scratch buffer size.
    pub scratch_size: vk::DeviceSize,
    /// Number of triangles.
    pub triangle_count: u32,
}

/// Calculate build sizes for an opacity micromap.
pub fn calculate_build_sizes(
    triangles: &[TriangleOpacityData],
    format: MicromapFormat,
) -> MicromapBuildInfo {
    let mut total_data_size = 0u64;

    for tri in triangles {
        let bits = tri.opacity_states.len() as u32 * format.bits_per_micro_triangle();
        total_data_size += ((bits + 7) / 8) as u64;
    }

    // Add alignment and headers (rough estimate)
    let buffer_size = total_data_size + triangles.len() as u64 * 16 + 256;
    let scratch_size = buffer_size / 2 + 1024;

    MicromapBuildInfo {
        buffer_size,
        scratch_size,
        triangle_count: triangles.len() as u32,
    }
}
