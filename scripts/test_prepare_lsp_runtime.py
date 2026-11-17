import importlib.util
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

spec = importlib.util.spec_from_file_location("lsp", Path(__file__).with_name("prepare-lsp-runtime.py"))
lsp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(lsp)


class RuntimeTests(unittest.TestCase):
    def test_offline_never_downloads_and_checksum_failure_is_fatal(self):
        with tempfile.TemporaryDirectory() as tmp, patch.object(lsp.urllib.request, "urlopen") as network:
            cache = Path(tmp)
            asset = {"file": "archive.zip", "url": "https://invalid.example/archive", "algorithm": "sha256", "digest": "0" * 64}
            with self.assertRaisesRegex(ValueError, "Missing cached"):
                lsp.fetch(asset, cache, True)
            (cache / asset["file"]).write_bytes(b"corrupt")
            with self.assertRaisesRegex(ValueError, "checksum mismatch"):
                lsp.fetch(asset, cache, False)
            network.assert_not_called()

    def test_archives_cannot_escape_destination(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            archive = root / "bad.zip"
            with zipfile.ZipFile(archive, "w") as out:
                out.writestr("package/../../escape", "bad")
            with self.assertRaises(ValueError):
                lsp.extract(archive, root / "zip", "package/")
            archive = root / "bad.tgz"
            with tarfile.open(archive, "w:gz") as out:
                entry = tarfile.TarInfo("package/link")
                entry.type = tarfile.SYMTYPE
                entry.linkname = "../../escape"
                out.addfile(entry)
            with self.assertRaises(ValueError):
                lsp.extract(archive, root / "tar", "package/")
            self.assertFalse((root / "escape").exists())

    def test_inventory_detects_missing_modified_and_extra_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            names = {arg for argv in lsp.PROGRAMS.values() for arg in argv if not arg.startswith("--")}
            names.add("node_modules/typescript/lib/tsserver.js")
            for name in names:
                dest = root / name
                dest.parent.mkdir(parents=True, exist_ok=True)
                dest.write_bytes(b"fixture")
            manifest = {"format": 1, "platform": "windows", "arch": "x86_64", "sha256": {name: lsp.digest(root / name) for name in names}}
            (root / "runtime.json").write_text(json.dumps(manifest))
            lsp.check(root)
            node = root / "node.exe"
            node.write_bytes(b"changed")
            with self.assertRaisesRegex(ValueError, "checksum"):
                lsp.check(root)
            node.unlink()
            with self.assertRaisesRegex(ValueError, "inventory"):
                lsp.check(root)
            node.write_bytes(b"fixture")
            (root / "unexpected").write_text("extra")
            with self.assertRaisesRegex(ValueError, "inventory"):
                lsp.check(root)


if __name__ == "__main__":
    unittest.main()
