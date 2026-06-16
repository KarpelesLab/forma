//! Raw Vulkan FFI foundation — no `ash`/`vulkano`, just `libvulkan` and the
//! Vulkan C structs, matching the "close to the OS" policy.
//!
//! This is the entry point for a future GPU-native Vulkan render backend: it
//! creates a `VkInstance` and enumerates the physical devices, returning their
//! names. The full offscreen pipeline (device + queues, image + memory, render
//! pass, SPIR-V pipeline, command buffers, readback) builds on this. It runs
//! headlessly under Mesa's **lavapipe** software Vulkan ICD in CI.

#![allow(unsafe_code, non_snake_case, non_upper_case_globals)]

use core::ffi::{c_char, c_void};

type VkInstance = *mut c_void;
type VkPhysicalDevice = *mut c_void;
type VkResult = i32;

const VK_SUCCESS: VkResult = 0;
const VK_STRUCTURE_TYPE_APPLICATION_INFO: i32 = 0;
const VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO: i32 = 1;
// VK_API_VERSION_1_0 = VK_MAKE_API_VERSION(0, 1, 0, 0) = 1 << 22.
const VK_API_VERSION_1_0: u32 = 1 << 22;
// VkPhysicalDeviceProperties.deviceName follows five u32 fields (apiVersion,
// driverVersion, vendorID, deviceID, deviceType) → byte offset 20, length 256.
const DEVICE_NAME_OFFSET: usize = 20;
const VK_MAX_PHYSICAL_DEVICE_NAME_SIZE: usize = 256;

#[repr(C)]
struct VkApplicationInfo {
    sType: i32,
    pNext: *const c_void,
    pApplicationName: *const c_char,
    applicationVersion: u32,
    pEngineName: *const c_char,
    engineVersion: u32,
    apiVersion: u32,
}

#[repr(C)]
struct VkInstanceCreateInfo {
    sType: i32,
    pNext: *const c_void,
    flags: u32,
    pApplicationInfo: *const VkApplicationInfo,
    enabledLayerCount: u32,
    ppEnabledLayerNames: *const *const c_char,
    enabledExtensionCount: u32,
    ppEnabledExtensionNames: *const *const c_char,
}

#[link(name = "vulkan")]
unsafe extern "C" {
    fn vkCreateInstance(
        pCreateInfo: *const VkInstanceCreateInfo,
        pAllocator: *const c_void,
        pInstance: *mut VkInstance,
    ) -> VkResult;
    fn vkDestroyInstance(instance: VkInstance, pAllocator: *const c_void);
    fn vkEnumeratePhysicalDevices(
        instance: VkInstance,
        pPhysicalDeviceCount: *mut u32,
        pPhysicalDevices: *mut VkPhysicalDevice,
    ) -> VkResult;
    // Writes a VkPhysicalDeviceProperties (~824 bytes); we hand it a generous
    // byte buffer and read only `deviceName` out of it.
    fn vkGetPhysicalDeviceProperties(physicalDevice: VkPhysicalDevice, pProperties: *mut c_void);
}

/// Create a Vulkan instance and return the name of each physical device the
/// loader exposes (e.g. `"llvmpipe (LLVM ...)"` under lavapipe). Errors if no
/// Vulkan loader/ICD is reachable.
pub fn devices() -> Result<Vec<String>, String> {
    unsafe {
        let app = VkApplicationInfo {
            sType: VK_STRUCTURE_TYPE_APPLICATION_INFO,
            pNext: core::ptr::null(),
            pApplicationName: c"forma".as_ptr(),
            applicationVersion: 0,
            pEngineName: c"forma".as_ptr(),
            engineVersion: 0,
            apiVersion: VK_API_VERSION_1_0,
        };
        let create = VkInstanceCreateInfo {
            sType: VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO,
            pNext: core::ptr::null(),
            flags: 0,
            pApplicationInfo: &app,
            enabledLayerCount: 0,
            ppEnabledLayerNames: core::ptr::null(),
            enabledExtensionCount: 0,
            ppEnabledExtensionNames: core::ptr::null(),
        };
        let mut instance: VkInstance = core::ptr::null_mut();
        let r = vkCreateInstance(&create, core::ptr::null(), &mut instance);
        if r != VK_SUCCESS {
            return Err(format!("vkCreateInstance failed ({r})"));
        }

        let mut count: u32 = 0;
        vkEnumeratePhysicalDevices(instance, &mut count, core::ptr::null_mut());
        let mut handles: Vec<VkPhysicalDevice> = vec![core::ptr::null_mut(); count as usize];
        if count > 0 {
            vkEnumeratePhysicalDevices(instance, &mut count, handles.as_mut_ptr());
        }

        let mut names = Vec::with_capacity(handles.len());
        for dev in handles {
            // A 2 KiB buffer comfortably exceeds sizeof(VkPhysicalDeviceProperties).
            let mut props = vec![0u8; 2048];
            vkGetPhysicalDeviceProperties(dev, props.as_mut_ptr() as *mut c_void);
            let name =
                &props[DEVICE_NAME_OFFSET..DEVICE_NAME_OFFSET + VK_MAX_PHYSICAL_DEVICE_NAME_SIZE];
            let end = name.iter().position(|&b| b == 0).unwrap_or(name.len());
            names.push(String::from_utf8_lossy(&name[..end]).into_owned());
        }

        vkDestroyInstance(instance, core::ptr::null());
        Ok(names)
    }
}
