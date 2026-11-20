// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::BTreeMap,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicI32, AtomicU8, Ordering},
    },
};

use mod_api::{
    ActiveEffect, ArrowContactPostEvent, ArrowHitArmorStandPostEvent, Compound,
    EntityShootBowEvent, Hand, Host, HurtSource, ItemKey, ItemPlace, ItemStack, LevelRef,
    LivingDamagePostEvent, LivingIncomingDamageEvent, ParticleKey, ProjectileImpactEvent,
    Registrar, ServerHost, SoundSource, Tag,
};

use crate::{
    elemental_staves::{STAFF_ELEMENTAL_FIRE, STAFF_ELEMENTAL_WATER},
    items::{Kind, SULFATE, item, item_id, text},
    random::Rolls,
    recipes::{CraftingRecipe, Research, Stack},
    research,
};

item!(
    BLADE_OF_VAMPIRES,
    "golden_sword",
    "Blade of Vampires",
    "red",
    None,
    None
);
item!(ICY_BOW, "bow", "Icy Bow", "aqua", None, None);
item!(EXPLOSIVE_BOW, "bow", "Explosive Bow", "red", None, None);

#[path = "explosive_bow.rs"]
mod explosive_bow;
use explosive_bow::Blast;

pub(crate) const VAMPIRES: Research = Research {
    id: "blade_of_vampires",
    name: "Blade of Vampires",
    cost: 26,
};
pub(crate) const BOWS: Research = Research {
    id: "special_bows",
    name: "Robin Hood",
    cost: 22,
};

const STICK: Option<Kind> = Some(Kind::Vanilla("minecraft:stick"));
const ICE: Option<Kind> = Some(Kind::Vanilla("minecraft:ice"));
const SKULL: Option<Kind> = Some(Kind::Vanilla("minecraft:wither_skeleton_skull"));
const GUNPOWDER: Option<Kind> = Some(Kind::Vanilla("minecraft:gunpowder"));

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    CraftingRecipe {
        inputs: [
            None,
            SKULL,
            None,
            None,
            SKULL,
            None,
            None,
            Some(Kind::Vanilla("minecraft:blaze_rod")),
            None,
        ],
        output: Stack {
            kind: BLADE_OF_VAMPIRES,
            count: 1,
        },
        research: &VAMPIRES,
    },
    CraftingRecipe {
        inputs: [
            None,
            STICK,
            ICE,
            Some(STAFF_ELEMENTAL_WATER),
            None,
            Some(Kind::Vanilla("minecraft:packed_ice")),
            None,
            STICK,
            ICE,
        ],
        output: Stack {
            kind: ICY_BOW,
            count: 1,
        },
        research: &BOWS,
    },
    CraftingRecipe {
        inputs: [
            None,
            STICK,
            GUNPOWDER,
            Some(STAFF_ELEMENTAL_FIRE),
            None,
            Some(SULFATE),
            None,
            STICK,
            GUNPOWDER,
        ],
        output: Stack {
            kind: EXPLOSIVE_BOW,
            count: 1,
        },
        research: &BOWS,
    },
];

pub(crate) fn decorate(id: &str, components: &mut Compound) {
    let lore = match id {
        "BLADE_OF_VAMPIRES" => {
            components.insert(
                "minecraft:enchantments".into(),
                Tag::compound([
                    ("minecraft:fire_aspect", Tag::Int(2)),
                    ("minecraft:unbreaking", Tag::Int(4)),
                    ("minecraft:sharpness", Tag::Int(2)),
                ]),
            );
            vec![
                text("Life Steal I", "gray", false),
                Tag::String("".into()),
                text("Everytime you attack something", "white", false),
                text("you have a 45% chance to", "white", false),
                text("recover 2 Hearts of your Health", "white", false),
            ]
        }
        "ICY_BOW" => [
            "Any Arrows fired using this Bow",
            "will prevent hit enemys from moving",
            "for 2 seconds",
        ]
        .into_iter()
        .map(|line| text(line, "white", false))
        .collect(),
        "EXPLOSIVE_BOW" => [
            "Any Arrows fired using this Bow",
            "will launch hit enemys into the air",
        ]
        .into_iter()
        .map(|line| text(line, "white", false))
        .collect(),
        _ => return,
    };
    components.insert("minecraft:lore".into(), Tag::List(lore));
}

#[derive(Clone, Copy)]
enum Reaction {
    Heal(u32),
    Freeze { level: LevelRef, victim: u32 },
    Explode(Blast),
}

type Hit = (i32, i32, i32);

fn hit(victim: i32, source: &HurtSource<'_>) -> Hit {
    (victim, source.causing, source.direct)
}

struct Shot {
    level: LevelRef,
    expires: Option<i64>,
}

#[derive(Default)]
struct State {
    shots: BTreeMap<u32, Shot>,
    tentative: BTreeMap<Hit, Reaction>,
    ready: Vec<Reaction>,
}

pub(crate) struct Weapons {
    state: Mutex<State>,
    sword: OnceLock<ItemKey>,
    bow: OnceLock<ItemKey>,
    blast_particle: OnceLock<ParticleKey>,
    chance: AtomicU8,
    blast_range: AtomicI32,
}

impl Default for Weapons {
    fn default() -> Self {
        Self {
            state: Mutex::new(State::default()),
            sword: OnceLock::new(),
            bow: OnceLock::new(),
            blast_particle: OnceLock::new(),
            chance: AtomicU8::new(45),
            blast_range: AtomicI32::new(3),
        }
    }
}

fn tagged(weapon: Option<&ItemStack>, item: Option<&ItemKey>, id: &str) -> bool {
    weapon.is_some_and(|stack| Some(&stack.item()) == item && item_id(stack).as_deref() == Some(id))
}

impl Weapons {
    pub(crate) fn register(&self, registrar: &mut dyn Registrar) {
        let _ = self
            .sword
            .set(registrar.item_key(BLADE_OF_VAMPIRES.material()));
        let _ = self.bow.set(registrar.item_key(ICY_BOW.material()));
        let _ = self
            .blast_particle
            .set(registrar.particle_key("minecraft:explosion"));
    }

    pub(crate) fn configure(&self, chance: u8) {
        self.chance.store(chance, Ordering::Relaxed);
    }

    pub(crate) fn configure_blast(&self, range: Option<i32>) {
        self.blast_range.store(
            range.filter(|&range| range > 0).unwrap_or(3),
            Ordering::Relaxed,
        );
    }

    pub(crate) fn shot(&self, host: &mut dyn ServerHost, event: &EntityShootBowEvent<'_>) {
        let Some(player) = host.player(event.player) else {
            return;
        };
        let shot_special_bow = [Hand::Main, Hand::Off].into_iter().any(|hand| {
            ItemPlace::PlayerHand {
                player: player.uuid(),
                hand,
            }
            .stack(host)
            .is_some_and(|stack| {
                ICY_BOW.matches(host, &stack) || EXPLOSIVE_BOW.matches(host, &stack)
            })
        });
        if !shot_special_bow
            || !host
                .entity(event.projectile)
                .and_then(|entity| entity.kind(host))
                .is_some_and(|kind| kind.id(host) == "minecraft:arrow")
        {
            return;
        }
        if let Some(level) = player.entity().level(host) {
            // Native damage will validate the arrow's original weapon, even if the
            // player switched items or was holding a different bow in the other hand.
            self.state.lock().unwrap().shots.insert(
                event.projectile,
                Shot {
                    level,
                    expires: None,
                },
            );
        }
    }

    pub(crate) fn impact(&self, host: &dyn Host, event: &ProjectileImpactEvent<'_>) {
        let mut state = self.state.lock().unwrap();
        if let Some(shot) = state.shots.get_mut(&event.projectile)
            && let Some(tick) = shot.level.game_time(host)
        {
            // Paper forgets an arrow four ticks after its impact event.
            shot.expires = Some(tick.saturating_add(4));
        }
    }

    pub(crate) fn departed(&self, entity: u32) {
        self.state.lock().unwrap().shots.remove(&entity);
    }

    pub(crate) fn incoming(&self, event: &LivingIncomingDamageEvent<'_>) {
        let source = event.source;
        if !matches!(
            source.damage_type,
            "minecraft:player_attack" | "minecraft:arrow"
        ) {
            return;
        }
        let mut state = self.state.lock().unwrap();
        let address = hit(event.entity, source);
        let reaction = match source.damage_type {
            "minecraft:player_attack"
                if source.causing == source.direct
                    && tagged(event.weapon, self.sword.get(), BLADE_OF_VAMPIRES.id()) =>
            {
                u32::try_from(source.causing).ok().map(Reaction::Heal)
            }
            "minecraft:arrow" => {
                let projectile = u32::try_from(source.direct).ok();
                let victim = u32::try_from(event.entity).ok();
                let level = projectile
                    .and_then(|projectile| state.shots.remove(&projectile))
                    .map(|shot| shot.level);
                match (level, victim, projectile) {
                    (Some(level), Some(victim), _)
                        if tagged(event.weapon, self.bow.get(), ICY_BOW.id()) =>
                    {
                        Some(Reaction::Freeze { level, victim })
                    }
                    (Some(level), Some(victim), Some(projectile))
                        if tagged(event.weapon, self.bow.get(), EXPLOSIVE_BOW.id()) =>
                    {
                        Some(Reaction::Explode(Blast {
                            level,
                            victim,
                            projectile,
                            owner: u32::try_from(source.causing).unwrap_or(0),
                            origin: event.victim.position,
                            damage: event.container.original(),
                        }))
                    }
                    _ => None,
                }
            }
            _ => return,
        };
        if let Some(reaction) = reaction {
            state.tentative.insert(address, reaction);
        } else {
            state.tentative.remove(&address);
        }
    }

    pub(crate) fn post(&self, event: &LivingDamagePostEvent<'_>) {
        if !matches!(
            event.source.damage_type,
            "minecraft:player_attack" | "minecraft:arrow"
        ) {
            return;
        }
        let mut state = self.state.lock().unwrap();
        if let Some(reaction) = state.tentative.remove(&hit(event.entity, event.source)) {
            // A player who absorbed the full hit with a shield gets no ice effects.
            if matches!(reaction, Reaction::Freeze { .. })
                && event.kind == "minecraft:player"
                && event.container.blocked() > 0.0
                && event.container.health_damage() <= 0.0
            {
                return;
            }
            state.ready.push(reaction);
        }
    }

    pub(crate) fn contact(&self, host: &mut dyn ServerHost, event: &ArrowContactPostEvent<'_>) {
        if let Some(blast) = self.take_blast(event.contact, event.projectile) {
            self.explode(host, blast);
        }
    }

    pub(crate) fn armor_stand_hit(
        &self,
        host: &mut dyn ServerHost,
        event: &ArrowHitArmorStandPostEvent<'_>,
    ) {
        if let Some(blast) = self.take_stand_blast(event) {
            self.explode(host, blast);
        }
    }

    fn take_stand_blast(&self, event: &ArrowHitArmorStandPostEvent<'_>) -> Option<Blast> {
        let shot = self.state.lock().unwrap().shots.remove(&event.projectile)?;
        tagged(event.weapon, self.bow.get(), EXPLOSIVE_BOW.id()).then_some(Blast {
            level: shot.level,
            projectile: event.projectile,
            owner: event.owner,
            victim: event.stand,
            origin: event.position,
            damage: event.damage,
        })
    }

    fn take_blast(&self, contact: &str, projectile: u32) -> Option<Blast> {
        if contact != "damage" {
            return None;
        }
        let mut state = self.state.lock().unwrap();
        let (index, blast) = state
            .ready
            .iter()
            .enumerate()
            .find_map(|(index, reaction)| match *reaction {
                Reaction::Explode(blast) if blast.projectile == projectile => Some((index, blast)),
                _ => None,
            })?;
        state.ready.remove(index);
        Some(blast)
    }

    fn explode(&self, host: &mut dyn ServerHost, blast: Blast) {
        blast.apply(
            host,
            self.blast_range.load(Ordering::Relaxed),
            self.blast_particle.get().copied(),
        );
    }

    pub(crate) fn tick(&self, host: &mut dyn ServerHost, current: LevelRef, rolls: &Rolls) {
        let mut reactions = {
            let mut state = self.state.lock().unwrap();
            // Incoming hits which another listener vetoed or which the damage cooldown
            // rejected never get a matching damage-post callback.
            state.tentative.clear();
            if let Some(now) = current.game_time(host) {
                state.shots.retain(|_, shot| {
                    shot.level != current || shot.expires.is_none_or(|expiry| expiry > now)
                });
            }
            std::mem::take(&mut state.ready)
        };
        reactions.retain(|&reaction| match reaction {
            Reaction::Heal(id) => heal(
                host,
                current,
                rolls,
                id,
                self.chance.load(Ordering::Relaxed),
            ),
            Reaction::Freeze { level, victim } => {
                if level != current {
                    return true;
                }
                freeze(host, level, victim);
                false
            }
            Reaction::Explode(blast) => {
                if blast.level != current {
                    return true;
                }
                // A shield can keep a valid zero-health-damage event from completing arrow contact.
                self.explode(host, blast);
                false
            }
        });
        self.state.lock().unwrap().ready.extend(reactions);
    }

    pub(crate) fn unload(&self, host: &dyn Host, dimension: &str) {
        let same_dimension =
            |level: LevelRef| level.dimension().id(host).as_deref() == Some(dimension);
        let mut state = self.state.lock().unwrap();
        state.shots.retain(|_, shot| !same_dimension(shot.level));
        state.ready.retain(|reaction| match reaction {
            Reaction::Heal(_) => true,
            Reaction::Freeze { level, .. } | Reaction::Explode(Blast { level, .. }) => {
                !same_dimension(*level)
            }
        });
    }

    pub(crate) fn clear(&self) {
        *self.state.lock().unwrap() = State::default();
    }
}

/// True means the attacker is still online in a different executing level.
fn heal(
    host: &mut dyn ServerHost,
    level: LevelRef,
    rolls: &Rolls,
    attacker: u32,
    chance: u8,
) -> bool {
    let Some(entity) = host.entity(attacker) else {
        return false;
    };
    let Some(uuid) = entity.uuid(host) else {
        return false;
    };
    let Some(player) = host.player(uuid).filter(|player| player.entity() == entity) else {
        return false;
    };
    if entity.level(host) != Some(level) {
        return true;
    }
    if !research::can_use(host, player, &VAMPIRES)
        || !rolls
            .next(host, level, 100)
            .is_some_and(|roll| roll < u32::from(chance))
    {
        return false;
    }
    if let Some(living) = host.living(entity)
        && let (Some(health), Some(maximum)) = (living.health(host), living.max_health(host))
        && health > 0.0
    {
        let _ = player.play_sound(
            host,
            "minecraft:entity.arrow.hit_player",
            SoundSource::Players,
            0.7,
            0.7,
        );
        let _ = living.set_health(host, (health + 4.0).min(maximum));
    }
    false
}

fn freeze(host: &mut dyn ServerHost, level: LevelRef, victim: u32) {
    let Some(entity) = host
        .entity(victim)
        .filter(|entity| entity.level(host) == Some(level))
    else {
        return;
    };
    let Some(living) = host
        .living(entity)
        .filter(|living| living.health(host).is_some_and(|health| health > 0.0))
    else {
        return;
    };
    if entity
        .kind(host)
        .is_some_and(|kind| kind.id(host) == "minecraft:player")
    {
        let _ = entity.set_ticks_frozen(host, 60);
    }
    if let Some(ice) = host
        .block_key("minecraft:ice")
        .and_then(|ice| host.block_state_default(ice))
    {
        for position in [entity.position(host), entity.eye_position(host)]
            .into_iter()
            .flatten()
        {
            let pos = position.map(|coordinate| coordinate.floor() as i32);
            let _ = level.block_break_effect(host, pos, ice);
        }
    }
    for (id, amplifier) in [("minecraft:slowness", 10), ("minecraft:jump_boost", -10)] {
        if let Some(effect) = host.effect_key(id) {
            // Paper 26.2 and Horizon apply the same vanilla [0, 255] clamp.
            let _ = living.add_effect(
                host,
                &ActiveEffect::new(effect, 40, amplifier, false, true, true),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Identifier, testing::FakeHost};

    #[test]
    fn paper_magic_workbench_recipes_use_original_ingredients_and_research() {
        assert_eq!(CRAFTING[0].research.cost, 26);
        assert_eq!(CRAFTING[1].research.cost, 22);
        assert_eq!(
            CRAFTING[0].inputs.map(|item| item.map_or("", Kind::id)),
            [
                "",
                "minecraft:wither_skeleton_skull",
                "",
                "",
                "minecraft:wither_skeleton_skull",
                "",
                "",
                "minecraft:blaze_rod",
                ""
            ],
        );
        assert_eq!(
            CRAFTING[1].inputs.map(|item| item.map_or("", Kind::id)),
            [
                "",
                "minecraft:stick",
                "minecraft:ice",
                "STAFF_ELEMENTAL_WATER",
                "",
                "minecraft:packed_ice",
                "",
                "minecraft:stick",
                "minecraft:ice"
            ],
        );
    }

    #[test]
    fn explosive_bow_uses_papers_recipe_and_only_spends_an_admitted_normal_arrow() {
        use mod_api::{DamageContainer, Victim, WorldEpoch};

        let recipe = CRAFTING
            .iter()
            .find(|recipe| recipe.output.kind == EXPLOSIVE_BOW)
            .unwrap();
        assert_eq!(recipe.research.id, "special_bows");
        assert_eq!(
            recipe.inputs.map(|item| item.map_or("", Kind::id)),
            [
                "",
                "minecraft:stick",
                "minecraft:gunpowder",
                "STAFF_ELEMENTAL_FIRE",
                "",
                "SULFATE",
                "",
                "minecraft:stick",
                "minecraft:gunpowder",
            ]
        );
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse(EXPLOSIVE_BOW.material()).unwrap()])
            .unwrap();
        let explosive = EXPLOSIVE_BOW.stack(&host, 1).unwrap();
        let plain = ItemStack::new(explosive.item(), 1).unwrap();
        let weapons = Weapons::default();
        weapons.bow.set(explosive.item()).unwrap();
        let level = LevelRef::new(
            WorldEpoch::new(1),
            mod_api::__private::dimension_key("minecraft:overworld"),
        );
        let victim = Victim {
            position: [10.0, 64.0, 15.0],
            view: [0.0, 0.0, 1.0],
        };
        let source = |projectile| HurtSource {
            damage_type: "minecraft:arrow",
            causing: 7,
            direct: projectile,
            position: None,
        };
        let incoming = |projectile: i32, weapon| {
            weapons.state.lock().unwrap().shots.insert(
                projectile as u32,
                Shot {
                    level,
                    expires: None,
                },
            );
            weapons.incoming(&LivingIncomingDamageEvent {
                entity: 15,
                kind: "minecraft:pig",
                source: &source(projectile),
                victim: &victim,
                weapon: Some(weapon),
                container: &mut DamageContainer::new(9.0),
                cancelled: false,
            });
        };
        let admitted = |projectile| {
            weapons.post(&LivingDamagePostEvent {
                entity: 15,
                kind: "minecraft:pig",
                source: &source(projectile),
                container: &DamageContainer::new(9.0),
            })
        };
        incoming(40, &explosive);
        // Another listener vetoed the damage; no damage-post followed.
        assert!(weapons.take_blast("damage", 40).is_none());
        assert!(weapons.state.lock().unwrap().ready.is_empty());
        incoming(41, &plain);
        admitted(41);
        assert!(weapons.state.lock().unwrap().ready.is_empty());

        incoming(42, &explosive);
        admitted(42);
        admitted(42);
        assert!(weapons.take_blast("portal", 42).is_none());
        {
            let state = weapons.state.lock().unwrap();
            let [Reaction::Explode(blast)] = state.ready.as_slice() else {
                panic!("one admitted blast")
            };
            assert_eq!((blast.owner, blast.victim), (7, 15));
            assert_eq!(blast.origin, victim.position);
            assert_eq!(blast.damage, 9.0);
        }
        assert!(weapons.take_blast("damage", 42).is_some());
        assert!(weapons.take_blast("damage", 42).is_none());
        assert!(weapons.state.lock().unwrap().ready.is_empty());
    }

    #[test]
    fn the_admitted_armor_stand_event_spends_one_tracked_explosive_arrow() {
        use mod_api::WorldEpoch;

        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse(EXPLOSIVE_BOW.material()).unwrap()])
            .unwrap();
        let bow = EXPLOSIVE_BOW.stack(&host, 1).unwrap();
        let plain = ItemStack::new(bow.item(), 1).unwrap();
        let weapons = Weapons::default();
        weapons.bow.set(bow.item()).unwrap();
        let level = LevelRef::new(
            WorldEpoch::new(1),
            mod_api::__private::dimension_key("minecraft:overworld"),
        );
        let mut event = ArrowHitArmorStandPostEvent {
            dimension: "minecraft:overworld",
            projectile: 42,
            owner: 7,
            stand: 99,
            position: [10.0, 69.0, 15.0],
            damage: 13.0,
            weapon: Some(&plain),
        };
        let track = || {
            weapons.state.lock().unwrap().shots.insert(
                42,
                Shot {
                    level,
                    expires: None,
                },
            )
        };
        track();
        assert!(weapons.take_stand_blast(&event).is_none());
        track();
        event.weapon = Some(&bow);
        let blast = weapons.take_stand_blast(&event).unwrap();
        assert_eq!((blast.victim, blast.owner), (99, 7));
        assert_eq!((blast.origin, blast.damage), ([10.0, 69.0, 15.0], 13.0));
        assert!(weapons.take_stand_blast(&event).is_none());
    }

    #[test]
    fn weapon_matching_rejects_plain_gear_and_keeps_paper_blade_enchantments() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                [BLADE_OF_VAMPIRES.material(), ICY_BOW.material()]
                    .map(|id| Identifier::parse(id).unwrap()),
            )
            .unwrap();
        let sword = BLADE_OF_VAMPIRES.stack(&host, 1).unwrap();
        let bow = ICY_BOW.stack(&host, 1).unwrap();
        let plain = ItemStack::new(bow.item(), 1).unwrap();
        assert!(tagged(
            Some(&sword),
            Some(&sword.item()),
            BLADE_OF_VAMPIRES.id()
        ));
        assert!(tagged(Some(&bow), Some(&bow.item()), ICY_BOW.id()));
        assert!(!tagged(Some(&plain), Some(&plain.item()), ICY_BOW.id()));
        assert!(!tagged(Some(&bow), Some(&sword.item()), ICY_BOW.id()));
        let patch = sword.components().unwrap().as_nbt();
        let enchantments = patch.get(&"minecraft:enchantments".into()).unwrap();
        assert_eq!(
            enchantments.get("minecraft:fire_aspect"),
            Some(&Tag::Int(2))
        );
        assert_eq!(enchantments.get("minecraft:unbreaking"), Some(&Tag::Int(4)));
        assert_eq!(enchantments.get("minecraft:sharpness"), Some(&Tag::Int(2)));
    }

    #[test]
    fn icy_reaction_requires_admitted_damage_and_uses_one_arrow_at_most_once() {
        use mod_api::{DamageContainer, DamageValues, Victim, WorldEpoch};

        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse(ICY_BOW.material()).unwrap()])
            .unwrap();
        let bow = ICY_BOW.stack(&host, 1).unwrap();
        let weapons = Weapons::default();
        assert_eq!(weapons.bow.set(bow.item()), Ok(()));
        let level = LevelRef::new(
            WorldEpoch::new(1),
            mod_api::__private::dimension_key("minecraft:overworld"),
        );
        let victim = Victim {
            position: [10.0, 64.0, 10.0],
            view: [0.0, 0.0, 1.0],
        };
        let start_hit = |direct| HurtSource {
            damage_type: "minecraft:arrow",
            causing: 1,
            direct,
            position: Some([10.0, 64.0, 7.0]),
        };
        let prepare = |direct| {
            weapons.state.lock().unwrap().shots.insert(
                direct,
                Shot {
                    level,
                    expires: None,
                },
            );
            let source = start_hit(direct as i32);
            weapons.incoming(&LivingIncomingDamageEvent {
                entity: 15,
                kind: "minecraft:player",
                source: &source,
                victim: &victim,
                weapon: Some(&bow),
                container: &mut DamageContainer::new(8.0),
                cancelled: false,
            });
        };
        prepare(41); // A later listener vetoes this arrow. No post event follows.
        assert!(weapons.state.lock().unwrap().ready.is_empty());

        let post = |direct, container: &DamageContainer| {
            weapons.post(&LivingDamagePostEvent {
                entity: 15,
                kind: "minecraft:player",
                source: &start_hit(direct),
                container,
            });
        };
        prepare(42);
        let mut shielded = DamageValues::from(&DamageContainer::new(8.0));
        shielded.blocked = 8.0;
        let shielded = DamageContainer::from(shielded);
        post(42, &shielded);
        assert!(weapons.state.lock().unwrap().ready.is_empty());

        prepare(43);
        let admitted = DamageContainer::new(8.0);
        post(43, &admitted);
        post(43, &admitted);
        let state = weapons.state.lock().unwrap();
        assert!(matches!(
            state.ready.as_slice(),
            [Reaction::Freeze { victim: 15, .. }]
        ));
        assert!(state.shots.is_empty());
    }
}
