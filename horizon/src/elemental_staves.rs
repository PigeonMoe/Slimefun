// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{Compound, GameMode, ItemStack, PlayerRef, ServerHost, SoundSource, Tag, Text};

use crate::{
    ancient_altar::ANCIENT_RUNE_FIRE,
    items::{
        ENDER_LUMP_3, HeadTexture, Kind, MAGIC_LUMP_1, MAGIC_LUMP_2, MAGIC_LUMP_3, item, item_id,
        text,
    },
    recipes::{CraftingRecipe, Research, Stack},
    research,
};

item!(
    MAGICAL_BOOK_COVER,
    "paper",
    "Magical Book Cover",
    "gold",
    None,
    None
);
item!(
    STAFF_ELEMENTAL,
    "stick",
    "Elemental Staff",
    "gold",
    None,
    None
);
item!(
    STAFF_ELEMENTAL_WIND,
    "stick",
    "Elemental Staff - Wind",
    "gold",
    None,
    None
);
item!(
    STAFF_ELEMENTAL_WATER,
    "stick",
    "Elemental Staff - Water",
    "gold",
    None,
    None
);
item!(
    STAFF_ELEMENTAL_FIRE,
    "stick",
    "Elemental Staff - Fire",
    "gold",
    None,
    None
);
item!(
    LAVA_CRYSTAL,
    "player_head",
    "Lava Crystal",
    "dark_red",
    None,
    Some(HeadTexture {
        profile: [31466098, 1526413804, -1566361612, -715947666],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2EzYWQ4ZWU4NDllZGYwNGVkOWEyNmNhMzM0MWY2MDMzYmQ3NmRjYzQyMzFlZDFlYTYzYjc1NjU3NTFiMjdhYyJ9fX0=",
    })
);

const BOOK_BINDING: Research = Research {
    id: "magical_book_cover",
    name: "Magical Book Binding",
    cost: 5,
};
const ELEMENTAL: Research = Research {
    id: "elemental_staff",
    name: "Elemental Staves",
    cost: 17,
};
const WIND: Research = Research {
    id: "wind_staff",
    name: "Wind Staff",
    cost: 17,
};
const WATER: Research = Research {
    id: "water_staff",
    name: "Water Staff",
    cost: 8,
};
const FIRE: Research = Research {
    id: "fire_staff",
    name: "Fire Staff",
    cost: 2,
};
const FIERY: Research = Research {
    id: "lava_crystal",
    name: "Firey Situation",
    cost: 14,
};
pub(crate) const RESEARCHES: &[&Research] =
    &[&BOOK_BINDING, &ELEMENTAL, &WIND, &WATER, &FIRE, &FIERY, &crate::storm_staff::RESEARCH];

const BOOK: Option<Kind> = Some(Kind::Vanilla("minecraft:book"));
const STICK: Option<Kind> = Some(Kind::Vanilla("minecraft:stick"));
const FEATHER: Option<Kind> = Some(Kind::Vanilla("minecraft:feather"));
const LILY: Option<Kind> = Some(Kind::Vanilla("minecraft:lily_pad"));
const POWDER: Option<Kind> = Some(Kind::Vanilla("minecraft:blaze_powder"));
const COVER: Option<Kind> = Some(MAGICAL_BOOK_COVER);
const BASE: Option<Kind> = Some(STAFF_ELEMENTAL);
const LUMP_1: Option<Kind> = Some(MAGIC_LUMP_1);
const LUMP_2: Option<Kind> = Some(MAGIC_LUMP_2);
const LUMP_3: Option<Kind> = Some(MAGIC_LUMP_3);

const fn craft(
    inputs: [Option<Kind>; 9],
    kind: Kind,
    research: &'static Research,
) -> CraftingRecipe {
    CraftingRecipe {
        inputs,
        output: Stack { kind, count: 1 },
        research,
    }
}

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    craft(
        [None, LUMP_2, None, LUMP_2, BOOK, LUMP_2, None, LUMP_2, None],
        MAGICAL_BOOK_COVER,
        &BOOK_BINDING,
    ),
    craft(
        [
            LUMP_1,
            POWDER,
            LUMP_1,
            POWDER,
            Some(ANCIENT_RUNE_FIRE),
            POWDER,
            LUMP_1,
            POWDER,
            LUMP_1,
        ],
        LAVA_CRYSTAL,
        &FIERY,
    ),
];

pub(crate) const MAGIC_CRAFTING: &[CraftingRecipe] = &[
    craft(
        [None, COVER, LUMP_3, None, STICK, COVER, LUMP_3, None, None],
        STAFF_ELEMENTAL,
        &ELEMENTAL,
    ),
    craft(
        [
            None,
            FEATHER,
            Some(ENDER_LUMP_3),
            None,
            BASE,
            FEATHER,
            BASE,
            None,
            None,
        ],
        STAFF_ELEMENTAL_WIND,
        &WIND,
    ),
    craft(
        [None, LILY, LUMP_2, None, BASE, LILY, BASE, None, None],
        STAFF_ELEMENTAL_WATER,
        &WATER,
    ),
    craft(
        [
            None,
            None,
            Some(LAVA_CRYSTAL),
            None,
            BASE,
            None,
            BASE,
            None,
            None,
        ],
        STAFF_ELEMENTAL_FIRE,
        &FIRE,
    ),
];

pub(crate) const DEFAULT_WIND_POWER: u32 = 4;

pub(crate) fn wind_power(config: Option<&toml::Value>) -> u32 {
    config
        .and_then(|config| config.get("power"))
        .and_then(toml::Value::as_integer)
        .and_then(|number| i32::try_from(number).ok())
        .filter(|number| *number >= 1)
        .map_or(DEFAULT_WIND_POWER, |number| number as u32)
}

pub(crate) fn decorate(id: &str, components: &mut Compound) {
    let blank = || Tag::String("".into());
    if id == MAGICAL_BOOK_COVER.id() {
        components.insert(
            "minecraft:lore".into(),
            Tag::List(vec![
                blank(),
                text("Used for various Magic Books", "green", true),
            ]),
        );
        return;
    }
    let (element, color, instruction) = match id {
        "STAFF_ELEMENTAL_WIND" => ("Wind", "aqua", Some(" to launch yourself forward")),
        "STAFF_ELEMENTAL_WATER" => ("Water", "dark_blue", Some(" to extinguish yourself")),
        "STAFF_ELEMENTAL_FIRE" => {
            components.insert(
                "minecraft:enchantments".into(),
                Tag::compound([("minecraft:fire_aspect", Tag::Int(5))]),
            );
            ("Fire", "red", None)
        }
        _ => return,
    };
    components.insert(
        "minecraft:custom_name".into(),
        styled([
            text("Elemental Staff", "gold", false),
            text(" - ", "gray", false),
            text(element, color, true),
        ]),
    );
    let mut lore = vec![
        blank(),
        styled([text("Element: ", "gray", false), text(element, color, true)]),
    ];
    if let Some(instruction) = instruction {
        lore.extend([
            blank(),
            styled([
                text("Right Click", "yellow", false),
                text(instruction, "gray", false),
            ]),
        ]);
    }
    components.insert("minecraft:lore".into(), Tag::List(lore));
}

fn styled<const N: usize>(parts: [Tag; N]) -> Tag {
    Tag::compound([
        ("text", Tag::String("".into())),
        ("italic", Tag::Byte(0)),
        ("extra", Tag::List(parts.into())),
    ])
}

fn wind_cost(level: i32, creative: bool) -> Option<i32> {
    (level >= 2).then_some(if creative { 0 } else { 2 })
}

pub(crate) fn use_staff(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    stack: &ItemStack,
    power: impl FnOnce() -> u32,
) -> bool {
    let (kind, research) = match item_id(stack).as_deref() {
        Some("STAFF_ELEMENTAL_WIND") => (STAFF_ELEMENTAL_WIND, &WIND),
        Some("STAFF_ELEMENTAL_WATER") => (STAFF_ELEMENTAL_WATER, &WATER),
        _ => return false,
    };
    if !kind.matches(host, stack) {
        return false;
    }
    let Some(mode @ (GameMode::Survival | GameMode::Adventure | GameMode::Creative)) =
        player.game_mode(host)
    else {
        return false;
    };
    if !research::can_use(host, player, research) {
        let _ = player.tell(
            host,
            Text::literal(format!(
                "Unlock {} with /sf research {}.",
                research.name, research.id
            )),
        );
        return true;
    }
    let entity = player.entity();
    if kind == STAFF_ELEMENTAL_WATER {
        let _ = entity.set_remaining_fire_ticks(host, 0);
        let mut text = Text::literal("You have extinguished yourself");
        text.style.color = Some("gray".into());
        let _ = player.tell(host, text);
        return true;
    }
    use_wind(host, player, power, mode != GameMode::Creative);
    true
}

pub(crate) fn use_wind(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    power: impl FnOnce() -> u32,
    spend_food: bool,
) {
    let entity = player.entity();
    let Some(food) = player.food(host) else {
        return;
    };
    let Some(cost) = wind_cost(food.level, !spend_food) else {
        let mut text = Text::literal("You are too hungry to do that!");
        text.style.color = Some("red".into());
        let _ = player.tell(host, text);
        return;
    };
    let Some(view) = entity.view_vector(host) else {
        return;
    };
    let power = f64::from(power());
    if !entity.set_velocity(host, view.map(|axis| axis * power)) {
        return;
    }
    if cost != 0 {
        let _ = player.set_food_level(host, food.level - cost);
    }
    let _ = player.play_sound(
        host,
        "minecraft:entity.tnt.primed",
        SoundSource::Players,
        1.0,
        1.0,
    );
    if let Some(level) = entity.level(host)
        && let Some(pos) = entity.block_position(host)
    {
        let _ = level.level_event(host, 2000, pos, 1);
    }
    let _ = entity.reset_fall_distance(host);
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Identifier, testing::FakeHost};

    #[test]
    fn elemental_staves_and_prerequisites_match_the_paper_recipe_matrices() {
        let expected = [
            (
                MAGICAL_BOOK_COVER,
                "magical_book_cover",
                5,
                [
                    "",
                    "MAGIC_LUMP_2",
                    "",
                    "MAGIC_LUMP_2",
                    "minecraft:book",
                    "MAGIC_LUMP_2",
                    "",
                    "MAGIC_LUMP_2",
                    "",
                ],
            ),
            (
                LAVA_CRYSTAL,
                "lava_crystal",
                14,
                [
                    "MAGIC_LUMP_1",
                    "minecraft:blaze_powder",
                    "MAGIC_LUMP_1",
                    "minecraft:blaze_powder",
                    "ANCIENT_RUNE_FIRE",
                    "minecraft:blaze_powder",
                    "MAGIC_LUMP_1",
                    "minecraft:blaze_powder",
                    "MAGIC_LUMP_1",
                ],
            ),
            (
                STAFF_ELEMENTAL,
                "elemental_staff",
                17,
                [
                    "",
                    "MAGICAL_BOOK_COVER",
                    "MAGIC_LUMP_3",
                    "",
                    "minecraft:stick",
                    "MAGICAL_BOOK_COVER",
                    "MAGIC_LUMP_3",
                    "",
                    "",
                ],
            ),
            (
                STAFF_ELEMENTAL_WIND,
                "wind_staff",
                17,
                [
                    "",
                    "minecraft:feather",
                    "ENDER_LUMP_3",
                    "",
                    "STAFF_ELEMENTAL",
                    "minecraft:feather",
                    "STAFF_ELEMENTAL",
                    "",
                    "",
                ],
            ),
            (
                STAFF_ELEMENTAL_WATER,
                "water_staff",
                8,
                [
                    "",
                    "minecraft:lily_pad",
                    "MAGIC_LUMP_2",
                    "",
                    "STAFF_ELEMENTAL",
                    "minecraft:lily_pad",
                    "STAFF_ELEMENTAL",
                    "",
                    "",
                ],
            ),
            (
                STAFF_ELEMENTAL_FIRE,
                "fire_staff",
                2,
                [
                    "",
                    "",
                    "LAVA_CRYSTAL",
                    "",
                    "STAFF_ELEMENTAL",
                    "",
                    "STAFF_ELEMENTAL",
                    "",
                    "",
                ],
            ),
        ];
        for (recipe, (kind, id, cost, inputs)) in
            CRAFTING.iter().chain(MAGIC_CRAFTING).zip(expected)
        {
            assert_eq!(
                recipe.inputs.map(|input| input.map_or("", Kind::id)),
                inputs
            );
            assert_eq!((recipe.output.kind, recipe.output.count), (kind, 1));
            assert_eq!((recipe.research.id, recipe.research.cost), (id, cost));
        }
    }

    #[test]
    fn wind_needs_two_food_even_in_creative_and_respects_paper_power_range() {
        assert_eq!(wind_cost(1, false), None);
        assert_eq!(wind_cost(1, true), None);
        assert_eq!(wind_cost(2, false), Some(2));
        assert_eq!(wind_cost(2, true), Some(0));
        for (configured, expected) in [
            (None, 4),
            (Some(0), 4),
            (Some(1), 1),
            (Some(12), 12),
            (Some(i64::from(i32::MAX) + 1), 4),
        ] {
            let config = configured.map(|n| {
                toml::Value::Table(toml::Table::from_iter([(
                    "power".into(),
                    toml::Value::Integer(n),
                )]))
            });
            assert_eq!(wind_power(config.as_ref()), expected);
        }
    }

    #[test]
    fn fire_staff_keeps_the_paper_fire_aspect_v() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse("minecraft:stick").unwrap()])
            .unwrap();
        let fire = STAFF_ELEMENTAL_FIRE.stack(&host, 1).unwrap();
        assert_eq!(
            fire.components()
                .unwrap()
                .as_nbt()
                .get(&"minecraft:enchantments".into())
                .and_then(|ench| ench.get("minecraft:fire_aspect")),
            Some(&Tag::Int(5))
        );
    }
}
