//! Bindless texture system using VK_EXT_descriptor_indexing
//!
//! Provides a single descriptor set containing all textures, allowing shaders
//! to index into the texture array dynamically. This eliminates descriptor
//! set switching overhead and enables more flexible rendering.
//!
//! Benefits:
//! - Single descriptor set bind per frame
//! - Dynamic texture selection in shaders
//! - Reduced CPU overhead from descriptor management
//! - Enables GPU-driven rendering patterns

use ash::vk;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

/// Maximum number of bindless textures.
pub const MAX_BINDLESS_TEXTURES: u32 = 4096;

/// Maximum number of bindless samplers.
pub const MAX_BINDLESS_SAMPLERS: u32 = 16;

/// Handle to a bindless texture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BindlessTextureHandle(pub u32);

impl BindlessTextureHandle {
    /// Invalid/null handle.
    pub const INVALID: Self = Self(u32::MAX);

    /// Check if this handle is valid.
    pub fn is_valid(&self) -> bool {
        self.0 != u32::MAX && self.0 < MAX_BINDLESS_TEXTURES
    }
}

/// Bindless texture capabilities.
#[derive(Debug, Clone)]
pub struct BindlessCapabilities {
    /// Whether bindless textures are supported.
    pub supported: bool,
    /// Whether partially bound descriptors are supported.
    pub partially_bound: bool,
    /// Whether update after bind is supported.
    pub update_after_bind: bool,
    /// Maximum number of sampled images per stage.
    pub max_sampled_images: u32,
    /// Maximum number of samplers per stage.
    pub max_samplers: u32,
    /// Whether runtime descriptor arrays are supported.
    pub runtime_descriptor_array: bool,
}

impl Default for BindlessCapabilities {
    fn default() -> Self {
        Self {
            supported: false,
            partially_bound: false,
            update_after_bind: false,
            max_sampled_images: 0,
            max_samplers: 0,
            runtime_descriptor_array: false,
        }
    }
}

/// Texture slot info for the bindless array.
#[derive(Debug, Clone)]
struct TextureSlot {
    /// Image view.
    view: vk::ImageView,
    /// Sampler index (into sampler array).
    sampler_index: u32,
    /// Whether this slot is in use.
    in_use: bool,
    /// Debug name.
    name: String,
}

/// Bindless texture manager.
pub struct BindlessManager {
    /// Capabilities.
    capabilities: BindlessCapabilities,
    /// Descriptor set layout for bindless textures.
    descriptor_layout: vk::DescriptorSetLayout,
    /// Descriptor pool.
    descriptor_pool: vk::DescriptorPool,
    /// The bindless descriptor set.
    descriptor_set: vk::DescriptorSet,
    /// Texture slots.
    slots: Vec<Option<TextureSlot>>,
    /// Free slot indices.
    free_slots: Vec<u32>,
    /// Next slot index for allocation.
    next_slot: AtomicU32,
    /// Name to handle mapping for fast lookup.
    name_to_handle: HashMap<String, BindlessTextureHandle>,
    /// Samplers array.
    samplers: Vec<vk::Sampler>,
    /// Default sampler index.
    default_sampler: u32,
    /// Pending descriptor updates.
    pending_updates: Vec<(u32, vk::ImageView, u32)>,
}

impl BindlessManager {
    /// Query bindless capabilities from device.
    pub fn query_capabilities(ctx: &super::context::VulkanContext) -> BindlessCapabilities {
        // Query descriptor indexing features
        let mut indexing_features = vk::PhysicalDeviceDescriptorIndexingFeatures::default();
        let mut features2 = vk::PhysicalDeviceFeatures2::default()
            .push_next(&mut indexing_features);

        unsafe {
            ctx.instance.get_physical_device_features2(ctx.physical_device, &mut features2);
        }

        // Query descriptor indexing properties
        let mut indexing_props = vk::PhysicalDeviceDescriptorIndexingProperties::default();
        let mut props2 = vk::PhysicalDeviceProperties2::default()
            .push_next(&mut indexing_props);

        unsafe {
            ctx.instance.get_physical_device_properties2(ctx.physical_device, &mut props2);
        }

        let supported = indexing_features.descriptor_binding_sampled_image_update_after_bind == vk::TRUE
            && indexing_features.descriptor_binding_partially_bound == vk::TRUE
            && indexing_features.runtime_descriptor_array == vk::TRUE;

        BindlessCapabilities {
            supported,
            partially_bound: indexing_features.descriptor_binding_partially_bound == vk::TRUE,
            update_after_bind: indexing_features.descriptor_binding_sampled_image_update_after_bind == vk::TRUE,
            max_sampled_images: indexing_props.max_descriptor_set_update_after_bind_sampled_images,
            max_samplers: indexing_props.max_descriptor_set_update_after_bind_samplers,
            runtime_descriptor_array: indexing_features.runtime_descriptor_array == vk::TRUE,
        }
    }

    /// Create a new bindless manager.
    pub fn new(ctx: &super::context::VulkanContext) -> Result<Self, String> {
        let capabilities = Self::query_capabilities(ctx);

        if !capabilities.supported {
            return Err("Bindless textures not supported".to_string());
        }

        // Create descriptor set layout with UPDATE_AFTER_BIND flag
        let texture_binding = vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
            .descriptor_count(MAX_BINDLESS_TEXTURES)
            .stage_flags(vk::ShaderStageFlags::ALL);

        let sampler_binding = vk::DescriptorSetLayoutBinding::default()
            .binding(1)
            .descriptor_type(vk::DescriptorType::SAMPLER)
            .descriptor_count(MAX_BINDLESS_SAMPLERS)
            .stage_flags(vk::ShaderStageFlags::ALL);

        let bindings = [texture_binding, sampler_binding];

        // Binding flags for partially bound and update after bind
        let binding_flags = [
            vk::DescriptorBindingFlags::PARTIALLY_BOUND
                | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND,
            vk::DescriptorBindingFlags::PARTIALLY_BOUND
                | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND,
        ];

        let mut binding_flags_info = vk::DescriptorSetLayoutBindingFlagsCreateInfo::default()
            .binding_flags(&binding_flags);

        let layout_info = vk::DescriptorSetLayoutCreateInfo::default()
            .bindings(&bindings)
            .flags(vk::DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL)
            .push_next(&mut binding_flags_info);

        let descriptor_layout = unsafe {
            ctx.device.create_descriptor_set_layout(&layout_info, None)
                .map_err(|e| format!("Failed to create bindless layout: {:?}", e))?
        };

        // Create descriptor pool with UPDATE_AFTER_BIND flag
        let pool_sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::SAMPLED_IMAGE,
                descriptor_count: MAX_BINDLESS_TEXTURES,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::SAMPLER,
                descriptor_count: MAX_BINDLESS_SAMPLERS,
            },
        ];

        let pool_info = vk::DescriptorPoolCreateInfo::default()
            .pool_sizes(&pool_sizes)
            .max_sets(1)
            .flags(vk::DescriptorPoolCreateFlags::UPDATE_AFTER_BIND);

        let descriptor_pool = unsafe {
            ctx.device.create_descriptor_pool(&pool_info, None)
                .map_err(|e| format!("Failed to create bindless pool: {:?}", e))?
        };

        // Allocate the descriptor set
        let layouts = [descriptor_layout];
        let alloc_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&layouts);

        let descriptor_set = unsafe {
            ctx.device.allocate_descriptor_sets(&alloc_info)
                .map_err(|e| format!("Failed to allocate bindless set: {:?}", e))?[0]
        };

        // Initialize slots
        let slots = (0..MAX_BINDLESS_TEXTURES).map(|_| None).collect();
        let free_slots = Vec::new();

        Ok(Self {
            capabilities,
            descriptor_layout,
            descriptor_pool,
            descriptor_set,
            slots,
            free_slots,
            next_slot: AtomicU32::new(0),
            name_to_handle: HashMap::new(),
            samplers: Vec::new(),
            default_sampler: 0,
            pending_updates: Vec::new(),
        })
    }

    /// Check if bindless textures are supported.
    pub fn is_supported(&self) -> bool {
        self.capabilities.supported
    }

    /// Get capabilities.
    pub fn capabilities(&self) -> &BindlessCapabilities {
        &self.capabilities
    }

    /// Get the descriptor set layout.
    pub fn layout(&self) -> vk::DescriptorSetLayout {
        self.descriptor_layout
    }

    /// Get the descriptor set.
    pub fn descriptor_set(&self) -> vk::DescriptorSet {
        self.descriptor_set
    }

    /// Register a sampler and return its index.
    pub fn register_sampler(&mut self, ctx: &super::context::VulkanContext, sampler: vk::Sampler) -> u32 {
        let index = self.samplers.len() as u32;
        self.samplers.push(sampler);

        // Update descriptor
        let sampler_info = vk::DescriptorImageInfo::default()
            .sampler(sampler);

        let write = vk::WriteDescriptorSet::default()
            .dst_set(self.descriptor_set)
            .dst_binding(1)
            .dst_array_element(index)
            .descriptor_type(vk::DescriptorType::SAMPLER)
            .image_info(std::slice::from_ref(&sampler_info));

        unsafe {
            ctx.device.update_descriptor_sets(&[write], &[]);
        }

        index
    }

    /// Set the default sampler.
    pub fn set_default_sampler(&mut self, index: u32) {
        self.default_sampler = index;
    }

    /// Allocate a texture handle.
    fn allocate_slot(&mut self) -> Option<u32> {
        // Try to reuse a free slot
        if let Some(slot) = self.free_slots.pop() {
            return Some(slot);
        }

        // Allocate a new slot
        let slot = self.next_slot.fetch_add(1, Ordering::Relaxed);
        if slot < MAX_BINDLESS_TEXTURES {
            Some(slot)
        } else {
            None
        }
    }

    /// Register a texture and return its handle.
    pub fn register_texture(
        &mut self,
        name: &str,
        view: vk::ImageView,
        sampler_index: Option<u32>,
    ) -> Option<BindlessTextureHandle> {
        // Check if already registered
        if let Some(&handle) = self.name_to_handle.get(name) {
            return Some(handle);
        }

        let slot = self.allocate_slot()?;
        let sampler_idx = sampler_index.unwrap_or(self.default_sampler);

        self.slots[slot as usize] = Some(TextureSlot {
            view,
            sampler_index: sampler_idx,
            in_use: true,
            name: name.to_string(),
        });

        let handle = BindlessTextureHandle(slot);
        self.name_to_handle.insert(name.to_string(), handle);

        // Queue descriptor update
        self.pending_updates.push((slot, view, sampler_idx));

        Some(handle)
    }

    /// Update a texture's image view.
    pub fn update_texture(&mut self, handle: BindlessTextureHandle, view: vk::ImageView) {
        if !handle.is_valid() {
            return;
        }

        if let Some(ref mut slot) = self.slots[handle.0 as usize] {
            slot.view = view;
            self.pending_updates.push((handle.0, view, slot.sampler_index));
        }
    }

    /// Unregister a texture and free its slot.
    pub fn unregister_texture(&mut self, handle: BindlessTextureHandle) {
        if !handle.is_valid() {
            return;
        }

        if let Some(slot) = self.slots[handle.0 as usize].take() {
            self.name_to_handle.remove(&slot.name);
            self.free_slots.push(handle.0);
        }
    }

    /// Get a texture handle by name.
    pub fn get_handle(&self, name: &str) -> Option<BindlessTextureHandle> {
        self.name_to_handle.get(name).copied()
    }

    /// Flush pending descriptor updates.
    pub fn flush_updates(&mut self, ctx: &super::context::VulkanContext) {
        if self.pending_updates.is_empty() {
            return;
        }

        // Build descriptor writes
        let image_infos: Vec<vk::DescriptorImageInfo> = self.pending_updates
            .iter()
            .map(|(_, view, _)| {
                vk::DescriptorImageInfo::default()
                    .image_view(*view)
                    .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            })
            .collect();

        let writes: Vec<vk::WriteDescriptorSet> = self.pending_updates
            .iter()
            .zip(image_infos.iter())
            .map(|((slot, _, _), info)| {
                vk::WriteDescriptorSet::default()
                    .dst_set(self.descriptor_set)
                    .dst_binding(0)
                    .dst_array_element(*slot)
                    .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                    .image_info(std::slice::from_ref(info))
            })
            .collect();

        unsafe {
            ctx.device.update_descriptor_sets(&writes, &[]);
        }

        self.pending_updates.clear();
    }

    /// Bind the bindless descriptor set to a command buffer.
    pub fn bind(
        &self,
        ctx: &super::context::VulkanContext,
        cmd: vk::CommandBuffer,
        pipeline_layout: vk::PipelineLayout,
        set_index: u32,
    ) {
        unsafe {
            ctx.device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline_layout,
                set_index,
                &[self.descriptor_set],
                &[],
            );
        }
    }

    /// Destroy the bindless manager.
    pub fn destroy(&mut self, ctx: &super::context::VulkanContext) {
        unsafe {
            ctx.device.destroy_descriptor_pool(self.descriptor_pool, None);
            ctx.device.destroy_descriptor_set_layout(self.descriptor_layout, None);
        }

        self.slots.clear();
        self.free_slots.clear();
        self.name_to_handle.clear();
        self.samplers.clear();
        self.pending_updates.clear();
    }
}

impl Drop for BindlessManager {
    fn drop(&mut self) {
        // Note: Vulkan resources should be destroyed via destroy() before drop
        // This is just for safety
        self.slots.clear();
        self.free_slots.clear();
        self.name_to_handle.clear();
    }
}

/// GLSL shader code for accessing bindless textures.
pub const BINDLESS_GLSL: &str = r#"
// Bindless texture declarations
// Include this in your shader with: #include "bindless.glsl"

#extension GL_EXT_nonuniform_qualifier : require

layout(set = 0, binding = 0) uniform texture2D u_Textures[];
layout(set = 0, binding = 1) uniform sampler u_Samplers[];

// Sample a bindless texture
vec4 sampleBindless(uint textureIndex, uint samplerIndex, vec2 uv) {
    return texture(sampler2D(u_Textures[nonuniformEXT(textureIndex)],
                             u_Samplers[nonuniformEXT(samplerIndex)]), uv);
}

// Sample with default sampler (index 0)
vec4 sampleBindlessDefault(uint textureIndex, vec2 uv) {
    return texture(sampler2D(u_Textures[nonuniformEXT(textureIndex)],
                             u_Samplers[0]), uv);
}

// Sample with LOD bias
vec4 sampleBindlessBias(uint textureIndex, uint samplerIndex, vec2 uv, float bias) {
    return texture(sampler2D(u_Textures[nonuniformEXT(textureIndex)],
                             u_Samplers[nonuniformEXT(samplerIndex)]), uv, bias);
}
"#;
