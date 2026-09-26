# Windows one-click installer

ImageLore is configured for a Tauri 2 NSIS installer in current-user mode.

## End user

The release artifact is a normal setup EXE. The user double-clicks it and does not need Node.js, Rust or Python.

## GitHub Actions build

The included `.github/workflows/windows-release.yml` runs on `windows-latest`, installs Node and Rust, builds ImageLore and publishes the NSIS setup EXE to GitHub Releases.

## Local developer build

Install the standard Tauri Windows prerequisites, then double-click `BUILD_WINDOWS_EXE.bat` or run:

```bash
npm install
npm run tauri:build
```

Output is normally under:

`src-tauri\target\release\bundle\nsis\`
