from pathlib import Path
import re
root=Path(__file__).resolve().parents[1]
api=(root/'src/api.ts').read_text()
cmd=(root/'src-tauri/src/commands.rs').read_text()
called=set(re.findall(r'call<[^>]+>\("([a-z_]+)"',api))
# calls with nested generic arrays can be missed; collect all literal command strings near call(
called |= set(re.findall(r'call\("([a-z_]+)"',api))
implemented=set(re.findall(r'pub fn ([a-z_]+)\s*\(',cmd))
missing=called-implemented
assert not missing, f'Frontend commands missing in Rust: {sorted(missing)}'
print(f'ImageLore frontend/native command contract: PASS ({len(called)} commands)')
