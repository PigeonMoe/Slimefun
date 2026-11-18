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
    electric_processing::{self, Family, MachineSpec},
    gold_pan::PANS,
    items::{GOLD_PAN, Kind, PULVERIZED_ORE, PURE_ORE_CLUSTER, SALT, SIFTED_ORE},
    machine::{Settings, makeshift_recipe},
    network_runtime,
    random::Rolls,
    research,
};

type Position = [i32; 3];
const SLOTS: [&str; 4] = ["19", "20", "24", "25"];
const INPUT: [usize; 2] = [0, 1];
const OUTPUT: [usize; 2] = [2, 3];
const PENDING: &str = "pending";
const REMAINING: &str = "remaining";
const TOTAL: &str = "total";
const MAX_WORK_TICKS: i32 = 180;
const IMPORT: &str = "paper_inventory_import";
const MENU_SLOTS: [usize; 4] = [19, 20, 24, 25];
const BACKGROUND: &[usize] = &[
    0, 1, 2, 3, 4, 5, 6, 7, 8, 13, 31, 36, 37, 38, 39, 40, 41, 42, 43, 44,
];
const INPUT_BORDER: &[usize] = &[9, 10, 11, 12, 18, 21, 27, 28, 29, 30];
const OUTPUT_BORDER: &[usize] = &[14, 15, 16, 17, 23, 26, 32, 33, 34, 35];

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
}

#[derive(Clone, Copy)]
struct Viewer {
    furnace: ElectricMachine,
    storage: Storage,
    entity: Option<BlockEntityRef>,
    window: i32,
    remaining: Option<u32>,
}

#[derive(Default)]
pub(crate) struct FactoryViewers(Mutex<BTreeMap<u128, Viewer>>);

pub(crate) static VIEWERS: FactoryViewers = FactoryViewers(Mutex::new(BTreeMap::new()));

impl FactoryViewers {
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
            let remaining = remaining(&data);
            if remaining == viewer.remaining {
                continue;
            }
            let items = menu_items(host, &data, viewer.furnace);
            if host.menu_set_items(uuid, viewer.window, items).is_ok()
                && let Some(current) = self.0.lock().unwrap().get_mut(&uuid)
                && current.window == viewer.window
            {
                current.remaining = remaining;
            }
        }
    }
}

impl ElectricMachine {
    pub fn for_item(id: &str) -> Option<Self> {
        use RecipeFamily::{Furnace, HeatedPressure, IngotPulverizer, OreGrinder};
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

    fn storage(self, level: LevelRef, pos: Position) -> Storage {
        Storage {
            level,
            pos,
            plain: network_runtime::is_plain_carrier(self.carrier),
        }
    }
}

pub(crate) fn register(registrar: &mut dyn mod_api::Registrar) {
    for material in [
        "minecraft:furnace",
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
                // An empty handler also refuses the backing vanilla slots for sided access.
                size: if at.side.is_none() { SLOTS.len() } else { 0 },
            }))
        });
    }
}

pub(crate) fn open(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    level: LevelRef,
    pos: Position,
) -> bool {
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
    let storage = furnace.storage(level, pos);
    let Some(data) = data_at(host, storage) else {
        return true;
    };
    let Some(block) = host.block_key(furnace.carrier) else {
        return true;
    };
    let entity = host.block_entity(level, pos);
    let items = menu_items(host, &data, furnace);
    let title = Text::literal(furnace.title);
    let opened = match entity {
        Some(entity) if !storage.plain => {
            host.open_block_inventory_menu(player, entity, 5, title, items, MenuReplace::CloseFirst)
        }
        None if storage.plain => host.open_inventory_menu_at(
            player,
            level,
            pos,
            5,
            title,
            items,
            MenuReplace::CloseFirst,
        ),
        _ => return true,
    };
    if let Ok(Some(window)) = opened {
        let mut backing = vec![MenuSlotPlace::Decoration; 45];
        for (slot, menu_slot) in MENU_SLOTS.into_iter().enumerate() {
            let allow_insert = INPUT.contains(&slot);
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
                    remaining: remaining(&data),
                },
            );
        }
    }
    true
}

fn remaining(data: &Compound) -> Option<u32> {
    match data.get(&REMAINING.into()) {
        Some(Tag::Int(value @ 0..=MAX_WORK_TICKS)) => Some(*value as u32),
        _ => None,
    }
}

fn menu_items(
    host: &dyn Host,
    data: &Compound,
    furnace: ElectricMachine,
) -> Vec<Option<ItemStack>> {
    let mut items = vec![None; 45];
    for (indices, material) in [
        (BACKGROUND, "minecraft:gray_stained_glass_pane"),
        (INPUT_BORDER, "minecraft:cyan_stained_glass_pane"),
        (OUTPUT_BORDER, "minecraft:orange_stained_glass_pane"),
    ] {
        let pane = display_item(host, material, " ");
        for &index in indices {
            items[index] = pane.clone();
        }
    }
    for (index, menu_slot) in MENU_SLOTS.into_iter().enumerate() {
        items[menu_slot] = stack_in(host, data, index);
    }
    items[22] = match remaining(data) {
        Some(remaining) => {
            let total = match data.get(&TOTAL.into()) {
                Some(Tag::Int(total @ 0..=MAX_WORK_TICKS)) => *total as u32,
                _ => furnace.work_ticks,
            };
            let progress = if total == 0 {
                100
            } else {
                100 - (remaining + 1).min(total) * 100 / total
            };
            let (icon, damage) = match furnace.family {
                RecipeFamily::Furnace | RecipeFamily::HeatedPressure => {
                    ("minecraft:flint_and_steel", 63)
                }
                RecipeFamily::OreGrinder | RecipeFamily::IngotPulverizer => {
                    ("minecraft:iron_pickaxe", 249)
                }
                RecipeFamily::Processing(spec) => match spec.family {
                    Family::Press => ("minecraft:iron_hoe", 249),
                    Family::GoldPan => ("minecraft:diamond_shovel", 1560),
                    Family::DustWasher => ("minecraft:golden_shovel", 31),
                    Family::IngotFactory => ("minecraft:flint_and_steel", 63),
                },
            };
            let mut bar = display_item(host, icon, &format!("{progress}%"));
            if let Some(stack) = &mut bar {
                stack.set_damage(damage - progress * damage / 100);
            }
            bar
        }
        _ => display_item(host, "minecraft:black_stained_glass_pane", " "),
    };
    items
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
    for slot in SLOTS {
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
    let Tag::Compound(saved) = data.get(&(*SLOTS.get(index)?).into())? else {
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
    let key = SLOTS[index].into();
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

fn max_stack(host: &dyn Host, resource: &ItemResource) -> u32 {
    let key = ComponentId::new("minecraft:max_stack_size").expect("a built-in component id");
    match resource.value(host, &key) {
        Some(Tag::Int(max @ 1..=99)) => max.min(64) as u32,
        _ => host.max_stack_size(resource.item()).unwrap_or(0).min(64),
    }
}

struct MachineInventory {
    storage: Storage,
    size: usize,
}

impl ResourceHandler<ItemResource> for MachineInventory {
    fn size(&self, _: &mut dyn ServerHost) -> usize {
        self.size
    }

    fn resource(&self, host: &mut dyn ServerHost, index: usize) -> Option<ItemResource> {
        (index < self.size).then_some(())?;
        let data = data_at(host, self.storage)?;
        stack_in(host, &data, index).map(|stack| ItemResource::of(&stack))
    }

    fn amount(&self, host: &mut dyn ServerHost, index: usize) -> u32 {
        if index >= self.size {
            return 0;
        }
        data_at(host, self.storage)
            .and_then(|data| stack_in(host, &data, index))
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
        let held = match stack_in(host, &data, index) {
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
        if write_stack(host, &mut data, index, Some(stack)) && save(host, self.storage, data) {
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
        let Some(stack) = stack_in(host, &data, index).filter(|stack| resource.matches(stack))
        else {
            return 0;
        };
        let extracted = amount.min(stack.count());
        if extracted > 0
            && write_stack(host, &mut data, index, stack.shrink(extracted))
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

struct RecipeRun {
    inputs: [Option<(ItemStack, u32)>; 2],
    output: ItemStack,
    work_override: Option<u32>,
}

impl RecipeRun {
    fn single(index: usize, input: ItemStack, amount: u32, output: ItemStack) -> Self {
        let mut inputs = [None, None];
        inputs[index] = Some((input, amount));
        Self {
            inputs,
            output,
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
        let mut consumed = [None, None];
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
            output: recipe.output.kind.stack(host, recipe.output.count)?,
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
    roll: impl FnMut(u32) -> Option<u32>,
) -> Option<RecipeRun> {
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
    }?;
    fits(host, data, &run.output).then_some(run)
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
    if let Some(Tag::Compound(saved)) = data.get(&PENDING.into()) {
        let next_charge = charge.checked_sub(furnace.cost)?;
        let remaining = remaining(&data)?;
        if remaining > 0 {
            let mut next = data;
            next.insert(REMAINING.into(), Tag::Int(remaining as i32 - 1));
            return save(host, storage, next).then_some(next_charge);
        }
        let output = ItemStack::from_nbt(host, saved)?;
        return host
            .transaction(|host, tx| {
                let resource = ItemResource::of(&output);
                let mut left = output.count();
                for index in OUTPUT {
                    left -= place(level, pos, index).insert(host, tx, &resource, left);
                    if left == 0 {
                        break;
                    }
                }
                if left != 0 {
                    return Err(());
                }
                let mut next = data_at(host, storage).ok_or(())?;
                next.remove(&PENDING.into());
                next.remove(&REMAINING.into());
                next.remove(&TOTAL.into());
                save(host, storage, next).then_some(next_charge).ok_or(())
            })
            .ok();
    }

    let RecipeRun {
        inputs,
        output,
        work_override,
    } = find_recipe(host, &data, furnace, settings, |bound| {
        rolls.next(host, level, bound)
    })?;
    let saved_result = output.to_nbt(host)?;
    let work_ticks = work_override.unwrap_or(furnace.work_ticks) as i32;
    host.transaction(|host, tx| {
        for (index, consumed) in inputs.into_iter().enumerate() {
            if let Some((input, amount)) = consumed
                && place(level, pos, index).extract(host, tx, &ItemResource::of(&input), amount)
                    != amount
            {
                return Err(());
            }
        }
        let mut next = data_at(host, storage).ok_or(())?;
        next.insert(PENDING.into(), Tag::Compound(saved_result));
        next.insert(REMAINING.into(), Tag::Int(work_ticks));
        next.insert(TOTAL.into(), Tag::Int(work_ticks));
        save(host, storage, next).then_some(charge).ok_or(())
    })
    .ok()
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
            if machine.carrier == "minecraft:furnace" {
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
        let Some(index) = SLOTS
            .iter()
            .position(|slot| slot.parse::<u32>() == Ok(paper_slot))
        else {
            return Err(format!(
                "Unexpected electric machine inventory slot {paper_slot}."
            ));
        };
        if stack.count() > max_stack(host, &ItemResource::of(&stack))
            || !write_stack(host, &mut restored, index, Some(stack))
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
    if !data.is_empty() {
        return Err(
            "The electric machine already holds local inventory or processing state.".into(),
        );
    }
    Ok(false)
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
    use mod_api::testing::FakeHost;

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
            (legacy.quantity(), legacy.output.item().id(&host)),
            (1, "minecraft:gravel".into())
        );
        let sorted = single_recipe(&host, &input, grinder_recipes(settings), true).unwrap();
        assert_eq!(
            (sorted.quantity(), sorted.output.item().id(&host)),
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
        assert!(BLISTERING_INGOT.matches(&host, &standard.output));
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
            roll,
        )
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
        assert!(Kind::Vanilla("minecraft:cobblestone").matches(&host, &cobble.output));
        assert_eq!(
            (
                cobble.quantity(),
                cobble.output.count(),
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
        assert!(Kind::Vanilla("minecraft:glass_pane").matches(&host, &panes.output));
        assert_eq!(
            (panes.quantity(), panes.output.count(), panes.work_override),
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
        assert!(flint.matches(&host, &rolled.output));

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
        assert!(Kind::Vanilla("minecraft:quartz").matches(&host, &nether.output));
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
        assert!(SIFTED_ORE.matches(&host, &sifted.output));

        let washer = "ELECTRIC_DUST_WASHER_2";
        assert!(write_stack(&host, &mut data, INPUT[0], Some(sifted.output)));
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
        assert!(LEAD_DUST.matches(&host, &dust.output));
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
        assert_eq!((salt.quantity(), salt.output.count()), (1, 1));
        assert!(SALT.matches(&host, &salt.output));

        data.clear();
        assert!(write_stack(&host, &mut data, INPUT[0], Some(dust.output)));
        let factory = ElectricMachine::for_item("ELECTRIC_INGOT_FACTORY_3").unwrap();
        let ingot = find_recipe(&host, &data, factory, settings, |_| unreachable!()).unwrap();
        assert!(LEAD_INGOT.matches(&host, &ingot.output));
        assert_eq!(
            (
                ingot.quantity(),
                ingot.output.count(),
                factory.work_ticks,
                factory.cost
            ),
            (1, 1, 2, 20)
        );
    }
}
