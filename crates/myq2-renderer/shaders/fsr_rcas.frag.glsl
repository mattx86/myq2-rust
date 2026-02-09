#version 450

layout(location = 0) in vec2 v_TexCoord;

layout(set = 0, binding = 0) uniform sampler2D u_InputTexture;

layout(std140, set = 3, binding = 0) uniform FragUniforms {
    float u_Sharpness;
    float _pad0;
    vec2 u_TexelSize;
};

layout(location = 0) out vec4 FragColor;

void main() {
    vec2 uv = v_TexCoord;

    // 5-tap cross pattern
    vec3 e = texture(u_InputTexture, uv).rgb;
    vec3 b = texture(u_InputTexture, uv + vec2( 0.0, -1.0) * u_TexelSize).rgb;
    vec3 d = texture(u_InputTexture, uv + vec2(-1.0,  0.0) * u_TexelSize).rgb;
    vec3 f = texture(u_InputTexture, uv + vec2( 1.0,  0.0) * u_TexelSize).rgb;
    vec3 h = texture(u_InputTexture, uv + vec2( 0.0,  1.0) * u_TexelSize).rgb;

    // Compute min/max of the cross
    vec3 mn = min(min(min(b, d), min(f, h)), e);
    vec3 mx = max(max(max(b, d), max(f, h)), e);

    // Compute RCAS weight
    vec3 amp = clamp(min(mn, 2.0 - mx) / mx, 0.0, 1.0);
    amp = sqrt(amp);
    float peak = -1.0 / mix(8.0, 5.0, clamp(u_Sharpness, 0.0, 1.0));

    // Apply sharpening
    vec3 w = amp * peak;
    vec3 rcas = clamp((b * w + d * w + f * w + h * w + e) / (4.0 * w + 1.0), 0.0, 1.0);

    FragColor = vec4(rcas, 1.0);
}
