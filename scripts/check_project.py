from pathlib import Path
import json
root=Path(__file__).resolve().parents[1]
required=[
 'VERSION','package.json','package-lock.json','scripts/version_targets.mjs','scripts/test_preview_workflow.mjs','scripts/test_remix_workflow.mjs','src/App.tsx','src/api.ts','src/styles.css','src/previewWorkflow.ts','src/hooks/useEditorDraft.ts','src/hooks/useNativeDrop.ts',
 'src-tauri/Cargo.toml','src-tauri/Cargo.lock','src-tauri/schema.sql','src-tauri/src/lib.rs','src-tauri/src/db.rs','src-tauri/src/importer.rs','src-tauri/src/migrations.rs',
 'src-tauri/src/backup.rs','src-tauri/src/diagnostics.rs','src-tauri/src/visual_dna.rs','src-tauri/src/vision.rs','src-tauri/src/remix.rs','src-tauri/src/commands.rs','src-tauri/src/generation.rs','src-tauri/src/generation_index.rs','src-tauri/src/jobs.rs','src-tauri/src/metadata.rs','src-tauri/src/semantic.rs','src-tauri/src/sources.rs','src-tauri/tauri.conf.json','scripts/test_windows_upgrade.ps1',
 'src/hooks/useImportJob.ts','src/hooks/useSemanticJob.ts','src/hooks/useCloseGuard.ts','src/components/LibraryManager.tsx','src/components/ParentPicker.tsx','.github/workflows/ci.yml','.github/workflows/native-ci.yml','.github/workflows/windows-release.yml'
]
missing=[x for x in required if not (root/x).exists()]
assert not missing,f'missing: {missing}'
version=(root/'VERSION').read_text(encoding='utf-8').strip()
package=json.loads((root/'package.json').read_text(encoding='utf-8'))
assert package['version']==version
assert 'test:preview' in package['scripts'] and 'npm run test:preview' in package['scripts']['check']
assert 'test:remix' in package['scripts'] and 'npm run test:remix' in package['scripts']['check']
assert json.loads((root/'package-lock.json').read_text(encoding='utf-8'))['version']==version
assert json.loads((root/'src-tauri/tauri.conf.json').read_text(encoding='utf-8'))['version']==version
cargo=(root/'src-tauri/Cargo.toml').read_text(encoding='utf-8')
assert f'version = "{version}"' in cargo.split('[lib]',1)[0]
assert 'fastembed' in cargo and 'reqwest' in cargo
tauri=json.loads((root/'src-tauri/tauri.conf.json').read_text(encoding='utf-8'))
assert tauri['productName']=='ImageLore'
assert tauri['identifier']=='app.imagelore.desktop'
assert tauri['bundle']['windows']['nsis']['installMode']=='currentUser'
assert tauri['bundle']['windows']['nsis']['startMenuFolder']=='ImageLore'
assert tauri['bundle']['windows']['allowDowngrades'] is False
assert tauri['app']['security']['assetProtocol']['scope']==['$LOCALDATA/app.imagelore.desktop/cache/**']
runtime='\n'.join(p.read_text(encoding='utf-8', errors='ignore') for p in (root/'src-tauri/src').glob('*.rs'))
assert 'PromptDock' not in runtime and 'imagelore.db' not in runtime and '.promptdock.json' not in runtime
assert 'library.sqlite3' in runtime
assert 'DATA_DIR_NAME:&str="app.imagelore.desktop"' in runtime
assert 'LEGACY_DATA_DIR_NAME:&str="ImageLore"' in runtime
assert 'ensure_auto_backup' in runtime and 'start_import_folder' in runtime and 'duplicate_groups' in runtime
assert 'recover_latest_valid_backup' in runtime and 'MAX_LOG_FILES:usize=5' in runtime and 'diagnostics_status' in runtime
assert 'const LATEST:i64=10' in (root/'src-tauri/src/migrations.rs').read_text(encoding='utf-8')
assert 'imagelore.sidecar.v3' in runtime and 'pending_relations' in runtime and 'generation_sessions' in runtime and 'source_folders' in runtime and 'generation_index' in runtime and 'semantic_embeddings' in runtime and 'asset_cjk_search' in runtime and 'visual_dna' in runtime
assert 'image_prompt_analyses' in runtime and 'analyze_image_to_prompt' in runtime and 'save_image_prompt_revision' in runtime and 'vision_settings' in runtime
assert 'remix_drafts' in runtime and 'remix_sources' in runtime and 'apply_remix_lineage' in runtime
assert 'vision_api_key:Mutex::new(String::new())' in (root/'src-tauri/src/jobs.rs').read_text(encoding='utf-8')
front='\n'.join(p.read_text(encoding='utf-8', errors='ignore') for p in (root/'src').rglob('*.tsx'))
assert 'pd.' not in front
print(f'ImageLore v{version} project integrity: PASS')
