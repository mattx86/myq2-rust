//! Vulkan 1.3 renderer backend with ray tracing support.
//!
//! This module replaces the SDL3 GPU abstraction with direct Vulkan access,
//! enabling advanced features like hardware ray tracing for shadows and reflections.

pub mod context;
pub mod surface;
pub mod swapchain;
pub mod memory;
pub mod commands;
pub mod descriptors;
pub mod raytracing;
pub mod render_config;
pub mod samplers;
pub mod vrs;
pub mod mesh_shading;
pub mod upscalers;
pub mod video_decode;
pub mod bindless;
pub mod gpu_culling;
pub mod pipeline_cache;
pub mod timeline_sync;
pub mod video_encode;
pub mod hdr;
pub mod push_descriptors;
pub mod pipeline_library;
pub mod present_wait;
pub mod memory_budget;
pub mod descriptor_buffer;
pub mod dynamic_state3;
pub mod shader_object;
pub mod device_commands;
pub mod memory_priority;
pub mod barycentric;
pub mod subgroup;
pub mod color_grading;
pub mod cooperative_matrix;
pub mod ray_query;
pub mod opacity_micromap;
pub mod host_image_copy;
pub mod nested_command_buffer;
pub mod maintenance;
pub mod image_compression;
pub mod pageable_memory;
pub mod ssr;
pub mod displacement_micromap;
pub mod msaa_optimization;
pub mod shader_tile_image;
pub mod atomic_float;
pub mod low_latency;
pub mod calibrated_timestamps;
pub mod swapchain_maintenance;
pub mod rasterization_order;
pub mod depth_bias_control;
pub mod depth_clip_control;
pub mod color_write_enable;
pub mod pipeline_robustness;
pub mod sdr_to_hdr;
pub mod video_denoise;
pub mod motion_interpolation;

pub use context::VulkanContext;
pub use surface::VulkanSurface;
pub use swapchain::Swapchain;
pub use memory::{Buffer, Image, MemoryManager};
pub use commands::CommandManager;
pub use descriptors::DescriptorManager;
pub use render_config::{RenderConfig, init_render_config, update_render_config, render_config as get_render_config, msaa_samples, is_msaa_enabled, anisotropy_level, is_anisotropy_enabled};
pub use samplers::{SamplerManager, SamplerFilter, SamplerAddress, SamplerKey};

use ash::vk;
use std::ffi::CStr;

/// Check if a Vulkan result is successful, returning an error message if not.
pub fn check_vk_result(result: vk::Result) -> Result<(), String> {
    if result == vk::Result::SUCCESS {
        Ok(())
    } else {
        Err(format!("Vulkan error: {:?}", result))
    }
}

/// Required Vulkan 1.3 features for the renderer.
pub const REQUIRED_VK_VERSION: u32 = vk::make_api_version(0, 1, 3, 0);

/// Application name for Vulkan instance.
pub const APP_NAME: &CStr = c"MyQ2";

/// Engine name for Vulkan instance.
pub const ENGINE_NAME: &CStr = c"MyQ2 Engine";

/// Engine version.
pub const ENGINE_VERSION: u32 = vk::make_api_version(0, 1, 0, 0);
