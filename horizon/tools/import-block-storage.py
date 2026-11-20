#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""Prepare a Horizon import from Paper Slimefun blocks, inventories, and GEO reserves."""

import argparse
from collections import defaultdict
from contextlib import contextmanager
import json
from pathlib import Path
import re
import shutil
import stat
import tempfile
from uuid import UUID
from zipfile import BadZipFile, ZipFile

import yaml


WORLDS = {
    "world": "minecraft:overworld",
    "world_nether": "minecraft:the_nether",
    "world_the_end": "minecraft:the_end",
}
DIMENSION = re.compile(r"[a-z0-9_.-]+:[a-z0-9_./-]+\Z")
COMPONENT = re.compile(r"!?[a-z0-9_.-]+:[a-z0-9_./-]+\Z")
PAPER_ITEM_CLASS = "org.bukkit.inventory.ItemStack"
DATA_FILE_MAX = 64 << 20  # Horizon DATA_FILE_MAX
STORAGE_DIRECTORIES = (
    "stored-blocks", "stored-inventories", "stored-chunks", "universal-inventories", "Players",
)


class UniqueYamlKeys(yaml.SafeLoader):
    def construct_mapping(self, node, deep=False):
        for key, _ in node.value:
            name = self.construct_object(key, deep=deep)
            if not isinstance(name, (str, int)):
                raise ValueError("A storage key must be a string or a slot number.")
        result = super().construct_mapping(node, deep=deep)
        if len(result) != len(node.value):
            raise ValueError("Duplicate YAML storage key.")
        return result


def unique_json_keys(entries):
    result = dict(entries)
    if len(result) != len(entries):
        raise ValueError("Duplicate JSON block metadata key.")
    return result


def location(key, mappings, expected_world=None):
    if not isinstance(key, str):
        raise ValueError(f"Invalid Paper location {key!r}.")
    parts = key.split(";")
    if len(parts) != 4:
        raise ValueError(f"Invalid Paper location {key!r}.")
    world, *coordinates = parts
    if expected_world is not None and world != expected_world:
        raise ValueError(f"Location {key!r} belongs to a different Paper world.")
    if world not in mappings:
        raise ValueError(f"Add --map-world {world}=NAMESPACE:DIMENSION for {key!r}.")
    if any(re.fullmatch(r"[+-]?[0-9]+", value) is None for value in coordinates):
        raise ValueError(f"Invalid coordinates in {key!r}.")
    position = tuple(map(int, coordinates))
    if not all(-(1 << 31) <= value < (1 << 31) for value in position):
        raise ValueError(f"Coordinates are outside the signed block range in {key!r}.")
    return mappings[world], position


def mapping(text, source):
    value = yaml.load(text, Loader=UniqueYamlKeys)
    if value is None:
        return {}
    if not isinstance(value, dict):
        raise ValueError(f"{source} must contain a YAML mapping.")
    return value


def item(slot, value):
    if not isinstance(value, dict) or value.get("==") != PAPER_ITEM_CLASS:
        raise ValueError("expected a Bukkit ItemStack")
    if type(value.get("schema_version")) is not int or value["schema_version"] != 1:
        raise ValueError("resave this legacy item with Paper's component-based item format")
    if value.keys() - {"==", "schema_version", "id", "DataVersion", "count", "components"}:
        raise ValueError("unknown Paper item fields")
    item_id = value.get("id")
    version = value.get("DataVersion")
    count = value.get("count", 1)
    components = value.get("components", {})
    if not isinstance(item_id, str) or DIMENSION.fullmatch(item_id) is None:
        raise ValueError("invalid item id")
    if type(version) is not int or not 0 < version < (1 << 31):
        raise ValueError("invalid item DataVersion")
    if type(count) is not int or not 1 <= count <= 99:
        raise ValueError("invalid item count")
    if not isinstance(components, dict) or any(
        not isinstance(key, str) or COMPONENT.fullmatch(key) is None
        or not isinstance(tag, str) or not tag.strip() for key, tag in components.items()
    ):
        raise ValueError("components must be named SNBT values")
    if item_id == "minecraft:air":
        if components:
            raise ValueError("an empty slot cannot have item components")
        return None
    return {"slot": slot, "id": item_id, "data_version": version,
            "count": count, "components": components}


def inventory_snapshot(data, size=54):
    items, issues, seen = [], [], set()
    for key, value in data.items():
        number = str(key)
        if re.fullmatch(r"[0-9]+", number) is None:
            continue
        slot = int(number)
        if not 0 <= slot < size or str(slot) != number or slot in seen:
            issues.append(f"Slot {number}: invalid or repeated Paper slot number")
            continue
        seen.add(slot)
        if value is None:
            continue
        try:
            stack = item(slot, value)
        except ValueError as error:
            issues.append(f"Slot {slot}: {error}")
        else:
            if stack is not None:
                items.append(stack)
    preset = data.get("preset")
    if preset is not None and (not isinstance(preset, str) or not preset):
        issues.append("Invalid Paper inventory preset")
        preset = None
    return preset, sorted(items, key=lambda stack: stack["slot"]), issues


def inventory_record(data, size=54):
    preset, items, issues = inventory_snapshot(data, size)
    return {
        "inventory_preset": preset,
        "paper_items": items,
        "paper_item_issues": issues,
        "inventory_slots": sum(
            bool(value) and re.fullmatch(r"[0-9]+", str(key)) is not None
            and 0 <= int(key) < size
            and (not isinstance(value, dict) or value.get("id") != "minecraft:air")
            for key, value in data.items()
        ),
    }


def collect(source, mappings):
    records = {}
    for file in sorted((source / "stored-blocks").glob("*/*.sfb")):
        if file.name == "null.sfb":
            print(f"Skipped Paper's corrupt-block quarantine file {file.relative_to(source)}.")
            continue
        try:
            for key, raw in mapping(file.read_text(encoding="utf-8"), file).items():
                address = location(key, mappings, file.parent.name)
                if address in records:
                    raise ValueError(f"More than one block record resolves to {address}.")
                if not isinstance(raw, str):
                    raise ValueError(f"Block metadata for {key!r} must be a JSON string.")
                data = json.loads(raw, object_pairs_hook=unique_json_keys)
                if not isinstance(data, dict) or not isinstance(data.get("id"), str) or not data["id"]:
                    raise ValueError(f"Block metadata for {key!r} has no Slimefun ID.")
                if not all(value is None or isinstance(value, str) for value in data.values()):
                    raise ValueError(f"Block metadata for {key!r} contains a non-string value.")
                records[address] = {"metadata": data}
        except (ValueError, TypeError, yaml.YAMLError) as error:
            raise ValueError(f"{file.relative_to(source)}: {error}") from error
    for file in sorted((source / "stored-inventories").glob("*.sfi")):
        try:
            address = location(file.stem, mappings)
            record = records.setdefault(address, {})
            if "inventory" in record:
                raise ValueError(f"More than one inventory resolves to {address}.")
            raw = file.read_bytes().decode("utf-8")
            data = mapping(raw, file)
            record["inventory"] = raw
            record.update(inventory_record(data))
        except (ValueError, TypeError, yaml.YAMLError) as error:
            raise ValueError(f"{file.relative_to(source)}: {error}") from error
    return records


def collect_universal(source):
    records = {}
    for file in sorted((source / "universal-inventories").glob("*.sfi")):
        try:
            raw = file.read_bytes().decode("utf-8")
            data = mapping(raw, file)
            row = inventory_record(data)
            paper_slots = set()
            for key in data:
                if key == "preset":
                    continue
                number = str(key)
                if re.fullmatch(r"0|[1-9]|[1-4][0-9]|5[0-3]", number) is None:
                    row["paper_item_issues"].append(f"Unknown universal inventory slot {key!r}")
                else:
                    paper_slots.add(int(number))
            if row["inventory_preset"] != file.stem:
                row["paper_item_issues"].append("The menu preset differs from its file name")
            records[file.stem] = {"inventory": raw, "paper_slots": sorted(paper_slots), **row}
        except (ValueError, TypeError, yaml.YAMLError) as error:
            raise ValueError(f"{file.relative_to(source)}: {error}") from error
    return records


GEO_KEYS = ("slimefun-oil", "slimefun-nether_ice", "slimefun-uranium", "slimefun-salt")
PAPER_ITEM_ID = re.compile(r"[A-Z][A-Z0-9_]*\Z")


def collect_geo_settings(config):
    values = mapping(config.read_text(encoding="utf8"), config).get("slimefun", {})
    if not isinstance(values, dict):
        raise ValueError(f"{config}: expected a mapping for the slimefun namespace.")
    settings = {}
    for legacy in GEO_KEYS:
        key = legacy.removeprefix("slimefun-")
        resource = values.get(key, {})
        if not isinstance(resource, dict) or type(resource.get("enabled", True)) is not bool:
            raise ValueError(f"{config}: slimefun.{key}.enabled must be a boolean.")
        if "enabled" in resource:
            settings[key] = resource["enabled"]
    return settings


def boolean_setting(value, path):
    if type(value) is not bool:
        raise ValueError(f"{path} must be a boolean.")
    return str(value).lower()


def availability_fragment(item_config, world_settings, mappings):
    sections = []
    if item_config is not None:
        items = mapping(item_config.read_text(encoding="utf-8"), item_config)
        for item, data in sorted(items.items(), key=lambda row: str(row[0])):
            if (isinstance(item, str) and PAPER_ITEM_ID.fullmatch(item)
                    and isinstance(data, dict) and "enabled" in data):
                enabled = boolean_setting(data["enabled"], f"{item_config}: {item}.enabled")
                sections.append(f"[item-settings.{item}]\nenabled = {enabled}")
    if world_settings is not None:
        if not world_settings.is_dir():
            raise ValueError(f"{world_settings} must be the Paper world-settings directory.")
        dimensions = set()
        for file in sorted(world_settings.glob("*.yml")):
            dimension = mappings.get(file.stem)
            if dimension is None:
                raise ValueError(f"Add --map-world {file.stem}=NAMESPACE:DIMENSION for {file}.")
            if dimension in dimensions:
                raise ValueError(f"Multiple Paper world settings map to {dimension}.")
            dimensions.add(dimension)
            world = mapping(file.read_text(encoding="utf-8"), file)
            stock = world.get("slimefun", {})
            if not isinstance(stock, dict):
                raise ValueError(f"{file}: expected a mapping for the slimefun namespace.")
            header = f"worlds.{json.dumps(dimension)}"
            flags = []
            if "enabled" in world:
                enabled = boolean_setting(world["enabled"], f"{file}: enabled")
                flags.append(f"enabled = {enabled}")
            if "enabled" in stock:
                enabled = boolean_setting(stock["enabled"], f"{file}: slimefun.enabled")
                flags.append(f"slimefun-enabled = {enabled}")
            if flags:
                sections.append("\n".join((f"[{header}]", *flags)))
            overrides = []
            for item, enabled in sorted(stock.items(), key=lambda row: str(row[0])):
                if isinstance(item, str) and PAPER_ITEM_ID.fullmatch(item):
                    value = boolean_setting(enabled, f"{file}: slimefun.{item}")
                    overrides.append(f"{item} = {value}")
            if overrides:
                sections.append("\n".join((f"[{header}.items]", *overrides)))
    return "\n\n".join(sections) + "\n" if sections else ""


def collect_geo(source, mappings):
    file = source / "stored-chunks" / "chunks.sfc"
    records = {}
    if not file.is_file():
        return records
    try:
        for key, raw in mapping(file.read_text(encoding="utf-8"), file).items():
            if not isinstance(raw, str):
                raise ValueError(f"Chunk metadata for {key!r} must be a JSON string.")
            data = json.loads(raw, object_pairs_hook=unique_json_keys)
            if not isinstance(data, dict):
                raise ValueError(f"Invalid Paper chunk metadata for {key!r}.")
            supplies = {}
            for resource in GEO_KEYS:
                if resource not in data:
                    continue
                amount = data[resource]
                if (not isinstance(amount, str)
                        or re.fullmatch(r"0|[1-9][0-9]{0,9}", amount) is None
                        or int(amount) >= 1 << 31):
                    raise ValueError(f"Invalid Paper GEO supply for {resource} in chunk {key!r}.")
                supplies[resource] = int(amount)
            if not supplies:
                continue
            if not isinstance(key, str):
                raise ValueError(f"Invalid Paper chunk {key!r}.")
            coordinates = key.split(";")
            if len(coordinates) != 4 or coordinates[1] != "Chunk":
                raise ValueError(f"Invalid Paper chunk {key!r}.")
            world, _, x, z = coordinates
            dimension, (x, _, z) = location(f"{world};{x};0;{z}", mappings)
            address = (dimension, x, z)
            if address in records:
                raise ValueError(f"Multiple Paper GEO records resolve to {address}.")
            records[address] = supplies
    except (ValueError, TypeError, yaml.YAMLError) as error:
        raise ValueError(f"{file.relative_to(source)}: {error}") from error
    return records


def collect_backpacks(source, profiles=None):
    records = {}
    directory = profiles if profiles is not None else source / "Players"
    for file in sorted(directory.glob("*.yml")):
        try:
            owner = UUID(file.stem).hex
        except ValueError:
            continue
        raw = file.read_bytes().decode("utf-8")
        bags, issues, seen = {}, [], set()
        try:
            data = mapping(raw, file).get("backpacks")
            if data is None or data == {}:
                continue
            if not isinstance(data, dict):
                raise ValueError("backpacks must be a mapping.")
            for key, bag in data.items():
                try:
                    bag_id = backpack_number(key)
                    if bag_id in seen:
                        bags.pop(bag_id, None)
                        raise ValueError(f"Repeated backpack ID {key!r}.")
                    seen.add(bag_id)
                    bags[bag_id] = backpack_record(bag_id, bag)
                except ValueError as error:
                    issues.append(str(error))
        except (ValueError, TypeError, yaml.YAMLError) as error:
            issues.append(str(error))
        if owner in records:
            raise ValueError(f"{file}: More than one profile has this UUID.")
        records[owner] = {"paper_profile": raw, "backpacks": bags, "issues": issues}
    return records


def backpack_number(key):
    number = str(key)
    if re.fullmatch(r"0|[1-9][0-9]{0,9}", number) is None or int(number) >= 1 << 31:
        raise ValueError(f"Invalid backpack ID {key!r}.")
    return int(number)


def backpack_record(key, bag):
    if not isinstance(bag, dict):
        raise ValueError(f"Backpack {key} must contain its size and contents.")
    size = bag.get("size")
    if type(size) is not int or not 9 <= size <= 54 or size % 9:
        raise ValueError(f"Invalid size for backpack {key}.")
    slots = bag.get("contents")
    if slots is not None and not isinstance(slots, dict):
        raise ValueError(f"Backpack {key} contents must be a slot mapping.")
    slots = slots or {}
    converted = inventory_record(slots, size)
    for slot in slots:
        if re.fullmatch(r"[0-9]+", str(slot)) is None:
            converted["paper_item_issues"].append(f"Invalid backpack inventory slot {slot!r}.")
    return {"size": size, **converted}


def render(records, universal=None, players=None, geo=None, geo_settings=None, availability=None):
    chunks = defaultdict(list)
    for (dimension, position), record in sorted(records.items()):
        x, _, z = position
        chunks[dimension, x // 16, z // 16].append((position, record))
    def string(value):
        return json.dumps(value, ensure_ascii=False).replace("\x7f", "\\u007f")
    def stack_table(stack):
        components = ", ".join(
            f"{string(key)} = {string(tag)}" for key, tag in sorted(stack["components"].items())
        )
        return (
            f'{{slot = {stack["slot"]}, id = {string(stack["id"])}, '
            f'data_version = {stack["data_version"]}, count = {stack["count"]}, '
            f'components = {{{components}}}}}'
        )
    def item_fields(record):
        yield f"inventory_slots = {record['inventory_slots']}"
        if "inventory" in record:
            yield f"paper_inventory = {string(record['inventory'])}"
        if record["inventory_preset"] is not None:
            yield f"inventory_preset = {string(record['inventory_preset'])}"
        yield "paper_items = ["
        yield from (f"  {stack_table(stack)}," for stack in record["paper_items"])
        yield "]"
        if record["paper_item_issues"]:
            yield f"paper_item_issues = {string(record['paper_item_issues'])}"
    summary = {
        "blocks": sum("metadata" in record for record in records.values()),
        "inventories": sum("inventory" in record for record in records.values()),
        "chunks": len(chunks),
    }
    if universal:
        summary["universal_inventories"] = len(universal)
    if players:
        summary["backpacks"] = sum(len(profile["backpacks"]) for profile in players.values())
    if geo:
        summary["geo_chunks"] = len(geo)
        summary["oil_reserves"] = sum("slimefun-oil" in supplies for supplies in geo.values())
    yield "index.toml", "\n".join(
        ["format = 1", *(f"{key} = {value}" for key, value in summary.items()), ""]
    )
    for (dimension, chunk_x, chunk_z), entries in sorted(chunks.items()):
        lines = ["format = 1", f"dimension = {string(dimension)}", ""]
        for position, record in entries:
            lines.extend(("[[records]]", f"position = {list(position)}"))
            if "inventory" in record:
                lines.extend(item_fields(record))
            if "metadata" in record:
                null_keys = sorted(key for key, value in record["metadata"].items() if value is None)
                if null_keys:
                    lines.append(f"null_metadata_keys = {string(null_keys)}")
                lines.append("[records.metadata]")
                for key, value in sorted(record["metadata"].items()):
                    if value is not None:
                        lines.append(f"{string(key)} = {string(value)}")
            lines.append("")
        # Hex makes custom dimension paths predictable even when their IDs contain slashes.
        bucket = f"{dimension.encode().hex()}/{chunk_x}_{chunk_z}.toml"
        yield bucket, "\n".join(lines)
    for (dimension, x, z), supplies in sorted((geo or {}).items()):
        yield f"geo/{dimension.encode().hex()}/{x}_{z}.toml", "\n".join((
            "format = 1", f"dimension = {string(dimension)}", f"chunk = [{x}, {z}]",
            *(f"{key} = {amount}" for key, amount in supplies.items()), "",
        ))
    for preset, row in sorted((universal or {}).items()):
        yield f"universal/{preset.encode().hex()}.toml", "\n".join((
            "format = 1", f"preset = {string(preset)}", f"paper_slots = {row['paper_slots']}",
            *item_fields(row), "",
        ))
    for uuid, profile in sorted((players or {}).items()):
        lines = ["format = 1", f"player = {string(uuid)}",
                 f"paper_profile = {string(profile['paper_profile'])}", ""]
        if profile["issues"]:
            lines.append(f"paper_profile_issues = {string(profile['issues'])}")
        for bag_id, bag in sorted(profile["backpacks"].items()):
            lines.extend(("[[backpacks]]", f"id = {bag_id}", f"size = {bag['size']}",
                          *item_fields(bag), ""))
        yield f"backpacks/{uuid}.toml", "\n".join(lines)
    if geo_settings is not None:
        yield "settings-geo.toml", "\n".join((
            "[geo-resources]",
            *(f"{key} = {str(enabled).lower()}" for key, enabled in geo_settings.items()),
            "",
        ))
    if availability is not None:
        yield "settings-item-availability.toml", availability


def publish(output, records, universal=None, players=None, geo=None, geo_settings=None,
            availability=None):
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".storage-import-", dir=output.parent) as temp:
        stage = Path(temp) / "snapshot"
        stage.mkdir()
        for name, text in render(records, universal, players, geo, geo_settings, availability):
            data = text.encode("utf-8")
            if len(data) > DATA_FILE_MAX:
                raise ValueError(f"Import chunk {name} exceeds Horizon's 64 MiB data file limit.")
            file = stage / name
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_bytes(data)
        if output.exists():
            files = sorted(file.relative_to(stage) for file in stage.rglob("*") if file.is_file())
            existing = sorted(file.relative_to(output) for file in output.rglob("*") if file.is_file())
            if files == existing and all(
                (stage / file).read_bytes() == (output / file).read_bytes() for file in files
            ):
                return
            raise ValueError(f"Existing import differs at {output}. Choose a new directory.")
        stage.rename(output)


def paper_backup_file(parts):
    if len(parts) == 3 and parts[0] == "stored-blocks":
        return parts[2].endswith(".sfb")
    if len(parts) == 2 and parts[0] in {"stored-inventories", "universal-inventories"}:
        return parts[1].endswith(".sfi")
    return parts == ["stored-chunks", "chunks.sfc"]


@contextmanager
def paper_snapshot(source):
    if source.is_dir():
        if not any((source / name).is_dir() for name in STORAGE_DIRECTORIES):
            raise ValueError("The directory does not contain a Paper Slimefun storage directory.")
        yield source
        return
    if not source.is_file() or source.suffix.lower() != ".zip":
        raise ValueError("Use a Paper Slimefun storage directory or a block-backups ZIP.")
    with ZipFile(source) as archive, tempfile.TemporaryDirectory(prefix="slimefun-paper-backup-") as temp:
        root = Path(temp)
        paths = set()
        for entry in archive.infolist():
            parts = entry.filename.removesuffix("/").split("/")
            if any(part in ("", ".", "..") or "\\" in part for part in parts):
                raise ValueError(f"Invalid path in Paper backup: {entry.filename!r}.")
            if entry.is_dir() or not paper_backup_file(parts):
                continue
            if stat.S_IFMT(entry.external_attr >> 16) not in (0, stat.S_IFREG):
                raise ValueError(f"Nonregular file in Paper backup: {entry.filename!r}.")
            if entry.flag_bits & 1:
                raise ValueError(f"Encrypted file in Paper backup: {entry.filename!r}.")
            path = root.joinpath(*parts)
            if path in paths:
                raise ValueError(f"Repeated file in Paper backup: {entry.filename!r}.")
            paths.add(path)
            path.parent.mkdir(parents=True, exist_ok=True)
            with archive.open(entry) as src, path.open("xb") as dst:
                shutil.copyfileobj(src, dst)
        if not paths:
            raise ValueError("The ZIP does not contain any Slimefun block backup records.")
        yield root


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("paper_storage", type=Path)
    parser.add_argument("horizon_import_dir", type=Path)
    parser.add_argument("--map-world", action="append", default=[], metavar="PAPER=DIMENSION")
    parser.add_argument("--player-profiles", type=Path, metavar="PLAYERS_DIRECTORY",
                        help="explicitly replace the source Players directory with an exported snapshot")
    parser.add_argument("--resources-config", type=Path, metavar="RESOURCES_YML",
                        help="export the stock Paper GEO resource toggles as a TOML section")
    parser.add_argument("--item-config", type=Path, metavar="ITEMS_YML",
                        help="export the Paper Items.yml global availability flags")
    parser.add_argument("--world-settings", type=Path, metavar="WORLD_SETTINGS_DIRECTORY",
                        help="export the Paper stock Slimefun world and item availability flags")
    args = parser.parse_args()
    source = args.paper_storage.resolve()
    output = args.horizon_import_dir.resolve()
    profiles = args.player_profiles.resolve() if args.player_profiles is not None else None
    worlds = dict(WORLDS)
    for pair in args.map_world:
        paper, separator, dimension = pair.partition("=")
        if (not separator or not paper or ";" in paper or DIMENSION.fullmatch(dimension) is None
                or any(part in ("", ".", "..") for part in dimension.partition(":")[2].split("/"))):
            parser.error(f"Invalid world mapping {pair!r}.")
        worlds[paper] = dimension
    if profiles is not None and not profiles.is_dir():
        parser.error("--player-profiles must point to an exported Players directory.")
    world_settings = args.world_settings.resolve() if args.world_settings else None
    for original in (source, profiles, world_settings):
        if original is not None and (output.is_relative_to(original) or original.is_relative_to(output)):
            parser.error("The import directory and Paper data must be separate.")
    try:
        geo_settings = collect_geo_settings(args.resources_config) if args.resources_config else None
        availability = (
            availability_fragment(args.item_config, world_settings, worlds)
            if args.item_config is not None or world_settings is not None else None
        )
        with paper_snapshot(source) as root:
            records = collect(root, worlds)
            universal = collect_universal(root)
            players = collect_backpacks(root, profiles)
            geo = collect_geo(root, worlds)
        publish(output, records, universal, players, geo, geo_settings, availability)
    except (OSError, ValueError, BadZipFile, yaml.YAMLError) as error:
        parser.error(str(error))
    print(f"Prepared an import with {len(records)} block or inventory locations, "
          f"{len(universal)} universal inventories, {len(players)} backpack profiles, "
          f"and {len(geo)} GEO chunks.")
    if geo_settings is not None:
        print(f"Merge the stock GEO resource settings in {output / 'settings-geo.toml'} "
              "into slimefun-server.toml.")
    if availability is not None:
        print(f"Merge the stock availability settings in {output / 'settings-item-availability.toml'} "
              "into slimefun-server.toml.")


if __name__ == "__main__":
    main()
