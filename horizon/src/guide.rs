// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use mod_api::{
    ComponentId, Compound, Hand, Host, HostTransaction, ItemPlace, ItemResource, ItemStack, MenuClickedEvent,
    MenuClosedEvent, MenuReplace, PermissionLevel, PlayerRef, ServerHost, SoundSource, Tag, Text,
    text::{TextClick, TextStyle},
};

use crate::{
    availability,
    hologram_projector::{colored_label, colored_lore},
    items::{HeadTexture, Kind},
    recipes::Research,
    research,
    ticker_policy::Tickers,
};

mod catalog;
mod learning;
mod locale;
use catalog::{Entry, GROUPS, Group, group_entries, lookup, named_group_entries};
use learning::Preference;
pub(crate) use learning::launch_random_fireworks;
pub(crate) use locale::localize_item_components;
pub(crate) use locale::research_name;

const TITLE: &str = "远古工艺之书";
const BOOK_LORE: &[&str] = &[
    "",
    "&e右键 &8⇨ &7浏览物品",
    "&e潜行并右键 &8⇨ &7打开设置与关于",
];
const GUIDE_KEY: &str = "slimefun:slimefun_guide_mode";
const SURVIVAL: &str = "SURVIVAL_MODE";
const DISCOVERY_SLOTS: usize = 36;
const INGREDIENT_SLOTS: [usize; 9] = [12, 13, 14, 21, 22, 23, 30, 31, 32];
const RECIPE_METHOD_SLOT: usize = 19;
const RECIPE_OUTPUT_SLOT: usize = 25;

fn book(host: &dyn Host) -> Option<ItemStack> {
    let mut item = icon(
        host,
        "minecraft:enchanted_book",
        "&a远古工艺之书",
        BOOK_LORE,
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

fn rename_old_book(host: &mut dyn ServerHost, player: PlayerRef, hand: Hand, stack: &ItemStack) {
    let name = stack
        .components()
        .and_then(|parts| parts.as_nbt().get(&"minecraft:custom_name".into()));
    if name != Some(&colored_lore("&aSlimefun Guide &7(Chest GUI)")) {
        return;
    }
    let held = ItemPlace::PlayerHand { player: player.uuid(), hand };
    if held.stack(host).as_ref() != Some(stack) {
        return;
    }
    let slot = match hand {
        Hand::Main => player
            .selected_slot(host)
            .and_then(|slot| u32::try_from(slot).ok())
            .filter(|slot| *slot < 9),
        Hand::Off => Some(40),
    };
    let Some(slot) = slot else { return };
    let mut updated = stack.clone();
    if updated.apply_components(&Compound::from([
        ("minecraft:custom_name".into(), colored_lore("&a远古工艺之书")),
        ("minecraft:lore".into(), Tag::List(BOOK_LORE.iter().map(|line| colored_lore(line)).collect())),
    ])).is_ok() {
        let _ = player.set_inventory_slot(host, slot, Some(updated));
    }
}

pub(crate) fn is_guide_book(host: &dyn Host, item: &ItemStack) -> bool {
    book_mode(host, item).is_some()
}

pub(crate) fn blocks_vanilla_station(item: &ItemStack) -> bool {
    let tickers = Tickers::current();
    crate::items::item_id(item).is_some_and(|id| {
        availability::visible(tickers, None, &id) && catalog::blocks_vanilla_station(&id)
    })
}

pub(crate) fn research_available(id: &str, tickers: Tickers, dimension: Option<&str>) -> bool {
    if dimension.is_some_and(|dimension| !availability::world_enabled(dimension)) {
        return false;
    }
    let mut items = catalog::items_for_research(id).peekable();
    items.peek().is_none() || items.any(|id| availability::visible(tickers, dimension, id))
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
                ItemPlace::PlayerSlot { player: player.uuid(), slot }
                    .insert(host, tx, &resource, 1) == 1
            })
            .then_some(())
            .ok_or(())
    });
}

fn give_cheat_item(host: &mut dyn ServerHost, player: PlayerRef, stack: &ItemStack, full_stack: bool) -> bool {
    let resource = ItemResource::of(stack);
    let key = ComponentId::new("minecraft:max_stack_size").expect("built-in item component");
    let limit = match resource.value(host, &key) {
        Some(Tag::Int(max @ 1..=99)) => max as u32,
        _ => host.max_stack_size(resource.item()).unwrap_or(1),
    };
    let count = if full_stack { limit } else { 1 };
    host.transaction(|host, tx| {
        let mut remaining = count;
        for slot in 0..36 {
            let place = ItemPlace::PlayerSlot {
                player: player.uuid(),
                slot,
            };
            let room = match place.stack(host) {
                Some(existing) if resource.matches(&existing) => limit.saturating_sub(existing.count()),
                Some(_) => 0,
                None => limit,
            };
            let amount = room.min(remaining);
            if amount != 0 {
                remaining -= place.insert(host, tx, &resource, amount);
            }
            if remaining == 0 {
                break;
            }
        }
        (remaining < count).then_some(()).ok_or(())
    })
    .is_ok()
}

fn can_cheat(host: &dyn Host, player: PlayerRef) -> bool {
    player
        .permission_level(host)
        .is_some_and(|level| level >= PermissionLevel::Gamemasters.level())
}

#[derive(Clone)]
enum Page {
    Main,
    Cheat,
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
    EnterCheat,
    LeaveCheat,
    Grant(&'static Entry),
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
    waiting_for_search: Option<Vec<Page>>,
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
            && player.dimension(host).is_none_or(|dimension| availability::world_enabled(&dimension.to_string()))
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
        let path = self
            .readers
            .lock()
            .unwrap()
            .get_mut(&player)
            .and_then(|reader| reader.waiting_for_search.take());
        if let Some(mut path) = path {
            if let Some(player) = host.player(player) {
                path.push(Page::Search(Arc::from(query)));
                self.show(host, player, path);
            }
            return true;
        }
        false
    }

    pub(crate) fn right_click(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        hand: Hand,
        stack: &ItemStack,
    ) -> bool {
        if !is_book(host, stack) {
            return false;
        }
        rename_old_book(host, player, hand, stack);
        let mut path = self.last_path(player.uuid());
        if player.entity().is_shift_key_down(host) == Some(true) {
            path.push(Page::Info);
        }
        self.show(host, player, path);
        true
    }

    fn show(&self, host: &mut dyn ServerHost, player: PlayerRef, mut path: Vec<Page>) {
        if !availability::world_enabled(host.dimension()) {
            availability::Denied::World.tell(host, player);
            return;
        }
        let cheat = matches!(path.first(), Some(Page::Cheat));
        if cheat && !can_cheat(host, player) {
            let _ = player.tell(host, colored_label("&c只有二级及以上管理员可以使用作弊模式。"));
            return;
        }
        let month = host.server_local_minute()[1];
        let availability = Availability::for_dimension(host.dimension());
        if matches!(path.last(), Some(Page::Group(group, _)) if !group.in_season(month))
            || matches!(path.last(), Some(Page::Recipe(entry)) if !availability.allows_search(entry))
        {
            path.truncate(1);
        }
        let Some(page) = path.last() else {
            return;
        };
        let research = player.persistent_data(host);
        let show_hidden = self.show_hidden_groups_in_search.load(Ordering::Relaxed);
        let mut menu = render(
            host,
            research.as_ref(),
            page,
            month,
            show_hidden,
            availability,
            cheat,
        );
        if matches!(page, Page::Info) {
            let options = [Preference::Fireworks, Preference::Animation]
                .into_iter()
                .filter_map(|option| Some((option, self.learning.option(host, player, option)?)));
            for (slot, (option, enabled)) in (19..).zip(options) {
                let (material, label, description) = match option {
                    Preference::Fireworks => (
                        "minecraft:firework_rocket",
                        "研究烟花",
                        "&7在解锁研究时燃放烟花。",
                    ),
                    Preference::Animation => (
                        if enabled {
                            "minecraft:map"
                        } else {
                            "minecraft:paper"
                        },
                        "解锁动画",
                        "&7在屏幕中央显示解锁进度。",
                    ),
                };
                let text = if enabled { "&a开启" } else { "&4关闭" };
                let click = if enabled { "&e点击关闭" } else { "&e点击开启" };
                let title = format!("&b{label}：{text}");
                menu.set(
                    slot,
                    icon(host, material, &title, &["", description, "", click]),
                    Some(Action::TogglePreference(option)),
                );
            }
            if can_cheat(host, player) {
                menu.set(
                    23,
                    icon(
                        host,
                        "minecraft:command_block",
                        "&c作弊模式",
                        &["&7点击领取物品，潜行点击领取一组。", "&7仅二级及以上管理员可用。"],
                    ),
                    Some(Action::EnterCheat),
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
                    "&7⇦ 返回",
                    &[
                        "",
                        "&f左键：&7返回上一页",
                        "&f潜行并左键：&7返回主页",
                    ],
                ),
                Some(Action::Back),
            );
        }
        let title = match page {
            Page::Info => Text::literal("设置与关于"),
            Page::Search(query) => Text::literal(search_title(query)),
            _ if cheat => Text::literal("远古工艺之书 · 作弊模式"),
            _ => Text::literal(TITLE),
        };
        if let Ok(Some(window)) =
            host.open_menu(player.uuid(), 6, title, menu.slots, MenuReplace::CloseFirst)
        {
            let settings = matches!(page, Page::Info);
            let len = path.len() - usize::from(settings);
            let mut readers = self.readers.lock().unwrap();
            let reader = readers.entry(player.uuid()).or_default();
            if !cheat {
                reader.history = path[..len].to_vec();
            }
            reader.view = Some(View {
                window,
                path,
                actions: menu.actions,
            });
            reader.waiting_for_search = None;
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
        if matches!(path.first(), Some(Page::Cheat)) && !can_cheat(host, player) {
            host.close_menu(event.uuid);
            let _ = player.tell(host, colored_label("&c只有二级及以上管理员可以使用作弊模式。"));
            return;
        }
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
                let availability = Availability::for_dimension(host.dimension());
                if (topic.id == crate::recipes::CHEST_RESEARCH.id && !availability.output_chest)
                    || !research_available(topic.id, availability.tickers, availability.dimension)
                {
                    self.show(host, player, path);
                    return;
                }
                if !learned(host, player, topic) && !self.learning.start(host, player, topic, &path)
                {
                    return;
                }
            }
            Action::TogglePreference(option) => {
                self.learning.toggle(host, player, option);
            }
            Action::EnterCheat => path = vec![Page::Cheat],
            Action::LeaveCheat => path = self.last_path(player.uuid()),
            Action::Grant(entry) => {
                if !availability::world_enabled(host.dimension())
                    || !Availability::for_dimension(host.dimension()).allows(entry)
                {
                    return;
                }
                if !matches!(entry.item.kind, Kind::Slimefun(_)) {
                    let _ = player.tell(host, colored_label("&c多方块结构需要手动搭建。"));
                    return;
                }
                if let Some(stack) = entry.item.kind.stack(host, 1) {
                    if !give_cheat_item(host, player, &stack, event.mode == 1) {
                        let _ = player.tell(host, colored_label("&c背包已满。"));
                    }
                }
                return;
            }
            Action::SearchPrompt => {
                host.close_menu(event.uuid);
                self.readers
                    .lock()
                    .unwrap()
                    .entry(event.uuid)
                    .or_default()
                    .waiting_for_search = Some(path);
                let _ = player.tell(host, colored_label("&b请在聊天栏输入要搜索的物品。"));
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

    pub(crate) fn cancel_research_animation(&self, player: PlayerRef) {
        self.learning.forget(player.uuid());
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

#[derive(Clone, Copy)]
struct Availability<'a> {
    output_chest: bool,
    tickers: Tickers,
    dimension: Option<&'a str>,
}

impl<'a> Availability<'a> {
    fn for_dimension(dimension: &'a str) -> Self {
        Self {
            output_chest: crate::output_chest::enabled(dimension),
            tickers: Tickers::current(),
            dimension: Some(dimension),
        }
    }

    fn allows(self, entry: &Entry) -> bool {
        (self.output_chest || entry.item.kind != crate::items::OUTPUT_CHEST)
            && availability::visible(self.tickers, self.dimension, entry.id)
    }

    fn allows_search(self, entry: &Entry) -> bool {
        // Paper searches its enabled global registry, unlike its group pages.
        Self { dimension: None, ..self }.allows(entry)
    }
}

fn render(
    host: &dyn Host,
    saved: Option<&Compound>,
    screen: &Page,
    month: i32,
    show_hidden: bool,
    availability: Availability<'_>,
    cheat: bool,
) -> Menu {
    let visible_entry = |entry: &&Entry| availability.allows(entry);
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
            if cheat {
                icon(host, "minecraft:enchanted_book", "&e返回生存指南", &[])
            } else {
                icon(host, "minecraft:comparator", "&e设置与关于", &[])
            },
            Some(if cheat { Action::LeaveCheat } else { Action::Push(Page::Info) }),
        );
        menu.set(
            7,
            icon(
                host,
                "minecraft:name_tag",
                "&7搜索……",
                &["", "&7⇨ &b点击搜索物品"],
            ),
            Some(Action::SearchPrompt),
        );
    }
    match screen {
        Page::Main | Page::Cheat => {
            for (slot, group) in GROUPS
                .iter()
                .filter(|group| group.in_season(month))
                .filter(|group| group_entries(group).any(|entry| visible_entry(&entry)))
                .enumerate()
            {
                let locked_parent = group
                    .parent
                    .and_then(|key| GROUPS.iter().find(|parent| parent.key == key))
                    .filter(|parent| {
                        !cheat &&
                        !group_entries(parent)
                            .filter(visible_entry)
                            .all(|entry| unlocked(saved, entry))
                    });
                let name = format!("&{}{}", color_code(group.color), group.name);
                let shown = if let Some(parent) = locked_parent {
                    icon(
                        host,
                        "minecraft:barrier",
                        &format!("&4未解锁 &7- &f{}", group.name),
                        &[
                            "",
                            "&f请先完成下列分类的研究：",
                            "",
                            &format!("&7{}", parent.name),
                        ],
                    )
                } else {
                    group_icon(host, group, &name)
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
            let entries: Vec<_> = group_entries(group).filter(visible_entry).collect();
            let pages = entries.len().div_ceil(DISCOVERY_SLOTS).max(1);
            let page = (*page).min(pages);
            for (slot, entry) in entries
                .into_iter()
                .skip((page - 1) * DISCOVERY_SLOTS)
                .take(DISCOVERY_SLOTS)
                .enumerate()
            {
                if let Some(topic) = entry.research.filter(|_| !cheat && !unlocked(saved, entry)) {
                    let cost = format!("&7需要 &b{} 级经验", topic.cost);
                    menu.set(
                        9 + slot,
                        icon(
                            host,
                            "minecraft:barrier",
                            &entry_name(entry),
                            &["&4&l未解锁", "", "&a点击解锁", "", &cost],
                        ),
                        Some(Action::Research(topic)),
                    );
                } else {
                    menu.set(
                        9 + slot,
                        entry_icon(host, entry, 1),
                        Some(entry_action(entry, cheat)),
                    );
                }
            }
            pagination(host, &mut menu, group, page, pages);
        }
        Page::Recipe(entry) => {
            for (index, (slot, input)) in INGREDIENT_SLOTS
                .into_iter()
                .zip(entry.ingredients)
                .enumerate()
            {
                let linked = input
                    .and_then(|stack| lookup(stack.kind.id()))
                    .filter(visible_entry);
                if let Some(locked) = linked.filter(|entry| !unlocked(saved, entry)) {
                    menu.set(
                        slot,
                        icon(
                            host,
                            "minecraft:barrier",
                            &entry_name(locked),
                            &["&4&l未解锁"],
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
            menu.slots[RECIPE_METHOD_SLOT] = method_icon(host, entry);
            menu.slots[RECIPE_OUTPUT_SLOT] = entry_icon(host, entry, entry.item.count);
        }
        Page::Info => {
            menu.slots[2] = icon(
                host,
                "minecraft:knowledge_book",
                "&cPigeonMoe 移植团队",
                &[
                    "",
                    "&7远古工艺由 PigeonMoe 移植至 Horizon。",
                    "&7感谢原版开发者和社区贡献者。",
                ],
            );
            let game = format!("&fMinecraft: &a{}", host.game_version());
            let plugin = format!("&f远古工艺：&a{}", env!("CARGO_PKG_VERSION"));
            menu.slots[4] = icon(
                host,
                "minecraft:writable_book",
                "&a已安装版本",
                &[
                    "&7&o反馈问题时请附上版本信息。",
                    "",
                    &game,
                    &plugin,
                ],
            );
            for (slot, material, title, url, tooltip) in [
                (
                    6,
                    "minecraft:comparator",
                    "&ePigeonMoe 源码与贡献者",
                    "https://github.com/PigeonMoe/Slimefun/tree/Horizon",
                    "&7⇨ &e点击打开移植仓库",
                ),
                (
                    8,
                    "minecraft:knowledge_book",
                    "&3远古工艺原版文档",
                    "https://github.com/Slimefun/Slimefun4/wiki",
                    "&7⇨ &e点击查看原版文档",
                ),
                (
                    47,
                    "minecraft:bookshelf",
                    "&3远古工艺原版附属",
                    "https://github.com/Slimefun/Slimefun4/wiki/Addons",
                    "&7⇨ &e点击查看原版附属列表",
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
            for (slot, (group, entry)) in
                search_matches(query, month, show_hidden, availability).enumerate()
            {
                let mut item = entry_icon(host, entry, 1);
                let group = colored_lore(&format!("&8⇨ &f{}", group.name));
                if let Some(item) = &mut item {
                    let lore_key = "minecraft:lore".into();
                    let mut lore = match item
                        .components()
                        .and_then(|patch| patch.as_nbt().get(&lore_key))
                    {
                        Some(Tag::List(lines)) => lines.clone(),
                        _ => Vec::new(),
                    };
                    lore.extend([Tag::String("".into()), group]);
                    let _ = item.apply_components(&Compound::from([(lore_key, Tag::List(lore))]));
                }
                menu.set(9 + slot, item, Some(entry_action(entry, cheat)));
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
    format!("搜索：{plain}")
}

fn search_matches(
    query: &str,
    month: i32,
    show_hidden: bool,
    availability: Availability<'_>,
) -> impl Iterator<Item = (&'static Group, &'static Entry)> {
    let query = strip_colors(query).to_lowercase();
    let id_query = query.replace(' ', "_");
    GROUPS
        .iter()
        .filter(move |group| show_hidden || group.in_season(month))
        .flat_map(|group| named_group_entries(group).map(move |(id, entry)| (group, id, entry)))
        .filter(move |(_, _, entry)| availability.allows_search(entry))
        .filter_map(move |(group, id, entry)| {
            (id.to_ascii_lowercase().contains(&id_query)
                || strip_colors(entry.label.unwrap_or_else(|| match entry.item.kind {
                    Kind::Slimefun(item) => item.name,
                    Kind::Vanilla(_) | Kind::Potion(_) => entry.id,
                }))
                    .to_lowercase()
                    .contains(&query)
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
            "&f⇦ 上一页",
            page.checked_sub(1).filter(|page| *page > 0),
        ),
        (52, "&f下一页 ⇨", (page < pages).then_some(page + 1)),
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

fn entry_action(entry: &'static Entry, cheat: bool) -> Action {
    if cheat {
        Action::Grant(entry)
    } else {
        Action::Push(Page::Recipe(entry))
    }
}

fn guide_stack(host: &dyn Host, kind: Kind, count: u32) -> Option<ItemStack> {
    let stack = kind.stack(host, count)?;
    match kind {
        Kind::Slimefun(item) => locale::item_icon(stack, item.id),
        _ => Some(stack),
    }
}

// The Paper categories use these actual item templates and seasonal head skins.
const CHRISTMAS_ICON: HeadTexture = HeadTexture {
    profile: [696803316, -1167118050, -1409946767, -1076270746],
    value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvMjE1YmEzMWNkZTI2NzFiOGYxNzZkZTZhOWZmZDAwODAzNWYwNTkwZDYzZWUyNDBiZTZlODkyMWNkMjAzN2E0NSJ9fX0=",
};

const VALENTINES_ICON: HeadTexture = HeadTexture {
    profile: [-1545001755, -1935133338, -1537546858, 312205723],
    value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvNTVkODk0MzFkMTRiZmVmMjA2MDQ2MWI0YTM1NjU2MTRkYzUxMTE1YzAwMWZhZTI1MDhlODY4NGJjMGFlNmE4MCJ9fX0=",
};

fn group_icon(host: &dyn Host, group: &Group, name: &str) -> Option<ItemStack> {
    let paper_item = match group.key {
        "weapons" => "BLADE_OF_VAMPIRES",
        "tools" => "SMELTERS_PICKAXE",
        "usefulItems" => "MEDIUM_BACKPACK",
        "resources" => "SYNTHETIC_SAPPHIRE",
        "food" => "FORTUNE_COOKIE",
        "armor" => "DAMASCUS_STEEL_CHESTPLATE",
        "magicalResources" => "ENDER_RUNE",
        "magicalGadgets" => "INFUSED_ELYTRA",
        "magicalArmor" => "ENDER_HELMET",
        "misc" => "CAN",
        "technicalComponents" => "HEATING_COIL",
        "technicalGadgets" => "STEEL_JETPACK",
        "gps" => "GPS_TRANSMITTER",
        "electricity" => "NUCLEAR_REACTOR",
        "androids" => "PROGRAMMABLE_ANDROID",
        "cargo" => "CARGO_MANAGER",
        "easter" => "EASTER_EGG",
        _ => "",
    };
    if let Some(mut stack) = lookup(paper_item).and_then(|entry| entry.item.kind.stack(host, 1)) {
        stack.apply_components(&Compound::from([
            ("minecraft:custom_name".into(), colored_lore(name)),
            ("minecraft:lore".into(), Tag::List(vec![])),
        ])).ok()?;
        return Some(stack);
    }
    let mut stack = icon(host, group.icon, name, &[])?;
    let seasonal = match group.key {
        "christmas" => Some(&CHRISTMAS_ICON),
        "valentinesDay" => Some(&VALENTINES_ICON),
        _ => None,
    };
    if let Some(texture) = seasonal {
        stack
            .apply_components(&Compound::from([("minecraft:profile".into(), texture.component())]))
            .ok()?;
    }
    Some(stack)
}

fn method_icon(host: &dyn Host, entry: &Entry) -> Option<ItemStack> {
    let method = entry.recipe_type;
    if matches!(method.icon, Kind::Vanilla(_)) {
        let lore = if entry.label.is_some() {
            &["", "&a&o请在世界中搭建"][..]
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
    let mut stack = guide_stack(host, method.icon, 1)?;
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
    let mut stack = guide_stack(host, item.kind, item.count)?;
    if let Some(visual) = entry.visuals.iter().find(|visual| visual.slot == index) {
        let mut parts =
            Compound::from([("minecraft:custom_name".into(), colored_lore(visual.label))]);
        if let Some(profile) = &visual.profile {
            parts.insert("minecraft:profile".into(), profile.component());
        }
        stack.apply_components(&parts).ok()?;
    }
    Some(stack)
}

fn entry_name(entry: &Entry) -> String {
    if let Some(name) = locale::item_name(entry.id) {
        return name.to_owned();
    }
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

fn entry_icon(host: &dyn Host, entry: &Entry, count: u32) -> Option<ItemStack> {
    let mut stack = guide_stack(host, entry.item.kind, count)?;
    if entry.label.is_some() {
        stack
            .apply_components(&Compound::from([(
                "minecraft:custom_name".into(),
                colored_lore(&entry_name(entry)),
            )]))
            .ok()?;
    }
    Some(stack)
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
    use mod_api::prelude::ItemsHost;
    use mod_api::{Identifier, prelude::PlayersHost, testing::FakeHost};

    #[test]
    fn vanilla_stations_keep_plain_unknown_and_vanilla_wrapped_items() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse("minecraft:glass_bottle").unwrap()])
            .unwrap();
        let normal = Kind::Vanilla("minecraft:glass_bottle")
            .stack(&host, 1)
            .unwrap();
        assert!(!blocks_vanilla_station(&normal));
        for (id, blocks) in [
            ("FLASK_OF_KNOWLEDGE", true),
            ("ENHANCED_CRAFTING_TABLE", true),
            ("QA_UNREGISTERED_FLASK", false),
            ("CHAIN_HELMET", false),
            ("flask_of_knowledge", false),
        ] {
            let mut marked = normal.clone();
            marked
                .set_components(Compound::from([(
                    "minecraft:custom_data".into(),
                    Tag::compound([(
                        "PublicBukkitValues",
                        Tag::compound([(crate::items::ID_KEY, Tag::String(id.into()))]),
                    )]),
                )]))
                .unwrap();
            assert_eq!(blocks_vanilla_station(&marked), blocks, "{id}");
        }
    }

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
    fn basic_machine_page_keeps_navigation_above_the_recipe() {
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
        let menu = render(&host, None, &Page::Group(basic, 1), 1, false, available(), false);
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
        let menu = render(
            &host,
            Some(&learned),
            &Page::Group(basic, 1),
            1,
            false,
            available(),
            false,
        );
        let grind = lookup("GRIND_STONE").unwrap();
        assert!(menu.actions.iter().any(|action| matches!(
            action, Some(Action::Push(Page::Recipe(entry))) if std::ptr::eq(*entry, grind)
        )));
        let recipe = render(&host, None, &Page::Recipe(craft), 1, false, available(), false);
        let shown = |slot: usize| {
            recipe.slots[slot]
                .as_ref()
                .map(|item| item.item().id(&host))
        };
        assert_eq!(shown(19).as_deref(), Some("minecraft:bricks"));
        assert_eq!(shown(22).as_deref(), Some("minecraft:crafting_table"));
        assert_eq!(shown(25).as_deref(), Some("minecraft:crafting_table"));
        assert_eq!(shown(31).as_deref(), Some("minecraft:dispenser"));
        assert_eq!(shown(4).as_deref(), Some("minecraft:gray_stained_glass_pane"));
    }

    #[test]
    fn rainbow_items_show_one_in_the_guide_and_eight_in_the_altar_recipe() {
        mod_api::__private::initialize().unwrap();
        let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
        let mut host = FakeHost::new();
        let entry = lookup("RAINBOW_WOOL").unwrap();
        host.registries
            .resolve_items([Identifier::parse(entry.item.kind.material()).unwrap()])
            .unwrap();
        let search = render(
            &host,
            None,
            &Page::Search("彩虹羊毛".into()),
            1,
            false,
            available(),
            false,
        );
        let (_, listed) = search.actions.iter().zip(&search.slots).find(|(action, _)| {
            matches!(action, Some(Action::Push(Page::Recipe(result))) if result.id == entry.id)
        }).unwrap();
        let listed = listed.as_ref().unwrap();
        assert_eq!(listed.count(), 1);
        let recipe = render(&host, None, &Page::Recipe(entry), 1, false, available(), false);
        let output = recipe.slots[25].as_ref().unwrap();
        assert_eq!(output.count(), 8);
        let lore = colored_lore("&d在彩虹的各个颜色之间循环切换！");
        for stack in [listed, output] {
            let Tag::List(lines) = stack.components().unwrap().as_nbt()
                .get(&"minecraft:lore".into()).unwrap()
            else {
                panic!("missing lore");
            };
            assert!(lines.contains(&lore));
        }
    }

    #[test]
    fn chinese_icons_keep_the_original_skin_and_cheat_items_keep_the_original_stack() {
        mod_api::__private::initialize().unwrap();
        let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                ["minecraft:player_head", "minecraft:stone"]
                    .map(|material| Identifier::parse(material).unwrap()),
            )
            .unwrap();
        let level = host.level.level();
        host.players.join(1, 100, level);
        let player = host.player(1).unwrap();
        let entry = lookup("PORTABLE_CRAFTER").unwrap();
        let normal = entry.item.kind.stack(&host, 1).unwrap();
        let displayed = entry_icon(&host, entry, 1).unwrap();
        let parts = |stack: ItemStack| stack.components().unwrap().as_nbt().clone();
        let original = parts(normal.clone());
        let displayed = parts(displayed);
        let profile = &"minecraft:profile".into();
        assert!(original.get(profile).is_some());
        assert_eq!(displayed.get(profile), original.get(profile));
        assert_eq!(
            original.get(&"minecraft:custom_name".into()),
            Some(&colored_lore("&6便携工作台")),
        );
        assert_eq!(
            displayed.get(&"minecraft:custom_name".into()),
            Some(&colored_lore(locale::item_name(entry.id).unwrap())),
        );
        assert!(search_matches("便携工作台", 1, false, available())
            .any(|(_, result)| result.id == entry.id));
        assert!(matches!(entry_action(entry, true), Action::Grant(item) if item.id == entry.id));

        let stone = Kind::Vanilla("minecraft:stone").stack(&host, 64).unwrap();
        host.players.get_mut(1).unwrap().slots.insert(0, stone);
        assert!(give_cheat_item(&mut host, player, &normal, true));
        let inventory = host.players.get(1).unwrap();
        assert!(inventory.given.is_empty());
        assert!(ItemResource::of(&normal).matches(&inventory.slots[&1]));
        assert_eq!(inventory.slots[&1].count(), host.max_stack_size(normal.item()).unwrap());
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
        let menu = render(&host, None, &Page::Recipe(table), 1, false, available(), false);
        for (slot, material) in [
            (21, "smooth_stone_slab"),
            (22, "stonecutter"),
            (23, "smooth_stone_slab"),
            (31, "iron_block"),
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
            let shown = render(&host, None, &Page::Recipe(miner), 1, false, available(), false);
            assert_eq!(
                shown.slots[22].as_ref().unwrap().item().id(&host),
                "minecraft:chest"
            );
            assert_eq!(shown.slots[30].as_ref().unwrap().item().id(&host), base);
            assert_eq!(
                shown.slots[31].as_ref().unwrap().item().id(&host),
                "minecraft:blast_furnace"
            );
            assert_eq!(shown.slots[32].as_ref().unwrap().item().id(&host), base);
            for slot in [21, 23] {
                let parts = shown.slots[slot].as_ref().unwrap().components().unwrap();
                assert_eq!(
                    parts.as_nbt().get(&"minecraft:custom_name".into()),
                    Some(&colored_lore("活塞（朝上）"))
                );
            }
        }
        for row in crate::food_machines::PRODUCE {
            let food = lookup(row.food.id()).unwrap();
            assert_eq!(food.item.count, 2);
            assert_eq!(food.ingredients[0].unwrap().kind, crate::food_machines::CAN);
            assert_eq!(food.ingredients[1].unwrap().kind, row.crop);
            assert_eq!(food.recipe_type.name, "食物加工机");
            assert_eq!(food.research.unwrap().id, "organic_food");
            let fertilizer = lookup(row.fertilizer.id()).unwrap();
            assert_eq!(fertilizer.item.count, 2);
            assert_eq!(fertilizer.ingredients[0].unwrap().kind, row.food);
            assert_eq!(fertilizer.recipe_type.name, "食物堆肥机");
            assert_eq!(fertilizer.research.unwrap().id, "organic_fertilizer");
        }
        let tin = lookup("TIN_DUST").unwrap();
        assert_eq!(tin.recipe_type.name, "洗矿机");
        assert_eq!(tin.ingredients[0].unwrap().kind, crate::items::SIFTED_ORE);
        let sifted = lookup("SIFTED_ORE").unwrap();
        assert_eq!(sifted.recipe_type.name, "淘金盘");
        assert_eq!(
            sifted.ingredients[0].unwrap().kind.material(),
            "minecraft:gravel"
        );
        assert_eq!(sifted.research.unwrap().id, "ore_washer");
        let circuit = lookup("BASIC_CIRCUIT_BOARD").unwrap();
        let shown = render(&host, None, &Page::Recipe(circuit), 1, false, available(), false);
        let head = shown.slots[22].as_ref().unwrap();
        assert_eq!(head.item().id(&host), "minecraft:player_head");
        assert!(
            head.components()
                .unwrap()
                .as_nbt()
                .contains_key(&"minecraft:profile".into())
        );
        assert_eq!(
            shown.slots[19].as_ref().unwrap().item().id(&host),
            "minecraft:iron_sword"
        );
    }

    #[test]
    fn paper_search_accepts_display_names_and_ids_and_limits_the_first_35() {
        let craft = lookup("ENHANCED_CRAFTING_TABLE").unwrap();
        for query in ["§aENHANCED_crafting_TABLE", "enhanced crafting table"] {
            assert!(
                search_matches(query, 1, false, available())
                    .any(|(_, entry)| std::ptr::eq(entry, craft))
            );
        }
        assert_eq!(search_matches("", 1, false, available()).count(), 35);
        assert_eq!(
            search_matches("does not exist", 1, false, available()).count(),
            0
        );
        assert!(search_matches("output_chest", 1, false, available()).count() > 0);
        assert_eq!(
            search_matches(
                "output_chest",
                1,
                false,
                Availability {
                    output_chest: false,
                    ..available()
                }
            )
            .count(),
            0
        );
        assert_eq!(search_title("§bCoal"), "搜索：Coal");
        assert_eq!(
            search_title("012345678901234567890"),
            "搜索：0123456789012345..."
        );
    }

    fn available() -> Availability<'static> {
        Availability {
            output_chest: true,
            tickers: Tickers::default(),
            dimension: None,
        }
    }

    #[test]
    fn disabled_tickers_disappear_from_search_without_hiding_cargo_nodes() {
        let unavailable = Availability {
            tickers: Tickers::disabled(),
            ..available()
        };
        assert!(
            search_matches("cargo_manager", 1, false, available())
                .next()
                .is_some()
        );
        assert!(
            search_matches("cargo_manager", 1, true, unavailable)
                .next()
                .is_none()
        );
        assert!(
            search_matches("cargo_node_input", 1, false, unavailable)
                .next()
                .is_some()
        );
        assert!(!unavailable.allows(lookup("ELECTRIC_FURNACE_3").unwrap()));
        assert!(unavailable.allows(lookup("SOLAR_GENERATOR_4").unwrap()));
        let hidden_topic = lookup("ELECTRIC_FURNACE").unwrap().research.unwrap();
        assert!(!research_available(hidden_topic.id, unavailable.tickers, None));
        assert!(research_available("cargo_nodes", unavailable.tickers, None));
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
            render(&host, None, &Page::Main, month, show_hidden, available(), false)
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
            assert!(
                !search_matches(query, 11, false, available()).any(|(group, _)| group.key == key)
            );
            assert!(
                search_matches(query, month, false, available()).any(|(group, _)| group.key == key)
            );
            assert!(
                search_matches(query, 11, true, available()).any(|(group, _)| group.key == key)
            );
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
                waiting_for_search: None,
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
