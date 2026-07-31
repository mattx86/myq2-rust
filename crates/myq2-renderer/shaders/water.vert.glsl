#version 450

layout(location = 0) in vec3 a_Position;
layout(location = 1) in vec2 a_TexCoord;
layout(location = 2) in vec2 a_LightmapCoord;
layout(location = 3) in float a_LightmapLayer;  // lightmap array layer (-1 = no lightmap)

layout(push_constant) uniform PushConstants {
    mat4 u_ModelViewProjection;  // 64 bytes
    float u_Alpha;               // 4 bytes at offset 64
    float u_Time;                // 4 bytes at offset 68
    float u_Scroll;              // 4 bytes at offset 72
    // Lightmap parameters (match world.frag.glsl so liquids are lit identically)
    float u_OverbrightScale;     // 76
    float u_LightmapGamma;       // 80
    float u_LightmapContrast;    // 84
    float u_ShadowLift;          // 88
    float u_LightmapScale;       // 92
};

layout(location = 0) out vec2 v_TexCoord;
layout(location = 1) out vec3 v_LightmapCoordLayer;  // (u, v, layer)
layout(location = 2) out vec3 v_WorldPos;            // world-space position (Fresnel for reflections)

void main() {
    gl_Position = u_ModelViewProjection * vec4(a_Position, 1.0);
    v_TexCoord = a_TexCoord;
    v_LightmapCoordLayer = vec3(a_LightmapCoord, a_LightmapLayer);
    v_WorldPos = a_Position;   // BSP verts are already in world space
}
