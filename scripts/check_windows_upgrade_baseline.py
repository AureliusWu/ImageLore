"""Read-only identity check for pinned historical Windows upgrade builds."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import tomllib


def git(root: Path, *args: str) -> bytes:
    return subprocess.check_output(["git", "-C", str(root), *args], stderr=subprocess.PIPE)


def verify(root: Path, commit: str, version: str) -> dict:
    root = root.resolve(strict=True)
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("expected a full pinned commit")
    if git(root, "rev-parse", "HEAD").decode().strip() != commit:
        raise ValueError("baseline HEAD differs from its pin")
    if git(root, "show", f"{commit}:VERSION").decode("utf-8-sig").strip() != version:
        raise ValueError("pinned baseline version differs")
    if (root / "VERSION").read_text(encoding="utf-8-sig").strip() != version:
        raise ValueError("working baseline version differs")
    changed = set(filter(None, git(root, "diff", "--no-ext-diff", "--name-only", "-z", commit, "--").decode().split("\0")))
    if changed - {"src-tauri/Cargo.toml"}:
        raise ValueError("tracked baseline source outside Cargo.toml changed")
    original = git(root, "show", f"{commit}:src-tauri/Cargo.toml")
    current = (root / "src-tauri/Cargo.toml").read_bytes()
    before = tomllib.loads(original.decode("utf-8-sig"))
    after = tomllib.loads(current.decode("utf-8-sig"))
    normalization = "none"
    old_build = before.get("build-dependencies", {}).get("tauri-build")
    new_build = after.get("build-dependencies", {}).get("tauri-build")
    if isinstance(old_build, dict) and isinstance(new_build, dict) and "features" not in old_build and new_build.get("features") == []:
        del new_build["features"]
        normalization = "tauri-build-added-empty-features-only"
    if after != before:
        raise ValueError("baseline Cargo semantics changed beyond empty tauri-build features")
    if changed and normalization == "none":
        raise ValueError("baseline Cargo change is not the known empty-features normalization")
    if before.get("package", {}).get("version") != version:
        raise ValueError("baseline Cargo version differs")
    allowed = {f"src-tauri/gen/schemas/{name}.json" for name in ("acl-manifests", "capabilities", "desktop-schema", "windows-schema")}
    extra = set(filter(None, git(root, "ls-files", "--others", "--exclude-standard", "-z").decode().split("\0")))
    if extra - allowed:
        raise ValueError("unexpected untracked baseline files")
    generated = {}
    for name in sorted(extra):
        path = root / name
        if path.is_symlink() or not path.is_file():
            raise ValueError("generated baseline output is not a regular file")
        data = path.read_bytes()
        json.loads(data)
        generated[name] = hashlib.sha256(data).hexdigest()
    return {"state": "PASS_PINNED_BASELINE_SOURCE_IDENTITY", "sourceCommit": commit, "version": version,
            "changedTrackedPaths": sorted(changed), "cargoNormalization": normalization,
            "originalCargoTomlSha256": hashlib.sha256(original).hexdigest(),
            "builtCargoTomlSha256": hashlib.sha256(current).hexdigest(), "generatedSchemaOutputs": generated,
            "scope": "Pinned HEAD/version, all tracked source changes refused except semantically identical empty tauri-build features insertion; four known generated schema JSON outputs recorded separately."}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository", type=Path, required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--version", required=True)
    args = parser.parse_args()
    try:
        result = verify(args.repository, args.commit, args.version)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        # Never echo TOML contents or subprocess output.
        print(f"Baseline source identity refused: {type(error).__name__}: {error if isinstance(error, ValueError) else 'read or Git command failed'}", file=sys.stderr)
        return 1
    print(json.dumps(result, ensure_ascii=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
