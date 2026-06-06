use std::process::Command;
use std::sync::mpsc;
use std::thread;

#[derive(Clone, Debug)]
struct CleanupEntry {
    id: usize,
    section: String,
    label: String,
    path: String,
    size_bytes: u64,
    selected: bool,
}

fn format_size(bytes: u64) -> String {
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    const KB: f64 = 1024.0;
    let b = bytes as f64;
    if b >= GB { format!("{:.1} GB", b / GB) }
    else if b >= MB { format!("{:.1} MB", b / MB) }
    else if b >= KB { format!("{:.1} KB", b / KB) }
    else { format!("{bytes} B") }
}

fn run_cmd_timeout(program: &str, args: &[&str], secs: u64) -> Option<std::process::Output> {
    let program = program.to_string();
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut cmd = std::process::Command::new(&program);
        cmd.args(&args);
        let _ = tx.send(cmd.output().ok());
    });
    match rx.recv_timeout(std::time::Duration::from_secs(secs)) {
        Ok(result) => result,
        Err(_) => None,
    }
}

fn du_sh(path: &str) -> u64 {
    let out = Command::new("/usr/bin/du").args(["-sk", path]).output().ok();
    if let Some(o) = out {
        if o.status.success() {
            let s = String::from_utf8_lossy(&o.stdout);
            if let Some(kb) = s.split_whitespace().next() {
                if let Ok(k) = kb.parse::<u64>() { return k * 1024; }
            }
        }
    }
    0
}

fn find_dirs(path: &str, name: &str, maxdepth: u32) -> Vec<String> {
    let depth_str = format!("{maxdepth}");
    Command::new("find")
        .args([path, "-maxdepth", &depth_str, "-type", "d", "-name", name])
        .output().ok().map(|o| {
            String::from_utf8_lossy(&o.stdout).lines()
                .filter(|l| !l.is_empty()).map(|s| s.to_string()).collect()
        }).unwrap_or_default()
}

enum ScanEvent {
    Progress(String),
    Entry(CleanupEntry),
    Done,
}

fn entry(id: usize, section: &str, label: &str, path: &str, size_bytes: u64) -> CleanupEntry {
    CleanupEntry { id, section: section.into(), label: label.into(), path: path.into(), size_bytes, selected: false }
}

fn scan_orphans(tx: &mpsc::Sender<ScanEvent>, next_id: &mut usize) {
    let _ = tx.send(ScanEvent::Progress("Detecting orphan app data...".into()));
    let home = std::env::var("HOME").unwrap_or_default();

    let mut known_ids: Vec<String> = vec![];
    let mut known_names: Vec<String> = vec![];

    for appdir in &["/Applications", &format!("{home}/Applications"), "/System/Applications"] {
        if let Some(o) = Command::new("find").args([appdir, "-maxdepth", "2", "-name", "*.app", "-type", "d"]).output().ok() {
            for line in String::from_utf8_lossy(&o.stdout).lines() {
                let line = line.trim();
                if line.is_empty() { continue; }
                let plist = format!("{line}/Contents/Info.plist");
                if !std::path::Path::new(&plist).exists() { continue; }
                if let Some(b) = Command::new("/usr/libexec/PlistBuddy").args(["-c", "Print :CFBundleIdentifier", &plist]).output().ok() {
                    let s = String::from_utf8_lossy(&b.stdout).trim().to_string();
                    if !s.is_empty() { known_ids.push(s.to_lowercase()); }
                }
                if let Some(name) = line.trim_end_matches(".app").split('/').last() {
                    let n = name.to_lowercase();
                    if !n.is_empty() { known_names.push(n); }
                }
            }
        }
    }

    for scantop in &[format!("{home}/Library/Application Support"), format!("{home}/Library/Preferences"),
                      format!("{home}/Library/Saved Application State"), format!("{home}/Library/Caches")] {
        let dir = match std::fs::read_dir(scantop) { Ok(d) => d, Err(_) => continue };
        for dir_entry_res in dir {
            let dir_entry = match dir_entry_res { Ok(e) => e, Err(_) => continue };
            let path = dir_entry.path();
            let name = match path.file_name() { Some(n) => n.to_string_lossy().to_string(), None => continue };
            let nl = name.to_lowercase();
            if nl.starts_with("com.apple.") || nl == "com.apple" || nl.starts_with("apple") || nl == "apple" || nl.starts_with('.') || nl == "caches" { continue; }
            if known_ids.iter().any(|kid| nl == *kid || nl.starts_with(&format!("{kid}.")) || kid.contains(&nl)) { continue; }
            if known_names.iter().any(|kn| nl == *kn || nl.contains(kn) || kn.contains(&nl)) { continue; }
            let sz = du_sh(&path.to_string_lossy());
            if sz > 0 {
                let _ = tx.send(ScanEvent::Entry(entry(*next_id, "Orphan App Data", &name, &path.to_string_lossy(), sz)));
                *next_id += 1;
            }
        }
    }
}

fn run_scan(tx: mpsc::Sender<ScanEvent>) {
    let mut next_id = 0usize;
    let home = std::env::var("HOME").unwrap_or_default();

    macro_rules! send_dir {
        ($section:expr, $label:expr, $path:expr) => {{
            let id = next_id; next_id += 1;
            let ls = $label; let _ = tx.send(ScanEvent::Progress(format!("Scanning {ls}...")));
            let pv = $path;
            let sz = du_sh(pv);
            let _ = tx.send(ScanEvent::Entry(entry(id, $section, ls, pv, sz)));
        }};
    }
    macro_rules! send_find {
        ($section:expr, $label:expr, $base:expr, $name:expr) => {{
            let ls = $label; let bp = $base; let dn = $name;
            let _ = tx.send(ScanEvent::Progress(format!("Scanning {ls}...")));
            let dirs = find_dirs(bp, dn, 5);
            for d in &dirs {
                let sz = du_sh(d);
                let short = d.strip_prefix(&format!("{bp}/")).unwrap_or(d);
                let _ = tx.send(ScanEvent::Entry(entry(next_id, $section, short, d, sz)));
                next_id += 1;
            }
        }};
    }

    send_dir!("System Caches", "~/Library/Caches", &format!("{home}/Library/Caches"));
    send_dir!("System Caches", "~/.cache", &format!("{home}/.cache"));

    send_dir!("Build Tools", "Gradle caches", &format!("{home}/.gradle/caches"));
    send_dir!("Build Tools", "Gradle wrappers", &format!("{home}/.gradle/wrapper"));
    send_dir!("Build Tools", "Cargo registry", &format!("{home}/.cargo/registry"));
    send_dir!("Build Tools", "Xcode DerivedData", &format!("{home}/Library/Developer/Xcode/DerivedData"));
    send_dir!("Build Tools", "Xcode Archives", &format!("{home}/Library/Developer/Xcode/Archives"));
    send_dir!("Build Tools", "iOS Device Logs", &format!("{home}/Library/Developer/Xcode/iOS Device Logs"));

    send_dir!("Go", "Go module cache", &format!("{home}/go/pkg/mod"));
    send_dir!("Go", "Go compiled binaries", &format!("{home}/go/bin"));
    send_dir!("Python", "pip cache", &format!("{home}/.cache/pip"));
    send_dir!("Python", "pip cache (macOS)", &format!("{home}/Library/Caches/pip"));

    let projects = format!("{home}/Projects");
    send_find!("Project Deps", "node_modules", &projects, "node_modules");
    send_find!("Project Deps", "target (Rust)", &projects, "target");
    send_find!("Project Deps", "build dirs", &projects, "build");
    send_find!("Project Deps", ".dart_tool", &projects, ".dart_tool");
    send_find!("Project Deps", "Pods (CocoaPods)", &projects, "Pods");
    send_find!("Project Deps", ".gradle (per-project)", &projects, ".gradle");
    send_find!("Project Deps", ".m2 (Maven)", &projects, ".m2");
    send_find!("Project Deps", "__pycache__", &projects, "__pycache__");
    send_find!("Project Deps", ".venv (Python)", &projects, ".venv");
    send_find!("Project Deps", "vendor dirs", &projects, "vendor");

    {
        let _ = tx.send(ScanEvent::Progress("Scanning npm cache...".into()));
        let sz = du_sh(&format!("{home}/.npm/_cacache"));
        let _ = tx.send(ScanEvent::Entry(entry(next_id, "Package Managers", "npm cache", &format!("{home}/.npm/_cacache"), sz)));
        next_id += 1;
    }
    {
        let _ = tx.send(ScanEvent::Progress("Scanning Homebrew cache...".into()));
        if let Some(o) = Command::new("brew").args(["--cache"]).output().ok() {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if !s.is_empty() {
                let sz = du_sh(&s);
                let _ = tx.send(ScanEvent::Entry(entry(next_id, "Package Managers", "Homebrew cache", &s, sz)));
                next_id += 1;
            }
        }
    }
    {
        let _ = tx.send(ScanEvent::Progress("Checking Docker...".into()));
        let docker_check = run_cmd_timeout("docker", &["info", "--format", "{{.ServerVersion}}"], 3);
        let sz = if let Some(ref out) = docker_check {
            if out.status.success() {
                if let Some(ref df) = run_cmd_timeout("docker", &["system", "df"], 5) {
                    String::from_utf8_lossy(&df.stdout).lines().last().map(|l| {
                        let p: Vec<&str> = l.split_whitespace().collect();
                        if p.len() >= 3 { parse_size_str(p[2]) } else { 0 }
                    }).unwrap_or(0)
                } else { 0 }
            } else { 0 }
        } else { 0 };
        let _ = tx.send(ScanEvent::Entry(entry(next_id, "Package Managers", "Docker (unused)", "", sz)));
        next_id += 1;
    }

    send_dir!("Logs & Temp", "~/Library/Logs", &format!("{home}/Library/Logs"));
    send_dir!("Logs & Temp", "/private/tmp", "/private/tmp");

    {
        let _ = tx.send(ScanEvent::Progress("Scanning Downloads...".into()));
        let downloads = format!("{home}/Downloads");
        if let Some(o) = Command::new("find").args([&downloads, "-maxdepth", "2", "(",
            "-iname", "*.dmg", "-o", "-iname", "*.pkg", "-o", "-iname", "*.zip", "-o",
            "-iname", "*.tar.gz", "-o", "-iname", "*.tgz", "-o", "-iname", "*.iso", ")",
            "-type", "f", "-mtime", "+30"]).output().ok() {
            let files: Vec<_> = String::from_utf8_lossy(&o.stdout).lines().filter(|l| !l.is_empty()).map(|s| s.to_string()).collect();
            let n = files.len();
            if n > 0 {
                let total: u64 = files.iter().map(|f| du_sh(f)).sum();
                let _ = tx.send(ScanEvent::Entry(entry(next_id, "Downloads", &format!("Stale installers (>30d, {n} files)"), &downloads, total)));
                next_id += 1;
            }
        }
    }

    scan_orphans(&tx, &mut next_id);
    let _ = tx.send(ScanEvent::Done);
}

fn parse_size_str(s: &str) -> u64 {
    let s = s.trim().to_lowercase();
    let (n, suffix) = if s.ends_with("tb") { (&s[..s.len()-2], "tb") }
    else if s.ends_with('t') { (&s[..s.len()-1], "tb") }
    else if s.ends_with("gb") { (&s[..s.len()-2], "gb") }
    else if s.ends_with('g') { (&s[..s.len()-1], "gb") }
    else if s.ends_with("mb") { (&s[..s.len()-2], "mb") }
    else if s.ends_with('m') { (&s[..s.len()-1], "mb") }
    else if s.ends_with("kb") { (&s[..s.len()-2], "kb") }
    else if s.ends_with('k') { (&s[..s.len()-1], "kb") }
    else if s.ends_with('b') { (&s[..s.len()-1], "b") }
    else { return s.parse().unwrap_or(0); };
    let v: f64 = n.trim().parse().unwrap_or(0.0);
    match suffix { "tb" => (v * 1099511627776.0) as u64, "gb" => (v * 1073741824.0) as u64, "mb" => (v * 1048576.0) as u64, "kb" => (v * 1024.0) as u64, _ => v as u64 }
}

struct CleanupApp {
    entries: Vec<CleanupEntry>,
    scanning: bool,
    scan_progress: String,
    done: bool,
    receiver: Option<mpsc::Receiver<ScanEvent>>,
    message: Option<String>,
    message_timer: f32,
    show_confirm: bool,
    pending_cleanup: Vec<CleanupEntry>,
}

impl Default for CleanupApp {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || run_scan(tx));
        Self { entries: vec![], scanning: true, scan_progress: "Starting scan...".into(), done: false, receiver: Some(rx), message: None, message_timer: 0.0, show_confirm: false, pending_cleanup: vec![] }
    }
}

impl eframe::App for CleanupApp {
    fn ui(&mut self, _: &mut egui::Ui, _: &mut eframe::Frame) {}
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(rx) = &self.receiver {
            while let Ok(event) = rx.try_recv() {
                match event {
                    ScanEvent::Progress(msg) => self.scan_progress = msg,
                    ScanEvent::Entry(entry) => { self.entries.push(entry); }
                    ScanEvent::Done => { self.scanning = false; self.done = true; self.scan_progress = "Scan complete.".into(); }
                }
                ctx.request_repaint();
            }
        }
        if self.message.is_some() {
            self.message_timer -= ctx.input(|i| i.unstable_dt);
            if self.message_timer <= 0.0 { self.message = None; }
            ctx.request_repaint();
        }

        // --- Confirmation dialog ---
        if self.show_confirm {
            let total_sel: u64 = self.pending_cleanup.iter().map(|e| e.size_bytes).sum();
            egui::Window::new("⚠️  Confirm Deletion")
                .collapsible(false)
                .resizable(true)
                .default_size([600.0, 400.0])
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.heading("WARNING: This action cannot be undone!");
                    ui.label("The following items will be permanently deleted:");
                    ui.label("");
                    ui.monospace(format!("Total: {} items — {}", self.pending_cleanup.len(), format_size(total_sel)));
                    ui.separator();

                    egui::ScrollArea::vertical()
                        .max_height(ui.available_height() - 60.0)
                        .show(ui, |ui| {
                            for item in &self.pending_cleanup {
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
                            self.show_confirm = false;
                            self.pending_cleanup.clear();
                        }
                        if ui.add_sized([140.0, 30.0], egui::Button::new("Confirm Deletion 🗑️").fill(egui::Color32::RED)).clicked() {
                            let n = self.pending_cleanup.len();
                            execute_cleanup(self.pending_cleanup.clone());
                            let paths: Vec<String> = self.pending_cleanup.iter().map(|e| e.path.clone()).collect();
                            self.entries.retain(|e| !paths.contains(&e.path));
                            self.show_confirm = false;
                            self.pending_cleanup.clear();
                            self.message = Some(format!("Deleted {n} items!"));
                            self.message_timer = 3.0;
                        }
                    });
                });
        }

        egui::Panel::top("header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("🧹 macOS Cleanup Tool");
                if self.scanning { ui.label("⏳"); ui.label(&self.scan_progress); ui.spinner(); }
                else if self.done { ui.label(format!("✓ Done — {} items", self.entries.len())); }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.entries.is_empty() && self.scanning {
                ui.vertical_centered(|ui| { ui.add_space(40.0); ui.heading("Scanning..."); ui.spinner(); ui.label(&self.scan_progress); });
                return;
            }
            if self.entries.is_empty() && !self.scanning {
                ui.vertical_centered(|ui| { ui.label("Nothing to clean!"); });
                return;
            }

            let mut sections: Vec<String> = self.entries.iter().map(|e| e.section.clone()).collect();
            sections.sort(); sections.dedup();
            let selected_bytes: u64 = self.entries.iter().filter(|e| e.selected).map(|e| e.size_bytes).sum();
            let selected_count = self.entries.iter().filter(|e| e.selected).count();
            let total_bytes: u64 = self.entries.iter().map(|e| e.size_bytes).sum();

            egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                for section in &sections {
                    let indices: Vec<usize> = self.entries.iter().enumerate().filter(|(_, e)| e.section == *section).map(|(i, _)| i).collect();
                    let all_sel = indices.iter().all(|&i| self.entries[i].selected);
                    let sec_total: u64 = indices.iter().map(|&i| self.entries[i].size_bytes).sum();
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(format!("📁 {section}"));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.strong(format_size(sec_total));
                            let mut a = all_sel;
                            if ui.checkbox(&mut a, "all").on_hover_text("Toggle all").clicked() {
                                for &i in &indices { self.entries[i].selected = a; }
                            }
                        });
                    });
                    ui.separator();
                    for &i in &indices {
                        let e = &mut self.entries[i];
                        ui.horizontal(|ui| { ui.checkbox(&mut e.selected, ""); ui.label(format_size(e.size_bytes)); ui.label(&e.label); });
                    }
                }
            });

            egui::Panel::bottom("footer").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if selected_count > 0 { ui.label(format!("Selected: {selected_count} items, {}", format_size(selected_bytes))); }
                    else { ui.label(format!("Total found: {}", format_size(total_bytes))); }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add_enabled(selected_count > 0, egui::Button::new("🧹 Clean Selected")).clicked() {
                            self.pending_cleanup = self.entries.iter().filter(|e| e.selected).cloned().collect();
                            self.show_confirm = true;
                        }
                        if let Some(ref m) = self.message { ui.label("✅"); ui.label(m); }
                    });
                });
            });
        });
    }
}

fn execute_cleanup(items: Vec<CleanupEntry>) {
    thread::spawn(move || {
        for item in &items {
            if item.path.is_empty() && item.label.contains("Docker") {
                let _ = Command::new("docker").args(["system", "prune", "-af"]).output();
            } else if !item.path.is_empty() {
                let _ = Command::new("rm").args(["-rf", &item.path]).output();
            }
        }
    });
}

fn main() -> eframe::Result<()> {
    eframe::run_native("macOS Cleanup Tool",
        eframe::NativeOptions { viewport: egui::ViewportBuilder::default().with_inner_size([800.0, 600.0]), ..Default::default() },
        Box::new(|_cc| Ok(Box::<CleanupApp>::default())))
}
