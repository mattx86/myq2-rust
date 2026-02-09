// reflection.rmiss.glsl - Reflection ray miss shader
// Returns sky color when reflection ray doesn't hit anything.

#version 460
#extension GL_EXT_ray_tracing : require

struct ReflectionPayload {
    vec3 color;
    float hitDistance;
};

layout(location = 0) rayPayloadInEXT ReflectionPayload payload;

layout(set = 1, binding = 1) uniform EnvironmentUBO {
    vec4 skyColorTop;
    vec4 skyColorHorizon;
    vec4 ambientColor;
} environment;

void main() {
    // Calculate sky color based on ray direction
    vec3 dir = normalize(gl_WorldRayDirectionEXT);

    // Vertical gradient: horizon to zenith
    float t = max(dir.z, 0.0);  // Z is up in Quake 2
    t = sqrt(t);  // Ease the gradient

    vec3 skyColor = mix(environment.skyColorHorizon.rgb, environment.skyColorTop.rgb, t);

    payload.color = skyColor;
    payload.hitDistance = -1.0;  // Indicates miss (sky)
}
