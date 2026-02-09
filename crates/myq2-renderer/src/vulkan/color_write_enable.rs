//! Color Write Enable (VK_EXT_color_write_enable)
//!
//! Dynamic per-attachment color write masking:
//! - Enable/disable color writes per attachment at runtime
//! - No pipeline recreation needed
//! - Useful for deferred rendering, g-buffer passes
//! - Performance optimization for selective output

use ash::vk;

/// Color write enable capabilities.
#[derive(Debug, Clone, Default)]
pub struct ColorWriteEnableCapabilities {
    /// Whether color write enable is supported.
    pub supported: bool,
}

/// Query color write enable capabilities.
pub fn query_capabilities(ctx: &super::context::VulkanContext) -> ColorWriteEnableCapabilities {
    let mut cwe_features = vk::PhysicalDeviceColorWriteEnableFeaturesEXT::default();
    let mut features2 = vk::PhysicalDeviceFeatures2::default()
        .push_next(&mut cwe_features);

    unsafe {
        ctx.instance.get_physical_device_features2(ctx.physical_device, &mut features2);
    }

    let _ = features2;

    ColorWriteEnableCapabilities {
        supported: cwe_features.color_write_enable == vk::TRUE,
    }
}

/// Color write mask for all channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorWriteMask {
    pub red: bool,
    pub green: bool,
    pub blue: bool,
    pub alpha: bool,
}

impl Default for ColorWriteMask {
    fn default() -> Self {
        Self::all()
    }
}

impl ColorWriteMask {
    /// All channels enabled.
    pub fn all() -> Self {
        Self {
            red: true,
            green: true,
            blue: true,
            alpha: true,
        }
    }

    /// No channels enabled.
    pub fn none() -> Self {
        Self {
            red: false,
            green: false,
            blue: false,
            alpha: false,
        }
    }

    /// RGB only (no alpha).
    pub fn rgb() -> Self {
        Self {
            red: true,
            green: true,
            blue: true,
            alpha: false,
        }
    }

    /// Alpha only.
    pub fn alpha_only() -> Self {
        Self {
            red: false,
            green: false,
            blue: false,
            alpha: true,
        }
    }

    /// Red channel only.
    pub fn red_only() -> Self {
        Self {
            red: true,
            green: false,
            blue: false,
            alpha: false,
        }
    }

    /// Convert to Vulkan color component flags.
    pub fn to_vk(&self) -> vk::ColorComponentFlags {
        let mut flags = vk::ColorComponentFlags::empty();

        if self.red {
            flags |= vk::ColorComponentFlags::R;
        }
        if self.green {
            flags |= vk::ColorComponentFlags::G;
        }
        if self.blue {
            flags |= vk::ColorComponentFlags::B;
        }
        if self.alpha {
            flags |= vk::ColorComponentFlags::A;
        }

        flags
    }

    /// Check if any channel is enabled.
    pub fn any(&self) -> bool {
        self.red || self.green || self.blue || self.alpha
    }

    /// Check if all channels are enabled.
    pub fn is_all(&self) -> bool {
        self.red && self.green && self.blue && self.alpha
    }
}

/// Per-attachment color write state.
#[derive(Debug, Clone)]
pub struct AttachmentColorWriteState {
    /// Write enables for each attachment.
    pub enables: Vec<bool>,
    /// Write masks for each attachment (static, set at pipeline creation).
    pub masks: Vec<ColorWriteMask>,
}

impl AttachmentColorWriteState {
    /// Create new state for number of attachments.
    pub fn new(count: usize) -> Self {
        Self {
            enables: vec![true; count],
            masks: vec![ColorWriteMask::all(); count],
        }
    }

    /// Create with all attachments disabled.
    pub fn all_disabled(count: usize) -> Self {
        Self {
            enables: vec![false; count],
            masks: vec![ColorWriteMask::all(); count],
        }
    }

    /// Enable/disable attachment.
    pub fn set_enable(&mut self, index: usize, enable: bool) {
        if index < self.enables.len() {
            self.enables[index] = enable;
        }
    }

    /// Enable all attachments.
    pub fn enable_all(&mut self) {
        for enable in &mut self.enables {
            *enable = true;
        }
    }

    /// Disable all attachments.
    pub fn disable_all(&mut self) {
        for enable in &mut self.enables {
            *enable = false;
        }
    }

    /// Set mask for attachment.
    pub fn set_mask(&mut self, index: usize, mask: ColorWriteMask) {
        if index < self.masks.len() {
            self.masks[index] = mask;
        }
    }

    /// Get enables as VkBool32 array for vkCmdSetColorWriteEnableEXT.
    pub fn get_vk_enables(&self) -> Vec<vk::Bool32> {
        self.enables.iter().map(|&e| if e { vk::TRUE } else { vk::FALSE }).collect()
    }
}

/// Common G-buffer write configurations.
pub mod gbuffer_configs {
    use super::*;

    /// Create state for geometry pass (all G-buffer outputs enabled).
    pub fn geometry_pass(attachment_count: usize) -> AttachmentColorWriteState {
        AttachmentColorWriteState::new(attachment_count)
    }

    /// Create state for lighting pass (only final output).
    pub fn lighting_pass(attachment_count: usize) -> AttachmentColorWriteState {
        let mut state = AttachmentColorWriteState::all_disabled(attachment_count);
        if attachment_count > 0 {
            state.set_enable(0, true);
        }
        state
    }

    /// Create state for depth-only pass (all color outputs disabled).
    pub fn depth_only(attachment_count: usize) -> AttachmentColorWriteState {
        AttachmentColorWriteState::all_disabled(attachment_count)
    }

    /// Create state for shadow pass (no color output).
    pub fn shadow_pass() -> AttachmentColorWriteState {
        AttachmentColorWriteState::all_disabled(0)
    }

    /// Create state for albedo-only pass.
    pub fn albedo_only(attachment_count: usize) -> AttachmentColorWriteState {
        let mut state = AttachmentColorWriteState::all_disabled(attachment_count);
        if attachment_count > 0 {
            state.set_enable(0, true); // Assuming albedo is attachment 0
        }
        state
    }

    /// Create state for normal-only pass.
    pub fn normal_only(attachment_count: usize, normal_index: usize) -> AttachmentColorWriteState {
        let mut state = AttachmentColorWriteState::all_disabled(attachment_count);
        state.set_enable(normal_index, true);
        state
    }
}

/// Color write enable manager.
pub struct ColorWriteEnableManager {
    capabilities: ColorWriteEnableCapabilities,
    current_state: AttachmentColorWriteState,
}

impl ColorWriteEnableManager {
    /// Create new manager.
    pub fn new(ctx: &super::context::VulkanContext, attachment_count: usize) -> Self {
        let capabilities = query_capabilities(ctx);

        Self {
            capabilities,
            current_state: AttachmentColorWriteState::new(attachment_count),
        }
    }

    /// Check if color write enable is supported.
    pub fn is_supported(&self) -> bool {
        self.capabilities.supported
    }

    /// Set state.
    pub fn set_state(&mut self, state: AttachmentColorWriteState) {
        self.current_state = state;
    }

    /// Get current state.
    pub fn state(&self) -> &AttachmentColorWriteState {
        &self.current_state
    }

    /// Get mutable state.
    pub fn state_mut(&mut self) -> &mut AttachmentColorWriteState {
        &mut self.current_state
    }

    /// Enable attachment.
    pub fn enable(&mut self, index: usize) {
        self.current_state.set_enable(index, true);
    }

    /// Disable attachment.
    pub fn disable(&mut self, index: usize) {
        self.current_state.set_enable(index, false);
    }

    /// Enable all attachments.
    pub fn enable_all(&mut self) {
        self.current_state.enable_all();
    }

    /// Disable all attachments.
    pub fn disable_all(&mut self) {
        self.current_state.disable_all();
    }

    /// Get enables for Vulkan command.
    pub fn get_vk_enables(&self) -> Vec<vk::Bool32> {
        self.current_state.get_vk_enables()
    }
}

/// Dynamic state for color write enable.
pub fn get_dynamic_state() -> vk::DynamicState {
    vk::DynamicState::COLOR_WRITE_ENABLE_EXT
}

/// Create pipeline with color write enable dynamic state.
pub fn create_dynamic_state_info() -> Vec<vk::DynamicState> {
    vec![
        vk::DynamicState::VIEWPORT,
        vk::DynamicState::SCISSOR,
        vk::DynamicState::COLOR_WRITE_ENABLE_EXT,
    ]
}
