---
name: multiwindow-model
description: Forma multi-window architecture — parent App owns global state, per-window panes
metadata:
  type: project
---

Forma's multi-window model (Phase 5 of the toolkit buildout): the parent `App`
object owns the **global app state `S`** (shared across all windows) plus shared
`theme`/`font`. Each OS window is a **pane** with its own view closure onto that
shared `S`, and its own per-window render/event state (tree, handlers, focus,
hover, scroll offsets, surface, damage baseline). Windows open/close; the app
runs while any window is open.

**Why:** the user chose this ("global state / parent object for all windows")
over fully-independent per-window state — it fits the common main+inspector /
document+palette pattern and keeps `App` a single generic over `S`.

**How to apply:** `App<S>` (views are boxed `dyn FnMut(&S, &mut Cx<S>) -> Element`,
no `F` generic). Backend scope is X11-first: true multi-window verified on X11
(the reference, per [[verify-via-ci-screenshots]]); Wayland/macOS/Windows compile
against the new `WindowChannel` seam but stay single-window until adopted. See
[[forma-overview]].
