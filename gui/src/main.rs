mod app;
mod capture;
mod controls;
mod devices;
mod ffi;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 740.0])
            .with_min_inner_size([760.0, 480.0])
            .with_title("Anker Webcam Settings"),
        ..Default::default()
    };
    eframe::run_native(
        "Anker Webcam Settings",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
