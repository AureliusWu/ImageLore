# ImageLore v0.10 Architecture

## Product boundary

ImageLore is not a general DAM and not a generation engine. It is the local memory layer between generated visual files and the context needed to find, understand, compare and continue them.

## Data model

```text
Asset (physical image identity)
├── PromptState (current editable generation text)
├── Tags[]
├── Revisions[]
├── Relations[]  <─> other Assets
├── Collections[]
├── Generation metadata JSON
└── Thumbnail cache
```

The database stores metadata and paths, not image blobs.

## Storage

```text
%LOCALAPPDATA%/ImageLore/
├── library.sqlite3
└── cache/
    └── thumbnails/
```

v0.10 intentionally does not open any pre-v0.10 database.

## Search

`asset_search` is an FTS5 virtual table. ImageLore rebuilds an asset's search row whenever its Prompt, Model, name or tags change.

## UI boundary

React never receives arbitrary filesystem capability. It invokes explicit Tauri commands. Rust owns SQLite, image decoding, fingerprints, metadata extraction, thumbnail caching and filesystem operations.
