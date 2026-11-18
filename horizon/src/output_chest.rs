// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentType, Compound, Holder, HostTransaction, ITEMS,
    ItemPart, ItemStack, LevelRef, OnBlockPlacedByEvent, ServerHost, Tag,
};

use crate::items::OUTPUT_CHEST;
use crate::machine::deposit_into;

static MARKER: AttachmentType<bool> = AttachmentType {
    namespace: "slimefun",
    name: "output_chest",
    default: || false,
    codec: Some(AttachmentCodec {
        encode: |value| {
            Ok(Compound::from([(
                "placed".into(),
                Tag::Byte(i8::from(*value)),
            )]))
        },
        decode: |value| match value.get(&"placed".into()) {
            Some(Tag::Byte(1)) => Ok(true),
            Some(Tag::Byte(0)) => Ok(false),
            _ => Err(AttachmentError::Decode),
        },
    }),
    copy_on_death: false,
    sync: false,
};

pub(crate) struct ChestPlacement;

#[mod_api::horizon_part]
impl ItemPart for ChestPlacement {
    fn on_block_placed_by(&self, host: &mut dyn ServerHost, event: &mut OnBlockPlacedByEvent<'_>) {
        if event.placer.is_some() && OUTPUT_CHEST.matches(host, event.stack) {
            let _ = MARKER.set(host, Holder::BlockEntity(event.pos), true);
        }
    }
}

/// Paper's output selection follows faces in this exact order. A chest with
/// insufficient space leaves its inventory untouched before the next candidate.
pub(crate) fn try_deposit(
    host: &mut dyn ServerHost,
    level: LevelRef,
    dispenser: [i32; 3],
    stack: &ItemStack,
) -> bool {
    if !level.in_place(host) {
        return false;
    }
    let Some(chest) = host.block_key("minecraft:chest") else {
        return false;
    };
    for pos in adjacent(dispenser) {
        if level
            .block_state(host, pos)
            .is_none_or(|state| state.block() != chest)
        {
            continue;
        }
        if !matches!(
            MARKER.get_existing(host, Holder::BlockEntity(pos)),
            Ok(Some(true))
        ) {
            continue;
        }
        if try_store(host, level, pos, stack) {
            return true;
        }
    }
    false
}

fn try_store(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], stack: &ItemStack) -> bool {
    let Some(handler) = level.capability(host, &ITEMS, pos, None) else {
        return false;
    };
    let slots = handler.size(host);
    host.transaction(|host, tx| {
        let remaining = deposit_into(host, tx, level, pos, slots, stack, stack.count());
        (remaining == 0).then_some(()).ok_or(())
    })
    .is_ok()
}

fn adjacent([x, y, z]: [i32; 3]) -> impl Iterator<Item = [i32; 3]> {
    [
        y.checked_add(1).map(|y| [x, y, z]),
        z.checked_sub(1).map(|z| [x, y, z]),
        x.checked_add(1).map(|x| [x, y, z]),
        z.checked_add(1).map(|z| [x, y, z]),
        x.checked_sub(1).map(|x| [x, y, z]),
    ]
    .into_iter()
    .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::items::Kind;
    use mod_api::{Identifier, testing::FakeHost};

    #[test]
    fn full_candidate_rolls_back_before_trying_the_next_chest() {
        let mut host = FakeHost::new();
        let first = [0, 64, -1];
        let next = [1, 64, 0];
        let level = host.level.level();
        let powder = Kind::Vanilla("minecraft:blaze_powder");
        host.registries
            .resolve_items([Identifier::parse(powder.id()).unwrap()])
            .unwrap();
        for pos in [first, next] {
            host.level.block_entity(pos).with_items(1);
        }
        let occupied = powder.stack(&host, 63).unwrap();
        host.level.block_entity(first).items.as_mut().unwrap().slots[0] = Some(occupied);
        let stack = powder.stack(&host, 4).unwrap();

        assert!(!try_store(&mut host, level, first, &stack));
        assert!(try_store(&mut host, level, next, &stack));
        let amounts = [first, next].map(|pos| {
            host.level
                .block_entity_at(pos)
                .unwrap()
                .items
                .as_ref()
                .unwrap()
                .slots[0]
                .as_ref()
                .unwrap()
                .count()
        });
        assert_eq!(amounts, [63, 4]);
    }

    #[test]
    fn adjacent_chests_follow_paper_order_and_skip_below() {
        assert_eq!(
            adjacent([6, 64, -3]).collect::<Vec<_>>(),
            [
                [6, 65, -3],
                [6, 64, -4],
                [7, 64, -3],
                [6, 64, -2],
                [5, 64, -3]
            ],
        );
    }
}
