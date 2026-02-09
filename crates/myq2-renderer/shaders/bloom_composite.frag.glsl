#version 450

layout(location = 0) in vec2 v_TexCoord;

layout(set = 0, binding = 0) uniform sampler2D u_SceneTexture;
layout(set = 0, binding = 1) uniform sampler2D u_BloomTexture0;
layout(set = 0, binding = 2) uniform sampler2D u_BloomTexture1;
layout(set = 0, binding = 3) uniform sampler2D u_BloomTexture2;
layout(set = 0, binding = 4) uniform sampler2D u_BloomTexture3;

layout(std140, set = 3, binding = 0) uniform FragUniforms {
    float u_Intensity;
};

layout(location = 0) out vec4 FragColor;

void main() {
    vec3 scene = texture(u_SceneTexture, v_TexCoord).rgb;

    vec3 bloom = texture(u_BloomTexture0, v_TexCoord).rgb
               + texture(u_BloomTexture1, v_TexCoord).rgb
               + texture(u_BloomTexture2, v_TexCoord).rgb
               + texture(u_BloomTexture3, v_TexCoord).rgb;

    FragColor = vec4(scene + bloom * u_Intensity, 1.0);
}
