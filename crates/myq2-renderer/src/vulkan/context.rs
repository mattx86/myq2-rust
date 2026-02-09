//! Vulkan context: instance, physical device, logical device, and queues.

use ash::{vk, Entry, Instance, Device};
use ash::khr::{surface, swapchain};
use ash::khr::{acceleration_structure, ray_tracing_pipeline, deferred_host_operations};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use super::{APP_NAME, ENGINE_NAME, ENGINE_VERSION, REQUIRED_VK_VERSION};

/// Ray tracing capabilities of the selected physical device.
#[derive(Debug, Clone, Default)]
pub struct RtCapabilities {
    pub supported: bool,
    pub max_ray_recursion_depth: u32,
    pub shader_group_handle_size: u32,
    pub max_geometry_count: u64,
    pub max_instance_count: u64,
    pub supports_ray_query: bool,
}

/// Queue family indices.
#[derive(Debug, Clone, Copy, Default)]
pub struct QueueFamilyIndices {
    pub graphics: Option<u32>,
    pub present: Option<u32>,
    pub transfer: Option<u32>,
    pub compute: Option<u32>,
}

impl QueueFamilyIndices {
    pub fn is_complete(&self) -> bool {
        self.graphics.is_some() && self.present.is_some()
    }
}

/// Main Vulkan context holding all core Vulkan objects.
pub struct VulkanContext {
    pub entry: Entry,
    pub instance: Instance,
    pub physical_device: vk::PhysicalDevice,
    pub device: Device,
    pub queue_families: QueueFamilyIndices,

    // Queues
    pub graphics_queue: vk::Queue,
    pub present_queue: vk::Queue,
    pub transfer_queue: Option<vk::Queue>,
    pub compute_queue: Option<vk::Queue>,

    // Extension loaders
    pub surface_loader: surface::Instance,
    pub swapchain_loader: swapchain::Device,

    // Ray tracing extension loaders (optional)
    pub accel_struct_loader: Option<acceleration_structure::Device>,
    pub rt_pipeline_loader: Option<ray_tracing_pipeline::Device>,

    // Capabilities
    pub rt_capabilities: RtCapabilities,
    pub device_properties: vk::PhysicalDeviceProperties,
    pub device_features: vk::PhysicalDeviceFeatures,

    // Debug messenger (only in debug builds)
    #[cfg(debug_assertions)]
    debug_messenger: Option<vk::DebugUtilsMessengerEXT>,
    #[cfg(debug_assertions)]
    debug_utils_loader: Option<ash::ext::debug_utils::Instance>,
}

impl VulkanContext {
    /// Create a new Vulkan context.
    ///
    /// # Arguments
    /// * `window` - Raw window handle for surface creation
    /// * `enable_validation` - Whether to enable validation layers
    pub unsafe fn new(
        display_handle: raw_window_handle::RawDisplayHandle,
        enable_validation: bool,
    ) -> Result<Self, String> {
        // Load Vulkan entry point
        let entry = Entry::linked();

        // Check Vulkan version
        let api_version = match entry.try_enumerate_instance_version()
            .map_err(|e| format!("Failed to enumerate instance version: {:?}", e))?
        {
            Some(version) => version,
            None => vk::API_VERSION_1_0,
        };

        if api_version < REQUIRED_VK_VERSION {
            return Err(format!(
                "Vulkan 1.3 required, but only {}.{}.{} available",
                vk::api_version_major(api_version),
                vk::api_version_minor(api_version),
                vk::api_version_patch(api_version)
            ));
        }

        // Create instance
        let instance = Self::create_instance(&entry, display_handle, enable_validation)?;

        // Setup debug messenger in debug builds
        #[cfg(debug_assertions)]
        let (debug_utils_loader, debug_messenger) = if enable_validation {
            Self::setup_debug_messenger(&entry, &instance)?
        } else {
            (None, None)
        };

        // Create surface loader
        let surface_loader = surface::Instance::new(&entry, &instance);

        // Select physical device
        let (physical_device, queue_families, rt_caps) =
            Self::pick_physical_device(&instance, &surface_loader, None)?;

        // Get device properties
        let device_properties = instance.get_physical_device_properties(physical_device);
        let device_features = instance.get_physical_device_features(physical_device);

        // Create logical device
        let (device, graphics_queue, present_queue, transfer_queue, compute_queue) =
            Self::create_logical_device(&instance, physical_device, &queue_families, rt_caps.supported)?;

        // Create swapchain loader
        let swapchain_loader = swapchain::Device::new(&instance, &device);

        // Create ray tracing loaders if supported
        let (accel_struct_loader, rt_pipeline_loader) = if rt_caps.supported {
            let accel = acceleration_structure::Device::new(&instance, &device);
            let rt_pipe = ray_tracing_pipeline::Device::new(&instance, &device);
            (Some(accel), Some(rt_pipe))
        } else {
            (None, None)
        };

        Ok(Self {
            entry,
            instance,
            physical_device,
            device,
            queue_families,
            graphics_queue,
            present_queue,
            transfer_queue,
            compute_queue,
            surface_loader,
            swapchain_loader,
            accel_struct_loader,
            rt_pipeline_loader,
            rt_capabilities: rt_caps,
            device_properties,
            device_features,
            #[cfg(debug_assertions)]
            debug_messenger,
            #[cfg(debug_assertions)]
            debug_utils_loader,
        })
    }

    /// Create Vulkan instance with required extensions.
    unsafe fn create_instance(
        entry: &Entry,
        display_handle: raw_window_handle::RawDisplayHandle,
        enable_validation: bool,
    ) -> Result<Instance, String> {
        let app_info = vk::ApplicationInfo::default()
            .application_name(APP_NAME)
            .application_version(vk::make_api_version(0, 1, 0, 0))
            .engine_name(ENGINE_NAME)
            .engine_version(ENGINE_VERSION)
            .api_version(REQUIRED_VK_VERSION);

        // Get required surface extensions
        let mut extensions = ash_window::enumerate_required_extensions(display_handle)
            .map_err(|e| format!("Failed to get required extensions: {:?}", e))?
            .to_vec();

        // Add debug utils extension in debug builds
        #[cfg(debug_assertions)]
        if enable_validation {
            extensions.push(ash::ext::debug_utils::NAME.as_ptr());
        }

        // Validation layers
        let layer_names: Vec<CString> = if enable_validation {
            vec![CString::new("VK_LAYER_KHRONOS_validation").unwrap()]
        } else {
            vec![]
        };
        let layer_name_ptrs: Vec<*const c_char> = layer_names.iter()
            .map(|n| n.as_ptr())
            .collect();

        let create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(&extensions)
            .enabled_layer_names(&layer_name_ptrs);

        entry.create_instance(&create_info, None)
            .map_err(|e| format!("Failed to create Vulkan instance: {:?}", e))
    }

    /// Setup debug messenger for validation layers.
    #[cfg(debug_assertions)]
    unsafe fn setup_debug_messenger(
        entry: &Entry,
        instance: &Instance,
    ) -> Result<(Option<ash::ext::debug_utils::Instance>, Option<vk::DebugUtilsMessengerEXT>), String> {
        let debug_utils = ash::ext::debug_utils::Instance::new(entry, instance);

        let create_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
            .message_severity(
                vk::DebugUtilsMessageSeverityFlagsEXT::ERROR |
                vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
            )
            .message_type(
                vk::DebugUtilsMessageTypeFlagsEXT::GENERAL |
                vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION |
                vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
            )
            .pfn_user_callback(Some(debug_callback));

        let messenger = debug_utils
            .create_debug_utils_messenger(&create_info, None)
            .map_err(|e| format!("Failed to create debug messenger: {:?}", e))?;

        Ok((Some(debug_utils), Some(messenger)))
    }

    /// Select the best physical device.
    unsafe fn pick_physical_device(
        instance: &Instance,
        surface_loader: &surface::Instance,
        surface: Option<vk::SurfaceKHR>,
    ) -> Result<(vk::PhysicalDevice, QueueFamilyIndices, RtCapabilities), String> {
        let devices = instance.enumerate_physical_devices()
            .map_err(|e| format!("Failed to enumerate physical devices: {:?}", e))?;

        if devices.is_empty() {
            return Err("No Vulkan-capable GPU found".to_string());
        }

        // Score and sort devices
        let mut scored_devices: Vec<_> = devices.iter()
            .filter_map(|&device| {
                let score = Self::rate_device(instance, device, surface_loader, surface);
                if score > 0 {
                    Some((device, score))
                } else {
                    None
                }
            })
            .collect();

        scored_devices.sort_by(|a, b| b.1.cmp(&a.1));

        if scored_devices.is_empty() {
            return Err("No suitable GPU found".to_string());
        }

        let physical_device = scored_devices[0].0;
        let queue_families = Self::find_queue_families(instance, physical_device, surface_loader, surface);
        let rt_caps = Self::check_rt_support(instance, physical_device);

        // Log selected device
        let props = instance.get_physical_device_properties(physical_device);
        let name = CStr::from_ptr(props.device_name.as_ptr()).to_string_lossy();
        println!("Selected GPU: {} (RT: {})", name, if rt_caps.supported { "yes" } else { "no" });

        Ok((physical_device, queue_families, rt_caps))
    }

    /// Rate a physical device (higher is better).
    unsafe fn rate_device(
        instance: &Instance,
        device: vk::PhysicalDevice,
        surface_loader: &surface::Instance,
        surface: Option<vk::SurfaceKHR>,
    ) -> u32 {
        let props = instance.get_physical_device_properties(device);
        let features = instance.get_physical_device_features(device);

        // Must have geometry shader
        if features.geometry_shader == vk::FALSE {
            return 0;
        }

        // Must have required queue families
        let queue_families = Self::find_queue_families(instance, device, surface_loader, surface);
        if !queue_families.graphics.is_some() {
            return 0;
        }

        let mut score = 0u32;

        // Prefer discrete GPU
        if props.device_type == vk::PhysicalDeviceType::DISCRETE_GPU {
            score += 10000;
        } else if props.device_type == vk::PhysicalDeviceType::INTEGRATED_GPU {
            score += 1000;
        }

        // Bonus for ray tracing support
        let rt_caps = Self::check_rt_support(instance, device);
        if rt_caps.supported {
            score += 5000;
        }

        // Add VRAM size to score
        let memory_props = instance.get_physical_device_memory_properties(device);
        for i in 0..memory_props.memory_heap_count as usize {
            let heap = memory_props.memory_heaps[i];
            if heap.flags.contains(vk::MemoryHeapFlags::DEVICE_LOCAL) {
                score += (heap.size / (1024 * 1024)) as u32; // MB of VRAM
            }
        }

        score
    }

    /// Find queue family indices for a physical device.
    unsafe fn find_queue_families(
        instance: &Instance,
        device: vk::PhysicalDevice,
        surface_loader: &surface::Instance,
        surface: Option<vk::SurfaceKHR>,
    ) -> QueueFamilyIndices {
        let queue_families = instance.get_physical_device_queue_family_properties(device);

        let mut indices = QueueFamilyIndices::default();

        for (i, family) in queue_families.iter().enumerate() {
            let i = i as u32;

            // Graphics queue
            if family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                indices.graphics = Some(i);
            }

            // Present queue (requires surface)
            if let Some(surf) = surface {
                if surface_loader.get_physical_device_surface_support(device, i, surf).unwrap_or(false) {
                    indices.present = Some(i);
                }
            } else {
                // Without surface, assume graphics queue supports present
                if family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                    indices.present = Some(i);
                }
            }

            // Dedicated transfer queue
            if family.queue_flags.contains(vk::QueueFlags::TRANSFER) &&
               !family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                indices.transfer = Some(i);
            }

            // Dedicated compute queue
            if family.queue_flags.contains(vk::QueueFlags::COMPUTE) &&
               !family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                indices.compute = Some(i);
            }
        }

        indices
    }

    /// Check ray tracing support on a physical device.
    unsafe fn check_rt_support(instance: &Instance, device: vk::PhysicalDevice) -> RtCapabilities {
        // Check for required extensions
        let extensions = match instance.enumerate_device_extension_properties(device) {
            Ok(ext) => ext,
            Err(_) => return RtCapabilities::default(),
        };

        let has_accel_struct = extensions.iter().any(|e| {
            let name = CStr::from_ptr(e.extension_name.as_ptr());
            name == acceleration_structure::NAME
        });

        let has_rt_pipeline = extensions.iter().any(|e| {
            let name = CStr::from_ptr(e.extension_name.as_ptr());
            name == ray_tracing_pipeline::NAME
        });

        if !has_accel_struct || !has_rt_pipeline {
            return RtCapabilities::default();
        }

        // Query RT properties
        let mut rt_props = vk::PhysicalDeviceRayTracingPipelinePropertiesKHR::default();
        let mut props2 = vk::PhysicalDeviceProperties2::default()
            .push_next(&mut rt_props);

        instance.get_physical_device_properties2(device, &mut props2);

        // Query RT features
        let mut accel_features = vk::PhysicalDeviceAccelerationStructureFeaturesKHR::default();
        let mut rt_features = vk::PhysicalDeviceRayTracingPipelineFeaturesKHR::default();
        let mut ray_query_features = vk::PhysicalDeviceRayQueryFeaturesKHR::default();
        let mut features2 = vk::PhysicalDeviceFeatures2::default()
            .push_next(&mut accel_features)
            .push_next(&mut rt_features)
            .push_next(&mut ray_query_features);

        instance.get_physical_device_features2(device, &mut features2);

        RtCapabilities {
            supported: accel_features.acceleration_structure == vk::TRUE &&
                       rt_features.ray_tracing_pipeline == vk::TRUE,
            max_ray_recursion_depth: rt_props.max_ray_recursion_depth,
            shader_group_handle_size: rt_props.shader_group_handle_size,
            max_geometry_count: 0, // Would need AccelerationStructure properties
            max_instance_count: 0,
            supports_ray_query: ray_query_features.ray_query == vk::TRUE,
        }
    }

    /// Create logical device with required features and extensions.
    unsafe fn create_logical_device(
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
        queue_families: &QueueFamilyIndices,
        enable_rt: bool,
    ) -> Result<(Device, vk::Queue, vk::Queue, Option<vk::Queue>, Option<vk::Queue>), String> {
        // Collect unique queue families
        let mut unique_families = vec![queue_families.graphics.unwrap()];
        if let Some(present) = queue_families.present {
            if !unique_families.contains(&present) {
                unique_families.push(present);
            }
        }
        if let Some(transfer) = queue_families.transfer {
            if !unique_families.contains(&transfer) {
                unique_families.push(transfer);
            }
        }
        if let Some(compute) = queue_families.compute {
            if !unique_families.contains(&compute) {
                unique_families.push(compute);
            }
        }

        let queue_priorities = [1.0f32];
        let queue_create_infos: Vec<_> = unique_families.iter()
            .map(|&family| {
                vk::DeviceQueueCreateInfo::default()
                    .queue_family_index(family)
                    .queue_priorities(&queue_priorities)
            })
            .collect();

        // Device extensions
        let mut extensions: Vec<*const c_char> = vec![
            swapchain::NAME.as_ptr(),
        ];

        if enable_rt {
            extensions.push(acceleration_structure::NAME.as_ptr());
            extensions.push(ray_tracing_pipeline::NAME.as_ptr());
            extensions.push(deferred_host_operations::NAME.as_ptr());
            extensions.push(vk::KHR_BUFFER_DEVICE_ADDRESS_NAME.as_ptr());
        }

        // Vulkan 1.3 features
        let mut vulkan_13_features = vk::PhysicalDeviceVulkan13Features::default()
            .synchronization2(true)
            .dynamic_rendering(true)
            .maintenance4(true);

        let mut vulkan_12_features = vk::PhysicalDeviceVulkan12Features::default()
            .buffer_device_address(enable_rt)
            .descriptor_indexing(true)
            .runtime_descriptor_array(true);

        // RT features (if enabled)
        let mut accel_features = vk::PhysicalDeviceAccelerationStructureFeaturesKHR::default()
            .acceleration_structure(enable_rt);

        let mut rt_features = vk::PhysicalDeviceRayTracingPipelineFeaturesKHR::default()
            .ray_tracing_pipeline(enable_rt);

        let mut ray_query_features = vk::PhysicalDeviceRayQueryFeaturesKHR::default()
            .ray_query(enable_rt);

        let device_features = vk::PhysicalDeviceFeatures::default()
            .geometry_shader(true)
            .sampler_anisotropy(true)
            .fill_mode_non_solid(true);

        let mut features2 = vk::PhysicalDeviceFeatures2::default()
            .features(device_features)
            .push_next(&mut vulkan_13_features)
            .push_next(&mut vulkan_12_features);

        if enable_rt {
            features2 = features2
                .push_next(&mut accel_features)
                .push_next(&mut rt_features)
                .push_next(&mut ray_query_features);
        }

        let create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_create_infos)
            .enabled_extension_names(&extensions)
            .push_next(&mut features2);

        let device = instance.create_device(physical_device, &create_info, None)
            .map_err(|e| format!("Failed to create logical device: {:?}", e))?;

        // Get queues
        let graphics_queue = device.get_device_queue(queue_families.graphics.unwrap(), 0);
        let present_queue = device.get_device_queue(queue_families.present.unwrap_or(queue_families.graphics.unwrap()), 0);
        let transfer_queue = queue_families.transfer.map(|f| device.get_device_queue(f, 0));
        let compute_queue = queue_families.compute.map(|f| device.get_device_queue(f, 0));

        Ok((device, graphics_queue, present_queue, transfer_queue, compute_queue))
    }

    /// Wait for all device operations to complete.
    pub fn wait_idle(&self) {
        unsafe {
            let _ = self.device.device_wait_idle();
        }
    }
}

impl Drop for VulkanContext {
    fn drop(&mut self) {
        unsafe {
            self.wait_idle();

            #[cfg(debug_assertions)]
            if let (Some(loader), Some(messenger)) = (&self.debug_utils_loader, self.debug_messenger) {
                loader.destroy_debug_utils_messenger(messenger, None);
            }

            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}

/// Debug callback for validation layers.
#[cfg(debug_assertions)]
unsafe extern "system" fn debug_callback(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    _ty: vk::DebugUtilsMessageTypeFlagsEXT,
    data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _user_data: *mut std::ffi::c_void,
) -> vk::Bool32 {
    let message = CStr::from_ptr((*data).p_message).to_string_lossy();

    if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) {
        eprintln!("[VK ERROR] {}", message);
    } else if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::WARNING) {
        eprintln!("[VK WARN] {}", message);
    }

    vk::FALSE
}
