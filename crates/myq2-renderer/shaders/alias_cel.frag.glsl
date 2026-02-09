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

    // Cel-shading: quantize lighting into bands
    float ndotl = dot(normal, normalize(u_LightDir));
    float shade;
    if (ndotl > 0.5) shade = 1.0;
    else if (ndotl > 0.0) shade = 0.7;
    else shade = 0.4;

    vec3 finalColor = texColor.rgb * u_ShadeLight * shade;
    FragColor = vec4(finalColor, texColor.a * u_Alpha);
}
