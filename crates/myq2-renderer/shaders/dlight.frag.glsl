#version 450

layout(location = 0) in vec3 v_Position;

layout(std140, set = 3, binding = 0) uniform FragUniforms {
    vec3 u_LightOrigin;
    float _pad0;
    vec3 u_LightColor;
    float u_LightRadius;
};

layout(location = 0) out vec4 FragColor;

void main() {
    float dist = length(v_Position - u_LightOrigin);
    float attenuation = 1.0 - clamp(dist / u_LightRadius, 0.0, 1.0);
    attenuation = attenuation * attenuation;  // Quadratic falloff

    vec3 color = u_LightColor * 0.2 * attenuation;
    FragColor = vec4(color, attenuation);
}
