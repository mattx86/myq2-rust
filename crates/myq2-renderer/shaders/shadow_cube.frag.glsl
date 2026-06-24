#version 450

layout(location = 0) in vec3 v_WorldPos;

layout(push_constant) uniform PushConstants {
    mat4  u_MVP;
    vec3  u_LightPos;
    float u_LightRadius;
} pc;

// Write linear dist/radius into the R32_SFLOAT color attachment.
// The depth buffer (D32_SFLOAT) handles z-testing so only the nearest
// fragment's colour value survives.  We do NOT use a depth-format cubemap
// because sampling depth images with a regular samplerCube is unreliable
// across drivers.
//
// NOTE: Use vec4 output (not scalar float) for portability — some drivers
// silently ignore writes from a scalar `out float` to an R32_SFLOAT
// attachment.  The R channel holds the actual depth value; G/B/A are
// unused but must be present.
layout(location = 0) out vec4 o_Depth;

void main() {
    float dist = length(v_WorldPos - pc.u_LightPos);
    float ndist = dist / max(pc.u_LightRadius, 1.0);
    o_Depth = vec4(ndist, 0.0, 0.0, 1.0);
}
