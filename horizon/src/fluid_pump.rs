// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{
    BlockUpdateFlags, Compound, Host, ItemResource, ItemStack, LevelRef, ServerHost, Tag,
    Transaction,
};

use crate::{
    geology::OIL_PUMP,
    items::{Kind, item},
    network_recipes::{CARGO_MOTOR, MEDIUM_CAPACITOR},
    random::Rolls,
    recipes::{CraftingRecipe, Research, Stack},
};

use super::{INPUT, OUTPUT, Position, Storage, data_at, place, stack_in};

item!(
    FLUID_PUMP,
    "blue_terracotta",
    "Fluid Pump",
    "dark_blue",
    None,
    None
);

const BUCKET: Kind = Kind::Vanilla("minecraft:bucket");
const BOTTLE: Kind = Kind::Vanilla("minecraft:glass_bottle");
pub(crate) const COST: u32 = 32;
pub(crate) const CAPACITY: u32 = 512;

pub(crate) const RESEARCH: Research = Research {
    id: "fluid_pump",
    name: "Fluid Pump",
    cost: 28,
};

pub(crate) const CRAFTING: CraftingRecipe = CraftingRecipe {
    inputs: [
        None,
        Some(MEDIUM_CAPACITOR),
        None,
        Some(BUCKET),
        Some(CARGO_MOTOR),
        Some(BUCKET),
        None,
        Some(OIL_PUMP),
        None,
    ],
    output: Stack {
        kind: FLUID_PUMP,
        count: 1,
    },
    research: &RESEARCH,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Fluid {
    Water,
    Lava,
}

impl Fluid {
    fn at(host: &dyn Host, level: LevelRef, pos: Position) -> Option<(Self, bool)> {
        if !level.is_loaded(host, pos) {
            return None;
        }
        let block = level.block_state(host, pos)?.block().id(host);
        let fluid = match block.as_str() {
            "minecraft:water" => Self::Water,
            "minecraft:lava" => Self::Lava,
            _ => return None,
        };
        Some((fluid, level.fluid_state(host, pos)?.is_source()))
    }

    fn output(self, host: &dyn Host, bottle: bool) -> Option<ItemStack> {
        let kind = match (self, bottle) {
            (Self::Water, false) => Kind::Vanilla("minecraft:water_bucket"),
            (Self::Lava, false) => Kind::Vanilla("minecraft:lava_bucket"),
            (Self::Lava, true) => BOTTLE,
            (Self::Water, true) => {
                let mut potion = Kind::Vanilla("minecraft:potion").stack(host, 1)?;
                potion
                    .apply_components(&Compound::from([(
                        "minecraft:potion_contents".into(),
                        Tag::compound([("potion", Tag::String("minecraft:water".into()))]),
                    )]))
                    .ok()?;
                return Some(potion);
            }
        };
        kind.stack(host, 1)
    }
}

pub(crate) struct Drain(Fluid, Position);

impl Drain {
    // World edits take effect immediately, so the caller drains only once all
    // transactional inventory and charge writes have succeeded.
    pub(crate) fn apply(self, host: &mut dyn ServerHost, level: LevelRef) -> Result<(), ()> {
        if Fluid::at(host, level, self.1) != Some((self.0, true)) {
            return Err(());
        }
        let air = host.block_state_id("minecraft:air").ok_or(())?;
        matches!(
            level.set_block(
                host,
                self.1,
                air,
                BlockUpdateFlags::CLIENTS | BlockUpdateFlags::NEIGHBORS
            ),
            Ok(true)
        )
        .then_some(())
        .ok_or(())
    }
}

pub(crate) fn stage(
    host: &mut dyn ServerHost,
    tx: &Transaction,
    level: LevelRef,
    pos: Position,
    rolls: &Rolls,
) -> Result<Option<Drain>, ()> {
    let below = [pos[0], pos[1] - 1, pos[2]];
    let (fluid, is_source) = Fluid::at(host, level, below).ok_or(())?;
    let data = data_at(
        host,
        Storage {
            level,
            pos,
            plain: true,
        },
    )
    .ok_or(())?;
    let (slot, input, bottle) = INPUT
        .into_iter()
        .find_map(|slot| {
            let input = stack_in(host, &data, slot)?;
            if BUCKET.matches(host, &input) {
                Some((slot, input, false))
            } else if BOTTLE.matches(host, &input) {
                Some((slot, input, true))
            } else {
                None
            }
        })
        .ok_or(())?;
    let output = fluid.output(host, bottle).ok_or(())?;
    let source = match fluid {
        Fluid::Water => is_source.then_some(below),
        Fluid::Lava => lava_source(below, |pos| {
            let (kind, source) = Fluid::at(host, level, pos)?;
            (kind == Fluid::Lava).then_some(source)
        }),
    }
    .ok_or(())?;
    let resource = ItemResource::of(&output);
    if !OUTPUT
        .into_iter()
        .any(|output| place(level, pos, output).insert(host, tx, &resource, 1) == 1)
        || place(level, pos, slot).extract(host, tx, &ItemResource::of(&input), 1) != 1
    {
        return Err(());
    }
    let removes_source = !bottle || rolls.next(host, level, 100).is_some_and(|roll| roll < 30);
    Ok(removes_source.then_some(Drain(fluid, source)))
}

fn lava_source(
    start: Position,
    mut cell: impl FnMut(Position) -> Option<bool>,
) -> Option<Position> {
    // Dough's Vein.find appends up to 42 cells in depth-first order, then the
    // Paper pump selects the last source in that ordered list.
    fn expand(
        pos: Position,
        cell: &mut impl FnMut(Position) -> Option<bool>,
        visited: &mut Vec<(Position, bool)>,
    ) {
        const LIMIT: usize = 42;
        const FACES: [Position; 10] = [
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
        if visited.len() == LIMIT || visited.iter().any(|&(seen, _)| seen == pos) {
            return;
        }
        let Some(source) = cell(pos) else { return };
        visited.push((pos, source));
        for [dx, dy, dz] in FACES {
            expand([pos[0] + dx, pos[1] + dy, pos[2] + dz], cell, visited);
        }
    }
    let mut visited = Vec::new();
    expand(start, &mut cell, &mut visited);
    visited
        .into_iter()
        .rev()
        .find_map(|(pos, source)| source.then_some(pos))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Identifier, testing::FakeHost};

    #[test]
    fn lava_search_takes_the_last_reached_source_and_limits_to_42_cells() {
        let cells = |[x, y, z]: Position| match (x, y, z) {
            (0, 0..=41, 0) => Some(false),
            (0, 42, 0) | (1, 0, 0) => Some(true),
            _ => None,
        };
        assert_eq!(lava_source([0; 3], cells), None);
        let choices = |pos: Position| match pos {
            [0, 0, 0] => Some(false),
            [0, 1, 0] | [-1, 0, 1] => Some(true),
            _ => None,
        };
        assert_eq!(lava_source([0; 3], choices), Some([-1, 0, 1]));
    }

    #[test]
    fn bottles_use_the_paper_water_potion_and_lava_fallback() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([
                Identifier::parse("minecraft:potion").unwrap(),
                Identifier::parse("minecraft:glass_bottle").unwrap(),
            ])
            .unwrap();
        let water = Fluid::Water.output(&host, true).unwrap();
        assert_eq!(
            water
                .components()
                .unwrap()
                .as_nbt()
                .get(&"minecraft:potion_contents".into()),
            Some(&Tag::compound([(
                "potion",
                Tag::String("minecraft:water".into())
            )]),)
        );
        assert_eq!(Fluid::Lava.output(&host, true), BOTTLE.stack(&host, 1));
    }
}
