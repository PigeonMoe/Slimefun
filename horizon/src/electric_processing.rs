// SPDX-License-Identifier: GPL-3.0-only

use crate::{
    items::{HeadTexture, Kind, item},
    recipes::{
        CraftingRecipe, HIGH_TIER_ELECTRIC_INGOT_MACHINES_RESEARCH,
        MEDIUM_TIER_ELECTRIC_INGOT_MACHINES_RESEARCH, Research, SingleRecipe, Stack,
    },
};

const PRESS_TEXTURE: HeadTexture = HeadTexture {
    profile: [-1654783849, -2110506701, -1199628433, -464018164],
    value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzhkNWNmOTJiYzc5ZWMxOWY0MTA2NDQxYWZmZmYxNDA2YTEzNjcwMTBkY2FmYjE5N2RkOTRjZmNhMWE2ZGUwZmMifX19",
};

item!(
    ELECTRIC_PRESS,
    "player_head",
    "Electric Press",
    "yellow",
    None,
    Some(PRESS_TEXTURE)
);
item!(
    ELECTRIC_PRESS_2,
    "player_head",
    "Electric Press - II",
    "yellow",
    None,
    Some(PRESS_TEXTURE)
);
item!(
    ELECTRIC_GOLD_PAN,
    "brown_terracotta",
    "Electric Gold Pan",
    "gold",
    None,
    None
);
item!(
    ELECTRIC_GOLD_PAN_2,
    "brown_terracotta",
    "Electric Gold Pan (II)",
    "gold",
    None,
    None
);
item!(
    ELECTRIC_GOLD_PAN_3,
    "brown_terracotta",
    "Electric Gold Pan (III)",
    "gold",
    None,
    None
);
item!(
    ELECTRIC_DUST_WASHER,
    "blue_stained_glass",
    "Electric Dust Washer",
    "dark_aqua",
    None,
    None
);
item!(
    ELECTRIC_DUST_WASHER_2,
    "blue_stained_glass",
    "Electric Dust Washer (II)",
    "dark_aqua",
    None,
    None
);
item!(
    ELECTRIC_DUST_WASHER_3,
    "blue_stained_glass",
    "Electric Dust Washer (III)",
    "dark_aqua",
    None,
    None
);
item!(
    ELECTRIC_INGOT_FACTORY,
    "red_terracotta",
    "Electric Ingot Factory",
    "red",
    None,
    None
);
item!(
    ELECTRIC_INGOT_FACTORY_2,
    "red_terracotta",
    "Electric Ingot Factory (II)",
    "red",
    None,
    None
);
item!(
    ELECTRIC_INGOT_FACTORY_3,
    "red_terracotta",
    "Electric Ingot Factory (III)",
    "red",
    None,
    None
);

#[derive(Clone, Copy)]
pub(crate) enum Family {
    Press,
    GoldPan,
    DustWasher,
    IngotFactory,
}

pub(crate) struct MachineSpec {
    pub item: Kind,
    pub title: &'static str,
    pub family: Family,
    pub capacity: u32,
    pub cost: u32,
    pub speed: u32,
    pub tier: usize,
    pub research: &'static Research,
}

pub(crate) fn machine(id: &str) -> Option<&'static MachineSpec> {
    MACHINES.iter().find(|machine| machine.item.id() == id)
}

// Paper SlimefunItemSetup sets the item identities and tier settings.
const MACHINES: &[MachineSpec] = &[
    MachineSpec {
        item: ELECTRIC_PRESS,
        title: "Electric Press",
        family: Family::Press,
        capacity: 256,
        cost: 8,
        speed: 1,
        tier: 1,
        research: &ELECTRIC_PRESS_RESEARCH,
    },
    MachineSpec {
        item: ELECTRIC_PRESS_2,
        title: "Electric Press - II",
        family: Family::Press,
        capacity: 1024,
        cost: 20,
        speed: 3,
        tier: 2,
        research: &ELECTRIC_PRESS_RESEARCH,
    },
    MachineSpec {
        item: ELECTRIC_GOLD_PAN,
        title: "Electric Gold Pan",
        family: Family::GoldPan,
        capacity: 128,
        cost: 1,
        speed: 1,
        tier: 1,
        research: &ELECTRIC_INGOT_MACHINES_RESEARCH,
    },
    MachineSpec {
        item: ELECTRIC_GOLD_PAN_2,
        title: "Electric Gold Pan (II)",
        family: Family::GoldPan,
        capacity: 128,
        cost: 2,
        speed: 3,
        tier: 2,
        research: &MEDIUM_TIER_ELECTRIC_INGOT_MACHINES_RESEARCH,
    },
    MachineSpec {
        item: ELECTRIC_GOLD_PAN_3,
        title: "Electric Gold Pan (III)",
        family: Family::GoldPan,
        capacity: 512,
        cost: 7,
        speed: 10,
        tier: 3,
        research: &HIGH_TIER_ELECTRIC_INGOT_MACHINES_RESEARCH,
    },
    MachineSpec {
        item: ELECTRIC_DUST_WASHER,
        title: "Electric Dust Washer",
        family: Family::DustWasher,
        capacity: 128,
        cost: 3,
        speed: 1,
        tier: 1,
        research: &ELECTRIC_INGOT_MACHINES_RESEARCH,
    },
    MachineSpec {
        item: ELECTRIC_DUST_WASHER_2,
        title: "Electric Dust Washer (II)",
        family: Family::DustWasher,
        capacity: 128,
        cost: 5,
        speed: 2,
        tier: 2,
        research: &MEDIUM_TIER_ELECTRIC_INGOT_MACHINES_RESEARCH,
    },
    MachineSpec {
        item: ELECTRIC_DUST_WASHER_3,
        title: "Electric Dust Washer (III)",
        family: Family::DustWasher,
        capacity: 512,
        cost: 15,
        speed: 10,
        tier: 3,
        research: &HIGH_TIER_ELECTRIC_INGOT_MACHINES_RESEARCH,
    },
    MachineSpec {
        item: ELECTRIC_INGOT_FACTORY,
        title: "Electric Ingot Factory",
        family: Family::IngotFactory,
        capacity: 256,
        cost: 4,
        speed: 1,
        tier: 1,
        research: &ELECTRIC_INGOT_MACHINES_RESEARCH,
    },
    MachineSpec {
        item: ELECTRIC_INGOT_FACTORY_2,
        title: "Electric Ingot Factory (II)",
        family: Family::IngotFactory,
        capacity: 256,
        cost: 7,
        speed: 2,
        tier: 2,
        research: &MEDIUM_TIER_ELECTRIC_INGOT_MACHINES_RESEARCH,
    },
    MachineSpec {
        item: ELECTRIC_INGOT_FACTORY_3,
        title: "Electric Ingot Factory (III)",
        family: Family::IngotFactory,
        capacity: 512,
        cost: 20,
        speed: 8,
        tier: 3,
        research: &HIGH_TIER_ELECTRIC_INGOT_MACHINES_RESEARCH,
    },
];

pub(crate) const ELECTRIC_PRESS_RESEARCH: Research = Research {
    id: "electric_press",
    name: "Electric Press",
    cost: 16,
};
pub(crate) const ELECTRIC_INGOT_MACHINES_RESEARCH: Research = Research {
    id: "electric_ingot_machines",
    name: "Electric Ingot Fabrication",
    cost: 18,
};
pub(crate) const RESEARCHES: &[Research] =
    &[ELECTRIC_PRESS_RESEARCH, ELECTRIC_INGOT_MACHINES_RESEARCH];

const fn stack(kind: Kind, count: u32) -> Stack {
    Stack { kind, count }
}

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    CraftingRecipe {
        inputs: [
            Some(Kind::Vanilla("minecraft:piston")),
            Some(crate::items::ELECTRIC_MOTOR),
            Some(Kind::Vanilla("minecraft:piston")),
            None,
            Some(crate::network_recipes::MEDIUM_CAPACITOR),
            None,
            Some(crate::items::DAMASCUS_STEEL_INGOT),
            Some(crate::items::DAMASCUS_STEEL_INGOT),
            Some(crate::items::DAMASCUS_STEEL_INGOT),
        ],
        output: stack(ELECTRIC_PRESS, 1),
        research: &ELECTRIC_PRESS_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            Some(Kind::Vanilla("minecraft:sticky_piston")),
            Some(ELECTRIC_PRESS),
            Some(Kind::Vanilla("minecraft:sticky_piston")),
            Some(crate::items::DAMASCUS_STEEL_INGOT),
            Some(crate::network_recipes::BIG_CAPACITOR),
            Some(crate::items::DAMASCUS_STEEL_INGOT),
            Some(crate::items::DAMASCUS_STEEL_INGOT),
            Some(crate::items::DAMASCUS_STEEL_INGOT),
            Some(crate::items::DAMASCUS_STEEL_INGOT),
        ],
        output: stack(ELECTRIC_PRESS_2, 1),
        research: &ELECTRIC_PRESS_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            None,
            Some(crate::items::GOLD_PAN),
            None,
            Some(Kind::Vanilla("minecraft:flint")),
            Some(crate::items::ELECTRIC_MOTOR),
            Some(Kind::Vanilla("minecraft:flint")),
            Some(crate::items::ALUMINUM_INGOT),
            Some(crate::items::ALUMINUM_INGOT),
            Some(crate::items::ALUMINUM_INGOT),
        ],
        output: stack(ELECTRIC_GOLD_PAN, 1),
        research: &ELECTRIC_INGOT_MACHINES_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            None,
            Some(crate::items::GOLD_PAN),
            None,
            Some(Kind::Vanilla("minecraft:iron_ingot")),
            Some(ELECTRIC_GOLD_PAN),
            Some(Kind::Vanilla("minecraft:iron_ingot")),
            Some(crate::items::DURALUMIN_INGOT),
            Some(crate::items::DURALUMIN_INGOT),
            Some(crate::items::DURALUMIN_INGOT),
        ],
        output: stack(ELECTRIC_GOLD_PAN_2, 1),
        research: &MEDIUM_TIER_ELECTRIC_INGOT_MACHINES_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            None,
            Some(crate::items::GOLD_PAN),
            None,
            Some(crate::items::ELECTRIC_MOTOR),
            Some(ELECTRIC_GOLD_PAN_2),
            Some(crate::items::ELECTRIC_MOTOR),
            Some(crate::items::COBALT_INGOT),
            Some(crate::network_recipes::BLISTERING_INGOT_3),
            Some(crate::items::COBALT_INGOT),
        ],
        output: stack(ELECTRIC_GOLD_PAN_3, 1),
        research: &HIGH_TIER_ELECTRIC_INGOT_MACHINES_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            None,
            Some(Kind::Vanilla("minecraft:water_bucket")),
            None,
            Some(crate::items::ELECTRO_MAGNET),
            Some(ELECTRIC_GOLD_PAN),
            Some(crate::items::ELECTRO_MAGNET),
            Some(crate::items::COPPER_INGOT),
            Some(crate::items::COPPER_INGOT),
            Some(crate::items::COPPER_INGOT),
        ],
        output: stack(ELECTRIC_DUST_WASHER, 1),
        research: &ELECTRIC_INGOT_MACHINES_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            None,
            Some(Kind::Vanilla("minecraft:water_bucket")),
            None,
            Some(crate::items::ELECTRO_MAGNET),
            Some(ELECTRIC_DUST_WASHER),
            Some(crate::items::ELECTRO_MAGNET),
            Some(crate::items::DAMASCUS_STEEL_INGOT),
            Some(crate::items::DAMASCUS_STEEL_INGOT),
            Some(crate::items::DAMASCUS_STEEL_INGOT),
        ],
        output: stack(ELECTRIC_DUST_WASHER_2, 1),
        research: &MEDIUM_TIER_ELECTRIC_INGOT_MACHINES_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            None,
            Some(Kind::Vanilla("minecraft:water_bucket")),
            None,
            Some(crate::items::ELECTRO_MAGNET),
            Some(ELECTRIC_DUST_WASHER_2),
            Some(crate::items::ELECTRO_MAGNET),
            Some(crate::items::CORINTHIAN_BRONZE_INGOT),
            Some(crate::network_recipes::BLISTERING_INGOT_3),
            Some(crate::items::CORINTHIAN_BRONZE_INGOT),
        ],
        output: stack(ELECTRIC_DUST_WASHER_3, 1),
        research: &HIGH_TIER_ELECTRIC_INGOT_MACHINES_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            None,
            Some(Kind::Vanilla("minecraft:flint_and_steel")),
            None,
            Some(crate::items::HEATING_COIL),
            Some(ELECTRIC_DUST_WASHER),
            Some(crate::items::HEATING_COIL),
            Some(crate::items::DAMASCUS_STEEL_INGOT),
            Some(crate::items::ELECTRIC_MOTOR),
            Some(crate::items::DAMASCUS_STEEL_INGOT),
        ],
        output: stack(ELECTRIC_INGOT_FACTORY, 1),
        research: &ELECTRIC_INGOT_MACHINES_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            Some(crate::items::GILDED_IRON),
            Some(Kind::Vanilla("minecraft:flint_and_steel")),
            Some(crate::items::GILDED_IRON),
            Some(crate::items::HEATING_COIL),
            Some(ELECTRIC_INGOT_FACTORY),
            Some(crate::items::HEATING_COIL),
            Some(crate::items::BRASS_INGOT),
            Some(crate::items::ELECTRIC_MOTOR),
            Some(crate::items::BRASS_INGOT),
        ],
        output: stack(ELECTRIC_INGOT_FACTORY_2, 1),
        research: &MEDIUM_TIER_ELECTRIC_INGOT_MACHINES_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            Some(crate::items::GILDED_IRON),
            Some(Kind::Vanilla("minecraft:flint_and_steel")),
            Some(crate::items::GILDED_IRON),
            Some(crate::items::HEATING_COIL),
            Some(ELECTRIC_INGOT_FACTORY_2),
            Some(crate::items::HEATING_COIL),
            Some(crate::items::BRASS_INGOT),
            Some(crate::network_recipes::BLISTERING_INGOT_3),
            Some(crate::items::BRASS_INGOT),
        ],
        output: stack(ELECTRIC_INGOT_FACTORY_3, 1),
        research: &HIGH_TIER_ELECTRIC_INGOT_MACHINES_RESEARCH,
    },
];

pub(crate) struct PressRecipe {
    pub seconds: u32,
    pub recipe: SingleRecipe,
}

const fn press(seconds: u32, input: Stack, output: Stack) -> PressRecipe {
    PressRecipe {
        seconds,
        recipe: SingleRecipe { input, output },
    }
}

// Registration order from ElectricPress.registerDefaultRecipes.
pub(crate) const PRESSING: &[PressRecipe] = &[
    press(
        4,
        stack(crate::items::STONE_CHUNK, 3),
        stack(Kind::Vanilla("minecraft:cobblestone"), 1),
    ),
    press(
        4,
        stack(Kind::Vanilla("minecraft:flint"), 6),
        stack(Kind::Vanilla("minecraft:cobblestone"), 1),
    ),
    press(
        5,
        stack(Kind::Vanilla("minecraft:glass"), 1),
        stack(Kind::Vanilla("minecraft:glass_pane"), 3),
    ),
    press(
        4,
        stack(Kind::Vanilla("minecraft:snowball"), 4),
        stack(Kind::Vanilla("minecraft:snow_block"), 1),
    ),
    press(
        4,
        stack(Kind::Vanilla("minecraft:magma_cream"), 4),
        stack(Kind::Vanilla("minecraft:magma_block"), 1),
    ),
    press(
        4,
        stack(Kind::Vanilla("minecraft:slime_ball"), 9),
        stack(Kind::Vanilla("minecraft:slime_block"), 1),
    ),
    press(
        3,
        stack(Kind::Vanilla("minecraft:dried_kelp"), 9),
        stack(Kind::Vanilla("minecraft:dried_kelp_block"), 1),
    ),
    press(
        3,
        stack(Kind::Vanilla("minecraft:bone_meal"), 9),
        stack(Kind::Vanilla("minecraft:bone_block"), 1),
    ),
    press(
        3,
        stack(Kind::Vanilla("minecraft:clay_ball"), 4),
        stack(Kind::Vanilla("minecraft:clay"), 1),
    ),
    press(
        3,
        stack(Kind::Vanilla("minecraft:brick"), 4),
        stack(Kind::Vanilla("minecraft:bricks"), 1),
    ),
    press(
        6,
        stack(crate::items::COPPER_INGOT, 1),
        stack(crate::items::COPPER_WIRE, 3),
    ),
    press(
        16,
        stack(crate::items::STEEL_INGOT, 8),
        stack(crate::items::STEEL_PLATE, 1),
    ),
    press(
        18,
        stack(crate::items::REINFORCED_ALLOY_INGOT, 8),
        stack(crate::items::REINFORCED_PLATE, 1),
    ),
    press(
        8,
        stack(Kind::Vanilla("minecraft:nether_wart"), 1),
        stack(crate::items::MAGIC_LUMP_1, 2),
    ),
    press(
        10,
        stack(crate::items::MAGIC_LUMP_1, 4),
        stack(crate::items::MAGIC_LUMP_2, 1),
    ),
    press(
        12,
        stack(crate::items::MAGIC_LUMP_2, 4),
        stack(crate::items::MAGIC_LUMP_3, 1),
    ),
    press(
        10,
        stack(Kind::Vanilla("minecraft:ender_eye"), 1),
        stack(crate::items::ENDER_LUMP_1, 2),
    ),
    press(
        12,
        stack(crate::items::ENDER_LUMP_1, 4),
        stack(crate::items::ENDER_LUMP_2, 1),
    ),
    press(
        14,
        stack(crate::items::ENDER_LUMP_2, 4),
        stack(crate::items::ENDER_LUMP_3, 1),
    ),
    press(
        18,
        stack(crate::heated_pressure::TINY_URANIUM, 9),
        stack(crate::heated_pressure::SMALL_URANIUM, 1),
    ),
    press(
        24,
        stack(crate::heated_pressure::SMALL_URANIUM, 4),
        stack(crate::heated_pressure::URANIUM, 1),
    ),
    press(
        4,
        stack(Kind::Vanilla("minecraft:quartz"), 4),
        stack(Kind::Vanilla("minecraft:quartz_block"), 1),
    ),
    press(
        4,
        stack(Kind::Vanilla("minecraft:iron_nugget"), 9),
        stack(Kind::Vanilla("minecraft:iron_ingot"), 1),
    ),
    press(
        4,
        stack(Kind::Vanilla("minecraft:gold_nugget"), 9),
        stack(Kind::Vanilla("minecraft:gold_ingot"), 1),
    ),
    press(
        4,
        stack(Kind::Vanilla("minecraft:coal"), 9),
        stack(Kind::Vanilla("minecraft:coal_block"), 1),
    ),
    press(
        4,
        stack(Kind::Vanilla("minecraft:sand"), 4),
        stack(Kind::Vanilla("minecraft:sandstone"), 1),
    ),
    press(
        4,
        stack(Kind::Vanilla("minecraft:red_sand"), 4),
        stack(Kind::Vanilla("minecraft:red_sandstone"), 1),
    ),
    press(
        5,
        stack(Kind::Vanilla("minecraft:iron_ingot"), 9),
        stack(Kind::Vanilla("minecraft:iron_block"), 1),
    ),
    press(
        5,
        stack(Kind::Vanilla("minecraft:gold_ingot"), 9),
        stack(Kind::Vanilla("minecraft:gold_block"), 1),
    ),
    press(
        6,
        stack(Kind::Vanilla("minecraft:redstone"), 9),
        stack(Kind::Vanilla("minecraft:redstone_block"), 1),
    ),
    press(
        6,
        stack(Kind::Vanilla("minecraft:lapis_lazuli"), 9),
        stack(Kind::Vanilla("minecraft:lapis_block"), 1),
    ),
    press(
        8,
        stack(Kind::Vanilla("minecraft:emerald"), 9),
        stack(Kind::Vanilla("minecraft:emerald_block"), 1),
    ),
    press(
        8,
        stack(Kind::Vanilla("minecraft:diamond"), 9),
        stack(Kind::Vanilla("minecraft:diamond_block"), 1),
    ),
    press(
        16,
        stack(Kind::Vanilla("minecraft:netherite_ingot"), 9),
        stack(Kind::Vanilla("minecraft:netherite_block"), 1),
    ),
    press(
        4,
        stack(Kind::Vanilla("minecraft:amethyst_shard"), 4),
        stack(Kind::Vanilla("minecraft:amethyst_block"), 1),
    ),
    press(
        5,
        stack(Kind::Vanilla("minecraft:copper_ingot"), 9),
        stack(Kind::Vanilla("minecraft:copper_block"), 1),
    ),
    press(
        5,
        stack(Kind::Vanilla("minecraft:raw_iron"), 9),
        stack(Kind::Vanilla("minecraft:raw_iron_block"), 1),
    ),
    press(
        5,
        stack(Kind::Vanilla("minecraft:raw_gold"), 9),
        stack(Kind::Vanilla("minecraft:raw_gold_block"), 1),
    ),
    press(
        5,
        stack(Kind::Vanilla("minecraft:raw_copper"), 9),
        stack(Kind::Vanilla("minecraft:raw_copper_block"), 1),
    ),
];
