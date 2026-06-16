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

const VK_STRUCTURE_TYPE_SUBMIT_INFO: i32 = 4;
const VK_STRUCTURE_TYPE_FENCE_CREATE_INFO: i32 = 8;
const VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO: i32 = 39;
const VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO: i32 = 40;
const VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO: i32 = 42;
const VK_STRUCTURE_TYPE_RENDER_PASS_BEGIN_INFO: i32 = 43;
const VK_COMMAND_BUFFER_LEVEL_PRIMARY: u32 = 0;
const VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT: u32 = 0x1;
const VK_SUBPASS_CONTENTS_INLINE: u32 = 0;
const VK_TRUE: u32 = 1;

const VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO: i32 = 12;
const VK_BUFFER_USAGE_TRANSFER_DST_BIT: u32 = 0x2;
const VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT: u32 = 0x2;
const VK_MEMORY_PROPERTY_HOST_COHERENT_BIT: u32 = 0x4;
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
type VkCommandPool = u64;
type VkFence = u64;
type VkBuffer = u64;
// VkCommandBuffer is a *dispatchable* handle (pointer-sized), not a u64 like the
// non-dispatchable handles above.
type VkCommandBuffer = *mut c_void;

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

#[repr(C)]
struct VkCommandPoolCreateInfo {
    sType: i32,
    pNext: *const c_void,
    flags: u32,
    queueFamilyIndex: u32,
}

#[repr(C)]
struct VkCommandBufferAllocateInfo {
    sType: i32,
    pNext: *const c_void,
    commandPool: VkCommandPool,
    level: u32,
    commandBufferCount: u32,
}

#[repr(C)]
struct VkCommandBufferBeginInfo {
    sType: i32,
    pNext: *const c_void,
    flags: u32,
    pInheritanceInfo: *const c_void,
}

#[repr(C)]
struct VkOffset2D {
    x: i32,
    y: i32,
}

#[repr(C)]
struct VkExtent2D {
    width: u32,
    height: u32,
}

#[repr(C)]
struct VkRect2D {
    offset: VkOffset2D,
    extent: VkExtent2D,
}

/// `VkClearValue` is a union; for a color attachment its largest relevant member
/// is `VkClearColorValue { float32: [f32; 4] }` (16 bytes), which is what we use.
#[repr(C)]
struct VkClearValue {
    float32: [f32; 4],
}

#[repr(C)]
struct VkRenderPassBeginInfo {
    sType: i32,
    pNext: *const c_void,
    renderPass: VkRenderPass,
    framebuffer: VkFramebuffer,
    renderArea: VkRect2D,
    clearValueCount: u32,
    pClearValues: *const VkClearValue,
}

#[repr(C)]
struct VkSubmitInfo {
    sType: i32,
    pNext: *const c_void,
    waitSemaphoreCount: u32,
    pWaitSemaphores: *const u64,
    pWaitDstStageMask: *const u32,
    commandBufferCount: u32,
    pCommandBuffers: *const VkCommandBuffer,
    signalSemaphoreCount: u32,
    pSignalSemaphores: *const u64,
}

#[repr(C)]
struct VkFenceCreateInfo {
    sType: i32,
    pNext: *const c_void,
    flags: u32,
}

#[repr(C)]
struct VkBufferCreateInfo {
    sType: i32,
    pNext: *const c_void,
    flags: u32,
    size: u64,
    usage: u32,
    sharingMode: u32,
    queueFamilyIndexCount: u32,
    pQueueFamilyIndices: *const u32,
}

#[repr(C)]
struct VkImageSubresourceLayers {
    aspectMask: u32,
    mipLevel: u32,
    baseArrayLayer: u32,
    layerCount: u32,
}

#[repr(C)]
struct VkOffset3D {
    x: i32,
    y: i32,
    z: i32,
}

#[repr(C)]
struct VkBufferImageCopy {
    bufferOffset: u64,
    bufferRowLength: u32,
    bufferImageHeight: u32,
    imageSubresource: VkImageSubresourceLayers,
    imageOffset: VkOffset3D,
    imageExtent: VkExtent3D,
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
    fn vkCreateCommandPool(
        device: VkDevice,
        pCreateInfo: *const VkCommandPoolCreateInfo,
        pAllocator: *const c_void,
        pCommandPool: *mut VkCommandPool,
    ) -> VkResult;
    fn vkDestroyCommandPool(
        device: VkDevice,
        commandPool: VkCommandPool,
        pAllocator: *const c_void,
    );
    fn vkAllocateCommandBuffers(
        device: VkDevice,
        pAllocateInfo: *const VkCommandBufferAllocateInfo,
        pCommandBuffers: *mut VkCommandBuffer,
    ) -> VkResult;
    fn vkBeginCommandBuffer(
        commandBuffer: VkCommandBuffer,
        pBeginInfo: *const VkCommandBufferBeginInfo,
    ) -> VkResult;
    fn vkEndCommandBuffer(commandBuffer: VkCommandBuffer) -> VkResult;
    fn vkCmdBeginRenderPass(
        commandBuffer: VkCommandBuffer,
        pRenderPassBegin: *const VkRenderPassBeginInfo,
        contents: u32,
    );
    fn vkCmdEndRenderPass(commandBuffer: VkCommandBuffer);
    fn vkQueueSubmit(
        queue: VkQueue,
        submitCount: u32,
        pSubmits: *const VkSubmitInfo,
        fence: VkFence,
    ) -> VkResult;
    fn vkCreateFence(
        device: VkDevice,
        pCreateInfo: *const VkFenceCreateInfo,
        pAllocator: *const c_void,
        pFence: *mut VkFence,
    ) -> VkResult;
    fn vkDestroyFence(device: VkDevice, fence: VkFence, pAllocator: *const c_void);
    fn vkWaitForFences(
        device: VkDevice,
        fenceCount: u32,
        pFences: *const VkFence,
        waitAll: u32,
        timeout: u64,
    ) -> VkResult;
    fn vkCreateBuffer(
        device: VkDevice,
        pCreateInfo: *const VkBufferCreateInfo,
        pAllocator: *const c_void,
        pBuffer: *mut VkBuffer,
    ) -> VkResult;
    fn vkDestroyBuffer(device: VkDevice, buffer: VkBuffer, pAllocator: *const c_void);
    fn vkGetBufferMemoryRequirements(
        device: VkDevice,
        buffer: VkBuffer,
        pMemoryRequirements: *mut VkMemoryRequirements,
    );
    fn vkBindBufferMemory(
        device: VkDevice,
        buffer: VkBuffer,
        memory: VkDeviceMemory,
        memoryOffset: u64,
    ) -> VkResult;
    fn vkCmdCopyImageToBuffer(
        commandBuffer: VkCommandBuffer,
        srcImage: VkImage,
        srcImageLayout: u32,
        dstBuffer: VkBuffer,
        regionCount: u32,
        pRegions: *const VkBufferImageCopy,
    );
    fn vkMapMemory(
        device: VkDevice,
        memory: VkDeviceMemory,
        offset: u64,
        size: u64,
        flags: u32,
        ppData: *mut *mut c_void,
    ) -> VkResult;
    fn vkUnmapMemory(device: VkDevice, memory: VkDeviceMemory);
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

    /// Build a complete offscreen render target: a color image + memory, an
    /// image view, a single-attachment render pass (clear→store, ending
    /// transfer-readable), and a framebuffer binding them. Frees what it already
    /// created on any failure.
    unsafe fn create_target(&self, width: u32, height: u32) -> Result<Target, String> {
        unsafe {
            let (image, memory, _bytes) = self.create_color_image(width, height)?;

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
            if vkCreateImageView(self.device, &view_ci, core::ptr::null(), &mut view) != VK_SUCCESS
            {
                vkFreeMemory(self.device, memory, core::ptr::null());
                vkDestroyImage(self.device, image, core::ptr::null());
                return Err("vkCreateImageView failed".into());
            }

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
            if vkCreateRenderPass(self.device, &rp_ci, core::ptr::null(), &mut render_pass)
                != VK_SUCCESS
            {
                vkDestroyImageView(self.device, view, core::ptr::null());
                vkFreeMemory(self.device, memory, core::ptr::null());
                vkDestroyImage(self.device, image, core::ptr::null());
                return Err("vkCreateRenderPass failed".into());
            }

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
            if vkCreateFramebuffer(self.device, &fb_ci, core::ptr::null(), &mut framebuffer)
                != VK_SUCCESS
            {
                vkDestroyRenderPass(self.device, render_pass, core::ptr::null());
                vkDestroyImageView(self.device, view, core::ptr::null());
                vkFreeMemory(self.device, memory, core::ptr::null());
                vkDestroyImage(self.device, image, core::ptr::null());
                return Err("vkCreateFramebuffer failed".into());
            }

            Ok(Target {
                image,
                memory,
                view,
                render_pass,
                framebuffer,
                width,
                height,
            })
        }
    }

    /// Create a `size`-byte buffer usable as a transfer destination, backed by
    /// `HOST_VISIBLE | HOST_COHERENT` memory so the CPU can map and read it.
    /// Returns `(buffer, memory)`; the caller frees both.
    unsafe fn create_host_buffer(&self, size: u64) -> Result<(VkBuffer, VkDeviceMemory), String> {
        unsafe {
            let bci = VkBufferCreateInfo {
                sType: VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO,
                pNext: core::ptr::null(),
                flags: 0,
                size,
                usage: VK_BUFFER_USAGE_TRANSFER_DST_BIT,
                sharingMode: VK_SHARING_MODE_EXCLUSIVE,
                queueFamilyIndexCount: 0,
                pQueueFamilyIndices: core::ptr::null(),
            };
            let mut buffer: VkBuffer = 0;
            if vkCreateBuffer(self.device, &bci, core::ptr::null(), &mut buffer) != VK_SUCCESS {
                return Err("vkCreateBuffer failed".into());
            }

            let mut req = VkMemoryRequirements {
                size: 0,
                alignment: 0,
                memoryTypeBits: 0,
            };
            vkGetBufferMemoryRequirements(self.device, buffer, &mut req);

            let want = VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT | VK_MEMORY_PROPERTY_HOST_COHERENT_BIT;
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
                if flags & want == want {
                    chosen = Some(i as u32);
                    break;
                }
            }
            let Some(mem_type) = chosen else {
                vkDestroyBuffer(self.device, buffer, core::ptr::null());
                return Err("no HOST_VISIBLE|COHERENT memory type for the buffer".into());
            };

            let mai = VkMemoryAllocateInfo {
                sType: VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO,
                pNext: core::ptr::null(),
                allocationSize: req.size,
                memoryTypeIndex: mem_type,
            };
            let mut memory: VkDeviceMemory = 0;
            if vkAllocateMemory(self.device, &mai, core::ptr::null(), &mut memory) != VK_SUCCESS {
                vkDestroyBuffer(self.device, buffer, core::ptr::null());
                return Err("vkAllocateMemory (host buffer) failed".into());
            }
            if vkBindBufferMemory(self.device, buffer, memory, 0) != VK_SUCCESS {
                vkFreeMemory(self.device, memory, core::ptr::null());
                vkDestroyBuffer(self.device, buffer, core::ptr::null());
                return Err("vkBindBufferMemory failed".into());
            }
            Ok((buffer, memory))
        }
    }

    unsafe fn destroy(self) {
        unsafe {
            vkDestroyDevice(self.device, core::ptr::null());
            vkDestroyInstance(self.instance, core::ptr::null());
        }
    }
}

/// A complete offscreen render target owned alongside a [`Gpu`]: the color image
/// and its backing memory, a view, a render pass, and a framebuffer. Destroyed
/// explicitly via [`Target::destroy`] (passing the owning device).
struct Target {
    image: VkImage,
    memory: VkDeviceMemory,
    view: VkImageView,
    render_pass: VkRenderPass,
    framebuffer: VkFramebuffer,
    width: u32,
    height: u32,
}

impl Target {
    unsafe fn destroy(self, device: VkDevice) {
        unsafe {
            vkDestroyFramebuffer(device, self.framebuffer, core::ptr::null());
            vkDestroyRenderPass(device, self.render_pass, core::ptr::null());
            vkDestroyImageView(device, self.view, core::ptr::null());
            vkFreeMemory(device, self.memory, core::ptr::null());
            vkDestroyImage(device, self.image, core::ptr::null());
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
        match gpu.create_target(width, height) {
            Ok(target) => {
                target.destroy(gpu.device);
                gpu.destroy();
                Ok(format!(
                    "{width}x{height} framebuffer + render pass + image view created"
                ))
            }
            Err(e) => {
                gpu.destroy();
                Err(e)
            }
        }
    }
}

/// Execute real GPU work: record a primary command buffer that runs the
/// single-attachment render pass — whose load-op **clears** the color image to a
/// fixed color — submit it to the graphics queue, and block on a fence until the
/// GPU signals completion. This is the first command-buffer round-trip the
/// eventual draw pipeline (pipeline + draw calls) slots into. Returns a one-line
/// summary. Errors if any step fails.
pub fn init_clear(width: u32, height: u32) -> Result<String, String> {
    unsafe {
        let gpu = Gpu::create()?;
        let target = match gpu.create_target(width, height) {
            Ok(t) => t,
            Err(e) => {
                gpu.destroy();
                return Err(e);
            }
        };

        // Tear down everything created so far, on failure or success.
        let finish = |gpu: Gpu, target: Target, pool: VkCommandPool, fence: VkFence| {
            if fence != 0 {
                vkDestroyFence(gpu.device, fence, core::ptr::null());
            }
            if pool != 0 {
                // Frees the command buffers allocated from it too.
                vkDestroyCommandPool(gpu.device, pool, core::ptr::null());
            }
            target.destroy(gpu.device);
            gpu.destroy();
        };

        // A command pool on the graphics family, and one primary command buffer.
        let pool_ci = VkCommandPoolCreateInfo {
            sType: VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO,
            pNext: core::ptr::null(),
            flags: 0,
            queueFamilyIndex: gpu.family,
        };
        let mut pool: VkCommandPool = 0;
        if vkCreateCommandPool(gpu.device, &pool_ci, core::ptr::null(), &mut pool) != VK_SUCCESS {
            finish(gpu, target, 0, 0);
            return Err("vkCreateCommandPool failed".into());
        }
        let alloc = VkCommandBufferAllocateInfo {
            sType: VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO,
            pNext: core::ptr::null(),
            commandPool: pool,
            level: VK_COMMAND_BUFFER_LEVEL_PRIMARY,
            commandBufferCount: 1,
        };
        let mut cmd: VkCommandBuffer = core::ptr::null_mut();
        if vkAllocateCommandBuffers(gpu.device, &alloc, &mut cmd) != VK_SUCCESS {
            finish(gpu, target, pool, 0);
            return Err("vkAllocateCommandBuffers failed".into());
        }

        // Record: begin → run the clearing render pass → end.
        let begin = VkCommandBufferBeginInfo {
            sType: VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO,
            pNext: core::ptr::null(),
            flags: VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT,
            pInheritanceInfo: core::ptr::null(),
        };
        if vkBeginCommandBuffer(cmd, &begin) != VK_SUCCESS {
            finish(gpu, target, pool, 0);
            return Err("vkBeginCommandBuffer failed".into());
        }
        // A recognizable forma blue (0x60, 0x9c, 0xff) so a later readback can
        // confirm the clear actually ran.
        let clear = VkClearValue {
            float32: [0x60 as f32 / 255.0, 0x9c as f32 / 255.0, 1.0, 1.0],
        };
        let rp_begin = VkRenderPassBeginInfo {
            sType: VK_STRUCTURE_TYPE_RENDER_PASS_BEGIN_INFO,
            pNext: core::ptr::null(),
            renderPass: target.render_pass,
            framebuffer: target.framebuffer,
            renderArea: VkRect2D {
                offset: VkOffset2D { x: 0, y: 0 },
                extent: VkExtent2D {
                    width: target.width,
                    height: target.height,
                },
            },
            clearValueCount: 1,
            pClearValues: &clear,
        };
        vkCmdBeginRenderPass(cmd, &rp_begin, VK_SUBPASS_CONTENTS_INLINE);
        vkCmdEndRenderPass(cmd);
        if vkEndCommandBuffer(cmd) != VK_SUCCESS {
            finish(gpu, target, pool, 0);
            return Err("vkEndCommandBuffer failed".into());
        }

        // Submit to the graphics queue, fenced, and wait for the GPU.
        let mut queue: VkQueue = core::ptr::null_mut();
        vkGetDeviceQueue(gpu.device, gpu.family, 0, &mut queue);

        let fence_ci = VkFenceCreateInfo {
            sType: VK_STRUCTURE_TYPE_FENCE_CREATE_INFO,
            pNext: core::ptr::null(),
            flags: 0,
        };
        let mut fence: VkFence = 0;
        if vkCreateFence(gpu.device, &fence_ci, core::ptr::null(), &mut fence) != VK_SUCCESS {
            finish(gpu, target, pool, 0);
            return Err("vkCreateFence failed".into());
        }
        let submit = VkSubmitInfo {
            sType: VK_STRUCTURE_TYPE_SUBMIT_INFO,
            pNext: core::ptr::null(),
            waitSemaphoreCount: 0,
            pWaitSemaphores: core::ptr::null(),
            pWaitDstStageMask: core::ptr::null(),
            commandBufferCount: 1,
            pCommandBuffers: &cmd,
            signalSemaphoreCount: 0,
            pSignalSemaphores: core::ptr::null(),
        };
        if vkQueueSubmit(queue, 1, &submit, fence) != VK_SUCCESS {
            finish(gpu, target, pool, fence);
            return Err("vkQueueSubmit failed".into());
        }
        let waited = vkWaitForFences(gpu.device, 1, &fence, VK_TRUE, u64::MAX) == VK_SUCCESS;

        finish(gpu, target, pool, fence);
        if waited {
            Ok(format!(
                "{width}x{height} render pass submitted and cleared (fence signaled)"
            ))
        } else {
            Err("vkWaitForFences failed".into())
        }
    }
}

/// The Vulkan offscreen capstone: run the clearing render pass on the GPU, then
/// `vkCmdCopyImageToBuffer` the result into a host-visible buffer, fence-wait,
/// map it, and return the `width`×`height` RGBA pixels — an actual GPU-rendered
/// frame read back to the CPU (so CI can turn it into a screenshot). The eventual
/// draw pipeline replaces only the "clear" with real draw calls; this readback
/// path stays. Errors if any step fails.
pub fn render_clear(width: u32, height: u32) -> Result<Vec<u8>, String> {
    unsafe {
        let gpu = Gpu::create()?;
        let target = match gpu.create_target(width, height) {
            Ok(t) => t,
            Err(e) => {
                gpu.destroy();
                return Err(e);
            }
        };
        let size = width as u64 * height as u64 * 4;
        let (buffer, buf_mem) = match gpu.create_host_buffer(size) {
            Ok(t) => t,
            Err(e) => {
                target.destroy(gpu.device);
                gpu.destroy();
                return Err(e);
            }
        };

        // Tear down everything created so far, on failure or success.
        let finish = |gpu: Gpu,
                      target: Target,
                      buffer: VkBuffer,
                      buf_mem: VkDeviceMemory,
                      pool: VkCommandPool,
                      fence: VkFence| {
            if fence != 0 {
                vkDestroyFence(gpu.device, fence, core::ptr::null());
            }
            if pool != 0 {
                vkDestroyCommandPool(gpu.device, pool, core::ptr::null());
            }
            vkFreeMemory(gpu.device, buf_mem, core::ptr::null());
            vkDestroyBuffer(gpu.device, buffer, core::ptr::null());
            target.destroy(gpu.device);
            gpu.destroy();
        };

        let pool_ci = VkCommandPoolCreateInfo {
            sType: VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO,
            pNext: core::ptr::null(),
            flags: 0,
            queueFamilyIndex: gpu.family,
        };
        let mut pool: VkCommandPool = 0;
        if vkCreateCommandPool(gpu.device, &pool_ci, core::ptr::null(), &mut pool) != VK_SUCCESS {
            finish(gpu, target, buffer, buf_mem, 0, 0);
            return Err("vkCreateCommandPool failed".into());
        }
        let alloc = VkCommandBufferAllocateInfo {
            sType: VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO,
            pNext: core::ptr::null(),
            commandPool: pool,
            level: VK_COMMAND_BUFFER_LEVEL_PRIMARY,
            commandBufferCount: 1,
        };
        let mut cmd: VkCommandBuffer = core::ptr::null_mut();
        if vkAllocateCommandBuffers(gpu.device, &alloc, &mut cmd) != VK_SUCCESS {
            finish(gpu, target, buffer, buf_mem, pool, 0);
            return Err("vkAllocateCommandBuffers failed".into());
        }

        let begin = VkCommandBufferBeginInfo {
            sType: VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO,
            pNext: core::ptr::null(),
            flags: VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT,
            pInheritanceInfo: core::ptr::null(),
        };
        if vkBeginCommandBuffer(cmd, &begin) != VK_SUCCESS {
            finish(gpu, target, buffer, buf_mem, pool, 0);
            return Err("vkBeginCommandBuffer failed".into());
        }
        // forma blue (0x60, 0x9c, 0xff) — the readback pixels should be this.
        let clear = VkClearValue {
            float32: [0x60 as f32 / 255.0, 0x9c as f32 / 255.0, 1.0, 1.0],
        };
        let rp_begin = VkRenderPassBeginInfo {
            sType: VK_STRUCTURE_TYPE_RENDER_PASS_BEGIN_INFO,
            pNext: core::ptr::null(),
            renderPass: target.render_pass,
            framebuffer: target.framebuffer,
            renderArea: VkRect2D {
                offset: VkOffset2D { x: 0, y: 0 },
                extent: VkExtent2D {
                    width: target.width,
                    height: target.height,
                },
            },
            clearValueCount: 1,
            pClearValues: &clear,
        };
        vkCmdBeginRenderPass(cmd, &rp_begin, VK_SUBPASS_CONTENTS_INLINE);
        vkCmdEndRenderPass(cmd);

        // The render pass left the image in TRANSFER_SRC_OPTIMAL — copy it,
        // tightly packed, into the host buffer.
        let region = VkBufferImageCopy {
            bufferOffset: 0,
            bufferRowLength: 0,
            bufferImageHeight: 0,
            imageSubresource: VkImageSubresourceLayers {
                aspectMask: VK_IMAGE_ASPECT_COLOR_BIT,
                mipLevel: 0,
                baseArrayLayer: 0,
                layerCount: 1,
            },
            imageOffset: VkOffset3D { x: 0, y: 0, z: 0 },
            imageExtent: VkExtent3D {
                width,
                height,
                depth: 1,
            },
        };
        vkCmdCopyImageToBuffer(
            cmd,
            target.image,
            VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL,
            buffer,
            1,
            &region,
        );
        if vkEndCommandBuffer(cmd) != VK_SUCCESS {
            finish(gpu, target, buffer, buf_mem, pool, 0);
            return Err("vkEndCommandBuffer failed".into());
        }

        let mut queue: VkQueue = core::ptr::null_mut();
        vkGetDeviceQueue(gpu.device, gpu.family, 0, &mut queue);
        let fence_ci = VkFenceCreateInfo {
            sType: VK_STRUCTURE_TYPE_FENCE_CREATE_INFO,
            pNext: core::ptr::null(),
            flags: 0,
        };
        let mut fence: VkFence = 0;
        if vkCreateFence(gpu.device, &fence_ci, core::ptr::null(), &mut fence) != VK_SUCCESS {
            finish(gpu, target, buffer, buf_mem, pool, 0);
            return Err("vkCreateFence failed".into());
        }
        let submit = VkSubmitInfo {
            sType: VK_STRUCTURE_TYPE_SUBMIT_INFO,
            pNext: core::ptr::null(),
            waitSemaphoreCount: 0,
            pWaitSemaphores: core::ptr::null(),
            pWaitDstStageMask: core::ptr::null(),
            commandBufferCount: 1,
            pCommandBuffers: &cmd,
            signalSemaphoreCount: 0,
            pSignalSemaphores: core::ptr::null(),
        };
        if vkQueueSubmit(queue, 1, &submit, fence) != VK_SUCCESS {
            finish(gpu, target, buffer, buf_mem, pool, fence);
            return Err("vkQueueSubmit failed".into());
        }
        if vkWaitForFences(gpu.device, 1, &fence, VK_TRUE, u64::MAX) != VK_SUCCESS {
            finish(gpu, target, buffer, buf_mem, pool, fence);
            return Err("vkWaitForFences failed".into());
        }

        // Map the host buffer and copy the pixels out.
        let mut ptr: *mut c_void = core::ptr::null_mut();
        if vkMapMemory(gpu.device, buf_mem, 0, size, 0, &mut ptr) != VK_SUCCESS || ptr.is_null() {
            finish(gpu, target, buffer, buf_mem, pool, fence);
            return Err("vkMapMemory failed".into());
        }
        let mut pixels = vec![0u8; size as usize];
        core::ptr::copy_nonoverlapping(ptr as *const u8, pixels.as_mut_ptr(), size as usize);
        vkUnmapMemory(gpu.device, buf_mem);

        finish(gpu, target, buffer, buf_mem, pool, fence);
        Ok(pixels)
    }
}
