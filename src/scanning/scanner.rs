use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

use crate::models::*;
use crate::util::*;

macro_rules! time_section {
    ($name:expr, $body:block) => {{
        let _start = Instant::now();
        $body;
        let _dur = _start.elapsed();
        eprintln!("  [{:>6?}] {}", _dur, $name);
    }};
}

pub fn run_scan(tx: mpsc::Sender<ScanEvent>) {
    let _scan_start = Instant::now();
    let mut next_id = 0usize;
    let home = std::env::var("HOME").unwrap_or_default();
    let projects = format!("{home}/Projects");

    macro_rules! emit_section {
        ($section:expr, [$(($label:expr, $path:expr)),+ $(,)?]) => {{
            let _ = tx.send(ScanEvent::Progress(format!("Scanning {}...", $section)));
            let paths: Vec<String> = vec![$($path.into()),+];
            let sizes = du_batch(&paths);
            let labels = [$(($label, $path)),+];
            for ((label, path), _) in labels.iter().zip(paths.iter()) {
                let sz = *sizes.get(path.as_str()).unwrap_or(&0);
                let _ = tx.send(ScanEvent::Entry(entry(next_id, $section, label, path, sz)));
                next_id += 1;
            }
        }};
    }

    time_section!("System Caches", {
        emit_section!("System Caches", [
            ("~/Library/Caches", format!("{home}/Library/Caches")),
            ("~/.cache", format!("{home}/.cache")),
        ]);
    });

    time_section!("Build Tools", {
        emit_section!("Build Tools", [
            ("Gradle caches", format!("{home}/.gradle/caches")),
            ("Gradle wrappers", format!("{home}/.gradle/wrapper")),
            ("Cargo registry", format!("{home}/.cargo/registry")),
            ("Xcode DerivedData", format!("{home}/Library/Developer/Xcode/DerivedData")),
            ("Xcode Archives", format!("{home}/Library/Developer/Xcode/Archives")),
            ("iOS Device Logs", format!("{home}/Library/Developer/Xcode/iOS Device Logs")),
            ("Swift Package cache", format!("{home}/.swiftpm")),
        ]);
    });

    time_section!("iOS Simulators", {
        let _ = tx.send(ScanEvent::Progress("Scanning iOS Simulators...".into()));
        let sim_path = format!("{home}/Library/Developer/CoreSimulator/Devices");
        let sz = du_sh(&sim_path);
        let _ = tx.send(ScanEvent::Entry(entry(next_id, "Build Tools", "CoreSimulator Devices", &sim_path, sz)));
        next_id += 1;
    });

    time_section!("Go", {
        emit_section!("Go", [
            ("Go module cache", format!("{home}/go/pkg/mod")),
            ("Go compiled binaries", format!("{home}/go/bin")),
        ]);
    });

    time_section!("Python", {
        emit_section!("Python", [
            ("pip cache", format!("{home}/.cache/pip")),
            ("pip cache (macOS)", format!("{home}/Library/Caches/pip")),
        ]);
    });

    time_section!("CocoaPods", {
        let _ = tx.send(ScanEvent::Progress("Scanning CocoaPods...".into()));
        let pods_path = format!("{home}/.cocoapods/repos");
        let sz = du_sh(&pods_path);
        let _ = tx.send(ScanEvent::Entry(entry(next_id, "Package Managers", "CocoaPods repo cache", &pods_path, sz)));
        next_id += 1;
    });

    time_section!("Project deps", {
        let _ = tx.send(ScanEvent::Progress("Searching project dependencies...".into()));
        const PATTERNS: &[(&str, &str)] = &[
            ("node_modules", "node_modules"),
            ("target (Rust)", "target"),
            ("build dirs", "build"),
            (".dart_tool", ".dart_tool"),
            ("Pods (CocoaPods)", "Pods"),
            (".gradle (per-project)", ".gradle"),
            (".m2 (Maven)", ".m2"),
            ("__pycache__", "__pycache__"),
            (".venv (Python)", ".venv"),
            ("vendor dirs", "vendor"),
        ];

        let mut handles = Vec::new();
        for (label, dirname) in PATTERNS {
            let projects = projects.clone();
            let label = label.to_string();
            let dirname = dirname.to_string();
            handles.push(thread::spawn(move || {
                let dirs = find_dirs(&projects, &dirname, 5);
                let sizes = du_batch(&dirs);
                dirs.into_iter().map(move |d| {
                    let sz = *sizes.get(&d).unwrap_or(&0);
                    let short = d.strip_prefix(&format!("{projects}/")).unwrap_or(&d).to_string();
                    (label.clone(), short, d, sz)
                }).collect::<Vec<_>>()
            }));
        }

        for h in handles {
            let entries = h.join().unwrap_or_default();
            for (label, _short, path, sz) in &entries {
                let _ = tx.send(ScanEvent::Entry(entry(next_id, "Project Deps", label, path, *sz)));
                next_id += 1;
            }
        }
    });

    time_section!("npm cache", {
        let _ = tx.send(ScanEvent::Progress("Scanning npm cache...".into()));
        let p = format!("{home}/.npm/_cacache");
        let sz = du_sh(&p);
        let _ = tx.send(ScanEvent::Entry(entry(next_id, "Package Managers", "npm cache", &p, sz)));
        next_id += 1;
    });

    time_section!("Homebrew cache", {
        let _ = tx.send(ScanEvent::Progress("Scanning Homebrew cache...".into()));
        if let Some(o) = Command::new("brew").args(["--cache"]).output().ok() {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if !s.is_empty() {
                let sz = du_sh(&s);
                let _ = tx.send(ScanEvent::Entry(entry(next_id, "Package Managers", "Homebrew cache", &s, sz)));
                next_id += 1;
            }
        }
    });

    time_section!("Docker", {
        let _ = tx.send(ScanEvent::Progress("Checking Docker...".into()));
        let docker_socket = std::path::Path::new("/var/run/docker.sock");
        let sz = if docker_socket.exists() {
            let alive = run_cmd_timeout("docker", &["info", "--format", "{{.ServerVersion}}"], 2)
                .map(|o| o.status.success())
                .unwrap_or(false);
            if alive {
                match run_cmd_timeout("docker", &["system", "df", "--format", "json"], 3) {
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
            } else {
                0
            }
        } else {
            0
        };
        let _ = tx.send(ScanEvent::Entry(entry(next_id, "Package Managers", "Docker (reclaimable)", "", sz)));
        next_id += 1;
    });

    time_section!("Logs & Temp", {
        emit_section!("Logs & Temp", [
            ("~/Library/Logs", format!("{home}/Library/Logs")),
            ("/private/tmp", "/private/tmp".to_string()),
        ]);
    });

    time_section!("Trash", {
        let trash = format!("{home}/.Trash");
        let sz = du_sh(&trash);
        if sz > 0 {
            let _ = tx.send(ScanEvent::Entry(entry(next_id, "Logs & Temp", "Trash contents (~/.Trash)", &trash, sz)));
            next_id += 1;
        }
    });

    time_section!("iOS Backups", {
        let backup = format!("{home}/Library/Application Support/MobileSync/Backup");
        let sz = du_sh(&backup);
        if sz > 0 {
            let _ = tx.send(ScanEvent::Entry(entry(next_id, "iOS", "iOS Backups (MobileSync)", &backup, sz)));
            next_id += 1;
        }
    });

    time_section!("Stale Downloads", {
        let _ = tx.send(ScanEvent::Progress("Scanning Downloads...".into()));
        let dl = format!("{home}/Downloads");
        if let Some(files) = find_stale_downloads(&dl) {
            let n = files.len();
            let paths: Vec<String> = files;
            let sizes = du_batch(&paths);
            let total: u64 = sizes.values().sum();
            let _ = tx.send(ScanEvent::Entry(entry(next_id, "Downloads", &format!("Stale installers (>30d, {n} files)"), &dl, total)));
            next_id += 1;
        }
    });

    time_section!("Large Files", {
        let _ = tx.send(ScanEvent::Progress("Scanning for large files (>500 MB)...".into()));
        let files = find_large_files(&home);
        let sizes = du_batch(&files);
        for f in &files {
            let sz = *sizes.get(f.as_str()).unwrap_or(&0);
            let label = f.strip_prefix(&format!("{home}/")).unwrap_or(f).to_string();
            let _ = tx.send(ScanEvent::Entry(entry(next_id, "Large Files", &label, f, sz)));
            next_id += 1;
        }
    });

    time_section!("Orphans", {
        crate::scanning::orphan::scan_orphans(&tx, &mut next_id);
    });

    let _total = _scan_start.elapsed();
    let _ = tx.send(ScanEvent::Done);
}