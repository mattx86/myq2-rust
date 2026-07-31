#version 450

// VXGI Phase 4: diffuse cone-traced global illumination.
//
// Fullscreen deferred pass. Reconstructs world position + geometric normal from the scene depth,
// fires a small fan of diffuse cones into the mip-chained radiance volume, and adds the gathered
// bounced light to the scene (additive blend). The radiance volume stores emission in RGB and
// opacity in A, so front-to-back accumulation occludes against walls.

layout(location = 0) in vec2 v_Uv;

layout(set = 0, binding = 0) uniform sampler2D u_Depth;     // scene depth (depth aspect)
layout(set = 0, binding = 1) uniform sampler3D u_Radiance;  // RGB = radiance, A = opacity (mipped)
layout(set = 0, binding = 2) uniform sampler3D u_Albedo;    // RGB = surface albedo, A = coverage

// Dynamic lights (rockets, muzzle flashes, explosions). Evaluated volumetrically along each
// cone so moving lights also bounce — gathered where a cone passes near them, occluded by the
// static walls via the same opacity accumulation.
struct DLight { vec4 pos_radius; vec4 color; };
layout(set = 0, binding = 3) uniform DLights {
    int   u_NumDLights;
    DLight lights[32];
} dl;

vec3 dynamic_emission(vec3 p) {
    vec3 e = vec3(0.0);
    int n = clamp(dl.u_NumDLights, 0, 32);
    for (int i = 0; i < n; ++i) {
        vec3 lp = dl.lights[i].pos_radius.xyz;
        float lr = dl.lights[i].pos_radius.w;
        float d = distance(p, lp);
        if (d < lr) {
            float f = 1.0 - d / lr;
            e += dl.lights[i].color.rgb * (f * f);
        }
    }
    return e;
}

layout(push_constant) uniform PushConstants {
    mat4  u_InvViewProj; // 0
    vec3  u_CamPos;      // 64
    float u_Strength;    // 76
    vec3  u_GridMin;     // 80
    float u_Extent;      // 92
    float u_VoxelSize;   // 96
} pc;

layout(location = 0) out vec4 FragColor;

vec3 world_pos(vec2 uv, float depth) {
    // Scene uses a Y-flipped viewport, so clip.y is negated.
    vec2 ndc = vec2(uv.x * 2.0 - 1.0, -(uv.y * 2.0 - 1.0));
    vec4 p = pc.u_InvViewProj * vec4(ndc, depth, 1.0);
    return p.xyz / p.w;
}

// March one cone through the radiance volume, front-to-back. `aperture` = tan(half-angle).
vec4 trace_cone(vec3 origin, vec3 dir, float aperture) {
    vec4 acc = vec4(0.0);
    float vs = pc.u_VoxelSize;
    float dist = vs * 1.5;
    // Occlusion boost: averaged opacity at coarse mips is weak, so walls stop blocking and
    // light leaks through into rooms it shouldn't reach (the "fog"). Amplify opacity so even an
    // averaged wall occludes, and clamp the mip so cones never sample the very coarsest levels
    // (which spread emission across the whole volume).
    const float OCC_BOOST = 5.0;
    const float MAX_MIP   = 3.0;
    // Limit the gather range so a cone only collects relatively LOCAL bounce, not light from
    // clear across the level (another source of uniform haze).
    float maxd = pc.u_Extent * 0.22;
    for (int i = 0; i < 32 && acc.a < 0.95; ++i) {
        float diameter = max(vs, 2.0 * aperture * dist);
        float mip = min(log2(max(diameter / vs, 1.0)), MAX_MIP);
        vec3 p = origin + dir * dist;
        vec3 uvw = (p - pc.u_GridMin) / pc.u_Extent;
        if (any(lessThan(uvw, vec3(0.0))) || any(greaterThan(uvw, vec3(1.0)))) break;
        vec4 s = textureLod(u_Radiance, uvw, mip);
        float a = clamp(s.a * OCC_BOOST, 0.0, 1.0);
        // Static surface radiance (occluded) + dynamic-light glow in this volume of space
        // (volumetric, not occluded by its own opacity but cut off once the cone hits a wall).
        vec3 dyn = dynamic_emission(p) * 0.12;
        acc.rgb += (1.0 - acc.a) * (a * s.rgb + dyn);
        acc.a   += (1.0 - acc.a) * a;
        dist += diameter * 0.6;
        if (dist > maxd) break;
    }
    return acc;
}

void main() {
    float depth = texture(u_Depth, v_Uv).r;
    if (depth >= 1.0) discard; // sky / no geometry

    vec3 P = world_pos(v_Uv, depth);
    // Geometric normal from screen-space derivatives, oriented toward the camera.
    vec3 N = normalize(cross(dFdx(P), dFdy(P)));
    vec3 toCam = normalize(pc.u_CamPos - P);
    if (dot(N, toCam) < 0.0) N = -N;

    // Hemisphere basis.
    vec3 up = abs(N.z) < 0.9 ? vec3(0.0, 0.0, 1.0) : vec3(1.0, 0.0, 0.0);
    vec3 T = normalize(cross(up, N));
    vec3 B = cross(N, T);

    // 5 cones (1 along N + 4 tilted 45°), aperture ~ tan(30°). Start off the surface to avoid
    // self-intersection.
    float ap = 0.577;
    vec3 o = P + N * pc.u_VoxelSize * 2.0;
    vec4 g = trace_cone(o, N, ap) * 0.25;
    g += trace_cone(o, normalize(N * 0.707 + T * 0.707), ap) * 0.1875;
    g += trace_cone(o, normalize(N * 0.707 - T * 0.707), ap) * 0.1875;
    g += trace_cone(o, normalize(N * 0.707 + B * 0.707), ap) * 0.1875;
    g += trace_cone(o, normalize(N * 0.707 - B * 0.707), ap) * 0.1875;

    // Indirect light reflects off the RECEIVER's albedo (sampled from the voxel albedo volume),
    // so the bounce is tinted by the surface colour and never white-washes. Sample just INSIDE
    // the surface (along -N) so the lookup lands in the covered voxel rather than the empty one
    // in front of it. No grey fallback — a missed/black albedo simply contributes no GI (which
    // is correct), instead of pasting a uniform grey haze over everything.
    // Output IRRADIANCE (the gathered incoming light), NOT albedo×light. The blend is
    // multiplicative — scene × (1 + irradiance·strength) — so this brightens the existing
    // per-texel surface (texColor × baked) instead of pasting a flat per-texture average colour
    // over it. The texColor already IS the per-texel albedo, so multiplying applies it correctly
    // and every texel of texture detail survives. Tints by the light colour carried in `g`.
    FragColor = vec4(g.rgb * pc.u_Strength, 1.0);
}
