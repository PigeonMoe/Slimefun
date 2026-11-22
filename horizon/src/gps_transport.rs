// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::BTreeMap,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicU32, Ordering},
    },
};

use mod_api::{
    GameMode, Hand, Host, HostTransaction, ItemPlace, ItemResource, LevelRef, MenuClickedEvent,
    MenuClosedEvent, MenuReplace, MenuSlotPlace, ParticleKey, ParticleOptions, PlayerRef,
    PlayerTeleportStatus, Registrar, ServerHost, SoundSource, Text,
};

use crate::{
    factory::charging_bench,
    geology::{GPS_TRANSMITTER, GPS_TRANSMITTER_3},
    gps_controls::{self, GPS_CONTROL_PANEL, VISIBLE},
    gps_waypoints::{self, Location, Waypoint},
    items::{
        BILLON_INGOT, COBALT_INGOT, ELECTRO_MAGNET, HEATING_COIL, Kind, LEAD_INGOT,
        REINFORCED_ALLOY_INGOT, REINFORCED_PLATE, ZINC_INGOT, item,
    },
    network_recipes::{BLISTERING_INGOT_3, MEDIUM_CAPACITOR},
    network_runtime::{self, Networks},
    recipes::{CraftingRecipe, Research, Stack},
    research,
};

type Position = [i32; 3];

item!(
    GPS_TELEPORTER_PYLON,
    "purple_stained_glass",
    "GPS Teleporter Pylon",
    "dark_purple",
    None,
    None
);
item!(
    GPS_TELEPORTATION_MATRIX,
    "iron_block",
    "GPS Teleporter Matrix",
    "aqua",
    None,
    None
);
item!(
    GPS_ACTIVATION_DEVICE_SHARED,
    "stone_pressure_plate",
    "GPS Activation Device (Shared)",
    "white",
    None,
    None
);
item!(
    GPS_ACTIVATION_DEVICE_PERSONAL,
    "stone_pressure_plate",
    "GPS Activation Device (Personal)",
    "white",
    None,
    None
);
item!(
    PORTABLE_TELEPORTER,
    "compass",
    "Portable Teleporter",
    "aqua",
    None,
    None
);

pub(crate) const CARRIERS: &[&str] = &[
    "minecraft:purple_stained_glass",
    "minecraft:stone_pressure_plate",
];
const PLATE_RESEARCH: Research = Research {
    id: "teleporter_activation_plates",
    name: "Teleporter Activation",
    cost: 36,
};
const PORTABLE_RESEARCH: Research = Research {
    id: "portable_teleporter",
    name: "Teleportation from Anywhere",
    cost: 42,
};
const TELEPORTER_RESEARCH: Research = Research {
    id: "teleporter",
    name: "Teleporter Base Components",
    cost: 42,
};
pub(crate) const RESEARCHES: &[Research] =
    &[TELEPORTER_RESEARCH, PLATE_RESEARCH, PORTABLE_RESEARCH];

const GLASS: Kind = Kind::Vanilla("minecraft:glass");
const ALL_TYPES: &[Kind] = &[
    GPS_TELEPORTER_PYLON,
    GPS_TELEPORTATION_MATRIX,
    GPS_ACTIVATION_DEVICE_SHARED,
    GPS_ACTIVATION_DEVICE_PERSONAL,
    crate::elevator::ELEVATOR_PLATE,
    PORTABLE_TELEPORTER,
];
pub(crate) fn item(id: &str) -> Option<Kind> {
    ALL_TYPES.iter().copied().find(|kind| kind.id() == id)
}

pub(crate) fn fixed(id: &str) -> Option<Kind> {
    item(id).filter(|kind| *kind != PORTABLE_TELEPORTER)
}

pub(crate) fn needs_owner(id: &str) -> bool {
    matches!(
        id,
        "GPS_TELEPORTATION_MATRIX" | "GPS_ACTIVATION_DEVICE_PERSONAL" | "ELEVATOR_PLATE"
    )
}

const fn craft(
    kind: Kind,
    count: u32,
    research: &'static Research,
    inputs: [Option<Kind>; 9],
) -> CraftingRecipe {
    CraftingRecipe {
        inputs,
        output: Stack { kind, count },
        research,
    }
}

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    craft(
        GPS_TELEPORTER_PYLON,
        8,
        &TELEPORTER_RESEARCH,
        [
            Some(ZINC_INGOT),
            Some(GLASS),
            Some(ZINC_INGOT),
            Some(GLASS),
            Some(HEATING_COIL),
            Some(GLASS),
            Some(ZINC_INGOT),
            Some(GLASS),
            Some(ZINC_INGOT),
        ],
    ),
    craft(
        GPS_TELEPORTATION_MATRIX,
        1,
        &TELEPORTER_RESEARCH,
        [
            Some(GPS_TELEPORTER_PYLON),
            Some(REINFORCED_ALLOY_INGOT),
            Some(GPS_TELEPORTER_PYLON),
            Some(ELECTRO_MAGNET),
            Some(GPS_CONTROL_PANEL),
            Some(ELECTRO_MAGNET),
            Some(GPS_TELEPORTER_PYLON),
            Some(REINFORCED_ALLOY_INGOT),
            Some(GPS_TELEPORTER_PYLON),
        ],
    ),
    craft(
        GPS_ACTIVATION_DEVICE_SHARED,
        1,
        &PLATE_RESEARCH,
        [
            None,
            Some(Kind::Vanilla("minecraft:stone_pressure_plate")),
            None,
            Some(Kind::Vanilla("minecraft:redstone")),
            Some(GPS_TRANSMITTER),
            None,
            Some(BILLON_INGOT),
            Some(BILLON_INGOT),
            Some(BILLON_INGOT),
        ],
    ),
    craft(
        GPS_ACTIVATION_DEVICE_PERSONAL,
        1,
        &PLATE_RESEARCH,
        [
            None,
            Some(LEAD_INGOT),
            None,
            Some(COBALT_INGOT),
            Some(GPS_ACTIVATION_DEVICE_SHARED),
            Some(COBALT_INGOT),
            None,
            Some(LEAD_INGOT),
            None,
        ],
    ),
    craft(
        PORTABLE_TELEPORTER,
        1,
        &PORTABLE_RESEARCH,
        [
            Some(ELECTRO_MAGNET),
            Some(GPS_TRANSMITTER_3),
            Some(ELECTRO_MAGNET),
            Some(REINFORCED_PLATE),
            Some(GPS_TELEPORTATION_MATRIX),
            Some(REINFORCED_PLATE),
            Some(BLISTERING_INGOT_3),
            Some(MEDIUM_CAPACITOR),
            Some(BLISTERING_INGOT_3),
        ],
    ),
];

static PORTAL: OnceLock<ParticleKey> = OnceLock::new();
const INTERVAL: i64 = 10;
const ARRIVAL_TIMEOUT: i64 = 1_200;
const ADMIN_COMPLEXITY: u32 = 999_999_999;

pub(crate) fn register(registrar: &mut dyn Registrar) {
    let _ = PORTAL.set(registrar.particle_key("minecraft:portal"));
}

#[derive(Clone)]
struct View {
    window: i32,
    owner: u128,
    level: LevelRef,
    dimension: String,
    source: [f64; 3],
    complexity: u32,
    entries: BTreeMap<usize, (String, Waypoint)>,
}

#[derive(Clone)]
struct Journey {
    location: Location,
    origin: LevelRef,
    source: [f64; 3],
    progress: u32,
    step: u32,
    due: i64,
    waiting: bool,
}

pub(crate) struct Transport {
    cost: AtomicU32,
    viewers: Mutex<BTreeMap<u128, View>>,
    journeys: Mutex<BTreeMap<u128, Journey>>,
    activations: Mutex<BTreeMap<(String, u128), Position>>,
    standing: Mutex<BTreeMap<(String, u128), Position>>,
}

impl Default for Transport {
    fn default() -> Self {
        Self {
            cost: AtomicU32::new(10),
            viewers: Mutex::new(BTreeMap::new()),
            journeys: Mutex::new(BTreeMap::new()),
            activations: Mutex::new(BTreeMap::new()),
            standing: Mutex::new(BTreeMap::new()),
        }
    }
}

impl Transport {
    pub fn configure(&self, table: &toml::Table) {
        let cost = table
            .get("portable-teleporter")
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("teleportation-cost"))
            .and_then(toml::Value::as_integer)
            .filter(|value| (0..=50).contains(value))
            .unwrap_or(10);
        self.cost.store(cost as u32, Ordering::Relaxed);
    }

    pub fn use_portable(
        &self,
        networks: &Mutex<Networks>,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        hand: Hand,
    ) -> bool {
        let held = ItemPlace::PlayerHand {
            player: player.uuid(),
            hand,
        };
        let Some(stack) = held
            .stack(host)
            .filter(|stack| PORTABLE_TELEPORTER.matches(host, stack))
        else {
            return false;
        };
        if !research::can_use(host, player, &PORTABLE_RESEARCH) {
            tell(
                host,
                player,
                "Unlock Teleportation from Anywhere to use the Portable Teleporter.",
            );
            return true;
        }
        let Some(level) = player.entity().level(host) else {
            return true;
        };
        let Some([x, y, z]) = player.entity().block_position(host) else {
            return true;
        };
        let cost = self.cost.load(Ordering::Relaxed);
        let Some(replacement) = charging_bench::discharge(host, &stack, cost as f32) else {
            tell(host, player, "The Portable Teleporter needs more charge.");
            return true;
        };
        let paid = host.transaction(|host, tx| {
            if held.extract(host, tx, &ItemResource::of(&stack), 1) != 1
                || held.insert(host, tx, &ItemResource::of(&replacement), 1) != 1
            {
                return Err(());
            }
            Ok(())
        });
        if paid.is_ok() {
            // Paper takes the charge before opening a portable menu.
            let complexity = networks.lock().unwrap().gps_complexity(player.uuid());
            self.open(
                host,
                player,
                level,
                player.uuid(),
                [x, y - 1, z],
                None,
                complexity,
            );
        }
        true
    }

    pub fn step(&self, dimension: &str, player: u128, pos: Position) {
        let key = (dimension.to_owned(), player);
        if self.standing.lock().unwrap().get(&key) == Some(&pos) {
            return;
        }
        self.activations.lock().unwrap().entry(key).or_insert(pos);
    }

    pub(crate) fn open_admin(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        owner: u128,
    ) -> Result<(), &'static str> {
        let level = player
            .entity()
            .level(host)
            .ok_or("无法读取你所在的世界。")?;
        let [x, y, z] = player
            .entity()
            .block_position(host)
            .ok_or("无法读取你的当前位置。")?;
        let below = y.checked_sub(1).ok_or("无法读取你的当前位置。")?;
        self.open(
            host,
            player,
            level,
            owner,
            [x, below, z],
            None,
            ADMIN_COMPLEXITY,
        );
        Ok(())
    }

    fn open(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        level: LevelRef,
        owner: u128,
        origin: Position,
        plate: Option<Position>,
        complexity: u32,
    ) {
        let uuid = player.uuid();
        if self.journeys.lock().unwrap().contains_key(&uuid)
            || self.viewers.lock().unwrap().contains_key(&uuid)
        {
            return;
        }
        let Some(dimension) = level.dimension().id(host) else {
            return;
        };
        let source = [
            f64::from(origin[0]) + 0.5,
            f64::from(origin[1]) + 2.0,
            f64::from(origin[2]) + 0.5,
        ];
        let points = match gps_waypoints::import(host, owner)
            .and_then(|()| gps_waypoints::load(host, owner))
        {
            Ok(waypoints) => waypoints,
            Err(error) => {
                tell(host, player, error);
                return;
            }
        };
        let background = gps_controls::icon(
            host,
            Kind::Vanilla("minecraft:gray_stained_glass_pane"),
            " ",
            &[],
        );
        let mut items = vec![background; 54];
        for slot in VISIBLE {
            items[slot] = None;
        }
        items[4] = gps_controls::world_icon(host, "&e你的传送点");
        let mut entries = BTreeMap::new();
        for (slot, (id, waypoint)) in VISIBLE.into_iter().zip(points.entries) {
            let time = time_steps(complexity, source, &dimension, &waypoint.location);
            let [x, y, z] = waypoint.location.position;
            let lore = [
                String::new(),
                format!("&8⇨ &7世界：&f{}", waypoint.location.dimension),
                format!("&8⇨ &7X: &f{x:?}"),
                format!("&8⇨ &7Y: &f{y:?}"),
                format!("&8⇨ &7Z: &f{z:?}"),
                format!("&8⇨ &7预计时间：&f{:.1}s", f64::from(time) / 2.0),
                String::new(),
                "&8⇨ &c单击传送".into(),
            ];
            items[slot] = gps_controls::waypoint_icon(host, &waypoint, &lore);
            entries.insert(slot, (id, waypoint));
        }

        let mut title = Text::literal("传送器");
        title.style.color = Some("dark_aqua".into());
        let opened = match plate {
            Some(plate) => host.open_inventory_menu_at(
                player,
                level,
                plate,
                6,
                title,
                items,
                MenuReplace::CloseFirst,
            ),
            None => host.open_menu(uuid, 6, title, items, MenuReplace::CloseFirst),
        };
        if let Ok(Some(window)) = opened {
            if player
                .back_mod_menu_slots(host, window, vec![MenuSlotPlace::Decoration; 54])
                .is_err()
            {
                host.close_menu(uuid);
                return;
            }
            self.viewers.lock().unwrap().insert(
                uuid,
                View {
                    window,
                    owner,
                    level,
                    dimension,
                    source,
                    complexity,
                    entries,
                },
            );
            play(host, player, "minecraft:ui.button.click", 1.0);
        }
    }

    pub fn clicked(&self, host: &mut dyn ServerHost, event: &MenuClickedEvent) {
        if event.mode != 0 || !matches!(event.button, 0 | 1) {
            return;
        }
        let Some(view) = self
            .viewers
            .lock()
            .unwrap()
            .get(&event.uuid)
            .cloned()
            .filter(|view| view.window == event.window)
        else {
            return;
        };
        let Some((id, point)) = view.entries.get(&(event.slot as usize)) else {
            return;
        };
        let Some(player) = host.player(event.uuid) else {
            return;
        };
        let Some(now) = view.level.game_time(host) else {
            return;
        };
        if !stays_near(host, player, view.level, view.source) {
            host.close_menu(event.uuid);
            cancelled(host, player);
            return;
        }
        if !gps_waypoints::load(host, view.owner)
            .is_ok_and(|points| points.entries.get(id) == Some(point))
        {
            host.close_menu(event.uuid);
            tell(host, player, "That GPS waypoint has changed.");
            return;
        }
        let time = time_steps(
            view.complexity,
            view.source,
            &view.dimension,
            &point.location,
        );
        let journey = Journey {
            location: point.location.clone(),
            origin: view.level,
            source: view.source,
            progress: 0,
            step: (100 / time).max(1),
            due: now.saturating_add(INTERVAL),
            waiting: false,
        };
        self.journeys.lock().unwrap().insert(event.uuid, journey);
        host.close_menu(event.uuid);
        progress(host, player, view.level, view.source, 0);
    }

    pub fn tick(&self, networks: &Mutex<Networks>, host: &mut dyn ServerHost, level: LevelRef) {
        let Some(now) = level.game_time(host) else {
            return;
        };
        let Some(dimension) = level.dimension().id(host) else {
            return;
        };
        self.standing
            .lock()
            .unwrap()
            .retain(|(dim, uuid), &mut plate| {
                dim != &dimension
                    || host
                        .player(*uuid)
                        .is_some_and(|player| on_plate(host, player, level, plate))
            });
        let triggers = {
            let mut queued = self.activations.lock().unwrap();
            let triggers: Vec<_> = queued
                .iter()
                .filter_map(|((dim, uuid), &pos)| (dim == &dimension).then_some((*uuid, pos)))
                .collect();
            queued.retain(|(dim, _), _| dim != &dimension);
            triggers
        };
        for (uuid, plate) in triggers {
            let Some(player) = host
                .player(uuid)
                .filter(|player| on_plate(host, *player, level, plate))
            else {
                continue;
            };
            let Some((owner, origin)) = self.activate(host, player, level, plate) else {
                continue;
            };
            self.standing
                .lock()
                .unwrap()
                .insert((dimension.clone(), uuid), plate);
            let complexity = networks.lock().unwrap().gps_complexity(owner);
            self.open(host, player, level, owner, origin, Some(plate), complexity);
        }

        let due: Vec<_> = self
            .journeys
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, journey)| journey.origin == level && journey.due <= now)
            .map(|(&uuid, _)| uuid)
            .collect();
        for uuid in due {
            let Some(mut journey) = self.journeys.lock().unwrap().remove(&uuid) else {
                continue;
            };
            let Some(player) = host.player(uuid) else {
                continue;
            };
            if journey.waiting || !stays_near(host, player, journey.origin, journey.source) {
                cancelled(host, player);
                continue;
            }
            journey.progress += journey.step;
            if journey.progress < 100 {
                progress(host, player, level, journey.source, journey.progress);
                journey.due = now.saturating_add(INTERVAL);
                self.journeys.lock().unwrap().insert(uuid, journey);
                continue;
            }
            let destination = &journey.location;
            match player.teleport_checked(
                host,
                &destination.dimension,
                destination.position,
                Some(destination.rotation),
            ) {
                Ok(PlayerTeleportStatus::Moved) => {
                    arrived(host, player, level, destination.position)
                }
                Ok(PlayerTeleportStatus::Queued) => {
                    journey.waiting = true;
                    journey.due = now.saturating_add(ARRIVAL_TIMEOUT);
                    self.journeys.lock().unwrap().insert(uuid, journey);
                }
                _ => cancelled(host, player),
            }
        }
    }

    fn activate(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        level: LevelRef,
        plate: Position,
    ) -> Option<(u128, Position)> {
        if !matches!(
            player.game_mode(host),
            Some(GameMode::Survival | GameMode::Creative | GameMode::Adventure)
        ) || !research::can_use(host, player, &PLATE_RESEARCH)
        {
            return None;
        }
        let (kind, owner) = network_runtime::gps_device_at(host, level, plate)?;
        if kind != GPS_ACTIVATION_DEVICE_SHARED
            && (kind != GPS_ACTIVATION_DEVICE_PERSONAL || owner != Some(player.uuid()))
        {
            return None;
        }
        let [x, y, z] = plate;
        let origin = [x, y.checked_sub(1)?, z];
        let (matrix, owner) = network_runtime::gps_device_at(host, level, origin)?;
        if matrix != GPS_TELEPORTATION_MATRIX {
            return None;
        }
        if !pylons(origin).all(|at| {
            network_runtime::gps_device_at(host, level, at)
                .is_some_and(|(kind, _)| kind == GPS_TELEPORTER_PYLON)
        }) {
            return None;
        }
        Some((owner?, origin))
    }

    pub fn teleported(
        &self,
        host: &mut dyn ServerHost,
        uuid: u128,
        dimension: &str,
        position: [f64; 3],
    ) {
        let mut journeys = self.journeys.lock().unwrap();
        if !journeys.get(&uuid).is_some_and(|journey| {
            journey.waiting
                && journey.location.dimension == dimension
                && journey.location.position == position
        }) {
            return;
        }
        journeys.remove(&uuid);
        drop(journeys);
        if let Some(player) = host.player(uuid)
            && let Some(level) = player.entity().level(host)
        {
            arrived(host, player, level, position);
        }
    }

    pub fn close(&self, event: &MenuClosedEvent) {
        let mut viewers = self.viewers.lock().unwrap();
        if viewers
            .get(&event.uuid)
            .is_some_and(|view| view.window == event.window)
        {
            viewers.remove(&event.uuid);
        }
    }

    pub fn logout(&self, player: u128) {
        self.viewers.lock().unwrap().remove(&player);
        self.journeys.lock().unwrap().remove(&player);
        self.activations
            .lock()
            .unwrap()
            .retain(|(_, uuid), _| *uuid != player);
        self.standing
            .lock()
            .unwrap()
            .retain(|(_, uuid), _| *uuid != player);
    }

    pub fn clear(&self) {
        self.viewers.lock().unwrap().clear();
        self.journeys.lock().unwrap().clear();
        self.activations.lock().unwrap().clear();
        self.standing.lock().unwrap().clear();
    }
}

fn distance_squared(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| (a - b).powi(2)).sum()
}

fn time_steps(complexity: u32, source: [f64; 3], dimension: &str, destination: &Location) -> u32 {
    if complexity < 100 {
        return 100;
    }
    let squared = if destination.dimension == dimension {
        (distance_squared(source, destination.position) as u32).min(100_000_000)
    } else {
        150_000_000
    };
    let speed = 50_000 + u128::from(complexity).pow(2);
    (4 * u128::from(squared) / speed).clamp(1, 40) as u32
}

fn stays_near(host: &dyn Host, player: PlayerRef, level: LevelRef, source: [f64; 3]) -> bool {
    player.entity().level(host) == Some(level)
        && host
            .living(player.entity())
            .and_then(|living| living.health(host))
            .is_some_and(|health| health > 0.0)
        && player
            .entity()
            .position(host)
            .is_some_and(|position| distance_squared(source, position) < 2.0)
}

fn on_plate(host: &dyn Host, player: PlayerRef, level: LevelRef, plate: Position) -> bool {
    player.entity().level(host) == Some(level)
        && player.entity().block_position(host) == Some(plate)
}

fn pylons([x, y, z]: Position) -> impl Iterator<Item = Position> {
    (-1..=1).flat_map(move |dx| {
        (-1..=1).filter_map(move |dz| {
            if dx == 0 && dz == 0 {
                return None;
            }
            Some([x.checked_add(dx)?, y, z.checked_add(dz)?])
        })
    })
}

fn progress(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    level: LevelRef,
    at: [f64; 3],
    progress: u32,
) {
    title(
        host,
        player,
        "传送中...",
        format!("{progress}%"),
        [0, 60, 0],
    );
    particles(host, level, at, (progress * 2) as i32);
    play(host, player, "minecraft:block.beacon.ambient", 0.6);
}

fn arrived(host: &mut dyn ServerHost, player: PlayerRef, level: LevelRef, at: [f64; 3]) {
    title(host, player, "传送成功!", "100%", [20, 60, 20]);
    particles(host, level, [at[0], at[1] + 1.0, at[2]], 200);
    play(host, player, "minecraft:block.beacon.activate", 1.0);
}

fn cancelled(host: &mut dyn ServerHost, player: PlayerRef) {
    title(host, player, "传送被取消!", "40%", [20, 60, 20]);
}

fn particles(host: &mut dyn ServerHost, level: LevelRef, at: [f64; 3], count: i32) {
    if let Some(&particle) = PORTAL.get() {
        level.send_particles(
            host,
            &ParticleOptions::simple(particle),
            None,
            at,
            [0.2, 0.8, 0.2],
            0.0,
            count,
        );
    }
}

fn title(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    title: &str,
    subtitle: impl Into<String>,
    times: [i32; 3],
) {
    let title = Text::literal(title);
    let mut subtitle = Text::literal(subtitle.into());
    subtitle.style.color = Some("aqua".into());
    let _ = player.title(host, &title, &subtitle, times);
}

fn tell(host: &mut dyn ServerHost, player: PlayerRef, message: impl Into<String>) {
    let _ = player.tell(host, Text::literal(message.into()));
}

fn play(host: &mut dyn ServerHost, player: PlayerRef, event: &str, pitch: f32) {
    let _ = player.play_sound(host, event, SoundSource::Master, 1.0, pitch);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paper_progress_uses_500_ms_steps_with_distance_and_complexity() {
        let source = [0.0; 3];
        let mut destination = Location {
            dimension: "minecraft:overworld".into(),
            position: [1000.0, 0.0, 0.0],
            rotation: [0.0; 2],
        };
        assert_eq!(
            time_steps(99, source, &destination.dimension, &destination),
            100
        );
        assert_eq!(
            time_steps(100, source, &destination.dimension, &destination),
            40
        );
        assert_eq!(
            time_steps(600, source, &destination.dimension, &destination),
            9
        );
        assert_eq!(
            time_steps(600, source, "minecraft:the_nether", &destination),
            40
        );
        destination.position = [30.0, 0.0, 0.0];
        assert_eq!(
            time_steps(100, source, &destination.dimension, &destination),
            1
        );
        assert_eq!(
            time_steps(u32::MAX, source, "minecraft:the_end", &destination),
            1
        );
        assert_eq!(
            time_steps(ADMIN_COMPLEXITY, source, "minecraft:the_end", &destination),
            1
        );
    }

    #[test]
    fn the_matrix_needs_each_neighbor_and_personal_checks_its_own_owner() {
        let offsets: Vec<_> = pylons([0, 64, 0]).collect();
        assert_eq!(offsets.len(), 8);
        assert!(!offsets.contains(&[0, 64, 0]));
        assert!(offsets.contains(&[-1, 64, 1]));
        assert!(needs_owner(GPS_TELEPORTATION_MATRIX.id()));
        assert!(needs_owner(GPS_ACTIVATION_DEVICE_PERSONAL.id()));
        assert!(!needs_owner(GPS_ACTIVATION_DEVICE_SHARED.id()));
    }
}
