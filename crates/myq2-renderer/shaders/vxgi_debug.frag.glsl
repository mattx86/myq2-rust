#version 450

// VXGI Phase 1 debug view: raymarch the static voxel grid from the camera and display it.
// Each occupied voxel stores its surface normal (RGB, encoded) — so walls, floors and ceilings
// read as distinct colours and the voxelized structure is legible. Misses are discarded so the
// real scene shows through for context.

layout(location = 0) in vec2 v_Uv;

layout(set = 0, binding = 0) uniform sampler3D u_Voxels;

layout(push_constant) uniform PushConstants {
    mat4  u_InvViewProj;  // 0
    vec3  u_CamPos;       // 64
    float u_Mode;         // 76 — 1 = albedo/normal volume, 2 = radiance (emitters)
    vec3  u_GridMin;      // 80 — world corner of the cube
    float u_Extent;       // 92 — world edge length of the cube
} pc;

layout(location = 0) out vec4 FragColor;

void main() {
    // Reconstruct a world-space ray. The 3D scene uses a Y-FLIPPED viewport, so clip.y is
    // negated (matches the shadow resolve convention).
    vec2 ndc = vec2(v_Uv.x * 2.0 - 1.0, -(v_Uv.y * 2.0 - 1.0));
    vec4 pn = pc.u_InvViewProj * vec4(ndc, 0.0, 1.0);
    vec4 pf = pc.u_InvViewProj * vec4(ndc, 1.0, 1.0);
    vec3 wn = pn.xyz / pn.w;
    vec3 wf = pf.xyz / pf.w;
    vec3 dir = normalize(wf - wn);

    float inv_extent = 1.0 / max(pc.u_Extent, 1.0);
    float vsize = pc.u_Extent / float(textureSize(u_Voxels, 0).x);
    float step = vsize * 0.5;
    float maxt = pc.u_Extent * 1.8;

    // Fixed soft "sun" direction just for legibility of the normal-coloured voxels.
    vec3 sun = normalize(vec3(0.4, 0.3, 1.0));

    for (float t = step; t < maxt; t += step) {
        vec3 p = pc.u_CamPos + dir * t;
        vec3 uvw = (p - pc.u_GridMin) * inv_extent;
        if (any(lessThan(uvw, vec3(0.0))) || any(greaterThan(uvw, vec3(1.0)))) {
            continue;
        }
        vec4 v = texture(u_Voxels, uvw);
        if (pc.u_Mode > 1.5) {
            // Radiance volume: every surface is opaque now, so pass through dark walls and only
            // stop at actual emitters (non-zero emission RGB), showing them as glow.
            if (max(v.r, max(v.g, v.b)) > 0.05) {
                FragColor = vec4(v.rgb * 3.0, 1.0);
                return;
            }
        } else if (v.a > 0.3) {
            // Albedo volume: show the stored surface colour directly.
            FragColor = vec4(v.rgb, 1.0);
            return;
        }
    }
    discard;
}
