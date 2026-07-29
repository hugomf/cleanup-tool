use std::collections::HashSet;
use std::process::Command;
use std::sync::mpsc;

use crate::models::*;
use crate::util::*;

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
        let mut candidates: Vec<(std::path::PathBuf, String, String)> = Vec::new();
        for dir_entry_res in dir {
            let Ok(dir_entry) = dir_entry_res else {
                continue;
            };
            let path = dir_entry.path();
            let name = match path.file_name() {
                Some(n) => n.to_string_lossy().to_string(),
                None => continue,
            };
            let nl = name.to_lowercase();

            if nl.starts_with("com.apple.")
                || nl == "com.apple"
                || nl.starts_with("apple.")
                || nl.starts_with('.')
                || nl == "caches"
                || nl == "metadata"
            {
                continue;
            }

            let is_known = known_bundle_ids.contains(&nl)
                || known_bundle_ids
                    .iter()
                    .any(|kid| nl.starts_with(&format!("{kid}.")))
                || known_app_names.contains(&nl);
            if is_known {
                continue;
            }

            candidates.push((path, name, nl));
        }

        if candidates.is_empty() {
            continue;
        }

        let paths: Vec<String> = candidates.iter().map(|(p, _, _)| p.to_string_lossy().to_string()).collect();
        let sizes = du_batch(&paths);

        for (path, name, nl) in &candidates {
            let sz = *sizes.get(path.to_string_lossy().as_ref()).unwrap_or(&0);
            if sz == 0 {
                continue;
            }

            let confidence = if nl.contains('.') && nl.split('.').count() >= 3 {
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
