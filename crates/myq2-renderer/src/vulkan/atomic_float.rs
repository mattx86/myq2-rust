//! Additional Atomic Operations (VK_EXT_shader_atomic_float2)
//!
//! Extended atomic operations for floating-point values:
//! - atomicAdd for float16/float32/float64
//! - atomicMin/atomicMax for float32/float64
//! - Useful for accumulation buffers, parallel reduction
//! - Order-independent transparency
//! - Soft shadows accumulation

use ash::vk;

/// Atomic float capabilities.
#[derive(Debug, Clone, Default)]
pub struct AtomicFloatCapabilities {
    /// VK_EXT_shader_atomic_float supported.
    pub atomic_float: bool,
    /// VK_EXT_shader_atomic_float2 supported.
    pub atomic_float2: bool,

    // VK_EXT_shader_atomic_float features
    /// atomicAdd on float32 in buffers.
    pub buffer_float32_atomics: bool,
    /// atomicAdd on float32 in images.
    pub image_float32_atomics: bool,
    /// atomicAdd on float32 in shared memory.
    pub shared_float32_atomics: bool,

    // VK_EXT_shader_atomic_float2 features
    /// atomicAdd on float16 in buffers.
    pub buffer_float16_atomics: bool,
    /// atomicMin/Max on float16 in buffers.
    pub buffer_float16_atomic_minmax: bool,
    /// atomicMin/Max on float32 in buffers.
    pub buffer_float32_atomic_minmax: bool,
    /// atomicMin/Max on float64 in buffers.
    pub buffer_float64_atomic_minmax: bool,
    /// atomicAdd on float16 in images.
    pub image_float32_atomic_minmax: bool,
    /// atomicAdd on float64 in shared memory.
    pub shared_float16_atomics: bool,
    /// atomicMin/Max on float16 in shared memory.
    pub shared_float16_atomic_minmax: bool,
    /// atomicMin/Max on float32 in shared memory.
    pub shared_float32_atomic_minmax: bool,
}

/// Query atomic float capabilities.
pub fn query_capabilities(ctx: &super::context::VulkanContext) -> AtomicFloatCapabilities {
    let mut atomic_float_features = vk::PhysicalDeviceShaderAtomicFloatFeaturesEXT::default();
    let mut atomic_float2_features = vk::PhysicalDeviceShaderAtomicFloat2FeaturesEXT::default();
    let mut features2 = vk::PhysicalDeviceFeatures2::default()
        .push_next(&mut atomic_float_features)
        .push_next(&mut atomic_float2_features);

    unsafe {
        ctx.instance.get_physical_device_features2(ctx.physical_device, &mut features2);
    }

    let _ = features2;

    AtomicFloatCapabilities {
        atomic_float: atomic_float_features.shader_buffer_float32_atomics == vk::TRUE
            || atomic_float_features.shader_image_float32_atomics == vk::TRUE,
        atomic_float2: atomic_float2_features.shader_buffer_float16_atomics == vk::TRUE,

        buffer_float32_atomics: atomic_float_features.shader_buffer_float32_atomics == vk::TRUE,
        image_float32_atomics: atomic_float_features.shader_image_float32_atomics == vk::TRUE,
        shared_float32_atomics: atomic_float_features.shader_shared_float32_atomics == vk::TRUE,

        buffer_float16_atomics: atomic_float2_features.shader_buffer_float16_atomics == vk::TRUE,
        buffer_float16_atomic_minmax: atomic_float2_features.shader_buffer_float16_atomic_min_max == vk::TRUE,
        buffer_float32_atomic_minmax: atomic_float2_features.shader_buffer_float32_atomic_min_max == vk::TRUE,
        buffer_float64_atomic_minmax: atomic_float2_features.shader_buffer_float64_atomic_min_max == vk::TRUE,
        image_float32_atomic_minmax: atomic_float2_features.shader_image_float32_atomic_min_max == vk::TRUE,
        shared_float16_atomics: atomic_float2_features.shader_shared_float16_atomics == vk::TRUE,
        shared_float16_atomic_minmax: atomic_float2_features.shader_shared_float16_atomic_min_max == vk::TRUE,
        shared_float32_atomic_minmax: atomic_float2_features.shader_shared_float32_atomic_min_max == vk::TRUE,
    }
}

/// GLSL code for atomic float operations.
pub mod glsl {
    /// Extension declarations.
    pub const EXTENSIONS: &str = r#"
#extension GL_EXT_shader_atomic_float : enable
#extension GL_EXT_shader_atomic_float2 : enable
"#;

    /// Basic atomic operations.
    pub const BASIC_ATOMICS: &str = r#"
// Atomic add to buffer
// layout(binding = 0, std430) buffer AccumBuffer { float values[]; };
// atomicAdd(values[index], value);

// Atomic add to image (r32f format required)
// layout(binding = 1, r32f) uniform image2D accumImage;
// imageAtomicAdd(accumImage, coord, value);

// Atomic min/max for depth
// atomicMin(depthBuffer[index], depth);
// atomicMax(luminanceBuffer[index], luminance);
"#;

    /// Software atomic add fallback.
    pub const SOFTWARE_ATOMIC_ADD: &str = r#"
// Software atomic add using compare-exchange
// Use when hardware atomics not available

float atomicAddSoftware(inout uint packed, float value) {
    uint expected;
    uint desired;
    float current;

    do {
        expected = packed;
        current = uintBitsToFloat(expected);
        desired = floatBitsToUint(current + value);
    } while (atomicCompSwap(packed, expected, desired) != expected);

    return current;
}

// For shared memory
shared uint sharedAccumPacked[256];

void atomicAddShared(uint index, float value) {
    uint expected;
    uint desired;
    float current;

    do {
        expected = sharedAccumPacked[index];
        current = uintBitsToFloat(expected);
        desired = floatBitsToUint(current + value);
    } while (atomicCompSwap(sharedAccumPacked[index], expected, desired) != expected);
}
"#;

    /// Parallel reduction using atomics.
    pub const PARALLEL_REDUCTION: &str = r#"
// Parallel reduction to compute sum/min/max
// Much more efficient with hardware atomic floats

layout(binding = 0, std430) buffer InputBuffer {
    float inputData[];
};

layout(binding = 1, std430) buffer OutputBuffer {
    float result;
    float minVal;
    float maxVal;
    uint count;
};

shared float sharedData[256];

layout(local_size_x = 256) in;

void reduceSum() {
    uint tid = gl_LocalInvocationID.x;
    uint gid = gl_GlobalInvocationID.x;

    // Load to shared memory
    sharedData[tid] = inputData[gid];
    barrier();

    // Local reduction in shared memory
    for (uint stride = 128u; stride > 0u; stride >>= 1u) {
        if (tid < stride) {
            sharedData[tid] += sharedData[tid + stride];
        }
        barrier();
    }

    // First thread of each workgroup does atomic add to global result
    if (tid == 0u) {
        atomicAdd(result, sharedData[0]);
    }
}

void reduceMinMax() {
    uint tid = gl_LocalInvocationID.x;
    uint gid = gl_GlobalInvocationID.x;

    float val = inputData[gid];
    sharedData[tid] = val;
    barrier();

    // Local reduction
    for (uint stride = 128u; stride > 0u; stride >>= 1u) {
        if (tid < stride) {
            sharedData[tid] = min(sharedData[tid], sharedData[tid + stride]);
        }
        barrier();
    }

    if (tid == 0u) {
        atomicMin(minVal, sharedData[0]);
    }
}
"#;

    /// Accumulation buffer for effects.
    pub const ACCUMULATION_BUFFER: &str = r#"
// Accumulation buffer for soft shadows, AO, etc.

layout(binding = 0, r32f) uniform image2D accumBuffer;
layout(binding = 1, r32ui) uniform uimage2D countBuffer;

// Accumulate sample
void accumulateSample(ivec2 coord, float value) {
    imageAtomicAdd(accumBuffer, coord, value);
    imageAtomicAdd(countBuffer, coord, 1u);
}

// Read normalized result
float readAccumulated(ivec2 coord) {
    float sum = imageLoad(accumBuffer, coord).r;
    uint count = imageLoad(countBuffer, coord).r;
    return count > 0u ? sum / float(count) : 0.0;
}

// Progressive accumulation with exponential moving average
void progressiveAccumulate(ivec2 coord, float newValue, float blendFactor) {
    float current = imageLoad(accumBuffer, coord).r;
    float blended = mix(current, newValue, blendFactor);
    // Note: This isn't truly atomic, use atomicExchange for strict ordering
    imageStore(accumBuffer, coord, vec4(blended));
}
"#;

    /// Order-independent transparency with atomics.
    pub const OIT_ATOMIC: &str = r#"
// Order-Independent Transparency using atomic operations

layout(binding = 0, r32f) uniform image2D oitAccumColor;
layout(binding = 1, r32f) uniform image2D oitAccumAlpha;
layout(binding = 2, r32f) uniform image2D oitRevealage;

// Weighted blended OIT accumulation
void oitAccumulate(ivec2 coord, vec4 color, float depth) {
    // Weight function
    float weight = color.a * max(0.01, min(3000.0,
        10.0 / (0.00001 + pow(abs(depth) / 5.0, 2.0) +
                         pow(abs(depth) / 200.0, 6.0))));

    // Accumulate color (weighted)
    // Need to accumulate RGB separately or use RGBA atomic
    imageAtomicAdd(oitAccumColor, coord, color.r * color.a * weight);
    // ... repeat for G, B, A or use structured buffer

    // Accumulate revealage (multiply)
    // This requires atomic multiply or special handling
    float currentReveal = imageLoad(oitRevealage, coord).r;
    float newReveal = currentReveal * (1.0 - color.a);
    // Note: Not truly atomic, may need mutex or linked list approach
}

// Alternative: Use per-pixel linked lists with atomic counters
layout(binding = 3, std430) buffer FragmentList {
    uint fragmentCount;
    uvec4 fragments[]; // RGBA + depth packed
};

layout(binding = 4, r32ui) uniform uimage2D headPointers;

void linkedListAppend(ivec2 coord, vec4 color, float depth) {
    uint newIndex = atomicAdd(fragmentCount, 1u);

    // Pack color and depth
    uint packedColor = packUnorm4x8(color);
    uint packedDepth = floatBitsToUint(depth);

    // Swap head pointer
    uint oldHead = imageAtomicExchange(headPointers, coord, newIndex);

    // Store fragment with link to previous
    fragments[newIndex] = uvec4(packedColor, packedDepth, oldHead, 0u);
}
"#;

    /// Histogram computation.
    pub const HISTOGRAM: &str = r#"
// Parallel histogram computation using atomics

layout(binding = 0) uniform sampler2D inputImage;
layout(binding = 1, std430) buffer HistogramBuffer {
    uint histogram[256];
};

shared uint localHistogram[256];

layout(local_size_x = 16, local_size_y = 16) in;

void main() {
    uint tid = gl_LocalInvocationID.x + gl_LocalInvocationID.y * 16u;

    // Initialize local histogram
    if (tid < 256u) {
        localHistogram[tid] = 0u;
    }
    barrier();

    // Sample image and accumulate to local histogram
    ivec2 coord = ivec2(gl_GlobalInvocationID.xy);
    vec4 color = texelFetch(inputImage, coord, 0);
    float luminance = dot(color.rgb, vec3(0.299, 0.587, 0.114));
    uint bin = clamp(uint(luminance * 255.0), 0u, 255u);

    atomicAdd(localHistogram[bin], 1u);
    barrier();

    // Merge to global histogram
    if (tid < 256u) {
        atomicAdd(histogram[tid], localHistogram[tid]);
    }
}
"#;

    /// Soft shadow accumulation.
    pub const SOFT_SHADOWS: &str = r#"
// Soft shadow accumulation using atomic add

layout(binding = 0, r32f) uniform image2D shadowAccum;

// Accumulate shadow samples
void accumulateShadowSample(ivec2 coord, float visibility) {
    imageAtomicAdd(shadowAccum, coord, visibility);
}

// Finalize with sample count
vec4 finalizeShadow(ivec2 coord, uint sampleCount) {
    float totalVisibility = imageLoad(shadowAccum, coord).r;
    float avgVisibility = totalVisibility / float(sampleCount);
    return vec4(vec3(avgVisibility), 1.0);
}
"#;

    /// Luminance adaptation.
    pub const LUMINANCE_ADAPTATION: &str = r#"
// Auto-exposure using atomic min/max

layout(binding = 0) uniform sampler2D sceneHDR;
layout(binding = 1, std430) buffer LuminanceBuffer {
    float minLuminance;
    float maxLuminance;
    float avgLuminance;
    uint pixelCount;
};

shared float sharedMin;
shared float sharedMax;
shared float sharedSum;
shared uint sharedCount;

layout(local_size_x = 16, local_size_y = 16) in;

void main() {
    uint tid = gl_LocalInvocationID.x + gl_LocalInvocationID.y * 16u;

    // Initialize shared memory
    if (tid == 0u) {
        sharedMin = 1e10;
        sharedMax = 0.0;
        sharedSum = 0.0;
        sharedCount = 0u;
    }
    barrier();

    // Sample and compute luminance
    ivec2 coord = ivec2(gl_GlobalInvocationID.xy);
    vec3 color = texelFetch(sceneHDR, coord, 0).rgb;
    float lum = dot(color, vec3(0.2126, 0.7152, 0.0722));
    float logLum = log(max(lum, 0.0001));

    // Local reduction
    atomicMin(sharedMin, logLum);
    atomicMax(sharedMax, logLum);
    atomicAdd(sharedSum, logLum);
    atomicAdd(sharedCount, 1u);
    barrier();

    // Global reduction
    if (tid == 0u) {
        atomicMin(minLuminance, sharedMin);
        atomicMax(maxLuminance, sharedMax);
        atomicAdd(avgLuminance, sharedSum);
        atomicAdd(pixelCount, sharedCount);
    }
}
"#;
}

/// Atomic operation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomicOp {
    /// Atomic add.
    Add,
    /// Atomic minimum.
    Min,
    /// Atomic maximum.
    Max,
    /// Atomic exchange.
    Exchange,
    /// Compare and swap.
    CompareSwap,
}

/// Data type for atomic operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomicDataType {
    /// 16-bit float (half).
    Float16,
    /// 32-bit float.
    Float32,
    /// 64-bit float (double).
    Float64,
    /// 32-bit unsigned integer.
    Uint32,
    /// 64-bit unsigned integer.
    Uint64,
}

/// Memory location for atomic operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomicMemoryLocation {
    /// Buffer storage.
    Buffer,
    /// Image storage.
    Image,
    /// Shared memory.
    Shared,
}

/// Atomic float manager.
pub struct AtomicFloatManager {
    capabilities: AtomicFloatCapabilities,
}

impl AtomicFloatManager {
    /// Create new atomic float manager.
    pub fn new(ctx: &super::context::VulkanContext) -> Self {
        let capabilities = query_capabilities(ctx);

        Self { capabilities }
    }

    /// Check if any atomic float support is available.
    pub fn is_supported(&self) -> bool {
        self.capabilities.atomic_float || self.capabilities.atomic_float2
    }

    /// Get capabilities.
    pub fn capabilities(&self) -> &AtomicFloatCapabilities {
        &self.capabilities
    }

    /// Check if specific operation is supported.
    pub fn is_operation_supported(
        &self,
        op: AtomicOp,
        data_type: AtomicDataType,
        location: AtomicMemoryLocation,
    ) -> bool {
        match (op, data_type, location) {
            // atomicAdd float32
            (AtomicOp::Add, AtomicDataType::Float32, AtomicMemoryLocation::Buffer) => {
                self.capabilities.buffer_float32_atomics
            }
            (AtomicOp::Add, AtomicDataType::Float32, AtomicMemoryLocation::Image) => {
                self.capabilities.image_float32_atomics
            }
            (AtomicOp::Add, AtomicDataType::Float32, AtomicMemoryLocation::Shared) => {
                self.capabilities.shared_float32_atomics
            }

            // atomicAdd float16
            (AtomicOp::Add, AtomicDataType::Float16, AtomicMemoryLocation::Buffer) => {
                self.capabilities.buffer_float16_atomics
            }
            (AtomicOp::Add, AtomicDataType::Float16, AtomicMemoryLocation::Shared) => {
                self.capabilities.shared_float16_atomics
            }

            // atomicMin/Max float32
            (AtomicOp::Min | AtomicOp::Max, AtomicDataType::Float32, AtomicMemoryLocation::Buffer) => {
                self.capabilities.buffer_float32_atomic_minmax
            }
            (AtomicOp::Min | AtomicOp::Max, AtomicDataType::Float32, AtomicMemoryLocation::Image) => {
                self.capabilities.image_float32_atomic_minmax
            }
            (AtomicOp::Min | AtomicOp::Max, AtomicDataType::Float32, AtomicMemoryLocation::Shared) => {
                self.capabilities.shared_float32_atomic_minmax
            }

            // atomicMin/Max float16
            (AtomicOp::Min | AtomicOp::Max, AtomicDataType::Float16, AtomicMemoryLocation::Buffer) => {
                self.capabilities.buffer_float16_atomic_minmax
            }
            (AtomicOp::Min | AtomicOp::Max, AtomicDataType::Float16, AtomicMemoryLocation::Shared) => {
                self.capabilities.shared_float16_atomic_minmax
            }

            // atomicMin/Max float64
            (AtomicOp::Min | AtomicOp::Max, AtomicDataType::Float64, AtomicMemoryLocation::Buffer) => {
                self.capabilities.buffer_float64_atomic_minmax
            }

            // Integer atomics are always available
            (_, AtomicDataType::Uint32 | AtomicDataType::Uint64, _) => true,

            _ => false,
        }
    }

    /// Get recommended accumulation method based on capabilities.
    pub fn recommended_accumulation_method(&self) -> AccumulationMethod {
        if self.capabilities.buffer_float32_atomics {
            AccumulationMethod::HardwareAtomic
        } else {
            AccumulationMethod::SoftwareCAS
        }
    }
}

/// Accumulation method based on hardware support.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccumulationMethod {
    /// Use hardware atomic float operations.
    HardwareAtomic,
    /// Use software compare-and-swap loop.
    SoftwareCAS,
    /// Use per-pixel linked lists.
    LinkedList,
    /// Use separate passes with barriers.
    MultiPass,
}
