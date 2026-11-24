// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use mod_api::{
    Compound, Hand, Host, HostTransaction, ItemPlace, ItemResource, ItemStack, MenuClickedEvent,
    MenuClosedEvent, MenuReplace, PlayerRef, ServerHost, SoundSource, Tag, Text,
    Transaction,
    text::{TextClick, TextStyle},
};

use crate::{
    availability,
    hologram_projector::{colored_label, colored_lore},
    items::{HeadTexture, Kind},
    network_runtime::MachineRecipes,
    recipes::Research,
    research,
    ticker_policy::Tickers,
};

mod catalog;
mod learning;
mod locale;
mod processing;
mod vanilla;
use catalog::{Entry, GROUPS, Group, MultiblockItem, group_entries, lookup, named_group_entries};
use learning::Preference;
pub(crate) use learning::launch_random_fireworks;
pub(crate) use locale::{Language, localize_item_components};
pub(crate) use locale::research_name;

const DEFAULT_CHAT_PREFIX: &str = "&a&lSlimefun 5&7> ";

const DASHBOARD_STYLE: &str = "pigeonmoe:life_skills_menu";
const BOOK_LORE: &[&str] = &[
    "",
    "&e右键 &8⇨ &7浏览物品",
    "&e潜行并右键 &8⇨ &7打开设置与关于",
];
const CHEAT_BOOK_LORE: &[&str] = &[
    "&4&l仅管理员可使用",
    "&e右键 &8⇨ &7浏览物品",
    "&e潜行并右键 &8⇨ &7打开设置与关于",
];
const BOOK_NAME: &str = "&a远古工艺之书";
const CHEAT_BOOK_NAME: &str = "&c远古工艺之书 &4（作弊模式）";
const GUIDE_KEY: &str = "slimefun:slimefun_guide_mode";
const SURVIVAL: &str = "SURVIVAL_MODE";
const CHEAT: &str = "CHEAT_MODE";
const DISCOVERY_SLOTS: usize = 36;
const INGREDIENT_SLOTS: [usize; 9] = [3, 4, 5, 12, 13, 14, 21, 22, 23];
const RECIPE_METHOD_SLOT: usize = 10;
const RECIPE_OUTPUT_SLOT: usize = 16;
const PROCESSING_PAGE_SIZE: usize = 18;

fn book(host: &dyn Host) -> Option<ItemStack> {
    let mut item = icon(
        host,
        "minecraft:enchanted_book",
        BOOK_NAME,
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
        Tag::String(mode) if mode.to_string_lossy() == CHEAT => Some(CHEAT),
        _ => None,
    }
}

#[cfg(test)]
fn is_book(host: &dyn Host, item: &ItemStack) -> bool {
    book_mode(host, item) == Some(SURVIVAL)
}

fn held_book_slot(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    hand: Hand,
    stack: &ItemStack,
) -> Option<u32> {
    let held = ItemPlace::PlayerHand { player: player.uuid(), hand };
    if held.stack(host).as_ref() != Some(stack) {
        return None;
    }
    match hand {
        Hand::Main => player
            .selected_slot(host)
            .and_then(|slot| u32::try_from(slot).ok())
            .filter(|slot| *slot < 9),
        Hand::Off => Some(40),
    }
}

fn book_in_slot(host: &dyn Host, player: PlayerRef, slot: u32) -> Option<ItemStack> {
    player
        .inventory_slots(host)?
        .into_iter()
        .find_map(|(index, stack)| (index == slot && is_guide_book(host, &stack)).then_some(stack))
}

fn rename_old_book(host: &mut dyn ServerHost, player: PlayerRef, slot: u32, stack: &ItemStack) {
    let name = stack
        .components()
        .and_then(|parts| parts.as_nbt().get(&"minecraft:custom_name".into()));
    if name != Some(&colored_lore("&aSlimefun Guide &7(Chest GUI)")) {
        return;
    }
    let mut updated = stack.clone();
    if updated.apply_components(&Compound::from([
        ("minecraft:custom_name".into(), colored_lore(BOOK_NAME)),
        ("minecraft:lore".into(), Tag::List(BOOK_LORE.iter().map(|line| colored_lore(line)).collect())),
    ])).is_ok() {
        let _ = player.set_inventory_slot(host, slot, Some(updated));
    }
}

fn switched_book(host: &dyn Host, mut stack: ItemStack) -> Option<ItemStack> {
    let (mode, name, lore) = if book_mode(host, &stack)? == SURVIVAL {
        (CHEAT, CHEAT_BOOK_NAME, CHEAT_BOOK_LORE)
    } else {
        (SURVIVAL, BOOK_NAME, BOOK_LORE)
    };
    let custom_data_key = "minecraft:custom_data".into();
    let mut data = stack.components()?.as_nbt().get(&custom_data_key)?.clone();
    let Tag::Compound(values) = &mut data else {
        return None;
    };
    let Some(Tag::Compound(bukkit)) = values.get_mut(&"PublicBukkitValues".into()) else {
        return None;
    };
    bukkit.insert(GUIDE_KEY.into(), Tag::String(mode.into()));
    stack
        .apply_components(&Compound::from([
            (custom_data_key, data),
            ("minecraft:custom_name".into(), colored_lore(name)),
            ("minecraft:lore".into(), Tag::List(lore.iter().map(|line| colored_lore(line)).collect())),
        ]))
        .ok()?;
    Some(stack)
}

pub(crate) fn is_guide_book(host: &dyn Host, item: &ItemStack) -> bool {
    book_mode(host, item).is_some()
}

pub(crate) fn has_stock_item_id(id: &str) -> bool {
    catalog::has_stock_item_id(id)
}

pub(crate) fn localized_plain_item_name(id: &str) -> Option<String> {
    locale::item_name(id).map(crate::hologram_projector::plain_label)
}

pub(crate) fn command_item(id: &str) -> Option<Result<Kind, &'static str>> {
    match id {
        "RESTORED_BACKPACK" => return Some(Ok(crate::backpacks::RESTORED_BACKPACK)),
        "FILLED_FLASK_OF_KNOWLEDGE" => {
            return Some(Ok(crate::factory::experience_collector::FILLED_FLASK_OF_KNOWLEDGE));
        }
        _ => {}
    }
    if let Some(kind) = crate::rainbow::item(id) {
        return Some(Ok(kind));
    }
    if !catalog::has_stock_item_id(id) {
        return None;
    }
    let entry = lookup(id)?;
    Some(if entry.multiblock.is_some() {
        Err("多方块机器需现场搭建，无法发放。")
    } else {
        Ok(entry.item.kind)
    })
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

pub(crate) fn research_has_enabled_items(id: &str, tickers: Tickers) -> bool {
    catalog::items_for_research(id).any(|item| availability::visible(tickers, None, item))
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
        insert_guide(host, tx, player, &resource)
            .then_some(())
            .ok_or(())
    });
}

fn insert_guide(
    host: &mut dyn ServerHost,
    tx: &Transaction,
    player: PlayerRef,
    resource: &ItemResource,
) -> bool {
    (0..36).any(|slot| {
        ItemPlace::PlayerSlot {
            player: player.uuid(),
            slot,
        }
        .insert(host, tx, resource, 1)
            == 1
    })
}

fn give_cheat_item(host: &mut dyn ServerHost, player: PlayerRef, stack: &ItemStack, full_stack: bool) -> bool {
    let resource = ItemResource::of(stack);
    let limit = crate::items::stack_limit(host, stack);
    let count = if full_stack { limit } else { stack.count() };
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
    crate::admin_commands::operator(host, player)
}

#[derive(Clone)]
enum Page {
    Main,
    Cheat,
    Group(&'static Group, usize),
    Recipe(&'static Entry),
    ProcessingRecipes(&'static Entry, usize),
    VanillaRecipe(ItemResource, usize),
    Info,
    Languages,
    Search(Arc<str>),
}

#[derive(Clone, Copy)]
enum Presentation {
    Book,
    HeldBook(u32),
    Dashboard,
}

#[derive(Clone)]
enum Action {
    Push(Page),
    Replace(Page),
    Back,
    Research(&'static Research),
    TogglePreference(Preference),
    SelectLanguage(Option<Language>),
    EnterCheat,
    SwitchBookMode(u32),
    Grant(&'static Entry),
    SearchPrompt,
    FollowVanilla,
    Link(&'static str),
}

struct View {
    window: i32,
    path: Vec<Page>,
    presentation: Presentation,
    actions: [Option<Action>; 54],
    vanilla_rotation: Option<vanilla::Rotation>,
}

#[derive(Default)]
struct Reader {
    history: Vec<Page>,
    view: Option<View>,
    waiting_for_search: Option<(Vec<Page>, Presentation)>,
}

pub(crate) struct Guide {
    readers: Mutex<BTreeMap<u128, Reader>>,
    receive_on_first_join: AtomicBool,
    show_hidden_groups_in_search: AtomicBool,
    show_vanilla_recipes: AtomicBool,
    languages: Mutex<locale::Settings>,
    chat_prefix: Mutex<String>,
    learning: learning::Learning,
    machine_settings: Mutex<crate::machine::Settings>,
    machine_recipes: Mutex<Arc<MachineRecipes>>,
}

impl Default for Guide {
    fn default() -> Self {
        Self {
            readers: Mutex::new(BTreeMap::new()),
            receive_on_first_join: AtomicBool::new(true),
            show_hidden_groups_in_search: AtomicBool::new(false),
            show_vanilla_recipes: AtomicBool::new(true),
            languages: Mutex::default(),
            chat_prefix: Mutex::new(DEFAULT_CHAT_PREFIX.into()),
            learning: learning::Learning::default(),
            machine_settings: Mutex::default(),
            machine_recipes: Mutex::default(),
        }
    }
}

impl Guide {
    pub(crate) fn set_registered_machine_recipes(&self, recipes: Arc<MachineRecipes>) {
        *self.machine_recipes.lock().unwrap() = recipes;
    }

    pub(crate) fn configure_machine_display(&self, settings: crate::machine::Settings) {
        *self.machine_settings.lock().unwrap() = settings;
    }

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
        let show_vanilla = section
            .and_then(|section| section.get("show-vanilla-recipes"))
            .and_then(toml::Value::as_bool)
            .unwrap_or(true);
        self.show_vanilla_recipes.store(show_vanilla, Ordering::Relaxed);
    }

    pub(crate) fn configure_languages(&self, options: Option<&toml::Value>) {
        *self.languages.lock().unwrap() = locale::Settings::from_config(options);
        *self.chat_prefix.lock().unwrap() = options
            .and_then(|options| options.get("chat-prefix"))
            .and_then(toml::Value::as_str)
            .unwrap_or(DEFAULT_CHAT_PREFIX)
            .into();
    }

    pub(crate) fn player_language(&self, host: &dyn Host, player: PlayerRef) -> Language {
        let settings = *self.languages.lock().unwrap();
        settings.player(host, player)
    }

    pub(crate) fn command_language(
        &self,
        host: &dyn Host,
        source: &mod_api::CommandSource,
    ) -> Language {
        match *source {
            mod_api::CommandSource::Player { player, .. } => self.player_language(host, player),
            _ => self.languages.lock().unwrap().default,
        }
    }

    pub(crate) fn chat_prefix(&self) -> String {
        self.chat_prefix.lock().unwrap().clone()
    }

    pub(crate) fn prefixed_message(&self, message: &str) -> Text {
        colored_label(&format!("{}{message}", self.chat_prefix()))
    }

    pub(crate) fn refuses_multiblock_item_use(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        stack: &ItemStack,
    ) -> bool {
        let Some(id) = crate::items::item_id(stack) else {
            return false;
        };
        let Some(entry) = lookup(&id).filter(|entry| entry.id == id && entry.multiblock.is_some())
        else {
            return false;
        };
        let Some(topic) = entry.research else {
            return false;
        };
        let saved = player.persistent_data(host);
        if saved.as_ref().is_some_and(|saved| research::unlocked(saved, topic)) {
            return false;
        }
        if saved.is_some() {
            let language = self.player_language(host, player);
            let name = entry_name(entry, language);
            let message = language.command_message("not-researched").replace("%item%", &name);
            let _ = player.tell(host, colored_label(&format!("{}{message}", self.chat_prefix())));
        }
        true
    }

    pub(crate) fn first_join(&self, host: &mut dyn ServerHost, uuid: u128) {
        if let Some(player) = host.player(uuid)
            && self.receives_first_guide(host, player)
        {
            give(host, player);
        }
    }

    fn receives_first_guide(&self, host: &dyn Host, player: PlayerRef) -> bool {
        self.receive_on_first_join.load(Ordering::Relaxed)
            && player
                .dimension(host)
                .is_none_or(|dimension| availability::world_enabled(&dimension.to_string()))
    }

    pub(crate) fn initialize_transferred_guide(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
    ) -> Result<(), &'static str> {
        let Some(mut data) = player.persistent_data(host) else {
            return Ok(());
        };
        if !research::mark_transferred_guide_initialized(&mut data) {
            return Ok(());
        }
        let resource = if self.receives_first_guide(host, player) {
            Some(ItemResource::of(
                &book(host).ok_or("无法创建跨服档案的远古工艺指南。")?,
            ))
        } else {
            None
        };
        host.transaction(|host, tx| {
            if let Some(resource) = &resource {
                // Paper marks the profile initialized even if the inventory is full.
                insert_guide(host, tx, player, resource);
            }
            player
                .set_persistent_data(host, data)
                .map_err(|_| "无法保存跨服指南的初始化状态。")
        })
    }

    pub(crate) fn announce_shared_research(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        name: &str,
    ) {
        self.learning.announce_completion(host, player, name, Language::CHINESE);
    }

    pub(crate) fn logged_in(&self, host: &dyn Host, player: PlayerRef) {
        self.learning.resume(host, player);
    }

    pub(crate) fn tick(&self, host: &mut dyn ServerHost, player: PlayerRef) {
        if let Some((path, presentation)) = self.learning.tick(host, player, |host, player| self.player_language(host, player)) {
            self.show(host, player, path, presentation);
        }
        let update = self.readers.lock().unwrap().get_mut(&player.uuid())
            .and_then(|reader| reader.view.as_mut())
            .and_then(|view| Some((view.window, view.vanilla_rotation.as_mut()?.advance()?)));
        if let Some((window, items)) = update {
            let _ = host.menu_set_items(player.uuid(), window, items);
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
        self.show(host, player, path, Presentation::Book);
    }

    pub(crate) fn open_cheat(&self, host: &mut dyn ServerHost, player: PlayerRef) {
        self.show(host, player, vec![Page::Cheat], Presentation::Book);
    }

    pub(crate) fn open_dashboard(&self, host: &mut dyn ServerHost, player: PlayerRef) {
        let path = self.last_path(player.uuid());
        self.show(host, player, path, Presentation::Dashboard);
    }

    pub(crate) fn search(&self, host: &mut dyn ServerHost, player: PlayerRef, query: &str) {
        let mut history = self.last_path(player.uuid());
        history.push(Page::Search(Arc::from(query)));
        self.show(host, player, history, Presentation::Book);
    }

    pub(crate) fn chat(&self, host: &mut dyn ServerHost, player: u128, query: &str) -> bool {
        let path = self
            .readers
            .lock()
            .unwrap()
            .get_mut(&player)
            .and_then(|reader| reader.waiting_for_search.take());
        if let Some((mut path, presentation)) = path {
            if let Some(player) = host.player(player) {
                path.push(Page::Search(Arc::from(query)));
                self.show(host, player, path, presentation);
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
        let Some(mode) = book_mode(host, stack) else {
            return false;
        };
        if !availability::world_enabled(host.dimension()) {
            availability::Denied::World.tell(host, player);
            return true;
        }
        let slot = held_book_slot(host, player, hand, stack);
        if let Some(slot) = slot
            && mode == SURVIVAL
        {
            rename_old_book(host, player, slot, stack);
        }
        let mut path = self.last_path(player.uuid());
        if player.entity().is_shift_key_down(host) == Some(true) {
            path.push(Page::Info);
        } else if mode == CHEAT {
            self.open_cheat(host, player);
            return true;
        }
        self.show(
            host,
            player,
            path,
            slot.map_or(Presentation::Book, Presentation::HeldBook),
        );
        true
    }

    fn show(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        mut path: Vec<Page>,
        presentation: Presentation,
    ) {
        let cheat = matches!(path.first(), Some(Page::Cheat));
        if !cheat && !availability::world_enabled(host.dimension()) {
            availability::Denied::World.tell(host, player);
            return;
        }
        let language_settings = *self.languages.lock().unwrap();
        let language = language_settings.player(host, player);
        if cheat && !can_cheat(host, player) {
            let notice = self.prefixed_message(language.command_message("no-permission"));
            let _ = player.tell(host, notice);
            return;
        }
        if !language_settings.enabled && matches!(path.last(), Some(Page::Languages)) {
            path.pop();
        }
        let month = host.server_local_minute()[1];
        let recipes = self.machine_recipes.lock().unwrap().clone();
        let availability = Availability {
            machine_settings: *self.machine_settings.lock().unwrap(),
            machine_recipes: Some(&recipes),
            language,
            show_vanilla_recipes: self.show_vanilla_recipes.load(Ordering::Relaxed),
            ..Availability::for_dimension(host.dimension())
        };
        if matches!(path.last(), Some(Page::Group(group, _)) if !cheat && !group.in_season(month))
            || matches!(path.last(), Some(Page::Recipe(entry) | Page::ProcessingRecipes(entry, _))
                if !availability.allows_search(entry))
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
            show_hidden || cheat,
            availability,
            cheat,
        );
        if matches!(page, Page::VanillaRecipe(_, _)) && !menu.vanilla_recipe_exists {
            path.pop();
            self.show(host, player, path, presentation);
            return;
        }
        if matches!(page, Page::Info) {
            let mut slot = 19;
            if can_cheat(host, player) {
                let held_book = match presentation {
                    Presentation::HeldBook(slot) => book_in_slot(host, player, slot)
                        .map(|stack| (slot, book_mode(host, &stack) == Some(CHEAT))),
                    Presentation::Book | Presentation::Dashboard => None,
                };
                let is_cheat = held_book.is_some_and(|(_, cheat)| cheat);
                let survival_mode = language.message("modes.SURVIVAL_MODE");
                let cheat_mode = language.message("modes.CHEAT_MODE");
                let selected = if is_cheat { cheat_mode } else { survival_mode };
                let title = format!("&7{} &e{selected}", language.message("modes.selected"));
                let survival_mode = format!("{}{survival_mode}", if is_cheat { "&7" } else { "&a" });
                let cheat_mode = format!("{}{cheat_mode}", if is_cheat { "&a" } else { "&7" });
                let click = format!("&7⇨ &e{}", language.message("modes.change"));
                let material = if is_cheat {
                    "minecraft:command_block"
                } else {
                    "minecraft:chest"
                };
                let action = held_book.map_or(Action::EnterCheat, |(slot, _)| {
                    Action::SwitchBookMode(slot)
                });
                menu.set(
                    slot,
                    icon(host, material, &title, &["", &survival_mode, &cheat_mode, "", &click]),
                    Some(action),
                );
                slot += 1;
            }
            for option in [Preference::Fireworks, Preference::Animation] {
                let Some(enabled) = self.learning.option(host, player, option) else { continue };
                let (material, key) = match option {
                    Preference::Fireworks => ("minecraft:firework_rocket", "fireworks"),
                    Preference::Animation if enabled => ("minecraft:map", "learning-animation"),
                    Preference::Animation => ("minecraft:paper", "learning-animation"),
                };
                let state = if enabled { "enabled" } else { "disabled" };
                let prefix = format!("options.{key}.{state}");
                let text = language.lines(&format!("{prefix}.text"));
                let title = text.first().copied().unwrap_or("");
                let mut lore = text.get(1..).unwrap_or_default().to_vec();
                lore.push("");
                lore.push(language.message(&format!("{prefix}.click")));
                menu.set(slot, icon(host, material, title, &lore), Some(Action::TogglePreference(option)));
                slot += 1;
            }
            if language_settings.enabled {
                selected_language(host, &mut menu, slot, language, language_settings.default);
            }
        }
        if matches!(page, Page::Languages) {
            language_picker(host, &mut menu, language, language_settings.default);
        }
        if path.len() > 1 && !matches!(page, Page::Languages) {
            let back = usize::from(!matches!(
                page,
                Page::Recipe(_) | Page::ProcessingRecipes(_, _) | Page::VanillaRecipe(_, _) | Page::Info
            ));
            let title = format!("&7⇦ {}", language.message("back.title"));
            let guide = format!("&7{}", language.message("back.guide"));
            let lore = if !cheat && language == Language::CHINESE {
                &["", "&f左键：&7返回上一页", "&f潜行并左键：&7返回主页"][..]
            } else {
                &["", guide.as_str()][..]
            };
            menu.set(
                back,
                icon(host, "minecraft:enchanted_book", &title, lore),
                Some(Action::Back),
            );
        }
        let mut title = match page {
            Page::Languages => colored_label(language.message("title.languages")),
            Page::Info => colored_label(language.message("title.settings")),
            Page::Search(query) => search_title(query, language),
            _ => colored_label(language.message("title.main")),
        };
        if matches!(presentation, Presentation::Dashboard) {
            title.style.insertion = Some(DASHBOARD_STYLE.into());
        }
        let vanilla_rotation = vanilla::Rotation::new(menu.vanilla_choices, &menu.slots);
        if let Ok(Some(window)) =
            host.open_menu(player.uuid(), 6, title, menu.slots, MenuReplace::CloseFirst)
        {
            let settings = matches!(page, Page::Info | Page::Languages);
            let len = path.iter().rposition(|page| !matches!(page, Page::Info | Page::Languages))
                .map_or(0, |index| index + 1);
            let mut readers = self.readers.lock().unwrap();
            let reader = readers.entry(player.uuid()).or_default();
            if !cheat {
                reader.history = path[..len].to_vec();
            }
            reader.view = Some(View {
                window,
                path,
                presentation,
                actions: menu.actions,
                vanilla_rotation,
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
        let Some((mut path, presentation, action)) = self
            .readers
            .lock()
            .unwrap()
            .get(&event.uuid)
            .and_then(|reader| reader.view.as_ref())
            .filter(|view| view.window == event.window)
            .and_then(|view| Some((view.path.clone(), view.presentation, view.actions.get(slot)?.clone()?)))
        else {
            return;
        };
        let Some(player) = host.player(event.uuid) else {
            return;
        };
        if matches!(path.first(), Some(Page::Cheat)) && !can_cheat(host, player) {
            host.close_menu(event.uuid);
            let language = self.player_language(host, player);
            let notice = self.prefixed_message(language.command_message("no-permission"));
            let _ = player.tell(host, notice);
            return;
        }
        match action {
            Action::Push(page) => path.push(page),
            Action::FollowVanilla => {
                if !self.show_vanilla_recipes.load(Ordering::Relaxed) {
                    return;
                }
                let Some(stack) = host.menu(event.uuid, event.window)
                    .and_then(|menu| menu.slot(host, slot))
                    .flatten()
                else {
                    return;
                };
                let item = ItemResource::of(&stack);
                if vanilla::recipes_for(host, &item).is_empty() {
                    return;
                }
                path.push(Page::VanillaRecipe(item, 0));
            }
            Action::Replace(page) => {
                path.pop();
                path.push(page);
            }
            Action::Back if matches!(path.first(), Some(Page::Cheat))
                || (event.mode == 1 && !matches!(path.last(), Some(Page::Info | Page::Languages))) => {
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
                    self.show(host, player, path, presentation);
                    return;
                }
                if !learned(host, player, topic)
                    && {
                        let language = self.player_language(host, player);
                        !self.learning.start(host, player, topic, &path, presentation, language)
                    }
                {
                    return;
                }
            }
            Action::TogglePreference(option) => {
                self.learning.toggle(host, player, option);
            }
            Action::SelectLanguage(selected) => {
                let settings = *self.languages.lock().unwrap();
                if !settings.enabled || !locale::select(host, player, selected) {
                    return;
                }
                let language = settings.player(host, player);
                let name = selected.map_or_else(|| language.default_name(), |selected| language.name(selected));
                let notice = language.message("languages.updated").replace("%lang%", name);
                let _ = player.tell(host, colored_label(&notice));
                path.pop();
            }
            Action::EnterCheat => path = vec![Page::Cheat],
            Action::SwitchBookMode(slot) => {
                if can_cheat(host, player)
                    && let Some(updated) =
                        book_in_slot(host, player, slot).and_then(|stack| switched_book(host, stack))
                {
                    let _ = player.set_inventory_slot(host, slot, Some(updated));
                }
            }
            Action::Grant(entry) => {
                // Paper searches the global item registry and gives the template as-is.
                // Category pages honor world settings and support shift-clicking a full stack.
                let from_search = matches!(path.last(), Some(Page::Search(_)));
                let availability = Availability::for_dimension(host.dimension());
                let available = if from_search {
                    availability.allows_search(entry)
                } else {
                    availability.allows(entry)
                };
                if !available {
                    return;
                }
                if entry.multiblock.is_some() && !from_search {
                    let language = self.player_language(host, player);
                    let notice = self.prefixed_message(language.message("cheat.no-multiblocks"));
                    let _ = player.tell(host, notice);
                    return;
                }
                if let Some(stack) = inventory_template(host, entry) {
                    give_cheat_item(host, player, &stack, !from_search && event.mode == 1);
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
                    .waiting_for_search = Some((path, presentation));
                let prompt = self.player_language(host, player).message("search.message");
                let _ = player.tell(host, colored_label(prompt));
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
        self.show(host, player, path, presentation);
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
    vanilla_choices: Vec<vanilla::Choice>,
    vanilla_recipe_exists: bool,
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
    machine_settings: crate::machine::Settings,
    machine_recipes: Option<&'a MachineRecipes>,
    show_vanilla_recipes: bool,
    language: Language,
}

impl<'a> Availability<'a> {
    fn for_dimension(dimension: &'a str) -> Self {
        Self {
            output_chest: crate::output_chest::enabled(dimension),
            tickers: Tickers::current(),
            dimension: Some(dimension),
            machine_settings: crate::machine::Settings::default(),
            machine_recipes: None,
            show_vanilla_recipes: true,
            language: Language::CHINESE,
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
    let language = availability.language;
    let visible_entry = |entry: &&Entry| availability.allows(entry);
    let mut menu = Menu {
        slots: vec![None; 54],
        actions: std::array::from_fn(|_| None),
        vanilla_choices: Vec::new(),
        vanilla_recipe_exists: false,
    };
    let pane = icon(host, "minecraft:gray_stained_glass_pane", " ", &[]);
    if matches!(screen, Page::Languages) {
        menu.slots[..9].fill(pane.clone());
    } else if matches!(screen, Page::Info) {
        for slot in [
            1, 3, 5, 7, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 26, 27, 35, 36, 37, 38, 39, 40, 41,
            42, 43, 44, 45, 46, 48, 50, 52, 53,
        ] {
            menu.slots[slot] = pane.clone();
        }
    } else if !matches!(
        screen,
        Page::Recipe(_) | Page::ProcessingRecipes(_, _) | Page::VanillaRecipe(_, _)
    ) {
        for slot in (0..9).chain(45..54) {
            menu.slots[slot] = pane.clone();
        }
        if !cheat {
            let title = format!("&e{}", language.message("title.settings"));
            menu.set(
                1,
                icon(host, "minecraft:comparator", &title, &[]),
                Some(Action::Push(Page::Info)),
            );
        }
        let search_tip = format!("&7⇨ &b{}", language.message("search.tooltip"));
        menu.set(
            7,
            icon(host, "minecraft:name_tag", language.message("search.name"), &["", &search_tip]),
            Some(Action::SearchPrompt),
        );
    }
    match screen {
        Page::Main | Page::Cheat => {
            for (slot, group) in GROUPS
                .iter()
                .filter(|group| cheat || group.in_season(month))
                .filter(|group| cheat || group_entries(group).any(|entry| visible_entry(&entry)))
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
                let group_name = language.category(group.key, group.name);
                let name = format!("&{}{group_name}", color_code(group.color));
                let shown = if let Some(parent) = locked_parent {
                    let title = format!("&4{} &7- &f{group_name}", language.message("locked"));
                    let parent = format!("&7{}", language.category(parent.key, parent.name));
                    let mut lore = vec![""];
                    let requirements: Vec<_> = language.lines("locked-itemgroup").into_iter()
                        .map(|line| format!("&f{line}")).collect();
                    lore.extend(requirements.iter().map(String::as_str));
                    lore.extend(["", &parent]);
                    icon(host, "minecraft:barrier", &title, &lore)
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
            let entries: Vec<_> = group_entries(group)
                .filter(|entry| cheat || visible_entry(entry))
                .collect();
            let pages = entries.len().div_ceil(DISCOVERY_SLOTS).max(1);
            let page = (*page).min(pages);
            for (slot, entry) in entries
                .into_iter()
                .skip((page - 1) * DISCOVERY_SLOTS)
                .take(DISCOVERY_SLOTS)
                .filter(visible_entry)
                .enumerate()
            {
                if let Some(topic) = entry.research.filter(|_| !cheat && !unlocked(saved, entry)) {
                    let cost = language.message("research-cost").replace("%cost%", &topic.cost.to_string());
                    let locked = format!("&4&l{}", language.message("locked"));
                    menu.set(
                        9 + slot,
                        icon(
                            host,
                            "minecraft:barrier",
                            &entry_name(entry, language),
                            &[&locked, "", language.message("unlock"), "", &cost],
                        ),
                        Some(Action::Research(topic)),
                    );
                } else {
                    menu.set(
                        9 + slot,
                        entry_icon(host, entry, 1, language),
                        Some(entry_action(entry, cheat)),
                    );
                }
            }
            pagination(host, &mut menu, group, page, pages, language);
        }
        Page::Recipe(entry) | Page::ProcessingRecipes(entry, _) => {
            let processing = processing::entries(
                host, entry.id, availability.machine_settings, availability.machine_recipes,
            );
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
                            &entry_name(locked, language),
                            &[&format!("&4&l{}", language.message("locked"))],
                        ),
                        None,
                    );
                } else {
                    menu.set(
                        slot,
                        input.and_then(|item| ingredient_icon(host, entry, index, item, language)),
                        linked.map(|entry| Action::Push(Page::Recipe(entry))).or_else(|| {
                            input.filter(|stack| !matches!(stack.kind, Kind::Slimefun(_)))
                                .filter(|_| availability.show_vanilla_recipes)
                                .map(|_| Action::FollowVanilla)
                        }),
                    );
                }
            }
            menu.slots[RECIPE_METHOD_SLOT] = method_icon(host, entry, language);
            menu.slots[RECIPE_OUTPUT_SLOT] = entry_icon(host, entry, entry.item.count, language);
            if !processing.is_empty() {
                let page = match screen {
                    Page::ProcessingRecipes(_, page) => *page,
                    _ => 0,
                };
                show_processing_recipes(host, &mut menu, entry, processing, page, availability);
            }
        }
        Page::VanillaRecipe(item, index) => {
            if availability.show_vanilla_recipes {
                show_vanilla_recipe(host, &mut menu, item, *index, language);
            }
        }
        Page::Info => {
            let (credits, credit_lore, label) = if language == Language::CHINESE {
                ("&cPigeonMoe 移植团队", &["", "&7远古工艺由 PigeonMoe 移植至 Horizon。", "&7感谢原版开发者和社区贡献者。"][..], "远古工艺")
            } else {
                ("&cPigeonMoe port", &["", "&7PigeonMoe ported Slimefun to Horizon.", "&7Thanks to the original developers and community."][..], "Slimefun")
            };
            menu.slots[2] = icon(host, "minecraft:knowledge_book", credits, credit_lore);
            let game = format!("&fMinecraft: &a{}", host.game_version());
            let plugin = format!("&f{label}: &a{}", env!("CARGO_PKG_VERSION"));
            let versions = format!("&a{}", language.message("title.versions"));
            let notice = format!("&7&o{}", language.message("tooltips.versions-notice"));
            menu.slots[4] = icon(
                host,
                "minecraft:writable_book",
                &versions,
                &[&notice, "", &game, &plugin],
            );
            for (slot, material, title, url, tooltip) in [
                (
                    6,
                    "minecraft:comparator",
                    format!("&ePigeonMoe {}", language.message("title.source")),
                    "https://github.com/PigeonMoe/Slimefun/tree/Horizon",
                    language.message("credits.open"),
                ),
                (
                    8,
                    "minecraft:knowledge_book",
                    format!("&3{}", language.message("title.wiki")),
                    "https://github.com/Slimefun/Slimefun4/wiki",
                    language.message("tooltips.open-itemgroup"),
                ),
                (
                    47,
                    "minecraft:bookshelf",
                    format!("&3{}", language.message("title.addons")),
                    "https://github.com/Slimefun/Slimefun4/wiki/Addons",
                    language.message("tooltips.open-itemgroup"),
                ),
            ] {
                menu.set(
                    slot,
                    icon(host, material, &title, &["", tooltip]),
                    Some(Action::Link(url)),
                );
            }
        }
        Page::Languages => {}
        Page::Search(query) => {
            for (slot, (group, entry)) in
                search_matches(query, month, show_hidden, availability).enumerate()
            {
                let mut item = entry_icon(host, entry, 1, language);
                let group = colored_lore(&format!("&8⇨ &f{}", language.category(group.key, group.name)));
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

fn search_title(query: &str, language: Language) -> Text {
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
    let title = language
        .message("search.inventory")
        .replace("%item%", &format!("&f{plain}"));
    colored_label(&title)
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
                || strip_colors(
                    entry.multiblock.map_or_else(
                        || match entry.item.kind {
                            Kind::Slimefun(item) => item.name,
                            Kind::Vanilla(_) | Kind::Potion(_) => entry.id,
                        },
                        |item| item.name,
                    ),
                )
                    .to_lowercase()
                    .contains(&query)
                || strip_colors(&entry_name(entry, availability.language))
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

fn show_vanilla_recipe(host: &dyn Host, menu: &mut Menu, item: &ItemResource, index: usize, language: Language) {
    let recipes = vanilla::recipes_for(host, item);
    paint_vanilla_recipe(host, menu, item, &recipes, index, language);
}

fn paint_vanilla_recipe(
    host: &dyn Host,
    menu: &mut Menu,
    item: &ItemResource,
    recipes: &[vanilla::Match],
    index: usize,
    language: Language,
) {
    let Some(index) = recipes.len().checked_sub(1).map(|last| index.min(last)) else {
        return;
    };
    let selected = &recipes[index];
    menu.vanilla_recipe_exists = true;
    if let Some((material, key)) = vanilla::method(&selected.recipe) {
        for choice in vanilla::choices(host, &selected.recipe) {
            menu.set(choice.slot, choice.items.first().cloned(), Some(Action::FollowVanilla));
            if choice.items.len() > 1 {
                menu.vanilla_choices.push(choice);
            }
        }
        if let Some((name, lore)) = language.recipe_badge(key) {
            let lore: Vec<_> = lore.iter().map(String::as_str).collect();
            menu.slots[RECIPE_METHOD_SLOT] = icon(host, material, &format!("&b{name}"), &lore);
        }
        menu.slots[RECIPE_OUTPUT_SLOT] = Some(selected.result.clone());
    } else {
        let title = if language == Language::CHINESE {
            "&4无法在指南中显示此配方 :/"
        } else {
            "&4This recipe cannot be displayed in the Guide :/"
        };
        menu.slots[INGREDIENT_SLOTS[4]] = icon(host, "minecraft:barrier", title, &[]);
    }
    if recipes.len() > 1 {
        recipe_pages(host, menu, index, recipes.len(), " ", language, |page| {
            Page::VanillaRecipe(item.clone(), page)
        });
    }
}

fn recipe_pages(
    host: &dyn Host,
    menu: &mut Menu,
    page: usize,
    count: usize,
    label: &str,
    language: Language,
    screen: impl Fn(usize) -> Page,
) {
    let position = format!("&7({} / {count})", page + 1);
    for slot in 27..36 {
        menu.slots[slot] = icon(host, "minecraft:gray_stained_glass_pane", label, &[]);
    }
    for (slot, label, target) in [
        (28, format!("⇦ {}", language.message("pages.previous")), page.checked_sub(1)),
        (34, format!("{} ⇨", language.message("pages.next")), (page + 1 < count).then_some(page + 1)),
    ] {
        let (material, color) = if target.is_some() {
            ("minecraft:lime_stained_glass_pane", "&f")
        } else {
            ("minecraft:black_stained_glass_pane", "&8")
        };
        menu.set(
            slot,
            icon(host, material, &format!("{color}{label}"), &["", &position]),
            target.map(|next| Action::Replace(screen(next))),
        );
    }
}

fn show_processing_recipes(
    host: &dyn Host,
    menu: &mut Menu,
    entry: &'static Entry,
    recipes: Vec<Option<ItemStack>>,
    page: usize,
    availability: Availability<'_>,
) {
    let pages = recipes.len().div_ceil(PROCESSING_PAGE_SIZE);
    let page = page.min(pages - 1);
    let language = availability.language;
    let label = format!("&7⇩ {} ⇩", processing::label(entry.id, language));
    recipe_pages(host, menu, page, pages, &label, language, |next| {
        Page::ProcessingRecipes(entry, next)
    });
    let mut rows = recipes.into_iter().skip(page * PROCESSING_PAGE_SIZE);
    for column in 0..9 {
        for slot in [36 + column, 45 + column] {
            let mut item = rows.next().flatten();
            if let Some(stack) = &mut item {
                language.processing_item(host, stack);
            }
            let action = item.as_ref().and_then(|stack| {
                if let Some(id) = crate::items::item_id(stack) {
                    lookup(&id)
                        .filter(|entry| availability.allows_search(entry))
                        .map(|entry| Action::Push(Page::Recipe(entry)))
                } else {
                    availability.show_vanilla_recipes.then_some(Action::FollowVanilla)
                }
            });
            menu.set(slot, item, action);
        }
    }
}

fn pagination(host: &dyn Host, menu: &mut Menu, group: &'static Group, page: usize, pages: usize, language: Language) {
    let position = format!("&7({page} / {pages})");
    for (slot, label, next) in [
        (
            46,
            format!("&f⇦ {}", language.message("pages.previous")),
            page.checked_sub(1).filter(|page| *page > 0),
        ),
        (52, format!("&f{} ⇨", language.message("pages.next")), (page < pages).then_some(page + 1)),
    ] {
        if let Some(next) = next {
            menu.set(
                slot,
                icon(
                    host,
                    "minecraft:lime_stained_glass_pane",
                    &label,
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

fn method_icon(host: &dyn Host, entry: &Entry, language: Language) -> Option<ItemStack> {
    let method = entry.recipe_type;
    let mut stack = method.icon.guide_stack(host, 1, language)?;
    if let Some((name, lore)) = method.locale_key.and_then(|key| language.recipe_badge(key)) {
        stack
            .apply_components(&Compound::from([
                ("minecraft:custom_name".into(), colored_lore(&format!("&b{name}"))),
                (
                    "minecraft:lore".into(),
                    Tag::List(lore.iter().map(|line| colored_lore(line)).collect()),
                ),
            ]))
            .ok()?;
        return Some(stack);
    }
    if matches!(method.icon, Kind::Vanilla(_)) {
        let lore = if entry.multiblock.is_some() && language == Language::CHINESE {
            &["", "&a&o请在世界中搭建"][..]
        } else {
            method.lore
        };
        return icon(host, method.icon.material(), &format!("&b{}", method.name), lore);
    }
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
    language: Language,
) -> Option<ItemStack> {
    let mut stack = item.kind.guide_stack(host, item.count, language)?;
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

fn entry_name(entry: &Entry, language: Language) -> String {
    if let Some(name) = language.item_name(entry.id) {
        return name.to_owned();
    }
    match entry.multiblock {
        Some(item) if language == Language::CHINESE => item.localized_fallback.to_owned(),
        Some(item) => item.name.to_owned(),
        None => match entry.item.kind {
            Kind::Slimefun(item) => item.name.to_owned(),
            Kind::Vanilla(id) | Kind::Potion(id) => {
                id.trim_start_matches("minecraft:").replace('_', " ")
            }
        },
    }
}

fn multiblock_components(entry: &Entry, template: MultiblockItem) -> Compound {
    let mut components = Compound::from([
        ("minecraft:custom_name".into(), colored_lore(template.name)),
        (
            "minecraft:lore".into(),
            Tag::List(template.lore.iter().map(|line| colored_lore(line)).collect()),
        ),
    ]);
    if matches!(entry.id, "INDUSTRIAL_MINER" | "ADVANCED_INDUSTRIAL_MINER") {
        components.insert(
            "minecraft:tooltip_display".into(),
            Tag::compound([(
                "hidden_components",
                Tag::List(vec![Tag::String("minecraft:attribute_modifiers".into())]),
            )]),
        );
    }
    components
}

fn inventory_template(host: &dyn Host, entry: &Entry) -> Option<ItemStack> {
    let mut stack = entry.item.kind.stack(host, 1)?;
    if let Some(template) = entry.multiblock {
        let mut components = multiblock_components(entry, template);
        components.insert(
            "minecraft:custom_data".into(),
            crate::items::paper_item_data(entry.id),
        );
        stack.apply_components(&components).ok()?;
    }
    Some(stack)
}

fn entry_icon(host: &dyn Host, entry: &Entry, count: u32, language: Language) -> Option<ItemStack> {
    let mut stack = entry.item.kind.guide_stack(host, count, language)?;
    if let Some(template) = entry.multiblock {
        let mut components = multiblock_components(entry, template);
        language.localize_components(entry.id, &mut components);
        components.insert(
            "minecraft:custom_name".into(),
            colored_lore(&entry_name(entry, language)),
        );
        stack.apply_components(&components).ok()?;
    }
    Some(stack)
}

fn language_icon(host: &dyn Host, flag: HeadTexture, name: &str, lore: &[&str]) -> Option<ItemStack> {
    let mut head = icon(host, "minecraft:player_head", name, lore)?;
    head.apply_components(&Compound::from([("minecraft:profile".into(), flag.component())])).ok()?;
    Some(head)
}

fn default_language_name(language: Language, default: Language) -> String {
    format!("{} &8({})", language.default_name(), language.name(default))
}

fn selected_language(host: &dyn Host, menu: &mut Menu, slot: usize, language: Language, default: Language) {
    let name = if language.is_server_default() { default_language_name(language, default) }
        else { language.name(language).to_owned() };
    let title = format!("&7{} &a{name}", language.message("languages.selected-language"));
    let incomplete = format!("&e&o{}", language.message("work-in-progress"));
    let change = format!("&7⇨ &e{}", language.message("languages.change"));
    let mut lore = vec!["", &incomplete, ""];
    lore.extend(language.lines("languages.description"));
    lore.extend(["", &change]);
    menu.set(slot, language.flag().and_then(|flag| language_icon(host, flag, &title, &lore)),
        Some(Action::Push(Page::Languages)));
}

fn language_picker(host: &dyn Host, menu: &mut Menu, language: Language, default: Language) {
    let back = format!("&e⇦ {}", language.message("back.title"));
    let settings = format!("&7{}", language.message("back.settings"));
    menu.set(1, icon(host, "minecraft:enchanted_book", &back, &["", &settings]), Some(Action::Back));
    let help = format!("&7⇨ &e{}", language.message("languages.translations.lore"));
    menu.set(7, locale::help_flag().and_then(|flag| {
        language_icon(host, flag, language.message("languages.translations.name"), &["", &help])
    }), Some(Action::Link("https://github.com/Slimefun/Slimefun4/wiki/Translating-Slimefun")));
    let default_title = format!("&7{}", default_language_name(language, default));
    let select = format!("&7⇨ &e{}", language.message("languages.select-default"));
    menu.set(9, locale::default_flag().and_then(|flag| language_icon(host, flag, &default_title, &["", &select])),
        Some(Action::SelectLanguage(None)));
    let select = format!("&7⇨ &e{}", language.message("languages.select"));
    for (slot, choice) in (10..54).zip(Language::all()) {
        let name = format!("&a{}", language.name(choice));
        let progress = format!("&b{}%", choice.progress());
        menu.set(slot, choice.flag().and_then(|flag| language_icon(host, flag, &name, &[&progress, "", &select])),
            Some(Action::SelectLanguage(Some(choice))));
    }
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
    fn the_active_guide_language_changes_copies_and_leaves_stock_items_unchanged() {
        mod_api::__private::initialize().unwrap();
        let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse("minecraft:player_head").unwrap()])
            .unwrap();
        let entry = lookup("POWER_CRYSTAL").unwrap();
        let original = entry.item.kind.stack(&host, 1).unwrap();
        let name = |stack: &ItemStack| {
            let name = stack.components().unwrap().as_nbt().get(&"minecraft:custom_name".into()).unwrap();
            crate::network_runtime::paper_text(name).unwrap()
        };
        let chinese = entry_icon(&host, entry, 1, Language::CHINESE).unwrap();
        let english = entry_icon(&host, entry, 1, Language::ENGLISH).unwrap();
        assert!(name(&english).contains("Power Crystal"));
        assert!(name(&chinese).contains("能量水晶"));
        assert_eq!(crate::items::item_id(&english).as_deref(), Some(entry.id));
        assert_eq!(name(&entry.item.kind.stack(&host, 1).unwrap()), name(&original));
        assert_eq!(
            original.components().unwrap().as_nbt().get(&"minecraft:profile".into()),
            english.components().unwrap().as_nbt().get(&"minecraft:profile".into()),
        );
    }

    #[test]
    fn output_chest_keeps_paper_lore_style_on_items_and_in_the_guide() {
        mod_api::__private::initialize().unwrap();
        let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse("minecraft:chest").unwrap()])
            .unwrap();

        let lore = |lines: &[&str]| Tag::List(lines.iter().map(|line| colored_lore(line)).collect());
        assert_eq!(
            crate::machine_lore::lore("OUTPUT_CHEST"),
            Some(lore(&[
                "",
                "&c&oA basic machine will try to put",
                "&c&oitems in this chest if it's placed",
                "&c&oadjacent to the dispenser.",
            ])),
        );
        let expected = lore(&[
            "",
            "&c&o如果此箱子紧邻发射器，",
            "&c&o基础机器会尝试将产物",
            "&c&o放入此箱子。",
        ]);
        let entry = lookup("OUTPUT_CHEST").unwrap();
        let page = render(&host, None, &Page::Recipe(entry), 1, false, available(), false);
        let item = crate::items::OUTPUT_CHEST.stack(&host, 1).unwrap();
        for stack in [&item, page.slots[RECIPE_OUTPUT_SLOT].as_ref().unwrap()] {
            let components = stack.components().unwrap().as_nbt();
            assert_eq!(crate::items::item_id(stack).as_deref(), Some("OUTPUT_CHEST"));
            assert_eq!(
                components.get(&"minecraft:custom_name".into()),
                Some(&colored_lore("&4物品输出箱")),
            );
            assert_eq!(components.get(&"minecraft:lore".into()), Some(&expected));
        }
    }

    #[test]
    fn the_enhanced_crafting_table_badge_uses_the_whole_paper_translation() {
        mod_api::__private::initialize().unwrap();
        let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse("minecraft:crafting_table").unwrap()])
            .unwrap();
        let entry = lookup("AUTO_DRIER").unwrap();
        let badge = method_icon(&host, entry, Language::CHINESE).unwrap();
        let components = badge.components().unwrap().as_nbt();
        assert_eq!(
            components.get(&"minecraft:custom_name".into()),
            Some(&colored_lore("&b增强型工作台 (Enhanced Crafting Table)")),
        );
        assert_eq!(
            components.get(&"minecraft:lore".into()),
            Some(&Tag::List(
                ["&7如合成表所示", "&7在增强型工作台中合成.", "&7一个普通的工作台远远不够!"]
                    .into_iter()
                    .map(colored_lore)
                    .collect(),
            )),
        );
    }

    #[test]
    fn the_gold_pan_badge_localizes_a_copy_of_the_original_device() {
        mod_api::__private::initialize().unwrap();
        let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
        let mut host = FakeHost::new();
        let material = crate::items::GOLD_PAN.material();
        host.registries.resolve_items([Identifier::parse(material).unwrap()]).unwrap();
        let original = crate::items::GOLD_PAN.stack(&host, 1).unwrap();
        let badge = method_icon(&host, lookup("SIFTED_ORE").unwrap(), Language::CHINESE).unwrap();
        assert_eq!(badge.item(), original.item());
        assert_eq!(crate::items::item_id(&badge).as_deref(), Some("GOLD_PAN"));
        let components = badge.components().unwrap().as_nbt();
        assert_eq!(
            components.get(&"minecraft:custom_name".into()),
            Some(&colored_lore("&b淘金盘 (Gold Pan)")),
        );
        assert_ne!(badge.components(), original.components());
    }

    #[test]
    fn strange_nether_goo_recipe_badge_keeps_papers_chinese_text_and_english_fallback() {
        mod_api::__private::initialize().unwrap();
        let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse("minecraft:gold_ingot").unwrap()])
            .unwrap();
        let entry = lookup("STRANGE_NETHER_GOO").unwrap();
        let page = render(&host, None, &Page::Recipe(entry), 1, false, available(), false);
        let shown = page.slots[RECIPE_METHOD_SLOT].as_ref().unwrap();
        let mut fallback_entry = *entry;
        fallback_entry.recipe_type.locale_key = Some("missing_translation");
        let fallback = method_icon(&host, &fallback_entry, Language::CHINESE).unwrap();
        for (badge, name, lines) in [
            (
                shown,
                "&b猪灵交易掉落",
                ["&7使用金锭与猪灵交易", "&7可以获得此物品"],
            ),
            (
                &fallback,
                "&bPiglin Bartering Drop",
                ["&7Barter with Piglins using", "&7Gold Ingots to obtain this Item"],
            ),
        ] {
            assert_eq!(badge.item().id(&host), "minecraft:gold_ingot");
            let components = badge.components().unwrap().as_nbt();
            assert_eq!(
                components.get(&"minecraft:custom_name".into()),
                Some(&colored_lore(name)),
            );
            let expected = Tag::List(lines.iter().map(|line| colored_lore(line)).collect());
            assert_eq!(components.get(&"minecraft:lore".into()), Some(&expected));
        }
    }

    #[test]
    fn plain_command_names_keep_the_translation_of_each_formatted_run() {
        let id = "CARGO_NODE_INPUT";
        assert_eq!(locale::item_name(id), Some("&7货运节点 &c(输入)"));
        assert_eq!(
            localized_plain_item_name(id).as_deref(),
            Some("货运节点 (输入)")
        );
        assert_eq!(localized_plain_item_name("UNRECOGNIZED_ITEM"), None);
    }

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
        let mut tagged = book.clone();
        tagged
            .apply_components(&Compound::from([(
                "minecraft:custom_data".into(),
                Tag::compound([(
                    "PublicBukkitValues",
                    Tag::compound([
                        (GUIDE_KEY, Tag::String(SURVIVAL.into())),
                        ("test:preserved", Tag::Int(42)),
                    ]),
                )]),
            )]))
            .unwrap();
        let cheat = switched_book(&host, tagged.clone()).unwrap();
        assert_eq!(book_mode(&host, &cheat), Some(CHEAT));
        assert!(is_guide_book(&host, &cheat));
        let parts = cheat.components().unwrap().as_nbt();
        assert_eq!(parts.get(&"minecraft:custom_name".into()), Some(&colored_lore(CHEAT_BOOK_NAME)));
        assert_eq!(
            parts.get(&"minecraft:lore".into()),
            Some(&Tag::List(CHEAT_BOOK_LORE.iter().map(|line| colored_lore(line)).collect())),
        );
        assert_eq!(switched_book(&host, cheat), Some(tagged));
    }

    #[test]
    fn cheat_denial_uses_the_players_paper_language_and_the_configured_prefix() {
        let mut host = FakeHost::new();
        let guide = Guide::default();
        let options = toml::from_str::<toml::Value>("chat-prefix = \"&aTest &7> \"").unwrap();
        guide.configure_languages(Some(&options));
        let level = host.level.level();
        for (uuid, code, expected) in [
            (1, "en", "&aTest &7> &4You do not have the required permission to do this"),
            (2, "zh-CN", "&aTest &7> &4你没有足够的权限这么做"),
        ] {
            let profile = host.players.join(uuid, uuid as u32, level);
            profile.persistent_data.insert(
                "PlayerPersisted".into(),
                Tag::compound([("slimefun:language", Tag::String(code.into()))]),
            );
            let language = guide.player_language(&host, host.player(uuid).unwrap());
            assert_eq!(
                guide.prefixed_message(language.command_message("no-permission")),
                colored_label(expected),
            );
        }
    }

    #[test]
    fn the_cheat_menu_exposes_seasonal_and_research_locked_items_without_a_settings_link() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                [
                    "minecraft:gray_stained_glass_pane",
                    "minecraft:comparator",
                    "minecraft:player_head",
                    "minecraft:potion",
                    "minecraft:barrier",
                ]
                .map(|id| Identifier::parse(id).unwrap()),
            )
            .unwrap();
        let mode = |page: &Page, cheat| render(&host, None, page, 1, cheat, available(), cheat);
        let normal = mode(&Page::Main, false);
        let creative = mode(&Page::Cheat, true);
        let christmas = |menu: &Menu| {
            menu.actions.iter().any(
                |action| matches!(action, Some(Action::Push(Page::Group(group, 1))) if group.key == "christmas"),
            )
        };
        assert!(!christmas(&normal));
        assert!(christmas(&creative));
        assert!(matches!(normal.actions[1], Some(Action::Push(Page::Info))));
        assert!(creative.actions[1].is_none());
        let search = Page::Search(Arc::from("CHRISTMAS_MILK"));
        assert!(
            mode(&search, true)
                .actions
                .iter()
                .any(|action| matches!(action, Some(Action::Grant(entry)) if entry.id == "CHRISTMAS_MILK"))
        );
        let useful = GROUPS.iter().find(|group| group.key == "usefulItems").unwrap();
        let group = Page::Group(useful, 1);
        assert!(matches!(mode(&group, false).actions[9], Some(Action::Research(_))));
        assert!(matches!(
            mode(&group, true).actions[9],
            Some(Action::Grant(entry)) if entry.id == "PORTABLE_CRAFTER"
        ));
    }

    #[test]
    fn multiblock_inventory_templates_keep_paper_metadata_and_stay_out_of_vanilla_crafting() {
        mod_api::__private::initialize().unwrap();
        let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
        let entries: Vec<_> = GROUPS.iter()
            .flat_map(group_entries)
            .filter(|entry| entry.multiblock.is_some())
            .collect();
        assert_eq!(entries.len(), 15);
        let mut host = FakeHost::new();
        host.registries.resolve_items(
            entries.iter().map(|entry| Identifier::parse(entry.item.kind.material()).unwrap()),
        ).unwrap();

        for entry in entries {
            let template = entry.multiblock.unwrap();
            let granted = inventory_template(&host, entry).unwrap();
            assert_eq!(granted.item().id(&host), entry.item.kind.material());
            assert_eq!(granted.count(), 1);
            assert_eq!(crate::items::item_id(&granted).as_deref(), Some(entry.id));
            let components = granted.components().unwrap().as_nbt();
            assert_eq!(components.get(&"minecraft:custom_name".into()), Some(&colored_lore(template.name)));
            assert_eq!(
                components.get(&"minecraft:lore".into()),
                Some(&Tag::List(template.lore.iter().map(|line| colored_lore(line)).collect())),
            );
            let preview = entry_icon(&host, entry, 1, Language::CHINESE).unwrap();
            let localized = preview.components().unwrap().as_nbt();
            assert_ne!(localized.get(&"minecraft:lore".into()), components.get(&"minecraft:lore".into()), "{}", entry.id);
            assert!(
                crate::items::item_id(&preview).is_none(),
                "Guide previews never become inventory templates."
            );
            assert!(!crate::items::usable_in_workbench(&granted), "{}", entry.id);
            assert!(blocks_vanilla_station(&granted), "{}", entry.id);

            let plain = entry.item.kind.stack(&host, 1).unwrap();
            assert!(crate::items::usable_in_workbench(&plain), "{}", entry.id);
            assert!(!blocks_vanilla_station(&plain), "{}", entry.id);
        }
    }

    #[test]
    fn held_multiblock_templates_follow_their_paper_research_without_disabling_plain_materials() {
        mod_api::__private::initialize().unwrap();
        let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
        let mut host = FakeHost::new();
        host.registries.resolve_items(
            ["minecraft:crafting_table", "minecraft:dispenser"].map(|id| Identifier::parse(id).unwrap()),
        ).unwrap();
        let level = host.level.level();
        let player_id = 1;
        host.players.join(player_id, 100, level);
        let player = host.player(player_id).unwrap();
        let entry = lookup("GRIND_STONE").unwrap();
        let grinder = inventory_template(&host, entry).unwrap();
        let plain = entry.item.kind.stack(&host, 1).unwrap();
        let enhanced = inventory_template(&host, lookup("ENHANCED_CRAFTING_TABLE").unwrap()).unwrap();
        let guide = Guide::default();
        let mut without_loaded_profile = FakeHost::new();
        assert!(guide.refuses_multiblock_item_use(&mut without_loaded_profile, player, &grinder));
        assert!(!guide.refuses_multiblock_item_use(&mut host, player, &plain));
        assert!(!guide.refuses_multiblock_item_use(&mut host, player, &enhanced));

        let research_key = format!("slimefun:{}", entry.research.unwrap().id);
        host.players.get_mut(player_id).unwrap().persistent_data.insert(
            "PlayerPersisted".into(),
            Tag::compound([(research_key.as_str(), Tag::Byte(1))]),
        );
        assert!(!guide.refuses_multiblock_item_use(&mut host, player, &grinder));
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
    fn a_transferred_guide_is_initialized_once_even_when_the_first_grant_is_disabled() {
        mod_api::__private::initialize().unwrap();
        let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse("minecraft:enchanted_book").unwrap()])
            .unwrap();
        let level = host.level.level();
        let guide = Guide::default();
        for (owner, already_initialized, should_receive) in
            [(1, false, true), (2, true, false), (3, false, false)]
        {
            host.players
                .join(owner, u32::try_from(owner).unwrap(), level);
            let player = (&host as &dyn Host).player(owner).unwrap();
            let data = research::stage_replacement(
                &Compound::new(),
                &Default::default(),
                already_initialized,
            )
            .unwrap();
            player.set_persistent_data(&mut host, data).unwrap();
            if should_receive {
                host.frozen_player_handovers.insert(owner, 17);
                assert!(guide.initialize_transferred_guide(&mut host, player).is_err());
                assert!(host.players.get(owner).unwrap().slots.is_empty());
                host.frozen_player_handovers.remove(&owner);
            }
            if owner == 3 {
                let config = "[guide]\nreceive-on-first-join = false"
                    .parse::<toml::Table>()
                    .unwrap();
                guide.configure(config.get("guide"), None);
            }
            guide
                .initialize_transferred_guide(&mut host, player)
                .unwrap();
            assert!(research::sync_guide_already_issued(&host, player));
            guide.configure(None, None);
            guide
                .initialize_transferred_guide(&mut host, player)
                .unwrap();
            let slots = &host.players.get(owner).unwrap().slots;
            assert_eq!(slots.len(), usize::from(should_receive));
            if should_receive {
                assert!(is_book(&host, &slots[&0]));
                assert_eq!(slots[&0].count(), 1);
            }
        }
    }

    #[test]
    fn auto_drier_shows_paper_processing_rows_navigation_and_localized_lore() {
        mod_api::__private::initialize().unwrap();
        let mut host = FakeHost::new();
        let entry = lookup("AUTO_DRIER").unwrap();
        let icons = [
            "minecraft:gray_stained_glass_pane",
            "minecraft:black_stained_glass_pane",
            "minecraft:lime_stained_glass_pane",
        ];
        let process_materials = crate::thermal_machines::recipes(
            crate::advanced_processing::Family::AutoDrier,
            false,
        ).flat_map(|recipe| recipe.inputs.iter().chain(recipe.outputs).map(|stack| stack.kind.material()));
        host.registries.resolve_items(
            icons.into_iter()
                .chain(process_materials)
                .chain(entry.ingredients.iter().flatten().map(|stack| stack.kind.material()))
                .chain([entry.recipe_type.icon.material(), entry.item.kind.material()])
                .map(|id| Identifier::parse(id).unwrap()),
        ).unwrap();
        let first = render(&host, None, &Page::Recipe(entry), 1, false, available(), false);
        let slot_id = |menu: &Menu, slot: usize| menu.slots[slot].as_ref().unwrap().item().id(&host);
        assert_eq!(slot_id(&first, RECIPE_OUTPUT_SLOT), "minecraft:smoker");
        assert_eq!(slot_id(&first, 36), "minecraft:rotten_flesh");
        assert_eq!(slot_id(&first, 45), "minecraft:leather");
        assert_eq!(slot_id(&first, 44), "minecraft:cooked_porkchop");
        assert!(matches!(first.actions[44], Some(Action::FollowVanilla)));
        assert!(matches!(first.actions[INGREDIENT_SLOTS[4]], Some(Action::FollowVanilla)));
        let mut disabled = available();
        disabled.show_vanilla_recipes = false;
        let without_links = render(&host, None, &Page::Recipe(entry), 1, false, disabled, false);
        assert!(without_links.actions[44].is_none());
        assert!(without_links.actions[INGREDIENT_SLOTS[4]].is_none());
        assert_eq!(slot_id(&first, 28), "minecraft:black_stained_glass_pane");
        assert!(first.actions[28].is_none());
        let Some(Action::Replace(next @ Page::ProcessingRecipes(_, 1))) = &first.actions[34] else {
            panic!("the remaining processing pairs must have a second page");
        };
        let second = render(&host, None, next, 1, false, available(), false);
        assert_eq!(slot_id(&second, 36), "minecraft:cooked_chicken");
        let jerky = second.slots[45].as_ref().unwrap();
        assert_eq!(jerky.item().id(&host), "minecraft:cooked_chicken");
        assert_eq!(crate::items::item_id(jerky).as_deref(), Some("CHICKEN_JERKY"));
        assert_eq!(
            jerky.components().unwrap().as_nbt().get(&"minecraft:lore".into()),
            Some(&Tag::List(vec![colored_lore(""), colored_lore("&r横扫饥饿!")])),
        );
        assert_eq!(slot_id(&second, 34), "minecraft:black_stained_glass_pane");
        assert!(second.actions[34].is_none());
        assert!(matches!(second.actions[28], Some(Action::Replace(Page::ProcessingRecipes(_, 0)))));
        assert!(matches!(&second.actions[45], Some(Action::Push(Page::Recipe(recipe))) if recipe.id == "CHICKEN_JERKY"));
    }

    #[test]
    fn vanilla_recipe_detail_keeps_paper_slots_metadata_and_page_navigation() {
        mod_api::__private::initialize().unwrap();
        let mut host = FakeHost::new();
        host.registries.resolve_items([
            "minecraft:gray_stained_glass_pane", "minecraft:black_stained_glass_pane",
            "minecraft:lime_stained_glass_pane", "minecraft:cooked_porkchop", "minecraft:porkchop",
            "minecraft:furnace", "minecraft:smoker", "minecraft:campfire",
        ].map(|id| Identifier::parse(id).unwrap())).unwrap();
        let target = Kind::Vanilla("minecraft:cooked_porkchop").stack(&host, 1).unwrap();
        let item = ItemResource::of(&target);
        let mut recipes = Vec::new();
        for kind in ["smelting", "smoking", "campfire_cooking"] {
            let mut recipe = mod_api::Recipe::new(format!("test:{kind}"), format!("minecraft:{kind}"), Arc::new(serde_json::json!({})));
            recipe.ingredients.push(mod_api::Ingredient::of(vec!["minecraft:porkchop".into()]));
            recipes.push(vanilla::Match { recipe, result: target.clone() });
        }
        // Paint the test recipe list into the regular recipe-page shell.
        let mut shell = available();
        shell.show_vanilla_recipes = false;
        let page = Page::VanillaRecipe(item.clone(), 0);
        let mut menu = render(&host, None, &page, 1, false, shell, false);
        paint_vanilla_recipe(&host, &mut menu, &item, &recipes, 0, Language::CHINESE);
        for slot in [1, 3, 5, 7, 45, 53] {
            assert!(menu.slots[slot].is_none() && menu.actions[slot].is_none());
        }
        let id = |menu: &Menu, slot: usize| menu.slots[slot].as_ref().unwrap().item().id(&host);
        assert_eq!(id(&menu, INGREDIENT_SLOTS[4]), "minecraft:porkchop");
        assert_eq!(id(&menu, RECIPE_METHOD_SLOT), "minecraft:furnace");
        assert_eq!(menu.slots[RECIPE_OUTPUT_SLOT], Some(target));
        assert!(matches!(menu.actions[INGREDIENT_SLOTS[4]], Some(Action::FollowVanilla)));
        assert!(menu.actions[28].is_none());
        let Some(Action::Replace(Page::VanillaRecipe(item, index))) = &menu.actions[34] else {
            panic!("next cooking recipe must replace this history page");
        };
        let page = Page::VanillaRecipe(item.clone(), *index);
        let mut next = render(&host, None, &page, 1, false, shell, false);
        paint_vanilla_recipe(&host, &mut next, item, &recipes, *index, Language::CHINESE);
        assert_eq!(id(&next, RECIPE_METHOD_SLOT), "minecraft:smoker");
        assert!(matches!(next.actions[28], Some(Action::Replace(Page::VanillaRecipe(_, 0)))));
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
        assert_eq!(shown(RECIPE_METHOD_SLOT).as_deref(), Some("minecraft:bricks"));
        assert_eq!(shown(INGREDIENT_SLOTS[4]).as_deref(), Some("minecraft:crafting_table"));
        assert_eq!(shown(RECIPE_OUTPUT_SLOT).as_deref(), Some("minecraft:crafting_table"));
        assert_eq!(shown(INGREDIENT_SLOTS[7]).as_deref(), Some("minecraft:dispenser"));
        assert_eq!(shown(6), None);
        assert_eq!(shown(45), None);
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
        let output = recipe.slots[RECIPE_OUTPUT_SLOT].as_ref().unwrap();
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
        let displayed = entry_icon(&host, entry, 1, Language::CHINESE).unwrap();
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
    fn vampires_blade_localization_retains_the_paper_style_and_enchantments() {
        mod_api::__private::initialize().unwrap();
        let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
        let mut host = FakeHost::new();
        let entry = lookup("BLADE_OF_VAMPIRES").unwrap();
        host.registries
            .resolve_items([Identifier::parse(entry.item.kind.material()).unwrap()])
            .unwrap();
        let actual = entry.item.kind.stack(&host, 1).unwrap();
        let icon = entry_icon(&host, entry, 1, Language::CHINESE).unwrap();
        let parts = actual.components().unwrap().as_nbt();
        assert_eq!(
            parts.get(&"minecraft:custom_name".into()),
            Some(&colored_lore("&c吸血鬼之刀")),
        );
        assert_eq!(
            parts.get(&"minecraft:lore".into()),
            Some(&Tag::List(vec![
                colored_lore("&7生命偷取 I"),
                Tag::String("".into()),
                colored_lore("&f每次攻击目标时"),
                colored_lore("&f有 45% 的几率"),
                colored_lore("&f恢复 2 颗心的生命值"),
            ])),
        );
        assert_eq!(
            parts.get(&"minecraft:enchantments".into()),
            Some(&Tag::compound([
                ("minecraft:fire_aspect", Tag::Int(2)),
                ("minecraft:unbreaking", Tag::Int(4)),
                ("minecraft:sharpness", Tag::Int(2)),
            ])),
        );
        assert_eq!(icon.components().unwrap().as_nbt(), parts);
    }

    #[test]
    fn special_bow_lore_preserves_the_paper_line_order_and_duration() {
        mod_api::__private::initialize().unwrap();
        let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
        let descriptions = [
            (
                "EXPLOSIVE_BOW",
                &["&f用这把弓射出的箭", "&f会将命中的敌人击飞"][..],
            ),
            (
                "ICY_BOW",
                &[
                    "&f用这把弓射出的箭",
                    "&f会使被命中的敌人无法移动",
                    "&f持续 2 秒",
                ][..],
            ),
        ];
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse("minecraft:bow").unwrap()])
            .unwrap();
        for (id, lines) in descriptions {
            let stack = entry_icon(&host, lookup(id).unwrap(), 1, Language::CHINESE).unwrap();
            let lore = stack
                .components()
                .unwrap()
                .as_nbt()
                .get(&"minecraft:lore".into());
            let expected = Tag::List(lines.iter().map(|line| colored_lore(line)).collect());
            assert_eq!(lore, Some(&expected), "{id}");
        }
    }

    #[test]
    fn every_talisman_guide_entry_localizes_all_of_its_paper_lore() {
        use crate::network_runtime::paper_text;

        mod_api::__private::initialize().unwrap();
        let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse("minecraft:emerald").unwrap()])
            .unwrap();
        let entries: Vec<_> = GROUPS
            .iter()
            .filter(|group| matches!(group.key, "talismans" | "ender_talismans"))
            .flat_map(group_entries)
            .collect();
        assert_eq!(entries.len(), 32);
        for entry in entries {
            let Some(Tag::List(original)) = crate::talismans::lore(entry.id) else {
                panic!("{} has no original lore", entry.id);
            };
            let icon = entry_icon(&host, entry, 1, Language::CHINESE).unwrap();
            let Some(Tag::List(localized)) = icon
                .components()
                .unwrap()
                .as_nbt()
                .get(&"minecraft:lore".into())
            else {
                panic!("{} has no guide lore", entry.id);
            };
            assert_eq!(localized.len(), original.len(), "{}", entry.id);
            for (source, target) in original.iter().zip(localized) {
                let source = paper_text(source).unwrap();
                let target = paper_text(target).unwrap();
                if source.is_empty() {
                    assert!(target.is_empty(), "{}", entry.id);
                } else {
                    assert!(
                        target.chars().any(|ch| ('\u{4e00}'..='\u{9fff}').contains(&ch)),
                        "{}: {source} -> {target}",
                        entry.id
                    );
                }
            }
        }
    }

    #[test]
    fn energy_network_items_keep_the_paper_lore_on_stacks_and_guide_icons() {
        use crate::items::{ENERGY_CONNECTOR, ENERGY_REGULATOR};
        use crate::network_runtime::paper_text;

        mod_api::__private::initialize().unwrap();
        let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse("minecraft:player_head").unwrap()])
            .unwrap();
        let descriptions = [
            (ENERGY_REGULATOR, &["", "&r能源网络的核心部分"][..]),
            (
                ENERGY_CONNECTOR,
                &[
                    "&7范围: &c6 格",
                    "",
                    "&f将它放在机器",
                    "&f与发电机之间，将它们",
                    "&f接入你的能源调节器。",
                ][..],
            ),
        ];
        for (item, expected) in descriptions {
            let stack = item.stack(&host, 1).unwrap();
            let guide = entry_icon(&host, lookup(item.id()).unwrap(), 1, Language::CHINESE).unwrap();
            let wanted: Vec<_> = expected
                .iter()
                .map(|line| paper_text(&colored_lore(line)).unwrap())
                .collect();
            for stack in [stack, guide] {
                let parts = stack.components().unwrap();
                let Some(Tag::List(lore)) = parts.as_nbt().get(&"minecraft:lore".into()) else {
                    panic!("missing lore on {}", item.id());
                };
                let actual: Vec<_> = lore.iter().map(|line| paper_text(line).unwrap()).collect();
                assert_eq!(actual, wanted, "{}", item.id());
            }
        }
    }

    #[test]
    fn cargo_items_keep_the_paper_lore_and_existing_chinese_on_stacks_and_guide_icons() {
        use crate::network_recipes::{cargo_lore, network_item};

        mod_api::__private::initialize().unwrap();
        let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse("minecraft:player_head").unwrap()])
            .unwrap();
        for (id, original, translated) in [
            (
                "CARGO_MANAGER",
                "&fCore Component of an Item Transport Network",
                "&r物品传输网络的核心组件",
            ),
            ("CARGO_NODE", "&fCargo Connector Pipe", "&r货运连接管道"),
            ("CARGO_NODE_INPUT", "&fCargo Input Pipe", "&r货运输入管道"),
            ("CARGO_NODE_OUTPUT", "&fCargo Output Pipe", "&r货运输出管道"),
            (
                "CARGO_NODE_OUTPUT_ADVANCED",
                "&fCargo Output Pipe",
                "&r货运输出管道",
            ),
        ] {
            assert_eq!(
                cargo_lore(id),
                Some(Tag::List(vec![colored_lore(""), colored_lore(original)])),
                "{id}"
            );
            let stack = network_item(id).unwrap().stack(&host, 1).unwrap();
            let guide = entry_icon(&host, lookup(id).unwrap(), 1, Language::CHINESE).unwrap();
            let localized = Tag::List(vec![colored_lore(""), colored_lore(translated)]);
            for stack in [stack, guide] {
                let components = stack.components().unwrap();
                assert_eq!(
                    components.as_nbt().get(&"minecraft:lore".into()),
                    Some(&localized),
                    "{id}"
                );
            }
        }
    }

    #[test]
    fn electrical_items_preserve_paper_machine_details_on_stacks_and_guide_icons() {
        mod_api::__private::initialize().unwrap();
        let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                ["minecraft:furnace", "minecraft:crafting_table", "minecraft:red_terracotta"]
                    .map(|material| Identifier::parse(material).unwrap()),
            )
            .unwrap();
        let cases = [
            (
                "ELECTRIC_ORE_GRINDER_2",
                &[
                    "",
                    "&r矿物粉碎机与磨石的完美结合",
                    "",
                    "&4终局机器",
                    "&8⇨ &b⚡ &7速度: &b4.0x",
                    "&8⇨ &e⚡ &730 J/s",
                ][..],
            ),
            (
                "ELECTRIC_INGOT_FACTORY_3",
                &["", "&4终局机器", "&8⇨ &b⚡ &7速度: &b8.0x", "&8⇨ &e⚡ &740 J/s"][..],
            ),
            (
                "CHARGING_BENCH",
                &[
                    "",
                    "&r能够给物品充电, 比如喷气背包",
                    "",
                    "&e基础机器",
                    "&8⇨ &e⚡ &7128 J 缓冲",
                    "&8⇨ &e⚡ &7能源损失率: &c50%",
                ][..],
            ),
        ];
        for (id, lore) in cases {
            let entry = lookup(id).unwrap();
            let real = entry.item.kind.stack(&host, 1).unwrap();
            let icon = entry_icon(&host, entry, 1, Language::CHINESE).unwrap();
            let expected = Tag::List(lore.iter().copied().map(colored_lore).collect());
            for stack in [real, icon] {
                assert_eq!(
                    stack.components().unwrap().as_nbt().get(&"minecraft:lore".into()),
                    Some(&expected),
                    "{id}"
                );
            }
        }
    }

    #[test]
    fn capacitor_lore_keeps_the_paper_range_tier_and_capacity_on_each_stack() {
        use crate::network_runtime::paper_text;

        mod_api::__private::initialize().unwrap();
        let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse("minecraft:player_head").unwrap()])
            .unwrap();
        for (id, tier, capacity) in [
            ("SMALL_CAPACITOR", "&e基础电容", 128),
            ("MEDIUM_CAPACITOR", "&6普通电容", 512),
            ("BIG_CAPACITOR", "&a中级电容", 1_024),
            ("LARGE_CAPACITOR", "&2优质电容", 8_192),
            ("CARBONADO_EDGED_CAPACITOR", "&4终局电容", 65_536),
            ("ENERGIZED_CAPACITOR", "&4终局电容", 524_288),
        ] {
            let entry = lookup(id).unwrap();
            let actual = entry.item.kind.stack(&host, 1).unwrap();
            let guide = entry_icon(&host, entry, 1, Language::CHINESE).unwrap();
            let expected = [
                "&7范围: &c6 格",
                "",
                tier,
                &format!("&8⇨ &e⚡ &7{capacity} J 可储存"),
            ]
            .map(|line| paper_text(&colored_lore(line)).unwrap());
            for stack in [actual, guide] {
                let parts = stack.components().unwrap();
                let Some(Tag::List(lore)) = parts.as_nbt().get(&"minecraft:lore".into()) else {
                    panic!("missing lore on {id}");
                };
                let observed: Vec<_> = lore.iter().map(|line| paper_text(line).unwrap()).collect();
                assert_eq!(observed, expected, "{id}");
            }
        }
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
            (INGREDIENT_SLOTS[3], "smooth_stone_slab"),
            (INGREDIENT_SLOTS[4], "stonecutter"),
            (INGREDIENT_SLOTS[5], "smooth_stone_slab"),
            (INGREDIENT_SLOTS[7], "iron_block"),
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
        for (id, base, cost, icon, lore) in [
            (
                "INDUSTRIAL_MINER",
                "minecraft:iron_block",
                28,
                "minecraft:golden_pickaxe",
                &[
                    "",
                    "&r工业矿机能够开采其下方 7x7",
                    "&r的范围内的所有矿物.",
                    "&r将煤炭或其他燃料放入",
                    "&r机器的箱子中为它添加燃料.",
                ][..],
            ),
            (
                "ADVANCED_INDUSTRIAL_MINER",
                "minecraft:diamond_block",
                36,
                "minecraft:diamond_pickaxe",
                &[
                    "",
                    "&r进阶工业矿机能够开采其下方 11x11",
                    "&r的范围中的所有矿物.",
                    "&r将燃油或岩浆放入机器的箱子中",
                    "&r为它添加燃料.",
                    "",
                    "&a+ 精准采集",
                ][..],
            ),
        ] {
            let miner = lookup(id).unwrap();
            assert_eq!(miner.research.unwrap().cost, cost);
            let shown = render(&host, None, &Page::Recipe(miner), 1, false, available(), false);
            assert_eq!(
                shown.slots[INGREDIENT_SLOTS[4]].as_ref().unwrap().item().id(&host),
                "minecraft:chest"
            );
            assert_eq!(shown.slots[INGREDIENT_SLOTS[6]].as_ref().unwrap().item().id(&host), base);
            assert_eq!(
                shown.slots[INGREDIENT_SLOTS[7]].as_ref().unwrap().item().id(&host),
                "minecraft:blast_furnace"
            );
            assert_eq!(shown.slots[INGREDIENT_SLOTS[8]].as_ref().unwrap().item().id(&host), base);
            let depicted = shown.slots[RECIPE_OUTPUT_SLOT].as_ref().unwrap();
            assert_eq!(depicted.item().id(&host), icon);
            let components = depicted.components().unwrap();
            let expected = Tag::List(lore.iter().map(|line| colored_lore(line)).collect());
            assert_eq!(
                components.as_nbt().get(&"minecraft:lore".into()),
                Some(&expected)
            );
            assert!(crate::items::item_id(depicted).is_none());
            for slot in [INGREDIENT_SLOTS[3], INGREDIENT_SLOTS[5]] {
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
        let head = shown.slots[INGREDIENT_SLOTS[4]].as_ref().unwrap();
        assert_eq!(head.item().id(&host), "minecraft:player_head");
        assert!(
            head.components()
                .unwrap()
                .as_nbt()
                .contains_key(&"minecraft:profile".into())
        );
        assert_eq!(
            shown.slots[RECIPE_METHOD_SLOT].as_ref().unwrap().item().id(&host),
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
    }

    #[test]
    fn the_search_window_uses_papers_template_white_query_and_length_limit() {
        let title = search_title("§bCoal", Language::CHINESE);
        assert_eq!(title, colored_label("正在搜索: &fCoal"));
        assert_eq!(title.children[1].style.color.as_deref(), Some("white"));
        assert_eq!(
            search_title("0123456789012345678", Language::CHINESE),
            colored_label("正在搜索: &f0123456789012345678"),
        );
        assert_eq!(
            search_title("01234567890123456789", Language::CHINESE),
            colored_label("正在搜索: &f0123456789012345..."),
        );
        assert_eq!(
            search_title("Coal", Language::ENGLISH),
            colored_label("Searching for: &fCoal"),
        );
    }

    fn available() -> Availability<'static> {
        Availability {
            output_chest: true,
            tickers: Tickers::default(),
            dimension: None,
            machine_settings: crate::machine::Settings::default(),
            machine_recipes: None,
            show_vanilla_recipes: true,
            language: Language::CHINESE,
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
                ["minecraft:player_head", "minecraft:jack_o_lantern", "minecraft:firework_rocket"]
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
            ("birthday", 10, "生日蛋糕"),
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
                    presentation: Presentation::Book,
                    actions: std::array::from_fn(|_| None),
                    vanilla_rotation: None,
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
