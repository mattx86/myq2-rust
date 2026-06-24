#version 450

layout(location = 0) in vec3 a_Position;

layout(push_constant) uniform PushConstants {
    mat4  u_MVP;
    vec3  u_LightPos;
    float u_LightRadius;
} pc;

layout(location = 0) out vec3 v_WorldPos;

void main() {
    gl_Position = pc.u_MVP * vec4(a_Position, 1.0);
    v_WorldPos  = a_Position;
}
