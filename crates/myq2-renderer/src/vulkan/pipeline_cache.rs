//! Pipeline cache for faster shader compilation and startup
//!
//! Serializes compiled pipeline state to disk, allowing subsequent launches
//! to skip expensive shader compilation. This significantly reduces startup time.
//!
//! Cache location: <game_dir>/cache/pipeline_cache.bin

use ash::vk;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Pipeline cache file header for validation.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct CacheHeader {
    /// Magic number for identification.
    magic: u32,
    /// Cache version.
    version: u32,
    /// Vendor ID.
    vendor_id: u32,
    /// Device ID.
    device_id: u32,
    /// Driver version.
    driver_version: u32,
    /// Pipeline cache UUID.
    uuid: [u8; vk::UUID_SIZE],
}

const CACHE_MAGIC: u32 = 0x4D51_5043; // "MQPC" - MyQ2 Pipeline Cache
const CACHE_VERSION: u32 = 1;

/// Pipeline cache manager.
pub struct PipelineCacheManager {
    /// Vulkan pipeline cache handle.
    cache: vk::PipelineCache,
    /// Path to cache file.
    cache_path: PathBuf,
    /// Whether the cache was loaded from disk.
    loaded_from_disk: bool,
    /// Device properties for validation.
    vendor_id: u32,
    device_id: u32,
    driver_version: u32,
    pipeline_cache_uuid: [u8; vk::UUID_SIZE],
    /// Whether the cache has been modified.
    dirty: bool,
}

impl PipelineCacheManager {
    /// Create a new pipeline cache manager.
    pub fn new(ctx: &super::context::VulkanContext, cache_dir: &Path) -> Result<Self, String> {
        // Get device properties
        let props = unsafe {
            ctx.instance.get_physical_device_properties(ctx.physical_device)
        };

        let vendor_id = props.vendor_id;
        let device_id = props.device_id;
        let driver_version = props.driver_version;
        let pipeline_cache_uuid = props.pipeline_cache_uuid;

        // Ensure cache directory exists
        let cache_path = cache_dir.join("pipeline_cache.bin");
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create cache directory: {}", e))?;
        }

        // Try to load existing cache
        let (initial_data, loaded_from_disk) = Self::load_cache_data(
            &cache_path,
            vendor_id,
            device_id,
            driver_version,
            &pipeline_cache_uuid,
        );

        // Create pipeline cache
        let create_info = if let Some(ref data) = initial_data {
            vk::PipelineCacheCreateInfo::default()
                .initial_data(data)
        } else {
            vk::PipelineCacheCreateInfo::default()
        };

        let cache = unsafe {
            ctx.device.create_pipeline_cache(&create_info, None)
                .map_err(|e| format!("Failed to create pipeline cache: {:?}", e))?
        };

        if loaded_from_disk {
            println!("Loaded pipeline cache from disk ({} bytes)",
                     initial_data.as_ref().map_or(0, |d| d.len()));
        }

        Ok(Self {
            cache,
            cache_path,
            loaded_from_disk,
            vendor_id,
            device_id,
            driver_version,
            pipeline_cache_uuid,
            dirty: false,
        })
    }

    /// Load cache data from disk with validation.
    fn load_cache_data(
        path: &Path,
        vendor_id: u32,
        device_id: u32,
        driver_version: u32,
        uuid: &[u8; vk::UUID_SIZE],
    ) -> (Option<Vec<u8>>, bool) {
        let mut file = match File::open(path) {
            Ok(f) => f,
            Err(_) => return (None, false),
        };

        let mut data = Vec::new();
        if file.read_to_end(&mut data).is_err() {
            return (None, false);
        }

        // Validate header
        if data.len() < std::mem::size_of::<CacheHeader>() {
            return (None, false);
        }

        // Read Vulkan cache header (first 32 bytes of Vulkan pipeline cache data)
        // The Vulkan spec defines the pipeline cache header format
        if data.len() < 32 {
            return (None, false);
        }

        // Vulkan pipeline cache header:
        // u32 header_size
        // u32 header_version (VK_PIPELINE_CACHE_HEADER_VERSION_ONE = 1)
        // u32 vendor_id
        // u32 device_id
        // u8[UUID_SIZE] pipeline_cache_uuid

        let header_size = u32::from_ne_bytes([data[0], data[1], data[2], data[3]]);
        let header_version = u32::from_ne_bytes([data[4], data[5], data[6], data[7]]);
        let cache_vendor_id = u32::from_ne_bytes([data[8], data[9], data[10], data[11]]);
        let cache_device_id = u32::from_ne_bytes([data[12], data[13], data[14], data[15]]);

        // Validate
        if header_version != 1 {
            println!("Pipeline cache: invalid header version {}", header_version);
            return (None, false);
        }

        if cache_vendor_id != vendor_id || cache_device_id != device_id {
            println!("Pipeline cache: device mismatch (cache: {:04x}:{:04x}, current: {:04x}:{:04x})",
                     cache_vendor_id, cache_device_id, vendor_id, device_id);
            return (None, false);
        }

        // Validate UUID
        if data.len() >= 16 + vk::UUID_SIZE {
            let cache_uuid = &data[16..16 + vk::UUID_SIZE];
            if cache_uuid != uuid {
                println!("Pipeline cache: UUID mismatch");
                return (None, false);
            }
        }

        (Some(data), true)
    }

    /// Get the Vulkan pipeline cache handle.
    pub fn cache(&self) -> vk::PipelineCache {
        self.cache
    }

    /// Mark the cache as dirty (modified).
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Check if the cache was loaded from disk.
    pub fn was_loaded_from_disk(&self) -> bool {
        self.loaded_from_disk
    }

    /// Save the cache to disk.
    pub fn save(&self, ctx: &super::context::VulkanContext) -> Result<(), String> {
        let data = unsafe {
            ctx.device.get_pipeline_cache_data(self.cache)
                .map_err(|e| format!("Failed to get pipeline cache data: {:?}", e))?
        };

        if data.is_empty() {
            return Ok(());
        }

        let mut file = File::create(&self.cache_path)
            .map_err(|e| format!("Failed to create cache file: {}", e))?;

        file.write_all(&data)
            .map_err(|e| format!("Failed to write cache file: {}", e))?;

        println!("Saved pipeline cache to disk ({} bytes)", data.len());

        Ok(())
    }

    /// Merge another pipeline cache into this one.
    pub fn merge(&mut self, ctx: &super::context::VulkanContext, other: vk::PipelineCache) -> Result<(), String> {
        unsafe {
            ctx.device.merge_pipeline_caches(self.cache, &[other])
                .map_err(|e| format!("Failed to merge pipeline caches: {:?}", e))?;
        }
        self.dirty = true;
        Ok(())
    }

    /// Get cache statistics.
    pub fn stats(&self, ctx: &super::context::VulkanContext) -> CacheStats {
        let data_size = unsafe {
            ctx.device.get_pipeline_cache_data(self.cache)
                .map_or(0, |d| d.len())
        };

        CacheStats {
            data_size,
            loaded_from_disk: self.loaded_from_disk,
            dirty: self.dirty,
        }
    }

    /// Destroy the pipeline cache.
    pub fn destroy(&mut self, ctx: &super::context::VulkanContext) {
        // Save before destroying if dirty
        if self.dirty {
            let _ = self.save(ctx);
        }

        unsafe {
            ctx.device.destroy_pipeline_cache(self.cache, None);
        }
    }
}

/// Pipeline cache statistics.
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Size of cache data in bytes.
    pub data_size: usize,
    /// Whether cache was loaded from disk.
    pub loaded_from_disk: bool,
    /// Whether cache has been modified.
    pub dirty: bool,
}

/// Helper to auto-save cache on drop.
impl Drop for PipelineCacheManager {
    fn drop(&mut self) {
        // Note: Cannot save here as we don't have device reference
        // Caller must explicitly call save() before dropping
    }
}
