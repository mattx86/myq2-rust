#version 450

layout(set = 1, binding = 0) uniform sampler2D u_Texture;

layout(location = 0) in vec2 v_TexCoord;
layout(location = 1) in vec4 v_Color;

layout(location = 0) out vec4 FragColor;

void main() {
    FragColor = texture(u_Texture, v_TexCoord) * v_Color;
}
