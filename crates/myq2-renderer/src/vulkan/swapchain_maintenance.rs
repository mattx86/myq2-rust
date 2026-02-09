//! Swapchain Maintenance (VK_EXT_swapchain_maintenance1)
//!
//! Enhanced swapchain lifecycle control:
//! - Query present scaling capabilities
//! - Release swapchain images without presenting
//! - Better fence management for present operations
//! - Present mode change without recreation

use ash::vk;

/// Swapchain maintenance capabilities.
#[derive(Debug, Clone, Default)]
pub struct SwapchainMaintenanceCapabilities {
    /// Whether swapchain maintenance is supported.
    pub supported: bool,
    /// Whether present fence is supported.
    pub present_fence: bool,
    /// Whether present mode change is supported.
    pub present_mode_change: bool,
}

/// Query swapchain maintenance capabilities.
pub fn query_capabilities(ctx: &super::context::VulkanContext) -> SwapchainMaintenanceCapabilities {
    let mut maint_features = vk::PhysicalDeviceSwapchainMaintenance1FeaturesEXT::default();
    let mut features2 = vk::PhysicalDeviceFeatures2::default()
        .push_next(&mut maint_features);

    unsafe {
        ctx.instance.get_physical_device_features2(ctx.physical_device, &mut features2);
    }

    let _ = features2;

    SwapchainMaintenanceCapabilities {
        supported: maint_features.swapchain_maintenance1 == vk::TRUE,
        present_fence: maint_features.swapchain_maintenance1 == vk::TRUE,
        present_mode_change: maint_features.swapchain_maintenance1 == vk::TRUE,
    }
}

/// Present scaling mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentScaling {
    /// No scaling - image must match surface size.
    None,
    /// Scale to fit, preserving aspect ratio.
    AspectRatioStretch,
    /// Stretch to fill entire surface.
    Stretch,
    /// One-to-one pixel mapping.
    OneToOne,
}

impl PresentScaling {
    /// Convert to Vulkan scaling flags.
    pub fn to_vk(&self) -> vk::PresentScalingFlagsEXT {
        match self {
            PresentScaling::None => vk::PresentScalingFlagsEXT::empty(),
            PresentScaling::AspectRatioStretch => vk::PresentScalingFlagsEXT::ASPECT_RATIO_STRETCH,
            PresentScaling::Stretch => vk::PresentScalingFlagsEXT::STRETCH,
            PresentScaling::OneToOne => vk::PresentScalingFlagsEXT::ONE_TO_ONE,
        }
    }
}

/// Present gravity (alignment when scaling).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentGravity {
    /// Top-left alignment.
    TopLeft,
    /// Top-center alignment.
    TopCenter,
    /// Top-right alignment.
    TopRight,
    /// Center-left alignment.
    CenterLeft,
    /// Center alignment.
    Center,
    /// Center-right alignment.
    CenterRight,
    /// Bottom-left alignment.
    BottomLeft,
    /// Bottom-center alignment.
    BottomCenter,
    /// Bottom-right alignment.
    BottomRight,
}

impl PresentGravity {
    /// Get X gravity flag.
    pub fn x_gravity(&self) -> vk::PresentGravityFlagsEXT {
        match self {
            PresentGravity::TopLeft | PresentGravity::CenterLeft | PresentGravity::BottomLeft => {
                vk::PresentGravityFlagsEXT::MIN
            }
            PresentGravity::TopCenter | PresentGravity::Center | PresentGravity::BottomCenter => {
                vk::PresentGravityFlagsEXT::CENTERED
            }
            PresentGravity::TopRight | PresentGravity::CenterRight | PresentGravity::BottomRight => {
                vk::PresentGravityFlagsEXT::MAX
            }
        }
    }

    /// Get Y gravity flag.
    pub fn y_gravity(&self) -> vk::PresentGravityFlagsEXT {
        match self {
            PresentGravity::TopLeft | PresentGravity::TopCenter | PresentGravity::TopRight => {
                vk::PresentGravityFlagsEXT::MIN
            }
            PresentGravity::CenterLeft | PresentGravity::Center | PresentGravity::CenterRight => {
                vk::PresentGravityFlagsEXT::CENTERED
            }
            PresentGravity::BottomLeft | PresentGravity::BottomCenter | PresentGravity::BottomRight => {
                vk::PresentGravityFlagsEXT::MAX
            }
        }
    }
}

/// Present mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentMode {
    /// No vsync, immediate presentation.
    Immediate,
    /// Single-buffered vsync (may block).
    Fifo,
    /// Triple-buffered vsync (relaxed).
    FifoRelaxed,
    /// Mailbox (latest frame only).
    Mailbox,
    /// Shared demand refresh.
    SharedDemandRefresh,
    /// Shared continuous refresh.
    SharedContinuousRefresh,
}

impl PresentMode {
    /// Convert to Vulkan present mode.
    pub fn to_vk(&self) -> vk::PresentModeKHR {
        match self {
            PresentMode::Immediate => vk::PresentModeKHR::IMMEDIATE,
            PresentMode::Fifo => vk::PresentModeKHR::FIFO,
            PresentMode::FifoRelaxed => vk::PresentModeKHR::FIFO_RELAXED,
            PresentMode::Mailbox => vk::PresentModeKHR::MAILBOX,
            PresentMode::SharedDemandRefresh => vk::PresentModeKHR::SHARED_DEMAND_REFRESH,
            PresentMode::SharedContinuousRefresh => vk::PresentModeKHR::SHARED_CONTINUOUS_REFRESH,
        }
    }

    /// Convert from Vulkan present mode.
    pub fn from_vk(mode: vk::PresentModeKHR) -> Self {
        match mode {
            vk::PresentModeKHR::IMMEDIATE => PresentMode::Immediate,
            vk::PresentModeKHR::MAILBOX => PresentMode::Mailbox,
            vk::PresentModeKHR::FIFO_RELAXED => PresentMode::FifoRelaxed,
            vk::PresentModeKHR::SHARED_DEMAND_REFRESH => PresentMode::SharedDemandRefresh,
            vk::PresentModeKHR::SHARED_CONTINUOUS_REFRESH => PresentMode::SharedContinuousRefresh,
            _ => PresentMode::Fifo,
        }
    }
}

/// Release swapchain images info.
#[derive(Debug, Clone)]
pub struct ReleaseSwapchainImagesInfo {
    /// Swapchain handle.
    pub swapchain: vk::SwapchainKHR,
    /// Image indices to release.
    pub image_indices: Vec<u32>,
}

/// Swapchain maintenance configuration.
#[derive(Debug, Clone)]
pub struct SwapchainMaintenanceConfig {
    /// Present scaling mode.
    pub scaling: PresentScaling,
    /// Present gravity.
    pub gravity: PresentGravity,
    /// Use present fence.
    pub use_present_fence: bool,
    /// Allow present mode changes.
    pub allow_mode_change: bool,
}

impl Default for SwapchainMaintenanceConfig {
    fn default() -> Self {
        Self {
            scaling: PresentScaling::Stretch,
            gravity: PresentGravity::Center,
            use_present_fence: true,
            allow_mode_change: true,
        }
    }
}

/// Swapchain maintenance manager.
pub struct SwapchainMaintenanceManager {
    capabilities: SwapchainMaintenanceCapabilities,
    config: SwapchainMaintenanceConfig,
    current_mode: PresentMode,
    pending_mode_change: Option<PresentMode>,
}

impl SwapchainMaintenanceManager {
    /// Create new manager.
    pub fn new(ctx: &super::context::VulkanContext) -> Self {
        let capabilities = query_capabilities(ctx);

        Self {
            capabilities,
            config: SwapchainMaintenanceConfig::default(),
            current_mode: PresentMode::Fifo,
            pending_mode_change: None,
        }
    }

    /// Check if maintenance is supported.
    pub fn is_supported(&self) -> bool {
        self.capabilities.supported
    }

    /// Set configuration.
    pub fn set_config(&mut self, config: SwapchainMaintenanceConfig) {
        self.config = config;
    }

    /// Get current configuration.
    pub fn config(&self) -> &SwapchainMaintenanceConfig {
        &self.config
    }

    /// Set current present mode.
    pub fn set_present_mode(&mut self, mode: PresentMode) {
        if self.capabilities.present_mode_change && self.config.allow_mode_change {
            self.pending_mode_change = Some(mode);
        } else {
            self.current_mode = mode;
        }
    }

    /// Get current present mode.
    pub fn present_mode(&self) -> PresentMode {
        self.current_mode
    }

    /// Check if there's a pending mode change.
    pub fn has_pending_mode_change(&self) -> bool {
        self.pending_mode_change.is_some()
    }

    /// Apply pending mode change.
    pub fn apply_mode_change(&mut self) {
        if let Some(mode) = self.pending_mode_change.take() {
            self.current_mode = mode;
        }
    }

    /// Check if present fence should be used.
    pub fn use_present_fence(&self) -> bool {
        self.capabilities.present_fence && self.config.use_present_fence
    }
}

/// Present fence tracking.
pub struct PresentFenceTracker {
    /// Fences for each frame in flight.
    fences: Vec<vk::Fence>,
    /// Current fence index.
    current_index: usize,
    /// Whether fences are in use.
    in_use: Vec<bool>,
}

impl PresentFenceTracker {
    /// Create new tracker.
    pub fn new(frames_in_flight: usize) -> Self {
        Self {
            fences: vec![vk::Fence::null(); frames_in_flight],
            current_index: 0,
            in_use: vec![false; frames_in_flight],
        }
    }

    /// Get current fence.
    pub fn current_fence(&self) -> vk::Fence {
        self.fences[self.current_index]
    }

    /// Set fence for current index.
    pub fn set_fence(&mut self, fence: vk::Fence) {
        self.fences[self.current_index] = fence;
        self.in_use[self.current_index] = true;
    }

    /// Advance to next frame.
    pub fn next_frame(&mut self) {
        self.current_index = (self.current_index + 1) % self.fences.len();
    }

    /// Check if current fence is in use.
    pub fn is_current_in_use(&self) -> bool {
        self.in_use[self.current_index]
    }

    /// Mark current fence as not in use.
    pub fn mark_complete(&mut self) {
        self.in_use[self.current_index] = false;
    }

    /// Get all fences that are in use.
    pub fn in_use_fences(&self) -> Vec<vk::Fence> {
        self.fences.iter()
            .zip(self.in_use.iter())
            .filter_map(|(&fence, &used)| {
                if used && fence != vk::Fence::null() {
                    Some(fence)
                } else {
                    None
                }
            })
            .collect()
    }
}
