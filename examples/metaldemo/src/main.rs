//! Exercises the raw Metal backend in `forma-gpu` and prints what it created.
//! The macOS visual CI job builds this with `--features forma-gpu/mtl` and runs
//! it, grepping the output to confirm a Metal device came up.
//!
//! Without the `mtl` feature (or off macOS) the entry point returns an error,
//! which is printed non-fatally so the demo still exits cleanly.

fn main() {
    match forma_gpu::metal_device() {
        Ok(name) => println!("Metal device: {name}"),
        Err(e) => println!("Metal unavailable: {e}"),
    }
}
