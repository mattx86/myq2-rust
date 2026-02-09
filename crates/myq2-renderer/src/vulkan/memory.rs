//! GPU memory management using gpu-allocator.

use ash::vk;
use gpu_allocator::vulkan::{Allocator, AllocatorCreateDesc, Allocation, AllocationCreateDesc, AllocationScheme};
use gpu_allocator::MemoryLocation;
use parking_lot::Mutex;
use std::sync::Arc;

use super::VulkanContext;

/// GPU buffer with associated memory.
pub struct Buffer {
    pub handle: vk::Buffer,
    pub allocation: Option<Allocation>,
    pub size: vk::DeviceSize,
    pub usage: vk::BufferUsageFlags,
    pub device_address: Option<vk::DeviceAddress>,
}

impl Buffer {
    /// Map the buffer memory for CPU access.
    ///
    /// # Safety
    /// Only valid for buffers created with CPU-visible memory.
    pub fn mapped_ptr(&self) -> Option<*mut u8> {
        self.allocation.as_ref().and_then(|a| a.mapped_ptr()).map(|p| p.as_ptr() as *mut u8)
    }

    /// Write data to the buffer.
    ///
    /// # Safety
    /// Buffer must be mappable and data must fit.
    pub unsafe fn write<T: Copy>(&self, data: &[T]) {
        if let Some(ptr) = self.mapped_ptr() {
            std::ptr::copy_nonoverlapping(
                data.as_ptr() as *const u8,
                ptr,
                std::mem::size_of_val(data),
            );
        }
    }
}

/// GPU image with associated memory.
pub struct Image {
    pub handle: vk::Image,
    pub view: vk::ImageView,
    pub allocation: Option<Allocation>,
    pub format: vk::Format,
    pub extent: vk::Extent3D,
    pub mip_levels: u32,
    pub array_layers: u32,
}

/// Memory manager wrapping gpu-allocator.
pub struct MemoryManager {
    allocator: Arc<Mutex<Allocator>>,
    device: ash::Device,
}

impl MemoryManager {
    /// Create a new memory manager.
    pub unsafe fn new(ctx: &VulkanContext) -> Result<Self, String> {
        let mut debug_settings = gpu_allocator::AllocatorDebugSettings::default();
        debug_settings.log_memory_information = cfg!(debug_assertions);
        debug_settings.log_leaks_on_shutdown = true;

        let allocator = Allocator::new(&AllocatorCreateDesc {
            instance: ctx.instance.clone(),
            device: ctx.device.clone(),
            physical_device: ctx.physical_device,
            debug_settings,
            buffer_device_address: ctx.rt_capabilities.supported,
            allocation_sizes: Default::default(),
        }).map_err(|e| format!("Failed to create allocator: {:?}", e))?;

        Ok(Self {
            allocator: Arc::new(Mutex::new(allocator)),
            device: ctx.device.clone(),
        })
    }

    /// Create a buffer with the specified usage and memory location.
    pub unsafe fn create_buffer(
        &self,
        size: vk::DeviceSize,
        usage: vk::BufferUsageFlags,
        location: MemoryLocation,
        name: &str,
    ) -> Result<Buffer, String> {
        let buffer_info = vk::BufferCreateInfo::default()
            .size(size)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let handle = self.device.create_buffer(&buffer_info, None)
            .map_err(|e| format!("Failed to create buffer: {:?}", e))?;

        let requirements = self.device.get_buffer_memory_requirements(handle);

        let allocation = self.allocator.lock()
            .allocate(&AllocationCreateDesc {
                name,
                requirements,
                location,
                linear: true,
                allocation_scheme: AllocationScheme::GpuAllocatorManaged,
            })
            .map_err(|e| format!("Failed to allocate buffer memory: {:?}", e))?;

        self.device.bind_buffer_memory(handle, allocation.memory(), allocation.offset())
            .map_err(|e| format!("Failed to bind buffer memory: {:?}", e))?;

        // Get device address if usage includes it
        let device_address = if usage.contains(vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS) {
            let addr_info = vk::BufferDeviceAddressInfo::default().buffer(handle);
            Some(self.device.get_buffer_device_address(&addr_info))
        } else {
            None
        };

        Ok(Buffer {
            handle,
            allocation: Some(allocation),
            size,
            usage,
            device_address,
        })
    }

    /// Create a staging buffer (CPU-visible, transfer source).
    pub unsafe fn create_staging_buffer(&self, size: vk::DeviceSize, name: &str) -> Result<Buffer, String> {
        self.create_buffer(
            size,
            vk::BufferUsageFlags::TRANSFER_SRC,
            MemoryLocation::CpuToGpu,
            name,
        )
    }

    /// Create a vertex buffer (GPU-only).
    pub unsafe fn create_vertex_buffer(&self, size: vk::DeviceSize, name: &str) -> Result<Buffer, String> {
        self.create_buffer(
            size,
            vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            MemoryLocation::GpuOnly,
            name,
        )
    }

    /// Create an index buffer (GPU-only).
    pub unsafe fn create_index_buffer(&self, size: vk::DeviceSize, name: &str) -> Result<Buffer, String> {
        self.create_buffer(
            size,
            vk::BufferUsageFlags::INDEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            MemoryLocation::GpuOnly,
            name,
        )
    }

    /// Create a uniform buffer (CPU-visible for frequent updates).
    pub unsafe fn create_uniform_buffer(&self, size: vk::DeviceSize, name: &str) -> Result<Buffer, String> {
        self.create_buffer(
            size,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            MemoryLocation::CpuToGpu,
            name,
        )
    }

    /// Create a storage buffer for ray tracing (GPU-only with device address).
    pub unsafe fn create_storage_buffer_rt(&self, size: vk::DeviceSize, name: &str) -> Result<Buffer, String> {
        self.create_buffer(
            size,
            vk::BufferUsageFlags::STORAGE_BUFFER |
            vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS |
            vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR,
            MemoryLocation::GpuOnly,
            name,
        )
    }

    /// Create an image with the specified format and usage.
    pub unsafe fn create_image(
        &self,
        extent: vk::Extent3D,
        format: vk::Format,
        usage: vk::ImageUsageFlags,
        mip_levels: u32,
        array_layers: u32,
        name: &str,
    ) -> Result<Image, String> {
        let image_type = if extent.depth > 1 {
            vk::ImageType::TYPE_3D
        } else if extent.height > 1 {
            vk::ImageType::TYPE_2D
        } else {
            vk::ImageType::TYPE_1D
        };

        let image_info = vk::ImageCreateInfo::default()
            .image_type(image_type)
            .format(format)
            .extent(extent)
            .mip_levels(mip_levels)
            .array_layers(array_layers)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED);

        let handle = self.device.create_image(&image_info, None)
            .map_err(|e| format!("Failed to create image: {:?}", e))?;

        let requirements = self.device.get_image_memory_requirements(handle);

        let allocation = self.allocator.lock()
            .allocate(&AllocationCreateDesc {
                name,
                requirements,
                location: MemoryLocation::GpuOnly,
                linear: false,
                allocation_scheme: AllocationScheme::GpuAllocatorManaged,
            })
            .map_err(|e| format!("Failed to allocate image memory: {:?}", e))?;

        self.device.bind_image_memory(handle, allocation.memory(), allocation.offset())
            .map_err(|e| format!("Failed to bind image memory: {:?}", e))?;

        // Create image view
        let view_type = if array_layers > 1 {
            vk::ImageViewType::TYPE_2D_ARRAY
        } else {
            match image_type {
                vk::ImageType::TYPE_1D => vk::ImageViewType::TYPE_1D,
                vk::ImageType::TYPE_2D => vk::ImageViewType::TYPE_2D,
                vk::ImageType::TYPE_3D => vk::ImageViewType::TYPE_3D,
                _ => vk::ImageViewType::TYPE_2D,
            }
        };

        let aspect_mask = if format == vk::Format::D32_SFLOAT ||
                           format == vk::Format::D24_UNORM_S8_UINT ||
                           format == vk::Format::D16_UNORM {
            vk::ImageAspectFlags::DEPTH
        } else {
            vk::ImageAspectFlags::COLOR
        };

        let view_info = vk::ImageViewCreateInfo::default()
            .image(handle)
            .view_type(view_type)
            .format(format)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask,
                base_mip_level: 0,
                level_count: mip_levels,
                base_array_layer: 0,
                layer_count: array_layers,
            });

        let view = self.device.create_image_view(&view_info, None)
            .map_err(|e| format!("Failed to create image view: {:?}", e))?;

        Ok(Image {
            handle,
            view,
            allocation: Some(allocation),
            format,
            extent,
            mip_levels,
            array_layers,
        })
    }

    /// Create a 2D texture.
    pub unsafe fn create_texture_2d(
        &self,
        width: u32,
        height: u32,
        format: vk::Format,
        mip_levels: u32,
        name: &str,
    ) -> Result<Image, String> {
        self.create_image(
            vk::Extent3D { width, height, depth: 1 },
            format,
            vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
            mip_levels,
            1,
            name,
        )
    }

    /// Create a render target (color attachment + sampled).
    pub unsafe fn create_render_target(
        &self,
        width: u32,
        height: u32,
        format: vk::Format,
        name: &str,
    ) -> Result<Image, String> {
        self.create_image(
            vk::Extent3D { width, height, depth: 1 },
            format,
            vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            1,
            1,
            name,
        )
    }

    /// Create a depth buffer.
    pub unsafe fn create_depth_buffer(
        &self,
        width: u32,
        height: u32,
        name: &str,
    ) -> Result<Image, String> {
        self.create_image(
            vk::Extent3D { width, height, depth: 1 },
            vk::Format::D32_SFLOAT,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            1,
            1,
            name,
        )
    }

    /// Destroy a buffer.
    pub unsafe fn destroy_buffer(&self, buffer: Buffer) {
        self.device.destroy_buffer(buffer.handle, None);
        if let Some(allocation) = buffer.allocation {
            let _ = self.allocator.lock().free(allocation);
        }
    }

    /// Destroy an image.
    pub unsafe fn destroy_image(&self, image: Image) {
        self.device.destroy_image_view(image.view, None);
        self.device.destroy_image(image.handle, None);
        if let Some(allocation) = image.allocation {
            let _ = self.allocator.lock().free(allocation);
        }
    }
}
