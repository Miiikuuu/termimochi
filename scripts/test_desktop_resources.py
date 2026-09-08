"""Regression tests for resource drift; all mutations stay in temporary copies."""

from pathlib import Path
import copy
import shutil
import tempfile
import unittest
import xml.etree.ElementTree as ET

import validate_desktop_resources as checks


class ResourceValidationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="termimochi resource test ")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        for directory in ["data/icons", "crates/termimochi/resources/icons"]:
            shutil.copytree(checks.ROOT / directory, self.root / directory)
        for relative in [
            checks.MANIFEST,
            Path("crates/termimochi/resources/fastfetch/LICENSE"),
            Path("crates/termimochi/resources/fastfetch/README.md"),
        ]:
            destination = self.root / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(checks.ROOT / relative, destination)
        self.xml = ET.parse(self.root / checks.MANIFEST)
        self.group = self.xml.getroot().find("gresource")
        self.expected = checks.expected_resources(self.root)
        self.bundle = self.root / "test.gresource"

    def save_manifest(self):
        self.xml.write(self.root / checks.MANIFEST, encoding="utf-8", xml_declaration=True)

    def test_current_bundle_contains_exact_paths_and_source_bytes(self):
        checks.validate_manifest(self.root, self.expected)
        checks.compile_bundle(self.root, self.bundle)
        checks.validate_bundle(self.bundle, self.expected)

    def test_new_shared_icon_does_not_need_a_magic_resource_count(self):
        relative = Path("scalable/apps/new-test-icon.svg")
        source = self.root / "crates/termimochi/resources/icons" / relative
        source.write_text('<svg xmlns="http://www.w3.org/2000/svg"/>')
        shutil.copyfile(source, self.root / "data/icons/hicolor" / relative)
        entry = ET.SubElement(self.group, "file", alias=f"icons/{relative.as_posix()}")
        entry.text = f"resources/icons/{relative.as_posix()}"
        self.save_manifest()
        expected = checks.expected_resources(self.root)
        self.assertEqual(len(expected), len(self.expected) + 1)
        checks.validate_manifest(self.root, expected)
        checks.compile_bundle(self.root, self.bundle)
        checks.validate_bundle(self.bundle, expected)

    def test_wrong_alias_fails_even_when_count_is_unchanged(self):
        self.group[0].set("alias", "licenses/wrong-name.txt")
        self.save_manifest()
        with self.assertRaisesRegex(ValueError, "Resource manifest: missing="):
            checks.validate_manifest(self.root, self.expected)
        checks.compile_bundle(self.root, self.bundle)
        with self.assertRaisesRegex(ValueError, "Compiled resources: missing="):
            checks.validate_bundle(self.bundle, self.expected)

    def test_missing_license_is_not_silently_accepted(self):
        self.group.remove(self.group[0])
        self.save_manifest()
        with self.assertRaisesRegex(ValueError, "Resource manifest: missing="):
            checks.validate_manifest(self.root, self.expected)

    def test_duplicate_alias_fails(self):
        self.group.append(copy.deepcopy(self.group[0]))
        self.save_manifest()
        with self.assertRaisesRegex(ValueError, "Duplicate resource alias"):
            checks.validate_manifest(self.root, self.expected)

    def test_wrong_source_fails(self):
        self.group[0].text = self.group[1].text
        self.save_manifest()
        with self.assertRaisesRegex(ValueError, "Wrong source"):
            checks.validate_manifest(self.root, self.expected)

    def test_missing_action_icon_fails(self):
        key = f"{checks.PREFIX}/icons/scalable/actions/termimochi-open-symbolic.svg"
        self.expected[key].unlink()
        with self.assertRaisesRegex(ValueError, "Missing application action icon"):
            checks.expected_resources(self.root)

    def test_corrupted_shared_icon_fails(self):
        key = f"{checks.PREFIX}/icons/scalable/apps/{checks.APP_ID}.svg"
        self.expected[key].write_bytes(b"corrupted")
        with self.assertRaisesRegex(ValueError, "Desktop icon copy differs"):
            checks.expected_resources(self.root)

    def test_stale_bundle_bytes_fail(self):
        checks.compile_bundle(self.root, self.bundle)
        source = self.expected[f"{checks.PREFIX}/licenses/Fastfetch-MIT.txt"]
        source.write_bytes(b"changed license")
        with self.assertRaisesRegex(ValueError, "Embedded resource content differs"):
            checks.validate_bundle(self.bundle, self.expected)


if __name__ == "__main__":
    unittest.main()
