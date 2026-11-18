"""Deterministic runtime-preparation boundary tests; no downloads or packages."""
import argparse
import importlib.util
import hashlib
from pathlib import Path
import sys
import tempfile
import unittest

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("runtime_prepare", Path(__file__).with_name("prepare-graphify-runtime.py"))
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)


class RuntimePreparation(unittest.TestCase):
    def test_shared_node_is_explicit_and_checksum_checked(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "graphify-runtime"
            root.mkdir()
            node = Path(directory) / "runtime/bin/node.exe"
            node.parent.mkdir(parents=True)
            node.write_bytes(b"fixture")
            manifest = {"node": "../runtime/bin/node.exe", "node_sha256": hashlib.sha256(b"fixture").hexdigest()}
            self.assertEqual(runtime.node_path(root, manifest), node.resolve())
            node.write_bytes(b"tampered")
            with self.assertRaisesRegex(ValueError, "checksum"):
                runtime.node_path(root, manifest)
            node.unlink()
            with self.assertRaises(FileNotFoundError):
                runtime.node_path(root, manifest)
            with self.assertRaises(ValueError):
                runtime.node_path(root, {"node": "../other/node.exe"})

    def test_program_paths_must_stay_in_the_bundle(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "python").write_text("fixture", encoding="utf-8")
            self.assertEqual(runtime.contained(root, "python"), (root / "python").resolve())
            for value in ["../python", str((root / "python").resolve())]:
                with self.assertRaises(ValueError):
                    runtime.contained(root, value)

    def test_venvs_are_refused_before_any_copy(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "venv"
            source.mkdir()
            (source / "pyvenv.cfg").write_text("home = elsewhere", encoding="utf-8")
            node = root / "node"
            node.write_text("fixture", encoding="utf-8")
            args = argparse.Namespace(python_root=source, node=node, node_license=node,
                out=root / "output", site_packages=None, wheelhouse=root, offline=True)
            with self.assertRaisesRegex(ValueError, "venv"):
                runtime.prepare(args)
            self.assertFalse(args.out.exists())

    def test_output_inside_source_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "LICENSE.txt").write_text("fixture", encoding="utf-8")
            node = root / "node"
            node.write_text("fixture", encoding="utf-8")
            args = argparse.Namespace(python_root=root, node=node, node_license=node,
                out=root / "output", site_packages=None, wheelhouse=root, offline=True)
            with self.assertRaisesRegex(ValueError, "inside"):
                runtime.prepare(args)
            self.assertFalse(args.out.exists())


if __name__ == "__main__":
    unittest.main()
