#version 450

layout(location = 0) in vec2 a_QuadOffset;
layout(location = 1) in vec3 a_Origin;
layout(location = 2) in vec4 a_Color;
layout(location = 3) in float a_Size;

layout(std140, set = 3, binding = 0) uniform ParticleUniforms {
    mat4 u_ViewProjection;
    vec3 u_ViewUp;
    float u_MinSize;
    vec3 u_ViewRight;
    float u_MaxSize;
    vec3 u_ViewOrigin;
    float _pad0;
};

layout(location = 0) out vec2 v_TexCoord;
layout(location = 1) out vec4 v_Color;

void main() {
    // Distance-based scaling
    float dist = length(a_Origin - u_ViewOrigin);
    float distScale = dist < 20.0 ? 1.0 : 1.0 + dist * 0.004;
    float finalSize = clamp(a_Size * distScale, u_MinSize, u_MaxSize);

    // Billboard expansion
    vec3 right = u_ViewRight * finalSize * 0.667;
    vec3 up = u_ViewUp * finalSize * 0.667;
    vec3 pos = a_Origin + right * a_QuadOffset.x + up * a_QuadOffset.y;

    gl_Position = u_ViewProjection * vec4(pos, 1.0);
    v_TexCoord = a_QuadOffset * 0.5 + 0.5;
    v_Color = a_Color;
}
