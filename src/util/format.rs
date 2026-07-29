use std::collections::HashMap;
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::SystemTime;

use crate::models::DiskSpace;

pub fn format_size(bytes: u64) -> String {
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

pub fn section_icon(section: &str) -> &'static str {
    match section {
        "System Caches" => "🧹",
        "Build Tools" => "🔧",
        "Go" => "🐹",
        "Python" => "🐍",
        "Package Managers" => "📦",
        "Project Deps" => "📁",
        "Logs & Temp" => "🪵",
        "iOS" => "🍎",
        "Downloads" => "⬇️",
        "Large Files" => "🐘",
        "Orphan App Data" => "🧩",
        "App Bundle" => "📱",
        "App Traces" => "🧬",
        _ => "📁",
    }
}

pub fn run_cmd_timeout(
    program: &str,
    args: &[&str],
    secs: u64,
) -> Option<std::process::Output> {
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

fn dir_size(path: &str) -> u64 {
    jwalk::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok().map(|m| m.len()))
        .sum()
}

pub fn du_batch(paths: &[String]) -> HashMap<String, u64> {
    paths.iter().map(|p| (p.clone(), dir_size(p))).collect()
}

pub fn du_sh(path: &str) -> u64 {
    dir_size(path)
}

pub fn find_dirs(path: &str, name: &str, maxdepth: u32) -> Vec<String> {
    jwalk::WalkDir::new(path)
        .max_depth(maxdepth as usize)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_dir())
        .filter(|e| e.file_name() == name)
        .map(|e| e.path().to_string_lossy().to_string())
        .collect()
}

pub fn parse_size_str(s: &str) -> u64 {
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
        DiskSpace {
            total_bytes: 0,
            available_bytes: 0,
        }
    }
}

const STALE_EXTENSIONS: &[&str] = &[".dmg", ".pkg", ".zip", ".tgz", ".iso", ".tar.gz", ".tar.bz2"];

pub fn find_stale_downloads(downloads: &str) -> Option<Vec<String>> {
    let thirty_days = std::time::Duration::from_secs(30 * 24 * 60 * 60);

    let results: Vec<String> = jwalk::WalkDir::new(downloads)
        .max_depth(2)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_lowercase();
            STALE_EXTENSIONS.iter().any(|ext| name.ends_with(ext))
        })
        .filter(|e| {
            e.metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| SystemTime::now().duration_since(t).ok())
                .map(|d| d > thirty_days)
                .unwrap_or(false)
        })
        .map(|e| e.path().to_string_lossy().to_string())
        .collect();

    if results.is_empty() { None } else { Some(results) }
}

const SKIP_DIRS: &[&str] = &[
    ".Trash",
    "CoreSimulator",
    "Caches",
    "node_modules",
    "registry",
    "objects",
];

pub fn find_large_files(home: &str) -> Vec<String> {
    jwalk::WalkDir::new(home)
        .max_depth(5)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            if e.file_type().is_dir() {
                let name = e.file_name().to_string_lossy();
                return !SKIP_DIRS.iter().any(|skip| name.as_ref() == *skip);
            }
            true
        })
        .filter(|e| e.file_type().is_file() && e.metadata().ok().map_or(false, |m| m.len() > 500 * 1024 * 1024))
        .map(|e| e.path().to_string_lossy().to_string())
        .collect()
}
