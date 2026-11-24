// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::BTreeMap,
    sync::{LazyLock, Mutex},
};

use mod_api::{
    AttachmentStore, AttachmentType, BlockState, BlockUpdateFlags, CommandSource, Compound, Hand,
    Holder, Host, HostBlockStates, HostTransaction, Identifier, ItemPlace, ItemStack, LevelRef,
    PlayerRef, ScheduledTaskEvent, ServerHost, SoundSource, Tag, TaskId,
};

use crate::{
    admin_commands, factory,
    guide::Guide,
    hologram_projector::{self, colored_label},
    items::{HeadTexture, item},
    network_runtime,
    rainbow::placed as rainbow_blocks,
    timings::Profiler,
};

item!(
    DEBUG_FISH,
    "salmon",
    "How much is the Fish?",
    "dark_aqua",
    None,
    None
);

pub(crate) const LORE: &[&str] = &[
    "",
    "&eRight Click &fany Block to view it's BlockData",
    "&eLeft Click &fto break a Block",
    "&eShift + Left Click &fany Block to erase it's BlockData",
    "&eShift + Right Click &fto place a Placeholder Block",
];

type Position = [i32; 3];

const PLACEMENT_DELAY: u32 = 2;
const MISSING_TEXTURE: HeadTexture = HeadTexture {
    profile: [1778937101, 719665152, -1642229225, -221997007],
    value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2U5ZWI5ZGEyNmNmMmQzMzQxMzk3YTdmNDkxM2JhM2QzN2QxYWQxMGVhZTMwYWIyNWZhMzljZWI4NGJjIn19fQ==",
};

fn no_permission(host: &dyn Host, guide: &Guide, source: &CommandSource) -> mod_api::Text {
    colored_label(&format!(
        "{}{}",
        guide.chat_prefix(),
        guide
            .command_language(host, source)
            .command_message("no-permission"),
    ))
}

pub(crate) fn give(host: &mut dyn ServerHost, source: &CommandSource, guide: &Guide) {
    if let CommandSource::Player { player, .. } = *source
        && admin_commands::operator(host, player)
    {
        let language = guide.player_language(host, player);
        if let Some(stack) = DEBUG_FISH.guide_stack(host, 1, language) {
            let _ = admin_commands::insert(host, player, &stack, 1);
        }
    } else {
        let line = no_permission(host, guide, source);
        let _ = host.commands().feedback(line);
    }
}

fn tell_no_permission(host: &mut dyn ServerHost, player: PlayerRef, guide: &Guide) {
    let message = guide
        .player_language(host, player)
        .command_message("no-permission");
    let _ = player.tell(
        host,
        colored_label(&format!("{}{message}", guide.chat_prefix())),
    );
}

pub(crate) fn held(host: &dyn Host, hand: Hand, stack: Option<&ItemStack>) -> bool {
    hand == Hand::Main && stack.is_some_and(|stack| DEBUG_FISH.matches(host, stack))
}

pub(crate) fn in_air(host: &mut dyn ServerHost, player: PlayerRef, guide: &Guide) {
    if !admin_commands::operator(host, player) {
        tell_no_permission(host, player, guide);
    }
}

#[derive(Clone, Copy)]
struct Placement {
    player: PlayerRef,
    level: LevelRef,
    target: Position,
    expected: BlockState,
}

impl Placement {
    fn apply(self, host: &mut dyn ServerHost) {
        let Self {
            player,
            level,
            target,
            expected,
        } = self;
        let Some(head) = host.block_state_id("minecraft:player_head") else {
            return;
        };
        if !admin_commands::operator(host, player)
            || !level.allow_device_placement(host, player.uuid(), target)
            || level.block_state(host, target) != Some(expected)
        {
            return;
        }
        let placed = host.transaction(|host, _| {
            let flags = BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS;
            if expected != head && level.set_block(host, target, head, flags) != Ok(true) {
                return Err(());
            }
            let skull = host.block_entity(level, target).ok_or(())?;
            let profile = MISSING_TEXTURE.component();
            let applied =
                skull.merge_data(host, Compound::from([("profile".into(), profile.clone())]));
            if applied != Ok(true)
                && skull
                    .data(host)
                    .as_ref()
                    .and_then(|data| data.get(&"profile".into()))
                    != Some(&profile)
            {
                return Err(());
            }
            Ok(())
        });
        if placed.is_ok() {
            let _ = player.play_sound(
                host,
                "minecraft:block.bamboo.place",
                SoundSource::Blocks,
                1.0,
                1.0,
            );
        }
    }
}

#[derive(Default)]
struct Queue {
    pending: BTreeMap<TaskId, Placement>,
    ready: Vec<Placement>,
}

#[derive(Default)]
pub(crate) struct Fish(Mutex<Queue>);

impl Fish {
    pub(crate) fn right_click(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        click: &mod_api::BlockClick<'_>,
        guide: &Guide,
        profiler: &Profiler,
    ) {
        if !admin_commands::operator(host, player) {
            tell_no_permission(host, player, guide);
            return;
        }
        let Some(level) = player.entity().level(host) else {
            return;
        };
        if player.entity().is_crouching(host) != Some(true) {
            inspect(host, player, level, click.pos, profiler);
            return;
        }
        let Some(target) = relative(click.pos, click.hit.face) else {
            return;
        };
        if !level.allow_device_placement(host, player.uuid(), target) {
            return;
        }
        if let Some(expected) = level.block_state(host, target)
            && let Ok(task) = host.schedule_task(PLACEMENT_DELAY, 0)
        {
            self.0.lock().unwrap().pending.insert(
                task,
                Placement {
                    player,
                    level,
                    target,
                    expected,
                },
            );
        }
    }

    pub(crate) fn due(&self, host: &mut dyn ServerHost, event: &ScheduledTaskEvent<'_>) {
        if event.owner != "slimefun" {
            return;
        }
        let placement = self.0.lock().unwrap().pending.remove(&event.id);
        if let Some(placement) = placement {
            if placement.level.in_place(host) {
                placement.apply(host);
            } else {
                self.0.lock().unwrap().ready.push(placement);
            }
        }
    }

    pub(crate) fn tick(&self, host: &mut dyn ServerHost, level: LevelRef) {
        let ready: Vec<_> = self
            .0
            .lock()
            .unwrap()
            .ready
            .extract_if(.., |placement| placement.level == level)
            .collect();
        for placement in ready {
            placement.apply(host);
        }
    }

    pub(crate) fn unload(&self, host: &dyn Host, dimension: &str) {
        let mut queue = self.0.lock().unwrap();
        let other_level = |placement: &Placement| {
            placement.level.dimension().id(host).as_deref() != Some(dimension)
        };
        queue.pending.retain(|_, placement| other_level(placement));
        queue.ready.retain(other_level);
    }

    pub(crate) fn clear(&self) {
        *self.0.lock().unwrap() = Queue::default();
    }
}

fn relative(position: Position, face: Position) -> Option<Position> {
    if !matches!(face, [0, 1 | -1, 0] | [1 | -1, 0, 0] | [0, 0, 1 | -1]) {
        return None;
    }
    Some([
        position[0].checked_add(face[0])?,
        position[1].checked_add(face[1])?,
        position[2].checked_add(face[2])?,
    ])
}

pub(crate) fn left_click(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    pos: Position,
    guide: &Guide,
) -> bool {
    let hand = ItemPlace::PlayerHand {
        player: player.uuid(),
        hand: Hand::Main,
    };
    let stack = hand.stack(host);
    if !held(host, Hand::Main, stack.as_ref()) {
        return false;
    }
    if !admin_commands::operator(host, player) {
        tell_no_permission(host, player, guide);
        return true;
    }
    if player.entity().is_crouching(host) != Some(true) {
        return false;
    }
    if let Some(level) = player.entity().level(host)
        && level.allow_device_breaking(host, player.uuid(), pos)
    {
        erase(host, level, pos);
    }
    true
}

const ENTITY_STATE: &[&str] = &[
    "network_state",
    "block_placer",
    "output_chest",
    "reinforced_spawner",
    "placed_produce",
    "ancient_altar_device",
    "programmable_android",
    "electric_furnace",
    "auto_crafter",
    "trash_contents",
];

const CHUNK_STATE: &[&AttachmentType<Compound>] = &[
    &network_runtime::PLAIN_NODES,
    &factory::PLAIN_INVENTORIES,
    &rainbow_blocks::MARKERS,
    &hologram_projector::STATES,
];

fn position_key([x, y, z]: Position) -> String {
    format!("{x},{y},{z}")
}

fn chunk([x, _, z]: Position) -> [i32; 2] {
    [x.div_euclid(16), z.div_euclid(16)]
}

fn text_field(data: &Compound, key: &str) -> Option<String> {
    let Tag::String(value) = data.get(&key.into())? else {
        return None;
    };
    value.to_string().ok()
}

#[derive(Default)]
struct BlockData {
    id: String,
    data: Compound,
    inventory: bool,
    projection: bool,
}

fn entity_data(host: &dyn Host, level: LevelRef, pos: Position) -> Compound {
    let Some(entity) = host.block_entity(level, pos) else {
        return Compound::new();
    };
    ENTITY_STATE
        .iter()
        .filter_map(|&key| {
            host.block_entity_saved_attachment(entity, "slimefun", key)
                .map(|value| (key.into(), Tag::Compound(value)))
        })
        .collect()
}

fn chunk_data(
    host: &mut dyn ServerHost,
    pos: Position,
    kind: &AttachmentType<Compound>,
) -> Option<Tag> {
    let saved = kind.get_existing(host, Holder::Chunk(chunk(pos))).ok()??;
    saved.get(&position_key(pos).as_str().into()).cloned()
}

fn identify(host: &mut dyn ServerHost, level: LevelRef, pos: Position) -> Option<BlockData> {
    let mut fields = entity_data(host, level, pos);
    for kind in CHUNK_STATE {
        if let Some(value) = chunk_data(host, pos, kind) {
            fields.insert(kind.name.into(), value);
        }
    }
    let projection = hologram_projector::marked(host, level, pos);
    let id = network_runtime::placed_id(host, level, pos)
        .or_else(|| rainbow_blocks::placed_id(host, level, pos).map(str::to_owned))
        .or_else(|| projection.then(|| crate::network_recipes::HOLOGRAM_PROJECTOR.id().to_owned()))
        .or_else(|| {
            [
                ("output_chest", Some("OUTPUT_CHEST")),
                ("reinforced_spawner", Some("REINFORCED_SPAWNER")),
                ("block_placer", Some("BLOCK_PLACER")),
                ("placed_produce", None),
                ("ancient_altar_device", None),
            ]
            .into_iter()
            .find_map(|(key, fixed)| {
                let Tag::Compound(data) = fields.get(&key.into())? else {
                    return None;
                };
                fixed.map(str::to_owned).or_else(|| text_field(data, "id"))
            })
        })?;
    let inventory = fields.contains_key(&factory::PLAIN_INVENTORIES.name.into())
        || [
            "programmable_android",
            "electric_furnace",
            "auto_crafter",
            "trash_contents",
        ]
        .iter()
        .any(|key| fields.contains_key(&(*key).into()))
        || crate::storage::has_restorable_menu(crate::networks::Component::for_item(&id));
    // Paper prints BlockData and keeps its separate virtual inventories out of the chat report.
    for key in [
        factory::PLAIN_INVENTORIES.name,
        "electric_furnace",
        "trash_contents",
    ] {
        fields.remove(&key.into());
    }
    if let Some(Tag::Compound(android)) = fields.get_mut(&"programmable_android".into()) {
        android.remove(&"slots".into());
    }
    for name in ["network_state", network_runtime::PLAIN_NODES.name] {
        if let Some(Tag::Compound(network)) = fields.get_mut(&name.into())
            && let Some(Tag::Compound(cargo)) = network.get_mut(&"cargo".into())
        {
            cargo.remove(&"filters".into());
        }
    }
    fields.insert("id".into(), Tag::String(id.as_str().into()));
    Some(BlockData {
        id,
        data: fields,
        inventory,
        projection,
    })
}

fn erase(host: &mut dyn ServerHost, level: LevelRef, pos: Position) -> bool {
    let Some(data) = identify(host, level, pos) else {
        return false;
    };
    let entity = host.block_entity(level, pos);
    let viewers = entity
        .and_then(|entity| entity.viewers(host))
        .unwrap_or_default();
    if erase_saved_data(host, pos, entity.is_some()).is_err() {
        return false;
    }

    for player in viewers {
        host.close_menu(player.uuid());
    }
    factory::VIEWERS.close_at(host, level, pos);
    network_runtime::remove_network_status(host, level, pos);
    if data.projection {
        hologram_projector::remove_projection(host, level, pos);
    }
    true
}

fn erase_saved_data(
    host: &mut dyn ServerHost,
    pos: Position,
    has_entity: bool,
) -> Result<(), mod_api::AttachmentError> {
    host.transaction(|host, _| {
        if has_entity {
            let holder = Holder::BlockEntity(pos);
            for &key in ENTITY_STATE {
                if host.holds(holder, "slimefun", key)? {
                    host.discard(holder, "slimefun", key, false)?;
                }
            }
        }
        let holder = Holder::Chunk(chunk(pos));
        let key = position_key(pos);
        for kind in CHUNK_STATE {
            if let Some(mut saved) = kind.get_existing(host, holder)?
                && saved.remove(&key.as_str().into()).is_some()
            {
                kind.set(host, holder, saved)?;
            }
        }
        Ok::<_, mod_api::AttachmentError>(())
    })
}

fn inspect(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    level: LevelRef,
    pos: Position,
    profiler: &Profiler,
) {
    let Some(state) = level.block_state(host, pos) else {
        return;
    };
    let material = state.block().id(host);
    let label = material
        .strip_prefix("minecraft:")
        .unwrap_or(&material)
        .to_ascii_uppercase();
    let Some(data) = identify(host, level, pos) else {
        let tags = applicable_tags(host, state, &material);
        if !tags.is_empty() {
            let delim = std::iter::once(" ".to_owned());
            for line in delim
                .clone()
                .chain(std::iter::once(format!("&dSlimefun tags for: &e{label}")))
                .chain(
                    tags.into_iter()
                        .map(|tag| format!("&d* &e{}", tag.to_ascii_uppercase())),
                )
                .chain(delim)
            {
                let _ = player.tell(host, colored_label(&line));
            }
        }
        return;
    };

    let [x, y, z] = pos;
    let yes_no = |yes| if yes { "&2✔" } else { "&4✘" };
    let mut lines = vec![
        " ".into(),
        format!("&d{label} &e@ X: {x} Y: {y} Z: {z}"),
        format!("&dId: &e{}", data.id),
        "&dPlugin: &eSlimefun".into(),
    ];
    if matches!(label.as_str(), "PLAYER_HEAD" | "PLAYER_WALL_HEAD") {
        lines.push("&dSkull: &2✔".into());
        let (title, key) = if label == "PLAYER_HEAD" {
            ("Rotation", "rotation")
        } else {
            ("Facing", "facing")
        };
        if let Ok(Some(value)) = state.property(&host.block_states(), key) {
            lines.push(format!("  &d{title}: &e{value}"));
        }
    }
    lines.push(format!("&dInventory: {}", yes_no(data.inventory)));
    if crate::ticker_policy::has_block_ticker(&data.id) {
        lines.push("&dTicking: &2✔".into());
        lines.push("  &dAsync: &4✘".into());
    } else if network_runtime::debug_energy(&data.id).is_some_and(|(kind, _)| kind == "GENERATOR") {
        lines.push("&dTicking: &3Indirect (Generator)".into());
    } else {
        lines.push("&dTicking: &4✘".into());
    }
    if let Some((block, total, chunk)) = level
        .dimension()
        .id(host)
        .and_then(|dimension| profiler.block_timings(&dimension, &data.id, pos))
    {
        lines.push(format!("  &dTimings: &e{block}"));
        lines.push(format!("  &dTotal Timings: &e{total}"));
        lines.push(format!("  &dChunk Timings: &e{chunk}"));
    }
    if let Some((energy_type, capacity)) = network_runtime::debug_energy(&data.id) {
        lines.push("&dEnergyNet Component".into());
        lines.push(format!("  &dType: &e{energy_type}"));
        lines.push(format!("  &dChargeable: {}", yes_no(capacity > 0)));
        if capacity > 0 {
            let charge = network_runtime::charge(host, level, pos).unwrap_or(0);
            lines.push(format!("  &dEnergy: &e{charge} / {capacity}"));
        }
    }
    for line in lines {
        let _ = player.tell(host, colored_label(&line));
    }
    let details = serde_json::to_string(&json(&Tag::Compound(data.data))).unwrap_or_default();
    let mut payload = mod_api::Text::literal(details);
    payload.style.color = Some("gold".into());
    let _ = player.tell(host, payload);
    let _ = player.tell(host, mod_api::Text::literal(" "));
}

fn json(tag: &Tag) -> serde_json::Value {
    match tag {
        Tag::Byte(value) => (*value).into(),
        Tag::Short(value) => (*value).into(),
        Tag::Int(value) => (*value).into(),
        Tag::Long(value) => (*value).into(),
        Tag::Float(value) => f32::from_bits(*value).into(),
        Tag::Double(value) => f64::from_bits(*value).into(),
        Tag::String(value) => value.to_string_lossy().into(),
        Tag::List(values) => serde_json::Value::Array(values.iter().map(json).collect()),
        Tag::Compound(data) => serde_json::Value::Object(
            data.iter()
                .map(|(key, item)| (key.to_string_lossy(), json(item)))
                .collect(),
        ),
        Tag::ByteArray(values) => serde_json::Value::from(values.clone()),
        Tag::IntArray(values) => serde_json::Value::from(values.clone()),
        Tag::LongArray(values) => serde_json::Value::from(values.clone()),
        Tag::End => serde_json::Value::Null,
    }
}

type StockTags = Vec<(String, Vec<String>)>;
static STOCK_TAGS: LazyLock<StockTags> = LazyLock::new(|| {
    serde_json::from_str(include_str!("debug-fish-tags.json")).expect("generated Paper tags")
});

fn applicable_tags(host: &dyn Host, state: BlockState, material: &str) -> Vec<&'static str> {
    fn matches(host: &dyn Host, state: BlockState, material: &str, values: &[String]) -> bool {
        values.iter().any(|value| {
            if let Some(name) = value.strip_prefix("#slimefun:") {
                STOCK_TAGS
                    .iter()
                    .find(|(key, _)| key == name)
                    .is_some_and(|(_, values)| matches(host, state, material, values))
            } else if let Some(name) = value.strip_prefix("#minecraft:") {
                let id = Identifier::parse(&format!("minecraft:{name}")).expect("Paper tag id");
                let tag = host.tags().key(&id).and_then(|key| host.tags().items(key));
                if let Some(items) = tag.filter(|items| !items.is_empty()) {
                    Identifier::parse(material)
                        .ok()
                        .and_then(|id| host.item_key(&id))
                        .is_some_and(|item| items.contains(&item))
                } else {
                    host.block_in_tag(state.block(), &id.to_string()) == Some(true)
                }
            } else {
                value == material
            }
        })
    }

    STOCK_TAGS
        .iter()
        .filter_map(|(name, values)| {
            matches(host, state, material, values).then_some(name.as_str())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{AttachmentCodec, testing::FakeHost};

    #[test]
    fn debug_fish_uses_original_and_translated_item_components() {
        let mut host = FakeHost::new();
        let salmon = Identifier::parse("minecraft:salmon").unwrap();
        host.registries.resolve_items([salmon]).unwrap();
        let original = DEBUG_FISH
            .guide_stack(&host, 1, crate::guide::Language::ENGLISH)
            .unwrap();
        let translated = DEBUG_FISH
            .guide_stack(&host, 1, crate::guide::Language::CHINESE)
            .unwrap();
        assert!(held(&host, Hand::Main, Some(&original)));
        assert!(held(&host, Hand::Main, Some(&translated)));
        assert!(!held(&host, Hand::Off, Some(&translated)));

        for (stack, name, lore) in [
            (
                original,
                "How much is the Fish?",
                "Right Click any Block to view it's BlockData",
            ),
            (translated, "这鱼多少钱?", "右键 任意方块以查看它的方块数据"),
        ] {
            let components = stack.components().unwrap();
            let data = components.as_nbt();
            let text = |tag: &Tag| {
                hologram_projector::plain_label(&crate::network_runtime::paper_text(tag).unwrap())
            };
            let Tag::List(lines) = &data[&"minecraft:lore".into()] else {
                panic!("Paper fish has lore");
            };
            assert_eq!(lines.len(), LORE.len());
            assert!(text(&data[&"minecraft:custom_name".into()]).contains(name));
            assert!(text(&lines[1]).contains(lore));
        }
    }

    #[test]
    fn erasing_marker_preserves_carrier_and_another_positions_chunk_data() {
        static MARK: AttachmentType<Compound> = AttachmentType {
            namespace: "slimefun",
            name: "output_chest",
            default: Compound::new,
            codec: Some(AttachmentCodec {
                encode: |data| Ok(data.clone()),
                decode: |data| Ok(data.clone()),
            }),
            copy_on_death: false,
            sync: false,
        };
        let mut host = FakeHost::new();
        let level = host.level.level();
        let pos = [-17, 72, 18];
        let next = [-18, 72, 18];
        let tile = Holder::BlockEntity(pos);
        let column = Holder::Chunk(chunk(pos));
        let first = position_key(pos);
        let second = position_key(next);
        host.level
            .set_block(pos, "minecraft:chest")
            .block_entity(pos);
        MARK.set(&mut host as &mut dyn ServerHost, tile, Compound::new())
            .unwrap();
        let data = Compound::from([
            (first.as_str().into(), Tag::Int(1)),
            (second.as_str().into(), Tag::Int(2)),
        ]);
        factory::PLAIN_INVENTORIES
            .set(&mut host as &mut dyn ServerHost, column, data)
            .unwrap();
        let carrier = level.block_state(&host, pos);

        erase_saved_data(&mut host, pos, true).unwrap();

        assert!(!MARK.has(&mut host as &mut dyn ServerHost, tile).unwrap());
        let saved = factory::PLAIN_INVENTORIES
            .get_existing(&mut host as &mut dyn ServerHost, column)
            .unwrap()
            .unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved.get(&second.as_str().into()), Some(&Tag::Int(2)));
        assert!(host.level.block_entity_at(pos).is_some());
        assert_eq!(level.block_state(&host, pos), carrier);
    }

    #[test]
    fn placement_accepts_only_adjacent_block_faces_without_overflow() {
        assert_eq!(relative([10, 30, 14], [0, -1, 0]), Some([10, 29, 14]));
        assert_eq!(relative([10, 30, 14], [0, 0, 1]), Some([10, 30, 15]));
        assert_eq!(relative([10, 30, 14], [1, 0, 1]), None);
        assert_eq!(relative([i32::MAX, 0, 0], [1, 0, 0]), None);
    }
}
