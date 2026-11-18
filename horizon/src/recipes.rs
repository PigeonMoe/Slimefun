// SPDX-License-Identifier: GPL-3.0-only

use crate::items::{
    CARBON, ENDER_LUMP_1, ENDER_LUMP_2, ENDER_LUMP_3, KELP_COOKIE, Kind, MAGIC_LUMP_1,
    MAGIC_LUMP_2, MAGIC_LUMP_3, STONE_CHUNK, WHEAT_FLOUR,
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

macro_rules! grind_recipe {
    ($input:expr, $input_count:literal, $output:expr, $output_count:literal) => {
        SingleRecipe {
            input: stack($input, $input_count),
            output: stack($output, $output_count),
        }
    };
}

// GrindStone.registerDefaultRecipes and SlimefunItemSetup, in registration order.
pub(crate) const GRINDING: &[SingleRecipe] = &[
    grind_recipe!(vanilla!("blaze_rod"), 1, vanilla!("blaze_powder"), 4),
    grind_recipe!(vanilla!("bone"), 1, vanilla!("bone_meal"), 4),
    grind_recipe!(vanilla!("bone_block"), 1, vanilla!("bone_meal"), 9),
    grind_recipe!(vanilla!("cobblestone"), 1, vanilla!("gravel"), 1),
    grind_recipe!(vanilla!("andesite"), 1, vanilla!("gravel"), 1),
    grind_recipe!(vanilla!("blackstone"), 1, vanilla!("gravel"), 1),
    grind_recipe!(vanilla!("diorite"), 1, vanilla!("gravel"), 1),
    grind_recipe!(vanilla!("granite"), 1, vanilla!("gravel"), 1),
    grind_recipe!(vanilla!("dirt"), 1, STONE_CHUNK, 1),
    grind_recipe!(vanilla!("sandstone"), 1, vanilla!("sand"), 4),
    grind_recipe!(vanilla!("red_sandstone"), 1, vanilla!("red_sand"), 4),
    grind_recipe!(vanilla!("prismarine_bricks"), 1, vanilla!("prismarine"), 2),
    grind_recipe!(vanilla!("prismarine"), 1, vanilla!("prismarine_shard"), 4),
    grind_recipe!(vanilla!("nether_wart_block"), 1, vanilla!("nether_wart"), 9),
    grind_recipe!(vanilla!("quartz_block"), 1, vanilla!("quartz"), 4),
    grind_recipe!(vanilla!("basalt"), 2, vanilla!("blackstone"), 1),
    grind_recipe!(vanilla!("amethyst_block"), 1, vanilla!("amethyst_shard"), 4),
    grind_recipe!(vanilla!("cobbled_deepslate"), 1, vanilla!("gravel"), 1),
    grind_recipe!(MAGIC_LUMP_2, 1, MAGIC_LUMP_1, 4),
    grind_recipe!(MAGIC_LUMP_3, 1, MAGIC_LUMP_2, 4),
    grind_recipe!(ENDER_LUMP_2, 1, ENDER_LUMP_1, 4),
    grind_recipe!(ENDER_LUMP_3, 1, ENDER_LUMP_2, 4),
    grind_recipe!(vanilla!("diamond"), 1, CARBON, 4),
    grind_recipe!(vanilla!("nether_wart"), 1, MAGIC_LUMP_1, 2),
    grind_recipe!(vanilla!("ender_eye"), 1, ENDER_LUMP_1, 2),
    grind_recipe!(vanilla!("wheat"), 1, WHEAT_FLOUR, 1),
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

pub(crate) const RESEARCHES: &[Research] = &[LUMPS, TASTY_KELP];

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
    lump_upgrade(MAGIC_LUMP_1, MAGIC_LUMP_2),
    lump_upgrade(MAGIC_LUMP_2, MAGIC_LUMP_3),
    lump_upgrade(ENDER_LUMP_1, ENDER_LUMP_2),
    lump_upgrade(ENDER_LUMP_2, ENDER_LUMP_3),
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
