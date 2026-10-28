import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
import zipfile

from prepare_release_assets import source_archives


class ReleaseAssetTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="imagelore-release-assets-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.collection = self.root / "third_party/distribution"
        self.collection.mkdir(parents=True)
        self.cache = self.root / "cargo/registry/cache/synthetic-registry"
        self.cache.mkdir(parents=True)
        self.contents = b"Synthetic published archive bytes; never executed."
        self.index = [{"package": "synthetic", "version": "1.0.0", "declaredLicense": "MIT", "crateSha256": hashlib.sha256(self.contents).hexdigest(), "sourceArchiveUrl": "https://example.invalid/never-requested"}]
        (self.collection / "rust-source-index.json").write_text(json.dumps(self.index), encoding="utf-8")
        self.cached = self.cache / "synthetic-1.0.0.crate"
        self.cached.write_bytes(self.contents)
        self.output = self.root / "sources.zip"
        self.environment = patch.dict(os.environ, {"CARGO_HOME": str(self.root / "cargo")})
        self.environment.start()
        self.addCleanup(self.environment.stop)

    def test_matching_source_archive_is_preserved(self):
        with patch("urllib.request.urlopen", side_effect=AssertionError("network must not be used")):
            self.assertEqual(source_archives(self.root, self.output), 1)
        with zipfile.ZipFile(self.output) as archive:
            self.assertEqual(archive.read("crates/synthetic-1.0.0.crate"), self.contents)
            self.assertEqual(len(archive.namelist()), 3)

    def test_wrong_cached_and_downloaded_bytes_are_refused(self):
        self.cached.write_bytes(b"tampered cache")
        with patch("urllib.request.urlopen", return_value=io.BytesIO(b"tampered download")):
            with self.assertRaisesRegex(ValueError, "Cargo.lock"):
                source_archives(self.root, self.output)

    def test_existing_source_archive_is_not_overwritten(self):
        self.output.write_bytes(b"existing original")
        with self.assertRaises(FileExistsError):
            source_archives(self.root, self.output)
        self.assertEqual(self.output.read_bytes(), b"existing original")

    def test_local_cli_refuses_before_reading_or_staging(self):
        environment = dict(os.environ, GITHUB_ACTIONS="false", RUNNER_ENVIRONMENT="local")
        result = subprocess.run([sys.executable, str(Path(__file__).with_name("prepare_release_assets.py")), "--output", str(self.root / "absent-output"), "--verification", "absent", "--application", "absent", "--installer", "absent"], env=environment, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(b"requires a GitHub-hosted runner", result.stderr)
        self.assertFalse((self.root / "absent-output").exists())


if __name__ == "__main__":
    unittest.main()
