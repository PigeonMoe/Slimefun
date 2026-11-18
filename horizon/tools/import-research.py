#!/usr/bin/env python3
"""Prepare Slimefun research imports from Paper player profiles."""

import argparse
from collections import defaultdict
import json
import os
from pathlib import Path
import re
import tempfile
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
    namespaced = {key for aliases in keys.values() for key in aliases}
    seen = set()
    for source in sorted(players.glob("*.yml")):
        try:
            player_id = UUID(source.stem).hex
        except ValueError:
            continue
        if player_id in seen:
            raise ValueError(f"Duplicate source profiles for player {player_id}.")
        seen.add(player_id)
        profile = yaml.safe_load(source.read_text(encoding="utf-8")) or {}
        if not isinstance(profile, dict):
            raise ValueError(f"{source.name} must contain a player profile mapping.")
        researches = profile.get("researches") or {}
        if not isinstance(researches, dict):
            raise ValueError(f"{source.name} must contain research IDs as a mapping.")

        unlocked = set()
        if "pm-research-keys" in profile:
            revision = profile.get("pm-snapshot-revision")
            keys_in_snapshot = profile["pm-research-keys"]
            if (type(revision) is not int or revision < 1 or not isinstance(keys_in_snapshot, list)
                    or any(not isinstance(key, str) or re.fullmatch(r"[a-z0-9._-]+:[a-z0-9/._-]+", key) is None
                           for key in keys_in_snapshot)
                    or len(set(keys_in_snapshot)) != len(keys_in_snapshot)):
                raise ValueError(f"{source.name} contains invalid PM-Sync research metadata.")
            for identifier in keys_in_snapshot:
                if identifier in namespaced:
                    unlocked.add(identifier)
                elif identifier.startswith("slimefun:"):
                    raise ValueError(f"The Paper catalog is missing research {identifier} in {source.name}.")
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
        exports[player_id] = unlocked
    return exports, unknown


def write_imports(path, exports):
    path.mkdir(parents=True, exist_ok=True)
    rows = {
        f"{player_id}.txt": "\n".join(["slimefun-research-import-v1", player_id, *sorted(ids), ""])
        for player_id, ids in exports.items()
    }
    for filename, text in rows.items():
        destination = path / filename
        if destination.is_symlink() or (destination.exists() and destination.read_text(encoding="utf-8") != text):
            raise ValueError(f"Existing research import differs at {destination}.")
    with tempfile.TemporaryDirectory(prefix=".research-import-", dir=path) as temporary:
        for filename, text in rows.items():
            staged = Path(temporary) / filename
            staged.write_text(text, encoding="utf-8")
            destination = path / filename
            try:
                os.link(staged, destination)
            except FileExistsError:
                if destination.is_symlink() or destination.read_text(encoding="utf-8") != text:
                    raise ValueError(f"Existing research import differs at {destination}.") from None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("paper_catalog", type=Path)
    parser.add_argument("paper_players", type=Path)
    parser.add_argument("horizon_import_dir", type=Path)
    args = parser.parse_args()
    source = args.paper_players.resolve()
    destination = args.horizon_import_dir.resolve()
    if not source.is_dir():
        parser.error("paper_players must point to the Slimefun Players directory.")
    if destination.is_relative_to(source) or source.is_relative_to(destination):
        parser.error("The research import directory and Paper data must be separate.")
    try:
        exports, unknown = collect(source, read_research_ids(args.paper_catalog))
        if unknown:
            ids = ", ".join(map(str, sorted(unknown)))
            parser.error(f"The Paper catalog is missing legacy research IDs {ids}.")
        write_imports(destination, exports)
    except (OSError, ValueError, TypeError, KeyError, yaml.YAMLError) as error:
        parser.error(str(error))
    print(f"Prepared research imports for {len(exports)} players.")


if __name__ == "__main__":
    main()
