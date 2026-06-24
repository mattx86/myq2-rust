#version 450

layout(location = 0) in vec3 v_Color;

layout(location = 0) out vec4 FragColor;

void main() {
    // Vertex color is interpolated by rasterizer: bright at center, black at edges.
    // This matches the original OpenGL GL_TRIANGLE_FAN with vertex colors.
    // alpha=1.0 matches original Q2 GL: glColor3f sets no alpha → alpha defaults to 1.0.
    // Blend mode is (src_alpha, ONE): contribution = v_Color * 1.0 (fully additive).
    FragColor = vec4(v_Color, 1.0);
}
