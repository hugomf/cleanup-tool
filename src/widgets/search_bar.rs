use egui::{Slider, TextEdit, Ui};

#[derive(Clone)]
pub struct SearchBar {
    pub search: String,
    pub min_size_mb: f32,
}

impl SearchBar {
    pub fn show(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label("🔎");
            ui.add(
                TextEdit::singleline(&mut self.search)
                    .hint_text("Filter by name, path, or category...")
                    .desired_width(280.0),
            );
            if !self.search.is_empty() && ui.button("✕").clicked() {
                self.search.clear();
            }
            ui.separator();
            ui.label("Hide items smaller than:");
            ui.add(
                Slider::new(&mut self.min_size_mb, 0.0..=500.0)
                    .suffix(" MB")
                    .fixed_decimals(0),
            );
            if self.min_size_mb > 0.0 && ui.button("Reset").clicked() {
                self.min_size_mb = 0.0;
            }
        });
    }
}
