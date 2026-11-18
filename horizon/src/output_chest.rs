// SPDX-License-Identifier: GPL-3.0-only

use std::sync::atomic::{AtomicBool, Ordering};

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentType, BlockDropsEvent, Compound, Holder,
    HostTransaction, ITEMS, ItemKey, ItemPart, ItemStack, LevelHost, LevelRef,
    OnBlockPlacedByEvent, ServerHost, Tag,
};

use crate::items::OUTPUT_CHEST;
use crate::machine::deposit_into;

static ENABLED: AtomicBool = AtomicBool::new(true);

pub(crate) fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub(crate) fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

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

pub(crate) fn restore_paper_marker(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
) -> bool {
    if !level.in_place(host)
        || !host.block_key("minecraft:chest").is_some_and(|chest| {
            level
                .block_state(host, pos)
                .is_some_and(|state| state.block() == chest)
        })
    {
        return false;
    }
    let holder = Holder::BlockEntity(pos);
    matches!(MARKER.get_existing(host, holder), Ok(Some(true)))
        || MARKER.set(host, holder, true).is_ok()
}

/// Paper's output selection follows faces in this exact order. A chest with
/// insufficient space leaves its inventory untouched before the next candidate.
pub(crate) fn try_deposit(
    host: &mut dyn ServerHost,
    level: LevelRef,
    dispenser: [i32; 3],
    stack: &ItemStack,
) -> bool {
    try_deposit_with_extra(host, level, dispenser, stack, None, false)
}

pub(crate) fn route_or_drop(
    host: &mut dyn ServerHost,
    level: LevelRef,
    adjacent_to: [i32; 3],
    drop_at: [i32; 3],
    stack: &ItemStack,
) -> bool {
    let [x, y, z] = drop_at.map(|axis| f64::from(axis) + 0.5);
    try_deposit(host, level, adjacent_to, stack)
        || level
            .spawn_item(host, x, y, z, stack.clone(), 10)
            .is_ok_and(|item| item.is_some())
}

pub(crate) fn try_deposit_with_extra(
    host: &mut dyn ServerHost,
    level: LevelRef,
    dispenser: [i32; 3],
    stack: &ItemStack,
    extra: Option<&ItemStack>,
    require_empty: bool,
) -> bool {
    if !enabled() {
        return false;
    }
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
        if try_store(host, level, pos, stack, extra, require_empty) {
            return true;
        }
    }
    false
}

fn try_store(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    stack: &ItemStack,
    extra: Option<&ItemStack>,
    require_empty: bool,
) -> bool {
    let Some(handler) = level.capability(host, &ITEMS, pos, None) else {
        return false;
    };
    let slots = handler.size(host);
    if require_empty && (0..slots).all(|slot| handler.resource(host, slot).is_some()) {
        return false;
    }
    host.transaction(|host, tx| {
        if deposit_into(host, tx, level, pos, slots, stack, stack.count()) != 0 {
            return Err(());
        }
        if let Some(extra) = extra {
            let _ = deposit_into(host, tx, level, pos, slots, extra, extra.count());
        }
        Ok(())
    })
    .is_ok()
}

pub(crate) fn restore_drop(level: &dyn LevelHost, event: &mut BlockDropsEvent<'_>, chest: ItemKey) {
    let Some(entity) = event.block_entity else {
        return;
    };
    let Some(marker) = level.block_entity_saved_attachment(entity, MARKER.namespace, MARKER.name)
    else {
        return;
    };
    if marker.get(&"placed".into()) != Some(&Tag::Byte(1)) {
        return;
    }
    if let Some(drop) = event
        .drops
        .iter_mut()
        .find(|drop| drop.stack.item() == chest)
    {
        OUTPUT_CHEST.decorate(&mut drop.stack);
    }
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

        assert!(!try_store(&mut host, level, first, &stack, None, false));
        assert!(try_store(&mut host, level, next, &stack, None, false));
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
