// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, sync::Mutex};

use mod_api::{
    BlockEntityRef, Compound, Host, ItemStack, LevelRef, MenuClickedEvent, MenuClosedEvent,
    MenuReplace, MenuSlotPlace, PlayerRef, ServerHost, SoundSource, Tag, Text,
};

use crate::{
    geology::{GPS_SETUP, GPS_TRANSMITTER},
    gps_waypoints::{self, Location, MAX_PLAYER_WAYPOINTS, Waypoint},
    items::{
        ADVANCED_CIRCUIT_BOARD, ALUMINUM_BRASS_INGOT, COBALT_INGOT, ELECTRO_MAGNET, HeadTexture,
        Kind, item, text,
    },
    network_runtime::Networks,
    recipes::{CraftingRecipe, Stack},
    research,
};

type Position = [i32; 3];
pub(crate) const VISIBLE: [usize; 21] = [
    19, 20, 21, 22, 23, 24, 25, 28, 29, 30, 31, 32, 33, 34, 37, 38, 39, 40, 41, 42, 43,
];

const CONTROL_TEXTURE: HeadTexture = HeadTexture {
    profile: [592909255, 967260779, -1631022274, -772334848],
    value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvZGRjZmJhNThmYWYxZjY0ODQ3ODg0MTExODIyYjY0YWZhMjFkN2ZjNjJkNDQ4MWYxNGYzZjNiY2I2MzMwIn19fQ==",
};
const OVERWORLD: HeadTexture = HeadTexture {
    profile: [2063506241, 1753561799, -1523982575, 787528304],
    value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2M5Yzg4ODFlNDI5MTVhOWQyOWJiNjFhMTZmYjI2ZDA1OTkxMzIwNGQyNjVkZjViNDM5YjNkNzkyYWNkNTYifX19",
};
const NETHER: HeadTexture = HeadTexture {
    profile: [-2007415551, 1599158715, -1779532905, 1534035434],
    value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2Q4MzU3MWZmNTg5ZjFhNTliYjAyYjgwODAwZmM3MzYxMTZlMjdjM2RjZjllZmViZWRlOGNmMWZkZGUifX19",
};
const END: HeadTexture = HeadTexture {
    profile: [-1404923294, 737820577, -1753257125, -141042882],
    value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2M2Y2FjNTliMmFhZTQ4OWFhMDY4N2I1ZDgwMmIyNTU1ZWIxNGE0MGJkNjJiMjFlYjExNmZhNTY5Y2RiNzU2In19fQ==",
};
const DEATH: HeadTexture = HeadTexture {
    profile: [-466508919, -1212858844, -1368932872, -1793809221],
    value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzFhZTM4NTVmOTUyY2Q0YTAzYzE0OGE5NDZlM2Y4MTJhNTk1NWFkMzVjYmNiNTI2MjdlYTRhY2Q0N2QzMDgxIn19fQ==",
};

item!(
    GPS_CONTROL_PANEL,
    "player_head",
    "GPS Control Panel",
    "aqua",
    None,
    Some(CONTROL_TEXTURE)
);

pub(crate) const CRAFTING: CraftingRecipe = CraftingRecipe {
    inputs: [
        None,
        None,
        Some(ELECTRO_MAGNET),
        Some(COBALT_INGOT),
        Some(ADVANCED_CIRCUIT_BOARD),
        Some(COBALT_INGOT),
        Some(ALUMINUM_BRASS_INGOT),
        Some(ALUMINUM_BRASS_INGOT),
        Some(ALUMINUM_BRASS_INGOT),
    ],
    output: Stack {
        kind: GPS_CONTROL_PANEL,
        count: 1,
    },
    research: &GPS_SETUP,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Transmitters,
    Waypoints,
}

#[derive(Clone, Copy)]
struct Anchor {
    level: LevelRef,
    pos: Position,
    entity: BlockEntityRef,
}

#[derive(Clone)]
struct View {
    window: i32,
    anchor: Anchor,
    tab: Tab,
    entries: BTreeMap<usize, (String, Waypoint)>,
}

#[derive(Default)]
pub(crate) struct Controls {
    naming: Mutex<BTreeMap<u128, Location>>,
    views: Mutex<BTreeMap<u128, View>>,
}

impl Controls {
    pub fn start_waypoint(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        level: LevelRef,
        pos: Position,
        face: Position,
    ) {
        if !known(host, player) {
            return;
        }
        match gps_waypoints::load(host, player.uuid()) {
            Ok(waypoints) if waypoints.entries.len() >= MAX_PLAYER_WAYPOINTS => {
                tell(host, player, "Your GPS network already has 20 waypoints.");
                return;
            }
            Err(error) => {
                tell(host, player, error);
                return;
            }
            _ => {}
        }
        let Some(dimension) = level.dimension().id(host) else {
            return;
        };
        let Some([yaw, _]) = player.entity().rotation(host) else {
            return;
        };
        if !face.iter().all(|part| (-1..=1).contains(part))
            || face.iter().map(|part| part.abs()).sum::<i32>() != 1
        {
            return;
        }
        let location = Location {
            dimension,
            position: std::array::from_fn(|axis| f64::from(pos[axis]) + f64::from(face[axis])),
            rotation: [yaw, 0.0],
        };
        self.naming.lock().unwrap().insert(player.uuid(), location);
        tell(host, player, "Type the new waypoint name in chat.");
        let _ = player.play_sound(
            host,
            "minecraft:block.note_block.pling",
            SoundSource::Master,
            0.5,
            1.0,
        );
    }

    pub fn take_chat(&self, host: &mut dyn ServerHost, uuid: u128, name: &str) -> Option<Text> {
        let location = self.naming.lock().unwrap().remove(&uuid)?;
        let player = host.player(uuid)?;
        if !research::can_use(host, player, &GPS_SETUP) {
            return Some(Text::literal("Unlock Basic GPS Setup to create waypoints."));
        }
        let result = gps_waypoints::add(host, uuid, name, location);
        match result {
            Ok(()) => {
                let _ = player.play_sound(
                    host,
                    "minecraft:block.note_block.pling",
                    SoundSource::Master,
                    1.0,
                    1.0,
                );
                Some(Text::literal(format!("Created GPS waypoint {name}.")))
            }
            Err(error) => Some(Text::literal(error)),
        }
    }

    pub fn use_panel(
        &self,
        networks: &Mutex<Networks>,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        level: LevelRef,
        pos: Position,
    ) -> bool {
        if !crate::network_runtime::gps_panel_at(host, level, pos) {
            return false;
        }
        let Some(entity) = host.block_entity(level, pos) else {
            return true;
        };
        if known(host, player) {
            self.open_panel(
                networks,
                host,
                player,
                Anchor { level, pos, entity },
                Tab::Transmitters,
            );
        }
        true
    }

    fn open_panel(
        &self,
        networks: &Mutex<Networks>,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        anchor: Anchor,
        tab: Tab,
    ) {
        let signals = networks.lock().unwrap().gps_network(player.uuid());
        let complexity = signals
            .iter()
            .fold(0u32, |sum, (_, _, strength)| sum.saturating_add(*strength));
        let status = if complexity > 0 { "ONLINE" } else { "OFFLINE" };
        let background = icon(
            host,
            Kind::Vanilla("minecraft:gray_stained_glass_pane"),
            " ",
            &[],
        );
        let mut items = vec![background; 54];
        for slot in VISIBLE {
            items[slot] = None;
        }
        items[2] = icon(host, GPS_TRANSMITTER, "Transmitters", &[]);
        items[4] = icon(
            host,
            GPS_CONTROL_PANEL,
            "Network Info",
            &[
                format!("Status: {status}"),
                format!("Complexity: {complexity}"),
            ],
        );
        items[6] = head_icon(host, &OVERWORLD, "Waypoints", &[]);
        let mut entries = BTreeMap::new();
        match tab {
            Tab::Transmitters => {
                for (slot, (dim, [x, y, z], strength)) in VISIBLE.into_iter().zip(signals) {
                    items[slot] = icon(
                        host,
                        GPS_TRANSMITTER,
                        "GPS Transmitter",
                        &[
                            format!("World: {dim}"),
                            format!("X: {x}"),
                            format!("Y: {y}"),
                            format!("Z: {z}"),
                            format!("Signal Strength: {strength}"),
                            format!("Ping: {:.2}ms", 1000.0 / f64::from(y)),
                        ],
                    );
                }
            }
            Tab::Waypoints => match gps_waypoints::load(host, player.uuid()) {
                Ok(points) => {
                    for (slot, (id, point)) in VISIBLE.into_iter().zip(points.entries) {
                        let mut lore = location_lines(&point.location);
                        lore.push("Click to delete".into());
                        items[slot] = waypoint_icon(host, &point, &lore);
                        entries.insert(slot, (id, point));
                    }
                }
                Err(error) => {
                    tell(host, player, error);
                    return;
                }
            },
        }
        let mut title = Text::literal("GPS Control Panel");
        title.style.color = Some("blue".into());
        if let Ok(Some(window)) = host.open_block_inventory_menu(
            player,
            anchor.entity,
            6,
            title,
            items,
            MenuReplace::CloseFirst,
        ) {
            if player
                .back_mod_menu_slots(host, window, vec![MenuSlotPlace::Decoration; 54])
                .is_err()
            {
                host.close_menu(player.uuid());
                return;
            }
            self.views.lock().unwrap().insert(
                player.uuid(),
                View {
                    window,
                    anchor,
                    tab,
                    entries,
                },
            );
        }
    }

    pub fn click(
        &self,
        networks: &Mutex<Networks>,
        host: &mut dyn ServerHost,
        event: &MenuClickedEvent,
    ) {
        if event.mode != 0 || !matches!(event.button, 0 | 1) {
            return;
        }
        let Some(view) = self
            .views
            .lock()
            .unwrap()
            .get(&event.uuid)
            .cloned()
            .filter(|view| view.window == event.window)
        else {
            return;
        };
        let Some(player) = host.player(event.uuid) else {
            return;
        };
        let anchor = view.anchor;
        if player.entity().level(host) != Some(anchor.level)
            || host.block_entity(anchor.level, anchor.pos) != Some(anchor.entity)
            || !crate::network_runtime::gps_panel_at(host, anchor.level, anchor.pos)
            || !known(host, player)
        {
            host.close_menu(event.uuid);
            return;
        }

        let tab = match (view.tab, event.slot) {
            (Tab::Transmitters, 6) => Tab::Waypoints,
            (Tab::Waypoints, 2) => Tab::Transmitters,
            (Tab::Waypoints, slot)
                if let Some((id, selected)) = view.entries.get(&(slot as usize)) =>
            {
                let removed = (|| {
                    let mut points = gps_waypoints::load(host, event.uuid)?;
                    if points.entries.get(id) == Some(selected) {
                        points.entries.remove(id);
                        gps_waypoints::save(host, event.uuid, &points)?;
                        let _ = player.play_sound(
                            host,
                            "minecraft:ui.button.click",
                            SoundSource::Master,
                            1.0,
                            1.0,
                        );
                    }
                    Ok::<_, String>(())
                })();
                if let Err(error) = removed {
                    tell(host, player, error);
                }
                Tab::Waypoints
            }
            _ => return,
        };
        self.open_panel(networks, host, player, anchor, tab);
    }

    pub fn close(&self, event: &MenuClosedEvent) {
        let mut views = self.views.lock().unwrap();
        if views
            .get(&event.uuid)
            .is_some_and(|view| view.window == event.window)
        {
            views.remove(&event.uuid);
        }
    }

    pub fn logout(&self, uuid: u128) {
        self.naming.lock().unwrap().remove(&uuid);
        self.views.lock().unwrap().remove(&uuid);
    }

    pub fn clear(&self) {
        self.naming.lock().unwrap().clear();
        self.views.lock().unwrap().clear();
    }
}

fn known(host: &mut dyn ServerHost, player: PlayerRef) -> bool {
    if !research::can_use(host, player, &GPS_SETUP) {
        tell(
            host,
            player,
            "Unlock Basic GPS Setup with /sf research gps_setup.",
        );
        return false;
    }
    true
}

fn tell(host: &mut dyn ServerHost, player: PlayerRef, message: impl Into<String>) {
    let _ = player.tell(host, Text::literal(message.into()));
}

pub(crate) fn location_lines(location: &Location) -> Vec<String> {
    let [x, y, z] = location.position;
    vec![
        format!("World: {}", location.dimension),
        format!("X: {x}"),
        format!("Y: {y}"),
        format!("Z: {z}"),
    ]
}

pub(crate) fn waypoint_icon(
    host: &dyn Host,
    waypoint: &Waypoint,
    lore: &[String],
) -> Option<ItemStack> {
    let (name, texture) = if let Some(name) = waypoint.name.strip_prefix("player:death ") {
        (name, &DEATH)
    } else {
        let texture = match waypoint.location.dimension.as_str() {
            "minecraft:the_nether" => &NETHER,
            "minecraft:the_end" => &END,
            _ => &OVERWORLD,
        };
        (waypoint.name.as_str(), texture)
    };
    head_icon(host, texture, name, lore)
}

pub(crate) fn world_icon(host: &dyn Host, name: &str) -> Option<ItemStack> {
    head_icon(host, &OVERWORLD, name, &[])
}

pub(crate) fn head_icon(
    host: &dyn Host,
    texture: &HeadTexture,
    name: &str,
    lore: &[String],
) -> Option<ItemStack> {
    let mut icon = icon(host, Kind::Vanilla("minecraft:player_head"), name, lore)?;
    icon.apply_components(&Compound::from([(
        "minecraft:profile".into(),
        Tag::compound([
            ("id", Tag::IntArray(texture.profile.to_vec())),
            (
                "properties",
                Tag::List(vec![Tag::compound([
                    ("name", Tag::String("textures".into())),
                    ("value", Tag::String(texture.value.into())),
                ])]),
            ),
        ]),
    )]))
    .ok()?;
    Some(icon)
}

pub(crate) fn icon(host: &dyn Host, kind: Kind, name: &str, lore: &[String]) -> Option<ItemStack> {
    let mut icon = kind.stack(host, 1)?;
    icon.apply_components(&Compound::from([
        ("minecraft:custom_name".into(), text(name, "gray", false)),
        (
            "minecraft:lore".into(),
            Tag::List(lore.iter().map(|line| text(line, "gray", false)).collect()),
        ),
    ]))
    .ok()?;
    Some(icon)
}
