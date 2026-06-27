# Stipple

A cross-platform UI library and toolkit in Rust.

Stipple draws **beautiful, fully themeable, pixel-identical** interfaces on Linux,
macOS, Windows, Android, iOS, and the web — staying **as close to the OS as
possible** while depending on **as little third-party code as possible**.

It builds on the pure-Rust [`oxideav`](https://github.com/OxideAV) media stack
for all 2D content rendering (scene graph, CPU rasterizer, font shaping, image
decode, SVG) and adds everything around it: native windowing and input per OS,
presenting the rendered buffer, and a declarative, reactive UI toolkit.

```rust
use stipple::prelude::*;

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
    stipple::App::new(Counter { n: 0 }, view)
        .title("Counter")
        .run();
}
```

> **Status: pre-alpha.** The architecture and phased plan live in
> [`ROADMAP.md`](./ROADMAP.md). APIs are unstable.

The same app, rendered by Stipple's **native backends** and screenshotted in CI
(the `Visual` workflow) — Linux X11 (under Xvfb) and Wayland (under headless
sway), Win32, and Cocoa, each from-scratch with no windowing crates:

| Linux / X11 | Linux / Wayland | Windows / Win32 | macOS / Cocoa |
|---|---|---|---|
| ![X11](./docs/screenshots/stipple-x11.png) | ![Wayland](./docs/screenshots/stipple-wayland.png) | ![Windows](./docs/screenshots/stipple-windows.png) | ![macOS](./docs/screenshots/stipple-macos.png) |

| Web / wasm + canvas | GPU / GLES (offscreen) |
|---|---|
| ![Web](./docs/screenshots/stipple-web.png) | ![GPU](./docs/screenshots/stipple-gpu.png) |

Input is verified too: CI synthesizes real events and screenshots the result —
X11 via `xdotool` (clicking a counter; caret-aware editing — type "Stipple",
arrow-left twice, insert "XY" → "ForXYma" with a mid-string caret), macOS via
`cliclick`, and **Wayland** via `wtype` (whose virtual keyboard exercises the
`wl_seat` keyboard + xkb-keymap decode path — Tab to focus, then type "stipple
wl"):

| X11 click | X11 edit (mid-string caret) | macOS click | Wayland type |
|---|---|---|---|
| ![clicks](./docs/screenshots/stipple-x11-clicks.png) | ![typing](./docs/screenshots/stipple-x11-textinput.png) | ![mac clicks](./docs/screenshots/stipple-macos-clicks.png) | ![wl type](./docs/screenshots/stipple-wayland-input.png) |

## Themeable by design

Every widget reads its colors and metrics from a [`Theme`] — a semantic
`Palette` (roles, interaction states, status colors, overlays), a `Typography`
scale, a `Spacing` scale, and a corner radius. Customizing is a one-liner:
`Theme::dark().with_accent(color).with_radius(14.0)` recolors the accent,
derives its hover/active tints, and picks a readable on-color automatically;
`high_contrast()` maximizes text/border contrast. The same card below is
rendered under four themes (light, dark, a violet-accent dark, high-contrast),
montaged in CI:

![themes](./docs/screenshots/stipple-themes.png)

## Design at a glance

- **Software-first rendering** behind a GPU-ready `Surface` seam (raw
  Metal/Vulkan/D3D12/WebGPU later — never wgpu).
- **Reactive / declarative** API: UI is a function of state.
- **Self-drawn widgets** — one theme engine, identical on every platform.
- **No** `winit` / `wgpu` / `taffy` / `lyon` / GTK / Qt. OS interfaces are
  hand-written per platform in `stipple-platform`.

## Workspace layout

| Crate | Role |
|---|---|
| `stipple-geometry` | Logical-pixel math (Point, Size, Rect, Affine) |
| `stipple-render` | Scene → oxideav `VectorFrame` → raster → `Surface` |
| `stipple-platform` | Per-OS windowing, input, IME, clipboard, vsync |
| `stipple-layout` | Flex/box layout solver |
| `stipple-core` | Reactive runtime: `View`, reconcile, state, events |
| `stipple-anim` | Frame clock, easing, springs, transitions |
| `stipple-style` | Design tokens and themes |
| `stipple-widgets` | Standard widget library |
| `stipple` | Umbrella crate: `App`, prelude, re-exports |

## Examples

```sh
cargo run -p window       # a settings panel in a native window
cargo run -p clickdemo    # a click-counting button
cargo run -p textinput    # an editable text field (Tab to focus, then type)
cargo run -p themegallery # one card rendered under four themes (writes .raw files)
```

Each opens a real native window (X11/Win32/Cocoa) via `App::run`, or falls back
to a one-shot headless render where no display is available. The web target
lives in `crates/stipple-web` (built for `wasm32`; see the `Visual` workflow).

## Status & MSRV

Pre-alpha. The whole workspace builds on **Rust 1.88** (edition 2024).

Working today: the full reactive toolkit (render, layout, state, tap/keyboard/
focus/drag, text, 12 widgets, theming) and **four rendering targets** — native
X11, Win32, and Cocoa (window + input + resize) plus **web** (wasm + canvas) —
each verified by a CI screenshot on its platform.

Next milestones (see `ROADMAP.md`): Wayland, mobile (Android/iOS), and GPU
backends; web font + canvas input.

## License

MIT

