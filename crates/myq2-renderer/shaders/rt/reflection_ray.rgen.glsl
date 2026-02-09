// reflection_ray.rgen.glsl - Reflection ray generation shader
// Traces reflection rays from reflective surfaces (water, metal, glass).
// Outputs a reflection color texture to blend with rasterized output.

#version 460
#extension GL_EXT_ray_tracing : require

layout(set = 0, binding = 0) uniform accelerationStructureEXT topLevelAS;
layout(set = 0, binding = 1, rgba8) uniform image2D reflectionOutput;
layout(set = 0, binding = 2) uniform sampler2D depthBuffer;
layout(set = 0, binding = 3) uniform sampler2D normalBuffer;
layout(set = 0, binding = 4) uniform sampler2D materialBuffer;  // r = reflectivity

layout(set = 1, binding = 0) uniform CameraUBO {
    mat4 viewInverse;
    mat4 projInverse;
    vec3 cameraPos;
    float time;
} camera;

layout(set = 1, binding = 1) uniform EnvironmentUBO {
    vec4 skyColorTop;
    vec4 skyColorHorizon;
    vec4 ambientColor;
} environment;

struct ReflectionPayload {
    vec3 color;
    float hitDistance;
};

layout(location = 0) rayPayloadEXT ReflectionPayload payload;

// Reconstruct world position from depth
vec3 reconstructWorldPos(vec2 uv, float depth) {
    vec4 clipPos = vec4(uv * 2.0 - 1.0, depth, 1.0);
    vec4 viewPos = camera.projInverse * clipPos;
    viewPos /= viewPos.w;
    vec4 worldPos = camera.viewInverse * viewPos;
    return worldPos.xyz;
}

// Fresnel-Schlick approximation
float fresnelSchlick(float cosTheta, float F0) {
    return F0 + (1.0 - F0) * pow(1.0 - cosTheta, 5.0);
}

void main() {
    ivec2 pixel = ivec2(gl_LaunchIDEXT.xy);
    ivec2 size = ivec2(gl_LaunchSizeEXT.xy);
    vec2 uv = (vec2(pixel) + 0.5) / vec2(size);

    // Sample buffers
    float depth = texture(depthBuffer, uv).r;
    vec3 normal = normalize(texture(normalBuffer, uv).rgb * 2.0 - 1.0);
    float reflectivity = texture(materialBuffer, uv).r;

    // Skip non-reflective pixels or sky
    if (reflectivity < 0.01 || depth >= 1.0) {
        imageStore(reflectionOutput, pixel, vec4(0.0, 0.0, 0.0, 0.0));
        return;
    }

    // Reconstruct world position
    vec3 worldPos = reconstructWorldPos(uv, depth);

    // Calculate view direction and reflection
    vec3 viewDir = normalize(worldPos - camera.cameraPos);
    vec3 reflectDir = reflect(viewDir, normal);

    // Calculate Fresnel term
    float cosTheta = max(dot(-viewDir, normal), 0.0);
    float F0 = 0.04;  // Dielectric base reflectivity
    float fresnel = fresnelSchlick(cosTheta, F0) * reflectivity;

    // Bias origin along reflection direction to prevent self-intersection
    vec3 origin = worldPos + normal * 0.01;

    // Trace reflection ray
    float tMin = 0.001;
    float tMax = 10000.0;

    payload.color = vec3(0.0);
    payload.hitDistance = -1.0;

    traceRayEXT(
        topLevelAS,
        gl_RayFlagsNoneEXT,
        0xFF,           // Cull mask
        1,              // SBT offset (reflection hit group)
        0,              // SBT stride
        1,              // Miss index (reflection miss)
        origin,
        tMin,
        reflectDir,
        tMax,
        0               // Payload location
    );

    // Apply fresnel to reflected color
    vec3 reflectedColor = payload.color * fresnel;

    // Alpha encodes reflection strength for blending
    imageStore(reflectionOutput, pixel, vec4(reflectedColor, fresnel));
}
