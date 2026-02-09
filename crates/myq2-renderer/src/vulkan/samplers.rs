//! Vulkan sampler management with anisotropic filtering support.
//!
//! Provides sampler creation and caching for texture filtering.

use ash::vk;
use std::collections::HashMap;

use super::VulkanContext;
use super::render_config;

/// Sampler filter mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SamplerFilter {
    /// Nearest neighbor filtering (pixelated)
    Nearest,
    /// Bilinear filtering
    Linear,
    /// Trilinear filtering (linear with mipmap interpolation)
    Trilinear,
}

/// Sampler address mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SamplerAddress {
    /// Repeat texture coordinates
    Repeat,
    /// Mirror and repeat
    MirroredRepeat,
    /// Clamp to edge
    ClampToEdge,
    /// Clamp to border color
    ClampToBorder,
}

/// Key for sampler cache lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SamplerKey {
    pub filter: SamplerFilter,
    pub address: SamplerAddress,
    pub with_anisotropy: bool,
    pub mip_levels: u32,
}

impl Default for SamplerKey {
    fn default() -> Self {
        Self {
            filter: SamplerFilter::Linear,
            address: SamplerAddress::Repeat,
            with_anisotropy: true,
            mip_levels: 1,
        }
    }
}

/// Manages Vulkan samplers with caching.
pub struct SamplerManager {
    samplers: HashMap<SamplerKey, vk::Sampler>,
    device: ash::Device,
}

impl SamplerManager {
    /// Create a new sampler manager.
    pub unsafe fn new(ctx: &VulkanContext) -> Self {
        Self {
            samplers: HashMap::new(),
            device: ctx.device.clone(),
        }
    }

    /// Get or create a sampler with the specified parameters.
    pub unsafe fn get_or_create(
        &mut self,
        ctx: &VulkanContext,
        key: SamplerKey,
    ) -> Result<vk::Sampler, String> {
        // Return cached sampler if available
        if let Some(&sampler) = self.samplers.get(&key) {
            return Ok(sampler);
        }

        // Create new sampler
        let sampler = self.create_sampler(ctx, &key)?;
        self.samplers.insert(key, sampler);
        Ok(sampler)
    }

    /// Create a sampler with the specified parameters.
    unsafe fn create_sampler(
        &self,
        ctx: &VulkanContext,
        key: &SamplerKey,
    ) -> Result<vk::Sampler, String> {
        // Determine filter modes
        let (mag_filter, min_filter, mipmap_mode) = match key.filter {
            SamplerFilter::Nearest => (
                vk::Filter::NEAREST,
                vk::Filter::NEAREST,
                vk::SamplerMipmapMode::NEAREST,
            ),
            SamplerFilter::Linear => (
                vk::Filter::LINEAR,
                vk::Filter::LINEAR,
                vk::SamplerMipmapMode::NEAREST,
            ),
            SamplerFilter::Trilinear => (
                vk::Filter::LINEAR,
                vk::Filter::LINEAR,
                vk::SamplerMipmapMode::LINEAR,
            ),
        };

        // Determine address mode
        let address_mode = match key.address {
            SamplerAddress::Repeat => vk::SamplerAddressMode::REPEAT,
            SamplerAddress::MirroredRepeat => vk::SamplerAddressMode::MIRRORED_REPEAT,
            SamplerAddress::ClampToEdge => vk::SamplerAddressMode::CLAMP_TO_EDGE,
            SamplerAddress::ClampToBorder => vk::SamplerAddressMode::CLAMP_TO_BORDER,
        };

        // Get anisotropy settings from render config
        let (anisotropy_enable, max_anisotropy) = if key.with_anisotropy && render_config::is_anisotropy_enabled() {
            (vk::TRUE, render_config::anisotropy_level())
        } else {
            (vk::FALSE, 1.0)
        };

        // Calculate max LOD based on mip levels
        let max_lod = if key.mip_levels > 1 {
            (key.mip_levels as f32).log2().floor()
        } else {
            0.0
        };

        let create_info = vk::SamplerCreateInfo::default()
            .mag_filter(mag_filter)
            .min_filter(min_filter)
            .mipmap_mode(mipmap_mode)
            .address_mode_u(address_mode)
            .address_mode_v(address_mode)
            .address_mode_w(address_mode)
            .mip_lod_bias(0.0)
            .anisotropy_enable(anisotropy_enable == vk::TRUE)
            .max_anisotropy(max_anisotropy)
            .compare_enable(false)
            .compare_op(vk::CompareOp::ALWAYS)
            .min_lod(0.0)
            .max_lod(max_lod.max(vk::LOD_CLAMP_NONE))
            .border_color(vk::BorderColor::INT_OPAQUE_BLACK)
            .unnormalized_coordinates(false);

        ctx.device
            .create_sampler(&create_info, None)
            .map_err(|e| format!("Failed to create sampler: {:?}", e))
    }

    /// Create the default texture sampler (trilinear + anisotropy).
    pub unsafe fn create_default_sampler(
        &mut self,
        ctx: &VulkanContext,
    ) -> Result<vk::Sampler, String> {
        self.get_or_create(ctx, SamplerKey {
            filter: SamplerFilter::Trilinear,
            address: SamplerAddress::Repeat,
            with_anisotropy: true,
            mip_levels: 12, // Enough for 4K textures
        })
    }

    /// Create a sampler for UI/HUD elements (linear, no mips, no anisotropy).
    pub unsafe fn create_ui_sampler(
        &mut self,
        ctx: &VulkanContext,
    ) -> Result<vk::Sampler, String> {
        self.get_or_create(ctx, SamplerKey {
            filter: SamplerFilter::Linear,
            address: SamplerAddress::ClampToEdge,
            with_anisotropy: false,
            mip_levels: 1,
        })
    }

    /// Create a sampler for lightmaps (bilinear, clamp).
    pub unsafe fn create_lightmap_sampler(
        &mut self,
        ctx: &VulkanContext,
    ) -> Result<vk::Sampler, String> {
        self.get_or_create(ctx, SamplerKey {
            filter: SamplerFilter::Linear,
            address: SamplerAddress::ClampToEdge,
            with_anisotropy: false,
            mip_levels: 1,
        })
    }

    /// Create a sampler for nearest-neighbor filtering (retro look).
    pub unsafe fn create_nearest_sampler(
        &mut self,
        ctx: &VulkanContext,
    ) -> Result<vk::Sampler, String> {
        self.get_or_create(ctx, SamplerKey {
            filter: SamplerFilter::Nearest,
            address: SamplerAddress::Repeat,
            with_anisotropy: false,
            mip_levels: 1,
        })
    }

    /// Destroy all samplers.
    pub unsafe fn destroy(&mut self) {
        for (_, sampler) in self.samplers.drain() {
            self.device.destroy_sampler(sampler, None);
        }
    }

    /// Get the number of cached samplers.
    pub fn sampler_count(&self) -> usize {
        self.samplers.len()
    }

    /// Invalidate all samplers (call when anisotropy setting changes).
    ///
    /// This destroys all cached samplers so they'll be recreated with
    /// the new anisotropy setting on next use.
    pub unsafe fn invalidate_all(&mut self) {
        for (_, sampler) in self.samplers.drain() {
            self.device.destroy_sampler(sampler, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // ============================================================
    // SamplerFilter enum
    // ============================================================

    #[test]
    fn test_sampler_filter_equality() {
        assert_eq!(SamplerFilter::Nearest, SamplerFilter::Nearest);
        assert_eq!(SamplerFilter::Linear, SamplerFilter::Linear);
        assert_eq!(SamplerFilter::Trilinear, SamplerFilter::Trilinear);
    }

    #[test]
    fn test_sampler_filter_inequality() {
        assert_ne!(SamplerFilter::Nearest, SamplerFilter::Linear);
        assert_ne!(SamplerFilter::Linear, SamplerFilter::Trilinear);
        assert_ne!(SamplerFilter::Nearest, SamplerFilter::Trilinear);
    }

    #[test]
    fn test_sampler_filter_clone_copy() {
        let f = SamplerFilter::Linear;
        let f2 = f; // Copy
        let f3 = f.clone(); // Clone
        assert_eq!(f, f2);
        assert_eq!(f, f3);
    }

    #[test]
    fn test_sampler_filter_debug() {
        // Ensure Debug is implemented and produces non-empty strings
        let s = format!("{:?}", SamplerFilter::Nearest);
        assert!(!s.is_empty());
        assert!(s.contains("Nearest"));
    }

    #[test]
    fn test_sampler_filter_hash_distinct() {
        let mut set = HashSet::new();
        set.insert(SamplerFilter::Nearest);
        set.insert(SamplerFilter::Linear);
        set.insert(SamplerFilter::Trilinear);
        assert_eq!(set.len(), 3);
    }

    // ============================================================
    // SamplerAddress enum
    // ============================================================

    #[test]
    fn test_sampler_address_equality() {
        assert_eq!(SamplerAddress::Repeat, SamplerAddress::Repeat);
        assert_eq!(SamplerAddress::MirroredRepeat, SamplerAddress::MirroredRepeat);
        assert_eq!(SamplerAddress::ClampToEdge, SamplerAddress::ClampToEdge);
        assert_eq!(SamplerAddress::ClampToBorder, SamplerAddress::ClampToBorder);
    }

    #[test]
    fn test_sampler_address_inequality() {
        assert_ne!(SamplerAddress::Repeat, SamplerAddress::MirroredRepeat);
        assert_ne!(SamplerAddress::ClampToEdge, SamplerAddress::ClampToBorder);
        assert_ne!(SamplerAddress::Repeat, SamplerAddress::ClampToEdge);
    }

    #[test]
    fn test_sampler_address_hash_distinct() {
        let mut set = HashSet::new();
        set.insert(SamplerAddress::Repeat);
        set.insert(SamplerAddress::MirroredRepeat);
        set.insert(SamplerAddress::ClampToEdge);
        set.insert(SamplerAddress::ClampToBorder);
        assert_eq!(set.len(), 4);
    }

    // ============================================================
    // SamplerKey
    // ============================================================

    #[test]
    fn test_sampler_key_default() {
        let key = SamplerKey::default();
        assert_eq!(key.filter, SamplerFilter::Linear);
        assert_eq!(key.address, SamplerAddress::Repeat);
        assert!(key.with_anisotropy);
        assert_eq!(key.mip_levels, 1);
    }

    #[test]
    fn test_sampler_key_equality_same() {
        let key1 = SamplerKey {
            filter: SamplerFilter::Trilinear,
            address: SamplerAddress::Repeat,
            with_anisotropy: true,
            mip_levels: 12,
        };
        let key2 = SamplerKey {
            filter: SamplerFilter::Trilinear,
            address: SamplerAddress::Repeat,
            with_anisotropy: true,
            mip_levels: 12,
        };
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_sampler_key_inequality_filter() {
        let key1 = SamplerKey {
            filter: SamplerFilter::Nearest,
            ..SamplerKey::default()
        };
        let key2 = SamplerKey {
            filter: SamplerFilter::Linear,
            ..SamplerKey::default()
        };
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_sampler_key_inequality_address() {
        let key1 = SamplerKey {
            address: SamplerAddress::Repeat,
            ..SamplerKey::default()
        };
        let key2 = SamplerKey {
            address: SamplerAddress::ClampToEdge,
            ..SamplerKey::default()
        };
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_sampler_key_inequality_anisotropy() {
        let key1 = SamplerKey {
            with_anisotropy: true,
            ..SamplerKey::default()
        };
        let key2 = SamplerKey {
            with_anisotropy: false,
            ..SamplerKey::default()
        };
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_sampler_key_inequality_mip_levels() {
        let key1 = SamplerKey {
            mip_levels: 1,
            ..SamplerKey::default()
        };
        let key2 = SamplerKey {
            mip_levels: 12,
            ..SamplerKey::default()
        };
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_sampler_key_hash_same_keys() {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;

        let key1 = SamplerKey::default();
        let key2 = SamplerKey::default();

        let mut hasher1 = DefaultHasher::new();
        key1.hash(&mut hasher1);
        let hash1 = hasher1.finish();

        let mut hasher2 = DefaultHasher::new();
        key2.hash(&mut hasher2);
        let hash2 = hasher2.finish();

        assert_eq!(hash1, hash2, "Equal keys must produce equal hashes");
    }

    #[test]
    fn test_sampler_key_hashmap_lookup() {
        let mut map = HashMap::new();
        let key = SamplerKey {
            filter: SamplerFilter::Trilinear,
            address: SamplerAddress::Repeat,
            with_anisotropy: true,
            mip_levels: 12,
        };
        map.insert(key, 42u32);

        // Look up with an identical key
        let lookup_key = SamplerKey {
            filter: SamplerFilter::Trilinear,
            address: SamplerAddress::Repeat,
            with_anisotropy: true,
            mip_levels: 12,
        };
        assert_eq!(map.get(&lookup_key), Some(&42));
    }

    #[test]
    fn test_sampler_key_hashmap_miss() {
        let mut map = HashMap::new();
        let key = SamplerKey {
            filter: SamplerFilter::Trilinear,
            address: SamplerAddress::Repeat,
            with_anisotropy: true,
            mip_levels: 12,
        };
        map.insert(key, 42u32);

        // Different key should miss
        let different_key = SamplerKey {
            filter: SamplerFilter::Nearest,
            address: SamplerAddress::Repeat,
            with_anisotropy: true,
            mip_levels: 12,
        };
        assert_eq!(map.get(&different_key), None);
    }

    #[test]
    fn test_sampler_key_all_combinations_distinct() {
        // Insert all meaningful combinations and check they're all distinct
        let mut set = HashSet::new();
        let filters = [SamplerFilter::Nearest, SamplerFilter::Linear, SamplerFilter::Trilinear];
        let addresses = [SamplerAddress::Repeat, SamplerAddress::ClampToEdge, SamplerAddress::ClampToBorder];

        for &filter in &filters {
            for &address in &addresses {
                for &aniso in &[true, false] {
                    let key = SamplerKey {
                        filter,
                        address,
                        with_anisotropy: aniso,
                        mip_levels: 1,
                    };
                    set.insert(key);
                }
            }
        }
        // 3 filters * 3 addresses * 2 aniso = 18 unique keys
        assert_eq!(set.len(), 18);
    }

    #[test]
    fn test_sampler_key_copy() {
        let key1 = SamplerKey {
            filter: SamplerFilter::Trilinear,
            address: SamplerAddress::Repeat,
            with_anisotropy: true,
            mip_levels: 8,
        };
        let key2 = key1; // Copy
        assert_eq!(key1, key2);
        assert_eq!(key2.mip_levels, 8);
    }
}
