# SPDX-License-Identifier: GPL-3.0-only

import importlib.util
from pathlib import Path
import tempfile
import tomllib
import unittest

spec = importlib.util.spec_from_file_location("import_waypoints", Path(__file__).with_name("import-waypoints.py"))
waypoints = importlib.util.module_from_spec(spec)
spec.loader.exec_module(waypoints)


class ImportWaypointsTest(unittest.TestCase):
    owner = "0d37928c-63a2-4e8c-bd35-6bcbdee763a1"
    world = "b92e2cb0-2127-4ccb-8b6a-bdf11e781433"
    record = """WEST_GATE:
  ==: org.bukkit.Location
  world: nether
  x: -18.75
  y: 65
  z: -0.25
  pitch: -3.5
  yaw: 87
  name: '家 &b"West"'
"""

    def test_paper_yaml_import_resolves_dimension_and_preserves_position(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            original = root / "waypoints" / f"{self.owner}.yml"
            original.parent.mkdir()
            original.write_text(self.record)
            output = root / "waypoints-import"
            mappings = {"nether": "minecraft:the_nether"}
            self.assertEqual(waypoints.convert(root, output, mappings), 1)
            self.assertEqual(waypoints.convert(root, output, mappings), 1)
            archive = tomllib.loads(next(output.glob("*.toml")).read_text())
            self.assertEqual(archive["owner"], self.owner.replace("-", ""))
            self.assertEqual(archive["waypoints"], [{
                "id": "WEST_GATE", "name": '家 &b"West"', "dimension": "minecraft:the_nether",
                "position": [-18.75, 65, -0.25], "rotation": [87, -3.5],
            }])
            self.assertEqual(original.read_text(), self.record)

    def test_sync_shadowing_distinguishes_empty_from_missing_and_checks_uuid(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            legacy = root / "waypoints" / f"{self.owner}.yml"
            legacy.parent.mkdir()
            legacy.write_text(self.record)
            local = root / "player-sync" / "east" / "waypoints" / legacy.name
            local.parent.mkdir(parents=True)
            local.write_text("")
            output = root / "synced"
            self.assertEqual(waypoints.convert(root, output, {}, "east"), 1)
            self.assertNotIn("waypoints", tomllib.loads(next(output.glob("*.toml")).read_text()))
            local.write_text(self.record + f"  world_uuid: {self.world}\n")
            with self.assertRaisesRegex(ValueError, "explicit mapping"):
                waypoints.convert(root, root / "bad", {"nether": "minecraft:the_nether"}, "east")
            output = root / "mapped"
            self.assertEqual(waypoints.convert(root, output, {self.world: "example:east"}, "east"), 1)
            contents = tomllib.loads(next(output.glob("*.toml")).read_text())
            self.assertEqual(contents["waypoints"][0]["dimension"], "example:east")

    def test_duplicate_locations_never_replace_existing_output(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            legacy = root / "waypoints" / f"{self.owner}.yml"
            legacy.parent.mkdir()
            legacy.write_text(self.record + self.record)
            output = root / "import"
            with self.assertRaisesRegex(ValueError, "repeated waypoint key"):
                waypoints.convert(root, output, {"nether": "minecraft:the_nether"})
            self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
