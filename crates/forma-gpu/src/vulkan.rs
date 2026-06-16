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

type VkDevice = *mut c_void;
type VkQueue = *mut c_void;

const VK_SUCCESS: VkResult = 0;
const VK_STRUCTURE_TYPE_APPLICATION_INFO: i32 = 0;
const VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO: i32 = 1;
const VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO: i32 = 2;
const VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO: i32 = 3;
const VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO: i32 = 5;
const VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO: i32 = 14;
const VK_QUEUE_GRAPHICS_BIT: u32 = 0x1;
// VkQueueFamilyProperties is 24 bytes; queueFlags is its first u32.
const QUEUE_FAMILY_PROPS_SIZE: usize = 24;
const VK_FORMAT_R8G8B8A8_UNORM: u32 = 37;
const VK_IMAGE_TYPE_2D: u32 = 1;
const VK_IMAGE_TILING_OPTIMAL: u32 = 0;
const VK_IMAGE_USAGE_TRANSFER_SRC_BIT: u32 = 0x1;
const VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT: u32 = 0x10;
const VK_SHARING_MODE_EXCLUSIVE: u32 = 0;
const VK_IMAGE_LAYOUT_UNDEFINED: u32 = 0;
const VK_SAMPLE_COUNT_1_BIT: u32 = 1;
const VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT: u32 = 0x1;
// VkPhysicalDeviceMemoryProperties: memoryTypeCount (u32) then memoryTypes[32]
// of VkMemoryType { propertyFlags: u32, heapIndex: u32 } (8 bytes each).
const MEM_TYPES_OFFSET: usize = 4;
const MEM_TYPE_STRIDE: usize = 8;

const VK_STRUCTURE_TYPE_IMAGE_VIEW_CREATE_INFO: i32 = 15;
const VK_STRUCTURE_TYPE_FRAMEBUFFER_CREATE_INFO: i32 = 37;
const VK_STRUCTURE_TYPE_RENDER_PASS_CREATE_INFO: i32 = 38;
const VK_IMAGE_VIEW_TYPE_2D: u32 = 1;
const VK_IMAGE_ASPECT_COLOR_BIT: u32 = 0x1;
const VK_ATTACHMENT_LOAD_OP_CLEAR: u32 = 1;
const VK_ATTACHMENT_STORE_OP_STORE: u32 = 0;
const VK_ATTACHMENT_LOAD_OP_DONT_CARE: u32 = 2;
const VK_ATTACHMENT_STORE_OP_DONT_CARE: u32 = 1;
const VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL: u32 = 2;
const VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL: u32 = 6;
const VK_PIPELINE_BIND_POINT_GRAPHICS: u32 = 0;
const VK_COMPONENT_SWIZZLE_IDENTITY: u32 = 0;
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

#[repr(C)]
struct VkDeviceQueueCreateInfo {
    sType: i32,
    pNext: *const c_void,
    flags: u32,
    queueFamilyIndex: u32,
    queueCount: u32,
    pQueuePriorities: *const f32,
}

#[repr(C)]
struct VkDeviceCreateInfo {
    sType: i32,
    pNext: *const c_void,
    flags: u32,
    queueCreateInfoCount: u32,
    pQueueCreateInfos: *const VkDeviceQueueCreateInfo,
    enabledLayerCount: u32,
    ppEnabledLayerNames: *const *const c_char,
    enabledExtensionCount: u32,
    ppEnabledExtensionNames: *const *const c_char,
    pEnabledFeatures: *const c_void,
}

type VkImage = u64;
type VkDeviceMemory = u64;
type VkImageView = u64;
type VkRenderPass = u64;
type VkFramebuffer = u64;

#[repr(C)]
struct VkExtent3D {
    width: u32,
    height: u32,
    depth: u32,
}

#[repr(C)]
struct VkImageCreateInfo {
    sType: i32,
    pNext: *const c_void,
    flags: u32,
    imageType: u32,
    format: u32,
    extent: VkExtent3D,
    mipLevels: u32,
    arrayLayers: u32,
    samples: u32,
    tiling: u32,
    usage: u32,
    sharingMode: u32,
    queueFamilyIndexCount: u32,
    pQueueFamilyIndices: *const u32,
    initialLayout: u32,
}

#[repr(C)]
struct VkMemoryRequirements {
    size: u64,
    alignment: u64,
    memoryTypeBits: u32,
}

#[repr(C)]
struct VkMemoryAllocateInfo {
    sType: i32,
    pNext: *const c_void,
    allocationSize: u64,
    memoryTypeIndex: u32,
}

#[repr(C)]
struct VkComponentMapping {
    r: u32,
    g: u32,
    b: u32,
    a: u32,
}

#[repr(C)]
struct VkImageSubresourceRange {
    aspectMask: u32,
    baseMipLevel: u32,
    levelCount: u32,
    baseArrayLayer: u32,
    layerCount: u32,
}

#[repr(C)]
struct VkImageViewCreateInfo {
    sType: i32,
    pNext: *const c_void,
    flags: u32,
    image: VkImage,
    viewType: u32,
    format: u32,
    components: VkComponentMapping,
    subresourceRange: VkImageSubresourceRange,
}

#[repr(C)]
struct VkAttachmentDescription {
    flags: u32,
    format: u32,
    samples: u32,
    loadOp: u32,
    storeOp: u32,
    stencilLoadOp: u32,
    stencilStoreOp: u32,
    initialLayout: u32,
    finalLayout: u32,
}

#[repr(C)]
struct VkAttachmentReference {
    attachment: u32,
    layout: u32,
}

#[repr(C)]
struct VkSubpassDescription {
    flags: u32,
    pipelineBindPoint: u32,
    inputAttachmentCount: u32,
    pInputAttachments: *const VkAttachmentReference,
    colorAttachmentCount: u32,
    pColorAttachments: *const VkAttachmentReference,
    pResolveAttachments: *const VkAttachmentReference,
    pDepthStencilAttachment: *const VkAttachmentReference,
    preserveAttachmentCount: u32,
    pPreserveAttachments: *const u32,
}

#[repr(C)]
struct VkRenderPassCreateInfo {
    sType: i32,
    pNext: *const c_void,
    flags: u32,
    attachmentCount: u32,
    pAttachments: *const VkAttachmentDescription,
    subpassCount: u32,
    pSubpasses: *const VkSubpassDescription,
    dependencyCount: u32,
    pDependencies: *const c_void,
}

#[repr(C)]
struct VkFramebufferCreateInfo {
    sType: i32,
    pNext: *const c_void,
    flags: u32,
    renderPass: VkRenderPass,
    attachmentCount: u32,
    pAttachments: *const VkImageView,
    width: u32,
    height: u32,
    layers: u32,
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
    fn vkGetPhysicalDeviceQueueFamilyProperties(
        physicalDevice: VkPhysicalDevice,
        pQueueFamilyPropertyCount: *mut u32,
        pQueueFamilyProperties: *mut c_void,
    );
    fn vkCreateDevice(
        physicalDevice: VkPhysicalDevice,
        pCreateInfo: *const VkDeviceCreateInfo,
        pAllocator: *const c_void,
        pDevice: *mut VkDevice,
    ) -> VkResult;
    fn vkDestroyDevice(device: VkDevice, pAllocator: *const c_void);
    fn vkGetDeviceQueue(
        device: VkDevice,
        queueFamilyIndex: u32,
        queueIndex: u32,
        pQueue: *mut VkQueue,
    );
    fn vkCreateImage(
        device: VkDevice,
        pCreateInfo: *const VkImageCreateInfo,
        pAllocator: *const c_void,
        pImage: *mut VkImage,
    ) -> VkResult;
    fn vkDestroyImage(device: VkDevice, image: VkImage, pAllocator: *const c_void);
    fn vkGetImageMemoryRequirements(
        device: VkDevice,
        image: VkImage,
        pMemoryRequirements: *mut VkMemoryRequirements,
    );
    // Writes a VkPhysicalDeviceMemoryProperties (~520 bytes); we hand it a
    // generous byte buffer and read the memoryType property flags out of it.
    fn vkGetPhysicalDeviceMemoryProperties(
        physicalDevice: VkPhysicalDevice,
        pMemoryProperties: *mut c_void,
    );
    fn vkAllocateMemory(
        device: VkDevice,
        pAllocateInfo: *const VkMemoryAllocateInfo,
        pAllocator: *const c_void,
        pMemory: *mut VkDeviceMemory,
    ) -> VkResult;
    fn vkFreeMemory(device: VkDevice, memory: VkDeviceMemory, pAllocator: *const c_void);
    fn vkBindImageMemory(
        device: VkDevice,
        image: VkImage,
        memory: VkDeviceMemory,
        memoryOffset: u64,
    ) -> VkResult;
    fn vkCreateImageView(
        device: VkDevice,
        pCreateInfo: *const VkImageViewCreateInfo,
        pAllocator: *const c_void,
        pView: *mut VkImageView,
    ) -> VkResult;
    fn vkDestroyImageView(device: VkDevice, imageView: VkImageView, pAllocator: *const c_void);
    fn vkCreateRenderPass(
        device: VkDevice,
        pCreateInfo: *const VkRenderPassCreateInfo,
        pAllocator: *const c_void,
        pRenderPass: *mut VkRenderPass,
    ) -> VkResult;
    fn vkDestroyRenderPass(device: VkDevice, renderPass: VkRenderPass, pAllocator: *const c_void);
    fn vkCreateFramebuffer(
        device: VkDevice,
        pCreateInfo: *const VkFramebufferCreateInfo,
        pAllocator: *const c_void,
        pFramebuffer: *mut VkFramebuffer,
    ) -> VkResult;
    fn vkDestroyFramebuffer(
        device: VkDevice,
        framebuffer: VkFramebuffer,
        pAllocator: *const c_void,
    );
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

/// A live Vulkan instance + logical device on the first physical device, with a
/// graphics queue family selected. Owns the raw handles; callers destroy them
/// via [`Gpu::destroy`] once done (we keep this manual rather than wiring `Drop`,
/// to keep the FFI surface explicit).
struct Gpu {
    instance: VkInstance,
    phys: VkPhysicalDevice,
    device: VkDevice,
    family: u32,
}

impl Gpu {
    /// Create an instance, pick the first physical device, find a graphics queue
    /// family, and create a logical device with one queue from it.
    unsafe fn create() -> Result<Gpu, String> {
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
            if vkCreateInstance(&create, core::ptr::null(), &mut instance) != VK_SUCCESS {
                return Err("vkCreateInstance failed".into());
            }

            let mut count: u32 = 0;
            vkEnumeratePhysicalDevices(instance, &mut count, core::ptr::null_mut());
            if count == 0 {
                vkDestroyInstance(instance, core::ptr::null());
                return Err("no Vulkan physical devices".into());
            }
            let mut handles: Vec<VkPhysicalDevice> = vec![core::ptr::null_mut(); count as usize];
            vkEnumeratePhysicalDevices(instance, &mut count, handles.as_mut_ptr());
            let phys = handles[0];

            // Find a queue family that supports graphics.
            let mut qcount: u32 = 0;
            vkGetPhysicalDeviceQueueFamilyProperties(phys, &mut qcount, core::ptr::null_mut());
            let mut qbuf = vec![0u8; qcount as usize * QUEUE_FAMILY_PROPS_SIZE];
            vkGetPhysicalDeviceQueueFamilyProperties(
                phys,
                &mut qcount,
                qbuf.as_mut_ptr() as *mut c_void,
            );
            let mut family: Option<u32> = None;
            for i in 0..qcount as usize {
                let off = i * QUEUE_FAMILY_PROPS_SIZE;
                let flags =
                    u32::from_ne_bytes([qbuf[off], qbuf[off + 1], qbuf[off + 2], qbuf[off + 3]]);
                if flags & VK_QUEUE_GRAPHICS_BIT != 0 {
                    family = Some(i as u32);
                    break;
                }
            }
            let Some(family) = family else {
                vkDestroyInstance(instance, core::ptr::null());
                return Err("no graphics queue family".into());
            };

            // Create a logical device with one queue from that family.
            let priority: f32 = 1.0;
            let qci = VkDeviceQueueCreateInfo {
                sType: VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO,
                pNext: core::ptr::null(),
                flags: 0,
                queueFamilyIndex: family,
                queueCount: 1,
                pQueuePriorities: &priority,
            };
            let dci = VkDeviceCreateInfo {
                sType: VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO,
                pNext: core::ptr::null(),
                flags: 0,
                queueCreateInfoCount: 1,
                pQueueCreateInfos: &qci,
                enabledLayerCount: 0,
                ppEnabledLayerNames: core::ptr::null(),
                enabledExtensionCount: 0,
                ppEnabledExtensionNames: core::ptr::null(),
                pEnabledFeatures: core::ptr::null(),
            };
            let mut device: VkDevice = core::ptr::null_mut();
            if vkCreateDevice(phys, &dci, core::ptr::null(), &mut device) != VK_SUCCESS {
                vkDestroyInstance(instance, core::ptr::null());
                return Err("vkCreateDevice failed".into());
            }
            Ok(Gpu {
                instance,
                phys,
                device,
                family,
            })
        }
    }

    /// Create a `width`×`height` `R8G8B8A8_UNORM` color-attachment image and back
    /// it with a `DEVICE_LOCAL` memory allocation. Returns `(image, memory,
    /// size)`; the caller frees both with `vkDestroyImage` + `vkFreeMemory`.
    unsafe fn create_color_image(
        &self,
        width: u32,
        height: u32,
    ) -> Result<(VkImage, VkDeviceMemory, u64), String> {
        unsafe {
            let ici = VkImageCreateInfo {
                sType: VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO,
                pNext: core::ptr::null(),
                flags: 0,
                imageType: VK_IMAGE_TYPE_2D,
                format: VK_FORMAT_R8G8B8A8_UNORM,
                extent: VkExtent3D {
                    width,
                    height,
                    depth: 1,
                },
                mipLevels: 1,
                arrayLayers: 1,
                samples: VK_SAMPLE_COUNT_1_BIT,
                tiling: VK_IMAGE_TILING_OPTIMAL,
                usage: VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT | VK_IMAGE_USAGE_TRANSFER_SRC_BIT,
                sharingMode: VK_SHARING_MODE_EXCLUSIVE,
                queueFamilyIndexCount: 0,
                pQueueFamilyIndices: core::ptr::null(),
                initialLayout: VK_IMAGE_LAYOUT_UNDEFINED,
            };
            let mut image: VkImage = 0;
            if vkCreateImage(self.device, &ici, core::ptr::null(), &mut image) != VK_SUCCESS {
                return Err("vkCreateImage failed".into());
            }

            let mut req = VkMemoryRequirements {
                size: 0,
                alignment: 0,
                memoryTypeBits: 0,
            };
            vkGetImageMemoryRequirements(self.device, image, &mut req);

            // Read the device's memory types and pick the first DEVICE_LOCAL one
            // permitted by the image's memoryTypeBits mask.
            let mut memprops = vec![0u8; 1024];
            vkGetPhysicalDeviceMemoryProperties(self.phys, memprops.as_mut_ptr() as *mut c_void);
            let type_count =
                u32::from_ne_bytes([memprops[0], memprops[1], memprops[2], memprops[3]]);
            let mut chosen: Option<u32> = None;
            for i in 0..type_count as usize {
                if req.memoryTypeBits & (1 << i) == 0 {
                    continue;
                }
                let off = MEM_TYPES_OFFSET + i * MEM_TYPE_STRIDE;
                let flags = u32::from_ne_bytes([
                    memprops[off],
                    memprops[off + 1],
                    memprops[off + 2],
                    memprops[off + 3],
                ]);
                if flags & VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT != 0 {
                    chosen = Some(i as u32);
                    break;
                }
            }
            let Some(mem_type) = chosen else {
                vkDestroyImage(self.device, image, core::ptr::null());
                return Err("no DEVICE_LOCAL memory type for the image".into());
            };

            let mai = VkMemoryAllocateInfo {
                sType: VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO,
                pNext: core::ptr::null(),
                allocationSize: req.size,
                memoryTypeIndex: mem_type,
            };
            let mut memory: VkDeviceMemory = 0;
            if vkAllocateMemory(self.device, &mai, core::ptr::null(), &mut memory) != VK_SUCCESS {
                vkDestroyImage(self.device, image, core::ptr::null());
                return Err("vkAllocateMemory failed".into());
            }
            if vkBindImageMemory(self.device, image, memory, 0) != VK_SUCCESS {
                vkFreeMemory(self.device, memory, core::ptr::null());
                vkDestroyImage(self.device, image, core::ptr::null());
                return Err("vkBindImageMemory failed".into());
            }
            Ok((image, memory, req.size))
        }
    }

    unsafe fn destroy(self) {
        unsafe {
            vkDestroyDevice(self.device, core::ptr::null());
            vkDestroyInstance(self.instance, core::ptr::null());
        }
    }
}

/// Create an instance, pick the first physical device, select a graphics queue
/// family, and create a logical device with that queue — the core a Vulkan
/// render backend builds on. Returns a one-line summary. Errors if any step
/// fails or no graphics-capable device exists.
pub fn init_device() -> Result<String, String> {
    unsafe {
        let gpu = Gpu::create()?;
        let mut queue: VkQueue = core::ptr::null_mut();
        vkGetDeviceQueue(gpu.device, gpu.family, 0, &mut queue);
        let ok = !queue.is_null();
        let family = gpu.family;
        gpu.destroy();
        if ok {
            Ok(format!(
                "logical device + graphics queue (family {family}) created"
            ))
        } else {
            Err("vkGetDeviceQueue returned null".into())
        }
    }
}

/// Allocate a GPU-resident render target: create a `width`×`height`
/// `R8G8B8A8_UNORM` color-attachment [`VkImage`], query its memory
/// requirements, choose a `DEVICE_LOCAL` memory type, and back it with a
/// [`VkDeviceMemory`] allocation — the offscreen target a Vulkan render pass
/// draws into. Returns a one-line summary (size + chosen memory type). Errors if
/// any step fails.
pub fn init_image(width: u32, height: u32) -> Result<String, String> {
    unsafe {
        let gpu = Gpu::create()?;
        let r = gpu.create_color_image(width, height);
        match r {
            Ok((image, memory, bytes)) => {
                vkFreeMemory(gpu.device, memory, core::ptr::null());
                vkDestroyImage(gpu.device, image, core::ptr::null());
                gpu.destroy();
                Ok(format!(
                    "{width}x{height} color image bound to {bytes}B of DEVICE_LOCAL memory"
                ))
            }
            Err(e) => {
                gpu.destroy();
                Err(e)
            }
        }
    }
}

/// Build the render-target objects a Vulkan render pass needs: a [`VkImageView`]
/// over a freshly-allocated color image, a single-attachment [`VkRenderPass`]
/// (clear on load, store on done, ending in `TRANSFER_SRC_OPTIMAL` ready for
/// readback), and a [`VkFramebuffer`] binding the view to the pass. Returns a
/// one-line summary. Errors if any step fails.
pub fn init_framebuffer(width: u32, height: u32) -> Result<String, String> {
    unsafe {
        let gpu = Gpu::create()?;
        let (image, memory, _bytes) = match gpu.create_color_image(width, height) {
            Ok(t) => t,
            Err(e) => {
                gpu.destroy();
                return Err(e);
            }
        };

        // A view over the whole color image.
        let view_ci = VkImageViewCreateInfo {
            sType: VK_STRUCTURE_TYPE_IMAGE_VIEW_CREATE_INFO,
            pNext: core::ptr::null(),
            flags: 0,
            image,
            viewType: VK_IMAGE_VIEW_TYPE_2D,
            format: VK_FORMAT_R8G8B8A8_UNORM,
            components: VkComponentMapping {
                r: VK_COMPONENT_SWIZZLE_IDENTITY,
                g: VK_COMPONENT_SWIZZLE_IDENTITY,
                b: VK_COMPONENT_SWIZZLE_IDENTITY,
                a: VK_COMPONENT_SWIZZLE_IDENTITY,
            },
            subresourceRange: VkImageSubresourceRange {
                aspectMask: VK_IMAGE_ASPECT_COLOR_BIT,
                baseMipLevel: 0,
                levelCount: 1,
                baseArrayLayer: 0,
                layerCount: 1,
            },
        };
        let mut view: VkImageView = 0;
        let cleanup_img = |gpu: Gpu| {
            vkFreeMemory(gpu.device, memory, core::ptr::null());
            vkDestroyImage(gpu.device, image, core::ptr::null());
            gpu.destroy();
        };
        if vkCreateImageView(gpu.device, &view_ci, core::ptr::null(), &mut view) != VK_SUCCESS {
            cleanup_img(gpu);
            return Err("vkCreateImageView failed".into());
        }

        // A render pass with one color attachment: clear → store, finishing in a
        // transfer-source layout so the result can be copied back to the host.
        let attachment = VkAttachmentDescription {
            flags: 0,
            format: VK_FORMAT_R8G8B8A8_UNORM,
            samples: VK_SAMPLE_COUNT_1_BIT,
            loadOp: VK_ATTACHMENT_LOAD_OP_CLEAR,
            storeOp: VK_ATTACHMENT_STORE_OP_STORE,
            stencilLoadOp: VK_ATTACHMENT_LOAD_OP_DONT_CARE,
            stencilStoreOp: VK_ATTACHMENT_STORE_OP_DONT_CARE,
            initialLayout: VK_IMAGE_LAYOUT_UNDEFINED,
            finalLayout: VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL,
        };
        let color_ref = VkAttachmentReference {
            attachment: 0,
            layout: VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL,
        };
        let subpass = VkSubpassDescription {
            flags: 0,
            pipelineBindPoint: VK_PIPELINE_BIND_POINT_GRAPHICS,
            inputAttachmentCount: 0,
            pInputAttachments: core::ptr::null(),
            colorAttachmentCount: 1,
            pColorAttachments: &color_ref,
            pResolveAttachments: core::ptr::null(),
            pDepthStencilAttachment: core::ptr::null(),
            preserveAttachmentCount: 0,
            pPreserveAttachments: core::ptr::null(),
        };
        let rp_ci = VkRenderPassCreateInfo {
            sType: VK_STRUCTURE_TYPE_RENDER_PASS_CREATE_INFO,
            pNext: core::ptr::null(),
            flags: 0,
            attachmentCount: 1,
            pAttachments: &attachment,
            subpassCount: 1,
            pSubpasses: &subpass,
            dependencyCount: 0,
            pDependencies: core::ptr::null(),
        };
        let mut render_pass: VkRenderPass = 0;
        if vkCreateRenderPass(gpu.device, &rp_ci, core::ptr::null(), &mut render_pass) != VK_SUCCESS
        {
            vkDestroyImageView(gpu.device, view, core::ptr::null());
            cleanup_img(gpu);
            return Err("vkCreateRenderPass failed".into());
        }

        // The framebuffer binds the view to the render pass at the target size.
        let fb_ci = VkFramebufferCreateInfo {
            sType: VK_STRUCTURE_TYPE_FRAMEBUFFER_CREATE_INFO,
            pNext: core::ptr::null(),
            flags: 0,
            renderPass: render_pass,
            attachmentCount: 1,
            pAttachments: &view,
            width,
            height,
            layers: 1,
        };
        let mut framebuffer: VkFramebuffer = 0;
        let ok = vkCreateFramebuffer(gpu.device, &fb_ci, core::ptr::null(), &mut framebuffer)
            == VK_SUCCESS;

        if ok {
            vkDestroyFramebuffer(gpu.device, framebuffer, core::ptr::null());
        }
        vkDestroyRenderPass(gpu.device, render_pass, core::ptr::null());
        vkDestroyImageView(gpu.device, view, core::ptr::null());
        cleanup_img(gpu);

        if ok {
            Ok(format!(
                "{width}x{height} framebuffer + render pass + image view created"
            ))
        } else {
            Err("vkCreateFramebuffer failed".into())
        }
    }
}
