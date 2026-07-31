#version 450

// Vertex shader for rendering BSP/brush geometry (movers: lifts, doors) as shadow CASTERS
// into the directional shadow map. Position-only; the light's view-projection (× the
// model matrix for the mover) is the MVP. The floor depth is supplied as a push constant
// and forwarded as v_Alpha so the shared shadow_caster.frag writes it to the G channel
// (bounding the mover's shadow to the surface below it, like alias casters).
layout(location = 0) in vec3 a_Position; // BspVertex position (stride 60, offset 0)

layout(push_constant) uniform PushConstants {
    mat4  u_MVP;        // light_view_proj * model
    float u_FloorDepth; // light-space depth of the floor below the mover
} pc;

layout(location = 0) out vec2 v_TexCoord;
layout(location = 1) out vec3 v_Color;
layout(location = 2) out float v_Alpha;

void main() {
    gl_Position = pc.u_MVP * vec4(a_Position, 1.0);
    v_TexCoord = vec2(0.0);
    v_Color = vec3(0.0);
    v_Alpha = pc.u_FloorDepth;
}
