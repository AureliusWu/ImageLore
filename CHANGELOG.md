# Changelog

## 0.18.0 — Semantic Recall

- Added Migration v6 with a rebuildable local semantic embedding index keyed by asset fingerprint and embedding model.
- Added local CLIP ViT-B/32 image and text encoders through FastEmbed for semantic text-to-image retrieval and image-to-image similarity.
- Added keyword / semantic search modes, semantic similarity ranking, and an Inspector action to find images visually similar to the current asset.
- Combined semantic retrieval with the existing Generation Explorer filters so model, collection, metadata source, orientation and generation parameters can narrow semantic results.
- Added an AI Index manager with coverage, stale-item count, model/index disk usage, incremental rebuild progress, cancellation, index clearing and model-cache removal.
- Semantic indexing is incremental: unchanged fingerprints are reused, while newly imported or externally changed images are re-embedded when semantic indexing is enabled.
- Generalized the background job registry so import/source-sync and semantic indexing share consistent job IDs and cancellation primitives.
- Added race-safe semantic progress handling and close-time cancellation that waits for active background work to finish.
- Added a 50,000-record semantic linear-scan smoke test alongside existing FTS and generation-parameter performance coverage.
- Semantic embeddings remain derived, disposable data; original images, metadata, Sidecars and user-authored library state remain authoritative.

## 0.17.0 — Generation Explorer

- Added Migration v5 with a rebuildable structured generation index for Seed, Steps, Sampler, Scheduler, CFG and Denoise.
- Existing libraries backfill the generation index automatically; imports, source sync and metadata rescans keep it updated.
- Added advanced library filters for generation parameters, metadata source and image orientation.
- Added explicit library sorting by update/import time, file name, resolution and file size.
- Upgraded Saved Views to preserve advanced filters and sorting while remaining compatible with existing saved filters.
- Added active filter chips and batch actions for favorite state, metadata rescan and Generation Session assignment.
- Raised the synthetic search smoke test to 50,000 records with generation-parameter queries.
- Hardened corrupted workspace layout recovery and prevented stale thumbnail promises from writing after unmount.
- Close-time import cancellation now waits for the import completion event before destroying the window.
- Reworked duplicate-group retrieval from N+1 queries to a single grouped query.


## 0.16.0 — Source Sync

- Hardened version metadata so `VERSION` automatically drives npm, Tauri, Cargo, runtime, README, UI preview and OG SVG; CI now rejects drift or a non-idempotent sync.

- Added persistent Source Folders for frequently used AI output and image directories.
- Source folders can be synchronized individually or together through the existing background import job with progress and cancellation.
- Added per-folder “sync on startup” control; startup performs one scan only and does not install a resident filesystem watcher.
- Added Library Manager controls for adding, removing and synchronizing source folders.
- Added Migration v4 and regression coverage for Source Folder persistence.
- Reused unchanged-file fast paths and duplicate detection so repeated syncs skip already indexed files efficiently.



## 0.15.2 — Showcase

- Added a 1280×640 ImageLore OG / README preview source matching the current Frutiger Aero desktop UI.
- Added a reproducible GitHub Actions renderer that produces `docs/og.png` from the SVG source with CJK fonts.
- Refreshed `docs/UI_PREVIEW.html` from the legacy v0.10 English mock to the current v0.15 Chinese interface and Generation Session concepts.
- Added friendly preview labels for NovelAI, InvokeAI, generic JSON and images without generation metadata.



## 0.15.1 — Consistency Hardening

- Fixed an autosave race where rapid edit → save → revert sequences could leave the database on an intermediate Prompt / Model / Tag state while the editor showed the reverted value.
- Prevented exact-content duplicate imports from creating misleading derivative lineage links.
- Made preview cache temporary files process-unique so concurrent preview generation cannot clobber another writer.


## 0.15.0 — Generation Intelligence

- Added Migration v3 with stable Portable IDs for generation records.
- Added Sidecar v3 with portable parent references, fingerprint fallback and import-order-independent lineage recovery.
- Exact-content duplicates can merge imported Sidecar session/lineage context into the existing record.
- Added Generation Sessions with per-session and per-asset notes; derivative imports inherit the active session.
- Added editable Branch Notes on parent and child lineage edges.
- Added side-by-side parent/current image Compare while preserving Prompt, Negative Prompt, model and tag diffs.
- Added model alias/normalization without rewriting original model metadata.
- Added saved Library filter views and Library Health diagnostics.
- Added NovelAI, InvokeAI and generic JSON metadata adapters alongside A1111 and ComfyUI.
- Added Portable ID display to reproducibility information.
- Made Migration v3 re-runnable after a partially completed schema change and added regression coverage for portable lineage recovery.


## 0.14.1 — Stability Fixes

- Fixed a race where very fast background imports could finish before the frontend learned the job ID, leaving import progress stuck.
- Buffered all early import events and made drag/drop listeners stable across React renders.
- Added close-time editor flush and import cancellation so the final Prompt/Tag edits are preserved when the window closes.
- Fixed stale async results overwriting newer image selections, previews, lineage data and Parent Picker searches.
- Fixed stale pagination results being appended after filters changed.
- Fixed hidden-item selection after importing while a Library filter is active.
- Kept active Tag filters coherent after tag rename/merge/delete.
- Fixed stale preview cache reuse when an original image is externally replaced at the same path.
- Fixed cache cleanup for records whose fingerprint is unavailable.
- Fixed CJK multi-term search so whitespace-separated Chinese/Japanese/Korean terms are matched independently with AND semantics.
- Folder import scanning now responds to cancellation while walking large directory trees.
- Removed long filesystem hashing/metadata work from the SQLite mutex in metadata rescan and missing-file relocation paths.
- Made delete, Collection membership writes, metadata rescan, missing-file updates and relocation updates atomic where multi-step writes are involved.
- Made pre-restore safety backups WAL-aware, staged restores atomic, and backup filenames collision-safe.
- A corrupt active database no longer prevents restoring a previously validated backup; startup failures now surface through a native error dialog and startup-error.log.
- Added backend validation for empty/duplicate Collection names and clearer UI error reporting for Library Manager operations.
- Added ComfyUI numeric node-ID compatibility and regression tests for A1111 / ComfyUI metadata parsing.
- Split Fast CI and Native CI so a later frontend-only commit can no longer cancel and accidentally skip Rust validation.
- Native CI now runs both cargo check and Rust unit tests.


## 0.14.0 — Library Continuity

- Replaced Base64 image IPC with scoped local WebP cache files served through Tauri asset protocol.
- Full-size preview now preserves source dimensions, with 1 GB background cache pruning and improved long-session readability.
- Added automatic daily SQLite backups, manual backup creation, rotating retention, integrity validation and restart-safe staged restore.
- Restore operations preserve a pre-restore safety copy before replacing the active library.
- Added SHA-256 content duplicate detection: exact copies imported from different paths are skipped and reported separately.
- Added a Library Manager for backup history, duplicate reports, tag rename/merge/delete and Collection rename/delete.
- Added background image/folder/drag-drop import jobs with live progress and cancellation.
- Added a metadata adapter layer and deeper ComfyUI extraction for Prompt, Negative Prompt, model, seed, steps, CFG, sampler, scheduler and denoise.
- Added a searchable full-library Parent Picker for lineage/reference relationships.
- Added database migration v2 and a partial fingerprint index for duplicate-aware library operations.
- Preserved atomic prompt/tag/revision writes, CJK search, lightweight library records and reproducible locked builds.

## 0.13.1 — Build Hygiene

- Split fast CI from Windows release packaging.
- Added concurrency cancellation so superseded branch builds stop automatically.
- Locked repository version metadata to 0.13.1 across VERSION, npm, Tauri and Cargo sources.
- Prepared reproducible npm/Cargo dependency locking and npm CI caching.
- Normalized the changelog to one document heading.
- Kept the release installer workflow focused on manual or version-tag builds.

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
- CI now runs schema, scale, command-contract and project-integrity tests before Windows release builds.

## 0.12.0 — Native Drag & Drop

- Added native drag-and-drop import across the whole ImageLore window.
- Supports dropping multiple images, folders, or a mixed selection.
- Dropped folders are scanned recursively for supported image formats.
- Added a Frutiger Aero drag target overlay with clear Chinese feedback.
- Duplicate files reuse the existing skip behavior and are not added twice.
- No database schema change; v0.11 data remains compatible.

## 0.11.0 — Chinese Desktop

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
