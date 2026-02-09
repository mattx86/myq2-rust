// shadow.rmiss.glsl - Shadow ray miss shader
// When a shadow ray misses all geometry, the point is fully lit.

#version 460
#extension GL_EXT_ray_tracing : require

layout(location = 0) rayPayloadInEXT float shadowPayload;

void main() {
    // Miss = no occluder = fully lit
    shadowPayload = 1.0;
}
