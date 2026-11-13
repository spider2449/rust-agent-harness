"""Offline compatibility tests using the pinned Graphify Python environment."""

import tempfile
import unittest
import sys
from pathlib import Path

sys.dont_write_bytecode = True
import extract


class CargoManifestCompatibility(unittest.TestCase):
    def test_plain_bom_and_member_manifests_share_the_compatible_reader(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            plain = root / "Cargo.toml"
            member = root / "member" / "Cargo.toml"
            member.parent.mkdir()
            plain.write_text('[package]\nname = "plain"\n', encoding="utf-8")
            member.write_text('[package]\nname = "bom-member"\n[dependencies]\nserde = "1"\n', encoding="utf-8-sig")
            self.assertEqual(extract.load_manifest(plain)["package"]["name"], "plain")
            self.assertEqual(extract.load_manifest(member)["package"]["name"], "bom-member")
            self.assertEqual(extract.cargo_introspect.package_name(member), "bom_member")
            graph = extract.manifest_ingest.extract_package_manifest(member)
            self.assertEqual(graph["nodes"][0]["id"], "pkg_bom_member")
            self.assertEqual(graph["edges"][0]["target"], "pkg_serde")

    def test_invalid_toml_and_invalid_encoding_remain_errors(self):
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "Cargo.toml"
            manifest.write_text("\ufeff[invalid syntax", encoding="utf-8")
            with self.assertRaises(ValueError):
                extract.load_manifest(manifest)
            manifest.write_bytes(b"\xff\xfe[package]")
            with self.assertRaises(UnicodeError):
                extract.load_manifest(manifest)


if __name__ == "__main__":
    unittest.main()
