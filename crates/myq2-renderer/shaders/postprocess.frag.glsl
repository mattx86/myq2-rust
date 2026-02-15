#version 450

layout(location = 0) in vec2 v_TexCoord;

layout(set = 1, binding = 0) uniform sampler2D u_SceneTexture;

layout(push_constant) uniform PushConstants {
    vec4 u_PolyBlend;
    int u_EnablePolyBlend;
    float u_Gamma;
    int u_EnableGamma;
} pc;

layout(location = 0) out vec4 FragColor;

void main() {
    vec4 color = texture(u_SceneTexture, v_TexCoord);

    // Polyblend overlay (damage flash, underwater tint)
    if ((pc.u_EnablePolyBlend != 0) && pc.u_PolyBlend.a > 0.0) {
        color.rgb = mix(color.rgb, pc.u_PolyBlend.rgb, pc.u_PolyBlend.a);
    }

    // Gamma correction
    if (pc.u_EnableGamma != 0) {
        color.rgb = pow(color.rgb, vec3(1.0 / pc.u_Gamma));
    }

    FragColor = color;
}
