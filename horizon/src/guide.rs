// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, sync::Mutex};

use mod_api::{
    Compound, Host, HostTransaction, ItemPlace, ItemResource, ItemStack, MenuClickedEvent,
    MenuClosedEvent, MenuReplace, PlayerRef, ServerHost, Tag, Text,
};

use crate::{
    hologram_projector::{colored_label, colored_lore},
    items::Kind,
    recipes::Research,
    research,
};

mod catalog;
use catalog::{Entry, GROUPS, Group, group_entries, lookup};

const TITLE: &str = "Slimefun Guide";
const GUIDE_KEY: &str = "slimefun:slimefun_guide_mode";
const SURVIVAL: &str = "SURVIVAL_MODE";
const DISCOVERY_SLOTS: usize = 36;
const INGREDIENT_SLOTS: [usize; 9] = [3, 4, 5, 12, 13, 14, 21, 22, 23];

fn book(host: &dyn Host) -> Option<ItemStack> {
    let mut item = icon(
        host,
        "minecraft:enchanted_book",
        "&aSlimefun Guide &7(Chest GUI)",
        &[
            "",
            "&eRight Click &8⇨ &7Browse Items",
            "&eShift + Right Click &8⇨ &7Open Settings / Credits",
        ],
    )?;
    item.apply_components(&Compound::from([(
        "minecraft:custom_data".into(),
        Tag::compound([(
            "PublicBukkitValues",
            Tag::compound([(GUIDE_KEY, Tag::String(SURVIVAL.into()))]),
        )]),
    )]))
    .ok()?;
    Some(item)
}

fn is_book(host: &dyn Host, item: &ItemStack) -> bool {
    item.item().id(host) == "minecraft:enchanted_book"
        && matches!(
            item.components()
                .and_then(|components| components.as_nbt().get(&"minecraft:custom_data".into()))
                .and_then(|custom| custom.get("PublicBukkitValues"))
                .and_then(|values| values.get(GUIDE_KEY)),
            Some(Tag::String(mode)) if mode.to_string_lossy() == SURVIVAL
        )
}

pub(crate) fn give(host: &mut dyn ServerHost, player: PlayerRef) {
    let Some(book) = book(host) else {
        return;
    };
    let resource = ItemResource::of(&book);
    let _ = host.transaction(|host, tx| {
        (0..36)
            .any(|slot| {
                ItemPlace::PlayerSlot {
                    player: player.uuid(),
                    slot,
                }
                .insert(host, tx, &resource, 1)
                    == 1
            })
            .then_some(())
            .ok_or(())
    });
}

#[derive(Clone, Copy)]
enum Page {
    Main,
    Group(&'static Group, usize),
    Recipe(&'static Entry),
    Info,
}

#[derive(Clone, Copy)]
enum Action {
    Push(Page),
    Replace(Page),
    Back,
    Research(&'static Research),
}

#[derive(Clone)]
struct View {
    window: i32,
    path: Vec<Page>,
    actions: [Option<Action>; 54],
}

#[derive(Default)]
pub(crate) struct Guide(Mutex<BTreeMap<u128, View>>);

impl Guide {
    pub(crate) fn open(&self, host: &mut dyn ServerHost, player: PlayerRef) {
        self.show(host, player, vec![Page::Main]);
    }

    pub(crate) fn right_click(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        stack: &ItemStack,
    ) -> bool {
        if !is_book(host, stack) {
            return false;
        }
        let mut path = vec![Page::Main];
        if player.entity().is_shift_key_down(host) == Some(true) {
            path.push(Page::Info);
        }
        self.show(host, player, path);
        true
    }

    fn show(&self, host: &mut dyn ServerHost, player: PlayerRef, path: Vec<Page>) {
        let Some(page) = path.last().copied() else {
            return;
        };
        let research = player.persistent_data(host);
        let mut menu = render(host, research.as_ref(), page);
        if path.len() > 1 {
            let back = usize::from(!matches!(page, Page::Recipe(_)));
            menu.set(
                back,
                icon(
                    host,
                    "minecraft:enchanted_book",
                    "&7⇦ Back",
                    &[
                        "",
                        "&fLeft Click: &7Go back to previous Page",
                        "&fShift + left Click: &7Go back to Main Menu",
                    ],
                ),
                Some(Action::Back),
            );
        }
        let title = match page {
            Page::Info => "Settings & Info",
            _ => TITLE,
        };
        if let Ok(Some(window)) = host.open_menu(
            player.uuid(),
            6,
            Text::literal(title),
            menu.slots,
            MenuReplace::CloseFirst,
        ) {
            self.0.lock().unwrap().insert(
                player.uuid(),
                View {
                    window,
                    path,
                    actions: menu.actions,
                },
            );
        }
    }

    pub(crate) fn clicked(&self, host: &mut dyn ServerHost, event: &MenuClickedEvent) {
        if !matches!(event.mode, 0 | 1) || !matches!(event.button, 0 | 1) {
            return;
        }
        let Ok(slot) = usize::try_from(event.slot) else {
            return;
        };
        let Some(view) = self
            .0
            .lock()
            .unwrap()
            .get(&event.uuid)
            .filter(|view| view.window == event.window)
            .cloned()
        else {
            return;
        };
        let Some(Some(action)) = view.actions.get(slot).copied() else {
            return;
        };
        let Some(player) = host.player(event.uuid) else {
            return;
        };
        let mut path = view.path;
        match action {
            Action::Push(page) => path.push(page),
            Action::Replace(page) => {
                path.pop();
                path.push(page);
            }
            Action::Back if event.mode == 1 => path.truncate(1),
            Action::Back => {
                path.pop();
            }
            Action::Research(topic) => {
                if !learned(host, player, topic) {
                    let reply = research::unlock(host, player, topic);
                    if !learned(host, player, topic) {
                        let _ = player.tell(host, colored_label(&format!("&4{reply}")));
                        return;
                    }
                }
            }
        }
        self.show(host, player, path);
    }

    pub(crate) fn closed(&self, event: &MenuClosedEvent) {
        let mut views = self.0.lock().unwrap();
        if views
            .get(&event.uuid)
            .is_some_and(|view| view.window == event.window)
        {
            views.remove(&event.uuid);
        }
    }

    pub(crate) fn logout(&self, player: u128) {
        self.0.lock().unwrap().remove(&player);
    }

    pub(crate) fn clear(&self) {
        self.0.lock().unwrap().clear();
    }
}

struct Menu {
    slots: Vec<Option<ItemStack>>,
    actions: [Option<Action>; 54],
}

impl Menu {
    fn set(&mut self, slot: usize, item: Option<ItemStack>, action: Option<Action>) {
        self.actions[slot] = item.as_ref().and(action);
        self.slots[slot] = item;
    }
}

fn unlocked(saved: Option<&Compound>, entry: &Entry) -> bool {
    entry
        .research
        .is_none_or(|topic| saved.is_some_and(|data| research::unlocked(data, topic)))
}

fn learned(host: &dyn Host, player: PlayerRef, topic: &Research) -> bool {
    player
        .persistent_data(host)
        .as_ref()
        .is_some_and(|data| research::unlocked(data, topic))
}

fn render(host: &dyn Host, saved: Option<&Compound>, screen: Page) -> Menu {
    let mut menu = Menu {
        slots: vec![None; 54],
        actions: [None; 54],
    };
    let pane = icon(host, "minecraft:gray_stained_glass_pane", " ", &[]);
    for slot in (0..9).chain(45..54) {
        menu.slots[slot] = pane.clone();
    }
    match screen {
        Page::Main => {
            menu.set(
                1,
                icon(host, "minecraft:comparator", "&eSettings & Info", &[]),
                Some(Action::Push(Page::Info)),
            );
            let basics = GROUPS
                .iter()
                .find(|group| group.key == "basicMachines")
                .is_none_or(|group| group_entries(group).all(|entry| unlocked(saved, entry)));
            for (slot, group) in GROUPS
                .iter()
                .filter(|group| group_entries(group).next().is_some())
                .enumerate()
            {
                let available = !group.requires_basics || basics;
                let (material, name, lore) = if available {
                    (
                        group.icon,
                        format!("&{}{}", color_code(group.color), group.name),
                        &[][..],
                    )
                } else {
                    (
                        "minecraft:barrier",
                        format!("&4Locked &7- &f{}", group.name),
                        &[
                            "",
                            "&fComplete these categories first:",
                            "",
                            "&7Basic Machines",
                        ][..],
                    )
                };
                menu.set(
                    9 + slot,
                    icon(host, material, &name, lore),
                    available.then_some(Action::Push(Page::Group(group, 1))),
                );
            }
        }
        Page::Group(group, page) => {
            let entries: Vec<_> = group_entries(group).collect();
            let pages = entries.len().div_ceil(DISCOVERY_SLOTS).max(1);
            for (slot, entry) in entries
                .into_iter()
                .skip((page - 1) * DISCOVERY_SLOTS)
                .take(DISCOVERY_SLOTS)
                .enumerate()
            {
                if let Some(topic) = entry.research.filter(|_| !unlocked(saved, entry)) {
                    let cost = format!("&7Cost: &b{} Level(s)", topic.cost);
                    menu.set(
                        9 + slot,
                        icon(
                            host,
                            "minecraft:barrier",
                            &entry_name(entry),
                            &["&4&lLocked", "", "&a> Click to unlock", "", &cost],
                        ),
                        Some(Action::Research(topic)),
                    );
                } else {
                    menu.set(
                        9 + slot,
                        entry_icon(host, entry),
                        Some(Action::Push(Page::Recipe(entry))),
                    );
                }
            }
            pagination(host, &mut menu, group, page, pages);
        }
        Page::Recipe(entry) => {
            for (slot, input) in INGREDIENT_SLOTS.into_iter().zip(entry.ingredients) {
                let linked = input.and_then(|stack| lookup(stack.kind.id()));
                if let Some(locked) = linked.filter(|entry| !unlocked(saved, entry)) {
                    menu.set(
                        slot,
                        icon(
                            host,
                            "minecraft:barrier",
                            &entry_name(locked),
                            &["&4&lLocked"],
                        ),
                        None,
                    );
                } else {
                    menu.set(
                        slot,
                        input.and_then(|item| item.kind.stack(host, item.count)),
                        linked.map(|entry| Action::Push(Page::Recipe(entry))),
                    );
                }
            }
            let method = entry.recipe_type;
            menu.slots[10] = icon(
                host,
                method.material,
                &format!("&b{}", method.name),
                if entry.label.is_some() {
                    &["", "&a&oBuild it in the World"]
                } else {
                    &[]
                },
            );
            menu.slots[16] = entry_icon(host, entry);
        }
        Page::Info => {
            menu.slots[13] = icon(
                host,
                "minecraft:knowledge_book",
                "&fSlimefun4 Contributors",
                &[
                    "",
                    "&7Slimefun is an open-source project",
                    "&7maintained by a community of people.",
                ],
            );
        }
    }
    menu
}

fn color_code(color: &str) -> char {
    match color {
        "aqua" => 'b',
        "red" => 'c',
        _ => '7',
    }
}

fn pagination(host: &dyn Host, menu: &mut Menu, group: &'static Group, page: usize, pages: usize) {
    let position = format!("&7({page} / {pages})");
    for (slot, label, next) in [
        (
            46,
            "&f⇦ Previous Page",
            page.checked_sub(1).filter(|page| *page > 0),
        ),
        (52, "&fNext Page ⇨", (page < pages).then_some(page + 1)),
    ] {
        if let Some(next) = next {
            menu.set(
                slot,
                icon(
                    host,
                    "minecraft:lime_stained_glass_pane",
                    label,
                    &["", &position],
                ),
                Some(Action::Replace(Page::Group(group, next))),
            );
        }
    }
}

fn entry_name(entry: &Entry) -> String {
    match entry.label {
        Some(label) => label.to_owned(),
        None => match entry.item.kind {
            Kind::Slimefun(item) => item.name.to_owned(),
            Kind::Vanilla(id) => id.trim_start_matches("minecraft:").replace('_', " "),
        },
    }
}

fn entry_icon(host: &dyn Host, entry: &Entry) -> Option<ItemStack> {
    if let Some(label) = entry.label {
        return icon(host, entry.item.kind.material(), label, &[]);
    }
    entry.item.kind.stack(host, entry.item.count)
}

fn icon(host: &dyn Host, material: &'static str, name: &str, lore: &[&str]) -> Option<ItemStack> {
    let mut item = Kind::Vanilla(material).stack(host, 1)?;
    let mut components = Compound::from([("minecraft:custom_name".into(), colored_lore(name))]);
    if !lore.is_empty() {
        components.insert(
            "minecraft:lore".into(),
            Tag::List(lore.iter().map(|line| colored_lore(line)).collect()),
        );
    }
    item.apply_components(&components).ok()?;
    Some(item)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Identifier, prelude::PlayersHost, testing::FakeHost};

    #[test]
    fn the_paper_book_has_the_original_mode_and_only_a_supported_book_opens() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                ["minecraft:enchanted_book", "minecraft:book"]
                    .map(|material| Identifier::parse(material).unwrap()),
            )
            .unwrap();
        let level = host.level.level();
        host.players.join(1, 100, level);
        let player = host.player(1).unwrap();
        give(&mut host, player);
        let book = host.players.get(1).unwrap().slots.get(&0).unwrap();
        assert!(is_book(&host, book));
        assert_eq!(
            book.components()
                .unwrap()
                .as_nbt()
                .get(&"minecraft:custom_data".into())
                .and_then(|tag| tag.get("PublicBukkitValues"))
                .and_then(|tag| tag.get(GUIDE_KEY)),
            Some(&Tag::String(SURVIVAL.into()))
        );
        let mut counterfeit = Kind::Vanilla("minecraft:book").stack(&host, 1).unwrap();
        counterfeit
            .set_components(book.components().unwrap().as_nbt().clone())
            .unwrap();
        assert!(!is_book(&host, &counterfeit));
        let mut wrong = book.clone();
        wrong
            .apply_components(&Compound::from([(
                "minecraft:custom_data".into(),
                Tag::compound([(
                    "PublicBukkitValues",
                    Tag::compound([(GUIDE_KEY, Tag::String("CHEAT_MODE".into()))]),
                )]),
            )]))
            .unwrap();
        assert!(!is_book(&host, &wrong));
    }

    #[test]
    fn basic_machine_page_and_real_recipe_use_the_paper_slots() {
        let mut host = FakeHost::new();
        let materials = [
            "minecraft:gray_stained_glass_pane",
            "minecraft:barrier",
            "minecraft:crafting_table",
            "minecraft:dispenser",
            "minecraft:bricks",
        ];
        host.registries
            .resolve_items(materials.map(|id| Identifier::parse(id).unwrap()))
            .unwrap();
        let basic = GROUPS
            .iter()
            .find(|group| group.key == "basicMachines")
            .unwrap();
        let menu = render(&host, None, Page::Group(basic, 1));
        assert_eq!(menu.slots.len(), 54);
        let craft = lookup("ENHANCED_CRAFTING_TABLE").unwrap();
        assert!(menu.actions.iter().any(|action| matches!(
            action, Some(Action::Push(Page::Recipe(entry))) if std::ptr::eq(*entry, craft)
        )));
        assert!(menu.actions.iter().any(|action| matches!(
            action, Some(Action::Research(topic)) if topic.id == "grind_stone"
        )));
        let learned = Compound::from([(
            "PlayerPersisted".into(),
            Tag::compound([("slimefun:grind_stone", Tag::Byte(1))]),
        )]);
        let menu = render(&host, Some(&learned), Page::Group(basic, 1));
        let grind = lookup("GRIND_STONE").unwrap();
        assert!(menu.actions.iter().any(|action| matches!(
            action, Some(Action::Push(Page::Recipe(entry))) if std::ptr::eq(*entry, grind)
        )));
        let recipe = render(&host, None, Page::Recipe(craft));
        let shown = |slot: usize| {
            recipe.slots[slot]
                .as_ref()
                .map(|item| item.item().id(&host))
        };
        assert_eq!(shown(10).as_deref(), Some("minecraft:bricks"));
        assert_eq!(shown(13).as_deref(), Some("minecraft:crafting_table"));
        assert_eq!(shown(16).as_deref(), Some("minecraft:crafting_table"));
        assert_eq!(shown(22).as_deref(), Some("minecraft:dispenser"));
    }
}
