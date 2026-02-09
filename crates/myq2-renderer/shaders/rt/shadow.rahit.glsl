// shadow.rahit.glsl - Shadow ray any-hit shader
// Handles alpha-tested geometry (fences, grates, etc.)

#version 460
#extension GL_EXT_ray_tracing : require
#extension GL_EXT_nonuniform_qualifier : require

// Bindless textures for alpha testing
layout(set = 2, binding = 0) uniform sampler2D textures[];

// Per-instance data
layout(set = 2, binding = 1) buffer InstanceData {
    uint textureIndex[];
} instances;

// Hit attributes from intersection
hitAttributeEXT vec2 attribs;

layout(location = 0) rayPayloadInEXT float shadowPayload;

void main() {
    // Get instance index for texture lookup
    uint instanceIndex = gl_InstanceCustomIndexEXT;
    uint texIdx = instances.textureIndex[instanceIndex];

    // Sample texture at hit point using barycentric coordinates
    // For simplicity, use a fixed UV (real implementation would interpolate)
    vec2 uv = attribs;
    vec4 texColor = texture(textures[nonuniformEXT(texIdx)], uv);

    // Alpha test - if transparent, ignore this hit
    if (texColor.a < 0.5) {
        ignoreIntersectionEXT;
    }

    // Otherwise, we hit an opaque surface - in shadow
    shadowPayload = 0.0;
    terminateRayEXT;
}
