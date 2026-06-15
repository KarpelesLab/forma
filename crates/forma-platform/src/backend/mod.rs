//! Backend selection.
//!
//! Each supported OS gets a module here that implements [`Window`](crate::Window)
//! and an event loop against that platform's lowest idiomatic interface. Only
//! one backend is active per build, chosen by `cfg`. Today only the
//! [`headless`] backend is implemented; the native backends are the subject of
//! roadmap Phases 1–4:
//!
//! - **`linux`** — Wayland (`wl_shm`) primary, X11 (MIT-SHM) fallback.
//! - **`macos`** — AppKit (`NSWindow`/`NSView`, `CVDisplayLink`).
//! - **`windows`** — Win32 (`HWND`, GDI/DXGI blit).
//! - **`android`** — NDK `NativeActivity` / `ANativeWindow`.
//! - **`ios`** — UIKit (`CADisplayLink`, `CALayer`).
//! - **`web`** — `<canvas>` + `putImageData`.
//!
//! [`headless`] is always available so the rest of the stack builds, runs, and
//! is golden-image testable on any target. On Linux a native [`x11`] backend is
//! also available; [`run`] selects it when `$DISPLAY` is set and falls back to
//! headless otherwise.

use crate::ControlFlow;
use crate::event::Event;
use crate::window::{Window, WindowAttributes};

pub mod headless;

#[cfg(target_os = "linux")]
pub mod x11;

#[cfg(target_os = "windows")]
pub mod windows;

/// Run `handler` against the best available native backend, falling back to a
/// one-shot [`headless`] present when no display is reachable.
///
/// The handler receives platform-neutral [`Event`]s and the live [`Window`];
/// returning [`ControlFlow::Exit`] tears the loop down.
pub fn run<H>(attrs: WindowAttributes, mut handler: H)
where
    H: FnMut(Event, &dyn Window) -> ControlFlow,
{
    #[cfg(target_os = "linux")]
    {
        if x11::available() {
            match x11::run(attrs.clone(), &mut handler) {
                Ok(()) => return,
                Err(err) => {
                    eprintln!("forma: X11 backend unavailable ({err}); falling back to headless");
                }
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        match windows::run(attrs.clone(), &mut handler) {
            Ok(()) => return,
            Err(err) => {
                eprintln!("forma: Windows backend unavailable ({err}); falling back to headless");
            }
        }
    }
    // Headless fallback: present one frame, then close.
    let _ = headless::run(
        attrs,
        [Event::RedrawRequested, Event::CloseRequested],
        handler,
    );
}
