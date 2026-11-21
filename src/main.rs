// A GUI program on Windows: no console window. The command-line modes attach to
// the console they were started from (see `attach_console`).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod clipboard;
mod document;
mod install;
mod leoview;
mod shapes;
mod text;
mod update;

use std::path::PathBuf;

fn main() -> eframe::Result {
    let first = std::env::args().nth(1);
    if first.as_deref().is_some_and(|a| a.starts_with("--")) {
        attach_console();
    }
    // Asked by the updater, to check a download before installing it.
    if first.as_deref() == Some("--version") {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    // Same as the Update button, from the command line.
    if first.as_deref() == Some("--update") {
        match update::update_now() {
            Ok(Some(version)) => println!("Updated xpaint to {version}"),
            Ok(None) => println!("xpaint {} is up to date", env!("CARGO_PKG_VERSION")),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    // `--install` / `--uninstall` do the same as the Settings menu, without the UI.
    if let Some(action @ ("--install" | "--uninstall")) = first.as_deref() {
        let result = install::Location::user().and_then(|location| {
            if action == "--install" {
                let exe = std::env::current_exe()
                    .map_err(|e| format!("Could not find the running program: {e}"))?;
                location.install(&exe)?;
                println!("Installed xpaint to {}", location.binary.display());
            } else {
                location.uninstall()?;
                println!("Uninstalled xpaint");
            }
            Ok(())
        });
        if let Err(e) = result {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return Ok(());
    }

    // Image files given on the command line are opened as tabs.
    let files: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("xpaint")
            .with_app_id("xpaint")
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([640.0, 400.0])
            .with_icon(window_icon()),
        // Low latency, so painting keeps up with the mouse. The default queues up to
        // two frames and waits for vsync, which lets strokes trail behind the cursor.
        // egui only redraws on input, so dropping vsync costs nothing while idle;
        // `AutoNoVsync` falls back to vsync where presenting without it is unsupported.
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            surface: eframe::egui_wgpu::SurfaceConfig {
                present_mode: eframe::wgpu::PresentMode::AutoNoVsync,
                desired_maximum_frame_latency: Some(1),
            },
            ..Default::default()
        },
        ..Default::default()
    };

    eframe::run_native(
        "xpaint",
        options,
        Box::new(|cc| Ok(Box::new(app::XPaintApp::new(cc, files)))),
    )
}

/// Lets `--version`, `--install` etc. print to the terminal they were run from.
/// Without a console (started from Explorer) or with redirected output (run by
/// the updater) nothing changes.
#[cfg(windows)]
fn attach_console() {
    use windows_sys::Win32::System::Console::{
        ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_OUTPUT_HANDLE,
    };
    // SAFETY: plain Win32 calls without pointers.
    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        if handle.is_null() {
            AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}

#[cfg(not(windows))]
fn attach_console() {}

fn window_icon() -> egui::IconData {
    let icon = image::load_from_memory(app::ICON_PNG)
        .expect("embedded icon is a valid PNG")
        .to_rgba8();
    egui::IconData {
        width: icon.width(),
        height: icon.height(),
        rgba: icon.into_raw(),
    }
}
