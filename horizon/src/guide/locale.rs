// SPDX-License-Identifier: GPL-3.0-only

use std::sync::LazyLock;

use mod_api::{Compound, ItemStack, Tag};
use serde_json::Value;

use super::colored_lore;

static ZH_CN: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("zh-cn.json")).expect("generated simplified Chinese guide")
});

pub(in crate::guide) fn item_name(id: &str) -> Option<&'static str> {
    ZH_CN["items"][id]["name"].as_str()
}

pub(crate) fn research_name(id: &str) -> &'static str {
    ZH_CN["researches"][id].as_str().unwrap_or("未知研究")
}

fn plain_text(component: &Tag, out: &mut String) {
    match component {
        Tag::String(value) => out.push_str(&value.to_string_lossy()),
        Tag::Compound(_) => {
            if let Some(Tag::String(value)) = component.get("text") {
                out.push_str(&value.to_string_lossy());
            }
            if let Some(Tag::List(children)) = component.get("extra") {
                for child in children {
                    plain_text(child, out);
                }
            }
        }
        _ => {}
    }
}

/// Localize the menu's copy. The stack produced by recipes and stored in
/// inventories keeps its canonical components for stacking and interoperability.
pub(in crate::guide) fn item_icon(mut stack: ItemStack, id: &str) -> Option<ItemStack> {
    let name = item_name(id).unwrap_or("远古工艺物品");
    let translations = ZH_CN["items"][id]["lore"].as_array();
    let mut lore = Vec::new();
    if let Some(Tag::List(lines)) = stack
        .components()
        .and_then(|patch| patch.as_nbt().get(&"minecraft:lore".into()))
    {
        for line in lines {
            let mut displayed = String::new();
            plain_text(line, &mut displayed);
            let translated = translations.into_iter().flatten().find_map(|entry| {
                let mut source = String::new();
                plain_text(&colored_lore(entry["source"].as_str()?), &mut source);
                (source == displayed).then(|| entry["target"].as_str()).flatten()
            });
            if let Some(translated) = translated {
                lore.push(colored_lore(translated));
            } else if !displayed.chars().any(|ch| ch.is_ascii_alphabetic())
                || displayed.chars().any(|ch| ('\u{4e00}'..='\u{9fff}').contains(&ch))
            {
                lore.push(line.clone());
            }
        }
    }
    stack
        .apply_components(&Compound::from([
            ("minecraft:custom_name".into(), colored_lore(name)),
            ("minecraft:lore".into(), Tag::List(lore)),
        ]))
        .ok()?;
    Some(stack)
}
