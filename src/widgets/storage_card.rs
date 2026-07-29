use egui::{Color32, Frame, RichText, Ui};

use crate::theme::THEME;

pub struct StorageCard {
    pub label: String,
    pub value: String,
    pub accent: Color32,
}

impl StorageCard {
    pub fn new(label: impl Into<String>, value: impl Into<String>, accent: Color32) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            accent,
        }
    }

    pub fn show(&self, ui: &mut Ui) {
        Frame::NONE
            .fill(THEME.card_bg)
            .corner_radius(8)
            .show(ui, |ui| {
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&self.label).size(11.0).color(THEME.text_muted));
                        ui.add_space(2.0);
                        ui.label(
                            RichText::new(&self.value)
                                .size(20.0)
                                .color(self.accent)
                                .strong(),
                        );
                    });
                });
                ui.add_space(12.0);
            });
    }
}
