use egui::{Button, Frame, ProgressBar, RichText, Ui};

use crate::models::{CleanupEntry, DiskSpace};
use crate::theme::THEME;
use crate::util::format_size;
use crate::widgets::StorageCard;

pub struct Dashboard;

impl Dashboard {
    pub fn show(
        ui: &mut Ui,
        disk: &DiskSpace,
        entries: &[CleanupEntry],
        scanning: bool,
        on_scan: &mut bool,
    ) {
        let total_recoverable: u64 = entries.iter().map(|e| e.size_bytes).sum();
        let used = disk.total_bytes.saturating_sub(disk.available_bytes);
        let frac = if disk.total_bytes > 0 {
            used as f32 / disk.total_bytes as f32
        } else {
            0.0
        };

        let _ = scanning; // used for future state display
        ui.add_space(24.0);

        // Storage card
        Frame::NONE
            .fill(THEME.card_bg)
            .corner_radius(10)
            .show(ui, |ui| {
                ui.add_space(20.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new("Storage")
                            .size(11.0)
                            .color(THEME.text_muted),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format_size(disk.total_bytes))
                            .size(32.0)
                            .color(THEME.accent)
                            .strong(),
                    );
                    ui.add_space(10.0);
                    ui.add(
                        ProgressBar::new(frac)
                            .desired_width(320.0)
                            .text(format!(
                                "{} used of {}",
                                format_size(used),
                                format_size(disk.total_bytes)
                            )),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("{} available", format_size(disk.available_bytes)))
                            .color(THEME.text_secondary),
                    );
                });
                ui.add_space(20.0);
            });

        ui.add_space(20.0);

        ui.columns(2, |cols| {
// Left column — Recoverable Space
            StorageCard::new(
                "Recoverable Space",
                format_size(total_recoverable),
                THEME.success,
            )
.show(&mut cols[0]);

            // Right column — Actions
            Frame::NONE
                .fill(THEME.card_bg)
                .corner_radius(10)
                .show(&mut cols[1], |ui| {
                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        ui.add_space(14.0);
                        ui.label(
                            RichText::new("Actions")
                                .size(13.0)
                                .color(THEME.text_primary),
                        );
                    });
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.add_space(14.0);
                        ui.vertical(|ui| {
                            let full_scan = Button::new(
                                RichText::new("  🔄  Full Scan")
                                    .size(13.0)
                                    .color(THEME.text_primary),
                            )
                            .fill(THEME.card_bg)
                            .min_size(egui::vec2(160.0, 36.0));
                            if ui.add(full_scan).clicked() {
                                *on_scan = true;
                            }

                            ui.add_space(8.0);

                            let clean_btn = Button::new(
                                RichText::new("  ✓  Clean Recommended")
                                    .size(13.0)
                                    .color(egui::Color32::WHITE),
                            )
                            .fill(THEME.success)
                            .min_size(egui::vec2(160.0, 36.0));
                            if ui.add(clean_btn).clicked() {
                                *on_scan = true;
                            }
                        });
                    });
                    ui.add_space(16.0);
                });
        });
    }
}