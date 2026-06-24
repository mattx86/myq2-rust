#version 450
// World depth prepass vertex shader.
// Writes only depth; no color output.
// Inputs match BspVertex layout (stride 60), but only position is used.

layout(location = 0) in vec3 a_Position;
// Other attributes (tex_coord, lm_coord, lm_layer, normal, tangent) are
// present in the vertex buffer but unused here.

layout(push_constant) uniform PushConstants {
    mat4 u_MVP;  // 64 bytes
} pc;

void main() {
    gl_Position = pc.u_MVP * vec4(a_Position, 1.0);
}
