//! Lightmap texture array
//!
//! Stores all lightmaps in a single Vulkan 2D texture array.
//! Data is uploaded via staging buffers and command buffers.
//! Supports parallel CPU preparation and batched GPU uploads.

use ash::vk;
use rayon::prelude::*;
use crate::modern::gpu_device;

/// Maximum number of lightmap layers.
pub const MAX_LIGHTMAPS: u32 = 128;

/// Lightmap block dimensions.
pub const BLOCK_WIDTH: u32 = 128;
pub const BLOCK_HEIGHT: u32 = 128;

/// Lightmap format (RGBA = 4 bytes per texel).
pub const LIGHTMAP_BYTES: u32 = 4;

/// Internal struct for prepared lightmap region upload data.
struct LightmapRegionUpload {
    layer: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    data: Vec<u8>,
    byte_size: usize,
}

/// Manages a texture array containing all lightmaps.
pub struct LightmapArray {
    /// GPU texture (2D array).
    texture: Option<vk::Image>,
    /// Image view for the texture array.
    image_view: Option<vk::ImageView>,
    /// GPU sampler for lightmap filtering.
    sampler: Option<vk::Sampler>,
    /// Device memory for the texture.
    texture_memory: Option<vk::DeviceMemory>,
    /// Number of layers currently in use.
    layer_count: u32,
    /// Allocation tracker for each row in each layer.
    allocated: Vec<[i32; BLOCK_WIDTH as usize]>,
    /// Whether the texture is initialized (initial layout transition done).
    initialized: bool,
}

impl LightmapArray {
    /// Create a new lightmap array with GPU texture and sampler.
    pub fn new() -> Self {
        let mut array = Self {
            texture: None,
            image_view: None,
            sampler: None,
            texture_memory: None,
            layer_count: 0,
            allocated: vec![[0; BLOCK_WIDTH as usize]; MAX_LIGHTMAPS as usize],
            initialized: false,
        };
        array.create_gpu_resources();
        array
    }

    /// Create the GPU texture array and sampler.
    fn create_gpu_resources(&mut self) {
        gpu_device::with_device(|ctx| {
            // SAFETY: Vulkan context is valid and we're on the main thread.
            unsafe {
                // Create 2D array image
                let image_info = vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(vk::Format::R8G8B8A8_UNORM)
                    .extent(vk::Extent3D {
                        width: BLOCK_WIDTH,
                        height: BLOCK_HEIGHT,
                        depth: 1,
                    })
                    .mip_levels(1)
                    .array_layers(MAX_LIGHTMAPS)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .initial_layout(vk::ImageLayout::UNDEFINED);

                let texture = match ctx.device.create_image(&image_info, None) {
                    Ok(img) => img,
                    Err(_) => return,
                };

                // Allocate memory
                let mem_reqs = ctx.device.get_image_memory_requirements(texture);
                let mem_props = ctx.instance.get_physical_device_memory_properties(ctx.physical_device);

                let mem_type = (0..mem_props.memory_type_count)
                    .find(|&i| {
                        (mem_reqs.memory_type_bits & (1 << i)) != 0 &&
                        mem_props.memory_types[i as usize].property_flags.contains(
                            vk::MemoryPropertyFlags::DEVICE_LOCAL
                        )
                    });

                let mem_type = match mem_type {
                    Some(i) => i,
                    None => {
                        ctx.device.destroy_image(texture, None);
                        return;
                    }
                };

                let alloc_info = vk::MemoryAllocateInfo::default()
                    .allocation_size(mem_reqs.size)
                    .memory_type_index(mem_type);

                let memory = match ctx.device.allocate_memory(&alloc_info, None) {
                    Ok(mem) => mem,
                    Err(_) => {
                        ctx.device.destroy_image(texture, None);
                        return;
                    }
                };

                if ctx.device.bind_image_memory(texture, memory, 0).is_err() {
                    ctx.device.free_memory(memory, None);
                    ctx.device.destroy_image(texture, None);
                    return;
                }

                // Create image view (2D array)
                let view_info = vk::ImageViewCreateInfo::default()
                    .image(texture)
                    .view_type(vk::ImageViewType::TYPE_2D_ARRAY)
                    .format(vk::Format::R8G8B8A8_UNORM)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: MAX_LIGHTMAPS,
                    });

                let image_view = match ctx.device.create_image_view(&view_info, None) {
                    Ok(view) => view,
                    Err(_) => {
                        ctx.device.free_memory(memory, None);
                        ctx.device.destroy_image(texture, None);
                        return;
                    }
                };

                // Create sampler with linear filtering
                let sampler_info = vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::LINEAR)
                    .min_filter(vk::Filter::LINEAR)
                    .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .mip_lod_bias(0.0)
                    .anisotropy_enable(false)
                    .max_anisotropy(1.0)
                    .compare_enable(false)
                    .min_lod(0.0)
                    .max_lod(0.0)
                    .border_color(vk::BorderColor::FLOAT_OPAQUE_BLACK)
                    .unnormalized_coordinates(false);

                let sampler = match ctx.device.create_sampler(&sampler_info, None) {
                    Ok(s) => s,
                    Err(_) => {
                        ctx.device.destroy_image_view(image_view, None);
                        ctx.device.free_memory(memory, None);
                        ctx.device.destroy_image(texture, None);
                        return;
                    }
                };

                self.texture = Some(texture);
                self.image_view = Some(image_view);
                self.texture_memory = Some(memory);
                self.sampler = Some(sampler);

                // Transition to SHADER_READ_ONLY_OPTIMAL for initial state
                gpu_device::with_commands_mut(|commands| {
                    let cmd = match commands.begin_single_time() {
                        Ok(c) => c,
                        Err(_) => return,
                    };

                    let barrier = vk::ImageMemoryBarrier::default()
                        .old_layout(vk::ImageLayout::UNDEFINED)
                        .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                        .image(texture)
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: MAX_LIGHTMAPS,
                        })
                        .src_access_mask(vk::AccessFlags::empty())
                        .dst_access_mask(vk::AccessFlags::SHADER_READ);

                    ctx.device.cmd_pipeline_barrier(
                        cmd,
                        vk::PipelineStageFlags::TOP_OF_PIPE,
                        vk::PipelineStageFlags::FRAGMENT_SHADER,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        &[barrier],
                    );

                    let _ = commands.end_single_time(ctx, cmd);
                });

                self.initialized = true;
            }
        });
    }

    /// Upload a complete lightmap layer.
    ///
    /// # Arguments
    /// * `layer` - Layer index (0 to MAX_LIGHTMAPS-1)
    /// * `data` - RGBA pixel data (BLOCK_WIDTH * BLOCK_HEIGHT * 4 bytes)
    pub fn upload_layer(&mut self, layer: u32, data: &[u8]) {
        assert!(layer < MAX_LIGHTMAPS);
        assert!(data.len() == (BLOCK_WIDTH * BLOCK_HEIGHT * LIGHTMAP_BYTES) as usize);

        if self.texture.is_none() {
            return;
        }

        // Delegate to batch upload with a single entry
        self.batch_upload_layers(&[(layer, data.to_vec())]);

        if layer >= self.layer_count {
            self.layer_count = layer + 1;
        }
    }

    /// Update a region within a layer.
    ///
    /// # Arguments
    /// * `layer` - Layer index
    /// * `x`, `y` - Top-left corner of region
    /// * `width`, `height` - Region dimensions
    /// * `data` - RGBA pixel data for the region
    pub fn update_region(
        &self,
        layer: u32,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        data: &[u8],
    ) {
        assert!(layer < MAX_LIGHTMAPS);
        assert!(x + width <= BLOCK_WIDTH);
        assert!(y + height <= BLOCK_HEIGHT);
        assert!(data.len() == (width * height * LIGHTMAP_BYTES) as usize);

        if self.texture.is_none() {
            return;
        }

        // Delegate to batch upload with a single entry
        self.batch_upload_regions(&[(layer, x, y, width, height, data.to_vec())]);
    }

    /// Allocate space for a new lightmap surface.
    ///
    /// Returns (layer, x, y) if successful, None if no space available.
    pub fn allocate(&mut self, width: u32, height: u32) -> Option<(u32, u32, u32)> {
        for layer in 0..MAX_LIGHTMAPS {
            if let Some((x, y)) = self.allocate_in_layer(layer, width, height) {
                if layer >= self.layer_count {
                    self.layer_count = layer + 1;
                }
                return Some((layer, x, y));
            }
        }
        None
    }

    /// Try to allocate space within a specific layer.
    fn allocate_in_layer(&mut self, layer: u32, width: u32, height: u32) -> Option<(u32, u32)> {
        let alloc = &mut self.allocated[layer as usize];

        // Simple row-based allocation (same algorithm as original)
        for x in 0..=(BLOCK_WIDTH - width) {
            let mut best_y = 0i32;

            // Find the maximum Y across the width we need
            for col in x..(x + width) {
                if alloc[col as usize] >= BLOCK_HEIGHT as i32 {
                    best_y = i32::MAX;
                    break;
                }
                if alloc[col as usize] > best_y {
                    best_y = alloc[col as usize];
                }
            }

            if best_y == i32::MAX {
                continue;
            }

            // Check if we have enough vertical space
            if best_y as u32 + height > BLOCK_HEIGHT {
                continue;
            }

            // Allocate by updating the row heights
            for col in x..(x + width) {
                alloc[col as usize] = best_y + height as i32;
            }

            return Some((x, best_y as u32));
        }

        None
    }

    /// Reset allocation state (for level reload).
    pub fn reset_allocation(&mut self) {
        for layer in self.allocated.iter_mut() {
            *layer = [0; BLOCK_WIDTH as usize];
        }
        self.layer_count = 0;
    }

    /// Batch upload multiple lightmap regions.
    ///
    /// Uses parallel CPU preparation and batched GPU upload for efficiency.
    /// Each update is specified as (layer, x, y, width, height, data).
    pub fn batch_upload_regions(&self, updates: &[(u32, u32, u32, u32, u32, Vec<u8>)]) {
        if updates.is_empty() {
            return;
        }

        let texture = match self.texture {
            Some(t) => t,
            None => return,
        };

        // Phase 1 (parallel): Prepare data and compute offsets
        let prepared: Vec<_> = updates
            .par_iter()
            .map(|(layer, x, y, width, height, data)| {
                let byte_size = (width * height * LIGHTMAP_BYTES) as usize;
                LightmapRegionUpload {
                    layer: *layer,
                    x: *x,
                    y: *y,
                    width: *width,
                    height: *height,
                    data: data.clone(),
                    byte_size,
                }
            })
            .collect();

        // Calculate total staging buffer size
        let total_size: usize = prepared.iter().map(|p| p.byte_size).sum();
        if total_size == 0 {
            return;
        }

        // Phase 2 (sequential): GPU upload via staging buffer
        gpu_device::with_device(|ctx| {
            unsafe {
                // Create staging buffer
                let buffer_info = vk::BufferCreateInfo::default()
                    .size(total_size as vk::DeviceSize)
                    .usage(vk::BufferUsageFlags::TRANSFER_SRC)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE);

                let staging_buffer = match ctx.device.create_buffer(&buffer_info, None) {
                    Ok(buf) => buf,
                    Err(_) => return,
                };

                let mem_requirements = ctx.device.get_buffer_memory_requirements(staging_buffer);
                let memory_properties = ctx.instance.get_physical_device_memory_properties(ctx.physical_device);

                // Find host-visible memory type
                let memory_type_index = (0..memory_properties.memory_type_count)
                    .find(|&i| {
                        (mem_requirements.memory_type_bits & (1 << i)) != 0 &&
                        memory_properties.memory_types[i as usize].property_flags.contains(
                            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT
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

                if ctx.device.bind_buffer_memory(staging_buffer, staging_memory, 0).is_err() {
                    ctx.device.free_memory(staging_memory, None);
                    ctx.device.destroy_buffer(staging_buffer, None);
                    return;
                }

                // Map and copy all region data to staging buffer
                let mapped_ptr = match ctx.device.map_memory(
                    staging_memory, 0, total_size as vk::DeviceSize, vk::MemoryMapFlags::empty()
                ) {
                    Ok(ptr) => ptr as *mut u8,
                    Err(_) => {
                        ctx.device.free_memory(staging_memory, None);
                        ctx.device.destroy_buffer(staging_buffer, None);
                        return;
                    }
                };

                // Copy all region data to staging buffer with computed offsets
                let mut offset = 0usize;
                let mut copy_regions = Vec::with_capacity(prepared.len());

                for region in &prepared {
                    std::ptr::copy_nonoverlapping(
                        region.data.as_ptr(),
                        mapped_ptr.add(offset),
                        region.byte_size,
                    );

                    // Build buffer→image copy region
                    copy_regions.push(vk::BufferImageCopy::default()
                        .buffer_offset(offset as vk::DeviceSize)
                        .buffer_row_length(0)
                        .buffer_image_height(0)
                        .image_subresource(vk::ImageSubresourceLayers {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            mip_level: 0,
                            base_array_layer: region.layer,
                            layer_count: 1,
                        })
                        .image_offset(vk::Offset3D {
                            x: region.x as i32,
                            y: region.y as i32,
                            z: 0,
                        })
                        .image_extent(vk::Extent3D {
                            width: region.width,
                            height: region.height,
                            depth: 1,
                        }));

                    offset += region.byte_size;
                }

                ctx.device.unmap_memory(staging_memory);

                // Record and submit copy commands
                gpu_device::with_commands_mut(|commands| {
                    let cmd = match commands.begin_single_time() {
                        Ok(c) => c,
                        Err(_) => return,
                    };

                    // Transition image to TRANSFER_DST
                    let barrier = vk::ImageMemoryBarrier::default()
                        .old_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                        .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                        .image(texture)
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: MAX_LIGHTMAPS,
                        })
                        .src_access_mask(vk::AccessFlags::SHADER_READ)
                        .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE);

                    ctx.device.cmd_pipeline_barrier(
                        cmd,
                        vk::PipelineStageFlags::FRAGMENT_SHADER,
                        vk::PipelineStageFlags::TRANSFER,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        &[barrier],
                    );

                    // Copy all regions
                    ctx.device.cmd_copy_buffer_to_image(
                        cmd,
                        staging_buffer,
                        texture,
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        &copy_regions,
                    );

                    // Transition image back to SHADER_READ_ONLY
                    let barrier = vk::ImageMemoryBarrier::default()
                        .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                        .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                        .image(texture)
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: MAX_LIGHTMAPS,
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
                });

                // Clean up staging buffer
                ctx.device.free_memory(staging_memory, None);
                ctx.device.destroy_buffer(staging_buffer, None);
            }
        });
    }

    /// Batch upload complete layers.
    ///
    /// Uses parallel processing for efficient multi-layer upload.
    /// Each entry is (layer_index, pixel_data).
    pub fn batch_upload_layers(&mut self, layers: &[(u32, Vec<u8>)]) {
        if layers.is_empty() {
            return;
        }

        // Convert to region updates (full layer = region covering entire layer)
        let region_updates: Vec<_> = layers
            .par_iter()
            .map(|(layer, data)| {
                (*layer, 0u32, 0u32, BLOCK_WIDTH, BLOCK_HEIGHT, data.clone())
            })
            .collect();

        self.batch_upload_regions(&region_updates);

        // Update layer count
        for (layer, _) in layers {
            if *layer >= self.layer_count {
                self.layer_count = layer + 1;
            }
        }
    }

    /// Bind the texture array to a texture unit (no-op in Vulkan).
    pub fn bind(&self, _unit: u32) {
        // Compatibility stub — texture binding happens at descriptor set update time.
    }

    /// Unbind from a texture unit (no-op in Vulkan).
    pub fn unbind(_unit: u32) {
        // Compatibility stub.
    }

    /// Compatibility stub: returns 0 (no GL texture ID).
    pub fn id(&self) -> u32 {
        0
    }

    /// Get the underlying Vulkan image, if allocated.
    pub fn vk_image(&self) -> Option<vk::Image> {
        self.texture
    }

    /// Get the Vulkan image view, if created.
    pub fn vk_image_view(&self) -> Option<vk::ImageView> {
        self.image_view
    }

    /// Get the Vulkan sampler, if created.
    pub fn vk_sampler(&self) -> Option<vk::Sampler> {
        self.sampler
    }

    /// Get the current layer count.
    pub fn layer_count(&self) -> u32 {
        self.layer_count
    }
}

impl Default for LightmapArray {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for LightmapArray {
    fn drop(&mut self) {
        gpu_device::with_device(|ctx| {
            // SAFETY: Vulkan context is valid and we're on the main thread.
            unsafe {
                if let Some(sampler) = self.sampler.take() {
                    ctx.device.destroy_sampler(sampler, None);
                }
                if let Some(view) = self.image_view.take() {
                    ctx.device.destroy_image_view(view, None);
                }
                if let Some(texture) = self.texture.take() {
                    ctx.device.destroy_image(texture, None);
                }
                if let Some(memory) = self.texture_memory.take() {
                    ctx.device.free_memory(memory, None);
                }
            }
        });
    }
}
