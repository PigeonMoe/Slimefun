// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{ActiveEffect, Compound, LevelRef, LivingRef, ServerHost, Tag};

use crate::{
    items::{
        DAMASCUS_STEEL_INGOT, ENDER_LUMP_1, ENDER_LUMP_3, GILDED_IRON, GOLD_8K, GOLD_12K, Kind,
        LEAD_INGOT, MAGIC_LUMP_2, MAGIC_LUMP_3, REINFORCED_ALLOY_INGOT, STEEL_INGOT, STEEL_PLATE,
        item, text,
    },
    recipes::{CraftingRecipe, Research, Stack},
    research,
};

macro_rules! vanilla {
    ($id:literal) => {
        Kind::Vanilla(concat!("minecraft:", $id))
    };
}

macro_rules! armor_items {
    ($($id:ident, $material:literal, $label:literal, $color:literal;)*) => {
        $(item!($id, $material, $label, $color, None, None);)*
    };
}

// These constants use Paper's actual on-disk item IDs.
armor_items! {
    ARMOR_FORGE, "anvil", "Armor Forge", "gold";
    CHAIN, "string", "Chain", "aqua";
    CLOTH, "paper", "Cloth", "aqua";
    REINFORCED_CLOTH, "paper", "Reinforced Cloth", "aqua";
    ELYTRA_SCALE, "feather", "Elytra Scale", "aqua";
    RAINBOW_LEATHER, "rabbit_hide", "Rainbow Leather", "light_purple";
    ANCIENT_RUNE_ENDER, "firework_star", "Ancient Rune [Ender]", "gray";
    ANCIENT_RUNE_RAINBOW, "firework_star", "Ancient Rune [Rainbow]", "gray";
    ENDER_HELMET, "leather_helmet", "Ender Helmet", "dark_purple";
    ENDER_CHESTPLATE, "leather_chestplate", "Ender Chestplate", "dark_purple";
    ENDER_LEGGINGS, "leather_leggings", "Ender Leggings", "dark_purple";
    ENDER_BOOTS, "leather_boots", "Ender Boots", "dark_purple";
    SLIME_HELMET, "leather_helmet", "Slime Helmet", "green";
    SLIME_CHESTPLATE, "leather_chestplate", "Slime Chestplate", "green";
    SLIME_LEGGINGS, "leather_leggings", "Slime Leggings", "green";
    SLIME_BOOTS, "leather_boots", "Slime Boots", "green";
    GLOWSTONE_HELMET, "leather_helmet", "Glowstone Helmet", "yellow";
    GLOWSTONE_CHESTPLATE, "leather_chestplate", "Glowstone Chestplate", "yellow";
    GLOWSTONE_LEGGINGS, "leather_leggings", "Glowstone Leggings", "yellow";
    GLOWSTONE_BOOTS, "leather_boots", "Glowstone Boots", "yellow";
    RAINBOW_HELMET, "leather_helmet", "Rainbow Helmet", "light_purple";
    RAINBOW_CHESTPLATE, "leather_chestplate", "Rainbow Chestplate", "light_purple";
    RAINBOW_LEGGINGS, "leather_leggings", "Rainbow Leggings", "light_purple";
    RAINBOW_BOOTS, "leather_boots", "Rainbow Boots", "light_purple";
    DAMASCUS_STEEL_HELMET, "iron_helmet", "Damascus Steel Helmet", "gray";
    DAMASCUS_STEEL_CHESTPLATE, "iron_chestplate", "Damascus Steel Chestplate", "gray";
    DAMASCUS_STEEL_LEGGINGS, "iron_leggings", "Damascus Steel Leggings", "gray";
    DAMASCUS_STEEL_BOOTS, "iron_boots", "Damascus Steel Boots", "gray";
    REINFORCED_ALLOY_HELMET, "iron_helmet", "Reinforced Helmet", "aqua";
    REINFORCED_ALLOY_CHESTPLATE, "iron_chestplate", "Reinforced Chestplate", "aqua";
    REINFORCED_ALLOY_LEGGINGS, "iron_leggings", "Reinforced Leggings", "aqua";
    REINFORCED_ALLOY_BOOTS, "iron_boots", "Reinforced Boots", "aqua";
    CACTUS_HELMET, "leather_helmet", "Cactus Helmet", "dark_green";
    CACTUS_CHESTPLATE, "leather_chestplate", "Cactus Chestplate", "dark_green";
    CACTUS_LEGGINGS, "leather_leggings", "Cactus Leggings", "dark_green";
    CACTUS_BOOTS, "leather_boots", "Cactus Boots", "dark_green";
    GILDED_IRON_HELMET, "golden_helmet", "Gilded Iron Helmet", "gold";
    GILDED_IRON_CHESTPLATE, "golden_chestplate", "Gilded Iron Chestplate", "gold";
    GILDED_IRON_LEGGINGS, "golden_leggings", "Gilded Iron Leggings", "gold";
    GILDED_IRON_BOOTS, "golden_boots", "Gilded Iron Boots", "gold";
    SCUBA_HELMET, "leather_helmet", "Scuba Helmet", "red";
    HAZMAT_CHESTPLATE, "leather_chestplate", "Hazmat Suit", "red";
    HAZMAT_LEGGINGS, "leather_leggings", "Hazmat Suit Leggings", "red";
    RUBBER_BOOTS, "leather_boots", "Hazmat Boots", "red";
    GOLD_12K_HELMET, "golden_helmet", "Golden Helmet (12-Carat)", "gold";
    GOLD_12K_CHESTPLATE, "golden_chestplate", "Golden Chestplate (12-Carat)", "gold";
    GOLD_12K_LEGGINGS, "golden_leggings", "Golden Leggings (12-Carat)", "gold";
    GOLD_12K_BOOTS, "golden_boots", "Golden Boots (12-Carat)", "gold";
    NIGHT_VISION_GOGGLES, "leather_helmet", "Night Vision Goggles", "green";
    SLIME_STEEL_HELMET, "iron_helmet", "Slime Helmet", "green";
    SLIME_STEEL_CHESTPLATE, "iron_chestplate", "Slime Chestplate", "green";
    SLIME_STEEL_LEGGINGS, "iron_leggings", "Slime Leggings", "green";
    SLIME_STEEL_BOOTS, "iron_boots", "Slime Boots", "green";
    FARMER_SHOES, "leather_boots", "Farmer Shoes", "yellow";
    BOOTS_OF_THE_STOMPER, "leather_boots", "Boots of the Stomper", "aqua";
    BEE_HELMET, "golden_helmet", "Bee Helmet", "yellow";
    BEE_WINGS, "elytra", "Bee Wings", "yellow";
    BEE_LEGGINGS, "golden_leggings", "Bee Leggings", "yellow";
    BEE_BOOTS, "golden_boots", "Bee Boots", "yellow";
    ELYTRA_CAP, "leather_helmet", "Elytra Cap", "dark_purple";
}

macro_rules! research {
    ($name:ident, $id:literal, $title:literal, $cost:expr) => {
        const $name: Research = Research {
            id: $id,
            name: $title,
            cost: $cost,
        };
    };
}

pub(crate) const FORGE_RESEARCH: Research = Research {
    id: "armor_forge",
    name: "Armor Crafting",
    cost: 2,
};
research!(ENDER_ARMOR, "ender_armor", "Ender Armor", 4);
research!(SLIME_ARMOR, "slime_armor", "Slime Armor", 5);
research!(GLOWSTONE_ARMOR, "glowstone_armor", "Glowstone Armor", 3);
research!(
    RAINBOW_ARMOR,
    "rainbow_armor",
    "I wanna see the rainbow high in the sky",
    22
);
research!(
    DAMASCUS_STEEL_ARMOR,
    "damascus_steel_armor",
    "Damascus Steel Armor",
    18
);
research!(REINFORCED_ARMOR, "reinforced_armor", "Reinforced Armor", 26);
research!(CACTUS_ARMOR, "cactus_armor", "Cactus Suit", 5);
research!(CHAINMAIL_ARMOR, "chainmail_armor", "Chainmail Armor", 8);
research!(
    GILDED_IRON_ARMOR,
    "gilded_iron_armor",
    "Gilded Iron Armor",
    16
);
research!(HAZMAT_SUIT, "hazmat_suit", "Hazmat Suit", 21);
research!(GOLD_ARMOR, "gold_armor", "Shiny Armor", 13);
research!(
    NIGHT_VISION_GOOGLES,
    "night_vision_googles",
    "Night Vision Goggles",
    10
);
research!(
    SLIME_STEEL_ARMOR,
    "slime_steel_armor",
    "Slimy Steel Armor",
    27
);
research!(FARMER_SHOES_RESEARCH, "farmer_shoes", "Farmer Shoes", 4);
research!(
    STOMPER_RESEARCH,
    "boots_of_the_stomper",
    "Boots of the Stomper",
    19
);
pub(crate) const BEE_ARMOR: Research = Research {
    id: "bee_armor",
    name: "Bee Armor",
    cost: 24,
};
research!(ELYTRA_CAP_RESEARCH, "elytra_cap", "Crash Gear", 20);
research!(LEAD_CLOTHING, "lead_clothing", "Lead Clothing", 14);
research!(SPECIAL_RUNES, "special_runes", "Purple Runes", 18);
research!(ELYTRA, "elytra", "Elytras", 20);

const UNLOCKED: Research = Research {
    id: "",
    name: "",
    cost: 0,
};

pub(crate) const RESEARCHES: &[&Research] = &[
    &FORGE_RESEARCH,
    &ENDER_ARMOR,
    &SLIME_ARMOR,
    &GLOWSTONE_ARMOR,
    &RAINBOW_ARMOR,
    &DAMASCUS_STEEL_ARMOR,
    &REINFORCED_ARMOR,
    &CACTUS_ARMOR,
    &CHAINMAIL_ARMOR,
    &GILDED_IRON_ARMOR,
    &HAZMAT_SUIT,
    &GOLD_ARMOR,
    &NIGHT_VISION_GOOGLES,
    &SLIME_STEEL_ARMOR,
    &FARMER_SHOES_RESEARCH,
    &STOMPER_RESEARCH,
    &BEE_ARMOR,
    &ELYTRA_CAP_RESEARCH,
    &LEAD_CLOTHING,
    &SPECIAL_RUNES,
    &ELYTRA,
];

const fn craft(
    inputs: [Option<Kind>; 9],
    kind: Kind,
    count: u32,
    research: &'static Research,
) -> CraftingRecipe {
    CraftingRecipe {
        inputs,
        output: Stack { kind, count },
        research,
    }
}

#[derive(Clone, Copy)]
enum Shape {
    Helmet,
    Chestplate,
    Leggings,
    Boots,
}

const fn shaped(
    kind: Kind,
    raw: Kind,
    shape: Shape,
    research: &'static Research,
) -> CraftingRecipe {
    let m = Some(raw);
    let inputs = match shape {
        Shape::Helmet => [m, m, m, m, None, m, None, None, None],
        Shape::Chestplate => [m, None, m, m, m, m, m, m, m],
        Shape::Leggings => [m, m, m, m, None, m, m, None, m],
        Shape::Boots => [None, None, None, m, None, m, m, None, m],
    };
    craft(inputs, kind, 1, research)
}

// SlimefunItemSetup order includes the registrations expanded by registerArmorSet.
#[rustfmt::skip]
pub(crate) const RECIPES: &[CraftingRecipe] = &[
    craft(
        [
            Some(ENDER_LUMP_1), Some(vanilla!("ender_eye")), Some(ENDER_LUMP_1),
            Some(vanilla!("obsidian")), None, Some(vanilla!("obsidian")),
            None, None, None,
        ],
        ENDER_HELMET, 1, &ENDER_ARMOR,
    ),
    craft(
        [
            Some(ENDER_LUMP_1), None, Some(ENDER_LUMP_1),
            Some(vanilla!("obsidian")), Some(vanilla!("ender_eye")), Some(vanilla!("obsidian")),
            Some(vanilla!("obsidian")), Some(vanilla!("obsidian")), Some(vanilla!("obsidian")),
        ],
        ENDER_CHESTPLATE, 1, &ENDER_ARMOR,
    ),
    craft(
        [
            Some(ENDER_LUMP_1), Some(vanilla!("ender_eye")), Some(ENDER_LUMP_1),
            Some(vanilla!("obsidian")), None, Some(vanilla!("obsidian")),
            Some(vanilla!("obsidian")), None, Some(vanilla!("obsidian")),
        ],
        ENDER_LEGGINGS, 1, &ENDER_ARMOR,
    ),
    craft(
        [
            None, None, None,
            Some(ENDER_LUMP_1), None, Some(ENDER_LUMP_1),
            Some(vanilla!("obsidian")), None, Some(vanilla!("obsidian")),
        ],
        ENDER_BOOTS, 1, &ENDER_ARMOR,
    ),
    craft(
        [
            Some(vanilla!("slime_ball")), Some(vanilla!("iron_ingot")), Some(vanilla!("slime_ball")),
            Some(vanilla!("iron_ingot")), None, Some(vanilla!("iron_ingot")),
            None, None, None,
        ],
        SLIME_HELMET, 1, &SLIME_ARMOR,
    ),
    craft(
        [
            Some(vanilla!("slime_ball")), None, Some(vanilla!("slime_ball")),
            Some(vanilla!("iron_ingot")), Some(vanilla!("slime_ball")), Some(vanilla!("iron_ingot")),
            Some(vanilla!("iron_ingot")), Some(vanilla!("slime_ball")), Some(vanilla!("iron_ingot")),
        ],
        SLIME_CHESTPLATE, 1, &SLIME_ARMOR,
    ),
    craft(
        [
            Some(vanilla!("slime_ball")), Some(vanilla!("iron_ingot")), Some(vanilla!("slime_ball")),
            Some(vanilla!("iron_ingot")), None, Some(vanilla!("iron_ingot")),
            Some(vanilla!("iron_ingot")), None, Some(vanilla!("iron_ingot")),
        ],
        SLIME_LEGGINGS, 1, &SLIME_ARMOR,
    ),
    craft(
        [
            None, None, None,
            Some(vanilla!("slime_ball")), None, Some(vanilla!("slime_ball")),
            Some(vanilla!("iron_ingot")), None, Some(vanilla!("iron_ingot")),
        ],
        SLIME_BOOTS, 1, &SLIME_ARMOR,
    ),
    shaped(GLOWSTONE_HELMET, vanilla!("glowstone"), Shape::Helmet, &GLOWSTONE_ARMOR),
    shaped(GLOWSTONE_CHESTPLATE, vanilla!("glowstone"), Shape::Chestplate, &GLOWSTONE_ARMOR),
    shaped(GLOWSTONE_LEGGINGS, vanilla!("glowstone"), Shape::Leggings, &GLOWSTONE_ARMOR),
    shaped(GLOWSTONE_BOOTS, vanilla!("glowstone"), Shape::Boots, &GLOWSTONE_ARMOR),
    craft(
        [
            Some(RAINBOW_LEATHER), Some(RAINBOW_LEATHER), Some(RAINBOW_LEATHER),
            Some(RAINBOW_LEATHER), None, Some(RAINBOW_LEATHER),
            None, None, None,
        ],
        RAINBOW_HELMET, 1, &RAINBOW_ARMOR,
    ),
    craft(
        [
            Some(RAINBOW_LEATHER), None, Some(RAINBOW_LEATHER),
            Some(RAINBOW_LEATHER), Some(RAINBOW_LEATHER), Some(RAINBOW_LEATHER),
            Some(RAINBOW_LEATHER), Some(RAINBOW_LEATHER), Some(RAINBOW_LEATHER),
        ],
        RAINBOW_CHESTPLATE, 1, &RAINBOW_ARMOR,
    ),
    craft(
        [
            Some(RAINBOW_LEATHER), Some(RAINBOW_LEATHER), Some(RAINBOW_LEATHER),
            Some(RAINBOW_LEATHER), None, Some(RAINBOW_LEATHER),
            Some(RAINBOW_LEATHER), None, Some(RAINBOW_LEATHER),
        ],
        RAINBOW_LEGGINGS, 1, &RAINBOW_ARMOR,
    ),
    craft(
        [
            None, None, None,
            Some(RAINBOW_LEATHER), None, Some(RAINBOW_LEATHER),
            Some(RAINBOW_LEATHER), None, Some(RAINBOW_LEATHER),
        ],
        RAINBOW_BOOTS, 1, &RAINBOW_ARMOR,
    ),
    shaped(DAMASCUS_STEEL_HELMET, DAMASCUS_STEEL_INGOT, Shape::Helmet, &DAMASCUS_STEEL_ARMOR),
    shaped(DAMASCUS_STEEL_CHESTPLATE, DAMASCUS_STEEL_INGOT, Shape::Chestplate, &DAMASCUS_STEEL_ARMOR),
    shaped(DAMASCUS_STEEL_LEGGINGS, DAMASCUS_STEEL_INGOT, Shape::Leggings, &DAMASCUS_STEEL_ARMOR),
    shaped(DAMASCUS_STEEL_BOOTS, DAMASCUS_STEEL_INGOT, Shape::Boots, &DAMASCUS_STEEL_ARMOR),
    shaped(REINFORCED_ALLOY_HELMET, REINFORCED_ALLOY_INGOT, Shape::Helmet, &REINFORCED_ARMOR),
    shaped(REINFORCED_ALLOY_CHESTPLATE, REINFORCED_ALLOY_INGOT, Shape::Chestplate, &REINFORCED_ARMOR),
    shaped(REINFORCED_ALLOY_LEGGINGS, REINFORCED_ALLOY_INGOT, Shape::Leggings, &REINFORCED_ARMOR),
    shaped(REINFORCED_ALLOY_BOOTS, REINFORCED_ALLOY_INGOT, Shape::Boots, &REINFORCED_ARMOR),
    shaped(CACTUS_HELMET, vanilla!("cactus"), Shape::Helmet, &CACTUS_ARMOR),
    shaped(CACTUS_CHESTPLATE, vanilla!("cactus"), Shape::Chestplate, &CACTUS_ARMOR),
    shaped(CACTUS_LEGGINGS, vanilla!("cactus"), Shape::Leggings, &CACTUS_ARMOR),
    shaped(CACTUS_BOOTS, vanilla!("cactus"), Shape::Boots, &CACTUS_ARMOR),
    shaped(vanilla!("chainmail_helmet"), CHAIN, Shape::Helmet, &CHAINMAIL_ARMOR),
    shaped(vanilla!("chainmail_chestplate"), CHAIN, Shape::Chestplate, &CHAINMAIL_ARMOR),
    shaped(vanilla!("chainmail_leggings"), CHAIN, Shape::Leggings, &CHAINMAIL_ARMOR),
    shaped(vanilla!("chainmail_boots"), CHAIN, Shape::Boots, &CHAINMAIL_ARMOR),
    shaped(GILDED_IRON_HELMET, GILDED_IRON, Shape::Helmet, &GILDED_IRON_ARMOR),
    shaped(GILDED_IRON_CHESTPLATE, GILDED_IRON, Shape::Chestplate, &GILDED_IRON_ARMOR),
    shaped(GILDED_IRON_LEGGINGS, GILDED_IRON, Shape::Leggings, &GILDED_IRON_ARMOR),
    shaped(GILDED_IRON_BOOTS, GILDED_IRON, Shape::Boots, &GILDED_IRON_ARMOR),
    craft(
        [
            Some(vanilla!("orange_wool")), Some(REINFORCED_CLOTH), Some(vanilla!("orange_wool")),
            Some(REINFORCED_CLOTH), Some(vanilla!("glass_pane")), Some(REINFORCED_CLOTH),
            None, None, None,
        ],
        SCUBA_HELMET, 1, &HAZMAT_SUIT,
    ),
    craft(
        [
            Some(vanilla!("orange_wool")), None, Some(vanilla!("orange_wool")),
            Some(REINFORCED_CLOTH), Some(REINFORCED_CLOTH), Some(REINFORCED_CLOTH),
            Some(vanilla!("black_wool")), Some(REINFORCED_CLOTH), Some(vanilla!("black_wool")),
        ],
        HAZMAT_CHESTPLATE, 1, &HAZMAT_SUIT,
    ),
    craft(
        [
            Some(vanilla!("black_wool")), Some(REINFORCED_CLOTH), Some(vanilla!("black_wool")),
            Some(REINFORCED_CLOTH), None, Some(REINFORCED_CLOTH),
            Some(REINFORCED_CLOTH), None, Some(REINFORCED_CLOTH),
        ],
        HAZMAT_LEGGINGS, 1, &HAZMAT_SUIT,
    ),
    craft(
        [
            Some(REINFORCED_CLOTH), None, Some(REINFORCED_CLOTH),
            Some(REINFORCED_CLOTH), None, Some(REINFORCED_CLOTH),
            Some(vanilla!("black_wool")), None, Some(vanilla!("black_wool")),
        ],
        RUBBER_BOOTS, 1, &HAZMAT_SUIT,
    ),
    shaped(GOLD_12K_HELMET, GOLD_12K, Shape::Helmet, &GOLD_ARMOR),
    shaped(GOLD_12K_CHESTPLATE, GOLD_12K, Shape::Chestplate, &GOLD_ARMOR),
    shaped(GOLD_12K_LEGGINGS, GOLD_12K, Shape::Leggings, &GOLD_ARMOR),
    shaped(GOLD_12K_BOOTS, GOLD_12K, Shape::Boots, &GOLD_ARMOR),
    craft(
        [
            Some(vanilla!("coal_block")), Some(vanilla!("coal_block")), Some(vanilla!("coal_block")),
            Some(vanilla!("lime_stained_glass_pane")), Some(vanilla!("coal_block")), Some(vanilla!("lime_stained_glass_pane")),
            Some(vanilla!("coal_block")), None, Some(vanilla!("coal_block")),
        ],
        NIGHT_VISION_GOGGLES, 1, &NIGHT_VISION_GOOGLES,
    ),
    craft(
        [
            Some(vanilla!("slime_ball")), Some(STEEL_PLATE), Some(vanilla!("slime_ball")),
            Some(vanilla!("slime_ball")), None, Some(vanilla!("slime_ball")),
            None, None, None,
        ],
        SLIME_STEEL_HELMET, 1, &SLIME_STEEL_ARMOR,
    ),
    craft(
        [
            Some(vanilla!("slime_ball")), None, Some(vanilla!("slime_ball")),
            Some(vanilla!("slime_ball")), Some(STEEL_PLATE), Some(vanilla!("slime_ball")),
            Some(vanilla!("slime_ball")), Some(vanilla!("slime_ball")), Some(vanilla!("slime_ball")),
        ],
        SLIME_STEEL_CHESTPLATE, 1, &SLIME_STEEL_ARMOR,
    ),
    craft(
        [
            Some(vanilla!("slime_ball")), Some(STEEL_PLATE), Some(vanilla!("slime_ball")),
            Some(vanilla!("slime_ball")), None, Some(vanilla!("slime_ball")),
            Some(vanilla!("slime_ball")), None, Some(vanilla!("slime_ball")),
        ],
        SLIME_STEEL_LEGGINGS, 1, &SLIME_STEEL_ARMOR,
    ),
    craft(
        [
            None, None, None,
            Some(vanilla!("slime_ball")), None, Some(vanilla!("slime_ball")),
            Some(vanilla!("slime_ball")), Some(STEEL_PLATE), Some(vanilla!("slime_ball")),
        ],
        SLIME_STEEL_BOOTS, 1, &SLIME_STEEL_ARMOR,
    ),
    craft(
        [
            None, None, None,
            Some(vanilla!("hay_block")), None, Some(vanilla!("hay_block")),
            Some(vanilla!("hay_block")), None, Some(vanilla!("hay_block")),
        ],
        FARMER_SHOES, 1, &FARMER_SHOES_RESEARCH,
    ),
    craft(
        [
            None, None, None,
            Some(vanilla!("yellow_wool")), None, Some(vanilla!("yellow_wool")),
            Some(vanilla!("piston")), None, Some(vanilla!("piston")),
        ],
        BOOTS_OF_THE_STOMPER, 1, &STOMPER_RESEARCH,
    ),
    craft(
        [
            Some(GOLD_8K), Some(vanilla!("honey_block")), Some(GOLD_8K),
            Some(vanilla!("honeycomb_block")), None, Some(vanilla!("honeycomb_block")),
            None, None, None,
        ],
        BEE_HELMET, 1, &BEE_ARMOR,
    ),
    craft(
        [
            Some(GOLD_8K), None, Some(GOLD_8K),
            Some(vanilla!("honeycomb_block")), Some(vanilla!("elytra")), Some(vanilla!("honeycomb_block")),
            Some(vanilla!("honey_block")), Some(GOLD_8K), Some(vanilla!("honey_block")),
        ],
        BEE_WINGS, 1, &BEE_ARMOR,
    ),
    craft(
        [
            Some(GOLD_8K), Some(vanilla!("honey_block")), Some(GOLD_8K),
            Some(vanilla!("honeycomb_block")), None, Some(vanilla!("honeycomb_block")),
            Some(vanilla!("honeycomb_block")), None, Some(vanilla!("honeycomb_block")),
        ],
        BEE_LEGGINGS, 1, &BEE_ARMOR,
    ),
    craft(
        [
            None, None, None,
            Some(GOLD_8K), None, Some(GOLD_8K),
            Some(vanilla!("honey_block")), None, Some(vanilla!("honey_block")),
        ],
        BEE_BOOTS, 1, &BEE_ARMOR,
    ),
    craft(
        [
            Some(vanilla!("slime_ball")), Some(vanilla!("slime_ball")), Some(vanilla!("slime_ball")),
            Some(ELYTRA_SCALE), Some(ELYTRA_SCALE), Some(ELYTRA_SCALE),
            Some(vanilla!("slime_ball")), Some(vanilla!("leather_helmet")), Some(vanilla!("slime_ball")),
        ],
        ELYTRA_CAP, 1, &ELYTRA_CAP_RESEARCH,
    ),
];

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    craft(
        [
            None,
            None,
            Some(STEEL_INGOT),
            None,
            Some(STEEL_INGOT),
            None,
            Some(STEEL_INGOT),
            None,
            None,
        ],
        CHAIN,
        8,
        &UNLOCKED,
    ),
    craft(
        [
            Some(vanilla!("white_wool")),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ],
        CLOTH,
        8,
        &UNLOCKED,
    ),
    craft(
        [
            None,
            Some(CLOTH),
            None,
            Some(CLOTH),
            Some(LEAD_INGOT),
            Some(CLOTH),
            None,
            Some(CLOTH),
            None,
        ],
        REINFORCED_CLOTH,
        2,
        &LEAD_CLOTHING,
    ),
];

pub(crate) const ALTAR_RECIPES: &[CraftingRecipe] = &[
    altar(
        [
            vanilla!("ender_pearl"),
            ENDER_LUMP_3,
            vanilla!("ender_pearl"),
            vanilla!("ender_eye"),
            crate::ancient_altar::BLANK_RUNE,
            vanilla!("ender_eye"),
            vanilla!("ender_pearl"),
            ENDER_LUMP_3,
            vanilla!("ender_pearl"),
        ],
        ANCIENT_RUNE_ENDER,
        6,
        &SPECIAL_RUNES,
    ),
    altar(
        [
            vanilla!("red_dye"),
            MAGIC_LUMP_3,
            vanilla!("cyan_dye"),
            vanilla!("white_wool"),
            ANCIENT_RUNE_ENDER,
            vanilla!("white_wool"),
            vanilla!("yellow_dye"),
            ENDER_LUMP_3,
            vanilla!("magenta_dye"),
        ],
        ANCIENT_RUNE_RAINBOW,
        1,
        &SPECIAL_RUNES,
    ),
    altar(
        [
            ENDER_LUMP_3,
            crate::ancient_altar::ANCIENT_RUNE_AIR,
            ENDER_LUMP_3,
            vanilla!("phantom_membrane"),
            vanilla!("feather"),
            vanilla!("phantom_membrane"),
            ENDER_LUMP_3,
            crate::ancient_altar::ANCIENT_RUNE_AIR,
            ENDER_LUMP_3,
        ],
        ELYTRA_SCALE,
        1,
        &ELYTRA,
    ),
    altar(
        [
            vanilla!("emerald"),
            vanilla!("leather"),
            MAGIC_LUMP_2,
            vanilla!("rabbit_hide"),
            ANCIENT_RUNE_RAINBOW,
            vanilla!("rabbit_hide"),
            MAGIC_LUMP_2,
            vanilla!("leather"),
            vanilla!("emerald"),
        ],
        RAINBOW_LEATHER,
        4,
        &UNLOCKED,
    ),
];

const fn altar(
    [a, b, c, d, e, f, g, h, i]: [Kind; 9],
    output: Kind,
    count: u32,
    research: &'static Research,
) -> CraftingRecipe {
    craft(
        [
            Some(a),
            Some(b),
            Some(c),
            Some(d),
            Some(e),
            Some(f),
            Some(g),
            Some(h),
            Some(i),
        ],
        output,
        count,
        research,
    )
}

pub(crate) fn paper_id(kind: Kind) -> &'static str {
    match kind {
        Kind::Vanilla("minecraft:chainmail_helmet") => "CHAIN_HELMET",
        Kind::Vanilla("minecraft:chainmail_chestplate") => "CHAIN_CHESTPLATE",
        Kind::Vanilla("minecraft:chainmail_leggings") => "CHAIN_LEGGINGS",
        Kind::Vanilla("minecraft:chainmail_boots") => "CHAIN_BOOTS",
        _ => kind.id(),
    }
}

pub(crate) fn registered_item(id: &str) -> Option<Kind> {
    ([ARMOR_FORGE].into_iter().chain(
        RECIPES
            .iter()
            .chain(CRAFTING)
            .chain(ALTAR_RECIPES)
            .map(|recipe| recipe.output.kind),
    ))
    .find(|item| paper_id(*item).eq_ignore_ascii_case(id) || item.id().eq_ignore_ascii_case(id))
}

pub(crate) fn decorate(id: &str, components: &mut Compound) {
    let resource_lore = match id {
        "ARMOR_FORGE" => &[(
            "Gives you the ability to create powerful armor",
            "green",
            false,
        )][..],
        "RAINBOW_LEATHER" => &[("Can be used to craft rainbow armor", "white", false)],
        "REINFORCED_CLOTH" => &[
            ("This cloth has been reinforced", "white", false),
            ("with Lead to protect against", "white", false),
            ("radioactive substances", "white", false),
        ],
        _ => &[],
    };
    if !resource_lore.is_empty() {
        set_lore(components, resource_lore);
    }
    if !RECIPES.iter().any(|recipe| recipe.output.kind.id() == id) {
        return;
    }
    let (dye, enchantments): (Option<i32>, &[(&str, i32)]) = if id.starts_with("GLOWSTONE_") {
        (Some(0xFFFF00), &[])
    } else if id.starts_with("RAINBOW_") {
        (Some(0xFF00FF), &[])
    } else if id.starts_with("ENDER_") {
        (Some(0x1C1970), &[])
    } else if id.starts_with("SLIME_STEEL_") {
        (
            None,
            &[("minecraft:unbreaking", 4), ("minecraft:protection", 2)],
        )
    } else if id.starts_with("SLIME_") {
        (Some(0x00FF00), &[])
    } else if id.starts_with("CACTUS_") {
        (
            Some(0x008000),
            &[("minecraft:thorns", 3), ("minecraft:unbreaking", 6)],
        )
    } else if id.starts_with("DAMASCUS_STEEL_") {
        (
            None,
            &[("minecraft:unbreaking", 5), ("minecraft:protection", 5)],
        )
    } else if id.starts_with("REINFORCED_ALLOY_") {
        (
            None,
            &[("minecraft:unbreaking", 9), ("minecraft:protection", 9)],
        )
    } else if id.starts_with("GILDED_IRON_") {
        (
            None,
            &[("minecraft:unbreaking", 6), ("minecraft:protection", 8)],
        )
    } else if id.starts_with("GOLD_12K_") {
        (None, &[("minecraft:unbreaking", 10)])
    } else if id.starts_with("BEE_") {
        (
            None,
            &[("minecraft:unbreaking", 4), ("minecraft:protection", 2)],
        )
    } else {
        (
            match id {
                "SCUBA_HELMET" | "HAZMAT_CHESTPLATE" | "HAZMAT_LEGGINGS" => Some(0xFFA500),
                "RUBBER_BOOTS" | "NIGHT_VISION_GOGGLES" => Some(0x000000),
                "BOOTS_OF_THE_STOMPER" => Some(0x00FFFF),
                "FARMER_SHOES" => Some(0xFFFF00),
                "ELYTRA_CAP" => Some(0x800080),
                _ => None,
            },
            &[],
        )
    };
    if let Some(dye) = dye {
        components.insert("minecraft:dyed_color".into(), Tag::Int(dye));
    }
    if !enchantments.is_empty() {
        components.insert(
            "minecraft:enchantments".into(),
            Tag::compound(
                enchantments
                    .iter()
                    .map(|&(key, level)| (key, Tag::Int(level))),
            ),
        );
    }
    let hazard = [
        ("", "gray", false),
        ("Full set effects:", "gold", false),
        ("- Radiation immunity", "yellow", false),
        ("- Bee Sting protection", "yellow", false),
    ];
    let bouncy = ("Bouncy Feeling", "green", true);
    let ender = ("Sometimes its here, sometimes there!", "green", true);
    let reinforced = ("Reinforced", "gray", true);
    let space = ("", "gray", false);
    let jump = ("+ Jump Boost", "blue", false);
    let soft = ("+ No Fall Damage", "blue", false);
    let speed = ("+ Speed", "blue", false);
    let buzzing = ("Bzzzzzzz", "white", false);
    let description: &[(&str, &str, bool)] = match id {
        "SCUBA_HELMET" => &[("Allows you to breathe underwater", "gray", false)],
        "HAZMAT_CHESTPLATE" => &[("Allows you to walk through fire and lava", "gray", false)],
        "FARMER_SHOES" => &[("Prevents you from trampling your Crops", "gold", true)],
        "BOOTS_OF_THE_STOMPER" => &[
            space,
            ("All Fall Damage you receive", "blue", false),
            ("will be applied to nearby Mobs/Players", "blue", false),
            space,
            soft,
        ],
        "NIGHT_VISION_GOGGLES" => &[("+ Night Vision", "blue", false)],
        "ELYTRA_CAP" => &[
            ("This helmet will protect you from", "gray", false),
            ("crashing while flying with an elytra.", "gray", false),
        ],
        "ENDER_BOOTS" => &[ender, space, ("+ No Enderpearl Damage", "blue", false)],
        "ENDER_HELMET" | "ENDER_CHESTPLATE" | "ENDER_LEGGINGS" => &[ender],
        "SLIME_BOOTS" => &[bouncy, space, jump, soft],
        "SLIME_STEEL_BOOTS" => &[reinforced, space, bouncy, space, jump, soft],
        "SLIME_LEGGINGS" => &[bouncy, space, speed],
        "SLIME_STEEL_LEGGINGS" => &[reinforced, space, bouncy, space, speed],
        "SLIME_HELMET" | "SLIME_CHESTPLATE" => &[bouncy],
        "SLIME_STEEL_HELMET" | "SLIME_STEEL_CHESTPLATE" => &[reinforced, space, bouncy],
        "BEE_WINGS" => &[
            buzzing,
            space,
            ("Activates Slow falling", "blue", false),
            ("when approaching the ground", "blue", false),
        ],
        "BEE_BOOTS" => &[buzzing, space, jump, soft],
        "BEE_HELMET" | "BEE_LEGGINGS" => &[buzzing],
        name if name.starts_with("GLOWSTONE_") => &[
            ("Shining like the sun!", "green", true),
            space,
            ("+ Night Vision", "blue", false),
        ],
        name if name.starts_with("RAINBOW_") => &[(
            "Cycles through all Colors of the Rainbow!",
            "light_purple",
            false,
        )],
        _ => &[],
    };
    let is_hazmat = matches!(
        id,
        "SCUBA_HELMET" | "HAZMAT_CHESTPLATE" | "HAZMAT_LEGGINGS" | "RUBBER_BOOTS"
    );
    if !description.is_empty() || is_hazmat {
        let lines = (!description.is_empty())
            .then_some(space)
            .into_iter()
            .chain(description.iter().copied())
            .chain(is_hazmat.then_some(hazard).into_iter().flatten());
        let lore = lines
            .map(|(line, color, italic)| text(line, color, italic))
            .collect();
        components.insert("minecraft:lore".into(), Tag::List(lore));
    }
}

fn set_lore(components: &mut Compound, lines: &[(&str, &str, bool)]) {
    let lore = [text("", "gray", false)]
        .into_iter()
        .chain(
            lines
                .iter()
                .map(|&(line, color, italic)| text(line, color, italic)),
        )
        .collect();
    components.insert("minecraft:lore".into(), Tag::List(lore));
}

const EFFECT_INTERVAL: i64 = 10 * 20;
const RAINBOW_INTERVAL: i64 = 5 * 20;
// Paper uses DyeColor.getColor(), not chat colors or dye firework colors.
const RAINBOW: [i32; 7] = [
    0xb02e26, 0xf9801d, 0xfed83d, 0x80c71f, 0x3ab3da, 0x8932b8, 0xc74ebd,
];

fn worn_effect(id: &str) -> Option<(&'static str, i32, i32, &'static Research)> {
    let effect = match id {
        "GLOWSTONE_HELMET" | "GLOWSTONE_CHESTPLATE" | "GLOWSTONE_LEGGINGS" | "GLOWSTONE_BOOTS" => {
            ("minecraft:night_vision", 600, 0, &GLOWSTONE_ARMOR)
        }
        "NIGHT_VISION_GOGGLES" => ("minecraft:night_vision", 600, 20, &NIGHT_VISION_GOOGLES),
        "SCUBA_HELMET" => ("minecraft:water_breathing", 300, 1, &HAZMAT_SUIT),
        "HAZMAT_CHESTPLATE" => ("minecraft:fire_resistance", 300, 1, &HAZMAT_SUIT),
        "SLIME_LEGGINGS" => ("minecraft:speed", 300, 2, &SLIME_ARMOR),
        "SLIME_STEEL_LEGGINGS" => ("minecraft:speed", 300, 2, &SLIME_STEEL_ARMOR),
        "SLIME_BOOTS" => ("minecraft:jump_boost", 300, 5, &SLIME_ARMOR),
        "SLIME_STEEL_BOOTS" => ("minecraft:jump_boost", 300, 5, &SLIME_STEEL_ARMOR),
        "BEE_BOOTS" => ("minecraft:jump_boost", 300, 2, &BEE_ARMOR),
        _ => return None,
    };
    Some(effect)
}

fn footwear_rule(action: &str, id: &str) -> Option<(&'static Research, Option<&'static str>)> {
    let rule = match (action, id) {
        ("farmland_trample", "FARMER_SHOES") => (&FARMER_SHOES_RESEARCH, None),
        ("ender_pearl_damage", "ENDER_BOOTS") => (&ENDER_ARMOR, None),
        ("fall_damage", "SLIME_BOOTS") => (&SLIME_ARMOR, Some("minecraft:block.slime_block.fall")),
        ("fall_damage", "SLIME_STEEL_BOOTS") => {
            (&SLIME_STEEL_ARMOR, Some("minecraft:block.slime_block.fall"))
        }
        ("fall_damage", "BEE_BOOTS") => (&BEE_ARMOR, Some("minecraft:block.honey_block.fall")),
        _ => return None,
    };
    Some(rule)
}

fn footwear_response(
    action: &str,
    boots: &mod_api::ItemStack,
    mode: mod_api::GameMode,
    persisted: Option<&Compound>,
) -> Option<Option<&'static str>> {
    let (unlock, sound) = footwear_rule(action, &crate::items::item_id(boots)?)?;
    equipment_researched(mode, persisted, unlock).then_some(sound)
}

fn equipment_researched(
    mode: mod_api::GameMode,
    persisted: Option<&Compound>,
    unlock: &Research,
) -> bool {
    mode == mod_api::GameMode::Creative
        || persisted.is_some_and(|saved| research::unlocked_persisted(saved, unlock))
}

pub(crate) fn stomp_unlocked(boots: &mod_api::ItemStack, persisted: Option<&Compound>) -> bool {
    crate::items::item_id(boots).as_deref() == Some("BOOTS_OF_THE_STOMPER")
        && persisted.is_some_and(|saved| research::unlocked_persisted(saved, &STOMPER_RESEARCH))
}

pub(crate) fn footwear(event: &mut mod_api::PlayerFootwearReactionEvent<'_>) -> mod_api::Flow {
    let Some(sound) = footwear_response(event.action, event.boots, event.actor.1, event.persisted)
    else {
        return mod_api::Flow::Continue;
    };
    *event.sound = sound.map(str::to_owned);
    mod_api::Flow::Cancel
}

fn protects_gliding_impact(
    helmet: &mod_api::ItemStack,
    mode: mod_api::GameMode,
    persisted: Option<&Compound>,
) -> bool {
    crate::items::item_id(helmet).as_deref() == Some(ELYTRA_CAP.id())
        && equipment_researched(mode, persisted, &ELYTRA_CAP_RESEARCH)
}

pub(crate) fn gliding_impact(event: &mut mod_api::PlayerGlidingImpactEvent<'_>) -> mod_api::Flow {
    if !protects_gliding_impact(event.helmet, event.actor.1, event.persisted) {
        return mod_api::Flow::Continue;
    }
    *event.reaction = (Some("minecraft:block.stone.hit".into()), true);
    mod_api::Flow::Cancel
}

pub(crate) fn tick_equipment(host: &mut dyn ServerHost, level: LevelRef) {
    let Some(tick) = level.game_time(host) else {
        return;
    };
    let use_effects = tick.rem_euclid(EFFECT_INTERVAL) == 0;
    let color = (tick.rem_euclid(RAINBOW_INTERVAL) == 0).then(|| {
        let index = (tick.div_euclid(RAINBOW_INTERVAL)).rem_euclid(RAINBOW.len() as i64);
        RAINBOW[index as usize]
    });
    if !use_effects && color.is_none() {
        return;
    }
    for player in level.online_players(host).unwrap_or_default() {
        let living = LivingRef::new(player.entity());
        let Some(slots) = player.inventory_slots(host) else {
            continue;
        };
        for (slot, mut item) in slots
            .into_iter()
            .filter(|(slot, _)| (36..40).contains(slot))
        {
            let Some(id) = crate::items::item_id(&item) else {
                continue;
            };
            if use_effects
                && let Some((effect, duration, amplifier, unlock)) = worn_effect(&id)
                && research::can_use(host, player, unlock)
                && let Some(key) = host.effect_key(effect)
            {
                let _ = living.remove_effect(host, key);
                let _ = living.add_effect(
                    host,
                    &ActiveEffect::new(key, duration, amplifier, false, true, true),
                );
            }
            if let Some(color) = color
                && matches!(
                    id.as_str(),
                    "RAINBOW_HELMET" | "RAINBOW_CHESTPLATE" | "RAINBOW_LEGGINGS" | "RAINBOW_BOOTS"
                )
                && research::can_use(host, player, &RAINBOW_ARMOR)
                && item
                    .apply_components(&Compound::from([(
                        "minecraft:dyed_color".into(),
                        Tag::Int(color),
                    )]))
                    .is_ok()
            {
                let _ = player.set_inventory_slot(host, slot, Some(item));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use mod_api::{Identifier, testing::FakeHost};

    use super::*;

    #[test]
    fn gliding_protection_requires_a_real_researched_elytra_cap() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse("minecraft:leather_helmet").unwrap()])
            .unwrap();
        let cap = ELYTRA_CAP.stack(&host, 1).unwrap();
        let vanilla = Kind::Vanilla("minecraft:leather_helmet")
            .stack(&host, 1)
            .unwrap();
        let research = Compound::from([("slimefun:elytra_cap".into(), Tag::Byte(1))]);
        let survival = mod_api::GameMode::Survival;
        assert!(!protects_gliding_impact(&cap, survival, None));
        assert!(!protects_gliding_impact(
            &vanilla,
            survival,
            Some(&research)
        ));
        assert!(protects_gliding_impact(&cap, survival, Some(&research)));
    }

    #[test]
    fn each_boot_reacts_only_to_its_original_action_and_research() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                [
                    "minecraft:leather_boots",
                    "minecraft:iron_boots",
                    "minecraft:golden_boots",
                ]
                .map(|id| Identifier::parse(id).unwrap()),
            )
            .unwrap();
        for (kind, action, unlock, sound) in [
            (FARMER_SHOES, "farmland_trample", "farmer_shoes", None),
            (ENDER_BOOTS, "ender_pearl_damage", "ender_armor", None),
            (
                SLIME_BOOTS,
                "fall_damage",
                "slime_armor",
                Some("minecraft:block.slime_block.fall"),
            ),
            (
                SLIME_STEEL_BOOTS,
                "fall_damage",
                "slime_steel_armor",
                Some("minecraft:block.slime_block.fall"),
            ),
            (
                BEE_BOOTS,
                "fall_damage",
                "bee_armor",
                Some("minecraft:block.honey_block.fall"),
            ),
        ] {
            assert_eq!(footwear_rule(action, kind.id()).unwrap().0.id, unlock);
            let boots = kind.stack(&host, 1).unwrap();
            let mode = mod_api::GameMode::Survival;
            assert_eq!(footwear_response(action, &boots, mode, None), None);
            let saved =
                Compound::from([(format!("slimefun:{unlock}").as_str().into(), Tag::Byte(1))]);
            assert_eq!(
                footwear_response(action, &boots, mode, Some(&saved)),
                Some(sound)
            );
        }
        assert!(footwear_rule("fall_damage", "ENDER_BOOTS").is_none());
        assert!(footwear_rule("fall_damage", "BOOTS_OF_THE_STOMPER").is_none());
        assert!(footwear_rule("farmland_trample", "SLIME_BOOTS").is_none());
    }

    #[test]
    fn a_stomp_requires_the_original_boots_and_research() {
        let mut host = FakeHost::new();
        let key = Identifier::parse("minecraft:leather_boots").unwrap();
        host.registries.resolve_items([key]).unwrap();
        let boots = BOOTS_OF_THE_STOMPER.stack(&host, 1).unwrap();
        let other = FARMER_SHOES.stack(&host, 1).unwrap();
        let saved = Compound::from([("slimefun:boots_of_the_stomper".into(), Tag::Byte(1))]);
        assert!(stomp_unlocked(&boots, Some(&saved)));
        assert!(!stomp_unlocked(&boots, None));
        assert!(!stomp_unlocked(&other, Some(&saved)));
    }

    #[test]
    fn all_paper_forge_ids_keep_their_patterns_and_vanilla_chainmail() {
        let ids: BTreeSet<_> = RECIPES.iter().map(|r| paper_id(r.output.kind)).collect();
        assert_eq!(RECIPES.len(), 56);
        assert_eq!(ids.len(), RECIPES.len());
        let boots = RECIPES
            .iter()
            .find(|r| paper_id(r.output.kind) == "CHAIN_BOOTS")
            .unwrap();
        assert_eq!(
            boots.inputs,
            [
                None,
                None,
                None,
                Some(CHAIN),
                None,
                Some(CHAIN),
                Some(CHAIN),
                None,
                Some(CHAIN)
            ]
        );
        assert_eq!(boots.research.id, "chainmail_armor");
        assert_eq!(
            registered_item("CHAIN_BOOTS"),
            Some(vanilla!("chainmail_boots"))
        );

        let ender = RECIPES
            .iter()
            .find(|r| r.output.kind == ENDER_HELMET)
            .unwrap();
        assert_eq!(ender.inputs[1], Some(vanilla!("ender_eye")));
        assert_eq!(ender.inputs[4], None);
        assert_eq!(ender.inputs[5], Some(vanilla!("obsidian")));
        assert_eq!(ender.research.cost, 4);

        let ingredients: BTreeSet<_> = RECIPES
            .iter()
            .flat_map(|r| r.inputs.iter().flatten())
            .filter(|kind| matches!(kind, Kind::Slimefun(_)))
            .map(|kind| kind.id())
            .collect();
        for important in [
            "CHAIN",
            "REINFORCED_CLOTH",
            "RAINBOW_LEATHER",
            "ELYTRA_SCALE",
        ] {
            assert!(ingredients.contains(important));
            assert!(
                CRAFTING
                    .iter()
                    .chain(ALTAR_RECIPES)
                    .any(|r| r.output.kind.id() == important)
            );
        }
    }

    #[test]
    fn wearables_have_real_minecraft_enchantments_dye_and_paper_effect_levels() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                [BEE_BOOTS, SCUBA_HELMET].map(|kind| Identifier::parse(kind.material()).unwrap()),
            )
            .unwrap();
        let bee = BEE_BOOTS.stack(&host, 1).unwrap();
        assert_eq!(crate::items::item_id(&bee).as_deref(), Some("BEE_BOOTS"));
        assert_eq!(
            bee.components()
                .unwrap()
                .as_nbt()
                .get(&"minecraft:enchantments".into())
                .and_then(|tag| tag.get("minecraft:unbreaking")),
            Some(&Tag::Int(4))
        );
        let scuba = SCUBA_HELMET.stack(&host, 1).unwrap();
        assert_eq!(
            scuba
                .components()
                .unwrap()
                .as_nbt()
                .get(&"minecraft:dyed_color".into()),
            Some(&Tag::Int(0xffa500))
        );
        let (id, duration, level, unlock) = worn_effect("SCUBA_HELMET").unwrap();
        assert_eq!(
            (id, duration, level, unlock.id),
            ("minecraft:water_breathing", 300, 1, "hazmat_suit")
        );
        assert!(worn_effect("ENDER_BOOTS").is_none());
    }
}
