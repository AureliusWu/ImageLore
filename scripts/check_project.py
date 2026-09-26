from pathlib import Path
import json,re
root=Path(__file__).resolve().parents[1]
required=[
 'VERSION','package.json','src/App.tsx','src/api.ts','src/styles.css','src-tauri/schema.sql','src-tauri/src/lib.rs','src-tauri/src/db.rs','src-tauri/src/commands.rs','src-tauri/src/metadata.rs','src-tauri/tauri.conf.json','.github/workflows/windows-release.yml'
]
missing=[x for x in required if not (root/x).exists()]
assert not missing, f'missing: {missing}'
version=(root/'VERSION').read_text().strip()
assert version=='0.10.0'
assert json.loads((root/'package.json').read_text())['version']==version
assert json.loads((root/'src-tauri/tauri.conf.json').read_text())['version']==version
# Legacy migration code must not survive in runtime source.
runtime='\n'.join(p.read_text(errors='ignore') for p in (root/'src-tauri/src').glob('*.rs'))
assert 'PromptDock' not in runtime
assert 'imagelore.db' not in runtime
assert '.promptdock.json' not in runtime
assert 'library.sqlite3' in runtime
# Frontend must not fall back to old layout keys.
front='\n'.join(p.read_text(errors='ignore') for p in (root/'src').rglob('*.tsx'))
assert 'pd.' not in front
print('ImageLore v0.10 project integrity: PASS')
