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

- ✅ **Workspace + 10 crates** scaffolded (edition 2024, rust 1.88 — floor set
  by the oxideav stack via `oxideav-png` → `compcol`), CI (lint +
  MSRV), `forma-geometry`.
- ✅ **Rendering seam** (`forma-render`): `Scene` → oxideav `VectorFrame` →
  `oxideav-raster` → `Pixmap`; `Surface` GPU-ready boundary.
- ✅ **Software rasterization** path verified end to end (off-screen PNGs).
- ✅ **Layout** (`forma-layout`): flex/box solver. **Paint**: `Element` IR +
  measure/layout/paint passes.
- ✅ **Reactivity MVP** (`forma-core`): retained `LayoutNode` tree, `hit_test`,
  `Cx` handler registry, `on_tap` dispatch → state mutation (the `clickdemo`
  example drives clicks through the real path).
- ✅ **Theming** (`forma-style`) and **animation primitives** (`forma-anim`).
- ✅ **Text rendering** via `oxideav-scribe`: `Font` (load + measure), glyph
  shaping → scene nodes with per-run color; `Text` element threaded through
  layout (intrinsic sizing) and paint; `label`/`button_labeled` widgets.
- ✅ **Keyboard + focus**: `FocusId`, click-to-focus, Tab traversal, `on_key`
  handlers, `KeyInput` routing; editable `text_field` widget (the `textinput`
  example types into a focused field through the real dispatch path).
- ✅ **Pointer drag**: `DragId` + `on_drag` handlers, press/move/release routing
  with fractional position; `slider` widget driven by drag.
- ✅ **Widgets** (scaffold): panel, row/column, button, labeled button, label,
  divider, swatch, spacer, text field, checkbox, switch, slider.
- ✅ **Golden-pixel conformance** tests (font-free, deterministic) over the
  headless path — the cross-platform "pixel-identical" guarantee, enforced in
  CI; native backends must match the same samples.
- ✅ **Native X11 backend** written directly against the wire protocol (pure
  sockets, no deps): connect + auth, window create/map/resize/close, present,
  pointer + raw key events. **CI-verified**: a `Visual` workflow runs the
  `window` example under Xvfb and screenshots the result
  (`docs/screenshots/forma-x11.png`). `App::run` selects X11 when `$DISPLAY` is
  set, else headless.
- ✅ **Native Wayland backend** written directly against the wire protocol (no
  `libwayland`/`wayland-client`): connects to `$WAYLAND_DISPLAY`, binds
  `wl_compositor`/`wl_shm`/`xdg_wm_base` via the registry roundtrip, creates an
  `xdg_toplevel`, runs the `xdg-shell` configure/ack handshake, and presents the
  software `Pixmap` through a `memfd`-backed `wl_shm` buffer (the fd passed with
  a raw `sendmsg` `SCM_RIGHTS` control message). Backend selection prefers
  Wayland, then X11, then headless. **CI-verified** under headless `sway` +
  `grim` (`docs/screenshots/forma-wayland.png`).
- ✅ **Wayland input** (`wl_seat`): binds the seat and lazily creates the
  keyboard/pointer once a `capabilities` event advertises them, re-acquiring the
  keyboard if the capability toggles (calling `get_keyboard` unconditionally is
  a protocol error on a device-less headless seat). Keys decode through the
  compositor's **xkb keymap** — captured as an fd via `recvmsg`/`SCM_RIGHTS`,
  `mmap`-ed, and parsed (keycode → keysym) — so text works for any layout, with
  a layout-independent evdev table as fallback. `wl_pointer` motion/buttons
  decode `wl_fixed` coordinates and BTN_LEFT/RIGHT/MIDDLE. Mappings are
  unit-tested; **CI-verified end to end** under headless `sway` — `wtype` types
  "forma wl" into a focused field (`docs/screenshots/forma-wayland-input.png`).
- ✅ **X11 MIT-SHM fast present**: when the server advertises MIT-SHM, frames go
  through a System V shared-memory segment the server maps directly, and
  `ShmPutImage` blits only the `Surface` damage rectangles — so an incremental
  repaint transfers no pixels over the socket. Set up before mapping
  (QueryExtension → shmget/shmat → ShmAttach → sync), with `IPC_RMID` auto-
  cleanup and a `PutImage` fallback when the extension is absent. The shm
  syscalls are the X11 backend's only `unsafe`/FFI. **CI-verified** under Xvfb
  (logs confirm `shm=true`; screenshots render correctly).
- ✅ **Native Windows backend** over raw Win32 FFI (`user32`/`gdi32`/`kernel32`,
  no `windows` crate): window create/show, `StretchDIBits` present. **CI-verified**
  — the Visual workflow's Windows job runs the example on the runner's desktop
  and screenshots it (`docs/screenshots/forma-windows.png`). Input + live resize
  are follow-ups.
- ✅ **Native macOS backend** over raw `objc_msgSend` Cocoa FFI (no
  `objc`/`cocoa` crate): `NSWindow` + a custom `NSView` whose `drawRect:` blits
  a `CGImage` (CTM-flipped for top-left origin). **CI-verified** —
  `docs/screenshots/forma-macos.png`.
- ✅ **Desktop trio native + CI-screenshot-verified**: X11, Win32, and Cocoa
  backends each render the demo on their own OS runner. The build matrix also
  compiles the whole workspace on all three.
- ✅ **Input on X11 + Win32** (pointer move/buttons/wheel, keys, text, resize);
  X11 resolves keysyms (`GetKeyboardMapping`) to text + editing keys and grabs
  focus so it works WM-less. The App re-renders + presents on every input.
  **Interaction CI-verified**: `xdotool` clicks a counter `0 → 2` and types
  `Forma!` into a focused field, both screenshot-confirmed.
- ✅ **Cocoa input + live resize**: a manual `nextEventMatchingMask:` loop
  routes `NSEvent`s (mouse y-flipped, keys) and polls view bounds for resize.
  **Input CI-verified** — `cliclick` drives the counter `0 → 2`
  (`docs/screenshots/forma-macos-clicks.png`). Desktop trio is now interactive
  (X11 + macOS pointer/keyboard screenshot-verified; Win32 build-verified).
- ✅ **Web target (Phase 5), interactive**: `forma-web` (wasm32) holds a
  persistent `App` and a small C ABI; a hand-written JS shim (no wasm-bindgen)
  uploads a font, blits the `Pixmap` to a `<canvas>` via `putImageData`, and
  forwards canvas mouse/text events. **CI-verified** — headless Chrome loads
  the font, self-drives two clicks, and the screenshot shows "Clicks: 2"
  (`docs/screenshots/forma-web.png`): text + input both work on web.
- ✅ **Focus ring + text caret**: the App overlays a primary-colored ring on
  the focused element and a caret at the end of a focused text field's text
  (CI-screenshot-verified via the X11 textinput job).
- ✅ **Hover highlight**: the App tracks the hovered tappable element and
  overlays a translucent highlight matching its shape, re-presenting on change
  (CI-verified — `xdotool` hovers one of two buttons, which lights up).
- ✅ **GPU present path (Phase 6 seam)**: `forma-gpu` routes the software
  `Pixmap` through raw EGL + OpenGL ES 2 (texture upload → fullscreen-quad
  shader → offscreen FBO → readback). **CI-verified** on Mesa software GL
  (`docs/screenshots/forma-gpu.png`). v1 composites the CPU frame on the GPU;
  GPU-native scene tessellation and Vulkan/Metal/D3D/WebGPU are future work.
- ✅ **Theme engine + customization**: a semantic `Palette` (roles, interaction
  states, status, overlays), a `Typography` scale, and a `Theme` builder —
  `with_accent` (recolor + derive hover/active + pick a readable on-color),
  `with_radius`, `with_font_size`, `high_contrast`. Widgets gained `heading`
  and `button_variant` (Primary/Secondary/Ghost/Danger); the App's focus ring
  and hover overlay read theme tokens. **CI-verified**: the `themegallery`
  example renders one card under four themes, montaged into
  `docs/screenshots/forma-themes.png`.
- ✅ **Frame reconciliation (damage diffing)**: `forma-core::diff_trees`
  compares the previously-presented `LayoutNode` tree against the freshly built
  one and returns a `Damage` region (changed rectangles, coalesced). The `App`
  retains the on-screen frame as a baseline and limits each present to the
  damaged region via the `Surface` damage seam — a state change repaints only
  what moved (expose/resize still force a full present). Unit-tested in
  `forma-core` (localized/full/none cases) and `forma` (incremental App frames).
- ✅ **Subtree memoization** (`Cx::memo`): `cx.memo(key, build)` returns the
  previous frame's `Element` for an unchanged `key`, skipping the build closure
  so unchanged static branches aren't rebuilt. The closure gets no `Cx` (so a
  memoized subtree can't register handlers whose ids would desync); the `App`
  threads the cache across frames and evicts untouched keys. Unit-tested
  (build-once-per-key, rebuild-on-change, eviction).
- ✅ **Caret-aware text editing**: a single-line `EditBuffer` with a
  UTF-8-boundary-safe caret (insert / backspace / delete at the caret;
  left / right / home / end motion; `apply(KeyInput)`). The `Element`/
  `LayoutNode` IR carries an optional caret byte index; `paint_focus` draws the
  caret bar at that index (prefix-measured), and the reconciler treats a caret
  move as damage. `text_editor` renders it; **CI-verified** on X11 (type
  "Forma", arrow-left ×2, insert "XY" → "ForXYma" with a mid-string caret).
- ✅ **Text selection**: `EditBuffer` gains a selection anchor — Shift+arrows /
  Home / End extend the selection, plain motions collapse it, typing/delete
  replace it, and Ctrl/Cmd+A selects all (UTF-8-boundary-safe). The X11 backend
  reports key modifiers (shift/ctrl) and Home/End/Delete; `map_key` maps
  Shift+motion to `Select*`. The IR carries a selection byte range, `paint_focus`
  draws a themed translucent highlight behind the selected glyphs, and the
  reconciler treats a selection change as damage. **CI-verified** on X11
  (`docs/screenshots/forma-x11-selection.png`).
- ✅ **Multi-line text rendering**: `Font::measure` and `Scene::fill_text` split
  on `\n` — measure returns the widest line's width and line-height × line count
  (a trailing newline adds an empty line), and `fill_text` places each line
  dropped by one line height. Editable fields stay single-line (caret/selection
  math unchanged). **CI-verified** — the `window` example's two-line caption
  (`docs/screenshots/forma-x11.png`).
- ✅ **Pointer-drag text selection**: `forma-core::caret_index_at` resolves a
  pointer x to the nearest caret byte index (prefix-measured); a `TextPosId` /
  `on_text_pos` handler routes presses (place caret) and drags (extend
  selection) through `text_pos_at`/`find_text_pos`. `EditBuffer` gains
  `place_caret`/`extend_to`; `text_editor` takes a `&mut EditBuffer` accessor and
  wires keyboard + pointer together. **CI-verified** on X11 — mouse drag selects
  "ForXYm" (`docs/screenshots/forma-x11-dragselect.png`).
- ✅ **Word-wrapping**: `Font::wrap` greedily wraps text to a max width (breaking
  at spaces, honoring hard newlines, shaping each word once); a `wrap` flag on
  text elements wraps to the laid-out content width in both measure (growing
  height) and paint. New `paragraph` widget. **CI-verified** — the `window`
  example's paragraph wraps across three lines (`docs/screenshots/forma-x11.png`).
- ✅ **Multi-line editing**: `EditBuffer` gains Enter→newline, Up/Down (keeping
  the byte column), and line-aware Home/End (plus the matching `Select*`).
  `paint_focus` positions the caret on its line and draws the selection as one
  rectangle per spanned line; `caret_index_at` takes a `Point` (line from y,
  column from x). New `text_area` widget. **CI-verified** on X11 — three typed
  lines with a cross-line selection (`docs/screenshots/forma-x11-multiline.png`).
- 🚧 **mobile portability**: the whole stack **cross-compiles for Android
  (`aarch64-linux-android`) and iOS (`aarch64-apple-ios`)** — oxideav is pure
  Rust, so no NDK is needed, and the platform layer falls through to the
  headless backend there. **CI-verified** (the `mobile` job builds the umbrella
  crate for both). ⬜ Native windowing backends (`ANativeWindow` / UIKit
  `CALayer`) so they render to a real device surface.
- ✅ **a11y foundation**: `forma-core::a11y::accessibility_tree` builds a pruned
  semantic `AccessNode` tree (Window/Group/Button/TextField/Text roles, names,
  focus) from the layout tree; `App::accessibility_tree()` exposes it.
  Unit-tested. On **Linux** the tree is wired to the OS accessibility API:
  `forma_platform::a11y` is a **hand-written D-Bus client + server** (no
  `zbus`/`dbus`/`libdbus` — just a `UnixStream` and the wire protocol, like the
  X11/Wayland backends) that runs the SASL `EXTERNAL` handshake, calls `Hello`,
  reaches the **AT-SPI** bus via `org.a11y.Bus.GetAddress`, claims a name, and
  **serves the accessibility tree over `org.a11y.atspi.Accessible`** —
  hand-marshalling method returns, properties, and variants. **CI-verified**: a
  `dbus-send` client reads the Forma UI's root as AT-SPI role 27 (Window→FRAME),
  `ChildCount` 2, and `Name` "Forma" from our server. The **macOS** and
  **Windows** bridges are wired too, each hand-written with no helper crate: the
  Cocoa `FormaView` overrides the **NSAccessibility** protocol (an accessible
  `AXGroup` with the window's label), and `forma_platform::uia` is a by-hand
  **UI Automation** `IRawElementProviderSimple` COM object (vtable + `IUnknown`
  refcounting; `GetPropertyValue` answers control-type and a `VT_BSTR` name).
  Both are **CI-verified** through their real OS dispatch (objc / COM vtable) on
  the macOS and Windows runners. ⬜ Depth: exposing the full element *tree* (not
  just the root) on macOS/Windows, and the `WM_GETOBJECT` window wiring.
- ✅ **GPU-native drawing**: a live forma `Scene` renders entirely on the GPU.
  The `Scene` records structured `DrawCmd`s; `forma-gpu::render_scene` turns box
  primitives (sharp/rounded fills + stroked borders) into geometry shaded by a
  rounded-rect signed-distance-field GLES2 shader, and composites each text run
  as an alpha-blended glyph-coverage mask (not by compositing a whole CPU
  pixmap). **CI-verified** on Mesa: the box/text primitives
  (`docs/screenshots/forma-gpu-rects.png`) and the actual widget-tree `Scene`,
  whose text is drawn from a packed **per-glyph atlas** (each unique glyph
  rasterized once into one shared texture; repeats reuse the slot)
  (`docs/screenshots/forma-gpu-scene.png`). A complete **raw Vulkan render
  pipeline** (no `ash`/`vulkano` — just `libvulkan` + hand-written C structs)
  now runs end to end: `VkInstance` + physical-device enumeration → logical
  device + graphics queue → a `DEVICE_LOCAL` color image + memory → image view +
  single-attachment render pass + framebuffer → a fenced command-buffer submit
  that clears the image, copies it to a `HOST_VISIBLE` buffer, and reads it back
  to the CPU → and finally a full `VkGraphicsPipeline` with two committed
  **SPIR-V** shader modules that `vkCmdDraw`s a triangle. **CI-verified** on Mesa
  lavapipe: the read-back clear is forma blue (`docs/screenshots/forma-gpu-vk.png`)
  and the shader-drawn triangle's center pixel is forma green
  (`docs/screenshots/forma-gpu-vk-tri.png`). The **Metal** (macOS) and
  **Direct3D 11** (Windows) backends now match it: each is hand-written raw FFI
  (no `metal`/`objc`/`windows` crate — `objc_msgSend` by hand for Metal, COM
  vtable slots by hand for D3D), creates a device, clears a render target to
  forma blue, and draws a triangle through a real shader pipeline (a runtime-
  compiled `.metal` library for Metal; `D3DCompile`d HLSL for D3D), reading each
  frame back to the CPU. **CI-verified** on the macOS runner's Metal device and
  the Windows runner's **WARP** software rasterizer: both read back forma blue for
  the clear and forma green at the triangle's center pixel. A **WebGPU** backend
  completes the set: a hand-written WGSL triangle (no `wgpu`/bindgen — the
  sanctioned web exception) drawn through the browser's WebGPU API, **CI-verified**
  in headless Chrome on the bundled **SwiftShader** Vulkan ICD (the screenshot's
  center pixel is forma green). All four GPU backends — Vulkan, Metal, D3D11, and
  WebGPU — thus render a real shader pipeline off-screen and read it back.
  Finally, GPU rendering is **wired into the live on-screen present path**:
  `App::render_with` swaps the software rasterizer for any
  `Scene → Pixmap` renderer, and the `gpuwindow` example drives a real X11 window
  whose every frame is produced by `forma-gpu::render_scene` (GLES SDF + glyph
  atlas) and presented through the platform `Surface`. **CI-verified** under
  Xvfb + Mesa: the window paints a full GPU-rendered frame with no software
  fallback (`docs/screenshots/forma-gpu-window.png`). (A zero-copy swapchain
  present — binding the GPU surface directly to the window instead of reading
  back to a `Pixmap` — remains a future optimization.)

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
Cargo workspace. Forma targets **edition 2024, rust 1.88** (the floor imposed
by the oxideav dependency chain), `version = 0.0.x`,
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
