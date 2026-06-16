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
const DXGI_FORMAT_R8G8B8A8_UNORM: u32 = 28;
const D3D11_USAGE_DEFAULT: u32 = 0;
const D3D11_USAGE_STAGING: u32 = 3;
const D3D11_BIND_RENDER_TARGET: u32 = 0x20;
const D3D11_CPU_ACCESS_READ: u32 = 0x2_0000;
const D3D11_MAP_READ: u32 = 1;

// COM vtable slots we call by hand (method order from d3d11.h, after the 3
// IUnknown / 7 ID3D11DeviceChild inherited entries).
const VT_DEVICE_CREATE_TEXTURE2D: usize = 5;
const VT_DEVICE_CREATE_RTV: usize = 9;
const VT_CTX_MAP: usize = 14;
const VT_CTX_UNMAP: usize = 15;
const VT_CTX_COPY_RESOURCE: usize = 47;
const VT_CTX_CLEAR_RTV: usize = 50;

#[repr(C)]
struct DxgiSampleDesc {
    Count: u32,
    Quality: u32,
}

#[repr(C)]
struct D3d11Texture2dDesc {
    Width: u32,
    Height: u32,
    MipLevels: u32,
    ArraySize: u32,
    Format: u32,
    SampleDesc: DxgiSampleDesc,
    Usage: u32,
    BindFlags: u32,
    CPUAccessFlags: u32,
    MiscFlags: u32,
}

#[repr(C)]
struct D3d11MappedSubresource {
    pData: *mut c_void,
    RowPitch: u32,
    DepthPitch: u32,
}

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

/// Fetch the `index`-th function pointer from a COM object's vtable, ready to
/// transmute to the method's signature. `obj -> *vtable -> [fn; ...]`.
unsafe fn vmethod(obj: *mut c_void, index: usize) -> *const c_void {
    unsafe {
        let vtbl = *(obj as *const *const *const c_void);
        *vtbl.add(index)
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

/// Render a `width`×`height` frame through Direct3D 11 on WARP and read it back:
/// create a render-target texture, clear it to forma blue, copy it into a
/// CPU-readable staging texture, map it, and return the RGBA pixels — an actual
/// GPU-rendered frame, the D3D analog of the Vulkan/Metal clear+readback. Errors
/// if any COM call fails.
pub fn render_clear(width: u32, height: u32) -> Result<Vec<u8>, String> {
    unsafe {
        let mut device: *mut c_void = core::ptr::null_mut();
        let mut ctx: *mut c_void = core::ptr::null_mut();
        let mut feature_level: u32 = 0;
        if D3D11CreateDevice(
            core::ptr::null_mut(),
            D3D_DRIVER_TYPE_WARP,
            core::ptr::null_mut(),
            0,
            core::ptr::null(),
            0,
            D3D11_SDK_VERSION,
            &mut device,
            &mut feature_level,
            &mut ctx,
        ) != S_OK
        {
            return Err("D3D11CreateDevice (WARP) failed".into());
        }

        // CreateTexture2D — a render-target RGBA8 texture.
        let rt_desc = D3d11Texture2dDesc {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            SampleDesc: DxgiSampleDesc {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_RENDER_TARGET,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let create_tex2d: unsafe extern "system" fn(
            *mut c_void,
            *const D3d11Texture2dDesc,
            *const c_void,
            *mut *mut c_void,
        ) -> Hresult = core::mem::transmute(vmethod(device, VT_DEVICE_CREATE_TEXTURE2D));
        let mut rt: *mut c_void = core::ptr::null_mut();
        if create_tex2d(device, &rt_desc, core::ptr::null(), &mut rt) != S_OK {
            release(ctx);
            release(device);
            return Err("CreateTexture2D (render target) failed".into());
        }

        // CreateRenderTargetView over it.
        let create_rtv: unsafe extern "system" fn(
            *mut c_void,
            *mut c_void,
            *const c_void,
            *mut *mut c_void,
        ) -> Hresult = core::mem::transmute(vmethod(device, VT_DEVICE_CREATE_RTV));
        let mut rtv: *mut c_void = core::ptr::null_mut();
        if create_rtv(device, rt, core::ptr::null(), &mut rtv) != S_OK {
            release(rt);
            release(ctx);
            release(device);
            return Err("CreateRenderTargetView failed".into());
        }

        // ClearRenderTargetView to forma blue (R,G,B,A floats).
        let clear_rtv: unsafe extern "system" fn(*mut c_void, *mut c_void, *const [f32; 4]) =
            core::mem::transmute(vmethod(ctx, VT_CTX_CLEAR_RTV));
        let color: [f32; 4] = [0x60 as f32 / 255.0, 0x9c as f32 / 255.0, 1.0, 1.0];
        clear_rtv(ctx, rtv, &color);

        // A staging texture the CPU can read, and a copy into it.
        let staging_desc = D3d11Texture2dDesc {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            SampleDesc: DxgiSampleDesc {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ,
            MiscFlags: 0,
        };
        let mut staging: *mut c_void = core::ptr::null_mut();
        if create_tex2d(device, &staging_desc, core::ptr::null(), &mut staging) != S_OK {
            release(rtv);
            release(rt);
            release(ctx);
            release(device);
            return Err("CreateTexture2D (staging) failed".into());
        }
        let copy_resource: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut c_void) =
            core::mem::transmute(vmethod(ctx, VT_CTX_COPY_RESOURCE));
        copy_resource(ctx, staging, rt);

        // Map the staging texture and copy the pixels out, honoring RowPitch.
        let map: unsafe extern "system" fn(
            *mut c_void,
            *mut c_void,
            u32,
            u32,
            u32,
            *mut D3d11MappedSubresource,
        ) -> Hresult = core::mem::transmute(vmethod(ctx, VT_CTX_MAP));
        let mut mapped = D3d11MappedSubresource {
            pData: core::ptr::null_mut(),
            RowPitch: 0,
            DepthPitch: 0,
        };
        if map(ctx, staging, 0, D3D11_MAP_READ, 0, &mut mapped) != S_OK || mapped.pData.is_null() {
            release(staging);
            release(rtv);
            release(rt);
            release(ctx);
            release(device);
            return Err("Map (staging) failed".into());
        }
        let mut pixels = vec![0u8; width as usize * height as usize * 4];
        let row_bytes = width as usize * 4;
        for y in 0..height as usize {
            let src = (mapped.pData as *const u8).add(y * mapped.RowPitch as usize);
            let dst = pixels.as_mut_ptr().add(y * row_bytes);
            core::ptr::copy_nonoverlapping(src, dst, row_bytes);
        }
        let unmap: unsafe extern "system" fn(*mut c_void, *mut c_void, u32) =
            core::mem::transmute(vmethod(ctx, VT_CTX_UNMAP));
        unmap(ctx, staging, 0);

        release(staging);
        release(rtv);
        release(rt);
        release(ctx);
        release(device);
        Ok(pixels)
    }
}
