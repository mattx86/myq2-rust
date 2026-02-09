#version 450

layout(location = 0) in vec3 v_TexCoord;

layout(set = 0, binding = 0) uniform samplerCube u_SkyCubemap;

layout(location = 0) out vec4 FragColor;

void main() {
    FragColor = texture(u_SkyCubemap, v_TexCoord);
}
