//! A native macOS backend over the raw Objective-C runtime + Cocoa/CoreGraphics.
//!
//! No `objc`/`cocoa`/`core-graphics` crate — direct `objc_msgSend` FFI and
//! framework links, so the only "dependency" is the OS (workspace policy in
//! `ROADMAP.md` §1). This module opts into `unsafe` for the crate.
//!
//! An `NSWindow` hosts a custom `NSView` whose `drawRect:` blits the software
//! [`Pixmap`] as a `CGImage`. A manual `nextEventMatchingMask:` loop translates
//! `NSEvent`s into Forma pointer/keyboard events (y-flipped to top-left origin)
//! and polls the view bounds for live resize.
//!
//! **Verification:** build-checked on the `macos-latest` CI runner via the
//! build matrix; runtime-screenshotted by the Visual workflow's macOS job
//! (`screencapture`).
#![allow(unsafe_code)]

use std::cell::RefCell;
use std::ffi::{CString, c_void};

use crate::ControlFlow;
use crate::error::PlatformError;
use crate::event::{ButtonState, Event, KeyCode, PointerButton, WindowId};
use crate::window::{Window, WindowAttributes};
use forma_geometry::{PhysicalSize, Point, ScaleFactor};
use forma_render::{Pixmap, Surface};

// ---- Objective-C runtime + framework FFI ------------------------------------

type Id = *mut c_void;
type Sel = *mut c_void;
type Class = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy)]
struct CgPoint {
    x: f64,
    y: f64,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct CgSize {
    width: f64,
    height: f64,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct CgRect {
    origin: CgPoint,
    size: CgSize,
}

#[link(name = "objc", kind = "dylib")]
unsafe extern "C" {
    fn objc_getClass(name: *const i8) -> Class;
    fn sel_registerName(name: *const i8) -> Sel;
    fn objc_msgSend();
    fn objc_allocateClassPair(superclass: Class, name: *const i8, extra: usize) -> Class;
    fn objc_registerClassPair(cls: Class);
    fn class_addMethod(cls: Class, name: Sel, imp: *const c_void, types: *const i8) -> bool;
}

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGColorSpaceCreateDeviceRGB() -> *mut c_void;
    fn CGColorSpaceRelease(space: *mut c_void);
    fn CGDataProviderCreateWithData(
        info: *mut c_void,
        data: *const c_void,
        size: usize,
        release: *const c_void,
    ) -> *mut c_void;
    fn CGDataProviderRelease(provider: *mut c_void);
    #[allow(clippy::too_many_arguments)]
    fn CGImageCreate(
        width: usize,
        height: usize,
        bits_per_component: usize,
        bits_per_pixel: usize,
        bytes_per_row: usize,
        space: *mut c_void,
        bitmap_info: u32,
        provider: *mut c_void,
        decode: *const f64,
        should_interpolate: bool,
        intent: i32,
    ) -> *mut c_void;
    fn CGImageRelease(image: *mut c_void);
    fn CGContextDrawImage(ctx: *mut c_void, rect: CgRect, image: *mut c_void);
    fn CGContextSaveGState(ctx: *mut c_void);
    fn CGContextRestoreGState(ctx: *mut c_void);
    fn CGContextTranslateCTM(ctx: *mut c_void, tx: f64, ty: f64);
    fn CGContextScaleCTM(ctx: *mut c_void, sx: f64, sy: f64);
}

const NS_WINDOW_STYLE_TITLED: u64 = 1;
const NS_WINDOW_STYLE_CLOSABLE: u64 = 2;
const NS_WINDOW_STYLE_RESIZABLE: u64 = 8;
const NS_BACKING_BUFFERED: u64 = 2;
// kCGImageAlphaPremultipliedLast | kCGBitmapByteOrder32Big — interprets the
// buffer bytes as R,G,B,A in memory.
const CG_BITMAP_RGBA8: u32 = 1 | (4 << 12);
const ACTIVATION_REGULAR: i64 = 0;

fn class(name: &str) -> Class {
    let c = CString::new(name).unwrap();
    unsafe { objc_getClass(c.as_ptr()) }
}

fn sel(name: &str) -> Sel {
    let c = CString::new(name).unwrap();
    unsafe { sel_registerName(c.as_ptr()) }
}

// Typed `objc_msgSend` shims. `objc_msgSend` is variadic in C; we transmute it
// to the exact signature of each call site (the documented way to use it).
unsafe fn msg_id(obj: Id, s: Sel) -> Id {
    let f: unsafe extern "C" fn(Id, Sel) -> Id =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(obj, s) }
}
unsafe fn msg_void(obj: Id, s: Sel) {
    let f: unsafe extern "C" fn(Id, Sel) =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(obj, s) }
}
unsafe fn msg_void_id(obj: Id, s: Sel, a: Id) {
    let f: unsafe extern "C" fn(Id, Sel, Id) =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(obj, s, a) }
}
unsafe fn msg_void_bool(obj: Id, s: Sel, a: bool) {
    let f: unsafe extern "C" fn(Id, Sel, bool) =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(obj, s, a) }
}
unsafe fn msg_void_i64(obj: Id, s: Sel, a: i64) {
    let f: unsafe extern "C" fn(Id, Sel, i64) =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(obj, s, a) }
}
unsafe fn msg_u64(obj: Id, s: Sel) -> u64 {
    let f: unsafe extern "C" fn(Id, Sel) -> u64 =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(obj, s) }
}
unsafe fn msg_u16(obj: Id, s: Sel) -> u16 {
    let f: unsafe extern "C" fn(Id, Sel) -> u16 =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(obj, s) }
}
// NSPoint / NSRect are 2 / 4 doubles. On arm64 (macos-latest) these are
// returned through plain `objc_msgSend` (no `_stret`), so the transmute is ABI-
// correct there.
unsafe fn msg_point(obj: Id, s: Sel) -> CgPoint {
    let f: unsafe extern "C" fn(Id, Sel) -> CgPoint =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(obj, s) }
}
unsafe fn msg_rect(obj: Id, s: Sel) -> CgRect {
    let f: unsafe extern "C" fn(Id, Sel) -> CgRect =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(obj, s) }
}

/// `[NSString stringWithUTF8String:s]` — an autoreleased NSString.
fn nsstring(s: &str) -> Id {
    let c = CString::new(s).unwrap();
    let f: unsafe extern "C" fn(Id, Sel, *const i8) -> Id =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(class("NSString"), sel("stringWithUTF8String:"), c.as_ptr()) }
}

// NSEventType values we care about.
const NS_LEFT_MOUSE_DOWN: u64 = 1;
const NS_LEFT_MOUSE_UP: u64 = 2;
const NS_RIGHT_MOUSE_DOWN: u64 = 3;
const NS_RIGHT_MOUSE_UP: u64 = 4;
const NS_MOUSE_MOVED: u64 = 5;
const NS_LEFT_MOUSE_DRAGGED: u64 = 6;
const NS_KEY_DOWN: u64 = 10;

// macOS virtual key codes for editing/navigation keys.
fn map_keycode(kc: u16) -> Option<KeyCode> {
    Some(match kc {
        36 => KeyCode::Enter,
        48 => KeyCode::Tab,
        51 => KeyCode::Backspace,
        53 => KeyCode::Escape,
        123 => KeyCode::ArrowLeft,
        124 => KeyCode::ArrowRight,
        125 => KeyCode::ArrowDown,
        126 => KeyCode::ArrowUp,
        _ => return None,
    })
}

/// Translate an `NSEvent` into a Forma event. `view_h` is the content height,
/// used to flip the y axis (Cocoa's window origin is bottom-left).
unsafe fn translate_event(etype: u64, ev: Id, view_h: f64) -> Option<Event> {
    let pointer = |button: PointerButton, state: ButtonState| {
        let p = unsafe { msg_point(ev, sel("locationInWindow")) };
        Event::PointerButton {
            button,
            state,
            position: Point::new(p.x, view_h - p.y),
        }
    };
    match etype {
        NS_LEFT_MOUSE_DOWN => Some(pointer(PointerButton::Left, ButtonState::Pressed)),
        NS_LEFT_MOUSE_UP => Some(pointer(PointerButton::Left, ButtonState::Released)),
        NS_RIGHT_MOUSE_DOWN => Some(pointer(PointerButton::Right, ButtonState::Pressed)),
        NS_RIGHT_MOUSE_UP => Some(pointer(PointerButton::Right, ButtonState::Released)),
        NS_MOUSE_MOVED | NS_LEFT_MOUSE_DRAGGED => {
            let p = unsafe { msg_point(ev, sel("locationInWindow")) };
            Some(Event::PointerMoved {
                position: Point::new(p.x, view_h - p.y),
            })
        }
        NS_KEY_DOWN => {
            let kc = unsafe { msg_u16(ev, sel("keyCode")) };
            if let Some(code) = map_keycode(kc) {
                return Some(Event::Key {
                    code,
                    state: ButtonState::Pressed,
                    modifiers: Default::default(),
                });
            }
            // Otherwise deliver the typed characters as text.
            let chars = unsafe { msg_id(ev, sel("characters")) };
            if chars.is_null() {
                return None;
            }
            let utf8 = unsafe { msg_id(chars, sel("UTF8String")) } as *const i8;
            if utf8.is_null() {
                return None;
            }
            let s = unsafe { std::ffi::CStr::from_ptr(utf8) }
                .to_string_lossy()
                .into_owned();
            if s.chars().all(|c| c.is_control()) {
                None
            } else {
                Some(Event::Text(s))
            }
        }
        _ => None,
    }
}

// ---- Shared framebuffer -----------------------------------------------------

#[derive(Default)]
struct MacCtx {
    fb: Vec<u8>, // straight RGBA8, top row first
    w: usize,
    h: usize,
    view: Id,
    /// Accessible label exposed to NSAccessibility (the window title for now).
    a11y_label: String,
}

// Cocoa is single-threaded (main thread); the backend runs there.
thread_local! {
    static CTX: RefCell<MacCtx> = RefCell::new(MacCtx::default());
}

/// `drawRect:` implementation for our NSView subclass: blit the framebuffer.
extern "C" fn draw_rect(_this: Id, _cmd: Sel, _dirty: CgRect) {
    CTX.with(|c| {
        let c = c.borrow();
        if c.fb.is_empty() {
            return;
        }
        unsafe {
            // Current CGContext: [[NSGraphicsContext currentContext] CGContext]
            let gctx_cls = class("NSGraphicsContext");
            let current = msg_id(gctx_cls, sel("currentContext"));
            if current.is_null() {
                return;
            }
            let cg = msg_id(current, sel("CGContext"));
            if cg.is_null() {
                return;
            }

            let space = CGColorSpaceCreateDeviceRGB();
            let provider = CGDataProviderCreateWithData(
                std::ptr::null_mut(),
                c.fb.as_ptr() as *const c_void,
                c.fb.len(),
                std::ptr::null(),
            );
            let image = CGImageCreate(
                c.w,
                c.h,
                8,
                32,
                c.w * 4,
                space,
                CG_BITMAP_RGBA8,
                provider,
                std::ptr::null(),
                false,
                0,
            );
            let rect = CgRect {
                origin: CgPoint { x: 0.0, y: 0.0 },
                size: CgSize {
                    width: c.w as f64,
                    height: c.h as f64,
                },
            };
            // The view is `isFlipped` (top-left origin), but CGContextDrawImage
            // interprets image data as Y=0-at-bottom, so without compensation
            // the framebuffer lands upside down. Flip the CTM around the draw —
            // the same fix the x11anywhere macOS backend uses.
            CGContextSaveGState(cg);
            CGContextTranslateCTM(cg, 0.0, c.h as f64);
            CGContextScaleCTM(cg, 1.0, -1.0);
            CGContextDrawImage(cg, rect, image);
            CGContextRestoreGState(cg);
            CGImageRelease(image);
            CGDataProviderRelease(provider);
            CGColorSpaceRelease(space);
        }
    });
}

/// `isFlipped` => YES, so the view's coordinate origin is top-left and our
/// top-row-first framebuffer draws upright.
extern "C" fn is_flipped(_this: Id, _cmd: Sel) -> bool {
    true
}

// ---- NSAccessibility ----------------------------------------------------
//
// Forma draws every control itself, so to AppKit the window is one opaque view.
// We override the NSAccessibility protocol methods on our view to vend semantic
// info (the macOS half of the cross-platform a11y bridge; AT-SPI is the Linux
// half). This first step exposes the view as an accessible group with the
// window's title as its label; exposing the full element tree builds on it.

/// `isAccessibilityElement` => YES (the view is a real accessibility element).
extern "C" fn acc_is_element(_this: Id, _cmd: Sel) -> bool {
    true
}

/// `accessibilityRole` => `NSAccessibilityGroupRole` ("AXGroup"): a self-drawn
/// container.
extern "C" fn acc_role(_this: Id, _cmd: Sel) -> Id {
    nsstring("AXGroup")
}

/// `accessibilityLabel` => the window's accessible label.
extern "C" fn acc_label(_this: Id, _cmd: Sel) -> Id {
    CTX.with(|c| nsstring(&c.borrow().a11y_label))
}

/// Copy an `NSString` out to a Rust `String` (via `-UTF8String`).
fn nsstring_to_string(s: Id) -> String {
    if s.is_null() {
        return String::new();
    }
    unsafe {
        let f: unsafe extern "C" fn(Id, Sel) -> *const i8 =
            std::mem::transmute(objc_msgSend as *const c_void);
        let p = f(s, sel("UTF8String"));
        if p.is_null() {
            String::new()
        } else {
            std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned()
        }
    }
}

fn register_view_class() -> Class {
    let name = c"FormaView";
    unsafe {
        let existing = objc_getClass(name.as_ptr());
        if !existing.is_null() {
            return existing;
        }
        let superclass = class("NSView");
        let cls = objc_allocateClassPair(superclass, name.as_ptr(), 0);
        let draw_types = c"v@:{CGRect={CGPoint=dd}{CGSize=dd}}";
        class_addMethod(
            cls,
            sel("drawRect:"),
            draw_rect as *const c_void,
            draw_types.as_ptr(),
        );
        let flip_types = c"c@:";
        class_addMethod(
            cls,
            sel("isFlipped"),
            is_flipped as *const c_void,
            flip_types.as_ptr(),
        );
        // NSAccessibility overrides: BOOL (c@:) and id (@@:) returns.
        let bool_types = c"c@:";
        let id_types = c"@@:";
        class_addMethod(
            cls,
            sel("isAccessibilityElement"),
            acc_is_element as *const c_void,
            bool_types.as_ptr(),
        );
        class_addMethod(
            cls,
            sel("accessibilityRole"),
            acc_role as *const c_void,
            id_types.as_ptr(),
        );
        class_addMethod(
            cls,
            sel("accessibilityLabel"),
            acc_label as *const c_void,
            id_types.as_ptr(),
        );
        objc_registerClassPair(cls);
        cls
    }
}

#[derive(Debug)]
struct MacWindow {
    size: PhysicalSize,
}

impl Window for MacWindow {
    fn id(&self) -> WindowId {
        WindowId(1)
    }
    fn inner_size(&self) -> PhysicalSize {
        self.size
    }
    fn scale_factor(&self) -> ScaleFactor {
        ScaleFactor::IDENTITY
    }
    fn request_redraw(&self) {
        CTX.with(|c| {
            let view = c.borrow().view;
            if !view.is_null() {
                unsafe { msg_void_bool(view, sel("setNeedsDisplay:"), true) };
            }
        });
    }
    fn set_title(&self, _title: &str) {}
    fn create_surface(&self) -> Box<dyn Surface> {
        Box::new(MacSurface { size: self.size })
    }
}

#[derive(Debug)]
struct MacSurface {
    size: PhysicalSize,
}

impl Surface for MacSurface {
    fn resize(&mut self, size: PhysicalSize) {
        self.size = size;
    }
    fn size(&self) -> PhysicalSize {
        self.size
    }
    fn present(&mut self, pixmap: &Pixmap, _damage: &[forma_geometry::Rect]) {
        let size = pixmap.size();
        CTX.with(|c| {
            let mut c = c.borrow_mut();
            c.fb = pixmap.as_bytes().to_vec();
            c.w = size.width as usize;
            c.h = size.height as usize;
            if !c.view.is_null() {
                unsafe { msg_void_bool(c.view, sel("setNeedsDisplay:"), true) };
            }
        });
    }
}

/// Create an NSWindow with our drawing view, render the initial frame, and run
/// the Cocoa event loop. `[NSApp run]` does not return, so close/exit is via
/// the process being terminated (input handling is a follow-up).
pub fn run<H>(attrs: WindowAttributes, mut handler: H) -> Result<(), PlatformError>
where
    H: FnMut(Event, &dyn Window) -> ControlFlow,
{
    let size = ScaleFactor::IDENTITY.to_physical(attrs.logical_size);
    unsafe {
        let app = msg_id(class("NSApplication"), sel("sharedApplication"));
        if app.is_null() {
            return Err(PlatformError::Os("NSApplication unavailable".into()));
        }
        msg_void_i64(app, sel("setActivationPolicy:"), ACTIVATION_REGULAR);

        let content = CgRect {
            origin: CgPoint { x: 0.0, y: 0.0 },
            size: CgSize {
                width: size.width as f64,
                height: size.height as f64,
            },
        };

        // window = [[NSWindow alloc] initWithContentRect:styleMask:backing:defer:]
        let window: Id = {
            let alloc = msg_id(class("NSWindow"), sel("alloc"));
            let init: unsafe extern "C" fn(Id, Sel, CgRect, u64, u64, bool) -> Id =
                std::mem::transmute(objc_msgSend as *const c_void);
            init(
                alloc,
                sel("initWithContentRect:styleMask:backing:defer:"),
                content,
                NS_WINDOW_STYLE_TITLED | NS_WINDOW_STYLE_CLOSABLE | NS_WINDOW_STYLE_RESIZABLE,
                NS_BACKING_BUFFERED,
                false,
            )
        };
        if window.is_null() {
            return Err(PlatformError::Os("NSWindow init failed".into()));
        }

        // view = [[FormaView alloc] initWithFrame:content]
        let view_cls = register_view_class();
        let view: Id = {
            let alloc = msg_id(view_cls, sel("alloc"));
            let init: unsafe extern "C" fn(Id, Sel, CgRect) -> Id =
                std::mem::transmute(objc_msgSend as *const c_void);
            init(alloc, sel("initWithFrame:"), content)
        };
        msg_void_id(window, sel("setContentView:"), view);
        CTX.with(|c| {
            let mut c = c.borrow_mut();
            c.view = view;
            // Expose the window title as the view's accessible label.
            c.a11y_label = attrs.title.clone();
        });

        // Self-check the NSAccessibility wiring (real objc dispatch) for CI:
        // cross-process AX reads need TCC approval the runner won't grant, so we
        // query our own view's accessibility attributes and print them.
        if std::env::var("FORMA_COCOA_A11Y").is_ok() {
            let role = nsstring_to_string(msg_id(view, sel("accessibilityRole")));
            let label = nsstring_to_string(msg_id(view, sel("accessibilityLabel")));
            let is_el: bool = {
                let f: unsafe extern "C" fn(Id, Sel) -> bool =
                    std::mem::transmute(objc_msgSend as *const c_void);
                f(view, sel("isAccessibilityElement"))
            };
            println!("Cocoa a11y: element={is_el} role={role} label={label}");
        }

        let mut win = MacWindow { size };
        handler(Event::RedrawRequested, &win); // populate the framebuffer

        msg_void_id(window, sel("center"), std::ptr::null_mut());
        msg_void_id(window, sel("makeKeyAndOrderFront:"), std::ptr::null_mut());
        msg_void_bool(app, sel("activateIgnoringOtherApps:"), true);
        // Needed for nextEventMatchingMask to dequeue events.
        msg_void(app, sel("finishLaunching"));

        // Manual event loop: pull each NSEvent, translate it to a Forma event
        // for the handler, then forward it to AppKit (drawing, window controls).
        // Poll the content view's bounds for live-resize.
        let distant_future = msg_id(class("NSDate"), sel("distantFuture"));
        let mode = nsstring("kCFRunLoopDefaultMode");
        let next_sel = sel("nextEventMatchingMask:untilDate:inMode:dequeue:");
        let next: unsafe extern "C" fn(Id, Sel, u64, Id, Id, bool) -> Id =
            std::mem::transmute(objc_msgSend as *const c_void);
        let mut last = win.size;
        loop {
            let ev = next(app, next_sel, u64::MAX, distant_future, mode, true);
            if !ev.is_null() {
                let etype = msg_u64(ev, sel("type"));
                if let Some(event) = translate_event(etype, ev, last.height as f64) {
                    if handler(event, &win) == ControlFlow::Exit {
                        break;
                    }
                }
                msg_void_id(app, sel("sendEvent:"), ev);
            }
            // Detect live-resize by polling the view bounds.
            let b = msg_rect(view, sel("bounds"));
            let now =
                PhysicalSize::new(b.size.width.max(1.0) as u32, b.size.height.max(1.0) as u32);
            if now != last {
                last = now;
                win.size = now;
                handler(Event::Resized(now), &win);
            }
        }
    }
    Ok(())
}
