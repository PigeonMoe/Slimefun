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
    charge: u32,
) -> Result<(i32, u32), ()> {
    if charge < COST {
        return Err(());
    }
    let dimension = level.dimension().id(host).ok_or(())?;
    let low = pos.map(|n| f64::from(n) - 4.0);
    let high = pos.map(|n| f64::from(n) + 4.0);
    let orbs = host.experience_orbs(&dimension, low, high);
    if orbs.is_empty() {
        return Err(());
    }
    let output = FILLED_FLASK_OF_KNOWLEDGE.stack(host, 1).ok_or(())?;
    let resource = ItemResource::of(&output);

    let orbs = orbs.into_iter().map(|orb| (orb.id, orb.value));
    collect_orbs(stored, charge, orbs, |id, requested| {
        let entity = host.entity(id).ok_or(())?;
        let mut remaining = requested;
        for slot in 0..MENU_SLOTS.len() {
            if remaining == 0 {
                break;
            }
            remaining -= place(level, pos, slot).insert(host, tx, &resource, remaining);
        }
        // Entity removal and bottle writes share the caller's transaction
        // with the final charge and saved balance.
        entity
            .remove(host, RemovalReason::Discarded)
            .then_some(requested - remaining)
            .ok_or(())
    })
}

fn collect_orbs(
    mut stored: i32,
    mut charge: u32,
    orbs: impl IntoIterator<Item = (u32, i32)>,
    mut collect: impl FnMut(u32, u32) -> Result<u32, ()>,
) -> Result<(i32, u32), ()> {
    let initial_charge = charge;
    for (id, value) in orbs {
        let Some(next_charge) = charge.checked_sub(COST) else {
            break;
        };
        // Paper checks the total before withdrawing any bottles. A signed
        // nonzero total ends the loop, even if the saved balance is zero.
        let total = i64::from(stored) + i64::from(value);
        // Bottling uses the balance from before this orb.
        let items = collect(id, bottles_for(stored))?;
        stored = i32::try_from(total - i64::from(items) * 10).map_err(|_| ())?;
        charge = next_charge;
        if total != 0 {
            break;
        }
    }
    (charge != initial_charge)
        .then_some((stored, charge))
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

    #[test]
    fn zero_total_advances_to_another_orb_and_charges_for_each() {
        // The ordinary orb sequence 1, 3, 3 reaches the starting balance -3.
        let orbs = [(41, 3), (42, 1), (43, 7)];
        let mut taken = Vec::new();
        assert_eq!(
            collect_orbs(-3, 30, orbs, |id, bottles| {
                taken.push((id, bottles));
                Ok(bottles)
            }),
            Ok((1, 10))
        );
        assert_eq!(taken, [(41, 0), (42, 0)]);

        taken.clear();
        assert_eq!(
            collect_orbs(-3, 19, orbs, |id, bottles| {
                taken.push((id, bottles));
                Ok(bottles)
            }),
            Ok((0, 9))
        );
        assert_eq!(taken, [(41, 0)]);
        assert_eq!(collect_orbs(-3, 9, orbs, |_, _| unreachable!()), Err(()));
    }

    #[test]
    fn the_stop_condition_uses_the_signed_total_before_bottle_withdrawal() {
        let orbs = [(41, 7), (42, 1)];
        assert_eq!(
            collect_orbs(-8, 20, orbs, |id, bottles| {
                assert_eq!((id, bottles), (41, 0));
                Ok(0)
            }),
            Ok((-1, 10))
        );
        assert_eq!(
            collect_orbs(3, 20, orbs, |id, bottles| {
                assert_eq!((id, bottles), (41, 1));
                Ok(bottles)
            }),
            Ok((0, 10))
        );
    }
}
