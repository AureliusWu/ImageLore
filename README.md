# ImageLore v0.11.0

ImageLore is a local-first desktop library for AI-generated images and the context behind them: prompts, generation metadata, revisions, collections, and parent/derivative relationships.

## Why v0.10 is a clean rebuild

The pre-v0.10 builds were prototypes and were never used as a real library. v0.10 deliberately removes legacy migration code and starts with a new data model.

- New database: `%LOCALAPPDATA%\ImageLore\library.sqlite3`
- New thumbnail cache: `%LOCALAPPDATA%\ImageLore\cache\thumbnails\`
- Old `PromptDock` databases are not read.
- Old `ImageLore\imagelore.db` is not read.
- Only `.imagelore.json` Sidecar v2 is part of the new contract.
- `CLEAN_LEGACY_DATA.bat` can remove old prototype data manually. It does not delete `library.sqlite3`.

## Clean Core architecture

The old single `images` table has been replaced with explicit domains:

- `assets` — physical image identity and file state
- `prompt_state` — current Prompt / Negative Prompt / Model
- `tags` + `asset_tags` — normalized tags
- `asset_search` — FTS5 full-text search index
- `prompt_revisions` — prompt snapshots
- `relations` — multi-parent generation lineage
- `collections` + `collection_assets` — project grouping

Original image files are never copied into SQLite.

## 快速导入

在 Windows 桌面版中，可以把图片、多个图片或整个文件夹直接拖进 ImageLore 窗口。文件夹会递归扫描，重复图片自动跳过。

## v0.10 user experience

The Frutiger Aero inspired workspace remains intentionally light and readable:

- Left: views, Collection / Tag / Model filters, virtualized library
- Center: image preview only
- Right: Prompt, generation information, lineage and compare
- Resizable panes and keyboard shortcuts
- Clear text labels on primary actions

### Shortcuts

- `Ctrl+F` — search
- `Ctrl+I` — import images
- `Ctrl+S` — save Prompt revision
- `Ctrl+Shift+C` — copy Prompt
- `F6` — focus Prompt editor
- `F7` — Fit / 100% preview
- `Alt+↑ / Alt+↓` — previous / next record

## Run in browser preview

```bash
npm install
npm run dev
```

Browser preview uses mock data and does not access local files.

## Native development

Requirements: Node.js, Rust, Windows WebView2, and the normal Tauri Windows build prerequisites.

```bash
npm install
npm run tauri:dev
```

## One-click Windows installer

For local developer builds, double-click:

`BUILD_WINDOWS_EXE.bat`

The NSIS installer is generated under:

`src-tauri\target\release\bundle\nsis\`

For normal distribution, use the included GitHub Actions workflow. End users only need the generated setup EXE; they do not need Rust, Node.js or Python.

## Versioning

`VERSION` is the single version source.

```bash
npm run version:sync
npm run version:check
npm run version:patch
npm run version:minor
npm run version:major
```
