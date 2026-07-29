use egui::{ScrollArea, Ui};

use crate::models::*;
use crate::util::*;

pub struct LargeFilesPanel;

impl LargeFilesPanel {
    pub fn show(ui: &mut Ui, entries: &mut [CleanupEntry]) {
        let mut large: Vec<&mut CleanupEntry> = entries
            .iter_mut()
            .filter(|e| e.section == "Large Files")
            .collect();

        if large.is_empty() {
            ui.label("No large files found.");
            return;
        }

        let mut sorted_indices: Vec<usize> = (0..large.len()).collect();
        sorted_indices.sort_by(|&a, &b| large[b].size_bytes.cmp(&large[a].size_bytes));

        ScrollArea::vertical()
            .id_salt("large_files_scroll")
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Size");
                    ui.add_space(60.0);
                    ui.label("Path");
                });
                ui.separator();
                for &idx in &sorted_indices {
                    let e = &mut large[idx];
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut e.selected, "");
                        ui.label(format_size(e.size_bytes));
                        ui.label(&e.label).on_hover_text(&e.path);
                    });
                }
            });
    }
}
