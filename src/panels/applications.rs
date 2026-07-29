use egui::{Button, ScrollArea, TextEdit, Ui};

use crate::models::*;
use crate::scanning::find_app_traces;
use crate::util::*;

pub struct ApplicationsPanel {
    pub app_search: String,
    pub installed_apps: Vec<InstalledApp>,
    pub app_traces: Vec<CleanupEntry>,
    pub uninstall_target: Option<usize>,
    pub scanning: bool,
}

impl ApplicationsPanel {
    pub fn new() -> Self {
        Self {
            app_search: String::new(),
            installed_apps: vec![],
            app_traces: vec![],
            uninstall_target: None,
            scanning: false,
        }
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        dry_run: &mut bool,
        confirm_items: &mut Vec<CleanupEntry>,
        confirm_show: &mut bool,
        rescan: &mut bool,
    ) {
        ui.heading("🗑️ Uninstall Apps Completely");
        ui.label(
            "Removes the app bundle plus its Application Support, Caches, \
             Preferences, Containers, and other user-level files together.",
        );
        ui.small(
            "Scope note: only ~/Library locations are scanned. System-level \
             files (/Library, LaunchDaemons, package receipts) need admin \
             rights and aren't touched here.",
        );
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("🔎");
            ui.add(
                TextEdit::singleline(&mut self.app_search)
                    .hint_text("Filter installed apps...")
                    .desired_width(280.0),
            );
            if !self.app_search.is_empty() && ui.button("✕").clicked() {
                self.app_search.clear();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.scanning {
                    ui.spinner();
                    ui.label("Scanning apps...");
                } else if ui.button("🔄 Rescan Apps").clicked() {
                    *rescan = true;
                }
            });
        });
        ui.separator();

        if self.installed_apps.is_empty() && !self.scanning {
            ui.label("No apps found yet — click Rescan Apps.");
        }

        let q = self.app_search.to_lowercase();
        let mut apps: Vec<&InstalledApp> = self
            .installed_apps
            .iter()
            .filter(|a| q.is_empty() || a.name.to_lowercase().contains(&q))
            .collect();
        apps.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));

        let mut clicked_app_id: Option<usize> = None;

        ScrollArea::vertical()
            .id_salt("installed_apps_scroll")
            .auto_shrink([false; 2])
            .max_height(240.0)
            .show(ui, |ui| {
                for app in &apps {
                    ui.horizontal(|ui| {
                        ui.label(format_size(app.size_bytes));
                        ui.label(&app.name);
                        ui.weak(&app.bundle_id);
                        if ui.button("Find traces & select").clicked() {
                            clicked_app_id = Some(app.id);
                        }
                    });
                }
            });

        if let Some(id) = clicked_app_id {
            if let Some(app) = self.installed_apps.iter().find(|a| a.id == id) {
                let mut traces = find_app_traces(app);
                for t in &mut traces {
                    t.selected = true;
                }
                self.app_traces = traces;
                self.uninstall_target = Some(id);
            }
        }

        if let Some(target_id) = self.uninstall_target {
            if self
                .installed_apps
                .iter()
                .any(|a| a.id == target_id)
            {
                let total_sel: u64 = self
                    .app_traces
                    .iter()
                    .filter(|e| e.selected)
                    .map(|e| e.size_bytes)
                    .sum();
                ui.separator();
                ui.label(format!(
                    "{} items found — {} selected for removal",
                    self.app_traces.len(),
                    format_size(total_sel)
                ));

                ScrollArea::vertical()
                    .id_salt("app_traces_scroll")
                    .auto_shrink([false; 2])
                    .max_height(240.0)
                    .show(ui, |ui| {
                        for t in &mut self.app_traces {
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut t.selected, "");
                                ui.label(format_size(t.size_bytes));
                                ui.label(format!("{} {}", section_icon(&t.section), t.label));
                                ui.weak(&t.path);
                            });
                        }
                    });

                ui.horizontal(|ui| {
                    if ui.button("Select all").clicked() {
                        for t in &mut self.app_traces {
                            t.selected = true;
                        }
                    }
                    if ui.button("Deselect all").clicked() {
                        for t in &mut self.app_traces {
                            t.selected = false;
                        }
                    }
                    let any_selected = self.app_traces.iter().any(|e| e.selected);
                    let btn_label = if *dry_run {
                        "🔍 Preview Uninstall (Dry Run)"
                    } else {
                        "🗑️ Uninstall Completely"
                    };
                    if ui
                        .add_enabled(any_selected, Button::new(btn_label))
                        .clicked()
                    {
                        *confirm_items = self
                            .app_traces
                            .iter()
                            .filter(|e| e.selected)
                            .cloned()
                            .collect();
                        *confirm_show = true;
                    }
                });
            }
        }
    }
}
