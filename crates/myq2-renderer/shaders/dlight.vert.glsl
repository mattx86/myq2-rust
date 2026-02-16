#version 450

layout(location = 0) in vec3 a_Position;
layout(location = 1) in vec3 a_Color;

layout(push_constant) uniform PushConstants {
    mat4 u_ModelViewProjection;  // 64 bytes
    vec3 u_LightOrigin;          // 12 bytes
    float u_LightRadius;         // 4 bytes
    vec3 u_LightColor;           // 12 bytes
    float _pad;                  // 4 bytes
} pc;                            // Total: 96 bytes

layout(location = 0) out vec3 v_Color;

void main() {
    gl_Position = pc.u_ModelViewProjection * vec4(a_Position, 1.0);
    v_Color = a_Color;
}
