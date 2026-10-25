# Frontend third-party notices

These original public notice texts accompany the frontend scope documented in [the byte-matched frontend review](../../docs/V0_26_FRONTEND_DISTRIBUTION_SCOPE.md). They cover five npm components and two generated helpers from application source `1dfe36999fd63eacdb26259da7ffde0a6f0a836b`; the scope and output digests remain recorded in [notice-manifest.json](notice-manifest.json).

| Component | Version | Package declaration | Original texts |
| --- | --- | --- | --- |
| @tauri-apps/api | 2.12.0 | Apache-2.0 OR MIT | [Apache](licenses/@tauri-apps__api/2.12.0/LICENSE-APACHE-2.0), [MIT](licenses/@tauri-apps__api/2.12.0/LICENSE-MIT), [SPDX](licenses/@tauri-apps__api/2.12.0/LICENSE.spdx) |
| @tauri-apps/plugin-dialog | 2.8.0 | MIT OR Apache-2.0 | [SPDX](licenses/@tauri-apps__plugin-dialog/2.8.0/LICENSE.spdx), [Apache](licenses/@tauri-apps__plugin-dialog/2.8.0/LICENSE_APACHE-2.0), [MIT](licenses/@tauri-apps__plugin-dialog/2.8.0/LICENSE_MIT) |
| react | 19.3.0 | MIT | [LICENSE](licenses/react/19.3.0/LICENSE) |
| react-dom | 19.3.0 | MIT | [LICENSE](licenses/react-dom/19.3.0/LICENSE) |
| scheduler | 0.28.0 | MIT | [LICENSE](licenses/scheduler/0.28.0/LICENSE) |

The [full Vite 7.3.7 license and component notices](licenses/vite-generated-helpers/7.3.7/LICENSE.md), including the plugin-commonjs notice, are retained for the modulepreload polyfill and CommonJS helper. This does not mean that the entire build tool is delivered to the client.

The dialog package publishes an SPDX declaration. Its unchanged declaration is retained with the full MIT/Apache texts from the upstream revision named by the exact-version npm gitHead metadata. That metadata is a publisher claim, not a cryptographic source attestation.

All ten text files preserve their original bytes, names, copyright notices and disclaimers. No owner or year has been invented. The project MIT license does not replace their respective terms. Git keeps these files without line-ending conversion; only the original Vite notice has a whitespace-check exception to retain upstream blockquote spaces. Its email-rule exception is limited to this exact path and SHA-256, with any byte change rejected by the upload guard. Credential and other privacy checks remain active.

This directory is source material for the verified frontend scope. It is **not a complete application notice bundle**, is not yet delivered by the installer, and does not close native code, DirectML, VC++, model, font or icon distribution review. No signed release or Foundation approval is implied.
