#version 450

layout(location = 0) in vec3 a_Position;
layout(location = 1) in vec2 a_TexCoord;

layout(std140, set = 3, binding = 0) uniform WaterUniforms {
    mat4 u_ModelViewProjection;
    float u_Time;
    float u_WaveAmplitude;
    int u_EnableWaves;  // bool not portable in std140; test as != 0
};

layout(location = 0) out vec2 v_TexCoord;
layout(location = 1) out vec3 v_WorldPos;

void main() {
    vec3 pos = a_Position;

    // Water wave vertex deformation
    if (u_EnableWaves != 0) {
        pos.z += u_WaveAmplitude * sin(pos.x * 0.025 + u_Time)
                                 * sin(pos.z * 0.05 + u_Time);
    }

    gl_Position = u_ModelViewProjection * vec4(pos, 1.0);
    v_TexCoord = a_TexCoord;
    v_WorldPos = pos;
}
