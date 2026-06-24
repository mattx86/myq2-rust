//! Geometry management
//!
//! VBO/VAO abstractions and geometry batching for world, models, particles, and 2D.

mod vbo;
mod bsp;
mod alias;
mod particles;
mod draw2d;
pub mod shadow_volume;

pub use vbo::{VertexBuffer, IndexBuffer, IndexFormat, VertexArray};
pub use bsp::{BspGeometryManager, BspVertex, SurfaceDrawInfo, SURF_DRAWTURB, SURF_TRANS33, SURF_TRANS66, SURF_FLOWING};
pub use alias::{AliasModelManager, AliasModelBuffers, AliasInstance, InstancedAliasBatch, InstancedAliasRenderer};
pub use particles::ParticleManager;
pub use draw2d::{Draw2DManager, BlendMode};
pub use shadow_volume::{ShadowVolume, ShadowVolumeCache, build_shadow_volume};
