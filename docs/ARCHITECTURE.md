# ImageLore Architecture

> Current architecture baseline: v0.26.x (database schema 11)

## Product boundary

ImageLore is a local-first memory and provenance layer for generated images. It is not a general DAM and it does not generate images itself. Its job is to preserve the context needed to find, understand, compare, revise and continue an image across tools.

## Runtime layers

```text
React + TypeScript UI
        │ explicit typed API
        ▼
Tauri command boundary
        │
        ├── library / prompt / lineage commands
        ├── import and source-sync jobs
        ├── Visual DNA / Vision / Remix / Reference services
        ├── semantic indexing and search
        └── diagnostics / backup / file operations
        │
        ▼
Rust domain modules
        │
        ├── SQLite metadata + indexes
        ├── filesystem assets and preview cache
        └── local semantic models
```

React never receives unrestricted filesystem access. Desktop-only operations cross explicit Tauri commands. Browser preview mode uses an isolated mock adapter and does not share production persistence.

## Core data model

```text
Asset
├── PromptState
├── Tags[]
├── PromptRevisions[]
├── Relations[] <─> other Assets
├── Collections[]
├── Generation metadata + structured GenerationIndex
├── GenerationSession membership
├── VisualDNA
├── ImagePromptAnalysis[]
├── RemixDraft[]
│   └── RemixSources[] + selected DNA fields
├── ReferenceSources[]
└── SemanticEmbedding
```

Sidecar data preserves portable metadata and provenance for round-tripping. Physical image bytes stay on disk; SQLite stores paths, metadata, relationships and derived indexes.

## Storage

Current persistent data root:

```text
%LOCALAPPDATA%/app.imagelore.desktop/
├── library.sqlite3
├── library.sqlite3-wal / -shm
├── backups/
├── cache/
├── logs/
└── models/
```

The legacy `%LOCALAPPDATA%/ImageLore/` layout is migrated forward on startup. Preview cache and semantic indexes are derived data and can be rebuilt; the library database, backups and model cache are treated as persistent user data.

Startup integrity probing copies DB/WAL into an isolated probe and rebuilds SHM there, because even a read-only WAL connection can change the source SHM. Only confirmed corruption permits automatic replacement. Restore validates supported ImageLore schema and business relations, preserves raw DB/WAL/SHM originals, reserves the old files beside the database and uses an interrupted-transaction marker for rollback. Migrations run inside a transaction and retain schema 11.

Backup decisions, validation, snapshot creation and rotation share a dedicated operation mutex. Runtime paths acquire it before the database mutex; only `VACUUM INTO` holds the live connection, so validating a recent backup does not block ordinary library reads. Automatic checks run in the blocking task pool, retain full validation and serialize concurrent requests.

Preview cache keys use the source file's nanosecond modification time and size alongside its library fingerprint. Non-thumbnail Fit requests that do not resize the actual source pixels share the full preview resource; large-image resizing and thumbnail caches remain separate. Older derived cache files remain subject to the existing soft size budget.

Native acceptance can select an isolated absolute directory using `IMAGELORE_TEST_DATA_DIR`; it must contain `.imagelore-acceptance-root` with the text `ImageLore acceptance fixture`. Missing or invalid markers stop startup. This path bypasses legacy migration; the asset protocol adds only its managed cache directory to the existing scope.

## Import and source synchronization

All imports converge on the same importer and fingerprint rules:

- direct image import;
- folder import and drag/drop;
- persistent source folders;
- browser-captured ImageLore Inbox references.

SHA-256 fingerprints prevent duplicate physical content from creating duplicate asset records. Source synchronization is explicit/background-job based rather than a permanent filesystem watcher.

## Search

ImageLore has two complementary search paths:

- **Keyword search** uses FTS plus a CJK trigram candidate index, then preserves substring semantics for CJK queries.
- **Semantic search** uses local CLIP-compatible text/image embeddings for natural-language recall and similar-image search.

Structured generation filters such as seed, steps, sampler, scheduler, CFG, denoise and orientation are composed with library search rather than replacing it.

## Provenance workflows

Relations, Generation Sessions, Branch Notes, Remix sources and Reference sources are first-class provenance rather than display-only metadata. A Remix result records its base image, additional references and the exact Visual DNA fields reused from each source.

Vision analysis is auditable: Image-to-Prompt output is stored separately and only changes Visual DNA or prompt history through explicit user actions.

## Background jobs and reliability

Import and semantic indexing share cancellable job infrastructure. Database migrations are versioned and regression-tested. Startup recovery can restore the latest valid backup, while diagnostics use bounded rotating logs.

Windows upgrades keep a stable application identity and migrate legacy local data without coupling user data to the installed program directory.

## Repository and release model

`main` is the only long-lived development branch. Fast frontend/project checks and native Rust checks run from `main`.

Windows packaging is deliberately separate from ordinary development pushes. It is triggered only by an explicit workflow dispatch, a `v*` release tag, or a deliberate update to `.github/windows-build-request.txt`.

## Code-quality invariants

- TypeScript uses `strict` mode and must pass `tsc --noEmit`.
- Frontend source is formatted by Biome with a 100-column target.
- Rust must pass `cargo fmt --check`, `cargo check`, `cargo clippy -D warnings` and library tests.
- `VERSION` is the canonical application version source.
- Database `schema_version` is independent from application SemVer.
- Windows installer generation is never part of the normal development loop.
