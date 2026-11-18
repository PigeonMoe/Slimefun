// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::HashMap,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

use mod_api::{
    BlockChangedEvent, BlockState, BlockUpdateFlags, Host, HostBlockStates, HostTransaction,
    ItemPlace, ItemResource, ItemStack, LevelHost, LevelRef, ParticleKey, ParticleOptions,
    PlayerRef, ServerHost, SoundSource, Text, Transaction, UseOnEvent,
};

use crate::{
    advanced_processing::{BASIC_CRUCIBLE, CRUCIBLE, LAVA_INPUTS, can_melt_leaves},
    network_runtime, research,
};

type Position = [i32; 3];
const STEP_DELAY: i64 = 50;
const MAX_STAGE: u8 = 8;
const EXTINGUISH: &str = "minecraft:block.lava.extinguish";
const FINISH: &str = "minecraft:entity.arrow.hit_player";
const OBSTRUCTED: &str = "minecraft:block.metal.break";

static SMOKE: OnceLock<ParticleKey> = OnceLock::new();

pub(crate) fn register(registrar: &mut dyn mod_api::Registrar) {
    let _ = SMOKE.set(registrar.particle_key("minecraft:smoke"));
}

pub(crate) const CARRIERS: &[&str] = &[
    "minecraft:cauldron",
    "minecraft:water_cauldron",
    "minecraft:lava_cauldron",
    "minecraft:powder_snow_cauldron",
];

pub(crate) fn carrier(id: &str) -> bool {
    CARRIERS.contains(&id)
}

pub(crate) fn refuses_placement(host: &mut dyn ServerHost, event: &UseOnEvent<'_>) -> bool {
    if !CRUCIBLE.matches(host, event.stack) {
        return false;
    }
    let Some(player) = host.player(event.player) else {
        return true;
    };
    if research::can_use(host, player, &BASIC_CRUCIBLE) {
        return false;
    }
    tell(
        host,
        player,
        "Unlock the Crucible with /sf research crucible.",
    );
    true
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Liquid {
    Lava,
    Water,
}

impl Liquid {
    fn name(self) -> &'static str {
        match self {
            Self::Lava => "minecraft:lava",
            Self::Water => "minecraft:water",
        }
    }

    fn sound(self) -> &'static str {
        match self {
            Self::Lava => "minecraft:block.lava.pop",
            Self::Water => "minecraft:entity.player.splash",
        }
    }
}

#[derive(Clone, Copy)]
enum Cell {
    Air,
    Fluid(Liquid, u8),
    Waterloggable,
    Obstructed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Step {
    Fill { liquid: Liquid, stage: u8 },
    Soak,
    React(&'static str),
}

impl Step {
    fn replacement(self, host: &dyn Host, expected: BlockState) -> Option<BlockState> {
        match self {
            Self::Fill { liquid, stage } => {
                let depth = MAX_STAGE.checked_sub(stage)?;
                host.block_state_id(&format!("{}[level={depth}]", liquid.name()))
            }
            Self::Soak => expected
                .with(&host.block_states(), "waterlogged", "true")
                .ok(),
            Self::React(id) => host.block_state_id(id),
        }
    }

    fn flags(self) -> BlockUpdateFlags {
        if matches!(self, Self::React(_)) {
            BlockUpdateFlags::CLIENTS | BlockUpdateFlags::NEIGHBORS
        } else {
            // CraftBlock#setBlockData(data, false) leaves neighbour physics out of the
            // gradual fill and waterlogging. Fluid on-place ticks could otherwise drain it.
            BlockUpdateFlags::CLIENTS
                | BlockUpdateFlags::KNOWN_SHAPE
                | BlockUpdateFlags::SKIP_ON_PLACE
        }
    }
}

fn start(liquid: Liquid, cell: Cell) -> Option<(Step, i64)> {
    Some(match cell {
        Cell::Air => (Step::Fill { liquid, stage: 1 }, STEP_DELAY),
        Cell::Fluid(current, depth) if current == liquid => {
            let normalized = depth % MAX_STAGE;
            if normalized == 0 {
                (Step::Fill { liquid, stage: 1 }, 1)
            } else {
                (
                    Step::Fill {
                        liquid,
                        stage: 7 - normalized,
                    },
                    STEP_DELAY,
                )
            }
        }
        Cell::Fluid(_, 0 | 8) => (Step::React("minecraft:obsidian"), 0),
        Cell::Fluid(..) => (Step::React("minecraft:stone"), 0),
        Cell::Waterloggable if liquid == Liquid::Water => (Step::Soak, STEP_DELAY),
        Cell::Waterloggable | Cell::Obstructed => return None,
    })
}

fn read_cell(host: &dyn Host, level: LevelRef, pos: Position, state: BlockState) -> Option<Cell> {
    let block = state.block().id(host);
    Some(match block.as_str() {
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air" => Cell::Air,
        "minecraft:water" | "minecraft:lava" => {
            let liquid = if block == "minecraft:water" {
                Liquid::Water
            } else {
                Liquid::Lava
            };
            let depth = level
                .block_property(host, pos, "level")?
                .parse::<u8>()
                .ok()?;
            if depth > 15 {
                return None;
            }
            Cell::Fluid(liquid, depth)
        }
        _ if state
            .property(&host.block_states(), "waterlogged")
            .ok()
            .flatten()
            .is_some() =>
        {
            Cell::Waterloggable
        }
        _ => Cell::Obstructed,
    })
}

fn catalyst(host: &dyn Host, input: &ItemStack) -> Option<(Liquid, u32)> {
    if can_melt_leaves(host, input) {
        return Some((Liquid::Water, 16));
    }
    LAVA_INPUTS.iter().find_map(|recipe| {
        (input.count() >= recipe.input.count && recipe.input.kind.matches(host, input))
            .then_some((Liquid::Lava, recipe.input.count))
    })
}

struct Payment {
    slot: ItemPlace,
    resource: ItemResource,
    count: u32,
}

impl Payment {
    fn take(&self, host: &mut dyn ServerHost, tx: &Transaction) -> bool {
        self.slot.extract(host, tx, &self.resource, self.count) == self.count
    }
}

struct Operation {
    anchor: Position,
    dimension: String,
    player: PlayerRef,
    expected: BlockState,
    step: Step,
    due: i64,
    payment: Option<Payment>,
}

#[derive(Default)]
pub(crate) struct Crucibles {
    pending: Mutex<HashMap<(LevelRef, Position), Operation>>,
    allow_nether_water: AtomicBool,
}

impl Crucibles {
    pub(crate) fn configure(&self, value: bool) {
        self.allow_nether_water.store(value, Ordering::Relaxed);
    }

    pub(crate) fn clear(&self) {
        self.pending.lock().unwrap().clear();
    }

    pub(crate) fn anchor_changed(&self, level: &dyn LevelHost, event: &BlockChangedEvent) {
        if !level.state_block(event.old).is_some_and(carrier)
            || level.state_block(event.new).is_some_and(carrier)
        {
            return;
        }
        let dimension = network_runtime::dimension_name(&event.dimension);
        let anchor = [event.pos.x, event.pos.y, event.pos.z];
        self.pending
            .lock()
            .unwrap()
            .retain(|_, job| job.anchor != anchor || job.dimension != dimension);
    }

    pub(crate) fn use_at(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        level: LevelRef,
        anchor: Position,
        use_item: bool,
    ) -> bool {
        if !network_runtime::manual_crucible_at(host, level, anchor) {
            return false;
        }
        if !use_item {
            return true;
        }
        if !research::can_use(host, player, &BASIC_CRUCIBLE) {
            tell(
                host,
                player,
                "Unlock the Crucible with /sf research crucible.",
            );
            return true;
        }

        let [x, y, z] = anchor;
        let Some(pos) = y.checked_add(1).map(|above| [x, above, z]) else {
            return true;
        };
        if level.in_world_bounds(host, pos) != Some(true) {
            tell(host, player, "The Crucible has no space above it.");
            return true;
        }
        if self.pending.lock().unwrap().contains_key(&(level, pos)) {
            tell(host, player, "The Crucible is still filling.");
            return true;
        }
        let Some(slot) = player.selected_slot(host) else {
            return true;
        };
        let place = ItemPlace::Strict(Box::new(ItemPlace::PlayerSlot {
            player: player.uuid(),
            slot: slot as i32,
        }));
        let Some(input) = place.stack(host) else {
            tell(
                host,
                player,
                "Hold a full Crucible recipe in the main hand.",
            );
            return true;
        };
        let Some((liquid, count)) = catalyst(host, &input) else {
            tell(
                host,
                player,
                "Hold a full Crucible recipe in the main hand.",
            );
            return true;
        };
        let payment = Payment {
            slot: place,
            resource: ItemResource::of(&input),
            count,
        };

        let Some(dimension) = level.dimension().id(host) else {
            return true;
        };
        if liquid == Liquid::Water
            && dimension == "minecraft:the_nether"
            && !self.allow_nether_water.load(Ordering::Relaxed)
        {
            // Paper consumes the leaves even when the Nether only produces steam.
            if host
                .transaction(|host, tx| payment.take(host, tx).then_some(()).ok_or(()))
                .is_ok()
            {
                if let Some(&key) = SMOKE.get() {
                    level.send_particles(
                        host,
                        &ParticleOptions::simple(key),
                        None,
                        center(pos),
                        [0.0; 3],
                        0.0,
                        4,
                    );
                }
                sound(host, level, pos, EXTINGUISH);
            }
            return true;
        }

        let Some(expected) = level.block_state(host, pos) else {
            return true;
        };
        let Some((step, delay)) =
            read_cell(host, level, pos, expected).and_then(|cell| start(liquid, cell))
        else {
            sound(host, level, pos, OBSTRUCTED);
            return true;
        };
        let Some(now) = level.game_time(host) else {
            return true;
        };
        let job = Operation {
            anchor,
            dimension,
            player,
            expected,
            step,
            due: now.saturating_add(delay),
            payment: Some(payment),
        };
        if delay == 0 {
            self.advance(host, level, pos, now, job);
        } else {
            self.pending.lock().unwrap().insert((level, pos), job);
        }
        true
    }

    pub(crate) fn tick(&self, host: &mut dyn ServerHost, level: LevelRef) {
        let Some(now) = level.game_time(host) else {
            return;
        };
        let ready: Vec<_> = self
            .pending
            .lock()
            .unwrap()
            .extract_if(|(owner, _), job| *owner == level && job.due <= now)
            .collect();
        for ((_, pos), job) in ready {
            self.advance(host, level, pos, now, job);
        }
    }

    fn advance(
        &self,
        host: &mut dyn ServerHost,
        level: LevelRef,
        pos: Position,
        now: i64,
        mut job: Operation,
    ) {
        if job.player.entity().level(host) != Some(level)
            || !network_runtime::manual_crucible_at(host, level, job.anchor)
        {
            return;
        }
        let Some(next) = job.step.replacement(host, job.expected) else {
            return;
        };
        let result = host.transaction(|host, tx| {
            if job
                .payment
                .as_ref()
                .is_some_and(|payment| !payment.take(host, tx))
            {
                return Err(());
            }
            level
                .replace_block_for_player(
                    host,
                    job.player,
                    pos,
                    job.expected,
                    next,
                    job.step.flags(),
                )
                .ok()
                .filter(|changed| *changed)
                .map(|_| ())
                .ok_or(())
        });
        if result.is_err() {
            if job.payment.is_some() {
                tell(
                    host,
                    job.player,
                    "The Crucible could not fill its target block.",
                );
            }
            return;
        }

        match job.step {
            Step::React(_) => sound(host, level, pos, EXTINGUISH),
            Step::Soak => sound(host, level, pos, Liquid::Water.sound()),
            Step::Fill { liquid, stage } => {
                sound(host, level, pos, liquid.sound());
                if stage == MAX_STAGE {
                    sound(host, level, pos, FINISH);
                } else {
                    job.expected = next;
                    job.step = Step::Fill {
                        liquid,
                        stage: stage + 1,
                    };
                    job.due = now.saturating_add(STEP_DELAY);
                    job.payment = None;
                    self.pending.lock().unwrap().insert((level, pos), job);
                }
            }
        }
    }
}

fn center(pos: Position) -> [f64; 3] {
    pos.map(|axis| f64::from(axis) + 0.5)
}

fn sound(host: &mut dyn ServerHost, level: LevelRef, pos: Position, id: &str) {
    let _ = level.play_sound(host, id, center(pos), SoundSource::Blocks, 1.0, 1.0);
}

fn tell(host: &mut dyn ServerHost, player: PlayerRef, message: &str) {
    let _ = player.tell(host, Text::literal(message));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::Kind;
    use mod_api::{Identifier, testing::FakeHost};

    #[test]
    fn paper_crucible_fill_waits_fifty_ticks_and_preserves_falling_depth() {
        assert_eq!(
            start(Liquid::Lava, Cell::Air),
            Some((
                Step::Fill {
                    liquid: Liquid::Lava,
                    stage: 1
                },
                50
            ))
        );
        for depth in [0, 8] {
            assert_eq!(
                start(Liquid::Water, Cell::Fluid(Liquid::Water, depth)),
                Some((
                    Step::Fill {
                        liquid: Liquid::Water,
                        stage: 1
                    },
                    1
                ))
            );
        }
        for depth in [7, 15] {
            assert_eq!(
                start(Liquid::Water, Cell::Fluid(Liquid::Water, depth)),
                Some((
                    Step::Fill {
                        liquid: Liquid::Water,
                        stage: 0
                    },
                    50
                ))
            );
        }
        assert_eq!(
            start(Liquid::Water, Cell::Fluid(Liquid::Water, 9)),
            Some((
                Step::Fill {
                    liquid: Liquid::Water,
                    stage: 6
                },
                50
            ))
        );
    }

    #[test]
    fn paper_crucible_mixes_opposite_source_and_flowing_liquids_distinctly() {
        for (depth, expected) in [
            (0, "minecraft:obsidian"),
            (8, "minecraft:obsidian"),
            (1, "minecraft:stone"),
            (10, "minecraft:stone"),
        ] {
            assert_eq!(
                start(Liquid::Water, Cell::Fluid(Liquid::Lava, depth)),
                Some((Step::React(expected), 0))
            );
            assert_eq!(
                start(Liquid::Lava, Cell::Fluid(Liquid::Water, depth)),
                Some((Step::React(expected), 0))
            );
        }
        assert_eq!(
            start(Liquid::Water, Cell::Waterloggable),
            Some((Step::Soak, 50))
        );
        assert!(start(Liquid::Lava, Cell::Waterloggable).is_none());
        assert!(start(Liquid::Water, Cell::Obstructed).is_none());
    }

    #[test]
    fn a_refused_first_write_leaves_the_original_held_slot_intact() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse("minecraft:cobblestone").unwrap()])
            .unwrap();
        let kind = Kind::Vanilla("minecraft:cobblestone");
        let material = kind.stack(&host, 16).unwrap();
        let short = kind.stack(&host, 15).unwrap();
        let level = host.level.level();
        host.players
            .join(1, 8, level)
            .slots
            .insert(0, short.clone());
        let payment = Payment {
            slot: ItemPlace::Strict(Box::new(ItemPlace::PlayerSlot { player: 1, slot: 0 })),
            resource: ItemResource::of(&material),
            count: 16,
        };
        assert_eq!(
            host.transaction(|host, tx| payment.take(host, tx).then_some(()).ok_or(())),
            Err(())
        );
        assert_eq!(payment.slot.stack(&mut host), Some(short));
        host.players
            .get_mut(1)
            .unwrap()
            .slots
            .insert(0, material.clone());
        assert_eq!(
            host.transaction(|host, tx| {
                assert!(payment.take(host, tx));
                Err::<(), _>("destination changed")
            }),
            Err("destination changed")
        );
        assert_eq!(payment.slot.stack(&mut host), Some(material));
        assert_eq!(
            host.transaction(|host, tx| payment.take(host, tx).then_some(()).ok_or(())),
            Ok(())
        );
        assert!(payment.slot.stack(&mut host).is_none());
    }
}
