#version 450

layout(location = 0) in vec2 v_TexCoord;

layout(set = 1, binding = 0) uniform sampler2D u_WaterTexture;

layout(push_constant) uniform PushConstants {
    mat4 u_ModelViewProjection;  // 64 bytes (vertex stage, not used here)
    float u_Alpha;               // 4 bytes at offset 64
    float u_Time;                // 4 bytes at offset 68
    float u_Scroll;              // 4 bytes at offset 72
};

layout(location = 0) out vec4 FragColor;

void main() {
    float os = v_TexCoord.s;
    float ot = v_TexCoord.t;

    // Replicate original Quake 2 turbulent warp formula:
    // s = os + r_turbsin[(int)((ot*0.125+time) * TURBSCALE) & 255] * 0.5
    // t = ot + r_turbsin[(int)((os*0.125+time) * TURBSCALE) & 255] * 0.5
    // Using sin() directly for smooth result (turbsin range is [-8, 8])
    float sOffset = sin((ot * 0.125 + u_Time) * 6.28318) * 8.0;
    float tOffset = sin((os * 0.125 + u_Time) * 6.28318) * 8.0;

    vec2 warpedCoord;
    warpedCoord.s = (os + sOffset) / 64.0;
    warpedCoord.t = (ot + tOffset) / 64.0;

    if (u_Scroll != 0.0) {
        warpedCoord.s += u_Scroll / 64.0;
    }

    vec4 waterColor = texture(u_WaterTexture, warpedCoord);
    FragColor = vec4(waterColor.rgb, u_Alpha);
}
