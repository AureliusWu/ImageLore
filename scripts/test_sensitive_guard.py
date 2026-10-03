"""Synthetic regression cases for upload blocking and redacted diagnostics."""
from __future__ import annotations

import string
import subprocess
import tempfile
import unittest
from pathlib import Path

import check_sensitive as guard


class SensitiveGuardTests(unittest.TestCase):
    def test_credentials_and_private_key_are_blocked(self):
        values = [
            "sk-" + string.ascii_letters + string.digits,
            "gh" + "p_" + string.ascii_letters,
            "hf" + "_" + string.ascii_letters,
            "AK" + "IA" + "A" * 16,
            "AIza" + string.ascii_letters,
            "xox" + "b-" + string.ascii_letters,
            "-----BEGIN " + "PRIVATE KEY-----",
        ]
        for value in values:
            with self.subTest(kind=value[:3]):
                self.assertTrue(guard.inspect("settings.txt", value.encode()))

    def test_placeholder_and_synthetic_paths_are_allowed(self):
        examples = b"""
api_key = "YOUR_API_KEY_EXAMPLE"
password = ""
const apiKey = runtimeKey;
C:/Users/Demo/Downloads/ImageLore Inbox
D:/AI/outputs
synthetic@example.invalid
"""
        self.assertEqual(guard.inspect("example.txt", examples), [])
        self.assertIsNone(guard.path_violation(".env.example"))
        self.assertIsNone(guard.path_violation(".env.development.template"))

    def test_nonplaceholder_high_entropy_assignment_is_blocked(self):
        value = string.ascii_letters + string.digits
        data = ('api_key="' + value + '"').encode()
        self.assertIn((1, "hardcoded-credential"), guard.inspect("config.txt", data))
        weak = ('password="' + "weakvalue12" + '"').encode()
        self.assertIn((1, "hardcoded-credential"), guard.inspect("config.txt", weak))

    def test_user_path_and_personal_email_are_blocked(self):
        path = "C:/Users/" + "real-owner" + "/Desktop/file.txt"
        email = "owner" + "@" + "mail.test"
        self.assertIn((1, "private-user-path"), guard.inspect("document.md", path.encode()))
        self.assertIn((1, "private-email"), guard.inspect("document.md", email.encode()))

    def test_private_artifacts_are_blocked_even_with_safe_content(self):
        for name in [
            ".env", ".env.local", "private/config.json", "credentials.json",
            "library.sqlite3", "library.sqlite3-wal", "server.pfx", "signing.key",
            "screenshots/window.png", "desktop-runtime/current/runtime.json",
            "src-tauri/gen/schemas/desktop-schema.json", "node_modules/module/a.js",
        ]:
            with self.subTest(path=name):
                self.assertTrue(guard.inspect(name, b"{}"))

    def test_database_header_and_unknown_binary_are_blocked(self):
        self.assertIn((0, "sqlite-content"), guard.inspect("renamed.txt", b"SQLite format 3\x00"))
        self.assertIn((0, "unapproved-binary"), guard.inspect("renamed.txt", b"x\x00y"))
        self.assertEqual(guard.inspect("docs/og.png", b"\x89PNG\x00"), [])
        self.assertTrue(guard.inspect("docs/private-photo.png", b"\x89PNG\x00"))

    def test_source_json_sql_and_public_identity_are_allowed(self):
        self.assertEqual(guard.inspect("fixtures/schema-11.sql", b"INSERT INTO app_meta VALUES('schema_version','11');"), [])
        self.assertEqual(guard.inspect("fixtures/manifest.json", b'{"schema":11}'), [])
        self.assertFalse(guard.private_email("public@users.noreply.github.com"))
        self.assertFalse(guard.private_email("noreply@github.com"))

    def fixture(self):
        temporary = tempfile.TemporaryDirectory(prefix="imagelore-sensitive-test-")
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        self.git(root, "init", "-q")
        self.git(root, "config", "user.name", "Synthetic contributor")
        self.git(root, "config", "user.email", "synthetic@example.invalid")
        return root

    @staticmethod
    def git(root, *args):
        result = subprocess.run(["git", "-C", str(root), *args], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        if result.returncode:
            raise AssertionError("Synthetic git fixture setup failed")
        return result.stdout

    def commit(self, root):
        self.git(root, "commit", "-qm", "Synthetic fixture")

    def test_staged_blob_is_checked_when_working_tree_is_safe(self):
        root = self.fixture()
        value = "gh" + "p_" + string.ascii_letters
        file = root / "config.txt"
        file.write_text(value)
        self.git(root, "add", "config.txt")
        file.write_text("safe now")
        findings, count = guard.scan(root, True, True, False)
        self.assertEqual(count, 2)
        self.assertTrue(any(scope == "staged" and rule == "github-token" for scope, _, _, rule in findings))
        self.assertFalse(any(scope == "working-tree" for scope, _, _, _ in findings))

    def test_force_added_ignored_env_is_blocked(self):
        root = self.fixture()
        (root / ".gitignore").write_text(".env\n")
        (root / ".env").write_text("SAFE_PLACEHOLDER\n")
        self.git(root, "add", "-f", ".env")
        findings, _ = guard.scan(root, False, True, False)
        self.assertTrue(any(rule == "environment-file" for _, _, _, rule in findings))

    def test_deleted_secret_remains_blocked_in_history(self):
        root = self.fixture()
        value = "hf" + "_" + string.ascii_letters
        (root / "config.txt").write_text(value)
        self.git(root, "add", "config.txt")
        self.commit(root)
        self.git(root, "rm", "-q", "config.txt")
        self.commit(root)
        findings, _ = guard.scan(root, False, False, True)
        self.assertTrue(any(rule == "huggingface-token" for _, _, _, rule in findings))

    def test_historical_private_filename_is_checked_with_reused_blob(self):
        root = self.fixture()
        (root / ".env").write_text("{}")
        (root / "safe.json").write_text("{}")
        self.git(root, "add", ".env", "safe.json")
        self.commit(root)
        self.git(root, "rm", "-q", ".env")
        self.commit(root)
        findings, _ = guard.scan(root, False, False, True)
        self.assertTrue(any(name == ".env" and rule == "environment-file" for _, name, _, rule in findings))

    def test_detached_outgoing_commit_is_checked_beyond_local_refs(self):
        root = self.fixture()
        (root / "config.txt").write_text("safe")
        self.git(root, "add", "config.txt")
        self.commit(root)
        safe_head = self.git(root, "rev-parse", "HEAD").decode().strip()
        (root / "config.txt").write_text("gh" + "p_" + string.ascii_letters)
        self.git(root, "add", "config.txt")
        self.commit(root)
        outgoing = self.git(root, "rev-parse", "HEAD").decode().strip()
        self.git(root, "reset", "--hard", safe_head)
        self.assertFalse(guard.scan(root, False, False, True)[0])
        findings, _ = guard.scan(root, False, False, True, (outgoing,))
        self.assertTrue(any(rule == "github-token" for _, _, _, rule in findings))

    def test_history_batch_preserves_embedded_newlines_and_binary_header(self):
        root = self.fixture()
        (root / "renamed.txt").write_bytes(b"SQLite format 3\x00\nbinary\n")
        (root / "safe.txt").write_text("first line\nsecond line\n")
        self.git(root, "add", "renamed.txt", "safe.txt")
        self.commit(root)
        findings, count = guard.scan(root, False, False, True)
        self.assertEqual(count, 2)
        self.assertTrue(any(name == "renamed.txt" and rule == "sqlite-content" for _, name, _, rule in findings))
        self.assertFalse(any(name == "safe.txt" for _, name, _, _ in findings))

    def test_invalid_repo_fails_closed_and_diagnostics_are_redacted(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(RuntimeError):
                guard.scan(Path(directory), False, False, True)
        root = self.fixture()
        value = "sk-" + string.ascii_letters
        (root / "config.txt").write_text(value)
        self.git(root, "add", "config.txt")
        command = [str(Path(guard.__file__).resolve()), "--repo", str(root), "--staged"]
        import sys
        result = subprocess.run([sys.executable, *command], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.assertEqual(result.returncode, 1)
        self.assertNotIn(value.encode(), result.stdout + result.stderr)
        self.assertIn(b"provider-token", result.stderr)


if __name__ == "__main__":
    unittest.main()
