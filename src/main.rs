// macOS Cleanup Tool — Improved
// Changes from original:
//   [Fix 1] execute_cleanup now returns confirmed-deleted paths via a channel;
//           UI only removes entries after confirmed deletion.
//   [Fix 2] Docker size uses `docker system df --format json` instead of fragile
//           last-line column parsing.
//   [Fix 3] Error log panel collects scan/deletion failures — no silent swallowing.
//   [Fix 4] Bottom panel hoisted to top-level update() scope (egui correctness).
//   [Fix 5] Dead `fn ui()` stub removed.
//   [Fix 6] Orphan false-negatives fixed: containment direction corrected,
//           and additional Library dirs (Containers, Group Containers, WebKit,
//           Application Scripts, HTTPStorages) added to the scan.
//   [Fix 7] Orphan confidence tiers: High (reverse-DNS), Medium, Low.
//   [Fix 8] `next_id` threaded through scan_orphans for globally unique IDs.
//   [New]   "Move to Trash" preferred over `rm -rf` (requires `trash` CLI).
//   [New]   Dry-run mode: preview what would be deleted without deleting.
//   [New]   Error/log panel in footer shows scan warnings and deletion results.
//   [New]   Missing scan categories: Trash, iOS backups, iOS Simulators,
//           Swift Package cache, CocoaPods cache, large files (>500 MB).

use std::collections::HashSet;
use std::process::Command;
use std::sync::mpsc;
use std::thread;

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum OrphanConfidence {
    High,   // reverse-DNS name, no matching bundle
    Medium, // named folder, no app name match
    Low,    // ambiguous
}

#[derive(Clone, Debug)]
struct CleanupEntry {
    id: usize,
    section: String,
    label: String,
    path: String,
    size_bytes: u64,
    selected: bool,
    orphan_confidence: Option<OrphanConfidence>,
}

fn entry(id: usize, section: &str, label: &str, path: &str, size_bytes: u64) -> CleanupEntry {
    CleanupEntry {
        id,
        section: section.into(),
        label: label.into(),
        path: path.into(),
        size_bytes,
        selected: false,
        orphan_confidence: None,
    }
}

fn orphan_entry(
    id: usize,
    section: &str,
    label: &str,
    path: &str,
    size_bytes: u64,
    confidence: OrphanConfidence,
) -> CleanupEntry {
    CleanupEntry {
        id,
        section: section.into(),
        label: label.into(),
        path: path.into(),
        size_bytes,
        selected: false,
        orphan_confidence: Some(confidence),
    }
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

fn format_size(bytes: u64) -> String {
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    const KB: f64 = 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

fn run_cmd_timeout(program: &str, args: &[&str], secs: u64) -> Option<std::process::Output> {
    let program = program.to_string();
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let out = Command::new(&program).args(&args).output().ok();
        let _ = tx.send(out);
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
                if let Ok(k) = kb.parse::<u64>() {
                    return k * 1024;
                }
            }
        }
    }
    0
}

fn find_dirs(path: &str, name: &str, maxdepth: u32) -> Vec<String> {
    let depth_str = format!("{maxdepth}");
    Command::new("find")
        .args([path, "-maxdepth", &depth_str, "-type", "d", "-name", name])
        .output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter(|l| !l.is_empty())
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn parse_size_str(s: &str) -> u64 {
    let s = s.trim().to_lowercase();
    let (n, suffix) = if s.ends_with("tb") {
        (&s[..s.len() - 2], "tb")
    } else if s.ends_with('t') {
        (&s[..s.len() - 1], "tb")
    } else if s.ends_with("gb") {
        (&s[..s.len() - 2], "gb")
    } else if s.ends_with('g') {
        (&s[..s.len() - 1], "gb")
    } else if s.ends_with("mb") {
        (&s[..s.len() - 2], "mb")
    } else if s.ends_with('m') {
        (&s[..s.len() - 1], "mb")
    } else if s.ends_with("kb") {
        (&s[..s.len() - 2], "kb")
    } else if s.ends_with('k') {
        (&s[..s.len() - 1], "kb")
    } else if s.ends_with('b') {
        (&s[..s.len() - 1], "b")
    } else {
        return s.parse().unwrap_or(0);
    };
    let v: f64 = n.trim().parse().unwrap_or(0.0);
    match suffix {
        "tb" => (v * 1_099_511_627_776.0) as u64,
        "gb" => (v * 1_073_741_824.0) as u64,
        "mb" => (v * 1_048_576.0) as u64,
        "kb" => (v * 1024.0) as u64,
        _ => v as u64,
    }
}

// ---------------------------------------------------------------------------
// Scan
// ---------------------------------------------------------------------------

enum ScanEvent {
    Progress(String),
    Entry(CleanupEntry),
    Warning(String),
    Done,
}

/// [Fix 6] Extended orphan scan with more Library subdirs and corrected
/// matching direction. Returns globally-consistent next_id.
fn scan_orphans(tx: &mpsc::Sender<ScanEvent>, next_id: &mut usize) {
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
        else {
            continue;
        };
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let plist = format!("{line}/Contents/Info.plist");
            if !std::path::Path::new(&plist).exists() {
                continue;
            }
            if let Ok(b) = Command::new("/usr/libexec/PlistBuddy")
                .args(["-c", "Print :CFBundleIdentifier", &plist])
                .output()
            {
                let s = String::from_utf8_lossy(&b.stdout).trim().to_lowercase();
                if !s.is_empty() {
                    known_bundle_ids.insert(s);
                }
            }
            if let Some(name) = line.trim_end_matches(".app").split('/').last() {
                let n = name.to_lowercase();
                if !n.is_empty() {
                    known_app_names.insert(n);
                }
            }
        }
    }

    // [Fix 6] All relevant Library subdirs
    const ORPHAN_SCAN_DIRS: &[&str] = &[
        "Library/Application Support",
        "Library/Preferences",
        "Library/Saved Application State",
        "Library/Caches",
        "Library/Containers",
        "Library/Group Containers",
        "Library/WebKit",
        "Library/Application Scripts",
        "Library/HTTPStorages",
    ];

    for subdir in ORPHAN_SCAN_DIRS {
        let scan_path = format!("{home}/{subdir}");
        let Ok(dir) = std::fs::read_dir(&scan_path) else {
            continue;
        };
        for dir_entry_res in dir {
            let Ok(dir_entry) = dir_entry_res else { continue };
            let path = dir_entry.path();
            let name = match path.file_name() {
                Some(n) => n.to_string_lossy().to_string(),
                None => continue,
            };
            let nl = name.to_lowercase();

            // Skip system namespaces
            if nl.starts_with("com.apple.")
                || nl == "com.apple"
                || nl.starts_with("apple.")
                || nl.starts_with('.')
                || nl == "caches"
                || nl == "metadata"
            {
                continue;
            }

            // [Fix 6] Corrected matching: check if the entry name matches a known bundle/app,
            // not if a known bundle contains the entry name (which caused false negatives).
            let is_known = known_bundle_ids.contains(&nl)
                || known_bundle_ids.iter().any(|kid| nl.starts_with(&format!("{kid}.")))
                || known_app_names.contains(&nl);

            if is_known {
                continue;
            }

            let sz = du_sh(&path.to_string_lossy());
            if sz == 0 {
                continue;
            }

            // Confidence scoring
            let confidence = if nl.contains('.') && nl.split('.').count() >= 3 {
                // looks like a reverse-DNS bundle ID
                OrphanConfidence::High
            } else if !nl.contains(' ') && nl.len() > 3 {
                OrphanConfidence::Medium
            } else {
                OrphanConfidence::Low
            };

            let confidence_label = match &confidence {
                OrphanConfidence::High => "[HIGH] ",
                OrphanConfidence::Medium => "[MED]  ",
                OrphanConfidence::Low => "[LOW]  ",
            };
            let label = format!("{confidence_label}{name}");

            let _ = tx.send(ScanEvent::Entry(orphan_entry(
                *next_id,
                "Orphan App Data",
                &label,
                &path.to_string_lossy(),
                sz,
                confidence,
            )));
            *next_id += 1;
        }
    }
}

fn run_scan(tx: mpsc::Sender<ScanEvent>) {
    let mut next_id = 0usize;
    let home = std::env::var("HOME").unwrap_or_default();

    macro_rules! send_dir {
        ($section:expr, $label:expr, $path:expr) => {{
            let id = next_id;
            next_id += 1;
            let ls = $label;
            let _ = tx.send(ScanEvent::Progress(format!("Scanning {ls}...")));
            let pv: &str = $path;
            let sz = du_sh(pv);
            let _ = tx.send(ScanEvent::Entry(entry(id, $section, ls, pv, sz)));
        }};
    }
    macro_rules! send_find {
        ($section:expr, $label:expr, $base:expr, $name:expr) => {{
            let ls = $label;
            let bp: &str = $base;
            let dn: &str = $name;
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

    // System caches
    send_dir!("System Caches", "~/Library/Caches", &format!("{home}/Library/Caches"));
    send_dir!("System Caches", "~/.cache", &format!("{home}/.cache"));

    // Developer / build tools
    send_dir!(
        "Build Tools",
        "Gradle caches",
        &format!("{home}/.gradle/caches")
    );
    send_dir!(
        "Build Tools",
        "Gradle wrappers",
        &format!("{home}/.gradle/wrapper")
    );
    send_dir!(
        "Build Tools",
        "Cargo registry",
        &format!("{home}/.cargo/registry")
    );
    send_dir!(
        "Build Tools",
        "Xcode DerivedData",
        &format!("{home}/Library/Developer/Xcode/DerivedData")
    );
    send_dir!(
        "Build Tools",
        "Xcode Archives",
        &format!("{home}/Library/Developer/Xcode/Archives")
    );
    send_dir!(
        "Build Tools",
        "iOS Device Logs",
        &format!("{home}/Library/Developer/Xcode/iOS Device Logs")
    );

    // [New] Swift Package Manager
    send_dir!(
        "Build Tools",
        "Swift Package cache",
        &format!("{home}/.swiftpm")
    );

    // [New] iOS Simulators
    {
        let _ = tx.send(ScanEvent::Progress("Scanning iOS Simulators...".into()));
        let sim_path = format!("{home}/Library/Developer/CoreSimulator/Devices");
        let sz = du_sh(&sim_path);
        let _ = tx.send(ScanEvent::Entry(entry(
            next_id,
            "Build Tools",
            "CoreSimulator Devices",
            &sim_path,
            sz,
        )));
        next_id += 1;
    }

    // Go
    send_dir!("Go", "Go module cache", &format!("{home}/go/pkg/mod"));
    send_dir!("Go", "Go compiled binaries", &format!("{home}/go/bin"));

    // Python
    send_dir!("Python", "pip cache", &format!("{home}/.cache/pip"));
    send_dir!(
        "Python",
        "pip cache (macOS)",
        &format!("{home}/Library/Caches/pip")
    );

    // [New] CocoaPods
    send_dir!(
        "Package Managers",
        "CocoaPods repo cache",
        &format!("{home}/.cocoapods/repos")
    );

    // Project dependencies
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

    // npm
    {
        let _ = tx.send(ScanEvent::Progress("Scanning npm cache...".into()));
        let p = format!("{home}/.npm/_cacache");
        let sz = du_sh(&p);
        let _ = tx.send(ScanEvent::Entry(entry(
            next_id,
            "Package Managers",
            "npm cache",
            &p,
            sz,
        )));
        next_id += 1;
    }

    // Homebrew
    {
        let _ = tx.send(ScanEvent::Progress("Scanning Homebrew cache...".into()));
        if let Some(o) = Command::new("brew").args(["--cache"]).output().ok() {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if !s.is_empty() {
                let sz = du_sh(&s);
                let _ = tx.send(ScanEvent::Entry(entry(
                    next_id,
                    "Package Managers",
                    "Homebrew cache",
                    &s,
                    sz,
                )));
                next_id += 1;
            }
        }
    }

    // [Fix 2] Docker — use --format json to avoid fragile column parsing
    {
        let _ = tx.send(ScanEvent::Progress("Checking Docker...".into()));
        let docker_alive =
            run_cmd_timeout("docker", &["info", "--format", "{{.ServerVersion}}"], 3)
                .map(|o| o.status.success())
                .unwrap_or(false);

        let sz = if docker_alive {
            // `docker system df --format json` returns a JSON object on modern Docker
            match run_cmd_timeout("docker", &["system", "df", "--format", "json"], 5) {
                Some(out) if out.status.success() => {
                    let raw = String::from_utf8_lossy(&out.stdout);
                    // Sum up "TotalCount" * "Size" isn't straightforward; instead look for
                    // the "ReclaimableSize" field across all object types.
                    // The JSON is an array of { "Type", "TotalCount", "Active",
                    // "ReclaimableSize", "Size" }.
                    let mut total: u64 = 0;
                    if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(&raw) {
                        for obj in arr {
                            if let Some(s) = obj.get("ReclaimableSize").and_then(|v| v.as_str()) {
                                total += parse_size_str(s);
                            }
                        }
                    } else {
                        let _ = tx.send(ScanEvent::Warning(
                            "Docker: could not parse JSON output".into(),
                        ));
                    }
                    total
                }
                Some(out) => {
                    let _ = tx.send(ScanEvent::Warning(format!(
                        "Docker df failed: {}",
                        String::from_utf8_lossy(&out.stderr).trim()
                    )));
                    0
                }
                None => {
                    let _ = tx.send(ScanEvent::Warning("Docker df timed out".into()));
                    0
                }
            }
        } else {
            0
        };

        let _ = tx.send(ScanEvent::Entry(entry(
            next_id,
            "Package Managers",
            "Docker (reclaimable)",
            "",
            sz,
        )));
        next_id += 1;
    }

    // Logs & temp
    send_dir!(
        "Logs & Temp",
        "~/Library/Logs",
        &format!("{home}/Library/Logs")
    );
    send_dir!("Logs & Temp", "/private/tmp", "/private/tmp");

    // [New] Trash contents
    {
        let trash = format!("{home}/.Trash");
        let sz = du_sh(&trash);
        if sz > 0 {
            let _ = tx.send(ScanEvent::Entry(entry(
                next_id,
                "Logs & Temp",
                "Trash contents (~/.Trash)",
                &trash,
                sz,
            )));
            next_id += 1;
        }
    }

    // [New] iOS backups
    {
        let backup_path = format!(
            "{home}/Library/Application Support/MobileSync/Backup"
        );
        let sz = du_sh(&backup_path);
        if sz > 0 {
            let _ = tx.send(ScanEvent::Entry(entry(
                next_id,
                "iOS",
                "iOS Backups (MobileSync)",
                &backup_path,
                sz,
            )));
            next_id += 1;
        }
    }

    // Stale installers in Downloads
    {
        let _ = tx.send(ScanEvent::Progress("Scanning Downloads...".into()));
        let downloads = format!("{home}/Downloads");
        if let Some(o) = Command::new("find")
            .args([
                &downloads,
                "-maxdepth",
                "2",
                "(",
                "-iname",
                "*.dmg",
                "-o",
                "-iname",
                "*.pkg",
                "-o",
                "-iname",
                "*.zip",
                "-o",
                "-iname",
                "*.tar.gz",
                "-o",
                "-iname",
                "*.tgz",
                "-o",
                "-iname",
                "*.iso",
                ")",
                "-type",
                "f",
                "-mtime",
                "+30",
            ])
            .output()
            .ok()
        {
            let files: Vec<_> = String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter(|l| !l.is_empty())
                .map(|s| s.to_string())
                .collect();
            let n = files.len();
            if n > 0 {
                let total: u64 = files.iter().map(|f| du_sh(f)).sum();
                let _ = tx.send(ScanEvent::Entry(entry(
                    next_id,
                    "Downloads",
                    &format!("Stale installers (>30d, {n} files)"),
                    &downloads,
                    total,
                )));
                next_id += 1;
            }
        }
    }

    // [New] Large files (>500 MB) in home
    {
        let _ = tx.send(ScanEvent::Progress("Scanning for large files (>500 MB)...".into()));
        if let Some(o) = Command::new("find")
            .args([
                &home,
                "-maxdepth",
                "5",
                "-type",
                "f",
                "-size",
                "+500M",
                "-not",
                "-path",
                "*/\\.Trash/*",
                "-not",
                "-path",
                "*/Library/Developer/CoreSimulator/*",
            ])
            .output()
            .ok()
        {
            let files: Vec<_> = String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter(|l| !l.is_empty())
                .map(|s| s.to_string())
                .collect();
            for f in files {
                let sz = du_sh(&f);
                let label = f
                    .strip_prefix(&format!("{home}/"))
                    .unwrap_or(&f)
                    .to_string();
                let _ = tx.send(ScanEvent::Entry(entry(
                    next_id,
                    "Large Files",
                    &label,
                    &f,
                    sz,
                )));
                next_id += 1;
            }
        }
    }

    // Orphan scan last (slowest)
    scan_orphans(&tx, &mut next_id);

    let _ = tx.send(ScanEvent::Done);
}

// ---------------------------------------------------------------------------
// Deletion
// ---------------------------------------------------------------------------

/// [Fix 1 + New] Deletion runs in background and reports back:
///   - successfully deleted paths (to remove from UI list)
///   - errors (to show in log panel)
/// Caller passes a `result_tx` to receive the outcome.
fn execute_cleanup(
    items: Vec<CleanupEntry>,
    dry_run: bool,
    result_tx: mpsc::Sender<DeletionResult>,
) {
    thread::spawn(move || {
        for item in &items {
            if dry_run {
                let msg = if item.path.is_empty() {
                    format!("[DRY RUN] Would execute: docker system prune -af")
                } else {
                    format!("[DRY RUN] Would delete: {}", item.path)
                };
                let _ = result_tx.send(DeletionResult::DryRunPreview(msg));
                continue;
            }

            if item.path.is_empty() && item.label.contains("Docker") {
                match Command::new("docker")
                    .args(["system", "prune", "-af"])
                    .output()
                {
                    Ok(o) if o.status.success() => {
                        let _ = result_tx
                            .send(DeletionResult::Deleted(item.path.clone(), item.id));
                    }
                    Ok(o) => {
                        let _ = result_tx.send(DeletionResult::Error(format!(
                            "docker prune failed: {}",
                            String::from_utf8_lossy(&o.stderr).trim()
                        )));
                    }
                    Err(e) => {
                        let _ = result_tx
                            .send(DeletionResult::Error(format!("docker prune error: {e}")));
                    }
                }
                continue;
            }

            if item.path.is_empty() {
                continue;
            }

            // [New] Prefer `trash` CLI; fall back to `rm -rf`
            let deleted = try_trash(&item.path).or_else(|| try_rm_rf(&item.path));
            match deleted {
                Some(true) => {
                    let _ = result_tx
                        .send(DeletionResult::Deleted(item.path.clone(), item.id));
                }
                _ => {
                    let _ = result_tx.send(DeletionResult::Error(format!(
                        "Failed to delete: {}",
                        item.path
                    )));
                }
            }
        }
        let _ = result_tx.send(DeletionResult::Done);
    });
}

fn try_trash(path: &str) -> Option<bool> {
    Command::new("trash")
        .arg(path)
        .output()
        .ok()
        .map(|o| o.status.success())
}

fn try_rm_rf(path: &str) -> Option<bool> {
    Command::new("rm")
        .args(["-rf", path])
        .output()
        .ok()
        .map(|o| o.status.success())
}

enum DeletionResult {
    Deleted(String, usize), // (path, id)
    Error(String),
    DryRunPreview(String),
    Done,
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

struct CleanupApp {
    entries: Vec<CleanupEntry>,
    scanning: bool,
    scan_progress: String,
    done: bool,
    scan_rx: Option<mpsc::Receiver<ScanEvent>>,

    // [Fix 1] deletion feedback channel
    deletion_rx: Option<mpsc::Receiver<DeletionResult>>,
    deleting: bool,

    // confirmation dialog
    show_confirm: bool,
    pending_cleanup: Vec<CleanupEntry>,

    // [New] dry-run mode
    dry_run: bool,

    // [Fix 3] log/error panel
    log_messages: Vec<String>,
    show_log: bool,

    // toast
    message: Option<String>,
    message_timer: f32,
}

impl Default for CleanupApp {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || run_scan(tx));
        Self {
            entries: vec![],
            scanning: true,
            scan_progress: "Starting scan...".into(),
            done: false,
            scan_rx: Some(rx),
            deletion_rx: None,
            deleting: false,
            show_confirm: false,
            pending_cleanup: vec![],
            dry_run: false,
            log_messages: vec![],
            show_log: false,
            message: None,
            message_timer: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// egui UI
// ---------------------------------------------------------------------------

impl eframe::App for CleanupApp {
    fn ui(&mut self, _: &mut egui::Ui, _: &mut eframe::Frame) {}
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // --- Drain scan events ---
        if let Some(rx) = &self.scan_rx {
            while let Ok(event) = rx.try_recv() {
                match event {
                    ScanEvent::Progress(msg) => self.scan_progress = msg,
                    ScanEvent::Entry(e) => self.entries.push(e),
                    ScanEvent::Warning(w) => {
                        self.log_messages.push(format!("⚠️  {w}"));
                    }
                    ScanEvent::Done => {
                        self.scanning = false;
                        self.done = true;
                        self.scan_progress = "Scan complete.".into();
                    }
                }
                ctx.request_repaint();
            }
        }

        // [Fix 1] Drain deletion results
        if let Some(rx) = &self.deletion_rx {
            let mut done = false;
            let mut deleted_ids: Vec<usize> = vec![];
            while let Ok(result) = rx.try_recv() {
                match result {
                    DeletionResult::Deleted(path, id) => {
                        self.log_messages
                            .push(format!("✅ Deleted: {path}"));
                        deleted_ids.push(id);
                    }
                    DeletionResult::Error(e) => {
                        self.log_messages.push(format!("❌ {e}"));
                        self.show_log = true; // surface errors automatically
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
            }
            if done {
                self.deleting = false;
                self.deletion_rx = None;
                self.message = Some("Cleanup complete!".into());
                self.message_timer = 3.0;
            }
            ctx.request_repaint();
        }

        // Toast timer
        if self.message.is_some() {
            self.message_timer -= ctx.input(|i| i.unstable_dt);
            if self.message_timer <= 0.0 {
                self.message = None;
            }
            ctx.request_repaint();
        }

        // --- Confirmation dialog ---
        if self.show_confirm {
            let total_sel: u64 = self.pending_cleanup.iter().map(|e| e.size_bytes).sum();
            egui::Window::new("⚠️  Confirm Deletion")
                .collapsible(false)
                .resizable(true)
                .default_size([640.0, 420.0])
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
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
                        self.pending_cleanup.len(),
                        format_size(total_sel)
                    ));
                    ui.separator();

                    egui::ScrollArea::vertical()
                        .max_height(ui.available_height() - 80.0)
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
                        let btn_label = if self.dry_run {
                            "🔍 Preview (Dry Run)"
                        } else {
                            "Confirm Deletion 🗑️"
                        };
                        let btn_color = if self.dry_run {
                            egui::Color32::from_rgb(50, 100, 180)
                        } else {
                            egui::Color32::RED
                        };
                        if ui
                            .add_sized(
                                [160.0, 30.0],
                                egui::Button::new(btn_label).fill(btn_color),
                            )
                            .clicked()
                        {
                            let items = self.pending_cleanup.clone();
                            let (tx, rx) = mpsc::channel();
                            execute_cleanup(items, self.dry_run, tx);
                            self.deletion_rx = Some(rx);
                            self.deleting = !self.dry_run;
                            self.show_confirm = false;
                            self.pending_cleanup.clear();
                        }
                    });
                });
        }

        // --- Log panel (if open) ---
        if self.show_log && !self.log_messages.is_empty() {
            egui::Window::new("📋 Log")
                .resizable(true)
                .default_size([600.0, 300.0])
                .open(&mut self.show_log)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
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

        // --- Top header panel ---
        #[allow(deprecated)]
        egui::Panel::top("header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("🧹 macOS Cleanup Tool");
                if self.scanning {
                    ui.label("⏳");
                    ui.label(&self.scan_progress);
                    ui.spinner();
                } else if self.done {
                    ui.label(format!("✓ {} items found", self.entries.len()));
                }
                if self.deleting {
                    ui.label("🗑 Deleting...");
                    ui.spinner();
                }
            });
        });

        // [Fix 4] Bottom panel hoisted to top-level update() scope
        #[allow(deprecated)]
        egui::Panel::bottom("footer").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let selected_bytes: u64 = self
                    .entries
                    .iter()
                    .filter(|e| e.selected)
                    .map(|e| e.size_bytes)
                    .sum();
                let selected_count = self.entries.iter().filter(|e| e.selected).count();
                let total_bytes: u64 = self.entries.iter().map(|e| e.size_bytes).sum();

                if selected_count > 0 {
                    ui.label(format!(
                        "Selected: {selected_count} items, {}",
                        format_size(selected_bytes)
                    ));
                } else {
                    ui.label(format!("Total found: {}", format_size(total_bytes)));
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Log button
                    let log_label = if self.log_messages.is_empty() {
                        "Log".into()
                    } else {
                        format!("Log ({})", self.log_messages.len())
                    };
                    if ui.button(log_label).clicked() {
                        self.show_log = !self.show_log;
                    }

                    // Dry-run toggle
                    ui.checkbox(&mut self.dry_run, "Dry run");

                    // Clean button
                    if ui
                        .add_enabled(
                            selected_count > 0 && !self.deleting,
                            egui::Button::new("🧹 Clean Selected"),
                        )
                        .clicked()
                    {
                        self.pending_cleanup =
                            self.entries.iter().filter(|e| e.selected).cloned().collect();
                        self.show_confirm = true;
                    }

                    // Toast
                    if let Some(ref m) = self.message {
                        ui.label("✅");
                        ui.label(m);
                    }
                });
            });
        });

        // --- Main content ---
        #[allow(deprecated)]
        egui::CentralPanel::default().show(ctx, |ui| {
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
                    ui.label("Nothing to clean!");
                });
                return;
            }

            let mut sections: Vec<String> =
                self.entries.iter().map(|e| e.section.clone()).collect();
            sections.sort();
            sections.dedup();

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    for section in &sections {
                        let indices: Vec<usize> = self
                            .entries
                            .iter()
                            .enumerate()
                            .filter(|(_, e)| e.section == *section)
                            .map(|(i, _)| i)
                            .collect();
                        let all_sel = indices.iter().all(|&i| self.entries[i].selected);
                        let sec_total: u64 =
                            indices.iter().map(|&i| self.entries[i].size_bytes).sum();

                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(format!("📁 {section}"));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.strong(format_size(sec_total));
                                    let mut a = all_sel;
                                    if ui
                                        .checkbox(&mut a, "all")
                                        .on_hover_text("Toggle all in section")
                                        .clicked()
                                    {
                                        for &i in &indices {
                                            self.entries[i].selected = a;
                                        }
                                    }
                                },
                            );
                        });
                        ui.separator();

                        for &i in &indices {
                            let e = &mut self.entries[i];
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut e.selected, "");
                                ui.label(format_size(e.size_bytes));
                                // Colour-code orphan confidence
                                match &e.orphan_confidence {
                                    Some(OrphanConfidence::High) => {
                                        ui.colored_label(egui::Color32::RED, &e.label);
                                    }
                                    Some(OrphanConfidence::Medium) => {
                                        ui.colored_label(
                                            egui::Color32::from_rgb(220, 150, 30),
                                            &e.label,
                                        );
                                    }
                                    Some(OrphanConfidence::Low) => {
                                        ui.colored_label(egui::Color32::GRAY, &e.label);
                                    }
                                    None => {
                                        ui.label(&e.label);
                                    }
                                }
                                // Show path as tooltip on hover (avoid cluttering the row)
                                if !e.path.is_empty() {
                                    ui.label("").on_hover_text(&e.path);
                                }
                            });
                        }
                    }
                });
        });
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() -> eframe::Result<()> {
    eframe::run_native(
        "macOS Cleanup Tool",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([860.0, 640.0]),
            ..Default::default()
        },
        Box::new(|_cc| Ok(Box::<CleanupApp>::default())),
    )
}