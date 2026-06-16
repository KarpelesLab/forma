//! Raw Direct3D 11 FFI — no `windows`/`winapi` crate, just `d3d11.dll` and the
//! handful of COM methods we call by hand, matching the "close to the OS"
//! policy. Windows-only.
//!
//! This is the entry point for a GPU-native Direct3D render backend: it creates
//! a D3D11 device on **WARP** (the software rasterizer shipped with Windows, so
//! it runs on any CI runner without a GPU), the same way the Vulkan path starts
//! from an instance + device. The offscreen pipeline (render-target texture,
//! HLSL shaders, draw, staging-texture readback) builds on this.
//!
//! COM objects are accessed by hand: an interface pointer points at a vtable
//! pointer, and `IUnknown::Release` is always vtable slot 2 — enough to clean up
//! what we create without binding the full COM machinery.

#![allow(unsafe_code, non_snake_case, non_upper_case_globals)]

use core::ffi::c_void;

type Hresult = i32;
const S_OK: Hresult = 0;
// D3D_DRIVER_TYPE_WARP — the software rasterizer (no GPU required).
const D3D_DRIVER_TYPE_WARP: u32 = 5;
const D3D11_SDK_VERSION: u32 = 7;

#[link(name = "d3d11")]
unsafe extern "system" {
    fn D3D11CreateDevice(
        pAdapter: *mut c_void,
        DriverType: u32,
        Software: *mut c_void,
        Flags: u32,
        pFeatureLevels: *const u32,
        FeatureLevels: u32,
        SDKVersion: u32,
        ppDevice: *mut *mut c_void,
        pFeatureLevel: *mut u32,
        ppImmediateContext: *mut *mut c_void,
    ) -> Hresult;
}

/// Release a COM object via `IUnknown::Release` (vtable slot 2). No-op on null.
unsafe fn release(obj: *mut c_void) {
    unsafe {
        if obj.is_null() {
            return;
        }
        // obj -> *vtable -> [QueryInterface, AddRef, Release, ...]
        let vtbl = *(obj as *mut *mut usize);
        let release_fn: extern "system" fn(*mut c_void) -> u32 = core::mem::transmute(*vtbl.add(2));
        release_fn(obj);
    }
}

/// Create a Direct3D 11 device on WARP and return its feature level (e.g.
/// `"0xb100"` for 11_1) — the foundation a GPU-native D3D backend builds on.
/// Errors if `d3d11.dll` can't create a WARP device.
pub fn device() -> Result<String, String> {
    unsafe {
        let mut device: *mut c_void = core::ptr::null_mut();
        let mut context: *mut c_void = core::ptr::null_mut();
        let mut feature_level: u32 = 0;
        let hr = D3D11CreateDevice(
            core::ptr::null_mut(),
            D3D_DRIVER_TYPE_WARP,
            core::ptr::null_mut(),
            0,
            core::ptr::null(),
            0,
            D3D11_SDK_VERSION,
            &mut device,
            &mut feature_level,
            &mut context,
        );
        if hr != S_OK {
            return Err(format!(
                "D3D11CreateDevice (WARP) failed (0x{:08x})",
                hr as u32
            ));
        }
        release(context);
        release(device);
        Ok(format!("WARP device, feature level 0x{feature_level:04x}"))
    }
}
