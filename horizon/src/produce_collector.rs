// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{LevelRef, ServerHost, Tag};

use crate::{
    items::{ALUMINUM_BRASS_INGOT, ELECTRIC_MOTOR, Kind, item, text},
    network_recipes::MEDIUM_CAPACITOR,
    recipes::{CraftingRecipe, Research, Stack},
};

item!(
    PRODUCE_COLLECTOR,
    "hay_block",
    "Produce Collector",
    "aqua",
    None,
    None
);

pub(crate) const CAPACITY: u32 = 256;
pub(crate) const COST: u32 = 16;
pub(crate) const WORK_TICKS: u32 = 10;
pub(crate) const DEFAULT_RANGE: u8 = 2;
pub(crate) const RESEARCH: Research = Research {
    id: "produce_collector",
    name: "Automatic Milking",
    cost: 20,
};

const BUCKET: Kind = Kind::Vanilla("minecraft:bucket");
const HAY: Kind = Kind::Vanilla("minecraft:hay_block");

pub(crate) const CRAFTING: CraftingRecipe = CraftingRecipe {
    inputs: [
        None,
        Some(HAY),
        None,
        Some(BUCKET),
        Some(MEDIUM_CAPACITOR),
        Some(BUCKET),
        Some(ALUMINUM_BRASS_INGOT),
        Some(ELECTRIC_MOTOR),
        Some(ALUMINUM_BRASS_INGOT),
    ],
    output: Stack {
        kind: PRODUCE_COLLECTOR,
        count: 1,
    },
    research: &RESEARCH,
};

#[derive(Clone, Copy)]
pub(super) struct Produce {
    pub input: Kind,
    pub output: Kind,
    animals: &'static [&'static str],
}

pub(super) const RECIPES: &[Produce] = &[
    Produce {
        input: BUCKET,
        output: Kind::Vanilla("minecraft:milk_bucket"),
        animals: &["minecraft:cow", "minecraft:goat"],
    },
    Produce {
        input: Kind::Vanilla("minecraft:bowl"),
        output: Kind::Vanilla("minecraft:mushroom_stew"),
        animals: &["minecraft:mooshroom"],
    },
];

impl Produce {
    pub(super) fn has_animal(
        self,
        host: &dyn ServerHost,
        level: LevelRef,
        pos: [i32; 3],
        range: u8,
    ) -> bool {
        let range = f64::from(range);
        let min = pos.map(|axis| f64::from(axis) - range);
        let max = pos.map(|axis| f64::from(axis) + range);
        level.entities_in(host, min, max).is_some_and(|entities| {
            entities.into_iter().any(|entity| {
                entity
                    .kind(host)
                    .is_some_and(|kind| self.animals.contains(&kind.id(host).as_str()))
                    && host.living(entity).is_some_and(|living| {
                        living.baby(host) == Some(false)
                            && living.health(host).is_some_and(|health| health > 0.0)
                    })
            })
        })
    }
}

pub(crate) fn lore() -> Tag {
    // Paper's item tooltip says 512 J. The registered machine capacity is 256 J.
    let power = |value| {
        Tag::compound([
            ("text", Tag::String("".into())),
            ("italic", Tag::Byte(0)),
            (
                "extra",
                Tag::List(vec![
                    text("⇨ ", "dark_gray", false),
                    text("⚡ ", "yellow", false),
                    text(value, "gray", false),
                ]),
            ),
        ])
    };
    Tag::List(vec![
        Tag::String("".into()),
        text("This machine allows you to", "white", false),
        text("collect produce from nearby animals.", "white", false),
        Tag::String("".into()),
        text("Advanced Machine", "gold", false),
        power("512 J Buffer"),
        power("32 J/s"),
    ])
}
