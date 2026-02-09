#version 450

layout(location = 0) in vec2 v_TexCoord;

layout(set = 0, binding = 0) uniform sampler2D u_SceneTexture;

layout(std140, set = 3, binding = 0) uniform FragUniforms {
    vec2 u_InverseResolution; // 1.0 / screen_size
};

layout(location = 0) out vec4 FragColor;

// Compute luminance from RGB
float FxaaLuma(vec3 rgb) {
    return rgb.g * (0.587/0.299) + rgb.r;
}

void main() {
    vec2 uv = v_TexCoord;
    vec2 inv = u_InverseResolution;

    // Sample center and 4 neighbors
    vec3 rgbM  = texture(u_SceneTexture, uv).rgb;
    vec3 rgbNW = texture(u_SceneTexture, uv + vec2(-1.0, -1.0) * inv).rgb;
    vec3 rgbNE = texture(u_SceneTexture, uv + vec2( 1.0, -1.0) * inv).rgb;
    vec3 rgbSW = texture(u_SceneTexture, uv + vec2(-1.0,  1.0) * inv).rgb;
    vec3 rgbSE = texture(u_SceneTexture, uv + vec2( 1.0,  1.0) * inv).rgb;

    float lumaM  = FxaaLuma(rgbM);
    float lumaNW = FxaaLuma(rgbNW);
    float lumaNE = FxaaLuma(rgbNE);
    float lumaSW = FxaaLuma(rgbSW);
    float lumaSE = FxaaLuma(rgbSE);

    float lumaMin = min(lumaM, min(min(lumaNW, lumaNE), min(lumaSW, lumaSE)));
    float lumaMax = max(lumaM, max(max(lumaNW, lumaNE), max(lumaSW, lumaSE)));
    float lumaRange = lumaMax - lumaMin;

    // Skip anti-aliasing if contrast is low
    float FXAA_EDGE_THRESHOLD = 0.125;
    float FXAA_EDGE_THRESHOLD_MIN = 0.0625;
    if (lumaRange < max(FXAA_EDGE_THRESHOLD_MIN, lumaMax * FXAA_EDGE_THRESHOLD)) {
        FragColor = vec4(rgbM, 1.0);
        return;
    }

    // Compute edge direction
    vec2 dir;
    dir.x = -((lumaNW + lumaNE) - (lumaSW + lumaSE));
    dir.y =  ((lumaNW + lumaSW) - (lumaNE + lumaSE));

    float dirReduce = max((lumaNW + lumaNE + lumaSW + lumaSE) * 0.03125, 1.0/128.0);
    float rcpDirMin = 1.0 / (min(abs(dir.x), abs(dir.y)) + dirReduce);

    dir = min(vec2(8.0), max(vec2(-8.0), dir * rcpDirMin)) * inv;

    // Sample along the edge
    vec3 rgbA = 0.5 * (
        texture(u_SceneTexture, uv + dir * (1.0/3.0 - 0.5)).rgb +
        texture(u_SceneTexture, uv + dir * (2.0/3.0 - 0.5)).rgb
    );
    vec3 rgbB = rgbA * 0.5 + 0.25 * (
        texture(u_SceneTexture, uv + dir * -0.5).rgb +
        texture(u_SceneTexture, uv + dir *  0.5).rgb
    );

    float lumaB = FxaaLuma(rgbB);
    if (lumaB < lumaMin || lumaB > lumaMax) {
        FragColor = vec4(rgbA, 1.0);
    } else {
        FragColor = vec4(rgbB, 1.0);
    }
}
