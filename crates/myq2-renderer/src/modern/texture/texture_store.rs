//! Vulkan texture store — maps texnum (GL-style integer IDs) to real Vulkan textures.
//!
//! Intercepts pixel data from `vk_upload32()` and creates VkImage/VkImageView/descriptor sets.
//! Used by `flush_2d_internal()` to bind per-batch textures during 2D rendering.

use ash::vk;
use std::collections::HashMap;
use std::sync::Mutex;
use crate::modern::gpu_device;

// ============================================================================
// Data structures
// ============================================================================

/// A Vulkan texture with all GPU resources needed for sampling.
struct VulkanTexture {
    image: vk::Image,
    view: vk::ImageView,
    memory: vk::DeviceMemory,
    descriptor_set: Option<vk::DescriptorSet>, // None = pending, needs lazy creation
    width: u32,
    height: u32,
    mip_levels: u32,
    cached_pixels: Option<Vec<u8>>, // Cached RGBA data for lazy descriptor creation
}

/// Manages the mapping from texnum → Vulkan texture resources.
struct TextureStore {
    textures: HashMap<i32, VulkanTexture>,
    sampler: vk::Sampler,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set_layout: vk::DescriptorSetLayout,
}

// ============================================================================
// Global static
// ============================================================================

static TEXTURE_STORE: Mutex<Option<TextureStore>> = Mutex::new(None);

// ============================================================================
// Public API
// ============================================================================

/// Initialize the texture store. Must be called after GPU device and pipeline layout are ready.
pub fn init_texture_store(descriptor_set_layout: vk::DescriptorSetLayout) {
    let store = gpu_device::with_device(|ctx| {
        // SAFETY: Vulkan context is valid, called from main thread.
        unsafe { TextureStore::new(ctx, descriptor_set_layout) }
    }).flatten();

    if let Some(store) = store {
        *TEXTURE_STORE.lock().unwrap_or_else(|e| e.into_inner()) = Some(store);
    }
}

/// Shutdown and release all texture store resources.
pub fn shutdown_texture_store() {
    let store = TEXTURE_STORE.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(store) = store {
        gpu_device::with_device(|ctx| {
            // SAFETY: Vulkan context is valid, called from main thread.
            unsafe { store.destroy(ctx); }
        });
    }
}

/// Create or update a texture from RGBA pixel data.
/// Called from `vk_upload32()` to intercept texture uploads.
/// Uses lazy descriptor creation to avoid lock contention during init.
pub fn create_or_update_texture(texnum: i32, rgba_data: &[u8], width: u32, height: u32) {
    let mut guard = TEXTURE_STORE.lock().unwrap_or_else(|e| e.into_inner());
    let store = match guard.as_mut() {
        Some(s) => s,
        None => return, // TextureStore not initialized yet
    };

    // If texture already exists, destroy it and recreate with new pixel data.
    // This handles both same-dimensions re-uploads (e.g. scrap atlas) and
    // dimension changes. Recreating avoids issues with uploading to an existing
    // VkImage during an active render frame (queue_wait_idle conflicts).
    if store.textures.contains_key(&texnum) {
        let old = store.textures.remove(&texnum).unwrap();
        let pool = store.descriptor_pool;
        gpu_device::with_device(|ctx| {
            // SAFETY: Vulkan context valid, main thread.
            unsafe { destroy_vulkan_texture(ctx, &old, pool); }
        });
    }

    // Create new Vulkan texture with deferred descriptor creation
    let sampler = store.sampler;
    let pool = store.descriptor_pool;
    let layout = store.descriptor_set_layout;

    // Use with_device_and_commands_mut to get both ctx and commands with a SINGLE lock
    let vk_tex = gpu_device::with_device_and_commands_mut(|ctx, commands| {
        // SAFETY: Vulkan context valid, main thread.
        unsafe {
            let vk_tex = create_vulkan_texture_deferred(ctx, rgba_data, width, height, sampler, pool, layout)?;

            // Upload pixels immediately (we have both ctx and commands)
            upload_to_image(ctx, commands, vk_tex.image, rgba_data, width, height, vk_tex.mip_levels);

            Some(vk_tex)
        }
    }).flatten();

    if let Some(vk_tex) = vk_tex {
        store.textures.insert(texnum, vk_tex);
    }
}

/// Get the descriptor set for a texnum, for binding during rendering.
/// Returns None if texture doesn't exist or descriptor hasn't been created yet.
pub fn get_descriptor_set(texnum: i32) -> Option<vk::DescriptorSet> {
    TEXTURE_STORE.lock().unwrap_or_else(|e| e.into_inner()).as_ref()
        .and_then(|store| store.textures.get(&texnum).and_then(|t| t.descriptor_set))
}

/// Ensure a descriptor set exists for the given texnum, creating it lazily if needed.
/// This is the function draw code should use instead of get_descriptor_set.
pub fn ensure_descriptor_set(texnum: i32) -> Option<vk::DescriptorSet> {
    // Fast path: descriptor already exists
    if let Some(ds) = get_descriptor_set(texnum) {
        return Some(ds);
    }

    // Slow path: need to create descriptor from cached pixels
    let mut guard = TEXTURE_STORE.lock().unwrap_or_else(|e| e.into_inner());
    let store = guard.as_mut()?;

    // Get texture and check if it has cached pixels
    let texture = store.textures.get_mut(&texnum)?;

    // Already has descriptor? (race condition check)
    if texture.descriptor_set.is_some() {
        return texture.descriptor_set;
    }

    // Create descriptor set using texture's existing image/view
    let sampler = store.sampler;
    let pool = store.descriptor_pool;
    let layout = store.descriptor_set_layout;
    let view = texture.view;

    let descriptor_set = gpu_device::with_device(|ctx| {
        // SAFETY: Vulkan context valid, called from draw thread.
        unsafe {
            // Pixel data was already uploaded during create_vulkan_texture_deferred
            // Just need to create the descriptor set now

            // Allocate descriptor set
            let layouts = [layout];
            let alloc_info = vk::DescriptorSetAllocateInfo::default()
                .descriptor_pool(pool)
                .set_layouts(&layouts);

            let ds = match ctx.device.allocate_descriptor_sets(&alloc_info) {
                Ok(sets) => sets[0],
                Err(e) => {
                    eprintln!("ensure_descriptor_set: allocate_descriptor_sets FAILED: {:?}", e);
                    return None;
                }
            };

            // Write descriptor
            let image_info_desc = vk::DescriptorImageInfo::default()
                .sampler(sampler)
                .image_view(view)
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);

            let write = vk::WriteDescriptorSet::default()
                .dst_set(ds)
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(std::slice::from_ref(&image_info_desc));

            ctx.device.update_descriptor_sets(&[write], &[]);

            Some(ds)
        }
    }).flatten();

    // Store the descriptor set in the texture
    texture.descriptor_set = descriptor_set;

    descriptor_set
}

/// Create the 1x1 white texture at texnum=0 for untextured draws.
pub fn create_white_texture() {
    let white_pixel: [u8; 4] = [255, 255, 255, 255];
    create_or_update_texture(0, &white_pixel, 1, 1);
}

/// Post-initialization pass: create descriptor sets for loaded textures.
/// Called after R_Init completes, outside of any initialization locks.
pub fn create_post_init_descriptors() {
    create_white_texture();

    // Additional textures can be created here if needed
    // For now, only texture 0 (white) is created
    // Texture 1160 (conchars) will need to be loaded/reloaded with proper descriptor sets
}

/// Create a descriptor set for an existing image view (e.g., scene FBO color attachment).
///
/// Unlike `create_or_update_texture`, this does not create a VkImage or allocate memory.
/// It only allocates a descriptor set and writes the provided view + sampler into it.
/// The descriptor is stored under the given `id` for later retrieval via `get_descriptor_set`.
pub fn create_descriptor_for_view(
    id: i32,
    image_view: vk::ImageView,
    sampler: vk::Sampler,
) {
    let mut guard = TEXTURE_STORE.lock().unwrap_or_else(|e| e.into_inner());
    let store = match guard.as_mut() {
        Some(s) => s,
        None => return,
    };

    // Remove existing entry if present (don't destroy image/memory — we don't own them)
    store.textures.remove(&id);

    let pool = store.descriptor_pool;
    let layout = store.descriptor_set_layout;

    let descriptor_set = gpu_device::with_device(|ctx| {
        // SAFETY: Vulkan context valid, main thread.
        unsafe {
            let layouts = [layout];
            let alloc_info = vk::DescriptorSetAllocateInfo::default()
                .descriptor_pool(pool)
                .set_layouts(&layouts);

            let ds = match ctx.device.allocate_descriptor_sets(&alloc_info) {
                Ok(sets) => sets[0],
                Err(_) => return None,
            };

            let image_info_desc = vk::DescriptorImageInfo::default()
                .sampler(sampler)
                .image_view(image_view)
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);

            let write = vk::WriteDescriptorSet::default()
                .dst_set(ds)
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(std::slice::from_ref(&image_info_desc));

            ctx.device.update_descriptor_sets(&[write], &[]);

            Some(ds)
        }
    }).flatten();

    if let Some(ds) = descriptor_set {
        // Store as a VulkanTexture with null image/memory (we don't own them)
        store.textures.insert(id, VulkanTexture {
            image: vk::Image::null(),
            view: vk::ImageView::null(),
            memory: vk::DeviceMemory::null(),
            descriptor_set: Some(ds),
            width: 0,
            height: 0,
            mip_levels: 1,
            cached_pixels: None, // No caching for view-only descriptors
        });
    }
}

// ============================================================================
// TextureStore implementation
// ============================================================================

impl TextureStore {
    /// Create a new TextureStore with sampler and descriptor pool.
    unsafe fn new(
        ctx: &crate::vulkan::context::VulkanContext,
        descriptor_set_layout: vk::DescriptorSetLayout,
    ) -> Option<Self> {
        // Create shared sampler — LINEAR filtering with trilinear mipmapping
        // BSP world textures tile/repeat across surfaces, so REPEAT is required
        let sampler_info = vk::SamplerCreateInfo::default()
            .mag_filter(vk::Filter::LINEAR)
            .min_filter(vk::Filter::LINEAR)
            .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
            .address_mode_u(vk::SamplerAddressMode::REPEAT)
            .address_mode_v(vk::SamplerAddressMode::REPEAT)
            .address_mode_w(vk::SamplerAddressMode::REPEAT)
            .mip_lod_bias(0.0)
            .anisotropy_enable(false)
            .max_anisotropy(1.0)
            .compare_enable(false)
            .min_lod(0.0)
            .max_lod(vk::LOD_CLAMP_NONE)
            .border_color(vk::BorderColor::FLOAT_OPAQUE_BLACK)
            .unnormalized_coordinates(false);

        let sampler = ctx.device.create_sampler(&sampler_info, None).ok()?;

        // Create descriptor pool — enough for all engine textures
        let pool_sizes = [vk::DescriptorPoolSize {
            ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: 1200,
        }];

        let pool_info = vk::DescriptorPoolCreateInfo::default()
            .pool_sizes(&pool_sizes)
            .max_sets(1200)
            .flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET);

        let descriptor_pool = match ctx.device.create_descriptor_pool(&pool_info, None) {
            Ok(pool) => pool,
            Err(_) => {
                ctx.device.destroy_sampler(sampler, None);
                return None;
            }
        };

        Some(Self {
            textures: HashMap::with_capacity(512),
            sampler,
            descriptor_pool,
            descriptor_set_layout,
        })
    }

    /// Destroy all resources.
    unsafe fn destroy(self, ctx: &crate::vulkan::context::VulkanContext) {
        let pool = self.descriptor_pool;
        for tex in self.textures.values() {
            destroy_vulkan_texture(ctx, tex, pool);
        }
        // Pool itself is also destroyed (frees any remaining descriptor sets)
        ctx.device.destroy_descriptor_pool(self.descriptor_pool, None);
        ctx.device.destroy_sampler(self.sampler, None);
    }
}

// ============================================================================
// Vulkan texture creation helpers
// ============================================================================

/// Create a Vulkan texture with deferred descriptor creation (Option B - Lazy Creation).
/// Creates image, memory, and view, but defers descriptor set allocation until first draw.
/// Caches pixel data for later descriptor creation.
unsafe fn create_vulkan_texture_deferred(
    ctx: &crate::vulkan::context::VulkanContext,
    rgba_data: &[u8],
    width: u32,
    height: u32,
    _sampler: vk::Sampler,
    _descriptor_pool: vk::DescriptorPool,
    _descriptor_set_layout: vk::DescriptorSetLayout,
) -> Option<VulkanTexture> {
    // 1. Create image with mipmap chain
    let mip_levels = ((width.max(height) as f32).log2().floor() as u32) + 1;

    let image_info = vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .format(vk::Format::R8G8B8A8_UNORM)
        .extent(vk::Extent3D { width, height, depth: 1 })
        .mip_levels(mip_levels)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(vk::ImageTiling::OPTIMAL)
        .usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::TRANSFER_SRC | vk::ImageUsageFlags::SAMPLED)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .initial_layout(vk::ImageLayout::UNDEFINED);

    let image = ctx.device.create_image(&image_info, None).ok()?;

    // 2. Allocate device-local memory
    let mem_reqs = ctx.device.get_image_memory_requirements(image);
    let mem_props = ctx.instance.get_physical_device_memory_properties(ctx.physical_device);

    let mem_type = (0..mem_props.memory_type_count).find(|&i| {
        (mem_reqs.memory_type_bits & (1 << i)) != 0
            && mem_props.memory_types[i as usize]
                .property_flags
                .contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)
    });

    let mem_type = match mem_type {
        Some(i) => i,
        None => {
            ctx.device.destroy_image(image, None);
            return None;
        }
    };

    let alloc_info = vk::MemoryAllocateInfo::default()
        .allocation_size(mem_reqs.size)
        .memory_type_index(mem_type);

    let memory = match ctx.device.allocate_memory(&alloc_info, None) {
        Ok(mem) => mem,
        Err(_) => {
            ctx.device.destroy_image(image, None);
            return None;
        }
    };

    if ctx.device.bind_image_memory(image, memory, 0).is_err() {
        ctx.device.free_memory(memory, None);
        ctx.device.destroy_image(image, None);
        return None;
    }

    // 3. Create image view (all mip levels)
    let view_info = vk::ImageViewCreateInfo::default()
        .image(image)
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(vk::Format::R8G8B8A8_UNORM)
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: mip_levels,
            base_array_layer: 0,
            layer_count: 1,
        });

    let view = match ctx.device.create_image_view(&view_info, None) {
        Ok(v) => v,
        Err(_) => {
            ctx.device.free_memory(memory, None);
            ctx.device.destroy_image(image, None);
            return None;
        }
    };

    // 4. Cache pixel data - will be uploaded after this function returns
    let cached_pixels = rgba_data.to_vec();

    // 5. Return VulkanTexture with None descriptor_set (will be created on first draw)
    Some(VulkanTexture {
        image,
        view,
        memory,
        descriptor_set: None, // Lazy creation!
        width,
        height,
        mip_levels,
        cached_pixels: Some(cached_pixels),
    })
}

/// Create a complete Vulkan texture: image, memory, view, upload, descriptor set.
/// This is the old immediate creation path, kept for reference but not used anymore.
#[allow(dead_code)]
unsafe fn create_vulkan_texture(
    ctx: &crate::vulkan::context::VulkanContext,
    rgba_data: &[u8],
    width: u32,
    height: u32,
    sampler: vk::Sampler,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set_layout: vk::DescriptorSetLayout,
) -> Option<VulkanTexture> {
    // 1. Create image with mipmap chain
    let mip_levels = ((width.max(height) as f32).log2().floor() as u32) + 1;

    let image_info = vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .format(vk::Format::R8G8B8A8_UNORM)
        .extent(vk::Extent3D { width, height, depth: 1 })
        .mip_levels(mip_levels)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(vk::ImageTiling::OPTIMAL)
        .usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::TRANSFER_SRC | vk::ImageUsageFlags::SAMPLED)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .initial_layout(vk::ImageLayout::UNDEFINED);

    let image = ctx.device.create_image(&image_info, None).ok()?;

    // 2. Allocate device-local memory
    let mem_reqs = ctx.device.get_image_memory_requirements(image);
    let mem_props = ctx.instance.get_physical_device_memory_properties(ctx.physical_device);

    let mem_type = (0..mem_props.memory_type_count).find(|&i| {
        (mem_reqs.memory_type_bits & (1 << i)) != 0
            && mem_props.memory_types[i as usize]
                .property_flags
                .contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)
    });

    let mem_type = match mem_type {
        Some(i) => i,
        None => {
            ctx.device.destroy_image(image, None);
            return None;
        }
    };

    let alloc_info = vk::MemoryAllocateInfo::default()
        .allocation_size(mem_reqs.size)
        .memory_type_index(mem_type);

    let memory = match ctx.device.allocate_memory(&alloc_info, None) {
        Ok(mem) => mem,
        Err(_) => {
            ctx.device.destroy_image(image, None);
            return None;
        }
    };

    if ctx.device.bind_image_memory(image, memory, 0).is_err() {
        ctx.device.free_memory(memory, None);
        ctx.device.destroy_image(image, None);
        return None;
    }

    // 3. Create image view (all mip levels)
    let view_info = vk::ImageViewCreateInfo::default()
        .image(image)
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(vk::Format::R8G8B8A8_UNORM)
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: mip_levels,
            base_array_layer: 0,
            layer_count: 1,
        });

    let view = match ctx.device.create_image_view(&view_info, None) {
        Ok(v) => v,
        Err(_) => {
            ctx.device.free_memory(memory, None);
            ctx.device.destroy_image(image, None);
            return None;
        }
    };

    // 4. Upload pixel data via staging buffer and generate mipmaps
    // NOTE: This function is dead code and doesn't work anymore because upload_to_image
    // now requires CommandManager which can't be accessed here without nested locks.
    // If you need to use this function, refactor to use with_device_and_commands_mut.
    // upload_to_image(ctx, commands, image, rgba_data, width, height, mip_levels);

    // 5. Allocate descriptor set and write texture binding
    let layouts = [descriptor_set_layout];
    let alloc_info = vk::DescriptorSetAllocateInfo::default()
        .descriptor_pool(descriptor_pool)
        .set_layouts(&layouts);

    let descriptor_set = match ctx.device.allocate_descriptor_sets(&alloc_info) {
        Ok(sets) => sets[0],
        Err(_) => {
            ctx.device.destroy_image_view(view, None);
            ctx.device.free_memory(memory, None);
            ctx.device.destroy_image(image, None);
            return None;
        }
    };

    let image_info_desc = vk::DescriptorImageInfo::default()
        .sampler(sampler)
        .image_view(view)
        .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);

    let write = vk::WriteDescriptorSet::default()
        .dst_set(descriptor_set)
        .dst_binding(0)
        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .image_info(std::slice::from_ref(&image_info_desc));

    ctx.device.update_descriptor_sets(&[write], &[]);

    Some(VulkanTexture {
        image,
        view,
        memory,
        descriptor_set: Some(descriptor_set),
        width,
        height,
        mip_levels,
        cached_pixels: None, // No caching for immediate creation
    })
}

/// Upload RGBA pixel data to an existing VkImage via staging buffer, then generate mipmaps.
unsafe fn upload_to_image(
    ctx: &crate::vulkan::context::VulkanContext,
    commands: &mut crate::modern::gpu_device::CommandManager,
    image: vk::Image,
    rgba_data: &[u8],
    width: u32,
    height: u32,
    mip_levels: u32,
) {
    let data_size = (width * height * 4) as usize;
    if rgba_data.len() < data_size {
        return;
    }

    // Create staging buffer
    let buffer_info = vk::BufferCreateInfo::default()
        .size(data_size as vk::DeviceSize)
        .usage(vk::BufferUsageFlags::TRANSFER_SRC)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);

    let staging_buffer = match ctx.device.create_buffer(&buffer_info, None) {
        Ok(buf) => buf,
        Err(_) => return,
    };

    let mem_requirements = ctx.device.get_buffer_memory_requirements(staging_buffer);
    let memory_properties = ctx
        .instance
        .get_physical_device_memory_properties(ctx.physical_device);

    let memory_type_index = (0..memory_properties.memory_type_count).find(|&i| {
        (mem_requirements.memory_type_bits & (1 << i)) != 0
            && memory_properties.memory_types[i as usize]
                .property_flags
                .contains(
                    vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                )
    });

    let memory_type_index = match memory_type_index {
        Some(i) => i,
        None => {
            ctx.device.destroy_buffer(staging_buffer, None);
            return;
        }
    };

    let alloc_info = vk::MemoryAllocateInfo::default()
        .allocation_size(mem_requirements.size)
        .memory_type_index(memory_type_index);

    let staging_memory = match ctx.device.allocate_memory(&alloc_info, None) {
        Ok(mem) => mem,
        Err(_) => {
            ctx.device.destroy_buffer(staging_buffer, None);
            return;
        }
    };

    if ctx
        .device
        .bind_buffer_memory(staging_buffer, staging_memory, 0)
        .is_err()
    {
        ctx.device.free_memory(staging_memory, None);
        ctx.device.destroy_buffer(staging_buffer, None);
        return;
    }

    // Map and copy data
    let mapped_ptr = match ctx.device.map_memory(
        staging_memory,
        0,
        data_size as vk::DeviceSize,
        vk::MemoryMapFlags::empty(),
    ) {
        Ok(ptr) => ptr as *mut u8,
        Err(_) => {
            ctx.device.free_memory(staging_memory, None);
            ctx.device.destroy_buffer(staging_buffer, None);
            return;
        }
    };

    std::ptr::copy_nonoverlapping(rgba_data.as_ptr(), mapped_ptr, data_size);
    ctx.device.unmap_memory(staging_memory);

    // Record copy commands
    let copy_region = vk::BufferImageCopy::default()
        .buffer_offset(0)
        .buffer_row_length(0)
        .buffer_image_height(0)
        .image_subresource(vk::ImageSubresourceLayers {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            mip_level: 0,
            base_array_layer: 0,
            layer_count: 1,
        })
        .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
        .image_extent(vk::Extent3D {
            width,
            height,
            depth: 1,
        });

    // Use commands parameter directly (no nested lock)
    let cmd = match commands.begin_single_time() {
        Ok(c) => c,
        Err(_) => {
            ctx.device.free_memory(staging_memory, None);
            ctx.device.destroy_buffer(staging_buffer, None);
            return;
        }
    };

        // Transition all mip levels: UNDEFINED → TRANSFER_DST_OPTIMAL
        let barrier = vk::ImageMemoryBarrier::default()
            .old_layout(vk::ImageLayout::UNDEFINED)
            .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(image)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: mip_levels,
                base_array_layer: 0,
                layer_count: 1,
            })
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE);

        ctx.device.cmd_pipeline_barrier(
            cmd,
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[barrier],
        );

        // Copy buffer → image (mip level 0)
        ctx.device.cmd_copy_buffer_to_image(
            cmd,
            staging_buffer,
            image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &[copy_region],
        );

        // Generate mip chain via cmd_blit_image
        let mut mip_width = width as i32;
        let mut mip_height = height as i32;

        for level in 1..mip_levels {
            // Transition level-1: TRANSFER_DST → TRANSFER_SRC
            let barrier = vk::ImageMemoryBarrier::default()
                .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(image)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: level - 1,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::TRANSFER_READ);

            ctx.device.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[], &[], &[barrier],
            );

            let next_width = (mip_width / 2).max(1);
            let next_height = (mip_height / 2).max(1);

            let blit = vk::ImageBlit::default()
                .src_subresource(vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: level - 1,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .src_offsets([
                    vk::Offset3D { x: 0, y: 0, z: 0 },
                    vk::Offset3D { x: mip_width, y: mip_height, z: 1 },
                ])
                .dst_subresource(vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: level,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .dst_offsets([
                    vk::Offset3D { x: 0, y: 0, z: 0 },
                    vk::Offset3D { x: next_width, y: next_height, z: 1 },
                ]);

            ctx.device.cmd_blit_image(
                cmd,
                image, vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                image, vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[blit],
                vk::Filter::LINEAR,
            );

            // Transition level-1: TRANSFER_SRC → SHADER_READ_ONLY
            let barrier = vk::ImageMemoryBarrier::default()
                .old_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(image)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: level - 1,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .src_access_mask(vk::AccessFlags::TRANSFER_READ)
                .dst_access_mask(vk::AccessFlags::SHADER_READ);

            ctx.device.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(),
                &[], &[], &[barrier],
            );

            mip_width = next_width;
            mip_height = next_height;
        }

        // Transition last mip level: TRANSFER_DST → SHADER_READ_ONLY
        let barrier = vk::ImageMemoryBarrier::default()
            .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(image)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: mip_levels - 1,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            })
            .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .dst_access_mask(vk::AccessFlags::SHADER_READ);

        ctx.device.cmd_pipeline_barrier(
            cmd,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::FRAGMENT_SHADER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[barrier],
        );

    let _ = commands.end_single_time(ctx, cmd);

    // Clean up staging resources
    ctx.device.free_memory(staging_memory, None);
    ctx.device.destroy_buffer(staging_buffer, None);
}

/// Destroy a single VulkanTexture's GPU resources including its descriptor set.
/// Handles null handles gracefully (for view-only entries from `create_descriptor_for_view`).
unsafe fn destroy_vulkan_texture(
    ctx: &crate::vulkan::context::VulkanContext,
    tex: &VulkanTexture,
    descriptor_pool: vk::DescriptorPool,
) {
    // Free descriptor set back to pool (pool created with FREE_DESCRIPTOR_SET flag)
    if let Some(ds) = tex.descriptor_set {
        let _ = ctx.device.free_descriptor_sets(descriptor_pool, &[ds]);
    }
    if tex.view != vk::ImageView::null() {
        ctx.device.destroy_image_view(tex.view, None);
    }
    if tex.memory != vk::DeviceMemory::null() {
        ctx.device.free_memory(tex.memory, None);
    }
    if tex.image != vk::Image::null() {
        ctx.device.destroy_image(tex.image, None);
    }
}
