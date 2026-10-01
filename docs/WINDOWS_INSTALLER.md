# Windows one-click installer

ImageLore uses a Tauri 2 NSIS installer in current-user mode. The application identity remains stable across upgrades so a newer installer replaces the existing installation while preserving local user data.

## End user

The release artifact is a normal setup EXE. The user double-clicks it and does not need Node.js, Rust or Python.

## GitHub Actions build

The workflow is `.github/workflows/windows-release.yml`.

The repository follows a main-only development model. A Windows installer build starts only when one of these explicit release signals occurs:

- the workflow is started manually with `workflow_dispatch`;
- a `v*` tag is pushed for a formal release;
- `.github/windows-build-request.txt` is deliberately changed after an explicit request to build an installer.

Ordinary commits to `main` do not build an EXE.

Before building the current installer, the same checkout runs the existing frontend check/build and Rust fmt, locked check, strict clippy and full library tests. The workflow records the triggering SHA and verifies that it and tracked sources remain unchanged before installer bundling and before upload/publication.

The Windows in-place-upgrade smoke test from the v0.19.0 baseline remains required before upload. Its baseline checkout uses a fresh directory under `RUNNER_TEMP`; the workflow does not recursively delete an existing baseline directory. `scripts/test_windows_upgrade.ps1` refuses execution unless `GITHUB_ACTIONS=true` and `RUNNER_ENVIRONMENT=github-hosted`. There is no local or self-hosted override. Every recursive cleanup resolves and checks exactly the two expected ImageLore data directories under `LOCALAPPDATA`, rejecting reparse points in their ancestors or contents.

The smoke test checks the existing install directory, executable and registered versions, the unique installed-app entry and the Start menu target. A synthetic database built from the actual v0.19.0 schema 7 seeds an asset, prompt, tag membership, collection membership and prompt revision. Each of two upgraded launches must append the current diagnostics `startup: library ready` evidence before receiving `WM_CLOSE` and exiting normally. After each close, read-only checks require schema 11, SQLite integrity, foreign-key integrity and all seeded business fields. Preserved backup and model sentinel files must retain their SHA-256 hashes. The model sentinel proves file preservation only; it does not exercise model inference. The startup log proves backend library initialization, not full UI readiness. Updater, signing and interactive save/cancel acceptance remain separate gates.

## Local developer build

For the versioned desktop shortcut and a local executable without an installer, use `npm run desktop:build`. See [desktop shortcut maintenance](DESKTOP_SHORTCUT.md). This does not trigger the Windows release workflow.

Install the standard Tauri Windows prerequisites, then double-click `BUILD_WINDOWS_EXE.bat` or run:

```bash
npm install
npm run tauri:build
```

Output is normally under:

`src-tauri\target\release\bundle\nsis\`
