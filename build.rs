//! Embeds the icon in xpaint.exe, so Explorer, the taskbar and the Start menu
//! shortcut show it. Nothing to do for other targets.

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/icon.ico");
        // Needs rc.exe (Windows SDK) or windres (MinGW).
        resource
            .compile()
            .unwrap_or_else(|e| panic!("could not embed the icon: {e}"));
    }
}
