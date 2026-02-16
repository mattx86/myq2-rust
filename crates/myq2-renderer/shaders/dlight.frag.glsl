#version 450

layout(location = 0) in vec3 v_Color;

layout(location = 0) out vec4 FragColor;

void main() {
    // Vertex color is interpolated by rasterizer: bright at center, black at edges.
    // This matches the original OpenGL GL_TRIANGLE_FAN with vertex colors.
    float alpha = max(v_Color.r, max(v_Color.g, v_Color.b));
    FragColor = vec4(v_Color, alpha);
}
