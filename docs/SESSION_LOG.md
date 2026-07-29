# Session Log

## 2026-07-05: Complete Redesign — Monolithic to Modular Architecture

### Summary
Refactored the entire 1853-line `main.rs` into a 21-file modular architecture:
**5 modules** — `models`, `theme`, `util`, `scanning`, `widgets`, `panels`, `app`.

### What Changed

| Before | After |
|---|---|
| 1 file (main.rs, 1853 lines) | 21 files across 5 module directories |
| Single `update()` god function | `app.rs` orchestrates panels + widgets |
| Hardcoded colors everywhere | `theme.rs` with 11 semantic color tokens |
| Inline scan logic | `scanning/scanner.rs`, `orphan.rs`, `deletion.rs` |
| Monolithic UI | `panels/` (6 panels) + `widgets/` (4 widgets) |
| Integer spacing | `f32` spacing (egui 0.34 compat) |

### Files Created
- `src/main.rs` — 12-line entry point
- `src/app.rs` — State + event loop
- `src/models.rs` — All data types
- `src/theme.rs` — Dark theme constants
- `src/util/mod.rs` + `format.rs` — Utilities
- `src/scanning/mod.rs` + `scanner.rs`, `orphan.rs`, `deletion.rs`
- `src/widgets/mod.rs` + `storage_card.rs`, `toast.rs`, `search_bar.rs`, `danger_dialog.rs`
- `src/panels/mod.rs` + `header.rs`, `sidebar.rs`, `dashboard.rs`, `cleanup.rs`, `applications.rs`, `large_files.rs`

### Features Preserved
- All scan categories (system caches, build tools, npm, Docker, Homebrew, orphan data, large files, app traces)
- Dry-run mode + trash-first deletion with `rm -rf` fallback
- Search/filter + min-size slider
- Section collapsing + orphan confidence color-coding
- Quick-select actions
- Post-cleanup disk refresh
- Toast notifications + log panel
- App uninstaller with bundle ID trace matching

### Build
- `cargo check`: 0 errors, 12 warnings (unused imports only)
- `cargo build --release`: succeeds
- Smoke test: app launches without panic

## 2026-07-29: Scan Performance Optimization (Round 1)

### Summary
Reduced scan time by addressing three bottlenecks:
1. **Batched `du` calls** — `du_batch()` passes up to 30 paths per `du -sk` call instead of spawning one per path
2. **Parallel project dep discovery** — 10 `find` calls run in parallel via threads
3. **Optimized `find_large_files`** — `-xdev`, excluded `node_modules`, `.cargo`, `.gradle`, `Library/Caches`, `.git/objects`
4. **Orphan scan batching** — collect candidates, batch du, then emit

### Files Changed
- `src/util/format.rs` — Added `du_batch()`, optimized `find_large_files`
- `src/scanning/scanner.rs` — `emit_section!` macro, parallel project deps
- `src/scanning/orphan.rs` — Batch du for orphans

### Build
- `cargo check`: 0 errors, 12 warnings (all pre-existing)

## 2026-07-29: jwalk migration + Docker fix + Dashboard redesign

### Round 2 — jwalk migration
Replaced all shell `du`/`find` commands with the `jwalk` Rust library:
- `du_sh()`/`du_batch()` → `dir_size()` using `jwalk::WalkDir`
- `find_dirs()` → jwalk-based directory search
- `find_large_files()` → jwalk depth 5, post-filter skip dirs
- `find_stale_downloads()` → jwalk depth 2, extension + modified filter
- Kept `Command` only for Docker/Homebrew/PlistBuddy calls
- Added `jwalk = "0.8"` to `Cargo.toml`

### Docker timeout fix
- Added socket check (`/var/run/docker.sock`) before running `docker info`
- Reduces timeouts from 8s to 0s when Docker isn't running

### Dashboard improvements
- Storage card shows immediately (no more spinner blocking the dashboard)
- Two-column layout: Recoverable Space (left) + Actions (right)
- Clean Recommended gets a filled green button (primary CTA)
- Full Scan gets a subtle outline button (secondary)

### Timing instrumentation
- All scan sections wrapped with `time_section!` macro logging to stderr
- Enables easy profiling of scan bottlenecks

### Files Changed
- `src/scanning/scanner.rs` — jwalk migration, Docker socket check, timing
- `src/scanning/orphan.rs` — jwalk migration
- `src/util/format.rs` — jwalk migration (`dir_size`, `find_dirs`, `find_large_files`, `find_stale_downloads`)
- `Cargo.toml` — added `jwalk = "0.8"`
- `src/panels/dashboard.rs` — two-column layout, redesigned buttons

### Build
- `cargo check`: 0 errors, 12 warnings (all pre-existing)
- `cargo build --release`: succeeds