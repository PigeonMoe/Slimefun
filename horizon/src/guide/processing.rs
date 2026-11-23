// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{Compound, Host, Identifier, ItemStack, Tag};

use crate::{
    advanced_processing::{self, Family},
    factory,
    hologram_projector::colored_lore,
    items::{self, Kind},
    machine::Settings,
    network_runtime::MachineRecipes,
    recipes::{self, SingleRecipe, Stack},
    thermal_machines,
};

mod fuel;
mod resources;

type DisplayItems = Vec<Option<ItemStack>>;

pub(super) fn label(id: &str, language: super::Language) -> &'static str {
    let (key, chinese) = match id {
        "GEO_MINER" => ("miner", "此采矿机可以获得的资源"),
        "GOLD_PAN" | "NETHER_GOLD_PAN" => ("gold-pan", "你可以获得的资源"),
        "CLIMBING_PICK" => ("climbing-pick", "你可以攀爬的平面"),
        _ if fuel::has_fuels(id) => ("generator", "可用的燃料类型"),
        _ => ("machine", "此机器可用的合成配方"),
    };
    if language == super::Language::CHINESE {
        chinese
    } else {
        language.message(&format!("tooltips.recipes.{key}"))
    }
}

pub(super) fn entries(
    host: &dyn Host,
    id: &str,
    settings: Settings,
    registered: Option<&MachineRecipes>,
) -> DisplayItems {
    if let Some(machine) = advanced_processing::machine(id) {
        match machine.family {
            Family::CarbonPress | Family::Freezer | Family::AutoDrier => {
                return thermal(host, machine, settings);
            }
            Family::Smeltery => {
                return if enabled("SMELTERY") {
                    smeltery(host, true)
                } else {
                    Vec::new()
                };
            }
            Family::FoodComposter => {
                let mut items = Vec::new();
                for crop in crate::food_machines::PRODUCE {
                    pair(
                        host,
                        &mut items,
                        stack(crop.food, 1),
                        stack(crop.fertilizer, 2),
                    );
                }
                return items;
            }
            Family::Refinery => {
                return single(
                    host,
                    stack(crate::heated_pressure::BUCKET_OF_OIL, 1),
                    stack(crate::fuel_generators::BUCKET_OF_FUEL, 1),
                );
            }
            // AContainer shows only recipes with one input. These machines have custom
            // findNextRecipe implementations or exclusively have multiple inputs.
            _ => {}
        }
    }
    match id {
        "GRIND_STONE" => singles(host, enabled_singles(recipes::GRINDING.iter().copied())),
        "ORE_CRUSHER" => singles(host, ore_crushing(settings)),
        "COMPRESSOR" => singles(host, enabled_singles(recipes::COMPRESSING.iter().copied())),
        "JUICER" => singles(host, enabled_singles(recipes::JUICING.iter().copied())),
        "PRESSURE_CHAMBER" => singles(host, enabled_singles(recipes::PRESSING.iter().copied())),
        "ORE_WASHER" => ore_washer(host),
        "COMPOSTER" => composter(host),
        "SMELTERY" => smeltery(host, false),
        "MAKESHIFT_SMELTERY" if enabled("SMELTERY") => singles(host, ingot_factory()),
        "ELECTRIC_PRESS" | "ELECTRIC_PRESS_2" => singles(
            host,
            crate::electric_processing::PRESSING
                .iter()
                .map(|recipe| recipe.recipe),
        ),
        "ELECTRIC_INGOT_FACTORY" | "ELECTRIC_INGOT_FACTORY_2" | "ELECTRIC_INGOT_FACTORY_3"
            if enabled("SMELTERY") =>
        {
            singles(host, ingot_factory())
        }
        "ELECTRIC_INGOT_PULVERIZER" => {
            let mut items = singles(host, factory::ingot_pulverizer_recipes(false));
            if enabled("SMELTERY") {
                items.extend(singles(
                    host,
                    ingot_factory().map(|recipe| SingleRecipe {
                        input: recipe.output,
                        output: recipe.input,
                    }),
                ));
            }
            items
        }
        "ELECTRIC_ORE_GRINDER" | "ELECTRIC_ORE_GRINDER_2" | "ELECTRIC_ORE_GRINDER_3" => {
            let mut source = Vec::new();
            if enabled("GRIND_STONE") {
                source.extend(enabled_singles(recipes::GRINDING.iter().copied()));
            }
            if enabled("ORE_CRUSHER") {
                source.extend(ore_crushing(settings));
            }
            if !settings.legacy_ore_grinder {
                source.sort_by_key(|recipe| std::cmp::Reverse(recipe.input.count));
            }
            singles(host, source)
        }
        "ELECTRIC_FURNACE" | "ELECTRIC_FURNACE_2" | "ELECTRIC_FURNACE_3" => registered
            .into_iter()
            .flat_map(|recipes| recipes.furnaces.display_recipes())
            .flat_map(|(input, result)| [ItemStack::new(input, 1), Some(result.clone())])
            .collect(),
        "HEATED_PRESSURE_CHAMBER" | "HEATED_PRESSURE_CHAMBER_2" => singles(
            host,
            crate::heated_pressure::RECIPES.iter().filter_map(|recipe| {
                let [input] = *recipe.inputs else { return None };
                Some(SingleRecipe {
                    input,
                    output: recipe.output,
                })
            }),
        ),
        "OIL_PUMP" => single(
            host,
            vanilla("minecraft:bucket", 1),
            stack(crate::heated_pressure::BUCKET_OF_OIL, 1),
        ),
        "PRODUCE_COLLECTOR" => produce_collector(host),
        _ if fuel::has_fuels(id) => {
            fuel::entries(host, id, registered.map(|recipes| &recipes.fuels))
        }
        _ => resources::entries(host, id, settings),
    }
}

const fn stack(kind: Kind, count: u32) -> Stack {
    Stack { kind, count }
}

const fn vanilla(id: &'static str, count: u32) -> Stack {
    stack(Kind::Vanilla(id), count)
}

fn item(host: &dyn Host, stack: Stack) -> Option<ItemStack> {
    stack.kind.stack(host, stack.count)
}

fn pair(host: &dyn Host, items: &mut DisplayItems, input: Stack, output: Stack) {
    items.extend([item(host, input), item(host, output)]);
}

fn single(host: &dyn Host, input: Stack, output: Stack) -> DisplayItems {
    vec![item(host, input), item(host, output)]
}

fn singles(host: &dyn Host, recipes: impl IntoIterator<Item = SingleRecipe>) -> DisplayItems {
    recipes
        .into_iter()
        .flat_map(|recipe| [item(host, recipe.input), item(host, recipe.output)])
        .collect()
}

fn kinds_in_tag(host: &dyn Host, tag: &str, count: u32) -> Vec<ItemStack> {
    let tags = host.tags();
    let Some(tag) = Identifier::parse(tag).ok().and_then(|id| tags.key(&id)) else {
        return Vec::new();
    };
    tags.blocks(tag)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|block| host.item_key(&Identifier::parse(&block.id(host)).ok()?))
        .filter_map(|item| ItemStack::new(item, count))
        .collect()
}

fn with_lore(mut item: ItemStack, lore: &[&str]) -> Option<ItemStack> {
    item.apply_components(&Compound::from([(
        "minecraft:lore".into(),
        Tag::List(lore.iter().map(|line| colored_lore(line)).collect()),
    )]))
    .ok()?;
    Some(item)
}

fn enabled(id: &str) -> bool {
    crate::availability::globally_enabled(id)
}

fn enabled_singles(
    recipes: impl Iterator<Item = SingleRecipe>,
) -> impl Iterator<Item = SingleRecipe> {
    recipes.filter(|recipe| {
        [recipe.input, recipe.output]
            .into_iter()
            .all(|stack| match stack.kind {
                Kind::Slimefun(_) => enabled(stack.kind.id()),
                _ => true,
            })
    })
}

fn thermal(
    host: &dyn Host,
    machine: &advanced_processing::Machine,
    settings: Settings,
) -> DisplayItems {
    let use_vanilla_ratios = machine.family == Family::Freezer
        && settings.freezer_use_vanilla_ratios[(machine.speed - 1) as usize];
    let mut items = Vec::new();
    for recipe in thermal_machines::recipes(machine.family, use_vanilla_ratios) {
        let [input] = *recipe.inputs else { continue };
        if let Some(&output) = recipe.outputs.last() {
            pair(host, &mut items, input, output);
        }
    }
    if machine.family == Family::AutoDrier {
        for (tag, count) in [("minecraft:saplings", 2), ("minecraft:leaves", 1)] {
            for input in kinds_in_tag(host, tag, 1) {
                items.extend([Some(input), item(host, vanilla("minecraft:stick", count))]);
            }
        }
    }
    items
}

fn ore_crushing(settings: Settings) -> impl Iterator<Item = SingleRecipe> {
    let multiplying = || {
        recipes::DOUBLING.iter().map(move |recipe| SingleRecipe {
            input: recipe.input,
            output: Stack {
                count: recipe.output.count * (1 + u32::from(settings.double_ores)),
                ..recipe.output
            },
        })
    };
    // OreCrusher loads its default and tag-era rows before adding SlimefunItemSetup recipes.
    enabled_singles(
        recipes::CRUSHING[..18]
            .iter()
            .copied()
            .chain(multiplying().take(8))
            .chain(recipes::CRUSHING[18..21].iter().copied())
            .chain(multiplying().skip(8))
            .chain(recipes::CRUSHING[21..].iter().copied()),
    )
}

fn ore_washer(host: &dyn Host) -> DisplayItems {
    let sifted = stack(items::SIFTED_ORE, 1);
    let mut items = vec![];
    for dust in [items::IRON_DUST, items::GOLD_DUST] {
        if enabled(dust.id()) {
            pair(host, &mut items, sifted, stack(dust, 1));
        }
    }
    if enabled(items::SALT.id()) {
        pair(
            host,
            &mut items,
            vanilla("minecraft:sand", 1),
            stack(items::SALT, 1),
        );
    }
    // SlimefunItemSetup registers the potential dusts and stone chunk as menu recipes.
    for result in [
        items::COPPER_DUST,
        items::TIN_DUST,
        items::SILVER_DUST,
        items::LEAD_DUST,
        items::ALUMINUM_DUST,
        items::ZINC_DUST,
        items::MAGNESIUM_DUST,
        items::STONE_CHUNK,
    ] {
        if enabled(result.id()) {
            pair(host, &mut items, sifted, stack(result, 1));
        }
    }
    items.extend(singles(
        host,
        enabled_singles(recipes::WASHING.iter().copied()),
    ));
    items
}

fn composter(host: &dyn Host) -> DisplayItems {
    let mut items = Vec::new();
    for tag in ["minecraft:leaves", "minecraft:saplings"] {
        for input in kinds_in_tag(host, tag, 8) {
            items.extend([Some(input), item(host, vanilla("minecraft:dirt", 1))]);
        }
    }
    for (material, count, result) in crate::composter::MATERIAL_RECIPES {
        pair(
            host,
            &mut items,
            vanilla(material, *count),
            vanilla(result, 1),
        );
    }
    items
}

fn ingot_factory() -> impl Iterator<Item = SingleRecipe> {
    // Smeltery's default iron recipe precedes the dust recipes added by SlimefunItemSetup.
    const PAPER_ORDER: &[&str] = &[
        "minecraft:iron_ingot",
        "COPPER_INGOT",
        "TIN_INGOT",
        "SILVER_INGOT",
        "LEAD_INGOT",
        "ALUMINUM_INGOT",
        "ZINC_INGOT",
        "MAGNESIUM_INGOT",
        "GOLD_4K",
    ];
    enabled_singles(PAPER_ORDER.iter().filter_map(|id| {
        recipes::smelting()
            .find(|recipe| recipe.output.kind.id() == *id)
            .map(|recipe| SingleRecipe {
                input: recipe.inputs[0],
                output: recipe.output,
            })
    }))
}

fn smeltery(host: &dyn Host, electric: bool) -> DisplayItems {
    let other = enabled_singles(advanced_processing::smeltery_recipes().iter().filter_map(
        |recipe| {
            let [input] = *recipe.inputs else { return None };
            Some(SingleRecipe {
                input,
                output: recipe.output,
            })
        },
    ));
    if electric {
        singles(host, other)
    } else {
        let mut source: Vec<_> = ingot_factory().collect();
        // The vanilla quartz recipe is registered after the gold carats.
        source.extend(other);
        singles(host, source)
    }
}

fn produce_collector(host: &dyn Host) -> DisplayItems {
    let mut items = Vec::new();
    for (container, animal, product) in [
        (
            "minecraft:bucket",
            "&f附近需要有&b牛",
            "minecraft:milk_bucket",
        ),
        (
            "minecraft:bucket",
            "&f附近需要有&b山羊",
            "minecraft:milk_bucket",
        ),
        (
            "minecraft:bowl",
            "&f附近需要有&b哞菇",
            "minecraft:mushroom_stew",
        ),
    ] {
        items.extend([
            item(host, vanilla(container, 1)).and_then(|item| with_lore(item, &[animal])),
            item(host, vanilla(product, 1)),
        ]);
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::testing::FakeHost;

    #[test]
    fn freezer_uses_configured_ice_ratios_and_displays_the_frozen_result() {
        mod_api::__private::initialize().unwrap();
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                [
                    "minecraft:ice",
                    "minecraft:packed_ice",
                    "minecraft:blue_ice",
                    "minecraft:water_bucket",
                    "minecraft:lava_bucket",
                    "minecraft:obsidian",
                    "minecraft:bucket",
                    "minecraft:player_head",
                    "minecraft:snow_block",
                    "minecraft:magma_cream",
                    "minecraft:magma_block",
                    "minecraft:slime_ball",
                    "minecraft:slime_block",
                ]
                .map(|id| Identifier::parse(id).unwrap()),
            )
            .unwrap();
        let settings = Settings {
            freezer_use_vanilla_ratios: [false, true, false],
            ..Settings::default()
        };
        let standard = entries(&host, "FREEZER", settings, None);
        let vanilla = entries(&host, "FREEZER_2", settings, None);
        assert_eq!(standard.len(), 16);
        assert_eq!(standard[0].as_ref().unwrap().count(), 1);
        assert_eq!(vanilla[0].as_ref().unwrap().count(), 9);
        assert_eq!(
            vanilla[4].as_ref().unwrap().item().id(&host),
            "minecraft:water_bucket"
        );
        assert_eq!(
            vanilla[5].as_ref().unwrap().item().id(&host),
            "minecraft:ice"
        );
        assert_eq!(
            vanilla[7].as_ref().unwrap().item().id(&host),
            "minecraft:obsidian"
        );
    }
}
