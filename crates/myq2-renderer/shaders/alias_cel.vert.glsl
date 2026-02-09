#version 450

layout(location = 0) in vec3 a_Position;
layout(location = 1) in vec3 a_OldPosition;
layout(location = 2) in vec2 a_TexCoord;
layout(location = 3) in int a_NormalIndex;

// Per-draw uniforms (small data)
layout(std140, set = 3, binding = 0) uniform AliasCelUniforms {
    mat4 u_ModelViewProjection;
    mat4 u_ModelView;
    vec3 u_Move;
    float u_BackLerp;
    vec3 u_FrontV;
    float _pad0;
    vec3 u_BackV;
    float _pad1;
};

// Large array data (std140 pads vec3 to 16 bytes)
layout(std140, set = 3, binding = 1) uniform AliasCelArrays {
    vec3 u_VertexNormals[162];  // each padded to 16 bytes in std140
};

layout(location = 0) out vec2 v_TexCoord;
layout(location = 1) out vec3 v_Normal;

void main() {
    vec3 pos = u_Move + a_OldPosition * u_BackV + a_Position * u_FrontV;
    gl_Position = u_ModelViewProjection * vec4(pos, 1.0);
    v_TexCoord = a_TexCoord;
    v_Normal = mat3(u_ModelView) * u_VertexNormals[a_NormalIndex];
}
