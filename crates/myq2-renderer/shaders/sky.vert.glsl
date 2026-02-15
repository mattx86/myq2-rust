#version 450

layout(location = 0) in vec3 a_Position;
layout(location = 1) in vec2 a_TexCoord;
// location 2 (lm_coord) present in vertex data but not used by shader

layout(push_constant) uniform PushConstants {
    mat4 u_ModelViewProjection;
} pc;

layout(location = 0) out vec2 v_TexCoord;

void main() {
    vec4 pos = pc.u_ModelViewProjection * vec4(a_Position, 1.0);
    gl_Position = pos.xyww;  // Force depth to 1.0 (far plane)
    v_TexCoord = a_TexCoord;
}
