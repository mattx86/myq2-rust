#version 450

layout(location = 0) in vec3 a_Position;
layout(location = 1) in vec3 a_OldPosition;
layout(location = 2) in vec2 a_TexCoord;
layout(location = 3) in int a_NormalIndex;

// Per-draw uniforms (small data)
layout(std140, set = 3, binding = 0) uniform AliasUniforms {
    mat4 u_ModelViewProjection;
    vec3 u_Move;
    float u_BackLerp;
    vec3 u_FrontV;
    float u_ShellScale;
    vec3 u_BackV;
    int u_IsShell;      // bool not portable in std140; test as != 0
    vec3 u_ShadeLight;
    float _pad0;
};

// Large array data (std140 pads float/vec3 to 16 bytes)
layout(std140, set = 3, binding = 1) uniform AliasArrays {
    float u_ShadeDots[256];     // each padded to 16 bytes in std140
    vec3 u_VertexNormals[162];  // each padded to 16 bytes in std140
};

layout(location = 0) out vec2 v_TexCoord;
layout(location = 1) out vec3 v_Color;

void main() {
    // Interpolate position between frames (GL_LerpVerts equivalent)
    vec3 pos = u_Move + a_OldPosition * u_BackV + a_Position * u_FrontV;

    // Shell expansion along normal
    if (u_IsShell != 0) {
        vec3 normal = u_VertexNormals[a_NormalIndex];
        pos += normal * u_ShellScale;
    }

    gl_Position = u_ModelViewProjection * vec4(pos, 1.0);
    v_TexCoord = a_TexCoord;

    // Per-vertex lighting using shadedots lookup
    float lightDot = u_ShadeDots[a_NormalIndex];
    v_Color = lightDot * u_ShadeLight;
}
