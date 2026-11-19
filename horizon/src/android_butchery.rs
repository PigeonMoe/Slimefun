// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, sync::Mutex};

use mod_api::{
    BlockEntityRef, EntityRef, Host, HostTransaction, LevelRef, RemovalReason, ServerHost,
};

use crate::{
    android_machine::{self, Position},
    android_program::{Facing, Tier},
    items::Kind,
    random::Rolls,
};

#[derive(Clone, Copy)]
pub(crate) enum Target {
    All,
    Monsters,
    Animals,
    AdultAnimals,
}

impl Target {
    fn allows(self, kind: &str, baby: bool) -> bool {
        match self {
            Self::All => !matches!(kind, "minecraft:player" | "minecraft:armor_stand"),
            Self::Monsters => MONSTERS.binary_search(&kind).is_ok(),
            Self::Animals | Self::AdultAnimals => {
                ANIMALS.binary_search(&kind).is_ok()
                    && (!matches!(self, Self::AdultAnimals) || !baby)
            }
        }
    }
}

struct Hit {
    level: LevelRef,
    android: Position,
    head: BlockEntityRef,
    victim: EntityRef,
    kind: String,
    at: [f64; 3],
    bounds: [[f64; 3]; 2],
}

struct Death {
    hit: Hit,
    due: i64,
}

#[derive(Default)]
pub(crate) struct Butchers {
    hits: Mutex<BTreeMap<u32, Hit>>,
    deaths: Mutex<Vec<Death>>,
}

impl Butchers {
    pub fn attack(
        &self,
        host: &mut dyn ServerHost,
        level: LevelRef,
        android: Position,
        facing: Facing,
        tier: Tier,
        target: Target,
    ) -> Option<()> {
        let head = host.block_entity(level, android)?;
        let origin = android.map(f64::from);
        let (radius, damage) = stats(tier);
        let min = origin.map(|coord| coord - radius);
        let max = origin.map(|coord| coord + radius);
        let (victim, kind, at, bounds) =
            level
                .entities_in(host, min, max)?
                .into_iter()
                .find_map(|entity| {
                    let living = host.living(entity)?;
                    let kind = entity.kind(host)?.id(host);
                    let baby = living.baby(host) == Some(true);
                    let at = entity.position(host)?;
                    let bounds = entity.bounding_box(host)?;
                    (living.health(host)? > 0.0
                        && target.allows(&kind, baby)
                        && faces(facing, origin, at))
                    .then_some((entity, kind, at, bounds))
                })?;
        self.hits.lock().unwrap().insert(
            victim.id(),
            Hit {
                level,
                android,
                head,
                victim,
                kind,
                at,
                bounds,
            },
        );
        // Paper damages a living target without a player or a direct attacking entity.
        let _ = victim.hurt(host, "minecraft:generic", damage, None, None);
        Some(())
    }

    pub fn death(&self, host: &dyn Host, id: u32) {
        let Some(mut hit) = self.hits.lock().unwrap().remove(&id) else {
            return;
        };
        if let Some(at) = hit.victim.position(host) {
            hit.at = at;
        }
        if let Some(bounds) = hit.victim.bounding_box(host) {
            hit.bounds = bounds;
        }
        let Some(due) = hit
            .level
            .game_time(host)
            .and_then(|time| time.checked_add(1))
        else {
            return;
        };
        self.deaths.lock().unwrap().push(Death { hit, due });
    }

    pub fn tick(&self, host: &mut dyn ServerHost, level: LevelRef, rolls: &Rolls) {
        let Some(time) = level.game_time(host) else {
            return;
        };
        if time % 20 == 0 {
            self.hits
                .lock()
                .unwrap()
                .retain(|_, hit| hit.level != level || hit.victim.position(host).is_some());
        }
        let ready: Vec<_> = self
            .deaths
            .lock()
            .unwrap()
            .extract_if(.., |death| death.hit.level == level && death.due <= time)
            .collect();
        for death in ready {
            collect(host, level, rolls, death);
        }
    }

    pub fn unload(&self, host: &dyn Host, dimension: &str) {
        self.hits
            .lock()
            .unwrap()
            .retain(|_, hit| hit.level.dimension().id(host).as_deref() != Some(dimension));
        self.deaths
            .lock()
            .unwrap()
            .retain(|death| death.hit.level.dimension().id(host).as_deref() != Some(dimension));
    }

    pub fn clear(&self) {
        self.hits.lock().unwrap().clear();
        self.deaths.lock().unwrap().clear();
    }
}

fn stats(tier: Tier) -> (f64, f32) {
    match tier {
        Tier::Solid => (5.0, 4.0),
        Tier::Liquid => (6.0, 8.0),
        Tier::Nuclear => (7.0, 20.0),
    }
}

fn faces(facing: Facing, origin: [f64; 3], at: [f64; 3]) -> bool {
    match facing {
        Facing::North => at[2] < origin[2],
        Facing::East => at[0] > origin[0],
        Facing::South => at[2] > origin[2],
        Facing::West => at[0] < origin[0],
    }
}

fn collect(host: &mut dyn ServerHost, level: LevelRef, rolls: &Rolls, death: Death) {
    let Hit {
        android,
        head,
        at: [x, y, z],
        bounds,
        ..
    } = death.hit;
    if host.block_entity(level, android) == Some(head) {
        let min = bounds[0].map(|coord| coord - 0.5);
        let max = bounds[1].map(|coord| coord + 0.5);
        let nearby = level.entities_in(host, min, max).unwrap_or_default();
        let extra = extra_drop(host, level, rolls, &death.hit.kind);
        let _ = host.transaction(|host, _| {
            let mut machine = android_machine::data(host, level, android).ok_or(())?;
            for entity in nearby {
                let Some(item) = host.item_entity(entity) else {
                    continue;
                };
                if item.pickup_delay(host) == Some(32767) {
                    continue;
                }
                if let Some(stack) = item.stack(host)
                    && entity.remove(host, RemovalReason::Discarded)
                {
                    android_machine::push(host, &mut machine, stack);
                }
            }
            if let Some(stack) = extra {
                android_machine::push(host, &mut machine, stack);
            }
            android_machine::save(host, android, machine)
                .then_some(())
                .ok_or(())
        });
    }
    if let Some(amount) = rolls.next(host, level, 6) {
        level.award_experience(host, x, y, z, amount + 1);
    }
}

fn extra_drop(
    host: &dyn Host,
    level: LevelRef,
    rolls: &Rolls,
    kind: &str,
) -> Option<mod_api::ItemStack> {
    let (kind, amount) = match kind {
        "minecraft:wither_skeleton" if rolls.next(host, level, 250)? < 2 => {
            ("minecraft:wither_skeleton_skull", 1)
        }
        "minecraft:blaze" => ("minecraft:blaze_rod", 1),
        "minecraft:vindicator" => ("minecraft:emerald", 1 + rolls.next(host, level, 2)?),
        _ => return None,
    };
    Kind::Vanilla(kind).stack(host, amount)
}

// The Bukkit Monster and Animals interfaces in Paper 26.2 build 132.
const MONSTERS: &[&str] = &[
    "minecraft:blaze",
    "minecraft:bogged",
    "minecraft:breeze",
    "minecraft:cave_spider",
    "minecraft:creaking",
    "minecraft:creeper",
    "minecraft:drowned",
    "minecraft:elder_guardian",
    "minecraft:enderman",
    "minecraft:endermite",
    "minecraft:evoker",
    "minecraft:giant",
    "minecraft:guardian",
    "minecraft:husk",
    "minecraft:illusioner",
    "minecraft:parched",
    "minecraft:piglin",
    "minecraft:piglin_brute",
    "minecraft:pillager",
    "minecraft:ravager",
    "minecraft:silverfish",
    "minecraft:skeleton",
    "minecraft:spider",
    "minecraft:stray",
    "minecraft:vex",
    "minecraft:vindicator",
    "minecraft:warden",
    "minecraft:witch",
    "minecraft:wither",
    "minecraft:wither_skeleton",
    "minecraft:zoglin",
    "minecraft:zombie",
    "minecraft:zombie_villager",
    "minecraft:zombified_piglin",
];
const ANIMALS: &[&str] = &[
    "minecraft:armadillo",
    "minecraft:axolotl",
    "minecraft:bee",
    "minecraft:camel",
    "minecraft:camel_husk",
    "minecraft:cat",
    "minecraft:chicken",
    "minecraft:cow",
    "minecraft:donkey",
    "minecraft:fox",
    "minecraft:frog",
    "minecraft:goat",
    "minecraft:happy_ghast",
    "minecraft:hoglin",
    "minecraft:horse",
    "minecraft:llama",
    "minecraft:mooshroom",
    "minecraft:mule",
    "minecraft:nautilus",
    "minecraft:ocelot",
    "minecraft:panda",
    "minecraft:parrot",
    "minecraft:pig",
    "minecraft:polar_bear",
    "minecraft:rabbit",
    "minecraft:sheep",
    "minecraft:skeleton_horse",
    "minecraft:sniffer",
    "minecraft:strider",
    "minecraft:trader_llama",
    "minecraft:turtle",
    "minecraft:wolf",
    "minecraft:zombie_horse",
    "minecraft:zombie_nautilus",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn butcher_matches_the_paper_tiers_and_forward_half_of_its_box() {
        assert_eq!(
            [Tier::Solid, Tier::Liquid, Tier::Nuclear].map(stats),
            [(5.0, 4.0), (6.0, 8.0), (7.0, 20.0)]
        );
        let center = [10.0, 64.0, -20.0];
        assert!(faces(Facing::North, center, [10.0, 64.0, -20.01]));
        assert!(!faces(Facing::North, center, [0.0, 64.0, -20.0]));
        assert!(faces(Facing::East, center, [10.001, 64.0, -20.0]));
        assert!(faces(Facing::West, center, [9.999, 64.0, -20.0]));
        assert!(faces(Facing::South, center, [10.0, 64.0, -19.99]));
    }

    #[test]
    fn target_filters_are_paper_interfaces_and_age_rules_not_spawn_categories() {
        assert_eq!(MONSTERS.len() + ANIMALS.len(), 68);
        assert!(Target::Monsters.allows("minecraft:breeze", false));
        assert!(!Target::Monsters.allows("minecraft:ghast", false));
        assert!(!Target::Monsters.allows("minecraft:hoglin", true));
        assert!(Target::Animals.allows("minecraft:hoglin", true));
        assert!(!Target::AdultAnimals.allows("minecraft:hoglin", true));
        assert!(Target::AdultAnimals.allows("minecraft:hoglin", false));
        assert!(Target::All.allows("minecraft:ghast", false));
        assert!(!Target::All.allows("minecraft:armor_stand", false));
        assert!(!Target::All.allows("minecraft:player", false));
    }
}
