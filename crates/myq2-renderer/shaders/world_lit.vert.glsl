#version 450

layout(location = 0) in vec3 a_Position;
layout(location = 1) in vec2 a_TexCoord;
layout(location = 2) in vec2 a_LmCoord;
layout(location = 3) in float a_LmLayer;

layout(push_constant) uniform PushConstants {
    mat4  u_MVP;
    vec3  u_LightPos;
    float u_LightRadius;
    vec3  u_LightColor;
    float u_LightIntensity;
    float u_SpecPower;
    float u_ShadowBias;
    float _pad1;
    float _pad2;
    vec3  u_ViewOrigin;
} pc;

layout(location = 0) out vec3 v_WorldPos;
layout(location = 1) out vec3 v_Normal;
layout(location = 2) out vec2 v_TexCoord;
layout(location = 3) out vec3 v_LmCoordLayer;  // baked lightmap (u, v, layer) for dark-fill

void main() {
    gl_Position = pc.u_MVP * vec4(a_Position, 1.0);
    v_WorldPos  = a_Position;
    // BSP vertices don't have per-vertex normals in the base format;
    // use a placeholder — the diffuse lighting will dominate.
    v_Normal    = vec3(0.0, 0.0, 1.0);
    v_TexCoord  = a_TexCoord;
    v_LmCoordLayer = vec3(a_LmCoord, a_LmLayer);
}
