use std::path::Path;
use std::process::Command;
use std::sync::mpsc;
use std::thread;

use crate::models::*;
use crate::util::du_sh;

pub fn execute_cleanup(
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
                        let _ =
                            result_tx.send(DeletionResult::Deleted(item.path.clone(), item.id));
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

            let deleted = try_trash(&item.path).or_else(|| try_rm_rf(&item.path));
            match deleted {
                Some(true) => {
                    let _ =
                        result_tx.send(DeletionResult::Deleted(item.path.clone(), item.id));
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

pub fn scan_installed_apps(tx: &mpsc::Sender<AppScanEvent>) {
    let _ = tx.send(AppScanEvent::Progress(()));
    let home = std::env::var("HOME").unwrap_or_default();
    let app_dirs = [
        "/Applications".to_string(),
        format!("{home}/Applications"),
        "/System/Applications".to_string(),
    ];

    let mut next_id = 0usize;
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
            if !Path::new(&plist).exists() {
                continue;
            }
            let bundle_id = Command::new("/usr/libexec/PlistBuddy")
                .args(["-c", "Print :CFBundleIdentifier", &plist])
                .output()
                .ok()
                .map(|b| String::from_utf8_lossy(&b.stdout).trim().to_string())
                .unwrap_or_default();
            let name = line
                .trim_end_matches(".app")
                .split('/')
                .last()
                .unwrap_or(line)
                .to_string();
            let sz = du_sh(line);

            let _ = tx.send(AppScanEvent::Entry(InstalledApp {
                id: next_id,
                name,
                bundle_id,
                app_path: line.to_string(),
                size_bytes: sz,
            }));
            next_id += 1;
        }
    }
    let _ = tx.send(AppScanEvent::Done);
}

const APP_TRACE_DIRS: &[&str] = &[
    "Library/Application Support",
    "Library/Preferences",
    "Library/Saved Application State",
    "Library/Caches",
    "Library/Containers",
    "Library/Group Containers",
    "Library/WebKit",
    "Library/Application Scripts",
    "Library/HTTPStorages",
    "Library/LaunchAgents",
    "Library/Logs",
];

pub fn find_app_traces(app: &InstalledApp) -> Vec<CleanupEntry> {
    let mut out = Vec::new();
    let mut local_id = 0usize;

    out.push(entry(
        APP_TRACE_ID_OFFSET + local_id,
        "App Bundle",
        &format!("{}.app", app.name),
        &app.app_path,
        app.size_bytes,
    ));
    local_id += 1;

    let home = std::env::var("HOME").unwrap_or_default();
    let bundle_lc = app.bundle_id.to_lowercase();
    let name_lc = app.name.to_lowercase();
    let name_nospace = name_lc.replace(' ', "");

    for subdir in APP_TRACE_DIRS {
        let scan_path = format!("{home}/{subdir}");
        let Ok(dir) = std::fs::read_dir(&scan_path) else {
            continue;
        };
        for dir_entry_res in dir {
            let Ok(dir_entry) = dir_entry_res else {
                continue;
            };
            let path = dir_entry.path();
            let Some(fname) = path.file_name().map(|f| f.to_string_lossy().to_string()) else {
                continue;
            };
            let fl = fname.to_lowercase();
            let fl_stem = fl.trim_end_matches(".plist");

            let is_match = (!bundle_lc.is_empty()
                && (fl == bundle_lc
                    || fl_stem == bundle_lc
                    || fl.starts_with(&format!("{bundle_lc}."))))
                || fl == name_lc
                || fl_stem == name_lc
                || fl == name_nospace
                || fl_stem == name_nospace;

            if !is_match {
                continue;
            }

            let sz = du_sh(&path.to_string_lossy());
            if sz == 0 {
                continue;
            }

            out.push(entry(
                APP_TRACE_ID_OFFSET + local_id,
                "App Traces",
                &format!("{subdir}/{fname}"),
                &path.to_string_lossy(),
                sz,
            ));
            local_id += 1;
        }
    }

    out
}
