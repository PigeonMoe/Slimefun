// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, sync::Mutex};

use mod_api::{
    BlockEntityRef, Host, ItemStack, LevelRef, MenuClickedEvent, MenuClosedEvent, MenuReplace,
    MenuSlotPlace, PlayerRef, ServerHost, SoundSource, Text,
};

use crate::{
    android_items,
    android_library::{Entries, Library},
    android_machine::{self, FUEL, PAPER_SLOTS, Position},
    android_program::{self, Instruction, MAX_LENGTH, Mode, Script, Tier},
    gps_controls::{head_icon, icon},
    items::{HeadTexture, Kind},
    network_runtime, research,
};

#[derive(Clone, Copy)]
struct Anchor {
    level: LevelRef,
    pos: Position,
    entity: BlockEntityRef,
    mode: Mode,
}

#[derive(Clone)]
enum Screen {
    Main,
    Memory,
    Script(Script),
    Choices { script: Script, index: usize },
    Library { page: usize, rows: Vec<usize> },
}

#[derive(Clone)]
struct View {
    anchor: Anchor,
    window: i32,
    screen: Screen,
    saved: Script,
}

#[derive(Default)]
pub(crate) struct Menus {
    views: Mutex<BTreeMap<u128, View>>,
    names: Mutex<BTreeMap<u128, Upload>>,
    library: Library,
}

#[derive(Clone)]
struct Upload {
    anchor: Anchor,
    page: usize,
    script: Script,
}

const PAGE_SIZE: usize = 45;

fn library_screen(page: usize) -> Screen {
    Screen::Library {
        page,
        rows: Vec::new(),
    }
}

fn can_manage(host: &dyn Host, player: PlayerRef, owner: u128) -> bool {
    player.uuid() == owner
        || player
            .permission_level(host)
            .is_some_and(|level| level >= 2)
}

impl Menus {
    pub fn use_at(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        level: LevelRef,
        pos: Position,
    ) -> bool {
        let Some(mode) = network_runtime::android_at(host, level, pos) else {
            return false;
        };
        let Some(entity) = host.block_entity(level, pos) else {
            return true;
        };
        let Some(machine) = android_machine::data(host, level, pos) else {
            tell(
                host,
                player,
                "This Android's stored program is unavailable.",
            );
            return true;
        };
        if !can_manage(host, player, machine.owner) {
            tell(
                host,
                player,
                "Only the Android's owner can open its controls.",
            );
            return true;
        }
        if let Some((_, required)) = android_items::kind_and_research(mode)
            && !research::can_use(host, player, required)
        {
            tell(
                host,
                player,
                &format!("Unlock {} to use this machine.", required.name),
            );
            return true;
        }
        self.open(
            host,
            player,
            Anchor {
                level,
                pos,
                entity,
                mode,
            },
            Screen::Main,
        );
        true
    }

    fn open(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        anchor: Anchor,
        mut screen: Screen,
    ) {
        let Some(machine) = android_machine::data(host, anchor.level, anchor.pos) else {
            return;
        };
        let (title, items) = match &mut screen {
            Screen::Main => (
                "Programmable Android",
                main_items(host, anchor.mode, &machine),
            ),
            Screen::Memory => ("Script Editor", memory_items(host, anchor.mode)),
            Screen::Script(script) => ("Script Editor", script_items(host, script)),
            Screen::Choices { .. } => ("Script Editor", choice_items(host, anchor.mode)),
            Screen::Library { page, rows } => {
                let entries = match self.library.load(host) {
                    Ok(entries) => entries,
                    Err(error) => {
                        tell(host, player, &error);
                        return;
                    }
                };
                let ids = entries.available(anchor.mode);
                let last = ids.len() / PAGE_SIZE;
                *page = (*page).min(last);
                let start = *page * PAGE_SIZE;
                *rows = ids[start..ids.len().min(start + PAGE_SIZE)].to_vec();
                (
                    "Android Scripts",
                    library_items(host, player, anchor.mode, &entries, rows, *page, last),
                )
            }
        };
        let rows = (items.len() / 9) as u8;
        let mut backing = vec![MenuSlotPlace::Decoration; items.len()];
        if matches!(screen, Screen::Main) {
            for (slot, &menu) in PAPER_SLOTS.iter().enumerate() {
                backing[menu] = MenuSlotPlace::BlockEntityItems {
                    entity: anchor.entity,
                    slot,
                    allow_insert: slot == FUEL,
                };
            }
        }
        if let Ok(Some(window)) = host.open_block_inventory_menu(
            player,
            anchor.entity,
            rows,
            Text::literal(title),
            items,
            MenuReplace::CloseFirst,
        ) {
            if player.back_mod_menu_slots(host, window, backing).is_err() {
                host.close_menu(player.uuid());
                return;
            }
            if matches!(screen, Screen::Library { .. }) {
                let _ = player.play_sound(
                    host,
                    "minecraft:block.note_block.hat",
                    SoundSource::Master,
                    0.7,
                    0.7,
                );
            }
            self.views.lock().unwrap().insert(
                player.uuid(),
                View {
                    anchor,
                    window,
                    screen,
                    saved: machine.script,
                },
            );
        }
    }

    pub fn clicked(&self, host: &mut dyn ServerHost, event: &MenuClickedEvent) {
        if !matches!(event.mode, 0 | 1) || !matches!(event.button, 0 | 1) {
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
        let Anchor {
            level,
            pos,
            entity,
            mode,
        } = view.anchor;
        if player.entity().level(host) != Some(level)
            || host.block_entity(level, pos) != Some(entity)
        {
            host.close_menu(event.uuid);
            return;
        }
        let Some(mut machine) = android_machine::data(host, level, pos)
            .filter(|machine| can_manage(host, player, machine.owner))
        else {
            host.close_menu(event.uuid);
            return;
        };
        let Ok(slot) = usize::try_from(event.slot) else {
            return;
        };
        let shift = event.mode == 1;
        let right = event.button == 1;
        let mut changed = false;
        let mut pause_notice = None;
        let next = match view.screen {
            Screen::Main if !shift => match slot {
                15 if mode != Mode::Archived => {
                    machine.paused = false;
                    save_or_close(
                        host,
                        player,
                        pos,
                        machine,
                        "Your Android is running its script.",
                    );
                    host.close_menu(event.uuid);
                    return;
                }
                15 => {
                    tell(
                        host,
                        player,
                        "This archived Android mode retains its original script and inventory.",
                    );
                    return;
                }
                17 => {
                    machine.paused = true;
                    save_or_close(
                        host,
                        player,
                        pos,
                        machine,
                        "Your Android has paused its script.",
                    );
                    return;
                }
                16 => {
                    machine.paused = true;
                    changed = true;
                    pause_notice = Some("Your Android has paused its script.");
                    Screen::Memory
                }
                _ => return,
            },
            Screen::Memory if !shift => match slot {
                1 => Screen::Script(machine.script.clone()),
                3 if mode != Mode::Archived => Screen::Script(Script::default()),
                5 if mode != Mode::Archived => library_screen(0),
                8 => Screen::Main,
                _ => return,
            },
            Screen::Script(mut script) => {
                let last = script.tokens().len() - 1;
                let add = script.tokens().len() < MAX_LENGTH;
                if slot == 0 || slot == last + usize::from(add) {
                    Screen::Main
                } else if mode == Mode::Archived || slot > last {
                    return;
                } else if slot == last {
                    Screen::Choices {
                        script,
                        index: slot,
                    }
                } else if right {
                    let edited = if shift {
                        script.duplicate(slot)
                    } else {
                        script.edit(slot, None)
                    };
                    if edited.is_none() {
                        return;
                    }
                    machine.script = script.clone();
                    changed = true;
                    Screen::Script(script)
                } else if !shift {
                    Screen::Choices {
                        script,
                        index: slot,
                    }
                } else {
                    return;
                }
            }
            Screen::Choices { mut script, index } if !shift => {
                let instruction = if slot == 9 {
                    None
                } else {
                    let Some(choice) = slot
                        .checked_sub(10)
                        .and_then(|offset| android_program::choices(mode).nth(offset))
                    else {
                        return;
                    };
                    Some(choice)
                };
                if script.edit(index, instruction).is_none() {
                    return;
                }
                machine.script = script.clone();
                changed = true;
                Screen::Script(script)
            }
            Screen::Library { page, rows } => {
                let next = match slot {
                    46 | 50 if !shift => {
                        let entries = match self.library.load(host) {
                            Ok(entries) => entries,
                            Err(error) => {
                                tell(host, player, &error);
                                return;
                            }
                        };
                        let pages = entries.available(mode).len() / PAGE_SIZE + 1;
                        let next = if slot == 46 {
                            (page + pages - 1) % pages
                        } else {
                            (page + 1) % pages
                        };
                        library_screen(next)
                    }
                    48 if !shift => {
                        let entries = match self.library.load(host) {
                            Ok(entries) => entries,
                            Err(error) => {
                                tell(host, player, &error);
                                return;
                            }
                        };
                        if entries.has_code(mode, &machine.script.source()) {
                            tell(host, player, "This script was already uploaded.");
                            return;
                        }
                        host.close_menu(event.uuid);
                        self.names.lock().unwrap().insert(
                            event.uuid,
                            Upload {
                                anchor: view.anchor,
                                page,
                                script: machine.script,
                            },
                        );
                        tell(
                            host,
                            player,
                            "Type the name of your Android script in chat.",
                        );
                        return;
                    }
                    53 if !shift => Screen::Memory,
                    _ => {
                        let Some(&id) = rows.get(slot) else {
                            return;
                        };
                        if !shift && right {
                            return;
                        }
                        let result = self.library.update(host, |host, entries| {
                            if shift {
                                entries.rate(id, event.uuid, !right)?;
                                Ok(Screen::Library { page, rows })
                            } else {
                                if machine.script != view.saved {
                                    return Err(
                                        "The Android script changed in another menu.".into()
                                    );
                                }
                                let script = entries.download(id, mode)?;
                                machine.script = script;
                                if !android_machine::save(host, pos, machine) {
                                    return Err("The Android's changes could not be saved.".into());
                                }
                                Ok(Screen::Memory)
                            }
                        });
                        match result {
                            Ok(screen) => screen,
                            Err(error) => {
                                tell(host, player, &error);
                                return;
                            }
                        }
                    }
                };
                self.open(host, player, view.anchor, next);
                return;
            }
            _ => return,
        };
        if changed {
            if !matches!(next, Screen::Memory)
                && !android_machine::data(host, level, pos)
                    .is_some_and(|current| current.script == view.saved)
            {
                tell(host, player, "The Android script changed in another menu.");
                return;
            }
            if !android_machine::save(host, pos, machine) {
                tell(host, player, "The Android's changes could not be saved.");
                return;
            }
            if let Some(message) = pause_notice {
                tell(host, player, message);
            }
        }
        self.open(host, player, view.anchor, next);
    }

    pub fn take_chat(
        &self,
        host: &mut dyn ServerHost,
        player_id: u128,
        title: &str,
    ) -> Option<Text> {
        let pending = self.names.lock().unwrap().remove(&player_id)?;
        let player = host.player(player_id)?;
        let Anchor {
            level,
            pos,
            entity,
            mode,
        } = pending.anchor;
        if player.entity().level(host) != Some(level)
            || host.block_entity(level, pos) != Some(entity)
            || !android_machine::data(host, level, pos)
                .is_some_and(|machine| can_manage(host, player, machine.owner))
        {
            return Some(Text::literal(
                "Your Android's script menu is no longer available.",
            ));
        }
        let Some(name) = player.name(host) else {
            return Some(Text::literal("The script author's name is unavailable."));
        };
        let upload = self.library.update(host, |_, entries| {
            entries.upload(mode, player_id, &name, title, &pending.script)
        });
        Some(Text::literal(match upload {
            Ok(()) => {
                self.open(host, player, pending.anchor, library_screen(pending.page));
                "Your script was uploaded.".to_owned()
            }
            Err(error) => error,
        }))
    }

    pub fn closed(&self, event: &MenuClosedEvent) {
        let mut views = self.views.lock().unwrap();
        if views
            .get(&event.uuid)
            .is_some_and(|view| view.window == event.window)
        {
            views.remove(&event.uuid);
        }
    }

    pub fn logout(&self, player: u128) {
        self.views.lock().unwrap().remove(&player);
        self.names.lock().unwrap().remove(&player);
    }

    pub fn clear(&self) {
        self.views.lock().unwrap().clear();
        self.names.lock().unwrap().clear();
    }
}

fn save_or_close(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    pos: Position,
    machine: android_machine::Machine,
    message: &str,
) {
    if android_machine::save(host, pos, machine) {
        tell(host, player, message);
    } else {
        tell(host, player, "The Android's changes could not be saved.");
    }
}

fn tell(host: &mut dyn ServerHost, player: PlayerRef, message: &str) {
    let _ = player.tell(host, Text::literal(message));
}

fn main_items(
    host: &dyn Host,
    mode: Mode,
    machine: &android_machine::Machine,
) -> Vec<Option<ItemStack>> {
    const BORDER: &[usize] = &[
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 18, 24, 25, 26, 27, 33, 35, 36, 42, 44, 45, 46, 47, 48, 49,
        50, 51, 52, 53,
    ];
    const OUT: &[usize] = &[10, 11, 12, 13, 14, 19, 23, 28, 32, 37, 38, 39, 40, 41];
    let mut items = vec![None; 54];
    for (slots, material) in [
        (BORDER, "minecraft:gray_stained_glass_pane"),
        (OUT, "minecraft:orange_stained_glass_pane"),
    ] {
        let pane = icon(host, Kind::Vanilla(material), " ", &[]);
        for &slot in slots {
            items[slot] = pane.clone();
        }
    }
    items[15] = head(host, "START", "Start/Continue", &[]);
    items[16] = head(
        host,
        "MEMORY",
        "Memory Core",
        &["Click to open the Script Editor".into()],
    );
    items[17] = head(host, "PAUSE", "Pause", &[]);
    let (fuel, examples) = match mode.tier() {
        Some(Tier::Solid) => ("solid fuel", "e.g. coal or wood"),
        Some(Tier::Liquid) => ("liquid fuel", "e.g. lava, oil or fuel"),
        Some(Tier::Nuclear) => (
            "radioactive fuel",
            "e.g. uranium, neptunium or boosted uranium",
        ),
        None => (
            "archived fuel",
            "This Android retains its old fuel and inventory",
        ),
    };
    items[34] = head(
        host,
        "FUEL",
        "⇩ Fuel Input ⇩",
        &[
            format!("This Android runs on {fuel}."),
            examples.to_owned(),
            format!("Remaining steps: {}", machine.fuel),
        ],
    );
    for (index, &slot) in PAPER_SLOTS.iter().enumerate() {
        items[slot] = android_machine::slot(host, machine, index);
    }
    items
}

fn memory_items(host: &dyn Host, mode: Mode) -> Vec<Option<ItemStack>> {
    let mut items = vec![None; 54];
    items[1] = head(host, "GO_FORWARD", "> Edit Script", &[]);
    if mode != Mode::Archived {
        items[3] = head(host, "NEW", "> Create new Script", &[]);
        items[5] = head(host, "GO_DOWN", "> Download a Script", &[]);
    }
    items[8] = head(host, "TURN_LEFT", "> Back", &[]);
    items
}

fn library_items(
    host: &dyn ServerHost,
    player: PlayerRef,
    mode: Mode,
    library: &Entries,
    rows: &[usize],
    page: usize,
    last: usize,
) -> Vec<Option<ItemStack>> {
    let mut items = vec![None; 54];
    items[PAGE_SIZE..].fill(icon(
        host,
        Kind::Vanilla("minecraft:gray_stained_glass_pane"),
        " ",
        &[],
    ));
    let pages = [format!("Page {} / {}", page + 1, last + 1)];
    items[46] = head(host, "TURN_LEFT", "Previous page", &pages);
    items[48] = head(
        host,
        "GO_UP",
        "Upload a Script",
        &["Click to share your current script".into()],
    );
    items[50] = head(host, "GO_FORWARD", "Next page", &pages);
    items[53] = head(host, "TURN_LEFT", "> Back", &[]);
    let Some((kind, _)) = android_items::kind_and_research(mode) else {
        return items;
    };
    for (slot, entry) in rows.iter().filter_map(|&id| library.get(id)).enumerate() {
        let author = host
            .player(entry.author)
            .and_then(|player| player.name(host));
        let mut lore = vec![
            format!("by {}", author.as_deref().unwrap_or(&entry.author_name)),
            String::new(),
            format!("Downloads: {}", entry.downloads),
            format!("Rating: {:?}% ", entry.rating()),
            format!("{} ☺ | ☹ {}", entry.positive.len(), entry.negative.len()),
            String::new(),
            "Left Click to download this Script".into(),
            "(This will override your current Script)".into(),
        ];
        if entry.can_rate(player.uuid()) {
            lore.extend([
                String::new(),
                "Shift + Left Click to leave a positive Rating".into(),
                "Shift + Right Click to leave a negative Rating".into(),
            ]);
        }
        items[slot] = icon(host, kind, &entry.name, &lore);
    }
    items
}

fn script_items(host: &dyn Host, script: &Script) -> Vec<Option<ItemStack>> {
    let mut items = vec![None; 54];
    let back = ["Left Click to return to the Android's interface".into()];
    items[0] = head(host, "START", "Start Script", &back);
    let last = script.tokens().len() - 1;
    let add = usize::from(script.tokens().len() < MAX_LENGTH);
    if add == 1 {
        items[last] = head(host, "NEW", "> Add new Command", &[]);
    }
    items[last + add] = head(host, "REPEAT", "Repeat Script", &back);
    let lore = [
        "Left Click to edit".into(),
        "Right Click to delete".into(),
        "Shift + Right Click to duplicate".into(),
    ];
    for (index, token) in script.tokens().iter().enumerate().take(last).skip(1) {
        let name = match Instruction::parse(token) {
            Some(instruction) => instruction.label(),
            None => token.as_str(),
        };
        items[index] = head(host, token, name, &lore);
    }
    items
}

fn choice_items(host: &dyn Host, mode: Mode) -> Vec<Option<ItemStack>> {
    let mut items = vec![None; 54];
    items[..9].fill(icon(
        host,
        Kind::Vanilla("minecraft:gray_stained_glass_pane"),
        " ",
        &[],
    ));
    items[9] = head(host, "PAUSE", "Do nothing", &[]);
    for (index, choice) in (10..).zip(android_program::choices(mode)) {
        items[index] = head(host, choice.id(), choice.label(), &[]);
    }
    items
}

fn head(host: &dyn Host, key: &str, name: &str, lore: &[String]) -> Option<ItemStack> {
    let key = match key {
        "FARM_EXOTIC_FORWARD" => "FARM_FORWARD",
        "FARM_EXOTIC_DOWN" => "FARM_DOWN",
        _ => key.strip_prefix("MOVE_AND_").unwrap_or(key),
    };
    if let Some((_, texture)) = SKINS.iter().find(|(id, _)| *id == key) {
        head_icon(host, texture, name, lore)
    } else {
        icon(host, Kind::Vanilla("minecraft:paper"), name, lore)
    }
}

const SKINS: &[(&str, HeadTexture)] = &[
    (
        "CHOP_TREE",
        HeadTexture {
            profile: [-476162738, 1038692848, -1749521871, 1196710938],
            value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzY0YmE0OTM4NGRiYTdiN2FjZGI0ZjcwZTkzNjFlNmQ1N2NiYmNiZjcyMGNmNGYxNmMyYmI4M2U0NTU3In19fQ==",
        },
    ),
    (
        "DIG_UP",
        HeadTexture {
            profile: [177738152, -712886039, -1887289086, -1381350062],
            value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzJlNmNlMDExYWM5YTdhNzViMmZjZDQwOGFkMjFhM2FjMTcyMmY2ZTJlZWQ4NzgxY2FmZDEyNTUyMjgyYjg4In19fQ==",
        },
    ),
    (
        "DIG_FORWARD",
        HeadTexture {
            profile: [1919431423, 1855863686, -1851580769, -446152524],
            value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2I2ZWEyMTM1ODM4NDYxNTM0MzcyZjJkYTZjODYyZDIxY2Q1ZjNkMmM3MTE5ZjJiYjY3NGJiZDQyNzkxIn19fQ==",
        },
    ),
    (
        "DIG_DOWN",
        HeadTexture {
            profile: [-77138982, -1332397099, -1746845057, 1867779294],
            value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzhkODYyMDI0MTA4Yzc4NWJjMGVmNzE5OWVjNzdjNDAyZGJiZmNjNjI0ZTlmNDFmODNkOGFlZDhiMzlmZDEzIn19fQ==",
        },
    ),
    (
        "FUEL",
        HeadTexture {
            profile: [-1442515976, -1100268200, -1139489874, 993810157],
            value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzkzNDNjZTU4ZGE1NGM3OTkyNGEyYzkzMzFjZmM0MTdmZThjY2JiZWE5YmU0NWE3YWM4NTg2MGE2YzczMCJ9fX0=",
        },
    ),
    (
        "CATCH_FISH",
        HeadTexture {
            profile: [614746120, 73350380, -1901274334, -1584263689],
            value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2ZkNGZkZTUxMWY0NDU0MTAxZTRhMmE3MmJjODZmMTI5ODVkZmNkYTc2YjY0YmIyNGRjNjNhOWZhOWUzYTMifX19",
        },
    ),
    (
        "START",
        HeadTexture {
            profile: [2051923214, 1764440002, -1465804083, 1654626908],
            value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzRhZTI5NDIyZGI0MDQ3ZWZkYjliYWMyY2RhZTVhMDcxOWViNzcyZmNjYzg4YTY2ZDkxMjMyMGIzNDNjMzQxIn19fQ==",
        },
    ),
    (
        "REPEAT",
        HeadTexture {
            profile: [781817581, 1102131614, -1748187270, 640604898],
            value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2JjOGRlZjY3YTEyNjIyZWFkMWRlY2QzZDg5MzY0MjU3YjUzMTg5NmQ4N2U0Njk4MTMxMzFjYTIzNWI1YzcifX19",
        },
    ),
    (
        "WAIT",
        HeadTexture {
            profile: [712385068, -309118490, -1293123308, -143694330],
            value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzJlZTE3NGY0MWU1OTRlNjRlYTMxNDFjMDdkYWY3YWNmMWZhMDQ1YzIzMGIyYjBiMGZiM2RhMTYzZGIyMmY0NTUifX19",
        },
    ),
    (
        "GO_FORWARD",
        HeadTexture {
            profile: [190581783, 516504217, -1762816564, 647721272],
            value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2Q5YmY2ZGI0YWVkYTlkODgyMmI5ZjczNjUzOGU4YzE4YjlhNDg0NGY4NGViNDU1MDRhZGZiZmVlODdlYiJ9fX0=",
        },
    ),
    (
        "GO_UP",
        HeadTexture {
            profile: [-670096659, 758594443, -1792588429, -1893041658],
            value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzEwNWEyY2FiOGI2OGVhNTdlM2FmOTkyYTM2ZTQ3YzhmZjlhYTg3Y2M4Nzc2MjgxOTY2ZjhjM2NmMzFhMzgifX19",
        },
    ),
    (
        "GO_DOWN",
        HeadTexture {
            profile: [-1408375074, -709478614, -2005606555, -1933638107],
            value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2MwMTU4NmUzOWY2ZmZhNjNiNGZiMzAxYjY1Y2E3ZGE4YTkyZjczNTNhYWFiODlkMzg4NjU3OTEyNWRmYmFmOSJ9fX0=",
        },
    ),
    (
        "TURN_LEFT",
        HeadTexture {
            profile: [-306346492, -1326761251, -1640572749, -1231546449],
            value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2ExODVjOTdkYmI4MzUzZGU2NTI2OThkMjRiNjQzMjdiNzkzYTNmMzJhOThiZTY3YjcxOWZiZWRhYjM1ZSJ9fX0=",
        },
    ),
    (
        "TURN_RIGHT",
        HeadTexture {
            profile: [-331308740, -2070925052, -1549426231, 1574221549],
            value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzMxYzBlZGVkZDcxMTVmYzFiMjNkNTFjZTk2NjM1OGIyNzE5NWRhZjI2ZWJiNmU0NWE2NmMzNGM2OWMzNDA5MSJ9fX0=",
        },
    ),
    (
        "FARM_FORWARD",
        HeadTexture {
            profile: [185821184, -1539034433, -1957073323, 213852716],
            value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzRkZTlhNTIyYzNkOWU3ZDg1ZjNkODJjMzc1ZGMzN2ZlY2M4NTZkYmQ4MDFlYjNiY2VkYzExNjUxOThiZiJ9fX0=",
        },
    ),
    (
        "FARM_DOWN",
        HeadTexture {
            profile: [-1628814217, 1089944264, -1437705605, -568766409],
            value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzJkNDI5NmIzMzNkMjUzMTlhZjNmMzMwNTE3OTdmOWU2ZDgyMWNkMTlhMDE0ZmI3MTM3YmViODZhNGU5ZTk2In19fQ==",
        },
    ),
    (
        "INTERFACE_ITEMS",
        HeadTexture {
            profile: [-2123506543, 1539325144, -2064804428, -1891490068],
            value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzkwYTRkYmY2NjI1YzQyYmU1N2E4YmEyYzMzMDk1NGE3NmJkZjIyNzg1NTQwZTg3YTVjOTY3MjY4NTIzOGVjIn19fQ==",
        },
    ),
    (
        "INTERFACE_FUEL",
        HeadTexture {
            profile: [131769886, -1323156926, -1590854046, -370301030],
            value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzI0MzJmNTI4MmE1MDc0NWI5MTJiZTE0ZGVkYTU4MWJkNGEwOWI5NzdhM2MzMmQ3ZTk1Nzg0OTFmZWU4ZmE3In19fQ==",
        },
    ),
    (
        "NEW",
        HeadTexture {
            profile: [-1914612957, 625753127, -1372670388, 2036152987],
            value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzE3MWQ4OTc5YzE4NzhhMDU5ODdhN2ZhZjIxYjU2ZDFiNzQ0ZjlkMDY4Yzc0Y2ZmY2RlMWVhMWVkYWQ1ODUyIn19fQ==",
        },
    ),
    (
        "PAUSE",
        HeadTexture {
            profile: [-1430806123, 377108335, -2105106683, -1825080643],
            value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzE2MTM5ZmQxYzU2NTRlNTZlOWU0ZTJjOGJlN2ViMmJkNWI0OTlkNjMzNjE2NjYzZmVlZTk5Yjc0MzUyYWQ2NCJ9fX0=",
        },
    ),
    (
        "MEMORY",
        HeadTexture {
            profile: [728422456, 1251750602, -1768418679, 1199211128],
            value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2Q3OGYyYjdlNWU3NTYzOWVhN2ZiNzk2YzM1ZDM2NGM0ZGYyOGI0MjQzZTY2Yjc2Mjc3YWFkY2Q2MjYxMzM3In19fQ==",
        },
    ),
];
