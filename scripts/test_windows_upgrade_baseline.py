"""Exercise pinned baseline identity on real, isolated Git working trees."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

from check_windows_upgrade_baseline import verify


class BaselineIdentityTests(unittest.TestCase):
    def setUp(self):
        self.fixture = tempfile.TemporaryDirectory(prefix="imagelore-baseline-identity-")
        self.addCleanup(self.fixture.cleanup)
        self.root = Path(self.fixture.name)
        (self.root / "src-tauri/src").mkdir(parents=True)
        (self.root / "VERSION").write_text("0.25.1\n", encoding="utf-8")
        self.original = '[package]\nname="synthetic"\nversion="0.25.1"\n[build-dependencies]\ntauri-build={version="2"}\n'
        self.toml = self.root / "src-tauri/Cargo.toml"
        self.toml.write_text(self.original, encoding="utf-8")
        (self.root / "src-tauri/src/lib.rs").write_text("// synthetic fixture, never compiled\n", encoding="utf-8")
        self.git("-c", "init.templateDir=", "init", "-q")
        self.git("config", "core.hooksPath", str(self.root / "disabled-hooks"))
        self.git("config", "commit.gpgSign", "false")
        self.git("config", "core.fsmonitor", "false")
        self.git("add", "--", "VERSION", "src-tauri")
        self.git("-c", "user.name=ImageLore synthetic fixture", "-c", "user.email=synthetic@example.invalid", "commit", "-qm", "Synthetic baseline identity")
        self.commit = self.git("rev-parse", "HEAD").decode().strip()

    def git(self, *args):
        return subprocess.check_output(["git", "-C", str(self.root), *args], stderr=subprocess.PIPE)

    def check(self):
        return verify(self.root, self.commit, "0.25.1")

    def test_exact_source(self):
        self.assertEqual(self.check()["cargoNormalization"], "none")

    def test_only_empty_build_features(self):
        self.toml.write_text(self.original.replace('version="2"}', 'version="2",features=[]}'), encoding="utf-8")
        result = self.check()
        self.assertEqual(result["cargoNormalization"], "tauri-build-added-empty-features-only")
        self.assertNotEqual(result["originalCargoTomlSha256"], result["builtCargoTomlSha256"])

    def test_nonempty_features_refused(self):
        self.toml.write_text(self.original.replace('version="2"}', 'version="2",features=["unexpected"]}'), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "Cargo semantics"):
            self.check()

    def test_dependency_version_refused(self):
        self.toml.write_text(self.original.replace('version="2"}', 'version="3",features=[]}'), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "Cargo semantics"):
            self.check()

    def test_other_tracked_source_refused(self):
        (self.root / "src-tauri/src/lib.rs").write_text("// changed fixture source\n", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "outside Cargo.toml"):
            self.check()

    def test_unrelated_manifest_edit_refused(self):
        self.toml.write_text(self.original + "# unrelated edit\n", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "not the known"):
            self.check()

    def test_other_table_change_with_empty_features_refused(self):
        self.toml.write_text(self.original.replace('version="2"}', 'version="2",features=[]}') + '[features]\nunexpected=[]\n', encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "Cargo semantics"):
            self.check()

    def test_lockfile_change_refused(self):
        lock = self.root / "src-tauri/Cargo.lock"
        lock.write_text("# synthetic original lock\n", encoding="utf-8")
        self.git("add", "--", "src-tauri/Cargo.lock")
        self.git("-c", "user.name=ImageLore synthetic fixture", "-c", "user.email=synthetic@example.invalid", "commit", "-qm", "Synthetic lock fixture")
        self.commit = self.git("rev-parse", "HEAD").decode().strip()
        lock.write_text("# changed synthetic lock\n", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "outside Cargo.toml"):
            self.check()

    def test_invalid_generated_schema_refused(self):
        path = self.root / "src-tauri/gen/schemas/windows-schema.json"
        path.parent.mkdir(parents=True)
        path.write_text("invalid-json", encoding="utf-8")
        with self.assertRaises(ValueError):
            self.check()

    def test_wrong_version_refused(self):
        (self.root / "VERSION").write_text("0.26.0\n", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "working baseline version"):
            self.check()

    def test_wrong_commit_refused(self):
        with self.assertRaisesRegex(ValueError, "HEAD differs"):
            verify(self.root, "0" * 40, "0.25.1")

    def test_known_generated_schema_recorded(self):
        path = self.root / "src-tauri/gen/schemas/windows-schema.json"
        path.parent.mkdir(parents=True)
        path.write_text(json.dumps({"synthetic": True}), encoding="utf-8")
        self.assertEqual(list(self.check()["generatedSchemaOutputs"]), [path.relative_to(self.root).as_posix()])

    def test_unexpected_extra_file_refused(self):
        (self.root / "untracked-source.rs").write_text("// unexpected fixture\n", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "unexpected untracked"):
            self.check()


if __name__ == "__main__":
    unittest.main()
