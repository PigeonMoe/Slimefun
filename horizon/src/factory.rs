// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, sync::Mutex};

use mod_api::{
    AttachmentCodec, AttachmentType, BlockDropsEvent, BlockEntityRef, ComponentId, Compound,
    Holder, Host, HostTransaction, ITEMS, Identifier, ItemDrop, ItemPlace, ItemResource, ItemStack,
    LevelHost, LevelRef, MenuClosedEvent, MenuReplace, MenuSlotPlace, PlayerRef, ResourceHandler,
    ServerHost, Tag, Text, Transaction,
};

use crate::recipes::{
    BETTER_ELECTRIC_FURNACE_RESEARCH, BETTER_HEATED_PRESSURE_CHAMBER_RESEARCH, CRUSHING, DOUBLING,
    ELECTRIC_FURNACES_RESEARCH, ELECTRIC_ORE_GRINDING_RESEARCH, GRINDING,
    HEATED_PRESSURE_CHAMBER_RESEARCH, HIGH_TIER_ELECTRIC_INGOT_MACHINES_RESEARCH,
    MEDIUM_TIER_ELECTRIC_INGOT_MACHINES_RESEARCH, Research, SingleRecipe, Stack, WASHER_DUSTS,
    smelting,
};
use crate::{
    advanced_processing,
    electric_processing::{self, Family, MachineSpec},
    fuel_generators::{self, Generator},
    geo_deposits::{self, Mining, Resource},
    geology,
    gold_pan::PANS,
    items::{GOLD_PAN, Kind, PULVERIZED_ORE, PURE_ORE_CLUSTER, SALT, SIFTED_ORE},
    machine::{Settings, makeshift_recipe},
    network_runtime,
    random::Rolls,
    research,
};

#[path = "animal_machines.rs"]
pub(crate) mod animal_machines;
#[path = "assemblers.rs"]
pub(crate) mod assemblers;
#[path = "charging_bench.rs"]
pub(crate) mod charging_bench;
#[path = "crop_growth.rs"]
pub(crate) mod crop_growth;
#[path = "experience_collector.rs"]
pub(crate) mod experience_collector;
#[path = "fluid_pump.rs"]
pub(crate) mod fluid_pump;
#[path = "produce_collector.rs"]
pub(crate) mod produce_collector;
#[path = "reactors.rs"]
pub(crate) mod reactors;
#[path = "tree_growth.rs"]
pub(crate) mod tree_growth;

type Position = [i32; 3];
const SLOTS: [&str; 8] = ["19", "20", "24", "25", "10", "11", "28", "29"];
const STANDARD_SLOT_COUNT: usize = 4;
pub(crate) const SMELTERY_INPUT: [usize; 6] = [4, 5, 0, 1, 6, 7];
const INPUT: [usize; 2] = [0, 1];
const OUTPUT: [usize; 2] = [2, 3];
const PENDING: &str = "pending";
const REMAINING: &str = "remaining";
const TOTAL: &str = "total";
const RETURNS_BUCKET: &str = "returns_bucket";
const IMPORT: &str = "paper_inventory_import";
const MENU_SLOTS: [usize; 8] = [19, 20, 24, 25, 10, 11, 28, 29];
const BACKGROUND: &[usize] = &[
    0, 1, 2, 3, 4, 5, 6, 7, 8, 13, 31, 36, 37, 38, 39, 40, 41, 42, 43, 44,
];
const INPUT_BORDER: &[usize] = &[9, 10, 11, 12, 18, 21, 27, 28, 29, 30];
const OUTPUT_BORDER: &[usize] = &[14, 15, 16, 17, 23, 26, 32, 33, 34, 35];
const SMELTERY_BACKGROUND: &[usize] = &[4, 5, 6, 7, 8, 13, 31, 40, 41, 42, 43, 44];
const SMELTERY_INPUT_BORDER: &[usize] = &[0, 1, 2, 3, 9, 12, 18, 21, 27, 30, 36, 37, 38, 39];

static INVENTORY: AttachmentType<Compound> = AttachmentType {
    namespace: "slimefun",
    name: "electric_furnace",
    default: Compound::new,
    codec: Some(AttachmentCodec {
        encode: |value| Ok(value.clone()),
        decode: |value| Ok(value.clone()),
    }),
    copy_on_death: false,
    sync: false,
};

static PLAIN_INVENTORIES: AttachmentType<Compound> = AttachmentType {
    namespace: "slimefun",
    name: "machine_positions",
    default: Compound::new,
    codec: Some(AttachmentCodec {
        encode: |value| Ok(value.clone()),
        decode: |value| Ok(value.clone()),
    }),
    copy_on_death: false,
    sync: false,
};

#[derive(Clone, Copy)]
struct Storage {
    level: LevelRef,
    pos: Position,
    plain: bool,
}

fn position_key([x, y, z]: Position) -> String {
    format!("{x},{y},{z}")
}

fn chunk_pos([x, _, z]: Position) -> [i32; 2] {
    [x.div_euclid(16), z.div_euclid(16)]
}

#[derive(Clone, Copy)]
pub(crate) struct ElectricMachine {
    id: &'static str,
    carrier: &'static str,
    pub title: &'static str,
    pub cost: u32,
    pub work_ticks: u32,
    research: &'static Research,
    family: RecipeFamily,
    speed: u32,
}

#[derive(Clone, Copy)]
enum RecipeFamily {
    Furnace,
    OreGrinder,
    IngotPulverizer,
    HeatedPressure,
    Processing(&'static MachineSpec),
    Fuel(&'static Generator),
    Advanced(&'static advanced_processing::Machine),
    OilPump,
    GeoMiner,
    FluidPump,
    ExperienceCollector,
    ProduceCollector,
    FeedingMachine,
    Assembler(&'static assemblers::Assembler),
    Reactor(&'static reactors::Reactor),
    ReactorPort,
}

#[derive(Clone, Copy)]
struct Viewer {
    furnace: ElectricMachine,
    storage: Storage,
    entity: Option<BlockEntityRef>,
    window: i32,
    view: ViewState,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct ViewState {
    progress: Option<u32>,
    enchantment_warning: Option<i32>,
    production: bool,
    link: Option<Position>,
    assembler: Option<assemblers::Controls>,
}

impl ViewState {
    fn read(
        host: &mut dyn ServerHost,
        storage: Storage,
        machine: ElectricMachine,
        data: &Compound,
    ) -> Self {
        let mut view = Self {
            progress: remaining(data),
            enchantment_warning: None,
            production: false,
            link: None,
            assembler: matches!(machine.family, RecipeFamily::Assembler(_))
                .then(|| assemblers::Controls::read(data)),
        };
        if view.progress.is_none()
            && let Some(spec) = machine.enchanting()
        {
            let inputs = INPUT.map(|index| stack_in(host, data, index));
            view.enchantment_warning = crate::enchantment_machines::warning(host, spec, &inputs);
        }
        if machine.reactor_menu() {
            view.link = reactors::link(host, storage.level, storage.pos, machine);
            if let RecipeFamily::Reactor(spec) = machine.family {
                view.progress = reactors::progress(data, spec).map(|(progress, _)| progress);
                view.production =
                    network_runtime::reactor_production(host, storage.level, storage.pos)
                        .unwrap_or(false);
            }
        }
        view
    }

    fn items(
        self,
        host: &dyn Host,
        data: &Compound,
        machine: ElectricMachine,
    ) -> Vec<Option<ItemStack>> {
        let mut items = menu_items(host, data, machine);
        if let Some(limit) = self.enchantment_warning
            && let Some(item) = crate::enchantment_machines::warning_item(host, limit)
        {
            items[22] = Some(item);
        }
        if machine.reactor_menu() {
            reactors::decorate_controls(
                host,
                &mut items,
                machine,
                self.production,
                self.link.is_some(),
            );
        }
        items
    }
}

#[derive(Default)]
pub(crate) struct FactoryViewers(Mutex<BTreeMap<u128, Viewer>>);

pub(crate) static VIEWERS: FactoryViewers = FactoryViewers(Mutex::new(BTreeMap::new()));

impl FactoryViewers {
    pub fn clicked(&self, host: &mut dyn ServerHost, event: &mod_api::MenuClickedEvent) {
        if event.mode != 0
            || !matches!(event.button, 0 | 1)
            || !matches!(event.slot, 4 | 22 | 31 | 49)
        {
            return;
        }
        let viewer = self.0.lock().unwrap().get(&event.uuid).copied();
        let Some(viewer) = viewer.filter(|viewer| {
            viewer.window == event.window
                && (viewer.furnace.reactor_menu()
                    || matches!(viewer.furnace.family, RecipeFamily::Assembler(_)))
        }) else {
            return;
        };
        let Some(player) = host.player(event.uuid) else {
            return;
        };
        let Storage { level, pos, .. } = viewer.storage;
        if player.entity().level(host) != Some(level)
            || network_runtime::factory_identity(host, level, pos).as_deref()
                != Some(viewer.furnace.id)
            || host.block_entity(level, pos) != viewer.entity
        {
            host.close_menu(event.uuid);
            return;
        }
        if matches!(viewer.furnace.family, RecipeFamily::Assembler(_)) {
            let _ = host.transaction(|host, _| {
                let mut current = data_at(host, viewer.storage).ok_or(())?;
                let next = assemblers::Controls::read(&current)
                    .click(event.slot.into(), event.button.into())
                    .ok_or(())?;
                next.write(&mut current);
                save(host, viewer.storage, current).then_some(()).ok_or(())
            });
        } else if event.slot == 49 {
            if let Some(link) = reactors::link(host, level, pos, viewer.furnace) {
                open(host, player, level, link, None);
            }
        } else if event.slot == 4 && matches!(viewer.furnace.family, RecipeFamily::Reactor(_)) {
            network_runtime::toggle_reactor_focus(host, level, pos);
        }
    }

    pub fn closed(&self, event: &MenuClosedEvent) {
        let mut viewers = self.0.lock().unwrap();
        if viewers
            .get(&event.uuid)
            .is_some_and(|viewer| viewer.window == event.window)
        {
            viewers.remove(&event.uuid);
        }
    }

    pub fn clear(&self) {
        self.0.lock().unwrap().clear();
    }

    fn close_at(&self, host: &mut dyn ServerHost, level: LevelRef, pos: Position) {
        let viewers: Vec<_> = self
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, viewer)| viewer.storage.level == level && viewer.storage.pos == pos)
            .map(|(&uuid, viewer)| (uuid, viewer.window))
            .collect();
        for (uuid, window) in viewers {
            if host.menu(uuid, window).is_some() {
                host.close_menu(uuid);
            }
        }
    }

    fn has_viewer(
        &self,
        host: &mut dyn ServerHost,
        storage: Storage,
        machine: ElectricMachine,
    ) -> bool {
        let viewers: Vec<_> = self
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, viewer)| {
                viewer.storage.level == storage.level
                    && viewer.storage.pos == storage.pos
                    && viewer.furnace.id == machine.id
            })
            .map(|(&uuid, viewer)| (uuid, viewer.window, viewer.entity))
            .collect();
        viewers.into_iter().any(|(uuid, window, entity)| {
            host.block_entity(storage.level, storage.pos) == entity
                && host.menu(uuid, window).is_some()
        })
    }

    pub fn refresh(&self, host: &mut dyn ServerHost, level: LevelRef) {
        if !level.in_place(host) || !level.game_time(host).is_some_and(|tick| tick % 10 == 0) {
            return;
        }
        let viewers: Vec<_> = self
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, viewer)| viewer.storage.level == level)
            .map(|(&uuid, &viewer)| (uuid, viewer))
            .collect();
        for (uuid, viewer) in viewers {
            if host.menu(uuid, viewer.window).is_none() {
                self.0.lock().unwrap().remove(&uuid);
                continue;
            }
            let pos = viewer.storage.pos;
            if host.block_entity(level, pos) != viewer.entity
                || network_runtime::factory_identity(host, level, pos).as_deref()
                    != Some(viewer.furnace.id)
            {
                host.close_menu(uuid);
                continue;
            }
            let Some(data) = data_at(host, viewer.storage) else {
                continue;
            };
            let view = ViewState::read(host, viewer.storage, viewer.furnace, &data);
            if view == viewer.view {
                continue;
            }
            let items = view.items(host, &data, viewer.furnace);
            if host.menu_set_items(uuid, viewer.window, items).is_ok()
                && let Some(current) = self.0.lock().unwrap().get_mut(&uuid)
                && current.window == viewer.window
            {
                current.view = view;
            }
        }
    }
}

impl ElectricMachine {
    fn enchanting(self) -> Option<&'static advanced_processing::Machine> {
        match self.family {
            RecipeFamily::Advanced(spec)
                if matches!(
                    spec.family,
                    advanced_processing::Family::AutoEnchanter
                        | advanced_processing::Family::AutoDisenchanter
                ) =>
            {
                Some(spec)
            }
            _ => None,
        }
    }

    pub fn for_item(id: &str) -> Option<Self> {
        use RecipeFamily::{Furnace, HeatedPressure, IngotPulverizer, OreGrinder};
        if let Some(assembler) = assemblers::by_id(id) {
            return Some(Self {
                id: assembler.item.id(),
                carrier: assembler.item.material(),
                title: assembler.title,
                cost: assembler.cost,
                work_ticks: 0,
                research: assembler.research,
                family: RecipeFamily::Assembler(assembler),
                speed: 1,
            });
        }
        if let Some(machine) = animal_machines::machine(id) {
            return Some(Self {
                id: machine.item.id(),
                carrier: machine.item.material(),
                title: machine.title,
                cost: machine.cost,
                work_ticks: 0,
                research: machine.research,
                family: RecipeFamily::FeedingMachine,
                speed: 1,
            });
        }
        if let Some(machine) = crop_growth::machine(id) {
            return Some(Self {
                id: machine.item.id(),
                carrier: machine.item.material(),
                title: machine.title,
                cost: machine.cost,
                work_ticks: 0,
                research: machine.research,
                family: RecipeFamily::FeedingMachine,
                speed: 1,
            });
        }
        if id == tree_growth::TREE_GROWTH_ACCELERATOR.id() {
            return Some(Self {
                id: tree_growth::TREE_GROWTH_ACCELERATOR.id(),
                carrier: tree_growth::TREE_GROWTH_ACCELERATOR.material(),
                title: "Tree Growth Accelerator",
                cost: tree_growth::COST,
                work_ticks: 0,
                research: &tree_growth::RESEARCH,
                family: RecipeFamily::FeedingMachine,
                speed: 1,
            });
        }
        if id == produce_collector::PRODUCE_COLLECTOR.id() {
            return Some(Self {
                id: produce_collector::PRODUCE_COLLECTOR.id(),
                carrier: produce_collector::PRODUCE_COLLECTOR.material(),
                title: "Produce Collector",
                cost: produce_collector::COST,
                work_ticks: produce_collector::WORK_TICKS,
                research: &produce_collector::RESEARCH,
                family: RecipeFamily::ProduceCollector,
                speed: 1,
            });
        }
        if id == experience_collector::XP_COLLECTOR.id() {
            return Some(Self {
                id: experience_collector::XP_COLLECTOR.id(),
                carrier: experience_collector::XP_COLLECTOR.material(),
                title: "EXP Collector",
                cost: experience_collector::COST,
                work_ticks: 0,
                research: &experience_collector::RESEARCH,
                family: RecipeFamily::ExperienceCollector,
                speed: 1,
            });
        }
        if let Some(spec) = reactors::reactor(id) {
            return Some(Self {
                id: spec.item.id(),
                carrier: spec.item.material(),
                title: spec.title,
                cost: 0,
                work_ticks: 0,
                research: spec.research,
                family: RecipeFamily::Reactor(spec),
                speed: 1,
            });
        }
        if id == reactors::REACTOR_ACCESS_PORT.id() {
            return Some(Self {
                id: reactors::REACTOR_ACCESS_PORT.id(),
                carrier: reactors::REACTOR_ACCESS_PORT.material(),
                title: "Reactor Access Port",
                cost: 0,
                work_ticks: 0,
                research: &reactors::ACCESS_RESEARCH,
                family: RecipeFamily::ReactorPort,
                speed: 1,
            });
        }
        if let Some(spec) = fuel_generators::generator(id) {
            return Some(Self {
                id: spec.item.id(),
                carrier: spec.item.material(),
                title: spec.title,
                cost: 0,
                work_ticks: 0,
                research: spec.research,
                family: RecipeFamily::Fuel(spec),
                speed: 1,
            });
        }
        if id == "OIL_PUMP" {
            return Some(Self {
                id: geology::OIL_PUMP.id(),
                carrier: geology::OIL_PUMP.material(),
                title: "Oil Pump",
                cost: geology::PUMP_COST,
                work_ticks: geology::PUMP_WORK_TICKS,
                research: &geology::OIL,
                family: RecipeFamily::OilPump,
                speed: 1,
            });
        }
        if id == geology::GEO_MINER.id() {
            return Some(Self {
                id: geology::GEO_MINER.id(),
                carrier: geology::GEO_MINER.material(),
                title: "GEO Miner",
                cost: geology::MINER_COST,
                work_ticks: geology::MINER_WORK_TICKS,
                research: &geology::MINER,
                family: RecipeFamily::GeoMiner,
                speed: 1,
            });
        }
        if id == fluid_pump::FLUID_PUMP.id() {
            return Some(Self {
                id: fluid_pump::FLUID_PUMP.id(),
                carrier: fluid_pump::FLUID_PUMP.material(),
                title: "Fluid Pump",
                cost: fluid_pump::COST,
                work_ticks: 0,
                research: &fluid_pump::RESEARCH,
                family: RecipeFamily::FluidPump,
                speed: 1,
            });
        }
        if let Some(spec) = advanced_processing::machine(id) {
            return Some(Self {
                id: spec.item.id(),
                carrier: spec.item.material(),
                title: spec.title,
                cost: spec.cost,
                work_ticks: 0,
                research: spec.research,
                family: RecipeFamily::Advanced(spec),
                speed: spec.speed,
            });
        }
        if let Some(spec) = electric_processing::machine(id) {
            return Some(Self {
                id: spec.item.id(),
                carrier: spec.item.material(),
                title: spec.title,
                cost: spec.cost,
                work_ticks: if matches!(spec.family, Family::IngotFactory) {
                    16 / spec.speed
                } else {
                    0
                },
                research: spec.research,
                family: RecipeFamily::Processing(spec),
                speed: spec.speed,
            });
        }
        let item = crate::items::network_item(id)?;
        let id = item.id();
        let (title, cost, work_ticks, research, family) = match id {
            "ELECTRIC_FURNACE" => (
                "Electric Furnace",
                2,
                8,
                &ELECTRIC_FURNACES_RESEARCH,
                Furnace,
            ),
            "ELECTRIC_FURNACE_2" => (
                "Electric Furnace - II",
                3,
                4,
                &BETTER_ELECTRIC_FURNACE_RESEARCH,
                Furnace,
            ),
            "ELECTRIC_FURNACE_3" => (
                "Electric Furnace - III",
                5,
                2,
                &BETTER_ELECTRIC_FURNACE_RESEARCH,
                Furnace,
            ),
            "ELECTRIC_ORE_GRINDER" => (
                "Electric Ore Grinder",
                6,
                8,
                &ELECTRIC_ORE_GRINDING_RESEARCH,
                OreGrinder,
            ),
            "ELECTRIC_ORE_GRINDER_2" => (
                "Electric Ore Grinder (II)",
                15,
                2,
                &MEDIUM_TIER_ELECTRIC_INGOT_MACHINES_RESEARCH,
                OreGrinder,
            ),
            "ELECTRIC_ORE_GRINDER_3" => (
                "Electric Ore Grinder (III)",
                45,
                0,
                &HIGH_TIER_ELECTRIC_INGOT_MACHINES_RESEARCH,
                OreGrinder,
            ),
            "ELECTRIC_INGOT_PULVERIZER" => (
                "Electric Ingot Pulverizer",
                7,
                6,
                &ELECTRIC_ORE_GRINDING_RESEARCH,
                IngotPulverizer,
            ),
            "HEATED_PRESSURE_CHAMBER" => (
                "Heated Pressure Chamber",
                5,
                0,
                &HEATED_PRESSURE_CHAMBER_RESEARCH,
                HeatedPressure,
            ),
            "HEATED_PRESSURE_CHAMBER_2" => (
                "Heated Pressure Chamber - II",
                22,
                0,
                &BETTER_HEATED_PRESSURE_CHAMBER_RESEARCH,
                HeatedPressure,
            ),
            _ => return None,
        };
        // Paper's MachineRecipe doubles recipe seconds, then AContainer divides by speed.
        Some(Self {
            id,
            carrier: item.material(),
            title,
            cost,
            work_ticks,
            research,
            family,
            speed: if id == "HEATED_PRESSURE_CHAMBER_2" {
                5
            } else {
                1
            },
        })
    }

    fn accepts_carrier(self, block: &str) -> bool {
        block == self.carrier
            || (self.carrier == "minecraft:player_head" && block == "minecraft:player_wall_head")
    }

    pub(crate) fn is_smeltery(self) -> bool {
        matches!(self.family, RecipeFamily::Advanced(spec) if spec.family == advanced_processing::Family::Smeltery)
    }

    fn slot_count(self) -> usize {
        self.menu_slots().len()
    }

    fn reactor_menu(self) -> bool {
        matches!(
            self.family,
            RecipeFamily::Reactor(_) | RecipeFamily::ReactorPort
        )
    }

    fn menu_slots(self) -> &'static [usize] {
        if matches!(self.family, RecipeFamily::Assembler(_)) {
            return assemblers::MENU_SLOTS;
        }
        if matches!(self.family, RecipeFamily::ExperienceCollector) {
            return experience_collector::MENU_SLOTS;
        }
        if self.feeding_menu() {
            return tree_growth::MENU_SLOTS;
        }
        if self.reactor_menu() {
            return reactors::SLOTS;
        }
        if matches!(self.family, RecipeFamily::GeoMiner) {
            return geology::MINER_OUTPUT;
        }
        let count = if self.is_smeltery() {
            SLOTS.len()
        } else {
            STANDARD_SLOT_COUNT
        };
        &MENU_SLOTS[..count]
    }

    fn input_slot(self, index: usize) -> bool {
        if matches!(self.family, RecipeFamily::Assembler(_)) {
            return index < assemblers::MENU_SLOTS.len();
        }
        if matches!(
            self.family,
            RecipeFamily::ExperienceCollector | RecipeFamily::GeoMiner
        ) {
            return false;
        }
        if self.feeding_menu() {
            return index < tree_growth::MENU_SLOTS.len();
        }
        if self.reactor_menu() {
            return index < 6;
        }
        !OUTPUT.contains(&index)
    }

    fn feeding_menu(self) -> bool {
        matches!(self.family, RecipeFamily::FeedingMachine)
    }

    fn slot_keys(self) -> &'static [&'static str] {
        if matches!(self.family, RecipeFamily::GeoMiner) {
            geology::MINER_SLOT_KEYS
        } else {
            &SLOTS
        }
    }

    fn storage(self, level: LevelRef, pos: Position) -> Storage {
        Storage {
            level,
            pos,
            plain: network_runtime::is_plain_carrier(self.carrier),
        }
    }
}

pub(crate) fn register(registrar: &mut dyn mod_api::Registrar) {
    animal_machines::register(registrar);
    tree_growth::register(registrar);
    for material in [
        "minecraft:furnace",
        "minecraft:smoker",
        "minecraft:enchanting_table",
        "minecraft:player_head",
        "minecraft:player_wall_head",
    ]
    .into_iter()
    .chain(network_runtime::PLAIN_CARRIERS.iter().copied())
    {
        let block = registrar.block_key(material);
        registrar.block_capability_before_game(&ITEMS, &block, move |host, at, _, entity| {
            let id = network_runtime::factory_identity(host, at.level, at.pos)?;
            let machine = ElectricMachine::for_item(&id)
                .filter(|machine| machine.accepts_carrier(material))?;
            let storage = machine.storage(at.level, at.pos);
            if storage.plain == entity.is_some() {
                return None;
            }
            Some(Box::new(MachineInventory {
                storage,
                slot_keys: machine.slot_keys(),
                // An empty handler also refuses the backing vanilla slots for sided access.
                size: if at.side.is_none() {
                    machine.slot_count()
                } else {
                    0
                },
            }))
        });
    }
}

pub(crate) fn open(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    level: LevelRef,
    pos: Position,
    held: Option<&ItemStack>,
) -> bool {
    // Paper leaves the held item to vanilla when a player uses it while sneaking.
    if held.is_some() && player.entity().is_shift_key_down(host) == Some(true) {
        return false;
    }
    let Some(furnace) = network_runtime::factory_identity(host, level, pos)
        .and_then(|id| ElectricMachine::for_item(&id))
    else {
        return false;
    };
    if !research::can_use(host, player, furnace.research) {
        let message = format!(
            "Unlock this research with /sf research {}.",
            furnace.research.id
        );
        let _ = player.tell(host, Text::literal(message));
        return true;
    }
    if matches!(furnace.family, RecipeFamily::OilPump) {
        match geo_deposits::supply(host, level, pos, Resource::Oil) {
            Ok(Some(_)) => {}
            Ok(None) => {
                let _ = player.tell(
                    host,
                    Text::literal(
                        "Scan this chunk with a GPS Geo-Scanner before using the Oil Pump.",
                    ),
                );
                return true;
            }
            Err(message) => {
                let _ = player.tell(host, Text::literal(message));
                return true;
            }
        }
    }
    let storage = furnace.storage(level, pos);
    let Some(data) = data_at(host, storage) else {
        return true;
    };
    let Some(block) = host.block_key(furnace.carrier) else {
        return true;
    };
    let entity = host.block_entity(level, pos);
    let view = ViewState::read(host, storage, furnace, &data);
    let items = view.items(host, &data, furnace);
    let rows = (items.len() / 9) as u8;
    let title = Text::literal(furnace.title);
    let opened = match entity {
        Some(entity) if !storage.plain => host.open_block_inventory_menu(
            player,
            entity,
            rows,
            title,
            items,
            MenuReplace::CloseFirst,
        ),
        None if storage.plain => host.open_inventory_menu_at(
            player,
            level,
            pos,
            rows,
            title,
            items,
            MenuReplace::CloseFirst,
        ),
        _ => return true,
    };
    if let Ok(Some(window)) = opened {
        let mut backing = vec![MenuSlotPlace::Decoration; usize::from(rows) * 9];
        for (slot, &menu_slot) in furnace.menu_slots().iter().enumerate() {
            let allow_insert = furnace.input_slot(slot);
            backing[menu_slot] = match entity {
                Some(entity) => MenuSlotPlace::BlockEntityItems {
                    entity,
                    slot,
                    allow_insert,
                },
                None => MenuSlotPlace::BlockItems {
                    level,
                    pos,
                    expected_block: block,
                    slot,
                    allow_insert,
                },
            };
        }
        if player.back_mod_menu_slots(host, window, backing).is_err() {
            host.close_menu(player.uuid());
        } else {
            VIEWERS.0.lock().unwrap().insert(
                player.uuid(),
                Viewer {
                    furnace,
                    storage,
                    entity,
                    window,
                    view,
                },
            );
        }
    }
    true
}

fn remaining(data: &Compound) -> Option<u32> {
    match data.get(&REMAINING.into()) {
        Some(Tag::Int(value @ 0..)) => Some(*value as u32),
        _ => None,
    }
}

fn menu_items(
    host: &dyn Host,
    data: &Compound,
    furnace: ElectricMachine,
) -> Vec<Option<ItemStack>> {
    if furnace.reactor_menu() {
        return reactors::menu_items(host, data, furnace);
    }
    if let RecipeFamily::Assembler(spec) = furnace.family {
        return assemblers::menu(host, data, spec);
    }
    let small_menu_pane = match furnace.family {
        RecipeFamily::ExperienceCollector => Some("minecraft:purple_stained_glass_pane"),
        RecipeFamily::FeedingMachine => Some("minecraft:cyan_stained_glass_pane"),
        _ => None,
    };
    if let Some(pane) = small_menu_pane {
        let mut items = vec![display_item(host, pane, " "); 27];
        for (index, &slot) in furnace.menu_slots().iter().enumerate() {
            items[slot] = stack_in(host, data, index);
        }
        return items;
    }
    if matches!(furnace.family, RecipeFamily::GeoMiner) {
        let mut items = vec![None; 54];
        for (slots, material) in [
            (geology::MINER_BORDER, "minecraft:gray_stained_glass_pane"),
            (
                geology::MINER_OUTPUT_BORDER,
                "minecraft:orange_stained_glass_pane",
            ),
        ] {
            let pane = display_item(host, material, " ");
            for &slot in slots {
                items[slot] = pane.clone();
            }
        }
        for (&slot, &key) in geology::MINER_OUTPUT.iter().zip(geology::MINER_SLOT_KEYS) {
            items[slot] = read_slot(host, data, key);
        }
        items[4] = progress_item(host, data, furnace);
        return items;
    }
    let mut items = vec![None; 45];
    let (background, input_border) = if furnace.is_smeltery() {
        (SMELTERY_BACKGROUND, SMELTERY_INPUT_BORDER)
    } else {
        (BACKGROUND, INPUT_BORDER)
    };
    for (indices, material) in [
        (background, "minecraft:gray_stained_glass_pane"),
        (input_border, "minecraft:cyan_stained_glass_pane"),
        (OUTPUT_BORDER, "minecraft:orange_stained_glass_pane"),
    ] {
        let pane = display_item(host, material, " ");
        for &index in indices {
            items[index] = pane.clone();
        }
    }
    for (index, &menu_slot) in MENU_SLOTS[..furnace.slot_count()].iter().enumerate() {
        items[menu_slot] = stack_in(host, data, index);
    }
    if matches!(furnace.family, RecipeFamily::FluidPump) {
        items[22] = display_item(host, "minecraft:gray_stained_glass_pane", " ");
        return items;
    }
    items[22] = progress_item(host, data, furnace);
    items
}

fn progress_item(host: &dyn Host, data: &Compound, furnace: ElectricMachine) -> Option<ItemStack> {
    let Some(remaining) = remaining(data) else {
        return display_item(host, "minecraft:black_stained_glass_pane", " ");
    };
    let total = match data.get(&TOTAL.into()) {
        Some(Tag::Int(total @ 0..)) => *total as u32,
        _ => furnace.work_ticks,
    };
    let displayed_remaining = if matches!(furnace.family, RecipeFamily::GeoMiner) {
        remaining
    } else {
        remaining.saturating_add(1).min(total)
    };
    let icon = match furnace.family {
        RecipeFamily::Furnace | RecipeFamily::HeatedPressure => "minecraft:flint_and_steel",
        RecipeFamily::OreGrinder | RecipeFamily::IngotPulverizer => "minecraft:iron_pickaxe",
        RecipeFamily::Processing(spec) => match spec.family {
            Family::Press => "minecraft:iron_hoe",
            Family::GoldPan => "minecraft:diamond_shovel",
            Family::DustWasher => "minecraft:golden_shovel",
            Family::IngotFactory => "minecraft:flint_and_steel",
        },
        RecipeFamily::Fuel(spec) => spec.progress_icon(),
        RecipeFamily::ProduceCollector => "minecraft:shears",
        RecipeFamily::Advanced(spec) => match spec.family {
            advanced_processing::Family::CarbonPress => "minecraft:diamond_pickaxe",
            advanced_processing::Family::Freezer => "minecraft:golden_pickaxe",
            advanced_processing::Family::FoodFabricator
            | advanced_processing::Family::FoodComposter => "minecraft:golden_hoe",
            advanced_processing::Family::AutoBrewer => "minecraft:fishing_rod",
            advanced_processing::Family::AutoAnvil => "minecraft:iron_pickaxe",
            advanced_processing::Family::ChargingBench => "minecraft:golden_pickaxe",
            advanced_processing::Family::AutoEnchanter => "minecraft:golden_chestplate",
            advanced_processing::Family::AutoDisenchanter => "minecraft:diamond_chestplate",
            advanced_processing::Family::BookBinder => "minecraft:iron_chestplate",
            _ => "minecraft:flint_and_steel",
        },
        RecipeFamily::GeoMiner => "minecraft:diamond_pickaxe",
        RecipeFamily::OilPump | RecipeFamily::FluidPump => "minecraft:diamond_shovel",
        RecipeFamily::Reactor(_)
        | RecipeFamily::ReactorPort
        | RecipeFamily::Assembler(_)
        | RecipeFamily::FeedingMachine
        | RecipeFamily::ExperienceCollector => return None,
    };
    paper_progress_item(host, Kind::Vanilla(icon), displayed_remaining, total)
}

fn paper_progress_item(
    host: &dyn Host,
    icon: Kind,
    remaining: u32,
    total: u32,
) -> Option<ItemStack> {
    // Paper's MachineProcessor skips jobs with no duration and no remaining ticks.
    if total == 0 {
        return display_item(host, "minecraft:black_stained_glass_pane", " ");
    }
    let remaining = remaining.min(total);
    let mut item = icon.stack(host, 1)?;
    if let Some(durability) = host.max_damage(item.item()) {
        // Bukkit's ChestMenuUtils divides first, so short jobs use the native durability bar.
        item.set_damage((durability / total) * remaining);
    }
    let percentage = (((total - remaining) as f32 * 100.0) / total as f32).round() as u32;
    let filled = (percentage / 5) as usize;
    let color = match percentage {
        0..=15 => "dark_red",
        16..=31 => "red",
        32..=47 => "gold",
        48..=63 => "yellow",
        64..=79 => "dark_green",
        _ => "green",
    };
    let seconds = remaining / 2;
    let time = if seconds >= 60 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{seconds}s")
    };
    let mut segments = Vec::with_capacity(2);
    if filled > 0 {
        segments.push(crate::items::text(&":".repeat(filled), color, false));
    }
    segments.push(crate::items::text(
        &format!("{} - {percentage}.0%", ":".repeat(20 - filled)),
        "gray",
        false,
    ));
    let progress = Tag::compound([
        ("text", Tag::String("".into())),
        ("extra", Tag::List(segments)),
    ]);
    item.apply_components(&Compound::from([
        (
            "minecraft:custom_name".into(),
            Tag::compound([
                ("text", Tag::String("".into())),
                ("extra", Tag::List(vec![Tag::String(" ".into())])),
            ]),
        ),
        (
            "minecraft:lore".into(),
            Tag::List(vec![
                progress,
                Tag::String("".into()),
                crate::items::text(&time, "gray", false),
            ]),
        ),
        (
            "minecraft:tooltip_display".into(),
            Tag::compound([(
                "hidden_components",
                Tag::List(vec![Tag::String("minecraft:attribute_modifiers".into())]),
            )]),
        ),
    ]))
    .ok()?;
    Some(item)
}

fn display_item(host: &dyn Host, material: &str, name: &str) -> Option<ItemStack> {
    let key = host.item_key(&Identifier::parse(material).ok()?)?;
    let mut item = ItemStack::new(key, 1)?;
    item.set_components(Compound::from([(
        "minecraft:custom_name".into(),
        Tag::compound([("text", Tag::String(name.into())), ("italic", Tag::Byte(0))]),
    )]))
    .ok()?;
    Some(item)
}

pub(crate) fn restore_drops(level: &dyn LevelHost, event: &mut BlockDropsEvent<'_>) {
    let Some(entity) = event.block_entity else {
        return;
    };
    let Some(saved) =
        level.block_entity_saved_attachment(entity, INVENTORY.namespace, INVENTORY.name)
    else {
        return;
    };
    drop_slots(level, event, &saved);
}

pub(crate) fn restore_plain_drops(level: &dyn LevelHost, event: &mut BlockDropsEvent<'_>) {
    let pos = [event.pos.x, event.pos.y, event.pos.z];
    let Some(chunk) = level.chunk_saved_attachment(
        chunk_pos(pos),
        PLAIN_INVENTORIES.namespace,
        PLAIN_INVENTORIES.name,
    ) else {
        return;
    };
    if let Some(Tag::Compound(saved)) = chunk.get(&position_key(pos).as_str().into()) {
        drop_slots(level, event, saved);
    }
}

fn drop_slots(level: &dyn LevelHost, event: &mut BlockDropsEvent<'_>, saved: &Compound) {
    let position = [event.pos.x, event.pos.y, event.pos.z].map(|axis| f64::from(axis) + 0.5);
    for &slot in SLOTS.iter().chain(geology::MINER_SLOT_KEYS) {
        let Some(Tag::Compound(item)) = saved.get(&slot.into()) else {
            continue;
        };
        if let Some(stack) = network_runtime::read_stack(item, |id| level.item_key_by_name(id)) {
            event
                .drops
                .push(ItemDrop::new(stack, position, [0.0, 0.2, 0.0], 10));
        }
    }
}

fn data_at(host: &mut dyn ServerHost, storage: Storage) -> Option<Compound> {
    if !storage.level.in_place(host) {
        return None;
    }
    if storage.plain {
        let placements = PLAIN_INVENTORIES
            .get_existing(host, Holder::Chunk(chunk_pos(storage.pos)))
            .ok()?
            .unwrap_or_default();
        return match placements.get(&position_key(storage.pos).as_str().into()) {
            Some(Tag::Compound(data)) => Some(data.clone()),
            Some(_) => None,
            None => Some(Compound::new()),
        };
    }
    INVENTORY
        .get_existing(host, Holder::BlockEntity(storage.pos))
        .ok()
        .map(Option::unwrap_or_default)
}

fn save(host: &mut dyn ServerHost, storage: Storage, data: Compound) -> bool {
    if !storage.level.in_place(host) {
        return false;
    }
    if storage.plain {
        let holder = Holder::Chunk(chunk_pos(storage.pos));
        let Ok(placements) = PLAIN_INVENTORIES.get_existing(host, holder) else {
            return false;
        };
        let mut placements = placements.unwrap_or_default();
        let key = position_key(storage.pos);
        if data.is_empty() {
            if placements.remove(&key.as_str().into()).is_none() {
                return true;
            }
        } else {
            placements.insert(key.as_str().into(), Tag::Compound(data));
        }
        return if placements.is_empty() {
            PLAIN_INVENTORIES.remove(host, holder).is_ok()
        } else {
            PLAIN_INVENTORIES.set(host, holder, placements).is_ok()
        };
    }
    INVENTORY
        .set(host, Holder::BlockEntity(storage.pos), data)
        .is_ok()
}

pub(crate) fn clear_plain_data(host: &mut dyn ServerHost, level: LevelRef, pos: Position) {
    VIEWERS.close_at(host, level, pos);
    let _ = save(
        host,
        Storage {
            level,
            pos,
            plain: true,
        },
        Compound::new(),
    );
}

fn stack_in(host: &dyn Host, data: &Compound, index: usize) -> Option<ItemStack> {
    read_slot(host, data, SLOTS.get(index)?)
}

fn read_slot(host: &dyn Host, data: &Compound, key: &str) -> Option<ItemStack> {
    let Tag::Compound(saved) = data.get(&key.into())? else {
        return None;
    };
    ItemStack::from_nbt(host, saved)
}

fn write_stack(
    host: &dyn Host,
    data: &mut Compound,
    index: usize,
    stack: Option<ItemStack>,
) -> bool {
    write_slot(host, data, SLOTS[index], stack)
}

fn write_slot(host: &dyn Host, data: &mut Compound, key: &str, stack: Option<ItemStack>) -> bool {
    let key = key.into();
    if let Some(stack) = stack {
        let Some(saved) = stack.to_nbt(host) else {
            return false;
        };
        data.insert(key, Tag::Compound(saved));
    } else {
        data.remove(&key);
    }
    true
}

pub(crate) fn max_stack(host: &dyn Host, resource: &ItemResource) -> u32 {
    let key = ComponentId::new("minecraft:max_stack_size").expect("a built-in component id");
    match resource.value(host, &key) {
        Some(Tag::Int(max @ 1..=99)) => max.min(64) as u32,
        _ => host.max_stack_size(resource.item()).unwrap_or(0).min(64),
    }
}

struct MachineInventory {
    storage: Storage,
    slot_keys: &'static [&'static str],
    size: usize,
}

impl ResourceHandler<ItemResource> for MachineInventory {
    fn size(&self, _: &mut dyn ServerHost) -> usize {
        self.size
    }

    fn resource(&self, host: &mut dyn ServerHost, index: usize) -> Option<ItemResource> {
        (index < self.size).then_some(())?;
        let data = data_at(host, self.storage)?;
        read_slot(host, &data, self.slot_keys[index]).map(|stack| ItemResource::of(&stack))
    }

    fn amount(&self, host: &mut dyn ServerHost, index: usize) -> u32 {
        if index >= self.size {
            return 0;
        }
        data_at(host, self.storage)
            .and_then(|data| read_slot(host, &data, self.slot_keys[index]))
            .map_or(0, |stack| stack.count())
    }

    fn capacity(&self, host: &mut dyn ServerHost, index: usize, resource: &ItemResource) -> u32 {
        if index < self.size {
            max_stack(host, resource)
        } else {
            0
        }
    }

    fn insert(
        &self,
        host: &mut dyn ServerHost,
        _: &Transaction,
        index: usize,
        resource: &ItemResource,
        amount: u32,
    ) -> u32 {
        let capacity = self.capacity(host, index, resource);
        if capacity == 0 {
            return 0;
        }
        let Some(mut data) = data_at(host, self.storage) else {
            return 0;
        };
        let held = match read_slot(host, &data, self.slot_keys[index]) {
            Some(stack) if resource.matches(&stack) => stack.count(),
            Some(_) => return 0,
            None => 0,
        };
        let accepted = amount.min(capacity.saturating_sub(held));
        if accepted == 0 {
            return 0;
        }
        let Some(stack) = resource.to_stack(held + accepted) else {
            return 0;
        };
        if write_slot(host, &mut data, self.slot_keys[index], Some(stack))
            && save(host, self.storage, data)
        {
            accepted
        } else {
            0
        }
    }

    fn extract(
        &self,
        host: &mut dyn ServerHost,
        _: &Transaction,
        index: usize,
        resource: &ItemResource,
        amount: u32,
    ) -> u32 {
        if index >= self.size {
            return 0;
        }
        let Some(mut data) = data_at(host, self.storage) else {
            return 0;
        };
        let Some(stack) =
            read_slot(host, &data, self.slot_keys[index]).filter(|stack| resource.matches(stack))
        else {
            return 0;
        };
        let extracted = amount.min(stack.count());
        if extracted > 0
            && write_slot(
                host,
                &mut data,
                self.slot_keys[index],
                stack.shrink(extracted),
            )
            && save(host, self.storage, data)
        {
            extracted
        } else {
            0
        }
    }
}

fn place(level: LevelRef, pos: Position, index: usize) -> ItemPlace {
    ItemPlace::Strict(Box::new(ItemPlace::BlockEntity {
        level,
        pos,
        side: None,
        slot: index,
    }))
}

fn fits(host: &dyn Host, data: &Compound, output: &ItemStack) -> bool {
    let resource = ItemResource::of(output);
    let limit = max_stack(host, &resource);
    OUTPUT
        .into_iter()
        .map(|index| match stack_in(host, data, index) {
            None => limit,
            Some(stack) if resource.matches(&stack) => limit.saturating_sub(stack.count()),
            Some(_) => 0,
        })
        .sum::<u32>()
        >= output.count()
}

fn fits_all(host: &dyn Host, data: &Compound, outputs: &[ItemStack]) -> bool {
    if let [output] = outputs {
        return fits(host, data, output);
    }
    let mut slots = OUTPUT.map(|index| stack_in(host, data, index));
    for output in outputs {
        let resource = ItemResource::of(output);
        let limit = max_stack(host, &resource);
        let mut left = output.count();
        for slot in &mut slots {
            let count = match slot.as_ref() {
                Some(current) if resource.matches(current) => current.count(),
                None => 0,
                _ => continue,
            };
            let amount = left.min(limit.saturating_sub(count));
            if amount == 0 {
                continue;
            }
            let mut placed = output.clone();
            if placed.set_count(count + amount).is_err() {
                return false;
            }
            *slot = Some(placed);
            left -= amount;
            if left == 0 {
                break;
            }
        }
        if left != 0 {
            return false;
        }
    }
    !outputs.is_empty()
}

struct RecipeRun {
    inputs: [Option<(ItemStack, u32)>; SLOTS.len()],
    outputs: Vec<ItemStack>,
    work_override: Option<u32>,
}

impl RecipeRun {
    fn single(index: usize, input: ItemStack, amount: u32, output: ItemStack) -> Self {
        let mut inputs = std::array::from_fn(|_| None);
        inputs[index] = Some((input, amount));
        Self {
            inputs,
            outputs: vec![output],
            work_override: None,
        }
    }

    fn quantity(&self) -> u32 {
        self.inputs.iter().flatten().map(|(_, amount)| amount).sum()
    }

    fn with_work_ticks(mut self, ticks: u32) -> Self {
        self.work_override = Some(ticks);
        self
    }
}

fn furnace_recipe(host: &dyn Host, inputs: &[Option<ItemStack>; 2]) -> Option<RecipeRun> {
    for recipe in host.recipes_of_type("minecraft:smelting").iter() {
        let [ingredient] = recipe.ingredients.as_slice() else {
            continue;
        };
        // Paper subscribes to FurnaceRecipe and accepts enumerable MaterialChoice ingredients.
        let (Ok(keys), Some(output)) = (ingredient.item_keys(host), recipe.results.first()) else {
            continue;
        };
        if let Some((index, input)) = INPUT.into_iter().zip(inputs).find_map(|(index, input)| {
            let input = input.as_ref()?;
            (input.components().is_none()
                && input.damage_component().is_none()
                && keys.contains(&input.item()))
            .then_some((index, input))
        }) {
            return Some(RecipeRun::single(index, input.clone(), 1, output.clone()));
        }
    }
    None
}

fn grinder_recipes(settings: Settings) -> impl Iterator<Item = SingleRecipe> {
    GRINDING
        .iter()
        .chain(CRUSHING)
        .copied()
        .chain(DOUBLING.iter().map(move |recipe| SingleRecipe {
            input: recipe.input,
            output: Stack {
                count: recipe.output.count * (1 + u32::from(settings.double_ores)),
                ..recipe.output
            },
        }))
}

fn dust_to_ingot_recipes() -> impl Iterator<Item = SingleRecipe> {
    smelting()
        .filter(|recipe| makeshift_recipe(recipe))
        .map(|recipe| SingleRecipe {
            input: recipe.inputs[0],
            output: recipe.output,
        })
}

fn single_recipe(
    host: &dyn Host,
    inputs: &[Option<ItemStack>; 2],
    recipes: impl Iterator<Item = SingleRecipe>,
    priority_by_amount: bool,
) -> Option<RecipeRun> {
    let mut candidates = recipes.filter_map(|recipe| {
        let (index, input) = INPUT.into_iter().zip(inputs).find_map(|(index, stack)| {
            let stack = stack.as_ref()?;
            (stack.count() >= recipe.input.count && recipe.input.kind.matches(host, stack))
                .then_some((index, stack))
        })?;
        Some(RecipeRun::single(
            index,
            input.clone(),
            recipe.input.count,
            recipe.output.kind.stack(host, recipe.output.count)?,
        ))
    });
    if priority_by_amount {
        candidates.reduce(|first, next| {
            if next.quantity() > first.quantity() {
                next
            } else {
                first
            }
        })
    } else {
        candidates.next()
    }
}

fn heated_recipe(
    host: &dyn Host,
    inputs: &[Option<ItemStack>; 2],
    speed: u32,
) -> Option<RecipeRun> {
    crate::heated_pressure::RECIPES.iter().find_map(|recipe| {
        let mut consumed = std::array::from_fn(|_| None);
        for ingredient in recipe.inputs {
            let (slot, item) = INPUT.into_iter().zip(inputs).find_map(|(slot, item)| {
                let item = item.as_ref()?;
                (consumed[slot].is_none()
                    && item.count() >= ingredient.count
                    && ingredient.kind.matches(host, item))
                .then_some((slot, item))
            })?;
            consumed[slot] = Some((item.clone(), ingredient.count));
        }
        Some(RecipeRun {
            inputs: consumed,
            outputs: vec![recipe.output.kind.stack(host, recipe.output.count)?],
            work_override: Some(recipe.seconds * 2 / speed),
        })
    })
}

fn processing_recipe(
    host: &dyn Host,
    data: &Compound,
    inputs: &[Option<ItemStack>; 2],
    spec: &MachineSpec,
    settings: Settings,
    mut roll: impl FnMut(u32) -> Option<u32>,
) -> Option<RecipeRun> {
    match spec.family {
        Family::Press => electric_processing::PRESSING.iter().find_map(|timed| {
            let planned = single_recipe(host, inputs, std::iter::once(timed.recipe), false)?;
            Some(planned.with_work_ticks(timed.seconds * 2 / spec.speed))
        }),
        Family::IngotFactory => single_recipe(host, inputs, dust_to_ingot_recipes(), false),
        Family::GoldPan => {
            if !settings.gold_pan_override_output_limit[spec.tier - 1] && !free_output(host, data) {
                return None;
            }
            INPUT.into_iter().zip(inputs).find_map(|(slot, input)| {
                let input = input.as_ref().filter(|item| {
                    item.components().is_none() && item.damage_component().is_none()
                })?;
                let material = input.item().id(host);
                let pan = PANS
                    .iter()
                    .find(|pan| pan.target_sound(&material).is_some())?;
                let output = pan.drop_for(roll(100)?)?.stack(host, 1)?;
                let seconds = if pan.item == GOLD_PAN { 3 } else { 4 };
                fits(host, data, &output).then(|| {
                    RecipeRun::single(slot, input.clone(), 1, output)
                        .with_work_ticks((seconds / spec.speed) * 2)
                })
            })
        }
        Family::DustWasher => {
            for (slot, input) in INPUT.into_iter().zip(inputs) {
                let Some(input) = input.as_ref() else {
                    continue;
                };
                let kind = if SIFTED_ORE.matches(host, input) {
                    if !settings.legacy_dust_washer && !free_output(host, data) {
                        return None;
                    }
                    *WASHER_DUSTS.get(roll(WASHER_DUSTS.len() as u32)? as usize)?
                } else if PULVERIZED_ORE.matches(host, input) {
                    PURE_ORE_CLUSTER
                } else if Kind::Vanilla("minecraft:sand").matches(host, input) {
                    SALT
                } else {
                    continue;
                };
                let Some(output) = kind.stack(host, 1) else {
                    continue;
                };
                if fits(host, data, &output) {
                    return Some(
                        RecipeRun::single(slot, input.clone(), 1, output)
                            .with_work_ticks((4 / spec.speed) * 2),
                    );
                }
            }
            None
        }
    }
}

fn free_output(host: &dyn Host, data: &Compound) -> bool {
    OUTPUT
        .into_iter()
        .any(|index| stack_in(host, data, index).is_none())
}

fn find_recipe(
    host: &dyn Host,
    data: &Compound,
    machine: ElectricMachine,
    settings: Settings,
    enchantment_viewed: bool,
    roll: impl FnMut(u32) -> Option<u32>,
) -> Option<RecipeRun> {
    if let RecipeFamily::Advanced(spec) = machine.family {
        let layout: &[usize] = if machine.is_smeltery() {
            &SMELTERY_INPUT
        } else {
            &INPUT
        };
        let stored: Vec<_> = layout
            .iter()
            .map(|&index| stack_in(host, data, index))
            .collect();
        let plan = advanced_processing::next(host, spec, &stored, settings, enchantment_viewed)?;
        if !fits_all(host, data, &plan.outputs) {
            return None;
        }
        let mut inputs = std::array::from_fn(|_| None);
        for (slot, stack, count) in plan.consumed {
            inputs[layout[slot]] = Some((stack, count));
        }
        return Some(RecipeRun {
            inputs,
            outputs: plan.outputs,
            work_override: Some(plan.work_ticks),
        });
    }
    let inputs = INPUT.map(|index| stack_in(host, data, index));
    if inputs.iter().all(Option::is_none) {
        return None;
    }
    let run = match machine.family {
        RecipeFamily::Furnace => furnace_recipe(host, &inputs),
        RecipeFamily::OreGrinder => single_recipe(
            host,
            &inputs,
            grinder_recipes(settings),
            !settings.legacy_ore_grinder,
        ),
        RecipeFamily::IngotPulverizer => single_recipe(
            host,
            &inputs,
            dust_to_ingot_recipes().map(|recipe| SingleRecipe {
                input: recipe.output,
                output: recipe.input,
            }),
            false,
        ),
        RecipeFamily::HeatedPressure => heated_recipe(host, &inputs, machine.speed),
        RecipeFamily::Processing(spec) => {
            processing_recipe(host, data, &inputs, spec, settings, roll)
        }
        RecipeFamily::OilPump => single_recipe(
            host,
            &inputs,
            std::iter::once(SingleRecipe {
                input: Stack {
                    kind: advanced_processing::EMPTY_BUCKET,
                    count: 1,
                },
                output: Stack {
                    kind: crate::heated_pressure::BUCKET_OF_OIL,
                    count: 1,
                },
            }),
            false,
        ),
        RecipeFamily::Fuel(_)
        | RecipeFamily::Advanced(_)
        | RecipeFamily::FluidPump
        | RecipeFamily::GeoMiner
        | RecipeFamily::ExperienceCollector
        | RecipeFamily::ProduceCollector
        | RecipeFamily::FeedingMachine
        | RecipeFamily::Assembler(_)
        | RecipeFamily::Reactor(_)
        | RecipeFamily::ReactorPort => None,
    }?;
    fits_all(host, data, &run.outputs).then_some(run)
}

fn encode_pending(host: &dyn Host, outputs: &[ItemStack]) -> Option<Tag> {
    if let [output] = outputs {
        return Some(Tag::Compound(output.to_nbt(host)?));
    }
    let stacks = outputs
        .iter()
        .map(|output| output.to_nbt(host).map(Tag::Compound))
        .collect::<Option<_>>()?;
    Some(Tag::List(stacks))
}

fn decode_pending(host: &dyn Host, saved: &Tag) -> Option<Vec<ItemStack>> {
    match saved {
        Tag::Compound(stack) => Some(vec![ItemStack::from_nbt(host, stack)?]),
        Tag::List(stacks) if (1..=OUTPUT.len()).contains(&stacks.len()) => stacks
            .iter()
            .map(|tag| match tag {
                Tag::Compound(stack) => ItemStack::from_nbt(host, stack),
                _ => None,
            })
            .collect(),
        _ => None,
    }
}

/// Runs one Paper machine tick, after power allocation. Caller journals the
/// returned charge with these state writes so a refused debit cannot cook for free.
pub(crate) fn tick(
    host: &mut dyn ServerHost,
    level: LevelRef,
    settings: Settings,
    rolls: &Rolls,
    pos: Position,
    id: &str,
    charge: u32,
) -> Option<u32> {
    let furnace = ElectricMachine::for_item(id)?;
    let storage = furnace.storage(level, pos);
    let data = data_at(host, storage)?;
    if matches!(furnace.family, RecipeFamily::Advanced(spec) if spec.family == advanced_processing::Family::ChargingBench)
    {
        return charge_items(host, storage, data, charge);
    }
    if let Some(saved) = data.get(&PENDING.into()) {
        let remaining = remaining(&data)?;
        let mining = matches!(furnace.family, RecipeFamily::GeoMiner);
        let next_charge = pending_charge(furnace, remaining, charge)?;
        if remaining > 0 {
            let mut next = data;
            next.insert(REMAINING.into(), Tag::Int(remaining as i32 - 1));
            return save(host, storage, next).then_some(next_charge);
        }
        let indices = if mining {
            0..geology::MINER_OUTPUT.len()
        } else {
            OUTPUT[0]..OUTPUT[1] + 1
        };
        let outputs = decode_pending(host, saved)?;
        return host
            .transaction(|host, tx| {
                for output in outputs {
                    let resource = ItemResource::of(&output);
                    let mut left = output.count();
                    for index in indices.clone() {
                        left -= place(level, pos, index).insert(host, tx, &resource, left);
                        if left == 0 {
                            break;
                        }
                    }
                    if left != 0 {
                        return Err(());
                    }
                }
                let mut next = data_at(host, storage).ok_or(())?;
                next.remove(&PENDING.into());
                next.remove(&REMAINING.into());
                next.remove(&TOTAL.into());
                save(host, storage, next).then_some(next_charge).ok_or(())
            })
            .ok();
    }

    if matches!(furnace.family, RecipeFamily::GeoMiner) {
        let Mining::Available(resource) = geo_deposits::mining(host, level, pos).ok()? else {
            return None;
        };
        let output = resource.item().stack(host, 1)?;
        if !miner_fits(host, &data, &output) {
            return None;
        }
        let pending = encode_pending(host, &[output])?;
        let ticks = i32::try_from(furnace.work_ticks).ok()?;
        return host
            .transaction(|host, _| {
                geo_deposits::reserve_one(host, level, pos, resource)?;
                let mut next = data;
                set_pending(&mut next, pending, ticks);
                save(host, storage, next).then_some(charge).ok_or(())
            })
            .ok();
    }
    let run = if matches!(furnace.family, RecipeFamily::ProduceCollector) {
        find_produce(host, &data, |recipe| {
            recipe.has_animal(host, level, pos, settings.produce_collector_range)
        })
    } else {
        let viewed = furnace.enchanting().is_some() && VIEWERS.has_viewer(host, storage, furnace);
        find_recipe(host, &data, furnace, settings, viewed, |bound| {
            rolls.next(host, level, bound)
        })
    }?;
    let RecipeRun {
        inputs,
        outputs,
        work_override,
    } = run;
    let pumping_oil = matches!(furnace.family, RecipeFamily::OilPump);
    if pumping_oil
        && geo_deposits::supply(host, level, pos, Resource::Oil)
            .ok()?
            .unwrap_or(0)
            == 0
    {
        let (index, (bucket, _)) = inputs
            .iter()
            .enumerate()
            .find_map(|(index, input)| input.as_ref().map(|input| (index, input)))?;
        return move_input_to_output(host, level, pos, index, bucket).then_some(charge);
    }
    let saved_result = encode_pending(host, &outputs)?;
    let work_ticks = i32::try_from(work_override.unwrap_or(furnace.work_ticks)).ok()?;
    host.transaction(|host, tx| {
        if pumping_oil {
            geo_deposits::reserve_one(host, level, pos, Resource::Oil)?;
        }
        for (index, consumed) in inputs.into_iter().enumerate() {
            if let Some((input, amount)) = consumed
                && place(level, pos, index).extract(host, tx, &ItemResource::of(&input), amount)
                    != amount
            {
                return Err(());
            }
        }
        let mut next = data_at(host, storage).ok_or(())?;
        set_pending(&mut next, saved_result, work_ticks);
        save(host, storage, next).then_some(charge).ok_or(())
    })
    .ok()
}

fn pending_charge(machine: ElectricMachine, remaining: u32, charge: u32) -> Option<u32> {
    if remaining == 0 && matches!(machine.family, RecipeFamily::GeoMiner) {
        Some(charge)
    } else {
        charge.checked_sub(machine.cost)
    }
}

fn set_pending(data: &mut Compound, pending: Tag, ticks: i32) {
    data.insert(PENDING.into(), pending);
    data.insert(REMAINING.into(), Tag::Int(ticks));
    data.insert(TOTAL.into(), Tag::Int(ticks));
}

fn miner_fits(host: &dyn Host, data: &Compound, output: &ItemStack) -> bool {
    let resource = ItemResource::of(output);
    let limit = max_stack(host, &resource);
    geology::MINER_SLOT_KEYS
        .iter()
        .any(|key| match read_slot(host, data, key) {
            None => true,
            Some(stack) => resource.matches(&stack) && stack.count() < limit,
        })
}

fn find_produce(
    host: &dyn Host,
    data: &Compound,
    has_animal: impl Fn(produce_collector::Produce) -> bool,
) -> Option<RecipeRun> {
    for index in INPUT {
        let Some(input) = stack_in(host, data, index) else {
            continue;
        };
        for &recipe in produce_collector::RECIPES {
            if recipe.input.matches(host, &input)
                && let Some(output) = recipe.output.stack(host, 1)
                && fits(host, data, &output)
                && has_animal(recipe)
            {
                return Some(RecipeRun::single(index, input, 1, output));
            }
        }
    }
    None
}

fn charge_items(
    host: &mut dyn ServerHost,
    storage: Storage,
    mut data: Compound,
    charge: u32,
) -> Option<u32> {
    let next_charge = charge.checked_sub(charging_bench::COST)?;
    for slot in INPUT {
        let Some(stack) = stack_in(host, &data, slot) else {
            continue;
        };
        match charging_bench::action(host, &stack) {
            Some(charging_bench::Action::Charge(charged)) => {
                return (write_stack(host, &mut data, slot, Some(charged))
                    && save(host, storage, data))
                .then_some(next_charge);
            }
            Some(charging_bench::Action::ReturnFull) => {
                return move_input_to_output(host, storage.level, storage.pos, slot, &stack)
                    .then_some(charge);
            }
            Some(charging_bench::Action::ReturnOther) => {
                if move_input_to_output(host, storage.level, storage.pos, slot, &stack) {
                    data = data_at(host, storage)?;
                }
            }
            None => {}
        }
    }
    None
}

fn move_input_to_output(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Position,
    index: usize,
    stack: &ItemStack,
) -> bool {
    let resource = ItemResource::of(stack);
    host.transaction(|host, tx| {
        let mut left = stack.count();
        for output in OUTPUT {
            left -= place(level, pos, output).insert(host, tx, &resource, left);
            if left == 0 {
                break;
            }
        }
        // Preserve the input until the output can hold the entire transferred stack.
        if left == 0
            && place(level, pos, index).extract(host, tx, &resource, stack.count()) == stack.count()
        {
            Ok(())
        } else {
            Err(())
        }
    })
    .is_ok()
}

/// Advance one generator when its regulator polls it. The caller journals
/// both the fuel operation and delivery into the network buffers.
pub(crate) fn generate(
    host: &mut dyn ServerHost,
    tx: &Transaction,
    level: LevelRef,
    pos: Position,
    id: &str,
    charge: u32,
) -> Result<u32, ()> {
    let machine = ElectricMachine::for_item(id).ok_or(())?;
    let RecipeFamily::Fuel(generator) = machine.family else {
        return Err(());
    };
    let storage = machine.storage(level, pos);
    let mut data = data_at(host, storage).ok_or(())?;
    let mut produced = 0;
    match remaining(&data) {
        Some(0) => {
            if matches!(data.get(&RETURNS_BUCKET.into()), Some(Tag::Byte(1))) {
                let bucket = Kind::Vanilla("minecraft:bucket").stack(host, 1).ok_or(())?;
                let resource = ItemResource::of(&bucket);
                // Paper's generator advances even if the output inventory is full.
                for index in OUTPUT {
                    if place(level, pos, index).insert(host, tx, &resource, 1) == 1 {
                        break;
                    }
                }
                data = data_at(host, storage).ok_or(())?;
            }
            data.remove(&RETURNS_BUCKET.into());
            data.remove(&REMAINING.into());
            data.remove(&TOTAL.into());
        }
        Some(remaining) => {
            if generator.capacity.saturating_sub(charge) < generator.production {
                return Ok(0);
            }
            data.insert(REMAINING.into(), Tag::Int(remaining as i32 - 1));
            produced = generator.production;
        }
        None => {
            let Some((index, stack, fuel)) = INPUT.into_iter().find_map(|index| {
                let stack = stack_in(host, &data, index)?;
                let fuel = generator.fuel(host, &stack)?;
                Some((index, stack, fuel))
            }) else {
                return Ok(0);
            };
            if place(level, pos, index).extract(host, tx, &ItemResource::of(&stack), 1) != 1 {
                return Err(());
            }
            data = data_at(host, storage).ok_or(())?;
            let ticks = i32::try_from(fuel.ticks).map_err(|_| ())?;
            data.insert(REMAINING.into(), Tag::Int(ticks));
            data.insert(TOTAL.into(), Tag::Int(ticks));
            data.insert(
                RETURNS_BUCKET.into(),
                Tag::Byte(i8::from(fuel.returns_bucket)),
            );
        }
    }
    save(host, storage, data).then_some(produced).ok_or(())
}

pub(crate) fn prepare_paper_inventory(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Position,
    id: &str,
    items: BTreeMap<u32, ItemStack>,
) -> Result<Compound, String> {
    let machine = ElectricMachine::for_item(id)
        .filter(|_| level.in_place(host))
        .ok_or("The electric machine is not supported in this level.")?;
    let storage = machine.storage(level, pos);
    match network_runtime::factory_identity(host, level, pos) {
        Some(current) if current == id => {}
        Some(_) => return Err("A different electric machine is installed.".into()),
        None => {
            if !level
                .block_state(host, pos)
                .is_some_and(|state| machine.accepts_carrier(&state.block().id(host)))
            {
                return Err("Load the matching Paper machine block first.".into());
            }
            if storage.plain && host.block_entity(level, pos).is_some() {
                return Err(
                    "The machine location is occupied by a different block inventory.".into(),
                );
            }
            if !storage.plain && host.block_entity(level, pos).is_none() {
                return Err("The resident machine block entity is unavailable.".into());
            }
            if id == "REFINERY"
                && level.block_property(host, pos, "extended").as_deref() != Some("false")
            {
                return Err("Load the original, unextended Paper refinery block first.".into());
            }
            if matches!(machine.carrier, "minecraft:furnace" | "minecraft:smoker") {
                if level.block_property(host, pos, "lit").as_deref() != Some("false") {
                    return Err("Load the original, unlit Paper furnace block first.".into());
                }
                let handler = level
                    .capability(host, &ITEMS, pos, None)
                    .ok_or("The resident furnace inventory is unavailable.")?;
                if (0..handler.size(host)).any(|slot| handler.amount(host, slot) != 0) {
                    return Err("The resident vanilla furnace already contains items.".into());
                }
            }
        }
    }
    let mut restored = Compound::new();
    for (paper_slot, stack) in items {
        if machine.reactor_menu() && reactors::paper_control(host, machine, paper_slot, &stack) {
            continue;
        }
        let Some(index) = machine
            .menu_slots()
            .iter()
            .position(|&slot| slot as u32 == paper_slot)
        else {
            return Err(format!(
                "Unexpected electric machine inventory slot {paper_slot}."
            ));
        };
        if stack.count() > max_stack(host, &ItemResource::of(&stack))
            || !write_slot(host, &mut restored, machine.slot_keys()[index], Some(stack))
        {
            return Err(format!("Cannot restore the stack in slot {paper_slot}."));
        }
    }
    check_import(
        &data_at(host, storage).ok_or("Could not read the electric machine inventory.")?,
        &restored,
    )?;
    Ok(restored)
}

fn check_import(data: &Compound, restored: &Compound) -> Result<bool, String> {
    if let Some(previous) = data.get(&IMPORT.into()) {
        return if previous == &Tag::Compound(restored.clone()) {
            Ok(true)
        } else {
            Err("A different Paper inventory was restored here already.".into())
        };
    }
    if !assemblers::only_controls(data) {
        return Err(
            "The electric machine already holds local inventory or processing state.".into(),
        );
    }
    Ok(false)
}

pub(crate) fn restore_paper_assembler_controls(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Position,
    id: &str,
    metadata: &toml::Table,
) -> Result<(), String> {
    if assemblers::by_id(id).is_none()
        || !(metadata.contains_key("enabled") || metadata.contains_key("offset"))
    {
        return Ok(());
    }
    let controls = assemblers::Controls::from_paper(metadata)?;
    let storage = ElectricMachine::for_item(id)
        .ok_or("Unknown entity assembler.")?
        .storage(level, pos);
    let mut data = data_at(host, storage).ok_or("Could not read the entity assembler.")?;
    if !controls.restore(&mut data) || save(host, storage, data) {
        Ok(())
    } else {
        Err("Could not save the entity assembler controls.".into())
    }
}

pub(crate) fn restore_paper_inventory(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Position,
    id: &str,
    restored: Compound,
) -> Result<(), String> {
    if network_runtime::factory_identity(host, level, pos).as_deref() != Some(id) {
        return Err("A matching electric machine must be installed first.".into());
    }
    let storage = ElectricMachine::for_item(id)
        .ok_or("Unknown electric machine.")?
        .storage(level, pos);
    let mut data =
        data_at(host, storage).ok_or("Could not read the electric machine inventory.")?;
    if check_import(&data, &restored)? {
        return Ok(());
    }
    data.extend(restored.clone());
    data.insert(IMPORT.into(), Tag::Compound(restored));
    save(host, storage, data)
        .then_some(())
        .ok_or("Could not save the electric machine inventory.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{prelude::ItemsHost, testing::FakeHost};

    #[test]
    fn factory_slots_render_paper_progress_for_enchantment_machines_and_geo() {
        #[rustfmt::skip]
        let cases = [
            ("AUTO_ENCHANTER_2", 22, "minecraft:golden_chestplate", 112, 29, 50, 60, "gold", 8, "40.0%", "15s"),
            ("AUTO_DISENCHANTER_2", 22, "minecraft:diamond_chestplate", 528, 39, 60, 320, "gold", 6, "33.0%", "20s"),
            ("BOOK_BINDER", 22, "minecraft:iron_chestplate", 240, 19, 50, 80, "yellow", 12, "60.0%", "10s"),
            ("GEO_MINER", 4, "minecraft:diamond_pickaxe", 1561, 7, 14, 777, "yellow", 10, "50.0%", "3s"),
            ("ELECTRIC_FURNACE", 22, "minecraft:flint_and_steel", 64, 6, 8, 56, "dark_red", 2, "13.0%", "3s"),
        ];
        let mut host = recipe_host([
            "minecraft:black_stained_glass_pane",
            "minecraft:golden_chestplate",
            "minecraft:diamond_chestplate",
            "minecraft:iron_chestplate",
            "minecraft:diamond_pickaxe",
            "minecraft:flint_and_steel",
        ]);
        for (id, slot, icon, maximum, remaining, total, damage, color, filled, percent, time) in
            cases
        {
            host.registries
                .item_component(icon, "minecraft:max_damage", Tag::Int(maximum));
            let machine = ElectricMachine::for_item(id).unwrap();
            let idle = menu_items(&host, &Compound::new(), machine);
            assert_eq!(
                idle[slot].as_ref().unwrap().item(),
                host.item_key(&Identifier::parse("minecraft:black_stained_glass_pane").unwrap())
                    .unwrap()
            );
            let state = Compound::from([
                (REMAINING.into(), Tag::Int(remaining)),
                (TOTAL.into(), Tag::Int(total)),
            ]);
            let displayed = menu_items(&host, &state, machine);
            let item = displayed[slot].as_ref().unwrap();
            assert_eq!(
                item.item(),
                host.item_key(&Identifier::parse(icon).unwrap()).unwrap()
            );
            assert_eq!(item.damage_component(), Some(damage));
            let patch = item.components().unwrap().as_nbt();
            assert_eq!(
                patch
                    .get(&"minecraft:custom_name".into())
                    .and_then(|name| name.get("extra")),
                Some(&Tag::List(vec![Tag::String(" ".into())]))
            );
            assert_eq!(
                patch.get(&"minecraft:lore".into()),
                Some(&Tag::List(vec![
                    Tag::compound([
                        ("text", Tag::String("".into())),
                        (
                            "extra",
                            Tag::List(vec![
                                crate::items::text(&":".repeat(filled), color, false),
                                crate::items::text(
                                    &format!("{} - {percent}", ":".repeat(20 - filled)),
                                    "gray",
                                    false
                                ),
                            ])
                        ),
                    ]),
                    Tag::String("".into()),
                    crate::items::text(time, "gray", false),
                ]))
            );
            assert_eq!(
                patch
                    .get(&"minecraft:tooltip_display".into())
                    .and_then(|display| display.get("hidden_components")),
                Some(&Tag::List(vec![Tag::String(
                    "minecraft:attribute_modifiers".into()
                )])),
            );
        }
    }

    #[test]
    fn animal_machines_keep_the_paper_seven_input_layout_and_crafting() {
        use crate::{items, networks::Component, recipes};

        let host = recipe_host(["minecraft:cyan_stained_glass_pane"]);
        for (spec, cost, research, recipe) in [
            (
                "AUTO_BREEDER",
                60,
                25,
                [
                    Some("GOLD_18K"),
                    Some("CAN"),
                    Some("GOLD_18K"),
                    Some("ELECTRIC_MOTOR"),
                    Some("minecraft:hay_block"),
                    Some("ELECTRIC_MOTOR"),
                    Some("LEAD_INGOT"),
                    Some("FOOD_FABRICATOR"),
                    Some("LEAD_INGOT"),
                ],
            ),
            (
                "ANIMAL_GROWTH_ACCELERATOR",
                14,
                32,
                [
                    None,
                    Some("BLISTERING_INGOT_3"),
                    None,
                    Some("ELECTRIC_MOTOR"),
                    Some("ORGANIC_FOOD_WHEAT"),
                    Some("ELECTRIC_MOTOR"),
                    Some("REINFORCED_ALLOY_INGOT"),
                    Some("AUTO_BREEDER"),
                    Some("REINFORCED_ALLOY_INGOT"),
                ],
            ),
        ] {
            assert!(items::network_item(spec).is_some());
            assert!(matches!(
                Component::for_item(spec),
                Some(Component::ElectricMachine {
                    capacity: 1024,
                    carrier: "minecraft:hay_block"
                })
            ));
            let craft = recipes::crafting()
                .find(|recipe| recipe.output.kind.id() == spec)
                .unwrap();
            assert_eq!(craft.inputs.map(|kind| kind.map(Kind::id)), recipe);
            let machine = ElectricMachine::for_item(spec).unwrap();
            assert_eq!(machine.cost, cost);
            assert_eq!(machine.research.cost, research);
            assert_eq!(machine.menu_slots(), [10, 11, 12, 13, 14, 15, 16]);
            assert!((0..7).all(|slot| machine.input_slot(slot)));
            assert!(!machine.input_slot(7));
            let menu = menu_items(&host, &Compound::new(), machine);
            assert_eq!(menu.len(), 27);
            assert_eq!(
                menu.iter()
                    .enumerate()
                    .filter_map(|(slot, item)| item.is_none().then_some(slot))
                    .collect::<Vec<_>>(),
                machine.menu_slots(),
            );
        }
    }

    #[test]
    fn geo_miner_reads_every_output_without_exposing_an_input() {
        let host = recipe_host([
            "minecraft:sugar",
            "minecraft:stone",
            "minecraft:gray_stained_glass_pane",
            "minecraft:orange_stained_glass_pane",
            "minecraft:black_stained_glass_pane",
            "minecraft:diamond_pickaxe",
        ]);
        let machine = ElectricMachine::for_item("GEO_MINER").unwrap();
        let slots = [29, 30, 31, 32, 33, 38, 39, 40, 41, 42];
        assert_eq!(machine.menu_slots(), slots);
        assert!((0..slots.len()).all(|index| !machine.input_slot(index)));
        let salt = Resource::Salt.item().stack(&host, 1).unwrap();
        let stone = Kind::Vanilla("minecraft:stone").stack(&host, 64).unwrap();
        let mut data = Compound::new();
        for &key in &machine.slot_keys()[..slots.len() - 1] {
            assert!(write_slot(&host, &mut data, key, Some(stone.clone())));
        }
        assert!(miner_fits(&host, &data, &salt));
        let last = machine.slot_keys()[slots.len() - 1];
        assert!(write_slot(&host, &mut data, last, Some(stone)));
        assert!(!miner_fits(&host, &data, &salt));
        assert!(write_slot(&host, &mut data, last, Some(salt.clone())));
        assert!(miner_fits(&host, &data, &salt));
        let items = menu_items(&host, &data, machine);
        assert_eq!(items.len(), 54);
        assert_eq!(items[42], Some(salt));
        assert!(items[4].is_some());
        assert!(items.iter().all(Option::is_some));
    }

    #[test]
    fn geo_miner_only_debits_for_fourteen_progress_ticks() {
        use crate::networks::Component;

        let id = "GEO_MINER";
        let machine = ElectricMachine::for_item(id).unwrap();
        assert!(matches!(
            Component::for_item(id),
            Some(Component::ElectricMachine {
                capacity: 512,
                carrier: "minecraft:player_head"
            })
        ));
        assert_eq!(machine.work_ticks, 14);
        assert_eq!(machine.cost, 24);
        let mut charge = 512;
        for remaining in (1..=machine.work_ticks).rev() {
            assert_eq!(pending_charge(machine, remaining, 23), None);
            charge = pending_charge(machine, remaining, charge).unwrap();
        }
        assert_eq!(charge, 176);
        assert_eq!(pending_charge(machine, 0, 0), Some(0));
    }

    #[test]
    fn produce_collector_checks_for_animals_and_output_space_before_starting() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                ["bucket", "bowl", "milk_bucket", "mushroom_stew", "stone"]
                    .map(|id| Identifier::parse(&format!("minecraft:{id}")).unwrap()),
            )
            .unwrap();
        let mut data = Compound::new();
        for (slot, recipe) in INPUT.into_iter().zip(produce_collector::RECIPES) {
            assert!(write_stack(
                &host,
                &mut data,
                slot,
                recipe.input.stack(&host, 1)
            ));
        }
        let machine = ElectricMachine::for_item("PRODUCE_COLLECTOR").unwrap();
        assert_eq!(machine.menu_slots(), [19, 20, 24, 25]);
        assert_eq!((machine.cost, machine.work_ticks), (16, 10));
        let milk = Kind::Vanilla("minecraft:milk_bucket");
        let stew = Kind::Vanilla("minecraft:mushroom_stew");
        assert!(find_produce(&host, &data, |_| false).is_none());
        let run = find_produce(&host, &data, |_| true).unwrap();
        assert!(milk.matches(&host, &run.outputs[0]));
        let run = find_produce(&host, &data, |recipe| recipe.output == stew).unwrap();
        assert!(stew.matches(&host, &run.outputs[0]));
        assert!(run.inputs[0].is_none() && run.inputs[1].is_some());

        for slot in OUTPUT {
            assert!(write_stack(
                &host,
                &mut data,
                slot,
                Kind::Vanilla("minecraft:stone").stack(&host, 64),
            ));
        }
        assert!(find_produce(&host, &data, |_| true).is_none());
    }

    #[test]
    fn freezer_checks_both_outputs_and_reads_the_legacy_single_product_format() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                ["water_bucket", "bucket", "ice", "packed_ice", "stone"]
                    .map(|id| Identifier::parse(&format!("minecraft:{id}")).unwrap()),
            )
            .unwrap();
        let water = Kind::Vanilla("minecraft:water_bucket")
            .stack(&host, 1)
            .unwrap();
        let stone = Kind::Vanilla("minecraft:stone").stack(&host, 1).unwrap();
        let mut data = Compound::new();
        assert!(write_stack(&host, &mut data, INPUT[0], Some(water)));
        let freezer = ElectricMachine::for_item("FREEZER").unwrap();
        let run = find_recipe(&host, &data, freezer, Settings::default(), false, |_| None).unwrap();
        let [bucket, ice] = run.outputs.as_slice() else {
            panic!("Paper's freezer returns both its bucket and ice");
        };
        assert!(Kind::Vanilla("minecraft:bucket").matches(&host, bucket));
        assert!(Kind::Vanilla("minecraft:ice").matches(&host, ice));
        assert_eq!(run.work_override, Some(4));
        assert_eq!(
            decode_pending(&host, &encode_pending(&host, &run.outputs).unwrap()),
            Some(run.outputs.clone())
        );
        let saved = Tag::Compound(ice.to_nbt(&host).unwrap());
        assert_eq!(decode_pending(&host, &saved), Some(vec![ice.clone()]));
        assert!(write_stack(&host, &mut data, OUTPUT[1], Some(stone)));
        assert!(find_recipe(&host, &data, freezer, Settings::default(), false, |_| None).is_none());
        assert!(write_stack(&host, &mut data, OUTPUT[1], Some(ice.clone())));
        assert!(find_recipe(&host, &data, freezer, Settings::default(), false, |_| None).is_some());
    }

    #[test]
    fn carbon_press_preserves_recipe_priority_and_long_operations() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                ["coal_block", "diamond", "charcoal", "flint", "player_head"]
                    .map(|id| Identifier::parse(&format!("minecraft:{id}")).unwrap()),
            )
            .unwrap();
        let mut data = Compound::new();
        let coal = Kind::Vanilla("minecraft:coal_block")
            .stack(&host, 8)
            .unwrap();
        assert!(write_stack(&host, &mut data, INPUT[0], Some(coal)));
        let press = ElectricMachine::for_item("CARBON_PRESS").unwrap();
        let run = find_recipe(&host, &data, press, Settings::default(), false, |_| None).unwrap();
        assert_eq!(run.quantity(), 8);
        assert_eq!(run.outputs[0].count(), 9);
        assert_eq!(run.work_override, Some(360));
        data.insert(REMAINING.into(), Tag::Int(360));
        assert_eq!(remaining(&data), Some(360));

        let chunk = crate::items::CARBON_CHUNK.stack(&host, 1).unwrap();
        let diamond = crate::items::SYNTHETIC_DIAMOND.stack(&host, 1).unwrap();
        assert!(write_stack(&host, &mut data, INPUT[0], Some(chunk)));
        assert!(write_stack(&host, &mut data, INPUT[1], Some(diamond)));
        let press = ElectricMachine::for_item("CARBON_PRESS_3").unwrap();
        let run = find_recipe(&host, &data, press, Settings::default(), false, |_| None).unwrap();
        assert!(crate::items::RAW_CARBONADO.matches(&host, &run.outputs[0]));
        assert_eq!(run.work_override, Some(8));
    }

    #[test]
    fn a_fuel_generator_waits_for_space_without_advancing_its_burn() {
        let mut host = FakeHost::new();
        let level = host.level.level();
        let pos = [3, 64, 2];
        let id = "LAVA_GENERATOR";
        let storage = ElectricMachine::for_item(id).unwrap().storage(level, pos);
        host.level.block_entity(pos);
        let burn = Compound::from([
            (REMAINING.into(), Tag::Int(2)),
            (TOTAL.into(), Tag::Int(80)),
            (RETURNS_BUCKET.into(), Tag::Byte(1)),
        ]);
        assert!(save(&mut host, storage, burn.clone()));
        let produced = host
            .transaction(|host, tx| generate(host, tx, level, pos, id, 503))
            .unwrap();
        assert_eq!(produced, 0);
        assert_eq!(data_at(&mut host, storage), Some(burn));
        let produced = host
            .transaction(|host, tx| generate(host, tx, level, pos, id, 502))
            .unwrap();
        assert_eq!(produced, 10);
        assert_eq!(remaining(&data_at(&mut host, storage).unwrap()), Some(1));
    }

    #[test]
    fn paper_reactor_exports_recreate_controls_and_preserve_items_at_their_original_slots() {
        use crate::heated_pressure::{PLUTONIUM, URANIUM};
        use crate::hologram_projector::colored_lore;
        use crate::thermal_machines::REACTOR_COLLANT_CELL;

        let mut host = FakeHost::new();
        let material = [
            "minecraft:player_head",
            "minecraft:green_wool",
            "minecraft:red_wool",
        ];
        host.registries
            .resolve_items(material.map(|id| Identifier::parse(id).unwrap()))
            .unwrap();
        let level = host.level.level();
        let pos = [1, 64, 2];
        let id = reactors::NUCLEAR_REACTOR.id();
        host.level.block_entity(pos);
        host.attachments.read_saved_over(Compound::from([(
            "slimefun".into(),
            Tag::compound([(
                "network_state",
                Tag::compound([(
                    "value",
                    Tag::compound([("id", Tag::String(id.into())), ("charge", Tag::Int(4096))]),
                )]),
            )]),
        )]));

        let button = |host: &FakeHost, kind: Kind, title: &str, lore: &[&str]| {
            let mut item = kind.stack(host, 1).unwrap();
            item.apply_components(&Compound::from([
                ("minecraft:custom_name".into(), colored_lore(title)),
                (
                    "minecraft:lore".into(),
                    Tag::List(lore.iter().copied().map(colored_lore).collect()),
                ),
            ]))
            .unwrap();
            item
        };
        let mode_button = button(
            &host,
            PLUTONIUM,
            "&7Focus: &eProduction",
            &[
                "",
                "&6Your Reactor will focus on producing goods",
                "&6If your Energy Network doesn't need Power",
                "&6it will continue to run and simply will",
                "&6not generate any Power in the mean time",
                "",
                "&7⇨ Click to change the Focus to &ePower Generation",
            ],
        );
        let access = button(
            &host,
            Kind::Vanilla(material[1]),
            "&7Access Port",
            &["", "&6Detected", "", "&7> Click to view Access Port"],
        );
        let uranium = URANIUM.stack(&host, 2).unwrap();
        let coolant = REACTOR_COLLANT_CELL.stack(&host, 3).unwrap();
        let byproduct = PLUTONIUM.stack(&host, 4).unwrap();
        let raw = BTreeMap::from([
            (4, mode_button),
            (19, uranium.clone()),
            (43, coolant.clone()),
            (40, byproduct.clone()),
            (49, access.clone()),
        ]);
        let mut malformed = raw.clone();
        let mut marked_access = access;
        marked_access
            .apply_components(&Compound::from([(
                "minecraft:custom_data".into(),
                Tag::compound([("an_addon_value", Tag::Int(3))]),
            )]))
            .unwrap();
        malformed.insert(49, marked_access);
        let start = host.attachments.to_nbt();
        assert!(prepare_paper_inventory(&mut host, level, pos, id, malformed).is_err());
        assert_eq!(host.attachments.to_nbt(), start);

        let prepared = prepare_paper_inventory(&mut host, level, pos, id, raw.clone()).unwrap();
        assert_eq!(prepared.len(), 3);
        restore_paper_inventory(&mut host, level, pos, id, prepared.clone()).unwrap();
        let machine = ElectricMachine::for_item(id).unwrap();
        let storage = machine.storage(level, pos);
        let mut restored = data_at(&mut host, storage).unwrap();
        for (index, item) in [(0, uranium), (5, coolant), (6, byproduct)] {
            assert_eq!(stack_in(&host, &restored, index), Some(item));
        }
        assert_eq!(machine.menu_slots()[5..=6], [43, 40]);
        assert_eq!(
            crate::network_runtime::reactor_production(&mut host, level, pos),
            Some(false)
        );
        assert_eq!(
            crate::network_runtime::charge(&mut host, level, pos),
            Some(4096)
        );

        // A new host decodes the complete saved state. An idempotent re-import
        // recognizes the original archive without refilling a withdrawn item.
        restored.remove(&SLOTS[0].into());
        assert!(save(&mut host, storage, restored));
        let saved = host.attachments.to_nbt();
        let mut reopened = host.clone();
        reopened.attachments.read_saved_over(saved.clone());
        let retry = prepare_paper_inventory(&mut reopened, level, pos, id, raw).unwrap();
        restore_paper_inventory(&mut reopened, level, pos, id, retry).unwrap();
        assert_eq!(reopened.attachments.to_nbt(), saved);

        let port = ElectricMachine::for_item(reactors::REACTOR_ACCESS_PORT.id()).unwrap();
        let link = button(
            &reopened,
            Kind::Vanilla(material[2]),
            "&7Reactor",
            &[
                "",
                "&cNot detected",
                "",
                "&7Reactor must be",
                "&7placed 3 blocks below",
                "&7the access port!",
            ],
        );
        assert!(reactors::paper_control(&reopened, port, 49, &link));
        assert!(!reactors::paper_control(&reopened, port, 40, &link));
        assert!(!reactors::paper_control(&reopened, machine, 49, &link));
    }

    #[test]
    fn paper_restore_rolls_back_and_cannot_refill_a_consumed_slot() {
        let mut host = FakeHost::new();
        let level = host.level.level();
        let pos = [1, 64, 2];
        let id = "ELECTRIC_FURNACE";
        let storage = ElectricMachine::for_item(id).unwrap().storage(level, pos);
        host.level.block_entity(pos);
        host.attachments.read_saved_over(Compound::from([(
            "slimefun".into(),
            Tag::Compound(Compound::from([(
                "network_state".into(),
                Tag::Compound(Compound::from([(
                    "value".into(),
                    Tag::Compound(Compound::from([
                        ("id".into(), Tag::String(id.into())),
                        ("charge".into(), Tag::Int(20)),
                    ])),
                )])),
            )])),
        )]));
        let material = Identifier::parse("minecraft:iron_ingot").unwrap();
        host.registries.resolve_items([material.clone()]).unwrap();
        let item = (&host as &dyn Host).item_key(&material).unwrap();
        let ingot = ItemStack::new(item, 1).unwrap();
        let slots = BTreeMap::from([(24, ingot.clone())]);
        let mut invalid = slots.clone();
        invalid.insert(26, ingot);

        let original = host.attachments.to_nbt();
        assert!(prepare_paper_inventory(&mut host, level, pos, id, invalid).is_err());
        assert_eq!(host.attachments.to_nbt(), original);

        let prepared = prepare_paper_inventory(&mut host, level, pos, id, slots).unwrap();
        let failure = host.transaction(|host, _| {
            restore_paper_inventory(host, level, pos, id, prepared.clone()).unwrap();
            Err::<(), _>("abort after the Paper inventory was written")
        });
        assert!(failure.is_err());
        assert_eq!(host.attachments.to_nbt(), original);

        restore_paper_inventory(&mut host, level, pos, id, prepared.clone()).unwrap();
        let mut current = data_at(&mut host, storage).unwrap();
        assert!(current.remove(&"24".into()).is_some());
        assert!(save(&mut host, storage, current.clone()));

        restore_paper_inventory(&mut host, level, pos, id, prepared).unwrap();
        assert!(restore_paper_inventory(&mut host, level, pos, id, Compound::new()).is_err());
        assert_eq!(data_at(&mut host, storage), Some(current));
    }

    #[test]
    fn furnace_slots_preserve_input_and_check_both_output_stacks() {
        let mut host = FakeHost::new();
        let identifiers = [
            "minecraft:raw_iron",
            "minecraft:iron_ingot",
            "minecraft:brick",
        ]
        .map(|id| Identifier::parse(id).unwrap());
        host.registries.resolve_items(identifiers.clone()).unwrap();
        let lookup: &dyn Host = &host;
        let stack = |index: usize, count| {
            ItemStack::new(lookup.item_key(&identifiers[index]).unwrap(), count).unwrap()
        };
        let mut data = Compound::new();
        let result = stack(1, 2);
        assert!(write_stack(&host, &mut data, INPUT[0], Some(stack(0, 4))));
        assert!(write_stack(&host, &mut data, OUTPUT[0], Some(stack(1, 63))));
        assert!(write_stack(&host, &mut data, OUTPUT[1], Some(stack(2, 1))));
        assert!(!fits(&host, &data, &result));
        assert!(write_stack(&host, &mut data, OUTPUT[1], Some(stack(1, 63))));
        assert!(fits(&host, &data, &result));
        assert_eq!(stack_in(&host, &data, INPUT[0]), Some(stack(0, 4)));
        assert!(matches!(data.get(&"19".into()), Some(Tag::Compound(_))));
    }

    #[test]
    fn a_powered_furnace_charges_each_progress_tick_and_the_delivery_tick() {
        let actual: Vec<_> = [
            "ELECTRIC_FURNACE",
            "ELECTRIC_FURNACE_2",
            "ELECTRIC_FURNACE_3",
        ]
        .into_iter()
        .map(|id| {
            let furnace = ElectricMachine::for_item(id).unwrap();
            (
                furnace.work_ticks,
                furnace.cost,
                (furnace.work_ticks + 1) * furnace.cost,
            )
        })
        .collect();
        assert_eq!(actual, [(8, 2, 18), (4, 3, 15), (2, 5, 15)]);
    }

    #[test]
    fn grinder_recipe_priority_switches_between_default_and_amount_order() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                [
                    "minecraft:cobblestone",
                    "minecraft:gravel",
                    "minecraft:sand",
                ]
                .map(|id| Identifier::parse(id).unwrap()),
            )
            .unwrap();
        let cobble = crate::items::Kind::Vanilla("minecraft:cobblestone")
            .stack(&host, 8)
            .unwrap();
        let input = [Some(cobble), None];
        let settings = Settings::default();
        let legacy = single_recipe(&host, &input, grinder_recipes(settings), false).unwrap();
        assert_eq!(
            (legacy.quantity(), legacy.outputs[0].item().id(&host)),
            (1, "minecraft:gravel".into())
        );
        let sorted = single_recipe(&host, &input, grinder_recipes(settings), true).unwrap();
        assert_eq!(
            (sorted.quantity(), sorted.outputs[0].item().id(&host)),
            (8, "minecraft:sand".into())
        );
        let max_tier = ElectricMachine::for_item("ELECTRIC_ORE_GRINDER_3").unwrap();
        assert_eq!((max_tier.work_ticks, max_tier.cost), (0, 45));
    }

    #[test]
    fn heated_pressure_requires_both_inputs_and_scales_the_paper_duration() {
        use crate::{
            heated_pressure::{BLISTERING_INGOT, URANIUM},
            items::GOLD_24K,
        };

        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                ["minecraft:player_head", "minecraft:gold_ingot"]
                    .map(|id| Identifier::parse(id).unwrap()),
            )
            .unwrap();
        let uranium = URANIUM.stack(&host, 1).unwrap();
        let gold = GOLD_24K.stack(&host, 1).unwrap();
        assert!(heated_recipe(&host, &[Some(gold.clone()), None], 1).is_none());
        let inputs = [Some(uranium), Some(gold)];
        let standard = heated_recipe(&host, &inputs, 1).unwrap();
        assert_eq!(standard.quantity(), 2);
        assert_eq!(standard.work_override, Some(60));
        assert!(BLISTERING_INGOT.matches(&host, &standard.outputs[0]));
        let upgraded = heated_recipe(&host, &inputs, 5).unwrap();
        assert_eq!(upgraded.work_override, Some(12));
    }

    fn recipe_host<const N: usize>(items: [&str; N]) -> FakeHost {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(items.map(|id| Identifier::parse(id).unwrap()))
            .unwrap();
        host
    }

    fn test_recipe(
        host: &dyn Host,
        id: &str,
        data: &Compound,
        settings: Settings,
        roll: impl FnMut(u32) -> Option<u32>,
    ) -> Option<RecipeRun> {
        find_recipe(
            host,
            data,
            ElectricMachine::for_item(id).unwrap(),
            settings,
            false,
            roll,
        )
    }

    #[test]
    fn auto_enchanter_transfers_supported_levels_and_reserves_both_outputs() {
        let mut host = recipe_host([
            "minecraft:diamond_sword",
            "minecraft:enchanted_book",
            "minecraft:book",
        ]);
        let sword_id = "minecraft:diamond_sword";
        for enchantment in [
            "minecraft:sharpness",
            "minecraft:smite",
            "minecraft:vanishing_curse",
        ] {
            host.registries.allow_enchantment(enchantment, sword_id);
        }
        let mut sword = Kind::Vanilla(sword_id).stack(&host, 1).unwrap();
        sword.set_damage(37);
        let identity = Tag::String("Keep this blade".into());
        sword
            .apply_components(&Compound::from([
                ("minecraft:custom_name".into(), identity.clone()),
                ("minecraft:repair_cost".into(), Tag::Int(17)),
                (
                    "minecraft:enchantments".into(),
                    Tag::compound([("minecraft:sharpness", Tag::Int(6))]),
                ),
            ]))
            .unwrap();
        let mut book = Kind::Vanilla("minecraft:enchanted_book")
            .stack(&host, 1)
            .unwrap();
        book.apply_components(&Compound::from([(
            "minecraft:stored_enchantments".into(),
            Tag::compound([
                ("minecraft:sharpness", Tag::Int(7)),
                ("minecraft:smite", Tag::Int(4)),
                ("minecraft:vanishing_curse", Tag::Int(1)),
                ("minecraft:efficiency", Tag::Int(3)),
            ]),
        )]))
        .unwrap();
        let mut data = Compound::new();
        assert!(write_stack(&host, &mut data, 0, Some(sword.clone())));
        assert!(write_stack(&host, &mut data, 1, Some(book.clone())));
        let run = |id, data: &Compound| {
            test_recipe(&host, id, data, Settings::default(), |_| unreachable!())
        };
        let first = run("AUTO_ENCHANTER", &data).unwrap();
        let second = run("AUTO_ENCHANTER_2", &data).unwrap();
        assert_eq!(
            (first.work_override, second.work_override),
            (Some(450), Some(150))
        );
        assert_eq!(
            remaining(&Compound::from([(REMAINING.into(), Tag::Int(450))])),
            Some(450)
        );
        assert_eq!(first.quantity(), 2);
        let enchanted = &first.outputs[0];
        let patch = enchanted.components().unwrap().as_nbt();
        assert_eq!(enchanted.damage_component(), Some(37));
        assert_eq!(patch.get(&"minecraft:custom_name".into()), Some(&identity));
        assert_eq!(
            patch.get(&"minecraft:repair_cost".into()),
            Some(&Tag::Int(17))
        );
        assert_eq!(
            patch.get(&"minecraft:enchantments".into()),
            Some(&Tag::compound([
                ("minecraft:sharpness", Tag::Int(7)),
                ("minecraft:smite", Tag::Int(4)),
                ("minecraft:vanishing_curse", Tag::Int(1)),
            ])),
        );
        assert!(Kind::Vanilla("minecraft:book").matches(&host, &first.outputs[1]));

        // An existing output blocks the two-product operation, even with one free slot.
        assert!(write_stack(&host, &mut data, 2, Some(book.clone())));
        assert!(run("AUTO_ENCHANTER", &data).is_none());
        assert_eq!(stack_in(&host, &data, 0), Some(sword));
        assert_eq!(stack_in(&host, &data, 1), Some(book.clone()));

        assert!(write_stack(&host, &mut data, 2, None));
        // The same book would have nothing to improve on the result.
        assert!(write_stack(&host, &mut data, 0, Some(enchanted.clone())));
        assert!(run("AUTO_ENCHANTER", &data).is_none());
        assert!(write_stack(&host, &mut data, 0, Some(book)));
        assert!(write_stack(
            &host,
            &mut data,
            1,
            Some(Kind::Vanilla(sword_id).stack(&host, 1).unwrap())
        ));
        assert!(run("AUTO_ENCHANTER", &data).is_some());
    }

    #[test]
    fn auto_disenchanter_moves_repair_cost_and_direct_levels_to_a_new_book() {
        let host = recipe_host([
            "minecraft:diamond_sword",
            "minecraft:enchanted_book",
            "minecraft:book",
        ]);
        let mut sword = Kind::Vanilla("minecraft:diamond_sword")
            .stack(&host, 1)
            .unwrap();
        sword.set_damage(37);
        let enchantments = Tag::compound([
            ("minecraft:sharpness", Tag::Int(7)),
            ("minecraft:smite", Tag::Int(4)),
            ("minecraft:vanishing_curse", Tag::Int(1)),
        ]);
        sword
            .apply_components(&Compound::from([
                ("minecraft:enchantments".into(), enchantments.clone()),
                ("minecraft:repair_cost".into(), Tag::Int(17)),
            ]))
            .unwrap();
        let book = Kind::Vanilla("minecraft:book").stack(&host, 12).unwrap();
        let mut data = Compound::new();
        assert!(write_stack(&host, &mut data, 0, Some(book)));
        assert!(write_stack(&host, &mut data, 1, Some(sword)));
        let run =
            |id| test_recipe(&host, id, &data, Settings::default(), |_| unreachable!()).unwrap();
        let first = run("AUTO_DISENCHANTER");
        let second = run("AUTO_DISENCHANTER_2");
        assert_eq!(
            (first.work_override, second.work_override),
            (Some(540), Some(180))
        );
        assert_eq!(first.quantity(), 2);
        assert_eq!(first.outputs[0].damage_component(), Some(37));
        assert!(first.outputs[0].components().is_none());
        assert_eq!(
            first.outputs[1].components().unwrap().as_nbt(),
            &Compound::from([
                ("minecraft:stored_enchantments".into(), enchantments),
                ("minecraft:repair_cost".into(), Tag::Int(17)),
            ]),
        );
        assert_eq!(first.outputs[1].count(), 1);
    }

    #[test]
    fn electric_press_preserves_registration_priority_quantities_and_speed() {
        let host = recipe_host([
            "minecraft:flint",
            "minecraft:cobblestone",
            "minecraft:glass",
            "minecraft:glass_pane",
        ]);
        let mut data = Compound::new();
        let flint = Kind::Vanilla("minecraft:flint");
        let glass = Kind::Vanilla("minecraft:glass");
        let settings = Settings::default();
        let run = |data: &Compound| {
            test_recipe(
                &host,
                "ELECTRIC_PRESS_2",
                data,
                settings,
                |_| unreachable!(),
            )
        };
        assert_eq!(electric_processing::PRESSING.len(), 39);
        assert!(write_stack(
            &host,
            &mut data,
            INPUT[0],
            glass.stack(&host, 1)
        ));
        assert!(write_stack(
            &host,
            &mut data,
            INPUT[1],
            flint.stack(&host, 6)
        ));
        let cobble = run(&data).unwrap();
        assert!(Kind::Vanilla("minecraft:cobblestone").matches(&host, &cobble.outputs[0]));
        assert_eq!(
            (
                cobble.quantity(),
                cobble.outputs[0].count(),
                cobble.work_override
            ),
            (6, 1, Some(2))
        );
        assert!(cobble.inputs[0].is_none());
        assert!(write_stack(
            &host,
            &mut data,
            INPUT[1],
            flint.stack(&host, 5)
        ));
        let panes = run(&data).unwrap();
        assert!(Kind::Vanilla("minecraft:glass_pane").matches(&host, &panes.outputs[0]));
        assert_eq!(
            (
                panes.quantity(),
                panes.outputs[0].count(),
                panes.work_override
            ),
            (1, 3, Some(3))
        );
    }

    #[test]
    fn electric_panning_only_rolls_on_eligible_inputs_and_respects_tier_output_limits() {
        let host = recipe_host([
            "minecraft:flint",
            "minecraft:brick",
            "minecraft:gravel",
            "minecraft:soul_soil",
            "minecraft:quartz",
        ]);
        let mut data = Compound::new();
        let flint = Kind::Vanilla("minecraft:flint");
        let brick = Kind::Vanilla("minecraft:brick");
        assert!(write_stack(
            &host,
            &mut data,
            OUTPUT[0],
            flint.stack(&host, 63)
        ));
        assert!(write_stack(
            &host,
            &mut data,
            OUTPUT[1],
            brick.stack(&host, 1)
        ));
        assert!(write_stack(
            &host,
            &mut data,
            INPUT[0],
            Kind::Vanilla("minecraft:gravel").stack(&host, 5)
        ));
        let mut settings = Settings::default();
        assert!(
            test_recipe(&host, "ELECTRIC_GOLD_PAN_2", &data, settings, |_| {
                panic!("Paper never rolls when both output slots are occupied by default")
            })
            .is_none()
        );
        settings.gold_pan_override_output_limit[1] = true;
        let rolled = test_recipe(&host, "ELECTRIC_GOLD_PAN_2", &data, settings, |bound| {
            assert_eq!(bound, 100);
            Some(0)
        })
        .unwrap();
        assert_eq!((rolled.quantity(), rolled.work_override), (1, Some(2)));
        assert!(flint.matches(&host, &rolled.outputs[0]));

        assert!(write_stack(&host, &mut data, OUTPUT[1], None));
        assert!(write_stack(
            &host,
            &mut data,
            INPUT[0],
            brick.stack(&host, 1)
        ));
        assert!(write_stack(
            &host,
            &mut data,
            INPUT[1],
            Kind::Vanilla("minecraft:soul_soil").stack(&host, 4)
        ));
        let mut calls = 0;
        let nether = test_recipe(&host, "ELECTRIC_GOLD_PAN_3", &data, settings, |_| {
            calls += 1;
            Some(0)
        })
        .unwrap();
        assert_eq!(calls, 1);
        assert!(nether.inputs[0].is_none());
        assert!(Kind::Vanilla("minecraft:quartz").matches(&host, &nether.outputs[0]));
        assert_eq!(nether.work_override, Some(0));
    }

    #[test]
    fn electric_gravel_washing_and_ingot_processing_follow_the_paper_chain() {
        use crate::items::{LEAD_DUST, LEAD_INGOT};

        let host = recipe_host([
            "minecraft:brick",
            "minecraft:gravel",
            "minecraft:gunpowder",
            "minecraft:sand",
            "minecraft:sugar",
            "minecraft:iron_ingot",
        ]);
        let mut data = Compound::new();
        let mut settings = Settings::default();
        let gravel = Kind::Vanilla("minecraft:gravel").stack(&host, 1);
        assert!(write_stack(&host, &mut data, INPUT[0], gravel));
        let sifted =
            test_recipe(&host, "ELECTRIC_GOLD_PAN", &data, settings, |_| Some(60)).unwrap();
        assert!(SIFTED_ORE.matches(&host, &sifted.outputs[0]));

        let washer = "ELECTRIC_DUST_WASHER_2";
        assert!(write_stack(
            &host,
            &mut data,
            INPUT[0],
            Some(sifted.outputs[0].clone())
        ));
        assert!(write_stack(
            &host,
            &mut data,
            INPUT[1],
            Kind::Vanilla("minecraft:sand").stack(&host, 1)
        ));
        assert!(write_stack(
            &host,
            &mut data,
            OUTPUT[0],
            LEAD_DUST.stack(&host, 63)
        ));
        assert!(write_stack(
            &host,
            &mut data,
            OUTPUT[1],
            SALT.stack(&host, 63)
        ));
        assert!(
            test_recipe(&host, washer, &data, settings, |_| {
                panic!("nonlegacy dust washing waits for an empty output slot")
            })
            .is_none()
        );
        settings.legacy_dust_washer = true;
        let dust = test_recipe(&host, washer, &data, settings, |bound| {
            assert_eq!(bound, 9);
            Some(7)
        })
        .unwrap();
        assert!(LEAD_DUST.matches(&host, &dust.outputs[0]));
        assert_eq!(dust.work_override, Some(4));

        assert!(write_stack(&host, &mut data, INPUT[0], None));
        let salt = test_recipe(
            &host,
            washer,
            &data,
            Settings::default(),
            |_| unreachable!(),
        )
        .unwrap();
        assert_eq!((salt.quantity(), salt.outputs[0].count()), (1, 1));
        assert!(SALT.matches(&host, &salt.outputs[0]));

        data.clear();
        assert!(write_stack(
            &host,
            &mut data,
            INPUT[0],
            Some(dust.outputs[0].clone())
        ));
        let factory = ElectricMachine::for_item("ELECTRIC_INGOT_FACTORY_3").unwrap();
        let ingot =
            find_recipe(&host, &data, factory, settings, false, |_| unreachable!()).unwrap();
        assert!(LEAD_INGOT.matches(&host, &ingot.outputs[0]));
        assert_eq!(
            (
                ingot.quantity(),
                ingot.outputs[0].count(),
                factory.work_ticks,
                factory.cost
            ),
            (1, 1, 2, 20)
        );
    }

    #[test]
    fn both_smeltery_tiers_turn_old_paper_chocolate_milk_into_one_hot_drink() {
        use crate::items::{CHRISTMAS_CHOCOLATE_MILK, CHRISTMAS_HOT_CHOCOLATE, ID_KEY};

        let host = recipe_host(["minecraft:potion"]);
        let archived = ItemStack::from_nbt(
            &host,
            &Compound::from([
                ("id".into(), Tag::String("minecraft:potion".into())),
                ("count".into(), Tag::Int(1)),
                (
                    "components".into(),
                    Tag::compound([(
                        "minecraft:custom_data",
                        Tag::compound([(
                            "PublicBukkitValues",
                            Tag::compound([(
                                ID_KEY,
                                Tag::String("CHRISTMAS_CHOCOLATE_MILK".into()),
                            )]),
                        )]),
                    )]),
                ),
            ]),
        )
        .unwrap();
        assert!(CHRISTMAS_CHOCOLATE_MILK.matches(&host, &archived));
        for (id, index, shown_slot, cost, work, required) in [
            ("ELECTRIC_SMELTERY", SMELTERY_INPUT[0], 10, 10, 24, 250),
            ("ELECTRIC_SMELTERY_2", SMELTERY_INPUT[5], 29, 20, 8, 180),
        ] {
            let machine = ElectricMachine::for_item(id).unwrap();
            assert_eq!(machine.menu_slots()[index], shown_slot);
            let mut data = Compound::new();
            assert!(write_stack(&host, &mut data, index, Some(archived.clone())));
            let plan =
                find_recipe(&host, &data, machine, Settings::default(), false, |_| None).unwrap();
            assert_eq!(plan.inputs[index], Some((archived.clone(), 1)));
            assert_eq!(plan.outputs.len(), 1);
            assert_eq!(plan.outputs[0].count(), 1);
            assert!(CHRISTMAS_HOT_CHOCOLATE.matches(&host, &plan.outputs[0]));
            assert_eq!((machine.cost, plan.work_override), (cost, Some(work)));
            let spent = (0..=work).rev().fold(required, |charge, remaining| {
                pending_charge(machine, remaining, charge).unwrap()
            });
            assert_eq!(spent, 0);
        }
    }

    #[test]
    fn electric_smeltery_uses_the_paper_slots_and_prioritizes_the_six_ingredient_alloy() {
        let recipes = advanced_processing::smeltery_recipes();
        assert_eq!(recipes.len(), 33);
        let reinforced = recipes[0];
        assert_eq!(reinforced.output.kind, crate::items::REINFORCED_ALLOY_INGOT);
        assert_eq!(reinforced.inputs.len(), 6);
        let host = recipe_host([
            "minecraft:iron_ingot",
            "minecraft:gold_ingot",
            "minecraft:nether_brick",
            "minecraft:brick",
            "minecraft:copper_ingot",
            "minecraft:gunpowder",
        ]);
        let mut data = Compound::new();
        let machine = ElectricMachine::for_item("ELECTRIC_SMELTERY_2").unwrap();
        assert_eq!(machine.slot_count(), 8);
        assert_eq!(
            SMELTERY_INPUT.map(|slot| MENU_SLOTS[slot]),
            [10, 11, 19, 20, 28, 29]
        );
        for (index, ingredient) in SMELTERY_INPUT
            .into_iter()
            .zip(reinforced.inputs.iter().rev())
        {
            let input = ingredient.kind.stack(&host, 3).unwrap();
            assert!(write_stack(&host, &mut data, index, Some(input)));
        }
        let recipe = find_recipe(
            &host,
            &data,
            machine,
            Settings::default(),
            false,
            |_| unreachable!(),
        )
        .unwrap();
        assert!(reinforced.output.kind.matches(&host, &recipe.outputs[0]));
        assert_eq!((recipe.quantity(), recipe.work_override), (6, Some(8)));
        for (slot, input) in recipe.inputs.iter().enumerate() {
            assert_eq!(input.is_some(), SMELTERY_INPUT.contains(&slot));
        }

        assert!(write_stack(&host, &mut data, SMELTERY_INPUT[0], None));
        assert!(
            find_recipe(
                &host,
                &data,
                machine,
                Settings::default(),
                false,
                |_| unreachable!()
            )
            .is_none()
        );
        data.clear();
        let iron_dust = crate::items::IRON_DUST.stack(&host, 3).unwrap();
        assert!(write_stack(
            &host,
            &mut data,
            SMELTERY_INPUT[0],
            Some(iron_dust)
        ));
        assert!(
            find_recipe(
                &host,
                &data,
                machine,
                Settings::default(),
                false,
                |_| unreachable!()
            )
            .is_none()
        );
    }

    #[test]
    fn electrified_crucible_melts_paper_material_quantities_and_powers_a_lava_generator() {
        use crate::advanced_processing::{EMPTY_BUCKET, LAVA_BUCKET};

        let host = recipe_host([
            "minecraft:bucket",
            "minecraft:blue_terracotta",
            "minecraft:lava_bucket",
        ]);
        let mut data = Compound::new();
        let machine = ElectricMachine::for_item("ELECTRIFIED_CRUCIBLE_3").unwrap();
        let terracotta = Kind::Vanilla("minecraft:blue_terracotta");
        assert!(write_stack(
            &host,
            &mut data,
            INPUT[1],
            EMPTY_BUCKET.stack(&host, 1)
        ));
        assert!(write_stack(
            &host,
            &mut data,
            INPUT[0],
            terracotta.stack(&host, 11)
        ));
        assert!(
            find_recipe(
                &host,
                &data,
                machine,
                Settings::default(),
                false,
                |_| unreachable!()
            )
            .is_none()
        );
        assert!(write_stack(
            &host,
            &mut data,
            INPUT[0],
            terracotta.stack(&host, 12)
        ));
        let melt = find_recipe(
            &host,
            &data,
            machine,
            Settings::default(),
            false,
            |_| unreachable!(),
        )
        .unwrap();
        assert!(LAVA_BUCKET.matches(&host, &melt.outputs[0]));
        assert_eq!(
            (melt.quantity(), melt.work_override, machine.cost),
            (13, Some(4), 60)
        );
        let fuel = fuel_generators::generator("LAVA_GENERATOR")
            .unwrap()
            .fuel(&host, &melt.outputs[0])
            .unwrap();
        assert_eq!(fuel.ticks, 80);
        assert!(fuel.returns_bucket);
    }

    #[test]
    fn refinery_produces_the_combustion_reactor_fuel_with_the_paper_duration() {
        let host = recipe_host(["minecraft:player_head", "minecraft:bucket"]);
        let mut data = Compound::new();
        let machine = ElectricMachine::for_item("REFINERY").unwrap();
        let empty = advanced_processing::EMPTY_BUCKET.stack(&host, 1);
        assert!(write_stack(&host, &mut data, INPUT[0], empty));
        assert!(
            find_recipe(
                &host,
                &data,
                machine,
                Settings::default(),
                false,
                |_| unreachable!()
            )
            .is_none()
        );
        let oil = crate::heated_pressure::BUCKET_OF_OIL.stack(&host, 2);
        assert!(write_stack(&host, &mut data, INPUT[1], oil));
        let refined = find_recipe(
            &host,
            &data,
            machine,
            Settings::default(),
            false,
            |_| unreachable!(),
        )
        .unwrap();
        assert!(fuel_generators::BUCKET_OF_FUEL.matches(&host, &refined.outputs[0]));
        assert_eq!(
            (refined.quantity(), refined.work_override, machine.cost),
            (1, Some(80), 16)
        );
        let fuel = fuel_generators::generator("COMBUSTION_REACTOR")
            .unwrap()
            .fuel(&host, &refined.outputs[0])
            .unwrap();
        assert_eq!(fuel.ticks, 180);
        assert!(fuel.returns_bucket);
    }

    #[test]
    fn an_exhausted_oil_pump_preserves_empty_buckets_until_output_has_space() {
        let mut host = recipe_host([
            "minecraft:bucket",
            "minecraft:stone",
            "minecraft:player_head",
        ]);
        let level = host.level.level();
        let pos = [8, 71, -12];
        let buckets = advanced_processing::EMPTY_BUCKET.stack(&host, 12).unwrap();
        let full = Kind::Vanilla("minecraft:stone").stack(&host, 64).unwrap();
        let items = host
            .level
            .block_entity(pos)
            .with_items(4)
            .items
            .as_mut()
            .unwrap();
        items.slots[0] = Some(buckets.clone());
        items.slots[2] = Some(full.clone());
        items.slots[3] = Some(full.clone());
        assert!(!move_input_to_output(&mut host, level, pos, 0, &buckets));
        let items = host.level.block_entity(pos).items.as_mut().unwrap();
        assert_eq!(items.slots[0], Some(buckets.clone()));
        assert_eq!(items.slots[2], Some(full));
        items.slots[3] = None;
        assert!(move_input_to_output(&mut host, level, pos, 0, &buckets));
        let items = host.level.block_entity(pos).items.as_ref().unwrap();
        assert!(items.slots[0].is_none());
        assert_eq!(items.slots[3], Some(buckets.clone()));

        let mut data = Compound::new();
        assert!(write_stack(&host, &mut data, INPUT[0], Some(buckets)));
        let pump = ElectricMachine::for_item("OIL_PUMP").unwrap();
        let run = find_recipe(
            &host,
            &data,
            pump,
            Settings::default(),
            false,
            |_| unreachable!(),
        )
        .unwrap();
        assert!(crate::heated_pressure::BUCKET_OF_OIL.matches(&host, &run.outputs[0]));
        assert_eq!((run.quantity(), pump.work_ticks, pump.cost), (1, 52, 14));
        use crate::networks::Component;
        assert_eq!(
            Component::for_item("OIL_PUMP"),
            Some(Component::ElectricMachine {
                capacity: 256,
                carrier: "minecraft:player_head"
            })
        );
    }

    #[test]
    fn charging_bench_debits_ten_joules_then_releases_a_full_item_to_its_output() {
        let mut host = recipe_host(["minecraft:compass", "minecraft:stone"]);
        let level = host.level.level();
        let pos = [5, 70, -12];
        let id = "CHARGING_BENCH";
        let machine = ElectricMachine::for_item(id).unwrap();
        let storage = machine.storage(level, pos);
        assert!(storage.plain);
        assert_eq!(
            crate::networks::Component::for_item(id),
            Some(crate::networks::Component::ElectricMachine {
                capacity: 128,
                carrier: "minecraft:crafting_table"
            },)
        );

        let mut item = Kind::Vanilla("minecraft:compass").stack(&host, 1).unwrap();
        item.set_components(Compound::from([(
            "minecraft:custom_data".into(),
            Tag::compound([(
                "PublicBukkitValues",
                Tag::compound([
                    (
                        crate::items::ID_KEY,
                        Tag::String("PORTABLE_TELEPORTER".into()),
                    ),
                    ("slimefun:item_charge", Tag::Float(49.75f32.to_bits())),
                ]),
            )]),
        )]))
        .unwrap();
        let mut initial = Compound::new();
        assert!(write_stack(
            &host,
            &mut initial,
            INPUT[0],
            Some(item.clone())
        ));
        assert!(save(&mut host, storage, initial.clone()));
        let run = |host: &mut FakeHost, joules| {
            tick(
                host,
                level,
                Settings::default(),
                &Rolls::default(),
                pos,
                id,
                joules,
            )
        };
        assert_eq!(run(&mut host, 9), None);
        assert_eq!(data_at(&mut host, storage), Some(initial));
        assert_eq!(run(&mut host, 10), Some(0));
        let stored = data_at(&mut host, storage).unwrap();
        let charged = stack_in(&host, &stored, INPUT[0]).unwrap();
        assert!(matches!(
            charging_bench::action(&host, &charged),
            Some(charging_bench::Action::ReturnFull)
        ));
        assert!(stack_in(&host, &stored, OUTPUT[0]).is_none());

        // The fake level's item-place backend supplies the handler reached by cargo.
        let stone = Kind::Vanilla("minecraft:stone").stack(&host, 64).unwrap();
        let inventory = host
            .level
            .block_entity(pos)
            .with_items(4)
            .items
            .as_mut()
            .unwrap();
        inventory.slots[INPUT[0]] = Some(charged.clone());
        inventory.slots[OUTPUT[0]] = Some(stone.clone());
        inventory.slots[OUTPUT[1]] = Some(stone);
        assert_eq!(run(&mut host, 10), None);
        let inventory = host.level.block_entity(pos).items.as_mut().unwrap();
        assert_eq!(inventory.slots[INPUT[0]], Some(charged.clone()));
        inventory.slots[OUTPUT[1]] = None;
        assert_eq!(run(&mut host, 10), Some(10));
        let inventory = host.level.block_entity(pos).items.as_ref().unwrap();
        assert!(inventory.slots[INPUT[0]].is_none());
        assert_eq!(inventory.slots[OUTPUT[1]], Some(charged));
    }
}
