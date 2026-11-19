// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, sync::Mutex};

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentType, BlockChangedEvent, BlockDropsEvent, Compound,
    Holder, LevelDimension, LevelHost, LevelRef, ServerHost, Tag,
};

use super::{BlockData, NETWORK_STATE, Position, accepts_carrier, saved_id};

pub(crate) const CARRIERS: &[&str] = &[
    "minecraft:light_gray_stained_glass",
    "minecraft:blue_stained_glass",
    "minecraft:black_stained_glass",
    "minecraft:light_blue_stained_glass",
    "minecraft:brown_terracotta",
    "minecraft:lime_terracotta",
    "minecraft:green_stained_glass",
    "minecraft:green_terracotta",
    "minecraft:crafting_table",
    "minecraft:cauldron",
    "minecraft:piston",
    "minecraft:red_terracotta",
    "minecraft:blue_terracotta",
    "minecraft:cyan_terracotta",
    "minecraft:obsidian",
    "minecraft:gold_block",
    "minecraft:iron_block",
    "minecraft:bookshelf",
    "minecraft:hay_block",
    crate::gps_transport::CARRIERS[0],
    crate::gps_transport::CARRIERS[1],
];

pub(crate) fn is_carrier(block: &str) -> bool {
    CARRIERS.contains(&block) || crate::crucible::carrier(block)
}

static NODES: AttachmentType<Compound> = AttachmentType {
    namespace: "slimefun",
    name: "plain_energy_nodes",
    default: Compound::new,
    codec: Some(AttachmentCodec {
        encode: |data| Ok(data.clone()),
        decode: |data| Ok(data.clone()),
    }),
    copy_on_death: false,
    sync: false,
};

#[derive(Clone, Copy, Eq, PartialEq)]
enum Change {
    Removed,
    Dropped,
    Superseded,
}

// The world's readonly block-change hook can guard a position immediately. Its saved
// attachment remains available to the synchronous drop hook until the next writable phase.
static RETIRED: Mutex<BTreeMap<(String, Position), Change>> = Mutex::new(BTreeMap::new());

pub(crate) fn dimension(dimension: &LevelDimension) -> &str {
    match dimension {
        LevelDimension::Overworld => "minecraft:overworld",
        LevelDimension::Nether => "minecraft:the_nether",
        LevelDimension::End => "minecraft:the_end",
        LevelDimension::Custom(id) => id.id(),
    }
}

fn chunk([x, _, z]: Position) -> [i32; 2] {
    [x.div_euclid(16), z.div_euclid(16)]
}

fn key([x, y, z]: Position) -> String {
    format!("{x},{y},{z}")
}

pub(crate) fn observe_plain_change(level: &dyn LevelHost, event: &BlockChangedEvent) {
    let old = level.state_block(event.old);
    let new = level.state_block(event.new);
    let replaced_carrier = old.is_some_and(is_carrier);
    let placed_carrier = new.is_some_and(is_carrier);
    if old == new
        || !(replaced_carrier || placed_carrier)
        || old.is_some_and(crate::crucible::carrier) && new.is_some_and(crate::crucible::carrier)
    {
        return;
    }
    let address = (
        dimension(&event.dimension).to_owned(),
        [event.pos.x, event.pos.y, event.pos.z],
    );
    let mut changes = RETIRED.lock().unwrap();
    if replaced_carrier {
        changes.entry(address.clone()).or_insert(Change::Removed);
    }
    if placed_carrier && let Some(change) = changes.get_mut(&address) {
        // A different newly placed carrier cannot inherit the retired machine.
        *change = Change::Superseded;
    }
}

pub(super) fn saved_at(
    level: &dyn LevelHost,
    dimension_id: &LevelDimension,
    pos: Position,
) -> Option<String> {
    let address = (dimension(dimension_id).to_owned(), pos);
    if RETIRED.lock().unwrap().contains_key(&address) {
        return None;
    }
    let saved = level.chunk_saved_attachment(chunk(pos), NODES.namespace, NODES.name)?;
    let Tag::Compound(data) = saved.get(&key(pos).as_str().into())? else {
        return None;
    };
    saved_id(data)
}

pub(super) fn read(host: &mut dyn ServerHost, level: LevelRef, pos: Position) -> Option<BlockData> {
    let address = (level.dimension().id(host)?, pos);
    if RETIRED.lock().unwrap().contains_key(&address) {
        return None;
    }
    read_persisted(host, pos)
}

pub(super) fn read_persisted(host: &mut dyn ServerHost, pos: Position) -> Option<BlockData> {
    read_persisted_checked(host, pos).ok().flatten()
}

pub(super) fn read_persisted_checked(
    host: &mut dyn ServerHost,
    pos: Position,
) -> Result<Option<BlockData>, AttachmentError> {
    let saved = NODES.get_existing(host, Holder::Chunk(chunk(pos)))?;
    match saved
        .as_ref()
        .and_then(|saved| saved.get(&key(pos).as_str().into()))
    {
        None => Ok(None),
        Some(Tag::Compound(data)) => {
            let codec = NETWORK_STATE.codec.as_ref().ok_or(AttachmentError::Decode)?;
            (codec.decode)(data).map(Some)
        }
        Some(_) => Err(AttachmentError::Decode),
    }
}

pub(super) fn write(
    host: &mut dyn ServerHost,
    pos: Position,
    data: &BlockData,
) -> Result<(), AttachmentError> {
    let holder = Holder::Chunk(chunk(pos));
    let mut map = NODES.get_existing(host, holder)?.unwrap_or_default();
    let codec = NETWORK_STATE.codec.as_ref().ok_or(AttachmentError::Decode)?;
    map.insert(key(pos).as_str().into(), Tag::Compound((codec.encode)(data)?));
    NODES.set(host, holder, map)
}

fn remove(host: &mut dyn ServerHost, pos: Position) -> Result<(), AttachmentError> {
    let holder = Holder::Chunk(chunk(pos));
    let Some(mut map) = NODES.get_existing(host, holder)? else {
        return Ok(());
    };
    if map.remove(&key(pos).as_str().into()).is_some() {
        NODES.set(host, holder, map)?;
    }
    Ok(())
}

pub(super) fn prepare_placement(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Position,
) -> Result<(), AttachmentError> {
    if let Some(id) = level.dimension().id(host) {
        let address = (id, pos);
        let pending = RETIRED.lock().unwrap().contains_key(&address);
        if !pending {
            return Ok(());
        }
        remove(host, pos)?;
        crate::factory::clear_plain_data(host, level, pos);
        RETIRED.lock().unwrap().remove(&address);
    }
    Ok(())
}

pub(super) fn clean_level(host: &mut dyn ServerHost, level: LevelRef) {
    let Some(id) = level.dimension().id(host) else {
        return;
    };
    let retired: Vec<_> = RETIRED
        .lock()
        .unwrap()
        .keys()
        .filter(|(dimension, _)| dimension == &id)
        .map(|(_, pos)| *pos)
        .collect();
    for pos in retired {
        if host.chunk(level, chunk(pos)).is_some() && remove(host, pos).is_ok() {
            crate::factory::clear_plain_data(host, level, pos);
            RETIRED.lock().unwrap().remove(&(id.clone(), pos));
        }
    }
}

pub(super) fn clear() {
    RETIRED.lock().unwrap().clear();
}

pub(super) fn take_drop(level: &dyn LevelHost, event: &BlockDropsEvent<'_>) -> Option<String> {
    let carrier = level.state_block(event.state)?;
    if !is_carrier(carrier) {
        return None;
    }
    let pos = [event.pos.x, event.pos.y, event.pos.z];
    let saved = level.chunk_saved_attachment(chunk(pos), NODES.namespace, NODES.name)?;
    let Tag::Compound(node) = saved.get(&key(pos).as_str().into())? else {
        return None;
    };
    let id = saved_id(node)?;
    if !crate::networks::Component::for_item(&id)
        .is_some_and(|component| accepts_carrier(component, carrier))
    {
        return None;
    }
    let address = (dimension(&event.dimension).to_owned(), pos);
    let mut retired = RETIRED.lock().unwrap();
    if retired
        .get(&address)
        .is_some_and(|change| *change != Change::Removed)
    {
        return None;
    }
    retired.insert(address, Change::Dropped);
    Some(id)
}
