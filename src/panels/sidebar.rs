use egui::{Color32, Frame, RichText, Sense, Ui};

use crate::models::AppView;
use crate::theme::THEME;

pub struct Sidebar {
    pub selected_view: AppView,
}

impl Sidebar {
    pub fn new() -> Self {
        Self {
            selected_view: AppView::Dashboard,
        }
    }

    pub fn show(&mut self, ui: &mut Ui) {
        Frame::NONE
            .fill(THEME.sidebar_bg)
            .show(ui, |ui| {
                ui.add_space(12.0);
                let items = [
                    (AppView::Dashboard, "📊", "Dashboard"),
                    (AppView::Cleanup, "🧹", "Cleanup"),
                    (AppView::Applications, "📱", "Applications"),
                    (AppView::LargeFiles, "🐘", "Large Files"),
                ];
                for (view, icon, label) in &items {
                    let selected = self.selected_view == *view;
                    let bg = if selected {
                        THEME.accent
                    } else {
                        Color32::TRANSPARENT
                    };
                    let response = Frame::NONE
                        .fill(bg)
                        .corner_radius(6)
                        .show(ui, |ui| {
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.add_space(10.0);
                                let text = format!("{icon}  {label}");
                                ui.label(
                                    RichText::new(text).color(if selected {
                                        Color32::WHITE
                                    } else {
                                        THEME.text_secondary
                                    }),
                                );
                            });
                            ui.add_space(4.0);
                        });
                    let sense = ui.interact(response.response.rect, ui.next_auto_id(), Sense::click());
                    if sense.clicked() {
                        self.selected_view = *view;
                    }
                    ui.add_space(2.0);
                }
                ui.add_space(12.0);
            });
    }
}
