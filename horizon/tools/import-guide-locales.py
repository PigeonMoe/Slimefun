#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""Regenerate Horizon's guide language picker and message tables from Paper."""
import base64
import json
import re
import struct
import uuid
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
LANGUAGES = ROOT / "src/main/resources/languages"
PRESETS = ROOT / (
    "src/main/java/io/github/thebusybiscuit/slimefun4/core/services/localization/LanguagePreset.java"
)
OUTPUT = ROOT / "horizon/src/guide/locales.json"
COMMAND_OUTPUT = OUTPUT.with_name("command-locales.json")
PRESET = re.compile(r'^    [A-Z_]+\("([^"]+)", (?:[^"]+)?"([0-9a-f]+)"\)[,;]$', re.M)


def read(path):
    # Bukkit recognizes the language code "no" as a mapping key (and strings
    # such as "On" as translated labels), never as a YAML 1.1 boolean.
    return yaml.load(path.read_text(encoding="utf-8"), Loader=yaml.BaseLoader) or {}


def keys(data, prefix=""):
    for key, value in data.items():
        path = f"{prefix}{key}"
        yield path
        if isinstance(value, dict):
            yield from keys(value, f"{path}.")


def flag(texture):
    url = f"http://textures.minecraft.net/texture/{texture}"
    value = json.dumps({"textures": {"SKIN": {"url": url}}}, separators=(",", ":"))
    profile = uuid.uuid5(uuid.NAMESPACE_URL, url)
    return [list(struct.unpack(">4i", profile.bytes)), base64.b64encode(value.encode()).decode()]


def main():
    presets = dict(PRESET.findall(PRESETS.read_text(encoding="utf-8")))
    sources = {}
    selectable = []
    # Locale folders outside LanguagePreset can be configured as the server's
    # fallback, but only presets with resources belong in the player picker.
    for code in [*presets, "hi", "si"]:
        language = LANGUAGES / code
        if (language / "messages.yml").is_file():
            sources[code] = {path.stem: read(path) for path in language.glob("*.yml")}
            if code in presets:
                selectable.append(code)

    english = set(key for content in sources["en"].values() for key in keys(content))
    packs = {}
    command_packs = {}
    for code, files in sources.items():
        messages = files["messages"]
        commands = messages.get("commands", {})
        command_pack = {name: messages.get("messages", {}).get(name)
                        for name in ("no-permission", "not-researched", "only-players")}
        for name, command in commands.items():
            if isinstance(command, str) and name in sources["en"]["messages"]["commands"]:
                command_pack[name] = command
            elif isinstance(command, dict):
                if isinstance(command.get("description"), str):
                    command_pack[name] = command["description"]
                if name == "debug":
                    command_pack.update({f"debug.{key}": value for key, value in command.items()
                                         if key != "description"})
        command_packs[code] = command_pack
        available = set(key for content in files.values() for key in keys(content))
        packs[code] = {
            **({"flag": flag(presets[code])} if code in presets else {}),
            "progress": round(100 * len(available & english) / len(english), 2),
            "guide": messages.get("guide", {}),
            "languages": messages.get("languages", {}),
            "research-messages": {
                "unlocked": messages.get("messages", {}).get("unlocked"),
                **messages.get("messages", {}).get("research", {}),
            },
            "categories": files["categories"].get("slimefun", {}),
            "researches": files["researches"].get("slimefun", {}),
            "recipes": {
                **files["recipes"].get("slimefun", {}),
                **{f"minecraft:{key}": value for key, value in files["recipes"].get("minecraft", {}).items()},
            },
            **({"items": files["items"]} if "items" in files and code != "zh-CN" else {}),
        }

    data = {
        "order": selectable,
        "presets": list(presets),
        "default": flag("11b3188fd44902f72602bd7c2141f5a70673a411adb3d81862c69e536166b"),
        "help": flag("3edd20be93520949e6ce789dc4f43efaeb28c717ee6bfcbbe02780142f716"),
        "packs": packs,
    }
    entries = [
        f'  {json.dumps(code)}:{json.dumps(pack, ensure_ascii=False, separators=(",", ":"))}'
        for code, pack in command_packs.items()
    ]
    COMMAND_OUTPUT.write_text("{\n" + ",\n".join(entries) + "\n}\n", encoding="utf-8")
    OUTPUT.write_text(json.dumps(data, ensure_ascii=False, separators=(",", ":")) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
