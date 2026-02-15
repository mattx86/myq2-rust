#version 450

layout(location = 0) in vec3 v_Position;

layout(push_constant) uniform PushConstants {
    mat4 u_ModelViewProjection;  // 64 bytes (vertex stage)
    vec3 u_LightOrigin;          // 12 bytes
    float u_LightRadius;         // 4 bytes
    vec3 u_LightColor;           // 12 bytes
    float _pad;                  // 4 bytes
} pc;                            // Total: 96 bytes

layout(location = 0) out vec4 FragColor;

void main() {
    float dist = length(v_Position - pc.u_LightOrigin);
    float attenuation = 1.0 - clamp(dist / pc.u_LightRadius, 0.0, 1.0);
    attenuation = attenuation * attenuation;  // Quadratic falloff
    vec3 color = pc.u_LightColor * 0.2 * attenuation;
    FragColor = vec4(color, attenuation);
}
