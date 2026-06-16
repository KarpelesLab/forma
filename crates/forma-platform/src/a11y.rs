//! Hand-written **D-Bus** client and the start of an **AT-SPI** accessibility
//! bridge — no `zbus`/`dbus`/`libdbus`, just a `UnixStream` and the D-Bus wire
//! protocol, the same "talk to the OS directly" approach as the X11 and Wayland
//! backends. Linux-only.
//!
//! AT-SPI (the Linux accessibility framework) is layered on D-Bus: an app
//! connects to a bus, claims a name, and exports a tree of objects implementing
//! the `org.a11y.atspi.*` interfaces that screen readers walk. This module is
//! the foundation: connect to the session bus, run the SASL `EXTERNAL`
//! handshake, and call `org.freedesktop.DBus.Hello` to obtain our unique
//! connection name. Exposing the [`AccessNode`](forma_core) tree over
//! `org.a11y.atspi.Accessible` builds on this.

#![allow(unsafe_code)]

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

unsafe extern "C" {
    fn geteuid() -> u32;
}

/// A minimal D-Bus connection: an authenticated stream plus a method-call serial
/// counter. Built by [`DBus::connect_session`].
#[derive(Debug)]
pub struct DBus {
    stream: UnixStream,
    serial: u32,
    /// The unique name the bus assigned us (e.g. `":1.42"`).
    unique_name: String,
}

impl DBus {
    /// Connect to the session bus (`$DBUS_SESSION_BUS_ADDRESS`), authenticate
    /// with SASL `EXTERNAL`, and call `Hello` to obtain our unique name.
    pub fn connect_session() -> Result<DBus, String> {
        let addr = std::env::var("DBUS_SESSION_BUS_ADDRESS")
            .map_err(|_| "DBUS_SESSION_BUS_ADDRESS not set".to_string())?;
        let path =
            parse_unix_path(&addr).ok_or_else(|| format!("unsupported bus address: {addr}"))?;
        let stream = UnixStream::connect(&path).map_err(|e| format!("connect {path}: {e}"))?;
        Self::handshake(stream)
    }

    /// Connect to an explicit `unix:path=…` / `unix:abstract=…` address.
    pub fn connect_address(addr: &str) -> Result<DBus, String> {
        let path =
            parse_unix_path(addr).ok_or_else(|| format!("unsupported bus address: {addr}"))?;
        let stream = UnixStream::connect(&path).map_err(|e| format!("connect {path}: {e}"))?;
        Self::handshake(stream)
    }

    fn handshake(mut stream: UnixStream) -> Result<DBus, String> {
        // SASL EXTERNAL: the leading NUL byte, then authenticate as our uid
        // (sent as the hex of its ASCII-decimal form, per the D-Bus spec).
        let uid = unsafe { geteuid() };
        let uid_hex: String = uid
            .to_string()
            .bytes()
            .map(|b| format!("{b:02x}"))
            .collect();
        stream.write_all(&[0]).map_err(io)?;
        stream
            .write_all(format!("AUTH EXTERNAL {uid_hex}\r\n").as_bytes())
            .map_err(io)?;
        let line = read_line(&mut stream)?;
        if !line.starts_with("OK ") {
            return Err(format!("SASL auth failed: {line:?}"));
        }
        stream.write_all(b"BEGIN\r\n").map_err(io)?;

        let mut bus = DBus {
            stream,
            serial: 0,
            unique_name: String::new(),
        };
        // org.freedesktop.DBus.Hello → our unique name (a STRING body).
        let reply = bus.call(
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "Hello",
        )?;
        bus.unique_name = read_string(&reply, &mut 0)?;
        Ok(bus)
    }

    /// Our unique bus name (e.g. `":1.42"`).
    pub fn unique_name(&self) -> &str {
        &self.unique_name
    }

    /// Ask the session bus's accessibility broker (`org.a11y.Bus`) for the
    /// address of the separate **AT-SPI** bus that screen readers listen on.
    /// Activates `at-spi-bus-launcher` on demand.
    pub fn a11y_bus_address(&mut self) -> Result<String, String> {
        let reply = self.call(
            "org.a11y.Bus",
            "/org/a11y/bus",
            "org.a11y.Bus",
            "GetAddress",
        )?;
        read_string(&reply, &mut 0)
    }

    /// Connect to the **AT-SPI** accessibility bus: a second D-Bus connection to
    /// the address the session bus's `org.a11y.Bus` broker hands out. This is the
    /// bus an app exports its accessibility tree on.
    pub fn connect_a11y() -> Result<DBus, String> {
        let mut session = Self::connect_session()?;
        let addr = session.a11y_bus_address()?;
        Self::connect_address(&addr)
    }

    /// Claim a well-known bus `name` via `org.freedesktop.DBus.RequestName`
    /// (flags 0). Returns the reply code (1 = primary owner). Apps that export an
    /// accessibility tree own a name so clients can address them.
    pub fn request_name(&mut self, name: &str) -> Result<u32, String> {
        self.serial += 1;
        let serial = self.serial;
        let mut fields = Vec::new();
        put_field(&mut fields, 1, b'o', "/org/freedesktop/DBus");
        put_field(&mut fields, 6, b's', "org.freedesktop.DBus");
        put_field(&mut fields, 2, b's', "org.freedesktop.DBus");
        put_field(&mut fields, 3, b's', "RequestName");
        put_field_sig(&mut fields, 8, "su");
        let mut body = Vec::new();
        put_string(&mut body, name);
        align(&mut body, 4);
        body.extend_from_slice(&0u32.to_le_bytes()); // flags
        let msg = build_message(1, serial, &fields, &body);
        self.stream.write_all(&msg).map_err(io)?;
        loop {
            let m = self.read_message()?;
            match m.mtype {
                2 if m.reply_serial == serial => return read_u32(&m.body, &mut 0),
                3 if m.reply_serial == serial => return Err("RequestName error".into()),
                _ => continue,
            }
        }
    }

    /// Serve incoming method calls forever (until the connection drops),
    /// answering the standard `org.freedesktop.DBus.Peer` and
    /// `Introspectable.Introspect` interfaces — the minimal surface a D-Bus
    /// object must expose. `introspect_xml` is returned for `Introspect`. This is
    /// the bidirectional half the AT-SPI tree export builds on (the registry
    /// calls back into us). Unknown methods get an `UnknownMethod` error.
    pub fn serve(&mut self, introspect_xml: &str) -> Result<(), String> {
        loop {
            let m = self.read_message()?;
            if m.mtype != 1 {
                continue; // only method calls
            }
            match (m.interface.as_str(), m.member.as_str()) {
                ("org.freedesktop.DBus.Peer", "Ping") => self.send_return(&m, "", &[])?,
                ("org.freedesktop.DBus.Peer", "GetMachineId") => {
                    let mut body = Vec::new();
                    put_string(&mut body, "00000000000000000000000000000000");
                    self.send_return(&m, "s", &body)?;
                }
                ("org.freedesktop.DBus.Introspectable", "Introspect") => {
                    let mut body = Vec::new();
                    put_string(&mut body, introspect_xml);
                    self.send_return(&m, "s", &body)?;
                }
                _ => self.send_error(&m, "org.freedesktop.DBus.Error.UnknownMethod")?,
            }
        }
    }

    /// Send a `METHOD_RETURN` for `req` with optional `signature`/`body`.
    fn send_return(&mut self, req: &Message, signature: &str, body: &[u8]) -> Result<(), String> {
        self.serial += 1;
        let mut fields = Vec::new();
        put_field_u32(&mut fields, 5, req.serial); // REPLY_SERIAL
        if !req.sender.is_empty() {
            put_field(&mut fields, 6, b's', &req.sender); // DESTINATION
        }
        if !signature.is_empty() {
            put_field_sig(&mut fields, 8, signature);
        }
        let msg = build_message(2, self.serial, &fields, body);
        self.stream.write_all(&msg).map_err(io)
    }

    /// Send an `ERROR` reply for `req` naming `error_name`.
    fn send_error(&mut self, req: &Message, error_name: &str) -> Result<(), String> {
        self.serial += 1;
        let mut fields = Vec::new();
        put_field_u32(&mut fields, 5, req.serial); // REPLY_SERIAL
        put_field(&mut fields, 4, b's', error_name); // ERROR_NAME
        if !req.sender.is_empty() {
            put_field(&mut fields, 6, b's', &req.sender); // DESTINATION
        }
        let msg = build_message(3, self.serial, &fields, &[]);
        self.stream.write_all(&msg).map_err(io)
    }

    /// Send a no-argument method call and return the reply's body bytes. Errors
    /// on a D-Bus `ERROR` reply or I/O failure.
    fn call(
        &mut self,
        destination: &str,
        path: &str,
        interface: &str,
        member: &str,
    ) -> Result<Vec<u8>, String> {
        self.serial += 1;
        let serial = self.serial;
        let msg = marshal_method_call(serial, destination, path, interface, member);
        self.stream.write_all(&msg).map_err(io)?;

        // Read replies until we see the method_return / error for our serial
        // (the bus may interleave signals).
        loop {
            let msg = self.read_message()?;
            match msg.mtype {
                2 if msg.reply_serial == serial => return Ok(msg.body), // METHOD_RETURN
                3 if msg.reply_serial == serial => return Err("D-Bus error reply".into()), // ERROR
                _ => continue,
            }
        }
    }

    /// Read one full D-Bus message (header fields + body). Little-endian only.
    fn read_message(&mut self) -> Result<Message, String> {
        // Fixed part of the header is 12 bytes, then the header-fields array
        // length (another u32).
        let mut fixed = [0u8; 16];
        self.stream.read_exact(&mut fixed).map_err(io)?;
        if fixed[0] != b'l' {
            return Err("only little-endian D-Bus is supported".into());
        }
        let mtype = fixed[1];
        let body_len = u32::from_le_bytes([fixed[4], fixed[5], fixed[6], fixed[7]]) as usize;
        let serial = u32::from_le_bytes([fixed[8], fixed[9], fixed[10], fixed[11]]);
        let fields_len = u32::from_le_bytes([fixed[12], fixed[13], fixed[14], fixed[15]]) as usize;

        // The header (fixed 12 + array-len 4 + fields) is padded to 8 bytes
        // before the body.
        let mut fields = vec![0u8; fields_len];
        self.stream.read_exact(&mut fields).map_err(io)?;
        let header_len = 16 + fields_len;
        let pad = (8 - (header_len % 8)) % 8;
        if pad > 0 {
            let mut p = vec![0u8; pad];
            self.stream.read_exact(&mut p).map_err(io)?;
        }
        let mut body = vec![0u8; body_len];
        self.stream.read_exact(&mut body).map_err(io)?;

        let mut msg = parse_fields(&fields);
        msg.mtype = mtype;
        msg.serial = serial;
        msg.body = body;
        Ok(msg)
    }
}

/// A received D-Bus message: header metadata plus the raw body bytes.
#[derive(Debug, Default)]
struct Message {
    mtype: u8,
    serial: u32,
    reply_serial: u32,
    path: String,
    interface: String,
    member: String,
    sender: String,
    body: Vec<u8>,
}

/// Map `unix:path=/x` or `unix:abstract=/x` (with optional trailing `,guid=…`)
/// to a connectable path. Abstract sockets are returned with a leading NUL.
fn parse_unix_path(addr: &str) -> Option<String> {
    let addr = addr.split(';').next().unwrap_or(addr);
    for kv in addr.trim_start_matches("unix:").split(',') {
        if let Some(p) = kv.strip_prefix("path=") {
            return Some(p.to_string());
        }
        if let Some(p) = kv.strip_prefix("abstract=") {
            return Some(format!("\0{p}"));
        }
    }
    None
}

fn io(e: std::io::Error) -> String {
    format!("dbus i/o: {e}")
}

/// Read a CRLF-terminated SASL line (without the CRLF).
fn read_line(stream: &mut UnixStream) -> Result<String, String> {
    let mut out = Vec::new();
    let mut b = [0u8; 1];
    loop {
        stream.read_exact(&mut b).map_err(io)?;
        if b[0] == b'\n' {
            if out.last() == Some(&b'\r') {
                out.pop();
            }
            break;
        }
        out.push(b[0]);
        if out.len() > 4096 {
            return Err("SASL line too long".into());
        }
    }
    String::from_utf8(out).map_err(|_| "SASL line not UTF-8".to_string())
}

// ---- marshalling -----------------------------------------------------------

fn align(buf: &mut Vec<u8>, n: usize) {
    while !buf.len().is_multiple_of(n) {
        buf.push(0);
    }
}

/// Append a D-Bus STRING/OBJECT_PATH value: a u32 length, the bytes, and a NUL.
fn put_string(buf: &mut Vec<u8>, s: &str) {
    align(buf, 4);
    buf.extend_from_slice(&(s.len() as u32).to_le_bytes());
    buf.extend_from_slice(s.as_bytes());
    buf.push(0);
}

/// Append a header field carrying a STRING/OBJECT_PATH value (sig `s` or `o`).
fn put_field(buf: &mut Vec<u8>, code: u8, sig: u8, value: &str) {
    align(buf, 8);
    buf.push(code);
    // VARIANT: a one-byte signature length, the signature, a NUL, then value.
    buf.push(1);
    buf.push(sig);
    buf.push(0);
    put_string(buf, value);
}

/// Append a header field carrying a UINT32 value (e.g. REPLY_SERIAL=5).
fn put_field_u32(buf: &mut Vec<u8>, code: u8, value: u32) {
    align(buf, 8);
    buf.push(code);
    buf.push(1);
    buf.push(b'u');
    buf.push(0);
    align(buf, 4);
    buf.extend_from_slice(&value.to_le_bytes());
}

/// Append a header field carrying a SIGNATURE value (e.g. SIGNATURE=8).
fn put_field_sig(buf: &mut Vec<u8>, code: u8, sig: &str) {
    align(buf, 8);
    buf.push(code);
    buf.push(1);
    buf.push(b'g');
    buf.push(0);
    buf.push(sig.len() as u8);
    buf.extend_from_slice(sig.as_bytes());
    buf.push(0);
}

/// Assemble a full message from a type, serial, marshalled header `fields`, and
/// `body` (the header is padded to 8 bytes before the body).
fn build_message(mtype: u8, serial: u32, fields: &[u8], body: &[u8]) -> Vec<u8> {
    // Fixed header: little-endian, type, no flags, protocol version 1.
    let mut msg = vec![b'l', mtype, 0, 1];
    msg.extend_from_slice(&(body.len() as u32).to_le_bytes());
    msg.extend_from_slice(&serial.to_le_bytes());
    msg.extend_from_slice(&(fields.len() as u32).to_le_bytes());
    msg.extend_from_slice(fields);
    align(&mut msg, 8);
    msg.extend_from_slice(body);
    msg
}

/// Marshal a no-argument `METHOD_CALL` (little-endian).
fn marshal_method_call(
    serial: u32,
    destination: &str,
    path: &str,
    interface: &str,
    member: &str,
) -> Vec<u8> {
    // Header fields (PATH=1 'o', INTERFACE=2 's', MEMBER=3 's', DESTINATION=6 's').
    let mut fields = Vec::new();
    put_field(&mut fields, 1, b'o', path);
    put_field(&mut fields, 6, b's', destination);
    put_field(&mut fields, 2, b's', interface);
    put_field(&mut fields, 3, b's', member);
    build_message(1, serial, &fields, &[])
}

/// Read a u32 at `*off` in `buf` (4-aligned), advancing `*off`.
fn read_u32(buf: &[u8], off: &mut usize) -> Result<u32, String> {
    while !off.is_multiple_of(4) {
        *off += 1;
    }
    if *off + 4 > buf.len() {
        return Err("truncated u32".into());
    }
    let v = u32::from_le_bytes([buf[*off], buf[*off + 1], buf[*off + 2], buf[*off + 3]]);
    *off += 4;
    Ok(v)
}

// ---- demarshalling ---------------------------------------------------------

/// Read a D-Bus STRING at `*off` in `buf` (u32 length + bytes + NUL), advancing.
fn read_string(buf: &[u8], off: &mut usize) -> Result<String, String> {
    while !off.is_multiple_of(4) {
        *off += 1;
    }
    if *off + 4 > buf.len() {
        return Err("truncated string length".into());
    }
    let len = u32::from_le_bytes([buf[*off], buf[*off + 1], buf[*off + 2], buf[*off + 3]]) as usize;
    *off += 4;
    if *off + len > buf.len() {
        return Err("truncated string body".into());
    }
    let s = String::from_utf8_lossy(&buf[*off..*off + len]).into_owned();
    *off += len + 1; // skip the trailing NUL
    Ok(s)
}

/// Parse a header-fields array into the fields we care about (path, interface,
/// member, sender, reply_serial). Walks the `a(yv)` grammar, skipping values by
/// their variant signature; stops on anything it can't decode.
fn parse_fields(fields: &[u8]) -> Message {
    let mut msg = Message::default();
    let mut off = 0usize;
    while off < fields.len() {
        // Each field struct is 8-aligned.
        while !off.is_multiple_of(8) && off < fields.len() {
            off += 1;
        }
        if off + 2 > fields.len() {
            break;
        }
        let code = fields[off];
        off += 1;
        // VARIANT: signature length, signature bytes, NUL.
        let sig_len = fields[off] as usize;
        off += 1;
        let sig = fields.get(off..off + sig_len).unwrap_or(&[]).to_vec();
        off += sig_len + 1; // signature + NUL
        match sig.first().copied() {
            Some(b'u') | Some(b'i') => {
                while !off.is_multiple_of(4) && off < fields.len() {
                    off += 1;
                }
                if off + 4 > fields.len() {
                    break;
                }
                let v = u32::from_le_bytes([
                    fields[off],
                    fields[off + 1],
                    fields[off + 2],
                    fields[off + 3],
                ]);
                off += 4;
                if code == 5 {
                    msg.reply_serial = v;
                }
            }
            Some(b's') | Some(b'o') | Some(b'g') => {
                // 'g' (signature) is length-prefixed by one byte, not a u32.
                let value = if sig.first() == Some(&b'g') {
                    let l = *fields.get(off).unwrap_or(&0) as usize;
                    let v =
                        String::from_utf8_lossy(fields.get(off + 1..off + 1 + l).unwrap_or(&[]))
                            .into_owned();
                    off += 1 + l + 1;
                    v
                } else {
                    match read_string(fields, &mut off) {
                        Ok(v) => v,
                        Err(_) => break,
                    }
                };
                match code {
                    1 => msg.path = value,
                    2 => msg.interface = value,
                    3 => msg.member = value,
                    7 => msg.sender = value,
                    _ => {}
                }
            }
            _ => break, // unknown — stop scanning
        }
    }
    msg
}
