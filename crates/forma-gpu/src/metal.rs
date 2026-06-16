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

#[link(name = "objc", kind = "dylib")]
unsafe extern "C" {
    fn sel_registerName(name: *const c_char) -> Sel;
    fn objc_msgSend();
}

#[link(name = "Metal", kind = "framework")]
unsafe extern "C" {
    fn MTLCreateSystemDefaultDevice() -> Id;
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
