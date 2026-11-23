// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use crate::{
    android_machine,
    android_program::{self, Tier},
    factory::reactors,
    fuel_generators::{self, TagFuels},
};

pub(super) fn has_fuels(id: &str) -> bool {
    fuel_generators::generator(id).is_some()
        || reactors::reactor(id).is_some()
        || android_program::mode(id).is_some()
        || matches!(id, "INDUSTRIAL_MINER" | "ADVANCED_INDUSTRIAL_MINER")
}

pub(super) fn entries(host: &dyn Host, id: &str, registered: Option<&TagFuels>) -> DisplayItems {
    if let Some(generator) = fuel_generators::generator(id) {
        let candidates = generator_inputs(host, id);
        let fallback = TagFuels::default();
        let tags = registered.unwrap_or(&fallback);
        return candidates
            .into_iter()
            .filter_map(|fuel| {
                let ticks = generator.fuel(host, tags, &fuel)?.ticks;
                Some(generator_lore(fuel, generator.production, ticks))
            })
            .collect();
    }
    if let Some(reactor) = reactors::reactor(id) {
        return reactor
            .fuels
            .iter()
            .map(|fuel| {
                generator_lore(
                    item(host, stack(fuel.kind, 1))?,
                    reactor.generation,
                    fuel.ticks,
                )
            })
            .collect();
    }
    if let Some(android) = android_program::mode(id) {
        return android_inputs(host, android.tier())
            .into_iter()
            .map(|fuel| {
                let seconds = android_machine::fuel_ticks(host, android.tier(), &fuel)?;
                with_lore(fuel, &[&format!("&8⇨ &7持续 {}", duration(seconds))])
            })
            .collect();
    }
    let advanced = id == "ADVANCED_INDUSTRIAL_MINER";
    let mut candidates = Vec::new();
    if advanced {
        candidates.extend(liquid_inputs(host));
    } else {
        candidates.extend(vanilla_inputs(
            host,
            &[
                "minecraft:coal",
                "minecraft:charcoal",
                "minecraft:coal_block",
                "minecraft:dried_kelp_block",
                "minecraft:blaze_rod",
            ],
        ));
        candidates.extend(kinds_in_tag(host, "minecraft:logs", 1));
    }
    candidates
        .into_iter()
        .map(|fuel| {
            let ores = crate::industrial_miner::fuel_value(host, &fuel, advanced)?;
            with_lore(fuel, &[&format!("&8⇨ &7最多可开采 {ores} 个矿石")])
        })
        .collect()
}

fn generator_lore(fuel: ItemStack, rate: u32, ticks: u32) -> Option<ItemStack> {
    with_lore(
        fuel,
        &[
            &format!("&8⇨ &7持续 {}", duration(ticks / 2)),
            &format!("&8⇨ &e⚡ &7{} J/s", rate * 2),
            &format!(
                "&8⇨ &e⚡ &7总计 {} J",
                compact_energy(u64::from(rate) * u64::from(ticks))
            ),
        ],
    )
}

fn duration(seconds: u32) -> String {
    if seconds >= 60 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{seconds}s")
    }
}

fn compact_energy(joules: u64) -> String {
    let (scale, suffix) = match joules {
        0..1000 => return joules.to_string(),
        1000..1_000_000 => (1000, 'K'),
        _ => (1_000_000, 'M'),
    };
    let value = format!("{:.2}", joules as f64 / f64::from(scale));
    format!(
        "{}{suffix}",
        value.trim_end_matches('0').trim_end_matches('.')
    )
}

fn vanilla_inputs(host: &dyn Host, materials: &[&'static str]) -> Vec<ItemStack> {
    materials
        .iter()
        .filter_map(|id| item(host, vanilla(id, 1)))
        .collect()
}

fn from_kinds(host: &dyn Host, materials: &[Kind]) -> Vec<ItemStack> {
    materials
        .iter()
        .filter_map(|&kind| item(host, stack(kind, 1)))
        .collect()
}

fn liquid_inputs(host: &dyn Host) -> Vec<ItemStack> {
    from_kinds(
        host,
        &[
            Kind::Vanilla("minecraft:lava_bucket"),
            crate::heated_pressure::BUCKET_OF_OIL,
            fuel_generators::BUCKET_OF_FUEL,
        ],
    )
}

fn android_inputs(host: &dyn Host, tier: Tier) -> Vec<ItemStack> {
    match tier {
        Tier::Solid => {
            let mut candidates = vanilla_inputs(
                host,
                &[
                    "minecraft:coal_block",
                    "minecraft:blaze_rod",
                    "minecraft:dried_kelp_block",
                    "minecraft:coal",
                    "minecraft:charcoal",
                    "minecraft:bamboo",
                ],
            );
            for tag in ["minecraft:logs", "minecraft:planks"] {
                candidates.extend(kinds_in_tag(host, tag, 1));
            }
            candidates
        }
        Tier::Liquid => liquid_inputs(host),
        Tier::Nuclear => from_kinds(
            host,
            &[
                crate::heated_pressure::URANIUM,
                reactors::NEPTUNIUM,
                crate::heated_pressure::BOOSTED_URANIUM,
            ],
        ),
    }
}

fn generator_inputs(host: &dyn Host, id: &str) -> Vec<ItemStack> {
    match id {
        "COAL_GENERATOR" | "COAL_GENERATOR_2" => coal_inputs(host),
        "LAVA_GENERATOR" | "LAVA_GENERATOR_2" => vanilla_inputs(host, &["minecraft:lava_bucket"]),
        "MAGNESIUM_GENERATOR" => from_kinds(host, &[crate::heated_pressure::MAGNESIUM_SALT]),
        "COMBUSTION_REACTOR" => from_kinds(
            host,
            &[
                crate::heated_pressure::BUCKET_OF_OIL,
                fuel_generators::BUCKET_OF_FUEL,
            ],
        ),
        "BIO_REACTOR" => bio_inputs(host),
        _ => Vec::new(),
    }
}

fn coal_inputs(host: &dyn Host) -> Vec<ItemStack> {
    let mut fuels = vanilla_inputs(
        host,
        &[
            "minecraft:coal_block",
            "minecraft:blaze_rod",
            "minecraft:dried_kelp_block",
        ],
    );
    let tags = host.tags();
    let boats = Identifier::parse("minecraft:boats").expect("built-in tag");
    if let Some(boats) = tags.key(&boats).and_then(|tag| tags.items(tag)) {
        fuels.extend(boats.into_iter().filter_map(|boat| ItemStack::new(boat, 1)));
    }
    fuels.extend(vanilla_inputs(
        host,
        &["minecraft:coal", "minecraft:charcoal"],
    ));
    for &(tag, _) in fuel_generators::COAL_BLOCK_TAGS {
        fuels.extend(kinds_in_tag(host, tag, 1));
    }
    fuels
}

fn bio_inputs(host: &dyn Host) -> Vec<ItemStack> {
    // BioGenerator interleaves material tag groups with fixed materials.
    let mut fuels = vanilla_inputs(
        host,
        &[
            "minecraft:rotten_flesh",
            "minecraft:spider_eye",
            "minecraft:bone",
            "minecraft:string",
            "minecraft:apple",
            "minecraft:melon_slice",
            "minecraft:melon",
            "minecraft:pumpkin",
            "minecraft:pumpkin_seeds",
            "minecraft:melon_seeds",
            "minecraft:wheat",
            "minecraft:wheat_seeds",
            "minecraft:carrot",
            "minecraft:potato",
            "minecraft:sugar_cane",
            "minecraft:nether_wart",
            "minecraft:red_mushroom",
            "minecraft:brown_mushroom",
            "minecraft:vine",
            "minecraft:cactus",
            "minecraft:lily_pad",
            "minecraft:chorus_fruit",
            "minecraft:kelp",
            "minecraft:dried_kelp",
            "minecraft:dried_kelp_block",
            "minecraft:seagrass",
            "minecraft:sea_pickle",
            "minecraft:bamboo",
            "minecraft:sweet_berries",
            "minecraft:cocoa_beans",
            "minecraft:beetroot",
            "minecraft:beetroot_seeds",
        ],
    );
    fuels.extend(kinds_in_tag(host, "minecraft:small_flowers", 1));
    fuels.extend(vanilla_inputs(
        host,
        &[
            "minecraft:honeycomb",
            "minecraft:honeycomb_block",
            "minecraft:shroomlight",
            "minecraft:crimson_fungus",
            "minecraft:warped_fungus",
        ],
    ));
    fuels.extend(from_kinds(host, &[fuel_generators::STRANGE_NETHER_GOO]));
    fuels.extend(vanilla_inputs(
        host,
        &[
            "minecraft:glow_berries",
            "minecraft:small_dripleaf",
            "minecraft:big_dripleaf",
            "minecraft:glow_lichen",
            "minecraft:spore_blossom",
        ],
    ));
    for &(tag, _) in &fuel_generators::BIO_BLOCK_TAGS[1..] {
        fuels.extend(kinds_in_tag(host, tag, 1));
    }
    fuels
}
