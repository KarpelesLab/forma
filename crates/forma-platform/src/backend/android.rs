//! A native Android present path over the NDK's `ANativeWindow` — no NDK crate
//! (`ndk`/`ndk-glue`), just the C ABI from `libandroid`, matching the "talk to
//! the OS directly" policy. Android-only.
//!
//! The software [`Pixmap`] is blitted to an `ANativeWindow` (the surface a
//! `NativeActivity` hands the app): set the buffer geometry to the frame size,
//! lock to get a CPU pointer + row stride, copy the pixels, and post. This is
//! the rendering core; wiring it to the full `NativeActivity` lifecycle (a
//! looper-driven event loop fed by the activity callbacks) is the next step, so
//! [`run`] still uses the headless fallback until then.
//!
//! **Verification:** build-checked for `aarch64-linux-android` by the `mobile`
//! cross-compile CI job; an emulator run is a follow-up.
#![allow(unsafe_code)]

use core::ffi::c_void;

use forma_geometry::PhysicalSize;
use forma_render::Pixmap;

// WINDOW_FORMAT_RGBA_8888 — straight R,G,B,A bytes, matching our Pixmap.
const WINDOW_FORMAT_RGBA_8888: i32 = 1;

/// `ANativeWindow_Buffer` — the locked back buffer. `stride` is in **pixels**.
#[repr(C)]
struct ANativeWindowBuffer {
    width: i32,
    height: i32,
    stride: i32,
    format: i32,
    bits: *mut c_void,
    reserved: [u32; 6],
}

#[link(name = "android")]
unsafe extern "C" {
    fn ANativeWindow_setBuffersGeometry(
        window: *mut c_void,
        width: i32,
        height: i32,
        format: i32,
    ) -> i32;
    fn ANativeWindow_lock(
        window: *mut c_void,
        out_buffer: *mut ANativeWindowBuffer,
        in_out_dirty_bounds: *mut c_void,
    ) -> i32;
    fn ANativeWindow_unlockAndPost(window: *mut c_void) -> i32;
}

/// Blit a software [`Pixmap`] to an `ANativeWindow`: size the buffer to the
/// frame, lock it, copy the pixels row by row (honoring the buffer's pixel
/// stride), and post. Returns whether the post succeeded. `window` must be a
/// valid `ANativeWindow*` from a `NativeActivity` surface.
///
/// # Safety
/// `window` must be a live `ANativeWindow*`.
pub unsafe fn present_to_native_window(window: *mut c_void, pixmap: &Pixmap) -> bool {
    if window.is_null() {
        return false;
    }
    let size = pixmap.size();
    let (w, h) = (size.width as i32, size.height as i32);
    unsafe {
        ANativeWindow_setBuffersGeometry(window, w, h, WINDOW_FORMAT_RGBA_8888);
        let mut buf = ANativeWindowBuffer {
            width: 0,
            height: 0,
            stride: 0,
            format: 0,
            bits: core::ptr::null_mut(),
            reserved: [0; 6],
        };
        if ANativeWindow_lock(window, &mut buf, core::ptr::null_mut()) != 0 || buf.bits.is_null() {
            return false;
        }
        // Copy each row into the locked buffer, honoring its pixel stride.
        let src = pixmap.as_bytes();
        let src_stride = pixmap.stride();
        let copy_w = (buf.width.min(w)) as usize * 4;
        let rows = buf.height.min(h) as usize;
        let dst_stride = buf.stride as usize * 4;
        for y in 0..rows {
            let s = &src[y * src_stride..y * src_stride + copy_w];
            let d = (buf.bits as *mut u8).add(y * dst_stride);
            core::ptr::copy_nonoverlapping(s.as_ptr(), d, copy_w);
        }
        ANativeWindow_unlockAndPost(window) == 0
    }
}

/// The drawable size of an `ANativeWindow` is not queried here yet; callers pass
/// the surface size in from the `NativeActivity` configuration. Kept for the
/// upcoming lifecycle integration.
#[allow(dead_code)]
pub fn surface_size_hint() -> PhysicalSize {
    PhysicalSize::new(0, 0)
}
