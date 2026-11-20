// SPDX-License-Identifier: GPL-3.0-only

use crate::items::{
    ADVANCED_CIRCUIT_BOARD, BASIC_CIRCUIT_BOARD, BATTERY, COPPER_WIRE, ELECTRIC_MOTOR,
    ELECTRO_MAGNET, ENERGY_CONNECTOR, ENERGY_REGULATOR, IGNITION_CHAMBER, MAGNET, REDSTONE_ALLOY,
    SMALL_CAPACITOR, SOLAR_GENERATOR, SOLAR_PANEL,
};
use crate::items::{
    ALUMINUM_BRASS_INGOT, ALUMINUM_BRONZE_INGOT, ALUMINUM_DUST, ALUMINUM_INGOT, BILLON_INGOT,
    BRASS_INGOT, BRONZE_INGOT, CARBON, CARBON_CHUNK, COBALT_INGOT, COMPRESSED_CARBON, COPPER_DUST,
    COPPER_INGOT, CORINTHIAN_BRONZE_INGOT, CRUSHED_ORE, DAMASCUS_STEEL_INGOT, DURALUMIN_INGOT,
    ENDER_LUMP_1, ENDER_LUMP_2, ENDER_LUMP_3, FERROSILICON, GOLD_4K, GOLD_6K, GOLD_8K, GOLD_10K,
    GOLD_12K, GOLD_14K, GOLD_16K, GOLD_18K, GOLD_20K, GOLD_22K, GOLD_24K, GOLD_DUST, GOLD_PAN,
    HARDENED_METAL_INGOT, IRON_DUST, KELP_COOKIE, Kind, LEAD_DUST, LEAD_INGOT, MAGIC_LUMP_1,
    MAGIC_LUMP_2, MAGIC_LUMP_3, MAGNESIUM_DUST, MAGNESIUM_INGOT, NETHER_GOLD_PAN, NICKEL_INGOT,
    OUTPUT_CHEST, PULVERIZED_ORE, PURE_ORE_CLUSTER, REINFORCED_ALLOY_INGOT, REINFORCED_PLATE, SALT,
    SIFTED_ORE, SILICON, SILVER_DUST, SILVER_INGOT, SOLDER_INGOT, STEEL_INGOT, STEEL_PLATE,
    STONE_CHUNK, SULFATE, TIN_DUST, TIN_INGOT, WHEAT_FLOUR, ZINC_DUST, ZINC_INGOT,
};
use crate::items::{
    APPLE_JUICE, CARBONADO, CARROT_JUICE, GLOW_BERRY_JUICE, GOLDEN_APPLE_JUICE, MELON_JUICE,
    PUMPKIN_JUICE, RAW_CARBONADO, SWEET_BERRY_JUICE, SYNTHETIC_DIAMOND,
};
use crate::items::{
    ELECTRIC_FURNACE, ELECTRIC_FURNACE_2, ELECTRIC_FURNACE_3, GILDED_IRON, HEATING_COIL,
};
use crate::items::{COBALT_PICKAXE, GRANDMAS_WALKING_STICK, GRANDPAS_WALKING_STICK};

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

pub(crate) fn crafting_container(material: &str) -> Option<Kind> {
    Some(Kind::Vanilla(match material {
        "minecraft:water_bucket" | "minecraft:lava_bucket" | "minecraft:milk_bucket" => {
            "minecraft:bucket"
        }
        "minecraft:dragon_breath" | "minecraft:potion" | "minecraft:honey_bottle" => {
            "minecraft:glass_bottle"
        }
        _ => return None,
    }))
}

pub(crate) fn crafting() -> impl Iterator<Item = &'static CraftingRecipe> {
    CRAFTING
        .iter()
        .chain([&crate::climbing_pick::CRAFTING])
        .chain(crate::armor_forge::CRAFTING)
        .chain(crate::auto_crafter::CRAFTING)
        .chain([&crate::composter::CRAFTING])
        .chain([&crate::block_placer::CRAFTING])
        .chain(crate::network_recipes::CRAFTING)
        .chain(crate::electric_processing::CRAFTING)
        .chain(crate::fuel_generators::CRAFTING)
        .chain(crate::energy_gadgets::CRAFTING)
        .chain(crate::advanced_processing::CRAFTING)
        .chain(crate::thermal_machines::CRAFTING)
        .chain(crate::geology::CRAFTING)
        .chain([&crate::gps_controls::CRAFTING])
        .chain(crate::gps_transport::CRAFTING)
        .chain([&crate::elevator::CRAFTING])
        .chain(crate::backpacks::CRAFTING)
        .chain(crate::food_machines::CRAFTING)
        .chain(crate::medical::CRAFTING)
        .chain(crate::snacks::CRAFTING)
        .chain([&crate::smelters_pickaxe::CRAFTING, &crate::beheading::CRAFTING])
        .chain(crate::jet_boots::CRAFTING)
        .chain(crate::jetpack::CRAFTING)
        .chain(crate::grappling_hook::CRAFTING)
        .chain(crate::multi_tool::recipes())
        .chain(crate::elemental_staves::CRAFTING)
        .chain([&crate::auto_brewer::CRAFTING])
        .chain([&crate::factory::fluid_pump::CRAFTING])
        .chain([&crate::factory::charging_bench::CRAFTING])
        .chain([&crate::factory::produce_collector::CRAFTING])
        .chain([&crate::factory::tree_growth::CRAFTING])
        .chain(crate::factory::animal_machines::CRAFTING)
        .chain(crate::android_items::CRAFTING)
        .chain(crate::factory::crop_growth::CRAFTING)
        .chain(crate::factory::assemblers::CRAFTING)
        .chain(crate::network_runtime::trash::CRAFTING)
        .chain(crate::factory::reactors::CRAFTING)
        .chain([&crate::factory::experience_collector::COLLECTOR_RECIPE])
        .chain(crate::enchantment_machines::CRAFTING)
        .chain([&crate::rainbow::COOKIE_RECIPE])
        .chain(crate::auto_anvil::CRAFTING)
        .chain(crate::enhanced_furnaces::CRAFTING)
        .chain([&crate::book_binder::CRAFTING, &crate::tape_measure::CRAFTING])
        .chain([&crate::portable_crafter::CRAFTING])
}

pub(crate) fn researches() -> impl Iterator<Item = &'static Research> {
    RESEARCHES
        .iter()
        .chain(crate::armor_forge::RESEARCHES.iter().copied())
        .chain([&crate::auto_crafter::RESEARCH])
        .chain([&crate::composter::RESEARCH])
        .chain([&crate::block_placer::RESEARCH])
        .chain([&crate::portable_crafter::RESEARCH, &crate::multi_tool::RESEARCH])
        .chain([&crate::infused::HOPPER_RESEARCH, &crate::infused::MAGNET_RESEARCH])
        .chain([
            &crate::industrial_miner::RESEARCH,
            &crate::industrial_miner::ADVANCED_RESEARCH,
        ])
        .chain(crate::network_recipes::RESEARCHES)
        .chain(crate::electric_processing::RESEARCHES)
        .chain([&crate::energy_gadgets::MULTIMETER_RESEARCH])
        .chain(crate::fuel_generators::RESEARCHES)
        .chain(crate::advanced_processing::RESEARCHES)
        .chain(crate::thermal_machines::RESEARCHES)
        .chain(crate::geology::RESEARCHES)
        .chain(crate::gps_transport::RESEARCHES)
        .chain([&crate::elevator::RESEARCH])
        .chain(crate::food_machines::RESEARCHES)
        .chain([&crate::auto_brewer::RESEARCH])
        .chain([&crate::factory::fluid_pump::RESEARCH])
        .chain([&crate::factory::charging_bench::RESEARCH])
        .chain([&crate::factory::produce_collector::RESEARCH])
        .chain([&crate::factory::tree_growth::RESEARCH])
        .chain(crate::factory::animal_machines::RESEARCHES)
        .chain(crate::android_items::RESEARCHES.iter().copied())
        .chain([&crate::heated_pressure::PLASTIC_RESEARCH])
        .chain(crate::factory::crop_growth::RESEARCHES)
        .chain(crate::factory::assemblers::RESEARCHES)
        .chain(crate::backpacks::RESEARCHES)
        .chain(crate::soulbound_gear::RESEARCHES.iter().copied())
        .chain([
            &crate::ancient_altar::ALTAR_RESEARCH,
            &crate::ancient_altar::RUNES_RESEARCH,
            &crate::soulbound_rune::RESEARCH,
        ])
        .chain(crate::network_runtime::trash::RESEARCHES)
        .chain(crate::factory::reactors::RESEARCHES)
        .chain([
            &crate::factory::experience_collector::RESEARCH,
            &crate::factory::experience_collector::FLASK_RESEARCH,
        ])
        .chain(crate::enchantment_machines::RESEARCHES)
        .chain(crate::elemental_staves::RESEARCHES.iter().copied())
        .chain([&crate::ancient_altar::SYNTHETIC_SHULKERS])
        .chain([&crate::magic_eye::RESEARCH, &crate::nether_goo::RESEARCH])
        .chain([&crate::infernal_bonemeal::RESEARCH, &crate::zombie_pills::RESEARCH])
        .chain(crate::snacks::RESEARCHES.iter().copied())
        .chain([&crate::smelters_pickaxe::RESEARCH])
        .chain([&crate::spawner_repair::CONTAINMENT, &crate::spawner_repair::REPAIRING])
        .chain([&crate::magic_weapons::VAMPIRES, &crate::magic_weapons::BOWS])
        .chain([&crate::ore_tools::VEIN_RESEARCH, &crate::ore_tools::SEEKER_RESEARCH])
        .chain([&crate::beheading::RESEARCH, &crate::explosive_tools::RESEARCH])
        .chain([&crate::teleposition_scroll::RESEARCH])
        .chain(crate::talismans::RESEARCHES.iter().copied())
        .chain([&crate::enchantment_rune::LIGHTNING_RESEARCH])
        .chain([&crate::knowledge_tome::RESEARCH])
        .chain(crate::jet_boots::RESEARCHES.iter().copied())
        .chain(crate::jetpack::RESEARCHES.iter().copied())
        .chain([&crate::lumber_axe::RESEARCH, &crate::climbing_pick::RESEARCH])
        .chain([&crate::rainbow::RESEARCH])
        .chain(crate::auto_anvil::RESEARCHES)
        .chain(crate::enhanced_furnaces::RESEARCHES)
        .chain([&crate::book_binder::RESEARCH, &crate::tape_measure::RESEARCH])
}

pub(crate) fn magic_crafting() -> impl Iterator<Item = &'static CraftingRecipe> {
    [&crate::factory::experience_collector::FLASK_RECIPE]
        .into_iter()
        .chain(crate::backpacks::MAGIC_CRAFTING)
        .chain(crate::elemental_staves::MAGIC_CRAFTING)
        .chain([&crate::lumber_axe::CRAFTING])
        .chain([&crate::magic_eye::CRAFTING, &crate::snacks::DIET_RECIPE])
        .chain(crate::soulbound_gear::MAGIC_RECIPES)
        .chain(crate::ancient_altar::MAGIC_CRAFTING)
        .chain([&crate::spawner_repair::TOOL])
        .chain(crate::magic_weapons::CRAFTING)
        .chain(crate::ore_tools::CRAFTING)
        .chain(crate::explosive_tools::CRAFTING)
        .chain([&crate::teleposition_scroll::CRAFTING])
        .chain(crate::talismans::CRAFTING)
        .chain([&crate::knowledge_tome::CRAFTING])
        .chain([&crate::infused::MAGIC, &crate::zombie_pills::CRAFTING])
}

pub(crate) fn smelting() -> impl Iterator<Item = &'static SmeltingRecipe> {
    SMELTING.iter().chain([
        &crate::network_recipes::SAPPHIRE_SMELTING,
        &crate::advanced_processing::SYNTHETIC_EMERALD_SMELTING,
    ])
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
    GOLDEN_CITY,
    WALKING_STICKS,
    SPEEDY_PICKAXE,
    LUMPS,
    TASTY_KELP,
    AUTOMATED_PANNING_MACHINE,
    SYNTHETIC_DIAMONDS,
    POWER_ITEMS,
    ORE_PURIFICATION,
    CHEST_RESEARCH,
    ALLOYS,
    BRONZE,
    DAMASCUS_STEEL,
    GOLD_CARATS,
    GOLD_PAN_RESEARCH,
    GOLDEN_POTION,
    INGREDIENTS_AND_CHEESE,
    JUICER,
    MAKESHIFT_SMELTERY,
    NETHER_GOLD_PAN_RESEARCH,
    NICKEL_AND_COBALT,
    ORE_WASHER,
    PRESSURE_CHAMBER,
    BLACK_DIAMONDS,
    REINFORCED_ALLOY,
    SILICON_RESEARCH,
    SLIMEFUN_METALS,
    SMELTERY,
    STEEL,
    TABLE_SAW,
    BASIC_CIRCUIT_BOARD_RESEARCH,
    ADVANCED_CIRCUIT_BOARD_RESEARCH,
    AUTOMATIC_IGNITION_CHAMBER_RESEARCH,
    BATTERY_RESEARCH,
    MAGNET_RESEARCH,
    ELECTRIC_MOTOR_RESEARCH,
    COPPER_WIRE_RESEARCH,
    SOLAR_PANEL_AND_HELMET_RESEARCH,
    REDSTONE_ALLOY_RESEARCH,
    ENERGY_REGULATOR_RESEARCH,
    ENERGY_CONNECTORS_RESEARCH,
    CAPACITORS_RESEARCH,
    SOLAR_GENERATORS_RESEARCH,
    ELECTRIC_FURNACES_RESEARCH,
    BETTER_ELECTRIC_FURNACE_RESEARCH,
    GILDED_IRON_RESEARCH,
    ELECTRIC_ORE_GRINDING_RESEARCH,
    MEDIUM_TIER_ELECTRIC_INGOT_MACHINES_RESEARCH,
    HIGH_TIER_ELECTRIC_INGOT_MACHINES_RESEARCH,
    URANIUM_RESEARCH,
    HEATED_PRESSURE_CHAMBER_RESEARCH,
    BETTER_HEATED_PRESSURE_CHAMBER_RESEARCH,
    BLISTERING_INGOTS_RESEARCH,
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

const WALKING_STICKS: Research = Research {
    id: "walking_sticks",
    name: "Walking Sticks",
    cost: 1,
};
const SPEEDY_PICKAXE: Research = Research {
    id: "cobalt_pickaxe",
    name: "Speedy Pickaxe",
    cost: 14,
};

pub(crate) const GOLDEN_CITY: Research = Research {
    id: "24k_gold_block",
    name: "Golden City",
    cost: 19,
};
const NO_RESEARCH: Research = Research {
    id: "",
    name: "",
    cost: 0,
};
const GOLD_BLOCK: Option<Kind> = Some(crate::items::GOLD_24K_BLOCK);

const fn seasonal(inputs: [Option<Kind>; 9], output: Kind, count: u32) -> CraftingRecipe {
    CraftingRecipe {
        inputs,
        output: stack(output, count),
        research: &NO_RESEARCH,
    }
}

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    CraftingRecipe {
        inputs: [
            Some(vanilla!("milk_bucket")),
            Some(vanilla!("glass_bottle")),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ],
        output: Stack {
            kind: crate::items::CHRISTMAS_MILK,
            count: 4,
        },
        research: &NO_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            Some(crate::items::CHRISTMAS_MILK),
            Some(vanilla!("cocoa_beans")),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ],
        output: Stack {
            kind: crate::items::CHRISTMAS_CHOCOLATE_MILK,
            count: 2,
        },
        research: &NO_RESEARCH,
    },
    seasonal(
        [
            Some(crate::items::CHRISTMAS_MILK),
            Some(vanilla!("egg")),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ],
        crate::items::CHRISTMAS_EGG_NOG,
        2,
    ),
    seasonal(
        [
            Some(APPLE_JUICE),
            Some(vanilla!("sugar")),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ],
        crate::items::CHRISTMAS_APPLE_CIDER,
        2,
    ),
    seasonal(
        [
            Some(vanilla!("egg")),
            Some(vanilla!("apple")),
            Some(vanilla!("melon")),
            Some(vanilla!("sugar")),
            None,
            None,
            None,
            None,
            None,
        ],
        crate::items::CHRISTMAS_FRUIT_CAKE,
        4,
    ),
    seasonal(
        [
            Some(vanilla!("sugar")),
            Some(vanilla!("apple")),
            Some(vanilla!("egg")),
            None,
            None,
            None,
            None,
            None,
            None,
        ],
        crate::items::CHRISTMAS_APPLE_PIE,
        2,
    ),
    seasonal(
        [
            Some(vanilla!("egg")),
            Some(vanilla!("sugar")),
            Some(WHEAT_FLOUR),
            Some(vanilla!("milk_bucket")),
            None,
            None,
            None,
            None,
            None,
        ],
        crate::items::CHRISTMAS_CAKE,
        4,
    ),
    seasonal(
        [
            None,
            Some(crate::items::CHRISTMAS_CARAMEL),
            None,
            None,
            Some(vanilla!("apple")),
            None,
            None,
            Some(vanilla!("stick")),
            None,
        ],
        crate::items::CHRISTMAS_CARAMEL_APPLE,
        2,
    ),
    seasonal(
        [
            None,
            Some(vanilla!("cocoa_beans")),
            None,
            None,
            Some(vanilla!("apple")),
            None,
            None,
            Some(vanilla!("stick")),
            None,
        ],
        crate::items::CHRISTMAS_CHOCOLATE_APPLE,
        2,
    ),
    seasonal(
        [
            Some(vanilla!("sugar")),
            Some(vanilla!("carrot")),
            Some(vanilla!("egg")),
            None,
            None,
            None,
            None,
            None,
            None,
        ],
        crate::items::CARROT_PIE,
        2,
    ),
    seasonal(
        [
            Some(vanilla!("sugar")),
            Some(vanilla!("apple")),
            Some(vanilla!("egg")),
            None,
            None,
            None,
            None,
            None,
            None,
        ],
        crate::items::EASTER_APPLE_PIE,
        2,
    ),
    CraftingRecipe {
        inputs: [Some(GOLD_24K); 9],
        output: stack(crate::items::GOLD_24K_BLOCK, 1),
        research: &GOLDEN_CITY,
    },
    CraftingRecipe {
        inputs: [
            GOLD_BLOCK,
            GOLD_BLOCK,
            GOLD_BLOCK,
            GOLD_BLOCK,
            Some(vanilla!("apple")),
            GOLD_BLOCK,
            GOLD_BLOCK,
            GOLD_BLOCK,
            GOLD_BLOCK,
        ],
        output: stack(vanilla!("enchanted_golden_apple"), 1),
        research: &NO_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            None, Some(vanilla!("oak_log")), None,
            None, Some(vanilla!("oak_log")), None,
            None, Some(vanilla!("oak_log")), None,
        ],
        output: stack(GRANDMAS_WALKING_STICK, 1),
        research: &WALKING_STICKS,
    },
    CraftingRecipe {
        inputs: [
            Some(vanilla!("leather")), Some(vanilla!("oak_log")), Some(vanilla!("leather")),
            None, Some(vanilla!("oak_log")), None,
            None, Some(vanilla!("oak_log")), None,
        ],
        output: stack(GRANDPAS_WALKING_STICK, 1),
        research: &WALKING_STICKS,
    },
    CraftingRecipe {
        inputs: [
            Some(COBALT_INGOT), Some(COBALT_INGOT), Some(COBALT_INGOT),
            None, Some(NICKEL_INGOT), None,
            None, Some(NICKEL_INGOT), None,
        ],
        output: stack(COBALT_PICKAXE, 1),
        research: &SPEEDY_PICKAXE,
    },
    CraftingRecipe {
        inputs: [
            None,
            None,
            None,
            Some(vanilla!("stone")),
            Some(vanilla!("bowl")),
            Some(vanilla!("stone")),
            Some(vanilla!("stone")),
            Some(vanilla!("stone")),
            Some(vanilla!("stone")),
        ],
        output: stack(GOLD_PAN, 1),
        research: &GOLD_PAN_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            None,
            None,
            None,
            Some(vanilla!("nether_brick")),
            Some(GOLD_PAN),
            Some(vanilla!("nether_brick")),
            Some(vanilla!("nether_brick")),
            Some(vanilla!("nether_brick")),
            Some(vanilla!("nether_brick")),
        ],
        output: stack(NETHER_GOLD_PAN, 1),
        research: &NETHER_GOLD_PAN_RESEARCH,
    },
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
    CraftingRecipe {
        inputs: [
            Some(vanilla!("lapis_block")),
            Some(vanilla!("lapis_block")),
            Some(vanilla!("lapis_block")),
            Some(vanilla!("redstone_block")),
            Some(BASIC_CIRCUIT_BOARD),
            Some(vanilla!("redstone_block")),
            Some(vanilla!("lapis_block")),
            Some(vanilla!("lapis_block")),
            Some(vanilla!("lapis_block")),
        ],
        output: stack(ADVANCED_CIRCUIT_BOARD, 1),
        research: &ADVANCED_CIRCUIT_BOARD_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            Some(vanilla!("iron_ingot")),
            Some(vanilla!("flint_and_steel")),
            Some(vanilla!("iron_ingot")),
            Some(vanilla!("iron_ingot")),
            Some(BASIC_CIRCUIT_BOARD),
            Some(vanilla!("iron_ingot")),
            None,
            Some(vanilla!("observer")),
            None,
        ],
        output: stack(IGNITION_CHAMBER, 1),
        research: &AUTOMATIC_IGNITION_CHAMBER_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            None,
            Some(vanilla!("redstone")),
            None,
            Some(ZINC_INGOT),
            Some(SULFATE),
            Some(COPPER_INGOT),
            Some(ZINC_INGOT),
            Some(SULFATE),
            Some(COPPER_INGOT),
        ],
        output: stack(BATTERY, 1),
        research: &BATTERY_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            Some(NICKEL_INGOT),
            Some(MAGNET),
            Some(COBALT_INGOT),
            None,
            Some(BATTERY),
            None,
            None,
            None,
            None,
        ],
        output: stack(ELECTRO_MAGNET, 1),
        research: &ELECTRIC_MOTOR_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            Some(COPPER_WIRE),
            Some(COPPER_WIRE),
            Some(COPPER_WIRE),
            None,
            Some(ELECTRO_MAGNET),
            None,
            Some(COPPER_WIRE),
            Some(COPPER_WIRE),
            Some(COPPER_WIRE),
        ],
        output: stack(ELECTRIC_MOTOR, 1),
        research: &ELECTRIC_MOTOR_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            None,
            None,
            None,
            Some(COPPER_INGOT),
            Some(COPPER_INGOT),
            Some(COPPER_INGOT),
            None,
            None,
            None,
        ],
        output: stack(COPPER_WIRE, 8),
        research: &COPPER_WIRE_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            Some(vanilla!("glass")),
            Some(vanilla!("glass")),
            Some(vanilla!("glass")),
            Some(SILICON),
            Some(SILICON),
            Some(SILICON),
            Some(FERROSILICON),
            Some(FERROSILICON),
            Some(FERROSILICON),
        ],
        output: stack(SOLAR_PANEL, 1),
        research: &SOLAR_PANEL_AND_HELMET_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            Some(SILVER_INGOT),
            Some(DAMASCUS_STEEL_INGOT),
            Some(SILVER_INGOT),
            Some(DAMASCUS_STEEL_INGOT),
            Some(ELECTRIC_MOTOR),
            Some(DAMASCUS_STEEL_INGOT),
            Some(SILVER_INGOT),
            Some(DAMASCUS_STEEL_INGOT),
            Some(SILVER_INGOT),
        ],
        output: stack(ENERGY_REGULATOR, 1),
        research: &ENERGY_REGULATOR_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            Some(CARBON),
            Some(COPPER_WIRE),
            Some(CARBON),
            Some(COPPER_WIRE),
            Some(vanilla!("redstone_block")),
            Some(COPPER_WIRE),
            Some(CARBON),
            Some(COPPER_WIRE),
            Some(CARBON),
        ],
        output: stack(ENERGY_CONNECTOR, 8),
        research: &ENERGY_CONNECTORS_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            Some(DURALUMIN_INGOT),
            Some(SULFATE),
            Some(DURALUMIN_INGOT),
            Some(REDSTONE_ALLOY),
            Some(ENERGY_CONNECTOR),
            Some(REDSTONE_ALLOY),
            Some(DURALUMIN_INGOT),
            Some(vanilla!("redstone")),
            Some(DURALUMIN_INGOT),
        ],
        output: stack(SMALL_CAPACITOR, 1),
        research: &CAPACITORS_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            Some(SOLAR_PANEL),
            Some(SOLAR_PANEL),
            Some(SOLAR_PANEL),
            Some(ALUMINUM_INGOT),
            Some(ELECTRIC_MOTOR),
            Some(ALUMINUM_INGOT),
            None,
            Some(ALUMINUM_INGOT),
            None,
        ],
        output: stack(SOLAR_GENERATOR, 1),
        research: &SOLAR_GENERATORS_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            Some(COPPER_WIRE),
            Some(COPPER_WIRE),
            Some(COPPER_WIRE),
            Some(COPPER_WIRE),
            Some(ELECTRIC_MOTOR),
            Some(COPPER_WIRE),
            Some(COPPER_WIRE),
            Some(COPPER_WIRE),
            Some(COPPER_WIRE),
        ],
        output: stack(HEATING_COIL, 1),
        research: &ELECTRIC_MOTOR_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            None,
            Some(vanilla!("furnace")),
            None,
            Some(GILDED_IRON),
            Some(HEATING_COIL),
            Some(GILDED_IRON),
            Some(GILDED_IRON),
            Some(ELECTRIC_MOTOR),
            Some(GILDED_IRON),
        ],
        output: stack(ELECTRIC_FURNACE, 1),
        research: &ELECTRIC_FURNACES_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            None,
            Some(ELECTRIC_MOTOR),
            None,
            Some(GILDED_IRON),
            Some(ELECTRIC_FURNACE),
            Some(GILDED_IRON),
            Some(GILDED_IRON),
            Some(HEATING_COIL),
            Some(GILDED_IRON),
        ],
        output: stack(ELECTRIC_FURNACE_2, 1),
        research: &BETTER_ELECTRIC_FURNACE_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            None,
            Some(ELECTRIC_MOTOR),
            None,
            Some(STEEL_INGOT),
            Some(ELECTRIC_FURNACE_2),
            Some(STEEL_INGOT),
            Some(GILDED_IRON),
            Some(HEATING_COIL),
            Some(GILDED_IRON),
        ],
        output: stack(ELECTRIC_FURNACE_3, 1),
        research: &BETTER_ELECTRIC_FURNACE_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            None,
            Some(vanilla!("diamond_pickaxe")),
            None,
            Some(GILDED_IRON),
            Some(HEATING_COIL),
            Some(GILDED_IRON),
            Some(GILDED_IRON),
            Some(ELECTRIC_MOTOR),
            Some(GILDED_IRON),
        ],
        output: stack(crate::items::ELECTRIC_ORE_GRINDER, 1),
        research: &ELECTRIC_ORE_GRINDING_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            None,
            Some(vanilla!("diamond_pickaxe")),
            None,
            Some(HEATING_COIL),
            Some(crate::items::ELECTRIC_ORE_GRINDER),
            Some(HEATING_COIL),
            Some(GILDED_IRON),
            Some(crate::network_recipes::BLISTERING_INGOT_3),
            Some(GILDED_IRON),
        ],
        output: stack(crate::items::ELECTRIC_ORE_GRINDER_2, 1),
        research: &MEDIUM_TIER_ELECTRIC_INGOT_MACHINES_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            Some(REINFORCED_PLATE),
            Some(HEATING_COIL),
            Some(REINFORCED_PLATE),
            None,
            Some(crate::items::ELECTRIC_ORE_GRINDER_2),
            None,
            Some(REINFORCED_PLATE),
            Some(crate::network_recipes::BLISTERING_INGOT_3),
            Some(REINFORCED_PLATE),
        ],
        output: stack(crate::items::ELECTRIC_ORE_GRINDER_3, 1),
        research: &HIGH_TIER_ELECTRIC_INGOT_MACHINES_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            None,
            Some(crate::items::ELECTRIC_ORE_GRINDER),
            None,
            Some(LEAD_INGOT),
            Some(crate::network_recipes::MEDIUM_CAPACITOR),
            Some(LEAD_INGOT),
            Some(LEAD_INGOT),
            Some(HEATING_COIL),
            Some(LEAD_INGOT),
        ],
        output: stack(crate::items::ELECTRIC_INGOT_PULVERIZER, 1),
        research: &ELECTRIC_ORE_GRINDING_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            Some(crate::heated_pressure::TINY_URANIUM),
            Some(crate::heated_pressure::TINY_URANIUM),
            Some(crate::heated_pressure::TINY_URANIUM),
            Some(crate::heated_pressure::TINY_URANIUM),
            Some(crate::heated_pressure::TINY_URANIUM),
            Some(crate::heated_pressure::TINY_URANIUM),
            Some(crate::heated_pressure::TINY_URANIUM),
            Some(crate::heated_pressure::TINY_URANIUM),
            Some(crate::heated_pressure::TINY_URANIUM),
        ],
        output: stack(crate::heated_pressure::SMALL_URANIUM, 1),
        research: &URANIUM_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            Some(crate::heated_pressure::SMALL_URANIUM),
            Some(crate::heated_pressure::SMALL_URANIUM),
            None,
            Some(crate::heated_pressure::SMALL_URANIUM),
            Some(crate::heated_pressure::SMALL_URANIUM),
            None,
            None,
            None,
            None,
        ],
        output: stack(crate::heated_pressure::URANIUM, 1),
        research: &URANIUM_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            Some(LEAD_INGOT),
            Some(ELECTRIC_MOTOR),
            Some(LEAD_INGOT),
            Some(LEAD_INGOT),
            Some(vanilla!("glass")),
            Some(LEAD_INGOT),
            Some(LEAD_INGOT),
            Some(HEATING_COIL),
            Some(LEAD_INGOT),
        ],
        output: stack(crate::heated_pressure::HEATED_PRESSURE_CHAMBER, 1),
        research: &HEATED_PRESSURE_CHAMBER_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            Some(LEAD_INGOT),
            Some(ELECTRIC_MOTOR),
            Some(LEAD_INGOT),
            Some(LEAD_INGOT),
            Some(crate::heated_pressure::HEATED_PRESSURE_CHAMBER),
            Some(LEAD_INGOT),
            Some(REINFORCED_ALLOY_INGOT),
            Some(HEATING_COIL),
            Some(REINFORCED_ALLOY_INGOT),
        ],
        output: stack(crate::heated_pressure::HEATED_PRESSURE_CHAMBER_2, 1),
        research: &BETTER_HEATED_PRESSURE_CHAMBER_RESEARCH,
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
    single!(PURE_ORE_CLUSTER, 1, crate::heated_pressure::TINY_URANIUM, 1),
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
        crate::heated_pressure::PLASTIC_SHEET => Some(&crate::heated_pressure::PLASTIC_RESEARCH),
        crate::heated_pressure::TINY_URANIUM => Some(&URANIUM_RESEARCH),
        CRUSHED_ORE | PULVERIZED_ORE | PURE_ORE_CLUSTER => Some(&ORE_PURIFICATION),
        SALT => Some(&INGREDIENTS_AND_CHEESE),
        GOLD_4K | GOLD_6K | GOLD_8K | GOLD_10K | GOLD_12K | GOLD_14K | GOLD_16K | GOLD_18K
        | GOLD_20K | GOLD_22K | GOLD_24K => Some(&GOLD_CARATS),
        ALUMINUM_BRASS_INGOT
        | ALUMINUM_BRONZE_INGOT
        | BILLON_INGOT
        | BRASS_INGOT
        | CORINTHIAN_BRONZE_INGOT
        | DURALUMIN_INGOT
        | SOLDER_INGOT => Some(&ALLOYS),
        BRONZE_INGOT => Some(&BRONZE),
        DAMASCUS_STEEL_INGOT => Some(&DAMASCUS_STEEL),
        COBALT_INGOT | NICKEL_INGOT => Some(&NICKEL_AND_COBALT),
        HARDENED_METAL_INGOT | REINFORCED_ALLOY_INGOT => Some(&REINFORCED_ALLOY),
        FERROSILICON | SILICON => Some(&SILICON_RESEARCH),
        RAW_CARBONADO => Some(&BLACK_DIAMONDS),
        MAGNET => Some(&MAGNET_RESEARCH),
        GILDED_IRON => Some(&GILDED_IRON_RESEARCH),
        REINFORCED_PLATE => Some(&crate::network_recipes::HARDENED_GLASS_RESEARCH),
        crate::network_recipes::SYNTHETIC_SAPPHIRE => {
            Some(&crate::network_recipes::SYNTHETIC_SAPPHIRE_RESEARCH)
        }
        crate::advanced_processing::SYNTHETIC_EMERALD => {
            Some(&crate::advanced_processing::EMERALD_RESEARCH)
        }
        REDSTONE_ALLOY => Some(&REDSTONE_ALLOY_RESEARCH),
        ALUMINUM_INGOT | COPPER_INGOT | LEAD_INGOT | MAGNESIUM_INGOT | SILVER_INGOT | TIN_INGOT
        | ZINC_INGOT => Some(&SLIMEFUN_METALS),
        STEEL_INGOT => Some(&STEEL),
        _ => None,
    }
}

pub(crate) const CHEST_RESEARCH: Research = Research {
    id: "output_chest",
    name: "Basic machinery output chest",
    cost: 20,
};

pub(crate) const PAN_DROPS: &[(u32, Kind)] = &[
    (40, vanilla!("flint")),
    (20, vanilla!("clay_ball")),
    (35, SIFTED_ORE),
    (5, vanilla!("iron_nugget")),
];

pub(crate) const NETHER_PAN_DROPS: &[(u32, Kind)] = &[
    (50, vanilla!("quartz")),
    (25, vanilla!("gold_nugget")),
    (10, vanilla!("nether_wart")),
    (8, vanilla!("blaze_powder")),
    (5, vanilla!("glowstone_dust")),
    (2, vanilla!("ghast_tear")),
];

pub(crate) const WASHER_DUSTS: &[Kind] = &[
    IRON_DUST,
    GOLD_DUST,
    COPPER_DUST,
    TIN_DUST,
    ZINC_DUST,
    ALUMINUM_DUST,
    MAGNESIUM_DUST,
    LEAD_DUST,
    SILVER_DUST,
];

pub(crate) const WASHING: &[SingleRecipe] = &[
    single!(vanilla!("sand"), 2, SALT, 1),
    single!(PULVERIZED_ORE, 1, PURE_ORE_CLUSTER, 1),
];

pub(crate) const PRESSING: &[SingleRecipe] = &[
    single!(CARBON_CHUNK, 1, SYNTHETIC_DIAMOND, 1),
    single!(RAW_CARBONADO, 1, CARBONADO, 1),
];

pub(crate) const JUICING: &[SingleRecipe] = &[
    single!(vanilla!("apple"), 1, APPLE_JUICE, 1),
    single!(vanilla!("carrot"), 1, CARROT_JUICE, 1),
    single!(vanilla!("melon_slice"), 1, MELON_JUICE, 1),
    single!(vanilla!("pumpkin"), 1, PUMPKIN_JUICE, 1),
    single!(vanilla!("sweet_berries"), 1, SWEET_BERRY_JUICE, 1),
    single!(vanilla!("golden_apple"), 1, GOLDEN_APPLE_JUICE, 1),
    single!(vanilla!("glow_berries"), 1, GLOW_BERRY_JUICE, 1),
];

#[derive(Clone, Copy)]
pub(crate) struct SmeltingRecipe {
    pub inputs: &'static [Stack],
    pub output: Stack,
}

const fn smelt(inputs: &'static [Stack], kind: Kind) -> SmeltingRecipe {
    SmeltingRecipe {
        inputs,
        output: stack(kind, 1),
    }
}

pub(crate) const ALLOY_RECIPES: &[SmeltingRecipe] = &[
    smelt(
        &[
            stack(DAMASCUS_STEEL_INGOT, 1),
            stack(HARDENED_METAL_INGOT, 1),
            stack(CORINTHIAN_BRONZE_INGOT, 1),
            stack(SOLDER_INGOT, 1),
            stack(BILLON_INGOT, 1),
            stack(GOLD_24K, 1),
        ],
        REINFORCED_ALLOY_INGOT,
    ),
    smelt(
        &[
            stack(DAMASCUS_STEEL_INGOT, 1),
            stack(DURALUMIN_INGOT, 1),
            stack(COMPRESSED_CARBON, 1),
            stack(ALUMINUM_BRONZE_INGOT, 1),
        ],
        HARDENED_METAL_INGOT,
    ),
    smelt(
        &[
            stack(STEEL_INGOT, 1),
            stack(IRON_DUST, 1),
            stack(CARBON, 1),
            stack(vanilla!("iron_ingot"), 1),
        ],
        DAMASCUS_STEEL_INGOT,
    ),
    smelt(
        &[
            stack(IRON_DUST, 1),
            stack(CARBON, 1),
            stack(vanilla!("iron_ingot"), 1),
        ],
        STEEL_INGOT,
    ),
    smelt(
        &[
            stack(COPPER_DUST, 1),
            stack(TIN_DUST, 1),
            stack(COPPER_INGOT, 1),
        ],
        BRONZE_INGOT,
    ),
    smelt(
        &[
            stack(ALUMINUM_DUST, 1),
            stack(COPPER_DUST, 1),
            stack(ALUMINUM_INGOT, 1),
        ],
        DURALUMIN_INGOT,
    ),
    smelt(
        &[
            stack(SILVER_DUST, 1),
            stack(COPPER_DUST, 1),
            stack(SILVER_INGOT, 1),
        ],
        BILLON_INGOT,
    ),
    smelt(
        &[
            stack(COPPER_DUST, 1),
            stack(ZINC_DUST, 1),
            stack(COPPER_INGOT, 1),
        ],
        BRASS_INGOT,
    ),
    smelt(
        &[
            stack(ALUMINUM_DUST, 1),
            stack(BRASS_INGOT, 1),
            stack(ALUMINUM_INGOT, 1),
        ],
        ALUMINUM_BRASS_INGOT,
    ),
    smelt(
        &[
            stack(ALUMINUM_DUST, 1),
            stack(BRONZE_INGOT, 1),
            stack(ALUMINUM_INGOT, 1),
        ],
        ALUMINUM_BRONZE_INGOT,
    ),
    smelt(
        &[
            stack(SILVER_DUST, 1),
            stack(GOLD_DUST, 1),
            stack(COPPER_DUST, 1),
            stack(BRONZE_INGOT, 1),
        ],
        CORINTHIAN_BRONZE_INGOT,
    ),
    smelt(
        &[
            stack(LEAD_DUST, 1),
            stack(TIN_DUST, 1),
            stack(LEAD_INGOT, 1),
        ],
        SOLDER_INGOT,
    ),
    smelt(
        &[
            stack(IRON_DUST, 1),
            stack(vanilla!("iron_ingot"), 1),
            stack(COPPER_DUST, 1),
        ],
        NICKEL_INGOT,
    ),
    smelt(
        &[
            stack(IRON_DUST, 1),
            stack(COPPER_DUST, 1),
            stack(NICKEL_INGOT, 1),
        ],
        COBALT_INGOT,
    ),
];

pub(crate) const SMELTING: &[SmeltingRecipe] = &[
    smelt(
        &[stack(crate::items::CHRISTMAS_CHOCOLATE_MILK, 1)],
        crate::items::CHRISTMAS_HOT_CHOCOLATE,
    ),
    SmeltingRecipe {
        inputs: &[stack(vanilla!("sugar"), 1), stack(vanilla!("sugar"), 1)],
        output: stack(crate::items::CHRISTMAS_CARAMEL, 4),
    },
    smelt(
        &[
            stack(SYNTHETIC_DIAMOND, 1),
            stack(CARBON_CHUNK, 1),
            stack(vanilla!("glass_pane"), 1),
        ],
        RAW_CARBONADO,
    ),
    smelt(
        &[
            stack(vanilla!("iron_ingot"), 1),
            stack(IRON_DUST, 1),
            stack(SILICON, 1),
        ],
        FERROSILICON,
    ),
    smelt(&[stack(ALUMINUM_DUST, 1)], ALUMINUM_INGOT),
    smelt(&[stack(COPPER_DUST, 1)], COPPER_INGOT),
    smelt(&[stack(LEAD_DUST, 1)], LEAD_INGOT),
    smelt(&[stack(MAGNESIUM_DUST, 1)], MAGNESIUM_INGOT),
    smelt(&[stack(SILVER_DUST, 1)], SILVER_INGOT),
    smelt(&[stack(TIN_DUST, 1)], TIN_INGOT),
    smelt(&[stack(ZINC_DUST, 1)], ZINC_INGOT),
    smelt(&[stack(GOLD_DUST, 1), stack(GOLD_22K, 1)], GOLD_24K),
    smelt(&[stack(GOLD_DUST, 1), stack(GOLD_20K, 1)], GOLD_22K),
    smelt(&[stack(GOLD_DUST, 1), stack(GOLD_18K, 1)], GOLD_20K),
    smelt(&[stack(GOLD_DUST, 1), stack(GOLD_16K, 1)], GOLD_18K),
    smelt(&[stack(GOLD_DUST, 1), stack(GOLD_14K, 1)], GOLD_16K),
    smelt(&[stack(GOLD_DUST, 1), stack(GOLD_12K, 1)], GOLD_14K),
    smelt(&[stack(GOLD_DUST, 1), stack(GOLD_10K, 1)], GOLD_12K),
    smelt(&[stack(GOLD_DUST, 1), stack(GOLD_8K, 1)], GOLD_10K),
    smelt(&[stack(GOLD_DUST, 1), stack(GOLD_6K, 1)], GOLD_8K),
    smelt(&[stack(GOLD_DUST, 1), stack(GOLD_4K, 1)], GOLD_6K),
    smelt(&[stack(GOLD_DUST, 1)], GOLD_4K),
    smelt(&[stack(vanilla!("quartz_block"), 1)], SILICON),
    smelt(&[stack(IRON_DUST, 1)], vanilla!("iron_ingot")),
    smelt(
        &[
            stack(NICKEL_INGOT, 1),
            stack(ALUMINUM_DUST, 1),
            stack(IRON_DUST, 1),
            stack(COBALT_INGOT, 1),
        ],
        MAGNET,
    ),
    smelt(
        &[
            stack(vanilla!("redstone"), 1),
            stack(vanilla!("redstone_block"), 1),
            stack(FERROSILICON, 1),
            stack(HARDENED_METAL_INGOT, 1),
        ],
        REDSTONE_ALLOY,
    ),
    smelt(&[stack(GOLD_24K, 1), stack(IRON_DUST, 1)], GILDED_IRON),
];

pub(crate) const ALLOYS: Research = Research {
    id: "alloys",
    name: "Advanced Alloys",
    cost: 12,
};

pub(crate) const BRONZE: Research = Research {
    id: "bronze",
    name: "Bronze Creation",
    cost: 8,
};

pub(crate) const DAMASCUS_STEEL: Research = Research {
    id: "damascus_steel",
    name: "Damascus Steel",
    cost: 17,
};

pub(crate) const GOLD_CARATS: Research = Research {
    id: "gold_carats",
    name: "Pure Gold",
    cost: 7,
};

pub(crate) const GOLD_PAN_RESEARCH: Research = Research {
    id: "gold_pan",
    name: "Gold Pan",
    cost: 5,
};

pub(crate) const GOLDEN_POTION: Research = Research {
    id: "golden_apple_juice",
    name: "Golden potion",
    cost: 24,
};

pub(crate) const JUICER: Research = Research {
    id: "juicer",
    name: "Delicious Drinks",
    cost: 29,
};

pub(crate) const INGREDIENTS_AND_CHEESE: Research = Research {
    id: "ingredients_and_cheese",
    name: "Slimefun Cuisine",
    cost: 5,
};

pub(crate) const MAKESHIFT_SMELTERY: Research = Research {
    id: "makeshift_smeltery",
    name: "Improvised Smeltery",
    cost: 6,
};

pub(crate) const NETHER_GOLD_PAN_RESEARCH: Research = Research {
    id: "nether_gold_pan",
    name: "Nether Gold Pan",
    cost: 8,
};

pub(crate) const NICKEL_AND_COBALT: Research = Research {
    id: "nickel_and_cobalt",
    name: "Even more Ores",
    cost: 10,
};

pub(crate) const ORE_WASHER: Research = Research {
    id: "ore_washer",
    name: "Ore Washer",
    cost: 5,
};

pub(crate) const PRESSURE_CHAMBER: Research = Research {
    id: "pressure_chamber",
    name: "Pressure Chamber",
    cost: 14,
};

pub(crate) const BLACK_DIAMONDS: Research = Research {
    id: "carbonado",
    name: "Black Diamonds",
    cost: 26,
};

pub(crate) const TABLE_SAW: Research = Research {
    id: "table_saw",
    name: "Table Saw",
    cost: 4,
};

pub(crate) const AUTOMATED_PANNING_MACHINE: Research = Research {
    id: "automated_panning_machine",
    name: "Automated Gold Pan",
    cost: 17,
};

pub(crate) const REINFORCED_ALLOY: Research = Research {
    id: "reinforced_alloy",
    name: "Reinforced Alloy",
    cost: 22,
};

pub(crate) const SILICON_RESEARCH: Research = Research {
    id: "silicon",
    name: "Silicon Valley",
    cost: 12,
};

pub(crate) const SLIMEFUN_METALS: Research = Research {
    id: "slimefun_metals",
    name: "New Metals",
    cost: 6,
};

pub(crate) const SMELTERY: Research = Research {
    id: "smeltery",
    name: "Hot Smelting",
    cost: 10,
};

pub(crate) const STEEL: Research = Research {
    id: "steel",
    name: "Steel Age",
    cost: 11,
};

pub(crate) const BASIC_CIRCUIT_BOARD_RESEARCH: Research = Research {
    id: "basic_circuit_board",
    name: "Electric Work",
    cost: 8,
};

pub(crate) const ADVANCED_CIRCUIT_BOARD_RESEARCH: Research = Research {
    id: "advanced_circuit_board",
    name: "Advanced Electricity",
    cost: 9,
};

pub(crate) const AUTOMATIC_IGNITION_CHAMBER_RESEARCH: Research = Research {
    id: "automatic_ignition_chamber",
    name: "Automatic Ignition Chamber",
    cost: 12,
};

pub(crate) const BATTERY_RESEARCH: Research = Research {
    id: "battery",
    name: "Your first Battery",
    cost: 10,
};

pub(crate) const MAGNET_RESEARCH: Research = Research {
    id: "magnet",
    name: "Magnetic Metals",
    cost: 16,
};

pub(crate) const ELECTRIC_MOTOR_RESEARCH: Research = Research {
    id: "electric_motor",
    name: "Heating up",
    cost: 32,
};

pub(crate) const COPPER_WIRE_RESEARCH: Research = Research {
    id: "copper_wire",
    name: "Thinned-down Conductivity",
    cost: 15,
};

pub(crate) const SOLAR_PANEL_AND_HELMET_RESEARCH: Research = Research {
    id: "solar_panel_and_helmet",
    name: "Solar Power",
    cost: 17,
};

pub(crate) const REDSTONE_ALLOY_RESEARCH: Research = Research {
    id: "redstone_alloy",
    name: "Redstone Alloy",
    cost: 16,
};

pub(crate) const ENERGY_REGULATOR_RESEARCH: Research = Research {
    id: "energy_regulator",
    name: "Energy Networks 101",
    cost: 6,
};

pub(crate) const ENERGY_CONNECTORS_RESEARCH: Research = Research {
    id: "energy_connectors",
    name: "Wired Connections",
    cost: 12,
};

pub(crate) const CAPACITORS_RESEARCH: Research = Research {
    id: "capacitors",
    name: "Tier 1 Capacitors",
    cost: 25,
};

pub(crate) const SOLAR_GENERATORS_RESEARCH: Research = Research {
    id: "solar_generators",
    name: "Solar Power Plant",
    cost: 14,
};

pub(crate) const ELECTRIC_FURNACES_RESEARCH: Research = Research {
    id: "electric_furnaces",
    name: "Powered Furnace",
    cost: 15,
};
pub(crate) const BETTER_ELECTRIC_FURNACE_RESEARCH: Research = Research {
    id: "better_electric_furnace",
    name: "Upgraded Electric Furnace",
    cost: 20,
};
pub(crate) const GILDED_IRON_RESEARCH: Research = Research {
    id: "gilded_iron",
    name: "Shiny Iron",
    cost: 11,
};

pub(crate) const ELECTRIC_ORE_GRINDING_RESEARCH: Research = Research {
    id: "electric_ore_grinding",
    name: "Crushing and Grinding",
    cost: 20,
};

pub(crate) const MEDIUM_TIER_ELECTRIC_INGOT_MACHINES_RESEARCH: Research = Research {
    id: "medium_tier_electric_ingot_machines",
    name: "Fast Ingot Fabrication",
    cost: 25,
};

pub(crate) const HIGH_TIER_ELECTRIC_INGOT_MACHINES_RESEARCH: Research = Research {
    id: "high_tier_electric_ingot_machines",
    name: "Super Fast Ingot Fabrication",
    cost: 32,
};

pub(crate) const URANIUM_RESEARCH: Research = Research {
    id: "uranium",
    name: "Radioactive",
    cost: 30,
};

pub(crate) const HEATED_PRESSURE_CHAMBER_RESEARCH: Research = Research {
    id: "heated_pressure_chamber",
    name: "Heated Pressure Chamber",
    cost: 22,
};

pub(crate) const BETTER_HEATED_PRESSURE_CHAMBER_RESEARCH: Research = Research {
    id: "better_heated_pressure_chamber",
    name: "Upgraded Heated Pressure Chamber",
    cost: 20,
};

pub(crate) const BLISTERING_INGOTS_RESEARCH: Research = Research {
    id: "blistering_ingots",
    name: "Blistering Radioactivity",
    cost: 38,
};

#[cfg(test)]
mod resource_recipe_tests {
    use super::*;
    use mod_api::testing::FakeHost;

    #[test]
    fn seasonal_foods_keep_their_paper_ids_and_only_the_original_apple_pie_conflict() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                [
                    "minecraft:potion",
                    "minecraft:pumpkin_pie",
                    "minecraft:apple",
                ]
                .map(|id| mod_api::Identifier::parse(id).unwrap()),
            )
            .unwrap();
        let cases: &[(&str, &str, u32, &[(usize, &str)])] = &[
            (
                "CHRISTMAS_EGG_NOG",
                "potion",
                2,
                &[(0, "CHRISTMAS_MILK"), (1, "minecraft:egg")],
            ),
            (
                "CHRISTMAS_APPLE_CIDER",
                "potion",
                2,
                &[(0, "APPLE_JUICE"), (1, "minecraft:sugar")],
            ),
            (
                "CHRISTMAS_FRUIT_CAKE",
                "pumpkin_pie",
                4,
                &[
                    (0, "minecraft:egg"),
                    (1, "minecraft:apple"),
                    (2, "minecraft:melon"),
                    (3, "minecraft:sugar"),
                ],
            ),
            (
                "CHRISTMAS_APPLE_PIE",
                "pumpkin_pie",
                2,
                &[
                    (0, "minecraft:sugar"),
                    (1, "minecraft:apple"),
                    (2, "minecraft:egg"),
                ],
            ),
            (
                "CHRISTMAS_CAKE",
                "pumpkin_pie",
                4,
                &[
                    (0, "minecraft:egg"),
                    (1, "minecraft:sugar"),
                    (2, "WHEAT_FLOUR"),
                    (3, "minecraft:milk_bucket"),
                ],
            ),
            (
                "CHRISTMAS_CARAMEL_APPLE",
                "apple",
                2,
                &[
                    (1, "CHRISTMAS_CARAMEL"),
                    (4, "minecraft:apple"),
                    (7, "minecraft:stick"),
                ],
            ),
            (
                "CHRISTMAS_CHOCOLATE_APPLE",
                "apple",
                2,
                &[
                    (1, "minecraft:cocoa_beans"),
                    (4, "minecraft:apple"),
                    (7, "minecraft:stick"),
                ],
            ),
            (
                "CARROT_PIE",
                "pumpkin_pie",
                2,
                &[
                    (0, "minecraft:sugar"),
                    (1, "minecraft:carrot"),
                    (2, "minecraft:egg"),
                ],
            ),
            (
                "EASTER_APPLE_PIE",
                "pumpkin_pie",
                2,
                &[
                    (0, "minecraft:sugar"),
                    (1, "minecraft:apple"),
                    (2, "minecraft:egg"),
                ],
            ),
        ];
        for &(id, material, count, positions) in cases {
            let recipe = crafting()
                .find(|entry| entry.output.kind.id() == id)
                .unwrap();
            let slots: [Option<&str>; 9] = std::array::from_fn(|slot| {
                positions
                    .iter()
                    .find(|&&(pos, _)| pos == slot)
                    .map(|&(_, id)| id)
            });
            assert_eq!(
                recipe.inputs.map(|input| input.map(Kind::id)),
                slots,
                "{id}"
            );
            assert!(recipe.research.id.is_empty());
            assert_eq!(
                recipe.output.kind.material(),
                format!("minecraft:{material}")
            );
            assert_eq!(recipe.output.count, count);
            let result = recipe.output.kind.stack(&host, count).unwrap();
            assert_eq!(crate::items::item_id(&result).as_deref(), Some(id));
            let competing: Vec<_> = crafting()
                .filter(|entry| entry.inputs == recipe.inputs)
                .map(|entry| entry.output.kind.id())
                .collect();
            let expected: &[&str] = match id {
                "CHRISTMAS_APPLE_PIE" | "EASTER_APPLE_PIE" => {
                    &["CHRISTMAS_APPLE_PIE", "EASTER_APPLE_PIE"]
                }
                _ => std::slice::from_ref(&id),
            };
            assert_eq!(competing, expected, "{id}");
        }
    }

    #[test]
    fn paper_gold_block_and_apple_form_the_exact_enhanced_crafting_chain() {
        let block = crafting()
            .find(|recipe| recipe.output.kind.id() == "GOLD_24K_BLOCK")
            .unwrap();
        assert_eq!(block.inputs, [Some(GOLD_24K); 9]);
        assert_eq!(block.research.id, "24k_gold_block");
        assert_eq!(block.research.cost, 19);

        let apple = crafting()
            .find(|recipe| recipe.output.kind.id() == "minecraft:enchanted_golden_apple")
            .unwrap();
        for (slot, ingredient) in apple.inputs.iter().enumerate() {
            assert_eq!(*ingredient, if slot == 4 { Some(vanilla!("apple")) } else { GOLD_BLOCK });
        }
        assert!(apple.research.id.is_empty());
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(["minecraft:gold_block", "minecraft:enchanted_golden_apple"].map(|id|
                mod_api::Identifier::parse(id).unwrap()
            ))
            .unwrap();
        let vanilla_block = vanilla!("gold_block").stack(&host, 1).unwrap();
        assert!(!block.output.kind.matches(&host, &vanilla_block));
        let result = apple.output.kind.stack(&host, apple.output.count).unwrap();
        assert!(crate::items::item_id(&result).is_none());
    }

    #[test]
    fn synthetic_shell_uses_the_paper_altar_and_keeps_its_item_id() {
        let altar = crate::ancient_altar::CRAFTING
            .iter()
            .find(|recipe| recipe.output.kind.id() == "SYNTHETIC_SHULKER_SHELL")
            .unwrap();
        assert_eq!(altar.research.id, "shulker_shell");
        assert_eq!(altar.research.cost, 30);
        assert_eq!(altar.inputs[4], Some(vanilla!("turtle_helmet")));
        assert_eq!(altar.inputs[3], Some(REINFORCED_PLATE));
        assert_eq!(altar.inputs[5], Some(REINFORCED_PLATE));

        let mut host = FakeHost::new();
        host.registries
            .resolve_items([mod_api::Identifier::parse("minecraft:shulker_shell").unwrap()])
            .unwrap();
        let shell = altar.output.kind.stack(&host, altar.output.count).unwrap();
        assert!(crate::items::item_id(&shell).as_deref() == Some("SYNTHETIC_SHULKER_SHELL"));
    }
}
