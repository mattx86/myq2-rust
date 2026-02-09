//! Memory Priority for smart GPU memory management
//!
//! VK_EXT_memory_priority allows hinting the driver about memory importance:
//! - Critical allocations kept in fastest memory
//! - Low-priority textures can be evicted first
//! - Better memory pressure handling
//! - Improved streaming texture performance

use ash::vk;

/// Memory priority levels.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum MemoryPriority {
    /// Lowest priority - can be evicted first (e.g., cached textures).
    Lowest = 0,
    /// Low priority (e.g., optional detail textures).
    Low = 1,
    /// Normal priority (default).
    Normal = 2,
    /// High priority (e.g., currently visible textures).
    High = 3,
    /// Highest priority - should never be evicted (e.g., render targets).
    Highest = 4,
}

impl MemoryPriority {
    /// Convert to Vulkan priority value (0.0 - 1.0).
    pub fn to_vk(&self) -> f32 {
        match self {
            MemoryPriority::Lowest => 0.0,
            MemoryPriority::Low => 0.25,
            MemoryPriority::Normal => 0.5,
            MemoryPriority::High => 0.75,
            MemoryPriority::Highest => 1.0,
        }
    }

    /// Create from a float value.
    pub fn from_f32(value: f32) -> Self {
        if value <= 0.125 {
            MemoryPriority::Lowest
        } else if value <= 0.375 {
            MemoryPriority::Low
        } else if value <= 0.625 {
            MemoryPriority::Normal
        } else if value <= 0.875 {
            MemoryPriority::High
        } else {
            MemoryPriority::Highest
        }
    }
}

/// Memory priority capabilities.
#[derive(Debug, Clone, Default)]
pub struct MemoryPriorityCapabilities {
    /// Whether memory priority is supported.
    pub supported: bool,
}

/// Query memory priority capabilities.
pub fn query_capabilities(ctx: &super::context::VulkanContext) -> MemoryPriorityCapabilities {
    let mut priority_features = vk::PhysicalDeviceMemoryPriorityFeaturesEXT::default();
    let mut features2 = vk::PhysicalDeviceFeatures2::default()
        .push_next(&mut priority_features);

    unsafe {
        ctx.instance.get_physical_device_features2(ctx.physical_device, &mut features2);
    }

    MemoryPriorityCapabilities {
        supported: priority_features.memory_priority == vk::TRUE,
    }
}

/// Tracked allocation with priority.
#[derive(Debug, Clone)]
pub struct TrackedAllocation {
    /// Memory handle.
    pub memory: vk::DeviceMemory,
    /// Allocation size.
    pub size: vk::DeviceSize,
    /// Current priority.
    pub priority: MemoryPriority,
    /// Allocation name for debugging.
    pub name: String,
    /// Memory type index.
    pub memory_type_index: u32,
    /// Whether this allocation is currently in use.
    pub in_use: bool,
    /// Last frame this allocation was accessed.
    pub last_access_frame: u64,
}

/// Memory priority manager.
pub struct MemoryPriorityManager {
    /// Whether memory priority is supported.
    supported: bool,
    /// Tracked allocations.
    allocations: Vec<TrackedAllocation>,
    /// Current frame number.
    current_frame: u64,
    /// Function pointer for setting memory priority.
    fp_set_priority: Option<vk::PFN_vkSetDeviceMemoryPriorityEXT>,
}

impl MemoryPriorityManager {
    /// Create a new memory priority manager.
    pub fn new(ctx: &super::context::VulkanContext) -> Self {
        let caps = query_capabilities(ctx);

        let fp_set_priority = if caps.supported {
            unsafe {
                let name = std::ffi::CStr::from_bytes_with_nul_unchecked(b"vkSetDeviceMemoryPriorityEXT\0");
                ctx.instance.get_device_proc_addr(ctx.device.handle(), name.as_ptr())
                    .map(|fp| std::mem::transmute(fp))
            }
        } else {
            None
        };

        Self {
            supported: caps.supported,
            allocations: Vec::new(),
            current_frame: 0,
            fp_set_priority,
        }
    }

    /// Check if memory priority is supported.
    pub fn is_supported(&self) -> bool {
        self.supported
    }

    /// Allocate memory with priority.
    pub fn allocate_with_priority(
        &mut self,
        ctx: &super::context::VulkanContext,
        alloc_info: &vk::MemoryAllocateInfo,
        priority: MemoryPriority,
        name: &str,
    ) -> Result<vk::DeviceMemory, String> {
        let memory = if self.supported {
            // Chain priority info
            let mut priority_info = vk::MemoryPriorityAllocateInfoEXT::default()
                .priority(priority.to_vk());

            let alloc_with_priority = vk::MemoryAllocateInfo::default()
                .allocation_size(alloc_info.allocation_size)
                .memory_type_index(alloc_info.memory_type_index)
                .push_next(&mut priority_info);

            unsafe {
                ctx.device.allocate_memory(&alloc_with_priority, None)
                    .map_err(|e| format!("Failed to allocate memory: {:?}", e))?
            }
        } else {
            unsafe {
                ctx.device.allocate_memory(alloc_info, None)
                    .map_err(|e| format!("Failed to allocate memory: {:?}", e))?
            }
        };

        // Track the allocation
        self.allocations.push(TrackedAllocation {
            memory,
            size: alloc_info.allocation_size,
            priority,
            name: name.to_string(),
            memory_type_index: alloc_info.memory_type_index,
            in_use: true,
            last_access_frame: self.current_frame,
        });

        Ok(memory)
    }

    /// Set priority for an existing allocation.
    pub fn set_priority(
        &mut self,
        ctx: &super::context::VulkanContext,
        memory: vk::DeviceMemory,
        priority: MemoryPriority,
    ) {
        // Update tracked allocation
        for alloc in &mut self.allocations {
            if alloc.memory == memory {
                alloc.priority = priority;
                break;
            }
        }

        // Update GPU priority
        if let Some(fp) = self.fp_set_priority {
            unsafe {
                fp(ctx.device.handle(), memory, priority.to_vk());
            }
        }
    }

    /// Mark an allocation as accessed this frame.
    pub fn mark_accessed(&mut self, memory: vk::DeviceMemory) {
        for alloc in &mut self.allocations {
            if alloc.memory == memory {
                alloc.last_access_frame = self.current_frame;
                alloc.in_use = true;
                break;
            }
        }
    }

    /// Update priorities based on access patterns.
    pub fn update_priorities(&mut self, ctx: &super::context::VulkanContext) {
        if !self.supported {
            return;
        }

        let current = self.current_frame;
        let stale_threshold = 120; // 2 seconds at 60fps

        for alloc in &mut self.allocations {
            let frames_since_access = current.saturating_sub(alloc.last_access_frame);

            let new_priority = if frames_since_access == 0 {
                // Accessed this frame - high priority
                MemoryPriority::High
            } else if frames_since_access < 30 {
                // Accessed recently - normal priority
                MemoryPriority::Normal
            } else if frames_since_access < stale_threshold {
                // Getting stale - low priority
                MemoryPriority::Low
            } else {
                // Very stale - lowest priority
                MemoryPriority::Lowest
            };

            if new_priority != alloc.priority {
                if let Some(fp) = self.fp_set_priority {
                    unsafe {
                        fp(ctx.device.handle(), alloc.memory, new_priority.to_vk());
                    }
                }
                alloc.priority = new_priority;
            }
        }
    }

    /// Advance to next frame.
    pub fn next_frame(&mut self) {
        self.current_frame += 1;

        // Reset in_use flags
        for alloc in &mut self.allocations {
            alloc.in_use = false;
        }
    }

    /// Free an allocation.
    pub fn free(&mut self, ctx: &super::context::VulkanContext, memory: vk::DeviceMemory) {
        self.allocations.retain(|a| a.memory != memory);

        unsafe {
            ctx.device.free_memory(memory, None);
        }
    }

    /// Get allocation statistics.
    pub fn get_stats(&self) -> MemoryPriorityStats {
        let mut stats = MemoryPriorityStats::default();

        for alloc in &self.allocations {
            stats.total_allocations += 1;
            stats.total_size += alloc.size;

            match alloc.priority {
                MemoryPriority::Lowest => {
                    stats.lowest_priority_count += 1;
                    stats.lowest_priority_size += alloc.size;
                }
                MemoryPriority::Low => {
                    stats.low_priority_count += 1;
                    stats.low_priority_size += alloc.size;
                }
                MemoryPriority::Normal => {
                    stats.normal_priority_count += 1;
                    stats.normal_priority_size += alloc.size;
                }
                MemoryPriority::High => {
                    stats.high_priority_count += 1;
                    stats.high_priority_size += alloc.size;
                }
                MemoryPriority::Highest => {
                    stats.highest_priority_count += 1;
                    stats.highest_priority_size += alloc.size;
                }
            }

            if alloc.in_use {
                stats.active_allocations += 1;
                stats.active_size += alloc.size;
            }
        }

        stats
    }

    /// Get allocations that could be evicted (lowest priority, not in use).
    pub fn get_eviction_candidates(&self) -> Vec<&TrackedAllocation> {
        self.allocations
            .iter()
            .filter(|a| !a.in_use && a.priority <= MemoryPriority::Low)
            .collect()
    }

    /// Clear all tracking (for shutdown).
    pub fn clear(&mut self) {
        self.allocations.clear();
    }
}

/// Memory priority statistics.
#[derive(Debug, Clone, Default)]
pub struct MemoryPriorityStats {
    /// Total number of tracked allocations.
    pub total_allocations: u32,
    /// Total size of all allocations.
    pub total_size: vk::DeviceSize,
    /// Number of active allocations.
    pub active_allocations: u32,
    /// Size of active allocations.
    pub active_size: vk::DeviceSize,
    /// Lowest priority count.
    pub lowest_priority_count: u32,
    /// Lowest priority size.
    pub lowest_priority_size: vk::DeviceSize,
    /// Low priority count.
    pub low_priority_count: u32,
    /// Low priority size.
    pub low_priority_size: vk::DeviceSize,
    /// Normal priority count.
    pub normal_priority_count: u32,
    /// Normal priority size.
    pub normal_priority_size: vk::DeviceSize,
    /// High priority count.
    pub high_priority_count: u32,
    /// High priority size.
    pub high_priority_size: vk::DeviceSize,
    /// Highest priority count.
    pub highest_priority_count: u32,
    /// Highest priority size.
    pub highest_priority_size: vk::DeviceSize,
}

/// Resource type for automatic priority assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    /// Render target (framebuffer attachment).
    RenderTarget,
    /// Depth/stencil buffer.
    DepthBuffer,
    /// Uniform/constant buffer.
    UniformBuffer,
    /// Vertex/index buffer.
    GeometryBuffer,
    /// Texture currently visible.
    VisibleTexture,
    /// Texture in streaming pool.
    StreamingTexture,
    /// Texture in cache (not recently used).
    CachedTexture,
    /// Staging buffer (temporary).
    StagingBuffer,
}

impl ResourceType {
    /// Get default priority for this resource type.
    pub fn default_priority(&self) -> MemoryPriority {
        match self {
            ResourceType::RenderTarget => MemoryPriority::Highest,
            ResourceType::DepthBuffer => MemoryPriority::Highest,
            ResourceType::UniformBuffer => MemoryPriority::High,
            ResourceType::GeometryBuffer => MemoryPriority::High,
            ResourceType::VisibleTexture => MemoryPriority::High,
            ResourceType::StreamingTexture => MemoryPriority::Normal,
            ResourceType::CachedTexture => MemoryPriority::Low,
            ResourceType::StagingBuffer => MemoryPriority::Lowest,
        }
    }
}
