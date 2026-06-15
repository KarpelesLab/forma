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
//! Until those land, [`headless`] is the default everywhere so the rest of the
//! stack builds, runs, and is golden-image testable on any target.

pub mod headless;
