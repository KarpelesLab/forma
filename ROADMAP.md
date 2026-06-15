# Forma — Roadmap

A cross-platform UI library and toolkit in Rust. Forma draws **beautiful, fully
themeable, pixel-identical** interfaces on Linux, macOS, Windows, Android, iOS,
and the web, while staying **as close to the OS as possible** and depending on
**as little third-party code as possible**.

Forma does not reinvent 2D content rendering — it builds on the
[`oxideav`](../oxideav-workspace) workspace, a mature pure-Rust media stack that
already provides the entire "vector + text + images → pixel buffer" pipeline.
Forma adds everything *around* that: native windowing and input per OS, getting
the buffer onto the screen, and the declarative UI toolkit itself.

---

## 0. Implementation status

> Living checklist — updated as work lands. ✅ done · 🚧 in progress · ⬜ not started.

- ✅ **Workspace + 9 crates** scaffolded (edition 2024, rust 1.86), CI (lint +
  MSRV), `forma-geometry`.
- ✅ **Rendering seam** (`forma-render`): `Scene` → oxideav `VectorFrame` →
  `oxideav-raster` → `Pixmap`; `Surface` GPU-ready boundary.
- ✅ **Software rasterization** path verified end to end (off-screen PNGs).
- ✅ **Layout** (`forma-layout`): flex/box solver. **Paint**: `Element` IR +
  measure/layout/paint passes.
- ✅ **Reactivity MVP** (`forma-core`): retained `LayoutNode` tree, `hit_test`,
  `Cx` handler registry, `on_tap` dispatch → state mutation (the `counter`
  example drives clicks through the real path).
- ✅ **Theming** (`forma-style`) and **animation primitives** (`forma-anim`).
- ✅ **Text rendering** via `oxideav-scribe`: `Font` (load + measure), glyph
  shaping → scene nodes with per-run color; `Text` element threaded through
  layout (intrinsic sizing) and paint; `label`/`button_labeled` widgets.
- ✅ **Widgets** (scaffold): panel, row/column, button, labeled button, label,
  divider, swatch, spacer.
- 🚧 **Platform layer**: headless backend only (full vocabulary + golden-image
  probe). Native backends pending.
- ⬜ **native Wayland/X11 backend**; ⬜ reconciliation/diffing; ⬜ focus +
  keyboard; ⬜ multi-line/editable text; ⬜ richer widgets; ⬜ a11y;
  ⬜ macOS/Windows; ⬜ mobile; ⬜ web; ⬜ GPU backends.

---

## 1. Guiding principles

1. **Minimal third-party dependencies.** No `winit`, `wgpu`, `taffy`, `lyon`,
   `tiny-skia`, `cosmic-text`, GTK/Qt, etc. We talk to each OS directly through
   its lowest stable, idiomatic interface, and we own our layout, reactivity,
   and rendering glue. oxideav crates are first-party (same author/ecosystem)
   and are the one sanctioned heavy dependency.
2. **Close to the OS.** The OS provides only what only it can: a window, an
   input stream, a presentable surface, clipboard, IME, and an accessibility
   bridge. Everything visual is ours.
3. **Self-drawn, pixel-identical, themeable.** Every control is drawn by Forma
   via `oxideav-raster`. One theme engine, one look, identical across every
   platform — and completely customizable.
4. **Declarative & reactive.** The public API is state-driven (SwiftUI / Jetpack
   Compose style): UI is a function of state; the runtime diffs and updates.
5. **Software-first rendering, GPU later.** v1 rasterizes on the CPU with
   `oxideav-raster` and blits to a native surface. A GPU backend lands later
   behind a stable `Surface` abstraction — without wgpu, using raw
   Metal / Vulkan / D3D12 / WebGPU.
6. **Portability is a layering discipline.** All OS-specific code lives behind
   `forma-platform`. The rest of the stack is `#![forbid]`-clean of platform
   `cfg`s and is tested headlessly with golden images.

### Non-negotiable dependency policy

| Concern | Forma's answer |
|---|---|
| 2D vector rasterization | `oxideav-raster` |
| Scene graph / primitives | `oxideav-core` (`VectorFrame`, `Node`, `Group`, `Transform2D`) |
| Font parsing + shaping | `oxideav-ttf`, `oxideav-otf`, `oxideav-scribe` |
| Image decode | `oxideav-png` + sibling codecs |
| SVG (icons) | `oxideav-svg` |
| Pixel conversion / blit prep | `oxideav-pixfmt` |
| Windowing, input, IME, clipboard | **Forma, hand-written per OS** |
| Layout, reactivity, widgets, theming, animation | **Forma** |

---

## 2. Architecture

Layered, bottom-up. Each layer is a crate (or a small crate group) in a single
Cargo workspace. Forma targets **edition 2024, rust 1.86**, `version = 0.0.x`,
pure-Rust. (oxideav itself is edition 2021 / rust 1.80; Forma consumes it as a
dependency but builds on the newer toolchain.)

```
                 ┌─────────────────────────────────────────┐
   app facade    │  forma  (umbrella: App, prelude, re-exports)
                 └─────────────────────────────────────────┘
   widgets       │  forma-widgets   Text Button TextField Stack Row Column …
   styling       │  forma-style     design tokens, themes, typography scales
   animation     │  forma-anim      clock, easing, springs, transitions
                 ├─────────────────────────────────────────┤
   runtime       │  forma-core      View trait · element tree · reconcile ·
                 │                  state/signals · events · focus · hit-test
   layout        │  forma-layout    fl/box layout solver, intrinsic sizing
                 ├─────────────────────────────────────────┤
   rendering     │  forma-render    Scene builder → oxideav VectorFrame ·
                 │                  Surface trait · software backend · text run
                 │                  shaping bridge · damage/dirty regions ·
                 │                  layer + glyph caches
   platform      │  forma-platform  windows · event loop · input · IME ·
                 │                  clipboard · DPI · vsync · a11y bridge
   geometry      │  forma-geometry  Point Size Rect Insets Affine (logical px)
                 └─────────────────────────────────────────┘
                       ↑ depends on oxideav-{core,raster,scribe,svg,png,…}
```

### Layer responsibilities

- **`forma-geometry`** — Logical-pixel math: `Point`, `Size`, `Rect`, `Insets`,
  `Affine`. Thin ergonomic layer; converts to/from `oxideav-core`'s
  `Transform2D` and physical pixels at the render boundary. Handles the
  logical↔physical DPI scale factor.

- **`forma-render`** — The seam between the toolkit and oxideav. Builds a
  `Scene` (an `oxideav_core::VectorFrame`) from draw commands; shapes text runs
  through `oxideav-scribe` (`shape` → `PositionedGlyph` →
  `shape_to_paths` → `Node`s); rasterizes via
  `oxideav_raster::Renderer::render` into an Rgba `VideoFrame`; and presents
  through a `Surface` trait. Owns **damage tracking** (only repaint dirty
  rects), **layer caching** (cache subtree raster output), and a **glyph cache**.
  The `Surface` trait is the GPU-readiness seam:
  ```rust
  pub trait Surface {
      fn resize(&mut self, size: PhysicalSize);
      fn present(&mut self, frame: &VideoFrame, damage: &[Rect]);
  }
  ```

- **`forma-platform`** — The only crate with per-OS code, selected by `cfg`.
  Exposes `EventLoop`, `Window`, an input event stream, IME, clipboard, DPI,
  vsync/frame callbacks, lifecycle, and a `Surface` factory. Backends added in
  roadmap order: `linux` (Wayland `wl_shm` first, X11 MIT-SHM fallback),
  `macos` (AppKit), `windows` (Win32), then `android` (NDK), `ios` (UIKit),
  `web` (canvas).

- **`forma-layout`** — Self-contained flex/box layout solver over the element
  tree: main/cross axis, flex grow/shrink/basis, alignment, gap, padding,
  min/max, and **intrinsic sizing** driven by `forma-render` text measurement.
  No `taffy`.

- **`forma-core`** — The reactive runtime ("the Compose/SwiftUI engine"):
  the `View` trait, building an element tree, diff/reconcile against the prior
  tree, fine-grained state (signals/state cells), effect scheduling, event
  dispatch + bubbling, focus management, and hit-testing. Drives layout and
  render each frame.

- **`forma-anim`** — Frame clock (fed by platform vsync), easing curves,
  spring physics, and value transitions wired into the reactive runtime.

- **`forma-style`** — Design tokens, theme definitions (light/dark + custom),
  color systems, typography scales, spacing, elevation/shadow, animation
  defaults. The single source of "the look."

- **`forma-widgets`** — The standard library drawn on top of everything:
  layout (`Row`, `Column`, `Stack`, `Grid`, `Scroll`), content (`Text`,
  `Image`, `Icon` via `oxideav-svg`), input (`Button`, `TextField`, `Checkbox`,
  `Radio`, `Switch`, `Slider`, `Dropdown`), structure (`List`/virtualized,
  `Table`, `Tabs`), overlay (`Menu`, `Popover`, `Tooltip`, `Dialog`).

- **`forma`** — Umbrella crate: `App` builder, prelude, re-exports, examples
  entry point.

- **`forma-a11y`** (lands in Phase 3) — Backend-agnostic accessibility tree
  plus per-OS bridges: AT-SPI (Linux), UI Automation (Windows),
  NSAccessibility (macOS), and the mobile/web equivalents later.

### Reactive API shape (target)

```rust
fn view(state: &Counter) -> impl View {
    Column((
        Text(format!("{}", state.n)).font_size(48.0),
        Row((
            Button("−").on_tap(|s: &mut Counter| s.n -= 1),
            Button("+").on_tap(|s: &mut Counter| s.n += 1),
        )).gap(8.0),
    ))
    .padding(24.0)
    .align(Align::Center)
}

fn main() {
    forma::App::new(Counter { n: 0 }, view)
        .title("Counter")
        .theme(Theme::system())
        .run();
}
```

---

## 3. Phased roadmap

Phases are sequenced for **earliest end-to-end proof, then breadth, then
depth**. Each phase ends with a runnable, demoable deliverable. The GPU track
(Phase 6) is cross-cutting and can start in parallel once the `Surface`
abstraction is frozen at the end of Phase 2.

### Phase 0 — Foundations & de-risking spikes
*Goal: prove the riskiest seam (CPU buffer → screen) before building a toolkit.*

- Cargo workspace scaffold, CI (fmt/clippy/test on Linux), license, oxideav
  dependency wiring (path deps now, versioned later).
- `forma-geometry` core types + `Transform2D` interop.
- **Spike 1 — present path (Linux):** open a Wayland window, allocate a
  `wl_shm` buffer, render a solid `VectorFrame` rect via `oxideav-raster`, blit,
  present, handle resize + close. The "hello rectangle."
- **Spike 2 — text:** shape a string with `oxideav-scribe`, rasterize the glyph
  paths, present. Confirms the scribe→raster→surface chain.
- **Exit criteria:** a window on Linux showing anti-aliased shapes + text from
  oxideav, resizing cleanly.

### Phase 1 — Single-platform vertical slice (Linux)
*Goal: a real, themeable, animated app on one platform — the full stack thin.*

- `forma-platform` Linux backend: Wayland (primary) + X11 (fallback) — window,
  resize, mouse, keyboard, scroll, DPI/scale, frame callbacks (vsync), basic
  clipboard, basic IME.
- `forma-render`: `Scene` builder, software `Surface`, text-run shaping bridge,
  double buffering, damage tracking.
- `forma-core` MVP: `View` trait, element tree, reconcile, signals/state, event
  dispatch, hit-testing, focus.
- `forma-layout`: flex subset (row/column/grow/align/gap/padding) + text
  intrinsic sizing.
- `forma-widgets` MVP: `Row`/`Column`/`Stack`, `Text`, `Button`, basic
  `TextField`, `Image`, `Scroll`.
- `forma-style` MVP theme + `forma-anim` clock/tween/spring.
- **Exit criteria:** demo apps (counter, todo, settings panel) run on Linux,
  themeable (light/dark), with at least one animated transition.

### Phase 2 — Desktop breadth (macOS, Windows)
*Goal: identical apps on all three desktops; freeze the platform/Surface API.*

- `forma-platform` macOS: `NSWindow`/`NSView`, `CVDisplayLink` vsync, blit via
  `CGImage`/`IOSurface`, IME via `NSTextInputClient`, clipboard, per-display
  scale.
- `forma-platform` Windows: `HWND`, `WM_PAINT` + GDI/DXGI blit, raw input, IME
  via TSF/IMM, clipboard, per-monitor-v2 DPI.
- Cross-platform **golden-image conformance suite** (headless render + pixel
  diff) so "pixel-identical" is enforced in CI.
- Multi-window; native menus, file dialogs, and message boxes (thin OS shims);
  HiDPI correctness on all three.
- **Exit criteria:** the Phase 1 demos run unmodified on Linux/macOS/Windows
  with matching golden images. `forma-platform` and `Surface` APIs frozen.

### Phase 3 — Toolkit maturity
*Goal: a toolkit you'd actually ship a product with.*

- Full widget set: virtualized `List`/`Table`, `Tabs`, `Menu`/`Popover`/
  `Tooltip`, `Dialog`, `Slider`, `Checkbox`/`Radio`/`Switch`, `Dropdown`/
  combobox, `Progress`.
- Rich text editing: selection, caret, multi-line, undo/redo, clipboard, **bidi
  + complex-script** input leveraging scribe; font fallback via `FaceChain`.
- **Accessibility** (`forma-a11y`): semantics tree + AT-SPI / UIA /
  NSAccessibility bridges; keyboard navigation + focus traversal.
- Styling depth: full theming/token system, transitions, gesture recognizers.
- i18n: RTL layout, locale-aware formatting, font fallback chains.
- Performance: render thread, layer caching, partial repaint, glyph atlas.
- **Exit criteria:** a non-trivial reference application (e.g., a file/media
  browser using oxideav decoders) ships on all three desktops, accessible and
  localized.

### Phase 4 — Mobile (Android, iOS)
*Goal: the same toolkit on touch.*

- Touch & gesture model, on-screen keyboard, safe areas, density scaling,
  app lifecycle (suspend/resume), back-navigation.
- `forma-platform` Android: NDK `NativeActivity`/`GameActivity`,
  `ANativeWindow` buffer blit, input/IME via minimal JNI, density.
- `forma-platform` iOS: UIKit, `CADisplayLink`, `CALayer`/`CGImage` present,
  `UITextInput` IME, touch, lifecycle.
- **Exit criteria:** a demo app runs on Android + iOS hardware/simulators with
  the same `view` code.

### Phase 5 — Web (WASM)
*Goal: the "maybe" target, made real via the software path.*

- `wasm32-unknown-unknown` target; thin hand-written JS interop (no heavy
  bindgen-driven dep tree where avoidable) for canvas + events.
- Software present via `putImageData`; event/IME/clipboard bridging; DPR
  scaling.
- **Exit criteria:** a demo runs in the browser from the same `view` code.

### Phase 6 — GPU backends (cross-cutting, optional)
*Goal: smooth high-DPI/animation perf without sacrificing the dep policy.*

- Implement `Surface` (and a scene-upload compositor) on raw **Metal** (macOS/
  iOS), **D3D12** (Windows), **Vulkan** (Linux/Android), **WebGPU** (web).
  No wgpu. Glyph/mask atlases on GPU; oxideav-raster remains the CPU fallback.
- **Exit criteria:** GPU backend is a drop-in `Surface` with measurable
  frame-time wins; software path stays the default/fallback.

---

## 4. Cross-cutting tracks (continuous)

- **Testing:** golden-image rendering tests, layout solver unit tests, input/
  event simulation, per-platform smoke tests in CI.
- **Docs & examples:** an examples gallery that doubles as the conformance
  corpus; rustdoc; architecture notes.
- **Tooling (stretch):** a UI inspector/devtools overlay; hot-reload of `view`.
- **Packaging:** app bundling per platform (`.app`, MSIX, APK, IPA, wasm).

---

## 5. Key risks & mitigations

| Risk | Mitigation |
|---|---|
| CPU rasterization too slow for large/animated/4K UIs | Damage tracking + layer/glyph caching in Phase 1; GPU `Surface` (Phase 6) behind the same trait. |
| Per-OS windowing/IME is a deep, hand-written surface | Confine to `forma-platform`; freeze the trait after Phase 2; ship one OS fully before porting. |
| Reactive runtime + Rust ownership friction | Prototype the `View`/state model in Phase 1 against real demos before widening the widget set. |
| Accessibility is hard when self-drawing everything | Dedicated `forma-a11y` semantics tree + native bridges in Phase 3, designed in from the element tree. |
| oxideav API churn | Pin via path deps now; track upstream; the `forma-render` seam isolates oxideav from the rest. |
| Web with "minimal deps" constraint | Accept a thin, hand-audited JS-interop shim as the one web exception. |

---

## 6. Decisions locked / still open

**Locked (this session):** software-first rendering with a GPU-ready `Surface`
seam; reactive/declarative public API; self-drawn widgets (OS provides only
window/input/clipboard/IME/a11y); platform order desktop-trio → mobile → web.

**Still open (revisit before/within Phase 1):**
- Threading model: single-threaded UI + render thread vs. fully async event loop.
- State/reactivity primitives: signals vs. message/`update`-reducer vs. hybrid.
- Whether layout folds into `forma-core` or stays a separate crate.
- Styling authoring: pure-Rust builder API only, or an optional declarative
  style/theme description format.
- Async integration (timers, IO, futures) and how it drives re-renders.
```
