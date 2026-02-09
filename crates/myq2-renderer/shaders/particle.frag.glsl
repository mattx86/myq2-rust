#version 450

layout(location = 0) in vec2 v_TexCoord;
layout(location = 1) in vec4 v_Color;

layout(set = 0, binding = 0) uniform sampler2D u_ParticleTexture;

layout(std140, set = 3, binding = 0) uniform FragUniforms {
    float u_OverbrightScale;
};

layout(location = 0) out vec4 FragColor;

void main() {
    vec4 texColor = texture(u_ParticleTexture, v_TexCoord);
    vec3 color = texColor.rgb * v_Color.rgb * u_OverbrightScale;
    FragColor = vec4(color, texColor.a * v_Color.a);
}
