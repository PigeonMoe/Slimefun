// SPDX-License-Identifier: GPL-3.0-only

use std::collections::BTreeSet;

use mod_api::{BlockUpdateFlags, HostTransaction, Identifier, LevelRef, ServerHost};

use crate::{
    android_machine::{self, Position},
    items::Kind,
};

const MAX_REACH: usize = 160;
const NEIGHBORS: [Position; 10] = [
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, -1],
    [1, 0, 0],
    [0, 0, 1],
    [-1, 0, 0],
    [1, 0, -1],
    [-1, 0, -1],
    [1, 0, 1],
    [-1, 0, 1],
];

/// Paper examines each connected log in a depth-first walk and cuts the last one.
fn last_log(start: Position, mut is_log: impl FnMut(Position) -> bool) -> Option<Position> {
    let mut found = BTreeSet::new();
    let mut pending = vec![start];
    let mut last = None;
    while let Some(pos) = pending.pop() {
        if found.contains(&pos) || !is_log(pos) {
            continue;
        }
        found.insert(pos);
        last = Some(pos);
        if found.len() == MAX_REACH {
            break;
        }
        pending.extend(
            NEIGHBORS
                .iter()
                .rev()
                .filter_map(|&face| android_machine::offset(pos, face)),
        );
    }
    last
}

pub(crate) fn chop(
    host: &mut dyn ServerHost,
    level: LevelRef,
    from: Position,
    target: Position,
    owner: u128,
) -> bool {
    if !android_machine::inside_border(host, level, target) {
        return true;
    }
    let logs = Identifier::parse("minecraft:logs").expect("built-in tag");
    let Some(logs) = host
        .tags()
        .key(&logs)
        .and_then(|key| host.tags().blocks(key))
    else {
        return true;
    };
    let Some(log) = last_log(target, |pos| {
        level
            .block(host, pos)
            .is_some_and(|kind| logs.contains(&kind))
    }) else {
        return true;
    };

    if let Some(old) = level.block_state(host, log) {
        level.block_break_effect(host, log, old);
        if level.allow_device_breaking(host, owner, log) {
            let kind = old.block().id(host);
            let sapling = (log[1] == target[1]).then(|| sapling(&kind)).flatten();
            let plant = sapling.is_some_and(|(_, soil)| {
                android_machine::offset(log, [0, -1, 0])
                    .and_then(|below| level.block(host, below))
                    .is_some_and(|below| soil.accepts(&below.id(host)))
            });
            let replacement = if let (true, Some((item, _))) = (plant, sapling) {
                item
            } else {
                "minecraft:air"
            };
            let output = host
                .item_key(&Identifier::parse(&kind).expect("canonical block ID"))
                .and_then(|key| mod_api::ItemStack::new(key, 1));
            let result = host.transaction(|host, _| {
                let mut state = android_machine::data(host, level, from).ok_or(())?;
                let output = output.ok_or(())?;
                android_machine::push(host, &mut state, output);
                if !android_machine::save(host, from, state)
                    || level.block_state(host, log) != Some(old)
                {
                    return Err(());
                }
                let next = host.block_state_id(replacement).ok_or(())?;
                if !level
                    .set_block(
                        host,
                        log,
                        next,
                        BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS,
                    )
                    .map_err(|_| ())?
                {
                    return Err(());
                }
                Ok(())
            });
            if result.is_ok() {
                level.block_break_effect(host, log, old);
                if let Some((item, _)) = sapling.filter(|_| !plant)
                    && let Some(stack) = Kind::Vanilla(item).stack(host, 1)
                {
                    let [x, y, z] = log.map(f64::from);
                    let _ = level.spawn_item(host, x + 0.5, y + 0.5, z + 0.5, stack, 10);
                }
            }
        }
    }
    // Even a blocked log keeps the current CHOP_TREE instruction active.
    false
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Soil {
    Dirt,
    Fungus,
    Mangrove,
}

impl Soil {
    fn accepts(self, soil: &str) -> bool {
        matches!(
            soil,
            "minecraft:dirt"
                | "minecraft:coarse_dirt"
                | "minecraft:grass_block"
                | "minecraft:grass_path"
                | "minecraft:dirt_path"
                | "minecraft:rooted_dirt"
                | "minecraft:mud"
                | "minecraft:farmland"
                | "minecraft:podzol"
                | "minecraft:mycelium"
        ) || match self {
            Self::Dirt => false,
            Self::Mangrove => soil == "minecraft:clay",
            Self::Fungus => matches!(
                soil,
                "minecraft:soul_soil" | "minecraft:crimson_nylium" | "minecraft:warped_nylium"
            ),
        }
    }
}

fn sapling(log: &str) -> Option<(&'static str, Soil)> {
    let kind = log.strip_prefix("minecraft:")?;
    let kind = kind.strip_prefix("stripped_").unwrap_or(kind);
    let (species, structure) = kind.rsplit_once('_')?;
    Some(match (species, structure) {
        ("oak", "log" | "wood") => ("minecraft:oak_sapling", Soil::Dirt),
        ("birch", "log" | "wood") => ("minecraft:birch_sapling", Soil::Dirt),
        ("jungle", "log" | "wood") => ("minecraft:jungle_sapling", Soil::Dirt),
        ("spruce", "log" | "wood") => ("minecraft:spruce_sapling", Soil::Dirt),
        ("acacia", "log" | "wood") => ("minecraft:acacia_sapling", Soil::Dirt),
        ("dark_oak", "log" | "wood") => ("minecraft:dark_oak_sapling", Soil::Dirt),
        ("crimson", "stem" | "hyphae") => ("minecraft:crimson_fungus", Soil::Fungus),
        ("warped", "stem" | "hyphae") => ("minecraft:warped_fungus", Soil::Fungus),
        ("mangrove", "log") => ("minecraft:mangrove_propagule", Soil::Mangrove),
        ("cherry", "log") => ("minecraft:cherry_sapling", Soil::Dirt),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn woodcutter_matches_dough_search_order_and_cutoff() {
        let marks = [[0, 0, 0], [0, 1, 0], [0, 2, 0], [1, 0, -1], [-1, 0, 1]];
        assert_eq!(
            last_log([0, 0, 0], |pos| marks.contains(&pos)),
            Some([-1, 0, 1])
        );
        assert_eq!(
            last_log([0, 0, 0], |[x, y, z]| x == 0
                && z == 0
                && (0..1600).contains(&y)),
            Some([0, 159, 0])
        );
    }

    #[test]
    fn replants_only_paper_species_and_checks_the_original_soil_tags() {
        assert_eq!(
            sapling("minecraft:stripped_dark_oak_wood"),
            Some(("minecraft:dark_oak_sapling", Soil::Dirt))
        );
        assert_eq!(
            sapling("minecraft:crimson_hyphae"),
            Some(("minecraft:crimson_fungus", Soil::Fungus))
        );
        assert_eq!(sapling("minecraft:pale_oak_log"), None);
        assert!(Soil::Mangrove.accepts("minecraft:clay"));
        assert!(Soil::Fungus.accepts("minecraft:warped_nylium"));
        assert!(!Soil::Dirt.accepts("minecraft:soul_soil"));
    }
}
