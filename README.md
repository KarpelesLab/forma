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

## License

MIT
