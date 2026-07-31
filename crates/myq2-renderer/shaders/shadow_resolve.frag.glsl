#version 450

// Projective shadow resolve.
//
// Fullscreen pass after the scene, before post-processing. For each pixel it reconstructs
// the world position from the scene depth, projects it into the shadow light's clip space,
// and darkens it if a caster is between it and the light — AND the pixel is between the
// caster and the FLOOR traced below that caster. The floor bound (G channel of the shadow
// map) makes a shadow land on the surface the caster actually sits above and stops it from
// bleeding through that surface onto things further along the light ray.
//
// It also darkens the whole view when the CAMERA is in a caster's shadow (something passing
// overhead between the light and the player).

layout(location = 0) in vec2 v_Uv;

layout(set = 0, binding = 0) uniform sampler2D u_SceneDepth;   // scene depth
layout(set = 0, binding = 1) uniform sampler2D u_ShadowMap;    // RG: R=caster depth, G=floor depth

layout(push_constant) uniform PushConstants {
    mat4  u_NdcToLightClip; // 0  (light_view_proj * inv_view_proj)
    float u_ShadowBias;     // 64
    float u_Darkness;       // 68 — darkening of a shadowed world pixel
    float u_CamMinGap;      // 72 — min light-space gap for the camera dim (excludes own body)
    float u_FloorBand;      // 76 — soft fade band (light-space depth) at the floor edge
    vec3  u_CamProj;        // 80 — camera position in light NDC
    float u_CamDim;         // 92 — view dim when the camera is shadowed
    float u_NearSkip;       // 96 — skip per-pixel shadows nearer than this depth (the held
                            //      view weapon, so it isn't darkened by your own body's shadow)
} pc;

layout(location = 0) out vec4 FragColor;

// Soft occlusion at a light-NDC position: receiver behind the caster (R) and in front of the
// caster's floor (G). Returns 0..1 (PCF-averaged), or -1 if outside the shadow map.
float occlusion(vec3 proj) {
    vec2 smUv = proj.xy * 0.5 + 0.5;
    if (smUv.x < 0.0 || smUv.x > 1.0 || smUv.y < 0.0 || smUv.y > 1.0) return -1.0;
    vec2 texel = 1.0 / vec2(textureSize(u_ShadowMap, 0));
    float occ = 0.0;
    const int   R = 1;
    const float SPREAD = 2.5;   // tap spacing in texels — widens the penumbra at no extra cost
    for (int y = -R; y <= R; ++y)
        for (int x = -R; x <= R; ++x) {
            vec2 d = texture(u_ShadowMap, smUv + vec2(float(x), float(y)) * texel * SPREAD).rg;
            // Smooth caster test (not a binary bias cut): shadow strength eases in across a
            // small depth band behind the caster, so contact edges and the transition onto an
            // adjacent surface ramp instead of snapping.
            float behind = smoothstep(pc.u_ShadowBias, pc.u_ShadowBias * 4.0, proj.z - d.r);
            // Full strength up to and INCLUDING the floor, PLUS a grace region below it
            // (0.75×band ≈ 80 world units) so a shadow draping across joined floor pieces
            // at slightly different heights (steps, cut floors) stays at FULL strength
            // across the seam instead of fading the moment it leaves the caster's own
            // floor plane. Beyond the grace it fades across another band so it still
            // can't bleed through onto rooms far below.
            occ += behind * (1.0 - smoothstep(d.g + pc.u_FloorBand * 0.75,
                                              d.g + pc.u_FloorBand * 2.0, proj.z));
        }
    return occ / float((2 * R + 1) * (2 * R + 1));
}

void main() {
    float depth = texture(u_SceneDepth, v_Uv).r;

    // ---- Per-pixel world shadow (skip the far plane: sky / nothing, and the very near
    // view weapon so your own body's shadow doesn't fall across it) ----
    float pixel_dark = 0.0;
    if (depth < 1.0 && depth >= pc.u_NearSkip) {
        // Scene NDC. The 3D pass uses a Y-FLIPPED viewport, so clip.y = -(2*V - 1).
        vec4 ndc = vec4(v_Uv.x * 2.0 - 1.0, -(v_Uv.y * 2.0 - 1.0), depth, 1.0);
        vec4 lc = pc.u_NdcToLightClip * ndc;
        if (lc.w > 0.0) {
            float occ = occlusion(lc.xyz / lc.w);
            if (occ > 0.0) pixel_dark = pc.u_Darkness * occ;
        }
    }

    // ---- Whole-view dim when the CAMERA is in a caster's shadow ----
    // Only count occluders meaningfully ABOVE the camera (gap > u_CamMinGap), so the player's
    // own body — a caster right at the eye — doesn't constantly dim or flicker the view; only
    // real overhead casters (something passing over you) do.
    float cam_dark = 0.0;
    {
        vec2 cuv = pc.u_CamProj.xy * 0.5 + 0.5;
        if (cuv.x >= 0.0 && cuv.x <= 1.0 && cuv.y >= 0.0 && cuv.y <= 1.0) {
            vec2 d = texture(u_ShadowMap, cuv).rg;
            float gap = pc.u_CamProj.z - d.r;
            if (gap > pc.u_CamMinGap && pc.u_CamProj.z < d.g) {
                cam_dark = pc.u_CamDim;
            }
        }
    }

    float darken = max(pixel_dark, cam_dark);
    if (darken <= 0.001) { discard; }
    FragColor = vec4(vec3(1.0 - clamp(darken, 0.0, 1.0)), 1.0);
}
