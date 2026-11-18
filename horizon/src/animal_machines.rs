// SPDX-License-Identifier: GPL-3.0-only

use std::sync::OnceLock;

use mod_api::{
    Host, ItemResource, ItemStack, LevelRef, MobRef, ParticleKey, ParticleOptions, Registrar,
    ServerHost, Tag, Transaction,
};

use crate::{
    food_machines::{self, CAN, FOOD_FABRICATOR, PRODUCE},
    items::{ELECTRIC_MOTOR, GOLD_18K, Kind, LEAD_INGOT, REINFORCED_ALLOY_INGOT, item, text},
    network_recipes::BLISTERING_INGOT_3,
    recipes::{CraftingRecipe, Research, Stack},
};

use super::tree_growth::MENU_SLOTS;

type Position = [i32; 3];

item!(
    AUTO_BREEDER,
    "hay_block",
    "Auto-Breeder",
    "yellow",
    None,
    None
);
item!(
    ANIMAL_GROWTH_ACCELERATOR,
    "hay_block",
    "Animal Growth Accelerator",
    "aqua",
    None,
    None
);

pub(crate) const CAPACITY: u32 = 1024;
const LOVE_DURATION: i32 = 600;
const AGE_INCREMENT: i32 = 2000;

const BREEDING: Research = Research {
    id: "auto_breeder",
    name: "Automated Feeding",
    cost: 25,
};
const GROWTH: Research = Research {
    id: "animal_growth_accelerator",
    name: "Animal Growth Manipulation",
    cost: 32,
};
pub(crate) const RESEARCHES: &[Research] = &[BREEDING, GROWTH];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Operation {
    Breed,
    Grow,
}

#[derive(Clone, Copy)]
pub(crate) struct Machine {
    pub item: Kind,
    pub title: &'static str,
    pub research: &'static Research,
    pub cost: u32,
    operation: Operation,
}

const MACHINES: &[Machine] = &[
    Machine {
        item: AUTO_BREEDER,
        title: "Auto-Breeder",
        research: &BREEDING,
        cost: 60,
        operation: Operation::Breed,
    },
    Machine {
        item: ANIMAL_GROWTH_ACCELERATOR,
        title: "Animal Growth Accelerator",
        research: &GROWTH,
        cost: 14,
        operation: Operation::Grow,
    },
];

pub(crate) fn machine(id: &str) -> Option<&'static Machine> {
    MACHINES.iter().find(|machine| machine.item.id() == id)
}

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    CraftingRecipe {
        inputs: [
            Some(GOLD_18K),
            Some(CAN),
            Some(GOLD_18K),
            Some(ELECTRIC_MOTOR),
            Some(Kind::Vanilla("minecraft:hay_block")),
            Some(ELECTRIC_MOTOR),
            Some(LEAD_INGOT),
            Some(FOOD_FABRICATOR),
            Some(LEAD_INGOT),
        ],
        output: Stack {
            kind: AUTO_BREEDER,
            count: 1,
        },
        research: &BREEDING,
    },
    CraftingRecipe {
        inputs: [
            None,
            Some(BLISTERING_INGOT_3),
            None,
            Some(ELECTRIC_MOTOR),
            Some(food_machines::WHEAT.food),
            Some(ELECTRIC_MOTOR),
            Some(REINFORCED_ALLOY_INGOT),
            Some(AUTO_BREEDER),
            Some(REINFORCED_ALLOY_INGOT),
        ],
        output: Stack {
            kind: ANIMAL_GROWTH_ACCELERATOR,
            count: 1,
        },
        research: &GROWTH,
    },
];

static HEART: OnceLock<ParticleKey> = OnceLock::new();
static HAPPY_VILLAGER: OnceLock<ParticleKey> = OnceLock::new();

pub(super) fn register(registrar: &mut dyn Registrar) {
    let _ = HEART.set(registrar.particle_key("minecraft:heart"));
    let _ = HAPPY_VILLAGER.set(registrar.particle_key("minecraft:happy_villager"));
}

pub(crate) fn lore(id: &str) -> Option<Tag> {
    let machine = machine(id)?;
    let power = |amount| {
        Tag::compound([
            ("text", Tag::String("".into())),
            ("italic", Tag::Byte(0)),
            (
                "extra",
                Tag::List(vec![
                    text("⇨ ", "dark_gray", false),
                    text("⚡ ", "yellow", false),
                    text(amount, "gray", false),
                ]),
            ),
        ])
    };
    Some(Tag::List(vec![
        Tag::String("".into()),
        Tag::compound([
            ("text", Tag::String("Runs on ".into())),
            ("color", Tag::String("white".into())),
            ("italic", Tag::Byte(0)),
            (
                "extra",
                Tag::List(vec![text("Organic Food", "green", false)]),
            ),
        ]),
        Tag::String("".into()),
        text("End-Game Machine", "dark_purple", false),
        power("1024 J Buffer"),
        power(match machine.operation {
            Operation::Breed => "60 J/Animal",
            Operation::Grow => "28 J/s",
        }),
    ]))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Change {
    Love,
    Age { from: i32, to: i32 },
    BecomeAdult,
}

#[derive(Clone, Copy)]
pub(crate) struct Feeding {
    operation: Operation,
    mob: MobRef,
    position: Position,
    change: Change,
}

impl Machine {
    pub(crate) fn stage(
        self,
        host: &mut dyn ServerHost,
        tx: &Transaction,
        level: LevelRef,
        pos: Position,
    ) -> Result<Feeding, ()> {
        let (slot, food) = (0..MENU_SLOTS.len())
            .find_map(|slot| {
                let place = super::place(level, pos, slot);
                let stack = place.stack(host).filter(|stack| is_food(host, stack))?;
                Some((place, stack))
            })
            .ok_or(())?;
        let [min, max] = self.operation.bounds(pos);
        let found = level
            .entities_in(host, min, max)
            .ok_or(())?
            .into_iter()
            .find_map(|entity| {
                let mob = host.mob(entity)?;
                let change = self.operation.change(host, mob)?;
                let position = entity.block_position(host)?;
                if !level.allow_inventory_transfer(host, pos, position, &food) {
                    return None;
                }
                Some(Feeding {
                    operation: self.operation,
                    mob,
                    change,
                    position,
                })
            })
            .ok_or(())?;
        (slot.extract(host, tx, &ItemResource::of(&food), 1) == 1)
            .then_some(found)
            .ok_or(())
    }
}

fn is_food(host: &dyn Host, stack: &ItemStack) -> bool {
    PRODUCE
        .iter()
        .any(|produce| produce.food.matches(host, stack))
}

impl Operation {
    fn bounds(self, position: Position) -> [[f64; 3]; 2] {
        let radius = match self {
            Self::Breed => [4.0, 2.0, 4.0],
            Self::Grow => [3.0; 3],
        };
        [-1.0, 1.0]
            .map(|sign| std::array::from_fn(|axis| f64::from(position[axis]) + sign * radius[axis]))
    }

    fn change(self, host: &dyn Host, mob: MobRef) -> Option<Change> {
        if !mob.living().health(host).is_some_and(|health| health > 0.0) {
            return None;
        }
        let species = mob.entity().kind(host)?.id(host);
        let age = mob.age_ticks(host);
        match self {
            Self::Breed => decision(self, age, mob.love_ticks(host), false),
            Self::Grow
                if matches!(species.as_str(), "minecraft:slime" | "minecraft:magma_cube") =>
            {
                None
            }
            Self::Grow => decision(
                self,
                age,
                None,
                age.is_none()
                    && paper_binary_age(&species)
                    && mob.living().baby(host) == Some(true),
            ),
        }
    }
}

/// Paper exposes these mobs through Ageable, with ages -1 for babies and 0 for adults.
fn paper_binary_age(species: &str) -> bool {
    matches!(
        species,
        "minecraft:zombie"
            | "minecraft:husk"
            | "minecraft:drowned"
            | "minecraft:zombie_villager"
            | "minecraft:zombified_piglin"
            | "minecraft:piglin"
            | "minecraft:zoglin"
    )
}

fn decision(
    operation: Operation,
    age: Option<i32>,
    love: Option<i32>,
    binary_baby: bool,
) -> Option<Change> {
    match operation {
        Operation::Breed if age == Some(0) && love == Some(0) => Some(Change::Love),
        Operation::Grow => match age {
            Some(age @ ..0) => Some(Change::Age {
                from: age,
                to: (age + AGE_INCREMENT).min(0),
            }),
            None if binary_baby => Some(Change::BecomeAdult),
            _ => None,
        },
        _ => None,
    }
}

impl Feeding {
    /// Write entity state last so refusals can still roll back food and charge.
    pub(crate) fn apply(self, host: &mut dyn ServerHost) -> Result<(), ()> {
        if self.mob.entity().block_position(host) != Some(self.position)
            || self.operation.change(host, self.mob) != Some(self.change)
        {
            return Err(());
        }
        match self.change {
            Change::Love => self.mob.set_love_ticks(host, LOVE_DURATION),
            Change::Age { to, .. } => self.mob.set_age_ticks(host, to),
            Change::BecomeAdult => self.mob.set_baby(host, false),
        }
        .then_some(())
        .ok_or(())
    }

    pub(crate) fn particles(self, host: &mut dyn ServerHost, level: LevelRef) {
        let particle = match self.operation {
            Operation::Breed => HEART.get(),
            Operation::Grow => HAPPY_VILLAGER.get(),
        };
        if let (Some(&particle), Some(eyes)) = (particle, self.mob.entity().eye_position(host)) {
            level.send_particles(
                host,
                &ParticleOptions::simple(particle),
                None,
                eyes,
                [0.2; 3],
                0.0,
                8,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::testing::FakeHost;

    #[test]
    fn species_readiness_and_aabb_follow_paper_bukkit_age() {
        assert_eq!(
            Operation::Breed.bounds([2, 67, -5]),
            [[-2.0, 65.0, -9.0], [6.0, 69.0, -1.0]],
        );
        assert_eq!(
            Operation::Grow.bounds([2, 67, -5]),
            [[-1.0, 64.0, -8.0], [5.0, 70.0, -2.0]],
        );
        assert_eq!(
            decision(Operation::Breed, Some(0), Some(0), false),
            Some(Change::Love)
        );
        for (age, love) in [
            (None, Some(0)),
            (Some(-1), Some(0)),
            (Some(550), Some(0)),
            (Some(0), None),
            (Some(0), Some(599)),
        ] {
            assert_eq!(decision(Operation::Breed, age, love, false), None);
        }
        assert_eq!(
            decision(Operation::Grow, Some(-24000), None, false),
            Some(Change::Age {
                from: -24000,
                to: -22000
            })
        );
        assert_eq!(
            decision(Operation::Grow, Some(-1999), None, false),
            Some(Change::Age { from: -1999, to: 0 })
        );
        assert_eq!(decision(Operation::Grow, Some(1), None, false), None);
        assert_eq!(
            decision(Operation::Grow, None, None, true),
            Some(Change::BecomeAdult)
        );
        assert!(paper_binary_age("minecraft:zombie_villager"));
        assert!(paper_binary_age("minecraft:zoglin"));
        assert!(!paper_binary_age("minecraft:tadpole"));
    }

    #[test]
    fn the_eleven_paper_food_variants_feed_both_machines() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([mod_api::Identifier::parse("minecraft:player_head").unwrap()])
            .unwrap();
        assert_eq!(PRODUCE.len(), 11);
        for produce in PRODUCE {
            assert!(
                is_food(&host, &produce.food.stack(&host, 1).unwrap()),
                "{}",
                produce.food.id()
            );
            assert!(
                !is_food(&host, &produce.fertilizer.stack(&host, 1).unwrap()),
                "{}",
                produce.fertilizer.id()
            );
        }
        for kind in [CAN, Kind::Vanilla("minecraft:player_head")] {
            assert!(!is_food(&host, &kind.stack(&host, 1).unwrap()));
        }
    }
}
