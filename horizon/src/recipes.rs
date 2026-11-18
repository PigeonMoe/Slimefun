// SPDX-License-Identifier: GPL-3.0-only

use crate::items::{
    CARBON, CARBON_CHUNK, COMPRESSED_CARBON, COPPER_DUST, CRUSHED_ORE, ENDER_LUMP_1, ENDER_LUMP_2,
    ENDER_LUMP_3, GOLD_4K, GOLD_6K, GOLD_8K, GOLD_10K, GOLD_12K, GOLD_14K, GOLD_16K, GOLD_18K,
    GOLD_20K, GOLD_22K, GOLD_24K, GOLD_DUST, IRON_DUST, KELP_COOKIE, Kind, LEAD_INGOT,
    MAGIC_LUMP_1, MAGIC_LUMP_2, MAGIC_LUMP_3, OUTPUT_CHEST, PULVERIZED_ORE, REINFORCED_ALLOY_INGOT,
    REINFORCED_PLATE, SIFTED_ORE, STEEL_INGOT, STEEL_PLATE, STONE_CHUNK, SULFATE, WHEAT_FLOUR,
};

#[derive(Clone, Copy)]
pub(crate) struct Stack {
    pub kind: Kind,
    pub count: u32,
}

const fn stack(kind: Kind, count: u32) -> Stack {
    Stack { kind, count }
}

macro_rules! vanilla {
    ($id:literal) => {
        Kind::Vanilla(concat!("minecraft:", $id))
    };
}

#[derive(Clone, Copy)]
pub(crate) struct SingleRecipe {
    pub input: Stack,
    pub output: Stack,
}

macro_rules! single {
    ($input:expr, $input_count:literal, $output:expr, $output_count:literal) => {
        SingleRecipe {
            input: stack($input, $input_count),
            output: stack($output, $output_count),
        }
    };
}

// GrindStone.registerDefaultRecipes and SlimefunItemSetup, in registration order.
pub(crate) const GRINDING: &[SingleRecipe] = &[
    single!(vanilla!("blaze_rod"), 1, vanilla!("blaze_powder"), 4),
    single!(vanilla!("bone"), 1, vanilla!("bone_meal"), 4),
    single!(vanilla!("bone_block"), 1, vanilla!("bone_meal"), 9),
    single!(vanilla!("cobblestone"), 1, vanilla!("gravel"), 1),
    single!(vanilla!("andesite"), 1, vanilla!("gravel"), 1),
    single!(vanilla!("blackstone"), 1, vanilla!("gravel"), 1),
    single!(vanilla!("diorite"), 1, vanilla!("gravel"), 1),
    single!(vanilla!("granite"), 1, vanilla!("gravel"), 1),
    single!(vanilla!("dirt"), 1, STONE_CHUNK, 1),
    single!(vanilla!("sandstone"), 1, vanilla!("sand"), 4),
    single!(vanilla!("red_sandstone"), 1, vanilla!("red_sand"), 4),
    single!(vanilla!("prismarine_bricks"), 1, vanilla!("prismarine"), 2),
    single!(vanilla!("prismarine"), 1, vanilla!("prismarine_shard"), 4),
    single!(vanilla!("nether_wart_block"), 1, vanilla!("nether_wart"), 9),
    single!(vanilla!("quartz_block"), 1, vanilla!("quartz"), 4),
    single!(vanilla!("basalt"), 2, vanilla!("blackstone"), 1),
    single!(vanilla!("amethyst_block"), 1, vanilla!("amethyst_shard"), 4),
    single!(vanilla!("cobbled_deepslate"), 1, vanilla!("gravel"), 1),
    single!(MAGIC_LUMP_2, 1, MAGIC_LUMP_1, 4),
    single!(MAGIC_LUMP_3, 1, MAGIC_LUMP_2, 4),
    single!(ENDER_LUMP_2, 1, ENDER_LUMP_1, 4),
    single!(ENDER_LUMP_3, 1, ENDER_LUMP_2, 4),
    single!(vanilla!("diamond"), 1, CARBON, 4),
    single!(vanilla!("nether_wart"), 1, MAGIC_LUMP_1, 2),
    single!(vanilla!("ender_eye"), 1, ENDER_LUMP_1, 2),
    single!(vanilla!("wheat"), 1, WHEAT_FLOUR, 1),
];

#[derive(Clone, Copy)]
pub(crate) struct CraftingRecipe {
    pub inputs: [Option<Kind>; 9],
    pub output: Stack,
    pub research: &'static Research,
}

#[derive(Clone, Copy)]
pub(crate) struct Research {
    pub id: &'static str,
    pub name: &'static str,
    pub cost: i32,
}

pub(crate) const LUMPS: Research = Research {
    id: "lumps",
    name: "Lumps and Magic",
    cost: 3,
};

pub(crate) const TASTY_KELP: Research = Research {
    id: "kelp_cookie",
    name: "Tasty Kelp",
    cost: 4,
};

pub(crate) const RESEARCHES: &[Research] = &[
    LUMPS,
    TASTY_KELP,
    SYNTHETIC_DIAMONDS,
    POWER_ITEMS,
    ORE_PURIFICATION,
    CHEST_RESEARCH,
];

const fn lump_upgrade(input: Kind, output: Kind) -> CraftingRecipe {
    CraftingRecipe {
        inputs: [
            Some(input),
            Some(input),
            None,
            Some(input),
            Some(input),
            None,
            None,
            None,
            None,
        ],
        output: stack(output, 1),
        research: &LUMPS,
    }
}

const KELP: Option<Kind> = Some(vanilla!("dried_kelp"));

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    CraftingRecipe {
        inputs: [
            Some(LEAD_INGOT),
            Some(vanilla!("hopper")),
            Some(LEAD_INGOT),
            Some(LEAD_INGOT),
            Some(vanilla!("chest")),
            Some(LEAD_INGOT),
            None,
            Some(LEAD_INGOT),
            None,
        ],
        output: stack(OUTPUT_CHEST, 1),
        research: &CHEST_RESEARCH,
    },
    lump_upgrade(MAGIC_LUMP_1, MAGIC_LUMP_2),
    lump_upgrade(MAGIC_LUMP_2, MAGIC_LUMP_3),
    lump_upgrade(ENDER_LUMP_1, ENDER_LUMP_2),
    lump_upgrade(ENDER_LUMP_2, ENDER_LUMP_3),
    CraftingRecipe {
        inputs: [
            Some(COMPRESSED_CARBON),
            Some(COMPRESSED_CARBON),
            Some(COMPRESSED_CARBON),
            Some(COMPRESSED_CARBON),
            Some(vanilla!("flint")),
            Some(COMPRESSED_CARBON),
            Some(COMPRESSED_CARBON),
            Some(COMPRESSED_CARBON),
            Some(COMPRESSED_CARBON),
        ],
        output: stack(CARBON_CHUNK, 1),
        research: &SYNTHETIC_DIAMONDS,
    },
    CraftingRecipe {
        inputs: [
            None,
            KELP,
            None,
            KELP,
            Some(vanilla!("sugar")),
            KELP,
            None,
            KELP,
            None,
        ],
        output: stack(KELP_COOKIE, 2),
        research: &TASTY_KELP,
    },
];

// Compressor.registerDefaultRecipes followed by SlimefunItemSetup.
pub(crate) const COMPRESSING: &[SingleRecipe] = &[
    single!(STONE_CHUNK, 4, vanilla!("cobblestone"), 1),
    single!(vanilla!("flint"), 8, vanilla!("cobblestone"), 1),
    single!(vanilla!("coal_block"), 8, CARBON, 9),
    single!(vanilla!("charcoal"), 4, vanilla!("coal"), 1),
    single!(vanilla!("coal"), 8, CARBON, 1),
    single!(STEEL_INGOT, 8, STEEL_PLATE, 1),
    single!(CARBON, 4, COMPRESSED_CARBON, 1),
    single!(REINFORCED_ALLOY_INGOT, 8, REINFORCED_PLATE, 1),
];

// OreCrusher.registerDefaultRecipes, non-multiplying postRegister rows, and item setup.
pub(crate) const CRUSHING: &[SingleRecipe] = &[
    single!(vanilla!("blackstone"), 8, vanilla!("red_sand"), 1),
    single!(vanilla!("cobblestone"), 8, vanilla!("sand"), 1),
    single!(GOLD_4K, 1, GOLD_DUST, 1),
    single!(GOLD_6K, 1, GOLD_DUST, 2),
    single!(GOLD_8K, 1, GOLD_DUST, 2),
    single!(GOLD_10K, 1, GOLD_DUST, 3),
    single!(GOLD_12K, 1, GOLD_DUST, 3),
    single!(GOLD_14K, 1, GOLD_DUST, 4),
    single!(GOLD_16K, 1, GOLD_DUST, 4),
    single!(GOLD_18K, 1, GOLD_DUST, 5),
    single!(GOLD_20K, 1, GOLD_DUST, 5),
    single!(GOLD_22K, 1, GOLD_DUST, 6),
    single!(GOLD_24K, 1, GOLD_DUST, 6),
    single!(vanilla!("gravel"), 1, vanilla!("sand"), 1),
    single!(vanilla!("magma_block"), 1, SULFATE, 1),
    single!(CARBON, 1, vanilla!("coal"), 8),
    single!(COMPRESSED_CARBON, 1, CARBON, 4),
    single!(vanilla!("cobbled_deepslate"), 8, vanilla!("sand"), 1),
    single!(vanilla!("raw_iron"), 1, IRON_DUST, 1),
    single!(vanilla!("raw_copper"), 1, COPPER_DUST, 1),
    single!(vanilla!("raw_gold"), 1, GOLD_DUST, 1),
    single!(vanilla!("netherrack"), 16, SULFATE, 1),
    single!(SIFTED_ORE, 1, CRUSHED_ORE, 1),
    single!(CRUSHED_ORE, 1, PULVERIZED_ORE, 1),
];

// Base yields with Paper's double-ores setting disabled.
pub(crate) const DOUBLING: &[SingleRecipe] = &[
    single!(vanilla!("coal_ore"), 1, vanilla!("coal"), 1),
    single!(vanilla!("lapis_ore"), 1, vanilla!("lapis_lazuli"), 7),
    single!(vanilla!("redstone_ore"), 1, vanilla!("redstone"), 4),
    single!(vanilla!("diamond_ore"), 1, vanilla!("diamond"), 1),
    single!(vanilla!("emerald_ore"), 1, vanilla!("emerald"), 1),
    single!(vanilla!("nether_quartz_ore"), 1, vanilla!("quartz"), 1),
    single!(vanilla!("nether_gold_ore"), 1, vanilla!("gold_nugget"), 4),
    single!(vanilla!("gilded_blackstone"), 1, vanilla!("gold_nugget"), 4),
    single!(vanilla!("deepslate_coal_ore"), 1, vanilla!("coal"), 1),
    single!(
        vanilla!("deepslate_lapis_ore"),
        1,
        vanilla!("lapis_lazuli"),
        7
    ),
    single!(
        vanilla!("deepslate_redstone_ore"),
        1,
        vanilla!("redstone"),
        4
    ),
    single!(vanilla!("deepslate_diamond_ore"), 1, vanilla!("diamond"), 1),
    single!(vanilla!("deepslate_emerald_ore"), 1, vanilla!("emerald"), 1),
    single!(vanilla!("deepslate_iron_ore"), 1, IRON_DUST, 1),
    single!(vanilla!("deepslate_gold_ore"), 1, GOLD_DUST, 1),
    single!(vanilla!("deepslate_copper_ore"), 1, COPPER_DUST, 1),
    single!(vanilla!("copper_ore"), 1, COPPER_DUST, 1),
    single!(vanilla!("iron_ore"), 1, IRON_DUST, 1),
    single!(vanilla!("gold_ore"), 1, GOLD_DUST, 1),
];

pub(crate) const SYNTHETIC_DIAMONDS: Research = Research {
    id: "synthetic_diamond",
    name: "Synthetic Diamonds",
    cost: 10,
};

pub(crate) const POWER_ITEMS: Research = Research {
    id: "misc_power_items",
    name: "Important Power-Related Items",
    cost: 12,
};

pub(crate) const ORE_PURIFICATION: Research = Research {
    id: "crushed_ore",
    name: "Ore Purification",
    cost: 25,
};

pub(crate) fn output_research(kind: Kind) -> Option<&'static Research> {
    match kind {
        SULFATE => Some(&POWER_ITEMS),
        CRUSHED_ORE | PULVERIZED_ORE => Some(&ORE_PURIFICATION),
        _ => None,
    }
}

pub(crate) const CHEST_RESEARCH: Research = Research {
    id: "output_chest",
    name: "Basic machinery output chest",
    cost: 20,
};
