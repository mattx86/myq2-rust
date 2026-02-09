#version 450

layout(location = 0) in vec2 v_TexCoord;

layout(set = 0, binding = 0) uniform sampler2D u_SceneTexture;

layout(std140, set = 3, binding = 0) uniform FragUniforms {
    float u_Threshold;
};

layout(location = 0) out vec4 FragColor;

void main() {
    vec3 color = texture(u_SceneTexture, v_TexCoord).rgb;
    float brightness = dot(color, vec3(0.2126, 0.7152, 0.0722));
    if (brightness > u_Threshold) {
        FragColor = vec4(color * (brightness - u_Threshold), 1.0);
    } else {
        FragColor = vec4(0.0, 0.0, 0.0, 1.0);
    }
}
