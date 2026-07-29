# Mac Cleaner Redesign — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor monolithic `main.rs` (1853 lines) into modular panels/widgets/scanning/util architecture.

**Architecture:** 20+ focused files organized into `panels/`, `widgets/`, `scanning/`, `util/` modules with a central `app.rs` managing state and event loop.

**Tech Stack:** Rust, egui 0.34, eframe 0.34, serde, chrono, libc

---

### Task 1: Foundation — models, theme, util

**Files:**
- Create: `src/models.rs`
- Create: `src/theme.rs`
- Create: `src/util/mod.rs`
- Create: `src/util/format.rs`

- [ ] **Step 1: Create `src/models.rs`** — Extract all data types from `main.rs:57-137` and `main.rs:1005-1010`. Types: `DiskSpace`, `OrphanConfidence`, `CleanupEntry`, `InstalledApp`, `ScanEvent`, `AppScanEvent`, `DeletionResult`, `AppView`.

```rust
use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq)]
pub struct DiskSpace {
    pub total_bytes: u64,
    pub available_bytes: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum OrphanConfidence {
    High, Medium, Low,
}

#[derive(Clone, Debug)]
pub struct CleanupEntry {
    pub id: usize,
    pub section: String,
    pub label: String,
    pub path: String,
    pub size_bytes: u64,
    pub selected: bool,
    pub orphan_confidence: Option<OrphanConfidence>,
}

pub fn entry(id: usize, section: &str, label: &str, path: &str, size_bytes: u64) -> CleanupEntry {
    CleanupEntry { id, section: section.into(), label: label.into(), path: path.into(), size_bytes, selected: false, orphan_confidence: None }
}

pub fn orphan_entry(id: usize, section: &str, label: &str, path: &str, size_bytes: u64, confidence: OrphanConfidence) -> CleanupEntry {
    CleanupEntry { id, section: section.into(), label: label.into(), path: path.into(), size_bytes, selected: false, orphan_confidence: Some(confidence) }
}

#[derive(Clone, Debug)]
pub struct InstalledApp {
    pub id: usize,
    pub name: String,
    pub bundle_id: String,
    pub app_path: String,
    pub size_bytes: u64,
}

pub enum ScanEvent {
    Progress(String),
    Entry(CleanupEntry),
    Warning(String),
    Done,
}

pub enum AppScanEvent {
    Progress(String),
    Entry(InstalledApp),
    Done,
}

pub enum DeletionResult {
    Deleted(String, usize),
    Error(String),
    DryRunPreview(String),
    Done,
}

#[derive(Clone, Copy, PartialEq)]
pub enum AppView {
    Dashboard,
    Cleanup,
    Applications,
    LargeFiles,
}

pub const APP_TRACE_ID_OFFSET: usize = 1_000_000;
```

- [ ] **Step 2: Run `cargo check` to verify models compile**

Run: `cargo check 2>&1`
Expected: compilation succeeds (warnings ok)

- [ ] **Step 3: Create `src/theme.rs`**

```rust
use egui::Color32;

pub struct Theme {
    pub sidebar_bg: Color32,
    pub content_bg: Color32,
    pub accent: Color32,
    pub success: Color32,
    pub error: Color32,
    pub warning: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_muted: Color32,
    pub card_bg: Color32,
    pub border: Color32,
}

pub const THEME: Theme = Theme {
    sidebar_bg: Color32::from_rgb(22, 22, 42),
    content_bg: Color32::from_rgb(16, 16, 32),
    accent: Color32::from_rgb(108, 92, 231),
    success: Color32::from_rgb(60, 180, 90),
    error: Color32::from_rgb(210, 60, 60),
    warning: Color32::from_rgb(220, 150, 30),
    text_primary: Color32::from_rgb(230, 230, 240),
    text_secondary: Color32::from_rgb(180, 180, 200),
    text_muted: Color32::from_rgb(120, 120, 140),
    card_bg: Color32::from_rgb(26, 26, 46),
    border: Color32::from_rgb(40, 40, 60),
};
```

- [ ] **Step 4: Create `src/util/format.rs`** — Extract utility functions from `main.rs:143-255`: `format_size`, `section_icon`, `run_cmd_timeout`, `du_sh`, `find_dirs`, `parse_size_str`. Move `get_disk_space` here too.

```rust
use std::process::Command;
use std::sync::mpsc;
use std::thread;

pub fn format_size(bytes: u64) -> String {
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    const KB: f64 = 1024.0;
    let b = bytes as f64;
    if b >= GB { format!("{:.1} GB", b / GB) }
    else if b >= MB { format!("{:.1} MB", b / MB) }
    else if b >= KB { format!("{:.1} KB", b / KB) }
    else { format!("{bytes} B") }
}

pub fn section_icon(section: &str) -> &'static str {
    match section {
        "System Caches" => "🧹", "Build Tools" => "🔧", "Go" => "🐹",
        "Python" => "🐍", "Package Managers" => "📦", "Project Deps" => "📁",
        "Logs & Temp" => "🪵", "iOS" => "🍎", "Downloads" => "⬇️",
        "Large Files" => "🐘", "Orphan App Data" => "🧩",
        "App Bundle" => "📱", "App Traces" => "🧬", _ => "📁",
    }
}

pub fn run_cmd_timeout(program: &str, args: &[&str], secs: u64) -> Option<std::process::Output> {
    let program = program.to_string();
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || { let out = Command::new(&program).args(&args).output().ok(); let _ = tx.send(out); });
    match rx.recv_timeout(std::time::Duration::from_secs(secs)) {
        Ok(result) => result, Err(_) => None,
    }
}

pub fn du_sh(path: &str) -> u64 {
    Command::new("/usr/bin/du").args(["-sk", path]).output().ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout);
            s.split_whitespace().next()?.parse::<u64>().ok().map(|kb| kb * 1024)
        }).unwrap_or(0)
}

pub fn find_dirs(path: &str, name: &str, maxdepth: u32) -> Vec<String> {
    let depth = format!("{maxdepth}");
    Command::new("find").args([path, "-maxdepth", &depth, "-type", "d", "-name", name]).output().ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().filter(|l| !l.is_empty()).map(|s| s.to_string()).collect())
        .unwrap_or_default()
}

pub fn parse_size_str(s: &str) -> u64 {
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
    else { return s.parse().unwrap_or(0) };
    let v: f64 = n.trim().parse().unwrap_or(0.0);
    match suffix {
        "tb" => (v * 1_099_511_627_776.0) as u64,
        "gb" => (v * 1_073_741_824.0) as u64,
        "mb" => (v * 1_048_576.0) as u64,
        "kb" => (v * 1024.0) as u64, _ => v as u64,
    }
}

use crate::models::DiskSpace;

pub fn get_disk_space(path: &str) -> DiskSpace {
    let cpath = std::ffi::CString::new(path).unwrap();
    let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
    let ret = unsafe { libc::statfs(cpath.as_ptr(), &mut stat) };
    if ret == 0 {
        DiskSpace {
            total_bytes: stat.f_blocks as u64 * stat.f_bsize as u64,
            available_bytes: stat.f_bavail as u64 * stat.f_bsize as u64,
        }
    } else {
        DiskSpace { total_bytes: 0, available_bytes: 0 }
    }
}
```

- [ ] **Step 5: Create `src/util/mod.rs`**

```rust
pub mod format;
pub use format::*;
```

- [ ] **Step 6: Run `cargo check`**

Run: `cargo check 2>&1`
Expected: compiles

- [ ] **Step 7: Commit**

```bash
git add src/models.rs src/theme.rs src/util/mod.rs src/util/format.rs
git commit -m "feat: extract models, theme, and util modules"
```

---

### Task 2: Scanning modules

**Files:**
- Create: `src/scanning/mod.rs`
- Create: `src/scanning/scanner.rs`
- Create: `src/scanning/orphan.rs`
- Create: `src/scanning/deletion.rs`

- [ ] **Step 1: Create `src/scanning/scanner.rs`** — Extract `run_scan` from `main.rs:400-759`. Same macros, same logic, just in a separate file.

```rust
use crate::models::*;
use crate::util::*;
use std::sync::mpsc;

pub fn run_scan(tx: mpsc::Sender<ScanEvent>) {
    let mut next_id = 0usize;
    let home = std::env::var("HOME").unwrap_or_default();

    macro_rules! send_dir {
        ($section:expr, $label:expr, $path:expr) => {{
            let id = next_id; next_id += 1;
            let _ = tx.send(ScanEvent::Progress(format!("Scanning {}...", $label)));
            let sz = du_sh($path);
            let _ = tx.send(ScanEvent::Entry(entry(id, $section, $label, $path, sz)));
        }};
    }
    macro_rules! send_find {
        ($section:expr, $label:expr, $base:expr, $name:expr) => {{
            let _ = tx.send(ScanEvent::Progress(format!("Scanning {}...", $label)));
            let dirs = find_dirs($base, $name, 5);
            for d in &dirs {
                let sz = du_sh(d);
                let short = d.strip_prefix(&format!("{}/", $base)).unwrap_or(d);
                let _ = tx.send(ScanEvent::Entry(entry(next_id, $section, short, d, sz)));
                next_id += 1;
            }
        }};
    }

    // System caches
    send_dir!("System Caches", "~/Library/Caches", &format!("{home}/Library/Caches"));
    send_dir!("System Caches", "~/.cache", &format!("{home}/.cache"));

    // Build tools
    send_dir!("Build Tools", "Gradle caches", &format!("{home}/.gradle/caches"));
    send_dir!("Build Tools", "Gradle wrappers", &format!("{home}/.gradle/wrapper"));
    send_dir!("Build Tools", "Cargo registry", &format!("{home}/.cargo/registry"));
    send_dir!("Build Tools", "Xcode DerivedData", &format!("{home}/Library/Developer/Xcode/DerivedData"));
    send_dir!("Build Tools", "Xcode Archives", &format!("{home}/Library/Developer/Xcode/Archives"));
    send_dir!("Build Tools", "iOS Device Logs", &format!("{home}/Library/Developer/Xcode/iOS Device Logs"));
    send_dir!("Build Tools", "Swift Package cache", &format!("{home}/.swiftpm"));

    // iOS Simulators
    {
        let _ = tx.send(ScanEvent::Progress("Scanning iOS Simulators...".into()));
        let sim_path = format!("{home}/Library/Developer/CoreSimulator/Devices");
        let sz = du_sh(&sim_path);
        let _ = tx.send(ScanEvent::Entry(entry(next_id, "Build Tools", "CoreSimulator Devices", &sim_path, sz)));
        next_id += 1;
    }

    // Go
    send_dir!("Go", "Go module cache", &format!("{home}/go/pkg/mod"));
    send_dir!("Go", "Go compiled binaries", &format!("{home}/go/bin"));

    // Python
    send_dir!("Python", "pip cache", &format!("{home}/.cache/pip"));
    send_dir!("Python", "pip cache (macOS)", &format!("{home}/Library/Caches/pip"));

    // CocoaPods
    send_dir!("Package Managers", "CocoaPods repo cache", &format!("{home}/.cocoapods/repos"));

    // Project deps
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

    // npm cache
    {
        let _ = tx.send(ScanEvent::Progress("Scanning npm cache...".into()));
        let p = format!("{home}/.npm/_cacache");
        let sz = du_sh(&p);
        let _ = tx.send(ScanEvent::Entry(entry(next_id, "Package Managers", "npm cache", &p, sz)));
        next_id += 1;
    }

    // Homebrew cache
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

    // Docker
    {
        let _ = tx.send(ScanEvent::Progress("Checking Docker...".into()));
        let docker_alive = run_cmd_timeout("docker", &["info", "--format", "{{.ServerVersion}}"], 3)
            .map(|o| o.status.success()).unwrap_or(false);
        let sz = if docker_alive {
            match run_cmd_timeout("docker", &["system", "df", "--format", "json"], 5) {
                Some(out) if out.status.success() => {
                    let raw = String::from_utf8_lossy(&out.stdout);
                    let mut total = 0u64;
                    if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(&raw) {
                        for obj in arr {
                            if let Some(s) = obj.get("ReclaimableSize").and_then(|v| v.as_str()) {
                                total += parse_size_str(s);
                            }
                        }
                    }
                    total
                }
                _ => 0,
            }
        } else { 0 };
        let _ = tx.send(ScanEvent::Entry(entry(next_id, "Package Managers", "Docker (reclaimable)", "", sz)));
        next_id += 1;
    }

    // Logs & Temp
    send_dir!("Logs & Temp", "~/Library/Logs", &format!("{home}/Library/Logs"));
    send_dir!("Logs & Temp", "/private/tmp", "/private/tmp");

    // Trash
    {
        let trash = format!("{home}/.Trash");
        let sz = du_sh(&trash);
        if sz > 0 {
            let _ = tx.send(ScanEvent::Entry(entry(next_id, "Logs & Temp", "Trash contents (~/.Trash)", &trash, sz)));
            next_id += 1;
        }
    }

    // iOS backups
    {
        let backup = format!("{home}/Library/Application Support/MobileSync/Backup");
        let sz = du_sh(&backup);
        if sz > 0 {
            let _ = tx.send(ScanEvent::Entry(entry(next_id, "iOS", "iOS Backups (MobileSync)", &backup, sz)));
            next_id += 1;
        }
    }

    // Stale Downloads
    {
        let _ = tx.send(ScanEvent::Progress("Scanning Downloads...".into()));
        let dl = format!("{home}/Downloads");
        if let Some(o) = find_stale_downloads(&dl) {
            let n = o.len();
            let total: u64 = o.iter().map(|f| du_sh(f)).sum();
            let _ = tx.send(ScanEvent::Entry(entry(next_id, "Downloads", &format!("Stale installers (>30d, {n} files)"), &dl, total)));
            next_id += 1;
        }
    }

    // Large files
    {
        let _ = tx.send(ScanEvent::Progress("Scanning for large files (>500 MB)...".into()));
        for f in find_large_files(home) {
            let sz = du_sh(&f);
            let label = f.strip_prefix(&format!("{home}/")).unwrap_or(&f).to_string();
            let _ = tx.send(ScanEvent::Entry(entry(next_id, "Large Files", &label, &f, sz)));
            next_id += 1;
        }
    }

    // Orphans
    crate::scanning::orphan::scan_orphans(&tx, &mut next_id);

    let _ = tx.send(ScanEvent::Done);
}
```

- [ ] **Step 2: Add `find_stale_downloads` and `find_large_files` to `src/util/format.rs`**

```rust
use std::process::Command;

pub fn find_stale_downloads(downloads: &str) -> Option<Vec<String>> {
    Command::new("find")
        .args([downloads, "-maxdepth", "2", "(", "-iname", "*.dmg", "-o", "-iname", "*.pkg",
               "-o", "-iname", "*.zip", "-o", "-iname", "*.tar.gz", "-o", "-iname", "*.tgz",
               "-o", "-iname", "*.iso", ")", "-type", "f", "-mtime", "+30"])
        .output().ok().map(|o| {
            String::from_utf8_lossy(&o.stdout).lines().filter(|l| !l.is_empty()).map(|s| s.to_string()).collect()
        })
}

pub fn find_large_files(home: String) -> Vec<String> {
    Command::new("find")
        .args([&home, "-maxdepth", "5", "-type", "f", "-size", "+500M",
               "-not", "-path", "*/\\.Trash/*",
               "-not", "-path", "*/Library/Developer/CoreSimulator/*"])
        .output().ok().map(|o| {
            String::from_utf8_lossy(&o.stdout).lines().filter(|l| !l.is_empty()).map(|s| s.to_string()).collect()
        }).unwrap_or_default()
}
```

- [ ] **Step 3: Create `src/scanning/orphan.rs`** — Extract `scan_orphans` from `main.rs:270-398`.

```rust
use crate::models::*;
use crate::util::du_sh;
use std::collections::HashSet;
use std::process::Command;
use std::sync::mpsc;

pub fn scan_orphans(tx: &mpsc::Sender<ScanEvent>, next_id: &mut usize) {
    let _ = tx.send(ScanEvent::Progress("Detecting orphan app data...".into()));
    let home = std::env::var("HOME").unwrap_or_default();

    let mut known_bundle_ids: HashSet<String> = HashSet::new();
    let mut known_app_names: HashSet<String> = HashSet::new();

    let app_dirs = [
        "/Applications".to_string(),
        format!("{home}/Applications"),
        "/System/Applications".to_string(),
    ];

    for appdir in &app_dirs {
        let Ok(output) = Command::new("find")
            .args([appdir.as_str(), "-maxdepth", "2", "-name", "*.app", "-type", "d"])
            .output()
        else { continue; };
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let line = line.trim();
            if line.is_empty() { continue; }
            let plist = format!("{line}/Contents/Info.plist");
            if !std::path::Path::new(&plist).exists() { continue; }
            if let Ok(b) = Command::new("/usr/libexec/PlistBuddy")
                .args(["-c", "Print :CFBundleIdentifier", &plist]).output()
            {
                let s = String::from_utf8_lossy(&b.stdout).trim().to_lowercase();
                if !s.is_empty() { known_bundle_ids.insert(s); }
            }
            if let Some(name) = line.trim_end_matches(".app").split('/').last() {
                let n = name.to_lowercase();
                if !n.is_empty() { known_app_names.insert(n); }
            }
        }
    }

    const ORPHAN_SCAN_DIRS: &[&str] = &[
        "Library/Application Support", "Library/Preferences",
        "Library/Saved Application State", "Library/Caches",
        "Library/Containers", "Library/Group Containers",
        "Library/WebKit", "Library/Application Scripts", "Library/HTTPStorages",
    ];

    for subdir in ORPHAN_SCAN_DIRS {
        let scan_path = format!("{home}/{subdir}");
        let Ok(dir) = std::fs::read_dir(&scan_path) else { continue; };
        for dir_entry_res in dir {
            let Ok(dir_entry) = dir_entry_res else { continue; };
            let path = dir_entry.path();
            let name = match path.file_name() { Some(n) => n.to_string_lossy().to_string(), None => continue };
            let nl = name.to_lowercase();

            if nl.starts_with("com.apple.") || nl == "com.apple" || nl.starts_with("apple.")
                || nl.starts_with('.') || nl == "caches" || nl == "metadata" { continue; }

            let is_known = known_bundle_ids.contains(&nl)
                || known_bundle_ids.iter().any(|kid| nl.starts_with(&format!("{kid}.")))
                || known_app_names.contains(&nl);
            if is_known { continue; }

            let sz = du_sh(&path.to_string_lossy());
            if sz == 0 { continue; }

            let confidence = if nl.contains('.') && nl.split('.').count() >= 3 { OrphanConfidence::High }
            else if !nl.contains(' ') && nl.len() > 3 { OrphanConfidence::Medium }
            else { OrphanConfidence::Low };

            let confidence_label = match &confidence {
                OrphanConfidence::High => "[HIGH] ", OrphanConfidence::Medium => "[MED]  ", OrphanConfidence::Low => "[LOW]  ",
            };
            let label = format!("{confidence_label}{name}");
            let _ = tx.send(ScanEvent::Entry(orphan_entry(*next_id, "Orphan App Data", &label, &path.to_string_lossy(), sz, confidence)));
            *next_id += 1;
        }
    }
}
```

- [ ] **Step 4: Create `src/scanning/deletion.rs`** — Extract deletion logic from `main.rs:926-987`, including `execute_cleanup`, `try_trash`, `try_rm_rf`. Also include `scan_installed_apps` and `find_app_traces` from `main.rs:774-916`.

```rust
use crate::models::*;
use crate::util::*;
use std::process::Command;
use std::sync::mpsc;
use std::thread;

pub fn execute_cleanup(items: Vec<CleanupEntry>, dry_run: bool, result_tx: mpsc::Sender<DeletionResult>) {
    thread::spawn(move || {
        for item in &items {
            if dry_run {
                let msg = if item.path.is_empty() { format!("[DRY RUN] Would execute: docker system prune -af") }
                else { format!("[DRY RUN] Would delete: {}", item.path) };
                let _ = result_tx.send(DeletionResult::DryRunPreview(msg));
                continue;
            }
            if item.path.is_empty() && item.label.contains("Docker") {
                match Command::new("docker").args(["system", "prune", "-af"]).output() {
                    Ok(o) if o.status.success() => { let _ = result_tx.send(DeletionResult::Deleted(item.path.clone(), item.id)); }
                    Ok(o) => { let _ = result_tx.send(DeletionResult::Error(format!("docker prune failed: {}", String::from_utf8_lossy(&o.stderr).trim()))); }
                    Err(e) => { let _ = result_tx.send(DeletionResult::Error(format!("docker prune error: {e}"))); }
                }
                continue;
            }
            if item.path.is_empty() { continue; }
            let deleted = try_trash(&item.path).or_else(|| try_rm_rf(&item.path));
            match deleted {
                Some(true) => { let _ = result_tx.send(DeletionResult::Deleted(item.path.clone(), item.id)); }
                _ => { let _ = result_tx.send(DeletionResult::Error(format!("Failed to delete: {}", item.path))); }
            }
        }
        let _ = result_tx.send(DeletionResult::Done);
    });
}

fn try_trash(path: &str) -> Option<bool> {
    Command::new("trash").arg(path).output().ok().map(|o| o.status.success())
}

fn try_rm_rf(path: &str) -> Option<bool> {
    Command::new("rm").args(["-rf", path]).output().ok().map(|o| o.status.success())
}

pub fn scan_installed_apps(tx: &mpsc::Sender<AppScanEvent>) {
    let _ = tx.send(AppScanEvent::Progress("Scanning installed apps...".into()));
    let home = std::env::var("HOME").unwrap_or_default();
    let app_dirs = ["/Applications".to_string(), format!("{home}/Applications"), "/System/Applications".to_string()];
    let mut next_id = 0usize;
    for appdir in &app_dirs {
        let Ok(output) = Command::new("find")
            .args([appdir.as_str(), "-maxdepth", "2", "-name", "*.app", "-type", "d"]).output()
        else { continue; };
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let line = line.trim();
            if line.is_empty() { continue; }
            let plist = format!("{line}/Contents/Info.plist");
            if !std::path::Path::new(&plist).exists() { continue; }
            let bundle_id = Command::new("/usr/libexec/PlistBuddy")
                .args(["-c", "Print :CFBundleIdentifier", &plist]).output().ok()
                .map(|b| String::from_utf8_lossy(&b.stdout).trim().to_string()).unwrap_or_default();
            let name = line.trim_end_matches(".app").split('/').last().unwrap_or(line).to_string();
            let sz = du_sh(line);
            let _ = tx.send(AppScanEvent::Entry(InstalledApp { id: next_id, name, bundle_id, app_path: line.to_string(), size_bytes: sz }));
            next_id += 1;
        }
    }
    let _ = tx.send(AppScanEvent::Done);
}

const APP_TRACE_DIRS: &[&str] = &[
    "Library/Application Support", "Library/Preferences", "Library/Saved Application State",
    "Library/Caches", "Library/Containers", "Library/Group Containers", "Library/WebKit",
    "Library/Application Scripts", "Library/HTTPStorages", "Library/LaunchAgents", "Library/Logs",
];

pub fn find_app_traces(app: &InstalledApp) -> Vec<CleanupEntry> {
    let mut out = Vec::new();
    let mut local_id = 0usize;
    out.push(entry(APP_TRACE_ID_OFFSET + local_id, "App Bundle", &format!("{}.app", app.name), &app.app_path, app.size_bytes));
    local_id += 1;
    let home = std::env::var("HOME").unwrap_or_default();
    let bundle_lc = app.bundle_id.to_lowercase();
    let name_lc = app.name.to_lowercase();
    let name_nospace = name_lc.replace(' ', "");

    for subdir in APP_TRACE_DIRS {
        let scan_path = format!("{home}/{subdir}");
        let Ok(dir) = std::fs::read_dir(&scan_path) else { continue; };
        for dir_entry_res in dir {
            let Ok(dir_entry) = dir_entry_res else { continue; };
            let path = dir_entry.path();
            let Some(fname) = path.file_name().map(|f| f.to_string_lossy().to_string()) else { continue; };
            let fl = fname.to_lowercase();
            let fl_stem = fl.trim_end_matches(".plist");
            let is_match = (!bundle_lc.is_empty() && (fl == bundle_lc || fl_stem == bundle_lc || fl.starts_with(&format!("{bundle_lc}."))))
                || fl == name_lc || fl_stem == name_lc || fl == name_nospace || fl_stem == name_nospace;
            if !is_match { continue; }
            let sz = du_sh(&path.to_string_lossy());
            if sz == 0 { continue; }
            out.push(entry(APP_TRACE_ID_OFFSET + local_id, "App Traces", &format!("{subdir}/{fname}"), &path.to_string_lossy(), sz));
            local_id += 1;
        }
    }
    out
}
```

- [ ] **Step 5: Create `src/scanning/mod.rs`**

```rust
pub mod scanner;
pub mod orphan;
pub mod deletion;
pub use scanner::run_scan;
pub use orphan::scan_orphans;
pub use deletion::{execute_cleanup, scan_installed_apps, find_app_traces};
```

- [ ] **Step 6: Run `cargo check`**

- [ ] **Step 7: Commit**

---

### Task 3: Widgets — reusable UI components

**Files:**
- Create: `src/widgets/mod.rs`
- Create: `src/widgets/storage_card.rs`
- Create: `src/widgets/cleanup_card.rs`
- Create: `src/widgets/toast.rs`
- Create: `src/widgets/search_bar.rs`
- Create: `src/widgets/danger_dialog.rs`

- [ ] **Step 1: Create `src/widgets/storage_card.rs`** — A reusable metric card showing a label, value, and optional accent color.

```rust
use egui::*;
use crate::theme::THEME;

pub struct StorageCard {
    pub label: String,
    pub value: String,
    pub accent: Color32,
}

impl StorageCard {
    pub fn new(label: impl Into<String>, value: impl Into<String>, accent: Color32) -> Self {
        Self { label: label.into(), value: value.into(), accent }
    }

    pub fn show(&self, ui: &mut Ui) {
        Frame::none()
            .fill(THEME.card_bg)
            .corner_radius(8)
            .show(ui, |ui| {
                ui.add_space(12);
                ui.horizontal(|ui| {
                    ui.add_space(12);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&self.label).size(11.0).color(THEME.text_muted));
                        ui.add_space(2);
                        ui.label(RichText::new(&self.value).size(20.0).color(self.accent).strong());
                    });
                });
                ui.add_space(12);
            });
    }
}
```

- [ ] **Step 2: Create `src/widgets/toast.rs`** — Auto-dismissing notification.

```rust
use egui::*;
use crate::theme::THEME;

pub struct ToastMessage {
    pub text: String,
    pub is_error: bool,
    pub timer: f32,
}

pub fn toast_ui(ui: &mut Ui, toast: &mut ToastMessage, dt: f32) {
    toast.timer -= dt;
    let color = if toast.is_error { THEME.error } else { THEME.success };
    let icon = if toast.is_error { "⚠️" } else { "✅" };
    ui.colored_label(color, icon);
    ui.label(&toast.text);
}
```

- [ ] **Step 3: Create `src/widgets/search_bar.rs`** — Search input with clear button and optional min-size slider.

```rust
use egui::*;

pub struct SearchBar {
    pub search: String,
    pub min_size_mb: f32,
}

impl SearchBar {
    pub fn show(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label("🔎");
            ui.add(TextEdit::singleline(&mut self.search)
                .hint_text("Filter by name, path, or category...")
                .desired_width(280.0));
            if !self.search.is_empty() && ui.button("✕").clicked() {
                self.search.clear();
            }
            ui.separator();
            ui.label("Hide items smaller than:");
            ui.add(Slider::new(&mut self.min_size_mb, 0.0..=500.0).suffix(" MB").fixed_decimals(0));
            if self.min_size_mb > 0.0 && ui.button("Reset").clicked() {
                self.min_size_mb = 0.0;
            }
        });
    }
}
```

- [ ] **Step 4: Create `src/widgets/danger_dialog.rs`** — Confirmation dialog (extracted from `main.rs:1284-1365`).

```rust
use egui::*;
use crate::models::*;
use crate::util::*;
use crate::theme::THEME;
use std::sync::mpsc;
use crate::scanning::execute_cleanup;

pub struct DangerDialog {
    pub show: bool,
    pub items: Vec<CleanupEntry>,
    pub dry_run: bool,
    pub deletion_rx: Option<mpsc::Receiver<DeletionResult>>,
    pub deleting: bool,
}

impl DangerDialog {
    pub fn new() -> Self {
        Self { show: false, items: vec![], dry_run: false, deletion_rx: None, deleting: false }
    }

    pub fn open(&mut self, items: Vec<CleanupEntry>, dry_run: bool) {
        self.show = true;
        self.items = items;
        self.dry_run = dry_run;
    }

    pub fn show(&mut self, ctx: &Context, disk_available: u64) -> Option<mpsc::Receiver<DeletionResult>> {
        let mut result: Option<mpsc::Receiver<DeletionResult>> = None;
        if !self.show { return None; }

        let total_sel: u64 = self.items.iter().map(|e| e.size_bytes).sum();
        egui::Window::new("⚠️  Confirm Deletion")
            .collapsible(false).resizable(true).default_size([640.0, 460.0])
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
                ui.monospace(format!("Total: {} items — {}", self.items.len(), format_size(total_sel)));

                if !self.dry_run {
                    let projected = disk_available.saturating_add(total_sel);
                    ui.horizontal(|ui| {
                        ui.label("Available space after cleanup:");
                        ui.strong(format_size(projected));
                        ui.label(format!("(+{})", format_size(total_sel)));
                    });
                }
                ui.separator();

                ScrollArea::vertical().max_height(ui.available_height() - 80.0).show(ui, |ui| {
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
                    let btn_label = if self.dry_run { "🔍 Preview (Dry Run)" } else { "Confirm Deletion 🗑️" };
                    let btn_color = if self.dry_run { Color32::from_rgb(50, 100, 180) } else { Color32::RED };
                    if ui.add_sized([160.0, 30.0], Button::new(btn_label).fill(btn_color)).clicked() {
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
```

- [ ] **Step 5: Create `src/widgets/mod.rs`**

```rust
pub mod storage_card;
pub mod toast;
pub mod search_bar;
pub mod danger_dialog;
pub use storage_card::StorageCard;
pub use toast::ToastMessage;
pub use search_bar::SearchBar;
pub use danger_dialog::DangerDialog;
```

- [ ] **Step 6: Run `cargo check`**

- [ ] **Step 7: Commit**

---

### Task 4: Panels — sidebar, header, dashboard

**Files:**
- Create: `src/panels/mod.rs`
- Create: `src/panels/sidebar.rs`
- Create: `src/panels/header.rs`
- Create: `src/panels/dashboard.rs`

- [ ] **Step 1: Create `src/panels/sidebar.rs`** — Navigation sidebar with view selection.

```rust
use egui::*;
use crate::models::AppView;
use crate::theme::THEME;

pub struct Sidebar {
    pub selected_view: AppView,
}

impl Sidebar {
    pub fn new() -> Self { Self { selected_view: AppView::Dashboard } }

    pub fn show(&mut self, ui: &mut Ui) {
        Frame::none().fill(THEME.sidebar_bg).show(ui, |ui| {
            ui.add_space(12);
            let items = [
                (AppView::Dashboard, "📊", "Dashboard"),
                (AppView::Cleanup, "🧹", "Cleanup"),
                (AppView::Applications, "📱", "Applications"),
                (AppView::LargeFiles, "🐘", "Large Files"),
            ];
            for (view, icon, label) in &items {
                let selected = self.selected_view == *view;
                let bg = if selected { THEME.accent } else { Color32::TRANSPARENT };
                let response = Frame::none().fill(bg).corner_radius(6).show(ui, |ui| {
                    ui.add_space(4);
                    ui.horizontal(|ui| {
                        ui.add_space(10);
                        let text = format!("{icon}  {label}");
                        ui.label(RichText::new(text).color(if selected { Color32::WHITE } else { THEME.text_secondary }));
                    });
                    ui.add_space(4);
                });
                let sense = ui.interact(response.response.rect, ui.next_auto_id(), Sense::click());
                if sense.clicked() {
                    self.selected_view = *view;
                }
                ui.add_space(2);
            }
            ui.add_space(12);
        });
    }
}
```

- [ ] **Step 2: Create `src/panels/header.rs`** — Top bar with disk usage bar + scan status + view controls.

```rust
use egui::*;
use crate::models::*;
use crate::theme::THEME;
use crate::util::*;

pub struct Header {}

impl Header {
    pub fn show(
        ui: &mut Ui,
        disk: &DiskSpace,
        scanning: bool,
        scan_progress: &str,
        done: bool,
        entries_len: usize,
        deleting: bool,
        scan_duration: Option<std::time::Duration>,
    ) -> bool {
        let mut rescan = false;
        ui.horizontal(|ui| {
            ui.heading(RichText::new("🧹 Mac Cleaner").color(THEME.text_primary));
            let used = disk.total_bytes.saturating_sub(disk.available_bytes);
            let frac = if disk.total_bytes > 0 { used as f32 / disk.total_bytes as f32 } else { 0.0 };
            ui.add(ProgressBar::new(frac).desired_width(160.0)
                .text(format!("{} / {}", format_size(used), format_size(disk.total_bytes))))
                .on_hover_text(format!("{} available", format_size(disk.available_bytes)));
            if scanning {
                ui.label("⏳");
                ui.label(scan_progress);
                ui.spinner();
            } else if done {
                let dur = scan_duration.map(|d| format!(" in {:.1}s", d.as_secs_f32())).unwrap_or_default();
                ui.label(format!("✓ {} items found{dur}", entries_len));
                if ui.button("🔄 Rescan").clicked() { rescan = true; }
            }
            if deleting { ui.label("🗑 Deleting..."); ui.spinner(); }
        });
        rescan
    }
}
```

- [ ] **Step 3: Create `src/panels/dashboard.rs`** — Dashboard with storage overview and summary.

```rust
use egui::*;
use crate::theme::THEME;
use crate::util::format_size;
use crate::models::CleanupEntry;

pub struct Dashboard;

impl Dashboard {
    pub fn show(ui: &mut Ui, disk: &crate::util::DiskSpace, entries: &[CleanupEntry], scanning: bool, on_scan: &mut bool) {
        if scanning {
            ui.vertical_centered(|ui| { ui.add_space(80.0); ui.heading("Scanning..."); ui.spinner(); });
            return;
        }

        let total_recoverable: u64 = entries.iter().map(|e| e.size_bytes).sum();
        let used = disk.total_bytes.saturating_sub(disk.available_bytes);
        let frac = if disk.total_bytes > 0 { used as f32 / disk.total_bytes as f32 } else { 0.0 };

        ui.add_space(16);

        // Storage card
        Frame::none().fill(THEME.card_bg).corner_radius(8).show(ui, |ui| {
            ui.add_space(16);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("Storage").size(11.0).color(THEME.text_muted));
                ui.add_space(4);
                ui.label(RichText::new(format_size(disk.total_bytes)).size(28.0).color(THEME.accent).strong());
                ui.add_space(8);
                ui.add(ProgressBar::new(frac).desired_width(300.0)
                    .text(format!("{} used of {}", format_size(used), format_size(disk.total_bytes))));
                ui.add_space(4);
                ui.label(RichText::new(format!("{} available", format_size(disk.available_bytes))).color(THEME.text_secondary));
            });
            ui.add_space(16);
        });

        ui.add_space(16);

        // Recoverable summary
        Frame::none().fill(THEME.card_bg).corner_radius(8).show(ui, |ui| {
            ui.add_space(12);
            ui.horizontal(|ui| {
                ui.add_space(12);
                ui.label(RichText::new("Recoverable Space").size(13.0).color(THEME.text_primary));
            });
            ui.add_space(8);
            ui.horizontal(|ui| {
                ui.add_space(12);
                ui.label(RichText::new(format_size(total_recoverable)).size(22.0).color(THEME.success).strong());
                ui.label(RichText::new("in ").color(THEME.text_muted));
                ui.label(RichText::new(format!("{} items", entries.len())).color(THEME.text_secondary));
            });
            ui.add_space(8);
            ui.separator();
            ui.add_space(8);
            ui.horizontal(|ui| {
                ui.add_space(12);
                if ui.button(RichText::new("🧹 Full Scan").color(THEME.text_primary)).clicked() {
                    *on_scan = true;
                }
                if ui.button(RichText::new("Clean Recommended").color(THEME.success)).clicked() {
                    *on_scan = true;
                }
            });
            ui.add_space(12);
        });
    }
}
```

- [ ] **Step 4: Create `src/panels/mod.rs`**

```rust
pub mod sidebar;
pub mod header;
pub mod dashboard;
pub mod cleanup;
pub mod applications;
pub mod large_files;
pub use sidebar::Sidebar;
pub use header::Header;
pub use dashboard::Dashboard;
pub use cleanup::CleanupPanel;
pub use applications::ApplicationsPanel;
pub use large_files::LargeFilesPanel;
```

- [ ] **Step 5: Run `cargo check`**

- [ ] **Step 6: Commit**

---

### Task 5: Cleanup panel

**Files:**
- Create: `src/panels/cleanup.rs`

- [ ] **Step 1: Create `src/panels/cleanup.rs`** — Main cleanup view with sectioned items, search filtering, selection. This is the core of the app.

```rust
use egui::*;
use crate::models::*;
use crate::theme::THEME;
use crate::util::*;
use crate::widgets::search_bar::SearchBar;
use std::collections::HashMap;

pub struct CleanupPanel {
    pub search_bar: SearchBar,
}

impl CleanupPanel {
    pub fn new() -> Self {
        Self { search_bar: SearchBar { search: String::new(), min_size_mb: 0.0 } }
    }

    pub fn show(&mut self, ui: &mut Ui, entries: &mut Vec<CleanupEntry>) {
        if entries.is_empty() {
            ui.vertical_centered(|ui| { ui.add_space(40.0); ui.label("Nothing to clean!"); });
            return;
        }

        // Search bar
        ui.horizontal(|ui| { self.search_bar.show(ui); });
        ui.separator();

        let filtered_ids: Vec<usize> = entries.iter().enumerate()
            .filter(|(_, e)| self.matches_filters(e)).map(|(i, _)| i).collect();

        if filtered_ids.is_empty() {
            ui.vertical_centered(|ui| { ui.add_space(20.0); ui.label("No items match the current filters."); });
            return;
        }

        // Section totals sorted by size desc
        let mut section_totals: HashMap<String, u64> = HashMap::new();
        for &i in &filtered_ids { *section_totals.entry(entries[i].section.clone()).or_insert(0) += entries[i].size_bytes; }
        let mut sections: Vec<String> = section_totals.keys().cloned().collect();
        sections.sort_by(|a, b| section_totals[b].cmp(&section_totals[a]));

        ScrollArea::vertical().id_salt("cleanup_scroll").auto_shrink([false; 2]).show(ui, |ui| {
            for section in &sections {
                let mut indices: Vec<usize> = filtered_ids.iter().copied()
                    .filter(|&i| entries[i].section == *section).collect();
                indices.sort_by(|&a, &b| entries[b].size_bytes.cmp(&entries[a].size_bytes));
                let all_sel = indices.iter().all(|&i| entries[i].selected);
                let sec_total = section_totals[section];

                CollapsingHeader::new(format!("{} {}  —  {}", section_icon(section), section, format_size(sec_total)))
                    .default_open(true).id_salt(section.clone()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let mut a = all_sel;
                        if ui.checkbox(&mut a, "select all in section").clicked() {
                            for &i in &indices { entries[i].selected = a; }
                        }
                    });
                    ui.separator();
                    for &i in &indices {
                        let e = &mut entries[i];
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut e.selected, "");
                            ui.label(format_size(e.size_bytes));
                            match &e.orphan_confidence {
                                Some(OrphanConfidence::High) => { ui.colored_label(THEME.error, &e.label); }
                                Some(OrphanConfidence::Medium) => { ui.colored_label(THEME.warning, &e.label); }
                                Some(OrphanConfidence::Low) => { ui.colored_label(THEME.text_muted, &e.label); }
                                None => { ui.label(&e.label); }
                            }
                            if !e.path.is_empty() {
                                ui.label(format!("  —  {}", e.path)).on_hover_text(&e.path);
                            }
                        });
                    }
                });
            }
        });
    }
}
```

- [ ] **Step 2: Run `cargo check`**

- [ ] **Step 3: Commit**

---

### Task 6: Applications and Large Files panels

**Files:**
- Create: `src/panels/applications.rs`
- Create: `src/panels/large_files.rs`

- [ ] **Step 1: Create `src/panels/applications.rs`** — App uninstaller view.

```rust
use egui::*;
use crate::models::*;
use crate::theme::THEME;
use crate::util::*;
use crate::scanning::find_app_traces;

pub struct ApplicationsPanel {
    pub app_search: String,
    pub installed_apps: Vec<InstalledApp>,
    pub app_traces: Vec<CleanupEntry>,
    pub uninstall_target: Option<usize>,
    pub scanning: bool,
}

impl ApplicationsPanel {
    pub fn new() -> Self {
        Self { app_search: String::new(), installed_apps: vec![], app_traces: vec![], uninstall_target: None, scanning: false }
    }

    pub fn show(&mut self, ui: &mut Ui, dry_run: &mut bool, confirm_items: &mut Vec<CleanupEntry>, confirm_show: &mut bool) {
        ui.heading("🗑️ Uninstall Apps Completely");
        ui.label("Removes the app bundle plus its Application Support, Caches, Preferences, Containers, and other user-level files together.");
        ui.small("Scope note: only ~/Library locations are scanned. System-level files (/Library, LaunchDaemons, package receipts) need admin rights and aren't touched here.");
        ui.separator();

        // Search
        ui.horizontal(|ui| {
            ui.label("🔎");
            ui.add(TextEdit::singleline(&mut self.app_search).hint_text("Filter installed apps...").desired_width(280.0));
            if !self.app_search.is_empty() && ui.button("✕").clicked() { self.app_search.clear(); }
        });
        ui.separator();

        if self.installed_apps.is_empty() && !self.scanning {
            ui.label("No apps found yet — click Rescan Apps.");
        }

        let q = self.app_search.to_lowercase();
        let mut apps: Vec<&InstalledApp> = self.installed_apps.iter()
            .filter(|a| q.is_empty() || a.name.to_lowercase().contains(&q)).collect();
        apps.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));

        let mut clicked_app_id: Option<usize> = None;

        ScrollArea::vertical().id_salt("installed_apps_scroll").auto_shrink([false; 2]).max_height(240.0).show(ui, |ui| {
            for app in &apps {
                ui.horizontal(|ui| {
                    ui.label(format_size(app.size_bytes));
                    ui.label(&app.name);
                    ui.weak(&app.bundle_id);
                    if ui.button("Find traces & select").clicked() { clicked_app_id = Some(app.id); }
                });
            }
        });

        if let Some(id) = clicked_app_id {
            if let Some(app) = self.installed_apps.iter().find(|a| a.id == id) {
                let mut traces = find_app_traces(app);
                for t in &mut traces { t.selected = true; }
                self.app_traces = traces;
                self.uninstall_target = Some(id);
            }
        }

        if let Some(target_id) = self.uninstall_target {
            if self.installed_apps.iter().any(|a| a.id == target_id) {
                let total_sel: u64 = self.app_traces.iter().filter(|e| e.selected).map(|e| e.size_bytes).sum();
                ui.separator();
                ui.label(format!("{} items found — {} selected for removal", self.app_traces.len(), format_size(total_sel)));

                ScrollArea::vertical().id_salt("app_traces_scroll").auto_shrink([false; 2]).max_height(240.0).show(ui, |ui| {
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
                    if ui.button("Select all").clicked() { for t in &mut self.app_traces { t.selected = true; } }
                    if ui.button("Deselect all").clicked() { for t in &mut self.app_traces { t.selected = false; } }
                    let any_selected = self.app_traces.iter().any(|e| e.selected);
                    let btn_label = if *dry_run { "🔍 Preview Uninstall (Dry Run)" } else { "🗑️ Uninstall Completely" };
                    if ui.add_enabled(any_selected, Button::new(btn_label)).clicked() {
                        *confirm_items = self.app_traces.iter().filter(|e| e.selected).cloned().collect();
                        *confirm_show = true;
                    }
                });
            }
        }
    }
}
```

- [ ] **Step 2: Create `src/panels/large_files.rs`** — View for large files found during scan.

```rust
use egui::*;
use crate::models::*;
use crate::util::*;
use crate::theme::THEME;

pub struct LargeFilesPanel;

impl LargeFilesPanel {
    pub fn show(ui: &mut Ui, entries: &mut [CleanupEntry], section: &str) {
        let mut large: Vec<&mut CleanupEntry> = entries.iter_mut().filter(|e| e.section == section).collect();
        if large.is_empty() {
            ui.label("No large files found.");
            return;
        }
        large.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));

        ScrollArea::vertical().id_salt("large_files_scroll").auto_shrink([false; 2]).show(ui, |ui| {
            for e in large.iter_mut() {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut e.selected, "");
                    ui.label(format_size(e.size_bytes));
                    ui.label(&e.label).on_hover_text(&e.path);
                });
            }
        });
    }
}
```

- [ ] **Step 3: Run `cargo check`**

- [ ] **Step 4: Commit**

---

### Task 7: app.rs — main application state + event loop

**Files:**
- Create: `src/app.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Create `src/app.rs`** — The central application state and `eframe::App` implementation.

```rust
use std::sync::mpsc;
use std::time::{Duration, Instant};
use std::collections::HashSet;
use egui::*;
use crate::models::*;
use crate::theme::THEME;
use crate::util::*;
use crate::scanning::{self, execute_cleanup, scan_installed_apps};
use crate::widgets::danger_dialog::DangerDialog;
use crate::panels::*;

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

    // Toast
    pub toast: Option<ToastMessage>,
    pub toast_timer: f32,

    // Scan timing
    pub scan_start: Option<Instant>,
    pub scan_duration: Option<Duration>,

    // Sub-panels
    pub sidebar: Sidebar,
    pub cleanup_panel: CleanupPanel,
    pub applications_panel: ApplicationsPanel,

    // Danger dialog (shared state)
    pub confirm: DangerDialog,

    // App scan state
    pub apps_scan_rx: Option<mpsc::Receiver<AppScanEvent>>,
}

pub struct ToastMessage {
    pub text: String,
    pub is_error: bool,
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
            cleanup_panel: CleanupPanel::new(),
            applications_panel: ApplicationsPanel::new(),
            confirm: DangerDialog::new(),
            apps_scan_rx: None,
        }
    }
}

impl CleanupApp {
    pub fn restart_scan(&mut self) {
        self.disk = get_disk_space("/");
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || scanning::run_scan(tx));
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
        std::thread::spawn(move || scanning::scan_installed_apps(&tx));
        self.applications_panel.installed_apps.clear();
        self.applications_panel.scanning = true;
        self.apps_scan_rx = Some(rx);
        self.applications_panel.uninstall_target = None;
        self.applications_panel.app_traces.clear();
    }

    fn toast(&mut self, msg: impl Into<String>, is_error: bool) {
        self.toast = Some(ToastMessage { text: msg.into(), is_error });
        self.toast_timer = 3.5;
    }
}

impl eframe::App for CleanupApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // --- Drain scan events ---
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

        // --- Drain app scan events ---
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

        // --- Drain deletion results ---
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
                        if !path.is_empty() { deleted_paths.push(path); }
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
                    DeletionResult::Done => { done = true; }
                }
            }
            if !deleted_ids.is_empty() {
                let id_set: HashSet<usize> = deleted_ids.into_iter().collect();
                self.entries.retain(|e| !id_set.contains(&e.id));
                self.applications_panel.app_traces.retain(|e| !id_set.contains(&e.id));
            }
            if !deleted_paths.is_empty() {
                let path_set: HashSet<String> = deleted_paths.into_iter().collect();
                self.applications_panel.installed_apps.retain(|a| !path_set.contains(&a.app_path));
                if let Some(target_id) = self.applications_panel.uninstall_target {
                    if !self.applications_panel.installed_apps.iter().any(|a| a.id == target_id) {
                        self.applications_panel.uninstall_target = None;
                        self.applications_panel.app_traces.clear();
                    }
                }
            }
            if done {
                self.deleting = false;
                self.deletion_rx = None;
                self.disk = get_disk_space("/");
                if had_error { self.toast("Cleanup finished with some errors — check the log.", true); }
                else { self.toast("Cleanup complete!", false); }
            }
            ctx.request_repaint();
        }

        // --- Toast timer ---
        if self.toast.is_some() {
            self.toast_timer -= ctx.input(|i| i.unstable_dt);
            if self.toast_timer <= 0.0 { self.toast = None; }
            ctx.request_repaint();
        }

        let dt = ctx.input(|i| i.unstable_dt);

        // --- Header ---
        #[allow(deprecated)]
        egui::Panel::top("header").show(ctx, |ui| {
            let rescan = Header::show(ui, &self.disk, self.scanning, &self.scan_progress, self.done, self.entries.len(), self.deleting, self.scan_duration);
            if rescan { self.restart_scan(); }
        });

        // --- Sidebar ---
        #[allow(deprecated)]
        egui::Panel::left("sidebar").resizable(false).default_width(180).show(ctx, |ui| {
            let prev = self.sidebar.selected_view;
            self.sidebar.show(ui);
            if self.sidebar.selected_view != prev && self.sidebar.selected_view == AppView::Applications
                && self.applications_panel.installed_apps.is_empty() && !self.applications_panel.scanning {
                self.start_apps_scan();
            }
        });

        // --- Footer ---
        #[allow(deprecated)]
        egui::Panel::bottom("footer").show(ctx, |ui| {
            self.footer_ui(ui, dt);
        });

        // --- Central panel ---
        #[allow(deprecated)]
        egui::CentralPanel::default().show(ctx, |ui| match self.sidebar.selected_view {
            AppView::Dashboard => {
                let mut do_scan = false;
                Dashboard::show(ui, &self.disk, &self.entries, self.scanning, &mut do_scan);
                if do_scan { self.restart_scan(); }
            }
            AppView::Cleanup => {
                if self.entries.is_empty() && self.scanning {
                    ui.vertical_centered(|ui| { ui.add_space(40.0); ui.heading("Scanning..."); ui.spinner(); ui.label(&self.scan_progress); });
                    return;
                }
                if self.entries.is_empty() && !self.scanning {
                    ui.vertical_centered(|ui| { ui.add_space(40.0); ui.label("Nothing to clean!"); });
                    return;
                }
                self.cleanup_panel.show(ui, &mut self.entries);
                // Footer handles cleanup activation
            }
            AppView::Applications => {
                let mut confirm_items = vec![];
                let mut show_confirm = false;
                self.applications_panel.show(ui, &mut self.dry_run, &mut confirm_items, &mut show_confirm);
                if show_confirm { self.confirm.open(confirm_items, self.dry_run); }
            }
            AppView::LargeFiles => {
                LargeFilesPanel::show(ui, &mut self.entries, "Large Files");
            }
        });

        // --- Confirmation dialog ---
        if self.confirm.show {
            if let Some(rx) = self.confirm.show(ctx, self.disk.available_bytes) {
                self.deletion_rx = Some(rx);
                self.deleting = self.confirm.deleting;
            }
        }

        // --- Log window ---
        if self.show_log && !self.log_messages.is_empty() {
            egui::Window::new("📋 Log").resizable(true).default_size([600.0, 300.0])
                .open(&mut self.show_log).show(ctx, |ui| {
                ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                    for msg in &self.log_messages { ui.monospace(msg); }
                });
                if ui.button("Clear log").clicked() { self.log_messages.clear(); }
            });
        }
    }
}
```

- [ ] **Step 2: Add the `footer_ui` method to `CleanupApp`**

Add this method inside `impl CleanupApp`:

```rust
fn footer_ui(&mut self, ui: &mut egui::Ui, _dt: f32) {
    // ... footer with totals, quick-actions, toast
    ui.horizontal(|ui| {
        let selected_bytes: u64 = self.entries.iter().filter(|e| e.selected).map(|e| e.size_bytes).sum();
        let selected_count = self.entries.iter().filter(|e| e.selected).count();
        let total_bytes: u64 = self.entries.iter().map(|e| e.size_bytes).sum();
        ui.label(format!("Total found: {}  —  Selected: {selected_count} items ({})", format_size(total_bytes), format_size(selected_bytes)));
        ui.separator();

        let filtered_ids: Vec<usize> = self.entries.iter().enumerate()
            .filter(|(_, e)| self.cleanup_panel.matches_filters(e))
            .filter(|(_, e)| e.section != "Large Files")
            .map(|(i, _)| i).collect();

        if ui.button("Select all (filtered)").clicked() {
            for &i in &filtered_ids { self.entries[i].selected = true; }
        }
        if ui.button("Clear selection").clicked() {
            for e in &mut self.entries { e.selected = false; }
        }
        if ui.button("Select ≥100 MB").clicked() {
            for &i in &filtered_ids {
                if self.entries[i].size_bytes >= 100 * 1024 * 1024 { self.entries[i].selected = true; }
            }
        }
        if ui.button("Select high-confidence orphans").clicked() {
            for &i in &filtered_ids {
                if self.entries[i].orphan_confidence == Some(OrphanConfidence::High) { self.entries[i].selected = true; }
            }
        }

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let log_label = if self.log_messages.is_empty() { "Log".into() } else { format!("Log ({})", self.log_messages.len()) };
            if ui.button(log_label).clicked() { self.show_log = !self.show_log; }
            ui.checkbox(&mut self.dry_run, "Dry run");
            if ui.add_enabled(selected_count > 0 && !self.deleting, Button::new("🧹 Clean Selected")).clicked() {
                let items: Vec<CleanupEntry> = self.entries.iter().filter(|e| e.selected).cloned().collect();
                self.confirm.open(items, self.dry_run);
            }
            if let Some(ref t) = self.toast {
                let color = if t.is_error { THEME.error } else { THEME.success };
                let icon = if t.is_error { "⚠️" } else { "✅" };
                ui.colored_label(color, icon);
                ui.label(&t.text);
            }
        });
    });
}
```

- [ ] **Step 3: Update `src/main.rs`** — Entry point only.

```rust
mod models;
mod theme;
mod util;
mod scanning;
mod widgets;
mod panels;
mod app;

use app::CleanupApp;

fn main() -> eframe::Result<()> {
    eframe::run_native(
        "Mac Cleaner",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([960.0, 680.0]),
            ..Default::default()
        },
        Box::new(|_cc| {
            let mut app = CleanupApp::default();
            app.restart_scan();
            Ok(Box::new(app))
        }),
    )
}
```

- [ ] **Step 4: Run `cargo check` and fix any compilation errors**

- [ ] **Step 5: Run `cargo build --release` to verify full build**

- [ ] **Step 6: Commit final state**

```bash
git add src/
git commit -m "refactor: complete modular redesign with panels, widgets, and scanning modules"
```

---

### Task 8: Verify

**Files:**
- Run: `cargo check`
- Run: `cargo build --release`
- Test: Launch the app to verify all views render, scan works, sidebar navigation works.

- [ ] **Step 1: Final cargo check**

```bash
cargo check 2>&1
```

- [ ] **Step 2: cargo build --release**

```bash
cargo build --release 2>&1
```

- [ ] **Step 3: Quick smoke test**

```bash
./target/release/cleanup-tool &
sleep 2
kill %1 2>/dev/null
```
Expected: launches and renders without panic.
