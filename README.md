# 🧹 Cleanup Tool

Interactive macOS cleanup utility with GUI. Scans for cache files, build artifacts, and dependency directories across your system, letting you pick exactly what to remove.

## Quick Start

```zsh
cd ~/Projects/cleanup-tool
cargo build --release
./target/release/cleanup-tool
```

> ⚠️ Always review the confirmation dialog before deleting — it shows every path that will be removed.

## Features

- **System caches**: User cache, Xcode DerivedData, Gradle, Cargo, Go, npm, pip, Docker
- **Project dependencies**: `node_modules`, `target`, `build`, `.dart_tool`, `Pods`, `.gradle`, `.m2`
- **Logs & temp**: System logs, `/tmp`, stale downloads
- **Per-item selection**: Check individual directories within each section
- **Confirmation popup**: Full path list shown before any deletion
- **Section summaries**: See total reclaimable space per category

## Safety

- All paths are listed with sizes before deletion
- Confirmation dialog shows every single path that will be removed
- You must explicitly click "Delete" to proceed
- Can be extended to move to Trash instead of `rm -rf`
