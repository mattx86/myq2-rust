#version 450

layout(location = 0) in vec3 a_Position;

layout(std140, set = 3, binding = 0) uniform DlightUniforms {
    mat4 u_ModelViewProjection;
};

layout(location = 0) out vec3 v_Position;

void main() {
    gl_Position = u_ModelViewProjection * vec4(a_Position, 1.0);
    v_Position = a_Position;
}
