#version 450

layout(location = 0) in vec2 v_TexCoord;

layout(set = 0, binding = 0) uniform sampler2D u_DepthTexture;
layout(set = 0, binding = 1) uniform sampler2D u_NoiseTexture;

layout(std140, set = 3, binding = 0) uniform FragUniforms {
    vec3 u_Samples[64];
    mat4 u_Projection;
    vec2 u_NoiseScale;
    float u_Radius;
    float u_Intensity;
    float u_Near;
    float u_Far;
};

layout(location = 0) out vec4 FragColor;

float linearize_depth(float d) {
    return (2.0 * u_Near * u_Far) / (u_Far + u_Near - (d * 2.0 - 1.0) * (u_Far - u_Near));
}

vec3 view_pos_from_depth(vec2 uv, float depth) {
    float z = linearize_depth(depth);
    vec2 ndc = uv * 2.0 - 1.0;
    vec4 clip = vec4(ndc, depth * 2.0 - 1.0, 1.0);
    vec4 view = inverse(u_Projection) * clip;
    return view.xyz / view.w;
}

void main() {
    float depth = texture(u_DepthTexture, v_TexCoord).r;
    if (depth >= 1.0) {
        FragColor = vec4(1.0); // sky — no occlusion
        return;
    }

    vec3 fragPos = view_pos_from_depth(v_TexCoord, depth);

    // Reconstruct normal from depth via cross product of screen-space derivatives
    vec3 normal = normalize(cross(dFdx(fragPos), dFdy(fragPos)));

    // Random rotation from noise texture
    vec3 randomVec = texture(u_NoiseTexture, v_TexCoord * u_NoiseScale).xyz;

    // Construct TBN matrix
    vec3 tangent = normalize(randomVec - normal * dot(randomVec, normal));
    vec3 bitangent = cross(normal, tangent);
    mat3 TBN = mat3(tangent, bitangent, normal);

    float occlusion = 0.0;
    int kernelSize = 64;
    for (int i = 0; i < kernelSize; ++i) {
        // Transform sample from tangent to view space
        vec3 samplePos = TBN * u_Samples[i];
        samplePos = fragPos + samplePos * u_Radius;

        // Project sample to screen
        vec4 offset = u_Projection * vec4(samplePos, 1.0);
        offset.xyz /= offset.w;
        offset.xyz = offset.xyz * 0.5 + 0.5;

        // Sample depth at projected position
        float sampleDepth = linearize_depth(texture(u_DepthTexture, offset.xy).r);
        float fragDepth = linearize_depth(depth);

        // Range check and accumulate
        float rangeCheck = smoothstep(0.0, 1.0, u_Radius / abs(fragDepth - sampleDepth));
        occlusion += (sampleDepth <= samplePos.z + 0.025 ? 1.0 : 0.0) * rangeCheck;
    }

    occlusion = 1.0 - (occlusion / float(kernelSize)) * u_Intensity;
    FragColor = vec4(vec3(occlusion), 1.0);
}
