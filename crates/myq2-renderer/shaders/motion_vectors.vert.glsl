// motion_vectors.vert.glsl - Motion Vector Generation Vertex Shader
// Transforms vertices using both current and previous view-projection matrices
// for per-pixel motion vector calculation in the fragment shader.

#version 450

layout(location = 0) in vec3 a_Position;
layout(location = 1) in vec2 a_TexCoord;

layout(std140, set = 0, binding = 0) uniform PerFrameUniforms {
    mat4 u_ViewProjection;
    mat4 u_PrevViewProjection;
    mat4 u_ModelMatrix;
    mat4 u_PrevModelMatrix;
};

layout(location = 0) out vec2 v_TexCoord;
layout(location = 1) out vec4 v_CurrentClipPos;
layout(location = 2) out vec4 v_PrevClipPos;

void main() {
    vec4 worldPos = u_ModelMatrix * vec4(a_Position, 1.0);
    vec4 prevWorldPos = u_PrevModelMatrix * vec4(a_Position, 1.0);

    v_CurrentClipPos = u_ViewProjection * worldPos;
    v_PrevClipPos = u_PrevViewProjection * prevWorldPos;

    gl_Position = v_CurrentClipPos;
    v_TexCoord = a_TexCoord;
}
