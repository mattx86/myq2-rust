#version 450

layout(location = 0) in vec2 v_TexCoord;
layout(location = 1) in vec2 v_LightmapCoord;

layout(set = 0, binding = 0) uniform sampler2D u_DiffuseTexture;
layout(set = 0, binding = 1) uniform sampler2D u_LightmapTexture;
layout(set = 0, binding = 2) uniform sampler2D u_OverlayTexture; // detail or caustic

layout(std140, set = 3, binding = 0) uniform FragUniforms {
    float u_OverbrightScale;  // 1.0, 2.0, or 4.0
    int u_Fullbright;
    int u_SaturateLighting;   // clamp lightmap to [0,1] before multiply
    int u_EnableDetail;       // enable detail texture overlay
    float u_DetailScale;      // detail texture UV scale (typically 8.0)
    int u_EnableCaustics;     // enable caustic overlay on underwater surfaces
    float u_CausticScroll;    // time-based UV scroll offset
    int u_IsUnderwater;       // per-draw: surface is underwater
    int u_LightmapOnly;       // debug: show lightmap only (vk_lightmap)
};

layout(location = 0) out vec4 FragColor;

void main() {
    vec4 diffuse = texture(u_DiffuseTexture, v_TexCoord);

    if (u_Fullbright != 0) {
        FragColor = diffuse;
    } else {
        vec4 lightmap = texture(u_LightmapTexture, v_LightmapCoord);

        // Saturate lighting: clamp lightmap values to prevent overbright
        if (u_SaturateLighting != 0) {
            lightmap.rgb = clamp(lightmap.rgb, 0.0, 1.0);
        }

        // Lightmap-only debug view (vk_lightmap cvar)
        if (u_LightmapOnly != 0) {
            FragColor = vec4(lightmap.rgb * u_OverbrightScale, 1.0);
            return;
        }

        // GL_MODULATE: diffuse * lightmap
        vec3 color = diffuse.rgb * lightmap.rgb * u_OverbrightScale;

        // Detail texture overlay for non-underwater surfaces
        if (u_EnableDetail != 0 && u_IsUnderwater == 0) {
            vec2 detailUV = v_TexCoord * u_DetailScale;
            vec3 detail = texture(u_OverlayTexture, detailUV).rgb;
            // Detail centered around 0.5 gray: multiply by 2 so 0.5 = no change
            color *= detail * 2.0;
        }

        // Caustic overlay for underwater surfaces
        if (u_EnableCaustics != 0 && u_IsUnderwater != 0) {
            vec2 causticUV = v_TexCoord + vec2(u_CausticScroll);
            vec3 caustic = texture(u_OverlayTexture, causticUV).rgb;
            // Additive-ish blend: brighten underwater surfaces with caustic pattern
            color *= (1.0 + caustic * 0.3);
        }

        FragColor = vec4(color, diffuse.a);
    }
}
