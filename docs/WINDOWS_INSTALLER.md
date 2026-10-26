# Windows one-click installer

ImageLore uses a Tauri 2 NSIS installer in current-user mode. The application identity remains stable across upgrades so a newer installer replaces the existing installation while preserving local user data.

## End user

The release artifact is a normal setup EXE. The user double-clicks it and does not need Node.js, Rust or Python.

The Windows bundle enables Tauri's `bundleVCRuntime` option. The native executable still imports `MSVCP140.dll` and `MSVCP140_1.dll`; the installer includes the Visual C++ runtime DLLs beside the application so that current-user installs do not depend on a developer machine's global runtime. Tauri obtains these files from the builder's installed Visual Studio redistributable directory and fails if it cannot find them. The Microsoft runtime keeps its own redistribution terms. Locally bundled runtimes must be updated with application releases; see [Microsoft's deployment guidance](https://learn.microsoft.com/en-us/cpp/windows/deployment-in-visual-cpp) and [Tauri's option](https://v2.tauri.app/reference/config/#bundlevcruntime).

`npm run tauri:build` first builds the application without an installer, then calls `scripts/bundle_windows_installer.ps1`. The explicit ort `copy-dylibs` feature places the pinned provider DLL in the build output; FastEmbed's disabled defaults did not enable that feature. The bundling step resolves Cargo's actual output directory, verifies the output `DirectML.dll` size/hash against `src-tauri/windows-runtime.json` and its locked ort-sys version, and passes a generated resource map to the standalone Tauri bundler. The DLL is installed beside the executable. Build inputs remain in the ignored target directory, including when `CARGO_TARGET_DIR` is customized. A new DirectML dependency must receive an updated, reviewed manifest; an absent or mismatched DLL stops packaging.

## GitHub Actions build

The workflow is `.github/workflows/windows-release.yml`.

The repository follows a main-only development model. A Windows installer build starts only when one of these explicit release signals occurs:

- the workflow is started manually with `workflow_dispatch`;
- a `v*` tag is pushed for a formal release;
- `.github/windows-build-request.txt` is deliberately changed after an explicit request to build an installer.

Ordinary commits to `main` do not build an EXE.

Manual workflow runs default to validation only: build, install/upgrade and data/runtime checks run, while the installer artifact upload is skipped. The `upload_installer` input explicitly enables upload after distribution review. A manual validation run does not create a GitHub Release.

Tag publication now requires a configured approved public signing-certificate SHA-256 list and valid embedded Authenticode signatures on both the application and final installer. The current list in `src-tauri/windows-signing-policy.json` is empty, so this source's tag publication is blocked. Product/version, timestamp and independent Windows SDK SignTool checks must pass before either upload or Release. This does not configure a signing service or place credentials in GitHub. See [release signature gate and evidence boundaries](V0_26_RELEASE_SIGNATURE_GATE.md).

Before building the current installer, the same checkout runs the existing frontend check/build and Rust fmt, locked check, strict clippy and full library tests. The workflow records the triggering SHA and verifies that it and tracked sources remain unchanged before installer bundling and before upload/publication.

The Windows in-place-upgrade smoke tests from pinned v0.19.0 and v0.25.1 baselines remain required before upload. Each baseline checkout uses a fresh directory under `RUNNER_TEMP`; the workflow does not recursively delete an existing baseline directory. `scripts/test_windows_upgrade.ps1` refuses execution unless `GITHUB_ACTIONS=true` and `RUNNER_ENVIRONMENT=github-hosted`. There is no local or self-hosted override. Every recursive cleanup resolves and checks exactly the two expected ImageLore data directories under `LOCALAPPDATA`, rejecting reparse points in their ancestors or contents.

The smoke test checks the existing install directory, executable and registered versions, the unique installed-app entry and the Start menu target. The actual v0.19.0 schema 7 seeds seven business tables in the old install directory. The pinned v0.25.1 schema 11 seeds 23 populated business/settings tables in the existing separate data root, including synthetic 512-dimensional vector bytes, Vision/Remix/reference records and settings. Each baseline requires two upgraded launches that append current `startup: library ready` evidence before `WM_CLOSE` and normal exit. After each close, read-only checks require schema 11, SQLite integrity, foreign-key integrity, all seeded columns and original image sizes/hashes. Preserved backup and model sentinel files retain their SHA-256 hashes. Synthetic vectors and model sentinel files do not prove inference; the startup log proves backend initialization, not full UI readiness. See [modern baseline preparation and current evidence](V0_26_MODERN_UPGRADE_BASELINE.md). Updater, signing and interactive save/cancel acceptance remain separate gates.

The mandatory `CurrentExecutable` parameter binds the installed inner application to the exact built EXE. The script holds the build output read-only throughout acceptance and compares the installed file hash before launch and after both normal closes. Signing only an output EXE after packaging cannot qualify an installer containing different inner bytes.

For each upgraded launch the smoke also requires DirectML and the two imported MSVCP DLLs to exist in the installation directory and verifies that the process actually loaded them from there. A globally installed runtime on the hosted runner therefore cannot satisfy this check. Filename, version and SHA-256 are recorded without exposing user library contents.

## Local developer build

For the versioned desktop shortcut and a local executable without an installer, use `npm run desktop:build`. See [desktop shortcut maintenance](DESKTOP_SHORTCUT.md). This does not trigger the Windows release workflow.

Install the standard Tauri Windows prerequisites, then double-click `BUILD_WINDOWS_EXE.bat` or run:

```bash
npm install
npm run tauri:build
```

Output is normally under:

`src-tauri\target\release\bundle\nsis\`
