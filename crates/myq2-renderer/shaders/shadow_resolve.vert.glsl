#version 450

// Fullscreen triangle (no vertex buffer) — covers the screen with 3 vertices.
// Shared by the projective shadow-resolve pass.
layout(location = 0) out vec2 v_Uv;

void main() {
    vec2 uv = vec2((gl_VertexIndex << 1) & 2, gl_VertexIndex & 2);
    v_Uv = uv;
    gl_Position = vec4(uv * 2.0 - 1.0, 0.0, 1.0);
}
