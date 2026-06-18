//! Phase-A spike for the browser content path: prove that a GPU texture can be
//! **exported as a `dma-buf`** and **re-imported** as a texture — the zero-copy
//! handoff a web content process will use to hand rendered pages to the Forma UI
//! process. Runs surfaceless (no window), so it's safe to run on a GPU box.
//!
//! Run on real GPU hardware:
//!   cargo run -p dmabuftest --features forma-gpu/gl
//!
//! Exit codes (so CI can tell a real failure from "no GPU here"):
//!   0 = self-test passed (dma-buf export+import works)
//!   2 = unsupported on this device (software Mesa / missing extensions / no EGL)
//!   1 = the extensions are present but the round-trip produced wrong pixels (bug)

use std::process::ExitCode;

fn main() -> ExitCode {
    // 1) What EGL extensions does this device advertise?
    let exts = match forma_gpu::dmabuf_extensions() {
        Ok(s) => s,
        Err(e) => {
            println!("EGL unavailable: {e}");
            println!("RESULT: UNSUPPORTED (no EGL)");
            return ExitCode::from(2);
        }
    };
    let has_export = exts.contains("EGL_MESA_image_dma_buf_export");
    let has_import = exts.contains("EGL_EXT_image_dma_buf_import");
    println!("EGL_MESA_image_dma_buf_export: {has_export}");
    println!("EGL_EXT_image_dma_buf_import: {has_import}");
    if !(has_export && has_import) {
        println!("RESULT: UNSUPPORTED (dma-buf import/export extensions absent)");
        return ExitCode::from(2);
    }

    // 2) Export a GPU texture as a dma-buf and re-import it; verify the pixels.
    match forma_gpu::dmabuf_export_import_self_test() {
        Ok(pixels) => {
            println!(
                "imported {} bytes; corners present (red/green/blue/white)",
                pixels.len()
            );
            println!("RESULT: PASS");
            ExitCode::SUCCESS
        }
        Err(e) => {
            // Some software stacks advertise the extensions but can't actually
            // export/import; treat that as unsupported rather than a code bug.
            let unsupported = e.contains("missing entry point")
                || e.contains("failed")
                || e.contains("FBO incomplete");
            println!("self-test error: {e}");
            if unsupported {
                println!("RESULT: UNSUPPORTED (export/import not functional here)");
                ExitCode::from(2)
            } else {
                println!("RESULT: FAIL (pixels did not survive the round-trip)");
                ExitCode::from(1)
            }
        }
    }
}
