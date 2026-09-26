# Changelog

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
