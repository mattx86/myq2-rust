#version 450

layout(location = 0) in vec2 v_TexCoord;
layout(location = 1) in vec3 v_Normal;

layout(set = 0, binding = 0) uniform sampler2D u_DiffuseTexture;

layout(std140, set = 3, binding = 0) uniform FragUniforms {
    vec3 u_ShadeLight;
    float _pad0;
    vec3 u_LightDir;
    float u_Alpha;
};

layout(location = 0) out vec4 FragColor;

void main() {
    vec4 texColor = texture(u_DiffuseTexture, v_TexCoord);
    vec3 normal = normalize(v_Normal);

    // Smooth shading: soft diffuse with a raised ambient floor so backfaces
    // don't go black.  Avoids hard-band artifacts on geometric model details.
    float ndotl = dot(normal, normalize(u_LightDir));
    float shade = mix(0.4, 1.0, clamp(ndotl * 0.5 + 0.5, 0.0, 1.0));

    vec3 finalColor = texColor.rgb * u_ShadeLight * shade;
    FragColor = vec4(finalColor, texColor.a * u_Alpha);
}
