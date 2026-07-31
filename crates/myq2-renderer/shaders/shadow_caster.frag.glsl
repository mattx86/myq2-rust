#version 450

// Fragment shader for rendering shadow CASTERS into the directional shadow map.
// Reuses alias.vert (frame interpolation) with the light's view-projection as the MVP.
// Outputs TWO values into an RG32F target:
//   R = this fragment's light-space depth (the caster surface).
//   G = the light-space depth of the FLOOR traced straight below the caster (passed in via
//       the otherwise-unused alpha push field -> v_Alpha). The resolve only shadows
//       receivers BETWEEN the caster (R) and its floor (G), so a caster's shadow lands on
//       the surface it actually sits above and can't bleed through it onto things below.
layout(location = 0) in vec2 v_TexCoord;
layout(location = 1) in vec3 v_Color;
layout(location = 2) in float v_Alpha;

layout(location = 0) out vec2 OutDepths;

void main() {
    OutDepths = vec2(gl_FragCoord.z, v_Alpha);
}
