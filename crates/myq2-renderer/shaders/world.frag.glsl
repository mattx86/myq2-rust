#version 450

layout(location = 0) in vec2 v_TexCoord;
layout(location = 1) in vec3 v_LightmapCoordLayer;  // (u, v, layer)
layout(location = 2) in vec3 v_FragPos;              // world-space position (unused now, kept for compat)

layout(set = 1, binding = 0) uniform sampler2D u_DiffuseTexture;
layout(set = 2, binding = 0) uniform sampler2DArray u_LightmapArray;
layout(set = 4, binding = 0) uniform sampler3D u_Irradiance;   // VXGI irradiance volume (3D lightmap)

layout(push_constant) uniform PushConstants {
    mat4  u_ModelViewProjection;  // 64 bytes at offset 0  (vertex stage)
    float u_Alpha;                // 4 bytes  at offset 64
    float u_OverbrightScale;      // 4 bytes  at offset 68
    float u_UvScroll;             // 4 bytes  at offset 72 (vertex stage)
    float u_LightmapGamma;        // 4 bytes  at offset 76
    float u_LightmapContrast;     // 4 bytes  at offset 80
    float u_ShadowLift;           // 4 bytes  at offset 84
    float u_LightmapScale;        // 4 bytes  at offset 88 — D3 lightmap blend (1.0=full, near-0=dim ambient)
    float u_UseDynLight;          // 4 bytes  at offset 92 — >0.5: relight a moving brush model
    // Moving brush models (lifts) are relit by the WORLD light sampled at the 4 corners of
    // their footprint at their CURRENT position, packed as RGB-in-a-float, then bilinearly
    // interpolated across the surface by world position so the area's light/shadow gradient
    // falls across the lift instead of one flat value. Corner order: (min,min),(max,min),
    // (min,max),(max,max) in footprint XY.
    float u_DynC0;                // 96
    float u_DynC1;                // 100
    float u_DynC2;                // 104
    float u_DynC3;                // 108
    vec2  u_DynMin;               // 112 — footprint min XY (authored space, matches v_FragPos)
    vec2  u_DynInvSize;           // 120 — 1/(max-min) XY
} pc;

layout(location = 0) out vec4 FragColor;

vec3 unpackRGB(float f) {
    float r = mod(f, 256.0);
    float g = mod(floor(f / 256.0), 256.0);
    float b = floor(f / 65536.0);
    return vec3(r, g, b) * (1.0 / 255.0);
}

void main() {
    // Planar-reflection clip: the mirrored water-reflection pass pushes u_DynInvSize =
    // (±1, planeZ) to discard geometry on the wrong side of the water plane — its mirror image
    // would otherwise wrongly occlude the real reflections. +1 keeps only above-plane geometry
    // (camera above water, mirror shows the room); −1 keeps only below-plane geometry (camera
    // underwater, the surface's underside mirrors the submerged world). Every other pass pushes
    // (0, 0) here (movers set u_UseDynLight=1 and are excluded by the first test). v_FragPos is
    // true world space (the mirroring lives in the MVP only). Small epsilon keeps the plane.
    if (pc.u_UseDynLight < 0.5 && abs(pc.u_DynInvSize.x) > 0.5
        && sign(pc.u_DynInvSize.x) * (v_FragPos.z - pc.u_DynInvSize.y) < -0.5) {
        discard;
    }

    // ---- Lightmap processing ----
    vec3 lightmap = vec3(1.0);
    if (v_LightmapCoordLayer.z >= 0.0) {
        // Classic lightmap: 5-tap cross blur + full baked lighting pipeline. Blur averages ±1
        // atlas texel to soften BSP face-boundary steps.
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
        // u_LightmapScale dims the baked contribution: 1.0 = full classic baked, < 1.0 lets
        // real-time VXGI drive the lighting instead of the baked radiosity (r_vxgi_bake).
        lightmap = lm * min(pc.u_LightmapScale, 1.0);
        // VXGI drives the lighting when scale < 1 (static world only). The irradiance volume is a
        // real-time 3D 'lightmap' — ADD it to the (dimmed) baked term, then the per-texel texture
        // multiply below keeps every texel of detail while VXGI provides the actual light. Grid
        // params ride in the otherwise-unused static dyn-block push slots; u_DynMin.y carries the
        // GI scale (= r_vxgi_strength × HDR scale).
        if (pc.u_LightmapScale < 1.0 && pc.u_UseDynLight < 0.5) {
            vec3 uvw = (v_FragPos - vec3(pc.u_DynC1, pc.u_DynC2, pc.u_DynC3)) / pc.u_DynMin.x;
            // Fade GI out as the sample approaches the grid boundary. clamp() makes any surface at or
            // outside the cube read the EDGE voxel, where dilation floods bright values and sky/
            // emitter voxels sit — so a boundary surface reads over-lit and whites out the view from
            // certain spots. Fading to 0 near the edge lets such surfaces fall back to the baked term.
            vec3 edge = min(uvw, 1.0 - uvw);                       // distance to nearest face (<0 = outside)
            float fade = clamp(min(min(edge.x, edge.y), edge.z) / 0.04, 0.0, 1.0);
            // Sample offset to BOTH sides of the surface along its normal and keep the brighter. A
            // sample taken exactly on a wall face trilinearly blends the lit room-side air with the
            // dark solid wall INTERIOR (interior voxels gather ~0), dragging walls toward black — far
            // worse on walls than floors. Offsetting ~1.5 voxels and taking the max reads the lit room
            // side regardless of which way the derivative-reconstructed normal happens to face.
            vec3 nrm = normalize(cross(dFdx(v_FragPos), dFdy(v_FragPos)));
            float offUVW = 22.0 / pc.u_DynMin.x;
            vec3 giA = textureLod(u_Irradiance, clamp(uvw + nrm * offUVW, 0.0, 1.0), 0.0).rgb;
            vec3 giB = textureLod(u_Irradiance, clamp(uvw - nrm * offUVW, 0.0, 1.0), 0.0).rgb;
            vec3 gi = max(giA, giB) * pc.u_DynMin.y * fade;
            // Soft roll-off (per-channel Reinhard) BEFORE adding. A linear ×strength makes any
            // decently-lit surface exceed 1.0 and slam to flat fullbright at the strength needed to
            // light dim areas; gi/(1+gi) asymptotes below 1 so raising strength keeps lifting the
            // dark without blowing the bright to a flat white region. Strength becomes a real dial.
            gi = gi / (1.0 + gi);
            // HYBRID: the baked lightmap (lm×bake, already set above) is the base — it carries the
            // real per-fixture COLOUR, proper DARKNESS in unlit rooms, and smoothness that voxel GI
            // can't match. The smooth cone-traced GI is ADDED on top as a bounce accent. Pure GI
            // alone was either splotchy (single-ray) or grey + flooding dark rooms (cone-traced); the
            // baked base fixes both while GI adds the dynamic bounce. Keep bake fairly high (~0.7).
            // Dark-room preservation: scale the GI bounce by how lit the BAKED lightmap says this
            // area is. Cone GI is smooth ambient that would flood genuinely dark rooms; keying it to
            // the baked level keeps unlit rooms moody (25% bounce) while lit areas get full bounce.
            float bakedLuma = dot(lm, vec3(0.299, 0.587, 0.114));
            gi *= mix(0.25, 1.0, smoothstep(0.02, 0.18, bakedLuma));
            lightmap += gi;
            // Cap the LIGHTMAP (not the final colour) at 1.0. A strong r_vxgi_strength can drive the
            // GI add well past 1; if we instead let texColor × bigLightmap clamp afterwards, several
            // channels saturate to 1 and the surface desaturates to white ("full bright"). Capping
            // the multiplier means a fully-lit surface reads as texColor × 1 — its real texture and
            // colour at full brightness — and never whites out.
            lightmap = min(lightmap, vec3(1.0));
        }
    }

    // Moving brush models (lifts/doors that have left their authored position) push
    // u_UseDynLight=1 with the world light sampled at the 4 corners of their footprint.
    // Bilinearly interpolate those across the surface by world position so the area's
    // light/shadow gradient falls across the lift (environmental shadow pickup), then blend
    // PARTLY over the model's baked lightmap so it keeps some of its own surface detail.
    // Every other world-shader pass pushes u_UseDynLight=0, so this never affects them.
    if (pc.u_UseDynLight > 0.5) {
        vec2 w = clamp((v_FragPos.xy - pc.u_DynMin) * pc.u_DynInvSize, 0.0, 1.0);
        vec3 dyn = mix(
            mix(unpackRGB(pc.u_DynC0), unpackRGB(pc.u_DynC1), w.x),
            mix(unpackRGB(pc.u_DynC2), unpackRGB(pc.u_DynC3), w.x),
            w.y);
        dyn = pow(clamp(dyn, 0.0, 1.0), vec3(pc.u_LightmapGamma));
        dyn = dyn + vec3(pc.u_ShadowLift) * (1.0 - dyn);
        float lm_pivot = 0.35;
        dyn = clamp((dyn - lm_pivot) * pc.u_LightmapContrast + lm_pivot, 0.0, 1.0);
        lightmap = mix(lightmap, dyn, 0.6);
    }

    // ---- Final output ----
    vec4 texColor = texture(u_DiffuseTexture, v_TexCoord);
    vec3 outc = texColor.rgb * lightmap;
    // SURF_LIGHT emissive boost. When NOT relighting a mover (u_UseDynLight<0.5), u_DynC0
    // is reused to carry an emit multiplier (1.0 for normal surfaces, >1 for light textures)
    // that pushes them above 1.0 into HDR so the bloom pass makes them glow. Every non-mover
    // world-shader pass pushes this value, so a mover's packed-corner u_DynC0 can't leak in.
    // Light textures push u_DynC0 > 1 to boost above the HDR bloom threshold so they glow; normal
    // surfaces push 1.0 (no-op). The diffuse lightmap is already capped at 1.0 above, so non-emit
    // surfaces can't reach the bloom threshold on their own — only genuine emitters bloom.
    if (pc.u_UseDynLight < 0.5) {
        outc *= max(pc.u_DynC0, 1.0);
    }
    FragColor = vec4(outc, texColor.a * pc.u_Alpha);
}
