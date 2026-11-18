// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{LevelRef, PlayerRef, ServerHost, SoundSource, Text};

use crate::items::Kind;
use crate::random::Rolls;
use crate::recipes::{GOLD_PAN_RESEARCH, PAN_DROPS};
use crate::research;

pub(crate) fn use_pan(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    level: LevelRef,
    pos: [i32; 3],
    rolls: &Rolls,
) {
    let Some(gravel) = level
        .block_state(host, pos)
        .filter(|state| Some(state.block()) == host.block_key("minecraft:gravel"))
    else {
        return;
    };
    if !research::can_use(host, player, &GOLD_PAN_RESEARCH) {
        let _ = player.tell(
            host,
            Text::literal("Unlock Gold Pan with /sf research gold_pan."),
        );
        return;
    }

    let Some(kind) = rolls.next(host, level, 100).and_then(drop_for) else {
        return;
    };
    let Some(stack) = kind.stack(host, 1) else {
        return;
    };
    if !player.remove_block_without_drops(host, pos, gravel) {
        return;
    }

    let [x, y, z] = pos.map(|axis| f64::from(axis) + 0.5);
    let _ = level.spawn_item(host, x, y, z, stack, 10);
    let _ = level.play_sound(
        host,
        "minecraft:block.gravel.break",
        [x, y, z],
        SoundSource::Blocks,
        1.0,
        1.0,
    );
}

fn drop_for(mut roll: u32) -> Option<Kind> {
    for &(weight, kind) in PAN_DROPS {
        if roll < weight {
            return Some(kind);
        }
        roll -= weight;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pans_keep_all_four_paper_weights() {
        let mut counts = std::collections::BTreeMap::new();
        for roll in 0..100 {
            *counts.entry(drop_for(roll).unwrap().id()).or_insert(0) += 1;
        }
        assert_eq!(counts.values().copied().sum::<usize>(), 100);
        assert_eq!(counts["minecraft:flint"], 40);
        assert_eq!(counts["minecraft:clay_ball"], 20);
        assert_eq!(counts["SIFTED_ORE"], 35);
        assert_eq!(counts["minecraft:iron_nugget"], 5);
        assert_eq!(drop_for(100), None);
    }
}
