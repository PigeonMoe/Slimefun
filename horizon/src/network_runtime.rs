// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    fmt::Write,
    sync::{Arc, Mutex},
};

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentType, BlockDropsEvent, Compound, ConfigFileError,
    ConfigPath, Holder, HostSide, HostTransaction, InteractionResult, ItemKey, ItemPart, ItemStack,
    LevelHost, LevelRef, LightLayer, OnBlockPlacedByEvent, ServerHost, Tag, UseOnEvent,
};

use crate::{
    items::{item_id, network_item},
    machine::Settings,
    networks::Component,
    random::Rolls,
    storage::{coordinates, encoded_dimension},
};

#[path = "cargo_menu.rs"]
mod cargo_menu;
#[path = "cargo_topology.rs"]
mod cargo_topology;
#[path = "plain_network.rs"]
mod plain_network;

pub(crate) use cargo_menu::{menu_click, menu_closed, register as register_cargo, use_node};
use cargo_topology::{CargoSettings, FILTER_SLOTS};
pub(crate) use plain_network::{
    CARRIERS as PLAIN_CARRIERS, is_carrier as is_plain_carrier, observe_plain_change,
};

type Position = [i32; 3];
type Nodes = BTreeMap<Position, Node>;

#[derive(Clone, Default)]
struct BlockData {
    id: String,
    charge: u32,
    cargo: Option<CargoSettings>,
}

// Marker presence binds a registration to the actual block entity. Replacing
// a head or detector removes that marker even if the replacement is identical.
static NETWORK_STATE: AttachmentType<BlockData> = AttachmentType {
    namespace: "slimefun",
    name: "network_state",
    default: BlockData::default,
    codec: Some(AttachmentCodec {
        encode: |value| {
            let charge = i32::try_from(value.charge).map_err(|_| AttachmentError::Decode)?;
            let mut data = Compound::from([
                ("id".into(), Tag::String(value.id.as_str().into())),
                ("charge".into(), Tag::Int(charge)),
            ]);
            if let Some(cargo) = &value.cargo {
                data.insert("cargo".into(), Tag::Compound(cargo.save()));
            }
            Ok(data)
        },
        decode: |data| {
            let id = saved_id(data).ok_or(AttachmentError::Decode)?;
            let Some(Tag::Int(charge)) = data.get(&"charge".into()) else {
                return Err(AttachmentError::Decode);
            };
            Ok(BlockData {
                id,
                charge: u32::try_from(*charge).map_err(|_| AttachmentError::Decode)?,
                cargo: match data.get(&"cargo".into()) {
                    None => None,
                    Some(Tag::Compound(cargo)) => Some(CargoSettings::load(cargo)?),
                    _ => return Err(AttachmentError::Decode),
                },
            })
        },
    }),
    copy_on_death: false,
    sync: false,
};

fn saved_id(data: &Compound) -> Option<String> {
    let Tag::String(value) = data.get(&"id".into())? else {
        return None;
    };
    let id = value.to_string().ok()?;
    Component::for_item(&id)?;
    Some(id)
}

pub(crate) fn read_stack(
    data: &Compound,
    key: impl FnOnce(&str) -> Option<ItemKey>,
) -> Option<ItemStack> {
    let (Some(Tag::String(id)), Some(Tag::Int(count))) =
        (data.get(&"id".into()), data.get(&"count".into()))
    else {
        return None;
    };
    if !(1..=99).contains(count) {
        return None;
    }
    let mut stack = ItemStack::new(key(&id.to_string().ok()?)?, *count as u32)?;
    if let Some(Tag::Compound(components)) = data.get(&"components".into()) {
        stack.set_components(components.clone()).ok()?;
    }
    Some(stack)
}

pub(crate) struct NetworkPlacement(pub Arc<Mutex<Networks>>);

#[mod_api::horizon_part]
impl ItemPart for NetworkPlacement {
    fn use_on(&self, host: HostSide<'_>, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        cargo_topology::block_invalid_placement(host, event).then_some(InteractionResult::Fail)
    }

    fn on_block_placed_by(&self, host: &mut dyn ServerHost, event: &mut OnBlockPlacedByEvent<'_>) {
        if event.placer.is_none() {
            return;
        }
        let Some(id) = item_id(event.stack) else {
            return;
        };
        if !network_item(&id).is_some_and(|item| item.matches(host, event.stack)) {
            return;
        }
        let Some(level) = host.level(host.dimension()) else {
            return;
        };
        let Ok((node, data)) = imported(&id, &toml::Table::new()) else {
            return;
        };
        let _ = self
            .0
            .lock()
            .unwrap()
            .register(host, level, event.pos, node, data, false);
    }
}

pub(crate) fn restore_drop(level: &dyn LevelHost, event: &mut BlockDropsEvent<'_>) {
    if let Some(id) = plain_network::take_drop(level, event) {
        if let Some(item) = network_item(&id)
            && let Some(key) = level.item_key_by_name(item.material())
        {
            if let Some(drop) = event.drops.iter_mut().find(|drop| drop.stack.item() == key) {
                item.decorate(&mut drop.stack);
            } else if let Some(mut stack) = ItemStack::new(key, 1) {
                item.decorate(&mut stack);
                let pos =
                    [event.pos.x, event.pos.y, event.pos.z].map(|value| f64::from(value) + 0.5);
                event
                    .drops
                    .push(mod_api::ItemDrop::new(stack, pos, [0.0, 0.2, 0.0], 10));
            }
        }
        crate::factory::restore_plain_drops(level, event);
        return;
    }
    let Some(entity) = event.block_entity else {
        return;
    };
    let Some(data) =
        level.block_entity_saved_attachment(entity, NETWORK_STATE.namespace, NETWORK_STATE.name)
    else {
        return;
    };
    let Some(id) = saved_id(&data) else {
        return;
    };
    let Some(item) = network_item(&id) else {
        return;
    };
    if let Some(key) = level.item_key_by_name(item.material())
        && let Some(drop) = event.drops.iter_mut().find(|drop| drop.stack.item() == key)
    {
        item.decorate(&mut drop.stack);
    }
    if let Some(Tag::Compound(cargo)) = data.get(&"cargo".into())
        && let Ok(cargo) = CargoSettings::load(cargo)
    {
        let pos = [
            f64::from(event.pos.x) + 0.5,
            f64::from(event.pos.y) + 0.5,
            f64::from(event.pos.z) + 0.5,
        ];
        event
            .drops
            .extend(cargo.filters.values().filter_map(|data| {
                let stack = read_stack(data, |name| level.item_key_by_name(name))?;
                Some(mod_api::ItemDrop::new(stack, pos, [0.0, 0.2, 0.0], 10))
            }));
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Node {
    id: String,
    component: Component,
    channel: u8,
    passes_unfiltered: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Network {
    Energy,
    Cargo,
}

impl Node {
    fn filters_items(&self) -> bool {
        matches!(
            self.component,
            Component::CargoInput | Component::CargoOutput { advanced: true }
        )
    }

    fn cargo_settings(
        &self,
        host: &mut dyn ServerHost,
        level: LevelRef,
        pos: Position,
    ) -> CargoSettings {
        block_data(host, level, pos)
            .and_then(|data| data.cargo)
            .unwrap_or_else(|| CargoSettings::legacy(self.channel, self.passes_unfiltered))
    }

    fn family(&self) -> Network {
        match self.component {
            Component::EnergyRegulator
            | Component::EnergyConnector
            | Component::Capacitor(_)
            | Component::ElectricMachine { .. }
            | Component::FuelGenerator { .. }
            | Component::SolarGenerator { .. } => Network::Energy,
            _ => Network::Cargo,
        }
    }

    fn regulator(&self) -> bool {
        matches!(
            self.component,
            Component::EnergyRegulator | Component::CargoManager
        )
    }

    fn relays(&self) -> bool {
        self.regulator()
            || matches!(
                self.component,
                Component::EnergyConnector | Component::Capacitor(_) | Component::CargoConnector
            )
    }

    fn accepts_block(&self, host: &dyn ServerHost, level: LevelRef, pos: Position) -> bool {
        let Some(actual) = level.block_state(host, pos).map(|state| state.block()) else {
            return false;
        };
        accepts_carrier(self.component, &actual.id(host))
    }
}

fn accepts_carrier(component: Component, actual: &str) -> bool {
    let expected = match component {
        Component::SolarGenerator { .. } => ["minecraft:daylight_detector"; 2],
        Component::ElectricMachine { carrier, .. } | Component::FuelGenerator { carrier, .. } => {
            if carrier == "minecraft:player_head" {
                ["minecraft:player_head", "minecraft:player_wall_head"]
            } else {
                [carrier; 2]
            }
        }
        Component::CargoInput | Component::CargoOutput { .. } => ["minecraft:player_wall_head"; 2],
        _ => ["minecraft:player_head", "minecraft:player_wall_head"],
    };
    expected.contains(&actual)
}

#[derive(Default)]
pub(crate) struct Networks {
    levels: BTreeMap<String, Arc<Nodes>>,
    menus: BTreeMap<u128, cargo_menu::Session>,
    round_robin: HashMap<(LevelRef, Position), usize>,
}

impl Networks {
    fn nodes(&mut self, host: &dyn ServerHost, dimension: &str) -> Result<Arc<Nodes>, String> {
        if let Some(nodes) = self.levels.get(dimension) {
            return Ok(nodes.clone());
        }
        let nodes = Arc::new(load(host, dimension)?);
        self.levels.insert(dimension.to_owned(), nodes.clone());
        Ok(nodes)
    }

    pub(crate) fn clear(&mut self) {
        self.levels.clear();
        self.menus.clear();
        self.round_robin.clear();
        plain_network::clear();
    }

    pub(crate) fn restore(
        &mut self,
        host: &mut dyn ServerHost,
        level: LevelRef,
        pos: Position,
        id: &str,
        metadata: &toml::Table,
        row: &toml::Table,
    ) -> Result<String, String> {
        let (node, mut data) = imported(id, metadata)?;
        if let Some(cargo) = &mut data.cargo
            && node.filters_items()
            && row.contains_key("paper_inventory")
        {
            crate::storage::validate_inventory_preset(row, id)?;
            if let Some(items) = crate::storage::decode_inventory(host, row)? {
                for (slot, item) in items {
                    if !FILTER_SLOTS.contains(&(slot as usize)) {
                        return Err(format!("Unexpected Paper cargo inventory slot {slot}."));
                    }
                    cargo.filters.insert(
                        slot as usize,
                        item.to_nbt(host)
                            .ok_or("Paper cargo filter item has no item registry id.")?,
                    );
                }
                cargo.imported_paper = true;
            }
        }
        self.register(host, level, pos, node, data, true)?;
        Ok(format!("Restored the Slimefun network node {id}."))
    }

    fn register(
        &mut self,
        host: &mut dyn ServerHost,
        level: LevelRef,
        pos: Position,
        node: Node,
        data: BlockData,
        import: bool,
    ) -> Result<(), String> {
        if !level.in_place(host) || !node.accepts_block(host, level, pos) {
            return Err("Load the matching Paper network block in the active dimension.".into());
        }
        let component = node.component;
        if is_plain_machine(component) {
            plain_network::prepare_placement(host, level, pos)
                .map_err(|_| "The block's previous machine state could not be cleared.")?;
        }
        let dimension = level
            .dimension()
            .id(host)
            .ok_or("Unknown Horizon dimension.")?;
        let current = self.nodes(host, &dimension)?;
        let old = block_data(host, level, pos);
        if old.as_ref().is_some_and(|old| old.id != data.id) {
            return Err("A different Slimefun network node is registered there.".into());
        }
        if import
            && data
                .cargo
                .as_ref()
                .is_some_and(|cargo| cargo.imported_paper)
            && old
                .as_ref()
                .and_then(|old| old.cargo.as_ref())
                .is_some_and(|cargo| !cargo.imported_paper && !cargo.filters.is_empty())
        {
            return Err("Cargo inventory already contains filter items.".into());
        }
        match current.get(&pos) {
            Some(existing) if existing == &node => {}
            _ => {
                let mut next = (*current).clone();
                next.insert(pos, node);
                save(host, &dimension, &next)?;
                self.levels.insert(dimension, Arc::new(next));
            }
        }
        let restore_inventory = import
            && data
                .cargo
                .as_ref()
                .is_some_and(|cargo| cargo.imported_paper)
            && old.as_ref().is_some_and(|previous| {
                previous
                    .cargo
                    .as_ref()
                    .is_none_or(|cargo| !cargo.imported_paper && cargo.filters.is_empty())
            });
        if old.is_none() || restore_inventory {
            if let Component::Capacitor(capacity) = component {
                set_capacitor_profile(host, level, pos, data.charge, capacity);
            }
            write_data(host, pos, data)
                .map_err(|_| "The block entity could not store its Slimefun network marker.")?;
        }
        Ok(())
    }
}

fn imported(id: &str, metadata: &toml::Table) -> Result<(Node, BlockData), &'static str> {
    let component =
        Component::for_item(id).ok_or("This Paper network component is not supported.")?;
    let field = |key| metadata.get(key).and_then(toml::Value::as_str);
    let node = Node {
        id: id.to_owned(),
        component,
        channel: 0,
        passes_unfiltered: true,
    };
    let mut data = BlockData {
        id: id.to_owned(),
        ..BlockData::default()
    };
    match component {
        Component::Capacitor(capacity)
        | Component::ElectricMachine { capacity, .. }
        | Component::FuelGenerator { capacity, .. } => {
            data.charge = field("energy-charge")
                .unwrap_or("0")
                .parse::<u32>()
                .ok()
                .filter(|charge| *charge <= capacity)
                .ok_or("Stored capacitor charge is invalid.")?;
        }
        Component::CargoInput | Component::CargoOutput { .. } => {
            data.cargo = Some(CargoSettings::from_paper(metadata, node.filters_items())?);
        }
        _ => {}
    }
    Ok((node, data))
}

fn file(dimension: &str) -> ConfigPath {
    ConfigPath::new(&format!("network/{}.toml", encoded_dimension(dimension)))
        .expect("hexadecimal dimension name is a valid path")
}

fn load(host: &dyn ServerHost, dimension: &str) -> Result<Nodes, String> {
    let data = match host.world_data_files().read(&file(dimension)) {
        Ok(bytes) => bytes,
        Err(ConfigFileError::NotFound) => return Ok(Nodes::new()),
        Err(error) => {
            return Err(format!(
                "Slimefun could not read its network registrations: {error}"
            ));
        }
    };
    let table = std::str::from_utf8(&data)
        .ok()
        .and_then(|text| text.parse::<toml::Table>().ok())
        .filter(|table| table.get("format").and_then(toml::Value::as_integer) == Some(1))
        .ok_or("Invalid Slimefun network registrations.")?;
    let records = table
        .get("nodes")
        .and_then(toml::Value::as_array)
        .ok_or("Invalid Slimefun network node list.")?;
    let mut nodes = Nodes::new();
    for value in records {
        let row = value.as_table().ok_or("Invalid Slimefun network node.")?;
        let pos = coordinates(row).ok_or("Invalid Slimefun network node position.")?;
        let id = row
            .get("id")
            .and_then(toml::Value::as_str)
            .ok_or("Missing Slimefun network ID.")?;
        let component = Component::for_item(id).ok_or("Unknown Slimefun network component.")?;
        let channel = row
            .get("channel")
            .and_then(toml::Value::as_integer)
            .filter(|channel| (0..16).contains(channel))
            .ok_or("Invalid Slimefun network channel.")? as u8;
        let passes_unfiltered = row
            .get("passes_unfiltered")
            .and_then(toml::Value::as_bool)
            .ok_or("Invalid Slimefun cargo filter state.")?;
        if nodes
            .insert(
                pos,
                Node {
                    id: id.to_owned(),
                    component,
                    channel,
                    passes_unfiltered,
                },
            )
            .is_some()
        {
            return Err("Duplicate position in Slimefun network registrations.".into());
        }
    }
    Ok(nodes)
}

fn save(host: &dyn ServerHost, dimension: &str, nodes: &Nodes) -> Result<(), String> {
    let mut text = String::from("format = 1\n");
    for (pos, node) in nodes {
        let _ = writeln!(
            text,
            "\n[[nodes]]\nposition = {pos:?}\nid = {:?}\nchannel = {}\npasses_unfiltered = {}",
            node.id, node.channel, node.passes_unfiltered
        );
    }
    host.world_data_files()
        .write(&file(dimension), text.as_bytes())
        .map_err(|error| format!("Slimefun could not save its network registrations: {error}"))
}

pub(crate) fn protect_piston(level: &dyn LevelHost, event: &mod_api::PistonPreEvent<'_>) -> bool {
    let point = |pos: mod_api::BlockPos| [pos.x, pos.y, pos.z];
    let piston = point(event.piston);
    let protected = |pos| protected_block(level, &event.dimension, pos);
    let step = |pos: Position| std::array::from_fn(|i| pos[i].saturating_add(event.direction[i]));
    protected(piston)
        || event
            .push
            .iter()
            .copied()
            .map(point)
            .any(|pos| protected(pos) || protected(step(pos)))
        || event.destroy.iter().copied().map(point).any(protected)
}

fn protected_block(
    level: &dyn LevelHost,
    dimension: &mod_api::LevelDimension,
    pos: Position,
) -> bool {
    let Some(material) = level
        .block_state(pos)
        .and_then(|state| level.state_block(state))
    else {
        return false;
    };
    let id = if is_plain_carrier(material) {
        plain_network::saved_at(level, dimension, pos)
    } else if let Some(entity) = level.block_entity_at(pos) {
        if material == "minecraft:chest" && crate::output_chest::marked(level, entity) {
            return true;
        }
        level
            .block_entity_saved_attachment(entity, NETWORK_STATE.namespace, NETWORK_STATE.name)
            .as_ref()
            .and_then(saved_id)
    } else {
        None
    };
    id.and_then(|id| Component::for_item(&id))
        .is_some_and(|component| accepts_carrier(component, material))
}

pub(crate) fn charge(host: &mut dyn ServerHost, level: LevelRef, pos: Position) -> Option<u32> {
    block_data(host, level, pos).map(|data| data.charge)
}

pub(crate) fn factory_identity(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Position,
) -> Option<String> {
    let data = block_data(host, level, pos)?;
    matches!(
        Component::for_item(&data.id),
        Some(Component::ElectricMachine { .. } | Component::FuelGenerator { .. })
    )
    .then_some(data.id)
}

fn block_data(host: &mut dyn ServerHost, level: LevelRef, pos: Position) -> Option<BlockData> {
    if !level.in_place(host) {
        return None;
    }
    if level
        .block(host, pos)
        .is_some_and(|actual| is_plain_carrier(&actual.id(host)))
    {
        return plain_network::read(host, level, pos);
    }
    if host.block_entity(level, pos).is_none() {
        return None;
    }
    NETWORK_STATE
        .get_existing(host, Holder::BlockEntity(pos))
        .ok()
        .flatten()
}

fn write_data(
    host: &mut dyn ServerHost,
    pos: Position,
    data: BlockData,
) -> Result<(), AttachmentError> {
    if Component::for_item(&data.id).is_some_and(is_plain_machine) {
        plain_network::write(host, pos, &data)
    } else {
        NETWORK_STATE.set(host, Holder::BlockEntity(pos), data)
    }
}

fn is_plain_machine(component: Component) -> bool {
    matches!(
        component, Component::ElectricMachine { carrier, .. } | Component::FuelGenerator { carrier, .. }
            if is_plain_carrier(carrier)
    )
}

pub(crate) fn tick(
    networks: &Mutex<Networks>,
    host: &mut dyn ServerHost,
    level: LevelRef,
    settings: Settings,
    rolls: &Rolls,
) {
    if !level.in_place(host) {
        return;
    }
    plain_network::clean_level(host, level);
    if !level
        .game_time(host)
        .is_some_and(|time| time.rem_euclid(10) == 0)
    {
        return;
    }
    let Some(dimension) = level.dimension().id(host) else {
        return;
    };
    let Ok(nodes) = networks.lock().unwrap().nodes(host, &dimension) else {
        return;
    };
    let active: BTreeMap<_, _> = nodes
        .iter()
        .filter_map(|(&pos, node)| {
            node.accepts_block(host, level, pos)
                .then(|| block_data(host, level, pos))
                .flatten()
                .filter(|data| data.id == node.id)
                .map(|data| (pos, (node, data.charge)))
        })
        .collect();
    let mut claimed = BTreeSet::new();
    for (&start, &(node, _)) in &active {
        if !node.regulator() || claimed.contains(&start) {
            continue;
        }
        let group: Vec<_> = connected(&active, start, node.family())
            .into_iter()
            .filter(|pos| claimed.insert(*pos))
            .collect();
        match node.family() {
            Network::Energy => tick_energy(host, level, &dimension, &active, &group),
            Network::Cargo => cargo_topology::tick(networks, host, level, &active, &group),
        }
    }
    for (&pos, &(node, _)) in &active {
        if let Component::ElectricMachine { .. } = node.component {
            let _ = host.transaction(|host, _| {
                let mut data = block_data(host, level, pos).ok_or(())?;
                if let Some(charge) =
                    crate::factory::tick(host, level, settings, rolls, pos, &node.id, data.charge)
                {
                    data.charge = charge;
                    write_data(host, pos, data).map_err(|_| ())?;
                }
                Ok::<_, ()>(())
            });
        }
    }
}

fn connected(
    active: &BTreeMap<Position, (&Node, u32)>,
    start: Position,
    family: Network,
) -> BTreeSet<Position> {
    // Paper's discoverNeighbors scans each axis, including range + 1.
    let limit = match family {
        Network::Energy => 7,
        Network::Cargo => 6,
    };
    let mut seen = BTreeSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some(pos) = queue.pop_front() {
        for axis in 0..3 {
            for offset in (-limit..=limit).filter(|offset| *offset != 0) {
                let Some(value) = pos[axis].checked_add(offset) else {
                    continue;
                };
                let mut neighbor = pos;
                neighbor[axis] = value;
                let Some(&(node, _)) = active.get(&neighbor) else {
                    continue;
                };
                if node.family() == family && seen.insert(neighbor) && node.relays() {
                    queue.push_back(neighbor);
                }
            }
        }
    }
    seen
}

fn tick_energy(
    host: &mut dyn ServerHost,
    level: LevelRef,
    dimension: &str,
    active: &BTreeMap<Position, (&Node, u32)>,
    group: &[Position],
) {
    let time = level.day_time(host).map(|time| time.rem_euclid(24_000));
    let day = time.is_some_and(|time| !(12_300..=23_850).contains(&time))
        && level.is_raining(host) == Some(false)
        && level.is_thundering(host) == Some(false);
    let _ = host.transaction(|host, tx| {
        let mut available = 0u64;
        for &pos in group {
            let (node, charge) = active[&pos];
            let energy = match node.component {
                Component::Capacitor(capacity) => charge.min(capacity),
                Component::FuelGenerator { capacity, .. } => {
                    charge.min(capacity)
                        + crate::factory::generate(host, tx, level, pos, &node.id, charge)?
                }
                Component::SolarGenerator {
                    day: sun,
                    night: moon,
                } if dimension == "minecraft:overworld"
                    && level.brightness(host, LightLayer::Sky, pos) == Some(15) =>
                {
                    if day {
                        sun
                    } else {
                        moon
                    }
                }
                _ => 0,
            };
            available += u64::from(energy);
        }
        for (pos, stored) in capacity_updates(available, active, group) {
            let mut data = block_data(host, level, pos).ok_or(())?;
            if data.id != active[&pos].0.id {
                return Err(());
            }
            if let Component::Capacitor(capacity) = active[&pos].0.component
                && capacitor_tier(data.charge, capacity) != capacitor_tier(stored, capacity)
            {
                set_capacitor_profile(host, level, pos, stored, capacity);
            }
            data.charge = stored;
            write_data(host, pos, data).map_err(|_| ())?;
        }
        Ok::<(), ()>(())
    });
}

fn capacitor_tier(charge: u32, capacity: u32) -> usize {
    // Paper's boundary values 25%, 50% and 75% retain the lower texture.
    (charge.saturating_mul(4).saturating_sub(1) / capacity).min(3) as usize
}

fn set_capacitor_profile(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Position,
    charge: u32,
    capacity: u32,
) {
    let Some(skull) = host.block_entity(level, pos) else {
        return;
    };
    let texture = &crate::network_recipes::CAPACITOR_TEXTURES[capacitor_tier(charge, capacity)];
    let profile = Tag::compound([
        ("id", Tag::IntArray(texture.profile.to_vec())),
        (
            "properties",
            Tag::List(vec![Tag::compound([
                ("name", Tag::String("textures".into())),
                ("value", Tag::String(texture.value.into())),
            ])]),
        ),
    ]);
    let _ = skull.merge_data(host, Compound::from([("profile".into(), profile)]));
}

fn capacity_updates(
    mut available: u64,
    active: &BTreeMap<Position, (&Node, u32)>,
    group: &[Position],
) -> Vec<(Position, u32)> {
    let mut updates = Vec::new();
    // Existing consumer energy is already in that machine. It does not enter the shared supply.
    for &pos in group {
        let (node, previous) = active[&pos];
        if let Component::ElectricMachine { capacity, .. } = node.component {
            let charge = available.min(u64::from(capacity.saturating_sub(previous))) as u32;
            if charge != 0 {
                updates.push((pos, previous + charge));
                available -= u64::from(charge);
            }
        }
    }
    for &pos in group {
        let (node, previous) = active[&pos];
        if let Component::Capacitor(capacity) = node.component {
            let stored = available.min(u64::from(capacity)) as u32;
            available -= u64::from(stored);
            if stored != previous {
                updates.push((pos, stored));
            }
        }
    }
    // Paper fills generator buffers after capacitors from the same supply.
    for &pos in group {
        let (node, previous) = active[&pos];
        if let Component::FuelGenerator { capacity, .. } = node.component {
            let stored = available.min(u64::from(capacity)) as u32;
            available -= u64::from(stored);
            if stored != previous {
                updates.push((pos, stored));
            }
        }
    }
    updates
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Identifier, testing::FakeHost};

    fn node(id: &str) -> Node {
        imported(id, &toml::Table::new()).unwrap().0
    }

    #[test]
    fn a_paper_network_fills_consumers_then_capacitors_then_generator_buffers() {
        let generator = node("LAVA_GENERATOR_2");
        let machine = node("ELECTRIC_FURNACE");
        let capacitor = node("SMALL_CAPACITOR");
        let active = BTreeMap::from([
            ([0, 70, 0], (&generator, 200)),
            ([1, 70, 0], (&capacitor, 0)),
            ([2, 70, 0], (&machine, 0)),
        ]);
        let group: Vec<_> = active.keys().copied().collect();
        assert_eq!(
            capacity_updates(220, &active, &group),
            vec![([2, 70, 0], 64), ([1, 70, 0], 128), ([0, 70, 0], 28)]
        );
    }

    #[test]
    fn paper_network_edges_are_axial_and_terminals_do_not_relay() {
        let manager = node("CARGO_MANAGER");
        let relay = node("CARGO_NODE");
        let input = node("CARGO_NODE_INPUT");
        let output = node("CARGO_NODE_OUTPUT");
        let active = BTreeMap::from([
            ([0, 70, 0], (&manager, 0)),
            ([6, 70, 0], (&relay, 0)),
            ([6, 70, 6], (&input, 0)),
            ([6, 70, 5], (&output, 0)),
            ([6, 70, 12], (&output, 0)),
            ([1, 70, 1], (&output, 0)),
        ]);
        assert_eq!(
            connected(&active, [0, 70, 0], Network::Cargo),
            BTreeSet::from([[0, 70, 0], [6, 70, 0], [6, 70, 6], [6, 70, 5]])
        );
    }

    #[test]
    fn capacitor_supply_is_stored_in_capacity_order_without_overflow() {
        let first = node("SMALL_CAPACITOR");
        let second = node("MEDIUM_CAPACITOR");
        let a = [1, 70, 3];
        let b = [3, 70, 3];
        let active = BTreeMap::from([(a, (&first, 120)), (b, (&second, 2))]);
        assert_eq!(
            capacity_updates(142, &active, &[a, b]),
            vec![(a, 128), (b, 14)]
        );
        assert_eq!(
            capacity_updates(u64::MAX, &active, &[a, b]),
            vec![(a, 128), (b, 512)]
        );
    }

    #[test]
    fn charge_reaches_machine_buffers_before_remaining_power_returns_to_capacitors() {
        let furnace = node("ELECTRIC_FURNACE");
        let capacitor = node("SMALL_CAPACITOR");
        let powered = [2, 70, 0];
        let stored = [1, 70, 0];
        let active = BTreeMap::from([(stored, (&capacitor, 70)), (powered, (&furnace, 32))]);
        assert_eq!(
            capacity_updates(75, &active, &[stored, powered]),
            vec![(powered, 64), (stored, 43)]
        );
    }

    #[test]
    fn filter_stacks_round_trip_named_material_and_components() {
        let mut host = FakeHost::new();
        let stone = crate::items::Kind::Vanilla("minecraft:stone");
        host.registries
            .resolve_items([Identifier::parse(stone.id()).unwrap()])
            .unwrap();
        let mut stack = stone.stack(&host, 2).unwrap();
        stack
            .set_components(Compound::from([(
                "minecraft:custom_name".into(),
                Tag::String("Marked".into()),
            )]))
            .unwrap();
        assert_eq!(
            read_stack(
                &stack.to_nbt(&host).unwrap(),
                |id| cargo_topology::item_key(&host, id)
            ),
            Some(stack)
        );
    }
}
