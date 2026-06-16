//! Raw Metal FFI — no `metal`/`objc` crate, just the Metal framework's
//! `MTLCreateSystemDefaultDevice` and a couple of `objc_msgSend` calls by hand
//! (mirroring the Cocoa backend's approach), matching the "close to the OS"
//! policy. macOS-only.
//!
//! This is the entry point for a GPU-native Metal render backend: it creates the
//! system default `MTLDevice` and reads its name, the same way the Vulkan path
//! starts from an instance + device. The offscreen pipeline (a `MTLTexture`
//! render target, a render pipeline from a `.metal` source string, a command
//! buffer that draws, and `getBytes` readback) builds on this.

#![allow(unsafe_code, non_snake_case)]

use core::ffi::{c_char, c_void};
use std::ffi::{CStr, CString};

type Id = *mut c_void;
type Sel = *mut c_void;
type Class = *mut c_void;

#[link(name = "objc", kind = "dylib")]
unsafe extern "C" {
    fn objc_getClass(name: *const c_char) -> Class;
    fn sel_registerName(name: *const c_char) -> Sel;
    fn objc_msgSend();
    fn objc_autoreleasePoolPush() -> *mut c_void;
    fn objc_autoreleasePoolPop(ctx: *mut c_void);
}

#[link(name = "Metal", kind = "framework")]
unsafe extern "C" {
    fn MTLCreateSystemDefaultDevice() -> Id;
}

// Metal enum values we use.
const MTL_PIXEL_FORMAT_RGBA8UNORM: u64 = 70;
const MTL_TEXTURE_USAGE_RENDER_TARGET: u64 = 0x0004;
const MTL_STORAGE_MODE_SHARED: u64 = 0;
const MTL_LOAD_ACTION_CLEAR: u64 = 2;
const MTL_STORE_ACTION_STORE: u64 = 1;

#[repr(C)]
#[derive(Clone, Copy)]
struct MtlClearColor {
    red: f64,
    green: f64,
    blue: f64,
    alpha: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct MtlOrigin {
    x: u64,
    y: u64,
    z: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct MtlSize {
    width: u64,
    height: u64,
    depth: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct MtlRegion {
    origin: MtlOrigin,
    size: MtlSize,
}

fn class(name: &str) -> Class {
    let c = CString::new(name).unwrap();
    unsafe { objc_getClass(c.as_ptr()) }
}

fn sel(name: &str) -> Sel {
    let c = CString::new(name).unwrap();
    unsafe { sel_registerName(c.as_ptr()) }
}

/// `objc_msgSend` typed as `(id, SEL) -> id` — the documented way to call it.
unsafe fn msg_id(obj: Id, s: Sel) -> Id {
    unsafe {
        let f: unsafe extern "C" fn(Id, Sel) -> Id =
            std::mem::transmute(objc_msgSend as *const c_void);
        f(obj, s)
    }
}

/// `objc_msgSend` typed as `(id, SEL) -> *const c_char` (for `-UTF8String`).
unsafe fn msg_cstr(obj: Id, s: Sel) -> *const c_char {
    unsafe {
        let f: unsafe extern "C" fn(Id, Sel) -> *const c_char =
            std::mem::transmute(objc_msgSend as *const c_void);
        f(obj, s)
    }
}

/// `objc_msgSend` typed as `(id, SEL)` returning nothing (for `-release`).
unsafe fn msg_void(obj: Id, s: Sel) {
    unsafe {
        let f: unsafe extern "C" fn(Id, Sel) = std::mem::transmute(objc_msgSend as *const c_void);
        f(obj, s);
    }
}

unsafe fn msg_id_id(obj: Id, s: Sel, a: Id) -> Id {
    unsafe {
        let f: unsafe extern "C" fn(Id, Sel, Id) -> Id =
            std::mem::transmute(objc_msgSend as *const c_void);
        f(obj, s, a)
    }
}

unsafe fn msg_id_u64(obj: Id, s: Sel, a: u64) -> Id {
    unsafe {
        let f: unsafe extern "C" fn(Id, Sel, u64) -> Id =
            std::mem::transmute(objc_msgSend as *const c_void);
        f(obj, s, a)
    }
}

unsafe fn msg_void_id(obj: Id, s: Sel, a: Id) {
    unsafe {
        let f: unsafe extern "C" fn(Id, Sel, Id) =
            std::mem::transmute(objc_msgSend as *const c_void);
        f(obj, s, a);
    }
}

unsafe fn msg_void_u64(obj: Id, s: Sel, a: u64) {
    unsafe {
        let f: unsafe extern "C" fn(Id, Sel, u64) =
            std::mem::transmute(objc_msgSend as *const c_void);
        f(obj, s, a);
    }
}

/// `+[MTLTextureDescriptor texture2DDescriptorWithPixelFormat:width:height:mipmapped:]`.
unsafe fn msg_texdesc(cls: Class, s: Sel, fmt: u64, w: u64, h: u64, mip: bool) -> Id {
    unsafe {
        let f: unsafe extern "C" fn(Class, Sel, u64, u64, u64, bool) -> Id =
            std::mem::transmute(objc_msgSend as *const c_void);
        f(cls, s, fmt, w, h, mip)
    }
}

/// `-[…setClearColor:]` — `MTLClearColor` is 4 doubles (an HFA, passed in SIMD
/// registers on arm64, which `extern "C"` matches).
unsafe fn msg_void_clearcolor(obj: Id, s: Sel, c: MtlClearColor) {
    unsafe {
        let f: unsafe extern "C" fn(Id, Sel, MtlClearColor) =
            std::mem::transmute(objc_msgSend as *const c_void);
        f(obj, s, c);
    }
}

/// `-[MTLTexture getBytes:bytesPerRow:fromRegion:mipmapLevel:]`. `MTLRegion` is
/// 48 bytes, passed indirectly per AAPCS64 — `extern "C"` matches.
unsafe fn msg_getbytes(obj: Id, s: Sel, ptr: *mut c_void, bpr: u64, region: MtlRegion, level: u64) {
    unsafe {
        let f: unsafe extern "C" fn(Id, Sel, *mut c_void, u64, MtlRegion, u64) =
            std::mem::transmute(objc_msgSend as *const c_void);
        f(obj, s, ptr, bpr, region, level);
    }
}

/// Create the system default `MTLDevice` and return its name (e.g.
/// `"Apple M1"`) — the foundation a GPU-native Metal backend builds on. Errors
/// if no Metal device is available.
pub fn device() -> Result<String, String> {
    unsafe {
        let dev = MTLCreateSystemDefaultDevice();
        if dev.is_null() {
            return Err("MTLCreateSystemDefaultDevice returned nil".into());
        }
        // `-[MTLDevice name]` → NSString → `-UTF8String` → C string we copy out.
        let ns_name = msg_id(dev, sel("name"));
        let name = if ns_name.is_null() {
            "<unnamed>".to_string()
        } else {
            let cstr = msg_cstr(ns_name, sel("UTF8String"));
            if cstr.is_null() {
                "<unnamed>".to_string()
            } else {
                CStr::from_ptr(cstr).to_string_lossy().into_owned()
            }
        };
        // We own the device returned by MTLCreateSystemDefaultDevice; release it.
        msg_void(dev, sel("release"));
        Ok(name)
    }
}

/// Render a `width`×`height` frame on the GPU through Metal and read it back: a
/// `MTLTexture` render target cleared to forma blue via a render command
/// encoder, then `getBytes` to the CPU — an actual GPU-rendered frame, the Metal
/// analog of the Vulkan clear+readback. Returns the RGBA pixels. Errors if no
/// Metal device is available or any object fails to create.
pub fn render_clear(width: u32, height: u32) -> Result<Vec<u8>, String> {
    unsafe {
        let pool = objc_autoreleasePoolPush();
        let result = render_clear_inner(width, height);
        objc_autoreleasePoolPop(pool);
        result
    }
}

unsafe fn render_clear_inner(width: u32, height: u32) -> Result<Vec<u8>, String> {
    unsafe {
        let dev = MTLCreateSystemDefaultDevice();
        if dev.is_null() {
            return Err("MTLCreateSystemDefaultDevice returned nil".into());
        }
        let queue = msg_id(dev, sel("newCommandQueue"));
        if queue.is_null() {
            msg_void(dev, sel("release"));
            return Err("newCommandQueue returned nil".into());
        }

        // A CPU-readable (Shared) RGBA8 render-target texture.
        let desc = msg_texdesc(
            class("MTLTextureDescriptor"),
            sel("texture2DDescriptorWithPixelFormat:width:height:mipmapped:"),
            MTL_PIXEL_FORMAT_RGBA8UNORM,
            width as u64,
            height as u64,
            false,
        );
        msg_void_u64(desc, sel("setUsage:"), MTL_TEXTURE_USAGE_RENDER_TARGET);
        msg_void_u64(desc, sel("setStorageMode:"), MTL_STORAGE_MODE_SHARED);
        let tex = msg_id_id(dev, sel("newTextureWithDescriptor:"), desc);
        if tex.is_null() {
            msg_void(queue, sel("release"));
            msg_void(dev, sel("release"));
            return Err("newTextureWithDescriptor returned nil".into());
        }

        // A render pass that clears the texture to forma blue and stores it.
        let rpd = msg_id(
            class("MTLRenderPassDescriptor"),
            sel("renderPassDescriptor"),
        );
        let attachments = msg_id(rpd, sel("colorAttachments"));
        let att0 = msg_id_u64(attachments, sel("objectAtIndexedSubscript:"), 0);
        msg_void_id(att0, sel("setTexture:"), tex);
        msg_void_u64(att0, sel("setLoadAction:"), MTL_LOAD_ACTION_CLEAR);
        msg_void_u64(att0, sel("setStoreAction:"), MTL_STORE_ACTION_STORE);
        msg_void_clearcolor(
            att0,
            sel("setClearColor:"),
            MtlClearColor {
                red: 0x60 as f64 / 255.0,
                green: 0x9c as f64 / 255.0,
                blue: 1.0,
                alpha: 1.0,
            },
        );

        // Encode an (empty) render pass — the clear load-action does the work.
        let cmdbuf = msg_id(queue, sel("commandBuffer"));
        let enc = msg_id_id(cmdbuf, sel("renderCommandEncoderWithDescriptor:"), rpd);
        if enc.is_null() {
            msg_void(tex, sel("release"));
            msg_void(queue, sel("release"));
            msg_void(dev, sel("release"));
            return Err("renderCommandEncoderWithDescriptor returned nil".into());
        }
        msg_void(enc, sel("endEncoding"));
        msg_void(cmdbuf, sel("commit"));
        msg_void(cmdbuf, sel("waitUntilCompleted"));

        // Read the texture back to the CPU.
        let size = width as usize * height as usize * 4;
        let mut pixels = vec![0u8; size];
        let region = MtlRegion {
            origin: MtlOrigin { x: 0, y: 0, z: 0 },
            size: MtlSize {
                width: width as u64,
                height: height as u64,
                depth: 1,
            },
        };
        msg_getbytes(
            tex,
            sel("getBytes:bytesPerRow:fromRegion:mipmapLevel:"),
            pixels.as_mut_ptr() as *mut c_void,
            width as u64 * 4,
            region,
            0,
        );

        msg_void(tex, sel("release"));
        msg_void(queue, sel("release"));
        msg_void(dev, sel("release"));
        Ok(pixels)
    }
}
