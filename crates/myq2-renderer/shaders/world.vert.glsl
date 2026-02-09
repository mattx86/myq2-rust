#version 450

layout(location = 0) in vec3 a_Position;
layout(location = 1) in vec2 a_TexCoord;
layout(location = 2) in vec2 a_LightmapCoord;

layout(std140, set = 3, binding = 0) uniform WorldUniforms {
    mat4 u_ModelViewProjection;
    float u_ScrollOffset;  // For flowing textures (0 for static)
};

layout(location = 0) out vec2 v_TexCoord;
layout(location = 1) out vec2 v_LightmapCoord;

void main() {
    gl_Position = u_ModelViewProjection * vec4(a_Position, 1.0);
    v_TexCoord = a_TexCoord + vec2(u_ScrollOffset, 0.0);
    v_LightmapCoord = a_LightmapCoord;
}
