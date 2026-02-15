#version 450

layout(location = 0) in vec2 v_TexCoord;
layout(location = 1) in vec2 v_LightmapCoord;

layout(set = 1, binding = 0) uniform sampler2D u_DiffuseTexture;

layout(push_constant) uniform PushConstants {
    mat4 u_ModelViewProjection;  // 64 bytes (vertex stage, not used here)
    float u_Alpha;               // 4 bytes at offset 64
} pc;

layout(location = 0) out vec4 FragColor;

void main() {
    vec4 texColor = texture(u_DiffuseTexture, v_TexCoord);
    FragColor = vec4(texColor.rgb, texColor.a * pc.u_Alpha);
}
