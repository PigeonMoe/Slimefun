// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{
    Hand, HostTransaction, ItemPlace, ItemResource, LevelRef, PlayerRef, RemovalReason, ServerHost,
    SoundSource, Text, Transaction,
};

use crate::{
    enchantment_machines::{AUTO_ENCHANTER, WITHER_PROOF_OBSIDIAN},
    items::{ALUMINUM_BRONZE_INGOT, ELECTRIC_MOTOR, HeadTexture, Kind, MAGIC_LUMP_2, item},
    network_recipes::BLISTERING_INGOT_3,
    recipes::{CraftingRecipe, Research, Stack},
    research,
};

use super::{Position, place};

item!(
    XP_COLLECTOR,
    "player_head",
    "EXP Collector",
    "green",
    None,
    Some(HeadTexture {
        profile: [771298631, -1856228317, -1863092781, 1458242691],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzE3NjJhMTViMDQ2OTJhMmU0YjNmYjM2NjNiZDRiNzg0MzRkY2UxNzMyYjhlYjFjN2E5ZjdjMGZiZjZmIn19fQ==",
    })
);
item!(
    FLASK_OF_KNOWLEDGE,
    "glass_bottle",
    "Flask of Knowledge",
    "red",
    None,
    None
);
item!(
    FILLED_FLASK_OF_KNOWLEDGE,
    "experience_bottle",
    "Flask of Knowledge",
    "green",
    None,
    None
);

pub(crate) const CAPACITY: u32 = 1_024;
pub(crate) const COST: u32 = 10;
pub(crate) const RESEARCH: Research = Research {
    id: "xp_collector",
    name: "XP Collector",
    cost: 36,
};
pub(crate) const FLASK_RESEARCH: Research = Research {
    id: "flask_of_knowledge",
    name: "XP Storage",
    cost: 13,
};
pub(super) const MENU_SLOTS: &[usize] = &[12, 13, 14];

pub(crate) const COLLECTOR_RECIPE: CraftingRecipe = CraftingRecipe {
    inputs: [
        None,
        Some(BLISTERING_INGOT_3),
        None,
        Some(WITHER_PROOF_OBSIDIAN),
        Some(AUTO_ENCHANTER),
        Some(WITHER_PROOF_OBSIDIAN),
        Some(ALUMINUM_BRONZE_INGOT),
        Some(ELECTRIC_MOTOR),
        Some(ALUMINUM_BRONZE_INGOT),
    ],
    output: Stack {
        kind: XP_COLLECTOR,
        count: 1,
    },
    research: &RESEARCH,
};

pub(crate) const FLASK_RECIPE: CraftingRecipe = CraftingRecipe {
    inputs: [
        None,
        None,
        None,
        Some(MAGIC_LUMP_2),
        Some(Kind::Vanilla("minecraft:glass_pane")),
        Some(MAGIC_LUMP_2),
        None,
        Some(MAGIC_LUMP_2),
        None,
    ],
    output: Stack {
        kind: FLASK_OF_KNOWLEDGE,
        count: 8,
    },
    research: &FLASK_RESEARCH,
};

pub(crate) fn collect(
    host: &mut dyn ServerHost,
    tx: &Transaction,
    level: LevelRef,
    pos: Position,
    stored: i32,
) -> Result<i32, ()> {
    let dimension = level.dimension().id(host).ok_or(())?;
    let low = pos.map(|n| f64::from(n) - 4.0);
    let high = pos.map(|n| f64::from(n) + 4.0);
    let orb = host
        .experience_orbs(&dimension, low, high)
        .into_iter()
        .next()
        .ok_or(())?;
    let entity = host.entity(orb.id).ok_or(())?;
    let output = FILLED_FLASK_OF_KNOWLEDGE.stack(host, 1).ok_or(())?;
    let resource = ItemResource::of(&output);

    // Paper fills against the balance from the previous orb, at one bottle
    // per started group of ten points. The incoming orb funds the next cycle.
    let requested = bottles_for(stored);
    let mut remaining = requested;
    for slot in 0..MENU_SLOTS.len() {
        remaining -= place(level, pos, slot).insert(host, tx, &resource, remaining);
        if remaining == 0 {
            break;
        }
    }
    let spent = i64::from(requested - remaining) * 10;
    let next = i32::try_from(i64::from(stored) + i64::from(orb.value) - spent).map_err(|_| ())?;
    // This uses the host's journalled entity removal, so a failing charge or
    // stored-balance write also restores the orb and all output slots.
    entity
        .remove(host, RemovalReason::Discarded)
        .then_some(next)
        .ok_or(())
}

fn bottles_for(previous: i32) -> u32 {
    (previous.max(0) as u32).div_ceil(10)
}

pub(crate) fn use_flask(host: &mut dyn ServerHost, player: PlayerRef, hand: Hand) -> bool {
    let held = ItemPlace::PlayerHand {
        player: player.uuid(),
        hand,
    };
    let Some(empty) = held
        .stack(host)
        .filter(|stack| FLASK_OF_KNOWLEDGE.matches(host, stack))
    else {
        return false;
    };
    if !research::can_use(host, player, &FLASK_RESEARCH) {
        let _ = player.tell(
            host,
            Text::literal("Unlock this research with /sf research flask_of_knowledge."),
        );
        return true;
    }
    let Some(experience) = player
        .experience(host)
        .filter(|experience| experience.level > 0)
    else {
        return true;
    };
    let Some(filled) = FILLED_FLASK_OF_KNOWLEDGE.stack(host, 1) else {
        return true;
    };
    let inserted = host.transaction(|host, tx| {
        let resource = ItemResource::of(&filled);
        let inserted = (0..36).any(|slot| {
            ItemPlace::PlayerSlot {
                player: player.uuid(),
                slot,
            }
            .insert(host, tx, &resource, 1)
                == 1
        });
        if !inserted {
            let entity = player.entity();
            let level = entity.level(host).ok_or(())?;
            let [x, y, z] = entity.position(host).ok_or(())?;
            level
                .spawn_item(host, x, y, z, filled, 10)
                .map_err(|_| ())?
                .ok_or(())?;
        }
        if held.extract(host, tx, &ItemResource::of(&empty), 1) != 1 {
            return Err(());
        }
        player
            .set_experience_levels(host, experience.level - 1)
            .then_some(())
            .ok_or(())
    });
    if inserted.is_ok() {
        player.play_sound(
            host,
            "minecraft:entity.experience_orb.pickup",
            SoundSource::Master,
            1.0,
            0.5,
        );
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flask_count_uses_the_previous_paper_balance_in_ten_point_steps() {
        assert_eq!([-8, 0, 1, 10, 11, 21].map(bottles_for), [0, 0, 1, 1, 2, 3]);
    }
}
