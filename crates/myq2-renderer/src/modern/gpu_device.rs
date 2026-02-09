//! Vulkan GPU Device management
//!
//! Replaces SDL3 GPU abstraction with direct Vulkan via ash.
//! Provides thread-safe upload queue for batching GPU transfers.

use ash::vk;
use crossbeam::queue::SegQueue;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::vulkan::{VulkanContext, VulkanSurface, Swapchain, CommandManager};

// ============================================================================
// Vulkan buffer wrapper types (replaces SDL3 GPU types)
// ============================================================================

/// GPU buffer handle (Vulkan buffer + allocation)
#[derive(Clone)]
pub struct GpuBuffer {
    pub buffer: vk::Buffer,
    pub allocation_index: usize, // Index into allocator for freeing
    pub size: vk::DeviceSize,
}

/// Transfer buffer for CPU→GPU uploads
pub struct TransferBuffer {
    pub buffer: vk::Buffer,
    pub allocation_index: usize,
    pub size: vk::DeviceSize,
    pub mapped_ptr: Option<*mut u8>,
}

// SAFETY: TransferBuffer is accessed only from main thread
unsafe impl Send for TransferBuffer {}
unsafe impl Sync for TransferBuffer {}

/// Command buffer wrapper
pub struct CommandBuffer {
    pub handle: vk::CommandBuffer,
    pub fence: vk::Fence,
}

/// Fence wrapper
pub struct Fence {
    pub handle: vk::Fence,
}

/// Shader format enum (SPIR-V only for Vulkan)
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ShaderFormat {
    Spirv,
}

// ============================================================================
// Global device storage
// ============================================================================

/// The global Vulkan context. Set during initialization by myq2-sys.
///
/// SAFETY: The engine is single-threaded. All access happens on the main thread.
static mut VULKAN_CTX: Option<VulkanContext> = None;

/// The global Vulkan surface.
static mut VULKAN_SURFACE: Option<VulkanSurface> = None;

/// The global swapchain.
static mut VULKAN_SWAPCHAIN: Option<Swapchain> = None;

/// The global command manager.
static mut VULKAN_COMMANDS: Option<CommandManager> = None;

/// Initialize the Vulkan device. Called by myq2-sys during startup.
///
/// # Safety
/// Must be called from the main thread, before any rendering occurs.
pub unsafe fn init_device(ctx: VulkanContext) {
    VULKAN_CTX = Some(ctx);
}

/// Initialize the rendering surface.
///
/// # Safety
/// Must be called from the main thread, after device init, with valid window.
pub unsafe fn init_surface(surface: VulkanSurface) {
    VULKAN_SURFACE = Some(surface);
}

/// Initialize the swapchain.
///
/// # Safety
/// Must be called from the main thread, after surface init.
pub unsafe fn init_swapchain(swapchain: Swapchain) {
    VULKAN_SWAPCHAIN = Some(swapchain);
}

/// Initialize the command manager.
///
/// # Safety
/// Must be called from the main thread, after device init.
pub unsafe fn init_commands(commands: CommandManager) {
    VULKAN_COMMANDS = Some(commands);
}

/// Shut down and release the Vulkan device.
///
/// # Safety
/// Must be called from the main thread, after all rendering has stopped.
pub unsafe fn shutdown_device() {
    // Shut down in reverse order of initialization
    if let Some(commands) = VULKAN_COMMANDS.take() {
        drop(commands);
    }
    if let Some(swapchain) = VULKAN_SWAPCHAIN.take() {
        drop(swapchain);
    }
    if let Some(surface) = VULKAN_SURFACE.take() {
        drop(surface);
    }
    if let Some(ctx) = VULKAN_CTX.take() {
        drop(ctx);
    }
}

/// Access the Vulkan context immutably.
///
/// # Safety
/// Must be called from the main thread.
pub fn with_device<R>(f: impl FnOnce(&VulkanContext) -> R) -> Option<R> {
    // SAFETY: Single-threaded engine, all access from main thread.
    unsafe { VULKAN_CTX.as_ref().map(f) }
}

/// Access the Vulkan context mutably.
///
/// # Safety
/// Must be called from the main thread.
pub fn with_device_mut<R>(f: impl FnOnce(&mut VulkanContext) -> R) -> Option<R> {
    // SAFETY: Single-threaded engine, all access from main thread.
    unsafe { VULKAN_CTX.as_mut().map(f) }
}

/// Access the swapchain immutably.
pub fn with_swapchain<R>(f: impl FnOnce(&Swapchain) -> R) -> Option<R> {
    // SAFETY: Single-threaded engine, all access from main thread.
    unsafe { VULKAN_SWAPCHAIN.as_ref().map(f) }
}

/// Access the swapchain mutably.
pub fn with_swapchain_mut<R>(f: impl FnOnce(&mut Swapchain) -> R) -> Option<R> {
    // SAFETY: Single-threaded engine, all access from main thread.
    unsafe { VULKAN_SWAPCHAIN.as_mut().map(f) }
}

/// Access the command manager immutably.
pub fn with_commands<R>(f: impl FnOnce(&CommandManager) -> R) -> Option<R> {
    // SAFETY: Single-threaded engine, all access from main thread.
    unsafe { VULKAN_COMMANDS.as_ref().map(f) }
}

/// Access the command manager mutably.
pub fn with_commands_mut<R>(f: impl FnOnce(&mut CommandManager) -> R) -> Option<R> {
    // SAFETY: Single-threaded engine, all access from main thread.
    unsafe { VULKAN_COMMANDS.as_mut().map(f) }
}

/// Access both the context and swapchain together.
pub fn with_device_and_swapchain<R>(
    f: impl FnOnce(&VulkanContext, &mut Swapchain) -> R
) -> Option<R> {
    // SAFETY: Single-threaded engine, all access from main thread.
    unsafe {
        match (VULKAN_CTX.as_ref(), VULKAN_SWAPCHAIN.as_mut()) {
            (Some(ctx), Some(sc)) => Some(f(ctx, sc)),
            _ => None,
        }
    }
}

/// Access context, swapchain, and surface together (for swapchain recreation).
pub fn with_device_swapchain_surface<R>(
    f: impl FnOnce(&VulkanContext, &mut Swapchain, &VulkanSurface) -> R
) -> Option<R> {
    // SAFETY: Single-threaded engine, all access from main thread.
    unsafe {
        match (VULKAN_CTX.as_ref(), VULKAN_SWAPCHAIN.as_mut(), VULKAN_SURFACE.as_ref()) {
            (Some(ctx), Some(sc), Some(surface)) => Some(f(ctx, sc, surface)),
            _ => None,
        }
    }
}

/// Check if the GPU device is initialized.
pub fn is_initialized() -> bool {
    // SAFETY: Single-threaded engine.
    unsafe { VULKAN_CTX.is_some() }
}

/// Check if the swapchain is initialized.
pub fn is_swapchain_initialized() -> bool {
    // SAFETY: Single-threaded engine.
    unsafe { VULKAN_SWAPCHAIN.is_some() }
}

/// Supported shader format for the current GPU backend.
pub fn shader_format() -> ShaderFormat {
    ShaderFormat::Spirv
}

// ============================================================================
// Upload queue for batched GPU transfers
// ============================================================================

/// A pending GPU buffer upload operation.
///
/// Contains all data needed to perform a CPU → GPU buffer copy.
pub struct PendingUpload {
    /// Raw bytes to upload
    pub data: Vec<u8>,
    /// Target GPU buffer
    pub target_buffer: GpuBuffer,
    /// Byte offset in target buffer
    pub offset: u32,
    /// Size in bytes
    pub size: u32,
}

// SAFETY: PendingUpload contains a Vulkan buffer handle which is thread-safe
// for queueing purposes. The actual upload happens on the main thread.
unsafe impl Send for PendingUpload {}

/// Thread-safe queue for pending GPU uploads.
///
/// Multiple threads can push uploads concurrently; the main thread flushes
/// all pending uploads in a single batched command buffer for efficiency.
static UPLOAD_QUEUE: SegQueue<PendingUpload> = SegQueue::new();

/// Flag indicating if deferred upload mode is enabled.
/// When true, uploads are queued; when false, uploads happen immediately.
static DEFERRED_UPLOADS: AtomicBool = AtomicBool::new(false);

/// Enable deferred upload mode. Uploads will be queued instead of
/// submitted immediately, to be flushed later in a batch.
pub fn enable_deferred_uploads() {
    DEFERRED_UPLOADS.store(true, Ordering::SeqCst);
}

/// Disable deferred upload mode. Subsequent uploads will submit immediately.
pub fn disable_deferred_uploads() {
    DEFERRED_UPLOADS.store(false, Ordering::SeqCst);
}

/// Check if deferred upload mode is enabled.
pub fn is_deferred_upload_mode() -> bool {
    DEFERRED_UPLOADS.load(Ordering::SeqCst)
}

/// Queue a GPU upload for batched submission.
///
/// Thread-safe: can be called from any thread.
pub fn queue_upload(upload: PendingUpload) {
    UPLOAD_QUEUE.push(upload);
}

/// Minimum number of uploads before batching is worthwhile.
const BATCH_UPLOAD_THRESHOLD: usize = 4;

/// Flush all pending GPU uploads in a single command buffer.
///
/// This batches all queued uploads into one command buffer submission,
/// which is more efficient than individual submissions per upload.
///
/// Must be called from the main thread with GPU device access.
pub fn flush_uploads() -> Result<usize, String> {
    if UPLOAD_QUEUE.is_empty() {
        return Ok(0);
    }

    // Collect all pending uploads
    let mut uploads = Vec::new();
    while let Some(upload) = UPLOAD_QUEUE.pop() {
        uploads.push(upload);
    }

    if uploads.is_empty() {
        return Ok(0);
    }

    let count = uploads.len();

    // For very small batches, process immediately without batched staging
    if count < BATCH_UPLOAD_THRESHOLD {
        return flush_uploads_immediate(uploads);
    }

    // Calculate total staging buffer size needed
    let total_size: usize = uploads.iter().map(|u| u.data.len()).sum();

    // Get access to Vulkan resources
    let result = with_device(|ctx| {
        unsafe {
            // Allocate staging buffer
            let staging_buffer = match allocate_staging_buffer(ctx, total_size) {
                Ok(buf) => buf,
                Err(e) => return Err(e),
            };

            // Copy all upload data into staging buffer
            let mut offset = 0usize;
            let mut copy_regions = Vec::with_capacity(uploads.len());

            if let Some(ptr) = staging_buffer.mapped_ptr {
                for upload in &uploads {
                    // Copy data to staging buffer at current offset
                    std::ptr::copy_nonoverlapping(
                        upload.data.as_ptr(),
                        ptr.add(offset),
                        upload.data.len(),
                    );

                    // Record copy region for this upload
                    copy_regions.push(BufferCopyInfo {
                        src_offset: offset as vk::DeviceSize,
                        dst_buffer: upload.target_buffer.buffer,
                        dst_offset: upload.offset as vk::DeviceSize,
                        size: upload.size as vk::DeviceSize,
                    });

                    offset += upload.data.len();
                }
            }

            // Record and submit copy commands
            with_commands_mut(|commands| {
                let cmd = commands.begin_single_time()
                    .map_err(|e| format!("Failed to begin command buffer: {}", e))?;

                // Record all buffer-to-buffer copies
                for region in &copy_regions {
                    let copy_region = vk::BufferCopy::default()
                        .src_offset(region.src_offset)
                        .dst_offset(region.dst_offset)
                        .size(region.size);

                    ctx.device.cmd_copy_buffer(
                        cmd,
                        staging_buffer.buffer,
                        region.dst_buffer,
                        &[copy_region],
                    );
                }

                commands.end_single_time(ctx, cmd)
                    .map_err(|e| format!("Failed to submit uploads: {}", e))?;

                Ok(())
            }).unwrap_or_else(|| Err("Command manager not initialized".to_string()))?;

            // Free staging buffer
            free_staging_buffer(ctx, staging_buffer);

            Ok(())
        }
    });

    match result {
        Some(Ok(())) => Ok(count),
        Some(Err(e)) => Err(e),
        None => Err("GPU device not initialized".to_string()),
    }
}

/// Buffer copy operation info.
struct BufferCopyInfo {
    src_offset: vk::DeviceSize,
    dst_buffer: vk::Buffer,
    dst_offset: vk::DeviceSize,
    size: vk::DeviceSize,
}

/// Staging buffer for CPU→GPU transfers.
struct StagingBufferInternal {
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
    mapped_ptr: Option<*mut u8>,
    #[allow(dead_code)]
    size: vk::DeviceSize,
}

/// Allocate a staging buffer for the given size.
unsafe fn allocate_staging_buffer(ctx: &VulkanContext, size: usize) -> Result<StagingBufferInternal, String> {
    let buffer_info = vk::BufferCreateInfo::default()
        .size(size as vk::DeviceSize)
        .usage(vk::BufferUsageFlags::TRANSFER_SRC)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);

    let buffer = ctx.device.create_buffer(&buffer_info, None)
        .map_err(|e| format!("Failed to create staging buffer: {:?}", e))?;

    let mem_requirements = ctx.device.get_buffer_memory_requirements(buffer);

    // Find host-visible, host-coherent memory type
    let memory_properties = ctx.instance.get_physical_device_memory_properties(ctx.physical_device);
    let memory_type_index = find_memory_type(
        &memory_properties,
        mem_requirements.memory_type_bits,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    ).ok_or("No suitable memory type for staging buffer")?;

    let alloc_info = vk::MemoryAllocateInfo::default()
        .allocation_size(mem_requirements.size)
        .memory_type_index(memory_type_index);

    let memory = ctx.device.allocate_memory(&alloc_info, None)
        .map_err(|e| format!("Failed to allocate staging memory: {:?}", e))?;

    ctx.device.bind_buffer_memory(buffer, memory, 0)
        .map_err(|e| format!("Failed to bind staging memory: {:?}", e))?;

    // Map the memory
    let mapped_ptr = ctx.device.map_memory(memory, 0, size as vk::DeviceSize, vk::MemoryMapFlags::empty())
        .map_err(|e| format!("Failed to map staging memory: {:?}", e))?;

    Ok(StagingBufferInternal {
        buffer,
        memory,
        mapped_ptr: Some(mapped_ptr as *mut u8),
        size: size as vk::DeviceSize,
    })
}

/// Free a staging buffer.
unsafe fn free_staging_buffer(ctx: &VulkanContext, staging: StagingBufferInternal) {
    ctx.device.unmap_memory(staging.memory);
    ctx.device.destroy_buffer(staging.buffer, None);
    ctx.device.free_memory(staging.memory, None);
}

/// Find a memory type that matches the requirements.
fn find_memory_type(
    memory_properties: &vk::PhysicalDeviceMemoryProperties,
    type_filter: u32,
    properties: vk::MemoryPropertyFlags,
) -> Option<u32> {
    for i in 0..memory_properties.memory_type_count {
        if (type_filter & (1 << i)) != 0 &&
           memory_properties.memory_types[i as usize].property_flags.contains(properties) {
            return Some(i);
        }
    }
    None
}

/// Flush uploads immediately without batching (for small counts).
fn flush_uploads_immediate(uploads: Vec<PendingUpload>) -> Result<usize, String> {
    let count = uploads.len();

    for upload in uploads {
        let result = with_device(|ctx| {
            unsafe {
                // Create small staging buffer for this upload
                let staging = allocate_staging_buffer(ctx, upload.data.len())?;

                // Copy data to staging
                if let Some(ptr) = staging.mapped_ptr {
                    std::ptr::copy_nonoverlapping(
                        upload.data.as_ptr(),
                        ptr,
                        upload.data.len(),
                    );
                }

                // Record and submit copy
                with_commands_mut(|commands| {
                    let cmd = commands.begin_single_time()?;

                    let copy_region = vk::BufferCopy::default()
                        .src_offset(0)
                        .dst_offset(upload.offset as vk::DeviceSize)
                        .size(upload.size as vk::DeviceSize);

                    ctx.device.cmd_copy_buffer(
                        cmd,
                        staging.buffer,
                        upload.target_buffer.buffer,
                        &[copy_region],
                    );

                    commands.end_single_time(ctx, cmd)
                }).unwrap_or_else(|| Err("Command manager not initialized".to_string()))?;

                // Free staging
                free_staging_buffer(ctx, staging);

                Ok(())
            }
        });

        if let Some(Err(e)) = result {
            return Err(e);
        }
    }

    Ok(count)
}

/// Get the number of pending uploads in the queue.
pub fn pending_upload_count() -> usize {
    UPLOAD_QUEUE.len()
}

// ============================================================================
// Triple-buffered frame management
// ============================================================================

/// Resources associated with a single frame in flight.
///
/// Triple buffering allows the CPU to prepare frame N+1 while the GPU
/// is still rendering frame N, with frame N-1's resources available for reuse.
pub struct FrameResources {
    /// Command buffer for this frame (if acquired).
    pub command_buffer: Option<vk::CommandBuffer>,
    /// Fence to track GPU completion of this frame's work.
    pub fence: Option<vk::Fence>,
    /// Transfer buffers used this frame (recycled after GPU signals fence).
    pub transfer_buffer_indices: Vec<usize>,
    /// Frame index for debugging/ordering.
    pub frame_index: u64,
}

impl Default for FrameResources {
    fn default() -> Self {
        Self {
            command_buffer: None,
            fence: None,
            transfer_buffer_indices: Vec::new(),
            frame_index: 0,
        }
    }
}

/// Manages triple-buffered frame submission for async GPU work.
///
/// This allows the CPU to stay ahead of the GPU by up to 2 frames,
/// reducing stalls and improving throughput.
pub struct FrameManager {
    /// Ring buffer of frame resources (triple buffered).
    frames: [FrameResources; 3],
    /// Index of the current frame being prepared.
    current_frame: usize,
    /// Global frame counter (monotonically increasing).
    frame_counter: u64,
    /// Whether the frame manager has been initialized.
    initialized: bool,
}

impl FrameManager {
    /// Create a new uninitialized frame manager.
    pub const fn new() -> Self {
        Self {
            frames: [
                FrameResources {
                    command_buffer: None,
                    fence: None,
                    transfer_buffer_indices: Vec::new(),
                    frame_index: 0,
                },
                FrameResources {
                    command_buffer: None,
                    fence: None,
                    transfer_buffer_indices: Vec::new(),
                    frame_index: 0,
                },
                FrameResources {
                    command_buffer: None,
                    fence: None,
                    transfer_buffer_indices: Vec::new(),
                    frame_index: 0,
                },
            ],
            current_frame: 0,
            frame_counter: 0,
            initialized: false,
        }
    }

    /// Initialize the frame manager. Called once at startup.
    pub fn init(&mut self) {
        for frame in &mut self.frames {
            frame.command_buffer = None;
            frame.fence = None;
            frame.transfer_buffer_indices.clear();
            frame.frame_index = 0;
        }
        self.current_frame = 0;
        self.frame_counter = 0;
        self.initialized = true;
    }

    /// Shut down the frame manager, releasing all resources.
    pub fn shutdown(&mut self) {
        // Wait for all in-flight frames to complete
        with_device(|ctx| unsafe {
            for frame in &mut self.frames {
                if let Some(fence) = frame.fence {
                    let _ = ctx.device.wait_for_fences(&[fence], true, u64::MAX);
                }
                frame.command_buffer = None;
                frame.fence = None;
                frame.transfer_buffer_indices.clear();
            }
        });
        self.initialized = false;
    }

    /// Begin a new frame. Waits for the oldest frame to complete if needed.
    ///
    /// Returns a mutable reference to the frame resources for the new frame.
    pub fn begin_frame(&mut self) -> Option<&mut FrameResources> {
        if !self.initialized {
            return None;
        }

        let frame = &mut self.frames[self.current_frame];

        // Wait for this frame slot's previous GPU work to complete
        with_device(|ctx| unsafe {
            if let Some(fence) = frame.fence {
                let _ = ctx.device.wait_for_fences(&[fence], true, u64::MAX);
            }
        });

        // Reset frame resources for reuse
        frame.command_buffer = None;
        frame.fence = None;
        frame.transfer_buffer_indices.clear();
        frame.frame_index = self.frame_counter;

        Some(frame)
    }

    /// End the current frame and advance to the next frame slot.
    ///
    /// The command buffer should have been submitted before calling this.
    pub fn end_frame(&mut self) {
        if !self.initialized {
            return;
        }

        self.current_frame = (self.current_frame + 1) % 3;
        self.frame_counter += 1;
    }

    /// Get the current frame index (slot 0-2).
    pub fn current_frame_index(&self) -> usize {
        self.current_frame
    }

    /// Get the global frame counter.
    pub fn frame_counter(&self) -> u64 {
        self.frame_counter
    }

    /// Get immutable access to the current frame's resources.
    pub fn current_frame(&self) -> Option<&FrameResources> {
        if self.initialized {
            Some(&self.frames[self.current_frame])
        } else {
            None
        }
    }

    /// Get mutable access to the current frame's resources.
    pub fn current_frame_mut(&mut self) -> Option<&mut FrameResources> {
        if self.initialized {
            Some(&mut self.frames[self.current_frame])
        } else {
            None
        }
    }

    /// Add a transfer buffer index to the current frame for tracking.
    ///
    /// Transfer buffers are kept alive until the frame's GPU work completes.
    pub fn track_transfer_buffer(&mut self, buffer_index: usize) {
        if self.initialized {
            self.frames[self.current_frame].transfer_buffer_indices.push(buffer_index);
        }
    }

    /// Check if the frame manager is initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
}

impl Default for FrameManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global frame manager instance.
///
/// SAFETY: Single-threaded engine, all access from main thread.
static mut FRAME_MANAGER: FrameManager = FrameManager::new();

/// Initialize the global frame manager.
///
/// # Safety
/// Must be called from the main thread, after GPU device init.
pub unsafe fn init_frame_manager() {
    FRAME_MANAGER.init();
}

/// Shut down the global frame manager.
///
/// # Safety
/// Must be called from the main thread, before GPU device shutdown.
pub unsafe fn shutdown_frame_manager() {
    FRAME_MANAGER.shutdown();
}

/// Access the frame manager immutably.
pub fn with_frame_manager<R>(f: impl FnOnce(&FrameManager) -> R) -> R {
    // SAFETY: Single-threaded engine, all access from main thread.
    unsafe { f(&FRAME_MANAGER) }
}

/// Access the frame manager mutably.
pub fn with_frame_manager_mut<R>(f: impl FnOnce(&mut FrameManager) -> R) -> R {
    // SAFETY: Single-threaded engine, all access from main thread.
    unsafe { f(&mut FRAME_MANAGER) }
}

/// Begin a new frame (convenience wrapper).
pub fn begin_frame() -> bool {
    with_frame_manager_mut(|fm| fm.begin_frame().is_some())
}

/// End the current frame (convenience wrapper).
pub fn end_frame() {
    with_frame_manager_mut(|fm| fm.end_frame());
}

/// Get the current global frame counter.
pub fn current_frame_counter() -> u64 {
    with_frame_manager(|fm| fm.frame_counter())
}
