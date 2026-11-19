// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{
    BlockState, BlockUpdateFlags, Flow, Hand, Host, HostBlockStates, HostTransaction, ItemPlace,
    ItemStack, LevelRef, PlayerDestroyedBlockEvent, PlayerRef, ServerHost, SoundSource, Tag, Text,
};

use crate::{
    advanced_processing::SYNTHETIC_EMERALD,
    android_machine::{self, Position},
    android_woodcutting::connected_logs,
    items::{GILDED_IRON, SYNTHETIC_DIAMOND, item},
    network_runtime,
    recipes::{CraftingRecipe, Research, Stack},
    research,
};

item!(LUMBER_AXE, "diamond_axe", "Lumber Axe", "gold", None, None);

pub(crate) fn lore() -> Tag {
    Tag::List(vec![crate::items::text(
        "Cuts down the whole Tree...",
        "green",
        true,
    )])
}

pub(crate) const RESEARCH: Research = Research {
    id: "lumber_axe",
    name: "Lumber Axe",
    cost: 21,
};

pub(crate) const CRAFTING: CraftingRecipe = CraftingRecipe {
    inputs: [
        Some(SYNTHETIC_DIAMOND),
        Some(SYNTHETIC_DIAMOND),
        None,
        Some(SYNTHETIC_EMERALD),
        Some(GILDED_IRON),
        None,
        None,
        Some(GILDED_IRON),
        None,
    ],
    output: Stack {
        kind: LUMBER_AXE,
        count: 1,
    },
    research: &RESEARCH,
};

const MAX_BROKEN: usize = 100;
const MAX_STRIPPED: usize = 20;

fn held_axe(host: &mut dyn ServerHost, player: PlayerRef) -> bool {
    ItemPlace::PlayerHand {
        player: player.uuid(),
        hand: Hand::Main,
    }
    .stack(host)
    .is_some_and(|stack| LUMBER_AXE.matches(host, &stack))
}

fn unlocked(host: &mut dyn ServerHost, player: PlayerRef) -> bool {
    if research::can_use(host, player, &RESEARCH) {
        return true;
    }
    let _ = player.tell(
        host,
        Text::literal("Unlock Lumber Axe with /sf research lumber_axe."),
    );
    false
}

pub(crate) fn allow_break(host: &mut dyn ServerHost, player: PlayerRef) -> Flow {
    if player.is_fake(host) == Some(true) || !held_axe(host, player) || unlocked(host, player) {
        Flow::Continue
    } else {
        Flow::Cancel
    }
}

fn log(host: &dyn Host, state: BlockState) -> bool {
    host.block_in_tag(state.block(), "minecraft:logs") == Some(true)
}

fn stripped_id(material: &str) -> Option<String> {
    let name = material.strip_prefix("minecraft:")?;
    if name.starts_with("stripped_") {
        return None;
    }
    Some(format!("minecraft:stripped_{name}"))
}

fn can_change(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    level: LevelRef,
    pos: Position,
) -> bool {
    android_machine::inside_border(host, level, pos)
        && !network_runtime::has_block_marker(host, level, pos)
        && level.allow_device_breaking(host, player.uuid(), pos)
}

pub(crate) fn destroyed(host: &mut dyn ServerHost, event: &PlayerDestroyedBlockEvent) {
    if !log(host, event.state) {
        return;
    }
    let Some(player) = host.players().acting_player() else {
        return;
    };
    if player.is_fake(host) == Some(true)
        || player.entity().is_shift_key_down(host) == Some(true)
        || !held_axe(host, player)
    {
        return;
    }
    let Some(level) = player.level(host) else {
        return;
    };
    let (Some(tool), Some(air)) = (
        LUMBER_AXE.stack(host, 1),
        host.block_state_id("minecraft:air"),
    ) else {
        return;
    };
    // The initiating block is already removed. It still counts toward Paper's limit.
    let logs = connected_logs(event.pos, MAX_BROKEN, |pos| {
        pos == event.pos
            || level
                .block_state(host, pos)
                .is_some_and(|state| log(host, state))
    });
    for pos in logs.into_iter().skip(1) {
        let Some(old) = level
            .block_state(host, pos)
            .filter(|&state| log(host, state))
        else {
            continue;
        };
        if !can_change(host, player, level, pos) {
            continue;
        }
        let Ok(Some(drops)) = level.block_drops(host, pos, Some(&tool), None) else {
            continue;
        };
        if host
            .transaction(|host, _| {
                if level.block_state(host, pos) != Some(old)
                    || !level
                        .set_block(
                            host,
                            pos,
                            air,
                            BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS,
                        )
                        .map_err(|_| ())?
                {
                    return Err(());
                }
                for drop in drops {
                    let [x, y, z] = crate::random::natural_drop_position(host, level, pos);
                    level
                        .spawn_item(host, x, y, z, drop, 10)
                        .map_err(|_| ())?
                        .ok_or(())?;
                }
                Ok(())
            })
            .is_ok()
        {
            level.block_break_effect(host, pos, old);
        }
    }
}

pub(crate) fn strip(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    held: &ItemStack,
    root: Position,
) -> Flow {
    if !LUMBER_AXE.matches(host, held) {
        return Flow::Continue;
    }
    if !unlocked(host, player) {
        return Flow::Cancel;
    }
    if player.entity().is_shift_key_down(host) == Some(true) {
        return Flow::Continue;
    }
    let Some(level) = player.level(host) else {
        return Flow::Continue;
    };
    let logs = connected_logs(root, MAX_STRIPPED, |pos| {
        level
            .block_state(host, pos)
            .is_some_and(|state| log(host, state) && stripped_id(&state.block().id(host)).is_some())
    });
    for pos in logs.into_iter().skip(1) {
        let Some((old, next)) = stripped_state(host, level, pos) else {
            continue;
        };
        if can_change(host, player, level, pos)
            && level.block_state(host, pos) == Some(old)
            && level.set_block(
                host,
                pos,
                next,
                BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS,
            ) == Ok(true)
        {
            let _ = level.play_sound(
                host,
                "minecraft:item.axe.strip",
                pos.map(f64::from),
                SoundSource::Master,
                1.0,
                1.0,
            );
        }
    }
    // Vanilla strips the block the player clicked and wears the held axe once.
    Flow::Continue
}

fn stripped_state(
    host: &dyn Host,
    level: LevelRef,
    pos: Position,
) -> Option<(BlockState, BlockState)> {
    let old = level.block_state(host, pos)?;
    let new_id = stripped_id(&old.block().id(host))?;
    let metadata = host.block_states();
    let axis = old.property(&metadata, "axis").ok()??;
    let new = host
        .block_state_id(&new_id)?
        .with(&metadata, "axis", axis)
        .ok()?;
    Some((old, new))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Identifier, testing::FakeHost};

    #[test]
    fn recipe_uses_the_paper_magic_workbench_positions() {
        assert_eq!(
            CRAFTING.inputs.map(|entry| entry.map(|kind| kind.id())),
            [
                Some("SYNTHETIC_DIAMOND"),
                Some("SYNTHETIC_DIAMOND"),
                None,
                Some("SYNTHETIC_EMERALD"),
                Some("GILDED_IRON"),
                None,
                None,
                Some("GILDED_IRON"),
                None
            ],
        );
        assert_eq!(CRAFTING.research.cost, 21);
        assert_eq!(CRAFTING.output.kind.material(), "minecraft:diamond_axe");
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse("minecraft:diamond_axe").unwrap()])
            .unwrap();
        let axe = CRAFTING.output.kind.stack(&host, 1).unwrap();
        assert_eq!(
            axe.components()
                .unwrap()
                .as_nbt()
                .get(&"minecraft:lore".into()),
            Some(&Tag::List(vec![crate::items::text(
                "Cuts down the whole Tree...",
                "green",
                true
            )])),
        );
    }

    #[test]
    fn log_limits_include_the_root_and_only_matching_logs_connect() {
        let vertical = |[x, y, z]: Position| x == 0 && z == 0 && (0..110).contains(&y);
        let broken = connected_logs([0, 0, 0], MAX_BROKEN, vertical);
        assert_eq!((broken.len(), broken[99]), (100, [0, 99, 0]));
        let strips = connected_logs([0, 0, 0], MAX_STRIPPED, |pos: Position| {
            vertical(pos) && pos[1] != 3
        });
        assert_eq!(strips, [[0, 0, 0], [0, 1, 0], [0, 2, 0]]);
        assert_eq!(connected_logs([0, 0, 0], MAX_STRIPPED, vertical).len(), 20);
        assert_eq!(
            stripped_id("minecraft:warped_hyphae").as_deref(),
            Some("minecraft:stripped_warped_hyphae")
        );
        assert_eq!(stripped_id("minecraft:stripped_oak_log"), None);
    }
}
