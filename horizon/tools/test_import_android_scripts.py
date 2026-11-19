# SPDX-License-Identifier: GPL-3.0-only

import importlib.util
import tempfile
import tomllib
import unittest
from pathlib import Path


SPEC = importlib.util.spec_from_file_location(
    "import_android_scripts", Path(__file__).with_name("import-android-scripts.py"),
)
IMPORTER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(IMPORTER)


class ImportAndroidScriptsTests(unittest.TestCase):
    def test_archives_real_sfs_fields_without_replacing_existing_file(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "scripts" / "FARMER"
            source.mkdir(parents=True)
            (source / "Example 1.sfs").write_text(
                "name: '农夫 Script'\n"
                "author: 3d93f77b-fadc-463f-9e55-cf359006ced1\n"
                "author_name: Example\n"
                "code: START-FARM_DOWN-REPEAT\n"
                "downloads: 15\n"
                "android: FARMER\n"
                "rating:\n"
                "  positive:\n"
                "  - b09045b6-50b9-48e8-b072-4e9844f27d9e\n"
                "  negative: []\n",
                encoding="utf-8",
            )
            (source / "Incomplete.sfs").write_text("name: Incomplete\n")
            output = root / "android-scripts-import.toml"
            self.assertEqual(IMPORTER.import_scripts(source.parent, output), 1)
            archive = tomllib.loads(output.read_text(encoding="utf-8"))
            self.assertEqual(archive["format"], 1)
            self.assertEqual(archive["scripts"], [{
                "name": "农夫 Script",
                "author": "3d93f77bfadc463f9e55cf359006ced1",
                "author_name": "Example",
                "category": "FARMER",
                "code": "START-FARM_DOWN-REPEAT",
                "downloads": 15,
                "positive": ["b09045b650b948e8b0724e9844f27d9e"],
                "negative": [],
            }])
            original = output.read_bytes()
            with self.assertRaises(FileExistsError):
                IMPORTER.import_scripts(source.parent, output)
            self.assertEqual(output.read_bytes(), original)
