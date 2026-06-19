---
name: browser-compositing
description: Forma-as-browser-UI compositing architecture — GPU dma-buf + CPU shm dual path, phases, dma-buf gotchas
metadata:
  type: project
---

Goal: use Forma as a web browser's UI. Chosen model: **Forma-as-compositor with
shared buffers** (Chromium model), for performance + process isolation. A
separate sandboxed content process renders the page and hands it to the Forma UI
process; Forma composites it into a **viewport element** with chrome drawn
over/around, and **forwards input** (Forma hit-tests the viewport, sends events
translated to content coords to the content process — first-class, not an
afterthought).

**Dual compositing path, one seam:**
- GPU: content exports a `dma-buf`; Forma imports it as a texture and SAMPLES it
  (zero-copy).
- CPU fallback (no-GPU systems): content writes a **shared-memory** buffer + marks
  **damage** rects; Forma BLITS the damaged sub-regions into the viewport — mirrors
  X11 MIT-SHM and reuses Forma's existing software-renderer + damage present.
  Same IPC, same SCM_RIGHTS fd-passing (dma-buf or shm fd), same damage/resize/
  input protocol; only the buffer type + composite op differ. **Isolation checks:**
  validate child damage rects lie inside its viewport, bounds-check offset/stride
  vs buffer size, never trust child-supplied lengths.

**Phases:** A=dma-buf export/import (done, GPU-verified) · B=windowed on-GPU
compositor (no Pixmap readback) · C=viewport element · D=IPC + 2-process fd-pass
+ sync · E=input forwarding + sandbox · F=macOS IOSurface/Metal + Windows D3D.

**dma-buf gotchas learned (Phase A, real GPU):** the importer MUST echo the
export's DRM format **modifier** (`EGL_DMA_BUF_PLANE0_MODIFIER_LO/HI_EXT`) —
buffers are tiled, importing as LINEAR gives `glEGLImageTargetTexture2DOES`
GL_INVALID_OPERATION and a black (incomplete) texture. Imported dma-buf textures
are **sample-only** (not color-renderable). See [[multiwindow-model]],
[[forma-overview]], [[verify-via-ci-screenshots]].

**Phase B decision (made): DRI3 + Present over raw X11.** GPU-render to a dma-buf,
wrap it as an X Pixmap via DRI3 `PixmapFromBuffers` (fd sent over the raw X socket
with SCM_RIGHTS), flip to the window via the Present extension `PresentPixmap`.
Zero-copy, no Xlib, fits the hand-rolled-protocol ethos. EGL device must match the
X server's GPU — bind EGL/GBM to the DRM fd from `DRI3Open` (not surfaceless).
Build order: B.1 SCM_RIGHTS fd-passing over the X UnixStream (locally testable, no
GPU/X) → B.2 DRI3Open + GBM/EGL on that fd → B.3 PixmapFromBuffers + PresentPixmap
+ Present events. Note: Xvfb has no DRM, so DRI3 present is **not CI-testable** —
verify on real GPU+X hardware. The SCM_RIGHTS primitive is shared with the
content-process IPC (Phase D).
