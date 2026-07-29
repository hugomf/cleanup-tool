use std::time::Duration;

use egui::{Button, ProgressBar, RichText, Ui};

use crate::models::DiskSpace;
use crate::theme::THEME;
use crate::util::format_size;

pub struct Header;

impl Header {
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        ui: &mut Ui,
        disk: &DiskSpace,
        scanning: bool,
        progress: &str,
        done: bool,
        entry_count: usize,
        deleting: bool,
        duration: Option<Duration>,
        show_sidebar: bool,
    ) -> (bool, bool) {
        let mut rescan = false;
        let mut toggle_sidebar = false;

        ui.horizontal(|ui| {
            let hamburger = if show_sidebar {
                "✕"
            } else {
                "☰"
            };
            if ui
                .add(Button::new(RichText::new(hamburger).size(16.0)).frame(false))
                .clicked()
            {
                toggle_sidebar = true;
            }

            ui.add_space(8.0);

            ui.label(RichText::new("Mac Cleaner").size(16.0).strong());

            ui.add_space(16.0);

            let used = disk.total_bytes.saturating_sub(disk.available_bytes);
            let frac = if disk.total_bytes > 0 {
                used as f32 / disk.total_bytes as f32
            } else {
                0.0
            };
            ui.add(
                ProgressBar::new(frac)
                    .desired_width(120.0)
                    .text(format!("{} / {}", format_size(used), format_size(disk.total_bytes))),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if done && !deleting {
                    if ui
                        .button(RichText::new("🔄 Rescan").color(THEME.accent))
                        .clicked()
                    {
                        rescan = true;
                    }
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(format!(
                            "✓ {} items found in {:.1}s",
                            entry_count,
                            duration.map(|d| d.as_secs_f64()).unwrap_or(0.0)
                        ))
                        .color(THEME.text_secondary),
                    );
                } else if scanning {
                    ui.label(
                        RichText::new(progress)
                            .color(THEME.text_secondary),
                    );
                    ui.add_space(6.0);
                    ui.spinner();
                } else if deleting {
                    ui.label(RichText::new("Cleaning...").color(THEME.warning));
                    ui.spinner();
                }
            });
        });

        (rescan, toggle_sidebar)
    }
}