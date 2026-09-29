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

Before the current installer artifact is uploaded, the workflow verifies version/lockfile consistency and performs the Windows in-place-upgrade smoke test from the v0.19.0 baseline.

## Local developer build

Install the standard Tauri Windows prerequisites, then double-click `BUILD_WINDOWS_EXE.bat` or run:

```bash
npm install
npm run tauri:build
```

Output is normally under:

`src-tauri\target\release\bundle\nsis\`
