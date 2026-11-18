// SPDX-License-Identifier: GPL-3.0-only

use std::sync::LazyLock;

use mod_api::{Host, ItemStack};

use crate::{
    electric_processing::ELECTRIC_INGOT_FACTORY,
    fuel_generators::BUCKET_OF_FUEL,
    heated_pressure::BUCKET_OF_OIL,
    items::{
        ALUMINUM_DUST, ALUMINUM_INGOT, CORINTHIAN_BRONZE_INGOT, DAMASCUS_STEEL_INGOT,
        ELECTRIC_MOTOR, GILDED_IRON, HEATING_COIL, Kind, LEAD_INGOT, REDSTONE_ALLOY, STEEL_PLATE,
        item,
    },
    network_recipes::{HARDENED_GLASS, LARGE_CAPACITOR, POWER_CRYSTAL, SYNTHETIC_SAPPHIRE},
    recipes::{self, ALLOY_RECIPES, CraftingRecipe, Research, SmeltingRecipe, Stack},
};

item!(
    ELECTRIC_SMELTERY,
    "furnace",
    "Electric Smeltery",
    "red",
    None,
    None
);
item!(
    ELECTRIC_SMELTERY_2,
    "furnace",
    "Electric Smeltery - II",
    "red",
    None,
    None
);
item!(
    ELECTRIFIED_CRUCIBLE,
    "red_terracotta",
    "Electrified Crucible",
    "red",
    None,
    None
);
item!(
    ELECTRIFIED_CRUCIBLE_2,
    "red_terracotta",
    "Electrified Crucible - II",
    "red",
    None,
    None
);
item!(
    ELECTRIFIED_CRUCIBLE_3,
    "red_terracotta",
    "Electrified Crucible - III",
    "red",
    None,
    None
);
item!(REFINERY, "piston", "Refinery", "red", None, None);
item!(CRUCIBLE, "cauldron", "Crucible", "red", None, None);
item!(
    SYNTHETIC_EMERALD,
    "emerald",
    "Synthetic Emerald",
    "aqua",
    None,
    None
);

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Family {
    Smeltery,
    Crucible,
    Refinery,
    CarbonPress,
    Freezer,
    AutoDrier,
    FoodFabricator,
    FoodComposter,
    AutoBrewer,
    ChargingBench,
    AutoEnchanter,
    AutoDisenchanter,
}

pub(crate) struct Machine {
    pub item: Kind,
    pub title: &'static str,
    pub family: Family,
    pub capacity: u32,
    pub cost: u32,
    pub speed: u32,
    pub research: &'static Research,
}

const MACHINES: &[Machine] = &[
    Machine {
        item: ELECTRIC_SMELTERY,
        title: "Electric Smeltery",
        family: Family::Smeltery,
        capacity: 512,
        cost: 10,
        speed: 1,
        research: &SMELTERY,
    },
    Machine {
        item: ELECTRIC_SMELTERY_2,
        title: "Electric Smeltery - II",
        family: Family::Smeltery,
        capacity: 1024,
        cost: 20,
        speed: 3,
        research: &ADVANCED_SMELTERY,
    },
    Machine {
        item: ELECTRIFIED_CRUCIBLE,
        title: "Electrified Crucible",
        family: Family::Crucible,
        capacity: 1024,
        cost: 24,
        speed: 1,
        research: &ELECTRIC_CRUCIBLE,
    },
    Machine {
        item: ELECTRIFIED_CRUCIBLE_2,
        title: "Electrified Crucible - II",
        family: Family::Crucible,
        capacity: 1024,
        cost: 40,
        speed: 2,
        research: &BETTER_ELECTRIC_CRUCIBLES,
    },
    Machine {
        item: ELECTRIFIED_CRUCIBLE_3,
        title: "Electrified Crucible - III",
        family: Family::Crucible,
        capacity: 1024,
        cost: 60,
        speed: 4,
        research: &BETTER_ELECTRIC_CRUCIBLES,
    },
    Machine {
        item: REFINERY,
        title: "Refinery",
        family: Family::Refinery,
        capacity: 256,
        cost: 16,
        speed: 1,
        research: &FUEL,
    },
];

pub(crate) fn machine(id: &str) -> Option<&'static Machine> {
    MACHINES
        .iter()
        .chain(crate::thermal_machines::MACHINES)
        .chain(crate::food_machines::MACHINES)
        .chain(crate::enchantment_machines::MACHINES)
        .chain([
            &crate::auto_brewer::MACHINE,
            &crate::factory::charging_bench::MACHINE,
        ])
        .find(|machine| machine.item.id() == id)
}

const SMELTERY: Research = Research {
    id: "electric_smeltery",
    name: "Electric Smeltery",
    cost: 28,
};
const ADVANCED_SMELTERY: Research = Research {
    id: "advanced_electric_smeltery",
    name: "Advanced Electric Smeltery",
    cost: 28,
};
const ELECTRIC_CRUCIBLE: Research = Research {
    id: "electric_crucible",
    name: "Electrified Crucible",
    cost: 26,
};
const BETTER_ELECTRIC_CRUCIBLES: Research = Research {
    id: "better_electric_crucibles",
    name: "Hot Crucibles",
    cost: 30,
};
pub(crate) const BASIC_CRUCIBLE: Research = Research {
    id: "crucible",
    name: "Crucible",
    cost: 13,
};
pub(crate) const FUEL: Research = Research {
    id: "fuel",
    name: "Fuel",
    cost: 30,
};
pub(crate) const EMERALD_RESEARCH: Research = Research {
    id: "synthetic_emerald",
    name: "Fake Gem",
    cost: 17,
};

pub(crate) const RESEARCHES: &[Research] = &[
    SMELTERY,
    ADVANCED_SMELTERY,
    ELECTRIC_CRUCIBLE,
    BETTER_ELECTRIC_CRUCIBLES,
    BASIC_CRUCIBLE,
    FUEL,
    EMERALD_RESEARCH,
];

const fn vanilla(material: &'static str) -> Kind {
    Kind::Vanilla(material)
}

const fn stack(kind: Kind, count: u32) -> Stack {
    Stack { kind, count }
}

const fn craft(
    output: Kind,
    research: &'static Research,
    inputs: [Option<Kind>; 9],
) -> CraftingRecipe {
    CraftingRecipe {
        inputs,
        output: stack(output, 1),
        research,
    }
}

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    craft(
        ELECTRIC_SMELTERY,
        &SMELTERY,
        [
            Some(vanilla("minecraft:nether_bricks")),
            Some(ELECTRIC_MOTOR),
            Some(vanilla("minecraft:nether_bricks")),
            Some(HEATING_COIL),
            Some(ELECTRIC_INGOT_FACTORY),
            Some(HEATING_COIL),
            Some(GILDED_IRON),
            Some(ELECTRIC_MOTOR),
            Some(GILDED_IRON),
        ],
    ),
    craft(
        ELECTRIC_SMELTERY_2,
        &ADVANCED_SMELTERY,
        [
            Some(DAMASCUS_STEEL_INGOT),
            Some(POWER_CRYSTAL),
            Some(DAMASCUS_STEEL_INGOT),
            Some(HEATING_COIL),
            Some(ELECTRIC_SMELTERY),
            Some(HEATING_COIL),
            Some(GILDED_IRON),
            Some(ELECTRIC_MOTOR),
            Some(GILDED_IRON),
        ],
    ),
    craft(
        CRUCIBLE,
        &BASIC_CRUCIBLE,
        [
            Some(vanilla("minecraft:terracotta")),
            None,
            Some(vanilla("minecraft:terracotta")),
            Some(vanilla("minecraft:terracotta")),
            None,
            Some(vanilla("minecraft:terracotta")),
            Some(vanilla("minecraft:terracotta")),
            Some(vanilla("minecraft:flint_and_steel")),
            Some(vanilla("minecraft:terracotta")),
        ],
    ),
    craft(
        ELECTRIFIED_CRUCIBLE,
        &ELECTRIC_CRUCIBLE,
        [
            Some(LEAD_INGOT),
            Some(CRUCIBLE),
            Some(LEAD_INGOT),
            Some(DAMASCUS_STEEL_INGOT),
            Some(HEATING_COIL),
            Some(DAMASCUS_STEEL_INGOT),
            Some(LEAD_INGOT),
            Some(LARGE_CAPACITOR),
            Some(LEAD_INGOT),
        ],
    ),
    craft(
        ELECTRIFIED_CRUCIBLE_2,
        &BETTER_ELECTRIC_CRUCIBLES,
        [
            Some(CORINTHIAN_BRONZE_INGOT),
            Some(ELECTRIFIED_CRUCIBLE),
            Some(CORINTHIAN_BRONZE_INGOT),
            Some(DAMASCUS_STEEL_INGOT),
            Some(ELECTRIC_MOTOR),
            Some(DAMASCUS_STEEL_INGOT),
            Some(LEAD_INGOT),
            Some(HEATING_COIL),
            Some(LEAD_INGOT),
        ],
    ),
    craft(
        ELECTRIFIED_CRUCIBLE_3,
        &BETTER_ELECTRIC_CRUCIBLES,
        [
            Some(CORINTHIAN_BRONZE_INGOT),
            Some(ELECTRIFIED_CRUCIBLE_2),
            Some(CORINTHIAN_BRONZE_INGOT),
            Some(STEEL_PLATE),
            Some(POWER_CRYSTAL),
            Some(STEEL_PLATE),
            Some(LEAD_INGOT),
            Some(LEAD_INGOT),
            Some(LEAD_INGOT),
        ],
    ),
    craft(
        REFINERY,
        &FUEL,
        [
            Some(HARDENED_GLASS),
            Some(REDSTONE_ALLOY),
            Some(HARDENED_GLASS),
            Some(HARDENED_GLASS),
            Some(REDSTONE_ALLOY),
            Some(HARDENED_GLASS),
            Some(vanilla("minecraft:piston")),
            Some(ELECTRIC_MOTOR),
            Some(vanilla("minecraft:piston")),
        ],
    ),
];

pub(crate) const SYNTHETIC_EMERALD_SMELTING: SmeltingRecipe = SmeltingRecipe {
    inputs: &[
        stack(SYNTHETIC_SAPPHIRE, 1),
        stack(ALUMINUM_DUST, 1),
        stack(ALUMINUM_INGOT, 1),
        stack(vanilla("minecraft:glass_pane"), 1),
    ],
    output: stack(SYNTHETIC_EMERALD, 1),
};

// PostSetup moves single dusts into the Ingot Factory and sorts the other
// recipes by input arity, retaining SlimefunItemSetup's order on ties.
static ELECTRIC_SMELTING: LazyLock<Vec<&'static SmeltingRecipe>> = LazyLock::new(|| {
    let raw: Vec<_> = ALLOY_RECIPES
        .iter()
        .chain(recipes::smelting())
        .filter(|recipe| !crate::machine::makeshift_recipe(recipe))
        .collect();
    let mut recipes: Vec<_> = SMELTERY_REGISTRATION
        .iter()
        .filter_map(|&id| {
            raw.iter()
                .copied()
                .find(|recipe| recipe.output.kind.id() == id)
        })
        .collect();
    recipes.sort_by_key(|recipe| std::cmp::Reverse(recipe.inputs.len()));
    recipes
});

const SMELTERY_REGISTRATION: &[&str] = &[
    "REINFORCED_ALLOY_INGOT",
    "HARDENED_METAL_INGOT",
    "DAMASCUS_STEEL_INGOT",
    "STEEL_INGOT",
    "BRONZE_INGOT",
    "DURALUMIN_INGOT",
    "BILLON_INGOT",
    "BRASS_INGOT",
    "ALUMINUM_BRASS_INGOT",
    "ALUMINUM_BRONZE_INGOT",
    "CORINTHIAN_BRONZE_INGOT",
    "SOLDER_INGOT",
    "SYNTHETIC_SAPPHIRE",
    "RAW_CARBONADO",
    "NICKEL_INGOT",
    "COBALT_INGOT",
    "FERROSILICON",
    "GOLD_24K",
    "GOLD_22K",
    "GOLD_20K",
    "GOLD_18K",
    "GOLD_16K",
    "GOLD_14K",
    "GOLD_12K",
    "GOLD_10K",
    "GOLD_8K",
    "GOLD_6K",
    "SILICON",
    "GILDED_IRON",
    "SYNTHETIC_EMERALD",
    "REDSTONE_ALLOY",
    "MAGNET",
];

pub(crate) fn smeltery_recipes() -> &'static [&'static SmeltingRecipe] {
    &ELECTRIC_SMELTING
}

pub(crate) const EMPTY_BUCKET: Kind = vanilla("minecraft:bucket");
pub(crate) const WATER_BUCKET: Kind = vanilla("minecraft:water_bucket");
pub(crate) const LAVA_BUCKET: Kind = vanilla("minecraft:lava_bucket");

#[derive(Clone, Copy)]
pub(crate) struct Meltable {
    pub input: Stack,
    pub seconds: u32,
}

const fn melt(material: &'static str, count: u32, seconds: u32) -> Meltable {
    Meltable {
        input: stack(vanilla(material), count),
        seconds,
    }
}

pub(crate) const LAVA_INPUTS: &[Meltable] = &[
    melt("minecraft:cobblestone", 16, 10),
    melt("minecraft:netherrack", 16, 8),
    melt("minecraft:stone", 12, 8),
    melt("minecraft:terracotta", 12, 8),
    melt("minecraft:obsidian", 1, 10),
    melt("minecraft:white_terracotta", 12, 8),
    melt("minecraft:orange_terracotta", 12, 8),
    melt("minecraft:magenta_terracotta", 12, 8),
    melt("minecraft:light_blue_terracotta", 12, 8),
    melt("minecraft:yellow_terracotta", 12, 8),
    melt("minecraft:lime_terracotta", 12, 8),
    melt("minecraft:pink_terracotta", 12, 8),
    melt("minecraft:gray_terracotta", 12, 8),
    melt("minecraft:light_gray_terracotta", 12, 8),
    melt("minecraft:cyan_terracotta", 12, 8),
    melt("minecraft:purple_terracotta", 12, 8),
    melt("minecraft:blue_terracotta", 12, 8),
    melt("minecraft:brown_terracotta", 12, 8),
    melt("minecraft:green_terracotta", 12, 8),
    melt("minecraft:red_terracotta", 12, 8),
    melt("minecraft:black_terracotta", 12, 8),
    melt("minecraft:blackstone", 8, 10),
    melt("minecraft:basalt", 12, 10),
    melt("minecraft:cobbled_deepslate", 12, 10),
    melt("minecraft:deepslate", 10, 10),
    melt("minecraft:tuff", 8, 10),
];

pub(crate) fn can_melt_leaves(host: &dyn Host, item: &ItemStack) -> bool {
    item.count() >= 16
        && item.components().is_none()
        && item.damage_component().is_none()
        && host
            .block_key(&item.item().id(host))
            .is_some_and(|block| host.block_in_tag(block, "minecraft:leaves") == Some(true))
}

pub(crate) struct Plan {
    pub consumed: Vec<(usize, ItemStack, u32)>,
    pub outputs: Vec<ItemStack>,
    pub work_ticks: u32,
}

pub(crate) fn plan(
    host: &dyn Host,
    slots: &[Option<ItemStack>],
    ingredients: &[Stack],
    outputs: &[Stack],
    work_ticks: u32,
) -> Option<Plan> {
    let mut consumed = Vec::with_capacity(ingredients.len());
    for ingredient in ingredients {
        let (index, input) = slots.iter().enumerate().find_map(|(index, slot)| {
            let input = slot.as_ref()?;
            (input.count() >= ingredient.count
                && ingredient.kind.matches(host, input)
                && !consumed.iter().any(|(used, _, _)| *used == index))
            .then_some((index, input))
        })?;
        consumed.push((index, input.clone(), ingredient.count));
    }
    Some(Plan {
        consumed,
        outputs: outputs
            .iter()
            .map(|output| output.kind.stack(host, output.count))
            .collect::<Option<_>>()?,
        work_ticks,
    })
}

fn crucible(host: &dyn Host, inputs: &[Option<ItemStack>], speed: u32) -> Option<Plan> {
    LAVA_INPUTS
        .iter()
        .find_map(|recipe| {
            plan(
                host,
                inputs,
                &[stack(EMPTY_BUCKET, 1), recipe.input],
                &[stack(LAVA_BUCKET, 1)],
                recipe.seconds * 2 / speed,
            )
        })
        .or_else(|| {
            let (leaf_slot, leaves) = inputs.iter().enumerate().find_map(|(slot, item)| {
                let item = item.as_ref()?;
                can_melt_leaves(host, item).then_some((slot, item))
            })?;
            let (bucket_slot, bucket) = inputs.iter().enumerate().find_map(|(slot, item)| {
                let item = item.as_ref()?;
                EMPTY_BUCKET.matches(host, item).then_some((slot, item))
            })?;
            Some(Plan {
                consumed: vec![
                    (bucket_slot, bucket.clone(), 1),
                    (leaf_slot, leaves.clone(), 16),
                ],
                outputs: vec![WATER_BUCKET.stack(host, 1)?],
                work_ticks: 20 / speed,
            })
        })
}

pub(crate) fn next(
    host: &dyn Host,
    machine: &Machine,
    inputs: &[Option<ItemStack>],
    freezer_ratios: [bool; 3],
) -> Option<Plan> {
    match machine.family {
        Family::Smeltery => smeltery_recipes().iter().find_map(|recipe| {
            plan(
                host,
                inputs,
                recipe.inputs,
                &[recipe.output],
                24 / machine.speed,
            )
        }),
        Family::Crucible => crucible(host, inputs, machine.speed),
        Family::Refinery => plan(
            host,
            inputs,
            &[stack(BUCKET_OF_OIL, 1)],
            &[stack(BUCKET_OF_FUEL, 1)],
            80 / machine.speed,
        ),
        Family::FoodFabricator | Family::FoodComposter => {
            crate::food_machines::next(host, machine, inputs)
        }
        Family::AutoBrewer => crate::auto_brewer::next(host, inputs),
        Family::ChargingBench => None,
        Family::AutoEnchanter | Family::AutoDisenchanter => {
            crate::enchantment_machines::next(host, machine, inputs)
        }
        Family::CarbonPress | Family::Freezer | Family::AutoDrier => {
            use crate::thermal_machines as thermal;
            let ratios =
                machine.family == Family::Freezer && freezer_ratios[(machine.speed - 1) as usize];
            thermal::recipes(machine.family, ratios)
                .find_map(|recipe| {
                    plan(
                        host,
                        inputs,
                        recipe.inputs,
                        recipe.outputs,
                        recipe.seconds * 2 / machine.speed,
                    )
                })
                .or_else(|| {
                    (machine.family == Family::AutoDrier)
                        .then(|| thermal::dry_plant(host, inputs))
                        .flatten()
                })
        }
    }
}
