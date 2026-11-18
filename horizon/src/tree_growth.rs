// SPDX-License-Identifier: GPL-3.0-only

use std::sync::OnceLock;

use mod_api::{
    ItemResource, ItemStack, LevelRef, ParticleKey, ParticleOptions, Registrar, ServerHost,
    Transaction,
};

use crate::{
    food_machines::PRODUCE,
    heated_pressure::MAGNESIUM_SALT,
    items::{CARBONADO, ELECTRIC_MOTOR, Kind, item},
    network_recipes::BIG_CAPACITOR,
    recipes::{CraftingRecipe, Research, Stack},
};

item!(
    TREE_GROWTH_ACCELERATOR,
    "brown_terracotta",
    "Tree Growth Accelerator",
    "green",
    None,
    None
);

pub(crate) const COST: u32 = 24;
pub(crate) const CAPACITY: u32 = 1024;
const RADIUS: i32 = 9;
pub(crate) const MENU_SLOTS: &[usize] = &[10, 11, 12, 13, 14, 15, 16];
pub(crate) const RESEARCH: Research = Research {
    id: "tree_growth_accelerator",
    name: "Faster Trees",
    cost: 18,
};

pub(crate) const CRAFTING: CraftingRecipe = CraftingRecipe {
    inputs: [
        None,
        Some(CARBONADO),
        None,
        Some(ELECTRIC_MOTOR),
        Some(Kind::Vanilla("minecraft:diamond_axe")),
        Some(ELECTRIC_MOTOR),
        Some(MAGNESIUM_SALT),
        Some(BIG_CAPACITOR),
        Some(MAGNESIUM_SALT),
    ],
    output: Stack {
        kind: TREE_GROWTH_ACCELERATOR,
        count: 1,
    },
    research: &RESEARCH,
};

static HAPPY_VILLAGER: OnceLock<ParticleKey> = OnceLock::new();

pub(crate) fn register(registrar: &mut dyn Registrar) {
    let _ = HAPPY_VILLAGER.set(registrar.particle_key("minecraft:happy_villager"));
}

pub(crate) fn stage(
    host: &mut dyn ServerHost,
    tx: &Transaction,
    level: LevelRef,
    machine: [i32; 3],
) -> Option<[i32; 3]> {
    let (input, fertilizer) = (0..MENU_SLOTS.len()).find_map(|i| {
        let place = super::place(level, machine, i);
        let stack = place
            .stack(host)
            .filter(|stack| is_fertilizer(host, stack))?;
        Some((place, stack))
    })?;
    let sapling = horizontal_square(machine).find(|&target| {
        is_sapling(host, level, target)
            && level.allow_inventory_transfer(host, machine, target, &fertilizer)
            && is_sapling(host, level, target)
    })?;
    (input.extract(host, tx, &ItemResource::of(&fertilizer), 1) == 1).then_some(sapling)
}

pub(crate) fn grow(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3]) {
    if !is_sapling(host, level, pos) {
        return;
    }
    let _ = level.bone_meal(host, pos);
    if let Some(&particle) = HAPPY_VILLAGER.get() {
        level.send_particles(
            host,
            &ParticleOptions::simple(particle),
            None,
            pos.map(|axis| f64::from(axis) + 0.5),
            [0.1; 3],
            0.0,
            4,
        );
    }
}

fn is_sapling(host: &dyn ServerHost, level: LevelRef, pos: [i32; 3]) -> bool {
    level
        .block(host, pos)
        .is_some_and(|block| host.block_in_tag(block, "minecraft:saplings") == Some(true))
}

fn horizontal_square([x, y, z]: [i32; 3]) -> impl Iterator<Item = [i32; 3]> {
    (-RADIUS..=RADIUS)
        .filter_map(move |offset| x.checked_add(offset))
        .flat_map(move |x| {
            (-RADIUS..=RADIUS).filter_map(move |offset| Some([x, y, z.checked_add(offset)?]))
        })
}

fn is_fertilizer(host: &dyn ServerHost, stack: &ItemStack) -> bool {
    PRODUCE
        .iter()
        .any(|produce| produce.fertilizer.matches(host, stack))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_growth_targets_the_first_sapling_on_the_paper_horizontal_square() {
        let positions = horizontal_square([0, 67, 0]).collect::<Vec<_>>();
        assert_eq!(positions.len(), 361);
        assert_eq!(positions.first(), Some(&[-9, 67, -9]));
        assert_eq!(positions[18..20], [[-9, 67, 9], [-8, 67, -9]]);
        assert_eq!(positions.last(), Some(&[9, 67, 9]));
    }
}
