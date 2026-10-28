"""Stage verified v1.0.0 release assets on GitHub-hosted Windows only."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import urllib.request
import zipfile

from verify_distribution_notices import regular, verify


def sha(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def add_bytes(archive, name, data):
    info = zipfile.ZipInfo(name, date_time=(2026, 10, 28, 0, 0, 0))
    info.external_attr = 0o100644 << 16
    archive.writestr(info, data)


def source_archives(root: Path, output: Path) -> int:
    index_path = root / "third_party/distribution/rust-source-index.json"
    entries = json.loads(regular(index_path))
    cache = Path(os.environ.get("CARGO_HOME", str(Path.home() / ".cargo"))) / "registry/cache"
    with zipfile.ZipFile(output, "x", compression=zipfile.ZIP_STORED) as archive:
        for entry in entries:
            name = f"{entry['package']}-{entry['version']}.crate"
            data = None
            for candidate in cache.glob(f"*/{name}"):
                if sha(candidate) == entry["crateSha256"]:
                    data = regular(candidate)
                    break
            if data is None:
                request = urllib.request.Request(entry["sourceArchiveUrl"], headers={"User-Agent": "ImageLore-public-source-distribution"})
                with urllib.request.urlopen(request, timeout=90) as response:
                    data = response.read(64 * 1024 * 1024 + 1)
            if len(data) > 64 * 1024 * 1024 or hashlib.sha256(data).hexdigest() != entry["crateSha256"]:
                raise ValueError("dependency source archive differs from Cargo.lock")
            add_bytes(archive, "crates/" + name, data)
        add_bytes(archive, "source-index.json", regular(index_path))
        add_bytes(archive, "README.txt", b"Original unmodified registry source archives; Cargo.lock SHA-256 pins are in source-index.json. Includes build dependencies; not a final static link map. Individual components retain their own licenses.\n")
    return len(entries)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--verification", type=Path, required=True)
    parser.add_argument("--application", type=Path, required=True)
    parser.add_argument("--installer", type=Path, required=True)
    args = parser.parse_args()
    if os.environ.get("GITHUB_ACTIONS") != "true" or os.environ.get("RUNNER_ENVIRONMENT") != "github-hosted":
        raise ValueError("release asset preparation requires a GitHub-hosted runner")
    if os.environ.get("GITHUB_REPOSITORY") != "AureliusWu/ImageLore" or os.environ.get("GITHUB_REF") != "refs/tags/v1.0.0":
        raise ValueError("release asset preparation requires the exact authorized repository/tag")
    root = Path(__file__).resolve().parents[1]
    source = os.environ["GITHUB_SHA"]
    if not re.fullmatch(r"[0-9a-f]{40}", source) or subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root).decode().strip() != source:
        raise ValueError("release checkout differs from triggering source")
    if (root / "VERSION").read_text().strip() != "1.0.0":
        raise ValueError("release version differs")
    evidence = json.loads(regular(args.verification))
    if evidence["state"] != "PASS_KNOWN_LIMITATIONS_RELEASE" or evidence["releaseStatus"] != "RELEASED_WITH_WARNINGS" or evidence["version"] != "1.0.0":
        raise ValueError("actual unsigned release verification is missing")
    if sha(args.application) != evidence["applicationSha256"] or sha(args.installer) != evidence["installerSha256"]:
        raise ValueError("verified release artifact bytes changed")
    notes = verify(root)
    output = args.output.resolve()
    if not output.is_relative_to(Path(os.environ["RUNNER_TEMP"]).resolve()):
        raise ValueError("release staging must remain inside RUNNER_TEMP")
    output.mkdir(parents=False, exist_ok=False)
    installer = output / "ImageLore_1.0.0_x64-setup.exe"
    shutil.copyfile(args.installer, installer)
    if sha(installer) != evidence["installerSha256"]:
        raise ValueError("staged installer differs from verified input")
    notice_zip = output / "ImageLore-1.0.0-notices.zip"
    collection = root / "third_party/distribution"
    with zipfile.ZipFile(notice_zip, "x", compression=zipfile.ZIP_DEFLATED) as archive:
        for path in sorted(collection.rglob("*")):
            if path.is_file():
                archive.writestr("licenses/" + path.relative_to(collection).as_posix(), regular(path))
    source_zip = output / "ImageLore-1.0.0-rust-dependency-sources.zip"
    count = source_archives(root, source_zip)
    assets = [{"name": p.name, "size": p.stat().st_size, "sha256": sha(p)} for p in sorted(output.iterdir())]
    manifest = {"state": "RELEASED_WITH_WARNINGS", "version": "1.0.0", "sourceCommit": source,
        "repository": "AureliusWu/ImageLore", "tag": "v1.0.0", "buildRunUrl": f"https://github.com/AureliusWu/ImageLore/actions/runs/{os.environ['GITHUB_RUN_ID']}",
        "verification": evidence, "distributionNotices": notes, "lockedDependencySourceArchives": count,
        "assets": assets, "updaterAvailable": False,
        "acceptanceScope": "This exact-source hosted job passed preceding frontend/native gates and both historical Windows upgrade paths. Local GUI, full performance/long-running/offline acceptance and trusted signing remain disclosed limitations."}
    (output / "release-manifest.json").write_text(json.dumps(manifest, indent=2, ensure_ascii=True) + "\n", encoding="utf-8")
    checksums = "".join(f"{sha(p)}  {p.name}\n" for p in sorted(output.iterdir()))
    (output / "SHA256SUMS.txt").write_text(checksums, encoding="utf-8")
    if sha(args.application) != evidence["applicationSha256"] or sha(args.installer) != evidence["installerSha256"]:
        raise ValueError("original verified artifacts changed during staging")
    print(json.dumps({"state": "PASS_VERIFIED_RELEASE_ASSET_STAGING", "assets": len(list(output.iterdir())), "sourceCommit": source, "sourceArchives": count}))


if __name__ == "__main__":
    main()
