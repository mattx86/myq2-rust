#version 450

layout(location = 0) in vec3 v_WorldPos;
layout(location = 1) in vec3 v_Normal;
layout(location = 2) in vec2 v_TexCoord;
layout(location = 3) in vec3 v_LmCoordLayer;   // baked lightmap (u, v, layer)

layout(set = 1, binding = 0) uniform sampler2D u_DiffuseTexture;
layout(set = 2, binding = 0) uniform samplerCube u_ShadowMap;
layout(set = 3, binding = 0) uniform sampler2DArray u_LightmapArray;

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

    // The BSP vertex stream fed to this pass carries no usable normal (v_Normal is a
    // placeholder), so reconstruct the geometric face normal from screen-space derivatives.
    // Orient it toward the viewer (we only ever see front faces) so it doesn't depend on
    // winding, then apply N·L so surfaces facing AWAY from a light stay dark instead of
    // every face accumulating light from every direction — which over-brightens and, with
    // many warm lights, saturates the red channel.
    vec3 N = normalize(cross(dFdx(v_WorldPos), dFdy(v_WorldPos)));
    vec3 V = normalize(pc.u_ViewOrigin - v_WorldPos);
    if (dot(N, V) < 0.0) N = -N;
    float ndotl = max(dot(N, to_light / max(dist, 0.001)), 0.0);

    // Smooth quadratic attenuation — zero at radius edge
    float x     = clamp(dist / max(pc.u_LightRadius, 0.001), 0.0, 1.0);
    float atten = (1.0 - x * x) * pc.u_LightIntensity * ndotl;

    // Omnidirectional shadow test: sample cubemap in the direction from
    // light to fragment.  The cubemap stores (closest_dist / radius) per
    // direction; compare against the current fragment's normalized distance.
    vec3  shadow_dir  = v_WorldPos - pc.u_LightPos;   // light → fragment
    float frag_ndist  = dist / max(pc.u_LightRadius, 0.001);
    float stored_dist = texture(u_ShadowMap, shadow_dir).r;
    float shadow      = (frag_ndist - pc.u_ShadowBias) > stored_dist ? 0.0 : 1.0;

    // DARK-FILL: this pass adds on top of the classic baked lightmap. Adding warm light to
    // ALREADY-lit surfaces shifts the whole scene red (Q2 lights are warm), so weight the
    // contribution by how DARK the baked lightmap already is here — a lamp then fills the
    // shadowed wall beside it but barely touches surfaces the baked pass already lit.
    float fill = 1.0;
    if (v_LmCoordLayer.z >= 0.0) {
        vec3  lm     = texture(u_LightmapArray, v_LmCoordLayer).rgb;
        float lmLuma = dot(lm, vec3(0.299, 0.587, 0.114));
        fill = 1.0 - clamp(lmLuma, 0.0, 1.0);
    }

    // Q2 lights are overwhelmingly warm, so adding their raw colour tints the whole scene red.
    // Desaturate the fill toward its own luminance (keep just a hint of hue) so it brightens
    // dark surfaces without shifting the colour balance.
    float lc_luma = dot(pc.u_LightColor, vec3(0.299, 0.587, 0.114));
    vec3  lc_fill = mix(vec3(lc_luma), pc.u_LightColor, 0.25);

    vec3 diffuse = texture(u_DiffuseTexture, v_TexCoord).rgb;
    FragColor = vec4(diffuse * lc_fill * atten * shadow * fill, 1.0);
}
