#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""Regenerate the guide's packaged translation table from the Paper resources."""
import json
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
LANGUAGE = ROOT / "src/main/resources/languages/zh-CN"
OUTPUT = ROOT / "horizon/src/guide/zh-cn.json"


def read(name):
    return yaml.safe_load((LANGUAGE / f"{name}.yml").read_text(encoding="utf-8"))


def main():
    items = {
        key: {
            "name": value["name"],
            **({"lore": value["lore"]} if value.get("lore") else {}),
        }
        for key, value in read("items").items()
    }
    researches = {
        key: value.replace("Slimefun", "远古工艺")
        for key, value in read("researches")["slimefun"].items()
    }
    barter = read("recipes")["slimefun"]["barter_drop"]
    recipes = {
        "barter_drop": {
            "name": barter["name"],
            "lore": [f"&7{line}" for line in barter["lore"]],
        },
    }
    data = {"items": items, "recipes": recipes, "researches": researches}
    OUTPUT.write_text(
        json.dumps(data, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()
