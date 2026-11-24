#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""Import the Paper tag list displayed when a Debug Fish inspects a vanilla block."""
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "src/main/java/io/github/thebusybiscuit/slimefun4/utils/tags/SlimefunTag.java"
RESOURCES = ROOT / "src/main/resources/tags"
OUTPUT = ROOT / "horizon/src/debug-fish-tags.json"


def main():
    declaration = SOURCE.read_text(encoding="utf-8").split("public enum SlimefunTag", 1)[1]
    names = [name.lower() for name in re.findall(r"^    ([A-Z][A-Z_]+)[,;]$", declaration, re.M)]
    assert set(names) == {path.stem for path in RESOURCES.glob("*.json")}
    tags = {}
    for name in names:
        values = json.loads((RESOURCES / f"{name}.json").read_text(encoding="utf-8"))["values"]
        tags[name] = [value if isinstance(value, str) else value["id"] for value in values]

    def validate(name, ancestors):
        if name in ancestors:
            raise ValueError(f"recursive Slimefun tag: {name}")
        for value in tags[name]:
            if value.startswith("#slimefun:"):
                validate(value.removeprefix("#slimefun:"), (*ancestors, name))

    for name in names:
        validate(name, ())
    rows = [json.dumps([name, tags[name]], separators=(",", ":")) for name in names]
    OUTPUT.write_text("[\n  " + ",\n  ".join(rows) + "\n]\n", encoding="utf-8")


if __name__ == "__main__":
    main()
