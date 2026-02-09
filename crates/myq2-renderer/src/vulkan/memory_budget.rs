//! Memory budget tracking for GPU memory management
//!
//! VK_EXT_memory_budget provides information about GPU memory heaps,
//! allowing the application to make informed decisions about:
//! - Texture quality/resolution
//! - Model LOD selection
//! - Buffer allocation strategies
//!
//! Helps prevent out-of-memory errors and optimize memory usage.

use ash::vk;
use std::sync::atomic::{AtomicU64, Ordering};

/// Memory heap budget info.
#[derive(Debug, Clone, Default)]
pub struct HeapBudget {
    /// Total heap size in bytes.
    pub heap_size: u64,
    /// Current budget (recommended max usage) in bytes.
    pub budget: u64,
    /// Current usage in bytes.
    pub usage: u64,
    /// Whether this is device-local memory.
    pub is_device_local: bool,
    /// Whether this is host-visible memory.
    pub is_host_visible: bool,
}

impl HeapBudget {
    /// Get available memory (budget - usage).
    pub fn available(&self) -> u64 {
        self.budget.saturating_sub(self.usage)
    }

    /// Get usage percentage (0.0 - 1.0).
    pub fn usage_percent(&self) -> f32 {
        if self.budget == 0 {
            0.0
        } else {
            self.usage as f32 / self.budget as f32
        }
    }

    /// Check if heap is running low (>80% used).
    pub fn is_low(&self) -> bool {
        self.usage_percent() > 0.80
    }

    /// Check if heap is critically low (>95% used).
    pub fn is_critical(&self) -> bool {
        self.usage_percent() > 0.95
    }
}

/// Memory budget capabilities.
#[derive(Debug, Clone)]
pub struct MemoryBudgetCapabilities {
    /// Whether memory budget extension is supported.
    pub supported: bool,
    /// Number of memory heaps.
    pub heap_count: u32,
}

impl Default for MemoryBudgetCapabilities {
    fn default() -> Self {
        Self {
            supported: false,
            heap_count: 0,
        }
    }
}

/// Memory budget manager.
pub struct MemoryBudgetManager {
    /// Capabilities.
    capabilities: MemoryBudgetCapabilities,
    /// Per-heap budgets.
    heap_budgets: Vec<HeapBudget>,
    /// Application-tracked allocations per heap.
    tracked_allocations: Vec<AtomicU64>,
    /// Memory pressure callback threshold.
    pressure_threshold: f32,
    /// Last query time (for rate limiting).
    last_query_ms: u64,
}

impl MemoryBudgetManager {
    /// Query memory budget capabilities.
    pub fn query_capabilities(ctx: &super::context::VulkanContext) -> MemoryBudgetCapabilities {
        let extensions = unsafe {
            ctx.instance.enumerate_device_extension_properties(ctx.physical_device)
                .unwrap_or_default()
        };

        let supported = extensions.iter().any(|ext| {
            let name = unsafe { std::ffi::CStr::from_ptr(ext.extension_name.as_ptr()) };
            name.to_str().ok() == Some("VK_EXT_memory_budget")
        });

        let mem_props = unsafe {
            ctx.instance.get_physical_device_memory_properties(ctx.physical_device)
        };

        MemoryBudgetCapabilities {
            supported,
            heap_count: mem_props.memory_heap_count,
        }
    }

    /// Create a new memory budget manager.
    pub fn new(ctx: &super::context::VulkanContext) -> Self {
        let capabilities = Self::query_capabilities(ctx);

        let heap_count = capabilities.heap_count as usize;
        let mut heap_budgets = Vec::with_capacity(heap_count);
        let mut tracked_allocations = Vec::with_capacity(heap_count);

        let mem_props = unsafe {
            ctx.instance.get_physical_device_memory_properties(ctx.physical_device)
        };

        for i in 0..heap_count {
            let heap = mem_props.memory_heaps[i];
            heap_budgets.push(HeapBudget {
                heap_size: heap.size,
                budget: heap.size, // Default to full size if budget not supported
                usage: 0,
                is_device_local: heap.flags.contains(vk::MemoryHeapFlags::DEVICE_LOCAL),
                is_host_visible: false, // Would need to check memory types
            });
            tracked_allocations.push(AtomicU64::new(0));
        }

        Self {
            capabilities,
            heap_budgets,
            tracked_allocations,
            pressure_threshold: 0.80,
            last_query_ms: 0,
        }
    }

    /// Check if memory budget is supported.
    pub fn is_supported(&self) -> bool {
        self.capabilities.supported
    }

    /// Get capabilities.
    pub fn capabilities(&self) -> &MemoryBudgetCapabilities {
        &self.capabilities
    }

    /// Query current memory budgets from the driver.
    pub fn query_budgets(&mut self, ctx: &super::context::VulkanContext) {
        if !self.capabilities.supported {
            return;
        }

        let mut budget_props = vk::PhysicalDeviceMemoryBudgetPropertiesEXT::default();
        let mut mem_props2 = vk::PhysicalDeviceMemoryProperties2::default()
            .push_next(&mut budget_props);

        unsafe {
            ctx.instance.get_physical_device_memory_properties2(ctx.physical_device, &mut mem_props2);
        }

        for i in 0..self.capabilities.heap_count as usize {
            self.heap_budgets[i].budget = budget_props.heap_budget[i];
            self.heap_budgets[i].usage = budget_props.heap_usage[i];
        }
    }

    /// Get heap budget info.
    pub fn get_heap(&self, heap_index: usize) -> Option<&HeapBudget> {
        self.heap_budgets.get(heap_index)
    }

    /// Get all heap budgets.
    pub fn heaps(&self) -> &[HeapBudget] {
        &self.heap_budgets
    }

    /// Get total device-local memory budget.
    pub fn device_local_budget(&self) -> u64 {
        self.heap_budgets.iter()
            .filter(|h| h.is_device_local)
            .map(|h| h.budget)
            .sum()
    }

    /// Get total device-local memory usage.
    pub fn device_local_usage(&self) -> u64 {
        self.heap_budgets.iter()
            .filter(|h| h.is_device_local)
            .map(|h| h.usage)
            .sum()
    }

    /// Get available device-local memory.
    pub fn device_local_available(&self) -> u64 {
        self.device_local_budget().saturating_sub(self.device_local_usage())
    }

    /// Track an allocation.
    pub fn track_allocation(&self, heap_index: usize, size: u64) {
        if let Some(tracker) = self.tracked_allocations.get(heap_index) {
            tracker.fetch_add(size, Ordering::Relaxed);
        }
    }

    /// Track a deallocation.
    pub fn track_deallocation(&self, heap_index: usize, size: u64) {
        if let Some(tracker) = self.tracked_allocations.get(heap_index) {
            tracker.fetch_sub(size, Ordering::Relaxed);
        }
    }

    /// Get tracked allocations for a heap.
    pub fn tracked_for_heap(&self, heap_index: usize) -> u64 {
        self.tracked_allocations.get(heap_index)
            .map(|t| t.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    /// Check if any heap is under memory pressure.
    pub fn is_under_pressure(&self) -> bool {
        self.heap_budgets.iter().any(|h| h.usage_percent() > self.pressure_threshold)
    }

    /// Check if device-local memory is under pressure.
    pub fn is_device_local_under_pressure(&self) -> bool {
        let budget = self.device_local_budget();
        let usage = self.device_local_usage();
        if budget == 0 {
            return false;
        }
        (usage as f32 / budget as f32) > self.pressure_threshold
    }

    /// Set memory pressure threshold.
    pub fn set_pressure_threshold(&mut self, threshold: f32) {
        self.pressure_threshold = threshold.clamp(0.5, 0.99);
    }

    /// Get memory pressure level (0.0 = none, 1.0 = critical).
    pub fn pressure_level(&self) -> f32 {
        let budget = self.device_local_budget();
        let usage = self.device_local_usage();
        if budget == 0 {
            return 0.0;
        }
        let ratio = usage as f32 / budget as f32;
        ((ratio - self.pressure_threshold) / (1.0 - self.pressure_threshold)).clamp(0.0, 1.0)
    }

    /// Suggest a texture quality reduction factor based on memory pressure.
    /// Returns 1.0 for no reduction, lower values for more reduction.
    pub fn suggest_texture_scale(&self) -> f32 {
        let pressure = self.pressure_level();
        if pressure < 0.1 {
            1.0
        } else if pressure < 0.5 {
            0.75 // Half resolution textures
        } else if pressure < 0.8 {
            0.5 // Quarter resolution
        } else {
            0.25 // Eighth resolution
        }
    }

    /// Estimate if an allocation of given size is safe.
    pub fn can_allocate(&self, size: u64) -> bool {
        self.device_local_available() > size
    }

    /// Find the best heap for an allocation with given requirements.
    pub fn find_best_heap(&self, size: u64, require_device_local: bool) -> Option<usize> {
        self.heap_budgets.iter().enumerate()
            .filter(|(_, h)| {
                if require_device_local && !h.is_device_local {
                    return false;
                }
                h.available() >= size
            })
            .min_by_key(|(_, h)| h.usage_percent() as u32)
            .map(|(i, _)| i)
    }

    /// Get a summary string for debugging.
    pub fn summary(&self) -> String {
        let mut s = String::new();
        for (i, heap) in self.heap_budgets.iter().enumerate() {
            let heap_type = if heap.is_device_local { "VRAM" } else { "RAM" };
            s.push_str(&format!(
                "Heap {}: {} {:.1}MB / {:.1}MB ({:.1}%)\n",
                i,
                heap_type,
                heap.usage as f64 / (1024.0 * 1024.0),
                heap.budget as f64 / (1024.0 * 1024.0),
                heap.usage_percent() * 100.0
            ));
        }
        s
    }
}
