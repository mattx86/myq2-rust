// fsr2_temporal.frag.glsl - FSR 2.0-style Temporal Accumulation
// Implements robust temporal upscaling with:
// - History reprojection using motion vectors
// - Neighborhood clamping to prevent ghosting
// - Confidence-based blending
// - Sub-pixel jitter awareness

#version 450

layout(location = 0) in vec2 v_TexCoord;

// Current frame (low-res, jittered)
layout(set = 0, binding = 0) uniform sampler2D u_CurrentFrame;
// History buffer (high-res, accumulated)
layout(set = 0, binding = 1) uniform sampler2D u_HistoryFrame;
// Motion vectors (RG16F, in pixels at output resolution)
layout(set = 0, binding = 2) uniform sampler2D u_MotionVectors;
// Depth buffer for disocclusion detection
layout(set = 0, binding = 3) uniform sampler2D u_DepthBuffer;
// Previous depth for temporal comparison
layout(set = 0, binding = 4) uniform sampler2D u_PrevDepthBuffer;

layout(std140, set = 3, binding = 0) uniform FSR2Uniforms {
    vec2 u_InputSize;          // Low-res input dimensions
    vec2 u_OutputSize;         // High-res output dimensions
    vec2 u_Jitter;             // Current frame jitter offset (in pixels)
    float u_HistoryWeight;     // Base history blend weight (0.9-0.95)
    float u_DepthThreshold;    // Disocclusion depth threshold
    float u_LumaStabilization; // Luma-based stability factor
    float u_MotionScale;       // Motion vector scaling
    float u_ColorClampScale;   // Neighborhood clamp box expansion
    float u_Sharpness;         // Output sharpening amount
};

layout(location = 0) out vec4 FragColor;

// Convert RGB to luma
float rgbToLuma(vec3 rgb) {
    return dot(rgb, vec3(0.299, 0.587, 0.114));
}

// YCoCg color space for better chroma handling
vec3 rgbToYCoCg(vec3 rgb) {
    return vec3(
        rgb.r * 0.25 + rgb.g * 0.5 + rgb.b * 0.25,
        rgb.r * 0.5 - rgb.b * 0.5,
        rgb.r * -0.25 + rgb.g * 0.5 + rgb.b * -0.25
    );
}

vec3 yCoCgToRgb(vec3 ycocg) {
    return vec3(
        ycocg.x + ycocg.y - ycocg.z,
        ycocg.x + ycocg.z,
        ycocg.x - ycocg.y - ycocg.z
    );
}

// Sample current frame with jitter compensation
vec3 sampleCurrentJittered(vec2 uv) {
    vec2 jitteredUV = uv + u_Jitter / u_OutputSize;
    return texture(u_CurrentFrame, jitteredUV).rgb;
}

// Get 3x3 neighborhood min/max in YCoCg space
void getNeighborhoodMinMax(vec2 uv, out vec3 minColor, out vec3 maxColor, out vec3 avgColor) {
    vec2 texelSize = 1.0 / u_InputSize;
    vec3 samples[9];
    int idx = 0;

    // Sample 3x3 neighborhood
    for (int y = -1; y <= 1; y++) {
        for (int x = -1; x <= 1; x++) {
            vec2 offset = vec2(float(x), float(y)) * texelSize;
            vec3 s = texture(u_CurrentFrame, uv + offset).rgb;
            samples[idx++] = rgbToYCoCg(s);
        }
    }

    // Find min, max, and average
    minColor = samples[0];
    maxColor = samples[0];
    avgColor = samples[0];

    for (int i = 1; i < 9; i++) {
        minColor = min(minColor, samples[i]);
        maxColor = max(maxColor, samples[i]);
        avgColor += samples[i];
    }
    avgColor /= 9.0;

    // Expand box based on local variance (reduces flickering)
    vec3 boxSize = (maxColor - minColor) * 0.5;
    vec3 boxCenter = (maxColor + minColor) * 0.5;

    // Scale the box
    minColor = boxCenter - boxSize * u_ColorClampScale;
    maxColor = boxCenter + boxSize * u_ColorClampScale;
}

// Clip color to AABB
vec3 clipToAABB(vec3 color, vec3 minColor, vec3 maxColor) {
    vec3 center = (maxColor + minColor) * 0.5;
    vec3 extents = (maxColor - minColor) * 0.5;

    vec3 offset = color - center;
    vec3 unit = offset / max(extents, vec3(0.0001));
    vec3 absUnit = abs(unit);
    float maxComp = max(absUnit.x, max(absUnit.y, absUnit.z));

    if (maxComp > 1.0) {
        return center + offset / maxComp;
    }
    return color;
}

// Catmull-Rom filter for history sampling
vec3 sampleHistoryCatmullRom(vec2 uv) {
    vec2 texelSize = 1.0 / u_OutputSize;
    vec2 pos = uv * u_OutputSize;
    vec2 center = floor(pos - 0.5) + 0.5;

    vec2 f = pos - center;
    vec2 f2 = f * f;
    vec2 f3 = f2 * f;

    // Catmull-Rom weights
    vec2 w0 = -0.5 * f3 + f2 - 0.5 * f;
    vec2 w1 = 1.5 * f3 - 2.5 * f2 + 1.0;
    vec2 w2 = -1.5 * f3 + 2.0 * f2 + 0.5 * f;
    vec2 w3 = 0.5 * f3 - 0.5 * f2;

    // Optimized: collapse to 4 bilinear samples
    vec2 w12 = w1 + w2;
    vec2 tc12 = (center + w2 / w12) * texelSize;
    vec2 tc0 = (center - 1.0) * texelSize;
    vec2 tc3 = (center + 2.0) * texelSize;

    vec3 result =
        (texture(u_HistoryFrame, vec2(tc12.x, tc0.y)).rgb * w12.x +
         texture(u_HistoryFrame, vec2(tc0.x, tc12.y)).rgb * w0.x +
         texture(u_HistoryFrame, vec2(tc12.x, tc12.y)).rgb * w12.x +
         texture(u_HistoryFrame, vec2(tc3.x, tc12.y)).rgb * w3.x) * w0.y;
    result +=
        (texture(u_HistoryFrame, vec2(tc12.x, tc12.y)).rgb * w12.x +
         texture(u_HistoryFrame, vec2(tc0.x, tc12.y)).rgb * w0.x +
         texture(u_HistoryFrame, vec2(tc3.x, tc12.y)).rgb * w3.x) * w12.y;
    result +=
        (texture(u_HistoryFrame, vec2(tc12.x, tc3.y)).rgb * w12.x +
         texture(u_HistoryFrame, vec2(tc0.x, tc3.y)).rgb * w0.x +
         texture(u_HistoryFrame, vec2(tc3.x, tc3.y)).rgb * w3.x) * w3.y;

    return max(result, vec3(0.0));
}

void main() {
    vec2 uv = v_TexCoord;
    vec2 outputUV = uv;

    // Sample motion vector (already in output pixel space)
    vec2 motion = texture(u_MotionVectors, uv).rg * u_MotionScale;
    vec2 historyUV = uv - motion / u_OutputSize;

    // Check for valid history coordinates
    bool historyValid = historyUV.x >= 0.0 && historyUV.x <= 1.0 &&
                        historyUV.y >= 0.0 && historyUV.y <= 1.0;

    // Depth-based disocclusion detection
    float currentDepth = texture(u_DepthBuffer, uv).r;
    float historyDepth = texture(u_PrevDepthBuffer, historyUV).r;
    float depthDiff = abs(currentDepth - historyDepth);
    bool disoccluded = depthDiff > u_DepthThreshold;

    // Sample current frame
    vec3 currentColor = sampleCurrentJittered(uv);
    vec3 currentYCoCg = rgbToYCoCg(currentColor);

    // Get neighborhood color bounds
    vec3 minYCoCg, maxYCoCg, avgYCoCg;
    getNeighborhoodMinMax(uv, minYCoCg, maxYCoCg, avgYCoCg);

    vec3 result;

    if (historyValid && !disoccluded) {
        // Sample history with high-quality filter
        vec3 historyColor = sampleHistoryCatmullRom(historyUV);
        vec3 historyYCoCg = rgbToYCoCg(historyColor);

        // Clip history to neighborhood AABB (prevents ghosting)
        vec3 clampedHistoryYCoCg = clipToAABB(historyYCoCg, minYCoCg, maxYCoCg);

        // Calculate confidence based on how much we had to clip
        float clipDist = length(historyYCoCg - clampedHistoryYCoCg);
        float confidence = exp(-clipDist * 10.0);

        // Motion-based weight reduction (fast motion = less history)
        float motionLength = length(motion);
        float motionWeight = 1.0 / (1.0 + motionLength * 0.1);

        // Luma-based stability (high contrast = less history)
        float lumaContrast = (maxYCoCg.x - minYCoCg.x);
        float lumaStability = exp(-lumaContrast * u_LumaStabilization);

        // Final history weight
        float historyWeight = u_HistoryWeight * confidence * motionWeight * lumaStability;
        historyWeight = clamp(historyWeight, 0.0, 0.97);

        // Blend current and clamped history
        vec3 resultYCoCg = mix(currentYCoCg, clampedHistoryYCoCg, historyWeight);
        result = yCoCgToRgb(resultYCoCg);
    } else {
        // No valid history - use current frame only
        result = currentColor;
    }

    // Optional sharpening pass
    if (u_Sharpness > 0.0) {
        vec2 texelSize = 1.0 / u_OutputSize;
        vec3 neighbors =
            texture(u_CurrentFrame, uv + vec2(-texelSize.x, 0.0)).rgb +
            texture(u_CurrentFrame, uv + vec2(texelSize.x, 0.0)).rgb +
            texture(u_CurrentFrame, uv + vec2(0.0, -texelSize.y)).rgb +
            texture(u_CurrentFrame, uv + vec2(0.0, texelSize.y)).rgb;
        neighbors *= 0.25;

        vec3 sharpened = result + (result - neighbors) * u_Sharpness;
        result = max(sharpened, vec3(0.0));
    }

    FragColor = vec4(result, 1.0);
}
