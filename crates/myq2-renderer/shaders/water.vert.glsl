#version 450

layout(location = 0) in vec3 a_Position;
layout(location = 1) in vec2 a_TexCoord;
// location 2 (lm_coord) present in vertex data but not used by water shader
// location 3 (color) present in vertex data but not used by water shader

layout(push_constant) uniform PushConstants {
    mat4 u_ModelViewProjection;  // 64 bytes
    float u_Alpha;               // 4 bytes at offset 64
    float u_Time;                // 4 bytes at offset 68
    float u_Scroll;              // 4 bytes at offset 72
};

layout(location = 0) out vec2 v_TexCoord;

void main() {
    gl_Position = u_ModelViewProjection * vec4(a_Position, 1.0);
    v_TexCoord = a_TexCoord;
}
