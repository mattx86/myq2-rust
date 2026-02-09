// Build script for myq2-sys
// Adds Vulkan SDK library path for linking

fn main() {
    // Add Vulkan SDK library path on Windows
    #[cfg(target_os = "windows")]
    {
        if let Ok(vulkan_sdk) = std::env::var("VULKAN_SDK") {
            println!("cargo:rustc-link-search=native={}/Lib", vulkan_sdk);
        } else {
            // Common installation paths
            let paths = [
                "C:/VulkanSDK/1.4.341.1/Lib",
                "C:/VulkanSDK/1.3.296.0/Lib",
                "C:/VulkanSDK/1.3.280.0/Lib",
            ];
            for path in &paths {
                if std::path::Path::new(path).exists() {
                    println!("cargo:rustc-link-search=native={}", path);
                    break;
                }
            }
        }
    }

    // Link the Vulkan loader library
    #[cfg(target_os = "windows")]
    println!("cargo:rustc-link-lib=vulkan-1");

    #[cfg(target_os = "linux")]
    println!("cargo:rustc-link-lib=vulkan");

    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-lib=vulkan");
}
