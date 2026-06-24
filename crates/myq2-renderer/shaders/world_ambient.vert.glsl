#version 450
// World ambient pass vertex shader.
// Renders the base texture at a low ambient level (no lightmap, no dynamic lights).
// This is the first color pass in the Doom 3 multi-pass rendering scheme.

layout(location = 0) in vec3 a_Position;
layout(location = 1) in vec2 a_TexCoord;
// Remaining attributes (lm_coord, lm_layer, normal, tangent) are present but unused.

layout(push_constant) uniform PushConstants {
    mat4  u_MVP;          // 64 bytes at offset 0
    float u_AmbientLevel; // 4 bytes at offset 64
} pc;

layout(location = 0) out vec2 v_TexCoord;

void main() {
    gl_Position = pc.u_MVP * vec4(a_Position, 1.0);
    v_TexCoord = a_TexCoord;
}
