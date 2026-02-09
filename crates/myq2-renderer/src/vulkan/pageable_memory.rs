//! Pageable Device Local Memory
//!
//! VK_EXT_pageable_device_local_memory provides better memory paging hints:
//! - Control memory residency
//! - Hint which allocations can be paged out
//! - Optimize for memory-constrained scenarios
//! - Better memory management for large scenes

use ash::vk;

/// Pageable memory capabilities.
#[derive(Debug, Clone, Default)]
pub struct PageableMemoryCapabilities {
    /// Whether pageable device local memory is supported.
    pub supported: bool,
}

/// Query pageable memory capabilities.
pub fn query_capabilities(ctx: &super::context::VulkanContext) -> PageableMemoryCapabilities {
    let mut pageable_features = vk::PhysicalDevicePageableDeviceLocalMemoryFeaturesEXT::default();
    let mut features2 = vk::PhysicalDeviceFeatures2::default()
        .push_next(&mut pageable_features);

    unsafe {
        ctx.instance.get_physical_device_features2(ctx.physical_device, &mut features2);
    }

    let _ = features2;

    PageableMemoryCapabilities {
        supported: pageable_features.pageable_device_local_memory == vk::TRUE,
    }
}

/// Memory priority hint for pageable allocations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PagePriority {
    /// Lowest priority - page out first.
    Lowest,
    /// Low priority.
    Low,
    /// Normal priority.
    Normal,
    /// High priority.
    High,
    /// Highest priority - keep resident.
    Highest,
}

impl PagePriority {
    /// Convert to priority value (0.0 - 1.0).
    pub fn to_value(&self) -> f32 {
        match self {
            PagePriority::Lowest => 0.0,
            PagePriority::Low => 0.25,
            PagePriority::Normal => 0.5,
            PagePriority::High => 0.75,
            PagePriority::Highest => 1.0,
        }
    }

    /// Create from value.
    pub fn from_value(value: f32) -> Self {
        if value < 0.125 {
            PagePriority::Lowest
        } else if value < 0.375 {
            PagePriority::Low
        } else if value < 0.625 {
            PagePriority::Normal
        } else if value < 0.875 {
            PagePriority::High
        } else {
            PagePriority::Highest
        }
    }
}

/// Pageable memory function pointers.
pub struct PageableMemoryFunctions {
    fp_set_memory_priority: Option<vk::PFN_vkSetDeviceMemoryPriorityEXT>,
}

impl PageableMemoryFunctions {
    /// Load function pointers.
    pub fn new(ctx: &super::context::VulkanContext) -> Self {
        let fp_set_memory_priority = unsafe {
            let name = std::ffi::CStr::from_bytes_with_nul_unchecked(
                b"vkSetDeviceMemoryPriorityEXT\0"
            );
            ctx.instance.get_device_proc_addr(ctx.device.handle(), name.as_ptr())
                .map(|fp| std::mem::transmute(fp))
        };

        Self {
            fp_set_memory_priority,
        }
    }

    /// Check if available.
    pub fn is_available(&self) -> bool {
        self.fp_set_memory_priority.is_some()
    }
}

/// Set memory priority for an allocation.
pub fn set_memory_priority(
    ctx: &super::context::VulkanContext,
    funcs: &PageableMemoryFunctions,
    memory: vk::DeviceMemory,
    priority: PagePriority,
) -> Result<(), String> {
    let fp = funcs.fp_set_memory_priority
        .ok_or("Pageable device local memory not supported")?;

    unsafe {
        fp(ctx.device.handle(), memory, priority.to_value());
    }

    Ok(())
}

/// Memory residency hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidencyHint {
    /// Memory should remain resident.
    Resident,
    /// Memory can be paged out if needed.
    Pageable,
    /// Memory is not currently needed.
    Evictable,
}

/// Tracked pageable allocation.
#[derive(Debug)]
pub struct PageableAllocation {
    /// Device memory handle.
    pub memory: vk::DeviceMemory,
    /// Allocation size.
    pub size: vk::DeviceSize,
    /// Current priority.
    pub priority: PagePriority,
    /// Residency hint.
    pub residency: ResidencyHint,
    /// Last access time (frame number).
    pub last_access_frame: u64,
    /// Name for debugging.
    pub name: String,
}

impl PageableAllocation {
    /// Create new allocation tracker.
    pub fn new(
        memory: vk::DeviceMemory,
        size: vk::DeviceSize,
        priority: PagePriority,
        name: &str,
    ) -> Self {
        Self {
            memory,
            size,
            priority,
            residency: ResidencyHint::Resident,
            last_access_frame: 0,
            name: name.to_string(),
        }
    }

    /// Mark as accessed.
    pub fn mark_accessed(&mut self, frame: u64) {
        self.last_access_frame = frame;
        self.residency = ResidencyHint::Resident;
    }

    /// Mark as unused.
    pub fn mark_unused(&mut self) {
        self.residency = ResidencyHint::Pageable;
    }

    /// Check if stale (not accessed for many frames).
    pub fn is_stale(&self, current_frame: u64, threshold: u64) -> bool {
        current_frame.saturating_sub(self.last_access_frame) > threshold
    }
}

/// Manager for pageable memory allocations.
pub struct PageableMemoryManager {
    /// Function pointers.
    funcs: PageableMemoryFunctions,
    /// Tracked allocations.
    allocations: Vec<PageableAllocation>,
    /// Current frame.
    current_frame: u64,
    /// Stale threshold (frames).
    stale_threshold: u64,
}

impl PageableMemoryManager {
    /// Create new manager.
    pub fn new(ctx: &super::context::VulkanContext) -> Self {
        Self {
            funcs: PageableMemoryFunctions::new(ctx),
            allocations: Vec::new(),
            current_frame: 0,
            stale_threshold: 60, // 1 second at 60fps
        }
    }

    /// Check if pageable memory is supported.
    pub fn is_supported(&self) -> bool {
        self.funcs.is_available()
    }

    /// Track a new allocation.
    pub fn track(
        &mut self,
        memory: vk::DeviceMemory,
        size: vk::DeviceSize,
        priority: PagePriority,
        name: &str,
    ) {
        self.allocations.push(PageableAllocation::new(memory, size, priority, name));
    }

    /// Update priorities based on usage.
    pub fn update(&mut self, ctx: &super::context::VulkanContext) {
        self.current_frame += 1;

        if !self.funcs.is_available() {
            return;
        }

        for alloc in &mut self.allocations {
            if alloc.is_stale(self.current_frame, self.stale_threshold) {
                // Reduce priority for stale allocations
                let new_priority = match alloc.priority {
                    PagePriority::Highest => PagePriority::High,
                    PagePriority::High => PagePriority::Normal,
                    PagePriority::Normal => PagePriority::Low,
                    PagePriority::Low => PagePriority::Lowest,
                    PagePriority::Lowest => PagePriority::Lowest,
                };

                if new_priority != alloc.priority {
                    alloc.priority = new_priority;
                    let _ = set_memory_priority(ctx, &self.funcs, alloc.memory, new_priority);
                }
            }
        }
    }

    /// Mark allocation as accessed.
    pub fn mark_accessed(&mut self, memory: vk::DeviceMemory) {
        if let Some(alloc) = self.allocations.iter_mut().find(|a| a.memory == memory) {
            alloc.mark_accessed(self.current_frame);
        }
    }

    /// Remove tracking for freed memory.
    pub fn untrack(&mut self, memory: vk::DeviceMemory) {
        self.allocations.retain(|a| a.memory != memory);
    }

    /// Get total tracked memory size.
    pub fn total_tracked_size(&self) -> vk::DeviceSize {
        self.allocations.iter().map(|a| a.size).sum()
    }

    /// Get count by priority.
    pub fn count_by_priority(&self, priority: PagePriority) -> usize {
        self.allocations.iter().filter(|a| a.priority == priority).count()
    }
}

/// Recommendations for allocation priorities.
pub fn recommend_priority(usage: MemoryUsageType) -> PagePriority {
    match usage {
        MemoryUsageType::RenderTarget => PagePriority::Highest,
        MemoryUsageType::DepthBuffer => PagePriority::Highest,
        MemoryUsageType::FrequentTexture => PagePriority::High,
        MemoryUsageType::LevelGeometry => PagePriority::High,
        MemoryUsageType::InfrequentTexture => PagePriority::Normal,
        MemoryUsageType::StaticData => PagePriority::Normal,
        MemoryUsageType::StreamingData => PagePriority::Low,
        MemoryUsageType::Cache => PagePriority::Lowest,
    }
}

/// Memory usage type for priority recommendations.
#[derive(Debug, Clone, Copy)]
pub enum MemoryUsageType {
    /// Active render target.
    RenderTarget,
    /// Depth buffer.
    DepthBuffer,
    /// Frequently accessed texture.
    FrequentTexture,
    /// Level geometry (BSP, etc.).
    LevelGeometry,
    /// Infrequently accessed texture.
    InfrequentTexture,
    /// Static data.
    StaticData,
    /// Streaming data.
    StreamingData,
    /// Cache data.
    Cache,
}
