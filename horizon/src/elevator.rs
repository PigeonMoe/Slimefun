// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Mutex,
};

use mod_api::{
    Compound, GameMode, Host, ItemStack, LevelRef, MenuClickedEvent, MenuClosedEvent, MenuReplace,
    PlayerRef, PlayerTeleportStatus, ScheduledTaskEvent, ServerHost, Tag, TaskId, Text, UseOnEvent,
};

use crate::{
    hologram_projector::{colored_label, colored_lore},
    items::{ALUMINUM_BRONZE_INGOT, ELECTRIC_MOTOR, Kind, item},
    network_runtime,
    recipes::{CraftingRecipe, Research, Stack},
    research,
};

item!(
    ELEVATOR_PLATE,
    "stone_pressure_plate",
    "Elevator Plate",
    "aqua",
    None,
    None
);
pub(crate) const RESEARCH: Research = Research {
    id: "elevator",
    name: "Elevators",
    cost: 28,
};
pub(crate) const CRAFTING: CraftingRecipe = CraftingRecipe {
    inputs: [
        None,
        Some(Kind::Vanilla("minecraft:stone_pressure_plate")),
        None,
        Some(Kind::Vanilla("minecraft:piston")),
        Some(ELECTRIC_MOTOR),
        Some(Kind::Vanilla("minecraft:piston")),
        Some(ALUMINUM_BRONZE_INGOT),
        Some(ALUMINUM_BRONZE_INGOT),
        Some(ALUMINUM_BRONZE_INGOT),
    ],
    output: Stack {
        kind: ELEVATOR_PLATE,
        count: 2,
    },
    research: &RESEARCH,
};
pub(crate) const DEFAULT_FLOOR: &str = "§fFloor #0";
type Position = [i32; 3];
const PAGE_SIZE: usize = 27;
const PREVIOUS: usize = 29;
const NEXT: usize = 33;
const NAME: usize = 4;

#[derive(Default)]
pub(crate) struct Elevators(Mutex<State>);

#[derive(Default)]
struct State {
    activations: BTreeMap<(String, u128), Position>,
    viewers: BTreeMap<u128, View>,
    naming: BTreeMap<u128, Anchor>,
    journeys: BTreeMap<TaskId, Journey>,
    ready: Vec<Journey>,
    arrived: BTreeSet<u128>,
}

#[derive(Clone, Copy)]
struct Anchor {
    level: LevelRef,
    pos: Position,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Floor {
    y: i32,
    number: usize,
    name: String,
}

#[derive(Clone)]
struct View {
    window: i32,
    anchor: Anchor,
    menu: Menu,
}

#[derive(Clone)]
enum Menu {
    Editor,
    Selector { floors: Vec<Floor>, page: usize },
}

struct Journey {
    player: u128,
    anchor: Anchor,
    floor: Floor,
}

fn researched(host: &dyn Host, player: PlayerRef) -> bool {
    player
        .persistent_data(host)
        .as_ref()
        .is_some_and(|saved| research::unlocked(saved, &RESEARCH))
}

pub(crate) fn refuses_placement(host: &mut dyn ServerHost, event: &UseOnEvent<'_>) -> bool {
    ELEVATOR_PLATE.matches(host, event.stack)
        && !host
            .player(event.player)
            .is_some_and(|player| can_use(host, player))
}

fn can_use(host: &mut dyn ServerHost, player: PlayerRef) -> bool {
    if researched(host, player) {
        return true;
    }
    let _ = player.tell(
        host,
        colored_label("&cUnlock Elevators with /sf research elevator."),
    );
    false
}

impl Elevators {
    pub(crate) fn step(&self, dimension: &str, player: u128, pos: Position) {
        self.0
            .lock()
            .unwrap()
            .activations
            .entry((dimension.to_owned(), player))
            .or_insert(pos);
    }

    pub(crate) fn tick(&self, host: &mut dyn ServerHost, level: LevelRef) {
        let Some(dimension) = level.dimension().id(host) else {
            return;
        };
        let triggers: Vec<_> = self
            .0
            .lock()
            .unwrap()
            .activations
            .extract_if(.., |(dim, _), _| dim == &dimension)
            .collect();
        for ((_, uuid), pos) in triggers {
            let Some(player) = host
                .player(uuid)
                .filter(|player| on_plate(host, *player, level, pos))
            else {
                continue;
            };
            let anchor = Anchor { level, pos };
            if network_runtime::elevator_floor(host, level, pos).is_none() || !can_use(host, player)
            {
                continue;
            }
            if self.0.lock().unwrap().arrived.remove(&uuid) {
                continue;
            }
            let floors = floors(host, anchor);
            if floors.len() < 2 {
                let _ = player.tell(host, colored_label("&4No destinations found"));
            } else {
                self.open(host, player, anchor, Menu::Selector { floors, page: 1 });
            }
        }
    }

    pub(crate) fn editor(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        level: LevelRef,
        pos: Position,
    ) -> bool {
        let Some((owner, _)) = network_runtime::elevator_floor(host, level, pos) else {
            return false;
        };
        if owner != player.uuid() {
            return false;
        }
        if can_use(host, player) {
            self.open(host, player, Anchor { level, pos }, Menu::Editor);
        }
        true
    }

    fn open(&self, host: &mut dyn ServerHost, player: PlayerRef, anchor: Anchor, menu: Menu) {
        let (rows, title, items) = match &menu {
            Menu::Editor => {
                let Some(name) = anchor.owned_name(host, player) else {
                    return;
                };
                let mut items = vec![None; 9];
                items[NAME] = icon(
                    host,
                    "minecraft:name_tag",
                    "&7Floor Name &e(Click to edit)",
                    &[String::new(), format!("&f{name}")],
                );
                (1, Text::literal("Configure this elevator"), items)
            }
            Menu::Selector { floors, page } => (
                4,
                colored_label("&3- Pick a floor -"),
                selector(host, floors, *page, anchor.pos[1]),
            ),
        };
        if let Ok(Some(window)) =
            host.open_menu(player.uuid(), rows, title, items, MenuReplace::CloseFirst)
        {
            self.0.lock().unwrap().viewers.insert(
                player.uuid(),
                View {
                    window,
                    anchor,
                    menu,
                },
            );
        }
    }

    pub(crate) fn clicked(&self, host: &mut dyn ServerHost, event: &MenuClickedEvent) {
        if !matches!(event.mode, 0 | 1) || !matches!(event.button, 0 | 1) {
            return;
        }
        let Some(view) = self
            .0
            .lock()
            .unwrap()
            .viewers
            .get(&event.uuid)
            .filter(|view| view.window == event.window)
            .cloned()
        else {
            return;
        };
        let Some(player) = host
            .player(event.uuid)
            .filter(|player| player.entity().level(host) == Some(view.anchor.level))
        else {
            return;
        };
        let Ok(slot) = usize::try_from(event.slot) else {
            return;
        };
        match view.menu {
            Menu::Editor if slot == NAME && view.anchor.owned_name(host, player).is_some() => {
                host.close_menu(event.uuid);
                self.0
                    .lock()
                    .unwrap()
                    .naming
                    .insert(event.uuid, view.anchor);
                let _ = player.tell(host, colored_label("&7Please enter your desired floor name into your Chat. &f(Color Codes are supported!)"));
            }
            Menu::Selector { floors, page } => {
                if let Some(page) = page_for_click(floors.len(), page, slot) {
                    self.open(host, player, view.anchor, Menu::Selector { floors, page });
                } else if slot < PAGE_SIZE
                    && let Some(floor) = floors
                        .get((page - 1) * PAGE_SIZE + slot)
                        .filter(|floor| floor.y != view.anchor.pos[1])
                    && let Ok(task) = host.schedule_task(0, 0)
                {
                    self.0.lock().unwrap().journeys.insert(
                        task,
                        Journey {
                            player: event.uuid,
                            anchor: view.anchor,
                            floor: floor.clone(),
                        },
                    );
                }
            }
            _ => {}
        }
    }

    pub(crate) fn chat(&self, host: &mut dyn ServerHost, uuid: u128, input: &str) -> Option<Text> {
        let anchor = self.0.lock().unwrap().naming.remove(&uuid)?;
        let player = host.player(uuid)?;
        let name = input.replace('§', "&");
        if anchor.owned_name(host, player).is_none()
            || !network_runtime::rename_elevator(host, anchor.level, anchor.pos, uuid, &name)
        {
            return Some(colored_label("&cThis elevator is no longer available."));
        }
        self.open(host, player, anchor, Menu::Editor);
        Some(colored_label(&format!(
            "&2Successfully named this floor: &f{name}"
        )))
    }

    pub(crate) fn due(&self, host: &mut dyn ServerHost, event: &ScheduledTaskEvent<'_>) {
        if event.owner != "slimefun" {
            return;
        }
        let journey = self.0.lock().unwrap().journeys.remove(&event.id);
        if let Some(journey) = journey {
            if journey.anchor.level.in_place(host) {
                self.teleport(host, journey);
            } else {
                self.0.lock().unwrap().ready.push(journey);
            }
        }
    }

    pub(crate) fn tick_pre(&self, host: &mut dyn ServerHost, level: LevelRef) {
        let ready: Vec<_> = self
            .0
            .lock()
            .unwrap()
            .ready
            .extract_if(.., |journey| journey.anchor.level == level)
            .collect();
        for journey in ready {
            self.teleport(host, journey);
        }
    }

    fn teleport(&self, host: &mut dyn ServerHost, journey: Journey) {
        let Journey {
            player,
            anchor: Anchor {
                level,
                pos: [x, _, z],
            },
            floor,
        } = journey;
        let Some(player) = host
            .player(player)
            .filter(|player| player.entity().level(host) == Some(level))
        else {
            return;
        };
        let Some((_, live_name)) = network_runtime::elevator_floor(host, level, [x, floor.y, z])
        else {
            return;
        };
        let Some([yaw, pitch]) = player.entity().rotation(host) else {
            return;
        };
        let Some(dimension) = level.dimension().id(host) else {
            return;
        };
        if player.teleport_checked(
            host,
            &dimension,
            destination([x, floor.y, z]),
            Some([flipped_yaw(yaw), pitch]),
        ) == Ok(PlayerTeleportStatus::Moved)
        {
            self.0.lock().unwrap().arrived.insert(player.uuid());
            let title = colored_label(&format!("&f{live_name}"));
            let _ = player.title(host, &title, &Text::literal(""), [20, 60, 20]);
        }
    }

    pub(crate) fn closed(&self, event: &MenuClosedEvent) {
        let mut state = self.0.lock().unwrap();
        if state
            .viewers
            .get(&event.uuid)
            .is_some_and(|view| view.window == event.window)
        {
            state.viewers.remove(&event.uuid);
        }
    }

    pub(crate) fn logout(&self, player: u128) {
        let mut state = self.0.lock().unwrap();
        state.viewers.remove(&player);
        state.naming.remove(&player);
        state.arrived.remove(&player);
        state.activations.retain(|(_, uuid), _| *uuid != player);
        state.journeys.retain(|_, journey| journey.player != player);
        state.ready.retain(|journey| journey.player != player);
    }

    pub(crate) fn unload(&self, host: &dyn Host, dimension: &str) {
        let still_open = |level: LevelRef| level.dimension().id(host).as_deref() != Some(dimension);
        let mut state = self.0.lock().unwrap();
        state
            .viewers
            .retain(|_, view| still_open(view.anchor.level));
        state.naming.retain(|_, anchor| still_open(anchor.level));
        state
            .journeys
            .retain(|_, journey| still_open(journey.anchor.level));
        state
            .ready
            .retain(|journey| still_open(journey.anchor.level));
        state.activations.retain(|(dim, _), _| dim != dimension);
    }

    pub(crate) fn clear(&self) {
        *self.0.lock().unwrap() = State::default();
    }
}

impl Anchor {
    fn owned_name(self, host: &mut dyn ServerHost, player: PlayerRef) -> Option<String> {
        if player.entity().level(host) != Some(self.level) {
            return None;
        }
        let (owner, name) = network_runtime::elevator_floor(host, self.level, self.pos)?;
        (owner == player.uuid()).then_some(name)
    }
}

fn floors(host: &mut dyn ServerHost, anchor: Anchor) -> Vec<Floor> {
    let Some((min, height)) = anchor.level.height(host) else {
        return Vec::new();
    };
    let [x, _, z] = anchor.pos;
    enumerate_floors((min..min.saturating_add(height)).filter_map(|y| {
        let (_, name) = network_runtime::elevator_floor(host, anchor.level, [x, y, z])?;
        Some((y, name))
    }))
}

fn enumerate_floors(iter: impl Iterator<Item = (i32, String)>) -> Vec<Floor> {
    let mut floors = iter
        .enumerate()
        .map(|(number, (y, name))| Floor { y, number, name })
        .collect::<Vec<_>>();
    floors.reverse();
    floors
}

fn page_for_click(count: usize, page: usize, slot: usize) -> Option<usize> {
    let pages = 1 + count / PAGE_SIZE;
    match slot {
        PREVIOUS if page > 1 => Some(page - 1),
        NEXT if page < pages => Some(page + 1),
        _ => None,
    }
}

fn selector(
    host: &dyn Host,
    floors: &[Floor],
    page: usize,
    current: i32,
) -> Vec<Option<ItemStack>> {
    let mut items = vec![None; 36];
    for (slot, floor) in floors
        .iter()
        .skip((page - 1) * PAGE_SIZE)
        .take(PAGE_SIZE)
        .enumerate()
    {
        let (material, detail) = if floor.y == current {
            (
                "minecraft:compass",
                "&eThis is the floor you are currently on:",
            )
        } else {
            ("minecraft:paper", "&eClick &7to teleport to this floor:")
        };
        items[slot] = icon(
            host,
            material,
            &format!("&7{}. &0{}", floor.number, floor.name),
            &[format!("{detail} &f{}", floor.name)],
        );
    }
    for (slot, item) in items.iter_mut().enumerate().skip(PAGE_SIZE) {
        *item = if page_for_click(floors.len(), page, slot).is_none() {
            icon(host, "minecraft:gray_stained_glass_pane", " ", &[])
        } else {
            let name = if slot == PREVIOUS {
                "&f⇦ Previous Page"
            } else {
                "&fNext Page ⇨"
            };
            icon(
                host,
                "minecraft:lime_stained_glass_pane",
                name,
                &[
                    String::new(),
                    format!("&7({page} / {})", 1 + floors.len() / PAGE_SIZE),
                ],
            )
        };
    }
    items
}

fn icon(
    host: &dyn Host,
    material: &'static str,
    name: &str,
    lines: &[String],
) -> Option<ItemStack> {
    let mut item = Kind::Vanilla(material).stack(host, 1)?;
    item.set_components(Compound::from([
        ("minecraft:custom_name".into(), colored_lore(name)),
        (
            "minecraft:lore".into(),
            Tag::List(lines.iter().map(|line| colored_lore(line)).collect()),
        ),
    ]))
    .ok()?;
    Some(item)
}

fn on_plate(host: &dyn Host, player: PlayerRef, level: LevelRef, pos: Position) -> bool {
    if player.entity().level(host) != Some(level)
        || player.game_mode(host) == Some(GameMode::Spectator)
    {
        return false;
    }
    let Some([min, max]) = player.entity().bounding_box(host) else {
        return false;
    };
    let [x, y, z] = pos.map(f64::from);
    let plate_min = [x + 0.0625, y, z + 0.0625];
    let plate_max = [x + 0.9375, y + 0.25, z + 0.9375];
    (0..3).all(|axis| min[axis] < plate_max[axis] && max[axis] > plate_min[axis])
}

fn destination(pos: Position) -> [f64; 3] {
    let [x, y, z] = pos.map(f64::from);
    [x + 0.5, y + 0.4, z + 0.5]
}

fn flipped_yaw(yaw: f32) -> f32 {
    let rotated = yaw + 180.0;
    if rotated > 180.0 {
        rotated - 360.0
    } else {
        rotated
    }
}

pub(crate) fn decorate(id: &str, components: &mut Compound) {
    if id == ELEVATOR_PLATE.id() {
        components.insert(
            "minecraft:lore".into(),
            Tag::List(
                [
                    "",
                    "&fPlace an Elevator Plate on every floor",
                    "&fand you will be able to teleport between them.",
                    "",
                    "&eRight Click this Block &7to name it",
                ]
                .into_iter()
                .map(colored_lore)
                .collect(),
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use mod_api::{Identifier, testing::FakeHost};

    use super::*;

    #[test]
    fn the_lowest_floor_is_zero_and_the_selector_shows_the_highest_floor_first() {
        let floors = enumerate_floors(
            [
                (-63, "&7Basement".to_owned()),
                (20, "&fLobby".to_owned()),
                (275, "&aRoof".to_owned()),
            ]
            .into_iter(),
        );
        assert_eq!(
            floors
                .iter()
                .map(|floor| (floor.number, floor.y))
                .collect::<Vec<_>>(),
            [(2, 275), (1, 20), (0, -63)]
        );
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                [
                    "minecraft:paper",
                    "minecraft:compass",
                    "minecraft:gray_stained_glass_pane",
                ]
                .map(|id| Identifier::parse(id).unwrap()),
            )
            .unwrap();
        let items = selector(&host, &floors, 1, 20);
        assert_eq!(items.len(), 36);
        assert_eq!(
            items[0].as_ref().unwrap().item().id(&host),
            "minecraft:paper"
        );
        assert_eq!(
            items[1].as_ref().unwrap().item().id(&host),
            "minecraft:compass"
        );
        assert!(items[3].is_none());
        assert_eq!(
            items[NEXT].as_ref().unwrap().item().id(&host),
            "minecraft:gray_stained_glass_pane"
        );
    }

    #[test]
    fn paper_pages_keep_the_extra_page_and_travel_keeps_the_original_coordinates_and_yaw() {
        assert_eq!(page_for_click(26, 1, NEXT), None);
        assert_eq!(page_for_click(27, 1, NEXT), Some(2));
        assert_eq!(page_for_click(27, 2, PREVIOUS), Some(1));
        assert_eq!(page_for_click(27, 2, NEXT), None);
        assert_eq!(destination([-20, 251, 7]), [-19.5, 251.4, 7.5]);
        assert_eq!(flipped_yaw(0.0), 180.0);
        assert_eq!(flipped_yaw(94.0), -86.0);
        assert_eq!(flipped_yaw(-95.0), 85.0);
        assert_eq!(CRAFTING.output.count, 2);
        assert_eq!(CRAFTING.inputs[4], Some(ELECTRIC_MOTOR));
        assert_eq!(CRAFTING.research.cost, 28);
    }
}
