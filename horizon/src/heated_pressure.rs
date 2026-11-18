// SPDX-License-Identifier: GPL-3.0-only

use crate::{
    items::{CARBONADO, GOLD_24K, HeadTexture, Kind, MAGNESIUM_DUST, SALT, item},
    network_recipes::BLISTERING_INGOT_3,
    recipes::Stack,
};

item!(
    HEATED_PRESSURE_CHAMBER,
    "light_gray_stained_glass",
    "Heated Pressure Chamber",
    "red",
    None,
    None
);
item!(
    HEATED_PRESSURE_CHAMBER_2,
    "light_gray_stained_glass",
    "Heated Pressure Chamber - II",
    "red",
    None,
    None
);
item!(
    TINY_URANIUM,
    "player_head",
    "Tiny Pile of Uranium",
    "red",
    None,
    Some(HeadTexture {
        profile: [1828052373, -159040324, -2058291774, -1124291328],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2M4YjI5YWZhNmQ2ZGM5MjNlMmUxMzI0YmY4MTkyNzUwZjdiZGJkZGM2ODk2MzJhMmI2YzE4ZDlmZTdhNWUifX19"
    })
);
item!(
    SMALL_URANIUM,
    "player_head",
    "Small Chunk of Uranium",
    "red",
    None,
    Some(HeadTexture {
        profile: [1828052373, -159040324, -2058291774, -1124291328],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2M4YjI5YWZhNmQ2ZGM5MjNlMmUxMzI0YmY4MTkyNzUwZjdiZGJkZGM2ODk2MzJhMmI2YzE4ZDlmZTdhNWUifX19"
    })
);
item!(
    URANIUM,
    "player_head",
    "Uranium",
    "dark_red",
    None,
    Some(HeadTexture {
        profile: [1828052373, -159040324, -2058291774, -1124291328],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2M4YjI5YWZhNmQ2ZGM5MjNlMmUxMzI0YmY4MTkyNzUwZjdiZGJkZGM2ODk2MzJhMmI2YzE4ZDlmZTdhNWUifX19"
    })
);
item!(
    BUCKET_OF_OIL,
    "player_head",
    "Bucket of Oil",
    "white",
    None,
    Some(HeadTexture {
        profile: [-1604049462, -716819284, -2100117910, 1969260707],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzZjZTA0YjQxZDE5ZWM3OTI3Zjk4MmE2M2E5NGEzZDc5Zjc4ZWNlYzMzMzYzMDUxZmRlMDgzMWJmYWJkYmQifX19"
    })
);
item!(PLASTIC_SHEET, "paper", "Plastic Sheet", "white", None, None);
item!(
    BLISTERING_INGOT,
    "gold_ingot",
    "Blistering Ingot (33%)",
    "gold",
    None,
    None
);
item!(
    BLISTERING_INGOT_2,
    "gold_ingot",
    "Blistering Ingot (66%)",
    "gold",
    None,
    None
);
item!(
    PLUTONIUM,
    "player_head",
    "Plutonium",
    "gray",
    None,
    Some(HeadTexture {
        profile: [-906569369, -597542983, -1587824617, -1223426625],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzI1Y2Y5MWI3Mzg4NjY1YTZkN2MxYjYwMjZiZGIyMzIyYzZkMjc4OTk3YTQ0NDc4Njc3Y2JjYzE1Zjc2MTI0ZiJ9fX0="
    })
);
item!(
    BOOSTED_URANIUM,
    "player_head",
    "Boosted Uranium",
    "dark_green",
    None,
    Some(HeadTexture {
        profile: [996486925, -268226073, -1585987113, -554402598],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzY4MzdjYTEyZjIyMmY0Nzg3MTk2YTE3YjhhYjY1Njk4NWY4NDA0YzUwNzY3YWRiY2I2ZTdmMTQyNTRmZWUifX19"
    })
);
item!(
    NETHER_ICE,
    "player_head",
    "Nether Ice",
    "yellow",
    None,
    Some(HeadTexture {
        profile: [-1003022059, 219035450, -1119522091, 442618726],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzNjZTJkYWQ5YmFmN2VhYmE3ZTgwZDRkMGY5ZmFjMGFhYjAxYTc2YjEyZmI3MWMzZDJhZjJhMTZmZGQ0YzczODMifX19"
    })
);
item!(
    ENRICHED_NETHER_ICE,
    "player_head",
    "Enriched Nether Ice",
    "yellow",
    None,
    Some(HeadTexture {
        profile: [-2128690400, 1418542637, -1852002904, 1159172858],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzdjODE4YWExM2FhYmM3Mjk0ODM4ZDIxY2FhYzA1N2U5N2JkOGM4OTY0MWEwYzBmOGE1NTQ0MmZmNGUyNyJ9fX0="
    })
);
item!(
    NETHER_ICE_COOLANT_CELL,
    "player_head",
    "Nether Ice Coolant Cell",
    "gold",
    None,
    Some(HeadTexture {
        profile: [-1057242607, 973222170, -1308579245, 1564993814],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzhkM2NkNDEyNTU1Zjg5NzAxNjIxM2U1ZDZjNzQzMWI0NDhiOWU1NjQ0ZTFiMTllYzUxYjUzMTZmMzU4NDBlMCJ9fX0="
    })
);
item!(MAGNESIUM_SALT, "sugar", "Magnesium Salt", "red", None, None);

pub(crate) fn network_item(id: &str) -> Option<Kind> {
    [HEATED_PRESSURE_CHAMBER, HEATED_PRESSURE_CHAMBER_2]
        .into_iter()
        .find(|item| item.id() == id)
}

pub(crate) struct HeatedRecipe {
    pub seconds: u32,
    pub inputs: &'static [Stack],
    pub output: Stack,
}

const fn recipe(seconds: u32, inputs: &'static [Stack], output: Stack) -> HeatedRecipe {
    HeatedRecipe {
        seconds,
        inputs,
        output,
    }
}

const fn stack(kind: Kind, count: u32) -> Stack {
    Stack { kind, count }
}

// Exact registration order from HeatedPressureChamber.registerDefaultRecipes.
pub(crate) const RECIPES: &[HeatedRecipe] = &[
    recipe(45, &[stack(BUCKET_OF_OIL, 1)], stack(PLASTIC_SHEET, 8)),
    recipe(
        30,
        &[stack(GOLD_24K, 1), stack(URANIUM, 1)],
        stack(BLISTERING_INGOT, 1),
    ),
    recipe(
        30,
        &[stack(BLISTERING_INGOT, 1), stack(CARBONADO, 1)],
        stack(BLISTERING_INGOT_2, 1),
    ),
    recipe(
        60,
        &[
            stack(BLISTERING_INGOT_2, 1),
            stack(Kind::Vanilla("minecraft:nether_star"), 1),
        ],
        stack(BLISTERING_INGOT_3, 1),
    ),
    recipe(
        90,
        &[stack(PLUTONIUM, 1), stack(URANIUM, 1)],
        stack(BOOSTED_URANIUM, 1),
    ),
    recipe(
        60,
        &[stack(NETHER_ICE, 1), stack(PLUTONIUM, 1)],
        stack(ENRICHED_NETHER_ICE, 4),
    ),
    recipe(
        45,
        &[stack(ENRICHED_NETHER_ICE, 1)],
        stack(NETHER_ICE_COOLANT_CELL, 8),
    ),
    recipe(
        8,
        &[stack(MAGNESIUM_DUST, 1), stack(SALT, 1)],
        stack(MAGNESIUM_SALT, 1),
    ),
];
