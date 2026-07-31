#version 450

layout(location = 0) in vec2 v_TexCoord;

layout(set = 1, binding = 0) uniform sampler2D u_SkyTexture;

layout(location = 0) out vec4 FragColor;

// Treat the sky as an emitter: push it brighter than 1.0 into HDR so the bloom pass blooms
// it (a glow halo where the sky meets geometry) and it reads as a light source rather than a
// flat backdrop.
const float SKY_EMIT = 2.5;

void main() {
    vec3 sky = texture(u_SkyTexture, v_TexCoord).rgb;
    FragColor = vec4(sky * SKY_EMIT, 1.0);
}
