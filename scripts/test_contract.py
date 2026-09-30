from pathlib import Path
import re

root = Path(__file__).resolve().parents[1]
api = (root / "src/api.ts").read_text(encoding="utf-8")
mock = (root / "src/api/mock.ts").read_text(encoding="utf-8")
lib = (root / "src-tauri/src/lib.rs").read_text(encoding="utf-8")
native_files = list((root / "src-tauri/src").glob("*.rs"))
native = "\n".join(p.read_text(encoding="utf-8", errors="ignore") for p in native_files)

called = set(re.findall(r'call(?:<[^>]+>)?\("([a-z_]+)"', api))
annotated = set(
    re.findall(
        r'#\[tauri::command\]\s*pub\s+(?:async\s+)?fn\s+([a-z_]+)\s*\(',
        native,
        re.MULTILINE,
    )
)
handler_match = re.search(r"tauri::generate_handler!\[(.*?)\]\s*\)", lib, re.DOTALL)
assert handler_match, "Could not locate tauri::generate_handler! registration block"
registered = set(re.findall(r"::([a-z_]+)\s*,?", handler_match.group(1)))
mocked = set(re.findall(r'case\s+["\']([a-z_]+)["\']\s*:', mock))

missing_impl = called - annotated
missing_registration = called - registered
missing_mock = called - mocked
stale_registration = registered - annotated

assert not missing_impl, f"Frontend commands missing #[tauri::command] implementation: {sorted(missing_impl)}"
assert not missing_registration, f"Frontend commands missing Tauri registration: {sorted(missing_registration)}"
assert not missing_mock, f"Frontend commands missing browser mock handler: {sorted(missing_mock)}"
assert not stale_registration, f"Tauri handler registers non-command functions: {sorted(stale_registration)}"

print(
    "ImageLore frontend/native command contract: PASS "
    f"({len(called)} frontend commands, {len(registered)} registered, {len(mocked)} mocked)"
)
