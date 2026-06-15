//! A native Windows backend over raw Win32 FFI (`user32`/`gdi32`/`kernel32`).
//!
//! No `windows`/`winapi` crate — direct `extern "system"` declarations, so the
//! only "dependency" is the OS itself (the workspace policy in `ROADMAP.md`
//! §1). This module is the one place `unsafe` is permitted in the crate.
//!
//! Scope (v1): register a window class, create + show a top-level window, and
//! present the software [`Pixmap`] by blitting a top-down 32-bit DIB with
//! `StretchDIBits` on `WM_PAINT`. Input (mouse/keyboard) and live resize are
//! follow-ups; the first milestone is a CI-screenshotable window.
//!
//! **Verification:** build-checked on the `windows-latest` CI runner via the
//! workspace build matrix; runtime-screenshotted by the Visual workflow's
//! Windows job.
#![allow(unsafe_code)]

use std::cell::RefCell;
use std::ffi::c_void;

use crate::ControlFlow;
use crate::error::PlatformError;
use crate::event::{Event, WindowId};
use crate::window::{Window, WindowAttributes};
use forma_geometry::{PhysicalSize, ScaleFactor};
use forma_render::{Pixmap, Surface};

// ---- Win32 type aliases -----------------------------------------------------

type Handle = *mut c_void;
type Hwnd = Handle;
type Wparam = usize;
type Lparam = isize;
type Lresult = isize;
type WndProc = unsafe extern "system" fn(Hwnd, u32, Wparam, Lparam) -> Lresult;

const WS_OVERLAPPEDWINDOW: u32 = 0x00CF_0000;
const CW_USEDEFAULT: i32 = -2147483648; // 0x80000000
const SW_SHOW: i32 = 5;
const WM_DESTROY: u32 = 0x0002;
const WM_CLOSE: u32 = 0x0010;
const WM_PAINT: u32 = 0x000F;
const BI_RGB: u32 = 0;
const DIB_RGB_COLORS: u32 = 0;
const SRCCOPY: u32 = 0x00CC_0020;
const IDC_ARROW: usize = 32512;

#[repr(C)]
struct WndClassW {
    style: u32,
    lpfn_wnd_proc: Option<WndProc>,
    cb_cls_extra: i32,
    cb_wnd_extra: i32,
    h_instance: Handle,
    h_icon: Handle,
    h_cursor: Handle,
    hbr_background: Handle,
    lpsz_menu_name: *const u16,
    lpsz_class_name: *const u16,
}

#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}

#[repr(C)]
struct Msg {
    hwnd: Hwnd,
    message: u32,
    w_param: Wparam,
    l_param: Lparam,
    time: u32,
    pt: Point,
    private: u32,
}

#[repr(C)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct PaintStruct {
    hdc: Handle,
    f_erase: i32,
    rc_paint: Rect,
    f_restore: i32,
    f_inc_update: i32,
    rgb_reserved: [u8; 32],
}

#[repr(C)]
struct BitmapInfoHeader {
    bi_size: u32,
    bi_width: i32,
    bi_height: i32,
    bi_planes: u16,
    bi_bit_count: u16,
    bi_compression: u32,
    bi_size_image: u32,
    bi_x_pels_per_meter: i32,
    bi_y_pels_per_meter: i32,
    bi_clr_used: u32,
    bi_clr_important: u32,
}

#[repr(C)]
struct BitmapInfo {
    header: BitmapInfoHeader,
    colors: [u32; 1],
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name: *const u16) -> Handle;
}

#[link(name = "user32")]
unsafe extern "system" {
    fn RegisterClassW(class: *const WndClassW) -> u16;
    fn LoadCursorW(instance: Handle, name: usize) -> Handle;
    #[allow(clippy::too_many_arguments)]
    fn CreateWindowExW(
        ex_style: u32,
        class_name: *const u16,
        window_name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: Hwnd,
        menu: Handle,
        instance: Handle,
        param: *mut c_void,
    ) -> Hwnd;
    fn ShowWindow(hwnd: Hwnd, cmd: i32) -> i32;
    fn UpdateWindow(hwnd: Hwnd) -> i32;
    fn DefWindowProcW(hwnd: Hwnd, msg: u32, wp: Wparam, lp: Lparam) -> Lresult;
    fn GetMessageW(msg: *mut Msg, hwnd: Hwnd, min: u32, max: u32) -> i32;
    fn TranslateMessage(msg: *const Msg) -> i32;
    fn DispatchMessageW(msg: *const Msg) -> Lresult;
    fn PostQuitMessage(code: i32);
    fn DestroyWindow(hwnd: Hwnd) -> i32;
    fn GetClientRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
    fn InvalidateRect(hwnd: Hwnd, rect: *const Rect, erase: i32) -> i32;
    fn BeginPaint(hwnd: Hwnd, ps: *mut PaintStruct) -> Handle;
    fn EndPaint(hwnd: Hwnd, ps: *const PaintStruct) -> i32;
}

#[link(name = "gdi32")]
unsafe extern "system" {
    #[allow(clippy::too_many_arguments)]
    fn StretchDIBits(
        hdc: Handle,
        x_dest: i32,
        y_dest: i32,
        w_dest: i32,
        h_dest: i32,
        x_src: i32,
        y_src: i32,
        w_src: i32,
        h_src: i32,
        bits: *const c_void,
        bmi: *const BitmapInfo,
        usage: u32,
        rop: u32,
    ) -> i32;
}

/// Per-thread window state shared between the message loop, the [`WndProc`]
/// (which blits on `WM_PAINT`), and the [`Surface`] (which updates the buffer).
#[derive(Default)]
struct WinCtx {
    /// Top-down BGRA framebuffer for the current frame.
    fb: Vec<u8>,
    fb_w: i32,
    fb_h: i32,
}

thread_local! {
    static CTX: RefCell<WinCtx> = RefCell::new(WinCtx::default());
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe extern "system" fn wnd_proc(hwnd: Hwnd, msg: u32, wp: Wparam, lp: Lparam) -> Lresult {
    match msg {
        WM_PAINT => {
            let mut ps: PaintStruct = unsafe { std::mem::zeroed() };
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
            CTX.with(|c| {
                let c = c.borrow();
                if !c.fb.is_empty() {
                    let bmi = BitmapInfo {
                        header: BitmapInfoHeader {
                            bi_size: core::mem::size_of::<BitmapInfoHeader>() as u32,
                            bi_width: c.fb_w,
                            // Negative height => top-down DIB (row 0 is the top).
                            bi_height: -c.fb_h,
                            bi_planes: 1,
                            bi_bit_count: 32,
                            bi_compression: BI_RGB,
                            bi_size_image: 0,
                            bi_x_pels_per_meter: 0,
                            bi_y_pels_per_meter: 0,
                            bi_clr_used: 0,
                            bi_clr_important: 0,
                        },
                        colors: [0],
                    };
                    unsafe {
                        StretchDIBits(
                            hdc,
                            0,
                            0,
                            c.fb_w,
                            c.fb_h,
                            0,
                            0,
                            c.fb_w,
                            c.fb_h,
                            c.fb.as_ptr() as *const c_void,
                            &bmi,
                            DIB_RGB_COLORS,
                            SRCCOPY,
                        );
                    }
                }
            });
            unsafe { EndPaint(hwnd, &ps) };
            0
        }
        WM_CLOSE => {
            unsafe { DestroyWindow(hwnd) };
            0
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
    }
}

#[derive(Debug)]
struct WinWindow {
    hwnd: Hwnd,
    size: PhysicalSize,
}

impl Window for WinWindow {
    fn id(&self) -> WindowId {
        WindowId(self.hwnd as u64)
    }
    fn inner_size(&self) -> PhysicalSize {
        self.size
    }
    fn scale_factor(&self) -> ScaleFactor {
        ScaleFactor::IDENTITY
    }
    fn request_redraw(&self) {
        unsafe { InvalidateRect(self.hwnd, std::ptr::null(), 0) };
    }
    fn set_title(&self, _title: &str) {}
    fn create_surface(&self) -> Box<dyn Surface> {
        Box::new(WinSurface {
            hwnd: self.hwnd,
            size: self.size,
        })
    }
}

#[derive(Debug)]
struct WinSurface {
    hwnd: Hwnd,
    size: PhysicalSize,
}

impl Surface for WinSurface {
    fn resize(&mut self, size: PhysicalSize) {
        self.size = size;
    }
    fn size(&self) -> PhysicalSize {
        self.size
    }
    fn present(&mut self, pixmap: &Pixmap, _damage: &[forma_geometry::Rect]) {
        let size = pixmap.size();
        let src = pixmap.as_bytes();
        // RGBA -> BGRA (Win32 DIBs are BGRA, top-down via negative height).
        let mut fb = vec![0u8; src.len()];
        for (d, s) in fb.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
            d[0] = s[2];
            d[1] = s[1];
            d[2] = s[0];
            d[3] = s[3];
        }
        CTX.with(|c| {
            let mut c = c.borrow_mut();
            c.fb = fb;
            c.fb_w = size.width as i32;
            c.fb_h = size.height as i32;
        });
        unsafe { InvalidateRect(self.hwnd, std::ptr::null(), 0) };
    }
}

/// Create a window, render the initial frame, and pump the Win32 message loop
/// until `WM_QUIT`. `handler` is invoked for [`Event::RedrawRequested`] (to
/// produce the frame) and [`Event::CloseRequested`].
pub fn run<H>(attrs: WindowAttributes, mut handler: H) -> Result<(), PlatformError>
where
    H: FnMut(Event, &dyn Window) -> ControlFlow,
{
    let class_name = wide("FormaWindowClass");
    let title = wide(&attrs.title);
    let size = ScaleFactor::IDENTITY.to_physical(attrs.logical_size);

    let hinstance = unsafe { GetModuleHandleW(std::ptr::null()) };
    let cursor = unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) };

    let class = WndClassW {
        style: 0,
        lpfn_wnd_proc: Some(wnd_proc),
        cb_cls_extra: 0,
        cb_wnd_extra: 0,
        h_instance: hinstance,
        h_icon: std::ptr::null_mut(),
        h_cursor: cursor,
        hbr_background: std::ptr::null_mut(),
        lpsz_menu_name: std::ptr::null(),
        lpsz_class_name: class_name.as_ptr(),
    };
    let atom = unsafe { RegisterClassW(&class) };
    if atom == 0 {
        return Err(PlatformError::Os("RegisterClassW failed".into()));
    }

    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            size.width as i32,
            size.height as i32,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinstance,
            std::ptr::null_mut(),
        )
    };
    if hwnd.is_null() {
        return Err(PlatformError::Os("CreateWindowExW failed".into()));
    }

    // Use the actual client area for the framebuffer size.
    let mut rect = Rect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    unsafe { GetClientRect(hwnd, &mut rect) };
    let client = PhysicalSize::new(
        (rect.right - rect.left).max(1) as u32,
        (rect.bottom - rect.top).max(1) as u32,
    );
    let win = WinWindow { hwnd, size: client };

    // Render the initial frame into the thread-local framebuffer.
    handler(Event::RedrawRequested, &win);

    unsafe {
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
    }

    // Message loop.
    let mut msg: Msg = unsafe { std::mem::zeroed() };
    loop {
        let r = unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) };
        if r <= 0 {
            // 0 = WM_QUIT, -1 = error.
            break;
        }
        unsafe {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        if msg.message == WM_CLOSE && handler(Event::CloseRequested, &win) == ControlFlow::Exit {
            unsafe { DestroyWindow(hwnd) };
        }
    }
    Ok(())
}
