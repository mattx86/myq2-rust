#version 450

layout(location = 0) in vec2 v_TexCoord;

layout(set = 0, binding = 0) uniform sampler2D u_SsaoTexture;

layout(std140, set = 3, binding = 0) uniform FragUniforms {
    vec2 u_TexelSize;
};

layout(location = 0) out vec4 FragColor;

void main() {
    float result = 0.0;
    for (int x = -2; x <= 2; ++x) {
        for (int y = -2; y <= 2; ++y) {
            vec2 offset = vec2(float(x), float(y)) * u_TexelSize;
            result += texture(u_SsaoTexture, v_TexCoord + offset).r;
        }
    }
    result /= 25.0;
    FragColor = vec4(vec3(result), 1.0);
}
