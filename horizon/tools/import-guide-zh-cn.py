#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""Regenerate the guide's packaged translation table from the Paper resources."""
import json
import re
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
LANGUAGE = ROOT / "src/main/resources/languages/zh-CN"
OUTPUT = ROOT / "horizon/src/guide/zh-cn.json"
ENGLISH_NAME = re.compile(r"\s*\([A-Za-z][A-Za-z0-9 /,'-]+\)$")


def read(name):
    return yaml.safe_load((LANGUAGE / f"{name}.yml").read_text(encoding="utf-8"))


def main():
    items = {
        key: {
            "name": ENGLISH_NAME.sub("", value["name"]).replace("TheBusyBiscuit", "原作者"),
            **({"lore": value["lore"]} if value.get("lore") else {}),
        }
        for key, value in read("items").items()
    }
    researches = {
        key: value.replace("Slimefun", "远古工艺")
        for key, value in read("researches")["slimefun"].items()
    }
    data = {"items": items, "researches": researches}
    OUTPUT.write_text(
        json.dumps(data, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()
