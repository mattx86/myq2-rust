//! Uniform Buffer Object (UBO) definitions (Vulkan)
//!
//! Grouped uniform data for efficient GPU updates. Uses Vulkan buffers
//! for uniform data storage.

use ash::vk;
use crate::modern::gpu_device;
use std::mem;

/// Per-frame uniform data (binding = 0).
///
/// Updated once per frame, contains view/projection matrices and global state.
#[repr(C, align(16))]
pub struct PerFrameUniforms {
    /// View matrix.
    pub view_matrix: [[f32; 4]; 4],
    /// Projection matrix.
    pub projection_matrix: [[f32; 4]; 4],
    /// Combined view-projection matrix.
    pub view_projection: [[f32; 4]; 4],
    /// Camera position in world space (r_origin).
    pub view_origin: [f32; 3],
    /// Current time (r_newrefdef.time).
    pub time: f32,
    /// Camera up vector (vup).
    pub view_up: [f32; 3],
    pub _pad1: f32,
    /// Camera right vector (vright).
    pub view_right: [f32; 3],
    pub _pad2: f32,
    /// Camera forward vector (vpn).
    pub view_forward: [f32; 3],
    /// Overbright bits (r_overbrightbits).
    pub overbright_bits: i32,
    /// Inverse intensity (vk_state.inverse_intensity).
    pub inverse_intensity: f32,
    /// Gamma value.
    pub gamma: f32,
    pub _pad3: [f32; 2],
}

impl Default for PerFrameUniforms {
    fn default() -> Self {
        Self {
            view_matrix: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
            projection_matrix: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
            view_projection: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
            view_origin: [0.0, 0.0, 0.0],
            time: 0.0,
            view_up: [0.0, 0.0, 1.0],
            _pad1: 0.0,
            view_right: [1.0, 0.0, 0.0],
            _pad2: 0.0,
            view_forward: [0.0, 1.0, 0.0],
            overbright_bits: 1,
            inverse_intensity: 1.0,
            gamma: 1.0,
            _pad3: [0.0, 0.0],
        }
    }
}

/// Per-object uniform data (binding = 1).
///
/// Updated for each entity being rendered.
#[repr(C, align(16))]
pub struct PerObjectUniforms {
    /// Model matrix.
    pub model_matrix: [[f32; 4]; 4],
    /// Combined model-view-projection matrix.
    pub mvp_matrix: [[f32; 4]; 4],
    /// Entity color tint.
    pub entity_color: [f32; 4],
    /// Entity alpha (RF_TRANSLUCENT).
    pub alpha: f32,
    /// Entity flags (RF_* packed).
    pub flags: u32,
    pub _pad: [f32; 2],
}

impl Default for PerObjectUniforms {
    fn default() -> Self {
        Self {
            model_matrix: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
            mvp_matrix: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
            entity_color: [1.0, 1.0, 1.0, 1.0],
            alpha: 1.0,
            flags: 0,
            _pad: [0.0, 0.0],
        }
    }
}

/// Manages a Uniform Buffer Object (Vulkan).
///
/// Uses host-visible, host-coherent memory for easy CPU updates.
pub struct UniformBuffer<T> {
    buffer: Option<vk::Buffer>,
    memory: Option<vk::DeviceMemory>,
    mapped_ptr: Option<*mut u8>,
    size: vk::DeviceSize,
    binding: u32,
    _marker: std::marker::PhantomData<T>,
}

// SAFETY: UniformBuffer is only used from the main thread
unsafe impl<T> Send for UniformBuffer<T> {}
unsafe impl<T> Sync for UniformBuffer<T> {}

impl<T> UniformBuffer<T> {
    /// Create a new UBO with the given binding point.
    pub fn new(binding: u32) -> Self {
        let size = mem::size_of::<T>() as vk::DeviceSize;

        let mut ubo = Self {
            buffer: None,
            memory: None,
            mapped_ptr: None,
            size,
            binding,
            _marker: std::marker::PhantomData,
        };

        ubo.create_buffer();
        ubo
    }

    fn create_buffer(&mut self) {
        gpu_device::with_device(|ctx| {
            // SAFETY: Vulkan context is valid and we're on the main thread.
            unsafe {
                // Create buffer
                let buffer_info = vk::BufferCreateInfo::default()
                    .size(self.size)
                    .usage(vk::BufferUsageFlags::UNIFORM_BUFFER)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE);

                let buffer = match ctx.device.create_buffer(&buffer_info, None) {
                    Ok(buf) => buf,
                    Err(_) => return,
                };

                // Find host-visible, host-coherent memory type
                let mem_requirements = ctx.device.get_buffer_memory_requirements(buffer);
                let mem_props = ctx.instance.get_physical_device_memory_properties(ctx.physical_device);

                let required_flags = vk::MemoryPropertyFlags::HOST_VISIBLE
                    | vk::MemoryPropertyFlags::HOST_COHERENT;

                let memory_type_index = (0..mem_props.memory_type_count)
                    .find(|&i| {
                        (mem_requirements.memory_type_bits & (1 << i)) != 0 &&
                        mem_props.memory_types[i as usize].property_flags.contains(required_flags)
                    });

                let memory_type_index = match memory_type_index {
                    Some(i) => i,
                    None => {
                        ctx.device.destroy_buffer(buffer, None);
                        return;
                    }
                };

                let alloc_info = vk::MemoryAllocateInfo::default()
                    .allocation_size(mem_requirements.size)
                    .memory_type_index(memory_type_index);

                let memory = match ctx.device.allocate_memory(&alloc_info, None) {
                    Ok(mem) => mem,
                    Err(_) => {
                        ctx.device.destroy_buffer(buffer, None);
                        return;
                    }
                };

                if ctx.device.bind_buffer_memory(buffer, memory, 0).is_err() {
                    ctx.device.free_memory(memory, None);
                    ctx.device.destroy_buffer(buffer, None);
                    return;
                }

                // Map memory persistently (host-coherent, no flush needed)
                let mapped = ctx.device.map_memory(
                    memory,
                    0,
                    self.size,
                    vk::MemoryMapFlags::empty(),
                );

                let mapped_ptr = match mapped {
                    Ok(ptr) => ptr as *mut u8,
                    Err(_) => {
                        ctx.device.free_memory(memory, None);
                        ctx.device.destroy_buffer(buffer, None);
                        return;
                    }
                };

                self.buffer = Some(buffer);
                self.memory = Some(memory);
                self.mapped_ptr = Some(mapped_ptr);
            }
        });
    }

    /// Update the entire buffer with new data.
    pub fn update(&self, data: &T) {
        if let Some(mapped_ptr) = self.mapped_ptr {
            // SAFETY: mapped_ptr is valid and points to properly aligned memory
            // of at least size_of::<T>() bytes. Host-coherent memory means
            // no explicit flush is needed.
            unsafe {
                std::ptr::copy_nonoverlapping(
                    data as *const T as *const u8,
                    mapped_ptr,
                    mem::size_of::<T>(),
                );
            }
        }
    }

    /// Get the binding point.
    pub fn binding(&self) -> u32 {
        self.binding
    }

    /// Compatibility stub. Returns 0.
    pub fn id(&self) -> u32 {
        0
    }

    /// Get the underlying Vulkan buffer handle.
    pub fn vk_buffer(&self) -> Option<vk::Buffer> {
        self.buffer
    }

    /// Destroy the buffer resources.
    fn destroy(&mut self) {
        gpu_device::with_device(|ctx| {
            // SAFETY: Vulkan context is valid and we're on the main thread.
            unsafe {
                if let Some(memory) = self.memory.take() {
                    // Unmap before freeing
                    ctx.device.unmap_memory(memory);
                    ctx.device.free_memory(memory, None);
                }
                if let Some(buffer) = self.buffer.take() {
                    ctx.device.destroy_buffer(buffer, None);
                }
            }
        });
        self.mapped_ptr = None;
    }
}

impl<T> Default for UniformBuffer<T> {
    fn default() -> Self {
        Self::new(0)
    }
}

impl<T> Drop for UniformBuffer<T> {
    fn drop(&mut self) {
        self.destroy();
    }
}

/// Uniforms for the postprocess (fullscreen quad) pass.
///
/// Layout matches `postprocess.frag.glsl` PushConstants (64 bytes):
///   vec4  u_PolyBlend          — 16 bytes
///   int   u_EnablePolyBlend    — 4 bytes
///   float u_Gamma              — 4 bytes
///   int   u_EnableGamma        — 4 bytes
///   float u_Saturation         — 4 bytes
///   float u_Contrast           — 4 bytes
///   float u_Brightness         — 4 bytes
///   float u_ShadowLift         — 4 bytes
///   int   u_LutEnabled         — 4 bytes
///   float u_LutIntensity       — 4 bytes
///
/// Uploaded via push constants (56 bytes, within the 128-byte minimum).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PostProcessUniforms {
    pub polyblend_color: [f32; 4],  // 16 bytes
    pub enable_polyblend: i32,      // 4 bytes
    pub gamma: f32,                 // 4 bytes
    pub enable_gamma: i32,          // 4 bytes
    pub saturation: f32,            // 4 bytes
    pub contrast: f32,              // 4 bytes
    pub brightness: f32,            // 4 bytes
    pub shadow_lift: f32,           // 4 bytes
    pub lut_enabled: i32,           // 4 bytes
    pub lut_intensity: f32,         // 4 bytes
    pub bloom_intensity: f32,       // 4 bytes — 0 = off (gated by r_bloom)
}                                   // 60 bytes total

impl Default for PostProcessUniforms {
    fn default() -> Self {
        Self {
            polyblend_color: [0.0; 4],
            enable_polyblend: 0,
            gamma: 1.0,
            enable_gamma: 0,
            saturation: 1.0,
            contrast: 1.0,
            brightness: 1.0,
            shadow_lift: 0.0,
            lut_enabled: 0,
            lut_intensity: 1.0,
            bloom_intensity: 0.0,
        }
    }
}

// ============================================================================
// Real-time point light list (set = 3, binding = 0 in world shader)
// ============================================================================

/// Extended per-light data for Doom 3 style stencil shadow rendering.

/// std430-compatible: four vec4 slots = 64 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct GpuLightD3 {
    /// World-space position.
    pub pos: [f32; 3],
    /// Attenuation radius.
    pub radius: f32,
    /// Normalized RGB color.
    pub color: [f32; 3],
    /// Normalized brightness (BSP intensity / 300).
    pub intensity: f32,
    /// Specular tint color (usually same as color).
    pub specular: [f32; 3],
    /// Blinn-Phong specular exponent.
    pub spec_pow: f32,
}

impl Default for GpuLightD3 {
    fn default() -> Self {
        Self {
            pos: [0.0; 3],
            radius: 300.0,
            color: [1.0; 3],
            intensity: 1.0,
            specular: [1.0; 3],
            spec_pow: 32.0,
        }
    }
}

impl GpuLightD3 {
    pub fn from_static_light(sl: &crate::vk_rsurf::StaticLight) -> Self {
        let max_c = sl.color[0].max(sl.color[1]).max(sl.color[2]);
        let color = if max_c > 2.0 {
            [sl.color[0] / 255.0, sl.color[1] / 255.0, sl.color[2] / 255.0]
        } else {
            sl.color
        };
        // Visual radius: Q2 intensity values (50–500) are BSP bake parameters, not physical
        // radii.  Scale to a pool-of-light size so each light covers its immediate area.
        let radius = (sl.intensity * 1.5).max(350.0_f32);
        // Intensity: the lightmap shadow mask (shadow = lm_lum²) reduces the average
        // effective brightness significantly — a typical lit surface with lm_lum=0.6
        // gives shadow=0.36, so an uncapped 0.20 light becomes only 0.072 effective.
        // Raise the cap to 0.50 so lit areas get good brightness while shadow zones
        // (lm_lum near 0 → shadow near 0) stay dark.  Screen blend prevents over-
        // saturation if many lights overlap in very bright areas.
        let intensity = (sl.intensity / 400.0).clamp(0.05, 0.50);
        Self {
            pos: sl.origin,
            radius,
            color,
            intensity,
            specular: color,
            spec_pow: 32.0,
        }
    }
}


