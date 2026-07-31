#version 450

// Water ripple shimmer: animated caustic light that water throws onto nearby walls,
// ceilings and walkways (and refracted caustics on the submerged floor below).
//
// Fullscreen additive pass after the scene. Each pixel reconstructs its world position from
// the scene depth; pixels within a height band around the active water plane get an animated
// ripple pattern ADDED, tinted and gated by the VXGI irradiance sampled AT THE WATER LEVEL
// below that pixel — so the shimmer only appears in columns that are actually over/near the
// water body (elsewhere that voxel is inside solid rock ≈ zero light), and it automatically
// carries the water's colour (blue-green pools, orange lava).

layout(location = 0) in vec2 v_Uv;

layout(set = 0, binding = 0) uniform sampler2D u_SceneDepth;
layout(set = 0, binding = 1) uniform sampler3D u_Irradiance;

layout(push_constant) uniform PushConstants {
    mat4  u_InvVP;      // 0   — NDC → world
    vec3  u_GridMin;    // 64  — VXGI grid origin
    float u_PlaneZ;     // 76  — active water plane height
    float u_Extent;     // 80  — VXGI grid extent
    float u_Time;       // 84
    float u_Strength;   // 88
    float u_Pad;        // 92
} pc;

layout(location = 0) out vec4 FragColor;

// Two-octave interference ripple — cheap, tileable-enough, animates like light refracted
// through a moving water surface. `p` is a 2D coordinate on the receiving surface.
float caustic(vec2 p, float t) {
    vec2 q = p * 0.055;
    float c = sin(q.x * 1.7 + t)         * sin(q.y * 1.3 - t * 0.8)
            + sin((q.x + q.y) * 1.1 - t * 0.6) * sin((q.x - q.y) * 1.9 + t * 0.5);
    // Sharpen the bright filaments (caustics are thin bright lines, not smooth waves).
    return pow(clamp(0.5 + 0.4 * c, 0.0, 1.0), 3.0);
}

void main() {
    float depth = texture(u_SceneDepth, v_Uv).r;
    if (depth >= 1.0) discard; // sky / empty

    // Reconstruct world position (3D pass renders with a Y-flipped viewport).
    vec4 ndc = vec4(v_Uv.x * 2.0 - 1.0, -(v_Uv.y * 2.0 - 1.0), depth, 1.0);
    vec4 wp = pc.u_InvVP * ndc;
    if (abs(wp.w) < 1e-6) discard;
    vec3 P = wp.xyz / wp.w;

    // WALLS ONLY: reconstruct the surface normal from depth derivatives and fade the effect
    // out on horizontal surfaces. Painting the ripple pattern on floors reads as "the water
    // texture projected onto the floor" (and on the pool floor below), which is not wanted —
    // rippling REFLECTED light lives on vertical surfaces near the water.
    vec3 nrm = normalize(cross(dFdx(P), dFdy(P)));
    float wallness = smoothstep(0.35, 0.75, 1.0 - abs(nrm.z));
    if (wallness <= 0.01) discard;

    float h = P.z - pc.u_PlaneZ;
    // Reflected ripple light climbs walls above the waterline (fading with height) and also
    // plays on submerged wall faces just under it.
    float band;
    if (h >= 0.0) {
        band = 1.0 - smoothstep(48.0, 240.0, h);
    } else {
        band = 0.8 * (1.0 - smoothstep(16.0, 200.0, -h));
    }
    if (band <= 0.001) discard;

    // Light at the water level directly below/above this pixel: confines the shimmer to the
    // water body's actual footprint and tints it with the water's bounced colour.
    vec3 wuvw = (vec3(P.xy, pc.u_PlaneZ - 4.0) - pc.u_GridMin) / pc.u_Extent;
    vec3 waterLight = textureLod(u_Irradiance, clamp(wuvw, 0.0, 1.0), 0.0).rgb;
    float gate = clamp(dot(waterLight, vec3(1.0)), 0.0, 1.0);
    if (gate <= 0.01) discard;

    // Ripple in a wall-aligned coordinate: horizontal position along the wall + height, so
    // the pattern flows ACROSS the wall face. Two drifting layers so it never reads static.
    vec2 wallUv = vec2(dot(P.xy, normalize(vec2(-nrm.y, nrm.x) + 1e-4)), P.z);
    float t = pc.u_Time;
    float c = caustic(wallUv, t * 1.35) * 0.65
            + caustic(wallUv * 1.7 + vec2(37.0, 11.0), -t * 0.9) * 0.35;

    vec3 shimmer = waterLight * c * band * wallness * pc.u_Strength;
    FragColor = vec4(shimmer, 1.0);
}
