// SPDX-License-Identifier: GPL-3.0-only

use std::sync::Mutex;

use mod_api::{
    BlockEntityRef, ComponentId, Compound, Holder, ITEMS, ItemResource, ItemStack, LevelRef,
    MenuClickedEvent, MenuClosedEvent, MenuReplace, MenuSlotPlace, PlayerRef, ResourceHandler,
    ServerHost, Tag, Text, Transaction,
};

use super::{
    Component, FILTER_SLOTS, NETWORK_STATE, Networks, Node, Position, block_data,
    cargo_topology::{CargoSettings, item_key},
    network_item, network_visuals, read_stack, trash,
};

#[derive(Clone, Copy)]
pub(super) struct Session {
    level: LevelRef,
    block: BlockEntityRef,
    pos: Position,
    window: i32,
}

pub(crate) fn use_node(
    networks: &Mutex<Networks>,
    host: &mut dyn ServerHost,
    player: PlayerRef,
    level: LevelRef,
    pos: Position,
) -> bool {
    let Some(mut data) = block_data(host, level, pos) else {
        return false;
    };
    let Some(component) = Component::for_item(&data.id) else {
        return false;
    };
    match component {
        Component::TrashCan => {
            trash::open(host, player, level, pos);
            return true;
        }
        Component::SolarGenerator { .. } => return true,
        Component::ElectricMachine { .. }
        | Component::FuelGenerator { .. }
        | Component::EnergyRegulator
        | Component::EnergyConnector
        | Component::Capacitor(_)
        | Component::CargoConnector => return false,
        Component::CargoManager => {
            data.cargo_visualizer_disabled = !data.cargo_visualizer_disabled;
            let disabled = data.cargo_visualizer_disabled;
            if NETWORK_STATE
                .set(host, Holder::BlockEntity(pos), data)
                .is_ok()
            {
                network_visuals::toggled(host, player, disabled);
            }
            return true;
        }
        _ => {}
    }
    let (Some(block), Some(dimension)) =
        (host.block_entity(level, pos), level.dimension().id(host))
    else {
        return false;
    };
    let Ok(nodes) = networks.lock().unwrap().nodes(host, &dimension) else {
        return false;
    };
    let Some(node) = nodes.get(&pos).filter(|node| node.id == data.id) else {
        return false;
    };
    if data.cargo.is_none() {
        data.cargo = Some(CargoSettings::legacy(node.channel, node.passes_unfiltered));
        if NETWORK_STATE
            .set(host, Holder::BlockEntity(pos), data.clone())
            .is_err()
        {
            return true;
        }
    }
    let Some(cargo) = &data.cargo else {
        return true;
    };
    let preview = view(host, node, cargo);
    let name = match network_item(&data.id) {
        Some(crate::items::Kind::Slimefun(item)) => item.name,
        _ => return false,
    };
    let uuid = player.uuid();
    if let Ok(Some(window)) = host.open_block_inventory_menu(
        player,
        block,
        (preview.len() / 9) as u8,
        Text::literal(name),
        preview,
        MenuReplace::InPlace,
    ) {
        let mut backing =
            vec![MenuSlotPlace::Decoration; if node.filters_items() { 54 } else { 27 }];
        if node.filters_items() {
            for (slot, menu_slot) in FILTER_SLOTS.into_iter().enumerate() {
                backing[menu_slot] = MenuSlotPlace::BlockEntityItems {
                    entity: block,
                    slot,
                    allow_insert: true,
                };
            }
        }
        if player.back_mod_menu_slots(host, window, backing).is_err() {
            host.close_menu(uuid);
            return true;
        }
        networks.lock().unwrap().menus.insert(
            uuid,
            Session {
                level,
                block,
                pos,
                window,
            },
        );
    }
    true
}

pub(crate) fn menu_click(
    networks: &Mutex<Networks>,
    host: &mut dyn ServerHost,
    event: &MenuClickedEvent,
) {
    let session = networks.lock().unwrap().menus.get(&event.uuid).copied();
    let Some(session) = session.filter(|menu| menu.window == event.window) else {
        return;
    };
    let Session {
        level, block, pos, ..
    } = session;
    let active = host
        .player(event.uuid)
        .is_some_and(|player| player.entity().level(host) == Some(level));
    if !active || host.block_entity(level, pos) != Some(block) {
        host.close_menu(event.uuid);
        return;
    }
    let Some(mut data) = block_data(host, level, pos) else {
        return;
    };
    let Some(dimension) = level.dimension().id(host) else {
        return;
    };
    let Ok(nodes) = networks.lock().unwrap().nodes(host, &dimension) else {
        return;
    };
    let Some(node) = nodes.get(&pos).filter(|node| node.id == data.id) else {
        return;
    };
    let cargo = data
        .cargo
        .get_or_insert_with(|| CargoSettings::legacy(node.channel, node.passes_unfiltered));
    let Ok(slot) = usize::try_from(event.slot) else {
        return;
    };
    if event.mode != 0 || !matches!(event.button, 0 | 1) || !settings_click(node, cargo, slot) {
        return;
    }
    if NETWORK_STATE
        .set(host, Holder::BlockEntity(pos), data.clone())
        .is_err()
    {
        return;
    }
    networks.lock().unwrap().round_robin.remove(&(level, pos));
    let cargo = data.cargo.expect("calculated this session's settings");
    // Rebuild other viewers of this same block, since the inventory and controls are shared.
    let viewers: Vec<_> = networks
        .lock()
        .unwrap()
        .menus
        .iter()
        .filter(|(_, other)| other.block == block)
        .map(|(&uuid, viewer)| (uuid, viewer.window))
        .collect();
    for (uuid, window) in viewers {
        let _ = host.menu_set_items(uuid, window, view(host, node, &cargo));
    }
}

pub(crate) fn menu_closed(networks: &Mutex<Networks>, event: &MenuClosedEvent) {
    let mut networks = networks.lock().unwrap();
    if networks
        .menus
        .get(&event.uuid)
        .is_some_and(|other| other.window == event.window)
    {
        networks.menus.remove(&event.uuid);
    }
}

pub(crate) fn register(registrar: &mut dyn mod_api::Registrar) {
    network_visuals::register(registrar);
    trash::register(registrar);
    let head = registrar.block_key("minecraft:player_wall_head");
    registrar.block_capability_before_game(&ITEMS, &head, |host, at, _, entity| {
        let entity = entity?;
        let data = block_data(host, at.level, at.pos)?;
        let component = Component::for_item(&data.id)?;
        let filters = matches!(
            component,
            Component::CargoInput | Component::CargoOutput { advanced: true }
        );
        (filters && at.side.is_none() && data.cargo.is_some()).then_some(Box::new(
            FilterInventory {
                level: at.level,
                pos: at.pos,
                entity,
            },
        ))
    });
}

struct FilterInventory {
    level: LevelRef,
    pos: Position,
    entity: BlockEntityRef,
}

impl FilterInventory {
    fn data(&self, host: &mut dyn ServerHost) -> Option<super::BlockData> {
        (host.block_entity(self.level, self.pos) == Some(self.entity))
            .then(|| block_data(host, self.level, self.pos))
            .flatten()
    }

    fn stack(&self, host: &mut dyn ServerHost, index: usize) -> Option<ItemStack> {
        let data = self.data(host)?;
        let saved = data.cargo.as_ref()?.filters.get(FILTER_SLOTS.get(index)?)?;
        read_stack(saved, |id| item_key(host, id))
    }

    fn store(
        &self,
        host: &mut dyn ServerHost,
        index: usize,
        stack: Option<&ItemStack>,
    ) -> Option<()> {
        let mut data = self.data(host)?;
        let filters = &mut data.cargo.as_mut()?.filters;
        let slot = *FILTER_SLOTS.get(index)?;
        if let Some(stack) = stack {
            filters.insert(slot, stack.to_nbt(host)?);
        } else {
            filters.remove(&slot);
        }
        NETWORK_STATE
            .set(host, Holder::BlockEntity(self.pos), data)
            .ok()
    }
}

impl ResourceHandler<ItemResource> for FilterInventory {
    fn size(&self, _: &mut dyn ServerHost) -> usize {
        FILTER_SLOTS.len()
    }

    fn resource(&self, host: &mut dyn ServerHost, index: usize) -> Option<ItemResource> {
        self.stack(host, index).as_ref().map(ItemResource::of)
    }

    fn amount(&self, host: &mut dyn ServerHost, index: usize) -> u32 {
        self.stack(host, index).as_ref().map_or(0, ItemStack::count)
    }

    fn capacity(&self, host: &mut dyn ServerHost, index: usize, item: &ItemResource) -> u32 {
        if index >= FILTER_SLOTS.len() {
            return 0;
        }
        let key =
            ComponentId::new("minecraft:max_stack_size").expect("valid built-in component id");
        match item.value(host, &key) {
            Some(Tag::Int(size @ 1..=99)) => size.min(64) as u32,
            _ => host.max_stack_size(item.item()).unwrap_or(0).min(64),
        }
    }

    fn insert(
        &self,
        host: &mut dyn ServerHost,
        _: &Transaction,
        index: usize,
        item: &ItemResource,
        amount: u32,
    ) -> u32 {
        let old = self.stack(host, index);
        if old
            .as_ref()
            .is_some_and(|existing| ItemResource::of(existing) != *item)
        {
            return 0;
        }
        let count = old.as_ref().map_or(0, ItemStack::count);
        let moved = self
            .capacity(host, index, item)
            .saturating_sub(count)
            .min(amount);
        if moved == 0 {
            return 0;
        }
        let Some(stack) = item.to_stack(count + moved) else {
            return 0;
        };
        if self.store(host, index, Some(&stack)).is_some() {
            moved
        } else {
            0
        }
    }

    fn extract(
        &self,
        host: &mut dyn ServerHost,
        _: &Transaction,
        index: usize,
        item: &ItemResource,
        amount: u32,
    ) -> u32 {
        let Some(stack) = self.stack(host, index) else {
            return 0;
        };
        if ItemResource::of(&stack) != *item {
            return 0;
        }
        let moved = stack.count().min(amount);
        if moved != 0
            && self
                .store(host, index, stack.shrink(moved).as_ref())
                .is_some()
        {
            moved
        } else {
            0
        }
    }
}

fn settings_click(node: &Node, cargo: &mut CargoSettings, slot: usize) -> bool {
    let channel = if node.filters_items() { 42 } else { 13 };
    if slot == channel - 1 {
        cargo.channel = if cargo.channel == 0 {
            15
        } else {
            cargo.channel - 1
        };
    } else if slot == channel + 1 {
        cargo.channel = if cargo.channel >= 15 {
            0
        } else {
            cargo.channel + 1
        };
    } else if node.filters_items() && slot == 15 {
        cargo.blacklist = !cargo.blacklist;
    } else if node.filters_items() && slot == 25 {
        cargo.lore = !cargo.lore;
    } else if node.component == Component::CargoInput && slot == 16 {
        cargo.smart_fill = !cargo.smart_fill;
    } else if node.component == Component::CargoInput && slot == 24 {
        cargo.round_robin = !cargo.round_robin;
    } else {
        return false;
    }
    true
}

fn icon(host: &dyn ServerHost, id: &str, name: &str) -> Option<ItemStack> {
    let mut stack = ItemStack::of(item_key(host, id)?);
    stack
        .set_components(Compound::from([(
            "minecraft:custom_name".into(),
            Tag::String(name.into()),
        )]))
        .ok()?;
    Some(stack)
}

fn view(host: &dyn ServerHost, node: &Node, cargo: &CargoSettings) -> Vec<Option<ItemStack>> {
    const COLORS: [&str; 16] = [
        "white",
        "orange",
        "magenta",
        "light_blue",
        "yellow",
        "lime",
        "pink",
        "gray",
        "light_gray",
        "cyan",
        "purple",
        "blue",
        "brown",
        "green",
        "red",
        "black",
    ];
    let mut items = vec![
        icon(host, "minecraft:cyan_stained_glass_pane", " ");
        if node.filters_items() { 54 } else { 27 }
    ];
    let channel = if node.filters_items() { 42 } else { 13 };
    items[channel - 1] = icon(host, "minecraft:arrow", "Previous Channel");
    items[channel + 1] = icon(host, "minecraft:arrow", "Next Channel");
    let wool = COLORS.get(usize::from(cargo.channel)).map_or_else(
        || "minecraft:chest".to_owned(),
        |color| format!("minecraft:{color}_wool"),
    );
    items[channel] = icon(host, &wool, &format!("Channel ID {}", cargo.channel + 1));
    if node.filters_items() {
        for slot in FILTER_SLOTS {
            items[slot] = cargo
                .filters
                .get(&slot)
                .and_then(|data| read_stack(data, |id| item_key(host, id)));
        }
        items[15] = if cargo.blacklist {
            icon(host, "minecraft:black_wool", "Type: Blacklist")
        } else {
            icon(host, "minecraft:white_wool", "Type: Whitelist")
        };
        items[25] = icon(
            host,
            "minecraft:map",
            if cargo.lore {
                "Include Lore: On"
            } else {
                "Include Lore: Off"
            },
        );
    }
    if node.component == Component::CargoInput {
        items[16] = if cargo.smart_fill {
            icon(host, "minecraft:written_book", "Smart-Filling Mode: On")
        } else {
            icon(host, "minecraft:writable_book", "Smart-Filling Mode: Off")
        };
        items[24] = icon(
            host,
            "minecraft:player_head",
            if cargo.round_robin {
                "Round-Robin Mode: On"
            } else {
                "Round-Robin Mode: Off"
            },
        );
    }
    items
}
