use std::collections::HashSet;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use egui::{Align, Button, Layout, RichText, ScrollArea, Ui};

use crate::models::*;
use crate::panels::*;
use crate::theme::THEME;
use crate::util::*;
use crate::widgets::DangerDialog;
use crate::widgets::ToastMessage;

pub struct CleanupApp {
    pub disk: DiskSpace,
    pub entries: Vec<CleanupEntry>,
    pub scanning: bool,
    pub scan_progress: String,
    pub done: bool,
    pub scan_rx: Option<mpsc::Receiver<ScanEvent>>,
    pub deleting: bool,
    pub deletion_rx: Option<mpsc::Receiver<DeletionResult>>,
    pub dry_run: bool,
    pub log_messages: Vec<String>,
    pub show_log: bool,
    pub toast: Option<ToastMessage>,
    pub toast_timer: f32,
    pub scan_start: Option<Instant>,
    pub scan_duration: Option<Duration>,
    pub show_sidebar: bool,
    pub sidebar: Sidebar,
    pub cleanup_panel: cleanup::CleanupPanel,
    pub applications_panel: applications::ApplicationsPanel,
    pub confirm: DangerDialog,
    pub apps_scan_rx: Option<mpsc::Receiver<AppScanEvent>>,
}

impl Default for CleanupApp {
    fn default() -> Self {
        Self {
            disk: get_disk_space("/"),
            entries: vec![],
            scanning: false,
            scan_progress: String::new(),
            done: false,
            scan_rx: None,
            deleting: false,
            deletion_rx: None,
            dry_run: false,
            log_messages: vec![],
            show_log: false,
            toast: None,
            toast_timer: 0.0,
            scan_start: None,
            scan_duration: None,
            sidebar: Sidebar::new(),
            show_sidebar: true,
            cleanup_panel: cleanup::CleanupPanel::new(),
            applications_panel: applications::ApplicationsPanel::new(),
            confirm: DangerDialog::new(),
            apps_scan_rx: None,
        }
    }
}

impl CleanupApp {
    pub fn restart_scan(&mut self) {
        self.disk = get_disk_space("/");
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || crate::scanning::run_scan(tx));
        self.entries.clear();
        self.scanning = true;
        self.done = false;
        self.scan_progress = "Starting scan...".into();
        self.scan_rx = Some(rx);
        self.deleting = false;
        self.log_messages.clear();
        self.toast = None;
        self.toast_timer = 0.0;
        self.scan_start = Some(Instant::now());
        self.scan_duration = None;
    }

    pub fn start_apps_scan(&mut self) {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || crate::scanning::scan_installed_apps(&tx));
        self.applications_panel.installed_apps.clear();
        self.applications_panel.scanning = true;
        self.apps_scan_rx = Some(rx);
        self.applications_panel.uninstall_target = None;
        self.applications_panel.app_traces.clear();
    }

    fn set_toast(&mut self, msg: impl Into<String>, is_error: bool) {
        self.toast = Some(ToastMessage::new(msg, is_error));
        self.toast_timer = 3.5;
    }

    fn footer_ui(&mut self, ui: &mut Ui, _dt: f32) {
        ui.horizontal(|ui| {
            let selected_bytes: u64 = self
                .entries
                .iter()
                .filter(|e| e.selected)
                .map(|e| e.size_bytes)
                .sum();
            let selected_count = self.entries.iter().filter(|e| e.selected).count();
            let total_bytes: u64 = self.entries.iter().map(|e| e.size_bytes).sum();

            ui.label(format!(
                "Total found: {}  —  Selected: {selected_count} items ({})",
                format_size(total_bytes),
                format_size(selected_bytes),
            ));

            ui.separator();

            let filtered_ids: Vec<usize> = self
                .entries
                .iter()
                .enumerate()
                .filter(|(_, e)| self.cleanup_panel.matches_filters(e))
                .map(|(i, _)| i)
                .collect();

            if ui.button("Select all (filtered)").clicked() {
                for &i in &filtered_ids {
                    self.entries[i].selected = true;
                }
            }
            if ui.button("Clear selection").clicked() {
                for e in &mut self.entries {
                    e.selected = false;
                }
            }
            if ui.button("Select >=100 MB").clicked() {
                for &i in &filtered_ids {
                    if self.entries[i].size_bytes >= 100 * 1024 * 1024 {
                        self.entries[i].selected = true;
                    }
                }
            }
            if ui.button("Select high-confidence orphans").clicked() {
                for &i in &filtered_ids {
                    if self.entries[i].orphan_confidence == Some(OrphanConfidence::High) {
                        self.entries[i].selected = true;
                    }
                }
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let log_label = if self.log_messages.is_empty() {
                    "Log".into()
                } else {
                    format!("Log ({})", self.log_messages.len())
                };
                if ui.button(log_label).clicked() {
                    self.show_log = !self.show_log;
                }

                ui.checkbox(&mut self.dry_run, "Dry run");

                if ui
                    .add_enabled(
                        selected_count > 0 && !self.deleting,
                        Button::new(RichText::new("🧹 Clean Selected").color(THEME.text_primary)),
                    )
                    .clicked()
                {
                    let items: Vec<CleanupEntry> =
                        self.entries.iter().filter(|e| e.selected).cloned().collect();
                    self.confirm.open(items, self.dry_run);
                }

                if let Some(ref t) = self.toast {
                    t.show(ui);
                }
            });
        });
    }
}

impl eframe::App for CleanupApp {
    fn ui(&mut self, _: &mut egui::Ui, _: &mut eframe::Frame) {}
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Drain scan events
        if let Some(rx) = &self.scan_rx {
            while let Ok(event) = rx.try_recv() {
                match event {
                    ScanEvent::Progress(msg) => self.scan_progress = msg,
                    ScanEvent::Entry(e) => self.entries.push(e),
                    ScanEvent::Warning(w) => self.log_messages.push(format!("⚠️  {w}")),
                    ScanEvent::Done => {
                        self.scanning = false;
                        self.done = true;
                        self.scan_progress = "Scan complete.".into();
                        self.scan_duration = self.scan_start.map(|s| s.elapsed());
                    }
                }
                ctx.request_repaint();
            }
        }

        // Drain app scan events
        if let Some(rx) = &self.apps_scan_rx {
            while let Ok(event) = rx.try_recv() {
                match event {
                    AppScanEvent::Progress(_) => {}
                    AppScanEvent::Entry(a) => self.applications_panel.installed_apps.push(a),
                    AppScanEvent::Done => {
                        self.applications_panel.scanning = false;
                    }
                }
                ctx.request_repaint();
            }
        }

        // Drain deletion results
        if let Some(rx) = &self.deletion_rx {
            let mut done = false;
            let mut deleted_ids: Vec<usize> = vec![];
            let mut deleted_paths: Vec<String> = vec![];
            let mut had_error = false;

            while let Ok(result) = rx.try_recv() {
                match result {
                    DeletionResult::Deleted(path, id) => {
                        self.log_messages.push(format!("✅ Deleted: {path}"));
                        deleted_ids.push(id);
                        if !path.is_empty() {
                            deleted_paths.push(path);
                        }
                    }
                    DeletionResult::Error(e) => {
                        self.log_messages.push(format!("❌ {e}"));
                        self.show_log = true;
                        had_error = true;
                    }
                    DeletionResult::DryRunPreview(msg) => {
                        self.log_messages.push(msg);
                        self.show_log = true;
                    }
                    DeletionResult::Done => {
                        done = true;
                    }
                }
            }

            if !deleted_ids.is_empty() {
                let id_set: HashSet<usize> = deleted_ids.into_iter().collect();
                self.entries.retain(|e| !id_set.contains(&e.id));
                self.applications_panel
                    .app_traces
                    .retain(|e| !id_set.contains(&e.id));
            }

            if !deleted_paths.is_empty() {
                let path_set: HashSet<String> = deleted_paths.into_iter().collect();
                self.applications_panel
                    .installed_apps
                    .retain(|a| !path_set.contains(&a.app_path));
                if let Some(target_id) = self.applications_panel.uninstall_target {
                    if !self
                        .applications_panel
                        .installed_apps
                        .iter()
                        .any(|a| a.id == target_id)
                    {
                        self.applications_panel.uninstall_target = None;
                        self.applications_panel.app_traces.clear();
                    }
                }
            }

            if done {
                self.deleting = false;
                self.deletion_rx = None;
                self.disk = get_disk_space("/");
                if had_error {
                    self.set_toast("Cleanup finished with some errors — check the log.", true);
                } else {
                    self.set_toast("Cleanup complete!", false);
                }
            }

            ctx.request_repaint();
        }

        // Toast timer
        if self.toast.is_some() {
            self.toast_timer -= ctx.input(|i| i.unstable_dt);
            if self.toast_timer <= 0.0 {
                self.toast = None;
            }
            ctx.request_repaint();
        }

        // Header
        #[allow(deprecated)]
        egui::Panel::top("header").show(ctx, |ui| {
            let (rescan, toggle_sidebar) = Header::show(
                ui,
                &self.disk,
                self.scanning,
                &self.scan_progress,
                self.done,
                self.entries.len(),
                self.deleting,
                self.scan_duration,
                self.show_sidebar,
            );
            if rescan {
                self.restart_scan();
            }
            if toggle_sidebar {
                self.show_sidebar = !self.show_sidebar;
            }
        });

        // Sidebar
        if self.show_sidebar {
            #[allow(deprecated)]
            egui::Panel::left("sidebar")
                .resizable(false)
                .default_width(180.0)
                .show(ctx, |ui| {
                    let prev = self.sidebar.selected_view;
                    self.sidebar.show(ui);
                    if self.sidebar.selected_view != prev
                        && self.sidebar.selected_view == AppView::Applications
                        && self.applications_panel.installed_apps.is_empty()
                        && !self.applications_panel.scanning
                    {
                        self.start_apps_scan();
                    }
                });
        }

        // Footer
        #[allow(deprecated)]
        egui::Panel::bottom("footer").show(ctx, |ui| {
            self.footer_ui(ui, ctx.input(|i| i.unstable_dt));
        });

        // Central panel
        #[allow(deprecated)]
        egui::CentralPanel::default().show(ctx, |ui| match self.sidebar.selected_view {
            AppView::Dashboard => {
                let mut do_scan = false;
                Dashboard::show(ui, &self.disk, &self.entries, self.scanning, &mut do_scan);
                if do_scan {
                    self.restart_scan();
                }
            }
            AppView::Cleanup => {
                if self.entries.is_empty() && self.scanning {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.heading("Scanning...");
                        ui.spinner();
                        ui.label(&self.scan_progress);
                    });
                    return;
                }
                if self.entries.is_empty() && !self.scanning {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label("Nothing to clean!");
                    });
                    return;
                }
                self.cleanup_panel.show(ui, &mut self.entries);
            }
            AppView::Applications => {
                let mut confirm_items = vec![];
                let mut show_confirm = false;
                let mut rescan = false;
                self.applications_panel.show(
                    ui,
                    &mut self.dry_run,
                    &mut confirm_items,
                    &mut show_confirm,
                    &mut rescan,
                );
                if rescan {
                    self.start_apps_scan();
                }
                if show_confirm {
                    self.confirm.open(confirm_items, self.dry_run);
                }
            }
            AppView::LargeFiles => {
                LargeFilesPanel::show(ui, &mut self.entries);
            }
        });

        // Confirmation dialog
        if self.confirm.show {
            if let Some(rx) = self.confirm.show(ctx, self.disk.available_bytes) {
                self.deletion_rx = Some(rx);
                self.deleting = self.confirm.deleting;
            }
        }

        // Log window
        if self.show_log && !self.log_messages.is_empty() {
            egui::Window::new("📋 Log")
                .resizable(true)
                .default_size([600.0, 300.0])
                .open(&mut self.show_log)
                .show(ctx, |ui| {
                    ScrollArea::vertical()
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for msg in &self.log_messages {
                                ui.monospace(msg);
                            }
                        });
                    if ui.button("Clear log").clicked() {
                        self.log_messages.clear();
                    }
                });
        }
    }
}
