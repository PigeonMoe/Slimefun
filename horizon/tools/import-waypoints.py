#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""Convert Paper waypoint files to Horizon's UUID-scoped GPS import format."""

import argparse
import json
import math
import os
import re
import tempfile
from pathlib import Path
from uuid import UUID

import yaml


DIMENSION = re.compile(r"[a-z0-9_.-]+:[a-z0-9_./-]+\Z")
MAX_FILE = 512 * 1024
MAX_RECORDS = 1024


class UniqueLoader(yaml.SafeLoader):
    """Reject duplicate keys instead of replacing an earlier GPS record."""

    def construct_mapping(self, node, deep=False):
        self.flatten_mapping(node)
        result = {}
        for key_node, value_node in node.value:
            key = self.construct_object(key_node, deep=deep)
            if not isinstance(key, (str, int)) or key in result:
                raise ValueError(f"Invalid or repeated waypoint key {key!r}.")
            result[key] = self.construct_object(value_node, deep=deep)
        return result


def world_map(arguments):
    worlds = {}
    for entry in arguments:
        key, separator, dimension = entry.partition("=")
        if not separator or not key or not DIMENSION.fullmatch(dimension) or key in worlds:
            raise ValueError(f"Invalid or repeated world mapping {entry!r}.")
        worlds[key] = dimension
    return worlds


def uuid_paths(folder):
    result = {}
    for path in sorted(folder.glob("*.yml")):
        try:
            owner = UUID(path.stem).hex
        except ValueError:
            continue
        if owner in result:
            raise ValueError(f"More than one waypoint file for {owner!r} in {folder}.")
        result[owner] = path
    return result


def valid_text(value):
    return (isinstance(value, str) and 0 < len(value.encode()) <= 1024
            and all(not (ord(char) < 32 or 127 <= ord(char) <= 159) for char in value))


def point_record(point_id, row, mappings):
    if not valid_text(point_id) or not isinstance(row, dict) or not valid_text(row.get("name")):
        raise ValueError(f"Invalid waypoint {point_id!r}.")
    world_uuid = row.get("world_uuid")
    key = str(UUID(world_uuid)) if isinstance(world_uuid, str) else row.get("world")
    if not isinstance(key, str) or key not in mappings:
        raise ValueError(f"Waypoint {point_id!r} needs an explicit mapping for world {key!r}.")

    angles = [row.get(axis, 0) for axis in ("yaw", "pitch")]
    position = [row.get(axis) for axis in ("x", "y", "z")]
    if not all(type(value) in (int, float) and math.isfinite(value)
               for value in [*position, *angles]):
        raise ValueError(f"Waypoint {point_id!r} has a non-finite coordinate or angle.")
    if (not all(-limit <= value < limit for value, limit in
                zip(position, (30_000_000, 20_000_000, 30_000_000)))
            or any(abs(angle) > 3.4028234663852886e38 for angle in angles)):
        raise ValueError(f"Waypoint {point_id!r} is outside the supported range.")
    return (point_id, row["name"], mappings[key], position, angles)


def read_points(path, mappings):
    raw = path.read_bytes()
    if len(raw) > MAX_FILE:
        raise ValueError("Waypoint file is too large.")
    values = yaml.load(raw, Loader=UniqueLoader)
    if values is None:
        return []
    if not isinstance(values, dict) or len(values) > MAX_RECORDS:
        raise ValueError("Invalid or oversized waypoint mapping.")
    return [point_record(point_id, row, mappings) for point_id, row in values.items()]


def render(owner, points):
    lines = ["format = 1", f"owner = \"{owner}\""]
    for point_id, name, dimension, position, angles in points:
        lines.extend((
            "", "[[waypoints]]",
            f"id = {json.dumps(point_id, ensure_ascii=False)}",
            f"name = {json.dumps(name, ensure_ascii=False)}",
            f"dimension = {json.dumps(dimension)}",
            f"position = {json.dumps(position, allow_nan=False)}",
            f"rotation = {json.dumps(angles, allow_nan=False)}",
        ))
    return ("\n".join(lines) + "\n").encode()


def convert(source, output, mappings, server_id=None):
    sources = uuid_paths(source / "waypoints")
    if server_id is not None:
        if not re.fullmatch(r"[a-zA-Z0-9_.-]{1,128}", server_id) or server_id in (".", ".."):
            raise ValueError("Invalid Paper sync server ID.")
        sources.update(uuid_paths(source / "player-sync" / server_id / "waypoints"))
    files = {}
    for owner, original in sources.items():
        try:
            content = render(owner, read_points(original, mappings))
            if len(content) > MAX_FILE:
                raise ValueError("The converted waypoint file is too large.")
        except (ValueError, OverflowError, TypeError, yaml.YAMLError) as error:
            raise ValueError(f"{original}: {error}") from error
        destination = output / f"{owner}.toml"
        if destination.exists() and destination.read_bytes() != content:
            raise ValueError(f"Different waypoint data already exists at {destination}.")
        files[destination] = content
    output.mkdir(parents=True, exist_ok=True)
    for destination, content in files.items():
        if not destination.exists():
            with tempfile.NamedTemporaryFile(dir=output) as writer:
                writer.write(content)
                writer.flush()
                os.fsync(writer.fileno())
                try:
                    os.link(writer.name, destination)
                except FileExistsError:
                    if destination.read_bytes() != content:
                        raise ValueError(f"Different waypoint data now exists at {destination}.")
    return len(files)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path, help="Paper's data-storage/Slimefun directory")
    parser.add_argument("output", type=Path, help="Horizon Slimefun world-data waypoints-import directory")
    parser.add_argument("--world", action="append", default=[], help="Paper world name or UUID=Horizon dimension")
    parser.add_argument("--server-id", help="Prefer this PM-Sync server's local waypoint files when present")
    args = parser.parse_args()
    if not args.source.is_dir():
        parser.error("The Paper directory does not exist.")
    try:
        count = convert(args.source, args.output, world_map(args.world), args.server_id)
    except (OSError, ValueError) as error:
        parser.error(str(error))
    print(f"Prepared GPS waypoints for {count} UUIDs.")


if __name__ == "__main__":
    main()
