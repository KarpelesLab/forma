//! Phase-B integration probe for the DRI3 + Present GPU-present path. Connects
//! to the X server over Forma's raw X11 socket, negotiates DRI3, and receives
//! the server's DRM device fd via SCM_RIGHTS (B.2); then binds Forma's GPU
//! context to *that* device via GBM and round-trips a dma-buf through it (B.3) —
//! proving the X server's own GPU can export and re-import the buffers the
//! compositor will hand it. Maps no window (DRI3Open targets the root).
//!
//! Run on a machine with a real GPU + X server (DRI3):
//!   cargo run -p dri3probe --features forma-gpu/gl
//!
//! Expected on GPU+X: the DRM fd, then "device dma-buf round-trip: PASS".

use std::process::ExitCode;

fn main() -> ExitCode {
    #[cfg(target_os = "linux")]
    {
        let fd = match forma_platform::backend::x11::dri3_open_drm_fd() {
            Ok(Some(fd)) => {
                println!("DRI3Open ok: DRM device fd = {fd}");
                fd
            }
            Ok(None) => {
                println!("DRI3 unavailable on this server (Xvfb has no DRM)");
                return ExitCode::from(2);
            }
            Err(e) => {
                eprintln!("DRI3 probe error: {e}");
                return ExitCode::from(2);
            }
        };

        // Bind the server's GPU via GBM and round-trip a dma-buf on it.
        match forma_gpu::dmabuf_self_test_on_device(fd) {
            Ok(px) => {
                println!("device dma-buf round-trip: PASS ({} bytes)", px.len());
                ExitCode::SUCCESS
            }
            Err(e) => {
                // Without the gl feature this returns an explanatory error.
                println!("device dma-buf round-trip: {e}");
                ExitCode::from(1)
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        println!("dri3probe: Linux/X11 only");
        ExitCode::SUCCESS
    }
}
