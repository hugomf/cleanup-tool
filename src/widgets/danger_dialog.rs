use egui::{Align2, Color32, Context, ScrollArea};
use std::sync::mpsc;

use crate::models::*;
use crate::scanning::execute_cleanup;
use crate::util::format_size;

pub struct DangerDialog {
    pub show: bool,
    pub items: Vec<CleanupEntry>,
    pub dry_run: bool,
    pub deleting: bool,
}

impl DangerDialog {
    pub fn new() -> Self {
        Self {
            show: false,
            items: vec![],
            dry_run: false,
            deleting: false,
        }
    }

    pub fn open(&mut self, items: Vec<CleanupEntry>, dry_run: bool) {
        self.show = true;
        self.items = items;
        self.dry_run = dry_run;
    }

    pub fn show(
        &mut self,
        ctx: &Context,
        disk_available: u64,
    ) -> Option<mpsc::Receiver<DeletionResult>> {
        let mut result: Option<mpsc::Receiver<DeletionResult>> = None;
        if !self.show {
            return None;
        }

        let total_sel: u64 = self.items.iter().map(|e| e.size_bytes).sum();
        egui::Window::new("⚠️  Confirm Deletion")
            .collapsible(false)
            .resizable(true)
            .default_size([640.0, 460.0])
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                if self.dry_run {
                    ui.heading("🔍 Dry Run Preview");
                    ui.label("Nothing will be deleted. This shows what would be removed:");
                } else {
                    ui.heading("WARNING: This action cannot be undone!");
                    ui.label("The following items will be permanently deleted:");
                }
                ui.label("");
                ui.monospace(format!(
                    "Total: {} items — {}",
                    self.items.len(),
                    format_size(total_sel)
                ));

                if !self.dry_run {
                    let projected = disk_available.saturating_add(total_sel);
                    ui.horizontal(|ui| {
                        ui.label("Available space after cleanup:");
                        ui.strong(format_size(projected));
                        ui.label(format!("(+{})", format_size(total_sel)));
                    });
                }
                ui.separator();

                ScrollArea::vertical()
                    .max_height(ui.available_height() - 80.0)
                    .show(ui, |ui| {
                        for item in &self.items {
                            ui.horizontal(|ui| {
                                ui.label(format_size(item.size_bytes));
                                ui.label(if item.path.is_empty() {
                                    format!("[action] {}", item.label)
                                } else {
                                    format!("{}  —  {}", item.label, item.path)
                                });
                            });
                        }
                    });

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.show = false;
                        self.items.clear();
                    }
                    let btn_label = if self.dry_run {
                        "🔍 Preview (Dry Run)"
                    } else {
                        "Confirm Deletion 🗑️"
                    };
                    let btn_color = if self.dry_run {
                        Color32::from_rgb(50, 100, 180)
                    } else {
                        Color32::RED
                    };
                    if ui
                        .add_sized(
                            [160.0, 30.0],
                            egui::Button::new(btn_label).fill(btn_color),
                        )
                        .clicked()
                    {
                        let items = self.items.clone();
                        let (tx, rx) = mpsc::channel();
                        execute_cleanup(items, self.dry_run, tx);
                        self.deleting = !self.dry_run;
                        self.show = false;
                        self.items.clear();
                        result = Some(rx);
                    }
                });
            });
        result
    }
}
