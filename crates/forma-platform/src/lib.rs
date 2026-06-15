//! Forma's platform layer.
//!
//! This is the **only** crate with per-OS code. It owns native windowing,
//! input, IME, clipboard, DPI, vsync, and surface acquisition, and exposes
//! them through a small platform-neutral vocabulary: [`Event`], [`Window`],
//! [`WindowAttributes`], and a [`ControlFlow`]-driven loop. Everything above
//! this layer (`forma-core`, widgets, …) is free of platform `cfg`s.
//!
//! Backends live under [`backend`] and are selected by `cfg`. The
//! [`backend::headless`] backend is always available and implements the full
//! contract without touching any OS API — it powers tests and the
//! golden-image conformance suite. Native backends are roadmap Phases 1–4.

#![forbid(unsafe_code)]

pub mod backend;
mod error;
mod event;
mod window;

pub use error::PlatformError;
pub use event::{ButtonState, Event, KeyCode, Modifiers, PointerButton, ScrollDelta, WindowId};
pub use window::{Window, WindowAttributes};

/// What a window should do after the application handles an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlFlow {
    /// Continue running; wake again as soon as more events arrive.
    Poll,
    /// Continue running; sleep until the next event.
    Wait,
    /// Tear down the window / event loop.
    Exit,
}
