#version 450

layout(location = 0) in vec2 v_TexCoord;
layout(location = 1) in vec3 v_Color;
layout(location = 2) in float v_Alpha;

layout(set = 1, binding = 0) uniform sampler2D u_DiffuseTexture;

layout(location = 0) out vec4 FragColor;

void main() {
    vec4 texColor = texture(u_DiffuseTexture, v_TexCoord);
    FragColor = vec4(texColor.rgb * v_Color, texColor.a * v_Alpha);
}
