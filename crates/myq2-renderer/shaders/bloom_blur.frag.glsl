#version 450

layout(location = 0) in vec2 v_TexCoord;

layout(set = 0, binding = 0) uniform sampler2D u_InputTexture;

layout(std140, set = 3, binding = 0) uniform FragUniforms {
    vec2 u_Direction; // (1,0) for horizontal, (0,1) for vertical
    vec2 u_TexelSize;
};

layout(location = 0) out vec4 FragColor;

void main() {
    // 9-tap Gaussian blur with pre-computed weights
    float weights[5] = float[](0.227027, 0.1945946, 0.1216216, 0.054054, 0.016216);

    vec3 result = texture(u_InputTexture, v_TexCoord).rgb * weights[0];

    for (int i = 1; i < 5; ++i) {
        vec2 offset = u_Direction * u_TexelSize * float(i);
        result += texture(u_InputTexture, v_TexCoord + offset).rgb * weights[i];
        result += texture(u_InputTexture, v_TexCoord - offset).rgb * weights[i];
    }

    FragColor = vec4(result, 1.0);
}
