// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{Host, Identifier, ItemKey, ItemStack, LevelRef, ServerHost, SoundSource};

use crate::{
    android_machine::{self, Position},
    android_program::Tier,
    random::Rolls,
};

const FISH_WEIGHT: u32 = 25;
const LOOT: &[(&str, u32)] = &[
    ("minecraft:bone", 10),
    ("minecraft:string", 10),
    ("minecraft:ink_sac", 8),
    ("minecraft:kelp", 6),
    ("minecraft:stick", 5),
    ("minecraft:rotten_flesh", 3),
    ("minecraft:leather", 2),
    ("minecraft:bamboo", 3),
    ("minecraft:saddle", 1),
    ("minecraft:name_tag", 1),
    ("minecraft:nautilus_shell", 1),
];

pub(crate) fn fish(
    host: &mut dyn ServerHost,
    level: LevelRef,
    position: Position,
    tier: Tier,
    rolls: &Rolls,
) -> Option<()> {
    let [x, y, z] = position;
    let below = [x, y.checked_sub(1)?, z];
    if level.block_state(host, below)?.block().id(host) != "minecraft:water" {
        return None;
    }

    let _ = level.play_sound(
        host,
        "minecraft:entity.player.splash",
        below.map(f64::from),
        SoundSource::Blocks,
        0.3,
        0.7,
    );
    if rolls.next(host, level, 100)? >= 10 * tier.number() {
        return None;
    }

    let fish_tag = Identifier::parse("minecraft:fishes").ok()?;
    let fish_tag = host.tags().key(&fish_tag)?;
    let fish = host.tags().items(fish_tag)?;
    let fish_end = u32::try_from(fish.len()).ok()?.checked_mul(FISH_WEIGHT)?;
    let roll = rolls.next(
        host,
        level,
        fish_end.checked_add(LOOT.iter().map(|(_, weight)| weight).sum())?,
    )?;
    let key = if roll < fish_end {
        fish.get((roll / FISH_WEIGHT) as usize).copied()?
    } else {
        junk_or_treasure(host, roll - fish_end)?
    };
    let mut machine = android_machine::data(host, level, position)?;
    if android_machine::push_one(host, &mut machine, ItemStack::new(key, 1)?) {
        android_machine::save(host, position, machine);
    }
    Some(())
}

fn material_at(mut roll: u32) -> Option<&'static str> {
    for &(material, weight) in LOOT {
        if roll < weight {
            return Some(material);
        }
        roll -= weight;
    }
    None
}

fn junk_or_treasure(host: &dyn Host, roll: u32) -> Option<ItemKey> {
    host.item_key(&Identifier::parse(material_at(roll)?).ok()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fisherman_uses_the_paper_tiers_and_exact_weight_boundaries() {
        assert_eq!(
            [Tier::Solid, Tier::Liquid, Tier::Nuclear].map(|tier| 10 * tier.number()),
            [10, 20, 30]
        );
        assert_eq!(
            FISH_WEIGHT * 4 + LOOT.iter().map(|(_, weight)| weight).sum::<u32>(),
            150
        );
        for &(material, weight) in LOOT {
            assert_eq!(
                (0..50)
                    .filter(|&roll| material_at(roll) == Some(material))
                    .count(),
                weight as usize
            );
        }
        assert_eq!(material_at(50), None);
    }
}
