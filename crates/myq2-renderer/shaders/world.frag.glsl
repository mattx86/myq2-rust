#version 450

layout(location = 0) in vec2 v_TexCoord;
layout(location = 1) in vec3 v_LightmapCoordLayer;  // (u, v, layer)
layout(location = 2) in vec3 v_FragPos;              // world-space position (unused now, kept for compat)

layout(set = 1, binding = 0) uniform sampler2D u_DiffuseTexture;
layout(set = 2, binding = 0) uniform sampler2DArray u_LightmapArray;

layout(push_constant) uniform PushConstants {
    mat4  u_ModelViewProjection;  // 64 bytes at offset 0  (vertex stage)
    float u_Alpha;                // 4 bytes  at offset 64
    float u_OverbrightScale;      // 4 bytes  at offset 68
    float u_UvScroll;             // 4 bytes  at offset 72 (vertex stage)
    float u_LightmapGamma;        // 4 bytes  at offset 76
    float u_LightmapContrast;     // 4 bytes  at offset 80
    float u_ShadowLift;           // 4 bytes  at offset 84
    float u_LightmapScale;        // 4 bytes  at offset 88 — D3 lightmap blend (1.0=full, near-0=dim ambient)
} pc;

layout(location = 0) out vec4 FragColor;

void main() {
    // ---- Lightmap processing ----
    vec3 lightmap = vec3(1.0);
    if (v_LightmapCoordLayer.z >= 0.0) {
        if (pc.u_LightmapScale >= 1.0) {
            // Classic lightmap mode: 5-tap cross blur + full baked lighting pipeline.
            // Blur averages ±1 atlas texel to soften BSP face-boundary steps.
            vec2  lm_d = 1.0 / vec2(textureSize(u_LightmapArray, 0).xy);
            vec2  uv   = v_LightmapCoordLayer.xy;
            float ly   = v_LightmapCoordLayer.z;
            vec3 lmBlur = (
                texture(u_LightmapArray, vec3(uv,                        ly)).rgb +
                texture(u_LightmapArray, vec3(uv + vec2( lm_d.x,  0.0), ly)).rgb +
                texture(u_LightmapArray, vec3(uv + vec2(-lm_d.x,  0.0), ly)).rgb +
                texture(u_LightmapArray, vec3(uv + vec2( 0.0,  lm_d.y), ly)).rgb +
                texture(u_LightmapArray, vec3(uv + vec2( 0.0, -lm_d.y), ly)).rgb
            ) * 0.2;
            vec3 lm = clamp(lmBlur * pc.u_OverbrightScale, 0.0, 1.0);
            lm = pow(clamp(lm, 0.0, 1.0), vec3(pc.u_LightmapGamma));
            lm = lm + vec3(pc.u_ShadowLift) * (1.0 - lm);
            float lm_pivot = 0.35;
            lm = clamp((lm - lm_pivot) * pc.u_LightmapContrast + lm_pivot, 0.0, 1.0);
            lightmap = lm;
        } else {
            // D3 mode: flat ambient floor — zero lightmap reads, zero per-face atlas data,
            // zero face-boundary seams.  Floating radial lights are added by the D3 lit pass.
            lightmap = vec3(pc.u_LightmapScale);
        }
    }

    // ---- Final output ----
    vec4 texColor = texture(u_DiffuseTexture, v_TexCoord);
    FragColor = vec4(texColor.rgb * lightmap, texColor.a * pc.u_Alpha);
}
