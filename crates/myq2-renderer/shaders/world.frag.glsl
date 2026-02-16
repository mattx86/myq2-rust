#version 450

layout(location = 0) in vec2 v_TexCoord;
layout(location = 1) in vec3 v_LightmapCoordLayer;  // (u, v, layer)

layout(set = 1, binding = 0) uniform sampler2D u_DiffuseTexture;
layout(set = 2, binding = 0) uniform sampler2DArray u_LightmapArray;

layout(push_constant) uniform PushConstants {
    mat4 u_ModelViewProjection;  // 64 bytes (vertex stage, not used here)
    float u_Alpha;               // 4 bytes at offset 64
    float u_OverbrightScale;     // 4 bytes at offset 68
} pc;

layout(location = 0) out vec4 FragColor;

void main() {
    vec4 texColor = texture(u_DiffuseTexture, v_TexCoord);

    // Sample per-pixel lightmap from texture array if layer >= 0
    if (v_LightmapCoordLayer.z >= 0.0) {
        vec4 lightmapColor = texture(u_LightmapArray, v_LightmapCoordLayer);
        // Apply overbright scale to recover dynamic range lost during lightmap build.
        // Lightmap values are stored at reduced scale (divided by r_overbrightbits)
        // to prevent clamping, then scaled back here for smooth light gradients.
        vec3 lit = texColor.rgb * lightmapColor.rgb * pc.u_OverbrightScale;
        FragColor = vec4(lit, texColor.a * pc.u_Alpha);
    } else {
        // No lightmap — full bright
        FragColor = vec4(texColor.rgb, texColor.a * pc.u_Alpha);
    }
}
