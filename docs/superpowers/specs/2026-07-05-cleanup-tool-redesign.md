# Mac Cleaner — Complete Redesign

## Overview

Refactor the monolithic 1853-line `main.rs` egui application into a modular,
production-quality architecture with clear separation of concerns: panels
(views), widgets (reusable components), scanning logic, and utilities. with gruvbox dark theme

## Module Structure

```
src/
├── main.rs                 # Entry point (~5 lines)
├── app.rs                  # Application state + event loop
├── theme.rs                # Colors, spacing, typography tokens
├── models.rs               # Data types (CleanupEntry, InstalledApp, etc.)
│
├── panels/
│   ├── mod.rs
│   ├── sidebar.rs          # Navigation sidebar
│   ├── dashboard.rs        # Overview / storage summary
│   ├── cleanup.rs          # Scan results + selection
│   ├── applications.rs     # App uninstaller view
│   ├── large_files.rs      # Large file review
│   └── header.rs           # Top bar with disk usage
│
├── widgets/
│   ├── mod.rs
│   ├── storage_card.rs     # Reusable metric card
│   ├── cleanup_card.rs     # Cleanup action card
│   ├── toast.rs            # Toast notifications
│   ├── search_bar.rs       # Filter/search bar
│   └── danger_dialog.rs    # Confirmation dialog
│
├── scanning/
│   ├── mod.rs
│   ├── scanner.rs          # Main scan orchestrator
│   ├── orphan.rs           # Orphan detection
│   └── deletion.rs         # Delete pipeline
│
└── util/
    ├── mod.rs
    └── format.rs           # format_size, du_sh, run_cmd, etc.
```

## Layout

```
┌────────────────────────────────────────────────────────┐
│ Header: Disk usage bar + scan status + view switcher   │
├──────────┬─────────────────────────────────────────────┤
│ Sidebar  │  Main Content Area                          │
│          │                                             │
│ Dashboard│  (varies by selected view)                  │
│ Cleanup  │                                             │
│ Apps     │                                             │
│ Large    │                                             │
│ Files    │                                             │
│          │                                             │
│ Settings │                                             │
├──────────┴─────────────────────────────────────────────┤
│ Footer: totals, quick-actions, toast, clean button     │
└────────────────────────────────────────────────────────┘
```

Three-zone panel layout: top header, sidebar + central panel, bottom footer.

## Views (Panels)

### Dashboard
- Storage overview card (total / used / available with progress bar)
- Recoverable space summary by category
- Quick-action buttons: "Scan Now", "Clean Recommended"
- Auto-scan on first launch

### Cleanup
- Search/filter bar (filters in real-time)
- Collapsible sections sorted by total size (descending)
- Per-item checkboxes with size + path
- Section-level "select all" checkboxes
- Orphan confidence color-coding (High = red, Medium = amber, Low = gray)
- Quick-select actions in footer: select all filtered, select ≥100 MB, select high-confidence orphans, clear
- Empty state when no items match filters
- Scanning state with progress spinner

### Applications
- Searchable list of installed apps (name, bundle ID, size)
- Per-app "Find traces" button
- Traces view: app bundle + matched Library files
- Select all / deselect all for traces
- Uninstall confirmation reuses the danger dialog

### Large Files
- Files >500 MB found under home
- Sortable by size, path
- Same selection/cleanup flow as Cleanup

## Reusable Widgets

| Widget | Description |
|---|---|
| `StorageCard` | A metric card showing a label, value, optional color accent |
| `CleanupCard` | Section card with header, items, select-all |
| `Toast` | Auto-dismissing notification (success green / error red) |
| `SearchBar` | Text input with clear button, optional min-size slider |
| `DangerDialog` | Confirmation dialog with item list, dry-run mode, space preview |

## Theme (`theme.rs`)

Define color constants and spacing values:

```rust
pub struct Theme {
    pub sidebar_bg: egui::Color32,
    pub content_bg: egui::Color32,
    pub accent: egui::Color32,
    pub success: egui::Color32,
    pub error: egui::Color32,
    pub text_primary: egui::Color32,
    pub text_secondary: egui::Color32,
    pub text_muted: egui::Color32,
    pub card_bg: egui::Color32,
    pub border: egui::Color32,
}
```

Initial theme: dark mode with purple accent (#6C5CE7).

## State Management (`app.rs`)

```rust
pub struct AppState {
    pub disk: DiskSpace,
    pub view: AppView,
    pub entries: Vec<CleanupEntry>,
    pub scanning: ScanState,
    pub scan_rx: Option<mpsc::Receiver<ScanEvent>>,
    pub deletion_rx: Option<mpsc::Receiver<DeletionResult>>,
    pub deleting: bool,
    pub installed_apps: Vec<InstalledApp>,
    pub app_traces: Vec<CleanupEntry>,
    pub uninstall_target: Option<usize>,
    pub apps_scan_rx: Option<mpsc::Receiver<AppScanEvent>>,
    pub apps_scanning: bool,
    pub search: String,
    pub min_size_mb: f32,
    pub app_search: String,
    pub show_confirm: bool,
    pub pending_cleanup: Vec<CleanupEntry>,
    pub dry_run: bool,
    pub toast: Option<ToastMessage>,
    pub log_messages: Vec<String>,
    pub show_log: bool,
}
```

Event loop: `update(ctx, _frame)` calls:
1. Drain channels (scan, deletion, app scan)
2. Toast timer
3. Render header panel
4. Render sidebar panel
5. Render central panel (matches current `AppView`)
6. Render footer panel
7. Render dialogs (confirmation, log)

## Data Flow

```
User clicks "Scan" 
  → app.rs starts scan thread
  → scan thread sends ScanEvent::Entry via channel
  → app drains channel, appends to entries[]
  → UI re-renders with new data

User selects items + clicks "Clean"
  → app collects selected entries
  → shows DangerDialog for confirmation
  → on confirm: spawns deletion thread
  → deletion thread sends DeletionResult via channel
  → app drains channel, removes deleted entries, refreshes disk
```

## Migration Strategy

Generated in full (all files written at once). No incremental migration needed —
the existing `main.rs` is replaced by the new module structure. All existing
features preserved:
- System caches, build tools, package managers, project deps, orphans
- Docker scanning via `docker system df --format json`
- App uninstaller with trace detection
- Dry-run mode, trash-first deletion, `rm -rf` fallback
- Toast notifications, log panel, search/filter, min-size slider
- Post-cleanup disk refresh
