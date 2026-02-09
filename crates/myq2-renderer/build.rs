use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let dest = env::var("OUT_DIR").unwrap();

    // ========================================================================
    // SPIR-V Shader Compilation (via glslc from Vulkan SDK)
    // ========================================================================
    // Targets Vulkan 1.3 with SPIR-V 1.6 for advanced features including
    // ray query support and dynamic rendering.
    let shader_dir = Path::new("shaders");
    if !shader_dir.exists() {
        return;
    }

    // Find glslc: check PATH first, then VULKAN_SDK
    let glslc = find_glslc();
    let glslc = match glslc {
        Some(path) => path,
        None => {
            panic!(
                "glslc not found. Install the Vulkan SDK to compile shaders.\n\
                 Download from: https://vulkan.lunarg.com/sdk/home\n\
                 Ensure VULKAN_SDK is set or glslc is on PATH."
            );
        }
    };

    println!("cargo:warning=Using glslc: {}", glslc.display());

    let spirv_dir = Path::new(&dest).join("spirv");
    fs::create_dir_all(&spirv_dir).unwrap();

    // Rasterization shaders (vertex/fragment)
    let raster_shaders = [
        "world.vert.glsl",
        "world.frag.glsl",
        "water.vert.glsl",
        "water.frag.glsl",
        "alias.vert.glsl",
        "alias.frag.glsl",
        "alias_cel.vert.glsl",
        "alias_cel.frag.glsl",
        "sky.vert.glsl",
        "sky.frag.glsl",
        "particle.vert.glsl",
        "particle.frag.glsl",
        "ui.vert.glsl",
        "ui.frag.glsl",
        "dlight.vert.glsl",
        "dlight.frag.glsl",
        "postprocess.vert.glsl",
        "postprocess.frag.glsl",
        "fxaa.frag.glsl",
        "ssao.frag.glsl",
        "ssao_blur.frag.glsl",
        "bloom_extract.frag.glsl",
        "bloom_blur.frag.glsl",
        "bloom_composite.frag.glsl",
        // FSR 1.0 (spatial upscaling)
        "fsr_easu.frag.glsl",
        "fsr_rcas.frag.glsl",
        // FSR 2.0 (temporal upscaling)
        "fsr2_temporal.frag.glsl",
        "motion_vectors.vert.glsl",
        "motion_vectors.frag.glsl",
    ];

    // Ray tracing shaders (in rt/ subdirectory)
    // Format: (filename, stage) - stages use glslc abbreviations
    let rt_shaders = [
        ("rt/shadow_ray.rgen.glsl", "rgen"),      // ray generation
        ("rt/shadow.rmiss.glsl", "rmiss"),        // ray miss
        ("rt/shadow.rahit.glsl", "rahit"),        // ray any-hit
        ("rt/reflection_ray.rgen.glsl", "rgen"),  // ray generation
        ("rt/reflection.rchit.glsl", "rchit"),    // ray closest-hit
        ("rt/reflection.rmiss.glsl", "rmiss"),    // ray miss
        ("rt/water.rchit.glsl", "rchit"),         // ray closest-hit
        ("rt/denoise.comp.glsl", "compute"),      // compute
    ];

    let mut any_failed = false;

    // Compile rasterization shaders
    for filename in &raster_shaders {
        let src_path = shader_dir.join(filename);
        let spv_name = filename.replace(".glsl", ".spv");
        let spv_path = spirv_dir.join(&spv_name);

        // Determine shader stage from filename (.vert.glsl / .frag.glsl)
        let stage = if filename.contains(".vert.") {
            "vertex"
        } else {
            "fragment"
        };

        any_failed |= !compile_shader(&glslc, &src_path, &spv_path, stage, filename);
        println!("cargo:rerun-if-changed=shaders/{filename}");
    }

    // Create RT output directory
    let rt_spirv_dir = spirv_dir.join("rt");
    fs::create_dir_all(&rt_spirv_dir).unwrap();

    // Compile ray tracing shaders
    for (filename, stage) in &rt_shaders {
        let src_path = shader_dir.join(filename);
        // Output to rt/ subdirectory with flattened name
        let spv_name = filename.replace("rt/", "").replace(".glsl", ".spv");
        let spv_path = rt_spirv_dir.join(&spv_name);

        any_failed |= !compile_shader(&glslc, &src_path, &spv_path, stage, filename);
        println!("cargo:rerun-if-changed=shaders/{filename}");
    }

    if any_failed {
        panic!("Some shaders failed to compile (see warnings above)");
    }
}

/// Compile a single shader file to SPIR-V.
fn compile_shader(glslc: &Path, src_path: &Path, spv_path: &Path, stage: &str, filename: &str) -> bool {
    let output = Command::new(glslc)
        .arg(format!("-fshader-stage={stage}"))
        .args(["--target-env=vulkan1.3", "--target-spv=spv1.6", "-O", "-o"])
        .arg(spv_path)
        .arg(src_path)
        .output();

    match output {
        Ok(result) => {
            if result.status.success() {
                let stderr = String::from_utf8_lossy(&result.stderr);
                if !stderr.is_empty() {
                    println!("cargo:warning=glslc {filename}: {stderr}");
                }
                true
            } else {
                let stderr = String::from_utf8_lossy(&result.stderr);
                println!("cargo:warning=glslc FAILED {filename}: {stderr}");
                false
            }
        }
        Err(e) => {
            println!("cargo:warning=glslc: failed to run for {filename}: {e}");
            false
        }
    }
}

/// Find glslc binary: check PATH, then VULKAN_SDK/Bin/
fn find_glslc() -> Option<PathBuf> {
    // Check PATH
    if let Ok(output) = Command::new("glslc").arg("--version").output() {
        if output.status.success() {
            return Some(PathBuf::from("glslc"));
        }
    }

    // Check VULKAN_SDK environment variable
    if let Ok(sdk) = env::var("VULKAN_SDK") {
        let glslc_path = PathBuf::from(&sdk).join("Bin").join("glslc.exe");
        if glslc_path.exists() {
            return Some(glslc_path);
        }
        // Try lowercase bin
        let glslc_path = PathBuf::from(&sdk).join("bin").join("glslc");
        if glslc_path.exists() {
            return Some(glslc_path);
        }
    }

    // Common Windows install paths
    let common_paths = [
        r"C:\VulkanSDK",
    ];
    for base in &common_paths {
        let base_path = Path::new(base);
        if base_path.exists() {
            // Find newest SDK version directory
            if let Ok(entries) = fs::read_dir(base_path) {
                let mut versions: Vec<_> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                    .collect();
                versions.sort_by(|a, b| b.file_name().cmp(&a.file_name()));

                for entry in versions {
                    let glslc_path = entry.path().join("Bin").join("glslc.exe");
                    if glslc_path.exists() {
                        return Some(glslc_path);
                    }
                }
            }
        }
    }

    None
}
