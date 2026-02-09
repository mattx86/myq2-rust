#version 450

layout(location = 0) in vec3 a_Position;

layout(std140, set = 3, binding = 0) uniform SkyUniforms {
    mat4 u_ViewProjection;
    vec3 u_SkyAxis;
    float u_SkyRotate;
    float u_Time;
};

layout(location = 0) out vec3 v_TexCoord;

mat3 rotationMatrix(vec3 axis, float angle) {
    float s = sin(angle);
    float c = cos(angle);
    float oc = 1.0 - c;
    return mat3(
        oc * axis.x * axis.x + c,           oc * axis.x * axis.y - axis.z * s,  oc * axis.z * axis.x + axis.y * s,
        oc * axis.x * axis.y + axis.z * s,  oc * axis.y * axis.y + c,           oc * axis.y * axis.z - axis.x * s,
        oc * axis.z * axis.x - axis.y * s,  oc * axis.y * axis.z + axis.x * s,  oc * axis.z * axis.z + c
    );
}

void main() {
    float angle = radians(u_SkyRotate * u_Time);
    mat3 rot = rotationMatrix(normalize(u_SkyAxis), angle);
    v_TexCoord = rot * a_Position;

    vec4 pos = u_ViewProjection * vec4(a_Position, 1.0);
    gl_Position = pos.xyww;  // Force to far plane
}
