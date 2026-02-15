//! Modern texture management
//!
//! Texture arrays for lightmaps, sampler objects, and the Vulkan texture store.

mod lightmap_array;
pub mod texture_store;

pub use lightmap_array::LightmapArray;
pub use texture_store::{
    init_texture_store, shutdown_texture_store, create_or_update_texture,
    get_descriptor_set, ensure_descriptor_set, create_white_texture, create_descriptor_for_view,
    create_post_init_descriptors,
};
