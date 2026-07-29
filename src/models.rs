#[derive(Clone, Copy, PartialEq)]
pub struct DiskSpace {
    pub total_bytes: u64,
    pub available_bytes: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum OrphanConfidence {
    High,
    Medium,
    Low,
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

pub fn entry(
    id: usize,
    section: &str,
    label: &str,
    path: &str,
    size_bytes: u64,
) -> CleanupEntry {
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

pub fn orphan_entry(
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

#[derive(Clone, Debug)]
pub struct InstalledApp {
    pub id: usize,
    pub name: String,
    pub bundle_id: String,
    pub app_path: String,
    pub size_bytes: u64,
}

#[allow(dead_code)]
pub enum ScanEvent {
    Entry(CleanupEntry),
    Warning(String),
    #[allow(dead_code)]
    Progress(String),
    Done,
}

#[derive(Clone, Debug)]
pub enum AppScanEvent {
    Entry(InstalledApp),
    Progress(()),
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
