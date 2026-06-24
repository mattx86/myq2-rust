#version 450
// Shadow volume vertex shader — position only.
// Used for z-fail stencil shadow volumes (Carmack's Reverse).
// No color output; only stencil is written by the pipeline.

layout(location = 0) in vec3 a_Position;

layout(push_constant) uniform PushConstants {
    mat4 u_MVP;  // 64 bytes
} pc;

void main() {
    gl_Position = pc.u_MVP * vec4(a_Position, 1.0);
}
