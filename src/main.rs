mod app;
mod clipboard;
mod document;
mod shapes;
mod text;

use std::path::PathBuf;

fn main() -> eframe::Result {
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
