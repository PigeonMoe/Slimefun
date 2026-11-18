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
    factory::{
        animal_machines, assemblers, experience_collector, fluid_pump, reactors, tree_growth,
    },
    geology,
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
#[path = "network_visuals.rs"]
mod network_visuals;
#[path = "plain_network.rs"]
mod plain_network;
#[path = "trash.rs"]
pub(crate) mod trash;

pub(crate) use cargo_menu::{menu_click, menu_closed, register as register_cargo, use_node};
use cargo_topology::{CargoSettings, FILTER_SLOTS};
use network_visuals::Status;
pub(crate) use plain_network::{
    CARRIERS as PLAIN_CARRIERS, dimension as dimension_name, is_carrier as is_plain_carrier,
    observe_plain_change,
};

type Position = [i32; 3];
type Nodes = BTreeMap<Position, Node>;

#[derive(Clone, Default)]
struct BlockData {
    id: String,
    charge: u32,
    cargo: Option<CargoSettings>,
    cargo_visualizer_disabled: bool,
    reactor_production: bool,
    stored_experience: i32,
    gps_owner: Option<u128>,
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
            if value.cargo_visualizer_disabled {
                data.insert("visualizer_disabled".into(), Tag::Byte(1));
            }
            if value.reactor_production {
                data.insert("reactor_production".into(), Tag::Byte(1));
            }
            if value.stored_experience != 0 {
                data.insert("stored_exp".into(), Tag::Int(value.stored_experience));
            }
            if let Some(owner) = value.gps_owner {
                data.insert(
                    "owner".into(),
                    Tag::String(format!("{owner:032x}").as_str().into()),
                );
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
                cargo_visualizer_disabled: matches!(
                    data.get(&"visualizer_disabled".into()),
                    Some(Tag::Byte(1))
                ),
                reactor_production: matches!(
                    data.get(&"reactor_production".into()),
                    Some(Tag::Byte(1))
                ),
                stored_experience: match data.get(&"stored_exp".into()) {
                    None => 0,
                    Some(Tag::Int(points)) => *points,
                    _ => return Err(AttachmentError::Decode),
                },
                gps_owner: match data.get(&"owner".into()) {
                    None => None,
                    Some(Tag::String(owner)) => Some(
                        parse_uuid(&owner.to_string().map_err(|_| AttachmentError::Decode)?)
                            .ok_or(AttachmentError::Decode)?,
                    ),
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

pub(crate) fn enhanced_furnace(
    host: &dyn mod_api::Host,
    entity: mod_api::BlockEntityRef,
) -> Option<crate::enhanced_furnaces::Furnace> {
    let data =
        host.block_entity_saved_attachment(entity, NETWORK_STATE.namespace, NETWORK_STATE.name)?;
    crate::enhanced_furnaces::by_id(&saved_id(&data)?)
}

fn parse_uuid(value: &str) -> Option<u128> {
    let owned;
    let clean = if value.len() == 36
        && [8, 13, 18, 23]
            .into_iter()
            .all(|index| value.as_bytes()[index] == b'-')
    {
        owned = value.replace('-', "");
        owned.as_str()
    } else {
        value
    };
    (clean.len() == 32 && clean.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| u128::from_str_radix(clean, 16).ok())
        .flatten()
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
        if let HostSide::Server(server) = &host
            && [
                crate::thermal_machines::COOLING_UNIT,
                crate::thermal_machines::REACTOR_COLLANT_CELL,
                crate::heated_pressure::URANIUM,
                crate::heated_pressure::PLUTONIUM,
                crate::heated_pressure::BOOSTED_URANIUM,
                crate::heated_pressure::NETHER_ICE,
                crate::heated_pressure::ENRICHED_NETHER_ICE,
                crate::heated_pressure::NETHER_ICE_COOLANT_CELL,
                reactors::NEPTUNIUM,
                crate::network_recipes::ANDROID_MEMORY_CORE,
                trash::PORTABLE_DUSTBIN,
            ]
            .iter()
            .any(|item| item.matches(*server, event.stack))
        {
            return Some(InteractionResult::Fail);
        }
        if let HostSide::Server(host) = host {
            if crate::crucible::refuses_placement(host, event)
                || crate::composter::refuses_placement(host, event)
                || crate::auto_crafter::refuses_placement(host, event)
                || cargo_topology::block_invalid_placement(HostSide::Server(host), event)
            {
                return Some(InteractionResult::Fail);
            }
        }
        None
    }

    fn on_block_placed_by(&self, host: &mut dyn ServerHost, event: &mut OnBlockPlacedByEvent<'_>) {
        let Some(player) = event.placer else {
            return;
        };
        let Some(id) = item_id(event.stack) else {
            return;
        };
        if !network_item(&id).is_some_and(|item| item.matches(host, event.stack)) {
            return;
        }
        let Some(level) = host.level(host.dimension()) else {
            return;
        };
        let Ok((node, mut data)) = imported(&id, &toml::Table::new()) else {
            return;
        };
        if matches!(node.component, Component::GpsTransmitter { .. }) {
            data.gps_owner = Some(player.uuid());
        }
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
    Independent,
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
            | Component::GpsTransmitter { .. }
            | Component::FuelGenerator { .. }
            | Component::SolarGenerator { .. } => Network::Energy,
            Component::GeoScanner
            | Component::TrashCan
            | Component::ManualCrucible
            | Component::ManualComposter
            | Component::IgnitionChamber
            | Component::EnhancedFurnace
            | Component::DuctTape
            | Component::WitherProofObsidian
            | Component::ReactorPort => Network::Independent,
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
        Component::ManualCrucible | Component::ManualComposter => {
            return crate::crucible::carrier(actual);
        }
        Component::IgnitionChamber => return actual == crate::items::IGNITION_CHAMBER.material(),
        Component::EnhancedFurnace => return actual == "minecraft:furnace",
        Component::ReactorPort => return actual == reactors::REACTOR_ACCESS_PORT.material(),
        Component::WitherProofObsidian => {
            return actual == crate::enchantment_machines::WITHER_PROOF_OBSIDIAN.material();
        }
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
    cargo_ticks: HashMap<(LevelRef, Position), u32>,
    gps_signals: BTreeMap<String, BTreeMap<Position, (u128, u32)>>,
}

impl Networks {
    fn cargo_due(&mut self, level: LevelRef, pos: Position, delay: u32) -> bool {
        if delay == 0 {
            self.cargo_ticks.remove(&(level, pos));
            return true;
        }
        let elapsed = self.cargo_ticks.entry((level, pos)).or_default();
        if *elapsed < delay {
            *elapsed += 1;
            return false;
        }
        *elapsed = 0;
        true
    }

    fn gps_complexity(&self, player: u128) -> u32 {
        self.gps_signals
            .values()
            .flat_map(BTreeMap::values)
            .filter_map(|&(owner, signal)| (owner == player).then_some(signal))
            .fold(0u32, u32::saturating_add)
    }

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
        self.cargo_ticks.clear();
        self.gps_signals.clear();
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
        if matches!(node.component, Component::GpsTransmitter { .. }) && data.gps_owner.is_none() {
            return Err("The Paper GPS transmitter is missing a valid owner UUID.".into());
        }
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
        if crate::auto_crafter::is_machine(id) {
            host.transaction(|host, _| {
                crate::auto_crafter::restore_paper(host, level, pos, id, row)?;
                self.register(host, level, pos, node, data, true)
            })?;
        } else if node.component == Component::TrashCan {
            host.transaction(|host, _| {
                trash::restore(host, pos, row)?;
                self.register(host, level, pos, node, data, true)
            })?;
        } else {
            self.register(host, level, pos, node, data, true)?;
        }
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
        if is_plain_node(component) {
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
        | Component::GpsTransmitter { capacity }
        | Component::FuelGenerator { capacity, .. } => {
            data.charge = field("energy-charge")
                .unwrap_or("0")
                .parse::<u32>()
                .ok()
                .filter(|charge| *charge <= capacity)
                .ok_or("Stored buffer charge is invalid.")?;
            if matches!(component, Component::GpsTransmitter { .. }) {
                data.gps_owner = match field("owner") {
                    Some(value) => {
                        Some(parse_uuid(value).ok_or("Invalid Paper GPS transmitter owner UUID.")?)
                    }
                    None => None,
                };
            }
            if reactors::reactor(id).is_some() {
                data.reactor_production = match field("reactor-mode") {
                    Some("PRODUCTION") => true,
                    None | Some("GENERATOR") => false,
                    _ => return Err("Invalid Paper reactor mode."),
                };
            }
            if id == experience_collector::XP_COLLECTOR.id() {
                data.stored_experience = field("stored-exp")
                    .unwrap_or("0")
                    .parse()
                    .map_err(|_| "Invalid Paper stored experience.")?;
            }
        }
        Component::CargoInput | Component::CargoOutput { .. } => {
            data.cargo = Some(CargoSettings::from_paper(metadata, node.filters_items())?);
        }
        Component::CargoManager => {
            data.cargo_visualizer_disabled = field("visualizer").is_some();
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
        if material == "minecraft:chest" && crate::output_chest::marked(level, entity)
            || matches!(
                material,
                "minecraft:player_head" | "minecraft:player_wall_head"
            ) && crate::food_machines::marked(level, entity)
        {
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

pub(crate) fn observe_gps_change(
    networks: &Mutex<Networks>,
    level: &dyn LevelHost,
    event: &mod_api::BlockChangedEvent,
) {
    let old = level.state_block(event.old);
    if old == level.state_block(event.new)
        || !matches!(
            old,
            Some("minecraft:player_head" | "minecraft:player_wall_head")
        )
    {
        return;
    }
    let dimension = plain_network::dimension(&event.dimension);
    if let Some(signals) = networks.lock().unwrap().gps_signals.get_mut(dimension) {
        signals.remove(&[event.pos.x, event.pos.y, event.pos.z]);
    }
}

pub(crate) fn open_scanner(
    networks: &Mutex<Networks>,
    host: &mut dyn ServerHost,
    player: mod_api::PlayerRef,
    level: LevelRef,
    pos: Position,
    portable: bool,
    rolls: &Rolls,
) -> bool {
    let entity = if portable {
        None
    } else {
        if !block_data(host, level, pos).is_some_and(|data| data.id == "GPS_GEO_SCANNER") {
            return false;
        }
        let Some(entity) = host.block_entity(level, pos) else {
            return false;
        };
        Some(entity)
    };
    let complexity = networks.lock().unwrap().gps_complexity(player.uuid());
    geology::open_scanner(host, player, level, pos, entity, complexity, rolls);
    true
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
        Some(
            Component::ElectricMachine { .. }
                | Component::FuelGenerator { .. }
                | Component::ReactorPort
        )
    )
    .then_some(data.id)
}

pub(crate) fn manual_crucible_at(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Position,
) -> bool {
    level
        .block(host, pos)
        .is_some_and(|block| crate::crucible::carrier(&block.id(host)))
        && block_data(host, level, pos).is_some_and(|data| data.id == "CRUCIBLE")
}

pub(crate) fn manual_composter_at(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Position,
) -> Option<mod_api::BlockState> {
    let state = level.block_state(host, pos)?;
    if !accepts_carrier(Component::ManualComposter, &state.block().id(host)) {
        return None;
    }
    (block_data(host, level, pos)?.id == crate::composter::COMPOSTER.id()).then_some(state)
}

pub(crate) fn wither_proof_obsidian_at(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Position,
) -> bool {
    let obsidian = crate::enchantment_machines::WITHER_PROOF_OBSIDIAN;
    level
        .block(host, pos)
        .is_some_and(|block| block.id(host) == obsidian.material())
        && plain_network::read(host, level, pos).is_some_and(|data| data.id == obsidian.id())
}

pub(crate) fn ignition_chamber_at(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Position,
) -> bool {
    let chamber = crate::items::IGNITION_CHAMBER;
    level
        .block(host, pos)
        .is_some_and(|block| block.id(host) == chamber.material())
        && block_data(host, level, pos).is_some_and(|data| data.id == chamber.id())
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
    if Component::for_item(&data.id).is_some_and(is_plain_node) {
        plain_network::write(host, pos, &data)
    } else {
        NETWORK_STATE.set(host, Holder::BlockEntity(pos), data)
    }
}

fn is_plain_node(component: Component) -> bool {
    if matches!(
        component,
        Component::ManualCrucible
            | Component::ManualComposter
            | Component::ReactorPort
            | Component::WitherProofObsidian
    ) {
        return true;
    }
    matches!(
        component, Component::ElectricMachine { carrier, .. } | Component::FuelGenerator { carrier, .. }
            if is_plain_carrier(carrier)
    )
}

pub(crate) fn reactor_production(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Position,
) -> Option<bool> {
    let state = block_data(host, level, pos)?;
    reactors::reactor(&state.id)?;
    Some(state.reactor_production)
}

pub(crate) fn toggle_reactor_focus(host: &mut dyn ServerHost, level: LevelRef, pos: Position) {
    if let Some(mut state) = block_data(host, level, pos)
        && reactors::reactor(&state.id).is_some()
    {
        state.reactor_production = !state.reactor_production;
        let _ = write_data(host, pos, state);
    }
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
    let mut labels = BTreeMap::new();
    networks
        .lock()
        .unwrap()
        .cargo_ticks
        .retain(|(held_level, pos), _| {
            *held_level != level
                || active
                    .get(pos)
                    .is_some_and(|(node, _)| node.component == Component::CargoManager)
        });
    for (&pos, &(node, _)) in &active {
        if node.component == Component::TrashCan {
            trash::clear(host, pos);
        }
    }
    for (&start, &(node, _)) in &active {
        if !node.regulator() || claimed.contains(&start) {
            continue;
        }
        let group: Vec<_> = connected(&active, start, node.family())
            .into_iter()
            .filter(|pos| claimed.insert(*pos))
            .collect();
        let (status, conflict) = match node.family() {
            Network::Energy => {
                let status = if group.iter().all(|pos| active[pos].0.regulator()) {
                    Some(Status::EnergyEmpty)
                } else {
                    tick_energy(host, level, &dimension, &active, &group, rolls)
                };
                (status, Status::EnergyConflict)
            }
            Network::Cargo => {
                let terminals = group
                    .iter()
                    .copied()
                    .filter(|pos| !active[pos].0.regulator());
                let status = if terminals.clone().next().is_some() {
                    let due =
                        networks
                            .lock()
                            .unwrap()
                            .cargo_due(level, start, settings.cargo_tick_delay);
                    if due {
                        if settings.cargo_visualizer
                            && block_data(host, level, start)
                                .is_some_and(|data| !data.cargo_visualizer_disabled)
                        {
                            network_visuals::cargo_particles(host, level, terminals);
                        }
                        cargo_topology::tick(networks, host, level, &active, &group);
                    }
                    Status::CargoOnline
                } else {
                    Status::CargoEmpty
                };
                (Some(status), Status::CargoConflict)
            }
            Network::Independent => continue,
        };
        labels.insert(start, status);
        for pos in group
            .into_iter()
            .filter(|&pos| pos != start && active[&pos].0.regulator())
        {
            labels.insert(pos, Some(conflict));
        }
    }
    for (&pos, &(node, _)) in &active {
        if let Some(spec) = reactors::reactor(&node.id)
            && let Some(cooling) = reactors::cooling_percent(host, level, pos, spec)
        {
            labels.insert(pos, Some(Status::ReactorCooling(cooling)));
        }
    }
    if nodes
        .values()
        .any(|node| node.regulator() || reactors::reactor(&node.id).is_some())
    {
        network_visuals::reconcile(host, level, &dimension, labels);
    }
    let mut gps_signals = BTreeMap::new();
    for (&pos, &(node, _)) in &active {
        if let Component::GpsTransmitter { .. } = node.component
            && let Some(transmitter) = geology::transmitter(&node.id)
            && let Ok(signal) = host.transaction(|host, _| {
                let mut data = block_data(host, level, pos).ok_or(())?;
                let owner = data.gps_owner.ok_or(())?;
                data.charge = data.charge.checked_sub(transmitter.cost).ok_or(())?;
                write_data(host, pos, data).map_err(|_| ())?;
                Ok::<_, ()>((owner, transmitter.strength(pos[1])))
            })
        {
            gps_signals.insert(pos, signal);
        }
        if let Component::ElectricMachine { .. } = node.component {
            if let Some(spec) = assemblers::by_id(&node.id) {
                let spawn = host.transaction(|host, tx| {
                    let mut data = block_data(host, level, pos).ok_or(())?;
                    data.charge = data.charge.checked_sub(spec.cost).ok_or(())?;
                    let spawn = assemblers::stage(host, tx, level, pos, spec).ok_or(())?;
                    write_data(host, pos, data).map_err(|_| ())?;
                    Ok::<_, ()>(spawn)
                });
                if let Ok(spawn) = spawn {
                    spawn.apply(host, level, pos);
                }
                continue;
            }
            if crate::auto_crafter::is_machine(&node.id) {
                let crafted = host.transaction(|host, tx| {
                    let mut data = block_data(host, level, pos).ok_or(())?;
                    data.charge = data
                        .charge
                        .checked_sub(crate::auto_crafter::COST)
                        .ok_or(())?;
                    crate::auto_crafter::stage(host, tx, level, pos, &node.id)?;
                    write_data(host, pos, data).map_err(|_| ())
                });
                if crafted.is_ok() {
                    crate::auto_crafter::complete(host, level, pos);
                }
                continue;
            }
            if let Some(machine) = animal_machines::machine(&node.id) {
                let feeding = host.transaction(|host, tx| {
                    let mut data = block_data(host, level, pos).ok_or(())?;
                    data.charge = data.charge.checked_sub(machine.cost).ok_or(())?;
                    let feeding = machine.stage(host, tx, level, pos)?;
                    write_data(host, pos, data).map_err(|_| ())?;
                    feeding.apply(host)?;
                    Ok::<_, ()>(feeding)
                });
                if let Ok(feeding) = feeding {
                    feeding.particles(host, level);
                }
                continue;
            }
            if node.id == tree_growth::TREE_GROWTH_ACCELERATOR.id() {
                let target = host.transaction(|host, tx| {
                    let mut data = block_data(host, level, pos).ok_or(())?;
                    data.charge = data.charge.checked_sub(tree_growth::COST).ok_or(())?;
                    let target = tree_growth::stage(host, tx, level, pos).ok_or(())?;
                    write_data(host, pos, data).map_err(|_| ())?;
                    Ok::<_, ()>(target)
                });
                if let Ok(target) = target {
                    tree_growth::grow(host, level, target);
                }
                continue;
            }
            let _ = host.transaction(|host, tx| {
                let mut data = block_data(host, level, pos).ok_or(())?;
                if node.id == fluid_pump::FLUID_PUMP.id() {
                    data.charge = data.charge.checked_sub(fluid_pump::COST).ok_or(())?;
                    let drain = fluid_pump::stage(host, tx, level, pos, rolls)?;
                    write_data(host, pos, data).map_err(|_| ())?;
                    return drain.map_or(Ok(()), |drain| drain.apply(host, level));
                }
                if node.id == experience_collector::XP_COLLECTOR.id() {
                    data.charge = data
                        .charge
                        .checked_sub(experience_collector::COST)
                        .ok_or(())?;
                    data.stored_experience = experience_collector::collect(
                        host,
                        tx,
                        level,
                        pos,
                        data.stored_experience,
                    )?;
                    return write_data(host, pos, data).map_err(|_| ());
                }
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
    let mut networks = networks.lock().unwrap();
    if gps_signals.is_empty() {
        networks.gps_signals.remove(&dimension);
    } else {
        networks.gps_signals.insert(dimension, gps_signals);
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
        Network::Independent => return BTreeSet::new(),
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
    rolls: &Rolls,
) -> Option<Status> {
    let time = level.day_time(host).map(|time| time.rem_euclid(24_000));
    let day = time.is_some_and(|time| !(12_300..=23_850).contains(&time))
        && level.is_raining(host) == Some(false)
        && level.is_thundering(host) == Some(false);
    let (status, reactors) = host
        .transaction(|host, tx| {
            let mut available = 0u64;
            let mut demand = 0u64;
            let mut effects = Vec::new();
            let mut surviving = Vec::with_capacity(group.len());
            for &pos in group {
                let (node, charge) = active[&pos];
                let energy = match node.component {
                    Component::ElectricMachine { capacity, .. }
                    | Component::GpsTransmitter { capacity } => {
                        demand += u64::from(capacity.saturating_sub(charge));
                        0
                    }
                    Component::Capacitor(capacity) => charge.min(capacity),
                    Component::FuelGenerator { capacity, .. } => {
                        let production = if let Some(spec) = reactors::reactor(&node.id) {
                            let focus = reactor_production(host, level, pos).ok_or(())?;
                            let outcome =
                                reactors::poll(host, tx, level, pos, spec, charge, focus, rolls)?;
                            let (energy, melt) = (outcome.energy, outcome.melt);
                            if outcome.wither || melt {
                                effects.push((pos, outcome));
                            }
                            if melt {
                                continue;
                            }
                            energy
                        } else {
                            crate::factory::generate(host, tx, level, pos, &node.id, charge)?
                        };
                        charge.min(capacity) + production
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
                surviving.push(pos);
            }
            for (pos, stored) in capacity_updates(available, active, &surviving) {
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
            Ok::<_, ()>((
                Status::Energy {
                    supply: available,
                    demand,
                },
                effects,
            ))
        })
        .ok()?;
    for (pos, poll) in reactors {
        reactors::apply_effects(host, level, pos, poll);
    }
    Some(status)
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
        if let Component::ElectricMachine { capacity, .. }
        | Component::GpsTransmitter { capacity } = node.component
        {
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
    fn the_paper_composter_restores_its_identity_from_the_saved_chunk_marker() {
        let (node, data) = imported("COMPOSTER", &toml::Table::new()).unwrap();
        assert_eq!(node.component, Component::ManualComposter);
        assert!(is_plain_node(node.component));
        assert!(accepts_carrier(node.component, "minecraft:cauldron"));
        assert!(accepts_carrier(node.component, "minecraft:water_cauldron"));
        assert!(!accepts_carrier(node.component, "minecraft:composter"));

        let mut host = FakeHost::new();
        let position = [-1, 63, 18];
        plain_network::write(&mut host, position, &data).unwrap();
        let saved = host.attachments.to_nbt();
        assert!(!saved.is_empty());

        let mut reopened = FakeHost::new();
        reopened.attachments = mod_api::Attachments::from_nbt(saved);
        let record = plain_network::read_persisted(&mut reopened, position).unwrap();
        assert_eq!(record.id, "COMPOSTER");
        assert!(plain_network::read_persisted(&mut reopened, [0, 63, 18]).is_none());
    }

    #[test]
    fn each_cargo_regulator_skips_its_own_configured_number_of_ticks() {
        let level = FakeHost::new().level.level();
        let a = [0, 65, 0];
        let b = [1, 65, 0];
        let mut networks = Networks::default();
        assert!(!networks.cargo_due(level, a, 2));
        assert!(!networks.cargo_due(level, b, 2));
        assert!(!networks.cargo_due(level, a, 2));
        assert!(networks.cargo_due(level, a, 2));
        assert!(!networks.cargo_due(level, b, 2));
        assert!(networks.cargo_due(level, b, 0));
        assert!(!networks.cargo_due(level, b, 2));
    }

    #[test]
    fn gps_import_keeps_the_owner_and_complexity_uses_only_its_powered_transmitters() {
        let metadata: toml::Table =
            "owner = \"00000000-0000-0000-0000-000000000002\"\nenergy-charge = \"3\""
                .parse()
                .unwrap();
        let (_, transmitter) = imported("GPS_TRANSMITTER_2", &metadata).unwrap();
        assert_eq!((transmitter.gps_owner, transmitter.charge), (Some(2), 3));
        let codec = NETWORK_STATE.codec.as_ref().unwrap();
        let round_trip = (codec.decode)(&(codec.encode)(&transmitter).unwrap()).unwrap();
        assert_eq!(round_trip.gps_owner, Some(2));

        let mut networks = Networks::default();
        networks.gps_signals.insert(
            "minecraft:overworld".to_owned(),
            BTreeMap::from([([0, 125, 0], (2, 600)), ([1, 125, 0], (1, 600))]),
        );
        networks.gps_signals.insert(
            "minecraft:the_nether".to_owned(),
            BTreeMap::from([([0, 20, 0], (2, 20))]),
        );
        assert_eq!(networks.gps_complexity(2), 620);
        assert_eq!(networks.gps_complexity(1), 600);
        assert_eq!(networks.gps_complexity(3), 0);
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
