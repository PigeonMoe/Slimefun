// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{LevelRef, PlayerRef, ServerHost, SoundSource, Text};

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
    pub drops: &'static [(u32, Kind)],
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

impl Pan {
    pub(crate) fn use_on(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        level: LevelRef,
        pos: [i32; 3],
        rolls: &Rolls,
    ) {
        let Some(state) = level.block_state(host, pos) else {
            return;
        };
        let Some(sound) = self.target_sound(&state.block().id(host)) else {
            return;
        };
        if !research::can_use(host, player, self.research) {
            let text = format!(
                "Unlock {} with /sf research {}.",
                self.research.name, self.research.id,
            );
            let _ = player.tell(host, Text::literal(text));
            return;
        }

        let Some(kind) = rolls
            .next(host, level, 100)
            .and_then(|roll| self.drop_for(roll))
        else {
            return;
        };
        let Some(stack) = kind.stack(host, 1) else {
            return;
        };
        if !player.remove_block_without_drops(host, pos, state) {
            return;
        }

        let [x, y, z] = pos.map(|axis| f64::from(axis) + 0.5);
        let _ = level.spawn_item(host, x, y, z, stack, 10);
        let _ = level.play_sound(host, sound, [x, y, z], SoundSource::Blocks, 1.0, 1.0);
    }

    fn target_sound(&self, block: &str) -> Option<&'static str> {
        self.targets
            .iter()
            .find_map(|&(target, sound)| (target == block).then_some(sound))
    }

    fn drop_for(&self, mut roll: u32) -> Option<Kind> {
        for &(weight, kind) in self.drops {
            if roll < weight {
                return Some(kind);
            }
            roll -= weight;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn distribution(pan: &Pan) -> BTreeMap<&'static str, usize> {
        let mut counts = BTreeMap::new();
        for roll in 0..100 {
            *counts.entry(pan.drop_for(roll).unwrap().id()).or_insert(0) += 1;
        }
        assert_eq!(pan.drop_for(100), None);
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
}
