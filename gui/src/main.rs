mod app;
mod capture;
mod controls;
mod devices;
mod ffi;
mod profiles;

/// The app icon, embedded so the window has one even when run from the build tree.
fn window_icon() -> Option<egui::IconData> {
    let png = include_bytes!("../assets/icons/256.png");
    let image = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .ok()?
        .into_rgba8();
    let (width, height) = image.dimensions();
    Some(egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    })
}

fn main() -> eframe::Result {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1180.0, 740.0])
        .with_min_inner_size([760.0, 480.0])
        .with_app_id("anker-webcam-gui")
        .with_title("Anker Webcam Settings");
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "Anker Webcam Settings",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
