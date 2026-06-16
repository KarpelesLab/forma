//! Exercises the raw Metal backend in `forma-gpu` and prints what it created.
//! The macOS visual CI job builds this with `--features forma-gpu/mtl` and runs
//! it, grepping the output to confirm a Metal device came up.
//!
//! Without the `mtl` feature (or off macOS) the entry point returns an error,
//! which is printed non-fatally so the demo still exits cleanly.

const W: u32 = 420;
const H: u32 = 300;

fn main() {
    match forma_gpu::metal_device() {
        Ok(name) => println!("Metal device: {name}"),
        Err(e) => println!("Metal unavailable: {e}"),
    }
    // Render a frame on the GPU and dump the read-back RGBA. We print the
    // top-left pixel so CI can confirm the cleared color tool-free (the macOS
    // runner has no guaranteed image tooling), and still write the raw buffer as
    // an artifact.
    match forma_gpu::metal_render_clear(W, H) {
        Ok(pixels) => {
            std::fs::write("metal-clear.raw", &pixels).expect("write raw");
            let px = &pixels[0..4];
            println!(
                "Metal readback: {} bytes ({W}x{H}) first pixel {},{},{},{}",
                pixels.len(),
                px[0],
                px[1],
                px[2],
                px[3]
            );
        }
        Err(e) => println!("Metal readback unavailable: {e}"),
    }
}
