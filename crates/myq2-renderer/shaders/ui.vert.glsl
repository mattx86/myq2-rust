#version 450

layout(location = 0) in vec2 a_Position;
layout(location = 1) in vec2 a_TexCoord;
layout(location = 2) in vec4 a_Color;

layout(push_constant) uniform PushConstants {
    mat4 u_Projection;
} pc;

layout(location = 0) out vec2 v_TexCoord;
layout(location = 1) out vec4 v_Color;

void main() {
    gl_Position = pc.u_Projection * vec4(a_Position, 0.0, 1.0);
    v_TexCoord = a_TexCoord;
    v_Color = a_Color;
}
