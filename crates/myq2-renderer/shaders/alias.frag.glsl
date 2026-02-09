#version 450

layout(location = 0) in vec2 v_TexCoord;
layout(location = 1) in vec3 v_Color;

layout(set = 0, binding = 0) uniform sampler2D u_DiffuseTexture;

layout(std140, set = 3, binding = 0) uniform FragUniforms {
    float u_Alpha;
    int u_IsShell;
    float u_OverbrightScale;
};

layout(location = 0) out vec4 FragColor;

void main() {
    vec4 texColor;

    if (u_IsShell != 0) {
        // Shell effect: solid color, no texture
        texColor = vec4(1.0);
    } else {
        texColor = texture(u_DiffuseTexture, v_TexCoord);
    }

    vec3 finalColor = texColor.rgb * v_Color * u_OverbrightScale;
    FragColor = vec4(finalColor, texColor.a * u_Alpha);
}
