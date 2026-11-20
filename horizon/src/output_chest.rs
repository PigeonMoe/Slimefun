// SPDX-License-Identifier: GPL-3.0-only

use std::collections::BTreeSet;
use std::sync::RwLock;

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentType, BlockDropsEvent, Compound, Holder, Host,
    HostSide, HostTransaction, ITEMS, InteractionResult, ItemKey, ItemPart, ItemStack, LevelHost,
    LevelRef, OnBlockPlacedByEvent, ServerHost, Tag, Text, UseOnEvent,
};

use crate::items::OUTPUT_CHEST;
use crate::machine::deposit_into;

struct Availability {
    enabled: bool,
    disabled_worlds: BTreeSet<String>,
}

impl Availability {
    const DEFAULT: Self = Self {
        enabled: true,
        disabled_worlds: BTreeSet::new(),
    };

    fn read(config: &toml::Table) -> Self {
        let enabled = config
            .get("output-chest-enabled")
            .and_then(toml::Value::as_bool)
            .unwrap_or(true);
        let disabled_worlds = config
            .get("worlds")
            .and_then(toml::Value::as_table)
            .into_iter()
            .flatten()
            .filter(|(_, settings)| {
                settings
                    .get("output-chest-enabled")
                    .and_then(toml::Value::as_bool)
                    == Some(false)
            })
            .map(|(world, _)| world.clone())
            .collect();
        Self {
            enabled,
            disabled_worlds,
        }
    }

    fn allows(&self, dimension: &str) -> bool {
        self.enabled && !self.disabled_worlds.contains(dimension)
    }
}

static AVAILABILITY: RwLock<Availability> = RwLock::new(Availability::DEFAULT);

pub(crate) fn configure(config: &toml::Table) {
    *AVAILABILITY.write().unwrap() = Availability::read(config);
}

pub(crate) fn enabled(dimension: &str) -> bool {
    AVAILABILITY.read().unwrap().allows(dimension)
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
    fn use_on(&self, host: HostSide<'_>, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        if !OUTPUT_CHEST.matches(host, event.stack) {
            return None;
        }
        let player = host.player(event.player)?;
        let message = if !enabled(host.dimension()) {
            "The output chest is disabled in this world."
        } else if !crate::research::can_use(host, player, &crate::recipes::CHEST_RESEARCH) {
            "Unlock the output chest with /sf research output_chest."
        } else {
            return None;
        };
        let _ = player.tell(host, Text::literal(message));
        Some(InteractionResult::Fail)
    }

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
    try_deposit_matching(
        host,
        level,
        dispenser,
        stack,
        extra,
        require_empty,
        |_, _| true,
    )
}

pub(crate) fn try_deposit_checked(
    host: &mut dyn ServerHost,
    level: LevelRef,
    origin: [i32; 3],
    stack: &ItemStack,
) -> bool {
    try_deposit_matching(host, level, origin, stack, None, false, |host, target| {
        if !level.allow_inventory_transfer(host, origin, target, stack) {
            return false;
        }
        let Some(items) = level.capability(host, &ITEMS, target, None) else {
            return false;
        };
        if items.size(host) <= 27 {
            return true;
        }
        // A capability can write slots from either half of a joined chest.
        // Both coordinates must permit the transfer before passing it to the inventory.
        double_chest_partner(
            target,
            level.block_property(host, target, "facing").as_deref(),
            level.block_property(host, target, "type").as_deref(),
        )
        .is_some_and(|other| level.allow_inventory_transfer(host, origin, other, stack))
    })
}

pub(crate) fn double_chest_partner(
    [x, y, z]: [i32; 3],
    facing: Option<&str>,
    side: Option<&str>,
) -> Option<[i32; 3]> {
    let (dx, dz): (i32, i32) = match facing? {
        "north" => (0, -1),
        "east" => (1, 0),
        "south" => (0, 1),
        "west" => (-1, 0),
        _ => return None,
    };
    match side? {
        "left" => Some([x.checked_sub(dz)?, y, z.checked_add(dx)?]),
        "right" => Some([x.checked_add(dz)?, y, z.checked_sub(dx)?]),
        _ => None,
    }
}

fn try_deposit_matching(
    host: &mut dyn ServerHost,
    level: LevelRef,
    dispenser: [i32; 3],
    stack: &ItemStack,
    extra: Option<&ItemStack>,
    require_empty: bool,
    mut allowed: impl FnMut(&mut dyn ServerHost, [i32; 3]) -> bool,
) -> bool {
    if !level.in_place(host) || !enabled(host.dimension()) {
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
        if allowed(host, pos) && try_store(host, level, pos, stack, extra, require_empty) {
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
    if !marked(level, entity) {
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

pub(crate) fn live_marker(host: &dyn Host, entity: mod_api::BlockEntityRef) -> bool {
    placed(host.block_entity_saved_attachment(entity, MARKER.namespace, MARKER.name))
}

fn placed(data: Option<Compound>) -> bool {
    data.is_some_and(|marker| marker.get(&"placed".into()) == Some(&Tag::Byte(1)))
}

pub(crate) fn marked(level: &dyn LevelHost, entity: mod_api::BlockEntityRef) -> bool {
    placed(level.block_entity_saved_attachment(entity, MARKER.namespace, MARKER.name))
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
    fn output_chest_world_settings_inherit_the_global_switch() {
        let config = r#"
            [worlds."minecraft:the_nether"]
            output-chest-enabled = false

            [worlds."minecraft:the_end"]
            output-chest-enabled = true
        "#;
        let table = config.parse::<toml::Table>().unwrap();
        let worlds = Availability::read(&table);
        assert!(worlds.allows("minecraft:overworld"));
        assert!(!worlds.allows("minecraft:the_nether"));
        assert!(worlds.allows("minecraft:the_end"));

        let all_disabled = format!("output-chest-enabled = false\n{config}")
            .parse::<toml::Table>()
            .unwrap();
        let all_disabled = Availability::read(&all_disabled);
        assert!(!all_disabled.allows("minecraft:overworld"));
        assert!(!all_disabled.allows("minecraft:the_end"));
    }

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

    #[test]
    fn guarded_output_resolves_the_other_physical_half_of_a_joined_chest() {
        let at = [15, 64, 0];
        for (facing, left, right) in [
            ("north", [16, 64, 0], [14, 64, 0]),
            ("east", [15, 64, 1], [15, 64, -1]),
            ("south", [14, 64, 0], [16, 64, 0]),
            ("west", [15, 64, -1], [15, 64, 1]),
        ] {
            assert_eq!(
                double_chest_partner(at, Some(facing), Some("left")),
                Some(left)
            );
            assert_eq!(
                double_chest_partner(at, Some(facing), Some("right")),
                Some(right)
            );
        }
        assert_eq!(
            double_chest_partner(at, Some("north"), Some("single")),
            None
        );
    }
}
