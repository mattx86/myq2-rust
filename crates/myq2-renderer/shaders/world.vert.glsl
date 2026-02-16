#version 450

layout(location = 0) in vec3 a_Position;
layout(location = 1) in vec2 a_TexCoord;
layout(location = 2) in vec2 a_LightmapCoord;
layout(location = 3) in float a_LightmapLayer;  // lightmap array layer index (-1 = no lightmap)

layout(push_constant) uniform PushConstants {
    mat4 u_ModelViewProjection;  // 64 bytes
    float u_Alpha;               // 4 bytes at offset 64 (used by fragment stage)
} pc;

layout(location = 0) out vec2 v_TexCoord;
layout(location = 1) out vec3 v_LightmapCoordLayer;  // (u, v, layer)

void main() {
    gl_Position = pc.u_ModelViewProjection * vec4(a_Position, 1.0);
    v_TexCoord = a_TexCoord;
    v_LightmapCoordLayer = vec3(a_LightmapCoord, a_LightmapLayer);
}
