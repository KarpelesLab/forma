# Forma

A cross-platform UI library and toolkit in Rust.

Forma draws **beautiful, fully themeable, pixel-identical** interfaces on Linux,
macOS, Windows, Android, iOS, and the web — staying **as close to the OS as
possible** while depending on **as little third-party code as possible**.

It builds on the pure-Rust [`oxideav`](https://github.com/OxideAV) media stack
for all 2D content rendering (scene graph, CPU rasterizer, font shaping, image
decode, SVG) and adds everything around it: native windowing and input per OS,
presenting the rendered buffer, and a declarative, reactive UI toolkit.

```rust
use forma::prelude::*;

struct Counter { n: i64 }

fn view(state: &Counter) -> impl View {
    Column((
        Text(format!("{}", state.n)).font_size(48.0),
        Row((
            Button("−").on_tap(|s: &mut Counter| s.n -= 1),
            Button("+").on_tap(|s: &mut Counter| s.n += 1),
        )).gap(8.0),
    ))
    .padding(24.0)
}

fn main() {
    forma::App::new(Counter { n: 0 }, view)
        .title("Counter")
        .run();
}
```

> **Status: pre-alpha / scaffolding.** The architecture and phased plan live in
> [`ROADMAP.md`](./ROADMAP.md). APIs are unstable and largely unimplemented.

## Design at a glance

- **Software-first rendering** behind a GPU-ready `Surface` seam (raw
  Metal/Vulkan/D3D12/WebGPU later — never wgpu).
- **Reactive / declarative** API: UI is a function of state.
- **Self-drawn widgets** — one theme engine, identical on every platform.
- **No** `winit` / `wgpu` / `taffy` / `lyon` / GTK / Qt. OS interfaces are
  hand-written per platform in `forma-platform`.

## Workspace layout

| Crate | Role |
|---|---|
| `forma-geometry` | Logical-pixel math (Point, Size, Rect, Affine) |
| `forma-render` | Scene → oxideav `VectorFrame` → raster → `Surface` |
| `forma-platform` | Per-OS windowing, input, IME, clipboard, vsync |
| `forma-layout` | Flex/box layout solver |
| `forma-core` | Reactive runtime: `View`, reconcile, state, events |
| `forma-anim` | Frame clock, easing, springs, transitions |
| `forma-style` | Design tokens and themes |
| `forma-widgets` | Standard widget library |
| `forma` | Umbrella crate: `App`, prelude, re-exports |

## Examples

```sh
cargo run -p gallery            # renders a themed settings panel to forma-gallery.png
```

The `gallery` example exercises the whole scaffolded stack — widgets →
`forma-core` layout/paint → `forma-render` scene → `oxideav-raster` → pixel
buffer — and encodes the result with `oxideav-png`. It renders off-screen (no
window yet; the native event loop is a later roadmap phase) via
`App::render_once`.

## Status & MSRV

Pre-alpha scaffolding. The library crates build on **Rust 1.86** (edition
2024); the `gallery` example pulls a newer transitive dependency through
`oxideav-png` and is excluded from the MSRV check. What works today:

- the full **build → layout → paint → rasterize → present** path, end to end,
  verified against the headless backend;
- self-drawn, themeable, anti-aliased surfaces (panels, rows/columns, buttons,
  dividers, swatches) with flex layout and DPI scaling.

Next milestones (see `ROADMAP.md`): the `forma-core` reactive runtime (state,
reconcile, event dispatch, hit-testing), the `oxideav-scribe` text-rendering
bridge, and the native Wayland/X11 platform backend.

## License

MIT

