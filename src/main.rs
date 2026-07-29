mod models;
mod theme;
mod util;
mod scanning;
mod widgets;
mod panels;
mod app;

use app::CleanupApp;

fn main() -> eframe::Result<()> {
    eframe::run_native(
        "Mac Cleaner",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([960.0, 680.0]),
            ..Default::default()
        },
        Box::new(|_cc| {
            let mut app = CleanupApp::default();
            app.restart_scan();
            Ok(Box::new(app))
        }),
    )
}
