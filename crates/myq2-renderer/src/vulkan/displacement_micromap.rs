//! RT Displacement Mapping (VK_NV_displacement_micromap)
//!
//! Hardware-accelerated displacement mapping for ray tracing:
//! - Subdivision and displacement in BVH traversal
//! - Micromap-based displacement encoding
//! - Efficient ray-heightfield intersection
//! - LOD support for distance-based detail

use ash::vk;

/// Displacement micromap capabilities.
#[derive(Debug, Clone, Default)]
pub struct DisplacementMicromapCapabilities {
    /// Whether displacement micromaps are supported.
    pub supported: bool,
    /// Maximum subdivision level.
    pub max_subdivision_level: u32,
    /// Maximum displacement magnitude.
    pub max_displacement: f32,
}

/// Query displacement micromap capabilities.
pub fn query_capabilities(ctx: &super::context::VulkanContext) -> DisplacementMicromapCapabilities {
    // Note: VK_NV_displacement_micromap is an NVIDIA extension
    // Check if extension is available in device extensions
    let extensions = unsafe {
        ctx.instance
            .enumerate_device_extension_properties(ctx.physical_device)
            .unwrap_or_default()
    };

    let has_extension = extensions.iter().any(|ext| {
        let name = unsafe { std::ffi::CStr::from_ptr(ext.extension_name.as_ptr()) };
        name.to_str().map(|s| s == "VK_NV_displacement_micromap").unwrap_or(false)
    });

    if !has_extension {
        return DisplacementMicromapCapabilities::default();
    }

    // Query features and properties
    DisplacementMicromapCapabilities {
        supported: true,
        max_subdivision_level: 5, // Typical max
        max_displacement: 1.0,    // Normalized
    }
}

/// Displacement encoding format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplacementFormat {
    /// 64 triangles per base triangle (subdivision level 3).
    Level64,
    /// 256 triangles per base triangle (subdivision level 4).
    Level256,
    /// 1024 triangles per base triangle (subdivision level 5).
    Level1024,
}

impl DisplacementFormat {
    /// Get subdivision level.
    pub fn subdivision_level(&self) -> u32 {
        match self {
            DisplacementFormat::Level64 => 3,
            DisplacementFormat::Level256 => 4,
            DisplacementFormat::Level1024 => 5,
        }
    }

    /// Get number of micro-triangles.
    pub fn num_triangles(&self) -> u32 {
        match self {
            DisplacementFormat::Level64 => 64,
            DisplacementFormat::Level256 => 256,
            DisplacementFormat::Level1024 => 1024,
        }
    }

    /// Get number of micro-vertices.
    pub fn num_vertices(&self) -> u32 {
        // For subdivision level n: vertices = (n+1)(n+2)/2
        let n = self.subdivision_level() + 1;
        n * (n + 1) / 2
    }
}

/// Displacement vector encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplacementVectorFormat {
    /// 2-component normalized (tangent space).
    Float2,
    /// 3-component direction.
    Float3,
    /// Single height value.
    Height,
}

/// Displacement micromap build info.
#[derive(Debug, Clone)]
pub struct DisplacementMicromapBuildInfo {
    /// Number of triangles in the base mesh.
    pub triangle_count: u32,
    /// Displacement format.
    pub format: DisplacementFormat,
    /// Vector format.
    pub vector_format: DisplacementVectorFormat,
    /// Displacement scale.
    pub scale: f32,
    /// Displacement bias.
    pub bias: f32,
}

/// Displacement data for a triangle.
#[derive(Debug, Clone)]
pub struct TriangleDisplacementData {
    /// Base triangle index.
    pub triangle_index: u32,
    /// Displacement values per micro-vertex.
    pub displacements: Vec<f32>,
    /// Direction vectors (if using direction mode).
    pub directions: Option<Vec<[f32; 3]>>,
}

/// Displacement micromap for a mesh.
pub struct DisplacementMicromap {
    /// Vulkan micromap handle (placeholder - actual type from extension).
    pub handle: u64,
    /// Buffer containing micromap data.
    pub buffer: vk::Buffer,
    /// Memory allocation.
    pub memory: vk::DeviceMemory,
    /// Build info.
    pub info: DisplacementMicromapBuildInfo,
}

/// Builder for displacement micromaps.
pub struct DisplacementMicromapBuilder {
    triangles: Vec<TriangleDisplacementData>,
    format: DisplacementFormat,
    vector_format: DisplacementVectorFormat,
    scale: f32,
    bias: f32,
}

impl DisplacementMicromapBuilder {
    /// Create new builder.
    pub fn new(format: DisplacementFormat) -> Self {
        Self {
            triangles: Vec::new(),
            format,
            vector_format: DisplacementVectorFormat::Height,
            scale: 1.0,
            bias: 0.0,
        }
    }

    /// Set vector format.
    pub fn vector_format(mut self, format: DisplacementVectorFormat) -> Self {
        self.vector_format = format;
        self
    }

    /// Set displacement scale.
    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    /// Set displacement bias.
    pub fn bias(mut self, bias: f32) -> Self {
        self.bias = bias;
        self
    }

    /// Add triangle displacement data.
    pub fn add_triangle(&mut self, data: TriangleDisplacementData) {
        self.triangles.push(data);
    }

    /// Add displacement from heightmap for a triangle.
    pub fn add_from_heightmap(
        &mut self,
        triangle_index: u32,
        heightmap: &[f32],
        width: u32,
        height: u32,
        uvs: [[f32; 2]; 3],
    ) {
        let num_vertices = self.format.num_vertices();
        let mut displacements = Vec::with_capacity(num_vertices as usize);

        // Generate barycentric coordinates for micro-vertices
        let subdiv = self.format.subdivision_level();
        for v in 0..=subdiv {
            for u in 0..=(subdiv - v) {
                let w = subdiv - u - v;
                let bary = [
                    u as f32 / subdiv as f32,
                    v as f32 / subdiv as f32,
                    w as f32 / subdiv as f32,
                ];

                // Interpolate UV
                let uv = [
                    bary[0] * uvs[0][0] + bary[1] * uvs[1][0] + bary[2] * uvs[2][0],
                    bary[0] * uvs[0][1] + bary[1] * uvs[1][1] + bary[2] * uvs[2][1],
                ];

                // Sample heightmap
                let x = (uv[0] * width as f32) as usize % width as usize;
                let y = (uv[1] * height as f32) as usize % height as usize;
                let h = heightmap[y * width as usize + x];

                displacements.push(h);
            }
        }

        self.triangles.push(TriangleDisplacementData {
            triangle_index,
            displacements,
            directions: None,
        });
    }

    /// Get build info.
    pub fn build_info(&self) -> DisplacementMicromapBuildInfo {
        DisplacementMicromapBuildInfo {
            triangle_count: self.triangles.len() as u32,
            format: self.format,
            vector_format: self.vector_format,
            scale: self.scale,
            bias: self.bias,
        }
    }

    /// Calculate required buffer size.
    pub fn calculate_buffer_size(&self) -> vk::DeviceSize {
        let vertices_per_tri = self.format.num_vertices();
        let bytes_per_vertex = match self.vector_format {
            DisplacementVectorFormat::Float2 => 8,
            DisplacementVectorFormat::Float3 => 12,
            DisplacementVectorFormat::Height => 4,
        };

        (self.triangles.len() as vk::DeviceSize) * (vertices_per_tri as vk::DeviceSize) * bytes_per_vertex
    }

    /// Serialize displacement data.
    pub fn serialize(&self) -> Vec<u8> {
        let mut data = Vec::new();

        for tri in &self.triangles {
            for disp in &tri.displacements {
                let scaled = (*disp - self.bias) / self.scale;
                data.extend_from_slice(&scaled.to_le_bytes());
            }
        }

        data
    }
}

/// GLSL code for displacement mapping fallback (non-RT).
pub mod glsl {
    /// Parallax occlusion mapping (software fallback).
    pub const PARALLAX_OCCLUSION: &str = r#"
// Parallax Occlusion Mapping
vec2 parallaxOcclusionMapping(
    vec2 uv,
    vec3 viewDirTangent,
    sampler2D heightMap,
    float scale,
    int numSteps
) {
    float stepSize = 1.0 / float(numSteps);
    float currentHeight = 1.0;
    vec2 deltaUV = viewDirTangent.xy * scale / (viewDirTangent.z * float(numSteps));

    vec2 currentUV = uv;
    float sampledHeight = texture(heightMap, currentUV).r;

    // Linear search
    while (currentHeight > sampledHeight) {
        currentUV -= deltaUV;
        sampledHeight = texture(heightMap, currentUV).r;
        currentHeight -= stepSize;
    }

    // Binary search refinement
    vec2 prevUV = currentUV + deltaUV;
    for (int i = 0; i < 5; i++) {
        vec2 midUV = (currentUV + prevUV) * 0.5;
        float midHeight = texture(heightMap, midUV).r;
        float midDepth = (currentHeight + stepSize) * 0.5;

        if (midHeight < midDepth) {
            currentUV = midUV;
            currentHeight = midDepth;
        } else {
            prevUV = midUV;
        }
    }

    return currentUV;
}
"#;

    /// Relief mapping (higher quality software fallback).
    pub const RELIEF_MAPPING: &str = r#"
// Relief Mapping with self-shadowing
vec2 reliefMapping(
    vec2 uv,
    vec3 viewDirTangent,
    sampler2D heightMap,
    float scale,
    int linearSteps,
    int binarySteps
) {
    // Normalize view direction
    vec3 V = normalize(viewDirTangent);

    // Calculate step size and offset
    float stepSize = 1.0 / float(linearSteps);
    vec2 deltaUV = V.xy * scale / V.z;
    deltaUV /= float(linearSteps);

    // Linear search
    float depth = 0.0;
    vec2 currentUV = uv;

    for (int i = 0; i < linearSteps; i++) {
        float h = texture(heightMap, currentUV).r;
        if (depth >= h) break;
        depth += stepSize;
        currentUV -= deltaUV;
    }

    // Binary search
    for (int i = 0; i < binarySteps; i++) {
        deltaUV *= 0.5;
        stepSize *= 0.5;

        float h = texture(heightMap, currentUV).r;
        if (depth >= h) {
            currentUV += deltaUV;
            depth -= stepSize;
        } else {
            currentUV -= deltaUV;
            depth += stepSize;
        }
    }

    return currentUV;
}

// Self-shadowing for relief mapping
float reliefShadow(
    vec2 uv,
    vec3 lightDirTangent,
    sampler2D heightMap,
    float scale,
    int steps
) {
    vec3 L = normalize(lightDirTangent);

    float height = texture(heightMap, uv).r;
    float stepSize = height / float(steps);
    vec2 deltaUV = L.xy * scale / L.z / float(steps);

    float currentHeight = height - stepSize;
    vec2 currentUV = uv + deltaUV;

    float shadow = 1.0;
    for (int i = 0; i < steps && currentHeight > 0.0; i++) {
        float h = texture(heightMap, currentUV).r;
        if (h > currentHeight) {
            shadow = 0.0;
            break;
        }
        currentHeight -= stepSize;
        currentUV += deltaUV;
    }

    return shadow;
}
"#;

    /// Displacement shader for ray tracing.
    pub const RT_DISPLACEMENT: &str = r#"
// Ray-heightfield intersection (for closest hit shader)
// Used when hardware displacement micromaps not available

struct DisplacementHit {
    vec3 position;
    vec3 normal;
    vec2 uv;
    bool hit;
};

DisplacementHit intersectDisplacementMap(
    vec3 rayOrigin,
    vec3 rayDir,
    mat3 tangentToWorld,
    vec2 uv0,
    vec2 uv1,
    vec2 uv2,
    vec3 bary,
    sampler2D heightMap,
    float scale,
    int steps
) {
    DisplacementHit result;
    result.hit = false;

    // Transform ray to tangent space
    mat3 worldToTangent = transpose(tangentToWorld);
    vec3 localOrigin = worldToTangent * rayOrigin;
    vec3 localDir = normalize(worldToTangent * rayDir);

    // Interpolate UV
    vec2 uv = bary.x * uv0 + bary.y * uv1 + bary.z * uv2;

    // March through height field
    float t = 0.0;
    float stepSize = scale / float(steps);

    for (int i = 0; i < steps; i++) {
        vec3 p = localOrigin + localDir * t;
        vec2 sampleUV = uv + p.xy * 0.1; // Adjust UV scale as needed

        float h = texture(heightMap, sampleUV).r * scale;

        if (p.z < h) {
            // Hit found - refine
            result.hit = true;
            result.position = tangentToWorld * p;
            result.uv = sampleUV;

            // Calculate normal from gradient
            vec2 texelSize = vec2(1.0) / textureSize(heightMap, 0);
            float hL = texture(heightMap, sampleUV - vec2(texelSize.x, 0.0)).r;
            float hR = texture(heightMap, sampleUV + vec2(texelSize.x, 0.0)).r;
            float hD = texture(heightMap, sampleUV - vec2(0.0, texelSize.y)).r;
            float hU = texture(heightMap, sampleUV + vec2(0.0, texelSize.y)).r;

            vec3 localNormal = normalize(vec3(hL - hR, hD - hU, 2.0 / scale));
            result.normal = tangentToWorld * localNormal;

            break;
        }

        t += stepSize;
    }

    return result;
}
"#;
}

/// LOD configuration for displacement.
#[derive(Debug, Clone)]
pub struct DisplacementLodConfig {
    /// Distance thresholds for LOD levels.
    pub lod_distances: [f32; 4],
    /// Subdivision levels per LOD.
    pub lod_subdivisions: [u32; 4],
    /// Blend range for LOD transitions.
    pub blend_range: f32,
}

impl Default for DisplacementLodConfig {
    fn default() -> Self {
        Self {
            lod_distances: [10.0, 25.0, 50.0, 100.0],
            lod_subdivisions: [5, 4, 3, 2],
            blend_range: 5.0,
        }
    }
}

impl DisplacementLodConfig {
    /// Get LOD level for distance.
    pub fn get_lod(&self, distance: f32) -> u32 {
        for (i, &d) in self.lod_distances.iter().enumerate() {
            if distance < d {
                return self.lod_subdivisions[i];
            }
        }
        self.lod_subdivisions[3]
    }

    /// Get blend factor for LOD transition.
    pub fn get_blend(&self, distance: f32) -> f32 {
        for (i, &d) in self.lod_distances.iter().enumerate() {
            let blend_start = d - self.blend_range;
            if distance < blend_start {
                return 0.0;
            }
            if distance < d {
                return (distance - blend_start) / self.blend_range;
            }
        }
        1.0
    }
}

/// Displacement micromap manager.
pub struct DisplacementManager {
    capabilities: DisplacementMicromapCapabilities,
    lod_config: DisplacementLodConfig,
}

impl DisplacementManager {
    /// Create new displacement manager.
    pub fn new(ctx: &super::context::VulkanContext) -> Self {
        let capabilities = query_capabilities(ctx);

        Self {
            capabilities,
            lod_config: DisplacementLodConfig::default(),
        }
    }

    /// Check if hardware displacement is supported.
    pub fn is_supported(&self) -> bool {
        self.capabilities.supported
    }

    /// Get capabilities.
    pub fn capabilities(&self) -> &DisplacementMicromapCapabilities {
        &self.capabilities
    }

    /// Set LOD configuration.
    pub fn set_lod_config(&mut self, config: DisplacementLodConfig) {
        self.lod_config = config;
    }

    /// Get LOD config.
    pub fn lod_config(&self) -> &DisplacementLodConfig {
        &self.lod_config
    }

    /// Create micromap builder with appropriate format for distance.
    pub fn create_builder(&self, distance: f32) -> DisplacementMicromapBuilder {
        let subdiv = self.lod_config.get_lod(distance);
        let format = match subdiv {
            5 => DisplacementFormat::Level1024,
            4 => DisplacementFormat::Level256,
            _ => DisplacementFormat::Level64,
        };

        DisplacementMicromapBuilder::new(format)
    }
}
