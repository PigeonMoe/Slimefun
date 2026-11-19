// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, ops::Range, sync::Mutex};

use mod_api::{
    AttachmentError, Compound, HostSide, HostTransaction, ITEMS, ItemKey, ItemResource, ItemStack,
    LevelRef, ServerHost, Tag, Text, UseOnEvent,
};

use super::{
    Component, Networks, Node, Position, factory_identity, item_id, parse_uuid, read_stack,
};

pub(super) const FILTER_SLOTS: [usize; 9] = [19, 20, 21, 28, 29, 30, 37, 38, 39];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CargoSettings {
    pub owner: Option<u128>,
    pub channel: u8,
    pub blacklist: bool,
    pub lore: bool,
    pub round_robin: bool,
    pub smart_fill: bool,
    pub imported_paper: bool,
    pub filters: BTreeMap<usize, Compound>,
}

impl CargoSettings {
    pub(super) fn legacy(channel: u8, passes_unfiltered: bool) -> Self {
        Self {
            owner: None,
            channel,
            blacklist: passes_unfiltered,
            lore: false,
            round_robin: false,
            smart_fill: false,
            imported_paper: false,
            filters: BTreeMap::new(),
        }
    }

    pub(super) fn from_paper(metadata: &toml::Table, filter: bool) -> Result<Self, &'static str> {
        let field = |name| metadata.get(name).and_then(toml::Value::as_str);
        let channel = field("frequency")
            .unwrap_or("0")
            .parse::<u8>()
            .ok()
            .filter(|value| *value <= 16)
            .ok_or("Invalid Paper cargo frequency.")?;
        let mut result = Self::legacy(
            channel,
            !filter || field("filter-type") != Some("whitelist"),
        );
        result.lore = filter && field("filter-lore") == Some("true");
        result.round_robin = field("round-robin") == Some("true");
        result.smart_fill = field("smart-fill") == Some("true");
        result.owner = match metadata.get("owner") {
            None => None,
            Some(owner) => Some(
                owner
                    .as_str()
                    .and_then(parse_uuid)
                    .ok_or("Invalid Paper cargo owner UUID.")?,
            ),
        };
        Ok(result)
    }

    pub(super) fn merge_paper(&mut self, paper: Self) -> Result<bool, &'static str> {
        let restore_items = paper.imported_paper && !self.imported_paper;
        if restore_items && !self.filters.is_empty() {
            return Err("Cargo inventory already contains filter items.");
        }
        let restore_owner = self.owner.is_none() && paper.owner.is_some();
        if restore_owner {
            self.owner = paper.owner;
        }
        if restore_items {
            self.filters = paper.filters;
            self.imported_paper = true;
        }
        Ok(restore_owner || restore_items)
    }

    pub(super) fn save(&self) -> Compound {
        let mut data = Compound::from([
            ("channel".into(), Tag::Int(i32::from(self.channel))),
            (
                "filters".into(),
                Tag::Compound(
                    self.filters
                        .iter()
                        .map(|(slot, stack)| {
                            (
                                slot.to_string().as_str().into(),
                                Tag::Compound(stack.clone()),
                            )
                        })
                        .collect(),
                ),
            ),
        ]);
        for (name, value) in [
            ("blacklist", self.blacklist),
            ("lore", self.lore),
            ("round_robin", self.round_robin),
            ("smart_fill", self.smart_fill),
            ("paper", self.imported_paper),
        ] {
            data.insert(name.into(), Tag::Byte(i8::from(value)));
        }
        if let Some(owner) = self.owner {
            data.insert(
                "owner".into(),
                Tag::String(format!("{owner:032x}").as_str().into()),
            );
        }
        data
    }

    pub(super) fn load(data: &Compound) -> Result<Self, AttachmentError> {
        let Some(Tag::Int(channel)) = data.get(&"channel".into()) else {
            return Err(AttachmentError::Decode);
        };
        if !(0..=16).contains(channel) {
            return Err(AttachmentError::Decode);
        }
        let flag = |key: &str| match data.get(&key.into()) {
            None | Some(Tag::Byte(0)) => Ok(false),
            Some(Tag::Byte(1)) => Ok(true),
            _ => Err(AttachmentError::Decode),
        };
        let mut filters = BTreeMap::new();
        if let Some(items) = data.get(&"filters".into()) {
            let Tag::Compound(items) = items else {
                return Err(AttachmentError::Decode);
            };
            for (slot, item) in items {
                let slot = slot
                    .to_string()
                    .ok()
                    .and_then(|slot| slot.parse::<usize>().ok())
                    .filter(|slot| FILTER_SLOTS.contains(slot))
                    .ok_or(AttachmentError::Decode)?;
                let Tag::Compound(item) = item else {
                    return Err(AttachmentError::Decode);
                };
                if filters.insert(slot, item.clone()).is_some() {
                    return Err(AttachmentError::Decode);
                }
            }
        }
        Ok(Self {
            owner: match data.get(&"owner".into()) {
                None => None,
                Some(Tag::String(owner)) => Some(
                    parse_uuid(&owner.to_string().map_err(|_| AttachmentError::Decode)?)
                        .ok_or(AttachmentError::Decode)?,
                ),
                _ => return Err(AttachmentError::Decode),
            },
            channel: *channel as u8,
            blacklist: flag("blacklist")?,
            lore: flag("lore")?,
            round_robin: flag("round_robin")?,
            smart_fill: flag("smart_fill")?,
            imported_paper: flag("paper")?,
            filters,
        })
    }
}

pub(super) fn block_invalid_placement(host: HostSide<'_>, event: &UseOnEvent<'_>) -> bool {
    let HostSide::Server(host) = host else {
        return false;
    };
    let Some(id) = item_id(event.stack) else {
        return false;
    };
    let Some(component) = Component::for_item(&id) else {
        return false;
    };
    let required = match component {
        Component::CargoManager | Component::CargoConnector => {
            &crate::network_recipes::CARGO_BASICS
        }
        Component::CargoInput | Component::CargoOutput { advanced: false } => {
            &crate::network_recipes::CARGO_NODES
        }
        Component::CargoOutput { advanced: true } => {
            &crate::network_recipes::ADVANCED_OUTPUT_NODE
        }
        _ => return false,
    };
    let Some(player) = host.player(event.player) else {
        return false;
    };
    if !crate::research::can_use(host, player, required) {
        let _ = player.tell(
            host,
            Text::literal(format!(
                "Unlock {} with /sf research {}.",
                required.name, required.id
            )),
        );
        return true;
    }
    if !matches!(component, Component::CargoInput | Component::CargoOutput { .. }) {
        return false;
    }
    let Some(level) = player.entity().level(host) else {
        return false;
    };
    let [x, y, z] = event.pos;
    let [dx, dy, dz] = event.face;
    if dy != 0 {
        return true;
    }
    let free = level
        .block_state(host, [x + dx, y, z + dz])
        .map(|state| state.block().id(host));
    !matches!(
        free.as_deref(),
        Some("minecraft:air" | "minecraft:cave_air" | "minecraft:void_air")
    )
}

pub(super) fn item_key(host: &dyn ServerHost, id: &str) -> Option<ItemKey> {
    host.item_key(&mod_api::Identifier::parse(id).ok()?)
}

fn component<'a>(stack: &'a ItemStack, key: &str) -> Option<&'a Tag> {
    stack.components()?.as_nbt().get(&key.into())
}

fn paper_lore(stack: &ItemStack) -> Option<&[Tag]> {
    match component(stack, "minecraft:lore") {
        Some(Tag::List(lines)) if !lines.is_empty() => Some(lines),
        _ => None,
    }
}

pub(crate) fn paper_text(tag: &Tag) -> Option<String> {
    let mut output = String::new();
    let mut formatted = false;
    append_paper_text(tag, &mut output, &mut formatted, 0)?;
    Some(output)
}

// Paper's legacy ItemMeta accessors visit literal components in order and
// preserve their color and formatting codes, ignoring hover and click events.
fn append_paper_text(
    tag: &Tag,
    output: &mut String,
    formatted: &mut bool,
    depth: usize,
) -> Option<()> {
    if depth >= 32 {
        return None;
    }
    let (literal, color) = match tag {
        Tag::List(children) => {
            for child in children {
                append_paper_text(child, output, formatted, depth + 1)?;
            }
            return Some(());
        }
        Tag::String(literal) => (literal.to_string().ok()?, None),
        Tag::Compound(_) => {
            let Tag::String(literal) = tag.get("text")? else {
                return None;
            };
            let color = match tag.get("color") {
                Some(Tag::String(color)) => Some(color.to_string().ok()?),
                None => None,
                _ => return None,
            };
            (literal.to_string().ok()?, color)
        }
        _ => return None,
    };
    if let Some(color) = color {
        const COLORS: [&str; 16] = [
            "black",
            "dark_blue",
            "dark_green",
            "dark_aqua",
            "dark_red",
            "dark_purple",
            "gold",
            "gray",
            "dark_gray",
            "blue",
            "green",
            "aqua",
            "red",
            "light_purple",
            "yellow",
            "white",
        ];
        const RGB: [u32; 16] = [
            0x000000, 0x0000aa, 0x00aa00, 0x00aaaa, 0xaa0000, 0xaa00aa, 0xffaa00, 0xaaaaaa,
            0x555555, 0x5555ff, 0x55ff55, 0x55ffff, 0xff5555, 0xff55ff, 0xffff55, 0xffffff,
        ];
        let rgb = color
            .strip_prefix('#')
            .filter(|hex| hex.len() == 6)
            .and_then(|hex| u32::from_str_radix(hex, 16).ok());
        let legacy = COLORS
            .iter()
            .position(|&name| name == color)
            .or_else(|| RGB.iter().position(|value| Some(*value) == rgb));
        if let Some(index) = legacy {
            output.push('§');
            output.push(char::from_digit(index as u32, 16)?);
        } else {
            output.push_str("§x");
            for digit in format!("{:06x}", rgb?).chars() {
                output.push('§');
                output.push(digit);
            }
        }
        *formatted = true;
    } else if !literal.is_empty() && *formatted {
        output.push_str("§r");
        *formatted = false;
    }
    for (style, code) in [
        ("bold", 'l'),
        ("italic", 'o'),
        ("underlined", 'n'),
        ("strikethrough", 'm'),
        ("obfuscated", 'k'),
    ] {
        if matches!(tag.get(style), Some(Tag::Byte(value)) if *value != 0) {
            output.push('§');
            output.push(code);
            *formatted = true;
        }
    }
    output.push_str(&literal);
    if let Some(extra) = tag.get("extra") {
        append_paper_text(extra, output, formatted, depth + 1)?;
    }
    Some(())
}

fn same_text(a: &Tag, b: &Tag) -> bool {
    a == b || matches!((paper_text(a), paper_text(b)), (Some(a), Some(b)) if a == b)
}

fn same_lore(a: &ItemStack, b: &ItemStack) -> bool {
    let lines = |line: &&Tag| !crate::soulbound_rune::is_soulbound_lore(line);
    fn comparable(line: &Tag) -> Result<String, &Tag> {
        paper_text(line).ok_or(line)
    }
    match (paper_lore(a), paper_lore(b)) {
        (Some(a), Some(b)) => a
            .iter()
            .filter(lines)
            .map(comparable)
            .eq(b.iter().filter(lines).map(comparable)),
        (None, None) => true,
        _ => false,
    }
}

fn paper_model_data(stack: &ItemStack) -> Option<i32> {
    match component(stack, "minecraft:custom_model_data")? {
        Tag::Int(value) => Some(*value),
        component => {
            let Tag::List(floats) = component.get("floats")? else {
                return None;
            };
            let Tag::Float(value) = floats.first()? else {
                return None;
            };
            Some(f32::from_bits(*value) as i32)
        }
    }
}

fn potion_type(stack: &ItemStack) -> Option<String> {
    let contents = component(stack, "minecraft:potion_contents")?;
    let Tag::String(key) = (match contents {
        Tag::String(_) => contents,
        _ => contents.get("potion")?,
    }) else {
        return None;
    };
    let key = key.to_string().ok()?;
    Some(key.strip_prefix("minecraft:").unwrap_or(&key).to_owned())
}

fn similar(host: &dyn ServerHost, a: &ItemStack, b: &ItemStack, check_lore: bool) -> bool {
    if a.item() != b.item() {
        return false;
    }
    if let (Some(a), Some(b)) = (item_id(a), item_id(b)) {
        // Paper's registered-item shortcut compares the registry templates. Two
        // bags of the same tier match even when their live ownership lore differs.
        return a == b;
    }
    let has_meta =
        |stack: &ItemStack| stack.components().is_some() || stack.damage_component().is_some();
    let same_name = match (
        component(a, "minecraft:custom_name"),
        component(b, "minecraft:custom_name"),
    ) {
        (Some(a), Some(b)) => same_text(a, b),
        (None, None) => true,
        _ => false,
    };
    has_meta(a) == has_meta(b)
        && same_name
        && paper_model_data(a) == paper_model_data(b)
        && (!check_lore || same_lore(a, b))
        && (potion_type(a) == potion_type(b)
            || !matches!(
                a.item().id(host).as_str(),
                "minecraft:potion"
                    | "minecraft:splash_potion"
                    | "minecraft:lingering_potion"
                    | "minecraft:tipped_arrow"
            ))
}

struct Filter {
    items: Vec<ItemStack>,
    blacklist: bool,
    lore: bool,
}

impl Filter {
    fn new(host: &dyn ServerHost, node: &Node, filter: &CargoSettings) -> Option<Self> {
        if !node.filters_items() {
            return Some(Self {
                items: Vec::new(),
                blacklist: true,
                lore: false,
            });
        }
        let items = filter
            .filters
            .values()
            .map(|data| read_stack(data, |id| item_key(host, id)))
            .collect::<Option<_>>()?;
        Some(Self {
            items,
            blacklist: filter.blacklist,
            lore: filter.lore,
        })
    }

    fn accepts(&self, host: &dyn ServerHost, item: &ItemStack) -> bool {
        self.items
            .iter()
            .any(|filter| similar(host, item, filter, self.lore))
            != self.blacklist
    }
}

#[derive(Clone, Copy)]
enum InventoryKind {
    Ordinary,
    Chest,
    TrashCan,
    ReactorPort,
    ExperienceCollector,
    GeoMiner,
    InputOnly,
    Assembler(&'static crate::factory::assemblers::Assembler),
    Factory { smeltery: bool },
    HeatedPressure,
    Smelter(&'static str),
    Brewer,
}

impl InventoryKind {
    fn from_block(id: &str) -> Option<Self> {
        Some(match id {
            "minecraft:furnace" => Self::Smelter("minecraft:smelting"),
            "minecraft:blast_furnace" => Self::Smelter("minecraft:blasting"),
            "minecraft:smoker" => Self::Smelter("minecraft:smoking"),
            "minecraft:brewing_stand" => Self::Brewer,
            "minecraft:chest" | "minecraft:trapped_chest" => Self::Chest,
            "minecraft:barrel"
            | "minecraft:hopper"
            | "minecraft:dispenser"
            | "minecraft:dropper" => Self::Ordinary,
            id if id.starts_with("minecraft:") && id.ends_with("shulker_box") => Self::Ordinary,
            _ => return None,
        })
    }

    fn extract(self, size: usize) -> Range<usize> {
        match self {
            Self::Ordinary | Self::Chest => 0..size,
            Self::TrashCan => 0..0,
            Self::ReactorPort => 6.min(size)..7.min(size),
            Self::ExperienceCollector => 0..3.min(size),
            Self::GeoMiner => 0..size,
            Self::InputOnly => 0..0,
            Self::Assembler(_) => 0..0,
            Self::Factory { .. } | Self::HeatedPressure => 2..4.min(size),
            Self::Smelter(_) => 2..3.min(size),
            Self::Brewer => 0..3.min(size),
        }
    }

    fn insert(self, host: &dyn ServerHost, size: usize, stack: &ItemStack) -> Range<usize> {
        match self {
            Self::Ordinary | Self::Chest | Self::TrashCan => 0..size,
            Self::Factory { smeltery: true } | Self::HeatedPressure => 0..0,
            Self::Factory { smeltery: false } => 0..2.min(size),
            Self::ReactorPort => {
                if crate::factory::reactors::coolant(host, stack) {
                    3.min(size)..6.min(size)
                } else {
                    0..3.min(size)
                }
            }
            Self::ExperienceCollector | Self::GeoMiner => 0..0,
            Self::InputOnly => 0..size,
            Self::Assembler(spec) => {
                let carrier = stack.item().id(host);
                let slots = if carrier == spec.head.kind.material() {
                    crate::factory::assemblers::HEAD
                } else if carrier == spec.body.kind.material() {
                    crate::factory::assemblers::BODY
                } else {
                    0..0
                };
                slots.start.min(size)..slots.end.min(size)
            }
            Self::Smelter(recipe) => {
                let fuel = host
                    .item_burn_time(stack.item())
                    .is_some_and(|time| time > 0);
                if !fuel {
                    return 0..1.min(size);
                }
                let smeltable = host.recipes_of_type(recipe).iter().any(|recipe| {
                    recipe
                        .ingredients
                        .first()
                        .is_some_and(|input| input.test(host, stack))
                });
                usize::from(!smeltable)..2.min(size)
            }
            Self::Brewer => match stack.item().id(host).as_str() {
                "minecraft:potion" | "minecraft:splash_potion" | "minecraft:lingering_potion" => {
                    0..3.min(size)
                }
                "minecraft:blaze_powder" => 4..5.min(size),
                _ => 3..4.min(size),
            },
        }
    }

    fn slot_position(
        self,
        host: &dyn ServerHost,
        level: LevelRef,
        pos: Position,
        size: usize,
        slot: usize,
    ) -> Option<Position> {
        if !matches!(self, Self::Chest) || size != 54 {
            return Some(pos);
        }
        let half = level.block_property(host, pos, "type")?;
        let left = match half.as_str() {
            "left" => true,
            "right" => false,
            _ => return None,
        };
        if left == (slot >= 27) {
            return Some(pos);
        }
        // The game exposes the right half first, including when querying at the left half.
        let facing = level.block_property(host, pos, "facing")?;
        let [dx, dz] = match facing.as_str() {
            "north" => [1, 0],
            "south" => [-1, 0],
            "east" => [0, 1],
            "west" => [0, -1],
            _ => return None,
        };
        let side = if left { 1 } else { -1 };
        Some([
            pos[0].checked_add(side * dx)?,
            pos[1],
            pos[2].checked_add(side * dz)?,
        ])
    }
}

fn attached_inventory(
    host: &mut dyn ServerHost,
    level: LevelRef,
    mut pos: Position,
) -> Option<(Position, InventoryKind)> {
    let (axis, delta) = match level.block_property(host, pos, "facing")?.as_str() {
        "north" => (2, 1),
        "south" => (2, -1),
        "east" => (0, -1),
        "west" => (0, 1),
        _ => return None,
    };
    pos[axis] = pos[axis].checked_add(delta)?;
    if super::trash::registered(host, level, pos) {
        return Some((pos, InventoryKind::TrashCan));
    }
    if let Some(id) = factory_identity(host, level, pos) {
        if crate::auto_crafter::is_machine(&id) {
            return None;
        }
        if id == crate::factory::experience_collector::XP_COLLECTOR.id() {
            return Some((pos, InventoryKind::ExperienceCollector));
        }
        if id == crate::geology::GEO_MINER.id() {
            return Some((pos, InventoryKind::GeoMiner));
        }
        if id == crate::factory::tree_growth::TREE_GROWTH_ACCELERATOR.id()
            || crate::factory::animal_machines::machine(&id).is_some()
            || crate::factory::crop_growth::machine(&id).is_some()
        {
            return Some((pos, InventoryKind::InputOnly));
        }
        if let Some(spec) = crate::factory::assemblers::by_id(&id) {
            return Some((pos, InventoryKind::Assembler(spec)));
        }
        if id == crate::factory::reactors::REACTOR_ACCESS_PORT.id() {
            return Some((pos, InventoryKind::ReactorPort));
        }
        if crate::factory::reactors::reactor(&id).is_some() {
            return None;
        }
        if matches!(
            id.as_str(),
            "HEATED_PRESSURE_CHAMBER" | "HEATED_PRESSURE_CHAMBER_2"
        ) {
            return Some((pos, InventoryKind::HeatedPressure));
        }
        let smeltery = crate::factory::ElectricMachine::for_item(&id)
            .is_some_and(crate::factory::ElectricMachine::is_smeltery);
        return Some((pos, InventoryKind::Factory { smeltery }));
    }
    let state = level.block_state(host, pos)?;
    Some((pos, InventoryKind::from_block(&state.block().id(host))?))
}

pub(super) fn tick(
    networks: &Mutex<Networks>,
    host: &mut dyn ServerHost,
    level: LevelRef,
    active: &BTreeMap<Position, (&Node, u32)>,
    group: &[Position],
) {
    let mut inputs = Vec::new();
    let mut outputs = BTreeMap::<u8, Vec<_>>::new();
    for &pos in group {
        let node = active[&pos].0;
        if !matches!(
            node.component,
            Component::CargoInput | Component::CargoOutput { .. }
        ) {
            continue;
        }
        let Some((inventory, kind)) = attached_inventory(host, level, pos) else {
            continue;
        };
        let options = node.cargo_settings(host, level, pos);
        let Some(filter) = Filter::new(host, node, &options) else {
            continue;
        };
        let terminal = Terminal {
            pos,
            inventory,
            kind,
            options,
            filter,
        };
        if node.component == Component::CargoInput && terminal.options.channel < 16 {
            inputs.push(terminal);
        } else if matches!(node.component, Component::CargoOutput { .. }) {
            outputs
                .entry(terminal.options.channel)
                .or_default()
                .push(terminal);
        }
    }
    for input in &inputs {
        let Some(outputs) = outputs.get(&input.options.channel) else {
            continue;
        };
        let cursor_key = (level, input.pos);
        let cursor = if input.options.round_robin {
            networks
                .lock()
                .unwrap()
                .round_robin
                .get(&cursor_key)
                .copied()
                .unwrap_or(0)
        } else {
            0
        };
        if let Some(next) = route(host, level, input, outputs, cursor)
            && input.options.round_robin
        {
            networks
                .lock()
                .unwrap()
                .round_robin
                .insert(cursor_key, next);
        }
    }
}

struct Terminal {
    pos: Position,
    inventory: Position,
    kind: InventoryKind,
    options: CargoSettings,
    filter: Filter,
}

fn route(
    host: &mut dyn ServerHost,
    level: LevelRef,
    input: &Terminal,
    outputs: &[Terminal],
    cursor: usize,
) -> Option<usize> {
    let source = level.capability(host, &ITEMS, input.inventory, None)?;
    let (slot, mut stack) = input.kind.extract(source.size(host)).find_map(|slot| {
        let stack = source
            .resource(host, slot)?
            .to_stack(source.amount(host, slot))?;
        input.filter.accepts(host, &stack).then_some((slot, stack))
    })?;
    let cursor = if cursor < outputs.len() { cursor } else { 0 };
    for offset in 0..outputs.len() {
        let index = (cursor + offset) % outputs.len();
        let output = &outputs[index];
        if !output.filter.accepts(host, &stack) || input.inventory == output.inventory {
            continue;
        }
        let moved = move_to_first(
            host,
            level,
            input,
            slot,
            output,
            &stack,
            |host, from, to, stack| {
                from != to && level.allow_inventory_transfer(host, from, to, stack)
            },
        );
        // Each destination fills its first compatible stack, and the next output can receive the remainder.
        if moved == stack.count() {
            return Some((index + 1) % outputs.len());
        }
        if moved != 0 {
            stack = stack.shrink(moved)?;
        }
    }
    None
}

// Paper spreads the same item only among inputs already holding that item.
fn smeltery_slots(
    size: usize,
    mut matching: impl FnMut(usize) -> Option<(u32, u32)>,
) -> Vec<usize> {
    let inputs = crate::factory::SMELTERY_INPUT
        .into_iter()
        .filter(|&slot| slot < size);
    let mut matching_slots: Vec<_> = inputs
        .clone()
        .filter_map(|slot| matching(slot).map(|(count, capacity)| (slot, count, capacity)))
        .collect();
    if matching_slots.is_empty() {
        return inputs.collect();
    }
    matching_slots.sort_by_key(|&(_, count, _)| count);
    matching_slots
        .into_iter()
        .filter_map(|(slot, count, capacity)| (count < capacity).then_some(slot))
        .collect()
}

// Paper uses its amount-sensitive similarity check here, so only
// existing stacks at least as large as this delivery participate in the sort.
fn pressure_slots(
    size: usize,
    incoming: u32,
    mut matching: impl FnMut(usize) -> Option<u32>,
) -> Vec<usize> {
    let slots = 0..2.min(size);
    let mut preferred: Vec<_> = slots
        .clone()
        .filter_map(|slot| {
            let count = matching(slot)?;
            (count >= incoming).then_some((slot, count))
        })
        .collect();
    if preferred.is_empty() {
        return slots.collect();
    }
    preferred.sort_by_key(|&(_, count)| count);
    preferred.into_iter().map(|(slot, _)| slot).collect()
}

fn move_to_first(
    host: &mut dyn ServerHost,
    level: LevelRef,
    input: &Terminal,
    source_slot: usize,
    output: &Terminal,
    stack: &ItemStack,
    mut permit: impl FnMut(&mut dyn ServerHost, Position, Position, &ItemStack) -> bool,
) -> u32 {
    let resource = ItemResource::of(stack);
    host.transaction(|host, tx| {
        let source = level
            .capability(host, &ITEMS, input.inventory, None)
            .ok_or(())?;
        let destination = level
            .capability(host, &ITEMS, output.inventory, None)
            .ok_or(())?;
        let size = destination.size(host);
        let source_size = source.size(host);
        let from = input
            .kind
            .slot_position(host, level, input.inventory, source_size, source_slot)
            .ok_or(())?;
        let mut endpoint_allowed = BTreeMap::new();
        let preferred = match output.kind {
            InventoryKind::Factory { smeltery: true } => smeltery_slots(size, |slot| {
                let resource = destination.resource(host, slot)?;
                let held = destination.amount(host, slot);
                let current = resource.to_stack(held)?;
                similar(host, &current, stack, true)
                    .then(|| (held, destination.capacity(host, slot, &resource)))
            }),
            InventoryKind::HeatedPressure => pressure_slots(size, stack.count(), |slot| {
                let resource = destination.resource(host, slot)?;
                let count = destination.amount(host, slot);
                let current = resource.to_stack(count)?;
                similar(host, &current, stack, true).then_some(count)
            }),
            _ => Vec::new(),
        };
        for slot in preferred
            .into_iter()
            .chain(output.kind.insert(host, size, stack))
        {
            let current = destination
                .resource(host, slot)
                .and_then(|item| item.to_stack(destination.amount(host, slot)));
            if let Some(current) = &current {
                if !similar(host, current, stack, true) {
                    continue;
                }
                if input.options.smart_fill
                    && current.count() >= destination.capacity(host, slot, &resource)
                {
                    return Ok(0);
                }
            }
            let to = output
                .kind
                .slot_position(host, level, output.inventory, size, slot)
                .ok_or(())?;
            if !*endpoint_allowed
                .entry(to)
                .or_insert_with(|| permit(host, from, to, stack))
            {
                continue;
            }
            let moved = destination.insert(host, tx, slot, &resource, stack.count());
            if moved == 0 {
                continue;
            }
            return if source.extract(host, tx, source_slot, &resource, moved) == moved {
                Ok(moved)
            } else {
                Err(())
            };
        }
        Ok(0)
    })
    .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Identifier, prelude::ItemsHost, testing::FakeHost};

    fn restored_filter(host: &dyn ServerHost, stack: &ItemStack) -> Filter {
        let node = Node {
            id: "CARGO_NODE_INPUT".to_owned(),
            component: Component::CargoInput,
            channel: 0,
            passes_unfiltered: false,
        };
        let mut settings = CargoSettings::legacy(0, false);
        settings.lore = true;
        settings.imported_paper = true;
        settings
            .filters
            .insert(FILTER_SLOTS[0], stack.to_nbt(host).unwrap());
        Filter::new(host, &node, &CargoSettings::load(&settings.save()).unwrap()).unwrap()
    }

    #[test]
    fn restored_potion_filters_use_the_base_type_for_every_paper_potion_material() {
        let mut host = FakeHost::new();
        let materials = [
            "minecraft:potion",
            "minecraft:splash_potion",
            "minecraft:lingering_potion",
            "minecraft:tipped_arrow",
        ];
        host.registries
            .resolve_items(materials.map(|id| Identifier::parse(id).unwrap()))
            .unwrap();
        for material in materials {
            let make = |potion: &str, color: i32| {
                let mut stack = ItemStack::new(item_key(&host, material).unwrap(), 1).unwrap();
                stack
                    .set_components(Compound::from([(
                        "minecraft:potion_contents".into(),
                        Tag::compound([
                            ("potion", Tag::String(potion.into())),
                            ("custom_color", Tag::Int(color)),
                        ]),
                    )]))
                    .unwrap();
                stack
            };
            let filter = restored_filter(&host, &make("minecraft:poison", 0));
            assert!(filter.accepts(&host, &make("poison", 0x1e21d3)));
            assert!(!filter.accepts(&host, &make("minecraft:strong_poison", 0)));
            assert!(!filter.accepts(&host, &make("minecraft:water", 0)));
        }
    }

    #[test]
    fn restored_backpack_filters_keep_papers_registered_tier_shortcut() {
        use crate::backpacks::{MEDIUM_BACKPACK, SMALL_BACKPACK};

        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse(SMALL_BACKPACK.material()).unwrap()])
            .unwrap();
        let make = |kind: crate::items::Kind, label: &str| {
            let mut stack = kind.stack(&host, 1).unwrap();
            stack
                .apply_components(&Compound::from([(
                    "minecraft:lore".into(),
                    Tag::List(vec![crate::items::text(label, "gray", false)]),
                )]))
                .unwrap();
            stack
        };
        let first = make(SMALL_BACKPACK, "ID: 2728cc32-8191-44e7-86c9-2120aa3896c4#1");
        let other_owner = "ID: 23938676-252c-4992-9a7a-1879732cdbe0#8";
        let filter = restored_filter(&host, &first);
        assert!(filter.accepts(&host, &make(SMALL_BACKPACK, other_owner)));
        assert!(!filter.accepts(&host, &make(MEDIUM_BACKPACK, other_owner)));
    }

    #[test]
    fn restored_lore_filters_ignore_only_the_soulbound_line_and_use_papers_model_number() {
        let mut host = FakeHost::new();
        let id = "minecraft:stick";
        host.registries
            .resolve_items([Identifier::parse(id).unwrap()])
            .unwrap();
        let make = |lines: Vec<Tag>, model: Tag| {
            let mut stack = ItemStack::new(item_key(&host, id).unwrap(), 1).unwrap();
            stack
                .set_components(Compound::from([
                    ("minecraft:lore".into(), Tag::List(lines)),
                    ("minecraft:custom_model_data".into(), model),
                ]))
                .unwrap();
            stack
        };
        let label = crate::items::text("Cargo", "green", false);
        let gray = crate::items::text("Soulbound", "gray", false);
        let model = |primary: f32, secondary: f32| {
            Tag::compound([(
                "floats",
                Tag::List(vec![
                    Tag::Float(primary.to_bits()),
                    Tag::Float(secondary.to_bits()),
                ]),
            )])
        };
        let original = make(vec![label.clone()], model(12.0, 5.0));
        let filter = restored_filter(&host, &original);
        assert!(filter.accepts(
            &host,
            &make(vec![label.clone(), gray.clone()], model(12.9, 1.0))
        ));
        assert!(!filter.accepts(
            &host,
            &make(
                vec![label.clone(), crate::items::text("Soulbound", "red", false)],
                model(12.0, 5.0)
            )
        ));
        assert!(!filter.accepts(&host, &make(vec![label], model(13.0, 5.0))));
        let gray_only = restored_filter(&host, &make(vec![gray], model(12.0, 5.0)));
        assert!(!gray_only.accepts(&host, &make(Vec::new(), model(12.0, 5.0))));
    }

    #[test]
    fn restored_filter_compares_paper_text_instead_of_the_component_tag_shape() {
        let mut host = FakeHost::new();
        let id = "minecraft:stick";
        host.registries
            .resolve_items([Identifier::parse(id).unwrap()])
            .unwrap();
        let make = |name: Tag, line: Tag| {
            let mut stack = ItemStack::new(item_key(&host, id).unwrap(), 1).unwrap();
            stack
                .set_components(Compound::from([
                    ("minecraft:custom_name".into(), name),
                    ("minecraft:lore".into(), Tag::List(vec![line])),
                ]))
                .unwrap();
            stack
        };
        let original = make(
            Tag::String("Archived Cargo".into()),
            crate::items::text("Restricted", "green", false),
        );
        let name = Tag::compound([
            ("text", Tag::String("Archived Cargo".into())),
            ("italic", Tag::Byte(0)),
        ]);
        let equivalent = Tag::compound([
            ("text", Tag::String("".into())),
            (
                "extra",
                Tag::List(vec![Tag::compound([
                    ("text", Tag::String("Restricted".into())),
                    ("color", Tag::String("#55ff55".into())),
                ])]),
            ),
        ]);
        let filter = restored_filter(&host, &original);
        assert!(filter.accepts(&host, &make(name.clone(), equivalent)));
        assert!(!filter.accepts(
            &host,
            &make(name, crate::items::text("Restricted", "aqua", false))
        ));
    }

    #[test]
    fn smeltery_cargo_prefers_existing_partial_stacks_and_refuses_full_matching_inputs() {
        assert_eq!(smeltery_slots(8, |_| None), [4, 5, 0, 1, 6, 7]);
        assert_eq!(
            smeltery_slots(8, |slot| match slot {
                4 => Some((12, 64)),
                0 => Some((3, 64)),
                6 => Some((64, 64)),
                _ => None,
            }),
            [0, 4]
        );
        assert!(smeltery_slots(8, |slot| (slot == 4).then_some((64, 64))).is_empty());
    }

    #[test]
    fn a_heated_pressure_chamber_preserves_the_empty_second_ingredient_slot() {
        let mut host = FakeHost::new();
        let level = host.level.level();
        let material = Identifier::parse("minecraft:sugar").unwrap();
        host.registries.resolve_items([material.clone()]).unwrap();
        let item = host.item_key(&material).unwrap();
        let source = [3, 70, 4];
        let target = [5, 70, 4];
        host.level.block_entity(source).with_items(1);
        host.level.block_entity(target).with_items(4);
        let terminal = |pos, kind| Terminal {
            pos,
            inventory: pos,
            kind,
            options: CargoSettings::legacy(0, true),
            filter: Filter {
                items: Vec::new(),
                blacklist: true,
                lore: false,
            },
        };
        let input = terminal(source, InventoryKind::Ordinary);
        let output = terminal(target, InventoryKind::HeatedPressure);
        let put = |host: &mut FakeHost, pos, slot, count| {
            host.level.block_entity(pos).items.as_mut().unwrap().slots[slot] =
                ItemStack::new(item, count);
        };
        let count = |host: &FakeHost, pos, slot: usize| {
            host.level
                .block_entity_at(pos)
                .unwrap()
                .items
                .as_ref()
                .unwrap()
                .slots[slot]
                .as_ref()
                .map_or(0, ItemStack::count)
        };
        let incoming = ItemStack::new(item, 2).unwrap();
        let deliver = |host: &mut FakeHost| {
            move_to_first(
                host,
                level,
                &input,
                0,
                &output,
                &incoming,
                |_, from, to, _| {
                    assert_eq!((from, to), (source, target));
                    true
                },
            )
        };

        // Both Paper tiers expose input 19 and 20 as internal slots 0 and 1.
        // A full matching stack in 20 reserves 19 for the other ingredient.
        put(&mut host, source, 0, 2);
        put(&mut host, target, 1, 64);
        assert_eq!(deliver(&mut host), 0);
        assert_eq!(
            (
                count(&host, source, 0),
                count(&host, target, 0),
                count(&host, target, 1)
            ),
            (2, 0, 64)
        );
        // Space made in a matching stack is used before the earlier empty slot.
        put(&mut host, target, 1, 20);
        assert_eq!(deliver(&mut host), 2);
        assert_eq!((count(&host, target, 0), count(&host, target, 1)), (0, 22));
        // A stack smaller than the incoming stack does not take part in the preference.
        put(&mut host, source, 0, 2);
        put(&mut host, target, 1, 1);
        assert_eq!(deliver(&mut host), 2);
        assert_eq!((count(&host, target, 0), count(&host, target, 1)), (2, 1));
        assert_eq!(
            pressure_slots(4, 2, |slot| [Some(12), Some(3)][slot]),
            [1, 0]
        );
    }

    #[test]
    fn only_the_moved_amount_leaves_source_and_a_stale_source_rolls_back_destination() {
        let mut host = FakeHost::new();
        let level = host.level.level();
        let material = Identifier::parse("minecraft:stone").unwrap();
        host.registries.resolve_items([material.clone()]).unwrap();
        let item = host.item_key(&material).unwrap();
        let source = [3, 70, 4];
        let target = [5, 70, 4];
        host.level
            .block_entity(source)
            .with_items(1)
            .items
            .as_mut()
            .unwrap()
            .slots[0] = ItemStack::new(item, 3);
        host.level
            .block_entity(target)
            .with_items(1)
            .items
            .as_mut()
            .unwrap()
            .slots[0] = ItemStack::new(item, 63);
        let terminal = |pos| Terminal {
            pos,
            inventory: pos,
            kind: InventoryKind::Ordinary,
            options: CargoSettings::legacy(0, true),
            filter: Filter {
                items: Vec::new(),
                blacklist: true,
                lore: false,
            },
        };
        let (input, output) = (terminal(source), terminal(target));
        let stack = ItemStack::new(item, 3).unwrap();
        let permit = |_: &mut dyn ServerHost, from, to, _: &ItemStack| {
            assert_eq!((from, to), (source, target));
            true
        };
        assert_eq!(
            move_to_first(&mut host, level, &input, 0, &output, &stack, permit),
            1
        );
        let count = |host: &FakeHost, pos| {
            host.level
                .block_entity_at(pos)
                .unwrap()
                .items
                .as_ref()
                .unwrap()
                .slots[0]
                .as_ref()
                .map_or(0, ItemStack::count)
        };
        assert_eq!((count(&host, source), count(&host, target)), (2, 64));
        host.level
            .block_entity(target)
            .items
            .as_mut()
            .unwrap()
            .slots[0] = None;
        assert_eq!(
            move_to_first(&mut host, level, &input, 0, &output, &stack, permit),
            0
        );
        assert_eq!((count(&host, source), count(&host, target)), (2, 0));
    }
}
