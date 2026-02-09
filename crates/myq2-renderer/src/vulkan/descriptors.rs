//! Vulkan descriptor set management.

use ash::vk;
use std::collections::HashMap;

use super::VulkanContext;

/// Descriptor set layouts for different binding sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DescriptorSetType {
    /// Set 0: Per-frame data (camera, time, etc.)
    PerFrame,
    /// Set 1: Material textures
    Material,
    /// Set 2: Lightmap array
    Lightmap,
    /// Set 3: Per-object uniforms
    PerObject,
    /// Set 4: Ray tracing TLAS
    RayTracing,
}

/// Manages descriptor pools, layouts, and sets.
pub struct DescriptorManager {
    pool: vk::DescriptorPool,
    layouts: HashMap<DescriptorSetType, vk::DescriptorSetLayout>,
    pipeline_layouts: HashMap<String, vk::PipelineLayout>,
    device: ash::Device,
}

impl DescriptorManager {
    /// Create a new descriptor manager with required layouts.
    pub unsafe fn new(ctx: &VulkanContext) -> Result<Self, String> {
        // Create descriptor pool with sufficient capacity
        let pool_sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: 1000,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: 5000,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::STORAGE_BUFFER,
                descriptor_count: 500,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::ACCELERATION_STRUCTURE_KHR,
                descriptor_count: 10,
            },
        ];

        let pool_info = vk::DescriptorPoolCreateInfo::default()
            .pool_sizes(&pool_sizes)
            .max_sets(2000)
            .flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET);

        let pool = ctx.device.create_descriptor_pool(&pool_info, None)
            .map_err(|e| format!("Failed to create descriptor pool: {:?}", e))?;

        // Create standard layouts
        let mut layouts = HashMap::new();

        // Set 0: Per-frame (camera matrix, time, etc.)
        let per_frame_bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT),
        ];
        layouts.insert(DescriptorSetType::PerFrame, Self::create_layout(ctx, &per_frame_bindings)?);

        // Set 1: Material textures (bindless array)
        let material_bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(256) // Max textures
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];
        let material_binding_flags = [vk::DescriptorBindingFlags::PARTIALLY_BOUND |
                                       vk::DescriptorBindingFlags::VARIABLE_DESCRIPTOR_COUNT];
        let mut binding_flags_info = vk::DescriptorSetLayoutBindingFlagsCreateInfo::default()
            .binding_flags(&material_binding_flags);
        layouts.insert(DescriptorSetType::Material,
            Self::create_layout_with_flags(ctx, &material_bindings, &mut binding_flags_info)?);

        // Set 2: Lightmap array texture
        let lightmap_bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];
        layouts.insert(DescriptorSetType::Lightmap, Self::create_layout(ctx, &lightmap_bindings)?);

        // Set 3: Per-object uniforms
        let per_object_bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT),
        ];
        layouts.insert(DescriptorSetType::PerObject, Self::create_layout(ctx, &per_object_bindings)?);

        // Set 4: Ray tracing (TLAS)
        if ctx.rt_capabilities.supported {
            let rt_bindings = [
                vk::DescriptorSetLayoutBinding::default()
                    .binding(0)
                    .descriptor_type(vk::DescriptorType::ACCELERATION_STRUCTURE_KHR)
                    .descriptor_count(1)
                    .stage_flags(vk::ShaderStageFlags::FRAGMENT | vk::ShaderStageFlags::RAYGEN_KHR),
            ];
            layouts.insert(DescriptorSetType::RayTracing, Self::create_layout(ctx, &rt_bindings)?);
        }

        Ok(Self {
            pool,
            layouts,
            pipeline_layouts: HashMap::new(),
            device: ctx.device.clone(),
        })
    }

    /// Create a descriptor set layout from bindings.
    unsafe fn create_layout(
        ctx: &VulkanContext,
        bindings: &[vk::DescriptorSetLayoutBinding],
    ) -> Result<vk::DescriptorSetLayout, String> {
        let layout_info = vk::DescriptorSetLayoutCreateInfo::default()
            .bindings(bindings);

        ctx.device.create_descriptor_set_layout(&layout_info, None)
            .map_err(|e| format!("Failed to create descriptor set layout: {:?}", e))
    }

    /// Create a descriptor set layout with binding flags.
    unsafe fn create_layout_with_flags(
        ctx: &VulkanContext,
        bindings: &[vk::DescriptorSetLayoutBinding],
        flags: &mut vk::DescriptorSetLayoutBindingFlagsCreateInfo,
    ) -> Result<vk::DescriptorSetLayout, String> {
        let layout_info = vk::DescriptorSetLayoutCreateInfo::default()
            .bindings(bindings)
            .push_next(flags);

        ctx.device.create_descriptor_set_layout(&layout_info, None)
            .map_err(|e| format!("Failed to create descriptor set layout: {:?}", e))
    }

    /// Get a descriptor set layout.
    pub fn get_layout(&self, set_type: DescriptorSetType) -> Option<vk::DescriptorSetLayout> {
        self.layouts.get(&set_type).copied()
    }

    /// Create a pipeline layout from descriptor set layouts.
    pub unsafe fn create_pipeline_layout(
        &mut self,
        ctx: &VulkanContext,
        name: &str,
        set_types: &[DescriptorSetType],
        push_constant_range: Option<vk::PushConstantRange>,
    ) -> Result<vk::PipelineLayout, String> {
        let set_layouts: Vec<_> = set_types.iter()
            .filter_map(|t| self.layouts.get(t).copied())
            .collect();

        let push_constant_ranges = push_constant_range
            .map(|r| vec![r])
            .unwrap_or_default();

        let layout_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(&set_layouts)
            .push_constant_ranges(&push_constant_ranges);

        let layout = ctx.device.create_pipeline_layout(&layout_info, None)
            .map_err(|e| format!("Failed to create pipeline layout: {:?}", e))?;

        self.pipeline_layouts.insert(name.to_string(), layout);
        Ok(layout)
    }

    /// Get a pipeline layout by name.
    pub fn get_pipeline_layout(&self, name: &str) -> Option<vk::PipelineLayout> {
        self.pipeline_layouts.get(name).copied()
    }

    /// Allocate descriptor sets.
    pub unsafe fn allocate_sets(
        &self,
        set_type: DescriptorSetType,
        count: u32,
    ) -> Result<Vec<vk::DescriptorSet>, String> {
        let layout = self.layouts.get(&set_type)
            .ok_or_else(|| format!("Unknown descriptor set type: {:?}", set_type))?;

        let layouts = vec![*layout; count as usize];

        let alloc_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(self.pool)
            .set_layouts(&layouts);

        self.device.allocate_descriptor_sets(&alloc_info)
            .map_err(|e| format!("Failed to allocate descriptor sets: {:?}", e))
    }

    /// Update a descriptor set with a uniform buffer.
    pub unsafe fn update_uniform_buffer(
        &self,
        set: vk::DescriptorSet,
        binding: u32,
        buffer: vk::Buffer,
        offset: vk::DeviceSize,
        range: vk::DeviceSize,
    ) {
        let buffer_info = vk::DescriptorBufferInfo {
            buffer,
            offset,
            range,
        };

        let write = vk::WriteDescriptorSet::default()
            .dst_set(set)
            .dst_binding(binding)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .buffer_info(std::slice::from_ref(&buffer_info));

        self.device.update_descriptor_sets(&[write], &[]);
    }

    /// Update a descriptor set with an image sampler.
    pub unsafe fn update_image_sampler(
        &self,
        set: vk::DescriptorSet,
        binding: u32,
        array_element: u32,
        image_view: vk::ImageView,
        sampler: vk::Sampler,
        layout: vk::ImageLayout,
    ) {
        let image_info = vk::DescriptorImageInfo {
            sampler,
            image_view,
            image_layout: layout,
        };

        let write = vk::WriteDescriptorSet::default()
            .dst_set(set)
            .dst_binding(binding)
            .dst_array_element(array_element)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .image_info(std::slice::from_ref(&image_info));

        self.device.update_descriptor_sets(&[write], &[]);
    }

    /// Batch update descriptor sets with multiple image samplers.
    ///
    /// More efficient than individual updates when updating many descriptors.
    /// Collects all writes and submits in a single Vulkan call.
    ///
    /// # Arguments
    /// * `updates` - Vector of (set, binding, array_element, image_view, sampler, layout)
    pub unsafe fn batch_update_image_samplers(
        &self,
        updates: &[(vk::DescriptorSet, u32, u32, vk::ImageView, vk::Sampler, vk::ImageLayout)],
    ) {
        if updates.is_empty() {
            return;
        }

        // Pre-allocate storage for image infos (must outlive writes)
        let image_infos: Vec<vk::DescriptorImageInfo> = updates.iter()
            .map(|&(_, _, _, image_view, sampler, layout)| {
                vk::DescriptorImageInfo {
                    sampler,
                    image_view,
                    image_layout: layout,
                }
            })
            .collect();

        // Build write descriptors referencing the image infos
        let writes: Vec<vk::WriteDescriptorSet> = updates.iter()
            .enumerate()
            .map(|(i, &(set, binding, array_element, _, _, _))| {
                vk::WriteDescriptorSet::default()
                    .dst_set(set)
                    .dst_binding(binding)
                    .dst_array_element(array_element)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(std::slice::from_ref(&image_infos[i]))
            })
            .collect();

        // Single Vulkan call for all updates
        self.device.update_descriptor_sets(&writes, &[]);
    }

    /// Batch update descriptor sets with multiple uniform buffers.
    ///
    /// More efficient than individual updates when updating many descriptors.
    ///
    /// # Arguments
    /// * `updates` - Vector of (set, binding, buffer, offset, range)
    pub unsafe fn batch_update_uniform_buffers(
        &self,
        updates: &[(vk::DescriptorSet, u32, vk::Buffer, vk::DeviceSize, vk::DeviceSize)],
    ) {
        if updates.is_empty() {
            return;
        }

        // Pre-allocate storage for buffer infos (must outlive writes)
        let buffer_infos: Vec<vk::DescriptorBufferInfo> = updates.iter()
            .map(|&(_, _, buffer, offset, range)| {
                vk::DescriptorBufferInfo {
                    buffer,
                    offset,
                    range,
                }
            })
            .collect();

        // Build write descriptors referencing the buffer infos
        let writes: Vec<vk::WriteDescriptorSet> = updates.iter()
            .enumerate()
            .map(|(i, &(set, binding, _, _, _))| {
                vk::WriteDescriptorSet::default()
                    .dst_set(set)
                    .dst_binding(binding)
                    .dst_array_element(0)
                    .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                    .buffer_info(std::slice::from_ref(&buffer_infos[i]))
            })
            .collect();

        // Single Vulkan call for all updates
        self.device.update_descriptor_sets(&writes, &[]);
    }

    /// Free descriptor sets.
    pub unsafe fn free_sets(&self, sets: &[vk::DescriptorSet]) -> Result<(), String> {
        self.device.free_descriptor_sets(self.pool, sets)
            .map_err(|e| format!("Failed to free descriptor sets: {:?}", e))
    }

    /// Destroy all resources.
    pub unsafe fn destroy(&mut self, ctx: &VulkanContext) {
        for (_, layout) in self.pipeline_layouts.drain() {
            ctx.device.destroy_pipeline_layout(layout, None);
        }

        for (_, layout) in self.layouts.drain() {
            ctx.device.destroy_descriptor_set_layout(layout, None);
        }

        ctx.device.destroy_descriptor_pool(self.pool, None);
    }
}
