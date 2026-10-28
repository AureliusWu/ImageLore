import json
from pathlib import Path
import shutil
import tempfile
import unittest

from verify_distribution_notices import verify

ROOT = Path(__file__).resolve().parents[1]


class DistributionNoticeTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="imagelore-notices-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.collection = self.root / "third_party/distribution"
        shutil.copytree(ROOT / "third_party/distribution", self.collection)
        for name in ("scripts/public_attribution_hashes.json", "src-tauri/Cargo.lock"):
            target = self.root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, target)
        self.entry = json.loads((self.collection / "notice-manifest.json").read_bytes())["files"][0]

    def test_exact_source_and_installed_bytes(self):
        self.assertEqual(verify(self.root)["files"], 790)
        installed = self.root / "installed/licenses"
        shutil.copytree(self.collection, installed)
        self.assertEqual(verify(self.root, installed)["state"], "PASS_INSTALLED_DISTRIBUTION_NOTICES")
        (installed / self.entry["path"]).write_bytes(b"changed installed text")
        with self.assertRaisesRegex(ValueError, "installed notice bytes"):
            verify(self.root, installed)

    def test_changed_source_text_refused(self):
        (self.collection / self.entry["path"]).write_bytes(b"changed source text")
        with self.assertRaisesRegex(ValueError, "source identity"):
            verify(self.root)

    def test_missing_notice_refused(self):
        (self.collection / self.entry["path"]).unlink()
        with self.assertRaisesRegex(ValueError, "absent"):
            verify(self.root)

    def test_unexpected_file_refused(self):
        (self.collection / "unreviewed.txt").write_text("unreviewed", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "unexpected files"):
            verify(self.root)

    def test_source_index_does_not_override_lock(self):
        path = self.collection / "rust-source-index.json"
        index = json.loads(path.read_bytes())
        index[0]["crateSha256"] = "0" * 64
        path.write_text(json.dumps(index), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "Cargo.lock"):
            verify(self.root)

    def test_combined_notice_is_byte_bound(self):
        path = self.collection / "THIRD_PARTY_NOTICES.txt"
        path.write_bytes(path.read_bytes() + b"\n")
        with self.assertRaisesRegex(ValueError, "combined notice"):
            verify(self.root)


if __name__ == "__main__":
    unittest.main()
