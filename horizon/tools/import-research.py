#!/usr/bin/env python3
"""Prepare Slimefun research imports from Paper's legacy player YAML files."""

import argparse
from collections import defaultdict
import json
from pathlib import Path
from uuid import UUID

import yaml


def read_research_ids(path):
    catalog = json.loads(path.read_text(encoding="utf-8"))
    if catalog.get("format") != 1:
        raise ValueError("Expected a version 1 Paper catalog from runtime-smoke.")
    keys = defaultdict(set)
    for research in catalog["researches"]:
        identifier = research["id"]
        if identifier.startswith("slimefun:"):
            keys[research["legacy_id"]].add(identifier)
    return keys


def collect(players, keys):
    exports = {}
    unknown = set()
    for source in sorted(players.glob("*.yml")):
        try:
            player_id = UUID(source.stem).hex
        except ValueError:
            continue
        profile = yaml.safe_load(source.read_text(encoding="utf-8")) or {}
        if not isinstance(profile, dict):
            raise ValueError(f"{source.name} must contain a player profile mapping.")
        researches = profile.get("researches") or {}
        if not isinstance(researches, dict):
            raise ValueError(f"{source.name} must contain research IDs as a mapping.")

        unlocked = set()
        for legacy_id, stored in researches.items():
            # LegacyStorage checks key presence rather than the stored Boolean value.
            if stored is None:
                continue
            key = int(legacy_id)
            if key not in keys:
                unknown.add(key)
            unlocked.update(keys.get(key, ()))
        if not unlocked:
            continue
        if player_id in exports:
            raise ValueError(f"Duplicate source profiles for player {player_id}.")
        exports[player_id] = unlocked
    return exports, unknown


def write_imports(path, exports):
    path.mkdir(parents=True, exist_ok=True)
    for player_id, ids in exports.items():
        destination = path / f"{player_id}.txt"
        text = "\n".join(["slimefun-research-import-v1", player_id, *sorted(ids), ""])
        if destination.exists() and destination.read_text(encoding="utf-8") != text:
            raise ValueError(f"Existing import differs for {player_id}.")
    for player_id, ids in exports.items():
        destination = path / f"{player_id}.txt"
        if not destination.exists():
            with destination.open("x", encoding="utf-8") as file:
                file.write("\n".join(["slimefun-research-import-v1", player_id, *sorted(ids), ""]))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("paper_catalog", type=Path)
    parser.add_argument("paper_players", type=Path)
    parser.add_argument("horizon_import_dir", type=Path)
    args = parser.parse_args()
    if not args.paper_players.is_dir():
        parser.error("paper_players must point to the Slimefun legacy Players directory.")
    try:
        exports, unknown = collect(args.paper_players, read_research_ids(args.paper_catalog))
        if unknown:
            ids = ", ".join(map(str, sorted(unknown)))
            parser.error(f"The Paper catalog is missing legacy research IDs {ids}.")
        write_imports(args.horizon_import_dir, exports)
    except (OSError, ValueError, TypeError, KeyError, yaml.YAMLError) as error:
        parser.error(str(error))
    print(f"Prepared research imports for {len(exports)} players.")


if __name__ == "__main__":
    main()
