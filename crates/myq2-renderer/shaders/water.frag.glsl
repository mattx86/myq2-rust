#version 450

layout(location = 0) in vec2 v_TexCoord;
layout(location = 1) in vec3 v_LightmapCoordLayer;  // (u, v, layer); layer < 0 = no lightmap
layout(location = 2) in vec3 v_WorldPos;            // world-space position (Fresnel)

layout(set = 1, binding = 0) uniform sampler2D u_WaterTexture;
layout(set = 2, binding = 0) uniform sampler2DArray u_LightmapArray;
// Planar reflection of the world+sky, rendered mirrored about the water plane at half scene
// resolution. Only bound/used when u_ReflStrength > 0.
layout(set = 3, binding = 0) uniform sampler2D u_ReflTexture;

layout(push_constant) uniform PushConstants {
    mat4 u_ModelViewProjection;  // 64 bytes (vertex stage, not used here)
    float u_Alpha;               // 4 bytes at offset 64
    float u_Time;                // 4 bytes at offset 68
    float u_Scroll;              // 4 bytes at offset 72
    float u_OverbrightScale;     // 76
    float u_LightmapGamma;       // 80
    float u_LightmapContrast;    // 84
    float u_ShadowLift;          // 88
    float u_LightmapScale;       // 92
    vec3  u_FlatLight;           // 96..108 — per-body flat liquid light; r<0 = none
    float u_ReflStrength;        // 108 — planar reflection blend; 0 = off for this surface
    float u_CamX;                // 112 — camera world position (Fresnel angle)
    float u_CamY;                // 116
    float u_CamZ;                // 120
};

layout(location = 0) out vec4 FragColor;

// Shared baked-light response curve (gamma / shadow lift / contrast) — matches world.frag.
vec3 processLm(vec3 lm) {
    lm = pow(clamp(lm, 0.0, 1.0), vec3(u_LightmapGamma));
    lm = lm + vec3(u_ShadowLift) * (1.0 - lm);
    float lm_pivot = 0.35;
    return clamp((lm - lm_pivot) * u_LightmapContrast + lm_pivot, 0.0, 1.0);
}

// Per-pixel baked lightmap (5-tap cross blur), processed through the same curve.
vec3 pixelLm() {
    vec2  lm_d = 1.0 / vec2(textureSize(u_LightmapArray, 0).xy);
    vec2  uv   = v_LightmapCoordLayer.xy;
    float ly   = v_LightmapCoordLayer.z;
    vec3 lmBlur = (
        texture(u_LightmapArray, vec3(uv,                       ly)).rgb +
        texture(u_LightmapArray, vec3(uv + vec2( lm_d.x,  0.0), ly)).rgb +
        texture(u_LightmapArray, vec3(uv + vec2(-lm_d.x,  0.0), ly)).rgb +
        texture(u_LightmapArray, vec3(uv + vec2( 0.0,  lm_d.y), ly)).rgb +
        texture(u_LightmapArray, vec3(uv + vec2( 0.0, -lm_d.y), ly)).rgb
    ) * 0.2;
    return processLm(clamp(lmBlur * u_OverbrightScale, 0.0, 1.0));
}

void main() {
    float os = v_TexCoord.s;
    float ot = v_TexCoord.t;

    // Replicate original Quake 2 turbulent warp formula:
    //   offset = 8*sin(coord*0.125+time) * 0.5 = 4*sin(coord*0.125+time)
    float sOffset = sin(ot * 0.125 + u_Time) * 4.0;
    float tOffset = sin(os * 0.125 + u_Time) * 4.0;

    vec2 warpedCoord;
    warpedCoord.s = (os + sOffset) / 64.0;
    warpedCoord.t = (ot + tOffset) / 64.0;

    if (u_Scroll != 0.0) {
        warpedCoord.s += u_Scroll / 64.0;
    }

    vec4 waterColor = texture(u_WaterTexture, warpedCoord);

    // ---- Lighting ----
    // The per-body flat light (u_FlatLight, computed at load; r<0 = none) lights the liquid
    // uniformly so BSP face seams don't show — but a big pool lit by ONE value looks flat.
    // Blend it with the per-pixel lightmap when one exists: the body light sets the level,
    // the lightmap layers the spatial light/shadow variation across the surface back on top.
    vec3 lightmap = vec3(1.0);
    bool hasPixelLm = v_LightmapCoordLayer.z >= 0.0 && u_LightmapScale >= 1.0;
    if (u_FlatLight.r >= 0.0) {
        vec3 bodyL = processLm(clamp(u_FlatLight, 0.0, 1.0));
        lightmap = hasPixelLm ? mix(bodyL, pixelLm(), 0.45) : bodyL;
    } else if (v_LightmapCoordLayer.z >= 0.0) {
        lightmap = hasPixelLm ? pixelLm() : vec3(u_LightmapScale);
    }

    vec3 outc = waterColor.rgb * lightmap;

    // ---- Planar reflection ----
    // The reflection target was rendered with the SAME projection but a camera mirrored about
    // the water plane, so the reflection of this fragment lands at this fragment's own screen
    // position — sample it at gl_FragCoord over the scene size (= refl size × 2, half-res
    // target). Perturb the sample with the same warp phase as the texture so reflections
    // ripple. Fresnel (Schlick): near-grazing views reflect strongly, top-down views barely.
    if (u_ReflStrength > 0.0) {
        vec2 rsz = vec2(textureSize(u_ReflTexture, 0));
        vec2 ruv = gl_FragCoord.xy / (rsz * 2.0);
        ruv += vec2(sin(ot * 0.125 + u_Time), sin(os * 0.125 + u_Time)) * 0.004;
        vec3 refl = texture(u_ReflTexture, clamp(ruv, vec2(0.001), vec2(0.999))).rgb;
        vec3 viewDir = normalize(vec3(u_CamX, u_CamY, u_CamZ) - v_WorldPos);
        float cosT = clamp(abs(viewDir.z), 0.0, 1.0);   // water plane normal is ±Z
        // Higher base than physical Fresnel (0.02) so the mirror clearly reads even when
        // looking down at the water; grazing angles still ramp toward a full mirror.
        // UNDERWATER (camera below the surface): the mirrored pass rendered the submerged
        // world instead, and total internal reflection makes the underside of a water
        // surface strongly mirror-like — use a much higher base.
        bool underwater = u_CamZ < v_WorldPos.z - 1.0;
        float fresnel = underwater
            ? 0.55 + 0.45 * pow(1.0 - cosT, 2.0)
            : 0.25 + 0.75 * pow(1.0 - cosT, 2.0);
        outc = mix(outc, refl, clamp(fresnel * u_ReflStrength * 2.5, 0.0, 1.0));
    }

    FragColor = vec4(outc, u_Alpha);
}
