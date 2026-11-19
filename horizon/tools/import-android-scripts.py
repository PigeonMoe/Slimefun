#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""Convert the Paper Slimefun scripts directory to Horizon's shared Android library."""

import argparse
import json
import os
import tempfile
from pathlib import Path
from uuid import UUID

import yaml


TYPES = (
    "NONE", "MINER", "FARMER", "ADVANCED_FARMER",
    "WOODCUTTER", "FIGHTER", "FISHERMAN", "NON_FIGHTER",
)
MAX_FILE = 16 * 1024 * 1024


def text(value, maximum):
    if (not isinstance(value, str) or not value.strip() or len(value.encode()) > maximum
            or any(ord(ch) < 32 or 127 <= ord(ch) <= 159 for ch in value)):
        raise ValueError("Invalid script text.")
    return json.dumps(value, ensure_ascii=False)


def uuid(value):
    if not isinstance(value, str):
        raise ValueError("Invalid script user UUID.")
    return UUID(value).hex


def parse_script(path, android_type):
    if path.stat().st_size > 1024 * 1024:
        raise ValueError("Script file is too large.")
    row = yaml.safe_load(path.read_text(encoding="utf-8"))
    if not isinstance(row, dict):
        raise ValueError("Invalid script metadata.")
    # A small number of early Paper installs save incomplete placeholders.
    if "code" not in row or "author" not in row:
        return None
    author = uuid(row["author"])
    if row.get("android", android_type) != android_type:
        raise ValueError("The Android type differs from the script directory.")
    rating = row.get("rating", {})
    downloads = row.get("downloads", 0)
    if not isinstance(rating, dict) or type(downloads) is not int or not 0 <= downloads <= 0xFFFFFFFF:
        raise ValueError("Invalid script statistics.")
    result = [
        "",
        "[[scripts]]",
        f"name = {text(row['name'], 1024)}",
        f'author = "{author}"',
        f"author_name = {text(row.get('author_name') or author, 1024)}",
        f'category = "{android_type}"',
        f"code = {text(row['code'], 64 * 1024)}",
        f"downloads = {downloads}",
    ]
    for vote in ("positive", "negative"):
        players = rating.get(vote, [])
        if not isinstance(players, list):
            raise ValueError("Invalid script voter list.")
        values = sorted({uuid(player) for player in players})
        result.append(f"{vote} = {json.dumps(values)}")
    return "\n".join(result) + "\n"


def import_scripts(source, destination):
    if not source.is_dir():
        raise ValueError(f"No such Paper scripts directory: {source}")
    records = ["format = 1\n"]
    for android_type in TYPES:
        for path in sorted((source / android_type).glob("*.sfs")):
            if not path.is_file():
                continue
            try:
                script = parse_script(path, android_type)
            except (KeyError, OSError, ValueError, yaml.YAMLError) as error:
                raise ValueError(f"{path}: {error}") from error
            if script is not None:
                records.append(script)
    archive = "".join(records).encode()
    if len(archive) > MAX_FILE:
        raise ValueError("The Android script archive is too large.")
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=destination.parent) as temporary:
        temporary.write(archive)
        temporary.flush()
        os.link(temporary.name, destination)
    return len(records) - 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path, help="Paper plugins/Slimefun/scripts directory")
    parser.add_argument("destination", type=Path, help="Horizon android-scripts-import.toml file")
    args = parser.parse_args()
    try:
        scripts = import_scripts(args.source, args.destination)
    except (OSError, ValueError) as error:
        parser.exit(1, f"{error}\n")
    print(f"Archived {scripts} Android scripts.")


if __name__ == "__main__":
    main()
