//! Exercises the hand-written D-Bus client in `stipple_platform::a11y` (no
//! `zbus`/`dbus` crate) — the foundation of the AT-SPI accessibility bridge.
//!
//! - no args: connect to the session bus and the AT-SPI bus, print our unique
//!   names (the SASL handshake + `Hello` + `org.a11y.Bus.GetAddress`).
//! - `serve`: claim a bus name and answer incoming method calls
//!   (`Peer.Ping`, `Introspectable.Introspect`) — the bidirectional half the
//!   accessibility-tree export needs.
//!
//! The CI a11y job runs both modes inside a private `dbus-run-session`.

#[cfg(target_os = "linux")]
const INTROSPECT_XML: &str = r#"<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node name="/org/stippleui/a11y">
  <interface name="org.freedesktop.DBus.Introspectable">
    <method name="Introspect"><arg name="data" type="s" direction="out"/></method>
  </interface>
  <interface name="org.freedesktop.DBus.Peer">
    <method name="Ping"/>
    <method name="GetMachineId"><arg name="machine_uuid" type="s" direction="out"/></method>
  </interface>
  <interface name="org.a11y.atspi.Accessible"/>
</node>"#;

/// Map a Stipple accessibility role to an `org.a11y.atspi` role number.
#[cfg(target_os = "linux")]
fn atspi_role(role: stipple::core::Role) -> u32 {
    use stipple::core::Role;
    match role {
        Role::Window => 27,    // ATSPI_ROLE_FRAME
        Role::Group => 54,     // ATSPI_ROLE_PANEL
        Role::Button => 44,    // ATSPI_ROLE_PUSH_BUTTON
        Role::TextField => 80, // ATSPI_ROLE_ENTRY
        Role::Text => 29,      // ATSPI_ROLE_LABEL
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    {
        use stipple::prelude::*;
        use stipple_platform::a11y::{AtspiNode, DBus};
        let serve = std::env::args().nth(1).as_deref() == Some("serve");

        if serve {
            // Build a real Stipple UI and derive its accessibility tree, then
            // expose the root over AT-SPI.
            let mut app = App::new((), |_s: &(), cx: &mut Cx<()>| {
                let theme = *cx.theme();
                panel(
                    &theme,
                    vec![label(&theme, "Hello"), button_labeled(&theme, "OK")],
                )
            });
            app.render_once();
            let root = app.accessibility_tree().expect("accessibility tree");
            let node = AtspiNode {
                role: atspi_role(root.role),
                name: if root.name.is_empty() {
                    "Stipple".to_string()
                } else {
                    root.name.clone()
                },
                child_count: root.children.len() as i32,
            };
            println!(
                "a11y root: role={} name={:?} children={}",
                node.role, node.name, node.child_count
            );

            let mut bus = match DBus::connect_session() {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("D-Bus connect failed: {e}");
                    std::process::exit(1);
                }
            };
            match bus.request_name("org.stippleui.A11yDemo") {
                Ok(code) => println!("RequestName org.stippleui.A11yDemo -> {code}"),
                Err(e) => {
                    eprintln!("RequestName failed: {e}");
                    std::process::exit(1);
                }
            }
            // Answer method calls until the connection drops (CI kills us).
            if let Err(e) = bus.serve_atspi(&node, INTROSPECT_XML) {
                eprintln!("serve ended: {e}");
            }
            return;
        }

        match DBus::connect_session() {
            Ok(bus) => println!("D-Bus connected: unique name {}", bus.unique_name()),
            Err(e) => {
                eprintln!("D-Bus connect failed: {e}");
                std::process::exit(1);
            }
        }
        match DBus::connect_a11y() {
            Ok(bus) => println!("AT-SPI bus connected: unique name {}", bus.unique_name()),
            Err(e) => {
                eprintln!("AT-SPI connect failed: {e}");
                std::process::exit(1);
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        println!("a11ydemo: D-Bus bridge is Linux-only");
    }
}
