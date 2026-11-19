// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, sync::LazyLock};

use crate::{
    ancient_altar, armor_forge,
    items::Kind,
    rainbow,
    recipes::{self, CraftingRecipe, Research, SingleRecipe, Stack},
    soulbound_gear,
};

#[derive(Clone, Copy)]
pub(super) struct RecipeType {
    pub material: &'static str,
    pub name: &'static str,
}

pub(super) const ENHANCED_CRAFTING: RecipeType = RecipeType {
    material: "minecraft:crafting_table",
    name: "Enhanced Crafting Table",
};
const MAGIC_WORKBENCH: RecipeType = RecipeType {
    material: "minecraft:bookshelf",
    name: "Magic Workbench",
};
const ARMOR_FORGE: RecipeType = RecipeType {
    material: "minecraft:anvil",
    name: "Armor Forge",
};
const ANCIENT_ALTAR: RecipeType = RecipeType {
    material: "minecraft:enchanting_table",
    name: "Ancient Altar",
};
const SMELTERY: RecipeType = RecipeType {
    material: "minecraft:furnace",
    name: "Smeltery",
};
const MULTIBLOCK: RecipeType = RecipeType {
    material: "minecraft:bricks",
    name: "MultiBlock",
};

#[derive(Clone, Copy)]
pub(super) struct Entry {
    pub item: Stack,
    pub label: Option<&'static str>,
    pub ingredients: [Option<Stack>; 9],
    pub recipe_type: RecipeType,
    pub research: Option<&'static Research>,
}

pub(super) struct Group {
    pub key: &'static str,
    pub name: &'static str,
    pub color: &'static str,
    pub icon: &'static str,
    pub requires_basics: bool,
}

macro_rules! group {
    ($key:literal, $name:literal, $icon:literal) => {
        group!($key, $name, $icon, "gray", false)
    };
    ($key:literal, $name:literal, $icon:literal, $color:literal, $locked:literal) => {
        Group {
            key: $key,
            name: $name,
            icon: concat!("minecraft:", $icon),
            color: $color,
            requires_basics: $locked,
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
    group!("magicalResources", "Magical Items", "firework_star"),
    group!("magicalArmor", "Magical Armor", "diamond_helmet"),
    group!("misc", "Miscellaneous", "player_head"),
    group!("armor", "Armor", "chainmail_chestplate"),
    group!("technicalComponents", "Technical Components", "blaze_rod"),
    group!("magicalGadgets", "Magical Gadgets", "elytra"),
    group!(
        "technicalGadgets",
        "Technical Gadgets",
        "leather_chestplate"
    ),
    group!("gps", "GPS-based Machines", "player_head", "aqua", true),
    group!(
        "electricity",
        "Energy and Electricity",
        "player_head",
        "aqua",
        true
    ),
    group!(
        "androids",
        "Programmable Androids",
        "player_head",
        "red",
        true
    ),
    group!("cargo", "Cargo Management", "player_head", "red", true),
];

static CATALOG: LazyLock<BTreeMap<String, Entry>> = LazyLock::new(build);

pub(super) fn lookup(id: &str) -> Option<&'static Entry> {
    CATALOG.get(&canonical(id))
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
                let recipe_type = RecipeType { material, name };
                entries
                    .entry(canonical(recipe.output.kind.id()))
                    .or_insert_with(|| processing(recipe_type, recipe));
            }
        }
    }
    multiblocks(&mut entries);
    entries
}
