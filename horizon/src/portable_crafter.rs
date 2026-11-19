// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{ItemStack, PlayerRef, ServerHost, SoundSource, Tag};

use crate::{
    items::{HeadTexture, Kind, item, text},
    recipes::{CraftingRecipe, Research, Stack},
    research,
};

item!(
    PORTABLE_CRAFTER,
    "player_head",
    "Portable Crafter",
    "gold",
    None,
    Some(HeadTexture {
        profile: [1419369518, -1126089183, -1497099438, 945621281],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzcyZWM0YTRiZDhhNThmODM2MWY4YTAzMDNlMjE5OWQzM2Q2MjRlYTVmOTJmN2NiMzQxNGZlZTk1ZTJkODYxIn19fQ=="
    })
);

pub(crate) const RESEARCH: Research = Research {
    id: "portable_crafter",
    name: "Portable Crafter",
    cost: 1,
};
pub(crate) const CRAFTING: CraftingRecipe = CraftingRecipe {
    inputs: [
        Some(Kind::Vanilla("minecraft:book")),
        Some(Kind::Vanilla("minecraft:crafting_table")),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    ],
    output: Stack {
        kind: PORTABLE_CRAFTER,
        count: 1,
    },
    research: &RESEARCH,
};

pub(crate) fn lore() -> Tag {
    Tag::List(vec![
        text("A portable Crafting Table", "green", true),
        text("", "white", false),
        text("Right Click to open", "gray", false),
    ])
}

pub(crate) fn use_held(host: &mut dyn ServerHost, player: PlayerRef, stack: &ItemStack) -> bool {
    if !PORTABLE_CRAFTER.matches(host, stack) || !research::can_use(host, player, &RESEARCH) {
        return false;
    }
    if player.open_crafting_menu(host).is_some()
        && let Some(level) = player.entity().level(host)
        && let Some(at) = player.entity().position(host)
    {
        let _ = level.play_sound(
            host,
            "minecraft:block.wooden_button.click_on",
            at,
            SoundSource::Players,
            1.0,
            1.0,
        );
    }
    true
}
