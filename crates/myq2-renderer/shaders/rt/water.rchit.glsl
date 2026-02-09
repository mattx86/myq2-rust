// water.rchit.glsl - Water surface closest-hit shader
// Computes Fresnel-blended reflection and refraction for water surfaces.
// Uses wave normal perturbation for realistic water appearance.

#version 460
#extension GL_EXT_ray_tracing : require
#extension GL_EXT_nonuniform_qualifier : require

layout(set = 0, binding = 0) uniform accelerationStructureEXT topLevelAS;

// Bindless textures
layout(set = 2, binding = 0) uniform sampler2D textures[];

// Water parameters
layout(set = 3, binding = 0) uniform WaterUBO {
    float time;
    float waveScale;
    float waveSpeed;
    float refractionIndex;  // ~1.33 for water
    vec4 waterTint;         // Color tint for water
    vec4 deepWaterColor;    // Color at depth
    float maxVisibleDepth;  // How far we can see underwater
    float fresnelPower;
    float padding[2];
} water;

struct ReflectionPayload {
    vec3 color;
    float hitDistance;
};

layout(location = 0) rayPayloadInEXT ReflectionPayload payload;
layout(location = 1) rayPayloadEXT ReflectionPayload secondaryPayload;

hitAttributeEXT vec2 attribs;

// Generate wave normal perturbation
vec3 waveNormal(vec3 worldPos, float time) {
    // Multi-octave wave pattern
    float wave1 = sin(worldPos.x * water.waveScale + time * water.waveSpeed) *
                  cos(worldPos.y * water.waveScale * 0.7 + time * water.waveSpeed * 1.3);
    float wave2 = sin(worldPos.x * water.waveScale * 2.3 - time * water.waveSpeed * 0.8) *
                  cos(worldPos.y * water.waveScale * 1.9 + time * water.waveSpeed * 0.5);

    // Combine waves into normal offset
    vec3 normal = vec3(
        wave1 * 0.15 + wave2 * 0.05,
        wave1 * 0.1 - wave2 * 0.08,
        1.0
    );
    return normalize(normal);
}

// Fresnel-Schlick with roughness
float fresnelSchlick(float cosTheta, float F0) {
    float f = F0 + (1.0 - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), water.fresnelPower);
    return clamp(f, 0.0, 1.0);
}

void main() {
    // Calculate world-space hit position
    vec3 worldPos = gl_WorldRayOriginEXT + gl_WorldRayDirectionEXT * gl_HitTEXT;

    // Get perturbed wave normal
    vec3 waveN = waveNormal(worldPos, water.time);

    // Transform wave normal to world space (assume water is horizontal)
    vec3 normal = normalize(vec3(waveN.x, waveN.y, waveN.z));

    // View direction
    vec3 viewDir = normalize(-gl_WorldRayDirectionEXT);

    // Calculate Fresnel term
    float cosTheta = max(dot(viewDir, normal), 0.0);
    float fresnel = fresnelSchlick(cosTheta, 0.02);  // Water F0 ~0.02

    // Reflection direction
    vec3 reflectDir = reflect(-viewDir, normal);

    // Refraction direction (entering water)
    float eta = 1.0 / water.refractionIndex;
    vec3 refractDir = refract(-viewDir, normal, eta);

    // Total internal reflection check
    bool totalInternalReflection = length(refractDir) < 0.001;
    if (totalInternalReflection) {
        fresnel = 1.0;
    }

    vec3 origin = worldPos + normal * 0.01;

    // Trace reflection ray
    secondaryPayload.color = vec3(0.0);
    secondaryPayload.hitDistance = -1.0;

    traceRayEXT(
        topLevelAS,
        gl_RayFlagsNoneEXT,
        0xFF,
        1,              // Reflection hit group
        0,
        1,              // Reflection miss
        origin,
        0.001,
        reflectDir,
        10000.0,
        1               // Secondary payload location
    );

    vec3 reflectionColor = secondaryPayload.color;

    // Trace refraction ray (if not total internal reflection)
    vec3 refractionColor = water.deepWaterColor.rgb;
    if (!totalInternalReflection) {
        vec3 refractionOrigin = worldPos - normal * 0.01;  // Bias into water

        secondaryPayload.color = vec3(0.0);
        secondaryPayload.hitDistance = -1.0;

        traceRayEXT(
            topLevelAS,
            gl_RayFlagsNoneEXT,
            0xFF,
            1,              // Same hit group
            0,
            1,
            refractionOrigin,
            0.001,
            refractDir,
            water.maxVisibleDepth,
            1
        );

        if (secondaryPayload.hitDistance > 0.0) {
            // Apply depth-based absorption
            float depth = secondaryPayload.hitDistance;
            float absorption = exp(-depth * 0.1);
            refractionColor = mix(water.deepWaterColor.rgb, secondaryPayload.color, absorption);
        }
    }

    // Apply water tint to refracted color
    refractionColor *= water.waterTint.rgb;

    // Blend reflection and refraction based on Fresnel
    vec3 finalColor = mix(refractionColor, reflectionColor, fresnel);

    payload.color = finalColor;
    payload.hitDistance = gl_HitTEXT;
}
