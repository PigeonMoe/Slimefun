// SPDX-License-Identifier: GPL-3.0-only

use super::*;

use mod_api::Compound;

use crate::{
    advanced_processing::{LAVA_INPUTS, WATER_BUCKET},
    climbing_pick, geo_deposits,
    gold_pan::{PANS, Pan, Weights},
    guide::localized_plain_item_name,
    hand_machine,
    hologram_projector::colored_lore,
    machine::Settings,
};

pub(super) fn entries(host: &dyn Host, id: &str, settings: Settings) -> Vec<Option<ItemStack>> {
    match id {
        "GOLD_PAN" | "NETHER_GOLD_PAN" => pan_entries(
            host,
            PANS.iter().filter(|pan| pan.item.id() == id),
            &settings.pan_weights,
        ),
        "AUTOMATED_PANNING_MACHINE"
        | "ELECTRIC_GOLD_PAN"
        | "ELECTRIC_GOLD_PAN_2"
        | "ELECTRIC_GOLD_PAN_3" => pan_entries(host, PANS, &settings.pan_weights),
        "CLIMBING_PICK" => climbing_entries(host),
        "GEO_MINER" => geo_entries(host),
        "TABLE_SAW" => saw_entries(host),
        "CRUCIBLE" => crucible_entries(host),
        _ => Vec::new(),
    }
}

fn pan_entries<'a>(
    host: &dyn Host,
    pans: impl IntoIterator<Item = &'a Pan>,
    weights: &Weights,
) -> Vec<Option<ItemStack>> {
    let mut items = Vec::new();
    for pan in pans {
        for (_, drop) in pan.weighted_drops(weights) {
            for &(material, _) in pan.targets {
                pair(
                    host,
                    &mut items,
                    vanilla(material, 1),
                    Stack {
                        kind: drop,
                        count: 1,
                    },
                );
            }
        }
    }
    items
}

fn climbing_entries(host: &dyn Host) -> Vec<Option<ItemStack>> {
    host.registry_entries("minecraft:item")
        .into_iter()
        .flatten()
        .filter(|id| climbing_pick::default_power(&id.to_string()).is_some())
        .filter_map(|id| ItemStack::new(host.item_key(&id)?, 1))
        .map(Some)
        .collect()
}

fn geo_entries(host: &dyn Host) -> Vec<Option<ItemStack>> {
    geo_deposits::registered_miner_resources()
        .filter_map(|resource| {
            let kind = resource.item();
            let mut stack = item(host, Stack { kind, count: 1 })?;
            let name =
                localized_plain_item_name(kind.id()).unwrap_or_else(|| resource.name().into());
            stack
                .apply_components(&Compound::from([(
                    "minecraft:custom_name".into(),
                    colored_lore(&name),
                )]))
                .ok()?;
            Some(Some(stack))
        })
        .collect()
}

fn saw_entries(host: &dyn Host) -> Vec<Option<ItemStack>> {
    ["minecraft:logs", "minecraft:planks"]
        .into_iter()
        .flat_map(|tag| kinds_in_tag(host, tag, 1))
        .filter_map(|input| {
            let output = hand_machine::saw_result(host, &input.item().id(host))?;
            Some([Some(input), Some(output)])
        })
        .flatten()
        .collect()
}

fn crucible_entries(host: &dyn Host) -> Vec<Option<ItemStack>> {
    const BASE_LAVA_ORDER: [&str; 5] = [
        "minecraft:cobblestone",
        "minecraft:netherrack",
        "minecraft:stone",
        "minecraft:obsidian",
        "minecraft:terracotta",
    ];
    let lava = vanilla("minecraft:lava_bucket", 1);
    let mut items = Vec::new();
    for material in BASE_LAVA_ORDER {
        if let Some(recipe) = LAVA_INPUTS
            .iter()
            .find(|recipe| recipe.input.kind.id() == material)
        {
            pair(host, &mut items, recipe.input, lava);
        }
    }

    for leaves in kinds_in_tag(host, "minecraft:leaves", 16) {
        let water = item(
            host,
            Stack {
                kind: WATER_BUCKET,
                count: 1,
            },
        );
        items.extend([Some(leaves), water]);
    }

    // Paper's terracotta tag includes the uncolored block already displayed above.
    for recipe in LAVA_INPUTS
        .iter()
        .filter(|recipe| recipe.input.kind.id().ends_with("terracotta"))
    {
        pair(host, &mut items, recipe.input, lava);
    }
    for recipe in LAVA_INPUTS.iter().filter(|recipe| {
        let material = recipe.input.kind.id();
        !BASE_LAVA_ORDER.contains(&material) && !material.ends_with("terracotta")
    }) {
        pair(host, &mut items, recipe.input, lava);
    }
    items
}
