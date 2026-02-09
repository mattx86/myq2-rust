#version 450

layout(location = 0) in vec2 v_TexCoord;

layout(set = 0, binding = 0) uniform sampler2D u_InputTexture;

layout(std140, set = 3, binding = 0) uniform FragUniforms {
    vec2 u_InputSize;    // Low-res input dimensions
    vec2 u_OutputSize;   // High-res output dimensions
};

layout(location = 0) out vec4 FragColor;

// Simplified EASU: edge-aware Lanczos-like upsampling
vec3 FsrEasuF(vec2 uv) {
    vec2 inputTexelSize = 1.0 / u_InputSize;
    vec2 p = uv * u_InputSize - 0.5;
    vec2 f = fract(p);
    vec2 ip = floor(p);

    // 12-tap sampling pattern (Lanczos-like with edge detection)
    vec3 a = texture(u_InputTexture, (ip + vec2(-0.5, -0.5)) * inputTexelSize).rgb;
    vec3 b = texture(u_InputTexture, (ip + vec2( 0.5, -0.5)) * inputTexelSize).rgb;
    vec3 c = texture(u_InputTexture, (ip + vec2( 1.5, -0.5)) * inputTexelSize).rgb;
    vec3 d = texture(u_InputTexture, (ip + vec2(-0.5,  0.5)) * inputTexelSize).rgb;
    vec3 e = texture(u_InputTexture, (ip + vec2( 0.5,  0.5)) * inputTexelSize).rgb;
    vec3 ff = texture(u_InputTexture, (ip + vec2( 1.5,  0.5)) * inputTexelSize).rgb;
    vec3 g = texture(u_InputTexture, (ip + vec2(-0.5,  1.5)) * inputTexelSize).rgb;
    vec3 h = texture(u_InputTexture, (ip + vec2( 0.5,  1.5)) * inputTexelSize).rgb;
    vec3 i = texture(u_InputTexture, (ip + vec2( 1.5,  1.5)) * inputTexelSize).rgb;

    // Edge detection: compute local gradients
    float luma_e = dot(e, vec3(0.299, 0.587, 0.114));
    float luma_b = dot(b, vec3(0.299, 0.587, 0.114));
    float luma_d = dot(d, vec3(0.299, 0.587, 0.114));
    float luma_f = dot(ff, vec3(0.299, 0.587, 0.114));
    float luma_h = dot(h, vec3(0.299, 0.587, 0.114));

    float dx = abs(luma_d - luma_f);
    float dy = abs(luma_b - luma_h);
    float edge = max(dx, dy);

    // Sharper kernel when edges are detected, smoother otherwise
    float sharpness = clamp(1.0 - edge * 4.0, 0.0, 1.0);

    // Bilinear base with sharpening
    vec3 bilinear = mix(mix(e, ff, f.x), mix(h, i, f.x), f.y);
    vec3 sharp = e; // Center sample for maximum sharpness

    return mix(sharp, bilinear, 0.5 + sharpness * 0.5);
}

void main() {
    FragColor = vec4(FsrEasuF(v_TexCoord), 1.0);
}
