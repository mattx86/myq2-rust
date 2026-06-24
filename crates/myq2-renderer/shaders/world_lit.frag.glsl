#version 450

layout(location = 0) in vec3 v_WorldPos;
layout(location = 1) in vec3 v_Normal;
layout(location = 2) in vec2 v_TexCoord;

layout(set = 1, binding = 0) uniform sampler2D u_DiffuseTexture;
layout(set = 2, binding = 0) uniform samplerCube u_ShadowMap;

layout(push_constant) uniform PushConstants {
    mat4  u_MVP;
    vec3  u_LightPos;
    float u_LightRadius;
    vec3  u_LightColor;
    float u_LightIntensity;
    float u_SpecPower;
    float u_ShadowBias;
    float _pad1;
    float _pad2;
    vec3  u_ViewOrigin;
} pc; // 124 bytes

layout(location = 0) out vec4 FragColor;

void main() {
    vec3  to_light = pc.u_LightPos - v_WorldPos;
    float dist     = length(to_light);

    // Smooth quadratic attenuation — zero at radius edge
    float x     = clamp(dist / max(pc.u_LightRadius, 0.001), 0.0, 1.0);
    float atten = (1.0 - x * x) * pc.u_LightIntensity;

    // Omnidirectional shadow test: sample cubemap in the direction from
    // light to fragment.  The cubemap stores (closest_dist / radius) per
    // direction; compare against the current fragment's normalized distance.
    vec3  shadow_dir  = v_WorldPos - pc.u_LightPos;   // light → fragment
    float frag_ndist  = dist / max(pc.u_LightRadius, 0.001);
    float stored_dist = texture(u_ShadowMap, shadow_dir).r;
    float shadow      = (frag_ndist - pc.u_ShadowBias) > stored_dist ? 0.0 : 1.0;

    vec3 diffuse = texture(u_DiffuseTexture, v_TexCoord).rgb;
    FragColor = vec4(diffuse * pc.u_LightColor * atten * shadow, 1.0);
}
