#version 450

layout(location = 0) in vec2 v_TexCoord;

layout(set = 1, binding = 0) uniform sampler2D u_SkyTexture;

layout(location = 0) out vec4 FragColor;

void main() {
    FragColor = texture(u_SkyTexture, v_TexCoord);
}
