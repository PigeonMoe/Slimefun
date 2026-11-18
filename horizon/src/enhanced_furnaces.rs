// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{
    ComponentId, Compound, FurnaceProcessEvent, FurnaceProcessStage, Host, ItemResource, ItemStack,
    Tag,
};

use crate::{
    items::{
        BASIC_CIRCUIT_BOARD, CARBONADO, ELECTRIC_MOTOR, HEATING_COIL, Kind, REINFORCED_ALLOY_INGOT,
        STEEL_INGOT, item, text,
    },
    random::Rolls,
    recipes::{CraftingRecipe, Research, Stack},
};

item!(
    ENHANCED_FURNACE,
    "furnace",
    "Enhanced Furnace - I",
    "gray",
    None,
    None
);
item!(
    ENHANCED_FURNACE_2,
    "furnace",
    "Enhanced Furnace - II",
    "gray",
    None,
    None
);
item!(
    ENHANCED_FURNACE_3,
    "furnace",
    "Enhanced Furnace - III",
    "gray",
    None,
    None
);
item!(
    ENHANCED_FURNACE_4,
    "furnace",
    "Enhanced Furnace - IV",
    "gray",
    None,
    None
);
item!(
    ENHANCED_FURNACE_5,
    "furnace",
    "Enhanced Furnace - V",
    "gray",
    None,
    None
);
item!(
    ENHANCED_FURNACE_6,
    "furnace",
    "Enhanced Furnace - VI",
    "gray",
    None,
    None
);
item!(
    ENHANCED_FURNACE_7,
    "furnace",
    "Enhanced Furnace - VII",
    "gray",
    None,
    None
);
item!(
    ENHANCED_FURNACE_8,
    "furnace",
    "Enhanced Furnace - VIII",
    "gray",
    None,
    None
);
item!(
    ENHANCED_FURNACE_9,
    "furnace",
    "Enhanced Furnace - IX",
    "gray",
    None,
    None
);
item!(
    ENHANCED_FURNACE_10,
    "furnace",
    "Enhanced Furnace - X",
    "gray",
    None,
    None
);
item!(
    ENHANCED_FURNACE_11,
    "furnace",
    "Enhanced Furnace - XI",
    "gray",
    None,
    None
);
item!(
    REINFORCED_FURNACE,
    "furnace",
    "Reinforced Furnace",
    "gray",
    None,
    None
);
item!(
    CARBONADO_EDGED_FURNACE,
    "furnace",
    "Carbonado Edged Furnace",
    "gray",
    None,
    None
);

const BASIC: Research = Research {
    id: "enhanced_furnace",
    name: "Enhanced Furnace",
    cost: 7,
};
const BETTER: Research = Research {
    id: "more_enhanced_furnaces",
    name: "Better Furnaces",
    cost: 18,
};
const HIGH: Research = Research {
    id: "high_tier_enhanced_furnaces",
    name: "High Tier Furnace",
    cost: 29,
};
const REINFORCED: Research = Research {
    id: "reinforced_furnace",
    name: "Reinforced Furnace",
    cost: 32,
};
const CARBONADO_RESEARCH: Research = Research {
    id: "carbonado_furnace",
    name: "Carbonado Edged Furnace",
    cost: 35,
};
pub(crate) const RESEARCHES: &[Research] = &[BASIC, BETTER, HIGH, REINFORCED, CARBONADO_RESEARCH];

#[derive(Clone, Copy)]
pub(crate) struct Furnace {
    pub item: Kind,
    speed: i32,
    efficiency: i32,
    fortune: u32,
}

const fn furnace(item: Kind, speed: i32, efficiency: i32, fortune: u32) -> Furnace {
    Furnace {
        item,
        speed,
        efficiency,
        fortune,
    }
}

const FURNACES: &[Furnace] = &[
    furnace(ENHANCED_FURNACE, 1, 1, 1),
    furnace(ENHANCED_FURNACE_2, 2, 1, 1),
    furnace(ENHANCED_FURNACE_3, 2, 2, 1),
    furnace(ENHANCED_FURNACE_4, 3, 2, 1),
    furnace(ENHANCED_FURNACE_5, 3, 2, 2),
    furnace(ENHANCED_FURNACE_6, 3, 3, 2),
    furnace(ENHANCED_FURNACE_7, 4, 3, 2),
    furnace(ENHANCED_FURNACE_8, 4, 4, 2),
    furnace(ENHANCED_FURNACE_9, 5, 4, 2),
    furnace(ENHANCED_FURNACE_10, 5, 5, 2),
    furnace(ENHANCED_FURNACE_11, 5, 5, 3),
    furnace(REINFORCED_FURNACE, 10, 10, 3),
    furnace(CARBONADO_EDGED_FURNACE, 20, 10, 3),
];

pub(crate) fn by_id(id: &str) -> Option<Furnace> {
    FURNACES
        .iter()
        .copied()
        .find(|furnace| furnace.item.id() == id)
}

const fn standard(prior: Kind, upgraded: Kind, research: &'static Research) -> CraftingRecipe {
    CraftingRecipe {
        inputs: [
            None,
            Some(STEEL_INGOT),
            None,
            Some(BASIC_CIRCUIT_BOARD),
            Some(prior),
            Some(HEATING_COIL),
            None,
            Some(ELECTRIC_MOTOR),
            None,
        ],
        output: Stack {
            kind: upgraded,
            count: 1,
        },
        research,
    }
}

const fn plated(
    prior: Kind,
    upgraded: Kind,
    material: Kind,
    research: &'static Research,
) -> CraftingRecipe {
    CraftingRecipe {
        inputs: [
            Some(material),
            Some(BASIC_CIRCUIT_BOARD),
            Some(material),
            Some(HEATING_COIL),
            Some(prior),
            Some(HEATING_COIL),
            Some(material),
            Some(ELECTRIC_MOTOR),
            Some(material),
        ],
        output: Stack {
            kind: upgraded,
            count: 1,
        },
        research,
    }
}

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    standard(Kind::Vanilla("minecraft:furnace"), ENHANCED_FURNACE, &BASIC),
    standard(ENHANCED_FURNACE, ENHANCED_FURNACE_2, &BASIC),
    standard(ENHANCED_FURNACE_2, ENHANCED_FURNACE_3, &BETTER),
    standard(ENHANCED_FURNACE_3, ENHANCED_FURNACE_4, &BETTER),
    standard(ENHANCED_FURNACE_4, ENHANCED_FURNACE_5, &BETTER),
    standard(ENHANCED_FURNACE_5, ENHANCED_FURNACE_6, &BETTER),
    standard(ENHANCED_FURNACE_6, ENHANCED_FURNACE_7, &BETTER),
    standard(ENHANCED_FURNACE_7, ENHANCED_FURNACE_8, &HIGH),
    standard(ENHANCED_FURNACE_8, ENHANCED_FURNACE_9, &HIGH),
    standard(ENHANCED_FURNACE_9, ENHANCED_FURNACE_10, &HIGH),
    standard(ENHANCED_FURNACE_10, ENHANCED_FURNACE_11, &HIGH),
    plated(
        ENHANCED_FURNACE_11,
        REINFORCED_FURNACE,
        REINFORCED_ALLOY_INGOT,
        &REINFORCED,
    ),
    plated(
        REINFORCED_FURNACE,
        CARBONADO_EDGED_FURNACE,
        CARBONADO,
        &CARBONADO_RESEARCH,
    ),
];

impl Furnace {
    pub(crate) fn fuel(self, original: i32) -> i32 {
        let multiplier = self.efficiency - 1;
        if multiplier == 0 {
            original
        } else {
            original
                .saturating_mul(multiplier)
                .min(i32::from(i16::MAX) - 1)
        }
    }

    pub(crate) fn progress(self, game_time: i64, elapsed: i32, total: i32) -> i32 {
        if game_time % 10 != 0 || elapsed <= 0 || total <= elapsed {
            return elapsed;
        }
        elapsed.saturating_add((self.speed - 1) * 10).min(total - 1)
    }

    /// Paper's tag includes raw items and the three ore groups. Ancient debris
    /// and raw-metal storage blocks do not receive this bonus.
    pub(crate) fn quantity(
        self,
        material: &str,
        spare: u32,
        roll: impl FnOnce(u32) -> Option<u32>,
    ) -> u32 {
        let kind = material.strip_prefix("minecraft:");
        let ore = kind.is_some_and(|item| {
            matches!(
                item.strip_prefix("deepslate_").unwrap_or(item),
                "coal_ore"
                    | "iron_ore"
                    | "copper_ore"
                    | "gold_ore"
                    | "redstone_ore"
                    | "emerald_ore"
                    | "lapis_ore"
                    | "diamond_ore"
            )
        });
        let material = matches!(
            kind,
            Some("nether_quartz_ore" | "nether_gold_ore" | "raw_iron" | "raw_copper" | "raw_gold")
        );
        let amount = if ore || material {
            1 + roll(self.fortune + 1).unwrap_or(0)
        } else {
            1
        };
        spare.min(amount)
    }
}

pub(crate) fn decorate(id: &str, name: &str, components: &mut Compound) {
    let Some(furnace) = by_id(id) else {
        return;
    };
    if let Some((title, tier)) = name.split_once(" - ") {
        components.insert(
            "minecraft:custom_name".into(),
            line(vec![
                text(title, "gray", false),
                text(" - ", "gray", false),
                text(tier, "yellow", false),
            ]),
        );
    }
    let mut lore = vec![Tag::String("".into())];
    for (label, value) in [
        ("Processing Speed: ", furnace.speed),
        ("Fuel Efficiency: ", furnace.efficiency),
        ("Luck Multiplier: ", furnace.fortune as i32),
    ] {
        let value = format!("{value}x");
        lore.push(line(vec![
            text(label, "gray", false),
            text(&value, "yellow", false),
        ]));
    }
    components.insert("minecraft:lore".into(), Tag::List(lore));
}

fn line(children: Vec<Tag>) -> Tag {
    Tag::compound([
        ("text", Tag::String("".into())),
        ("italic", Tag::Byte(0)),
        ("extra", Tag::List(children)),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paper_lore_and_runtime_multipliers_are_distinct() {
        let first = by_id("ENHANCED_FURNACE").unwrap();
        assert_eq!(first.fuel(1600), 1600);
        assert_eq!(
            first.quantity("minecraft:raw_iron", 64, |bound| Some(bound - 1)),
            2
        );
        let third = by_id("ENHANCED_FURNACE_3").unwrap();
        assert_eq!(third.fuel(1600), 1600);
        assert_eq!(third.progress(20, 85, 100), 95);
        assert_eq!(third.progress(21, 85, 100), 85);
        let final_tier = by_id("CARBONADO_EDGED_FURNACE").unwrap();
        assert_eq!(final_tier.fuel(20_000), 32766);
        assert_eq!(final_tier.progress(20, 5, 200), 195);
        assert_eq!(final_tier.progress(20, 9, 200), 199);
        assert_eq!(final_tier.progress(20, 0, 200), 0);
        assert_eq!(
            final_tier.quantity("minecraft:deepslate_gold_ore", 2, |bound| Some(bound - 1)),
            2
        );
        assert_eq!(
            final_tier.quantity("minecraft:nether_quartz_ore", 64, |bound| Some(bound - 1)),
            4
        );
        for material in [
            "minecraft:ancient_debris",
            "minecraft:raw_iron_block",
            "modded:gold_ore",
        ] {
            assert_eq!(final_tier.quantity(material, 64, |_| unreachable!()), 1);
        }
        assert_eq!(CRAFTING.len(), FURNACES.len());
        for (index, (recipe, furnace)) in CRAFTING.iter().zip(FURNACES).enumerate() {
            assert_eq!(recipe.output.kind, furnace.item);
            let prior = if index == 0 {
                Kind::Vanilla("minecraft:furnace")
            } else {
                FURNACES[index - 1].item
            };
            assert_eq!(recipe.inputs[4], Some(prior));
        }
    }
}

pub(crate) fn process(host: &dyn Host, event: &mut FurnaceProcessEvent<'_>, rolls: &Rolls) {
    if matches!(event.stage, FurnaceProcessStage::Progress { .. }) && event.game_time % 10 != 0 {
        return;
    }
    let Some(furnace) = crate::network_runtime::enhanced_furnace(host, event.entity) else {
        return;
    };
    let Some(level) = event.entity.level(host) else {
        return;
    };
    let Some(pos) = event.entity.position(host) else {
        return;
    };
    if level
        .block(host, pos)
        .is_none_or(|block| block.id(host) != "minecraft:furnace")
    {
        return;
    }
    match event.stage {
        FurnaceProcessStage::Fuel { duration, .. } => {
            *duration = furnace.fuel(*duration);
        }
        FurnaceProcessStage::Progress { elapsed, total } => {
            *elapsed = furnace.progress(event.game_time, *elapsed, *total);
        }
        FurnaceProcessStage::Result {
            input,
            previous,
            result,
        } => {
            let Some(proposed) = result else {
                return;
            };
            let Some(max_stack_size) = ComponentId::new("minecraft:max_stack_size") else {
                return;
            };
            let Some(Tag::Int(limit)) = ItemResource::of(proposed).value(host, &max_stack_size)
            else {
                return;
            };
            let existing = previous.as_ref().map_or(0, ItemStack::count);
            let spare = u32::try_from(limit)
                .unwrap_or_default()
                .saturating_sub(existing);
            let count = furnace.quantity(&input.item().id(host), spare, |bound| {
                rolls.next(host, level, bound)
            });
            // Paper constructs a new material stack instead of copying the recipe result's components.
            *result = ItemStack::new(proposed.item(), count);
        }
        _ => {}
    }
}
