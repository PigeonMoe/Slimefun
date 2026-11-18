// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, ops::Range, sync::Mutex};

use mod_api::{
    AttachmentError, Compound, HostSide, HostTransaction, ITEMS, ItemKey, ItemResource, ItemStack,
    LevelRef, ServerHost, Tag, UseOnEvent,
};

use super::{Component, Networks, Node, Position, factory_identity, item_id, read_stack};

pub(super) const FILTER_SLOTS: [usize; 9] = [19, 20, 21, 28, 29, 30, 37, 38, 39];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CargoSettings {
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
            !filter || field("filter-type") == Some("blacklist"),
        );
        result.lore = filter && field("filter-lore") != Some("false");
        result.round_robin = field("round-robin") == Some("true");
        result.smart_fill = field("smart-fill") == Some("true");
        Ok(result)
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
        data
    }

    pub(super) fn load(data: &Compound) -> Result<Self, AttachmentError> {
        let Some(Tag::Int(channel)) = data.get(&"channel".into()) else {
            return Err(AttachmentError::Decode);
        };
        if !(0..=16).contains(channel) {
            return Err(AttachmentError::Decode);
        }
        let flag = |key: &str| matches!(data.get(&key.into()), Some(Tag::Byte(1)));
        let mut filters = BTreeMap::new();
        if let Some(Tag::Compound(items)) = data.get(&"filters".into()) {
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
            channel: *channel as u8,
            blacklist: flag("blacklist"),
            lore: flag("lore"),
            round_robin: flag("round_robin"),
            smart_fill: flag("smart_fill"),
            imported_paper: flag("paper"),
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
    if !matches!(
        Component::for_item(&id),
        Some(Component::CargoInput | Component::CargoOutput { .. })
    ) {
        return false;
    }
    let Some(player) = host.player(event.player) else {
        return false;
    };
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

fn similar(a: &ItemStack, b: &ItemStack, check_lore: bool) -> bool {
    if a.item() != b.item() {
        return false;
    }
    if let (Some(a), Some(b)) = (item_id(a), item_id(b)) {
        return a == b;
    }
    let has_meta =
        |stack: &ItemStack| stack.components().is_some() || stack.damage_component().is_some();
    let component = |stack: &ItemStack, key: &str| {
        stack
            .components()
            .and_then(|patch| patch.as_nbt().get(&key.into()))
            .cloned()
    };
    has_meta(a) == has_meta(b)
        && component(a, "minecraft:custom_name") == component(b, "minecraft:custom_name")
        && component(a, "minecraft:custom_model_data")
            == component(b, "minecraft:custom_model_data")
        && (!check_lore || component(a, "minecraft:lore") == component(b, "minecraft:lore"))
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

    fn accepts(&self, item: &ItemStack) -> bool {
        self.items
            .iter()
            .any(|filter| similar(item, filter, self.lore))
            != self.blacklist
    }
}

#[derive(Clone, Copy)]
enum InventoryKind {
    Ordinary,
    Chest,
    Factory { smeltery: bool },
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
            Self::Factory { .. } => 2..4.min(size),
            Self::Smelter(_) => 2..3.min(size),
            Self::Brewer => 0..3.min(size),
        }
    }

    fn insert(self, host: &dyn ServerHost, size: usize, stack: &ItemStack) -> Range<usize> {
        match self {
            Self::Ordinary | Self::Chest => 0..size,
            Self::Factory { smeltery: true } => 0..0,
            Self::Factory { smeltery: false } => 0..2.min(size),
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
    if let Some(id) = factory_identity(host, level, pos) {
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
        input.filter.accepts(&stack).then_some((slot, stack))
    })?;
    let cursor = if cursor < outputs.len() { cursor } else { 0 };
    for offset in 0..outputs.len() {
        let index = (cursor + offset) % outputs.len();
        let output = &outputs[index];
        if !output.filter.accepts(&stack) || input.inventory == output.inventory {
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
        let smeltery = match output.kind {
            InventoryKind::Factory { smeltery: true } => smeltery_slots(size, |slot| {
                let resource = destination.resource(host, slot)?;
                let held = destination.amount(host, slot);
                let current = resource.to_stack(held)?;
                similar(&current, stack, true)
                    .then(|| (held, destination.capacity(host, slot, &resource)))
            }),
            _ => Vec::new(),
        };
        for slot in smeltery
            .into_iter()
            .chain(output.kind.insert(host, size, stack))
        {
            let current = destination
                .resource(host, slot)
                .and_then(|item| item.to_stack(destination.amount(host, slot)));
            if let Some(current) = &current {
                if !similar(current, stack, true) {
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
