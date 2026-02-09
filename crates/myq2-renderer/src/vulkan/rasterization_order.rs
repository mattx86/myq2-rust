//! Rasterization Order Attachment Access (VK_EXT_rasterization_order_attachment_access)
//!
//! Control rasterization order for attachment access:
//! - Programmable blending with guaranteed order
//! - Order-independent transparency improvements
//! - Read-modify-write operations in fragment shader
//! - Better deferred rendering patterns

use ash::vk;

/// Rasterization order capabilities.
#[derive(Debug, Clone, Default)]
pub struct RasterizationOrderCapabilities {
    /// Whether rasterization order access is supported.
    pub supported: bool,
    /// Color attachment access.
    pub color_attachment_access: bool,
    /// Depth attachment access.
    pub depth_attachment_access: bool,
    /// Stencil attachment access.
    pub stencil_attachment_access: bool,
}

/// Query rasterization order capabilities.
pub fn query_capabilities(ctx: &super::context::VulkanContext) -> RasterizationOrderCapabilities {
    let mut roa_features = vk::PhysicalDeviceRasterizationOrderAttachmentAccessFeaturesEXT::default();
    let mut features2 = vk::PhysicalDeviceFeatures2::default()
        .push_next(&mut roa_features);

    unsafe {
        ctx.instance.get_physical_device_features2(ctx.physical_device, &mut features2);
    }

    let _ = features2;

    RasterizationOrderCapabilities {
        supported: roa_features.rasterization_order_color_attachment_access == vk::TRUE
            || roa_features.rasterization_order_depth_attachment_access == vk::TRUE
            || roa_features.rasterization_order_stencil_attachment_access == vk::TRUE,
        color_attachment_access: roa_features.rasterization_order_color_attachment_access == vk::TRUE,
        depth_attachment_access: roa_features.rasterization_order_depth_attachment_access == vk::TRUE,
        stencil_attachment_access: roa_features.rasterization_order_stencil_attachment_access == vk::TRUE,
    }
}

/// Rasterization order mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterizationOrder {
    /// Strict primitive order (default).
    Strict,
    /// Relaxed order (implementation-defined).
    Relaxed,
}

/// Pipeline flags for rasterization order.
#[derive(Debug, Clone, Copy, Default)]
pub struct RasterizationOrderFlags {
    /// Enable color attachment access.
    pub color_access: bool,
    /// Enable depth attachment access.
    pub depth_access: bool,
    /// Enable stencil attachment access.
    pub stencil_access: bool,
}

// Rasterization order attachment access flag bits (VK_EXT_rasterization_order_attachment_access)
const PIPELINE_CREATE_RASTERIZATION_ORDER_ATTACHMENT_COLOR_ACCESS_BIT: u32 = 0x00800000;
const PIPELINE_CREATE_RASTERIZATION_ORDER_ATTACHMENT_DEPTH_ACCESS_BIT: u32 = 0x01000000;
const PIPELINE_CREATE_RASTERIZATION_ORDER_ATTACHMENT_STENCIL_ACCESS_BIT: u32 = 0x02000000;

const SUBPASS_DESC_RASTERIZATION_ORDER_ATTACHMENT_COLOR_ACCESS_BIT: u32 = 0x00000010;
const SUBPASS_DESC_RASTERIZATION_ORDER_ATTACHMENT_DEPTH_ACCESS_BIT: u32 = 0x00000020;
const SUBPASS_DESC_RASTERIZATION_ORDER_ATTACHMENT_STENCIL_ACCESS_BIT: u32 = 0x00000040;

impl RasterizationOrderFlags {
    /// Convert to pipeline create flags.
    pub fn to_pipeline_flags(&self) -> vk::PipelineCreateFlags {
        let mut flags_bits = 0u32;

        if self.color_access {
            flags_bits |= PIPELINE_CREATE_RASTERIZATION_ORDER_ATTACHMENT_COLOR_ACCESS_BIT;
        }
        if self.depth_access {
            flags_bits |= PIPELINE_CREATE_RASTERIZATION_ORDER_ATTACHMENT_DEPTH_ACCESS_BIT;
        }
        if self.stencil_access {
            flags_bits |= PIPELINE_CREATE_RASTERIZATION_ORDER_ATTACHMENT_STENCIL_ACCESS_BIT;
        }

        vk::PipelineCreateFlags::from_raw(flags_bits)
    }

    /// Convert to subpass description flags.
    pub fn to_subpass_flags(&self) -> vk::SubpassDescriptionFlags {
        let mut flags_bits = 0u32;

        if self.color_access {
            flags_bits |= SUBPASS_DESC_RASTERIZATION_ORDER_ATTACHMENT_COLOR_ACCESS_BIT;
        }
        if self.depth_access {
            flags_bits |= SUBPASS_DESC_RASTERIZATION_ORDER_ATTACHMENT_DEPTH_ACCESS_BIT;
        }
        if self.stencil_access {
            flags_bits |= SUBPASS_DESC_RASTERIZATION_ORDER_ATTACHMENT_STENCIL_ACCESS_BIT;
        }

        vk::SubpassDescriptionFlags::from_raw(flags_bits)
    }
}

/// GLSL code for rasterization order access.
pub mod glsl {
    /// Extension and layout declarations.
    pub const DECLARATIONS: &str = r#"
#extension GL_EXT_fragment_shader_interlock : enable

// Use one of these layout qualifiers:
// layout(pixel_interlock_ordered) in;     // Strict order within pixel
// layout(pixel_interlock_unordered) in;   // Unordered within pixel
// layout(sample_interlock_ordered) in;    // Strict order per sample
// layout(sample_interlock_unordered) in;  // Unordered per sample
"#;

    /// Programmable blending example.
    pub const PROGRAMMABLE_BLEND: &str = r#"
#version 450
#extension GL_EXT_fragment_shader_interlock : enable

layout(pixel_interlock_ordered) in;

layout(binding = 0, rgba8) uniform image2D framebuffer;

layout(location = 0) in vec4 inColor;
layout(location = 0) out vec4 outColor;

// Custom blend modes
vec4 blendMultiply(vec4 src, vec4 dst) {
    return src * dst;
}

vec4 blendScreen(vec4 src, vec4 dst) {
    return 1.0 - (1.0 - src) * (1.0 - dst);
}

vec4 blendOverlay(vec4 src, vec4 dst) {
    vec4 result;
    for (int i = 0; i < 4; i++) {
        result[i] = dst[i] < 0.5
            ? 2.0 * src[i] * dst[i]
            : 1.0 - 2.0 * (1.0 - src[i]) * (1.0 - dst[i]);
    }
    return result;
}

void main() {
    ivec2 coord = ivec2(gl_FragCoord.xy);

    // Begin critical section
    beginInvocationInterlockARB();

    // Read current framebuffer value
    vec4 dst = imageLoad(framebuffer, coord);

    // Apply custom blend
    vec4 blended = blendOverlay(inColor, dst);

    // Write back
    imageStore(framebuffer, coord, blended);

    // End critical section
    endInvocationInterlockARB();

    outColor = blended;
}
"#;

    /// Order-independent transparency with exact order.
    pub const OIT_EXACT_ORDER: &str = r#"
#version 450
#extension GL_EXT_fragment_shader_interlock : enable

layout(pixel_interlock_ordered) in;

// Per-pixel linked list head pointers
layout(binding = 0, r32ui) uniform uimage2D headPointers;

// Fragment storage
layout(binding = 1, std430) buffer FragmentBuffer {
    uint nextIndex;
    uvec4 fragments[]; // color, depth, next, padding
};

layout(location = 0) in vec4 inColor;
layout(location = 1) in float inDepth;

void main() {
    ivec2 coord = ivec2(gl_FragCoord.xy);

    beginInvocationInterlockARB();

    // Allocate new fragment
    uint newIndex = atomicAdd(nextIndex, 1u);

    // Get current head
    uint oldHead = imageLoad(headPointers, coord).r;

    // Pack color
    uint packedColor = packUnorm4x8(inColor);

    // Insert at sorted position
    uint prev = 0xFFFFFFFFu;
    uint curr = oldHead;

    while (curr != 0xFFFFFFFFu) {
        float currDepth = uintBitsToFloat(fragments[curr].y);
        if (inDepth < currDepth) {
            break;
        }
        prev = curr;
        curr = fragments[curr].z;
    }

    // Store new fragment
    fragments[newIndex] = uvec4(packedColor, floatBitsToUint(inDepth), curr, 0u);

    // Update links
    if (prev == 0xFFFFFFFFu) {
        imageStore(headPointers, coord, uvec4(newIndex));
    } else {
        fragments[prev].z = newIndex;
    }

    endInvocationInterlockARB();

    discard; // Resolve in separate pass
}
"#;

    /// A-buffer accumulation.
    pub const ABUFFER_ACCUMULATION: &str = r#"
#version 450
#extension GL_EXT_fragment_shader_interlock : enable

layout(pixel_interlock_ordered) in;

// A-buffer: fixed-size per-pixel array
#define MAX_FRAGMENTS 8

layout(binding = 0, std430) buffer ABuffer {
    uvec4 fragments[][MAX_FRAGMENTS]; // color, depth per slot
};

layout(binding = 1, r32ui) uniform uimage2D countBuffer;

layout(push_constant) uniform PushConstants {
    uint width;
} pc;

layout(location = 0) in vec4 inColor;

void main() {
    ivec2 coord = ivec2(gl_FragCoord.xy);
    uint pixelIndex = coord.y * pc.width + coord.x;
    float depth = gl_FragCoord.z;

    beginInvocationInterlockARB();

    uint count = imageLoad(countBuffer, coord).r;

    if (count < MAX_FRAGMENTS) {
        // Add to A-buffer
        fragments[pixelIndex][count] = uvec4(
            packUnorm4x8(inColor),
            floatBitsToUint(depth),
            0u, 0u
        );
        imageStore(countBuffer, coord, uvec4(count + 1u));
    } else {
        // A-buffer full - find furthest fragment and replace if closer
        uint furthestIndex = 0u;
        float furthestDepth = 0.0;

        for (uint i = 0u; i < MAX_FRAGMENTS; i++) {
            float d = uintBitsToFloat(fragments[pixelIndex][i].y);
            if (d > furthestDepth) {
                furthestDepth = d;
                furthestIndex = i;
            }
        }

        if (depth < furthestDepth) {
            fragments[pixelIndex][furthestIndex] = uvec4(
                packUnorm4x8(inColor),
                floatBitsToUint(depth),
                0u, 0u
            );
        }
    }

    endInvocationInterlockARB();

    discard;
}
"#;

    /// Deferred decals with blending.
    pub const DEFERRED_DECALS: &str = r#"
#version 450
#extension GL_EXT_fragment_shader_interlock : enable

layout(pixel_interlock_ordered) in;

// GBuffer attachments as images
layout(binding = 0, rgba8) uniform image2D gbufferAlbedo;
layout(binding = 1, rgba16f) uniform image2D gbufferNormal;

layout(binding = 2) uniform sampler2D decalAlbedo;
layout(binding = 3) uniform sampler2D decalNormal;

layout(location = 0) in vec2 inUV;
layout(location = 1) in float inBlend;

void main() {
    ivec2 coord = ivec2(gl_FragCoord.xy);

    vec4 decalColor = texture(decalAlbedo, inUV);
    vec3 decalNorm = texture(decalNormal, inUV).rgb * 2.0 - 1.0;

    beginInvocationInterlockARB();

    // Read current GBuffer
    vec4 currentAlbedo = imageLoad(gbufferAlbedo, coord);
    vec4 currentNormal = imageLoad(gbufferNormal, coord);

    // Blend decal
    float alpha = decalColor.a * inBlend;
    vec4 newAlbedo = mix(currentAlbedo, decalColor, alpha);
    vec3 newNormal = normalize(mix(currentNormal.rgb, decalNorm, alpha));

    // Write back
    imageStore(gbufferAlbedo, coord, newAlbedo);
    imageStore(gbufferNormal, coord, vec4(newNormal, currentNormal.a));

    endInvocationInterlockARB();

    discard;
}
"#;
}

/// Rasterization order manager.
pub struct RasterizationOrderManager {
    capabilities: RasterizationOrderCapabilities,
    flags: RasterizationOrderFlags,
}

impl RasterizationOrderManager {
    /// Create new manager.
    pub fn new(ctx: &super::context::VulkanContext) -> Self {
        let capabilities = query_capabilities(ctx);

        Self {
            capabilities,
            flags: RasterizationOrderFlags::default(),
        }
    }

    /// Check if rasterization order access is supported.
    pub fn is_supported(&self) -> bool {
        self.capabilities.supported
    }

    /// Check if color attachment access is supported.
    pub fn supports_color_access(&self) -> bool {
        self.capabilities.color_attachment_access
    }

    /// Check if depth attachment access is supported.
    pub fn supports_depth_access(&self) -> bool {
        self.capabilities.depth_attachment_access
    }

    /// Check if stencil attachment access is supported.
    pub fn supports_stencil_access(&self) -> bool {
        self.capabilities.stencil_attachment_access
    }

    /// Set flags.
    pub fn set_flags(&mut self, flags: RasterizationOrderFlags) {
        self.flags = flags;
    }

    /// Get current flags.
    pub fn flags(&self) -> &RasterizationOrderFlags {
        &self.flags
    }

    /// Get pipeline create flags.
    pub fn pipeline_flags(&self) -> vk::PipelineCreateFlags {
        self.flags.to_pipeline_flags()
    }

    /// Get subpass description flags.
    pub fn subpass_flags(&self) -> vk::SubpassDescriptionFlags {
        self.flags.to_subpass_flags()
    }
}
