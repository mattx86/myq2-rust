//! Ray tracing support via Vulkan KHR extensions.
//!
//! This module provides acceleration structure management (BLAS/TLAS),
//! shader binding tables, and ray tracing pipeline creation.

pub mod acceleration;
pub mod sbt;

pub use acceleration::{AccelerationStructureManager, BlasHandle, TlasHandle};
pub use sbt::ShaderBindingTable;
