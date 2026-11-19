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

fn book_mode(host: &dyn Host, item: &ItemStack) -> Option<&'static str> {
    if item.item().id(host) != "minecraft:enchanted_book" {
        return None;
    }
    match item
        .components()?
        .as_nbt()
        .get(&"minecraft:custom_data".into())?
        .get("PublicBukkitValues")?
        .get(GUIDE_KEY)?
    {
        Tag::String(mode) if mode.to_string_lossy() == SURVIVAL => Some(SURVIVAL),
        Tag::String(mode) if mode.to_string_lossy() == "CHEAT_MODE" => Some("CHEAT_MODE"),
        _ => None,
    }
}

fn is_book(host: &dyn Host, item: &ItemStack) -> bool {
    book_mode(host, item) == Some(SURVIVAL)
}

pub(crate) fn is_guide_book(host: &dyn Host, item: &ItemStack) -> bool {
    book_mode(host, item).is_some()
}

pub(crate) fn research_topics() -> impl Iterator<Item = &'static Research> {
    catalog::research_topics()
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
    show_hidden_groups_in_search: AtomicBool,
    learning: learning::Learning,
}

impl Default for Guide {
    fn default() -> Self {
        Self {
            readers: Mutex::new(BTreeMap::new()),
            receive_on_first_join: AtomicBool::new(true),
            show_hidden_groups_in_search: AtomicBool::new(false),
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
        let show_hidden = section
            .and_then(|section| section.get("show-hidden-item-groups-in-search"))
            .and_then(toml::Value::as_bool)
            .unwrap_or(false);
        self.show_hidden_groups_in_search
            .store(show_hidden, Ordering::Relaxed);
    }

    pub(crate) fn first_join(&self, host: &mut dyn ServerHost, uuid: u128) {
        if self.receive_on_first_join.load(Ordering::Relaxed)
            && let Some(player) = host.player(uuid)
        {
            give(host, player);
        }
    }
    pub(crate) fn announce_shared_research(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        name: &str,
    ) {
        self.learning.announce_completion(host, player, name);
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

    fn show(&self, host: &mut dyn ServerHost, player: PlayerRef, mut path: Vec<Page>) {
        let month = host.server_local_minute()[1];
        if matches!(path.last(), Some(Page::Group(group, _)) if !group.in_season(month)) {
            path = vec![Page::Main];
        }
        let Some(page) = path.last() else {
            return;
        };
        let research = player.persistent_data(host);
        let show_hidden = self.show_hidden_groups_in_search.load(Ordering::Relaxed);
        let mut menu = render(host, research.as_ref(), page, month, show_hidden);
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

fn render(
    host: &dyn Host,
    saved: Option<&Compound>,
    screen: &Page,
    month: i32,
    show_hidden: bool,
) -> Menu {
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
            for (slot, group) in GROUPS
                .iter()
                .filter(|group| group.in_season(month))
                .filter(|group| group_entries(group).next().is_some())
                .enumerate()
            {
                let locked_parent = group
                    .parent
                    .and_then(|key| GROUPS.iter().find(|parent| parent.key == key))
                    .filter(|parent| !group_entries(parent).all(|entry| unlocked(saved, entry)));
                let name = format!("&{}{}", color_code(group.color), group.name);
                let shown = if let Some(parent) = locked_parent {
                    icon(
                        host,
                        "minecraft:barrier",
                        &format!("&4Locked &7- &f{}", group.name),
                        &[
                            "",
                            "&fComplete these categories first:",
                            "",
                            &format!("&7{}", parent.name),
                        ],
                    )
                } else {
                    icon(host, group.icon, &name, &[])
                };
                menu.set(
                    9 + slot,
                    shown,
                    locked_parent
                        .is_none()
                        .then_some(Action::Push(Page::Group(group, 1))),
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
            for (index, (slot, input)) in INGREDIENT_SLOTS
                .into_iter()
                .zip(entry.ingredients)
                .enumerate()
            {
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
                        input.and_then(|item| ingredient_icon(host, entry, index, item)),
                        linked.map(|entry| Action::Push(Page::Recipe(entry))),
                    );
                }
            }
            menu.slots[10] = method_icon(host, entry);
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
            for (slot, (group, entry)) in search_matches(query, month, show_hidden).enumerate() {
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

pub(crate) fn strip_colors(input: &str) -> String {
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

fn search_matches(
    query: &str,
    month: i32,
    show_hidden: bool,
) -> impl Iterator<Item = (&'static Group, &'static Entry)> {
    let query = strip_colors(query).to_lowercase();
    GROUPS
        .iter()
        .filter(move |group| show_hidden || group.in_season(month))
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
        "green" => 'a',
        "gold" => '6',
        "light_purple" => 'd',
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

fn method_icon(host: &dyn Host, entry: &Entry) -> Option<ItemStack> {
    let method = entry.recipe_type;
    if matches!(method.icon, Kind::Vanilla(_)) {
        let lore = if entry.label.is_some() {
            &["", "&a&oBuild it in the World"][..]
        } else {
            method.lore
        };
        return icon(
            host,
            method.icon.material(),
            &format!("&b{}", method.name),
            lore,
        );
    }
    let mut stack = method.icon.stack(host, 1)?;
    if !method.lore.is_empty() {
        let mut lore = stack
            .components()
            .and_then(|patch| patch.as_nbt().get(&"minecraft:lore".into()))
            .and_then(|tag| match tag {
                Tag::List(lines) => Some(lines.clone()),
                _ => None,
            })
            .unwrap_or_default();
        lore.extend(method.lore.iter().map(|line| colored_lore(line)));
        stack
            .apply_components(&Compound::from([(
                "minecraft:lore".into(),
                Tag::List(lore),
            )]))
            .ok()?;
    }
    Some(stack)
}

fn ingredient_icon(
    host: &dyn Host,
    entry: &Entry,
    index: usize,
    item: crate::recipes::Stack,
) -> Option<ItemStack> {
    let mut stack = item.kind.stack(host, item.count)?;
    if let Some(visual) = entry.visuals.iter().find(|visual| visual.slot == index) {
        let mut parts =
            Compound::from([("minecraft:custom_name".into(), colored_lore(visual.label))]);
        if let Some(profile) = &visual.profile {
            parts.insert(
                "minecraft:profile".into(),
                Tag::compound([
                    ("id", Tag::IntArray(profile.profile.to_vec())),
                    (
                        "properties",
                        Tag::List(vec![Tag::compound([
                            ("name", Tag::String("textures".into())),
                            ("value", Tag::String(profile.value.into())),
                        ])]),
                    ),
                ]),
            );
        }
        stack.apply_components(&parts).ok()?;
    }
    Some(stack)
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
        assert!(is_guide_book(&host, book));
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
        assert!(!is_guide_book(&host, &counterfeit));
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
        assert!(is_guide_book(&host, &wrong));
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
        let menu = render(&host, None, &Page::Group(basic, 1), 1, false);
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
        let menu = render(&host, Some(&learned), &Page::Group(basic, 1), 1, false);
        let grind = lookup("GRIND_STONE").unwrap();
        assert!(menu.actions.iter().any(|action| matches!(
            action, Some(Action::Push(Page::Recipe(entry))) if std::ptr::eq(*entry, grind)
        )));
        let recipe = render(&host, None, &Page::Recipe(craft), 1, false);
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
    fn exact_paper_machine_matrices_and_item_sources_are_visible_in_the_guide() {
        const MATERIALS: &[&str] = &[
            "minecraft:gray_stained_glass_pane",
            "minecraft:comparator",
            "minecraft:name_tag",
            "minecraft:stonecutter",
            "minecraft:smooth_stone_slab",
            "minecraft:iron_block",
            "minecraft:oak_trapdoor",
            "minecraft:bowl",
            "minecraft:cauldron",
            "minecraft:piston",
            "minecraft:chest",
            "minecraft:blast_furnace",
            "minecraft:bricks",
            "minecraft:diamond_block",
            "minecraft:golden_pickaxe",
            "minecraft:diamond_pickaxe",
            "minecraft:iron_sword",
            "minecraft:player_head",
            "minecraft:ghast_tear",
            "minecraft:gravel",
            "minecraft:green_terracotta",
            "minecraft:green_stained_glass",
            "minecraft:wheat",
        ];
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(MATERIALS.iter().map(|id| Identifier::parse(id).unwrap()))
            .unwrap();
        let table = lookup("TABLE_SAW").unwrap();
        let menu = render(&host, None, &Page::Recipe(table), 1, false);
        for (slot, material) in [
            (12, "smooth_stone_slab"),
            (13, "stonecutter"),
            (14, "smooth_stone_slab"),
            (22, "iron_block"),
        ] {
            assert_eq!(
                menu.slots[slot].as_ref().unwrap().item().id(&host),
                format!("minecraft:{material}")
            );
        }
        assert_eq!(table.research.unwrap().cost, 4);
        let pan = lookup("AUTOMATED_PANNING_MACHINE").unwrap();
        assert_eq!(
            pan.ingredients[4].unwrap().kind.material(),
            "minecraft:oak_trapdoor"
        );
        assert_eq!(
            pan.ingredients[7].unwrap().kind.material(),
            "minecraft:cauldron"
        );
        for (id, base, cost) in [
            ("INDUSTRIAL_MINER", "minecraft:iron_block", 28),
            ("ADVANCED_INDUSTRIAL_MINER", "minecraft:diamond_block", 36),
        ] {
            let miner = lookup(id).unwrap();
            assert_eq!(miner.research.unwrap().cost, cost);
            let shown = render(&host, None, &Page::Recipe(miner), 1, false);
            assert_eq!(
                shown.slots[13].as_ref().unwrap().item().id(&host),
                "minecraft:chest"
            );
            assert_eq!(shown.slots[21].as_ref().unwrap().item().id(&host), base);
            assert_eq!(
                shown.slots[22].as_ref().unwrap().item().id(&host),
                "minecraft:blast_furnace"
            );
            assert_eq!(shown.slots[23].as_ref().unwrap().item().id(&host), base);
            for slot in [12, 14] {
                let parts = shown.slots[slot].as_ref().unwrap().components().unwrap();
                assert_eq!(
                    parts.as_nbt().get(&"minecraft:custom_name".into()),
                    Some(&colored_lore("Piston (facing up)"))
                );
            }
        }
        for row in crate::food_machines::PRODUCE {
            let food = lookup(row.food.id()).unwrap();
            assert_eq!(food.item.count, 2);
            assert_eq!(food.ingredients[0].unwrap().kind, crate::food_machines::CAN);
            assert_eq!(food.ingredients[1].unwrap().kind, row.crop);
            assert_eq!(food.recipe_type.name, "Food Fabricator");
            assert_eq!(food.research.unwrap().id, "organic_food");
            let fertilizer = lookup(row.fertilizer.id()).unwrap();
            assert_eq!(fertilizer.item.count, 2);
            assert_eq!(fertilizer.ingredients[0].unwrap().kind, row.food);
            assert_eq!(fertilizer.recipe_type.name, "Food Composter");
            assert_eq!(fertilizer.research.unwrap().id, "organic_fertilizer");
        }
        let tin = lookup("TIN_DUST").unwrap();
        assert_eq!(tin.recipe_type.name, "Ore Washer");
        assert_eq!(tin.ingredients[0].unwrap().kind, crate::items::SIFTED_ORE);
        let sifted = lookup("SIFTED_ORE").unwrap();
        assert_eq!(sifted.recipe_type.name, "Gold Pan");
        assert_eq!(
            sifted.ingredients[0].unwrap().kind.material(),
            "minecraft:gravel"
        );
        assert_eq!(sifted.research.unwrap().id, "ore_washer");
        let circuit = lookup("BASIC_CIRCUIT_BOARD").unwrap();
        let shown = render(&host, None, &Page::Recipe(circuit), 1, false);
        let head = shown.slots[13].as_ref().unwrap();
        assert_eq!(head.item().id(&host), "minecraft:player_head");
        assert!(
            head.components()
                .unwrap()
                .as_nbt()
                .contains_key(&"minecraft:profile".into())
        );
        assert_eq!(
            shown.slots[10].as_ref().unwrap().item().id(&host),
            "minecraft:iron_sword"
        );
    }

    #[test]
    fn paper_search_accepts_display_names_and_ids_and_limits_the_first_35() {
        let craft = lookup("ENHANCED_CRAFTING_TABLE").unwrap();
        for query in ["§aENHANCED_crafting_TABLE", "enhanced crafting table"] {
            assert!(search_matches(query, 1, false).any(|(_, entry)| std::ptr::eq(entry, craft)));
        }
        assert_eq!(search_matches("", 1, false).count(), 35);
        assert_eq!(search_matches("does not exist", 1, false).count(), 0);
        assert_eq!(search_title("§bCoal"), "Searching for: Coal");
        assert_eq!(
            search_title("012345678901234567890"),
            "Searching for: 0123456789012345..."
        );
    }

    #[test]
    fn seasonal_groups_follow_the_server_month_and_paper_search_setting() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                ["minecraft:player_head", "minecraft:jack_o_lantern"]
                    .map(|material| Identifier::parse(material).unwrap()),
            )
            .unwrap();
        let groups = |month, show_hidden| {
            render(&host, None, &Page::Main, month, show_hidden)
                .actions
                .into_iter()
                .filter_map(|action| match action {
                    Some(Action::Push(Page::Group(group, _))) => Some(group.key),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        for (key, month, query) in [
            ("christmas", 12, "hot chocolate"),
            ("halloween", 10, "rainbow_wool_halloween"),
            ("valentinesDay", 2, "rainbow_wool_valentine"),
        ] {
            assert!(!groups(11, true).contains(&key));
            assert!(groups(month, false).contains(&key));
            assert!(!search_matches(query, 11, false).any(|(group, _)| group.key == key));
            assert!(search_matches(query, month, false).any(|(group, _)| group.key == key));
            assert!(search_matches(query, 11, true).any(|(group, _)| group.key == key));
        }
        let guide = Guide::default();
        let config: toml::Table = "[guide]\nshow-hidden-item-groups-in-search = true"
            .parse()
            .unwrap();
        guide.configure(config.get("guide"), None);
        assert!(guide.show_hidden_groups_in_search.load(Ordering::Relaxed));
        guide.configure(None, None);
        assert!(!guide.show_hidden_groups_in_search.load(Ordering::Relaxed));
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
