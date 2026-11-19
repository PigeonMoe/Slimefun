// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use mod_api::{
    Compound, Host, HostTransaction, ItemPlace, ItemResource, ItemStack, MenuClickedEvent,
    MenuClosedEvent, MenuReplace, PlayerRef, ServerHost, SoundSource, Tag, Text,
    text::{TextClick, TextStyle},
};

use crate::{
    hologram_projector::{colored_label, colored_lore},
    items::Kind,
    recipes::Research,
    research,
};

mod catalog;
mod learning;
use catalog::{Entry, GROUPS, Group, group_entries, lookup, named_group_entries};
use learning::Preference;

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

pub(crate) fn prevent_research_firework_damage(item: &ItemStack) -> bool {
    learning::research_rocket(item)
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

#[derive(Clone)]
enum Page {
    Main,
    Group(&'static Group, usize),
    Recipe(&'static Entry),
    Info,
    Search(Arc<str>),
}

#[derive(Clone)]
enum Action {
    Push(Page),
    Replace(Page),
    Back,
    Research(&'static Research),
    TogglePreference(Preference),
    SearchPrompt,
    Link(&'static str),
}

#[derive(Clone)]
struct View {
    window: i32,
    path: Vec<Page>,
    actions: [Option<Action>; 54],
}

#[derive(Default)]
struct Reader {
    history: Vec<Page>,
    view: Option<View>,
    waiting_for_search: bool,
}

pub(crate) struct Guide {
    readers: Mutex<BTreeMap<u128, Reader>>,
    receive_on_first_join: AtomicBool,
    learning: learning::Learning,
}

impl Default for Guide {
    fn default() -> Self {
        Self {
            readers: Mutex::new(BTreeMap::new()),
            receive_on_first_join: AtomicBool::new(true),
            learning: learning::Learning::default(),
        }
    }
}

impl Guide {
    pub(crate) fn configure(
        &self,
        section: Option<&toml::Value>,
        researches: Option<&toml::Value>,
    ) {
        self.learning.configure(researches);
        let receive = section
            .and_then(|section| section.get("receive-on-first-join"))
            .and_then(toml::Value::as_bool)
            .unwrap_or(true);
        self.receive_on_first_join.store(receive, Ordering::Relaxed);
    }

    pub(crate) fn first_join(&self, host: &mut dyn ServerHost, uuid: u128) {
        if self.receive_on_first_join.load(Ordering::Relaxed)
            && let Some(player) = host.player(uuid)
        {
            give(host, player);
        }
    }
    pub(crate) fn logged_in(&self, host: &dyn Host, player: PlayerRef) {
        self.learning.resume(host, player);
    }

    pub(crate) fn tick(&self, host: &mut dyn ServerHost, player: PlayerRef) {
        if let Some(page) = self.learning.tick(host, player) {
            self.show(host, player, page);
        }
    }

    fn last_path(&self, player: u128) -> Vec<Page> {
        self.readers
            .lock()
            .unwrap()
            .get(&player)
            .filter(|reader| !reader.history.is_empty())
            .map(|reader| reader.history.clone())
            .unwrap_or_else(|| vec![Page::Main])
    }

    pub(crate) fn open(&self, host: &mut dyn ServerHost, player: PlayerRef) {
        let path = self.last_path(player.uuid());
        self.show(host, player, path);
    }

    pub(crate) fn search(&self, host: &mut dyn ServerHost, player: PlayerRef, query: &str) {
        let mut history = self.last_path(player.uuid());
        history.push(Page::Search(Arc::from(query)));
        self.show(host, player, history);
    }

    pub(crate) fn chat(&self, host: &mut dyn ServerHost, player: u128, query: &str) -> bool {
        let waiting = self
            .readers
            .lock()
            .unwrap()
            .get_mut(&player)
            .is_some_and(|reader| std::mem::take(&mut reader.waiting_for_search));
        if waiting && let Some(player) = host.player(player) {
            self.search(host, player, query);
        }
        waiting
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
        let mut path = self.last_path(player.uuid());
        if player.entity().is_shift_key_down(host) == Some(true) {
            path.push(Page::Info);
        }
        self.show(host, player, path);
        true
    }

    fn show(&self, host: &mut dyn ServerHost, player: PlayerRef, path: Vec<Page>) {
        let Some(page) = path.last() else {
            return;
        };
        let research = player.persistent_data(host);
        let mut menu = render(host, research.as_ref(), page);
        if matches!(page, Page::Info) {
            let options = [Preference::Fireworks, Preference::Animation]
                .into_iter()
                .filter_map(|option| Some((option, self.learning.option(host, player, option)?)));
            for (slot, (option, enabled)) in (19..).zip(options) {
                let (material, label, description, setting) = match option {
                    Preference::Fireworks => (
                        "minecraft:firework_rocket",
                        "Fireworks",
                        &["&7will see fireworks upon researching an item."][..],
                        "fireworks",
                    ),
                    Preference::Animation => (
                        if enabled {
                            "minecraft:map"
                        } else {
                            "minecraft:paper"
                        },
                        "Learning Animation",
                        &[
                            "&7will see information about your pondering in chat",
                            "&7upon researching an item.",
                        ][..],
                        "learning animation",
                    ),
                };
                let text = if enabled { "&aYes" } else { "&4No" };
                let verb = if enabled { "disable" } else { "enable" };
                let title = format!("&b{label}: {text}");
                let click = format!("&eClick to {verb} your {setting}");
                let mut lore = vec!["", "&7You can now toggle whether you"];
                lore.extend_from_slice(description);
                lore.extend(["", click.as_str()]);
                menu.set(
                    slot,
                    icon(host, material, &title, &lore),
                    Some(Action::TogglePreference(option)),
                );
            }
        }
        if path.len() > 1 {
            let back = usize::from(!matches!(page, Page::Recipe(_) | Page::Info));
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
            Page::Info => Text::literal("Settings & Info"),
            Page::Search(query) => Text::literal(search_title(query)),
            _ => Text::literal(TITLE),
        };
        if let Ok(Some(window)) =
            host.open_menu(player.uuid(), 6, title, menu.slots, MenuReplace::CloseFirst)
        {
            let settings = matches!(page, Page::Info);
            let len = path.len() - usize::from(settings);
            let mut readers = self.readers.lock().unwrap();
            let reader = readers.entry(player.uuid()).or_default();
            reader.history = path[..len].to_vec();
            reader.view = Some(View {
                window,
                path,
                actions: menu.actions,
            });
            reader.waiting_for_search = false;
            drop(readers);
            let (sound, volume, pitch) = if settings {
                ("minecraft:block.note_block.harp", 0.7, 0.7)
            } else {
                ("minecraft:item.book.page_turn", 1.0, 1.0)
            };
            player.play_sound(host, sound, SoundSource::Master, volume, pitch);
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
            .readers
            .lock()
            .unwrap()
            .get(&event.uuid)
            .and_then(|reader| reader.view.as_ref())
            .filter(|view| view.window == event.window)
            .cloned()
        else {
            return;
        };
        let Some(Some(action)) = view.actions.get(slot).cloned() else {
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
            Action::Back if event.mode == 1 && !matches!(path.last(), Some(Page::Info)) => {
                path.truncate(1);
            }
            Action::Back => {
                path.pop();
            }
            Action::Research(topic) => {
                if !learned(host, player, topic) && !self.learning.start(host, player, topic, &path)
                {
                    return;
                }
            }
            Action::TogglePreference(option) => {
                self.learning.toggle(host, player, option);
            }
            Action::SearchPrompt => {
                host.close_menu(event.uuid);
                self.readers
                    .lock()
                    .unwrap()
                    .entry(event.uuid)
                    .or_default()
                    .waiting_for_search = true;
                let _ = player.tell(host, colored_label("&bWhat would you like to search for?"));
                return;
            }
            Action::Link(url) => {
                host.close_menu(event.uuid);
                let mut style = TextStyle::default();
                style.click = Some(TextClick::OpenUrl(url.to_owned()));
                style.color = Some("aqua".into());
                style.underlined = Some(true);
                let _ = player.tell(host, Text::literal(url).with_style(style));
                return;
            }
        }
        self.show(host, player, path);
    }

    pub(crate) fn closed(&self, event: &MenuClosedEvent) {
        let mut readers = self.readers.lock().unwrap();
        if let Some(reader) = readers.get_mut(&event.uuid)
            && reader
                .view
                .as_ref()
                .is_some_and(|view| view.window == event.window)
        {
            reader.view = None;
        }
    }

    pub(crate) fn logout(&self, player: u128) {
        self.learning.forget(player);
        self.readers.lock().unwrap().remove(&player);
    }

    pub(crate) fn clear(&self) {
        self.learning.clear();
        self.readers.lock().unwrap().clear();
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

fn render(host: &dyn Host, saved: Option<&Compound>, screen: &Page) -> Menu {
    let mut menu = Menu {
        slots: vec![None; 54],
        actions: std::array::from_fn(|_| None),
    };
    let pane = icon(host, "minecraft:gray_stained_glass_pane", " ", &[]);
    if matches!(screen, Page::Info) {
        for slot in [
            1, 3, 5, 7, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 26, 27, 35, 36, 37, 38, 39, 40, 41,
            42, 43, 44, 45, 46, 48, 50, 52, 53,
        ] {
            menu.slots[slot] = pane.clone();
        }
    } else {
        for slot in (0..9).chain(45..54) {
            menu.slots[slot] = pane.clone();
        }
        menu.set(
            1,
            icon(host, "minecraft:comparator", "&eSettings & Info", &[]),
            Some(Action::Push(Page::Info)),
        );
        menu.set(
            7,
            icon(
                host,
                "minecraft:name_tag",
                "&7Search...",
                &["", "&7⇨ &bClick to search for an item"],
            ),
            Some(Action::SearchPrompt),
        );
    }
    match screen {
        Page::Main => {
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
            pagination(host, &mut menu, group, *page, pages);
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
            menu.slots[2] = icon(
                host,
                "minecraft:knowledge_book",
                "&cSlimefun4 Contributors",
                &[
                    "",
                    "&7Slimefun is an open-source project",
                    "&7maintained by a community of people.",
                ],
            );
            let game = format!("&fMinecraft: &a{}", host.game_version());
            let plugin = format!("&fSlimefun: &a{}", env!("CARGO_PKG_VERSION"));
            menu.slots[4] = icon(
                host,
                "minecraft:writable_book",
                "&aInstalled versions",
                &[
                    "&7&oThese are very important when reporting bugs!",
                    "",
                    &game,
                    &plugin,
                ],
            );
            for (slot, material, title, url, tooltip) in [
                (
                    6,
                    "minecraft:comparator",
                    "&eSource Code",
                    "https://github.com/Slimefun/Slimefun4",
                    "&7⇨ &eClick to go to GitHub",
                ),
                (
                    8,
                    "minecraft:knowledge_book",
                    "&3Slimefun4 Wiki",
                    "https://github.com/Slimefun/Slimefun4/wiki",
                    "&7⇨ &eClick to go to the official Slimefun Wiki",
                ),
                (
                    47,
                    "minecraft:bookshelf",
                    "&3Addons for Slimefun4",
                    "https://github.com/Slimefun/Slimefun4/wiki/Addons",
                    "&7⇨ &eClick to see all available addons for Slimefun4",
                ),
            ] {
                menu.set(
                    slot,
                    icon(host, material, title, &["", tooltip]),
                    Some(Action::Link(url)),
                );
            }
        }
        Page::Search(query) => {
            for (slot, (group, entry)) in search_matches(query).enumerate() {
                let mut item = entry_icon(host, entry);
                let group = colored_lore(&format!("&8⇨ &f{}", group.name));
                if let Some(item) = &mut item {
                    let _ = item.apply_components(&Compound::from([(
                        "minecraft:lore".into(),
                        Tag::List(vec![Tag::String("".into()), group]),
                    )]));
                }
                menu.set(9 + slot, item, Some(Action::Push(Page::Recipe(entry))));
            }
        }
    }
    menu
}

fn strip_colors(input: &str) -> String {
    let mut chars = input.chars().peekable();
    let mut plain = String::new();
    while let Some(c) = chars.next() {
        if c == '§'
            && chars.peek().is_some_and(|code| {
                matches!(
                    code.to_ascii_lowercase(), '0'..='9' | 'a'..='f' | 'k'..='o' | 'r' | 'x'
                )
            })
        {
            chars.next();
        } else {
            plain.push(c);
        }
    }
    plain
}

fn search_title(query: &str) -> String {
    let mut plain = strip_colors(query);
    if plain.encode_utf16().count() > 19 {
        let mut length = 0;
        plain = plain
            .chars()
            .take_while(|c| {
                length += c.len_utf16();
                length <= 16
            })
            .collect();
        plain.push_str("...");
    }
    format!("Searching for: {plain}")
}

fn search_matches(query: &str) -> impl Iterator<Item = (&'static Group, &'static Entry)> {
    let query = strip_colors(query).to_lowercase();
    GROUPS
        .iter()
        .flat_map(|group| named_group_entries(group).map(move |(id, entry)| (group, id, entry)))
        .filter_map(move |(group, id, entry)| {
            (id.to_ascii_lowercase().contains(&query)
                || strip_colors(&entry_name(entry))
                    .to_lowercase()
                    .contains(&query))
            .then_some((group, entry))
        })
        .take(35)
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
            Kind::Vanilla(id) | Kind::Potion(id) => {
                id.trim_start_matches("minecraft:").replace('_', " ")
            }
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
    fn first_join_uses_the_paper_server_option_and_gives_the_same_book() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse("minecraft:enchanted_book").unwrap()])
            .unwrap();
        let level = host.level.level();
        host.players.join(1, 100, level);
        let guide = Guide::default();
        let config = "[guide]\nreceive-on-first-join = false"
            .parse::<toml::Table>()
            .unwrap();
        guide.configure(config.get("guide"), None);
        guide.first_join(&mut host, 1);
        assert!(host.players.get(1).unwrap().slots.is_empty());
        guide.configure(None, None);
        guide.first_join(&mut host, 1);
        assert!(is_book(
            &host,
            host.players.get(1).unwrap().slots.get(&0).unwrap()
        ));
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
        let menu = render(&host, None, &Page::Group(basic, 1));
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
        let menu = render(&host, Some(&learned), &Page::Group(basic, 1));
        let grind = lookup("GRIND_STONE").unwrap();
        assert!(menu.actions.iter().any(|action| matches!(
            action, Some(Action::Push(Page::Recipe(entry))) if std::ptr::eq(*entry, grind)
        )));
        let recipe = render(&host, None, &Page::Recipe(craft));
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

    #[test]
    fn paper_search_accepts_display_names_and_ids_and_limits_the_first_35() {
        let craft = lookup("ENHANCED_CRAFTING_TABLE").unwrap();
        for query in ["§aENHANCED_crafting_TABLE", "enhanced crafting table"] {
            assert!(search_matches(query).any(|(_, entry)| std::ptr::eq(entry, craft)));
        }
        assert_eq!(search_matches("").count(), 35);
        assert_eq!(search_matches("does not exist").count(), 0);
        assert_eq!(search_title("§bCoal"), "Searching for: Coal");
        assert_eq!(
            search_title("012345678901234567890"),
            "Searching for: 0123456789012345..."
        );
    }

    #[test]
    fn gui_close_and_old_windows_keep_the_readers_last_page() {
        let guide = Guide::default();
        let history = vec![Page::Main, Page::Search("jetpack".into())];
        guide.readers.lock().unwrap().insert(
            7,
            Reader {
                history: history.clone(),
                view: Some(View {
                    window: 3,
                    path: history,
                    actions: std::array::from_fn(|_| None),
                }),
                waiting_for_search: false,
            },
        );
        guide.closed(&MenuClosedEvent { uuid: 7, window: 2 });
        assert!(
            guide
                .readers
                .lock()
                .unwrap()
                .get(&7)
                .unwrap()
                .view
                .is_some()
        );
        guide.closed(&MenuClosedEvent { uuid: 7, window: 3 });
        assert!(
            guide
                .readers
                .lock()
                .unwrap()
                .get(&7)
                .unwrap()
                .view
                .is_none()
        );
        assert!(
            matches!(guide.last_path(7).as_slice(), [Page::Main, Page::Search(query)] if &**query == "jetpack")
        );
        guide.logout(7);
        assert!(matches!(guide.last_path(7).as_slice(), [Page::Main]));
    }
}
