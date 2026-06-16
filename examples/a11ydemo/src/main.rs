//! Connects to the session bus with the hand-written D-Bus client in
//! `forma_platform::a11y` (no `zbus`/`dbus` crate) and prints the unique name
//! the bus assigns — the foundation of the AT-SPI accessibility bridge.
//!
//! The CI a11y job runs this inside a private `dbus-run-session` and greps the
//! output for a `:1.x` unique name, proving the SASL handshake + `Hello` work.

fn main() {
    #[cfg(target_os = "linux")]
    {
        match forma_platform::a11y::DBus::connect_session() {
            Ok(bus) => println!("D-Bus connected: unique name {}", bus.unique_name()),
            Err(e) => {
                eprintln!("D-Bus connect failed: {e}");
                std::process::exit(1);
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        println!("a11ydemo: D-Bus bridge is Linux-only");
    }
}
