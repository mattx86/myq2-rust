//! Vertex, index, and vertex layout abstractions using Vulkan.
//!
//! Replaces the SDL3 GPU buffer implementation with direct Vulkan buffers.
//! Data is uploaded to the GPU via staging buffers and command buffers.

use std::mem;
use ash::vk;
use crate::modern::gpu_device;
use crate::vulkan::VulkanContext;

// ============================================================================
// Vertex Buffer
// ============================================================================

/// A GPU-resident vertex buffer.
pub struct VertexBuffer {
    buffer: Option<vk::Buffer>,
    size: u32,
}

impl VertexBuffer {
    /// Create a new empty vertex buffer (no GPU allocation yet).
    pub fn new() -> Self {
        Self {
            buffer: None,
            size: 0,
        }
    }

    /// Create a vertex buffer pre-loaded with data.
    pub fn with_data<T: Copy>(data: &[T], _usage: u32) -> Self {
        let mut vb = Self::new();
        vb.upload(data, _usage);
        vb
    }

    /// Upload data to the vertex buffer, replacing any existing content.
    pub fn upload<T: Copy>(&mut self, data: &[T], _usage: u32) {
        let byte_size = (data.len() * mem::size_of::<T>()) as u32;
        if byte_size == 0 {
            self.buffer = None;
            self.size = 0;
            return;
        }

        // Convert data to bytes
        let bytes = unsafe {
            std::slice::from_raw_parts(
                data.as_ptr() as *const u8,
                data.len() * mem::size_of::<T>(),
            )
        };

        // Create vertex buffer and upload via staging buffer
        self.buffer = gpu_device::with_device(|ctx| {
            unsafe {
                // Create GPU-local vertex buffer
                let buffer_info = vk::BufferCreateInfo::default()
                    .size(byte_size as vk::DeviceSize)
                    .usage(vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE);

                let buffer = match ctx.device.create_buffer(&buffer_info, None) {
                    Ok(buf) => buf,
                    Err(_) => return None,
                };

                let mem_requirements = ctx.device.get_buffer_memory_requirements(buffer);
                let memory_properties = ctx.instance.get_physical_device_memory_properties(ctx.physical_device);

                // Find GPU-local memory type
                let memory_type_index = (0..memory_properties.memory_type_count)
                    .find(|&i| {
                        (mem_requirements.memory_type_bits & (1 << i)) != 0 &&
                        memory_properties.memory_types[i as usize].property_flags.contains(
                            vk::MemoryPropertyFlags::DEVICE_LOCAL
                        )
                    });

                let memory_type_index = match memory_type_index {
                    Some(i) => i,
                    None => {
                        ctx.device.destroy_buffer(buffer, None);
                        return None;
                    }
                };

                let alloc_info = vk::MemoryAllocateInfo::default()
                    .allocation_size(mem_requirements.size)
                    .memory_type_index(memory_type_index);

                let buffer_memory = match ctx.device.allocate_memory(&alloc_info, None) {
                    Ok(mem) => mem,
                    Err(_) => {
                        ctx.device.destroy_buffer(buffer, None);
                        return None;
                    }
                };

                if ctx.device.bind_buffer_memory(buffer, buffer_memory, 0).is_err() {
                    ctx.device.free_memory(buffer_memory, None);
                    ctx.device.destroy_buffer(buffer, None);
                    return None;
                }

                // Upload via staging buffer
                if !upload_buffer_data(ctx, buffer, bytes) {
                    ctx.device.free_memory(buffer_memory, None);
                    ctx.device.destroy_buffer(buffer, None);
                    return None;
                }

                Some(buffer)
            }
        }).flatten();

        self.size = byte_size;
    }

    /// Update a region of the vertex buffer.
    pub fn update<T: Copy>(&self, offset: usize, data: &[T]) {
        let buffer = match self.buffer {
            Some(b) => b,
            None => return,
        };

        if data.is_empty() {
            return;
        }

        let bytes = unsafe {
            std::slice::from_raw_parts(
                data.as_ptr() as *const u8,
                data.len() * std::mem::size_of::<T>(),
            )
        };

        gpu_device::with_device(|ctx| {
            // SAFETY: Vulkan context is valid and we're on the main thread.
            unsafe {
                update_buffer_region(ctx, buffer, offset as vk::DeviceSize, bytes);
            }
        });
    }

    /// Bind this vertex buffer for rendering (no-op in Vulkan - handled via command buffer).
    pub fn bind(&self) {
        // Vulkan binding happens through command buffer
    }

    /// Unbind vertex buffer (no-op in Vulkan).
    pub fn unbind() {
        // No-op
    }

    /// Get the underlying Vulkan buffer handle.
    pub fn vk_buffer(&self) -> Option<vk::Buffer> {
        self.buffer
    }

    /// Get the size in bytes.
    pub fn size(&self) -> u32 {
        self.size
    }

    /// Check if buffer is allocated.
    pub fn is_valid(&self) -> bool {
        self.buffer.is_some()
    }
}

impl Default for VertexBuffer {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Index Buffer
// ============================================================================

/// A GPU-resident index buffer.
pub struct IndexBuffer {
    buffer: Option<vk::Buffer>,
    count: u32,
    index_type: vk::IndexType,
}

impl IndexBuffer {
    /// Create a new empty index buffer.
    pub fn new() -> Self {
        Self {
            buffer: None,
            count: 0,
            index_type: vk::IndexType::UINT16,
        }
    }

    /// Create an index buffer pre-loaded with u16 indices.
    pub fn with_data_u16(data: &[u16], _usage: u32) -> Self {
        let mut ib = Self::new();
        ib.upload_u16(data, _usage);
        ib
    }

    /// Create an index buffer pre-loaded with u32 indices.
    pub fn with_data_u32(data: &[u32], _usage: u32) -> Self {
        let mut ib = Self::new();
        ib.upload_u32(data, _usage);
        ib
    }

    /// Upload u16 index data.
    pub fn upload_u16(&mut self, data: &[u16], _usage: u32) {
        if data.is_empty() {
            self.buffer = None;
            self.count = 0;
            return;
        }

        let byte_size = (data.len() * mem::size_of::<u16>()) as u32;

        // Convert data to bytes
        let bytes = unsafe {
            std::slice::from_raw_parts(
                data.as_ptr() as *const u8,
                data.len() * mem::size_of::<u16>(),
            )
        };

        // Create index buffer and upload via staging buffer
        self.buffer = gpu_device::with_device(|ctx| {
            unsafe {
                // Create GPU-local index buffer
                let buffer_info = vk::BufferCreateInfo::default()
                    .size(byte_size as vk::DeviceSize)
                    .usage(vk::BufferUsageFlags::INDEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE);

                let buffer = match ctx.device.create_buffer(&buffer_info, None) {
                    Ok(buf) => buf,
                    Err(_) => return None,
                };

                let mem_requirements = ctx.device.get_buffer_memory_requirements(buffer);
                let memory_properties = ctx.instance.get_physical_device_memory_properties(ctx.physical_device);

                // Find GPU-local memory type
                let memory_type_index = (0..memory_properties.memory_type_count)
                    .find(|&i| {
                        (mem_requirements.memory_type_bits & (1 << i)) != 0 &&
                        memory_properties.memory_types[i as usize].property_flags.contains(
                            vk::MemoryPropertyFlags::DEVICE_LOCAL
                        )
                    });

                let memory_type_index = match memory_type_index {
                    Some(i) => i,
                    None => {
                        ctx.device.destroy_buffer(buffer, None);
                        return None;
                    }
                };

                let alloc_info = vk::MemoryAllocateInfo::default()
                    .allocation_size(mem_requirements.size)
                    .memory_type_index(memory_type_index);

                let buffer_memory = match ctx.device.allocate_memory(&alloc_info, None) {
                    Ok(mem) => mem,
                    Err(_) => {
                        ctx.device.destroy_buffer(buffer, None);
                        return None;
                    }
                };

                if ctx.device.bind_buffer_memory(buffer, buffer_memory, 0).is_err() {
                    ctx.device.free_memory(buffer_memory, None);
                    ctx.device.destroy_buffer(buffer, None);
                    return None;
                }

                // Upload via staging buffer
                if !upload_buffer_data(ctx, buffer, bytes) {
                    ctx.device.free_memory(buffer_memory, None);
                    ctx.device.destroy_buffer(buffer, None);
                    return None;
                }

                Some(buffer)
            }
        }).flatten();

        self.count = data.len() as u32;
        self.index_type = vk::IndexType::UINT16;
    }

    /// Upload u32 index data.
    pub fn upload_u32(&mut self, data: &[u32], _usage: u32) {
        if data.is_empty() {
            self.buffer = None;
            self.count = 0;
            return;
        }

        let byte_size = (data.len() * mem::size_of::<u32>()) as u32;

        // Convert data to bytes
        let bytes = unsafe {
            std::slice::from_raw_parts(
                data.as_ptr() as *const u8,
                data.len() * mem::size_of::<u32>(),
            )
        };

        // Create index buffer and upload via staging buffer
        self.buffer = gpu_device::with_device(|ctx| {
            unsafe {
                // Create GPU-local index buffer
                let buffer_info = vk::BufferCreateInfo::default()
                    .size(byte_size as vk::DeviceSize)
                    .usage(vk::BufferUsageFlags::INDEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE);

                let buffer = match ctx.device.create_buffer(&buffer_info, None) {
                    Ok(buf) => buf,
                    Err(_) => return None,
                };

                let mem_requirements = ctx.device.get_buffer_memory_requirements(buffer);
                let memory_properties = ctx.instance.get_physical_device_memory_properties(ctx.physical_device);

                // Find GPU-local memory type
                let memory_type_index = (0..memory_properties.memory_type_count)
                    .find(|&i| {
                        (mem_requirements.memory_type_bits & (1 << i)) != 0 &&
                        memory_properties.memory_types[i as usize].property_flags.contains(
                            vk::MemoryPropertyFlags::DEVICE_LOCAL
                        )
                    });

                let memory_type_index = match memory_type_index {
                    Some(i) => i,
                    None => {
                        ctx.device.destroy_buffer(buffer, None);
                        return None;
                    }
                };

                let alloc_info = vk::MemoryAllocateInfo::default()
                    .allocation_size(mem_requirements.size)
                    .memory_type_index(memory_type_index);

                let buffer_memory = match ctx.device.allocate_memory(&alloc_info, None) {
                    Ok(mem) => mem,
                    Err(_) => {
                        ctx.device.destroy_buffer(buffer, None);
                        return None;
                    }
                };

                if ctx.device.bind_buffer_memory(buffer, buffer_memory, 0).is_err() {
                    ctx.device.free_memory(buffer_memory, None);
                    ctx.device.destroy_buffer(buffer, None);
                    return None;
                }

                // Upload via staging buffer
                if !upload_buffer_data(ctx, buffer, bytes) {
                    ctx.device.free_memory(buffer_memory, None);
                    ctx.device.destroy_buffer(buffer, None);
                    return None;
                }

                Some(buffer)
            }
        }).flatten();

        self.count = data.len() as u32;
        self.index_type = vk::IndexType::UINT32;
    }

    /// Bind this index buffer (no-op in Vulkan - handled via command buffer).
    pub fn bind(&self) {
        // Vulkan binding happens through command buffer
    }

    /// Unbind index buffer (no-op in Vulkan).
    pub fn unbind() {
        // No-op
    }

    /// Get the index count.
    pub fn count(&self) -> u32 {
        self.count
    }

    /// Get the index type.
    pub fn index_type(&self) -> vk::IndexType {
        self.index_type
    }

    /// Get the underlying Vulkan buffer handle.
    pub fn vk_buffer(&self) -> Option<vk::Buffer> {
        self.buffer
    }

    /// Check if buffer is allocated.
    pub fn is_valid(&self) -> bool {
        self.buffer.is_some()
    }
}

impl Default for IndexBuffer {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Vertex Layout
// ============================================================================

/// Describes the layout of vertex attributes.
#[derive(Clone, Default)]
pub struct VertexLayout {
    attributes: Vec<VertexAttribute>,
    stride: u32,
}

/// A single vertex attribute.
#[derive(Clone)]
pub struct VertexAttribute {
    pub location: u32,
    pub format: vk::Format,
    pub offset: u32,
}

impl VertexLayout {
    /// Create a new empty vertex layout.
    pub fn new() -> Self {
        Self {
            attributes: Vec::new(),
            stride: 0,
        }
    }

    /// Add a float attribute.
    pub fn add_float(&mut self, location: u32, count: u32, offset: u32) {
        let format = match count {
            1 => vk::Format::R32_SFLOAT,
            2 => vk::Format::R32G32_SFLOAT,
            3 => vk::Format::R32G32B32_SFLOAT,
            4 => vk::Format::R32G32B32A32_SFLOAT,
            _ => vk::Format::R32_SFLOAT,
        };
        self.attributes.push(VertexAttribute {
            location,
            format,
            offset,
        });
    }

    /// Add an unsigned byte attribute (normalized).
    pub fn add_ubyte(&mut self, location: u32, count: u32, offset: u32) {
        let format = match count {
            1 => vk::Format::R8_UNORM,
            2 => vk::Format::R8G8_UNORM,
            3 => vk::Format::R8G8B8_UNORM,
            4 => vk::Format::R8G8B8A8_UNORM,
            _ => vk::Format::R8_UNORM,
        };
        self.attributes.push(VertexAttribute {
            location,
            format,
            offset,
        });
    }

    /// Set the vertex stride.
    pub fn set_stride(&mut self, stride: u32) {
        self.stride = stride;
    }

    /// Get the stride.
    pub fn stride(&self) -> u32 {
        self.stride
    }

    /// Get the attributes.
    pub fn attributes(&self) -> &[VertexAttribute] {
        &self.attributes
    }

    /// Apply the layout (no-op in Vulkan - handled at pipeline creation).
    pub fn apply(&self) {
        // Vulkan uses this at pipeline creation time, not draw time
    }
}

// ============================================================================
// Index Format (compatibility enum)
// ============================================================================

/// Index format enum for compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexFormat {
    U16,
    U32,
}

impl IndexFormat {
    /// Get the Vulkan index type.
    pub fn vk_type(&self) -> vk::IndexType {
        match self {
            IndexFormat::U16 => vk::IndexType::UINT16,
            IndexFormat::U32 => vk::IndexType::UINT32,
        }
    }
}

// ============================================================================
// Vertex Array (compatibility wrapper)
// ============================================================================

/// Vertex array object wrapper (Vulkan doesn't have VAOs - this is for compatibility).
pub struct VertexArray {
    /// Associated vertex buffer.
    pub vbo: Option<VertexBuffer>,
    /// Associated index buffer.
    pub ibo: Option<IndexBuffer>,
    /// Vertex layout.
    pub layout: VertexLayout,
}

impl VertexArray {
    /// Create a new vertex array.
    pub fn new() -> Self {
        Self {
            vbo: None,
            ibo: None,
            layout: VertexLayout::new(),
        }
    }

    /// Create with a vertex buffer.
    pub fn with_vbo(vbo: VertexBuffer) -> Self {
        Self {
            vbo: Some(vbo),
            ibo: None,
            layout: VertexLayout::new(),
        }
    }

    /// Bind the vertex array (no-op in Vulkan).
    pub fn bind(&self) {
        // Vulkan handles this through command buffer binding
    }

    /// Unbind the vertex array (no-op in Vulkan).
    pub fn unbind() {
        // No-op
    }

    /// Set a float attribute (compatibility method).
    /// In Vulkan, this stores the attribute in the layout for use at pipeline creation time.
    pub fn set_attribute_float(&mut self, location: u32, count: u32, stride: i32, offset: u32) {
        self.layout.add_float(location, count, offset);
        self.layout.set_stride(stride as u32);
    }

    /// Set an unsigned byte attribute (compatibility method).
    pub fn set_attribute_ubyte(&mut self, location: u32, count: u32, stride: i32, offset: u32) {
        self.layout.add_ubyte(location, count, offset);
        self.layout.set_stride(stride as u32);
    }

    /// Set an integer attribute (compatibility method).
    /// In Vulkan, this stores the attribute in the layout for use at pipeline creation time.
    pub fn set_attribute_int(&mut self, location: u32, count: u32, _normalized: u32, stride: i32, offset: u32) {
        // Use unsigned 32-bit int format
        let format = match count {
            1 => vk::Format::R32_UINT,
            2 => vk::Format::R32G32_UINT,
            3 => vk::Format::R32G32B32_UINT,
            4 => vk::Format::R32G32B32A32_UINT,
            _ => vk::Format::R32_UINT,
        };
        self.layout.attributes.push(VertexAttribute {
            location,
            format,
            offset,
        });
        self.layout.set_stride(stride as u32);
    }

    /// Check if valid.
    pub fn is_valid(&self) -> bool {
        self.vbo.is_some()
    }

    /// Get the vertex layout.
    pub fn layout(&self) -> &VertexLayout {
        &self.layout
    }
}

impl Default for VertexArray {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Staging Buffer Upload Helper
// ============================================================================

/// Upload data to a GPU buffer via a staging buffer.
///
/// Creates a temporary host-visible staging buffer, copies data to it,
/// then records a buffer copy command to transfer to the destination buffer.
///
/// # Safety
/// The destination buffer must have TRANSFER_DST usage flag and be large enough.
unsafe fn upload_buffer_data(ctx: &VulkanContext, dst_buffer: vk::Buffer, data: &[u8]) -> bool {
    if data.is_empty() {
        return true;
    }

    let size = data.len() as vk::DeviceSize;

    // Create staging buffer
    let staging_info = vk::BufferCreateInfo::default()
        .size(size)
        .usage(vk::BufferUsageFlags::TRANSFER_SRC)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);

    let staging_buffer = match ctx.device.create_buffer(&staging_info, None) {
        Ok(buf) => buf,
        Err(_) => return false,
    };

    let staging_requirements = ctx.device.get_buffer_memory_requirements(staging_buffer);
    let memory_properties = ctx.instance.get_physical_device_memory_properties(ctx.physical_device);

    // Find host-visible, host-coherent memory type
    let staging_memory_type = (0..memory_properties.memory_type_count).find(|&i| {
        (staging_requirements.memory_type_bits & (1 << i)) != 0
            && memory_properties.memory_types[i as usize]
                .property_flags
                .contains(vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT)
    });

    let staging_memory_type = match staging_memory_type {
        Some(i) => i,
        None => {
            ctx.device.destroy_buffer(staging_buffer, None);
            return false;
        }
    };

    let staging_alloc_info = vk::MemoryAllocateInfo::default()
        .allocation_size(staging_requirements.size)
        .memory_type_index(staging_memory_type);

    let staging_memory = match ctx.device.allocate_memory(&staging_alloc_info, None) {
        Ok(mem) => mem,
        Err(_) => {
            ctx.device.destroy_buffer(staging_buffer, None);
            return false;
        }
    };

    if ctx.device.bind_buffer_memory(staging_buffer, staging_memory, 0).is_err() {
        ctx.device.free_memory(staging_memory, None);
        ctx.device.destroy_buffer(staging_buffer, None);
        return false;
    }

    // Map and copy data to staging buffer
    let mapped_ptr = match ctx.device.map_memory(staging_memory, 0, size, vk::MemoryMapFlags::empty()) {
        Ok(ptr) => ptr,
        Err(_) => {
            ctx.device.free_memory(staging_memory, None);
            ctx.device.destroy_buffer(staging_buffer, None);
            return false;
        }
    };

    std::ptr::copy_nonoverlapping(data.as_ptr(), mapped_ptr as *mut u8, data.len());
    ctx.device.unmap_memory(staging_memory);

    // Record and submit copy command
    let success = gpu_device::with_commands_mut(|commands| {
        let cmd = match commands.begin_single_time() {
            Ok(c) => c,
            Err(_) => return false,
        };

        let copy_region = vk::BufferCopy::default()
            .src_offset(0)
            .dst_offset(0)
            .size(size);

        ctx.device.cmd_copy_buffer(cmd, staging_buffer, dst_buffer, &[copy_region]);

        commands.end_single_time(ctx, cmd).is_ok()
    }).unwrap_or(false);

    // Cleanup staging resources
    ctx.device.free_memory(staging_memory, None);
    ctx.device.destroy_buffer(staging_buffer, None);

    success
}

/// Upload data to a region of an existing buffer via staging buffer.
///
/// # Safety
/// - `ctx` must be a valid Vulkan context.
/// - `dst_buffer` must be a valid buffer with TRANSFER_DST usage.
/// - Offset + data length must not exceed buffer size.
unsafe fn update_buffer_region(
    ctx: &VulkanContext,
    dst_buffer: vk::Buffer,
    dst_offset: vk::DeviceSize,
    data: &[u8],
) {
    if data.is_empty() {
        return;
    }

    let size = data.len() as vk::DeviceSize;

    // Create staging buffer
    let staging_info = vk::BufferCreateInfo::default()
        .size(size)
        .usage(vk::BufferUsageFlags::TRANSFER_SRC)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);

    let staging_buffer = match ctx.device.create_buffer(&staging_info, None) {
        Ok(buf) => buf,
        Err(_) => return,
    };

    let staging_requirements = ctx.device.get_buffer_memory_requirements(staging_buffer);
    let memory_properties = ctx.instance.get_physical_device_memory_properties(ctx.physical_device);

    // Find host-visible, host-coherent memory type
    let staging_memory_type = (0..memory_properties.memory_type_count).find(|&i| {
        (staging_requirements.memory_type_bits & (1 << i)) != 0
            && memory_properties.memory_types[i as usize]
                .property_flags
                .contains(vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT)
    });

    let staging_memory_type = match staging_memory_type {
        Some(i) => i,
        None => {
            ctx.device.destroy_buffer(staging_buffer, None);
            return;
        }
    };

    let staging_alloc_info = vk::MemoryAllocateInfo::default()
        .allocation_size(staging_requirements.size)
        .memory_type_index(staging_memory_type);

    let staging_memory = match ctx.device.allocate_memory(&staging_alloc_info, None) {
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

    // Map and copy data to staging buffer
    let mapped_ptr = match ctx.device.map_memory(staging_memory, 0, size, vk::MemoryMapFlags::empty()) {
        Ok(ptr) => ptr,
        Err(_) => {
            ctx.device.free_memory(staging_memory, None);
            ctx.device.destroy_buffer(staging_buffer, None);
            return;
        }
    };

    std::ptr::copy_nonoverlapping(data.as_ptr(), mapped_ptr as *mut u8, data.len());
    ctx.device.unmap_memory(staging_memory);

    // Record and submit copy command
    gpu_device::with_commands_mut(|commands| {
        let cmd = match commands.begin_single_time() {
            Ok(c) => c,
            Err(_) => return,
        };

        let copy_region = vk::BufferCopy::default()
            .src_offset(0)
            .dst_offset(dst_offset)
            .size(size);

        ctx.device.cmd_copy_buffer(cmd, staging_buffer, dst_buffer, &[copy_region]);

        let _ = commands.end_single_time(ctx, cmd);
    });

    // Cleanup staging resources
    ctx.device.free_memory(staging_memory, None);
    ctx.device.destroy_buffer(staging_buffer, None);
}
