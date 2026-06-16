//! Exercises the hand-written UI Automation provider in `forma_platform::uia`
//! (no `windows`/`uiautomation` crate) — the Windows accessibility bridge.
//!
//! The Windows visual CI job runs this and greps the output to confirm the
//! `IRawElementProviderSimple` COM object answers UIA property queries through
//! its vtable (a cross-process UIA client needs a live desktop UIA stack the
//! runner doesn't reliably provide, so we self-query via real COM dispatch).

fn main() {
    #[cfg(target_os = "windows")]
    {
        if let Err(e) = forma_platform::uia::selftest("Forma") {
            eprintln!("UIA selftest failed: {e}");
            std::process::exit(1);
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        println!("uiademo: UI Automation is Windows-only");
    }
}
