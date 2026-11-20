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
        Tag::List(children) => {
            for child in children {
                plain_text(child, out);
            }
        }
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

fn literal(component: &Tag) -> String {
    let mut text = String::new();
    plain_text(component, &mut text);
    text
}

fn translations(id: &str) -> impl Iterator<Item = (&'static str, &'static str)> {
    ZH_CN["items"][id]["lore"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| Some((entry["source"].as_str()?, entry["target"].as_str()?)))
}

fn shared_lore(line: &str) -> Option<&'static str> {
    Some(match line {
        "Cycles through all Colors of the Rainbow!" => "&d在彩虹的各个颜色之间循环切换！",
        "[Christmas Edition]" => "&a[圣诞节版]",
        "[Valentine's Day Edition]" => "&5[&d情人节版&5]",
        "[Halloween Edition]" => "&c[&6万圣节版&c]",
        "Right Click to use" => "&e右键&7 使用",
        "Right Click to open" => "&e右键&7 打开",
        "Crouch to use" => "&e潜行&7 使用",
        _ => return None,
    })
}

/// Localize newly created Slimefun items without changing their identity or state.
pub(crate) fn localize_item_components(id: &str, components: &mut Compound) {
    let name = item_name(id);
    let base = ["ORGANIC_FOOD", "FERTILIZER"].into_iter().find(|base| {
        id.strip_prefix(*base)
            .is_some_and(|suffix| suffix.starts_with('_'))
    });
    if let Some(display_name) = base.and_then(item_name).or(name) {
        components.insert("minecraft:custom_name".into(), colored_lore(display_name));
    }

    let Some(Tag::List(lore)) = components.get_mut(&"minecraft:lore".into()) else {
        return;
    };
    let translations: Vec<_> = translations(id)
        .map(|(source, target)| (literal(&colored_lore(source)), target))
        .collect();
    for line in lore {
        let displayed = literal(line);
        let content = base.and(name).filter(|_| displayed.starts_with("Content: "));
        let translated = content
            .or_else(|| {
                translations.iter().find_map(|(source, target)| {
                    (source.trim() == displayed.trim()).then_some(*target)
                })
            })
            .or_else(|| shared_lore(&displayed));
        if let Some(target) = translated {
            *line = colored_lore(target);
        }
    }
}

/// The guide can omit untranslated hints and fill in missing reference descriptions.
pub(in crate::guide) fn item_icon(mut stack: ItemStack, id: &str) -> Option<ItemStack> {
    let name = item_name(id).unwrap_or("远古工艺物品");
    let mut translations: Vec<_> = translations(id)
        .map(|(source, target)| (literal(&colored_lore(source)), target, false))
        .collect();
    let mut lore = Vec::new();
    if let Some(Tag::List(lines)) = stack
        .components()
        .and_then(|patch| patch.as_nbt().get(&"minecraft:lore".into()))
    {
        for line in lines {
            let displayed = literal(line);
            if let Some((_, target, shown)) = translations
                .iter_mut()
                .find(|(source, _, _)| source.trim() == displayed.trim())
            {
                *shown = true;
                lore.push(colored_lore(target));
            } else if let Some(target) = shared_lore(&displayed) {
                lore.push(colored_lore(target));
            } else if !displayed.chars().any(|ch| ch.is_ascii_alphabetic())
                || displayed.chars().any(|ch| ('\u{4e00}'..='\u{9fff}').contains(&ch))
            {
                lore.push(line.clone());
            }
        }
    }
    for (source, target, shown) in translations {
        // Dynamic templates, such as a spawner's mob type, need a value from the stack.
        if !shown && !source.contains('<') {
            let line = colored_lore(target);
            if !lore.contains(&line) {
                lore.push(line);
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
