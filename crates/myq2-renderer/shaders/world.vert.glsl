#version 450

layout(location = 0) in vec3 a_Position;
layout(location = 1) in vec2 a_TexCoord;
layout(location = 2) in vec2 a_LightmapCoord;

layout(push_constant) uniform PushConstants {
    mat4 u_ModelViewProjection;  // 64 bytes
    float u_Alpha;               // 4 bytes at offset 64 (used by fragment stage)
} pc;

layout(location = 0) out vec2 v_TexCoord;
layout(location = 1) out vec2 v_LightmapCoord;

void main() {
    gl_Position = pc.u_ModelViewProjection * vec4(a_Position, 1.0);
    v_TexCoord = a_TexCoord;
    v_LightmapCoord = a_LightmapCoord;
}
