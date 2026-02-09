//! Cooperative Matrix Operations for GPU Compute Acceleration
//!
//! VK_KHR_cooperative_matrix enables efficient matrix operations:
//! - Hardware-accelerated matrix multiply-add
//! - Optimal for neural network inference
//! - GEMM operations for compute shaders
//! - Tensor core utilization on NVIDIA/AMD

use ash::vk;

/// Cooperative matrix capabilities.
#[derive(Debug, Clone, Default)]
pub struct CooperativeMatrixCapabilities {
    /// Whether cooperative matrix is supported.
    pub supported: bool,
    /// Supported matrix properties.
    pub properties: Vec<CooperativeMatrixProperty>,
}

/// A single cooperative matrix property configuration.
#[derive(Debug, Clone, Copy)]
pub struct CooperativeMatrixProperty {
    /// Matrix M dimension.
    pub m_size: u32,
    /// Matrix N dimension.
    pub n_size: u32,
    /// Matrix K dimension.
    pub k_size: u32,
    /// A matrix component type.
    pub a_type: ComponentType,
    /// B matrix component type.
    pub b_type: ComponentType,
    /// C matrix component type.
    pub c_type: ComponentType,
    /// Result matrix component type.
    pub result_type: ComponentType,
    /// Whether saturating accumulation is supported.
    pub saturating_accumulation: bool,
    /// Scope of the cooperative operation.
    pub scope: CooperativeScope,
}

/// Component type for matrix elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentType {
    Float16,
    Float32,
    Float64,
    Int8,
    Int16,
    Int32,
    Uint8,
    Uint16,
    Uint32,
}

impl ComponentType {
    /// Convert from Vulkan component type.
    pub fn from_vk(ty: vk::ComponentTypeKHR) -> Option<Self> {
        match ty {
            vk::ComponentTypeKHR::FLOAT16 => Some(ComponentType::Float16),
            vk::ComponentTypeKHR::FLOAT32 => Some(ComponentType::Float32),
            vk::ComponentTypeKHR::FLOAT64 => Some(ComponentType::Float64),
            vk::ComponentTypeKHR::SINT8 => Some(ComponentType::Int8),
            vk::ComponentTypeKHR::SINT16 => Some(ComponentType::Int16),
            vk::ComponentTypeKHR::SINT32 => Some(ComponentType::Int32),
            vk::ComponentTypeKHR::UINT8 => Some(ComponentType::Uint8),
            vk::ComponentTypeKHR::UINT16 => Some(ComponentType::Uint16),
            vk::ComponentTypeKHR::UINT32 => Some(ComponentType::Uint32),
            _ => None,
        }
    }

    /// Convert to Vulkan component type.
    pub fn to_vk(&self) -> vk::ComponentTypeKHR {
        match self {
            ComponentType::Float16 => vk::ComponentTypeKHR::FLOAT16,
            ComponentType::Float32 => vk::ComponentTypeKHR::FLOAT32,
            ComponentType::Float64 => vk::ComponentTypeKHR::FLOAT64,
            ComponentType::Int8 => vk::ComponentTypeKHR::SINT8,
            ComponentType::Int16 => vk::ComponentTypeKHR::SINT16,
            ComponentType::Int32 => vk::ComponentTypeKHR::SINT32,
            ComponentType::Uint8 => vk::ComponentTypeKHR::UINT8,
            ComponentType::Uint16 => vk::ComponentTypeKHR::UINT16,
            ComponentType::Uint32 => vk::ComponentTypeKHR::UINT32,
        }
    }

    /// Get the size in bytes.
    pub fn size_bytes(&self) -> usize {
        match self {
            ComponentType::Float16 | ComponentType::Int16 | ComponentType::Uint16 => 2,
            ComponentType::Float32 | ComponentType::Int32 | ComponentType::Uint32 => 4,
            ComponentType::Float64 => 8,
            ComponentType::Int8 | ComponentType::Uint8 => 1,
        }
    }
}

/// Cooperative operation scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CooperativeScope {
    /// Subgroup scope (wave/warp).
    Subgroup,
    /// Workgroup scope.
    Workgroup,
    /// Device scope.
    Device,
}

impl CooperativeScope {
    /// Convert from Vulkan scope.
    pub fn from_vk(scope: vk::ScopeKHR) -> Option<Self> {
        match scope {
            vk::ScopeKHR::SUBGROUP => Some(CooperativeScope::Subgroup),
            vk::ScopeKHR::WORKGROUP => Some(CooperativeScope::Workgroup),
            vk::ScopeKHR::DEVICE => Some(CooperativeScope::Device),
            _ => None,
        }
    }

    /// Convert to Vulkan scope.
    pub fn to_vk(&self) -> vk::ScopeKHR {
        match self {
            CooperativeScope::Subgroup => vk::ScopeKHR::SUBGROUP,
            CooperativeScope::Workgroup => vk::ScopeKHR::WORKGROUP,
            CooperativeScope::Device => vk::ScopeKHR::DEVICE,
        }
    }
}

/// Query cooperative matrix capabilities.
pub fn query_capabilities(ctx: &super::context::VulkanContext) -> CooperativeMatrixCapabilities {
    // Check if feature is supported
    let mut coop_features = vk::PhysicalDeviceCooperativeMatrixFeaturesKHR::default();
    let mut features2 = vk::PhysicalDeviceFeatures2::default()
        .push_next(&mut coop_features);

    unsafe {
        ctx.instance.get_physical_device_features2(ctx.physical_device, &mut features2);
    }

    let _ = features2;
    let supported = coop_features.cooperative_matrix == vk::TRUE;

    if !supported {
        return CooperativeMatrixCapabilities::default();
    }

    // Query supported matrix properties
    // Note: This would require the extension to be loaded to enumerate properties
    // For now, return common configurations that are typically supported

    CooperativeMatrixCapabilities {
        supported,
        properties: vec![
            // Common FP16 configuration (tensor cores)
            CooperativeMatrixProperty {
                m_size: 16,
                n_size: 16,
                k_size: 16,
                a_type: ComponentType::Float16,
                b_type: ComponentType::Float16,
                c_type: ComponentType::Float16,
                result_type: ComponentType::Float16,
                saturating_accumulation: false,
                scope: CooperativeScope::Subgroup,
            },
            // FP16 input, FP32 accumulate
            CooperativeMatrixProperty {
                m_size: 16,
                n_size: 16,
                k_size: 16,
                a_type: ComponentType::Float16,
                b_type: ComponentType::Float16,
                c_type: ComponentType::Float32,
                result_type: ComponentType::Float32,
                saturating_accumulation: false,
                scope: CooperativeScope::Subgroup,
            },
            // INT8 configuration
            CooperativeMatrixProperty {
                m_size: 16,
                n_size: 16,
                k_size: 32,
                a_type: ComponentType::Int8,
                b_type: ComponentType::Int8,
                c_type: ComponentType::Int32,
                result_type: ComponentType::Int32,
                saturating_accumulation: true,
                scope: CooperativeScope::Subgroup,
            },
        ],
    }
}

/// Find a matrix configuration matching requirements.
pub fn find_configuration(
    caps: &CooperativeMatrixCapabilities,
    a_type: ComponentType,
    b_type: ComponentType,
    result_type: ComponentType,
) -> Option<&CooperativeMatrixProperty> {
    caps.properties.iter().find(|p| {
        p.a_type == a_type && p.b_type == b_type && p.result_type == result_type
    })
}

/// GLSL code snippets for cooperative matrix operations.
pub mod glsl {
    /// Extension for cooperative matrix.
    pub const EXTENSION: &str = "#extension GL_KHR_cooperative_matrix : require";

    /// Declare a cooperative matrix type.
    pub fn declare_matrix(name: &str, element_type: &str, rows: u32, cols: u32, use_type: &str) -> String {
        format!(
            "coopmat<{}, gl_ScopeSubgroup, {}, {}, {}> {};",
            element_type, rows, cols, use_type, name
        )
    }

    /// Load matrix from memory.
    pub const LOAD_MATRIX: &str = r#"
// Load a cooperative matrix from buffer
void loadMatrix(out coopmat<float16_t, gl_ScopeSubgroup, 16, 16, gl_MatrixUseA> mat,
                buffer float16_t data[], uint offset, uint stride) {
    coopMatLoad(mat, data, offset, stride, gl_CooperativeMatrixLayoutRowMajor);
}
"#;

    /// Store matrix to memory.
    pub const STORE_MATRIX: &str = r#"
// Store a cooperative matrix to buffer
void storeMatrix(coopmat<float16_t, gl_ScopeSubgroup, 16, 16, gl_MatrixUseAccumulator> mat,
                 buffer float16_t data[], uint offset, uint stride) {
    coopMatStore(mat, data, offset, stride, gl_CooperativeMatrixLayoutRowMajor);
}
"#;

    /// Matrix multiply-add operation.
    pub const MULTIPLY_ADD: &str = r#"
// Cooperative matrix multiply-add: D = A * B + C
coopmat<float16_t, gl_ScopeSubgroup, 16, 16, gl_MatrixUseAccumulator>
matrixMultiplyAdd(
    coopmat<float16_t, gl_ScopeSubgroup, 16, 16, gl_MatrixUseA> A,
    coopmat<float16_t, gl_ScopeSubgroup, 16, 16, gl_MatrixUseB> B,
    coopmat<float16_t, gl_ScopeSubgroup, 16, 16, gl_MatrixUseAccumulator> C
) {
    return coopMatMulAdd(A, B, C);
}
"#;

    /// Example: Simple matrix multiplication kernel.
    pub const GEMM_EXAMPLE: &str = r#"
#version 450
#extension GL_KHR_cooperative_matrix : require
#extension GL_EXT_shader_explicit_arithmetic_types_float16 : require

layout(local_size_x = 32, local_size_y = 1, local_size_z = 1) in;

layout(binding = 0) buffer MatrixA { float16_t A[]; };
layout(binding = 1) buffer MatrixB { float16_t B[]; };
layout(binding = 2) buffer MatrixC { float16_t C[]; };

layout(push_constant) uniform PushConstants {
    uint M, N, K;  // Matrix dimensions
    uint lda, ldb, ldc;  // Leading dimensions
} pc;

void main() {
    // Each subgroup computes a 16x16 tile of C
    uint tileM = gl_WorkGroupID.x * 16;
    uint tileN = gl_WorkGroupID.y * 16;

    // Declare cooperative matrices
    coopmat<float16_t, gl_ScopeSubgroup, 16, 16, gl_MatrixUseA> matA;
    coopmat<float16_t, gl_ScopeSubgroup, 16, 16, gl_MatrixUseB> matB;
    coopmat<float16_t, gl_ScopeSubgroup, 16, 16, gl_MatrixUseAccumulator> matC;

    // Initialize accumulator to zero
    matC = coopmat<float16_t, gl_ScopeSubgroup, 16, 16, gl_MatrixUseAccumulator>(0.0hf);

    // Loop over K dimension in tiles
    for (uint k = 0; k < pc.K; k += 16) {
        // Load tiles of A and B
        coopMatLoad(matA, A, tileM * pc.lda + k, pc.lda, gl_CooperativeMatrixLayoutRowMajor);
        coopMatLoad(matB, B, k * pc.ldb + tileN, pc.ldb, gl_CooperativeMatrixLayoutRowMajor);

        // Accumulate: C += A * B
        matC = coopMatMulAdd(matA, matB, matC);
    }

    // Store result
    coopMatStore(matC, C, tileM * pc.ldc + tileN, pc.ldc, gl_CooperativeMatrixLayoutRowMajor);
}
"#;
}

/// Configuration for matrix multiply operations.
#[derive(Debug, Clone, Copy)]
pub struct MatrixMultiplyConfig {
    /// M dimension (rows of A and C).
    pub m: u32,
    /// N dimension (columns of B and C).
    pub n: u32,
    /// K dimension (columns of A, rows of B).
    pub k: u32,
    /// Leading dimension of A.
    pub lda: u32,
    /// Leading dimension of B.
    pub ldb: u32,
    /// Leading dimension of C.
    pub ldc: u32,
    /// Whether to transpose A.
    pub trans_a: bool,
    /// Whether to transpose B.
    pub trans_b: bool,
    /// Alpha scaling factor.
    pub alpha: f32,
    /// Beta scaling factor.
    pub beta: f32,
}

impl MatrixMultiplyConfig {
    /// Create config for C = A * B.
    pub fn simple(m: u32, n: u32, k: u32) -> Self {
        Self {
            m,
            n,
            k,
            lda: k,
            ldb: n,
            ldc: n,
            trans_a: false,
            trans_b: false,
            alpha: 1.0,
            beta: 0.0,
        }
    }

    /// Calculate workgroup count for dispatch.
    pub fn workgroup_count(&self, tile_m: u32, tile_n: u32) -> [u32; 3] {
        [
            (self.m + tile_m - 1) / tile_m,
            (self.n + tile_n - 1) / tile_n,
            1,
        ]
    }
}

/// Push constants for matrix multiply shader.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct MatrixMultiplyPushConstants {
    pub m: u32,
    pub n: u32,
    pub k: u32,
    pub lda: u32,
    pub ldb: u32,
    pub ldc: u32,
    pub alpha: f32,
    pub beta: f32,
}

impl From<&MatrixMultiplyConfig> for MatrixMultiplyPushConstants {
    fn from(config: &MatrixMultiplyConfig) -> Self {
        Self {
            m: config.m,
            n: config.n,
            k: config.k,
            lda: config.lda,
            ldb: config.ldb,
            ldc: config.ldc,
            alpha: config.alpha,
            beta: config.beta,
        }
    }
}
