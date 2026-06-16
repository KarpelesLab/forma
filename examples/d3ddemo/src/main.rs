//! Exercises the raw Direct3D 11 backend in `forma-gpu` and prints what it
//! created. The Windows visual CI job builds this with `--features
//! forma-gpu/d3d` and runs it against **WARP** (Windows' software rasterizer),
//! grepping the output to confirm the D3D device came up.
//!
//! Without the `d3d` feature (or off Windows) the entry point returns an error,
//! which is printed non-fatally so the demo still exits cleanly.

fn main() {
    match forma_gpu::d3d11_device() {
        Ok(summary) => println!("D3D11 device: {summary}"),
        Err(e) => println!("D3D11 unavailable: {e}"),
    }
}
