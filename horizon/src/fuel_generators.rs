// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{Host, Identifier, ItemStack};

use crate::{
    heated_pressure::{BUCKET_OF_OIL, MAGNESIUM_SALT},
    items::{
        ALUMINUM_BRASS_INGOT, COMPRESSED_CARBON, CORINTHIAN_BRONZE_INGOT, DAMASCUS_STEEL_INGOT,
        DURALUMIN_INGOT, ELECTRIC_MOTOR, GOLD_16K, HARDENED_METAL_INGOT, HEATING_COIL, HeadTexture,
        Kind, NICKEL_INGOT, STEEL_INGOT, item,
    },
    recipes::{CraftingRecipe, Research, Stack},
};

const GENERATOR: HeadTexture = HeadTexture {
    profile: [-1442515976, -1100268200, -1139489874, 993810157],
    value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzkzNDNjZTU4ZGE1NGM3OTkyNGEyYzkzMzFjZmM0MTdmZThjY2JiZWE5YmU0NWE3YWM4NTg2MGE2YzczMCJ9fX0=",
};

item!(
    COAL_GENERATOR,
    "player_head",
    "Coal Generator",
    "red",
    None,
    Some(GENERATOR)
);
item!(
    COAL_GENERATOR_2,
    "player_head",
    "Coal Generator (II)",
    "red",
    None,
    Some(GENERATOR)
);
item!(
    LAVA_GENERATOR,
    "player_head",
    "Lava Generator",
    "dark_red",
    None,
    Some(GENERATOR)
);
item!(
    LAVA_GENERATOR_2,
    "player_head",
    "Lava Generator (II)",
    "dark_red",
    None,
    Some(GENERATOR)
);
item!(
    MAGNESIUM_GENERATOR,
    "player_head",
    "Magnesium-powered Generator",
    "red",
    None,
    Some(GENERATOR)
);
item!(
    COMBUSTION_REACTOR,
    "player_head",
    "Combustion Reactor",
    "red",
    None,
    Some(GENERATOR)
);
item!(
    BIO_REACTOR,
    "lime_terracotta",
    "Bio Reactor",
    "dark_green",
    None,
    None
);
item!(
    STRANGE_NETHER_GOO,
    "purple_dye",
    "Strange Nether Goo",
    "dark_purple",
    None,
    None
);
item!(
    BUCKET_OF_FUEL,
    "player_head",
    "Bucket of Fuel",
    "white",
    None,
    Some(HeadTexture {
        profile: [-2130704526, -216319229, -1462964295, -58088772],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2E4NGRkY2E3NjY3MjViOGI5NzQxM2YyNTljM2Y3NjY4MDcwZjZhZTU1NDgzYTkwYzhlNTUyNTM5NGY5YzA5OSJ9fX0="
    })
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FuelKind {
    Coal,
    Lava,
    Magnesium,
    Bio,
    Combustion,
}

#[derive(Clone, Copy)]
pub(crate) struct Generator {
    pub item: Kind,
    pub title: &'static str,
    pub capacity: u32,
    pub production: u32,
    pub research: &'static Research,
    fuel: FuelKind,
}

pub(crate) fn generator(id: &str) -> Option<&'static Generator> {
    GENERATORS
        .iter()
        .find(|generator| generator.item.id() == id)
}

// Paper SlimefunItemSetup declares the per-operation energy and capacity.
const GENERATORS: &[Generator] = &[
    Generator {
        item: COAL_GENERATOR,
        title: "Coal Generator",
        capacity: 64,
        production: 8,
        research: &COAL,
        fuel: FuelKind::Coal,
    },
    Generator {
        item: COAL_GENERATOR_2,
        title: "Coal Generator (II)",
        capacity: 256,
        production: 15,
        research: &IMPROVED,
        fuel: FuelKind::Coal,
    },
    Generator {
        item: LAVA_GENERATOR,
        title: "Lava Generator",
        capacity: 512,
        production: 10,
        research: &LAVA,
        fuel: FuelKind::Lava,
    },
    Generator {
        item: LAVA_GENERATOR_2,
        title: "Lava Generator (II)",
        capacity: 1024,
        production: 20,
        research: &IMPROVED,
        fuel: FuelKind::Lava,
    },
    Generator {
        item: MAGNESIUM_GENERATOR,
        title: "Magnesium-powered Generator",
        capacity: 128,
        production: 18,
        research: &MAGNESIUM,
        fuel: FuelKind::Magnesium,
    },
    Generator {
        item: BIO_REACTOR,
        title: "Bio Reactor",
        capacity: 128,
        production: 4,
        research: &BIO,
        fuel: FuelKind::Bio,
    },
    Generator {
        item: COMBUSTION_REACTOR,
        title: "Combustion Reactor",
        capacity: 256,
        production: 12,
        research: &COMBUSTION,
        fuel: FuelKind::Combustion,
    },
];

pub(crate) struct Fuel {
    pub ticks: u32,
    pub returns_bucket: bool,
}

impl Generator {
    pub(crate) fn progress_icon(self) -> &'static str {
        if self.fuel == FuelKind::Bio {
            "minecraft:golden_hoe"
        } else {
            "minecraft:flint_and_steel"
        }
    }

    pub(crate) fn fuel(self, host: &dyn Host, stack: &ItemStack) -> Option<Fuel> {
        let seconds = match self.fuel {
            FuelKind::Coal => coal_fuel(host, stack),
            FuelKind::Lava => Kind::Vanilla("minecraft:lava_bucket")
                .matches(host, stack)
                .then_some(40),
            FuelKind::Magnesium => MAGNESIUM_SALT.matches(host, stack).then_some(20),
            FuelKind::Bio if STRANGE_NETHER_GOO.matches(host, stack) => Some(16),
            FuelKind::Bio => bio_fuel(host, stack),
            FuelKind::Combustion if BUCKET_OF_OIL.matches(host, stack) => Some(30),
            FuelKind::Combustion if BUCKET_OF_FUEL.matches(host, stack) => Some(90),
            FuelKind::Combustion => None,
        }?;
        Some(Fuel {
            ticks: seconds * 2,
            returns_bucket: matches!(self.fuel, FuelKind::Lava | FuelKind::Combustion),
        })
    }
}

fn coal_fuel(host: &dyn Host, stack: &ItemStack) -> Option<u32> {
    if stack.components().is_some() || stack.damage_component().is_some() {
        return None;
    }
    match stack.item().id(host).as_str() {
        "minecraft:coal_block" => Some(80),
        "minecraft:blaze_rod" => Some(12),
        "minecraft:dried_kelp_block" => Some(20),
        "minecraft:coal" | "minecraft:charcoal" => Some(8),
        _ if item_in_tag(host, stack, "minecraft:boats") => Some(5),
        id => block_tag_fuel(
            host,
            id,
            &[
                ("minecraft:logs", 4),
                ("minecraft:planks", 1),
                ("minecraft:wooden_slabs", 1),
                ("minecraft:wooden_buttons", 1),
                ("minecraft:wooden_fences", 1),
                ("minecraft:wooden_trapdoors", 3),
                ("minecraft:wooden_pressure_plates", 2),
                ("minecraft:wooden_doors", 2),
                ("minecraft:standing_signs", 2),
            ],
        ),
    }
}

fn bio_fuel(host: &dyn Host, stack: &ItemStack) -> Option<u32> {
    if stack.components().is_some() || stack.damage_component().is_some() {
        return None;
    }
    match stack.item().id(host).as_str() {
        "minecraft:rotten_flesh"
        | "minecraft:spider_eye"
        | "minecraft:bone"
        | "minecraft:string"
        | "minecraft:red_mushroom"
        | "minecraft:brown_mushroom"
        | "minecraft:vine"
        | "minecraft:cactus"
        | "minecraft:lily_pad"
        | "minecraft:dried_kelp"
        | "minecraft:sea_pickle"
        | "minecraft:sweet_berries"
        | "minecraft:cocoa_beans"
        | "minecraft:crimson_fungus"
        | "minecraft:warped_fungus"
        | "minecraft:glow_berries"
        | "minecraft:glow_lichen" => Some(2),
        "minecraft:apple"
        | "minecraft:melon_slice"
        | "minecraft:pumpkin"
        | "minecraft:pumpkin_seeds"
        | "minecraft:melon_seeds"
        | "minecraft:wheat"
        | "minecraft:wheat_seeds"
        | "minecraft:carrot"
        | "minecraft:potato"
        | "minecraft:sugar_cane"
        | "minecraft:nether_wart"
        | "minecraft:beetroot"
        | "minecraft:beetroot_seeds"
        | "minecraft:small_dripleaf"
        | "minecraft:big_dripleaf" => Some(3),
        "minecraft:kelp" | "minecraft:seagrass" | "minecraft:bamboo" => Some(1),
        "minecraft:honeycomb" | "minecraft:shroomlight" => Some(4),
        "minecraft:chorus_fruit" => Some(8),
        "minecraft:dried_kelp_block" | "minecraft:spore_blossom" => Some(20),
        "minecraft:melon" => Some(27),
        "minecraft:honeycomb_block" => Some(40),
        id => block_tag_fuel(
            host,
            id,
            &[
                ("minecraft:small_flowers", 1),
                ("minecraft:leaves", 1),
                ("minecraft:saplings", 1),
                ("minecraft:corals", 2),
                ("minecraft:coral_blocks", 2),
            ],
        ),
    }
}

fn item_in_tag(host: &dyn Host, stack: &ItemStack, id: &str) -> bool {
    Identifier::parse(id)
        .ok()
        .and_then(|tag| host.tags().key(&tag))
        .and_then(|tag| host.tags().is_in_tag(stack.item(), tag))
        == Some(true)
}

fn block_tag_fuel(host: &dyn Host, id: &str, tags: &[(&str, u32)]) -> Option<u32> {
    // Bukkit's generator tags are block tags except for ITEMS_BOATS above.
    let block = host.block_key(id)?;
    tags.iter().find_map(|&(tag, seconds)| {
        (host.block_in_tag(block, tag) == Some(true)).then_some(seconds)
    })
}

const COAL: Research = Research {
    id: "coal_generator",
    name: "Coal Generator",
    cost: 14,
};
const LAVA: Research = Research {
    id: "lava_generator",
    name: "Lava Generator",
    cost: 16,
};
const BIO: Research = Research {
    id: "bio_reactor",
    name: "Bio-Reactor",
    cost: 18,
};
const MAGNESIUM: Research = Research {
    id: "magnesium_generator",
    name: "Power from Magnesium",
    cost: 20,
};
const COMBUSTION: Research = Research {
    id: "combustion_reactor",
    name: "Combustion Reactor",
    cost: 38,
};
const IMPROVED: Research = Research {
    id: "improved_generators",
    name: "Improved Generators",
    cost: 24,
};

pub(crate) const RESEARCHES: &[Research] = &[COAL, LAVA, BIO, MAGNESIUM, COMBUSTION, IMPROVED];

macro_rules! craft {
    ($result:ident, $research:ident; $($input:expr),+ $(,)?) => {
        CraftingRecipe {
            inputs: [$($input),+],
            output: Stack { kind: $result, count: 1 },
            research: &$research,
        }
    };
}

const fn v(id: &'static str) -> Option<Kind> {
    Some(Kind::Vanilla(id))
}

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    craft!(COAL_GENERATOR, COAL;
        Some(HEATING_COIL), v("minecraft:furnace"), Some(HEATING_COIL),
        Some(NICKEL_INGOT), Some(ELECTRIC_MOTOR), Some(NICKEL_INGOT),
        None, Some(NICKEL_INGOT), None
    ),
    craft!(COAL_GENERATOR_2, IMPROVED;
        v("minecraft:magma_block"), Some(HEATING_COIL), v("minecraft:magma_block"),
        Some(HARDENED_METAL_INGOT), Some(COAL_GENERATOR), Some(HARDENED_METAL_INGOT),
        None, Some(ELECTRIC_MOTOR), None
    ),
    craft!(BIO_REACTOR, BIO;
        Some(HEATING_COIL), v("minecraft:composter"), Some(HEATING_COIL),
        Some(ALUMINUM_BRASS_INGOT), Some(ELECTRIC_MOTOR), Some(ALUMINUM_BRASS_INGOT),
        None, Some(ALUMINUM_BRASS_INGOT), None
    ),
    craft!(MAGNESIUM_GENERATOR, MAGNESIUM;
        None, Some(ELECTRIC_MOTOR), None,
        Some(COMPRESSED_CARBON), v("minecraft:water_bucket"), Some(COMPRESSED_CARBON),
        Some(DURALUMIN_INGOT), Some(DURALUMIN_INGOT), Some(DURALUMIN_INGOT)
    ),
    craft!(LAVA_GENERATOR, LAVA;
        None, Some(GOLD_16K), None,
        Some(DAMASCUS_STEEL_INGOT), Some(ELECTRIC_MOTOR), Some(DAMASCUS_STEEL_INGOT),
        Some(HEATING_COIL), Some(DAMASCUS_STEEL_INGOT), Some(HEATING_COIL)
    ),
    craft!(LAVA_GENERATOR_2, IMPROVED;
        Some(CORINTHIAN_BRONZE_INGOT), Some(CORINTHIAN_BRONZE_INGOT), Some(CORINTHIAN_BRONZE_INGOT),
        Some(COMPRESSED_CARBON), Some(LAVA_GENERATOR), Some(COMPRESSED_CARBON),
        Some(HEATING_COIL), Some(COMPRESSED_CARBON), Some(HEATING_COIL)
    ),
    craft!(COMBUSTION_REACTOR, COMBUSTION;
        None, Some(STEEL_INGOT), None,
        Some(STEEL_INGOT), Some(ELECTRIC_MOTOR), Some(STEEL_INGOT),
        Some(HEATING_COIL), Some(STEEL_INGOT), Some(HEATING_COIL)
    ),
];
