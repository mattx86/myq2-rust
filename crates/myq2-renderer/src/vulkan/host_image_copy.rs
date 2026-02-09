//! Host Image Copy for Direct CPU-GPU Image Transfers
//!
//! VK_EXT_host_image_copy enables direct image copies without staging buffers:
//! - Copy directly from host memory to device images
//! - Copy from device images to host memory
//! - Transition image layouts from host
//! - Reduced memory usage and faster uploads for some cases

use ash::vk;

/// Host image copy capabilities.
#[derive(Debug, Clone, Default)]
pub struct HostImageCopyCapabilities {
    /// Whether host image copy is supported.
    pub supported: bool,
    /// Supported source layouts for copy to image.
    pub copy_src_layouts: Vec<vk::ImageLayout>,
    /// Supported destination layouts for copy from image.
    pub copy_dst_layouts: Vec<vk::ImageLayout>,
    /// Identical memory layout hint available.
    pub identical_memory_layout: bool,
}

/// Query host image copy capabilities.
pub fn query_capabilities(ctx: &super::context::VulkanContext) -> HostImageCopyCapabilities {
    let mut hic_features = vk::PhysicalDeviceHostImageCopyFeaturesEXT::default();
    let mut features2 = vk::PhysicalDeviceFeatures2::default()
        .push_next(&mut hic_features);

    unsafe {
        ctx.instance.get_physical_device_features2(ctx.physical_device, &mut features2);
    }

    let _ = features2;
    let supported = hic_features.host_image_copy == vk::TRUE;

    if !supported {
        return HostImageCopyCapabilities::default();
    }

    // Query properties for supported layouts
    let mut hic_props = vk::PhysicalDeviceHostImageCopyPropertiesEXT::default();
    let mut props2 = vk::PhysicalDeviceProperties2::default()
        .push_next(&mut hic_props);

    unsafe {
        ctx.instance.get_physical_device_properties2(ctx.physical_device, &mut props2);
    }

    // Common layouts that are typically supported
    let copy_src_layouts = vec![
        vk::ImageLayout::GENERAL,
        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
    ];

    let copy_dst_layouts = vec![
        vk::ImageLayout::GENERAL,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
    ];

    HostImageCopyCapabilities {
        supported,
        copy_src_layouts,
        copy_dst_layouts,
        identical_memory_layout: hic_props.identical_memory_type_requirements == vk::TRUE,
    }
}

/// Host image copy function pointers.
pub struct HostImageCopyFunctions {
    fp_copy_memory_to_image: Option<vk::PFN_vkCopyMemoryToImageEXT>,
    fp_copy_image_to_memory: Option<vk::PFN_vkCopyImageToMemoryEXT>,
    fp_transition_image_layout: Option<vk::PFN_vkTransitionImageLayoutEXT>,
    fp_get_image_subresource_layout: Option<vk::PFN_vkGetImageSubresourceLayout2KHR>,
}

impl HostImageCopyFunctions {
    /// Load function pointers.
    pub fn new(ctx: &super::context::VulkanContext) -> Self {
        macro_rules! get_fp {
            ($name:literal) => {
                unsafe {
                    let name = std::ffi::CStr::from_bytes_with_nul_unchecked(
                        concat!($name, "\0").as_bytes()
                    );
                    ctx.instance.get_device_proc_addr(ctx.device.handle(), name.as_ptr())
                        .map(|fp| std::mem::transmute(fp))
                }
            };
        }

        Self {
            fp_copy_memory_to_image: get_fp!("vkCopyMemoryToImageEXT"),
            fp_copy_image_to_memory: get_fp!("vkCopyImageToMemoryEXT"),
            fp_transition_image_layout: get_fp!("vkTransitionImageLayoutEXT"),
            fp_get_image_subresource_layout: get_fp!("vkGetImageSubresourceLayout2KHR"),
        }
    }

    /// Check if host image copy is available.
    pub fn is_available(&self) -> bool {
        self.fp_copy_memory_to_image.is_some()
    }
}

/// Region for host-to-image copy.
#[derive(Debug, Clone)]
pub struct MemoryToImageCopyRegion {
    /// Source memory pointer.
    pub host_pointer: *const std::ffi::c_void,
    /// Row length in pixels (0 for tightly packed).
    pub row_length: u32,
    /// Image height in pixels (0 for tightly packed).
    pub image_height: u32,
    /// Image subresource.
    pub subresource: vk::ImageSubresourceLayers,
    /// Offset in image.
    pub offset: vk::Offset3D,
    /// Extent to copy.
    pub extent: vk::Extent3D,
}

/// Copy memory directly to an image.
pub fn copy_memory_to_image(
    ctx: &super::context::VulkanContext,
    funcs: &HostImageCopyFunctions,
    image: vk::Image,
    layout: vk::ImageLayout,
    regions: &[MemoryToImageCopyRegion],
) -> Result<(), String> {
    let fp = funcs.fp_copy_memory_to_image
        .ok_or("Host image copy not supported")?;

    let vk_regions: Vec<vk::MemoryToImageCopyEXT> = regions.iter().map(|r| {
        vk::MemoryToImageCopyEXT::default()
            .host_pointer(r.host_pointer)
            .memory_row_length(r.row_length)
            .memory_image_height(r.image_height)
            .image_subresource(r.subresource)
            .image_offset(r.offset)
            .image_extent(r.extent)
    }).collect();

    let copy_info = vk::CopyMemoryToImageInfoEXT::default()
        .dst_image(image)
        .dst_image_layout(layout)
        .regions(&vk_regions);

    let result = unsafe { fp(ctx.device.handle(), &copy_info) };

    if result != vk::Result::SUCCESS {
        Err(format!("Failed to copy memory to image: {:?}", result))
    } else {
        Ok(())
    }
}

/// Region for image-to-host copy.
#[derive(Debug, Clone)]
pub struct ImageToMemoryCopyRegion {
    /// Destination memory pointer.
    pub host_pointer: *mut std::ffi::c_void,
    /// Row length in pixels (0 for tightly packed).
    pub row_length: u32,
    /// Image height in pixels (0 for tightly packed).
    pub image_height: u32,
    /// Image subresource.
    pub subresource: vk::ImageSubresourceLayers,
    /// Offset in image.
    pub offset: vk::Offset3D,
    /// Extent to copy.
    pub extent: vk::Extent3D,
}

/// Copy image directly to memory.
pub fn copy_image_to_memory(
    ctx: &super::context::VulkanContext,
    funcs: &HostImageCopyFunctions,
    image: vk::Image,
    layout: vk::ImageLayout,
    regions: &[ImageToMemoryCopyRegion],
) -> Result<(), String> {
    let fp = funcs.fp_copy_image_to_memory
        .ok_or("Host image copy not supported")?;

    let vk_regions: Vec<vk::ImageToMemoryCopyEXT> = regions.iter().map(|r| {
        vk::ImageToMemoryCopyEXT::default()
            .host_pointer(r.host_pointer)
            .memory_row_length(r.row_length)
            .memory_image_height(r.image_height)
            .image_subresource(r.subresource)
            .image_offset(r.offset)
            .image_extent(r.extent)
    }).collect();

    let copy_info = vk::CopyImageToMemoryInfoEXT::default()
        .src_image(image)
        .src_image_layout(layout)
        .regions(&vk_regions);

    let result = unsafe { fp(ctx.device.handle(), &copy_info) };

    if result != vk::Result::SUCCESS {
        Err(format!("Failed to copy image to memory: {:?}", result))
    } else {
        Ok(())
    }
}

/// Transition image layout from host.
pub fn transition_image_layout(
    ctx: &super::context::VulkanContext,
    funcs: &HostImageCopyFunctions,
    image: vk::Image,
    old_layout: vk::ImageLayout,
    new_layout: vk::ImageLayout,
    subresource_range: vk::ImageSubresourceRange,
) -> Result<(), String> {
    let fp = funcs.fp_transition_image_layout
        .ok_or("Host image copy not supported")?;

    let transition = vk::HostImageLayoutTransitionInfoEXT::default()
        .image(image)
        .old_layout(old_layout)
        .new_layout(new_layout)
        .subresource_range(subresource_range);

    let transitions = [transition];

    let result = unsafe { fp(ctx.device.handle(), transitions.len() as u32, transitions.as_ptr()) };

    if result != vk::Result::SUCCESS {
        Err(format!("Failed to transition image layout: {:?}", result))
    } else {
        Ok(())
    }
}

/// Helper to upload a 2D texture using host image copy.
pub fn upload_texture_2d(
    ctx: &super::context::VulkanContext,
    funcs: &HostImageCopyFunctions,
    image: vk::Image,
    data: &[u8],
    width: u32,
    height: u32,
    bytes_per_pixel: u32,
) -> Result<(), String> {
    let region = MemoryToImageCopyRegion {
        host_pointer: data.as_ptr() as *const _,
        row_length: 0,
        image_height: 0,
        subresource: vk::ImageSubresourceLayers {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            mip_level: 0,
            base_array_layer: 0,
            layer_count: 1,
        },
        offset: vk::Offset3D { x: 0, y: 0, z: 0 },
        extent: vk::Extent3D { width, height, depth: 1 },
    };

    copy_memory_to_image(ctx, funcs, image, vk::ImageLayout::GENERAL, &[region])
}

/// Helper to download a 2D texture using host image copy.
pub fn download_texture_2d(
    ctx: &super::context::VulkanContext,
    funcs: &HostImageCopyFunctions,
    image: vk::Image,
    data: &mut [u8],
    width: u32,
    height: u32,
) -> Result<(), String> {
    let region = ImageToMemoryCopyRegion {
        host_pointer: data.as_mut_ptr() as *mut _,
        row_length: 0,
        image_height: 0,
        subresource: vk::ImageSubresourceLayers {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            mip_level: 0,
            base_array_layer: 0,
            layer_count: 1,
        },
        offset: vk::Offset3D { x: 0, y: 0, z: 0 },
        extent: vk::Extent3D { width, height, depth: 1 },
    };

    copy_image_to_memory(ctx, funcs, image, vk::ImageLayout::GENERAL, &[region])
}

/// Check if a format supports host image copy.
pub fn format_supports_host_copy(
    ctx: &super::context::VulkanContext,
    format: vk::Format,
    tiling: vk::ImageTiling,
    usage: vk::ImageUsageFlags,
) -> bool {
    let mut format_props = vk::FormatProperties3::default();
    let mut format_props2 = vk::FormatProperties2::default()
        .push_next(&mut format_props);

    unsafe {
        ctx.instance.get_physical_device_format_properties2(
            ctx.physical_device,
            format,
            &mut format_props2,
        );
    }

    let features = if tiling == vk::ImageTiling::OPTIMAL {
        format_props.optimal_tiling_features
    } else {
        format_props.linear_tiling_features
    };

    features.contains(vk::FormatFeatureFlags2::HOST_IMAGE_TRANSFER_EXT)
}
