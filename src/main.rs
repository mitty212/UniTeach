mod ai;
mod app;
mod capture;
mod models;
mod session;
mod stealth;
mod ui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_transparent(true)
            .with_decorations(false)
            .with_always_on_top()
            .with_inner_size(egui::vec2(800.0, 650.0))
            .with_min_inner_size(egui::vec2(360.0, 320.0))
            .with_resizable(true)
            .with_title("UniTeach_Overlay_Window_Unique"),
        ..Default::default()
    };

    eframe::run_native(
        "UniTeach_Overlay_Window_Unique",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
