use std::collections::HashMap;

use egui::{CollapsingHeader, ScrollArea, Ui};

use crate::models::*;
use crate::theme::THEME;
use crate::util::*;
use crate::widgets::SearchBar;

pub struct CleanupPanel {
    pub search_bar: SearchBar,
}

impl CleanupPanel {
    pub fn new() -> Self {
        Self {
            search_bar: SearchBar {
                search: String::new(),
                min_size_mb: 0.0,
            },
        }
    }

    pub fn matches_filters(&self, e: &CleanupEntry) -> bool {
        let min = (self.search_bar.min_size_mb as f64 * 1024.0 * 1024.0) as u64;
        if e.size_bytes < min {
            return false;
        }
        if self.search_bar.search.trim().is_empty() {
            return true;
        }
        let q = self.search_bar.search.to_lowercase();
        e.label.to_lowercase().contains(&q)
            || e.path.to_lowercase().contains(&q)
            || e.section.to_lowercase().contains(&q)
    }

    pub fn show(&mut self, ui: &mut Ui, entries: &mut Vec<CleanupEntry>) {
        if entries.is_empty() {
            return;
        }

        ui.horizontal(|ui| {
            self.search_bar.show(ui);
        });
        ui.separator();

        let filtered_ids: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter(|(_, e)| self.matches_filters(e))
            .map(|(i, _)| i)
            .collect();

        if filtered_ids.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.label("No items match the current filters.");
            });
            return;
        }

        let mut section_totals: HashMap<String, u64> = HashMap::new();
        for &i in &filtered_ids {
            *section_totals
                .entry(entries[i].section.clone())
                .or_insert(0) += entries[i].size_bytes;
        }
        let mut sections: Vec<String> = section_totals.keys().cloned().collect();
        sections.sort_by(|a, b| section_totals[b].cmp(&section_totals[a]));

        ScrollArea::vertical()
            .id_salt("cleanup_scroll")
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                for section in &sections {
                    let mut indices: Vec<usize> = filtered_ids
                        .iter()
                        .copied()
                        .filter(|&i| entries[i].section == *section)
                        .collect();
                    indices.sort_by(|&a, &b| entries[b].size_bytes.cmp(&entries[a].size_bytes));
                    let all_sel = indices.iter().all(|&i| entries[i].selected);
                    let sec_total = section_totals[section];

                    CollapsingHeader::new(format!(
                        "{} {}  —  {}",
                        section_icon(section),
                        section,
                        format_size(sec_total)
                    ))
                    .default_open(true)
                    .id_salt(section.clone())
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let mut a = all_sel;
                            if ui.checkbox(&mut a, "select all in section").clicked() {
                                for &i in &indices {
                                    entries[i].selected = a;
                                }
                            }
                        });
                        ui.separator();
                        for &i in &indices {
                            let e = &mut entries[i];
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut e.selected, "");
                                ui.label(format_size(e.size_bytes));
                                match &e.orphan_confidence {
                                    Some(OrphanConfidence::High) => {
                                        ui.colored_label(THEME.error, &e.label);
                                    }
                                    Some(OrphanConfidence::Medium) => {
                                        ui.colored_label(THEME.warning, &e.label);
                                    }
                                    Some(OrphanConfidence::Low) => {
                                        ui.colored_label(THEME.text_muted, &e.label);
                                    }
                                    None => {
                                        ui.label(&e.label);
                                    }
                                }
                                if !e.path.is_empty() {
                                    ui.label(format!("  —  {}", e.path))
                                        .on_hover_text(&e.path);
                                }
                            });
                        }
                    });
                }
            });
    }
}
