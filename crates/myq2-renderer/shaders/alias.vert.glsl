#version 450

layout(location = 0) in vec3 a_Position;
layout(location = 1) in vec3 a_OldPosition;
layout(location = 2) in vec2 a_TexCoord;
layout(location = 3) in int a_NormalIndex;  // kept for future per-vertex lighting

layout(push_constant) uniform PushConstants {
    mat4 u_ModelViewProjection;  // 64 bytes
    vec3 u_ShadeLight;           // 12 bytes
    float u_Alpha;               // 4 bytes  (offset 76)
    vec3 u_Move;                 // 12 bytes
    float u_Backlerp;            // 4 bytes  (offset 92)
    vec3 u_FrontV;               // 12 bytes
    float u_ShellScale;          // 4 bytes  (offset 108)
    vec3 u_BackV;                // 12 bytes
    int u_IsShell;               // 4 bytes  (offset 124)
} pc;                            // Total: 128 bytes

layout(location = 0) out vec2 v_TexCoord;
layout(location = 1) out vec3 v_Color;
layout(location = 2) out float v_Alpha;

void main() {
    // Interpolate position between frames (GL_LerpVerts equivalent)
    vec3 pos = pc.u_Move + a_OldPosition * pc.u_BackV + a_Position * pc.u_FrontV;

    // Shell expansion (simplified — full normal table deferred to follow-up)
    if (pc.u_IsShell != 0) {
        vec3 dir = a_Position - a_OldPosition;
        float len = length(dir);
        if (len > 0.001) {
            pos += normalize(dir) * pc.u_ShellScale;
        } else {
            pos += vec3(0.0, 0.0, 1.0) * pc.u_ShellScale;
        }
    }

    gl_Position = pc.u_ModelViewProjection * vec4(pos, 1.0);
    v_TexCoord = a_TexCoord;
    v_Color = pc.u_ShadeLight;  // flat per-entity lighting
    v_Alpha = pc.u_Alpha;
}
