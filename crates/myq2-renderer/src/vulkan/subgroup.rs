//! Subgroup Operations for SIMD-style shader operations
//!
//! Vulkan subgroup operations enable efficient parallel operations within
//! a workgroup by exposing GPU SIMD capabilities:
//! - Broadcast values across invocations
//! - Reduction operations (sum, min, max, etc.)
//! - Ballot and vote operations
//! - Shuffle operations for data exchange
//!
//! Key for optimizing compute shaders and reducing memory traffic.

use ash::vk;

/// Subgroup capabilities.
#[derive(Debug, Clone, Default)]
pub struct SubgroupCapabilities {
    /// Subgroup size (typically 32 or 64).
    pub subgroup_size: u32,
    /// Supported shader stages.
    pub supported_stages: vk::ShaderStageFlags,
    /// Supported operations.
    pub supported_operations: vk::SubgroupFeatureFlags,
    /// Whether quad operations are supported in all stages.
    pub quad_operations_in_all_stages: bool,
    /// Minimum subgroup size (for subgroup size control).
    pub min_subgroup_size: u32,
    /// Maximum subgroup size.
    pub max_subgroup_size: u32,
    /// Whether full subgroups are required.
    pub full_subgroups: bool,
    /// Whether subgroup size control is supported.
    pub subgroup_size_control: bool,
}

/// Query subgroup capabilities.
pub fn query_capabilities(ctx: &super::context::VulkanContext) -> SubgroupCapabilities {
    // Get Vulkan 1.1 subgroup properties
    let mut subgroup_props = vk::PhysicalDeviceSubgroupProperties::default();
    let mut props2 = vk::PhysicalDeviceProperties2::default()
        .push_next(&mut subgroup_props);

    unsafe {
        ctx.instance.get_physical_device_properties2(ctx.physical_device, &mut props2);
    }

    // Get Vulkan 1.3 subgroup size control properties
    let mut size_control_props = vk::PhysicalDeviceSubgroupSizeControlProperties::default();
    let mut props2_size = vk::PhysicalDeviceProperties2::default()
        .push_next(&mut size_control_props);

    unsafe {
        ctx.instance.get_physical_device_properties2(ctx.physical_device, &mut props2_size);
    }

    // Get subgroup size control features
    let mut size_control_features = vk::PhysicalDeviceSubgroupSizeControlFeatures::default();
    let mut features2 = vk::PhysicalDeviceFeatures2::default()
        .push_next(&mut size_control_features);

    unsafe {
        ctx.instance.get_physical_device_features2(ctx.physical_device, &mut features2);
    }

    SubgroupCapabilities {
        subgroup_size: subgroup_props.subgroup_size,
        supported_stages: subgroup_props.supported_stages,
        supported_operations: subgroup_props.supported_operations,
        quad_operations_in_all_stages: subgroup_props.quad_operations_in_all_stages == vk::TRUE,
        min_subgroup_size: size_control_props.min_subgroup_size,
        max_subgroup_size: size_control_props.max_subgroup_size,
        full_subgroups: size_control_features.compute_full_subgroups == vk::TRUE,
        subgroup_size_control: size_control_features.subgroup_size_control == vk::TRUE,
    }
}

/// Subgroup operation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubgroupOperation {
    /// Basic operations (elect, barrier).
    Basic,
    /// Vote operations (all, any, equal).
    Vote,
    /// Arithmetic operations (add, mul, min, max).
    Arithmetic,
    /// Ballot operations.
    Ballot,
    /// Shuffle operations.
    Shuffle,
    /// Shuffle relative operations.
    ShuffleRelative,
    /// Clustered operations.
    Clustered,
    /// Quad operations.
    Quad,
}

impl SubgroupOperation {
    /// Convert to Vulkan feature flag.
    pub fn to_vk(&self) -> vk::SubgroupFeatureFlags {
        match self {
            SubgroupOperation::Basic => vk::SubgroupFeatureFlags::BASIC,
            SubgroupOperation::Vote => vk::SubgroupFeatureFlags::VOTE,
            SubgroupOperation::Arithmetic => vk::SubgroupFeatureFlags::ARITHMETIC,
            SubgroupOperation::Ballot => vk::SubgroupFeatureFlags::BALLOT,
            SubgroupOperation::Shuffle => vk::SubgroupFeatureFlags::SHUFFLE,
            SubgroupOperation::ShuffleRelative => vk::SubgroupFeatureFlags::SHUFFLE_RELATIVE,
            SubgroupOperation::Clustered => vk::SubgroupFeatureFlags::CLUSTERED,
            SubgroupOperation::Quad => vk::SubgroupFeatureFlags::QUAD,
        }
    }

    /// Check if this operation is supported.
    pub fn is_supported(&self, caps: &SubgroupCapabilities) -> bool {
        caps.supported_operations.contains(self.to_vk())
    }
}

/// GLSL code snippets for subgroup operations.
pub mod glsl {
    /// Extension for subgroup operations.
    pub const EXTENSION_BASIC: &str = "#extension GL_KHR_shader_subgroup_basic : require";
    pub const EXTENSION_VOTE: &str = "#extension GL_KHR_shader_subgroup_vote : require";
    pub const EXTENSION_ARITHMETIC: &str = "#extension GL_KHR_shader_subgroup_arithmetic : require";
    pub const EXTENSION_BALLOT: &str = "#extension GL_KHR_shader_subgroup_ballot : require";
    pub const EXTENSION_SHUFFLE: &str = "#extension GL_KHR_shader_subgroup_shuffle : require";
    pub const EXTENSION_QUAD: &str = "#extension GL_KHR_shader_subgroup_quad : require";

    /// Built-in variables.
    pub const SUBGROUP_SIZE: &str = "gl_SubgroupSize";
    pub const SUBGROUP_INVOCATION_ID: &str = "gl_SubgroupInvocationID";
    pub const SUBGROUP_ID: &str = "gl_SubgroupID";
    pub const NUM_SUBGROUPS: &str = "gl_NumSubgroups";

    /// Basic operations.
    pub const ELECT: &str = "subgroupElect()";
    pub const BARRIER: &str = "subgroupBarrier()";
    pub const MEMORY_BARRIER: &str = "subgroupMemoryBarrier()";

    /// Vote operations.
    pub const ALL: &str = "subgroupAll(value)";
    pub const ANY: &str = "subgroupAny(value)";
    pub const ALL_EQUAL: &str = "subgroupAllEqual(value)";

    /// Arithmetic reductions.
    pub const ADD: &str = "subgroupAdd(value)";
    pub const MUL: &str = "subgroupMul(value)";
    pub const MIN: &str = "subgroupMin(value)";
    pub const MAX: &str = "subgroupMax(value)";
    pub const AND: &str = "subgroupAnd(value)";
    pub const OR: &str = "subgroupOr(value)";
    pub const XOR: &str = "subgroupXor(value)";

    /// Inclusive/exclusive scans.
    pub const INCLUSIVE_ADD: &str = "subgroupInclusiveAdd(value)";
    pub const EXCLUSIVE_ADD: &str = "subgroupExclusiveAdd(value)";
    pub const INCLUSIVE_MUL: &str = "subgroupInclusiveMul(value)";
    pub const EXCLUSIVE_MUL: &str = "subgroupExclusiveMul(value)";

    /// Ballot operations.
    pub const BALLOT: &str = "subgroupBallot(value)";
    pub const BALLOT_BIT_COUNT: &str = "subgroupBallotBitCount(value)";
    pub const BALLOT_BIT_EXTRACT: &str = "subgroupBallotBitExtract(value, index)";
    pub const BALLOT_FIND_LSB: &str = "subgroupBallotFindLSB(value)";
    pub const BALLOT_FIND_MSB: &str = "subgroupBallotFindMSB(value)";

    /// Shuffle operations.
    pub const SHUFFLE: &str = "subgroupShuffle(value, id)";
    pub const SHUFFLE_XOR: &str = "subgroupShuffleXor(value, mask)";
    pub const SHUFFLE_UP: &str = "subgroupShuffleUp(value, delta)";
    pub const SHUFFLE_DOWN: &str = "subgroupShuffleDown(value, delta)";

    /// Broadcast operations.
    pub const BROADCAST: &str = "subgroupBroadcast(value, id)";
    pub const BROADCAST_FIRST: &str = "subgroupBroadcastFirst(value)";

    /// Quad operations.
    pub const QUAD_BROADCAST: &str = "subgroupQuadBroadcast(value, id)";
    pub const QUAD_SWAP_HORIZONTAL: &str = "subgroupQuadSwapHorizontal(value)";
    pub const QUAD_SWAP_VERTICAL: &str = "subgroupQuadSwapVertical(value)";
    pub const QUAD_SWAP_DIAGONAL: &str = "subgroupQuadSwapDiagonal(value)";

    /// Example: parallel reduction using subgroups.
    pub const PARALLEL_REDUCTION_EXAMPLE: &str = r#"
// Parallel sum reduction within a subgroup
float parallelSum(float value) {
    // Reduce within subgroup
    float sum = subgroupAdd(value);

    // If first invocation in subgroup, we have the partial sum
    if (subgroupElect()) {
        // Store to shared memory or atomic add to global
        return sum;
    }
    return 0.0;
}
"#;

    /// Example: prefix sum (exclusive scan).
    pub const PREFIX_SUM_EXAMPLE: &str = r#"
// Exclusive prefix sum within a subgroup
uint prefixSum(uint value) {
    return subgroupExclusiveAdd(value);
}

// Inclusive prefix sum within a subgroup
uint inclusivePrefixSum(uint value) {
    return subgroupInclusiveAdd(value);
}
"#;

    /// Example: compact operation using ballot.
    pub const COMPACT_EXAMPLE: &str = r#"
// Compact active elements to contiguous indices
uint compactIndex(bool active) {
    uvec4 ballot = subgroupBallot(active);
    uint activeMask = ballot.x; // For subgroups <= 32

    // Count active invocations before this one
    uint prefixCount = bitCount(activeMask & ((1u << gl_SubgroupInvocationID) - 1u));

    return prefixCount;
}
"#;
}

/// Subgroup size control for pipeline creation.
#[derive(Debug, Clone, Copy)]
pub struct SubgroupSizeControl {
    /// Required subgroup size (0 = default/any).
    pub required_size: u32,
    /// Whether to allow varying subgroup size.
    pub allow_varying: bool,
    /// Whether to require full subgroups.
    pub require_full: bool,
}

impl SubgroupSizeControl {
    /// Use default subgroup size.
    pub fn default_size() -> Self {
        Self {
            required_size: 0,
            allow_varying: true,
            require_full: false,
        }
    }

    /// Require specific subgroup size.
    pub fn fixed_size(size: u32) -> Self {
        Self {
            required_size: size,
            allow_varying: false,
            require_full: false,
        }
    }

    /// Require full subgroups (no partial subgroups).
    pub fn full_subgroups(size: u32) -> Self {
        Self {
            required_size: size,
            allow_varying: false,
            require_full: true,
        }
    }

    /// Create the Vulkan struct for pipeline creation.
    /// Returns None if using default size.
    pub fn to_vk(&self) -> Option<vk::PipelineShaderStageRequiredSubgroupSizeCreateInfo<'static>> {
        if self.required_size == 0 {
            None
        } else {
            Some(vk::PipelineShaderStageRequiredSubgroupSizeCreateInfo::default()
                .required_subgroup_size(self.required_size))
        }
    }
}

/// Calculate optimal workgroup size based on subgroup capabilities.
pub fn optimal_workgroup_size(caps: &SubgroupCapabilities, min_threads: u32) -> [u32; 3] {
    let subgroup_size = caps.subgroup_size;

    // Try to use multiple of subgroup size
    let threads_x = if min_threads <= subgroup_size {
        subgroup_size
    } else {
        // Round up to multiple of subgroup size
        ((min_threads + subgroup_size - 1) / subgroup_size) * subgroup_size
    };

    // For 2D workgroups, try to keep threads_x * threads_y divisible by subgroup_size
    let threads_y = 1;
    let threads_z = 1;

    [threads_x, threads_y, threads_z]
}

/// Calculate number of subgroups for a given thread count.
pub fn num_subgroups(caps: &SubgroupCapabilities, thread_count: u32) -> u32 {
    (thread_count + caps.subgroup_size - 1) / caps.subgroup_size
}
