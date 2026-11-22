// SPDX-License-Identifier: GPL-3.0-only

use std::{
    cell::OnceCell,
    collections::{BTreeMap, HashMap},
    fmt::Write,
    sync::{Arc, Mutex},
};

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentType, BlockDropsEvent, BlockItemPlacePreEvent,
    Compound, ConfigFileError, ConfigPath, Flow, Holder, HostSide, HostTransaction,
    InteractionResult, ItemKey, ItemPart, ItemStack, LevelHost, LevelRef, LightLayer,
    OnBlockPlacedByEvent, ServerHost, Tag, UseOnEvent,
};

use crate::{
    factory::{
        animal_machines, assemblers, crop_growth, experience_collector, fluid_pump, reactors,
        tree_growth,
    },
    geo_deposits, geology,
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
pub(crate) use cargo_topology::paper_text;
#[path = "network_discovery.rs"]
mod network_discovery;
#[path = "network_visuals.rs"]
mod network_visuals;
#[path = "plain_network.rs"]
mod plain_network;
#[path = "trash.rs"]
pub(crate) mod trash;

pub(crate) use cargo_menu::{menu_click, menu_closed, register as register_cargo, use_node};
use cargo_topology::{CargoSettings, FILTER_SLOTS};
use network_discovery::Discovery;
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
    elevator_floor: Option<String>,
}

impl BlockData {
    fn rename_floor(&mut self, owner: u128, name: &str) -> bool {
        if self.gps_owner != Some(owner) {
            return false;
        }
        self.elevator_floor = Some(name.to_owned());
        true
    }
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
            if let Some(floor) = &value.elevator_floor {
                data.insert("floor".into(), Tag::String(floor.as_str().into()));
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
                elevator_floor: match data.get(&"floor".into()) {
                    None => None,
                    Some(Tag::String(floor)) => {
                        Some(floor.to_string().map_err(|_| AttachmentError::Decode)?)
                    }
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

pub(crate) fn parse_uuid(value: &str) -> Option<u128> {
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
                crate::network_recipes::CARGO_MOTOR,
                crate::infused::INFUSED_MAGNET,
                crate::elemental_staves::LAVA_CRYSTAL,
                crate::portable_crafter::PORTABLE_CRAFTER,
                crate::tape_measure::TAPE_MEASURE,
                trash::PORTABLE_DUSTBIN,
            ]
            .iter()
            .any(|item| item.matches(*server, event.stack))
        {
            return Some(InteractionResult::Fail);
        }
        if let HostSide::Server(host) = host {
            if crate::availability::stack_denial(host.dimension(), event.stack).is_some() {
                return Some(InteractionResult::Fail);
            }
            if crate::items::GOLD_24K_BLOCK.matches(host, event.stack)
                && !host.player(event.player).is_some_and(|player| {
                    crate::research::can_use(host, player, &crate::recipes::GOLDEN_CITY)
                })
            {
                return Some(InteractionResult::Fail);
            }
            if geology::GPS_EMERGENCY_TRANSMITTER.matches(host, event.stack)
                && !host.player(event.player).is_some_and(|player| {
                    crate::research::can_use(host, player, &geology::EMERGENCY_WAYPOINT)
                })
            {
                return Some(InteractionResult::Fail);
            }
            if (event.face != [0, 1, 0]
                && item_id(event.stack)
                    .is_some_and(|id| crate::android_program::mode(&id).is_some()))
                || crate::crucible::refuses_placement(host, event)
                || crate::composter::refuses_placement(host, event)
                || crate::auto_crafter::refuses_placement(host, event)
                || crate::elevator::refuses_placement(host, event)
                || cargo_topology::deny_unresearched_cargo(host, event)
            {
                return Some(InteractionResult::Fail);
            }
        }
        None
    }

    fn block_item_place_pre(
        &self,
        host: &mut dyn ServerHost,
        event: &mut BlockItemPlacePreEvent<'_>,
    ) -> Flow {
        if cargo_topology::deny_cargo_placement(host, event) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn on_block_placed_by(&self, host: &mut dyn ServerHost, event: &mut OnBlockPlacedByEvent<'_>) {
        let Some(player) = event.placer else {
            return;
        };
        let Some(id) = item_id(event.stack) else {
            return;
        };
        if crate::availability::use_denial(host.dimension(), &id).is_some()
            || !network_item(&id).is_some_and(|item| item.matches(host, event.stack))
        {
            return;
        }
        let Some(level) = host.level(host.dimension()) else {
            return;
        };
        let Ok((node, mut data)) = imported(&id, &toml::Table::new()) else {
            return;
        };
        if matches!(node.component, Component::GpsTransmitter { .. })
            || crate::gps_transport::needs_owner(&id)
        {
            data.gps_owner = Some(player.uuid());
        }
        if let Some(cargo) = &mut data.cargo {
            cargo.owner = Some(player.uuid());
            cargo.blacklist = !node.filters_items();
            cargo.lore = node.filters_items();
        }
        let android = node.component == Component::Android;
        let geo = id == geology::GEO_MINER.id();
        let mut replaced_geo = false;
        let placed = host.transaction(|host, _| {
            if android && !crate::android_machine::init(host, level, event.pos, player) {
                return Err("Could not initialize the Programmable Android.".to_owned());
            }
            let mut networks = self.0.lock().unwrap();
            if !geo && let Some(dimension) = level.dimension().id(host) {
                replaced_geo = networks.nodes(host, &dimension).is_ok_and(|nodes| {
                    nodes
                        .get(&event.pos)
                        .is_some_and(|node| node.id == geology::GEO_MINER.id())
                });
            }
            networks.register(host, level, event.pos, node, data, false)
        });
        if placed.is_ok() && (geo || replaced_geo) {
            network_visuals::geo_miner_placement(host, level, event.pos, geo);
        }
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
            | Component::GpsControlPanel
            | Component::GpsEmergencyTransmitter
            | Component::GpsDevice(_)
            | Component::Android
            | Component::AndroidInterface
            | Component::TrashCan
            | Component::ManualCrucible
            | Component::ManualComposter
            | Component::IgnitionChamber
            | Component::IngredientHead
            | Component::EnhancedFurnace
            | Component::InfusedHopper
            | Component::DuctTape
            | Component::DecorativeGold
            | Component::BlastResistant(_)
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
        Component::GpsDevice(kind) => return actual == kind.material(),
        Component::Android => return actual == crate::android_machine::DEVICE,
        Component::AndroidInterface => return actual == "minecraft:dispenser",
        Component::IgnitionChamber => return actual == crate::items::IGNITION_CHAMBER.material(),
        Component::EnhancedFurnace => return actual == "minecraft:furnace",
        Component::InfusedHopper => return actual == "minecraft:hopper",
        Component::ReactorPort => return actual == reactors::REACTOR_ACCESS_PORT.material(),
        Component::DecorativeGold => return actual == "minecraft:gold_block",
        Component::BlastResistant(kind) => return actual == kind.material(),
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
    discoveries: BTreeMap<String, Discovery>,
    gps_signals: BTreeMap<String, BTreeMap<Position, (u128, u32)>>,
}

impl Networks {
    pub(crate) fn gps_network(&self, player: u128) -> Vec<(String, Position, u32)> {
        self.gps_signals
            .iter()
            .flat_map(|(dimension, signals)| {
                signals.iter().filter_map(move |(&pos, &(owner, signal))| {
                    (owner == player).then(|| (dimension.clone(), pos, signal))
                })
            })
            .collect()
    }

    pub(crate) fn gps_complexity(&self, player: u128) -> u32 {
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
        self.discoveries.clear();
        self.gps_signals.clear();
        plain_network::clear();
    }

    pub(crate) fn unload_level(&mut self, dimension: &str) {
        self.discoveries.remove(dimension);
    }

    pub(crate) fn installed_menu(
        &mut self,
        host: &mut dyn ServerHost,
        level: LevelRef,
        pos: Position,
        expected_id: Option<&str>,
    ) -> Result<(String, Component), String> {
        let error =
            "An inventory-only Paper archive requires a matching, registered Slimefun block.";
        if !level.in_place(host) {
            return Err(error.into());
        }
        let dimension = level
            .dimension()
            .id(host)
            .ok_or("Unknown Horizon dimension.")?;
        let nodes = self.nodes(host, &dimension)?;
        let node = nodes
            .get(&pos)
            .filter(|node| {
                expected_id.is_none_or(|id| node.id == id) && node.accepts_block(host, level, pos)
            })
            .ok_or(error)?;
        if block_data(host, level, pos).is_none_or(|data| data.id != node.id) {
            return Err(error.into());
        }
        Ok((node.id.clone(), node.component))
    }

    #[cfg(test)]
    pub(crate) fn bind_fixture_node(
        &mut self,
        host: &mut dyn ServerHost,
        level: LevelRef,
        pos: Position,
        id: &str,
        metadata: &toml::Table,
        marked: bool,
    ) {
        let (node, data) = imported(id, metadata).unwrap();
        let dimension = level.dimension().id(host).unwrap();
        Arc::make_mut(self.levels.entry(dimension).or_default()).insert(pos, node);
        if marked {
            write_data(host, pos, data).unwrap();
        }
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
        if (matches!(node.component, Component::GpsTransmitter { .. })
            || crate::gps_transport::needs_owner(id))
            && data.gps_owner.is_none()
        {
            return Err("The Paper GPS device is missing a valid owner UUID.".into());
        }
        let cargo_receipt = if let Some(cargo) = &mut data.cargo
            && node.filters_items()
            && row.contains_key("paper_inventory")
        {
            crate::storage::validate_block_inventory_preset(row, id)?;
            let items = crate::storage::decode_archived_slots(host, row, 54)?;
            cargo.filters = paper_cargo_filters(host, items)?;
            cargo.imported_paper = true;
            if !level.in_place(host) {
                return Err(
                    "Load the matching Paper network block in the active dimension.".into(),
                );
            }
            let Some(receipt) = crate::storage::prepare_paired_menu_receipt(host, pos, row, id)?
            else {
                return Ok(format!(
                    "The Paper inventory for {id} was already restored."
                ));
            };
            Some(receipt)
        } else {
            None
        };
        if crate::auto_crafter::is_machine(id) {
            host.transaction(|host, _| {
                crate::auto_crafter::restore_paper(host, level, pos, id, row)?;
                self.register(host, level, pos, node, data, true)
            })?;
        } else if node.component == Component::Android {
            host.transaction(|host, _| {
                if !node.accepts_block(host, level, pos) {
                    return Err("Load the original Paper Android head first.".to_owned());
                }
                crate::android_machine::restore_paper(host, level, pos, id, metadata, row)?;
                self.register(host, level, pos, node, data, true)
            })?;
        } else if node.component == Component::TrashCan {
            host.transaction(|host, _| {
                trash::restore(host, pos, row)?;
                self.register(host, level, pos, node, data, true)
            })?;
        } else if let Some(receipt) = cargo_receipt {
            host.transaction(|host, _| {
                receipt.save(host)?;
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
        let old = read_registration_marker(host, pos, component)?;
        if old.as_ref().is_some_and(|old| old.id != data.id) {
            return Err("A different Slimefun network node is registered there.".into());
        }
        let fresh_regulator = node.regulator() && old.is_none();
        let current = self.nodes(host, &dimension)?;
        let saved_node = current.get(&pos);
        let node = saved_node
            .filter(|saved| old.is_some() && saved.id == node.id)
            .cloned()
            .unwrap_or(node);
        let marker = merge_import(old, data, &node, import)?;
        let next = (saved_node != Some(&node)).then(|| {
            let mut next = (*current).clone();
            next.insert(pos, node);
            next
        });
        host.transaction(|host, _| {
            if let Some(marker) = marker {
                if let Component::Capacitor(capacity) = component {
                    set_capacitor_profile(host, level, pos, marker.charge, capacity);
                }
                write_data(host, pos, marker)
                    .map_err(|_| "The block entity could not store its Slimefun network marker.")?;
            }
            if let Some(next) = &next {
                save(host, &dimension, next)?;
            }
            Ok::<_, String>(())
        })?;
        if fresh_regulator
            && let Some(discovery) = self.discoveries.get_mut(&dimension)
        {
            discovery.retire_regulator(pos);
        }
        if let Some(next) = next {
            self.levels.insert(dimension, Arc::new(next));
        }
        Ok(())
    }
}

fn paper_cargo_filters(
    host: &dyn ServerHost,
    items: BTreeMap<u32, ItemStack>,
) -> Result<BTreeMap<usize, Compound>, String> {
    items
        .into_iter()
        .map(|(slot, item)| {
            if !FILTER_SLOTS.contains(&(slot as usize)) {
                return Err(format!("Unexpected Paper cargo inventory slot {slot}."));
            }
            let saved = item
                .to_nbt(host)
                .ok_or("Paper cargo filter item has no item registry id.")?;
            Ok((slot as usize, saved))
        })
        .collect()
}

pub(crate) fn restore_installed_menu(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Position,
    id: &str,
    component: Component,
    items: BTreeMap<u32, ItemStack>,
) -> Result<(), String> {
    match component {
        Component::ElectricMachine { .. }
        | Component::FuelGenerator { .. }
        | Component::ReactorPort => {
            let prepared = crate::factory::prepare_paper_inventory(host, level, pos, id, items)?;
            crate::factory::restore_paper_inventory(host, level, pos, id, prepared)
        }
        Component::Android => {
            crate::android_machine::restore_installed_inventory(host, level, pos, items)
        }
        Component::TrashCan => trash::restore_items(host, pos, items),
        Component::CargoInput | Component::CargoOutput { advanced: true } => {
            let filters = paper_cargo_filters(host, items)?;
            let mut data =
                block_data(host, level, pos).ok_or("Could not read the existing cargo node.")?;
            let cargo = data
                .cargo
                .as_mut()
                .ok_or("The existing cargo node has no saved settings.")?;
            if cargo.import_filters(filters)? {
                write_data(host, pos, data)
                    .map_err(|_| "Could not save the Paper cargo filters.")?;
            }
            Ok(())
        }
        _ => Err("This inventory-only Paper menu preset is unsupported.".into()),
    }
}

fn read_registration_marker(
    host: &mut dyn ServerHost,
    pos: Position,
    component: Component,
) -> Result<Option<BlockData>, &'static str> {
    if is_plain_node(component) {
        plain_network::read_persisted_checked(host, pos)
    } else {
        NETWORK_STATE.get_existing(host, Holder::BlockEntity(pos))
    }
    .map_err(|_| "The existing Slimefun network marker could not be read.")
}

fn merge_import(
    old: Option<BlockData>,
    paper: BlockData,
    node: &Node,
    import: bool,
) -> Result<Option<BlockData>, &'static str> {
    let Some(mut old) = old else {
        return Ok(Some(paper));
    };
    if !import {
        return Ok(None);
    }
    let restore_owner = old.gps_owner.is_none() && paper.gps_owner.is_some();
    if restore_owner {
        old.gps_owner = paper.gps_owner;
        if old.elevator_floor.is_none() {
            old.elevator_floor = paper.elevator_floor;
        }
    }
    let restore_cargo = if let Some(paper) = paper.cargo {
        old.cargo
            .get_or_insert_with(|| CargoSettings::legacy(node.channel, node.passes_unfiltered))
            .merge_paper(paper)?
    } else {
        false
    };
    Ok((restore_owner || restore_cargo).then_some(old))
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
    if matches!(component, Component::GpsTransmitter { .. })
        || crate::gps_transport::needs_owner(id)
    {
        data.gps_owner = field("owner")
            .map(|owner| parse_uuid(owner).ok_or("Invalid Paper GPS device owner UUID."))
            .transpose()?;
    }
    if id == crate::elevator::ELEVATOR_PLATE.id() {
        data.elevator_floor = Some(match metadata.get("floor") {
            None => crate::elevator::DEFAULT_FLOOR.to_owned(),
            Some(toml::Value::String(floor)) => floor.clone(),
            Some(_) => return Err("Invalid Paper elevator floor name."),
        });
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
            .filter(|channel| (0..=16).contains(channel))
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
    if crate::rainbow::placed::protected(level, dimension, pos) {
        return true;
    }
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

pub(crate) fn android_at(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Position,
) -> Option<crate::android_program::Mode> {
    let data = block_data(host, level, pos)?;
    let mode = crate::android_program::mode(&data.id)?;
    (level.block(host, pos)?.id(host) == crate::android_machine::DEVICE).then_some(mode)
}

pub(crate) fn android_interface_at(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Position,
    id: &str,
) -> bool {
    level
        .block(host, pos)
        .is_some_and(|block| block.id(host) == "minecraft:dispenser")
        && block_data(host, level, pos).is_some_and(|data| data.id == id)
}

pub(crate) fn saved_android_interface(
    host: &dyn mod_api::Host,
    level: LevelRef,
    pos: Position,
) -> bool {
    host.block_entity(level, pos)
        .and_then(|entity| {
            host.block_entity_saved_attachment(entity, NETWORK_STATE.namespace, NETWORK_STATE.name)
        })
        .as_ref()
        .and_then(saved_id)
        .is_some_and(|id| {
            matches!(
                id.as_str(),
                "ANDROID_INTERFACE_ITEMS" | "ANDROID_INTERFACE_FUEL"
            )
        })
}

pub(crate) fn plate_marker(
    level: &dyn LevelHost,
    dimension: &mod_api::LevelDimension,
    pos: Position,
) -> Option<String> {
    plain_network::saved_at(level, dimension, pos)
}

fn elevator_data(host: &mut dyn ServerHost, level: LevelRef, pos: Position) -> Option<BlockData> {
    let plate = crate::elevator::ELEVATOR_PLATE;
    if level.block(host, pos)?.id(host) != plate.material() {
        return None;
    }
    let data = block_data(host, level, pos)?;
    (data.id == plate.id()).then_some(data)
}

pub(crate) fn elevator_floor(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Position,
) -> Option<(u128, String)> {
    let data = elevator_data(host, level, pos)?;
    Some((
        data.gps_owner?,
        data.elevator_floor
            .unwrap_or_else(|| crate::elevator::DEFAULT_FLOOR.to_owned()),
    ))
}

pub(crate) fn rename_elevator(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Position,
    owner: u128,
    name: &str,
) -> bool {
    let Some(mut data) = elevator_data(host, level, pos) else {
        return false;
    };
    data.rename_floor(owner, name) && write_data(host, pos, data).is_ok()
}

pub(crate) fn gps_device_at(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Position,
) -> Option<(crate::items::Kind, Option<u128>)> {
    let data = block_data(host, level, pos)?;
    let kind = crate::gps_transport::fixed(&data.id)?;
    let carrier = level.block(host, pos)?.id(host);
    accepts_carrier(Component::GpsDevice(kind), &carrier).then_some((kind, data.gps_owner))
}

pub(crate) fn gps_panel_at(host: &mut dyn ServerHost, level: LevelRef, pos: Position) -> bool {
    block_data(host, level, pos).is_some_and(|data| data.id == "GPS_CONTROL_PANEL")
        && level
            .block(host, pos)
            .is_some_and(|block| accepts_carrier(Component::GpsControlPanel, &block.id(host)))
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

pub(crate) use network_visuals::compact as compact_energy;

pub(crate) fn energy_buffer(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Position,
) -> Option<(u32, u32)> {
    let state = block_data(host, level, pos)?;
    let capacity = match Component::for_item(&state.id)? {
        Component::Capacitor(capacity)
        | Component::ElectricMachine { capacity, .. }
        | Component::GpsTransmitter { capacity }
        | Component::FuelGenerator { capacity, .. } => capacity,
        _ => return None,
    };
    (capacity > 0).then_some((state.charge.min(capacity), capacity))
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

pub(crate) fn blast_resistant_at(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Position,
    direct_wither: bool,
) -> bool {
    let Some(material) = level.block(host, pos).map(|block| block.id(host)) else {
        return false;
    };
    if !matches!(
        material.as_str(),
        "minecraft:obsidian" | "minecraft:purple_stained_glass" | "minecraft:light_gray_stained_glass"
    ) {
        return false;
    }
    blast_resistant_marker(
        plain_network::read(host, level, pos),
        &material,
        direct_wither,
    )
}

fn blast_resistant_marker(data: Option<BlockData>, material: &str, direct_wither: bool) -> bool {
    let Some(Component::BlastResistant(kind)) = data.and_then(|data| Component::for_item(&data.id))
    else {
        return false;
    };
    material == kind.material()
        && (!direct_wither || kind != crate::network_recipes::HARDENED_GLASS)
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

pub(crate) fn has_block_marker(host: &mut dyn ServerHost, level: LevelRef, pos: Position) -> bool {
    crate::rainbow::placed::marked(host, level, pos)
        || block_data(host, level, pos).is_some()
        || host.block_entity(level, pos).is_some_and(|entity| {
            host.block_entity_saved_attachment(entity, NETWORK_STATE.namespace, NETWORK_STATE.name)
                .is_some()
                || crate::block_placer::marked(host, entity)
                || crate::output_chest::live_marker(host, entity)
                || crate::ancient_altar::marked(host, entity)
        })
        || crate::hologram_projector::marked(host, level, pos)
}

pub(crate) fn placed_id(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Position,
) -> Option<String> {
    block_data(host, level, pos).map(|data| data.id)
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
            | Component::DecorativeGold
            | Component::BlastResistant(_)
            | Component::GpsDevice(_)
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
    butchers: &crate::android_butchery::Butchers,
    profiler: &crate::timings::Profiler,
) {
    if !level.in_place(host) {
        return;
    }
    plain_network::clean_level(host, level);
    if !level
        .game_time(host)
        .is_some_and(|time| time.rem_euclid(i64::from(settings.block_ticker_delay)) == 0)
    {
        return;
    }
    let Some(dimension) = level.dimension().id(host) else {
        return;
    };
    let Ok(nodes) = networks.lock().unwrap().nodes(host, &dimension) else {
        return;
    };
    let mut profile = crate::timings::Cycle::new(&dimension);
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
    let mut labels = BTreeMap::new();
    for (&pos, &(node, _)) in &active {
        if !crate::availability::globally_enabled(&node.id) {
            continue;
        }
        if node.component == Component::TrashCan {
            profile.measure(host, &node.id, pos, |host| trash::clear(host, pos));
        }
        if node.component == Component::InfusedHopper {
            profile.measure(host, &node.id, pos, |host| {
                crate::infused::tick_hopper(host, level, pos, settings.infused);
            });
        }
        if node.component == Component::Android {
            profile.measure(host, &node.id, pos, |host| {
                if let Some(movement) = crate::android_machine::tick(
                    host,
                    level,
                    pos,
                    &node.id,
                    rolls,
                    settings.android_reduced_block_updates,
                    butchers,
                ) {
                    move_android(networks, host, level, &dimension, node, movement);
                }
            });
        }
    }
    let mut loaded_chunks = BTreeMap::new();
    let groups = networks
        .lock()
        .unwrap()
        .discoveries
        .entry(dimension.clone())
        .or_default()
        .discover(&active, settings.network_discovery_budget, |pos| {
            let column = [pos[0].div_euclid(16), pos[2].div_euclid(16)];
            *loaded_chunks
                .entry(column)
                .or_insert_with(|| level.is_loaded(host, pos))
        });
    for (start, family, group) in groups {
        if !active.contains_key(&start) {
            let conflict = match family {
                Network::Energy => Status::EnergyConflict,
                Network::Cargo => Status::CargoConflict,
                Network::Independent => continue,
            };
            labels.extend(group.into_iter().map(|pos| (pos, Some(conflict))));
            continue;
        }
        let (status, conflict) = match family {
            Network::Energy => {
                let started = host.server_monotonic_nanos();
                let mut generators = 0;
                let status = if group.iter().all(|pos| active[pos].0.regulator()) {
                    Some(Status::EnergyEmpty)
                } else {
                    tick_energy(
                        host,
                        level,
                        &dimension,
                        &active,
                        &group,
                        rolls,
                        settings.solar_night_in_other_dimensions,
                        &mut profile,
                        &mut generators,
                    )
                };
                profile.record_parent(host, &active[&start].0.id, start, started, generators);
                (status, Status::EnergyConflict)
            }
            Network::Cargo => {
                let terminals = group
                    .iter()
                    .copied()
                    .filter(|pos| !active[pos].0.regulator());
                let status = if terminals.clone().next().is_some() {
                    let due = networks
                        .lock()
                        .unwrap()
                        .discoveries
                        .get_mut(&dimension)
                        .is_some_and(|discovery| {
                            discovery.cargo_due(start, settings.cargo_tick_delay)
                        });
                    if due {
                        if settings.cargo_visualizer
                            && block_data(host, level, start)
                                .is_some_and(|data| !data.cargo_visualizer_disabled)
                        {
                            network_visuals::cargo_particles(host, level, terminals);
                        }
                        let started = host.server_monotonic_nanos();
                        let inputs = cargo_topology::tick(
                            networks,
                            host,
                            level,
                            &active,
                            &group,
                            &mut profile,
                        );
                        profile.record_parent(host, &active[&start].0.id, start, started, inputs);
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
    let mut gps_signals = BTreeMap::new();
    for (&pos, &(node, _)) in &active {
        if !crate::availability::globally_enabled(&node.id) {
            continue;
        }
        if let Component::GpsTransmitter { .. } = node.component
            && let Some(transmitter) = geology::transmitter(&node.id)
            && let Ok(signal) = profile.measure(host, &node.id, pos, |host| {
                host.transaction(|host, _| {
                    let mut data = block_data(host, level, pos).ok_or(())?;
                    let owner = data.gps_owner.ok_or(())?;
                    data.charge = data.charge.checked_sub(transmitter.cost).ok_or(())?;
                    write_data(host, pos, data).map_err(|_| ())?;
                    Ok::<_, ()>((owner, transmitter.strength(pos[1])))
                })
            }).0
        {
            gps_signals.insert(pos, signal);
        }
        if let Component::ElectricMachine { .. } = node.component {
            profile.measure(host, &node.id, pos, |host| {
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
                    return;
                }
                if let Some(cost) = crate::auto_crafter::operation_cost(&node.id) {
                    let crafted = host.transaction(|host, tx| {
                        let mut data = block_data(host, level, pos).ok_or(())?;
                        data.charge = data.charge.checked_sub(cost).ok_or(())?;
                        crate::auto_crafter::stage(host, tx, level, pos, &node.id)?;
                        write_data(host, pos, data).map_err(|_| ())
                    });
                    if crafted.is_ok() {
                        crate::auto_crafter::complete(host, level, pos);
                    }
                    return;
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
                    return;
                }
                if let Some(machine) = crop_growth::machine(&node.id) {
                    let growth = host.transaction(|host, tx| {
                        let mut data = block_data(host, level, pos).ok_or(())?;
                        data.charge = data.charge.checked_sub(machine.cost).ok_or(())?;
                        let growth = machine.stage(host, tx, level, pos)?;
                        write_data(host, pos, data).map_err(|_| ())?;
                        growth.apply(host, level)?;
                        Ok::<_, ()>(growth)
                    });
                    if let Ok(growth) = growth {
                        growth.particles(host, level);
                    }
                    return;
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
                    return;
                }
                let geo = node.id == geology::GEO_MINER.id();
                let previous_mining = if geo {
                    crate::factory::geo_mining_resource(host, level, pos)
                } else {
                    None
                };
                let _ = host.transaction(|host, tx| {
                    let mut data = block_data(host, level, pos).ok_or(())?;
                    if node.id == fluid_pump::FLUID_PUMP.id() {
                        data.charge = data.charge.checked_sub(fluid_pump::COST).ok_or(())?;
                        let drain = fluid_pump::stage(host, tx, level, pos, rolls)?;
                        write_data(host, pos, data).map_err(|_| ())?;
                        return drain.map_or(Ok(()), |drain| drain.apply(host, level));
                    }
                    if node.id == experience_collector::XP_COLLECTOR.id() {
                        (data.stored_experience, data.charge) = experience_collector::collect(
                            host,
                            tx,
                            level,
                            pos,
                            data.stored_experience,
                            data.charge,
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
                if geo {
                    // Paper keeps the Mining label through the completion tick.
                    // Read a newly started operation only after the outer transaction.
                    let operation = previous_mining
                        .or_else(|| crate::factory::geo_mining_resource(host, level, pos));
                    let status =
                        Status::geo_miner(operation, || geo_deposits::mining(host, level, pos).ok());
                    labels.insert(pos, Some(status));
                }
            });
        }
    }
    if nodes.values().any(|node| {
        node.regulator()
            || reactors::reactor(&node.id).is_some()
            || node.id == geology::GEO_MINER.id()
    }) {
        network_visuals::reconcile(host, level, &dimension, labels);
    }
    profiler.collect(profile);
    let mut networks = networks.lock().unwrap();
    if gps_signals.is_empty() {
        networks.gps_signals.remove(&dimension);
    } else {
        networks.gps_signals.insert(dimension, gps_signals);
    }
}

fn move_android(
    networks: &Mutex<Networks>,
    host: &mut dyn ServerHost,
    level: LevelRef,
    dimension: &str,
    node: &Node,
    movement: crate::android_machine::Movement,
) {
    let crate::android_machine::Movement { from, to, rotation } = movement;
    if !level
        .height(host)
        .is_some_and(|(min, height)| to[1] > min && to[1] < min.saturating_add(height))
        || !level.block(host, to).is_some_and(|block| {
            matches!(
                block.id(host).as_str(),
                "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
            )
        })
        || !crate::android_machine::inside_border(host, level, to)
    {
        return;
    }
    let Some(head) = host.block_entity(level, from) else {
        return;
    };
    let previous = {
        let mut networks = networks.lock().unwrap();
        let Ok(nodes) = networks.nodes(host, dimension) else {
            return;
        };
        if nodes.get(&from) != Some(node) {
            return;
        }
        let mut next = (*nodes).clone();
        let previous = next.insert(to, node.clone());
        // The head at either position stays discoverable if the world stops during the move.
        if save(host, dimension, &next).is_err() {
            return;
        }
        networks.levels.insert(dimension.to_owned(), Arc::new(next));
        previous
    };
    let moved = head
        .relocate_player_head(host, to, rotation)
        .ok()
        .flatten()
        .is_some();
    let mut networks = networks.lock().unwrap();
    let Ok(nodes) = networks.nodes(host, dimension) else {
        return;
    };
    let mut next = (*nodes).clone();
    if moved {
        next.remove(&from);
    } else if let Some(previous) = previous {
        next.insert(to, previous);
    } else {
        next.remove(&to);
    }
    if save(host, dimension, &next).is_ok() {
        networks.levels.insert(dimension.to_owned(), Arc::new(next));
    }
}

fn tick_energy(
    host: &mut dyn ServerHost,
    level: LevelRef,
    dimension: &str,
    active: &BTreeMap<Position, (&Node, u32)>,
    group: &[Position],
    rolls: &Rolls,
    solar_night_in_other_dimensions: bool,
    profile: &mut crate::timings::Cycle,
    generator_elapsed: &mut u64,
) -> Option<Status> {
    let time = level.day_time(host).map(|time| time.rem_euclid(24_000));
    let day = time.is_some_and(|time| !(12_300..=23_850).contains(&time))
        && level.is_raining(host) == Some(false)
        && level.is_thundering(host) == Some(false);
    let normal_environment = OnceCell::new();
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
                        let (production, elapsed) = profile.measure(host, &node.id, pos, |host| {
                            if let Some(spec) = reactors::reactor(&node.id) {
                                let focus = reactor_production(host, level, pos).ok_or(())?;
                                let outcome =
                                    reactors::poll(host, tx, level, pos, spec, charge, focus, rolls)?;
                                let (energy, melt) = (outcome.energy, outcome.melt);
                                if outcome.wither || melt {
                                    effects.push((pos, outcome));
                                }
                                return Ok((!melt).then_some(energy));
                            }
                            crate::factory::generate(host, tx, level, pos, &node.id, charge)
                                .map(Some)
                        });
                        *generator_elapsed = generator_elapsed.saturating_add(elapsed);
                        let Some(production) = production? else {
                            continue;
                        };
                        charge.min(capacity) + production
                    }
                    Component::SolarGenerator {
                        day: sun,
                        night: moon,
                    } => {
                        let (production, elapsed) = profile.measure(host, &node.id, pos, |host| {
                            solar_power(
                                *normal_environment.get_or_init(|| {
                                    solar_uses_daylight(dimension, || {
                                        level.dimension_type(host).map(|kind| kind.id)
                                    })
                                }),
                                day,
                                [sun, moon],
                                solar_night_in_other_dimensions,
                                || level.brightness(host, LightLayer::Sky, pos) == Some(15),
                            )
                        });
                        *generator_elapsed = generator_elapsed.saturating_add(elapsed);
                        production
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

fn solar_uses_daylight(level_id: &str, dimension_type: impl FnOnce() -> Option<String>) -> bool {
    match level_id {
        "minecraft:overworld" => true,
        "minecraft:the_nether" | "minecraft:the_end" => false,
        // Paper's WorldCreator can give a NORMAL world a separate key while using
        // the Overworld dimension type. Horizon retains that type on imported worlds.
        _ => dimension_type().as_deref() == Some("minecraft:overworld"),
    }
}

fn solar_power(
    normal_environment: bool,
    daytime: bool,
    [sun, moon]: [u32; 2],
    night_in_other_dimensions: bool,
    open_sky: impl FnOnce() -> bool,
) -> u32 {
    if !normal_environment {
        return if night_in_other_dimensions { moon } else { 0 };
    }
    let output = if daytime { sun } else { moon };
    if output != 0 && open_sky() { output } else { 0 }
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
    fn an_orphan_cargo_menu_recovers_filters_without_replacing_local_settings() {
        let mut host = FakeHost::new();
        let level = host.level.level();
        let pos = [7, 64, 8];
        let id = "CARGO_NODE_INPUT";
        let metadata: toml::Table = "
            owner = '27959afe-dde4-4b81-b630-87e7f43461a1'
            frequency = '16'
            filter-type = 'whitelist'
            filter-lore = 'true'
            round-robin = 'true'
            smart-fill = 'true'
        "
        .parse()
        .unwrap();
        host.level.block_entity(pos);
        let (node, marker) = imported(id, &metadata).unwrap();
        write_data(&mut host, pos, marker).unwrap();
        let component = node.component;
        let material = Identifier::parse("minecraft:stone").unwrap();
        host.registries.resolve_items([material.clone()]).unwrap();
        let stack =
            ItemStack::new((&host as &dyn ServerHost).item_key(&material).unwrap(), 1).unwrap();
        let saved = stack.to_nbt(&host).unwrap();
        let mut expected = block_data(&mut host, level, pos).unwrap().cargo.unwrap();
        assert!(
            restore_installed_menu(
                &mut host,
                level,
                pos,
                id,
                component,
                BTreeMap::from([(20, stack.clone()), (22, stack.clone())])
            )
            .is_err()
        );
        assert_eq!(
            block_data(&mut host, level, pos).unwrap().cargo,
            Some(expected.clone())
        );
        restore_installed_menu(
            &mut host,
            level,
            pos,
            id,
            component,
            BTreeMap::from([(20, stack)]),
        )
        .unwrap();
        expected.filters.insert(20, saved);
        expected.imported_paper = true;
        assert_eq!(
            block_data(&mut host, level, pos).unwrap().cargo,
            Some(expected)
        );
    }

    #[test]
    fn decorative_gold_restores_as_a_plain_block_and_cannot_join_energy_or_cargo() {
        let (gold, marker) = imported("GOLD_24K_BLOCK", &toml::Table::new()).unwrap();
        assert!(is_plain_node(gold.component));
        assert!(is_plain_carrier("minecraft:gold_block"));
        assert!(accepts_carrier(gold.component, "minecraft:gold_block"));
        assert!(!accepts_carrier(gold.component, "minecraft:raw_gold_block"));
        assert!(matches!(gold.family(), Network::Independent));
        assert!(!gold.relays());
        let codec = NETWORK_STATE.codec.as_ref().unwrap();
        let attachment = (codec.encode)(&marker).unwrap();
        assert_eq!(saved_id(&attachment).as_deref(), Some("GOLD_24K_BLOCK"));
        assert_eq!((codec.decode)(&attachment).unwrap().id, "GOLD_24K_BLOCK");
    }

    #[test]
    fn cargo_restore_only_fills_missing_state_and_does_not_regenerate_used_filters() {
        let source: toml::Table = "
            owner = '2728cc32-8191-44e7-86c9-2120aa3896c4'
            frequency = '16'
            filter-type = 'whitelist'
            filter-lore = 'true'
            round-robin = 'true'
        "
        .parse()
        .unwrap();
        let (node, mut archived) = imported("CARGO_NODE_INPUT", &source).unwrap();
        let empty = imported(&node.id, &toml::Table::new()).unwrap().1;
        let absent_fields = empty.cargo.as_ref().unwrap();
        assert!(absent_fields.blacklist);
        assert!(!absent_fields.lore);
        let source_options = archived.cargo.as_mut().unwrap();
        assert_eq!(source_options.channel, 16);
        assert!(!source_options.blacklist);
        assert!(source_options.lore);
        assert!(source_options.round_robin);
        source_options.imported_paper = true;
        let filter = Compound::from([("id".into(), Tag::String("minecraft:stone".into()))]);
        source_options.filters.insert(19, filter.clone());

        let mut edited = empty.clone();
        let local = edited.cargo.as_mut().unwrap();
        local.channel = 5;
        local.smart_fill = true;
        local.filters.insert(20, filter);
        assert!(merge_import(Some(edited.clone()), archived.clone(), &node, true).is_err());
        assert_eq!(edited.cargo.as_ref().unwrap().owner, None);
        edited.cargo.as_mut().unwrap().filters.clear();

        let mut restored = merge_import(Some(edited), archived.clone(), &node, true)
            .unwrap()
            .unwrap();
        let settings = restored.cargo.as_ref().unwrap();
        assert_eq!(settings.channel, 5);
        assert!(settings.blacklist && settings.smart_fill);
        assert!(!settings.lore && !settings.round_robin);
        assert_eq!(settings.owner, archived.cargo.as_ref().unwrap().owner);
        assert_eq!(settings.filters.keys().copied().collect::<Vec<_>>(), [19]);
        assert_eq!(CargoSettings::load(&settings.save()).unwrap(), *settings);
        restored.cargo.as_mut().unwrap().filters.clear();
        assert!(merge_import(Some(restored), archived, &node, true).unwrap().is_none());
        assert!(imported(&node.id, &"owner = 'broken'".parse::<toml::Table>().unwrap()).is_err());
    }

    #[test]
    fn restore_reports_a_damaged_network_attachment_before_touching_the_registration() {
        let mut host = FakeHost::new();
        let pos = [12, 66, -7];
        let damaged = Compound::from([(
            "slimefun".into(),
            Tag::compound([(
                "network_state",
                Tag::compound([(
                    "value",
                    Tag::compound([
                        ("id", Tag::String("CARGO_NODE_INPUT".into())),
                        ("charge", Tag::Int(0)),
                        ("cargo", Tag::compound([
                            ("channel", Tag::Int(5)),
                            ("filters", Tag::String("invalid layout".into())),
                        ])),
                    ]),
                )]),
            )]),
        )]);
        host.attachments = mod_api::Attachments::from_nbt(damaged.clone());
        let marker = read_registration_marker(&mut host, pos, Component::CargoInput);
        assert!(matches!(marker, Err("The existing Slimefun network marker could not be read.")));
        assert_eq!(host.attachments.to_nbt(), damaged);

        let damaged = Compound::from([(
            "slimefun".into(),
            Tag::compound([(
                "plain_energy_nodes",
                Tag::compound([(
                    "value",
                    Tag::compound([("12,66,-7", Tag::Int(17))]),
                )]),
            )]),
        )]);
        host.attachments = mod_api::Attachments::from_nbt(damaged.clone());
        let marker = read_registration_marker(&mut host, pos, Component::ManualComposter);
        assert!(marker.is_err());
        assert_eq!(host.attachments.to_nbt(), damaged);
    }

    #[test]
    fn paper_elevator_floor_and_owner_survive_restart_and_only_the_owner_can_rename() {
        use crate::elevator::{DEFAULT_FLOOR, ELEVATOR_PLATE};

        let id = ELEVATOR_PLATE.id();
        let owner = 0x2728_cc32_8191_44e7_86c9_2120_aa38_96c4;
        let paper: toml::Table =
            "owner = '2728cc32-8191-44e7-86c9-2120aa3896c4'\nfloor = '§b旧大厅'"
                .parse()
                .unwrap();
        let (node, data) = imported(id, &paper).unwrap();
        assert!(is_plain_node(node.component));
        assert!(accepts_carrier(node.component, "minecraft:stone_pressure_plate"));
        assert!(crate::gps_transport::needs_owner(id));
        assert_eq!(data.gps_owner, Some(owner));
        assert_eq!(data.elevator_floor.as_deref(), Some("§b旧大厅"));
        assert_eq!(
            imported(id, &toml::Table::new()).unwrap().1.elevator_floor.as_deref(),
            Some(DEFAULT_FLOOR)
        );
        assert!(imported(id, &"floor = 17".parse::<toml::Table>().unwrap()).is_err());

        let pos = [451, -23, -801];
        let mut host = FakeHost::new();
        write_data(&mut host, pos, data).unwrap();
        host.attachments = mod_api::Attachments::from_nbt(host.attachments.to_nbt());
        let mut loaded = plain_network::read_persisted(&mut host, pos).unwrap();
        assert_eq!(loaded.gps_owner, Some(owner));
        assert_eq!(loaded.elevator_floor.as_deref(), Some("§b旧大厅"));
        assert!(!loaded.rename_floor(owner + 1, "Foreign name"));
        assert!(loaded.rename_floor(owner, "&6大厅"));
        write_data(&mut host, pos, loaded).unwrap();
        let stored = plain_network::read_persisted(&mut host, pos).unwrap();
        assert_eq!(stored.elevator_floor.as_deref(), Some("&6大厅"));
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
    fn paper_blast_protection_uses_the_saved_identity_and_distinguishes_wither_attacks() {
        use crate::enchantment_machines::{WITHER_PROOF_GLASS, WITHER_PROOF_OBSIDIAN};
        use crate::network_recipes::HARDENED_GLASS;

        let mut host = FakeHost::new();
        for (x, (kind, wither_proof)) in [
            (HARDENED_GLASS, false),
            (WITHER_PROOF_GLASS, true),
            (WITHER_PROOF_OBSIDIAN, true),
        ]
        .into_iter()
        .enumerate()
        {
            let pos = [x as i32, 70, 0];
            assert!(!blast_resistant_marker(None, kind.material(), false));
            let (node, data) = imported(kind.id(), &toml::Table::new()).unwrap();
            assert!(is_plain_node(node.component));
            assert_eq!(network_item(kind.id()), Some(kind));
            write_data(&mut host, pos, data).unwrap();
            let mut marker = || plain_network::read_persisted(&mut host, pos);
            assert!(blast_resistant_marker(marker(), kind.material(), false));
            assert_eq!(
                blast_resistant_marker(marker(), kind.material(), true),
                wither_proof
            );
            assert!(!blast_resistant_marker(marker(), "minecraft:stone", false));
        }
    }

    #[test]
    fn each_cargo_regulator_skips_its_own_configured_number_of_ticks() {
        let manager = node("CARGO_MANAGER");
        let a = [0, 65, 0];
        let b = [30, 65, 0];
        let active = BTreeMap::from([(a, (&manager, 0)), (b, (&manager, 0))]);
        let mut discovery = Discovery::default();
        assert_eq!(discovery.discover(&active, 200, |_| true).len(), 2);
        assert!(!discovery.cargo_due(a, 2));
        assert!(!discovery.cargo_due(b, 2));
        assert!(!discovery.cargo_due(a, 2));
        assert!(discovery.cargo_due(a, 2));
        assert!(!discovery.cargo_due(b, 2));
        assert!(discovery.cargo_due(b, 0));
        assert!(!discovery.cargo_due(b, 2));
    }

    #[test]
    fn paper_transport_recovers_independent_matrix_and_personal_plate_owners() {
        let id = "aaaa0000-1111-2222-3333-444455556666";
        let metadata = toml::Table::from_iter([("owner".into(), toml::Value::String(id.into()))]);
        let mut host = FakeHost::new();
        for (paper_id, material, position) in [
            ("GPS_TELEPORTATION_MATRIX", "minecraft:iron_block", [-1, 65, 15]),
            (
                "GPS_ACTIVATION_DEVICE_PERSONAL",
                "minecraft:stone_pressure_plate",
                [0, 65, 16],
            ),
        ] {
            let (node, data) = imported(paper_id, &metadata).unwrap();
            assert!(matches!(node.family(), Network::Independent));
            assert!(is_plain_node(node.component));
            assert!(accepts_carrier(node.component, material));
            assert_eq!(data.gps_owner, parse_uuid(id));
            write_data(&mut host, position, data).unwrap();
        }
        let mut reopened = FakeHost::new();
        reopened.attachments = mod_api::Attachments::from_nbt(host.attachments.to_nbt());
        for position in [[-1, 65, 15], [0, 65, 16]] {
            let data = plain_network::read_persisted(&mut reopened, position).unwrap();
            assert_eq!(data.gps_owner, parse_uuid(id));
        }
        let (shared, data) = imported("GPS_ACTIVATION_DEVICE_SHARED", &toml::Table::new()).unwrap();
        assert!(is_plain_node(shared.component));
        assert!(accepts_carrier(
            shared.component,
            "minecraft:stone_pressure_plate"
        ));
        assert_eq!(data.gps_owner, None);
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
        let groups = Discovery::default().discover(&active, 200, |_| true);
        assert_eq!(groups.len(), 1);
        assert!(groups[0].1 == Network::Cargo);
        assert_eq!(
            groups[0].2,
            [[0, 70, 0], [6, 70, 0], [6, 70, 5], [6, 70, 6]]
        );
    }

    #[test]
    fn paper_network_discovery_counts_queued_air_and_never_caps_membership() {
        let regulator = node("ENERGY_REGULATOR");
        let connector = node("ENERGY_CONNECTOR");
        let capacitor = node("SMALL_CAPACITOR");
        let root = [0, 70, 0];
        let relay = [1, 70, 0];
        let target = [8, 70, 0];
        let mut active = BTreeMap::from([(root, (&regulator, 0)), (relay, (&connector, 0))]);
        active.insert(target, (&capacitor, 0));
        let mut discovery = Discovery::default();
        for _ in 0..7 {
            assert_eq!(discovery.discover(&active, 1, |_| true)[0].2, [root]);
        }
        assert_eq!(discovery.discover(&active, 1, |_| true)[0].2, [root, relay]);
        active.insert(target, (&capacitor, 17));
        for _ in 0..35 {
            assert_eq!(discovery.discover(&active, 1, |_| true)[0].2, [root, relay]);
        }
        assert_eq!(discovery.discover(&active, 1, |_| true)[0].2, [root, relay, target]);
        active.remove(&relay);
        assert_eq!(discovery.discover(&active, 200, |_| true)[0].2, [root]);
        active.insert(relay, (&connector, 0));
        assert_eq!(discovery.discover(&active, 200, |_| true)[0].2, [root, relay, target]);
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
    fn solar_night_option_is_independent_of_sky_light_in_the_other_dimensions() {
        let advanced = [128, 64];
        assert_eq!(solar_power(true, false, [32, 0], true, || panic!()), 0);
        for dimension in ["minecraft:the_nether", "minecraft:the_end", "import:custom"] {
            let normal = solar_uses_daylight(dimension, || Some("import:custom".into()));
            assert!(!normal);
            assert_eq!(solar_power(normal, true, advanced, false, || panic!()), 0);
            assert_eq!(solar_power(normal, true, advanced, true, || panic!()), 64);
            assert_eq!(solar_power(normal, false, advanced, true, || panic!()), 64);
            for day in [2, 8, 32] {
                assert_eq!(solar_power(normal, true, [day, 0], true, || panic!()), 0);
            }
        }
        assert_eq!(solar_power(true, true, advanced, true, || true), 128);
        assert_eq!(solar_power(true, false, advanced, false, || true), 64);
        assert_eq!(solar_power(true, false, advanced, true, || false), 0);
    }

    #[test]
    fn solar_power_in_a_named_overworld_type_uses_its_time_and_sky() {
        let normal = solar_uses_daylight("import:normal", || Some("minecraft:overworld".into()));
        assert!(normal);
        assert!(solar_uses_daylight("minecraft:overworld", || panic!()));
        assert!(!solar_uses_daylight("minecraft:the_nether", || panic!()));
        assert!(!solar_uses_daylight("minecraft:the_end", || panic!()));
        let nether = solar_uses_daylight("import:nether", || Some("minecraft:the_nether".into()));
        assert!(!nether);
        assert!(!solar_uses_daylight("import:unavailable", || None));

        for (sun, moon) in [(2, 0), (8, 0), (32, 0), (128, 64)] {
            for other_dimensions in [false, true] {
                assert_eq!(
                    solar_power(normal, true, [sun, moon], other_dimensions, || true),
                    sun
                );
                assert_eq!(
                    solar_power(normal, false, [sun, moon], other_dimensions, || true),
                    moon
                );
                assert_eq!(
                    solar_power(normal, true, [sun, moon], other_dimensions, || false),
                    0
                );
            }
        }
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
