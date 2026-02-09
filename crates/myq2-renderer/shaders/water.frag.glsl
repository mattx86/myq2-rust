#version 450

layout(location = 0) in vec2 v_TexCoord;
layout(location = 1) in vec3 v_WorldPos;

layout(set = 0, binding = 0) uniform sampler2D u_WaterTexture;

layout(std140, set = 3, binding = 0) uniform FragUniforms {
    float u_Time;
    float u_Alpha;
    int u_IsFlowing;
    float u_ScrollOffset;
};

layout(location = 0) out vec4 FragColor;

// Turbulence scale constant from original: 256.0 / (2.0 * PI)
const float TURBSCALE = 40.743665;

void main() {
    float os = v_TexCoord.s;
    float ot = v_TexCoord.t;

    // Replicate original warp formula:
    // s = os + r_turbsin[(int)((ot*0.125+time) * TURBSCALE) & 255] / 64.0
    // Using sin() directly for smooth result
    float sOffset = sin((ot * 0.125 + u_Time) * 6.28318) * 8.0;
    float tOffset = sin((os * 0.125 + u_Time) * 6.28318) * 8.0;

    vec2 warpedCoord;
    warpedCoord.s = (os + sOffset) / 64.0;
    warpedCoord.t = (ot + tOffset) / 64.0;

    if (u_IsFlowing != 0) {
        warpedCoord.s += u_ScrollOffset / 64.0;
    }

    vec4 waterColor = texture(u_WaterTexture, warpedCoord);
    FragColor = vec4(waterColor.rgb, u_Alpha);
}
