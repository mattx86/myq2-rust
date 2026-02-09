//! Vulkan surface creation from window handles.

use ash::vk;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};

use super::VulkanContext;

/// Wrapper around a Vulkan surface.
pub struct VulkanSurface {
    pub handle: vk::SurfaceKHR,
    pub format: vk::SurfaceFormatKHR,
    pub present_mode: vk::PresentModeKHR,
    pub capabilities: vk::SurfaceCapabilitiesKHR,
}

impl VulkanSurface {
    /// Create a new Vulkan surface from window handles.
    ///
    /// # Safety
    /// The window handles must be valid and outlive the surface.
    pub unsafe fn new(
        ctx: &VulkanContext,
        display_handle: RawDisplayHandle,
        window_handle: RawWindowHandle,
    ) -> Result<Self, String> {
        let handle = ash_window::create_surface(
            &ctx.entry,
            &ctx.instance,
            display_handle,
            window_handle,
            None,
        ).map_err(|e| format!("Failed to create Vulkan surface: {:?}", e))?;

        // Query surface capabilities
        let capabilities = ctx.surface_loader
            .get_physical_device_surface_capabilities(ctx.physical_device, handle)
            .map_err(|e| format!("Failed to get surface capabilities: {:?}", e))?;

        // Choose surface format (prefer SRGB)
        let formats = ctx.surface_loader
            .get_physical_device_surface_formats(ctx.physical_device, handle)
            .map_err(|e| format!("Failed to get surface formats: {:?}", e))?;

        let format = Self::choose_surface_format(&formats);

        // Choose present mode (prefer mailbox for low-latency, fifo for vsync)
        let present_modes = ctx.surface_loader
            .get_physical_device_surface_present_modes(ctx.physical_device, handle)
            .map_err(|e| format!("Failed to get present modes: {:?}", e))?;

        let present_mode = Self::choose_present_mode(&present_modes, true);

        Ok(Self {
            handle,
            format,
            present_mode,
            capabilities,
        })
    }

    /// Create a surface from a winit window.
    pub unsafe fn from_winit(
        ctx: &VulkanContext,
        window: &winit::window::Window,
    ) -> Result<Self, String> {
        let display_handle = window.display_handle()
            .map_err(|e| format!("Failed to get display handle: {:?}", e))?
            .as_raw();
        let window_handle = window.window_handle()
            .map_err(|e| format!("Failed to get window handle: {:?}", e))?
            .as_raw();

        Self::new(ctx, display_handle, window_handle)
    }

    /// Choose the best surface format.
    fn choose_surface_format(formats: &[vk::SurfaceFormatKHR]) -> vk::SurfaceFormatKHR {
        // Prefer SRGB B8G8R8A8
        for format in formats {
            if format.format == vk::Format::B8G8R8A8_SRGB &&
               format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR {
                return *format;
            }
        }

        // Fallback to UNORM B8G8R8A8
        for format in formats {
            if format.format == vk::Format::B8G8R8A8_UNORM {
                return *format;
            }
        }

        // Just use the first available
        formats.first().copied().unwrap_or(vk::SurfaceFormatKHR {
            format: vk::Format::B8G8R8A8_UNORM,
            color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
        })
    }

    /// Choose the best present mode.
    fn choose_present_mode(modes: &[vk::PresentModeKHR], vsync: bool) -> vk::PresentModeKHR {
        if vsync {
            // FIFO is guaranteed to be available
            return vk::PresentModeKHR::FIFO;
        }

        // Prefer mailbox (triple-buffering with low latency)
        if modes.contains(&vk::PresentModeKHR::MAILBOX) {
            return vk::PresentModeKHR::MAILBOX;
        }

        // Immediate (no vsync, may tear)
        if modes.contains(&vk::PresentModeKHR::IMMEDIATE) {
            return vk::PresentModeKHR::IMMEDIATE;
        }

        // Fallback to FIFO
        vk::PresentModeKHR::FIFO
    }

    /// Refresh surface capabilities (e.g., after window resize).
    pub unsafe fn refresh_capabilities(&mut self, ctx: &VulkanContext) -> Result<(), String> {
        self.capabilities = ctx.surface_loader
            .get_physical_device_surface_capabilities(ctx.physical_device, self.handle)
            .map_err(|e| format!("Failed to refresh surface capabilities: {:?}", e))?;
        Ok(())
    }

    /// Get the current extent, clamped to surface capabilities.
    pub fn get_extent(&self, desired_width: u32, desired_height: u32) -> vk::Extent2D {
        if self.capabilities.current_extent.width != u32::MAX {
            // The surface size is defined
            self.capabilities.current_extent
        } else {
            // Clamp to min/max
            vk::Extent2D {
                width: desired_width.clamp(
                    self.capabilities.min_image_extent.width,
                    self.capabilities.max_image_extent.width,
                ),
                height: desired_height.clamp(
                    self.capabilities.min_image_extent.height,
                    self.capabilities.max_image_extent.height,
                ),
            }
        }
    }

    /// Destroy the surface.
    pub unsafe fn destroy(&mut self, ctx: &VulkanContext) {
        ctx.surface_loader.destroy_surface(self.handle, None);
        self.handle = vk::SurfaceKHR::null();
    }
}
