//! A native X11 backend implemented directly against the wire protocol.
//!
//! No `xcb`/`x11` crate, no `libX11` FFI — just a `UnixStream` and the X11
//! byte protocol, keeping the crate `#![forbid(unsafe_code)]` and dependency
//! free (the workspace policy in `ROADMAP.md` §1). It connects to `$DISPLAY`,
//! creates a top-level window, and presents the software [`Pixmap`] via
//! `PutImage`.
//!
//! Scope: window creation, resize, close, pointer (move + buttons), and raw
//! key events; `PutImage` presentation. Proper keysym→text translation
//! (via `GetKeyboardMapping`) and MIT-SHM fast presentation are follow-ups, so
//! text input is not yet delivered here.
//!
//! **Verification status:** the pure codec — connection-setup parsing,
//! `$DISPLAY` parsing, `.Xauthority` lookup, and the RGBA→X11 pixel
//! conversion — is unit-tested against synthetic buffers below. The live
//! socket round-trip (handshake, window mapping, event loop) needs a running X
//! server and has not been exercised in CI.

use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};

use crate::ControlFlow;
use crate::error::PlatformError;
use crate::event::{ButtonState, Event, KeyCode, PointerButton, ScrollDelta, WindowId};
use crate::window::{Window, WindowAttributes};
use forma_geometry::{PhysicalSize, Point, ScaleFactor};
use forma_render::{Pixmap, Surface};

// ---- X11 protocol constants -------------------------------------------------

const OP_CREATE_WINDOW: u8 = 1;
const OP_MAP_WINDOW: u8 = 8;
const OP_INTERN_ATOM: u8 = 16;
const OP_CHANGE_PROPERTY: u8 = 18;
const OP_CREATE_GC: u8 = 55;
const OP_PUT_IMAGE: u8 = 72;

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

    // Skip: fixed 40 bytes, vendor (padded to 4), pixmap-formats (8 each).
    let vendor_pad = (vendor_len + 3) & !3;
    let screens_off = 40 + vendor_pad + num_formats * 8;

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
    while buf.len() % 4 != 0 {
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
        })
    }
}

struct X11Surface {
    conn: SharedConn,
    window: u32,
    gc: u32,
    size: PhysicalSize,
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
    fn present(&mut self, pixmap: &Pixmap, _damage: &[forma_geometry::Rect]) {
        let size = pixmap.size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        let mut conn = self.conn.lock().unwrap();
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

    // MapWindow.
    let mut req = vec![OP_MAP_WINDOW, 0, 0, 0];
    req.extend_from_slice(&window.to_le_bytes());
    conn.send(&finish(req)).map_err(os)?;

    let shared: SharedConn = Arc::new(Mutex::new(conn));
    let win = X11Window {
        conn: shared.clone(),
        window,
        gc,
        size,
    };

    // Event loop. Events are 32 bytes each.
    let mut buf = [0u8; 32];
    loop {
        {
            let mut conn = shared.lock().unwrap();
            conn.stream.read_exact(&mut buf).map_err(os)?;
        }
        let flow = match buf[0] & 0x7f {
            X_EXPOSE => handler(Event::RedrawRequested, &win),
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
                // Raw keycode only; keysym/text mapping is a follow-up.
                let state = if code == X_KEY_PRESS {
                    ButtonState::Pressed
                } else {
                    ButtonState::Released
                };
                handler(
                    Event::Key {
                        code: KeyCode::Unidentified(buf[1] as u32),
                        state,
                        modifiers: Default::default(),
                    },
                    &win,
                )
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
        // Build a minimal additional-data buffer: 40-byte fixed header, no
        // vendor, one pixmap format (8 bytes), then a screen.
        let mut d = vec![0u8; 40];
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
}
