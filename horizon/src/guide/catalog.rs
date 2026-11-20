// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, sync::LazyLock};

use crate::{
    ancient_altar, armor_forge, food_machines, fuel_generators, industrial_miner,
    items::{self, Kind},
    nether_goo, rainbow,
    recipes::{self, CraftingRecipe, Research, SingleRecipe, Stack},
    soulbound_gear,
};

#[derive(Clone, Copy)]
pub(super) struct RecipeType {
    pub icon: Kind,
    pub name: &'static str,
    pub lore: &'static [&'static str],
}

impl RecipeType {
    const fn plain(material: &'static str, name: &'static str) -> Self {
        Self {
            icon: Kind::Vanilla(material),
            name,
            lore: &[],
        }
    }
}

pub(super) const ENHANCED_CRAFTING: RecipeType =
    RecipeType::plain("minecraft:crafting_table", "Enhanced Crafting Table");
const MAGIC_WORKBENCH: RecipeType = RecipeType::plain("minecraft:bookshelf", "Magic Workbench");
const ARMOR_FORGE: RecipeType = RecipeType::plain("minecraft:anvil", "Armor Forge");
const ANCIENT_ALTAR: RecipeType = RecipeType::plain("minecraft:enchanting_table", "Ancient Altar");
const SMELTERY: RecipeType = RecipeType::plain("minecraft:furnace", "Smeltery");
const MULTIBLOCK: RecipeType = RecipeType::plain("minecraft:bricks", "MultiBlock");

const WASHER: RecipeType = RecipeType {
    lore: &["", "&a&oWash it in an Ore Washer"],
    ..RecipeType::plain("minecraft:cauldron", "Ore Washer")
};
const GOLD_PAN: RecipeType = RecipeType {
    icon: items::GOLD_PAN,
    name: "Gold Pan",
    lore: &["", "&a&oUse a Gold Pan on Gravel to obtain this Item"],
};
const MOB_DROP: RecipeType = RecipeType {
    lore: &["", "&rKill the specified Mob to obtain this Item"],
    ..RecipeType::plain("minecraft:iron_sword", "Mob Drop")
};
const BARTER_DROP: RecipeType = RecipeType {
    lore: &[
        "",
        "&aBarter with piglins for a chance",
        "&ato obtain this item",
    ],
    ..RecipeType::plain("minecraft:gold_ingot", "Barter Drop")
};
const FOOD_FABRICATOR: RecipeType = RecipeType {
    icon: food_machines::FOOD_FABRICATOR,
    name: "Food Fabricator",
    lore: &[],
};
const FOOD_COMPOSTER: RecipeType = RecipeType {
    icon: food_machines::FOOD_COMPOSTER,
    name: "Food Composter",
    lore: &[],
};

const IRON_GOLEM: &[IngredientVisual] = &[IngredientVisual {
    slot: 4,
    label: "&aIron Golem",
    profile: Some(items::HeadTexture {
        profile: [669471756, -955559616, -1944180456, -1177071508],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvODkwOTFkNzllYTBmNTllZjdlZjk0ZDdiYmE2ZTVmMTdmMmY3ZDQ1NzJjNDRmOTBmNzZjNDgxOWE3MTQifX19",
    }),
}];

const PIGLIN: &[IngredientVisual] = &[IngredientVisual {
    slot: 4,
    label: "&fPiglin",
    profile: Some(items::HeadTexture {
        profile: [1031950828, 1086148763, -1932514438, -1171656326],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvMjg4MmFmMTI5NGE3NDAyM2U2OTE5YTMxZDFhMDI3MzEwZjJlMTQyYWZiNDY2N2QyMzBkMTU1ZTdmMjFkYmI0MSJ9fX0=",
    }),
}];

pub(super) struct IngredientVisual {
    pub slot: usize,
    pub label: &'static str,
    pub profile: Option<items::HeadTexture>,
}

const FACED_PISTONS: &[IngredientVisual] = &[
    IngredientVisual {
        slot: 3,
        label: "Piston (facing up)",
        profile: None,
    },
    IngredientVisual {
        slot: 5,
        label: "Piston (facing up)",
        profile: None,
    },
];

#[derive(Clone, Copy)]
pub(super) struct Entry {
    pub item: Stack,
    pub label: Option<&'static str>,
    pub ingredients: [Option<Stack>; 9],
    pub visuals: &'static [IngredientVisual],
    pub recipe_type: RecipeType,
    pub research: Option<&'static Research>,
}

pub(super) struct Group {
    pub key: &'static str,
    pub name: &'static str,
    pub color: &'static str,
    pub icon: &'static str,
    pub parent: Option<&'static str>,
    pub month: Option<i32>,
}

impl Group {
    const fn seasonal(mut self, month: i32) -> Self {
        self.month = Some(month);
        self
    }

    pub(super) fn in_season(&self, month: i32) -> bool {
        self.month.is_none_or(|expected| expected == month)
    }
}

macro_rules! group {
    ($key:literal, $name:literal, $icon:literal) => {
        group!($key, $name, $icon, "gray", None)
    };
    ($key:literal, $name:literal, $icon:literal, $color:literal, $parent:expr) => {
        Group {
            key: $key,
            name: $name,
            icon: concat!("minecraft:", $icon),
            color: $color,
            parent: $parent,
            month: None,
        }
    };
}

// ItemGroup sorts by the configured tier, then preserves first registration order.
pub(super) const GROUPS: &[Group] = &[
    group!("weapons", "Weapons", "golden_sword"),
    group!("usefulItems", "Useful Items", "player_head"),
    group!("basicMachines", "Basic Machines", "crafting_table"),
    group!("tools", "Tools", "diamond_pickaxe"),
    group!("resources", "Resources", "lapis_lazuli"),
    group!("food", "Food", "cookie"),
    group!(
        "birthday",
        "TheBusyBiscuit's Birthday &7(26th October)",
        "firework_rocket",
        "green",
        None
    )
    .seasonal(10),
    group!(
        "christmas",
        "Christmas &7(December only)",
        "player_head",
        "green",
        None
    )
    .seasonal(12),
    group!(
        "halloween",
        "Halloween &7(31st October)",
        "jack_o_lantern",
        "gold",
        None
    )
    .seasonal(10),
    group!("magicalResources", "Magical Items", "firework_star"),
    group!("magicalArmor", "Magical Armor", "diamond_helmet"),
    group!("misc", "Miscellaneous", "player_head"),
    group!("armor", "Armor", "chainmail_chestplate"),
    group!("technicalComponents", "Technical Components", "blaze_rod"),
    group!("easter", "Easter &7(April)", "player_head", "gold", None).seasonal(4),
    group!(
        "valentinesDay",
        "Valentine's Day &7(14th February)",
        "player_head",
        "light_purple",
        None
    )
    .seasonal(2),
    group!("magicalGadgets", "Magical Gadgets", "elytra"),
    group!(
        "technicalGadgets",
        "Technical Gadgets",
        "leather_chestplate"
    ),
    group!("talismans", "Talismans - &aTier I", "emerald"),
    group!(
        "ender_talismans",
        "Talismans - &aTier II",
        "emerald",
        "gray",
        Some("talismans")
    ),
    group!(
        "gps",
        "GPS-based Machines",
        "player_head",
        "aqua",
        Some("basicMachines")
    ),
    group!(
        "electricity",
        "Energy and Electricity",
        "player_head",
        "aqua",
        Some("basicMachines")
    ),
    group!(
        "androids",
        "Programmable Androids",
        "player_head",
        "red",
        Some("basicMachines")
    ),
    group!(
        "cargo",
        "Cargo Management",
        "player_head",
        "red",
        Some("basicMachines")
    ),
];

static CATALOG: LazyLock<BTreeMap<String, Entry>> = LazyLock::new(build);

pub(super) fn lookup(id: &str) -> Option<&'static Entry> {
    CATALOG.get(&canonical(id))
}

pub(super) fn blocks_vanilla_station(id: &str) -> bool {
    if !id.bytes().all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == b'_') {
        return false;
    }
    if let Some(entry) = CATALOG.get(id) {
        return entry.label.is_some() || matches!(entry.item.kind, Kind::Slimefun(_));
    }
    crate::rainbow::item(id).is_some()
        || include_str!("stock.tsv").lines().any(|line| {
            line.split_once('\t')
                .is_some_and(|(_, items)| items.split_ascii_whitespace().any(|item| item == id))
        })
}

pub(super) fn research_topics() -> impl Iterator<Item = &'static Research> {
    CATALOG.values().filter_map(|item| item.research)
}

pub(super) fn research_topic(id: &str) -> Option<&'static Research> {
    research_topics().find(|topic| topic.id == id)
}

pub(super) fn group_entries(group: &Group) -> impl Iterator<Item = &'static Entry> {
    named_group_entries(group).map(|(_, entry)| entry)
}

pub(super) fn named_group_entries(
    group: &Group,
) -> impl Iterator<Item = (&'static str, &'static Entry)> {
    include_str!("stock.tsv")
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .find(|(key, _)| *key == group.key)
        .into_iter()
        .flat_map(|(_, ids)| ids.split_ascii_whitespace())
        .filter_map(|id| Some((id, lookup(id)?)))
}

const GRIND_STONE: Research = Research {
    id: "grind_stone",
    name: "Grind Stone",
    cost: 4,
};
const ORE_CRUSHER: Research = Research {
    id: "ore_crusher",
    name: "Ore Doubling",
    cost: 6,
};
const COMPRESSOR: Research = Research {
    id: "compressor_and_carbon",
    name: "Carbon Creation",
    cost: 9,
};
const MAGIC: Research = Research {
    id: "magic_workbench",
    name: "Magic Workbench",
    cost: 12,
};

// Exact arrangement displayed by each Paper MultiBlockMachine. The material
// index is the same row-major recipe index that Paper puts into the guide.
fn multiblocks(entries: &mut BTreeMap<String, Entry>) {
    for (id, name, icon, ingredients, research) in [
        (
            "ENHANCED_CRAFTING_TABLE",
            "Enhanced Crafting Table",
            "minecraft:crafting_table",
            &[(4, "minecraft:crafting_table"), (7, "minecraft:dispenser")][..],
            None,
        ),
        (
            "GRIND_STONE",
            "Grind Stone",
            "minecraft:dispenser",
            &[(4, "minecraft:oak_fence"), (7, "minecraft:dispenser")],
            Some(&GRIND_STONE),
        ),
        (
            "ARMOR_FORGE",
            "Armor Forge",
            "minecraft:anvil",
            &[(4, "minecraft:anvil"), (7, "minecraft:dispenser")],
            Some(&armor_forge::FORGE_RESEARCH),
        ),
        (
            "ORE_CRUSHER",
            "Ore Crusher",
            "minecraft:dispenser",
            &[
                (4, "minecraft:nether_brick_fence"),
                (6, "minecraft:iron_bars"),
                (7, "minecraft:dispenser"),
                (8, "minecraft:iron_bars"),
            ],
            Some(&ORE_CRUSHER),
        ),
        (
            "COMPRESSOR",
            "Compressor",
            "minecraft:piston",
            &[
                (4, "minecraft:nether_brick_fence"),
                (6, "minecraft:piston"),
                (7, "minecraft:dispenser"),
                (8, "minecraft:piston"),
            ],
            Some(&COMPRESSOR),
        ),
        (
            "MAKESHIFT_SMELTERY",
            "Makeshift Smeltery",
            "minecraft:blast_furnace",
            &[
                (1, "minecraft:oak_fence"),
                (3, "minecraft:bricks"),
                (4, "minecraft:dispenser"),
                (5, "minecraft:bricks"),
                (7, "minecraft:flint_and_steel"),
            ],
            Some(&recipes::MAKESHIFT_SMELTERY),
        ),
        (
            "SMELTERY",
            "Smeltery",
            "minecraft:furnace",
            &[
                (1, "minecraft:nether_brick_fence"),
                (3, "minecraft:nether_bricks"),
                (4, "minecraft:dispenser"),
                (5, "minecraft:nether_bricks"),
                (7, "minecraft:flint_and_steel"),
            ],
            Some(&recipes::SMELTERY),
        ),
        (
            "PRESSURE_CHAMBER",
            "Pressure Chamber",
            "minecraft:glass",
            &[
                (0, "minecraft:smooth_stone_slab"),
                (1, "minecraft:dispenser"),
                (2, "minecraft:smooth_stone_slab"),
                (3, "minecraft:piston"),
                (4, "minecraft:glass"),
                (5, "minecraft:piston"),
                (6, "minecraft:piston"),
                (7, "minecraft:cauldron"),
                (8, "minecraft:piston"),
            ],
            Some(&recipes::PRESSURE_CHAMBER),
        ),
        (
            "MAGIC_WORKBENCH",
            "Magic Workbench",
            "minecraft:crafting_table",
            &[
                (6, "minecraft:bookshelf"),
                (7, "minecraft:crafting_table"),
                (8, "minecraft:dispenser"),
            ],
            Some(&MAGIC),
        ),
        (
            "ORE_WASHER",
            "Ore Washer",
            "minecraft:cauldron",
            &[
                (1, "minecraft:dispenser"),
                (4, "minecraft:oak_fence"),
                (7, "minecraft:cauldron"),
            ],
            Some(&recipes::ORE_WASHER),
        ),
        (
            "TABLE_SAW",
            "Table Saw",
            "minecraft:stonecutter",
            &[
                (3, "minecraft:smooth_stone_slab"),
                (4, "minecraft:stonecutter"),
                (5, "minecraft:smooth_stone_slab"),
                (7, "minecraft:iron_block"),
            ],
            Some(&recipes::TABLE_SAW),
        ),
        (
            "AUTOMATED_PANNING_MACHINE",
            "Automated Panning Machine",
            "minecraft:bowl",
            &[(4, "minecraft:oak_trapdoor"), (7, "minecraft:cauldron")],
            Some(&recipes::AUTOMATED_PANNING_MACHINE),
        ),
        (
            "INDUSTRIAL_MINER",
            "Industrial Miner",
            "minecraft:golden_pickaxe",
            &[
                (3, "minecraft:piston"),
                (4, "minecraft:chest"),
                (5, "minecraft:piston"),
                (6, "minecraft:iron_block"),
                (7, "minecraft:blast_furnace"),
                (8, "minecraft:iron_block"),
            ],
            Some(&industrial_miner::RESEARCH),
        ),
        (
            "ADVANCED_INDUSTRIAL_MINER",
            "Advanced Industrial Miner",
            "minecraft:diamond_pickaxe",
            &[
                (3, "minecraft:piston"),
                (4, "minecraft:chest"),
                (5, "minecraft:piston"),
                (6, "minecraft:diamond_block"),
                (7, "minecraft:blast_furnace"),
                (8, "minecraft:diamond_block"),
            ],
            Some(&industrial_miner::ADVANCED_RESEARCH),
        ),
        (
            "JUICER",
            "Juicer",
            "minecraft:glass_bottle",
            &[
                (1, "minecraft:glass"),
                (4, "minecraft:nether_brick_fence"),
                (7, "minecraft:dispenser"),
            ],
            Some(&recipes::JUICER),
        ),
    ] {
        let mut stacks = [None; 9];
        for &(slot, material) in ingredients {
            stacks[slot] = Some(stack_of(Kind::Vanilla(material)));
        }
        entries.insert(
            id.to_owned(),
            Entry {
                item: stack_of(Kind::Vanilla(icon)),
                label: Some(name),
                ingredients: stacks,
                visuals: if matches!(id, "INDUSTRIAL_MINER" | "ADVANCED_INDUSTRIAL_MINER") {
                    FACED_PISTONS
                } else {
                    &[]
                },
                recipe_type: MULTIBLOCK,
                research,
            },
        );
    }
}

fn canonical(id: &str) -> String {
    id.strip_prefix("minecraft:")
        .unwrap_or(id)
        .to_ascii_uppercase()
}

fn stack_of(kind: Kind) -> Stack {
    Stack { kind, count: 1 }
}

fn workbench(recipe_type: RecipeType, recipe: &CraftingRecipe) -> Entry {
    Entry {
        item: recipe.output,
        label: None,
        ingredients: recipe.inputs.map(|entry| entry.map(stack_of)),
        visuals: &[],
        recipe_type,
        research: (!recipe.research.id.is_empty()).then_some(recipe.research),
    }
}

fn processing(recipe_type: RecipeType, recipe: &SingleRecipe) -> Entry {
    let mut ingredients = [None; 9];
    ingredients[0] = Some(recipe.input);
    Entry {
        item: recipe.output,
        label: None,
        ingredients,
        visuals: &[],
        recipe_type,
        research: recipes::output_research(recipe.output.kind),
    }
}

fn build() -> BTreeMap<String, Entry> {
    let mut entries = BTreeMap::new();
    let benches = armor_forge::RECIPES
        .iter()
        .map(|r| (ARMOR_FORGE, r))
        .chain(recipes::magic_crafting().map(|r| (MAGIC_WORKBENCH, r)))
        .chain(
            ancient_altar::CRAFTING
                .iter()
                .chain(armor_forge::ALTAR_RECIPES)
                .chain(rainbow::ALTAR_RECIPES)
                .chain(soulbound_gear::ALTAR_RECIPES)
                .map(|r| (ANCIENT_ALTAR, r)),
        )
        .chain(recipes::crafting().map(|r| (ENHANCED_CRAFTING, r)));
    for (recipe_type, recipe) in benches {
        let id = if recipe_type.name == ARMOR_FORGE.name {
            armor_forge::paper_id(recipe.output.kind)
        } else {
            recipe.output.kind.id()
        };
        entries
            .entry(canonical(id))
            .or_insert_with(|| workbench(recipe_type, recipe));
    }
    for r in recipes::ALLOY_RECIPES.iter().chain(recipes::smelting()) {
        let mut ingredients = [None; 9];
        for (target, source) in ingredients.iter_mut().zip(r.inputs) {
            *target = Some(*source);
        }
        entries
            .entry(canonical(r.output.kind.id()))
            .or_insert(Entry {
                item: r.output,
                label: None,
                ingredients,
                visuals: &[],
                recipe_type: SMELTERY,
                research: recipes::output_research(r.output.kind),
            });
    }
    for (material, name, recipes) in [
        ("minecraft:grindstone", "Grind Stone", recipes::GRINDING),
        ("minecraft:iron_bars", "Ore Crusher", recipes::CRUSHING),
        (
            "minecraft:nether_brick_fence",
            "Compressor",
            recipes::COMPRESSING,
        ),
        ("minecraft:cauldron", "Pressure Chamber", recipes::PRESSING),
        ("minecraft:glass_bottle", "Juicer", recipes::JUICING),
        ("minecraft:cauldron", "Ore Washer", recipes::WASHING),
    ] {
        for recipe in recipes {
            if matches!(recipe.output.kind, Kind::Slimefun(_)) {
                let recipe_type = RecipeType::plain(material, name);
                entries
                    .entry(canonical(recipe.output.kind.id()))
                    .or_insert_with(|| processing(recipe_type, recipe));
            }
        }
    }
    multiblocks(&mut entries);
    stock_sources(&mut entries);
    entries
}

fn stock_sources(entries: &mut BTreeMap<String, Entry>) {
    let mut add = |recipe_type,
                   output: Kind,
                   count,
                   inputs: &[(usize, Kind)],
                   research: Option<&'static Research>,
                   visuals: &'static [IngredientVisual]| {
        let mut ingredients = [None; 9];
        for &(slot, kind) in inputs {
            ingredients[slot] = Some(stack_of(kind));
        }
        entries.insert(
            canonical(output.id()),
            Entry {
                item: Stack {
                    kind: output,
                    count,
                },
                label: None,
                ingredients,
                visuals,
                recipe_type,
                research,
            },
        );
    };

    add(
        GOLD_PAN,
        items::SIFTED_ORE,
        1,
        &[(0, Kind::Vanilla("minecraft:gravel"))],
        Some(&recipes::ORE_WASHER),
        &[],
    );
    add(
        MOB_DROP,
        items::BASIC_CIRCUIT_BOARD,
        1,
        &[(4, Kind::Vanilla("minecraft:player_head"))],
        Some(&recipes::BASIC_CIRCUIT_BOARD_RESEARCH),
        IRON_GOLEM,
    );

    add(
        BARTER_DROP,
        fuel_generators::STRANGE_NETHER_GOO,
        1,
        &[(4, Kind::Vanilla("minecraft:player_head"))],
        Some(&nether_goo::RESEARCH),
        PIGLIN,
    );

    // Paper's iron and gold entries are registered to the Ore Crusher. Copper
    // and the six Slimefun-only metals list the Ore Washer's sifted ore route.
    for &dust in recipes::WASHER_DUSTS {
        if !matches!(dust, items::IRON_DUST | items::GOLD_DUST) {
            add(WASHER, dust, 1, &[(0, items::SIFTED_ORE)], None, &[]);
        }
    }

    let food = food_machines::RESEARCHES
        .iter()
        .find(|topic| topic.id == "organic_food");
    let fertilizer = food_machines::RESEARCHES
        .iter()
        .find(|topic| topic.id == "organic_fertilizer");
    for produce in food_machines::PRODUCE {
        add(
            FOOD_FABRICATOR,
            produce.food,
            2,
            &[(0, food_machines::CAN), (1, produce.crop)],
            food,
            &[],
        );
        add(
            FOOD_COMPOSTER,
            produce.fertilizer,
            2,
            &[(0, produce.food)],
            fertilizer,
            &[],
        );
    }
}
