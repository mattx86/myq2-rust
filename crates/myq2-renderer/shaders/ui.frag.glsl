#version 450

layout(location = 0) in vec2 v_TexCoord;
layout(location = 1) in vec4 v_Color;

layout(set = 0, binding = 0) uniform sampler2D u_Texture;

layout(std140, set = 3, binding = 0) uniform FragUniforms {
    int u_UseTexture;
    int u_AlphaTest;
};

layout(location = 0) out vec4 FragColor;

void main() {
    vec4 texColor = (u_UseTexture != 0) ? texture(u_Texture, v_TexCoord) : vec4(1.0);

    // Alpha test (replaces GL_ALPHA_TEST)
    if ((u_AlphaTest != 0) && texColor.a < 0.5) {
        discard;
    }

    FragColor = texColor * v_Color;
}
