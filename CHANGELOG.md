# Changelog

## 0.26.0 — Stability Acceptance

- Added version-aware Windows desktop shortcuts backed by the actual EXE version, safe ownership checks, cross-process serialization, downgrade protection and local runtime rollback.
- Added transactional schema 1–11 migration fixtures tied to historical commits and interruption/retry checks.
- Hardened Backup/Restore identity and relation validation, preserved DB/WAL/SHM before probing, added recoverable file reservation and tested failure stages.
- Added startup, timer and focus backup checks with serialized 24-hour decisions and visible failures.
- Fixed editor dirty detection, failed-save propagation, close protection and late Vision updates across asset changes.
- Kept in-flight gallery pagination valid during rapid image selection, while filter refreshes still reject old pages and newer selections or edits survive delayed refreshes.
- Made Sidecar writes atomic and surfaced malformed Sidecars, source discovery/import failures and failed scan timestamp commits.
- Excluded stale or missing semantic embeddings, removed unnecessary search joins and retained CJK substring verification.
- Added file-backed 50k/75k native benchmarks with 512-dimensional synthetic vectors and known-answer checks; acceptance records separate these from real-model and desktop evidence.


## 0.25.1 — Codebase Hygiene

- Added a repository-wide formatting gate with Biome for frontend/browser source and `cargo fmt --check` for Rust.
- Added strict TypeScript linting through `tsc --noEmit` plus native `cargo clippy -D warnings`; both now run as release-quality CI gates.
- Reformatted the frontend and Rust codebase for readable diffs, stable indentation and consistent line wrapping.
- Split browser preview mocks out of the production Tauri API bridge, reducing `src/api.ts` to the typed desktop boundary.
- Extracted selected-asset context and Vision workflows from `App.tsx` into dedicated hooks.
- Extracted native filesystem commands and library search-query construction from oversized Rust modules.
- Rewrote the architecture and Windows installer documentation for the current v0.25 data model, storage layout, main-only branch model and explicit installer trigger.
- Hardened project-integrity checks so source formatting cannot break structural assertions.
- Kept Windows installer generation opt-in; this maintenance release does not build an EXE automatically.

## 0.25.0 — Save to ImageLore

- Added a Chrome / Edge Manifest V3 companion extension with image context-menu actions for normal references and Remix references.
- Browser captures save the image plus a same-name `.imagelore.json` Sidecar into `Downloads/ImageLore Inbox`, preserving source image URL, page URL, page title, capture time and capture intent.
- Added Migration v11 with persistent `reference_sources` so one local asset can retain multiple web origins without duplicating the image record.
- Reference provenance now participates in standard FTS, CJK trigram candidate search and final substring verification.
- Existing SHA-256 duplicate handling merges newly captured web origins into the existing asset.
- Added Reference Cards in the Inspector with page/original-image links and capture context.
- Added a Library Manager action to prepare the browser Inbox as a Source Folder; the Inbox is scanned at startup and when ImageLore regains focus instead of using a permanent filesystem watcher.
- Sidecar v3 export now round-trips web reference provenance.
- Added browser-extension regression tests plus v11 migration, fresh-schema, source-merge and command-contract coverage.

## 0.24.0 — Remix Workspace

- Added Migration v10 with persistent Remix drafts and provenance-aware reference sources.
- Added a dedicated Remix Inspector workspace that starts from the current image and supports multiple library reference images.
- Each source can contribute selected Visual DNA dimensions such as subject, outfit, pose, composition, camera, lighting, environment, palette, material and style.
- Remix Prompt composition records where borrowed traits came from, deduplicates repeated DNA values and remains fully editable before saving.
- Remix drafts are stored independently and never modify the source image Prompt.
- Importing a Remix result records the base image as `derived_from` and additional references as `reference`, with relation notes listing the DNA fields actually used.
- Existing metadata Prompt on imported result images is preserved; the Remix Prompt is only used when the result has no Prompt.
- Remix results inherit the base image Generation Session when available.
- Remix source storage reserves source URL and reference metadata fields for the upcoming browser Save to ImageLore workflow.
- Added Node and Rust regression coverage for prompt composition, v10 migration, multi-source persistence, field filtering, Prompt preservation and lineage writes.

## 0.23.0 — Image to Prompt

- Added Migration v9 with persistent Vision Provider settings and auditable Image-to-Prompt analysis records.
- Added an OpenAI-compatible vision workflow: the selected image is resized locally to a maximum edge of about 1600 px, sent only on explicit Analyze action, and parsed into structured Visual DNA plus a generation-ready Prompt.
- Vision Base URL and model are persisted locally; API keys remain in process memory only, with optional IMAGELORE_VISION_API_KEY environment-variable loading. Keys are never written to SQLite, Sidecars or diagnostic logs.
- Added an Image to Prompt review workspace inside the Visual DNA Inspector, showing provider, model, analysis time, summary and generated Prompt before any library content is changed.
- Applying AI Visual DNA preserves existing non-empty fields by default; explicit overwrite requires confirmation and mixed manual/AI records are labeled accordingly.
- Generated Prompts can be loaded into the normal Prompt editor or stored as a separate Prompt Revision without replacing the current Prompt.
- Added Library Manager configuration for OpenAI-compatible Vision endpoints and session-only API keys.
- Added regression coverage for JSON response parsing, endpoint normalization, safe DNA merging, Migration v9, fresh schema and frontend/native command contracts.

## 0.22.0 — Visual DNA Foundation

- Added Migration v8 with structured Visual DNA for subject, character, outfit, pose, expression, composition, camera, lighting, environment, palette, material and style.
- Added a dedicated Visual DNA Inspector tab plus a compact Prompt Card that summarizes model, Prompt, size, aspect ratio, lineage and the most important visual traits.
- Visual DNA is editable, locally searchable through both FTS and CJK trigram paths, and participates in normal asset reindexing.
- Added Visual DNA round-trip support to ImageLore Sidecar v3; imported Sidecars can restore structured visual context without overwriting an existing non-empty record.
- Added startup database recovery: if an existing library cannot be initialized, ImageLore preserves forensic copies of the damaged DB/WAL/SHM and attempts the newest valid backup instead of silently creating an empty library.
- Added bounded local diagnostics with 1 MB × 5 rotating logs, startup/recovery events and Library Manager shortcuts for data and log folders.
- Added recovery, log rotation, Visual DNA normalization/search, schema and Sidecar regression coverage.

## 0.21.1 — Preview Regression Hardening

- Kept the current preview visible while Fit / 100% / wheel-zoom requests load, eliminating the temporary empty-import state during mode switches.
- Split preview-image loading from lineage and Generation Session loading so zoom changes no longer re-fetch record relationships.
- Fixed zero-delta wheel events so horizontal or inertial events cannot accidentally zoom out.
- Centralized preview workflow math for zoom limits, navigation bounds, context-menu placement and Save As extension selection.
- Hardened Save As with canonical same-file detection and supported-image extension fallback.
- Added Node behavior regressions for zoom, wheel steps, navigation edges, context-menu bounds and Save As extensions.
- Added Rust unit regressions for Save As byte copying, same-path rejection and missing-source rejection.
- Wired preview workflow regressions into the standard Fast CI check.

## 0.21.0 — Preview Workflow

- Expanded image context actions with open, reveal in folder, copy image, copy path, Save As, favorite and visual-similarity search.
- Added a native copy-to-destination command so Save As preserves the original library asset while creating an explicit user-selected copy.
- Added preview wheel zoom from 25% to 400%, drag-to-pan, double-click Fit/100%, zoom controls and current zoom feedback.
- Unified thumbnail and main-preview context menus behind a shared `AssetContextMenu`, keeping actions and missing-file behavior consistent.
- Added fast browsing shortcuts: Left/Right and J/K navigate the visible result set, while F toggles favorite without firing inside text-entry controls or dialogs.
- Made the Inspector responsive to its own pane width with container queries, improved narrow-layout wrapping and full-name hover disclosure.
- Reworked the preview footer into a compact quick-action bar for favorite, path/image copy, similarity search, file reveal and external open.

## 0.20.0 — Windows Upgrade Reliability

- Kept the Windows NSIS install identity stable (`ImageLore`, current-user mode, same Start Menu folder) so a newer installer replaces the existing installation instead of creating a side-by-side copy.
- Disabled Windows installer downgrades to prevent an older setup EXE from replacing a newer installation.
- Moved persistent data to `%LOCALAPPDATA%\\app.imagelore.desktop`, separating the library database, backups, semantic models and caches from the executable install directory.
- Added first-launch migration from the legacy `%LOCALAPPDATA%\\ImageLore` data layout; the database/WAL files, backups and local models are preserved, while derived preview cache is rebuilt.
- Updated the local asset-protocol scope to the new cache location.
- Added a Windows release smoke test that builds v0.19.0 and the current installer, performs a real silent in-place upgrade, then verifies one installed-app entry, the same install location, the new executable version, shortcut target, and preservation/migration of a seeded library.
- Windows release artifacts are uploaded only after the upgrade smoke test passes.

## 0.19.0 — Search Scale

- Added Migration v7 with a local FTS5 trigram candidate index for CJK-heavy libraries.
- CJK search terms with 3 or more characters now use the trigram index to narrow candidates before the existing substring checks verify the final result, preserving prior search semantics.
- One- and two-character CJK terms keep the existing substring fallback so short-query behavior remains unchanged.
- Prompt, Negative Prompt, model, tags and relocated file names keep the trigram index synchronized through the existing asset reindex path.
- Asset deletion now removes both classic FTS and CJK trigram search rows.
- Extended fresh-schema, migration and 50,000-record performance coverage with an indexed CJK search smoke test.


## 0.18.1 — Editor & Semantic Security Hardening

- Added a gallery right-click context menu to locate the selected image in the system file manager.
- Fixed Prompt Revision saving so the current Prompt, Negative Prompt, model and tags are committed atomically with the revision instead of racing the autosave queue.
- Pinned the CLIP ViT-B/32 vision and text components to immutable Qdrant commit revisions.
- Added exact ONNX byte-size and SHA-256 trust anchors; mismatched model files are rejected before ONNX Runtime loads them.
- Added bounded JSON validation for tokenizer / preprocessor support files, staged installation, atomic promotion into a trusted cache, and cleanup of temporary download caches.
- Added native regression tests for SHA-256 verification and rejection of untrusted model bytes.


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
