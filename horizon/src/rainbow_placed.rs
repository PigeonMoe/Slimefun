// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{LazyLock, Mutex},
};

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentType, BlockChangedEvent, BlockDropsEvent,
    BlockMetadata, BlockState, BlockUpdateFlags, Compound, Holder, HostBlockStates, ItemDrop,
    ItemPart, ItemStack, LevelHost, LevelRef, OnBlockPlacedByEvent, ServerHost, Tag,
};

use crate::{items::item_id, network_runtime::dimension_name};

use super::{BLOCK_COUNT, Rainbow, block, is_carrier};

type Pos = [i32; 3];
type ChunkPos = [i32; 2];
type Address = (String, Pos);

const INTERVAL: u64 = 10;
const PANE_PROPERTIES: [&str; 5] = ["north", "east", "south", "west", "waterlogged"];

const MARKERS: AttachmentType<Compound> = AttachmentType {
    namespace: "slimefun",
    name: "rainbow_blocks",
    default: Compound::new,
    codec: Some(AttachmentCodec {
        encode: |value| Ok(value.clone()),
        decode: |value| Ok(value.clone()),
    }),
    copy_on_death: false,
    sync: false,
};

#[derive(Clone, Copy, Default)]
struct Phase {
    round: u64,
    color: usize,
}

impl Phase {
    fn color(&mut self, round: u64, colors: &'static [&'static str]) -> &'static str {
        if self.round != round {
            self.round = round;
            self.color = (self.color + 1) % colors.len();
        }
        colors[self.color]
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Retired {
    Removed,
    Dropped,
    Superseded,
}

struct Recolor {
    at: Address,
    old: BlockState,
    new: BlockState,
}

#[derive(Default)]
struct Runtime {
    ticks: u64,
    phases: [Phase; BLOCK_COUNT],
    chunks: BTreeMap<String, BTreeSet<ChunkPos>>,
    seeded: BTreeSet<String>,
    retired: BTreeMap<Address, Retired>,
    recolor: Option<Recolor>,
}

static RUNTIME: LazyLock<Mutex<Runtime>> = LazyLock::new(|| Mutex::new(Runtime::default()));

fn chunk([x, _, z]: Pos) -> ChunkPos {
    [x.div_euclid(16), z.div_euclid(16)]
}

fn key([x, y, z]: Pos) -> String {
    format!("{x},{y},{z}")
}

fn pos(key: &str) -> Option<Pos> {
    let mut values = key.split(',');
    let out = [
        values.next()?.parse().ok()?,
        values.next()?.parse().ok()?,
        values.next()?.parse().ok()?,
    ];
    values.next().is_none().then_some(out)
}

fn recorded(map: &Compound, pos: Pos) -> Option<Rainbow> {
    let Tag::String(id) = map.get(&key(pos).as_str().into())? else {
        return None;
    };
    block(&id.to_string().ok()?).map(|(_, rainbow)| rainbow)
}

fn read(host: &mut dyn ServerHost, column: ChunkPos) -> Result<Compound, AttachmentError> {
    MARKERS
        .get_existing(host, Holder::Chunk(column))
        .map(Option::unwrap_or_default)
}

fn saved(level: &dyn LevelHost, pos: Pos) -> Option<Rainbow> {
    let map = level.chunk_saved_attachment(chunk(pos), MARKERS.namespace, MARKERS.name)?;
    recorded(&map, pos)
}

fn place(
    host: &mut dyn ServerHost,
    dimension: &str,
    pos: Pos,
    rainbow: Rainbow,
) -> Result<(), AttachmentError> {
    let column = chunk(pos);
    let mut map = read(host, column)?;
    map.insert(
        key(pos).as_str().into(),
        Tag::String(rainbow.kind.id().into()),
    );
    MARKERS.set(host, Holder::Chunk(column), map)?;
    let mut runtime = RUNTIME.lock().unwrap();
    runtime
        .chunks
        .entry(dimension.into())
        .or_default()
        .insert(column);
    runtime.retired.remove(&(dimension.into(), pos));
    Ok(())
}

pub(crate) struct Placement;

#[mod_api::horizon_part]
impl ItemPart for Placement {
    fn use_on(
        &self,
        host: mod_api::HostSide<'_>,
        event: &mut mod_api::UseOnEvent<'_>,
    ) -> Option<mod_api::InteractionResult> {
        (matches!(host, mod_api::HostSide::Server(_))
            && crate::ticker_policy::Tickers::current().blocks_stack(event.stack))
        .then_some(mod_api::InteractionResult::Fail)
    }

    fn on_block_placed_by(&self, host: &mut dyn ServerHost, event: &mut OnBlockPlacedByEvent<'_>) {
        let Some((_, rainbow)) = item_id(event.stack).and_then(|id| block(&id)) else {
            return;
        };
        if event.placer.is_none()
            || !crate::ticker_policy::Tickers::current().allows(rainbow.kind.id())
            || !rainbow.kind.matches(host, event.stack)
        {
            return;
        }
        let Some(level) = host.level(host.dimension()) else {
            return;
        };
        let Some(dimension) = level.dimension().id(host) else {
            return;
        };
        if level
            .block(host, event.pos)
            .is_some_and(|actual| actual.id(host) == rainbow.kind.material())
        {
            let _ = place(host, &dimension, event.pos, rainbow);
        }
    }
}

pub(crate) fn restore_paper(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Pos,
    id: &str,
) -> Result<String, String> {
    let (_, rainbow) = block(id).ok_or("The archive does not identify a Paper rainbow block.")?;
    let dimension = level
        .dimension()
        .id(host)
        .ok_or("Unknown Horizon dimension.")?;
    if !level.in_place(host)
        || !level
            .block(host, pos)
            .is_some_and(|material| rainbow.accepts(&material.id(host)))
    {
        return Err("The loaded block does not match the recorded Paper rainbow palette.".into());
    }
    if let Some(current) = active_marker(host, &dimension, pos) {
        return if current == rainbow {
            Ok(format!("{id} is already restored."))
        } else {
            Err("Another rainbow block already owns this position.".into())
        };
    }
    if crate::network_runtime::has_block_marker(host, level, pos) {
        return Err("Another Slimefun device already owns this position.".into());
    }
    place(host, &dimension, pos, rainbow)
        .map_err(|_| "Could not save the Paper rainbow block marker.")?;
    Ok(format!("Restored the Paper {id}."))
}

fn active_marker(host: &mut dyn ServerHost, dimension: &str, pos: Pos) -> Option<Rainbow> {
    if RUNTIME
        .lock()
        .unwrap()
        .retired
        .contains_key(&(dimension.into(), pos))
    {
        return None;
    }
    recorded(&read(host, chunk(pos)).ok()?, pos)
}

pub(crate) fn marked(host: &mut dyn ServerHost, level: LevelRef, pos: Pos) -> bool {
    placed_id(host, level, pos).is_some()
}

pub(crate) fn placed_id(host: &mut dyn ServerHost, level: LevelRef, pos: Pos) -> Option<&'static str> {
    let dimension = level.dimension().id(host)?;
    let id = level.block(host, pos).map(|state| state.id(host)).filter(|id| is_carrier(id))?;
    let mark = active_marker(host, &dimension, pos)?;
    mark.accepts(&id).then(|| mark.kind.id())
}

pub(crate) fn protected(
    level: &dyn LevelHost,
    dimension: &mod_api::LevelDimension,
    pos: Pos,
) -> bool {
    let Some(carrier) = level
        .block_state(pos)
        .and_then(|state| level.state_block(state))
        .filter(|id| is_carrier(id))
    else {
        return false;
    };
    let address = (dimension_name(dimension).into(), pos);
    saved(level, pos).is_some_and(|mark| mark.accepts(carrier))
        && !RUNTIME.lock().unwrap().retired.contains_key(&address)
}

pub(crate) fn changed(level: &dyn LevelHost, event: &BlockChangedEvent) {
    let old = level.state_block(event.old);
    let new = level.state_block(event.new);
    if old == new || !old.is_some_and(is_carrier) && !new.is_some_and(is_carrier) {
        return;
    }
    let pos = [event.pos.x, event.pos.y, event.pos.z];
    let address = (dimension_name(&event.dimension).to_owned(), pos);
    let mut runtime = RUNTIME.lock().unwrap();
    if runtime.recolor.as_ref().is_some_and(|current| {
        current.at == address && current.old == event.old && current.new == event.new
    }) {
        return;
    }
    if let Some(change) = runtime.retired.get_mut(&address) {
        if new.is_some_and(is_carrier) {
            *change = Retired::Superseded;
        }
    } else if let Some(previous) = old
        && saved(level, pos).is_some_and(|record| record.accepts(previous))
    {
        runtime.retired.insert(address, Retired::Removed);
    }
}

pub(crate) fn drop_item(level: &dyn LevelHost, event: &mut BlockDropsEvent<'_>) {
    let Some(carrier) = level.state_block(event.state).filter(|id| is_carrier(id)) else {
        return;
    };
    let pos = [event.pos.x, event.pos.y, event.pos.z];
    let Some(record) = saved(level, pos).filter(|record| record.accepts(carrier)) else {
        return;
    };
    let address = (dimension_name(&event.dimension).to_owned(), pos);
    let mut runtime = RUNTIME.lock().unwrap();
    if runtime
        .retired
        .get(&address)
        .is_some_and(|removed| *removed != Retired::Removed)
    {
        return;
    }
    runtime.retired.insert(address, Retired::Dropped);
    drop(runtime);

    if event.breaker.is_none() {
        event.drops.clear();
        return;
    }
    let Some(mut stack) = level
        .item_key_by_name(record.kind.material())
        .and_then(|key| ItemStack::new(key, 1))
    else {
        return;
    };
    record.kind.decorate(&mut stack);
    let prepared = level.item_key_by_name(carrier).and_then(|vanilla| {
        let mut found = None;
        event.drops.retain(|drop| {
            if drop.stack.item() != vanilla {
                return true;
            }
            found = found.or(Some((drop.position, drop.velocity, drop.pickup_delay)));
            false
        });
        found
    });
    let (at, velocity, delay) = prepared.unwrap_or_else(|| {
        (
            pos.map(|coordinate| f64::from(coordinate) + 0.5),
            [0.0, 0.2, 0.0],
            10,
        )
    });
    event.drops.push(ItemDrop::new(stack, at, velocity, delay));
}

pub(crate) fn begin_tick() {
    let mut state = RUNTIME.lock().unwrap();
    state.ticks = state.ticks.saturating_add(1);
}

pub(crate) fn loaded(host: &mut dyn ServerHost, dimension: &str, column: ChunkPos) {
    if host
        .level(dimension)
        .is_some_and(|level| level.in_place(host))
        && read(host, column).is_ok_and(|map| !map.is_empty())
    {
        RUNTIME
            .lock()
            .unwrap()
            .chunks
            .entry(dimension.into())
            .or_default()
            .insert(column);
    }
}

pub(crate) fn unloaded(dimension: &str, column: ChunkPos) {
    if let Some(chunks) = RUNTIME.lock().unwrap().chunks.get_mut(dimension) {
        chunks.remove(&column);
    }
}

pub(crate) fn unload_level(dimension: &str) {
    let mut runtime = RUNTIME.lock().unwrap();
    runtime.chunks.remove(dimension);
    runtime.seeded.remove(dimension);
}

pub(crate) fn clear() {
    *RUNTIME.lock().unwrap() = Runtime::default();
}

fn cleanup(host: &mut dyn ServerHost, level: LevelRef, dimension: &str) {
    let addresses: Vec<_> = RUNTIME
        .lock()
        .unwrap()
        .retired
        .keys()
        .filter(|(known, _)| known == dimension)
        .cloned()
        .collect();
    for address in addresses {
        let column = chunk(address.1);
        if host.chunk(level, column).is_none() {
            continue;
        }
        let Ok(mut map) = read(host, column) else {
            continue;
        };
        if map.remove(&key(address.1).as_str().into()).is_some()
            && MARKERS.set(host, Holder::Chunk(column), map).is_err()
        {
            continue;
        }
        RUNTIME.lock().unwrap().retired.remove(&address);
    }
}

pub(crate) fn tick(host: &mut dyn ServerHost, level: LevelRef) {
    if !level.in_place(host) {
        return;
    }
    let Some(dimension) = level.dimension().id(host) else {
        return;
    };
    cleanup(host, level, &dimension);
    let seed = RUNTIME.lock().unwrap().seeded.insert(dimension.clone());
    if seed {
        for column in level.loaded_chunks(host).unwrap_or_default() {
            loaded(host, &dimension, column.pos());
        }
    }
    let (round, columns) = {
        let state = RUNTIME.lock().unwrap();
        if state.ticks == 0 || state.ticks % INTERVAL != 0 {
            return;
        }
        (
            state.ticks / INTERVAL,
            state.chunks.get(&dimension).cloned().unwrap_or_default(),
        )
    };
    for column in columns {
        let Ok(map) = read(host, column) else {
            continue;
        };
        if map.is_empty() {
            unloaded(&dimension, column);
            continue;
        }
        for (key, id) in map {
            let Some(at) = key
                .to_string()
                .ok()
                .as_deref()
                .and_then(pos)
                .filter(|at| chunk(*at) == column)
            else {
                continue;
            };
            let Tag::String(id) = id else {
                continue;
            };
            let Some((index, rainbow)) = id.to_string().ok().as_deref().and_then(block) else {
                continue;
            };
            if !crate::availability::globally_enabled(rainbow.kind.id()) {
                continue;
            }
            let address = (dimension.clone(), at);
            if RUNTIME.lock().unwrap().retired.contains_key(&address) {
                continue;
            }
            recolor(host, level, address, rainbow, index, round);
        }
    }
}

fn recolor(
    host: &mut dyn ServerHost,
    level: LevelRef,
    at: Address,
    rainbow: Rainbow,
    index: usize,
    round: u64,
) -> Option<()> {
    let pos = at.1;
    let old = level.block_state(host, pos)?;
    if !rainbow.accepts(&old.block().id(host)) {
        RUNTIME
            .lock()
            .unwrap()
            .retired
            .insert(at, Retired::Superseded);
        return None;
    }
    let color = RUNTIME.lock().unwrap().phases[index].color(round, rainbow.palette.colors());
    let mut new = host.block_state_id(&rainbow.material(color))?;
    if rainbow.suffix == "stained_glass_pane" {
        new = pane_state(&host.block_states(), old, new)?;
    }
    if old == new {
        return Some(());
    }
    struct RecolorGuard;
    impl Drop for RecolorGuard {
        fn drop(&mut self) {
            RUNTIME.lock().unwrap().recolor = None;
        }
    }
    RUNTIME.lock().unwrap().recolor = Some(Recolor { at, old, new });
    let _guard = RecolorGuard;
    let flags = BlockUpdateFlags::CLIENTS | BlockUpdateFlags::KNOWN_SHAPE;
    level.set_block(host, pos, new, flags).ok()?.then_some(())
}

fn pane_state(
    metadata: &dyn BlockMetadata,
    old: BlockState,
    new: BlockState,
) -> Option<BlockState> {
    PANE_PROPERTIES.into_iter().try_fold(new, |next, key| {
        let value = old.property(metadata, key).ok()??;
        next.with(metadata, key, value).ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_blocks_of_a_paper_variant_use_the_same_color_per_round() {
        let (_, all) = block("RAINBOW_WOOL").unwrap();
        let (_, christmas) = block("RAINBOW_WOOL_XMAS").unwrap();
        let (_, valentine) = block("RAINBOW_WOOL_VALENTINE").unwrap();
        let (_, halloween) = block("RAINBOW_WOOL_HALLOWEEN").unwrap();
        for (record, expected) in [
            (all, ["orange", "magenta"]),
            (christmas, ["green", "red"]),
            (valentine, ["pink", "magenta"]),
            (halloween, ["black", "orange"]),
        ] {
            let mut phase = Phase::default();
            assert_eq!(phase.color(1, record.palette.colors()), expected[0]);
            assert_eq!(phase.color(1, record.palette.colors()), expected[0]);
            assert_eq!(phase.color(18, record.palette.colors()), expected[1]);
        }
    }

    #[test]
    fn recolored_glass_panes_retain_every_edge_and_the_waterlogged_flag() {
        use mod_api::{BlockKey, BlockStateError, BlockStateRecord, StateConstraint};

        struct PaneStates;
        impl BlockMetadata for PaneStates {
            fn name(&self, _: BlockKey) -> Result<&str, BlockStateError> {
                unreachable!()
            }

            fn property(
                &self,
                state: BlockStateRecord,
                name: &str,
            ) -> Result<Option<&str>, BlockStateError> {
                Ok(PANE_PROPERTIES
                    .iter()
                    .position(|&key| key == name)
                    .map(|index| {
                        if state.state & (1 << index) != 0 {
                            "true"
                        } else {
                            "false"
                        }
                    }))
            }

            fn with(
                &self,
                state: BlockStateRecord,
                name: &str,
                value: &str,
            ) -> Result<BlockStateRecord, BlockStateError> {
                let index = PANE_PROPERTIES.iter().position(|&key| key == name).unwrap();
                let mask = 1 << index;
                let bits = if value == "true" {
                    state.state | mask
                } else {
                    state.state & !mask
                };
                Ok(BlockStateRecord::new(state.block, bits))
            }

            fn of(
                &self,
                _: BlockKey,
                _: &[StateConstraint],
            ) -> Result<BlockStateRecord, BlockStateError> {
                unreachable!()
            }
        }

        let before = mod_api::__private::parts_installation::block_key(0);
        let after = mod_api::__private::parts_installation::block_key(1);
        for edges_and_water in 0..32 {
            let old = BlockStateRecord::new(before, edges_and_water).into();
            let default = BlockStateRecord::new(after, 0).into();
            let next = pane_state(&PaneStates, old, default).unwrap();
            assert_eq!(next.block(), after);
            assert_eq!(next.ordinal(), edges_and_water);
        }
    }

    #[test]
    fn chunk_marker_keeps_exact_id_and_valid_variant_carriers() {
        let position = [-17, 127, 31];
        let map = Compound::from([(
            key(position).as_str().into(),
            Tag::String("RAINBOW_GLASS_PANE_XMAS".into()),
        )]);
        assert_eq!(chunk(position), [-2, 1]);
        assert_eq!(pos(&key(position)), Some(position));
        let record = recorded(&map, position).unwrap();
        assert!(record.accepts("minecraft:white_stained_glass_pane"));
        assert!(record.accepts("minecraft:green_stained_glass_pane"));
        assert!(!record.accepts("minecraft:black_stained_glass_pane"));
        assert!(!record.accepts("minecraft:green_stained_glass"));
        assert!(recorded(&map, [-17, 126, 31]).is_none());
    }
}
