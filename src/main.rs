mod models;
mod theme;
mod util;
mod scanning;
mod widgets;
mod panels;
mod app;

use app::CleanupApp;
use theme::THEME;

fn main() -> eframe::Result<()> {
    eframe::run_native(
        "Mac Cleaner",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([960.0, 680.0]),
            ..Default::default()
        },
        Box::new(|cc| {
            let mut visuals = egui::Visuals::dark();
            visuals.panel_fill = THEME.content_bg;
            visuals.window_fill = THEME.card_bg;
            visuals.widgets.noninteractive.bg_fill = THEME.card_bg;
            visuals.widgets.inactive.bg_fill = THEME.sidebar_bg;
            visuals.widgets.inactive.fg_stroke.color = THEME.text_secondary;
            visuals.widgets.active.bg_fill = THEME.accent;
            visuals.widgets.hovered.bg_fill = THEME.accent;
            visuals.selection.bg_fill = THEME.accent;
            cc.egui_ctx.set_visuals(visuals);

            let mut style = (*cc.egui_ctx.global_style()).clone();
            style.spacing.button_padding = egui::vec2(10.0, 4.0);
            style.spacing.item_spacing = egui::vec2(6.0, 4.0);
            style.visuals.widgets.inactive.corner_radius = 6.0.into();
            style.visuals.widgets.active.corner_radius = 6.0.into();
            style.visuals.widgets.hovered.corner_radius = 6.0.into();
            cc.egui_ctx.set_global_style(style);

            let mut app = CleanupApp::default();
            app.restart_scan();
            Ok(Box::new(app))
        }),
    )
}