// texture_constants.rs -- Minimal texture filtering constants
//
// These constants are still actively used by the texture system (vk_image.rs)
// for configuring texture filtering modes. All other legacy GL constants
// have been removed as they're not needed by the Vulkan renderer.

#![allow(dead_code)]

// Texture filtering modes (used in vk_image.rs for sampler configuration)
pub const VK_NEAREST: u32 = 0x2600;
pub const VK_LINEAR: u32 = 0x2601;
pub const VK_NEAREST_MIPMAP_NEAREST: u32 = 0x2700;
pub const VK_LINEAR_MIPMAP_NEAREST: u32 = 0x2701;
pub const VK_NEAREST_MIPMAP_LINEAR: u32 = 0x2702;
pub const VK_LINEAR_MIPMAP_LINEAR: u32 = 0x2703;

// Texture parameters (used in vk_image.rs, vk_draw.rs)
pub const VK_TEXTURE_MIN_FILTER: u32 = 0x2801;
pub const VK_TEXTURE_MAG_FILTER: u32 = 0x2800;
pub const VK_TEXTURE_MAX_ANISOTROPY_EXT: u32 = 0x84FE;

// Texture formats (used in vk_local.rs, vk_rsurf.rs)
pub const VK_RGB8: u32 = 0x8051;
pub const VK_RGBA8: u32 = 0x8058;

// State enable/disable (used in vk_rmisc.rs, vk_draw.rs)
pub const VK_TEXTURE_2D: u32 = 0x0DE1;
pub const VK_BLEND: u32 = 0x0BE2;

// Texture env mode (used in vk_rmisc.rs)
pub const VK_REPLACE: u32 = 0x1E01;
