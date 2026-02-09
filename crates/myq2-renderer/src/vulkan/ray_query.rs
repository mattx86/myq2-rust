//! Ray Query for Inline Ray Tracing
//!
//! VK_KHR_ray_query allows ray tracing queries in any shader stage:
//! - Inline ray tracing without separate RT pipeline
//! - Use in fragment/compute shaders for shadows
//! - Flexible intersection testing
//! - Simpler integration than full RT pipeline

use ash::vk;

/// Ray query capabilities.
#[derive(Debug, Clone, Default)]
pub struct RayQueryCapabilities {
    /// Whether ray query is supported.
    pub supported: bool,
    /// Whether acceleration structures are available.
    pub acceleration_structure_available: bool,
}

/// Query ray query capabilities.
pub fn query_capabilities(ctx: &super::context::VulkanContext) -> RayQueryCapabilities {
    let mut ray_query_features = vk::PhysicalDeviceRayQueryFeaturesKHR::default();
    let mut features2 = vk::PhysicalDeviceFeatures2::default()
        .push_next(&mut ray_query_features);

    unsafe {
        ctx.instance.get_physical_device_features2(ctx.physical_device, &mut features2);
    }

    let _ = features2;
    let supported = ray_query_features.ray_query == vk::TRUE;

    // Check for acceleration structure support
    let mut accel_features = vk::PhysicalDeviceAccelerationStructureFeaturesKHR::default();
    let mut features2_accel = vk::PhysicalDeviceFeatures2::default()
        .push_next(&mut accel_features);

    unsafe {
        ctx.instance.get_physical_device_features2(ctx.physical_device, &mut features2_accel);
    }

    let _ = features2_accel;
    let acceleration_structure_available = accel_features.acceleration_structure == vk::TRUE;

    RayQueryCapabilities {
        supported,
        acceleration_structure_available,
    }
}

/// Ray flags for ray query.
#[derive(Debug, Clone, Copy, Default)]
pub struct RayFlags {
    /// Treat all geometry as opaque.
    pub force_opaque: bool,
    /// Treat all geometry as non-opaque.
    pub force_non_opaque: bool,
    /// Terminate on first hit.
    pub terminate_on_first_hit: bool,
    /// Skip closest hit shader.
    pub skip_closest_hit_shader: bool,
    /// Cull back facing triangles.
    pub cull_back_facing: bool,
    /// Cull front facing triangles.
    pub cull_front_facing: bool,
    /// Cull opaque geometry.
    pub cull_opaque: bool,
    /// Cull non-opaque geometry.
    pub cull_non_opaque: bool,
    /// Skip triangles.
    pub skip_triangles: bool,
    /// Skip AABBs.
    pub skip_aabbs: bool,
}

impl RayFlags {
    /// Flags for shadow rays (terminate on any hit).
    pub fn shadow() -> Self {
        Self {
            force_opaque: true,
            terminate_on_first_hit: true,
            skip_closest_hit_shader: true,
            ..Default::default()
        }
    }

    /// Flags for visibility testing.
    pub fn visibility() -> Self {
        Self {
            terminate_on_first_hit: true,
            ..Default::default()
        }
    }

    /// Flags for closest hit queries.
    pub fn closest_hit() -> Self {
        Self::default()
    }

    /// Convert to GLSL flags expression.
    pub fn to_glsl(&self) -> String {
        let mut flags = Vec::new();

        if self.force_opaque {
            flags.push("gl_RayFlagsOpaqueEXT");
        }
        if self.force_non_opaque {
            flags.push("gl_RayFlagsNoOpaqueEXT");
        }
        if self.terminate_on_first_hit {
            flags.push("gl_RayFlagsTerminateOnFirstHitEXT");
        }
        if self.skip_closest_hit_shader {
            flags.push("gl_RayFlagsSkipClosestHitShaderEXT");
        }
        if self.cull_back_facing {
            flags.push("gl_RayFlagsCullBackFacingTrianglesEXT");
        }
        if self.cull_front_facing {
            flags.push("gl_RayFlagsCullFrontFacingTrianglesEXT");
        }
        if self.cull_opaque {
            flags.push("gl_RayFlagsCullOpaqueEXT");
        }
        if self.cull_non_opaque {
            flags.push("gl_RayFlagsCullNoOpaqueEXT");
        }
        if self.skip_triangles {
            flags.push("gl_RayFlagsSkipTrianglesEXT");
        }
        if self.skip_aabbs {
            flags.push("gl_RayFlagsSkipAABBEXT");
        }

        if flags.is_empty() {
            "gl_RayFlagsNoneEXT".to_string()
        } else {
            flags.join(" | ")
        }
    }
}

/// GLSL code snippets for ray query.
pub mod glsl {
    /// Required extensions.
    pub const EXTENSIONS: &str = r#"
#extension GL_EXT_ray_query : require
#extension GL_EXT_ray_tracing : enable
"#;

    /// Acceleration structure uniform declaration.
    pub const ACCEL_STRUCT_BINDING: &str =
        "layout(binding = 0, set = 0) uniform accelerationStructureEXT topLevelAS;";

    /// Basic ray query for shadow testing.
    pub const SHADOW_RAY_QUERY: &str = r#"
// Test if a point is in shadow
bool isInShadow(vec3 origin, vec3 direction, float maxDist) {
    rayQueryEXT rayQuery;

    rayQueryInitializeEXT(rayQuery, topLevelAS,
        gl_RayFlagsTerminateOnFirstHitEXT | gl_RayFlagsOpaqueEXT,
        0xFF,           // Cull mask
        origin,
        0.001,          // tMin
        direction,
        maxDist);       // tMax

    // Traverse the acceleration structure
    while (rayQueryProceedEXT(rayQuery)) {
        // For opaque geometry, we don't need to handle intersections
    }

    // Check if we hit anything
    return rayQueryGetIntersectionTypeEXT(rayQuery, true) != gl_RayQueryCommittedIntersectionNoneEXT;
}
"#;

    /// Ray query for closest hit with hit info.
    pub const CLOSEST_HIT_QUERY: &str = r#"
struct HitInfo {
    bool hit;
    float t;
    vec2 barycentrics;
    int instanceId;
    int primitiveId;
    int geometryIndex;
    bool frontFace;
    mat4x3 objectToWorld;
    mat4x3 worldToObject;
};

// Find closest intersection
HitInfo traceRay(vec3 origin, vec3 direction, float tMin, float tMax) {
    HitInfo info;
    info.hit = false;

    rayQueryEXT rayQuery;

    rayQueryInitializeEXT(rayQuery, topLevelAS,
        gl_RayFlagsNoneEXT,
        0xFF,
        origin,
        tMin,
        direction,
        tMax);

    while (rayQueryProceedEXT(rayQuery)) {
        if (rayQueryGetIntersectionTypeEXT(rayQuery, false) == gl_RayQueryCandidateIntersectionTriangleEXT) {
            rayQueryConfirmIntersectionEXT(rayQuery);
        }
    }

    if (rayQueryGetIntersectionTypeEXT(rayQuery, true) == gl_RayQueryCommittedIntersectionTriangleEXT) {
        info.hit = true;
        info.t = rayQueryGetIntersectionTEXT(rayQuery, true);
        info.barycentrics = rayQueryGetIntersectionBarycentricsEXT(rayQuery, true);
        info.instanceId = rayQueryGetIntersectionInstanceIdEXT(rayQuery, true);
        info.primitiveId = rayQueryGetIntersectionPrimitiveIndexEXT(rayQuery, true);
        info.geometryIndex = rayQueryGetIntersectionGeometryIndexEXT(rayQuery, true);
        info.frontFace = rayQueryGetIntersectionFrontFaceEXT(rayQuery, true);
        info.objectToWorld = rayQueryGetIntersectionObjectToWorldEXT(rayQuery, true);
        info.worldToObject = rayQueryGetIntersectionWorldToObjectEXT(rayQuery, true);
    }

    return info;
}
"#;

    /// Ambient occlusion using ray query.
    pub const AMBIENT_OCCLUSION: &str = r#"
// Compute ambient occlusion at a point
float computeAO(vec3 position, vec3 normal, float radius, int sampleCount) {
    float occlusion = 0.0;

    // Generate sample directions in hemisphere
    for (int i = 0; i < sampleCount; i++) {
        // Cosine-weighted hemisphere sampling
        float u1 = float(i) / float(sampleCount);
        float u2 = fract(u1 * 12.9898 + 78.233);

        float r = sqrt(u1);
        float theta = 2.0 * 3.14159 * u2;

        vec3 tangent = normalize(cross(normal, vec3(0.0, 1.0, 0.0)));
        if (abs(dot(normal, vec3(0.0, 1.0, 0.0))) > 0.99) {
            tangent = normalize(cross(normal, vec3(1.0, 0.0, 0.0)));
        }
        vec3 bitangent = cross(normal, tangent);

        vec3 sampleDir = tangent * (r * cos(theta)) +
                         bitangent * (r * sin(theta)) +
                         normal * sqrt(1.0 - u1);

        if (isInShadow(position + normal * 0.001, sampleDir, radius)) {
            occlusion += 1.0;
        }
    }

    return 1.0 - (occlusion / float(sampleCount));
}
"#;

    /// Reflection ray query.
    pub const REFLECTION_QUERY: &str = r#"
// Trace a reflection ray
vec3 traceReflection(vec3 position, vec3 normal, vec3 viewDir, float roughness) {
    vec3 reflectDir = reflect(-viewDir, normal);

    // Add roughness perturbation
    if (roughness > 0.0) {
        // Simple box-muller for random perturbation
        float u1 = fract(sin(dot(position.xy, vec2(12.9898, 78.233))) * 43758.5453);
        float u2 = fract(sin(dot(position.yz, vec2(12.9898, 78.233))) * 43758.5453);

        float theta = 2.0 * 3.14159 * u1;
        float phi = acos(pow(u2, 1.0 / (1.0 + roughness * 100.0)));

        vec3 tangent = normalize(cross(reflectDir, vec3(0.0, 1.0, 0.0)));
        vec3 bitangent = cross(reflectDir, tangent);

        reflectDir = normalize(
            reflectDir * cos(phi) +
            tangent * sin(phi) * cos(theta) +
            bitangent * sin(phi) * sin(theta)
        );
    }

    HitInfo hit = traceRay(position + normal * 0.001, reflectDir, 0.0, 1000.0);

    if (hit.hit) {
        // Return hit position for further shading
        return position + reflectDir * hit.t;
    }

    // Return sky direction
    return position + reflectDir * 10000.0;
}
"#;

    /// Complete shadow fragment shader example.
    pub const SHADOW_FRAGMENT_SHADER: &str = r#"
#version 460
#extension GL_EXT_ray_query : require

layout(binding = 0, set = 0) uniform accelerationStructureEXT topLevelAS;

layout(binding = 1) uniform LightData {
    vec3 lightPos;
    vec3 lightColor;
    float lightIntensity;
} light;

layout(location = 0) in vec3 fragPos;
layout(location = 1) in vec3 fragNormal;
layout(location = 2) in vec2 fragUV;

layout(location = 0) out vec4 outColor;

void main() {
    vec3 N = normalize(fragNormal);
    vec3 L = normalize(light.lightPos - fragPos);
    float dist = length(light.lightPos - fragPos);

    // Shadow ray
    rayQueryEXT rayQuery;
    rayQueryInitializeEXT(rayQuery, topLevelAS,
        gl_RayFlagsTerminateOnFirstHitEXT | gl_RayFlagsOpaqueEXT,
        0xFF, fragPos + N * 0.001, 0.0, L, dist);

    while (rayQueryProceedEXT(rayQuery)) {}

    float shadow = 1.0;
    if (rayQueryGetIntersectionTypeEXT(rayQuery, true) != gl_RayQueryCommittedIntersectionNoneEXT) {
        shadow = 0.2;  // In shadow
    }

    // Simple diffuse lighting
    float NdotL = max(dot(N, L), 0.0);
    float attenuation = 1.0 / (dist * dist);
    vec3 diffuse = light.lightColor * light.lightIntensity * NdotL * attenuation * shadow;

    outColor = vec4(diffuse + vec3(0.03), 1.0);  // Add ambient
}
"#;
}

/// Configuration for ray query usage.
#[derive(Debug, Clone)]
pub struct RayQueryConfig {
    /// Ray flags to use.
    pub flags: RayFlags,
    /// Cull mask.
    pub cull_mask: u8,
    /// Minimum ray distance.
    pub t_min: f32,
    /// Maximum ray distance.
    pub t_max: f32,
}

impl Default for RayQueryConfig {
    fn default() -> Self {
        Self {
            flags: RayFlags::default(),
            cull_mask: 0xFF,
            t_min: 0.001,
            t_max: 10000.0,
        }
    }
}

impl RayQueryConfig {
    /// Config for shadow rays.
    pub fn shadow(max_distance: f32) -> Self {
        Self {
            flags: RayFlags::shadow(),
            t_max: max_distance,
            ..Default::default()
        }
    }

    /// Config for ambient occlusion.
    pub fn ambient_occlusion(radius: f32) -> Self {
        Self {
            flags: RayFlags {
                terminate_on_first_hit: true,
                force_opaque: true,
                ..Default::default()
            },
            t_max: radius,
            ..Default::default()
        }
    }

    /// Config for reflections.
    pub fn reflection() -> Self {
        Self {
            flags: RayFlags::closest_hit(),
            ..Default::default()
        }
    }
}
