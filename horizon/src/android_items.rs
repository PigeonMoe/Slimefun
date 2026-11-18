// SPDX-License-Identifier: GPL-3.0-only

use mod_api::Tag;

use crate::{
    fuel_generators::COAL_GENERATOR,
    heated_pressure::PLASTIC_SHEET,
    items::{ELECTRIC_MOTOR, HeadTexture, Kind, item, text},
    network_recipes::ANDROID_MEMORY_CORE,
    recipes::{CraftingRecipe, Research, Stack},
};

item!(
    PROGRAMMABLE_ANDROID,
    "player_head",
    "Programmable Android (Normal)",
    "red",
    None,
    Some(HeadTexture {
        profile: [-611664877, -1615382484, -1654842984, -1366702800],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvMzUwM2NiN2VkODQ1ZTdhNTA3ZjU2OWFmYzY0N2M0N2FjNDgzNzcxNDY1YzlhNjc5YTU0NTk0Yzc2YWZiYSJ9fX0="
    })
);
item!(
    PROGRAMMABLE_ANDROID_FARMER,
    "player_head",
    "Programmable Android (Farmer)",
    "red",
    None,
    Some(HeadTexture {
        profile: [1427415659, 644231234, -1727825369, 1915481921],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvZjlkMzMzNTdlODQxODgyM2JmNzgzZGU5MmRlODAyOTFiNGViZDM5MmFlYzg3MDY2OThlMDY4OTZkNDk4ZjYifX19"
    })
);

pub(crate) const RESEARCH: Research = Research {
    id: "programmable_androids",
    name: "Programmable Androids",
    cost: 50,
};

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    CraftingRecipe {
        inputs: [
            Some(PLASTIC_SHEET),
            Some(ANDROID_MEMORY_CORE),
            Some(PLASTIC_SHEET),
            Some(COAL_GENERATOR),
            Some(ELECTRIC_MOTOR),
            Some(Kind::Vanilla("minecraft:chest")),
            Some(PLASTIC_SHEET),
            Some(PLASTIC_SHEET),
            Some(PLASTIC_SHEET),
        ],
        output: Stack {
            kind: PROGRAMMABLE_ANDROID,
            count: 1,
        },
        research: &RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            None,
            None,
            None,
            Some(Kind::Vanilla("minecraft:diamond_hoe")),
            Some(PROGRAMMABLE_ANDROID),
            Some(Kind::Vanilla("minecraft:diamond_hoe")),
            None,
            Some(ELECTRIC_MOTOR),
            None,
        ],
        output: Stack {
            kind: PROGRAMMABLE_ANDROID_FARMER,
            count: 1,
        },
        research: &RESEARCH,
    },
];

pub(crate) fn item(id: &str) -> Option<Kind> {
    [PROGRAMMABLE_ANDROID, PROGRAMMABLE_ANDROID_FARMER]
        .into_iter()
        .find(|kind| kind.id() == id)
}

pub(crate) fn lore(id: &str) -> Option<Tag> {
    let kind = item(id)?;
    let function = if kind == PROGRAMMABLE_ANDROID_FARMER {
        "Farming"
    } else {
        "None"
    };
    Some(Tag::List(vec![
        Tag::String("".into()),
        text(&format!("⇨ Function: {function}"), "gray", false),
        text("⇨ Fuel Efficiency: 1.0x", "gray", false),
    ]))
}
