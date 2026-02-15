#version 450

layout(location = 0) out vec2 v_TexCoord;

void main() {
    // Generate oversized fullscreen triangle from vertex index (no VBO needed).
    // Vertex 0: (-1, -1), Vertex 1: (3, -1), Vertex 2: (-1, 3)
    // The oversized triangle is clipped to the viewport, covering the full screen.
    vec2 pos = vec2((gl_VertexIndex << 1) & 2, gl_VertexIndex & 2);
    v_TexCoord = pos;
    gl_Position = vec4(pos * 2.0 - 1.0, 0.0, 1.0);
}
