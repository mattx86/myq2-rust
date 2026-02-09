// reflection.rchit.glsl - Reflection closest-hit shader
// Samples the texture at the hit point and returns the color.

#version 460
#extension GL_EXT_ray_tracing : require
#extension GL_EXT_nonuniform_qualifier : require

// Bindless textures
layout(set = 2, binding = 0) uniform sampler2D textures[];

// Per-instance data with texture indices and transforms
layout(set = 2, binding = 1) buffer InstanceData {
    mat4 transforms[];
} instances;

layout(set = 2, binding = 2) buffer TextureIndices {
    uint textureIndex[];
} texIndices;

// Vertex data (interleaved position, normal, UV)
layout(set = 2, binding = 3) buffer VertexBuffer {
    float vertices[];
} vertexData;

layout(set = 2, binding = 4) buffer IndexBuffer {
    uint indices[];
} indexData;

struct ReflectionPayload {
    vec3 color;
    float hitDistance;
};

layout(location = 0) rayPayloadInEXT ReflectionPayload payload;

hitAttributeEXT vec2 attribs;

// Light info for basic shading
layout(set = 1, binding = 1) uniform EnvironmentUBO {
    vec4 skyColorTop;
    vec4 skyColorHorizon;
    vec4 ambientColor;
} environment;

void main() {
    // Get hit instance and primitive
    uint instanceIndex = gl_InstanceCustomIndexEXT;
    uint primitiveIndex = gl_PrimitiveID;
    uint texIdx = texIndices.textureIndex[instanceIndex];

    // Barycentric coordinates
    vec3 barycentrics = vec3(1.0 - attribs.x - attribs.y, attribs.x, attribs.y);

    // Get vertex indices for this triangle
    uint i0 = indexData.indices[primitiveIndex * 3 + 0];
    uint i1 = indexData.indices[primitiveIndex * 3 + 1];
    uint i2 = indexData.indices[primitiveIndex * 3 + 2];

    // Vertex stride: pos(3) + normal(3) + uv(2) = 8 floats
    const uint stride = 8;

    // Interpolate UV coordinates
    vec2 uv0 = vec2(vertexData.vertices[i0 * stride + 6], vertexData.vertices[i0 * stride + 7]);
    vec2 uv1 = vec2(vertexData.vertices[i1 * stride + 6], vertexData.vertices[i1 * stride + 7]);
    vec2 uv2 = vec2(vertexData.vertices[i2 * stride + 6], vertexData.vertices[i2 * stride + 7]);
    vec2 hitUV = uv0 * barycentrics.x + uv1 * barycentrics.y + uv2 * barycentrics.z;

    // Interpolate normal
    vec3 n0 = vec3(vertexData.vertices[i0 * stride + 3], vertexData.vertices[i0 * stride + 4], vertexData.vertices[i0 * stride + 5]);
    vec3 n1 = vec3(vertexData.vertices[i1 * stride + 3], vertexData.vertices[i1 * stride + 4], vertexData.vertices[i1 * stride + 5]);
    vec3 n2 = vec3(vertexData.vertices[i2 * stride + 3], vertexData.vertices[i2 * stride + 4], vertexData.vertices[i2 * stride + 5]);
    vec3 hitNormal = normalize(n0 * barycentrics.x + n1 * barycentrics.y + n2 * barycentrics.z);

    // Transform normal to world space
    mat4 instanceTransform = instances.transforms[instanceIndex];
    hitNormal = normalize((instanceTransform * vec4(hitNormal, 0.0)).xyz);

    // Sample texture
    vec4 texColor = texture(textures[nonuniformEXT(texIdx)], hitUV);

    // Basic diffuse lighting (use ray direction as light direction proxy)
    vec3 lightDir = -gl_WorldRayDirectionEXT;
    float NdotL = max(dot(hitNormal, lightDir), 0.0);

    // Combine texture with basic lighting
    vec3 color = texColor.rgb * (environment.ambientColor.rgb + NdotL * 0.5);

    payload.color = color;
    payload.hitDistance = gl_HitTEXT;
}
