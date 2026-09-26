from pathlib import Path
import re
root=Path(__file__).resolve().parents[1]
api=(root/'src/api.ts').read_text()
native='\n'.join(p.read_text(errors='ignore') for p in (root/'src-tauri/src').glob('*.rs'))
called=set(re.findall(r'call<[^>]+>\("([a-z_]+)"',api))|set(re.findall(r'call\("([a-z_]+)"',api))
implemented=set(re.findall(r'pub fn ([a-z_]+)\s*\(',native))
missing=called-implemented
assert not missing,f'Frontend commands missing in Rust: {sorted(missing)}'
print(f'ImageLore frontend/native command contract: PASS ({len(called)} commands)')
