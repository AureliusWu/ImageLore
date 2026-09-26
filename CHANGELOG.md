# Changelog

## 0.13.0 — Reliability & Flow

- Prevents unsaved Prompt/Negative Prompt/Model/Tags edits from being lost when switching records.
- Ctrl+S flushes the editor before creating a Prompt revision.
- Library pages now transfer lightweight AssetSummary records; full Prompt/JSON is loaded only for the active image.
- Import scanning, metadata parsing and SHA-256 work no longer hold the SQLite mutex for the whole batch.
- Consolidated image/file/folder import into one native importer module.
- Prompt updates, tag writes and revision restores use atomic SQLite transactions.
- Added schema migration foundation without resetting schema_version on startup.
- Added CJK substring search fallback for Chinese/Japanese/Korean queries.
- Split editor, drag-drop, workspace layout and dialogs into focused modules.
- CI now runs schema, scale, command-contract and project-integrity tests before the Windows build.

# Changelog

## 0.12.0

- Added native drag-and-drop import across the whole ImageLore window.
- Supports dropping multiple images, folders, or a mixed selection.
- Dropped folders are scanned recursively for supported image formats.
- Added a Frutiger Aero drag target overlay with clear Chinese feedback.
- Duplicate files reuse the existing skip behavior and are not added twice.
- No database schema change; v0.11 data remains compatible.

# Changelog

## 0.11.0

- Windows release binary now uses the GUI subsystem, so launching ImageLore no longer opens a console window.
- Main application UI is now Simplified Chinese while keeping the ImageLore brand name.
- Localized search, library filters, Prompt editor, generation info, lineage, compare, dialogs, status messages and empty states.
- No database schema change; existing v0.10 data remains compatible.

## 0.10.0 — Clean Core

### Breaking / reset
- Removed all PromptDock migration behavior.
- Removed pre-v0.10 `imagelore.db` migration behavior.
- New clean database contract: `library.sqlite3`.
- Sidecar contract reset to `imagelore.sidecar.v2` only.

### Architecture
- Split the Rust backend into database, metadata, preview, commands, models and state modules.
- Replaced the monolithic image record table with explicit assets, prompt state, normalized tags, revisions, relations and collections.
- Added SQLite FTS5 search index for filename, Prompt, Negative Prompt, Model and Tags.
- Enabled WAL mode for better desktop read/write behavior.
- Kept persistent on-disk WebP thumbnail cache.

### Library UX
- Added first-class Collection / Tag / Model filters.
- Kept paged loading and virtualized image cards for large libraries.
- Kept explicit Frutiger Aero inspired light workspace with clear text buttons.
- Simplified pane responsibilities: Library / Preview / Inspector.

### Data safety
- Original image files remain external and are never deleted by record removal.
- `CLEAN_LEGACY_DATA.bat` performs optional cleanup of prototype-only data and leaves the v0.10 database untouched.
