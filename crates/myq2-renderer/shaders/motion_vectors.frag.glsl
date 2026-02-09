// motion_vectors.frag.glsl - Motion Vector Generation
// Calculates per-pixel motion vectors for temporal effects (FSR 2.0, TAA).
// Outputs velocity in screen-space pixels.

#version 450

layout(location = 0) in vec2 v_TexCoord;
layout(location = 1) in vec4 v_CurrentClipPos;
layout(location = 2) in vec4 v_PrevClipPos;

layout(std140, set = 3, binding = 0) uniform MotionUniforms {
    vec2 u_OutputSize;         // Output resolution
    vec2 u_CurrentJitter;      // Current frame jitter
    vec2 u_PrevJitter;         // Previous frame jitter
    float u_MotionScale;       // Scale factor for motion
    float u_padding;
};

layout(location = 0) out vec2 MotionOutput;

void main() {
    // Convert clip positions to NDC
    vec2 currentNDC = v_CurrentClipPos.xy / v_CurrentClipPos.w;
    vec2 prevNDC = v_PrevClipPos.xy / v_PrevClipPos.w;

    // Remove jitter from positions
    vec2 currentUnjittered = currentNDC - u_CurrentJitter * 2.0 / u_OutputSize;
    vec2 prevUnjittered = prevNDC - u_PrevJitter * 2.0 / u_OutputSize;

    // Calculate motion in NDC space
    vec2 motionNDC = currentUnjittered - prevUnjittered;

    // Convert to pixel space
    vec2 motionPixels = motionNDC * u_OutputSize * 0.5 * u_MotionScale;

    MotionOutput = motionPixels;
}
