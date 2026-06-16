//! A native X11 backend implemented directly against the wire protocol.
//!
//! No `xcb`/`x11` crate, no `libX11` FFI — just a `UnixStream` and the X11
//! byte protocol (the workspace policy in `ROADMAP.md` §1). It connects to
//! `$DISPLAY`, creates a top-level window, and presents the software [`Pixmap`].
//!
//! Scope: window creation, resize, close, pointer (move + buttons + wheel),
//! and keyboard — `GetKeyboardMapping` resolves keycodes to keysyms, which
//! become [`Event::Text`] (Latin-1 + Unicode) or editing [`Event::Key`]s.
//!
//! Presentation has two paths: when the server advertises the **MIT-SHM**
//! extension, frames are copied into a System V shared-memory segment the
//! server maps directly and blitted with `ShmPutImage` — limited to the
//! [`Surface::present`] damage rectangles, so a small change uploads no pixels
//! over the socket at all. Otherwise it falls back to plain `PutImage`. The shm
//! segment is the one place this otherwise-safe backend needs FFI (the SysV
//! `shm*` syscalls), hence the module-level `allow(unsafe_code)`.
//!
//! **Verification:** the pure codec (setup-reply / `$DISPLAY` / `.Xauthority`
//! parsing, RGBA→X11 conversion) is unit-tested below; the live socket path —
//! handshake, window mapping, present (both shm and `PutImage`), pointer +
//! keyboard input — is exercised by the `Visual` workflow's Xvfb jobs
//! (screenshot + `xdotool` click/type); Xvfb supports MIT-SHM, so CI covers the
//! shm path.

#![allow(unsafe_code)]

use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};

use crate::ControlFlow;
use crate::error::PlatformError;
use crate::event::{ButtonState, Event, KeyCode, Modifiers, PointerButton, ScrollDelta, WindowId};
use crate::window::{Window, WindowAttributes};
use forma_geometry::{PhysicalSize, Point, ScaleFactor};
use forma_render::{Pixmap, Surface};

// ---- X11 protocol constants -------------------------------------------------

const OP_CREATE_WINDOW: u8 = 1;
const OP_MAP_WINDOW: u8 = 8;
const OP_INTERN_ATOM: u8 = 16;
const OP_CHANGE_PROPERTY: u8 = 18;
const OP_GET_INPUT_FOCUS: u8 = 43;
const OP_QUERY_EXTENSION: u8 = 98;
const OP_SET_INPUT_FOCUS: u8 = 42;
const OP_CREATE_GC: u8 = 55;
const OP_PUT_IMAGE: u8 = 72;
const OP_GET_KEYBOARD_MAPPING: u8 = 101;

// MIT-SHM minor opcodes (within the extension's major opcode).
const SHM_ATTACH: u8 = 1;
const SHM_PUT_IMAGE: u8 = 3;

// System V IPC flags for the shared-memory segment.
const IPC_PRIVATE: i32 = 0;
const IPC_CREAT: i32 = 0o1000;
const IPC_RMID: i32 = 0;
const SHM_PERM: i32 = 0o600;

// Event masks.
const EV_EXPOSURE: u32 = 0x0000_8000;
const EV_KEY_PRESS: u32 = 0x0000_0001;
const EV_KEY_RELEASE: u32 = 0x0000_0002;
const EV_BUTTON_PRESS: u32 = 0x0000_0004;
const EV_BUTTON_RELEASE: u32 = 0x0000_0008;
const EV_POINTER_MOTION: u32 = 0x0000_0040;
const EV_STRUCTURE_NOTIFY: u32 = 0x0002_0000;

// Value-mask bits for CreateWindow.
const CW_BACK_PIXEL: u32 = 0x0000_0002;
const CW_EVENT_MASK: u32 = 0x0000_0800;

// Event codes (low 7 bits of the first byte).
const X_EXPOSE: u8 = 12;
const X_BUTTON_PRESS: u8 = 4;
const X_BUTTON_RELEASE: u8 = 5;
const X_MOTION_NOTIFY: u8 = 6;
const X_KEY_PRESS: u8 = 2;
const X_KEY_RELEASE: u8 = 3;
const X_CONFIGURE_NOTIFY: u8 = 22;
const X_CLIENT_MESSAGE: u8 = 33;

/// Whether an X11 display is reachable (cheap check of `$DISPLAY`).
pub fn available() -> bool {
    std::env::var_os("DISPLAY").is_some()
}

/// Emit a diagnostic line to stderr when `FORMA_X11_DEBUG` is set. Used to
/// trace the live socket path (which CI exercises) without spamming normal use.
fn dbg(args: std::fmt::Arguments<'_>) {
    if std::env::var_os("FORMA_X11_DEBUG").is_some() {
        eprintln!("forma x11: {args}");
    }
}

/// Parsed `$DISPLAY` (the local/unix-socket case).
#[derive(Debug, PartialEq, Eq)]
struct DisplayAddr {
    /// Display number (the `0` in `:0`).
    number: u32,
    /// Screen number (rarely non-zero); kept for completeness.
    screen: u32,
}

fn parse_display(display: &str) -> Option<DisplayAddr> {
    // Forms: ":0", ":0.1", "hostname:0". We support the local unix path, so we
    // only need the part after the last ':'.
    let after = display.rsplit(':').next()?;
    let mut parts = after.splitn(2, '.');
    let number: u32 = parts.next()?.parse().ok()?;
    let screen: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    Some(DisplayAddr { number, screen })
}

/// The fields we need from the X11 connection-setup reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Setup {
    resource_id_base: u32,
    resource_id_mask: u32,
    root: u32,
    root_visual: u32,
    root_depth: u8,
    /// Server's max request length, in 4-byte units.
    max_request_len: u32,
    min_keycode: u8,
    max_keycode: u8,
}

/// Parse the additional-data portion of a successful setup reply (everything
/// after the first 8-byte header).
fn parse_setup(data: &[u8]) -> Option<Setup> {
    // Offsets within the additional data (see X11 protocol §Connection Setup).
    let resource_id_base = rd_u32(data, 4)?;
    let resource_id_mask = rd_u32(data, 8)?;
    let max_request_len = rd_u16(data, 18)? as u32;
    let vendor_len = rd_u16(data, 16)? as usize;
    let num_formats = *data.get(21)? as usize;

    // Skip the 32-byte fixed block, then vendor (padded to 4), then the
    // pixmap-formats list (8 bytes each), to reach the first SCREEN.
    let vendor_pad = (vendor_len + 3) & !3;
    let screens_off = 32 + vendor_pad + num_formats * 8;

    let min_keycode = *data.get(26)?;
    let max_keycode = *data.get(27)?;

    let root = rd_u32(data, screens_off)?;
    let root_visual = rd_u32(data, screens_off + 32)?;
    let root_depth = *data.get(screens_off + 38)?;

    Some(Setup {
        resource_id_base,
        resource_id_mask,
        root,
        root_visual,
        root_depth,
        max_request_len,
        min_keycode,
        max_keycode,
    })
}

fn rd_u16(b: &[u8], off: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(off)?, *b.get(off + 1)?]))
}
fn rd_u32(b: &[u8], off: usize) -> Option<u32> {
    Some(u32::from_le_bytes([
        *b.get(off)?,
        *b.get(off + 1)?,
        *b.get(off + 2)?,
        *b.get(off + 3)?,
    ]))
}

/// Convert a straight-RGBA8 pixmap row into X11 ZPixmap bytes for a 24-depth
/// little-endian (LSBFirst) server: each pixel becomes `[B, G, R, 0]`.
fn rgba_to_bgrx(rgba: &[u8], out: &mut Vec<u8>) {
    out.clear();
    out.reserve(rgba.len());
    for px in rgba.chunks_exact(4) {
        out.extend_from_slice(&[px[2], px[1], px[0], 0]);
    }
}

/// Swizzle a straight-RGBA8 row in place into a ZPixmap (`[B, G, R, 0]`) row of
/// equal pixel count. Used to fill the shared-memory image without allocating.
fn swizzle_row(src: &[u8], dst: &mut [u8]) {
    for (s, d) in src.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
        d[0] = s[2];
        d[1] = s[1];
        d[2] = s[0];
        d[3] = 0;
    }
}

// ---- MIT-SHM shared-memory presentation -------------------------------------

use core::ffi::c_void;

// System V shared-memory syscalls (libc, linked by default on Linux). Declared
// directly to keep the crate dependency-free; this is the only FFI in the X11
// backend.
unsafe extern "C" {
    fn shmget(key: i32, size: usize, shmflg: i32) -> i32;
    fn shmat(shmid: i32, shmaddr: *const c_void, shmflg: i32) -> *mut c_void;
    fn shmdt(shmaddr: *const c_void) -> i32;
    fn shmctl(shmid: i32, cmd: i32, buf: *mut c_void) -> i32;
}

/// A System V shared-memory segment attached to both this process and the X
/// server, used as the backing image for `ShmPutImage`.
#[derive(Debug)]
struct ShmState {
    /// MIT-SHM extension major opcode.
    major: u8,
    /// X resource id naming the attached segment (`ShmSeg`).
    seg: u32,
    /// Mapped address in this process.
    ptr: *mut u8,
    /// Bytes available at `ptr`.
    capacity: usize,
}

impl Drop for ShmState {
    fn drop(&mut self) {
        // Detach our mapping; the kernel frees the segment once the server
        // (which detaches on connection close) has also let go.
        unsafe {
            shmdt(self.ptr as *const c_void);
        }
    }
}

impl ShmState {
    /// The mapped segment as a mutable byte slice.
    fn buffer(&mut self) -> &mut [u8] {
        // Safe: we own this mapping for `capacity` bytes; access is single-
        // threaded (present runs on the event-loop thread).
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.capacity) }
    }

    /// Copy the damaged regions of `pixmap` into the segment and blit them with
    /// `ShmPutImage`. Returns `false` if the frame doesn't fit (caller should
    /// fall back to `PutImage`); `true` on a successful (queued) present.
    fn present(
        &mut self,
        conn: &mut Conn,
        window: u32,
        gc: u32,
        pixmap: &Pixmap,
        damage: &[forma_geometry::Rect],
    ) -> bool {
        let size = pixmap.size();
        let (w, h) = (size.width as usize, size.height as usize);
        let stride = w * 4;
        if stride * h > self.capacity {
            return false; // grew past our segment; let PutImage handle it
        }
        let (major, seg) = (self.major, self.seg);
        let src = pixmap.as_bytes();
        let dst = self.buffer();

        // Build the integer regions to upload: explicit damage, or the whole
        // frame when none is given (first frame / expose / resize).
        let mut regions: Vec<(usize, usize, usize, usize)> = Vec::new();
        if damage.is_empty() {
            regions.push((0, 0, w, h));
        } else {
            for r in damage {
                let x0 = (r.min_x().floor().max(0.0) as usize).min(w);
                let y0 = (r.min_y().floor().max(0.0) as usize).min(h);
                let x1 = (r.max_x().ceil().max(0.0) as usize).min(w);
                let y1 = (r.max_y().ceil().max(0.0) as usize).min(h);
                if x1 > x0 && y1 > y0 {
                    regions.push((x0, y0, x1 - x0, y1 - y0));
                }
            }
        }

        for &(x, y, rw, rh) in &regions {
            // Copy each row's sub-span into the shared image (matching strides).
            for row in y..y + rh {
                let off = row * stride + x * 4;
                let len = rw * 4;
                swizzle_row(&src[off..off + len], &mut dst[off..off + len]);
            }
            if conn
                .send(&finish(shm_put_image(
                    major, window, gc, seg, w, h, x, y, rw, rh,
                )))
                .is_err()
            {
                return false;
            }
        }
        true
    }
}

/// Encode a `ShmPutImage` request blitting the sub-rectangle `(x, y, rw, rh)` of
/// a `total_w × total_h` ZPixmap image (depth 24) from the attached segment to
/// the drawable at the same position.
#[allow(clippy::too_many_arguments)]
fn shm_put_image(
    major: u8,
    drawable: u32,
    gc: u32,
    seg: u32,
    total_w: usize,
    total_h: usize,
    x: usize,
    y: usize,
    rw: usize,
    rh: usize,
) -> Vec<u8> {
    let mut req = vec![major, SHM_PUT_IMAGE, 0, 0];
    req.extend_from_slice(&drawable.to_le_bytes());
    req.extend_from_slice(&gc.to_le_bytes());
    req.extend_from_slice(&(total_w as u16).to_le_bytes());
    req.extend_from_slice(&(total_h as u16).to_le_bytes());
    req.extend_from_slice(&(x as u16).to_le_bytes()); // src-x
    req.extend_from_slice(&(y as u16).to_le_bytes()); // src-y
    req.extend_from_slice(&(rw as u16).to_le_bytes()); // src-width
    req.extend_from_slice(&(rh as u16).to_le_bytes()); // src-height
    req.extend_from_slice(&(x as i16).to_le_bytes()); // dst-x
    req.extend_from_slice(&(y as i16).to_le_bytes()); // dst-y
    req.push(24); // depth
    req.push(2); // format: ZPixmap
    req.push(0); // send-event: false
    req.push(0); // unused
    req.extend_from_slice(&seg.to_le_bytes());
    req.extend_from_slice(&0u32.to_le_bytes()); // offset into segment
    req
}

/// Query whether the server supports an extension, returning its major opcode.
/// Must be called before the window is mapped, so the reply doesn't interleave
/// with events.
fn query_extension(conn: &mut Conn, name: &[u8]) -> io::Result<Option<u8>> {
    let mut req = vec![OP_QUERY_EXTENSION, 0, 0, 0];
    req.extend_from_slice(&(name.len() as u16).to_le_bytes());
    req.extend_from_slice(&[0u8; 2]); // unused
    req.extend_from_slice(name);
    pad4(&mut req);
    conn.send(&finish(req))?;
    let mut pkt = [0u8; 32];
    conn.stream.read_exact(&mut pkt)?;
    // Reply: byte 8 = present (bool), byte 9 = major-opcode.
    if pkt[0] != 1 || pkt[8] == 0 {
        return Ok(None);
    }
    Ok(Some(pkt[9]))
}

/// Set up MIT-SHM: create a shared segment of `capacity` bytes, attach it to the
/// server, and confirm the attach succeeded. Returns `None` (with everything
/// cleaned up) if the extension is absent or any step fails — the caller then
/// presents via `PutImage`. Must run before the window is mapped.
fn setup_shm(conn: &mut Conn, capacity: usize) -> Option<ShmState> {
    let major = query_extension(conn, b"MIT-SHM").ok()??;

    let shmid = unsafe { shmget(IPC_PRIVATE, capacity, IPC_CREAT | SHM_PERM) };
    if shmid < 0 {
        return None;
    }
    let addr = unsafe { shmat(shmid, core::ptr::null(), 0) };
    if addr as isize == -1 {
        unsafe {
            shmctl(shmid, IPC_RMID, core::ptr::null_mut());
        }
        return None;
    }
    let ptr = addr as *mut u8;

    // ShmAttach: hand the segment to the server.
    let seg = conn.new_id();
    let mut req = vec![major, SHM_ATTACH, 0, 0];
    req.extend_from_slice(&seg.to_le_bytes());
    req.extend_from_slice(&(shmid as u32).to_le_bytes());
    req.push(0); // read-only: false
    req.extend_from_slice(&[0u8; 3]);
    let cleanup = |ptr: *mut u8, shmid: i32| unsafe {
        shmdt(ptr as *const c_void);
        shmctl(shmid, IPC_RMID, core::ptr::null_mut());
    };
    if conn.send(&finish(req)).is_err() {
        cleanup(ptr, shmid);
        return None;
    }

    // Synchronize on GetInputFocus (which has a reply): if ShmAttach errored, the
    // error packet arrives first. No window is mapped yet, so nothing else can
    // interleave with the reply.
    if conn
        .send(&finish(vec![OP_GET_INPUT_FOCUS, 0, 0, 0]))
        .is_err()
    {
        cleanup(ptr, shmid);
        return None;
    }
    let mut pkt = [0u8; 32];
    if conn.stream.read_exact(&mut pkt).is_err() {
        cleanup(ptr, shmid);
        return None;
    }
    if pkt[0] == 0 {
        // ShmAttach failed; drain the GetInputFocus reply and bail.
        let mut reply = [0u8; 32];
        let _ = conn.stream.read_exact(&mut reply);
        cleanup(ptr, shmid);
        return None;
    }

    // Attach confirmed: mark the segment for deletion now (the server holds a
    // reference), so it's freed automatically once everyone detaches.
    unsafe {
        shmctl(shmid, IPC_RMID, core::ptr::null_mut());
    }
    Some(ShmState {
        major,
        seg,
        ptr,
        capacity,
    })
}

/// Read the first MIT-MAGIC-COOKIE-1 entry's cookie from `.Xauthority`, if any.
/// Best-effort: returns `None` when the file is absent or has no cookie (the
/// server may still accept an unauthenticated local connection).
fn read_auth_cookie() -> Option<Vec<u8>> {
    let path = std::env::var_os("XAUTHORITY")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| std::path::Path::new(&h).join(".Xauthority"))
        })?;
    let data = std::fs::read(path).ok()?;
    parse_xauth_cookie(&data)
}

fn parse_xauth_cookie(data: &[u8]) -> Option<Vec<u8>> {
    // Each entry: family(2) addr_len(2) addr name_len(2) name data_len(2) data,
    // all big-endian lengths.
    let be16 = |b: &[u8], o: usize| -> Option<usize> {
        Some(u16::from_be_bytes([*b.get(o)?, *b.get(o + 1)?]) as usize)
    };
    let mut o = 0;
    while o + 2 <= data.len() {
        o += 2; // family
        let addr_len = be16(data, o)?;
        o += 2 + addr_len;
        let num_len = be16(data, o)?;
        o += 2 + num_len;
        let name_len = be16(data, o)?;
        let name = data.get(o + 2..o + 2 + name_len)?;
        o += 2 + name_len;
        let data_len = be16(data, o)?;
        let cookie = data.get(o + 2..o + 2 + data_len)?;
        o += 2 + data_len;
        if name == b"MIT-MAGIC-COOKIE-1" {
            return Some(cookie.to_vec());
        }
    }
    None
}

// ---- Connection -------------------------------------------------------------

struct Conn {
    stream: UnixStream,
    setup: Setup,
    next_id: u32,
    id_step: u32,
}

impl Conn {
    fn connect() -> Result<Self, PlatformError> {
        let display =
            std::env::var("DISPLAY").map_err(|_| PlatformError::NoBackend("DISPLAY unset"))?;
        let addr =
            parse_display(&display).ok_or(PlatformError::NoBackend("could not parse DISPLAY"))?;
        let path = format!("/tmp/.X11-unix/X{}", addr.number);
        let mut stream = UnixStream::connect(&path)
            .map_err(|e| PlatformError::Os(format!("connect {path}: {e}")))?;

        // --- Setup request ---
        let cookie = read_auth_cookie().unwrap_or_default();
        let (auth_name, auth_data): (&[u8], &[u8]) = if cookie.is_empty() {
            (b"", b"")
        } else {
            (b"MIT-MAGIC-COOKIE-1", &cookie)
        };
        let mut req = Vec::new();
        req.push(b'l'); // little-endian
        req.push(0);
        req.extend_from_slice(&11u16.to_le_bytes()); // protocol major
        req.extend_from_slice(&0u16.to_le_bytes()); // protocol minor
        req.extend_from_slice(&(auth_name.len() as u16).to_le_bytes());
        req.extend_from_slice(&(auth_data.len() as u16).to_le_bytes());
        req.extend_from_slice(&0u16.to_le_bytes()); // pad
        req.extend_from_slice(auth_name);
        pad4(&mut req);
        req.extend_from_slice(auth_data);
        pad4(&mut req);
        stream.write_all(&req).map_err(os)?;
        stream.flush().map_err(os)?;

        // --- Setup reply ---
        let mut header = [0u8; 8];
        stream.read_exact(&mut header).map_err(os)?;
        if header[0] != 1 {
            // 0 = failed, 2 = authenticate.
            return Err(PlatformError::Os(format!(
                "X11 setup refused (status {})",
                header[0]
            )));
        }
        let extra_words = u16::from_le_bytes([header[6], header[7]]) as usize;
        let mut rest = vec![0u8; extra_words * 4];
        stream.read_exact(&mut rest).map_err(os)?;
        let setup = parse_setup(&rest).ok_or(PlatformError::Os("malformed setup reply".into()))?;

        dbg(format_args!(
            "connected: root={:#x} visual={:#x} depth={} max_req={}",
            setup.root, setup.root_visual, setup.root_depth, setup.max_request_len
        ));
        let id_step = setup.resource_id_mask & setup.resource_id_mask.wrapping_neg();
        Ok(Self {
            stream,
            setup,
            next_id: 0,
            id_step: id_step.max(1),
        })
    }

    /// Allocate a fresh XID.
    fn new_id(&mut self) -> u32 {
        let id = self.setup.resource_id_base | (self.next_id * self.id_step);
        self.next_id += 1;
        id
    }

    fn send(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.stream.write_all(bytes)?;
        self.stream.flush()
    }
}

fn os(e: io::Error) -> PlatformError {
    PlatformError::Os(e.to_string())
}

fn pad4(buf: &mut Vec<u8>) {
    while !buf.len().is_multiple_of(4) {
        buf.push(0);
    }
}

/// Set the request-length field (offset 2, in 4-byte units) from the final
/// buffer length, then return the buffer.
fn finish(mut req: Vec<u8>) -> Vec<u8> {
    let words = (req.len() / 4) as u16;
    req[2..4].copy_from_slice(&words.to_le_bytes());
    req
}

// ---- Window + Surface -------------------------------------------------------

/// Shared mutable connection so the window's surface and the event loop can
/// both talk to the server.
type SharedConn = Arc<Mutex<Conn>>;

#[derive(Debug)]
struct X11Window {
    conn: SharedConn,
    window: u32,
    gc: u32,
    size: PhysicalSize,
    // Set up once before mapping; moved into the surface on `create_surface`.
    shm: std::cell::RefCell<Option<ShmState>>,
}

impl std::fmt::Debug for Conn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Conn")
            .field("setup", &self.setup)
            .finish_non_exhaustive()
    }
}

impl Window for X11Window {
    fn id(&self) -> WindowId {
        WindowId(self.window as u64)
    }
    fn inner_size(&self) -> PhysicalSize {
        self.size
    }
    fn scale_factor(&self) -> ScaleFactor {
        // X11 DPI handling (Xft.dpi / RandR) is a follow-up; assume 1×.
        ScaleFactor::IDENTITY
    }
    fn request_redraw(&self) {
        // Driven by Expose; an explicit invalidate would need a SendEvent.
    }
    fn set_title(&self, _title: &str) {}
    fn create_surface(&self) -> Box<dyn Surface> {
        Box::new(X11Surface {
            conn: self.conn.clone(),
            window: self.window,
            gc: self.gc,
            size: self.size,
            shm: self.shm.borrow_mut().take(),
        })
    }
}

struct X11Surface {
    conn: SharedConn,
    window: u32,
    gc: u32,
    size: PhysicalSize,
    /// Present via MIT-SHM when available; `None` falls back to `PutImage`.
    shm: Option<ShmState>,
}

impl std::fmt::Debug for X11Surface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("X11Surface")
            .field("window", &self.window)
            .field("size", &self.size)
            .finish()
    }
}

impl Surface for X11Surface {
    fn resize(&mut self, size: PhysicalSize) {
        self.size = size;
    }
    fn size(&self) -> PhysicalSize {
        self.size
    }
    fn present(&mut self, pixmap: &Pixmap, damage: &[forma_geometry::Rect]) {
        let size = pixmap.size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        dbg(format_args!(
            "present {}x{} to window={:#x} (shm={})",
            size.width,
            size.height,
            self.window,
            self.shm.is_some()
        ));
        let mut conn = self.conn.lock().unwrap();

        // Fast path: copy only the damaged regions into shared memory and blit
        // them. On failure (frame grew past the segment), drop shm and fall
        // through to PutImage for this and subsequent frames.
        if let Some(shm) = self.shm.as_mut() {
            if shm.present(&mut conn, self.window, self.gc, pixmap, damage) {
                return;
            }
            self.shm = None;
        }
        // PutImage may exceed the server's max request length; send in row
        // bands that each fit. Header is 24 bytes; budget the rest for pixels.
        let max_bytes = (conn.setup.max_request_len as usize)
            .saturating_mul(4)
            .max(256 * 1024);
        let row_bytes = size.width as usize * 4;
        let rows_per = (max_bytes.saturating_sub(24) / row_bytes.max(1)).max(1);

        let src = pixmap.as_bytes();
        let mut bgrx = Vec::new();
        let mut y = 0u32;
        while y < size.height {
            let band = rows_per.min((size.height - y) as usize);
            let start = y as usize * row_bytes;
            let end = start + band * row_bytes;
            rgba_to_bgrx(&src[start..end], &mut bgrx);

            let mut req = vec![OP_PUT_IMAGE, 2 /* ZPixmap */, 0, 0];
            req.extend_from_slice(&self.window.to_le_bytes());
            req.extend_from_slice(&self.gc.to_le_bytes());
            req.extend_from_slice(&(size.width as u16).to_le_bytes());
            req.extend_from_slice(&(band as u16).to_le_bytes());
            req.extend_from_slice(&0i16.to_le_bytes()); // dst-x
            req.extend_from_slice(&(y as i16).to_le_bytes()); // dst-y
            req.push(0); // left-pad
            req.push(24); // depth
            req.extend_from_slice(&0u16.to_le_bytes()); // pad
            req.extend_from_slice(&bgrx);
            pad4(&mut req);
            if conn.send(&finish(req)).is_err() {
                break;
            }
            y += band as u32;
        }
    }
}

// ---- Run loop ---------------------------------------------------------------

/// Connect to the X server, create a window, and drive `handler` over its event
/// stream until the handler returns [`ControlFlow::Exit`] or the window closes.
pub fn run<H>(attrs: WindowAttributes, mut handler: H) -> Result<(), PlatformError>
where
    H: FnMut(Event, &dyn Window) -> ControlFlow,
{
    let mut conn = Conn::connect()?;
    let size = ScaleFactor::IDENTITY.to_physical(attrs.logical_size);
    let (w, h) = (size.width.max(1) as u16, size.height.max(1) as u16);

    // Fetch the keyboard mapping up front (no window yet, so the reply is the
    // next message and won't interleave with events).
    let keymap = fetch_keymap(&mut conn).map_err(os)?;
    dbg(format_args!(
        "keymap: {} keysyms, {}/keycode",
        keymap.syms.len(),
        keymap.per
    ));

    let window = conn.new_id();
    let gc = conn.new_id();
    let setup = conn.setup;

    // CreateWindow.
    let mut req = vec![OP_CREATE_WINDOW, setup.root_depth, 0, 0];
    req.extend_from_slice(&window.to_le_bytes());
    req.extend_from_slice(&setup.root.to_le_bytes());
    req.extend_from_slice(&0i16.to_le_bytes()); // x
    req.extend_from_slice(&0i16.to_le_bytes()); // y
    req.extend_from_slice(&w.to_le_bytes());
    req.extend_from_slice(&h.to_le_bytes());
    req.extend_from_slice(&0u16.to_le_bytes()); // border width
    req.extend_from_slice(&1u16.to_le_bytes()); // class: InputOutput
    req.extend_from_slice(&setup.root_visual.to_le_bytes());
    req.extend_from_slice(&(CW_BACK_PIXEL | CW_EVENT_MASK).to_le_bytes());
    req.extend_from_slice(&0u32.to_le_bytes()); // back-pixel: black
    let mask = EV_EXPOSURE
        | EV_KEY_PRESS
        | EV_KEY_RELEASE
        | EV_BUTTON_PRESS
        | EV_BUTTON_RELEASE
        | EV_POINTER_MOTION
        | EV_STRUCTURE_NOTIFY;
    req.extend_from_slice(&mask.to_le_bytes());
    conn.send(&finish(req)).map_err(os)?;

    // CreateGC.
    let mut req = vec![OP_CREATE_GC, 0, 0, 0];
    req.extend_from_slice(&gc.to_le_bytes());
    req.extend_from_slice(&window.to_le_bytes());
    req.extend_from_slice(&0u32.to_le_bytes()); // value-mask: none
    conn.send(&finish(req)).map_err(os)?;

    // Title via WM_NAME (atom 39 = WM_NAME, predefined; type STRING = 31).
    set_property_str(&mut conn, window, 39, 31, attrs.title.as_bytes()).map_err(os)?;

    // WM_PROTOCOLS / WM_DELETE_WINDOW so the close button delivers an event.
    let wm_protocols = intern_atom(&mut conn, b"WM_PROTOCOLS").map_err(os)?;
    let wm_delete = intern_atom(&mut conn, b"WM_DELETE_WINDOW").map_err(os)?;
    // type ATOM = 4.
    set_property_atoms(&mut conn, window, wm_protocols, &[wm_delete]).map_err(os)?;

    // Set up MIT-SHM presentation before mapping (its handshake needs a reply
    // that must not interleave with window events). Falls back to PutImage when
    // unavailable.
    let shm = setup_shm(&mut conn, (w as usize) * (h as usize) * 4);
    dbg(format_args!("shm present path: {}", shm.is_some()));

    // MapWindow.
    let mut req = vec![OP_MAP_WINDOW, 0, 0, 0];
    req.extend_from_slice(&window.to_le_bytes());
    conn.send(&finish(req)).map_err(os)?;

    // Grab keyboard focus so key events arrive even with no window manager
    // (e.g. under Xvfb in CI). revert-to = PointerRoot (1), time = 0
    // (CurrentTime). Pointer events are delivered regardless of focus.
    let mut req = vec![OP_SET_INPUT_FOCUS, 1, 0, 0];
    req.extend_from_slice(&window.to_le_bytes());
    req.extend_from_slice(&0u32.to_le_bytes());
    conn.send(&finish(req)).map_err(os)?;

    dbg(format_args!(
        "mapped window={window:#x} gc={gc:#x} size={}x{}",
        w, h
    ));

    let shared: SharedConn = Arc::new(Mutex::new(conn));
    let win = X11Window {
        conn: shared.clone(),
        window,
        gc,
        size,
        shm: std::cell::RefCell::new(shm),
    };

    // Event loop. Events are 32 bytes each.
    let mut buf = [0u8; 32];
    loop {
        {
            let mut conn = shared.lock().unwrap();
            conn.stream.read_exact(&mut buf).map_err(os)?;
        }
        // An error reply (first byte 0) is not an event; log and skip it.
        if buf[0] == 0 {
            dbg(format_args!(
                "X error code={} major={} minor={} bad_resource={:#x}",
                buf[1],
                buf[10],
                u16::from_le_bytes([buf[8], buf[9]]),
                rd_u32(&buf, 4).unwrap_or(0)
            ));
            continue;
        }
        let flow = match buf[0] & 0x7f {
            X_EXPOSE => {
                dbg(format_args!("expose"));
                handler(Event::RedrawRequested, &win)
            }
            X_CONFIGURE_NOTIFY => {
                let nw = u16::from_le_bytes([buf[20], buf[21]]) as u32;
                let nh = u16::from_le_bytes([buf[22], buf[23]]) as u32;
                handler(Event::Resized(PhysicalSize::new(nw, nh)), &win)
            }
            X_MOTION_NOTIFY => {
                let (x, y) = event_xy(&buf);
                handler(
                    Event::PointerMoved {
                        position: Point::new(x, y),
                    },
                    &win,
                )
            }
            code @ (X_BUTTON_PRESS | X_BUTTON_RELEASE) => {
                let detail = buf[1];
                let (x, y) = event_xy(&buf);
                let state = if code == X_BUTTON_PRESS {
                    ButtonState::Pressed
                } else {
                    ButtonState::Released
                };
                // Buttons 4/5 are the scroll wheel.
                if detail == 4 || detail == 5 {
                    if code == X_BUTTON_PRESS {
                        let dy = if detail == 4 { -40.0 } else { 40.0 };
                        handler(
                            Event::Scroll {
                                delta: ScrollDelta { dx: 0.0, dy },
                            },
                            &win,
                        )
                    } else {
                        ControlFlow::Wait
                    }
                } else {
                    let button = match detail {
                        1 => PointerButton::Left,
                        2 => PointerButton::Middle,
                        3 => PointerButton::Right,
                        n => PointerButton::Other(n as u16),
                    };
                    handler(
                        Event::PointerButton {
                            button,
                            state,
                            position: Point::new(x, y),
                        },
                        &win,
                    )
                }
            }
            code @ (X_KEY_PRESS | X_KEY_RELEASE) => {
                let state = if code == X_KEY_PRESS {
                    ButtonState::Pressed
                } else {
                    ButtonState::Released
                };
                // buf[1] = keycode; buf[28..30] = modifier mask
                // (bit 0 = Shift, bit 2 = Control).
                let keycode = buf[1];
                let mask = rd_u16(&buf, 28).unwrap_or(0);
                let modifiers = Modifiers {
                    shift: mask & 0x0001 != 0,
                    ctrl: mask & 0x0004 != 0,
                    ..Default::default()
                };
                let ks = keymap.keysym(keycode, modifiers.shift);
                match keysym_to_event(ks, state, modifiers) {
                    Some(ev) => handler(ev, &win),
                    None => ControlFlow::Wait,
                }
            }
            X_CLIENT_MESSAGE => {
                // data starts at byte 12; first 32-bit word is the protocol atom.
                let atom = rd_u32(&buf, 12).unwrap_or(0);
                if atom == wm_delete {
                    handler(Event::CloseRequested, &win)
                } else {
                    ControlFlow::Wait
                }
            }
            _ => ControlFlow::Wait, // errors, replies, unhandled events
        };
        if flow == ControlFlow::Exit {
            break;
        }
    }
    Ok(())
}

/// The keyboard mapping (keycode → keysyms) fetched via `GetKeyboardMapping`,
/// used to turn key events into text + editing keys.
struct Keymap {
    syms: Vec<u32>,
    per: usize,
    min: u8,
}

impl Keymap {
    fn keysym(&self, keycode: u8, shift: bool) -> u32 {
        if keycode < self.min || self.per == 0 {
            return 0;
        }
        let base = (keycode - self.min) as usize * self.per;
        let i = base + if shift && self.per > 1 { 1 } else { 0 };
        let ks = self.syms.get(i).copied().unwrap_or(0);
        // Fall back to the unshifted keysym when the shifted slot is empty.
        if ks == 0 {
            self.syms.get(base).copied().unwrap_or(0)
        } else {
            ks
        }
    }
}

fn fetch_keymap(conn: &mut Conn) -> io::Result<Keymap> {
    let min = conn.setup.min_keycode;
    let max = conn.setup.max_keycode;
    let count = max.saturating_sub(min).saturating_add(1);
    let mut req = vec![OP_GET_KEYBOARD_MAPPING, 0, 0, 0, min, count, 0, 0];
    conn.send(&finish(std::mem::take(&mut req)))?;
    let mut reply = [0u8; 32];
    conn.stream.read_exact(&mut reply)?;
    let per = reply[1] as usize;
    let len_words = rd_u32(&reply, 4).unwrap_or(0) as usize; // = count * per
    let mut body = vec![0u8; len_words * 4];
    conn.stream.read_exact(&mut body)?;
    let syms = body
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    Ok(Keymap { syms, per, min })
}

/// Translate an X11 keysym into a Forma event: editing/navigation keys become
/// [`Event::Key`] (carrying `modifiers`); printable Latin-1 / Unicode keysyms
/// become [`Event::Text`] (on press only).
fn keysym_to_event(ks: u32, state: ButtonState, modifiers: Modifiers) -> Option<Event> {
    let code = match ks {
        0xff08 => Some(KeyCode::Backspace),
        0xff09 => Some(KeyCode::Tab),
        0xff0d => Some(KeyCode::Enter),
        0xff1b => Some(KeyCode::Escape),
        0xff51 => Some(KeyCode::ArrowLeft),
        0xff52 => Some(KeyCode::ArrowUp),
        0xff53 => Some(KeyCode::ArrowRight),
        0xff54 => Some(KeyCode::ArrowDown),
        0xff50 => Some(KeyCode::Home),
        0xff57 => Some(KeyCode::End),
        0xffff => Some(KeyCode::Delete),
        _ => None,
    };
    if let Some(code) = code {
        return Some(Event::Key {
            code,
            state,
            modifiers,
        });
    }
    if state != ButtonState::Pressed {
        return None;
    }
    // Latin-1 keysyms map directly to codepoints; the Unicode plane is
    // 0x0100_0000 | codepoint.
    let cp = if (0x20..=0x7e).contains(&ks) || (0xa0..=0xff).contains(&ks) {
        ks
    } else if ks & 0xff00_0000 == 0x0100_0000 {
        ks & 0x00ff_ffff
    } else {
        return None;
    };
    char::from_u32(cp).map(|c| Event::Text(c.to_string()))
}

fn event_xy(buf: &[u8; 32]) -> (f64, f64) {
    // event-x / event-y are i16 at offsets 24 / 26 for pointer events.
    let x = i16::from_le_bytes([buf[24], buf[25]]) as f64;
    let y = i16::from_le_bytes([buf[26], buf[27]]) as f64;
    (x, y)
}

fn intern_atom(conn: &mut Conn, name: &[u8]) -> io::Result<u32> {
    let mut req = vec![OP_INTERN_ATOM, 0 /* only-if-exists = false */, 0, 0];
    req.extend_from_slice(&(name.len() as u16).to_le_bytes());
    req.extend_from_slice(&0u16.to_le_bytes()); // pad
    req.extend_from_slice(name);
    pad4(&mut req);
    conn.send(&finish(req))?;
    // InternAtom reply: 32-byte header; atom at offset 8.
    let mut reply = [0u8; 32];
    conn.stream.read_exact(&mut reply)?;
    Ok(rd_u32(&reply, 8).unwrap_or(0))
}

fn set_property_str(
    conn: &mut Conn,
    window: u32,
    prop: u32,
    ty: u32,
    val: &[u8],
) -> io::Result<()> {
    let mut req = vec![OP_CHANGE_PROPERTY, 0 /* Replace */, 0, 0];
    req.extend_from_slice(&window.to_le_bytes());
    req.extend_from_slice(&prop.to_le_bytes());
    req.extend_from_slice(&ty.to_le_bytes());
    req.push(8); // format
    req.extend_from_slice(&[0, 0, 0]); // pad
    req.extend_from_slice(&(val.len() as u32).to_le_bytes());
    req.extend_from_slice(val);
    pad4(&mut req);
    conn.send(&finish(req))
}

fn set_property_atoms(conn: &mut Conn, window: u32, prop: u32, atoms: &[u32]) -> io::Result<()> {
    let mut req = vec![OP_CHANGE_PROPERTY, 0, 0, 0];
    req.extend_from_slice(&window.to_le_bytes());
    req.extend_from_slice(&prop.to_le_bytes());
    req.extend_from_slice(&4u32.to_le_bytes()); // type ATOM
    req.push(32); // format
    req.extend_from_slice(&[0, 0, 0]);
    req.extend_from_slice(&(atoms.len() as u32).to_le_bytes());
    for a in atoms {
        req.extend_from_slice(&a.to_le_bytes());
    }
    conn.send(&finish(req))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_display_forms() {
        assert_eq!(
            parse_display(":0"),
            Some(DisplayAddr {
                number: 0,
                screen: 0
            })
        );
        assert_eq!(
            parse_display(":1.2"),
            Some(DisplayAddr {
                number: 1,
                screen: 2
            })
        );
        assert_eq!(
            parse_display("host:3"),
            Some(DisplayAddr {
                number: 3,
                screen: 0
            })
        );
        assert_eq!(parse_display("garbage"), None);
    }

    #[test]
    fn rgba_to_bgrx_swizzles() {
        let mut out = Vec::new();
        rgba_to_bgrx(&[10, 20, 30, 255, 1, 2, 3, 128], &mut out);
        assert_eq!(out, vec![30, 20, 10, 0, 3, 2, 1, 0]);
    }

    #[test]
    fn parse_setup_reads_screen_fields() {
        // Build a minimal additional-data buffer: 32-byte fixed block, no
        // vendor, one pixmap format (8 bytes), then a screen — matching the
        // X11 connection-setup reply layout.
        let mut d = vec![0u8; 32];
        d[4..8].copy_from_slice(&0x0040_0000u32.to_le_bytes()); // resource-id-base
        d[8..12].copy_from_slice(&0x001f_ffffu32.to_le_bytes()); // resource-id-mask
        d[16..18].copy_from_slice(&0u16.to_le_bytes()); // vendor-length
        d[18..20].copy_from_slice(&65535u16.to_le_bytes()); // max-request-length
        d[21] = 1; // number-of-formats
        d.extend_from_slice(&[0u8; 8]); // one pixmap format
        // Screen: root at +0, root-visual at +32, root-depth at +38.
        let mut screen = vec![0u8; 40];
        screen[0..4].copy_from_slice(&0x0000_0123u32.to_le_bytes()); // root
        screen[32..36].copy_from_slice(&0x0000_0021u32.to_le_bytes()); // root-visual
        screen[38] = 24; // root-depth
        d.extend_from_slice(&screen);

        let s = parse_setup(&d).expect("parse");
        assert_eq!(s.resource_id_base, 0x0040_0000);
        assert_eq!(s.resource_id_mask, 0x001f_ffff);
        assert_eq!(s.root, 0x0000_0123);
        assert_eq!(s.root_visual, 0x0000_0021);
        assert_eq!(s.root_depth, 24);
        assert_eq!(s.max_request_len, 65535);
    }

    #[test]
    fn parse_xauth_finds_cookie() {
        // family(2) addrlen(2)=1 addr("a") numlen(2)=1 num("0")
        // namelen(2)=18 name("MIT-MAGIC-COOKIE-1") datalen(2)=4 data(DE AD BE EF)
        let mut e = Vec::new();
        e.extend_from_slice(&0u16.to_be_bytes());
        e.extend_from_slice(&1u16.to_be_bytes());
        e.push(b'a');
        e.extend_from_slice(&1u16.to_be_bytes());
        e.push(b'0');
        e.extend_from_slice(&18u16.to_be_bytes());
        e.extend_from_slice(b"MIT-MAGIC-COOKIE-1");
        e.extend_from_slice(&4u16.to_be_bytes());
        e.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(parse_xauth_cookie(&e), Some(vec![0xDE, 0xAD, 0xBE, 0xEF]));
        assert_eq!(parse_xauth_cookie(&[]), None);
    }

    #[test]
    fn finish_sets_length_in_words() {
        let req = finish(vec![1, 0, 0, 0, 0, 0, 0, 0]); // 8 bytes = 2 words
        assert_eq!(u16::from_le_bytes([req[2], req[3]]), 2);
    }

    #[test]
    fn swizzle_row_reorders_to_bgrx() {
        let mut dst = [0u8; 8];
        swizzle_row(&[10, 20, 30, 255, 1, 2, 3, 128], &mut dst);
        assert_eq!(dst, [30, 20, 10, 0, 3, 2, 1, 0]);
    }

    #[test]
    fn shm_put_image_encodes_fields() {
        // major=130, draw=0xaa, gc=0xbb, seg=0xcc, total 200x100, rect (8,16,4,2).
        let req = finish(shm_put_image(130, 0xaa, 0xbb, 0xcc, 200, 100, 8, 16, 4, 2));
        assert_eq!(req[0], 130); // major opcode
        assert_eq!(req[1], SHM_PUT_IMAGE); // minor opcode
        assert_eq!(u16::from_le_bytes([req[2], req[3]]), 10); // length in words
        assert_eq!(rd_u32(&req, 4), Some(0xaa)); // drawable
        assert_eq!(rd_u32(&req, 8), Some(0xbb)); // gc
        assert_eq!(u16::from_le_bytes([req[12], req[13]]), 200); // total-width
        assert_eq!(u16::from_le_bytes([req[14], req[15]]), 100); // total-height
        assert_eq!(u16::from_le_bytes([req[16], req[17]]), 8); // src-x
        assert_eq!(u16::from_le_bytes([req[18], req[19]]), 16); // src-y
        assert_eq!(u16::from_le_bytes([req[20], req[21]]), 4); // src-width
        assert_eq!(u16::from_le_bytes([req[22], req[23]]), 2); // src-height
        assert_eq!(i16::from_le_bytes([req[24], req[25]]), 8); // dst-x
        assert_eq!(i16::from_le_bytes([req[26], req[27]]), 16); // dst-y
        assert_eq!(req[28], 24); // depth
        assert_eq!(req[29], 2); // ZPixmap
        assert_eq!(req[30], 0); // send-event
        assert_eq!(rd_u32(&req, 32), Some(0xcc)); // shmseg
        assert_eq!(rd_u32(&req, 36), Some(0)); // offset
    }
}
