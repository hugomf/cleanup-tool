use egui::{Color32, Ui};

use crate::theme::THEME;

pub struct ToastMessage {
    pub text: String,
    pub is_error: bool,
}

impl ToastMessage {
    pub fn new(text: impl Into<String>, is_error: bool) -> Self {
        Self {
            text: text.into(),
            is_error,
        }
    }

    pub fn show(&self, ui: &mut Ui) {
        let color = if self.is_error {
            THEME.error
        } else {
            Color32::from_rgb(60, 180, 90)
        };
        let icon = if self.is_error { "⚠️" } else { "✅" };
        ui.colored_label(color, icon);
        ui.label(&self.text);
    }
}
