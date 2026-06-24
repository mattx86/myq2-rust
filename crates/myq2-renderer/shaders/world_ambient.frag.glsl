#version 450
// World ambient pass fragment shader.
// Writes the base texture color scaled by the global ambient level.
// No lightmap; no per-light contribution.
// This establishes the minimum illumination in unlit areas.

layout(location = 0) in vec2 v_TexCoord;

layout(set = 1, binding = 0) uniform sampler2D u_DiffuseTexture;

layout(push_constant) uniform PushConstants {
    mat4  u_MVP;
    float u_AmbientLevel;
} pc;

layout(location = 0) out vec4 FragColor;

void main() {
    vec4 diffTex = texture(u_DiffuseTexture, v_TexCoord);
    // Scale by ambient level — produces the dark base that lit passes add onto.
    FragColor = vec4(diffTex.rgb * pc.u_AmbientLevel, 1.0);
}
