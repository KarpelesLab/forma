//! Phase-B probe for the DRI3 + Present GPU-present path: connect to the X
//! server over Forma's raw X11 socket, negotiate DRI3, and receive the server's
//! DRM device fd via SCM_RIGHTS. Maps no window (DRI3Open targets the root), so
//! it's a read-only query safe to run against a live session.
//!
//! Run on a machine with a real GPU + X server (DRI3):
//!   cargo run -p dri3probe
//!
//! Expected on GPU+X: "DRI3Open ok: DRM device fd = N". Under Xvfb (no DRM) it
//! reports DRI3 unavailable.

use std::process::ExitCode;

fn main() -> ExitCode {
    #[cfg(target_os = "linux")]
    {
        match forma_platform::backend::x11::dri3_open_probe() {
            Ok(msg) => {
                println!("{msg}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("dri3 probe error: {e}");
                ExitCode::from(2)
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        println!("dri3probe: Linux/X11 only");
        ExitCode::SUCCESS
    }
}
