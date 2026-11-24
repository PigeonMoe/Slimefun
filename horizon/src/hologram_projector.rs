// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::BTreeMap,
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use mod_api::{
    AttachmentCodec, AttachmentType, BlockChangedEvent, BlockDropsEvent, Compound,
    EntityJoinLevelEvent, EntityRef, Holder, Host, ItemPart, ItemStack, LevelHost, LevelRef,
    MenuClickedEvent, MenuClosedEvent, MenuReplace, OnBlockPlacedByEvent, PlayerRef, Registrar,
    RemovalReason, ServerHost, Tag, Text,
};

use crate::{
    items::{Kind, text},
    network_recipes::HOLOGRAM_PROJECTOR,
    network_runtime,
};

type Pos = [i32; 3];
pub(crate) const CARRIER: &str = "minecraft:quartz_slab";
const DEFAULT_LABEL: &str = "Edit me via the Projector";
const DEFAULT_OFFSET: i64 = 50;
const PROJECTOR_TAG: &str = "slimefun.projector.";
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);
static STATES: AttachmentType<Compound> = AttachmentType {
    namespace: "slimefun",
    name: "hologram_projectors",
    default: Compound::new,
    codec: Some(AttachmentCodec {
        encode: |data| Ok(data.clone()),
        decode: |data| Ok(data.clone()),
    }),
    copy_on_death: false,
    sync: false,
};
static RETIRED: Mutex<BTreeMap<(String, Pos), Retirement>> = Mutex::new(BTreeMap::new());

#[derive(Clone, Copy, PartialEq, Eq)]
enum Retirement {
    Removed,
    Dropped,
    Superseded,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Projector {
    owner: u128,
    generation: i64,
    label: String,
    offset: i64,
}

impl Projector {
    fn new(owner: u128, label: String, offset: i64, tick: i64) -> Self {
        Self {
            owner,
            generation: tick.rotate_left(32)
                ^ NEXT_GENERATION.fetch_add(1, Ordering::Relaxed) as i64,
            label,
            offset,
        }
    }

    fn encode(&self) -> Tag {
        Tag::compound([
            (
                "owner",
                Tag::String(format!("{:032x}", self.owner).as_str().into()),
            ),
            ("generation", Tag::Long(self.generation)),
            ("text", Tag::String(self.label.as_str().into())),
            ("offset", Tag::Long(self.offset)),
        ])
    }

    fn decode(value: &Tag) -> Option<Self> {
        let Tag::Compound(data) = value else {
            return None;
        };
        let Tag::String(owner) = data.get(&"owner".into())? else {
            return None;
        };
        let Tag::Long(generation) = data.get(&"generation".into())? else {
            return None;
        };
        let Tag::String(label) = data.get(&"text".into())? else {
            return None;
        };
        let Tag::Long(offset) = data.get(&"offset".into())? else {
            return None;
        };
        Some(Self {
            owner: network_runtime::parse_uuid(&owner.to_string().ok()?)?,
            generation: *generation,
            label: label.to_string().ok()?,
            offset: *offset,
        })
    }

    fn location(&self, pos: Pos) -> [f64; 3] {
        [
            f64::from(pos[0]) + 0.5,
            f64::from(pos[1]) + self.offset as f64 / 100.0,
            f64::from(pos[2]) + 0.5,
        ]
    }
}

fn key([x, y, z]: Pos) -> String {
    format!("{x},{y},{z}")
}
fn tag([x, y, z]: Pos) -> String {
    format!("{PROJECTOR_TAG}{x}.{y}.{z}")
}

fn tagged_anchor(tags: &[String]) -> Option<Pos> {
    let mut owned = tags.iter().filter(|tag| tag.starts_with(PROJECTOR_TAG));
    let value = owned.next()?;
    if owned.next().is_some() {
        return None;
    }
    let (x, yz) = value.strip_prefix(PROJECTOR_TAG)?.split_once('.')?;
    let (y, z) = yz.split_once('.')?;
    let pos = [x.parse().ok()?, y.parse().ok()?, z.parse().ok()?];
    (tag(pos) == *value).then_some(pos)
}

fn projection_stand(host: &dyn Host, entity: EntityRef) -> bool {
    entity
        .kind(host)
        .is_some_and(|kind| kind.id(host) == "minecraft:armor_stand")
        && entity.is_invisible(host) == Some(true)
        && entity.is_silent(host) == Some(true)
        && entity.no_gravity(host) == Some(true)
}

fn near_projection(here: [f64; 3], desired: [f64; 3]) -> bool {
    here.into_iter()
        .zip(desired)
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f64>()
        < 0.4
}

fn protect_projection(host: &mut dyn ServerHost, entity: EntityRef) {
    if entity.permanently_invulnerable(host) == Some(false) {
        let _ = entity.set_invulnerable(host, true);
    }
}

pub(crate) fn protect_loaded_projection(
    host: &mut dyn ServerHost,
    event: &EntityJoinLevelEvent<'_>,
) {
    if !event.loaded_from_disk || event.kind != "minecraft:armor_stand" {
        return;
    }
    if let Some(entity) = host.entity(event.entity)
        && let Some(pos) = entity.tags(host).and_then(|tags| tagged_anchor(&tags))
        && projection_stand(host, entity)
        && let Some(level) = entity.level(host)
        && let Some(record) = state(host, level, pos)
        && near_projection(event.pos, record.location(pos))
    {
        protect_projection(host, entity);
    }
}

fn chunk([x, _, z]: Pos) -> [i32; 2] {
    [x.div_euclid(16), z.div_euclid(16)]
}

fn state(host: &mut dyn ServerHost, level: LevelRef, pos: Pos) -> Option<Projector> {
    if !level.in_place(host)
        || level.block(host, pos)?.id(host) != CARRIER
        || RETIRED
            .lock()
            .unwrap()
            .contains_key(&(level.dimension().id(host)?, pos))
    {
        return None;
    }
    let data = STATES
        .get_existing(host, Holder::Chunk(chunk(pos)))
        .ok()
        .flatten()?;
    Projector::decode(data.get(&key(pos).as_str().into())?)
}

fn save(host: &mut dyn ServerHost, pos: Pos, value: Option<&Projector>) -> bool {
    let holder = Holder::Chunk(chunk(pos));
    let Ok(current) = STATES.get_existing(host, holder) else {
        return false;
    };
    let mut data = current.unwrap_or_default();
    if let Some(value) = value {
        data.insert(key(pos).as_str().into(), value.encode());
    } else if data.remove(&key(pos).as_str().into()).is_none() {
        return true;
    }
    STATES.set(host, holder, data).is_ok()
}

pub(crate) fn register(registrar: &mut dyn Registrar) {
    let carrier = registrar.item_key(CARRIER);
    registrar.item_part(carrier, Placement);
}

struct Placement;

#[mod_api::horizon_part]
impl ItemPart for Placement {
    fn on_block_placed_by(&self, host: &mut dyn ServerHost, event: &mut OnBlockPlacedByEvent<'_>) {
        if !HOLOGRAM_PROJECTOR.matches(host, event.stack) {
            return;
        }
        let Some(owner) = event.placer else { return };
        let Some(level) = owner.entity().level(host) else {
            return;
        };
        let Some(dimension) = level.dimension().id(host) else {
            return;
        };
        let record = Projector::new(
            owner.uuid(),
            DEFAULT_LABEL.into(),
            DEFAULT_OFFSET,
            level.game_time(host).unwrap_or_default(),
        );
        if save(host, event.pos, Some(&record)) {
            RETIRED.lock().unwrap().remove(&(dimension, event.pos));
            sync_projection(host, level, event.pos, &record);
        }
    }
}

pub(crate) fn observe_change(level: &dyn LevelHost, event: &BlockChangedEvent) {
    let was_slab = level.state_block(event.old) == Some(CARRIER);
    let is_slab = level.state_block(event.new) == Some(CARRIER);
    if was_slab == is_slab {
        return;
    }
    let pos = [event.pos.x, event.pos.y, event.pos.z];
    let dimension = network_runtime::dimension_name(&event.dimension);
    let mut retired = RETIRED.lock().unwrap();
    if is_slab {
        if let Some(change) = retired.get_mut(&(dimension.to_owned(), pos)) {
            *change = Retirement::Superseded;
        }
    } else if saved_at(level, pos) {
        retired
            .entry((dimension.to_owned(), pos))
            .or_insert(Retirement::Removed);
    }
}

fn saved_at(level: &dyn LevelHost, pos: Pos) -> bool {
    level
        .chunk_saved_attachment(chunk(pos), STATES.namespace, STATES.name)
        .is_some_and(|data| {
            data.get(&key(pos).as_str().into())
                .and_then(Projector::decode)
                .is_some()
        })
}

pub(crate) fn saved_marker(level: &dyn LevelHost, dimension: &str, pos: Pos) -> bool {
    saved_at(level, pos) && !RETIRED.lock().unwrap().contains_key(&(dimension.into(), pos))
}

pub(crate) fn restore_drop(level: &dyn LevelHost, event: &mut BlockDropsEvent<'_>) {
    let pos = [event.pos.x, event.pos.y, event.pos.z];
    if level.state_block(event.state) == Some(CARRIER)
        && saved_at(level, pos)
        && let Some(item) = level.item_key_by_name(CARRIER)
        && let Some(drop) = event
            .drops
            .iter_mut()
            .find(|drop| drop.stack.item() == item)
    {
        let dim = network_runtime::dimension_name(&event.dimension);
        let address = (dim.to_owned(), pos);
        let mut retired = RETIRED.lock().unwrap();
        if retired
            .get(&address)
            .is_some_and(|state| *state != Retirement::Removed)
        {
            return;
        }
        // Paper's SimpleBlockBreakHandler drops one projector even for a merged slab.
        if let Some(mut stack) = ItemStack::new(item, 1) {
            HOLOGRAM_PROJECTOR.decorate(&mut stack);
            drop.stack = stack;
            retired.insert(address, Retirement::Dropped);
        }
    }
}

pub(crate) fn tick(host: &mut dyn ServerHost, level: LevelRef) {
    let Some(dimension) = level.dimension().id(host) else {
        return;
    };
    let retired = RETIRED
        .lock()
        .unwrap()
        .keys()
        .filter_map(|(id, pos)| (id == &dimension).then_some(*pos))
        .collect::<Vec<_>>();
    for pos in retired {
        if level.is_loaded(host, pos) && save(host, pos, None) {
            remove_projection(host, level, pos);
            RETIRED.lock().unwrap().remove(&(dimension.clone(), pos));
        }
    }
}

pub(crate) fn remove_projection(host: &mut dyn ServerHost, level: LevelRef, pos: Pos) {
    if let Some(entity) = projection(host, level, pos) {
        entity.remove(host, RemovalReason::Discarded);
    }
}

fn projection(host: &dyn Host, level: LevelRef, pos: Pos) -> Option<EntityRef> {
    level.all_entities(host)?.into_iter().find(|&entity| {
        entity
            .tags(host)
            .and_then(|tags| tagged_anchor(&tags))
            == Some(pos)
            && projection_stand(host, entity)
    })
}

fn projection_properties(pos: Pos) -> Compound {
    let mut properties = Compound::from([(
        "Tags".into(),
        Tag::List(vec![Tag::String(tag(pos).as_str().into())]),
    )]);
    for key in [
        "Invisible",
        "Invulnerable",
        "Silent",
        "Marker",
        "NoGravity",
        "NoBasePlate",
        "PersistenceRequired",
    ] {
        properties.insert(key.into(), Tag::Byte(1));
    }
    properties
}

fn sync_projection(host: &mut dyn ServerHost, level: LevelRef, pos: Pos, state: &Projector) {
    let desired = state.location(pos);
    let name = colored_label(&state.label);
    let entity = projection(host, level, pos).or_else(|| {
        let dimension = level.dimension().id(host)?;
        host.summon(
            &dimension,
            "minecraft:armor_stand",
            desired,
            projection_properties(pos),
        )
        .and_then(|uuid| host.entity(uuid))
    });
    if let Some(entity) = entity {
        protect_projection(host, entity);
        if entity.position(host) != Some(desired) {
            let _ = entity.teleport_to(host, desired, None);
        }
        let _ = entity.set_custom_name(host, Some(name));
        entity.set_custom_name_visible(host, true);
    }
}

#[derive(Clone, Copy)]
struct Editor {
    level: LevelRef,
    pos: Pos,
    generation: i64,
}

#[derive(Default)]
pub(crate) struct Editors {
    active: Mutex<BTreeMap<u128, (i32, Editor)>>,
    waiting: Mutex<BTreeMap<u128, Editor>>,
}

impl Editors {
    pub(crate) fn use_at(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        level: LevelRef,
        pos: Pos,
    ) -> bool {
        let Some(record) = state(host, level, pos) else {
            return false;
        };
        if record.owner == player.uuid() {
            sync_projection(host, level, pos, &record);
            self.open(
                host,
                player,
                Editor {
                    level,
                    pos,
                    generation: record.generation,
                },
                &record,
            );
        }
        true
    }

    fn open(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        editor: Editor,
        record: &Projector,
    ) {
        let mut items = vec![None; 9];
        items[0] = button(
            host,
            "minecraft:name_tag",
            "&7Text &e(Click to edit)",
            vec![text("", "white", false), colored_lore(&record.label)],
        );
        let offset = format!("&7Offset: &e{}", display_offset(record.offset));
        items[1] = button(
            host,
            "minecraft:clock",
            &offset,
            vec![
                text("", "white", false),
                colored_lore("&fLeft Click: &7+0.1"),
                colored_lore("&fRight Click: &7-0.1"),
            ],
        );
        if let Ok(Some(window)) = host.open_menu(
            player.uuid(),
            1,
            Text::literal("Hologram Editor"),
            items,
            MenuReplace::CloseFirst,
        ) {
            self.active
                .lock()
                .unwrap()
                .insert(player.uuid(), (window, editor));
        }
    }

    pub(crate) fn clicked(&self, host: &mut dyn ServerHost, event: &MenuClickedEvent) {
        if !matches!(event.mode, 0 | 1)
            || !matches!(event.button, 0 | 1)
            || !matches!(event.slot, 0 | 1)
        {
            return;
        }
        let Some((_, editor)) = self
            .active
            .lock()
            .unwrap()
            .get(&event.uuid)
            .copied()
            .filter(|(id, _)| *id == event.window)
        else {
            return;
        };
        let Some(player) = host.player(event.uuid) else {
            return;
        };
        let Some(mut record) = editor.read(host, player) else {
            return;
        };
        if event.slot == 0 {
            host.close_menu(event.uuid);
            self.waiting.lock().unwrap().insert(event.uuid, editor);
            let _ = player.tell(host, Text::literal("Please enter your desired Hologram Text into your Chat. (Color Codes are supported!)"));
        } else {
            record.offset = record
                .offset
                .saturating_add(if event.button == 1 { -10 } else { 10 });
            if save(host, editor.pos, Some(&record)) {
                sync_projection(host, editor.level, editor.pos, &record);
                self.open(host, player, editor, &record);
            }
        }
    }

    pub(crate) fn take_chat(
        &self,
        host: &mut dyn ServerHost,
        player: u128,
        input: &str,
    ) -> Option<Text> {
        let editor = self.waiting.lock().unwrap().remove(&player)?;
        if let Some(player) = host.player(player)
            && let Some(mut record) = editor.read(host, player)
        {
            record.label = input.to_owned();
            if save(host, editor.pos, Some(&record)) {
                sync_projection(host, editor.level, editor.pos, &record);
                self.open(host, player, editor, &record);
                return Some(Text::literal(""));
            }
        }
        Some(Text::literal(
            "The hologram projector is no longer available.",
        ))
    }

    pub(crate) fn closed(&self, event: &MenuClosedEvent) {
        let mut active = self.active.lock().unwrap();
        if active
            .get(&event.uuid)
            .is_some_and(|(id, _)| *id == event.window)
        {
            active.remove(&event.uuid);
        }
    }

    pub(crate) fn logout(&self, uuid: u128) {
        self.active.lock().unwrap().remove(&uuid);
        self.waiting.lock().unwrap().remove(&uuid);
    }

    pub(crate) fn clear(&self) {
        self.active.lock().unwrap().clear();
        self.waiting.lock().unwrap().clear();
        RETIRED.lock().unwrap().clear();
    }
}

impl Editor {
    fn read(self, host: &mut dyn ServerHost, player: PlayerRef) -> Option<Projector> {
        if player.entity().level(host) != Some(self.level) {
            return None;
        }
        state(host, self.level, self.pos)
            .filter(|record| record.owner == player.uuid() && record.generation == self.generation)
    }
}

fn display_offset(offset: i64) -> String {
    let value = format!("{:.2}", offset.saturating_add(100) as f64 / 100.0);
    value.trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn button(
    host: &dyn Host,
    material: &'static str,
    label: &str,
    lore: Vec<Tag>,
) -> Option<ItemStack> {
    let mut stack = Kind::Vanilla(material).stack(host, 1)?;
    let mut components = Compound::new();
    components.insert("minecraft:custom_name".into(), colored_lore(label));
    components.insert("minecraft:lore".into(), Tag::List(lore));
    stack.set_components(components).ok()?;
    Some(stack)
}

pub(crate) fn colored_lore(label: &str) -> Tag {
    let extras = formatted_runs(label)
        .into_iter()
        .map(|(value, style)| {
            let mut tag = Compound::from([("text".into(), Tag::String(value.as_str().into()))]);
            if let Some(color) = style.color {
                tag.insert("color".into(), Tag::String(color.into()));
            }
            for (key, active) in [
                "obfuscated",
                "bold",
                "strikethrough",
                "underlined",
                "italic",
            ]
            .into_iter()
            .zip(style.flags)
            {
                if active {
                    tag.insert(key.into(), Tag::Byte(1));
                }
            }
            Tag::Compound(tag)
        })
        .collect();
    Tag::compound([
        ("text", Tag::String("".into())),
        ("extra", Tag::List(extras)),
        ("italic", Tag::Byte(0)),
    ])
}

pub(crate) fn marked(host: &mut dyn ServerHost, level: LevelRef, pos: Pos) -> bool {
    state(host, level, pos).is_some()
}

pub(crate) fn piston_protected(level: &dyn LevelHost, event: &mod_api::PistonPreEvent<'_>) -> bool {
    event.push.iter().chain(event.destroy.iter()).any(|pos| {
        let coords = [pos.x, pos.y, pos.z];
        level
            .block_state(coords)
            .and_then(|state| level.state_block(state))
            == Some(CARRIER)
            && saved_at(level, coords)
    })
}

pub(crate) fn colored_label(label: &str) -> Text {
    let mut result = Text::literal("");
    for (value, style) in formatted_runs(label) {
        let mut run = Text::literal(value);
        run.style.color = style.color.map(str::to_owned);
        let [obfuscated, bold, strikethrough, underlined, italic] = style.flags;
        run.style.obfuscated = Some(obfuscated);
        run.style.bold = Some(bold);
        run.style.strikethrough = Some(strikethrough);
        run.style.underlined = Some(underlined);
        run.style.italic = Some(italic);
        result.children.push(run);
    }
    result
}

pub(crate) fn plain_label(label: &str) -> String {
    formatted_runs(label)
        .into_iter()
        .map(|(value, _)| value)
        .collect()
}

#[derive(Clone, Copy, Default)]
struct LegacyStyle {
    color: Option<&'static str>,
    flags: [bool; 5],
}

fn formatted_runs(label: &str) -> Vec<(String, LegacyStyle)> {
    const COLORS: [&str; 16] = [
        "black",
        "dark_blue",
        "dark_green",
        "dark_aqua",
        "dark_red",
        "dark_purple",
        "gold",
        "gray",
        "dark_gray",
        "blue",
        "green",
        "aqua",
        "red",
        "light_purple",
        "yellow",
        "white",
    ];
    let mut chars = label.chars().peekable();
    let mut result = Vec::new();
    let mut style = LegacyStyle::default();
    let mut pending = String::new();
    while let Some(ch) = chars.next() {
        let code = if matches!(ch, '&' | '§') {
            chars.peek().map(|c| c.to_ascii_lowercase())
        } else {
            None
        };
        let Some(code @ ('0'..='9' | 'a'..='f' | 'k'..='o' | 'r')) = code else {
            pending.push(ch);
            continue;
        };
        chars.next();
        if !pending.is_empty() {
            result.push((std::mem::take(&mut pending), style));
        }
        if let Some(index) = code.to_digit(16) {
            style = LegacyStyle {
                color: Some(COLORS[index as usize]),
                ..LegacyStyle::default()
            };
        } else if code == 'r' {
            style = LegacyStyle::default();
        } else {
            style.flags[(code as u8 - b'k') as usize] = true;
        }
    }
    if !pending.is_empty() {
        result.push((pending, style));
    }
    result
}

pub(crate) fn restore_paper(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Pos,
    row: &toml::Table,
) -> Result<String, String> {
    if !level.in_place(host)
        || level
            .block(host, pos)
            .is_none_or(|block| block.id(host) != CARRIER)
    {
        return Err("Load the original quartz slab in the active dimension first.".into());
    }
    if state(host, level, pos).is_some() {
        return Ok("This Hologram Projector is already restored.".into());
    }
    let metadata = row
        .get("metadata")
        .and_then(toml::Value::as_table)
        .ok_or("The Paper Hologram Projector is missing metadata.")?;
    let field = |key| metadata.get(key).and_then(toml::Value::as_str);
    let owner = field("owner")
        .and_then(network_runtime::parse_uuid)
        .ok_or("The Paper Hologram Projector has no valid owner.")?;
    let label = field("text").unwrap_or(DEFAULT_LABEL);
    let offset = field("offset")
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|number| number.is_finite())
        .map_or(DEFAULT_OFFSET, |number| (number * 100.0).round() as i64);
    let record = Projector::new(
        owner,
        label.to_owned(),
        offset,
        level.game_time(host).unwrap_or_default(),
    );
    if !save(host, pos, Some(&record)) {
        return Err("Slimefun could not write the projector marker.".into());
    }
    let plain = formatted_runs(label)
        .into_iter()
        .map(|(text, _)| text)
        .collect::<String>();
    let desired = record.location(pos);
    if let Some(entity) = level
        .all_entities(host)
        .into_iter()
        .flatten()
        .find(|&entity| {
            projection_stand(host, entity)
                && entity.custom_name_visible(host) == Some(true)
                && entity.name(host).as_deref() == Some(plain.as_str())
                && entity
                    .position(host)
                    .is_some_and(|here| near_projection(here, desired))
                && entity.nbt(host).is_some_and(|data| {
                    matches!(data.get(&"BukkitValues".into()), Some(Tag::Compound(values))
                        if network_runtime::paper_hologram_id(values, pos))
                })
        })
    {
        entity.add_tag(host, &tag(pos));
    }
    sync_projection(host, level, pos, &record);
    Ok("Restored the Paper Hologram Projector.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_text_keeps_color_boundaries_and_treats_unknown_codes_as_text() {
        let runs = formatted_runs("&aGreen &l大字 §rplain &? &cRed");
        assert_eq!(
            runs.iter()
                .map(|(label, _)| label.as_str())
                .collect::<Vec<_>>(),
            ["Green ", "大字 ", "plain &? ", "Red"]
        );
        assert_eq!(runs[0].1.color, Some("green"));
        assert!(runs[1].1.flags[1]);
        assert_eq!(runs[2].1.color, None);
        assert!(!runs[2].1.flags[1]);
        assert_eq!(runs[3].1.color, Some("red"));
    }

    #[test]
    fn projector_state_round_trips_custom_labels_and_hundredth_block_offsets() {
        let record = Projector::new(3, "§b航点".into(), 37, 42);
        assert_eq!(Projector::decode(&record.encode()), Some(record.clone()));
        assert_eq!(record.location([-1, 64, 5]), [-0.5, 64.37, 5.5]);
        assert_eq!(display_offset(record.offset), "1.37");
        assert_eq!(display_offset(DEFAULT_OFFSET), "1.5");
        assert_eq!(display_offset(0), "1");
    }

    #[test]
    fn spawned_projection_is_invulnerable_and_uses_one_canonical_anchor() {
        let pos = [-1, 64, 5];
        let properties = projection_properties(pos);
        assert_eq!(properties.get(&"Invulnerable".into()), Some(&Tag::Byte(1)));
        assert_eq!(
            properties.get(&"Tags".into()),
            Some(&Tag::List(vec![Tag::String("slimefun.projector.-1.64.5".into())]))
        );
        assert_eq!(tagged_anchor(&[tag(pos), "paper".into()]), Some(pos));
        assert_eq!(tagged_anchor(&[tag(pos), tag([1, 64, 5])]), None);
        assert_eq!(tagged_anchor(&["slimefun.projector.-01.64.5".into()]), None);
        assert_eq!(tagged_anchor(&["slimefun.projector.-1.64.5.copy".into()]), None);
        assert_eq!(tagged_anchor(&["vanilla_marker".into()]), None);
    }
}
