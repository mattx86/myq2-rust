#version 450

layout(location = 0) in vec2 v_TexCoord;

layout(set = 1, binding = 0) uniform sampler2D u_SceneTexture;
layout(set = 2, binding = 0) uniform sampler3D u_ColorLUT;

layout(push_constant) uniform PushConstants {
    vec4  u_PolyBlend;         // 16 bytes
    int   u_EnablePolyBlend;   // 4 bytes
    float u_Gamma;             // 4 bytes
    int   u_EnableGamma;       // 4 bytes
    float u_Saturation;        // 4 bytes
    float u_Contrast;          // 4 bytes
    float u_Brightness;        // 4 bytes
    float u_ShadowLift;        // 4 bytes
    int   u_LutEnabled;        // 4 bytes
    float u_LutIntensity;      // 4 bytes
    float u_BloomIntensity;    // 4 bytes — 0 = off
} pc;                          // 60 bytes total

layout(location = 0) out vec4 FragColor;

void main() {
    // FXAA — soften high-contrast edges in screen space.
    // Lightmap seams appear as hard luminance steps at BSP face boundaries; FXAA
    // detects these edges and blends across them without affecting flat surfaces.
    vec2  px  = 1.0 / vec2(textureSize(u_SceneTexture, 0));
    vec3  cC  = texture(u_SceneTexture, v_TexCoord).rgb;
    vec3  cN  = texture(u_SceneTexture, v_TexCoord + vec2( 0.0,  px.y)).rgb;
    vec3  cS  = texture(u_SceneTexture, v_TexCoord + vec2( 0.0, -px.y)).rgb;
    vec3  cE  = texture(u_SceneTexture, v_TexCoord + vec2( px.x,  0.0)).rgb;
    vec3  cW  = texture(u_SceneTexture, v_TexCoord + vec2(-px.x,  0.0)).rgb;
    vec3  lum = vec3(0.2126, 0.7152, 0.0722);
    float lC  = dot(cC, lum),  lN = dot(cN, lum),  lS = dot(cS, lum);
    float lE  = dot(cE, lum),  lW = dot(cW, lum);
    float lMin   = min(lC, min(min(lN, lS), min(lE, lW)));
    float lMax   = max(lC, max(max(lN, lS), max(lE, lW)));
    float range  = lMax - lMin;
    // Only blend where the local contrast is significant (skip flat surfaces / noise).
    if (range >= max(0.0625, lMax * 0.125)) {
        float blend = clamp(range / max(lMax, 0.001), 0.0, 1.0) * 0.5;
        cC = mix(cC, (cN + cS + cE + cW) * 0.25, blend);
    }
    vec4 color = vec4(cC, 1.0);

    // Bloom — single-pass HDR glow folded into the composite.
    // The scene FBO is float16 HDR, so emissive light textures and the sky are boosted well
    // above 1.0 by the world/sky shaders. We gather neighbours, keep only the part above the
    // HDR threshold, blur it radially, and add a soft glow BEFORE ACES so the highlight reads
    // as light spilling out of the fixture rather than a flat bright texel. (The standalone
    // multi-pass bloom in PostProcessor is dead code; this is the live path.)
    if (pc.u_BloomIntensity > 0.0) {
        const float threshold = 1.0;
        vec3  glow = max(cC - threshold, vec3(0.0));
        float wsum = 1.0;
        // Dense, ROTATED rings: 6 rings × 12 taps. Rotating each ring decorrelates the sample
        // directions so a single bright texel spreads into a smooth disc instead of a sparse
        // starburst of discrete ghost dots (the "pixelated" look). Radius is modest so the
        // taps stay dense enough to overlap.
        const int   RINGS = 6;
        const int   TAPS  = 12;
        const float DA    = 6.2831853 / float(TAPS);
        for (int ring = 1; ring <= RINGS; ++ring) {
            float radius = float(ring) * 3.5;          // scene texels
            float w      = exp(-float(ring * ring) / 14.0);
            float rot    = float(ring) * 0.5;          // decorrelate rings
            for (int k = 0; k < TAPS; ++k) {
                float ang = rot + float(k) * DA;
                vec2  off = vec2(cos(ang), sin(ang)) * radius * px;
                glow += max(texture(u_SceneTexture, v_TexCoord + off).rgb - threshold, vec3(0.0)) * w;
                wsum += w;
            }
        }
        glow /= wsum;
        // r_bloom_intensity (default 0.3) scaled so the averaged glow reads without blowing out.
        color.rgb += glow * pc.u_BloomIntensity * 3.0;
    }

    // Polyblend overlay (damage flash, underwater tint)
    if ((pc.u_EnablePolyBlend != 0) && pc.u_PolyBlend.a > 0.0) {
        color.rgb = mix(color.rgb, pc.u_PolyBlend.rgb, pc.u_PolyBlend.a);
    }

    // Gamma correction
    if (pc.u_EnableGamma != 0) {
        color.rgb = pow(color.rgb, vec3(1.0 / pc.u_Gamma));
    }

    // Shadow lift — raises pitch-black floors to avoid crushed shadows
    color.rgb = color.rgb + vec3(pc.u_ShadowLift) * (1.0 - color.rgb);

    // Brightness (exposure-style multiplier)
    color.rgb = color.rgb * pc.u_Brightness;

    // Saturation (luma-preserving desaturate/boost)
    float luma = dot(color.rgb, vec3(0.299, 0.587, 0.114));
    color.rgb = mix(vec3(luma), color.rgb, pc.u_Saturation);

    // Contrast (linear contrast around 0.5 midpoint)
    color.rgb = (color.rgb - 0.5) * pc.u_Contrast + 0.5;

    // Guard against negative values before ACES (contrast/shadow-lift can go slightly negative).
    // Do NOT clamp to 1.0 here — the scene FBO is float16 HDR; we need ACES to see values
    // above 1.0 so it can compress them proportionally across all channels.
    color.rgb = max(color.rgb, vec3(0.0));

    // ACES filmic tone mapping — deepens shadows, compresses blown-out highlights.
    // Gives a cinematic response curve with no new uniforms required.
    // Applied after linear exposure/contrast, before LUT grading.
    {
        float a = 2.51, b = 0.03, c = 2.43, d = 0.59, e = 0.14;
        color.rgb = clamp((color.rgb*(a*color.rgb+b)) / (color.rgb*(c*color.rgb+d)+e), 0.0, 1.0);
    }

    // Color LUT grading (applied last, after all other adjustments)
    if (pc.u_LutEnabled != 0) {
        // Remap [0,1] to texel-center coordinates for a 32-entry LUT axis
        float lutSize = 32.0;
        vec3 lut_coord = color.rgb * (lutSize - 1.0) / lutSize + 0.5 / lutSize;
        vec3 graded = texture(u_ColorLUT, lut_coord).rgb;
        color.rgb = mix(color.rgb, graded, pc.u_LutIntensity);
    }

    FragColor = color;
}
