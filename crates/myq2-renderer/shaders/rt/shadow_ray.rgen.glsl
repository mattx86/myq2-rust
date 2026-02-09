// shadow_ray.rgen.glsl - Shadow ray generation shader
// Traces shadow rays from world positions to light sources.
// Outputs a shadow mask texture for use in the world fragment shader.

#version 460
#extension GL_EXT_ray_tracing : require

layout(set = 0, binding = 0) uniform accelerationStructureEXT topLevelAS;
layout(set = 0, binding = 1, rgba8) uniform image2D shadowMask;
layout(set = 0, binding = 2) uniform sampler2D depthBuffer;
layout(set = 0, binding = 3) uniform sampler2D normalBuffer;

layout(set = 1, binding = 0) uniform CameraUBO {
    mat4 viewInverse;
    mat4 projInverse;
    vec3 cameraPos;
    float padding;
} camera;

layout(set = 1, binding = 1) uniform LightUBO {
    vec4 lightPositions[32];  // w = radius, 0 = unused
    vec4 lightColors[32];     // w = intensity
    int numLights;
    int padding[3];
} lights;

layout(location = 0) rayPayloadEXT float shadowPayload;

// Reconstruct world position from depth
vec3 reconstructWorldPos(vec2 uv, float depth) {
    vec4 clipPos = vec4(uv * 2.0 - 1.0, depth, 1.0);
    vec4 viewPos = camera.projInverse * clipPos;
    viewPos /= viewPos.w;
    vec4 worldPos = camera.viewInverse * viewPos;
    return worldPos.xyz;
}

void main() {
    ivec2 pixel = ivec2(gl_LaunchIDEXT.xy);
    ivec2 size = ivec2(gl_LaunchSizeEXT.xy);
    vec2 uv = (vec2(pixel) + 0.5) / vec2(size);

    // Sample depth and normal
    float depth = texture(depthBuffer, uv).r;
    vec3 normal = texture(normalBuffer, uv).rgb * 2.0 - 1.0;

    // Skip sky pixels (depth = 1.0)
    if (depth >= 1.0) {
        imageStore(shadowMask, pixel, vec4(1.0));
        return;
    }

    // Reconstruct world position
    vec3 worldPos = reconstructWorldPos(uv, depth);

    // Bias along normal to prevent self-shadowing
    vec3 origin = worldPos + normal * 0.01;

    // Accumulate shadow from all lights
    float totalShadow = 0.0;
    float totalWeight = 0.0;

    for (int i = 0; i < lights.numLights && i < 32; i++) {
        vec4 lightPos = lights.lightPositions[i];
        if (lightPos.w <= 0.0) continue;  // Unused light

        vec3 toLight = lightPos.xyz - origin;
        float lightDist = length(toLight);

        // Skip if beyond light radius
        if (lightDist > lightPos.w) continue;

        vec3 lightDir = toLight / lightDist;

        // Check if surface faces the light
        float NdotL = dot(normal, lightDir);
        if (NdotL <= 0.0) continue;

        // Trace shadow ray
        float tMin = 0.001;
        float tMax = lightDist - 0.01;

        shadowPayload = 1.0;  // Assume lit

        traceRayEXT(
            topLevelAS,
            gl_RayFlagsTerminateOnFirstHitEXT | gl_RayFlagsSkipClosestHitShaderEXT,
            0xFF,           // Cull mask
            0,              // SBT offset
            0,              // SBT stride
            0,              // Miss index
            origin,
            tMin,
            lightDir,
            tMax,
            0               // Payload location
        );

        // Weight by light intensity and attenuation
        float attenuation = 1.0 - (lightDist / lightPos.w);
        attenuation = attenuation * attenuation;
        float weight = lights.lightColors[i].w * attenuation * NdotL;

        totalShadow += shadowPayload * weight;
        totalWeight += weight;
    }

    // Normalize shadow factor
    float shadow = totalWeight > 0.0 ? totalShadow / totalWeight : 1.0;

    imageStore(shadowMask, pixel, vec4(shadow, shadow, shadow, 1.0));
}
