// SPDX-License-Identifier: GPL-3.0-only

use std::sync::Mutex;

use mod_api::{
    BlockEntityRef, ComponentId, Compound, Holder, ITEMS, ItemResource, ItemStack, LevelRef,
    MenuClickedEvent, MenuClosedEvent, MenuReplace, MenuSlotPlace, PlayerRef, ResourceHandler,
    ServerHost, Tag, Text, Transaction,
};

use crate::hologram_projector::colored_lore;

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
        | Component::EnhancedFurnace
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

fn icon(
    host: &dyn ServerHost,
    id: &str,
    name: &str,
    lore: &[&str],
    head: Option<PaperHead>,
) -> Option<ItemStack> {
    let mut stack = ItemStack::of(item_key(host, id)?);
    let mut components = Compound::from([("minecraft:custom_name".into(), colored_lore(name))]);
    if !lore.is_empty() {
        components.insert(
            "minecraft:lore".into(),
            Tag::List(lore.iter().map(|line| colored_lore(line)).collect()),
        );
    }
    if let Some((profile, texture)) = head {
        components.insert(
            "minecraft:profile".into(),
            Tag::compound([
                ("id", Tag::IntArray(profile.to_vec())),
                ("name", Tag::String("".into())),
                (
                    "properties",
                    Tag::List(vec![Tag::compound([
                        ("name", Tag::String("textures".into())),
                        ("value", Tag::String(texture.into())),
                    ])]),
                ),
            ]),
        );
    }
    if id == "minecraft:written_book" {
        components.insert(
            "minecraft:written_book_content".into(),
            empty_book_content(),
        );
    }
    stack.set_components(components).ok()?;
    Some(stack)
}

fn empty_book_content() -> Tag {
    Tag::compound([
        ("author", Tag::String("".into())),
        ("title", Tag::compound([("raw", Tag::String("".into()))])),
    ])
}

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

type PaperHead = ([i32; 4], &'static str);

// Paper CustomHeads derives these UUIDs from the full texture URL.
const PREVIOUS_CHANNEL_HEAD: PaperHead = (
    [-1919439658, 1078672705, -1174910964, 145128982],
    "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2YyNTk5YmQ5ODY2NTliOGNlMmM0OTg4NTI1Yzk0ZTE5ZGRkMzlmYWQwOGEzODI4NGExOTdmMWI3MDY3NWFjYyJ9fX0=",
);
const NEXT_CHANNEL_HEAD: PaperHead = (
    [1784358170, -2073676120, -1924980063, -1612165449],
    "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2MyZjkxMGM0N2RhMDQyZTRhYTI4YWY2Y2M4MWNmNDhhYzZjYWYzN2RhYjM1Zjg4ZGI5OTNhY2NiOWRmZTUxNiJ9fX0=",
);
const TERMINAL_CHANNEL_HEAD: PaperHead = (
    [1353565477, 1734883325, -1331914614, -828615412],
    "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzdhNDRmZjNhNWY0OWM2OWNhYjY3NmJhZDhkOThhMDYzZmE3OGNmYTYxOTE2ZmRlZjNlMjY3NTU3ZmVjMTgyODMifX19",
);
const ROUND_ROBIN_HEAD: PaperHead = (
    [728422456, 1251750602, -1768418679, 1199211128],
    "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2Q3OGYyYjdlNWU3NTYzOWVhN2ZiNzk2YzM1ZDM2NGM0ZGYyOGI0MjQzZTY2Yjc2Mjc3YWFkY2Q2MjYxMzM3In19fQ==",
);

/// Paper adds these buttons to individual menus, so its preset still
/// classifies their slots as inventory and archives the rendered icons.
pub(super) fn paper_control(
    host: &dyn ServerHost,
    component: Component,
    slot: u32,
    stack: &ItemStack,
) -> bool {
    if stack.count() != 1 {
        return false;
    }
    let Some(parts) = stack.components().map(|components| components.as_nbt()) else {
        return false;
    };
    let plain_text =
        |part: &Tag| super::paper_text(part).map(|text| crate::guide::strip_colors(&text));
    let Some(name) = parts
        .get(&"minecraft:custom_name".into())
        .and_then(plain_text)
    else {
        return false;
    };
    let material = stack.item().id(host);
    let filtered = matches!(
        component,
        Component::CargoInput | Component::CargoOutput { advanced: true }
    );
    let channel = if filtered { 42 } else { 13 };
    let round_robin_lore = [
        "",
        if name.ends_with('✔') {
            "> Click to disable Round Robin Mode"
        } else {
            "> Click to enable Round Robin Mode"
        },
        "(Items will be equally distributed on the Channel)",
    ];
    let smart_filling_lore = [
        "",
        if name.ends_with('✔') {
            "> Click to disable \"Smart-Filling\" Mode"
        } else {
            "> Click to enable \"Smart-Filling\" Mode"
        },
        "",
        "In this mode, the Cargo node will attempt",
        "to keep a constant amount of items",
        "in the inventory. This is not perfect",
        "and will still fill in empty slots that",
        "come before a stack of a configured item.",
    ];
    let (head, lore): (Option<PaperHead>, &[&str]) = match (slot, material.as_str(), name.as_str())
    {
        (s, "minecraft:player_head", "Previous Channel") if s == channel - 1 => (
            Some(PREVIOUS_CHANNEL_HEAD),
            &["", "> Click to decrease the Channel ID by 1"],
        ),
        (s, "minecraft:player_head", "Next Channel") if s == channel + 1 => (
            Some(NEXT_CHANNEL_HEAD),
            &["", "> Click to increase the Channel ID by 1"],
        ),
        (s, "minecraft:player_head", "Channel ID: 17") if s == channel => {
            (Some(TERMINAL_CHANNEL_HEAD), &[])
        }
        (s, material, name) if s == channel && paper_channel_wool(material, name) => (None, &[]),
        (15, "minecraft:white_wool", "Type: Whitelist") if filtered => {
            (None, &["", "> Click to change it to Blacklist"])
        }
        (15, "minecraft:black_wool", "Type: Blacklist") if filtered => {
            (None, &["", "> Click to change it to Whitelist"])
        }
        (25, "minecraft:map", "Include Lore: ✔" | "Include Lore: ✘") if filtered => (
            None,
            &["", "> Click to toggle whether the Lore has to match"],
        ),
        (24, "minecraft:player_head", "Round-Robin Mode: ✔" | "Round-Robin Mode: ✘")
            if component == Component::CargoInput =>
        {
            (Some(ROUND_ROBIN_HEAD), &round_robin_lore)
        }
        (16, "minecraft:writable_book", "\"Smart-Filling\" Mode: ✘")
        | (16, "minecraft:written_book", "\"Smart-Filling\" Mode: ✔")
            if component == Component::CargoInput =>
        {
            (None, &smart_filling_lore)
        }
        _ => return false,
    };
    // Paper serializes its empty written-book button with default book data.
    let book = parts.get(&"minecraft:written_book_content".into());
    if let Some(book) = book
        && (material != "minecraft:written_book" || *book != empty_book_content())
    {
        return false;
    }
    if parts.len()
        != 1 + usize::from(head.is_some())
            + usize::from(!lore.is_empty())
            + usize::from(book.is_some())
    {
        return false;
    }
    if let Some((id, texture)) = head {
        let Some(Tag::Compound(profile)) = parts.get(&"minecraft:profile".into()) else {
            return false;
        };
        let property = Tag::compound([
            ("name", Tag::String("textures".into())),
            ("value", Tag::String(texture.into())),
        ]);
        if profile.len() != 2 + usize::from(profile.contains_key(&"name".into()))
            || profile
                .get(&"name".into())
                .is_some_and(|name| name != &Tag::String("".into()))
            || !matches!(profile.get(&"id".into()), Some(Tag::IntArray(value)) if value.as_slice() == id)
            || !matches!(profile.get(&"properties".into()), Some(Tag::List(values)) if values.as_slice() == [property])
        {
            return false;
        }
    }
    match parts.get(&"minecraft:lore".into()) {
        None => lore.is_empty(),
        Some(Tag::List(saved)) if saved.len() == lore.len() => saved
            .iter()
            .zip(lore)
            .all(|(saved, expected)| plain_text(saved).as_deref() == Some(*expected)),
        _ => false,
    }
}

fn paper_channel_wool(material: &str, name: &str) -> bool {
    let Some(channel) = name
        .strip_prefix("Channel ID: ")
        .and_then(|number| number.parse::<usize>().ok())
    else {
        return false;
    };
    match channel.checked_sub(1).and_then(|index| COLORS.get(index)) {
        Some(color) => material == format!("minecraft:{color}_wool"),
        None => false,
    }
}

fn view(host: &dyn ServerHost, node: &Node, cargo: &CargoSettings) -> Vec<Option<ItemStack>> {
    let mut items = vec![
        icon(host, "minecraft:cyan_stained_glass_pane", " ", &[], None);
        if node.filters_items() { 54 } else { 27 }
    ];
    let channel = if node.filters_items() { 42 } else { 13 };
    items[channel - 1] = icon(
        host,
        "minecraft:player_head",
        "&bPrevious Channel",
        &["", "&e> Click to decrease the Channel ID by 1"],
        Some(PREVIOUS_CHANNEL_HEAD),
    );
    items[channel + 1] = icon(
        host,
        "minecraft:player_head",
        "&bNext Channel",
        &["", "&e> Click to increase the Channel ID by 1"],
        Some(NEXT_CHANNEL_HEAD),
    );
    let selected = usize::from(cargo.channel).min(COLORS.len());
    let name = format!("&bChannel ID: &3{}", selected + 1);
    items[channel] = match COLORS.get(selected) {
        Some(color) => icon(host, &format!("minecraft:{color}_wool"), &name, &[], None),
        None => icon(
            host,
            "minecraft:player_head",
            &name,
            &[],
            Some(TERMINAL_CHANNEL_HEAD),
        ),
    };
    if node.filters_items() {
        items[2] = icon(
            host,
            "minecraft:paper",
            "&3Items",
            &[
                "",
                "&bPut in all Items you want to",
                "&bblacklist/whitelist",
            ],
            None,
        );
        for slot in FILTER_SLOTS {
            items[slot] = cargo
                .filters
                .get(&slot)
                .and_then(|data| read_stack(data, |id| item_key(host, id)));
        }
        let (wool, name, toggle) = if cargo.blacklist {
            (
                "minecraft:black_wool",
                "&7Type: &8Blacklist",
                "&e> Click to change it to Whitelist",
            )
        } else {
            (
                "minecraft:white_wool",
                "&7Type: &rWhitelist",
                "&e> Click to change it to Blacklist",
            )
        };
        items[15] = icon(host, wool, name, &["", toggle], None);
        items[25] = icon(
            host,
            "minecraft:map",
            if cargo.lore {
                "&7Include Lore: &2✔"
            } else {
                "&7Include Lore: &4✘"
            },
            &["", "&e> Click to toggle whether the Lore has to match"],
            None,
        );
    }
    if node.component == Component::CargoInput {
        let (book, name, toggle) = if cargo.smart_fill {
            (
                "minecraft:written_book",
                "&7\"Smart-Filling\" Mode: &2✔",
                "&e> Click to disable \"Smart-Filling\" Mode",
            )
        } else {
            (
                "minecraft:writable_book",
                "&7\"Smart-Filling\" Mode: &4✘",
                "&e> Click to enable \"Smart-Filling\" Mode",
            )
        };
        items[16] = icon(
            host,
            book,
            name,
            &[
                "",
                toggle,
                "",
                "&fIn this mode, the Cargo node will attempt",
                "&fto keep a constant amount of items",
                "&fin the inventory. This is not perfect",
                "&fand will still fill in empty slots that",
                "&fcome before a stack of a configured item.",
            ],
            None,
        );
        let (name, toggle) = if cargo.round_robin {
            (
                "&7Round-Robin Mode: &2✔",
                "&e> Click to disable Round Robin Mode",
            )
        } else {
            (
                "&7Round-Robin Mode: &4✘",
                "&e> Click to enable Round Robin Mode",
            )
        };
        items[24] = icon(
            host,
            "minecraft:player_head",
            name,
            &[
                "",
                toggle,
                "&e(Items will be equally distributed on the Channel)",
            ],
            Some(ROUND_ROBIN_HEAD),
        );
    }
    items
}

#[cfg(test)]
mod paper_archive_tests {
    use super::*;
    use mod_api::{Identifier, testing::FakeHost};
    use std::collections::BTreeMap;

    fn button(
        host: &FakeHost,
        id: &str,
        name: &str,
        lore: &[&str],
        head: Option<PaperHead>,
    ) -> ItemStack {
        let id = Identifier::parse(id).unwrap();
        let mut stack = ItemStack::of((host as &dyn ServerHost).item_key(&id).unwrap());
        let mut parts =
            Compound::from([("minecraft:custom_name".into(), Tag::String(name.into()))]);
        if !lore.is_empty() {
            parts.insert(
                "minecraft:lore".into(),
                Tag::List(lore.iter().map(|&line| Tag::String(line.into())).collect()),
            );
        }
        if let Some((profile, texture)) = head {
            parts.insert(
                "minecraft:profile".into(),
                Tag::compound([
                    ("id", Tag::IntArray(profile.to_vec())),
                    (
                        "properties",
                        Tag::List(vec![Tag::compound([
                            ("name", Tag::String("textures".into())),
                            ("value", Tag::String(texture.into())),
                        ])]),
                    ),
                ]),
            );
        }
        stack.set_components(parts).unwrap();
        stack
    }

    #[test]
    fn cargo_menu_renders_paper_controls_without_rewriting_filter_items() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                [
                    "minecraft:cyan_stained_glass_pane",
                    "minecraft:paper",
                    "minecraft:player_head",
                    "minecraft:light_blue_wool",
                    "minecraft:white_wool",
                    "minecraft:black_wool",
                    "minecraft:map",
                    "minecraft:writable_book",
                    "minecraft:written_book",
                    "minecraft:iron_ingot",
                ]
                .map(|material| Identifier::parse(material).unwrap()),
            )
            .unwrap();
        let filter_parts = Compound::from([
            ("minecraft:custom_name".into(), colored_lore("&b纸服铁锭")),
            (
                "minecraft:lore".into(),
                Tag::List(vec![colored_lore("&7存档过滤样本")]),
            ),
        ]);
        let filter = Compound::from([
            ("id".into(), Tag::String("minecraft:iron_ingot".into())),
            ("count".into(), Tag::Int(7)),
            ("components".into(), Tag::Compound(filter_parts.clone())),
        ]);
        for component in [
            Component::CargoInput,
            Component::CargoOutput { advanced: true },
            Component::CargoOutput { advanced: false },
        ] {
            let node = Node {
                id: String::new(),
                component,
                channel: 16,
                passes_unfiltered: true,
            };
            let mut cargo = CargoSettings::legacy(16, true);
            cargo.filters.insert(19, filter.clone());
            let channel = if node.filters_items() { 42 } else { 13 };
            let mut controls = vec![channel - 1, channel, channel + 1];
            if node.filters_items() {
                controls.extend([15, 25]);
            }
            if component == Component::CargoInput {
                controls.extend([16, 24]);
            }
            for enabled in [false, true] {
                cargo.blacklist = !enabled;
                cargo.lore = enabled;
                cargo.round_robin = enabled;
                cargo.smart_fill = enabled;
                cargo.channel = if enabled { 3 } else { 16 };
                let menu = view(&host, &node, &cargo);
                assert_eq!(menu.len(), if node.filters_items() { 54 } else { 27 });
                for &slot in &controls {
                    assert!(
                        paper_control(&host, component, slot as u32, menu[slot].as_ref().unwrap()),
                        "nonstock cargo control in slot {slot}"
                    );
                }
                let parts =
                    |slot: usize| menu[slot].as_ref().unwrap().components().unwrap().as_nbt();
                let name = parts(channel);
                let channel_name = super::super::paper_text(&name[&"minecraft:custom_name".into()]);
                assert_eq!(
                    channel_name.as_deref(),
                    Some(if enabled {
                        "§bChannel ID: §34"
                    } else {
                        "§bChannel ID: §317"
                    })
                );
                let selector = parts(channel - 1);
                let Some(Tag::List(lore)) = selector.get(&"minecraft:lore".into()) else {
                    panic!("missing selector lore");
                };
                assert_eq!(
                    super::super::paper_text(&lore[1]).as_deref(),
                    Some("§e> Click to decrease the Channel ID by 1")
                );
                if node.filters_items() {
                    let name = parts(2);
                    assert_eq!(
                        super::super::paper_text(&name[&"minecraft:custom_name".into()]).as_deref(),
                        Some("§3Items")
                    );
                    assert_eq!(menu[19].as_ref().unwrap().count(), 7);
                    assert_eq!(parts(19), &filter_parts);
                    assert!(menu[20].is_none());
                } else {
                    assert_eq!(
                        menu[2].as_ref().unwrap().item().id(&host).as_str(),
                        "minecraft:cyan_stained_glass_pane"
                    );
                    assert_eq!(
                        menu[19].as_ref().unwrap().item().id(&host).as_str(),
                        "minecraft:cyan_stained_glass_pane"
                    );
                }
            }
        }
    }

    #[test]
    fn paper_cargo_buttons_are_removed_only_from_the_stock_control_slots() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                [
                    "minecraft:player_head",
                    "minecraft:yellow_wool",
                    "minecraft:red_wool",
                    "minecraft:white_wool",
                    "minecraft:black_wool",
                    "minecraft:map",
                    "minecraft:writable_book",
                    "minecraft:written_book",
                ]
                .map(|material| Identifier::parse(material).unwrap()),
            )
            .unwrap();
        let previous = button(
            &host,
            "minecraft:player_head",
            "§bPrevious Channel",
            &["", "§e> Click to decrease the Channel ID by 1"],
            Some(PREVIOUS_CHANNEL_HEAD),
        );
        let next = button(
            &host,
            "minecraft:player_head",
            "§bNext Channel",
            &["", "§e> Click to increase the Channel ID by 1"],
            Some(NEXT_CHANNEL_HEAD),
        );
        let channel = button(
            &host,
            "minecraft:yellow_wool",
            "§bChannel ID: §35",
            &[],
            None,
        );
        let output = Component::CargoOutput { advanced: false };
        let input = Component::CargoInput;
        let advanced = Component::CargoOutput { advanced: true };
        assert!(
            super::super::paper_cargo_filters(
                &host,
                output,
                BTreeMap::from([
                    (12, previous.clone()),
                    (13, channel.clone()),
                    (14, next.clone())
                ]),
            )
            .unwrap()
            .is_empty()
        );
        let terminal = button(
            &host,
            "minecraft:player_head",
            "§bChannel ID: §317",
            &[],
            Some(TERMINAL_CHANNEL_HEAD),
        );
        assert!(paper_control(&host, input, 42, &terminal));
        assert!(paper_control(&host, output, 13, &terminal));
        let mode = button(
            &host,
            "minecraft:white_wool",
            "§7Type: §rWhitelist",
            &["", "§e> Click to change it to Blacklist"],
            None,
        );
        let lore = button(
            &host,
            "minecraft:map",
            "§7Include Lore: §2✔",
            &["", "§e> Click to toggle whether the Lore has to match"],
            None,
        );
        for component in [input, advanced] {
            let slots = BTreeMap::from([
                (15, mode.clone()),
                (25, lore.clone()),
                // An item with the exact same name and profile in a real filter
                // position remains a player item, even if it resembles an icon.
                (19, previous.clone()),
                (41, previous.clone()),
                (42, channel.clone()),
                (43, next.clone()),
            ]);
            let kept = super::super::paper_cargo_filters(&host, component, slots).unwrap();
            assert_eq!(
                kept,
                BTreeMap::from([(19, previous.to_nbt(&host).unwrap())])
            );
        }
        let robin = button(
            &host,
            "minecraft:player_head",
            "§7Round-Robin Mode: §2✔",
            &[
                "",
                "§e> Click to disable Round Robin Mode",
                "§e(Items will be equally distributed on the Channel)",
            ],
            Some(ROUND_ROBIN_HEAD),
        );
        let mut smart_fill = button(
            &host,
            "minecraft:writable_book",
            "§7\"Smart-Filling\" Mode: §4✘",
            &[
                "",
                "§e> Click to enable \"Smart-Filling\" Mode",
                "",
                "§fIn this mode, the Cargo node will attempt",
                "§fto keep a constant amount of items",
                "§fin the inventory. This is not perfect",
                "§fand will still fill in empty slots that",
                "§fcome before a stack of a configured item.",
            ],
            None,
        );
        assert!(paper_control(&host, input, 24, &robin));
        assert!(paper_control(&host, input, 16, &smart_fill));

        // The real .sfi also stores the written-book default as a component.
        let mut book_parts = smart_fill.components().unwrap().as_nbt().clone();
        book_parts.insert(
            "minecraft:custom_name".into(),
            Tag::String("§7\"Smart-Filling\" Mode: §2✔".into()),
        );
        let Tag::List(lore) = book_parts.get_mut(&"minecraft:lore".into()).unwrap() else {
            panic!("stock Smart-Filling lore");
        };
        lore[1] = Tag::String("§e> Click to disable \"Smart-Filling\" Mode".into());
        book_parts.insert(
            "minecraft:written_book_content".into(),
            Tag::compound([
                ("author", Tag::String("".into())),
                ("title", Tag::compound([("raw", Tag::String("".into()))])),
            ]),
        );
        let written = Identifier::parse("minecraft:written_book").unwrap();
        smart_fill = ItemStack::of((&host as &dyn ServerHost).item_key(&written).unwrap());
        smart_fill.set_components(book_parts.clone()).unwrap();
        assert!(paper_control(&host, input, 16, &smart_fill));
        book_parts.insert(
            "minecraft:written_book_content".into(),
            Tag::compound([("author", Tag::String("a real author".into()))]),
        );
        smart_fill.set_components(book_parts).unwrap();
        assert!(!paper_control(&host, input, 16, &smart_fill));
        assert!(!paper_control(&host, advanced, 24, &robin));
        assert!(!paper_control(&host, advanced, 16, &smart_fill));
        assert!(
            super::super::paper_cargo_filters(
                &host,
                output,
                BTreeMap::from([(19, previous.clone())]),
            )
            .is_err()
        );
        assert!(!paper_control(&host, output, 14, &previous));
        let foreign = button(
            &host,
            "minecraft:player_head",
            "§bPrevious Channel",
            &["", "§e> Click to decrease the Channel ID by 1"],
            Some(NEXT_CHANNEL_HEAD),
        );
        assert!(!paper_control(&host, output, 12, &foreign));
        let wrong_channel = button(&mut host, "minecraft:red_wool", "Channel ID: 5", &[], None);
        assert!(!paper_control(&host, output, 13, &wrong_channel));
    }
}
