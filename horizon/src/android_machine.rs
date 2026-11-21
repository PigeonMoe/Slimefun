// SPDX-License-Identifier: GPL-3.0-only

use std::collections::BTreeMap;

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentType, BlockDropsEvent, BlockState,
    BlockUpdateFlags, Compound, Holder, Host, HostBlockStates, HostTransaction, ITEMS, Identifier,
    ItemDrop, ItemResource, ItemStack, LevelHost, LevelRef, PlayerRef, Registrar, ResourceHandler,
    ServerHost, Tag, Transaction,
};

use crate::{
    android_butchery::{Butchers, Target},
    android_fishing, android_mining,
    android_program::{self, Facing, Instruction, Script, Tier},
    android_woodcutting,
    factory::reactors::NEPTUNIUM,
    fuel_generators::BUCKET_OF_FUEL,
    heated_pressure::{BOOSTED_URANIUM, BUCKET_OF_OIL, URANIUM},
    items::{Kind, item_id},
    random::Rolls,
};

pub(crate) type Position = [i32; 3];
pub(crate) const FUEL: usize = 6;
pub(crate) const PAPER_SLOTS: [usize; 7] = [20, 21, 22, 29, 30, 31, 43];
const WORLD_UPDATE: BlockUpdateFlags = BlockUpdateFlags::CLIENTS;
pub(crate) const DEVICE: &str = "minecraft:player_head";

struct PaperControl {
    profile: [i32; 4],
    texture: &'static str,
    title: &'static str,
    lore: Option<&'static str>,
}

// ProgrammableAndroid.newInstance adds these heads after the Paper menu preset
// is built. BlockMenu.save therefore writes them alongside the actual slots.
const PAPER_CONTROLS: [PaperControl; 3] = [
    PaperControl {
        profile: [2051923214, 1764440002, -1465804083, 1654626908],
        texture: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzRhZTI5NDIyZGI0MDQ3ZWZkYjliYWMyY2RhZTVhMDcxOWViNzcyZmNjYzg4YTY2ZDkxMjMyMGIzNDNjMzQxIn19fQ==",
        title: "§aStart/Continue",
        lore: None,
    },
    PaperControl {
        profile: [728422456, 1251750602, -1768418679, 1199211128],
        texture: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2Q3OGYyYjdlNWU3NTYzOWVhN2ZiNzk2YzM1ZDM2NGM0ZGYyOGI0MjQzZTY2Yjc2Mjc3YWFkY2Q2MjYxMzM3In19fQ==",
        title: "§bMemory Core",
        lore: Some("§8⇨ §7Click to open the Script Editor"),
    },
    PaperControl {
        profile: [-1430806123, 377108335, -2105106683, -1825080643],
        texture: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzE2MTM5ZmQxYzU2NTRlNTZlOWU0ZTJjOGJlN2ViMmJkNWI0OTlkNjMzNjE2NjYzZmVlZTk5Yjc0MzUyYWQ2NCJ9fX0=",
        title: "§4Pause",
        lore: None,
    },
];

#[derive(Clone)]
pub(crate) struct Machine {
    pub owner: u128,
    pub script: Script,
    pub paused: bool,
    pub index: usize,
    pub fuel: f32,
    pub facing: Facing,
    // These are vanilla-backed chest slots, keyed by their Paper menu indices on disk.
    slots: BTreeMap<usize, Compound>,
    imported: bool,
    inventory_imported: bool,
}

impl Machine {
    fn new(owner: u128, facing: Facing) -> Self {
        Self {
            owner,
            script: Script::default(),
            paused: true,
            index: 0,
            fuel: 0.0,
            facing,
            slots: BTreeMap::new(),
            imported: false,
            inventory_imported: false,
        }
    }

    fn encode(&self) -> Result<Compound, AttachmentError> {
        let nbt_slots = self
            .slots
            .iter()
            .map(|(&slot, value)| {
                (
                    PAPER_SLOTS[slot].to_string().as_str().into(),
                    Tag::Compound(value.clone()),
                )
            })
            .collect::<Compound>();
        Ok(Compound::from([
            (
                "owner".into(),
                Tag::String(format!("{:032x}", self.owner).as_str().into()),
            ),
            (
                "script".into(),
                Tag::String(self.script.source().as_str().into()),
            ),
            ("paused".into(), Tag::Byte(i8::from(self.paused))),
            (
                "index".into(),
                Tag::Int(i32::try_from(self.index).map_err(|_| AttachmentError::Decode)?),
            ),
            ("fuel".into(), Tag::Float(self.fuel.to_bits())),
            ("rotation".into(), Tag::String(self.facing.id().into())),
            ("slots".into(), Tag::Compound(nbt_slots)),
            ("paper".into(), Tag::Byte(i8::from(self.imported))),
            (
                "paper_inventory".into(),
                Tag::Byte(i8::from(self.inventory_imported)),
            ),
        ]))
    }

    fn decode(data: &Compound) -> Result<Self, AttachmentError> {
        let integer = |key: &str| match data.get(&key.into()) {
            Some(Tag::Int(value)) => usize::try_from(*value).ok(),
            _ => None,
        };
        let string = |key: &str| match data.get(&key.into()) {
            Some(Tag::String(value)) => value.to_string().ok(),
            _ => None,
        };
        let flag = |key: &str| match data.get(&key.into()) {
            Some(Tag::Byte(0)) => Some(false),
            Some(Tag::Byte(1)) => Some(true),
            _ => None,
        };
        let invalid = AttachmentError::Decode;
        let mut state = Machine::new(
            crate::network_runtime::parse_uuid(&string("owner").ok_or(invalid)?).ok_or(invalid)?,
            Facing::from_id(&string("rotation").ok_or(invalid)?).ok_or(invalid)?,
        );
        state.script = Script::parse(&string("script").ok_or(invalid)?).ok_or(invalid)?;
        state.index = integer("index").ok_or(invalid)?;
        state.paused = flag("paused").ok_or(invalid)?;
        state.imported = flag("paper").ok_or(invalid)?;
        state.inventory_imported = if data.contains_key(&"paper_inventory".into()) {
            flag("paper_inventory").ok_or(invalid)?
        } else {
            // Old Android attachments could import the script and the slots
            // together. Treat those files as already consumed to avoid refilling.
            state.imported
        };
        state.fuel = match data.get(&"fuel".into()) {
            Some(Tag::Float(raw)) if f32::from_bits(*raw).is_finite() => f32::from_bits(*raw),
            _ => return Err(invalid),
        };
        let Some(Tag::Compound(saved)) = data.get(&"slots".into()) else {
            return Err(invalid);
        };
        for (index, slot) in PAPER_SLOTS.iter().enumerate() {
            if let Some(value) = saved.get(&slot.to_string().as_str().into()) {
                let Tag::Compound(value) = value else {
                    return Err(invalid);
                };
                state.slots.insert(index, value.clone());
            }
        }
        Ok(state)
    }
}

static MACHINE: AttachmentType<Machine> = AttachmentType {
    namespace: "slimefun",
    name: "programmable_android",
    default: || Machine::new(0, Facing::North),
    codec: Some(AttachmentCodec {
        encode: Machine::encode,
        decode: Machine::decode,
    }),
    copy_on_death: false,
    sync: false,
};

pub(crate) fn data(host: &mut dyn ServerHost, level: LevelRef, pos: Position) -> Option<Machine> {
    (level.in_place(host) && host.block_entity(level, pos).is_some()).then_some(())?;
    MACHINE
        .get_existing(host, Holder::BlockEntity(pos))
        .ok()
        .flatten()
}

pub(crate) fn save(host: &mut dyn ServerHost, pos: Position, machine: Machine) -> bool {
    MACHINE.set(host, Holder::BlockEntity(pos), machine).is_ok()
}

pub(crate) fn init(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Position,
    player: PlayerRef,
) -> bool {
    let facing = player
        .entity()
        .direction(host)
        .and_then(Facing::from_id)
        .unwrap_or(Facing::North)
        .turned(2);
    if !level.in_place(host)
        || !level
            .block(host, pos)
            .is_some_and(|block| block.id(host) == DEVICE)
        || !save(host, pos, Machine::new(player.uuid(), facing))
    {
        return false;
    }
    turn(host, level, pos, facing);
    true
}

fn legacy(
    host: &dyn Host,
    metadata: &toml::Table,
    items: BTreeMap<u32, ItemStack>,
) -> Result<Machine, &'static str> {
    let field = |name| metadata.get(name).and_then(toml::Value::as_str);
    let owner = field("owner")
        .and_then(crate::network_runtime::parse_uuid)
        .ok_or("Invalid Paper Android owner.")?;
    let facing = Facing::from_id(field("rotation").unwrap_or("NORTH"))
        .ok_or("Invalid Paper Android rotation.")?;
    let mut state = Machine::new(owner, facing);
    if let Some(source) = field("script") {
        state.script = Script::parse(source).ok_or("Invalid Paper Android script.")?;
    }
    if let Some(index) = field("index") {
        state.index = index
            .parse::<usize>()
            .map_err(|_| "Invalid Paper Android instruction index.")?;
    }
    state.fuel = field("fuel")
        .unwrap_or("0")
        .parse::<f32>()
        .ok()
        .filter(|fuel| fuel.is_finite())
        .ok_or("Invalid Paper Android fuel.")?;
    state.paused = match field("paused") {
        None | Some("true") => true,
        Some("false") => false,
        _ => return Err("Invalid Paper Android paused state."),
    };
    state.slots = paper_slots(host, items)?;
    state.imported = true;
    Ok(state)
}

fn paper_slots(
    host: &dyn Host,
    items: BTreeMap<u32, ItemStack>,
) -> Result<BTreeMap<usize, Compound>, &'static str> {
    let mut saved = BTreeMap::new();
    for (slot, stack) in items {
        if let Some(index) = PAPER_SLOTS.iter().position(|&value| value as u32 == slot) {
            let encoded = stack
                .to_nbt(host)
                .ok_or("Could not encode a Paper Android item.")?;
            saved.insert(index, encoded);
        } else if !is_paper_control(host, slot, &stack) {
            return Err("Paper Android archive has an item outside its inventory slots.");
        }
    }
    Ok(saved)
}

fn is_paper_control(host: &dyn Host, slot: u32, stack: &ItemStack) -> bool {
    let Some(control) = slot
        .checked_sub(15)
        .and_then(|index| PAPER_CONTROLS.get(index as usize))
    else {
        return false;
    };
    if stack.count() != 1 || stack.item().id(host) != DEVICE {
        return false;
    }
    let Some(parts) = stack.components().map(|components| components.as_nbt()) else {
        return false;
    };
    let Some(Tag::Compound(profile)) = parts.get(&"minecraft:profile".into()) else {
        return false;
    };
    let expected_property = Tag::compound([
        ("name", Tag::String("textures".into())),
        ("value", Tag::String(control.texture.into())),
    ]);
    if profile.len() != 2 + usize::from(profile.contains_key(&"name".into()))
        || profile
            .get(&"name".into())
            .is_some_and(|name| name != &Tag::String("".into()))
        || !matches!(profile.get(&"id".into()), Some(Tag::IntArray(id)) if id.as_slice() == control.profile)
        || !matches!(profile.get(&"properties".into()), Some(Tag::List(properties)) if properties.as_slice() == [expected_property])
        || !parts
            .get(&"minecraft:custom_name".into())
            .is_some_and(|name| paper_control_text(name, control.title))
    {
        return false;
    }
    match (control.lore, parts.get(&"minecraft:lore".into())) {
        (None, None) => parts.len() == 2,
        (Some(expected), Some(Tag::List(lore))) => {
            parts.len() == 3
                && matches!(lore.as_slice(), [blank, hint] if paper_control_text(blank, "") && paper_control_text(hint, expected))
        }
        _ => false,
    }
}

fn paper_control_text(tag: &Tag, expected: &str) -> bool {
    fn is_literal(tag: &Tag, depth: usize) -> bool {
        if depth >= 8 {
            return false;
        }
        match tag {
            Tag::String(_) => true,
            Tag::List(children) => children.iter().all(|child| is_literal(child, depth + 1)),
            Tag::Compound(parts) => {
                parts
                    .iter()
                    .all(|(key, value)| match key.to_string().as_deref() {
                        Ok("text" | "color") => matches!(value, Tag::String(_)),
                        Ok("bold" | "italic" | "underlined" | "strikethrough" | "obfuscated") => {
                            matches!(value, Tag::Byte(0))
                        }
                        Ok("extra") => is_literal(value, depth + 1),
                        _ => false,
                    })
            }
            _ => false,
        }
    }

    is_literal(tag, 0) && crate::network_runtime::paper_text(tag).as_deref() == Some(expected)
}

pub(crate) fn restore_paper(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Position,
    id: &str,
    metadata: &toml::Table,
    row: &toml::Table,
) -> Result<(), String> {
    if data(host, level, pos).is_some_and(|state| state.imported) {
        return Ok(());
    }
    if data(host, level, pos).is_some() {
        return Err("That Android already has a native owner and script.".into());
    }
    let has_inventory = row.contains_key("paper_inventory");
    let items = if has_inventory {
        crate::storage::validate_block_inventory_preset(row, id)?;
        crate::storage::decode_archived_slots(host, row, 54)?
    } else {
        BTreeMap::new()
    };
    let mut state = legacy(host, metadata, items)?;
    state.inventory_imported = has_inventory;
    save(host, pos, state)
        .then_some(())
        .ok_or("Could not save the Paper Android configuration.".into())
}

pub(crate) fn restore_installed_inventory(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Position,
    items: BTreeMap<u32, ItemStack>,
) -> Result<(), String> {
    let mut state = data(host, level, pos).ok_or("Could not read the existing Android state.")?;
    let slots = paper_slots(host, items)?;
    if state.inventory_imported {
        return Ok(());
    }
    if !state.slots.is_empty() {
        return Err("The existing Android already contains items.".into());
    }
    state.slots = slots;
    state.inventory_imported = true;
    save(host, pos, state)
        .then_some(())
        .ok_or("Could not save the Paper Android inventory.".into())
}

pub(crate) fn drop_items(level: &dyn LevelHost, event: &mut BlockDropsEvent<'_>) {
    let Some(entity) = event.block_entity else {
        return;
    };
    let Some(saved) = level.block_entity_saved_attachment(entity, MACHINE.namespace, MACHINE.name)
    else {
        return;
    };
    let Ok(machine) = Machine::decode(&saved) else {
        return;
    };
    let position = [event.pos.x, event.pos.y, event.pos.z].map(|axis| f64::from(axis) + 0.5);
    for value in machine.slots.values() {
        if let Some(stack) =
            crate::network_runtime::read_stack(value, |key| level.item_key_by_name(key))
        {
            event
                .drops
                .push(ItemDrop::new(stack, position, [0.0, 0.2, 0.0], 10));
        }
    }
}

pub(crate) fn slot(host: &dyn Host, data: &Machine, index: usize) -> Option<ItemStack> {
    ItemStack::from_nbt(host, data.slots.get(&index)?)
}

fn replace_slot(
    host: &dyn Host,
    data: &mut Machine,
    index: usize,
    value: Option<ItemStack>,
) -> Option<()> {
    if let Some(value) = value {
        data.slots.insert(index, value.to_nbt(host)?);
    } else {
        data.slots.remove(&index);
    }
    Some(())
}

struct Store {
    level: LevelRef,
    position: Position,
    accessible: bool,
}

impl ResourceHandler<ItemResource> for Store {
    fn size(&self, _: &mut dyn ServerHost) -> usize {
        if self.accessible {
            PAPER_SLOTS.len()
        } else {
            0
        }
    }

    fn resource(&self, host: &mut dyn ServerHost, index: usize) -> Option<ItemResource> {
        self.stack(host, index)
            .map(|stack| ItemResource::of(&stack))
    }

    fn amount(&self, host: &mut dyn ServerHost, index: usize) -> u32 {
        self.stack(host, index).map_or(0, |stack| stack.count())
    }

    fn capacity(&self, host: &mut dyn ServerHost, index: usize, resource: &ItemResource) -> u32 {
        if index < self.size(host) {
            crate::factory::max_stack(host, resource)
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
        let Some(mut state) = data(host, self.level, self.position) else {
            return 0;
        };
        let held = match slot(host, &state, index) {
            Some(stack) if resource.matches(&stack) => stack.count(),
            Some(_) => return 0,
            None => 0,
        };
        let amount = amount.min(capacity.saturating_sub(held));
        if amount == 0 {
            return 0;
        }
        let Some(updated) = resource.to_stack(held + amount) else {
            return 0;
        };
        if replace_slot(host, &mut state, index, Some(updated)).is_some()
            && save(host, self.position, state)
        {
            amount
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
        if index >= self.size(host) {
            return 0;
        }
        let Some(mut state) = data(host, self.level, self.position) else {
            return 0;
        };
        let Some(stack) = slot(host, &state, index).filter(|stack| resource.matches(stack)) else {
            return 0;
        };
        let amount = amount.min(stack.count());
        if amount > 0
            && replace_slot(host, &mut state, index, stack.shrink(amount)).is_some()
            && save(host, self.position, state)
        {
            amount
        } else {
            0
        }
    }
}

impl Store {
    fn stack(&self, host: &mut dyn ServerHost, index: usize) -> Option<ItemStack> {
        if index >= self.size(host) {
            return None;
        }
        let state = data(host, self.level, self.position)?;
        slot(host, &state, index)
    }
}

pub(crate) fn register(registrar: &mut dyn Registrar) {
    let skull = registrar.block_key(DEVICE);
    registrar.block_capability_before_game(&ITEMS, &skull, |host, at, _, _| {
        data(host, at.level, at.pos)?;
        Some(Box::new(Store {
            level: at.level,
            position: at.pos,
            accessible: at.side.is_none(),
        }))
    });
}

const LIQUID_FUELS: &[(Kind, u32)] = &[
    (Kind::Vanilla("minecraft:lava_bucket"), 100),
    (BUCKET_OF_OIL, 200),
    (BUCKET_OF_FUEL, 500),
];

const NUCLEAR_FUELS: &[(Kind, u32)] =
    &[(URANIUM, 2500), (NEPTUNIUM, 1200), (BOOSTED_URANIUM, 3000)];

fn fuel_ticks(host: &dyn Host, tier: Tier, stack: &ItemStack) -> Option<u32> {
    let table = match tier {
        Tier::Solid => return solid_fuel(host, stack),
        Tier::Liquid => LIQUID_FUELS,
        Tier::Nuclear => NUCLEAR_FUELS,
    };
    table
        .iter()
        .find(|(fuel, _)| fuel.matches(host, stack))
        .map(|(_, ticks)| *ticks)
}

fn solid_fuel(host: &dyn Host, stack: &ItemStack) -> Option<u32> {
    if item_id(stack).is_some()
        || stack.components().is_some()
        || stack.damage_component().is_some()
    {
        return None;
    }
    let id = stack.item().id(host);
    let ticks = match id.as_str() {
        "minecraft:coal_block" => 80,
        "minecraft:blaze_rod" => 45,
        "minecraft:dried_kelp_block" => 70,
        "minecraft:coal" | "minecraft:charcoal" => 8,
        "minecraft:bamboo" => 1,
        _ if host
            .tags()
            .key(&Identifier::parse("minecraft:logs").expect("built-in tag"))
            .and_then(|tag| host.tags().is_in_tag(stack.item(), tag))
            == Some(true) =>
        {
            2
        }
        _ if host
            .tags()
            .key(&Identifier::parse("minecraft:planks").expect("built-in tag"))
            .and_then(|tag| host.tags().is_in_tag(stack.item(), tag))
            == Some(true) =>
        {
            1
        }
        _ => return None,
    };
    Some(ticks)
}

/// Insert as much of the output as Paper would accept, in menu order.
pub(crate) fn push(host: &dyn Host, machine: &mut Machine, stack: ItemStack) -> u32 {
    let resource = ItemResource::of(&stack);
    let capacity = crate::factory::max_stack(host, &resource);
    let mut left = stack.count();
    for index in 0..FUEL {
        let held = match slot(host, machine, index) {
            Some(held) if resource.matches(&held) => held.count(),
            Some(_) => continue,
            None => 0,
        };
        let amount = left.min(capacity.saturating_sub(held));
        if amount == 0 {
            continue;
        }
        if let Some(updated) = resource.to_stack(held + amount)
            && replace_slot(host, machine, index, Some(updated)).is_some()
        {
            left -= amount;
        }
        if left == 0 {
            break;
        }
    }
    stack.count() - left
}

fn refill(host: &dyn Host, tier: Tier, machine: &mut Machine) -> Option<()> {
    let stack = slot(host, machine, FUEL)?;
    machine.fuel = fuel_ticks(host, tier, &stack)? as f32;
    replace_slot(host, machine, FUEL, stack.shrink(1))?;
    if tier == Tier::Liquid {
        let empty = Kind::Vanilla("minecraft:bucket").stack(host, 1)?;
        // Paper still refuels when all output slots are full.
        push(host, machine, empty);
    }
    Some(())
}

pub(crate) fn tick(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Position,
    item: &str,
    rolls: &Rolls,
    reduced_block_updates: bool,
    butchers: &Butchers,
) -> Option<Movement> {
    let mode = android_program::mode(item)?;
    let tier = mode.tier();
    let mut state = data(host, level, pos)?;
    if state.paused {
        return None;
    }
    if state.fuel < 0.001 {
        refill(host, tier, &mut state)?;
        save(host, pos, state);
        return None;
    }

    let step = state.script.advance(mode, state.index);
    state.fuel -= 1.0;
    if let Some((instruction, index)) = step
        && instruction != Instruction::ChopTree
    {
        state.index = index;
    }
    let owner = state.owner;
    let facing = state.facing;
    if !save(host, pos, state) {
        return None;
    }
    let (instruction, next) = step?;
    match instruction {
        Instruction::TurnLeft => turn(host, level, pos, facing.turned(-1)),
        Instruction::TurnRight => turn(host, level, pos, facing.turned(1)),
        Instruction::DigUp
        | Instruction::DigForward
        | Instruction::DigDown
        | Instruction::MoveAndDigUp
        | Instruction::MoveAndDigForward
        | Instruction::MoveAndDigDown => {
            let direction = match instruction {
                Instruction::DigUp | Instruction::MoveAndDigUp => [0, 1, 0],
                Instruction::DigDown | Instruction::MoveAndDigDown => [0, -1, 0],
                _ => facing.offset(),
            };
            let target = offset(pos, direction)?;
            android_mining::dig(host, level, pos, target, owner, reduced_block_updates);
            if matches!(
                instruction,
                Instruction::MoveAndDigUp
                    | Instruction::MoveAndDigForward
                    | Instruction::MoveAndDigDown
            ) {
                return Some(Movement {
                    from: pos,
                    to: target,
                    rotation: facing.skull_rotation(),
                });
            }
        }
        Instruction::CatchFish => {
            android_fishing::fish(host, level, pos, tier, rolls);
        }
        Instruction::AttackMobsAnimals
        | Instruction::AttackMobs
        | Instruction::AttackAnimals
        | Instruction::AttackAnimalsAdult => {
            let target = match instruction {
                Instruction::AttackMobs => Target::Monsters,
                Instruction::AttackAnimals => Target::Animals,
                Instruction::AttackAnimalsAdult => Target::AdultAnimals,
                _ => Target::All,
            };
            butchers.attack(host, level, pos, facing, tier, target);
        }
        Instruction::ChopTree => {
            let target = offset(pos, facing.offset())?;
            if android_woodcutting::chop(host, level, pos, target, owner)
                && let Some(mut state) = data(host, level, pos)
            {
                state.index = next;
                save(host, pos, state);
            }
        }
        Instruction::FarmForward
        | Instruction::FarmDown
        | Instruction::FarmExoticForward
        | Instruction::FarmExoticDown => {
            let direction = if matches!(
                instruction,
                Instruction::FarmDown | Instruction::FarmExoticDown
            ) {
                [0, -1, 0]
            } else {
                facing.offset()
            };
            harvest(host, level, pos, offset(pos, direction)?, owner, rolls);
        }
        Instruction::GoForward | Instruction::GoUp | Instruction::GoDown => {
            let direction = match instruction {
                Instruction::GoUp => [0, 1, 0],
                Instruction::GoDown => [0, -1, 0],
                _ => facing.offset(),
            };
            return Some(Movement {
                from: pos,
                to: offset(pos, direction)?,
                rotation: facing.skull_rotation(),
            });
        }
        Instruction::InterfaceItems => {
            interface_items(host, level, pos, offset(pos, facing.offset())?)
        }
        Instruction::InterfaceFuel => {
            interface_fuel(host, level, pos, offset(pos, facing.offset())?)
        }
        Instruction::Start | Instruction::Wait | Instruction::Repeat => {}
    }
    None
}

pub(crate) struct Movement {
    pub from: Position,
    pub to: Position,
    pub rotation: u8,
}

pub(crate) fn offset([x, y, z]: Position, [dx, dy, dz]: Position) -> Option<Position> {
    Some([x.checked_add(dx)?, y.checked_add(dy)?, z.checked_add(dz)?])
}

fn turn(host: &mut dyn ServerHost, level: LevelRef, pos: Position, facing: Facing) {
    let Some(mut state) = data(host, level, pos) else {
        return;
    };
    let Some(old) = level.block_state(host, pos) else {
        return;
    };
    let Ok(block) = old.with(
        &host.block_states(),
        "rotation",
        &facing.skull_rotation().to_string(),
    ) else {
        return;
    };
    state.facing = facing;
    let _ = host.transaction(|host, _| {
        if !save(host, pos, state) || level.block_state(host, pos) != Some(old) {
            return Err(());
        }
        if block != old
            && !level
                .set_block(host, pos, block, WORLD_UPDATE)
                .map_err(|_| ())?
        {
            return Err(());
        }
        Ok(())
    });
}

fn crop(id: &str) -> Option<(&'static str, u32)> {
    Some(match id {
        "minecraft:wheat" => ("minecraft:wheat", 2),
        "minecraft:potatoes" => ("minecraft:potato", 3),
        "minecraft:carrots" => ("minecraft:carrot", 3),
        "minecraft:beetroots" => ("minecraft:beetroot", 3),
        "minecraft:cocoa" => ("minecraft:cocoa_beans", 3),
        "minecraft:nether_wart" => ("minecraft:nether_wart", 3),
        "minecraft:sweet_berry_bush" => ("minecraft:sweet_berries", 3),
        _ => return None,
    })
}

pub(crate) fn inside_border(host: &dyn Host, level: LevelRef, [x, _, z]: Position) -> bool {
    level
        .world_border(host)
        .is_some_and(|[left, front, right, back]| {
            f64::from(x) + 1.0 > left
                && f64::from(x) < right
                && f64::from(z) + 1.0 > front
                && f64::from(z) < back
        })
}

fn replanted(host: &dyn Host, old: BlockState) -> Option<(BlockState, &'static str, u32)> {
    let (drop, count) = crop(&old.block().id(host))?;
    let metadata = host.block_states();
    let age = old.property(&metadata, "age").ok()??.parse::<u32>().ok()?;
    if old
        .with(&metadata, "age", &age.checked_add(1)?.to_string())
        .is_ok()
    {
        return None;
    }
    Some((old.with(&metadata, "age", "0").ok()?, drop, count))
}

fn harvest(
    host: &mut dyn ServerHost,
    level: LevelRef,
    from: Position,
    target: Position,
    owner: u128,
    rolls: &Rolls,
) {
    if !inside_border(host, level, target) {
        return;
    }
    let Some(old) = level.block_state(host, target) else {
        return;
    };
    let Some((planted, material, kinds)) = replanted(host, old) else {
        return;
    };
    if !level.allow_device_placement(host, owner, target) {
        return;
    }
    let Some(count) = rolls.next(host, level, kinds).map(|random| random + 1) else {
        return;
    };
    let Some(drop) = Kind::Vanilla(material).stack(host, count) else {
        return;
    };
    let resource = ItemResource::of(&drop);
    let result = host.transaction(|host, tx| {
        let store = Store {
            level,
            position: from,
            accessible: true,
        };
        let mut left = count;
        for index in 0..FUEL {
            left -= store.insert(host, tx, index, &resource, left);
            if left == 0 {
                break;
            }
        }
        if left != 0
            || level.block_state(host, target) != Some(old)
            || !level
                .set_block(
                    host,
                    target,
                    planted,
                    BlockUpdateFlags::CLIENTS | BlockUpdateFlags::NEIGHBORS,
                )
                .map_err(|_| ())?
        {
            return Err(());
        }
        Ok(())
    });
    if result.is_ok() {
        level.block_break_effect(host, target, old);
    }
}

fn interface_items(host: &mut dyn ServerHost, level: LevelRef, from: Position, target: Position) {
    if !crate::network_runtime::android_interface_at(host, level, target, "ANDROID_INTERFACE_ITEMS")
    {
        return;
    }
    let Some(destination) = level.capability(host, &ITEMS, target, None) else {
        return;
    };
    let store = Store {
        level,
        position: from,
        accessible: true,
    };
    for index in 0..FUEL {
        let Some(stack) = store.stack(host, index) else {
            continue;
        };
        if !level.allow_inventory_transfer(host, from, target, &stack) {
            continue;
        }
        let resource = ItemResource::of(&stack);
        let _ = host.transaction(|host, tx| {
            let mut left = stack.count();
            for occupied in [true, false] {
                for slot in 0..destination.size(host) {
                    if (destination.amount(host, slot) != 0) == occupied {
                        left -= destination.insert(host, tx, slot, &resource, left);
                        if left == 0 {
                            break;
                        }
                    }
                }
            }
            let amount = stack.count() - left;
            (store.extract(host, tx, index, &resource, amount) == amount)
                .then_some(())
                .ok_or(())
        });
    }
}

fn interface_fuel(host: &mut dyn ServerHost, level: LevelRef, to: Position, source: Position) {
    if !crate::network_runtime::android_interface_at(host, level, source, "ANDROID_INTERFACE_FUEL")
    {
        return;
    }
    let Some(input) = level.capability(host, &ITEMS, source, None) else {
        return;
    };
    let store = Store {
        level,
        position: to,
        accessible: true,
    };
    for index in 0..input.size(host) {
        let Some(resource) = input.resource(host, index) else {
            continue;
        };
        let amount = input.amount(host, index);
        let Some(stack) = resource.to_stack(amount) else {
            continue;
        };
        if !level.allow_inventory_transfer(host, source, to, &stack) {
            continue;
        }
        let _ = host.transaction(|host, tx| {
            let moved = store.insert(host, tx, FUEL, &resource, amount);
            (input.extract(host, tx, index, &resource, moved) == moved)
                .then_some(())
                .ok_or(())
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripted_farming_keeps_paper_maturity_outputs_and_rates() {
        assert_eq!(crop("minecraft:wheat"), Some(("minecraft:wheat", 2)));
        assert_eq!(
            crop("minecraft:nether_wart"),
            Some(("minecraft:nether_wart", 3))
        );
        assert_eq!(crop("minecraft:pumpkin_stem"), None);
        assert_eq!(PAPER_SLOTS, [20, 21, 22, 29, 30, 31, 43]);
    }

    #[test]
    fn liquid_and_nuclear_fuels_accept_only_their_paper_item_id_and_duration() {
        let mut host = mod_api::testing::FakeHost::new();
        host.registries
            .resolve_items(
                [
                    "minecraft:lava_bucket",
                    "minecraft:bucket",
                    "minecraft:coal",
                    "minecraft:player_head",
                ]
                .map(|id| Identifier::parse(id).unwrap()),
            )
            .unwrap();
        for (tier, table) in [(Tier::Liquid, LIQUID_FUELS), (Tier::Nuclear, NUCLEAR_FUELS)] {
            for &(kind, duration) in table {
                let stack = kind.stack(&host, 1).unwrap();
                assert_eq!(
                    fuel_ticks(&host, tier, &stack),
                    Some(duration),
                    "{}",
                    kind.id()
                );
                assert_eq!(
                    fuel_ticks(
                        &host,
                        if tier == Tier::Liquid {
                            Tier::Nuclear
                        } else {
                            Tier::Liquid
                        },
                        &stack
                    ),
                    None
                );
            }
        }
        let coal = Kind::Vanilla("minecraft:coal").stack(&host, 1).unwrap();
        assert_eq!(fuel_ticks(&host, Tier::Solid, &coal), Some(8));
        assert_eq!(fuel_ticks(&host, Tier::Liquid, &coal), None);

        let mut device = Machine::new(1, Facing::South);
        let lava = Kind::Vanilla("minecraft:lava_bucket").stack(&host, 1);
        replace_slot(&host, &mut device, FUEL, lava).unwrap();
        refill(&host, Tier::Liquid, &mut device).unwrap();
        assert_eq!(device.fuel, 100.0);
        assert!(slot(&host, &device, FUEL).is_none());
        assert!(
            Kind::Vanilla("minecraft:bucket").matches(&host, &slot(&host, &device, 0).unwrap())
        );
    }

    fn paper_menu(host: &mut mod_api::testing::FakeHost) -> BTreeMap<u32, ItemStack> {
        host.registries
            .resolve_items(
                [DEVICE, "minecraft:cod", "minecraft:coal"]
                    .map(|id| Identifier::parse(id).unwrap()),
            )
            .unwrap();
        let paper_name = |labels: &[(&str, &str)]| {
            Tag::compound([
                ("text", Tag::String("".into())),
                (
                    "extra",
                    Tag::List(
                        labels
                            .iter()
                            .map(|(label, color)| crate::items::text(label, color, false))
                            .collect(),
                    ),
                ),
            ])
        };
        let mut items = BTreeMap::new();
        for (slot, label, color) in [
            (15, "Start/Continue", "green"),
            (16, "Memory Core", "aqua"),
            (17, "Pause", "dark_red"),
        ] {
            let control = &PAPER_CONTROLS[(slot - 15) as usize];
            let mut head = Kind::Vanilla(DEVICE).stack(host, 1).unwrap();
            let mut parts = Compound::from([
                (
                    "minecraft:profile".into(),
                    Tag::compound([
                        ("id", Tag::IntArray(control.profile.into())),
                        ("name", Tag::String("".into())),
                        (
                            "properties",
                            Tag::List(vec![Tag::compound([
                                ("name", Tag::String("textures".into())),
                                ("value", Tag::String(control.texture.into())),
                            ])]),
                        ),
                    ]),
                ),
                (
                    "minecraft:custom_name".into(),
                    paper_name(&[(label, color)]),
                ),
            ]);
            if slot == 16 {
                parts.insert(
                    "minecraft:lore".into(),
                    Tag::List(vec![
                        Tag::String("".into()),
                        paper_name(&[
                            ("⇨ ", "dark_gray"),
                            ("Click to open the Script Editor", "gray"),
                        ]),
                    ]),
                );
            }
            head.apply_components(&parts).unwrap();
            items.insert(slot, head);
        }
        items.insert(20, Kind::Vanilla("minecraft:cod").stack(host, 3).unwrap());
        items.insert(43, Kind::Vanilla("minecraft:coal").stack(host, 2).unwrap());
        items
    }

    #[test]
    fn paired_paper_android_slots_drop_only_the_stock_buttons_and_keep_the_program() {
        let mut host = mod_api::testing::FakeHost::new();
        let items = paper_menu(&mut host);
        let metadata = toml::Table::from_iter([
            (
                "owner".into(),
                "aaaa0000-1111-2222-3333-444455556666".into(),
            ),
            (
                "script".into(),
                "START-CATCH_FISH-INTERFACE_ITEMS-REPEAT".into(),
            ),
            ("rotation".into(), "SOUTH".into()),
            ("fuel".into(), "8.25".into()),
            ("index".into(), "1".into()),
            ("paused".into(), "false".into()),
        ]);
        let saved = legacy(&host, &metadata, items.clone()).unwrap();
        let read = Machine::decode(&saved.encode().unwrap()).unwrap();
        assert_eq!(read.owner, saved.owner);
        assert_eq!(
            read.script.source(),
            "START-CATCH_FISH-INTERFACE_ITEMS-REPEAT"
        );
        assert_eq!(read.fuel, 8.25);
        assert_eq!(read.index, 1);
        assert_eq!(read.facing, Facing::South);
        assert!(!read.paused);
        assert_eq!(read.slots.len(), 2);
        assert_eq!(slot(&host, &read, 0).as_ref(), items.get(&20));
        assert_eq!(slot(&host, &read, FUEL).as_ref(), items.get(&43));

        let mut swapped = items.clone();
        swapped.insert(15, items[&17].clone());
        assert!(legacy(&host, &metadata, swapped).is_err());

        let mut wrong_owner = items[&15].components().unwrap().as_nbt().clone();
        let Some(Tag::Compound(profile)) = wrong_owner.get_mut(&"minecraft:profile".into()) else {
            panic!("stock button profile")
        };
        profile.insert("id".into(), Tag::IntArray(vec![0, 0, 0, 0]));
        let mut arbitrary_head = items.clone();
        arbitrary_head
            .get_mut(&15)
            .unwrap()
            .set_components(wrong_owner)
            .unwrap();
        assert!(legacy(&host, &metadata, arbitrary_head).is_err());
    }

    #[test]
    fn an_orphan_android_menu_keeps_the_current_owner_script_fuel_and_instruction() {
        let mut host = mod_api::testing::FakeHost::new();
        let pos = [2, 64, 4];
        let level = host.level.level();
        host.level.block_entity(pos);
        let items = paper_menu(&mut host);
        let mut live = Machine::new(0xb758, Facing::East);
        live.script = Script::parse("START-TURN_LEFT-REPEAT").unwrap();
        live.index = 1;
        live.fuel = 5.5;
        live.paused = false;
        assert!(save(&mut host, pos, live.clone()));
        let original = host.attachments.to_nbt();
        let mut invalid = items.clone();
        invalid
            .get_mut(&16)
            .unwrap()
            .apply_components(&Compound::from([(
                "minecraft:custom_data".into(),
                Tag::compound([("addon_data", Tag::Byte(1))]),
            )]))
            .unwrap();
        assert!(restore_installed_inventory(&mut host, level, pos, invalid).is_err());
        assert_eq!(host.attachments.to_nbt(), original);

        restore_installed_inventory(&mut host, level, pos, items.clone()).unwrap();
        let restored = data(&mut host, level, pos).unwrap();
        assert_eq!(restored.slots.len(), 2);
        assert_eq!(slot(&host, &restored, 0).as_ref(), items.get(&20));
        assert_eq!(slot(&host, &restored, FUEL).as_ref(), items.get(&43));
        live.inventory_imported = true;
        live.slots = restored.slots.clone();
        assert_eq!(restored.encode().unwrap(), live.encode().unwrap());

        let mut old = live.encode().unwrap();
        old.remove(&"paper_inventory".into());
        old.insert("paper".into(), Tag::Byte(1));
        assert!(Machine::decode(&old).unwrap().inventory_imported);
    }
}
