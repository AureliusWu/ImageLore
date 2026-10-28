"""Verify source-attributed texts, lock pins and optional installed notice bytes."""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import tomllib


def regular(path: Path) -> bytes:
    if not path.is_file() or any(p.is_symlink() or p.is_junction() for p in (path, *path.parents)):
        raise ValueError("notice input is absent or has a reparse-point ancestor")
    return path.read_bytes()


def verify(root: Path, installed: Path | None = None) -> dict:
    collection = root / "third_party/distribution"
    manifest = json.loads(regular(collection / "notice-manifest.json"))
    files = manifest["files"]
    if len(files) != 786:
        raise ValueError("distribution text inventory count changed")
    names = set()
    for item in files:
        name = item["path"]
        relative = PurePosixPath(name)
        if relative.is_absolute() or ".." in relative.parts or "\\" in name or name.casefold() in names:
            raise ValueError("ambiguous distribution notice path")
        names.add(name.casefold())
        data = regular(collection / name)
        if len(data) != item["bytes"] or hashlib.sha256(data).hexdigest() != item["sha256"]:
            raise ValueError("distribution notice differs from its source identity")
    expected = {item["path"] for item in files} | {"README.md", "THIRD_PARTY_NOTICES.txt", "notice-manifest.json", "rust-source-index.json"}
    actual = {p.relative_to(collection).as_posix() for p in collection.rglob("*") if p.is_file()}
    if actual != expected:
        raise ValueError("distribution collection has missing or unexpected files")
    # The combined text contains upstream emails and is separately byte-pinned.
    attribution = json.loads(regular(root / "scripts/public_attribution_hashes.json"))
    combined = regular(collection / "THIRD_PARTY_NOTICES.txt")
    if hashlib.sha256(combined).hexdigest() != attribution["third_party/distribution/THIRD_PARTY_NOTICES.txt"]:
        raise ValueError("combined notice differs from reviewed text")
    source_index = json.loads(regular(collection / "rust-source-index.json"))
    locked = {(p["name"], p["version"]): p.get("checksum") for p in tomllib.loads(regular(root / "src-tauri/Cargo.lock").decode())["package"]}
    identities = set()
    for item in source_index:
        name, version, digest = item["package"], item["version"], item["crateSha256"]
        key = name, version
        if key in identities or not re.fullmatch(r"[0-9a-f]{64}", digest) or locked.get(key) != digest:
            raise ValueError("source archive index differs from Cargo.lock")
        if item["sourceArchiveUrl"] != f"https://crates.io/api/v1/crates/{name}/{version}/download":
            raise ValueError("source archive URL differs from canonical registry identity")
        identities.add(key)
    if len(identities) != 403:
        raise ValueError("Windows dependency source inventory count changed")
    bindings = {name: hashlib.sha256(regular(collection / name)).hexdigest() for name in sorted(expected)}
    if installed is not None:
        actual = {p.relative_to(installed).as_posix() for p in installed.rglob("*") if p.is_file()}
        if actual != expected:
            raise ValueError("installed notice file set differs from reviewed inputs")
        for name, digest in bindings.items():
            if hashlib.sha256(regular(installed / name)).hexdigest() != digest:
                raise ValueError("installed notice bytes differ from reviewed inputs")
    return {"state": "PASS_INSTALLED_DISTRIBUTION_NOTICES" if installed else "PASS_SOURCE_DISTRIBUTION_NOTICES", "textEntries": len(files), "files": len(expected), "lockedRustSourceArchives": len(identities), "noticeManifestSha256": hashlib.sha256(regular(collection / "notice-manifest.json")).hexdigest(), "scope": "Exact collected text/source pins and installed byte identity when requested; conservative inventory includes build tools and is not a final link map or legal approval."}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--installed-root", type=Path)
    args = parser.parse_args()
    print(json.dumps(verify(args.root, args.installed_root), separators=(",", ":")))
