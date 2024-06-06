use std::sync::Arc;
use std::{
    borrow::Cow,
    ffi::{c_char, CStr},
};

use ash::vk;
use raw_window_handle::HasDisplayHandle;
use winit::window::Window;

const VALIDATION_LAYER_NAME: &CStr =
    unsafe { CStr::from_bytes_with_nul_unchecked(b"VK_LAYER_KHRONOS_validation\0") };

#[derive(Clone, Copy, Debug)]
pub enum VulkanApiVersion {
    V1_0,
    V1_1,
    V1_2,
    V1_3,
}

pub struct InstanceForWindow {
    handle: Arc<ash::Instance>,
    vk_api_version: VulkanApiVersion,
    #[allow(unused)]
    entry: ash::Entry,
    #[allow(unused)]
    debug_worker: Option<(ash::ext::debug_utils::Instance, vk::DebugUtilsMessengerEXT)>,
}

impl InstanceForWindow {
    pub fn new(
        window: Arc<Window>,
        debug_strategy: VulkanDebugInfoStrategy,
        vulkan_api_version: VulkanApiVersion,
    ) -> Self {
        let mut extensions_for_window = ash_window::enumerate_required_extensions(
            window
                .display_handle()
                .expect("Failed to get window handle")
                .as_raw(),
        )
        .expect("Failed to enumerate required vulkan extensions for window app")
        .to_vec();

        extensions_for_window.push(vk::KHR_PORTABILITY_ENUMERATION_NAME.as_ptr());
        extensions_for_window.push(vk::KHR_GET_PHYSICAL_DEVICE_PROPERTIES2_NAME.as_ptr());

        match debug_strategy {
            VulkanDebugInfoStrategy::Idle => {}
            _ => extensions_for_window.push(vk::EXT_DEBUG_UTILS_NAME.as_ptr()),
        }

        let app_info = vk::ApplicationInfo::default().api_version(match vulkan_api_version {
            VulkanApiVersion::V1_0 => vk::API_VERSION_1_0,
            VulkanApiVersion::V1_1 => vk::API_VERSION_1_1,
            VulkanApiVersion::V1_2 => vk::API_VERSION_1_2,
            VulkanApiVersion::V1_3 => vk::API_VERSION_1_3,
        });

        let enabled_layers: Vec<*const c_char> = match debug_strategy {
            VulkanDebugInfoStrategy::Idle => vec![],
            _ => vec![VALIDATION_LAYER_NAME.as_ptr()],
        };

        let instance_create_flags =
            vk::InstanceCreateFlags::default() | vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR;

        let instance_create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(&extensions_for_window)
            .enabled_layer_names(&enabled_layers)
            .flags(instance_create_flags);

        let entry = ash::Entry::linked();
        let instance = unsafe {
            entry
                .create_instance(&instance_create_info, None)
                .expect("Failed to create vulkan instance")
        };

        let debug_worker = match debug_strategy {
            VulkanDebugInfoStrategy::Idle => None,
            VulkanDebugInfoStrategy::PrintAll(p_fn)
            | VulkanDebugInfoStrategy::PanicOnErrorsPrintOthers(p_fn) => {
                let debug_utils_loader = ash::ext::debug_utils::Instance::new(&entry, &instance);
                let messenger_create_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
                    .message_severity(
                        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                            | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                            | vk::DebugUtilsMessageSeverityFlagsEXT::INFO,
                    )
                    .message_type(
                        vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                            | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                            | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
                    )
                    .pfn_user_callback(p_fn);
                let debug_messenger = unsafe {
                    debug_utils_loader
                        .create_debug_utils_messenger(&messenger_create_info, None)
                        .expect("Failed to create debug messenger")
                };
                Some((debug_utils_loader, debug_messenger))
            }
        };

        Self {
            handle: Arc::new(instance),
            vk_api_version: vulkan_api_version,
            entry,
            debug_worker,
        }
    }

    pub fn with_window(window: Arc<Window>) -> Self {
        Self::new(
            window,
            VulkanDebugInfoStrategy::DEFAULT_PANIC_ON_ERRORS,
            VulkanApiVersion::V1_1,
        )
    }

    pub fn api_version(&self) -> VulkanApiVersion {
        self.vk_api_version
    }

    pub fn handle(&self) -> Arc<ash::Instance> {
        self.handle.clone()
    }
}

impl Drop for InstanceForWindow {
    fn drop(&mut self) {
        assert!(
            Arc::strong_count(&self.handle) == 1,
            "Attempted to drop instance while being used by others"
        );
        unsafe {
            if let Some((debug_utils, debug_messenger)) = self.debug_worker.as_ref() {
                debug_utils.destroy_debug_utils_messenger(*debug_messenger, None)
            }
            self.handle.destroy_instance(None);
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum VulkanDebugInfoStrategy {
    Idle,
    PrintAll(vk::PFN_vkDebugUtilsMessengerCallbackEXT),
    PanicOnErrorsPrintOthers(vk::PFN_vkDebugUtilsMessengerCallbackEXT),
}

impl VulkanDebugInfoStrategy {
    pub const DEFAULT_PRINT_ALL: Self = Self::PrintAll(Some(vulkan_debug_callback_print_all));
    pub const DEFAULT_PANIC_ON_ERRORS: Self =
        Self::PanicOnErrorsPrintOthers(Some(vulkan_debug_callback_panic_on_errors_print_others));
}

unsafe extern "system" fn vulkan_debug_callback_print_all(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    let callback_data = *p_callback_data;
    let message_id_number = callback_data.message_id_number;

    let message_id_name = if callback_data.p_message_id_name.is_null() {
        Cow::from("")
    } else {
        CStr::from_ptr(callback_data.p_message_id_name).to_string_lossy()
    };

    let message = if callback_data.p_message.is_null() {
        Cow::from("")
    } else {
        CStr::from_ptr(callback_data.p_message).to_string_lossy()
    };

    println!(
        "{message_severity:?}:\n{message_type:?} [{message_id_name} ({message_id_number})] : {message}\n",
    );

    vk::FALSE
}

unsafe extern "system" fn vulkan_debug_callback_panic_on_errors_print_others(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    let callback_data = *p_callback_data;
    let message_id_number = callback_data.message_id_number;

    let message_id_name = if callback_data.p_message_id_name.is_null() {
        Cow::from("")
    } else {
        CStr::from_ptr(callback_data.p_message_id_name).to_string_lossy()
    };

    let message = if callback_data.p_message.is_null() {
        Cow::from("")
    } else {
        CStr::from_ptr(callback_data.p_message).to_string_lossy()
    };

    if message_severity == vk::DebugUtilsMessageSeverityFlagsEXT::ERROR {
        panic!(
            "{message_severity:?}:\n{message_type:?} [{message_id_name} ({message_id_number})] : {message}\n",
        );
    } else {
        println!(
            "{message_severity:?}:\n{message_type:?} [{message_id_name} ({message_id_number})] : {message}\n",
        );
    }

    vk::FALSE
}
