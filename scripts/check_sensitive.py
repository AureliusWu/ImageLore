"""Block private files and credentials without printing their contents."""
from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path, PurePosixPath

MAX_BYTES = 8 * 1024 * 1024
PUBLIC_IMAGES = {"docs/og.png", "src-tauri/icons/icon.png", "src-tauri/icons/icon.ico"}
PRIVATE_DIRS = {
    ".git", "node_modules", "target", "desktop-runtime", "task-evidence",
    "private", "private-data", "user-data", "secrets", "credentials",
    "artifacts", "screenshots", "acceptance-runs", "test-results",
    "playwright-report", "backups", "logs", "captures",
    "__pycache__", ".aws", ".azure", ".ssh", ".private",
    "acceptance", "acceptance-data", "acceptance-evidence", "evidence", "screen-recordings",
}
PRIVATE_SUFFIXES = {
    ".db", ".db3", ".sqlite", ".sqlite3", ".wal", ".shm", ".pfx", ".p12",
    ".pem", ".key", ".ppk", ".jks", ".keystore", ".log", ".har", ".dmp",
    ".exe", ".dll", ".onnx", ".zip", ".7z", ".tar", ".gz", ".bundle",
}
IMAGE_SUFFIXES = {".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp", ".ico"}
RULES = [
    ("private-key", re.compile(r"-----BEGIN (?:[A-Z0-9 ]+ )?PRIVATE KEY-----")),
    ("provider-token", re.compile(r"\bsk-[A-Za-z0-9_-]{20,}\b")),
    ("github-token", re.compile(r"\b(?:gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,})\b")),
    ("huggingface-token", re.compile(r"\bhf_[A-Za-z0-9]{20,}\b")),
    ("aws-access-key", re.compile(r"\b(?:AKIA|ASIA)[A-Z0-9]{16}\b")),
    ("google-api-key", re.compile(r"\bAIza[A-Za-z0-9_-]{30,}\b")),
    ("slack-token", re.compile(r"\bxox[baprs]-[A-Za-z0-9-]{12,}\b")),
    ("credential-url", re.compile(r"https?://[^/\s:@]+:[^/\s@]+@", re.I)),
    ("jwt", re.compile(r"\beyJ[A-Za-z0-9_-]{8,}\.eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{16,}\b")),
    ("private-workspace-path", re.compile(r"[A-Z]:[\\/]+AI项目[\\/]", re.I)),
]
USER_PATH = re.compile(r"(?:[A-Z]:[\\/]+Users[\\/]+|/(?:Users|home)/)([^\\/\s\"'<>]+)", re.I)
DEMO_USERS = {"demo", "public", "default", "defaultuser0", "runneradmin", "runner", "test", "example"}
EMAIL = re.compile(r"\b[A-Z0-9._%+-]+@(?:[A-Z0-9-]+\.)+[A-Z]{2,}\b", re.I)
ASSIGNMENT = re.compile(
    r"\b(?:api[_-]?key|access[_-]?token|auth[_-]?token|client[_-]?secret|"
    r"secret[_-]?key|password|authorization)\b[\"']?\s*[:=]\s*[\"']([^\"'\r\n]{8,})[\"']",
    re.I,
)


def private_email(value: str) -> bool:
    domain = value.rsplit("@", 1)[-1].lower()
    return not (
        value.lower() == "noreply@github.com"
        or
        domain in {"example.com", "example.org", "example.net", "example.invalid", "noreply.github.com"}
        or domain.endswith(".example") or domain.endswith(".invalid")
        or domain == "users.noreply.github.com"
    )


def path_violation(name: str) -> str | None:
    path = PurePosixPath(name.replace("\\", "/"))
    parts = {part.lower() for part in path.parts}
    base = path.name.lower()
    if parts & PRIVATE_DIRS or name.replace("\\", "/").lower().startswith("src-tauri/gen/"):
        return "private-directory"
    if base.startswith(".env") and not base.endswith((".example", ".template", ".sample")):
        return "environment-file"
    if base in {".netrc", ".pypirc", ".npmrc", "id_rsa", "id_ed25519"} or re.match(r"^(?:credentials|cookies|secrets)(?:[._-]|$)", base):
        return "credential-file"
    if path.suffix.lower() in PRIVATE_SUFFIXES or base.endswith(("-wal", "-shm", "-journal")):
        return "private-artifact"
    if path.suffix.lower() in IMAGE_SUFFIXES and name not in PUBLIC_IMAGES:
        return "unapproved-image"
    return None


def inspect(name: str, data: bytes) -> list[tuple[int, str]]:
    failures: set[tuple[int, str]] = set()
    reason = path_violation(name)
    if reason:
        failures.add((0, reason))
    if len(data) > MAX_BYTES:
        failures.add((0, "oversized-unreviewed-file"))
        return sorted(failures)
    if data.startswith(b"SQLite format 3\x00"):
        failures.add((0, "sqlite-content"))
        return sorted(failures)
    if name in PUBLIC_IMAGES:
        return sorted(failures)
    if b"\x00" in data:
        failures.add((0, "unapproved-binary"))
        return sorted(failures)
    try:
        text = data.decode("utf-8-sig")
    except UnicodeDecodeError:
        failures.add((0, "unreviewed-encoding"))
        return sorted(failures)
    for number, line in enumerate(text.splitlines(), 1):
        for rule, pattern in RULES:
            if pattern.search(line):
                failures.add((number, rule))
        if any(match.group(1).lower() not in DEMO_USERS for match in USER_PATH.finditer(line)):
            failures.add((number, "private-user-path"))
        if any(private_email(match.group()) for match in EMAIL.finditer(line)):
            failures.add((number, "private-email"))
        for match in ASSIGNMENT.finditer(line):
            value = match.group(1)
            placeholder = re.fullmatch(
                r"(?i)(?:YOUR[_ -][A-Z0-9_ -]+|(?:EXAMPLE|PLACEHOLDER|REDACTED|CHANGE_ME|"
                r"DUMMY|DEMO|TEST|FAKE)(?:[_ -][A-Z0-9_ -]+)?|<[^>]*>|\$\{[^}]*\}|\$\{\{.*\}\}|\*+)",
                value,
            )
            if not placeholder:
                failures.add((number, "hardcoded-credential"))
    return sorted(failures)


def git(root: Path, *args: str, input_data: bytes | None = None) -> bytes:
    result = subprocess.run(
        ["git", "-C", str(root), *args], input=input_data,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
    )
    if result.returncode:
        raise RuntimeError("Git inspection failed; refusing upload (diagnostic contents withheld)")
    return result.stdout


def scan(root: Path, tracked: bool, staged: bool, history: bool, revisions: tuple[str, ...] = ()) -> tuple[list[tuple[str, str, int, str]], int]:
    findings: list[tuple[str, str, int, str]] = []
    inspected = 0

    def check(name: str, data: bytes, scope: str) -> None:
        nonlocal inspected
        inspected += 1
        findings.extend((scope, name, line, rule) for line, rule in inspect(name, data))

    if tracked:
        for raw in git(root, "ls-files", "-z").split(b"\0"):
            if not raw:
                continue
            name = raw.decode("utf-8")
            target = root / name
            if not target.exists():
                continue  # A locally deleted file is still checked by staged/history modes.
            if target.is_symlink() or not target.resolve().is_relative_to(root.resolve()):
                findings.append(("working-tree", name, 0, "unreviewed-symlink"))
                continue
            check(name, target.read_bytes(), "working-tree")
    if staged:
        names = git(root, "diff", "--cached", "--name-only", "-z", "--diff-filter=ACMR")
        for raw in names.split(b"\0"):
            if raw:
                name = raw.decode("utf-8")
                check(name, git(root, "show", ":" + name), "staged")
    if history:
        if any(not re.fullmatch(r"[0-9a-fA-F]{40}", revision) for revision in revisions):
            raise ValueError("Invalid outgoing revision")
        for raw in git(root, "log", "--all", *revisions, "--name-only", "-z", "--format=").split(b"\0"):
            name = raw.decode("utf-8").strip("\n")
            if name and (reason := path_violation(name)):
                findings.append(("history-path", name, 0, reason))
        objects: dict[str, str] = {}
        for raw in git(root, "-c", "core.quotePath=false", "rev-list", "--objects", "--all", *revisions).splitlines():
            oid, _, raw_name = raw.partition(b" ")
            objects[oid.decode("ascii")] = raw_name.decode("utf-8")
        if objects:
            request = ("\n".join(objects) + "\n").encode("ascii")
            rows = git(root, "cat-file", "--batch-check=%(objectname) %(objecttype) %(objectsize)", input_data=request)
            blobs = []
            for row in rows.splitlines():
                oid, kind, size = row.decode("ascii").split()
                if kind != "blob":
                    continue
                name = objects[oid] or "<unnamed-blob>"
                if int(size) > MAX_BYTES:
                    findings.append(("history:" + oid[:12], name, 0, "oversized-unreviewed-file"))
                    continue
                blobs.append((oid, name, int(size)))
            # Communicate drains both pipes; bounded batches avoid Windows pipe
            # deadlocks and one process per blob without weakening coverage.
            batches = []
            current = []
            current_bytes = 0
            for blob in blobs:
                if current and current_bytes + blob[2] > 16 * 1024 * 1024:
                    batches.append(current)
                    current, current_bytes = [], 0
                current.append(blob)
                current_bytes += blob[2]
            if current:
                batches.append(current)
            for batch in batches:
                request = ("\n".join(item[0] for item in batch) + "\n").encode("ascii")
                contents = git(root, "cat-file", "--batch", input_data=request)
                cursor = 0
                for oid, name, size in batch:
                    boundary = contents.index(b"\n", cursor)
                    header = contents[cursor:boundary].decode("ascii").split()
                    if header != [oid, "blob", str(size)]:
                        raise RuntimeError("Unexpected Git object response")
                    start = boundary + 1
                    end = start + size
                    if contents[end:end + 1] != b"\n":
                        raise RuntimeError("Truncated Git object response")
                    check(name, contents[start:end], "history:" + oid[:12])
                    cursor = end + 1
                if cursor != len(contents):
                    raise RuntimeError("Unexpected trailing Git object data")
        metadata = git(root, "log", "--all", *revisions, "--format=%H%x00%ae%x00%ce%x00%B%x00")
        # Commit metadata may leak credentials, personal paths, or non-noreply email.
        for rule_line, rule in inspect("<commit-metadata>", metadata.replace(b"\0", b"\n")):
            findings.append(("history", "<commit-metadata>", rule_line, rule))
    return sorted(set(findings)), inspected


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--tracked", action="store_true")
    parser.add_argument("--staged", action="store_true")
    parser.add_argument("--history", action="store_true")
    parser.add_argument("--push", action="store_true", help="Inspect actual outgoing revisions from Git's pre-push input, then run Gitleaks.")
    options = parser.parse_args()
    revisions: tuple[str, ...] = ()
    if options.push:
        outgoing = set()
        for line in sys.stdin:
            fields = line.split()
            if len(fields) != 4 or not re.fullmatch(r"[0-9a-fA-F]{40}", fields[1]):
                print("Malformed pre-push input; upload refused.", file=sys.stderr)
                return 2
            if fields[1] != "0" * 40:
                outgoing.add(fields[1])
        revisions = tuple(sorted(outgoing))
        options.tracked = options.history = True
    if not any((options.tracked, options.staged, options.history)):
        parser.error("Choose --tracked, --staged, or --history.")
    try:
        findings, count = scan(options.repo.resolve(), options.tracked, options.staged, options.history, revisions)
    except (OSError, UnicodeError, ValueError, RuntimeError):
        print("Sensitive-information check could not complete; upload refused.", file=sys.stderr)
        return 2
    if findings:
        for scope, name, line, rule in findings:
            print(f"BLOCK {scope} {name}:{line} [{rule}]", file=sys.stderr)
        print(f"Sensitive-information check failed: {len(findings)} findings; no matched values printed.", file=sys.stderr)
        return 1
    print(f"Sensitive-information check passed: {count} file/blob inspections.")
    if options.push:
        try:
            binary = git(options.repo, "config", "--get", "imagelore.gitleaksBinary").decode("utf-8").strip()
            if not Path(binary).is_file():
                raise OSError("Scanner unavailable")
            result = subprocess.run([
                binary, "git", str(options.repo),
                "--config", str(options.repo / ".gitleaks.toml"),
                "--log-opts=" + " ".join(("--all", *revisions)),
                "--redact=100", "--no-banner", "--no-color", "--log-level=warn",
            ], check=False)
            return 0 if result.returncode == 0 else 1
        except (OSError, UnicodeError, RuntimeError):
            print("Push refused: configure the verified local Gitleaks binary as described in SECURITY.md.", file=sys.stderr)
            return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
