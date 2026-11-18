#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only

import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib
import unittest
from zipfile import ZIP_DEFLATED, ZipFile

import yaml


MODULE_PATH = Path(__file__).with_name("import-block-storage.py")
SPEC = importlib.util.spec_from_file_location("slimefun_storage_import", MODULE_PATH)
IMPORT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(IMPORT)
RESEARCH_SPEC = importlib.util.spec_from_file_location(
    "slimefun_research_import", MODULE_PATH.with_name("import-research.py"))
RESEARCH = importlib.util.module_from_spec(RESEARCH_SPEC)
RESEARCH_SPEC.loader.exec_module(RESEARCH)


class BlockStorageTest(unittest.TestCase):
    def test_imports_the_paper_backup_zip_with_authoritative_profiles_twice(self):
        owner = "341da016-f646-4030-af46-6e652265fa34"
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            profiles = root / "Players"
            profiles.mkdir()
            file = profiles / f"{owner}.yml"
            file.write_text(yaml.safe_dump({
                "pm-snapshot-revision": 18,
                "pm-research-keys": ["addon:research", "slimefun:lumps"],
                "researches": {31: True},
                "backpacks": {"14": {"size": 9, "contents": {"7": {
                    "==": "org.bukkit.inventory.ItemStack", "schema_version": 1,
                    "id": "minecraft:diamond", "DataVersion": 5013, "count": 3,
                    "components": {"minecraft:custom_data": '{example:"kept"}'},
                }}}},
            }))
            zipped = root / "paper.zip"
            with ZipFile(zipped, "w", compression=ZIP_DEFLATED) as archive:
                archive.writestr("stored-blocks/world/CARGO_NODE_INPUT.sfb", yaml.safe_dump({
                    "world;3;64;8": '{"id":"CARGO_NODE_INPUT"}',
                }))
                archive.writestr("stored-inventories/world;3;64;8.sfi", "preset: CARGO_NODE_INPUT\n")
                archive.writestr("stored-chunks/chunks.sfc", yaml.safe_dump({
                    "world;Chunk;0;0": '{"slimefun-oil":"0"}',
                }))
            archive_original = zipped.read_bytes()
            destination = root / "storage-import"
            command = [sys.executable, MODULE_PATH, zipped, destination, "--player-profiles", profiles]
            subprocess.run(command, check=True, capture_output=True)
            subprocess.run(command, check=True, capture_output=True)
            self.assertEqual(zipped.read_bytes(), archive_original)
            self.assertEqual(tomllib.loads((destination / "index.toml").read_text()), {
                "format": 1, "blocks": 1, "inventories": 1, "chunks": 1, "backpacks": 1,
                "geo_chunks": 1, "oil_reserves": 1,
            })
            converted = tomllib.loads((destination / "backpacks" / (owner.replace("-", "") + ".toml")).read_text())
            self.assertEqual(converted["paper_profile"], file.read_text())
            stack = converted["backpacks"][0]["paper_items"][0]
            self.assertEqual((stack["slot"], stack["count"]), (7, 3))
            research, unknown = RESEARCH.collect(profiles, {3: {"slimefun:lumps"}, 31: {"slimefun:cactus_armor"}})
            self.assertFalse(unknown)
            self.assertEqual(research, {owner.replace("-", ""): {"slimefun:lumps", "slimefun:cactus_armor"}})
            research_output = root / "research-import"
            RESEARCH.write_imports(research_output, research)
            RESEARCH.write_imports(research_output, research)
            player_key = owner.replace("-", "")
            research_file = research_output / f"{player_key}.txt"
            old = research_file.read_bytes()
            another = "0" * 32
            with self.assertRaisesRegex(ValueError, "Existing research import differs"):
                RESEARCH.write_imports(research_output, {
                    another: {"slimefun:lumps"}, player_key: {"slimefun:lumps"},
                })
            self.assertFalse((research_output / f"{another}.txt").exists())
            self.assertEqual(research_file.read_bytes(), old)

    def test_a_paper_backup_with_an_escaping_entry_publishes_nothing(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            zipped = root / "paper.zip"
            with ZipFile(zipped, "w") as archive:
                archive.writestr("stored-inventories/world;3;64;8.sfi", "preset: CARGO_NODE_INPUT\n")
                archive.writestr("../bad.sfi", "ignore")
            output = root / "storage-import"
            result = subprocess.run([sys.executable, MODULE_PATH, zipped, output], capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Invalid path in Paper backup", result.stderr)
            self.assertFalse(output.exists())

    def test_backpack_archive_preserves_source_and_refuses_ambiguous_numeric_aliases(self):
        owner = "341da016-f646-4030-af46-6e652265fa34"
        original = """backpacks:
  3:
    size: 9
  '3':
    size: 18
  '4':
    size: 9
    contents:
      invalid_slot: null
  '5':
    size: 54
"""
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            profiles = root / "Players"
            profiles.mkdir()
            source = profiles / f"{owner}.yml"
            source.write_text(original)
            archive = IMPORT.collect_backpacks(root)
            output = root / "storage-import"
            IMPORT.publish(output, {}, players=archive)
            IMPORT.publish(output, {}, players=archive)
            destination = output / "backpacks" / (owner.replace("-", "") + ".toml")
            profile = tomllib.loads(destination.read_text())
            self.assertEqual(source.read_text(), original)
            self.assertEqual(profile["paper_profile"], original)
            self.assertIn("Repeated backpack ID '3'.", profile["paper_profile_issues"])
            self.assertEqual([record["id"] for record in profile["backpacks"]], [4, 5])
            self.assertEqual(profile["backpacks"][0]["paper_item_issues"],
                             ["Invalid backpack inventory slot 'invalid_slot'."])

    def test_archives_partial_and_depleted_paper_geo_reserves(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            source = root / "paper"
            chunks = source / "stored-chunks"
            chunks.mkdir(parents=True)
            file = chunks / "chunks.sfc"
            file.write_text(yaml.safe_dump({
                "survival;Chunk;-1;-2": ('{"slimefun-oil":"0","slimefun-salt":"2",'
                                         '"slimefun-nether_ice":"45","slimefun-uranium":"0"}'),
                "survival;Chunk;1;-2": '{"slimefun-oil":"79"}',
                "survival;Chunk;0;0": '{"slimefun-salt":"1"}',
            }))
            dimension = "example:survival"
            reserves = IMPORT.collect_geo(source, {"survival": dimension})
            self.assertEqual(reserves, {
                (dimension, -1, -2): {"slimefun-oil": 0, "slimefun-salt": 2,
                                      "slimefun-nether_ice": 45, "slimefun-uranium": 0},
                (dimension, 1, -2): {"slimefun-oil": 79},
                (dimension, 0, 0): {"slimefun-salt": 1},
            })
            output = root / "archive"
            IMPORT.publish(output, {}, geo=reserves)
            archived = output / "geo" / dimension.encode().hex() / "-1_-2.toml"
            self.assertEqual(tomllib.loads(archived.read_text()), {
                "format": 1, "dimension": dimension, "chunk": [-1, -2],
                "slimefun-oil": 0, "slimefun-salt": 2,
                "slimefun-nether_ice": 45, "slimefun-uranium": 0,
            })
            index = tomllib.loads((output / "index.toml").read_text())
            self.assertEqual(index["oil_reserves"], 2)
            self.assertEqual(index["geo_chunks"], 3)
            for invalid in ('-1', '2147483648', '002'):
                file.write_text(yaml.safe_dump({
                    "survival;Chunk;-1;-2": f'{{"slimefun-nether_ice":"{invalid}"}}',
                }))
                with self.assertRaisesRegex(ValueError, "Invalid Paper GEO supply"):
                    IMPORT.collect_geo(source, {"survival": dimension})

    def test_archives_paper_metadata_and_inventory_without_overwriting(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            paper = root / "paper"
            blocks = paper / "stored-blocks" / "survival"
            inventory = paper / "stored-inventories"
            blocks.mkdir(parents=True)
            inventory.mkdir()
            payload = {
                "id": "CARGO_NODE_OUTPUT_ADVANCED",
                "frequency": "15",
                "opaque": "utf-8 😀 and DEL\x7f",
                "nullable": None,
            }
            source_text = yaml.safe_dump({
                "survival;-1;72;-17": json.dumps(payload),
                "survival;0;72;0": json.dumps({"id": "OUTPUT_CHEST"}),
            })
            block_file = blocks / "component.sfb"
            block_file.write_text(source_text)
            original_inventory = (
                "preset: CARGO_NODE_OUTPUT_ADVANCED\r\n"
                "0:\r\n  ==: org.bukkit.inventory.ItemStack\r\n"
                "  type: STONE\r\n  amount: 24\r\n"
            )
            paper_inventory = inventory / "survival;-1;72;-17.sfi"
            paper_inventory.write_bytes(original_inventory.encode())
            dimension = "example:side/world"
            records = IMPORT.collect(paper, {"survival": dimension})
            output = root / "storage-import"
            IMPORT.publish(output, records)
            IMPORT.publish(output, records)

            chunk = output / dimension.encode().hex() / "-1_-2.toml"
            row = tomllib.loads(chunk.read_text())["records"][0]
            self.assertEqual(row["position"], [-1, 72, -17])
            self.assertEqual(row["inventory_slots"], 1)
            self.assertEqual(row["paper_inventory"], original_inventory)
            self.assertEqual(row["inventory_preset"], "CARGO_NODE_OUTPUT_ADVANCED")
            self.assertEqual(row["paper_items"], [])
            self.assertRegex(row["paper_item_issues"][0], r"Slot 0: resave this legacy item")
            self.assertEqual(row["null_metadata_keys"], ["nullable"])
            self.assertEqual(row["metadata"], {
                key: value for key, value in payload.items() if value is not None
            })
            self.assertEqual(tomllib.loads((output / "index.toml").read_text()),
                             {"format": 1, "blocks": 2, "inventories": 1, "chunks": 2})
            with self.assertRaisesRegex(ValueError, "Existing import differs"):
                IMPORT.publish(output, {})
            self.assertEqual(block_file.read_text(), source_text)
            self.assertEqual(paper_inventory.read_bytes(), original_inventory.encode())
            self.assertEqual(tomllib.loads(chunk.read_text())["records"][0], row)

    def test_refuses_a_duplicate_yml_location(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            blocks = root / "stored-blocks" / "world"
            blocks.mkdir(parents=True)
            payload = "world;1;2;3: '{\"id\": \"OUTPUT_CHEST\"}'\n"
            (blocks / "duplicate.sfb").write_text(payload * 2)
            with self.assertRaisesRegex(ValueError, "Duplicate YAML storage key"):
                IMPORT.collect(root, IMPORT.WORLDS)

    def test_component_stacks_keep_their_slots_data_versions_and_snbt(self):
        components = {
            "minecraft:custom_data": '{PublicBukkitValues:{"slimefun:slimefun_item":"BASIC_CIRCUIT_BOARD"},keys:[I;4,8],active:1b}',
            "minecraft:custom_name": '{text:"电路板",color:"yellow"}',
            "!minecraft:repair_cost": "{}",
        }
        paper_stack = {
            "==": "org.bukkit.inventory.ItemStack",
            "schema_version": 1,
            "id": "minecraft:gold_ingot",
            "count": 4,
            "DataVersion": 5013,
            "components": components,
        }
        empty = {**paper_stack, "id": "minecraft:air", "components": {}}
        with tempfile.TemporaryDirectory() as tmp:
            paper = Path(tmp)
            entries = paper / "stored-inventories"
            entries.mkdir()
            name = "world;-1;72;-17.sfi"
            raw = yaml.safe_dump({
                "preset": "ELECTRIC_FURNACE",
                "25": None,
                "19": paper_stack,
                "20": empty,
                "24": {**paper_stack, "count": 1, "DataVersion": 5015},
            }, allow_unicode=True)
            (entries / name).write_text(raw)
            records = IMPORT.collect(paper, IMPORT.WORLDS)
            output = paper / "output"
            IMPORT.publish(output, records)
            bucket = "minecraft:overworld".encode().hex()
            row = tomllib.loads((output / bucket / "-1_-2.toml").read_text())["records"][0]
            self.assertEqual(row["paper_inventory"], raw)
            self.assertEqual(row["inventory_slots"], 2)
            self.assertEqual(row["inventory_preset"], "ELECTRIC_FURNACE")
            self.assertNotIn("paper_item_issues", row)
            self.assertEqual(row["paper_items"], [
                {"slot": 19, "data_version": 5013, "id": "minecraft:gold_ingot",
                 "count": 4, "components": components},
                {"slot": 24, "data_version": 5015, "id": "minecraft:gold_ingot",
                 "count": 1, "components": components},
            ])

    def test_invalid_or_ambiguous_slots_report_issues_with_other_slots_preserved(self):
        stack = {
            "==": "org.bukkit.inventory.ItemStack",
            "schema_version": 1,
            "id": "minecraft:stone",
            "DataVersion": 5013,
        }
        preset, items, issues = IMPORT.inventory_snapshot({
            "preset": "CARGO_NODE_INPUT",
            19: stack,
            "19": stack,
            "20": {**stack, "count": True},
            "21": {**stack, "components": {"minecraft:custom_data": {"typed": 4}}},
            "54": stack,
        })
        self.assertEqual(preset, "CARGO_NODE_INPUT")
        self.assertEqual([item["slot"] for item in items], [19])
        self.assertEqual(len(issues), 4)
        self.assertRegex(issues[0], "repeated Paper slot")
        self.assertRegex(issues[1], "invalid item count")
        self.assertRegex(issues[2], "named SNBT")
        self.assertRegex(issues[3], "Slot 54")

    def test_archives_global_and_player_owned_inventories_separately(self):
        owner = "341da016-f646-4030-af46-6e652265fa34"
        stack = {
            "==": "org.bukkit.inventory.ItemStack",
            "schema_version": 1,
            "id": "minecraft:stone",
            "DataVersion": 5013,
        }
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            global_dir = root / "universal-inventories"
            global_dir.mkdir()
            (global_dir / "TEST_SHARED.sfi").write_text(yaml.safe_dump({
                "preset": "TEST_SHARED",
                "8": stack,
                "17": None,
            }))
            profiles = root / "Players"
            profiles.mkdir()
            original = yaml.safe_dump({
                "researches": {9: True},
                "backpacks": {"2": {"size": 18, "contents": {
                    "17": stack,
                    "18": stack,
                }}, "7": {"size": 9}},
            })
            (profiles / f"{owner}.yml").write_text(original)
            archives = IMPORT.collect_universal(root)
            players = IMPORT.collect_backpacks(root)
            destination = root / "imported"
            IMPORT.publish(destination, {}, archives, players)

            global_row = tomllib.loads((destination / "universal" /
                                       ("TEST_SHARED".encode().hex() + ".toml")).read_text())
            self.assertEqual(global_row["preset"], "TEST_SHARED")
            self.assertEqual(global_row["paper_slots"], [8, 17])
            self.assertEqual(global_row["paper_items"][0]["slot"], 8)
            profile = tomllib.loads((destination / "backpacks" /
                                     (owner.replace("-", "") + ".toml")).read_text())
            self.assertEqual(profile["paper_profile"], original)
            first, empty = profile["backpacks"]
            self.assertEqual((first["id"], first["size"]), (2, 18))
            self.assertEqual([slot["slot"] for slot in first["paper_items"]], [17])
            self.assertIn("Slot 18", first["paper_item_issues"][0])
            self.assertEqual((empty["id"], empty["size"], empty["paper_items"]), (7, 9, []))
            index = tomllib.loads((destination / "index.toml").read_text())
            self.assertEqual((index["backpacks"], index["universal_inventories"]), (2, 1))


if __name__ == "__main__":
    unittest.main()
