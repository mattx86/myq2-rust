//! Acceleration structure management for ray tracing.

use ash::vk;
use ash::khr::acceleration_structure;
use std::collections::HashMap;

use crate::vulkan::{VulkanContext, MemoryManager, Buffer, CommandManager};

/// Handle to a bottom-level acceleration structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlasHandle(pub usize);

/// Handle to a top-level acceleration structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TlasHandle(pub usize);

/// Bottom-level acceleration structure (BLAS).
pub struct Blas {
    pub handle: vk::AccelerationStructureKHR,
    pub buffer: Buffer,
    pub device_address: vk::DeviceAddress,
    pub geometry_count: u32,
}

/// Top-level acceleration structure (TLAS).
pub struct Tlas {
    pub handle: vk::AccelerationStructureKHR,
    pub buffer: Buffer,
    pub device_address: vk::DeviceAddress,
    pub instance_count: u32,
}

/// Instance data for TLAS building.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RtInstance {
    pub transform: [[f32; 4]; 3],
    pub instance_custom_index_and_mask: u32,
    pub instance_shader_binding_table_record_offset_and_flags: u32,
    pub acceleration_structure_reference: u64,
}

impl Default for RtInstance {
    fn default() -> Self {
        Self {
            transform: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
            ],
            instance_custom_index_and_mask: 0xFF << 24,
            instance_shader_binding_table_record_offset_and_flags: 0,
            acceleration_structure_reference: 0,
        }
    }
}

impl RtInstance {
    /// Create a new instance with the given transform and BLAS reference.
    pub fn new(transform: [[f32; 4]; 3], custom_index: u32, blas_address: vk::DeviceAddress) -> Self {
        Self {
            transform,
            instance_custom_index_and_mask: (0xFF << 24) | (custom_index & 0xFFFFFF),
            instance_shader_binding_table_record_offset_and_flags: 0,
            acceleration_structure_reference: blas_address,
        }
    }

    /// Set the visibility mask.
    pub fn with_mask(mut self, mask: u8) -> Self {
        self.instance_custom_index_and_mask =
            ((mask as u32) << 24) | (self.instance_custom_index_and_mask & 0xFFFFFF);
        self
    }

    /// Set the SBT offset and flags.
    pub fn with_sbt_offset(mut self, offset: u32, flags: vk::GeometryInstanceFlagsKHR) -> Self {
        self.instance_shader_binding_table_record_offset_and_flags =
            ((flags.as_raw() as u32) << 24) | (offset & 0xFFFFFF);
        self
    }
}

/// Manages acceleration structures for ray tracing.
pub struct AccelerationStructureManager {
    loader: acceleration_structure::Device,
    blas_map: HashMap<BlasHandle, Blas>,
    tlas_map: HashMap<TlasHandle, Tlas>,
    scratch_buffer: Option<Buffer>,
    next_blas_id: usize,
    next_tlas_id: usize,
}

impl AccelerationStructureManager {
    /// Create a new acceleration structure manager.
    pub unsafe fn new(ctx: &VulkanContext) -> Result<Self, String> {
        let loader = ctx.accel_struct_loader.as_ref()
            .ok_or("Ray tracing not supported")?
            .clone();

        Ok(Self {
            loader,
            blas_map: HashMap::new(),
            tlas_map: HashMap::new(),
            scratch_buffer: None,
            next_blas_id: 0,
            next_tlas_id: 0,
        })
    }

    /// Build a BLAS from triangle geometry.
    ///
    /// # Arguments
    /// * `vertex_buffer` - Buffer containing vertex positions (vec3)
    /// * `vertex_stride` - Stride between vertices in bytes
    /// * `vertex_count` - Number of vertices
    /// * `index_buffer` - Buffer containing indices (u32)
    /// * `index_count` - Number of indices (must be multiple of 3)
    pub unsafe fn build_blas_triangles(
        &mut self,
        ctx: &VulkanContext,
        memory: &MemoryManager,
        commands: &CommandManager,
        vertex_buffer: vk::Buffer,
        vertex_address: vk::DeviceAddress,
        vertex_stride: vk::DeviceSize,
        vertex_count: u32,
        index_buffer: vk::Buffer,
        index_address: vk::DeviceAddress,
        index_count: u32,
    ) -> Result<BlasHandle, String> {
        let triangle_count = index_count / 3;

        let triangles = vk::AccelerationStructureGeometryTrianglesDataKHR::default()
            .vertex_format(vk::Format::R32G32B32_SFLOAT)
            .vertex_data(vk::DeviceOrHostAddressConstKHR { device_address: vertex_address })
            .vertex_stride(vertex_stride)
            .max_vertex(vertex_count)
            .index_type(vk::IndexType::UINT32)
            .index_data(vk::DeviceOrHostAddressConstKHR { device_address: index_address });

        let geometry = vk::AccelerationStructureGeometryKHR::default()
            .geometry_type(vk::GeometryTypeKHR::TRIANGLES)
            .geometry(vk::AccelerationStructureGeometryDataKHR { triangles })
            .flags(vk::GeometryFlagsKHR::OPAQUE);

        let geometries = [geometry];
        let build_info = vk::AccelerationStructureBuildGeometryInfoKHR::default()
            .ty(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL)
            .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE)
            .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
            .geometries(&geometries);

        let primitive_counts = [triangle_count];
        let mut size_info = vk::AccelerationStructureBuildSizesInfoKHR::default();
        self.loader.get_acceleration_structure_build_sizes(
            vk::AccelerationStructureBuildTypeKHR::DEVICE,
            &build_info,
            &primitive_counts,
            &mut size_info,
        );

        // Create acceleration structure buffer
        let as_buffer = memory.create_buffer(
            size_info.acceleration_structure_size,
            vk::BufferUsageFlags::ACCELERATION_STRUCTURE_STORAGE_KHR |
            vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            gpu_allocator::MemoryLocation::GpuOnly,
            "blas_buffer",
        )?;

        // Create acceleration structure
        let as_create_info = vk::AccelerationStructureCreateInfoKHR::default()
            .buffer(as_buffer.handle)
            .size(size_info.acceleration_structure_size)
            .ty(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL);

        let handle = self.loader.create_acceleration_structure(&as_create_info, None)
            .map_err(|e| format!("Failed to create BLAS: {:?}", e))?;

        // Get device address
        let addr_info = vk::AccelerationStructureDeviceAddressInfoKHR::default()
            .acceleration_structure(handle);
        let device_address = self.loader.get_acceleration_structure_device_address(&addr_info);

        // Ensure scratch buffer is large enough
        self.ensure_scratch_buffer(ctx, memory, size_info.build_scratch_size)?;

        // Build the BLAS
        let scratch_address = self.scratch_buffer.as_ref().unwrap().device_address.unwrap();

        let build_info = vk::AccelerationStructureBuildGeometryInfoKHR::default()
            .ty(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL)
            .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE)
            .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
            .dst_acceleration_structure(handle)
            .geometries(&geometries)
            .scratch_data(vk::DeviceOrHostAddressKHR { device_address: scratch_address });

        let build_range = vk::AccelerationStructureBuildRangeInfoKHR::default()
            .primitive_count(triangle_count)
            .primitive_offset(0)
            .first_vertex(0)
            .transform_offset(0);

        let build_ranges: &[vk::AccelerationStructureBuildRangeInfoKHR] = &[build_range];

        // Record and submit build command
        let cmd = commands.begin_single_time()?;
        self.loader.cmd_build_acceleration_structures(cmd, &[build_info], &[build_ranges]);
        commands.end_single_time(ctx, cmd)?;

        let blas_handle = BlasHandle(self.next_blas_id);
        self.next_blas_id += 1;

        self.blas_map.insert(blas_handle, Blas {
            handle,
            buffer: as_buffer,
            device_address,
            geometry_count: 1,
        });

        Ok(blas_handle)
    }

    /// Build or update a TLAS from instances.
    pub unsafe fn build_tlas(
        &mut self,
        ctx: &VulkanContext,
        memory: &MemoryManager,
        commands: &CommandManager,
        instances: &[RtInstance],
    ) -> Result<TlasHandle, String> {
        let instance_count = instances.len() as u32;

        // Create instance buffer
        let instance_data_size = std::mem::size_of_val(instances) as vk::DeviceSize;
        let instance_buffer = memory.create_buffer(
            instance_data_size,
            vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR |
            vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            gpu_allocator::MemoryLocation::CpuToGpu,
            "tlas_instances",
        )?;

        // Upload instance data
        instance_buffer.write(instances);

        let instance_address = instance_buffer.device_address.unwrap();

        let instances_data = vk::AccelerationStructureGeometryInstancesDataKHR::default()
            .array_of_pointers(false)
            .data(vk::DeviceOrHostAddressConstKHR { device_address: instance_address });

        let geometry = vk::AccelerationStructureGeometryKHR::default()
            .geometry_type(vk::GeometryTypeKHR::INSTANCES)
            .geometry(vk::AccelerationStructureGeometryDataKHR { instances: instances_data });

        let geometries = [geometry];
        let build_info = vk::AccelerationStructureBuildGeometryInfoKHR::default()
            .ty(vk::AccelerationStructureTypeKHR::TOP_LEVEL)
            .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_BUILD)
            .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
            .geometries(&geometries);

        let primitive_counts = [instance_count];
        let mut size_info = vk::AccelerationStructureBuildSizesInfoKHR::default();
        self.loader.get_acceleration_structure_build_sizes(
            vk::AccelerationStructureBuildTypeKHR::DEVICE,
            &build_info,
            &primitive_counts,
            &mut size_info,
        );

        // Create TLAS buffer
        let as_buffer = memory.create_buffer(
            size_info.acceleration_structure_size,
            vk::BufferUsageFlags::ACCELERATION_STRUCTURE_STORAGE_KHR |
            vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            gpu_allocator::MemoryLocation::GpuOnly,
            "tlas_buffer",
        )?;

        // Create TLAS
        let as_create_info = vk::AccelerationStructureCreateInfoKHR::default()
            .buffer(as_buffer.handle)
            .size(size_info.acceleration_structure_size)
            .ty(vk::AccelerationStructureTypeKHR::TOP_LEVEL);

        let handle = self.loader.create_acceleration_structure(&as_create_info, None)
            .map_err(|e| format!("Failed to create TLAS: {:?}", e))?;

        // Get device address
        let addr_info = vk::AccelerationStructureDeviceAddressInfoKHR::default()
            .acceleration_structure(handle);
        let device_address = self.loader.get_acceleration_structure_device_address(&addr_info);

        // Ensure scratch buffer
        self.ensure_scratch_buffer(ctx, memory, size_info.build_scratch_size)?;
        let scratch_address = self.scratch_buffer.as_ref().unwrap().device_address.unwrap();

        let build_info = vk::AccelerationStructureBuildGeometryInfoKHR::default()
            .ty(vk::AccelerationStructureTypeKHR::TOP_LEVEL)
            .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_BUILD)
            .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
            .dst_acceleration_structure(handle)
            .geometries(&geometries)
            .scratch_data(vk::DeviceOrHostAddressKHR { device_address: scratch_address });

        let build_range = vk::AccelerationStructureBuildRangeInfoKHR::default()
            .primitive_count(instance_count)
            .primitive_offset(0)
            .first_vertex(0)
            .transform_offset(0);

        let build_ranges: &[vk::AccelerationStructureBuildRangeInfoKHR] = &[build_range];

        // Build TLAS
        let cmd = commands.begin_single_time()?;
        self.loader.cmd_build_acceleration_structures(cmd, &[build_info], &[build_ranges]);
        commands.end_single_time(ctx, cmd)?;

        // Clean up instance buffer (temporary)
        memory.destroy_buffer(instance_buffer);

        let tlas_handle = TlasHandle(self.next_tlas_id);
        self.next_tlas_id += 1;

        self.tlas_map.insert(tlas_handle, Tlas {
            handle,
            buffer: as_buffer,
            device_address,
            instance_count,
        });

        Ok(tlas_handle)
    }

    /// Get a BLAS by handle.
    pub fn get_blas(&self, handle: BlasHandle) -> Option<&Blas> {
        self.blas_map.get(&handle)
    }

    /// Get a TLAS by handle.
    pub fn get_tlas(&self, handle: TlasHandle) -> Option<&Tlas> {
        self.tlas_map.get(&handle)
    }

    /// Ensure scratch buffer is at least the given size.
    unsafe fn ensure_scratch_buffer(
        &mut self,
        ctx: &VulkanContext,
        memory: &MemoryManager,
        min_size: vk::DeviceSize,
    ) -> Result<(), String> {
        let needs_resize = match &self.scratch_buffer {
            Some(buf) => buf.size < min_size,
            None => true,
        };

        if needs_resize {
            if let Some(old) = self.scratch_buffer.take() {
                memory.destroy_buffer(old);
            }

            // Round up to power of 2 for reuse
            let size = min_size.next_power_of_two().max(1024 * 1024);

            self.scratch_buffer = Some(memory.create_buffer(
                size,
                vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
                gpu_allocator::MemoryLocation::GpuOnly,
                "as_scratch",
            )?);
        }

        Ok(())
    }

    /// Destroy all acceleration structures.
    pub unsafe fn destroy(&mut self, memory: &MemoryManager) {
        for (_, blas) in self.blas_map.drain() {
            self.loader.destroy_acceleration_structure(blas.handle, None);
            memory.destroy_buffer(blas.buffer);
        }

        for (_, tlas) in self.tlas_map.drain() {
            self.loader.destroy_acceleration_structure(tlas.handle, None);
            memory.destroy_buffer(tlas.buffer);
        }

        if let Some(scratch) = self.scratch_buffer.take() {
            memory.destroy_buffer(scratch);
        }
    }
}
