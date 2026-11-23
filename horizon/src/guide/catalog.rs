// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, sync::LazyLock};

use crate::{
    advanced_processing, ancient_altar, armor_forge,
    factory::reactors,
    food_machines, fuel_generators, geology, heated_pressure, industrial_miner,
    items::{self, Kind},
    nether_goo, rainbow,
    recipes::{self, CraftingRecipe, Research, SingleRecipe, Stack},
    soulbound_gear, spawner_repair, thermal_machines,
};

#[derive(Clone, Copy)]
pub(super) struct RecipeType {
    pub icon: Kind,
    pub name: &'static str,
    pub lore: &'static [&'static str],
    pub locale_key: Option<&'static str>,
}

impl RecipeType {
    const fn local(mut self, key: &'static str) -> Self {
        self.locale_key = Some(key);
        self
    }

    const fn plain(material: &'static str, name: &'static str) -> Self {
        Self::device(Kind::Vanilla(material), name)
    }

    const fn device(icon: Kind, name: &'static str) -> Self {
        Self {
            icon,
            name,
            lore: &[],
            locale_key: None,
        }
    }
}

pub(super) const ENHANCED_CRAFTING: RecipeType =
    RecipeType::plain("minecraft:crafting_table", "增强型工作台").local("enhanced_crafting_table");
const MAGIC_WORKBENCH: RecipeType = RecipeType::plain("minecraft:crafting_table", "魔法工作台").local("magic_workbench");
const ARMOR_FORGE: RecipeType = RecipeType::plain("minecraft:anvil", "盔甲锻造台").local("armor_forge");
const ANCIENT_ALTAR: RecipeType = RecipeType::plain("minecraft:enchanting_table", "古代祭坛").local("ancient_altar");
const SMELTERY: RecipeType = RecipeType::plain("minecraft:furnace", "冶炼炉").local("smeltery");
const MULTIBLOCK: RecipeType = RecipeType::plain("minecraft:bricks", "多方块结构").local("multiblock");

const WASHER: RecipeType = RecipeType {
    lore: &["", "&a&o使用洗矿机清洗"],
    ..RecipeType::plain("minecraft:cauldron", "洗矿机").local("ore_washer")
};
const GOLD_PAN: RecipeType = RecipeType {
    lore: &["", "&a&o对砂砾使用淘金盘即可获得"],
    ..RecipeType::device(items::GOLD_PAN, "淘金盘").local("gold_pan")
};
const MOB_DROP: RecipeType = RecipeType {
    lore: &["", "&r击杀指定生物即可获得"],
    ..RecipeType::plain("minecraft:iron_sword", "生物掉落").local("mob_drop")
};
const BARTER_DROP: RecipeType = RecipeType {
    locale_key: Some("barter_drop"),
    lore: &[
        "&7Barter with Piglins using",
        "&7Gold Ingots to obtain this Item",
    ],
    ..RecipeType::plain("minecraft:gold_ingot", "Piglin Bartering Drop")
};
const FOOD_FABRICATOR: RecipeType =
    RecipeType::device(food_machines::FOOD_FABRICATOR, "食物加工机").local("food_fabricator");
const FOOD_COMPOSTER: RecipeType =
    RecipeType::device(food_machines::FOOD_COMPOSTER, "食物堆肥机").local("food_composter");

const HEATED_PRESSURE: RecipeType =
    RecipeType::device(heated_pressure::HEATED_PRESSURE_CHAMBER, "加热压力舱").local("heated_pressure_chamber");
const FREEZER: RecipeType = RecipeType::device(thermal_machines::FREEZER, "冰箱").local("freezer");
const GEO_MINER: RecipeType = RecipeType::device(geology::GEO_MINER, "自然资源开采机").local("geo_miner");
const OIL_PUMP: RecipeType = RecipeType::device(geology::OIL_PUMP, "原油泵").local("oil_pump");
const REFINERY: RecipeType = RecipeType::device(advanced_processing::REFINERY, "炼油机").local("refinery");
const NUCLEAR_REACTOR: RecipeType = RecipeType::device(reactors::NUCLEAR_REACTOR, "核反应堆").local("nuclear_reactor");
const CONTAINMENT: RecipeType =
    RecipeType::device(spawner_repair::PICKAXE_OF_CONTAINMENT, "刷怪笼之镐").local("pickaxe_of_containment");

const NETHER_ICE_RESEARCH: Research = Research {
    id: "nether_ice",
    name: "Nether Ice Coolant",
    cost: 45,
};
const BOOSTED_URANIUM_RESEARCH: Research = Research {
    id: "boosted_uranium",
    name: "Never-Ending Circle",
    cost: 30,
};
const REACTOR_ESSENTIALS: Research = Research {
    id: "reactor_essentials",
    name: "Reactor Essentials",
    cost: 36,
};

const IRON_GOLEM: &[IngredientVisual] = &[IngredientVisual {
    slot: 4,
    label: "&a铁傀儡",
    profile: Some(items::HeadTexture {
        profile: [669471756, -955559616, -1944180456, -1177071508],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvODkwOTFkNzllYTBmNTllZjdlZjk0ZDdiYmE2ZTVmMTdmMmY3ZDQ1NzJjNDRmOTBmNzZjNDgxOWE3MTQifX19",
    }),
}];

const PIGLIN: &[IngredientVisual] = &[IngredientVisual {
    slot: 4,
    label: "&f猪灵",
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
        label: "活塞（朝上）",
        profile: None,
    },
    IngredientVisual {
        slot: 5,
        label: "活塞（朝上）",
        profile: None,
    },
];

#[derive(Clone, Copy)]
pub(super) struct Entry {
    pub id: &'static str,
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
    group!("weapons", "武器", "golden_sword"),
    group!("usefulItems", "实用物品", "player_head"),
    group!("basicMachines", "基础机器", "crafting_table"),
    group!("tools", "工具", "diamond_pickaxe"),
    group!("resources", "资源", "lapis_lazuli"),
    group!("food", "食物", "cookie"),
    group!(
        "birthday",
        "原作者生日 &7（10 月 26 日）",
        "firework_rocket",
        "green",
        None
    )
    .seasonal(10),
    group!(
        "christmas",
        "圣诞节 &7（12 月）",
        "player_head",
        "green",
        None
    )
    .seasonal(12),
    group!(
        "halloween",
        "万圣节 &7（10 月 31 日）",
        "jack_o_lantern",
        "gold",
        None
    )
    .seasonal(10),
    group!("magicalResources", "魔法物品", "firework_star"),
    group!("magicalArmor", "魔法防具", "diamond_helmet"),
    group!("misc", "杂项用品", "player_head"),
    group!("armor", "防具", "chainmail_chestplate"),
    group!("technicalComponents", "科技零件", "blaze_rod"),
    group!("easter", "复活节 &7（4 月）", "player_head", "gold", None).seasonal(4),
    group!(
        "valentinesDay",
        "情人节 &7（2 月 14 日）",
        "player_head",
        "light_purple",
        None
    )
    .seasonal(2),
    group!("magicalGadgets", "魔法道具", "elytra"),
    group!(
        "technicalGadgets",
        "科技工具",
        "leather_chestplate"
    ),
    group!("talismans", "护身符 &a（一级）", "emerald"),
    group!(
        "ender_talismans",
        "护身符 &a（二级）",
        "emerald",
        "gray",
        Some("talismans")
    ),
    group!(
        "gps",
        "定位设备",
        "player_head",
        "aqua",
        Some("basicMachines")
    ),
    group!(
        "electricity",
        "能源与电力",
        "player_head",
        "aqua",
        Some("basicMachines")
    ),
    group!(
        "androids",
        "可编程机器人",
        "player_head",
        "red",
        Some("basicMachines")
    ),
    group!(
        "cargo",
        "货物管理",
        "player_head",
        "red",
        Some("basicMachines")
    ),
];

static CATALOG: LazyLock<BTreeMap<String, Entry>> = LazyLock::new(build);

pub(super) fn lookup(id: &str) -> Option<&'static Entry> {
    CATALOG.get(&canonical(id))
}

pub(super) fn has_stock_item_id(id: &str) -> bool {
    CATALOG.get(id).is_some_and(|entry| entry.id == id) || listed_stock_item(id)
}

pub(super) fn blocks_vanilla_station(id: &str) -> bool {
    if !id.bytes().all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == b'_') {
        return false;
    }
    if let Some(entry) = CATALOG.get(id) {
        return entry.label.is_some() || matches!(entry.item.kind, Kind::Slimefun(_));
    }
    listed_stock_item(id)
}

fn listed_stock_item(id: &str) -> bool {
    crate::rainbow::item(id).is_some()
        || include_str!("stock.tsv").lines().any(|line| {
            line.split_once('\t')
                .is_some_and(|(_, items)| items.split_ascii_whitespace().any(|item| item == id))
        })
}

pub(super) fn items_for_research(id: &str) -> impl Iterator<Item = &'static str> {
    CATALOG.values().filter_map(move |entry| {
        entry
            .research
            .filter(|topic| topic.id == id)
            .map(|_| entry.id)
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
            "增强型工作台",
            "minecraft:crafting_table",
            &[(4, "minecraft:crafting_table"), (7, "minecraft:dispenser")][..],
            None,
        ),
        (
            "GRIND_STONE",
            "磨石",
            "minecraft:dispenser",
            &[(4, "minecraft:oak_fence"), (7, "minecraft:dispenser")],
            Some(&GRIND_STONE),
        ),
        (
            "ARMOR_FORGE",
            "盔甲锻造台",
            "minecraft:anvil",
            &[(4, "minecraft:anvil"), (7, "minecraft:dispenser")],
            Some(&armor_forge::FORGE_RESEARCH),
        ),
        (
            "ORE_CRUSHER",
            "矿石粉碎机",
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
            "压缩机",
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
            "简易冶炼炉",
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
            "冶炼炉",
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
            "压力舱",
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
            "魔法工作台",
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
            "洗矿机",
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
            "台锯",
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
            "自动淘金机",
            "minecraft:bowl",
            &[(4, "minecraft:oak_trapdoor"), (7, "minecraft:cauldron")],
            Some(&recipes::AUTOMATED_PANNING_MACHINE),
        ),
        (
            "INDUSTRIAL_MINER",
            "工业采矿机",
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
            "高级工业采矿机",
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
            "榨汁机",
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
                id,
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

fn workbench(id: &'static str, recipe_type: RecipeType, recipe: &CraftingRecipe) -> Entry {
    Entry {
        id,
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
        id: crate::availability::paper_id(recipe.output.kind),
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
        let id = crate::availability::paper_id(recipe.output.kind);
        entries
            .entry(canonical(id))
            .or_insert_with(|| workbench(id, recipe_type, recipe));
    }
    for r in recipes::ALLOY_RECIPES.iter().chain(recipes::smelting()) {
        let mut ingredients = [None; 9];
        for (target, source) in ingredients.iter_mut().zip(r.inputs) {
            *target = Some(*source);
        }
        entries
            .entry(canonical(r.output.kind.id()))
            .or_insert(Entry {
                id: crate::availability::paper_id(r.output.kind),
                item: r.output,
                label: None,
                ingredients,
                visuals: &[],
                recipe_type: SMELTERY,
                research: recipes::output_research(r.output.kind),
            });
    }
    for (material, name, key, recipes) in [
        ("minecraft:dispenser", "磨石", "grind_stone", recipes::GRINDING),
        ("minecraft:dispenser", "矿石粉碎机", "ore_crusher", recipes::CRUSHING),
        (
            "minecraft:piston",
            "压缩机",
            "compressor", recipes::COMPRESSING,
        ),
        ("minecraft:glass", "压力舱", "pressure_chamber", recipes::PRESSING),
        ("minecraft:glass_bottle", "榨汁机", "juicer", recipes::JUICING),
        ("minecraft:cauldron", "洗矿机", "ore_washer", recipes::WASHING),
    ] {
        for recipe in recipes {
            if matches!(recipe.output.kind, Kind::Slimefun(_)) {
                let recipe_type = RecipeType::plain(material, name).local(key);
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
                id: crate::availability::paper_id(output),
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

    let magnesium = fuel_generators::RESEARCHES
        .iter()
        .find(|topic| topic.id == "magnesium_generator");
    for recipe in heated_pressure::RECIPES {
        let (first_slot, research) = match recipe.output.kind {
            heated_pressure::PLASTIC_SHEET => (4, Some(&heated_pressure::PLASTIC_RESEARCH)),
            heated_pressure::BOOSTED_URANIUM => (0, Some(&BOOSTED_URANIUM_RESEARCH)),
            heated_pressure::ENRICHED_NETHER_ICE | heated_pressure::NETHER_ICE_COOLANT_CELL => {
                (0, Some(&NETHER_ICE_RESEARCH))
            }
            heated_pressure::MAGNESIUM_SALT => (0, magnesium),
            heated_pressure::BLISTERING_INGOT
            | heated_pressure::BLISTERING_INGOT_2
            | crate::network_recipes::BLISTERING_INGOT_3 => {
                (0, Some(&recipes::BLISTERING_INGOTS_RESEARCH))
            }
            _ => continue,
        };
        let ingredients: Vec<_> = recipe
            .inputs
            .iter()
            .enumerate()
            .map(|(index, input)| (first_slot + index, input.kind))
            .collect();
        add(
            HEATED_PRESSURE,
            recipe.output.kind,
            recipe.output.count,
            &ingredients,
            research,
            &[],
        );
    }

    for (method, output, inputs, research) in [
        (
            OIL_PUMP,
            heated_pressure::BUCKET_OF_OIL,
            &[(4, Kind::Vanilla("minecraft:bucket"))][..],
            &geology::OIL,
        ),
        (
            REFINERY,
            fuel_generators::BUCKET_OF_FUEL,
            &[(4, heated_pressure::BUCKET_OF_OIL)],
            &advanced_processing::FUEL,
        ),
        (
            GEO_MINER,
            heated_pressure::NETHER_ICE,
            &[],
            &NETHER_ICE_RESEARCH,
        ),
        (
            FREEZER,
            thermal_machines::REACTOR_COLLANT_CELL,
            &[(0, Kind::Vanilla("minecraft:blue_ice"))],
            &REACTOR_ESSENTIALS,
        ),
        (
            NUCLEAR_REACTOR,
            reactors::NEPTUNIUM,
            &[(0, heated_pressure::URANIUM)],
            &REACTOR_ESSENTIALS,
        ),
        (
            NUCLEAR_REACTOR,
            heated_pressure::PLUTONIUM,
            &[(0, reactors::NEPTUNIUM)],
            &REACTOR_ESSENTIALS,
        ),
        (
            CONTAINMENT,
            spawner_repair::BROKEN_SPAWNER,
            &[(4, Kind::Vanilla("minecraft:spawner"))],
            &spawner_repair::CONTAINMENT,
        ),
    ] {
        add(method, output, 1, inputs, Some(research), &[]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paper_lists_every_stock_guide_item_except_its_two_hidden_items() {
        let missing: Vec<_> = include_str!("stock.tsv")
            .lines()
            .filter_map(|line| line.split_once('\t'))
            .flat_map(|(_, ids)| ids.split_ascii_whitespace())
            .filter(|id| lookup(id).is_none())
            .collect();
        assert_eq!(missing, ["RESTORED_BACKPACK", "FILLED_FLASK_OF_KNOWLEDGE"]);
    }

    #[test]
    fn stock_byproducts_keep_the_paper_source_slots_counts_and_research() {
        for (id, method, output, topic, ingredients) in [
            (
                "PLASTIC_SHEET",
                HEATED_PRESSURE,
                8,
                "plastic_sheet",
                &[(4, "BUCKET_OF_OIL")][..],
            ),
            (
                "MAGNESIUM_SALT",
                HEATED_PRESSURE,
                1,
                "magnesium_generator",
                &[(0, "MAGNESIUM_DUST"), (1, "SALT")],
            ),
            (
                "NETHER_ICE_COOLANT_CELL",
                HEATED_PRESSURE,
                8,
                "nether_ice",
                &[(0, "ENRICHED_NETHER_ICE")],
            ),
            ("NETHER_ICE", GEO_MINER, 1, "nether_ice", &[]),
            (
                "REACTOR_COLLANT_CELL",
                FREEZER,
                1,
                "reactor_essentials",
                &[(0, "minecraft:blue_ice")],
            ),
            (
                "PLUTONIUM",
                NUCLEAR_REACTOR,
                1,
                "reactor_essentials",
                &[(0, "NEPTUNIUM")],
            ),
            (
                "BROKEN_SPAWNER",
                CONTAINMENT,
                1,
                "pickaxe_of_containment",
                &[(4, "minecraft:spawner")],
            ),
        ] {
            let entry = lookup(id).unwrap();
            assert_eq!(entry.id, id);
            assert_eq!(entry.recipe_type.icon, method.icon);
            assert_eq!(entry.item.count, output);
            assert_eq!(entry.research.unwrap().id, topic);
            let actual: Vec<_> = entry
                .ingredients
                .iter()
                .enumerate()
                .filter_map(|(slot, item)| item.map(|item| (slot, item.kind.id())))
                .collect();
            assert_eq!(actual, ingredients);
            assert!(crate::guide::locale::item_name(id).is_some());
        }
    }
}
