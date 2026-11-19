import importlib.util
import json
import hashlib
import shutil
from pathlib import Path
import tarfile
import tempfile
import unittest
from types import SimpleNamespace
from unittest.mock import patch
import zipfile

spec = importlib.util.spec_from_file_location("lsp", Path(__file__).with_name("prepare-lsp-runtime.py"))
lsp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(lsp)


class RuntimeTests(unittest.TestCase):
    def test_shared_node_relocation_mismatch_and_tampering(self):
        with tempfile.TemporaryDirectory() as tmp:
            package = Path(tmp) / "package"
            root = package / "lsp-runtime"
            graph = package / "graphify-runtime"
            graph.mkdir(parents=True)
            (graph / "runtime.json").write_text(json.dumps({"node": "node.exe", "python": "python/python.exe"}))
            names = {arg for argv in lsp.PROGRAMS.values() for arg in argv if not arg.startswith("--")}
            names.add("node_modules/typescript/lib/tsserver.js")
            for name in names:
                dest = root / name
                dest.parent.mkdir(parents=True, exist_ok=True)
                dest.write_bytes(b"fixture")
            manifest = {"format": 1, "platform": "windows", "arch": "x86_64", "sha256": {name: lsp.digest(root / name) for name in names}}
            (root / "runtime.json").write_text(json.dumps(manifest))
            shared = graph / "node.exe"
            shared.write_bytes(b"other version")
            lsp.share_node(root)
            self.assertTrue((root / "node.exe").exists())
            shared.write_bytes(b"fixture")
            source = Path(__file__).with_name("update.ps1").read_text(encoding="utf-8-sig")
            code = source.split("manifest = json.loads((lsp / 'runtime.json')", 1)[1].split("\n'@", 1)[0]
            code = "manifest = json.loads((lsp / 'runtime.json')" + code
            context = {"lsp": root, "graph": graph, "json": json, "hashlib": hashlib, "shutil": shutil, "pathlib": __import__("pathlib"),
                       "sys": SimpleNamespace(argv=["-", str(graph), str(root), "--share-node"])}
            def run_staging():
                context["graph_manifest"] = json.loads((graph / "runtime.json").read_text())
                context["python_name"] = context["graph_manifest"]["python"]
                context["python_root"] = graph / "python"
                exec(code, context)

            run_staging()
            self.assertFalse((root / "node.exe").exists())
            self.assertFalse((graph / "node.exe").exists())
            run_staging()  # updater accepts an already shared release
            lsp.check(root)
            # Restore the standalone fixture to exercise package deduplication too.
            (root / "node.exe").write_bytes(b"fixture")
            (root / "runtime.json").write_text(json.dumps(manifest))
            (graph / "runtime.json").write_text(json.dumps({"node": "node.exe", "python": "python/python.exe"}))
            (graph / "node.exe").write_bytes(b"fixture")
            (package / "runtime/bin/node.exe").unlink()
            lsp.share_node(root)
            self.assertFalse((root / "node.exe").exists())
            self.assertFalse((graph / "node.exe").exists())
            lsp.share_node(root)  # idempotent
            # The updater stages already shared input without changing it.
            stage = Path(tmp) / "staged"
            (graph / "python").mkdir()
            (graph / "python/python.exe").write_bytes(b"interpreter")
            context["sys"].argv = ["-", str(graph), str(root), "--stage", str(stage), "--share-node", "--share-python"]
            run_staging()
            lsp.check(stage / "lsp-runtime")
            self.assertEqual((stage / "runtime/bin/node.exe").read_bytes(), b"fixture")
            self.assertFalse((stage / "graphify-runtime/node.exe").exists())
            self.assertFalse((stage / "graphify-runtime/python").exists())
            self.assertEqual((stage / "runtime/bin/python.exe").read_bytes(), b"interpreter")
            self.assertEqual(json.loads((stage / "graphify-runtime/runtime.json").read_text())["python"],
                             "../runtime/bin/python.exe")
            # Packaging likewise accepts a previously shared distribution.
            lsp.materialize_node(root, stage / "lsp-runtime", True)
            lsp.materialize_node(graph, stage / "graphify-runtime", False)
            lsp.share_node(stage / "lsp-runtime")
            lsp.check(stage / "lsp-runtime")
            relocated = Path(tmp) / "relocated package"
            package.rename(relocated)
            root = relocated / "lsp-runtime"
            lsp.check(root)
            shared = relocated / "runtime/bin/node.exe"
            shared.write_bytes(b"tampered")
            with self.assertRaisesRegex(ValueError, "checksum"):
                lsp.check(root)
            shared.unlink()
            with self.assertRaises(FileNotFoundError):
                lsp.check(root)
            with self.assertRaisesRegex(ValueError, "Unsupported"):
                lsp.node_path(root, {"node": "../../node.exe"})

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
