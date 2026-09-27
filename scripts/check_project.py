from pathlib import Path
import json
root=Path(__file__).resolve().parents[1]
required=[
 'VERSION','package.json','package-lock.json','src/App.tsx','src/api.ts','src/styles.css','src/hooks/useEditorDraft.ts','src/hooks/useNativeDrop.ts',
 'src-tauri/Cargo.toml','src-tauri/Cargo.lock','src-tauri/schema.sql','src-tauri/src/lib.rs','src-tauri/src/db.rs','src-tauri/src/importer.rs','src-tauri/src/migrations.rs',
 'src-tauri/src/commands.rs','src-tauri/src/metadata.rs','src-tauri/tauri.conf.json','.github/workflows/ci.yml','.github/workflows/windows-release.yml'
]
missing=[x for x in required if not (root/x).exists()]
assert not missing,f'missing: {missing}'
version=(root/'VERSION').read_text().strip()
assert json.loads((root/'package.json').read_text())['version']==version
assert json.loads((root/'package-lock.json').read_text())['version']==version
assert json.loads((root/'src-tauri/tauri.conf.json').read_text())['version']==version
cargo=(root/'src-tauri/Cargo.toml').read_text()
assert f'version = "{version}"' in cargo.split('[lib]',1)[0]
runtime='\n'.join(p.read_text(errors='ignore') for p in (root/'src-tauri/src').glob('*.rs'))
assert 'PromptDock' not in runtime and 'imagelore.db' not in runtime and '.promptdock.json' not in runtime
assert 'library.sqlite3' in runtime
front='\n'.join(p.read_text(errors='ignore') for p in (root/'src').rglob('*.tsx'))
assert 'pd.' not in front
print(f'ImageLore v{version} project integrity: PASS')
