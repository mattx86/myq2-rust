#version 450

layout(location = 0) in vec2 v_TexCoord;

layout(set = 0, binding = 0) uniform sampler2D u_SceneTexture;

layout(std140, set = 3, binding = 0) uniform FragUniforms {
    vec4 u_PolyBlend;
    int u_EnablePolyBlend;
    float u_Gamma;
    int u_EnableGamma;
};

layout(location = 0) out vec4 FragColor;

void main() {
    vec4 color = texture(u_SceneTexture, v_TexCoord);

    // Polyblend overlay (damage flash, underwater tint)
    if ((u_EnablePolyBlend != 0) && u_PolyBlend.a > 0.0) {
        color.rgb = mix(color.rgb, u_PolyBlend.rgb, u_PolyBlend.a);
    }

    // Gamma correction
    if (u_EnableGamma != 0) {
        color.rgb = pow(color.rgb, vec3(1.0 / u_Gamma));
    }

    FragColor = color;
}
