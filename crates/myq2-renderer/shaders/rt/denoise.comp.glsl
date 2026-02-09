// denoise.comp.glsl - Temporal denoising compute shader
// Accumulates samples over time with neighborhood clamping to reduce noise
// from stochastic ray tracing while preventing ghosting artifacts.

#version 460

layout(local_size_x = 8, local_size_y = 8, local_size_z = 1) in;

// Current frame input (noisy)
layout(set = 0, binding = 0) uniform sampler2D currentFrame;
// Previous frame (denoised history)
layout(set = 0, binding = 1) uniform sampler2D historyFrame;
// Motion vectors for reprojection
layout(set = 0, binding = 2) uniform sampler2D motionVectors;
// Depth buffer for edge detection
layout(set = 0, binding = 3) uniform sampler2D depthBuffer;
// Previous depth for disocclusion detection
layout(set = 0, binding = 4) uniform sampler2D prevDepthBuffer;

// Denoised output
layout(set = 0, binding = 5, rgba16f) uniform image2D outputImage;

layout(set = 1, binding = 0) uniform DenoiseParams {
    float blendFactor;          // Temporal blend (0.9-0.95 typical)
    float colorBoxScale;        // Neighborhood clamp scale (1.0-2.0)
    float motionScale;          // Motion vector scale
    float depthThreshold;       // Disocclusion depth threshold
    vec2 texelSize;
    vec2 padding;
} params;

// Sample neighborhood min/max for clamping
void getNeighborhoodMinMax(vec2 uv, out vec3 minColor, out vec3 maxColor) {
    vec3 samples[9];
    int idx = 0;

    // 3x3 neighborhood
    for (int y = -1; y <= 1; y++) {
        for (int x = -1; x <= 1; x++) {
            vec2 offset = vec2(float(x), float(y)) * params.texelSize;
            samples[idx++] = texture(currentFrame, uv + offset).rgb;
        }
    }

    // Calculate min and max
    minColor = samples[0];
    maxColor = samples[0];
    for (int i = 1; i < 9; i++) {
        minColor = min(minColor, samples[i]);
        maxColor = max(maxColor, samples[i]);
    }

    // Expand the box slightly to reduce flickering
    vec3 center = samples[4];  // Center sample
    vec3 boxSize = (maxColor - minColor) * 0.5;
    minColor = center - boxSize * params.colorBoxScale;
    maxColor = center + boxSize * params.colorBoxScale;
}

// Clamp color to neighborhood box
vec3 clipToBox(vec3 color, vec3 minColor, vec3 maxColor) {
    return clamp(color, minColor, maxColor);
}

void main() {
    ivec2 pixel = ivec2(gl_GlobalInvocationID.xy);
    ivec2 size = imageSize(outputImage);

    if (pixel.x >= size.x || pixel.y >= size.y) {
        return;
    }

    vec2 uv = (vec2(pixel) + 0.5) / vec2(size);

    // Sample current frame
    vec3 currentColor = texture(currentFrame, uv).rgb;

    // Sample motion vector and reproject
    vec2 motion = texture(motionVectors, uv).rg * params.motionScale;
    vec2 historyUV = uv - motion;

    // Check for disocclusion
    float currentDepth = texture(depthBuffer, uv).r;
    float historyDepth = texture(prevDepthBuffer, historyUV).r;
    bool disoccluded = abs(currentDepth - historyDepth) > params.depthThreshold;

    // Check if history UV is valid (inside screen)
    bool historyValid = historyUV.x >= 0.0 && historyUV.x <= 1.0 &&
                        historyUV.y >= 0.0 && historyUV.y <= 1.0 &&
                        !disoccluded;

    vec3 result;

    if (historyValid) {
        // Sample history
        vec3 historyColor = texture(historyFrame, historyUV).rgb;

        // Get neighborhood color bounds for clamping
        vec3 minColor, maxColor;
        getNeighborhoodMinMax(uv, minColor, maxColor);

        // Clamp history to neighborhood to prevent ghosting
        vec3 clampedHistory = clipToBox(historyColor, minColor, maxColor);

        // Calculate confidence based on how much we had to clamp
        float historyDiff = length(historyColor - clampedHistory);
        float confidence = exp(-historyDiff * 10.0);

        // Blend factor adjusted by confidence
        float blend = params.blendFactor * confidence;

        // Temporal accumulation
        result = mix(currentColor, clampedHistory, blend);
    } else {
        // No valid history - use current frame only
        result = currentColor;
    }

    imageStore(outputImage, pixel, vec4(result, 1.0));
}
