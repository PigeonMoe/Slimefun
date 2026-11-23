// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{Host, ItemStack, LevelRef, PlayerRef, ServerHost, SoundSource, Text};

use crate::items::{GOLD_PAN, Kind, NETHER_GOLD_PAN};
use crate::random::Rolls;
use crate::recipes::{
    GOLD_PAN_RESEARCH, NETHER_GOLD_PAN_RESEARCH, NETHER_PAN_DROPS, PAN_DROPS, Research,
};
use crate::research;

pub(crate) struct Pan {
    pub item: Kind,
    pub research: &'static Research,
    pub targets: &'static [(&'static str, &'static str)],
    drops: &'static [(u32, Kind)],
}

pub(crate) const PANS: &[Pan] = &[
    Pan {
        item: GOLD_PAN,
        research: &GOLD_PAN_RESEARCH,
        targets: &[("minecraft:gravel", "minecraft:block.gravel.break")],
        drops: PAN_DROPS,
    },
    Pan {
        item: NETHER_GOLD_PAN,
        research: &NETHER_GOLD_PAN_RESEARCH,
        targets: &[
            ("minecraft:soul_sand", "minecraft:block.soul_sand.break"),
            ("minecraft:soul_soil", "minecraft:block.soul_soil.break"),
        ],
        drops: NETHER_PAN_DROPS,
    },
];

pub(crate) fn held(host: &dyn Host, stack: &ItemStack) -> Option<&'static Pan> {
    PANS.iter().find(|pan| pan.item.matches(host, stack))
}

const OVERWORLD_KEYS: [&str; 4] = ["FLINT", "CLAY", "SIFTED_ORE", "IRON_NUGGET"];
const NETHER_KEYS: [&str; 6] = [
    "QUARTZ",
    "GOLD_NUGGET",
    "NETHER_WART",
    "BLAZE_POWDER",
    "GLOWSTONE_DUST",
    "GHAST_TEAR",
];
// Dough samples with Random.nextFloat, which uses 24 bits of entropy.
const SAMPLE_COUNT: u32 = 1 << 24;

#[derive(Clone, Copy)]
pub(crate) struct Weights {
    overworld: [u32; OVERWORLD_KEYS.len()],
    nether: [u32; NETHER_KEYS.len()],
}

impl Default for Weights {
    fn default() -> Self {
        Self {
            overworld: std::array::from_fn(|index| PAN_DROPS[index].0),
            nether: std::array::from_fn(|index| NETHER_PAN_DROPS[index].0),
        }
    }
}

impl Weights {
    pub(crate) fn read_config(&mut self, items: Option<&toml::Value>) {
        let defaults = Self::default();
        let chance = |id| items.and_then(|items| items.get(id)?.get("chance"));
        apply_weights(
            &mut self.overworld,
            defaults.overworld,
            OVERWORLD_KEYS,
            chance("GOLD_PAN"),
        );
        apply_weights(
            &mut self.nether,
            defaults.nether,
            NETHER_KEYS,
            chance("NETHER_GOLD_PAN"),
        );
    }
}

fn apply_weights<const COUNT: usize>(
    weights: &mut [u32; COUNT],
    defaults: [u32; COUNT],
    keys: [&str; COUNT],
    config: Option<&toml::Value>,
) {
    for (index, key) in keys.into_iter().enumerate() {
        match config
            .and_then(|chance| chance.get(key)?.as_integer())
            .and_then(|n| i32::try_from(n).ok())
        {
            Some(n) if n >= 0 => weights[index] = n as u32,
            Some(_) => {} // Paper retains the previous value for a negative integer.
            None => weights[index] = defaults[index],
        }
    }
}

impl Pan {
    pub(crate) fn can_use(&self, host: &mut dyn ServerHost, player: PlayerRef) -> bool {
        if research::can_use(host, player, self.research) {
            return true;
        }
        let text = format!(
            "Unlock {} with /sf research {}.",
            self.research.name, self.research.id,
        );
        let _ = player.tell(host, Text::literal(text));
        false
    }

    pub(crate) fn allows_entity_use(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        kind: &str,
    ) -> bool {
        self.can_use(host, player)
            && matches!(kind, "minecraft:item_frame" | "minecraft:glow_item_frame")
    }

    pub(crate) fn use_on(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        level: LevelRef,
        pos: [i32; 3],
        rolls: &Rolls,
        weights: &Weights,
    ) {
        let Some(state) = level.block_state(host, pos) else {
            return;
        };
        let Some(sound) = self.target_sound(&state.block().id(host)) else {
            return;
        };
        // Preserve the successful action's original seed without drawing on denied breaks.
        let mut seed = level.random(host);
        if !player.remove_block_without_drops(host, pos, state) {
            return;
        }

        if let Ok(Some(stack)) =
            self.roll_stack(host, weights, |bound| rolls.next_from(bound, || seed.take()))
        {
            let [x, y, z] = crate::random::natural_drop_position(host, level, pos);
            let _ = level.spawn_item(host, x, y, z, stack, 10);
        }
        let center = pos.map(|axis| f64::from(axis) + 0.5);
        let _ = level.play_sound(host, sound, center, SoundSource::Blocks, 1.0, 1.0);
    }

    pub(crate) fn target_sound(&self, block: &str) -> Option<&'static str> {
        self.targets
            .iter()
            .find_map(|&(target, sound)| (target == block).then_some(sound))
    }

    pub(crate) fn weighted_drops(
        &self,
        weights: &Weights,
    ) -> impl Iterator<Item = (u32, Kind)> + Clone {
        let configured: &[u32] = if self.item == GOLD_PAN {
            &weights.overworld
        } else {
            &weights.nether
        };
        configured
            .iter()
            .copied()
            .zip(self.drops.iter().map(|&(_, kind)| kind))
            .filter(|&(weight, _)| weight != 0)
    }

    pub(crate) fn roll_stack(
        &self,
        host: &dyn Host,
        weights: &Weights,
        sample: impl FnMut(u32) -> Option<u32>,
    ) -> Result<Option<ItemStack>, ()> {
        self.choose(weights, sample)?
            .map(|kind| kind.stack(host, 1).ok_or(()))
            .transpose()
    }

    fn choose(
        &self,
        weights: &Weights,
        mut sample: impl FnMut(u32) -> Option<u32>,
    ) -> Result<Option<Kind>, ()> {
        let drops = self.weighted_drops(weights);
        let total: f32 = drops.clone().map(|(weight, _)| weight as f32).sum();
        if total == 0.0 {
            return Ok(None);
        }
        let sample = sample(SAMPLE_COUNT)
            .filter(|&sample| sample < SAMPLE_COUNT)
            .ok_or(())?;
        let target = (sample as f32 / SAMPLE_COUNT as f32) * total;
        let mut cumulative = 0.0;
        for (weight, kind) in drops {
            cumulative += weight as f32;
            if cumulative >= target {
                return Ok(Some(kind));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
#[path = "gold_pan_interaction_tests.rs"]
mod interaction_tests;

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn distribution(pan: &Pan) -> BTreeMap<&'static str, usize> {
        let mut counts = BTreeMap::new();
        let weights = Weights::default();
        for bucket in 0..100 {
            let drop = pan
                .choose(&weights, |bound| Some((2 * bucket + 1) * bound / 200))
                .unwrap()
                .unwrap();
            *counts.entry(drop.id()).or_insert(0) += 1;
        }
        counts
    }

    #[test]
    fn gravel_pan_keeps_all_four_paper_weights() {
        assert_eq!(
            distribution(&PANS[0]),
            BTreeMap::from([
                ("minecraft:flint", 40),
                ("minecraft:clay_ball", 20),
                ("SIFTED_ORE", 35),
                ("minecraft:iron_nugget", 5),
            ]),
        );
    }

    #[test]
    fn nether_pan_matches_both_paper_targets_and_all_six_drop_weights() {
        let pan = &PANS[1];
        assert!(pan.target_sound("minecraft:soul_sand").is_some());
        assert!(pan.target_sound("minecraft:soul_soil").is_some());
        assert!(pan.target_sound("minecraft:gravel").is_none());
        assert!(PANS[0].target_sound("minecraft:soul_sand").is_none());
        assert_eq!(
            distribution(pan),
            BTreeMap::from([
                ("minecraft:quartz", 50),
                ("minecraft:gold_nugget", 25),
                ("minecraft:nether_wart", 10),
                ("minecraft:blaze_powder", 8),
                ("minecraft:glowstone_dust", 5),
                ("minecraft:ghast_tear", 2),
            ]),
        );
    }
    #[test]
    fn config_uses_paper_keys_and_positive_weights_normalize_without_a_hundred_cap() {
        let mut weights = Weights::default();
        let options = r#"
            [items.GOLD_PAN.chance]
            FLINT = 0
            CLAY = 2
            SIFTED_ORE = 0
            IRON_NUGGET = 6
            [items.NETHER_GOLD_PAN.chance]
            QUARTZ = 0
            GOLD_NUGGET = 0
            NETHER_WART = 0
            BLAZE_POWDER = 0
            GLOWSTONE_DUST = 0
            GHAST_TEAR = 2147483647
        "#
        .parse::<toml::Table>()
        .unwrap();
        weights.read_config(options.get("items"));
        let choose = |pan: usize, sample| {
            PANS[pan]
                .choose(&weights, |_| Some(sample))
                .unwrap()
                .map(Kind::id)
        };
        assert_eq!(choose(0, 0), Some("minecraft:clay_ball"));
        assert_eq!(choose(0, SAMPLE_COUNT / 4), Some("minecraft:clay_ball"));
        assert_eq!(
            choose(0, SAMPLE_COUNT / 4 + 1),
            Some("minecraft:iron_nugget")
        );
        assert_eq!(choose(0, SAMPLE_COUNT - 1), Some("minecraft:iron_nugget"));
        assert_eq!(choose(1, SAMPLE_COUNT - 1), Some("minecraft:ghast_tear"));
        assert!(PANS[0].choose(&weights, |_| None).is_err());

        let changed = r#"
            [items.GOLD_PAN.chance]
            FLINT = 0
            CLAY = 0
            SIFTED_ORE = 0
            IRON_NUGGET = 0
            [items.NETHER_GOLD_PAN.chance]
            GHAST_TEAR = -1
            QUARTZ = "invalid"
            GOLD_NUGGET = 2147483648
        "#
        .parse::<toml::Table>()
        .unwrap();
        weights.read_config(changed.get("items"));
        assert_eq!(
            PANS[0].choose(&weights, |_| panic!("disabled pan should not draw")),
            Ok(None)
        );
        assert_eq!(weights.nether, [50, 25, 10, 8, 5, 2147483647]);

        weights.read_config(None);
        assert_eq!(weights.overworld, Weights::default().overworld);
        assert_eq!(weights.nether, Weights::default().nether);
    }
}
